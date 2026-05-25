use tauri::{AppHandle, Emitter, Manager, State, Window};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use parking_lot::Mutex;
use crate::config::{get_data_dir, get_login_history_path, atomic_write, list_account_names};
use super::state::{AppState, CommandResult};

const AUTOSTART_REG_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
const AUTOSTART_REG_VALUE: &str = "CampusLogin";

static LOGIN_HISTORY_LOCK: Mutex<()> = Mutex::new(());

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

fn get_auto_launch_enabled() -> bool {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey(AUTOSTART_REG_KEY) {
        if let Ok(val) = key.get_value::<String, _>(AUTOSTART_REG_VALUE) {
            return !val.is_empty();
        }
    }
    false
}

fn set_auto_launch_registry(enabled: bool, exe_path: &str) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey_with_flags(AUTOSTART_REG_KEY, KEY_SET_VALUE)
        .map_err(|e| format!("打开注册表失败: {}", e))?;

    if enabled {
        let value = format!("\"{}\" --autostart", exe_path);
        key.set_value(AUTOSTART_REG_VALUE, &value)
            .map_err(|e| format!("写入注册表失败: {}", e))?;
    } else {
        if let Err(e) = key.delete_value(AUTOSTART_REG_VALUE) {
            crate::log_warn!("system", "删除自启动注册表项失败: {}", e);
        }
    }

    Ok(())
}

#[tauri::command]
pub fn get_auto_launch() -> Result<serde_json::Value, String> {
    let enabled = get_auto_launch_enabled();
    Ok(serde_json::json!({ "enabled": enabled }))
}

#[tauri::command]
pub fn set_auto_launch(enabled: bool, app_handle: AppHandle, state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let exe_path = std::env::current_exe().map_err(|e| format!("获取程序路径失败: {}", e))?;
    let exe_str = exe_path.to_str().ok_or("程序路径无效")?;

    // 先写磁盘，确认成功后再更新内存状态
    {
        let current = state.config.load();
        let mut cfg = current.as_ref().clone();
        cfg.auto_launch = enabled;
        if let Err(e) = super::config_cmd::save_config_to_disk(&app_handle, &cfg) {
            return Ok(serde_json::json!({ "success": false, "message": format!("保存配置失败: {}", e) }));
        }
        // 磁盘写入成功后才更新内存
        state.config.store(Arc::new(cfg));
    }

    let result = set_auto_launch_registry(enabled, exe_str);

    match result {
        Ok(_) => {
            Ok(serde_json::json!({ "success": true, "message": if enabled { "已开启开机自启" } else { "已关闭开机自启" } }))
        }
        Err(e) => Ok(serde_json::json!({ "success": false, "message": format!("设置开机自启失败: {}", e) })),
    }
}

#[tauri::command]
pub fn get_notification_enabled(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.config.load().enable_notification)
}

#[tauri::command]
pub fn set_notification_enabled(enabled: bool, state: State<'_, AppState>, app_handle: AppHandle) -> Result<bool, String> {
    {
        let current = state.config.load();
        let mut cfg = current.as_ref().clone();
        cfg.enable_notification = enabled;
        state.config.store(Arc::new(cfg.clone()));
        if let Err(e) = super::config_cmd::save_config_to_disk(&app_handle, &cfg) {
            crate::log_warn!("system", "保存通知设置失败: {}", e);
        }
    }
    Ok(enabled)
}

pub fn emit_notification(app_handle: &AppHandle, title: &str, body: &str) {
    if let Err(e) = app_handle.emit("system-notification", serde_json::json!({
        "title": title,
        "body": body,
    })) {
        crate::log_warn!("system", "发送系统通知失败: {}", e);
    }

    let enable_notification = {
        let s = app_handle.state::<crate::commands::state::AppState>();
        s.config.load().enable_notification
    };
    if !enable_notification {
        return;
    }

    let is_focused = app_handle.get_webview_window("main")
        .map(|w| w.is_focused().unwrap_or(false))
        .unwrap_or(false);
    if is_focused {
        return;
    }

    let title = title.to_string();
    let body = body.to_string();
    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        use tauri_plugin_notification::NotificationExt;
        match app_h.notification()
            .builder()
            .title(&title)
            .body(&body)
            .show()
        {
            Ok(_) => {}
            Err(e) => crate::log_warn!("system", "系统通知发送失败: {}", e),
        }
    });
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
    super::auto_exit::cancel_auto_exit_inner(&app_handle, &s)
}

pub fn append_login_history(app_handle: &AppHandle, success: bool, message: &str, adapter: &str, user: &str, login_type: &str) -> Result<(), String> {
    let _lock = LOGIN_HISTORY_LOCK.lock();
    let data_dir = get_data_dir(app_handle);
    let history_path = get_login_history_path(&data_dir);
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

    atomic_write(&history_path, &json)?;

    Ok(())
}

#[tauri::command]
pub fn get_logs(app_handle: AppHandle, lines: Option<usize>) -> Result<String, String> {
    let n = lines.unwrap_or(200);
    crate::logger::read_recent_logs(&app_handle, n)
}

#[tauri::command]
pub fn clear_logs(app_handle: AppHandle) -> Result<bool, String> {
    crate::logger::clear_logs(&app_handle)?;
    Ok(true)
}

#[tauri::command]
pub async fn get_init_data(app_handle: AppHandle, state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let config = state.config.load_full();
    let display_config = config.masked_for_display();

    let notification_enabled = config.enable_notification;
    let active_account = config.active_account.clone();

    let app_h = app_handle.clone();
    let (adapter_result, auto_launch_result, accounts_result) = tokio::join!(
        tauri::async_runtime::spawn_blocking(|| {
            let (adapters, details, disabled) = crate::network::get_all_adapters_cached().unwrap_or_default();
            (adapters, details, disabled)
        }),
        tauri::async_runtime::spawn_blocking(get_auto_launch_enabled),
        tauri::async_runtime::spawn_blocking({
            let app_h = app_h.clone();
            move || list_account_names(&app_h)
        }),
    );

    let (a, d, dd) = adapter_result.unwrap_or_default();
    let al = auto_launch_result.unwrap_or(false);
    let acc = accounts_result.unwrap_or_default();

    let was_online = state.network.any_adapter_online.load(Ordering::Acquire);
    let server_avail = state.network.server_available.load(Ordering::Acquire);

    let adapter_statuses = {
        let mut statuses = Vec::new();
        if let Ok(adapters) = crate::network::get_adapters_cached() {
            let (adapter1_name, adapter2_name) = crate::network::resolve_adapter_names(&adapters, &config);
            if let Some(a1) = adapters.iter().find(|a| a.name == adapter1_name) {
                if a1.ip.is_empty() {
                    statuses.push(serde_json::json!({
                        "name": adapter1_name, "ip": "", "wireless": a1.wireless,
                        "online": false, "message": "未连接"
                    }));
                } else {
                    statuses.push(serde_json::json!({
                        "name": adapter1_name, "ip": a1.ip, "wireless": a1.wireless,
                        "online": was_online, "message": if was_online { "已在线" } else { "未在线" }
                    }));
                }
            } else if !adapter1_name.is_empty() {
                statuses.push(serde_json::json!({
                    "name": adapter1_name, "ip": "", "wireless": false,
                    "online": false, "message": "已禁用"
                }));
            }
            if config.dual_adapter && !adapter2_name.is_empty() {
                if let Some(a2) = adapters.iter().find(|a| a.name == adapter2_name) {
                    if a2.ip.is_empty() {
                        statuses.push(serde_json::json!({
                            "name": adapter2_name, "ip": "", "wireless": a2.wireless,
                            "online": false, "message": "未连接"
                        }));
                    } else {
                        statuses.push(serde_json::json!({
                            "name": adapter2_name, "ip": a2.ip, "wireless": a2.wireless,
                            "online": was_online, "message": if was_online { "已在线" } else { "未在线" }
                        }));
                    }
                } else {
                    statuses.push(serde_json::json!({
                        "name": adapter2_name, "ip": "", "wireless": false,
                        "online": false, "message": "已禁用"
                    }));
                }
            }
        }
        serde_json::Value::Array(statuses)
    };

    let bg_status = serde_json::json!({
        "isRunning": state.tasks.background_running.is_active(),
        "checkCount": state.network.background_check_count.load(Ordering::Acquire),
        "serverAvailable": server_avail,
        "online": was_online,
        "adapterStatuses": adapter_statuses,
        "currentSsid": state.network.current_ssid.load().as_ref(),
        "onCampusNetwork": state.network.on_campus_network.load(Ordering::Acquire),
        "enableNetworkNameCheck": config.enable_network_name_check,
        "requiredNetworkName": config.required_network_name,
    });

    let is_auto_start = std::env::args().any(|a| a == "--autostart");
    let cpu_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    Ok(serde_json::json!({
        "config": display_config,
        "adapters": a,
        "adapterDetails": d,
        "disabledAdapters": dd,
        "autoLaunch": al,
        "notificationEnabled": notification_enabled,
        "backgroundStatus": bg_status,
        "accounts": acc,
        "activeAccount": active_account,
        "isAutoStart": is_auto_start,
        "cpuCores": cpu_cores,
    }))
}
