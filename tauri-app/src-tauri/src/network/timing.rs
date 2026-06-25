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
                eprintln!("TLS protocol versions fallback: {e}");
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
}

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

pub async fn measure_https_timing(
    host: &str,
    port: u16,
    bind_addr: Option<IpAddr>,
    timeout: Duration,
    skip_ttfb: bool,
    skip_content: bool,
) -> HttpTimingResult {
    let url = format!("https://{host}:{port}/");
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
    // 使用单一整体 deadline，各阶段从剩余时间分配预算，避免独立累加导致总耗时远超入参 timeout
    let deadline = overall_start + timeout;
    let dns_timeout = timeout / 3;

    let dns_start = Instant::now();
    let ip = if let Some(cached) = super::dns::dns_cache_get(host) {
        result.dns_ms = ms_from(dns_start);
        cached
    } else {
        match super::dns::resolve_host_smart(host, dns_timeout, bind_addr).await {
            Ok(ip) => {
                super::dns::dns_cache_put(host, ip);
                result.dns_ms = ms_from(dns_start);
                ip
            }
            Err(e) => {
                let detail = if e.contains("DoH") {
                    format!("DNS解析失败(DoH+传统DNS均不可用): {e}")
                } else if e.contains("超时") || e.contains("timeout") {
                    format!("DNS解析超时: {e}")
                } else if e.contains("劫持") || e.contains("hijack") {
                    format!("DNS可能被劫持: {e}")
                } else {
                    format!("DNS解析失败: {e}")
                };
                result.error = Some(detail);
                result.total_ms = ms_from(overall_start);
                return result;
            }
        }
    };

    let addr = std::net::SocketAddr::new(ip, port);
    let tcp_start = Instant::now();
    // TCP 阶段从整体 deadline 剩余时间分配，且不超过 timeout/3
    let tcp_timeout = std::cmp::min(
        deadline.saturating_duration_since(Instant::now()),
        timeout / 3,
    );
    let tcp_stream = match bind_and_connect(addr, bind_addr, tcp_timeout).await {
        Ok(s) => s,
        Err(e) => {
            result.error = Some(format!("TCP连接失败: {e}"));
            result.total_ms = ms_from(overall_start);
            return result;
        }
    };
    result.tcp_ms = ms_from(tcp_start);

    let tls_start = Instant::now();
    // TLS 阶段从整体 deadline 剩余时间分配，且不超过 timeout/3
    let tls_timeout = std::cmp::min(
        deadline.saturating_duration_since(Instant::now()),
        timeout / 3,
    );
    let (mut tls_stream, negotiated_version) = match do_tls_handshake(host, tcp_stream, tls_timeout).await {
        Ok(r) => r,
        Err(e) => {
            result.error = Some(format!("TLS握手失败: {e}"));
            result.total_ms = ms_from(overall_start);
            return result;
        }
    };
    result.tls_ms = ms_from(tls_start);
    result.tls_version = negotiated_version;

    if !skip_ttfb {
        let request = format!(
            "GET / HTTP/1.1\r\nHost: {host}\r\nUser-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64)\r\nAccept: */*\r\nConnection: close\r\n\r\n"
        );

        let ttfb_start = Instant::now();
        if let Err(e) = tls_stream.write_all(request.as_bytes()).await {
            result.error = Some(format!("发送请求失败: {e}"));
            result.total_ms = ms_from(overall_start);
            return result;
        }

        let mut buf = vec![0u8; 8192];
        let mut total_read = 0usize;
        let mut first_byte_received = false;
        let mut content_start = Instant::now();

        loop {
            // skip_content=true 时，收到第一个字节后即可停止
            if skip_content && first_byte_received {
                break;
            }
            // 每次读取从整体 deadline 剩余时间分配预算，且不超过 timeout/3，避免累加超时
            let http_timeout = std::cmp::min(
                deadline.saturating_duration_since(Instant::now()),
                timeout / 3,
            );
            match tokio::time::timeout(http_timeout, tls_stream.read(&mut buf)).await {
                Ok(Ok(0)) => break,
                Ok(Ok(n)) => {
                    if !first_byte_received {
                        result.ttfb_ms = ms_from(ttfb_start);
                        content_start = Instant::now();
                        first_byte_received = true;
                        if skip_content {
                            break;
                        }
                    }
                    total_read += n;
                    if total_read > 64 * 1024 {
                        break;
                    }
                }
                Ok(Err(e)) => {
                    if !first_byte_received {
                        result.error = Some(format!("读取响应失败: {e}"));
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
            // 未收到任何字节（Ok(0) 直接结束），不应记录 TTFB
            result.ttfb_ms = -1;
            result.content_ms = -1;
        }
    } else {
        result.ttfb_ms = -1;
        result.content_ms = -1;
    }

    result.total_ms = ms_from(overall_start);
    result.success = true;
    result
}

pub(crate) async fn bind_and_connect(
    addr: std::net::SocketAddr,
    bind_addr: Option<IpAddr>,
    timeout: Duration,
) -> Result<TcpStream, String> {
    let stream = if let Some(bind) = bind_addr {
        let bind_addr = std::net::SocketAddr::new(bind, 0);
        let socket = match addr {
            std::net::SocketAddr::V4(_) => {
                tokio::net::TcpSocket::new_v4().map_err(|e| format!("{e}"))?
            }
            std::net::SocketAddr::V6(_) => {
                tokio::net::TcpSocket::new_v6().map_err(|e| format!("{e}"))?
            }
        };
        socket.bind(bind_addr).map_err(|e| format!("绑定失败: {e}"))?;
        tokio::time::timeout(timeout, socket.connect(addr))
            .await
            .map_err(|_| "TCP连接超时".to_string())?
            .map_err(|e| format!("{e}"))?
    } else {
        tokio::time::timeout(timeout, TcpStream::connect(addr))
            .await
            .map_err(|_| "TCP连接超时".to_string())?
            .map_err(|e| format!("{e}"))?
    };
    Ok(stream)
}

pub(crate) async fn do_tls_handshake(
    host: &str,
    tcp_stream: TcpStream,
    timeout: Duration,
) -> Result<(tokio_rustls::client::TlsStream<TcpStream>, String), String> {
    let server_name = ServerName::try_from(host.to_string())
        .map_err(|e| format!("无效主机名: {e}"))?;

    let tls_stream = tokio::time::timeout(timeout, TLS_CONNECTOR.connect(server_name, tcp_stream))
        .await
        .map_err(|_| "TLS握手超时".to_string())?
        .map_err(|e| format!("{e}"))?;

    let version_str = {
        let (_, connection) = tls_stream.get_ref();
        let negotiated = connection.protocol_version();
        match negotiated {
            Some(tokio_rustls::rustls::ProtocolVersion::TLSv1_3) => "TLS 1.3".to_string(),
            Some(tokio_rustls::rustls::ProtocolVersion::TLSv1_2) => "TLS 1.2".to_string(),
            _ => format!("{negotiated:?}"),
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
        super::dns::dns_lookup(server_ip, &query_domain, bind_addr, timeout, hickory_resolver::config::Protocol::Udp),
        super::dns::dns_lookup(server_ip, &query_domain, bind_addr, timeout, hickory_resolver::config::Protocol::Tcp)
    );
    let (_, udp_ms) = udp_result;
    let (_, tcp_ms) = tcp_result;

    let mut udp_err: Option<String> = None;
    let mut tcp_err: Option<String> = None;
    match udp_result {
        (Ok(_), _) => { result.udp_ms = udp_ms; }
        (Err(e), _) => { udp_err = Some(e.to_string()); }
    }
    match tcp_result {
        (Ok(_), _) => { result.tcp_ms = tcp_ms; }
        (Err(e), _) => { tcp_err = Some(e.to_string()); }
    }
    if result.udp_ms < 0 && result.tcp_ms < 0 {
        result.error = match (udp_err, tcp_err) {
            (Some(u), Some(t)) => Some(format!("UDP: {u} | TCP: {t}")),
            (Some(u), None) => Some(format!("UDP: {u}")),
            (None, Some(t)) => Some(format!("TCP: {t}")),
            (None, None) => None,
        };
    }

    result.success = result.udp_ms >= 0 || result.tcp_ms >= 0;
    result
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
    let connect_timeout = std::cmp::min(timeout, Duration::from_secs(5));
    let http_timeout = std::cmp::min(timeout, Duration::from_secs(5));

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
        match super::dns::resolve_host_uncached_with_bind(doh_server, timeout, bind_addr).await {
            Ok(ip) => (ip, true),
            Err(e) => {
                result.error = Some(format!("DoH域名解析失败: {e}"));
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
        match super::dns::resolve_host_uncached_with_bind(doh_server, timeout, bind_addr).await {
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
            Err(e) => {
                crate::log_warn!("timing", "DoH回退DNS解析失败: {}", e);
            }
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
            result.error = Some(format!("TCP连接失败: {e}"));
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
            result.error = Some(format!("TLS握手失败: {e}"));
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

    let query_wire = super::dns::build_dns_query_wire(query_domain, 1);
    let dns_param = super::dns::base64url_encode_no_pad(&query_wire);
    let path = format!("/dns-query?dns={dns_param}");
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {doh_server}\r\nAccept: application/dns-message\r\nUser-Agent: Mozilla/5.0\r\nConnection: close\r\n\r\n"
    );

    let http_start = Instant::now();
    if let Err(e) = tls_stream.write_all(request.as_bytes()).await {
        result.error = Some(format!("发送请求失败: {e}"));
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
                    result.error = Some(format!("读取响应失败: {e}"));
                }
                break;
            }
            Err(_) => {
                if first_read {
                    result.error = Some("DoH响应超时".to_string());
                    result.http_ms = -1;
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
