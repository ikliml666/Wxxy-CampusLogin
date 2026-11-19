//! 校园网检测时段（按星期分组）的判定与剩余时间计算（纯函数，双端单点）。
//!
//! 历史：检测时段原本只有一组 `campusCheckStartMinutes` / `campusCheckEndMinutes`，
//! 但校园网的实际断网时刻按星期分档（周日~周四 23:00、周五六 23:30，见
//! `config::night_switch::switch_time_for`），一套起止无法同时贴合两档，用户只能
//! 把结束时刻取折中值，导致其中一端整段空转或整段漏检。本模块让时段可以按星期
//! 分组：每组带自己的生效星期与起止，多组并存。
//!
//! 兼容口径：`groups` 为空 = 沿用旧的单一 `start`/`end`（存量配置零迁移）；
//! 非空时分组即完整定义——**没有被任何组覆盖的星期视为整天不检测**，这与
//! 「检测时段限定在窗口内」的既有语义一致（窗口外的时刻就是静默）。

use serde::{Deserialize, Serialize};

use super::schedule::is_campus_check_silent;

/// 一天的分钟数，跨天推进用。
const MINUTES_PER_DAY: u32 = 1440;
/// 一周七天，逐日推进的上限（跨 7 天必回原点，多算一天兜住「今天已过点」）。
const MAX_LOOKAHEAD_DAYS: u32 = 8;

/// 一组按星期生效的检测时段。
///
/// `weekdays` 用 `0=周日 … 6=周六`（与 `chrono::Weekday::num_days_from_sunday`
/// 及后端夜切判定同口径）；起止为当日分钟数，退化规则与
/// [`is_campus_check_silent`] 完全一致（`start == 0` 禁用门控、
/// `end == 0` 或 `end <= start` 只受开始时间限制）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckWindowGroup {
    /// 生效星期（去重后的 0..=6；空组由校验层丢弃，进入本模块的组至少一天）
    pub weekdays: Vec<u8>,
    pub start: u16,
    pub end: u16,
}

impl CheckWindowGroup {
    fn covers(&self, weekday: u32) -> bool {
        self.weekdays.iter().any(|d| u32::from(*d) == weekday)
    }
}

/// 命中的第一个组（顺序即优先级：同一天被多组覆盖时取表中靠前的一组）。
pub fn group_for(weekday: u32, groups: &[CheckWindowGroup]) -> Option<&CheckWindowGroup> {
    groups.iter().find(|g| g.covers(weekday))
}

/// 按星期分组的静默期判定。
///
/// `groups` 为空 → 完全等价旧语义 [`is_campus_check_silent`]；
/// 非空 → 取当天命中的组按该组起止判定，当天没有命中组则整天静默。
pub fn is_campus_check_silent_on(
    weekday: u32,
    now_minutes: u16,
    start: u16,
    end: u16,
    groups: &[CheckWindowGroup],
) -> bool {
    if groups.is_empty() {
        return is_campus_check_silent(now_minutes, start, end);
    }
    match group_for(weekday, groups) {
        Some(g) => is_campus_check_silent(now_minutes, g.start, g.end),
        None => true,
    }
}

/// 静默期剩余毫秒（仅处于静默期时返回 Some），用于把下一拍巡检提前到开检时刻。
///
/// 语义与 [`crate::monitor::background_check::silent_remaining_ms`] 的旧实现互补：
/// 那里按「今天还没到 start」或「今天已过 end」两分支估算，本函数逐日推进，
/// 因而能跨过「周五六 23:30 收工 → 周六 07:50 开检」这类按星期变化的窗口。
pub fn silent_remaining_ms_on(
    weekday: u32,
    now_minutes: u16,
    start: u16,
    end: u16,
    groups: &[CheckWindowGroup],
) -> Option<u64> {
    if !is_campus_check_silent_on(weekday, now_minutes, start, end, groups) {
        return None;
    }
    if groups.is_empty() {
        let remaining_min = if now_minutes < start {
            u32::from(start - now_minutes)
        } else {
            // end>start 且 now>=end：次日 start 才算离开静默
            MINUTES_PER_DAY - u32::from(now_minutes) + u32::from(start)
        };
        return Some(u64::from(remaining_min) * 60_000);
    }
    let now = u32::from(now_minutes);
    for day in 0..=MAX_LOOKAHEAD_DAYS {
        let base = day * MINUTES_PER_DAY;
        let wd = (weekday % 7 + day) % 7;
        let Some(g) = group_for(wd, groups) else { continue };
        // 该日开检时刻的绝对分钟：s==0 表示该日不禁用门控，0 点即开检
        let open_at = base + u32::from(g.start);
        if open_at > now {
            return Some(u64::from(open_at - now) * 60_000);
        }
    }
    // 一周内没有任何一天开检（分组未覆盖任何星期）：不存在「下一次开检」，
    // 交给调用方按普通节奏走
    None
}

/// 当天生效的检测时段文案（日志与通知用）：返回 "HH:MM–HH:MM"，
/// 或 "全天不检测"（分组未覆盖当天）。分组为空时回落到单一组起止。
pub fn window_label(weekday: u32, start: u16, end: u16, groups: &[CheckWindowGroup]) -> String {
    let fmt = |m: u16| format!("{:02}:{:02}", m / 60, m % 60);
    if groups.is_empty() {
        return format!("{}–{}", fmt(start), fmt(end));
    }
    match group_for(weekday, groups) {
        Some(g) => format!("{}–{}", fmt(g.start), fmt(g.end)),
        None => "全天不检测".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 周日~周四 07:50–23:00、周五六 07:50–23:30（对齐夜切时刻的典型配置）
    fn aligned() -> Vec<CheckWindowGroup> {
        vec![
            CheckWindowGroup { weekdays: vec![0, 1, 2, 3, 4], start: 470, end: 1380 },
            CheckWindowGroup { weekdays: vec![5, 6], start: 470, end: 1410 },
        ]
    }

    #[test]
    fn 空分组_完全沿用单一时段() {
        let (start, end) = (460u16, 1380u16);
        for now in [0u16, 400, 460, 600, 1379, 1380, 1439] {
            assert_eq!(
                is_campus_check_silent_on(3, now, start, end, &[]),
                is_campus_check_silent(now, start, end),
                "now={now} 时空分组必须与旧语义一致"
            );
        }
    }

    #[test]
    fn 分组_工作日与周末各自的终点生效() {
        let g = aligned();
        // 周四 23:00 收工
        assert!(!is_campus_check_silent_on(4, 1379, 0, 0, &g));
        assert!(is_campus_check_silent_on(4, 1380, 0, 0, &g));
        // 周六 23:00 仍在检测时段内，23:30 才收工
        assert!(!is_campus_check_silent_on(6, 1380, 0, 0, &g));
        assert!(!is_campus_check_silent_on(6, 1409, 0, 0, &g));
        assert!(is_campus_check_silent_on(6, 1410, 0, 0, &g));
        // 起点对所有天一致
        assert!(is_campus_check_silent_on(1, 469, 0, 0, &g));
        assert!(!is_campus_check_silent_on(1, 470, 0, 0, &g));
    }

    #[test]
    fn 未被任何组覆盖的星期整天静默() {
        let groups = vec![CheckWindowGroup { weekdays: vec![5, 6], start: 470, end: 1410 }];
        // 周一：没有任何组覆盖 → 整天静默，含白天
        assert!(is_campus_check_silent_on(1, 600, 0, 0, &groups));
        assert!(is_campus_check_silent_on(1, 0, 0, 0, &groups));
        // 周六按组内窗口判定
        assert!(!is_campus_check_silent_on(6, 600, 0, 0, &groups));
    }

    #[test]
    fn 同一天多组命中_取表中靠前的一组() {
        let groups = vec![
            CheckWindowGroup { weekdays: vec![1], start: 600, end: 1380 },
            CheckWindowGroup { weekdays: vec![1], start: 0, end: 0 },
        ];
        assert!(is_campus_check_silent_on(1, 500, 0, 0, &groups), "取第一组：500 < 600 静默");
        assert!(!is_campus_check_silent_on(1, 600, 0, 0, &groups));
    }

    #[test]
    fn 剩余时间_当天未到起点() {
        let g = aligned();
        // 周一 03:00 → 距 07:50 还有 290 分钟
        assert_eq!(silent_remaining_ms_on(1, 180, 0, 0, &g), Some(290 * 60_000));
    }

    #[test]
    fn 剩余时间_跨星期到下一次开检() {
        let g = aligned();
        // 周四 23:30（静默）→ 周五 07:50：剩余 8 小时 20 分 = 500 分钟
        assert_eq!(silent_remaining_ms_on(4, 1410, 0, 0, &g), Some(500 * 60_000));
        // 周六 23:40（静默）→ 周日 07:50：剩余 490 分钟
        assert_eq!(silent_remaining_ms_on(6, 1420, 0, 0, &g), Some(490 * 60_000));
    }

    #[test]
    fn 剩余时间_跳过错过的整天() {
        // 只有周六有窗口：周日 12:00 静默 → 下一个开检是下周六 07:50
        let groups = vec![CheckWindowGroup { weekdays: vec![6], start: 470, end: 1410 }];
        // 周日 12:00 = 720 分；到周六 0:00 是 6 天 = 8640 分；再 +470
        assert_eq!(
            silent_remaining_ms_on(0, 720, 0, 0, &groups),
            Some((6 * 1440 + 470 - 720) as u64 * 60_000)
        );
    }

    #[test]
    fn 剩余时间_检测时段内为none() {
        let g = aligned();
        assert_eq!(silent_remaining_ms_on(2, 600, 0, 0, &g), None);
    }

    #[test]
    fn 剩余时间_组只覆盖部分星期仍能算出下一次开检() {
        let groups = vec![CheckWindowGroup { weekdays: vec![6], start: 470, end: 1410 }];
        // 周一 03:00：组只覆盖周六，但它确实存在 → 有下一次开检
        assert_eq!(
            silent_remaining_ms_on(1, 180, 0, 0, &groups),
            Some((5 * 1440 + 470 - 180) as u64 * 60_000)
        );
    }

    #[test]
    fn 组内起点0_沿用禁门控语义即全天检测() {
        // start=0 在 is_campus_check_silent 里是「禁用门控」，这里沿用同一退化规则：
        // 该组等价「全天检测」。要表达「整天不检测」用不勾选该星期。
        let groups = vec![CheckWindowGroup { weekdays: vec![1], start: 0, end: 0 }];
        assert!(!is_campus_check_silent_on(1, 0, 0, 0, &groups));
        assert!(!is_campus_check_silent_on(1, 1439, 0, 0, &groups));
    }
}
