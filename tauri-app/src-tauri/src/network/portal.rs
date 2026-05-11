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

pub fn check_portal_full(adapter_ip: &str, adapter_name: Option<&str>) -> Result<PortalStatus, String> {
    {
        let cache_arc = NET_CACHE.portal.load();
        if let Some(entry) = cache_arc.as_ref() {
            if entry.adapter_ip == adapter_ip && entry.time.elapsed().as_secs() < 2 {
                return Ok(entry.status.clone());
            }
        }
    }

    let lock = NET_CACHE.portal_lock.lock();

    {
        let cache_arc = NET_CACHE.portal.load();
        if let Some(entry) = cache_arc.as_ref() {
            if entry.adapter_ip == adapter_ip && entry.time.elapsed().as_secs() < 2 {
                return Ok(entry.status.clone());
            }
        }
    }

    let result = check_portal_full_inner(adapter_ip, adapter_name);

    match result {
        Ok(status) => {
            NET_CACHE.portal.store(Arc::new(Some(super::cache::PortalCacheEntry {
                time: std::time::Instant::now(),
                status: status.clone(),
                adapter_ip: adapter_ip.to_string(),
            })));
            drop(lock);
            Ok(status)
        }
        Err(e) => {
            drop(lock);
            Err(e)
        }
    }
}

fn check_portal_full_inner(adapter_ip: &str, _adapter_name: Option<&str>) -> Result<PortalStatus, String> {
    let portal_url = NET_CACHE.portal_url.load().clone();
    let portal_host = url::Url::parse(&portal_url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .unwrap_or_default();
    let portal_host_check = if portal_host.is_empty() { "___NEVER_MATCH___" } else { &portal_host };
    let local_addr = if !adapter_ip.is_empty() {
        adapter_ip.parse::<std::net::IpAddr>().ok()
    } else {
        None
    };

    let client = create_safe_http_client(std::time::Duration::from_secs(6), local_addr)?;
    let client2 = client.clone();

    let (baidu_result, portal_result) = std::thread::scope(|s| {
        let baidu = s.spawn(|| {
            client.get("http://www.baidu.com/").timeout(std::time::Duration::from_secs(2)).send().map_err(|e| e.to_string())
        });
        let url = format!("{}/", portal_url.trim_end_matches('/'));
        let portal = s.spawn(move || {
            client2.get(&url).timeout(std::time::Duration::from_secs(5)).send().map_err(|e| e.to_string())
        });
        (baidu.join().unwrap_or_else(|_| Err("线程错误".to_string())), portal.join().unwrap_or_else(|_| Err("线程错误".to_string())))
    });

    if let Ok(mut resp) = baidu_result {
        let final_url = resp.url().to_string();
        let is_portal = final_url.contains(portal_host_check)
            || final_url.contains("eportal")
            || final_url.contains("portal/login");

        let mut body = String::new();
        let mut limited = (&mut resp).take(4096);
        let _ = limited.read_to_string(&mut body);
        let _ = std::io::copy(&mut resp, &mut std::io::sink());
        if !is_portal {
            if !body.contains("eportal") && !body.contains("dr1003") && !body.contains("portal/login") {
                let label = "已在线".to_string();
                return Ok(PortalStatus {
                    reachable: true,
                    login_available: false,
                    online: true,
                    message: label,
                    data_length: body.len(),
                });
            }
        }
    } else if let Err(e) = baidu_result {
        crate::log_debug!("network", "百度检测请求失败: {}", e);
    }

    let mut resp = match portal_result {
        Ok(r) => r,
        Err(e) => {
            crate::log_debug!("network", "Portal检测请求失败: {}", e);
            return Ok(PortalStatus {
                reachable: false,
                login_available: false,
                online: false,
                message: "未登录".to_string(),
                data_length: 0,
            });
        }
    };

    let mut data = String::new();
    let mut limited = (&mut resp).take(MAX_RESPONSE_SIZE as u64);
    let _ = limited.read_to_string(&mut data);
    let _ = std::io::copy(&mut resp, &mut std::io::sink());
    let total_length = data.len();

    let reachable = total_length > 0;
    let login_available = data.contains("eportal") || data.contains("login")
        || data.contains("portal") || data.contains("dr1003");

    let online = (data.contains("uid='") && data.contains("oltime=") && !data.contains("uid=''"))
        || data.contains("已经在线")
        || data.contains("ret_code\":2")
        || (data.contains("dr1003") && data.contains("\"result\":0") && data.contains("ret_code"));

    let label = if online {
        "已在线".to_string()
    } else if login_available {
        "未登录".to_string()
    } else {
        "未登录".to_string()
    };

    Ok(PortalStatus {
        reachable,
        login_available,
        online,
        message: label,
        data_length: total_length,
    })
}
