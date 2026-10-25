use tauri::{AppHandle, Manager};
use crate::infra::command_context::CommandContext;
use crate::infra::state::AppState;
use super::auto_auth::run_auto_login_on_start;
use super::latency::{run_startup_quality_once, spawn_latency_test_loop};

// 后台检测主体（拆分自原 watcher.rs 大文件）
// 审计 A9：run_background_check 的外部调用已统一走 background_task::
// spawn_background_check_once，原 re-export 无使用者，移除
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

    // 启动即测一次（2026-10-25 需求修订）：只要质量检测总开关开启，启动后等
    // 网络就绪立即执行一次全量检测——不再以定时测试开关为前提。与定时循环
    // 首轮的立即检测并发时由 is_quality_checking 信号量互斥，后到者直接放弃。
    if config.enable_network_quality {
        let app_h = app_handle.clone();
        if let Err(e) = task_manager.spawn("startup_quality_once", move |cancel_token| async move {
            run_startup_quality_once(app_h, cancel_token).await;
        }) {
            crate::log_warn!("background", "注册 startup_quality_once 跟踪任务失败: {}", e);
        }
    }

    if config.enable_network_quality && config.enable_latency_test {
        let app_h = app_handle.clone();
        if let Err(e) = task_manager.spawn("startup_latency", |_cancel| async move {
            // 间隔由循环内每轮重读（配置 + 轻量化系数），启动时无需读取
            if let Err(e) = spawn_latency_test_loop(&app_h) {
                crate::log_warn!("background", "启动定时测试循环失败: {}", e);
            }
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

    // 每日定时登录/定时注销（P2-32）：独立轻量循环，不受 enable_background_check
    // 开关管（定时动作与巡检是两个功能）；循环内部逐拍读配置，两目标均 0=禁用时零开销
    let app_h = app_handle.clone();
    if let Err(e) = task_manager.spawn("scheduled_actions", move |cancel_token| async move {
        super::scheduled::run_scheduled_action_loop(&app_h, cancel_token).await;
    }) {
        crate::log_warn!("background", "注册 scheduled_actions 跟踪任务失败: {}", e);
    }
}
