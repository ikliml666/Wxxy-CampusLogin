use std::time::Instant;
use std::sync::Arc;
use serde::Serialize;

use super::adapter::get_gateway_ip_cached;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkQualityResult {
    pub gateway_latency: i64,
    pub external_latency: i64,
    pub average_external_latency: i64,
    pub gateway: String,
    pub quality: String,
    pub timestamp: u64,
    pub details: serde_json::Value,
    pub metrics: serde_json::Value,
}

enum LatencyTask {
    Gateway { name: String, target: String },
    Doh { name: String, doh_server: String, doh_ip: String, doh_host: String },
    Https { name: String, host: String },
    DnsResolve { name: String, target: String },
    DnsServer { name: String, ip: String, domain: String },
}

struct LatencyTaskCtx {
    task: LatencyTask,
    bind_addr: Option<std::net::IpAddr>,
}

struct LatencyResult {
    name: String,
    target: String,
    latency: i64,
    lat_type: String,
    is_external: bool,
    dns_ms: i64,
    tcp_ms: i64,
    tls_ms: i64,
    udp_ms: i64,
    ttfb_ms: i64,
    content_ms: i64,
}

async fn ping_host_async(host: &str, timeout_ms: u32) -> Result<u64, String> {
    use surge_ping::Client;
    use surge_ping::ICMP;
    use surge_ping::PingIdentifier;
    use surge_ping::PingSequence;

    let addr: std::net::IpAddr = host.parse().map_err(|_| format!("无效的主机地址: {}", host))?;
    let config = match addr {
        std::net::IpAddr::V4(_) => surge_ping::Config::default(),
        std::net::IpAddr::V6(_) => surge_ping::Config::builder().kind(ICMP::V6).build(),
    };
    let client = Client::new(&config).map_err(|e| format!("创建ping客户端失败: {}", e))?;

    let timeout = std::time::Duration::from_millis(timeout_ms as u64);
    let mut total_ms = 0u64;
    let mut success_count = 0u32;

    let ident = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().subsec_nanos() & 0xFFFF) as u16;
    let mut pinger = client.pinger(addr, PingIdentifier(ident)).await;
    pinger.timeout(timeout);

    for seq in 0..3u16 {
        match tokio::time::timeout(timeout, pinger.ping(PingSequence(seq), &[])).await {
            Ok(Ok((_, duration))) => {
                if seq > 0 {
                    total_ms += duration.as_millis() as u64;
                    success_count += 1;
                }
            }
            Ok(Err(_)) => continue,
            Err(_) => continue,
        }
    }

    if success_count > 0 {
        Ok((total_ms / success_count as u64).max(1))
    } else {
        Err("ping failed: 100% packet loss".to_string())
    }
}

async fn check_tcp_latency_async(host: &str, port: u16, timeout_ms: u64) -> i64 {
    if host.is_empty() {
        return -1;
    }
    let timeout_ms = timeout_ms.clamp(100, 30000);
    let start = Instant::now();
    let target = match format!("{}:{}", host, port).parse::<std::net::SocketAddr>() {
        Ok(a) => a,
        Err(_) => return -1,
    };
    match tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        tokio::net::TcpStream::connect(&target),
    ).await {
        Ok(Ok(_)) => {
            let elapsed_us = start.elapsed().as_micros();
            ((elapsed_us + 500) / 1000).max(1) as i64
        }
        _ => -1,
    }
}

async fn check_dns_latency_async(hostname: &str, timeout_ms: u64) -> i64 {
    let start = Instant::now();
    let addr = if hostname.contains(':') {
        format!("[{}]:0", hostname)
    } else {
        format!("{}:0", hostname)
    };
    let lookup = tokio::net::lookup_host(&addr);
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        lookup,
    ).await;
    match result {
        Ok(Ok(mut addrs)) => {
            if addrs.next().is_some() {
                let elapsed_us = start.elapsed().as_micros();
                let elapsed = ((elapsed_us + 500) / 1000) as i64;
                elapsed.max(1)
            } else {
                -1
            }
        }
        _ => -1,
    }
}

async fn tcp_then_icmp_latency(host: &str, ports: &[u16], tcp_timeout_ms: u64) -> (i64, &'static str) {
    let mut tcp_tasks = tokio::task::JoinSet::new();
    for &port in ports {
        let h = host.to_string();
        tcp_tasks.spawn(async move {
            check_tcp_latency_async(&h, port, tcp_timeout_ms).await
        });
    }
    while let Some(res) = tcp_tasks.join_next().await {
        if let Ok(lat) = res {
            if lat >= 0 {
                tcp_tasks.abort_all();
                return (lat, "tcp");
            }
        }
    }

    let host_icmp = host.to_string();
    let icmp_lat = ping_host_async(&host_icmp, 500)
        .await
        .map(|v| v as i64)
        .unwrap_or(-1i64);

    if icmp_lat >= 0 {
        (icmp_lat, "icmp")
    } else {
        (-1i64, "icmp")
    }
}

pub async fn check_network_quality_async(adapter_name: &str, adapter_ip: &str, skip_ttfb: bool, skip_content: bool, fixed_gateway: &str, is_quitting: Arc<std::sync::atomic::AtomicBool>) -> NetworkQualityResult {
    let now = Instant::now();

    let gateway = if !fixed_gateway.is_empty() {
        Some(fixed_gateway.to_string())
    } else {
        tokio::task::spawn_blocking({
            let an = adapter_name.to_string();
            let ai = adapter_ip.to_string();
            move || get_gateway_ip_cached(&an, &ai).ok()
        }).await.unwrap_or(None)
    };

    let gateway = gateway.filter(|gw| {
        !gw.starts_with("192.168.")
    });

    let gateway_str = gateway.as_deref().unwrap_or("");

    let bind_addr: Option<std::net::IpAddr> = if adapter_ip.is_empty() {
        None
    } else {
        adapter_ip.parse().ok()
    };

    let mut tasks: Vec<LatencyTaskCtx> = Vec::new();

    if let Some(ref gw) = gateway {
        tasks.push(LatencyTaskCtx {
            task: LatencyTask::Gateway {
                name: "网关".to_string(),
                target: gw.clone(),
            },
            bind_addr,
        });
    }

    tasks.push(LatencyTaskCtx { task: LatencyTask::Doh {
        name: "阿里DoH".to_string(),
        doh_server: "dns.alidns.com".to_string(),
        doh_ip: "223.5.5.5".to_string(),
        doh_host: "baidu.com".to_string(),
    }, bind_addr });
    tasks.push(LatencyTaskCtx { task: LatencyTask::Doh {
        name: "腾讯DoH".to_string(),
        doh_server: "doh.pub".to_string(),
        doh_ip: String::new(),
        doh_host: "baidu.com".to_string(),
    }, bind_addr });

    tasks.push(LatencyTaskCtx { task: LatencyTask::DnsResolve {
        name: "DNS解析".to_string(),
        target: "www.baidu.com".to_string(),
    }, bind_addr });

    tasks.push(LatencyTaskCtx { task: LatencyTask::DnsServer {
        name: "阿里DNS".to_string(),
        ip: "223.5.5.5".to_string(),
        domain: "www.baidu.com".to_string(),
    }, bind_addr });
    tasks.push(LatencyTaskCtx { task: LatencyTask::DnsServer {
        name: "腾讯DNS".to_string(),
        ip: "119.29.29.29".to_string(),
        domain: "www.baidu.com".to_string(),
    }, bind_addr });
    tasks.push(LatencyTaskCtx { task: LatencyTask::DnsServer {
        name: "信风DNS".to_string(),
        ip: "114.114.114.114".to_string(),
        domain: "www.baidu.com".to_string(),
    }, bind_addr });

    let https_hosts: &[(&str, &str)] = &[
        ("百度", "www.baidu.com"),
        ("京东", "www.jd.com"),
        ("必应", "cn.bing.com"),
        ("12306", "www.12306.cn"),
        ("英雄联盟", "lol.qq.com"),
        ("原神", "mhyy.mihoyo.com"),
        ("绝地求生", "pubg.qq.com"),
        ("永劫无间", "www.yjwujian.cn"),
        ("哔哩哔哩", "www.bilibili.com"),
        ("哔哩哔哩直播", "live.bilibili.com"),
        ("抖音", "www.douyin.com"),
        ("抖音直播", "live.douyin.com"),
    ];

    for (name, host) in https_hosts {
        tasks.push(LatencyTaskCtx { task: LatencyTask::Https {
            name: name.to_string(),
            host: host.to_string(),
        }, bind_addr });
    }

    let mut join_set = tokio::task::JoinSet::new();
    for ctx in tasks {
        join_set.spawn(async move {
            match ctx.task {
                LatencyTask::Gateway { name, target } => {
                    let (lat, lat_type) = tcp_then_icmp_latency(&target, &[80, 53], 800).await;
                    LatencyResult { name, target, latency: lat, lat_type: lat_type.to_string(), is_external: false, dns_ms: -1, tcp_ms: -1, tls_ms: -1, udp_ms: -1, ttfb_ms: -1, content_ms: -1 }
                }
                LatencyTask::Doh { name, doh_server, doh_ip, doh_host } => {
                    let r = crate::http_timing::measure_doh_timing(&doh_server, &doh_ip, &doh_host, ctx.bind_addr, std::time::Duration::from_millis(2000), skip_ttfb).await;
                    let lat = if r.success { r.total_ms } else { -1 };
                    LatencyResult {
                        name,
                        target: format!("https://{}", doh_server),
                        latency: lat,
                        lat_type: "doh".to_string(),
                        is_external: true,
                        dns_ms: r.dns_ms,
                        tcp_ms: r.tcp_ms,
                        tls_ms: r.tls_ms,
                        udp_ms: -1,
                        ttfb_ms: r.http_ms,
                        content_ms: -1,
                    }
                }
                LatencyTask::Https { name, host } => {
                    let r = crate::http_timing::measure_https_timing(&host, 443, ctx.bind_addr, std::time::Duration::from_secs(3), skip_ttfb, skip_content).await;
                    let lat = if r.success { r.total_ms } else { -1 };
                    LatencyResult {
                        name,
                        target: format!("https://{}", host),
                        latency: lat,
                        lat_type: "https".to_string(),
                        is_external: true,
                        dns_ms: r.dns_ms,
                        tcp_ms: r.tcp_ms,
                        tls_ms: r.tls_ms,
                        udp_ms: -1,
                        ttfb_ms: r.ttfb_ms,
                        content_ms: r.content_ms,
                    }
                }
                LatencyTask::DnsResolve { name, target } => {
                    let lat = check_dns_latency_async(&target, 800).await;
                    LatencyResult { name, target, latency: lat, lat_type: "dns".to_string(), is_external: true, dns_ms: -1, tcp_ms: -1, tls_ms: -1, udp_ms: -1, ttfb_ms: -1, content_ms: -1 }
                }
                LatencyTask::DnsServer { name, ip, domain } => {
                    let r = crate::http_timing::measure_dns_query(&ip, &domain, ctx.bind_addr, std::time::Duration::from_millis(1500)).await;
                    let lat = match (r.udp_ms, r.tcp_ms) {
                        (u, t) if u >= 0 && t >= 0 => u.min(t),
                        (u, _) if u >= 0 => u,
                        (_, t) => t,
                    };
                    LatencyResult {
                        name,
                        target: format!("{}:53", ip),
                        latency: lat,
                        lat_type: "dns".to_string(),
                        is_external: true,
                        dns_ms: -1,
                        tcp_ms: r.tcp_ms,
                        tls_ms: -1,
                        udp_ms: r.udp_ms,
                        ttfb_ms: -1,
                        content_ms: -1,
                    }
                }
            }
        });
    }

    let mut results: Vec<LatencyResult> = Vec::new();
    let mut gateway_latency: i64 = -1;
    while let Some(res) = join_set.join_next().await {
        if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
            join_set.abort_all();
            break;
        }
        if let Ok(r) = res {
            results.push(r);
        }
    }

    let mut details = serde_json::Map::new();
    let mut metrics = serde_json::Map::new();
    let mut external_values: Vec<i64> = Vec::new();

    for r in &results {
        if r.is_external && r.latency >= 0 {
            external_values.push(r.latency);
        }
        if r.name == "网关" {
            gateway_latency = r.latency;
        }
        let mut detail_json = serde_json::json!({
            "target": r.target, "latency": r.latency, "type": r.lat_type
        });
        if r.dns_ms >= 0 {
            detail_json["dnsLatency"] = serde_json::json!(r.dns_ms);
            detail_json["tcpLatency"] = serde_json::json!(r.tcp_ms);
            detail_json["tlsLatency"] = serde_json::json!(r.tls_ms);
            let segments = r.dns_ms.max(0) + r.tcp_ms.max(0) + r.tls_ms.max(0);
            if r.latency > segments {
                detail_json["networkLatency"] = serde_json::json!(r.latency - segments);
            }
        }
        if r.udp_ms >= 0 && r.lat_type == "dns" {
            detail_json["udpLatency"] = serde_json::json!(r.udp_ms);
        }
        if r.tcp_ms >= 0 && r.lat_type == "dns" {
            detail_json["tcpLatency"] = serde_json::json!(r.tcp_ms);
        }
        if r.ttfb_ms >= 0 {
            detail_json["ttfbLatency"] = serde_json::json!(r.ttfb_ms);
        }
        if r.content_ms >= 0 {
            detail_json["contentLatency"] = serde_json::json!(r.content_ms);
        }
        details.insert(r.name.clone(), detail_json);
        metrics.insert(r.name.clone(), serde_json::json!({
            "latency": r.latency, "type": r.lat_type, "elapsed": now.elapsed().as_millis()
        }));
    }

    let (external_latency, average_external_latency) = if !external_values.is_empty() {
        let mut sorted = external_values.clone();
        sorted.sort();
        let mid = sorted.len() / 2;
        let med = if sorted.len() % 2 != 0 {
            sorted[mid]
        } else {
            (sorted[mid - 1] + sorted[mid]) / 2
        };
        let trimmed = if sorted.len() >= 4 {
            let trim = (sorted.len() as f64 * 0.15).ceil() as usize;
            let trim = trim.min(sorted.len() / 2);
            sorted[trim..sorted.len() - trim].to_vec()
        } else if sorted.len() >= 3 {
            let mut v = sorted.clone();
            v.remove(0);
            v.remove(v.len() - 1);
            v
        } else {
            sorted.clone()
        };
        let avg = if !trimmed.is_empty() {
            trimmed.iter().sum::<i64>() / trimmed.len() as i64
        } else {
            sorted.iter().sum::<i64>() / sorted.len() as i64
        };
        (med.max(1), avg.max(1))
    } else {
        (-1, -1)
    };

    fn get_latency_level(latency: i64) -> usize {
        if latency < 0 { return 5; }
        if latency <= 20 { return 0; }
        if latency <= 50 { return 1; }
        if latency <= 100 { return 2; }
        if latency <= 200 { return 3; }
        if latency <= 400 { return 4; }
        5
    }
    let level_names = ["excellent", "great", "good", "fair", "poor", "bad"];

    let quality = if gateway_latency >= 0 && external_latency >= 0 {
        let level = std::cmp::max(get_latency_level(gateway_latency), get_latency_level(external_latency));
        level_names[level].to_string()
    } else if gateway_latency >= 0 {
        level_names[get_latency_level(gateway_latency)].to_string()
    } else if external_latency >= 0 {
        level_names[get_latency_level(external_latency)].to_string()
    } else {
        "unknown".to_string()
    };

    let total_elapsed = now.elapsed().as_millis() as u64;

    NetworkQualityResult {
        gateway_latency,
        external_latency,
        average_external_latency,
        gateway: gateway_str.to_string(),
        quality,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        details: serde_json::Value::Object(details),
        metrics: serde_json::json!({
            "totalElapsed": total_elapsed,
            "tests": serde_json::Value::Object(metrics),
        }),
    }
}
