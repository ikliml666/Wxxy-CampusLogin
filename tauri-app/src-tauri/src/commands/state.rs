use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::Instant;
use arc_swap::ArcSwap;
use crate::config::Config;
use parking_lot::Mutex;

pub const AUTO_EXIT_DELAY_MS: u64 = 10000;
pub const CANCEL_EXIT_SHORTCUT: &str = "CommandOrControl+Shift+C";
const LOGIN_RATE_LIMIT_SECS: u64 = 1;
const LOGIN_RATE_LIMIT_MAX: u32 = 1;

macro_rules! atomic_guard {
    ($name:ident, $field:ident) => {
        struct $name<'a>(&'a crate::commands::state::AppState);
        impl Drop for $name<'_> {
            fn drop(&mut self) {
                self.0.$field.store(false, Ordering::Release);
            }
        }
    };
}
pub(crate) use atomic_guard;

lazy_static::lazy_static! {
    static ref ACCOUNT_NAME_RE: regex::Regex = regex::Regex::new(r"^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$").unwrap();
    static ref CUSTOM_COLOR_RE: regex::Regex = regex::Regex::new(r"^#[0-9a-fA-F]{6}$").unwrap();
}

pub fn validate_config(config: Config) -> Result<Config, String> {
    let mut config = config;
    if !config.user.is_empty() {
        crate::config::validate_username(&config.user)?;
    }
    if !config.password.is_empty() {
        crate::config::validate_password(&config.password)?;
    }
    if config.operator == "@ctcc" {
        config.operator = "@telecom".to_string();
    } else if config.operator == "@cucc" {
        config.operator = "@unicom".to_string();
    }
    config.operator = crate::config::validate_operator(&config.operator).to_string();
    if config.adapter1.len() > 128 {
        return Err("适配器1名称过长".to_string());
    }
    if config.adapter2.len() > 128 {
        return Err("适配器2名称过长".to_string());
    }
    if config.active_account.len() > 64 {
        return Err("活动账号名称过长".to_string());
    }
    if !config.custom_theme_color.is_empty() {
        if !CUSTOM_COLOR_RE.is_match(&config.custom_theme_color) {
            config.custom_theme_color = "#6366f1".to_string();
        }
    }
    if config.default_panel.len() > 32 {
        config.default_panel = String::new();
    }
    if config.theme_mode != "dark" && config.theme_mode != "light" {
        config.theme_mode = "dark".to_string();
    }
    config.background_check_interval = config.background_check_interval.clamp(10000, 3600000);
    config.latency_test_interval = config.latency_test_interval.clamp(10000, 3600000);
    if config.portal_url == "http://10.1.99.100" {
        config.portal_url = "http://10.1.99.100:801".to_string();
    }
    if config.portal_url.is_empty() {
        config.portal_url = "http://10.1.99.100:801".to_string();
    }
    match url::Url::parse(&config.portal_url) {
        Ok(parsed) => {
            let scheme = parsed.scheme();
            if scheme != "http" && scheme != "https" {
                config.portal_url = "http://10.1.99.100:801".to_string();
            }
            if let Some(host) = parsed.host_str() {
                if let Ok(ip) = host.parse::<std::net::IpAddr>() {
                    match ip {
                        std::net::IpAddr::V4(v4) if v4.is_loopback() || v4.is_link_local() => {
                            config.portal_url = "http://10.1.99.100:801".to_string();
                        }
                        std::net::IpAddr::V6(v6) if v6.is_loopback() => {
                            config.portal_url = "http://10.1.99.100:801".to_string();
                        }
                        _ => {}
                    }
                } else if host == "localhost" {
                    config.portal_url = "http://10.1.99.100:801".to_string();
                }
            }
        }
        Err(_) => {
            config.portal_url = "http://10.1.99.100:801".to_string();
        }
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

pub struct AppState {
    pub config: ArcSwap<Config>,

    pub background_running: AtomicBool,
    pub latency_running: AtomicBool,
    pub is_checking: AtomicBool,
    pub is_logging_in: AtomicBool,
    pub is_quality_checking: AtomicBool,
    pub is_quitting: AtomicBool,

    pub server_available: AtomicBool,
    pub was_online: AtomicBool,
    pub has_logged_online: AtomicBool,
    pub background_check_count: AtomicU64,
    pub disconnect_reconnect_count: AtomicU32,

    pub auto_exit_deadline: Mutex<Option<Instant>>,
    pub auto_exit_cancelled: AtomicBool,
    pub last_network_quality: ArcSwap<Option<String>>,
    pub cached_online_status: ArcSwap<Option<serde_json::Value>>,
    pub last_disabled_notification_epoch_ms: AtomicU64,
    pub login_timestamps: Mutex<Vec<Instant>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            config: ArcSwap::from(std::sync::Arc::new(Config::default())),
            background_running: AtomicBool::new(false),
            latency_running: AtomicBool::new(false),
            is_checking: AtomicBool::new(false),
            is_logging_in: AtomicBool::new(false),
            is_quality_checking: AtomicBool::new(false),
            is_quitting: AtomicBool::new(false),
            server_available: AtomicBool::new(false),
            was_online: AtomicBool::new(false),
            has_logged_online: AtomicBool::new(false),
            background_check_count: AtomicU64::new(0),
            disconnect_reconnect_count: AtomicU32::new(0),
            auto_exit_deadline: Mutex::new(None),
            auto_exit_cancelled: AtomicBool::new(false),
            last_network_quality: ArcSwap::from(std::sync::Arc::new(None)),
            cached_online_status: ArcSwap::from(std::sync::Arc::new(None)),
            last_disabled_notification_epoch_ms: AtomicU64::new(0),
            login_timestamps: Mutex::new(Vec::new()),
        }
    }

    pub fn check_login_rate_limit(&self) -> Result<(), String> {
        let mut timestamps = self.login_timestamps.lock();
        let now = Instant::now();
        let cutoff = now - std::time::Duration::from_secs(LOGIN_RATE_LIMIT_SECS);
        timestamps.retain(|t| *t > cutoff);
        if timestamps.len() >= LOGIN_RATE_LIMIT_MAX as usize {
            return Err(format!("登录操作过于频繁，请{}秒后重试", LOGIN_RATE_LIMIT_SECS));
        }
        timestamps.push(now);
        Ok(())
    }

    pub fn auto_exit_deadline(&self) -> Option<Instant> {
        *self.auto_exit_deadline.lock()
    }

    pub fn set_auto_exit_deadline(&self, deadline: Option<Instant>) {
        *self.auto_exit_deadline.lock() = deadline;
    }

    pub fn last_disabled_notification_elapsed(&self) -> Option<std::time::Duration> {
        let epoch_ms = self.last_disabled_notification_epoch_ms.load(Ordering::Acquire);
        if epoch_ms == 0 {
            return None;
        }
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        if epoch_ms >= now_epoch {
            return Some(std::time::Duration::ZERO);
        }
        Some(std::time::Duration::from_millis(now_epoch - epoch_ms))
    }

    pub fn set_last_disabled_notification(&self) {
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.last_disabled_notification_epoch_ms.store(now_epoch, Ordering::Release);
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
