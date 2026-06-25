use serde::Serialize;
use crate::network::Adapter;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionCampusStatus {
    pub on_campus: bool,
    pub name: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CampusCheckResult {
    pub wifi: Option<ConnectionCampusStatus>,
    pub wired: Option<ConnectionCampusStatus>,
    pub on_campus: bool,
    pub current_ssid: Option<String>,
    pub message: String,
}

pub(super) fn adapter_campus_status<'a>(adapter_name: &str, adapters: &'a [Adapter], campus_result: &'a CampusCheckResult) -> Option<&'a ConnectionCampusStatus> {
    let info = crate::network::find_by_name(adapters, adapter_name)?;
    let is_wireless = info.wireless;
    let status = if is_wireless { &campus_result.wifi } else { &campus_result.wired };
    status.as_ref()
}

pub(super) fn adapter_campus_message(adapter_name: &str, adapters: &[Adapter], campus_result: &CampusCheckResult) -> Option<String> {
    adapter_campus_status(adapter_name, adapters, campus_result).map(|s| s.message.clone())
}

pub fn check_campus_network(config: &crate::config::model::Config, adapters: &[crate::network::Adapter]) -> CampusCheckResult {
    crate::log_info!("campus", "[校园网检测] enable_network_name_check={}, required_network_name='{}', campus_gateway='{}'",
        config.enable_network_name_check, config.required_network_name, config.campus_gateway);

    if !config.enable_network_name_check {
        let gateway_ok = crate::network::check_gateway_reachable(&config.campus_gateway);
        crate::log_info!("campus", "[校园网检测] 名称检查已禁用，网关可达性: {}", gateway_ok);
        let msg = if gateway_ok {
            format!("网关{}可达", config.campus_gateway)
        } else {
            "未连接到校园网络(网关不可达)".to_string()
        };
        return CampusCheckResult {
            wifi: None,
            wired: None,
            on_campus: gateway_ok,
            current_ssid: None,
            message: msg,
        };
    }

    let required_name = &config.required_network_name;
    let campus_gw = &config.campus_gateway;

    let wifi_ssid = crate::network::get_wireless_ssid().ok().flatten();
    let wired_profile = crate::network::get_wired_network_profile().ok().flatten();

    crate::log_info!("campus", "[校园网检测] wifi_ssid={:?}, wired_profile={:?}", wifi_ssid, wired_profile);

    let mut gateway_checked: Option<bool> = None;
    let check_gateway = |gw: &str, cache: &mut Option<bool>| -> bool {
        if let Some(cached) = cache {
            crate::log_info!("campus", "[校园网检测] 使用缓存的网关可达性: {}", cached);
            *cached
        } else {
            let ok = crate::network::check_gateway_reachable(gw);
            *cache = Some(ok);
            crate::log_info!("campus", "[校园网检测] 网关可达性检查: gw={}, reachable={}", gw, ok);
            ok
        }
    };

    let wifi_status = {
        let wifi_adapters: Vec<&crate::network::Adapter> = adapters.iter().filter(|a| a.wireless).collect();
        match &wifi_ssid {
            Some(ssid) if ssid.eq_ignore_ascii_case(required_name) => {
                crate::log_info!("campus", "[校园网检测] ✅ WiFi名称匹配: '{}'", ssid);
                Some(ConnectionCampusStatus {
                    on_campus: true,
                    name: Some(ssid.clone()),
                    message: format!("已连接到校园WiFi({ssid})"),
                })
            }
            Some(ssid) => {
                crate::log_info!("campus", "[校园网检测] WiFi SSID '{}' 不匹配校园网名称'{}'", ssid, required_name);
                let mut found = false;
                let mut msg = String::new();
                for a in &wifi_adapters {
                    if !a.ip.is_empty() {
                        let same_subnet = crate::network::is_same_subnet_18(&a.ip, campus_gw);
                        crate::log_info!("campus", "[校园网检测] WiFi SSID不匹配，尝试子网检查: adapter={}, ip={}, /18匹配={}", a.name, a.ip, same_subnet);
                        if same_subnet {
                            found = true;
                            msg = format!("WiFi\"{ssid}\"名称不匹配但与网关在同一/18网段");
                            break;
                        }
                    }
                }
                if !found {
                    let gateway_ok = check_gateway(campus_gw, &mut gateway_checked);
                    if gateway_ok {
                        found = true;
                        msg = format!("WiFi\"{ssid}\"名称不匹配但网关{campus_gw}可达");
                    }
                }
                if found {
                    Some(ConnectionCampusStatus { on_campus: true, name: Some(ssid.clone()), message: msg })
                } else {
                    Some(ConnectionCampusStatus {
                        on_campus: false,
                        name: Some(ssid.clone()),
                        message: format!("当前WiFi\"{ssid}\"非校园网络"),
                    })
                }
            }
            None => {
                if wifi_adapters.is_empty() {
                    None
                } else {
                    let mut found = false;
                    let mut msg = String::new();
                    for a in &wifi_adapters {
                        if !a.ip.is_empty() {
                            let same_subnet = crate::network::is_same_subnet_18(&a.ip, campus_gw);
                            crate::log_info!("campus", "[校园网检测] WiFi子网检查: adapter={}, ip={}, /18匹配={}", a.name, a.ip, same_subnet);
                            if same_subnet {
                                found = true;
                                msg = format!("WiFi已连接校园网({}与网关在同一/18网段)", a.ip);
                                break;
                            }
                        }
                    }
                    // 仅当至少一个 WiFi 网卡拥有合法 IP 时，才信任网关可达性
                    // 否则可达性可能来自其他类型网卡（如有线），错误归因到 WiFi
                    if !found && wifi_adapters.iter().any(|a| !a.ip.is_empty()) {
                        let gateway_ok = check_gateway(campus_gw, &mut gateway_checked);
                        if gateway_ok {
                            found = true;
                            msg = format!("WiFi通过网关{campus_gw}连接校园网");
                        }
                    }
                    if found {
                        Some(ConnectionCampusStatus { on_campus: true, name: None, message: msg })
                    } else {
                        Some(ConnectionCampusStatus { on_campus: false, name: None, message: "WiFi未连接校园网".to_string() })
                    }
                }
            }
        }
    };

    let wired_status = {
        let wired_adapters: Vec<&crate::network::Adapter> = adapters.iter().filter(|a| !a.wireless).collect();
        if wired_adapters.is_empty() {
            None
        } else {
            match &wired_profile {
                Some(profile) if profile.eq_ignore_ascii_case(required_name) => {
                    crate::log_info!("campus", "[校园网检测] ✅ 有线名称匹配: '{}'", profile);
                    Some(ConnectionCampusStatus {
                        on_campus: true,
                        name: Some(profile.clone()),
                        message: format!("已连接到校园有线网络({profile})"),
                    })
                }
                _ => {
                    let mut found = false;
                    let mut msg = String::new();
                    for a in &wired_adapters {
                        if !a.ip.is_empty() {
                            let same_subnet = crate::network::is_same_subnet_18(&a.ip, campus_gw);
                            crate::log_info!("campus", "[校园网检测] 有线子网检查: adapter={}, ip={}, /18匹配={}", a.name, a.ip, same_subnet);
                            if same_subnet {
                                found = true;
                                msg = format!("有线已连接校园网({}与网关在同一/18网段)", a.ip);
                                break;
                            }
                        }
                    }
                    // 仅当至少一个有线网卡拥有合法 IP 时，才信任网关可达性
                    // 否则可达性可能来自其他类型网卡（如 WiFi），错误归因到有线
                    if !found && wired_adapters.iter().any(|a| !a.ip.is_empty()) {
                        let gateway_ok = check_gateway(campus_gw, &mut gateway_checked);
                        if gateway_ok {
                            found = true;
                            msg = format!("有线通过网关{campus_gw}连接校园网");
                        }
                    }
                    if found {
                        Some(ConnectionCampusStatus { on_campus: true, name: wired_profile.clone(), message: msg })
                    } else {
                        let fail_msg = match &wired_profile {
                            Some(p) => format!("当前有线网络\"{p}\"非校园网络"),
                            None => "有线网络未连接校园网".to_string(),
                        };
                        Some(ConnectionCampusStatus { on_campus: false, name: wired_profile.clone(), message: fail_msg })
                    }
                }
            }
        }
    };

    let on_campus = wifi_status.as_ref().map(|s| s.on_campus).unwrap_or(false)
        || wired_status.as_ref().map(|s| s.on_campus).unwrap_or(false);

    let message = if on_campus {
        let mut parts = Vec::new();
        if let Some(ref ws) = wifi_status {
            if ws.on_campus { parts.push(ws.message.clone()); }
        }
        if let Some(ref ws) = wired_status {
            if ws.on_campus { parts.push(ws.message.clone()); }
        }
        if parts.is_empty() { "已连接校园网".to_string() } else { parts.join("；") }
    } else {
        let mut parts = Vec::new();
        if let Some(ref ws) = wifi_status {
            if !ws.on_campus { parts.push(ws.message.clone()); }
        }
        if let Some(ref ws) = wired_status {
            if !ws.on_campus { parts.push(ws.message.clone()); }
        }
        if parts.is_empty() { "未连接到校园网络".to_string() } else { parts.join("；") }
    };

    crate::log_info!("campus", "[校园网检测] 结果: on_campus={}, wifi={:?}, wired={:?}, message={}",
        on_campus, wifi_status.as_ref().map(|s| s.on_campus), wired_status.as_ref().map(|s| s.on_campus), message);

    CampusCheckResult {
        wifi: wifi_status,
        wired: wired_status,
        on_campus,
        current_ssid: wifi_ssid.or(wired_profile),
        message,
    }
}
