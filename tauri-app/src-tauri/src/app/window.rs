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

/// 用户已主动收起窗口（最小化或隐藏到托盘 / 静默启动中）。
///
/// 后台任务看到 true 就应静默完成，不得把窗口拉回前台（v2.4.2 强拉缺陷）。
/// 托盘单击、single_instance、通知点击等用户主动唤起路径不走本判定。
pub fn is_user_dismissed(window: &tauri::WebviewWindow) -> bool {
    window.is_minimized().unwrap_or(false) || is_hidden_by_design()
}

/// 挂起 / 恢复主 WebView2（最小化、隐藏到托盘、轻量化关闭共用）。
///
/// **只用 `controller.SetIsVisible`，绝不调用 `ICoreWebView2_3::TrySuspend`。**
/// v2.4.2 曾经按微软休眠范式加上 `TrySuspend`/`Resume` —— 那是本条缺陷的根因：
/// TrySuspend 把渲染进程整体冻结，React 并发调度器排在 `MessageChannel` 上的
/// 调度回调随之丢失；进程恢复后 `ReactFiberRoot.pendingLanes` 仍有待办更新，
/// 但 `callbackNode` 永远为空，React 再也不渲染。表现为输入链路、onClick、
/// zustand store 全部正常，DOM 却一成不变 —— 正好就是用户看到的「按钮全失效、
/// 切页无反应，只有合成器线程的滚动还能用」。
///
/// `SetIsVisible(false)` 让 WebView 进入隐藏态：rAF 停止、定时器降频、内存级别
/// 可降到 LOW，但渲染进程与 React 调度器都活着，恢复后立即可用。
pub fn set_main_webview_suspended(app: &tauri::AppHandle, suspended: bool) {
    #[cfg(target_os = "windows")]
    if let Some(ww) = app.get_webview_window("main") {
        let _ = ww.as_ref().with_webview(move |pw| {
            // [忽略错误] COM 调用在窗口销毁竞态下失败无补救意义
            let _ = apply_webview_suspended(&pw, suspended);
        });
    }
    #[cfg(not(target_os = "windows"))]
    let _ = (app, suspended);

    // 工作集回收只在最小化路径触发（见 handle_window_resized_event 与
    // commands::system::minimize_window）：隐藏到托盘要求秒恢复，换出后重新换入
    // 反而更慢，不符合该模式的"状态全保留"语义。
}

/// 新 WebView 建立/销毁后复位挂起相关静态态：`LAST_MINIMIZED` 是跨窗口生命
/// 周期的进程级标志，轻量化重建窗口后若残留旧值，会让新窗口的首个 Resized
/// 事件被去抖吞掉（该恢复时不恢复）。
pub fn reset_suspend_state_for_new_webview() {
    #[cfg(target_os = "windows")]
    LAST_MINIMIZED.store(false, Ordering::Release);
}

#[cfg(target_os = "windows")]
fn apply_webview_suspended(
    pw: &tauri::webview::PlatformWebview,
    suspended: bool,
) -> windows_core::Result<()> {
    use webview2_com_sys::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL,
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
    };
    use windows_core::Interface;

    let controller = pw.controller();
    let core = unsafe { controller.CoreWebView2() }?;

    // 内存目标级别与可见性解耦：隐藏期 LOW（尽快归还），恢复后 NORMAL
    match core.cast::<ICoreWebView2_19>() {
        Ok(icw2_19) => {
            let raw = if suspended {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
            } else {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
            };
            if let Err(e) = unsafe {
                icw2_19.SetMemoryUsageTargetLevel(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL(raw.0))
            } {
                crate::log_warn!("webview", "设置内存使用目标级别({})失败: {}", suspended, e);
            }
        }
        // 运行时 < 105 没有 ICoreWebView2_19：LOW 完全不生效，必须留证据
        Err(_) => crate::log_warn!(
            "webview",
            "ICoreWebView2_19 不可用（WebView2 运行时版本过低），内存目标级别未生效"
        ),
    }

    // 只切可见性：渲染进程保持存活，React 调度器与 MessageChannel 都不受影响。
    // 绝不在此调用 TrySuspend —— 见 set_main_webview_suspended 的文档说明。
    unsafe { controller.SetIsVisible(!suspended) }?;
    Ok(())
}

/// 窗口焦点变化时调整 WebView2 内存使用目标级别。
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

/// 上次同步给 WebView2 的「最小化」取值（即上一次挂起判定的输入）。Resized
/// 事件高频（拖拽调整尺寸逐像素触发），仅在 is_minimized 翻转时才触碰 COM。
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

    // 隐藏到托盘的窗口不归尺寸事件管：它本就挂起，别被还原事件顺手唤醒。
    // 放在去抖之前，否则虚事件会污染 LAST_MINIMIZED 的「上次同步值」语义。
    if !minimized && is_hidden_by_design() {
        return;
    }

    if LAST_MINIMIZED.swap(minimized, Ordering::Relaxed) == minimized {
        return;
    }

    // 最小化即整体挂起（停帧 + 暂停脚本定时器），还原即恢复；
    // 同时归还本进程与 WebView2 后代进程的工作集
    set_main_webview_suspended(window.app_handle(), minimized);
    if minimized {
        crate::platform::memory_trim::trim_working_sets_async();
    }
}

/// 标注「最小化挂起已同步」，供 commands::system::minimize_window 在主动
/// 挂起后调用，避免随后的 Resized 事件重复触碰 COM。非 Windows 空实现。
pub fn note_minimized_synced() {
    #[cfg(target_os = "windows")]
    LAST_MINIMIZED.store(true, Ordering::Relaxed);
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
        // 先清标记再 show：show 触发的 Resized 事件靠 is_hidden_by_design 区分
        // 「用户主动收起」与「该恢复」，标记晚清会让这次恢复被去抖吞掉
        mark_hidden_by_design(false);
        let _ = window.unminimize();
        let _ = window.show();
        // show 之后再置可见，避免 WebView 在不可见窗口里恢复时被渲染进程忽略
        set_main_webview_suspended(&window.app_handle(), false);
        // 恢复的最大化必须在 show 之后：show 前 maximize 会让窗口提前显示，
        // show 后再 set_size 又会清掉最大化状态
        crate::app::lightweight::apply_pending_maximize(&window);
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

    let builder = tauri::WebviewWindowBuilder::from_config(app, &window_config)?;
    // 同进程重建优先用内存几何；take 为空（关闭时没捕获到有效值）回退上次落盘值
    let geometry = crate::app::lightweight::take_geometry()
        .or_else(|| crate::app::lightweight::load_geometry(app));
    // 新 WebView 是全新的 controller/进程：进程级挂起标志必须归零，否则旧窗口
    // 残留的 LAST_MINIMIZED=true 会吞掉新窗口首个 Resized 恢复事件
    reset_suspend_state_for_new_webview();
    // 清掉首次启动遗留的就绪信号，否则 ready 门会立刻消费旧信号、在前端挂载前显示
    crate::app::lightweight::clear_window_ready("main");
    let window = builder.build()?;
    // 统一物理像素口径：builder 的 position/inner_size 是逻辑像素，构建后按物理值设置
    match geometry {
        Some(geo) => {
            let monitors = crate::app::lightweight::available_monitor_rects(app);
            match crate::app::lightweight::restore_geometry(&window, &geo, &monitors) {
                Ok(maximized) => {
                    // 恢复的几何即后续最大化捕获的"普通几何"基准，必须先回填
                    crate::app::lightweight::remember_geometry(geo);
                    if maximized {
                        crate::app::lightweight::request_maximize_on_show();
                    }
                }
                Err(reason) => {
                    crate::log_warn!("lightweight", "未恢复窗口几何（{}），保持配置默认", reason);
                    crate::app::lightweight::capture_geometry(&window);
                }
            }
        }
        None => crate::app::lightweight::capture_geometry(&window),
    }
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

#[cfg(test)]
mod tests {
    /// 回归护栏：`ICoreWebView2_3::TrySuspend` 冻结渲染进程会让 React 调度器
    /// 永久死锁（`pendingLanes` 有待办、`callbackNode` 永空），表现为按钮与
    /// 切页全失效、仅合成器滚动可用。任何再次引入都会让本用例失败。
    #[test]
    fn never_calls_try_suspend() {
        // 拼接构造，避免本用例自身的字面量被 include_str 匹配到
        let needle = concat!(".Try", "Suspend(");
        let src = include_str!("window.rs");
        assert!(!src.contains(needle), "window.rs 不得调用 TrySuspend");
    }
}
