//! Windows 专有适配器发现实现
//!
//! 包含 Win32 GetAdaptersAddresses 调用与注册表可见性检查。
//! 非 Windows 平台不编译此模块（由 discovery::mod 的平台分发返回空列表）。

use super::{Adapter, AdapterDetail, AdapterStatus, DisabledAdapter, AdapterQueryResult};

// 注册表可见性 / 禁用状态检查已迁移到 `super::registry`
use super::registry::{is_visible_in_ncpa, is_admin_disabled_via_registry};

/// 调用 Win32 GetAdaptersAddresses 获取适配器列表。
pub fn query_adapters_addresses() -> AdapterQueryResult {
    use windows::Win32::NetworkManagement::IpHelper::*;
    use windows::Win32::Networking::WinSock::*;

    const GAA_FLAGS: GET_ADAPTERS_ADDRESSES_FLAGS = GET_ADAPTERS_ADDRESSES_FLAGS(0x0080 | 0x0100);
    const IF_TYPE_ETHERNET_CSMACD: u32 = 6;
    const IF_TYPE_IEEE80211: u32 = 71;

    let mut size: u32 = 0;
    unsafe {
        GetAdaptersAddresses(AF_INET.0 as u32, GAA_FLAGS, None, None, &mut size);
    };

    if size == 0 {
        return Ok((vec![], vec![], vec![]));
    }

    let max_retries = 3;
    for attempt in 0..max_retries {
        let buffer_size = if attempt == 0 { size as usize } else { (size as usize) + 4096 };
        let mut buffer = vec![0u8; buffer_size];
        let ptr = buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;
        let mut actual_size = buffer_size as u32;

        let result = unsafe {
            GetAdaptersAddresses(AF_INET.0 as u32, GAA_FLAGS, None, Some(ptr), &mut actual_size)
        };

        if result == 0 {
            return parse_adapter_addresses(ptr, IF_TYPE_ETHERNET_CSMACD, IF_TYPE_IEEE80211);
        }

        if result == 111 || actual_size as usize > buffer_size {
            size = actual_size;
            if attempt < max_retries - 1 {
                continue;
            }
            return Err(format!("GetAdaptersAddresses buffer too small after {max_retries} retries"));
        }

        if attempt < max_retries - 1 {
            unsafe {
                GetAdaptersAddresses(AF_INET.0 as u32, GAA_FLAGS, None, None, &mut size);
            }
            continue;
        }

        return Err(format!("GetAdaptersAddresses failed: {result}"));
    }

    Ok((vec![], vec![], vec![]))
}

fn parse_adapter_addresses(
    ptr: *mut windows::Win32::NetworkManagement::IpHelper::IP_ADAPTER_ADDRESSES_LH,
    if_type_ethernet: u32,
    if_type_wireless: u32,
) -> AdapterQueryResult {
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows::Win32::Networking::WinSock::*;

    let mut adapters = Vec::new();
    let mut details = Vec::new();
    let mut disabled = Vec::new();

    let mut current = ptr;
    while !current.is_null() {
        let addr = unsafe { &*current };

        let name = unsafe { read_pwstr(addr.FriendlyName) };

        let guid_raw = unsafe {
            if addr.AdapterName.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(addr.AdapterName.0 as *const i8)
                    .to_string_lossy()
                    .into_owned()
            }
        };
        let guid = if guid_raw.starts_with('{') {
            guid_raw
        } else if !guid_raw.is_empty() {
            format!("{{{guid_raw}}}")
        } else {
            guid_raw
        };

        let if_type = addr.IfType;
        if if_type != if_type_ethernet && if_type != if_type_wireless {
            current = addr.Next;
            continue;
        }

        if addr.PhysicalAddressLength == 0 {
            current = addr.Next;
            continue;
        }

        let is_up = addr.OperStatus == IfOperStatusUp;
        // NotPresent（已禁用）适配器不跳过，进入 else 分支加入 disabled 列表

        if !is_visible_in_ncpa(&guid) {
            current = addr.Next;
            continue;
        }

        let description = unsafe { read_pwstr(addr.Description) };

        if super::is_blacklisted(&name) || super::is_blacklisted(&description) {
            current = addr.Next;
            continue;
        }

        let is_wireless = if_type == if_type_wireless;
        let if_index = unsafe { addr.Anonymous1.Anonymous.IfIndex };
        // 连接速度（bit/s）：IP_ADAPTER_ADDRESSES 自带字段，无需额外调用。
        // 未连接时 Windows 返回 u64::MAX（内部 -1 哨兵），归 0 表示未知（原样透传
        // 会被前端换算成 18446744073.7 Gbps）
        let link_speed = match addr.ReceiveLinkSpeed {
            u64::MAX => 0,
            v => v,
        };

        let mac = if addr.PhysicalAddressLength >= 6 {
            let bytes = unsafe { std::slice::from_raw_parts(addr.PhysicalAddress.as_ptr(), 6) };
            format!("{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}", bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5])
        } else {
            String::new()
        };

        let oper_status = addr.OperStatus;
        // 计算 IP / prefix_len（仅 is_up 时尝试拿，否则保持空）
        let mut ip = String::new();
        let mut prefix_len: u8 = 0;
        if is_up {
            let mut ua = addr.FirstUnicastAddress;
            while !ua.is_null() {
                let u = unsafe { &*ua };
                if u.Address.lpSockaddr.is_null() {
                    ua = unsafe { (*ua).Next };
                    continue;
                }
                let sa = unsafe { &*u.Address.lpSockaddr };
                if sa.sa_family == AF_INET {
                    let sin = unsafe { &*(u.Address.lpSockaddr as *const SOCKADDR_IN) };
                    ip = unsafe { ipv4_from_in_addr(sin.sin_addr) };
                    prefix_len = u.OnLinkPrefixLength;
                    break;
                }
                ua = u.Next;
            }
            // 169.254 APIPA 视为无 IP（DHCP 失败的自配地址）
            if ip.starts_with("169.254.") {
                ip.clear();
            }
        }

        // 网关 / DHCP 服务器（仅 is_up 时查询，否则保持空）
        let mut gateway = String::new();
        let mut dhcp_server = String::new();
        if is_up {
            let mut ga = addr.FirstGatewayAddress;
            while !ga.is_null() {
                let g = unsafe { &*ga };
                if g.Address.lpSockaddr.is_null() {
                    ga = unsafe { (*ga).Next };
                    continue;
                }
                let sa = unsafe { &*g.Address.lpSockaddr };
                if sa.sa_family == AF_INET {
                    let sin = unsafe { &*(g.Address.lpSockaddr as *const SOCKADDR_IN) };
                    gateway = unsafe { ipv4_from_in_addr(sin.sin_addr) };
                    break;
                }
                ga = g.Next;
            }

            let dhcp_sa = addr.Dhcpv4Server;
            if !dhcp_sa.lpSockaddr.is_null() {
                let sa = unsafe { &*dhcp_sa.lpSockaddr };
                if sa.sa_family == AF_INET {
                    let sin = unsafe { &*(dhcp_sa.lpSockaddr as *const SOCKADDR_IN) };
                    dhcp_server = unsafe { ipv4_from_in_addr(sin.sin_addr) };
                }
            }
        }

        // 严格四分类判定（决策表见 classify_adapter_status 单测）。
        // 非 Up 时查 GetIfEntry2 的 AdminStatus：微软文档化的管理性禁用标志（禁用→Down，
        // 拔线/媒体断开→保持 Up）。历史缺陷：旧实现只认「OperStatus==NotPresent 且
        // ConfigFlags DISABLED」，但文档明文禁用后 OperStatus「Down 或 NotPresent 皆可能」，
        // 禁用报 Down 的网卡被归为「未连接」，自动启用永远不触发。
        let admin_down = if is_up { None } else { is_admin_status_down(if_index) };
        let registry_disabled = if is_up { false } else { is_admin_disabled_via_registry(&guid) };
        let status = classify_adapter_status(is_up, !ip.is_empty(), oper_status, admin_down, registry_disabled);

        // 所有适配器都推入 adapters 列表（带状态，便于前端统一展示和启用操作）
        adapters.push(Adapter {
            name: name.clone(),
            ip: ip.clone(),
            wireless: is_wireless,
            guid: guid.clone(),
            mac: mac.clone(),
            if_index,
            status,
            link_speed,
        });

        // Connected 和 EnabledNoIp 状态推入 details（EnabledNoIp 保留 dhcp_server 供诊断）
        // 仅 Disabled 状态推入 disabled（保留 DisabledAdapter 兼容旧 API）
        match status {
            AdapterStatus::Connected => {
                details.push(AdapterDetail {
                    name,
                    ip,
                    wireless: is_wireless,
                    subnet_mask: super::prefix_len_to_mask(prefix_len as u32),
                    gateway,
                    dhcp_server,
                    mac,
                    if_index,
                    status,
                    link_speed,
                });
            }
            AdapterStatus::EnabledNoIp => {
                details.push(AdapterDetail {
                    name,
                    ip: String::new(),
                    wireless: is_wireless,
                    subnet_mask: String::new(),
                    gateway,
                    dhcp_server,
                    mac,
                    if_index,
                    status,
                    link_speed,
                });
            }
            AdapterStatus::Disabled => {
                disabled.push(DisabledAdapter {
                    name,
                    status: status.as_str().to_string(),
                    description,
                });
            }
            _ => {}
        }

        current = addr.Next;
    }

    Ok((adapters, details, disabled))
}

unsafe fn read_pwstr(ptr: windows::core::PWSTR) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let pcwstr = windows::core::PCWSTR(ptr.0 as *const u16);
    pcwstr.to_string().unwrap_or_else(|_| {
        let mut len = 0;
        let max_len = 4096;
        while len < max_len && *ptr.0.add(len) != 0 {
            len += 1;
        }
        if len == max_len {
            crate::log_warn!("network", "read_pwstr: 适配器名称超过{}个UTF-16单元，跳过该适配器", max_len);
            return String::new();
        }
        if len == 0 {
            return String::new();
        }
        let slice = std::slice::from_raw_parts(ptr.0, len);
        String::from_utf16_lossy(slice)
    })
}

unsafe fn ipv4_from_in_addr(addr: windows::Win32::Networking::WinSock::IN_ADDR) -> String {
    std::net::Ipv4Addr::from(addr).to_string()
}

/// 查询接口 AdminStatus 是否为 Down（管理性禁用的文档化标志）。
///
/// 幽灵虚拟副本（接口不存在）或查询失败返回 `None`，调用方回退注册表 ConfigFlags 判定。
/// `GetIfEntry2` 在 `InterfaceLuid` 为零时按 `InterfaceIndex` 查找。
fn is_admin_status_down(if_index: u32) -> Option<bool> {
    use windows::Win32::NetworkManagement::IpHelper::{GetIfEntry2, MIB_IF_ROW2};
    use windows::Win32::NetworkManagement::Ndis::NET_IF_ADMIN_STATUS_DOWN;

    let mut row = MIB_IF_ROW2::default();
    row.InterfaceIndex = if_index;
    let hr = unsafe { GetIfEntry2(&mut row) };
    if hr != windows::Win32::Foundation::WIN32_ERROR(0) {
        return None;
    }
    Some(row.AdminStatus == NET_IF_ADMIN_STATUS_DOWN)
}

/// 四分类决策（纯函数，便于单测）。
///
/// `admin_down`：`GetIfEntry2` 查得的 `AdminStatus == Down`（`None` = 接口不存在/查询失败）。
/// AdminStatus 是微软文档化的管理性禁用标志：禁用 → Down；拔线/媒体断开 → 保持 Up。
/// `registry_disabled`：注册表 `ConfigFlags` 的 `CONFIGFLAG_DISABLED` 位（未文档化行为，
/// 仅作 NotPresent 且查不到 AdminStatus 时的回退）。
///
/// 历史缺陷：旧实现只在 `OperStatus == NotPresent` 时查 ConfigFlags 判禁用，
/// 但文档明文禁用后 OperStatus「Down 或 NotPresent 两者皆可能」——
/// 禁用报 Down 的网卡被归为「未连接」，自动启用永远不触发。
fn classify_adapter_status(
    is_up: bool,
    has_ip: bool,
    oper_status: windows::Win32::NetworkManagement::Ndis::IF_OPER_STATUS,
    admin_down: Option<bool>,
    registry_disabled: bool,
) -> AdapterStatus {
    if is_up {
        return if has_ip { AdapterStatus::Connected } else { AdapterStatus::EnabledNoIp };
    }
    if admin_down == Some(true) {
        return AdapterStatus::Disabled;
    }
    if oper_status == windows::Win32::NetworkManagement::Ndis::IfOperStatusNotPresent && registry_disabled {
        return AdapterStatus::Disabled;
    }
    AdapterStatus::Disconnected
}

#[cfg(test)]
mod tests {
    use super::classify_adapter_status;
    use super::super::AdapterStatus;
    use windows::Win32::NetworkManagement::Ndis::{IfOperStatusDown, IfOperStatusNotPresent};

    #[test]
    fn up_with_ip_is_connected() {
        assert_eq!(
            classify_adapter_status(true, true, IfOperStatusDown, None, false),
            AdapterStatus::Connected
        );
    }

    #[test]
    fn up_without_ip_is_enabled_no_ip() {
        assert_eq!(
            classify_adapter_status(true, false, IfOperStatusDown, None, false),
            AdapterStatus::EnabledNoIp
        );
    }

    // 核心修复：禁用后 OperStatus 报 Down（文档明文 Down/NotPresent 皆可能），
    // AdminStatus=Down 必须判为禁用——旧实现漏判为「未连接」
    #[test]
    fn down_with_admin_down_is_disabled() {
        assert_eq!(
            classify_adapter_status(false, false, IfOperStatusDown, Some(true), false),
            AdapterStatus::Disabled
        );
    }

    #[test]
    fn down_with_admin_up_is_disconnected() {
        // 拔线/媒体断开：AdminStatus 保持 Up
        assert_eq!(
            classify_adapter_status(false, false, IfOperStatusDown, Some(false), false),
            AdapterStatus::Disconnected
        );
    }

    #[test]
    fn down_with_admin_query_failed_is_disconnected() {
        // GetIfEntry2 查询失败：回退旧行为，不误判
        assert_eq!(
            classify_adapter_status(false, false, IfOperStatusDown, None, false),
            AdapterStatus::Disconnected
        );
    }

    #[test]
    fn not_present_with_admin_down_is_disabled() {
        // NotPresent + AdminStatus=Down（ConfigFlags 可能被开机驱动重装洗掉）
        assert_eq!(
            classify_adapter_status(false, false, IfOperStatusNotPresent, Some(true), false),
            AdapterStatus::Disabled
        );
    }

    #[test]
    fn not_present_with_registry_flag_is_disabled() {
        // 旧行为保持：NotPresent + ConfigFlags DISABLED 位
        assert_eq!(
            classify_adapter_status(false, false, IfOperStatusNotPresent, None, true),
            AdapterStatus::Disabled
        );
    }

    #[test]
    fn not_present_without_any_flag_is_disconnected() {
        // 幽灵虚拟副本（如 WLAN 2/5）：NotPresent 但非禁用
        assert_eq!(
            classify_adapter_status(false, false, IfOperStatusNotPresent, None, false),
            AdapterStatus::Disconnected
        );
    }
}
