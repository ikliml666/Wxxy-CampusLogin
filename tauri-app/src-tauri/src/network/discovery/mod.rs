//! 适配器发现模块（跨平台类型 + 平台分发）
//!
//! 本模块只负责"发现适配器"（调用系统 API 获取适配器列表），
//! 不涉及缓存、DHCP、MAC 重置等逻辑（分别由 adapter_cache/dhcp/subnet 模块负责）。

use regex::Regex;
use serde::Serialize;
use lazy_static::lazy_static;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
pub mod registry;

pub(crate) type AdapterQueryResult = Result<(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>), String>;

lazy_static! {
    // 名称/描述黑名单：作为 is_visible_in_ncpa 的纵深防御层
    // 规则：nat/tor/virtual 加 \b 词边界，避免误伤 "Native/Intel NAT Offload/Toronto/Tornado" 等合法名
    //      中文补全：覆盖 "虚拟/伪/假/测试/模拟/隧道"，避免漏过滤中文命名的虚拟网卡
    //      保留 "本地连接"：用户特定业务规则（Win11 高级网络设置不可见，强制排除）
    // 注意：WLAN/以太网的具体可见性判断已移到 is_visible_in_ncpa（注册表检查），
    //      避免按名称误伤多物理网卡场景（如 2 块真实无线网卡可能都叫 "WLAN"）
    pub(crate) static ref BL_REGEX: Regex = Regex::new(r"(?i)hyper-v|\bvirtual\b|vmware|veth|docker|wsl|loopback|tunnel|isatap|6to4|teredo|bluetooth|vpn|hamachi|zerotier|tailscale|wireguard|vEthernet|HNS|\bnat\b|filter.?driver|packet.?driver|npcap|qos|packet.?scheduler|wfp|lightweight.?filter|kernel.?debug|clash|v2ray|xray|sing-box|shadowsocks|ss-local|hysteria|trojan|naiveproxy|mihomo|surge|quantumult|loon|stash|surfboard|netch|proxifier|privoxy|\btor\b|i2p|tun2socks|tap-|tun0|wg0|utun|clash\.tun|clash\.tap|meta\.tun|sing\.tun|cloudflare.?warp|warp|本地连接|虚拟|伪|假|测试|模拟|隧道").expect("BL_REGEX compilation failed");
}

/// 适配器状态四分类（基于 IF_OPER_STATUS 枚举 + IP 是否为空）
/// - Disabled: 已禁用（OperStatus Down 或 NotPresent，管理员禁用或硬件缺失）
/// - Disconnected: 未连接（OperStatus LowerLayerDown 或 Dormant，线缆未插或等待外部事件）
/// - EnabledNoIp: 未禁用无IP（OperStatus Up 但无有效 IP，含 169.254 APIPA 清空后）
/// - Connected: 已连接（OperStatus Up 且有有效 IP）
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AdapterStatus {
    Disabled,
    Disconnected,
    EnabledNoIp,
    Connected,
}

impl AdapterStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            AdapterStatus::Disabled => "已禁用",
            AdapterStatus::Disconnected => "未连接",
            AdapterStatus::EnabledNoIp => "未禁用无IP",
            AdapterStatus::Connected => "已连接",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Adapter {
    pub name: String,
    pub ip: String,
    pub wireless: bool,
    pub guid: String,
    pub mac: String,
    pub if_index: u32,
    pub status: AdapterStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterDetail {
    pub name: String,
    pub ip: String,
    pub wireless: bool,
    pub subnet_mask: String,
    pub gateway: String,
    pub dhcp_server: String,
    pub mac: String,
    pub if_index: u32,
    pub status: AdapterStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisabledAdapter {
    pub name: String,
    pub status: String,
    pub description: String,
}

/// 创建子进程时附加 Windows 隐藏窗口标志，避免弹出控制台窗口。
pub fn new_command(program: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);
    cmd
}

pub fn is_blacklisted(name: &str) -> bool {
    BL_REGEX.is_match(name)
}

/// 将 IPv4 前缀长度转换为点分十进制掩码。
pub(crate) fn prefix_len_to_mask(len: u32) -> String {
    if len > 32 { return String::new(); }
    let mask: u32 = if len == 0 { 0 } else { !0u32 << (32 - len) };
    format!(
        "{}.{}.{}.{}",
        (mask >> 24) & 0xFF,
        (mask >> 16) & 0xFF,
        (mask >> 8) & 0xFF,
        mask & 0xFF,
    )
}

/// 平台分发：查询系统适配器列表（Windows 调用 Win32 API，其他平台返回空）。
pub(crate) fn query_adapters_addresses() -> AdapterQueryResult {
    #[cfg(target_os = "windows")]
    {
        self::windows::query_adapters_addresses()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok((vec![], vec![], vec![]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === is_blacklisted 词边界回归测试 ===
    // 防止 nat/tor/virtual 误伤 "Native/National/Toronto" 等合法物理网卡名
    #[test]
    fn blacklist_word_boundary_does_not_match_legit_nics() {
        assert!(!is_blacklisted("Intel Native Ethernet Adapter"));
        assert!(!is_blacklisted("National Semiconductor NIC"));
        assert!(!is_blacklisted("NATO Secure Adapter"));
        assert!(!is_blacklisted("Native Ethernet"));
        assert!(!is_blacklisted("Toronto Office Ethernet"));
        assert!(!is_blacklisted("Tornado Net Bridge"));
        assert!(!is_blacklisted("Vector Network"));
        assert!(!is_blacklisted("Mentor Lab Network"));
        assert!(!is_blacklisted("Tutorial Lab NIC"));
    }

    #[test]
    fn blacklist_word_boundary_still_matches_known_virtuals() {
        assert!(is_blacklisted("NAT Network"));
        assert!(is_blacklisted("nat"));
        assert!(is_blacklisted("My NAT Adapter"));
        assert!(is_blacklisted("tor"));
        assert!(is_blacklisted("Tor Service"));
        assert!(is_blacklisted("Virtual Ethernet"));
        assert!(is_blacklisted("Hyper-V Virtual NIC"));
    }

    #[test]
    fn blacklist_chinese_virtual_keywords() {
        assert!(is_blacklisted("虚拟网卡"));
        assert!(is_blacklisted("伪 VPN"));
        assert!(is_blacklisted("假测试网卡"));
        assert!(is_blacklisted("测试虚拟连接"));
        assert!(is_blacklisted("模拟网络"));
        assert!(is_blacklisted("IPv6 隧道"));
        assert!(is_blacklisted("本地连接"));
        assert!(is_blacklisted("本地连接 2"));
    }

    #[test]
    fn blacklist_does_not_match_legit_chinese_nics() {
        assert!(!is_blacklisted("以太网"));
        assert!(!is_blacklisted("WLAN"));
        assert!(!is_blacklisted("校园网认证"));
    }
}
