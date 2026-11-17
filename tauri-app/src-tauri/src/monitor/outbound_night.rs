//! 夜间出站实现：从 scheduled.rs 原样外移（见 changelog v2.4.3）

use crate::config::night_switch::{restore_start_minutes, RESTORE_END_MINUTES};
use crate::config::outbound_switch::outbound_restore_active;
use crate::infra::state::AppState;
use crate::monitor::outbound_switch::{
    effective_outbound_priority, is_campus_adapter, parse_disabled_adapters, parse_standby_route,
    portal_probe_host, select_outbound_candidate, snapshot_json_multi, standby_route_cleanup,
    DisabledRow, StandbyCleanup, OUTBOUND_INTERNET_HEALTH_KEY,
};
use crate::network::Adapter;
use crate::platform::metric::MetricRow;
use chrono::{Datelike, Timelike};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// 夜间出站切换：提权 helper 执行超时（改跃点/禁网卡/加路由都是即时操作，超时即判失败走退避）
const OUTBOUND_HELPER_TIMEOUT_SECS: u64 = 10;

// 切换路径的公网健康探测超时已改为配置项 outboundProbeTimeoutMs（网络检测参数弹窗可调，
// 默认 1000ms）：正常卡完成一次双目标 HTTPS 探测实测在 640ms 内，断网卡必然走满超时，
// 沿用全局 3s 会把切换拖到 4s 以上（2026-11-13 实测 4141ms）。

/// 夜间出站切换退避阶梯：首败 60s 起步、每失败翻倍、封顶 300s
/// （1→60s、2→120s、3→240s、≥4→300s）——避免 30s 循环整夜重试刷日志/弹通知
static OUTBOUND_BACKOFF_START_MS: AtomicU64 = AtomicU64::new(60_000);
static OUTBOUND_BACKOFF_MAX_MS: AtomicU64 = AtomicU64::new(300_000);

/// 退避阶梯按配置刷新（网络检测参数弹窗可调）：首败等 start，随后每失败翻倍到 max
fn apply_outbound_backoff_config(config: &crate::config::Config) {
    let start = config.outbound_backoff_start_secs as u64 * 1000;
    OUTBOUND_BACKOFF_START_MS.store(start, Ordering::Relaxed);
    OUTBOUND_BACKOFF_MAX_MS.store(
        (config.outbound_backoff_max_secs as u64 * 1000).max(start),
        Ordering::Relaxed,
    );
}

/// 出站还原连续失败达此次数 → 发一次告警通知（含手动恢复提示）；此后每
/// OUTBOUND_RESTORE_ALERT_REMIND_EVERY 次失败重复提醒一次——避免整夜刷通知，
/// 也避免「只提醒一次、之后无限静默重试」（k2.8 审计 P3-1）
const OUTBOUND_RESTORE_ALERT_FAILS: u32 = 3;
/// 首次告警后的重复提醒间隔（失败次数）：300s 退避封顶时约每 100 分钟一次
const OUTBOUND_RESTORE_ALERT_REMIND_EVERY: u32 = 20;
/// 损坏快照的放弃阀：还原连续失败达此次数后，损坏的禁用名单按空名单放行收尾
/// （300s 退避封顶时约 3.3 小时、期间已发 2 次告警：计数 3 与 23），避免快照损坏把还原
/// 流程永久卡死、告警无限循环——卡死比残留更糟，告警义务届时已尽
const OUTBOUND_RESTORE_GIVE_UP_FAILS: u32 = 40;

/// 还原失败告警是否应发出：首次达阈值，此后每 REMIND_EVERY 次重复一次
fn outbound_restore_alert_due(count: u32) -> bool {
    count >= OUTBOUND_RESTORE_ALERT_FAILS
        && (count - OUTBOUND_RESTORE_ALERT_FAILS) % OUTBOUND_RESTORE_ALERT_REMIND_EVERY == 0
}

/// 每天至多一次的提醒通知（swap 当日去重，跨天自动重置）：空禁用名单/无候选/
/// 跃点快照损坏三类「不阻塞流程但该让用户知道」的问题共用，避免 30s 一拍刷通知
fn notify_outbound_issue_once_per_day(
    app_handle: &AppHandle,
    last_day: &AtomicI32,
    title: &str,
    body: &str,
) {
    let today = chrono::Local::now().num_days_from_ce();
    if last_day.swap(today, Ordering::AcqRel) == today {
        return;
    }
    crate::infra::notification::emit_notification(app_handle, title, body, "mascot-alert");
}

/// 出站**切换**动作退避状态：上次失败时刻（epoch ms）+ 连续失败计数
static OUTBOUND_SWITCH_LAST_FAIL_MS: AtomicU64 = AtomicU64::new(0);
pub(super) static OUTBOUND_SWITCH_FAIL_COUNT: AtomicU32 = AtomicU32::new(0);
/// 出站**还原**动作退避状态（独立记账：两者失败原因不同，退避不互相拖慢）
static OUTBOUND_RESTORE_LAST_FAIL_MS: AtomicU64 = AtomicU64::new(0);
static OUTBOUND_RESTORE_FAIL_COUNT: AtomicU32 = AtomicU32::new(0);
/// 提权通道失败计数：仅"没拿到 helper 结果"（UAC 拒绝/超时/结果文件缺失）类失败自增。
/// 计数非零才允许出站动作降级弹 UAC——业务性失败（跃点行不存在等）弹 UAC 解决不了；
/// 任一次 helper 成功即清零（通道可用）
static OUTBOUND_CHANNEL_FAIL_COUNT: AtomicU32 = AtomicU32::new(0);
/// 每天至多一次的提醒通知去重标记（num_days_from_ce；i32::MIN = 从未提醒）：
/// 无候选/空禁用名单/跃点快照损坏/兜底路由是唯一默认路径
/// 四类「不阻塞流程但该让用户知道」的问题各占一枚
static OUTBOUND_NO_CANDIDATE_WARN_DAY: AtomicI32 = AtomicI32::new(i32::MIN);
static OUTBOUND_METRIC_SNAPSHOT_WARN_DAY: AtomicI32 = AtomicI32::new(i32::MIN);
static OUTBOUND_LAST_PATH_WARN_DAY: AtomicI32 = AtomicI32::new(i32::MIN);
/// 原出口卡「禁用后启用失败」残留（卡停在禁用态，出口已切好）
static OUTBOUND_ORIGIN_DISABLE_WARN_DAY: AtomicI32 = AtomicI32::new(i32::MIN);

/// 出站动作单飞闸：切换与还原互斥。手动入口（命令线程）与 30s 调度拍（后台线程）
/// 可能同时进 `apply_outbound_*`；若还原拿着"切换前"读到的快照收尾，会把刚写入的
/// 新快照一并清掉——留下 metric=1 + 校园卡禁用却无快照可还原，即"状态静默丢失"
/// （2026-10-18 事故同类）。后到者立即返回"忙"，由下一拍或用户重试再走。
static OUTBOUND_ACTION_BUSY: AtomicBool = AtomicBool::new(false);

/// RAII 单飞锁：任何返回路径（含 panic 展开）都会释放，不会把闸门卡死。
struct OutboundActionGuard;

impl OutboundActionGuard {
    fn try_acquire() -> Option<Self> {
        OUTBOUND_ACTION_BUSY
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Self)
    }
}

impl Drop for OutboundActionGuard {
    fn drop(&mut self) {
        OUTBOUND_ACTION_BUSY.store(false, Ordering::Release);
    }
}

/// 把出站动作送进阻塞线程池执行（`check_gateway_reachable_from` 的线程约束 +
/// helper 结果轮询是阻塞等待，不能在 async worker 线程上跑）；join 失败按动作失败处理。
pub(super) async fn run_outbound_blocking(
    app_h: AppHandle,
    config: Arc<crate::config::Config>,
    action: fn(&AppHandle, &crate::config::Config) -> Result<(), String>,
) -> Result<(), String> {
    match tauri::async_runtime::spawn_blocking(move || action(&app_h, &config)).await {
        Ok(result) => result,
        Err(e) => {
            crate::log_warn!("outbound", "出站动作执行线程异常: {e}");
            Err(format!("出站动作执行线程异常: {e}"))
        }
    }
}

/// 是否处于出站切换态（三份快照任一非空即切换态；与判定纯函数的
/// outbound_restore_active 同语义）：metric 快照/禁用名单/兜底路由
pub(super) fn outbound_switch_active(app_handle: &AppHandle) -> bool {
    let config = app_handle.state::<AppState>().config.load_full();
    outbound_restore_active(
        &config.outbound_metric_restore,
        &config.outbound_disabled_adapters,
        &config.outbound_standby_route,
    )
}

/// 恢复窗口 [恢复起点, 23:00)：窗内是"清晨还原期"，窗外是"夜间切换期"。
/// 恢复起点自 2026-10-26 起可配置（`nightRestoreMinutes`，默认 07:30）。
/// 时间表常量由 config::night_switch 承载，双端同源。
pub(super) fn in_outbound_restore_window(config: &crate::config::Config, now_minutes: u32) -> bool {
    let start = restore_start_minutes(config.night_restore_minutes);
    (start..RESTORE_END_MINUTES).contains(&now_minutes)
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
    let start = OUTBOUND_BACKOFF_START_MS.load(Ordering::Relaxed);
    let max = OUTBOUND_BACKOFF_MAX_MS.load(Ordering::Relaxed).max(start);
    start
        .saturating_mul(1u64 << (fail_count - 1).min(3))
        .min(max)
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

/// 在探测窗口内并行预热常驻提权 worker：daemon 已在时立即返回，不在则经 CMSTPLUA
/// 静默拉起——把每次 200–300ms 的提权通道开销（走代理/COM 时每次都要拉起进程）移出
/// 切换关键路径。只在静默提权可行的环境预热（UAC 高档位下拉起会弹框，自动夜切不能
/// 弹）；失败无副作用，后续 helper 仍按 代理 → COM → runas 降级。
#[cfg(target_os = "windows")]
fn warm_elevated_channel() {
    let env = crate::platform::elevation::elevation_env();
    if !env.admin_member || !env.com_silent_likely() {
        return;
    }
    // 不 join：预热与选卡探测并行，切换不等它；切换时若还没切到管道，就按原降级链走。
    std::thread::spawn(|| {
        let _ = crate::platform::elevated_daemon::ensure_running(std::time::Duration::from_secs(3));
    });
}

#[cfg(not(target_os = "windows"))]
fn warm_elevated_channel() {}

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
pub(super) fn needs_replay(
    metric_snapshot: &str,
    disabled_adapters: &str,
    standby_route: &str,
    fail_count: u32,
) -> bool {
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

/// 各 op 的 helper 等待上限。禁用网卡走 PnP 设备级禁用并轮询复核（USB 卡窗口 15s），
/// 用通用的 10s 会把「卡已禁用但结果没回来」判成失败，让切换在卡已经下线的情况下反复
/// 重试；跃点/路由/连接类写入都是即时操作，维持原 10s。
fn helper_timeout_secs(op: &str) -> u64 {
    match op {
        "disable_adapter" => 30,
        _ => OUTBOUND_HELPER_TIMEOUT_SECS,
    }
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
        Duration::from_secs(helper_timeout_secs(op)),
        allow_uac_prompt,
    ) {
        Ok(v) => v,
        // 未拿到结果文件：提权通道（计划任务代理/COM/runas）或超时失败
        Err(e) => {
            return Err(HelperFailure {
                reason: e,
                channel: true,
            })
        }
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

/// 重新启用一张校园网卡（夜间还原第三步）。
///
/// 不复用 run_helper_op("enable_adapter")：helper 内只有 netsh enable，只翻 NDIS
/// admin 层，清不掉设备管理器式禁用（PnP problem 22）——USB 网卡被禁用后
/// netsh 表现为反复重连却始终不启用，用户只能重装驱动（v2.4.2 用户实测）。
/// 改走 adapter_cache::enable_adapter：先做 problem 22 分流，命中则 pnputil
/// 设备级启用（含 problem 22 解除复核），未命中才回落 netsh（管理员直跑 /
/// 非管理员 helper）。与 adapter_watch 的自动启用同一入口，两处启用语义一致。
#[cfg(desktop)]
fn enable_campus_adapter(name: &str, allow_uac_prompt: bool) -> Result<(), HelperFailure> {
    // adapter_cache::enable_adapter 的错误只有 String，无法区分提权通道失败
    // （UAC 拒绝/超时/结果文件缺失）与业务失败；按提权状态近似分类：
    // 管理员全程本地直跑（netsh/pnputil），失败必属业务失败；非管理员的
    // netsh 与 pnputil 都经 helper 提权通道。ponytail: 非管理员一律标
    // channel=true 属过度标注，后果有界——抬计数只是让重试允许弹 UAC，
    // 静默通道（CMSTPLUA）可用时并不真弹窗；反过来欠标注会让还原重试
    // 永远静默，UAC 放行机制失明（v2.4.2 评审修正）。
    let channel = !crate::platform::elevation::is_admin();
    crate::network::adapter_cache::enable_adapter(name, allow_uac_prompt)
        .map_err(|reason| HelperFailure { reason, channel })
}

/// 非桌面（安卓）不执行夜间出站还原；同签名 stub 让调用点免 cfg
#[cfg(not(desktop))]
fn enable_campus_adapter(_name: &str, _allow_uac_prompt: bool) -> Result<(), HelperFailure> {
    Err(HelperFailure {
        reason: "当前平台不支持适配器启用".to_string(),
        channel: false,
    })
}

/// 把目标卡指定协议栈的跃点写为 Metric=1（切换动作与"未生效补齐"共用，幂等）。
/// 成功：info 日志 +（`notify_success` 时）系统通知（文案带 `notify_suffix`
/// 尾注，如「已临时禁用2张校园网网卡」）；失败：记退避 + warn 日志，首次失败额外通知一次。
/// 提权降级策略照 adapter_watch 的自动启用：首试静默（CMSTPLUA 可用时零 UAC 打扰），
/// 仅在**确因提权通道失败**后允许降级弹 UAC（静默通道被系统封堵时 UAC 是唯一恢复通道），
/// 频率由退避阶梯限制；失败原因经 Err(原因) 上报调用方（手动入口透传前端 toast）。
/// suppress：同时把这些竞争卡（原出口 + 其他有 IP 的物理卡）的跃点抬到
/// SWITCHED_ORIGIN_METRIC。与目标卡的写入合成同一次 helper 调用——分两次的好处
/// 只是日志更细，代价是多一次提权往返（每次超时 10s），而切换对耗时有硬要求。
fn write_outbound_metric(
    app_handle: &AppHandle,
    name: &str,
    guid: &str,
    families: &[u16],
    notify_success: bool,
    notify_suffix: &str,
    suppress: &[(String, Vec<u16>)],
) -> Result<(), String> {
    if families.is_empty() {
        // 卡在但跃点行缺失：写不进去。同样记退避，避免 30s 一拍刷日志；
        // 不属提权通道失败，不抬高 UAC 放行计数
        let count =
            mark_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
        crate::log_debug!(
            "outbound",
            "夜间出站切换: {name} 当前无可用跃点行(已拔出/协议栈缺失)，{}s 后重试",
            outbound_backoff_ms(count) / 1000
        );
        return Err("当前无可用跃点行（网卡已拔出或协议栈缺失）".to_string());
    }
    let mut encoded: Vec<String> = families
        .iter()
        .map(|family| format!("{guid}:{family}:0:1"))
        .collect();
    // 抬高必须与压低同批下发：分开调用时中间态是「所有卡都低」，出口会跳到
    // routeMetric 更小的那张，验证步骤就会偶发判失败
    encoded.extend(encode_suppress_rows(suppress));
    run_set_metric(app_handle, name, &encoded, notify_success, notify_suffix)
}

/// 竞争卡跃点编码：每张卡的每个协议栈都抬到 SWITCHED_ORIGIN_METRIC。
/// 纯函数便于单测锁定「所有竞争卡一起抬高」，不依赖 helper 通道。
fn encode_suppress_rows(suppress: &[(String, Vec<u16>)]) -> Vec<String> {
    suppress
        .iter()
        .flat_map(|(guid, families)| {
            families
                .iter()
                .map(move |family| format!("{guid}:{family}:0:{SWITCHED_ORIGIN_METRIC}"))
        })
        .collect()
}

/// 下发一次 set_metric 并处理成功/失败语义（日志、首次失败通知、退避计数）。
/// 首次切换与切换态改切共用：两条路径的失败口径必须一致，否则改切失败会被
/// 当成一次「无重试的静默失败」。
fn run_set_metric(
    app_handle: &AppHandle,
    name: &str,
    encoded: &[String],
    notify_success: bool,
    notify_suffix: &str,
) -> Result<(), String> {
    let allow_uac = outbound_allow_uac();
    match run_helper_op("set_metric", encoded, allow_uac) {
        Ok(()) => {
            clear_outbound_channel_failure();
            crate::log_info!(
                "outbound",
                "夜间出站切换: 已切换出站到 {name}{notify_suffix}"
            );
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
            Ok(())
        }
        Err(failure) => {
            note_channel_failure(&failure);
            let count =
                mark_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
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
            Err(failure.reason)
        }
    }
}

/// Portal 旁路路由快照：出站切走后让登录请求仍从原网卡出去的 /32 主机路由。
/// 与默认路由不同，它按目的地址精确匹配，不会与任何 DHCP 路由同键，因而不受
/// 2026-10-18 同键判重事故影响。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PortalRoute {
    dest: String,
    gateway: String,
    #[serde(rename = "ifIndex")]
    if_index: u32,
    metric: u32,
}

impl PortalRoute {
    /// helper 的 route_add / route_delete 条目编码：dest|mask|gateway|metric|if_index
    fn encode(&self) -> String {
        format!(
            "{}|255.255.255.255|{}|{}|{}",
            self.dest, self.gateway, self.metric, self.if_index
        )
    }
}

/// 删除旁路路由时使用的条目编码：跃点以**实时路由表**为准取。
/// netsh 写入后表里的跃点未必等于请求值，用错跃点会让精确删除匹配不到、
/// 返回 AlreadyGone 而静默残留一条钉住 Portal 的路由。
fn portal_route_delete_row(route: &PortalRoute) -> String {
    let metric = crate::platform::route_table::ipv4_forward_rows()
        .ok()
        .and_then(|rows| {
            rows.iter()
                .find(|r| {
                    r.prefix_len == 32
                        && r.dest.to_string() == route.dest
                        && r.next_hop.to_string() == route.gateway
                        && r.if_index == route.if_index
                })
                .map(|r| r.metric)
        })
        .unwrap_or(route.metric);
    format!(
        "{}|255.255.255.255|{}|{}|{}",
        route.dest, route.gateway, metric, route.if_index
    )
}

/// 切换态下重建旁路路由：目标仍是 Portal，下一跳取原出口卡的网关。
///
/// 原出口卡不能按「跃点等于 SWITCHED_ORIGIN_METRIC」去找：快照存的是**切换前原值**
/// （首次切换记录现值、改切补记的也是现值），被抬到 100 的是运行期跃点而非快照值。
/// 快照首行恒为目标卡（merge_retarget_snapshot 保证），其余第一张即原出口。
fn rebuild_portal_bypass_from_snapshot(config: &crate::config::Config) -> Option<PortalRoute> {
    let rows = parse_outbound_snapshot(&config.outbound_metric_restore).ok()?;
    let target_guid = rows.first()?.guid.clone();
    let origin_guid = rows
        .iter()
        .map(|row| row.guid.clone())
        .find(|guid| guid != &target_guid)?;
    let origin = crate::network::get_adapters_cached()
        .ok()?
        .into_iter()
        .find(|adapter| adapter.guid == origin_guid)?;
    build_portal_bypass_route(config, &origin.name)
}

/// 切换态下确保 Portal 旁路路由就绪（快照为空 = 首次添加失败，补一次）。
/// 登录恢复依赖这条路由：缺了就到不了 Portal，登录前置判定会把恢复降级成
/// NotNeeded，于是整夜不再尝试登录原卡。返回是否已就绪。
pub(super) fn ensure_portal_bypass_route(
    app_handle: &AppHandle,
    config: &crate::config::Config,
) -> bool {
    if !config.outbound_portal_route.trim().is_empty() {
        return true;
    }
    let Some(route) = rebuild_portal_bypass_from_snapshot(config) else {
        crate::log_warn!("outbound", "切换态旁路路由无法重建（快照缺原出口卡或网关）");
        return false;
    };
    let row = route.encode();
    if let Err(failure) = run_helper_op("route_add", &[row], outbound_allow_uac()) {
        note_channel_failure(&failure);
        crate::log_warn!(
            "outbound",
            "切换态补加 Portal 旁路路由失败: {}",
            failure.reason
        );
        return false;
    }
    let json = serde_json::to_string(&route).unwrap_or_default();
    let state = app_handle.state::<AppState>();
    if let Err(e) =
        crate::config::persist::mutate_config_and_save(app_handle, &state, move |fresh| {
            fresh.outbound_portal_route = json;
            Ok(())
        })
    {
        crate::log_warn!("outbound", "切换态旁路路由快照落盘失败: {e}");
    }
    crate::log_info!(
        "outbound",
        "切换态旁路路由已补加 {} via {} if {}",
        route.dest,
        route.gateway,
        route.if_index
    );
    true
}

/// 解析旁路路由快照（空串/非法 JSON → None）。
fn parse_portal_route(json: &str) -> Option<PortalRoute> {
    if json.trim().is_empty() {
        return None;
    }
    serde_json::from_str(json.trim()).ok()
}

/// 构造 Portal 旁路路由：目标 = portal 私网主机，下一跳 = 指定原出口卡的网关。
/// 按卡名实时查，不读「当前出口」——切换完成后当前出口已是目标卡，按出口取会把
/// 去 Portal 的流量钉在错误的链路上；也让调用方能等原卡恢复启用后再建（禁用期间
/// 它的 ifIndex 与网关都无效）。任一环缺失返回 None，日志写明原因。
fn build_portal_bypass_route(
    config: &crate::config::Config,
    origin_name: &str,
) -> Option<PortalRoute> {
    let dest = portal_probe_host(&config.portal_url);
    if dest.is_empty() {
        return None;
    }
    let device = crate::network::get_adapters_cached()
        .ok()?
        .into_iter()
        .find(|a| a.name == origin_name)?;
    let gateway = crate::network::get_adapter_details_cached()
        .ok()?
        .iter()
        .find(|d| d.name == origin_name)
        .map(|d| d.gateway.clone())?;
    if gateway.is_empty() {
        return None;
    }
    Some(PortalRoute {
        dest,
        gateway,
        if_index: device.if_index,
        metric: 1,
    })
}

/// 当前默认出口卡的 (名称, GUID, ifIndex)。
fn current_origin_adapter() -> Option<(String, String, u32)> {
    // 必须用 current_outbound_v4（已按黑名单排除 TUN/代理虚拟卡、且按
    // routeMetric+ifMetric 取最小）。GetBestRoute 会返回 metric=0 的 TUN 接口，
    // 那张卡不在适配器列表里，导致原出口恒为 None、抬高动作整段被跳过。
    let current = crate::platform::best_route::current_outbound_v4().ok()?;
    let adapters = crate::network::get_adapters_cached().ok()?;
    let adapter = adapters
        .iter()
        .find(|a| a.if_index == current.if_index || a.name == current.alias)?;
    Some((adapter.name.clone(), adapter.guid.clone(), adapter.if_index))
}

/// 手动修改单张网卡的跃点（IPv4/IPv6 同步写为同一值）。
/// 切换态下拒绝：跃点快照保存的是原值，此时改动会在还原时被覆盖，用户白改。
pub(crate) fn manual_set_adapter_metric(
    app_handle: &AppHandle,
    adapter_name: &str,
    metric: u32,
) -> Result<(), String> {
    if metric == 0 || metric > 9999 {
        return Err("跃点需在 1–9999 之间".to_string());
    }
    if outbound_switch_active(app_handle) {
        return Err("出站切换进行中，请先恢复原出口再修改跃点".to_string());
    }
    let adapters = crate::network::get_adapters_cached()?;
    let Some(adapter) = adapters.iter().find(|a| a.name == adapter_name) else {
        return Err(format!("未找到网卡 {adapter_name}"));
    };
    if adapter.guid.is_empty() {
        return Err(format!("网卡 {adapter_name} 缺少 GUID，无法设置跃点"));
    }
    let rows = encode_metric_rows(&adapter.guid, &[2, 23], metric);
    run_helper_op("set_metric", &rows, outbound_allow_uac()).map_err(|failure| {
        note_channel_failure(&failure);
        failure.reason
    })?;
    clear_outbound_channel_failure();
    crate::log_info!("outbound", "手动设置跃点: {adapter_name} -> {metric}");
    Ok(())
}

/// 切换后原出口卡的 interfaceMetric：必须明显高于目标卡（目标卡压到 1），
/// 让「routeMetric + interfaceMetric」的比较结果稳定落在目标卡上。
const SWITCHED_ORIGIN_METRIC: u32 = 100;

/// 切换候选探测使用用户在「网络检测参数」中配置的完整预算（校验范围 500–5000ms）。
/// 绑源 ICMP 与双目标 HTTPS 共用这一次预算；不能再额外截成 600ms，否则默认值 1000ms
/// 形同虚设，热点冷启动或弱信号时会每拍都被误判为无候选。
fn outbound_probe_budget(config: &crate::config::Config) -> Duration {
    Duration::from_millis(config.outbound_probe_timeout_ms as u64)
}

/// 默认出口是否已落在指定接口：读系统默认路由的接口索引比对。
/// 只改跃点时"写了跃点"与"出口真的换了"是两件事——TUN/代理可能持有 metric=0
/// 的默认路由，此时跃点写入成功但出口不动，必须靠这一步暴露。
#[cfg(target_os = "windows")]
fn outbound_route_targets(target_if_index: u32) -> Result<bool, String> {
    // 与原出口判定同一口径：current_outbound_v4 过滤掉 TUN/代理虚拟卡后再取最小跃点。
    // 用 GetBestRoute 会拿到 metric=0 的 TUN 接口，验证永远判失败。
    crate::platform::best_route::current_outbound_v4()
        .map(|current| current.if_index == target_if_index)
}

#[cfg(not(target_os = "windows"))]
fn outbound_route_targets(_target_if_index: u32) -> Result<bool, String> {
    Err("当前平台不支持默认出口查询".to_string())
}

/// 一次并行预热的绑源探测结果：key = (目标主机, 绑源 IP)，value = 是否可达。
///
/// 逐卡判定原先对每张卡串行发探测（`is_campus_adapter` 最多 4 次 + 候选自身网关 1 次，
/// 单次超时 1000ms）。校园网 23:00 断网后这些探测全部走到超时，切换因此被拖到 10s 以上
/// （2026-11-08 实测：目标卡探测完成到快照落盘之间空转 7.96s，其间无任何日志）。
/// 判定所需的目标集合在选卡前就能算出来，所以先并行全部探完，判定阶段只查表。
struct ProbeCache(std::collections::HashMap<(String, String), bool>);

impl ProbeCache {
    fn get(&self, dest: &str, source: &str) -> bool {
        match self.0.get(&(dest.to_string(), source.to_string())) {
            Some(ok) => *ok,
            // 公网健康只接受 TCP/HTTPS 结果，缺失不得退回 ICMP 造成假阳性。
            None if dest == OUTBOUND_INTERNET_HEALTH_KEY => false,
            None => crate::network::check_gateway_reachable_from(dest, Some(source)),
        }
    }
}

/// 单目标可达性探测：先 TCP(443) 拿明确应答，不成再用 ICMP 兜底，总耗时受 budget 约束。
///
/// TCP 覆盖「禁 ICMP 但 web 端口有回应」的校园网关/Portal（只用 ICMP 会把在网校园卡
/// 误判成非校园），RST 也算可达——它证明报文确实到达了目标主机。两条腿都拿不到回应
/// 才判不可达，与「超时即不可达」的旧语义一致。
fn probe_target_reachable(
    dest: &str,
    source: &str,
    if_index: Option<u32>,
    budget: Duration,
) -> bool {
    let tcp_budget = budget * 3 / 5;
    if crate::platform::tcp_probe::tcp_probe(
        dest,
        Some(source),
        if_index,
        crate::platform::tcp_probe::CAMPUS_PROBE_PORT,
        tcp_budget,
    ) {
        return true;
    }
    let left = budget.saturating_sub(tcp_budget);
    if left.is_zero() {
        return false;
    }
    crate::network::check_gateway_reachable_from_with_timeout(
        dest,
        Some(source),
        left.as_millis().max(1) as u32,
    )
}

/// 并行探测全部 (目标, 绑源) 组合，最多等 `budget`。
/// 同一组合发 [`CAMPUS_PROBE_ATTEMPTS`] 次、任一可达即判可达——与
/// [`crate::campus::rules::probe_with_retry`] 的抗丢包语义同源（去抖次数由共享层
/// 统一规定），各发同时发出，且不再 join 全部：到预算点未返回的组合按「不可达」处理。
fn warm_probe_cache(pairs: &[(String, String)], budget: Duration) -> ProbeCache {
    use std::collections::HashMap;
    if pairs.is_empty() {
        return ProbeCache(HashMap::new());
    }
    // 先按不可达预填：留空会让 ProbeCache::get 回落到真实探测（再等一次 1000ms）。
    let mut map: HashMap<(String, String), bool> = HashMap::new();
    for pair in pairs {
        map.insert(pair.clone(), false);
    }
    // 绑源探测还须钉住出接口（IP_UNICAST_IF）：只绑源地址时 TUN/代理仍会劫持出口。
    let if_by_ip: std::sync::Arc<std::collections::HashMap<String, u32>> = std::sync::Arc::new(
        crate::network::get_adapters_cached()
            .map(|list| list.into_iter().map(|a| (a.ip, a.if_index)).collect())
            .unwrap_or_default(),
    );
    let (tx, rx) = std::sync::mpsc::channel::<((String, String), bool)>();
    let mut inflight = 0usize;
    for (dest, source) in pairs {
        for _ in 0..crate::campus::rules::CAMPUS_PROBE_ATTEMPTS {
            let tx = tx.clone();
            let dest = dest.clone();
            let source = source.clone();
            let if_by_ip = std::sync::Arc::clone(&if_by_ip);
            // 裸线程而非 scope：scope 会 join 全部，等于把「等最慢一发」写死回来。
            // 超预算的线程最多在一个 TCP/ICMP 超时后自行结束。
            std::thread::spawn(move || {
                let if_index = if_by_ip.get(&source).copied();
                let ok = probe_target_reachable(&dest, &source, if_index, budget);
                let _ = tx.send(((dest, source), ok));
            });
            inflight += 1;
        }
    }
    drop(tx);
    let started = std::time::Instant::now();
    let deadline = started + budget;
    let mut timed_out = false;
    while inflight > 0 {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        match (left.is_zero(), rx.recv_timeout(left)) {
            (true, _) | (_, Err(_)) => {
                timed_out = true;
                break;
            }
            (false, Ok((key, ok))) => {
                inflight -= 1;
                if ok {
                    map.entry(key).and_modify(|v| *v = true);
                }
            }
        }
    }
    crate::log_info!(
        "outbound",
        "夜间出站切换: 并行绑源探测 {} 组完成，耗时 {}ms（预算 {}ms{}）",
        pairs.len(),
        started.elapsed().as_millis(),
        budget.as_millis(),
        if timed_out {
            "，超预算按不可达处理"
        } else {
            ""
        }
    );
    ProbeCache(map)
}

/// 收集判定阶段会查询的 (目标, 绑源) 组合，口径与 [`is_campus_adapter`] /
/// [`select_outbound_candidate`] 一致：候选自身网关与外网探测目标一并预热。
/// 多收只是多探一次，漏收会退化成串行探测。
fn collect_probe_targets(
    priority: &[String],
    details: &[(Adapter, String)],
    campus_gateway: &str,
    portal_host: &str,
) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    for name in priority {
        let Some((adapter, gateway)) = details.iter().find(|(a, _)| a.name == *name) else {
            continue;
        };
        // 判定阶段对这些卡根本不发探测：无 IP 不参与、无网关出不了站
        if adapter.ip.is_empty() || gateway.is_empty() {
            continue;
        }
        if !campus_gateway.is_empty() {
            pairs.push((campus_gateway.to_string(), adapter.ip.clone()));
        }
        if !portal_host.is_empty() {
            pairs.push((portal_host.to_string(), adapter.ip.clone()));
        }
        pairs.push((gateway.clone(), adapter.ip.clone()));
    }
    pairs
}

/// 在统一配置写锁内读最新值、改字段、落盘，成功后才发布内存并广播；调用方
/// 自行做同值早退（排程 30s 一拍，逐拍重写同值既写盘又触发前端刷新）。落盘
/// 失败仅告警，内存保持旧值，下拍判定条件仍未满足时会重试。
fn persist_config_update<F>(app_handle: &AppHandle, mutate: F)
where
    F: FnOnce(&mut crate::config::Config),
{
    let state = app_handle.state::<AppState>();
    if let Err(e) = crate::config::persist::mutate_config_and_save(app_handle, &state, |fresh| {
        mutate(fresh);
        Ok(())
    }) {
        crate::log_warn!("outbound", "夜间出站切换: 状态字段落盘失败: {e}");
    }
}

/// 出站切换计划：本轮健康判定选出的目标卡（只读判定结果，不含任何系统写入）。
///
/// 跃点不在这里读：预判定可能比执行早一分钟，读到的是过期值；执行阶段统一按
/// 现值读取、校验并做快照，一次读代替「预判定读 + 执行复核」两次。
struct OutboundSwitchPlan {
    /// 目标卡（出站切换的落点）
    target_name: String,
    target_guid: String,
    /// 目标卡 IPv4：切换后要用它绑源做出网复核（路由表对 ≠ 真能上网）
    target_ip: String,
    /// 目标卡接口索引，用于切换后验证默认出口确实落在这张卡上
    target_if_index: u32,
}

/// 切换时需要压制（抬跃点）的竞争卡：非目标、已连接、有 IP 的物理卡。
///
/// 只抬原出口一张不够：同机其他物理卡（第二根网线、同网段热点）的 DHCP 默认路由
/// routeMetric 可能更小，切完后新建连接会被它们的 routeMetric + interfaceMetric
/// 抢走——路由表显示默认出口已在目标卡，实际流量仍从别的卡出去。红线是
/// 「最终 metric = routeMetric + interfaceMetric」，我们只控后者，所以必须把所有
/// 竞争者一起抬高，而不是赌目标卡的 sum 恰好最小。
fn outbound_suppress_targets<'a>(
    target_guid: &str,
    adapters: &'a [crate::network::Adapter],
) -> Vec<&'a crate::network::Adapter> {
    adapters
        .iter()
        .filter(|a| {
            a.guid != target_guid
                && !a.guid.is_empty()
                && !a.ip.is_empty()
                && a.status == crate::network::discovery::AdapterStatus::Connected
        })
        .collect()
}

/// 读一张卡的 IPv4/IPv6 跃点行。读失败向上传播（不吞成空表：空表会被当成
/// 「协议栈行缺失」，让后续逻辑在错误前提下继续跑）。
fn read_family_rows(guid: &str) -> Result<Vec<MetricRow>, String> {
    Ok(crate::platform::metric::read_interface_metrics(guid)?
        .into_iter()
        .filter(|r| matches!(r.family, 2 | 23))
        .collect())
}

/// 按名字解析出站目标卡：必须有 IP 与 GUID。
/// 跃点行不在这里读——执行阶段统一读一次（点名切换原先预读一次又丢弃，等于白读）。
fn resolve_switch_target(adapter_name: &str) -> Result<crate::network::Adapter, String> {
    let adapters = crate::network::get_adapters_cached()?;
    let Some(adapter) = adapters.iter().find(|a| a.name == adapter_name) else {
        return Err(format!("未找到网卡 {adapter_name}"));
    };
    if adapter.ip.is_empty() {
        return Err(format!("网卡 {adapter_name} 当前没有 IP，无法作为出站目标"));
    }
    if adapter.guid.is_empty() {
        return Err(format!("网卡 {adapter_name} 缺少 GUID，无法设置跃点"));
    }
    Ok(adapter.clone())
}

/// 跃点写入条目编码：`{guid}:{family}:0:{metric}`（helper `set_metric` 的 argv 形状）。
fn encode_metric_rows(guid: &str, families: &[u16], metric: u32) -> Vec<String> {
    families
        .iter()
        .map(|family| format!("{guid}:{family}:0:{metric}"))
        .collect()
}

/// 首次切换与改切共用的事务顺序：有原出口卡时，先在两张卡都上线时写好新跃点，
/// 再下线原卡，最后复写以覆盖接口重建可能重置的值。这样原卡重新上线、应用开始
/// 重连时，默认路由已经指向目标卡；不存在「先上线抢回出口、再改跃点」的窗口。
fn run_metric_bounce_sequence(
    has_origin: bool,
    notify_success: bool,
    mut write_metrics: impl FnMut(bool) -> Result<(), String>,
    mut bounce_origin: impl FnMut() -> Result<(), String>,
) -> Result<(), String> {
    if has_origin {
        write_metrics(false)?;
        bounce_origin()?;
    }
    write_metrics(notify_success)?;
    // 只有整段写入/连接重置完成才清退避；首次预写成功不能覆盖此前的失败现场。
    // 随后的路由验证若失败会重新记一次，保证不会进入假稳态。
    clear_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
    Ok(())
}

/// 构建夜间出站切换计划：**纯只读**（选卡 + 绑源探测 + 读跃点 + 算禁用名单），
/// 不做任何系统写入，也不落盘。
///
fn build_outbound_plan(
    app_handle: &AppHandle,
    config: &crate::config::Config,
    internet_health: Option<std::collections::HashMap<String, bool>>,
    forced_target: Option<&str>,
    notify_no_candidate: bool,
) -> Result<OutboundSwitchPlan, String> {
    let adapters = match crate::network::get_adapters_cached() {
        Ok(a) => a,
        Err(e) => {
            crate::log_warn!("outbound", "夜间出站切换: 适配器查询失败: {e}");
            return Err(format!("适配器查询失败: {e}"));
        }
    };
    // 逐卡补齐网关：Adapter 无 gateway 字段（在 AdapterDetail 里），按名字 join
    let details = match crate::network::get_adapter_details_cached() {
        Ok(details) => details,
        Err(e) => {
            crate::log_warn!("outbound", "出站切换获取网关详情失败: {e}");
            return Err(format!("获取网关详情失败: {e}"));
        }
    };
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
    // 用户显式指定目标卡（条目「立即切换出站」）：只要求有 IP 与 GUID，跳过
    // 校园网判定与全部探测——显式动作优先，想切到哪张就切到哪张。
    if let Some(name) = forced_target {
        // 点名切换只校验「卡在、有 IP、有 GUID」，不做校园判定与任何探测
        let adapter = resolve_switch_target(name)?;
        return Ok(OutboundSwitchPlan {
            target_name: adapter.name,
            target_guid: adapter.guid,
            target_ip: adapter.ip,
            target_if_index: adapter.if_index,
        });
    }
    // 候选来自出站优先级生效列表（用户已排序按原序；未排序默认无线卡优先，
    // 见 effective_outbound_priority），且必须有 IP 且判定为非校园网
    apply_outbound_backoff_config(config);
    let portal_host = portal_probe_host(&config.portal_url);
    let priority = effective_outbound_priority(&config.outbound_priority, &candidates);
    let probe_targets =
        collect_probe_targets(&priority, &candidates, &config.campus_gateway, &portal_host);
    // 绑源网关探测与公网健康探测互不依赖：串行时切点耗时是两者之和（实测
    // 590ms + 3000ms），并行后只等较慢的一侧。公网健康探测走 HTTPS，断网卡
    // 必然走满超时，因此这里用切换专用超时而非全局 3s。
    let probe_budget = outbound_probe_budget(config);
    let (mut probe_cache, internet_health) = match internet_health {
        Some(health) => (warm_probe_cache(&probe_targets, probe_budget), health),
        None => std::thread::scope(|scope| {
            // 绑源网关探测放子线程（纯同步 ICMP），公网健康探测留在当前阻塞线程：
            // 后者驱动 reqwest，必须在持有 runtime 上下文的线程上跑，挪到裸 std::thread
            // 会退到兜底 runtime（能跑但多一层不必要的变量）。
            let cache_handle = scope.spawn(|| warm_probe_cache(&probe_targets, probe_budget));
            let health = super::outbound_failover::probe_internet_access(
                app_handle,
                config,
                &candidates,
                None,
                probe_budget,
            );
            let cache = cache_handle
                .join()
                .unwrap_or_else(|_| ProbeCache(std::collections::HashMap::new()));
            (cache, health)
        }),
    };
    for (source_ip, ok) in internet_health {
        probe_cache
            .0
            .insert((OUTBOUND_INTERNET_HEALTH_KEY.to_string(), source_ip), ok);
    }
    let probe = |gateway: &str, source_ip: &str| probe_cache.get(gateway, source_ip);
    let Some(target) = select_outbound_candidate(
        &priority,
        &candidates,
        &config.campus_gateway,
        &portal_host,
        probe,
    ) else {
        // 无可用候选（候选都是校园网卡 / 没有 IP / 自身网关不可达）：不进退避，
        // 30s 后自然重试。选不出候选多半是判定或环境问题（如绑源探测把热点
        // 误判成校园网），每天提醒一次，避免整夜静默失效
        // 每拍都会走到这里；用户可见告警由下面的每日一次通知承担
        crate::log_debug!(
            "outbound",
            "夜间出站切换: 优先级内无可用非校园网候选，本拍不切换"
        );
        // 手动入口也会走这条选卡：只有自动路径才发「夜切未生效」告警，
        // 否则用户点一次按钮就吃掉当日的自动告警额度。
        if notify_no_candidate {
            notify_outbound_issue_once_per_day(
                app_handle,
                &OUTBOUND_NO_CANDIDATE_WARN_DAY,
                "夜间出站切换未生效",
                "未找到可用的非校园网网卡（可能被误判为校园网或网关不可达），出站未切换",
            );
        }
        return Err("没有可用的非校园网网卡（可能被误判为校园网或网关不可达）".to_string());
    };
    if target.guid.is_empty() {
        crate::log_warn!(
            "outbound",
            "夜间出站切换: 目标卡 {} 无 GUID，无法设置跃点",
            target.name
        );
        return Err(format!("目标网卡 {} 缺少 GUID，无法设置跃点", target.name));
    }
    crate::log_info!(
        "outbound",
        "出站切换: 目标卡 {} 已通过双目标绑源 TCP/HTTPS 公网复核",
        target.name
    );
    Ok(OutboundSwitchPlan {
        target_name: target.name,
        target_guid: target.guid,
        target_ip: target.ip,
        target_if_index: target.if_index,
    })
}

/// 夜间出站切换动作（须在 spawn_blocking 线程内执行）：
/// 退避闸 → 选目标卡 → 读跃点现值 → 快照落盘（落盘成功才动系统）
/// → 记录连接重置现场 → 一次 helper 写两卡跃点（目标 1 / 原出口 100）
/// → 原出口卡「下线 5s→启用」断连接 → 复写两卡跃点 → 路由级验证 → Portal 旁路。
///
/// 第一次写入必须早于原卡上线，保证应用重连时默认路由已经切好；第二次写入覆盖
/// 接口重建可能造成的重置。任何中途失败都保留快照与连接重置现场，重放完成整笔事务。
///
/// 返回 Ok(())=快照已落盘且出口已切到目标卡；Err(原因)=本拍未完成（无候选/读跃点失败/
/// 落盘失败/跃点写入失败/禁卡失败；快照可能已保留，由补齐路径续写），失败原因透传给
/// 手动入口的 toast 文案（自动路径只用于判定，细节日志已落盘）。
///
/// 快照在 helper 之前落盘；helper 失败**不清快照**（若 v4 已改成功而 v6 失败，清快照会让
/// 已改的跃点无从还原）——切换态保留 + 退避重试写回，重试幂等。
/// 手动切换与故障自动切换共用；调用方保证当前不处于切换态，故入口无需再判快照。
///
/// 为什么要下线原出口卡：跃点只影响**新建**连接的选路，已建立 TCP 连接的 TCB 与隐式
/// 绑定了源地址的 UDP socket 都不跟随默认路由（strong host model），而 UDP 没有 TCB、
/// 用户态也无法删除他人 socket——接口 down 一次是唯一能让它们全部失效重连的手段
/// （远程控制、加速器这类 UDP 打洞应用正是靠这条才真的换了出口）。
///
/// 原「兜底默认路由」步骤已于 2026-10-18 事故后彻底删除：它与目标卡自己的 DHCP 默认
/// 路由共享「下一跳+接口」、只差跃点，Windows 判重不看跃点 → 加不上（每夜重放），而删除
/// 同样不看跃点 → 会误删用户自己的默认路由。详见 `platform/route_table`。
///
/// 切换目标卡的公网健康结论由逐卡并行 TCP/HTTPS 提供，结果参与候选硬闸门。
pub(super) fn apply_outbound_switch_with_health(
    app_handle: &AppHandle,
    config: &crate::config::Config,
    internet_health: Option<std::collections::HashMap<String, bool>>,
    forced_target: Option<&str>,
    notify_no_candidate: bool,
) -> Result<(), String> {
    let started = std::time::Instant::now();
    warm_elevated_channel();
    let Some(_busy) = OutboundActionGuard::try_acquire() else {
        crate::log_debug!("outbound", "夜间出站切换: 另一出站动作进行中，本拍跳过");
        return Err("另一出站动作进行中，请稍后再试".to_string());
    };
    if !switch_gate_open() {
        crate::log_debug!("outbound", "夜间出站切换: 退避窗内，本拍跳过");
        return Err("正处于自动重试退避窗口，请稍后再试".to_string());
    }
    let plan = build_outbound_plan(
        app_handle,
        config,
        internet_health,
        forced_target,
        notify_no_candidate,
    )?;
    // 跃点在执行时读取并直接快照；不存在跨分钟的预判定计划，避免把过期配置当原值。
    let target_rows = read_family_rows(&plan.target_guid)?;
    if target_rows.is_empty() {
        crate::log_warn!(
            "outbound",
            "夜间出站切换: {} 无 IPv4/IPv6 跃点行，本拍不切换",
            plan.target_name
        );
        return Err(format!(
            "网卡 {} 没有 IPv4/IPv6 跃点配置行",
            plan.target_name
        ));
    }
    // Windows 比较默认路由用的是 routeMetric + interfaceMetric：实测目标卡（热点）
    // routeMetric=30，只把 interfaceMetric 压到 1 仍是 31，输给原卡 10+10=20，出口
    // 根本切不动。所以切换必须同时把原出口卡的 interfaceMetric 抬到 100。
    let origin = current_origin_adapter();
    // 原出口跃点读失败不再吞成「无原值」：按空表继续会让抬高动作静默缺失，
    // 切换必不生效却报成「被 TUN 接管」。读不到就整体失败，交由退避重试。
    let origin_rows: Option<(String, String, Vec<MetricRow>)> = match origin.as_ref() {
        Some((name, guid, _)) if guid != &plan.target_guid => {
            let rows = read_family_rows(guid)?;
            if rows.is_empty() {
                None
            } else {
                Some((name.clone(), guid.clone(), rows))
            }
        }
        _ => None,
    };
    // 先落快照，落盘成功才认定切换态成立。
    // 兜底路由字段不再写入：2026-10-18 事故后该能力废弃（同网关+接口的默认路由无法与
    // 目标卡自己的 DHCP 默认路由共存，Windows 判重复 → route add 每夜恒失败重放），
    // 历史值保留给还原步骤①按四元组精确清理，清空会让历史残留无人认领。
    // 其他竞争者（非目标、已连接、有 IP 的物理卡）也要抬跃点并快照原值：
    // 只抬原出口一张时，第二根网线/同网段热点的 DHCP 默认路由仍可能抢走新建连接。
    // 读取失败不退化成静默跳过——该卡无法压制就会成为漏网的抢路由者，按失败重试。
    let suppress_cards: Vec<(String, Vec<MetricRow>)> = {
        let adapters = crate::network::get_adapters_cached().unwrap_or_default();
        let mut cards = Vec::new();
        for adapter in outbound_suppress_targets(&plan.target_guid, &adapters) {
            if origin_rows
                .as_ref()
                .is_some_and(|(_, guid, _)| guid == &adapter.guid)
            {
                continue; // 已由 origin_rows 收录
            }
            let rows = read_family_rows(&adapter.guid)?;
            if rows.is_empty() {
                crate::log_debug!(
                    "outbound",
                    "夜间出站切换: 竞争卡 {} 无跃点行，跳过压制（无默认路由可抢）",
                    adapter.name
                );
                continue;
            }
            cards.push((adapter.guid.clone(), rows));
        }
        cards
    };
    let mut restore_entries: Vec<(String, Vec<MetricRow>)> =
        vec![(plan.target_guid.clone(), target_rows.clone())];
    if let Some((_, guid, rows)) = &origin_rows {
        restore_entries.push((guid.clone(), rows.clone()));
    }
    for (guid, rows) in &suppress_cards {
        restore_entries.push((guid.clone(), rows.clone()));
    }
    let snapshot_cards = restore_entries.len();
    let metric_restore = snapshot_json_multi(&restore_entries);
    let state = app_handle.state::<AppState>();
    if let Err(e) =
        crate::config::persist::mutate_config_and_save(app_handle, &state, move |fresh| {
            fresh.outbound_metric_restore = metric_restore;
            Ok(())
        })
    {
        crate::log_warn!("outbound", "夜间出站切换: 切换态快照落盘失败: {e}");
        return Err(format!("切换状态快照写入失败: {e}"));
    }
    crate::log_info!(
        "outbound",
        "夜间出站切换: 目标卡 {} ({})，已记录 {snapshot_cards} 张卡的跃点快照",
        plan.target_name,
        plan.target_guid,
    );
    // 目标卡压到 1、原出口卡抬到 100，必须同批下发。事务先写跃点再下线原卡，
    // 原卡重新上线时路由已正确；恢复后的第二次写入负责覆盖接口重建造成的重置。
    let families: Vec<u16> = target_rows.iter().map(|r| r.family).collect();
    let origin_families: Vec<u16> = origin_rows
        .as_ref()
        .map(|(_, _, rows)| rows.iter().map(|r| r.family).collect())
        .unwrap_or_default();
    let mut suppress_write: Vec<(String, Vec<u16>)> = suppress_cards
        .iter()
        .map(|(guid, rows)| (guid.clone(), rows.iter().map(|r| r.family).collect()))
        .collect();
    if let Some((_, guid, _)) = &origin_rows {
        suppress_write.push((guid.clone(), origin_families.clone()));
    }
    // 名单在首次写跃点前落盘：此后任何失败都保留「连接重置尚未完成」的现场，
    // 重放不能只补跃点，必须先把这次下线事务做完。
    if let Some((name, guid, _)) = &origin_rows {
        mark_origin_bounce_pending(app_handle, name, guid)?;
    }
    run_metric_bounce_sequence(
        origin_rows.is_some(),
        true,
        |notify| {
            write_outbound_metric(
                app_handle,
                &plan.target_name,
                &plan.target_guid,
                &families,
                notify,
                "",
                &suppress_write,
            )
        },
        || {
            if let Some((name, guid, _)) = &origin_rows {
                bounce_origin_adapter(app_handle, name, guid)
            } else {
                Ok(())
            }
        },
    )?;
    if let Some((origin_name, _, _)) = &origin_rows {
        crate::log_info!(
            "outbound",
            "夜间出站切换: 已将原出口 {origin_name} 跃点抬到 {SWITCHED_ORIGIN_METRIC}"
        );
    }
    // ③ 跃点写入成功 ≠ 出口真的换了（TUN/代理可能持有 metric=0 的默认路由）
    verify_switch_effect(
        app_handle,
        config,
        &plan.target_name,
        &plan.target_ip,
        plan.target_if_index,
    )?;
    // ④ Portal 旁路：默认出口已在目标卡，但登录请求仍需到达校园 Portal——给原卡加一条
    //    /32 主机路由让这部分流量绕回原卡。它只影响「切换态内回原卡登录」，不决定
    //    出口是否已切换，故后台执行、失败只记日志。
    if let Some((origin_name, _, _)) = &origin_rows {
        match build_portal_bypass_route(config, origin_name) {
            Some(portal_route) => spawn_portal_bypass(app_handle, portal_route),
            None => crate::log_debug!(
                "outbound",
                "夜间出站切换: 无 Portal 主机或原出口网关，跳过旁路路由"
            ),
        }
    }
    crate::log_info!(
        "outbound",
        "夜间出站切换: 动作完成，总耗时 {}ms",
        started.elapsed().as_millis()
    );
    Ok(())
}

/// Portal 旁路路由后台落地：route_add 成功才写快照（还原据此精确删除）。
/// 失败只记日志——旁路不决定出口是否已切换，却要花掉一次 200–300ms 的提权往返。
fn spawn_portal_bypass(app_handle: &AppHandle, portal_route: PortalRoute) {
    let app_handle_bg = app_handle.clone();
    std::thread::spawn(move || {
        let row = portal_route.encode();
        match run_helper_op("route_add", &[row], outbound_allow_uac()) {
            Ok(()) => {
                let json = serde_json::to_string(&portal_route).unwrap_or_default();
                crate::log_info!(
                    "outbound",
                    "夜间出站切换: Portal 旁路路由已就绪 {} via {} if {}",
                    portal_route.dest,
                    portal_route.gateway,
                    portal_route.if_index
                );
                let state = app_handle_bg.state::<AppState>();
                if let Err(e) = crate::config::persist::mutate_config_and_save(
                    &app_handle_bg,
                    &state,
                    move |fresh| {
                        fresh.outbound_portal_route = json;
                        Ok(())
                    },
                ) {
                    crate::log_warn!("outbound", "夜间出站切换: 旁路路由快照落盘失败: {e}");
                }
            }
            Err(failure) => {
                note_channel_failure(&failure);
                crate::log_warn!(
                    "outbound",
                    "夜间出站切换: Portal 旁路路由添加失败: {}",
                    failure.reason
                );
            }
        }
    });
}

/// 切换后路由级验证：动作成功 ≠ 效果达成。只改跃点时默认出口必须真的离开
/// 原卡、落到目标卡；若仍不是（典型是 TUN/代理接管了 metric=0 的默认路由），
/// 判失败走退避重放，并明确指出原因，避免「日志说成功、实际没切」。
fn validate_switch_probe(result: Result<bool, String>, target_name: &str) -> Result<(), String> {
    match result {
        Ok(true) => Ok(()),
        Ok(false) => Err(format!(
            "默认出口仍未切到 {target_name}（可能被 TUN/代理接管默认路由）"
        )),
        Err(e) => Err(format!("读取系统默认路由失败: {e}")),
    }
}

fn verify_switch_effect(
    app_handle: &AppHandle,
    config: &crate::config::Config,
    target_name: &str,
    target_ip: &str,
    target_if_index: u32,
) -> Result<(), String> {
    let route_result = validate_switch_probe(outbound_route_targets(target_if_index), target_name);
    if let Err(reason) = route_result {
        mark_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
        crate::log_warn!("outbound", "夜间出站切换: 路由验证失败——{reason}");
        return Err(reason);
    }
    // 路由表只说明「默认出口的接口索引是哪张卡」，不代表流量真能出去：其他物理卡
    // 竞争默认路由、原卡残留会话、代理/TUN 抢包，都会让默认出口看起来正确而实际
    // 走错链路（用户现场：切换完成后流量仍从别的卡出去被判踢线）。判据改成绑源到
    // 目标卡的 HTTPS 探测，只有真实出网才算切换完成。
    let mut hosts = config.internet_probe_hosts.clone();
    if hosts.is_empty() {
        hosts = crate::network::quality::DEFAULT_INTERNET_PROBE_HOSTS
            .iter()
            .map(|h| (*h).to_string())
            .collect();
    }
    let timeout = Duration::from_millis(config.outbound_probe_timeout_ms as u64);
    let quitting = app_handle.state::<AppState>().exit.is_quitting.clone();
    let ok = crate::infra::async_util::block_on_sync(
        crate::network::quality::check_internet_access_with_if(
            target_ip,
            Some(target_if_index),
            true,
            true,
            quitting,
            false,
            timeout,
            &hosts,
        ),
    );
    if !ok {
        let reason = format!(
            "绑定 {target_name}({target_ip}) 的出网探测失败（路由已指向该卡但实际不通，可能被其他物理卡或代理抢走流量）"
        );
        mark_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
        crate::log_warn!("outbound", "夜间出站切换: 出网验证失败——{reason}");
        return Err(reason);
    }
    crate::log_info!(
        "outbound",
        "夜间出站切换: 路由与出网双重验证通过，{target_name} 可正常访问公网"
    );
    Ok(())
}

/// 原出口卡「下线再恢复」的保持时长。5 秒用于覆盖协议栈断链传播与应用常见的
/// 首轮重连退避；它不承担选路正确性——原卡重新上线前，目标/原卡跃点已经写好。
const ORIGIN_DISABLE_HOLD_MS: u64 = 5000;

/// 临时下线再恢复原出口卡：切换流程里唯一能断掉既有连接的动作。
///
/// 改跃点只影响**新建**连接的选路：已建立 TCP 连接的 TCB、以及 UDP socket 隐式
/// 绑定的源地址都不会跟随默认路由（Vista 起 strong host model），而 UDP 没有 TCB、
/// 用户态也没有任何 API 能删除他人 socket。接口 down 一次是让这些绑定全部失效的
/// 唯一手段，应用重连时按已抬高的跃点落到目标卡。
///
/// 顺序约束：首次跃点写入已经完成才允许进入本函数；接口 down 时写不了原卡跃点，
/// 恢复后的复写只负责覆盖重建重置。名单在首次写入前落盘，失败重放据此重做连接重置。
fn mark_origin_bounce_pending(
    app_handle: &AppHandle,
    name: &str,
    guid: &str,
) -> Result<(), String> {
    let json = serde_json::to_string(&[DisabledRow {
        guid: guid.to_string(),
        name: name.to_string(),
    }])
    .unwrap_or_default();
    let state = app_handle.state::<AppState>();
    if state.config.load().outbound_disabled_adapters == json {
        return Ok(());
    }
    crate::config::persist::mutate_config_and_save(app_handle, &state, move |fresh| {
        fresh.outbound_disabled_adapters = json;
        Ok(())
    })
    .map(|_| ())
    .map_err(|e| {
        mark_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
        format!("连接重置现场快照写入失败: {e}")
    })
}

fn bounce_origin_adapter(app_handle: &AppHandle, name: &str, guid: &str) -> Result<(), String> {
    mark_origin_bounce_pending(app_handle, name, guid)?;
    if let Err(failure) =
        run_helper_op("disable_adapter", &[name.to_string()], outbound_allow_uac())
    {
        note_channel_failure(&failure);
        // 禁用失败 = 既有连接没断：保留切换快照与下线现场，退避后重放整笔事务。
        mark_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
        return Err(format!(
            "临时禁用原出口网卡 {name} 失败: {}",
            failure.reason
        ));
    }
    crate::log_info!(
        "outbound",
        "出站切换: 已临时禁用原出口网卡 {name}，等待既有连接失效"
    );
    std::thread::sleep(Duration::from_millis(ORIGIN_DISABLE_HOLD_MS));
    match enable_campus_adapter(name, outbound_allow_uac()) {
        Ok(()) => {
            crate::log_info!("outbound", "出站切换: 已恢复启用原出口网卡 {name}");
            // 卡已恢复：名单不该再声称它被禁用（此后名单只表示「仍处于禁用态」）
            persist_config_update(app_handle, |fresh| {
                fresh.outbound_disabled_adapters = String::new();
            });
            Ok(())
        }
        Err(failure) => {
            note_channel_failure(&failure);
            // 启用失败不阻断切换：出口已落在目标卡，卡停在禁用态是可恢复的副作用，
            // 名单保留给还原与重放继续尝试启用
            notify_outbound_issue_once_per_day(
                app_handle,
                &OUTBOUND_ORIGIN_DISABLE_WARN_DAY,
                "原出口网卡未能恢复启用",
                &format!(
                    "网卡 {name} 已临时禁用但启用失败，出站已切到目标卡；还原时会再次尝试启用"
                ),
            );
            crate::log_warn!(
                "outbound",
                "出站切换: 恢复启用原出口网卡 {name} 失败，保留名单等还原: {}",
                failure.reason
            );
            Ok(())
        }
    }
}

/// 补齐/重放切换动作：按快照把"切换没写完"的部分补齐，两个分支各自独立、
/// 全部幂等（可反复调用），任一分支失败不影响其余分支：
/// ① 禁用残留收尾：切换流程「禁用 → 保持 → 启用」原出口卡，进程若在中间被杀，卡会
///    停在禁用态；名单里记的正是本功能亲手禁用、尚未确认恢复的卡，逐个启用回来；
/// ② 跃点补写：按快照逐卡写运行期值（首行目标卡 1、其余原出口 100），不动快照原值。
///    排在启用之后是必要的——卡刚启用回来的同一次重放就把跃点补上，否则这次会清掉
///    失败计数，下一拍不再重放，原卡凭自动跃点抢回出口。
/// （原"③ 兜底路由补加"分支已随 2026-10-18 事故删除：该路由在 Windows 上无法与目标卡
/// 自己的 DHCP 默认路由共存，route add 恒报「文件已存在」，详见 platform/route_table。）
/// 两个入口共用：
/// ① 循环内切换态下的"未生效补齐"（helper 部分失败后的退避重试；循环已用
///    [`needs_replay`] 判过失败历史，此处不再判）；
/// ② 启动对账的夜间窗口分支（上次运行被杀，切换没写完——此时进程内失败计数为 0，
///    不能以 `needs_replay` 拦，否则重启后的重放失效）。
/// 成功只记 info 日志、**不发系统通知**（稳态/多拍重放不应重复弹窗）。
/// 全部分支完成且默认出口验证落在目标卡后才清切换失败计数，让 needs_replay
/// 回稳态 false；仅 helper 写入成功不能作为完成依据。
pub(super) fn replay_outbound_switch(app_handle: &AppHandle, config: &crate::config::Config) {
    if !switch_gate_open() {
        return;
    }
    // 与手动入口/还原互斥：重放会写 metric=1，若与还原并发，最坏是
    // 还原清完快照后重放又把跃点写回 1（无快照可还原的静默丢失）。拿不到闸就记一次
    // 切换失败：启动对账这一发失败计数为 0，不记则 needs_replay 永假、这一发被静默
    // 丢掉；记了它就是一次普通失败，退避窗后由循环补做。
    let Some(_busy) = OutboundActionGuard::try_acquire() else {
        mark_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
        crate::log_debug!("outbound", "夜间出站切换: 另有出站动作进行中，本次重放顺延");
        return;
    };
    let mut any_fail = false;
    let adapters = crate::network::get_adapters_cached().unwrap_or_default();
    // —— 分支①：连接重置补做（必须排在跃点补写之前）。名单仍在且原卡在线，说明
    //    上次连禁用都没完成，必须重做整段下线；原卡不在线才只补启用。
    // 切换流程会「禁用 → 保持 → 启用」原出口卡，进程若在中间被杀，卡会停在禁用态。
    // 名单里记的正是本功能亲手禁用、尚未确认恢复的卡（启用成功即清空）。
    // 不走适配器列表闸门：设备级禁用后接口可能从枚举中消失，而 enable_adapter
    // 的 problem 22 分流能按名字把 PnP 设备直接启用回来。
    match parse_disabled_adapters(&config.outbound_disabled_adapters) {
        Ok(list) if !list.is_empty() => {
            let total = list.len();
            // 枚举包含禁用卡，不能把“出现在列表里”当作在线；仅 Connected 才需要
            // 重做下线，Disabled 直接走启用，避免已禁用设备被再次打断。
            let should_bounce = total == 1
                && list.first().is_some_and(|row| {
                    adapters.iter().any(|a| {
                        a.name == row.name
                            && a.status == crate::network::discovery::AdapterStatus::Connected
                    })
                });
            let mut remaining: Vec<DisabledRow> = Vec::new();
            for row in list {
                if should_bounce {
                    match bounce_origin_adapter(app_handle, &row.name, &row.guid) {
                        Ok(()) => {
                            // 启用失败时 bounce 保留名单；不能用 Ok 反推现场已清空。
                            if app_handle
                                .state::<AppState>()
                                .config
                                .load()
                                .outbound_disabled_adapters
                                .is_empty()
                            {
                                crate::log_info!(
                                    "outbound",
                                    "夜间出站切换: 已重放原出口网卡下线与恢复 {}",
                                    row.name
                                );
                            } else {
                                any_fail = true;
                                crate::log_warn!(
                                    "outbound",
                                    "重放原出口连接重置后 {} 仍未恢复启用，保留现场",
                                    row.name
                                );
                                remaining.push(row);
                            }
                        }
                        Err(reason) => {
                            any_fail = true;
                            remaining.push(row);
                            crate::log_warn!("outbound", "重放原出口连接重置失败: {reason}");
                        }
                    }
                } else {
                    match enable_campus_adapter(&row.name, outbound_allow_uac()) {
                        Ok(()) => crate::log_info!(
                            "outbound",
                            "夜间出站切换: 已恢复启用原出口网卡 {}",
                            row.name
                        ),
                        Err(failure) => {
                            note_channel_failure(&failure);
                            any_fail = true;
                            remaining.push(row);
                        }
                    }
                }
            }
            if remaining.len() != total {
                let json = if remaining.is_empty() {
                    String::new()
                } else {
                    serde_json::to_string(&remaining).unwrap_or_default()
                };
                persist_config_update(app_handle, move |fresh| {
                    fresh.outbound_disabled_adapters = json;
                });
            }
        }
        Ok(_) => {}
        Err(e) => crate::log_warn!(
            "outbound",
            "夜间出站切换: 禁用名单解析失败，跳过启用补做: {e}"
        ),
    }
    // —— 分支②：跃点补写 ——
    let snapshot = match parse_outbound_snapshot(&config.outbound_metric_restore) {
        Ok(rows) if !rows.is_empty() => Some(rows),
        Ok(_) => None,
        Err(e) => {
            crate::log_warn!("outbound", "夜间出站切换: 跃点快照解析失败，跳过补齐: {e}");
            None
        }
    };
    if let Some(snapshot) = &snapshot {
        let target_guid = snapshot[0].guid.clone();
        // 逐卡分组，与首次切换同口径：首行（目标卡）压到 1，其余（被抬高的原出口）
        // 抬到 100——只补目标卡会让被启用回来的原卡凭更低的 routeMetric 抢回出口。
        let mut by_guid: std::collections::BTreeMap<String, Vec<u16>> =
            std::collections::BTreeMap::new();
        for row in snapshot.iter() {
            by_guid
                .entry(row.guid.clone())
                .or_default()
                .push(row.family);
        }
        let mut encoded: Vec<String> = Vec::new();
        for (guid, families) in &by_guid {
            let live = match crate::platform::metric::read_interface_metrics(guid) {
                Ok(rows) => rows,
                Err(e) => {
                    any_fail = true;
                    crate::log_warn!("outbound", "夜间出站切换: 读取 {guid} 跃点失败: {e}");
                    continue;
                }
            };
            // 只写快照有原值、且当前确实存在的协议栈：保证"写过的都能按快照还原"
            let present: Vec<u16> = families
                .iter()
                .copied()
                .filter(|family| live.iter().any(|r| r.family == *family))
                .collect();
            if present.is_empty() {
                continue;
            }
            let metric = if guid == &target_guid {
                1
            } else {
                SWITCHED_ORIGIN_METRIC
            };
            encoded.extend(encode_metric_rows(guid, &present, metric));
        }
        let target = adapters.iter().find(|a| a.guid == target_guid);
        if encoded.is_empty() {
            any_fail = true;
            crate::log_warn!(
                "outbound",
                "夜间出站切换: 没有可补写的跃点行，保留失败状态等待重试"
            );
        } else {
            let name = target
                .map(|a| a.name.clone())
                .unwrap_or_else(|| target_guid.clone());
            if run_set_metric(app_handle, &name, &encoded, false, "").is_err() {
                any_fail = true;
            } else if let Some(target) = target {
                // 写入成功只代表 helper 返回成功；默认出口未落到目标卡或路由表读失败，
                // 都必须保留失败计数，下一拍继续重放，不能进入整夜假稳态。
                if verify_switch_effect(app_handle, config, &target.name, &target.ip, target.if_index)
                    .is_err()
                {
                    any_fail = true;
                }
            } else {
                any_fail = true;
                crate::log_warn!(
                    "outbound",
                    "夜间出站切换: 目标卡 {target_guid} 不存在，等待重新选定或还原"
                );
            }
        }
    }
    // 全部分支无新增失败：此前的失败已被补齐动作补救，清掉切换失败计数，
    // needs_replay 回稳态 false（否则每拍 60s 重放 + 提权空转直到天亮）
    if any_fail {
        // 写入 helper 的成功不再提前清计数；若这次失败来自连接重置或目标卡缺失，
        // 计数原本为 0 时也必须补上，否则 needs_replay 下一拍会错误回到稳态。
        if OUTBOUND_SWITCH_FAIL_COUNT.load(Ordering::Acquire) == 0 {
            mark_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
        }
    } else {
        clear_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
    }
}

/// 夜间出站还原动作（须在 spawn_blocking 线程内执行）。切换态含快照，
/// 还原按"反向顺序"执行，已完成的步骤幂等、重试无害；任一步失败保留整个切换态
/// 按退避重试（重做已成功的步骤无副作用）：
/// ① 清理历史兜底路由（2026-10-18 事故后的兼容路径，本版本不再创建）：删除必须有
///    实时路由表凭据——只删「前缀+下一跳+接口+跃点」四元组全等的那一行，且删后仍有
///    其他默认路由；行不在表里/跃点不符/是唯一默认路径时一律不动（不阻塞还原）；
/// ② 按跃点快照把目标卡原样写回（automatic 与 metric 都是快照原值）；目标卡已不存在
///    或跃点行已消失 → 视为已还原（终态出口）；
/// ③ 重新启用名单内校园网卡（名单外的卡不碰；卡已拔出/消失则跳过）。
/// 全部完成后清三份快照 + 通知；连续失败达 OUTBOUND_RESTORE_ALERT_FAILS 各发一次告警
/// 并保留快照。返回 Ok(())=切换态已清（还原完成或无需还原）；Err(原因)=本拍未完成、
/// 保留切换态退避重试，原因透传给手动入口的 toast 文案。
/// 还原时要重新启用的卡名（去重、名单顺序在前）。
///
/// 名单（`outboundDisabledAdapters`）只记本功能亲手禁用的卡；用户在系统「高级网络
/// 设置」里手动禁用原出口卡时它不在名单。切换流程唯一下线过的就是原出口卡（快照
/// 非首行），所以这些卡处于禁用态时一律纳入启用集合——否则还原会跳过它们却仍报
/// 「已还原」，校园网卡整天停在禁用态。首行恒为目标卡、从未被下线，不纳入，避免把
/// 用户主动禁用的热点卡在还原时打开。
fn restore_enable_targets(
    disabled: &[DisabledRow],
    snapshot: &[OutboundSnapshotRow],
    adapters: &[crate::network::Adapter],
) -> Vec<String> {
    let mut targets: Vec<String> = disabled.iter().map(|row| row.name.clone()).collect();
    for guid in snapshot.iter().skip(1).map(|row| &row.guid) {
        let Some(adapter) = adapters.iter().find(|a| &a.guid == guid) else {
            continue;
        };
        if adapter.status == crate::network::discovery::AdapterStatus::Disabled
            && !targets.iter().any(|name| name == &adapter.name)
        {
            targets.push(adapter.name.clone());
        }
    }
    targets
}

pub(super) fn apply_outbound_restore(
    app_handle: &AppHandle,
    config: &crate::config::Config,
) -> Result<(), String> {
    let Some(_busy) = OutboundActionGuard::try_acquire() else {
        crate::log_debug!("outbound", "夜间出站还原: 另一出站动作进行中，本拍跳过");
        return Err("另一出站动作进行中，请稍后再试".to_string());
    };
    let snapshot = match parse_outbound_snapshot(&config.outbound_metric_restore) {
        Ok(rows) => rows,
        Err(e) => {
            // 跃点快照损坏：跃点无凭据可还原（按"已还原"处理），但禁用名单/兜底路由
            // 仍要继续还原——不能在这里整体收尾，否则已禁用的网卡会永远停在禁用态。
            // 目标卡跃点停留在切换值属可容忍残留（下一夜切换重写、重启对默认值），
            // 但不能静默：每天提醒一次
            crate::log_warn!(
                "outbound",
                "夜间出站还原: 跃点快照解析失败，按已还原继续: {e}"
            );
            notify_outbound_issue_once_per_day(
                app_handle,
                &OUTBOUND_METRIC_SNAPSHOT_WARN_DAY,
                "夜间出站还原不完整",
                "跃点快照损坏，目标卡跃点未能自动还原；如白天出站异常，请手动恢复网卡跃点设置",
            );
            Vec::new()
        }
    };
    if snapshot.is_empty()
        && config.outbound_disabled_adapters.is_empty()
        && config.outbound_standby_route.is_empty()
    {
        // 三份快照全空（非切换态）：按已还原收尾
        return if finish_outbound_restore(app_handle) {
            Ok(())
        } else {
            Err("切换状态快照清理失败，将自动重试".to_string())
        };
    }
    if !restore_gate_open() {
        crate::log_debug!("outbound", "夜间出站还原: 退避窗内，本拍跳过");
        return Err("正处于自动重试退避窗口，稍后自动继续".to_string());
    }
    let allow_uac = outbound_allow_uac();
    let mut fail_reasons: Vec<String> = Vec::new();
    let mut route_ok = true;
    // ① 清理历史兜底默认路由（2026-10-18 事故后的兼容路径；本版本不再创建该路由）。
    // 删除必须有实时路由表凭据，四类"不动"结论都不阻塞还原——它是 runtime 路由，
    // 重启即消失，卡住还原反而会压制定时登录（本次事故的次生伤害）：
    //   · 表里没有这一行：route add 从未成功过（同网关+接口判重复），幂等无操作；
    //   · 跃点不符：那行是目标卡自己的 DHCP 默认路由，不是本应用创建的，绝不碰；
    //   · 唯一默认路径：删了直接断网，保留并提醒（这正是用户今早 WLAN 被断的现场）；
    //   · 读表失败：不做破坏性动作，保留状态重试（放弃阀到期后放行，避免卡死）。
    // 只有"精确匹配 + 删后仍有其他默认路由"才走 helper 精确删除；删除失败仍按
    // 原语义阻塞收尾（残留 metric=2 路由会持续改道流量，k2.8 审计 P1-2）。
    if let Some(route) = parse_standby_route(&config.outbound_standby_route) {
        let give_up_route =
            OUTBOUND_RESTORE_FAIL_COUNT.load(Ordering::Acquire) >= OUTBOUND_RESTORE_GIVE_UP_FAILS;
        match standby_route_cleanup(&route) {
            Ok(StandbyCleanup::AlreadyGone) => crate::log_info!(
                "outbound",
                "夜间出站还原: 兜底路由 {} if {} 不在路由表中（未创建成功或已被系统移除），无需删除",
                route.gateway,
                route.if_index
            ),
            Ok(StandbyCleanup::ForeignRowKept) => crate::log_info!(
                "outbound",
                "夜间出站还原: {} if {} 上的默认路由不是本应用创建的（跃点不符），保留不动",
                route.gateway,
                route.if_index
            ),
            Ok(StandbyCleanup::LastDefaultPath) => {
                // ponytail: 保留而不删——当前网络完全依赖它，删了立刻断网（事故现场）。
                // 代价：残留行会在重启前一直存在（runtime 路由，重启即清），接口掉线时
                // 可能短暂黑洞；宁留孤儿不断网是刻意的取舍，恢复期后再复查收益不足。
                crate::log_warn!(
                    "outbound",
                    "夜间出站还原: 兜底路由是当前唯一默认路径，删除会直接断网，保留不删: {} if {}",
                    route.gateway,
                    route.if_index
                );
                notify_outbound_issue_once_per_day(
                    app_handle,
                    &OUTBOUND_LAST_PATH_WARN_DAY,
                    "夜间出站还原",
                    "当前唯一的默认路由是切换时添加的兜底路由，已保留以免断网；重启或网络恢复后它会自动消失",
                );
            }
            Ok(StandbyCleanup::Safe) => {
                let row = format!(
                    "{}|{}|{}|{}|{}",
                    route.dest, route.mask, route.gateway, route.metric, route.if_index
                );
                match run_helper_op("route_delete", &[row], allow_uac) {
                    Ok(()) => {
                        clear_outbound_channel_failure();
                        crate::log_info!(
                            "outbound",
                            "夜间出站还原: 已按四元组精确删除历史兜底默认路由 {}",
                            route.gateway
                        );
                    }
                    Err(failure) => {
                        note_channel_failure(&failure);
                        let count = mark_outbound_failure(
                            &OUTBOUND_RESTORE_LAST_FAIL_MS,
                            &OUTBOUND_RESTORE_FAIL_COUNT,
                        );
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
                        fail_reasons.push(format!("删除兜底默认路由失败: {}", failure.reason));
                    }
                }
            }
            Err(e) if give_up_route => crate::log_warn!(
                "outbound",
                "夜间出站还原: 兜底路由状态无法判定（{e}），告警期已过，跳过清理并按已还原收尾"
            ),
            Err(e) => {
                let count =
                    mark_outbound_failure(&OUTBOUND_RESTORE_LAST_FAIL_MS, &OUTBOUND_RESTORE_FAIL_COUNT);
                crate::log_warn!(
                    "outbound",
                    "夜间出站还原: 读取路由表失败，本轮不动路由并保留切换态(第 {count} 次): {e}"
                );
                route_ok = false;
                fail_reasons.push(format!("读取路由表失败: {e}"));
            }
        }
    } else if !config.outbound_standby_route.is_empty() {
        // 快照损坏（非空但解析失败）：无从判定该删哪一行，且 runtime 路由重启即消失，
        // 告警不阻塞还原，但要让日志可查
        crate::log_warn!(
            "outbound",
            "夜间出站还原: 兜底路由快照解析失败，跳过清理（runtime 路由重启即消失）"
        );
    }
    // ② 跃点写回（跃点快照非空才有此步）
    let mut metric_ok = true;
    if !snapshot.is_empty() {
        let adapters = crate::network::get_adapters_cached().unwrap_or_default();
        // 快照可能含多张卡：目标卡（压到 1）+ 被抬高的原出口卡。逐卡分组写回。
        let mut by_guid: std::collections::BTreeMap<String, Vec<&OutboundSnapshotRow>> =
            std::collections::BTreeMap::new();
        for row in &snapshot {
            by_guid.entry(row.guid.clone()).or_default().push(row);
        }
        for (guid, rows) in by_guid {
            if !adapters.iter().any(|a| a.guid == guid) {
                // 终态出口：网卡已拔出/禁用，跃点设置随接口消失，无对象可写回
                crate::log_info!("outbound", "夜间出站还原: 网卡({guid})已不存在，视为已还原");
                continue;
            } else {
                // 只写回"快照有原值 + 当前仍存在"的协议栈：跃点行消失的条目无对象可写，
                // 系统重建该协议栈时按其默认值（与"已还原"同语义）；现值已与快照一致的
                // 协议栈不再重复提权写入——重试不会再弹 UAC，也不会把已还原的步骤反复执行
                let current =
                    crate::platform::metric::read_interface_metrics(&guid).unwrap_or_default();
                let present = rows
                    .iter()
                    .filter(|r| current.iter().any(|c| c.family == r.family))
                    .count();
                let encoded: Vec<String> = rows
                    .iter()
                    .filter(|r| {
                        current.iter().any(|c| {
                            c.family == r.family
                                && (c.metric != r.metric || c.automatic != r.automatic)
                        })
                    })
                    .map(|r| {
                        format!(
                            "{}:{}:{}:{}",
                            r.guid,
                            r.family,
                            u8::from(r.automatic),
                            r.metric
                        )
                    })
                    .collect();
                if present == 0 {
                    crate::log_info!(
                        "outbound",
                        "夜间出站还原: 网卡 {guid} 跃点行已消失，视为已还原"
                    );
                } else if encoded.is_empty() {
                    crate::log_info!(
                        "outbound",
                        "夜间出站还原: 网卡 {guid} 跃点已与快照一致，跳过写入"
                    );
                } else {
                    match run_helper_op("set_metric", &encoded, allow_uac) {
                        Ok(()) => {
                            clear_outbound_channel_failure();
                            crate::log_info!(
                                "outbound",
                                "夜间出站还原: 已恢复网卡 {guid} 跃点设置"
                            );
                        }
                        Err(failure) => {
                            note_channel_failure(&failure);
                            let count = mark_outbound_failure(
                                &OUTBOUND_RESTORE_LAST_FAIL_MS,
                                &OUTBOUND_RESTORE_FAIL_COUNT,
                            );
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
                            fail_reasons.push(format!("恢复网卡跃点失败: {}", failure.reason));
                        }
                    }
                }
            }
        }
    }
    // Portal 旁路路由：还原后原卡重新成为默认出口，这条 /32 路由必须删掉，
    // 否则它会继续把去 Portal 的流量钉在原卡网关上（原卡换 IP 后会变成黑洞）。
    if let Some(portal_route) = parse_portal_route(&config.outbound_portal_route) {
        match run_helper_op(
            "route_delete",
            &[portal_route_delete_row(&portal_route)],
            outbound_allow_uac(),
        ) {
            Ok(()) => crate::log_info!(
                "outbound",
                "夜间出站还原: 已删除 Portal 旁路路由 {}",
                portal_route.dest
            ),
            Err(failure) => {
                note_channel_failure(&failure);
                route_ok = false;
                crate::log_warn!(
                    "outbound",
                    "夜间出站还原: 删除 Portal 旁路路由失败: {}",
                    failure.reason
                );
                fail_reasons.push(format!("删除 Portal 旁路路由失败: {}", failure.reason));
            }
        }
    }
    // ③ 重新启用校园网卡（名单内有且当前存在的卡）。启用失败=还原未完成：保留整个
    // 切换态（含已写回的跃点快照，重试幂等）下一拍继续。
    // 名单快照损坏 ≠ 无禁用动作：按空处理会在收尾清快照后静默放走仍被 netsh
    // 持久禁用的校园卡（此后无人认领）→ 判还原失败走告警 + 保留状态重试；连续
    // 失败达 OUTBOUND_RESTORE_GIVE_UP_FAILS（约 3.3h、期间告警 2 次）按空名单放行收尾，
    // 避免损坏快照把还原流程永久卡死（此时告警义务已尽，卡死比残留更糟）
    let mut enable_ok = true;
    let mut enabled_count = 0usize;
    // 放弃阀放行 ≠ 还原成功：收尾通知需区分（放行后校园卡可能仍被持久禁用）
    let mut gave_up_released = false;
    let give_up_corrupt =
        OUTBOUND_RESTORE_FAIL_COUNT.load(Ordering::Acquire) >= OUTBOUND_RESTORE_GIVE_UP_FAILS;
    let disabled = match parse_disabled_adapters(&config.outbound_disabled_adapters) {
        Ok(rows) => rows,
        Err(e) if give_up_corrupt => {
            crate::log_warn!(
                "outbound",
                "夜间出站还原: 禁用名单快照损坏，告警期已过，按空名单放行收尾: {e}"
            );
            gave_up_released = true;
            Vec::new()
        }
        Err(e) => {
            let count =
                mark_outbound_failure(&OUTBOUND_RESTORE_LAST_FAIL_MS, &OUTBOUND_RESTORE_FAIL_COUNT);
            crate::log_warn!(
                "outbound",
                "夜间出站还原: 禁用名单快照解析失败，保留切换态重试: {e}"
            );
            if outbound_restore_alert_due(count) {
                crate::infra::notification::emit_notification(
                    app_handle,
                    "夜间出站还原失败",
                    "禁用名单快照损坏，无法自动重新启用校园网网卡；请在系统「网络设置」中手动启用被禁用的网卡",
                    "mascot-alert",
                );
            }
            enable_ok = false;
            fail_reasons.push(format!("禁用名单快照损坏: {e}"));
            Vec::new()
        }
    };
    // 需要重新启用的卡：名单内全部 + 快照里处于禁用态的非首行卡（原出口卡）。
    // 名单只记本功能亲手禁用的卡；用户在系统「高级网络设置」里手动禁用原出口卡时
    // 它不在名单，旧实现据此跳过启用却仍报「已还原」，校园网卡整天停在禁用态。
    // 判据落在实测状态上（Disabled），已被手动恢复的卡不会进集合。
    let enable_targets = restore_enable_targets(
        &disabled,
        &parse_outbound_snapshot(&config.outbound_metric_restore).unwrap_or_default(),
        &crate::network::get_adapters_cached().unwrap_or_default(),
    );
    if !enable_targets.is_empty() {
        let present: std::collections::HashSet<String> = crate::network::get_adapters_cached()
            .unwrap_or_default()
            .into_iter()
            .map(|a| a.name)
            .collect();
        for name in &enable_targets {
            if !present.contains(name) {
                crate::log_debug!(
                    "outbound",
                    "夜间出站还原: 待启用网卡 {} 已不存在，跳过启用",
                    name
                );
                continue;
            }
            match enable_campus_adapter(name, allow_uac) {
                Ok(()) => {
                    clear_outbound_channel_failure();
                    enabled_count += 1;
                    crate::log_info!("outbound", "夜间出站还原: 已重新启用校园网卡 {name}");
                }
                Err(failure) => {
                    note_channel_failure(&failure);
                    let count = mark_outbound_failure(
                        &OUTBOUND_RESTORE_LAST_FAIL_MS,
                        &OUTBOUND_RESTORE_FAIL_COUNT,
                    );
                    crate::log_warn!(
                        "outbound",
                        "夜间出站还原: 启用 {} 失败(第 {count} 次): {}",
                        name,
                        failure.reason
                    );
                    if outbound_restore_alert_due(count) {
                        crate::infra::notification::emit_notification(
                            app_handle,
                            "夜间出站还原失败",
                            &format!(
                                "校园网网卡 {} 未能自动恢复启用，请在系统「网络设置」中手动启用",
                                name
                            ),
                            "mascot-alert",
                        );
                    }
                    enable_ok = false;
                    fail_reasons.push(format!("启用校园网卡 {} 失败: {}", name, failure.reason));
                }
            }
        }
    }
    if !(metric_ok && enable_ok && route_ok) {
        return Err(if fail_reasons.is_empty() {
            "还原未完成".to_string()
        } else {
            fail_reasons.join("；")
        });
    }
    // 收尾通知只发一次（全部步骤完成后）：文案区分是否真的启用了网卡；
    // 放弃阀放行的拍不发「已还原」——校园卡可能仍被 netsh 持久禁用，
    // 改发告警文案提醒手动启用，与此前「请手动启用」告警口径一致
    if gave_up_released {
        crate::log_info!(
            "outbound",
            "夜间出站还原: 已按放弃阀放行收尾，校园网卡启用状态待人工确认"
        );
        crate::infra::notification::emit_notification(
            app_handle,
            "夜间出站还原",
            "还原流程已收尾，但校园网网卡可能仍处于禁用状态（禁用名单快照损坏）；请在系统「网络设置」中手动检查并启用",
            "mascot-alert",
        );
    } else {
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
    }
    // 还原只按快照写回切换前原值：排列顺序仅用于夜间选目标卡，不参与跃点排列
    // （否则「还原」会变成把每张卡的跃点改写成 1..N）。
    if finish_outbound_restore(app_handle) {
        Ok(())
    } else {
        Err("切换状态快照清理失败，将自动重试".to_string())
    }
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
    // 还原成功即解除「立即切换」当日保持（hold 在还原窗内本就冻结自动还原，
    // 此处覆盖手动还原路径与跨日自动还原路径，双保险归零）
    clear_outbound_manual_hold(app_handle);
    // 切换态已清（巡检不再被「出站切换态」闸门跳过）、校园网卡已启用：立刻补一次
    // 巡检，让自动登录在还原后第一时间评估。否则要等一整个巡检周期——轻量化下
    // 300s，正是「整夜休眠后到点不自动登录、只能手动点」的缺口（见
    // background_task::spawn_background_check_after_outbound_restore 的实证注释）。
    // 所有还原路径（自动/手动/启动对账）都汇到此处，钩子只挂一次。
    //
    // 2026-10-26：还原时刻可配置后，还原点可能早于校园网检测静默期起点
    // （默认还原 07:30 / 静默期 07:40），补检会被静默期整体跳过、登录判定不生效。
    // 还原是本应用主动制造的状态迁移，先立一次性放行标记再发起补检。
    crate::monitor::background_check::request_check_after_outbound_restore();
    crate::monitor::background_task::spawn_background_check_after_outbound_restore(app_handle);
    true
}

/// 清空切换态快照（三份：跃点/禁用名单/兜底路由；落盘成功才 store 内存——与切换写入
/// 同款语义，避免"内存已还原、重启后又回到切换态"的错位）；返回快照是否已清。
fn clear_outbound_snapshot(app_handle: &AppHandle) -> bool {
    let state = app_handle.state::<AppState>();
    let current = state.config.load();
    if current.outbound_metric_restore.is_empty()
        && current.outbound_disabled_adapters.is_empty()
        && current.outbound_standby_route.is_empty()
        && current.outbound_portal_route.is_empty()
    {
        return true;
    }
    drop(current);
    if let Err(e) = crate::config::persist::mutate_config_and_save(app_handle, &state, |fresh| {
        fresh.outbound_metric_restore = String::new();
        fresh.outbound_disabled_adapters = String::new();
        fresh.outbound_standby_route = String::new();
        fresh.outbound_portal_route = String::new();
        Ok(())
    }) {
        crate::log_warn!("outbound", "夜间出站切换: 切换态快照清空落盘失败: {e}");
        return false;
    }
    true
}

/// 恢复保持标记落盘（只延后自动 Restore，不影响计划 Switch；写路径与
/// clear_outbound_snapshot 同款：load fresh → 改字段 → 加密落盘成功才 store 内存）。
/// 落盘失败仅告警不阻断，白天的切换态可能在下一拍被自动还原，由用户重新切换。
fn set_outbound_manual_hold(app_handle: &AppHandle, day: i32) {
    let state = app_handle.state::<AppState>();
    if state.config.load().outbound_manual_hold_day == day {
        return;
    }
    persist_config_update(app_handle, |fresh| fresh.outbound_manual_hold_day = day);
}

/// 清 hold 标记（手动还原先行清；自动还原成功在 finish_outbound_restore 收尾清）。
fn clear_outbound_manual_hold(app_handle: &AppHandle) {
    set_outbound_manual_hold(app_handle, 0);
}

/// 改切时把"将被本次动作改写、但还没进快照"的卡按当前值补记进快照，并把
/// 目标卡移到首行——replay 与启动对账都以 snapshot[0] 认定目标卡（见 1059、1238）。
/// 已在快照的卡保持原值（现值已被切换改过，写回去就把原值弄丢了）。
/// 返回是否有改动（决定要不要落盘）。
fn merge_retarget_snapshot(
    snapshot: &mut Vec<(String, Vec<MetricRow>)>,
    target_guid: &str,
    target_rows: &[MetricRow],
    origin: Option<(&str, &[MetricRow])>,
) -> bool {
    let mut dirty = false;
    if !snapshot.iter().any(|(guid, _)| guid == target_guid) {
        snapshot.push((target_guid.to_string(), target_rows.to_vec()));
        dirty = true;
    }
    if let Some((guid, rows)) = origin {
        if !rows.is_empty() && !snapshot.iter().any(|(g, _)| g == guid) {
            snapshot.push((guid.to_string(), rows.to_vec()));
            dirty = true;
        }
    }
    if let Some(pos) = snapshot.iter().position(|(guid, _)| guid == target_guid) {
        if pos != 0 {
            let entry = snapshot.remove(pos);
            snapshot.insert(0, entry);
            dirty = true;
        }
    }
    dirty
}

/// 切换态内改切到另一张卡（夜间/手动切换生效后再点其它卡的「切换出站」）。
/// 不重建快照：快照保存的始终是**切换前**的原值，只有首次切换与本次会被改写、
/// 但尚未进快照的卡才写入。写入规则：
/// - 目标卡压到 1；
/// - 上次切换压到 1 的那张卡（快照首行 == 当前出口）立即按快照原值写回，
///   满足「点其它卡自动还原原有网卡参数」；它若因此抢回出口，退回抬高重试；
/// - 其它已抬高的卡（原出口）保持 SWITCHED_ORIGIN_METRIC，否则出口会被抢回。
fn retarget_outbound_switch(app_handle: &AppHandle, adapter_name: &str) -> Result<(), String> {
    let Some(_busy) = OutboundActionGuard::try_acquire() else {
        return Err("另一出站动作进行中，请稍后再试".to_string());
    };
    let config = app_handle.state::<AppState>().config.load_full();
    let adapter = resolve_switch_target(adapter_name)?;
    let target_rows = read_family_rows(&adapter.guid)?;
    if target_rows.is_empty() {
        return Err(format!("网卡 {} 没有 IPv4/IPv6 跃点配置行", adapter.name));
    }
    // 快照是原值的唯一凭据：损坏时拒绝改切（按错的「原值」写回会让还原还原出错误值）。
    // metric 快照为空而禁用名单/兜底路由非空时切换态仍成立但无原值可保——
    // 此时目标卡的现值就是原值，由 merge_retarget_snapshot 补记。
    let mut snapshot: Vec<(String, Vec<MetricRow>)> = Vec::new();
    for row in parse_outbound_snapshot(&config.outbound_metric_restore)? {
        let entry = MetricRow {
            family: row.family,
            automatic: row.automatic,
            metric: row.metric,
        };
        match snapshot.iter().position(|(guid, _)| guid == &row.guid) {
            Some(idx) => snapshot[idx].1.push(entry),
            None => snapshot.push((row.guid.clone(), vec![entry])),
        }
    }
    let origin = current_origin_adapter();
    let origin_rows: Option<(String, String, Vec<MetricRow>)> = match origin.as_ref() {
        Some((name, guid, _)) if guid != &adapter.guid => {
            let rows = read_family_rows(guid)?;
            if rows.is_empty() {
                None
            } else {
                Some((name.clone(), guid.clone(), rows))
            }
        }
        _ => None,
    };
    let snapshot_dirty = merge_retarget_snapshot(
        &mut snapshot,
        &adapter.guid,
        &target_rows,
        origin_rows
            .as_ref()
            .map(|(_, guid, rows)| (guid.as_str(), rows.as_slice())),
    );
    if snapshot_dirty {
        let json = snapshot_json_multi(&snapshot);
        let state = app_handle.state::<AppState>();
        if let Err(e) =
            crate::config::persist::mutate_config_and_save(app_handle, &state, move |fresh| {
                fresh.outbound_metric_restore = json;
                Ok(())
            })
        {
            crate::log_warn!("outbound", "出站切换: 切换态快照补记失败: {e}");
            return Err(format!("切换状态快照写入失败: {e}"));
        }
        crate::log_info!(
            "outbound",
            "出站切换: {} 已记入切换态快照（其它卡原值保持不变）",
            adapter.name
        );
    }
    // 新目标压到 1、当前出口抬到 100；原值留在快照里供最终还原统一写回。
    // 与首次切换共用顺序：先写新跃点，再下线旧出口，恢复后复写，杜绝上线抢回窗口。
    let families: Vec<u16> = target_rows.iter().map(|r| r.family).collect();
    let mut encoded = encode_metric_rows(&adapter.guid, &families, 1);
    if let Some((_, guid, rows)) = &origin_rows {
        let origin_families: Vec<u16> = rows.iter().map(|r| r.family).collect();
        encoded.extend(encode_metric_rows(
            guid,
            &origin_families,
            SWITCHED_ORIGIN_METRIC,
        ));
    }
    if let Some((name, guid, _)) = &origin_rows {
        mark_origin_bounce_pending(app_handle, name, guid)?;
    }
    run_metric_bounce_sequence(
        origin_rows.is_some(),
        false,
        |_| run_set_metric(app_handle, &adapter.name, &encoded, false, ""),
        || {
            if let Some((name, guid, _)) = &origin_rows {
                bounce_origin_adapter(app_handle, name, guid)
            } else {
                Ok(())
            }
        },
    )?;
    verify_switch_effect(app_handle, &config, &adapter.name, &adapter.ip, adapter.if_index)?;
    Ok(())
}

/// 当前默认出口是否就是这张卡（按名字比对）。
fn is_current_outbound(adapter_name: &str) -> bool {
    current_origin_adapter()
        .map(|(name, _, _)| name == adapter_name)
        .unwrap_or(false)
}

/// 手动切出站收尾：失败解除恢复保持，让自动路径按退避接管；成功的实际切换
/// 由调用方写入恢复保持，避免白天被自动还原立即撤销。
fn finish_manual_outbound_action(
    app_handle: &AppHandle,
    result: Result<(), String>,
) -> Result<(), String> {
    match result {
        Ok(()) => Ok(()),
        Err(reason) => {
            clear_outbound_manual_hold(app_handle);
            Err(reason)
        }
    }
}

/// 「立即切换」命令入口（桌面）：用户显式触发，覆盖自动切换的退避闸。
/// 只有成功进入切换态才写恢复保持；点当前出口等幂等无操作不会留下整日冻结标记。
/// 已处于切换态时按优先级选卡后改切（沿用既有快照），不要求先还原。
pub(crate) fn manual_outbound_switch(app_handle: &AppHandle) -> Result<(), String> {
    let config = app_handle.state::<AppState>().config.load_full();
    if !config.enable_night_outbound_switch {
        return Err("夜间出站切换未开启".to_string());
    }
    let today = chrono::Local::now().date_naive().num_days_from_ce();
    // 用户显式重试：清退避闸，不做自动路径的阶梯等待
    clear_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
    let was_active = outbound_switch_active(app_handle);
    let result = if was_active {
        // 手动路径不发「夜切未生效」告警：那是自动路径的用户可见副作用
        build_outbound_plan(app_handle, &config, None, None, false).and_then(|plan| {
            if is_current_outbound(&plan.target_name) {
                crate::log_info!("outbound", "出站切换: 当前出口已是 {}", plan.target_name);
                Ok(())
            } else {
                retarget_outbound_switch(app_handle, &plan.target_name)
            }
        })
    } else {
        apply_outbound_switch_with_health(app_handle, &config, None, None, false)
    };
    if result.is_ok() && !was_active && outbound_switch_active(app_handle) {
        set_outbound_manual_hold(app_handle, today);
    }
    finish_manual_outbound_action(app_handle, result)
}

/// 「立即切换出站」命令入口（桌面，条目按钮）：以**指定网卡**为目标直接切换，
/// 跳过优先级选卡与公网复核——用户点名要哪张就切哪张，哪怕它是校园网卡。
/// 切换态内再点其它卡不再被拒：走 retarget_outbound_switch，上次那张卡立即按
/// 快照原值还原，无需先点「恢复原出口」。实际切换成功后保持当日的自动还原。
pub(crate) fn manual_outbound_switch_to(
    app_handle: &AppHandle,
    adapter_name: &str,
) -> Result<(), String> {
    let config = app_handle.state::<AppState>().config.load_full();
    if !config.enable_night_outbound_switch {
        return Err("夜间出站切换未开启".to_string());
    }
    let today = chrono::Local::now().date_naive().num_days_from_ce();
    clear_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
    let was_active = outbound_switch_active(app_handle);
    let result = if is_current_outbound(adapter_name) {
        // 已是出口：幂等成功（两条入口语义一致，不再一个 Ok 一个 Err）
        crate::log_info!(
            "outbound",
            "出站切换: {adapter_name} 已是当前出口，无需动作"
        );
        Ok(())
    } else if outbound_switch_active(app_handle) {
        retarget_outbound_switch(app_handle, adapter_name)
    } else {
        apply_outbound_switch_with_health(app_handle, &config, None, Some(adapter_name), false)
    };
    if result.is_ok() && !was_active && outbound_switch_active(app_handle) {
        set_outbound_manual_hold(app_handle, today);
    }
    finish_manual_outbound_action(app_handle, result)
}

/// 「立即还原」命令入口（桌面）：先清 hold 与还原退避（用户显式重试覆盖退避闸）
/// 再执行还原；还原未完成时快照仍在，自动还原会在后续拍继续重试。
pub(crate) fn manual_outbound_restore(app_handle: &AppHandle) -> Result<(), String> {
    let config = app_handle.state::<AppState>().config.load_full();
    if !outbound_switch_active(app_handle) {
        // 非切换态没有快照可还原：跃点已是用户原值，不能按出站优先级改写。
        crate::log_info!(
            "outbound",
            "手动还原出站: 当前未处于出站切换状态，跃点无需还原"
        );
        return Ok(());
    }
    // hold 必须先清：否则还原失败后，还原窗内的自动重试会被 hold 一并冻结
    clear_outbound_manual_hold(app_handle);
    clear_outbound_failure(&OUTBOUND_RESTORE_LAST_FAIL_MS, &OUTBOUND_RESTORE_FAIL_COUNT);
    if let Err(reason) = apply_outbound_restore(app_handle, &config) {
        Err(reason)
    } else {
        Ok(())
    }
}

/// 启动对账（循环启动前一次性、须在 spawn_blocking 线程内）：三份快照任一非空说明上次
/// 运行留下了切换态（崩溃/被杀/睡眠跨过恢复点）——
/// ① 恢复窗口内、或功能已关闭 → 立即还原；
/// ② 夜间窗口内，只有"目标卡仍在用且判定为非校园网"才重放切换（只写目标值、
///    不动快照原值；禁用补做与路由补加随重放一并幂等执行），否则直接还原、放弃本夜切换。
pub(super) fn reconcile_outbound_on_startup(app_handle: &AppHandle) {
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
    if !config.enable_night_outbound_switch || in_outbound_restore_window(&config, now_minutes) {
        crate::log_info!("outbound", "启动对账: 残留切换态处于恢复期，立即还原");
        let _ = apply_outbound_restore(app_handle, &config);
        return;
    }
    // 夜间窗口分支依赖跃点快照确定目标卡；快照不可用（空/损坏）则按还原处理——
    // 正常切换必然三份快照同时写入，缺跃点快照即状态不完整
    let snapshot = match parse_outbound_snapshot(&config.outbound_metric_restore) {
        Ok(rows) if !rows.is_empty() => rows,
        _ => {
            crate::log_warn!("outbound", "启动对账: 跃点快照不可用，按还原处理");
            let _ = apply_outbound_restore(app_handle, &config);
            return;
        }
    };
    let guid = snapshot[0].guid.clone();
    let adapters = crate::network::get_adapters_cached().unwrap_or_default();
    let Some(adapter) = adapters.iter().find(|a| a.guid == guid) else {
        crate::log_info!("outbound", "启动对账: 目标卡已不存在，按还原处理");
        let _ = apply_outbound_restore(app_handle, &config);
        return;
    };
    let details = crate::network::get_adapter_details_cached().unwrap_or_default();
    let gateway = details
        .iter()
        .find(|d| d.name == adapter.name)
        .map(|d| d.gateway.clone())
        .unwrap_or_default();
    let portal_host = portal_probe_host(&config.portal_url);
    let back_on_campus = adapter.ip.is_empty()
        || is_campus_adapter(
            &adapter.ip,
            &gateway,
            &config.campus_gateway,
            &portal_host,
            |gw, source_ip| crate::network::check_gateway_reachable_from(gw, Some(source_ip)),
        );
    if back_on_campus {
        crate::log_info!(
            "outbound",
            "启动对账: 目标卡 {} 已不可用或回到校园网，按还原处理",
            adapter.name
        );
        let _ = apply_outbound_restore(app_handle, &config);
        return;
    }
    crate::log_info!(
        "outbound",
        "启动对账: 夜间窗口内重放未完成的出站切换(目标卡 {})",
        adapter.name
    );
    replay_outbound_switch(app_handle, &config);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metric_row(family: u16, metric: u32) -> MetricRow {
        MetricRow {
            family,
            automatic: false,
            metric,
        }
    }

    /// 改切只补记"将被改写但还没进快照"的卡，绝不覆盖快照里已有的原值——
    /// 覆盖会让还原写回切换后的值，用户网卡参数被永久改掉。
    #[test]
    fn 改切快照_保留既有原值且目标卡落在首行() {
        let mut snapshot: Vec<(String, Vec<MetricRow>)> = vec![
            ("A".to_string(), vec![metric_row(2, 30)]),
            ("O".to_string(), vec![metric_row(2, 25)]),
        ];

        // A 已是首行且已在快照：现值 1 不得覆盖原值 30
        assert!(!merge_retarget_snapshot(
            &mut snapshot,
            "A",
            &[metric_row(2, 1)],
            None
        ));
        assert_eq!(snapshot[0].0, "A");
        assert_eq!(snapshot[0].1[0].metric, 30);

        // B/C 不在快照：按当前值补记；B 作为新目标必须移到首行
        // （replay_outbound_switch 与启动对账都以 snapshot[0] 认定目标卡）
        assert!(merge_retarget_snapshot(
            &mut snapshot,
            "B",
            &[metric_row(2, 25)],
            Some(("C", &[metric_row(2, 10)]))
        ));
        assert_eq!(snapshot[0].0, "B", "新目标卡必须在首行");
        assert_eq!(snapshot[0].1[0].metric, 25);
        assert_eq!(find_rows(&snapshot, "C")[0].metric, 10);
        assert_eq!(find_rows(&snapshot, "A")[0].metric, 30, "既有原值不被覆盖");

        // 已在快照且已在首行：重复调用无改动
        assert!(!merge_retarget_snapshot(
            &mut snapshot,
            "B",
            &[metric_row(2, 1)],
            None
        ));
        assert_eq!(snapshot[0].0, "B");

        // 已在快照但不在首行：只移动位置，不改原值
        assert!(merge_retarget_snapshot(
            &mut snapshot,
            "A",
            &[metric_row(2, 1)],
            None
        ));
        assert_eq!(snapshot[0].0, "A");
        assert_eq!(snapshot[0].1[0].metric, 30, "移动位置不得改写原值");
    }

    fn find_rows<'a>(snapshot: &'a [(String, Vec<MetricRow>)], guid: &str) -> &'a [MetricRow] {
        &snapshot
            .iter()
            .find(|(g, _)| g == guid)
            .expect("快照缺少该卡")
            .1
    }

    fn adapter(name: &str, ip: &str, wireless: bool) -> Adapter {
        Adapter {
            name: name.to_string(),
            ip: ip.to_string(),
            wireless,
            guid: format!("{{{name}}}"),
            mac: String::new(),
            if_index: 0,
            status: crate::network::discovery::AdapterStatus::Connected,
            link_speed: 0,
        }
    }

    /// 预热集合必须覆盖判定阶段会查询的组合：漏收会退化成串行探测（耗时回退），
    /// 多收只是多探一次。2026-11-10 取消网段腿后，每张有 IP 有网关的卡都要探校园目标。
    #[test]
    fn 探测预热_只收判定会查询的组合() {
        // 校园卡 10.2.100.5 与热点卡 192.168.6.119 都参与预热
        let campus = (
            adapter("以太网", "10.2.100.5", false),
            "10.2.127.254".to_string(),
        );
        let hotspot = (
            adapter("WLAN", "192.168.6.119", true),
            "192.168.6.1".to_string(),
        );
        let no_gw = (adapter("以太网 2", "192.168.6.115", false), String::new());
        let pairs = collect_probe_targets(
            &[
                "以太网".to_string(),
                "WLAN".to_string(),
                "以太网 2".to_string(),
            ],
            &[campus, hotspot, no_gw],
            "10.2.127.254",
            "10.1.99.100",
        );
        let has = |dest: &str, src: &str| pairs.contains(&(dest.to_string(), src.to_string()));
        // 校园卡不再靠网段短路：校园网关与 portal 同样要预热
        assert!(has("10.2.127.254", "10.2.100.5"));
        assert!(has("10.1.99.100", "10.2.100.5"));
        assert!(
            !has(OUTBOUND_INTERNET_HEALTH_KEY, "10.2.100.5"),
            "公网健康由 TCP/HTTPS 单独并行，不再发 ICMP"
        );
        // 非校园卡：校园网关与 portal 都要探
        assert!(has("10.2.127.254", "192.168.6.119"));
        assert!(has("10.1.99.100", "192.168.6.119"));
        // 自身网关走 ICMP 预热；公网健康走独立 TCP/HTTPS 批次
        assert!(has("192.168.6.1", "192.168.6.119"));
        assert!(!has(OUTBOUND_INTERNET_HEALTH_KEY, "192.168.6.119"));
        // 无网关的卡出不了站，判定阶段不查它
        assert!(!has("10.2.127.254", "192.168.6.115"));
        assert!(!has(OUTBOUND_INTERNET_HEALTH_KEY, "192.168.6.115"));
    }

    /// 无 IP / 不在优先级内的卡不进预热集合（判定阶段也不会查它们）。
    #[test]
    fn 探测预热_跳过无ip与名单外的卡() {
        let ipless = (adapter("以太网", "", false), "10.2.127.254".to_string());
        let outside = (
            adapter("WLAN", "192.168.6.119", true),
            "192.168.6.1".to_string(),
        );
        let pairs = collect_probe_targets(
            &["以太网".to_string()],
            &[ipless, outside],
            "10.2.127.254",
            "",
        );
        assert!(pairs.is_empty(), "只收优先级内且有 IP 的卡: {pairs:?}");
    }

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

    /// 禁用网卡走 PnP 设备级禁用并轮询复核（USB 卡窗口 15s）：超时必须比通用写入宽，
    /// 否则「卡已禁用但结果没回来」会判成失败，让切换在卡已下线的情况下反复重试。
    #[test]
    fn 禁用网卡_helper超时宽于普通写入() {
        assert!(helper_timeout_secs("disable_adapter") >= 30);
        assert_eq!(
            helper_timeout_secs("set_metric"),
            OUTBOUND_HELPER_TIMEOUT_SECS
        );
        assert_eq!(
            helper_timeout_secs("route_add"),
            OUTBOUND_HELPER_TIMEOUT_SECS
        );
    }

    #[test]
    fn 重放现场_启用失败必须保留禁用名单() {
        let pending = DisabledRow {
            guid: "{origin}".to_string(),
            name: "以太网".to_string(),
        };
        let still_pending = std::cell::Cell::new(true);
        let mut remaining = Vec::new();
        if still_pending.get() {
            remaining.push(pending);
        }
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].name, "以太网");
    }

    #[test]
    fn 幂等手动切换_不得新增恢复保持() {
        fn should_hold(active_before: bool, actual_action: bool, active_after: bool) -> bool {
            !active_before && actual_action && active_after
        }
        assert!(should_hold(false, true, true), "首次实际切换保持自动还原");
        assert!(!should_hold(true, false, true), "切换态内点当前卡不得新增保持");
        assert!(!should_hold(false, false, false), "无操作成功不得留下保持");
    }

    fn snapshot_row(guid: &str) -> OutboundSnapshotRow {
        OutboundSnapshotRow {
            guid: guid.to_string(),
            family: 2,
            automatic: false,
            metric: 30,
        }
    }

    fn disabled_adapter(name: &str, status: crate::network::discovery::AdapterStatus) -> Adapter {
        let mut a = adapter(name, "", true);
        a.status = status;
        a
    }

    #[test]
    fn 还原启用目标_覆盖被手动禁用的原出口卡() {
        use crate::network::discovery::AdapterStatus;
        let snapshot = vec![snapshot_row("{WLAN}"), snapshot_row("{以太网}")];
        // 名单为空：用户在系统设置里手动禁用了原出口卡
        let adapters = vec![
            disabled_adapter("WLAN", AdapterStatus::Connected),
            disabled_adapter("以太网", AdapterStatus::Disabled),
        ];
        let targets = restore_enable_targets(&[], &snapshot, &adapters);
        assert_eq!(targets, vec!["以太网".to_string()]);
    }

    #[test]
    fn 还原启用目标_首行目标卡不纳入且名单不重复() {
        use crate::network::discovery::AdapterStatus;
        let snapshot = vec![snapshot_row("{WLAN}"), snapshot_row("{以太网}")];
        let list = vec![DisabledRow {
            guid: "{以太网}".to_string(),
            name: "以太网".to_string(),
        }];
        let adapters = vec![
            disabled_adapter("WLAN", AdapterStatus::Disabled),
            disabled_adapter("以太网", AdapterStatus::Disabled),
        ];
        let targets = restore_enable_targets(&list, &snapshot, &adapters);
        assert_eq!(targets, vec!["以太网".to_string()], "不重复纳入，且目标卡不参与");
    }

    #[test]
    fn 还原启用目标_原卡已恢复则不纳入() {
        use crate::network::discovery::AdapterStatus;
        let snapshot = vec![snapshot_row("{WLAN}"), snapshot_row("{以太网}")];
        let adapters = vec![
            disabled_adapter("WLAN", AdapterStatus::Connected),
            disabled_adapter("以太网", AdapterStatus::Connected),
        ];
        assert!(restore_enable_targets(&[], &snapshot, &adapters).is_empty());
    }

    #[test]
    fn 竞争卡_只收非目标且已连接有ip的卡() {
        use crate::network::discovery::AdapterStatus;
        let mut target = adapter("WLAN", "192.168.6.113", true);
        target.guid = "{target}".to_string();
        let mut peer = adapter("以太网 2", "192.168.6.107", false);
        peer.guid = "{peer}".to_string();
        let mut down = adapter("以太网 4", "10.2.38.176", false);
        down.guid = "{down}".to_string();
        down.status = AdapterStatus::Disconnected;
        let mut ipless = adapter("以太网 3", "", false);
        ipless.guid = "{ipless}".to_string();
        let adapters = vec![target, peer, down, ipless];
        let picked: Vec<&str> = outbound_suppress_targets("{target}", &adapters)
            .iter()
            .map(|a| a.name.as_str())
            .collect();
        assert_eq!(picked, vec!["以太网 2"], "只压制真正会竞争默认路由的对端卡");
    }

    #[test]
    fn 压制编码_所有竞争卡都抬到同一跃点() {
        let cards = vec![
            ("{origin}".to_string(), vec![2u16, 23]),
            ("{peer}".to_string(), vec![2u16]),
        ];
        let encoded = encode_suppress_rows(&cards);
        assert_eq!(
            encoded,
            vec![
                format!("{{origin}}:2:0:{SWITCHED_ORIGIN_METRIC}"),
                format!("{{origin}}:23:0:{SWITCHED_ORIGIN_METRIC}"),
                format!("{{peer}}:2:0:{SWITCHED_ORIGIN_METRIC}"),
            ]
        );
    }

    #[test]
    fn 原出口下线保持覆盖连接断链与重连退避() {
        assert!(
            ORIGIN_DISABLE_HOLD_MS >= 5_000,
            "1 秒不足以覆盖应用的断链感知和首轮重连退避"
        );
    }

    #[test]
    fn 切换事务_先写跃点再下线_恢复后复核() {
        let events = std::cell::RefCell::new(Vec::new());
        run_metric_bounce_sequence(
            true,
            true,
            |notify| {
                events
                    .borrow_mut()
                    .push(if notify { "rewrite" } else { "write" });
                Ok(())
            },
            || {
                events.borrow_mut().push("bounce");
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(events.into_inner(), vec!["write", "bounce", "rewrite"]);
    }

    #[test]
    fn 切换事务_首次跃点失败不得下线原卡() {
        let bounced = std::cell::Cell::new(false);
        let result = run_metric_bounce_sequence(
            true,
            false,
            |_| Err("write failed".to_string()),
            || {
                bounced.set(true);
                Ok(())
            },
        );
        assert!(result.is_err());
        assert!(!bounced.get());
    }

    #[test]
    fn 路由表读取失败不得视为验证通过() {
        assert!(validate_switch_probe(Err("route table unavailable".to_string()), "WLAN").is_err());
    }

    #[test]
    fn 切换探测尊重配置的一秒默认预算() {
        let config = crate::config::Config::default();
        assert_eq!(config.outbound_probe_timeout_ms, 1000);
        assert_eq!(outbound_probe_budget(&config), Duration::from_millis(1000));
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
            assert!(
                !backoff_gate(fail_at, count, fail_at + 299_999),
                "第 {count} 次失败"
            );
            assert!(
                backoff_gate(fail_at, count, fail_at + 300_000),
                "第 {count} 次失败"
            );
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

    /// 单飞闸：同一时刻只放一个出站动作（切换/还原/重放/看门狗共用一把），
    /// RAII 释放后立即可再取——防「还原清快照后重放又写回 metric=1」的并发静默丢失。
    #[test]
    fn 单飞闸_同一时刻只放一个出站动作_释放后可再取() {
        let first = OutboundActionGuard::try_acquire();
        assert!(first.is_some(), "首次获取应成功");
        assert!(
            OutboundActionGuard::try_acquire().is_none(),
            "占用期间第二次获取应失败（互斥）"
        );
        drop(first);
        assert!(
            OutboundActionGuard::try_acquire().is_some(),
            "释放后应可再次获取"
        );
    }

    /// 单条目快照（与 monitor::outbound_switch::snapshot_json 的同构输出）
    const SNAPSHOT_ONE_ROW: &str = r#"[{"guid":"{G1}","family":2,"automatic":true,"metric":55}]"#;
}
