pub mod store;
pub mod network;
pub mod exit;

use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
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

impl Default for TaskLock {
    fn default() -> Self {
        Self::new()
    }
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

    /// 强制释放锁，仅供测试使用。
    #[cfg(test)]
    pub fn force_release(&self) {
        self.flag.store(false, Ordering::Release);
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
    pub is_downloading: TaskLock,
}

/// 更新与通知相关统计字段
///
/// 将原 AppState 顶层的 4 个原子标志合并为语义内聚的子结构体。
/// 所有字段保持原有原子语义（Acquire/Release/Relaxed ordering）。
pub struct UpdateStats {
    pub last_update_check_epoch_ms: AtomicU64,
    pub update_notified: AtomicBool,
    pub last_disabled_notification_ms: AtomicU64,
    pub last_network_change_notification_ms: AtomicU64,
    pub last_render_heartbeat_ms: AtomicU64,
    /// WebView 恢复动作（reload）滑动窗口起点 epoch ms（app/webview_recovery.rs），
    /// 0 表示尚未触发过恢复
    pub webview_recovery_window_start_ms: AtomicU64,
    /// 当前恢复窗口内已执行的 reload 次数
    pub webview_recovery_count: AtomicU32,
    /// 自动启用被禁适配器：上次尝试时间 epoch ms（adapter_watch），0 表示从未尝试
    pub auto_enable_last_attempt_ms: AtomicU64,
    /// 自动启用连续失败次数（退避输入；成功或手选适配器全部恢复时清零）
    pub auto_enable_failure_count: AtomicU32,
}

impl Default for UpdateStats {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdateStats {
    pub fn new() -> Self {
        Self {
            last_update_check_epoch_ms: AtomicU64::new(0),
            update_notified: AtomicBool::new(false),
            last_disabled_notification_ms: AtomicU64::new(0),
            last_network_change_notification_ms: AtomicU64::new(0),
            last_render_heartbeat_ms: AtomicU64::new(0),
            webview_recovery_window_start_ms: AtomicU64::new(0),
            webview_recovery_count: AtomicU32::new(0),
            auto_enable_last_attempt_ms: AtomicU64::new(0),
            auto_enable_failure_count: AtomicU32::new(0),
        }
    }
}

pub struct AppState {
    pub config: ConfigStore,
    pub tasks: TaskFlags,
    pub task_manager: BackgroundTaskManager,
    pub network: NetworkState,
    pub exit: ExitStateStore,
    pub update_stats: UpdateStats,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
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
                is_downloading: TaskLock::new(),
            },
            task_manager: BackgroundTaskManager::new(),
            network: NetworkState::new(),
            exit: ExitStateStore::new(),
            update_stats: UpdateStats::new(),
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
    pub fn err(msg: &str) -> Self {
        Self { success: false, message: Some(msg.to_string()), data: None }
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
