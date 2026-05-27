use serde::Serialize;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::TlsConnector;

lazy_static::lazy_static! {
    static ref TLS_CONNECTOR: TlsConnector = {
        let mut root_store = tokio_rustls::rustls::RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let provider = tokio_rustls::rustls::crypto::ring::default_provider();
        let mut config = tokio_rustls::rustls::ClientConfig::builder_with_provider(Arc::new(provider.clone()))
            .with_safe_default_protocol_versions()
            .unwrap_or_else(|e| {
                eprintln!("TLS protocol versions fallback: {}", e);
                // 降级到 TLS 1.2+1.3 默认版本
                tokio_rustls::rustls::ClientConfig::builder_with_provider(Arc::new(provider))
                    .with_protocol_versions(&[
                        &tokio_rustls::rustls::version::TLS13,
                        &tokio_rustls::rustls::version::TLS12,
                    ])
                    .expect("TLS 1.2/1.3 protocol versions must be supported")
            })
            .with_root_certificates(root_store)
            .with_no_client_auth();
        config.resumption = tokio_rustls::rustls::client::Resumption::default();
        TlsConnector::from(Arc::new(config))
    };

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

fn get_best_dns_servers() -> Vec<String> {
    let scored: Vec<_> = DNS_SERVER_SCORES.iter()
        .filter(|e| e.value().success && e.value().last_tested.elapsed().as_secs() < 600)
        .collect();

    if scored.is_empty() {
        return DNS_FALLBACK_SERVERS.iter().map(|s| s.to_string()).collect();
    }

    let mut servers: Vec<_> = scored.iter()
        .map(|e| (e.key().clone(), e.value().latency_ms))
        .collect();
    servers.sort_by_key(|(_, lat)| *lat);
    servers.into_iter().map(|(ip, _)| ip).collect()
}

fn get_best_doh_servers() -> Vec<(String, String)> {
    let scored: Vec<_> = DOH_SERVER_SCORES.iter()
        .filter(|e| e.value().success && e.value().last_tested.elapsed().as_secs() < 600)
        .collect();

    if scored.is_empty() {
        return DOH_FALLBACK_SERVERS.iter()
            .map(|(s, ip)| (s.to_string(), ip.to_string()))
            .collect();
    }

    let mut servers: Vec<_> = scored.iter()
        .map(|e| {
            let fallback_ip = DOH_FALLBACK_SERVERS.iter()
                .find(|(name, _)| *name == e.key())
                .map(|(_, ip)| ip.to_string())
                .unwrap_or_default();
            (e.key().clone(), e.value().latency_ms, fallback_ip)
        })
        .collect();
    servers.sort_by_key(|(_, lat, _)| *lat);
    servers.into_iter().map(|(name, _, ip)| (name, ip)).collect()
}

const DNS_CACHE_TTL_SECS: u64 = 60;
const DNS_CACHE_MAX_ENTRIES: usize = 64;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpTimingResult {
    pub url: String,
    pub success: bool,
    pub error: Option<String>,
    pub dns_ms: i64,
    pub tcp_ms: i64,
    pub tls_ms: i64,
    pub ttfb_ms: i64,
    pub content_ms: i64,
    pub total_ms: i64,
    pub tls_version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsQueryResult {
    pub server: String,
    pub domain: String,
    pub success: bool,
    pub error: Option<String>,
    pub udp_ms: i64,
    pub tcp_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DohTimingResult {
    pub server: String,
    pub success: bool,
    pub error: Option<String>,
    pub dns_ms: i64,
    pub tcp_ms: i64,
    pub tls_ms: i64,
    pub http_ms: i64,
    pub total_ms: i64,
    pub tls_version: String,
    pub port: u16,
}

fn ms_from(start: Instant) -> i64 {
    let us = start.elapsed().as_micros();
    ((us + 500) / 1000).max(1) as i64
}

fn dns_cache_get(host: &str) -> Option<IpAddr> {
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

fn dns_cache_put(host: &str, ip: IpAddr) {
    DNS_CACHE.insert(host.to_string(), (ip, Instant::now()));
    let now = Instant::now();
    DNS_CACHE.retain(|_, (_, ts)| now.duration_since(*ts).as_secs() < DNS_CACHE_TTL_SECS);
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
    DNS_CACHE.retain(|_, (_, ts)| now.duration_since(*ts).as_secs() < DNS_CACHE_TTL_SECS);
}

pub async fn measure_https_timing(
    host: &str,
    port: u16,
    bind_addr: Option<IpAddr>,
    timeout: Duration,
    skip_ttfb: bool,
    skip_content: bool,
) -> HttpTimingResult {
    let url = format!("https://{}:{}/", host, port);
    let mut result = HttpTimingResult {
        url: url.clone(),
        success: false,
        error: None,
        dns_ms: -1,
        tcp_ms: -1,
        tls_ms: -1,
        ttfb_ms: -1,
        content_ms: -1,
        total_ms: -1,
        tls_version: String::new(),
    };

    let overall_start = Instant::now();
    let dns_timeout = timeout;
    let tcp_timeout = Duration::from_secs(5);
    let tls_timeout = Duration::from_secs(5);
    let http_timeout = Duration::from_secs(5);

    // === Phase 1: DNS Resolution (with cache) ===
    let dns_start = Instant::now();
    let ip = if let Some(cached) = dns_cache_get(host) {
        result.dns_ms = ms_from(dns_start);
        cached
    } else {
        match resolve_host_smart(host, dns_timeout, bind_addr).await {
            Ok(ip) => {
                dns_cache_put(host, ip);
                result.dns_ms = ms_from(dns_start);
                ip
            }
            Err(e) => {
                result.error = Some(format!("DNS解析失败: {}", e));
                result.total_ms = ms_from(overall_start);
                return result;
            }
        }
    };

    // === Phase 2: TCP Connection ===
    let addr = std::net::SocketAddr::new(ip, port);
    let tcp_start = Instant::now();
    let tcp_stream = match bind_and_connect(addr, bind_addr, tcp_timeout).await {
        Ok(s) => s,
        Err(e) => {
            result.error = Some(format!("TCP连接失败: {}", e));
            result.total_ms = ms_from(overall_start);
            return result;
        }
    };
    result.tcp_ms = ms_from(tcp_start);

    // === Phase 3: TLS Handshake (session resumption supported by rustls default) ===
    let tls_start = Instant::now();
    let (mut tls_stream, negotiated_version) = match do_tls_handshake(host, tcp_stream, tls_timeout).await {
        Ok(r) => r,
        Err(e) => {
            result.error = Some(format!("TLS握手失败: {}", e));
            result.total_ms = ms_from(overall_start);
            return result;
        }
    };
    result.tls_ms = ms_from(tls_start);
    result.tls_version = negotiated_version;

    // === Phase 4: Send HTTP Request + TTFB + Content (optional) ===
    if !skip_ttfb && !skip_content {
        let request = format!(
            "GET / HTTP/1.1\r\nHost: {}\r\nUser-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64)\r\nAccept: */*\r\nConnection: close\r\n\r\n",
            host
        );

        let ttfb_start = Instant::now();
        if let Err(e) = tls_stream.write_all(request.as_bytes()).await {
            result.error = Some(format!("发送请求失败: {}", e));
            result.total_ms = ms_from(overall_start);
            return result;
        }

        let mut buf = vec![0u8; 8192];
        let mut total_read = 0usize;
        let mut first_byte_received = false;
        let mut content_start = Instant::now();

        loop {
            match tokio::time::timeout(http_timeout, tls_stream.read(&mut buf)).await {
                Ok(Ok(0)) => break,
                Ok(Ok(n)) => {
                    if !first_byte_received {
                        result.ttfb_ms = ms_from(ttfb_start);
                        content_start = Instant::now();
                        first_byte_received = true;
                    }
                    total_read += n;
                    if total_read > 64 * 1024 {
                        break;
                    }
                }
                Ok(Err(e)) => {
                    if !first_byte_received {
                        result.error = Some(format!("读取响应失败: {}", e));
                        result.total_ms = ms_from(overall_start);
                        return result;
                    }
                    break;
                }
                Err(_) => {
                    if !first_byte_received {
                        result.error = Some("TTFB超时".to_string());
                        result.total_ms = ms_from(overall_start);
                        return result;
                    }
                    break;
                }
            }
        }

        if first_byte_received {
            result.content_ms = ms_from(content_start);
        } else {
            result.ttfb_ms = ms_from(ttfb_start);
        }
    } else {
        result.ttfb_ms = -1;
        result.content_ms = -1;
    }

    result.total_ms = ms_from(overall_start);
    result.success = true;
    result
}

async fn resolve_host_uncached_with_bind(
    host: &str,
    timeout: Duration,
    // bind_addr 未传入 NameServerConfig，因为此函数解析的是公共域名，
    // 通常不受源接口影响；同文件 dns_lookup 函数已正确实现绑定逻辑
    _bind_addr: Option<IpAddr>,
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
                    bind_addr: None,
                });
            }
        }

        let mut opts = ResolverOpts::default();
        opts.try_tcp_on_error = true;
        opts.timeout = timeout;
        opts.attempts = 2;
        opts.num_concurrent_reqs = servers.len().min(3);

        let resolver = Resolver::new(config, opts)
            .map_err(|e| format!("创建解析器失败: {}", e))?;

        match resolver.lookup_ip(&host) {
            Ok(response) => {
                response.iter().next()
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
                    .map_err(|e| format!("创建系统解析器失败: {}", e))?;

                sys_resolver.lookup_ip(&host)
                    .map_err(|e| format!("{}", e))
                    .and_then(|response| {
                        response.iter().next()
                            .ok_or_else(|| "系统DNS无结果".to_string())
                    })
            }
        }
    }).await;

    match result {
        Ok(Ok(ip)) => Ok(ip),
        Ok(Err(e)) => Err(e),
        Err(e) => Err(format!("解析任务失败: {}", e)),
    }
}

async fn bind_and_connect(
    addr: std::net::SocketAddr,
    bind_addr: Option<IpAddr>,
    timeout: Duration,
) -> Result<TcpStream, String> {
    let stream = if let Some(bind) = bind_addr {
        let bind_addr = std::net::SocketAddr::new(bind, 0);
        let socket = match addr {
            std::net::SocketAddr::V4(_) => {
                tokio::net::TcpSocket::new_v4().map_err(|e| format!("{}", e))?
            }
            std::net::SocketAddr::V6(_) => {
                tokio::net::TcpSocket::new_v6().map_err(|e| format!("{}", e))?
            }
        };
        socket.bind(bind_addr).map_err(|e| format!("绑定失败: {}", e))?;
        tokio::time::timeout(timeout, socket.connect(addr))
            .await
            .map_err(|_| "TCP连接超时".to_string())?
            .map_err(|e| format!("{}", e))?
    } else {
        tokio::time::timeout(timeout, TcpStream::connect(addr))
            .await
            .map_err(|_| "TCP连接超时".to_string())?
            .map_err(|e| format!("{}", e))?
    };
    Ok(stream)
}

async fn do_tls_handshake(
    host: &str,
    tcp_stream: TcpStream,
    timeout: Duration,
) -> Result<(tokio_rustls::client::TlsStream<TcpStream>, String), String> {
    let server_name = ServerName::try_from(host.to_string())
        .map_err(|e| format!("无效主机名: {}", e))?;

    let tls_stream = tokio::time::timeout(timeout, TLS_CONNECTOR.connect(server_name, tcp_stream))
        .await
        .map_err(|_| "TLS握手超时".to_string())?
        .map_err(|e| format!("{}", e))?;

    let version_str = {
        let (_, connection) = tls_stream.get_ref();
        let negotiated = connection.protocol_version();
        match negotiated {
            Some(tokio_rustls::rustls::ProtocolVersion::TLSv1_3) => "TLS 1.3".to_string(),
            Some(tokio_rustls::rustls::ProtocolVersion::TLSv1_2) => "TLS 1.2".to_string(),
            _ => format!("{:?}", negotiated),
        }
    };

    Ok((tls_stream, version_str))
}

pub async fn measure_dns_query(
    server_ip: &str,
    domain: &str,
    bind_addr: Option<IpAddr>,
    timeout: Duration,
) -> DnsQueryResult {
    let query_domain = domain.to_string();

    let mut result = DnsQueryResult {
        server: server_ip.to_string(),
        domain: domain.to_string(),
        success: false,
        error: None,
        udp_ms: -1,
        tcp_ms: -1,
    };

    let (udp_result, tcp_result) = tokio::join!(
        dns_lookup(server_ip, &query_domain, bind_addr, timeout, hickory_resolver::config::Protocol::Udp),
        dns_lookup(server_ip, &query_domain, bind_addr, timeout, hickory_resolver::config::Protocol::Tcp)
    );
    let (_, udp_ms) = udp_result;
    let (_, tcp_ms) = tcp_result;

    match udp_result {
        (Ok(_), _) => { result.udp_ms = udp_ms; }
        (Err(e), _) => { if tcp_ms < 0 { result.error = Some(format!("UDP查询失败: {}", e)); } }
    }
    match tcp_result {
        (Ok(_), _) => { result.tcp_ms = tcp_ms; }
        (Err(e), _) => { if result.udp_ms < 0 { result.error = Some(format!("TCP查询失败: {}", e)); } }
    }

    result.success = result.udp_ms >= 0 || result.tcp_ms >= 0;
    result
}

async fn dns_lookup(
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
        Err(e) => return (Err(format!("{}", e)), -1),
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
        Err(e) => return (Err(format!("创建解析器失败: {}", e)), -1),
    };

    let domain = domain.to_string();
    match tokio::task::spawn_blocking(move || {
        resolver.lookup_ip(&domain)
            .map_err(|e| format!("{}", e))
    }).await {
        Ok(Ok(_)) => (Ok(()), ms_from(start)),
        Ok(Err(e)) => (Err(e), ms_from(start)),
        Err(e) => (Err(format!("任务执行失败: {}", e)), ms_from(start)),
    }
}

pub async fn measure_doh_timing(
    doh_server: &str,
    doh_ip: &str,
    query_domain: &str,
    bind_addr: Option<IpAddr>,
    timeout: Duration,
    skip_http: bool,
) -> DohTimingResult {
    let query_domain = query_domain.to_string();

    let mut result = DohTimingResult {
        server: doh_server.to_string(),
        success: false,
        error: None,
        dns_ms: -1,
        tcp_ms: -1,
        tls_ms: -1,
        http_ms: -1,
        total_ms: -1,
        tls_version: String::new(),
        port: 443,
    };

    let overall_start = Instant::now();
    let connect_timeout = Duration::from_secs(5);
    let http_timeout = Duration::from_secs(5);

    let dns_start = Instant::now();
    let (ip, used_dns_resolve) = if !doh_ip.is_empty() {
        match doh_ip.parse() {
            Ok(ip) => (ip, false),
            Err(_) => {
                result.error = Some("DoH服务器IP无效".to_string());
                return result;
            }
        }
    } else {
        match resolve_host_uncached_with_bind(doh_server, timeout, bind_addr).await {
            Ok(ip) => (ip, true),
            Err(e) => {
                result.error = Some(format!("DoH域名解析失败: {}", e));
                return result;
            }
        }
    };
    result.dns_ms = ms_from(dns_start);
    let dns_ms = result.dns_ms;

    let https_result = do_doh_https(doh_server, ip, &query_domain, bind_addr, connect_timeout, http_timeout, skip_http).await;
    if https_result.success {
        let mut r = https_result;
        r.dns_ms = dns_ms;
        r.total_ms = ms_from(overall_start);
        return r;
    }

    result.tcp_ms = https_result.tcp_ms;
    result.tls_ms = https_result.tls_ms;
    result.http_ms = https_result.http_ms;
    result.tls_version = https_result.tls_version;

    if !used_dns_resolve {
        let fallback_dns_start = Instant::now();
        match resolve_host_uncached_with_bind(doh_server, timeout, bind_addr).await {
            Ok(dns_ip) => {
                let fallback_dns_ms = ms_from(fallback_dns_start);
                let fallback_result = do_doh_https(doh_server, dns_ip, &query_domain, bind_addr, connect_timeout, http_timeout, skip_http).await;
                if fallback_result.success {
                    let mut r = fallback_result;
                    r.dns_ms = fallback_dns_ms;
                    r.total_ms = ms_from(overall_start);
                    return r;
                }
            }
            Err(_) => {}
        }
    }

    result.error = Some("DoH请求失败(443不可达)".to_string());
    let completed: i64 = [result.dns_ms, result.tcp_ms, result.tls_ms, result.http_ms]
        .iter().filter(|&&x| x > 0).sum();
    result.total_ms = if completed > 0 { completed } else { -1 };
    result
}

async fn do_doh_https(
    doh_server: &str,
    doh_ip: IpAddr,
    query_domain: &str,
    bind_addr: Option<IpAddr>,
    connect_timeout: Duration,
    http_timeout: Duration,
    skip_http: bool,
) -> DohTimingResult {
    let overall_start = Instant::now();
    let mut result = DohTimingResult {
        server: doh_server.to_string(),
        success: false,
        error: None,
        dns_ms: 0,
        tcp_ms: -1,
        tls_ms: -1,
        http_ms: -1,
        total_ms: -1,
        tls_version: String::new(),
        port: 443,
    };

    let addr = std::net::SocketAddr::new(doh_ip, 443);
    let tcp_start = Instant::now();
    let tcp_stream = match bind_and_connect(addr, bind_addr, connect_timeout).await {
        Ok(s) => { result.tcp_ms = ms_from(tcp_start); s }
        Err(e) => {
            result.error = Some(format!("TCP连接失败: {}", e));
            let completed: i64 = [result.tcp_ms, result.tls_ms, result.http_ms].iter().filter(|&&x| x > 0).sum();
            result.total_ms = if completed > 0 { completed } else { -1 };
            return result;
        }
    };

    let tls_start = Instant::now();
    let mut tls_stream = match do_tls_handshake(doh_server, tcp_stream, connect_timeout).await {
        Ok((stream, tls_ver)) => {
            result.tls_ms = ms_from(tls_start);
            result.tls_version = tls_ver;
            stream
        }
        Err(e) => {
            result.error = Some(format!("TLS握手失败: {}", e));
            let completed: i64 = [result.tcp_ms, result.tls_ms, result.http_ms].iter().filter(|&&x| x > 0).sum();
            result.total_ms = if completed > 0 { completed } else { -1 };
            return result;
        }
    };

    if skip_http {
        result.http_ms = -1;
        result.total_ms = ms_from(overall_start);
        result.success = true;
        return result;
    }

    let query_wire = build_dns_query_wire(query_domain, 1);
    let dns_param = base64url_encode_no_pad(&query_wire);
    let path = format!("/dns-query?dns={}", dns_param);
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nAccept: application/dns-message\r\nUser-Agent: Mozilla/5.0\r\nConnection: close\r\n\r\n",
        path, doh_server
    );

    let http_start = Instant::now();
    if let Err(e) = tls_stream.write_all(request.as_bytes()).await {
        result.error = Some(format!("发送请求失败: {}", e));
        let completed: i64 = [result.tcp_ms, result.tls_ms, result.http_ms].iter().filter(|&&x| x > 0).sum();
        result.total_ms = if completed > 0 { completed } else { -1 };
        return result;
    }

    let mut buf = vec![0u8; 4096];
    let mut total_read = 0usize;
    let mut first_read = true;
    loop {
        match tokio::time::timeout(http_timeout, tls_stream.read(&mut buf)).await {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => {
                if first_read {
                    result.http_ms = ms_from(http_start);
                    first_read = false;
                }
                total_read += n;
                if total_read > 64 * 1024 {
                    break;
                }
            }
            Ok(Err(e)) => {
                if first_read {
                    result.error = Some(format!("读取响应失败: {}", e));
                }
                break;
            }
            Err(_) => {
                if first_read {
                    result.error = Some("DoH响应超时".to_string());
                    result.http_ms = ms_from(http_start);
                }
                break;
            }
        }
    }
    if !first_read {
        result.total_ms = ms_from(overall_start);
        result.success = true;
    } else if result.error.is_none() {
        result.error = Some("DoH响应为空".to_string());
    }
    if !result.success {
        let completed: i64 = [result.tcp_ms, result.tls_ms, result.http_ms].iter().filter(|&&x| x > 0).sum();
        result.total_ms = if completed > 0 { completed } else { -1 };
    }
    result
}

fn build_dns_query_wire(domain: &str, qtype: u16) -> Vec<u8> {
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

fn base64url_encode_no_pad(data: &[u8]) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    base64::Engine::encode(&URL_SAFE_NO_PAD, data)
}

fn parse_dns_response_wire(data: &[u8]) -> Result<Vec<IpAddr>, String> {
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

fn skip_dns_name(data: &[u8], mut pos: usize) -> Result<usize, String> {
    let mut jumped = false;
    let mut after_jump_pos: usize = 0;
    let mut seen = std::collections::HashSet::new();
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

async fn resolve_via_doh(
    doh_server: &str,
    doh_ip: IpAddr,
    domain: &str,
    bind_addr: Option<IpAddr>,
    timeout: Duration,
) -> Result<IpAddr, String> {
    let addr = std::net::SocketAddr::new(doh_ip, 443);
    let tcp_stream = bind_and_connect(addr, bind_addr, timeout).await
        .map_err(|e| format!("DoH TCP连接失败: {}", e))?;

    let (mut tls_stream, _) = do_tls_handshake(doh_server, tcp_stream, timeout).await
        .map_err(|e| format!("DoH TLS握手失败: {}", e))?;

    let query_wire = build_dns_query_wire(domain, 1);
    let dns_param = base64url_encode_no_pad(&query_wire);
    let path = format!("/dns-query?dns={}", dns_param);
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nAccept: application/dns-message\r\nUser-Agent: Mozilla/5.0\r\nConnection: close\r\n\r\n",
        path, doh_server
    );

    tls_stream.write_all(request.as_bytes()).await
        .map_err(|e| format!("DoH发送请求失败: {}", e))?;

    let mut response = Vec::new();
    let mut buf = vec![0u8; 4096];
    loop {
        match tokio::time::timeout(timeout, tls_stream.read(&mut buf)).await {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => {
                response.extend_from_slice(&buf[..n]);
                if response.len() > 64 * 1024 { break; }
            }
            Ok(Err(e)) => {
                if response.is_empty() {
                    return Err(format!("DoH读取响应失败: {}", e));
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
    let body = &response[header_end + 4..];

    if body.is_empty() {
        return Err("DoH响应体为空".to_string());
    }

    let ips = parse_dns_response_wire(body)?;
    ips.into_iter().next()
        .ok_or_else(|| "DoH响应无有效A记录".to_string())
}

pub async fn resolve_host_smart(host: &str, timeout: Duration, bind_addr: Option<IpAddr>) -> Result<IpAddr, String> {
    if let Some(ip) = dns_cache_get(host) {
        return Ok(ip);
    }

    let doh_servers = get_best_doh_servers();
    let doh_timeout = Duration::from_secs(3);

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
    while let Some(res) = set.join_next().await {
        match res {
            Ok(Ok(ip)) => {
                set.abort_all();
                dns_cache_put(host, ip);
                return Ok(ip);
            }
            Ok(Err(e)) => {
                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
            Err(e) => {
                if first_error.is_none() {
                    first_error = Some(format!("任务失败: {}", e));
                }
            }
        }
    }

    Err(first_error.unwrap_or_else(|| "DNS解析失败: 所有方式均不可用".to_string()))
}
