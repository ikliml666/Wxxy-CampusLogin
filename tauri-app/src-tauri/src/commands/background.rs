use tauri::{AppHandle, State};
use crate::infra::command_context::{AppHandleExt, CommandContext};
use std::sync::Arc;
use crate::infra::state::{AppState, CommandResult};
use crate::monitor::watcher;

#[tauri::command]
pub fn start_background_check(app_handle: AppHandle, state: State<'_, AppState>) -> Result<CommandResult, String> {
    let ctx = CommandContext::new(&app_handle, &state);
    watcher::start_background_check_inner(ctx.app, ctx.state)
}

#[tauri::command]
pub fn stop_background_check(_state: State<'_, AppState>, app_handle: AppHandle) -> Result<CommandResult, String> {
    let s = CommandContext::from_app(&app_handle);
    s.task_manager.cancel("background_check");
    let cfg = s.config.update(|cfg| {
        cfg.enable_background_check = false;
    });
    if let Err(e) = super::config_cmd::save_config_to_disk_encrypted(&app_handle, &cfg) {
        crate::log_warn!("background", "保存停止检测配置失败: {}", e);
    }
    let mut emit_cfg = (*cfg).clone();
    emit_cfg.password = crate::config::model::PASSWORD_MASK.to_string();
    let _ = app_handle.notify_config_changed(&emit_cfg);
    Ok(CommandResult::ok_msg("后台检测已停止"))
}

#[tauri::command]
pub fn trigger_background_check(_state: State<'_, AppState>, app_handle: AppHandle) -> Result<CommandResult, String> {
    let s = CommandContext::from_app(&app_handle);
    if s.tasks.is_checking.is_active() {
        return Ok(CommandResult::err("检测正在进行中"));
    }
    let app_h = app_handle.clone();
    let manual_cancel = s.task_manager
        .cancel_token("background_check")
        .unwrap_or_else(|| Arc::new(tokio_util::sync::CancellationToken::new()));
    tauri::async_runtime::spawn(async move {
        watcher::run_background_check(&app_h, manual_cancel).await;
    });
    Ok(CommandResult::ok_msg("已触发后台检测"))
}

pub fn get_background_status_value(state: &AppState, _app_handle: &AppHandle) -> serde_json::Value {
    let config = state.config.load_full();
    let running = state.task_manager.is_running("background_check");
    let server_avail = state.network.load().server_available;

    let adapter_statuses = {
        let mut adapter_statuses = Vec::new();
        let a1_online = state.network.load().last_a1_online;

        if let Ok(adapters) = crate::network::get_adapters_cached() {
            let (adapter1_name, adapter2_name) = crate::network::resolve_adapter_names(&adapters, &config);

            if let Some(a1) = crate::network::find_by_name(&adapters, &adapter1_name) {
                if a1.ip.is_empty() {
                    adapter_statuses.push(watcher::adapter_disconnected_entry(&adapter1_name, a1.wireless));
                } else {
                    adapter_statuses.push(watcher::adapter_status_entry(&adapter1_name, &a1.ip, a1.wireless, a1_online, if a1_online { "已在线" } else { "未在线" }));
                }
            } else if !adapter1_name.is_empty() {
                adapter_statuses.push(watcher::adapter_disabled_entry(&adapter1_name));
            }

            if crate::network::is_secondary_adapter_enabled(&config, &adapter2_name) {
                let a2_online_state = state.network.load().last_a2_online;
                if let Some(a2) = crate::network::find_by_name(&adapters, &adapter2_name) {
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

    let check_count = state.network.load().background_check_count;
    let current_ssid = state.network.load().current_ssid.clone();
    let on_campus = state.network.load().on_campus_network;

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
    let state = CommandContext::from_app(&app_handle);
    Ok(get_background_status_value(&state, &app_handle))
}

