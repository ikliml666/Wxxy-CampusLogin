use tauri::{AppHandle, Manager, State};
use crate::network::{
    Adapter, AdapterDetail, DisabledAdapter,
    get_adapters_cached, get_adapters_force, get_disabled_adapters_cached,
    enable_adapter as enable_adapter_inner, get_adapter_details_cached,
    dhcp_renew_wired_only, dhcp_release_renew_all, dhcp_release_renew_single,
    select_adapter,
    check_network_quality_async,
};
use crate::infra::command_context::CommandContext;
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
    crate::network::adapter_cache::validate_adapter_name(&adapter_name)?;
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

    // 状态探测为只读操作：不向登录端点发送账号密码（check_portal_full 已不接受凭据）
    tauri::async_runtime::spawn_blocking(move || {
        let status = crate::auth::portal::check_portal_full(&adapter_ip, None, None, None)?;
        Ok(serde_json::json!({
            "online": status.online,
            "message": status.message,
            "reachable": status.reachable,
            "loginAvailable": status.login_available,
        }))
    }).await.map_err(|e| e.to_string())?
}

fn get_campus_gateway(state: &AppState) -> String {
    let config = state.config.load();
    let gw = config.campus_gateway.clone();
    if gw.is_empty() { crate::config::model::default_campus_gateway() } else { gw }
}

fn wrap_dhcp_result(results: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({ "success": true, "results": results })
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
        Ok(wrap_dhcp_result(results))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn dhcp_release_renew(app_handle: AppHandle) -> Result<serde_json::Value, String> {
    crate::log_info!("network", "开始DHCP续租");
    let campus_gateway = {
        let ctx = CommandContext::from_app(&app_handle);
        get_campus_gateway(ctx.state)
    };
    tauri::async_runtime::spawn_blocking(move || {
        let results = dhcp_release_renew_all(&campus_gateway).map_err(|e| {
            crate::log_error!("network", "DHCP续租失败: {}", e);
            e
        })?;
        crate::log_info!("network", "DHCP续租完成");
        Ok(wrap_dhcp_result(results))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn dhcp_release_renew_adapter(adapter_name: String, app_handle: AppHandle) -> Result<serde_json::Value, String> {
    crate::log_info!("network", "开始DHCP续租");
    crate::network::adapter_cache::validate_adapter_name(&adapter_name)?;
    let campus_gateway = {
        let ctx = CommandContext::from_app(&app_handle);
        get_campus_gateway(ctx.state)
    };
    tauri::async_runtime::spawn_blocking(move || {
        let result = dhcp_release_renew_single(&adapter_name, &campus_gateway)?;
        crate::log_info!("network", "DHCP续租完成");
        Ok(wrap_dhcp_result(vec![result]))
    }).await.map_err(|e| {
        crate::log_error!("network", "DHCP续租失败: {}", e);
        e.to_string()
    })?
}

#[tauri::command]
pub async fn check_network_quality(app_handle: AppHandle) -> Result<serde_json::Value, String> {
    crate::log_info!("network", "开始网络质量检测");
    let state = CommandContext::from_app(&app_handle);
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
    serde_json::to_value(&result).map_err(|e| format!("序列化结果失败: {e}"))
}

#[tauri::command]
pub fn start_latency_test(app_handle: AppHandle, _state: State<'_, AppState>) -> Result<CommandResult, String> {
    let s = CommandContext::from_app(&app_handle);
    let interval = {
        let config = s.config.load();
        if config.latency_test_interval < 10000 { 30000 } else { config.latency_test_interval }
    };

    if crate::monitor::latency::spawn_latency_test_loop(&app_handle, interval).is_err() {
        return Ok(CommandResult::ok_msg("延迟测试已在运行"));
    }

    // 持久化开关，避免重启后 enableLatencyTest 丢失（历史缺陷：仅前端本地更新，重启即回退）
    let cfg = s.config.update(|c| c.enable_latency_test = true);
    if let Err(e) = super::config_cmd::save_config_to_disk_encrypted(&app_handle, &cfg) {
        crate::log_warn!("network", "保存延迟测试开关失败: {}", e);
    }

    Ok(CommandResult::ok_msg("延迟测试已启动"))
}

#[tauri::command]
pub fn stop_latency_test(app_handle: AppHandle, state: State<'_, AppState>) -> Result<CommandResult, String> {
    state.task_manager.cancel("latency_test");

    let s = CommandContext::from_app(&app_handle);
    let cfg = s.config.update(|c| c.enable_latency_test = false);
    if let Err(e) = super::config_cmd::save_config_to_disk_encrypted(&app_handle, &cfg) {
        crate::log_warn!("network", "保存延迟测试开关失败: {}", e);
    }

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
    }).await.map_err(|e| format!("检测DNS状态失败: {e}"))?
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
                .filter(|a| !a.ip.is_empty() && !crate::network::is_blacklisted(&a.name))
                .collect();

            if active.is_empty() {
                return Ok(serde_json::json!({
                    "success": false,
                    "message": "未找到活跃的网络适配器".to_string(),
                }));
            }

            if elevation::is_admin() {
                return Ok(crate::network::dns_setup::setup_dns_doh_admin());
            }

            crate::log_info!("dns", "非管理员运行，通过 --helper 提权设置DNS+DoH");
            let result_path = crate::platform::helper_spawn::unique_result_path();
            match crate::platform::helper_spawn::spawn_elevated_helper(
                "dns",
                &[],
                &result_path,
                std::time::Duration::from_secs(30),
            ) {
                Ok(v) => {
                    // 把 helper details（完整 DNS 设置明细 dnsSuccess/dnsFailed/dohAdded/dohFailed）
                    // 提升到顶层，保证与管理员路径返回结构一致
                    if let Some(details) = v.get("details").and_then(|d| d.as_object()) {
                        let mut merged = v.clone();
                        if let Some(obj) = merged.as_object_mut() {
                            for (k, val) in details {
                                if k != "success" && k != "message" && !obj.contains_key(k) {
                                    obj.insert(k.clone(), val.clone());
                                }
                            }
                        }
                        return Ok(merged);
                    }
                    Ok(v)
                }
                Err(e) => {
                    crate::log_warn!("dns", "helper提权设置DNS失败: {}", e);
                    Ok(serde_json::json!({
                        "success": false,
                        "message": format!("需要管理员权限: {}", e),
                    }))
                }
            }
        }
    }).await.map_err(|e| format!("设置DNS+DoH失败: {e}"))?
}
