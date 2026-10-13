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
                    // 轻量化期间下限 300s（app/lightweight），非轻量化即配置值
                    crate::app::lightweight::effective_background_interval_ms(
                        cfg.background_check_interval.max(10000),
                        crate::app::lightweight::is_lightweight_active(),
                    )
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

    // 走 config::persist 统一落盘+广播路径：持久化后广播 config-changed，
    // 与 start/stop_latency_test 等命令保持一致的配置同步语义
    if let Err(e) = crate::config::persist::save_config_and_broadcast(app_handle, &cfg) {
        crate::log_warn!("background", "保存后台检测配置失败: {}", e);
    }

    Ok(CommandResult::ok_msg("后台检测已启动"))
}

/// 审计 A9：一次性后台检测的统一发起入口——检测任务经 task_manager 注册运行
/// （"background_check_once"），取消令牌全程可达，退出/关机时可被统一取消。
/// 旧实现在三处调用点直接 `cancel_token("background_check").unwrap_or_else(新令牌)`
/// 再裸 spawn：周期任务未启动时拿到的是从未注册的孤儿令牌，任务不可取消。
///
/// 注册策略：task_manager.spawn 失败（同名一次性检测已在跑）时，复用其令牌
/// 再裸 spawn 一轮——is_checking 信号量由 run_background_check 内部 try_acquire
/// 兜底，并发轮次安全跳过。
pub fn spawn_background_check_once(app_handle: &AppHandle) {
    let app_h = app_handle.clone();
    let spawned = app_handle.state::<AppState>().task_manager.spawn(
        "background_check_once",
        move |cancel_token| async move {
            run_background_check(&app_h, cancel_token.clone()).await;
        },
    );
    if let Err(e) = spawned {
        // 已有一次性检测在跑：复用其注册令牌补跑一轮（注册态可达，可被统一取消）
        let s = app_handle.state::<AppState>();
        if let Some(token) = s.task_manager.cancel_token("background_check_once") {
            let app_h2 = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                super::background_check::run_background_check(&app_h2, token).await;
            });
        } else {
            crate::log_warn!("background", "一次性后台检测注册失败且令牌缺失: {}", e);
        }
    }
}
