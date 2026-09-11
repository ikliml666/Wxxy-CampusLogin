//! WebView2 进程崩溃订阅与恢复动作统一入口（2026-09-11 白屏调研落地）
//!
//! 背景：Tauri 2 Windows 侧不暴露 WebView2 `ProcessFailed` 事件（官方 PR #15162
//! 仅 Apple 平台有等价钩子），渲染/浏览器进程崩溃只能靠前端心跳超时推断
//! （最坏 20-35s 才触发第一次 reload），且 `eval reload` 对已退出/无响应进程无效。
//!
//! 本模块四件事：
//! 1. [`subscribe_process_failed`] — 经 webview2-com 直接订阅
//!    `ICoreWebView2_4::add_ProcessFailed`，按崩溃种类 + **Reason / ExitCode**
//!    记日志（诊断链：Reason 直接区分 CRASHED / OUT_OF_MEMORY / TERMINATED，
//!    是"为什么退出"的第一手证据），渲染进程退出立即走恢复入口（比心跳快 20-35s）；
//! 2. [`attempt_webview_recovery`] — reload 路径（滑动窗口限流），
//!    **仅用于 WebView 仍有效**的故障（渲染进程退出/心跳超时）；
//! 3. [`attempt_app_restart`] — 重启路径，**浏览器进程退出（WebView 已 Closed）专用**：
//!    2026-09-11 实测复现——杀掉浏览器进程后窗口整体变黑，`eval reload` 返回成功
//!    但页面不恢复（与微软文档一致：BrowserProcessExited 后 WebView 进入 Closed，
//!    必须重建 WebView 才能恢复），用户只能手动重启应用。本条路径把"手动重启"
//!    自动化为应用自重启，并带跨进程限流防"必现故障 → 无限重启"闪屏循环；
//! 4. [`record_webview2_runtime_version`] — 启动时记录运行时版本：
//!    Evergreen 随微软自动更新，白屏类故障可能仅特定版本存在
//!    （WebView2Feedback#5692：4191.47 导航后白屏），版本号是定位第一证据。

use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

/// 恢复滑动窗口：5 分钟内最多 3 次 reload。超限判定为"reload 对当前故障无效"，
/// 停止自动恢复避免每 20-35s 空转一轮（历史行为），只留 ERROR 日志作为诊断链。
const RECOVERY_WINDOW_MS: u64 = 5 * 60 * 1000;
const RECOVERY_MAX_PER_WINDOW: u32 = 3;

/// 自动重启防护：10 分钟窗口内最多 2 次。计数落盘（跨进程持久化）——重启后
/// 内存限流状态归零，若不落盘则"必现故障"会变成无限重启闪屏循环。
const RESTART_GUARD_WINDOW_MS: u64 = 10 * 60 * 1000;
const RESTART_GUARD_MAX: usize = 2;
const RESTART_GUARD_FILE: &str = "webview_restart_guard";

/// 纯函数限流判定，返回 (是否放行, 新窗口起点 epoch ms, 新计数)。
/// 窗口过期即重置计数。单测锁行为。
fn recovery_gate(window_start_ms: u64, count: u32, now_ms: u64) -> (bool, u64, u32) {
    if now_ms.saturating_sub(window_start_ms) >= RECOVERY_WINDOW_MS {
        (true, now_ms, 1)
    } else if count < RECOVERY_MAX_PER_WINDOW {
        (true, window_start_ms, count + 1)
    } else {
        (false, window_start_ms, count)
    }
}

fn epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// 恢复动作唯一入口：滑动窗口限流后 eval reload 主 WebView。
/// 心跳超时（app/heartbeat.rs）与 ProcessFailed 事件两条路径都走这里。
pub fn attempt_webview_recovery(app: &AppHandle, reason: &str) {
    use std::sync::atomic::Ordering;

    let state = app.state::<crate::infra::state::AppState>();
    let window_start = state
        .update_stats
        .webview_recovery_window_start_ms
        .load(Ordering::Acquire);
    let count = state
        .update_stats
        .webview_recovery_count
        .load(Ordering::Relaxed);
    let (allowed, new_start, new_count) = recovery_gate(window_start, count, epoch_ms());
    state
        .update_stats
        .webview_recovery_window_start_ms
        .store(new_start, Ordering::Release);
    state
        .update_stats
        .webview_recovery_count
        .store(new_count, Ordering::Relaxed);

    if !allowed {
        crate::log_error!(
            "webview_recovery",
            "WebView 恢复被限流（{}分钟内已达 {} 次），停止自动恢复，可能需要重启应用: {}",
            RECOVERY_WINDOW_MS / 60_000,
            RECOVERY_MAX_PER_WINDOW,
            reason
        );
        return;
    }
    crate::log_warn!(
        "webview_recovery",
        "尝试重载 WebView（窗口内第 {}/{} 次）: {}",
        new_count,
        RECOVERY_MAX_PER_WINDOW,
        reason
    );
    if let Some(window) = app.get_webview_window("main") {
        // eval 失败即 webview 已整体失效（浏览器进程退出场景），此处留 ERROR 证据；
        // 不自动重启应用——重启是用户决策，日志提供依据
        if let Err(e) = window.eval("window.location.reload()") {
            crate::log_error!(
                "webview_recovery",
                "reload 指令发送失败，WebView 可能已整体失效: {}",
                e
            );
        }
    } else {
        crate::log_error!(
            "webview_recovery",
            "主窗口不存在，无法执行恢复动作: {}",
            reason
        );
    }
}

/// 自动重启防护的纯函数判定：返回 (是否放行, 写回文件的时间戳列表)。
/// 窗口外的旧记录丢弃；放行时把 now_ms 追加进列表。
fn restart_guard_decide(timestamps: &[u64], now_ms: u64) -> (bool, Vec<u64>) {
    let mut kept: Vec<u64> = timestamps
        .iter()
        .copied()
        .filter(|t| now_ms.saturating_sub(*t) < RESTART_GUARD_WINDOW_MS)
        .collect();
    let allowed = kept.len() < RESTART_GUARD_MAX;
    if allowed {
        kept.push(now_ms);
    }
    (allowed, kept)
}

/// 读取重启记录文件（每行一个 epoch ms；缺失/损坏按空处理）。
fn restart_guard_read(path: &std::path::Path) -> Vec<u64> {
    std::fs::read_to_string(path)
        .map(|s| s.lines().filter_map(|l| l.trim().parse::<u64>().ok()).collect())
        .unwrap_or_default()
}

/// 跨进程限流判定 + 落盘。文件读写失败按放行处理（宁可多试一次，
/// 也不要在真黑屏时因为磁盘问题彻底放弃自愈）。
fn restart_guard_allow(app: &AppHandle, now_ms: u64) -> bool {
    let Ok(dir) = app.path().app_data_dir() else {
        return true;
    };
    let path = dir.join(RESTART_GUARD_FILE);
    let (allowed, kept) = restart_guard_decide(&restart_guard_read(&path), now_ms);
    if allowed {
        let body = kept.iter().map(|t| t.to_string()).collect::<Vec<_>>().join("\n");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(&path, body);
    }
    allowed
}

/// WebView 已整体失效（浏览器进程退出）时的恢复动作：reload 对 Closed 状态的
/// WebView 无效（实测 2026-09-11：eval 返回成功但窗口保持全黑），唯一可靠的自愈
/// 是重启应用——等效用户手动重启，但无需用户操作。跨进程限流防闪屏循环。
pub fn attempt_app_restart(app: &AppHandle, reason: &str) {
    if !restart_guard_allow(app, epoch_ms()) {
        crate::log_error!(
            "webview_recovery",
            "WebView 已整体失效，但 {} 分钟内已自动重启 {} 次，停止自动重启（疑似必现故障，请检查运行环境）: {}",
            RESTART_GUARD_WINDOW_MS / 60_000,
            RESTART_GUARD_MAX,
            reason
        );
        return;
    }
    crate::log_error!(
        "webview_recovery",
        "WebView 已整体失效，自动重启应用恢复: {}",
        reason
    );
    // 重启会直接结束当前进程，先把崩溃链日志刷盘（否则诊断证据丢失）
    crate::infra::logger::flush();
    app.restart();
}

/// 订阅 WebView2 ProcessFailed 事件（webview 创建后调用一次，贯穿应用生命周期）。
#[cfg(target_os = "windows")]
pub fn subscribe_process_failed(app: &AppHandle) {
    let Some(ww) = app.get_webview_window("main") else {
        crate::log_warn!("webview_recovery", "主窗口不存在，跳过 ProcessFailed 订阅");
        return;
    };
    let app_h = app.clone();
    if let Err(e) = ww.as_ref().with_webview(move |pw| {
        use webview2_com::ProcessFailedEventHandler;
        use webview2_com_sys::Microsoft::Web::WebView2::Win32::{
            ICoreWebView2_4, ICoreWebView2ProcessFailedEventArgs2,
            COREWEBVIEW2_PROCESS_FAILED_KIND, COREWEBVIEW2_PROCESS_FAILED_REASON,
        };
        use windows_core::Interface;

        let controller = pw.controller();
        let core = match unsafe { controller.CoreWebView2() } {
            Ok(c) => c,
            Err(_) => return,
        };
        let icw2_4 = match core.cast::<ICoreWebView2_4>() {
            Ok(i) => i,
            Err(_) => {
                crate::log_warn!(
                    "webview_recovery",
                    "ICoreWebView2_4 不可用，跳过 ProcessFailed 订阅"
                );
                return;
            }
        };

        let handler = ProcessFailedEventHandler::create(Box::new(move |_, args| {
            let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND(0);
            // Reason/ExitCode 在 v2 接口上：v1 只有 kind，拿不到"为什么退出"
            let mut reason = COREWEBVIEW2_PROCESS_FAILED_REASON(-1);
            let mut exit_code = 0i32;
            if let Some(args) = args {
                unsafe { args.ProcessFailedKind(&mut kind).ok(); }
                if let Ok(args2) = args.cast::<ICoreWebView2ProcessFailedEventArgs2>() {
                    unsafe { args2.Reason(&mut reason).ok(); }
                    unsafe { args2.ExitCode(&mut exit_code).ok(); }
                }
            }
            handle_process_failed(&app_h, kind, reason.0, exit_code);
            Ok(())
        }));

        let mut token: i64 = 0;
        if let Err(e) = unsafe { icw2_4.add_ProcessFailed(&handler, &mut token) } {
            crate::log_error!("webview_recovery", "add_ProcessFailed 订阅失败: {}", e);
        }
    }) {
        crate::log_error!(
            "webview_recovery",
            "with_webview 失败，ProcessFailed 订阅未建立: {}",
            e
        );
    }
}

/// 崩溃分级处理：
/// - **浏览器进程退出**（WebView 已进入 Closed）→ [`attempt_app_restart`]：
///   reload 对本状态无效（实测复现），重启是唯一可靠自愈；
/// - **渲染进程退出**（WebView 仍有效，页面白屏）→ [`attempt_webview_recovery`]（reload 有效）；
/// - 其余（GPU/Utility/PPAPI 等）由 WebView2 自动重启对应进程，页面一般自愈，
///   仅记日志——GPU 未自愈场景由现有 rAF 冻结 → 心跳链路兜底。
#[cfg(target_os = "windows")]
fn handle_process_failed(
    app: &AppHandle,
    kind: webview2_com_sys::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_PROCESS_FAILED_KIND,
    reason: i32,
    exit_code: i32,
) {
    use webview2_com_sys::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED as BROWSER_PROCESS_EXITED,
        COREWEBVIEW2_PROCESS_FAILED_KIND_FRAME_RENDER_PROCESS_EXITED as FRAME_RENDER_PROCESS_EXITED,
        COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED as RENDER_PROCESS_EXITED,
    };
    let description = describe_process_failed(kind, reason, exit_code);
    if kind.0 == BROWSER_PROCESS_EXITED.0 {
        attempt_app_restart(app, &description);
    } else if kind.0 == RENDER_PROCESS_EXITED.0 || kind.0 == FRAME_RENDER_PROCESS_EXITED.0 {
        attempt_webview_recovery(app, &description);
    } else {
        crate::log_warn!(
            "webview_recovery",
            "WebView2 进程事件（交由运行时自愈）: {}",
            description
        );
    }
}

/// kind + Reason + ExitCode 组成诊断串。Kind 只说"哪个进程没了"，
/// Reason 才是"为什么退出"（崩溃/内存不足/被外部终止）——缺了它无法定根因。
#[cfg(target_os = "windows")]
fn describe_process_failed(
    kind: webview2_com_sys::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_PROCESS_FAILED_KIND,
    reason: i32,
    exit_code: i32,
) -> String {
    format!(
        "{}, reason={} ({}), exitCode={}",
        describe_process_failed_kind(kind),
        reason,
        describe_process_failed_reason(reason),
        exit_code
    )
}

/// Reason 枚举 → 语义文本（-1 = 接口取不到，旧运行时 v2 接口不可用）。
#[cfg(target_os = "windows")]
fn describe_process_failed_reason(reason: i32) -> &'static str {
    use webview2_com_sys::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_PROCESS_FAILED_REASON_CRASHED as CRASHED,
        COREWEBVIEW2_PROCESS_FAILED_REASON_LAUNCH_FAILED as LAUNCH_FAILED,
        COREWEBVIEW2_PROCESS_FAILED_REASON_OUT_OF_MEMORY as OUT_OF_MEMORY,
        COREWEBVIEW2_PROCESS_FAILED_REASON_PROFILE_DELETED as PROFILE_DELETED,
        COREWEBVIEW2_PROCESS_FAILED_REASON_TERMINATED as TERMINATED,
        COREWEBVIEW2_PROCESS_FAILED_REASON_UNEXPECTED as UNEXPECTED,
        COREWEBVIEW2_PROCESS_FAILED_REASON_UNRESPONSIVE as UNRESPONSIVE,
    };
    if reason == CRASHED.0 {
        "进程崩溃"
    } else if reason == LAUNCH_FAILED.0 {
        "启动失败"
    } else if reason == OUT_OF_MEMORY.0 {
        "内存不足"
    } else if reason == PROFILE_DELETED.0 {
        "用户数据目录被删除"
    } else if reason == TERMINATED.0 {
        "进程被外部终止"
    } else if reason == UNEXPECTED.0 {
        "未预期退出"
    } else if reason == UNRESPONSIVE.0 {
        "进程无响应"
    } else {
        "原因未知"
    }
}

/// 崩溃种类 → 日志描述（诊断链的语义层）。
#[cfg(target_os = "windows")]
fn describe_process_failed_kind(
    kind: webview2_com_sys::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_PROCESS_FAILED_KIND,
) -> String {
    use webview2_com_sys::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED as BROWSER_PROCESS_EXITED,
        COREWEBVIEW2_PROCESS_FAILED_KIND_FRAME_RENDER_PROCESS_EXITED as FRAME_RENDER_PROCESS_EXITED,
        COREWEBVIEW2_PROCESS_FAILED_KIND_GPU_PROCESS_EXITED as GPU_PROCESS_EXITED,
        COREWEBVIEW2_PROCESS_FAILED_KIND_PPAPI_BROKER_PROCESS_EXITED as PPAPI_BROKER_PROCESS_EXITED,
        COREWEBVIEW2_PROCESS_FAILED_KIND_PPAPI_PLUGIN_PROCESS_EXITED as PPAPI_PLUGIN_PROCESS_EXITED,
        COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED as RENDER_PROCESS_EXITED,
        COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE as RENDER_PROCESS_UNRESPONSIVE,
        COREWEBVIEW2_PROCESS_FAILED_KIND_SANDBOX_HELPER_PROCESS_EXITED as SANDBOX_HELPER_PROCESS_EXITED,
        COREWEBVIEW2_PROCESS_FAILED_KIND_UNKNOWN_PROCESS_EXITED as UNKNOWN_PROCESS_EXITED,
        COREWEBVIEW2_PROCESS_FAILED_KIND_UTILITY_PROCESS_EXITED as UTILITY_PROCESS_EXITED,
    };
    let text = if kind.0 == BROWSER_PROCESS_EXITED.0 {
        "浏览器进程退出（WebView 已整体失效）"
    } else if kind.0 == RENDER_PROCESS_EXITED.0 {
        "渲染进程退出"
    } else if kind.0 == FRAME_RENDER_PROCESS_EXITED.0 {
        "iframe 渲染进程退出"
    } else if kind.0 == RENDER_PROCESS_UNRESPONSIVE.0 {
        "渲染进程无响应"
    } else if kind.0 == GPU_PROCESS_EXITED.0 {
        "GPU 进程退出"
    } else if kind.0 == UTILITY_PROCESS_EXITED.0 {
        "Utility 进程退出"
    } else if kind.0 == PPAPI_PLUGIN_PROCESS_EXITED.0 {
        "PPAPI 插件进程退出"
    } else if kind.0 == PPAPI_BROKER_PROCESS_EXITED.0 {
        "PPAPI Broker 进程退出"
    } else if kind.0 == SANDBOX_HELPER_PROCESS_EXITED.0 {
        "沙箱辅助进程退出"
    } else if kind.0 == UNKNOWN_PROCESS_EXITED.0 {
        "未知进程退出"
    } else {
        "未知进程失败种类"
    };
    format!("{} (kind={})", text, kind.0)
}

/// 启动时记录 WebView2 运行时版本到日志（非 Windows 为空实现）。
pub fn record_webview2_runtime_version() {
    #[cfg(target_os = "windows")]
    {
        use winreg::RegKey;

        // WebView2 Runtime 的 EdgeUpdate 安装记录 GUID（微软分发文档约定）：
        // 系统级安装在 HKLM 且 64 位进程需走 WOW6432Node 视图，用户级安装在 HKCU
        const SUB_KEY: &str =
            r"Microsoft\EdgeUpdate\Clients\{F3017226-FAC6-4E36-9A38-E4524AA30166}";
        const WOW_SUB_KEY: &str =
            r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FAC6-4E36-9A38-E4524AA30166}";

        for (hive, hive_name, path) in [
            (winreg::enums::HKEY_LOCAL_MACHINE, "HKLM", WOW_SUB_KEY),
            (winreg::enums::HKEY_LOCAL_MACHINE, "HKLM", SUB_KEY),
            (winreg::enums::HKEY_CURRENT_USER, "HKCU", SUB_KEY),
        ] {
            if let Ok(key) = RegKey::predef(hive).open_subkey(path) {
                if let Ok(pv) = key.get_value::<String, _>("pv") {
                    crate::log_info!(
                        "webview_recovery",
                        "WebView2 运行时版本: {} ({}\\{})",
                        pv,
                        hive_name,
                        path
                    );
                    return;
                }
            }
        }
        crate::log_debug!("webview_recovery", "未找到 WebView2 运行时版本注册表记录");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 恢复窗口内计数递增并放行() {
        let start = 1_000_000;
        let (allowed, s, c) = recovery_gate(0, 0, start);
        assert!(allowed);
        assert_eq!((s, c), (start, 1));

        let (allowed, s, c) = recovery_gate(s, c, start + 60_000);
        assert!(allowed);
        assert_eq!((s, c), (start, 2));

        let (allowed, s, c) = recovery_gate(s, c, start + 120_000);
        assert!(allowed);
        assert_eq!((s, c), (start, 3));
    }

    #[test]
    fn 恢复窗口内超限拒绝且状态不变() {
        let start = 1_000_000;
        let (allowed, s, c) = recovery_gate(start, 3, start + 60_000);
        assert!(!allowed);
        assert_eq!((s, c), (start, 3));
    }

    #[test]
    fn 恢复窗口过期重置计数() {
        let start = 1_000_000;
        // 窗口起点 5 分钟前，计数 3 → 放行且重置窗口
        let (allowed, s, c) = recovery_gate(start, 3, start + RECOVERY_WINDOW_MS);
        assert!(allowed);
        assert_eq!((s, c), (start + RECOVERY_WINDOW_MS, 1));
    }

    #[test]
    fn 重启防护放行至上限后拒绝且状态不变() {
        let now = 10_000_000u64;
        let (ok, kept) = restart_guard_decide(&[], now);
        assert!(ok);
        assert_eq!(kept, vec![now]);

        let (ok, kept) = restart_guard_decide(&kept, now + 1_000);
        assert!(ok);
        assert_eq!(kept.len(), RESTART_GUARD_MAX);

        let (ok, kept2) = restart_guard_decide(&kept, now + 2_000);
        assert!(!ok, "达到上限后必须拒绝，否则必现故障会无限重启");
        assert_eq!(kept2, kept);
    }

    #[test]
    fn 重启防护窗口过期后旧记录被丢弃() {
        let now = 10_000_000u64;
        let stale = vec![now - RESTART_GUARD_WINDOW_MS - 1, now - RESTART_GUARD_WINDOW_MS - 2];
        let (ok, kept) = restart_guard_decide(&stale, now);
        assert!(ok);
        assert_eq!(kept, vec![now]);
    }
}
