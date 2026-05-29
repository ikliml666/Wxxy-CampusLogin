use regex::Regex;
use serde::Serialize;
use lazy_static::lazy_static;
use parking_lot::Mutex;
use std::time::Instant;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

lazy_static! {
    static ref BL_REGEX: Regex = Regex::new(r"(?i)hyper-v|virtual|vmware|veth|docker|wsl|loopback|tunnel|isatap|6to4|teredo|bluetooth|vpn|hamachi|zerotier|tailscale|wireguard|vEthernet|HNS|nat|filter.?driver|packet.?driver|npcap|qos|packet.?scheduler|wfp|lightweight.?filter|kernel.?debug|clash|v2ray|xray|sing-box|shadowsocks|ss-local|hysteria|trojan|naiveproxy|mihomo|surge|quantumult|loon|stash|surfboard|netch|proxifier|privoxy|tor|i2p|tun2socks|tap-|tun0|wg0|utun|clash\.tun|clash\.tap|meta\.tun|sing\.tun|cloudflare.?warp|warp|本地连接").expect("BL_REGEX compilation failed");
    static ref ADAPTER_CACHE: Mutex<Option<(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>, Instant)>> = Mutex::new(None);
}

const ADAPTER_CACHE_TTL_SECS: u64 = 5;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Adapter {
    pub name: String,
    pub ip: String,
    pub wireless: bool,
    pub guid: String,
    pub mac: String,
    pub if_index: u32,
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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisabledAdapter {
    pub name: String,
    pub status: String,
    pub description: String,
}

pub fn new_command(program: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);
    cmd
}

pub fn is_blacklisted(name: &str) -> bool {
    BL_REGEX.is_match(name)
}

fn is_virtual_description(desc: &str) -> bool {
    BL_REGEX.is_match(desc)
}

#[cfg(target_os = "windows")]
fn is_visible_in_ncpa(guid: &str) -> bool {
    if guid.is_empty() {
        return true;
    }
    let key_path = format!(
        "SYSTEM\\CurrentControlSet\\Control\\Network\\{{4D36E972-E325-11CE-BFC1-08002BE10318}}\\{}\\Connection",
        guid
    );
    match winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE).open_subkey(key_path) {
        Ok(key) => {
            match key.get_value::<u32, _>("ShowInNetworkConnections") {
                Ok(val) => val != 0,
                Err(_) => true,
            }
        }
        Err(_) => true,
    }
}

#[cfg(target_os = "windows")]
fn has_media_sub_type(guid: &str) -> bool {
    if guid.is_empty() {
        return true;
    }
    let key_path = format!(
        "SYSTEM\\CurrentControlSet\\Control\\Network\\{{4D36E972-E325-11CE-BFC1-08002BE10318}}\\{}\\Connection",
        guid
    );
    match winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE).open_subkey(key_path) {
        Ok(key) => {
            match key.get_value::<u32, _>("MediaSubType") {
                Ok(_) => true,
                Err(_) => false,
            }
        }
        Err(_) => true,
    }
}

fn prefix_len_to_mask(len: u32) -> String {
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

#[cfg(target_os = "windows")]
fn query_adapters_addresses() -> Result<(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>), String> {
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
            return Err(format!("GetAdaptersAddresses buffer too small after {} retries", max_retries));
        }

        if attempt < max_retries - 1 {
            unsafe {
                GetAdaptersAddresses(AF_INET.0 as u32, GAA_FLAGS, None, None, &mut size);
            }
            continue;
        }

        return Err(format!("GetAdaptersAddresses failed: {}", result));
    }

    Ok((vec![], vec![], vec![]))
}

#[cfg(target_os = "windows")]
fn parse_adapter_addresses(
    ptr: *mut windows::Win32::NetworkManagement::IpHelper::IP_ADAPTER_ADDRESSES_LH,
    if_type_ethernet: u32,
    if_type_wireless: u32,
) -> Result<(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>), String> {
    use windows::Win32::NetworkManagement::Ndis::{IfOperStatusUp, IfOperStatusNotPresent};
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
            format!("{{{}}}", guid_raw)
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
        if !is_up && !has_media_sub_type(&guid) {
            current = addr.Next;
            continue;
        }

        if !is_visible_in_ncpa(&guid) {
            current = addr.Next;
            continue;
        }

        let description = unsafe { read_pwstr(addr.Description) };

        if is_blacklisted(&name) || is_virtual_description(&description) {
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

        if is_up {
            let mut ip = String::new();
            let mut prefix_len: u8 = 0;

            let mut ua = addr.FirstUnicastAddress;
            while !ua.is_null() {
                let u = unsafe { &*ua };
                let sa = unsafe { &*u.Address.lpSockaddr };
                if sa.sa_family == AF_INET {
                    let sin = unsafe { &*(u.Address.lpSockaddr as *const SOCKADDR_IN) };
                    ip = unsafe { ipv4_from_in_addr(sin.sin_addr) };
                    prefix_len = u.OnLinkPrefixLength;
                    break;
                }
                ua = u.Next;
            }

            if ip.starts_with("169.254.") {
                ip.clear();
            }

            let mut gateway = String::new();
            let mut ga = addr.FirstGatewayAddress;
            while !ga.is_null() {
                let g = unsafe { &*ga };
                let sa = unsafe { &*g.Address.lpSockaddr };
                if sa.sa_family == AF_INET {
                    let sin = unsafe { &*(g.Address.lpSockaddr as *const SOCKADDR_IN) };
                    gateway = unsafe { ipv4_from_in_addr(sin.sin_addr) };
                    break;
                }
                ga = g.Next;
            }

            let mut dhcp_server = String::new();
            let dhcp_sa = addr.Dhcpv4Server;
            if !dhcp_sa.lpSockaddr.is_null() {
                let sa = unsafe { &*dhcp_sa.lpSockaddr };
                if sa.sa_family == AF_INET {
                    let sin = unsafe { &*(dhcp_sa.lpSockaddr as *const SOCKADDR_IN) };
                    dhcp_server = unsafe { ipv4_from_in_addr(sin.sin_addr) };
                }
            }

            adapters.push(Adapter { name: name.clone(), ip: ip.clone(), wireless: is_wireless, guid: guid.clone(), mac: mac.clone(), if_index });
            if !ip.is_empty() {
                details.push(AdapterDetail {
                    name,
                    ip,
                    wireless: is_wireless,
                    subnet_mask: prefix_len_to_mask(prefix_len as u32),
                    gateway,
                    dhcp_server,
                    mac,
                    if_index,
                });
            }
        } else {
            let status = if addr.OperStatus == IfOperStatusNotPresent {
                "已禁用"
            } else {
                "未连接"
            };
            disabled.push(DisabledAdapter {
                name,
                status: status.to_string(),
                description,
            });
        }

        current = addr.Next;
    }

    Ok((adapters, details, disabled))
}

#[cfg(target_os = "windows")]
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

#[cfg(target_os = "windows")]
unsafe fn ipv4_from_in_addr(addr: windows::Win32::Networking::WinSock::IN_ADDR) -> String {
    std::net::Ipv4Addr::from(addr).to_string()
}

fn query_adapters_cached_inner() -> Result<(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>), String> {
    {
        let cache = ADAPTER_CACHE.lock();
        if let Some((adapters, details, disabled, ts)) = cache.as_ref() {
            if ts.elapsed().as_secs() < ADAPTER_CACHE_TTL_SECS {
                return Ok((adapters.clone(), details.clone(), disabled.clone()));
            }
        }
    }
    let result = query_adapters_addresses()?;
    {
        let mut cache = ADAPTER_CACHE.lock();
        *cache = Some((result.0.clone(), result.1.clone(), result.2.clone(), Instant::now()));
    }
    Ok(result)
}

pub fn get_all_adapters_cached() -> Result<(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>), String> {
    query_adapters_cached_inner()
}

pub fn get_adapters_cached() -> Result<Vec<Adapter>, String> {
    let (adapters, _, _) = query_adapters_cached_inner()?;
    Ok(adapters)
}

pub fn get_disabled_adapters_cached() -> Result<Vec<DisabledAdapter>, String> {
    let (_, _, disabled) = query_adapters_cached_inner()?;
    Ok(disabled)
}

pub fn get_adapters_force() -> Result<Vec<Adapter>, String> {
    ADAPTER_CACHE.lock().take();
    get_adapters_cached()
}

pub fn get_disabled_adapters_force() -> Result<Vec<DisabledAdapter>, String> {
    ADAPTER_CACHE.lock().take();
    get_disabled_adapters_cached()
}

pub fn validate_adapter_name(name: &str) -> Result<(), String> {
    if name.is_empty() { return Err("适配器名称不能为空".to_string()); }
    if name.len() > 128 { return Err("适配器名称过长".to_string()); }
    const FORBIDDEN: &[char] = &['&', '|', ';', '`', '$', '(', ')', '<', '>', '"', '\'', '\n', '\r', '\0'];
    if name.chars().any(|c| FORBIDDEN.contains(&c)) { return Err("适配器名称包含非法字符".to_string()); }
    Ok(())
}

pub fn enable_adapter(adapter_name: &str) -> Result<(), String> {
    validate_adapter_name(adapter_name)?;
    let output = new_command("netsh")
        .args(["interface", "set", "interface", adapter_name, "enable"])
        .output()
        .map_err(|e| format!("启用适配器失败: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("启用适配器失败: {}", stderr.trim()));
    }
    Ok(())
}

pub fn get_adapter_details_cached() -> Result<Vec<AdapterDetail>, String> {
    let (_, details, _) = query_adapters_cached_inner()?;
    Ok(details)
}

pub fn resolve_adapter_names(adapters: &[Adapter], config: &crate::config::Config) -> (String, String) {
    let adapter1 = if config.adapter1.is_empty() || config.adapter1 == "自动检测" {
        adapters.iter()
            .find(|a| !a.wireless && !a.ip.is_empty())
            .or_else(|| adapters.iter().find(|a| !a.ip.is_empty()))
            .or_else(|| adapters.first())
            .map(|a| a.name.clone())
            .unwrap_or_default()
    } else {
        config.adapter1.clone()
    };

    let adapter2 = if config.dual_adapter {
        if config.adapter2.is_empty() || config.adapter2 == "自动检测" {
            adapters.iter()
                .find(|a| a.name != adapter1 && !a.wireless && !a.ip.is_empty())
                .or_else(|| adapters.iter().find(|a| a.name != adapter1 && !a.ip.is_empty()))
                .or_else(|| adapters.iter().find(|a| a.name != adapter1))
                .map(|a| a.name.clone())
                .unwrap_or_default()
        } else {
            config.adapter2.clone()
        }
    } else {
        String::new()
    };

    (adapter1, adapter2)
}

pub fn select_adapter(adapters: &[Adapter], config: &crate::config::Config) -> (String, String) {
    if adapters.is_empty() { return (String::new(), String::new()); }

    if !config.adapter1.is_empty() && config.adapter1 != "自动检测" {
        if let Some(a) = adapters.iter().find(|a| a.name == config.adapter1 && !a.ip.is_empty()) {
            return (a.ip.clone(), a.name.clone());
        }
    }

    if let Some(wired) = adapters.iter().find(|a| !a.ip.is_empty() && !a.wireless) {
        return (wired.ip.clone(), wired.name.clone());
    }

    if let Some(with_ip) = adapters.iter().find(|a| !a.ip.is_empty()) {
        return (with_ip.ip.clone(), with_ip.name.clone());
    }

    (String::new(), String::new())
}

pub fn dhcp_renew(adapter_name: &str) -> Result<bool, String> {
    if adapter_name.is_empty() { return Err("适配器名称无效".to_string()); }
    if adapter_name.len() > 128 { return Err("适配器名称过长".to_string()); }
    let forbidden = ['&', '|', ';', '`', '$', '(', ')', '<', '>', '"', '\'', '\n', '\r', '\0'];
    if adapter_name.chars().any(|c| forbidden.contains(&c)) {
        return Err("适配器名称包含非法字符".to_string());
    }
    let output = new_command("ipconfig")
        .args(["/renew", adapter_name])
        .output()
        .map_err(|e| format!("DHCP续租失败: {}", e))?;
    Ok(output.status.success())
}

pub fn dhcp_release(adapter_name: &str) -> Result<bool, String> {
    if adapter_name.is_empty() { return Err("适配器名称无效".to_string()); }
    if adapter_name.len() > 128 { return Err("适配器名称过长".to_string()); }
    let forbidden = ['&', '|', ';', '`', '$', '(', ')', '<', '>', '"', '\'', '\n', '\r', '\0'];
    if adapter_name.chars().any(|c| forbidden.contains(&c)) {
        return Err("适配器名称包含非法字符".to_string());
    }
    let output = new_command("ipconfig")
        .args(["/release", adapter_name])
        .output()
        .map_err(|e| format!("DHCP释放失败: {}", e))?;
    Ok(output.status.success())
}

pub fn dhcp_renew_wired_only() -> Result<Vec<serde_json::Value>, String> {
    let adapters = get_adapters_cached()?;
    let wired: Vec<&Adapter> = adapters.iter().filter(|a| !a.wireless).collect();
    if wired.is_empty() { return Ok(vec![]); }

    let mut results = Vec::new();
    for adapter in wired {
        let success = dhcp_renew(&adapter.name).unwrap_or(false);
        results.push(serde_json::json!({
            "name": adapter.name,
            "success": success
        }));
    }
    Ok(results)
}

fn generate_random_mac() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    // 使用简单的线性同余生成器替代 _rdtsc，避免平台依赖
    let mut rng = seed;
    let mut next = || { rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); rng };
    let b1 = (next() & 0xFF) as u8;
    let b2 = (next() & 0xFF) as u8;
    let b3 = (next() & 0xFF) as u8;
    let b4 = (next() & 0xFF) as u8;
    let b5 = (next() & 0xFF) as u8;
    let b6 = (next() & 0xFF) as u8;
    let first = (b1 & 0xFC) | 0x02;
    format!("{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}", first, b2, b3, b4, b5, b6)
}

fn mac_with_dashes(mac: &str) -> String {
    mac.as_bytes()
        .chunks(2)
        .filter_map(|c| std::str::from_utf8(c).ok())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(target_os = "windows")]
fn is_access_denied(e: &std::io::Error) -> bool {
    e.raw_os_error() == Some(5)
}

#[cfg(target_os = "windows")]
pub fn set_mac_via_registry(adapter_guid: &str, mac_no_dash: &str) -> Result<(), String> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_ALL_ACCESS};
    let class_path = r"SYSTEM\CurrentControlSet\Control\Class\{4D36E972-E325-11CE-BFC1-08002BE10318}";
    let hklm = winreg::RegKey::predef(HKEY_LOCAL_MACHINE);
    let class_key = hklm.open_subkey_with_flags(class_path, KEY_ALL_ACCESS)
        .map_err(|e| {
            if is_access_denied(&e) {
                "修改MAC地址需要管理员权限，请以管理员身份运行应用".to_string()
            } else {
                format!("打开网卡注册表失败: {}", e)
            }
        })?;
    for subkey_name in class_key.enum_keys().filter_map(|r| r.ok()) {
        if let Ok(subkey) = class_key.open_subkey_with_flags(&subkey_name, KEY_ALL_ACCESS) {
            if let Ok(instance_id) = subkey.get_value::<String, _>("NetCfgInstanceId") {
                if instance_id.eq_ignore_ascii_case(adapter_guid) {
                    subkey.set_value("NetworkAddress", &mac_no_dash)
                        .map_err(|e| format!("写入NetworkAddress失败: {}", e))?;
                    return Ok(());
                }
            }
        }
    }
    Err("未找到适配器注册表项".to_string())
}

#[cfg(target_os = "windows")]
pub fn remove_mac_from_registry(adapter_guid: &str) -> Result<(), String> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_ALL_ACCESS};
    let class_path = r"SYSTEM\CurrentControlSet\Control\Class\{4D36E972-E325-11CE-BFC1-08002BE10318}";
    let hklm = winreg::RegKey::predef(HKEY_LOCAL_MACHINE);
    let class_key = hklm.open_subkey_with_flags(class_path, KEY_ALL_ACCESS)
        .map_err(|e| {
            if is_access_denied(&e) {
                "清理MAC地址需要管理员权限".to_string()
            } else {
                format!("打开网卡注册表失败: {}", e)
            }
        })?;
    for subkey_name in class_key.enum_keys().filter_map(|r| r.ok()) {
        if let Ok(subkey) = class_key.open_subkey_with_flags(&subkey_name, KEY_ALL_ACCESS) {
            if let Ok(instance_id) = subkey.get_value::<String, _>("NetCfgInstanceId") {
                if instance_id.eq_ignore_ascii_case(adapter_guid) {
                    let _ = subkey.delete_value("NetworkAddress");
                    return Ok(());
                }
            }
        }
    }
    Err("未找到适配器注册表项".to_string())
}

pub fn netsh_disable(adapter_name: &str) -> bool {
    if validate_adapter_name(adapter_name).is_err() {
        return false;
    }
    // netsh语法要求 "name=适配器名" 作为单个参数传递，无法拆分为独立args
    new_command("netsh")
        .args(["interface", "set", "interface", &format!("name={}", adapter_name), "admin=disable"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn netsh_enable(adapter_name: &str) -> bool {
    if validate_adapter_name(adapter_name).is_err() {
        return false;
    }
    // netsh语法要求 "name=适配器名" 作为单个参数传递，无法拆分为独立args
    new_command("netsh")
        .args(["interface", "set", "interface", &format!("name={}", adapter_name), "admin=enable"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[allow(dead_code)]
fn is_access_denied_str(e: &str) -> bool {
    e.contains("管理员权限") || e.contains("Access is denied")
}

pub fn poll_ip_change(adapter_name: &str, old_ip: &str, timeout_ms: u64) -> Option<String> {
    let start = std::time::Instant::now();
    let interval = std::time::Duration::from_millis(300);
    let timeout = std::time::Duration::from_millis(timeout_ms);
    while start.elapsed() < timeout {
        std::thread::sleep(interval);
        if let Ok(adapters) = get_adapters_force() {
            if let Some(a) = adapters.iter().find(|a| a.name == adapter_name) {
                if !a.ip.is_empty() && a.ip != old_ip {
                    return Some(a.ip.clone());
                }
            }
        }
    }
    None
}

pub fn poll_adapter_has_ip(adapter_name: &str, timeout_ms: u64) -> bool {
    let start = std::time::Instant::now();
    let interval = std::time::Duration::from_millis(300);
    let timeout = std::time::Duration::from_millis(timeout_ms);
    while start.elapsed() < timeout {
        std::thread::sleep(interval);
        if let Ok(adapters) = get_adapters_force() {
            if let Some(a) = adapters.iter().find(|a| a.name == adapter_name) {
                if !a.ip.is_empty() {
                    return true;
                }
            }
        }
    }
    false
}

fn escape_ps_single_quote(s: &str) -> String {
    s.replace("'", "''")
}

fn try_elevated_mac_script(adapter_name: &str, _guid: &str, mac_no_dash: &str, old_ip: &str) -> (bool, Option<String>) {
    let mac_dashed = mac_with_dashes(mac_no_dash);
    let script = format!(
        "$name='{name}';$mac='{mac}';\
         Set-NetAdapter -Name $name -MacAddress $mac -Confirm:$false -ErrorAction Stop;\
         ipconfig /release $name;\
         Start-Sleep -Seconds 1;\
         ipconfig /renew $name",
        mac = mac_dashed, name = escape_ps_single_quote(adapter_name)
    );
    crate::log_info!("adapter", "尝试提权修改MAC(Set-NetAdapter): adapter={}, mac={}", adapter_name, mac_dashed);
    match crate::commands::network_cmd::run_elevated("powershell", &format!("-WindowStyle Hidden -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command \"{}\"", script)) {
        Ok(()) => {
            crate::log_info!("adapter", "提权脚本已启动，等待IP变更...");
            if let Some(changed_ip) = poll_ip_change(adapter_name, old_ip, 25_000) {
                crate::log_info!("adapter", "提权修改MAC成功: 新IP={}", changed_ip);
                (true, None)
            } else {
                crate::log_warn!("adapter", "提权修改MAC超时: 25秒内IP未变更");
                (false, Some("提权脚本已执行但IP未变更，可能网卡驱动不支持MAC伪装".to_string()))
            }
        }
        Err(e) => {
            crate::log_warn!("adapter", "提权执行MAC修改失败: {}", e);
            (false, Some(format!("提权失败: {}，请尝试以管理员身份运行应用", e)))
        }
    }
}

pub fn dhcp_release_renew_all(campus_gateway: &str) -> Result<Vec<serde_json::Value>, String> {
    let adapters = get_adapters_cached()?;
    if adapters.is_empty() { return Ok(vec![]); }

    let mut results = Vec::new();
    for adapter in &adapters {
        if !adapter.ip.is_empty() && !is_same_subnet_18(&adapter.ip, campus_gateway) {
            results.push(serde_json::json!({
                "name": adapter.name,
                "wireless": adapter.wireless,
                "ip": adapter.ip,
                "success": false,
                "skipped": true,
                "reason": "非校园网子网，跳过"
            }));
            continue;
        }

        let fake_mac = generate_random_mac();
        let mac_dashed = mac_with_dashes(&fake_mac);

        let (reg_ok, elevated_done, elevate_msg) = if crate::commands::network_cmd::is_admin() {
            match set_mac_via_registry(&adapter.guid, &fake_mac) {
                Ok(()) => {
                    crate::log_info!("adapter", "管理员直写注册表成功: guid={}", adapter.guid);
                    (true, false, None)
                }
                Err(e) => (false, false, Some(format!("MAC地址修改失败: {}", e))),
            }
        } else {
            crate::log_info!("adapter", "非管理员运行，跳过注册表直写，直接COM ShellExec提权: guid={}", adapter.guid);
            let ps_cmd = format!(
                "-WindowStyle Hidden -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command \"Set-NetAdapter -Name '{}' -MacAddress '{}' -Confirm:$false; ipconfig /release '{}'; Start-Sleep -Seconds 1; ipconfig /renew '{}'\"",
                escape_ps_single_quote(&adapter.name), mac_dashed, escape_ps_single_quote(&adapter.name), escape_ps_single_quote(&adapter.name)
            );
            match crate::commands::network_cmd::shell_exec_elevated("powershell", &ps_cmd, true) {
                Ok(()) => {
                    crate::log_info!("adapter", "COM ShellExec提权成功，等待IP变更...");
                    if let Some(changed_ip) = poll_ip_change(&adapter.name, &adapter.ip, 25_000) {
                        crate::log_info!("adapter", "COM提权修改MAC成功: 新IP={}", changed_ip);
                        (true, true, None)
                    } else {
                        crate::log_warn!("adapter", "COM提权修改MAC超时: 25秒内IP未变更");
                        (true, true, Some("COM提权已执行但IP未变更，可能网卡驱动不支持MAC伪装".to_string()))
                    }
                }
                Err(com_err) => {
                    crate::log_warn!("adapter", "COM ShellExec失败: {}，降级到ShellExecuteW", com_err);
                    let (ok, msg) = try_elevated_mac_script(&adapter.name, &adapter.guid, &fake_mac, &adapter.ip);
                    (ok, ok, msg)
                }
            }
        };

        let old_ip = adapter.ip.clone();
        let mut new_ip = old_ip.clone();
        let mut ip_changed = false;
        let mut message: Option<String> = elevate_msg;

        if !reg_ok {
            let _ = dhcp_release(&adapter.name);
            let _ = dhcp_renew(&adapter.name);
            if message.is_none() {
                message = Some("MAC地址修改失败，仅执行了DHCP释放/续租".to_string());
            }
        } else if elevated_done {
            // 提权脚本已完成全部操作（设MAC、禁用适配器、启用适配器、DHCP续租、删除MAC）
            // 仅验证IP是否变更
            if let Ok(refreshed) = get_adapters_force() {
                if let Some(a) = refreshed.iter().find(|a| a.name == adapter.name) {
                    if !a.ip.is_empty() {
                        new_ip = a.ip.clone();
                        ip_changed = new_ip != old_ip;
                    }
                }
            }
            if !ip_changed && message.is_none() {
                message = Some("提权脚本已执行但IP未变更，可能网卡驱动不支持MAC伪装".to_string());
            }
        } else {
            let _ = dhcp_release(&adapter.name);
            let disable_ok = netsh_disable(&adapter.name);
            if disable_ok {
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            let enable_ok = netsh_enable(&adapter.name);
            if enable_ok {
                poll_adapter_has_ip(&adapter.name, 3000);
            }
            let renew_ok = dhcp_renew(&adapter.name).unwrap_or(false);
            if renew_ok {
                if let Some(changed_ip) = poll_ip_change(&adapter.name, &old_ip, 5000) {
                    new_ip = changed_ip;
                    ip_changed = true;
                } else if let Ok(refreshed) = get_adapters_force() {
                    if let Some(a) = refreshed.iter().find(|a| a.name == adapter.name) {
                        if !a.ip.is_empty() {
                            new_ip = a.ip.clone();
                            ip_changed = new_ip != old_ip;
                        }
                    }
                }
            }
            let _ = remove_mac_from_registry(&adapter.guid);
            if !ip_changed && message.is_none() {
                message = Some("MAC已修改但IP未变更，可能网卡驱动不支持MAC伪装或DHCP服务器分配了相同IP".to_string());
            }
        }

        results.push(serde_json::json!({
            "name": adapter.name,
            "wireless": adapter.wireless,
            "ip": new_ip,
            "regOk": reg_ok,
            "success": ip_changed,
            "skipped": false,
            "message": message
        }));
    }
    Ok(results)
}

pub fn get_wireless_ssid() -> Result<Option<String>, String> {
    let output = new_command("netsh")
        .args(["wlan", "show", "interfaces"])
        .output()
        .map_err(|e| format!("获取无线网络信息失败: {}", e))?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("SSID") {
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
    let output = new_command("netsh")
        .args(["lan", "show", "interfaces"])
        .output()
        .map_err(|e| format!("获取有线网络信息失败: {}", e))?;

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

pub fn get_connected_network_names() -> Vec<String> {
    let mut names = Vec::new();

    if let Ok(Some(ssid)) = get_wireless_ssid() {
        names.push(ssid);
    }

    if let Ok(Some(profile)) = get_wired_network_profile() {
        if !names.contains(&profile) {
            names.push(profile);
        }
    }

    names
}

pub fn check_gateway_reachable(gateway: &str) -> bool {
    if gateway.is_empty() {
        return false;
    }
    let output = new_command("ping")
        .args(["-n", "1", "-w", "2000", gateway])
        .output();
    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

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

pub fn wait_for_adapter(max_wait_ms: u64, is_quitting: &std::sync::atomic::AtomicBool) -> Result<Vec<Adapter>, String> {
    let start = std::time::Instant::now();
    let mut delay_ms: u64 = 1000;

    while start.elapsed().as_millis() < max_wait_ms as u128 {
        if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(vec![]);
        }

        let adapters = get_adapters_force()?;
        if !adapters.is_empty() {
            return Ok(adapters);
        }

        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        delay_ms = (delay_ms * 2).min(5000);
    }

    get_adapters_cached()
}
