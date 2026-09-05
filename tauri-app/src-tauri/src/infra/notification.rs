use tauri::{AppHandle, Manager};

/// 发送系统通知（Windows 通知中心）
///
/// 仅在"用户看不到主窗口且用户启用通知"时调用操作系统通知 API；
/// 用户看不到界面时弹系统通知，是这类通知的唯一出口。
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

    // 抑制判定与 app/heartbeat.rs 的 monitorable 语义一致：窗口可见且未最小化即视为
    // 用户能看到的界面，此时不弹系统通知（改用应用内 toast/日志通道）。
    // 仅看 is_focused 会在窗口可见但失焦（焦点在别的窗口）时误弹，打扰正在看界面的用户；
    // 仅看 is_visible 又因 Win32 特性在最小化时仍返回 true，无法单独作为"能看到"依据。
    let monitorable = app_handle.get_webview_window("main")
        .map(|w| w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false))
        .unwrap_or(false);
    if monitorable {
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
