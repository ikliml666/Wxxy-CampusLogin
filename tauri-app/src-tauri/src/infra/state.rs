use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use arc_swap::ArcSwap;
use crate::config::model::Config;
use parking_lot::Mutex;

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

    pub fn acquire_guard(&self) -> Option<TaskGuard<'_>> {
        if self.flag.swap(true, Ordering::Acquire) {
            None
        } else {
            Some(TaskGuard { lock: self })
        }
    }

    pub fn is_active(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }

    pub fn force_release(&self) {
        self.flag.store(false, Ordering::Release);
    }

    pub fn swap_acquire(&self) -> bool {
        self.flag.swap(true, Ordering::Acquire)
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
    if name.is_empty() || name.len() > 32 {
        return Err("账号名称长度需在1-32之间".to_string());
    }
    if !ACCOUNT_NAME_RE.is_match(name) {
        return Err("账号名称仅允许字母、数字、下划线、中文和连字符".to_string());
    }
    Ok(name.to_string())
}

pub struct TaskFlags {
    pub background_running: TaskLock,
    pub bg_check_cancel: ArcSwap<tokio_util::sync::CancellationToken>,
    pub latency_running: TaskLock,
    pub latency_cancel: ArcSwap<tokio_util::sync::CancellationToken>,
    pub adapter_watch_running: TaskLock,
    pub adapter_watch_cancel: ArcSwap<tokio_util::sync::CancellationToken>,
    pub is_checking: TaskLock,
    pub is_logging_in: TaskLock,
    pub is_logging_out: TaskLock,
    pub is_quality_checking: TaskLock,
}

pub struct NetworkStatus {
    pub server_available: AtomicBool,
    pub any_adapter_online: AtomicBool,
    pub last_a1_online: AtomicBool,
    pub last_a2_online: AtomicBool,
    pub has_logged_online: AtomicBool,
    pub disconnect_reconnect_count: AtomicU32,
    pub background_check_count: AtomicU32,
    pub last_auto_login_attempt: ArcSwap<std::time::Instant>,
    pub last_network_quality: ArcSwap<Option<String>>,
    pub current_ssid: ArcSwap<Option<String>>,
    pub on_campus_network: AtomicBool,
    pub logout_protected_until: ArcSwap<std::time::Instant>,
    pub last_quality_check_time: ArcSwap<std::time::Instant>,
    pub portal_failure_count: AtomicU32,
}

pub struct ExitState {
    pub is_quitting: std::sync::Arc<AtomicBool>,
    pub auto_exit_deadline: Mutex<Option<std::time::Instant>>,
    pub auto_exit_cancelled: AtomicBool,
    pub campus_exit_started: AtomicBool,
}

impl ExitState {
    pub fn deadline(&self) -> Option<std::time::Instant> {
        *self.auto_exit_deadline.lock()
    }

    pub fn set_deadline(&self, deadline: Option<std::time::Instant>) {
        *self.auto_exit_deadline.lock() = deadline;
    }
}

pub struct AppState {
    pub config: ArcSwap<Config>,
    pub tasks: TaskFlags,
    pub network: NetworkStatus,
    pub exit: ExitState,
    pub last_update_check_epoch_ms: AtomicU64,
    pub update_notified: AtomicBool,
    pub last_disabled_notification_ms: AtomicU64,
    pub last_render_heartbeat_ms: AtomicU64,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            config: ArcSwap::from(std::sync::Arc::new(Config::default())),
            tasks: TaskFlags {
                background_running: TaskLock::new(),
                bg_check_cancel: ArcSwap::from(Arc::new(tokio_util::sync::CancellationToken::new())),
                latency_running: TaskLock::new(),
                latency_cancel: ArcSwap::from(Arc::new(tokio_util::sync::CancellationToken::new())),
                adapter_watch_running: TaskLock::new(),
                adapter_watch_cancel: ArcSwap::from(Arc::new(tokio_util::sync::CancellationToken::new())),
                is_checking: TaskLock::new(),
                is_logging_in: TaskLock::new(),
                is_logging_out: TaskLock::new(),
                is_quality_checking: TaskLock::new(),
            },
            network: NetworkStatus {
                server_available: AtomicBool::new(false),
                any_adapter_online: AtomicBool::new(false),
                last_a1_online: AtomicBool::new(false),
                last_a2_online: AtomicBool::new(false),
                has_logged_online: AtomicBool::new(false),
                disconnect_reconnect_count: AtomicU32::new(0),
                background_check_count: AtomicU32::new(0),
                last_auto_login_attempt: ArcSwap::from(std::sync::Arc::new(std::time::Instant::now())),
                last_network_quality: ArcSwap::from(std::sync::Arc::new(None)),
                current_ssid: ArcSwap::from(std::sync::Arc::new(None)),
                on_campus_network: AtomicBool::new(false),
                logout_protected_until: ArcSwap::from(std::sync::Arc::new(std::time::Instant::now())),
                last_quality_check_time: ArcSwap::from(std::sync::Arc::new(
                    std::time::Instant::now().checked_sub(std::time::Duration::from_secs(86400))
                        .unwrap_or_else(std::time::Instant::now)
                )),
                portal_failure_count: AtomicU32::new(0),
            },
            exit: ExitState {
                is_quitting: std::sync::Arc::new(AtomicBool::new(false)),
                auto_exit_deadline: Mutex::new(None),
                auto_exit_cancelled: AtomicBool::new(false),
                campus_exit_started: AtomicBool::new(false),
            },
            last_update_check_epoch_ms: AtomicU64::new(0),
            update_notified: AtomicBool::new(false),
            last_disabled_notification_ms: AtomicU64::new(0),
            last_render_heartbeat_ms: AtomicU64::new(0),
        }
    }

    /// 使用 CAS 实现原子更新配置，避免 TOCTOU 竞态条件
    pub fn update_config<F>(&self, f: F) -> Arc<Config>
    where
        F: Fn(&mut Config),
    {
        loop {
            let current = self.config.load_full();
            let mut new_cfg = (*current).clone();
            f(&mut new_cfg);
            let new_arc = Arc::new(new_cfg);
            let prev = self.config.compare_and_swap(&current, new_arc);
            if Arc::ptr_eq(&current, &prev) {
                return self.config.load_full();
            }
        }
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
