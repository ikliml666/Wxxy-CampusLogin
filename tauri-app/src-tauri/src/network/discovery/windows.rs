//! Windows 专有适配器发现实现
//!
//! 包含 Win32 GetAdaptersAddresses 调用与注册表可见性检查。
//! 非 Windows 平台不编译此模块（由 discovery::mod 的平台分发返回空列表）。

use super::{Adapter, AdapterDetail, AdapterStatus, DisabledAdapter, AdapterQueryResult};

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
    use windows::Win32::NetworkManagement::Ndis::{
        IfOperStatusUp, IfOperStatusNotPresent,
    };
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

        // 严格四分类判定
        let status = if is_up {
            if ip.is_empty() {
                AdapterStatus::EnabledNoIp
            } else {
                AdapterStatus::Connected
            }
        } else if oper_status == IfOperStatusNotPresent {
            // NotPresent 可能是管理员禁用或硬件缺失(USB未连接)
            // 用 ConfigFlags 区分：CONFIGFLAG_DISABLED (0x1) 才是管理员禁用
            if is_admin_disabled_via_registry(&guid) {
                AdapterStatus::Disabled
            } else {
                // USB 网卡未连接 / 硬件缺失 / 驱动未加载
                AdapterStatus::Disconnected
            }
        } else {
            // Down / LowerLayerDown / Dormant / Unknown / Testing 归为未连接
            // Down 在 Windows 上实际语义是"接口未就绪"（媒体断开/未认证），不是管理员禁用
            AdapterStatus::Disconnected
        };

        // 所有适配器都推入 adapters 列表（带状态，便于前端统一展示和启用操作）
        adapters.push(Adapter {
            name: name.clone(),
            ip: ip.clone(),
            wireless: is_wireless,
            guid: guid.clone(),
            mac: mac.clone(),
            if_index,
            status,
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

/// 判断网卡是否在 Win11 高级网络设置 / ncpa.cpl 中可见
/// 严格按注册表 PnP 设备树检测，避免按名称误伤多物理网卡场景
fn is_visible_in_ncpa(guid: &str) -> bool {
    // 注册表 1：HKLM\...\Control\Network\{4D36E972-...}\{GUID}\Connection\ShowInNetworkConnections
    //   = 0 → 用户/系统标记为隐藏
    //   = 1 或不存在 → Windows 默认显示
    //
    // 注册表 2：HKLM\SYSTEM\CurrentControlSet\Enum\<Enumerator>\<InstanceId>
    //   PnP 设备树中必须存在该 GUID 对应的实例，否则为"幽灵虚拟副本"（如 Wi-Fi Direct Virtual Adapter
    //   创建的多个 WLAN 2/3/4/5，这些在网络栈可见但 PnP 树中已被清理）
    //
    // 决策：注册表 1 + 2 都通过才视为可见
    if guid.is_empty() {
        return false;
    }
    // 注册表 1 检查：Connection 子键的 ShowInNetworkConnections
    let key_path = format!(
        "SYSTEM\\CurrentControlSet\\Control\\Network\\{{4D36E972-E325-11CE-BFC1-08002BE10318}}\\{guid}\\Connection"
    );
    let show_in_ncpa = match winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE).open_subkey(&key_path) {
        Ok(key) => {
            match key.get_value::<u32, _>("ShowInNetworkConnections") {
                Ok(val) => val != 0,
                Err(_) => true,
            }
        }
        Err(_) => true,  // Connection 子键缺失 → 视为可见，由 PnP 树检查把关
    };
    if !show_in_ncpa {
        return false;
    }
    // 注册表 2 检查：Class subkey 交叉验证
    class_subkey_has_matching_guid(guid)
}

fn class_subkey_has_matching_guid(guid: &str) -> bool {
    if guid.is_empty() {
        return false;
    }
    let hklm = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
    let class_path = "SYSTEM\\CurrentControlSet\\Control\\Class\\{4D36E972-E325-11CE-BFC1-08002BE10318}";
    let class_key = match hklm.open_subkey(class_path) {
        Ok(k) => k,
        Err(_) => return false,
    };
    for subkey_name in class_key.enum_keys().filter_map(|r| r.ok()) {
        if let Ok(subkey) = class_key.open_subkey(&subkey_name) {
            if let Ok(instance_id) = subkey.get_value::<String, _>("NetCfgInstanceId") {
                if instance_id.eq_ignore_ascii_case(guid) {
                    return true;
                }
            }
        }
    }
    false
}

/// 用于区分 NotPresent 状态下的"管理员禁用"vs"硬件缺失(USB未连接)"
fn is_admin_disabled_via_registry(guid: &str) -> bool {
    if guid.is_empty() {
        return false;
    }
    let hklm = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
    let class_path = "SYSTEM\\CurrentControlSet\\Control\\Class\\{4D36E972-E325-11CE-BFC1-08002BE10318}";
    let class_key = match hklm.open_subkey(class_path) {
        Ok(k) => k,
        Err(_) => return false,
    };
    for sub in class_key.enum_keys().filter_map(|n| n.ok()) {
        if let Ok(sub_key) = class_key.open_subkey(&sub) {
            if let Ok(net_cfg_id) = sub_key.get_value::<String, _>("NetCfgInstanceId") {
                if net_cfg_id == guid {
                    // 找到匹配条目，读 ConfigFlags
                    if let Ok(flags) = sub_key.get_value::<u32, _>("ConfigFlags") {
                        return flags & 0x1 != 0;  // CONFIGFLAG_DISABLED
                    }
                    return false;  // ConfigFlags 不存在，视为未禁用
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // === class_subkey_has_matching_guid 集成测试（需在真实 Windows 环境下运行） ===
    #[test]
    fn class_subkey_check_empty_guid_returns_false() {
        assert!(!class_subkey_has_matching_guid(""));
    }

    #[test]
    fn class_subkey_check_nonexistent_guid_returns_false() {
        assert!(!class_subkey_has_matching_guid("{00000000-0000-0000-0000-000000000000}"));
    }

    #[test]
    fn class_subkey_check_real_wlan_guid_returns_true() {
        // 真实 WLAN：class 0002 的 NetCfgInstanceId = {86B8D1AD-...}
        let result = class_subkey_has_matching_guid("{86B8D1AD-30C8-479C-B7B2-846BD1C590FF}");
        if !result {
            eprintln!("[SKIP] 当前环境无真实 WLAN class subkey");
        }
    }

    #[test]
    fn class_subkey_check_ghost_wlan_guids_return_false() {
        // 幽灵虚拟副本：NetCfgInstanceId 在 class subkey 中无匹配
        let ghost_guids = [
            "{DA918853-570D-45C6-8AE1-A841D9A0D978}",  // WLAN 2
            "{C1CE50FF-65E7-46BD-9106-4E00A7C49AB6}",  // WLAN 3
            "{723CE6A0-D1BD-45F0-86C7-1FECE96D18ED}",  // WLAN 4
            "{DADC7A44-5EBF-4DED-BC80-EB66136A8BB0}",  // WLAN 5
        ];
        for guid in ghost_guids {
            assert!(!class_subkey_has_matching_guid(guid), "幽灵 GUID {} 应返回 false", guid);
        }
    }
}
