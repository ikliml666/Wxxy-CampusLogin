use tauri::{AppHandle, Emitter, Manager};
use std::sync::atomic::Ordering;
use std::time::Duration;
use crate::network::{get_adapters_force, check_portal_full};
use super::state::{AppState, CommandResult, atomic_guard};
use super::system::emit_notification;
use super::auto_exit::start_auto_exit; // 耦合点：auto_login 需要在启动登录成功后触发 auto_exit，见 run_auto_login_on_start

const MAX_DISCONNECT_RECONNECT: u32 = 3;
const RECONNECT_REMINDER_INTERVAL: u32 = 10;
const AUTO_LOGIN_COOLDOWN_SECS: u64 = 30;

pub fn try_auto_login_on_preparation(
    app_handle: &AppHandle,
    state: &AppState,
    login_available: bool,
    online: bool,
    config: &crate::config::Config,
) {
    if !login_available || online || !config.auto_login_on_preparation {
        return;
    }

    if state.network.has_logged_online.load(Ordering::Acquire) {
        return;
    }

    let last_attempt = state.network.last_auto_login_attempt.load();
    if last_attempt.elapsed().as_secs() < AUTO_LOGIN_COOLDOWN_SECS {
        crate::log_debug!("background", "自动登录冷却中（{}秒内不重复），跳过", AUTO_LOGIN_COOLDOWN_SECS);
        return;
    }

    crate::log_info!("background", "触发自动登录: loginAvailable={}, online={}", login_available, online);
    if state.tasks.is_logging_in.try_acquire().is_some() {
        state.network.last_auto_login_attempt.store(std::sync::Arc::new(std::time::Instant::now()));
        atomic_guard!(LoginGuard, is_logging_in);
        let _login_guard = LoginGuard(state);
        let login_result = super::login::full_login_inner(state, app_handle);

        if let Err(e) = app_handle.emit("auto-login-result", serde_json::json!({
            "success": login_result.success,
            "message": login_result.message.clone().unwrap_or_default(),
        })) {
            crate::log_warn!("auto_login", "发送自动登录结果失败: {}", e);
        }

        if login_result.success {
            state.network.has_logged_online.store(true, Ordering::Release);
            if config.enable_notification {
                emit_notification(app_handle, "自动登录成功", &login_result.message.unwrap_or_default());
            }
        }
    }
}

pub fn try_disconnect_reconnect(
    app_handle: &AppHandle,
    state: &AppState,
    online: bool,
    secondary_online: Option<bool>,
    a1: Option<&crate::network::Adapter>,
    adapter1_name: &str,
    adapter2_name: &str,
    reachable: bool,
    login_available: bool,
    config: &crate::config::Config,
) {
    let any_offline = (!online && a1.is_some()) || secondary_online == Some(false);

    if !state.network.was_online.load(Ordering::Acquire) || !any_offline || !reachable || !login_available || !config.auto_login_on_preparation {
        return;
    }

    let last_attempt = state.network.last_auto_login_attempt.load();
    if last_attempt.elapsed().as_secs() < AUTO_LOGIN_COOLDOWN_SECS {
        return;
    }

    let reconnect_count = state.network.disconnect_reconnect_count.fetch_add(1, Ordering::Relaxed) + 1;
    if reconnect_count <= MAX_DISCONNECT_RECONNECT {
        let offline_adapter = if !online { adapter1_name } else { adapter2_name };
        emit_notification(app_handle, "检测到断线", &format!("{} 已离线，正在自动重连 ({}/{})", offline_adapter, reconnect_count, MAX_DISCONNECT_RECONNECT));

        if state.tasks.is_logging_in.try_acquire().is_some() {
            state.network.last_auto_login_attempt.store(std::sync::Arc::new(std::time::Instant::now()));
            atomic_guard!(LoginGuard2, is_logging_in);
            let _login_guard2 = LoginGuard2(state);
            let reconnect_result = super::login::full_login_inner(state, app_handle);

            if reconnect_result.success {
                state.network.disconnect_reconnect_count.store(0, Ordering::Release);
                state.network.was_online.store(true, Ordering::Release);
                state.network.has_logged_online.store(true, Ordering::Release);
                if let Err(e) = super::system::append_login_history(app_handle, true, "断线重连成功", offline_adapter, &config.user, "reconnect") {
                    crate::log_warn!("auto_login", "记录重连历史失败: {}", e);
                }
                if let Err(e) = app_handle.emit("auto-login-result", serde_json::json!({
                    "success": true,
                    "message": format!("断线重连成功: {}", reconnect_result.message.unwrap_or_default()),
                })) {
                    crate::log_warn!("auto_login", "发送重连结果失败: {}", e);
                }
            }
        }
    } else if reconnect_count == MAX_DISCONNECT_RECONNECT + 1 {
        emit_notification(app_handle, "断线重连失败", "已达到最大重连次数，请手动登录");
    } else if reconnect_count > MAX_DISCONNECT_RECONNECT + 1 && (reconnect_count - MAX_DISCONNECT_RECONNECT) % RECONNECT_REMINDER_INTERVAL == 0 {
        emit_notification(app_handle, "网络仍断线", &format!("{} 仍处于离线状态，请手动登录或检查网络", if !online { adapter1_name } else { adapter2_name }));
    }
}

pub fn run_auto_login_on_start(app_handle: &AppHandle) {
    let s = app_handle.state::<AppState>();
    let config = s.config.load_full();

    if !config.auto_login_on_start {
        return;
    }

    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1500)).await;

        let s = app_h.state::<AppState>();
        if s.exit.is_quitting.load(Ordering::Acquire) || s.network.has_logged_online.load(Ordering::Acquire) {
            return;
        }

        let adapters = match tauri::async_runtime::spawn_blocking(move || {
            get_adapters_force()
        }).await {
            Ok(Ok(a)) => a,
            _ => return,
        };

        let s = app_h.state::<AppState>();
        let config = s.config.load_full();

        let (adapter1_name, _) = crate::network::resolve_adapter_names(&adapters, &config);
        let user_account = if !config.operator.is_empty() && config.operator != "__default__" {
            format!("{}@{}", config.user, config.operator)
        } else {
            config.user.clone()
        };
        let user_password = config.password.clone();

        if let Some(a1) = adapters.iter().find(|a| a.name == adapter1_name && !a.ip.is_empty()) {
            let ip1 = a1.ip.clone();
            let name1 = a1.name.clone();
            let (_, adapter2_name) = crate::network::resolve_adapter_names(&adapters, &config);
            let a2_opt = if config.dual_adapter && !adapter2_name.is_empty() {
                adapters.iter().find(|a| a.name == adapter2_name && !a.ip.is_empty()).cloned()
            } else {
                None
            };

            let portal_result = if let Some(a2) = a2_opt {
                let ip2 = a2.ip.clone();
                let name2 = a2.name.clone();
                let ua1 = user_account.clone();
                let up1 = user_password.clone();
                let r1 = tauri::async_runtime::spawn_blocking(move || check_portal_full(&ip1, Some(&name1), Some(&ua1), Some(&up1))).await;
                let ua2 = user_account.clone();
                let up2 = user_password.clone();
                let r2 = tauri::async_runtime::spawn_blocking(move || check_portal_full(&ip2, Some(&name2), Some(&ua2), Some(&up2))).await;
                (r1, Some(r2))
            } else {
                let ua = user_account.clone();
                let up = user_password.clone();
                let r1 = tauri::async_runtime::spawn_blocking(move || check_portal_full(&ip1, Some(&name1), Some(&ua), Some(&up))).await;
                (r1, None)
            };

            if let (Ok(Ok(portal_status)), sec_res) = portal_result {
                if portal_status.online {
                    let sec_msg = match sec_res {
                        Some(Ok(Ok(sec_status))) => format!(", {}", sec_status.message),
                        _ => String::new(),
                    };

                    let _ = app_h.emit("auto-login-result", serde_json::json!({ // [忽略错误] 前端可能未加载，emit 失败不影响登录逻辑
                        "success": true,
                        "message": format!("{}{}", portal_status.message, sec_msg),
                        "skipped": true,
                    })).map_err(|e| crate::log_warn!("auto_login", "发送跳过登录结果失败: {}", e));
                    return;
                }
            }
        }

        if s.tasks.is_logging_in.try_acquire().is_some() {
            s.network.last_auto_login_attempt.store(std::sync::Arc::new(std::time::Instant::now()));
            let app_h_login = app_h.clone();
            let login_result = tauri::async_runtime::spawn_blocking(move || {
                let s = app_h_login.state::<AppState>();
                if s.network.has_logged_online.load(Ordering::Acquire) {
                    s.tasks.is_logging_in.force_release();
                    return CommandResult { success: true, message: Some("已在线，跳过登录".to_string()), data: None };
                }
                atomic_guard!(AutoLoginGuard, is_logging_in);
                let _guard = AutoLoginGuard(&s);
                super::login::full_login_inner(&s, &app_h_login)
            }).await;

            if let Ok(login_result) = login_result {
                if let Err(e) = app_h.emit("auto-login-result", serde_json::json!({
                    "success": login_result.success,
                    "message": login_result.message.clone().unwrap_or_default(),
                })) {
                    crate::log_warn!("auto_login", "发送自动登录结果失败: {}", e);
                }

                if login_result.success {
                    let s = app_h.state::<AppState>();
                    s.network.has_logged_online.store(true, Ordering::Release);
                    if config.enable_notification {
                        emit_notification(&app_h, "自动登录成功", &login_result.message.unwrap_or_default());
                    }

                    if config.auto_exit_after_login {
                        let s = app_h.state::<AppState>();
                        start_auto_exit(&app_h, &s);
                    }
                }
            }
        }
    });
}
