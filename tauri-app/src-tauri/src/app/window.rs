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

/// 期望的挂起态：`TrySuspend` 是**异步**的，其完成回调晚于调用返回，而 `Resume`
/// 是同步的。用户「最小化后立刻唤回」时 Resume 先落地、挂起后落地，页面就停在
/// 挂起态再无任何后续 Resume（v2.4.2 实测：连续多次最小化后按钮全失效、仅合成器
/// 滚动可用）。完成回调据此判定是否需要即时补偿恢复。
#[cfg(target_os = "windows")]
static SUSPEND_DESIRED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 挂起 / 恢复主 WebView2 渲染进程（最小化、隐藏到托盘、轻量化关闭共用）。
///
/// 挂起走微软官方休眠范式：先把 controller.IsVisible 置 false（TrySuspend 的硬
/// 约束，否则返回 ERROR_INVALID_STATE），再 `TrySuspend` —— 渲染进程停派帧、
/// 暂停脚本定时器，内存允许被 OS 回收；恢复用 `Resume` 再置可见。仅靠
/// `SetIsVisible(false)` 只停合成、不停定时器，CSS 无限动画仍在空转。
pub fn set_main_webview_suspended(app: &tauri::AppHandle, suspended: bool) {
    // 期望态先于 COM 调用写入：异步挂起回调在任何时序下都能读到最新意图
    #[cfg(target_os = "windows")]
    SUSPEND_DESIRED.store(suspended, Ordering::Release);
    #[cfg(target_os = "windows")]
    if let Some(ww) = app.get_webview_window("main") {
        let _ = ww.as_ref().with_webview(move |pw| {
            // [忽略错误] COM 调用在窗口销毁竞态下失败无补救意义
            let _ = apply_webview_suspended(&pw, suspended);
        });
    }
    #[cfg(not(target_os = "windows"))]
    let _ = (app, suspended);
}

/// 新 WebView 建立/销毁后复位挂起相关静态态：`LAST_MINIMIZED` 与 `SUSPEND_DESIRED`
/// 都是跨窗口生命周期的进程级标志，轻量化重建窗口后若残留旧值，会让新窗口的
/// 首个 Resized 事件被去抖吞掉（该恢复时不恢复）。
pub fn reset_suspend_state_for_new_webview() {
    #[cfg(target_os = "windows")]
    {
        SUSPEND_DESIRED.store(false, Ordering::Release);
        LAST_MINIMIZED.store(false, Ordering::Release);
    }
}

#[cfg(target_os = "windows")]
fn apply_webview_suspended(
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
        let controller_for_cb = controller.clone();
        let icw2_3_for_cb = icw2_3.clone();
        let handler = webview2_com::TrySuspendCompletedHandler::create(Box::new(
            move |hr, succeeded| {
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
                // 挂起是异步的：回调落地时窗口可能已被唤回（用户最小化后立即点击
                // 托盘/二次启动），此时期望态已翻回「不挂起」，但先前的 Resume
                // 抢先于挂起完成、对挂起态无效——必须在这里补一次恢复，否则
                // 页面永久停在挂起态（合成器滚动可用、主线程事件全失效）。
                compensate_resume_if_needed(&icw2_3_for_cb, &controller_for_cb);
                Ok(())
            },
        ));
        unsafe { icw2_3.TrySuspend(&handler) }?;
    } else {
        // Resume 失败也必须恢复可见：否则窗口可见而 WebView 冻死，且此处错误被
        // 调用方忽略（见 set_main_webview_suspended），用户只会看到白屏
        match unsafe { icw2_3.Resume() } {
            Ok(()) => crate::log_info!("window", "WebView2 已恢复渲染"),
            Err(e) => crate::log_warn!("window", "WebView2 恢复失败（继续置可见）: {}", e),
        }
        unsafe { controller.SetIsVisible(true) }?;
    }
    Ok(())
}

/// 补偿判定（纯函数，单测锁行为）：仅在「期望不挂起」却「实际仍挂起」时补
/// 一次 Resume。其余组合都交给既有路径，避免重复触碰 COM。
#[cfg(any(target_os = "windows", test))]
fn should_compensate_resume(desired_suspended: bool, actual_suspended: bool) -> bool {
    !desired_suspended && actual_suspended
}

/// 挂起完成回调里的时序补偿：期望态已翻回「不挂起」但 WebView 实际仍挂起时，
/// 补一次 Resume + 置可见（幂等：未挂起时不动作）。
#[cfg(target_os = "windows")]
fn compensate_resume_if_needed(
    icw2_3: &webview2_com_sys::Microsoft::Web::WebView2::Win32::ICoreWebView2_3,
    controller: &webview2_com_sys::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller,
) {
    let desired = SUSPEND_DESIRED.load(Ordering::Acquire);
    let mut actual = windows_core::BOOL(0);
    if unsafe { icw2_3.IsSuspended(&mut actual) }.is_err() {
        return;
    }
    if should_compensate_resume(desired, actual.as_bool()) {
        crate::log_warn!("window", "挂起回调晚于唤回落地，补一次恢复避免页面冻死");
        if let Err(e) = unsafe { icw2_3.Resume() } {
            crate::log_warn!("window", "补偿恢复失败: {}", e);
        }
        let _ = unsafe { controller.SetIsVisible(true) };
    }
}

/// 窗口焦点变化时调整 WebView2 内存使用目标级别；窗口重新获得焦点时顺带做
/// 一次挂起态自愈（不确定时序的兜底，见 `compensate_resume_if_needed`）。
#[cfg(target_os = "windows")]
pub fn handle_window_focus_event(window: &Window, event: &WindowEvent) {
    let WindowEvent::Focused(focused) = event else {
        return;
    };

    use windows_core::Interface;
    use webview2_com_sys::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_19, ICoreWebView2_3, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL,
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
    };

    let level = if *focused {
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL.0)
    } else {
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW.0)
    };

    // 只对「用户能看见的窗口」自愈：隐藏到托盘/最小化时的焦点事件不该唤醒渲染
    let visible = window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(false);
    let heal = *focused && visible;

    if let Some(ww) = window.app_handle().get_webview_window("main") {
        let _ = ww.as_ref().with_webview(move |pw| {
            let controller = pw.controller();
            if let Ok(core_webview) = unsafe { controller.CoreWebView2() } {
                if let Ok(icw2_19) = core_webview.cast::<ICoreWebView2_19>() {
                    let _ = unsafe { icw2_19.SetMemoryUsageTargetLevel(level) };
                }
                if heal {
                    if let Ok(icw2_3) = core_webview.cast::<ICoreWebView2_3>() {
                        compensate_resume_if_needed(&icw2_3, &controller);
                    }
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

    // 最小化即整体挂起（停帧 + 暂停脚本定时器），还原即恢复
    set_main_webview_suspended(window.app_handle(), minimized);
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
        // 恢复渲染放在 show 之后：窗口可见时 Resume 才不会被渲染进程判定为
        // 「不可见」而忽略（挂起态下先 Resume 再 show 的旧顺序是 v2.4.2 冻屏
        // 缺陷的帮凶）。显式 Resume 仍必要——它让解冻立刻发生，不必等首帧。
        set_main_webview_suspended(&window.app_handle(), false);
        let _ = window.set_focus();
        verify_resumed_after_show(&window.app_handle());
    }
}

/// show 之后的延迟自检：`TrySuspend` 异步完成回调可能晚于上面的 Resume，
/// 把页面留在挂起态（v2.4.2 缺陷：按钮/切页全失效、仅合成器滚动可用）。
/// 这里做两次不依赖时序的最终校验，命中即补恢复；期间用户若再次最小化，
/// `SUSPEND_DESIRED` 已翻回挂起，补偿逻辑自动不动作。
#[cfg(target_os = "windows")]
fn verify_resumed_after_show(app: &tauri::AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // 首探覆盖常规唤醒；次探覆盖 WebView2 慢启动/进程回收后重建
        for delay_ms in [250u64, 1000] {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            let Some(ww) = app.get_webview_window("main") else {
                return;
            };
            // 窗口没真正可见就不自愈：隐藏/最小化是用户意图，别把渲染抢回来
            if !ww.is_visible().unwrap_or(false) || ww.is_minimized().unwrap_or(false) {
                return;
            }
            let _ = ww.as_ref().with_webview(move |pw| {
                use windows_core::Interface;
                let controller = pw.controller();
                if let Ok(core_webview) = unsafe { controller.CoreWebView2() } {
                    if let Ok(icw2_3) =
                        core_webview.cast::<webview2_com_sys::Microsoft::Web::WebView2::Win32::ICoreWebView2_3>()
                    {
                        compensate_resume_if_needed(&icw2_3, &controller);
                    }
                }
            });
        }
    });
}

#[cfg(not(target_os = "windows"))]
fn verify_resumed_after_show(_app: &tauri::AppHandle) {}

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
    // 新 WebView 是全新的 controller/进程：进程级挂起标志必须归零，否则旧窗口
    // 残留的 LAST_MINIMIZED=true 会吞掉新窗口首个 Resized 恢复事件
    reset_suspend_state_for_new_webview();
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

#[cfg(test)]
mod tests {
    use super::should_compensate_resume;

    /// 挂起回调晚于唤回落地：期望不挂起、实际已挂起 → 必须补 Resume。
    /// 这是 v2.4.2「最小化后界面按钮全失效、仅能滚动」的根因路径。
    #[test]
    fn compensates_only_when_wake_arrives_before_suspend_completes() {
        assert!(should_compensate_resume(false, true));
    }

    /// 正常挂起（期望挂起 + 实际挂起）、正常恢复（期望不挂起 + 实际不挂起）、
    /// 挂起被拒（期望挂起 + 实际不挂起）都不该重复触碰 COM。
    #[test]
    fn skips_compensation_in_steady_states() {
        assert!(!should_compensate_resume(true, true));
        assert!(!should_compensate_resume(false, false));
        assert!(!should_compensate_resume(true, false));
    }
}
