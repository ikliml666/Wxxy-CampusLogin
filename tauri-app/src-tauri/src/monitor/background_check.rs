use tauri::{AppHandle, Manager};
use std::sync::atomic::Ordering;
use chrono::Timelike;
use crate::network::{get_adapters_cached, get_adapters_force};
use crate::infra::state::AppState;
use crate::infra::lifecycle::{start_campus_exit, cancel_campus_exit};
use crate::auth::failure_tracker::AdapterFailureCounter;
use super::auto_auth::{try_auto_login_on_preparation, try_disconnect_reconnect};
use super::campus_check::{CampusCheckResult, adapter_campus_status, adapter_campus_message, check_campus_network};
use super::portal_check::{PortalCheckResult, check_adapter_portal};
use super::portal_failure::handle_portal_request_failure;
use super::quality_scheduler::run_quality_check;
use super::background_emit::{handle_status_change, emit_background_check_result, update_network_state, BackgroundCheckResult};

pub(crate) fn run_background_check_blocking(app_handle: &AppHandle, state: &AppState, cancel_token: &tokio_util::sync::CancellationToken) -> Option<(String, String)> {
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
    // 始终更新 on_campus_network（静默期内 campus_result.on_campus=true，确保 emit 字段一致）
    state.network.update(|s| {
        s.current_ssid = campus_result.current_ssid.clone();
        s.on_campus_network = campus_result.on_campus;
    });

    if config.enable_network_name_check && !campus_result.on_campus {
        crate::log_debug!("background", "校园网检测未通过: {}", campus_result.message);
        state.network.update(|s| {
            s.any_adapter_online = false;
            s.last_a1_online = false;
        });
        let a1_campus = adapter_campus_message(&adapter1_name, &adapters, &campus_result);
        let a2_campus = if crate::network::is_secondary_adapter_enabled(&config, &adapter2_name) {
            adapter_campus_message(&adapter2_name, &adapters, &campus_result)
        } else { None };
        let a1_on_campus = adapter_campus_status(&adapter1_name, &adapters, &campus_result).map(|s| s.on_campus);
        let a2_on_campus = if crate::network::is_secondary_adapter_enabled(&config, &adapter2_name) {
            adapter_campus_status(&adapter2_name, &adapters, &campus_result).map(|s| s.on_campus)
        } else { None };
        emit_background_check_result(
            app_handle, state,
            &BackgroundCheckResult {
                online: false,
                reachable: false,
                login_available: false,
                message: a1_campus.as_deref().unwrap_or(&campus_result.message),
                adapter1_name: &adapter1_name,
                adapter2_name: &adapter2_name,
                secondary_online: None,
                secondary_message: a2_campus.as_deref().unwrap_or(""),
                dual_adapter: config.dual_adapter,
                config: &config,
                campus_result: &campus_result,
                a1_campus_msg: a1_campus.as_deref(),
                a2_campus_msg: a2_campus.as_deref(),
                a1_on_campus,
                a2_on_campus,
            },
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
            // 改用 tauri::async_runtime::spawn_blocking + tokio::join! 并行检测双适配器，
            // 替代 std::thread::scope 创建 OS 线程的方式（参考 auto_auth.rs 同场景实现）。
            // run_background_check_blocking 运行在 spawn_blocking 线程内，通过 Handle::current().block_on
            // 进入 async 上下文，使 spawn_blocking 提交的任务能被 await。
            let runtime_handle = tokio::runtime::Handle::current();
            let a1_owned = adapter1.clone();
            let a2_owned = adapter2.clone();
            let app_h1 = app_handle.clone();
            let app_h2 = app_handle.clone();
            runtime_handle.block_on(async {
                let h1 = tauri::async_runtime::spawn_blocking(move || check_adapter_portal(&a1_owned, &app_h1));
                let h2 = tauri::async_runtime::spawn_blocking(move || check_adapter_portal(&a2_owned, &app_h2));
                let (r1, r2) = tokio::join!(h1, h2);
                let r1 = r1.unwrap_or(PortalCheckResult::Error { is_request_failed: false });
                let r2 = r2.unwrap_or(PortalCheckResult::Error { is_request_failed: false });
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

    // Portal 请求失败容错：累加失败计数，连续5次 request_failed 时触发 MAC 重置（阈值见 portal_failure.rs::PORTAL_REQUEST_FAILURE_THRESHOLD）
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
            handle_portal_request_failure(
                state, app_handle, a1, a1_ip,
                campus_gw, AdapterFailureCounter::A1, "适配器1",
            );
        }

        // 适配器2 失败处理
        if secondary_is_request_failed {
            handle_portal_request_failure(
                state, app_handle, a2, a2_ip,
                campus_gw, AdapterFailureCounter::A2, "适配器2",
            );
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
        app_handle, state,
        &BackgroundCheckResult {
            online,
            reachable,
            login_available,
            message: &message,
            adapter1_name: &adapter1_name,
            adapter2_name: &adapter2_name,
            secondary_online,
            secondary_message: &secondary_message,
            dual_adapter: config.dual_adapter,
            config: &config,
            campus_result: &campus_result,
            a1_campus_msg: a1_campus.as_deref(),
            a2_campus_msg: a2_campus.as_deref(),
            a1_on_campus,
            a2_on_campus,
        },
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
