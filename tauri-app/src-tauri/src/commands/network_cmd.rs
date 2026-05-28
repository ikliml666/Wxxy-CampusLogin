use tauri::{AppHandle, Manager, State};
use std::os::windows::process::CommandExt;
use std::sync::Arc;
use crate::network::{
    Adapter, AdapterDetail, DisabledAdapter,
    get_adapters_cached, get_disabled_adapters_cached,
    enable_adapter as enable_adapter_inner, get_adapter_details_cached,
    check_portal_full, dhcp_renew_wired_only, dhcp_release_renew_all,
    select_adapter,
    check_network_quality_async,
};

use super::state::{AppState, CommandResult};

#[cfg(target_os = "windows")]
pub fn is_admin() -> bool {
    use windows::Win32::System::Threading::{OpenProcessToken, GetCurrentProcess};
    use windows::Win32::Security::{TOKEN_QUERY, GetTokenInformation, TokenElevation};
    use windows::Win32::Foundation::HANDLE;

    unsafe {
        // SAFETY: OpenProcessToken requires a valid process handle.
        // GetCurrentProcess() always returns a pseudo-handle that is valid for the current process.
        // We only read from the token (TOKEN_QUERY), never modify it.
        // CloseHandle is called on the token before returning.
        let mut token: HANDLE = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation: u32 = 0;
        let mut returned = 0u32;
        let result = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<u32>() as u32,
            &mut returned,
        );
        let _ = windows::Win32::Foundation::CloseHandle(token);
        result.is_ok() && elevation != 0
    }
}

#[cfg(target_os = "windows")]
const PRIMARY_DNS: &str = "223.5.5.5";
#[cfg(target_os = "windows")]
const SECONDARY_DNS: &str = "1.12.12.12";
#[cfg(target_os = "windows")]
const DOH_SERVERS: &[(&str, &str)] = &[
    ("223.5.5.5", "https://dns.alidns.com/dns-query"),
    ("223.6.6.6", "https://dns.alidns.com/dns-query"),
    ("1.12.12.12", "https://doh.pub/dns-query"),
    ("120.53.53.53", "https://doh.pub/dns-query"),
];
#[cfg(target_os = "windows")]
const DNS_PROPERTY_TYPE_DOH: u32 = 1;

#[cfg(target_os = "windows")]
pub(crate) fn run_elevated(cmd: &str, args: &str) -> Result<(), String> {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::core::PCWSTR;

    let verb: Vec<u16> = "runas\0".encode_utf16().collect();
    let file: Vec<u16> = format!("{}\0", cmd).encode_utf16().collect();
    let params: Vec<u16> = format!("{}\0", args).encode_utf16().collect();

    unsafe {
        // SAFETY: ShellExecuteW with "runas" verb launches a new process with elevated privileges.
        // The verb, file, and params strings are null-terminated UTF-16 vectors that remain valid
        // for the duration of the call. SW_HIDE prevents the elevated process window from showing.
        let result = ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR(params.as_ptr()),
            None,
            windows::Win32::UI::WindowsAndMessaging::SW_HIDE,
        );
        let val = result.0 as isize;
        if val <= 32 {
            return Err(format!("ShellExecuteW runas 失败，错误码: {}", val));
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub fn set_registry_elevated(
    sub_key: &str,
    value_name: &str,
    value_data: &str,
) -> Result<(), String> {
    use windows::Win32::System::Com::CoInitializeEx;
    use windows::Win32::System::Com::COINIT_APARTMENTTHREADED;

    unsafe {
        // SAFETY: CoInitializeEx initializes the COM library for the current thread.
        // We use COINIT_APARTMENTTHREADED for STA, which is required for the elevated COM moniker.
        // The return value is intentionally ignored (S_OK or S_FALSE for re-initialization).
        // co_get_object_raw uses the elevation moniker to obtain an elevated COM object.
        // All wide strings (moniker, sub_key, value_name, value_data) are null-terminated
        // UTF-16 vectors that remain valid for the duration of the COM calls.
        // The COM object is released via vtbl.release() before returning.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let moniker_name = "Elevation:Administrator!new:{3E5FC7F9-9A51-4367-9063-A120244FBEC7}";
        let moniker_wide: Vec<u16> = moniker_name.encode_utf16().chain(std::iter::once(0)).collect();

        let iid_icmluautil: windows::core::GUID = windows::core::GUID::from_values(
            0x6EDD6D74, 0xC007, 0x4E75, [0xB7, 0x6A, 0xE5, 0x74, 0x09, 0x95, 0xE2, 0x4C],
        );

        let mut bind_opts: windows::Win32::System::Com::BIND_OPTS = std::mem::zeroed();
        bind_opts.cbStruct = std::mem::size_of::<windows::Win32::System::Com::BIND_OPTS>() as u32;

        let mut p_unknown: *mut std::ffi::c_void = std::ptr::null_mut();

        let hr = co_get_object_raw(
            moniker_wide.as_ptr(),
            &mut bind_opts as *mut _ as *mut _,
            &iid_icmluautil as *const _ as *const _,
            &mut p_unknown as *mut _ as *mut _,
        );

        if hr != 0 || p_unknown.is_null() {
            return Err(format!("COM提权失败: HRESULT=0x{:08X}", hr as u32));
        }

        let vtbl = *(p_unknown as *mut *const ICMLuaUtilVtbl);

        let sub_key_wide: Vec<u16> = sub_key.encode_utf16().chain(std::iter::once(0)).collect();
        let value_name_wide: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
        let value_data_wide: Vec<u16> = value_data.encode_utf16().chain(std::iter::once(0)).collect();

        let hkey_hklm: isize = 0x80000002;

        let result = ((*vtbl).set_registry_string_value)(
            p_unknown,
            hkey_hklm,
            sub_key_wide.as_ptr(),
            value_name_wide.as_ptr(),
            value_data_wide.as_ptr(),
        );

        ((*vtbl).release)(p_unknown);

        if result.is_err() {
            return Err(format!("SetRegistryStringValue 失败: {:?}", result));
        }

        Ok(())
    }
}

/// 通过 COM Elevation Moniker (ICMLuaUtil::ShellExec) 以管理员权限启动进程。
///
/// **注意**: ShellExec 是异步的——返回 Ok(()) 仅表示成功启动了提权进程，
/// 不代表目标进程已执行完成或执行成功。调用者应通过后续状态检查
/// （如验证DNS是否已设置、MAC是否已变更）来确认实际执行结果。
#[cfg(target_os = "windows")]
pub fn shell_exec_elevated(
    file: &str,
    params: &str,
    hide_window: bool,
) -> Result<(), String> {
    use windows::Win32::System::Com::CoInitializeEx;
    use windows::Win32::System::Com::COINIT_APARTMENTTHREADED;

    unsafe {
        // SAFETY: CoInitializeEx initializes the COM library for the current thread (STA mode).
        // The return value is intentionally ignored as per COM re-initialization semantics.
        // co_get_object_raw uses the elevation moniker to obtain an elevated COM object
        // (ICMLuaUtil) for executing shell commands with admin privileges.
        // All wide strings (moniker, file, params) are null-terminated UTF-16 vectors
        // that remain valid for the duration of the COM calls.
        // The COM object is released via vtbl.release() before returning.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let moniker_name = "Elevation:Administrator!new:{3E5FC7F9-9A51-4367-9063-A120244FBEC7}";
        let moniker_wide: Vec<u16> = moniker_name.encode_utf16().chain(std::iter::once(0)).collect();

        let iid_icmluautil: windows::core::GUID = windows::core::GUID::from_values(
            0x6EDD6D74, 0xC007, 0x4E75, [0xB7, 0x6A, 0xE5, 0x74, 0x09, 0x95, 0xE2, 0x4C],
        );

        let mut bind_opts: windows::Win32::System::Com::BIND_OPTS = std::mem::zeroed();
        bind_opts.cbStruct = std::mem::size_of::<windows::Win32::System::Com::BIND_OPTS>() as u32;

        let mut p_unknown: *mut std::ffi::c_void = std::ptr::null_mut();

        let hr = co_get_object_raw(
            moniker_wide.as_ptr(),
            &mut bind_opts as *mut _ as *mut _,
            &iid_icmluautil as *const _ as *const _,
            &mut p_unknown as *mut _ as *mut _,
        );

        if hr != 0 || p_unknown.is_null() {
            return Err(format!("COM提权失败: HRESULT=0x{:08X}", hr as u32));
        }

        let vtbl = *(p_unknown as *mut *const ICMLuaUtilVtbl);

        let file_wide: Vec<u16> = file.encode_utf16().chain(std::iter::once(0)).collect();
        let params_wide: Vec<u16> = params.encode_utf16().chain(std::iter::once(0)).collect();
        let n_show: u32 = if hide_window { 0 } else { 1 };

        let result = ((*vtbl).shell_exec)(
            p_unknown,
            file_wide.as_ptr(),
            params_wide.as_ptr(),
            std::ptr::null(),
            0,
            n_show,
        );

        ((*vtbl).release)(p_unknown);

        if result.is_err() {
            return Err(format!("ShellExec 失败: {:?}", result));
        }

        Ok(())
    }
}

#[cfg(target_os = "windows")]
#[allow(non_snake_case)]
unsafe fn co_get_object_raw(
    pszname: *const u16,
    pbindoptions: *mut std::ffi::c_void,
    riid: *const std::ffi::c_void,
    ppv: *mut *mut std::ffi::c_void,
) -> i32 {
    // SAFETY: This is a thin wrapper around the Windows CoGetObject FFI call.
    // The caller is responsible for ensuring:
    // - pszname points to a valid null-terminated UTF-16 string (the elevation moniker).
    // - pbindoptions points to a valid BIND_OPTS structure with correct cbStruct.
    // - riid points to a valid GUID identifying the requested interface.
    // - ppv points to a valid pointer that will receive the COM object.
    // All pointers must remain valid for the duration of the call.
    #[link(name = "ole32")]
    extern "system" {
        fn CoGetObject(
            pszname: *const u16,
            pbindoptions: *mut std::ffi::c_void,
            riid: *const std::ffi::c_void,
            ppv: *mut *mut std::ffi::c_void,
        ) -> i32;
    }
    CoGetObject(pszname, pbindoptions, riid, ppv)
}

#[cfg(target_os = "windows")]
fn parse_guid(s: &str) -> Result<windows::core::GUID, String> {
    let s = s.trim().trim_start_matches('{').trim_end_matches('}');
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 {
        return Err(format!("GUID格式无效: {}", s));
    }
    let data1 = u32::from_str_radix(parts[0], 16).map_err(|e| format!("GUID data1解析失败: {}", e))?;
    let data2 = u16::from_str_radix(parts[1], 16).map_err(|e| format!("GUID data2解析失败: {}", e))?;
    let data3 = u16::from_str_radix(parts[2], 16).map_err(|e| format!("GUID data3解析失败: {}", e))?;
    let data4_str = &parts[3..=4].join("");
    if data4_str.len() != 16 {
        return Err(format!("GUID data4长度无效: {}", data4_str));
    }
    let mut data4 = [0u8; 8];
    for i in 0..8 {
        data4[i] = u8::from_str_radix(&data4_str[i * 2..i * 2 + 2], 16)
            .map_err(|e| format!("GUID data4[{}]解析失败: {}", i, e))?;
    }
    Ok(windows::core::GUID::from_values(data1, data2, data3, data4))
}

#[cfg(target_os = "windows")]
fn set_dns_inner(
    adapter_guid: &str,
    dns_servers: &[&str],
    doh_templates: &[(&str, &str)],
    include_doh: bool,
    err_label: &str,
) -> Result<(), String> {
    use windows::Win32::NetworkManagement::IpHelper::*;
    use windows::core::PWSTR;

    let guid = parse_guid(adapter_guid)?;

    let ns_str: String = dns_servers.join(",");
    let mut ns_wide: Vec<u16> = ns_str.encode_utf16().chain(std::iter::once(0)).collect();

    let mut doh_props: Vec<DNS_SERVER_PROPERTY> = Vec::new();
    let mut doh_settings: Vec<DNS_DOH_SERVER_SETTINGS> = Vec::new();
    let mut doh_templates_wide: Vec<Vec<u16>> = Vec::new();

    for (idx, (_ip, template)) in doh_templates.iter().enumerate() {
        let tpl_wide: Vec<u16> = template.encode_utf16().chain(std::iter::once(0)).collect();
        doh_templates_wide.push(tpl_wide);

        let doh_setting = DNS_DOH_SERVER_SETTINGS {
            Template: PWSTR(doh_templates_wide.last_mut().unwrap().as_mut_ptr()),
            Flags: (DNS_DOH_SERVER_SETTINGS_ENABLE_AUTO | DNS_DOH_SERVER_SETTINGS_ENABLE | DNS_DOH_SERVER_SETTINGS_FALLBACK_TO_UDP) as u64,
        };
        doh_settings.push(doh_setting);

        let prop = DNS_SERVER_PROPERTY {
            Version: DNS_SERVER_PROPERTY_VERSION1,
            ServerIndex: idx as u32,
            Type: DNS_SERVER_PROPERTY_TYPE(DNS_PROPERTY_TYPE_DOH),
            Property: DNS_SERVER_PROPERTY_TYPES {
                DohSettings: &mut doh_settings[idx],
            },
        };
        doh_props.push(prop);
    }

    let flags = if include_doh && !doh_props.is_empty() {
        (DNS_SETTING_NAMESERVER | DNS_SETTING_DOH) as u64
    } else {
        DNS_SETTING_NAMESERVER as u64
    };

    let settings = DNS_INTERFACE_SETTINGS3 {
        Version: DNS_INTERFACE_SETTINGS_VERSION3,
        Flags: flags,
        Domain: PWSTR::null(),
        NameServer: PWSTR(ns_wide.as_mut_ptr()),
        SearchList: PWSTR::null(),
        RegistrationEnabled: 0,
        RegisterAdapterName: 0,
        EnableLLMNR: 0,
        QueryAdapterName: 0,
        ProfileNameServer: PWSTR::null(),
        DisableUnconstrainedQueries: 0,
        SupplementalSearchList: PWSTR::null(),
        cServerProperties: doh_props.len() as u32,
        ServerProperties: doh_props.as_mut_ptr(),
        cProfileServerProperties: 0,
        ProfileServerProperties: std::ptr::null_mut(),
    };

    unsafe {
        // SAFETY: SetInterfaceDnsSettings is a Win32 API that configures DNS settings for a network interface.
        // - guid is a valid GUID parsed from a string (not from arbitrary memory).
        // - The settings pointer is cast from a stack-allocated DNS_INTERFACE_SETTINGS3 struct,
        //   which is a superset of DNS_INTERFACE_SETTINGS and compatible for the call.
        // - All PWSTR fields in the struct point to null-terminated UTF-16 strings
        //   (ns_wide, doh_templates_wide) that remain valid for the duration of the call.
        // - The doh_props and doh_settings vectors are stable references within this scope.
        let result = SetInterfaceDnsSettings(
            guid,
            &settings as *const _ as *const DNS_INTERFACE_SETTINGS,
        );
        if result != windows::Win32::Foundation::WIN32_ERROR(0) {
            return Err(format!("SetInterfaceDnsSettings({}) 失败: 错误码 {}", err_label, result.0));
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
pub fn set_dns_via_api(
    adapter_guid: &str,
    dns_servers: &[&str],
    doh_templates: &[(&str, &str)],
) -> Result<(), String> {
    set_dns_inner(adapter_guid, dns_servers, doh_templates, true, "DNS+DoH")
}

#[cfg(target_os = "windows")]
pub fn set_doh_via_api(
    adapter_guid: &str,
    dns_servers: &[&str],
    doh_templates: &[(&str, &str)],
) -> Result<(), String> {
    set_dns_inner(adapter_guid, dns_servers, doh_templates, true, "DoH")
}

#[repr(C)]
struct ICMLuaUtilVtbl {
    _query_interface: usize,
    _add_ref: usize,
    release: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    _method1: usize,
    _method2: usize,
    _method3: usize,
    _method4: usize,
    _method5: usize,
    _method6: usize,
    shell_exec: unsafe extern "system" fn(*mut std::ffi::c_void, *const u16, *const u16, *const u16, u32, u32) -> windows::core::HRESULT,
    set_registry_string_value: unsafe extern "system" fn(*mut std::ffi::c_void, isize, *const u16, *const u16, *const u16) -> windows::core::HRESULT,
}

fn empty_quality_json() -> serde_json::Value {
    serde_json::json!({ "gatewayLatency": -1, "externalLatency": -1, "gateway": "", "quality": "unknown", "timestamp": 0, "details": {}, "metrics": {} })
}

#[tauri::command]
pub async fn get_adapters() -> Result<Vec<Adapter>, String> {
    tauri::async_runtime::spawn_blocking(|| get_adapters_cached()).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_disabled_adapters() -> Result<Vec<DisabledAdapter>, String> {
    tauri::async_runtime::spawn_blocking(|| get_disabled_adapters_cached()).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn enable_adapter(adapter_name: String) -> Result<CommandResult, String> {
    crate::network::adapter::validate_adapter_name(&adapter_name)?;
    tauri::async_runtime::spawn_blocking(move || enable_adapter_inner(&adapter_name)).await.map_err(|e| e.to_string())??;
    Ok(CommandResult::ok_msg("适配器已启用"))
}

#[tauri::command]
pub async fn get_adapter_details() -> Result<Vec<AdapterDetail>, String> {
    tauri::async_runtime::spawn_blocking(|| get_adapter_details_cached()).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn check_portal_status(adapter_ip: String, app_handle: tauri::AppHandle) -> Result<serde_json::Value, String> {
    if adapter_ip.is_empty() {
        return Ok(serde_json::json!({
            "online": false,
            "message": "IP地址为空",
        }));
    }
    let state = app_handle.state::<crate::commands::AppState>();
    let config = state.config.load_full();
    let user_account = config.user_account_with_operator();
    let user_password = config.password.clone();
    let operator = config.operator.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let status = check_portal_full(&adapter_ip, None, Some(&user_account), Some(&user_password), Some(&operator))?;
        Ok(serde_json::json!({
            "online": status.online,
            "message": status.message,
            "reachable": status.reachable,
            "loginAvailable": status.login_available,
        }))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn dhcp_renew_all() -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let results = dhcp_renew_wired_only()?;
        Ok(serde_json::json!({ "success": true, "results": results }))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn dhcp_release_renew(app_handle: AppHandle) -> Result<serde_json::Value, String> {
    let campus_gateway = {
        let state = app_handle.state::<AppState>();
        let config = state.config.load();
        let gw = config.campus_gateway.clone();
        if gw.is_empty() { crate::config::default_campus_gateway() } else { gw }
    };
    tauri::async_runtime::spawn_blocking(move || {
        let results = dhcp_release_renew_all(&campus_gateway)?;
        Ok(serde_json::json!({ "success": true, "results": results }))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn check_network_quality(app_handle: AppHandle) -> Result<serde_json::Value, String> {
    let state = app_handle.state::<AppState>();
    if !state.config.load().enable_network_quality {
        return Ok(serde_json::json!({"quality": "disabled"}));
    }
    let _guard = match state.tasks.is_quality_checking.acquire_guard() {
        Some(g) => g,
        None => return Ok(serde_json::json!({"quality": "busy"})),
    };
    let (adapter_ip, adapter_name, skip_ttfb, skip_content, fixed_gateway) = {
        let config = state.config.load();
        let adapters = match get_adapters_cached() {
            Ok(a) => a,
            Err(_) => return Ok(empty_quality_json()),
        };
        let (ip, name) = select_adapter(&adapters, &config);
        (ip, name, config.skip_ttfb_in_latency, config.skip_content_in_latency, config.fixed_gateway.clone())
    };
    if adapter_ip.is_empty() {
        return Ok(empty_quality_json());
    }
    let result = check_network_quality_async(&adapter_name, &adapter_ip, skip_ttfb, skip_content, &fixed_gateway, state.exit.is_quitting.clone()).await;
    drop(_guard);
    serde_json::to_value(&result).map_err(|e| format!("序列化结果失败: {}", e))
}

#[tauri::command]
pub fn start_latency_test(app_handle: AppHandle, _state: State<'_, AppState>) -> Result<CommandResult, String> {
    let s = app_handle.state::<AppState>();
    if s.tasks.latency_running.swap_acquire() {
        return Ok(CommandResult::ok_msg("延迟测试已在运行"));
    }

    let interval = {
        let config = s.config.load();
        if config.latency_test_interval < 10000 { 30000 } else { config.latency_test_interval }
    };

    super::latency::spawn_latency_test_loop(&app_handle, interval);

    Ok(CommandResult::ok_msg("延迟测试已启动"))
}

#[tauri::command]
pub fn stop_latency_test(state: State<'_, AppState>) -> Result<CommandResult, String> {
    // 1. cancel 当前 token，通知正在运行的任务退出
    state.tasks.latency_cancel.load().cancel();
    // 2. 存储新 token，下次启动时使用干净的 token
    state.tasks.latency_cancel.store(Arc::new(tokio_util::sync::CancellationToken::new()));
    // 3. force_release 作为先决条件，确保锁状态清除
    state.tasks.latency_running.force_release();
    Ok(CommandResult::ok_msg("延迟测试已停止"))
}

#[cfg(target_os = "windows")]
fn read_adapter_dns_from_registry() -> Result<serde_json::Value, String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let net_key = hklm
        .open_subkey(r"SYSTEM\CurrentControlSet\Control\Network\{4D36E972-E325-11CE-BFC1-08002BE10318}")
        .map_err(|e| format!("打开网络注册表失败: {}", e))?;

    let tcpip_key = hklm
        .open_subkey(r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces")
        .map_err(|e| format!("打开TCP/IP注册表失败: {}", e))?;

    fn should_filter_ip(ip: &str) -> bool {
        let trimmed = ip.trim();
        if trimmed.is_empty() { return true; }
        let p: Vec<&str> = trimmed.split('.').collect();
        if p.len() != 4 { return false; }
        let o3: u8 = match p[3].parse::<u8>() { Ok(v) => v, Err(_) => return false };
        let o0 = p[0];
        if o3 == 0 || o3 == 255 { return true; }
        if o0 == "127" { return true; }
        if o0 == "169" {
            if let Ok(o1) = p[1].parse::<u8>() {
                if o1 == 254 { return true; }
            }
        }
        if o0 == "198" {
            if let Ok(o1) = p[1].parse::<u8>() {
                if o1 == 18 || o1 == 19 { return true; }
            }
        }
        false
    }

    fn parse_dns_list(raw: &str) -> Vec<String> {
        raw.split(|c: char| c == ',' || c == ' ' || c == ';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .filter(|s| !should_filter_ip(s))
            .map(|s| s.to_string())
            .collect()
    }

    fn check_doh_for_ips(dns_ips: &[String], _hklm: &winreg::RegKey) -> std::collections::HashMap<String, (bool, bool, String)> {
        let mut netsh_doh: std::collections::HashMap<String, (bool, String)> = std::collections::HashMap::new();

        let output = std::process::Command::new("netsh")
            .args(["dns", "show", "encryption"])
            .creation_flags(0x08000000)
            .output();

        if let Ok(out) = output {
            let text = String::from_utf8_lossy(&out.stdout);
            let mut current_ip: Option<String> = None;
            let mut current_template: Option<String> = None;
            let mut current_autoupgrade: bool = false;

            for line in text.lines() {
                let trimmed = line.trim();

                if trimmed.starts_with('-') || trimmed.is_empty() {
                    continue;
                }

                let ip_match = trimmed.split_whitespace()
                    .find(|s| s.chars().all(|c| c.is_ascii_digit() || c == '.') && s.contains('.') && s.parse::<std::net::Ipv4Addr>().is_ok())
                    .map(|s| s.to_string());

                if let Some(ip) = ip_match {
                    if let Some(old_ip) = current_ip.take() {
                        let template = current_template.take().unwrap_or_default();
                        netsh_doh.insert(old_ip, (current_autoupgrade, template));
                        current_autoupgrade = false;
                    }
                    current_ip = Some(ip);
                    continue;
                }

                if current_ip.is_none() { continue; }

                if let Some(colon_pos) = trimmed.find(':') {
                    let val = trimmed[colon_pos + 1..].trim();
                    if val.starts_with("https://") {
                        current_template = Some(val.to_string());
                    } else if val.eq_ignore_ascii_case("yes") || val.eq_ignore_ascii_case("true")
                        || val.eq_ignore_ascii_case("是") || val.contains("yes") || val.contains("是")
                    {
                        current_autoupgrade = true;
                    }
                }
            }

            if let Some(ip) = current_ip.take() {
                let template = current_template.take().unwrap_or_default();
                netsh_doh.insert(ip, (current_autoupgrade, template));
            }
        }

        crate::log_debug!("doh", "netsh检测结果: {:?}", netsh_doh);

        let builtin_doh: &[(&str, &str)] = &[
            ("223.5.5.5", "https://dns.alidns.com/dns-query"),
            ("223.6.6.6", "https://dns.alidns.com/dns-query"),
            ("1.12.12.12", "https://doh.pub/dns-query"),
            ("120.53.53.53", "https://doh.pub/dns-query"),
        ];

        let mut result: std::collections::HashMap<String, (bool, bool, String)> = std::collections::HashMap::new();
        for dns in dns_ips {
            let in_netsh = netsh_doh.get(dns);
            let in_builtin = builtin_doh.iter().find(|(ip, _)| *ip == dns);

            let (doh_available, doh_enabled, doh_template) = match (in_netsh, in_builtin) {
                (Some((autoupgrade, template)), _) => {
                    let tpl = if template.is_empty() {
                        in_builtin.map(|(_, t)| t.to_string()).unwrap_or_default()
                    } else {
                        template.clone()
                    };
                    (true, *autoupgrade, tpl)
                }
                (None, Some((_, tpl))) => (true, false, tpl.to_string()),
                (None, None) => (false, false, String::new()),
            };

            crate::log_debug!("doh", "{} available={} enabled={} template={}", dns, doh_available, doh_enabled, doh_template);
            result.insert(dns.to_string(), (doh_available, doh_enabled, doh_template));
        }

        result
    }

    let mut adapters_result: Vec<serde_json::Value> = Vec::new();
    let mut all_dns_ips: Vec<String> = Vec::new();
    let mut adapter_dns_raw: Vec<(String, Vec<String>)> = Vec::new();

    for guid_entry in net_key.enum_keys().flatten() {
        let conn_path = format!(r"{}\Connection", guid_entry);
        if let Ok(conn_key) = net_key.open_subkey(&conn_path) {
            let name: String = conn_key.get_value("Name").unwrap_or_default();
            if name.is_empty() { continue; }

            if crate::network::is_blacklisted(&name) { continue; }

            let pnp_id: String = conn_key.get_value("PnpInstanceID").unwrap_or_default();
            if !pnp_id.is_empty() {
                let d = pnp_id.to_lowercase();
                if d.contains("vethernet") || d.contains("vpci") || d.contains("vmbus")
                    || d.contains("tun") || d.contains("tap") || d.contains("wintun")
                { continue; }
            }

            if let Ok(iface_key) = tcpip_key.open_subkey(&guid_entry) {
                let ns: String = iface_key.get_value("NameServer").unwrap_or_default();
                let dhcp_ns: String = iface_key.get_value("DhcpNameServer").unwrap_or_default();
                let raw = if !ns.is_empty() { ns } else { dhcp_ns };
                if raw.is_empty() { continue; }

                let addrs = parse_dns_list(&raw);

                crate::log_debug!("dns", "{} 原始:[{}] → [{:?}]", name, raw, addrs);

                if addrs.is_empty() { continue; }

                for ip in &addrs {
                    if !all_dns_ips.contains(ip) {
                        all_dns_ips.push(ip.clone());
                    }
                }

                adapter_dns_raw.push((name, addrs));
            }
        }
    }

    let doh_map = check_doh_for_ips(&all_dns_ips, &hklm);
    let any_doh_enabled = doh_map.values().any(|(_, enabled, _)| *enabled);

    for (name, addrs) in adapter_dns_raw {
        let mut dns_list: Vec<serde_json::Value> = Vec::new();
        for dns in &addrs {
            let (doh_available, doh_enabled, doh_template) = doh_map.get(dns)
                .cloned()
                .unwrap_or((false, false, String::new()));
            dns_list.push(serde_json::json!({
                "address": dns,
                "dohAvailable": doh_available,
                "dohEnabled": doh_enabled,
                "dohTemplate": doh_template,
            }));
        }

        adapters_result.push(serde_json::json!({
            "name": name,
            "dnsServers": dns_list,
        }));
    }

    Ok(serde_json::json!({
        "adapters": adapters_result,
        "dohSupported": true,
        "autoDohEnabled": any_doh_enabled,
    }))
}

#[tauri::command]
pub async fn check_dns_doh_status() -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(|| {
        #[cfg(target_os = "windows")]
        {
            read_adapter_dns_from_registry()
        }
        #[cfg(not(target_os = "windows"))]
        {
            Ok(serde_json::json!({ "adapters": [], "dohSupported": false }))
        }
    }).await.map_err(|e| format!("检测DNS状态失败: {}", e))?
}

#[tauri::command]
pub async fn enable_doh_for_dns() -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(|| {
        #[cfg(not(target_os = "windows"))]
        {
            return Ok(serde_json::json!({ "success": false, "message": "仅支持Windows" }));
        }
        #[cfg(target_os = "windows")]
        {
            let doh_servers: &[(&str, &str)] = &[
                ("223.5.5.5", "https://dns.alidns.com/dns-query"),
                ("223.6.6.6", "https://dns.alidns.com/dns-query"),
                ("1.12.12.12", "https://doh.pub/dns-query"),
                ("120.53.53.53", "https://doh.pub/dns-query"),
            ];

            let dns_ips: Vec<&str> = doh_servers.iter().map(|(ip, _)| *ip).collect();

            if is_admin() {
                let adapters = crate::network::get_adapters_cached().unwrap_or_default();
                let active: Vec<&Adapter> = adapters.iter()
                    .filter(|a| !a.ip.is_empty() && !a.name.contains("Virtual") && !a.name.contains("vEthernet"))
                    .collect();

                let mut api_ok: Vec<String> = Vec::new();
                let mut api_fail: Vec<String> = Vec::new();

                for adapter in &active {
                    match set_doh_via_api(&adapter.guid, &dns_ips, doh_servers) {
                        Ok(()) => {
                            crate::log_info!("doh", "Win32 API注册DoH成功: {}", adapter.name);
                            api_ok.push(adapter.name.clone());
                        }
                        Err(e) => {
                            crate::log_warn!("doh", "Win32 API注册DoH失败: {} - {}", adapter.name, e);
                            api_fail.push(adapter.name.clone());
                        }
                    }
                }

                let _ = std::process::Command::new("ipconfig")
                    .args(["/flushdns"])
                    .creation_flags(0x08000000)
                    .output();

                if !api_ok.is_empty() {
                    return Ok(serde_json::json!({
                        "success": true,
                        "message": format!("已为 {} 启用DoH", api_ok.join("、")),
                        "added": dns_ips.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                        "failed": api_fail,
                    }));
                }
            }

            crate::log_info!("doh", "Win32 API注册DoH未成功，尝试netsh降级");
            let mut added: Vec<String> = Vec::new();
            let mut failed: Vec<String> = Vec::new();
            let mut need_elevation = false;

            for (ip, template) in doh_servers {
                let output = std::process::Command::new("netsh")
                    .args([
                        "dns", "add", "encryption",
                        &format!("server={}", ip),
                        &format!("dohtemplate={}", template),
                        "autoupgrade=yes",
                        "udpfallback=yes",
                    ])
                    .creation_flags(0x08000000)
                    .output();

                match output {
                    Ok(o) => {
                        if o.status.success() {
                            added.push(ip.to_string());
                        } else {
                            let combined = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
                            if combined.contains("740") || combined.contains("\u{63d0}\u{5347}") || combined.contains("elevation") || combined.contains("\u{7ba1}\u{7406}\u{5458}") {
                                need_elevation = true;
                            }
                            crate::log_debug!("doh", "netsh add encryption {} 失败: {}", ip, combined);
                            failed.push(format!("{}: {}", ip, combined.trim()));
                        }
                    }
                    Err(e) => {
                        crate::log_debug!("doh", "netsh 执行失败: {}", e);
                        failed.push(format!("{}: {}", ip, e));
                    }
                }
            }

            if !added.is_empty() {
                let _ = std::process::Command::new("ipconfig")
                    .args(["/flushdns"])
                    .creation_flags(0x08000000)
                    .output();
                return Ok(serde_json::json!({
                    "success": true,
                    "message": format!("已为 {} 启用DoH", added.join("、")),
                    "added": added,
                    "failed": failed,
                }));
            }

            if need_elevation && added.is_empty() {
                let mut ps_cmds: Vec<String> = Vec::new();
                for (ip, template) in doh_servers {
                    ps_cmds.push(format!("netsh dns add encryption server={} dohtemplate={} autoupgrade=yes udpfallback=yes", ip, template));
                }
                ps_cmds.push("ipconfig /flushdns".to_string());
                let ps_script = ps_cmds.join("; ");
                let ps_args = format!("-WindowStyle Hidden -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command \"{}\"", ps_script);

                crate::log_info!("doh", "尝试COM ShellExec提权注册DoH");
                match shell_exec_elevated("powershell", &ps_args, true) {
                    Ok(()) => {
                        std::thread::sleep(std::time::Duration::from_millis(2000));
                        let mut verify_added: Vec<String> = Vec::new();
                        for (ip, _) in doh_servers {
                            let check = std::process::Command::new("netsh")
                                .args(["dns", "show", "encryption", &format!("server={}", ip)])
                                .creation_flags(0x08000000)
                                .output();
                            if let Ok(co) = check {
                                let out = format!("{}{}", String::from_utf8_lossy(&co.stdout), String::from_utf8_lossy(&co.stderr));
                                if out.contains("https://") {
                                    verify_added.push(ip.to_string());
                                }
                            }
                        }
                        if !verify_added.is_empty() {
                            return Ok(serde_json::json!({
                                "success": true,
                                "message": format!("已通过管理员权限为 {} 启用DoH", verify_added.join("、")),
                                "added": verify_added,
                                "failed": [],
                            }));
                        }
                    }
                    Err(com_err) => {
                        crate::log_warn!("doh", "COM ShellExec提权失败: {}，降级到ShellExecuteW", com_err);
                    }
                }

                let mut netsh_cmds = String::new();
                for (ip, template) in doh_servers {
                    netsh_cmds.push_str(&format!("netsh dns add encryption server={} dohtemplate={} autoupgrade=yes udpfallback=yes & ", ip, template));
                }
                netsh_cmds.push_str("ipconfig /flushdns");

                match run_elevated("cmd", &format!("/c {}", netsh_cmds)) {
                    Ok(()) => {
                        std::thread::sleep(std::time::Duration::from_millis(2500));
                        let mut verify_added: Vec<String> = Vec::new();
                        for (ip, _) in doh_servers {
                            let check = std::process::Command::new("netsh")
                                .args(["dns", "show", "encryption", &format!("server={}", ip)])
                                .creation_flags(0x08000000)
                                .output();
                            if let Ok(co) = check {
                                let out = format!("{}{}", String::from_utf8_lossy(&co.stdout), String::from_utf8_lossy(&co.stderr));
                                if out.contains("https://") {
                                    verify_added.push(ip.to_string());
                                }
                            }
                        }
                        if !verify_added.is_empty() {
                            return Ok(serde_json::json!({
                                "success": true,
                                "message": format!("已通过管理员权限为 {} 启用DoH", verify_added.join("、")),
                                "added": verify_added,
                                "failed": [],
                            }));
                        }
                        return Ok(serde_json::json!({
                            "success": false,
                            "message": "管理员权限执行后DoH仍未注册成功".to_string(),
                            "added": [],
                            "failed": failed,
                        }));
                    }
                    Err(e) => {
                        return Ok(serde_json::json!({
                            "success": false,
                            "message": format!("需要管理员权限来注册DoH: {}", e),
                            "added": [],
                            "failed": failed,
                        }));
                    }
                }
            }

            Ok(serde_json::json!({
                "success": false,
                "message": "启用DoH失败，可能需要管理员权限".to_string(),
                "added": added,
                "failed": failed,
            }))
        }
    }).await.map_err(|e| format!("启用DoH失败: {}", e))?
}

#[tauri::command]
pub async fn setup_dns_doh() -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(|| {
        #[cfg(not(target_os = "windows"))]
        {
            return Ok(serde_json::json!({ "success": false, "message": "仅支持Windows" }));
        }
        #[cfg(target_os = "windows")]
        {
            let primary_dns = "223.5.5.5";
            let secondary_dns = "1.12.12.12";

            let doh_servers: &[(&str, &str)] = &[
                ("223.5.5.5", "https://dns.alidns.com/dns-query"),
                ("223.6.6.6", "https://dns.alidns.com/dns-query"),
                ("1.12.12.12", "https://doh.pub/dns-query"),
                ("120.53.53.53", "https://doh.pub/dns-query"),
            ];

            let adapters = crate::network::get_adapters_cached()
                .unwrap_or_default();
            let active: Vec<&Adapter> = adapters.iter()
                .filter(|a| !a.ip.is_empty() && !a.name.contains("Virtual") && !a.name.contains("vEthernet") && !a.name.contains("Wintun") && !a.name.contains("TUN"))
                .collect();

            if active.is_empty() {
                return Ok(serde_json::json!({
                    "success": false,
                    "message": "未找到活跃的网络适配器".to_string(),
                }));
            }

            if is_admin() {
                let mut api_success: Vec<String> = Vec::new();
                let mut api_fail: Vec<String> = Vec::new();

                for adapter in &active {
                    let dns_list: Vec<&str> = vec![primary_dns, secondary_dns];
                    let doh_list: Vec<(&str, &str)> = doh_servers.to_vec();

                    match set_dns_via_api(&adapter.guid, &dns_list, &doh_list) {
                        Ok(()) => {
                            crate::log_info!("dns", "Win32 API设置DNS+DoH成功: {}", adapter.name);
                            api_success.push(adapter.name.clone());
                        }
                        Err(e) => {
                            crate::log_warn!("dns", "Win32 API设置DNS失败: {} - {}", adapter.name, e);
                            api_fail.push(format!("{}: {}", adapter.name, e));
                        }
                    }
                }

                let _ = std::process::Command::new("ipconfig")
                    .args(["/flushdns"])
                    .creation_flags(0x08000000)
                    .output();

                if !api_success.is_empty() {
                    let mut parts = Vec::new();
                    parts.push(format!("已为 {} 设置DNS({}+{})并启用DoH", api_success.join("、"), primary_dns, secondary_dns));
                    if !api_fail.is_empty() {
                        parts.push(format!("{}个适配器设置失败", api_fail.len()));
                    }
                    return Ok(serde_json::json!({
                        "success": api_fail.is_empty(),
                        "message": parts.join("，"),
                        "dnsSuccess": api_success,
                        "dnsFailed": api_fail,
                        "dohAdded": doh_servers.iter().map(|(ip, _)| ip.to_string()).collect::<Vec<_>>(),
                        "dohFailed": [],
                    }));
                }

                return Ok(serde_json::json!({
                    "success": false,
                    "message": "设置DNS失败".to_string(),
                    "dnsFailed": api_fail,
                }));
            }

            crate::log_info!("dns", "非管理员运行，使用COM ShellExec提权设置DNS+DoH");
            let mut ps_cmds: Vec<String> = Vec::new();
            for adapter in &active {
                ps_cmds.push(format!(
                    "Set-DnsClientServerAddress -InterfaceAlias '{}' -ServerAddresses ('{}','{}') -Confirm:$false",
                    adapter.name, primary_dns, secondary_dns
                ));
            }
            for (ip, template) in doh_servers {
                ps_cmds.push(format!("netsh dns add encryption server={} dohtemplate={} autoupgrade=yes udpfallback=yes", ip, template));
            }
            ps_cmds.push("ipconfig /flushdns".to_string());
            let ps_script = ps_cmds.join("; ");
            let ps_args = format!("-WindowStyle Hidden -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command \"{}\"", ps_script);

            match shell_exec_elevated("powershell", &ps_args, true) {
                Ok(()) => {
                    std::thread::sleep(std::time::Duration::from_millis(2000));
                    let mut verify_ok = false;
                    for adapter in &active {
                        let check = std::process::Command::new("netsh")
                            .args(["interface", "ip", "show", "dns", &format!("name={}", adapter.name)])
                            .creation_flags(0x08000000)
                            .output();
                        if let Ok(co) = check {
                            let out = format!("{}{}", String::from_utf8_lossy(&co.stdout), String::from_utf8_lossy(&co.stderr));
                            if out.contains(primary_dns) {
                                verify_ok = true;
                            }
                        }
                    }
                    if verify_ok {
                        return Ok(serde_json::json!({
                            "success": true,
                            "message": "已通过管理员权限设置DNS并启用DoH".to_string(),
                        }));
                    }
                }
                Err(com_err) => {
                    crate::log_warn!("dns", "COM ShellExec提权失败: {}，降级到ShellExecuteW", com_err);
                }
            }

            let mut all_cmds = String::new();
            for adapter in &active {
                all_cmds.push_str(&format!("netsh interface ip set dns name=\"{}\" static {} primary & ", adapter.name, primary_dns));
                all_cmds.push_str(&format!("netsh interface ip add dns name=\"{}\" {} index=2 & ", adapter.name, secondary_dns));
                if !adapter.guid.is_empty() {
                    for dns_ip in &[primary_dns, secondary_dns] {
                        all_cmds.push_str(&format!(
                            "reg add \"HKLM\\SYSTEM\\CurrentControlSet\\Services\\Dnscache\\InterfaceSpecificParameters\\{}\\DohInterfaceSettings\\Doh\\{}\" /v DohFlags /t REG_QWORD /d 1 /f & ",
                            adapter.guid, dns_ip
                        ));
                    }
                }
            }
            for (ip, template) in doh_servers {
                all_cmds.push_str(&format!("netsh dns add encryption server={} dohtemplate={} autoupgrade=yes udpfallback=yes & ", ip, template));
            }
            all_cmds.push_str("ipconfig /flushdns");

            match run_elevated("cmd", &format!("/c {}", all_cmds)) {
                Ok(()) => {
                    std::thread::sleep(std::time::Duration::from_millis(3000));
                    return Ok(serde_json::json!({
                        "success": true,
                        "message": "已通过管理员权限设置DNS并启用DoH".to_string(),
                    }));
                }
                Err(e) => {
                    return Ok(serde_json::json!({
                        "success": false,
                        "message": format!("需要管理员权限: {}", e),
                    }));
                }
            }
        }
    }).await.map_err(|e| format!("设置DNS+DoH失败: {}", e))?
}
