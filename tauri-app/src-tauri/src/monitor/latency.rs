use tauri::{AppHandle, Emitter, Manager};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use crate::network::{check_network_quality_async, get_adapters_cached};
use crate::infra::state::AppState;
use crate::infra::notification::emit_notification;

pub fn notify_network_quality_change(app_handle: &AppHandle, state: &AppState, quality: &serde_json::Value, enable_notification: bool) {
    let current = quality["quality"].as_str().unwrap_or("unknown").to_string();

    let should_notify = {
        let last_arc = state.network.last_network_quality.load();
        let last = last_arc.as_ref().as_ref();
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

    state.network.last_network_quality.store(Arc::new(Some(current)));
}

pub fn spawn_latency_test_loop(app_handle: &AppHandle, interval: u64) {
    let app_h = app_handle.clone();
    let s = app_h.state::<AppState>();
    s.tasks.latency_cancel.load().cancel();
    let cancel = {
        let new_token = tokio_util::sync::CancellationToken::new();
        let cloned = new_token.clone();
        s.tasks.latency_cancel.store(Arc::new(new_token));
        cloned
    };
    let _ = s.tasks.latency_running.swap_acquire();
    tauri::async_runtime::spawn(async move {
        let mut interval_timer = tokio::time::interval(Duration::from_millis(interval));
        loop {
            tokio::select! {
                _ = interval_timer.tick() => {}
                _ = cancel.cancelled() => break,
            }
            let s = app_h.state::<AppState>();
            if !s.tasks.latency_running.is_active()
                || s.exit.is_quitting.load(Ordering::Acquire) {
                break;
            }
            let (adapter_ip, adapter_name) = {
                let config = s.config.load();
                let adapters = match get_adapters_cached() {
                    Ok(a) => a,
                    Err(_) => continue,
                };
                crate::network::select_adapter(&adapters, &config)
            };
            if adapter_ip.is_empty() {
                continue;
            }
            // 检测前等待1秒，避免网络未稳定时HTTPS测试延迟异常
            tokio::time::sleep(Duration::from_secs(1)).await;
            let (skip_ttfb, skip_content, fixed_gateway) = {
                let cfg = s.config.load();
                (cfg.skip_ttfb_in_latency, cfg.skip_content_in_latency, cfg.fixed_gateway.clone())
            };
            let _guard = match s.tasks.is_quality_checking.try_acquire() {
                Some(g) => g,
                None => continue,
            };
            let quality = check_network_quality_async(&adapter_name, &adapter_ip, skip_ttfb, skip_content, &fixed_gateway, s.exit.is_quitting.clone(), Some(&app_h)).await;
            drop(_guard);
            let quality_val = match serde_json::to_value(&quality) {
                Ok(v) => v,
                Err(e) => {
                    crate::log_warn!("latency", "序列化网络质量结果失败: {}", e);
                    continue;
                }
            };
            if let Err(e) = app_h.emit("network-quality-result", &quality_val) {
                crate::log_warn!("latency", "发送网络质量结果失败: {}", e);
            }
            let s = app_h.state::<AppState>();
            let enable_notification = s.config.load().enable_notification;
            notify_network_quality_change(&app_h, &s, &quality_val, enable_notification);
        }
    });
}
