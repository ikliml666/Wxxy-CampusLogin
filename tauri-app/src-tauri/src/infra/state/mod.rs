pub mod store;
pub mod network;
pub mod exit;

use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use crate::config::model::Config;
use store::ConfigStore;
use network::NetworkState;
use exit::ExitStateStore;
use crate::infra::task_manager::BackgroundTaskManager;

pub const AUTO_EXIT_DELAY_MS: u64 = 20000;
pub const CANCEL_EXIT_SHORTCUT: &str = "CommandOrControl+Shift+C";

pub struct TaskLock {
    flag: AtomicBool,
}

pub struct TaskGuard<'a> {
    lock: &'a TaskLock,
}

impl TaskLock {
    pub fn new() -> Self {
        Self { flag: AtomicBool::new(false) }
    }

    pub fn try_acquire(&self) -> Option<TaskGuard<'_>> {
        if self.flag.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
            Some(TaskGuard { lock: self })
        } else {
            None
        }
    }

    pub fn is_active(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }
}

impl Drop for TaskGuard<'_> {
    fn drop(&mut self) {
        self.lock.flag.store(false, Ordering::Release);
    }
}

lazy_static::lazy_static! {
    static ref ACCOUNT_NAME_RE: regex::Regex = regex::Regex::new(r"^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$").expect("ACCOUNT_NAME_RE compilation failed");
}

pub fn validate_account_name(name: &str) -> Result<String, String> {
    if name.is_empty() || name.chars().count() > 32 {
        return Err("账号名称长度需在1-32之间".to_string());
    }
    if !ACCOUNT_NAME_RE.is_match(name) {
        return Err("账号名称仅允许字母、数字、下划线、中文和连字符".to_string());
    }
    Ok(name.to_string())
}

pub struct TaskFlags {
    pub is_checking: TaskLock,
    pub is_logging_in: TaskLock,
    pub is_logging_out: TaskLock,
    pub is_quality_checking: TaskLock,
}

pub struct AppState {
    pub config: ConfigStore,
    pub tasks: TaskFlags,
    pub task_manager: BackgroundTaskManager,
    pub network: NetworkState,
    pub exit: ExitStateStore,
    pub last_update_check_epoch_ms: AtomicU64,
    pub update_notified: AtomicBool,
    pub last_disabled_notification_ms: AtomicU64,
    pub last_render_heartbeat_ms: AtomicU64,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            config: ConfigStore::new(Config::default()),
            tasks: TaskFlags {
                is_checking: TaskLock::new(),
                is_logging_in: TaskLock::new(),
                is_logging_out: TaskLock::new(),
                is_quality_checking: TaskLock::new(),
            },
            task_manager: BackgroundTaskManager::new(),
            network: NetworkState::new(),
            exit: ExitStateStore::new(),
            last_update_check_epoch_ms: AtomicU64::new(0),
            update_notified: AtomicBool::new(false),
            last_disabled_notification_ms: AtomicU64::new(0),
            last_render_heartbeat_ms: AtomicU64::new(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_lock_acquire_and_release() {
        let lock = TaskLock::new();
        assert!(!lock.is_active());
        let guard = lock.try_acquire().unwrap();
        assert!(lock.is_active());
        drop(guard);
        assert!(!lock.is_active());
    }

    #[test]
    fn task_lock_rejects_second_acquire() {
        let lock = TaskLock::new();
        let _guard = lock.try_acquire().unwrap();
        assert!(lock.try_acquire().is_none());
    }

    #[test]
    fn task_lock_force_release_allows_reacquire() {
        let lock = TaskLock::new();
        let _guard = lock.try_acquire().unwrap();
        lock.force_release();
        assert!(!lock.is_active());
        assert!(lock.try_acquire().is_some());
    }

    #[test]
    fn task_lock_guard_releases_on_drop() {
        let lock = TaskLock::new();
        {
            let _guard = lock.try_acquire();
            assert!(lock.is_active());
        }
        assert!(!lock.is_active());
    }

    #[test]
    fn validate_account_name_accepts_valid() {
        assert!(validate_account_name("user_123").is_ok());
        assert!(validate_account_name("用户名").is_ok());
    }

    #[test]
    fn validate_account_name_rejects_invalid() {
        assert!(validate_account_name("").is_err());
        assert!(validate_account_name("user@name").is_err());
    }
}

#[derive(Serialize)]
pub struct CommandResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl CommandResult {
    pub fn ok() -> Self {
        Self { success: true, message: None, data: None }
    }
    pub fn ok_msg(msg: &str) -> Self {
        Self { success: true, message: Some(msg.to_string()), data: None }
    }
    pub fn ok_data(data: serde_json::Value) -> Self {
        Self { success: true, message: None, data: Some(data) }
    }
    pub fn err(msg: &str) -> Self {
        Self { success: false, message: Some(msg.to_string()), data: None }
    }
    #[allow(dead_code)]
    pub fn from_json_result(value: serde_json::Value) -> Self {
        let success = value.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
        let message = value.get("message").and_then(|v| v.as_str()).map(|s| s.to_string());
        Self { success, message, data: Some(value) }
    }
}

#[cfg(test)]
mod command_result_tests {
    use super::CommandResult;

    #[test]
    fn from_json_result_extracts_success_and_message() {
        let value = serde_json::json!({
            "success": true,
            "message": "login ok",
            "code": "0"
        });
        let result = CommandResult::from_json_result(value);
        assert!(result.success);
        assert_eq!(result.message, Some("login ok".to_string()));
        assert!(result.data.is_some());
    }

    #[test]
    fn from_json_result_defaults_to_failure_without_success_field() {
        let value = serde_json::json!({
            "message": "no success field"
        });
        let result = CommandResult::from_json_result(value);
        assert!(!result.success);
        assert_eq!(result.message, Some("no success field".to_string()));
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_account: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<Config>,
}

impl AccountResult {
    pub fn ok(config: Config) -> Self {
        Self { success: true, message: None, active_account: None, config: Some(config) }
    }
    pub fn ok_with_account(account: String, config: Config) -> Self {
        Self { success: true, message: None, active_account: Some(account), config: Some(config) }
    }
    pub fn err(msg: &str) -> Self {
        Self { success: false, message: Some(msg.to_string()), active_account: None, config: None }
    }
}
