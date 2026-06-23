use std::time::Instant;
use std::sync::Arc;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

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
    DnsServer { name: String, ip: String, domain: String },
    SystemDns { name: String, domains: Vec<String> },
}

struct LatencyTaskCtx {
    task: LatencyTask,
    bind_addr: Option<std::net::IpAddr>,
}

#[derive(Clone)]
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
    let client = match Client::new(&config) {
        Ok(c) => c,
        Err(e) => return Err(format!("创建ping客户端失败: {}", e)),
    };

    let timeout = std::time::Duration::from_millis(timeout_ms as u64);
    let deadline = std::time::Instant::now() + timeout;
    let mut total_ms = 0u64;
    let mut success_count = 0u32;

    let ident = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().subsec_nanos() & 0xFFFF) as u16;
    let mut pinger = client.pinger(addr, PingIdentifier(ident)).await;
    pinger.timeout(timeout);

    for seq in 0..3u16 {
        let remaining = deadline.checked_duration_since(std::time::Instant::now()).unwrap_or_default();
        if remaining.is_zero() { break; }
        match tokio::time::timeout(remaining, pinger.ping(PingSequence(seq), &[])).await {
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

async fn check_tcp_latency_async(host: &str, port: u16, timeout_ms: u64, bind_addr: Option<std::net::IpAddr>) -> i64 {
    if host.is_empty() {
        return -1;
    }
    let timeout_ms = timeout_ms.clamp(100, 30000);
    let start = Instant::now();
    let target = match format!("{}:{}", host, port).parse::<std::net::SocketAddr>() {
        Ok(a) => a,
        Err(_) => return -1,
    };
    let connect_result = if let Some(bind) = bind_addr {
        let bind_sock = std::net::SocketAddr::new(bind, 0);
        let socket = match target {
            std::net::SocketAddr::V4(_) => tokio::net::TcpSocket::new_v4(),
            std::net::SocketAddr::V6(_) => tokio::net::TcpSocket::new_v6(),
        };
        match socket {
            Ok(s) => match s.bind(bind_sock) {
                Ok(()) => tokio::time::timeout(
                    std::time::Duration::from_millis(timeout_ms),
                    s.connect(target),
                ).await,
                Err(_) => return -1,
            },
            Err(_) => return -1,
        }
    } else {
        tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            tokio::net::TcpStream::connect(&target),
        ).await
    };
    match connect_result {
        Ok(Ok(_)) => {
            let elapsed_us = start.elapsed().as_micros();
            ((elapsed_us + 500) / 1000).max(1) as i64
        }
        _ => -1,
    }
}

async fn tcp_then_icmp_latency(host: &str, ports: &[u16], tcp_timeout_ms: u64, bind_addr: Option<std::net::IpAddr>) -> (i64, &'static str) {
    let mut tcp_tasks = tokio::task::JoinSet::new();
    for &port in ports {
        let h = host.to_string();
        let ba = bind_addr;
        tcp_tasks.spawn(async move {
            check_tcp_latency_async(&h, port, tcp_timeout_ms, ba).await
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

async fn execute_task(ctx: LatencyTaskCtx, skip_ttfb: bool, skip_content: bool) -> LatencyResult {
    match ctx.task {
        LatencyTask::Gateway { name, target } => {
            let (lat, lat_type) = tcp_then_icmp_latency(&target, &[80, 53], 800, ctx.bind_addr).await;
            if lat < 0 {
                crate::log_warn!("quality", "网关测试失败 [{}]: {} (TCP和ICMP均不可达)", name, target);
            }
            LatencyResult { name, target, latency: lat, lat_type: lat_type.to_string(), is_external: false, dns_ms: -1, tcp_ms: -1, tls_ms: -1, udp_ms: -1, ttfb_ms: -1, content_ms: -1 }
        }
        LatencyTask::Doh { name, doh_server, doh_ip, doh_host } => {
            let r = crate::network::timing::measure_doh_timing(&doh_server, &doh_ip, &doh_host, ctx.bind_addr, std::time::Duration::from_millis(2000), skip_ttfb).await;
            let lat = if r.success { r.total_ms } else { -1 };
            if !r.success {
                crate::log_warn!("quality", "DoH测试失败 [{}]: {} - {}", name, doh_server, r.error.as_deref().unwrap_or("未知错误"));
            }
            crate::network::dns::update_doh_server_latency(&doh_server, lat, r.success);
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
            // HTTPS 测试不绑定适配器：校园网环境下绑定主适配器 IP 可能导致 TCP 连接外网超时
            // （主适配器路由表可能没有外网默认路由），让系统路由表决定出口网卡
            let r = crate::network::timing::measure_https_timing(&host, 443, None, std::time::Duration::from_secs(3), skip_ttfb, skip_content).await;
            let lat = if r.success { r.total_ms } else { -1 };
            if !r.success {
                crate::log_warn!("quality", "HTTPS测试失败 [{}]: {} - {}", name, r.url, r.error.as_deref().unwrap_or("未知错误"));
            }
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
        LatencyTask::DnsServer { name, ip, domain } => {
            let r = crate::network::timing::measure_dns_query(&ip, &domain, ctx.bind_addr, std::time::Duration::from_millis(3000)).await;
            let lat = if r.tcp_ms >= 0 { r.tcp_ms } else { r.udp_ms };
            if !r.success {
                crate::log_warn!("quality", "DNS服务器测试失败 [{}]: {}:{} - {}", name, ip, domain, r.error.as_deref().unwrap_or("未知错误"));
            }
            crate::network::dns::update_dns_server_latency(&ip, lat, r.success);
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
        LatencyTask::SystemDns { name, domains } => {
            // 并发解析所有域名，避免断网时串行超时导致首屏延迟膨胀（4域名串行可达12s）
            let mut set = tokio::task::JoinSet::new();
            for domain in domains.iter().cloned() {
                let bind_addr = ctx.bind_addr;
                set.spawn(async move {
                    let start = Instant::now();
                    let ok = crate::network::dns::resolve_host_smart(&domain, std::time::Duration::from_secs(3), bind_addr).await.is_ok();
                    (domain, ok, start.elapsed().as_millis() as i64)
                });
            }
            let mut latencies: Vec<i64> = Vec::new();
            let mut failed_domains: Vec<String> = Vec::new();
            while let Some(res) = set.join_next().await {
                if let Ok((domain, ok, elapsed)) = res {
                    if ok {
                        latencies.push(elapsed.max(1));
                    } else {
                        failed_domains.push(domain);
                    }
                }
            }
            let lat = if latencies.is_empty() {
                -1
            } else {
                latencies.iter().sum::<i64>() / latencies.len() as i64
            };
            if !failed_domains.is_empty() {
                crate::log_warn!("quality", "系统DNS测试失败 [{}]: {}/{}个域名解析失败 [{}]", name, failed_domains.len(), domains.len(), failed_domains.join(", "));
            }
            LatencyResult {
                name,
                target: format!("{}个域名", domains.len()),
                latency: lat,
                lat_type: "system-dns".to_string(),
                is_external: true,
                dns_ms: -1,
                tcp_ms: -1,
                tls_ms: -1,
                udp_ms: -1,
                ttfb_ms: -1,
                content_ms: -1,
            }
        }
    }
}

fn get_latency_level(latency: i64) -> usize {
    if latency < 0 { return 5; }
    if latency <= 20 { return 0; }
    if latency <= 50 { return 1; }
    if latency <= 100 { return 2; }
    if latency <= 200 { return 3; }
    if latency <= 400 { return 4; }
    5
}

fn build_quality_result(results: &[LatencyResult], gateway_str: &str, start: Instant) -> NetworkQualityResult {
    let mut details = serde_json::Map::new();
    let mut metrics = serde_json::Map::new();
    let mut external_values: Vec<i64> = Vec::new();
    let mut gateway_latency: i64 = -1;

    for r in results {
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
            "latency": r.latency, "type": r.lat_type, "elapsed": start.elapsed().as_millis()
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

    const LEVEL_NAMES: [&str; 6] = ["excellent", "great", "good", "fair", "poor", "bad"];

    let quality = if gateway_latency >= 0 && external_latency >= 0 {
        let level = std::cmp::max(get_latency_level(gateway_latency), get_latency_level(external_latency));
        LEVEL_NAMES.get(level).unwrap_or(&"unknown").to_string()
    } else if gateway_latency >= 0 {
        LEVEL_NAMES.get(get_latency_level(gateway_latency)).unwrap_or(&"unknown").to_string()
    } else if external_latency >= 0 {
        LEVEL_NAMES.get(get_latency_level(external_latency)).unwrap_or(&"unknown").to_string()
    } else {
        "unknown".to_string()
    };

    let total_elapsed = start.elapsed().as_millis() as u64;

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

pub async fn check_network_quality_async(_adapter_name: &str, adapter_ip: &str, skip_ttfb: bool, skip_content: bool, fixed_gateway: &str, is_quitting: Arc<std::sync::atomic::AtomicBool>, app_handle: Option<&AppHandle>) -> NetworkQualityResult {
    let now = Instant::now();

    let gateway = if !fixed_gateway.is_empty() {
        Some(fixed_gateway.to_string())
    } else {
        Some("10.2.127.254".to_string())
    };

    let gateway_str = gateway.as_deref().unwrap_or("");

    let bind_addr: Option<std::net::IpAddr> = if adapter_ip.is_empty() {
        None
    } else {
        match adapter_ip.parse() {
            Ok(addr) => Some(addr),
            Err(_) => {
                crate::log_warn!("quality", "适配器IP解析失败: {}", adapter_ip);
                None
            }
        }
    };

    let mut phase1_tasks: Vec<LatencyTaskCtx> = Vec::new();

    if let Some(ref gw) = gateway {
        phase1_tasks.push(LatencyTaskCtx {
            task: LatencyTask::Gateway {
                name: "网关".to_string(),
                target: gw.clone(),
            },
            bind_addr,
        });
    }

    phase1_tasks.push(LatencyTaskCtx { task: LatencyTask::DnsServer {
        name: "阿里DNS".to_string(),
        ip: "223.5.5.5".to_string(),
        domain: "www.baidu.com".to_string(),
    }, bind_addr });
    phase1_tasks.push(LatencyTaskCtx { task: LatencyTask::DnsServer {
        name: "腾讯DNS".to_string(),
        ip: "1.12.12.12".to_string(),
        domain: "www.baidu.com".to_string(),
    }, bind_addr });
    phase1_tasks.push(LatencyTaskCtx { task: LatencyTask::DnsServer {
        name: "信风DNS".to_string(),
        ip: "114.114.114.114".to_string(),
        domain: "www.baidu.com".to_string(),
    }, bind_addr });

    phase1_tasks.push(LatencyTaskCtx { task: LatencyTask::Doh {
        name: "阿里DoH".to_string(),
        doh_server: "dns.alidns.com".to_string(),
        doh_ip: "223.5.5.5".to_string(),
        doh_host: "baidu.com".to_string(),
    }, bind_addr });
    phase1_tasks.push(LatencyTaskCtx { task: LatencyTask::Doh {
        name: "腾讯DoH".to_string(),
        doh_server: "doh.pub".to_string(),
        doh_ip: "1.12.12.12".to_string(),
        doh_host: "baidu.com".to_string(),
    }, bind_addr });

    phase1_tasks.push(LatencyTaskCtx { task: LatencyTask::SystemDns {
        name: "DNS解析".to_string(),
        domains: vec![
            "www.baidu.com".to_string(),
            "www.bilibili.com".to_string(),
            "www.jd.com".to_string(),
            "cn.bing.com".to_string(),
        ],
    }, bind_addr });

    let mut phase1_set = tokio::task::JoinSet::new();
    for ctx in phase1_tasks {
        let st = skip_ttfb;
        let sc = skip_content;
        phase1_set.spawn(async move { execute_task(ctx, st, sc).await });
    }

    let mut phase1_results: Vec<LatencyResult> = Vec::new();
    while let Some(res) = phase1_set.join_next().await {
        if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
            phase1_set.abort_all();
            break;
        }
        if let Ok(r) = res {
            phase1_results.push(r);
        }
    }

    if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
        return NetworkQualityResult {
            gateway_latency: -1, external_latency: -1, average_external_latency: -1,
            gateway: gateway_str.to_string(), quality: "unknown".to_string(),
            timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
            details: serde_json::Value::Object(serde_json::Map::new()),
            metrics: serde_json::json!({ "totalElapsed": now.elapsed().as_millis() }),
        };
    }

    // 增量推送：Phase 1 完成后立即 emit，让前端先显示网关+DNS+DoH结果
    if let Some(ah) = app_handle {
        let mut partial = build_quality_result(&phase1_results, gateway_str, now);
        partial.quality = "busy".to_string();
        if let Ok(val) = serde_json::to_value(&partial) {
            if let Err(e) = ah.emit("network-quality-result", &val) {
                crate::log_warn!("quality", "[增量推送] Phase 1 emit 失败: {}", e);
            } else {
                crate::log_info!("quality", "[增量推送] Phase 1 emit 成功, details数={}", phase1_results.len());
            }
        }
    }

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

    // HTTPS 测试分批并发：每批最多 4 个，减少校园网环境下并发 TLS 握手竞争带宽导致延迟叠加
    const HTTPS_BATCH_SIZE: usize = 4;
    let mut phase2_results: Vec<LatencyResult> = Vec::new();
    for batch in https_hosts.chunks(HTTPS_BATCH_SIZE) {
        if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
            break;
        }
        let mut batch_set = tokio::task::JoinSet::new();
        for (name, host) in batch {
            let ctx = LatencyTaskCtx {
                task: LatencyTask::Https { name: name.to_string(), host: host.to_string() },
                bind_addr,
            };
            let st = skip_ttfb;
            let sc = skip_content;
            batch_set.spawn(async move { execute_task(ctx, st, sc).await });
        }
        while let Some(res) = batch_set.join_next().await {
            if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
                batch_set.abort_all();
                break;
            }
            if let Ok(r) = res {
                phase2_results.push(r);
            }
        }
        // 增量推送：每批 HTTPS 完成后 emit 累计结果
        if let Some(ah) = app_handle {
            let mut cumulative = phase1_results.clone();
            cumulative.extend(phase2_results.iter().cloned());
            let mut partial = build_quality_result(&cumulative, gateway_str, now);
            partial.quality = "busy".to_string();
            if let Ok(val) = serde_json::to_value(&partial) {
                if let Err(e) = ah.emit("network-quality-result", &val) {
                    crate::log_warn!("quality", "[增量推送] HTTPS 批次 emit 失败: {}", e);
                } else {
                    crate::log_info!("quality", "[增量推送] HTTPS 批次 emit 成功, 累计结果数={}, 耗时{}ms", cumulative.len(), now.elapsed().as_millis());
                }
            }
        }
    }

    let mut results: Vec<LatencyResult> = phase1_results;
    results.extend(phase2_results);

    let result = build_quality_result(&results, gateway_str, now);

    // 汇总：统计失败数量
    let failed_count = results.iter().filter(|r| r.latency < 0).count();
    let total_count = results.len();
    if failed_count > 0 {
        let failed_names: Vec<&str> = results.iter().filter(|r| r.latency < 0).map(|r| r.name.as_str()).collect();
        crate::log_warn!("quality", "网络质量检测完成: {}/{}项测试失败 [{}]", failed_count, total_count, failed_names.join(", "));
    } else {
        crate::log_info!("quality", "网络质量检测完成: 全部{}项测试成功, 耗时{}ms", total_count, now.elapsed().as_millis());
    }

    result
}
