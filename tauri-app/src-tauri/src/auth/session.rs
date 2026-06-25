use tauri::AppHandle;
use std::sync::atomic::AtomicBool;
use crate::config::model::Config;
use crate::network::{
    Adapter, get_adapters_cached,
    ensure_ethernet_ip_for_login,
    wait_for_adapter,
};
use crate::auth::portal::check_portal_full;
use crate::auth::protocol::do_login_with_retry;
use crate::auth::traits::{AdapterResolver, DefaultAdapterResolver};
use crate::infra::events::EventBus;
use crate::infra::state::{AppState, CommandResult};
use crate::commands::system::append_login_history;
use crate::auth::failure_tracker::{
    update_auth_failure_count, update_dual_adapter_auth_failure,
};

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
                            let event_bus = EventBus::new(app_handle);
                            if let Err(e) = event_bus.emit_login_log(&format!("{} 已在线", adapter_name), "success") {
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

    // DHCP 续租可能改变了适配器 IP，绕过缓存重新获取，
    // 避免后续 find 仍用续租前的旧快照（IP 为空）导致登录失败
    let adapters = match crate::network::get_adapters_force() {
        Ok(a) => a,
        Err(_) => adapters,
    };

    if let Some(name) = adapter_name {
        let adapter = adapters.iter().find(|a| a.name == name && !a.ip.is_empty());
        match adapter {
            Some(a) => {
                let result = login_adapter_with_log(a, &config, app_handle, state.exit.is_quitting.as_ref())
                    .unwrap_or_else(|| CommandResult::err("登录请求失败"));
                update_auth_failure_count(state, app_handle, &result, &config.campus_gateway);
                return result;
            }
            None => return CommandResult::err(&format!("未找到适配器: {}", name)),
        }
    }

    let (adapter1_name, adapter2_name) = DefaultAdapterResolver.resolve_adapter_names(&adapters, &config);

    let a1 = adapters.iter().find(|a| a.name == adapter1_name && !a.ip.is_empty());
    if a1.is_none() {
        return CommandResult::err("未找到有效IP地址的适配器");
    }

    if config.dual_adapter && !adapter2_name.is_empty() && adapter2_name != adapter1_name {
        let a2 = adapters.iter().find(|a| a.name == adapter2_name && !a.ip.is_empty());
        if let Some(a2_ref) = a2 {
            let a1_ref = a1.unwrap();

            // 双适配器错峰并行登录：适配器2延迟1s启动，避免同时登录触发系统封禁
            // 使用 DualAdapterExecutor 统一并发执行与结果合并
            let a1_clone = a1_ref.clone();
            let a2_clone = a2_ref.clone();
            let config_clone1 = config.clone();
            let config_clone2 = config.clone();
            let app_h1 = app_handle.clone();
            let app_h2 = app_handle.clone();
            let is_quitting1 = state.exit.is_quitting.clone();
            let is_quitting2 = state.exit.is_quitting.clone();
            let dual_result = crate::auth::dual_adapter_executor::execute_dual(
                Box::new(move || login_adapter_with_log(&a1_clone, &config_clone1, &app_h1, is_quitting1.as_ref())),
                Box::new(move || login_adapter_with_log(&a2_clone, &config_clone2, &app_h2, is_quitting2.as_ref())),
                state.exit.is_quitting.clone(),
            );

            let result = dual_result.build_command_result();
            // 双适配器分别计数：对认证失败的适配器单独递增计数，连续5次触发该适配器 MAC 重置
            update_dual_adapter_auth_failure(
                state, app_handle, &dual_result.primary, &dual_result.secondary,
                &adapter1_name, &adapter2_name, &config.campus_gateway,
            );
            return result;
        }
    }

    let a1_ref = a1.unwrap();

    let result = login_adapter_with_log(a1_ref, &config, app_handle, state.exit.is_quitting.as_ref())
        .unwrap_or_else(|| CommandResult::err("登录请求失败"));
    update_auth_failure_count(state, app_handle, &result, &config.campus_gateway);
    result
}
