//! 每日定时登录 / 定时注销（P2-32）+ 夜间出站切换编排（Task 5）。
//!
//! 独立轻量循环（30s 一拍，分钟粒度判定），判定复用跨平台纯函数
//! `config::schedule::should_fire_scheduled_action`（桌面与安卓同语义），
//! 动作复用既有入口 `auth::service::full_login` / `full_logout`——不复制任何协议逻辑。
//!
//! 夜间出站切换（改目标网卡跃点）在同一循环内编排，顺序硬约束（spec §2）：
//! 同拍**先做**出站动作（切换/还原），随后以切换态（快照非空）门控运营商夜切——
//! 切入切换态后运营商夜切让位（从热点出站登录校园 portal 必然失败，切了白切），
//! 出站还原未完成（重试中）时运营商夜切恢复同样等待；出站没切上（无候选）则运营商
//! 夜切照常。定时登录在切换态下跳过，定时注销不受影响。
//!
//! 与既有机制的隔离：
//! - 与后台检测解耦：随 run_startup_tasks 无条件启动，`enable_background_check`
//!   关闭时定时动作仍生效（两者是独立功能）；
//! - 互斥：登录走 `tasks.is_logging_in`、注销走 `tasks.is_logging_out`（与手动登录/
//!   托盘快速登录/掉线重连同一把锁，抢不到即本拍跳过，下一拍「过点补触发」兜底）；
//! - 标记独立：当日触发标记存 `state.scheduled`（ScheduledFired），不与 auto_auth 的
//!   冷却时间/重连计数/has_logged_online 共享，互不干扰；
//! - 单次语义（2026-10-24 修订）：到点置当日标记，但登录失败时按拍间隔有限
//!   补试（SCHEDULED_LOGIN_MAX_RETRIES 次）后才放弃当日额度——原"失败也当日
//!   不重试"会让登录时段刚开的瞬时拒绝直接吞掉当日登录；注销仍是单次语义。

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use chrono::Datelike;
use tauri::{AppHandle, Manager};
use crate::config::night_switch::{parse_chkstatus, uid_matches, NightSwitchAction};
use crate::config::outbound_switch::{outbound_restore_active, NightOutboundAction};
use crate::config::scheduled_tick::{
    evaluate_night_switch_gated_with_restore, evaluate_outbound_action_with_restore, evaluate_scheduled_actions,
};
use crate::infra::events::EventBus;
use crate::infra::state::AppState;
use super::outbound_night::{
    apply_outbound_restore, apply_outbound_switch, drive_pending_switch, needs_replay,
    outbound_switch_active, reconcile_outbound_on_startup, replay_outbound_switch,
    run_outbound_blocking, watchdog_re_disable_campus, OUTBOUND_SWITCH_FAIL_COUNT,
};

/// 判定拍间隔：分钟粒度判定，30s 保证最多半分钟偏差；错过（休眠/拍间隔）由
/// 「过点补触发」语义兜底，不会整天静默失效
const SCHEDULED_TICK_MS: u64 = 30_000;

/// 夜切验证时序参数（与安卓端同构）：登录编排完成后等 15s 再查（eportal 在线表
/// 非即时，立即查会读到切换前状态）；查询失败重试间隔 5s、最多 3 次，单次请求 15s 超时
const NIGHT_VERIFY_WAIT_SECS: u64 = 15;
const NIGHT_VERIFY_RETRY_SECS: u64 = 5;
const NIGHT_VERIFY_ATTEMPTS: u32 = 3;
const NIGHT_VERIFY_HTTP_TIMEOUT_SECS: u64 = 15;

/// 循环体：由 watcher::run_startup_tasks 经 task_manager.spawn 拉起并跟踪
pub async fn run_scheduled_action_loop(app_handle: &AppHandle, cancel_token: std::sync::Arc<tokio_util::sync::CancellationToken>) {
    let app_h = app_handle.clone();
    // 启动对账（循环启动前一次性）：上次运行残留的切换态先归位——恢复窗口内立即还原；
    // 夜间窗口内目标卡仍可用则重放切换。阻塞动作送阻塞线程池执行：
    // check_gateway_reachable_from 与 helper 结果轮询都要求非 async worker 线程
    // （见 network/subnet.rs 的调用约束）
    let app_h_reconcile = app_h.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || reconcile_outbound_on_startup(&app_h_reconcile)).await;
    // 定时注销启动对账：冷启动不补触发注销（见函数注释）
    reconcile_scheduled_logout_on_startup(&app_h);
    let mut tick = tokio::time::interval(Duration::from_millis(SCHEDULED_TICK_MS));
    loop {
        tokio::select! {
            _ = tick.tick() => {}
            _ = cancel_token.cancelled() => {
                crate::log_debug!("scheduled", "定时动作循环收到取消信号，退出");
                break;
            }
        }
        // AppState 守卫不跨 await 持有：本轮有多个 await 点（见 is_quitting 注释）。
        // 已知偏差（本轮不治理、不改行为）：apply_night_switch_action 内含同步落盘与托盘
        // 刷新，仍直接跑在 async worker 线程上（既有实现）；本任务新增的出站动作一律经
        // run_outbound_blocking 进阻塞线程池
        if is_quitting(&app_h) {
            break;
        }
        let now = chrono::Local::now();
        let now_minutes = crate::infra::local_now_minutes();
        let today_day = now.date_naive().num_days_from_ce();
        let config_snapshot: Arc<crate::config::Config> = {
            let s = app_h.state::<AppState>();
            s.config.load_full()
        };
        // 出站切换态（快照非空）下定时登录跳过——且**不消耗当日标记**（共享编排
        // 函数 evaluate_scheduled_actions 内部直接返回 false，调用方不置标记）：
        // 否则当日登录额度被静默吞掉，而"过点补触发"语义要求还原成功后的下一拍仍能补登
        let outbound_active_at_tick = outbound_switch_active(&app_h);
        let (fire_login, fire_logout) = {
            let s = app_h.state::<AppState>();
            evaluate_scheduled(&s, now_minutes, today_day, outbound_active_at_tick)
        };
        if fire_login {
            run_scheduled_login(&app_h, now_minutes, today_day);
        }
        if fire_logout {
            run_scheduled_logout(&app_h, now_minutes, today_day);
        }

        // ===== 夜间出站切换（spec §2：出站动作先行，与运营商夜切互斥）=====
        // 出站动作拍不靠 continue 跳过后续，互斥由下方 outbound_active 门控达成——
        // 两条要求因此同时成立：① 切换态成立（快照非空，含 helper 部分失败的重试期）
        // 则运营商夜切让位；② 出站没切上（无可用候选）则运营商夜切照常（spec §2），
        // 与安卓端 run_scheduled_actions 同构（同样以状态标记门控，而非提前返回）
        // 出站动作判定走共享编排函数（双端单点）：切换态快照（拍前）、关闭兜底、
        // 「立即切换」hold 冻结都在函数内
        let restore_active = outbound_restore_active(
            &config_snapshot.outbound_metric_restore,
            &config_snapshot.outbound_disabled_adapters,
            &config_snapshot.outbound_standby_route,
        );
        match evaluate_outbound_action_with_restore(
            config_snapshot.enable_night_outbound_switch,
            now.weekday().num_days_from_sunday(),
            now_minutes as u32,
            restore_active,
            config_snapshot.outbound_manual_hold_day,
            today_day,
            // 恢复时刻可配置（默认 07:30）：原始值直传，归一化在判定函数内部完成
            config_snapshot.night_restore_minutes,
        ) {
            NightOutboundAction::Switch => {
                let switched = run_outbound_blocking(app_h.clone(), config_snapshot.clone(), apply_outbound_switch)
                    .await
                    .is_ok();
                if switched {
                    crate::log_debug!("outbound", "夜间出站切换已完成，本拍跳过运营商夜切判定");
                }
            }
            NightOutboundAction::Restore => {
                let restored = run_outbound_blocking(app_h.clone(), config_snapshot.clone(), apply_outbound_restore)
                    .await
                    .is_ok();
                if !restored {
                    crate::log_debug!("outbound", "夜间出站还原未完成(退避重试中)，本拍跳过运营商夜切恢复");
                }
            }
            NightOutboundAction::None => {
                // 切换态下的"未生效补齐"：helper 部分失败时快照已保留（切换态成立），
                // Switch 分支不会再触发（restore_active 下判定恒为 None）→ 由这里按退避重试。
                // 终止条件看 needs_replay：切换成功后失败计数清零，稳态（夜间约 900 拍）
                // 不再每拍提权重写跃点/刷日志——只有确实留有失败历史才补
                let switch_fails = OUTBOUND_SWITCH_FAIL_COUNT.load(Ordering::Acquire);
                if needs_replay(
                    &config_snapshot.outbound_metric_restore,
                    &config_snapshot.outbound_disabled_adapters,
                    &config_snapshot.outbound_standby_route,
                    switch_fails,
                ) {
                    let _ = run_outbound_blocking(app_h.clone(), config_snapshot.clone(), |h, c| {
                        replay_outbound_switch(h, c);
                        Ok(())
                    })
                    .await;
                }
                // 待切换残留清理：旧版本登录闸写入的 waiting-login 行（2026-10-26 起
                // 本版本不再产生挂起）。残留会让前端徽标常驻，故每拍清一次；清空后
                // 该字段恒为空，本分支不再进入。切换态成立时同理已随切换成功清空
                if !config_snapshot.outbound_pending_switch.is_empty() && !restore_active {
                    let _ = run_outbound_blocking(app_h.clone(), config_snapshot.clone(), drive_pending_switch).await;
                }
                // 夜间看门狗：guard 窗口 ∧ 切换态下复核禁用名单，名单内校园网卡被
                // 外部重新启用（用户手动启用/网络重置/驱动重装）时重新禁用（P2-3）。
                // 提权结果轮询最长 30s/卡，与其他出站动作一样下放阻塞线程池
                let _ =
                    run_outbound_blocking(app_h.clone(), config_snapshot.clone(), watchdog_re_disable_campus)
                        .await;
            }
        }
        // 切换态（快照非空）：运营商夜切让位——切侧从热点出站登录校园 portal 必然失败；
        // 恢复侧快照非空即"还原未成功（重试中）"，同样不让运营商夜切恢复先跑
        let outbound_active = outbound_switch_active(&app_h);
        // 晚间断网切换：判定独立于当日标记（切换态由配置自身承载，纯函数幂等），
        // 与定时动作同循环、同"独立于静默期闸门"的取位（见
        // .codewiki/decisions/scheduled-actions-outside-silent-window）
        let night_action = {
            let s = app_h.state::<AppState>();
            // 出站动作可能刚改写切换态，判定用重读的新鲜配置；门控用**拍后**
            // 切换态（出站动作执行后重取，见 :149 注释）
            let config = s.config.load_full();
            evaluate_night_switch_gated_with_restore(
                config.enable_night_operator_switch,
                now.weekday().num_days_from_sunday(),
                now_minutes as u32,
                &config.operator,
                &config.night_operator_restore,
                outbound_active,
                // 原始值直传，归一化在判定函数内部完成
                config.night_restore_minutes,
            )
        };
        if night_action != NightSwitchAction::None {
            apply_night_switch_action(&app_h, night_action);
        }
    }
}

/// App 是否正在退出。抽成独立函数只为让循环体不跨 await 持有 State 守卫
/// （循环各拍有多个 await 点，而该 future 必须 Send）
fn is_quitting(app_handle: &AppHandle) -> bool {
    app_handle.state::<AppState>().exit.is_quitting.load(Ordering::Acquire)
}

/// 启动对账（定时注销防冷启动误触发）：应用启动时若当日注销时刻已过，「过点补触发」
/// 语义会让首拍立即注销——用户刚打开应用就被踢下线，是意外而非预期。对账只消耗
/// 当日额度（置 logout_day）不执行动作；运行中的休眠补触发/锁争用补触发不受影响
/// （本函数只在启动路径调用一次，不进 tick 循环）。
/// 定时登录不消耗：启动补登录 = 补上线，正是用户预期。注销禁用（哨兵 1440）时
/// now_minutes 恒小于目标，条件天然不成立。
/// 已知边界（不治理）：启动后当天再把注销时刻改成另一个已过时刻，下一拍仍会立即
/// 触发——与「改完就想立刻注销」无法区分意图，留待用户反馈。
fn reconcile_scheduled_logout_on_startup(app_handle: &AppHandle) {
    let s = app_handle.state::<AppState>();
    let config = s.config.load_full();
    let now = chrono::Local::now();
    let now_minutes = crate::infra::local_now_minutes();
    let today_day = now.date_naive().num_days_from_ce();
    if now_minutes >= config.scheduled_logout_minutes {
        s.scheduled.logout_day.store(today_day, Ordering::Release);
        crate::log_info!(
            "scheduled",
            "启动对账: 当日定时注销时刻已过({:02}:{:02})，消耗今日额度不执行",
            config.scheduled_logout_minutes / 60,
            config.scheduled_logout_minutes % 60
        );
    }
}

/// 单拍判定：命中返回 (应登录, 应注销)，不置任何标记。审计 A7 后额度标记
/// 移到动作抢锁成功之后（见 [`mark_scheduled_day_fired`]）——被互斥锁跳过的
/// 拍不消耗当日重试机会；跨天后 today_day 变化，旧标记不再相等，视为未执行
/// （纯函数语义）。判定本体是共享 crate 的 [`evaluate_scheduled_actions`]
/// （双端单点，含出站切换态互斥），本函数只负责从 AppState 装配输入。
/// 门控用**拍前**切换态：定时判定先于本拍出站动作（安卓在出站动作之后判定，
/// 传执行后值——两端时点差异见 config::scheduled_tick 模块文档）。
fn evaluate_scheduled(state: &AppState, now_minutes: u16, today_day: i32, outbound_active: bool) -> (bool, bool) {
    let config = state.config.load_full();
    evaluate_scheduled_actions(
        now_minutes,
        today_day,
        config.scheduled_login_minutes,
        config.scheduled_logout_minutes,
        state.scheduled.login_day.load(Ordering::Acquire),
        state.scheduled.logout_day.load(Ordering::Acquire),
        outbound_active,
    )
}

/// 审计 A7：当日额度标记，在动作抢锁成功后置位（无论后续登录/注销成败，
/// 当日不重试；切换态下 fire_login=false 不置标记，出站还原成功后的下一拍
/// 仍走"过点补触发"补上今日登录）。生产路径在 spawn_login_action/perform_logout
/// 的闭包内置位；本函数供单测模拟同一置位语义。
#[cfg_attr(not(test), allow(dead_code))]
fn mark_scheduled_day_fired(state: &AppState, fire_login: bool, fire_logout: bool, today_day: i32) {
    if fire_login {
        state.scheduled.login_day.store(today_day, Ordering::Release);
    }
    if fire_logout {
        state.scheduled.logout_day.store(today_day, Ordering::Release);
    }
}

fn format_minutes(minutes: u16) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

/// 定时登录：复用 full_login（与手动/托盘登录同一互斥锁），成功后走
/// post_login_handler（托盘快速登录同款编排，含解除保护期/延迟检测/自动退出）。
/// today_day 用于抢锁成功后消耗当日登录额度（审计 A7）。
fn run_scheduled_login(app_handle: &AppHandle, target_minutes: u16, today_day: i32) {
    spawn_login_action(
        app_handle,
        &format!("定时登录触发 (目标 {})", format_minutes(target_minutes)),
        "定时登录",
        Some(today_day),
    );
}

/// 登录动作共用执行体（定时登录与晚间断网切换登录同款）：抢 `is_logging_in`
/// 互斥锁，抢不到本拍跳过并对外发 scheduled-action-skipped 事件（审计 A7）；
/// login_quota_day 传入 Some(当日) 时，抢锁成功后消耗定时登录当日额度
/// （夜切触发路径传 None，不消耗额度）；日志/通知复用 emit_login_log（"success"/"error"）
fn spawn_login_action(app_handle: &AppHandle, trigger_log: &str, result_prefix: &str, login_quota_day: Option<i32>) {
    crate::log_info!("scheduled", "{trigger_log}");
    let app_h = app_handle.clone();
    let result_prefix = result_prefix.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let s = app_h.state::<AppState>();
        let _guard = match s.tasks.is_logging_in.try_acquire() {
            Some(g) => g,
            // 已有登录任务在跑：不重复发起。审计 A7：对外发跳过事件，
            // 且本拍不消耗当日登录额度（额度标记移到抢锁成功之后）
            None => {
                crate::log_warn!("scheduled", "登录动作跳过：已有登录任务进行中");
                let _ = EventBus::new(&app_h).emit_scheduled_action_skipped("login");
                return;
            }
        };
        // 审计 A7：当日额度标记在抢锁成功之后置位——被互斥跳过的拍不白耗额度。
        // 失败补试：登录失败且当日补试（SCHEDULED_LOGIN_MAX_RETRIES）未用尽时
        // 回退额度标记（login_day 复位），下一拍"过点补触发"再试；成功或补试用尽
        // 才保留当日标记。修复：原"到点即置标记（无论成败），当日不重试"——
        // 登录时段刚开的服务端瞬时拒绝会让当日直接放弃，到可登录时段也无人再试。
        if let Some(day) = login_quota_day {
            s.scheduled.login_day.store(day, Ordering::Release);
        }
        let result = crate::auth::service::full_login(&s, &app_h, None);
        let message = result.message.clone().unwrap_or_default();
        let _ = EventBus::new(&app_h).emit_login_log(
            &format!("{result_prefix}: {message}"),
            if result.success { "success" } else { "error" },
        );
        if result.success {
            if login_quota_day.is_some() {
                s.scheduled.reset_login_retry();
            }
            crate::auth::service::post_login_handler(&app_h, &s);
        } else if let Some(day) = login_quota_day {
            let retries = s.scheduled.bump_login_retry(day);
            if retries >= SCHEDULED_LOGIN_MAX_RETRIES {
                // 补试用尽：保留当日标记（其上方已置位），当日停止重试
                s.scheduled.reset_login_retry();
                crate::log_warn!(
                    "scheduled",
                    "定时登录失败且补试用尽({}次)，当日停止重试",
                    SCHEDULED_LOGIN_MAX_RETRIES
                );
            } else {
                // 回退当日额度标记，下一拍过点补触发再试
                s.scheduled.login_day.store(i32::MIN, Ordering::Release);
                crate::log_info!(
                    "scheduled",
                    "定时登录失败，当日补试 [{}/{}]，下一拍重试",
                    retries, SCHEDULED_LOGIN_MAX_RETRIES
                );
            }
        }
    });
}

/// 定时登录当日失败补试上限：到点登录失败（如登录时段刚开服务端仍拒绝）后，
/// 按 30s 拍间隔有限补试，而非一次失败即当日放弃。20 次 × 30s ≈ 10 分钟窗口，
/// 覆盖服务端延迟放行几分钟的常见情况（评审：原 3 次 90s 窗口太短）。
const SCHEDULED_LOGIN_MAX_RETRIES: u32 = 20;

/// 晚间断网切换动作：改内存配置 → 落盘（复用 save_config_to_disk_encrypted，
/// 含 config-changed 事件与托盘刷新）→ 触发登录（复用定时登录的登录路径）。
/// 切换态由配置自身承载，配置改完下一拍判定即回 None，无需去重标记。
fn apply_night_switch_action(app_handle: &AppHandle, action: NightSwitchAction) {
    let s = app_handle.state::<AppState>();
    let mut config = (*s.config.load_full()).clone();
    let trigger_log = match action {
        NightSwitchAction::None => return,
        NightSwitchAction::SwitchToCampus => {
            let original = std::mem::take(&mut config.operator);
            config.night_operator_restore = original;
            "晚间断网切换: 运营商已切至无锡学院".to_string()
        }
        NightSwitchAction::Restore => {
            config.operator = std::mem::take(&mut config.night_operator_restore);
            "晚间断网切换: 已恢复原运营商".to_string()
        }
    };
    if let Err(e) = crate::config::persist::save_config_and_broadcast(app_handle, &config) {
        // 落盘失败不 store 内存（避免"内存已切换、重启后回退"的错位），下拍重试
        crate::log_warn!("scheduled", "晚间断网切换配置落盘失败: {e}");
        return;
    }
    s.config.store(config);
    crate::log_info!("scheduled", "{trigger_log}");
    spawn_login_action(app_handle, &format!("{trigger_log}, 触发登录"), "晚间断网切换", None);
    // 独立 tokio 任务验证切换是否生效，不阻塞监控循环；期望值取落盘后的 config.operator
    // （SwitchToCampus 后为空串=无锡学院，Restore 后为恢复的后缀）
    let expected_operator = app_handle.state::<AppState>().config.load_full().operator.clone();
    tauri::async_runtime::spawn(verify_night_switch(app_handle.clone(), expected_operator));
}

/// 定时注销：复用 full_logout（与手动注销同一互斥锁）。成功后对齐全量注销后处理
/// （commands::login_cmd::do_logout 的 adapter_name=None 分支）：取消登录后自动退出、
/// 重置登录/重连状态并启动 60s 注销保护期——防止定时注销后被准备自动登录/
/// 掉线重连立即登回（安卓端 do_logout 内置同款保护期语义）。
/// today_day 用于抢锁成功后消耗当日注销额度（审计 A7）。
fn run_scheduled_logout(app_handle: &AppHandle, target_minutes: u16, today_day: i32) {
    crate::log_info!("scheduled", "定时注销触发 (目标 {})", format_minutes(target_minutes));
    let app_h = app_handle.clone();
    tauri::async_runtime::spawn_blocking(move || perform_logout(&app_h, "定时注销", Some(today_day)));
}

/// 注销执行体（定时注销与夜切验证复登前的注销同款）：抢 `is_logging_out` 互斥锁，
/// 抢不到本拍跳过并对外发 scheduled-action-skipped 事件（审计 A7）；
/// logout_quota_day 传入 Some(当日) 时，抢锁成功后消耗定时注销当日额度
/// （夜切验证路径传 None）；成功后取消登录后自动退出、重置登录/重连状态并
/// 启动 60s 注销保护期。
fn perform_logout(app_handle: &AppHandle, result_prefix: &str, logout_quota_day: Option<i32>) {
    let s = app_handle.state::<AppState>();
    let result = {
        let _guard = match s.tasks.is_logging_out.try_acquire() {
            Some(g) => g,
            None => {
                crate::log_warn!("scheduled", "{result_prefix}跳过：已有注销任务进行中");
                let _ = EventBus::new(app_handle).emit_scheduled_action_skipped("logout");
                return;
            }
        };
        // 审计 A7：当日额度标记在抢锁成功之后置位——被互斥跳过的拍不白耗额度
        if let Some(day) = logout_quota_day {
            s.scheduled.logout_day.store(day, Ordering::Release);
        }
        crate::auth::service::full_logout(&s, app_handle, None)
    };
    let message = result.message.clone().unwrap_or_default();
    let _ = EventBus::new(app_handle).emit_login_log(
        &format!("{result_prefix}: {message}"),
        if result.success { "success" } else { "error" },
    );
    if result.success {
        s.exit.auto_exit_cancelled.store(true, Ordering::Release);
        s.exit.set_deadline(None);
        crate::auth::failure_tracker::reset_all(&s);
        let protected_until = std::time::Instant::now() + Duration::from_secs(60);
        s.network.update(|n| {
            n.has_logged_online = false;
            n.disconnect_reconnect_count = 0;
            n.last_auto_login_attempt = std::time::Instant::now();
            n.logout_protected_until = protected_until;
        });
    }
}

/// 夜切后的登录生效自动验证（与安卓端 verify_night_switch 同构，独立 tokio 任务
/// 不阻塞监控循环）：等 15s → 查 eportal chkstatus(JSONP) 核对在线账号 uid 与期望
/// 运营商后缀（重试 3 次、间隔 5s）→ 仍不符则注销重登一轮再复验 → 最终不符发告警
/// 日志 + 系统通知，生效写 success 日志。uid 核对复用共享纯函数 config::night_switch。
async fn verify_night_switch(app_handle: AppHandle, expected_operator: String) {
    tokio::time::sleep(Duration::from_secs(NIGHT_VERIFY_WAIT_SECS)).await;
    let Some(config) = night_verify_config(&app_handle, &expected_operator) else {
        return;
    };
    if let Ok(uid) = night_switch_verify_round(&config, &expected_operator).await {
        let _ = EventBus::new(&app_handle)
            .emit_login_log(&format!("夜切验证: 已生效, 在线账号 {uid}"), "success");
        return;
    }
    // 未生效：注销 → 再登录 → 复验一轮。复登前复查 operator，用户已手动改走则止步，
    // 避免把用户刚改的配置又注销掉
    let _ = EventBus::new(&app_handle)
        .emit_login_log("夜切验证: 未生效, 注销重登后复验", "warning");
    let Some(config) = night_verify_config(&app_handle, &expected_operator) else {
        return;
    };
    let app_h = app_handle.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || perform_logout(&app_h, "夜切验证注销", None)).await;
    spawn_login_action(&app_handle, "夜切验证复登", "夜切验证", None);
    tokio::time::sleep(Duration::from_secs(NIGHT_VERIFY_WAIT_SECS)).await;
    match night_switch_verify_round(&config, &expected_operator).await {
        Ok(uid) => {
            let _ = EventBus::new(&app_handle)
                .emit_login_log(&format!("夜切验证: 已生效, 在线账号 {uid}"), "success");
        }
        Err(e) => {
            let _ = EventBus::new(&app_handle)
                .emit_login_log(&format!("夜切验证: {e}, 请手动检查"), "warning");
            crate::infra::notification::emit_notification(
                &app_handle,
                "夜切验证失败",
                "夜切后在线账号与期望运营商不符, 请手动检查",
                "mascot-alert",
            );
        }
    }
}

/// 读取验证所需的配置快照；operator 已不等于期望值（用户手动改走其他运营商）时
/// 返回 None 跳过验证——验证前提不成立，继续核对只会误报。
fn night_verify_config(app_handle: &AppHandle, expected_operator: &str) -> Option<crate::config::model::Config> {
    let s = app_handle.state::<AppState>();
    let config = (*s.config.load_full()).clone();
    (config.operator == expected_operator).then_some(config)
}

/// 一轮验证（NIGHT_VERIFY_ATTEMPTS 次重试、间隔 5s）：Ok(uid)=已生效，
/// Err=最后一轮失败原因。HTTP/解析失败不 panic，收敛为 Err。
async fn night_switch_verify_round(config: &crate::config::model::Config, expected_operator: &str) -> Result<String, String> {
    // 复用桌面既有 reqwest 客户端池构造（超时 15s 作池 key，与 protocol.rs 同款）
    let client = crate::network::client::create_safe_http_client(
        Duration::from_secs(NIGHT_VERIFY_HTTP_TIMEOUT_SECS),
        None,
    )
    .map_err(|e| format!("chkstatus 客户端创建失败: {e}"))?;
    let mut last_err = String::new();
    for attempt in 0..NIGHT_VERIFY_ATTEMPTS {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_secs(NIGHT_VERIFY_RETRY_SECS)).await;
        }
        match night_switch_check_once(&client, &config.portal_url, &config.user, expected_operator).await {
            Ok(uid) => return Ok(uid),
            Err(e) => last_err = e,
        }
    }
    Err(last_err)
}

/// 单次 chkstatus 查询 + uid 核对：Ok=在线账号(uid)，Err=失败原因（不符时含观测 uid）
async fn night_switch_check_once(
    client: &reqwest::Client,
    portal_url: &str,
    user: &str,
    expected_operator: &str,
) -> Result<String, String> {
    let url = format!("{}/drcom/chkstatus?callback=dr1003", portal_origin(portal_url));
    let body = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("chkstatus 请求失败: {e}"))?
        .text()
        .await
        .map_err(|e| format!("chkstatus 响应读取失败: {e}"))?;
    match parse_chkstatus(&body) {
        Some(info) if info.online && uid_matches(&info.uid, user, expected_operator) => Ok(info.uid),
        Some(info) => Err(format!(
            "在线账号 {} 与期望 {} 不符",
            info.uid,
            expected_operator_display(expected_operator)
        )),
        None => Err("chkstatus 响应解析失败".to_string()),
    }
}

/// portal_url 的 origin（chkstatus 与 portal 同源，eportal 按请求源 IP 判定本机）；
/// 空值回退默认 portal 地址（config::model 的既有默认值）
pub(crate) fn portal_origin(portal_url: &str) -> String {
    let url = if portal_url.is_empty() {
        crate::config::model::default_portal_url()
    } else {
        portal_url.to_string()
    };
    match url.split_once("://") {
        Some((scheme, rest)) => format!("{scheme}://{}", rest.split('/').next().unwrap_or("")),
        None => url.split('/').next().unwrap_or("").to_string(),
    }
}

/// 期望运营商的展示名：空串=无锡学院，其余原样（@telecom 等）
fn expected_operator_display(expected_operator: &str) -> &str {
    if expected_operator.is_empty() {
        "无锡学院"
    } else {
        expected_operator
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering as AOrd;

    const TODAY: i32 = 739_776;

    #[test]
    fn 禁用目标_不触发_不置标记() {
        let state = AppState::new();
        let (login, logout) = evaluate_scheduled(&state, 600, TODAY, false);
        assert!(!login);
        assert!(!logout);
        assert_eq!(state.scheduled.login_day.load(AOrd::Acquire), i32::MIN);
        assert_eq!(state.scheduled.logout_day.load(AOrd::Acquire), i32::MIN);
    }

    #[test]
    fn 到点触发_置当日标记() {
        let state = AppState::new();
        state.config.update(|c| {
            c.scheduled_login_minutes = 7 * 60 + 40;
            c.scheduled_logout_minutes = 23 * 60 + 30;
        });
        // 未到点
        assert!(!evaluate_scheduled(&state, 460 - 1, TODAY, false).0);
        // 过点补触发（循环拍错过精确分钟）→ 抢锁成功后置当日标记（审计 A7）
        assert!(evaluate_scheduled(&state, 505, TODAY, false).0);
        mark_scheduled_day_fired(&state, true, false, TODAY);
        assert_eq!(state.scheduled.login_day.load(AOrd::Acquire), TODAY);
        // 同日不重复
        assert!(!evaluate_scheduled(&state, 600, TODAY, false).0);
        // 注销同款
        assert!(!evaluate_scheduled(&state, 23 * 60 + 29, TODAY, false).1);
        assert!(evaluate_scheduled(&state, 23 * 60 + 30, TODAY, false).1);
        mark_scheduled_day_fired(&state, false, true, TODAY);
        assert_eq!(state.scheduled.logout_day.load(AOrd::Acquire), TODAY);
    }

    #[test]
    fn 跨天重置_次日再触发() {
        let state = AppState::new();
        state.config.update(|c| {
            c.scheduled_login_minutes = 7 * 60 + 40;
        });
        state.scheduled.login_day.store(TODAY - 1, AOrd::Release);
        assert!(evaluate_scheduled(&state, 460, TODAY, false).0);
    }

    #[test]
    fn 切换态_跳过定时登录且不消耗当日标记_注销照常() {
        let state = AppState::new();
        state.config.update(|c| {
            c.scheduled_login_minutes = 7 * 60 + 40;
            c.scheduled_logout_minutes = 23 * 60 + 30;
        });
        // 切换态下过点（07:40 已过、23:30 未到）：登录被跳过，且 login_day 未被置位
        let (login, logout) = evaluate_scheduled(&state, 505, TODAY, true);
        assert!(!login, "切换态下定时登录应跳过");
        assert!(!logout, "未到注销时刻");
        assert_eq!(
            state.scheduled.login_day.load(AOrd::Acquire),
            i32::MIN,
            "跳过的拍不得消耗当日登录标记（否则还原后无法过点补触发）"
        );
        // 注销到点：切换态下照常触发并置位
        let (login, logout) = evaluate_scheduled(&state, 23 * 60 + 30, TODAY, true);
        assert!(!login, "切换态下定时登录仍应跳过");
        assert!(logout, "定时注销在切换态下照常执行");
        mark_scheduled_day_fired(&state, false, true, TODAY);
        assert_eq!(state.scheduled.logout_day.load(AOrd::Acquire), TODAY);
        // 还原成功后的下一拍（outbound_active=false）：过点补触发补上今日登录
        let (login_after, _) = evaluate_scheduled(&state, 23 * 60 + 31, TODAY, false);
        assert!(login_after, "还原成功后应补触发当日定时登录");
        mark_scheduled_day_fired(&state, true, false, TODAY);
        assert_eq!(state.scheduled.login_day.load(AOrd::Acquire), TODAY);
    }

    #[test]
    fn format_minutes_补零() {
        assert_eq!(format_minutes(460), "07:40");
        assert_eq!(format_minutes(1439), "23:59");
    }

    /// 定时登录失败补试：计数累加、手动清零、跨天自动归零
    #[test]
    fn 定时登录补试计数_累加清零跨天归零() {
        let state = AppState::new();
        assert_eq!(state.scheduled.bump_login_retry(TODAY), 1);
        assert_eq!(state.scheduled.bump_login_retry(TODAY), 2);
        for _ in 3..SCHEDULED_LOGIN_MAX_RETRIES {
            state.scheduled.bump_login_retry(TODAY);
        }
        assert_eq!(state.scheduled.bump_login_retry(TODAY), SCHEDULED_LOGIN_MAX_RETRIES);
        // 成功/放弃路径清零后重新计数
        state.scheduled.reset_login_retry();
        assert_eq!(state.scheduled.bump_login_retry(TODAY), 1);
        // 跨天自动归零：新的一天补试额度重新开始
        assert_eq!(state.scheduled.bump_login_retry(TODAY + 1), 1);
    }

    /// 补试语义端到端（纯函数层模拟）：失败时 login_day 被回退 → 过点补触发
    /// 仍可 fire；补试用尽（>= SCHEDULED_LOGIN_MAX_RETRIES）后当日不再触发
    #[test]
    fn 定时登录失败回退额度_补试用尽后当日停止() {
        let state = AppState::new();
        state.config.update(|c| {
            c.scheduled_login_minutes = 7 * 60 + 40;
        });
        // 第一次失败：补试 1/3，回退 login_day → 下一拍仍可 fire
        state.scheduled.login_day.store(TODAY, AOrd::Release);
        state.scheduled.login_day.store(i32::MIN, AOrd::Release);
        assert!(evaluate_scheduled(&state, 505, TODAY, false).0, "额度回退后过点补触发必须放行");
        // 三次补试用尽：login_day 保留 TODAY → 当日不再触发
        for _ in 1..SCHEDULED_LOGIN_MAX_RETRIES {
            state.scheduled.bump_login_retry(TODAY);
        }
        assert_eq!(state.scheduled.bump_login_retry(TODAY), SCHEDULED_LOGIN_MAX_RETRIES);
        state.scheduled.login_day.store(TODAY, AOrd::Release);
        assert!(!evaluate_scheduled(&state, 600, TODAY, false).0, "补试用尽后当日不得再触发");
    }
}
