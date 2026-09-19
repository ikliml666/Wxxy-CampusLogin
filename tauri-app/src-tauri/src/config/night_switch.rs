//! 晚间断网自动切换运营商的命中判定（纯函数，跨平台可见面）。
//!
//! 校园网运营商服务在周日晚/周一晚 23:00、周五晚/周六晚 23:30 下线，届时需把
//! 登录运营商切至"无锡学院"（operator = 空串）才能继续上网，次日早晨再切回。
//! 判定与"谁调用、状态存哪"完全解耦：切换态由配置自身承载
//! （`night_operator_restore` 非空 + `operator` 为空 = 当前处于无锡学院切换态），
//! 因此无需当日去重标记，天然幂等——重复调用不会重复写配置。

/// 恢复窗口起点 06:30（含）
const RESTORE_START_MINUTES: u32 = 390;
/// 恢复窗口终点 23:00（不含，恰为最早的切换时刻，两窗口无缝衔接）
const RESTORE_END_MINUTES: u32 = 1380;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NightSwitchAction {
    /// 本拍无需任何动作
    None,
    /// 把当前 operator 暂存进 night_operator_restore，operator 置空串并触发登录
    SwitchToCampus,
    /// operator 恢复为 night_operator_restore 暂存值，restore 清空并触发登录
    Restore,
}

/// 当日切换时刻：weekday 0=周日 … 6=周六；周日/周一 1380（23:00），
/// 周五/周六 1410（23:30），其余日运营商服务不下线、无切换时刻
fn switch_time_for(weekday: u32) -> Option<u32> {
    match weekday {
        0 | 1 => Some(1380),
        5 | 6 => Some(1410),
        _ => None,
    }
}

/// 参与夜切的运营商白名单：仅三家 ISP 后缀会在晚间断网，需要切至无锡学院；
/// 空串表示已处于无锡学院切换态。非白名单非空值（如账号档案历史脏值"校园"）
/// 不属于断网 ISP，不触发切换。
fn is_switchable_operator(operator: &str) -> bool {
    matches!(operator, "@telecom" | "@unicom" | "@cmcc")
}

/// 晚间断网切换判定（纯函数）：
/// - `!enabled`：功能关闭，一律 None；
/// - 恢复：`restore_operator` 非空（处于切换态）且当前 operator 为空串（确实在
///   无锡学院态，用户手动切回的场景不误恢复）且 `now_minutes` 落在恢复窗口
///   `[06:30, 23:00)` 内 → Restore；
/// - 切换：当日有切换时刻且 `now_minutes >= 时刻`（含过点补触发，循环拍间隔/
///   系统休眠可能错过精确分钟）且当前 operator 为白名单 ISP 后缀
///   （`is_switchable_operator`：@telecom/@unicom/@cmcc；已是无锡学院（空串）
///   无需切，非白名单脏值如"校园"不属于断网 ISP，不切）→ SwitchToCampus；
/// - 其余 → None。
pub fn evaluate_night_switch(
    enabled: bool,
    weekday: u32,
    now_minutes: u32,
    current_operator: &str,
    restore_operator: &str,
) -> NightSwitchAction {
    if !enabled {
        return NightSwitchAction::None;
    }
    if !restore_operator.is_empty()
        && current_operator.is_empty()
        && (RESTORE_START_MINUTES..RESTORE_END_MINUTES).contains(&now_minutes)
    {
        return NightSwitchAction::Restore;
    }
    if let Some(switch_at) = switch_time_for(weekday) {
        if now_minutes >= switch_at && is_switchable_operator(current_operator) {
            return NightSwitchAction::SwitchToCampus;
        }
    }
    NightSwitchAction::None
}

#[cfg(test)]
mod tests {
    use super::{evaluate_night_switch, NightSwitchAction};
    use NightSwitchAction::*;

    const TELECOM: &str = "@telecom";

    fn eval(enabled: bool, weekday: u32, now: u32, current: &str, restore: &str) -> NightSwitchAction {
        evaluate_night_switch(enabled, weekday, now, current, restore)
    }

    #[test]
    fn 开关关闭_一律不触发() {
        assert_eq!(eval(false, 0, 1380, TELECOM, ""), None);
        assert_eq!(eval(false, 0, 600, "", TELECOM), None);
    }

    #[test]
    fn 周日1380恰好命中_触发切换() {
        assert_eq!(eval(true, 0, 1380, TELECOM, ""), SwitchToCampus);
    }

    #[test]
    fn 周日1379_差一分钟不切换() {
        assert_eq!(eval(true, 0, 1379, TELECOM, ""), None);
    }

    #[test]
    fn 周一1380与周五周六1410命中_周二不切换() {
        assert_eq!(eval(true, 1, 1380, TELECOM, ""), SwitchToCampus);
        assert_eq!(eval(true, 5, 1410, TELECOM, ""), SwitchToCampus);
        assert_eq!(eval(true, 6, 1410, TELECOM, ""), SwitchToCampus);
        // 周二~周四无切换时刻
        assert_eq!(eval(true, 2, 1380, TELECOM, ""), None);
        assert_eq!(eval(true, 3, 1439, TELECOM, ""), None);
        assert_eq!(eval(true, 4, 1439, TELECOM, ""), None);
    }

    #[test]
    fn 周五1409_差一分钟不切换() {
        assert_eq!(eval(true, 5, 1409, TELECOM, ""), None);
    }

    #[test]
    fn 恢复窗口内_切换态_恢复() {
        // 窗口起点 06:30、午间、终点前一分钟
        assert_eq!(eval(true, 1, 390, "", TELECOM), Restore);
        assert_eq!(eval(true, 2, 600, "", TELECOM), Restore);
        assert_eq!(eval(true, 6, 1379, "", TELECOM), Restore);
    }

    #[test]
    fn 恢复窗口外_不恢复() {
        // 06:29 尚未到窗口
        assert_eq!(eval(true, 1, 389, "", TELECOM), None);
        // 23:00 已出窗口（且恰好是切换时刻，但 operator 为空不重复切）
        assert_eq!(eval(true, 0, 1380, "", TELECOM), None);
        // 次日凌晨（周日 00:10）不恢复
        assert_eq!(eval(true, 0, 10, "", TELECOM), None);
    }

    #[test]
    fn restore为空_未处于切换态_不恢复() {
        assert_eq!(eval(true, 1, 600, "", ""), None);
        assert_eq!(eval(true, 1, 600, TELECOM, ""), None);
    }

    #[test]
    fn 当前非空_不在无锡学院态_不恢复() {
        assert_eq!(eval(true, 1, 600, TELECOM, TELECOM), None);
    }

    #[test]
    fn 已切换态_过了切换时刻_不重复切换() {
        // 周日 23:01 已处于无锡学院态：既不在恢复窗口也不满足切换条件，幂等返回 None
        assert_eq!(eval(true, 0, 1381, "", TELECOM), None);
        assert_eq!(eval(true, 0, 1439, "", TELECOM), None);
    }

    #[test]
    fn 非白名单运营商_到点不触发切换() {
        // 账号档案历史脏值与任意非法后缀均不属于断网 ISP,不参与夜切
        assert_eq!(eval(true, 0, 1380, "校园", ""), None);
        assert_eq!(eval(true, 5, 1410, "@foo", ""), None);
    }

    #[test]
    fn 白名单三家运营商_到点均正常切换() {
        assert_eq!(eval(true, 0, 1380, "@telecom", ""), SwitchToCampus);
        assert_eq!(eval(true, 0, 1380, "@unicom", ""), SwitchToCampus);
        assert_eq!(eval(true, 6, 1410, "@cmcc", ""), SwitchToCampus);
    }

    #[test]
    fn 已切换态_窗口内每拍持续判定为恢复_幂等不重复写配置() {
        // 判定语义：窗口内重复调用始终返回 Restore，由调用方在恢复成功后
        // 清空 restore 使下一拍回到 None，保证写配置只发生一次
        let first = eval(true, 1, 390, "", TELECOM);
        assert_eq!(first, Restore);
        // 模拟调用方已清空 restore 后的下一拍
        assert_eq!(eval(true, 1, 390, "", ""), None);
    }
}
