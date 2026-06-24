use tauri::{Manager, Window, WindowEvent};

/// 窗口焦点变化时调整 WebView2 内存使用目标级别
#[cfg(target_os = "windows")]
pub fn handle_window_focus_event(window: &Window, event: &WindowEvent) {
    let WindowEvent::Focused(focused) = event else {
        return;
    };

    use windows_core::Interface;
    use webview2_com_sys::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL,
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
    };

    let level = if *focused {
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL.0)
    } else {
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW.0)
    };

    if let Some(ww) = window.app_handle().get_webview_window("main") {
        let _ = ww.as_ref().with_webview(move |pw| {
            let controller = pw.controller();
            if let Ok(core_webview) = unsafe { controller.CoreWebView2() } {
                if let Ok(icw2_19) = core_webview.cast::<ICoreWebView2_19>() {
                    let _ = unsafe { icw2_19.SetMemoryUsageTargetLevel(level) };
                }
            }
        });
    }
}

/// 非 Windows 平台为空实现
#[cfg(not(target_os = "windows"))]
pub fn handle_window_focus_event(_window: &tauri::WebviewWindow, _event: &WindowEvent) {}
