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
        crate::log_warn!("campus", "[校园网检测] WiFi SSID 查询失败（{}）：无线卡名称匹配判定腿失效，无线卡仅剩网关/Portal 探测（2026-11-10 起已无网段腿）。Win11 需在系统设置→隐私和安全性→位置中开启位置服务", reason);
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

    let stdout = crate::platform::console_output::decode_console_bytes(&output.stdout);
    // Win11 位置服务关闭时 netsh wlan 报位置权限错误（实测退出码 1 且带
    // 中文提示），必须在退出码检查之前识别，否则被"退出码非 0"分支吞掉
    if stdout.contains("位置权限") || stdout.to_lowercase().contains("location permission") {
        // 免定位兜底，按优先级：NLA COM（实时连接状态）→ WLAN 事件日志（最近事件推断）。
        // 2026-11-18 本机实测：netsh 被拒的同一台机器上两者均给出真实 SSID；
        // NLA 反映实时状态无事件日志的陈旧读数缺陷，故优先。
        // NLA COM 依赖 windows crate（定义处 cfg(windows)）；安卓交叉编译时跳过，
        // 该分支本就只在 netsh 输出含位置权限提示（Windows 行为）时可达。
        #[cfg(target_os = "windows")]
        if let Ok(v) = ssid_from_nla() {
            return Ok(v);
        }
        return match ssid_from_wlan_eventlog() {
            Ok(v) => Ok(v),
            Err(reason) => {
                warn_ssid_query_failed(&format!("系统位置服务未开启，NLA 与事件日志兜底均不可用（{reason}）"));
                Ok(None)
            }
        };
    }

    if !output.status.success() {
        warn_ssid_query_failed("netsh 退出码非 0");
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

/// Win11 关位置服务时 netsh 拿不到 SSID 的首选兜底：NLA（网络位置感知）
/// NetworkListManager COM——网络名不在位置权限管控栈内（2026-11-18 本机实测：
/// netsh 被拒的同一台机器上 COM 枚举给出真实 SSID，且反映实时连接状态）。
/// WiFi 网络的 NLA 网络名默认即 SSID；按「无线适配器 GUID」过滤，避免把
/// VPN TUN / 有线网的 NLA 名误当 SSID（本机实测 FlClash TUN 也在列表中）。
/// 无线卡存在但 NLA 无其连接 → Ok(None)（实时状态：未连接）；
/// COM 不可用 / 无线 GUID 集合为空 / 枚举失败 → Err（交由调用方走下一级兜底）。
#[cfg(target_os = "windows")]
fn ssid_from_nla() -> Result<Option<String>, String> {
    use windows::core::GUID;
    use windows::Win32::Networking::NetworkListManager::{
        INetworkConnection, INetworkListManager, IEnumNetworkConnections,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    // CLSID_NetworkListManager {DCB00C01-570F-4A9B-8D69-199FDBA5723B}：
    // windows 0.58 的 coclass 绑定不稳定，直接手写常量
    const CLSID_NETWORK_LIST_MANAGER: GUID = GUID::from_values(
        0xDCB00C01, 0x570F, 0x4A9B, [0x8D, 0x69, 0x19, 0x9F, 0xDB, 0xA5, 0x72, 0x3B],
    );

    // 无线适配器 GUID 集合（复用适配器枚举；格式 "{...}"，统一去括号大写后比较）
    let (adapters, _, _) = crate::network::discovery::query_adapters_addresses()
        .map_err(|e| format!("枚举适配器失败: {e}"))?;
    let wireless_guids: Vec<String> = adapters
        .iter()
        .filter(|a| a.wireless)
        .map(|a| a.guid.trim_matches(|c| c == '{' || c == '}').to_uppercase())
        .collect();
    if wireless_guids.is_empty() {
        return Err("无无线适配器".to_string());
    }

    unsafe {
        // 调用线程可能已有 STA 初始化（RPC_E_CHANGED_MODE），此时沿用现有初始化
        let co = CoInitializeEx(None, COINIT_MULTITHREADED);
        let need_uninit = co.is_ok();

        let result = (|| -> Result<Option<String>, String> {
            let nlm: INetworkListManager =
                CoCreateInstance(&CLSID_NETWORK_LIST_MANAGER, None, CLSCTX_ALL)
                    .map_err(|e| format!("创建 NetworkListManager 失败: {e}"))?;
            let conns: IEnumNetworkConnections = nlm
                .GetNetworkConnections()
                .map_err(|e| format!("枚举网络连接失败: {e}"))?;

            let mut items: [Option<INetworkConnection>; 1] = [None];
            let mut fetched: u32 = 0;
            loop {
                if conns.Next(&mut items, Some(&mut fetched)).is_err() || fetched == 0 {
                    break;
                }
                let Some(conn) = items[0].take() else { break };
                let adapter_guid = conn
                    .GetAdapterId()
                    .map_err(|e| format!("读连接适配器失败: {e}"))?;
                let g = adapter_guid;
                let normalized = format!(
                    "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
                    g.data1, g.data2, g.data3,
                    g.data4[0], g.data4[1], g.data4[2], g.data4[3],
                    g.data4[4], g.data4[5], g.data4[6], g.data4[7],
                );
                if wireless_guids.iter().any(|g| *g == normalized) {
                    // 无线连接：NLA 网络名即 SSID；名字为空视为异常交下一级兜底
                    let network = conn
                        .GetNetwork()
                        .map_err(|e| format!("读网络对象失败: {e}"))?;
                    let name = network
                        .GetName()
                        .map_err(|e| format!("读网络名失败: {e}"))?
                        .to_string();
                    if name.is_empty() {
                        return Err("无线 NLA 网络名为空".to_string());
                    }
                    return Ok(Some(name));
                }
            }
            // 枚举完整结束仍无无线连接：实时状态为未连接
            Ok(None)
        })();

        if need_uninit {
            CoUninitialize();
        }
        result
    }
}

/// Win11 关位置服务时 netsh 拿不到 SSID 的最后兜底：查 WLAN-AutoConfig 操作日志的
/// 最近一次 8001（连接成功）/8003（断开）——事件日志不经过位置权限检查。
/// 8001 → 当前 SSID；8003 → None（未连接）；日志被清空/解析失败 → Err。
/// 局限：结论来自最近一次连接/断开事件，日志通道被禁期间的状态变化无从感知。
fn ssid_from_wlan_eventlog() -> Result<Option<String>, String> {
    let output = new_command("wevtutil")
        .args([
            "qe",
            "Microsoft-Windows-WLAN-AutoConfig/Operational",
            "/q:*[System[(EventID=8001) or (EventID=8003)]]",
            "/c:4",
            "/rd:true",
            "/f:text",
        ])
        .output()
        .map_err(|e| format!("查询 WLAN 事件日志失败: {e}"))?;
    if !output.status.success() {
        return Err("wevtutil 退出码非 0".to_string());
    }
    let stdout = crate::platform::console_output::decode_console_bytes(&output.stdout);
    parse_wlan_eventlog_ssid(&stdout)
}

/// wevtutil `/f:text` 输出按 `Event[N]` 分段；只看第一个（最新）事件的
/// Event ID 与行首 `SSID:` 行（键不本地化，中文系统实测同为英文键；
/// BSSID 行以 B 开头不会误匹配）。
fn parse_wlan_eventlog_ssid(stdout: &str) -> Result<Option<String>, String> {
    let mut in_first_event = false;
    let mut event_id: Option<u32> = None;
    let mut ssid: Option<String> = None;
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Event[") {
            if in_first_event {
                break; // 第二个事件开始，第一段已完整
            }
            in_first_event = true;
        } else if in_first_event {
            if let Some(rest) = trimmed.strip_prefix("Event ID:") {
                event_id = rest.trim().parse().ok();
            } else if trimmed.starts_with("SSID:") {
                let value = trimmed["SSID:".len()..].trim();
                if !value.is_empty() {
                    ssid = Some(value.to_string());
                }
            }
        }
    }
    match (in_first_event, event_id) {
        (true, Some(8001)) => {
            ssid.map(Some).ok_or_else(|| "8001 事件缺少 SSID 行".to_string())
        }
        (true, Some(8003)) => Ok(None),
        _ => Err("未找到 8001/8003 事件".to_string()),
    }
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
async fn gateway_reachable_async(gateway: &str, source_ip: Option<&str>, timeout_ms: u32) -> bool {
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
    pinger.timeout(std::time::Duration::from_millis(timeout_ms as u64));
    // pinger.ping 内部已按 timeout 等待，外层 tokio timeout 仅作硬上限兜底
    matches!(
        tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms as u64),
            pinger.ping(surge_ping::PingSequence(0), &[]),
        )
        .await,
        Ok(Ok((_, _)))
    )
}

pub fn check_gateway_reachable_from(gateway: &str, source_ip: Option<&str>) -> bool {
    check_gateway_reachable_from_with_timeout(gateway, source_ip, GATEWAY_PROBE_TIMEOUT_MS)
}

/// 同 check_gateway_reachable_from，但可指定单次超时。夜间切换的探测池有自己的总预算，
/// ICMP 腿只能拿到「预算扣掉 TCP 腿」的剩余时间，不能再走全局 1000ms。
pub fn check_gateway_reachable_from_with_timeout(
    gateway: &str,
    source_ip: Option<&str>,
    timeout_ms: u32,
) -> bool {
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
        return crate::platform::icmp_probe::icmp_probe_v4(dest, source, timeout_ms);
    }
    // 调用链（campus_check / failure_tracker / background_check）均运行在
    // spawn_blocking 线程内：block_on_sync 用 Handle::block_on 驱动 async 探测，
    // 不会在 async worker 线程上执行（后者会导致 panic）。
    crate::infra::async_util::block_on_sync(gateway_reachable_async(gateway, source_ip, timeout_ms))
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

    /// 真机验证（本机 Win11 位置服务关闭 + WiFi 连接中）：
    /// NLA COM 通道应给出与实际一致的 SSID。
    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "需真实 Windows 环境（无线连接中）"]
    fn nla通道_位置关闭时返回真实ssid() {
        match ssid_from_nla() {
            Ok(Some(ssid)) => eprintln!("NLA SSID = {ssid}"),
            Ok(None) => panic!("无线已连接但 NLA 报未连接"),
            Err(e) => panic!("NLA 通道失败: {e}"),
        }
    }

    #[test]
    fn 事件日志解析_最新8001返回ssid() {
        let text = "Event[0]\n  Event ID: 8001\n  SSID: kiku-5G\n  BSS 类型: Infrastructure\n\nEvent[1]\n  Event ID: 8003\n  SSID: 旧网络\n";
        assert_eq!(parse_wlan_eventlog_ssid(text), Ok(Some("kiku-5G".to_string())));
    }

    #[test]
    fn 事件日志解析_最新8003返回未连接() {
        let text = "Event[0]\n  Event ID: 8003\n  SSID: kiku-5G\n\nEvent[1]\n  Event ID: 8001\n  SSID: kiku-5G\n";
        assert_eq!(parse_wlan_eventlog_ssid(text), Ok(None));
    }

    #[test]
    fn 事件日志解析_无事件或残缺报错() {
        // 空日志、非目标事件、8001 缺 SSID 行都如实报错
        assert!(parse_wlan_eventlog_ssid("").is_err());
        assert!(parse_wlan_eventlog_ssid("Event[0]\n  Event ID: 8002\n").is_err());
        assert!(parse_wlan_eventlog_ssid("Event[0]\n  Event ID: 8001\n").is_err());
    }
}
