use serde::{Deserialize, Serialize};

pub const PASSWORD_MASK: &str = "***";
pub const AUTO_DETECT_ADAPTER: &str = "自动检测";

#[derive(Debug, Clone, Serialize, Deserialize)]
// 容器级 default：任一字段缺失（旧版本配置/手工编辑）都用 Default 补齐，
// 否则 serde 缺一个字段即整体反序列化失败 → 上层全量重置丢配置
#[serde(default)]
pub struct Config {
    pub user: String,
    #[serde(default)]
    pub password: String,
    /// 自助服务系统登录密码（内存明文，磁盘 DPAPI 加密；回传前端时替换为 MASK）
    #[serde(rename = "selfPassword", default)]
    pub self_password: String,
    /// 是否启用 Windows Hello 操作验证（默认 true）。关闭后自助服务面板与绑定
    /// 操作不再弹验证，但"查看运营商账户密码"仍强制验证（后端 TTL 校验不受此
    /// 开关影响，防止一键关闭保护后明文裸奔）
    #[serde(rename = "selfHelloEnabled", default = "default_true")]
    pub self_hello_enabled: bool,
    /// 自助服务面板每次操作都二次验证（默认 false：切入面板验证一次后操作共用）
    #[serde(rename = "selfReverifyEachAction", default)]
    pub self_reverify_each_action: bool,
    pub operator: String,
    pub adapter1: String,
    pub adapter2: String,
    /// 主适配器绑定的账号 id（R1，设备级配置，仅桌面）；空 = 跟随当前激活账号。
    /// 切换账号时不得被账号档案覆盖（切账号不改变"网卡→账号"映射）
    #[serde(rename = "adapter1Account")]
    pub adapter1_account: String,
    /// 副适配器绑定的账号 id，语义同 adapter1_account
    #[serde(rename = "adapter2Account")]
    pub adapter2_account: String,
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
    /// 晚间断网自动切换运营商总开关：到点把 operator 切至无锡学院（空串），次日
    /// 恢复窗口内切回；判定逻辑见 config::night_switch（跨平台纯函数）
    #[serde(rename = "enableNightOperatorSwitch", default)]
    pub enable_night_operator_switch: bool,
    /// 切至无锡学院前暂存的原运营商；空 = 未处于切换态。恢复后清空
    #[serde(rename = "nightOperatorRestore", default)]
    pub night_operator_restore: String,
    #[serde(rename = "autoExitOnOnline")]
    pub auto_exit_on_online: bool,
    #[serde(rename = "themeMode")]
    pub theme_mode: String,
    #[serde(rename = "enableNotification")]
    pub enable_notification: bool,
    #[serde(rename = "activeAccount")]
    pub active_account: String,
    /// 本文件所属账号的显示名（R3，与内部 id 分离，可含空格/emoji 等任意字符）；
    /// 空 → 读取阶段兜底用账号 id
    #[serde(rename = "displayName")]
    pub display_name: String,
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
    #[serde(rename = "fixedGateway", default = "default_fixed_gateway")]
    pub fixed_gateway: String,
    #[serde(rename = "requiredNetworkName", default = "default_required_network_name", deserialize_with = "deserialize_required_network_name")]
    pub required_network_name: String,
    #[serde(rename = "enableNetworkNameCheck", default = "default_true")]
    pub enable_network_name_check: bool,
    #[serde(rename = "campusGateway", default = "default_campus_gateway", deserialize_with = "deserialize_campus_gateway")]
    pub campus_gateway: String,
    /// 检查/下载更新渠道优先级: "mirror"=镜像加速优先(默认) | "github"=官方优先
    #[serde(rename = "updateSource", default)]
    pub update_source: String,
    #[serde(rename = "campusExitOnFail", default = "default_true")]
    pub campus_exit_on_fail: bool,
    /// 非校园网自动退出生效时段起点（分钟数，480=8:00；结束<=起点时视为仅受起点限制）
    #[serde(rename = "campusExitStartMinutes", default = "default_campus_exit_start_minutes")]
    pub campus_exit_start_minutes: u16,
    /// 非校园网自动退出生效时段终点（分钟数，1380=23:00，不含该时刻）
    #[serde(rename = "campusExitEndMinutes", default = "default_campus_exit_end_minutes")]
    pub campus_exit_end_minutes: u16,
    #[serde(rename = "campusCheckStartMinutes", alias = "campusCheckStartHour", default = "default_campus_check_start_minutes")]
    pub campus_check_start_minutes: u16,
    /// 校园网检测时段终点（分钟数，1380=23:00；<= 开始时间时退化为仅开始时间限制）
    #[serde(rename = "campusCheckEndMinutes", default = "default_campus_check_end_minutes")]
    pub campus_check_end_minutes: u16,
    /// 每日定时登录时刻（分钟数，0=禁用；到点即触发含过点补触发，判定见 config::schedule）
    #[serde(rename = "scheduledLoginMinutes", default)]
    pub scheduled_login_minutes: u16,
    /// 每日定时注销时刻（分钟数，0=禁用；语义同 scheduled_login_minutes）
    #[serde(rename = "scheduledLogoutMinutes", default)]
    pub scheduled_logout_minutes: u16,
    #[serde(rename = "logRetentionDays", default = "default_log_retention_days")]
    pub log_retention_days: u32,
    #[serde(rename = "maxDisconnectReconnect", default = "default_max_disconnect_reconnect")]
    pub max_disconnect_reconnect: u32,
    #[serde(rename = "autoLoginCooldownSecs", default = "default_auto_login_cooldown_secs")]
    pub auto_login_cooldown_secs: u64,
    #[serde(rename = "skipSha256WhenMissing", default)]
    pub skip_sha256_when_missing: bool,
    #[serde(rename = "configVersion", default)]
    pub config_version: u32,
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

fn default_campus_check_start_minutes() -> u16 { 460 }

/// 2026-09-13 起 1380=23:00（旧默认 0=仅开始时间限制），存量配置由
/// validate 的 config_version v2→v3 迁移一次性刷新
fn default_campus_check_end_minutes() -> u16 { 1380 }

fn default_campus_exit_start_minutes() -> u16 { 480 }

fn default_campus_exit_end_minutes() -> u16 { 1380 }

pub fn default_fixed_gateway() -> String {
    "10.2.127.254".to_string()
}

pub fn default_log_retention_days() -> u32 { 7 }

fn default_max_disconnect_reconnect() -> u32 { 3 }

fn default_auto_login_cooldown_secs() -> u64 { 60 }

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
            self_password: String::new(),
            self_hello_enabled: true,
            self_reverify_each_action: false,
            operator: String::new(),
            adapter1: AUTO_DETECT_ADAPTER.to_string(),
            adapter2: String::new(),
            adapter1_account: String::new(),
            adapter2_account: String::new(),
            dual_adapter: false,
            auto_login_on_start: true,
            auto_exit_after_login: true,
            minimize_to_tray: false,
            hidden_start: false,
            auto_launch: true,
            enable_background_check: true,
            background_check_interval: 15000,
            auto_login_on_preparation: true,
            enable_night_operator_switch: false,
            night_operator_restore: String::new(),
            auto_exit_on_online: true,
            theme_mode: "dark".to_string(),
            enable_notification: true,
            active_account: String::new(),
            display_name: String::new(),
            enable_latency_test: true,
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
            update_source: "mirror".to_string(),
            campus_exit_on_fail: true,
            campus_exit_start_minutes: 480,
            campus_exit_end_minutes: 1380,
            campus_check_start_minutes: 460,
            campus_check_end_minutes: 1380,
            scheduled_login_minutes: 0,
            scheduled_logout_minutes: 0,
            log_retention_days: 7,
            max_disconnect_reconnect: 3,
            auto_login_cooldown_secs: 60,
            skip_sha256_when_missing: false,
            config_version: 3,
        }
    }
}

impl Config {
    /// 出站掩码（唯一出口约定：所有把 Config 发往前端的路径必须经由本方法，
    /// 不得手工逐字段打码——漏一个字段就是一次明文泄露，account 三命令即前车之鉴）。
    /// 空值保留（未设置语义），非空一律替换为 MASK。
    pub fn masked_for_display(&self) -> Config {
        let mut c = self.clone();
        c.mask_in_place();
        c
    }

    pub fn mask_in_place(&mut self) {
        if !self.password.is_empty() {
            self.password = PASSWORD_MASK.to_string();
        }
        if !self.self_password.is_empty() {
            self.self_password = PASSWORD_MASK.to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_reconnect_and_cooldown_config() {
        let config = Config::default();
        assert_eq!(config.max_disconnect_reconnect, 3);
        assert_eq!(config.auto_login_cooldown_secs, 60);
    }

    #[test]
    fn serde_missing_reconnect_field_uses_default() {
        // 从默认 Config 序列化的 JSON 中删除新字段，验证反序列化时回退到默认值
        let mut json = serde_json::to_value(Config::default()).unwrap();
        let obj = json.as_object_mut().unwrap();
        obj.remove("maxDisconnectReconnect");
        obj.remove("autoLoginCooldownSecs");
        let config: Config = serde_json::from_value(json).unwrap();
        assert_eq!(config.max_disconnect_reconnect, 3);
        assert_eq!(config.auto_login_cooldown_secs, 60);
    }

    #[test]
    fn serde_custom_reconnect_config() {
        let mut json = serde_json::to_value(Config::default()).unwrap();
        json["maxDisconnectReconnect"] = serde_json::json!(5);
        json["autoLoginCooldownSecs"] = serde_json::json!(120);
        let config: Config = serde_json::from_value(json).unwrap();
        assert_eq!(config.max_disconnect_reconnect, 5);
        assert_eq!(config.auto_login_cooldown_secs, 120);
    }

    /// 新增账号字段 serde 契约锁（双端一致，字段名不得擅改）：
    /// displayName / adapter1Account / adapter2Account，缺省回退空串
    #[test]
    fn serde_account_fields_json_names_and_defaults() {
        let json = serde_json::to_value(Config::default()).unwrap();
        assert_eq!(json["displayName"], "");
        assert_eq!(json["adapter1Account"], "");
        assert_eq!(json["adapter2Account"], "");

        // 旧版本配置文件缺这三个字段 → 容器级 serde(default) 补空串，不整体失败
        let mut old = serde_json::to_value(Config::default()).unwrap();
        let obj = old.as_object_mut().unwrap();
        obj.remove("displayName");
        obj.remove("adapter1Account");
        obj.remove("adapter2Account");
        let config: Config = serde_json::from_value(old).unwrap();
        assert_eq!(config.display_name, "");
        assert_eq!(config.adapter1_account, "");
        assert_eq!(config.adapter2_account, "");

        // 序列化回读
        let config = Config {
            display_name: "我的账号".to_string(),
            adapter1_account: "acc-1".to_string(),
            adapter2_account: "acc_2".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["displayName"], "我的账号");
        assert_eq!(json["adapter1Account"], "acc-1");
        assert_eq!(json["adapter2Account"], "acc_2");
    }
}
