//! 定时编排拍的单点判定（定时登录/注销 + 夜间出站切换 + 运营商夜切）。
//!
//! 桌面 [`crate::monitor::scheduled`] 与安卓端监控循环各自执行副作用（登录/注销/
//! 改跃点/注销等蜂窝/落盘），本模块只承载「给定输入 → 应执行动作」的纯判定：
//! 零 IO、零协议逻辑、零 Tauri 依赖，双端共用同一实现与同一组测试，保证编排
//! 语义单点一致。
//!
//! 切换态快照时点约定（结构性差异，非缺陷，见任务回执「语义差异清单」）：
//! - 出站动作判定用**拍开始**的切换态——出站动作尚未执行，快照自然取自拍前；
//! - 运营商夜切门控用**出站动作执行后**的切换态——本拍出站动作 Switch/Restore
//!   成功会改变切换态，夜切必须让位（spec §2：出站动作先行）；
//! - 定时登录门控的切换态由调用方按各自循环结构注入：桌面定时判定先于出站动作
//!   （传拍前值），安卓定时判定在出站动作之后（传执行后最新值）。两端时点不同
//!   是既有结构差异，统一任一侧都会改变某端行为。
//!
//! 状态源差异同样由调用方收口：切换态 `bool` 由调用方从各自的承载推导（桌面内存
//! 快照三字段 / 安卓 `night_outbound_restore` 配置标记），本模块不感知存储形态。

use super::night_switch::{evaluate_night_switch_with_restore, NightSwitchAction};
use super::outbound_switch::{evaluate_night_outbound_with_restore, NightOutboundAction};
use super::schedule::should_fire_scheduled_action;

/// 定时动作判定（双端单点）：命中返回 (应登录, 应注销)，不置任何标记——
/// 当日额度标记由执行器在动作抢锁/发起成功后落（审计 A7 语义）。出站切换态
/// （`outbound_active`）只门控定时登录：切换态下校园网 portal 从热点出站不可达，
/// 登录注定失败，返回 `false` 且调用方不置标记（跳过的拍不消耗额度，
/// 「过点补触发」在还原成功后的下一拍补上）；定时注销无害，不受切换态影响。
#[allow(clippy::too_many_arguments)] // 纯函数透传判定所需的 7 个入参，拆结构体只为少两个参数不值
pub fn evaluate_scheduled_actions(
    now_minutes: u16,
    today_day: i32,
    login_target_minutes: u16,
    logout_target_minutes: u16,
    login_fired_day: i32,
    logout_fired_day: i32,
    outbound_active: bool,
) -> (bool, bool) {
    let fire_login = !outbound_active
        && should_fire_scheduled_action(now_minutes, login_target_minutes, login_fired_day, today_day);
    let fire_logout =
        should_fire_scheduled_action(now_minutes, logout_target_minutes, logout_fired_day, today_day);
    (fire_login, fire_logout)
}

/// 出站动作判定（双端单点）：在共享纯函数 [`evaluate_night_outbound`] 之上叠加
/// 两条编排策略——
/// ① 功能已关闭但切换态残留（用户中途关掉开关、或导入了带切换态的旧配置）时
/// 按 Restore 处理立即还原，不留悬挂的跃点/蜂窝改动；清理分支优先且不受 hold
/// 冻结（残留改动没有出口会永久压住巡检）；
/// ② 「立即切换」当日保持（桌面独有输入，安卓无此概念恒传 0）：手动切换落盘的
/// hold 标记冻结当日的自动 Switch/Restore 判定（否则 23:00 自动切换与手动切换
/// 叠加、恢复窗内的自动还原会撤销手动切换），次日自动恢复原有生命周期。
pub fn evaluate_outbound_action(
    enable_night_outbound_switch: bool,
    weekday_sunday: u32,
    now_minutes: u32,
    outbound_restore_active: bool,
    outbound_manual_hold_day: i32,
    today_day: i32,
) -> NightOutboundAction {
    evaluate_outbound_action_with_restore(
        enable_night_outbound_switch,
        weekday_sunday,
        now_minutes,
        outbound_restore_active,
        outbound_manual_hold_day,
        today_day,
        super::night_switch::DEFAULT_RESTORE_START_MINUTES,
    )
}

/// 带自定义恢复起点的出站动作判定（`restore_start` 由
/// [`super::night_switch::restore_start_minutes`] 归一化后传入）：语义与
/// [`evaluate_outbound_action`] 完全一致，仅恢复窗起点可变。
#[allow(clippy::too_many_arguments)] // 透传纯判定所需入参，拆结构体只为少一两个参数不值
pub fn evaluate_outbound_action_with_restore(
    enable_night_outbound_switch: bool,
    weekday_sunday: u32,
    now_minutes: u32,
    outbound_restore_active: bool,
    outbound_manual_hold_day: i32,
    today_day: i32,
    restore_start: u32,
) -> NightOutboundAction {
    // 清理分支优先，不受 hold 冻结：此时 evaluate 的 !enabled 短路恒产 None，
    // 直接改判 Restore 与原 match 守卫语义等价
    if outbound_restore_active && !enable_night_outbound_switch {
        return NightOutboundAction::Restore;
    }
    let evaluated = evaluate_night_outbound_with_restore(
        enable_night_outbound_switch,
        weekday_sunday,
        now_minutes,
        outbound_restore_active,
        restore_start,
    );
    // hold=0 表示无保持（num_days_from_ce 恒 >0，测试传 0 即无保持）
    if outbound_manual_hold_day != 0 && outbound_manual_hold_day == today_day {
        return NightOutboundAction::None;
    }
    evaluated
}

/// 运营商夜切判定（双端单点）：出站切换态下夜切让位——切侧从热点出站登录校园
/// portal 必然失败，切了白切；恢复侧标记非空即还原未成功（重试中），也不允许
/// 夜切恢复抢跑。非切换态透传 [`evaluate_night_switch`]。
pub fn evaluate_night_switch_gated(
    enable_night_operator_switch: bool,
    weekday_sunday: u32,
    now_minutes: u32,
    current_operator: &str,
    restore_operator: &str,
    outbound_active: bool,
) -> NightSwitchAction {
    evaluate_night_switch_gated_with_restore(
        enable_night_operator_switch,
        weekday_sunday,
        now_minutes,
        current_operator,
        restore_operator,
        outbound_active,
        super::night_switch::DEFAULT_RESTORE_START_MINUTES,
    )
}

/// 带自定义恢复起点的运营商夜切判定：还原窗起点与出站还原共用同一配置
/// （`nightRestoreMinutes`），保证「运营商切回校园」与「出站还原」同时发生。
#[allow(clippy::too_many_arguments)]
pub fn evaluate_night_switch_gated_with_restore(
    enable_night_operator_switch: bool,
    weekday_sunday: u32,
    now_minutes: u32,
    current_operator: &str,
    restore_operator: &str,
    outbound_active: bool,
    restore_start: u32,
) -> NightSwitchAction {
    if outbound_active {
        return NightSwitchAction::None;
    }
    evaluate_night_switch_with_restore(
        enable_night_operator_switch,
        weekday_sunday,
        now_minutes,
        current_operator,
        restore_operator,
        restore_start,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用「今天」：num_days_from_ce 量级的任意固定值
    const TODAY: i32 = 739_776;

    // ===== evaluate_scheduled_actions =====

    #[test]
    fn 定时_禁用目标_不触发() {
        // 双禁用（哨兵 1440）：登录/注销都不触发
        let (login, logout) = evaluate_scheduled_actions(600, TODAY, 1440, 1440, i32::MIN, i32::MIN, false);
        assert!(!login);
        assert!(!logout);
    }

    #[test]
    fn 定时_未到点_不触发() {
        let (login, logout) =
            evaluate_scheduled_actions(459, TODAY, 7 * 60 + 40, 23 * 60 + 30, i32::MIN, i32::MIN, false);
        assert!(!login);
        assert!(!logout);
    }

    #[test]
    fn 定时_到点及过点补触发() {
        // 精确到点与过点（循环拍错过精确分钟）都触发
        assert_eq!(evaluate_scheduled_actions(460, TODAY, 460, 1410, i32::MIN, i32::MIN, false), (true, false));
        assert_eq!(evaluate_scheduled_actions(505, TODAY, 460, 1410, i32::MIN, i32::MIN, false), (true, false));
        // 23:30 时登录目标 07:40 也已过点且未置标记：过点补触发语义下同样触发
        assert_eq!(evaluate_scheduled_actions(1410, TODAY, 460, 1410, i32::MIN, i32::MIN, false), (true, true));
        // 登录当日已触发（标记置位后）：只剩注销触发
        assert_eq!(evaluate_scheduled_actions(1410, TODAY, 460, 1410, TODAY, i32::MIN, false), (false, true));
    }

    #[test]
    fn 定时_同日已触发_不重复() {
        assert_eq!(evaluate_scheduled_actions(600, TODAY, 460, 1410, TODAY, TODAY, false), (false, false));
    }

    #[test]
    fn 定时_跨天重置_次日再触发() {
        // 旧标记（昨日）不再等于 today_day，视为未执行
        assert_eq!(evaluate_scheduled_actions(460, TODAY, 460, 1410, TODAY - 1, i32::MIN, false), (true, false));
    }

    #[test]
    fn 定时_切换态_登录跳过_注销照常() {
        // 切换态下登录到点也跳过（不消耗标记），注销照常
        assert_eq!(evaluate_scheduled_actions(460, TODAY, 460, 1410, i32::MIN, i32::MIN, true), (false, false));
        assert_eq!(evaluate_scheduled_actions(1410, TODAY, 460, 1410, i32::MIN, i32::MIN, true), (false, true));
    }

    #[test]
    fn 定时_独立判定_注销不受登录目标禁用影响() {
        // 登录禁用（1440）但注销启用：注销照常触发（两动作独立判定）
        assert_eq!(evaluate_scheduled_actions(1410, TODAY, 1440, 1410, i32::MIN, i32::MIN, false), (false, true));
    }

    // ===== evaluate_outbound_action =====

    #[test]
    fn 出站_功能关闭但切换态残留_立即还原() {
        // 未处于切换态：关闭状态下无动作
        assert_eq!(evaluate_outbound_action(false, 0, 1380, false, 0, TODAY), NightOutboundAction::None);
        // 快照残留（用户中途关掉开关）：立即还原，不留悬挂的跃点改动（全时段生效）
        assert_eq!(evaluate_outbound_action(false, 0, 1380, true, 0, TODAY), NightOutboundAction::Restore);
        assert_eq!(evaluate_outbound_action(false, 0, 500, true, 0, TODAY), NightOutboundAction::Restore);
    }

    #[test]
    fn 出站_开启时沿用窗口判定() {
        // 夜间窗口内切换态：保持（由补齐路径负责重写目标值），不重复切换
        assert_eq!(evaluate_outbound_action(true, 0, 1380, true, 0, TODAY), NightOutboundAction::None);
        // 恢复窗口内切换态：还原
        assert_eq!(evaluate_outbound_action(true, 0, 450, true, 0, TODAY), NightOutboundAction::Restore);
        // 未切换态且过点：切换（周日 switch_time_for=1380）
        assert_eq!(evaluate_outbound_action(true, 0, 1380, false, 0, TODAY), NightOutboundAction::Switch);
        // 未到点：无动作
        assert_eq!(evaluate_outbound_action(true, 0, 1379, false, 0, TODAY), NightOutboundAction::None);
    }

    #[test]
    fn 出站_hold_冻结当日自动动作() {
        // hold=today（用户点过「立即切换」）：冻结 Switch——否则自动切换与手动
        // 切换叠加，把已改动的跃点当原值重新快照
        assert_eq!(evaluate_outbound_action(true, 0, 1381, false, TODAY, TODAY), NightOutboundAction::None);
        // 凌晨补切同样被冻结
        assert_eq!(evaluate_outbound_action(true, 0, 200, false, TODAY, TODAY), NightOutboundAction::None);
        // 次日（hold≠today）：自动切换恢复
        assert_eq!(evaluate_outbound_action(true, 0, 1381, false, TODAY - 1, TODAY), NightOutboundAction::Switch);
    }

    #[test]
    fn 出站_hold_不冻结清理分支() {
        // 关开关时切换态残留仍立即还原（残留改动没有出口会永久压住巡检）
        assert_eq!(evaluate_outbound_action(false, 0, 1381, true, TODAY, TODAY), NightOutboundAction::Restore);
    }

    #[test]
    fn 出站_hold_冻结恢复窗还原() {
        // 恢复窗内（用户手动切换当晚不被恢复窗自动还原撤销）
        assert_eq!(evaluate_outbound_action(true, 0, 450, true, TODAY, TODAY), NightOutboundAction::None);
        // hold=0（无保持）：窗口语义不变
        assert_eq!(evaluate_outbound_action(true, 0, 450, true, 0, TODAY), NightOutboundAction::Restore);
    }

    // ===== evaluate_night_switch_gated =====

    #[test]
    fn 夜切_切换态让位() {
        // 切换态下即使到点+可切运营商也返回 None（切换侧登录必败、恢复侧重试中）
        assert_eq!(
            evaluate_night_switch_gated(true, 3, 1380, "@telecom", "", true),
            NightSwitchAction::None
        );
        assert_eq!(
            evaluate_night_switch_gated(true, 3, 500, "", "@telecom", true),
            NightSwitchAction::None
        );
    }

    #[test]
    fn 夜切_非切换态透传判定() {
        // 周三 23:00（1380）+ 可切运营商：切至校园
        assert_eq!(
            evaluate_night_switch_gated(true, 3, 1380, "@telecom", "", false),
            NightSwitchAction::SwitchToCampus
        );
        // 07:30（450）+ 恢复标记非空 + 当前空：还原原运营商
        assert_eq!(
            evaluate_night_switch_gated(true, 3, 450, "", "@telecom", false),
            NightSwitchAction::Restore
        );
        // 未到点：无动作
        assert_eq!(
            evaluate_night_switch_gated(true, 3, 1379, "@telecom", "", false),
            NightSwitchAction::None
        );
        // 功能关闭：无动作
        assert_eq!(
            evaluate_night_switch_gated(false, 3, 1380, "@telecom", "", false),
            NightSwitchAction::None
        );
    }
}
