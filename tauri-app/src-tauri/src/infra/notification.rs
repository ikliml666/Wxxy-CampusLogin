use tauri::{AppHandle, Manager};

/// 发送系统通知（Windows 通知中心）
///
/// 仅在"应用不在前台且用户启用通知"时调用操作系统通知 API；
/// 应用不在前台时弹系统通知，是这类通知的唯一出口。
///
/// 应用内（前台）的 toast/日志展示由各业务专用事件负责
/// （`onAutoLoginResult`/`onAutoExitCountdown`/`emit_login_log` 等），
/// 本函数不再向前端发事件——双通道 toast 重复的历史缺陷由此根除。
/// 系统通知文案为中文硬编码：后端无法感知前端 UI 语言（i18next 存储在前端），
/// 且该通知仅在用户未看界面时出现，主要用户群为中文。
pub fn emit_notification(app_handle: &AppHandle, title: &str, body: &str) {
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
