use std::net::IpAddr;
use std::sync::Arc;
use std::time::Instant;
use arc_swap::ArcSwap;
use dashmap::DashMap;

lazy_static::lazy_static! {
    pub(crate) static ref PORTAL_URL: ArcSwap<String> = ArcSwap::from(Arc::new(crate::config::model::default_portal_url()));
    static ref CLIENT_POOL: DashMap<ClientPoolKey, (reqwest::Client, Instant)> = DashMap::new();
}

/// 客户端池 key 类型：轻量复合键（本机地址、TLS 版本标识、超时毫秒）。
/// 相比原 String 键，热路径零堆分配；仅日志输出时才 format。
type ClientPoolKey = (Option<IpAddr>, u8, u64);

/// TLS 版本 → 紧凑 u8 标识（reqwest::tls::Version 未派生 Hash，无法直接入键；
/// 其内部类型 non_exhaustive，需通配分支）
fn tls_version_id(v: reqwest::tls::Version) -> u8 {
    match v {
        reqwest::tls::Version::TLS_1_0 => 0,
        reqwest::tls::Version::TLS_1_1 => 1,
        reqwest::tls::Version::TLS_1_2 => 2,
        reqwest::tls::Version::TLS_1_3 => 3,
        _ => 4,
    }
}

/// 客户端池 TTL（秒），与 dns.rs DNS_CACHE_TTL_SECS 对齐
const CLIENT_POOL_TTL_SECS: u64 = 600;
/// 客户端池最大容量
const CLIENT_POOL_MAX_ENTRIES: usize = 32;

pub fn update_portal_url(url: &str) {
    if !url.is_empty() {
        PORTAL_URL.store(Arc::new(url.to_string()));
    }
}

fn client_pool_key(local_addr: Option<IpAddr>, min_tls: reqwest::tls::Version, timeout: std::time::Duration) -> ClientPoolKey {
    (local_addr, tls_version_id(min_tls), timeout.as_millis() as u64)
}

fn build_client(timeout: std::time::Duration, local_addr: Option<IpAddr>, min_tls: reqwest::tls::Version) -> Result<reqwest::Client, String> {
    let mut default_headers = reqwest::header::HeaderMap::new();
    default_headers.insert(
        reqwest::header::CACHE_CONTROL,
        reqwest::header::HeaderValue::from_static("no-store"),
    );
    default_headers.insert(
        reqwest::header::PRAGMA,
        reqwest::header::HeaderValue::from_static("no-cache"),
    );

    let mut builder = reqwest::Client::builder()
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

/// 命中检查：返回 Some(client) 表示有效命中；返回 None 表示未命中或已过期清除
/// 命中时更新 Instant 为当前时间（LRU 按访问时间淘汰，非 FIFO 按创建时间淘汰）
fn client_pool_get(key: &ClientPoolKey, label: &str) -> Option<reqwest::Client> {
    let mut entry = CLIENT_POOL.get_mut(key)?;
    let (client, instant) = entry.value_mut();
    if instant.elapsed().as_secs() < CLIENT_POOL_TTL_SECS {
        *instant = Instant::now();
        crate::log_debug!("http", "客户端池命中{}: key={:?}", label, key);
        Some(client.clone())
    } else {
        drop(entry);
        // remove_if 原子判断+删除：drop(entry) 与 remove 之间另一线程可能
        // or_insert 新客户端，无条件的 remove 会误删新条目
        CLIENT_POOL.remove_if(key, |_, v| v.1.elapsed().as_secs() >= CLIENT_POOL_TTL_SECS);
        crate::log_debug!("http", "客户端池TTL过期清除{}: key={:?}", label, key);
        None
    }
}

pub fn create_safe_http_client(timeout: std::time::Duration, local_addr: Option<IpAddr>) -> Result<reqwest::Client, String> {
    let tls13_key = client_pool_key(local_addr, reqwest::tls::Version::TLS_1_3, timeout);
    if let Some(client) = client_pool_get(&tls13_key, "(TLS 1.3)") {
        return Ok(client);
    }

    let tls12_key = client_pool_key(local_addr, reqwest::tls::Version::TLS_1_2, timeout);
    if let Some(client) = client_pool_get(&tls12_key, "(TLS 1.2 fallback)") {
        return Ok(client);
    }

    let (client, actual_key) = match build_client(timeout, local_addr, reqwest::tls::Version::TLS_1_3) {
        Ok(c) => {
            crate::log_info!("http", "客户端池新建: key={:?}, poolSize={}", tls13_key, CLIENT_POOL.len() + 1);
            (c, tls13_key)
        }
        Err(_) => {
            crate::log_info!("http", "客户端池新建(TLS 1.2 fallback): key={:?}, poolSize={}", tls12_key, CLIENT_POOL.len() + 1);
            let c = build_client(timeout, local_addr, reqwest::tls::Version::TLS_1_2)
                .map_err(|e| format!("TLS 1.3/1.2 客户端均构建失败: {e}"))?;
            (c, tls12_key)
        }
    };

    CLIENT_POOL.entry(actual_key).or_insert_with(|| (client.clone(), Instant::now()));
    // 容量上限清理：按 Instant 找最久未访问条目剔除（LRU 按访问时间淘汰）
    while CLIENT_POOL.len() > CLIENT_POOL_MAX_ENTRIES {
        if let Some(entry) = CLIENT_POOL.iter().min_by_key(|e| e.value().1) {
            let key = entry.key().clone();
            drop(entry);
            CLIENT_POOL.remove(&key);
        } else {
            break;
        }
    }
    Ok(client)
}
