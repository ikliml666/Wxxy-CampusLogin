use tauri::{AppHandle, Manager};
use std::sync::atomic::Ordering;
use std::time::Duration;
use crate::infra::state::AppState;

/// 前端心跳检测线程
///
/// 检测前端是否定期发送心跳，若长时间未收到则尝试重载 WebView。
pub fn spawn_heartbeat_thread(app_handle: AppHandle) {
    let task_manager = app_handle.state::<AppState>().task_manager.clone();
    if let Err(e) = task_manager.spawn("heartbeat", move |cancel_token| async move {
        let check_interval = Duration::from_secs(5);
        let crash_threshold_ms: u64 = 20_000;
        let mut consecutive_stale = 0u32;
        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => break,
                _ = tokio::time::sleep(check_interval) => {}
            }
            if let Some(window) = app_handle.get_webview_window("main") {
                // 前端 useHeartbeat 在 document.hidden（含最小化）时暂停发送心跳，
                // 而 Win32 下最小化窗口 is_visible() 仍为 true，仅排除可见性会导致
                // 最小化超过阈值后必误触发重载，故需一并排除最小化状态。
                let monitorable = window.is_visible().unwrap_or(false)
                    && !window.is_minimized().unwrap_or(false);
                if !monitorable {
                    consecutive_stale = 0;
                    continue;
                }
            } else {
                continue;
            }
            let s = app_handle.state::<AppState>();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let last = s.update_stats.last_render_heartbeat_ms.load(Ordering::Acquire);
            if last == 0 {
                continue;
            }
            let elapsed = now.saturating_sub(last);
            if elapsed > crash_threshold_ms {
                consecutive_stale += 1;
                if consecutive_stale >= 3 {
                    crate::log_warn!("heartbeat", "前端心跳丢失 {}ms，尝试重载WebView", elapsed);
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.eval("window.location.reload()");
                    }
                    consecutive_stale = 0;
                }
            } else {
                consecutive_stale = 0;
            }
        }
    }) {
        crate::log_warn!("heartbeat", "启动心跳任务失败: {}", e);
    }
}

/// 窗口兜底显示线程
///
/// 启动 3 秒后检查窗口是否可见，不可见则强制显示，防止 WebView2 初始化时序问题导致黑屏。
pub fn spawn_window_safety_thread(app_handle: AppHandle) {
    let task_manager = app_handle.state::<AppState>().task_manager.clone();
    if let Err(e) = task_manager.spawn("window_safety", move |cancel_token| async move {
        tokio::select! {
            _ = cancel_token.cancelled() => return,
            _ = tokio::time::sleep(Duration::from_secs(3)) => {}
        }
        for attempt in 1..=3 {
            if let Some(window) = app_handle.get_webview_window("main") {
                let is_visible = window.is_visible().unwrap_or(false);
                if is_visible {
                    return;
                }
                crate::log_warn!("startup", "窗口{}秒后仍不可见，第{}次强制显示", 3 * attempt, attempt);
                crate::app::window::show_and_focus_main(&app_handle);
            }
            if attempt < 3 {
                tokio::select! {
                    _ = cancel_token.cancelled() => return,
                    _ = tokio::time::sleep(Duration::from_secs(3)) => {}
                }
            }
        }
    }) {
        crate::log_warn!("heartbeat", "启动窗口兜底任务失败: {}", e);
    }
}
