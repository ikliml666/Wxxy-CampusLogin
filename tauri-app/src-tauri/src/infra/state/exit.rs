use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;
use parking_lot::Mutex;

/// 退出状态存储，封装与应用退出相关的所有状态和截止时间。
pub struct ExitStateStore {
    pub is_quitting: Arc<AtomicBool>,
    pub auto_exit_deadline: Mutex<Option<Instant>>,
    pub auto_exit_cancelled: AtomicBool,
    pub campus_exit_started: AtomicBool,
    pub campus_exit_deadline: Mutex<Option<Instant>>,
}

impl ExitStateStore {
    pub fn new() -> Self {
        Self {
            is_quitting: Arc::new(AtomicBool::new(false)),
            auto_exit_deadline: Mutex::new(None),
            auto_exit_cancelled: AtomicBool::new(false),
            campus_exit_started: AtomicBool::new(false),
            campus_exit_deadline: Mutex::new(None),
        }
    }

    pub fn deadline(&self) -> Option<Instant> {
        *self.auto_exit_deadline.lock()
    }

    pub fn set_deadline(&self, deadline: Option<Instant>) {
        *self.auto_exit_deadline.lock() = deadline;
    }

    pub fn campus_exit_deadline(&self) -> Option<Instant> {
        *self.campus_exit_deadline.lock()
    }

    pub fn set_campus_exit_deadline(&self, deadline: Option<Instant>) {
        *self.campus_exit_deadline.lock() = deadline;
    }
}
