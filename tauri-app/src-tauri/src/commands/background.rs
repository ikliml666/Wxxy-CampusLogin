use tauri::{AppHandle, Emitter, Manager, State};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use crate::network::{
    Adapter, DisabledAdapter, get_adapters_cached, get_adapters_force,
    get_disabled_adapters_force,
    check_portal_full, check_network_quality_async,
};
use super::state::{AppState, CommandResult, AUTO_EXIT_DELAY_MS, CANCEL_EXIT_SHORTCUT, atomic_guard};
use super::system::emit_notification;

fn adapter_status_entry(name: &str, ip: &str, wireless: bool, online: bool, message: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name, "ip": ip, "wireless": wireless,
        "online": online, "message": message
    })
}

fn adapter_disabled_entry(name: &str) -> serde_json::Value {
    adapter_status_entry(name, "", false, false, "适配器已禁用或未找到")
}

fn adapter_disconnected_entry(name: &str, wireless: bool) -> serde_json::Value {
    adapter_status_entry(name, "", wireless, false, "适配器未连接")
}

const ADAPTER_WATCH_INTERVAL: u64 = 15000;
const MAX_DISCONNECT_RECONNECT: u32 = 3;

fn run_background_check_blocking(app_handle: &AppHandle, state: &AppState) -> Option<(String, String)> {
    if state.is_checking.swap(true, Ordering::Acquire) || state.is_quitting.load(Ordering::Acquire) {
        return None;
    }

    atomic_guard!(CheckGuard, is_checking);
    let _check_guard = CheckGuard(state);

    let config = state.config.load_full();
    crate::log_debug!("background", "开始后台检测 #{}", state.background_check_count.load(Ordering::Relaxed) + 1);

    let check_count = state.background_check_count.fetch_add(1, Ordering::Relaxed) + 1;

    let adapters = match get_adapters_cached() {
        Ok(a) if !a.is_empty() => a,
        _ => match get_adapters_force() {
            Ok(a) => a,
            Err(_) => return None,
        }
    };

    let (adapter1_name, adapter2_name) = crate::network::resolve_adapter_names(&adapters, &config);

    let a1 = adapters.iter().find(|a| a.name == adapter1_name && !a.ip.is_empty());
    let a2 = if config.dual_adapter && !adapter2_name.is_empty() {
        adapters.iter().find(|a| a.name == adapter2_name && !a.ip.is_empty())
    } else {
        None
    };

    let (status, secondary_result) = if let Some(adapter) = a1 {
        let a1_ip = adapter.ip.clone();
        let a1_name_clone = adapter1_name.clone();
        if let Some(a2_ref) = a2 {
            let a2_ip = a2_ref.ip.clone();
            let a2_name = a2_ref.name.clone();
            let (primary_res, secondary_res) = std::thread::scope(|s| {
                let a1_ip_s = a1_ip.clone();
                let a1_name_s = a1_name_clone.clone();
                let a2_ip_s = a2_ip.clone();
                let a2_name_s = a2_name.clone();
                let t1 = s.spawn(move || check_portal_full(&a1_ip_s, Some(&a1_name_s)));
                let t2 = s.spawn(move || check_portal_full(&a2_ip_s, Some(&a2_name_s)));
                (t1.join().unwrap_or_else(|_| Err("线程错误".to_string())),
                 t2.join().unwrap_or_else(|_| Err("线程错误".to_string())))
            });
            let status = match primary_res {
                Ok(ps) => serde_json::json!({
                    "online": ps.online,
                    "message": ps.message,
                    "reachable": ps.reachable,
                    "loginAvailable": ps.login_available,
                }),
                Err(_) => serde_json::json!({
                    "online": false,
                    "message": "检测失败",
                    "reachable": false,
                    "loginAvailable": false,
                }),
            };
            let sec = match secondary_res {
                Ok(sec_status) => Some((sec_status.online, sec_status.message)),
                Err(_) => None,
            };
            (status, sec)
        } else {
            let primary_res = check_portal_full(&a1_ip, Some(&a1_name_clone));
            let status = match primary_res {
                Ok(ps) => serde_json::json!({
                    "online": ps.online,
                    "message": ps.message,
                    "reachable": ps.reachable,
                    "loginAvailable": ps.login_available,
                }),
                Err(_) => serde_json::json!({
                    "online": false,
                    "message": "检测失败",
                    "reachable": false,
                    "loginAvailable": false,
                }),
            };
            (status, None)
        }
    } else {
        (serde_json::json!({
            "online": false,
            "message": "未找到主适配器",
            "reachable": false,
            "loginAvailable": false,
        }), None)
    };

    let online = status["online"].as_bool().unwrap_or(false);
    let reachable = status["reachable"].as_bool().unwrap_or(false);
    let login_available = status["loginAvailable"].as_bool().unwrap_or(false);

    let prev_online = state.was_online.load(Ordering::Acquire);
    if online != prev_online {
        crate::log_info!("background", "状态变更: {} → {} ({})", 
            if prev_online { "在线" } else { "离线" },
            if online { "在线" } else { "离线" },
            status["message"].as_str().unwrap_or(""));
        state.was_online.store(online, Ordering::Release);
    } else {
        crate::log_debug!("background", "检测结果: online={}, reachable={}, loginAvailable={}, checkCount={}", online, reachable, login_available, check_count);
    }

    state.server_available.store(reachable, Ordering::Release);

    if !online {
        state.has_logged_online.store(false, Ordering::Release);
    }

    let mut secondary_online: Option<bool> = None;
    let mut secondary_message = String::new();

    if let Some((sec_online, sec_msg)) = secondary_result {
        secondary_online = Some(sec_online);
        secondary_message = sec_msg;
    }

    let _ = app_handle.emit("background-check-result", serde_json::json!({
        "serverAvailable": reachable,
        "loginAvailable": login_available,
        "online": online,
        "message": status["message"].as_str().unwrap_or(""),
        "secondaryOnline": secondary_online,
        "secondaryMessage": secondary_message,
        "timestamp": chrono::Utc::now().timestamp_millis(),
        "checkCount": check_count,
    }));

    {
        let mut adapter_statuses = Vec::new();
        if let Some(a1_ref) = a1 {
            adapter_statuses.push(adapter_status_entry(&adapter1_name, &a1_ref.ip, a1_ref.wireless, online, status["message"].as_str().unwrap_or("")));
        } else if !adapter1_name.is_empty() {
            adapter_statuses.push(adapter_disabled_entry(&adapter1_name));
        }
        if config.dual_adapter && !adapter2_name.is_empty() {
            if let Some(a2_ref) = a2 {
                if a2_ref.ip.is_empty() {
                    adapter_statuses.push(adapter_disconnected_entry(&adapter2_name, a2_ref.wireless));
                } else {
                    adapter_statuses.push(adapter_status_entry(&adapter2_name, &a2_ref.ip, a2_ref.wireless, secondary_online.unwrap_or(false), &secondary_message));
                }
            } else {
                adapter_statuses.push(adapter_disabled_entry(&adapter2_name));
            }
        }
        let any_online = online || secondary_online == Some(true);
        let cached_value = serde_json::json!({
            "serverAvailable": reachable,
            "loginPreparationMode": config.auto_login_on_preparation,
            "checkCount": check_count,
            "isRunning": state.background_running.load(Ordering::Acquire),
            "interval": config.background_check_interval,
            "enabled": config.enable_background_check,
            "adapterStatuses": adapter_statuses,
            "online": any_online,
        });
        state.cached_online_status.store(Arc::new(Some(cached_value)));
    }

    let should_check_quality = online && a1.is_some() && config.enable_network_quality;

    if login_available && !online && config.auto_login_on_preparation {
        crate::log_info!("background", "触发自动登录: loginAvailable={}, online={}", login_available, online);
        if state.is_logging_in.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
            atomic_guard!(LoginGuard, is_logging_in);
            let _login_guard = LoginGuard(state);
            let login_result = super::login::full_login_inner(state, app_handle);

            let _ = app_handle.emit("auto-login-result", serde_json::json!({
                "success": login_result.success,
                "message": login_result.message.clone().unwrap_or_default(),
            }));

            if login_result.success {
                state.has_logged_online.store(true, Ordering::Release);
                if config.enable_notification {
                    emit_notification(app_handle, "自动登录成功", &login_result.message.unwrap_or_default());
                }
            }
        }
    }

    let any_online = online || secondary_online == Some(true);
    let any_offline = (!online && a1.is_some()) || secondary_online == Some(false);

    if any_online {
        state.was_online.store(true, Ordering::Release);
        state.disconnect_reconnect_count.store(0, Ordering::Release);
    }

    if state.was_online.load(Ordering::Acquire) && any_offline && reachable && login_available && config.auto_login_on_preparation {
        let reconnect_count = state.disconnect_reconnect_count.fetch_add(1, Ordering::Relaxed) + 1;
        if reconnect_count <= MAX_DISCONNECT_RECONNECT {
            let offline_adapter = if !online { &adapter1_name } else { &adapter2_name };
            emit_notification(app_handle, "检测到断线", &format!("{} 已离线，正在自动重连 ({}/{})", offline_adapter, reconnect_count, MAX_DISCONNECT_RECONNECT));

            if state.is_logging_in.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
                atomic_guard!(LoginGuard2, is_logging_in);
                let _login_guard2 = LoginGuard2(state);
                let reconnect_result = super::login::full_login_inner(state, app_handle);

                if reconnect_result.success {
                    state.disconnect_reconnect_count.store(0, Ordering::Release);
                    state.was_online.store(true, Ordering::Release);
                    state.has_logged_online.store(true, Ordering::Release);
                    let _ = super::system::append_login_history(app_handle, true, "断线重连成功", offline_adapter, &config.user, "reconnect");
                    let _ = app_handle.emit("auto-login-result", serde_json::json!({
                        "success": true,
                        "message": format!("断线重连成功: {}", reconnect_result.message.unwrap_or_default()),
                    }));
                }
            }
        } else if reconnect_count == MAX_DISCONNECT_RECONNECT + 1 {
            emit_notification(app_handle, "断线重连失败", "已达到最大重连次数，请手动登录");
        }
    }

    if reachable && !state.has_logged_online.load(Ordering::Acquire) && online {
        state.has_logged_online.store(true, Ordering::Release);
        if state.config.load().auto_exit_on_online {
            start_auto_exit(app_handle, state);
        }
    }

    drop(_check_guard);

    if should_check_quality {
        if let Some(a1_ref) = a1 {
            return Some((a1_ref.name.clone(), a1_ref.ip.clone()));
        }
    }

    None
}

pub async fn run_background_check(app_handle: &AppHandle, _state: &AppState) {
    let app_h = app_handle.clone();
    let quality_info = tauri::async_runtime::spawn_blocking(move || {
        let s = app_h.state::<AppState>();
        run_background_check_blocking(&app_h, &s)
    }).await.unwrap_or_else(|e| {
        crate::log_error!("background", "后台检测异常: {}", e);
        None
    });

    if let Some((adapter_name, adapter_ip)) = quality_info {
        let s = app_handle.state::<AppState>();
        let (skip_ttfb, skip_content) = {
            let cfg = s.config.load();
            (cfg.skip_ttfb_in_latency, cfg.skip_content_in_latency)
        };
        if s.is_quality_checking.swap(true, Ordering::Acquire) {
            return;
        }
        atomic_guard!(QualityGuard, is_quality_checking);
        let _guard = QualityGuard(&s);
        let quality = check_network_quality_async(&adapter_name, &adapter_ip, skip_ttfb, skip_content).await;
        drop(_guard);
        let app_h = app_handle.clone();
        let s = app_h.state::<AppState>();
        let enable_notification = s.config.load().enable_notification;
        let quality_val = serde_json::to_value(&quality).unwrap_or_default();
        let _ = app_handle.emit("network-quality-result", &quality_val);
        notify_network_quality_change(app_handle, &s, &quality_val, enable_notification);
    }
}

fn notify_network_quality_change(app_handle: &AppHandle, state: &AppState, quality: &serde_json::Value, enable_notification: bool) {
    let current = quality["quality"].as_str().unwrap_or("unknown").to_string();

    let should_notify = {
        let last_arc = state.last_network_quality.load();
        let last = last_arc.as_ref().as_ref();
        if !enable_notification {
            None
        } else if let Some(last_q) = last {
            if current != *last_q {
                let bad_levels: &[&str] = &["poor", "bad"];
                let good_levels: &[&str] = &["excellent", "great", "good"];
                let was_bad = bad_levels.contains(&last_q.as_str());
                let is_bad = bad_levels.contains(&current.as_str());
                let was_good = good_levels.contains(&last_q.as_str());
                let is_good = good_levels.contains(&current.as_str());

                if is_bad && !was_bad {
                    Some("bad")
                } else if is_good && !was_good && was_bad {
                    Some("good")
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    };

    if let Some(kind) = should_notify {
        if kind == "bad" {
            emit_notification(app_handle, "网络拥堵", "校园网延迟升高，网络可能拥堵");
        } else {
            emit_notification(app_handle, "网络恢复", "校园网延迟已恢复正常");
        }
    }

    state.last_network_quality.store(Arc::new(Some(current)));
}

pub fn start_auto_exit(app_handle: &AppHandle, state: &AppState) {
    let should_start = {
        let deadline = state.auto_exit_deadline();
        if state.auto_exit_cancelled.load(Ordering::Acquire) || deadline.is_some() {
            false
        } else {
            state.set_auto_exit_deadline(Some(std::time::Instant::now() + Duration::from_millis(AUTO_EXIT_DELAY_MS)));
            true
        }
    };

    if !should_start {
        return;
    }

    let _ = app_handle.emit("auto-exit-countdown", serde_json::json!({
        "delay": AUTO_EXIT_DELAY_MS,
        "shortcut": "Ctrl+Shift+C",
    }));

    emit_notification(app_handle, "即将自动退出", &format!("{}秒后自动退出，按 Ctrl+Shift+C 可取消", AUTO_EXIT_DELAY_MS / 1000));

    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let shortcut_registered = if !app_handle.global_shortcut().is_registered(CANCEL_EXIT_SHORTCUT) {
        app_handle.global_shortcut().register(CANCEL_EXIT_SHORTCUT).is_ok()
    } else {
        true
    };

    if !shortcut_registered {
        let extended_delay = AUTO_EXIT_DELAY_MS * 3;
        state.set_auto_exit_deadline(Some(std::time::Instant::now() + Duration::from_millis(extended_delay)));
        crate::log_warn!("auto_exit", "快捷键注册失败，自动退出倒计时延长至{}秒", extended_delay / 1000);
        emit_notification(app_handle, "快捷键注册失败", &format!("无法注册取消快捷键，{}秒后自动退出", extended_delay / 1000));
    }

    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let sleep_duration = {
            let s = app_h.state::<AppState>();
            let deadline = s.auto_exit_deadline();
            match deadline {
                Some(d) => d.saturating_duration_since(std::time::Instant::now()),
                None => Duration::from_millis(AUTO_EXIT_DELAY_MS),
            }
        };
        tokio::time::sleep(sleep_duration).await;
        let s = app_h.state::<AppState>();
        {
            let deadline = s.auto_exit_deadline();
            if let Some(d) = deadline {
                if std::time::Instant::now() < d {
                    return;
                }
            } else {
                return;
            }
        }
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        if app_h.global_shortcut().is_registered(CANCEL_EXIT_SHORTCUT) {
            let _ = app_h.global_shortcut().unregister(CANCEL_EXIT_SHORTCUT);
        }
        s.is_quitting.store(true, Ordering::Release);
        app_h.exit(0);
    });
}

pub fn cancel_auto_exit_inner(app_handle: &AppHandle, state: &AppState) -> Result<CommandResult, String> {
    {
        let deadline = state.auto_exit_deadline();
        if deadline.is_none() {
            return Ok(CommandResult::ok_msg("无需取消"));
        }
        state.set_auto_exit_deadline(None);
    }
    state.auto_exit_cancelled.store(true, Ordering::Release);

    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    if app_handle.global_shortcut().is_registered(CANCEL_EXIT_SHORTCUT) {
        let _ = app_handle.global_shortcut().unregister(CANCEL_EXIT_SHORTCUT);
    }

    emit_notification(app_handle, "已取消退出", "自动退出已取消，程序将继续运行");

    let _ = app_handle.emit("auto-exit-cancelled", serde_json::json!({}));

    Ok(CommandResult::ok_msg("自动退出已取消"))
}

fn start_background_check_inner(app_handle: &AppHandle, state: &AppState) -> Result<CommandResult, String> {
    if state.background_running.swap(true, Ordering::Acquire) {
        return Ok(CommandResult::ok_msg("后台检测已在运行"));
    }

    let (interval, cfg) = {
        let current = state.config.load();
        let mut cfg = current.as_ref().clone();
        cfg.enable_background_check = true;
        if cfg.background_check_interval < 10000 {
            cfg.background_check_interval = 60000;
        }
        let interval = cfg.background_check_interval;
        state.config.store(Arc::new(cfg.clone()));
        (interval, cfg)
    };

    let _ = super::config_cmd::save_config_to_disk(app_handle, &cfg);

    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let s = app_h.state::<AppState>();
        run_background_check(&app_h, &s).await;

        let mut interval_timer = tokio::time::interval(Duration::from_millis(interval));
        loop {
            interval_timer.tick().await;
            let s = app_h.state::<AppState>();
            if !s.background_running.load(Ordering::Acquire) || s.is_quitting.load(Ordering::Acquire) {
                break;
            }
            run_background_check(&app_h, &s).await;
        }
    });

    Ok(CommandResult::ok_msg("后台检测已启动"))
}

#[tauri::command]
pub fn start_background_check(app_handle: AppHandle, _state: State<'_, AppState>) -> Result<CommandResult, String> {
    let s = app_handle.state::<AppState>();
    start_background_check_inner(&app_handle, &s)
}

#[tauri::command]
pub fn stop_background_check(_state: State<'_, AppState>, app_handle: AppHandle) -> Result<CommandResult, String> {
    let s = app_handle.state::<AppState>();
    s.background_running.store(false, Ordering::Release);
    let cfg = {
        let current = s.config.load();
        let mut cfg = current.as_ref().clone();
        cfg.enable_background_check = false;
        s.config.store(Arc::new(cfg.clone()));
        cfg
    };
    let _ = super::config_cmd::save_config_to_disk(&app_handle, &cfg);
    Ok(CommandResult::ok_msg("后台检测已停止"))
}

#[tauri::command]
pub fn trigger_background_check(_state: State<'_, AppState>, app_handle: AppHandle) -> Result<CommandResult, String> {
    let s = app_handle.state::<AppState>();
    if s.is_checking.load(Ordering::Acquire) {
        return Ok(CommandResult::err("检测正在进行中"));
    }
    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let s = app_h.state::<AppState>();
        run_background_check(&app_h, &s).await;
    });
    Ok(CommandResult::ok_msg("已触发后台检测"))
}

#[tauri::command]
pub async fn get_background_status(app_handle: AppHandle) -> Result<serde_json::Value, String> {
    let state = app_handle.state::<AppState>();
    let config = state.config.load_full();
    let running = state.background_running.load(Ordering::Acquire);
    let count = state.background_check_count.load(Ordering::Relaxed);
    let server_avail = state.server_available.load(Ordering::Acquire);

    let cached_adapter_statuses = {
        let cached_arc = state.cached_online_status.load();
        cached_arc.as_ref().as_ref().and_then(|v| v.get("adapterStatuses").cloned())
    };

    let adapter_statuses = if let Some(statuses) = cached_adapter_statuses {
        statuses
    } else {
        let mut adapter_statuses = Vec::new();

        if let Ok(adapters) = get_adapters_cached() {
            let (adapter1_name, adapter2_name) = crate::network::resolve_adapter_names(&adapters, &config);

            if let Some(a1) = adapters.iter().find(|a| a.name == adapter1_name) {
                if a1.ip.is_empty() {
                    adapter_statuses.push(adapter_disconnected_entry(&adapter1_name, a1.wireless));
                } else {
                    adapter_statuses.push(adapter_status_entry(&adapter1_name, &a1.ip, a1.wireless, false, "待检测"));
                }
            } else if !adapter1_name.is_empty() {
                adapter_statuses.push(adapter_disabled_entry(&adapter1_name));
            }

            if config.dual_adapter && !adapter2_name.is_empty() {
                if let Some(a2) = adapters.iter().find(|a| a.name == adapter2_name) {
                    if a2.ip.is_empty() {
                        adapter_statuses.push(adapter_disconnected_entry(&adapter2_name, a2.wireless));
                    } else {
                        adapter_statuses.push(adapter_status_entry(&adapter2_name, &a2.ip, a2.wireless, false, "待检测"));
                    }
                } else {
                    adapter_statuses.push(adapter_disabled_entry(&adapter2_name));
                }
            }
        }

        serde_json::Value::Array(adapter_statuses)
    };

    let any_online = adapter_statuses.as_array().map(|arr| arr.iter().any(|s| s["online"].as_bool().unwrap_or(false))).unwrap_or(false);

    let result = serde_json::json!({
        "serverAvailable": server_avail,
        "loginPreparationMode": config.auto_login_on_preparation,
        "checkCount": count,
        "isRunning": running,
        "interval": config.background_check_interval,
        "enabled": config.enable_background_check,
        "adapterStatuses": adapter_statuses,
        "online": any_online,
    });

    state.cached_online_status.store(Arc::new(Some(result.clone())));

    Ok(result)
}

pub fn start_adapter_watch(app_handle: &AppHandle) {
    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let mut last_adapters: Vec<Adapter> = Vec::new();
        let mut last_disabled: Vec<DisabledAdapter> = Vec::new();
        let mut interval_timer = tokio::time::interval(Duration::from_millis(ADAPTER_WATCH_INTERVAL));
        interval_timer.tick().await;

        loop {
            interval_timer.tick().await;

            let s = app_h.state::<AppState>();
            if s.is_quitting.load(Ordering::Acquire) {
                break;
            }

            let adapters_result = tauri::async_runtime::spawn_blocking(|| {
                get_adapters_force()
            }).await;

            if let Ok(Ok(adapters)) = adapters_result {
                let changed = adapters.len() != last_adapters.len()
                    || adapters.iter().zip(last_adapters.iter()).any(|(a, b)| a.name != b.name || a.ip != b.ip);

                if changed {
                    let _ = app_h.emit("adapters-changed", &adapters);

                    let disabled_result = tauri::async_runtime::spawn_blocking(|| {
                        get_disabled_adapters_force()
                    }).await;

                    if let Ok(Ok(disabled)) = disabled_result {
                        let disabled_changed = disabled.len() != last_disabled.len()
                            || disabled.iter().zip(last_disabled.iter()).any(|(a, b)| a.name != b.name);

                        if disabled_changed {
                            let _ = app_h.emit("disabled-adapters-changed", &disabled);
                            let should_notify = {
                                let s = app_h.state::<AppState>();
                                let elapsed = s.last_disabled_notification_elapsed();
                                match elapsed {
                                    Some(d) => d >= Duration::from_secs(60),
                                    None => true,
                                }
                            };
                            if should_notify {
                                let s = app_h.state::<AppState>();
                                s.set_last_disabled_notification();
                                let config = {
                                    let c = s.config.load();
                                    (c.adapter1.clone(), c.adapter2.clone(), c.dual_adapter)
                                };
                                let (adapter1, adapter2, dual_adapter) = config;
                                let configured_names: Vec<&str> = if dual_adapter && !adapter2.is_empty() && adapter2.as_str() != "自动检测" {
                                    vec![&adapter1, &adapter2]
                                } else if !adapter1.is_empty() && adapter1.as_str() != "自动检测" {
                                    vec![&adapter1]
                                } else {
                                    vec![]
                                };
                                for da in &disabled {
                                    if !last_disabled.iter().any(|ld| ld.name == da.name) {
                                        if configured_names.iter().any(|n| *n == da.name) {
                                            let _ = app_h.emit("adapter-disabled-warning", serde_json::json!({
                                                "name": da.name,
                                                "message": format!("适配器{} 当前已禁用，请启用后重试", da.name),
                                            }));
                                        }
                                    }
                                }
                            }
                        }
                        last_disabled = disabled;
                    }

                    last_adapters = adapters;
                }
            }
        }
    });
}

pub fn spawn_latency_test_loop(app_handle: &AppHandle, interval: u64) {
    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval_timer = tokio::time::interval(Duration::from_millis(interval));
        loop {
            interval_timer.tick().await;
            let s = app_h.state::<AppState>();
            if !s.latency_running.load(Ordering::Acquire) || s.is_quitting.load(Ordering::Acquire) {
                break;
            }
            let (adapter_ip, adapter_name) = {
                let config = s.config.load();
                let adapters = match get_adapters_cached() {
                    Ok(a) => a,
                    Err(_) => continue,
                };
                crate::network::select_adapter(&adapters, &config)
            };
            if adapter_ip.is_empty() {
                continue;
            }
            let (skip_ttfb, skip_content) = {
                let cfg = s.config.load();
                (cfg.skip_ttfb_in_latency, cfg.skip_content_in_latency)
            };
            if s.is_quality_checking.swap(true, Ordering::Acquire) {
                continue;
            }
            atomic_guard!(LatencyGuard, is_quality_checking);
            let _guard = LatencyGuard(&s);
            let quality = check_network_quality_async(&adapter_name, &adapter_ip, skip_ttfb, skip_content).await;
            drop(_guard);
            let quality_val = serde_json::to_value(&quality).unwrap_or_default();
            let _ = app_h.emit("network-quality-result", &quality_val);
            let s = app_h.state::<AppState>();
            let enable_notification = s.config.load().enable_notification;
            notify_network_quality_change(&app_h, &s, &quality_val, enable_notification);
        }
    });
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
        if s.is_quitting.load(Ordering::Acquire) || s.has_logged_online.load(Ordering::Acquire) {
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
                let (r1, r2) = tokio::join!(
                    tauri::async_runtime::spawn_blocking(move || check_portal_full(&ip1, Some(&name1))),
                    tauri::async_runtime::spawn_blocking(move || check_portal_full(&ip2, Some(&name2))),
                );
                (r1, Some(r2))
            } else {
                let r1 = tauri::async_runtime::spawn_blocking(move || check_portal_full(&ip1, Some(&name1))).await;
                (r1, None)
            };

            if let (Ok(Ok(portal_status)), sec_res) = portal_result {
                if portal_status.online {
                    let sec_msg = match sec_res {
                        Some(Ok(Ok(sec_status))) => format!(", {}", sec_status.message),
                        _ => String::new(),
                    };

                    let _ = app_h.emit("auto-login-result", serde_json::json!({
                        "success": true,
                        "message": format!("{}{}", portal_status.message, sec_msg),
                        "skipped": true,
                    }));
                    return;
                }
            }
        }

        if s.is_logging_in.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
            let app_h_login = app_h.clone();
            let login_result = tauri::async_runtime::spawn_blocking(move || {
                let s = app_h_login.state::<AppState>();
                struct AutoLoginGuard<'a>(&'a crate::commands::state::AppState);
                impl Drop for AutoLoginGuard<'_> {
                    fn drop(&mut self) {
                        self.0.is_logging_in.store(false, Ordering::Release);
                    }
                }
                let _guard = AutoLoginGuard(&s);
                super::login::full_login_inner(&s, &app_h_login)
            }).await;

            if let Ok(login_result) = login_result {
                let _ = app_h.emit("auto-login-result", serde_json::json!({
                    "success": login_result.success,
                    "message": login_result.message.clone().unwrap_or_default(),
                }));

                if login_result.success {
                    let s = app_h.state::<AppState>();
                    s.has_logged_online.store(true, Ordering::Release);
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

pub fn run_startup_tasks(app_handle: &AppHandle) {
    let s = app_handle.state::<AppState>();
    let config = s.config.load_full();

    if config.enable_background_check {
        let app_h = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            let s = app_h.state::<AppState>();
            let _ = start_background_check_inner(&app_h, &s);
        });
    }

    if config.enable_network_quality && config.enable_latency_test {
        let app_h = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            let s = app_h.state::<AppState>();
            if !s.latency_running.swap(true, Ordering::Acquire) {
                let interval = {
                    let c = s.config.load();
                    if c.latency_test_interval < 10000 { 30000 } else { c.latency_test_interval }
                };
                spawn_latency_test_loop(&app_h, interval);
            }
        });
    }

    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        run_auto_login_on_start(&app_h);
    });
}
