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
        Err(format!("运营商后缀无效: {op}，可选：@telecom、@unicom、@cmcc"))
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
                return Err(format!("Portal地址协议不支持: {scheme}，仅允许http/https"));
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
        Err(e) => Err(format!("Portal地址格式无效: {e}")),
    }
}

fn migrate_operator(op: &mut String) {
    if *op == "@ctcc" {
        *op = "@telecom".to_string();
    } else if *op == "@cucc" {
        *op = "@unicom".to_string();
    }
}

fn normalize_portal_url(url: &mut String) {
    if *url == "http://10.1.99.100:801" || url.is_empty() {
        *url = "http://10.1.99.100".to_string();
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
    migrate_operator(&mut config.operator);
    config.operator = validate_operator(&config.operator)?.to_string();
    if !config.custom_theme_color.is_empty() && !CUSTOM_COLOR_RE.is_match(&config.custom_theme_color) {
        return Err("自定义主题颜色格式无效，需为#开头的6位十六进制色值".to_string());
    }
    if config.theme_mode != "dark" && config.theme_mode != "light" && config.theme_mode != "system" {
        return Err("主题模式必须为\"dark\"、\"light\"或\"system\"".to_string());
    }
    config.background_check_interval = config.background_check_interval.clamp(10000, 3600000);
    config.latency_test_interval = config.latency_test_interval.clamp(10000, 3600000);
    normalize_portal_url(&mut config.portal_url);
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
    config.campus_check_end_minutes = config.campus_check_end_minutes.min(1439);
    config.campus_exit_start_minutes = config.campus_exit_start_minutes.min(1439);
    config.campus_exit_end_minutes = config.campus_exit_end_minutes.min(1439);
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
    migrate_operator(&mut config.operator);
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
    normalize_portal_url(&mut config.portal_url);
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

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    // ===== validate_username =====

    #[test]
    fn validate_username_valid_simple() {
        assert_eq!(validate_username("user123"), Ok("user123"));
    }

    #[test]
    fn validate_username_valid_with_dots() {
        assert_eq!(validate_username("user.name"), Ok("user.name"));
    }

    #[test]
    fn validate_username_valid_with_hyphens() {
        assert_eq!(validate_username("user-name"), Ok("user-name"));
    }

    #[test]
    fn validate_username_valid_with_underscores() {
        assert_eq!(validate_username("user_name"), Ok("user_name"));
    }

    #[test]
    fn validate_username_valid_single_char() {
        assert_eq!(validate_username("a"), Ok("a"));
    }

    #[test]
    fn validate_username_valid_max_length() {
        let user = "a".repeat(64);
        assert_eq!(validate_username(&user), Ok(user.as_str()));
    }

    #[test]
    fn validate_username_rejects_empty() {
        assert!(validate_username("").is_err());
    }

    #[test]
    fn validate_username_rejects_too_long() {
        let user = "a".repeat(65);
        assert!(validate_username(&user).is_err());
    }

    #[test]
    fn validate_username_rejects_at_sign() {
        assert!(validate_username("user@domain").is_err());
    }

    #[test]
    fn validate_username_rejects_spaces() {
        assert!(validate_username("user name").is_err());
    }

    #[test]
    fn validate_username_rejects_chinese() {
        assert!(validate_username("用户名").is_err());
    }

    #[test]
    fn validate_username_rejects_special_chars() {
        assert!(validate_username("user!").is_err());
        assert!(validate_username("user#").is_err());
        assert!(validate_username("user/").is_err());
    }

    // ===== validate_operator =====

    #[test]
    fn validate_operator_valid_empty() {
        assert_eq!(validate_operator(""), Ok(""));
    }

    #[test]
    fn validate_operator_valid_telecom() {
        assert_eq!(validate_operator("@telecom"), Ok("@telecom"));
    }

    #[test]
    fn validate_operator_valid_unicom() {
        assert_eq!(validate_operator("@unicom"), Ok("@unicom"));
    }

    #[test]
    fn validate_operator_valid_cmcc() {
        assert_eq!(validate_operator("@cmcc"), Ok("@cmcc"));
    }

    #[test]
    fn validate_operator_rejects_ctcc() {
        // validate_operator itself does NOT migrate; migration happens in validate_config
        assert!(validate_operator("@ctcc").is_err());
    }

    #[test]
    fn validate_operator_rejects_cucc() {
        assert!(validate_operator("@cucc").is_err());
    }

    #[test]
    fn validate_operator_rejects_unknown() {
        assert!(validate_operator("@unknown").is_err());
    }

    #[test]
    fn validate_operator_rejects_no_at_prefix() {
        assert!(validate_operator("telecom").is_err());
    }

    // ===== validate_password =====

    #[test]
    fn validate_password_valid_simple() {
        assert_eq!(validate_password("pass"), Ok(()));
    }

    #[test]
    fn validate_password_valid_single_char() {
        assert_eq!(validate_password("a"), Ok(()));
    }

    #[test]
    fn validate_password_valid_max_length() {
        let pwd = "a".repeat(128);
        assert_eq!(validate_password(&pwd), Ok(()));
    }

    #[test]
    fn validate_password_rejects_empty() {
        assert!(validate_password("").is_err());
    }

    #[test]
    fn validate_password_rejects_too_long() {
        let pwd = "a".repeat(129);
        assert!(validate_password(&pwd).is_err());
    }

    // ===== validate_portal_url =====

    #[test]
    fn validate_portal_url_valid_http_private() {
        assert!(validate_portal_url("http://10.1.99.100").is_ok());
    }

    #[test]
    fn validate_portal_url_valid_https_private() {
        assert!(validate_portal_url("https://10.1.99.100").is_ok());
    }

    #[test]
    fn validate_portal_url_valid_loopback() {
        assert!(validate_portal_url("http://127.0.0.1").is_ok());
    }

    #[test]
    fn validate_portal_url_valid_192_168() {
        assert!(validate_portal_url("http://192.168.1.1").is_ok());
    }

    #[test]
    fn validate_portal_url_valid_localhost() {
        assert!(validate_portal_url("http://localhost").is_ok());
    }

    #[test]
    fn validate_portal_url_valid_with_port() {
        assert!(validate_portal_url("http://10.1.99.100:801").is_ok());
    }

    #[test]
    fn validate_portal_url_rejects_public_ip() {
        assert!(validate_portal_url("http://8.8.8.8").is_err());
    }

    #[test]
    fn validate_portal_url_rejects_domain() {
        assert!(validate_portal_url("http://example.com").is_err());
    }

    #[test]
    fn validate_portal_url_rejects_ftp_scheme() {
        assert!(validate_portal_url("ftp://10.1.99.100").is_err());
    }

    #[test]
    fn validate_portal_url_rejects_invalid_url() {
        assert!(validate_portal_url("not a url").is_err());
    }

    #[test]
    fn validate_portal_url_rejects_empty() {
        assert!(validate_portal_url("").is_err());
    }

    // ===== validate_config =====

    #[test]
    fn validate_config_default_ok() {
        assert!(validate_config(Config::default()).is_ok());
    }

    #[test]
    fn validate_config_rejects_bad_username() {
        let mut config = Config::default();
        config.user = "user@bad".to_string();
        assert!(validate_config(config).is_err());
    }

    #[test]
    fn validate_config_rejects_bad_operator() {
        let mut config = Config::default();
        config.operator = "@unknown".to_string();
        assert!(validate_config(config).is_err());
    }

    #[test]
    fn validate_config_migrates_ctcc_operator() {
        let mut config = Config::default();
        config.operator = "@ctcc".to_string();
        let result = validate_config(config).unwrap();
        assert_eq!(result.operator, "@telecom");
    }

    #[test]
    fn validate_config_migrates_cucc_operator() {
        let mut config = Config::default();
        config.operator = "@cucc".to_string();
        let result = validate_config(config).unwrap();
        assert_eq!(result.operator, "@unicom");
    }

    #[test]
    fn validate_config_rejects_bad_theme_mode() {
        let mut config = Config::default();
        config.theme_mode = "purple".to_string();
        assert!(validate_config(config).is_err());
    }

    #[test]
    fn validate_config_rejects_bad_color() {
        let mut config = Config::default();
        config.custom_theme_color = "red".to_string();
        assert!(validate_config(config).is_err());
    }

    #[test]
    fn validate_config_rejects_bad_portal_url() {
        let mut config = Config::default();
        config.portal_url = "http://8.8.8.8".to_string();
        assert!(validate_config(config).is_err());
    }

    #[test]
    fn validate_config_rejects_bad_fixed_gateway() {
        let mut config = Config::default();
        config.fixed_gateway = "not-an-ip".to_string();
        assert!(validate_config(config).is_err());
    }

    #[test]
    fn validate_config_rejects_bad_campus_gateway() {
        let mut config = Config::default();
        config.campus_gateway = "not-an-ip".to_string();
        assert!(validate_config(config).is_err());
    }

    #[test]
    fn validate_config_clamps_background_check_interval_low() {
        let mut config = Config::default();
        config.background_check_interval = 1000;
        let result = validate_config(config).unwrap();
        assert_eq!(result.background_check_interval, 10000);
    }

    #[test]
    fn validate_config_clamps_background_check_interval_high() {
        let mut config = Config::default();
        config.background_check_interval = 5000000;
        let result = validate_config(config).unwrap();
        assert_eq!(result.background_check_interval, 3600000);
    }

    #[test]
    fn validate_config_clamps_latency_test_interval() {
        let mut config = Config::default();
        config.latency_test_interval = 1000;
        let result = validate_config(config).unwrap();
        assert_eq!(result.latency_test_interval, 10000);
    }

    #[test]
    fn validate_config_clamps_log_retention_days() {
        let mut config = Config::default();
        config.log_retention_days = 9999;
        let result = validate_config(config).unwrap();
        assert_eq!(result.log_retention_days, 365);
    }

    #[test]
    fn validate_config_migrates_old_hour_to_minutes() {
        let mut config = Config::default();
        config.config_version = 1;
        config.campus_check_start_minutes = 8; // 8 hours → 480 minutes
        let result = validate_config(config).unwrap();
        assert_eq!(result.campus_check_start_minutes, 480);
        assert_eq!(result.config_version, 2);
    }

    #[test]
    fn validate_config_preserves_v2_minutes() {
        let mut config = Config::default();
        config.config_version = 2;
        config.campus_check_start_minutes = 600;
        let result = validate_config(config).unwrap();
        assert_eq!(result.campus_check_start_minutes, 600);
    }

    #[test]
    fn validate_config_clamps_campus_check_start_minutes() {
        let mut config = Config::default();
        config.config_version = 2;
        config.campus_check_start_minutes = 2000;
        let result = validate_config(config).unwrap();
        assert_eq!(result.campus_check_start_minutes, 1439);
    }

    #[test]
    fn validate_config_clamps_campus_check_end_minutes() {
        let mut config = Config::default();
        config.config_version = 2;
        config.campus_check_end_minutes = 2000;
        let result = validate_config(config).unwrap();
        assert_eq!(result.campus_check_end_minutes, 1439);
    }

    #[test]
    fn validate_config_normalizes_empty_portal_url() {
        let mut config = Config::default();
        config.portal_url = String::new();
        let result = validate_config(config).unwrap();
        assert_eq!(result.portal_url, "http://10.1.99.100");
    }

    #[test]
    fn validate_config_normalizes_portal_url_with_port() {
        let mut config = Config::default();
        config.portal_url = "http://10.1.99.100:801".to_string();
        let result = validate_config(config).unwrap();
        assert_eq!(result.portal_url, "http://10.1.99.100");
    }

    #[test]
    fn validate_config_fills_empty_campus_gateway() {
        let mut config = Config::default();
        config.campus_gateway = String::new();
        let result = validate_config(config).unwrap();
        assert_eq!(result.campus_gateway, "10.2.127.254");
    }

    #[test]
    fn validate_config_skips_username_validation_when_empty() {
        let mut config = Config::default();
        config.user = String::new();
        assert!(validate_config(config).is_ok());
    }

    #[test]
    fn validate_config_skips_password_when_masked() {
        let mut config = Config::default();
        config.password = PASSWORD_MASK.to_string();
        assert!(validate_config(config).is_ok());
    }

    // ===== validate_config_lenient =====

    #[test]
    fn validate_config_lenient_preserves_valid_config() {
        let mut config = Config::default();
        config.user = "testuser".to_string();
        let result = validate_config_lenient(config);
        assert_eq!(result.user, "testuser");
    }

    #[test]
    fn validate_config_lenient_falls_back_bad_username() {
        let mut config = Config::default();
        config.user = "user@bad".to_string();
        let result = validate_config_lenient(config);
        assert_eq!(result.user, Config::default().user);
    }

    #[test]
    fn validate_config_lenient_falls_back_bad_operator() {
        let mut config = Config::default();
        config.operator = "@unknown".to_string();
        let result = validate_config_lenient(config);
        assert_eq!(result.operator, Config::default().operator);
    }

    #[test]
    fn validate_config_lenient_falls_back_bad_theme_mode() {
        let mut config = Config::default();
        config.theme_mode = "purple".to_string();
        let result = validate_config_lenient(config);
        assert_eq!(result.theme_mode, "dark");
    }

    #[test]
    fn validate_config_lenient_falls_back_bad_color() {
        let mut config = Config::default();
        config.custom_theme_color = "red".to_string();
        let result = validate_config_lenient(config);
        assert_eq!(result.custom_theme_color, "#6366f1");
    }

    #[test]
    fn validate_config_lenient_falls_back_bad_portal_url() {
        let mut config = Config::default();
        config.portal_url = "http://8.8.8.8".to_string();
        let result = validate_config_lenient(config);
        assert_eq!(result.portal_url, Config::default().portal_url);
    }

    #[test]
    fn validate_config_lenient_falls_back_bad_fixed_gateway() {
        let mut config = Config::default();
        config.fixed_gateway = "not-an-ip".to_string();
        let result = validate_config_lenient(config);
        assert_eq!(result.fixed_gateway, Config::default().fixed_gateway);
    }

    #[test]
    fn validate_config_lenient_falls_back_bad_campus_gateway() {
        let mut config = Config::default();
        config.campus_gateway = "not-an-ip".to_string();
        let result = validate_config_lenient(config);
        assert_eq!(result.campus_gateway, Config::default().campus_gateway);
    }

    #[test]
    fn validate_config_lenient_migrates_ctcc() {
        let mut config = Config::default();
        config.operator = "@ctcc".to_string();
        let result = validate_config_lenient(config);
        assert_eq!(result.operator, "@telecom");
    }

    #[test]
    fn validate_config_lenient_fills_empty_campus_gateway() {
        let mut config = Config::default();
        config.campus_gateway = String::new();
        let result = validate_config_lenient(config);
        assert_eq!(result.campus_gateway, "10.2.127.254");
    }

    #[test]
    fn validate_config_lenient_preserves_valid_password() {
        let mut config = Config::default();
        config.password = "mypassword".to_string();
        let result = validate_config_lenient(config);
        assert_eq!(result.password, "mypassword");
    }
}
