pub mod state;
pub mod logger;
pub mod notification;
pub mod lifecycle;
pub mod events;
pub mod command_context;
pub mod task_manager;
pub mod async_util;

use chrono::{Datelike, Timelike};

/// 当前本地时间的当日分钟数（0..=1439）：静默窗/时间窗判断共用，避免各处重复表达式
pub fn local_now_minutes() -> u16 {
    let now = chrono::Local::now();
    now.hour() as u16 * 60 + now.minute() as u16
}

/// 当前本地时间的星期（0=周日 … 6=周六，与 chrono 的 num_days_from_sunday、
/// 后端夜切判定同口径）与当日分钟数。
///
/// 两者必须取自同一个 Local::now()：按星期分组的检测时段要同时用它们判定，
/// 分开取两次时钟可能在跨分钟/跨零点的一瞬间对不上（周日的 23:59:59 与周一的
/// 00:00:00），得出「用今天的星期套昨天的时刻」的错位结论。
pub fn local_now_weekday_minutes() -> (u32, u16) {
    let now = chrono::Local::now();
    (
        now.weekday().num_days_from_sunday(),
        now.hour() as u16 * 60 + now.minute() as u16,
    )
}
