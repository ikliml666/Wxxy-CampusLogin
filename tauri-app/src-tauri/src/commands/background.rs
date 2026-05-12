use tauri::{AppHandle, Emitter, Manager, State};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use crate::network::{
    get_adapters_cached, get_adapters_force,
    check_portal_full, check_network_quality_async,
};
use super::state::{AppState, CommandResult, atomic_guard};
use super::auto_exit::start_auto_exit;
use super::auto_login::{try_auto_login_on_preparation, try_disconnect_reconnect, run_auto_login_on_start};
use super::latency::{notify_network_quality_change, spawn_latency_test_loop};

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

fn run_background_check_blocking(app_handle: &AppHandle, state: &AppState) -> Option<(String, String)> {
    if state.tasks.is_checking.swap_acquire() || state.exit.is_quitting.load(Ordering::Acquire) {
        return None;
    }

    atomic_guard!(CheckGuard, is_checking);
    let _check_guard = CheckGuard(state);

    let config = state.config.load_full();
    let user_account = if !config.operator.is_empty() && config.operator != "__default__" {
        format!("{}@{}", config.user, config.operator)
    } else {
        config.user.clone()
    };
    let user_password = config.password.clone();
    crate::log_debug!("background", "开始后台检测 #{}", state.network.background_check_count.load(Ordering::Relaxed) + 1);

    let check_count = state.network.background_check_count.fetch_add(1, Ordering::Relaxed) + 1;

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
            let primary_res = check_portal_full(&a1_ip, Some(&a1_name_clone), Some(&user_account), Some(&user_password));
            let secondary_res = check_portal_full(&a2_ip, Some(&a2_name), Some(&user_account), Some(&user_password));
            let status = match primary_res {
                Ok(ps) => serde_json::json!({
                    "online": ps.online,
                    "message": ps.message,
                    "reachable": ps.reachable,
                    "loginAvailable": ps.login_available,
                }),
                Err(e) => {
                    crate::log_warn!("background", "主适配器Portal检测异常: {}", e);
                    let _ = app_handle.emit("login-log", serde_json::json!({
                        "message": format!("{} Portal检测异常: {}", adapter1_name, e),
                        "type": "error"
                    }));
                    serde_json::json!({
                        "online": false,
                        "message": "检测失败",
                        "reachable": false,
                        "loginAvailable": false,
                    })
                },
            };
            let sec = match secondary_res {
                Ok(sec_status) => Some((sec_status.online, sec_status.message)),
                Err(e) => {
                    crate::log_warn!("background", "副适配器Portal检测异常: {}", e);
                    let _ = app_handle.emit("login-log", serde_json::json!({
                        "message": format!("{} Portal检测异常: {}", adapter2_name, e),
                        "type": "error"
                    }));
                    None
                },
            };
            (status, sec)
        } else {
            let primary_res = check_portal_full(&a1_ip, Some(&a1_name_clone), Some(&user_account), Some(&user_password));
            let status = match primary_res {
                Ok(ps) => serde_json::json!({
                    "online": ps.online,
                    "message": ps.message,
                    "reachable": ps.reachable,
                    "loginAvailable": ps.login_available,
                }),
                Err(e) => {
                    crate::log_warn!("background", "主适配器Portal检测异常: {}", e);
                    let _ = app_handle.emit("login-log", serde_json::json!({
                        "message": format!("{} Portal检测异常: {}", adapter1_name, e),
                        "type": "error"
                    }));
                    serde_json::json!({
                        "online": false,
                        "message": "检测失败",
                        "reachable": false,
                        "loginAvailable": false,
                    })
                },
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
    let status_msg = status["message"].as_str().unwrap_or("");
    let prev_online = state.network.was_online.load(Ordering::Acquire);

    let is_check_failure = status_msg == "网络检测失败";
    if is_check_failure {
        let failures = state.network.consecutive_check_failures.fetch_add(1, Ordering::Relaxed) + 1;
        if failures < 2 {
            crate::log_info!("background", "网络检测失败({}/2)，保留上次状态: {}", failures, if prev_online { "在线" } else { "离线" });
            return None;
        }
    } else {
        state.network.consecutive_check_failures.store(0, Ordering::Relaxed);
    }

    if online != prev_online {
        let mut adapter_details = vec![format!("{}: {}", adapter1_name, status["message"].as_str().unwrap_or("未知"))];
        if let Some((_sec_online, sec_msg)) = &secondary_result {
            if config.dual_adapter && !adapter2_name.is_empty() {
                adapter_details.push(format!("{}: {}", adapter2_name, sec_msg));
            }
        }
        crate::log_info!("background", "状态变更: {} → {} [{}]",
            if prev_online { "在线" } else { "离线" },
            if online { "在线" } else { "离线" },
            adapter_details.join(", "));

        if !online {
            if config.enable_notification {
                let log_msg = adapter_details.join(", ");
                super::system::emit_notification(app_handle, "网络状态变更", &log_msg);
            }
        }
    } else {
        let mut adapter_details = vec![format!("{}: {}", adapter1_name, status["message"].as_str().unwrap_or("未知"))];
        if let Some((_sec_online, sec_msg)) = &secondary_result {
            if config.dual_adapter && !adapter2_name.is_empty() {
                adapter_details.push(format!("{}: {}", adapter2_name, sec_msg));
            }
        }
        crate::log_debug!("background", "检测结果: online={}, reachable={}, loginAvailable={}, [{}]", online, reachable, login_available, adapter_details.join(", "));
    }

    state.network.server_available.store(reachable, Ordering::Release);

    let mut secondary_online: Option<bool> = None;
    let mut secondary_message = String::new();

    if let Some((sec_online, sec_msg)) = secondary_result {
        secondary_online = Some(sec_online);
        secondary_message = sec_msg;
    }

    if let Err(e) = app_handle.emit("background-check-result", serde_json::json!({
        "serverAvailable": reachable,
        "loginAvailable": login_available,
        "online": online,
        "message": status["message"].as_str().unwrap_or(""),
        "adapter1Name": adapter1_name,
        "adapter2Name": if config.dual_adapter { &adapter2_name } else { "" },
        "secondaryOnline": secondary_online,
        "secondaryMessage": secondary_message,
        "timestamp": chrono::Utc::now().timestamp_millis(),
        "checkCount": check_count,
    })) {
        crate::log_warn!("background", "发送后台检测结果失败: {}", e);
    }

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
            "isRunning": state.tasks.background_running.is_active(),
            "interval": config.background_check_interval,
            "enabled": config.enable_background_check,
            "adapterStatuses": adapter_statuses,
            "online": any_online,
        });
        state.network.cached_online_status.store(Arc::new(Some(cached_value)));
    }

    let should_check_quality = online && a1.is_some() && config.enable_network_quality;

    try_auto_login_on_preparation(app_handle, state, login_available, online, &config);

    try_disconnect_reconnect(
        app_handle, state, online, secondary_online,
        a1, &adapter1_name, &adapter2_name,
        reachable, login_available, &config,
    );

    let any_online = online || secondary_online == Some(true);
    state.network.was_online.store(any_online, Ordering::Release);
    if any_online {
        state.network.disconnect_reconnect_count.store(0, Ordering::Release);
    }

    if reachable && !state.network.has_logged_online.load(Ordering::Acquire) && online {
        state.network.has_logged_online.store(true, Ordering::Release);
        if state.config.load().auto_exit_on_online {
            start_auto_exit(app_handle, state);
        }
    }

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
        let (skip_ttfb, skip_content, fixed_gateway) = {
            let cfg = s.config.load();
            (cfg.skip_ttfb_in_latency, cfg.skip_content_in_latency, cfg.fixed_gateway.clone())
        };
        if s.tasks.is_quality_checking.swap_acquire() {
            return;
        }
        atomic_guard!(QualityGuard, is_quality_checking);
        let _guard = QualityGuard(&s);
        let quality = check_network_quality_async(&adapter_name, &adapter_ip, skip_ttfb, skip_content, &fixed_gateway, s.exit.is_quitting.clone()).await;
        drop(_guard);
        let enable_notification = s.config.load().enable_notification;
        let quality_val = serde_json::to_value(&quality).unwrap_or_default();
        if let Err(e) = app_handle.emit("network-quality-result", &quality_val) {
            crate::log_warn!("background", "发送网络质量结果失败: {}", e);
        }
        notify_network_quality_change(app_handle, &s, &quality_val, enable_notification);
    }
}

fn start_background_check_inner(app_handle: &AppHandle, state: &AppState) -> Result<CommandResult, String> {
    if state.tasks.background_running.swap_acquire() {
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

    if let Err(e) = super::config_cmd::save_config_to_disk(app_handle, &cfg) {
        crate::log_warn!("background", "保存后台检测配置失败: {}", e);
    }

    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let s = app_h.state::<AppState>();
        run_background_check(&app_h, &s).await;

        let mut interval_timer = tokio::time::interval(Duration::from_millis(interval));
        loop {
            interval_timer.tick().await;
            let s = app_h.state::<AppState>();
            if !s.tasks.background_running.is_active() || s.exit.is_quitting.load(Ordering::Acquire) {
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
    s.tasks.background_running.force_release();
    let cfg = {
        let current = s.config.load();
        let mut cfg = current.as_ref().clone();
        cfg.enable_background_check = false;
        s.config.store(Arc::new(cfg.clone()));
        cfg
    };
    if let Err(e) = super::config_cmd::save_config_to_disk(&app_handle, &cfg) {
        crate::log_warn!("background", "保存停止检测配置失败: {}", e);
    }
    Ok(CommandResult::ok_msg("后台检测已停止"))
}

#[tauri::command]
pub fn trigger_background_check(_state: State<'_, AppState>, app_handle: AppHandle) -> Result<CommandResult, String> {
    let s = app_handle.state::<AppState>();
    if s.tasks.is_checking.is_active() {
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
    let running = state.tasks.background_running.is_active();
    let count = state.network.background_check_count.load(Ordering::Relaxed);
    let server_avail = state.network.server_available.load(Ordering::Acquire);

    let cached_adapter_statuses = {
        let cached_arc = state.network.cached_online_status.load();
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

    state.network.cached_online_status.store(Arc::new(Some(result.clone())));

    Ok(result)
}

pub fn run_startup_tasks(app_handle: &AppHandle) {
    let s = app_handle.state::<AppState>();
    let config = s.config.load_full();

    if config.enable_background_check {
        let app_h = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            let s = app_h.state::<AppState>();
            if let Err(e) = start_background_check_inner(&app_h, &s) {
                crate::log_warn!("background", "启动后台检测失败: {}", e);
            }
        });
    }

    if config.enable_network_quality && config.enable_latency_test {
        let app_h = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            let s = app_h.state::<AppState>();
            if !s.tasks.latency_running.swap_acquire() {
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
