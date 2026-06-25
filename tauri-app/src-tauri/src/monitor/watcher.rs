use tauri::{AppHandle, Manager};
use std::sync::atomic::Ordering;
use std::time::Duration;
use chrono::Timelike;
use crate::network::{get_adapters_cached, get_adapters_force};
use crate::infra::command_context::CommandContext;
use crate::infra::events::EventBus;
use crate::infra::state::{AppState, CommandResult};
use crate::infra::lifecycle::{start_campus_exit, cancel_campus_exit};
use super::auto_auth::{try_auto_login_on_preparation, try_disconnect_reconnect, run_auto_login_on_start};
use super::latency::spawn_latency_test_loop;
use super::campus_check::{CampusCheckResult, adapter_campus_status, adapter_campus_message};
pub use super::campus_check::check_campus_network;
use super::portal_check::{PortalCheckResult, check_adapter_portal};
use super::quality_scheduler::run_quality_check;
use super::background_emit::{handle_status_change, emit_background_check_result, update_network_state};
pub use super::background_emit::{adapter_status_entry, adapter_disabled_entry, adapter_disconnected_entry};

fn run_background_check_blocking(app_handle: &AppHandle, state: &AppState, cancel_token: &tokio_util::sync::CancellationToken) -> Option<(String, String)> {
    if state.exit.is_quitting.load(Ordering::Acquire) || cancel_token.is_cancelled() {
        return None;
    }
    let _check_guard = state.tasks.is_checking.try_acquire()?;
    let t_total = std::time::Instant::now();

    let config = state.config.load_full();
    crate::log_debug!("background", "开始后台检测 (dualAdapter={}, interval={}ms)",
        config.dual_adapter, config.background_check_interval);

    let t_adapters = std::time::Instant::now();
    let adapters = match get_adapters_cached() {
        Ok(a) if !a.is_empty() => a,
        _ => match get_adapters_force() {
            Ok(a) => a,
            Err(e) => {
                crate::log_error!("background", "获取适配器列表失败: {}", e);
                return None;
            }
        }
    };

    crate::log_debug!("background", "适配器列表: {}个 (耗时{}ms)", adapters.len(), t_adapters.elapsed().as_millis());

    let (adapter1_name, adapter2_name) = crate::network::resolve_adapter_names(&adapters, &config);

    let (a1, a2) = crate::network::find_dual_adapters(&adapters, &config, &adapter1_name, &adapter2_name);

    let campus_result = if config.campus_check_start_minutes > 0 && (chrono::Local::now().hour() as u16 * 60 + chrono::Local::now().minute() as u16) < config.campus_check_start_minutes {
        let hour = config.campus_check_start_minutes / 60;
        let minute = config.campus_check_start_minutes % 60;
        crate::log_info!("background", "校园网检测静默期（当前时间早于{}:{:02}），跳过校园网环境验证", hour, minute);
        cancel_campus_exit(app_handle, state);
        CampusCheckResult {
            wifi: None,
            wired: None,
            on_campus: true,
            current_ssid: None,
            message: format!("校园网检测静默期（早于{hour}:{minute:02}），跳过验证"),
        }
    } else {
        check_campus_network(&config, &adapters)
    };
    state.network.update(|s| s.current_ssid = campus_result.current_ssid.clone());
    // 始终更新 on_campus_network（静默期内 campus_result.on_campus=true，确保 emit 字段一致）
    state.network.update(|s| s.on_campus_network = campus_result.on_campus);

    if config.enable_network_name_check && !campus_result.on_campus {
        crate::log_debug!("background", "校园网检测未通过: {}", campus_result.message);
        state.network.update(|s| s.any_adapter_online = false);
        state.network.update(|s| s.last_a1_online = false);
        let a1_campus = adapter_campus_message(&adapter1_name, &adapters, &campus_result);
        let a2_campus = if crate::network::is_secondary_adapter_enabled(&config, &adapter2_name) {
            adapter_campus_message(&adapter2_name, &adapters, &campus_result)
        } else { None };
        let a1_on_campus = adapter_campus_status(&adapter1_name, &adapters, &campus_result).map(|s| s.on_campus);
        let a2_on_campus = if crate::network::is_secondary_adapter_enabled(&config, &adapter2_name) {
            adapter_campus_status(&adapter2_name, &adapters, &campus_result).map(|s| s.on_campus)
        } else { None };
        emit_background_check_result(
            app_handle, state, false, false, false, a1_campus.as_deref().unwrap_or(&campus_result.message),
            &adapter1_name, &adapter2_name,
            None, a2_campus.as_deref().unwrap_or(""), config.dual_adapter, &config, &campus_result,
            a1_campus.as_deref(), a2_campus.as_deref(),
            a1_on_campus, a2_on_campus,
        );
        // 如果配置的适配器均无IP（完全无网络），跳过退出，等待网络恢复
        let no_configured_ip = a1.is_none() && a2.is_none();
        if no_configured_ip {
            crate::log_info!("background", "配置的适配器均无IP地址，跳过校园网退出，等待网络恢复");
        } else {
            // 校园网验证不通过：触发最小化+退出流程
            start_campus_exit(app_handle, state);
        }
        crate::log_debug!("background", "后台检测周期完成(校园网检测未通过), 总耗时{}ms", t_total.elapsed().as_millis());
        return None;
    }

    // 校园网验证通过：取消之前的退出流程（如果有的话）
    cancel_campus_exit(app_handle, state);

    if cancel_token.is_cancelled() {
        return None;
    }

    let t_portal = std::time::Instant::now();
    let (primary_result, secondary_result) = if config.dual_adapter {
        if let (Some(adapter1), Some(adapter2)) = (a1, a2) {
            // 获取 Tokio runtime handle 传入 scope 子线程：
            // spawn_blocking 线程中 Handle::current() 可用，但 std::thread::scope 子线程
            // 无 Tokio 上下文，check_portal_full 中的 block_on 会因找不到 reactor 而 panic。
            // 在子线程中 enter() 设置上下文，使 Handle::current().block_on() 能正确工作。
            let runtime_handle = tokio::runtime::Handle::current();
            std::thread::scope(|s| {
                let h1 = runtime_handle.clone();
                let h2 = runtime_handle.clone();
                let t1 = s.spawn(move || {
                    let _guard = h1.enter();
                    check_adapter_portal(adapter1, app_handle)
                });
                let t2 = s.spawn(move || {
                    let _guard = h2.enter();
                    check_adapter_portal(adapter2, app_handle)
                });
                let r1 = t1.join().unwrap_or(PortalCheckResult::Error { is_request_failed: false });
                let r2 = t2.join().unwrap_or(PortalCheckResult::Error { is_request_failed: false });
                (r1, Some(r2))
            })
        } else {
            let primary = match a1 {
                Some(adapter) => check_adapter_portal(adapter, app_handle),
                None => PortalCheckResult::NotFound,
            };
            let secondary = a2.map(|a| check_adapter_portal(a, app_handle));
            (primary, secondary)
        }
    } else {
        let primary = match a1 {
            Some(adapter) => check_adapter_portal(adapter, app_handle),
            None => PortalCheckResult::NotFound,
        };
        (primary, None)
    };

    let portal_elapsed = t_portal.elapsed();

    // Portal 请求失败容错：累加失败计数，连续3次 request_failed 时触发 MAC 重置
    let primary_is_request_failed = matches!(&primary_result, PortalCheckResult::Error { is_request_failed: true });
    let secondary_is_request_failed = secondary_result.as_ref().map(|r| matches!(r, PortalCheckResult::Error { is_request_failed: true })).unwrap_or(false);
    let any_request_failed = primary_is_request_failed || secondary_is_request_failed;

    if any_request_failed {
        // 按适配器分别检查网关可达性：每个适配器从自己的 IP 绑定 ping 网关
        let campus_gw = &config.campus_gateway;
        let a1_ip = a1.map(|a| a.ip.as_str());
        let a2_ip = a2.map(|a| a.ip.as_str());

        // 适配器1 失败处理
        if primary_is_request_failed {
            let gw_reachable = crate::network::check_gateway_reachable_from(campus_gw, a1_ip);
            if !gw_reachable {
                crate::log_info!("background", "适配器1 Portal失败但网关[{}]从[{}]不可达，跳过计数（校园网断网/维护）", campus_gw, a1_ip.unwrap_or(""));
                let prev = state.network.load().a1_auth_failure_count;
                state.network.update(|s| s.a1_auth_failure_count = 0);
                if prev > 0 {
                    crate::log_debug!("background", "适配器1 网关不可达，重置失败计数(原值={})", prev);
                }
            } else {
                let prev_count = state.network.load().a1_auth_failure_count;
                state.network.increment_a1_auth_failure_count();
                let new_count = prev_count + 1;
                crate::log_info!("background", "适配器1 Portal失败计数: {}/5 (网关可达)", new_count);
                if new_count >= 5 {
                    crate::log_warn!("background", "适配器1 连续{}次Portal失败(网关可达)，触发该适配器MAC重置", new_count);
                    let event_bus = EventBus::new(app_handle);
                    let _ = event_bus.emit_login_log(
                        "适配器1 连续5次 Portal 请求失败，正在重置该适配器MAC...",
                        "warning",
                    );
                    if let Some(a1_ref) = a1 {
                        match crate::network::dhcp_release_renew_single(&a1_ref.name, campus_gw) {
                            Ok(r) => {
                                let skipped = r.get("skipped").and_then(|v| v.as_bool()).unwrap_or(false);
                                let success = r.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
                                if skipped {
                                    crate::log_debug!("background", "适配器1 MAC重置跳过(非校园网子网)");
                                } else if success {
                                    crate::log_info!("background", "适配器1 MAC重置成功");
                                } else {
                                    crate::log_warn!("background", "适配器1 MAC重置失败");
                                }
                            }
                            Err(e) => {
                                crate::log_error!("background", "适配器1 MAC重置失败: {}", e);
                            }
                        }
                    }
                    state.network.update(|s| s.a1_auth_failure_count = 0);
                }
            }
        }

        // 适配器2 失败处理
        if secondary_is_request_failed {
            let gw_reachable = crate::network::check_gateway_reachable_from(campus_gw, a2_ip);
            if !gw_reachable {
                crate::log_info!("background", "适配器2 Portal失败但网关[{}]从[{}]不可达，跳过计数（校园网断网/维护）", campus_gw, a2_ip.unwrap_or(""));
                let prev = state.network.load().a2_auth_failure_count;
                state.network.update(|s| s.a2_auth_failure_count = 0);
                if prev > 0 {
                    crate::log_debug!("background", "适配器2 网关不可达，重置失败计数(原值={})", prev);
                }
            } else {
                let prev_count = state.network.load().a2_auth_failure_count;
                state.network.increment_a2_auth_failure_count();
                let new_count = prev_count + 1;
                crate::log_info!("background", "适配器2 Portal失败计数: {}/5 (网关可达)", new_count);
                if new_count >= 5 {
                    crate::log_warn!("background", "适配器2 连续{}次Portal失败(网关可达)，触发该适配器MAC重置", new_count);
                    let event_bus = EventBus::new(app_handle);
                    let _ = event_bus.emit_login_log(
                        "适配器2 连续5次 Portal 请求失败，正在重置该适配器MAC...",
                        "warning",
                    );
                    if let Some(a2_ref) = a2 {
                        match crate::network::dhcp_release_renew_single(&a2_ref.name, campus_gw) {
                            Ok(r) => {
                                let skipped = r.get("skipped").and_then(|v| v.as_bool()).unwrap_or(false);
                                let success = r.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
                                if skipped {
                                    crate::log_debug!("background", "适配器2 MAC重置跳过(非校园网子网)");
                                } else if success {
                                    crate::log_info!("background", "适配器2 MAC重置成功");
                                } else {
                                    crate::log_warn!("background", "适配器2 MAC重置失败");
                                }
                            }
                            Err(e) => {
                                crate::log_error!("background", "适配器2 MAC重置失败: {}", e);
                            }
                        }
                    }
                    state.network.update(|s| s.a2_auth_failure_count = 0);
                }
            }
        }
    } else {
        // 任一适配器 Success 即重置对应计数器
        let primary_success = matches!(&primary_result, PortalCheckResult::Success { .. });
        let secondary_success = secondary_result.as_ref().map(|r| matches!(r, PortalCheckResult::Success { .. })).unwrap_or(false);

        if primary_success {
            let prev = state.network.load().a1_auth_failure_count;
            state.network.update(|s| s.a1_auth_failure_count = 0);
            if prev > 0 {
                crate::log_debug!("background", "适配器1 Portal检测恢复正常，重置失败计数(原值={})", prev);
            }
        }
        if secondary_success {
            let prev = state.network.load().a2_auth_failure_count;
            state.network.update(|s| s.a2_auth_failure_count = 0);
            if prev > 0 {
                crate::log_debug!("background", "适配器2 Portal检测恢复正常，重置失败计数(原值={})", prev);
            }
        }
    }

    let primary_online = primary_result.online();
    let reachable = primary_result.reachable();
    let login_available = primary_result.login_available();
    let a1_has_ip = a1.is_some();

    let message: String = if a1_has_ip {
        primary_result.message().to_string()
    } else {
        adapter_campus_message(&adapter1_name, &adapters, &campus_result)
            .unwrap_or_else(|| primary_result.message().to_string())
    };
    let online = if a1_has_ip { primary_online } else { false };

    let prev_online = state.network.load().any_adapter_online;

    crate::log_debug!("background", "Portal检测完成({}ms): 主[{}]={}/{} |副={:?}",
        portal_elapsed.as_millis(),
        adapter1_name,
        if online { "online" } else if reachable { "offline" } else { "unreachable" },
        message,
        secondary_result.as_ref().map(|r| format!("{}/{}", if r.online() {"online"} else {r.message()}, r.reachable())));

    let a2_has_ip = a2.is_some();
    let (secondary_online, secondary_message) = match &secondary_result {
        Some(PortalCheckResult::Success { online, message: msg, .. }) => (Some(*online), msg.clone()),
        _ => {
            if crate::network::is_secondary_adapter_enabled(&config, &adapter2_name) && !a2_has_ip {
                let msg = adapter_campus_message(&adapter2_name, &adapters, &campus_result);
                match msg {
                    Some(ref m) => (Some(false), m.clone()),
                    None => (None, String::new()),
                }
            } else if crate::network::is_secondary_adapter_enabled(&config, &adapter2_name) && a2_has_ip {
                (None, secondary_result.as_ref().map(|r| r.message().to_string()).unwrap_or_default())
            } else {
                (None, String::new())
            }
        }
    };

    state.network.update(|s| s.last_a2_online = secondary_online == Some(true));

    handle_status_change(
        prev_online, online, reachable, login_available,
        &adapter1_name, &message,
        &adapter2_name, if secondary_message.is_empty() { None } else { Some(secondary_message.as_str()) },
        &config, app_handle,
    );

    let a1_campus = adapter_campus_message(&adapter1_name, &adapters, &campus_result);
    let a2_campus = if crate::network::is_secondary_adapter_enabled(&config, &adapter2_name) {
        adapter_campus_message(&adapter2_name, &adapters, &campus_result)
    } else { None };
    let a1_on_campus = adapter_campus_status(&adapter1_name, &adapters, &campus_result).map(|s| s.on_campus);
    let a2_on_campus = if crate::network::is_secondary_adapter_enabled(&config, &adapter2_name) {
        adapter_campus_status(&adapter2_name, &adapters, &campus_result).map(|s| s.on_campus)
    } else { None };

    emit_background_check_result(
        app_handle, state, online, reachable, login_available, &message,
        &adapter1_name, &adapter2_name,
        secondary_online, &secondary_message, config.dual_adapter, &config, &campus_result,
        a1_campus.as_deref(), a2_campus.as_deref(),
        a1_on_campus, a2_on_campus,
    );

    try_auto_login_on_preparation(app_handle, state, login_available, online, &config);

    let reconnected = try_disconnect_reconnect(
        app_handle, state, online, secondary_online,
        a1, &adapter1_name, &adapter2_name,
        reachable, login_available, &config,
    );

    // 重连成功时状态已在 try_disconnect_reconnect 内设置，
    // 跳过 update_network_state 避免用重连前旧 Portal 结果覆盖 any_adapter_online
    if !reconnected {
        update_network_state(state, online, secondary_online, reachable, app_handle);
    }

    crate::log_debug!("background", "后台检测周期完成, 总耗时{}ms", t_total.elapsed().as_millis());

    if online && a1.is_some() && config.enable_network_quality {
        if let Some(a1_ref) = a1 {
            return Some((a1_ref.name.clone(), a1_ref.ip.clone()));
        }
    }

    None
}

pub async fn run_background_check(app_handle: &AppHandle, cancel_token: std::sync::Arc<tokio_util::sync::CancellationToken>) {
    let app_h = app_handle.clone();
    let quality_info = tauri::async_runtime::spawn_blocking(move || {
        let s = app_h.state::<AppState>();
        run_background_check_blocking(&app_h, &s, &cancel_token)
    }).await.unwrap_or_else(|e| {
        crate::log_error!("background", "后台检测异常: {}", e);
        None
    });

    if let Some((adapter_name, adapter_ip)) = quality_info {
        run_quality_check(app_handle, &adapter_name, &adapter_ip).await;
    }
}

pub fn start_background_check_inner(app_handle: &AppHandle, state: &AppState) -> Result<CommandResult, String> {
    crate::log_info!("background", "start_background_check_inner 入口");
    let (interval, cfg) = {
        let cfg = state.config.update(|cfg| {
            cfg.enable_background_check = true;
            if cfg.background_check_interval < 10000 {
                cfg.background_check_interval = 15000;
            }
        });
        let interval = cfg.background_check_interval;
        (interval, cfg)
    };
    crate::log_info!("background", "start_background_check_inner: interval={}ms, 即将 spawn background_check 任务", interval);

    let app_h = app_handle.clone();
    state.task_manager.spawn("background_check", move |cancel_token| {
        async move {
            crate::log_info!("background", "background_check 任务实际启动");
            {
                let mut waited = 0u64;
                while waited < 5000 {
                    let s = app_h.state::<AppState>();
                    if !s.tasks.is_checking.is_active() {
                        break;
                    }
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_millis(50)) => { waited += 50; }
                        _ = cancel_token.cancelled() => { break; }
                    }
                }
                if waited >= 5000 {
                    crate::log_warn!("background", "background_check: 等待 is_checking 释放超时(5000ms)，继续执行");
                }
            }

            crate::log_info!("background", "background_check: 等待完成，开始首次 run_background_check");
            run_background_check(&app_h, cancel_token.clone()).await;

            let mut interval_timer = tokio::time::interval(Duration::from_millis(interval));
            interval_timer.tick().await;
            loop {
                tokio::select! {
                    _ = interval_timer.tick() => {}
                    _ = cancel_token.cancelled() => {
                        crate::log_debug!("background", "后台检测收到取消信号，退出循环");
                        break;
                    }
                }
                let s = app_h.state::<AppState>();
                if !s.task_manager.is_running("background_check") || s.exit.is_quitting.load(Ordering::Acquire) {
                    break;
                }
                run_background_check(&app_h, cancel_token.clone()).await;
            }
        }
    })?;

    if let Err(e) = crate::commands::config_cmd::save_config_to_disk_encrypted(app_handle, &cfg) {
        crate::log_warn!("background", "保存后台检测配置失败: {}", e);
    }

    Ok(CommandResult::ok_msg("后台检测已启动"))
}

pub fn run_startup_tasks(app_handle: &AppHandle) {
    let s = CommandContext::from_app(app_handle);
    let config = s.config.load_full();

    crate::log_info!("startup", "run_startup_tasks 入口: enableBackgroundCheck={}, enableNetworkQuality={}, enableLatencyTest={}, autoLoginOnStart={}",
        config.enable_background_check, config.enable_network_quality, config.enable_latency_test, config.auto_login_on_start);

    if config.enable_background_check {
        let app_h = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            crate::log_info!("startup", "[task] background_check spawn 开始");
            let s = app_h.state::<AppState>();
            if let Err(e) = start_background_check_inner(&app_h, &s) {
                crate::log_warn!("background", "启动后台检测失败: {}", e);
            }
        });
    }

    if config.enable_network_quality && config.enable_latency_test {
        let app_h = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            crate::log_info!("startup", "[task] latency_test spawn 开始");
            let s = app_h.state::<AppState>();
            let interval = {
                let c = s.config.load();
                if c.latency_test_interval < 10000 { 30000 } else { c.latency_test_interval }
            };
            let _ = spawn_latency_test_loop(&app_h, interval);
        });
    }

    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        crate::log_info!("startup", "[task] auto_login_on_start spawn 开始");
        run_auto_login_on_start(&app_h);
    });
}
