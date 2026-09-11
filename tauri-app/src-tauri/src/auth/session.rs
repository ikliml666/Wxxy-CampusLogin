use tauri::AppHandle;
use std::sync::atomic::AtomicBool;
use crate::config::model::Config;
use crate::network::Adapter;
use crate::auth::portal::check_portal_full;
use crate::auth::protocol::do_login_with_retry;
use crate::infra::events::EventBus;
use crate::infra::state::CommandResult;
use crate::config::persist::append_login_history;

pub fn adapter_action_with_log<F>(
    adapter: &Adapter,
    config: &Config,
    app_handle: &AppHandle,
    action_name: &str,
    log_tag: &str,
    action_type: &str,
    do_action: F,
) -> Option<CommandResult>
where
    F: FnOnce() -> Result<serde_json::Value, String>,
{
    if adapter.ip.is_empty() {
        return None;
    }

    let event_bus = EventBus::new(app_handle);
    if let Err(e) = event_bus.emit_login_log(&format!("{} 正在{}...", adapter.name, action_name), "info") {
        crate::log_warn!(log_tag, "发送{}日志失败: {}", action_name, e);
    }

    match do_action() {
        Ok(result) => {
            let success = result.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
            let message = result.get("message").and_then(|v| v.as_str()).unwrap_or("");
            let display_msg = format!("{} {}", adapter.name, message);
            if success {
                if let Err(e) = event_bus.emit_login_log(&format!("{} {}成功", adapter.name, action_name), "success") {
                    crate::log_warn!(log_tag, "发送{}日志失败: {}", action_name, e);
                }
                if let Err(e) = append_login_history(app_handle, true, message, &adapter.name, &config.user, action_type) {
                    crate::log_warn!(log_tag, "记录{}历史失败: {}", action_name, e);
                }
            } else {
                if let Err(e) = event_bus.emit_login_log(
                    &format!("{} {}失败: {}", adapter.name, action_name, message),
                    "warning",
                ) {
                    crate::log_warn!(log_tag, "发送{}日志失败: {}", action_name, e);
                }
                if let Err(e) = append_login_history(app_handle, false, message, &adapter.name, &config.user, action_type) {
                    crate::log_warn!(log_tag, "记录{}历史失败: {}", action_name, e);
                }
            }
            Some(CommandResult {
                success,
                message: Some(display_msg),
                data: Some(result),
            })
        }
        Err(e) => {
            if let Err(emit_err) = event_bus.emit_login_log(
                &format!("{} {}请求失败: {}", adapter.name, action_name, e),
                "error",
            ) {
                crate::log_warn!(log_tag, "发送{}日志失败: {}", action_name, emit_err);
            }
            Some(CommandResult {
                success: false,
                message: Some(format!("{} {}请求失败: {}", adapter.name, action_name, e)),
                data: Some(serde_json::json!({ "code": "error", "message": e })),
            })
        }
    }
}

pub fn login_adapter_with_log(
    adapter: &Adapter,
    config: &Config,
    app_handle: &AppHandle,
    is_quitting: &AtomicBool,
) -> Option<CommandResult> {
    if adapter.ip.is_empty() {
        return None;
    }

    if let Ok(sec_status) = check_portal_full(&adapter.ip, Some(&adapter.name)) {
        if sec_status.online {
            // 预检"已在线"直通成功，与正常成功路径一致补记登录历史
            if let Err(e) = append_login_history(app_handle, true, &sec_status.message, &adapter.name, &config.user, "login") {
                crate::log_warn!("login", "记录登录历史失败: {}", e);
            }
            return Some(CommandResult {
                success: true,
                message: Some(sec_status.message),
                data: Some(serde_json::json!({ "code": "0" })),
            });
        }
    }

    let adapter_ip = adapter.ip.clone();
    let adapter_name = adapter.name.clone();
    let config_user = config.user.clone();
    let config_password = config.password.clone();
    let config_operator = config.operator.clone();
    let is_quitting_ref = is_quitting;

    let result = adapter_action_with_log(
        adapter, config, app_handle,
        "登录", "login", "login",
        || do_login_with_retry(&config_user, &config_password, &config_operator, Some(adapter_ip.as_str()), 3, is_quitting_ref),
    );

    if let Some(ref cmd_result) = result {
        if !cmd_result.success {
            if let Some(ref data) = cmd_result.data {
                // code=parse_error 且 retryable=true 仅对应 protocol.rs 的"无法解析登录响应"
                // 分支（HTML 分支同 code 但 retryable=false），改判 data 字段消除跨文件文案耦合
                let is_parse_error = data.get("code").and_then(|v| v.as_str()) == Some("parse_error")
                    && data.get("retryable").and_then(|v| v.as_bool()).unwrap_or(false);
                if is_parse_error {
                    if let Ok(sec_status) = check_portal_full(&adapter_ip, Some(&adapter_name)) {
                        if sec_status.online {
                            let event_bus = EventBus::new(app_handle);
                            if let Err(e) = event_bus.emit_login_log(&format!("{adapter_name} 已在线"), "success") {
                                crate::log_warn!("login", "发送登录日志失败: {}", e);
                            }
                            return Some(CommandResult {
                                success: true,
                                message: Some(format!("{adapter_name} 已在线")),
                                data: Some(serde_json::json!({ "code": "0" })),
                            });
                        }
                    }
                }
            }
        }
    }

    // "已经在线"假成功复核：注销后网关在线表残留的"僵尸"会话会让 Portal 对本机 IP
    // 回"已经在线"（success=true）但实际流量不通。保持成功会掩盖断网并抑制重连；
    // 降级为认证失败（code=1）走 update_auth_failure_count 计数，连续 5 次触发该
    // 适配器 MAC 重置自动自愈。预检直通与 parse_error 复核路径已实测确认，无需再验。
    if let Some(ref cmd_result) = result {
        if cmd_result.success {
            let already_online_msg = cmd_result
                .data
                .as_ref()
                .and_then(|d| d.get("message"))
                .and_then(|m| m.as_str())
                .map(|m| m.contains("已经在线"))
                .unwrap_or(false);
            if already_online_msg {
                let verified = check_portal_full(&adapter_ip, Some(&adapter_name))
                    .map(|s| s.online)
                    .unwrap_or(false);
                if !verified {
                    crate::log_warn!("login", "{adapter_name} 返回已在线但 Portal 探测不通，判定服务端会话残留");
                    return Some(CommandResult {
                        success: false,
                        message: Some(format!(
                            "{adapter_name} 已在线但网络不通（服务端会话残留），建议自助服务强制下线或获取新IP后重试"
                        )),
                        data: Some(serde_json::json!({
                            "code": "1",
                            "message": "已在线但网络不通（服务端会话残留）",
                            "retryable": false,
                        })),
                    });
                }
            }
        }
    }

    result
}

