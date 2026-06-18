use regex::Regex;
use lazy_static::lazy_static;
use super::model::{Config, PASSWORD_MASK, default_campus_gateway, default_required_network_name};

lazy_static! {
    static ref USERNAME_RE: Regex = Regex::new(r"^[a-zA-Z0-9._-]+$").expect("USERNAME_RE compilation failed");
    static ref CUSTOM_COLOR_RE: Regex = Regex::new(r"^#[0-9a-fA-F]{6}$").expect("CUSTOM_COLOR_RE compilation failed");
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

/// 校验 Portal URL：协议必须 http/https，host 必须是内网 IP 或 localhost
fn validate_portal_url(url: &str) -> Result<(), String> {
    match url::Url::parse(url) {
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
            Ok(())
        }
        Err(e) => Err(format!("Portal地址格式无效: {}", e)),
    }
}

pub fn validate_config(config: Config) -> Result<Config, String> {
    let mut config = config;
    if !config.user.is_empty() {
        validate_username(&config.user)?;
    }
    if !config.password.is_empty() && config.password != PASSWORD_MASK {
        validate_password(&config.password)?;
    }
    if config.operator == "@ctcc" {
        config.operator = "@telecom".to_string();
    } else if config.operator == "@cucc" {
        config.operator = "@unicom".to_string();
    }
    config.operator = validate_operator(&config.operator)?.to_string();
    if !config.custom_theme_color.is_empty() && !CUSTOM_COLOR_RE.is_match(&config.custom_theme_color) {
        return Err("自定义主题颜色格式无效，需为#开头的6位十六进制色值".to_string());
    }
    if config.theme_mode != "dark" && config.theme_mode != "light" && config.theme_mode != "system" {
        return Err("主题模式必须为\"dark\"、\"light\"或\"system\"".to_string());
    }
    config.background_check_interval = config.background_check_interval.clamp(10000, 3600000);
    config.latency_test_interval = config.latency_test_interval.clamp(10000, 3600000);
    if config.portal_url == "http://10.1.99.100:801" || config.portal_url.is_empty() {
        config.portal_url = "http://10.1.99.100".to_string();
    }
    validate_portal_url(&config.portal_url)?;
    if !config.fixed_gateway.is_empty() && config.fixed_gateway.parse::<std::net::IpAddr>().is_err() {
        return Err(format!("固定网关地址无效: {}", config.fixed_gateway));
    }
    if config.campus_gateway.is_empty() {
        config.campus_gateway = default_campus_gateway();
    }
    if !config.campus_gateway.is_empty() && config.campus_gateway.parse::<std::net::IpAddr>().is_err() {
        return Err(format!("校园网关地址无效: {}", config.campus_gateway));
    }
    if config.required_network_name.is_empty() {
        config.required_network_name = default_required_network_name();
    }
    // 0 表示永久保留（见 logger.rs cleanup_old_logs_by_time），仅限制上限，不重置为默认值
    if config.log_retention_days > 365 {
        config.log_retention_days = 365;
    }
    // 配置版本迁移：config_version < 2 为旧版，campus_check_start_minutes 可能是旧字段 campusCheckStartHour 的小时值（通过 alias 反序列化）
    // config_version >= 2 为新版，campus_check_start_minutes 直接是分钟值
    if config.config_version < 2 {
        // 旧配置：值 > 0 且 < 24 视为小时值，转为分钟
        if config.campus_check_start_minutes > 0 && config.campus_check_start_minutes < 24 {
            config.campus_check_start_minutes *= 60;
        }
        // 迁移完成，升级配置版本
        config.config_version = 2;
    }
    config.campus_check_start_minutes = config.campus_check_start_minutes.min(1439);
    Ok(config)
}

/// 宽松验证：对每个字段独立降级，无效字段回退默认值并记录警告。
/// 用于加载磁盘配置，避免单个字段无效导致全量配置丢失（F1）。
/// 保存/导入仍应使用 validate_config（严格版）拒绝非法输入。
pub fn validate_config_lenient(mut config: Config) -> Config {
    let defaults = Config::default();

    // user
    if !config.user.is_empty() {
        if let Err(e) = validate_username(&config.user) {
            crate::log_warn!("config", "用户名字段无效，回退默认: {}", e);
            config.user = defaults.user;
        }
    }
    // password
    if !config.password.is_empty() && config.password != PASSWORD_MASK {
        if let Err(e) = validate_password(&config.password) {
            crate::log_warn!("config", "密码字段无效，回退默认: {}", e);
            config.password = defaults.password;
        }
    }
    // operator 迁移
    if config.operator == "@ctcc" {
        config.operator = "@telecom".to_string();
    } else if config.operator == "@cucc" {
        config.operator = "@unicom".to_string();
    }
    if let Err(e) = validate_operator(&config.operator) {
        crate::log_warn!("config", "运营商后缀无效，回退默认: {}", e);
        config.operator = defaults.operator;
    }
    // custom_theme_color
    if !config.custom_theme_color.is_empty() && !CUSTOM_COLOR_RE.is_match(&config.custom_theme_color) {
        crate::log_warn!("config", "主题颜色格式无效({})，回退默认", config.custom_theme_color);
        config.custom_theme_color = defaults.custom_theme_color;
    }
    // theme_mode
    if !["dark", "light", "system"].contains(&config.theme_mode.as_str()) {
        crate::log_warn!("config", "主题模式无效({})，回退默认", config.theme_mode);
        config.theme_mode = defaults.theme_mode;
    }
    // portal_url 规范化 + 校验
    if config.portal_url == "http://10.1.99.100:801" || config.portal_url.is_empty() {
        config.portal_url = "http://10.1.99.100".to_string();
    }
    if let Err(e) = validate_portal_url(&config.portal_url) {
        crate::log_warn!("config", "Portal地址无效({})，回退默认: {}", config.portal_url, e);
        config.portal_url = defaults.portal_url;
    }
    // fixed_gateway
    if !config.fixed_gateway.is_empty() && config.fixed_gateway.parse::<std::net::IpAddr>().is_err() {
        crate::log_warn!("config", "固定网关地址无效({})，回退默认", config.fixed_gateway);
        config.fixed_gateway = defaults.fixed_gateway;
    }
    // campus_gateway
    if config.campus_gateway.is_empty() {
        config.campus_gateway = default_campus_gateway();
    }
    if !config.campus_gateway.is_empty() && config.campus_gateway.parse::<std::net::IpAddr>().is_err() {
        crate::log_warn!("config", "校园网关地址无效({})，回退默认", config.campus_gateway);
        config.campus_gateway = defaults.campus_gateway;
    }
    if config.required_network_name.is_empty() {
        config.required_network_name = default_required_network_name();
    }

    // 降级后跑严格验证兜底（处理 clamp/迁移/其他未覆盖字段）
    match validate_config(config) {
        Ok(c) => c,
        Err(e) => {
            crate::log_warn!("config", "降级后仍验证失败({})，使用全默认配置", e);
            Config::default()
        }
    }
}
