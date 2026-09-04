use tauri::{AppHandle, Manager};
use std::sync::atomic::Ordering;
use std::time::Duration;
use crate::infra::state::{AppState, CommandResult};
use super::background_check::run_background_check;

pub fn start_background_check_inner(app_handle: &AppHandle, state: &AppState) -> Result<CommandResult, String> {
    let cfg = state.config.update(|cfg| {
        cfg.enable_background_check = true;
        if cfg.background_check_interval < 10000 {
            cfg.background_check_interval = 15000;
        }
    });

    let app_h = app_handle.clone();
    state.task_manager.spawn("background_check", move |cancel_token| {
        async move {
            // 首启直接执行首次检测，不再忙等 is_checking 信号量；
            // 若上次检测仍在进行，run_background_check 内部 try_acquire 会安全跳过。
            if cancel_token.is_cancelled() {
                return;
            }

            run_background_check(&app_h, cancel_token.clone()).await;

            // 间隔动态读取：每次 tick 后重读 config.background_check_interval，
            // 使设置面板修改的间隔即时生效（历史缺陷：interval 在 spawn 时捕获一次，
            // 运行中修改间隔直到重启任务才生效）
            loop {
                let interval_ms = {
                    let s = app_h.state::<AppState>();
                    let cfg = s.config.load();
                    cfg.background_check_interval.max(10000)
                };
                let mut interval_timer = tokio::time::interval(Duration::from_millis(interval_ms));
                interval_timer.tick().await;
                tokio::select! {
                    _ = interval_timer.tick() => {}
                    _ = cancel_token.cancelled() => {
                        crate::log_debug!("background", "后台检测收到取消信号，退出循环");
                        break;
                    }
                }
                let s = app_h.state::<AppState>();
                if !s.task_manager.is_running("background_check") || s.exit.is_quitting.load(Ordering::Acquire) {
                    break;
                }
                run_background_check(&app_h, cancel_token.clone()).await;
            }
        }
    })?;

    // 走 commands 层统一落盘路径：持久化后广播 config-changed，
    // 与 start/stop_latency_test 等命令保持一致的配置同步语义
    if let Err(e) = crate::commands::config_cmd::save_config_to_disk_encrypted(app_handle, &cfg) {
        crate::log_warn!("background", "保存后台检测配置失败: {}", e);
    }

    Ok(CommandResult::ok_msg("后台检测已启动"))
}
