use tauri::{AppHandle, Emitter, Manager};
use std::sync::atomic::Ordering;
use std::time::Duration;
use serde::Serialize;
use chrono::Timelike;
use crate::network::{
    Adapter, get_adapters_cached, get_adapters_force,
    check_network_quality_async,
};
use crate::auth::portal::check_portal_full;
use crate::infra::state::{AppState, CommandResult};
use crate::infra::lifecycle::{start_campus_exit, cancel_campus_exit};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionCampusStatus {
    pub on_campus: bool,
    pub name: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CampusCheckResult {
    pub wifi: Option<ConnectionCampusStatus>,
    pub wired: Option<ConnectionCampusStatus>,
    pub on_campus: bool,
    pub current_ssid: Option<String>,
    pub message: String,
}
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
    Error {
        is_request_failed: bool,
    },
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
            PortalCheckResult::Error { .. } => "检测失败",
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
                crate::log_warn!("network", "{} Portal页面检测请求失败: {}", adapter.name, ps.message);
                let _ = app_handle.emit("login-log", serde_json::json!({
                    "message": format!("{} Portal页面检测请求失败: {}", adapter.name, ps.message),
                    "type": "error"
                }));
                PortalCheckResult::Error { is_request_failed: true }
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
            PortalCheckResult::Error { is_request_failed: false }
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
    config: &crate::config::model::Config,
    campus_result: &CampusCheckResult,
    a1_campus_msg: Option<&str>,
    a2_campus_msg: Option<&str>,
    a1_on_campus: Option<bool>,
    a2_on_campus: Option<bool>,
) {
    let check_count = state.network.background_check_count.fetch_add(1, Ordering::AcqRel) + 1;
    let is_running = state.tasks.background_running.is_active();
    let ssid_val = state.network.current_ssid.load();
    let on_campus_val = state.network.on_campus_network.load(Ordering::Acquire);

    // 注销保护期内，强制 online=false，避免 Portal 延迟导致前端误判为在线
    let protected_until = state.network.logout_protected_until.load();
    let is_logout_protected = std::time::Instant::now() < **protected_until;
    let (effective_online, effective_secondary_online) = if is_logout_protected {
        crate::log_debug!("background", "注销保护期内，emit 事件强制 online=false");
        (false, Some(false))
    } else {
        (online, secondary_online)
    };

    if let Err(e) = app_handle.emit("background-check-result", serde_json::json!({
        "serverAvailable": reachable,
        "loginAvailable": login_available,
        "online": effective_online,
        "message": message,
        "adapter1Name": adapter1_name,
        "adapter2Name": if dual_adapter { adapter2_name } else { "" },
        "secondaryOnline": effective_secondary_online,
        "secondaryMessage": secondary_message,
        "timestamp": chrono::Utc::now().timestamp_millis(),
        "checkCount": check_count,
        "isRunning": is_running,
        "currentSsid": ssid_val.as_ref(),
        "onCampusNetwork": on_campus_val,
        "enableNetworkNameCheck": config.enable_network_name_check,
        "requiredNetworkName": config.required_network_name,
        "campusWifi": campus_result.wifi,
        "campusWired": campus_result.wired,
        "a1CampusMessage": a1_campus_msg,
        "a2CampusMessage": a2_campus_msg,
        "a1OnCampus": a1_on_campus,
        "a2OnCampus": a2_on_campus,
    })) {
        crate::log_warn!("background", "发送后台检测结果失败: {}", e);
    }
}

fn adapter_campus_status<'a>(adapter_name: &str, adapters: &'a [Adapter], campus_result: &'a CampusCheckResult) -> Option<&'a ConnectionCampusStatus> {
    let info = adapters.iter().find(|a| a.name == adapter_name)?;
    let is_wireless = info.wireless;
    let status = if is_wireless { &campus_result.wifi } else { &campus_result.wired };
    status.as_ref()
}

fn adapter_campus_message(adapter_name: &str, adapters: &[Adapter], campus_result: &CampusCheckResult) -> Option<String> {
    adapter_campus_status(adapter_name, adapters, campus_result).map(|s| s.message.clone())
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

pub fn check_campus_network(config: &crate::config::model::Config, adapters: &[crate::network::Adapter]) -> CampusCheckResult {
    crate::log_info!("campus", "[校园网检测] enable_network_name_check={}, required_network_name='{}', campus_gateway='{}'",
        config.enable_network_name_check, config.required_network_name, config.campus_gateway);

    if !config.enable_network_name_check {
        let gateway_ok = crate::network::check_gateway_reachable(&config.campus_gateway);
        crate::log_info!("campus", "[校园网检测] 名称检查已禁用，网关可达性: {}", gateway_ok);
        let msg = if gateway_ok {
            format!("网关{}可达", config.campus_gateway)
        } else {
            "未连接到校园网络(网关不可达)".to_string()
        };
        return CampusCheckResult {
            wifi: None,
            wired: None,
            on_campus: gateway_ok,
            current_ssid: None,
            message: msg,
        };
    }

    let required_name = &config.required_network_name;
    let campus_gw = &config.campus_gateway;

    let wifi_ssid = crate::network::get_wireless_ssid().ok().flatten();
    let wired_profile = crate::network::get_wired_network_profile().ok().flatten();

    crate::log_info!("campus", "[校园网检测] wifi_ssid={:?}, wired_profile={:?}", wifi_ssid, wired_profile);

    let mut gateway_checked: Option<bool> = None;
    let check_gateway = |gw: &str, cache: &mut Option<bool>| -> bool {
        if let Some(cached) = cache {
            crate::log_info!("campus", "[校园网检测] 使用缓存的网关可达性: {}", cached);
            *cached
        } else {
            let ok = crate::network::check_gateway_reachable(gw);
            *cache = Some(ok);
            crate::log_info!("campus", "[校园网检测] 网关可达性检查: gw={}, reachable={}", gw, ok);
            ok
        }
    };

    let wifi_status = {
        let wifi_adapters: Vec<&crate::network::Adapter> = adapters.iter().filter(|a| a.wireless).collect();
        match &wifi_ssid {
            Some(ssid) if ssid.eq_ignore_ascii_case(required_name) => {
                crate::log_info!("campus", "[校园网检测] ✅ WiFi名称匹配: '{}'", ssid);
                Some(ConnectionCampusStatus {
                    on_campus: true,
                    name: Some(ssid.clone()),
                    message: format!("已连接到校园WiFi({})", ssid),
                })
            }
            Some(ssid) => {
                crate::log_info!("campus", "[校园网检测] WiFi SSID '{}' 不匹配校园网名称'{}'", ssid, required_name);
                let mut found = false;
                let mut msg = String::new();
                for a in &wifi_adapters {
                    if !a.ip.is_empty() {
                        let same_subnet = crate::network::is_same_subnet_18(&a.ip, campus_gw);
                        crate::log_info!("campus", "[校园网检测] WiFi SSID不匹配，尝试子网检查: adapter={}, ip={}, /18匹配={}", a.name, a.ip, same_subnet);
                        if same_subnet {
                            found = true;
                            msg = format!("WiFi\"{}\"名称不匹配但与网关在同一/18网段", ssid);
                            break;
                        }
                    }
                }
                if !found {
                    let gateway_ok = check_gateway(campus_gw, &mut gateway_checked);
                    if gateway_ok {
                        found = true;
                        msg = format!("WiFi\"{}\"名称不匹配但网关{}可达", ssid, campus_gw);
                    }
                }
                if found {
                    Some(ConnectionCampusStatus { on_campus: true, name: Some(ssid.clone()), message: msg })
                } else {
                    Some(ConnectionCampusStatus {
                        on_campus: false,
                        name: Some(ssid.clone()),
                        message: format!("当前WiFi\"{}\"非校园网络", ssid),
                    })
                }
            }
            None => {
                if wifi_adapters.is_empty() {
                    None
                } else {
                    let mut found = false;
                    let mut msg = String::new();
                    for a in &wifi_adapters {
                        if !a.ip.is_empty() {
                            let same_subnet = crate::network::is_same_subnet_18(&a.ip, campus_gw);
                            crate::log_info!("campus", "[校园网检测] WiFi子网检查: adapter={}, ip={}, /18匹配={}", a.name, a.ip, same_subnet);
                            if same_subnet {
                                found = true;
                                msg = format!("WiFi已连接校园网({}与网关在同一/18网段)", a.ip);
                                break;
                            }
                        }
                    }
                    // 仅当至少一个 WiFi 网卡拥有合法 IP 时，才信任网关可达性
                    // 否则可达性可能来自其他类型网卡（如有线），错误归因到 WiFi
                    if !found && wifi_adapters.iter().any(|a| !a.ip.is_empty()) {
                        let gateway_ok = check_gateway(campus_gw, &mut gateway_checked);
                        if gateway_ok {
                            found = true;
                            msg = format!("WiFi通过网关{}连接校园网", campus_gw);
                        }
                    }
                    if found {
                        Some(ConnectionCampusStatus { on_campus: true, name: None, message: msg })
                    } else {
                        Some(ConnectionCampusStatus { on_campus: false, name: None, message: "WiFi未连接校园网".to_string() })
                    }
                }
            }
        }
    };

    let wired_status = {
        let wired_adapters: Vec<&crate::network::Adapter> = adapters.iter().filter(|a| !a.wireless).collect();
        if wired_adapters.is_empty() {
            None
        } else {
            match &wired_profile {
                Some(profile) if profile.eq_ignore_ascii_case(required_name) => {
                    crate::log_info!("campus", "[校园网检测] ✅ 有线名称匹配: '{}'", profile);
                    Some(ConnectionCampusStatus {
                        on_campus: true,
                        name: Some(profile.clone()),
                        message: format!("已连接到校园有线网络({})", profile),
                    })
                }
                _ => {
                    let mut found = false;
                    let mut msg = String::new();
                    for a in &wired_adapters {
                        if !a.ip.is_empty() {
                            let same_subnet = crate::network::is_same_subnet_18(&a.ip, campus_gw);
                            crate::log_info!("campus", "[校园网检测] 有线子网检查: adapter={}, ip={}, /18匹配={}", a.name, a.ip, same_subnet);
                            if same_subnet {
                                found = true;
                                msg = format!("有线已连接校园网({}与网关在同一/18网段)", a.ip);
                                break;
                            }
                        }
                    }
                    // 仅当至少一个有线网卡拥有合法 IP 时，才信任网关可达性
                    // 否则可达性可能来自其他类型网卡（如 WiFi），错误归因到有线
                    if !found && wired_adapters.iter().any(|a| !a.ip.is_empty()) {
                        let gateway_ok = check_gateway(campus_gw, &mut gateway_checked);
                        if gateway_ok {
                            found = true;
                            msg = format!("有线通过网关{}连接校园网", campus_gw);
                        }
                    }
                    if found {
                        Some(ConnectionCampusStatus { on_campus: true, name: wired_profile.clone(), message: msg })
                    } else {
                        let fail_msg = match &wired_profile {
                            Some(p) => format!("当前有线网络\"{}\"非校园网络", p),
                            None => "有线网络未连接校园网".to_string(),
                        };
                        Some(ConnectionCampusStatus { on_campus: false, name: wired_profile.clone(), message: fail_msg })
                    }
                }
            }
        }
    };

    let on_campus = wifi_status.as_ref().map(|s| s.on_campus).unwrap_or(false)
        || wired_status.as_ref().map(|s| s.on_campus).unwrap_or(false);

    let message = if on_campus {
        let mut parts = Vec::new();
        if let Some(ref ws) = wifi_status {
            if ws.on_campus { parts.push(ws.message.clone()); }
        }
        if let Some(ref ws) = wired_status {
            if ws.on_campus { parts.push(ws.message.clone()); }
        }
        if parts.is_empty() { "已连接校园网".to_string() } else { parts.join("；") }
    } else {
        let mut parts = Vec::new();
        if let Some(ref ws) = wifi_status {
            if !ws.on_campus { parts.push(ws.message.clone()); }
        }
        if let Some(ref ws) = wired_status {
            if !ws.on_campus { parts.push(ws.message.clone()); }
        }
        if parts.is_empty() { "未连接到校园网络".to_string() } else { parts.join("；") }
    };

    crate::log_info!("campus", "[校园网检测] 结果: on_campus={}, wifi={:?}, wired={:?}, message={}",
        on_campus, wifi_status.as_ref().map(|s| s.on_campus), wired_status.as_ref().map(|s| s.on_campus), message);

    CampusCheckResult {
        wifi: wifi_status,
        wired: wired_status,
        on_campus,
        current_ssid: wifi_ssid.or(wired_profile),
        message,
    }
}

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

    let a1 = adapters.iter().find(|a| a.name == adapter1_name && !a.ip.is_empty());
    let a2 = if config.dual_adapter && !adapter2_name.is_empty() {
        adapters.iter().find(|a| a.name == adapter2_name && !a.ip.is_empty())
    } else {
        None
    };

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
            message: format!("校园网检测静默期（早于{}:{:02}），跳过验证", hour, minute),
        }
    } else {
        check_campus_network(&config, &adapters)
    };
    state.network.current_ssid.store(std::sync::Arc::new(campus_result.current_ssid.clone()));
    // 始终更新 on_campus_network（静默期内 campus_result.on_campus=true，确保 emit 字段一致）
    state.network.on_campus_network.store(campus_result.on_campus, Ordering::Release);

    if config.enable_network_name_check && !campus_result.on_campus {
        crate::log_debug!("background", "校园网检测未通过: {}", campus_result.message);
        state.network.any_adapter_online.store(false, Ordering::Release);
        state.network.last_a1_online.store(false, Ordering::Release);
        let a1_campus = adapter_campus_message(&adapter1_name, &adapters, &campus_result);
        let a2_campus = if config.dual_adapter && !adapter2_name.is_empty() {
            adapter_campus_message(&adapter2_name, &adapters, &campus_result)
        } else { None };
        let a1_on_campus = adapter_campus_status(&adapter1_name, &adapters, &campus_result).map(|s| s.on_campus);
        let a2_on_campus = if config.dual_adapter && !adapter2_name.is_empty() {
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
            std::thread::scope(|s| {
                let h1 = s.spawn(|| check_adapter_portal(adapter1, app_handle));
                let h2 = s.spawn(|| check_adapter_portal(adapter2, app_handle));
                let r1 = h1.join().unwrap_or(PortalCheckResult::Error { is_request_failed: false });
                let r2 = h2.join().unwrap_or(PortalCheckResult::Error { is_request_failed: false });
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
                let prev = state.network.a1_auth_failure_count.swap(0, Ordering::AcqRel);
                if prev > 0 {
                    crate::log_debug!("background", "适配器1 网关不可达，重置失败计数(原值={})", prev);
                }
            } else {
                let prev_count = state.network.a1_auth_failure_count.fetch_add(1, Ordering::AcqRel);
                let new_count = prev_count + 1;
                crate::log_info!("background", "适配器1 Portal失败计数: {}/5 (网关可达)", new_count);
                if new_count >= 5 {
                    crate::log_warn!("background", "适配器1 连续{}次Portal失败(网关可达)，触发该适配器MAC重置", new_count);
                    let _ = app_handle.emit("login-log", serde_json::json!({
                        "message": "适配器1 连续5次 Portal 请求失败，正在重置该适配器MAC...",
                        "type": "warning"
                    }));
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
                    state.network.a1_auth_failure_count.store(0, Ordering::Release);
                }
            }
        }

        // 适配器2 失败处理
        if secondary_is_request_failed {
            let gw_reachable = crate::network::check_gateway_reachable_from(campus_gw, a2_ip);
            if !gw_reachable {
                crate::log_info!("background", "适配器2 Portal失败但网关[{}]从[{}]不可达，跳过计数（校园网断网/维护）", campus_gw, a2_ip.unwrap_or(""));
                let prev = state.network.a2_auth_failure_count.swap(0, Ordering::AcqRel);
                if prev > 0 {
                    crate::log_debug!("background", "适配器2 网关不可达，重置失败计数(原值={})", prev);
                }
            } else {
                let prev_count = state.network.a2_auth_failure_count.fetch_add(1, Ordering::AcqRel);
                let new_count = prev_count + 1;
                crate::log_info!("background", "适配器2 Portal失败计数: {}/5 (网关可达)", new_count);
                if new_count >= 5 {
                    crate::log_warn!("background", "适配器2 连续{}次Portal失败(网关可达)，触发该适配器MAC重置", new_count);
                    let _ = app_handle.emit("login-log", serde_json::json!({
                        "message": "适配器2 连续5次 Portal 请求失败，正在重置该适配器MAC...",
                        "type": "warning"
                    }));
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
                    state.network.a2_auth_failure_count.store(0, Ordering::Release);
                }
            }
        }
    } else {
        // 任一适配器 Success 即重置对应计数器
        let primary_success = matches!(&primary_result, PortalCheckResult::Success { .. });
        let secondary_success = secondary_result.as_ref().map(|r| matches!(r, PortalCheckResult::Success { .. })).unwrap_or(false);

        if primary_success {
            let prev = state.network.a1_auth_failure_count.swap(0, Ordering::AcqRel);
            if prev > 0 {
                crate::log_debug!("background", "适配器1 Portal检测恢复正常，重置失败计数(原值={})", prev);
            }
        }
        if secondary_success {
            let prev = state.network.a2_auth_failure_count.swap(0, Ordering::AcqRel);
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

    let prev_online = state.network.any_adapter_online.load(Ordering::Acquire);

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
            if config.dual_adapter && !adapter2_name.is_empty() && !a2_has_ip {
                let msg = adapter_campus_message(&adapter2_name, &adapters, &campus_result);
                match msg {
                    Some(ref m) => (Some(false), m.clone()),
                    None => (None, String::new()),
                }
            } else if config.dual_adapter && !adapter2_name.is_empty() && a2_has_ip {
                (None, secondary_result.as_ref().map(|r| r.message().to_string()).unwrap_or_default())
            } else {
                (None, String::new())
            }
        }
    };

    state.network.last_a2_online.store(secondary_online == Some(true), Ordering::Release);

    handle_status_change(
        prev_online, online, reachable, login_available,
        &adapter1_name, &message,
        &adapter2_name, if secondary_message.is_empty() { None } else { Some(secondary_message.as_str()) },
        &config, app_handle,
    );

    let a1_campus = adapter_campus_message(&adapter1_name, &adapters, &campus_result);
    let a2_campus = if config.dual_adapter && !adapter2_name.is_empty() {
        adapter_campus_message(&adapter2_name, &adapters, &campus_result)
    } else { None };
    let a1_on_campus = adapter_campus_status(&adapter1_name, &adapters, &campus_result).map(|s| s.on_campus);
    let a2_on_campus = if config.dual_adapter && !adapter2_name.is_empty() {
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
        let quality = check_network_quality_async(&adapter_name, &adapter_ip, skip_ttfb, skip_content, &fixed_gateway, s.exit.is_quitting.clone(), Some(app_handle)).await;
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
        let cfg = state.update_config(|cfg| {
            cfg.enable_background_check = true;
            if cfg.background_check_interval < 10000 {
                cfg.background_check_interval = 15000;
            }
        });
        let interval = cfg.background_check_interval;
        (interval, cfg)
    };

    if let Err(e) = crate::commands::config_cmd::save_config_to_disk_encrypted(app_handle, &cfg) {
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
        interval_timer.tick().await;
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
