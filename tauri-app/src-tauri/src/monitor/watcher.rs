use tauri::{AppHandle, Emitter, Manager};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use crate::network::{
    Adapter, get_adapters_cached, get_adapters_force,
    check_network_quality_async,
};
use crate::auth::portal::check_portal_full;
use crate::infra::state::{AppState, CommandResult};
use crate::infra::lifecycle::start_auto_exit;
use super::auto_auth::{try_auto_login_on_preparation, try_disconnect_reconnect, run_auto_login_on_start};
use super::latency::{notify_network_quality_change, spawn_latency_test_loop};

enum PortalCheckResult {
    Success {
        online: bool,
        message: String,
        reachable: bool,
        login_available: bool,
    },
    Error,
    NotFound,
}

impl PortalCheckResult {
    fn online(&self) -> bool {
        match self {
            PortalCheckResult::Success { online, .. } => *online,
            _ => false,
        }
    }

    fn message(&self) -> &str {
        match self {
            PortalCheckResult::Success { message, .. } => message,
            PortalCheckResult::Error => "检测失败",
            PortalCheckResult::NotFound => "未找到主适配器",
        }
    }

    fn reachable(&self) -> bool {
        match self {
            PortalCheckResult::Success { reachable, .. } => *reachable,
            _ => false,
        }
    }

    fn login_available(&self) -> bool {
        match self {
            PortalCheckResult::Success { login_available, .. } => *login_available,
            _ => false,
        }
    }
}

fn check_adapter_portal(
    adapter: &Adapter,
    app_handle: &AppHandle,
) -> PortalCheckResult {
    match check_portal_full(&adapter.ip, Some(&adapter.name), None, None, None) {
        Ok(ps) => {
            if ps.error_kind.as_deref() == Some("request_failed") {
                crate::log_warn!("background", "{} Portal页面检测请求失败: {}", adapter.name, ps.message);
                let _ = app_handle.emit("login-log", serde_json::json!({
                    "message": format!("{} Portal页面检测请求失败: {}", adapter.name, ps.message),
                    "type": "error"
                }));
                PortalCheckResult::Error
            } else {
                PortalCheckResult::Success {
                    online: ps.online,
                    message: ps.message,
                    reachable: ps.reachable,
                    login_available: ps.login_available,
                }
            }
        }
        Err(e) => {
            crate::log_warn!("background", "{} Portal页面检测异常: {}", adapter.name, e);
            let _ = app_handle.emit("login-log", serde_json::json!({
                "message": format!("{} Portal页面检测异常: {}", adapter.name, e),
                "type": "error"
            }));
            PortalCheckResult::Error
        }
    }
}

pub fn adapter_status_entry(name: &str, ip: &str, wireless: bool, online: bool, message: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name, "ip": ip, "wireless": wireless,
        "online": online, "message": message
    })
}

pub fn adapter_disabled_entry(name: &str) -> serde_json::Value {
    adapter_status_entry(name, "", false, false, "适配器已禁用或未找到")
}

pub fn adapter_disconnected_entry(name: &str, wireless: bool) -> serde_json::Value {
    adapter_status_entry(name, "", wireless, false, "适配器未连接")
}

fn build_adapter_details(
    adapter1_name: &str,
    adapter1_message: &str,
    adapter2_name: &str,
    adapter2_message: Option<&str>,
    dual_adapter: bool,
) -> String {
    let mut details = vec![format!("{}: {}", adapter1_name, adapter1_message)];
    if let Some(msg) = adapter2_message {
        if dual_adapter && !adapter2_name.is_empty() {
            details.push(format!("{}: {}", adapter2_name, msg));
        }
    }
    details.join(", ")
}

fn handle_status_change(
    prev_online: bool,
    current_online: bool,
    reachable: bool,
    login_available: bool,
    adapter1_name: &str,
    adapter1_message: &str,
    adapter2_name: &str,
    adapter2_message: Option<&str>,
    config: &crate::config::model::Config,
    app_handle: &AppHandle,
) {
    let adapter_details = build_adapter_details(
        adapter1_name, adapter1_message,
        adapter2_name, adapter2_message,
        config.dual_adapter,
    );

    if current_online != prev_online {
        crate::log_info!("background", "状态变更: {} → {} [{}]",
            if prev_online { "在线" } else { "离线" },
            if current_online { "在线" } else { "离线" },
            adapter_details);

        if !current_online && config.enable_notification {
            crate::infra::notification::emit_notification(app_handle, "网络状态变更", &adapter_details);
        }
    } else {
        crate::log_debug!("background", "检测结果: online={}, reachable={}, loginAvailable={}, [{}]", current_online, reachable, login_available, adapter_details);
    }
}

fn emit_background_check_result(
    app_handle: &AppHandle,
    state: &AppState,
    online: bool,
    reachable: bool,
    login_available: bool,
    message: &str,
    adapter1_name: &str,
    adapter2_name: &str,
    secondary_online: Option<bool>,
    secondary_message: &str,
    dual_adapter: bool,
) {
    let check_count = state.network.background_check_count.fetch_add(1, Ordering::AcqRel) + 1;
    let is_running = state.tasks.background_running.is_active();
    let ssid_val = state.network.current_ssid.load();
    let on_campus_val = state.network.on_campus_network.load(Ordering::Acquire);
    if let Err(e) = app_handle.emit("background-check-result", serde_json::json!({
        "serverAvailable": reachable,
        "loginAvailable": login_available,
        "online": online,
        "message": message,
        "adapter1Name": adapter1_name,
        "adapter2Name": if dual_adapter { adapter2_name } else { "" },
        "secondaryOnline": secondary_online,
        "secondaryMessage": secondary_message,
        "timestamp": chrono::Utc::now().timestamp_millis(),
        "checkCount": check_count,
        "isRunning": is_running,
        "currentSsid": ssid_val.as_ref(),
        "onCampusNetwork": on_campus_val,
    })) {
        crate::log_warn!("background", "发送后台检测结果失败: {}", e);
    }
}

fn update_network_state(
    state: &AppState,
    online: bool,
    secondary_online: Option<bool>,
    reachable: bool,
    app_handle: &AppHandle,
) {
    state.network.server_available.store(reachable, Ordering::Release);

    let any_online = online || secondary_online == Some(true);

    let protected_until = state.network.logout_protected_until.load();
    let is_logout_protected = std::time::Instant::now() < **protected_until;

    if is_logout_protected {
        crate::log_debug!("background", "注销保护期内，跳过网络状态更新: any_online={}", any_online);
        return;
    }

    state.network.any_adapter_online.store(any_online, Ordering::Release);
    state.network.last_a1_online.store(online, Ordering::Release);
    if any_online {
        state.network.disconnect_reconnect_count.store(0, Ordering::Release);
    }

    if reachable && !state.network.has_logged_online.load(Ordering::Acquire) && online {
        state.network.has_logged_online.store(true, Ordering::Release);
        if state.config.load().auto_exit_on_online {
            start_auto_exit(app_handle, state);
        }
    }
}

fn check_campus_network(config: &crate::config::model::Config, adapters: &[crate::network::Adapter]) -> (bool, Option<String>, String) {
    crate::log_info!("campus", "[校园网检测] enable_network_name_check={}, required_network_name='{}', campus_gateway='{}'",
        config.enable_network_name_check, config.required_network_name, config.campus_gateway);

    if !config.enable_network_name_check {
        crate::log_info!("campus", "[校园网检测] 名称检查已禁用，直接返回 true");
        return (true, None, String::new());
    }

    let required_name = &config.required_network_name;
    let campus_gw = &config.campus_gateway;

    let network_names = crate::network::get_connected_network_names();
    crate::log_info!("campus", "[校园网检测] get_connected_network_names() = {:?}", network_names);

    if !network_names.is_empty() {
        let matched = network_names.iter().find(|n| n.eq_ignore_ascii_case(required_name));
        if let Some(matched_name) = matched {
            crate::log_info!("campus", "[校园网检测] ✅ 名称匹配成功: '{}'", matched_name);
            return (true, Some(matched_name.clone()), format!("已连接到校园网络({})", matched_name));
        }
        crate::log_info!("campus", "[校园网检测] 名称匹配失败: network_names 中无 '{}'", required_name);
    }

    for a in adapters {
        if !a.ip.is_empty() {
            let same_subnet = crate::network::is_same_subnet_18(&a.ip, campus_gw);
            crate::log_info!("campus", "[校园网检测] 子网检查: adapter={}, ip={}, gw={}, /18匹配={}",
                a.name, a.ip, campus_gw, same_subnet);
            if same_subnet {
                crate::log_info!("campus", "[校园网检测] ✅ /18 子网匹配成功: {}", a.ip);
                return (true, None, format!("已连接校园网({}与网关在同一/18网段)", a.ip));
            }
        }
    }

    let gateway_ok = crate::network::check_gateway_reachable(campus_gw);
    crate::log_info!("campus", "[校园网检测] 网关可达性检查: gw={}, reachable={}", campus_gw, gateway_ok);

    if gateway_ok {
        crate::log_info!("campus", "[校园网检测] ✅ 网关可达");
        return (true, None, format!("通过路由器连接校园网(网关{}可达)", campus_gw));
    }

    let reason = if let Some(ref name) = network_names.first() {
        format!("当前网络\"{}\"非校园网络", name)
    } else {
        "未连接到校园网络".to_string()
    };

    crate::log_warn!("campus", "[校园网检测] ❌ 所有检测均未通过: {}", reason);
    (false, None, reason)
}

fn run_background_check_blocking(app_handle: &AppHandle, state: &AppState, cancel_token: &tokio_util::sync::CancellationToken) -> Option<(String, String)> {
    if state.exit.is_quitting.load(Ordering::Acquire) || cancel_token.is_cancelled() {
        return None;
    }
    let _check_guard = match state.tasks.is_checking.try_acquire() {
        Some(guard) => guard,
        None => return None,
    };
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

    let a1 = adapters.iter().find(|a| a.name == adapter1_name && !a.ip.is_empty());
    let a2 = if config.dual_adapter && !adapter2_name.is_empty() {
        adapters.iter().find(|a| a.name == adapter2_name && !a.ip.is_empty())
    } else {
        None
    };

    let (on_campus, current_ssid, campus_message) = check_campus_network(&config, &adapters);
    state.network.current_ssid.store(std::sync::Arc::new(current_ssid));
    state.network.on_campus_network.store(on_campus, Ordering::Release);

    if config.enable_network_name_check && !on_campus {
        crate::log_debug!("background", "校园网检测未通过: {}", campus_message);
        state.network.any_adapter_online.store(false, Ordering::Release);
        emit_background_check_result(
            app_handle, state, false, false, false, &campus_message,
            &adapter1_name, &adapter2_name,
            None, "", config.dual_adapter,
        );
        crate::log_debug!("background", "后台检测周期完成(校园网检测未通过), 总耗时{}ms", t_total.elapsed().as_millis());
        return None;
    }

    if cancel_token.is_cancelled() {
        return None;
    }

    let t_portal = std::time::Instant::now();
    let (primary_result, secondary_result) = if config.dual_adapter {
        if let (Some(adapter1), Some(adapter2)) = (a1, a2) {
            std::thread::scope(|s| {
                let h1 = s.spawn(|| check_adapter_portal(adapter1, app_handle));
                let h2 = s.spawn(|| check_adapter_portal(adapter2, app_handle));
                let r1 = h1.join().unwrap_or(PortalCheckResult::Error);
                let r2 = h2.join().unwrap_or(PortalCheckResult::Error);
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

    let online = primary_result.online();
    let reachable = primary_result.reachable();
    let login_available = primary_result.login_available();
    let message = primary_result.message();
    let prev_online = state.network.any_adapter_online.load(Ordering::Acquire);

    crate::log_debug!("background", "Portal检测完成({}ms): 主[{}]={}/{} | 副={:?}",
        portal_elapsed.as_millis(),
        adapter1_name,
        if online { "online" } else { if reachable { "offline" } else { "unreachable" } },
        message,
        secondary_result.as_ref().map(|r| format!("{}/{}", if r.online() {"online"} else {r.message()}, r.reachable())));

    let (secondary_online, secondary_message) = match &secondary_result {
        Some(PortalCheckResult::Success { online, message, .. }) => (Some(*online), message.clone()),
        _ => (None, String::new()),
    };

    state.network.last_a2_online.store(secondary_online == Some(true), Ordering::Release);

    handle_status_change(
        prev_online, online, reachable, login_available,
        &adapter1_name, message,
        &adapter2_name, secondary_result.as_ref().and_then(|r| {
            if let PortalCheckResult::Success { message, .. } = r { Some(message.as_str()) } else { None }
        }),
        &config, app_handle,
    );

    emit_background_check_result(
        app_handle, state, online, reachable, login_available, message,
        &adapter1_name, &adapter2_name,
        secondary_online, &secondary_message, config.dual_adapter,
    );

    try_auto_login_on_preparation(app_handle, state, login_available, online, &config);

    try_disconnect_reconnect(
        app_handle, state, online, secondary_online,
        a1, &adapter1_name, &adapter2_name,
        reachable, login_available, &config,
    );

    update_network_state(state, online, secondary_online, reachable, app_handle);

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
        let s = app_handle.state::<AppState>();
        let (skip_ttfb, skip_content, fixed_gateway) = {
            let cfg = s.config.load();
            (cfg.skip_ttfb_in_latency, cfg.skip_content_in_latency, cfg.fixed_gateway.clone())
        };
        let _quality_guard = match s.tasks.is_quality_checking.try_acquire() {
            Some(g) => g,
            None => return,
        };
        let quality = check_network_quality_async(&adapter_name, &adapter_ip, skip_ttfb, skip_content, &fixed_gateway, s.exit.is_quitting.clone()).await;
        drop(_quality_guard);
        let enable_notification = s.config.load().enable_notification;
        let quality_val = match serde_json::to_value(&quality) {
            Ok(v) => v,
            Err(e) => {
                crate::log_warn!("background", "序列化网络质量结果失败: {}", e);
                return;
            }
        };
        if let Err(e) = app_handle.emit("network-quality-result", &quality_val) {
            crate::log_warn!("background", "发送网络质量结果失败: {}", e);
        }
        notify_network_quality_change(app_handle, &s, &quality_val, enable_notification);
    }
}

pub fn start_background_check_inner(app_handle: &AppHandle, state: &AppState) -> Result<CommandResult, String> {
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

    if let Err(e) = crate::commands::config_cmd::save_config_to_disk(app_handle, &cfg) {
        crate::log_warn!("background", "保存后台检测配置失败: {}", e);
    }

    let app_h = app_handle.clone();
    let bg_cancel = state.tasks.bg_check_cancel.load().clone();
    tauri::async_runtime::spawn(async move {
        {
            let mut waited = 0u64;
            while waited < 5000 {
                let s = app_h.state::<AppState>();
                if !s.tasks.is_checking.is_active() {
                    break;
                }
                drop(s);
                tokio::time::sleep(Duration::from_millis(50)).await;
                waited += 50;
            }
        }

        run_background_check(&app_h, bg_cancel.clone()).await;

        let mut interval_timer = tokio::time::interval(Duration::from_millis(interval));
        loop {
            tokio::select! {
                _ = interval_timer.tick() => {}
                _ = bg_cancel.cancelled() => {
                    crate::log_debug!("background", "后台检测收到取消信号，退出循环");
                    break;
                }
            }
            let s = app_h.state::<AppState>();
            if !s.tasks.background_running.is_active() || s.exit.is_quitting.load(Ordering::Acquire) {
                break;
            }
            run_background_check(&app_h, bg_cancel.clone()).await;
        }
    });

    Ok(CommandResult::ok_msg("后台检测已启动"))
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
