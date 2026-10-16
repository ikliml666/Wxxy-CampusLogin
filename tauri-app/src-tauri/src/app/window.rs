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

/// 上次同步给 WebView2 的最小化状态。Resized 事件高频（拖拽调整尺寸逐像素
/// 触发），仅在 is_minimized 翻转时才触碰 COM，其余事件直接跳过。
#[cfg(target_os = "windows")]
static LAST_MINIMIZED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 窗口最小化状态翻转时同步 WebView2 可见性：最小化挂起渲染，还原时恢复。
///
/// 背景：wry 只在显式 hide()/show() 时调 controller.SetIsVisible（见 wry
/// 0.55.1 webview2/mod.rs，SetIsVisible 仅创建时与 set_visible 两处调用），
/// 窗口最小化不会触发任何通知；WebView2 默认也不做原生遮挡检测（微软要求
/// 宿主在窗口不可见时主动 PutIsVisible(false)）。结果页面长期处于「半挂起」：
/// 定时器被节流但渲染合成继续派发，最小化后 rAF/CSS 动画/合成器全速空转，
/// v2.4.1 为 .glass-dock 加 backdrop-filter 后每帧成本陡增，问题显性化。
#[cfg(target_os = "windows")]
pub fn handle_window_resized_event(window: &Window, event: &WindowEvent) {
    let WindowEvent::Resized(_) = event else {
        return;
    };

    let minimized = window.is_minimized().unwrap_or(false);
    use std::sync::atomic::Ordering;
    if LAST_MINIMIZED.swap(minimized, Ordering::Relaxed) == minimized {
        return;
    }

    if let Some(ww) = window.app_handle().get_webview_window("main") {
        let _ = ww.as_ref().with_webview(move |pw| {
            let controller = pw.controller();
            let _ = unsafe { controller.SetIsVisible(!minimized) };
        });
    }
}

/// 非 Windows 平台为空实现
#[cfg(not(target_os = "windows"))]
pub fn handle_window_resized_event(_window: &tauri::WebviewWindow, _event: &WindowEvent) {}

/// 显示主窗口。窗口不存在或操作失败时静默忽略。
///
/// **最小化保护**：窗口已最小化时不调 set_focus/unminimize——set_focus 在最小化
/// 下虽是 no-op（tao 有 is_minimized 守卫），unminimize 会清 MINIMIZED flag 发
/// SW_RESTORE 把窗口拉回前台。启动后短时间内用户最小化，前端 getInitData 完成
/// 即调 show_window（v2.4.2 前）会经由本函数把窗口弹回，故最小化态只 show()
/// 恢复渲染、不还原窗口。用户主动唤起场景（托盘单击等）需要还原时，另行显式
/// 调 unminimize（本仓库现有主动路径均经由 show_and_focus_main，暂不放开）。
pub fn show_and_focus_main<M: tauri::Manager<tauri::Wry>>(app: &M) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        if window.is_minimized().unwrap_or(false) {
            return;
        }
        let _ = window.set_focus();
        let _ = window.unminimize();
    }
}

/// 统一入口：窗口在则显示聚焦；不在（轻量化已销毁）则重建。
/// 托盘「显示主窗口」、托盘左键单击、single_instance 回调三处共用。
/// 接 AppHandle：重建路径需要 available_monitors（AppHandle 固有方法，
/// 不在 Manager trait 上）。
pub fn show_or_rebuild_main(app: &tauri::AppHandle) {
    match app.get_webview_window("main") {
        Some(_) => show_and_focus_main(app),
        None => {
            if let Err(e) = rebuild_main_window(app) {
                crate::log_error!("lightweight", "重建主窗口失败: {}", e);
            }
        }
    }
}

/// 从 tauri.conf.json 窗口配置重建主窗口（绝不用 WindowBuilder——会得到
/// 无 webview 的白窗口，tauri issue #9307）。visible(false) 起步，等前端
/// notify_window_ready 或 5s 超时兜底才 show，防重建白闪。
fn rebuild_main_window(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let window_config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .cloned()
        .ok_or("tauri.conf.json 中未找到 main 窗口配置")?;

    let mut builder = tauri::WebviewWindowBuilder::from_config(app, &window_config)?;
    // 几何恢复：离屏（拔显示器）则回退配置默认（center）
    if let Some(geo) = crate::app::lightweight::take_geometry() {
        let monitors: Vec<(i32, i32, u32, u32)> = app
            .available_monitors()
            .map(|list| {
                list.iter()
                    .map(|m| {
                        let p = m.position();
                        let s = m.size();
                        (p.x, p.y, s.width, s.height)
                    })
                    .collect()
            })
            .unwrap_or_default();
        if crate::app::lightweight::geometry_is_on_screen(&geo, &monitors) {
            builder = builder.position(geo.x, geo.y).inner_size(geo.width, geo.height);
        } else {
            crate::log_warn!("lightweight", "记忆的窗口几何已不在任何显示器上，回退居中默认");
        }
    }
    let window = builder.build()?;
    crate::log_info!("lightweight", "主窗口已重建，等待前端就绪后显示");

    // 退出轻量化态：间隔恢复、效率模式关闭
    crate::app::lightweight::set_lightweight_active(false);
    #[cfg(windows)]
    crate::platform::ecoqos::set_ecoqos(false);

    // ready 门：等前端 notify_window_ready 或 5s 超时
    let app_h = app.clone();
    let win_label = window.label().to_string();
    tauri::async_runtime::spawn(async move {
        crate::app::lightweight::wait_window_ready_and_show(&app_h, &win_label).await;
    });
    Ok(())
}
