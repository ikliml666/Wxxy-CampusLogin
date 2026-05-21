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
    #[serde(default)]
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
    #[serde(rename = "fixedGateway", default)]
    pub fixed_gateway: String,
    #[serde(rename = "requiredNetworkName", default = "default_required_network_name", deserialize_with = "deserialize_required_network_name")]
    pub required_network_name: String,
    #[serde(rename = "enableNetworkNameCheck", default = "default_true")]
    pub enable_network_name_check: bool,
    #[serde(rename = "campusGateway", default = "default_campus_gateway", deserialize_with = "deserialize_campus_gateway")]
    pub campus_gateway: String,
}

fn deserialize_non_empty_or<'de, D>(deserializer: D, default_fn: fn() -> String) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer)?;
    if s.is_empty() {
        Ok(default_fn())
    } else {
        Ok(s)
    }
}

fn deserialize_campus_gateway<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_non_empty_or(deserializer, default_campus_gateway)
}

fn deserialize_required_network_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_non_empty_or(deserializer, default_required_network_name)
}

fn default_true() -> bool { true }

pub fn default_portal_url() -> String {
    "http://10.1.99.100".to_string()
}

pub fn default_required_network_name() -> String {
    "i-wxxy".to_string()
}

pub fn default_campus_gateway() -> String {
    "10.2.127.254".to_string()
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
            portal_url: "http://10.1.99.100".to_string(),
            fixed_gateway: "10.2.127.254".to_string(),
            required_network_name: "i-wxxy".to_string(),
            enable_network_name_check: true,
            campus_gateway: "10.2.127.254".to_string(),
        }
    }
}

impl Config {
    pub fn masked_for_display(&self) -> Config {
        let mut c = self.clone();
        if !c.password.is_empty() {
            c.password = "***".to_string();
        }
        c
    }

    pub fn user_account_with_operator(&self) -> String {
        if !self.operator.is_empty() && self.operator != "__default__" {
            format!("{}{}", self.user, self.operator)
        } else {
            self.user.clone()
        }
    }
}

pub fn atomic_write(path: &std::path::Path, content: &str) -> Result<(), String> {
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, content)
        .map_err(|e| format!("写入临时文件失败: {}", e))?;
    for attempt in 0..3 {
        if std::fs::rename(&tmp_path, path).is_ok() {
            return Ok(());
        }
        if attempt < 2 {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    let _ = std::fs::remove_file(&tmp_path);
    Err(format!("重命名临时文件失败（重试3次后）"))
}

pub fn get_data_dir(app_handle: &tauri::AppHandle) -> PathBuf {
    let tauri_dir = app_handle.path().app_data_dir().unwrap_or_else(|_| {
        dirs::data_dir().unwrap_or_else(|| PathBuf::from("."))
    });

    if !tauri_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&tauri_dir) {
            crate::log_warn!("config", "创建Tauri数据目录失败: {}", e);
        }
    }

    tauri_dir
}

pub fn get_config_path(data_dir: &PathBuf) -> PathBuf {
    data_dir.join("config.json")
}

pub fn get_accounts_dir(data_dir: &PathBuf) -> PathBuf {
    data_dir.join("accounts")
}

pub fn list_account_names(app_handle: &tauri::AppHandle) -> Vec<String> {
    let data_dir = get_data_dir(app_handle);
    let accounts_dir = get_accounts_dir(&data_dir);

    if !accounts_dir.exists() {
        return vec![];
    }

    let mut accounts = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&accounts_dir) {
        for entry in entries.flatten() {
            if entry.path().extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(name) = entry.path().file_stem().and_then(|n| n.to_str()) {
                    accounts.push(name.to_string());
                }
            }
        }
    }

    accounts.sort();
    accounts
}

pub(crate) fn get_login_history_path(data_dir: &PathBuf) -> PathBuf {
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

pub fn validate_operator(op: &str) -> Result<&str, String> {
    if ["", "@telecom", "@unicom", "@cmcc"].contains(&op) {
        Ok(op)
    } else {
        Err(format!("运营商后缀无效: {}，可选：@telecom、@unicom、@cmcc", op))
    }
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
