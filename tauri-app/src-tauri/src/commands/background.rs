use tauri::{AppHandle, Emitter, Manager, State};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use crate::infra::state::{AppState, CommandResult};
use crate::monitor::watcher;

#[tauri::command]
pub fn start_background_check(app_handle: AppHandle, _state: State<'_, AppState>) -> Result<CommandResult, String> {
    let s = app_handle.state::<AppState>();
    watcher::start_background_check_inner(&app_handle, &s)
}

#[tauri::command]
pub fn stop_background_check(_state: State<'_, AppState>, app_handle: AppHandle) -> Result<CommandResult, String> {
    let s = app_handle.state::<AppState>();
    s.tasks.bg_check_cancel.load().cancel();
    s.tasks.bg_check_cancel.store(Arc::new(tokio_util::sync::CancellationToken::new()));
    s.tasks.background_running.force_release();
    let cfg = s.update_config(|cfg| {
        cfg.enable_background_check = false;
    });
    if let Err(e) = super::config_cmd::save_config_to_disk_encrypted(&app_handle, &cfg) {
        crate::log_warn!("background", "保存停止检测配置失败: {}", e);
    }
    let mut emit_cfg = (*cfg).clone();
    emit_cfg.password = crate::config::model::PASSWORD_MASK.to_string();
    let _ = app_handle.emit("config-changed", serde_json::json!({ "config": emit_cfg }));
    Ok(CommandResult::ok_msg("后台检测已停止"))
}

#[tauri::command]
pub fn trigger_background_check(_state: State<'_, AppState>, app_handle: AppHandle) -> Result<CommandResult, String> {
    let s = app_handle.state::<AppState>();
    if s.tasks.is_checking.is_active() {
        return Ok(CommandResult::err("检测正在进行中"));
    }
    let app_h = app_handle.clone();
    let manual_cancel = s.tasks.bg_check_cancel.load().clone();
    tauri::async_runtime::spawn(async move {
        watcher::run_background_check(&app_h, manual_cancel).await;
    });
    Ok(CommandResult::ok_msg("已触发后台检测"))
}

pub fn get_background_status_value(state: &AppState, _app_handle: &AppHandle) -> serde_json::Value {
    let config = state.config.load_full();
    let running = state.tasks.background_running.is_active();
    let server_avail = state.network.server_available.load(Ordering::Acquire);

    let adapter_statuses = {
        let mut adapter_statuses = Vec::new();
        let a1_online = state.network.last_a1_online.load(Ordering::Acquire);

        if let Ok(adapters) = crate::network::get_adapters_cached() {
            let (adapter1_name, adapter2_name) = crate::network::resolve_adapter_names(&adapters, &config);

            if let Some(a1) = adapters.iter().find(|a| a.name == adapter1_name) {
                if a1.ip.is_empty() {
                    adapter_statuses.push(watcher::adapter_disconnected_entry(&adapter1_name, a1.wireless));
                } else {
                    adapter_statuses.push(watcher::adapter_status_entry(&adapter1_name, &a1.ip, a1.wireless, a1_online, if a1_online { "已在线" } else { "未在线" }));
                }
            } else if !adapter1_name.is_empty() {
                adapter_statuses.push(watcher::adapter_disabled_entry(&adapter1_name));
            }

            if config.dual_adapter && !adapter2_name.is_empty() {
                let a2_online_state = state.network.last_a2_online.load(Ordering::Acquire);
                if let Some(a2) = adapters.iter().find(|a| a.name == adapter2_name) {
                    if a2.ip.is_empty() {
                        adapter_statuses.push(watcher::adapter_disconnected_entry(&adapter2_name, a2.wireless));
                    } else {
                        adapter_statuses.push(watcher::adapter_status_entry(&adapter2_name, &a2.ip, a2.wireless, a2_online_state, if a2_online_state { "已在线" } else { "未在线" }));
                    }
                } else {
                    adapter_statuses.push(watcher::adapter_disabled_entry(&adapter2_name));
                }
            }
        }

        serde_json::Value::Array(adapter_statuses)
    };

    let any_online = adapter_statuses.as_array().map(|arr| arr.iter().any(|s| s["online"].as_bool().unwrap_or(false))).unwrap_or(false);

    let check_count = state.network.background_check_count.load(Ordering::Acquire);
    let current_ssid = state.network.current_ssid.load();
    let on_campus = state.network.on_campus_network.load(Ordering::Acquire);

    serde_json::json!({
        "serverAvailable": server_avail,
        "loginPreparationMode": config.auto_login_on_preparation,
        "checkCount": check_count,
        "isRunning": running,
        "interval": config.background_check_interval,
        "enabled": config.enable_background_check,
        "adapterStatuses": adapter_statuses,
        "online": any_online,
        "currentSsid": current_ssid.as_ref(),
        "onCampusNetwork": on_campus,
        "enableNetworkNameCheck": config.enable_network_name_check,
        "requiredNetworkName": config.required_network_name,
        "campusWifi": serde_json::Value::Null,
        "campusWired": serde_json::Value::Null,
        "a1OnCampus": serde_json::Value::Null,
        "a2OnCampus": serde_json::Value::Null,
        "a1CampusMessage": serde_json::Value::Null,
        "a2CampusMessage": serde_json::Value::Null,
    })
}

#[tauri::command]
pub async fn get_background_status(app_handle: AppHandle) -> Result<serde_json::Value, String> {
    let state = app_handle.state::<AppState>();
    Ok(get_background_status_value(&state, &app_handle))
}

