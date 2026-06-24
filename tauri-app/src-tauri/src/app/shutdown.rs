use tauri::{AppHandle, Manager, Window, WindowEvent};
use std::sync::atomic::Ordering;
use std::time::Duration;
use crate::infra::state::AppState;

/// 应用优雅退出
///
/// 取消所有后台任务，释放运行标志，然后退出进程。
pub fn graceful_exit(app_handle: &AppHandle, state: &AppState) {
    state.exit.is_quitting.store(true, Ordering::Release);
    state.tasks.bg_check_cancel.load().cancel();
    state.tasks.latency_cancel.load().cancel();
    state.tasks.adapter_watch_cancel.load().cancel();

    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        // 短暂等待后台任务响应取消
        tokio::time::sleep(Duration::from_millis(200)).await;
        let s = app_h.state::<AppState>();
        s.tasks.background_running.force_release();
        s.tasks.latency_running.force_release();
        s.tasks.adapter_watch_running.force_release();
        crate::log_info!("app", "应用退出");
        app_h.exit(0);
    });
}

/// 窗口关闭事件处理
///
/// 根据配置决定是最小化到托盘还是执行优雅退出。
pub fn handle_window_close_event(window: &Window, event: &WindowEvent) {
    let WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };

    let s = window.state::<AppState>();
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
