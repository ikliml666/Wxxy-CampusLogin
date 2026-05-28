use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use arc_swap::ArcSwap;
use crate::config::Config;
use parking_lot::Mutex;

pub const AUTO_EXIT_DELAY_MS: u64 = 10000;
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
    static ref CUSTOM_COLOR_RE: regex::Regex = regex::Regex::new(r"^#[0-9a-fA-F]{6}$").expect("CUSTOM_COLOR_RE compilation failed");
}

pub fn validate_config(config: Config) -> Result<Config, String> {
    let mut config = config;
    if !config.user.is_empty() {
        crate::config::validate_username(&config.user)?;
    }
    if !config.password.is_empty() {
        if config.password == crate::config::PASSWORD_MASK {
            return Err("密码不能为\"***\"".to_string());
        }
        crate::config::validate_password(&config.password)?;
    }
    if config.operator == "@ctcc" {
        config.operator = "@telecom".to_string();
    } else if config.operator == "@cucc" {
        config.operator = "@unicom".to_string();
    }
    config.operator = crate::config::validate_operator(&config.operator)?.to_string();
    if !config.custom_theme_color.is_empty() {
        if !CUSTOM_COLOR_RE.is_match(&config.custom_theme_color) {
            return Err("自定义主题颜色格式无效，需为#开头的6位十六进制色值".to_string());
        }
    }
    if config.theme_mode != "dark" && config.theme_mode != "light" && config.theme_mode != "system" {
        return Err("主题模式必须为\"dark\"、\"light\"或\"system\"".to_string());
    }
    config.background_check_interval = config.background_check_interval.clamp(10000, 3600000);
    config.latency_test_interval = config.latency_test_interval.clamp(10000, 3600000);
    if config.portal_url == "http://10.1.99.100:801" {
        config.portal_url = "http://10.1.99.100".to_string();
    }
    if config.portal_url.is_empty() {
        config.portal_url = "http://10.1.99.100".to_string();
    }
    match url::Url::parse(&config.portal_url) {
        Ok(parsed) => {
            let scheme = parsed.scheme();
            if scheme != "http" && scheme != "https" {
                return Err(format!("Portal地址协议不支持: {}，仅允许http/https", scheme));
            }
            if let Some(host) = parsed.host_str() {
                if let Ok(ip) = host.parse::<std::net::IpAddr>() {
                    match ip {
                        std::net::IpAddr::V4(v4) => {
                            if !v4.is_private() && !v4.is_loopback() {
                                return Err("Portal地址仅允许内网IP或localhost".to_string());
                            }
                        }
                        std::net::IpAddr::V6(v6) => {
                            if !v6.is_loopback() {
                                return Err("Portal地址仅允许内网IPv4或localhost".to_string());
                            }
                        }
                    }
                } else if host != "localhost" {
                    return Err("Portal地址仅允许IP地址，不支持域名".to_string());
                }
            }
        }
        Err(e) => {
            return Err(format!("Portal地址格式无效: {}", e));
        }
    }
    if !config.fixed_gateway.is_empty() {
        if config.fixed_gateway.parse::<std::net::IpAddr>().is_err() {
            return Err(format!("固定网关地址无效: {}", config.fixed_gateway));
        }
    }
    if config.campus_gateway.is_empty() {
        config.campus_gateway = crate::config::default_campus_gateway();
    }
    if !config.campus_gateway.is_empty() {
        if config.campus_gateway.parse::<std::net::IpAddr>().is_err() {
            return Err(format!("校园网关地址无效: {}", config.campus_gateway));
        }
    }
    if config.required_network_name.is_empty() {
        config.required_network_name = crate::config::default_required_network_name();
    }
    Ok(config)
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
}

pub struct ExitState {
    pub is_quitting: std::sync::Arc<AtomicBool>,
    pub auto_exit_deadline: Mutex<Option<std::time::Instant>>,
    pub auto_exit_cancelled: AtomicBool,
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
            },
            exit: ExitState {
                is_quitting: std::sync::Arc::new(AtomicBool::new(false)),
                auto_exit_deadline: Mutex::new(None),
                auto_exit_cancelled: AtomicBool::new(false),
            },
            last_update_check_epoch_ms: AtomicU64::new(0),
            last_disabled_notification_ms: AtomicU64::new(0),
            last_render_heartbeat_ms: AtomicU64::new(0),
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
    pub fn ok_msg(msg: &str) -> Self {
        Self { success: true, message: Some(msg.to_string()), data: None }
    }
    pub fn err(msg: &str) -> Self {
        Self { success: false, message: Some(msg.to_string()), data: None }
    }
}

#[derive(Serialize)]
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
    pub fn ok_msg(msg: &str) -> Self {
        Self { success: true, message: Some(msg.to_string()), active_account: None, config: None }
    }
    pub fn err(msg: &str) -> Self {
        Self { success: false, message: Some(msg.to_string()), active_account: None, config: None }
    }
}
