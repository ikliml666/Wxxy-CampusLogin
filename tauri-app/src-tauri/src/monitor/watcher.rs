use tauri::{AppHandle, Manager};
use crate::infra::command_context::CommandContext;
use crate::infra::state::AppState;
use super::auto_auth::run_auto_login_on_start;
use super::latency::spawn_latency_test_loop;

// 后台检测主体（拆分自原 watcher.rs 大文件）
pub use super::background_check::run_background_check;
pub use super::background_task::start_background_check_inner;

// 已有 re-export（保持外部 watcher::X 调用路径不变）
pub use super::campus_check::check_campus_network;
pub use super::background_emit::{adapter_status_entry, adapter_disabled_entry, adapter_disconnected_entry};

pub fn run_startup_tasks(app_handle: &AppHandle) {
    let s = CommandContext::from_app(app_handle);
    let config = s.config.load_full();
    let task_manager = s.task_manager.clone();

    if config.enable_background_check {
        let app_h = app_handle.clone();
        if let Err(e) = task_manager.spawn("startup_bg_check", |_cancel| async move {
            let s = app_h.state::<AppState>();
            if let Err(e) = start_background_check_inner(&app_h, &s) {
                crate::log_warn!("background", "启动后台检测失败: {}", e);
            }
        }) {
            crate::log_warn!("background", "注册 startup_bg_check 跟踪任务失败: {}", e);
        }
    }

    if config.enable_network_quality && config.enable_latency_test {
        let app_h = app_handle.clone();
        if let Err(e) = task_manager.spawn("startup_latency", |_cancel| async move {
            let s = app_h.state::<AppState>();
            let interval = {
                let c = s.config.load();
                if c.latency_test_interval < 10000 { 30000 } else { c.latency_test_interval }
            };
            let _ = spawn_latency_test_loop(&app_h, interval);
        }) {
            crate::log_warn!("background", "注册 startup_latency 跟踪任务失败: {}", e);
        }
    }

    let app_h = app_handle.clone();
    if let Err(e) = task_manager.spawn("startup_auto_login", |_cancel| async move {
        run_auto_login_on_start(&app_h);
    }) {
        crate::log_warn!("background", "注册 startup_auto_login 跟踪任务失败: {}", e);
    }
}
