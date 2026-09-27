//! 夜间出站自动切换的命中判定（纯函数，跨平台可见面）。
//!
//! 与运营商夜切（night_switch）共用时间表，但动作语义正交：本模块只产出
//! Switch/Restore 决策，"切到哪张卡（桌面）"与"注销+重检（安卓）"由各端
//! 调用方实现。切换态由配置自身承载（restore 快照/标记非空），幂等无需去重标记。

use super::night_switch::{switch_time_for, RESTORE_END_MINUTES, RESTORE_START_MINUTES};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NightOutboundAction {
    /// 本拍无需任何动作（含：夜间窗口内已处于切换态）
    None,
    /// 桌面：把目标卡 metric 调至 1 并写快照；安卓：注销 + report_wifi_unusable 并写标记
    Switch,
    /// 桌面：按快照还原 metric；安卓：登录当前账号并清标记
    Restore,
}

/// 夜间出站切换判定（纯函数）：
/// - `!enabled` → None；
/// - `restore_active`（切换态）且 now ∈ [390, 1380) → Restore；
/// - `!restore_active` 且当日有切换时刻且 now >= 时刻（过点补触发）→ Switch；
/// - 其余（午夜后、切换态夜间保持）→ None。
pub fn evaluate_night_outbound(enabled: bool, weekday: u32, now_minutes: u32, restore_active: bool) -> NightOutboundAction {
    if !enabled {
        return NightOutboundAction::None;
    }
    if restore_active {
        if (RESTORE_START_MINUTES..RESTORE_END_MINUTES).contains(&now_minutes) {
            return NightOutboundAction::Restore;
        }
        return NightOutboundAction::None;
    }
    match switch_time_for(weekday) {
        Some(t) if now_minutes >= t => NightOutboundAction::Switch,
        _ => NightOutboundAction::None,
    }
}

/// 夜间出站切换的守护窗口判定（纯函数，桌面巡检闸门用）：
/// 当日切换时刻（含过点补触发语义）起，至次日 06:30 恢复窗开为止。
/// 用户在切换前手动禁用的手选适配器无 IP、进不了六期闸门名单
/// （select 只收有 IP 的卡），名单为空时闸门失效，巡检自动启用会顶掉
/// 切换（2026-09-27 夜实证）；窗口内巡检一律不出自动启用目标，还原交给
/// 恢复窗的 scheduled.rs。凌晨分支（now < 06:30）覆盖前一晚切换时刻
/// （23:00/23:30）之后的全部时段，跨日天然成立。
pub fn is_night_outbound_guard_window(enabled: bool, weekday: u32, now_minutes: u32) -> bool {
    if !enabled {
        return false;
    }
    if now_minutes < RESTORE_START_MINUTES {
        return true;
    }
    switch_time_for(weekday).is_some_and(|t| now_minutes >= t)
}

/// 夜间出站切换的切换态判定（纯函数，桌面）：
/// metric 快照 / 禁用名单 / 兜底路由三份快照任一非空即处于切换态。
/// 六期起切换动作=写 metric + 禁校园网卡 + 加兜底路由三件套一次落盘，
/// 任一残留都说明有待还原动作，避免 background_check/adapter_watch 只看
/// metric 快照漏判。
pub fn outbound_restore_active(metric_restore: &str, disabled_adapters: &str, standby_route: &str) -> bool {
    !metric_restore.is_empty() || !disabled_adapters.is_empty() || !standby_route.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    // 七期守护窗口：[当日切换时刻, 次日 06:30 恢复窗开)
    #[test]
    fn guard_window_covers_switch_time_to_restore_start() {
        // 周日 23:00 起守护，22:59 不守护
        assert!(is_night_outbound_guard_window(true, 0, 1380));
        assert!(!is_night_outbound_guard_window(true, 0, 1379));
        // 周五/周六 23:30 起，23:29 不守护
        assert!(is_night_outbound_guard_window(true, 5, 1410));
        assert!(!is_night_outbound_guard_window(true, 5, 1409));
        // 凌晨跨日：前一晚切换后凌晨仍守护，06:30 恢复窗开即出窗
        assert!(is_night_outbound_guard_window(true, 0, 0));
        assert!(is_night_outbound_guard_window(true, 6, 389));
        assert!(!is_night_outbound_guard_window(true, 0, 390));
        // 白天不守护
        assert!(!is_night_outbound_guard_window(true, 0, 600));
        // 功能关闭一律不守护
        assert!(!is_night_outbound_guard_window(false, 0, 1380));
        assert!(!is_night_outbound_guard_window(false, 0, 10));
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
        // 06:30 起、restore 非空 → Restore
        assert_eq!(evaluate_night_outbound(true, 0, 390, true), NightOutboundAction::Restore);
        assert_eq!(evaluate_night_outbound(true, 3, 1379, true), NightOutboundAction::Restore);
        // 06:29 未到恢复窗口（午夜后不补触发，沿用既有边界）
        assert_eq!(evaluate_night_outbound(true, 0, 389, true), NightOutboundAction::None);
        // 恢复窗口内已还原（restore 空）
        assert_eq!(evaluate_night_outbound(true, 0, 500, false), NightOutboundAction::None);
    }
}
