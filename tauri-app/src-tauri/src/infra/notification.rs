use tauri::{AppHandle, Manager};
use crate::infra::events::EventBus;

/// 系统通知服务
///
/// Phase 1 中统一通知入口：先向前端发射 `system-notification` 事件，
/// 再根据配置决定是否调用系统通知 API。
pub struct NotificationService<'a> {
    app_handle: &'a AppHandle,
    event_bus: EventBus<'a>,
}

impl<'a> NotificationService<'a> {
    pub fn new(app_handle: &'a AppHandle) -> Self {
        Self {
            app_handle,
            event_bus: EventBus::new(app_handle),
        }
    }

    /// 发送系统通知
    ///
    /// 1. 无论是否启用系统通知，都向前端发送 `system-notification` 事件
    /// 2. 仅在非前台且用户启用通知时调用操作系统通知 API
    pub fn notify(&self, title: &str, body: &str) {
        if let Err(e) = self.event_bus.emit_system_notification(title, body) {
            crate::log_warn!("system", "发送系统通知事件失败: {}", e);
        }

        let enable_notification = {
            let s = self.app_handle.state::<crate::infra::state::AppState>();
            s.config.load().enable_notification
        };
        if !enable_notification {
            return;
        }

        let is_focused = self.app_handle.get_webview_window("main")
            .map(|w| w.is_focused().unwrap_or(false))
            .unwrap_or(false);
        if is_focused {
            return;
        }

        let title = title.to_string();
        let body = body.to_string();
        let app_h = self.app_handle.clone();
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
}

/// 兼容旧接口：直接使用函数形式发送通知
///
/// 保留该函数可避免一次性修改所有调用点；新增代码推荐直接使用 `NotificationService`。
pub fn emit_notification(app_handle: &AppHandle, title: &str, body: &str) {
    NotificationService::new(app_handle).notify(title, body);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_service_struct_size_check() {
        // 编译期保证：NotificationService 持有两个引用（app_handle + event_bus）
        assert_eq!(std::mem::size_of::<NotificationService<'_>>(), std::mem::size_of::<&AppHandle>() * 2);
    }
}
