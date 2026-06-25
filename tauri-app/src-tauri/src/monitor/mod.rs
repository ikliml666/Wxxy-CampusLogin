pub mod watcher;
pub mod auto_auth;
pub mod latency;
pub mod adapter_watch;
pub mod campus_check;
pub mod portal_check;
pub mod quality_scheduler;
pub mod background_emit;

/// 触发后台检测的统一入口
pub use watcher::start_background_check_inner as trigger_background_check;
