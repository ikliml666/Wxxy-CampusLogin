use tauri::{AppHandle, Manager, State};
use std::sync::Arc;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use crate::network::{
    Adapter, AdapterDetail, DisabledAdapter,
    get_adapters_cached, get_adapters_force, get_disabled_adapters_cached,
    enable_adapter as enable_adapter_inner, get_adapter_details_cached,
    dhcp_renew_wired_only, dhcp_release_renew_all, dhcp_release_renew_single,
    select_adapter,
    check_network_quality_async,
};
use crate::infra::state::{AppState, CommandResult};
use crate::platform::elevation;
use crate::platform::dns_config;

fn empty_quality_json(quality: &str) -> serde_json::Value {
    serde_json::json!({ "gatewayLatency": -1, "externalLatency": -1, "averageExternalLatency": -1, "gateway": "", "quality": quality, "timestamp": 0, "details": {}, "metrics": {} })
}

#[tauri::command]
pub async fn get_adapters(force: Option<bool>) -> Result<Vec<Adapter>, String> {
    let f = force.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || {
        if f { get_adapters_force() } else { get_adapters_cached() }
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_disabled_adapters() -> Result<Vec<DisabledAdapter>, String> {
    tauri::async_runtime::spawn_blocking(get_disabled_adapters_cached).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn enable_adapter(adapter_name: String) -> Result<CommandResult, String> {
    crate::log_info!("network", "启用适配器: {}", adapter_name);
    crate::network::adapter::validate_adapter_name(&adapter_name)?;
    let adapter_name_log = adapter_name.clone();
    tauri::async_runtime::spawn_blocking(move || enable_adapter_inner(&adapter_name)).await.map_err(|e| e.to_string())??;
    crate::log_info!("network", "适配器启用成功: {}", adapter_name_log);
    Ok(CommandResult::ok_msg("适配器已启用"))
}

#[tauri::command]
pub async fn get_adapter_details() -> Result<Vec<AdapterDetail>, String> {
    tauri::async_runtime::spawn_blocking(get_adapter_details_cached).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn check_campus_status(app_handle: AppHandle) -> Result<serde_json::Value, String> {
    let state = app_handle.state::<crate::infra::state::AppState>();
    let config = state.config.load_full();
    let enable_network_name_check = config.enable_network_name_check;
    let required_network_name = config.required_network_name.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let adapters = crate::network::get_adapters_force().map_err(|e| e.to_string())?;
        Ok::<_, String>(crate::monitor::watcher::check_campus_network(&config, &adapters))
    }).await.map_err(|e| e.to_string())??;

    Ok(serde_json::json!({
        "onCampusNetwork": result.on_campus,
        "currentSsid": result.current_ssid,
        "campusMessage": result.message,
        "enableNetworkNameCheck": enable_network_name_check,
        "requiredNetworkName": required_network_name,
        "campusWifi": result.wifi,
        "campusWired": result.wired,
    }))
}

#[tauri::command]
pub async fn check_portal_status(adapter_ip: String, app_handle: tauri::AppHandle) -> Result<serde_json::Value, String> {
    if adapter_ip.is_empty() {
        return Ok(serde_json::json!({
            "online": false,
            "message": "IP地址为空",
        }));
    }
    let state = app_handle.state::<crate::infra::state::AppState>();

    // 注销保护期内，直接返回离线状态，避免 Portal 服务器延迟导致误判为在线
    let protected_until = state.network.load().logout_protected_until;
    if std::time::Instant::now() < protected_until {
        crate::log_debug!("portal", "注销保护期内，check_portal_status 返回离线");
        return Ok(serde_json::json!({
            "online": false,
            "message": "已注销",
        }));
    }

    let config = state.config.load_full();
    let user_account = config.user_account_with_operator();
    let user_password = config.password.clone();
    let operator = config.operator.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let status = crate::auth::portal::check_portal_full(&adapter_ip, None, Some(&user_account), Some(&user_password), Some(&operator))?;
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
    crate::log_info!("network", "开始DHCP续租");
    tauri::async_runtime::spawn_blocking(move || {
        let results = dhcp_renew_wired_only().map_err(|e| {
            crate::log_error!("network", "DHCP续租失败: {}", e);
            e
        })?;
        crate::log_info!("network", "DHCP续租完成");
        Ok(serde_json::json!({ "success": true, "results": results }))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn dhcp_release_renew(app_handle: AppHandle) -> Result<serde_json::Value, String> {
    crate::log_info!("network", "开始DHCP续租");
    let campus_gateway = {
        let state = app_handle.state::<AppState>();
        let config = state.config.load();
        let gw = config.campus_gateway.clone();
        if gw.is_empty() { crate::config::model::default_campus_gateway() } else { gw }
    };
    tauri::async_runtime::spawn_blocking(move || {
        let results = dhcp_release_renew_all(&campus_gateway).map_err(|e| {
            crate::log_error!("network", "DHCP续租失败: {}", e);
            e
        })?;
        crate::log_info!("network", "DHCP续租完成");
        Ok(serde_json::json!({ "success": true, "results": results }))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn dhcp_release_renew_adapter(adapter_name: String, app_handle: AppHandle) -> Result<serde_json::Value, String> {
    crate::log_info!("network", "开始DHCP续租");
    crate::network::adapter::validate_adapter_name(&adapter_name)?;
    let campus_gateway = {
        let state = app_handle.state::<AppState>();
        let config = state.config.load();
        let gw = config.campus_gateway.clone();
        if gw.is_empty() { crate::config::model::default_campus_gateway() } else { gw }
    };
    tauri::async_runtime::spawn_blocking(move || {
        dhcp_release_renew_single(&adapter_name, &campus_gateway)
    }).await.map_err(|e| {
        crate::log_error!("network", "DHCP续租失败: {}", e);
        e.to_string()
    })?
}

#[tauri::command]
pub async fn check_network_quality(app_handle: AppHandle) -> Result<serde_json::Value, String> {
    crate::log_info!("network", "开始网络质量检测");
    let state = app_handle.state::<AppState>();
    if !state.config.load().enable_network_quality {
        return Ok(empty_quality_json("disabled"));
    }
    let _guard = match state.tasks.is_quality_checking.try_acquire() {
        Some(g) => g,
        None => return Ok(empty_quality_json("busy")),
    };
    let (adapter_ip, adapter_name, skip_ttfb, skip_content, fixed_gateway) = {
        let config = state.config.load();
        let adapters = match get_adapters_cached() {
            Ok(a) => a,
            Err(_) => return Ok(empty_quality_json("unknown")),
        };
        let (ip, name) = select_adapter(&adapters, &config);
        (ip, name, config.skip_ttfb_in_latency, config.skip_content_in_latency, config.fixed_gateway.clone())
    };
    if adapter_ip.is_empty() {
        return Ok(empty_quality_json("unknown"));
    }
    let result = check_network_quality_async(&adapter_name, &adapter_ip, skip_ttfb, skip_content, &fixed_gateway, state.exit.is_quitting.clone(), None).await;
    crate::log_info!("network", "网络质量检测完成");
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

    crate::monitor::latency::spawn_latency_test_loop(&app_handle, interval);

    Ok(CommandResult::ok_msg("延迟测试已启动"))
}

#[tauri::command]
pub fn stop_latency_test(state: State<'_, AppState>) -> Result<CommandResult, String> {
    state.tasks.latency_cancel.load().cancel();
    state.tasks.latency_cancel.store(Arc::new(tokio_util::sync::CancellationToken::new()));
    state.tasks.latency_running.force_release();
    Ok(CommandResult::ok_msg("延迟测试已停止"))
}

#[tauri::command]
pub async fn check_dns_doh_status() -> Result<serde_json::Value, String> {
    crate::log_debug!("dns", "检测DNS/DoH状态");
    tauri::async_runtime::spawn_blocking(|| {
        #[cfg(target_os = "windows")]
        {
            dns_config::read_adapter_dns_from_registry()
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
            let dns_ips: Vec<&str> = dns_config::DOH_SERVERS.iter().map(|(ip, _)| *ip).collect();

            if elevation::is_admin() {
                let adapters = crate::network::get_adapters_cached().unwrap_or_default();
                let active: Vec<&Adapter> = adapters.iter()
                    .filter(|a| !a.ip.is_empty() && !a.name.contains("Virtual") && !a.name.contains("vEthernet"))
                    .collect();

                let mut api_ok: Vec<String> = Vec::new();
                let mut api_fail: Vec<String> = Vec::new();

                for adapter in &active {
                    match dns_config::set_doh_via_api(&adapter.guid, &dns_ips, dns_config::DOH_SERVERS) {
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

            for (ip, template) in dns_config::DOH_SERVERS {
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
                            if combined.contains("740") || combined.contains("elevation") || combined.contains("elevated") || combined.contains("\u{63d0}\u{5347}") || combined.contains("\u{7ba1}\u{7406}\u{5458}") {
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
                for (ip, template) in dns_config::DOH_SERVERS {
                    ps_cmds.push(format!("netsh dns add encryption server={} dohtemplate={} autoupgrade=yes udpfallback=yes", ip, template));
                }
                ps_cmds.push("ipconfig /flushdns".to_string());
                let ps_script = ps_cmds.join("; ");
                let ps_args = format!("-WindowStyle Hidden -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command \"{}\"", ps_script);

                crate::log_info!("doh", "尝试COM ShellExec提权注册DoH");
                match elevation::shell_exec_elevated("powershell", &ps_args, true) {
                    Ok(()) => {
                        std::thread::sleep(std::time::Duration::from_millis(2000));
                        let mut verify_added: Vec<String> = Vec::new();
                        for (ip, _) in dns_config::DOH_SERVERS {
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
                for (ip, template) in dns_config::DOH_SERVERS {
                    netsh_cmds.push_str(&format!("netsh dns add encryption server={} dohtemplate={} autoupgrade=yes udpfallback=yes & ", ip, template));
                }
                netsh_cmds.push_str("ipconfig /flushdns");

                match elevation::run_elevated("cmd", &format!("/c {}", netsh_cmds)) {
                    Ok(()) => {
                        std::thread::sleep(std::time::Duration::from_millis(1500));
                        let mut verify_added: Vec<String> = Vec::new();
                        for (ip, _) in dns_config::DOH_SERVERS {
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
    crate::log_info!("dns", "开始一键设置DNS+DoH");
    tauri::async_runtime::spawn_blocking(|| {
        #[cfg(not(target_os = "windows"))]
        {
            return Ok(serde_json::json!({ "success": false, "message": "仅支持Windows" }));
        }
        #[cfg(target_os = "windows")]
        {
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

            if elevation::is_admin() {
                let mut api_success: Vec<String> = Vec::new();
                let mut api_fail: Vec<String> = Vec::new();

                for adapter in &active {
                    let dns_list: Vec<&str> = vec![dns_config::PRIMARY_DNS, dns_config::SECONDARY_DNS];
                    let doh_list: Vec<(&str, &str)> = dns_config::DOH_SERVERS.to_vec();

                    // WiFi 适配器：先清除适配器级 DNS，再设置配置文件级 DNS
                    // 有线适配器：保持适配器级 DNS
                    if adapter.wireless {
                        // 清除适配器级 DNS，确保配置文件级 DNS 生效
                        if let Err(e) = dns_config::clear_adapter_dns_via_api(&adapter.guid) {
                            crate::log_warn!("dns", "清除适配器级DNS失败: {} - {}", adapter.name, e);
                        }
                        match dns_config::set_profile_dns_via_api(&adapter.guid, &dns_list, &doh_list) {
                            Ok(()) => {
                                crate::log_info!("dns", "配置文件级DNS+DoH设置成功: {}", adapter.name);
                                api_success.push(adapter.name.clone());
                            }
                            Err(e) => {
                                crate::log_warn!("dns", "配置文件级DNS设置失败: {} - {}, 降级到适配器级", adapter.name, e);
                                // 降级到适配器级 DNS
                                match dns_config::set_dns_via_api(&adapter.guid, &dns_list, &doh_list) {
                                    Ok(()) => {
                                        crate::log_info!("dns", "降级适配器级DNS+DoH成功: {}", adapter.name);
                                        api_success.push(adapter.name.clone());
                                    }
                                    Err(e2) => {
                                        crate::log_warn!("dns", "适配器级DNS也失败: {} - {}", adapter.name, e2);
                                        api_fail.push(format!("{}: {}", adapter.name, e2));
                                    }
                                }
                            }
                        }
                    } else {
                        match dns_config::set_dns_via_api(&adapter.guid, &dns_list, &doh_list) {
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
                }

                let _ = std::process::Command::new("ipconfig")
                    .args(["/flushdns"])
                    .creation_flags(0x08000000)
                    .output();

                if !api_success.is_empty() {
                    let mut parts = Vec::new();
                    parts.push(format!("已为 {} 设置DNS({}+{})并启用DoH", api_success.join("、"), dns_config::PRIMARY_DNS, dns_config::SECONDARY_DNS));
                    if !api_fail.is_empty() {
                        parts.push(format!("{}个适配器设置失败", api_fail.len()));
                    }
                    return Ok(serde_json::json!({
                        "success": api_fail.is_empty(),
                        "message": parts.join("，"),
                        "dnsSuccess": api_success,
                        "dnsFailed": api_fail,
                        "dohAdded": dns_config::DOH_SERVERS.iter().map(|(ip, _)| ip.to_string()).collect::<Vec<_>>(),
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
                if adapter.wireless {
                    // WiFi: 清除适配器级 DNS，设置配置文件级 DNS
                    ps_cmds.push(format!(
                        "netsh interface ip set dns name='{}' dhcp",
                        crate::network::adapter::escape_ps_single_quote(&adapter.name)
                    ));
                    // 设置 ProfileNameServer（通过注册表）
                    if !adapter.guid.is_empty() {
                        ps_cmds.push(format!(
                            "Set-ItemProperty -Path 'HKLM:\\SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters\\Interfaces\\{}' -Name 'ProfileNameServer' -Value '{},{}'",
                            adapter.guid, dns_config::PRIMARY_DNS, dns_config::SECONDARY_DNS
                        ));
                    }
                } else {
                    ps_cmds.push(format!(
                        "Set-DnsClientServerAddress -InterfaceAlias '{}' -ServerAddresses ('{}','{}') -Confirm:$false",
                        crate::network::adapter::escape_ps_single_quote(&adapter.name), dns_config::PRIMARY_DNS, dns_config::SECONDARY_DNS
                    ));
                }
            }
            for (ip, template) in dns_config::DOH_SERVERS {
                ps_cmds.push(format!("netsh dns add encryption server={} dohtemplate={} autoupgrade=yes udpfallback=yes", ip, template));
            }
            ps_cmds.push("ipconfig /flushdns".to_string());
            let ps_script = ps_cmds.join("; ");
            let ps_args = format!("-WindowStyle Hidden -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command \"{}\"", ps_script);

            match elevation::shell_exec_elevated("powershell", &ps_args, true) {
                Ok(()) => {
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                    let mut verify_ok = false;
                    for adapter in &active {
                        let check = std::process::Command::new("netsh")
                            .args(["interface", "ip", "show", "dns", &format!("name={}", adapter.name)])
                            .creation_flags(0x08000000)
                            .output();
                        if let Ok(co) = check {
                            let out = format!("{}{}", String::from_utf8_lossy(&co.stdout), String::from_utf8_lossy(&co.stderr));
                            if out.contains(dns_config::PRIMARY_DNS) {
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

            // 校验适配器名称不含 cmd 元字符，防止命令注入
            for adapter in &active {
                if adapter.name.chars().any(|c| matches!(c, '"' | '&' | '|' | '<' | '>' | '^' | '%')) {
                    return Ok(serde_json::json!({
                        "success": false,
                        "message": format!("适配器名称含特殊字符，无法通过cmd设置DNS: {}", adapter.name)
                    }));
                }
            }

            let mut all_cmds = String::new();
            for adapter in &active {
                if adapter.wireless {
                    // WiFi: 清除适配器级 DNS (恢复 DHCP)，设置 ProfileNameServer
                    all_cmds.push_str(&format!("netsh interface ip set dns name=\"{}\" dhcp & ", adapter.name));
                    if !adapter.guid.is_empty() {
                        all_cmds.push_str(&format!(
                            "reg add \"HKLM\\SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters\\Interfaces\\{}\" /v ProfileNameServer /t REG_SZ /d \"{},{}\" /f & ",
                            adapter.guid, dns_config::PRIMARY_DNS, dns_config::SECONDARY_DNS
                        ));
                    }
                } else {
                    all_cmds.push_str(&format!("netsh interface ip set dns name=\"{}\" static {} primary & ", adapter.name, dns_config::PRIMARY_DNS));
                    all_cmds.push_str(&format!("netsh interface ip add dns name=\"{}\" {} index=2 & ", adapter.name, dns_config::SECONDARY_DNS));
                    if !adapter.guid.is_empty() {
                        for dns_ip in &[dns_config::PRIMARY_DNS, dns_config::SECONDARY_DNS] {
                            all_cmds.push_str(&format!(
                                "reg add \"HKLM\\SYSTEM\\CurrentControlSet\\Services\\Dnscache\\InterfaceSpecificParameters\\{}\\DohInterfaceSettings\\Doh\\{}\" /v DohFlags /t REG_QWORD /d 1 /f & ",
                                adapter.guid, dns_ip
                            ));
                        }
                    }
                }
            }
            for (ip, template) in dns_config::DOH_SERVERS {
                all_cmds.push_str(&format!("netsh dns add encryption server={} dohtemplate={} autoupgrade=yes udpfallback=yes & ", ip, template));
            }
            all_cmds.push_str("ipconfig /flushdns");

            match elevation::run_elevated("cmd", &format!("/c {}", all_cmds)) {
                Ok(()) => {
                    std::thread::sleep(std::time::Duration::from_millis(2000));
                    Ok(serde_json::json!({
                        "success": true,
                        "message": "已通过管理员权限设置DNS并启用DoH".to_string(),
                    }))
                }
                Err(e) => {
                    Ok(serde_json::json!({
                        "success": false,
                        "message": format!("需要管理员权限: {}", e),
                    }))
                }
            }
        }
    }).await.map_err(|e| format!("设置DNS+DoH失败: {}", e))?
}
