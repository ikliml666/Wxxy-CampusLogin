use tauri::{AppHandle, Manager};
use std::sync::atomic::Ordering;
use std::time::Duration;
use crate::network::get_adapters_cached_async;
use crate::infra::state::AppState;
use crate::infra::notification::emit_notification;
use super::quality_scheduler::run_quality_check;

pub fn notify_network_quality_change(app_handle: &AppHandle, state: &AppState, quality: &serde_json::Value, enable_notification: bool) {
    let current = quality["quality"].as_str().unwrap_or("unknown").to_string();

    let should_notify = {
        let last_arc = state.network.load().last_network_quality.clone();
        let last = last_arc.as_ref();
        if !enable_notification {
            None
        } else if let Some(last_q) = last {
            if current != *last_q {
                let bad_levels: &[&str] = &["poor", "bad"];
                let good_levels: &[&str] = &["excellent", "great", "good"];
                let was_bad = bad_levels.contains(&last_q.as_str());
                let is_bad = bad_levels.contains(&current.as_str());
                let was_good = good_levels.contains(&last_q.as_str());
                let is_good = good_levels.contains(&current.as_str());

                if is_bad && !was_bad {
                    Some("bad")
                } else if is_good && !was_good && was_bad {
                    Some("good")
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    };

    if let Some(kind) = should_notify {
        if kind == "bad" {
            emit_notification(app_handle, "网络拥堵", "校园网延迟升高，网络可能拥堵");
        } else {
            emit_notification(app_handle, "网络恢复", "校园网延迟已恢复正常");
        }
    }

    state.network.update(|s| s.last_network_quality = Some(current.clone()));
}

pub fn spawn_latency_test_loop(app_handle: &AppHandle, interval: u64) -> Result<(), String> {
    let app_h = app_handle.clone();
    app_handle.state::<AppState>().task_manager.spawn("latency_test", move |cancel_token| {
        async move {
            let mut interval_timer = tokio::time::interval(Duration::from_millis(interval));
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
                let (adapter_ip, adapter_name) = {
                    let config = s.config.load();
                    let adapters = match get_adapters_cached_async().await {
                        Ok(a) => a,
                        Err(_) => continue,
                    };
                    crate::network::select_adapter(&adapters, &config)
                };
                if adapter_ip.is_empty() {
                    continue;
                }
                // 检测前等待1秒，避免网络未稳定时HTTPS测试延迟异常
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {}
                    _ = cancel_token.cancelled() => break,
                }
                // 调用统一的 quality_scheduler 执行检测（含 semaphore 互斥、emit、通知）
                run_quality_check(&app_h, &adapter_name, &adapter_ip).await;
            }
        }
    })
}
