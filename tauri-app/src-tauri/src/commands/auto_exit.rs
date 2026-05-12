use tauri::{AppHandle, Emitter, Manager};
use std::sync::atomic::Ordering;
use std::time::Duration;
use super::state::{AppState, CommandResult, AUTO_EXIT_DELAY_MS, CANCEL_EXIT_SHORTCUT};
use super::system::emit_notification;

pub fn start_auto_exit(app_handle: &AppHandle, state: &AppState) {
    let should_start = {
        let deadline = state.exit.deadline();
        if deadline.is_some() {
            false
        } else {
            state.exit.auto_exit_cancelled.store(false, Ordering::Release);
            state.exit.set_deadline(Some(std::time::Instant::now() + Duration::from_millis(AUTO_EXIT_DELAY_MS)));
            true
        }
    };

    if !should_start {
        return;
    }

    if let Err(e) = app_handle.emit("auto-exit-countdown", serde_json::json!({
        "delay": AUTO_EXIT_DELAY_MS,
        "shortcut": "Ctrl+Shift+C",
    })) {
        crate::log_warn!("auto_exit", "发送退出倒计时事件失败: {}", e);
    }

    emit_notification(app_handle, "即将自动退出", &format!("{}秒后自动退出，按 Ctrl+Shift+C 可取消", AUTO_EXIT_DELAY_MS / 1000));

    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let shortcut_registered = if !app_handle.global_shortcut().is_registered(CANCEL_EXIT_SHORTCUT) {
        app_handle.global_shortcut().register(CANCEL_EXIT_SHORTCUT).is_ok()
    } else {
        true
    };

    if !shortcut_registered {
        crate::log_warn!("auto_exit", "快捷键注册失败，请通过界面取消自动退出");
    }

    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let sleep_duration = {
            let s = app_h.state::<AppState>();
            let deadline = s.exit.deadline();
            match deadline {
                Some(d) => d.saturating_duration_since(std::time::Instant::now()),
                None => Duration::from_millis(AUTO_EXIT_DELAY_MS),
            }
        };
        tokio::time::sleep(sleep_duration).await;
        let s = app_h.state::<AppState>();
        {
            let deadline = s.exit.deadline();
            if let Some(d) = deadline {
                if std::time::Instant::now() < d {
                    return;
                }
            } else {
                return;
            }
        }
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        if app_h.global_shortcut().is_registered(CANCEL_EXIT_SHORTCUT) {
            let _ = app_h.global_shortcut().unregister(CANCEL_EXIT_SHORTCUT); // [忽略错误] 快捷键注销失败不影响退出流程
        }
        s.exit.is_quitting.store(true, Ordering::Release);
        app_h.exit(0);
    });
}

pub fn cancel_auto_exit_inner(app_handle: &AppHandle, state: &AppState) -> Result<CommandResult, String> {
    {
        let deadline = state.exit.deadline();
        if deadline.is_none() {
            return Ok(CommandResult::ok_msg("无需取消"));
        }
        state.exit.set_deadline(None);
    }
    state.exit.auto_exit_cancelled.store(true, Ordering::Release);

    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    if app_handle.global_shortcut().is_registered(CANCEL_EXIT_SHORTCUT) {
        let _ = app_handle.global_shortcut().unregister(CANCEL_EXIT_SHORTCUT); // [忽略错误] 快捷键注销失败不影响取消流程
    }

    emit_notification(app_handle, "已取消退出", "自动退出已取消，程序将继续运行");

    if let Err(e) = app_handle.emit("auto-exit-cancelled", serde_json::json!({})) {
        crate::log_warn!("auto_exit", "发送取消退出事件失败: {}", e);
    }

    Ok(CommandResult::ok_msg("自动退出已取消"))
}
