//! 安卓侧校园网探针:本机 IPv4 枚举 + /18 子网判定(复用桌面纯函数)+ Portal TCP 可达。
//! ICMP 原始 socket 在安卓被 SELinux 禁止(surge-ping 不可用),网关可达改用 Portal TCP 可达表达。

use network_interface::NetworkInterfaceConfig;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

const PORTAL_PORT: u16 = 80;
const TCP_TIMEOUT: Duration = Duration::from_secs(3);

/// 从网卡枚举结果中选出登录请求的源 IPv4。
/// 规则:排除蜂窝接口(绝不让登录源指向移动数据)、link-local/回环/未指定;
/// wlan0 优先,无 wlan0 时取其他非蜂窝接口(如 eth0)。
pub fn pick_campus_source_ip(interfaces: &[(String, IpAddr)]) -> Option<Ipv4Addr> {
    // 蜂窝数据接口主流命名:rmnet*(高通系)/ ccmni*(MTK 系)
    let is_cellular = |name: &str| name.starts_with("rmnet") || name.starts_with("ccmni");
    let valid: Vec<(String, Ipv4Addr)> = interfaces
        .iter()
        .filter_map(|(name, ip)| match ip {
            IpAddr::V4(v4)
                if !v4.is_link_local()
                    && !v4.is_loopback()
                    && !v4.is_unspecified()
                    && !is_cellular(name) =>
            {
                Some((name.clone(), *v4))
            }
            _ => None,
        })
        .collect();
    valid
        .iter()
        .find(|(name, _)| name == "wlan0")
        .or_else(|| valid.first())
        .map(|(_, ip)| *ip)
}

/// Portal 服务器 TCP 可达性(替代 ICMP ping;portal 可达即视为校园网连通)。
/// 连接被拒/超时 = 不通(返回 false),非调用错误。
pub async fn portal_reachable(host: &str, port: u16, timeout: Duration) -> bool {
    matches!(
        tokio::time::timeout(timeout, tokio::net::TcpStream::connect((host, port))).await,
        Ok(Ok(_))
    )
}

/// 一次校园网探测结果(检测命令与 check_campus_status 共用)
pub struct CampusProbe {
    pub source: Option<Ipv4Addr>,
    pub on_campus_by_subnet: bool,
    pub portal_ok: bool,
    pub on_campus: bool,
}

pub async fn probe_campus(campus_gateway: &str, portal_url: &str) -> Result<CampusProbe, String> {
    let interfaces = network_interface::NetworkInterface::show()
        .map_err(|e| format!("枚举网卡失败: {e}"))?;
    let flat: Vec<(String, IpAddr)> = interfaces
        .iter()
        .flat_map(|i| {
            i.addr
                .iter()
                .filter_map(|a| match a {
                    network_interface::Addr::V4(v4) => Some(v4.ip),
                    _ => None,
                })
                .map(move |ip| (i.name.clone(), IpAddr::V4(ip)))
        })
        .collect();
    let source = pick_campus_source_ip(&flat);

    let on_campus_by_subnet = source
        .map(|ip| campus_login_lib::network::is_same_subnet_18(&ip.to_string(), campus_gateway))
        .unwrap_or(false);

    let portal_host = portal_host_of(portal_url);
    let portal_ok = portal_reachable(&portal_host, PORTAL_PORT, TCP_TIMEOUT).await;

    // 对齐桌面 campus_check 三层判定的安卓版(2026-09-09 真机反馈:能到达网关
    // 却被判非校园网)。桌面三层=SSID→/18 子网→网关 ICMP;安卓无 SSID 通道且
    // 非 root 无 ICMP,兜底层改 TCP:
    // ①网关 TCP(campus_gateway 必为内网地址,跨 /18 的 AP 区段靠它救回);
    // ②Portal TCP(配置公网 portal 域名时家宽也可能连通,属已知边界——
    //   建议保持 portal_url 为内网地址;放最后作为网关误配时的最后兜底)
    let gateway_ok = if on_campus_by_subnet {
        true
    } else {
        portal_reachable(campus_gateway, PORTAL_PORT, TCP_TIMEOUT).await
    };
    let on_campus = on_campus_by_subnet || gateway_ok || portal_ok;

    Ok(CampusProbe {
        source,
        on_campus_by_subnet,
        portal_ok,
        on_campus,
    })
}

/// "http://10.1.99.100" -> "10.1.99.100"(去掉 scheme/端口/路径;解析失败回退原串)
pub fn portal_host_of(url: &str) -> String {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let host_port = after_scheme.split('/').next().unwrap_or(after_scheme);
    match host_port.rsplit_once(':') {
        Some((host, port)) if port.chars().all(|c| c.is_ascii_digit()) && !host.is_empty() => {
            host.to_string()
        }
        _ => host_port.to_string(),
    }
}

/// 缓存源 IP 供登录/注销的 adapter_ip 绑定使用
pub fn cache_source_ip(
    state: &tauri::State<'_, crate::android_state::AndroidState>,
    source: Option<Ipv4Addr>,
) {
    if let Ok(mut cached) = state.cached_source_ip.lock() {
        *cached = source;
    }
}

/// 阶段 1 兼容入口:检测卡详情(前端旧 UI 仍在用)
#[tauri::command]
pub async fn detect_campus(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::android_state::AndroidState>,
) -> Result<serde_json::Value, String> {
    let settings = crate::config_state::current_settings(&app).await?;
    let probe = probe_campus(&settings.campus_gateway, &settings.portal_url).await?;
    cache_source_ip(&state, probe.source);

    let detail = match (probe.on_campus_by_subnet, probe.portal_ok) {
        (true, true) => "子网命中且 Portal 可达".to_string(),
        (true, false) => "子网命中但 Portal 不可达".to_string(),
        (false, true) => "已连接校园网(内网 Portal 可达,跨网段接入)".to_string(),
        (false, false) => "未检测到校园网环境".to_string(),
    };

    Ok(serde_json::json!({
        "onCampus": probe.on_campus,
        "sourceIp": probe.source.map(|i| i.to_string()),
        "portalReachable": probe.portal_ok,
        "detail": detail,
    }))
}

/// 桌面同名命令:形状对齐 network_cmd.rs check_campus_status
/// (安卓无 netsh,SSID 取不到,currentSsid 恒空;enableNetworkNameCheck 关闭时跳过 SSID 判定)
#[tauri::command]
pub async fn check_campus_status(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::android_state::AndroidState>,
) -> Result<serde_json::Value, String> {
    let settings = crate::config_state::current_settings(&app).await?;
    let probe = probe_campus(&settings.campus_gateway, &settings.portal_url).await?;
    cache_source_ip(&state, probe.source);

    let campus_message = if probe.on_campus {
        "已连接校园网".to_string()
    } else {
        "未连接校园网".to_string()
    };

    Ok(serde_json::json!({
        "onCampusNetwork": probe.on_campus,
        "currentSsid": "",
        "campusMessage": campus_message,
        "enableNetworkNameCheck": settings.enable_network_name_check,
        "requiredNetworkName": settings.required_network_name,
        "sourceIp": probe.source.map(|i| i.to_string()),
        "portalReachable": probe.portal_ok,
    }))
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn ifc(name: &str, ip: &str) -> (String, IpAddr) {
        (name.to_string(), IpAddr::V4(ip.parse::<Ipv4Addr>().unwrap()))
    }

    #[test]
    fn wlan0_campus_ip_被优先选中() {
        let ifs = vec![
            ifc("rmnet_data0", "10.44.5.6"),
            ifc("wlan0", "10.2.99.31"),
        ];
        let picked = pick_campus_source_ip(&ifs).unwrap();
        assert_eq!(picked.to_string(), "10.2.99.31");
    }

    #[test]
    fn 蜂窝接口永不返回() {
        let ifs = vec![ifc("rmnet_data0", "10.44.5.6")];
        assert!(pick_campus_source_ip(&ifs).is_none());
    }

    #[test]
    fn link_local_地址被排除() {
        let ifs = vec![ifc("wlan0", "169.254.10.2")];
        assert!(pick_campus_source_ip(&ifs).is_none());
    }

    #[test]
    fn 无_wlan0_时取任意合法内网ipv4() {
        let ifs = vec![ifc("eth0", "10.2.120.8")];
        assert!(pick_campus_source_ip(&ifs).is_some());
    }

    #[test]
    fn portal_host_提取() {
        assert_eq!(portal_host_of("http://10.1.99.100"), "10.1.99.100");
        assert_eq!(portal_host_of("http://10.1.99.100:8080/x"), "10.1.99.100");
        assert_eq!(portal_host_of("10.1.99.100"), "10.1.99.100");
        assert_eq!(portal_host_of("http://portal.example.com/"), "portal.example.com");
    }
}
