use crate::network::client::{PORTAL_URL, create_safe_http_client};

const LOGOUT_PLACEHOLDER_ACCOUNT: &str = "drcom";
const LOGOUT_PLACEHOLDER_PASSWORD: &str = "123";

pub fn random_v() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos() as u64;
    let v = 1000 + (seed % 9000);
    format!("{v}")
}

/// 读取 HTTP 响应体并限制 1MB 上限（无 Content-Length 的 chunked/流式响应也受限）。
/// 读取失败返回空串（调用方按"无可解析内容"处理），超限同样返回空串并告警。
/// 按 Content-Type charset 解码（GBK Portal 的中文成败关键词依赖正确解码）。
fn read_bounded_body(resp: reqwest::Response, label: &str) -> String {
    const MAX_BODY: u64 = 1024 * 1024;
    if resp.content_length().map(|len| len > MAX_BODY).unwrap_or(false) {
        crate::log_warn!("logout", "{label}响应体过大(Content-Length={:?})，忽略", resp.content_length());
        return String::new();
    }
    let charset = content_type_charset(&resp);
    match crate::infra::async_util::block_on_sync(resp.bytes()) {
        Ok(b) => {
            if b.len() as u64 > MAX_BODY {
                crate::log_warn!("logout", "{label}响应体超限({}B)，忽略", b.len());
                String::new()
            } else {
                crate::platform::console_output::decode_charset_bytes(&b, charset.as_deref())
            }
        }
        Err(e) => {
            crate::log_warn!("logout", "{label}响应体读取失败: {}", e);
            String::new()
        }
    }
}

/// 提取 Content-Type 中的 charset 参数（UTF-8 响应无该参数时返回 None）
fn content_type_charset(resp: &reqwest::Response) -> Option<String> {
    resp.headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|ct| ct.split(';').find_map(|p| p.trim().strip_prefix("charset=").map(|s| s.to_string())))
}

/// 从 JSONP 响应中按花括号平衡（字符串/转义感知）截取第一个完整 JSON 对象。
/// 旧实现 `rfind(')')` 取响应最后一个右括号，msg 含半角 `)`（如"密码错误(剩余2次)"）
/// 时 JSON 被截断导致解析失败、登录误报；纯 JSON（非 JSONP）响应回退原文。
fn jsonp_json_slice(response: &str) -> &str {
    let extracted = (|| {
        let open = response.find('(')?;
        let obj_start = open + response[open..].find('{')?;
        let bytes = response.as_bytes();
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escaped = false;
        for (i, &b) in bytes.iter().enumerate().skip(obj_start) {
            if in_string {
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'"' {
                    in_string = false;
                }
                continue;
            }
            match b {
                b'"' => in_string = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(&response[obj_start..=i]);
                    }
                }
                _ => {}
            }
        }
        None
    })();
    extracted.unwrap_or(response)
}

/// 可中断等待：在指定时长内每 100ms 检查退出标志，返回 true 表示未取消，false 表示已取消
fn wait_cancellable(duration_ms: u64, is_quitting: &std::sync::atomic::AtomicBool) -> bool {
    let steps = duration_ms / 100;
    for _ in 0..steps {
        if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    true
}

fn do_login_request(user: &str, password: &str, operator: &str, adapter_ip: Option<&str>) -> Result<serde_json::Value, String> {
    let validated_user = crate::config::validate::validate_username(user).map_err(|e| e.to_string())?;
    let validated_operator = crate::config::validate::validate_operator(operator).map_err(|e| e.to_string())?;
    crate::config::validate::validate_password(password).map_err(|e| e.to_string())?;
    let user_account = format!("{validated_user}{validated_operator}");
    let portal_base = PORTAL_URL.load().clone();
    let base_url = format!("{}/eportal/portal/login", crate::auth::portal::ensure_portal_port(&portal_base));
    let callback = "dr1003";
    let query_params = format!(
        "callback={}&login_method=1&user_account={}&user_password={}&wlan_user_ip=&wlan_user_ipv6=&wlan_user_mac=000000000000&wlan_ac_ip=&wlan_ac_name=&jsVersion=4.1.3&terminal_type=1&lang=zh-cn&v={}&lang=zh",
        urlencoding::encode(callback),
        urlencoding::encode(&user_account),
        urlencoding::encode(password),
        random_v(),
    );
    let url = format!("{base_url}?{query_params}");
    let safe_url = format!("{base_url}?***");

    crate::log_info!("login", "登录请求开始: user={}, operator={}, adapterIp={}",
        validated_user, validated_operator, adapter_ip.unwrap_or("default"));

    let local_addr = adapter_ip.and_then(|ip| ip.parse::<std::net::IpAddr>().ok());

    let client = create_safe_http_client(std::time::Duration::from_secs(15), local_addr)?;
    let t_req = std::time::Instant::now();
    // 历史缺陷：错误串脱敏仅替换字面密码（URL 编码后的 %xx 无法匹配），
    // reqwest 错误一旦包含完整 URL 即泄漏编码后的凭据。错误信息中的完整 URL
    // 统一替换为 base_url?***，密码（明文与 URL 编码形式）替换为 ***。
    let resp = crate::infra::async_util::block_on_sync(
        client.get(&url).timeout(std::time::Duration::from_secs(15)).send()
    ).map_err(|e| {
        let msg = crate::auth::portal::redact_credentials(e.to_string(), &url, &base_url, password);
        format!("登录请求失败: {}", crate::auth::portal::safe_truncate(&msg, 200))
    })?;

    let status_code = resp.status();
    // 历史缺陷：content_length() 缺失（chunked/流式响应）时不设上限，恶意/异常
    // Portal 可耗尽内存。改为无论是否带 Content-Length 都硬限制 1MB。
    const MAX_BODY: u64 = 1024 * 1024;
    if resp.content_length().map(|len| len > MAX_BODY).unwrap_or(false) {
        return Err("登录响应体过大".to_string());
    }
    let charset = content_type_charset(&resp);
    let body_bytes = crate::infra::async_util::block_on_sync(resp.bytes())
        .map_err(|e| format!("读取登录响应失败: {e}"))?;
    if body_bytes.len() as u64 > MAX_BODY {
        return Err("登录响应体过大".to_string());
    }
    let body = crate::platform::console_output::decode_charset_bytes(&body_bytes, charset.as_deref());
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
                // 历史缺陷：请求失败只进 last_result，日志完全静默，
                // 排障时只见连续"登录请求开始"而无任何失败原因
                crate::log_warn!("login", "登录请求失败 [{}/{}]: {}", attempt, max_retries, e);
                last_result = Some(serde_json::json!({ "code": "error", "message": e, "success": false, "retryable": true }));
            }
        }

        if attempt < max_retries && !wait_cancellable(2000, is_quitting) {
            return Ok(serde_json::json!({ "code": "error", "message": "应用正在退出", "success": false }));
        }
    }

    Ok(last_result.unwrap_or_else(|| serde_json::json!({ "code": "max_retries", "message": "多次重试后仍失败", "success": false })))
}

fn parse_login_result(response: &str) -> Result<serde_json::Value, String> {
    // BE-A-07: 直接对响应切片解析，避免 to_string() 克隆整段响应体
    let json_data: &str = jsonp_json_slice(response);

    match serde_json::from_str::<serde_json::Value>(json_data) {
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
                    // 未知 msg 视为失败，避免把“账号过期”“余额不足”等真实业务失败误报为登录成功
                    Ok(serde_json::json!({ "code": "unknown_failure", "message": if msg.is_empty() { "未识别的登录响应" } else { msg }, "success": false, "retryable": false }))
                }
            } else if result == 1 {
                if msg.contains("非法") || msg.contains("失败") || msg.contains("错误") || msg.contains("拒绝") {
                    Ok(serde_json::json!({ "code": "1", "message": msg, "success": false, "retryable": false }))
                } else {
                    Ok(serde_json::json!({ "code": "0", "message": if msg.is_empty() { "Portal协议认证成功" } else { msg }, "success": true, "retryable": false }))
                }
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
                Ok(serde_json::json!({ "code": format!("{}", result), "message": if msg.is_empty() { format!("未知响应码: {result}") } else { msg.to_string() }, "success": false, "retryable": true }))
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

fn do_logout_request(user: &str, adapter_ip: Option<&str>, is_quitting: &std::sync::atomic::AtomicBool) -> Result<serde_json::Value, String> {
    let validated_user = crate::config::validate::validate_username(user).map_err(|e| e.to_string())?;
    let portal_base = PORTAL_URL.load().clone();
    let portal_base_url = crate::auth::portal::ensure_portal_port(&portal_base);

    // NAT 内网 IP 检测：NAT 环境下不发送 wlan_user_ip（与 portal.rs 行为一致）
    let adapter_ip_str = adapter_ip.unwrap_or("");
    let nat_ip = crate::auth::portal::is_nat_private_ip(adapter_ip_str);
    let wlan_user_ip = if nat_ip { "" } else { adapter_ip_str };
    if nat_ip {
        crate::log_info!("logout", "检测到NAT内网IP({}), 不发送wlan_user_ip", adapter_ip_str);
    }
    let local_addr = adapter_ip.and_then(|ip| ip.parse::<std::net::IpAddr>().ok());
    let client = create_safe_http_client(std::time::Duration::from_secs(15), local_addr)?;

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
            urlencoding::encode(wlan_user_ip),
            random_v(),
        );

        let t_unbind = std::time::Instant::now();
        // MAC 解绑为 best-effort：网络失败/端点不可用时记录并继续，
        // 不得中断更关键的 Radius 注销（历史缺陷：unbind 的 ? 直接 abort 整个注销流程，
        // 解绑端点不可用时注销永远失败，且跳过第 2 轮重试）
        match crate::infra::async_util::block_on_sync(
            client.get(&unbind_url).timeout(std::time::Duration::from_secs(15)).send()
        ) {
            Ok(resp) => {
                let body_unbind = read_bounded_body(resp, "MAC解绑");
                crate::log_info!("logout", "第{}轮MAC解绑完成({}ms): body={}", round, t_unbind.elapsed().as_millis(), crate::auth::portal::safe_truncate(&body_unbind, 500));
                let unbind_result = parse_logout_result(&body_unbind)?;
                if unbind_result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                    any_unbind_ok = true;
                }
            }
            Err(e) => {
                crate::log_warn!("logout", "第{}轮MAC解绑请求失败(降级继续): {}", round, e);
            }
        };

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
        // Radius 注销发送失败同样降级：记录并进入下一轮，避免单次网络抖动跳过重试
        match crate::infra::async_util::block_on_sync(
            client.get(&logout_url).timeout(std::time::Duration::from_secs(15)).send()
        ) {
            Ok(resp) => {
                let body_logout = read_bounded_body(resp, "Radius注销");
                crate::log_info!("logout", "第{}轮Radius注销完成({}ms): body={}", round, t_logout.elapsed().as_millis(), crate::auth::portal::safe_truncate(&body_logout, 500));
                let logout_result = parse_logout_result(&body_logout)?;
                if logout_result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                    any_radius_ok = true;
                }
            }
            Err(e) => {
                crate::log_warn!("logout", "第{}轮Radius注销请求失败(降级继续): {}", round, e);
            }
        };

        if round == 1 {
            // 第1轮已全部成功则跳过第2轮，避免多发无谓请求
            if any_radius_ok && any_unbind_ok {
                break;
            }
            // 可中断等待 1.5s（15×100ms，每次检查退出标志）
            for _ in 0..15 {
                if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            // 退出标志被设置时跳过 round 2，避免阻塞应用退出
            if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
                break;
            }
        }
    }

    let combined_msg = merge_logout_results(any_radius_ok, any_unbind_ok);

    Ok(serde_json::json!({
        "code": if any_radius_ok { "0" } else { "1" },
        "message": combined_msg,
        "success": any_radius_ok,
        "retryable": !any_radius_ok,
    }))
}

/// 合并 Radius 注销与 MAC 解绑的结果消息。
///
/// Radius 注销是主操作（决定 success/retryable），MAC 解绑为 best-effort
/// 辅助步骤：解绑失败不应使整体注销失败，但要在消息中如实反映。
pub fn merge_logout_results(any_radius_ok: bool, any_unbind_ok: bool) -> &'static str {
    match (any_radius_ok, any_unbind_ok) {
        (true, true) => "注销成功",
        (true, false) => "Radius注销成功，MAC解绑失败",
        (false, true) => "Radius注销失败，MAC解绑成功",
        (false, false) => "注销失败",
    }
}

pub fn do_logout_with_retry(user: &str, adapter_ip: Option<&str>, _if_index: u32, _mac: &str, max_retries: u32, is_quitting: &std::sync::atomic::AtomicBool) -> Result<serde_json::Value, String> {
    let mut last_result = None;

    for attempt in 1..=max_retries {
        if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(serde_json::json!({ "code": "error", "message": "应用正在退出", "success": false }));
        }

        let result = do_logout_request(user, adapter_ip, is_quitting);
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

        if attempt < max_retries && !wait_cancellable(2000, is_quitting) {
            return Ok(serde_json::json!({ "code": "error", "message": "应用正在退出", "success": false }));
        }
    }

    let last = last_result.unwrap_or_else(|| serde_json::json!({ "code": "max_retries", "message": "多次重试后仍失败", "success": false }));
    Ok(last)
}

fn parse_logout_result(response: &str) -> Result<serde_json::Value, String> {
    crate::log_info!("logout", "parse_logout_result原始响应: {}", crate::auth::portal::safe_truncate(response, 1000));
    // BE-A-07: 直接对响应切片解析，避免 to_string() 克隆整段响应体
    let json_data: &str = jsonp_json_slice(response);

    match serde_json::from_str::<serde_json::Value>(json_data) {
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
                // result=1 表示 Radius 注销成功，但仍需排除错误关键词（与 result=0 一致）
                if msg.contains("非法") || msg.contains("失败") || msg.contains("错误") || msg.contains("拒绝") {
                    Ok(serde_json::json!({ "code": "1", "message": msg, "success": false, "retryable": false }))
                } else {
                    Ok(serde_json::json!({ "code": "1", "message": if msg.is_empty() { "注销成功" } else { msg }, "success": true, "retryable": false }))
                }
            } else {
                Ok(serde_json::json!({ "code": format!("{}", result), "message": if msg.is_empty() { format!("注销失败，响应码: {result}") } else { msg.to_string() }, "success": false, "retryable": true }))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_login_result_success() {
        let response = r#"dr1003({"result":0,"msg":"认证成功"})"#;
        let result = parse_login_result(response).unwrap();
        assert!(result["success"].as_bool().unwrap());
        assert_eq!(result["code"], "0");
        assert_eq!(result["message"], "登录成功");
    }

    // ===== jsonp_json_slice：msg 含半角括号/花括号/转义时不再截断 JSON =====

    #[test]
    fn jsonp_msg_with_ascii_paren_not_truncated() {
        let response = r#"dr1003({"result":1,"msg":"密码错误(剩余2次)","error_info":"E2601"})"#;
        let result = parse_login_result(response).unwrap();
        assert_eq!(result["code"], "1");
        assert!(result["message"].as_str().unwrap().contains("(剩余2次)"));
    }

    #[test]
    fn jsonp_msg_with_braces_and_escapes() {
        // 验证字符串内的花括号/转义引号/半角括号不会截断 JSON：msg 完整保留
        let response = r#"dr1003({"result":1,"msg":"格式 {a} 与 \"引号\" 混入)","ok":1})"#;
        let result = parse_login_result(response).unwrap();
        assert!(result["message"].as_str().unwrap().contains("混入)"));
    }

    #[test]
    fn plain_json_response_still_parses() {
        let result = parse_login_result(r#"{"result":0,"msg":"认证成功"}"#).unwrap();
        assert!(result["success"].as_bool().unwrap());
    }

    #[test]
    fn parse_login_result_already_online() {
        let response = r#"dr1003({"result":0,"msg":"用户已经在线"})"#;
        let result = parse_login_result(response).unwrap();
        assert!(result["success"].as_bool().unwrap());
        assert_eq!(result["code"], "0");
    }

    #[test]
    fn parse_login_result_auth_failed() {
        let response = r#"dr1003({"result":0,"msg":"AC认证失败"})"#;
        let result = parse_login_result(response).unwrap();
        assert!(!result["success"].as_bool().unwrap());
        assert_eq!(result["code"], "ac_auth_failed");
    }

    #[test]
    fn parse_login_result_illegal() {
        let response = r#"dr1003({"result":1,"msg":"非法用户"})"#;
        let result = parse_login_result(response).unwrap();
        assert!(!result["success"].as_bool().unwrap());
        assert_eq!(result["code"], "1");
    }

    #[test]
    fn parse_login_result_disabled() {
        let response = r#"dr1003({"result":4,"msg":"账号被禁用"})"#;
        let result = parse_login_result(response).unwrap();
        assert!(!result["success"].as_bool().unwrap());
        assert_eq!(result["code"], "4");
    }

    #[test]
    fn parse_login_result_html_parse_error() {
        let response = "<!DOCTYPE html><html>error</html>";
        let result = parse_login_result(response).unwrap();
        assert!(!result["success"].as_bool().unwrap());
        assert_eq!(result["code"], "parse_error");
    }

    #[test]
    fn parse_logout_result_success() {
        let response = r#"dr1002({"result":0,"msg":"解绑终端MAC成功"})"#;
        let result = parse_logout_result(response).unwrap();
        assert!(result["success"].as_bool().unwrap());
        assert_eq!(result["code"], "0");
    }

    #[test]
    fn parse_logout_result_no_online_device() {
        let response = r#"dr1002({"result":0,"msg":"获取用户在线信息数据为空"})"#;
        let result = parse_logout_result(response).unwrap();
        assert!(result["success"].as_bool().unwrap());
        assert_eq!(result["message"], "当前无在线设备");
    }

    #[test]
    fn parse_logout_result_radius_success() {
        let response = r#"dr1003({"result":1,"msg":"下线成功"})"#;
        let result = parse_logout_result(response).unwrap();
        assert!(result["success"].as_bool().unwrap());
        assert_eq!(result["code"], "1");
    }

    #[test]
    fn parse_logout_result_html_success() {
        let response = "<html>注销成功</html>";
        let result = parse_logout_result(response).unwrap();
        assert!(result["success"].as_bool().unwrap());
        assert_eq!(result["message"], "注销成功");
    }

    #[test]
    fn do_login_with_retry_respects_quit_flag() {
        let quitting = std::sync::atomic::AtomicBool::new(true);
        let result = do_login_with_retry("user", "pass", "", Some("10.0.0.1"), 3, &quitting).unwrap();
        assert!(!result["success"].as_bool().unwrap());
        assert_eq!(result["message"], "应用正在退出");
    }

    #[test]
    fn do_logout_with_retry_respects_quit_flag() {
        let quitting = std::sync::atomic::AtomicBool::new(true);
        let result = do_logout_with_retry("user", Some("10.0.0.1"), 0, "00:00:00:00:00:00", 3, &quitting).unwrap();
        assert!(!result["success"].as_bool().unwrap());
        assert_eq!(result["message"], "应用正在退出");
    }

    #[test]
    fn merge_radius_success_unbind_failure_reports_success() {
        // 关键约束：MAC 解绑失败不得使整体注销失败（Radius 是主操作）
        assert_eq!(merge_logout_results(true, false), "Radius注销成功，MAC解绑失败");
    }

    #[test]
    fn merge_both_success_is_full_success() {
        assert_eq!(merge_logout_results(true, true), "注销成功");
    }

    #[test]
    fn merge_both_failure_is_failure() {
        assert_eq!(merge_logout_results(false, false), "注销失败");
        assert_eq!(merge_logout_results(false, true), "Radius注销失败，MAC解绑成功");
    }
}
