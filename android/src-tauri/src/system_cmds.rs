//! 聚合初始化与日志命令:与桌面 commands/system.rs 同构的安卓子集。


/// 启动聚合数据:前端 useInitialDataLoad 一次拉全(config 已掩码,明文不出后端)
#[tauri::command]
pub async fn get_init_data(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let settings = crate::config_state::current_settings(&app).await?;
    let dir = crate::account_cmds::accounts_dir(&app)?;
    // AccountItem[](id + displayName,空显示名已兜底为 id);activeAccount 语义不变(账号 id)
    let accounts = crate::account_cmds::list_account_items_sync(&dir);

    Ok(serde_json::json!({
        "config": crate::config_state::masked_for_display(&settings),
        "accounts": accounts,
        "version": env!("CARGO_PKG_VERSION"),
        // 桌面专属字段补空默认,避免前端 useInitialDataLoad 读到 undefined 崩溃
        "autoLaunch": settings.enable_boot_autostart,
        "gpuInfo": serde_json::Value::Null,
        "refreshRate": 60,
        "adapters": [],
        "adapterDetails": [],
        "disabledAdapters": [],
        "activeAccount": settings.active_account,
        "notificationEnabled": settings.enable_notification,
        // 与桌面 get_init_data 同形：原一次性 get 命令并入返回体（logger 运行期真实值）
        "logRetentionDays": campus_login_lib::infra::logger::get_log_retention_days(),
        "debugMode": campus_login_lib::infra::logger::get_debug_mode().unwrap_or(false),
        "isAutoStart": settings.enable_boot_autostart,
        "backgroundStatus": crate::monitor_loop::status_value(),
    }))
}

#[tauri::command]
pub async fn get_logs(app: tauri::AppHandle, lines: Option<usize>) -> Result<String, String> {
    let lines = lines.unwrap_or(200);
    tauri::async_runtime::spawn_blocking(move || {
        campus_login_lib::infra::logger::read_recent_logs(&app, lines)
    })
    .await
    .map_err(|e| format!("读取日志任务失败: {e}"))?
}

#[tauri::command]
pub async fn clear_logs(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || campus_login_lib::infra::logger::clear_logs(&app))
        .await
        .map_err(|e| format!("清理日志任务失败: {e}"))?
}

#[tauri::command]
pub fn get_log_retention_days() -> u32 {
    campus_login_lib::infra::logger::get_log_retention_days()
}

#[tauri::command]
pub fn set_log_retention_days(days: u32) {
    campus_login_lib::infra::logger::set_log_retention_days(days);
}

#[tauri::command]
pub fn get_debug_mode() -> Result<bool, String> {
    campus_login_lib::infra::logger::get_debug_mode()
}

#[tauri::command]
pub fn set_debug_mode(enabled: bool) -> Result<bool, String> {
    campus_login_lib::infra::logger::set_debug_mode(enabled)
}
