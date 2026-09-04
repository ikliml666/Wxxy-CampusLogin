use tauri::AppHandle;
use crate::network::check_network_quality_async;
use crate::infra::command_context::CommandContext;
use crate::infra::events::EventBus;
use super::latency::notify_network_quality_change;

/// 全量质量检测的统一入口。周期驱动者只剩定时测试循环（`spawn_latency_test_loop`），
/// 后台巡检不再触发（2026-09-04 收敛，避免双定时器叠加 + 节流吞掉自定义间隔）。
/// 手动触发的 check_network_quality 命令不经过本函数。
/// `is_quality_checking` 信号量保证与手动检测并发时互斥执行。
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
    let quality = check_network_quality_async(adapter_name, adapter_ip, skip_ttfb, skip_content, &fixed_gateway, s.exit.is_quitting.clone(), Some(app_handle)).await;
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
