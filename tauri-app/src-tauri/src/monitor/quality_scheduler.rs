use tauri::AppHandle;
use std::sync::atomic::{AtomicU64, Ordering};
use crate::network::check_network_quality_async;
use crate::infra::command_context::CommandContext;
use crate::infra::events::EventBus;
use super::latency::notify_network_quality_change;

/// 全局质量检测最小间隔（毫秒）。
/// BE-A-01/BE-A-05：后台巡检（默认 15s）与延迟测试循环（最小 30s）两个定时器叠加，
/// 都会经 run_quality_check 触发全量外网检测（12 HTTPS + DNS + DoH + 网关，开销大）。
/// 以"上次全量检测完成时间戳"做全局节流，两个定时器共用，间隔不足 60s 的调用直接跳过。
/// 手动触发的 check_network_quality 命令不经过 run_quality_check，不受本节流限制。
const QUALITY_CHECK_MIN_INTERVAL_MS: u64 = 60_000;

/// 上次全量质量检测完成时间戳（UNIX_EPOCH 毫秒，0 表示从未执行）。
/// is_quality_checking 信号量保证检测执行互斥，读-判定-写之间不存在并发执行窗口。
static LAST_QUALITY_CHECK_DONE_MS: AtomicU64 = AtomicU64::new(0);

fn now_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// 执行网络质量检测并 emit 结果
pub(super) async fn run_quality_check(app_handle: &AppHandle, adapter_name: &str, adapter_ip: &str) {
    let s = CommandContext::from_app(app_handle);
    let (skip_ttfb, skip_content, fixed_gateway) = {
        let cfg = s.config.load();
        (cfg.skip_ttfb_in_latency, cfg.skip_content_in_latency, cfg.fixed_gateway.clone())
    };
    let _quality_guard = match s.tasks.is_quality_checking.try_acquire() {
        Some(g) => g,
        None => return,
    };
    // 全局节流检查放在信号量之后：拿到信号量才判定，保证时间基准与互斥执行对齐
    let now = now_epoch_ms();
    let last_done = LAST_QUALITY_CHECK_DONE_MS.load(Ordering::Acquire);
    if last_done != 0 && now.saturating_sub(last_done) < QUALITY_CHECK_MIN_INTERVAL_MS {
        // 距上次完成用 saturating_sub 计算，避免系统时钟回拨导致 now < last_done 时减法下溢
        crate::log_debug!("background", "质量检测节流：距上次完成{}ms（最小间隔{}ms），跳过本次触发",
            now.saturating_sub(last_done), QUALITY_CHECK_MIN_INTERVAL_MS);
        return;
    }
    let quality = check_network_quality_async(adapter_name, adapter_ip, skip_ttfb, skip_content, &fixed_gateway, s.exit.is_quitting.clone(), Some(app_handle)).await;
    LAST_QUALITY_CHECK_DONE_MS.store(now_epoch_ms(), Ordering::Release);
    drop(_quality_guard);
    let enable_notification = s.config.load().enable_notification;
    let quality_val = match serde_json::to_value(&quality) {
        Ok(v) => v,
        Err(e) => {
            crate::log_warn!("background", "序列化网络质量结果失败: {}", e);
            return;
        }
    };
    if let Err(e) = EventBus::new(app_handle).emit_network_quality_result(&quality_val) {
        crate::log_warn!("background", "发送网络质量结果失败: {}", e);
    }
    notify_network_quality_change(app_handle, &s, &quality_val, enable_notification);
}
