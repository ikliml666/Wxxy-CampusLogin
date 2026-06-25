use std::net::IpAddr;
use std::sync::Arc;
use arc_swap::ArcSwap;
use dashmap::DashMap;

lazy_static::lazy_static! {
    pub(crate) static ref PORTAL_URL: ArcSwap<String> = ArcSwap::from(Arc::new(crate::config::model::default_portal_url()));
    static ref CLIENT_POOL: DashMap<String, reqwest::blocking::Client> = DashMap::new();
}

pub fn update_portal_url(url: &str) {
    if !url.is_empty() {
        PORTAL_URL.store(Arc::new(url.to_string()));
    }
}

fn client_pool_key(local_addr: Option<IpAddr>, min_tls: reqwest::tls::Version, timeout: std::time::Duration) -> String {
    match local_addr {
        Some(ip) => format!("{}:{:?}:{}", ip, min_tls, timeout.as_millis()),
        None => format!("none:{:?}:{}", min_tls, timeout.as_millis()),
    }
}

fn build_client(timeout: std::time::Duration, local_addr: Option<IpAddr>, min_tls: reqwest::tls::Version) -> Result<reqwest::blocking::Client, String> {
    let mut default_headers = reqwest::header::HeaderMap::new();
    default_headers.insert(
        reqwest::header::CACHE_CONTROL,
        reqwest::header::HeaderValue::from_static("no-store"),
    );
    default_headers.insert(
        reqwest::header::PRAGMA,
        reqwest::header::HeaderValue::from_static("no-cache"),
    );

    let mut builder = reqwest::blocking::Client::builder()
        .min_tls_version(min_tls)
        .timeout(timeout)
        .connect_timeout(std::time::Duration::from_secs(3))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::limited(5))
        .pool_max_idle_per_host(4)
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .tcp_keepalive(std::time::Duration::from_secs(30))
        .default_headers(default_headers);

    if let Some(ip) = local_addr {
        builder = builder.local_address(ip);
    }

    builder.build().map_err(|e| format!("创建HTTP客户端失败: {e}"))
}

pub fn create_safe_http_client(timeout: std::time::Duration, local_addr: Option<IpAddr>) -> Result<reqwest::blocking::Client, String> {
    let tls13_key = client_pool_key(local_addr, reqwest::tls::Version::TLS_1_3, timeout);
    if let Some(entry) = CLIENT_POOL.get(&tls13_key) {
        crate::log_debug!("http", "客户端池命中: key={}", tls13_key);
        return Ok(entry.value().clone());
    }

    let tls12_key = client_pool_key(local_addr, reqwest::tls::Version::TLS_1_2, timeout);
    if let Some(entry) = CLIENT_POOL.get(&tls12_key) {
        crate::log_debug!("http", "客户端池命中(TLS 1.2 fallback): key={}", tls12_key);
        return Ok(entry.value().clone());
    }

    let (client, actual_key) = match build_client(timeout, local_addr, reqwest::tls::Version::TLS_1_3) {
        Ok(c) => {
            crate::log_info!("http", "客户端池新建: key={}, poolSize={}", tls13_key, CLIENT_POOL.len() + 1);
            (c, tls13_key)
        }
        Err(_) => {
            crate::log_info!("http", "客户端池新建(TLS 1.2 fallback): key={}, poolSize={}", tls12_key, CLIENT_POOL.len() + 1);
            let c = build_client(timeout, local_addr, reqwest::tls::Version::TLS_1_2)
                .map_err(|e| format!("TLS 1.3/1.2 客户端均构建失败: {e}"))?;
            (c, tls12_key)
        }
    };

    CLIENT_POOL.entry(actual_key).or_insert_with(|| client.clone());
    // 容量上限清理，避免无界增长（与 dns.rs DNS_CACHE 模式一致）
    const CLIENT_POOL_MAX_ENTRIES: usize = 32;
    while CLIENT_POOL.len() > CLIENT_POOL_MAX_ENTRIES {
        if let Some(entry) = CLIENT_POOL.iter().next() {
            let key = entry.key().clone();
            drop(entry);
            CLIENT_POOL.remove(&key);
        } else {
            break;
        }
    }
    Ok(client)
}
