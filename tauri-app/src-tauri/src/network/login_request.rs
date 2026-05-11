use std::io::Read;
use regex::Regex;
use lazy_static::lazy_static;

use super::cache::{NET_CACHE, MAX_RESPONSE_SIZE, create_safe_http_client};

lazy_static! {
    static ref DR1003_RE: Regex = Regex::new(r"dr1003\((.+)\)").unwrap();
}

fn do_login_request(user: &str, password: &str, operator: &str, adapter_ip: Option<&str>) -> Result<serde_json::Value, String> {
    let validated_user = crate::config::validate_username(user).map_err(|e| e.to_string())?;
    let validated_operator = crate::config::validate_operator(operator);
    crate::config::validate_password(password).map_err(|e| e.to_string())?;
    let user_account = format!("{}{}", validated_user, validated_operator);
    let portal_base = NET_CACHE.portal_url.load().clone();
    let base_url = format!("{}/eportal/portal/login", portal_base.trim_end_matches('/'));
    let callback = "dr1003";
    let v = if validated_operator.is_empty() { "3043" } else { "2098" };
    let query_params = format!(
        "callback={}&login_method=1&user_account={}&user_password={}&wlan_user_ip&wlan_user_ipv6=&wlan_user_mac=000000000000&wlan_ac_ip=&wlan_ac_name=&jsVersion=4.1.3&terminal_type=1&lang=zh-cn&v={}&lang=zh",
        urlencoding::encode(callback),
        urlencoding::encode(&user_account),
        urlencoding::encode(password),
        v,
    );
    let url = format!("{}?{}", base_url, query_params);
    let safe_url = format!("{}?***", base_url);

    let local_addr = adapter_ip.and_then(|ip| ip.parse::<std::net::IpAddr>().ok());

    let client = create_safe_http_client(std::time::Duration::from_secs(15), local_addr)?;
    let mut resp = client.get(&url).timeout(std::time::Duration::from_secs(15)).send()
        .map_err(|e| format!("登录请求失败: {}", e.to_string().replace(&url, &safe_url)))?;

    let mut body = String::new();
    let mut limited = (&mut resp).take(MAX_RESPONSE_SIZE as u64);
    let _ = limited.read_to_string(&mut body);
    let _ = std::io::copy(&mut resp, &mut std::io::sink());

    crate::log_debug!("login", "登录请求完成, URL: {}", safe_url);

    parse_login_result(&body)
}

pub fn do_login_with_retry(user: &str, password: &str, operator: &str, adapter_ip: Option<&str>, max_retries: u32, is_quitting: &std::sync::atomic::AtomicBool) -> Result<serde_json::Value, String> {
    let mut last_result = None;

    for attempt in 1..=max_retries {
        if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(serde_json::json!({ "code": "error", "message": "应用正在退出", "success": false }));
        }

        let result = do_login_request(user, password, operator, adapter_ip);
        match result {
            Ok(ref r) if r.get("success").and_then(|v| v.as_bool()).unwrap_or(false) => {
                return Ok(r.clone());
            }
            Ok(r) => {
                last_result = Some(r);
            }
            Err(e) => {
                last_result = Some(serde_json::json!({ "code": "error", "message": e, "success": false }));
            }
        }

        if attempt < max_retries {
            let base_ms = 500u64 * 2u64.pow(attempt - 1);
            let jitter = (rand::random::<f64>() * 300.0) as u64;
            std::thread::sleep(std::time::Duration::from_millis(base_ms + jitter));
        }
    }

    let last = last_result.unwrap_or_else(|| serde_json::json!({ "code": "max_retries", "message": "多次重试后仍失败", "success": false }));
    Ok(last)
}

fn parse_login_result(response: &str) -> Result<serde_json::Value, String> {
    let json_data = if let Some(captures) = DR1003_RE
        .captures(response)
        .and_then(|caps| caps.get(1))
    {
        captures.as_str().to_string()
    } else {
        response.to_string()
    };

    match serde_json::from_str::<serde_json::Value>(&json_data) {
        Ok(data) => {
            let result = data.get("result").and_then(|v| v.as_i64()).unwrap_or(-1);
            let msg = data.get("msg").and_then(|v| v.as_str()).unwrap_or("");

            if result == 0 {
                if msg.contains("已经在线") {
                    Ok(serde_json::json!({ "code": "0", "message": msg, "success": true }))
                } else if msg.contains("认证成功") {
                    Ok(serde_json::json!({ "code": "0", "message": "登录成功", "success": true }))
                } else if msg.contains("AC认证失败") {
                    Ok(serde_json::json!({ "code": "ac_auth_failed", "message": format!("认证失败：{}", msg), "success": false }))
                } else {
                    Ok(serde_json::json!({ "code": "0", "message": if msg.is_empty() { "登录成功" } else { msg }, "success": true }))
                }
            } else if result == 1 {
                if msg.contains("认证成功") {
                    Ok(serde_json::json!({ "code": "0", "message": "登录成功", "success": true }))
                } else {
                    Ok(serde_json::json!({ "code": "1", "message": if msg.is_empty() { "认证失败" } else { msg }, "success": false }))
                }
            } else if result == 2 {
                Ok(serde_json::json!({ "code": "2", "message": if msg.is_empty() { "已在线（IP冲突或重复登录）" } else { msg }, "success": false }))
            } else if result == 3 {
                Ok(serde_json::json!({ "code": "3", "message": if msg.is_empty() { "流量超限" } else { msg }, "success": false }))
            } else if result == 4 {
                Ok(serde_json::json!({ "code": "4", "message": if msg.is_empty() { "账号被禁用" } else { msg }, "success": false }))
            } else {
                Ok(serde_json::json!({ "code": format!("{}", result), "message": if msg.is_empty() { format!("未知响应码: {}", result) } else { msg.to_string() }, "success": false }))
            }
        }
        Err(_) => {
            Ok(serde_json::json!({ "code": "parse_error", "message": "无法解析登录响应", "success": false }))
        }
    }
}
