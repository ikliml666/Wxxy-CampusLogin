use tauri::{AppHandle, Emitter, Manager, State};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use crate::network::{
    Adapter, get_adapters_cached, get_adapters_force,
    check_portal_full, check_network_quality_async,
};
use super::state::{AppState, CommandResult};
use super::auto_exit::start_auto_exit;
use super::auto_login::{try_auto_login_on_preparation, try_disconnect_reconnect, run_auto_login_on_start};
use super::latency::{notify_network_quality_change, spawn_latency_test_loop};

enum PortalCheckResult {
    Success {
        online: bool,
        message: String,
        reachable: bool,
        login_available: bool,
    },
    Error,
    NotFound,
}

impl PortalCheckResult {
    fn online(&self) -> bool {
        match self {
            PortalCheckResult::Success { online, .. } => *online,
            _ => false,
        }
    }

    fn message(&self) -> &str {
        match self {
            PortalCheckResult::Success { message, .. } => message,
            PortalCheckResult::Error => "检测失败",
            PortalCheckResult::NotFound => "未找到主适配器",
        }
    }

    fn reachable(&self) -> bool {
        match self {
            PortalCheckResult::Success { reachable, .. } => *reachable,
            _ => false,
        }
    }

    fn login_available(&self) -> bool {
        match self {
            PortalCheckResult::Success { login_available, .. } => *login_available,
            _ => false,
        }
    }
}

fn check_adapter_portal(
    adapter: &Adapter,
    user_account: &str,
    user_password: &str,
    app_handle: &AppHandle,
) -> PortalCheckResult {
    match check_portal_full(&adapter.ip, Some(&adapter.name), Some(user_account), Some(user_password)) {
        Ok(ps) => PortalCheckResult::Success {
            online: ps.online,
            message: ps.message,
            reachable: ps.reachable,
            login_available: ps.login_available,
        },
        Err(e) => {
            crate::log_warn!("background", "{} Portal检测异常: {}", adapter.name, e);
            let _ = app_handle.emit("login-log", serde_json::json!({
                "message": format!("{} Portal检测异常: {}", adapter.name, e),
                "type": "error"
            }));
            PortalCheckResult::Error
        }
    }
}

fn adapter_status_entry(name: &str, ip: &str, wireless: bool, online: bool, message: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name, "ip": ip, "wireless": wireless,
        "online": online, "message": message
    })
}

fn adapter_disabled_entry(name: &str) -> serde_json::Value {
    adapter_status_entry(name, "", false, false, "适配器已禁用或未找到")
}

fn adapter_disconnected_entry(name: &str, wireless: bool) -> serde_json::Value {
    adapter_status_entry(name, "", wireless, false, "适配器未连接")
}

fn build_adapter_details(
    adapter1_name: &str,
    adapter1_message: &str,
    adapter2_name: &str,
    adapter2_message: Option<&str>,
    dual_adapter: bool,
) -> String {
    let mut details = vec![format!("{}: {}", adapter1_name, adapter1_message)];
    if let Some(msg) = adapter2_message {
        if dual_adapter && !adapter2_name.is_empty() {
            details.push(format!("{}: {}", adapter2_name, msg));
        }
    }
    details.join(", ")
}

fn handle_status_change(
    prev_online: bool,
    current_online: bool,
    reachable: bool,
    login_available: bool,
    adapter1_name: &str,
    adapter1_message: &str,
    adapter2_name: &str,
    adapter2_message: Option<&str>,
    config: &crate::config::Config,
    app_handle: &AppHandle,
) {
    let adapter_details = build_adapter_details(
        adapter1_name, adapter1_message,
        adapter2_name, adapter2_message,
        config.dual_adapter,
    );

    if current_online != prev_online {
        crate::log_info!("background", "状态变更: {} → {} [{}]",
            if prev_online { "在线" } else { "离线" },
            if current_online { "在线" } else { "离线" },
            adapter_details);

        if !current_online && config.enable_notification {
            super::system::emit_notification(app_handle, "网络状态变更", &adapter_details);
        }
    } else {
        crate::log_debug!("background", "检测结果: online={}, reachable={}, loginAvailable={}, [{}]", current_online, reachable, login_available, adapter_details);
    }
}

fn emit_background_check_result(
    app_handle: &AppHandle,
    online: bool,
    reachable: bool,
    login_available: bool,
    message: &str,
    adapter1_name: &str,
    adapter2_name: &str,
    secondary_online: Option<bool>,
    secondary_message: &str,
    dual_adapter: bool,
) {
    if let Err(e) = app_handle.emit("background-check-result", serde_json::json!({
        "serverAvailable": reachable,
        "loginAvailable": login_available,
        "online": online,
        "message": message,
        "adapter1Name": adapter1_name,
        "adapter2Name": if dual_adapter { adapter2_name } else { "" },
        "secondaryOnline": secondary_online,
        "secondaryMessage": secondary_message,
        "timestamp": chrono::Utc::now().timestamp_millis(),
        "checkCount": 0,
    })) {
        crate::log_warn!("background", "发送后台检测结果失败: {}", e);
    }
}

fn update_network_state(
    state: &AppState,
    online: bool,
    secondary_online: Option<bool>,
    reachable: bool,
    app_handle: &AppHandle,
) {
    state.network.server_available.store(reachable, Ordering::Release);

    let any_online = online || secondary_online == Some(true);
    state.network.was_online.store(any_online, Ordering::Release);
    if any_online {
        state.network.disconnect_reconnect_count.store(0, Ordering::Release);
    }

    if reachable && !state.network.has_logged_online.load(Ordering::Acquire) && online {
        state.network.has_logged_online.store(true, Ordering::Release);
        if state.config.load().auto_exit_on_online {
            start_auto_exit(app_handle, state);
        }
    }
}

fn run_background_check_blocking(app_handle: &AppHandle, state: &AppState) -> Option<(String, String)> {
    if state.tasks.is_checking.swap_acquire() || state.exit.is_quitting.load(Ordering::Acquire) {
        return None;
    }

    let _check_guard = state.tasks.is_checking.release_guard();

    let config = state.config.load_full();
    let user_account = config.user_account_with_operator();
    let user_password = config.password.clone();
    crate::log_debug!("background", "开始后台检测");

    let adapters = match get_adapters_cached() {
        Ok(a) if !a.is_empty() => a,
        _ => match get_adapters_force() {
            Ok(a) => a,
            Err(_) => return None,
        }
    };

    let (adapter1_name, adapter2_name) = crate::network::resolve_adapter_names(&adapters, &config);

    let a1 = adapters.iter().find(|a| a.name == adapter1_name && !a.ip.is_empty());
    let a2 = if config.dual_adapter && !adapter2_name.is_empty() {
        adapters.iter().find(|a| a.name == adapter2_name && !a.ip.is_empty())
    } else {
        None
    };

    let primary_result = match a1 {
        Some(adapter) => check_adapter_portal(adapter, &user_account, &user_password, app_handle),
        None => PortalCheckResult::NotFound,
    };

    let secondary_result = a2.map(|a| check_adapter_portal(a, &user_account, &user_password, app_handle));

    let online = primary_result.online();
    let reachable = primary_result.reachable();
    let login_available = primary_result.login_available();
    let message = primary_result.message();
    let prev_online = state.network.was_online.load(Ordering::Acquire);

    let (secondary_online, secondary_message) = match &secondary_result {
        Some(PortalCheckResult::Success { online, message, .. }) => (Some(*online), message.clone()),
        _ => (None, String::new()),
    };

    handle_status_change(
        prev_online, online, reachable, login_available,
        &adapter1_name, message,
        &adapter2_name, secondary_result.as_ref().and_then(|r| {
            if let PortalCheckResult::Success { message, .. } = r { Some(message.as_str()) } else { None }
        }),
        &config, app_handle,
    );

    emit_background_check_result(
        app_handle, online, reachable, login_available, message,
        &adapter1_name, &adapter2_name,
        secondary_online, &secondary_message, config.dual_adapter,
    );

    try_auto_login_on_preparation(app_handle, state, login_available, online, &config);

    try_disconnect_reconnect(
        app_handle, state, online, secondary_online,
        a1, &adapter1_name, &adapter2_name,
        reachable, login_available, &config,
    );

    update_network_state(state, online, secondary_online, reachable, app_handle);

    if online && a1.is_some() && config.enable_network_quality {
        if let Some(a1_ref) = a1 {
            return Some((a1_ref.name.clone(), a1_ref.ip.clone()));
        }
    }

    None
}

pub async fn run_background_check(app_handle: &AppHandle, _state: &AppState) {
    let app_h = app_handle.clone();
    let quality_info = tauri::async_runtime::spawn_blocking(move || {
        let s = app_h.state::<AppState>();
        run_background_check_blocking(&app_h, &s)
    }).await.unwrap_or_else(|e| {
        crate::log_error!("background", "后台检测异常: {}", e);
        None
    });

    if let Some((adapter_name, adapter_ip)) = quality_info {
        let s = app_handle.state::<AppState>();
        let (skip_ttfb, skip_content, fixed_gateway) = {
            let cfg = s.config.load();
            (cfg.skip_ttfb_in_latency, cfg.skip_content_in_latency, cfg.fixed_gateway.clone())
        };
        if s.tasks.is_quality_checking.try_acquire().is_none() {
            return;
        }
        let _guard = s.tasks.is_quality_checking.release_guard();
        let quality = check_network_quality_async(&adapter_name, &adapter_ip, skip_ttfb, skip_content, &fixed_gateway, s.exit.is_quitting.clone()).await;
        drop(_guard);
        let enable_notification = s.config.load().enable_notification;
        let quality_val = serde_json::to_value(&quality).unwrap_or_default();
        if let Err(e) = app_handle.emit("network-quality-result", &quality_val) {
            crate::log_warn!("background", "发送网络质量结果失败: {}", e);
        }
        notify_network_quality_change(app_handle, &s, &quality_val, enable_notification);
    }
}

fn start_background_check_inner(app_handle: &AppHandle, state: &AppState) -> Result<CommandResult, String> {
    if state.tasks.background_running.swap_acquire() {
        return Ok(CommandResult::ok_msg("后台检测已在运行"));
    }

    let (interval, cfg) = {
        let current = state.config.load();
        let mut cfg = current.as_ref().clone();
        cfg.enable_background_check = true;
        if cfg.background_check_interval < 10000 {
            cfg.background_check_interval = 60000;
        }
        let interval = cfg.background_check_interval;
        state.config.store(Arc::new(cfg.clone()));
        (interval, cfg)
    };

    if let Err(e) = super::config_cmd::save_config_to_disk(app_handle, &cfg) {
        crate::log_warn!("background", "保存后台检测配置失败: {}", e);
    }

    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let s = app_h.state::<AppState>();
        run_background_check(&app_h, &s).await;

        let mut interval_timer = tokio::time::interval(Duration::from_millis(interval));
        loop {
            interval_timer.tick().await;
            let s = app_h.state::<AppState>();
            if !s.tasks.background_running.is_active() || s.exit.is_quitting.load(Ordering::Acquire) {
                break;
            }
            run_background_check(&app_h, &s).await;
        }
    });

    Ok(CommandResult::ok_msg("后台检测已启动"))
}

#[tauri::command]
pub fn start_background_check(app_handle: AppHandle, _state: State<'_, AppState>) -> Result<CommandResult, String> {
    let s = app_handle.state::<AppState>();
    start_background_check_inner(&app_handle, &s)
}

#[tauri::command]
pub fn stop_background_check(_state: State<'_, AppState>, app_handle: AppHandle) -> Result<CommandResult, String> {
    let s = app_handle.state::<AppState>();
    s.tasks.background_running.force_release();
    let cfg = {
        let current = s.config.load();
        let mut cfg = current.as_ref().clone();
        cfg.enable_background_check = false;
        s.config.store(Arc::new(cfg.clone()));
        cfg
    };
    if let Err(e) = super::config_cmd::save_config_to_disk(&app_handle, &cfg) {
        crate::log_warn!("background", "保存停止检测配置失败: {}", e);
    }
    Ok(CommandResult::ok_msg("后台检测已停止"))
}

#[tauri::command]
pub fn trigger_background_check(_state: State<'_, AppState>, app_handle: AppHandle) -> Result<CommandResult, String> {
    let s = app_handle.state::<AppState>();
    if s.tasks.is_checking.is_active() {
        return Ok(CommandResult::err("检测正在进行中"));
    }
    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let s = app_h.state::<AppState>();
        run_background_check(&app_h, &s).await;
    });
    Ok(CommandResult::ok_msg("已触发后台检测"))
}

#[tauri::command]
pub async fn get_background_status(app_handle: AppHandle) -> Result<serde_json::Value, String> {
    let state = app_handle.state::<AppState>();
    let config = state.config.load_full();
    let running = state.tasks.background_running.is_active();
    let server_avail = state.network.server_available.load(Ordering::Acquire);

    let adapter_statuses = {
        let mut adapter_statuses = Vec::new();

        if let Ok(adapters) = get_adapters_cached() {
            let (adapter1_name, adapter2_name) = crate::network::resolve_adapter_names(&adapters, &config);

            if let Some(a1) = adapters.iter().find(|a| a.name == adapter1_name) {
                if a1.ip.is_empty() {
                    adapter_statuses.push(adapter_disconnected_entry(&adapter1_name, a1.wireless));
                } else {
                    adapter_statuses.push(adapter_status_entry(&adapter1_name, &a1.ip, a1.wireless, false, "待检测"));
                }
            } else if !adapter1_name.is_empty() {
                adapter_statuses.push(adapter_disabled_entry(&adapter1_name));
            }

            if config.dual_adapter && !adapter2_name.is_empty() {
                if let Some(a2) = adapters.iter().find(|a| a.name == adapter2_name) {
                    if a2.ip.is_empty() {
                        adapter_statuses.push(adapter_disconnected_entry(&adapter2_name, a2.wireless));
                    } else {
                        adapter_statuses.push(adapter_status_entry(&adapter2_name, &a2.ip, a2.wireless, false, "待检测"));
                    }
                } else {
                    adapter_statuses.push(adapter_disabled_entry(&adapter2_name));
                }
            }
        }

        serde_json::Value::Array(adapter_statuses)
    };

    let any_online = adapter_statuses.as_array().map(|arr| arr.iter().any(|s| s["online"].as_bool().unwrap_or(false))).unwrap_or(false);

    let result = serde_json::json!({
        "serverAvailable": server_avail,
        "loginPreparationMode": config.auto_login_on_preparation,
        "checkCount": 0,
        "isRunning": running,
        "interval": config.background_check_interval,
        "enabled": config.enable_background_check,
        "adapterStatuses": adapter_statuses,
        "online": any_online,
    });

    Ok(result)
}

pub fn run_startup_tasks(app_handle: &AppHandle) {
    let s = app_handle.state::<AppState>();
    let config = s.config.load_full();

    if config.enable_background_check {
        let app_h = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            let s = app_h.state::<AppState>();
            if let Err(e) = start_background_check_inner(&app_h, &s) {
                crate::log_warn!("background", "启动后台检测失败: {}", e);
            }
        });
    }

    if config.enable_network_quality && config.enable_latency_test {
        let app_h = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            let s = app_h.state::<AppState>();
            if !s.tasks.latency_running.swap_acquire() {
                let interval = {
                    let c = s.config.load();
                    if c.latency_test_interval < 10000 { 30000 } else { c.latency_test_interval }
                };
                spawn_latency_test_loop(&app_h, interval);
            }
        });
    }

    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        run_auto_login_on_start(&app_h);
    });
}
