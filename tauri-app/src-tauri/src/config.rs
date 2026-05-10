use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager;
use regex::Regex;
use lazy_static::lazy_static;

lazy_static! {
    static ref USERNAME_RE: Regex = Regex::new(r"^[a-zA-Z0-9._-]+$").unwrap();
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub user: String,
    pub password: String,
    pub operator: String,
    pub adapter1: String,
    pub adapter2: String,
    #[serde(rename = "dualAdapter")]
    pub dual_adapter: bool,
    #[serde(rename = "autoLoginOnStart")]
    pub auto_login_on_start: bool,
    #[serde(rename = "autoExitAfterLogin")]
    pub auto_exit_after_login: bool,
    #[serde(rename = "minimizeToTray")]
    pub minimize_to_tray: bool,
    #[serde(rename = "hiddenStart")]
    pub hidden_start: bool,
    #[serde(rename = "autoLaunch")]
    pub auto_launch: bool,
    #[serde(rename = "enableBackgroundCheck")]
    pub enable_background_check: bool,
    #[serde(rename = "backgroundCheckInterval")]
    pub background_check_interval: u64,
    #[serde(rename = "autoLoginOnPreparation")]
    pub auto_login_on_preparation: bool,
    #[serde(rename = "autoExitOnOnline")]
    pub auto_exit_on_online: bool,
    #[serde(rename = "themeMode")]
    pub theme_mode: String,
    #[serde(rename = "enableNotification")]
    pub enable_notification: bool,
    #[serde(rename = "activeAccount")]
    pub active_account: String,
    #[serde(rename = "enableLatencyTest")]
    pub enable_latency_test: bool,
    #[serde(rename = "latencyTestInterval")]
    pub latency_test_interval: u64,
    #[serde(rename = "customThemeColor")]
    pub custom_theme_color: String,
    #[serde(rename = "defaultPanel")]
    pub default_panel: String,
    #[serde(rename = "enableNetworkQuality")]
    pub enable_network_quality: bool,
    #[serde(rename = "skipTtfbInLatency", default = "default_true")]
    pub skip_ttfb_in_latency: bool,
    #[serde(rename = "skipContentInLatency", default = "default_true")]
    pub skip_content_in_latency: bool,
    #[serde(rename = "portalUrl", default = "default_portal_url")]
    pub portal_url: String,
}

fn default_true() -> bool { true }

fn default_portal_url() -> String {
    "http://10.1.99.100:801".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            user: String::new(),
            password: String::new(),
            operator: String::new(),
            adapter1: "自动检测".to_string(),
            adapter2: String::new(),
            dual_adapter: false,
            auto_login_on_start: true,
            auto_exit_after_login: true,
            minimize_to_tray: false,
            hidden_start: true,
            auto_launch: true,
            enable_background_check: true,
            background_check_interval: 60000,
            auto_login_on_preparation: true,
            auto_exit_on_online: true,
            theme_mode: "dark".to_string(),
            enable_notification: true,
            active_account: String::new(),
            enable_latency_test: false,
            latency_test_interval: 30000,
            custom_theme_color: "#6366f1".to_string(),
            default_panel: String::new(),
            enable_network_quality: true,
            skip_ttfb_in_latency: true,
            skip_content_in_latency: true,
            portal_url: default_portal_url(),
        }
    }
}

pub fn get_data_dir(app_handle: &tauri::AppHandle) -> PathBuf {
    let tauri_dir = app_handle.path().app_data_dir().unwrap_or_else(|_| {
        dirs::data_dir().unwrap_or_else(|| PathBuf::from("."))
    });

    if !tauri_dir.exists() {
        let _ = std::fs::create_dir_all(&tauri_dir);
    }

    tauri_dir
}

pub fn get_config_path(data_dir: &PathBuf) -> PathBuf {
    data_dir.join("config.json")
}

pub fn get_accounts_dir(data_dir: &PathBuf) -> PathBuf {
    data_dir.join("accounts")
}

pub fn get_login_history_path(data_dir: &PathBuf) -> PathBuf {
    data_dir.join("login-history.json")
}

pub fn validate_username(user: &str) -> Result<&str, String> {
    if user.is_empty() {
        return Err("用户名不能为空".to_string());
    }
    if user.len() > 64 {
        return Err("用户名过长".to_string());
    }
    if !USERNAME_RE.is_match(user) {
        return Err("用户名包含非法字符".to_string());
    }
    Ok(user)
}

pub fn validate_operator(op: &str) -> &str {
    if ["", "@telecom", "@unicom", "@cmcc"].contains(&op) { op } else { "" }
}

pub fn validate_password(password: &str) -> Result<(), String> {
    if password.is_empty() {
        return Err("密码不能为空".to_string());
    }
    if password.len() > 128 {
        return Err("密码过长".to_string());
    }
    Ok(())
}
