use crate::network::client::{PORTAL_URL, create_safe_http_client};

const LOGOUT_PLACEHOLDER_ACCOUNT: &str = "drcom";
const LOGOUT_PLACEHOLDER_PASSWORD: &str = "123";

pub fn random_v() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos() as u64;
    let v = 1000 + (seed % 9000);
    format!("{}", v)
}

fn do_login_request(user: &str, password: &str, operator: &str, adapter_ip: Option<&str>) -> Result<serde_json::Value, String> {
    let validated_user = crate::config::validate::validate_username(user).map_err(|e| e.to_string())?;
    let validated_operator = crate::config::validate::validate_operator(operator).map_err(|e| e.to_string())?;
    crate::config::validate::validate_password(password).map_err(|e| e.to_string())?;
    let user_account = format!("{}{}", validated_user, validated_operator);
    let portal_base = PORTAL_URL.load().clone();
    let base_url = if portal_base.contains(":801") {
        format!("{}/eportal/portal/login", portal_base.trim_end_matches('/'))
    } else {
        format!("{}:801/eportal/portal/login", portal_base.trim_end_matches('/'))
    };
    let callback = "dr1003";
    let query_params = format!(
        "callback={}&login_method=1&user_account={}&user_password={}&wlan_user_ip=&wlan_user_ipv6=&wlan_user_mac=000000000000&wlan_ac_ip=&wlan_ac_name=&jsVersion=4.1.3&terminal_type=1&lang=zh-cn&v={}&lang=zh",
        urlencoding::encode(callback),
        urlencoding::encode(&user_account),
        urlencoding::encode(password),
        random_v(),
    );
    let url = format!("{}?{}", base_url, query_params);
    let safe_url = format!("{}?***", base_url);

    crate::log_info!("login", "登录请求开始: user={}, operator={}, adapterIp={}",
        validated_user, validated_operator, adapter_ip.unwrap_or("default"));

    let local_addr = adapter_ip.and_then(|ip| ip.parse::<std::net::IpAddr>().ok());

    let client = create_safe_http_client(std::time::Duration::from_secs(15), local_addr)?;
    let t_req = std::time::Instant::now();
    let resp = client.get(&url).timeout(std::time::Duration::from_secs(15)).send()
        .map_err(|e| format!("登录请求失败: {}", e.to_string().replace(&url, &safe_url)))?;

    let status_code = resp.status();
    if resp.content_length().map_or(false, |len| len > 1024 * 1024) {
        return Err("登录响应体过大".to_string());
    }
    let body = resp.text().unwrap_or_default();
    let req_elapsed = t_req.elapsed();

    crate::log_info!("login", "登录请求完成({}ms): URL={}, status={:?}, bodyLen={}",
        req_elapsed.as_millis(), safe_url, status_code, body.len());

    parse_login_result(&body)
}

pub fn do_login_with_retry(user: &str, password: &str, operator: &str, adapter_ip: Option<&str>, max_retries: u32, is_quitting: &std::sync::atomic::AtomicBool) -> Result<serde_json::Value, String> {
    let mut last_result: Option<serde_json::Value> = None;

    for attempt in 1..=max_retries {
        if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(serde_json::json!({ "code": "error", "message": "应用正在退出", "success": false }));
        }

        crate::log_debug!("login", "登录尝试 [{}/{}]", attempt, max_retries);

        match do_login_request(user, password, operator, adapter_ip) {
            Ok(r) => {
                let success = r.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
                let retryable = r.get("retryable").and_then(|v| v.as_bool()).unwrap_or(true);
                if success || !retryable {
                    return Ok(r);
                }
                last_result = Some(r);
            }
            Err(e) => {
                last_result = Some(serde_json::json!({ "code": "error", "message": e, "success": false, "retryable": true }));
            }
        }

        if attempt < max_retries {
            for _ in 0..20 {
                if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
                    return Ok(serde_json::json!({ "code": "error", "message": "应用正在退出", "success": false }));
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }

    Ok(last_result.unwrap_or_else(|| serde_json::json!({ "code": "max_retries", "message": "多次重试后仍失败", "success": false })))
}

fn parse_login_result(response: &str) -> Result<serde_json::Value, String> {
    let json_data = if let Some(start) = response.find("dr1003(") {
        let inner_start = start + 7;
        if let Some(inner_end) = response[inner_start..].rfind(')').map(|i| inner_start + i) {
            response[inner_start..inner_end].to_string()
        } else {
            response.to_string()
        }
    } else {
        response.to_string()
    };

    match serde_json::from_str::<serde_json::Value>(&json_data) {
        Ok(data) => {
            let result = data.get("result").and_then(|v| v.as_i64()).unwrap_or(-1);
            let msg = data.get("msg").and_then(|v| v.as_str()).unwrap_or("");

            if result == 0 {
                if msg.contains("已经在线") {
                    Ok(serde_json::json!({ "code": "0", "message": msg, "success": true, "retryable": false }))
                } else if msg.contains("认证成功") {
                    Ok(serde_json::json!({ "code": "0", "message": "登录成功", "success": true, "retryable": false }))
                } else if msg.contains("AC认证失败") {
                    Ok(serde_json::json!({ "code": "ac_auth_failed", "message": format!("认证失败：{}", msg), "success": false, "retryable": false }))
                } else {
                    Ok(serde_json::json!({ "code": "0", "message": if msg.is_empty() { "操作完成" } else { msg }, "success": true, "retryable": false }))
                }
            } else if result == 1 {
                Ok(serde_json::json!({ "code": "0", "message": if msg.is_empty() { "Portal协议认证成功" } else { msg }, "success": true, "retryable": false }))
            } else if result == 2 {
                if msg.contains("已经在线") {
                    Ok(serde_json::json!({ "code": "2", "message": if msg.is_empty() { "已在线" } else { msg }, "success": true, "retryable": false }))
                } else {
                    Ok(serde_json::json!({ "code": "2", "message": if msg.is_empty() { "已在线（IP冲突或重复登录）" } else { msg }, "success": false, "retryable": false }))
                }
            } else if result == 3 {
                Ok(serde_json::json!({ "code": "3", "message": if msg.is_empty() { "流量超限" } else { msg }, "success": false, "retryable": false }))
            } else if result == 4 {
                Ok(serde_json::json!({ "code": "4", "message": if msg.is_empty() { "账号被禁用" } else { msg }, "success": false, "retryable": false }))
            } else {
                Ok(serde_json::json!({ "code": format!("{}", result), "message": if msg.is_empty() { format!("未知响应码: {}", result) } else { msg.to_string() }, "success": false, "retryable": true }))
            }
        }
        Err(_) => {
            let is_html = response.trim_start().starts_with("<!") || response.trim_start().starts_with("<html") || response.trim_start().starts_with("<HTML");
            if is_html {
                Ok(serde_json::json!({ "code": "parse_error", "message": "Portal返回非预期格式(HTML)，请稍后重试", "success": false, "retryable": false }))
            } else {
                Ok(serde_json::json!({ "code": "parse_error", "message": "无法解析登录响应", "success": false, "retryable": true }))
            }
        }
    }
}

fn do_logout_request(user: &str, adapter_ip: Option<&str>, _if_index: u32, _mac: &str) -> Result<serde_json::Value, String> {
    let validated_user = crate::config::validate::validate_username(user).map_err(|e| e.to_string())?;
    let portal_base = PORTAL_URL.load().clone();
    let portal_base_url = if portal_base.contains(":801") {
        portal_base.trim_end_matches('/').to_string()
    } else {
        format!("{}:801", portal_base.trim_end_matches('/'))
    };

    let wlan_user_ip = adapter_ip.unwrap_or("");
    let local_addr = adapter_ip.and_then(|ip| ip.parse::<std::net::IpAddr>().ok());
    let client = create_safe_http_client(std::time::Duration::from_secs(15), local_addr)?;

    let wlan_user_ip_int = adapter_ip.and_then(|ip| {
        let parts: Vec<u32> = ip.split('.').filter_map(|p| p.parse().ok()).collect();
        if parts.len() == 4 {
            Some((parts[0] << 24) | (parts[1] << 16) | (parts[2] << 8) | parts[3])
        } else {
            None
        }
    }).unwrap_or(0);

    let mut any_radius_ok = false;
    let mut any_unbind_ok = false;

    for round in 1..=2 {
        let unbind_cb = format!("dr100{}", round + 1);
        let logout_cb = format!("dr100{}", round + 2);

        crate::log_info!("logout", "第{}轮: MAC解绑: user={}", round, validated_user);

        let unbind_url = format!(
            "{}/eportal/portal/mac/unbind?callback={}&user_account={}&wlan_user_mac=000000000000&wlan_user_ip={}&jsVersion=4.1.3&v={}&lang=zh",
            portal_base_url,
            unbind_cb,
            urlencoding::encode(validated_user),
            wlan_user_ip_int,
            random_v(),
        );

        let t_unbind = std::time::Instant::now();
        let resp_unbind = client.get(&unbind_url).timeout(std::time::Duration::from_secs(15)).send()
            .map_err(|e| format!("第{}轮MAC解绑请求失败: {}", round, e))?;
        let body_unbind = resp_unbind.text().unwrap_or_default();
        crate::log_info!("logout", "第{}轮MAC解绑完成({}ms): body={}", round, t_unbind.elapsed().as_millis(), crate::auth::portal::safe_truncate(&body_unbind, 500));

        let unbind_result = parse_logout_result(&body_unbind)?;
        let unbind_ok = unbind_result.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
        if unbind_ok { any_unbind_ok = true; }

        crate::log_info!("logout", "第{}轮: Radius注销: adapterIp={}", round, wlan_user_ip);

        let logout_url = format!(
            "{}/eportal/portal/logout?callback={}&login_method=1&user_account={}&user_password={}&ac_logout=1&register_mode=1&wlan_user_ip={}&wlan_user_ipv6=&wlan_vlan_id=1&wlan_user_mac=000000000000&wlan_ac_ip=&wlan_ac_name=&jsVersion=4.1.3&v={}&lang=zh",
            portal_base_url,
            logout_cb,
            LOGOUT_PLACEHOLDER_ACCOUNT,
            LOGOUT_PLACEHOLDER_PASSWORD,
            urlencoding::encode(wlan_user_ip),
            random_v(),
        );

        let t_logout = std::time::Instant::now();
        let resp_logout = client.get(&logout_url).timeout(std::time::Duration::from_secs(15)).send()
            .map_err(|e| format!("第{}轮Radius注销请求失败: {}", round, e))?;
        let body_logout = resp_logout.text().unwrap_or_default();
        crate::log_info!("logout", "第{}轮Radius注销完成({}ms): body={}", round, t_logout.elapsed().as_millis(), crate::auth::portal::safe_truncate(&body_logout, 500));

        let logout_result = parse_logout_result(&body_logout)?;
        let radius_ok = logout_result.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
        if radius_ok { any_radius_ok = true; }

        if round == 1 {
            std::thread::sleep(std::time::Duration::from_millis(1500));
        }
    }

    let combined_msg = match (any_radius_ok, any_unbind_ok) {
        (true, true) => "注销成功",
        (true, false) => "Radius注销成功，MAC解绑失败",
        (false, true) => "Radius注销失败，MAC解绑成功",
        (false, false) => "注销失败",
    };

    Ok(serde_json::json!({
        "code": if any_radius_ok { "0" } else { "1" },
        "message": combined_msg,
        "success": any_radius_ok,
        "retryable": !any_radius_ok,
    }))
}

pub fn do_logout_with_retry(user: &str, adapter_ip: Option<&str>, if_index: u32, mac: &str, max_retries: u32, is_quitting: &std::sync::atomic::AtomicBool) -> Result<serde_json::Value, String> {
    let mut last_result = None;

    for attempt in 1..=max_retries {
        if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(serde_json::json!({ "code": "error", "message": "应用正在退出", "success": false }));
        }

        let result = do_logout_request(user, adapter_ip, if_index, mac);
        match result {
            Ok(ref r) if r.get("success").and_then(|v| v.as_bool()).unwrap_or(false) => {
                return Ok(r.clone());
            }
            Ok(r) => {
                let retryable = r.get("retryable").and_then(|v| v.as_bool()).unwrap_or(true);
                if !retryable {
                    return Ok(r);
                }
                last_result = Some(r);
            }
            Err(e) => {
                last_result = Some(serde_json::json!({ "code": "error", "message": e, "success": false }));
            }
        }

        if attempt < max_retries {
            for _ in 0..20 {
                if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
                    return Ok(serde_json::json!({ "code": "error", "message": "应用正在退出", "success": false }));
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }

    let last = last_result.unwrap_or_else(|| serde_json::json!({ "code": "max_retries", "message": "多次重试后仍失败", "success": false }));
    Ok(last)
}

fn parse_logout_result(response: &str) -> Result<serde_json::Value, String> {
    crate::log_info!("logout", "parse_logout_result原始响应: {}", crate::auth::portal::safe_truncate(response, 1000));
    let json_data = if let Some(start) = response.find('(') {
        let inner_start = start + 1;
        if let Some(inner_end) = response[inner_start..].rfind(')').map(|i| inner_start + i) {
            response[inner_start..inner_end].to_string()
        } else {
            response.to_string()
        }
    } else {
        response.to_string()
    };

    match serde_json::from_str::<serde_json::Value>(&json_data) {
        Ok(data) => {
            let result = data.get("result").and_then(|v| v.as_i64()).unwrap_or(-1);
            let msg = data.get("msg").and_then(|v| v.as_str()).unwrap_or("");

            if result == 0 {
                if msg.contains("解绑终端MAC成功") {
                    Ok(serde_json::json!({ "code": "0", "message": "注销成功", "success": true, "retryable": false }))
                } else if msg.contains("获取用户在线信息数据为空") {
                    Ok(serde_json::json!({ "code": "0", "message": "当前无在线设备", "success": true, "retryable": false }))
                } else if msg.contains("非法") || msg.contains("失败") || msg.contains("错误") || msg.contains("拒绝") {
                    Ok(serde_json::json!({ "code": "0", "message": msg, "success": false, "retryable": false }))
                } else {
                    Ok(serde_json::json!({ "code": "0", "message": if msg.is_empty() { "操作完成" } else { msg }, "success": true, "retryable": false }))
                }
            } else if result == 1 {
                Ok(serde_json::json!({ "code": "1", "message": if msg.is_empty() { "注销成功" } else { msg }, "success": true, "retryable": false }))
            } else {
                Ok(serde_json::json!({ "code": format!("{}", result), "message": if msg.is_empty() { format!("注销失败，响应码: {}", result) } else { msg.to_string() }, "success": false, "retryable": true }))
            }
        }
        Err(_) => {
            crate::log_warn!("logout", "JSON解析失败, json_data={}", crate::auth::portal::safe_truncate(&json_data, 500));
            let is_html = response.trim_start().starts_with("<!") || response.trim_start().starts_with("<html") || response.trim_start().starts_with("<HTML");
            if is_html {
                if response.contains("注销成功") || response.contains("下线成功") || response.contains("已下线") || response.contains("logout") {
                    Ok(serde_json::json!({ "code": "0", "message": "注销成功", "success": true, "retryable": false }))
                } else {
                    Ok(serde_json::json!({ "code": "parse_error", "message": "Portal返回非预期格式(HTML)，请稍后重试", "success": false, "retryable": false }))
                }
            } else if response.contains("注销成功") || response.contains("下线成功") || response.contains("已下线") || response.contains("解绑成功") {
                Ok(serde_json::json!({ "code": "0", "message": "注销成功", "success": true, "retryable": false }))
            } else {
                Ok(serde_json::json!({ "code": "parse_error", "message": format!("无法解析注销响应: {}", crate::auth::portal::safe_truncate(response, 200)), "success": false, "retryable": true }))
            }
        }
    }
}
