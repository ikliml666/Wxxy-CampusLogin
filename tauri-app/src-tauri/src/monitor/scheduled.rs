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
//! - 单次语义：到点即置当日标记（无论动作成败），当日不重试——避免凭据错误或
//!   非校园网环境下每拍重发请求刷日志/通知；失败由登录日志暴露。

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use chrono::{Datelike, Timelike};
use tauri::{AppHandle, Manager};
use crate::config::night_switch::{
    evaluate_night_switch, parse_chkstatus, uid_matches, NightSwitchAction,
    RESTORE_END_MINUTES, RESTORE_START_MINUTES,
};
use crate::config::outbound_switch::{
    evaluate_night_outbound, outbound_restore_active, NightOutboundAction,
};
use crate::config::schedule::should_fire_scheduled_action;
use crate::infra::events::EventBus;
use crate::infra::state::AppState;
use crate::monitor::outbound_switch::{
    disabled_adapters_json, is_campus_adapter, parse_disabled_adapters, parse_standby_route,
    select_campus_to_disable, select_outbound_candidate, snapshot_json, standby_route_json,
    StandbyRoute,
};
use crate::network::Adapter;
use crate::platform::metric::MetricRow;

/// 判定拍间隔：分钟粒度判定，30s 保证最多半分钟偏差；错过（休眠/拍间隔）由
/// 「过点补触发」语义兜底，不会整天静默失效
const SCHEDULED_TICK_MS: u64 = 30_000;

/// 夜切验证时序参数（与安卓端同构）：登录编排完成后等 15s 再查（eportal 在线表
/// 非即时，立即查会读到切换前状态）；查询失败重试间隔 5s、最多 3 次，单次请求 15s 超时
const NIGHT_VERIFY_WAIT_SECS: u64 = 15;
const NIGHT_VERIFY_RETRY_SECS: u64 = 5;
const NIGHT_VERIFY_ATTEMPTS: u32 = 3;
const NIGHT_VERIFY_HTTP_TIMEOUT_SECS: u64 = 15;

/// 夜间出站切换：提权 helper 执行超时（改跃点/禁网卡/加路由都是即时操作，超时即判失败走退避）
const OUTBOUND_HELPER_TIMEOUT_SECS: u64 = 30;

/// 兜底默认路由的跃点：必须排在既有默认路由之后（TUN 接管时为 0、物理卡自动跃点
/// 通常 25+），仅当更高优先级的默认路由全部消失（TUN 退出、热点断开）时接管出站。
/// 前缀同为 /0 时按最长前缀优先无优势，纯靠跃点做 failover，故取一个"比 TUN 大、
/// 比一切物理卡自动跃点小"的值
const OUTBOUND_STANDBY_ROUTE_METRIC: u32 = 2;

/// 夜间出站切换退避阶梯：首败 60s 起步、每失败翻倍、封顶 300s
/// （1→60s、2→120s、3→240s、≥4→300s）——避免 30s 循环整夜重试刷日志/弹通知
const OUTBOUND_BACKOFF_START_MS: u64 = 60_000;
const OUTBOUND_BACKOFF_MAX_MS: u64 = 300_000;

/// 出站还原连续失败达此次数 → 发一次告警通知（含手动恢复提示）；此后每
/// OUTBOUND_RESTORE_ALERT_REMIND_EVERY 次失败重复提醒一次——避免整夜刷通知，
/// 也避免「只提醒一次、之后无限静默重试」（k2.8 审计 P3-1）
const OUTBOUND_RESTORE_ALERT_FAILS: u32 = 3;
/// 首次告警后的重复提醒间隔（失败次数）：300s 退避封顶时约每 100 分钟一次
const OUTBOUND_RESTORE_ALERT_REMIND_EVERY: u32 = 20;

/// 还原失败告警是否应发出：首次达阈值，此后每 REMIND_EVERY 次重复一次
fn outbound_restore_alert_due(count: u32) -> bool {
    count >= OUTBOUND_RESTORE_ALERT_FAILS
        && (count - OUTBOUND_RESTORE_ALERT_FAILS) % OUTBOUND_RESTORE_ALERT_REMIND_EVERY == 0
}

/// 出站**切换**动作退避状态：上次失败时刻（epoch ms）+ 连续失败计数
static OUTBOUND_SWITCH_LAST_FAIL_MS: AtomicU64 = AtomicU64::new(0);
static OUTBOUND_SWITCH_FAIL_COUNT: AtomicU32 = AtomicU32::new(0);
/// 出站**还原**动作退避状态（独立记账：两者失败原因不同，退避不互相拖慢）
static OUTBOUND_RESTORE_LAST_FAIL_MS: AtomicU64 = AtomicU64::new(0);
static OUTBOUND_RESTORE_FAIL_COUNT: AtomicU32 = AtomicU32::new(0);
/// 提权通道失败计数：仅"没拿到 helper 结果"（UAC 拒绝/超时/结果文件缺失）类失败自增。
/// 计数非零才允许出站动作降级弹 UAC——业务性失败（跃点行不存在等）弹 UAC 解决不了；
/// 任一次 helper 成功即清零（通道可用）
static OUTBOUND_CHANNEL_FAIL_COUNT: AtomicU32 = AtomicU32::new(0);

/// 循环体：由 watcher::run_startup_tasks 经 task_manager.spawn 拉起并跟踪
pub async fn run_scheduled_action_loop(app_handle: &AppHandle, cancel_token: std::sync::Arc<tokio_util::sync::CancellationToken>) {
    let app_h = app_handle.clone();
    // 启动对账（循环启动前一次性）：上次运行残留的切换态先归位——恢复窗口内立即还原；
    // 夜间窗口内目标卡仍可用则重放切换。阻塞动作送阻塞线程池执行：
    // check_gateway_reachable_from 与 helper 结果轮询都要求非 async worker 线程
    // （见 network/subnet.rs 的调用约束）
    let app_h_reconcile = app_h.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || reconcile_outbound_on_startup(&app_h_reconcile)).await;
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
        let now_minutes = now.hour() as u16 * 60 + now.minute() as u16;
        let today_day = now.date_naive().num_days_from_ce();
        let config_snapshot: Arc<crate::config::Config> = {
            let s = app_h.state::<AppState>();
            s.config.load_full()
        };
        // 出站切换态（快照非空）下定时登录跳过——且**不消耗当日标记**（gated_night_action
        // 内部直接返回 false，调用方不置标记）：否则当日登录额度被静默吞掉，
        // 而"过点补触发"语义要求还原成功后的下一拍仍能补登
        let outbound_active_at_tick = outbound_switch_active(&app_h);
        let (fire_login, fire_logout) = {
            let s = app_h.state::<AppState>();
            evaluate_and_mark(&s, now_minutes, today_day, outbound_active_at_tick)
        };
        if fire_login {
            run_scheduled_login(&app_h, now_minutes);
        }
        if fire_logout {
            run_scheduled_logout(&app_h, now_minutes);
        }

        // ===== 夜间出站切换（spec §2：出站动作先行，与运营商夜切互斥）=====
        // 出站动作拍不靠 continue 跳过后续，互斥由下方 outbound_active 门控达成——
        // 两条要求因此同时成立：① 切换态成立（快照非空，含 helper 部分失败的重试期）
        // 则运营商夜切让位；② 出站没切上（无可用候选）则运营商夜切照常（spec §2），
        // 与安卓端 run_scheduled_actions 同构（同样以状态标记门控，而非提前返回）
        match outbound_action_for(&config_snapshot, now.weekday().num_days_from_sunday(), now_minutes as u32) {
            NightOutboundAction::Switch => {
                let switched = run_outbound_blocking(app_h.clone(), config_snapshot.clone(), apply_outbound_switch).await;
                if switched {
                    crate::log_debug!("outbound", "夜间出站切换已完成，本拍跳过运营商夜切判定");
                }
            }
            NightOutboundAction::Restore => {
                let restored = run_outbound_blocking(app_h.clone(), config_snapshot.clone(), apply_outbound_restore).await;
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
                        false
                    })
                    .await;
                }
            }
        }
        // 切换态（快照非空）：运营商夜切让位——切侧从热点出站登录校园 portal 必然失败；
        // 恢复侧快照非空即"还原未成功（重试中）"，同样不让运营商夜切恢复先跑
        let outbound_active = outbound_switch_active(&app_h);
        // 晚间断网切换：判定独立于当日标记（切换态由配置自身承载，纯函数幂等），
        // 与定时动作同循环、同"独立于静默期闸门"的取位（见
        // .codewiki/decisions/scheduled-actions-outside-silent-window）
        let night_action = if outbound_active {
            NightSwitchAction::None
        } else {
            let s = app_h.state::<AppState>();
            let config = s.config.load_full();
            evaluate_night_switch(
                config.enable_night_operator_switch,
                now.weekday().num_days_from_sunday(),
                now_minutes as u32,
                &config.operator,
                &config.night_operator_restore,
            )
        };
        if night_action != NightSwitchAction::None {
            apply_night_switch_action(&app_h, night_action);
        }
    }
}

/// 把出站动作送进阻塞线程池执行（`check_gateway_reachable_from` 的线程约束 +
/// helper 结果轮询是阻塞等待，不能在 async worker 线程上跑）；join 失败按动作失败处理。
async fn run_outbound_blocking(
    app_h: AppHandle,
    config: Arc<crate::config::Config>,
    action: fn(&AppHandle, &crate::config::Config) -> bool,
) -> bool {
    match tauri::async_runtime::spawn_blocking(move || action(&app_h, &config)).await {
        Ok(result) => result,
        Err(e) => {
            crate::log_warn!("outbound", "出站动作执行线程异常: {e}");
            false
        }
    }
}

/// App 是否正在退出。抽成独立函数只为让循环体不跨 await 持有 State 守卫
/// （循环各拍有多个 await 点，而该 future 必须 Send）
fn is_quitting(app_handle: &AppHandle) -> bool {
    app_handle.state::<AppState>().exit.is_quitting.load(Ordering::Acquire)
}

/// 是否处于出站切换态（三份快照任一非空即切换态；与判定纯函数的
/// outbound_restore_active 同语义）：metric 快照/禁用名单/兜底路由
fn outbound_switch_active(app_handle: &AppHandle) -> bool {
    let config = app_handle.state::<AppState>().config.load_full();
    outbound_restore_active(
        &config.outbound_metric_restore,
        &config.outbound_disabled_adapters,
        &config.outbound_standby_route,
    )
}

/// 本拍出站动作：判定复用跨平台纯函数，外加一条编排策略——功能已关闭但快照残留
/// （用户中途关掉开关、或导入了带切换态的旧配置）时按 Restore 处理，立即还原，
/// 不留悬挂的跃点改动（否则跃点会一直停在被改过的值上，直到下次应用启动对账）
fn outbound_action_for(config: &crate::config::Config, weekday: u32, now_minutes: u32) -> NightOutboundAction {
    let restore_active = outbound_restore_active(
        &config.outbound_metric_restore,
        &config.outbound_disabled_adapters,
        &config.outbound_standby_route,
    );
    match evaluate_night_outbound(config.enable_night_outbound_switch, weekday, now_minutes, restore_active) {
        NightOutboundAction::None if restore_active && !config.enable_night_outbound_switch => {
            NightOutboundAction::Restore
        }
        other => other,
    }
}

/// 恢复窗口 [06:30, 23:00)：窗内是"清晨还原期"，窗外是"夜间切换期"
/// （时间表常量由 config::night_switch 承载，双端同源）
fn in_outbound_restore_window(now_minutes: u32) -> bool {
    (RESTORE_START_MINUTES..RESTORE_END_MINUTES).contains(&now_minutes)
}

fn now_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 退避窗（纯函数）：无失败历史（0 次）为 0；否则 60s 起步、每失败翻倍、封顶 300s
fn outbound_backoff_ms(fail_count: u32) -> u64 {
    if fail_count == 0 {
        return 0;
    }
    OUTBOUND_BACKOFF_START_MS
        .saturating_mul(1u64 << (fail_count - 1).min(3))
        .min(OUTBOUND_BACKOFF_MAX_MS)
}

/// 退避闸（纯函数）：true=允许本拍发起动作。距上次失败不足当前退避窗则压制。
/// 无失败历史恒放行；时钟回拨（now < last）按压制处理（安全侧：宁可晚一拍再试）
fn backoff_gate(last_fail_ms: u64, fail_count: u32, now_ms: u64) -> bool {
    if fail_count == 0 || last_fail_ms == 0 {
        return true;
    }
    now_ms.saturating_sub(last_fail_ms) >= outbound_backoff_ms(fail_count)
}

fn switch_gate_open() -> bool {
    backoff_gate(
        OUTBOUND_SWITCH_LAST_FAIL_MS.load(Ordering::Acquire),
        OUTBOUND_SWITCH_FAIL_COUNT.load(Ordering::Acquire),
        now_epoch_ms(),
    )
}

fn restore_gate_open() -> bool {
    backoff_gate(
        OUTBOUND_RESTORE_LAST_FAIL_MS.load(Ordering::Acquire),
        OUTBOUND_RESTORE_FAIL_COUNT.load(Ordering::Acquire),
        now_epoch_ms(),
    )
}

/// 记一次失败：失败时刻置当前、计数自增，返回累计连续失败次数
fn mark_outbound_failure(last_fail_ms: &AtomicU64, fail_count: &AtomicU32) -> u32 {
    last_fail_ms.store(now_epoch_ms(), Ordering::Release);
    fail_count.fetch_add(1, Ordering::AcqRel) + 1
}

/// 清零退避状态（动作成功后调用，退避阶梯从头计）
fn clear_outbound_failure(last_fail_ms: &AtomicU64, fail_count: &AtomicU32) {
    fail_count.store(0, Ordering::Release);
    last_fail_ms.store(0, Ordering::Release);
}

/// 是否允许降级弹 UAC：仅提权通道失败过才放行（首试保持静默，零打扰）
fn outbound_allow_uac() -> bool {
    OUTBOUND_CHANNEL_FAIL_COUNT.load(Ordering::Acquire) > 0
}

/// 提权通道失败才抬高 UAC 放行计数；业务性失败不抬（弹 UAC 无意义且打扰用户）
fn note_channel_failure(failure: &HelperFailure) {
    if failure.channel {
        OUTBOUND_CHANNEL_FAIL_COUNT.fetch_add(1, Ordering::AcqRel);
    }
}

/// 提权通道可用（helper 成功）：UAC 放行计数归零，下次仍先走静默通道
fn clear_outbound_channel_failure() {
    OUTBOUND_CHANNEL_FAIL_COUNT.store(0, Ordering::Release);
}

/// 是否需要走"未生效补齐"重放（纯函数）：处于切换态（三份快照任一非空）**且**切换侧
/// 留有失败历史时才需要。切换成功后失败计数已清零、快照仍在 → 稳态下恒为 false，
/// 避免夜间窗口约 900 拍重复提权写跃点/禁网卡/加路由、刷日志
fn needs_replay(metric_snapshot: &str, disabled_adapters: &str, standby_route: &str, fail_count: u32) -> bool {
    outbound_restore_active(metric_snapshot, disabled_adapters, standby_route) && fail_count > 0
}

/// 切换态快照条目（对应 `monitor::outbound_switch::snapshot_json` 的序列化结构
/// `[{guid, family, automatic, metric}]`）
#[derive(Debug, Clone, serde::Deserialize)]
struct OutboundSnapshotRow {
    guid: String,
    family: u16,
    automatic: bool,
    metric: u32,
}

/// 解析切换态快照：空串 → 空表（= 非切换态）；JSON 非法 → Err（调用方按数据损坏处理）
fn parse_outbound_snapshot(snapshot: &str) -> Result<Vec<OutboundSnapshotRow>, String> {
    if snapshot.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(snapshot).map_err(|e| e.to_string())
}

/// 出站动作 helper 调用的失败分类
struct HelperFailure {
    /// 失败原因（helper message 或提权通道错误）
    reason: String,
    /// 是否属提权/执行通道失败（没拿到 helper 结果：UAC 拒绝、超时、结果文件缺失）。
    /// 只有这类失败抬高 UAC 放行计数才有意义——helper 正常跑完但逐条失败（跃点行不存在等）
    /// 属业务失败，弹 UAC 解决不了
    channel: bool,
}

/// 调提权 helper 执行单个操作（op 名与参数编码见 helper/mod.rs `build_op_from_args`）。
/// Ok=helper 报告成功；Err=分类失败（提权通道失败 / helper 报告失败）。
/// 六期起出站动作不止写跃点（禁用网卡/加删路由同走此通道），故从
/// run_set_metric_helper 泛化而来。
fn run_helper_op(op: &str, args: &[String], allow_uac_prompt: bool) -> Result<(), HelperFailure> {
    let argv: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let result_name = crate::platform::helper_spawn::new_result_name();
    let v = match crate::platform::helper_spawn::spawn_elevated_helper(
        op,
        &argv,
        &result_name,
        Duration::from_secs(OUTBOUND_HELPER_TIMEOUT_SECS),
        allow_uac_prompt,
    ) {
        Ok(v) => v,
        // 未拿到结果文件：提权通道（计划任务代理/COM/runas）或超时失败
        Err(e) => return Err(HelperFailure { reason: e, channel: true }),
    };
    if v.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
        Ok(())
    } else {
        Err(HelperFailure {
            reason: v
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("helper 操作失败(helper 无 message)")
                .to_string(),
            channel: false,
        })
    }
}

/// 把目标卡指定协议栈的跃点写为 Metric=1（切换动作与"未生效补齐"共用，幂等）。
/// 成功：清退避 + info 日志 +（`notify_success` 时）系统通知（文案带 `notify_suffix`
/// 尾注，如「已临时禁用2张校园网网卡」）；失败：记退避 + warn 日志，首次失败额外通知一次。
/// 提权降级策略照 adapter_watch 的自动启用：首试静默（CMSTPLUA 可用时零 UAC 打扰），
/// 仅在**确因提权通道失败**后允许降级弹 UAC（静默通道被系统封堵时 UAC 是唯一恢复通道），
/// 频率由退避阶梯限制。
fn write_outbound_metric(
    app_handle: &AppHandle,
    name: &str,
    guid: &str,
    families: &[u16],
    notify_success: bool,
    notify_suffix: &str,
) -> bool {
    if families.is_empty() {
        // 卡在但跃点行缺失：写不进去。同样记退避，避免 30s 一拍刷日志；
        // 不属提权通道失败，不抬高 UAC 放行计数
        let count = mark_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
        crate::log_warn!(
            "outbound",
            "夜间出站切换: {name} 当前无可用跃点行(已拔出/协议栈缺失)，{}s 后重试",
            outbound_backoff_ms(count) / 1000
        );
        return false;
    }
    let encoded: Vec<String> = families
        .iter()
        .map(|family| format!("{guid}:{family}:0:1"))
        .collect();
    let allow_uac = outbound_allow_uac();
    match run_helper_op("set_metric", &encoded, allow_uac) {
        Ok(()) => {
            clear_outbound_channel_failure();
            clear_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
            crate::log_info!("outbound", "夜间出站切换: 已切换出站到 {name}{notify_suffix}");
            // 通知只在切换态**首次建立**（apply_outbound_switch）发；补齐/重放路径只记日志，
            // 避免稳态下每拍重复弹同一通知
            if notify_success {
                crate::infra::notification::emit_notification(
                    app_handle,
                    "夜间出站切换",
                    &format!("已切换出站到 {name}{notify_suffix}"),
                    "mascot-portrait",
                );
            }
            true
        }
        Err(failure) => {
            note_channel_failure(&failure);
            let count = mark_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
            crate::log_warn!(
                "outbound",
                "夜间出站切换: {name} 跃点设置失败(第 {count} 次)，{}s 后重试: {}",
                outbound_backoff_ms(count) / 1000,
                failure.reason
            );
            if count == 1 {
                crate::infra::notification::emit_notification(
                    app_handle,
                    "夜间出站切换失败",
                    &format!("无法修改 {name} 的跃点设置，将自动重试"),
                    "mascot-alert",
                );
            }
            false
        }
    }
}

/// 夜间出站切换动作（须在 spawn_blocking 线程内执行）：退避闸 → 选目标卡 → 读两族跃点
/// → 三件套快照落盘（metric/禁用名单/兜底路由，切换态成立的唯一凭据，落盘成功才进内存）
/// → helper 写 Metric=1 → helper 禁用校园网卡 → helper 加兜底路由。
/// 返回 true=快照已落盘且三步 helper 全部成功（本拍完成切换）；false=本拍未完成
/// （无候选/读跃点失败/落盘失败/任一 helper 失败；快照可能已保留，由补齐路径续写）。
///
/// 快照在 helper 之前落盘；helper 失败**不清快照**（裁决 Important-2：若 v4 已改成功而
/// v6 失败，清快照会让已改的跃点无从还原）——切换态保留 + 退避重试写回，重试幂等。
/// 本函数只由 `Switch` 动作调用（`evaluate_night_outbound` 仅在 restore_active=false 时
/// 产出 `Switch`），故入口无需再判快照。
///
/// 六期新增两步（TUN 代理并存时的实测复盘）：
/// - 禁用校园网卡是主力：网卡级下线后其路由/DNS/跃点竞争全部消失，代理内核
///   （FlClashCore auto-detect-interface）只能改绑热点；metric=1 单独压不住
///   metric=0 的 TUN 默认路由，也不产生路由事件、代理不跟随。
/// - 兜底默认路由：同前缀 /0 下 TUN 永远赢（跃点 0 vs 2），本路由只在 TUN/热点
///   默认路由消失后接管，是代理退出后的 failover 保险。
/// - USB 总线网卡绝不入禁用名单（用户实证：USB 网卡被禁用后下次开机无法正常启用）。
fn apply_outbound_switch(app_handle: &AppHandle, config: &crate::config::Config) -> bool {
    if !switch_gate_open() {
        crate::log_debug!("outbound", "夜间出站切换: 退避窗内，本拍跳过");
        return false;
    }
    let adapters = match crate::network::get_adapters_cached() {
        Ok(a) => a,
        Err(e) => {
            crate::log_warn!("outbound", "夜间出站切换: 适配器查询失败: {e}");
            return false;
        }
    };
    // 逐卡补齐网关：Adapter 无 gateway 字段（在 AdapterDetail 里），按名字 join
    let details = crate::network::get_adapter_details_cached().unwrap_or_default();
    let candidates: Vec<(Adapter, String)> = adapters
        .iter()
        .map(|a| {
            let gateway = details
                .iter()
                .find(|d| d.name == a.name)
                .map(|d| d.gateway.clone())
                .unwrap_or_default();
            (a.clone(), gateway)
        })
        .collect();
    // 候选只来自用户排序列表（outbound_priority），且必须有 IP 且判定为非校园网
    let Some(target) = select_outbound_candidate(
        &config.outbound_priority,
        &candidates,
        &config.campus_gateway,
        |gateway, source_ip| crate::network::check_gateway_reachable_from(gateway, Some(source_ip)),
    ) else {
        // 无可用候选（未排序 / 候选都是校园网卡 / 都没有 IP）：不进退避，30s 后自然重试
        crate::log_debug!("outbound", "夜间出站切换: 无可用非校园网候选，本拍不切换");
        return false;
    };
    if target.guid.is_empty() {
        crate::log_warn!("outbound", "夜间出站切换: 目标卡 {} 无 GUID，无法设置跃点", target.name);
        return false;
    }
    // 只处理 IPv4+IPv6 两族（helper 的 set_metric 只认 2/23）
    let rows: Vec<MetricRow> = match crate::platform::metric::read_interface_metrics(&target.guid) {
        Ok(rows) => rows.into_iter().filter(|r| matches!(r.family, 2 | 23)).collect(),
        Err(e) => {
            crate::log_warn!("outbound", "夜间出站切换: 读取 {} 跃点失败: {e}", target.name);
            return false;
        }
    };
    if rows.is_empty() {
        crate::log_warn!("outbound", "夜间出站切换: {} 无 IPv4/IPv6 跃点行，本拍不切换", target.name);
        return false;
    }
    // 要临时禁用的校园网卡：目标卡之外、在优先级列表内、在网且判定为校园网；
    // USB 总线（或总线无法判定）一律跳过。usb_guard 平台注入（非 Windows 恒跳过，
    // 本功能桌面侧仅 Windows 有完整实现）
    #[cfg(target_os = "windows")]
    let usb_guard: fn(&str) -> bool = crate::monitor::outbound_switch::unsafe_to_disable;
    #[cfg(not(target_os = "windows"))]
    let usb_guard: fn(&str) -> bool = |_| true;
    let campus_to_disable = select_campus_to_disable(
        &config.outbound_priority,
        &candidates,
        &config.campus_gateway,
        &target.guid,
        |gateway, source_ip| crate::network::check_gateway_reachable_from(gateway, Some(source_ip)),
        usb_guard,
    );
    // 兜底默认路由：挂在目标卡（出站网卡）的网关+接口上；无网关（罕见）则不挂
    let standby = candidates
        .iter()
        .find(|(a, _)| a.guid == target.guid)
        .and_then(|(_, gateway)| {
            (!gateway.is_empty()).then(|| StandbyRoute {
                dest: "0.0.0.0".to_string(),
                mask: "0.0.0.0".to_string(),
                gateway: gateway.clone(),
                metric: OUTBOUND_STANDBY_ROUTE_METRIC,
                if_index: target.if_index,
            })
        });
    // 先落快照：落盘成功才认定切换态成立（沿夜切"落盘先行"语义，失败不 store 内存）
    let state = app_handle.state::<AppState>();
    let mut fresh: crate::config::Config = (*state.config.load_full()).clone();
    fresh.outbound_metric_restore = snapshot_json(&target.guid, &rows);
    fresh.outbound_disabled_adapters = disabled_adapters_json(&campus_to_disable);
    fresh.outbound_standby_route = standby_route_json(standby.as_ref());
    if let Err(e) = crate::commands::config_cmd::save_config_to_disk_encrypted(app_handle, &fresh) {
        crate::log_warn!("outbound", "夜间出站切换: 切换态快照落盘失败: {e}");
        return false;
    }
    state.config.store(fresh);
    crate::log_info!(
        "outbound",
        "夜间出站切换: 目标卡 {} ({}), 已记录两族跃点快照（待禁用校园网卡 {} 张、兜底路由 {}）",
        target.name,
        target.guid,
        campus_to_disable.len(),
        standby
            .as_ref()
            .map(|r| r.gateway.as_str())
            .unwrap_or("无"),
    );
    let families: Vec<u16> = rows.iter().map(|r| r.family).collect();
    let notify_suffix = if campus_to_disable.is_empty() {
        String::new()
    } else {
        format!("，已临时禁用{}张校园网网卡", campus_to_disable.len())
    };
    let mut all_ok = write_outbound_metric(app_handle, &target.name, &target.guid, &families, true, &notify_suffix);
    // 禁用校园网卡（幂等：已禁用的卡重复 disable 无副作用）。失败记切换退避，
    // 由补齐路径重试——名单已落盘，重试只补失败的卡
    for row in &campus_to_disable {
        match run_helper_op("disable_adapter", std::slice::from_ref(&row.name), outbound_allow_uac()) {
            Ok(()) => {
                clear_outbound_channel_failure();
                crate::log_info!("outbound", "夜间出站切换: 已临时禁用校园网卡 {}", row.name);
            }
            Err(failure) => {
                note_channel_failure(&failure);
                mark_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
                all_ok = false;
                crate::log_warn!("outbound", "夜间出站切换: 禁用校园网卡 {} 失败: {}", row.name, failure.reason);
            }
        }
    }
    // 兜底默认路由（幂等：同参数重复 add 结果一致）
    if let Some(route) = &standby {
        match run_helper_op(
            "route_add",
            &[format!(
                "{}|{}|{}|{}|{}",
                route.dest, route.mask, route.gateway, route.metric, route.if_index
            )],
            outbound_allow_uac(),
        ) {
            Ok(()) => {
                clear_outbound_channel_failure();
                crate::log_info!(
                    "outbound",
                    "夜间出站切换: 已添加兜底默认路由 {} metric {} if {}",
                    route.gateway,
                    route.metric,
                    route.if_index
                );
            }
            Err(failure) => {
                note_channel_failure(&failure);
                mark_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
                all_ok = false;
                crate::log_warn!("outbound", "夜间出站切换: 添加兜底路由失败: {}", failure.reason);
            }
        }
    }
    all_ok
}

/// 补齐/重放切换动作：按三份快照把"切换没写完"的部分补齐，三个分支各自独立、
/// 全部幂等（可反复调用），任一分支失败不影响其余分支：
/// ① 跃点补写：只写"快照记录过原值 + 当前仍存在"的协议栈的 Metric=1，不动快照原值；
/// ② 禁用补做：对名单内且当前存在的校园网卡重发 disable（已在禁用态的卡重复执行无副作用，
///    卡已拔出/消失则跳过）；③ 路由补加：快照有兜底路由则按原参数重发 route add。
/// 两个入口共用：
/// ① 循环内切换态下的"未生效补齐"（helper 部分失败后的退避重试；循环已用
///    [`needs_replay`] 判过失败历史，此处不再判）；
/// ② 启动对账的夜间窗口分支（上次运行被杀，切换没写完——此时进程内失败计数为 0，
///    不能以 `needs_replay` 拦，否则重启后的重放失效）。
/// 成功只记 info 日志、**不发系统通知**（稳态/多拍重放不应重复弹窗）。
fn replay_outbound_switch(app_handle: &AppHandle, config: &crate::config::Config) {
    if !switch_gate_open() {
        return;
    }
    // —— 分支①：跃点补写 ——
    let snapshot = match parse_outbound_snapshot(&config.outbound_metric_restore) {
        Ok(rows) if !rows.is_empty() => Some(rows),
        Ok(_) => None,
        Err(e) => {
            crate::log_warn!("outbound", "夜间出站切换: 跃点快照解析失败，跳过补齐: {e}");
            None
        }
    };
    if let Some(snapshot) = &snapshot {
        let guid = snapshot[0].guid.clone();
        let rows = match crate::platform::metric::read_interface_metrics(&guid) {
            Ok(rows) => rows,
            Err(e) => {
                crate::log_warn!("outbound", "夜间出站切换: 读取目标卡({guid})跃点失败: {e}");
                return;
            }
        };
        // 只写快照有原值、且当前确实存在的协议栈：保证"写过的都能按快照还原"
        let families: Vec<u16> = snapshot
            .iter()
            .map(|r| r.family)
            .filter(|family| rows.iter().any(|r| r.family == *family))
            .collect();
        let name = {
            let adapters = crate::network::get_adapters_cached().unwrap_or_default();
            adapters
                .iter()
                .find(|a| a.guid == guid)
                .map(|a| a.name.clone())
                .unwrap_or_else(|| guid.clone())
        };
        write_outbound_metric(app_handle, &name, &guid, &families, false, "");
    }
    // —— 分支②：禁用补做 ——
    let disabled = parse_disabled_adapters(&config.outbound_disabled_adapters);
    if !disabled.is_empty() {
        let present: std::collections::HashSet<String> =
            crate::network::get_adapters_cached().unwrap_or_default().into_iter().map(|a| a.name).collect();
        for row in &disabled {
            if !present.contains(&row.name) {
                crate::log_debug!("outbound", "夜间出站切换: 禁用名单中的 {} 已不存在，跳过", row.name);
                continue;
            }
            match run_helper_op("disable_adapter", std::slice::from_ref(&row.name), outbound_allow_uac()) {
                Ok(()) => {
                    clear_outbound_channel_failure();
                    crate::log_info!("outbound", "夜间出站切换: 补做禁用校园网卡 {}", row.name);
                }
                Err(failure) => {
                    note_channel_failure(&failure);
                    mark_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
                    crate::log_warn!("outbound", "夜间出站切换: 补做禁用 {} 失败: {}", row.name, failure.reason);
                }
            }
        }
    }
    // —— 分支③：兜底路由补加 ——
    if let Some(route) = parse_standby_route(&config.outbound_standby_route) {
        match run_helper_op(
            "route_add",
            &[format!(
                "{}|{}|{}|{}|{}",
                route.dest, route.mask, route.gateway, route.metric, route.if_index
            )],
            outbound_allow_uac(),
        ) {
            Ok(()) => {
                clear_outbound_channel_failure();
                crate::log_info!("outbound", "夜间出站切换: 补加兜底默认路由 {} metric {}", route.gateway, route.metric);
            }
            Err(failure) => {
                note_channel_failure(&failure);
                mark_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
                crate::log_warn!("outbound", "夜间出站切换: 补加兜底路由失败: {}", failure.reason);
            }
        }
    }
}

/// 夜间出站还原动作（须在 spawn_blocking 线程内执行）。六期起切换态含三份快照，
/// 还原按"反向顺序"执行，已完成的步骤幂等、重试无害；任一步失败保留整个切换态
/// 按退避重试（重做已成功的步骤无副作用）：
/// ① 删兜底路由：失败仅告警继续（runtime 路由重启即消失，不阻塞还原）；
/// ② 按跃点快照把目标卡原样写回（automatic 与 metric 都是快照原值）；目标卡已不存在
///    或跃点行已消失 → 视为已还原（终态出口）；
/// ③ 重新启用名单内校园网卡（名单外的卡不碰；卡已拔出/消失则跳过）。
/// 全部完成后清三份快照 + 通知；连续失败达 OUTBOUND_RESTORE_ALERT_FAILS 各发一次告警
/// 并保留快照。返回 true=切换态已清（还原完成或无需还原）。
fn apply_outbound_restore(app_handle: &AppHandle, config: &crate::config::Config) -> bool {
    let snapshot = match parse_outbound_snapshot(&config.outbound_metric_restore) {
        Ok(rows) => rows,
        Err(e) => {
            // 跃点快照损坏：跃点无凭据可还原（按"已还原"处理），但禁用名单/兜底路由
            // 仍要继续还原——不能在这里整体收尾，否则已禁用的网卡会永远停在禁用态
            crate::log_warn!("outbound", "夜间出站还原: 跃点快照解析失败，按已还原继续: {e}");
            Vec::new()
        }
    };
    if snapshot.is_empty()
        && config.outbound_disabled_adapters.is_empty()
        && config.outbound_standby_route.is_empty()
    {
        // 三份快照全空（非切换态）：按已还原收尾
        return finish_outbound_restore(app_handle);
    }
    if !restore_gate_open() {
        crate::log_debug!("outbound", "夜间出站还原: 退避窗内，本拍跳过");
        return false;
    }
    let allow_uac = outbound_allow_uac();
    let mut route_ok = true;
    // ① 删兜底路由：找不到/not found 同样算成功（已不在）；其余失败不阻塞本拍
    // 后续还原步骤，但阻止收尾清快照——metric=2 兜底路由残留会持续改道流量，
    // 不能按成功收尾（k2.8 审计 P1-2），下一拍整体幂等重试直到删掉为止
    if let Some(route) = parse_standby_route(&config.outbound_standby_route) {
        let row = format!(
            "{}|{}|{}|{}|{}",
            route.dest, route.mask, route.gateway, route.metric, route.if_index
        );
        match run_helper_op("route_delete", &[row], allow_uac) {
            Ok(()) => {
                clear_outbound_channel_failure();
                crate::log_info!("outbound", "夜间出站还原: 已删除兜底默认路由 {}", route.gateway);
            }
            Err(failure) => {
                note_channel_failure(&failure);
                let count = mark_outbound_failure(&OUTBOUND_RESTORE_LAST_FAIL_MS, &OUTBOUND_RESTORE_FAIL_COUNT);
                crate::log_warn!(
                    "outbound",
                    "夜间出站还原: 删除兜底路由失败(第 {count} 次)，{}s 后重试: {}",
                    outbound_backoff_ms(count) / 1000,
                    failure.reason
                );
                if outbound_restore_alert_due(count) {
                    crate::infra::notification::emit_notification(
                        app_handle,
                        "夜间出站还原失败",
                        "兜底路由未能自动删除，出站流量可能仍被改道；将自动重试，也可在系统路由表中手动删除",
                        "mascot-alert",
                    );
                }
                route_ok = false;
            }
        }
    }
    // ② 跃点写回（跃点快照非空才有此步）
    let mut metric_ok = true;
    if !snapshot.is_empty() {
        let guid = snapshot[0].guid.clone();
        let adapters = crate::network::get_adapters_cached().unwrap_or_default();
        if !adapters.iter().any(|a| a.guid == guid) {
            // 终态出口：网卡已拔出/禁用，跃点设置随接口消失，无对象可写回
            crate::log_info!("outbound", "夜间出站还原: 目标卡({guid})已不存在，视为已还原");
        } else {
            // 只写回"快照有原值 + 当前仍存在"的协议栈：跃点行消失的条目无对象可写，
            // 系统重建该协议栈时按其默认值（与"已还原"同语义）
            let current = crate::platform::metric::read_interface_metrics(&guid).unwrap_or_default();
            let encoded: Vec<String> = snapshot
                .iter()
                .filter(|r| current.iter().any(|c| c.family == r.family))
                .map(|r| format!("{}:{}:{}:{}", r.guid, r.family, u8::from(r.automatic), r.metric))
                .collect();
            if encoded.is_empty() {
                crate::log_info!("outbound", "夜间出站还原: 目标卡跃点行已消失，视为已还原");
            } else {
                match run_helper_op("set_metric", &encoded, allow_uac) {
                    Ok(()) => {
                        clear_outbound_channel_failure();
                        crate::log_info!("outbound", "夜间出站还原: 已恢复目标卡跃点设置");
                    }
                    Err(failure) => {
                        note_channel_failure(&failure);
                        let count = mark_outbound_failure(&OUTBOUND_RESTORE_LAST_FAIL_MS, &OUTBOUND_RESTORE_FAIL_COUNT);
                        crate::log_warn!(
                            "outbound",
                            "夜间出站还原失败(第 {count} 次)，{}s 后重试: {}",
                            outbound_backoff_ms(count) / 1000,
                            failure.reason
                        );
                        if outbound_restore_alert_due(count) {
                            crate::infra::notification::emit_notification(
                                app_handle,
                                "夜间出站还原失败",
                                "网卡跃点未能自动恢复，请在系统「网络设置」中手动恢复跃点",
                                "mascot-alert",
                            );
                        }
                        metric_ok = false;
                    }
                }
            }
        }
    }
    // ③ 重新启用校园网卡（名单内有且当前存在的卡）。启用失败=还原未完成：保留整个
    // 切换态（含已写回的跃点快照，重试幂等）下一拍继续
    let disabled = parse_disabled_adapters(&config.outbound_disabled_adapters);
    let mut enable_ok = true;
    let mut enabled_count = 0usize;
    if !disabled.is_empty() {
        let present: std::collections::HashSet<String> = crate::network::get_adapters_cached()
            .unwrap_or_default()
            .into_iter()
            .map(|a| a.name)
            .collect();
        for row in &disabled {
            if !present.contains(&row.name) {
                crate::log_debug!("outbound", "夜间出站还原: 禁用名单中的 {} 已不存在，跳过启用", row.name);
                continue;
            }
            match run_helper_op("enable_adapter", std::slice::from_ref(&row.name), allow_uac) {
                Ok(()) => {
                    clear_outbound_channel_failure();
                    enabled_count += 1;
                    crate::log_info!("outbound", "夜间出站还原: 已重新启用校园网卡 {}", row.name);
                }
                Err(failure) => {
                    note_channel_failure(&failure);
                    let count = mark_outbound_failure(&OUTBOUND_RESTORE_LAST_FAIL_MS, &OUTBOUND_RESTORE_FAIL_COUNT);
                    crate::log_warn!(
                        "outbound",
                        "夜间出站还原: 启用 {} 失败(第 {count} 次): {}",
                        row.name,
                        failure.reason
                    );
                    if outbound_restore_alert_due(count) {
                        crate::infra::notification::emit_notification(
                            app_handle,
                            "夜间出站还原失败",
                            &format!("校园网网卡 {} 未能自动恢复启用，请在系统「网络设置」中手动启用", row.name),
                            "mascot-alert",
                        );
                    }
                    enable_ok = false;
                }
            }
        }
    }
    if !(metric_ok && enable_ok && route_ok) {
        return false;
    }
    // 成功通知只发一次（全部步骤完成后）：文案区分是否真的启用了网卡
    crate::log_info!("outbound", "夜间出站还原: 还原完成");
    let restore_msg = if enabled_count > 0 {
        format!("已还原网络设置（启用 {} 张校园网网卡）", enabled_count)
    } else {
        "已还原网卡跃点设置".to_string()
    };
    crate::infra::notification::emit_notification(
        app_handle,
        "夜间出站切换",
        &restore_msg,
        "mascot-portrait",
    );
    finish_outbound_restore(app_handle)
}

/// 还原收尾：先清切换态快照（三份一起清；落盘成功才进内存），**确认清掉之后**才清退避状态。
/// 磁盘持续故障时快照清不掉：记一次失败让退避闸继续关住重放（不记失败的话
/// 下次判定"无失败历史"会立刻再试，形成每拍写一次跃点的重放风暴）。
/// 此处不计提权通道失败（还原动作本身可能已成功，问题在配置落盘），
/// 也不会触发"还原失败"告警（该告警只在 helper 失败分支发）。
fn finish_outbound_restore(app_handle: &AppHandle) -> bool {
    if !clear_outbound_snapshot(app_handle) {
        mark_outbound_failure(&OUTBOUND_RESTORE_LAST_FAIL_MS, &OUTBOUND_RESTORE_FAIL_COUNT);
        return false;
    }
    clear_outbound_failure(&OUTBOUND_RESTORE_LAST_FAIL_MS, &OUTBOUND_RESTORE_FAIL_COUNT);
    true
}

/// 清空切换态快照（三份：跃点/禁用名单/兜底路由；落盘成功才 store 内存——与切换写入
/// 同款语义，避免"内存已还原、重启后又回到切换态"的错位）；返回快照是否已清。
fn clear_outbound_snapshot(app_handle: &AppHandle) -> bool {
    let state = app_handle.state::<AppState>();
    let mut fresh: crate::config::Config = (*state.config.load_full()).clone();
    if fresh.outbound_metric_restore.is_empty()
        && fresh.outbound_disabled_adapters.is_empty()
        && fresh.outbound_standby_route.is_empty()
    {
        return true;
    }
    fresh.outbound_metric_restore = String::new();
    fresh.outbound_disabled_adapters = String::new();
    fresh.outbound_standby_route = String::new();
    if let Err(e) = crate::commands::config_cmd::save_config_to_disk_encrypted(app_handle, &fresh) {
        crate::log_warn!("outbound", "夜间出站切换: 切换态快照清空落盘失败: {e}");
        return false;
    }
    state.config.store(fresh);
    true
}

/// 启动对账（循环启动前一次性、须在 spawn_blocking 线程内）：三份快照任一非空说明上次
/// 运行留下了切换态（崩溃/被杀/睡眠跨过恢复点）——
/// ① 恢复窗口内、或功能已关闭 → 立即还原；
/// ② 夜间窗口内，只有"目标卡仍在用且判定为非校园网"才重放切换（只写目标值、
///    不动快照原值；禁用补做与路由补加随重放一并幂等执行），否则直接还原、放弃本夜切换。
fn reconcile_outbound_on_startup(app_handle: &AppHandle) {
    let config: crate::config::Config = {
        let s = app_handle.state::<AppState>();
        (*s.config.load_full()).clone()
    };
    if !outbound_restore_active(
        &config.outbound_metric_restore,
        &config.outbound_disabled_adapters,
        &config.outbound_standby_route,
    ) {
        return;
    }
    let now = chrono::Local::now();
    let now_minutes = now.hour() * 60 + now.minute();
    if !config.enable_night_outbound_switch || in_outbound_restore_window(now_minutes) {
        crate::log_info!("outbound", "启动对账: 残留切换态处于恢复期，立即还原");
        apply_outbound_restore(app_handle, &config);
        return;
    }
    // 夜间窗口分支依赖跃点快照确定目标卡；快照不可用（空/损坏）则按还原处理——
    // 正常切换必然三份快照同时写入，缺跃点快照即状态不完整
    let snapshot = match parse_outbound_snapshot(&config.outbound_metric_restore) {
        Ok(rows) if !rows.is_empty() => rows,
        _ => {
            crate::log_warn!("outbound", "启动对账: 跃点快照不可用，按还原处理");
            apply_outbound_restore(app_handle, &config);
            return;
        }
    };
    let guid = snapshot[0].guid.clone();
    let adapters = crate::network::get_adapters_cached().unwrap_or_default();
    let Some(adapter) = adapters.iter().find(|a| a.guid == guid) else {
        crate::log_info!("outbound", "启动对账: 目标卡已不存在，按还原处理");
        apply_outbound_restore(app_handle, &config);
        return;
    };
    let details = crate::network::get_adapter_details_cached().unwrap_or_default();
    let gateway = details
        .iter()
        .find(|d| d.name == adapter.name)
        .map(|d| d.gateway.clone())
        .unwrap_or_default();
    let back_on_campus = adapter.ip.is_empty()
        || is_campus_adapter(&adapter.ip, &gateway, &config.campus_gateway, |gw, source_ip| {
            crate::network::check_gateway_reachable_from(gw, Some(source_ip))
        });
    if back_on_campus {
        crate::log_info!("outbound", "启动对账: 目标卡 {} 已不可用或回到校园网，按还原处理", adapter.name);
        apply_outbound_restore(app_handle, &config);
        return;
    }
    crate::log_info!("outbound", "启动对账: 夜间窗口内重放未完成的出站切换(目标卡 {})", adapter.name);
    replay_outbound_switch(app_handle, &config);
}

/// 单拍判定并置当日标记：命中即标记（无论后续动作成败），当日不重试；
/// 跨天后 today_day 变化，旧标记不再相等，视为未执行（纯函数语义）。
/// 判定本身见 [`gated_night_action`]（含出站切换态互斥）。返回 (应登录, 应注销)。
fn evaluate_and_mark(state: &AppState, now_minutes: u16, today_day: i32, outbound_active: bool) -> (bool, bool) {
    let config = state.config.load_full();
    let (fire_login, fire_logout) = gated_night_action(
        outbound_active,
        now_minutes,
        config.scheduled_login_minutes,
        config.scheduled_logout_minutes,
        state.scheduled.login_day.load(Ordering::Acquire),
        state.scheduled.logout_day.load(Ordering::Acquire),
        today_day,
    );
    // 只有真正要发起动作的拍才耗掉当日额度：切换态下 fire_login=false 且不置标记，
    // 出站还原成功后的下一拍仍走"过点补触发"补上今日登录
    if fire_login {
        state.scheduled.login_day.store(today_day, Ordering::Release);
    }
    if fire_logout {
        state.scheduled.logout_day.store(today_day, Ordering::Release);
    }
    (fire_login, fire_logout)
}

/// 每拍定时动作的最终判定（纯函数，供单测直接断言）：调用 [`should_fire_scheduled_action`]
/// 后再叠加出站互斥——切换态（快照非空）下定时登录注定失败（校园网 portal 从热点出站
/// 不可达），返回 `false` 且调用方不置当日标记（跳过的拍不消耗额度）；定时注销无害，
/// 不受切换态影响。
#[allow(clippy::too_many_arguments)] // 纯函数透传判定所需的 7 个入参，拆结构体只为少两个参数不值
fn gated_night_action(
    outbound_active: bool,
    now_minutes: u16,
    login_target_minutes: u16,
    logout_target_minutes: u16,
    login_day: i32,
    logout_day: i32,
    today_day: i32,
) -> (bool, bool) {
    let fire_login =
        !outbound_active && should_fire_scheduled_action(now_minutes, login_target_minutes, login_day, today_day);
    let fire_logout =
        should_fire_scheduled_action(now_minutes, logout_target_minutes, logout_day, today_day);
    (fire_login, fire_logout)
}

fn format_minutes(minutes: u16) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

/// 定时登录：复用 full_login（与手动/托盘登录同一互斥锁），成功后走
/// post_login_handler（托盘快速登录同款编排，含解除保护期/延迟检测/自动退出）
fn run_scheduled_login(app_handle: &AppHandle, target_minutes: u16) {
    spawn_login_action(
        app_handle,
        &format!("定时登录触发 (目标 {})", format_minutes(target_minutes)),
        "定时登录",
    );
}

/// 登录动作共用执行体（定时登录与晚间断网切换登录同款）：抢 `is_logging_in`
/// 互斥锁，抢不到本拍跳过；日志/通知复用 emit_login_log（"success"/"error"）
fn spawn_login_action(app_handle: &AppHandle, trigger_log: &str, result_prefix: &str) {
    crate::log_info!("scheduled", "{trigger_log}");
    let app_h = app_handle.clone();
    let result_prefix = result_prefix.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let s = app_h.state::<AppState>();
        let _guard = match s.tasks.is_logging_in.try_acquire() {
            Some(g) => g,
            // 已有登录任务在跑：不重复发起
            None => {
                crate::log_warn!("scheduled", "登录动作跳过：已有登录任务进行中");
                return;
            }
        };
        let result = crate::auth::service::full_login(&s, &app_h, None);
        let message = result.message.clone().unwrap_or_default();
        let _ = EventBus::new(&app_h).emit_login_log(
            &format!("{result_prefix}: {message}"),
            if result.success { "success" } else { "error" },
        );
        if result.success {
            crate::auth::service::post_login_handler(&app_h, &s);
        }
    });
}

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
    if let Err(e) = crate::commands::config_cmd::save_config_to_disk_encrypted(app_handle, &config) {
        // 落盘失败不 store 内存（避免"内存已切换、重启后回退"的错位），下拍重试
        crate::log_warn!("scheduled", "晚间断网切换配置落盘失败: {e}");
        return;
    }
    s.config.store(config);
    crate::log_info!("scheduled", "{trigger_log}");
    spawn_login_action(app_handle, &format!("{trigger_log}, 触发登录"), "晚间断网切换");
    // 独立 tokio 任务验证切换是否生效，不阻塞监控循环；期望值取落盘后的 config.operator
    // （SwitchToCampus 后为空串=无锡学院，Restore 后为恢复的后缀）
    let expected_operator = app_handle.state::<AppState>().config.load_full().operator.clone();
    tauri::async_runtime::spawn(verify_night_switch(app_handle.clone(), expected_operator));
}

/// 定时注销：复用 full_logout（与手动注销同一互斥锁）。成功后对齐全量注销后处理
/// （commands::login_cmd::do_logout 的 adapter_name=None 分支）：取消登录后自动退出、
/// 重置登录/重连状态并启动 60s 注销保护期——防止定时注销后被准备自动登录/
/// 掉线重连立即登回（安卓端 do_logout 内置同款保护期语义）。
fn run_scheduled_logout(app_handle: &AppHandle, target_minutes: u16) {
    crate::log_info!("scheduled", "定时注销触发 (目标 {})", format_minutes(target_minutes));
    let app_h = app_handle.clone();
    tauri::async_runtime::spawn_blocking(move || perform_logout(&app_h, "定时注销"));
}

/// 注销执行体（定时注销与夜切验证复登前的注销同款）：抢 `is_logging_out` 互斥锁，
/// 抢不到本拍跳过；成功后取消登录后自动退出、重置登录/重连状态并启动 60s 注销保护期。
fn perform_logout(app_handle: &AppHandle, result_prefix: &str) {
    let s = app_handle.state::<AppState>();
    let result = {
        let _guard = match s.tasks.is_logging_out.try_acquire() {
            Some(g) => g,
            None => {
                crate::log_warn!("scheduled", "{result_prefix}跳过：已有注销任务进行中");
                return;
            }
        };
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
    let _ = tauri::async_runtime::spawn_blocking(move || perform_logout(&app_h, "夜切验证注销")).await;
    spawn_login_action(&app_handle, "夜切验证复登", "夜切验证");
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
fn portal_origin(portal_url: &str) -> String {
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
    fn 还原失败告警_首达阈值触发_此后周期重复() {
        // 未达阈值不告警
        assert!(!outbound_restore_alert_due(0));
        assert!(!outbound_restore_alert_due(1));
        assert!(!outbound_restore_alert_due(2));
        // 首次达阈值（3）告警；此后每 20 次失败重复一次（23、43、…）
        assert!(outbound_restore_alert_due(3));
        assert!(!outbound_restore_alert_due(4));
        assert!(!outbound_restore_alert_due(22));
        assert!(outbound_restore_alert_due(23));
        assert!(!outbound_restore_alert_due(42));
        assert!(outbound_restore_alert_due(43));
    }

    #[test]
    fn 禁用目标_不触发_不置标记() {
        let state = AppState::new();
        let (login, logout) = evaluate_and_mark(&state, 600, TODAY, false);
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
        assert!(!evaluate_and_mark(&state, 460 - 1, TODAY, false).0);
        // 过点补触发（循环拍错过精确分钟）
        assert!(evaluate_and_mark(&state, 505, TODAY, false).0);
        assert_eq!(state.scheduled.login_day.load(AOrd::Acquire), TODAY);
        // 同日不重复
        assert!(!evaluate_and_mark(&state, 600, TODAY, false).0);
        // 注销同款
        assert!(!evaluate_and_mark(&state, 23 * 60 + 29, TODAY, false).1);
        assert!(evaluate_and_mark(&state, 23 * 60 + 30, TODAY, false).1);
        assert_eq!(state.scheduled.logout_day.load(AOrd::Acquire), TODAY);
    }

    #[test]
    fn 跨天重置_次日再触发() {
        let state = AppState::new();
        state.config.update(|c| {
            c.scheduled_login_minutes = 7 * 60 + 40;
        });
        state.scheduled.login_day.store(TODAY - 1, AOrd::Release);
        assert!(evaluate_and_mark(&state, 460, TODAY, false).0);
    }

    #[test]
    fn 切换态_跳过定时登录且不消耗当日标记_注销照常() {
        let state = AppState::new();
        state.config.update(|c| {
            c.scheduled_login_minutes = 7 * 60 + 40;
            c.scheduled_logout_minutes = 23 * 60 + 30;
        });
        // 切换态下过点（07:40 已过、23:30 未到）：登录被跳过，且 login_day 未被置位
        let (login, logout) = evaluate_and_mark(&state, 505, TODAY, true);
        assert!(!login, "切换态下定时登录应跳过");
        assert!(!logout, "未到注销时刻");
        assert_eq!(
            state.scheduled.login_day.load(AOrd::Acquire),
            i32::MIN,
            "跳过的拍不得消耗当日登录标记（否则还原后无法过点补触发）"
        );
        // 注销到点：切换态下照常触发并置位
        let (login, logout) = evaluate_and_mark(&state, 23 * 60 + 30, TODAY, true);
        assert!(!login, "切换态下定时登录仍应跳过");
        assert!(logout, "定时注销在切换态下照常执行");
        assert_eq!(state.scheduled.logout_day.load(AOrd::Acquire), TODAY);
        // 还原成功后的下一拍（outbound_active=false）：过点补触发补上今日登录
        let (login_after, _) = evaluate_and_mark(&state, 23 * 60 + 31, TODAY, false);
        assert!(login_after, "还原成功后应补触发当日定时登录");
        assert_eq!(state.scheduled.login_day.load(AOrd::Acquire), TODAY);
    }

    #[test]
    fn gated_night_action_透传注销判定_仅登录受切换态门控() {
        // 纯函数直测：登录目标 460、注销目标 1410，今日未触发
        assert_eq!(gated_night_action(false, 460, 460, 1410, i32::MIN, i32::MIN, TODAY), (true, false));
        assert_eq!(gated_night_action(true, 460, 460, 1410, i32::MIN, i32::MIN, TODAY), (false, false));
        assert_eq!(gated_night_action(true, 1410, 460, 1410, i32::MIN, i32::MIN, TODAY), (false, true));
        // 未到点：两者皆否
        assert_eq!(gated_night_action(false, 459, 460, 1410, i32::MIN, i32::MIN, TODAY), (false, false));
        // 当日已触发：不再重复
        assert_eq!(gated_night_action(false, 460, 460, 1410, TODAY, TODAY, TODAY), (false, false));
    }

    #[test]
    fn format_minutes_补零() {
        assert_eq!(format_minutes(460), "07:40");
        assert_eq!(format_minutes(1439), "23:59");
    }

    // === 夜间出站切换：退避阶梯与编排策略（纯函数）===

    #[test]
    fn 重放闸_仅切换态且留有失败历史时才补齐() {
        // 非切换态：无论有无失败历史都不重放
        assert!(!needs_replay("", "", "", 0));
        assert!(!needs_replay("", "", "", 3));
        // 切换态但无失败历史（切换已成功、计数已清零）→ 稳态不重放
        // （C1 回归拦截：否则夜间窗口约 900 拍每拍提权重写跃点 + 重复通知）
        assert!(!needs_replay(SNAPSHOT_ONE_ROW, "", "", 0));
        // 切换态 + 失败历史 → 补齐
        assert!(needs_replay(SNAPSHOT_ONE_ROW, "", "", 1));
        assert!(needs_replay(SNAPSHOT_ONE_ROW, "", "", u32::MAX));
    }

    #[test]
    fn 退避窗_起步六十秒_每失败翻倍封顶三百秒() {
        assert_eq!(outbound_backoff_ms(0), 0, "无失败历史不应有退避");
        assert_eq!(outbound_backoff_ms(1), 60_000);
        assert_eq!(outbound_backoff_ms(2), 120_000);
        assert_eq!(outbound_backoff_ms(3), 240_000);
        assert_eq!(outbound_backoff_ms(4), 300_000, "第 4 次失败起封顶");
        assert_eq!(outbound_backoff_ms(u32::MAX), 300_000, "封顶后不再增长");
    }

    #[test]
    fn 退避闸_无失败历史恒放行() {
        assert!(backoff_gate(0, 0, 0));
        assert!(backoff_gate(0, 0, u64::MAX));
        // 时钟基准为 0（未记过失败）时同样放行
        assert!(backoff_gate(0, 3, 1_000_000));
    }

    #[test]
    fn 退避闸_首败六十秒内压制_六十秒后放行() {
        let fail_at = 1_000_000u64;
        assert!(!backoff_gate(fail_at, 1, fail_at + 59_999));
        assert!(backoff_gate(fail_at, 1, fail_at + 60_000));
    }

    #[test]
    fn 退避闸_第二败退避翻倍() {
        let fail_at = 1_000_000u64;
        assert!(!backoff_gate(fail_at, 2, fail_at + 119_999));
        assert!(backoff_gate(fail_at, 2, fail_at + 120_000));
    }

    #[test]
    fn 退避闸_封顶后三百秒内压制_三百秒后放行() {
        let fail_at = 1_000_000u64;
        for count in [4u32, 99, u32::MAX] {
            assert!(!backoff_gate(fail_at, count, fail_at + 299_999), "第 {count} 次失败");
            assert!(backoff_gate(fail_at, count, fail_at + 300_000), "第 {count} 次失败");
        }
    }

    #[test]
    fn 退避闸_时钟回拨按压制处理() {
        assert!(!backoff_gate(1_000_000, 1, 999_000));
    }

    #[test]
    fn 快照解析_空串为空表_非法json报错() {
        assert!(parse_outbound_snapshot("").unwrap().is_empty());
        assert!(parse_outbound_snapshot("[]").unwrap().is_empty());
        assert!(parse_outbound_snapshot("not-json").is_err());
        let rows = parse_outbound_snapshot(SNAPSHOT_ONE_ROW).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].guid, "{G1}");
        assert_eq!(rows[0].family, 2);
        assert!(rows[0].automatic);
        assert_eq!(rows[0].metric, 55);
    }

    /// 单条目快照（与 monitor::outbound_switch::snapshot_json 的同构输出）
    const SNAPSHOT_ONE_ROW: &str = r#"[{"guid":"{G1}","family":2,"automatic":true,"metric":55}]"#;

    /// 出站动作测试用的配置（只改与本功能相关的两个字段）
    fn outbound_test_config(enabled: bool, snapshot: &str) -> crate::config::Config {
        crate::config::Config {
            enable_night_outbound_switch: enabled,
            outbound_metric_restore: snapshot.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn 出站动作_功能关闭但快照残留按还原处理() {
        // 未处于切换态：关闭状态下无动作（既有语义）
        let idle = outbound_test_config(false, "");
        assert_eq!(outbound_action_for(&idle, 0, 1380), NightOutboundAction::None);
        // 快照残留（用户中途关掉开关）：立即还原，不留悬挂的跃点改动
        let leftover = outbound_test_config(false, SNAPSHOT_ONE_ROW);
        assert_eq!(outbound_action_for(&leftover, 0, 1380), NightOutboundAction::Restore);
        assert_eq!(outbound_action_for(&leftover, 0, 500), NightOutboundAction::Restore);
    }

    #[test]
    fn 出站动作_开启时沿用跨平台判定() {
        let active = outbound_test_config(true, SNAPSHOT_ONE_ROW);
        // 夜间窗口内切换态：保持（由补齐路径负责重写目标值），不重复切换
        assert_eq!(outbound_action_for(&active, 0, 1380), NightOutboundAction::None);
        // 恢复窗口内切换态：还原
        assert_eq!(outbound_action_for(&active, 0, 390), NightOutboundAction::Restore);
        // 未切换态且过点：切换
        let idle = outbound_test_config(true, "");
        assert_eq!(outbound_action_for(&idle, 0, 1380), NightOutboundAction::Switch);
        // 未到点：无动作
        assert_eq!(outbound_action_for(&idle, 0, 1379), NightOutboundAction::None);
    }
}
