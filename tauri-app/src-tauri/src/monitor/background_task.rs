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

/// 夜间出站还原后「立即补检」的等待参数。
///
/// 还原刚把校园网卡从设备级禁用（PnP problem 22）里启用，Windows 还要几秒
/// 到几十秒才拿到 DHCP 地址。此刻立刻巡检必然读到「配置的适配器均无 IP」
/// 而空转，随后又要等一整个巡检周期才重试——轻量化下该周期下限 300s
/// （app/lightweight::effective_background_interval_ms），用户实测表现就是
/// 「到点不自动登录，只能手动点」。
///
/// 2026-10-22 实证：08:11:20 夜间出站还原完成（校园网卡此后才重新启用），
/// 至 08:12:17 用户手动登录之间没有任何 [background]/[auto_login]/[login]
/// 日志；轻量化下限 300s 意味着下一拍巡检最早落在 08:16。还原动作本身不带
/// 「还原后立即评估」的钩子，只能等周期巡检，这是缺口所在。
const POST_RESTORE_IP_WAIT_MAX_MS: u64 = 45_000;
const POST_RESTORE_IP_POLL_MS: u64 = 3_000;

/// 补检等待是否还应继续（纯函数，便于单测边界）。
///
/// 已等待时长严格小于上限才继续轮询；达到上限即收尾——不再多睡一轮，
/// 避免超时后仍空等一个轮询间隔才发起巡检。
fn post_restore_wait_should_continue(waited_ms: u64, max_ms: u64) -> bool {
    waited_ms < max_ms
}

/// 主/副适配器是否至少有一张已拿到 IP（还原是否已「网络就绪」）。
///
/// 用 force 版本枚举：cached 带 TTL 缓存，网卡刚启用时会返回启用前的旧快照
/// （无 IP），轮询因此永远等不到就绪。
fn post_restore_adapters_have_ip(app_handle: &AppHandle) -> bool {
    let s = app_handle.state::<AppState>();
    let config = s.config.load_full();
    let adapters = match crate::network::get_adapters_force() {
        Ok(a) if !a.is_empty() => a,
        _ => return false,
    };
    let (adapter1_name, adapter2_name) = crate::network::resolve_adapter_names(&adapters, &config);
    let (a1, a2) = crate::network::find_dual_adapters(&adapters, &config, &adapter1_name, &adapter2_name);
    a1.is_some() || a2.is_some()
}

/// 夜间出站还原成功后的补检入口：等适配器 IP 就绪（最长
/// POST_RESTORE_IP_WAIT_MAX_MS）后发起一次一次性后台巡检，让自动登录
/// 在还原后第一时间评估，而不是被巡检周期拖到几分钟后。
///
/// 阻塞枚举送阻塞线程池（get_adapters_force 是同步网络枚举）；巡检本身走
/// spawn_background_check_once（task_manager 注册、可被统一取消），
/// 与周期巡检的并发由 run_background_check 内部的 is_checking 信号量兜住。
///
/// 未读到 IP 也照发巡检：让巡检自己给出「配置的适配器均无 IP」的结论并走
/// 周期重试，比静默不发更好排障（本次故障就缺这条线索）。
pub fn spawn_background_check_after_outbound_restore(app_handle: &AppHandle) {
    let app_h = app_handle.clone();
    let probe_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let waited = tauri::async_runtime::spawn_blocking(move || {
            let mut waited_ms: u64 = 0;
            while post_restore_wait_should_continue(waited_ms, POST_RESTORE_IP_WAIT_MAX_MS)
                && !post_restore_adapters_have_ip(&probe_h)
            {
                std::thread::sleep(Duration::from_millis(POST_RESTORE_IP_POLL_MS));
                waited_ms = waited_ms.saturating_add(POST_RESTORE_IP_POLL_MS);
            }
            waited_ms
        })
        .await
        // join 失败（阻塞线程池异常）按「等满上限」处理：宁可记一条 warn 让排障
        // 有据可查，也不要谎报「IP 已就绪（等待 0ms）」掩盖探测根本没跑过。
        .unwrap_or(POST_RESTORE_IP_WAIT_MAX_MS);

        if waited >= POST_RESTORE_IP_WAIT_MAX_MS {
            crate::log_warn!(
                "background",
                "夜间出站还原后补检: 等待 {}ms 仍未读到适配器 IP，仍发起巡检（下一巡检周期会继续重试）",
                waited
            );
        } else {
            crate::log_info!(
                "background",
                "夜间出站还原后补检: 适配器 IP 已就绪（等待 {}ms），发起巡检",
                waited
            );
        }
        spawn_background_check_once(&app_h);
    });
}

#[cfg(test)]
mod tests {
    use super::post_restore_wait_should_continue;

    /// 等待上限语义：达到上限即收尾，不多睡一轮空等
    #[test]
    fn post_restore_wait_stops_at_cap() {
        assert!(post_restore_wait_should_continue(0, 45_000));
        assert!(post_restore_wait_should_continue(42_000, 45_000));
        assert!(!post_restore_wait_should_continue(45_000, 45_000));
        assert!(!post_restore_wait_should_continue(90_000, 45_000));
    }

    /// 上限为 0 表示不等待：首轮就不进循环，直接发起巡检
    #[test]
    fn post_restore_wait_zero_budget_never_waits() {
        assert!(!post_restore_wait_should_continue(0, 0));
    }
}
