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

pub fn validate_config(config: Config) -> Result<Config, String> {
    let mut config = config;
    if !config.user.is_empty() {
        validate_username(&config.user)?;
    }
    if !config.password.is_empty() {
        if config.password != PASSWORD_MASK {
            validate_password(&config.password)?;
        }
    }
    if config.operator == "@ctcc" {
        config.operator = "@telecom".to_string();
    } else if config.operator == "@cucc" {
        config.operator = "@unicom".to_string();
    }
    config.operator = validate_operator(&config.operator)?.to_string();
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
        config.campus_gateway = default_campus_gateway();
    }
    if !config.campus_gateway.is_empty() {
        if config.campus_gateway.parse::<std::net::IpAddr>().is_err() {
            return Err(format!("校园网关地址无效: {}", config.campus_gateway));
        }
    }
    if config.required_network_name.is_empty() {
        config.required_network_name = default_required_network_name();
    }
    if config.log_retention_days > 365 {
        config.log_retention_days = 7;
    }
    // 兼容旧配置：旧字段 campusCheckStartHour 值为 0-23 的小时值
    // 如果值 < 24 且不是 0（0 表示禁用，保持不变），视为小时值并转为分钟
    if config.campus_check_start_minutes > 0 && config.campus_check_start_minutes < 24 {
        config.campus_check_start_minutes *= 60;
    }
    config.campus_check_start_minutes = config.campus_check_start_minutes.min(1439);
    Ok(config)
}
