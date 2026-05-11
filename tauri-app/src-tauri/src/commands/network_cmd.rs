use tauri::{AppHandle, Manager, State};
use std::sync::atomic::Ordering;
use crate::network::{
    Adapter, AdapterDetail, DisabledAdapter,
    get_adapters_cached, get_disabled_adapters_cached,
    enable_adapter as enable_adapter_inner, get_adapter_details_cached,
    check_portal_full, dhcp_renew_wired_only,
    select_adapter,
    check_network_quality_async,
};

fn is_restricted_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => v4.is_loopback() || v4.is_link_local(),
        std::net::IpAddr::V6(v6) => v6.is_loopback(),
    }
}
use crate::http_timing::{measure_https_timing, measure_dns_query, measure_doh_timing, HttpTimingResult, DnsQueryResult, DohTimingResult};
use super::state::{AppState, CommandResult, atomic_guard};

fn empty_quality_json() -> serde_json::Value {
    serde_json::json!({ "gatewayLatency": -1, "externalLatency": -1, "gateway": "", "quality": "unknown", "timestamp": 0, "details": {}, "metrics": {} })
}

#[tauri::command]
pub async fn get_adapters() -> Result<Vec<Adapter>, String> {
    tauri::async_runtime::spawn_blocking(|| get_adapters_cached()).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_disabled_adapters() -> Result<Vec<DisabledAdapter>, String> {
    tauri::async_runtime::spawn_blocking(|| get_disabled_adapters_cached()).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn enable_adapter(adapter_name: String) -> Result<CommandResult, String> {
    if adapter_name.is_empty() {
        return Err("适配器名称不能为空".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || enable_adapter_inner(&adapter_name)).await.map_err(|e| e.to_string())??;
    Ok(CommandResult::ok_msg("适配器已启用"))
}

#[tauri::command]
pub async fn get_adapter_details() -> Result<Vec<AdapterDetail>, String> {
    tauri::async_runtime::spawn_blocking(|| get_adapter_details_cached()).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn check_portal_status(adapter_ip: String) -> Result<serde_json::Value, String> {
    if adapter_ip.is_empty() {
        return Ok(serde_json::json!({
            "online": false,
            "message": "IP地址为空",
        }));
    }
    tauri::async_runtime::spawn_blocking(move || {
        let status = check_portal_full(&adapter_ip, None)?;
        Ok(serde_json::json!({
            "online": status.online,
            "message": status.message,
        }))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn dhcp_renew_all() -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let results = dhcp_renew_wired_only()?;
        Ok(serde_json::json!({ "success": true, "results": results }))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn check_network_quality(app_handle: AppHandle) -> Result<serde_json::Value, String> {
    let state = app_handle.state::<AppState>();
    if !state.config.load().enable_network_quality {
        return Ok(serde_json::json!({"quality": "disabled"}));
    }
    if !state.tasks.is_quality_checking.swap(true, Ordering::Acquire) {
        let (adapter_ip, adapter_name, skip_ttfb, skip_content, fixed_gateway) = {
            let config = state.config.load();
            let adapters = match get_adapters_cached() {
                Ok(a) => a,
                Err(_) => {
                    state.tasks.is_quality_checking.store(false, Ordering::Release);
                    return Ok(empty_quality_json());
                }
            };
            let (ip, name) = select_adapter(&adapters, &config);
            (ip, name, config.skip_ttfb_in_latency, config.skip_content_in_latency, config.fixed_gateway.clone())
        };
        if adapter_ip.is_empty() {
            state.tasks.is_quality_checking.store(false, Ordering::Release);
            return Ok(empty_quality_json());
        }
        atomic_guard!(QualityGuard, is_quality_checking);
        let _guard = QualityGuard(&state);
        let result = check_network_quality_async(&adapter_name, &adapter_ip, skip_ttfb, skip_content, &fixed_gateway, state.is_quitting.clone()).await;
        drop(_guard);
        Ok(serde_json::to_value(&result).unwrap_or_default())
    } else {
        Ok(serde_json::json!({"quality": "busy"}))
    }
}

#[tauri::command]
pub fn start_latency_test(app_handle: AppHandle, _state: State<'_, AppState>) -> Result<CommandResult, String> {
    let s = app_handle.state::<AppState>();
    if s.tasks.latency_running.swap(true, Ordering::Acquire) {
        return Ok(CommandResult::ok_msg("延迟测试已在运行"));
    }

    let interval = {
        let config = s.config.load();
        if config.latency_test_interval < 10000 { 30000 } else { config.latency_test_interval }
    };

    super::background::spawn_latency_test_loop(&app_handle, interval);

    Ok(CommandResult::ok_msg("延迟测试已启动"))
}

#[tauri::command]
pub fn stop_latency_test(state: State<'_, AppState>) -> Result<CommandResult, String> {
    state.tasks.latency_running.store(false, Ordering::Release);
    Ok(CommandResult::ok_msg("延迟测试已停止"))
}

#[tauri::command]
pub fn get_latency_test_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let running = state.tasks.latency_running.load(Ordering::Acquire);
    let config = state.config.load();
    Ok(serde_json::json!({
        "enabled": config.enable_latency_test,
        "isRunning": running,
        "interval": config.latency_test_interval
    }))
}

#[tauri::command]
pub async fn http_timing_test(url: String) -> Result<HttpTimingResult, String> {
    if url.len() > 2048 {
        return Err("URL长度超出限制".to_string());
    }
    let parsed = url::Url::parse(&url).map_err(|e| format!("URL解析失败: {}", e))?;
    let scheme = parsed.scheme();
    if scheme != "https" {
        return Err("仅支持HTTPS协议测试".to_string());
    }
    let host = parsed.host_str().ok_or("URL缺少主机名")?.to_string();
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        if is_restricted_ip(&ip) {
            return Err("不允许测试内网地址".to_string());
        }
    } else if host == "localhost" {
        return Err("不允许测试内网地址".to_string());
    }
    let port = parsed.port().unwrap_or(443);
    let bind_addr: Option<std::net::IpAddr> = None;
    Ok(measure_https_timing(&host, port, bind_addr, std::time::Duration::from_secs(10), false, false).await)
}

#[tauri::command]
pub async fn dns_query_test(server_ip: String, domain: String) -> Result<DnsQueryResult, String> {
    if domain.is_empty() || domain.len() > 253 {
        return Err("域名参数无效".to_string());
    }
    if server_ip.is_empty() {
        return Err("DNS服务器地址不能为空".to_string());
    }
    if let Ok(ip) = server_ip.parse::<std::net::IpAddr>() {
        if is_restricted_ip(&ip) {
            return Err("不允许查询内网DNS服务器".to_string());
        }
    }
    let bind_addr: Option<std::net::IpAddr> = None;
    Ok(measure_dns_query(&server_ip, &domain, bind_addr, std::time::Duration::from_millis(3000)).await)
}

#[tauri::command]
pub async fn doh_timing_test(server: String, server_ip: String, domain: String) -> Result<DohTimingResult, String> {
    if domain.is_empty() || domain.len() > 253 {
        return Err("域名参数无效".to_string());
    }
    if server.is_empty() || server.len() > 253 {
        return Err("DoH服务器地址无效".to_string());
    }
    if !server_ip.is_empty() {
        if let Ok(ip) = server_ip.parse::<std::net::IpAddr>() {
            if is_restricted_ip(&ip) {
                return Err("不允许测试内网DoH服务器".to_string());
            }
        }
    }
    let bind_addr: Option<std::net::IpAddr> = None;
    Ok(measure_doh_timing(&server, &server_ip, &domain, bind_addr, std::time::Duration::from_secs(5), false).await)
}
