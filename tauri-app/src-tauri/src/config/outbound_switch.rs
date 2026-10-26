//! 夜间出站自动切换的命中判定（纯函数，跨平台可见面）。
//!
//! 与运营商夜切（night_switch）共用时间表，但动作语义正交：本模块只产出
//! Switch/Restore 决策，"切到哪张卡（桌面）"与"注销+重检（安卓）"由各端
//! 调用方实现。切换态由配置自身承载（restore 快照/标记非空），幂等无需去重标记。

use super::night_switch::{switch_time_for, RESTORE_END_MINUTES};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NightOutboundAction {
    /// 本拍无需任何动作（含：夜间窗口内已处于切换态）
    None,
    /// 桌面：把目标卡 metric 调至 1 并写快照；安卓：注销 + report_wifi_unusable 并写标记
    Switch,
    /// 桌面：按快照还原 metric；安卓：登录当前账号并清标记
    Restore,
}

/// 夜间出站切换判定（纯函数，恢复起点固定默认 07:30）。
///
/// **仅测试期可用**：生产代码一律调 [`evaluate_night_outbound_with_restore`] 并显式
/// 传入 `nightRestoreMinutes`——恢复时刻已可配置，生产路径走默认常量包装等于把
/// 用户设置写死。留它只为让既有用例保持可读。
#[cfg(test)]
pub fn evaluate_night_outbound(enabled: bool, weekday: u32, now_minutes: u32, restore_active: bool) -> NightOutboundAction {
    evaluate_night_outbound_with_restore(
        enabled,
        weekday,
        now_minutes,
        restore_active,
        super::night_switch::DEFAULT_RESTORE_START_MINUTES,
    )
}

/// 带自定义恢复起点的判定。`restore_start` 可传原始配置值，函数内部经
/// [`super::night_switch::restore_start_minutes`] 归一化（该函数幂等）。
pub fn evaluate_night_outbound_with_restore(
    enabled: bool,
    weekday: u32,
    now_minutes: u32,
    restore_active: bool,
    restore_start: u32,
) -> NightOutboundAction {
    let restore_start = super::night_switch::restore_start_minutes(restore_start);
    if !enabled {
        return NightOutboundAction::None;
    }
    if restore_active {
        if (restore_start..RESTORE_END_MINUTES).contains(&now_minutes) {
            return NightOutboundAction::Restore;
        }
        return NightOutboundAction::None;
    }
    if now_minutes < restore_start {
        return NightOutboundAction::Switch;
    }
    match switch_time_for(weekday) {
        Some(t) if now_minutes >= t => NightOutboundAction::Switch,
        _ => NightOutboundAction::None,
    }
}

/// 夜间出站切换的守护窗口判定（纯函数，桌面巡检闸门用，恢复起点固定默认 07:30）：
/// 当日切换时刻（含过点补触发语义）起，至次日恢复窗开为止。
/// 用户在切换前手动禁用的手选适配器无 IP、进不了六期闸门名单
/// （select 只收有 IP 的卡），名单为空时闸门失效，巡检自动启用会顶掉
/// 切换（2026-09-27 夜实证）；窗口内巡检一律不出自动启用目标，还原交给
/// 恢复窗的 scheduled.rs。凌晨分支（now < 恢复起点）覆盖前一晚切换时刻
/// （23:00/23:30）之后的全部时段，跨日天然成立。
///
/// **仅测试期可用**：生产代码一律调 [`is_night_outbound_guard_window_with_restore`]。
#[cfg(test)]
pub fn is_night_outbound_guard_window(enabled: bool, weekday: u32, now_minutes: u32) -> bool {
    is_night_outbound_guard_window_with_restore(
        enabled,
        weekday,
        now_minutes,
        super::night_switch::DEFAULT_RESTORE_START_MINUTES,
    )
}

/// 带自定义恢复起点的守护窗口判定：窗口止于 `restore_start`（恢复窗一开，
/// 巡检即可正常自动启用，无需再让位于夜切）。`restore_start` 可传原始配置值，
/// 函数内部归一化。
pub fn is_night_outbound_guard_window_with_restore(
    enabled: bool,
    weekday: u32,
    now_minutes: u32,
    restore_start: u32,
) -> bool {
    let restore_start = super::night_switch::restore_start_minutes(restore_start);
    if !enabled {
        return false;
    }
    if now_minutes < restore_start {
        return true;
    }
    switch_time_for(weekday).is_some_and(|t| now_minutes >= t)
}

/// 夜间出站切换的**预判定提前量**（分钟）：切换时刻前 1 分钟开始算计划。
///
/// 动机：到点那一拍要做的绑源 ICMP 探测（每张候选卡 × 网关/portal 各两次）与
/// helper 提权都在秒级，23:00 断网后晚几秒切就等于多掉线几秒。预判定把这些**只读**
/// 计算前置到 T-1 分钟，到点只做落盘 + 提权写入。
pub const OUTBOUND_PREP_LEAD_MINUTES: u32 = 1;

/// 是否处于夜间出站切换的**预判定窗口** `[t - 1min, t)`（纯函数）。
///
/// 判定口径：
/// - 功能关闭 / 已处于切换态（三份快照任一非空）→ 不需要预判定：前者不切，
///   后者等还原（还原侧不做预判定，2026-10-26 用户明确只优化切换侧）；
/// - `now < restore_start` 的凌晨补切分支排除：那一支的 Switch 是**立即**补切
///   （错过的切换），没有提前量可言；
/// - 只有「当日确实有切换时刻 t」且 `t - 1 <= now < t` 才为真。
///
/// tick 为 30s、时刻为分钟粒度，因此窗口内通常落 2 拍：每拍重算、后者覆盖前者
/// （越接近到点越可信，网卡状态在这 1 分钟内也可能变化）。
pub fn in_outbound_prep_window(
    enabled: bool,
    weekday: u32,
    now_minutes: u32,
    restore_active: bool,
    restore_start: u32,
) -> bool {
    let restore_start = super::night_switch::restore_start_minutes(restore_start);
    if !enabled || restore_active || now_minutes < restore_start {
        return false;
    }
    match switch_time_for(weekday) {
        // t - LEAD <= now < t（saturating 防 t=0 的极端配置下溢）
        Some(t) => now_minutes < t && now_minutes.saturating_add(OUTBOUND_PREP_LEAD_MINUTES) >= t,
        None => false,
    }
}

/// 夜间出站切换的切换态判定（纯函数，桌面）：
/// metric 快照 / 禁用名单 / 兜底路由三份快照任一非空即处于切换态。
/// 六期起切换动作=写 metric + 禁校园网卡一次落盘；兜底路由自 2026-10-18 事故后
/// 不再创建，但字段仍参与判定——历史残留要能被识别出来走精确清理，避免只删了
/// metric 快照就把待还原动作漏掉。
pub fn outbound_restore_active(metric_restore: &str, disabled_adapters: &str, standby_route: &str) -> bool {
    !metric_restore.is_empty() || !disabled_adapters.is_empty() || !standby_route.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    // 七期守护窗口：[当日切换时刻, 次日 07:30 恢复窗开)
    #[test]
    fn guard_window_covers_switch_time_to_restore_start() {
        // 周日 23:00 起守护，22:59 不守护
        assert!(is_night_outbound_guard_window(true, 0, 1380));
        assert!(!is_night_outbound_guard_window(true, 0, 1379));
        // 周五/周六 23:30 起，23:29 不守护
        assert!(is_night_outbound_guard_window(true, 5, 1410));
        assert!(!is_night_outbound_guard_window(true, 5, 1409));
        // 凌晨跨日：前一晚切换后凌晨仍守护，07:30 恢复窗开即出窗
        assert!(is_night_outbound_guard_window(true, 0, 0));
        assert!(is_night_outbound_guard_window(true, 6, 449));
        assert!(!is_night_outbound_guard_window(true, 0, 450));
        // 白天不守护
        assert!(!is_night_outbound_guard_window(true, 0, 600));
        // 功能关闭一律不守护
        assert!(!is_night_outbound_guard_window(false, 0, 1380));
        assert!(!is_night_outbound_guard_window(false, 0, 10));
    }

    // 自定义恢复起点的守护窗口：窗口止于恢复起点
    #[test]
    fn guard_window_respects_custom_restore_start() {
        // 恢复起点 06:00（360）：359 仍守护、360 出窗
        assert!(is_night_outbound_guard_window_with_restore(true, 0, 359, 360));
        assert!(!is_night_outbound_guard_window_with_restore(true, 0, 360, 360));
        // 默认起点 450 下 360 仍在窗内
        assert!(is_night_outbound_guard_window_with_restore(true, 0, 360, 450));
    }

    // 自定义恢复起点驱动 Restore 判定
    #[test]
    fn restore_window_follows_custom_start() {
        // 默认 07:30 前不还原；自定义 06:00 后即还原
        assert_eq!(evaluate_night_outbound(true, 0, 400, true), NightOutboundAction::None);
        assert_eq!(
            evaluate_night_outbound_with_restore(true, 0, 400, true, 360),
            NightOutboundAction::Restore
        );
        // 自定义起点下凌晨仍保持切换态（400 属恢复窗内才还原，359 不还原）
        assert_eq!(
            evaluate_night_outbound_with_restore(true, 0, 359, true, 360),
            NightOutboundAction::None
        );
    }

    // 时间表：与运营商夜切共用（0..=4→1380，5|6→1410）
    #[test]
    fn switch_time_follows_operator_night_schedule() {
        assert_eq!(super::super::night_switch::switch_time_for(0), Some(1380)); // 周日
        assert_eq!(super::super::night_switch::switch_time_for(4), Some(1380)); // 周四
        assert_eq!(super::super::night_switch::switch_time_for(5), Some(1410)); // 周五
        assert_eq!(super::super::night_switch::switch_time_for(6), Some(1410)); // 周六
    }

    #[test]
    fn restore_active_covers_all_three_snapshots() {
        // 任一快照非空即切换态
        assert!(outbound_restore_active("[{\"guid\":\"{G}\"}]", "", ""));
        assert!(outbound_restore_active("", "[{\"guid\":\"{G}\",\"name\":\"以太网\"}]", ""));
        assert!(outbound_restore_active("", "", "{\"dest\":\"0.0.0.0\"}"));
        assert!(!outbound_restore_active("", "", ""));
    }

    #[test]
    fn switch_fires_after_time_when_not_in_switched_state() {
        // 周日 23:00 整（1380）：未切换态 → Switch
        assert_eq!(evaluate_night_outbound(true, 0, 1380, false), NightOutboundAction::Switch);
        // 22:59 未到时刻
        assert_eq!(evaluate_night_outbound(true, 0, 1379, false), NightOutboundAction::None);
        // 已处于切换态（restore 非空）不再 Switch
        assert_eq!(evaluate_night_outbound(true, 0, 1380, true), NightOutboundAction::None);
        // 功能关闭
        assert_eq!(evaluate_night_outbound(false, 0, 1380, false), NightOutboundAction::None);
    }

    #[test]
    fn restore_fires_in_restore_window_only() {
        // 07:30 起、restore 非空 → Restore
        assert_eq!(evaluate_night_outbound(true, 0, 450, true), NightOutboundAction::Restore);
        assert_eq!(evaluate_night_outbound(true, 3, 1379, true), NightOutboundAction::Restore);
        // 06:29 未到恢复窗口（切换态凌晨保持，不提前还原）
        assert_eq!(evaluate_night_outbound(true, 0, 449, true), NightOutboundAction::None);
        // 恢复窗口内已还原（restore 空）
        assert_eq!(evaluate_night_outbound(true, 0, 500, false), NightOutboundAction::None);
    }

    #[test]
    fn predawn_missed_switch_is_replayed() {
        // 凌晨（00:00–06:29）未处于切换态 → 视为前一晚窗口尾，补切
        assert_eq!(evaluate_night_outbound(true, 0, 0, false), NightOutboundAction::Switch);
        assert_eq!(evaluate_night_outbound(true, 5, 100, false), NightOutboundAction::Switch);
        assert_eq!(evaluate_night_outbound(true, 0, 449, false), NightOutboundAction::Switch);
        // 切换态凌晨保持（等 07:30 恢复窗），不重复切换
        assert_eq!(evaluate_night_outbound(true, 0, 100, true), NightOutboundAction::None);
        // 功能关闭不补切
        assert_eq!(evaluate_night_outbound(false, 0, 100, false), NightOutboundAction::None);
        // 白天（≥450）未到切点仍不切
        assert_eq!(evaluate_night_outbound(true, 0, 600, false), NightOutboundAction::None);
    }

    // 预判定窗口：切点前 1 分钟那一整分钟（分钟粒度，30s 拍内落 2 拍）
    #[test]
    fn prep_window_covers_minute_before_switch_time() {
        // 周日至周四 23:00（1380）→ 窗口只有 1379（22:59）
        assert!(in_outbound_prep_window(true, 0, 1379, false, 450));
        assert!(in_outbound_prep_window(true, 4, 1379, false, 450));
        // 1378（22:58）太早、1380（23:00）已到点（那拍该直接切，不再预判定）
        assert!(!in_outbound_prep_window(true, 0, 1378, false, 450));
        assert!(!in_outbound_prep_window(true, 0, 1380, false, 450));
        // 周五/周六 23:30（1410）→ 窗口 1409
        assert!(in_outbound_prep_window(true, 5, 1409, false, 450));
        assert!(in_outbound_prep_window(true, 6, 1409, false, 450));
        assert!(!in_outbound_prep_window(true, 5, 1408, false, 450));
        assert!(!in_outbound_prep_window(true, 5, 1410, false, 450));
    }

    #[test]
    fn prep_window_needs_enabled_and_not_switched() {
        // 功能关闭不预判定
        assert!(!in_outbound_prep_window(false, 0, 1379, false, 450));
        // 已处切换态（等还原）不预判定：切换侧已完成，还原侧不做预判定
        assert!(!in_outbound_prep_window(true, 0, 1379, true, 450));
    }

    #[test]
    fn prep_window_never_overlaps_predawn_catch_up_branch() {
        // 凌晨补切分支的条件是 now < 恢复起点，而恢复起点经归一化最大只能到 1379
        // （`restore_start_minutes` 把 ≥1380 一律回落 450）；预判定窗口下界同为 1379
        // （t-1，t 最小 1380）→ 两者在真实取值范围内互斥。这里守住该不变量：
        // 越界恢复起点（1380）归一化回 450，窗口照常成立而非被误判成凌晨分支
        assert_eq!(super::super::night_switch::restore_start_minutes(1380), 450);
        assert!(in_outbound_prep_window(true, 0, 1379, false, 1380));
        // 恢复起点取合法上界 1379 时，1379 已不属于凌晨分支（now >= restore_start）
        assert_eq!(super::super::night_switch::restore_start_minutes(1379), 1379);
        assert!(in_outbound_prep_window(true, 0, 1379, false, 1379));
    }
}
