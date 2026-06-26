use tauri::{AppHandle, Manager};
use crate::infra::events::EventBus;

/// 发送系统通知
///
/// 1. 无论是否启用系统通知，都向前端发送 `system-notification` 事件
/// 2. 仅在非前台且用户启用通知时调用操作系统通知 API
pub fn emit_notification(app_handle: &AppHandle, title: &str, body: &str) {
    let event_bus = EventBus::new(app_handle);
    if let Err(e) = event_bus.emit_system_notification(title, body) {
        crate::log_warn!("system", "发送系统通知事件失败: {}", e);
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
