//! 定时动作（定时登录 / 定时注销）的命中判定（P2-32）。
//!
//! 本模块在跨平台可见面（lib.rs 无 cfg 门控），桌面 monitor::scheduled 与
//! 安卓 monitor_loop 共用同一实现与同一组测试，保证两端判定语义单点一致。

/// 定时动作当日触发判定（纯函数，双端同语义）：
/// - `target_minutes = 0`：禁用，永不触发（与 campus_check_start_minutes 的 0=禁用约定一致）；
/// - `now_minutes < target_minutes`：未到点，不触发；
/// - 已到点（`now_minutes >= target_minutes`，含过点补触发）且 `last_fired_day != today_day`
///   时触发；`last_fired_day == today_day` 表示当日已执行过，不重复触发。
///
/// 跨天重置由「`last_fired_day` 记录的是日期序号」天然实现：新的一天
/// `today_day` 变化后，旧标记不再相等，视为未执行。
///
/// 「已到点即触发（而非严格等于配置分钟）」是有意为之：循环拍间隔/系统休眠
/// 可能恰好错过配置分钟，等值比较会让定时动作整天静默失效。
pub fn should_fire_scheduled_action(now_minutes: u16, target_minutes: u16, last_fired_day: i32, today_day: i32) -> bool {
    if target_minutes == 0 {
        return false;
    }
    if now_minutes < target_minutes {
        return false;
    }
    last_fired_day != today_day
}

#[cfg(test)]
mod tests {
    use super::should_fire_scheduled_action;

    const TODAY: i32 = 739_776;

    #[test]
    fn 禁用值0_永不触发() {
        assert!(!should_fire_scheduled_action(0, 0, i32::MIN, TODAY));
        assert!(!should_fire_scheduled_action(600, 0, i32::MIN, TODAY));
    }

    #[test]
    fn 未到点_不触发() {
        assert!(!should_fire_scheduled_action(459, 460, i32::MIN, TODAY));
        assert!(!should_fire_scheduled_action(0, 460, i32::MIN, TODAY));
    }

    #[test]
    fn 到点_当日未执行过_触发() {
        // 恰好等于配置分钟
        assert!(should_fire_scheduled_action(460, 460, i32::MIN, TODAY));
        // 过点补触发（循环拍错过配置分钟的场景）
        assert!(should_fire_scheduled_action(505, 460, i32::MIN, TODAY));
    }

    #[test]
    fn 同日已执行_不重复触发() {
        assert!(!should_fire_scheduled_action(461, 460, TODAY, TODAY));
        assert!(!should_fire_scheduled_action(1439, 460, TODAY, TODAY));
    }

    #[test]
    fn 跨天重置_次日再触发() {
        // 昨日已执行，今天重新视为未执行
        assert!(should_fire_scheduled_action(460, 460, TODAY - 1, TODAY));
    }
}
