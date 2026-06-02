use tauri::{AppHandle, Emitter, Manager, State};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use crate::config::model::Config;
use crate::network::{
    Adapter, get_adapters_cached,
    select_adapter,
    wait_for_adapter,
};
use crate::auth::portal::check_portal_full;
use crate::auth::protocol::do_logout_with_retry;
use crate::infra::state::{AppState, CommandResult};

fn logout_adapter_with_log(
    adapter: &Adapter,
    config: &Config,
    app_handle: &AppHandle,
    is_quitting: &AtomicBool,
) -> Option<CommandResult> {
    let adapter_ip = adapter.ip.clone();
    let adapter_if_index = adapter.if_index;
    let adapter_mac = adapter.mac.clone();
    let is_quitting_ref = is_quitting;

    crate::auth::session::adapter_action_with_log(
        adapter, config, app_handle,
        "注销", "logout", "logout",
        || do_logout_with_retry(&config.user, Some(adapter_ip.as_str()), adapter_if_index, &adapter_mac, 2, is_quitting_ref),
    )
}

fn full_logout_inner(state: &AppState, app_handle: &AppHandle, adapter_name: Option<&str>) -> CommandResult {
    let config = {
        let guard = state.config.load();
        if guard.user.is_empty() {
            crate::log_warn!("logout", "注销失败: 用户名为空");
            return CommandResult::err("用户名为空，无法注销");
        }
        guard.clone()
    };

    crate::log_info!("logout", "开始注销, 用户: {}, 指定适配器: {:?}", config.user, adapter_name);

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

    if let Some(name) = adapter_name {
        let adapter = adapters.iter().find(|a| a.name == name && !a.ip.is_empty());
        match adapter {
            Some(a) => {
                return logout_adapter_with_log(a, &config, app_handle, state.exit.is_quitting.as_ref())
                    .unwrap_or_else(|| CommandResult::err("注销请求失败"));
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

                let r1 = logout_adapter_with_log(a1_ref, &config, app_handle, state.exit.is_quitting.as_ref());
                let r2 = logout_adapter_with_log(a2_ref, &config, app_handle, state.exit.is_quitting.as_ref());

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

    logout_adapter_with_log(a1_ref, &config, app_handle, state.exit.is_quitting.as_ref())
        .unwrap_or_else(|| CommandResult::err("注销请求失败"))
}

fn check_any_adapter_online(state: &AppState) -> bool {
    let adapters = match get_adapters_cached() {
        Ok(a) => a,
        Err(_) => return false,
    };
    let config = state.config.load_full();
    let (a1_name, a2_name) = crate::network::resolve_adapter_names(&adapters, &config);

    let names: Vec<&str> = if config.dual_adapter && !a2_name.is_empty() {
        vec![&a1_name, &a2_name]
    } else {
        vec![&a1_name]
    };

    for name in names {
        let adapter = match adapters.iter().find(|a| a.name == name && !a.ip.is_empty()) {
            Some(a) => a,
            None => continue,
        };
        match check_portal_full(&adapter.ip, Some(&adapter.name), None, None, None) {
            Ok(ps) if ps.online => return true,
            _ => continue,
        }
    }

    false
}

#[tauri::command]
pub async fn do_login(state: State<'_, AppState>, app_handle: AppHandle, adapter_name: Option<String>) -> Result<CommandResult, String> {
    state.exit.auto_exit_cancelled.store(false, Ordering::Release);

    let result = {
        let adapter = adapter_name.clone();
        let app_h = app_handle.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let s = app_h.state::<AppState>();
            let _guard = match s.tasks.is_logging_in.try_acquire() {
                Some(g) => g,
                None => return CommandResult::err("登录正在进行中"),
            };
            crate::auth::session::full_login_inner(&s, &app_h, adapter.as_deref())
        }).await.map_err(|e| format!("登录任务失败: {}", e))?
    };

    if result.success {
        let app_h_bg = app_handle.clone();
        let config = state.config.load_full();
        let auto_exit = config.auto_exit_after_login;
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let s = app_h_bg.state::<AppState>();
            let cancel_token = s.tasks.bg_check_cancel.load().clone();
            crate::monitor::watcher::run_background_check(&app_h_bg, cancel_token).await;

            if auto_exit {
                crate::infra::lifecycle::start_auto_exit(&app_h_bg, &s);
            }
        });
    }

    Ok(result)
}

#[tauri::command]
pub async fn do_logout(_state: State<'_, AppState>, app_handle: AppHandle, adapter_name: Option<String>) -> Result<CommandResult, String> {
    let result = {
        let adapter = adapter_name.clone();
        let app_h = app_handle.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let s = app_h.state::<AppState>();
            let _guard = match s.tasks.is_logging_out.try_acquire() {
                Some(g) => g,
                None => return CommandResult::err("注销正在进行中，请稍后再试"),
            };

            let result = full_logout_inner(&s, &app_h, adapter.as_deref());

            if result.success {
                std::thread::sleep(std::time::Duration::from_secs(1));
                if check_any_adapter_online(&s) {
                    let _ = app_h.emit("login-log", serde_json::json!({
                        "message": "页面检测仍显示在线，注销可能未完全生效",
                        "type": "warning"
                    }));
                } else {
                    let _ = app_h.emit("login-log", serde_json::json!({
                        "message": "注销成功（页面检测已确认离线）",
                        "type": "success"
                    }));
                }
            }

            result
        }).await.map_err(|e| format!("注销任务失败: {}", e))?
    };

    if result.success {
        let s = app_handle.state::<AppState>();
        s.exit.auto_exit_cancelled.store(true, Ordering::Release);
        s.exit.set_deadline(None);
        s.network.any_adapter_online.store(false, Ordering::Release);
        s.network.last_auto_login_attempt.store(std::sync::Arc::new(std::time::Instant::now()));
        let protected_until = std::time::Instant::now() + std::time::Duration::from_secs(60);
        s.network.logout_protected_until.store(std::sync::Arc::new(protected_until));
    }
    Ok(result)
}
