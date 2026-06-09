use serde::{Deserialize, Serialize};

pub const PASSWORD_MASK: &str = "***";

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
    #[serde(rename = "campusExitOnFail", default = "default_true")]
    pub campus_exit_on_fail: bool,
    #[serde(rename = "campusCheckStartMinutes", alias = "campusCheckStartHour", default = "default_campus_check_start_minutes")]
    pub campus_check_start_minutes: u16,
    #[serde(rename = "logRetentionDays", default)]
    pub log_retention_days: u32,
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

fn default_campus_check_start_minutes() -> u16 { 480 }

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
            background_check_interval: 15000,
            auto_login_on_preparation: true,
            auto_exit_on_online: true,
            theme_mode: "dark".to_string(),
            enable_notification: true,
            active_account: String::new(),
            enable_latency_test: false,
            latency_test_interval: 60000,
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
            campus_exit_on_fail: true,
            campus_check_start_minutes: 480,
            log_retention_days: 7,
        }
    }
}

impl Config {
    pub fn masked_for_display(&self) -> Config {
        let mut c = self.clone();
        if !c.password.is_empty() {
            c.password = PASSWORD_MASK.to_string();
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
