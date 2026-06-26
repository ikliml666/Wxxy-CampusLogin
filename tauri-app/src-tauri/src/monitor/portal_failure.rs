use tauri::AppHandle;
use crate::infra::events::EventBus;
use crate::infra::state::AppState;
use crate::auth::failure_tracker::{
    AdapterFailureCounter,
    get_adapter_failure_count,
    set_adapter_failure_count,
    increment_adapter_failure_count,
};

/// Portal 请求失败连续触发阈值
const PORTAL_REQUEST_FAILURE_THRESHOLD: u32 = 5;

/// 处理单个适配器 Portal 请求失败（`PortalCheckResult::Error { is_request_failed: true }`）的统一逻辑
///
/// 网关不可达时跳过计数并重置（校园网断网/维护期避免误重置 MAC）；
/// 网关可达时累加失败计数，连续 5 次触发该适配器 MAC 重置。
///
/// 与 `auth::failure_tracker::handle_single_adapter_failure` 的区别：
/// 后者针对认证失败（ac_auth_failed/1/4），本函数针对 Portal HTTP 请求失败，
/// 触发条件不同，但共用 `AdapterFailureCounter` 枚举与计数访问器。
pub fn handle_portal_request_failure(
    state: &AppState,
    app_handle: &AppHandle,
    adapter_ref: Option<&crate::network::Adapter>,
    adapter_ip: Option<&str>,
    campus_gw: &str,
    counter: AdapterFailureCounter,
    adapter_label: &str,
) {
    let gw_reachable = crate::network::check_gateway_reachable_from(campus_gw, adapter_ip);
    if !gw_reachable {
        crate::log_info!(
            "background",
            "{} Portal失败但网关[{}]从[{}]不可达，跳过计数（校园网断网/维护）",
            adapter_label, campus_gw, adapter_ip.unwrap_or("")
        );
        let prev = get_adapter_failure_count(state, counter);
        set_adapter_failure_count(state, counter, 0);
        if prev > 0 {
            crate::log_debug!("background", "{} 网关不可达，重置失败计数(原值={})", adapter_label, prev);
        }
        return;
    }

    let prev_count = get_adapter_failure_count(state, counter);
    increment_adapter_failure_count(state, counter);
    let new_count = prev_count + 1;
    crate::log_info!(
        "background",
        "{} Portal失败计数: {}/{} (网关可达)",
        adapter_label, new_count, PORTAL_REQUEST_FAILURE_THRESHOLD
    );

    if new_count < PORTAL_REQUEST_FAILURE_THRESHOLD {
        return;
    }

    crate::log_warn!(
        "background",
        "{} 连续{}次Portal失败(网关可达)，触发该适配器MAC重置",
        adapter_label, new_count
    );
    let event_bus = EventBus::new(app_handle);
    let _ = event_bus.emit_login_log(
        &format!("{} 连续{}次 Portal 请求失败，正在重置该适配器MAC...", adapter_label, PORTAL_REQUEST_FAILURE_THRESHOLD),
        "warning",
    );

    if let Some(adapter) = adapter_ref {
        match crate::network::dhcp_release_renew_single(&adapter.name, campus_gw) {
            Ok(r) => {
                let skipped = r.get("skipped").and_then(|v| v.as_bool()).unwrap_or(false);
                let success = r.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
                if skipped {
                    crate::log_debug!("background", "{} MAC重置跳过(非校园网子网)", adapter_label);
                } else if success {
                    crate::log_info!("background", "{} MAC重置成功", adapter_label);
                } else {
                    crate::log_warn!("background", "{} MAC重置失败", adapter_label);
                }
            }
            Err(e) => {
                crate::log_error!("background", "{} MAC重置失败: {}", adapter_label, e);
            }
        }
    }

    set_adapter_failure_count(state, counter, 0);
}
