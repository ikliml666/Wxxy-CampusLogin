//! 后台检测结果发射与状态更新辅助函数
//!
//! 从 watcher.rs 拆分，集中管理 background_check 结果的构造、事件发射与网络状态更新。

use tauri::AppHandle;
use crate::infra::events::EventBus;
use crate::infra::state::AppState;
use crate::infra::lifecycle::start_auto_exit;
use super::campus_check::CampusCheckResult;

/// 后台检测结果聚合，封装 emit_background_check_result 所需的全部数据字段，
/// 替代原先 16 个独立参数，避免调用方传参错位。
pub(super) struct BackgroundCheckResult<'a> {
    pub online: bool,
    pub reachable: bool,
    pub login_available: bool,
    pub message: &'a str,
    pub adapter1_name: &'a str,
    pub adapter2_name: &'a str,
    pub secondary_online: Option<bool>,
    pub secondary_message: &'a str,
    pub dual_adapter: bool,
    pub config: &'a crate::config::model::Config,
    pub campus_result: &'a CampusCheckResult,
    pub a1_campus_msg: Option<&'a str>,
    pub a2_campus_msg: Option<&'a str>,
    pub a1_on_campus: Option<bool>,
    pub a2_on_campus: Option<bool>,
}

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
    config: &crate::config::model::Config,
) -> String {
    let mut details = vec![format!("{}: {}", adapter1_name, adapter1_message)];
    if let Some(msg) = adapter2_message {
        if crate::network::is_secondary_adapter_enabled(config, adapter2_name) {
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
        config,
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

pub(super) fn emit_background_check_result(
    app_handle: &AppHandle,
    state: &AppState,
    result: &BackgroundCheckResult,
) {
    let check_count = state.network.load().background_check_count + 1;
    state.network.increment_background_check_count();
    let is_running = state.task_manager.is_running("background_check");
    // increment 后单次 load 快照，复用读取 current_ssid / on_campus_network / logout_protected_until
    let snap = state.network.load();
    let ssid_val = snap.current_ssid.clone();
    let on_campus_val = snap.on_campus_network;

    // 注销保护期内，强制 online=false，避免 Portal 延迟导致前端误判为在线
    let protected_until = snap.logout_protected_until;
    let is_logout_protected = std::time::Instant::now() < protected_until;
    let (effective_online, effective_secondary_online) = if is_logout_protected {
        crate::log_debug!("background", "注销保护期内，emit 事件强制 online=false");
        (false, Some(false))
    } else {
        (result.online, result.secondary_online)
    };

    if let Err(e) = EventBus::new(app_handle).emit_background_check_result(serde_json::json!({
        "serverAvailable": result.reachable,
        "loginAvailable": result.login_available,
        "online": effective_online,
        "message": result.message,
        "adapter1Name": result.adapter1_name,
        "adapter2Name": if result.dual_adapter { result.adapter2_name } else { "" },
        "secondaryOnline": effective_secondary_online,
        "secondaryMessage": result.secondary_message,
        "timestamp": chrono::Utc::now().timestamp_millis(),
        "checkCount": check_count,
        "isRunning": is_running,
        "currentSsid": ssid_val.as_ref(),
        "onCampusNetwork": on_campus_val,
        "enableNetworkNameCheck": result.config.enable_network_name_check,
        "requiredNetworkName": result.config.required_network_name,
        "campusWifi": result.campus_result.wifi,
        "campusWired": result.campus_result.wired,
        "a1CampusMessage": result.a1_campus_msg,
        "a2CampusMessage": result.a2_campus_msg,
        "a1OnCampus": result.a1_on_campus,
        "a2OnCampus": result.a2_on_campus,
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

    state.network.update(|s| {
        s.any_adapter_online = any_online;
        s.last_a1_online = online;
        if any_online {
            s.disconnect_reconnect_count = 0;
        }
    });

    if reachable && !state.network.load().has_logged_online && online {
        state.network.update(|s| s.has_logged_online = true);
        if state.config.load().auto_exit_on_online {
            start_auto_exit(app_handle, state);
        }
    }
}
