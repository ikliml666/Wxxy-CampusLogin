use tauri::{AppHandle, Manager, State, Window};
use std::sync::atomic::Ordering;
use crate::infra::state::{AppState, CommandResult};
use crate::infra::notification::emit_notification;
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
pub fn window_move(window: Window, delta_x: i32, delta_y: i32) -> Result<(), String> {
    if delta_x.abs() > 5000 || delta_y.abs() > 5000 {
        return Err("窗口移动距离超出合理范围".to_string());
    }
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    window.set_position(tauri::Position::Physical(
        tauri::PhysicalPosition::new(pos.x + delta_x, pos.y + delta_y)
    )).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_external(url: String) -> Result<bool, String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("仅支持http/https 链接".to_string());
    }
    if url.len() > 2048 {
        return Err("URL长度超出限制".to_string());
    }
    let parsed = url::Url::parse(&url).map_err(|e| format!("URL解析失败: {}", e))?;
    if parsed.username() != "" || parsed.password().is_some() {
        return Err("URL不允许包含用户名或密码".to_string());
    }
    open::that(&url).map(|_| true).map_err(|e| format!("打开链接失败: {}", e))
}

#[tauri::command]
pub fn get_auto_launch() -> Result<serde_json::Value, String> {
    let enabled = autostart::get_auto_launch_enabled();
    Ok(serde_json::json!({ "enabled": enabled }))
}

#[tauri::command]
pub fn set_auto_launch(enabled: bool, app_handle: AppHandle, state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let exe_path = std::env::current_exe().map_err(|e| format!("获取程序路径失败: {}", e))?;
    let exe_str = exe_path.to_str().ok_or("程序路径无效")?;

    let cfg = state.update_config(|cfg| {
        cfg.auto_launch = enabled;
    });

    if let Err(e) = super::config_cmd::save_config_to_disk_encrypted(&app_handle, &cfg) {
        return Ok(serde_json::json!({ "success": false, "message": format!("保存配置失败: {}", e) }));
    }

    let result = if enabled {
        autostart::set_auto_start(exe_str)
    } else {
        autostart::remove_auto_start()
    };

    match result {
        Ok(_) => {
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
    let cfg = state.update_config(|cfg| {
        cfg.enable_notification = enabled;
    });
    if let Err(e) = super::config_cmd::save_config_to_disk_encrypted(&app_handle, &cfg) {
        crate::log_warn!("system", "保存通知设置失败: {}", e);
    }
    crate::log_info!("system", "通知已{}", if enabled { "开启" } else { "关闭" });
    Ok(enabled)
}

#[tauri::command]
pub fn send_notification(title: String, body: String, app_handle: AppHandle) -> Result<bool, String> {
    if title.is_empty() || title.len() > 256 {
        return Err("通知标题长度需在1-256之间".to_string());
    }
    if body.len() > 1024 {
        return Err("通知内容过长".to_string());
    }
    emit_notification(&app_handle, &title, &body);
    Ok(true)
}

#[tauri::command]
pub fn cancel_auto_exit(app_handle: AppHandle, _state: State<'_, AppState>) -> Result<CommandResult, String> {
    let s = app_handle.state::<AppState>();
    // 统一取消：同时取消自动退出和校园网退出
    let result = crate::infra::lifecycle::cancel_auto_exit_inner(&app_handle, &s);
    crate::infra::lifecycle::cancel_campus_exit_with_notification(&app_handle, &s);
    result
}

pub fn append_login_history(app_handle: &AppHandle, success: bool, message: &str, adapter: &str, user: &str, login_type: &str) -> Result<(), String> {
    let data_dir = crate::config::persist::get_data_dir(app_handle);
    let history_path = crate::config::persist::get_login_history_path(&data_dir);
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("创建数据目录失败: {}", e))?;

    let mut history: Vec<serde_json::Value> = if history_path.exists() {
        let content = std::fs::read_to_string(&history_path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        vec![]
    };

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    history.insert(0, serde_json::json!({
        "time": now,
        "success": success,
        "message": message,
        "adapter": adapter,
        "user": user,
        "type": login_type
    }));

    if history.len() > 100 {
        history.truncate(100);
    }

    let json = serde_json::to_string_pretty(&history)
        .map_err(|e| format!("序列化登录历史失败: {}", e))?;

    crate::config::persist::atomic_write(&history_path, &json)?;

    Ok(())
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
    let config = state.config.load();
    let mut cfg = config.as_ref().clone();
    cfg.password = crate::config::model::PASSWORD_MASK.to_string();

    let data_dir = crate::config::persist::get_data_dir(&app_handle);
    let accounts_dir = crate::config::persist::get_accounts_dir(&data_dir);
    let mut accounts = Vec::new();
    if accounts_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&accounts_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") { continue; }
                let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or("").to_string();
                if name.starts_with('.') || name.is_empty() { continue; }
                accounts.push(name);
            }
        }
    }
    accounts.sort();

    let version = env!("CARGO_PKG_VERSION").to_string();
    let auto_launch = crate::platform::autostart::get_auto_launch_enabled();
    let gpu_info = crate::platform::gpu::detect_gpu_info();
    let refresh_rate = crate::platform::gpu::detect_display_refresh_rate();

    let adapters = crate::network::get_adapters_cached().unwrap_or_default();
    let adapter_details = crate::network::get_adapter_details_cached().unwrap_or_default();
    let disabled_adapters = crate::network::get_disabled_adapters_cached().unwrap_or_default();
    let active_account = config.active_account.clone();
    let notification_enabled = config.enable_notification;

    let is_auto_start = std::env::args().any(|a| a == "--autostart");

    let bg_status = super::background::get_background_status_value(&state, &app_handle);

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
    let online = state.network.any_adapter_online.load(Ordering::Acquire);
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
