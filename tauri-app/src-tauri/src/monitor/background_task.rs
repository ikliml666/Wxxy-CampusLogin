use tauri::{AppHandle, Manager};
use std::sync::atomic::Ordering;
use std::time::Duration;
use crate::infra::state::{AppState, CommandResult};
use super::background_check::run_background_check;

pub fn start_background_check_inner(app_handle: &AppHandle, state: &AppState) -> Result<CommandResult, String> {
    let (interval, cfg) = {
        let cfg = state.config.update(|cfg| {
            cfg.enable_background_check = true;
            if cfg.background_check_interval < 10000 {
                cfg.background_check_interval = 15000;
            }
        });
        let interval = cfg.background_check_interval;
        (interval, cfg)
    };

    let app_h = app_handle.clone();
    state.task_manager.spawn("background_check", move |cancel_token| {
        async move {
            // 首启直接执行首次检测，不再忙等 is_checking 信号量；
            // 若上次检测仍在进行，run_background_check 内部 try_acquire 会安全跳过。
            if cancel_token.is_cancelled() {
                return;
            }

            run_background_check(&app_h, cancel_token.clone()).await;

            let mut interval_timer = tokio::time::interval(Duration::from_millis(interval));
            interval_timer.tick().await;
            loop {
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

    let data_dir = crate::config::persist::get_data_dir(app_handle);
    if let Err(e) = crate::config::persist::save_config_to_disk_encrypted(&data_dir, &cfg) {
        crate::log_warn!("background", "保存后台检测配置失败: {}", e);
    }

    Ok(CommandResult::ok_msg("后台检测已启动"))
}
