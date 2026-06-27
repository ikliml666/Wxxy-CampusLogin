pub mod watcher;
pub mod auto_auth;
pub mod latency;
pub mod adapter_watch;
pub mod campus_check;
pub mod portal_check;
pub mod portal_failure;
pub mod quality_scheduler;
pub mod background_emit;
pub mod background_check;
pub mod background_task;

/// 触发后台检测的统一入口
pub use background_task::start_background_check_inner as trigger_background_check;
