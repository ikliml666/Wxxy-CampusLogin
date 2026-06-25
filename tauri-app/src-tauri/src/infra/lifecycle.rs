use tauri::{AppHandle, Manager};
use std::sync::atomic::Ordering;
use std::time::Duration;
use crate::infra::state::{AppState, CommandResult, AUTO_EXIT_DELAY_MS, CANCEL_EXIT_SHORTCUT};
use crate::infra::events::EventBus;
use crate::infra::notification::emit_notification;

/// 校园网验证不通过时的退出延迟（毫秒）
const CAMPUS_MINIMIZE_DELAY_MS: u64 = 30000;
const CAMPUS_EXIT_DELAY_MS: u64 = 60000;

/// 校园网验证不通过时：30s后最小化到托盘，再30s后强制退出
/// 受 config.campus_exit_on_fail 控制；关闭时仅记录日志不触发退出
pub fn start_campus_exit(app_handle: &AppHandle, state: &AppState) {
    let config = state.config.load();
    if !config.campus_exit_on_fail {
        crate::log_info!("campus_exit", "校园网验证未通过，但 campus_exit_on_fail 已关闭，跳过最小化+退出流程");
        return;
    }

    let deadline = std::time::Instant::now() + Duration::from_millis(CAMPUS_EXIT_DELAY_MS);

    // 先 CAS 防止重复触发，成功后再设置 deadline
    // 注：原实现先 set_deadline 再 CAS，周期性重复调用时第二次会把 deadline 推后，
    // CAS 失败 return 后，运行中的任务最终校验发现 deadline 未到期而 return，
    // campus_exit_started 永久为 true，退出流程卡死。改为 CAS 成功后再 set_deadline，
    // 代价是 CAS 到 set_deadline 之间存在极小窗口若 cancel 介入会导致一次取消无效，
    // 但避免了确定性的永久卡死。
    if state.exit.campus_exit_started.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        return;
    }
    state.exit.set_campus_exit_deadline(Some(deadline));

    crate::log_info!("campus_exit", "校园网验证未通过，{}秒后最小化到托盘，{}秒后退出",
        CAMPUS_MINIMIZE_DELAY_MS / 1000, CAMPUS_EXIT_DELAY_MS / 1000);

    if let Err(e) = EventBus::new(app_handle).emit_campus_exit_countdown(CAMPUS_MINIMIZE_DELAY_MS, CAMPUS_EXIT_DELAY_MS) {
        crate::log_warn!("campus_exit", "发送校园网退出倒计时事件失败: {}", e);
    }

    emit_notification(app_handle, "非校园网络", &format!("{}秒后最小化，{}秒后退出，按 Ctrl+Shift+C 可取消", CAMPUS_MINIMIZE_DELAY_MS / 1000, CAMPUS_EXIT_DELAY_MS / 1000));

    // 注册统一取消快捷键（与自动退出共用 Ctrl+Shift+C）
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    if !app_handle.global_shortcut().is_registered(CANCEL_EXIT_SHORTCUT)
        && app_handle.global_shortcut().register(CANCEL_EXIT_SHORTCUT).is_err()
    {
        crate::log_warn!("campus_exit", "快捷键注册失败，请通过界面取消退出");
    }

    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        // 阶段1：等待30秒后最小化到托盘
        tokio::time::sleep(Duration::from_millis(CAMPUS_MINIMIZE_DELAY_MS)).await;

        let s = app_h.state::<AppState>();
        // 用 deadline 做二次校验：若 deadline 已被清除（取消）或已变更（重触发），则退出
        let current_deadline = s.exit.campus_exit_deadline();
        if s.exit.is_quitting.load(Ordering::Acquire) || current_deadline.is_none() {
            return;
        }

        if let Some(window) = app_h.get_webview_window("main") {
            let _ = window.hide();
            crate::log_info!("campus_exit", "已最小化到托盘");
        }

        // 阶段2：再等30秒后强制退出
        tokio::time::sleep(Duration::from_millis(CAMPUS_EXIT_DELAY_MS - CAMPUS_MINIMIZE_DELAY_MS)).await;

        let s = app_h.state::<AppState>();
        // 用 deadline 做最终校验：必须存在且已到期，且未被取消
        let current_deadline = s.exit.campus_exit_deadline();
        match current_deadline {
            Some(d) if std::time::Instant::now() >= d => {
                // deadline 匹配，继续退出
            }
            _ => {
                // deadline 已被清除（取消）或未到期（被新任务替换），退出
                return;
            }
        }

        crate::log_info!("campus_exit", "校园网验证退出流程完成，正在退出");

        // 注销快捷键（如果自动退出未在运行）
        {
            // 注意：此处存在 TOCTOU 竞态——is_some() 检查后释放锁，unregister 前另一线程可能
            // 调用 start_auto_exit 注册快捷键。窗口极小（纳秒级），且仅影响快捷键可用性，
            // 不影响退出流程正确性。如需彻底修复，需引入快捷键引用计数或统一锁。
            let auto_exit_active = s.exit.auto_exit_deadline.lock().is_some();
            if !auto_exit_active {
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                if app_h.global_shortcut().is_registered(CANCEL_EXIT_SHORTCUT) {
                    let _ = app_h.global_shortcut().unregister(CANCEL_EXIT_SHORTCUT);
                }
            }
        }

        shutdown_and_exit(&app_h, &s).await;
    });
}

/// 取消校园网退出流程（当重新检测到校园网时调用，或通过快捷键取消）
pub fn cancel_campus_exit(app_handle: &AppHandle, state: &AppState) {
    if state.exit.campus_exit_started.swap(false, Ordering::AcqRel) {
        state.exit.set_campus_exit_deadline(None);
        crate::log_info!("campus_exit", "校园网退出流程已取消");

        // 如果自动退出也未在运行，注销快捷键
        // 注意：此处存在 TOCTOU 竞态——is_some() 检查后释放锁，unregister 前另一线程可能
        // 调用 start_auto_exit 注册快捷键。窗口极小（纳秒级），且仅影响快捷键可用性，
        // 不影响退出流程正确性。如需彻底修复，需引入快捷键引用计数或统一锁。
        let auto_exit_active = state.exit.auto_exit_deadline.lock().is_some();
        if !auto_exit_active {
            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            if app_handle.global_shortcut().is_registered(CANCEL_EXIT_SHORTCUT) {
                let _ = app_handle.global_shortcut().unregister(CANCEL_EXIT_SHORTCUT);
            }
        }
    }
}

/// 通过快捷键统一取消校园网退出（含通知和快捷键注销）
pub fn cancel_campus_exit_with_notification(app_handle: &AppHandle, state: &AppState) {
    let was_active = state.exit.campus_exit_started.swap(false, Ordering::AcqRel);
    if !was_active {
        return;
    }
    state.exit.set_campus_exit_deadline(None);

    crate::log_info!("campus_exit", "校园网退出流程已取消（快捷键）");

    // 如果自动退出也未在运行，注销快捷键
    // 注意：此处存在 TOCTOU 竞态——is_some() 检查后释放锁，unregister 前另一线程可能
    // 调用 start_auto_exit 注册快捷键。窗口极小（纳秒级），且仅影响快捷键可用性，
    // 不影响退出流程正确性。如需彻底修复，需引入快捷键引用计数或统一锁。
    let auto_exit_active = state.exit.auto_exit_deadline.lock().is_some();
    if !auto_exit_active {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        if app_handle.global_shortcut().is_registered(CANCEL_EXIT_SHORTCUT) {
            let _ = app_handle.global_shortcut().unregister(CANCEL_EXIT_SHORTCUT);
        }
    }

    emit_notification(app_handle, "已取消退出", "校园网退出已取消，程序将继续运行");

    if let Err(e) = EventBus::new(app_handle).emit_campus_exit_cancelled() {
        crate::log_warn!("campus_exit", "发送取消校园网退出事件失败: {}", e);
    }
}

pub fn start_auto_exit(app_handle: &AppHandle, state: &AppState) {
    // 用户已取消自动退出时不再重新触发，直到下次 do_login 重置标志
    if state.exit.auto_exit_cancelled.load(Ordering::Acquire) {
        crate::log_info!("auto_exit", "自动退出已被用户取消，跳过重新触发");
        return;
    }
    let should_start = {
        let mut guard = state.exit.auto_exit_deadline.lock();
        if guard.is_some() {
            false
        } else {
            state.exit.auto_exit_cancelled.store(false, Ordering::Release);
            *guard = Some(std::time::Instant::now() + Duration::from_millis(AUTO_EXIT_DELAY_MS));
            true
        }
    };

    if !should_start {
        return;
    }

    if let Err(e) = EventBus::new(app_handle).emit_auto_exit_countdown(AUTO_EXIT_DELAY_MS, "Ctrl+Shift+C") {
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
        // 仅在校园网退出未启动时注销快捷键，避免影响校园网退出的取消能力
        if !s.exit.campus_exit_started.load(Ordering::Acquire) && app_h.global_shortcut().is_registered(CANCEL_EXIT_SHORTCUT) {
            let _ = app_h.global_shortcut().unregister(CANCEL_EXIT_SHORTCUT);
        }
        shutdown_and_exit(&app_h, &s).await;
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
    // 仅在校园网退出未启动时注销快捷键，避免影响校园网退出的取消能力
    if !state.exit.campus_exit_started.load(Ordering::Acquire) && app_handle.global_shortcut().is_registered(CANCEL_EXIT_SHORTCUT) {
        let _ = app_handle.global_shortcut().unregister(CANCEL_EXIT_SHORTCUT);
    }

    emit_notification(app_handle, "已取消退出", "自动退出已取消，程序将继续运行");

    if let Err(e) = EventBus::new(app_handle).emit_auto_exit_cancelled() {
        crate::log_warn!("auto_exit", "发送取消自动退出事件失败: {}", e);
    }

    Ok(CommandResult::ok_msg("自动退出已取消"))
}

/// 统一的后台任务清理与进程退出。
///
/// 设置退出标志、通过 BackgroundTaskManager 取消并等待所有后台任务结束，然后退出进程。
pub async fn shutdown_and_exit(app_handle: &AppHandle, state: &AppState) {
    state.exit.is_quitting.store(true, Ordering::Release);
    state.task_manager.shutdown().await;
    crate::log_info!("lifecycle", "后台任务已清理，执行退出");
    app_handle.exit(0);
}
