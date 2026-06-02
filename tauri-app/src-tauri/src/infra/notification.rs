use tauri::{AppHandle, Emitter, Manager};

pub fn emit_notification(app_handle: &AppHandle, title: &str, body: &str) {
    if let Err(e) = app_handle.emit("system-notification", serde_json::json!({
        "title": title,
        "body": body,
    })) {
        crate::log_warn!("system", "发送系统通知失败: {}", e);
    }

    let enable_notification = {
        let s = app_handle.state::<crate::infra::state::AppState>();
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
