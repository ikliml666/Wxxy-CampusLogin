use tauri::{AppHandle, State, Window};
use std::sync::atomic::Ordering;
use crate::infra::command_context::CommandContext;
use crate::infra::state::{AppState, CommandResult};
use crate::platform::autostart;

#[tauri::command]
pub fn minimize_window(window: Window) -> Result<(), String> {
    window.minimize().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn close_window(window: Window, state: State<'_, AppState>) -> Result<(), String> {
    let minimize_to_tray = state.config.load().minimize_to_tray;
    if minimize_to_tray {
        window.hide().map_err(|e| e.to_string())
    } else {
        state.exit.is_quitting.store(true, Ordering::Release);
        window.close().map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn open_external(url: String) -> Result<bool, String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("仅支持http/https 链接".to_string());
    }
    if url.len() > 2048 {
        return Err("URL长度超出限制".to_string());
    }
    let parsed = url::Url::parse(&url).map_err(|e| format!("URL解析失败: {e}"))?;
    if parsed.username() != "" || parsed.password().is_some() {
        return Err("URL不允许包含用户名或密码".to_string());
    }
    open::that(&url).map(|_| true).map_err(|e| format!("打开链接失败: {e}"))
}

#[tauri::command]
pub fn get_auto_launch() -> Result<serde_json::Value, String> {
    let enabled = autostart::get_auto_launch_enabled();
    Ok(serde_json::json!({ "enabled": enabled }))
}

#[tauri::command]
pub fn set_auto_launch(enabled: bool, app_handle: AppHandle, state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let exe_path = std::env::current_exe().map_err(|e| format!("获取程序路径失败: {e}"))?;
    let exe_str = exe_path.to_str().ok_or("程序路径无效")?;

    // 先执行注册表操作，成功后再更新配置，避免注册表失败但配置已改导致状态矛盾
    let result = if enabled {
        autostart::set_auto_start(exe_str)
    } else {
        autostart::remove_auto_start()
    };

    match result {
        Ok(_) => {
            let cfg = state.config.update(|cfg| {
                cfg.auto_launch = enabled;
            });
            if let Err(e) = super::config_cmd::save_config_to_disk_encrypted(&app_handle, &cfg) {
                crate::log_warn!("system", "保存自启配置失败: {}", e);
            }
            crate::log_info!("system", "开机自启已{}", if enabled { "开启" } else { "关闭" });
            Ok(serde_json::json!({ "success": true, "message": if enabled { "已开启开机自启" } else { "已关闭开机自启" } }))
        }
        Err(e) => {
            crate::log_error!("system", "设置开机自启失败: {}", e);
            Ok(serde_json::json!({ "success": false, "message": format!("设置开机自启失败: {}", e) }))
        }
    }
}

#[tauri::command]
pub fn get_notification_enabled(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.config.load().enable_notification)
}

#[tauri::command]
pub fn set_notification_enabled(enabled: bool, state: State<'_, AppState>, app_handle: AppHandle) -> Result<bool, String> {
    let cfg = state.config.update(|cfg| {
        cfg.enable_notification = enabled;
    });
    if let Err(e) = super::config_cmd::save_config_to_disk_encrypted(&app_handle, &cfg) {
        crate::log_warn!("system", "保存通知设置失败: {}", e);
    }
    crate::log_info!("system", "通知已{}", if enabled { "开启" } else { "关闭" });
    Ok(enabled)
}

#[tauri::command]
pub fn cancel_auto_exit(app_handle: AppHandle, _state: State<'_, AppState>) -> Result<CommandResult, String> {
    let s = CommandContext::from_app(&app_handle);
    // 统一取消：同时取消自动退出和校园网退出
    let result = crate::infra::lifecycle::cancel_auto_exit_inner(&app_handle, &s);
    crate::infra::lifecycle::cancel_campus_exit_with_notification(&app_handle, &s);
    result
}

#[tauri::command]
pub fn get_logs(app_handle: AppHandle, lines: Option<usize>) -> Result<String, String> {
    let n = lines.unwrap_or(200);
    crate::infra::logger::read_recent_logs(&app_handle, n)
}

#[tauri::command]
pub fn clear_logs(app_handle: AppHandle) -> Result<bool, String> {
    crate::infra::logger::clear_logs(&app_handle)?;
    Ok(true)
}

#[tauri::command]
pub fn get_init_data(state: State<'_, AppState>, app_handle: AppHandle) -> Result<serde_json::Value, String> {
    // 出站掩码唯一出口（含 self_password）：state 内是解密后的明文，漏掩码会把
    // 明文发给 webview，且前端 selfPasswordSaved（依赖 === MASK）永远 false →
    // 重启后密码框显示空、切入自助服务面板的自动 Hello 验证永不触发（2026-09-06 真机缺陷）
    let cfg = state.config.load().masked_for_display();

    let accounts = {
        let data_dir = crate::config::persist::get_data_dir(&app_handle);
        crate::config::persist::list_account_items(&data_dir)
    };

    let version = env!("APP_VERSION").to_string();
    let auto_launch = crate::platform::autostart::get_auto_launch_enabled();
    let gpu_info = crate::platform::gpu::detect_gpu_info();
    let refresh_rate = crate::platform::gpu::detect_display_refresh_rate();

    let adapters = crate::network::get_adapters_cached().unwrap_or_default();
    let adapter_details = crate::network::get_adapter_details_cached().unwrap_or_default();
    let disabled_adapters = crate::network::get_disabled_adapters_cached().unwrap_or_default();
    let active_account = cfg.active_account.clone();
    let notification_enabled = cfg.enable_notification;

    let is_auto_start = std::env::args().any(|a| a == "--autostart");

    // 复用上方已取好的适配器快照，避免 get_background_status_value 内部重复全量取一次
    let bg_status = super::background::get_background_status_value(&state, &app_handle, &adapters);

    Ok(serde_json::json!({
        "config": cfg,
        "accounts": accounts,
        "version": version,
        "autoLaunch": auto_launch,
        "gpuInfo": gpu_info,
        "refreshRate": refresh_rate,
        "adapters": adapters,
        "adapterDetails": adapter_details,
        "disabledAdapters": disabled_adapters,
        "activeAccount": active_account,
        "notificationEnabled": notification_enabled,
        "isAutoStart": is_auto_start,
        "backgroundStatus": bg_status,
    }))
}

#[tauri::command]
pub fn render_heartbeat(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    // 更新前端心跳时间戳，供 main.rs 心跳监控线程检测 WebView 崩溃后自动重载
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    state.update_stats.last_render_heartbeat_ms.store(now_ms, Ordering::Release);

    let online = state.network.load().any_adapter_online;
    let checking = state.tasks.is_checking.is_active();
    Ok(serde_json::json!({
        "online": online,
        "checking": checking,
    }))
}

#[tauri::command]
pub fn get_gpu_info() -> Result<serde_json::Value, String> {
    let info = crate::platform::gpu::detect_gpu_info();
    Ok(serde_json::to_value(info).unwrap_or(serde_json::json!({ "gpu": "unknown" })))
}

#[tauri::command]
pub fn set_log_retention_days(days: u32) -> Result<(), String> {
    crate::infra::logger::set_log_retention_days(days);
    Ok(())
}

#[tauri::command]
pub fn get_log_retention_days() -> u32 {
    crate::infra::logger::get_log_retention_days()
}

/// 导出诊断包到 <data_dir>/diagnostics/diag-<时间戳>/，返回目录路径（P2-29）。
/// 包含：近 days 天应用日志（days=0 视为全部，与日志保留 0=永久语义一致）、
/// 掩码后配置（敏感出站唯一出口 masked_for_display，严禁明文密码）、
/// 适配器列表与详情快照、GPU 信息与刷新率、manifest.json 内容清单。
#[tauri::command]
pub fn export_diagnostics(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    days: Option<u32>,
) -> Result<String, String> {
    let days = days.unwrap_or(3);
    // 先 flush：避免最后一批日志仍停留在 logger 缓冲，拷出截断的当天日志
    crate::infra::logger::flush();

    let data_dir = crate::config::persist::get_data_dir(&app_handle);
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let out_dir = data_dir.join("diagnostics").join(format!("diag-{stamp}"));
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("创建诊断目录失败: {e}"))?;

    // 1) 近 N 天应用日志（文件名过滤复用 clear_logs 同款判断）
    let log_dir = crate::infra::logger::get_log_dir(&app_handle);
    let cutoff = if days == 0 {
        None
    } else {
        std::time::SystemTime::now()
            .checked_sub(std::time::Duration::from_secs(u64::from(days) * 86400))
    };
    let mut log_files: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_string) else { continue };
            if !crate::infra::logger::is_app_log_file(&name) {
                continue;
            }
            if let Some(cutoff) = cutoff {
                let Ok(modified) = entry.metadata().and_then(|m| m.modified()) else { continue };
                if modified < cutoff {
                    continue;
                }
            }
            // 单文件拷贝失败（如被占用）不致命：跳过，不影响其余内容
            if std::fs::copy(entry.path(), out_dir.join(&name)).is_ok() {
                log_files.push(name);
            }
        }
    }
    log_files.sort();

    // 2) 掩码后配置（唯一出口）
    let masked = state.config.load().masked_for_display();
    let config_json = serde_json::to_string_pretty(&masked).map_err(|e| format!("序列化配置失败: {e}"))?;
    std::fs::write(out_dir.join("config-masked.json"), config_json)
        .map_err(|e| format!("写入配置快照失败: {e}"))?;

    // 3) 适配器列表与详情快照（枚举失败时写空数组，不阻断导出）
    let adapters_json = serde_json::json!({
        "adapters": crate::network::get_adapters_cached().unwrap_or_default(),
        "adapterDetails": crate::network::get_adapter_details_cached().unwrap_or_default(),
    });
    std::fs::write(out_dir.join("adapters.json"), serde_json::to_string_pretty(&adapters_json).unwrap_or_else(|_| "{}".into()))
        .map_err(|e| format!("写入适配器快照失败: {e}"))?;

    // 4) GPU 信息与刷新率
    let gpu_json = serde_json::json!({
        "gpu": crate::platform::gpu::detect_gpu_info(),
        "refreshRateHz": crate::platform::gpu::detect_display_refresh_rate(),
    });
    std::fs::write(out_dir.join("gpu.json"), serde_json::to_string_pretty(&gpu_json).unwrap_or_else(|_| "{}".into()))
        .map_err(|e| format!("写入 GPU 快照失败: {e}"))?;

    // 5) manifest：内容清单与生成环境
    let manifest = serde_json::json!({
        "type": "campus-login-diagnostics",
        "appVersion": env!("APP_VERSION"),
        "os": std::env::consts::OS,
        "generatedAt": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        "logDays": days,
        "contents": {
            "logFiles": log_files,
            "config": "config-masked.json (passwords masked)",
            "adapters": "adapters.json",
            "gpu": "gpu.json",
        },
    });
    std::fs::write(out_dir.join("manifest.json"), serde_json::to_string_pretty(&manifest).unwrap_or_else(|_| "{}".into()))
        .map_err(|e| format!("写入 manifest 失败: {e}"))?;

    crate::log_info!("system", "诊断包导出成功: {:?}", out_dir);
    Ok(out_dir.to_string_lossy().to_string())
}
