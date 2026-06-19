use tauri::{AppHandle, Emitter};
use std::sync::atomic::{AtomicBool, Ordering};
use crate::config::model::Config;
use crate::network::{
    Adapter, get_adapters_cached,
    ensure_ethernet_ip_for_login,
    wait_for_adapter,
};
use crate::auth::portal::check_portal_full;
use crate::auth::protocol::do_login_with_retry;
use crate::infra::state::{AppState, CommandResult};
use crate::commands::system::append_login_history;

/// 登录认证失败计数：连续5次认证失败触发 MAC 重置
/// 仅认证失败码（ac_auth_failed/1/4）递增计数；网络错误不计数；成功重置计数
fn update_auth_failure_count(state: &AppState, app_handle: &AppHandle, cmd_result: &CommandResult, campus_gw: &str) {
    let code = cmd_result.data.as_ref()
        .and_then(|d| d.get("code"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if cmd_result.success {
        let prev = state.network.portal_failure_count.swap(0, Ordering::AcqRel);
        if prev > 0 {
            crate::log_debug!("login", "登录成功，重置认证失败计数(原值={})", prev);
        }
        return;
    }

    // 认证失败码：ac_auth_failed(AC认证失败), 1(非法/失败/拒绝), 4(账号禁用)
    let is_auth_failure = matches!(code, "ac_auth_failed" | "1" | "4");
    if !is_auth_failure {
        return;
    }

    let prev_count = state.network.portal_failure_count.fetch_add(1, Ordering::AcqRel);
    let new_count = prev_count + 1;
    crate::log_info!("login", "认证失败计数: {}/5 (code={})", new_count, code);

    if new_count >= 5 {
        crate::log_warn!("login", "连续{}次认证失败，触发MAC重置+DHCP续租", new_count);
        let _ = app_handle.emit("login-log", serde_json::json!({
            "message": "连续5次认证失败，正在重置MAC并重新获取IP...",
            "type": "warning"
        }));
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
        state.network.portal_failure_count.store(0, Ordering::Release);
    }
}

/// 双适配器分别计数：对认证失败的适配器单独递增计数，连续5次触发该适配器的 MAC 重置
fn update_dual_adapter_auth_failure(
    state: &AppState,
    app_handle: &AppHandle,
    r1: &Option<CommandResult>,
    r2: &Option<CommandResult>,
    a1_name: &str,
    a2_name: &str,
    campus_gw: &str,
) {
    // 适配器1处理
    handle_single_adapter_failure(
        state, app_handle, r1, a1_name, campus_gw,
        &state.network.a1_auth_failure_count,
    );
    // 适配器2处理
    handle_single_adapter_failure(
        state, app_handle, r2, a2_name, campus_gw,
        &state.network.a2_auth_failure_count,
    );
}

/// 处理单个适配器的认证失败计数与 MAC 重置
fn handle_single_adapter_failure(
    state: &AppState,
    app_handle: &AppHandle,
    result: &Option<CommandResult>,
    adapter_name: &str,
    campus_gw: &str,
    counter: &std::sync::atomic::AtomicU32,
) {
    let _ = state; // state 暂未在单适配器分支使用，保留参数以备扩展
    let code = result.as_ref()
        .and_then(|r| r.data.as_ref())
        .and_then(|d| d.get("code"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let success = result.as_ref().map(|r| r.success).unwrap_or(false);

    if success {
        let prev = counter.swap(0, Ordering::AcqRel);
        if prev > 0 {
            crate::log_debug!("login", "{} 登录成功，重置认证失败计数(原值={})", adapter_name, prev);
        }
        return;
    }

    // 认证失败码：ac_auth_failed(AC认证失败), 1(非法/失败/拒绝), 4(账号禁用)
    let is_auth_failure = matches!(code, "ac_auth_failed" | "1" | "4");
    if !is_auth_failure {
        return;
    }

    let prev_count = counter.fetch_add(1, Ordering::AcqRel);
    let new_count = prev_count + 1;
    crate::log_info!("login", "{} 认证失败计数: {}/5 (code={})", adapter_name, new_count, code);

    if new_count >= 5 {
        crate::log_warn!("login", "{} 连续{}次认证失败，触发该适配器MAC重置", adapter_name, new_count);
        let _ = app_handle.emit("login-log", serde_json::json!({
            "message": format!("{} 连续5次认证失败，正在重置该适配器MAC...", adapter_name),
            "type": "warning"
        }));
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
        counter.store(0, Ordering::Release);
    }
}

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

    let (adapter1_name, adapter2_name) = crate::network::resolve_adapter_names(&adapters, &config);

    let a1 = adapters.iter().find(|a| a.name == adapter1_name && !a.ip.is_empty());
    if a1.is_none() {
        return CommandResult::err("未找到有效IP地址的适配器");
    }

    if config.dual_adapter && !adapter2_name.is_empty() && adapter2_name != adapter1_name {
        let a2 = adapters.iter().find(|a| a.name == adapter2_name && !a.ip.is_empty());
        if let Some(a2_ref) = a2 {
            let a1_ref = a1.unwrap();

            // 双适配器错峰并行登录：适配器2延迟1s启动，避免同时登录触发系统封禁
            // thread::scope 安全借用栈数据；panic 降级为 None
            let (r1, r2) = std::thread::scope(|s| {
                let h1 = s.spawn(|| {
                    login_adapter_with_log(a1_ref, &config, app_handle, state.exit.is_quitting.as_ref())
                });
                let h2 = s.spawn(|| {
                    // sleep 拆分为 10×100ms 循环，每次检查 is_quitting，确保退出时适配器2不再发起登录
                    for _ in 0..10 {
                        if state.exit.is_quitting.load(std::sync::atomic::Ordering::Acquire) {
                            return None;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    login_adapter_with_log(a2_ref, &config, app_handle, state.exit.is_quitting.as_ref())
                });
                let r1 = h1.join().unwrap_or_else(|_| None);
                let r2 = h2.join().unwrap_or_else(|_| None);
                (r1, r2)
            });

            let a1_success = r1.as_ref().map(|r| r.success).unwrap_or(false);
            let a2_success = r2.as_ref().map(|r| r.success).unwrap_or(false);

            let a1_msg = r1.as_ref().and_then(|r| r.message.clone()).unwrap_or_default();
            let a2_msg = r2.as_ref().and_then(|r| r.message.clone()).unwrap_or_default();

            let combined_msg = if !a1_msg.is_empty() && !a2_msg.is_empty() {
                format!("{}, {}", a1_msg, a2_msg)
            } else {
                format!("{}{}", a1_msg, a2_msg)
            };

            let result = CommandResult {
                success: a1_success || a2_success,
                message: Some(combined_msg),
                data: Some(serde_json::json!({ "code": if a1_success || a2_success { "0" } else { "1" } })),
            };
            // 双适配器分别计数：对认证失败的适配器单独递增计数，连续5次触发该适配器 MAC 重置
            update_dual_adapter_auth_failure(
                state, app_handle, &r1, &r2,
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
