use crate::network::client::{PORTAL_URL, create_safe_http_client};

/// Portal 检测相关配置常量
mod portal_config {
    use std::time::Duration;

    /// HTTP 客户端创建超时
    pub const CLIENT_TIMEOUT: Duration = Duration::from_secs(8);
    /// 单次 HTTP 请求超时
    pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

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
    // 统一走 block_on_sync：其 Err 分支（无 runtime context 的裸线程）必须用
    // 自持 Runtime::block_on 驱动——tauri::async_runtime::block_on 在 set(handle)
    // 后退化为 Handle::block_on，实测无法支撑 reqwest 的 timer 资源访问
    // （详见 infra/async_util.rs 顶部说明）。
    crate::infra::async_util::block_on_sync(future)
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

/// 错误信息脱敏：reqwest 错误的 Display 会携带完整请求 URL（含 user_password），
/// 将完整 URL、URL 编码后的密码与明文密码统一替换为占位符，避免凭据泄漏到通知/日志。
pub fn redact_credentials(mut msg: String, url: &str, base_url: &str, password: &str) -> String {
    if !url.is_empty() {
        msg = msg.replace(url, &format!("{base_url}?***"));
    }
    let encoded = urlencoding::encode(password).into_owned();
    if !encoded.is_empty() {
        msg = msg.replace(&encoded, "***");
    }
    if !password.is_empty() {
        msg = msg.replace(password, "***");
    }
    msg
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalStatus {
    pub reachable: bool,
    pub login_available: bool,
    pub online: bool,
    pub message: String,
    pub data_length: usize,
    /// 在线时 Portal 页面携带的 uid（完整在线账号=账号+运营商后缀，如 "2023xxxx@telecom"）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
}

pub fn check_portal_full(adapter_ip: &str, adapter_name: Option<&str>) -> Result<PortalStatus, String> {
    let t0 = std::time::Instant::now();
    let portal_url = PORTAL_URL.load().clone();
    let local_addr = parse_adapter_ip(adapter_ip);

    let client = create_safe_http_client(portal_config::CLIENT_TIMEOUT, local_addr)?;
    let portal_base = portal_url.trim_end_matches('/');

    log_portal_query_start(adapter_name, adapter_ip);
    let page_result = check_portal_page(&client, portal_base);

    match page_result {
        PageCheckResult::Determined(online, uid) => {
            let status = build_determined_status(online, uid);
            log_portal_page_result(t0.elapsed(), adapter_name, adapter_ip, &status);
            Ok(status)
        }
        PageCheckResult::Unknown => {
            Ok(handle_unknown_page_status(adapter_name, adapter_ip))
        }
        PageCheckResult::Failed => {
            log_portal_page_failed(adapter_name, adapter_ip);
            Ok(build_request_failed_status())
        }
    }
}

/// 处理页面检测无法判断登录状态的情况。
///
/// 安全约束：状态探测是只读操作，绝不允许携带用户密码调用登录端点。
/// Portal 页面无法识别时返回"需要人工确认"状态，由用户在界面上手动判断，
/// 避免两类风险：
/// 1. 错误凭据在每次状态轮询时向登录端点发起认证，触发账号锁定风险；
/// 2. 正确凭据被静默登录，但结果被 `parse_portal_api_result` 误报为"未登录"。
fn handle_unknown_page_status(adapter_name: Option<&str>, adapter_ip: &str) -> PortalStatus {
    log_portal_no_credentials(adapter_name, adapter_ip);
    PortalStatus {
        reachable: true,
        login_available: true,
        online: false,
        message: "Portal 页面无法判断登录状态，请手动确认".to_string(),
        data_length: 0,
        uid: None,
        error_kind: Some("need_manual_check".to_string()),
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
fn build_determined_status(online: bool, uid: Option<String>) -> PortalStatus {
    let (online_val, login_available) = if online { (true, false) } else { (false, true) };
    PortalStatus {
        reachable: true,
        login_available,
        online: online_val,
        message: if online_val { "已在线".to_string() } else { "未登录".to_string() },
        data_length: 0,
        uid: if online_val { uid } else { None },
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
        uid: None,
        error_kind: Some("request_failed".to_string()),
    }
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
    /// (是否在线, 页面携带的 uid——完整在线账号=账号+运营商后缀)
    Determined(bool, Option<String>),
    Unknown,
    Failed,
}

fn check_portal_page(client: &reqwest::Client, portal_base: &str) -> PageCheckResult {
    // 页面探测使用配置的原始地址（默认 80 端口 Dr.COM 网关页：HTML 内嵌
    // "注销页"/"Dr.COMWebLoginID_*"/uid=/v4ip= 等状态特征，可判定登录状态）。
    // 不要探测 :801——那是 Dr.COM EPortal 管理系统前端（SPA 登录表单），
    // 已在线状态下打开仍渲染登录页、无任何状态特征，探测必然 Unknown。
    // 历史缺陷：v2.2.x 曾把页面探测强制对齐 :801（与协议请求一致端口），
    // 校园网 801 改版为 EPortal 后误报"Portal 页面无法判断登录状态，请手动确认"，
    // 2026-09-03 经浏览器实测 80/801 渲染结果后回退为原始地址探测。
    // 登录/注销/协议请求仍强制 :801（ensure_portal_port），两者端口本就不同。
    let page_url = format!("{}/", portal_base.trim_end_matches('/'));

    // Android 旁路优先:客户端切换到旁路代理版(传输层 SO_BINDTODEVICE 物理网卡直连,
    // 见 do_login_request 注释),下方 reqwest 代码(头/重定向/charset)原样复用;
    // 旁路不可用(能力被 ROM 封堵/无物理网卡)→ 沿用传入的原客户端。
    #[cfg(target_os = "android")]
    let client = match crate::network::bound_socket::bypass_proxy_addr() {
        Some(_) => match crate::network::client::create_bypass_http_client(portal_config::REQUEST_TIMEOUT) {
            Ok(c) => c,
            Err(e) => {
                crate::log_warn!("network", "旁路客户端创建失败,走原路径: {e}");
                client.clone()
            }
        },
        None => client.clone(),
    };

    let resp = match block_on_http(
        client.get(&page_url).timeout(portal_config::REQUEST_TIMEOUT).send()
    ) {
        Ok(r) => r,
        Err(e) => {
            crate::log_warn!("network", "Portal页面请求失败: {}", e);
            return PageCheckResult::Failed;
        }
    };

    // 非 2xx 状态码：4xx/5xx 错误页不得作为正常页面分析
    // （历史缺陷：状态码未校验，错误页无登录特征时被当作 Unknown/Offline）
    if !resp.status().is_success() {
        crate::log_warn!("network", "Portal页面请求状态异常: {}", resp.status());
        return PageCheckResult::Failed;
    }

    // 复用协议路径的限长读取（1MB 上限 + charset 解码），防止异常 Portal 无限响应体
    // 读取失败/超限时 read_bounded_body 返回空串并已告警，按请求失败处理
    let html = crate::auth::protocol::read_bounded_body(resp, "Portal页面");
    if html.is_empty() {
        return PageCheckResult::Failed;
    }

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
            return PageCheckResult::Determined(is_online, extract_uid_from_html(html));
        }
    }

    // 检查用户会话信息特征
    if has_user_session_indicators(html) {
        crate::log_debug!("network", "Portal页面检测: 发现用户信息(uid/v4ip/oltime)，判定已在线");
        return PageCheckResult::Determined(true, extract_uid_from_html(html));
    }

    // 后台巡检周期性触发，Unknown 属常见中间态（801 SPA 页面回退 80 前），降为 debug 防日志刷屏
    crate::log_debug!("network", "Portal页面无法判断登录状态: {}", safe_truncate(html, 300));
    PageCheckResult::Unknown
}

/// 检查页面中是否包含用户会话信息特征
fn has_user_session_indicators(html: &str) -> bool {
    let has_uid = html.contains("uid='") && !html.contains("uid=''");
    let has_v4ip = html.contains("v4ip='") && !html.contains("v4ip='0.") && !html.contains("v4ip=''");
    let has_oltime = html.contains("oltime=") && !html.contains("oltime=0");
    has_uid || (has_v4ip && has_oltime)
}

/// 从 Portal 页面 HTML 提取 uid 值（完整在线账号=账号+运营商后缀，如 "2023xxxx@telecom"）。
/// 页面内嵌 JS 形如 `uid='xxx'`；空占位 `uid=''` 跳过继续找，全部为空/缺失返回 None。
fn extract_uid_from_html(html: &str) -> Option<String> {
    let mut from = 0;
    while let Some(rel) = html[from..].find("uid='") {
        let start = from + rel + "uid='".len();
        let rest = &html[start..];
        match rest.find('\'') {
            Some(end) if end > 0 => return Some(rest[..end].to_string()),
            // uid=''：跳过空占位，继续找下一个出现位置
            Some(_) => from = start + 1,
            None => return None,
        }
    }
    None
}

/// 从完整在线账号推导运营商后缀（与 config.operator 同口径）：`2023xxxx@telecom` → `@telecom`，
/// 无 `@` → `""`（无锡学院默认线路）；空 uid 或未知后缀 → None（前端徽标隐藏）。
pub fn operator_suffix_from_uid(uid: &str) -> Option<&str> {
    if uid.is_empty() {
        return None;
    }
    match uid.rsplit_once('@') {
        Some((_, suffix)) => match suffix {
            "telecom" => Some("@telecom"),
            "unicom" => Some("@unicom"),
            "cmcc" => Some("@cmcc"),
            _ => None,
        },
        None => Some(""),
    }
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

fn log_portal_page_failed(adapter_name: Option<&str>, adapter_ip: &str) {
    crate::log_debug!("network", "Portal页面请求失败: adapter={}, ip={}",
        adapter_name.unwrap_or("unknown"), adapter_ip);
}

fn log_portal_no_credentials(adapter_name: Option<&str>, adapter_ip: &str) {
    crate::log_debug!("network", "Portal页面检测无法判断且无凭据: adapter={}, ip={}",
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
        let status = build_determined_status(true, Some("20230001@telecom".to_string()));
        assert!(status.online);
        assert!(!status.login_available);
        assert!(status.reachable);
        assert_eq!(status.message, "已在线");
        assert_eq!(status.data_length, 0);
        assert!(status.error_kind.is_none());
        // 在线时保留解析出的 uid
        assert_eq!(status.uid.as_deref(), Some("20230001@telecom"));
    }

    #[test]
    fn build_determined_status_offline() {
        // 离线判定时即使误携带 uid 也必须丢弃（徽标仅在线态渲染）
        let status = build_determined_status(false, Some("20230001@telecom".to_string()));
        assert!(!status.online);
        assert!(status.login_available);
        assert!(status.reachable);
        assert_eq!(status.message, "未登录");
        assert!(status.uid.is_none());
    }

    #[test]
    fn handle_unknown_page_status_returns_manual_check() {
        // 页面无法识别时：可达、可登录但 online=false，且 error_kind=need_manual_check
        let status = handle_unknown_page_status(Some("以太网"), "192.168.1.1");
        assert!(status.reachable);
        assert!(status.login_available);
        assert!(!status.online);
        assert_eq!(status.error_kind, Some("need_manual_check".to_string()));
    }

    #[test]
    fn handle_unknown_page_status_is_readonly_no_credentials() {
        // 关键安全约束：状态探测不得执行登录。页面无法识别时直接返回，
        // 不得携带任何凭据向登录端点发起请求（此函数签名已不再接受密码）。
        let status = handle_unknown_page_status(None, "");
        assert!(!status.online);
        assert_eq!(status.message, "Portal 页面无法判断登录状态，请手动确认");
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
    fn analyze_page_logout_indicator() {
        let html = "<html>Dr.COMWebLoginID_1</html>";
        let result = analyze_portal_page_content(html);
        assert!(matches!(result, PageCheckResult::Determined(true, _)));
    }

    #[test]
    fn analyze_page_login_indicator_0() {
        let html = "<html>Dr.COMWebLoginID_0</html>";
        let result = analyze_portal_page_content(html);
        assert!(matches!(result, PageCheckResult::Determined(false, _)));
    }

    #[test]
    fn analyze_page_login_indicator_2() {
        let html = "<html>Dr.COMWebLoginID_2</html>";
        let result = analyze_portal_page_content(html);
        assert!(matches!(result, PageCheckResult::Determined(false, _)));
    }

    #[test]
    fn analyze_page_logout_title() {
        let html = "<html><title>注销页</title></html>";
        let result = analyze_portal_page_content(html);
        assert!(matches!(result, PageCheckResult::Determined(true, _)));
    }

    #[test]
    fn analyze_page_login_title() {
        let html = "<html><title>登录页</title></html>";
        let result = analyze_portal_page_content(html);
        assert!(matches!(result, PageCheckResult::Determined(false, _)));
    }

    #[test]
    fn analyze_page_user_session_uid() {
        let html = "<html>uid='user123'</html>";
        let result = analyze_portal_page_content(html);
        assert!(matches!(result, PageCheckResult::Determined(true, _)));
    }

    #[test]
    fn analyze_page_user_session_v4ip_oltime() {
        let html = "<html>v4ip='192.168.1.1' oltime=100</html>";
        let result = analyze_portal_page_content(html);
        assert!(matches!(result, PageCheckResult::Determined(true, _)));
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

    #[test]
    fn extract_uid_在线页取值() {
        let html = "var uid='20240001@cmcc';";
        assert_eq!(extract_uid_from_html(html), Some("20240001@cmcc".to_string()));
    }

    #[test]
    fn extract_uid_空占位跳过取后续值() {
        let html = "uid='' uid='x@telecom'";
        assert_eq!(extract_uid_from_html(html), Some("x@telecom".to_string()));
    }

    #[test]
    fn extract_uid_仅空占位返回none() {
        assert_eq!(extract_uid_from_html("uid=''"), None);
    }

    #[test]
    fn extract_uid_无闭合引号返回none() {
        assert_eq!(extract_uid_from_html("uid='abc"), None);
    }

    #[test]
    fn extract_uid_缺失返回none() {
        assert_eq!(extract_uid_from_html("<html>v4ip='1.2.3.4' oltime=9</html>"), None);
    }

    #[test]
    fn operator_suffix_三家运营商() {
        assert_eq!(operator_suffix_from_uid("20240001@telecom"), Some("@telecom"));
        assert_eq!(operator_suffix_from_uid("20240001@unicom"), Some("@unicom"));
        assert_eq!(operator_suffix_from_uid("20240001@cmcc"), Some("@cmcc"));
    }

    #[test]
    fn operator_suffix_无at为默认线路() {
        assert_eq!(operator_suffix_from_uid("20240001"), Some(""));
    }

    #[test]
    fn operator_suffix_未知后缀返回none() {
        assert_eq!(operator_suffix_from_uid("20240001@edu"), None);
    }

    #[test]
    fn operator_suffix_空uid返回none() {
        assert_eq!(operator_suffix_from_uid(""), None);
    }
}
