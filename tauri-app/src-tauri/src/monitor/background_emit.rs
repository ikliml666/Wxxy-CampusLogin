//! 后台检测结果发射与状态更新辅助函数
//!
//! 从 watcher.rs 拆分，集中管理 background_check 结果的构造、事件发射与网络状态更新。

use tauri::AppHandle;
use crate::infra::events::EventBus;
use crate::infra::state::AppState;
use crate::infra::lifecycle::start_auto_exit;
use super::campus_check::CampusCheckResult;

pub fn adapter_status_entry(name: &str, ip: &str, wireless: bool, online: bool, message: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name, "ip": ip, "wireless": wireless,
        "online": online, "message": message
    })
}

pub fn adapter_disabled_entry(name: &str) -> serde_json::Value {
    adapter_status_entry(name, "", false, false, "适配器已禁用或未找到")
}

pub fn adapter_disconnected_entry(name: &str, wireless: bool) -> serde_json::Value {
    adapter_status_entry(name, "", wireless, false, "适配器未连接")
}

pub(super) fn build_adapter_details(
    adapter1_name: &str,
    adapter1_message: &str,
    adapter2_name: &str,
    adapter2_message: Option<&str>,
    dual_adapter: bool,
) -> String {
    let mut details = vec![format!("{}: {}", adapter1_name, adapter1_message)];
    if let Some(msg) = adapter2_message {
        if dual_adapter && !adapter2_name.is_empty() {
            details.push(format!("{adapter2_name}: {msg}"));
        }
    }
    details.join(", ")
}

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_status_change(
    prev_online: bool,
    current_online: bool,
    reachable: bool,
    login_available: bool,
    adapter1_name: &str,
    adapter1_message: &str,
    adapter2_name: &str,
    adapter2_message: Option<&str>,
    config: &crate::config::model::Config,
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
            crate::infra::notification::emit_notification(app_handle, "网络状态变更", &adapter_details);
        }
    } else {
        crate::log_debug!("background", "检测结果: online={}, reachable={}, loginAvailable={}, [{}]", current_online, reachable, login_available, adapter_details);
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_background_check_result(
    app_handle: &AppHandle,
    state: &AppState,
    online: bool,
    reachable: bool,
    login_available: bool,
    message: &str,
    adapter1_name: &str,
    adapter2_name: &str,
    secondary_online: Option<bool>,
    secondary_message: &str,
    dual_adapter: bool,
    config: &crate::config::model::Config,
    campus_result: &CampusCheckResult,
    a1_campus_msg: Option<&str>,
    a2_campus_msg: Option<&str>,
    a1_on_campus: Option<bool>,
    a2_on_campus: Option<bool>,
) {
    let check_count = state.network.load().background_check_count + 1;
    state.network.increment_background_check_count();
    let is_running = state.task_manager.is_running("background_check");
    let ssid_val = state.network.load().current_ssid.clone();
    let on_campus_val = state.network.load().on_campus_network;

    // 注销保护期内，强制 online=false，避免 Portal 延迟导致前端误判为在线
    let protected_until = state.network.load().logout_protected_until;
    let is_logout_protected = std::time::Instant::now() < protected_until;
    let (effective_online, effective_secondary_online) = if is_logout_protected {
        crate::log_debug!("background", "注销保护期内，emit 事件强制 online=false");
        (false, Some(false))
    } else {
        (online, secondary_online)
    };

    if let Err(e) = EventBus::new(app_handle).emit_background_check_result(serde_json::json!({
        "serverAvailable": reachable,
        "loginAvailable": login_available,
        "online": effective_online,
        "message": message,
        "adapter1Name": adapter1_name,
        "adapter2Name": if dual_adapter { adapter2_name } else { "" },
        "secondaryOnline": effective_secondary_online,
        "secondaryMessage": secondary_message,
        "timestamp": chrono::Utc::now().timestamp_millis(),
        "checkCount": check_count,
        "isRunning": is_running,
        "currentSsid": ssid_val.as_ref(),
        "onCampusNetwork": on_campus_val,
        "enableNetworkNameCheck": config.enable_network_name_check,
        "requiredNetworkName": config.required_network_name,
        "campusWifi": campus_result.wifi,
        "campusWired": campus_result.wired,
        "a1CampusMessage": a1_campus_msg,
        "a2CampusMessage": a2_campus_msg,
        "a1OnCampus": a1_on_campus,
        "a2OnCampus": a2_on_campus,
    })) {
        crate::log_warn!("background", "发送后台检测结果失败: {}", e);
    }
}

pub(super) fn update_network_state(
    state: &AppState,
    online: bool,
    secondary_online: Option<bool>,
    reachable: bool,
    app_handle: &AppHandle,
) {
    state.network.update(|s| s.server_available = reachable);

    let any_online = online || secondary_online == Some(true);

    let protected_until = state.network.load().logout_protected_until;
    let is_logout_protected = std::time::Instant::now() < protected_until;

    if is_logout_protected {
        crate::log_debug!("background", "注销保护期内，跳过网络状态更新: any_online={}", any_online);
        return;
    }

    state.network.update(|s| s.any_adapter_online = any_online);
    state.network.update(|s| s.last_a1_online = online);
    if any_online {
        state.network.update(|s| s.disconnect_reconnect_count = 0);
    }

    if reachable && !state.network.load().has_logged_online && online {
        state.network.update(|s| s.has_logged_online = true);
        if state.config.load().auto_exit_on_online {
            start_auto_exit(app_handle, state);
        }
    }
}
