use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Manager, Window, WindowEvent};

/// 窗口「被应用自身隐藏」标记：隐藏到托盘、静默启动等用户意图导致的不可见。
///
/// 启动兜底显示线程与前端 show_window 命令据此区分「该显示却没显示」与
/// 「用户就是要它不可见」，避免后者被强拉回前台（v2.4.2 缺陷）。
static HIDDEN_BY_DESIGN: AtomicBool = AtomicBool::new(false);

pub fn mark_hidden_by_design(hidden: bool) {
    HIDDEN_BY_DESIGN.store(hidden, Ordering::Release);
}

pub fn is_hidden_by_design() -> bool {
    HIDDEN_BY_DESIGN.load(Ordering::Acquire)
}

/// 挂起 / 恢复主 WebView2 渲染进程（最小化、隐藏到托盘、轻量化关闭共用）。
///
/// 挂起走微软官方休眠范式：先把 controller.IsVisible 置 false（TrySuspend 的硬
/// 约束，否则返回 ERROR_INVALID_STATE），再 `TrySuspend` —— 渲染进程停派帧、
/// 暂停脚本定时器，内存允许被 OS 回收；恢复用 `Resume` 再置可见。仅靠
/// `SetIsVisible(false)` 只停合成、不停定时器，CSS 无限动画仍在空转。
pub fn set_main_webview_suspended(app: &tauri::AppHandle, suspended: bool) {
    #[cfg(target_os = "windows")]
    if let Some(ww) = app.get_webview_window("main") {
        let _ = ww.as_ref().with_webview(move |pw| {
            // [忽略错误] COM 调用在窗口销毁竞态下失败无补救意义
            let _ = unsafe { apply_webview_suspended(&pw, suspended) };
        });
    }
    #[cfg(not(target_os = "windows"))]
    let _ = (app, suspended);
}

#[cfg(target_os = "windows")]
unsafe fn apply_webview_suspended(
    pw: &tauri::webview::PlatformWebview,
    suspended: bool,
) -> windows_core::Result<()> {
    use webview2_com_sys::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_19, ICoreWebView2_3, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL,
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
    };
    use windows_core::Interface;

    let controller = pw.controller();
    let core = unsafe { controller.CoreWebView2() }?;

    // 内存目标级别与挂起解耦：挂起期 LOW（尽快归还），恢复后 NORMAL
    if let Ok(icw2_19) = core.cast::<ICoreWebView2_19>() {
        let raw = if suspended {
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
        } else {
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
        };
        let _ = unsafe {
            icw2_19.SetMemoryUsageTargetLevel(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL(raw.0))
        };
    }

    let icw2_3 = core.cast::<ICoreWebView2_3>()?;
    if suspended {
        // 顺序不可颠倒：IsVisible=false 是 TrySuspend 的前置条件
        unsafe { controller.SetIsVisible(false) }?;
        let handler = webview2_com::TrySuspendCompletedHandler::create(Box::new(
            |hr, succeeded| {
                match (hr, succeeded) {
                    (Ok(()), true) => {
                        crate::log_info!("window", "WebView2 已挂起：渲染停帧、脚本定时器暂停")
                    }
                    (Ok(()), false) => crate::log_warn!(
                        "window",
                        "WebView2 挂起请求未被接受（页面仍有脚本在执行），渲染继续"
                    ),
                    (Err(e), _) => crate::log_warn!("window", "WebView2 挂起失败: {}", e),
                }
                Ok(())
            },
        ));
        unsafe { icw2_3.TrySuspend(&handler) }?;
    } else {
        unsafe { icw2_3.Resume() }?;
        unsafe { controller.SetIsVisible(true) }?;
    }
    Ok(())
}

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
    if LAST_MINIMIZED.swap(minimized, Ordering::Relaxed) == minimized {
        return;
    }

    // 隐藏到托盘的窗口不归尺寸事件管：它本就挂起，别被还原事件顺手唤醒
    if !minimized && is_hidden_by_design() {
        return;
    }

    // 最小化即整体挂起（停帧 + 暂停脚本定时器），还原即恢复
    set_main_webview_suspended(window.app_handle(), minimized);
}

/// 非 Windows 平台为空实现
#[cfg(not(target_os = "windows"))]
pub fn handle_window_resized_event(_window: &tauri::WebviewWindow, _event: &WindowEvent) {}

/// 显示主窗口并聚焦。窗口不存在或操作失败时静默忽略。
///
/// 对已最小化的窗口会 unminimize（SW_RESTORE）还原到前台——这是托盘单击、
/// single_instance 二次启动等用户主动唤起路径的期望语义。前端启动期自动显示
/// （getInitData 完成后调 show_window 命令）不允许打扰用户最小化，最小化保护
/// 在该命令内做（commands/config_cmd.rs show_window）。
pub fn show_and_focus_main<M: tauri::Manager<tauri::Wry>>(app: &M) {
    if let Some(window) = app.get_webview_window("main") {
        mark_hidden_by_design(false);
        // 先恢复渲染再显示：Resume 是挂起的显式解药（SetIsVisible(true) 也会
        // 触发自动恢复，但显式 Resume 能立刻解冻，避免首帧等待）
        set_main_webview_suspended(&window.app_handle(), false);
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
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
