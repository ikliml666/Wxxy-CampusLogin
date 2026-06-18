use tauri::{AppHandle, Emitter, Manager};
use std::sync::atomic::Ordering;
use std::time::Duration;
use chrono::Timelike;
use crate::network::get_adapters_force;
use crate::auth::portal::check_portal_full;
use crate::infra::state::{AppState, CommandResult};
use crate::infra::notification::emit_notification;
use crate::infra::lifecycle::{start_auto_exit, start_campus_exit};

const MAX_DISCONNECT_RECONNECT: u32 = 3;
const RECONNECT_REMINDER_INTERVAL: u32 = 10;
const AUTO_LOGIN_COOLDOWN_SECS: u64 = 60;

pub fn try_auto_login_on_preparation(
    app_handle: &AppHandle,
    state: &AppState,
    login_available: bool,
    online: bool,
    config: &crate::config::model::Config,
) {
    if !login_available || online || !config.auto_login_on_preparation {
        return;
    }

    if state.network.has_logged_online.load(Ordering::Acquire) {
        return;
    }

    let protected_until = state.network.logout_protected_until.load();
    if std::time::Instant::now() < **protected_until {
        crate::log_debug!("auto_login", "注销保护期内，跳过自动登录");
        return;
    }

    let last_attempt = state.network.last_auto_login_attempt.load();
    if last_attempt.elapsed().as_secs() < AUTO_LOGIN_COOLDOWN_SECS {
        crate::log_debug!("auto_login", "自动登录冷却中（{}秒内不重复），跳过", AUTO_LOGIN_COOLDOWN_SECS);
        return;
    }

    crate::log_info!("auto_login", "触发自动登录: loginAvailable={}, online={}", login_available, online);
    if let Some(_login_guard) = state.tasks.is_logging_in.try_acquire() {
        let t0 = std::time::Instant::now();
        state.network.last_auto_login_attempt.store(std::sync::Arc::new(std::time::Instant::now()));
        let login_result = crate::auth::session::full_login_inner(state, app_handle, None);
        let elapsed = t0.elapsed();

        crate::log_info!("login", "自动登录完成: success={}, message={}, 耗时{}ms",
            login_result.success,
            login_result.message.clone().unwrap_or_else(|| "无消息".to_string()),
            elapsed.as_millis());

        if let Err(e) = app_handle.emit("auto-login-result", serde_json::json!({
            "success": login_result.success,
            "message": login_result.message.clone().unwrap_or_default(),
            "skipped": false,
        })) {
            crate::log_warn!("auto_login", "发送自动登录结果失败: {}", e);
        }

        if login_result.success {
            state.network.has_logged_online.store(true, Ordering::Release);
            if config.enable_notification {
                emit_notification(app_handle, "自动登录成功", &login_result.message.unwrap_or_default());
            }
        }
    } else {
        crate::log_debug!("auto_login", "自动登录跳过：已有登录任务在进行");
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
    config: &crate::config::model::Config,
) {
    let any_offline = (!online && a1.is_some()) || secondary_online == Some(false);

    if !state.network.any_adapter_online.load(Ordering::Acquire) || !any_offline || !reachable || !login_available || !config.auto_login_on_preparation {
        return;
    }

    let protected_until = state.network.logout_protected_until.load();
    if std::time::Instant::now() < **protected_until {
        crate::log_debug!("auto_login", "注销保护期内，跳过断线重连");
        return;
    }

    let last_attempt = state.network.last_auto_login_attempt.load();
    if last_attempt.elapsed().as_secs() < AUTO_LOGIN_COOLDOWN_SECS {
        return;
    }

    // 先尝试获取登录锁，成功后再增加重连计数，避免锁获取失败但计数器已增加
    let login_guard = match state.tasks.is_logging_in.try_acquire() {
        Some(g) => g,
        None => {
            crate::log_debug!("auto_login", "断线重连：登录锁被占用，跳过本次");
            return;
        }
    };

    let reconnect_count = state.network.disconnect_reconnect_count.fetch_add(1, Ordering::Relaxed) + 1;
    if reconnect_count <= MAX_DISCONNECT_RECONNECT {
        let offline_adapter = if !online { adapter1_name } else { adapter2_name };
        emit_notification(app_handle, "检测到断线", &format!("{} 已离线，正在自动重连 ({}/{})", offline_adapter, reconnect_count, MAX_DISCONNECT_RECONNECT));

        crate::log_info!("auto_login", "断线重连 [{}/{}]: 离线适配器={}, online={}, secondaryOnline={}",
            reconnect_count, MAX_DISCONNECT_RECONNECT, offline_adapter, online, secondary_online.unwrap_or(true));

        let _login_guard = login_guard;
        let t0 = std::time::Instant::now();
        state.network.last_auto_login_attempt.store(std::sync::Arc::new(std::time::Instant::now()));
        let reconnect_result = crate::auth::session::full_login_inner(state, app_handle, None);
        let elapsed = t0.elapsed();

        crate::log_info!("login", "断线重连结果 [{}/{}]: success={}, 耗时{}ms",
            reconnect_count, MAX_DISCONNECT_RECONNECT, reconnect_result.success, elapsed.as_millis());

        if reconnect_result.success {
            state.network.disconnect_reconnect_count.store(0, Ordering::Release);
            state.network.any_adapter_online.store(true, Ordering::Release);
            state.network.has_logged_online.store(true, Ordering::Release);
            if let Err(e) = crate::commands::system::append_login_history(app_handle, true, "断线重连成功", offline_adapter, &config.user, "reconnect") {
                crate::log_warn!("auto_login", "记录重连历史失败: {}", e);
            }
            if let Err(e) = app_handle.emit("auto-login-result", serde_json::json!({
                "success": true,
                "message": format!("断线重连成功: {}", reconnect_result.message.unwrap_or_default()),
                "skipped": false,
            })) {
                crate::log_warn!("auto_login", "发送重连结果失败: {}", e);
            }
        }
    } else {
        // 超限：重连次数已达上限，释放登录锁，避免阻塞用户手动登录
        drop(login_guard);
        if reconnect_count == MAX_DISCONNECT_RECONNECT + 1 {
            crate::log_warn!("auto_login", "断线重连已达上限({}), 停止自动重连", MAX_DISCONNECT_RECONNECT);
            emit_notification(app_handle, "断线重连失败", "已达到最大重连次数，请手动登录");
        } else if reconnect_count > MAX_DISCONNECT_RECONNECT + 1 && (reconnect_count - MAX_DISCONNECT_RECONNECT - 1) % RECONNECT_REMINDER_INTERVAL == 0 {
            emit_notification(app_handle, "网络仍断线", &format!("{} 仍处于离线状态，请手动登录或检查网络", if !online { adapter1_name } else { adapter2_name }));
        }
    }
}

pub fn run_auto_login_on_start(app_handle: &AppHandle) {
    let s = app_handle.state::<AppState>();
    let config = s.config.load_full();

    if !config.auto_login_on_start {
        return;
    }

    let is_auto_start = std::env::args().any(|a| a == "--autostart");
    let initial_delay = if is_auto_start { 5000u64 } else { 1500u64 };

    crate::log_info!("auto_login", "开机自启登录流程启动: isAutoStart={}, initialDelay={}ms, dualAdapter={}",
        is_auto_start, initial_delay, config.dual_adapter);

    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(initial_delay)).await;

        let s = app_h.state::<AppState>();
        if s.exit.is_quitting.load(Ordering::Acquire) || s.network.has_logged_online.load(Ordering::Acquire) {
            return;
        }

        let t_adapters = std::time::Instant::now();
        let mut adapters = match tauri::async_runtime::spawn_blocking(move || {
            get_adapters_force()
        }).await {
            Ok(Ok(a)) => a,
            _ => {
                crate::log_error!("auto_login", "获取适配器列表失败，终止开机自启登录");
                if let Err(e) = app_h.emit("auto-login-result", serde_json::json!({
                    "success": false,
                    "message": "获取适配器列表失败，终止开机自启登录",
                })) {
                    crate::log_warn!("auto_login", "发送启动登录失败事件失败: {}", e);
                }
                return;
            },
        };

        crate::log_info!("auto_login", "适配器列表获取完成: {}个适配器, 耗时{}ms",
            adapters.len(), t_adapters.elapsed().as_millis());

        for (i, a) in adapters.iter().enumerate() {
            crate::log_debug!("auto_login", "  [{}] name={}, ip={}, wireless={}",
                i, a.name, a.ip, a.wireless);
        }

        if is_auto_start {
            for retry in 0..3 {
                let has_ip = adapters.iter().any(|a| !a.ip.is_empty());
                if has_ip { break; }
                crate::log_debug!("auto_login", "开机自启：适配器未就绪，等待重试 ({}/3)", retry + 1);
                tokio::time::sleep(Duration::from_secs(3)).await;
                let s = app_h.state::<AppState>();
                if s.exit.is_quitting.load(Ordering::Acquire) { return; }
                adapters = match tauri::async_runtime::spawn_blocking(|| {
                    get_adapters_force()
                }).await {
                    Ok(Ok(a)) => a,
                    _ => return,
                };
            }
        }

        let s = app_h.state::<AppState>();
        let config = s.config.load_full();
        let (adapter1_name, adapter2_name) = crate::network::resolve_adapter_names(&adapters, &config);

        if config.enable_network_name_check {
            let skip_campus = config.campus_check_start_minutes > 0 && (chrono::Local::now().hour() as u16 * 60 + chrono::Local::now().minute() as u16) < config.campus_check_start_minutes;

            if skip_campus {
                let hour = config.campus_check_start_minutes / 60;
                let minute = config.campus_check_start_minutes % 60;
                crate::log_info!("auto_login", "开机自启: 校园网检测静默期（当前时间早于{}:{:02}），跳过校园网环境验证", hour, minute);
                s.network.on_campus_network.store(true, std::sync::atomic::Ordering::Release);
            } else {
                // 校园网检测任务异常（panic 或被取消）时不能误判为"不在校园网"，
                // 否则会触发 start_campus_exit 将用户错误踢出。此处直接跳过本次检测。
                let campus_result = match tauri::async_runtime::spawn_blocking({
                    let cfg = config.clone();
                    let adps = adapters.clone();
                    move || crate::monitor::watcher::check_campus_network(&cfg, &adps)
                }).await {
                    Ok(result) => result,
                    Err(e) => {
                        crate::log_error!("auto_login", "校园网检测任务异常，跳过本次检测以避免误触发退出: {}", e);
                        return;
                    }
                };

                if !campus_result.on_campus {
                    crate::log_info!("auto_login", "开机自启: 校园网检测未通过，跳过自动登录 - {}", campus_result.message);
                    s.network.current_ssid.store(std::sync::Arc::new(campus_result.current_ssid));
                    s.network.on_campus_network.store(campus_result.on_campus, std::sync::atomic::Ordering::Release);
                    let _ = app_h.emit("auto-login-result", serde_json::json!({
                        "success": false,
                        "message": campus_result.message,
                        "skipped": true,
                    }));
                    // 如果配置的适配器均无IP（完全无网络），跳过退出，等待网络恢复
                    let a1_has_ip = adapters.iter().any(|a| a.name == adapter1_name && !a.ip.is_empty());
                    let a2_has_ip = config.dual_adapter && !adapter2_name.is_empty()
                        && adapters.iter().any(|a| a.name == adapter2_name && !a.ip.is_empty());
                    if !a1_has_ip && !a2_has_ip {
                        crate::log_info!("auto_login", "配置的适配器均无IP地址，跳过校园网退出，等待网络恢复");
                    } else {
                        // 校园网验证不通过：触发最小化+退出流程
                        start_campus_exit(&app_h, &s);
                    }
                    return;
                }

                s.network.current_ssid.store(std::sync::Arc::new(campus_result.current_ssid));
                s.network.on_campus_network.store(true, std::sync::atomic::Ordering::Release);
                crate::log_info!("auto_login", "开机自启: 校园网检测通过 - {}", campus_result.message);
            }
        }

        let user_account = config.user_account_with_operator();
        let user_password = config.password.clone();
        let operator = config.operator.clone();

        if let Some(a1) = adapters.iter().find(|a| a.name == adapter1_name && !a.ip.is_empty()) {
            let ip1 = a1.ip.clone();
            let name1 = a1.name.clone();
            let ip1_for_log = ip1.clone();
            let a2_opt = if config.dual_adapter && !adapter2_name.is_empty() {
                adapters.iter().find(|a| a.name == adapter2_name && !a.ip.is_empty()).cloned()
            } else {
                None
            };

            let mut name2_opt = None;
            let name1_for_msg = name1.clone();

            let t_portal = std::time::Instant::now();
            let portal_result = if let Some(a2) = a2_opt {
                let ip2 = a2.ip.clone();
                name2_opt = Some(a2.name.clone());
                let ua1 = user_account.clone();
                let up1 = user_password.clone();
                let op1 = operator.clone();
                let ua2 = user_account.clone();
                let up2 = user_password.clone();
                let op2 = operator.clone();
                // 双适配器并行 Portal 检测：先 spawn 两个 handle，再分别 await
                // 原 spawn->await->spawn->await 串行，改为并行可显著缩短双适配器检测耗时
                let h1 = tauri::async_runtime::spawn_blocking(move || check_portal_full(&ip1, Some(&name1), Some(&ua1), Some(&up1), Some(&op1)));
                let h2 = tauri::async_runtime::spawn_blocking(move || check_portal_full(&ip2, Some(&a2.name), Some(&ua2), Some(&up2), Some(&op2)));
                let r1 = h1.await;
                let r2 = h2.await;
                (r1, Some(r2))
            } else {
                let ua = user_account.clone();
                let up = user_password.clone();
                let op = operator.clone();
                let r1 = tauri::async_runtime::spawn_blocking(move || check_portal_full(&ip1, Some(&name1), Some(&ua), Some(&up), Some(&op))).await;
                (r1, None)
            };

            let portal_elapsed = t_portal.elapsed();

            if let (Ok(Ok(portal_status)), sec_res) = &portal_result {
                crate::log_info!("auto_login", "Portal检测结果: {}[ip={}] → online={}, reachable={}, loginAvailable={}, dataLen={}, 耗时{}ms; 副适配器→ {:?}",
                    name1_for_msg, ip1_for_log, portal_status.online, portal_status.reachable,
                    portal_status.login_available, portal_status.data_length,
                    portal_elapsed.as_millis(),
                    sec_res.as_ref().map(|r| match r {
                        Ok(Ok(s)) => format!("online={}, msg={}", s.online, s.message),
                        Ok(Err(e)) => format!("err={}", e),
                        Err(e) => format!("join_err={}", e),
                    }));

                if portal_status.online {
                    let mut adapter_names = vec![name1_for_msg.clone()];
                    let _sec_online = match sec_res {
                        Some(Ok(Ok(sec_status))) => {
                            if sec_status.online {
                                if let Some(ref n2) = name2_opt {
                                    adapter_names.push(n2.clone());
                                }
                            }
                            sec_status.online
                        },
                        _ => false,
                    };
                    let msg = format!("已在线（{}）", adapter_names.join("、"));

                    s.network.any_adapter_online.store(true, Ordering::Release);
                    s.network.has_logged_online.store(true, Ordering::Release);

                    crate::log_info!("auto_login", "已在线，跳过登录: 适配器=[{}]", adapter_names.join(", "));

                    let _ = app_h.emit("auto-login-result", serde_json::json!({
                        "success": true,
                        "message": msg,
                        "skipped": true,
                    })).map_err(|e| crate::log_warn!("auto_login", "发送跳过登录结果失败: {}", e));
                    return;
                }
            } else {
                match &portal_result {
                    (Err(e), _) => crate::log_warn!("auto_login", "主适配器Portal检测异常: {}, 耗时{}ms", e, portal_elapsed.as_millis()),
                    (_, Some(Err(e))) => crate::log_warn!("auto_login", "副适配器Portal检测异常: {}", e),
                    _ => {}
                }
            }
        } else {
            crate::log_warn!("auto_login", "未找到可用主适配器: 目标={}, 可用适配器数={}",
                adapter1_name, adapters.iter().filter(|a| !a.ip.is_empty()).count());
        }

        let t_login = std::time::Instant::now();
        let app_h_login = app_h.clone();
        let login_result = tauri::async_runtime::spawn_blocking(move || {
            let s = app_h_login.state::<AppState>();
            if s.network.has_logged_online.load(Ordering::Acquire) {
                return CommandResult { success: true, message: Some("已在线，跳过登录".to_string()), data: None };
            }
            let _guard = match s.tasks.is_logging_in.try_acquire() {
                Some(g) => g,
                None => return CommandResult::err("登录正在进行中"),
            };
            s.network.last_auto_login_attempt.store(std::sync::Arc::new(std::time::Instant::now()));
            crate::auth::session::full_login_inner(&s, &app_h_login, None)
        }).await;

        let login_elapsed = t_login.elapsed();

        if let Ok(login_result) = login_result {
            crate::log_info!("login", "开机自启登录结果: success={}, message={}, 耗时{}ms",
                login_result.success,
                login_result.message.clone().unwrap_or_else(|| "无".to_string()),
                login_elapsed.as_millis());

            if let Err(e) = app_h.emit("auto-login-result", serde_json::json!({
                "success": login_result.success,
                "message": login_result.message.clone().unwrap_or_default(),
                "skipped": false,
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
    });
}
