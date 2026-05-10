use std::time::Instant;
use std::net::IpAddr;
use std::io::Read;
use std::sync::Arc;
use regex::Regex;
use serde::Serialize;
use lazy_static::lazy_static;
use arc_swap::ArcSwap;
use parking_lot::RwLock;
use urlencoding;
use crate::http_timing::{measure_https_timing, measure_dns_query, measure_doh_timing};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const MAX_RESPONSE_SIZE: usize = 16 * 1024;

const CACHE_TTL_MS: u64 = 60000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Adapter {
    pub name: String,
    pub ip: String,
    pub wireless: bool,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalStatus {
    pub reachable: bool,
    pub login_available: bool,
    pub online: bool,
    pub message: String,
    pub data_length: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkQualityResult {
    pub gateway_latency: i64,
    pub external_latency: i64,
    pub average_external_latency: i64,
    pub gateway: String,
    pub quality: String,
    pub timestamp: u64,
    pub details: serde_json::Value,
    pub metrics: serde_json::Value,
}

struct AdapterCache {
    adapters: Vec<Adapter>,
    details: Vec<AdapterDetail>,
    disabled: Vec<DisabledAdapter>,
    time: Instant,
}

struct GatewayCacheEntry {
    time: Instant,
    gateway: String,
    adapter_name: String,
}

struct PortalCacheEntry {
    time: Instant,
    status: PortalStatus,
    adapter_ip: String,
}

lazy_static! {
    static ref ADAPTER_CACHE: ArcSwap<Option<AdapterCache>> = ArcSwap::from(Arc::new(None));
    static ref GATEWAY_CACHE: ArcSwap<Option<GatewayCacheEntry>> = ArcSwap::from(Arc::new(None));
    static ref PORTAL_URL: ArcSwap<String> = ArcSwap::from(Arc::new("http://10.1.99.100:801".to_string()));
    static ref PORTAL_CACHE: ArcSwap<Option<PortalCacheEntry>> = ArcSwap::from(Arc::new(None));

    static ref IP_REGEX: Regex = Regex::new(r"^(?:(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d?)$").unwrap();
    static ref BL_REGEX: Regex = Regex::new(r"(?i)hyper-v|virtual|vmware|veth|docker|wsl|loopback|tunnel|isatap|6to4|teredo|bluetooth|vpn|hamachi|zerotier|tailscale|wireguard|vEthernet|HNS|nat|filter.?driver|packet.?driver|npcap|qos|packet.?scheduler|wfp|lightweight.?filter|kernel.?debug|(?:#|[*])\s*\d+$").unwrap();
    static ref DEFAULT_GW_RE: Regex = Regex::new(r"(?i)default\s+gateway[:\s]+(\d+\.\d+\.\d+\.\d+)").unwrap();
    static ref DR1003_RE: Regex = Regex::new(r"dr1003\((.+)\)").unwrap();

    static ref HTTP_CLIENTS: RwLock<std::collections::HashMap<String, (reqwest::blocking::Client, Instant)>> = RwLock::new(std::collections::HashMap::new());
}

fn new_command(program: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);
    cmd
}

fn is_blacklisted(name: &str) -> bool {
    BL_REGEX.is_match(name)
}

fn is_virtual_description(desc: &str) -> bool {
    BL_REGEX.is_match(desc)
}

fn create_safe_http_client(timeout: std::time::Duration, local_addr: Option<IpAddr>) -> Result<reqwest::blocking::Client, String> {
    let timeout_secs = timeout.as_secs();
    let cache_key = match local_addr {
        Some(ip) => format!("{}:{}", ip, timeout_secs),
        None => format!("default:{}", timeout_secs),
    };

    {
        let clients = HTTP_CLIENTS.read();
        if let Some((client, ts)) = clients.get(&cache_key) {
            if ts.elapsed().as_secs() < 300 {
                return Ok(client.clone());
            }
        }
    }

    {
        let mut clients = HTTP_CLIENTS.write();
        let now = Instant::now();
        clients.retain(|_, (_, ts)| now.duration_since(*ts).as_secs() < 300);
        if let Some((client, ts)) = clients.get(&cache_key) {
            if ts.elapsed().as_secs() < 300 {
                return Ok(client.clone());
            }
        }
        let client = build_http_client(timeout, local_addr)?;
        clients.insert(cache_key, (client.clone(), Instant::now()));
        Ok(client)
    }
}

fn build_http_client(timeout: std::time::Duration, local_addr: Option<IpAddr>) -> Result<reqwest::blocking::Client, String> {
    let mut builder = reqwest::blocking::Client::builder()
        .min_tls_version(reqwest::tls::Version::TLS_1_3)
        .timeout(timeout)
        .connect_timeout(std::time::Duration::from_secs(5))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::limited(5))
        .pool_max_idle_per_host(2)
        .pool_idle_timeout(std::time::Duration::from_secs(30));

    if let Some(ip) = local_addr {
        builder = builder.local_address(ip);
    }

    match builder.build() {
        Ok(c) => Ok(c),
        Err(_) => {
            let mut fallback = reqwest::blocking::Client::builder()
                .min_tls_version(reqwest::tls::Version::TLS_1_2)
                .timeout(timeout)
                .connect_timeout(std::time::Duration::from_secs(5))
                .no_proxy()
                .redirect(reqwest::redirect::Policy::limited(5))
                .pool_max_idle_per_host(2)
                .pool_idle_timeout(std::time::Duration::from_secs(30));
            if let Some(ip) = local_addr {
                fallback = fallback.local_address(ip);
            }
            fallback.build().map_err(|e| format!("创建HTTP客户端失败: {}", e))
        }
    }
}

fn get_cached_adapters() -> Option<(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>)> {
    let cache_arc = ADAPTER_CACHE.load();
    match cache_arc.as_ref() {
        Some(c) if c.time.elapsed().as_millis() < CACHE_TTL_MS as u128 => {
            Some((c.adapters.clone(), c.details.clone(), c.disabled.clone()))
        }
        _ => None,
    }
}

fn set_adapters_cache(adapters: Vec<Adapter>, details: Vec<AdapterDetail>, disabled: Vec<DisabledAdapter>) {
    ADAPTER_CACHE.store(Arc::new(Some(AdapterCache { adapters, details, disabled, time: Instant::now() })));
}

pub fn get_all_adapters_cached() -> Result<(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>), String> {
    if let Some(cached) = get_cached_adapters() {
        return Ok(cached);
    }
    let (adapters, details, disabled) = query_adapters_addresses()?;
    set_adapters_cache(adapters.clone(), details.clone(), disabled.clone());
    Ok((adapters, details, disabled))
}

pub fn get_adapters_cached() -> Result<Vec<Adapter>, String> {
    if let Some((adapters, _, _)) = get_cached_adapters() {
        return Ok(adapters);
    }
    let (adapters, details, disabled) = query_adapters_addresses()?;
    set_adapters_cache(adapters.clone(), details, disabled);
    Ok(adapters)
}

pub fn get_disabled_adapters_cached() -> Result<Vec<DisabledAdapter>, String> {
    if let Some((_, _, disabled)) = get_cached_adapters() {
        return Ok(disabled);
    }
    let (adapters, details, disabled) = query_adapters_addresses()?;
    set_adapters_cache(adapters, details, disabled.clone());
    Ok(disabled)
}

pub fn get_adapters_force() -> Result<Vec<Adapter>, String> {
    clear_adapter_cache();
    get_adapters_cached()
}

pub fn get_disabled_adapters_force() -> Result<Vec<DisabledAdapter>, String> {
    clear_adapter_cache();
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
    clear_adapter_cache();
    Ok(())
}

pub fn clear_adapter_cache() {
    ADAPTER_CACHE.store(Arc::new(None));
    GATEWAY_CACHE.store(Arc::new(None));
    PORTAL_CACHE.store(Arc::new(None));
}

pub fn get_adapter_details_cached() -> Result<Vec<AdapterDetail>, String> {
    if let Some((_, details, _)) = get_cached_adapters() {
        return Ok(details);
    }
    let (adapters, details, disabled) = query_adapters_addresses()?;
    set_adapters_cache(adapters, details.clone(), disabled);
    Ok(details)
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
    use windows::Win32::NetworkManagement::Ndis::{IfOperStatusUp, IfOperStatusDown};
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

    let mut buffer = vec![0u8; size as usize];
    let ptr = buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;

    let result = unsafe {
        GetAdaptersAddresses(AF_INET.0 as u32, GAA_FLAGS, None, Some(ptr), &mut size)
    };

    if result != 0 {
        return Err(format!("GetAdaptersAddresses failed: {}", result));
    }

    if size as usize > buffer.len() {
        return Err("GetAdaptersAddresses returned inconsistent size".to_string());
    }

    let mut adapters = Vec::new();
    let mut details = Vec::new();
    let mut disabled = Vec::new();

    let mut current = ptr;
    while !current.is_null() {
        let addr = unsafe { &*current };

        let name = unsafe { read_pwstr(addr.FriendlyName) };

        let if_type = addr.IfType;
        if if_type != IF_TYPE_ETHERNET_CSMACD && if_type != IF_TYPE_IEEE80211 {
            current = addr.Next;
            continue;
        }

        let description = unsafe { read_pwstr(addr.Description) };

        if is_blacklisted(&name) || is_virtual_description(&description) {
            current = addr.Next;
            continue;
        }

        let is_up = addr.OperStatus == IfOperStatusUp;
        let is_wireless = if_type == IF_TYPE_IEEE80211;

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

            adapters.push(Adapter { name: name.clone(), ip: ip.clone(), wireless: is_wireless });
            details.push(AdapterDetail {
                name,
                ip,
                wireless: is_wireless,
                subnet_mask: prefix_len_to_mask(prefix_len as u32),
                gateway,
                dhcp_server,
            });
        } else {
            let status = if addr.OperStatus == IfOperStatusDown { "Disabled" } else { "Down" };
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
    let mut len = 0;
    while len < 1024 && *ptr.0.add(len) != 0 {
        len += 1;
    }
    if len == 0 {
        return String::new();
    }
    let slice = std::slice::from_raw_parts(ptr.0, len);
    String::from_utf16_lossy(slice)
}

#[cfg(target_os = "windows")]
unsafe fn ipv4_from_in_addr(addr: windows::Win32::Networking::WinSock::IN_ADDR) -> String {
    std::net::Ipv4Addr::from(addr).to_string()
}
pub fn check_portal_full(adapter_ip: &str, adapter_name: Option<&str>) -> Result<PortalStatus, String> {
    {
        let cache_arc = PORTAL_CACHE.load();
        if let Some(entry) = cache_arc.as_ref() {
            if entry.adapter_ip == adapter_ip && entry.time.elapsed().as_secs() < 2 {
                return Ok(entry.status.clone());
            }
        }
    }

    let result = check_portal_full_inner(adapter_ip, adapter_name)?;
    PORTAL_CACHE.store(Arc::new(Some(PortalCacheEntry {
        time: Instant::now(),
        status: result.clone(),
        adapter_ip: adapter_ip.to_string(),
    })));
    Ok(result)
}

fn check_portal_full_inner(adapter_ip: &str, adapter_name: Option<&str>) -> Result<PortalStatus, String> {
    let portal_url = PORTAL_URL.load().clone();
    let portal_host = url::Url::parse(&portal_url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .unwrap_or_default();
    let portal_host_check = if portal_host.is_empty() { "___NEVER_MATCH___" } else { &portal_host };
    let local_addr = if !adapter_ip.is_empty() {
        adapter_ip.parse::<std::net::IpAddr>().ok()
    } else {
        None
    };

    let client = create_safe_http_client(std::time::Duration::from_secs(6), local_addr)?;
    let client2 = client.clone();

    let (baidu_result, portal_result) = std::thread::scope(|s| {
        let baidu = s.spawn(|| {
            client.get("http://www.baidu.com/").timeout(std::time::Duration::from_secs(2)).send().map_err(|e| e.to_string())
        });
        let url = format!("{}/", portal_url.trim_end_matches('/'));
        let portal = s.spawn(move || {
            client2.get(&url).timeout(std::time::Duration::from_secs(5)).send().map_err(|e| e.to_string())
        });
        (baidu.join().unwrap_or_else(|_| Err("线程错误".to_string())), portal.join().unwrap_or_else(|_| Err("线程错误".to_string())))
    });

    if let Ok(mut resp) = baidu_result {
        let final_url = resp.url().to_string();
        let is_portal = final_url.contains(portal_host_check)
            || final_url.contains("eportal")
            || final_url.contains("portal/login");

        let mut body = String::new();
        let mut limited = (&mut resp).take(4096);
        let _ = limited.read_to_string(&mut body);
        let _ = std::io::copy(&mut resp, &mut std::io::sink());
        if !is_portal {
            if !body.contains("eportal") && !body.contains("dr1003") && !body.contains("portal/login") {
                let label = match adapter_name {
                    Some(name) => format!("{} 已在线", name),
                    _ => "已在线".to_string(),
                };
                return Ok(PortalStatus {
                    reachable: true,
                    login_available: false,
                    online: true,
                    message: label,
                    data_length: body.len(),
                });
            }
        }
    } else if let Err(e) = baidu_result {
        crate::log_debug!("network", "百度检测请求失败: {}", e);
    }

    let mut resp = match portal_result {
        Ok(r) => r,
        Err(e) => {
            crate::log_debug!("network", "Portal检测请求失败: {}", e);
            return Ok(PortalStatus {
                reachable: false,
                login_available: false,
                online: false,
                message: "未登录".to_string(),
                data_length: 0,
            });
        }
    };

    let mut data = String::new();
    let mut limited = (&mut resp).take(MAX_RESPONSE_SIZE as u64);
    let _ = limited.read_to_string(&mut data);
    let _ = std::io::copy(&mut resp, &mut std::io::sink());
    let total_length = data.len();

    let reachable = total_length > 0;
    let login_available = data.contains("eportal") || data.contains("login")
        || data.contains("portal") || data.contains("dr1003");

    let online = (data.contains("uid='") && data.contains("oltime=") && !data.contains("uid=''"))
        || data.contains("已经在线")
        || data.contains("ret_code\":2")
        || (data.contains("dr1003") && data.contains("\"result\":0") && data.contains("ret_code"));

    let label = match adapter_name {
        Some(name) if online => format!("{} 已在线", name),
        _ if online => "已在线".to_string(),
        _ if login_available => "未登录".to_string(),
        _ => "未登录".to_string(),
    };

    Ok(PortalStatus {
        reachable,
        login_available,
        online,
        message: label,
        data_length: total_length,
    })
}

pub fn update_portal_url(url: &str) {
    if !url.is_empty() {
        PORTAL_URL.store(Arc::new(url.to_string()));
    }
}

fn do_login_request(user: &str, password: &str, operator: &str, adapter_ip: Option<&str>) -> Result<serde_json::Value, String> {
    let validated_user = crate::config::validate_username(user).map_err(|e| e.to_string())?;
    let validated_operator = crate::config::validate_operator(operator);
    crate::config::validate_password(password).map_err(|e| e.to_string())?;
    let user_account = format!("{}{}", validated_user, validated_operator);
    let portal_base = PORTAL_URL.load().clone();
    let base_url = format!("{}/eportal/portal/login", portal_base.trim_end_matches('/'));
    let callback = "dr1003";
    let v = if validated_operator.is_empty() { "3043" } else { "2098" };
    let query_params = format!(
        "callback={}&login_method=1&user_account={}&user_password={}&wlan_user_ip&wlan_user_ipv6=&wlan_user_mac=000000000000&wlan_ac_ip=&wlan_ac_name=&jsVersion=4.1.3&terminal_type=1&lang=zh-cn&v={}&lang=zh",
        urlencoding::encode(callback),
        urlencoding::encode(&user_account),
        urlencoding::encode(password),
        v,
    );
    let url = format!("{}?{}", base_url, query_params);

    let local_addr = adapter_ip.and_then(|ip| ip.parse::<std::net::IpAddr>().ok());

    let client = create_safe_http_client(std::time::Duration::from_secs(15), local_addr)?;
    let mut resp = client.get(&url).timeout(std::time::Duration::from_secs(15)).send()
        .map_err(|e| format!("登录请求失败: {}", e.to_string().replace(&url, &format!("{}?***", base_url))))?;

    let mut body = String::new();
    let mut limited = (&mut resp).take(MAX_RESPONSE_SIZE as u64);
    let _ = limited.read_to_string(&mut body);
    let _ = std::io::copy(&mut resp, &mut std::io::sink());

    parse_login_result(&body)
}

pub fn do_login_with_retry(user: &str, password: &str, operator: &str, adapter_ip: Option<&str>, max_retries: u32, is_quitting: &std::sync::atomic::AtomicBool) -> Result<serde_json::Value, String> {
    let mut last_result = None;

    for attempt in 1..=max_retries {
        if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(serde_json::json!({ "code": "error", "message": "应用正在退出", "success": false }));
        }

        let result = do_login_request(user, password, operator, adapter_ip);
        match result {
            Ok(ref r) if r.get("success").and_then(|v| v.as_bool()).unwrap_or(false) => {
                return Ok(r.clone());
            }
            Ok(r) => {
                last_result = Some(r);
            }
            Err(e) => {
                last_result = Some(serde_json::json!({ "code": "error", "message": e, "success": false }));
            }
        }

        if attempt < max_retries {
            std::thread::sleep(std::time::Duration::from_millis(1000 * attempt as u64));
        }
    }

    let last = last_result.unwrap_or_else(|| serde_json::json!({ "code": "max_retries", "message": "多次重试后仍失败", "success": false }));
    Ok(last)
}

fn parse_login_result(response: &str) -> Result<serde_json::Value, String> {
    let json_data = if let Some(captures) = DR1003_RE
        .captures(response)
        .and_then(|caps| caps.get(1))
    {
        captures.as_str().to_string()
    } else {
        response.to_string()
    };

    match serde_json::from_str::<serde_json::Value>(&json_data) {
        Ok(data) => {
            let result = data.get("result").and_then(|v| v.as_i64()).unwrap_or(-1);
            let msg = data.get("msg").and_then(|v| v.as_str()).unwrap_or("");

            if result == 0 {
                if msg.contains("已经在线") {
                    Ok(serde_json::json!({ "code": "0", "message": msg, "success": true }))
                } else if msg.contains("认证成功") {
                    Ok(serde_json::json!({ "code": "0", "message": "登录成功", "success": true }))
                } else if msg.contains("AC认证失败") {
                    Ok(serde_json::json!({ "code": "0", "message": format!("认证失败：{}", msg), "success": false }))
                } else {
                    Ok(serde_json::json!({ "code": "0", "message": if msg.is_empty() { "登录成功" } else { msg }, "success": true }))
                }
            } else if result == 1 {
                if msg.contains("认证成功") {
                    Ok(serde_json::json!({ "code": "0", "message": "登录成功", "success": true }))
                } else {
                    Ok(serde_json::json!({ "code": "1", "message": if msg.is_empty() { "认证失败" } else { msg }, "success": false }))
                }
            } else {
                Ok(serde_json::json!({ "code": "unknown", "message": if msg.is_empty() { "未知响应" } else { msg }, "success": false }))
            }
        }
        Err(_) => {
            Ok(serde_json::json!({ "code": "parse_error", "message": "无法解析登录响应", "success": false }))
        }
    }
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
    clear_adapter_cache();
    Ok(results)
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

fn get_gateway_ip_cached(adapter_name: &str, adapter_ip: &str) -> Result<String, String> {
    {
        let cache_arc = GATEWAY_CACHE.load();
        if let Some(entry) = cache_arc.as_ref() {
            if entry.adapter_name == adapter_name && entry.time.elapsed().as_millis() < CACHE_TTL_MS as u128 {
                return Ok(entry.gateway.clone());
            }
        }
    }

    if let Ok(details) = get_adapter_details_cached() {
        if let Some(d) = details.iter().find(|d| d.name == adapter_name) {
            if !d.gateway.is_empty() {
                GATEWAY_CACHE.store(Arc::new(Some(GatewayCacheEntry {
                    time: Instant::now(),
                    gateway: d.gateway.clone(),
                    adapter_name: adapter_name.to_string(),
                })));
                return Ok(d.gateway.clone());
            }
        }
    }

    let mut gateway = None;

    if !adapter_ip.is_empty() {
        if let Ok(output) = new_command("cmd")
            .args(["/C", "chcp 437 >nul & ipconfig"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let sections: Vec<&str> = stdout.split('\n').collect();
            let mut in_section = false;
            for line in &sections {
                if line.contains(adapter_ip) { in_section = true; }
                if in_section {
                    if let Some(caps) = DEFAULT_GW_RE.captures(line) {
                        gateway = Some(caps[1].to_string());
                        break;
                    }
                }
            }
            if gateway.is_none() {
                if let Some(caps) = DEFAULT_GW_RE.captures(&stdout) {
                    gateway = Some(caps[1].to_string());
                }
            }
        }
    }

    if gateway.is_none() && !adapter_ip.is_empty() {
        let parts: Vec<&str> = adapter_ip.split('.').collect();
        if parts.len() == 4 {
            gateway = Some(format!("{}.{}.{}.1", parts[0], parts[1], parts[2]));
        }
    }

    if let Some(ref gw) = gateway {
        GATEWAY_CACHE.store(Arc::new(Some(GatewayCacheEntry {
            time: Instant::now(),
            gateway: gw.clone(),
            adapter_name: adapter_name.to_string(),
        })));
    }

    gateway.ok_or_else(|| "未找到网关".to_string())
}

async fn ping_host_async(host: &str, timeout_ms: u32) -> Result<u64, String> {
    use surge_ping::Client;
    use surge_ping::ICMP;
    use surge_ping::PingIdentifier;
    use surge_ping::PingSequence;
    use std::net::IpAddr;

    let addr: IpAddr = host.parse().map_err(|_| format!("无效的主机地址: {}", host))?;
    let config = match addr {
        IpAddr::V4(_) => surge_ping::Config::default(),
        IpAddr::V6(_) => surge_ping::Config::builder().kind(ICMP::V6).build(),
    };
    let client = Client::new(&config).map_err(|e| format!("创建ping客户端失败: {}", e))?;

    let timeout = std::time::Duration::from_millis(timeout_ms as u64);
    let mut total_ms = 0u64;
    let mut success_count = 0u32;

    let ident = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().subsec_nanos() & 0xFFFF) as u16;
    let mut pinger = client.pinger(addr, PingIdentifier(ident)).await;
    pinger.timeout(timeout);

    for seq in 0..3u16 {
        match tokio::time::timeout(timeout, pinger.ping(PingSequence(seq), &[])).await {
            Ok(Ok((_, duration))) => {
                if seq > 0 {
                    total_ms += duration.as_millis() as u64;
                    success_count += 1;
                }
            }
            Ok(Err(_)) => continue,
            Err(_) => continue,
        }
    }

    if success_count > 0 {
        Ok((total_ms / success_count as u64).max(1))
    } else {
        Err("ping failed: 100% packet loss".to_string())
    }
}

async fn check_tcp_latency_async(host: &str, port: u16, timeout_ms: u64) -> i64 {
    if host.is_empty() {
        return -1;
    }
    let timeout_ms = timeout_ms.clamp(100, 30000);
    let start = Instant::now();
    let target = match format!("{}:{}", host, port).parse::<std::net::SocketAddr>() {
        Ok(a) => a,
        Err(_) => return -1,
    };
    match tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        tokio::net::TcpStream::connect(&target),
    ).await {
        Ok(Ok(_)) => {
            let elapsed_us = start.elapsed().as_micros();
            ((elapsed_us + 500) / 1000).max(1) as i64
        }
        _ => -1,
    }
}

async fn check_dns_latency_async(hostname: &str, timeout_ms: u64) -> i64 {
    let start = Instant::now();
    let addr = if hostname.contains(':') {
        format!("[{}]:0", hostname)
    } else {
        format!("{}:0", hostname)
    };
    let lookup = tokio::net::lookup_host(&addr);
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        lookup,
    ).await;
    match result {
        Ok(Ok(mut addrs)) => {
            if addrs.next().is_some() {
                let elapsed_us = start.elapsed().as_micros();
                let elapsed = ((elapsed_us + 500) / 1000) as i64;
                elapsed.max(1)
            } else {
                -1
            }
        }
        _ => -1,
    }
}

async fn tcp_then_icmp_latency(host: &str, ports: &[u16], tcp_timeout_ms: u64) -> (i64, &'static str) {
    let mut tcp_tasks = tokio::task::JoinSet::new();
    for &port in ports {
        let h = host.to_string();
        tcp_tasks.spawn(async move {
            check_tcp_latency_async(&h, port, tcp_timeout_ms).await
        });
    }
    while let Some(res) = tcp_tasks.join_next().await {
        if let Ok(lat) = res {
            if lat >= 0 {
                tcp_tasks.abort_all();
                return (lat, "tcp");
            }
        }
    }

    let host_icmp = host.to_string();
    let icmp_lat = ping_host_async(&host_icmp, 500)
        .await
        .map(|v| v as i64)
        .unwrap_or(-1i64);

    if icmp_lat >= 0 {
        (icmp_lat, "icmp")
    } else {
        (-1i64, "icmp")
    }
}

enum LatencyTask {
    Gateway { name: String, target: String },
    Doh { name: String, doh_server: String, doh_ip: String, doh_host: String },
    Https { name: String, host: String },
    DnsResolve { name: String, target: String },
    DnsServer { name: String, ip: String, domain: String },
}

struct LatencyTaskCtx {
    task: LatencyTask,
    bind_addr: Option<std::net::IpAddr>,
}

struct LatencyResult {
    name: String,
    target: String,
    latency: i64,
    lat_type: String,
    is_external: bool,
    dns_ms: i64,
    tcp_ms: i64,
    tls_ms: i64,
    udp_ms: i64,
    ttfb_ms: i64,
    content_ms: i64,
}

pub async fn check_network_quality_async(adapter_name: &str, adapter_ip: &str, skip_ttfb: bool, skip_content: bool) -> NetworkQualityResult {
    let now = Instant::now();

    let gateway = tokio::task::spawn_blocking({
        let an = adapter_name.to_string();
        let ai = adapter_ip.to_string();
        move || get_gateway_ip_cached(&an, &ai).ok()
    }).await.unwrap_or(None);

    let gateway_str = gateway.as_deref().unwrap_or("");

    let bind_addr: Option<std::net::IpAddr> = if adapter_ip.is_empty() {
        None
    } else {
        adapter_ip.parse().ok()
    };

    let mut tasks: Vec<LatencyTaskCtx> = Vec::new();

    if let Some(ref gw) = gateway {
        tasks.push(LatencyTaskCtx {
            task: LatencyTask::Gateway {
                name: "网关".to_string(),
                target: gw.clone(),
            },
            bind_addr,
        });
    }

    tasks.push(LatencyTaskCtx { task: LatencyTask::Doh {
        name: "阿里DoH".to_string(),
        doh_server: "dns.alidns.com".to_string(),
        doh_ip: "223.5.5.5".to_string(),
        doh_host: "baidu.com".to_string(),
    }, bind_addr });
    tasks.push(LatencyTaskCtx { task: LatencyTask::Doh {
        name: "腾讯DoH".to_string(),
        doh_server: "doh.pub".to_string(),
        doh_ip: String::new(),
        doh_host: "baidu.com".to_string(),
    }, bind_addr });

    tasks.push(LatencyTaskCtx { task: LatencyTask::DnsResolve {
        name: "DNS解析".to_string(),
        target: "www.baidu.com".to_string(),
    }, bind_addr });

    tasks.push(LatencyTaskCtx { task: LatencyTask::DnsServer {
        name: "阿里DNS".to_string(),
        ip: "223.5.5.5".to_string(),
        domain: "www.baidu.com".to_string(),
    }, bind_addr });
    tasks.push(LatencyTaskCtx { task: LatencyTask::DnsServer {
        name: "腾讯DNS".to_string(),
        ip: "119.29.29.29".to_string(),
        domain: "www.baidu.com".to_string(),
    }, bind_addr });
    tasks.push(LatencyTaskCtx { task: LatencyTask::DnsServer {
        name: "信风DNS".to_string(),
        ip: "114.114.114.114".to_string(),
        domain: "www.baidu.com".to_string(),
    }, bind_addr });

    let https_hosts: &[(&str, &str)] = &[
        ("百度", "www.baidu.com"),
        ("京东", "www.jd.com"),
        ("必应", "cn.bing.com"),
        ("12306", "www.12306.cn"),
        ("英雄联盟", "lol.qq.com"),
        ("原神", "mhyy.mihoyo.com"),
        ("绝地求生", "pubg.qq.com"),
        ("永劫无间", "www.yjwujian.cn"),
        ("哔哩哔哩", "www.bilibili.com"),
        ("哔哩哔哩直播", "live.bilibili.com"),
        ("抖音", "www.douyin.com"),
        ("抖音直播", "live.douyin.com"),
    ];

    for (name, host) in https_hosts {
        tasks.push(LatencyTaskCtx { task: LatencyTask::Https {
            name: name.to_string(),
            host: host.to_string(),
        }, bind_addr });
    }

    let mut join_set = tokio::task::JoinSet::new();
    for ctx in tasks {
        join_set.spawn(async move {
            match ctx.task {
                LatencyTask::Gateway { name, target } => {
                    let (lat, lat_type) = tcp_then_icmp_latency(&target, &[80, 53], 800).await;
                    LatencyResult { name, target, latency: lat, lat_type: lat_type.to_string(), is_external: false, dns_ms: -1, tcp_ms: -1, tls_ms: -1, udp_ms: -1, ttfb_ms: -1, content_ms: -1 }
                }
                LatencyTask::Doh { name, doh_server, doh_ip, doh_host } => {
                    let r = measure_doh_timing(&doh_server, &doh_ip, &doh_host, ctx.bind_addr, std::time::Duration::from_millis(2000), skip_ttfb).await;
                    let lat = if r.success { r.total_ms } else { -1 };
                    LatencyResult {
                        name,
                        target: format!("https://{}", doh_server),
                        latency: lat,
                        lat_type: "doh".to_string(),
                        is_external: true,
                        dns_ms: r.dns_ms,
                        tcp_ms: r.tcp_ms,
                        tls_ms: r.tls_ms,
                        udp_ms: -1,
                        ttfb_ms: r.http_ms,
                        content_ms: -1,
                    }
                }
                LatencyTask::Https { name, host } => {
                    let r = measure_https_timing(&host, 443, ctx.bind_addr, std::time::Duration::from_secs(3), skip_ttfb, skip_content).await;
                    let lat = if r.success { r.total_ms } else { -1 };
                    LatencyResult {
                        name,
                        target: format!("https://{}", host),
                        latency: lat,
                        lat_type: "https".to_string(),
                        is_external: true,
                        dns_ms: r.dns_ms,
                        tcp_ms: r.tcp_ms,
                        tls_ms: r.tls_ms,
                        udp_ms: -1,
                        ttfb_ms: r.ttfb_ms,
                        content_ms: r.content_ms,
                    }
                }
                LatencyTask::DnsResolve { name, target } => {
                    let lat = check_dns_latency_async(&target, 800).await;
                    LatencyResult { name, target, latency: lat, lat_type: "dns".to_string(), is_external: true, dns_ms: -1, tcp_ms: -1, tls_ms: -1, udp_ms: -1, ttfb_ms: -1, content_ms: -1 }
                }
                LatencyTask::DnsServer { name, ip, domain } => {
                    let r = measure_dns_query(&ip, &domain, ctx.bind_addr, std::time::Duration::from_millis(1500)).await;
                    let lat = match (r.udp_ms, r.tcp_ms) {
                        (u, t) if u >= 0 && t >= 0 => u.min(t),
                        (u, _) if u >= 0 => u,
                        (_, t) => t,
                    };
                    LatencyResult {
                        name,
                        target: format!("{}:53", ip),
                        latency: lat,
                        lat_type: "dns".to_string(),
                        is_external: true,
                        dns_ms: -1,
                        tcp_ms: r.tcp_ms,
                        tls_ms: -1,
                        udp_ms: r.udp_ms,
                        ttfb_ms: -1,
                        content_ms: -1,
                    }
                }
            }
        });
    }

    let mut results: Vec<LatencyResult> = Vec::new();
    let mut gateway_latency: i64 = -1;
    while let Some(res) = join_set.join_next().await {
        if let Ok(r) = res {
            results.push(r);
        }
    }

    let mut details = serde_json::Map::new();
    let mut metrics = serde_json::Map::new();
    let mut external_values: Vec<i64> = Vec::new();

    for r in &results {
        if r.is_external && r.latency >= 0 {
            external_values.push(r.latency);
        }
        if r.name == "网关" {
            gateway_latency = r.latency;
        }
        let mut detail_json = serde_json::json!({
            "target": r.target, "latency": r.latency, "type": r.lat_type
        });
        if r.dns_ms >= 0 {
            detail_json["dnsLatency"] = serde_json::json!(r.dns_ms);
            detail_json["tcpLatency"] = serde_json::json!(r.tcp_ms);
            detail_json["tlsLatency"] = serde_json::json!(r.tls_ms);
            let segments = r.dns_ms.max(0) + r.tcp_ms.max(0) + r.tls_ms.max(0);
            if r.latency > segments {
                detail_json["networkLatency"] = serde_json::json!(r.latency - segments);
            }
        }
        if r.udp_ms >= 0 && r.lat_type == "dns" {
            detail_json["udpLatency"] = serde_json::json!(r.udp_ms);
        }
        if r.tcp_ms >= 0 && r.lat_type == "dns" {
            detail_json["tcpLatency"] = serde_json::json!(r.tcp_ms);
        }
        if r.ttfb_ms >= 0 {
            detail_json["ttfbLatency"] = serde_json::json!(r.ttfb_ms);
        }
        if r.content_ms >= 0 {
            detail_json["contentLatency"] = serde_json::json!(r.content_ms);
        }
        details.insert(r.name.clone(), detail_json);
        metrics.insert(r.name.clone(), serde_json::json!({
            "latency": r.latency, "type": r.lat_type, "elapsed": now.elapsed().as_millis()
        }));
    }

    let (external_latency, average_external_latency) = if !external_values.is_empty() {
        let mut sorted = external_values.clone();
        sorted.sort();
        let mid = sorted.len() / 2;
        let med = if sorted.len() % 2 != 0 {
            sorted[mid]
        } else {
            (sorted[mid - 1] + sorted[mid]) / 2
        };
        let trimmed = if sorted.len() >= 4 {
            let trim = (sorted.len() as f64 * 0.15).ceil() as usize;
            let trim = trim.min(sorted.len() / 2);
            sorted[trim..sorted.len() - trim].to_vec()
        } else if sorted.len() >= 3 {
            let mut v = sorted.clone();
            v.remove(0);
            v.remove(v.len() - 1);
            v
        } else {
            sorted.clone()
        };
        let avg = if !trimmed.is_empty() {
            trimmed.iter().sum::<i64>() / trimmed.len() as i64
        } else {
            sorted.iter().sum::<i64>() / sorted.len() as i64
        };
        (med.max(1), avg.max(1))
    } else {
        (-1, -1)
    };

    fn get_latency_level(latency: i64) -> usize {
        if latency < 0 { return 5; }
        if latency <= 20 { return 0; }
        if latency <= 50 { return 1; }
        if latency <= 100 { return 2; }
        if latency <= 200 { return 3; }
        if latency <= 400 { return 4; }
        5
    }
    let level_names = ["excellent", "great", "good", "fair", "poor", "bad"];

    let quality = if gateway_latency >= 0 && external_latency >= 0 {
        let level = std::cmp::max(get_latency_level(gateway_latency), get_latency_level(external_latency));
        level_names[level].to_string()
    } else if gateway_latency >= 0 {
        level_names[get_latency_level(gateway_latency)].to_string()
    } else if external_latency >= 0 {
        level_names[get_latency_level(external_latency)].to_string()
    } else {
        "unknown".to_string()
    };

    let total_elapsed = now.elapsed().as_millis() as u64;

    NetworkQualityResult {
        gateway_latency,
        external_latency,
        average_external_latency,
        gateway: gateway_str.to_string(),
        quality,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        details: serde_json::Value::Object(details),
        metrics: serde_json::json!({
            "totalElapsed": total_elapsed,
            "tests": serde_json::Value::Object(metrics),
        }),
    }
}

pub fn wait_for_adapter(max_wait_ms: u64, is_quitting: &std::sync::atomic::AtomicBool) -> Result<Vec<Adapter>, String> {
    let start = Instant::now();
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
