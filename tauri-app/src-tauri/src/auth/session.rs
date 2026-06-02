use tauri::{AppHandle, Emitter};
use std::sync::atomic::AtomicBool;
use crate::config::model::Config;
use crate::network::{
    Adapter, get_adapters_cached,
    ensure_ethernet_ip_for_login,
    select_adapter,
    wait_for_adapter,
};
use crate::auth::portal::check_portal_full;
use crate::auth::protocol::do_login_with_retry;
use crate::infra::state::{AppState, CommandResult};
use crate::commands::system::append_login_history;

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

    if let Err(e) = app_handle.emit("login-log", serde_json::json!({
        "message": format!("{} 正在{}...", adapter.name, action_name),
        "type": "info"
    })) {
        crate::log_warn!(log_tag, "发送{}日志失败: {}", action_name, e);
    }

    match do_action() {
        Ok(result) => {
            let success = result.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
            let message = result.get("message").and_then(|v| v.as_str()).unwrap_or("");
            let display_msg = format!("{} {}", adapter.name, message);
            if success {
                if let Err(e) = app_handle.emit("login-log", serde_json::json!({
                    "message": format!("{} {}成功", adapter.name, action_name),
                    "type": "success"
                })) {
                    crate::log_warn!(log_tag, "发送{}日志失败: {}", action_name, e);
                }
                if let Err(e) = append_login_history(app_handle, true, message, &adapter.name, &config.user, action_type) {
                    crate::log_warn!(log_tag, "记录{}历史失败: {}", action_name, e);
                }
            } else {
                if let Err(e) = app_handle.emit("login-log", serde_json::json!({
                    "message": format!("{} {}失败: {}", adapter.name, action_name, message),
                    "type": "warning"
                })) {
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
            if let Err(emit_err) = app_handle.emit("login-log", serde_json::json!({
                "message": format!("{} {}请求失败: {}", adapter.name, action_name, e),
                "type": "error"
            })) {
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

    if let Ok(sec_status) = check_portal_full(&adapter.ip, Some(&adapter.name), None, None, Some(&config.operator)) {
        if sec_status.online {
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
                let message = data.get("message").and_then(|v| v.as_str()).unwrap_or("");
                if message.contains("无法解析登录响应") {
                    if let Ok(sec_status) = check_portal_full(&adapter_ip, Some(&adapter_name), None, None, Some(&config_operator)) {
                        if sec_status.online {
                            if let Err(e) = app_handle.emit("login-log", serde_json::json!({
                                "message": format!("{} 已在线", adapter_name),
                                "type": "success"
                            })) {
                                crate::log_warn!("login", "发送登录日志失败: {}", e);
                            }
                            return Some(CommandResult {
                                success: true,
                                message: Some(format!("{} 已在线", adapter_name)),
                                data: Some(serde_json::json!({ "code": "0" })),
                            });
                        }
                    }
                }
            }
        }
    }

    result
}

pub fn full_login_inner(state: &AppState, app_handle: &AppHandle, adapter_name: Option<&str>) -> CommandResult {
    let config = {
        let guard = state.config.load();
        if guard.user.is_empty() || guard.password.is_empty() {
            crate::log_warn!("login", "登录失败: 用户名或密码为空");
            return CommandResult::err("用户名或密码为空");
        }
        guard.clone()
    };

    crate::log_info!("login", "开始登录, 用户: {}{}, 指定适配器: {:?}", config.user, config.operator, adapter_name);

    let adapters = match get_adapters_cached() {
        Ok(a) => a,
        Err(_) => match wait_for_adapter(10000, state.exit.is_quitting.as_ref()) {
            Ok(a) => a,
            Err(e) => return CommandResult::err(&format!("获取适配器失败: {}", e)),
        },
    };

    if adapters.is_empty() {
        return CommandResult::err("未找到可用网络适配器");
    }

    ensure_ethernet_ip_for_login(app_handle, &adapters, &config, state.exit.is_quitting.as_ref());

    if let Some(name) = adapter_name {
        let adapter = adapters.iter().find(|a| a.name == name && !a.ip.is_empty());
        match adapter {
            Some(a) => {
                return login_adapter_with_log(a, &config, app_handle, state.exit.is_quitting.as_ref())
                    .unwrap_or_else(|| CommandResult::err("登录请求失败"));
            }
            None => return CommandResult::err(&format!("未找到适配器: {}", name)),
        }
    }

    let (adapter1_ip, adapter1_name) = select_adapter(&adapters, &config);
    if adapter1_ip.is_empty() {
        return CommandResult::err("未找到有效IP地址的适配器");
    }

    let a1 = adapters.iter().find(|a| a.name == adapter1_name);

    if config.dual_adapter {
        let (_, a2n) = crate::network::resolve_adapter_names(&adapters, &config);
        if !a2n.is_empty() {
            let a2 = adapters.iter().find(|a| a.name == a2n && !a.ip.is_empty());
            if let Some(a2_ref) = a2 {
                let a1_ref = match a1.or_else(|| adapters.iter().find(|a| a.name == adapter1_name)) {
                    Some(a) => a,
                    None => return CommandResult::err("未找到主适配器"),
                };

                let r1 = login_adapter_with_log(a1_ref, &config, app_handle, state.exit.is_quitting.as_ref());

                let r2 = login_adapter_with_log(a2_ref, &config, app_handle, state.exit.is_quitting.as_ref());

                let a1_success = r1.as_ref().map(|r| r.success).unwrap_or(false);
                let a2_success = r2.as_ref().map(|r| r.success).unwrap_or(false);

                let a1_msg = r1.and_then(|r| r.message).unwrap_or_default();
                let a2_msg = r2.and_then(|r| r.message).unwrap_or_default();

                let combined_msg = if !a1_msg.is_empty() && !a2_msg.is_empty() {
                    format!("{}, {}", a1_msg, a2_msg)
                } else {
                    format!("{}{}", a1_msg, a2_msg)
                };

                return CommandResult {
                    success: a1_success || a2_success,
                    message: Some(combined_msg),
                    data: Some(serde_json::json!({ "code": if a1_success || a2_success { "0" } else { "1" } })),
                };
            }
        }
    }

    let a1_ref = match a1 {
        Some(a) if !a.ip.is_empty() => a,
        _ => return CommandResult::err("未找到有效适配器"),
    };

    login_adapter_with_log(a1_ref, &config, app_handle, state.exit.is_quitting.as_ref())
        .unwrap_or_else(|| CommandResult::err("登录请求失败"))
}
