use tauri::{AppHandle, Manager, State};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use crate::infra::events::EventBus;
use crate::network::get_adapters_cached;
use crate::auth::portal::check_portal_full;
use crate::auth::failure_tracker;
use crate::infra::state::{AppState, CommandResult};

struct AdapterOnlineStatus {
    any_online: bool,
    a1_online: bool,
    a2_online: bool,
}

fn check_any_adapter_online(state: &AppState) -> AdapterOnlineStatus {
    let adapters = match get_adapters_cached() {
        Ok(a) => a,
        Err(_) => return AdapterOnlineStatus { any_online: false, a1_online: false, a2_online: false },
    };
    let config = state.config.load_full();
    let (a1_name, a2_name) = crate::network::resolve_adapter_names(&adapters, &config);

    let check_one = |name: &str| -> bool {
        adapters.iter().find(|a| a.name == name && !a.ip.is_empty())
            .map(|a| check_portal_full(&a.ip, Some(&a.name), None, None, None)
                .map(|ps| ps.online).unwrap_or(false))
            .unwrap_or(false)
    };

    let a1_online = check_one(&a1_name);
    let a2_online = if config.dual_adapter && !a2_name.is_empty() {
        check_one(&a2_name)
    } else {
        false
    };

    AdapterOnlineStatus {
        any_online: a1_online || a2_online,
        a1_online,
        a2_online,
    }
}

#[tauri::command]
pub async fn do_login(state: State<'_, AppState>, app_handle: AppHandle, adapter_name: Option<String>) -> Result<CommandResult, String> {
    state.exit.auto_exit_cancelled.store(false, Ordering::Release);
    // 取消可能残留的自动退出倒计时，避免重新登录后被旧倒计时强制退出
    state.exit.set_deadline(None);

    let result = {
        let adapter = adapter_name.clone();
        let app_h = app_handle.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let s = app_h.state::<AppState>();
            let _guard = match s.tasks.is_logging_in.try_acquire() {
                Some(g) => g,
                None => {
                    crate::log_warn!("login", "登录被拒绝：已有登录任务在进行");
                    return CommandResult::err("登录正在进行中");
                }
            };
            crate::auth::service::full_login(&s, &app_h, adapter.as_deref())
        }).await.map_err(|e| format!("登录任务失败: {e}"))?
    };

    if result.success {
        post_login_handler(&app_handle, &state);
    }

    Ok(result)
}

/// 登录成功后的公共后处理：解除注销保护期、延迟后台检测、按需触发自动退出。
pub fn post_login_handler(app_handle: &AppHandle, state: &AppState) {
    crate::log_info!("login", "登录成功");
    // 手动/快速登录成功后解除注销保护期，避免后台检测强制 online=false 覆盖登录状态
    // 保护期仅用于阻止注销后自动登录立即触发，手动登录不受影响
    state.network.update(|s| s.logout_protected_until = std::time::Instant::now());
    crate::log_debug!("login", "已解除注销保护期");

    let app_h_bg = app_handle.clone();
    let config = state.config.load_full();
    let auto_exit = config.auto_exit_after_login;
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let s = app_h_bg.state::<AppState>();
        // 退出流程已开始时不再执行后台检查或触发自动退出
        if s.exit.is_quitting.load(Ordering::Acquire) {
            return;
        }
        let cancel_token = s.task_manager
            .cancel_token("background_check")
            .unwrap_or_else(|| Arc::new(tokio_util::sync::CancellationToken::new()));
        crate::monitor::watcher::run_background_check(&app_h_bg, cancel_token).await;

        if auto_exit && !s.exit.is_quitting.load(Ordering::Acquire) {
            crate::infra::lifecycle::start_auto_exit(&app_h_bg, &s);
        }
    });
}

#[tauri::command]
pub async fn do_logout(_state: State<'_, AppState>, app_handle: AppHandle, adapter_name: Option<String>) -> Result<CommandResult, String> {
    let (result, any_online_after_logout) = {
        let adapter = adapter_name.clone();
        let app_h = app_handle.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let s = app_h.state::<AppState>();
            let _guard = match s.tasks.is_logging_out.try_acquire() {
                Some(g) => g,
                None => {
                    crate::log_warn!("logout", "注销被拒绝：已有注销任务在进行");
                    return (CommandResult::err("注销正在进行中，请稍后再试"), None);
                }
            };

            let result = crate::auth::service::full_logout(&s, &app_h, adapter.as_deref());

            let any_online_after_logout = if result.success {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let status = check_any_adapter_online(&s);
                if status.any_online {
                    let event_bus = EventBus::new(&app_h);
                    let _ = event_bus.emit_login_log(
                        "页面检测仍显示在线，注销可能未完全生效",
                        "warning",
                    );
                } else {
                    let event_bus = EventBus::new(&app_h);
                    let _ = event_bus.emit_login_log(
                        "注销成功（页面检测已确认离线）",
                        "success",
                    );
                }
                Some(status)
            } else {
                None
            };

            (result, any_online_after_logout)
        }).await.map_err(|e| format!("注销任务失败: {e}"))?
    };

    if result.success {
        let s = app_handle.state::<AppState>();

        if adapter_name.is_none() {
            // 全量注销：重置所有全局标志 + 取消自动退出 + 60秒注销保护期
            crate::log_info!("logout", "全量注销成功，已重置网络状态，60秒注销保护期开始");
            s.exit.auto_exit_cancelled.store(true, Ordering::Release);
            s.exit.set_deadline(None);
            failure_tracker::reset_all(&s);

            // 复用闭包内 check_any_adapter_online 的逐适配器检测结果，避免重复 HTTP 请求
            let status = any_online_after_logout.unwrap_or(AdapterOnlineStatus {
                any_online: false, a1_online: false, a2_online: false,
            });
            s.network.update(|n| {
                n.any_adapter_online = status.any_online;
                n.last_a1_online = status.a1_online;
            });
            let cfg = s.config.load_full();
            if cfg.dual_adapter {
                s.network.update(|n| n.last_a2_online = status.a2_online);
            } else {
                s.network.update(|n| n.last_a2_online = false);
            }

            // 全量注销专属：重置登录/重连状态 + 注销保护期
            let protected_until = std::time::Instant::now() + std::time::Duration::from_secs(60);
            s.network.update(|n| {
                n.has_logged_online = false;
                n.disconnect_reconnect_count = 0;
                n.last_auto_login_attempt = std::time::Instant::now();
                n.logout_protected_until = protected_until;
            });
        } else {
            // 单适配器注销：复用 check_any_adapter_online 的逐适配器检测结果重置状态
            // 避免用原始配置名（自动检测时为空/"自动检测"）比较导致状态残留
            crate::log_info!("logout", "单适配器注销成功: {:?}", adapter_name);
            let status = any_online_after_logout.unwrap_or(AdapterOnlineStatus {
                any_online: false, a1_online: false, a2_online: false,
            });
            let cfg = s.config.load_full();
            s.network.update(|n| {
                n.last_a1_online = status.a1_online;
                if cfg.dual_adapter {
                    n.last_a2_online = status.a2_online;
                } else {
                    n.last_a2_online = false;
                }
                n.any_adapter_online = status.any_online;
            });
        }
    }
    Ok(result)
}
