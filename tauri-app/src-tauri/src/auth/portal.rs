use crate::network::client::{PORTAL_URL, create_safe_http_client};

/// Portal 检测相关配置常量
mod portal_config {
    use std::time::Duration;

    /// HTTP 客户端创建超时
    pub const CLIENT_TIMEOUT: Duration = Duration::from_secs(8);
    /// 单次 HTTP 请求超时
    pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
    /// 响应体最大允许大小（1MB）
    pub const MAX_RESPONSE_SIZE: u64 = 1024 * 1024;
    /// JSONP 回调前缀
    pub const JSONP_CALLBACK_PREFIX: &str = "dr1003(";

    // 页面特征字符串
    pub const PAGE_INDICATOR_LOGOUT: &str = "Dr.COMWebLoginID_1";
    pub const PAGE_INDICATOR_LOGIN_0: &str = "Dr.COMWebLoginID_0";
    pub const PAGE_INDICATOR_LOGIN_2: &str = "Dr.COMWebLoginID_2";
    pub const PAGE_TITLE_LOGOUT: &str = "<title>注销页</title>";
    pub const PAGE_TITLE_LOGIN: &str = "<title>登录页</title>";
}

/// 在同步上下文中运行 async HTTP future。
///
/// 优先使用当前 Tokio runtime handle（Handle::block_on 会设置 reactor guard，
/// 使 reqwest future 能正常注册 IO 事件）。
/// 如果当前线程无 runtime 上下文（如测试线程），fallback 到 tauri 全局 runtime。
///
/// 背景：b4d8e82 将 reqwest::blocking 迁移到异步 reqwest + block_on，但
/// std::thread::scope 子线程无 Tokio reactor 上下文，导致 panic
/// "there is no reactor running"。此函数 + watcher.rs 的 Handle::enter() 修复该问题。
///
/// 注意：不能在 async worker 线程上直接调用（Handle::block_on 会 panic）。
/// 所有调用者必须通过 spawn_blocking 或在同步线程中调用。
fn block_on_http<F: std::future::Future>(future: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.block_on(future),
        Err(_) => tauri::async_runtime::block_on(future),
    }
}

/// 确保 Portal 地址包含 :801 端口，正确处理带路径的 URL
/// 原 format!("{}:801", base) 在 base 含路径时会把端口拼到路径后产生非法 URL
pub fn ensure_portal_port(base: &str) -> String {
    let trimmed = base.trim_end_matches('/');
    match url::Url::parse(trimmed) {
        Ok(mut u) => {
            if u.port().is_none() {
                let _ = u.set_port(Some(801));
            }
            u.as_str().trim_end_matches('/').to_string()
        }
        Err(_) => format!("{trimmed}:801"),
    }
}

pub fn safe_truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        return s;
    }
    let boundary = s.char_indices()
        .take_while(|(i, _)| *i < max_len)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    &s[..boundary]
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalStatus {
    pub reachable: bool,
    pub login_available: bool,
    pub online: bool,
    pub message: String,
    pub data_length: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
}

pub fn check_portal_full(adapter_ip: &str, adapter_name: Option<&str>, user_account: Option<&str>, user_password: Option<&str>, _operator: Option<&str>) -> Result<PortalStatus, String> {
    let t0 = std::time::Instant::now();
    let portal_url = PORTAL_URL.load().clone();
    let local_addr = parse_adapter_ip(adapter_ip);

    let client = create_safe_http_client(portal_config::CLIENT_TIMEOUT, local_addr)?;
    let portal_base = portal_url.trim_end_matches('/');
    let account = user_account.unwrap_or("");
    let password = user_password.unwrap_or("");

    log_portal_query_start(adapter_name, adapter_ip);
    let page_result = check_portal_page(&client, portal_base);

    match page_result {
        PageCheckResult::Determined(online) => {
            let status = build_determined_status(online);
            log_portal_page_result(t0.elapsed(), adapter_name, adapter_ip, &status);
            Ok(status)
        }
        PageCheckResult::Unknown => {
            handle_unknown_page_status(&client, portal_base, account, password, adapter_ip, adapter_name, t0)
        }
        PageCheckResult::Failed => {
            log_portal_page_failed(adapter_name, adapter_ip);
            Ok(build_request_failed_status())
        }
    }
}

/// 处理页面检测无法判断登录状态的情况，尝试 API 备用检测
fn handle_unknown_page_status(
    client: &reqwest::Client,
    portal_base: &str,
    account: &str,
    password: &str,
    adapter_ip: &str,
    adapter_name: Option<&str>,
    t0: std::time::Instant,
) -> Result<PortalStatus, String> {
    if account.is_empty() {
        log_portal_no_credentials(adapter_name, adapter_ip);
        return Ok(build_unknown_status());
    }

    log_portal_api_fallback(adapter_name, adapter_ip);

    let nat_ip = is_nat_private_ip(adapter_ip);
    if nat_ip {
        crate::log_info!("network", "检测到NAT内网IP({}), 不发送wlan_user_ip", adapter_ip);
    }

    let wlan_user_ip_param = if nat_ip { "" } else { adapter_ip };
    let portal_base_with_port = ensure_portal_port(portal_base);
    let status_url = build_portal_status_url(&portal_base_with_port, account, password, wlan_user_ip_param);

    log_portal_api_request(adapter_name, adapter_ip);

    match execute_portal_api_request(client, &status_url) {
        Ok(data) => {
            let (online, login_available) = parse_portal_api_result(&data);
            let status = PortalStatus {
                reachable: true,
                login_available,
                online,
                message: if online { "已在线".to_string() } else { "未登录".to_string() },
                data_length: data.len(),
                error_kind: None,
            };
            log_portal_final_result(t0.elapsed(), adapter_name, adapter_ip, &status);
            Ok(status)
        }
        Err(ApiRequestError::RequestFailed) => {
            // 页面检测已确认 Portal 可达（Unknown 仅表示无法判断登录态），API 失败不应推翻页面可达性
            Ok(PortalStatus {
                reachable: true,
                login_available: true,
                online: false,
                message: "Portal 页面可达，API 检测失败".to_string(),
                data_length: 0,
                error_kind: Some("request_failed".to_string()),
            })
        }
        Err(ApiRequestError::ResponseTooLarge) => {
            Ok(PortalStatus {
                reachable: false,
                online: false,
                login_available: true,
                message: "响应体过大".to_string(),
                data_length: 0,
                error_kind: Some("response_too_large".to_string()),
            })
        }
    }
}

/// 解析适配器 IP 地址
fn parse_adapter_ip(adapter_ip: &str) -> Option<std::net::IpAddr> {
    if !adapter_ip.is_empty() {
        adapter_ip.parse::<std::net::IpAddr>().ok()
    } else {
        None
    }
}

/// 构建"已确定登录状态"的 PortalStatus
fn build_determined_status(online: bool) -> PortalStatus {
    let (online_val, login_available) = if online { (true, false) } else { (false, true) };
    PortalStatus {
        reachable: true,
        login_available,
        online: online_val,
        message: if online_val { "已在线".to_string() } else { "未登录".to_string() },
        data_length: 0,
        error_kind: None,
    }
}

/// 构建"页面检测无法判断且无凭据"的 PortalStatus
fn build_unknown_status() -> PortalStatus {
    PortalStatus {
        reachable: true,
        login_available: true,
        online: false,
        message: "页面检测无法判断登录状态".to_string(),
        data_length: 0,
        error_kind: None,
    }
}

/// 构建"页面请求失败"的 PortalStatus
fn build_request_failed_status() -> PortalStatus {
    PortalStatus {
        reachable: false,
        login_available: false,
        online: false,
        message: "Portal页面请求失败".to_string(),
        data_length: 0,
        error_kind: Some("request_failed".to_string()),
    }
}

/// 构建 Portal API 状态查询 URL
fn build_portal_status_url(portal_base: &str, account: &str, password: &str, wlan_user_ip: &str) -> String {
    format!(
        "{}/eportal/portal/login?callback=dr1003&login_method=1&user_account={}&user_password={}&wlan_user_ip={}&wlan_user_ipv6=&wlan_user_mac=000000000000&wlan_ac_ip=&wlan_ac_name=&jsVersion=4.1.3&terminal_type=1&lang=zh-cn&v={}&lang=zh",
        portal_base,
        urlencoding::encode(account),
        urlencoding::encode(password),
        urlencoding::encode(wlan_user_ip),
        crate::auth::protocol::random_v()
    )
}

/// API 请求错误类型
enum ApiRequestError {
    RequestFailed,
    ResponseTooLarge,
}

/// 执行 Portal API 请求并返回响应文本
fn execute_portal_api_request(client: &reqwest::Client, url: &str) -> Result<String, ApiRequestError> {
    let t_req = std::time::Instant::now();

    let resp = match block_on_http(
        client.get(url).timeout(portal_config::REQUEST_TIMEOUT).send()
    ) {
        Ok(r) => r,
        Err(e) => {
            crate::log_warn!("network", "Portal API备用检测失败({}ms): {}", t_req.elapsed().as_millis(), e);
            return Err(ApiRequestError::RequestFailed);
        }
    };

    let status_code = resp.status();
    if resp.content_length().map(|len| len > portal_config::MAX_RESPONSE_SIZE).unwrap_or(false) {
        return Err(ApiRequestError::ResponseTooLarge);
    }

    let data = block_on_http(resp.text()).unwrap_or_default();
    let req_elapsed = t_req.elapsed();

    crate::log_debug!("network", "Portal API备用检测响应: 状态码={:?}, bodyLen={}, 耗时{}ms",
        status_code, data.len(), req_elapsed.as_millis());

    Ok(data)
}

/// 解析 Portal API 响应，返回 (online, login_available)
fn parse_portal_api_result(data: &str) -> (bool, bool) {
    let dr1003_result = parse_dr1003_result(data);
    match dr1003_result {
        Some((result_val, ret_code)) => match result_val {
            1 => (true, false),
            0 => match ret_code {
                Some(2) => (true, false),
                _ => (false, true),
            },
            // result=2 表示已经在线，与 protocol.rs 中 parse_login_result 的语义保持一致
            2 => (true, false),
            _ => (false, true),
        },
        None => {
            crate::log_warn!("network", "Portal API也无法解析: {}", safe_truncate(data, 200));
            (false, true)
        }
    }
}

fn parse_dr1003_result(data: &str) -> Option<(i64, Option<i64>)> {
    let start = data.find(portal_config::JSONP_CALLBACK_PREFIX)?;
    let inner_start = start + portal_config::JSONP_CALLBACK_PREFIX.len();
    let inner_end = data[inner_start..].rfind(')').map(|i| inner_start + i)?;
    let json_str = &data[inner_start..inner_end];
    let val: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let result_val = val.get("result")?.as_i64()?;
    let ret_code = val.get("ret_code").and_then(|v| v.as_i64());
    Some((result_val, ret_code))
}

pub(crate) fn is_nat_private_ip(ip: &str) -> bool {
    if ip.starts_with("10.") {
        return true;
    }
    if ip.starts_with("192.168.") || ip.starts_with("169.254.") {
        return true;
    }
    if let Some(rest) = ip.strip_prefix("172.") {
        if let Some(second) = rest.split('.').next() {
            if let Ok(o) = second.parse::<u8>() {
                return (16..=31).contains(&o);
            }
        }
    }
    if let Some(rest) = ip.strip_prefix("100.") {
        if let Some(second) = rest.split('.').next() {
            if let Ok(o) = second.parse::<u8>() {
                return (64..=127).contains(&o);
            }
        }
    }
    false
}

enum PageCheckResult {
    Determined(bool),
    Unknown,
    Failed,
}

fn check_portal_page(client: &reqwest::Client, portal_base: &str) -> PageCheckResult {
    let page_url = format!("{portal_base}/");
    let resp = match block_on_http(
        client.get(&page_url).timeout(portal_config::REQUEST_TIMEOUT).send()
    ) {
        Ok(r) => r,
        Err(e) => {
            crate::log_warn!("network", "Portal页面请求失败: {}", e);
            return PageCheckResult::Failed;
        }
    };

    let html = match block_on_http(resp.text()) {
        Ok(t) => t,
        Err(e) => {
            crate::log_warn!("network", "Portal页面读取失败: {}", e);
            return PageCheckResult::Failed;
        }
    };

    crate::log_debug!("network", "Portal页面响应长度: {}", html.len());

    analyze_portal_page_content(&html)
}

/// 分析 Portal 页面内容，判断登录状态
fn analyze_portal_page_content(html: &str) -> PageCheckResult {
    // 检查已知页面特征字符串
    let page_indicators = [
        (portal_config::PAGE_INDICATOR_LOGOUT, true),
        (portal_config::PAGE_TITLE_LOGOUT, true),
        (portal_config::PAGE_INDICATOR_LOGIN_0, false),
        (portal_config::PAGE_INDICATOR_LOGIN_2, false),
        (portal_config::PAGE_TITLE_LOGIN, false),
    ];

    for (indicator, is_online) in page_indicators {
        if html.contains(indicator) {
            log_page_indicator_found(indicator, is_online);
            return PageCheckResult::Determined(is_online);
        }
    }

    // 检查用户会话信息特征
    if has_user_session_indicators(html) {
        crate::log_debug!("network", "Portal页面检测: 发现用户信息(uid/v4ip/oltime)，判定已在线");
        return PageCheckResult::Determined(true);
    }

    crate::log_info!("network", "Portal页面无法判断登录状态: {}", safe_truncate(html, 300));
    PageCheckResult::Unknown
}

/// 检查页面中是否包含用户会话信息特征
fn has_user_session_indicators(html: &str) -> bool {
    let has_uid = html.contains("uid='") && !html.contains("uid=''");
    let has_v4ip = html.contains("v4ip='") && !html.contains("v4ip='0.") && !html.contains("v4ip=''");
    let has_oltime = html.contains("oltime=") && !html.contains("oltime=0");
    has_uid || (has_v4ip && has_oltime)
}

// ===== 日志辅助函数 =====

fn log_portal_query_start(adapter_name: Option<&str>, adapter_ip: &str) {
    crate::log_debug!("network", "Portal状态查询: adapter={}, ip={}, 优先页面检测",
        adapter_name.unwrap_or("unknown"), adapter_ip);
}

fn log_portal_page_result(elapsed: std::time::Duration, adapter_name: Option<&str>, adapter_ip: &str, status: &PortalStatus) {
    crate::log_debug!("network", "Portal页面检测结果({}ms): adapter={}, ip={}, online={}, msg={}",
        elapsed.as_millis(), adapter_name.unwrap_or("unknown"), adapter_ip, status.online, status.message);
}

fn log_portal_final_result(elapsed: std::time::Duration, adapter_name: Option<&str>, adapter_ip: &str, status: &PortalStatus) {
    crate::log_debug!("network", "Portal检测结果({}ms): adapter={}, ip={}, reachable={}, loginAvailable={}, online={}, msg={}",
        elapsed.as_millis(), adapter_name.unwrap_or("unknown"), adapter_ip,
        status.reachable, status.login_available, status.online, status.message);
}

fn log_portal_page_failed(adapter_name: Option<&str>, adapter_ip: &str) {
    crate::log_debug!("network", "Portal页面请求失败: adapter={}, ip={}",
        adapter_name.unwrap_or("unknown"), adapter_ip);
}

fn log_portal_no_credentials(adapter_name: Option<&str>, adapter_ip: &str) {
    crate::log_debug!("network", "Portal页面检测无法判断且无凭据: adapter={}, ip={}",
        adapter_name.unwrap_or("unknown"), adapter_ip);
}

fn log_portal_api_fallback(adapter_name: Option<&str>, adapter_ip: &str) {
    crate::log_info!("network", "Portal页面检测无法判断, 尝试API备用检测: adapter={}, ip={}",
        adapter_name.unwrap_or("unknown"), adapter_ip);
}

fn log_portal_api_request(adapter_name: Option<&str>, adapter_ip: &str) {
    crate::log_debug!("network", "Portal API备用检测请求: adapter={}, ip={}",
        adapter_name.unwrap_or("unknown"), adapter_ip);
}

fn log_page_indicator_found(indicator: &str, is_online: bool) {
    let label = if is_online { "已在线" } else { "未登录" };
    crate::log_debug!("network", "Portal页面检测: 发现{}，判定{}", indicator, label);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_portal_port_adds_default_port() {
        assert_eq!(ensure_portal_port("http://10.0.0.1"), "http://10.0.0.1:801");
    }

    #[test]
    fn ensure_portal_port_preserves_existing_port() {
        assert_eq!(ensure_portal_port("http://10.0.0.1:8080"), "http://10.0.0.1:8080");
    }

    #[test]
    fn ensure_portal_port_handles_trailing_slash() {
        assert_eq!(ensure_portal_port("http://10.0.0.1/"), "http://10.0.0.1:801");
    }

    #[test]
    fn is_nat_private_ip_recognizes_class_a() {
        assert!(is_nat_private_ip("10.1.2.3"));
    }

    #[test]
    fn is_nat_private_ip_recognizes_class_b() {
        assert!(is_nat_private_ip("172.16.0.1"));
        assert!(is_nat_private_ip("172.31.255.255"));
        assert!(!is_nat_private_ip("172.32.0.1"));
    }

    #[test]
    fn is_nat_private_ip_recognizes_class_c() {
        assert!(is_nat_private_ip("192.168.1.1"));
    }

    #[test]
    fn is_nat_private_ip_rejects_public_ip() {
        assert!(!is_nat_private_ip("8.8.8.8"));
    }

    // ===== 新增测试：覆盖提取的函数 =====

    #[test]
    fn parse_adapter_ip_parses_valid_ip() {
        assert!(parse_adapter_ip("192.168.1.1").is_some());
        assert!(parse_adapter_ip("10.0.0.1").is_some());
    }

    #[test]
    fn parse_adapter_ip_returns_none_for_empty() {
        assert!(parse_adapter_ip("").is_none());
    }

    #[test]
    fn parse_adapter_ip_returns_none_for_invalid() {
        assert!(parse_adapter_ip("invalid").is_none());
        assert!(parse_adapter_ip("999.999.999.999").is_none());
    }

    #[test]
    fn build_determined_status_online() {
        let status = build_determined_status(true);
        assert!(status.online);
        assert!(!status.login_available);
        assert!(status.reachable);
        assert_eq!(status.message, "已在线");
        assert_eq!(status.data_length, 0);
        assert!(status.error_kind.is_none());
    }

    #[test]
    fn build_determined_status_offline() {
        let status = build_determined_status(false);
        assert!(!status.online);
        assert!(status.login_available);
        assert!(status.reachable);
        assert_eq!(status.message, "未登录");
    }

    #[test]
    fn build_unknown_status_correct() {
        let status = build_unknown_status();
        assert!(status.reachable);
        assert!(status.login_available);
        assert!(!status.online);
        assert_eq!(status.message, "页面检测无法判断登录状态");
    }

    #[test]
    fn build_request_failed_status_correct() {
        let status = build_request_failed_status();
        assert!(!status.reachable);
        assert!(!status.login_available);
        assert!(!status.online);
        assert_eq!(status.message, "Portal页面请求失败");
        assert_eq!(status.error_kind, Some("request_failed".to_string()));
    }

    #[test]
    fn build_portal_status_url_contains_required_params() {
        let url = build_portal_status_url("http://10.0.0.1:801", "user", "pass", "192.168.1.1");
        assert!(url.contains("callback=dr1003"));
        assert!(url.contains("login_method=1"));
        assert!(url.contains("user_account=user"));
        assert!(url.contains("user_password=pass"));
        assert!(url.contains("wlan_user_ip=192.168.1.1"));
        assert!(url.contains("jsVersion=4.1.3"));
    }

    #[test]
    fn build_portal_status_url_encodes_special_chars() {
        let url = build_portal_status_url("http://10.0.0.1:801", "user@test", "p@ss", "10.0.0.1");
        assert!(url.contains("user_account=user%40test"));
        assert!(url.contains("user_password=p%40ss"));
    }

    #[test]
    fn build_portal_status_url_empty_ip_for_nat() {
        let url = build_portal_status_url("http://10.0.0.1:801", "user", "pass", "");
        assert!(url.contains("wlan_user_ip="));
    }

    #[test]
    fn parse_portal_api_result_online_result_1() {
        let data = "dr1003({\"result\":1,\"ret_code\":0})";
        let (online, login_available) = parse_portal_api_result(data);
        assert!(online);
        assert!(!login_available);
    }

    #[test]
    fn parse_portal_api_result_offline_result_0() {
        let data = "dr1003({\"result\":0,\"ret_code\":0})";
        let (online, login_available) = parse_portal_api_result(data);
        assert!(!online);
        assert!(login_available);
    }

    #[test]
    fn parse_portal_api_result_online_result_0_ret_code_2() {
        let data = "dr1003({\"result\":0,\"ret_code\":2})";
        let (online, login_available) = parse_portal_api_result(data);
        assert!(online);
        assert!(!login_available);
    }

    #[test]
    fn parse_portal_api_result_online_result_2() {
        let data = "dr1003({\"result\":2})";
        let (online, login_available) = parse_portal_api_result(data);
        assert!(online);
        assert!(!login_available);
    }

    #[test]
    fn parse_portal_api_result_unparseable() {
        let data = "invalid data";
        let (online, login_available) = parse_portal_api_result(data);
        assert!(!online);
        assert!(login_available);
    }

    #[test]
    fn parse_dr1003_result_valid() {
        let data = "dr1003({\"result\":1,\"ret_code\":0})";
        let result = parse_dr1003_result(data);
        assert_eq!(result, Some((1, Some(0))));
    }

    #[test]
    fn parse_dr1003_result_no_callback() {
        let data = "no callback here";
        assert!(parse_dr1003_result(data).is_none());
    }

    #[test]
    fn parse_dr1003_result_missing_result_field() {
        let data = "dr1003({\"ret_code\":0})";
        assert!(parse_dr1003_result(data).is_none());
    }

    #[test]
    fn analyze_page_logout_indicator() {
        let html = "<html>Dr.COMWebLoginID_1</html>";
        let result = analyze_portal_page_content(html);
        assert!(matches!(result, PageCheckResult::Determined(true)));
    }

    #[test]
    fn analyze_page_login_indicator_0() {
        let html = "<html>Dr.COMWebLoginID_0</html>";
        let result = analyze_portal_page_content(html);
        assert!(matches!(result, PageCheckResult::Determined(false)));
    }

    #[test]
    fn analyze_page_login_indicator_2() {
        let html = "<html>Dr.COMWebLoginID_2</html>";
        let result = analyze_portal_page_content(html);
        assert!(matches!(result, PageCheckResult::Determined(false)));
    }

    #[test]
    fn analyze_page_logout_title() {
        let html = "<html><title>注销页</title></html>";
        let result = analyze_portal_page_content(html);
        assert!(matches!(result, PageCheckResult::Determined(true)));
    }

    #[test]
    fn analyze_page_login_title() {
        let html = "<html><title>登录页</title></html>";
        let result = analyze_portal_page_content(html);
        assert!(matches!(result, PageCheckResult::Determined(false)));
    }

    #[test]
    fn analyze_page_user_session_uid() {
        let html = "<html>uid='user123'</html>";
        let result = analyze_portal_page_content(html);
        assert!(matches!(result, PageCheckResult::Determined(true)));
    }

    #[test]
    fn analyze_page_user_session_v4ip_oltime() {
        let html = "<html>v4ip='192.168.1.1' oltime=100</html>";
        let result = analyze_portal_page_content(html);
        assert!(matches!(result, PageCheckResult::Determined(true)));
    }

    #[test]
    fn analyze_page_empty_uid_not_online() {
        let html = "<html>uid=''</html>";
        let result = analyze_portal_page_content(html);
        assert!(matches!(result, PageCheckResult::Unknown));
    }

    #[test]
    fn analyze_page_zero_v4ip_not_online() {
        let html = "<html>v4ip='0.0.0.0' oltime=0</html>";
        let result = analyze_portal_page_content(html);
        assert!(matches!(result, PageCheckResult::Unknown));
    }

    #[test]
    fn analyze_page_unknown_content() {
        let html = "<html>random content</html>";
        let result = analyze_portal_page_content(html);
        assert!(matches!(result, PageCheckResult::Unknown));
    }

    #[test]
    fn has_user_session_indicators_with_valid_uid() {
        let html = "uid='user123'";
        assert!(has_user_session_indicators(html));
    }

    #[test]
    fn has_user_session_indicators_with_empty_uid() {
        let html = "uid=''";
        assert!(!has_user_session_indicators(html));
    }

    #[test]
    fn has_user_session_indicators_with_v4ip_and_oltime() {
        let html = "v4ip='192.168.1.1' oltime=100";
        assert!(has_user_session_indicators(html));
    }

    #[test]
    fn has_user_session_indicators_with_zero_v4ip() {
        let html = "v4ip='0.0.0.0' oltime=100";
        assert!(!has_user_session_indicators(html));
    }

    #[test]
    fn has_user_session_indicators_with_zero_oltime() {
        let html = "v4ip='192.168.1.1' oltime=0";
        assert!(!has_user_session_indicators(html));
    }

    #[test]
    fn has_user_session_indicators_no_indicators() {
        let html = "no session info";
        assert!(!has_user_session_indicators(html));
    }
}
