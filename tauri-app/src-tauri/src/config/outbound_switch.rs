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

#[cfg(test)]
mod tests {
    use super::*;

    // 时间表：与运营商夜切共用（0..=4→1380，5|6→1410）
    #[test]
    fn switch_time_follows_operator_night_schedule() {
        assert_eq!(super::super::night_switch::switch_time_for(0), Some(1380)); // 周日
        assert_eq!(super::super::night_switch::switch_time_for(4), Some(1380)); // 周四
        assert_eq!(super::super::night_switch::switch_time_for(5), Some(1410)); // 周五
        assert_eq!(super::super::night_switch::switch_time_for(6), Some(1410)); // 周六
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
