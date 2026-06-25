use crate::network::client::{PORTAL_URL, create_safe_http_client};

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
    let local_addr = if !adapter_ip.is_empty() {
        adapter_ip.parse::<std::net::IpAddr>().ok()
    } else {
        None
    };

    let client = create_safe_http_client(std::time::Duration::from_secs(8), local_addr)?;
    let portal_base = portal_url.trim_end_matches('/');
    let account = user_account.unwrap_or("");
    let password = user_password.unwrap_or("");

    crate::log_debug!("network", "Portal状态查询: adapter={}, ip={}, 优先页面检测", adapter_name.unwrap_or("unknown"), adapter_ip);
    let page_result = check_portal_page(&client, portal_base);

    match page_result {
        PageCheckResult::Determined(online) => {
            let (online_val, login_available) = if online { (true, false) } else { (false, true) };
            let label = if online_val { "已在线".to_string() } else { "未登录".to_string() };
            crate::log_debug!("network", "Portal页面检测结果({}ms): adapter={}, ip={}, online={}, msg={}",
                t0.elapsed().as_millis(), adapter_name.unwrap_or("unknown"), adapter_ip, online_val, label);
            Ok(PortalStatus {
                reachable: true,
                login_available,
                online: online_val,
                message: label,
                data_length: 0,
                error_kind: None,
            })
        }
        PageCheckResult::Unknown => {
            if account.is_empty() {
                crate::log_debug!("network", "Portal页面检测无法判断且无凭据: adapter={}, ip={}", adapter_name.unwrap_or("unknown"), adapter_ip);
                return Ok(PortalStatus {
                    reachable: true,
                    login_available: true,
                    online: false,
                    message: "页面检测无法判断登录状态".to_string(),
                    data_length: 0,
                    error_kind: None,
                });
            }

            crate::log_info!("network", "Portal页面检测无法判断, 尝试API备用检测: adapter={}, ip={}", adapter_name.unwrap_or("unknown"), adapter_ip);

            let nat_ip = is_nat_private_ip(adapter_ip);
            if nat_ip {
                crate::log_info!("network", "检测到NAT内网IP({}), 不发送wlan_user_ip", adapter_ip);
            }

            let wlan_user_ip_param = if nat_ip { "" } else { adapter_ip };
            let portal_base_with_port = ensure_portal_port(portal_base);
            let status_url = format!("{}/eportal/portal/login?callback=dr1003&login_method=1&user_account={}&user_password={}&wlan_user_ip={}&wlan_user_ipv6=&wlan_user_mac=000000000000&wlan_ac_ip=&wlan_ac_name=&jsVersion=4.1.3&terminal_type=1&lang=zh-cn&v={}&lang=zh",
                portal_base_with_port,
                urlencoding::encode(account),
                urlencoding::encode(password),
                urlencoding::encode(wlan_user_ip_param),
                crate::auth::protocol::random_v()
            );

            crate::log_debug!("network", "Portal API备用检测请求: adapter={}, ip={}",
                adapter_name.unwrap_or("unknown"), adapter_ip);

            let t_req = std::time::Instant::now();
            let resp = match tauri::async_runtime::block_on(
                client.get(&status_url).timeout(std::time::Duration::from_secs(3)).send()
            ) {
                Ok(r) => r,
                Err(e) => {
                    crate::log_warn!("network", "Portal API备用检测失败({}ms): {}", t_req.elapsed().as_millis(), e);
                    return Ok(PortalStatus {
                        // 页面检测已确认 Portal 可达（Unknown 仅表示无法判断登录态），API 失败不应推翻页面可达性
                        reachable: true,
                        login_available: true,
                        online: false,
                        message: "Portal 页面可达，API 检测失败".to_string(),
                        data_length: 0,
                        error_kind: Some("request_failed".to_string()),
                    });
                }
            };

            let status_code = resp.status();
            if resp.content_length().map(|len| len > 1024 * 1024).unwrap_or(false) {
                return Ok(PortalStatus {
                    reachable: false,
                    online: false,
                    login_available: true,
                    message: "响应体过大".to_string(),
                    data_length: 0,
                    error_kind: Some("response_too_large".to_string()),
                });
            }
            let data = tauri::async_runtime::block_on(resp.text()).unwrap_or_default();
            let req_elapsed = t_req.elapsed();

            crate::log_debug!("network", "Portal API备用检测响应: 状态码={:?}, bodyLen={}, 耗时{}ms",
                status_code, data.len(), req_elapsed.as_millis());

            let dr1003_result = parse_dr1003_result(&data);
            let (online, login_available) = match dr1003_result {
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
                    crate::log_warn!("network", "Portal API也无法解析: {}", safe_truncate(&data, 200));
                    (false, true)
                }
            };

            let label = if online { "已在线".to_string() } else { "未登录".to_string() };

            crate::log_debug!("network", "Portal检测结果({}ms): adapter={}, ip={}, reachable={}, loginAvailable={}, online={}, msg={}",
                t0.elapsed().as_millis(), adapter_name.unwrap_or("unknown"), adapter_ip, true, login_available, online, label);

            Ok(PortalStatus {
                reachable: true,
                login_available,
                online,
                message: label,
                data_length: data.len(),
                error_kind: None,
            })
        }
        PageCheckResult::Failed => {
            crate::log_debug!("network", "Portal页面请求失败: adapter={}, ip={}", adapter_name.unwrap_or("unknown"), adapter_ip);
            Ok(PortalStatus {
                reachable: false,
                login_available: false,
                online: false,
                message: "Portal页面请求失败".to_string(),
                data_length: 0,
                error_kind: Some("request_failed".to_string()),
            })
        }
    }
}

fn parse_dr1003_result(data: &str) -> Option<(i64, Option<i64>)> {
    let start = data.find("dr1003(")?;
    let inner_start = start + 7;
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
}

enum PageCheckResult {
    Determined(bool),
    Unknown,
    Failed,
}

fn check_portal_page(client: &reqwest::Client, portal_base: &str) -> PageCheckResult {
    let page_url = format!("{portal_base}/");
    let resp = match tauri::async_runtime::block_on(
        client.get(&page_url).timeout(std::time::Duration::from_secs(3)).send()
    ) {
        Ok(r) => r,
        Err(e) => {
            crate::log_warn!("network", "Portal页面请求失败: {}", e);
            return PageCheckResult::Failed;
        }
    };

    let html = match tauri::async_runtime::block_on(resp.text()) {
        Ok(t) => t,
        Err(e) => {
            crate::log_warn!("network", "Portal页面读取失败: {}", e);
            return PageCheckResult::Failed;
        }
    };

    crate::log_debug!("network", "Portal页面响应长度: {}", html.len());

    if html.contains("Dr.COMWebLoginID_1") {
        crate::log_debug!("network", "Portal页面检测: 发现Dr.COMWebLoginID_1(注销页)，判定已在线");
        return PageCheckResult::Determined(true);
    }

    if html.contains("Dr.COMWebLoginID_0") || html.contains("Dr.COMWebLoginID_2") {
        crate::log_debug!("network", "Portal页面检测: 发现Dr.COMWebLoginID_0/2(登录页)，判定未登录");
        return PageCheckResult::Determined(false);
    }

    if html.contains("<title>注销页</title>") {
        crate::log_debug!("network", "Portal页面检测: 发现注销页标题，判定已在线");
        return PageCheckResult::Determined(true);
    }

    if html.contains("<title>登录页</title>") {
        crate::log_debug!("network", "Portal页面检测: 发现登录页标题，判定未登录");
        return PageCheckResult::Determined(false);
    }

    let has_uid = html.contains("uid='") && !html.contains("uid=''");
    let has_v4ip = html.contains("v4ip='") && !html.contains("v4ip='0.") && !html.contains("v4ip=''");
    let has_oltime = html.contains("oltime=") && !html.contains("oltime=0");

    if has_uid || (has_v4ip && has_oltime) {
        crate::log_debug!("network", "Portal页面检测: 发现用户信息(uid/v4ip/oltime)，判定已在线");
        return PageCheckResult::Determined(true);
    }

    crate::log_info!("network", "Portal页面无法判断登录状态: {}", safe_truncate(&html, 300));
    PageCheckResult::Unknown
}
