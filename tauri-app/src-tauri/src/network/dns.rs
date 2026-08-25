use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

lazy_static::lazy_static! {
    static ref DNS_CACHE: dashmap::DashMap<String, (IpAddr, Instant)> = dashmap::DashMap::new();

    static ref DNS_SERVER_SCORES: dashmap::DashMap<String, ServerScore> = dashmap::DashMap::new();
    static ref DOH_SERVER_SCORES: dashmap::DashMap<String, ServerScore> = dashmap::DashMap::new();

    /// BP-5: 缓存系统 ResolverConfig，避免 fallback 路径每次重复构造。
    /// 历史缺陷：此处曾用 ResolverConfig::default()（硬编码 Google 8.8.8.8/8.8.4.4），
    /// 在仅 Portal 可达的校园网/捕获门户场景必然失败，导致"系统 DNS fallback"形同虚设。
    /// 现改用真实 OS 配置（system_conf::read_system_conf），失败时回退 default。
    static ref SYSTEM_RESOLVER_CONFIG: hickory_resolver::config::ResolverConfig = {
        hickory_resolver::system_conf::read_system_conf()
            .map(|(config, _opts)| config)
            .unwrap_or_else(|_| hickory_resolver::config::ResolverConfig::default())
    };
}

const DNS_FALLBACK_SERVERS: &[&str] = &["223.5.5.5", "1.12.12.12", "114.114.114.114"];
const DOH_FALLBACK_SERVERS: &[(&str, &str)] = &[
    ("dns.alidns.com", "223.5.5.5"),
    ("doh.pub", "1.12.12.12"),
];

// ===== Resolver 复用池（BE-D-01）=====
// hickory 0.24 的同步 Resolver 内部自带一个 current-thread Tokio Runtime，
// 每次解析都新建会重复创建 Runtime + AsyncResolver（连接池、后台任务），
// 是传统 DNS 解析的主要固定开销。按 (bind_addr, server 集, 超时) 维度缓存复用。
//
// 为避免共享单个 Resolver 时其内部 Mutex<Runtime> 把并发解析串行化
// （quality Phase1 的 SystemDns 会并发解析多个域名，若串行会把离线超时
// 逐域名累加），每个 key 维护一个小池，池满后轮转取用：
// 既保证并发并行度，又避免重复建 Runtime。
const RESOLVER_POOL_SIZE: usize = 4;
const RESOLVER_CACHE_MAX_KEYS: usize = 16;

struct ResolverPoolEntry {
    resolvers: Vec<Arc<hickory_resolver::Resolver>>,
    next: usize,
}

lazy_static::lazy_static! {
    static ref RESOLVER_CACHE: parking_lot::Mutex<HashMap<String, ResolverPoolEntry>> =
        parking_lot::Mutex::new(HashMap::new());
}

fn resolver_cache_key(bind_addr: Option<IpAddr>, servers: &[String], timeout: Duration) -> String {
    format!("{:?}|{}|{}", bind_addr, servers.join(","), timeout.as_millis())
}

/// 获取或创建 Resolver：池未满时按需扩容（首次调用建 1 个，最多 POOL_SIZE 个），
/// 池满后轮转复用；key 数量极少（几个 bind_addr × 几组 server），超限整体重建即可。
fn resolver_get_or_create(
    key: &str,
    config: hickory_resolver::config::ResolverConfig,
    opts: hickory_resolver::config::ResolverOpts,
) -> Result<Arc<hickory_resolver::Resolver>, String> {
    let mut cache = RESOLVER_CACHE.lock();
    if cache.len() >= RESOLVER_CACHE_MAX_KEYS && !cache.contains_key(key) {
        cache.clear();
    }
    let entry = cache.entry(key.to_string()).or_insert_with(|| ResolverPoolEntry {
        resolvers: Vec::with_capacity(RESOLVER_POOL_SIZE),
        next: 0,
    });
    if entry.resolvers.len() < RESOLVER_POOL_SIZE {
        let resolver = Arc::new(
            hickory_resolver::Resolver::new(config, opts)
                .map_err(|e| format!("创建解析器失败: {e}"))?,
        );
        entry.resolvers.push(resolver.clone());
        Ok(resolver)
    } else {
        let idx = entry.next;
        entry.next = (entry.next + 1) % entry.resolvers.len();
        Ok(entry.resolvers[idx].clone())
    }
}

#[derive(Clone)]
struct ServerScore {
    latency_ms: i64,
    success: bool,
    last_tested: Instant,
}

pub fn update_dns_server_latency(ip: &str, latency_ms: i64, success: bool) {
    DNS_SERVER_SCORES.insert(ip.to_string(), ServerScore {
        latency_ms,
        success,
        last_tested: Instant::now(),
    });
}

pub fn update_doh_server_latency(server: &str, latency_ms: i64, success: bool) {
    DOH_SERVER_SCORES.insert(server.to_string(), ServerScore {
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

/// 缓存 key：host + bind_addr。
/// bind_addr 参与 key 使不同出口接口的解析结果互不污染
/// （split-horizon/地理 DNS 下同一域名经不同接口可解析到不同 IP）。
fn dns_cache_key(host: &str, bind_addr: Option<IpAddr>) -> String {
    match bind_addr {
        Some(ip) => format!("{host}@{ip}"),
        None => host.to_string(),
    }
}

pub(crate) fn dns_cache_get(host: &str, bind_addr: Option<IpAddr>) -> Option<IpAddr> {
    let key = dns_cache_key(host, bind_addr);
    DNS_CACHE.get(&key).and_then(|entry| {
        if entry.value().1.elapsed().as_secs() < DNS_CACHE_TTL_SECS {
            Some(entry.value().0)
        } else {
            None
        }
    }).or_else(|| {
        DNS_CACHE.remove_if(&key, |_, (_, ts)| ts.elapsed().as_secs() >= DNS_CACHE_TTL_SECS);
        None
    })
}

pub(crate) fn dns_cache_put(host: &str, bind_addr: Option<IpAddr>, ip: IpAddr) {
    let now = Instant::now();
    let key = dns_cache_key(host, bind_addr);
    DNS_CACHE.insert(key, (ip, now));
    // BE-D-03: 清理合并到容量超限时一次性执行——先剔除过期项（合并原每次
    // put 全表 retain 的职责），仍超限时一次收集排序取最旧 N 条删除，
    // 替代原"循环 min_by_key 逐条删 + 每次 put 全表 retain"。
    if DNS_CACHE.len() > DNS_CACHE_MAX_ENTRIES {
        let deadline = now.checked_sub(Duration::from_secs(DNS_CACHE_TTL_SECS)).unwrap_or(now);
        DNS_CACHE.retain(|_, (_, ts)| *ts > deadline);
    }
    if DNS_CACHE.len() > DNS_CACHE_MAX_ENTRIES {
        let mut entries: Vec<(String, Instant)> = DNS_CACHE.iter()
            .map(|e| (e.key().clone(), e.value().1))
            .collect();
        entries.sort_by_key(|(_, ts)| *ts);
        let mut remove_count = DNS_CACHE.len().saturating_sub(DNS_CACHE_MAX_ENTRIES);
        for (key, _) in entries {
            if remove_count == 0 {
                break;
            }
            DNS_CACHE.remove(&key);
            remove_count -= 1;
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

        // BE-D-01: 从复用池取 Resolver，避免每次解析新建内部 Tokio Runtime
        let key = resolver_cache_key(bind_addr, &servers, timeout);
        let resolver = resolver_get_or_create(&key, config, opts)?;

        match resolver.lookup_ip(&host) {
            Ok(response) => {
                response.iter()
                    .find(|ip| ip.is_ipv4())
                    .or_else(|| response.iter().next())
                    .ok_or_else(|| "无DNS结果".to_string())
            }
            Err(_) => {
                // BP-5: 使用缓存的系统 config，避免每次 fallback 都重新构造
                let mut sys_config = SYSTEM_RESOLVER_CONFIG.clone();
                // 保留 bind_addr：双适配器场景下系统 fallback 也必须从指定接口出站，
                // 否则 egress 走错接口导致解析失败（历史缺陷：fallback 丢弃 bind_addr）
                if let Some(bind_ip) = bind_addr {
                    let bind_sock = std::net::SocketAddr::new(bind_ip, 0);
                    let mut updated = hickory_resolver::config::ResolverConfig::new();
                    for ns in sys_config.name_servers() {
                        let mut ns = ns.clone();
                        ns.bind_addr = Some(bind_sock);
                        updated.add_name_server(ns);
                    }
                    sys_config = updated;
                }
                let mut sys_opts = ResolverOpts::default();
                sys_opts.try_tcp_on_error = true;
                sys_opts.timeout = timeout;
                sys_opts.attempts = 2;
                sys_opts.num_concurrent_reqs = 2;

                // 系统 fallback 也走复用池（key 带 sys 前缀，config 由 SYSTEM_RESOLVER_CONFIG 固定）
                let sys_key = format!("sys|{:?}|{}", bind_addr, timeout.as_millis());
                let sys_resolver = resolver_get_or_create(&sys_key, sys_config, sys_opts)?;

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

    // BE-D-01: 单服务器解析器也走复用池（key 含 server/protocol/bind/timeout）
    let key = format!("single|{}|{:?}|{:?}|{}", server_ip, protocol, bind_addr, timeout.as_millis());
    let resolver = match resolver_get_or_create(&key, resolver_config, opts) {
        Ok(r) => r,
        Err(e) => return (Err(e), -1),
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
    // 校验响应头：QR 位必须为 1（响应）、RCODE 必须为 0（无错误）。
    // 历史缺陷：仅解析 ANCOUNT 与 A 记录，RCODE!=0 的错误响应（如 NXDOMAIN）
    // 或非响应报文会被当作"无答案"吞掉，无法区分 DNS 劫持与解析失败。
    let flags = u16::from_be_bytes([data[2], data[3]]);
    let is_response = (flags >> 15) & 0x1 == 1;
    let rcode = flags & 0x000F;
    if !is_response {
        return Err("DNS响应格式无效: QR位不是响应".to_string());
    }
    if rcode != 0 {
        return Err(format!("DNS响应错误码: RCODE={rcode}"));
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
    if let Some(ip) = dns_cache_get(host, bind_addr) {
        return Ok(ip);
    }

    let doh_servers = get_best_doh_servers();
    let doh_timeout = std::cmp::min(timeout, Duration::from_secs(3));

    let mut set = tokio::task::JoinSet::new();

    // BE-D-02: 竞速降级——只并发"历史最快 1 个 DoH + 传统 DNS"两路。
    // 原实现对全部 DoH（默认 2 个）都发起 TLS 握手，每域名 3 路 TLS 且连接一次即弃；
    // 改为仅 1 路 DoH（get_best_doh_servers 已按延迟升序，first 即最快），
    // 每域名 TLS 握手从 2 路降到 1 路。DoH 响应的 QR/RCODE 劫持校验逻辑不变。
    if let Some((server, ip_str)) = doh_servers.first() {
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
                dns_cache_put(host, bind_addr, ip);
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

#[cfg(test)]
mod tests {
    use super::*;

    // ===== build_dns_query_wire =====

    #[test]
    fn build_dns_query_wire_basic_structure() {
        let query = build_dns_query_wire("example.com", 1);
        // Header: 12 bytes + labels + root + qtype(2) + class(2)
        // \x07example(8) + \x03com(4) + \x00(1) + qtype(2) + class(2) = 17
        assert_eq!(query.len(), 12 + 17);
    }

    #[test]
    fn build_dns_query_wire_header_flags() {
        let query = build_dns_query_wire("example.com", 1);
        // flags = 0x0100 (standard query, recursion desired)
        assert_eq!(&query[2..4], &0x0100u16.to_be_bytes());
        // qdcount = 1
        assert_eq!(&query[4..6], &1u16.to_be_bytes());
        // ancount = 0
        assert_eq!(&query[6..8], &0u16.to_be_bytes());
        // nscount = 0
        assert_eq!(&query[8..10], &0u16.to_be_bytes());
        // arcount = 0
        assert_eq!(&query[10..12], &0u16.to_be_bytes());
    }

    #[test]
    fn build_dns_query_wire_domain_encoding() {
        let query = build_dns_query_wire("example.com", 1);
        // First label: length 7 + "example"
        assert_eq!(query[12], 7);
        assert_eq!(&query[13..20], b"example");
        // Second label: length 3 + "com"
        assert_eq!(query[20], 3);
        assert_eq!(&query[21..24], b"com");
        // Root label
        assert_eq!(query[24], 0);
    }

    #[test]
    fn build_dns_query_wire_qtype_and_class() {
        let query = build_dns_query_wire("example.com", 1);
        // qtype at end-4..end-2
        let qtype_pos = query.len() - 4;
        assert_eq!(&query[qtype_pos..qtype_pos + 2], &1u16.to_be_bytes());
        // class = IN (1) at end-2..end
        let class_pos = query.len() - 2;
        assert_eq!(&query[class_pos..class_pos + 2], &1u16.to_be_bytes());
    }

    #[test]
    fn build_dns_query_wire_single_label() {
        let query = build_dns_query_wire("localhost", 1);
        // \x09localhost(10) + \x00(1) + qtype(2) + class(2) = 15
        assert_eq!(query.len(), 12 + 15);
        assert_eq!(query[12], 9);
        assert_eq!(&query[13..22], b"localhost");
    }

    #[test]
    fn build_dns_query_wire_aaaa_type() {
        let query = build_dns_query_wire("example.com", 28); // AAAA
        let qtype_pos = query.len() - 4;
        assert_eq!(&query[qtype_pos..qtype_pos + 2], &28u16.to_be_bytes());
    }

    // ===== base64url_encode_no_pad =====

    #[test]
    fn base64url_encode_empty() {
        assert_eq!(base64url_encode_no_pad(&[]), "");
    }

    #[test]
    fn base64url_encode_known_values() {
        assert_eq!(base64url_encode_no_pad(&[0x01, 0x02, 0x03]), "AQID");
    }

    #[test]
    fn base64url_encode_high_bytes() {
        // 0xFF 0xFF 0xFF → "____" in URL-safe base64 (no pad)
        assert_eq!(base64url_encode_no_pad(&[0xff, 0xff, 0xff]), "____");
    }

    #[test]
    fn base64url_encode_single_byte() {
        // 0x01 → 000000 010000 → "AQ" (no pad)
        assert_eq!(base64url_encode_no_pad(&[0x01]), "AQ");
    }

    #[test]
    fn base64url_encode_two_bytes() {
        // 0x01 0x02 → 000000 010000 001000 → "AQI" (no pad)
        assert_eq!(base64url_encode_no_pad(&[0x01, 0x02]), "AQI");
    }

    // ===== skip_dns_name =====

    #[test]
    fn skip_dns_name_simple() {
        // \x07example\x03com\x00
        let data: Vec<u8> = vec![
            0x07, b'e', b'x', b'a', b'm', b'p', b'l', b'e',
            0x03, b'c', b'o', b'm',
            0x00,
        ];
        let result = skip_dns_name(&data, 0);
        assert_eq!(result, Ok(13));
    }

    #[test]
    fn skip_dns_name_root_only() {
        let data: Vec<u8> = vec![0x00];
        let result = skip_dns_name(&data, 0);
        assert_eq!(result, Ok(1));
    }

    #[test]
    fn skip_dns_name_compression_pointer() {
        // Pointer at pos 0 → offset 2, where name "foo" is
        let data: Vec<u8> = vec![
            0xC0, 0x02,           // pointer to offset 2
            0x03, b'f', b'o', b'o', // label "foo" at offset 2
            0x00,                  // root at offset 6
        ];
        let result = skip_dns_name(&data, 0);
        // Should return position after the pointer (pos + 2 = 2)
        assert_eq!(result, Ok(2));
    }

    #[test]
    fn skip_dns_name_out_of_bounds() {
        // Label says 7 bytes but data is too short
        let data: Vec<u8> = vec![0x07, b'e'];
        let result = skip_dns_name(&data, 0);
        assert!(result.is_err());
    }

    #[test]
    fn skip_dns_name_empty_data() {
        let data: Vec<u8> = vec![];
        let result = skip_dns_name(&data, 0);
        assert!(result.is_err());
    }

    #[test]
    fn skip_dns_name_circular_pointer() {
        // Self-referencing pointer: offset 0 points to offset 0
        let data: Vec<u8> = vec![0xC0, 0x00];
        let result = skip_dns_name(&data, 0);
        assert!(result.is_err());
    }

    #[test]
    fn skip_dns_name_pointer_out_of_bounds() {
        // Pointer at pos 0, but pos+1 is out of bounds
        let data: Vec<u8> = vec![0xC0];
        let result = skip_dns_name(&data, 0);
        assert!(result.is_err());
    }

    // ===== parse_dns_response_wire =====

    fn build_test_dns_response() -> Vec<u8> {
        let mut data = Vec::new();
        // Header
        data.extend_from_slice(&[0x12, 0x34]); // txid
        data.extend_from_slice(&[0x81, 0x80]); // flags
        data.extend_from_slice(&[0x00, 0x01]); // qdcount: 1
        data.extend_from_slice(&[0x00, 0x01]); // ancount: 1
        data.extend_from_slice(&[0x00, 0x00]); // nscount: 0
        data.extend_from_slice(&[0x00, 0x00]); // arcount: 0
        // Question: example.com A IN
        data.push(0x07);
        data.extend_from_slice(b"example");
        data.push(0x03);
        data.extend_from_slice(b"com");
        data.push(0x00);
        data.extend_from_slice(&[0x00, 0x01]); // type A
        data.extend_from_slice(&[0x00, 0x01]); // class IN
        // Answer: compression pointer to offset 12 + A record 1.2.3.4
        data.extend_from_slice(&[0xC0, 0x0C]); // pointer to offset 12
        data.extend_from_slice(&[0x00, 0x01]); // type A
        data.extend_from_slice(&[0x00, 0x01]); // class IN
        data.extend_from_slice(&[0x00, 0x00, 0x0E, 0x10]); // TTL 3600
        data.extend_from_slice(&[0x00, 0x04]); // rdlength 4
        data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]); // 1.2.3.4
        data
    }

    #[test]
    fn parse_dns_response_wire_valid_a_record() {
        let data = build_test_dns_response();
        let result = parse_dns_response_wire(&data);
        assert!(result.is_ok());
        let ips = result.unwrap();
        assert_eq!(ips.len(), 1);
        assert_eq!(ips[0], IpAddr::V4(std::net::Ipv4Addr::new(1, 2, 3, 4)));
    }

    #[test]
    fn parse_dns_response_wire_too_short() {
        let data: Vec<u8> = vec![0x00; 11]; // < 12 bytes
        let result = parse_dns_response_wire(&data);
        assert!(result.is_err());
    }

    #[test]
    fn parse_dns_response_wire_no_answers() {
        let mut data = Vec::new();
        // Header with ancount=0
        data.extend_from_slice(&[0x12, 0x34]); // txid
        data.extend_from_slice(&[0x81, 0x80]); // flags
        data.extend_from_slice(&[0x00, 0x01]); // qdcount: 1
        data.extend_from_slice(&[0x00, 0x00]); // ancount: 0
        data.extend_from_slice(&[0x00, 0x00]); // nscount: 0
        data.extend_from_slice(&[0x00, 0x00]); // arcount: 0
        // Question
        data.push(0x07);
        data.extend_from_slice(b"example");
        data.push(0x03);
        data.extend_from_slice(b"com");
        data.push(0x00);
        data.extend_from_slice(&[0x00, 0x01]); // type A
        data.extend_from_slice(&[0x00, 0x01]); // class IN
        let result = parse_dns_response_wire(&data);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn parse_dns_response_wire_multiple_a_records() {
        let mut data = Vec::new();
        // Header with ancount=2
        data.extend_from_slice(&[0x12, 0x34, 0x81, 0x80]);
        data.extend_from_slice(&[0x00, 0x01]); // qdcount: 1
        data.extend_from_slice(&[0x00, 0x02]); // ancount: 2
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        // Question
        data.push(0x03);
        data.extend_from_slice(b"foo");
        data.push(0x00);
        data.extend_from_slice(&[0x00, 0x01, 0x00, 0x01]);
        // Answer 1: 10.0.0.1
        data.extend_from_slice(&[0xC0, 0x0C]);
        data.extend_from_slice(&[0x00, 0x01, 0x00, 0x01]);
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x3C]); // TTL 60
        data.extend_from_slice(&[0x00, 0x04, 0x0A, 0x00, 0x00, 0x01]);
        // Answer 2: 10.0.0.2
        data.extend_from_slice(&[0xC0, 0x0C]);
        data.extend_from_slice(&[0x00, 0x01, 0x00, 0x01]);
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x3C]);
        data.extend_from_slice(&[0x00, 0x04, 0x0A, 0x00, 0x00, 0x02]);
        let result = parse_dns_response_wire(&data);
        assert!(result.is_ok());
        let ips = result.unwrap();
        assert_eq!(ips.len(), 2);
        assert_eq!(ips[0], IpAddr::V4(std::net::Ipv4Addr::new(10, 0, 0, 1)));
        assert_eq!(ips[1], IpAddr::V4(std::net::Ipv4Addr::new(10, 0, 0, 2)));
    }

    // ===== dns_cache_get / dns_cache_put =====

    #[test]
    fn dns_cache_put_then_get() {
        let host = "test_put_get_unique_v1.example.com";
        let ip: IpAddr = "203.0.113.42".parse().unwrap();
        dns_cache_put(host, None, ip);
        let result = dns_cache_get(host, None);
        assert_eq!(result, Some(ip));
    }

    #[test]
    fn dns_cache_get_miss() {
        let host = "test_cache_miss_unique_v1.example.com";
        let result = dns_cache_get(host, None);
        assert_eq!(result, None);
    }

    #[test]
    fn dns_cache_put_overwrite() {
        let host = "test_overwrite_unique_v1.example.com";
        let ip1: IpAddr = "203.0.113.1".parse().unwrap();
        let ip2: IpAddr = "203.0.113.2".parse().unwrap();
        dns_cache_put(host, None, ip1);
        dns_cache_put(host, None, ip2);
        let result = dns_cache_get(host, None);
        assert_eq!(result, Some(ip2));
    }

    #[test]
    fn dns_cache_keyed_by_bind_addr() {
        // 不同 bind_addr 的解析结果必须互相隔离（split-horizon/地理 DNS）
        let host = "test_bind_isolated_unique_v1.example.com";
        let ip_a: IpAddr = "203.0.113.10".parse().unwrap();
        let ip_b: IpAddr = "203.0.113.20".parse().unwrap();
        let bind_a: IpAddr = "192.168.1.10".parse().unwrap();
        let bind_b: IpAddr = "192.168.2.10".parse().unwrap();
        dns_cache_put(host, Some(bind_a), ip_a);
        dns_cache_put(host, Some(bind_b), ip_b);
        assert_eq!(dns_cache_get(host, Some(bind_a)), Some(ip_a));
        assert_eq!(dns_cache_get(host, Some(bind_b)), Some(ip_b));
    }

    // ===== SYSTEM_RESOLVER_CONFIG (BP-5) =====

    #[test]
    fn system_resolver_config_initializes_without_panic() {
        // BP-5: 验证缓存的 SYSTEM_RESOLVER_CONFIG 能成功初始化
        let config = &*SYSTEM_RESOLVER_CONFIG;
        assert!(!config.name_servers().is_empty());
    }

    #[test]
    fn system_resolver_config_clone_is_independent() {
        // BP-5: 验证 clone 产生独立副本，修改不影响原缓存
        let clone1 = SYSTEM_RESOLVER_CONFIG.clone();
        let clone2 = SYSTEM_RESOLVER_CONFIG.clone();
        // 两个 clone 应该有相同的 name servers 数量
        assert_eq!(clone1.name_servers().len(), clone2.name_servers().len());
    }

    // ===== DoH 响应头校验（RCODE / QR）=====

    #[test]
    fn parse_dns_response_wire_rejects_nxdomain_rcode() {
        // RCODE=3 (NXDOMAIN) 的错误响应不得被当作"无答案"吞掉
        let mut data = build_test_dns_response();
        data[3] = 0x83; // flags: 0x81 0x83 → RCODE=3
        let result = parse_dns_response_wire(&data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("RCODE"));
    }

    #[test]
    fn parse_dns_response_wire_rejects_query_packet() {
        // QR=0（查询报文）不是响应，必须拒绝
        let mut data = build_test_dns_response();
        data[2] = 0x01; // flags: 0x01 0x80 → QR=0
        let result = parse_dns_response_wire(&data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("QR"));
    }
}
