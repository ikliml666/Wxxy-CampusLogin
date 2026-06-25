use std::net::IpAddr;
use std::time::{Duration, Instant};

lazy_static::lazy_static! {
    static ref DNS_CACHE: dashmap::DashMap<String, (IpAddr, Instant)> = dashmap::DashMap::new();

    static ref DNS_SERVER_SCORES: dashmap::DashMap<String, DnsServerScore> = dashmap::DashMap::new();
    static ref DOH_SERVER_SCORES: dashmap::DashMap<String, DohServerScore> = dashmap::DashMap::new();
}

const DNS_FALLBACK_SERVERS: &[&str] = &["223.5.5.5", "1.12.12.12", "114.114.114.114"];
const DOH_FALLBACK_SERVERS: &[(&str, &str)] = &[
    ("dns.alidns.com", "223.5.5.5"),
    ("doh.pub", "1.12.12.12"),
];

#[derive(Clone)]
struct DnsServerScore {
    latency_ms: i64,
    success: bool,
    last_tested: Instant,
}

#[derive(Clone)]
struct DohServerScore {
    latency_ms: i64,
    success: bool,
    last_tested: Instant,
}

pub fn update_dns_server_latency(ip: &str, latency_ms: i64, success: bool) {
    DNS_SERVER_SCORES.insert(ip.to_string(), DnsServerScore {
        latency_ms,
        success,
        last_tested: Instant::now(),
    });
}

pub fn update_doh_server_latency(server: &str, latency_ms: i64, success: bool) {
    DOH_SERVER_SCORES.insert(server.to_string(), DohServerScore {
        latency_ms,
        success,
        last_tested: Instant::now(),
    });
}

pub(crate) fn get_best_dns_servers() -> Vec<String> {
    let mut scored: Vec<(String, i64)> = DNS_SERVER_SCORES.iter()
        .filter(|e| e.value().success && e.value().last_tested.elapsed().as_secs() < 600)
        .map(|e| (e.key().clone(), e.value().latency_ms))
        .collect();

    if scored.is_empty() {
        return DNS_FALLBACK_SERVERS.iter().map(|s| s.to_string()).collect();
    }

    scored.sort_by_key(|(_, lat)| *lat);
    scored.into_iter().map(|(ip, _)| ip).collect()
}

pub(crate) fn get_best_doh_servers() -> Vec<(String, String)> {
    let mut scored: Vec<(String, i64, String)> = DOH_SERVER_SCORES.iter()
        .filter(|e| e.value().success && e.value().last_tested.elapsed().as_secs() < 600)
        .map(|e| {
            let fallback_ip = DOH_FALLBACK_SERVERS.iter()
                .find(|(name, _)| *name == e.key())
                .map(|(_, ip)| ip.to_string())
                .unwrap_or_default();
            (e.key().clone(), e.value().latency_ms, fallback_ip)
        })
        .collect();

    if scored.is_empty() {
        return DOH_FALLBACK_SERVERS.iter()
            .map(|(s, ip)| (s.to_string(), ip.to_string()))
            .collect();
    }

    scored.sort_by_key(|(_, lat, _)| *lat);
    scored.into_iter().map(|(name, _, ip)| (name, ip)).collect()
}

const DNS_CACHE_TTL_SECS: u64 = 60;
const DNS_CACHE_MAX_ENTRIES: usize = 64;

pub(crate) fn dns_cache_get(host: &str) -> Option<IpAddr> {
    DNS_CACHE.get(host).and_then(|entry| {
        if entry.value().1.elapsed().as_secs() < DNS_CACHE_TTL_SECS {
            Some(entry.value().0)
        } else {
            None
        }
    }).or_else(|| {
        DNS_CACHE.remove_if(host, |_, (_, ts)| ts.elapsed().as_secs() >= DNS_CACHE_TTL_SECS);
        None
    })
}

pub(crate) fn dns_cache_put(host: &str, ip: IpAddr) {
    let now = Instant::now();
    DNS_CACHE.insert(host.to_string(), (ip, now));
    DNS_CACHE.retain(|_, (_, ts)| now.saturating_duration_since(*ts).as_secs() < DNS_CACHE_TTL_SECS);
    while DNS_CACHE.len() > DNS_CACHE_MAX_ENTRIES {
        let oldest = DNS_CACHE.iter()
            .min_by_key(|e| e.value().1)
            .map(|e| e.key().clone());
        if let Some(key) = oldest {
            DNS_CACHE.remove(&key);
        } else {
            break;
        }
    }
}

pub fn cleanup_expired_dns_cache() {
    let now = Instant::now();
    DNS_CACHE.retain(|_, (_, ts)| now.saturating_duration_since(*ts).as_secs() < DNS_CACHE_TTL_SECS);
}

pub(crate) async fn resolve_host_uncached_with_bind(
    host: &str,
    timeout: Duration,
    bind_addr: Option<IpAddr>,
) -> Result<IpAddr, String> {
    let host = host.to_string();
    let result = tokio::task::spawn_blocking(move || {
        use hickory_resolver::config::*;
        use hickory_resolver::Resolver;

        let mut config = ResolverConfig::new();
        let servers = get_best_dns_servers();
        for server_ip in &servers {
            if let Ok(ip) = server_ip.parse::<IpAddr>() {
                config.add_name_server(NameServerConfig {
                    socket_addr: std::net::SocketAddr::new(ip, 53),
                    protocol: Protocol::Udp,
                    tls_dns_name: None,
                    trust_negative_responses: false,
                    bind_addr: bind_addr.map(|ip| std::net::SocketAddr::new(ip, 0)),
                });
            }
        }

        let mut opts = ResolverOpts::default();
        opts.try_tcp_on_error = true;
        opts.timeout = timeout;
        opts.attempts = 2;
        opts.num_concurrent_reqs = servers.len().min(3);

        let resolver = Resolver::new(config, opts)
            .map_err(|e| format!("创建解析器失败: {e}"))?;

        match resolver.lookup_ip(&host) {
            Ok(response) => {
                response.iter()
                    .find(|ip| ip.is_ipv4())
                    .or_else(|| response.iter().next())
                    .ok_or_else(|| "无DNS结果".to_string())
            }
            Err(_) => {
                let sys_config = ResolverConfig::default();
                let mut sys_opts = ResolverOpts::default();
                sys_opts.try_tcp_on_error = true;
                sys_opts.timeout = timeout;
                sys_opts.attempts = 2;
                sys_opts.num_concurrent_reqs = 2;

                let sys_resolver = Resolver::new(sys_config, sys_opts)
                    .map_err(|e| format!("创建系统解析器失败: {e}"))?;

                sys_resolver.lookup_ip(&host)
                    .map_err(|e| format!("{e}"))
                    .and_then(|response| {
                        response.iter()
                            .find(|ip| ip.is_ipv4())
                            .or_else(|| response.iter().next())
                            .ok_or_else(|| "系统DNS无结果".to_string())
                    })
            }
        }
    }).await;

    match result {
        Ok(Ok(ip)) => Ok(ip),
        Ok(Err(e)) => Err(e),
        Err(e) => Err(format!("解析任务失败: {e}")),
    }
}

pub(crate) async fn dns_lookup(
    server_ip: &str,
    domain: &str,
    bind_addr: Option<IpAddr>,
    timeout: Duration,
    protocol: hickory_resolver::config::Protocol,
) -> (Result<(), String>, i64) {
    use hickory_resolver::config::*;
    use hickory_resolver::Resolver;

    let start = Instant::now();
    let ip: IpAddr = match server_ip.parse() {
        Ok(ip) => ip,
        Err(e) => return (Err(format!("{e}")), -1),
    };
    let sock_addr = std::net::SocketAddr::new(ip, 53);
    let bind = bind_addr.map(|a| std::net::SocketAddr::new(a, 0));

    let mut resolver_config = ResolverConfig::new();
    resolver_config.add_name_server(NameServerConfig {
        socket_addr: sock_addr,
        protocol,
        tls_dns_name: None,
        trust_negative_responses: false,
        bind_addr: bind,
    });

    let mut opts = ResolverOpts::default();
    opts.try_tcp_on_error = true;
    opts.timeout = timeout;
    opts.attempts = 2;
    opts.num_concurrent_reqs = 1;

    let resolver = match Resolver::new(resolver_config, opts) {
        Ok(r) => r,
        Err(e) => return (Err(format!("创建解析器失败: {e}")), -1),
    };

    let domain = domain.to_string();
    match tokio::task::spawn_blocking(move || {
        resolver.lookup_ip(&domain)
            .map_err(|e| format!("{e}"))
    }).await {
        Ok(Ok(_)) => (Ok(()), ((start.elapsed().as_micros() + 500) / 1000).max(1) as i64),
        Ok(Err(e)) => (Err(e), ((start.elapsed().as_micros() + 500) / 1000).max(1) as i64),
        Err(e) => (Err(format!("任务执行失败: {e}")), ((start.elapsed().as_micros() + 500) / 1000).max(1) as i64),
    }
}

pub(crate) fn build_dns_query_wire(domain: &str, qtype: u16) -> Vec<u8> {
    let mut buf = Vec::with_capacity(64);
    let txid = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() & 0xFFFF) as u16;
    buf.extend_from_slice(&txid.to_be_bytes());
    buf.extend_from_slice(&0x0100u16.to_be_bytes());
    buf.extend_from_slice(&1u16.to_be_bytes());
    buf.extend_from_slice(&0u16.to_be_bytes());
    buf.extend_from_slice(&0u16.to_be_bytes());
    buf.extend_from_slice(&0u16.to_be_bytes());
    for label in domain.split('.') {
        let b = label.as_bytes();
        buf.push(b.len() as u8);
        buf.extend_from_slice(b);
    }
    buf.push(0);
    buf.extend_from_slice(&qtype.to_be_bytes());
    buf.extend_from_slice(&1u16.to_be_bytes());
    buf
}

pub(crate) fn base64url_encode_no_pad(data: &[u8]) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    base64::Engine::encode(&URL_SAFE_NO_PAD, data)
}

pub(crate) fn parse_dns_response_wire(data: &[u8]) -> Result<Vec<IpAddr>, String> {
    if data.len() < 12 {
        return Err("DNS响应太短".to_string());
    }
    let qdcount = u16::from_be_bytes([data[4], data[5]]) as usize;
    let ancount = u16::from_be_bytes([data[6], data[7]]) as usize;
    let mut pos = 12usize;
    for _ in 0..qdcount {
        pos = skip_dns_name(data, pos)?;
        pos += 4;
    }
    let mut ips = Vec::new();
    for _ in 0..ancount {
        pos = skip_dns_name(data, pos)?;
        if pos + 10 > data.len() { break; }
        let rtype = u16::from_be_bytes([data[pos], data[pos + 1]]);
        let rdlength = u16::from_be_bytes([data[pos + 8], data[pos + 9]]) as usize;
        pos += 10;
        if rtype == 1 && rdlength == 4 && pos + 4 <= data.len() {
            let ip = std::net::Ipv4Addr::new(data[pos], data[pos + 1], data[pos + 2], data[pos + 3]);
            ips.push(IpAddr::V4(ip));
        }
        pos += rdlength;
    }
    Ok(ips)
}

pub(crate) fn skip_dns_name(data: &[u8], mut pos: usize) -> Result<usize, String> {
    let mut jumped = false;
    let mut after_jump_pos: usize = 0;
    let mut seen = std::collections::HashSet::with_capacity(8);
    loop {
        if pos >= data.len() { return Err("DNS名称解析越界".to_string()); }
        let len = data[pos];
        if len == 0 {
            pos += 1;
            break;
        }
        if len >= 0xC0 {
            if pos + 1 >= data.len() { return Err("DNS压缩指针越界".to_string()); }
            if !jumped { after_jump_pos = pos + 2; }
            jumped = true;
            let offset = (((len as usize) & 0x3F) << 8) | (data[pos + 1] as usize);
            if !seen.insert(offset) { return Err("DNS压缩指针循环".to_string()); }
            pos = offset;
            continue;
        }
        pos += 1 + len as usize;
    }
    if jumped { Ok(after_jump_pos) } else { Ok(pos) }
}

pub(crate) async fn resolve_via_doh(
    doh_server: &str,
    doh_ip: IpAddr,
    domain: &str,
    bind_addr: Option<IpAddr>,
    timeout: Duration,
) -> Result<IpAddr, String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let deadline = std::time::Instant::now() + timeout;
    let addr = std::net::SocketAddr::new(doh_ip, 443);
    let tcp_stream = super::timing::bind_and_connect(addr, bind_addr, timeout).await
        .map_err(|e| format!("DoH TCP连接失败: {e}"))?;

    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
    let (mut tls_stream, _) = super::timing::do_tls_handshake(doh_server, tcp_stream, remaining).await
        .map_err(|e| format!("DoH TLS握手失败: {e}"))?;

    let query_wire = build_dns_query_wire(domain, 1);
    let dns_param = base64url_encode_no_pad(&query_wire);
    let path = format!("/dns-query?dns={dns_param}");
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {doh_server}\r\nAccept: application/dns-message\r\nUser-Agent: Mozilla/5.0\r\nConnection: close\r\n\r\n"
    );

    tls_stream.write_all(request.as_bytes()).await
        .map_err(|e| format!("DoH发送请求失败: {e}"))?;

    let mut response = Vec::new();
    let mut buf = vec![0u8; 4096];
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err("DoH读取超时".into());
        }
        match tokio::time::timeout(remaining, tls_stream.read(&mut buf)).await {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => {
                response.extend_from_slice(&buf[..n]);
                if response.len() > 64 * 1024 { break; }
            }
            Ok(Err(e)) => {
                if response.is_empty() {
                    return Err(format!("DoH读取响应失败: {e}"));
                }
                break;
            }
            Err(_) => {
                if response.is_empty() {
                    return Err("DoH响应超时".to_string());
                }
                break;
            }
        }
    }

    let header_end = response.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("DoH响应格式无效: 无HTTP头分隔")?;

    // 校验HTTP状态行（必须 200），防止 4xx/5xx 响应体被当作 DNS 报文解析
    let status_line_end = response.iter().position(|&b| b == b'\r').unwrap_or(header_end);
    let status_line = std::str::from_utf8(&response[..status_line_end]).unwrap_or("");
    if status_line.split_whitespace().nth(1) != Some("200") {
        return Err(format!("DoH响应状态异常: {}", status_line.trim()));
    }

    let body = &response[header_end + 4..];

    if body.is_empty() {
        return Err("DoH响应体为空".to_string());
    }

    let ips = parse_dns_response_wire(body)
        .map_err(|e| format!("DoH解析响应失败: {e}"))?;
    ips.into_iter().next()
        .ok_or_else(|| "DoH响应无有效A记录".to_string())
}

pub async fn resolve_host_smart(host: &str, timeout: Duration, bind_addr: Option<IpAddr>) -> Result<IpAddr, String> {
    if let Some(ip) = dns_cache_get(host) {
        return Ok(ip);
    }

    let doh_servers = get_best_doh_servers();
    let doh_timeout = std::cmp::min(timeout, Duration::from_secs(3));

    let mut set = tokio::task::JoinSet::new();

    for (server, ip_str) in &doh_servers {
        if let Ok(doh_ip) = ip_str.parse::<IpAddr>() {
            let s = server.clone();
            let h = host.to_string();
            let ba = bind_addr;
            set.spawn(async move {
                resolve_via_doh(&s, doh_ip, &h, ba, doh_timeout).await
            });
        }
    }

    let host_clone = host.to_string();
    let ba = bind_addr;
    set.spawn(async move {
        resolve_host_uncached_with_bind(&host_clone, timeout, ba).await
    });

    let mut first_error: Option<String> = None;
    let mut doh_failed = 0u32;
    let mut dns_failed = false;
    while let Some(res) = set.join_next().await {
        match res {
            Ok(Ok(ip)) => {
                set.abort_all();
                dns_cache_put(host, ip);
                return Ok(ip);
            }
            Ok(Err(e)) => {
                if e.contains("DoH") {
                    doh_failed += 1;
                } else {
                    dns_failed = true;
                }
                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
            Err(e) => {
                if first_error.is_none() {
                    first_error = Some(format!("任务失败: {e}"));
                }
            }
        }
    }

    let summary = if doh_failed > 0 && dns_failed {
        format!("DNS解析失败: DoH({}个失败)+传统DNS均不可用 - {}", doh_failed, first_error.unwrap_or_default())
    } else if doh_failed > 0 {
        format!("DNS解析失败: DoH({}个失败) - {}", doh_failed, first_error.unwrap_or_default())
    } else {
        format!("DNS解析失败: 传统DNS不可用 - {}", first_error.unwrap_or_default())
    };

    Err(summary)
}
