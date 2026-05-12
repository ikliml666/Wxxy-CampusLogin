use tauri::{AppHandle, Emitter, Manager, State};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use crate::config::Config;
use crate::network::{
    Adapter, get_adapters_cached, check_portal_full,
    do_login_with_retry,
    clear_portal_cache, select_adapter,
    wait_for_adapter,
};
use super::state::{AppState, CommandResult};
use super::system::append_login_history;

pub fn login_adapter_with_log(
    adapter: &Adapter,
    config: &Config,
    app_handle: &AppHandle,
    is_quitting: &AtomicBool,
) -> Option<CommandResult> {
    if adapter.ip.is_empty() {
        return None;
    }

    if let Ok(sec_status) = check_portal_full(&adapter.ip, Some(&adapter.name), Some(&config.user), Some(&config.password)) {
        if sec_status.online {
            return Some(CommandResult {
                success: true,
                message: Some(sec_status.message),
                data: Some(serde_json::json!({ "code": "0" })),
            });
        }
    }

    if let Err(e) = app_handle.emit("login-log", serde_json::json!({
        "message": format!("{} 正在登录...", adapter.name),
        "type": "info"
    })) {
        crate::log_warn!("login", "发送登录日志失败: {}", e);
    }

    match do_login_with_retry(&config.user, &config.password, &config.operator, Some(adapter.ip.as_str()), 3, is_quitting) {
        Ok(result) => {
            let success = result.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
            let message = result.get("message").and_then(|v| v.as_str()).unwrap_or("");
            if !success && message.contains("无法解析登录响应") {
                if !adapter.ip.is_empty() {
                    if let Ok(sec_status) = check_portal_full(&adapter.ip, Some(&adapter.name), Some(&config.user), Some(&config.password)) {
                        if sec_status.online {
                            if let Err(e) = app_handle.emit("login-log", serde_json::json!({
                                "message": format!("{} 已在线", adapter.name),
                                "type": "success"
                            })) {
                                crate::log_warn!("login", "发送登录日志失败: {}", e);
                            }
                            return Some(CommandResult {
                                success: true,
                                message: Some(format!("{} 已在线", adapter.name)),
                                data: Some(serde_json::json!({ "code": "0" })),
                            });
                        }
                    }
                }
            }
            let display_msg = format!("{} {}", adapter.name, message);
            if success {
                if let Err(e) = app_handle.emit("login-log", serde_json::json!({
                    "message": format!("{} 登录成功", adapter.name),
                    "type": "success"
                })) {
                    crate::log_warn!("login", "发送登录日志失败: {}", e);
                }
                if let Err(e) = append_login_history(app_handle, true, message, &adapter.name, &config.user, "login") {
                    crate::log_warn!("login", "记录登录历史失败: {}", e);
                }
            } else {
                if let Err(e) = app_handle.emit("login-log", serde_json::json!({
                    "message": format!("{} 登录失败: {}", adapter.name, message),
                    "type": "warning"
                })) {
                    crate::log_warn!("login", "发送登录日志失败: {}", e);
                }
                if let Err(e) = append_login_history(app_handle, false, message, &adapter.name, &config.user, "login") {
                    crate::log_warn!("login", "记录登录历史失败: {}", e);
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
                "message": format!("{} 登录请求失败: {}", adapter.name, e),
                "type": "error"
            })) {
                crate::log_warn!("login", "发送登录日志失败: {}", emit_err);
            }
            Some(CommandResult {
                success: false,
                message: Some(format!("{} 登录请求失败: {}", adapter.name, e)),
                data: Some(serde_json::json!({ "code": "error", "message": e })),
            })
        }
    }
}

pub fn full_login_inner(state: &AppState, app_handle: &AppHandle) -> CommandResult {
    let config = {
        let guard = state.config.load();
        if guard.user.is_empty() || guard.password.is_empty() {
            crate::log_warn!("login", "登录失败: 用户名或密码为空");
            return CommandResult::err("用户名或密码为空");
        }
        guard.clone()
    };

    crate::log_info!("login", "开始登录, 用户: {}{}, 双适配器: {}", config.user, config.operator, config.dual_adapter);

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

                let a1_clone = a1_ref.clone();
                let cfg1 = Arc::clone(&config);
                let app_h1 = app_handle.clone();

                let a2_clone = a2_ref.clone();
                let cfg2 = Arc::clone(&config);
                let app_h2 = app_handle.clone();

                let is_quitting = &state.exit.is_quitting;
                let (r1, r2) = std::thread::scope(|s| {
                    let q1 = is_quitting.as_ref();
                    let q2 = is_quitting.as_ref();
                    let t1 = s.spawn(move || login_adapter_with_log(&a1_clone, &*cfg1, &app_h1, q1));
                    let t2 = s.spawn(move || login_adapter_with_log(&a2_clone, &*cfg2, &app_h2, q2));
                    (t1.join().unwrap_or(None), t2.join().unwrap_or(None))
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

#[tauri::command]
pub async fn do_login(state: State<'_, AppState>, app_handle: AppHandle) -> Result<CommandResult, String> {
    state.check_login_rate_limit()?;
    if state.tasks.is_logging_in.swap_acquire() {
        return Ok(CommandResult::err("登录正在进行中"));
    }
    state.exit.auto_exit_cancelled.store(false, Ordering::Release);
    state.network.has_logged_online.store(false, Ordering::Release);
    clear_portal_cache();

    let result = {
        let app_h = app_handle.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let s = app_h.state::<AppState>();
            struct LoginGuard<'a>(&'a crate::commands::state::AppState);
            impl Drop for LoginGuard<'_> {
                fn drop(&mut self) {
                    self.0.tasks.is_logging_in.force_release();
                }
            }
            let _guard = LoginGuard(&s);
            full_login_inner(&s, &app_h)
        }).await.map_err(|e| format!("登录任务失败: {}", e))?
    };

    if result.success {
        state.network.cached_online_status.store(Arc::new(None));
        clear_portal_cache();

        let app_h_bg = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let s = app_h_bg.state::<AppState>();
            super::background::run_background_check(&app_h_bg, &s).await;
        });

        let config = state.config.load_full();
        if config.auto_exit_after_login {
            let s = app_handle.state::<AppState>();
            super::auto_exit::start_auto_exit(&app_handle, &s);
        }
    }

    Ok(result)
}
