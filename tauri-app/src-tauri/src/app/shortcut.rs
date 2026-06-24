use tauri::{AppHandle, Manager};
use crate::infra::state::{AppState, CANCEL_EXIT_SHORTCUT};

/// 全局快捷键插件事件处理
pub fn handle_shortcut_event(app: &AppHandle, shortcut: &tauri_plugin_global_shortcut::Shortcut, event: tauri_plugin_global_shortcut::ShortcutEvent) {
    use tauri_plugin_global_shortcut::ShortcutState;
    if event.state() != ShortcutState::Pressed {
        return;
    }

    let Ok(cancel_key) = CANCEL_EXIT_SHORTCUT.parse::<tauri_plugin_global_shortcut::Shortcut>() else {
        return;
    };

    if *shortcut != cancel_key {
        return;
    }

    let app_h = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let s = app_h.state::<AppState>();
        // 统一取消：同时取消自动退出和校园网退出
        let _ = crate::infra::lifecycle::cancel_auto_exit_inner(&app_h, &s);
        crate::infra::lifecycle::cancel_campus_exit_with_notification(&app_h, &s);
    });
}
