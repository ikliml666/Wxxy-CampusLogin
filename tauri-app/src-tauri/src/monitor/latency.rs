use tauri::{AppHandle, Manager};
use std::sync::atomic::Ordering;
use std::time::Duration;
use crate::network::get_adapters_cached_async;
use crate::infra::state::AppState;
use crate::infra::notification::emit_notification;
use super::quality_scheduler::run_quality_check;

/// 触发"网络拥堵"通知的质量档位（quality_scheduler 复核同样以此为准）
pub(super) const BAD_LEVELS: &[&str] = &["poor", "bad"];
const GOOD_LEVELS: &[&str] = &["excellent", "great", "good"];

/// 与上次质量档位比较判断是否需要通知（纯判断，不落状态不发通知）：
/// Some("bad") = 恶化到 poor/bad，需经 quality_scheduler 复核确认后才通知；
/// Some("good") = 从 poor/bad 恢复。
pub(super) fn classify_quality_change(last: Option<&str>, current: &str, enable_notification: bool) -> Option<&'static str> {
    if !enable_notification {
        return None;
    }
    let last_q = last?;
    if current == last_q {
        return None;
    }
    let was_bad = BAD_LEVELS.contains(&last_q);
    let is_bad = BAD_LEVELS.contains(&current);
    let was_good = GOOD_LEVELS.contains(&last_q);
    let is_good = GOOD_LEVELS.contains(&current);

    if is_bad && !was_bad {
        Some("bad")
    } else if is_good && !was_good && was_bad {
        Some("good")
    } else {
        None
    }
}

pub(super) fn record_last_quality(state: &AppState, current: &str) {
    state.network.update(|s| s.last_network_quality = Some(current.to_string()));
}

pub(super) fn notify_quality_change(app_handle: &AppHandle, kind: &str) {
    if kind == "bad" {
        emit_notification(app_handle, "网络拥堵", "校园网延迟升高，网络可能拥堵", "mascot-busy");
        let _ = crate::infra::events::EventBus::new(app_handle).emit_login_log("校园网延迟升高，网络可能拥堵", "warning");
    } else {
        emit_notification(app_handle, "网络恢复", "校园网延迟已恢复正常", "mascot-celebrate");
        let _ = crate::infra::events::EventBus::new(app_handle).emit_login_log("校园网延迟已恢复正常", "info");
    }
}

pub fn spawn_latency_test_loop(app_handle: &AppHandle, interval: u64) -> Result<(), String> {
    let app_h = app_handle.clone();
    app_handle.state::<AppState>().task_manager.spawn("latency_test", move |cancel_token| {
        async move {
            let mut interval_timer = tokio::time::interval(Duration::from_millis(interval));
            // 单轮检测耗时超过周期时默认 Burst 会连续补发错过的 tick 造成连发，
            // 改为 Delay 保持固定周期、错过的不补发
            interval_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            let mut first_run = true;
            loop {
                if !first_run {
                    tokio::select! {
                        _ = interval_timer.tick() => {}
                        _ = cancel_token.cancelled() => break,
                    }
                }
                first_run = false;
                let s = app_h.state::<AppState>();
                if !s.task_manager.is_running("latency_test")
                    || s.exit.is_quitting.load(Ordering::Acquire) {
                    break;
                }
                // 就绪等待：适配器未解析出 IP、或 Portal 尚未认证（此时外网 HTTPS
                // 必被拦截全超时，检测结果无意义且会误报"网络拥堵"）时无法产出
                // 有效质量数据，每 2s 短重试且不消耗周期 tick。此前未就绪走 continue，
                // 会立刻耗尽 interval 的即时首 tick，之后干等完整周期——启动后首次
                // 结果要 30s+；现在条件一旦满足立即检测，首结果缩短到数秒内
                let (adapter_ip, adapter_name) = loop {
                    let config = s.config.load();
                    let adapters = get_adapters_cached_async().await.unwrap_or_default();
                    let (ip, name) = crate::network::select_adapter(&adapters, &config);
                    if !ip.is_empty() && s.network.load().any_adapter_online {
                        break (ip, name);
                    }
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(2)) => {}
                        _ = cancel_token.cancelled() => return,
                    }
                };
                // 检测前等待1秒，避免网络未稳定时HTTPS测试延迟异常
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {}
                    _ = cancel_token.cancelled() => break,
                }
                // 调用统一的 quality_scheduler 执行检测（含 semaphore 互斥、emit、通知）；
                // 传循环取消令牌，停止定时测试后正在执行的一轮提前返回
                run_quality_check(&app_h, &adapter_name, &adapter_ip, Some(&cancel_token)).await;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::classify_quality_change;

    #[test]
    fn degraded_to_bad_level_triggers_pending_bad() {
        assert_eq!(classify_quality_change(Some("good"), "bad", true), Some("bad"));
        assert_eq!(classify_quality_change(Some("excellent"), "poor", true), Some("bad"));
    }

    #[test]
    fn recovery_from_bad_level_triggers_good() {
        assert_eq!(classify_quality_change(Some("bad"), "good", true), Some("good"));
        assert_eq!(classify_quality_change(Some("poor"), "excellent", true), Some("good"));
    }

    #[test]
    fn same_level_or_unknown_last_never_notifies() {
        assert_eq!(classify_quality_change(Some("bad"), "bad", true), None);
        assert_eq!(classify_quality_change(Some("good"), "good", true), None);
        assert_eq!(classify_quality_change(None, "bad", true), None);
    }

    #[test]
    fn middle_levels_crossing_never_notifies() {
        // fair/unknown 等中间档位与任意档位互转均不通知（与历史行为一致）
        assert_eq!(classify_quality_change(Some("good"), "fair", true), None);
        assert_eq!(classify_quality_change(Some("fair"), "bad", true), Some("bad"));
        assert_eq!(classify_quality_change(Some("bad"), "fair", true), None);
        assert_eq!(classify_quality_change(Some("poor"), "poor", true), None);
    }

    #[test]
    fn notification_disabled_never_classifies() {
        assert_eq!(classify_quality_change(Some("good"), "bad", false), None);
        assert_eq!(classify_quality_change(Some("bad"), "good", false), None);
    }
}
