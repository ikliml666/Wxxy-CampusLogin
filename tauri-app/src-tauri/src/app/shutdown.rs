use tauri::{AppHandle, Manager, Window, WindowEvent};
use crate::infra::command_context::CommandContext;
use crate::infra::state::AppState;

/// 应用优雅退出
///
/// 通过 `shutdown_and_exit` 取消并等待所有后台任务结束后退出进程。
pub fn graceful_exit(app_handle: &AppHandle, _state: &AppState) {
    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let s = app_h.state::<AppState>();
        crate::infra::lifecycle::shutdown_and_exit(&app_h, &s).await;
    });
}

/// 窗口关闭事件处理
///
/// 根据配置决定是最小化到托盘还是执行优雅退出。
pub fn handle_window_close_event(window: &Window, event: &WindowEvent) {
    let WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };

    let s = CommandContext::from_app(window.app_handle());
    let minimize_to_tray = s.config.load().minimize_to_tray;
    if minimize_to_tray {
        api.prevent_close();
        if let Some(ww) = window.app_handle().get_webview_window("main") {
            let _ = ww.hide(); // [忽略错误] 窗口可能已关闭
        }
    } else {
        let app_h = window.app_handle().clone();
        graceful_exit(&app_h, &s);
    }
}
