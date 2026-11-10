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

/// SSID 查询失败告警只发一次（进程生命周期内）。Win11 关闭位置服务后
/// netsh wlan 永久失败，后台巡检高频调用，重复告警会刷爆日志。
static SSID_QUERY_WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn warn_ssid_query_failed(reason: &str) {
    if !SSID_QUERY_WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        crate::log_warn!("campus", "[校园网检测] WiFi SSID 查询失败（{}）：WiFi 名称匹配判定腿失效，仅剩子网/网关/Portal 探测。Win11 需在系统设置→隐私和安全性→位置中开启位置服务", reason);
    }
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
        warn_ssid_query_failed("netsh 退出码非 0");
        return Ok(None);
    }

    let stdout = crate::platform::console_output::decode_console_bytes(&output.stdout);
    // Win11 位置服务关闭时 netsh wlan 可能正常退出但输出错误提示而非 SSID
    if stdout.contains("位置权限") || stdout.to_lowercase().contains("location permission") {
        warn_ssid_query_failed("系统位置服务未开启");
        return Ok(None);
    }
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

    let stdout = crate::platform::console_output::decode_console_bytes(&output.stdout);
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

/// 绑源可达性探测的单次超时（毫秒）。
///
/// 探测目标都是本机所在局域网/校园网的内网地址（网关、portal），正常 RTT 远小于 10ms。
/// 2000ms 是为慢网络留的保守余量，但断网时这段超时会被逐卡多次探测累加进夜间出站
/// 切换耗时（2026-11-08 实测切换 9.8s，主体就是这些超时），故取 1000ms：既容得下
/// 拥塞/首包丢包，又不会把切点推迟两秒。
pub const GATEWAY_PROBE_TIMEOUT_MS: u32 = 1000;

/// 网关 ICMP 可达性探测（surge_ping 异步实现，替代 spawn `ping` 子进程）。
/// 需要绑定源 IP 时通过 Config::bind 绑定（对应原 `ping -S <src>`）。
/// 返回 true 表示可达，语义等价原 `ping -n 1 -w <GATEWAY_PROBE_TIMEOUT_MS>`。
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
    pinger.timeout(std::time::Duration::from_millis(GATEWAY_PROBE_TIMEOUT_MS as u64));
    // pinger.ping 内部已按 timeout 等待，外层 tokio timeout 仅作硬上限兜底
    matches!(
        tokio::time::timeout(
            std::time::Duration::from_millis(GATEWAY_PROBE_TIMEOUT_MS as u64),
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
    // Windows 优先走 IcmpSendEcho2Ex（官方探测通道，绑源可靠）：surge_ping 的
    // socket2 绑源 DGRAM 在 TUN 环境下会系统性失败，导致校园网卡被误判为
    // 非校园（夜间禁用名单漏卡），见 platform/icmp_probe.rs 模块注释。
    // 仅替换 IPv4 路径；v6 网关与非 Windows 平台仍走下方 surge_ping。
    #[cfg(target_os = "windows")]
    if let Ok(dest) = gateway.parse::<std::net::Ipv4Addr>() {
        let source = source_ip.and_then(|s| s.parse::<std::net::Ipv4Addr>().ok());
        return crate::platform::icmp_probe::icmp_probe_v4(dest, source, GATEWAY_PROBE_TIMEOUT_MS);
    }
    // 调用链（campus_check / failure_tracker / background_check）均运行在
    // spawn_blocking 线程内：block_on_sync 用 Handle::block_on 驱动 async 探测，
    // 不会在 async worker 线程上执行（后者会导致 panic）。
    crate::infra::async_util::block_on_sync(gateway_reachable_async(gateway, source_ip))
}

/// 判断 IP 是否落在校园私网 `10.0.0.0/8`。
///
/// 2026-11-10：校园网**判定**已取消网段腿（网关基准落在核心出口 `10.1.1.1`，
/// 而接入网在 `10.2.x`，两者不同 /16，按网关划网段已无判定价值；判定改由绑源
/// 网关/Portal 探测承担，见 `monitor/campus_check.rs` 与 `monitor/outbound_switch.rs`）。
/// 本函数仅服务于一处**取舍**：`network/dhcp.rs` 判断「换本机 MAC 是否有意义」——
/// 本机 IP 不在校园私网（如路由器后的 `192.168.x`）时换本机 MAC 无用，需到路由器侧换。
/// 因此这里用整张校园私网 /8，与「是否校园网」解耦。
pub fn is_private_campus_range(ip_str: &str) -> bool {
    let ip: u32 = match ip_str.parse::<std::net::Ipv4Addr>() {
        Ok(addr) => u32::from(addr),
        Err(_) => return false,
    };
    let mask: u32 = 0xFF00_0000; // 255.0.0.0
    (ip & mask) == (10u32 << 24)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_private_campus_range_accepts_campus_private_ranges() {
        // 校园私网 10.0.0.0/8：接入网 10.2、核心出口 10.1、业务 10.3 都算
        assert!(is_private_campus_range("10.2.38.176"));
        assert!(is_private_campus_range("10.1.1.1"));
        assert!(is_private_campus_range("10.1.99.100"));
        assert!(is_private_campus_range("10.3.100.110"));
        assert!(is_private_campus_range("10.0.0.1"));
        assert!(is_private_campus_range("10.255.255.255"));
    }

    #[test]
    fn is_private_campus_range_rejects_other_ranges() {
        // 家用/热点私网与链路本地地址都不是校园私网
        assert!(!is_private_campus_range("192.168.6.106"));
        assert!(!is_private_campus_range("172.16.0.1"));
        assert!(!is_private_campus_range("169.254.60.55"));
        assert!(!is_private_campus_range("223.5.5.5"));
        assert!(!is_private_campus_range("11.0.0.1"));
    }

    #[test]
    fn is_private_campus_range_invalid_returns_false() {
        assert!(!is_private_campus_range("invalid"));
        assert!(!is_private_campus_range(""));
        assert!(!is_private_campus_range("10.2.0"));
    }
}
