use tauri::AppHandle;
use std::sync::atomic::Ordering;
use std::time::Duration;
use crate::network::check_network_quality_async;
use crate::infra::command_context::CommandContext;
use crate::infra::events::EventBus;
use super::latency::{classify_quality_change, notify_quality_change, record_last_quality, BAD_LEVELS};

/// 延迟升高的复核参数：首次触发 poor/bad 后不立即通知，再连续 2 次检测
/// （间隔 15s）均达到通知阈值才发系统通知；任一次未达标即放弃，避免瞬时
/// 抖动误报。期间前端结果推送照常进行，仅系统通知被门控。
const SPIKE_CONFIRM_COUNT: usize = 2;
const SPIKE_CONFIRM_INTERVAL_SECS: u64 = 15;

/// 单次质量检测：互斥抢占 → 执行 → 推送前端。返回 None 表示信号量被占用
/// （如手动检测进行中）或序列化失败，调用方不得当作有效档位使用。
async fn perform_quality_check(app_handle: &AppHandle, adapter_name: &str, adapter_ip: &str) -> Option<serde_json::Value> {
    let s = CommandContext::from_app(app_handle);
    let (skip_ttfb, skip_content, fixed_gateway) = {
        let cfg = s.config.load();
        (cfg.skip_ttfb_in_latency, cfg.skip_content_in_latency, cfg.fixed_gateway.clone())
    };
    let _quality_guard = match s.tasks.is_quality_checking.try_acquire() {
        Some(g) => g,
        None => return None,
    };
    let quality = check_network_quality_async(adapter_name, adapter_ip, skip_ttfb, skip_content, &fixed_gateway, s.exit.is_quitting.clone(), Some(app_handle)).await;
    let quality_val = match serde_json::to_value(&quality) {
        Ok(v) => v,
        Err(e) => {
            crate::log_warn!("background", "序列化网络质量结果失败: {}", e);
            return None;
        }
    };
    if let Err(e) = EventBus::new(app_handle).emit_network_quality_result(&quality_val) {
        crate::log_warn!("background", "发送网络质量结果失败: {}", e);
    }
    Some(quality_val)
}

/// 全量质量检测的统一入口。周期驱动者只剩定时测试循环（`spawn_latency_test_loop`），
/// 后台巡检不再触发（2026-09-04 收敛，避免双定时器叠加 + 节流吞掉自定义间隔）。
/// 手动触发的 check_network_quality 命令不经过本函数。
/// `is_quality_checking` 信号量保证与手动检测并发时互斥执行。
pub(super) async fn run_quality_check(app_handle: &AppHandle, adapter_name: &str, adapter_ip: &str) {
    let s = CommandContext::from_app(app_handle);
    let enable_notification = s.config.load().enable_notification;
    let Some(first) = perform_quality_check(app_handle, adapter_name, adapter_ip).await else {
        return;
    };
    let current = first["quality"].as_str().unwrap_or("unknown").to_string();
    let last = s.network.load().last_network_quality.clone();
    match classify_quality_change(last.as_deref(), &current, enable_notification) {
        Some("bad") => {
            // 复核确认：15s 间隔再测 2 次，全部达到阈值才通知
            let is_quitting = s.exit.is_quitting.clone();
            let mut confirmed = true;
            let mut record: Option<String> = None;
            for _ in 0..SPIKE_CONFIRM_COUNT {
                tokio::time::sleep(Duration::from_secs(SPIKE_CONFIRM_INTERVAL_SECS)).await;
                if is_quitting.load(Ordering::Acquire) {
                    return;
                }
                match perform_quality_check(app_handle, adapter_name, adapter_ip).await {
                    Some(val) => {
                        let q = val["quality"].as_str().unwrap_or("unknown").to_string();
                        let bad = BAD_LEVELS.contains(&q.as_str());
                        record = Some(q);
                        if !bad {
                            confirmed = false;
                            break;
                        }
                    }
                    None => {
                        // 复核未能执行（如与手动检测互斥冲突）：证据不足，不通知也不落状态，
                        // 下轮周期检测可重新触发复核
                        confirmed = false;
                        break;
                    }
                }
            }
            if confirmed {
                notify_quality_change(app_handle, "bad");
            }
            if let Some(q) = record {
                record_last_quality(&s, &q);
            }
        }
        kind => {
            if let Some(k) = kind {
                notify_quality_change(app_handle, k);
            }
            record_last_quality(&s, &current);
        }
    }
}
