//! 子网计算、SSID 处理、网关可达性检查
//!
//! 从 `adapter.rs` 迁移，集中网络诊断相关工具。

use crate::network::discovery::new_command;
use std::time::Instant;

/// netsh 查询结果 TTL 缓存有效期（秒）。
/// BE-B-03：后台巡检每 15s 固定调用 SSID/有线 Profile 查询，各 spawn 一次 netsh
/// 子进程。加 TTL 缓存避免高频 spawn；SSID 变化检测由 adapter_watch 15s 周期负责
/// （基于适配器列表而非 SSID 字符串），缓存不会影响变更通知功能。
const NETSH_QUERY_CACHE_TTL_SECS: u64 = 60;

/// netsh 查询缓存条目：(写入时间, 查询结果)。仅缓存 Ok 结果，失败（Err）不缓存以便下次重试。
type NetshCache = parking_lot::Mutex<Option<(Instant, Option<String>)>>;

lazy_static::lazy_static! {
    static ref SSID_CACHE: NetshCache = parking_lot::Mutex::new(None);
    static ref WIRED_PROFILE_CACHE: NetshCache = parking_lot::Mutex::new(None);
}

/// 命中未过期缓存时返回 Some(缓存值)
fn netsh_cache_get(cache: &NetshCache) -> Option<Option<String>> {
    let guard = cache.lock();
    if let Some((ts, val)) = guard.as_ref() {
        if ts.elapsed().as_secs() < NETSH_QUERY_CACHE_TTL_SECS {
            return Some(val.clone());
        }
    }
    None
}

fn netsh_cache_put(cache: &NetshCache, val: Option<String>) {
    *cache.lock() = Some((Instant::now(), val));
}

pub fn get_wireless_ssid() -> Result<Option<String>, String> {
    if let Some(cached) = netsh_cache_get(&SSID_CACHE) {
        return Ok(cached);
    }
    let result = get_wireless_ssid_uncached();
    if let Ok(ref val) = result {
        netsh_cache_put(&SSID_CACHE, val.clone());
    }
    result
}

fn get_wireless_ssid_uncached() -> Result<Option<String>, String> {
    let output = new_command("netsh")
        .args(["wlan", "show", "interfaces"])
        .output()
        .map_err(|e| format!("获取无线网络信息失败: {e}"))?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("SSID") && !trimmed.starts_with("BSSID") {
            let after = trimmed;
            if let Some(colon) = after.find(':') {
                let ssid = after[colon + 1..].trim();
                if !ssid.is_empty()
                    && !ssid.contains("不在")
                    && !ssid.contains("not connected")
                    && !ssid.contains("disconnected")
                {
                    return Ok(Some(ssid.to_string()));
                }
            }
        }
    }

    Ok(None)
}

pub fn get_wired_network_profile() -> Result<Option<String>, String> {
    if let Some(cached) = netsh_cache_get(&WIRED_PROFILE_CACHE) {
        return Ok(cached);
    }
    let result = get_wired_network_profile_uncached();
    if let Ok(ref val) = result {
        netsh_cache_put(&WIRED_PROFILE_CACHE, val.clone());
    }
    result
}

fn get_wired_network_profile_uncached() -> Result<Option<String>, String> {
    let output = new_command("netsh")
        .args(["lan", "show", "interfaces"])
        .output()
        .map_err(|e| format!("获取有线网络信息失败: {e}"))?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        let is_profile_line = trimmed.to_lowercase().contains("profile")
            || trimmed.contains("配置文件")
            || trimmed.contains("設定檔");
        if is_profile_line {
            if let Some(colon) = trimmed.find(':') {
                let name = trimmed[colon + 1..].trim();
                if !name.is_empty() {
                    return Ok(Some(name.to_string()));
                }
            }
        }
    }

    Ok(None)
}

pub fn check_gateway_reachable(gateway: &str) -> bool {
    check_gateway_reachable_from(gateway, None)
}

/// 网关 ICMP 可达性探测（surge_ping 异步实现，替代 spawn `ping` 子进程）。
/// 需要绑定源 IP 时通过 Config::bind 绑定（对应原 `ping -S <src>`）。
/// 返回 true 表示可达，语义与原 `ping -n 1 -w 2000` 一致。
async fn gateway_reachable_async(gateway: &str, source_ip: Option<&str>) -> bool {
    let target: std::net::IpAddr = match gateway.parse() {
        Ok(ip) => ip,
        Err(_) => return false,
    };
    let mut config = surge_ping::Config::builder().kind(match target {
        std::net::IpAddr::V4(_) => surge_ping::ICMP::V4,
        std::net::IpAddr::V6(_) => surge_ping::ICMP::V6,
    });
    if let Some(src) = source_ip {
        if !src.is_empty() {
            if let Ok(src_ip) = src.parse::<std::net::IpAddr>() {
                config = config.bind(std::net::SocketAddr::new(src_ip, 0));
            }
        }
    }
    let client = match surge_ping::Client::new(&config.build()) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let ident = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() & 0xFFFF) as u16;
    let mut pinger = client.pinger(target, surge_ping::PingIdentifier(ident)).await;
    pinger.timeout(std::time::Duration::from_millis(2000));
    // pinger.ping 内部已按 timeout 等待，外层 tokio timeout 仅作硬上限兜底
    matches!(
        tokio::time::timeout(
            std::time::Duration::from_millis(2000),
            pinger.ping(surge_ping::PingSequence(0), &[]),
        )
        .await,
        Ok(Ok((_, _)))
    )
}

pub fn check_gateway_reachable_from(gateway: &str, source_ip: Option<&str>) -> bool {
    if gateway.is_empty() {
        return false;
    }
    // 调用链（campus_check / failure_tracker / background_check）均运行在
    // spawn_blocking 线程内：block_on_sync 用 Handle::block_on 驱动 async 探测，
    // 不会在 async worker 线程上执行（后者会导致 panic）。
    crate::infra::async_util::block_on_sync(gateway_reachable_async(gateway, source_ip))
}

/// 判断 IP 与网关是否在同一 /18 子网（掩码 255.255.192.0）
pub fn is_same_subnet_18(ip_str: &str, gateway_str: &str) -> bool {
    let ip: u32 = match ip_str.parse::<std::net::Ipv4Addr>() {
        Ok(addr) => u32::from(addr),
        Err(_) => return false,
    };
    let gw: u32 = match gateway_str.parse::<std::net::Ipv4Addr>() {
        Ok(addr) => u32::from(addr),
        Err(_) => return false,
    };
    let mask: u32 = 0xFFFF_C000;
    (ip & mask) == (gw & mask)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_same_subnet_18_same_subnet_returns_true() {
        // 10.2.0.0/18 范围：10.2.0.0 - 10.2.63.255
        assert!(is_same_subnet_18("10.2.0.1", "10.2.63.254"));
        assert!(is_same_subnet_18("10.2.0.0", "10.2.0.1"));
        assert!(is_same_subnet_18("10.2.63.255", "10.2.0.0"));
    }

    #[test]
    fn is_same_subnet_18_different_subnet_returns_false() {
        // 10.2.0.0/18 与 10.2.64.0/18 不同子网
        assert!(!is_same_subnet_18("10.2.0.1", "10.2.64.1"));
        // 10.2.63.255 与 10.2.64.0 跨子网边界
        assert!(!is_same_subnet_18("10.2.63.255", "10.2.64.0"));
        // 完全不同的网段
        assert!(!is_same_subnet_18("192.168.1.1", "10.2.0.1"));
    }

    #[test]
    fn is_same_subnet_18_invalid_ip_returns_false() {
        assert!(!is_same_subnet_18("invalid", "10.2.0.1"));
        assert!(!is_same_subnet_18("", "10.2.0.1"));
        assert!(!is_same_subnet_18("10.2.0", "10.2.0.1"));
    }

    #[test]
    fn is_same_subnet_18_invalid_gateway_returns_false() {
        assert!(!is_same_subnet_18("10.2.0.1", "invalid"));
        assert!(!is_same_subnet_18("10.2.0.1", ""));
        assert!(!is_same_subnet_18("10.2.0.1", "10.2.0"));
    }
}
