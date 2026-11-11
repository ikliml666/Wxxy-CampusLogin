use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Instant;
use arc_swap::ArcSwap;
use dashmap::DashMap;

#[cfg(test)]
static CLIENT_TEST_TLS13_BUILD_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(test)]
static CLIENT_TEST_TLS12_BUILD_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(test)]
static CLIENT_TEST_COLD_START_BARRIER: parking_lot::Mutex<Option<Arc<std::sync::Barrier>>> = parking_lot::Mutex::new(None);
#[cfg(test)]
static CLIENT_TEST_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

lazy_static::lazy_static! {
    pub(crate) static ref PORTAL_URL: ArcSwap<String> = ArcSwap::from(Arc::new(crate::config::model::default_portal_url()));
    static ref CLIENT_POOL: DashMap<ClientPoolKey, (reqwest::Client, Instant)> = DashMap::new();
    static ref CLIENT_POOL_BUILD_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
}

/// 客户端池 key：本机地址、TLS 版本、超时与旁路代理地址共同决定连接池代际。
/// 旁路客户端不绑定 local_address，其 key 的本机地址固定为 None。
type ClientPoolKey = (Option<IpAddr>, u8, u64, Option<SocketAddr>);

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

fn client_pool_key(local_addr: Option<IpAddr>, min_tls: reqwest::tls::Version, timeout: std::time::Duration, bypass_addr: Option<SocketAddr>) -> ClientPoolKey {
    let direct_addr = if bypass_addr.is_some() { None } else { local_addr };
    (direct_addr, tls_version_id(min_tls), timeout.as_millis() as u64, bypass_addr)
}

fn build_client(timeout: std::time::Duration, local_addr: Option<IpAddr>, min_tls: reqwest::tls::Version, bypass_addr: Option<SocketAddr>) -> Result<reqwest::Client, String> {
    #[cfg(test)]
    match tls_version_id(min_tls) {
        3 => CLIENT_TEST_TLS13_BUILD_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
        2 => CLIENT_TEST_TLS12_BUILD_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
        _ => unreachable!("客户端池只构建 TLS 1.3 或 TLS 1.2"),
    };
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
        .redirect(reqwest::redirect::Policy::limited(5))
        .pool_max_idle_per_host(4)
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .tcp_keepalive(std::time::Duration::from_secs(30))
        .default_headers(default_headers);

    if let Some(addr) = bypass_addr {
        let proxy = reqwest::Proxy::all(format!("http://{addr}"))
            .map_err(|e| format!("旁路代理配置失败: {e}"))?;
        builder = builder.proxy(proxy);
    } else {
        builder = builder.no_proxy();
        if let Some(ip) = local_addr {
            builder = builder.local_address(ip);
        }
    }

    builder.build().map_err(|e| {
        if bypass_addr.is_some() {
            format!("创建旁路HTTP客户端失败: {e}")
        } else {
            format!("创建HTTP客户端失败: {e}")
        }
    })
}

/// 命中检查：返回 Some(client) 表示有效命中；返回 None 表示未命中或已过期清除。
/// 命中路径用读锁取 clone 返回、不刷新时间戳（get_mut 写锁会串行化同 key 并发请求）；
/// 代价是淘汰退化为按条目插入时间（原 LRU 按访问时间），精度略降可接受，TTL 语义不变。
fn client_pool_get(key: &ClientPoolKey, label: &str) -> Option<reqwest::Client> {
    let entry = CLIENT_POOL.get(key)?;
    let (client, instant) = entry.value();
    if instant.elapsed().as_secs() < CLIENT_POOL_TTL_SECS {
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

fn create_http_client_with_bypass(
    timeout: std::time::Duration,
    local_addr: Option<IpAddr>,
    bypass_addr: Option<SocketAddr>,
) -> Result<reqwest::Client, String> {
    let tls13_key = client_pool_key(local_addr, reqwest::tls::Version::TLS_1_3, timeout, bypass_addr);
    if let Some(client) = client_pool_get(&tls13_key, "(TLS 1.3)") {
        return Ok(client);
    }

    let tls12_key = client_pool_key(local_addr, reqwest::tls::Version::TLS_1_2, timeout, bypass_addr);
    if let Some(client) = client_pool_get(&tls12_key, "(TLS 1.2 fallback)") {
        return Ok(client);
    }

    #[cfg(test)]
    {
        let barrier = CLIENT_TEST_COLD_START_BARRIER.lock().clone();
        if let Some(barrier) = barrier {
            barrier.wait();
        }
    }

    // 构建本身不发起网络 IO；只在冷启动慢路径串行化，避免并发重复初始化和返回不同连接池。
    let _build_guard = CLIENT_POOL_BUILD_LOCK.lock();
    if let Some(client) = client_pool_get(&tls13_key, "(TLS 1.3)") {
        return Ok(client);
    }
    if let Some(client) = client_pool_get(&tls12_key, "(TLS 1.2 fallback)") {
        return Ok(client);
    }

    let (client, actual_key) = match build_client(timeout, local_addr, reqwest::tls::Version::TLS_1_3, bypass_addr) {
        Ok(c) => {
            crate::log_info!("http", "客户端池新建: key={:?}, poolSize={}", tls13_key, CLIENT_POOL.len() + 1);
            (c, tls13_key)
        }
        Err(_) => {
            crate::log_info!("http", "客户端池新建(TLS 1.2 fallback): key={:?}, poolSize={}", tls12_key, CLIENT_POOL.len() + 1);
            let c = build_client(timeout, local_addr, reqwest::tls::Version::TLS_1_2, bypass_addr)
                .map_err(|e| format!("TLS 1.3/1.2 客户端均构建失败: {e}"))?;
            (c, tls12_key)
        }
    };

    CLIENT_POOL.insert(actual_key, (client.clone(), Instant::now()));
    // 容量上限清理：按 Instant 找最早插入的条目剔除（命中路径不刷新时间戳，见 client_pool_get）
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

/// 仅创建普通直连客户端；Portal 保留旁路构建失败时的既有回退路径。
#[cfg(target_os = "android")]
pub(crate) fn create_direct_http_client(timeout: std::time::Duration, local_addr: Option<IpAddr>) -> Result<reqwest::Client, String> {
    create_http_client_with_bypass(timeout, local_addr, None)
}

/// 统一获取 HTTP 客户端；旁路选择在此完成，避免业务层先创建普通 client 再丢弃。
pub fn create_safe_http_client(timeout: std::time::Duration, local_addr: Option<IpAddr>) -> Result<reqwest::Client, String> {
    #[cfg(target_os = "android")]
    let bypass_addr = crate::network::bound_socket::bypass_proxy_addr().copied();
    #[cfg(not(target_os = "android"))]
    let bypass_addr = None;
    create_http_client_with_bypass(timeout, local_addr, bypass_addr)
}

/// 保留给库调用方的显式旁路入口；内部业务统一使用 create_safe_http_client。
#[cfg(target_os = "android")]
pub fn create_bypass_http_client(timeout: std::time::Duration) -> Result<reqwest::Client, String> {
    let Some(addr) = crate::network::bound_socket::bypass_proxy_addr() else {
        return Err("旁路代理未启动".to_string());
    };
    create_http_client_with_bypass(timeout, None, Some(*addr))
}

/// 清空 HTTP 客户端池，返回被清理的条目数。
///
/// **网络路由变化后必须调用**：安卓端 `bindProcessToNetwork` 的 fwmark 只在
/// **socket 创建时**由 netd 的 eBPF 打标，池里 keep-alive 的既有连接仍走绑定前的
/// 路由——复用它们等于绑定没生效（登录/探测/注销全链路都吃这个坑）。代价是
/// 下一批请求重建 TCP+TLS；调用点都在操作发起前，此时没有在途请求，故安全。
// 桌面 bin 不调用（无多网络路由切换）；仅安卓端经 campus_login_lib 路径依赖调用。
#[allow(dead_code)]
pub fn clear_client_pool() -> usize {
    let count = CLIENT_POOL.len();
    if count > 0 {
        CLIENT_POOL.clear();
        crate::log_info!("http", "客户端池已清空: {} 条（网络路由变化，避免复用旧路由连接）", count);
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddr};
    use std::sync::Barrier;
    use std::time::Duration;

    #[test]
    fn 冷启动并发每个_tls版本至多构建一次() {
        let _guard = CLIENT_TEST_LOCK.lock();
        clear_client_pool();
        CLIENT_TEST_TLS13_BUILD_COUNT.store(0, std::sync::atomic::Ordering::SeqCst);
        CLIENT_TEST_TLS12_BUILD_COUNT.store(0, std::sync::atomic::Ordering::SeqCst);
        const WORKERS: usize = 8;
        let barrier = Arc::new(Barrier::new(WORKERS));
        *CLIENT_TEST_COLD_START_BARRIER.lock() = Some(barrier.clone());
        let mut workers = Vec::new();
        for _ in 0..WORKERS {
            let barrier = barrier.clone();
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                create_safe_http_client(Duration::from_secs(17), None).unwrap();
            }));
        }
        for worker in workers {
            worker.join().unwrap();
        }
        *CLIENT_TEST_COLD_START_BARRIER.lock() = None;

        let tls13_builds = CLIENT_TEST_TLS13_BUILD_COUNT.load(std::sync::atomic::Ordering::SeqCst);
        let tls12_builds = CLIENT_TEST_TLS12_BUILD_COUNT.load(std::sync::atomic::Ordering::SeqCst);
        assert!(tls13_builds <= 1, "TLS 1.3 冷启动并发重复构建 {tls13_builds} 次");
        assert!(tls12_builds <= 1, "TLS 1.2 冷启动并发重复构建 {tls12_builds} 次");
        let total_builds = tls13_builds + tls12_builds;
        assert!((1..=2).contains(&total_builds),
            "一个客户端最多经历 TLS 1.3 失败和 TLS 1.2 成功两次构建，实际 {total_builds} 次");
        assert_eq!(CLIENT_POOL.len(), 1);
        clear_client_pool();
    }

    #[test]
    fn 统一入口按旁路地址与超时复用并在路由切换时整体失效() {
        let _guard = CLIENT_TEST_LOCK.lock();
        clear_client_pool();
        let timeout = Duration::from_secs(15);
        let bypass = SocketAddr::from((Ipv4Addr::LOCALHOST, 18080));

        create_http_client_with_bypass(timeout, None, None).unwrap();
        assert_eq!(CLIENT_POOL.len(), 1, "旁路不可用时应只创建普通客户端");
        assert!(CLIENT_POOL.iter().all(|entry| entry.key().3.is_none()));

        create_http_client_with_bypass(timeout, None, Some(bypass)).unwrap();
        create_http_client_with_bypass(timeout, None, Some(bypass)).unwrap();
        assert_eq!(CLIENT_POOL.len(), 2, "同一旁路地址与超时应复用缓存");

        create_http_client_with_bypass(Duration::from_secs(3), None, Some(bypass)).unwrap();
        assert_eq!(CLIENT_POOL.len(), 3, "超时不属于同一代际缓存键时不得复用");

        let restarted = SocketAddr::from((Ipv4Addr::LOCALHOST, 18081));
        create_http_client_with_bypass(timeout, None, Some(restarted)).unwrap();
        assert_eq!(CLIENT_POOL.len(), 4, "代理重启后的新地址不得复用旧客户端");

        assert_eq!(clear_client_pool(), 4, "路由切换必须同时清理普通与旁路客户端");
        assert!(CLIENT_POOL.is_empty());

        create_http_client_with_bypass(timeout, None, Some(restarted)).unwrap();
        assert_eq!(CLIENT_POOL.len(), 1, "清池后的请求必须重新构建客户端");
    }
}
