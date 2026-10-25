pub mod state;
pub mod logger;
pub mod notification;
pub mod lifecycle;
pub mod events;
pub mod command_context;
pub mod task_manager;
pub mod async_util;

use chrono::Timelike;

/// 当前本地时间的当日分钟数（0..=1439）：静默窗/时间窗判断共用，避免各处重复表达式
pub fn local_now_minutes() -> u16 {
    let now = chrono::Local::now();
    now.hour() as u16 * 60 + now.minute() as u16
}
