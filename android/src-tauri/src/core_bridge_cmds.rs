//! UI 进程的 core 转发命令面。
//!
//! UI 不再运行后台核心:启动/停止/立即检测/登录/注销全部经 Binder 转发 :core,
//! 状态读 `:core` 唯一写入的 `core_status.json`。命令名与返回形状与改造前保持一致,
//! 前端无需改动。

use serde_json::Value;
use tauri::Manager;

#[cfg(mobile)]
fn forward(app: &tauri::AppHandle, action: &str, args: &str) -> Result<String, String> {
    use tauri_plugin_campus_monitor_service::CampusMonitorServiceExt;
    app.campus_monitor_service()
        .core_command(action, args)
        .map_err(|e| e.to_string())
}

#[cfg(not(mobile))]
fn forward(_app: &tauri::AppHandle, _action: &str, _args: &str) -> Result<String, String> {
    Err("后台核心仅安卓端支持".to_string())
}

fn parse_result(raw: String) -> Result<Value, String> {
    let v: Value = serde_json::from_str(&raw).map_err(|e| format!("core 返回非 JSON: {e}"))?;
    if let Some(err) = v["error"].as_str() {
        return Err(err.to_string());
    }
    Ok(v)
}

fn call(app: &tauri::AppHandle, action: &str, args: &str) -> Result<Value, String> {
    parse_result(forward(app, action, args)?)
}

#[tauri::command]
pub async fn start_background_check(app: tauri::AppHandle) -> Result<Value, String> {
    call(&app, "start", "{}")
}

#[tauri::command]
pub async fn stop_background_check(app: tauri::AppHandle) -> Result<Value, String> {
    call(&app, "stop", "{}")
}

#[tauri::command]
pub async fn trigger_background_check(app: tauri::AppHandle) -> Result<(), String> {
    call(&app, "trigger", "{}").map(|_| ())
}

#[tauri::command]
pub async fn get_background_status(app: tauri::AppHandle) -> Result<Value, String> {
    Ok(read_background_status(&app))
}

/// 读 :core 写的状态文件,组装成前端既有形状(不再由 UI 进程内存态给出)
pub fn read_background_status(app: &tauri::AppHandle) -> Value {
    let status = app
        .path()
        .app_data_dir()
        .map(|dir| crate::core::status_file::read(&dir))
        .unwrap_or_default();
    let last = status.last_result.clone();
    let now = crate::core::platform::now_ms();
    let core_alive = campus_core_logic::status::core_is_alive(
        &status,
        now,
        campus_core_logic::status::MAX_STATUS_AGE_MS,
    );
    let mut v = serde_json::json!({
        "isRunning": status.is_running && core_alive,
        "checkCount": status.check_count,
        "consecutiveFailures": status.consecutive_failures,
        "reconnectCount": status.reconnect_count,
        "lastResult": last.as_ref(),
        "watcherFailed": status.watcher_failed,
        "coreAlive": core_alive,
        "notificationStale": campus_core_logic::status::notification_stale(
            &status,
            now,
            campus_core_logic::status::MAX_STATUS_AGE_MS,
        ),
    });
    if let Some(last) = &last {
        for key in [
            "online",
            "message",
            "onlineOperator",
            "serverAvailable",
            "onCampusNetwork",
        ] {
            if let Some(val) = last.get(key) {
                v[key] = val.clone();
            }
        }
    }
    v
}

#[tauri::command]
pub async fn do_login(
    user: Option<String>,
    password: Option<String>,
    operator: Option<String>,
    adapter: Option<String>,
    app: tauri::AppHandle,
) -> Result<Value, String> {
    let _ = adapter;
    let args = serde_json::json!({
        "user": user.unwrap_or_default(),
        "password": password.unwrap_or_default(),
        "operator": operator.unwrap_or_default(),
    });
    call(&app, "login", &args.to_string())
}

#[tauri::command]
pub async fn do_logout(
    user: Option<String>,
    adapter: Option<String>,
    app: tauri::AppHandle,
) -> Result<Value, String> {
    let _ = adapter;
    let args = serde_json::json!({ "user": user.unwrap_or_default() });
    call(&app, "logout", &args.to_string())
}

#[tauri::command]
pub async fn get_boot_autostart(app: tauri::AppHandle) -> Result<bool, String> {
    #[cfg(mobile)]
    {
        use tauri_plugin_campus_monitor_service::CampusMonitorServiceExt;
        return app
            .campus_monitor_service()
            .is_boot_autostart_enabled()
            .map_err(|e| e.to_string());
    }
    #[cfg(not(mobile))]
    {
        let _ = app;
        Ok(false)
    }
}

#[tauri::command]
pub async fn set_boot_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    #[cfg(mobile)]
    {
        use tauri_plugin_campus_monitor_service::CampusMonitorServiceExt;
        app.campus_monitor_service()
            .set_boot_autostart(enabled)
            .map_err(|e| e.to_string())?;
    }
    crate::config_state::update(&app, |settings| {
        settings.enable_boot_autostart = enabled;
    })
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn get_notification_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    Ok(crate::config_state::current_settings(&app)
        .await?
        .enable_notification)
}

#[tauri::command]
pub async fn set_notification_enabled(
    app: tauri::AppHandle,
    enabled: bool,
) -> Result<bool, String> {
    crate::config_state::update(&app, |settings| {
        settings.enable_notification = enabled;
    })
    .await?;
    Ok(enabled)
}
/// UI 进程内的系统通知(更新提醒等,不依赖 :core 存活)
pub fn notify_system_local(
    app: &tauri::AppHandle,
    enabled: bool,
    title: &str,
    body: &str,
    mascot: &str,
) {
    if !enabled {
        return;
    }
    use tauri_plugin_notification::NotificationExt;
    let result = app
        .notification()
        .builder()
        .title(title)
        .body(body)
        .large_icon(mascot)
        .show();
    if result.is_err() {
        let _ = app.notification().builder().title(title).body(body).show();
    }
}

/// 账号切换/删除后的 avoid_bad_wifi 收尾统一在 :core 执行,避免两进程各自缓存
/// SharedPreferences 后整文件覆盖,丢掉夜切快照(幂等,无快照无动作)。
pub async fn restore_avoid_bad_wifi_local(app: &tauri::AppHandle) {
    #[cfg(mobile)]
    {
        let handle = app.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || {
            call(&handle, "restoreAvoidBadWifi", "{}")
        })
        .await;
    }
    #[cfg(not(mobile))]
    {
        let _ = app;
    }
}
