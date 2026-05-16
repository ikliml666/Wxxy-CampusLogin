use regex::Regex;
use serde::Serialize;
use lazy_static::lazy_static;
use parking_lot::Mutex;
use std::time::Instant;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

lazy_static! {
    pub(crate) static ref BL_REGEX: Regex = Regex::new(r"(?i)hyper-v|virtual|vmware|veth|docker|wsl|loopback|tunnel|isatap|6to4|teredo|bluetooth|vpn|hamachi|zerotier|tailscale|wireguard|vEthernet|HNS|nat|filter.?driver|packet.?driver|npcap|qos|packet.?scheduler|wfp|lightweight.?filter|kernel.?debug|(?:#|[*])\s*\d+$|clash|v2ray|xray|sing-box|shadowsocks|ss-local|hysteria|trojan|naiveproxy|mihomo|surge|quantumult|loon|stash|surfboard|netch|proxifier|privoxy|tor|i2p|tun2socks|tap-|tun0|wg0|utun|clash\.tun|clash\.tap|meta\.tun|sing\.tun|cloudflare.?warp|warp").unwrap();
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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisabledAdapter {
    pub name: String,
    pub status: String,
    pub description: String,
}

pub(crate) fn new_command(program: &str) -> std::process::Command {
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

        let description = unsafe { read_pwstr(addr.Description) };

        if is_blacklisted(&name) || is_virtual_description(&description) {
            current = addr.Next;
            continue;
        }

        let is_up = addr.OperStatus == IfOperStatusUp;
        let is_wireless = if_type == if_type_wireless;

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

            if ip.is_empty() {
            } else {
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

                adapters.push(Adapter { name: name.clone(), ip: ip.clone(), wireless: is_wireless, guid: guid.clone() });
                details.push(AdapterDetail {
                    name,
                    ip,
                    wireless: is_wireless,
                    subnet_mask: prefix_len_to_mask(prefix_len as u32),
                    gateway,
                    dhcp_server,
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

pub fn enable_adapter(adapter_name: &str) -> Result<(), String> {
    if adapter_name.is_empty() {
        return Err("适配器名称不能为空".to_string());
    }
    if adapter_name.len() > 128 {
        return Err("适配器名称过长".to_string());
    }
    let forbidden = ['&', '|', ';', '`', '$', '(', ')', '<', '>', '"', '\'', '\n', '\r', '\0'];
    if adapter_name.chars().any(|c| forbidden.contains(&c)) {
        return Err("适配器名称包含非法字符".to_string());
    }
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
        adapters.first().map(|a| a.name.clone()).unwrap_or_default()
    } else {
        config.adapter1.clone()
    };

    let adapter2 = if config.dual_adapter {
        if config.adapter2.is_empty() || config.adapter2 == "自动检测" {
            adapters.iter().find(|a| a.name != adapter1).map(|a| a.name.clone()).unwrap_or_default()
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

pub fn get_gateway_ip(adapter_name: &str, adapter_ip: &str) -> Result<String, String> {
    if let Ok(details) = get_adapter_details_cached() {
        if let Some(d) = details.iter().find(|d| d.name == adapter_name) {
            if !d.gateway.is_empty() {
                return Ok(d.gateway.clone());
            }
        }
    }

    if !adapter_ip.is_empty() {
        let parts: Vec<&str> = adapter_ip.split('.').collect();
        if parts.len() == 4 {
            return Ok(format!("{}.{}.{}.1", parts[0], parts[1], parts[2]));
        }
    }

    Err("未找到网关".to_string())
}

fn dhcp_renew(adapter_name: &str) -> Result<bool, String> {
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
