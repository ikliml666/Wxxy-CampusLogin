use std::io::Read;
use std::sync::Arc;

use super::cache::{NET_CACHE, MAX_RESPONSE_SIZE, create_safe_http_client};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalStatus {
    pub reachable: bool,
    pub login_available: bool,
    pub online: bool,
    pub message: String,
    pub data_length: usize,
}

pub fn check_portal_full(adapter_ip: &str, _adapter_name: Option<&str>, user_account: Option<&str>, user_password: Option<&str>) -> Result<PortalStatus, String> {
    {
        let cache_arc = NET_CACHE.portal.load();
        if let Some(entry) = cache_arc.as_ref() {
            if entry.adapter_ip == adapter_ip && entry.time.elapsed().as_secs() < 8 {
                return Ok(entry.status.clone());
            }
        }
    }

    let result = check_portal_full_inner(adapter_ip, _adapter_name, user_account, user_password);

    match result {
        Ok(status) => {
            NET_CACHE.portal.store(Arc::new(Some(super::cache::PortalCacheEntry {
                time: std::time::Instant::now(),
                status: status.clone(),
                adapter_ip: adapter_ip.to_string(),
            })));
            Ok(status)
        }
        Err(e) => Err(e),
    }
}

fn parse_dr1003_result(data: &str) -> Option<(i64, Option<i64>)> {
    let start = data.find("dr1003(")?;
    let inner_start = start + 7;
    let inner_end = data[inner_start..].find(')').map(|i| inner_start + i)?;
    let json_str = &data[inner_start..inner_end];
    let val: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let result_val = val.get("result")?.as_i64()?;
    let ret_code = val.get("ret_code").and_then(|v| v.as_i64());
    Some((result_val, ret_code))
}

fn is_nat_private_ip(ip: &str) -> bool {
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
    false
}

fn check_portal_full_inner(adapter_ip: &str, _adapter_name: Option<&str>, user_account: Option<&str>, user_password: Option<&str>) -> Result<PortalStatus, String> {
    let portal_url = NET_CACHE.portal_url.load().clone();
    let local_addr = if !adapter_ip.is_empty() {
        adapter_ip.parse::<std::net::IpAddr>().ok()
    } else {
        None
    };

    let client = create_safe_http_client(std::time::Duration::from_secs(6), local_addr)?;
    let portal_base = portal_url.trim_end_matches('/');
    let account = user_account.unwrap_or("");
    let password = user_password.unwrap_or("");

    let nat_ip = is_nat_private_ip(adapter_ip);
    if nat_ip {
        crate::log_info!("network", "检测到NAT内网IP({}), 不发送wlan_user_ip", adapter_ip);
    }

    let wlan_user_ip_param = if nat_ip { "" } else { adapter_ip };
    let status_url = format!("{}:801/eportal/portal/login?callback=dr1003&login_method=1&user_account={}&user_password={}&wlan_user_ip={}&wlan_user_ipv6=&wlan_user_mac=000000000000&wlan_ac_ip=&wlan_ac_name=&jsVersion=4.1.3&terminal_type=1&lang=zh-cn&v=3043&lang=zh",
        portal_base,
        urlencoding::encode(account),
        urlencoding::encode(password),
        urlencoding::encode(wlan_user_ip_param)
    );

    let mut resp = match client.get(&status_url).timeout(std::time::Duration::from_secs(5)).send() {
        Ok(r) => r,
        Err(e) => {
            crate::log_warn!("network", "Portal状态查询失败: {}", e);
            return Ok(PortalStatus {
                reachable: false,
                login_available: false,
                online: false,
                message: "网络检测失败".to_string(),
                data_length: 0,
            });
        }
    };

    let mut data = String::new();
    let mut limited = (&mut resp).take(MAX_RESPONSE_SIZE as u64);
    let _ = limited.read_to_string(&mut data);
    let _ = std::io::copy(&mut resp, &mut std::io::sink());

    let dr1003_result = parse_dr1003_result(&data);
    let (online, login_available) = match dr1003_result {
        Some((result_val, ret_code)) => match result_val {
            1 => (true, false),
            0 => match ret_code {
                Some(2) => (true, false),
                _ => (false, true),
            },
            _ => (false, true),
        },
        None => {
            crate::log_info!("network", "801端口API无法解析，尝试Portal页面备用检测");
            match check_portal_page(&client, &portal_base) {
                Some(portal_online) => {
                    if portal_online {
                        crate::log_info!("network", "Portal页面备用检测: 已登录");
                        (true, false)
                    } else {
                        crate::log_info!("network", "Portal页面备用检测: 未登录");
                        (false, true)
                    }
                }
                None => {
                    crate::log_warn!("network", "Portal页面备用检测也失败: {}", &data[..data.len().min(200)]);
                    (false, true)
                }
            }
        }
    };

    let label = if online { "已在线".to_string() } else { "未登录".to_string() };

    Ok(PortalStatus {
        reachable: true,
        login_available,
        online,
        message: label,
        data_length: data.len(),
    })
}

fn check_portal_page(client: &reqwest::blocking::Client, portal_base: &str) -> Option<bool> {
    let page_url = format!("{}/", portal_base);
    let resp = match client.get(&page_url).timeout(std::time::Duration::from_secs(4)).send() {
        Ok(r) => r,
        Err(e) => {
            crate::log_warn!("network", "Portal页面请求失败: {}", e);
            return None;
        }
    };

    let html = match resp.text() {
        Ok(t) => t,
        Err(e) => {
            crate::log_warn!("network", "Portal页面读取失败: {}", e);
            return None;
        }
    };

    crate::log_debug!("network", "Portal页面响应长度: {}", html.len());
    if html.contains("您已经成功登录") || html.contains("已登录") || html.contains("成功登录") {
        Some(true)
    } else if html.contains("用户登录") || html.contains("请输入") || html.contains("password") {
        Some(false)
    } else {
        crate::log_info!("network", "Portal页面无法判断登录状态: {}", &html[..html.len().min(300)]);
        None
    }
}
