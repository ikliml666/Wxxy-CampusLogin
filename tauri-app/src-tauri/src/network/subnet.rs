//! 子网计算、SSID 处理、网关可达性检查
//!
//! 从 `adapter.rs` 迁移，集中网络诊断相关工具。

use crate::network::discovery::new_command;

pub fn get_wireless_ssid() -> Result<Option<String>, String> {
    let output = new_command("netsh")
        .args(["wlan", "show", "interfaces"])
        .output()
        .map_err(|e| format!("获取无线网络信息失败: {e}"))?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("SSID") && !trimmed.starts_with("BSSID") {
            let after = trimmed;
            if let Some(colon) = after.find(':') {
                let ssid = after[colon + 1..].trim();
                if !ssid.is_empty()
                    && !ssid.contains("不在")
                    && !ssid.contains("not connected")
                    && !ssid.contains("disconnected")
                {
                    return Ok(Some(ssid.to_string()));
                }
            }
        }
    }

    Ok(None)
}

pub fn get_wired_network_profile() -> Result<Option<String>, String> {
    let output = new_command("netsh")
        .args(["lan", "show", "interfaces"])
        .output()
        .map_err(|e| format!("获取有线网络信息失败: {e}"))?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        let is_profile_line = trimmed.to_lowercase().contains("profile")
            || trimmed.contains("配置文件")
            || trimmed.contains("設定檔");
        if is_profile_line {
            if let Some(colon) = trimmed.find(':') {
                let name = trimmed[colon + 1..].trim();
                if !name.is_empty() {
                    return Ok(Some(name.to_string()));
                }
            }
        }
    }

    Ok(None)
}

pub fn check_gateway_reachable(gateway: &str) -> bool {
    check_gateway_reachable_from(gateway, None)
}

pub fn check_gateway_reachable_from(gateway: &str, source_ip: Option<&str>) -> bool {
    if gateway.is_empty() {
        return false;
    }
    let mut cmd = new_command("ping");
    cmd.args(["-n", "1", "-w", "2000"]);
    if let Some(src) = source_ip {
        if !src.is_empty() && src.parse::<std::net::IpAddr>().is_ok() {
            cmd.args(["-S", src]);
        }
    }
    cmd.arg(gateway);
    match cmd.output() {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

/// 判断 IP 与网关是否在同一 /18 子网（掩码 255.255.192.0）
pub fn is_same_subnet_18(ip_str: &str, gateway_str: &str) -> bool {
    let ip: u32 = match ip_str.parse::<std::net::Ipv4Addr>() {
        Ok(addr) => u32::from(addr),
        Err(_) => return false,
    };
    let gw: u32 = match gateway_str.parse::<std::net::Ipv4Addr>() {
        Ok(addr) => u32::from(addr),
        Err(_) => return false,
    };
    let mask: u32 = 0xFFFF_C000;
    (ip & mask) == (gw & mask)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_same_subnet_18_same_subnet_returns_true() {
        // 10.2.0.0/18 范围：10.2.0.0 - 10.2.63.255
        assert!(is_same_subnet_18("10.2.0.1", "10.2.63.254"));
        assert!(is_same_subnet_18("10.2.0.0", "10.2.0.1"));
        assert!(is_same_subnet_18("10.2.63.255", "10.2.0.0"));
    }

    #[test]
    fn is_same_subnet_18_different_subnet_returns_false() {
        // 10.2.0.0/18 与 10.2.64.0/18 不同子网
        assert!(!is_same_subnet_18("10.2.0.1", "10.2.64.1"));
        // 10.2.63.255 与 10.2.64.0 跨子网边界
        assert!(!is_same_subnet_18("10.2.63.255", "10.2.64.0"));
        // 完全不同的网段
        assert!(!is_same_subnet_18("192.168.1.1", "10.2.0.1"));
    }

    #[test]
    fn is_same_subnet_18_invalid_ip_returns_false() {
        assert!(!is_same_subnet_18("invalid", "10.2.0.1"));
        assert!(!is_same_subnet_18("", "10.2.0.1"));
        assert!(!is_same_subnet_18("10.2.0", "10.2.0.1"));
    }

    #[test]
    fn is_same_subnet_18_invalid_gateway_returns_false() {
        assert!(!is_same_subnet_18("10.2.0.1", "invalid"));
        assert!(!is_same_subnet_18("10.2.0.1", ""));
        assert!(!is_same_subnet_18("10.2.0.1", "10.2.0"));
    }
}
