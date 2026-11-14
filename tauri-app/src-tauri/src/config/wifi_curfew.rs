//! WiFi 就寝守则：周日至周四 23:00、周五六 23:30 起阻断指定 WiFi 的连接，
//! 次日 `nightRestoreMinutes`（与夜切恢复时刻同源）解除。纯函数，双端共用。
//!
//! 时段与夜切共用一份时间表（`switch_time_for`），只多了「哪些 SSID 要断」
//! 这条与运营商/出站完全无关的维度——所以判定单独成模块，动作由各端实现：
//! 桌面改 WLAN profile 连接模式并断开，安卓按系统能力取最优路径。

use super::night_switch::{restore_start_minutes, switch_time_for};

/// 是否处于就寝守则生效窗口：当日切换时刻（含过点补触发）起，至次日恢复窗开为止。
///
/// 凌晨分支（`now < 恢复起点`）覆盖前一晚切换时刻之后的全部时段，跨日天然成立；
/// `restore_start` 可传原始配置值，函数内部归一化（与夜切同一口径）。
pub fn in_curfew_window(weekday: u32, now_minutes: u32, restore_start: u32) -> bool {
    let restore_start = restore_start_minutes(restore_start);
    if now_minutes < restore_start {
        return true;
    }
    switch_time_for(weekday).is_some_and(|t| now_minutes >= t)
}

/// 按本机当前时间判定是否处于守则窗口。双端调用方共用——安卓侧没有独立的
/// 时间换算，拼一遍 chrono 等于把「周日至周四 23:00」这条口径复制一份。
pub fn in_curfew_window_now(restore_start: u32) -> bool {
    use chrono::{Datelike, Timelike};
    let now = chrono::Local::now();
    let weekday = now.weekday().num_days_from_sunday();
    let now_minutes = now.hour() * 60 + now.minute();
    in_curfew_window(weekday, now_minutes, restore_start)
}

/// 当前 SSID 是否被就寝守则阻断。
///
/// 规则：命中排除列表且不在允许列表 → 阻断。排除列表含 `"*"` 时表示
/// 「除允许列表外全部阻断」（宿舍自带路由器的白名单场景）。
/// 空 SSID（未连 WiFi/读不到）一律不阻断——没有可断的对象。
pub fn ssid_blocked(excluded: &[String], allowed: &[String], ssid: &str) -> bool {
    if ssid.is_empty() {
        return false;
    }
    if allowed.iter().any(|a| a.eq_ignore_ascii_case(ssid)) {
        return false;
    }
    excluded
        .iter()
        .any(|e| e == "*" || e.eq_ignore_ascii_case(ssid))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn window_covers_switch_time_to_restore_start() {
        // 周日 23:00 起生效，22:59 不生效
        assert!(in_curfew_window(0, 1380, 0));
        assert!(!in_curfew_window(0, 1379, 0));
        // 周五/周六 23:30 起，23:29 不生效
        assert!(in_curfew_window(5, 1410, 0));
        assert!(!in_curfew_window(5, 1409, 0));
        // 凌晨跨日：前一晚之后仍生效，恢复起点一到即出窗
        assert!(in_curfew_window(0, 0, 0));
        assert!(in_curfew_window(6, 449, 0));
        assert!(!in_curfew_window(0, 450, 0));
        // 白天不生效
        assert!(!in_curfew_window(3, 720, 0));
    }

    #[test]
    fn window_honours_custom_restore_start() {
        // 恢复时刻配成 06:00（360）：05:59 仍在窗内，06:00 出窗
        assert!(in_curfew_window(1, 359, 360));
        assert!(!in_curfew_window(1, 360, 360));
    }

    #[test]
    fn excluded_ssid_is_blocked() {
        let ex = v(&["i-wxxy", "iwxxy-2"]);
        assert!(ssid_blocked(&ex, &[], "i-wxxy"));
        assert!(ssid_blocked(&ex, &[], "IWXXY-2"));
        assert!(!ssid_blocked(&ex, &[], "kiku-5G"));
        assert!(!ssid_blocked(&ex, &[], ""));
    }

    #[test]
    fn allowed_list_overrides_excluded() {
        let ex = v(&["i-wxxy"]);
        let al = v(&["i-wxxy"]);
        assert!(!ssid_blocked(&ex, &al, "i-wxxy"));
    }

    #[test]
    fn wildcard_blocks_everything_except_allowed() {
        let ex = v(&["*"]);
        let al = v(&["dorm-router"]);
        assert!(ssid_blocked(&ex, &al, "kiku-5G"));
        assert!(!ssid_blocked(&ex, &al, "dorm-router"));
        assert!(!ssid_blocked(&ex, &al, ""));
    }
}
