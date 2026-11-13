//! 夜间出站实现：从 scheduled.rs 原样外移（见 changelog v2.4.3）

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use chrono::{Datelike, Timelike};
use tauri::{AppHandle, Manager};
use crate::config::night_switch::{restore_start_minutes, RESTORE_END_MINUTES};
use crate::config::outbound_switch::outbound_restore_active;
use crate::infra::state::AppState;
use crate::monitor::outbound_switch::{
    effective_outbound_priority, is_campus_adapter, parse_disabled_adapters,
    parse_standby_route, portal_probe_host,
    select_outbound_candidate, snapshot_json, standby_route_cleanup, StandbyCleanup,
    OUTBOUND_INTERNET_HEALTH_KEY,
};
use crate::network::Adapter;
use crate::platform::metric::MetricRow;

/// 夜间出站切换：提权 helper 执行超时（改跃点/禁网卡/加路由都是即时操作，超时即判失败走退避）
const OUTBOUND_HELPER_TIMEOUT_SECS: u64 = 30;

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
fn notify_outbound_issue_once_per_day(app_handle: &AppHandle, last_day: &AtomicI32, title: &str, body: &str) {
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
pub(super) fn needs_replay(metric_snapshot: &str, disabled_adapters: &str, standby_route: &str, fail_count: u32) -> bool {
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
/// 成功：清退避 + info 日志 +（`notify_success` 时）系统通知（文案带 `notify_suffix`
/// 尾注，如「已临时禁用2张校园网网卡」）；失败：记退避 + warn 日志，首次失败额外通知一次。
/// 提权降级策略照 adapter_watch 的自动启用：首试静默（CMSTPLUA 可用时零 UAC 打扰），
/// 仅在**确因提权通道失败**后允许降级弹 UAC（静默通道被系统封堵时 UAC 是唯一恢复通道），
/// 频率由退避阶梯限制；失败原因经 Err(原因) 上报调用方（手动入口透传前端 toast）。
fn write_outbound_metric(
    app_handle: &AppHandle,
    name: &str,
    guid: &str,
    families: &[u16],
    notify_success: bool,
    notify_suffix: &str,
) -> Result<(), String> {
    if families.is_empty() {
        // 卡在但跃点行缺失：写不进去。同样记退避，避免 30s 一拍刷日志；
        // 不属提权通道失败，不抬高 UAC 放行计数
        let count = mark_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
        crate::log_warn!(
            "outbound",
            "夜间出站切换: {name} 当前无可用跃点行(已拔出/协议栈缺失)，{}s 后重试",
            outbound_backoff_ms(count) / 1000
        );
        return Err("当前无可用跃点行（网卡已拔出或协议栈缺失）".to_string());
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
            Ok(())
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

/// 解析旁路路由快照（空串/非法 JSON → None）。
fn parse_portal_route(json: &str) -> Option<PortalRoute> {
    if json.trim().is_empty() {
        return None;
    }
    serde_json::from_str(json.trim()).ok()
}

/// 构造 Portal 旁路路由：目标 = portal 私网主机，下一跳 = 当前出口（原网卡）网关。
/// 任一环缺失（portal 是域名/公网地址、出口读不到、网关为空）都返回 None——
/// 此时不加路由，登录恢复会因到不了 Portal 而失败，日志写明原因。
fn build_portal_bypass_route(config: &crate::config::Config) -> Option<PortalRoute> {
    let dest = portal_probe_host(&config.portal_url);
    if dest.is_empty() {
        return None;
    }
    let probe_dest: std::net::Ipv4Addr = OUTBOUND_INTERNET_HEALTH_KEY.parse().ok()?;
    let if_index = crate::platform::best_route::best_route_if_index_v4(probe_dest).ok()?;
    let adapters = crate::network::get_adapters_cached().ok()?;
    let adapter = adapters.iter().find(|a| a.if_index == if_index)?;
    let details = crate::network::get_adapter_details_cached().ok()?;
    let gateway = details
        .iter()
        .find(|d| d.name == adapter.name)
        .map(|d| d.gateway.clone())?;
    if gateway.is_empty() {
        return None;
    }
    Some(PortalRoute {
        dest,
        gateway,
        if_index,
        metric: 1,
    })
}

/// 默认出口是否已落在指定接口：读系统默认路由的接口索引比对。
/// 只改跃点时"写了跃点"与"出口真的换了"是两件事——TUN/代理可能持有 metric=0
/// 的默认路由，此时跃点写入成功但出口不动，必须靠这一步暴露。
#[cfg(target_os = "windows")]
fn outbound_route_targets(target_if_index: u32) -> Result<bool, String> {
    let dest: std::net::Ipv4Addr = OUTBOUND_INTERNET_HEALTH_KEY
        .parse()
        .map_err(|e| format!("健康探测目标地址非法: {e}"))?;
    crate::platform::best_route::best_route_if_index_v4(dest).map(|idx| idx == target_if_index)
}

#[cfg(not(target_os = "windows"))]
fn outbound_route_targets(_target_if_index: u32) -> Result<bool, String> {
    Err("当前平台不支持默认出口查询".to_string())
}

/// 夜间出站切换动作（须在 spawn_blocking 线程内执行）：退避闸 → 选目标卡 → 读两族跃点
/// → 快照落盘（metric/禁用名单，切换态成立的唯一凭据，落盘成功才进内存）
/// → helper 写 Metric=1 → helper 禁用校园网卡。
/// 返回 Ok(())=快照已落盘且两步 helper 全部成功（本拍完成切换）；Err(原因)=本拍未完成
/// （无候选/读跃点失败/落盘失败/任一 helper 失败；快照可能已保留，由补齐路径续写），
/// 失败原因透传给手动入口的 toast 文案（自动路径只用于判定，细节日志已落盘）。
///
/// 快照在 helper 之前落盘；helper 失败**不清快照**（裁决 Important-2：若 v4 已改成功而
/// v6 失败，清快照会让已改的跃点无从还原）——切换态保留 + 退避重试写回，重试幂等。
/// 手动切换与故障自动切换共用；调用方保证当前不处于切换态，故入口无需再判快照。
///
/// 六期新增两步（TUN 代理并存时的实测复盘）：
/// - 禁用校园网卡是主力：网卡级下线后其路由/DNS/跃点竞争全部消失，代理内核
///   （FlClashCore auto-detect-interface）只能改绑热点；metric=1 单独压不住
///   metric=0 的 TUN 默认路由，也不产生路由事件、代理不跟随。
/// - 原「兜底默认路由」第三步已于 2026-10-18 事故后彻底删除：它与目标卡自己的 DHCP
///   默认路由共享「下一跳+接口」、只差跃点，Windows 判重不看跃点 → 加不上（每夜重放），
///   而删除同样不看跃点 → 会误删用户自己的默认路由（WLAN 直接断网）。详见
///   `platform/route_table` 模块注释与 `.codewiki/decisions/night-outbound-switch`。
/// - 禁用名单只跳过总线类型无法判定的卡（USB 卡 2026-10 起纳入禁用：用户确认
///   其 USB 副卡应参与，运行期 disable/enable 对称性已实证）。
/// - 切换完成后路由级验证（GetBestRoute）：到校园网关的最优路由仍指向校园卡
///   ifIndex 则判失败走重放；空禁用名单（误判漏禁）单独每天告警一次。
///
/// 切换目标卡的公网健康结论由逐卡并行 TCP/HTTPS 提供，结果参与候选硬闸门。

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

/// 并行探测全部 (目标, 绑源) 组合。同一组合发 [`CAMPUS_PROBE_ATTEMPTS`] 次、
/// 任一可达即判可达——与 [`crate::campus::rules::probe_with_retry`] 的抗丢包语义
/// 同源（去抖次数由共享层统一规定），但各发同时发出，最坏只等一个超时。
fn warm_probe_cache(pairs: &[(String, String)]) -> ProbeCache {
    use std::collections::HashMap;
    if pairs.is_empty() {
        return ProbeCache(HashMap::new());
    }
    let tasks: Vec<(String, String)> = pairs
        .iter()
        .flat_map(|(dest, source)| {
            std::iter::repeat((dest.clone(), source.clone()))
                .take(crate::campus::rules::CAMPUS_PROBE_ATTEMPTS)
        })
        .collect();
    let started = std::time::Instant::now();
    let results: Vec<((String, String), bool)> = std::thread::scope(|scope| {
        let handles: Vec<_> = tasks
            .into_iter()
            .map(|(dest, source)| {
                scope.spawn(move || {
                    let ok =
                        crate::network::check_gateway_reachable_from(&dest, Some(&source));
                    ((dest, source), ok)
                })
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    });
    let mut map: HashMap<(String, String), bool> = HashMap::new();
    for (key, ok) in results {
        map.entry(key).and_modify(|v| *v |= ok).or_insert(ok);
    }
    crate::log_info!(
        "outbound",
        "夜间出站切换: 并行绑源探测 {} 组完成，耗时 {}ms",
        pairs.len(),
        started.elapsed().as_millis()
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

/// 写/清待切换状态（仅内容变化时才落盘+广播，见 persist_config_update）。
/// 落盘失败时内存保持旧值，待切换状态未成立，下拍会再次尝试。
fn set_pending_switch(app_handle: &AppHandle, pending: &str) {
    let state = app_handle.state::<AppState>();
    if state.config.load().outbound_pending_switch == pending {
        return;
    }
    persist_config_update(app_handle, |fresh| {
        fresh.outbound_pending_switch = pending.to_string();
    });
}

/// 待切换状态的**清理入口**（排程 None 拍调用，须在 spawn_blocking 线程内）。
///
/// 2026-10-26 起夜间切换不再产生挂起：旧版登录闸在「目标卡可达 Portal 但未登录」时
/// 写入 `waiting-login` 挂起行等监测确认，而校园网 23:00 已断、热点永不可能登录，
/// 于是整夜挂起、切换从不执行（现场事故）。改用外网可达诊断信号后不再有条件能挂起，
/// 故本函数只剩兼容职责——把历史版本留下的残留行清掉（前端徽标依赖它，留着会一直
/// 显示过期状态）。功能关闭与跨日两条路径语义不变，均为"清残留"。
pub(super) fn drive_pending_switch(
    app_handle: &AppHandle,
    config: &crate::config::Config,
) -> Result<(), String> {
    if config.outbound_pending_switch.is_empty() {
        return Ok(());
    }
    crate::log_debug!(
        "outbound",
        "清理历史遗留的待切换状态: {}（本版本夜间切换不再挂起）",
        config.outbound_pending_switch
    );
    set_pending_switch(app_handle, "");
    Ok(())
}

/// 出站切换计划：根据本轮健康结果选择目标，并记录可还原的跃点。
/// 切换只写跃点（`SetIpInterfaceEntry`），**不禁用任何网卡**——禁卡会切断该卡、
/// 使其无法继续登录，而只改跃点在真机实测 37–125ms 内即可完成出口迁移。
struct OutboundSwitchPlan {
    /// 目标卡（出站切换的落点）
    target_name: String,
    target_guid: String,
    /// 目标卡接口索引，用于切换后验证默认出口确实落在这张卡上
    target_if_index: u32,
    /// 目标卡跃点行（构建计划时读到的原值；执行阶段读不到现值时作兜底快照）
    rows: Vec<MetricRow>,
}

/// 构建夜间出站切换计划：**纯只读**（选卡 + 绑源探测 + 读跃点 + 算禁用名单），
/// 不做任何系统写入，也不落盘。
///
fn build_outbound_plan(
    app_handle: &AppHandle,
    config: &crate::config::Config,
    internet_health: Option<std::collections::HashMap<String, bool>>,
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
    // 候选来自出站优先级生效列表（用户已排序按原序；未排序默认无线卡优先，
    // 见 effective_outbound_priority），且必须有 IP 且判定为非校园网
    let portal_host = portal_probe_host(&config.portal_url);
    let priority = effective_outbound_priority(&config.outbound_priority, &candidates);
    // 并行预热本轮判定的全部绑源探测（逐卡串行探测是切换耗时主体，见 ProbeCache）
    let mut probe_cache = warm_probe_cache(&collect_probe_targets(
        &priority,
        &candidates,
        &config.campus_gateway,
        &portal_host,
    ));
    let internet_health = match internet_health {
        Some(health) => health,
        None => super::outbound_failover::probe_internet_access(app_handle, config, &candidates, None),
    };
    for (source_ip, ok) in internet_health {
        probe_cache.0.insert((OUTBOUND_INTERNET_HEALTH_KEY.to_string(), source_ip), ok);
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
        crate::log_warn!("outbound", "夜间出站切换: 优先级内无可用非校园网候选，本拍不切换");
        notify_outbound_issue_once_per_day(
            app_handle,
            &OUTBOUND_NO_CANDIDATE_WARN_DAY,
            "夜间出站切换未生效",
            "未找到可用的非校园网网卡（可能被误判为校园网或网关不可达），出站未切换",
        );
        return Err("没有可用的非校园网网卡（可能被误判为校园网或网关不可达）".to_string());
    };
    if target.guid.is_empty() {
        crate::log_warn!("outbound", "夜间出站切换: 目标卡 {} 无 GUID，无法设置跃点", target.name);
        return Err(format!("目标网卡 {} 缺少 GUID，无法设置跃点", target.name));
    }
    crate::log_info!("outbound", "出站切换: 目标卡 {} 已通过双目标绑源 TCP/HTTPS 公网复核", target.name);
    // 只处理 IPv4+IPv6 两族（helper 的 set_metric 只认 2/23）
    let rows: Vec<MetricRow> = match crate::platform::metric::read_interface_metrics(&target.guid) {
        Ok(rows) => rows.into_iter().filter(|r| matches!(r.family, 2 | 23)).collect(),
        Err(e) => {
            crate::log_warn!("outbound", "夜间出站切换: 读取 {} 跃点失败: {e}", target.name);
            return Err(format!("读取网卡 {} 跃点配置失败: {e}", target.name));
        }
    };
    if rows.is_empty() {
        crate::log_warn!("outbound", "夜间出站切换: {} 无 IPv4/IPv6 跃点行，本拍不切换", target.name);
        return Err(format!("网卡 {} 没有 IPv4/IPv6 跃点配置行", target.name));
    }
    Ok(OutboundSwitchPlan {
        target_name: target.name,
        target_guid: target.guid,
        target_if_index: target.if_index,
        rows,
    })
}

pub(super) fn apply_outbound_switch_with_health(
    app_handle: &AppHandle,
    config: &crate::config::Config,
    internet_health: Option<std::collections::HashMap<String, bool>>,
) -> Result<(), String> {
    let started = std::time::Instant::now();
    let Some(_busy) = OutboundActionGuard::try_acquire() else {
        crate::log_debug!("outbound", "夜间出站切换: 另一出站动作进行中，本拍跳过");
        return Err("另一出站动作进行中，请稍后再试".to_string());
    };
    if !switch_gate_open() {
        crate::log_debug!("outbound", "夜间出站切换: 退避窗内，本拍跳过");
        return Err("正处于自动重试退避窗口，请稍后再试".to_string());
    }
    let mut plan = build_outbound_plan(app_handle, config, internet_health)?;
    // 跃点现值复核：还原快照必须以执行时刻现值为准；读不到时才使用计划里的原值
    match crate::platform::metric::read_interface_metrics(&plan.target_guid) {
        Ok(rows) => {
            let fresh: Vec<MetricRow> = rows.into_iter().filter(|r| matches!(r.family, 2 | 23)).collect();
            if fresh.is_empty() {
                crate::log_warn!("outbound", "夜间出站切换: {} 无 IPv4/IPv6 跃点行，本拍不切换", plan.target_name);
                return Err(format!("网卡 {} 没有 IPv4/IPv6 跃点配置行", plan.target_name));
            }
            plan.rows = fresh;
        }
        Err(e) => {
            if plan.rows.is_empty() {
                crate::log_warn!("outbound", "夜间出站切换: 读取 {} 跃点失败: {e}", plan.target_name);
                return Err(format!("读取网卡 {} 跃点配置失败: {e}", plan.target_name));
            }
            crate::log_warn!(
                "outbound",
                "夜间出站切换: 读取 {} 跃点现值失败（{e}），改用计划中的原值做快照",
                plan.target_name
            );
        }
    }
    // 清掉历史版本遗留的待切换状态（旧登录闸写入的 waiting-login 行）：本版本不再
    // 产生挂起，残留只会让前端一直显示过期徽标，随后的切换落盘也会一并覆盖。
    set_pending_switch(app_handle, "");
    // 先落快照：落盘成功才认定切换态成立（沿夜切"落盘先行"语义，失败不 store 内存）。
    // 兜底路由字段不再写入：2026-10-18 事故后该能力废弃（同网关+接口的默认路由无法与
    // 目标卡自己的 DHCP 默认路由共存，Windows 判重复 → route add 每夜恒失败重放），
    // 历史值保留给还原步骤①按四元组精确清理，清空会让历史残留无人认领
    let state = app_handle.state::<AppState>();
    let metric_restore = snapshot_json(&plan.target_guid, &plan.rows);
    if let Err(e) = crate::config::persist::mutate_config_and_save(app_handle, &state, move |fresh| {
        fresh.outbound_metric_restore = metric_restore;
        Ok(())
    }) {
        crate::log_warn!("outbound", "夜间出站切换: 切换态快照落盘失败: {e}");
        return Err(format!("切换状态快照写入失败: {e}"));
    }
    crate::log_info!(
        "outbound",
        "夜间出站切换: 目标卡 {} ({}), 已记录两族跃点快照",
        plan.target_name,
        plan.target_guid,
    );
    let families: Vec<u16> = plan.rows.iter().map(|r| r.family).collect();
    // 旁路路由必须在改跃点**之前**构造：改完之后默认出口已是新卡，再读到的网关
    // 就是新卡的，会把去 Portal 的流量钉在错误的卡上。
    let portal_bypass = build_portal_bypass_route(config);
    let mut all_ok = true;
    let mut fail_reasons: Vec<String> = Vec::new();
    if let Err(reason) = write_outbound_metric(app_handle, &plan.target_name, &plan.target_guid, &families, true, "") {
        all_ok = false;
        fail_reasons.push(format!("设置网卡 {} 跃点失败: {reason}", plan.target_name));
    }
    // 切换后路由级验证：动作成功 ≠ 效果达成。只改跃点时，默认出口必须真的离开
    // 原卡、落到目标卡；若仍不是（典型是 TUN/代理接管了 metric=0 的默认路由），
    // 判失败走退避重放，并明确指出原因，避免"日志说成功、实际没切"。
    if all_ok {
        match outbound_route_targets(plan.target_if_index) {
            Ok(true) => crate::log_info!("outbound", "夜间出站切换: 路由验证通过，默认出口已指向 {}", plan.target_name),
            Ok(false) => {
                all_ok = false;
                mark_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
                crate::log_warn!(
                    "outbound",
                    "夜间出站切换: 路由验证失败——默认出口仍不在 {}（可能被 TUN/代理接管了默认路由）",
                    plan.target_name
                );
                fail_reasons.push(format!(
                    "默认出口仍未切到 {}（可能被 TUN/代理接管默认路由）",
                    plan.target_name
                ));
            }
            Err(e) => crate::log_debug!("outbound", "夜间出站切换: 路由验证跳过: {e}"),
        }
    }
    // Portal 旁路：默认出口已切到新卡，但登录请求仍需到达校园 Portal——给原卡加一条
    // /32 主机路由，让这部分流量绕回原卡，其余流量照常走新出口。添加失败不阻断切换
    // （系统出口已切好），只记录原因，登录恢复失败时日志能对上。
    if all_ok {
        match portal_bypass {
            Some(portal_route) => {
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
                        if let Err(e) = crate::config::persist::mutate_config_and_save(app_handle, &state, move |fresh| {
                            fresh.outbound_portal_route = json;
                            Ok(())
                        }) {
                            crate::log_warn!("outbound", "夜间出站切换: 旁路路由快照落盘失败: {e}");
                        }
                    }
                    Err(failure) => {
                        note_channel_failure(&failure);
                        crate::log_warn!("outbound", "夜间出站切换: Portal 旁路路由添加失败: {}", failure.reason);
                    }
                }
            }
            None => crate::log_debug!("outbound", "夜间出站切换: 无 Portal 主机或原出口网关，跳过旁路路由"),
        }
    }
    if all_ok {
        crate::log_info!(
            "outbound",
            "夜间出站切换: 动作完成，总耗时 {}ms",
            started.elapsed().as_millis()
        );
        Ok(())
    } else {
        Err(if fail_reasons.is_empty() { "切换未完成".to_string() } else { fail_reasons.join("；") })
    }
}

/// 补齐/重放切换动作：按快照把"切换没写完"的部分补齐，两个分支各自独立、
/// 全部幂等（可反复调用），任一分支失败不影响其余分支：
/// ① 跃点补写：只写"快照记录过原值 + 当前仍存在"的协议栈的 Metric=1，不动快照原值；
/// ② 禁用补做：对名单内且当前存在的校园网卡重发 disable（已在禁用态的卡重复执行无副作用，
///    卡已拔出/消失则跳过）。
/// （原"③ 兜底路由补加"分支已随 2026-10-18 事故删除：该路由在 Windows 上无法与目标卡
/// 自己的 DHCP 默认路由共存，route add 恒报「文件已存在」，详见 platform/route_table。）
/// 两个入口共用：
/// ① 循环内切换态下的"未生效补齐"（helper 部分失败后的退避重试；循环已用
///    [`needs_replay`] 判过失败历史，此处不再判）；
/// ② 启动对账的夜间窗口分支（上次运行被杀，切换没写完——此时进程内失败计数为 0，
///    不能以 `needs_replay` 拦，否则重启后的重放失效）。
/// 成功只记 info 日志、**不发系统通知**（稳态/多拍重放不应重复弹窗）。
/// 全部分支无新增失败时清切换失败计数，让 needs_replay 回稳态 false
/// （否则任一遗留失败史都会让每拍 60s 重放 + 提权空转直到天亮）。
pub(super) fn replay_outbound_switch(app_handle: &AppHandle, config: &crate::config::Config) {
    if !switch_gate_open() {
        return;
    }
    // 与手动入口/还原互斥：重放会写 metric=1 并禁用校园卡，若与还原并发，最坏是
    // 还原清完快照后重放又把跃点写回 1（无快照可还原的静默丢失）。拿不到闸就记一次
    // 切换失败：启动对账这一发失败计数为 0，不记则 needs_replay 永假、这一发被静默
    // 丢掉；记了它就是一次普通失败，退避窗后由循环补做。
    let Some(_busy) = OutboundActionGuard::try_acquire() else {
        mark_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
        crate::log_debug!("outbound", "夜间出站切换: 另有出站动作进行中，本次重放顺延");
        return;
    };
    let mut any_fail = false;
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
                // 读取失败只影响本分支：记失败后跳过补写，落回②③（不整体 return）
                any_fail = true;
                crate::log_warn!("outbound", "夜间出站切换: 读取目标卡({guid})跃点失败: {e}");
                Vec::new()
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
        if !families.is_empty() && write_outbound_metric(app_handle, &name, &guid, &families, false, "").is_err() {
            any_fail = true;
        }
    }
    // 全部分支无新增失败：此前的失败已被补齐动作补救，清掉切换失败计数，
    // needs_replay 回稳态 false（否则每拍 60s 重放 + 提权空转直到天亮）
    if !any_fail {
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
pub(super) fn apply_outbound_restore(app_handle: &AppHandle, config: &crate::config::Config) -> Result<(), String> {
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
            crate::log_warn!("outbound", "夜间出站还原: 跃点快照解析失败，按已还原继续: {e}");
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
        crate::log_warn!("outbound", "夜间出站还原: 兜底路由快照解析失败，跳过清理（runtime 路由重启即消失）");
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
            // 系统重建该协议栈时按其默认值（与"已还原"同语义）；现值已与快照一致的
            // 协议栈不再重复提权写入——重试不会再弹 UAC，也不会把已还原的步骤反复执行
            let current = crate::platform::metric::read_interface_metrics(&guid).unwrap_or_default();
            let present = snapshot
                .iter()
                .filter(|r| current.iter().any(|c| c.family == r.family))
                .count();
            let encoded: Vec<String> = snapshot
                .iter()
                .filter(|r| {
                    current.iter().any(|c| {
                        c.family == r.family
                            && (c.metric != r.metric || c.automatic != r.automatic)
                    })
                })
                .map(|r| format!("{}:{}:{}:{}", r.guid, r.family, u8::from(r.automatic), r.metric))
                .collect();
            if present == 0 {
                crate::log_info!("outbound", "夜间出站还原: 目标卡跃点行已消失，视为已还原");
            } else if encoded.is_empty() {
                crate::log_info!("outbound", "夜间出站还原: 目标卡跃点已与快照一致，跳过写入");
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
                        fail_reasons.push(format!("恢复网卡跃点失败: {}", failure.reason));
                    }
                }
            }
        }
    }
    // Portal 旁路路由：还原后原卡重新成为默认出口，这条 /32 路由必须删掉，
    // 否则它会继续把去 Portal 的流量钉在原卡网关上（原卡换 IP 后会变成黑洞）。
    if let Some(portal_route) = parse_portal_route(&config.outbound_portal_route) {
        match run_helper_op("route_delete", &[portal_route_delete_row(&portal_route)], outbound_allow_uac()) {
            Ok(()) => crate::log_info!(
                "outbound",
                "夜间出站还原: 已删除 Portal 旁路路由 {}",
                portal_route.dest
            ),
            Err(failure) => {
                note_channel_failure(&failure);
                route_ok = false;
                crate::log_warn!("outbound", "夜间出站还原: 删除 Portal 旁路路由失败: {}", failure.reason);
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
            let count = mark_outbound_failure(&OUTBOUND_RESTORE_LAST_FAIL_MS, &OUTBOUND_RESTORE_FAIL_COUNT);
            crate::log_warn!("outbound", "夜间出站还原: 禁用名单快照解析失败，保留切换态重试: {e}");
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
            match enable_campus_adapter(&row.name, allow_uac) {
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
                    fail_reasons.push(format!("启用校园网卡 {} 失败: {}", row.name, failure.reason));
                }
            }
        }
    }
    if !(metric_ok && enable_ok && route_ok) {
        return Err(if fail_reasons.is_empty() { "还原未完成".to_string() } else { fail_reasons.join("；") });
    }
    // 收尾通知只发一次（全部步骤完成后）：文案区分是否真的启用了网卡；
    // 放弃阀放行的拍不发「已还原」——校园卡可能仍被 netsh 持久禁用，
    // 改发告警文案提醒手动启用，与此前「请手动启用」告警口径一致
    if gave_up_released {
        crate::log_info!("outbound", "夜间出站还原: 已按放弃阀放行收尾，校园网卡启用状态待人工确认");
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
        && current.outbound_pending_switch.is_empty()
    {
        return true;
    }
    drop(current);
    if let Err(e) = crate::config::persist::mutate_config_and_save(app_handle, &state, |fresh| {
        fresh.outbound_metric_restore = String::new();
        fresh.outbound_disabled_adapters = String::new();
        fresh.outbound_standby_route = String::new();
        fresh.outbound_portal_route = String::new();
        fresh.outbound_pending_switch = String::new();
        Ok(())
    }) {
        crate::log_warn!("outbound", "夜间出站切换: 切换态快照清空落盘失败: {e}");
        return false;
    }
    true
}

/// 「立即切换」当日保持标记落盘（hold 只影响自动排程冻结；写路径与
/// clear_outbound_snapshot 同款：load fresh → 改字段 → 加密落盘成功才 store 内存）。
/// 落盘失败仅告警不阻断——最坏后果是当日 23:00 自动切换与手动切换叠加
/// （apply 幂等性保障下可接受），不应因此拒绝手动切换。
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

/// 「立即切换」命令入口（桌面）：用户显式触发，覆盖自动切换的退避闸。先落 hold
/// 再执行切换（hold 落盘后即使切换中途崩溃，当日的自动 Switch 也不会叠加，
/// 残留快照由启动对账收敛）。已处于切换态时拒绝——此时重跑会把已被改动的
/// 跃点当原值重新快照，覆盖原始跃点导致还原写入错误值。
pub(crate) fn manual_outbound_switch(app_handle: &AppHandle) -> Result<(), String> {
    let config = app_handle.state::<AppState>().config.load_full();
    if !config.enable_night_outbound_switch {
        return Err("夜间出站切换未开启".to_string());
    }
    if outbound_switch_active(app_handle) {
        return Err("已处于出站切换状态，如需重试请先还原".to_string());
    }
    let today = chrono::Local::now().date_naive().num_days_from_ce();
    // 用户显式重试：清退避闸，不做自动路径的阶梯等待
    clear_outbound_failure(&OUTBOUND_SWITCH_LAST_FAIL_MS, &OUTBOUND_SWITCH_FAIL_COUNT);
    set_outbound_manual_hold(app_handle, today);
    if let Err(reason) = apply_outbound_switch_with_health(app_handle, &config, None) {
        // 切换失败（无候选/退避等）按原语义清 hold，恢复当日自动切换。
        // 2026-10-26 前还有「登录闸挂起不算失败、保留 hold 让补驱动接管」的分支；
        // 挂起机制已随登录闸一起移除，失败即失败。
        clear_outbound_manual_hold(app_handle);
        Err(reason)
    } else {
        Ok(())
    }
}

/// 「立即还原」命令入口（桌面）：先清 hold 与还原退避（用户显式重试覆盖退避闸）
/// 再执行还原；还原未完成时快照仍在，自动还原会在后续拍继续重试。
pub(crate) fn manual_outbound_restore(app_handle: &AppHandle) -> Result<(), String> {
    let config = app_handle.state::<AppState>().config.load_full();
    if !outbound_switch_active(app_handle) {
        return Err("当前未处于出站切换状态".to_string());
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
        crate::log_info!("outbound", "启动对账: 目标卡 {} 已不可用或回到校园网，按还原处理", adapter.name);
        let _ = apply_outbound_restore(app_handle, &config);
        return;
    }
    crate::log_info!("outbound", "启动对账: 夜间窗口内重放未完成的出站切换(目标卡 {})", adapter.name);
    replay_outbound_switch(app_handle, &config);
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let campus = (adapter("以太网", "10.2.100.5", false), "10.2.127.254".to_string());
        let hotspot = (adapter("WLAN", "192.168.6.119", true), "192.168.6.1".to_string());
        let no_gw = (adapter("以太网 2", "192.168.6.115", false), String::new());
        let pairs = collect_probe_targets(
            &["以太网".to_string(), "WLAN".to_string(), "以太网 2".to_string()],
            &[campus, hotspot, no_gw],
            "10.2.127.254",
            "10.1.99.100",
        );
        let has = |dest: &str, src: &str| {
            pairs.contains(&(dest.to_string(), src.to_string()))
        };
        // 校园卡不再靠网段短路：校园网关与 portal 同样要预热
        assert!(has("10.2.127.254", "10.2.100.5"));
        assert!(has("10.1.99.100", "10.2.100.5"));
        assert!(!has(OUTBOUND_INTERNET_HEALTH_KEY, "10.2.100.5"), "公网健康由 TCP/HTTPS 单独并行，不再发 ICMP");
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
        let outside = (adapter("WLAN", "192.168.6.119", true), "192.168.6.1".to_string());
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
