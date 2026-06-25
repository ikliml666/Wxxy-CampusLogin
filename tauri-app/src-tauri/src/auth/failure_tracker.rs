use tauri::AppHandle;
use crate::infra::events::EventBus;
use crate::infra::state::{AppState, CommandResult};

/// 连续认证失败达到此次数后触发 MAC 重置
const MAX_FAILURES: u32 = 5;

/// 认证失败码：ac_auth_failed(AC认证失败), 1(非法/失败/拒绝), 4(账号禁用)
const AUTH_FAILURE_CODES: &[&str] = &["ac_auth_failed", "1", "4"];

/// 双适配器失败计数器标识
#[derive(Clone, Copy)]
pub enum AdapterFailureCounter {
    A1,
    A2,
}

/// 判断 CommandResult 是否为认证失败（非网络错误）
pub fn is_auth_failure(result: &CommandResult) -> bool {
    let code = result.data.as_ref()
        .and_then(|d| d.get("code"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    AUTH_FAILURE_CODES.contains(&code)
}

fn get_adapter_failure_count(state: &AppState, counter: AdapterFailureCounter) -> u32 {
    let snap = state.network.load();
    match counter {
        AdapterFailureCounter::A1 => snap.a1_auth_failure_count,
        AdapterFailureCounter::A2 => snap.a2_auth_failure_count,
    }
}

fn set_adapter_failure_count(state: &AppState, counter: AdapterFailureCounter, value: u32) {
    state.network.update(|s| {
        match counter {
            AdapterFailureCounter::A1 => s.a1_auth_failure_count = value,
            AdapterFailureCounter::A2 => s.a2_auth_failure_count = value,
        }
    });
}

fn increment_adapter_failure_count(state: &AppState, counter: AdapterFailureCounter) {
    state.network.update(|s| {
        match counter {
            AdapterFailureCounter::A1 => s.a1_auth_failure_count += 1,
            AdapterFailureCounter::A2 => s.a2_auth_failure_count += 1,
        }
    });
}

/// 单适配器全局失败计数：连续 5 次认证失败触发全部适配器 MAC 重置
pub fn update_auth_failure_count(state: &AppState, app_handle: &AppHandle, cmd_result: &CommandResult, campus_gw: &str) {
    if cmd_result.success {
        let prev = state.network.load().portal_failure_count;
        if prev > 0 {
            state.network.update(|s| s.portal_failure_count = 0);
            crate::log_debug!("login", "登录成功，重置认证失败计数(原值={})", prev);
        }
        return;
    }

    if !is_auth_failure(cmd_result) {
        return;
    }

    let prev_count = state.network.load().portal_failure_count;
    state.network.increment_portal_failure_count();
    let new_count = prev_count + 1;
    crate::log_info!("login", "认证失败计数: {}/5 (code={})", new_count, cmd_result.data.as_ref()
        .and_then(|d| d.get("code"))
        .and_then(|v| v.as_str())
        .unwrap_or(""));

    if new_count >= MAX_FAILURES {
        crate::log_warn!("login", "连续{}次认证失败，触发MAC重置+DHCP续租", new_count);
        let event_bus = EventBus::new(app_handle);
        let _ = event_bus.emit_login_log("连续5次认证失败，正在重置MAC并重新获取IP...", "warning");
        match crate::network::dhcp_release_renew_all(campus_gw) {
            Ok(results) => {
                for r in &results {
                    let skipped = r.get("skipped").and_then(|v| v.as_bool()).unwrap_or(false);
                    let success = r.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
                    let name = r.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
                    if skipped {
                        crate::log_debug!("login", "MAC重置跳过非校园网适配器: {}", name);
                    } else if success {
                        crate::log_info!("login", "MAC重置成功: {}", name);
                    } else {
                        crate::log_warn!("login", "MAC重置失败: {}", name);
                    }
                }
            }
            Err(e) => {
                crate::log_error!("login", "MAC重置+DHCP续租失败: {}", e);
            }
        }
        state.network.update(|s| s.portal_failure_count = 0);
    }
}

/// 双适配器分别计数：对认证失败的适配器单独递增计数，连续 5 次触发该适配器的 MAC 重置
pub fn update_dual_adapter_auth_failure(
    state: &AppState,
    app_handle: &AppHandle,
    r1: &Option<CommandResult>,
    r2: &Option<CommandResult>,
    a1_name: &str,
    a2_name: &str,
    campus_gw: &str,
) {
    handle_single_adapter_failure(state, app_handle, r1, a1_name, campus_gw, AdapterFailureCounter::A1);
    handle_single_adapter_failure(state, app_handle, r2, a2_name, campus_gw, AdapterFailureCounter::A2);
}

/// 处理单个适配器的认证失败计数与 MAC 重置
fn handle_single_adapter_failure(
    state: &AppState,
    app_handle: &AppHandle,
    result: &Option<CommandResult>,
    adapter_name: &str,
    campus_gw: &str,
    counter: AdapterFailureCounter,
) {
    let success = result.as_ref().map(|r| r.success).unwrap_or(false);

    if success {
        let prev = get_adapter_failure_count(state, counter);
        if prev > 0 {
            set_adapter_failure_count(state, counter, 0);
            crate::log_debug!("login", "{} 登录成功，重置认证失败计数(原值={})", adapter_name, prev);
        }
        return;
    }

    if !result.as_ref().map(is_auth_failure).unwrap_or(false) {
        return;
    }

    let prev_count = get_adapter_failure_count(state, counter);
    increment_adapter_failure_count(state, counter);
    let new_count = prev_count + 1;
    crate::log_info!("login", "{} 认证失败计数: {}/5 (code={})", adapter_name, new_count,
        result.as_ref()
            .and_then(|r| r.data.as_ref())
            .and_then(|d| d.get("code"))
            .and_then(|v| v.as_str())
            .unwrap_or(""));

    if new_count >= MAX_FAILURES {
        crate::log_warn!("login", "{} 连续{}次认证失败，触发该适配器MAC重置", adapter_name, new_count);
        let event_bus = EventBus::new(app_handle);
        let _ = event_bus.emit_login_log(
            &format!("{adapter_name} 连续5次认证失败，正在重置该适配器MAC..."),
            "warning",
        );
        match crate::network::dhcp_release_renew_single(adapter_name, campus_gw) {
            Ok(r) => {
                let skipped = r.get("skipped").and_then(|v| v.as_bool()).unwrap_or(false);
                let success = r.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
                if skipped {
                    crate::log_debug!("login", "{} MAC重置跳过(非校园网子网)", adapter_name);
                } else if success {
                    crate::log_info!("login", "{} MAC重置成功", adapter_name);
                } else {
                    crate::log_warn!("login", "{} MAC重置失败", adapter_name);
                }
            }
            Err(e) => {
                crate::log_error!("login", "{} MAC重置失败: {}", adapter_name, e);
            }
        }
        set_adapter_failure_count(state, counter, 0);
    }
}

/// 重置所有认证失败计数（注销成功时调用）
pub fn reset_all(state: &AppState) {
    state.network.update(|s| {
        s.portal_failure_count = 0;
        s.a1_auth_failure_count = 0;
        s.a2_auth_failure_count = 0;
    });
    crate::log_info!("logout", "已重置所有认证失败计数");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::state::CommandResult;

    #[test]
    fn is_auth_failure_recognizes_known_codes() {
        for code in AUTH_FAILURE_CODES {
            let result = CommandResult {
                success: false,
                message: None,
                data: Some(serde_json::json!({ "code": code })),
            };
            assert!(is_auth_failure(&result), "code {} should be auth failure", code);
        }
    }

    #[test]
    fn is_auth_failure_rejects_network_error() {
        let result = CommandResult {
            success: false,
            message: None,
            data: Some(serde_json::json!({ "code": "network_error" })),
        };
        assert!(!is_auth_failure(&result));
    }

    #[test]
    fn is_auth_failure_rejects_success() {
        let result = CommandResult {
            success: true,
            message: None,
            data: Some(serde_json::json!({ "code": "0" })),
        };
        assert!(!is_auth_failure(&result));
    }

    #[test]
    fn is_auth_failure_handles_missing_code() {
        let result = CommandResult {
            success: false,
            message: None,
            data: Some(serde_json::json!({})),
        };
        assert!(!is_auth_failure(&result));
    }

    #[test]
    fn is_auth_failure_handles_missing_data() {
        let result = CommandResult {
            success: false,
            message: None,
            data: None,
        };
        assert!(!is_auth_failure(&result));
    }
}
