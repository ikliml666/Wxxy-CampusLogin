use tauri::AppHandle;
use crate::network::check_network_quality_async;
use crate::infra::command_context::CommandContext;
use crate::infra::events::EventBus;
use super::latency::notify_network_quality_change;

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
    let quality = check_network_quality_async(adapter_name, adapter_ip, skip_ttfb, skip_content, &fixed_gateway, s.exit.is_quitting.clone(), Some(app_handle)).await;
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
