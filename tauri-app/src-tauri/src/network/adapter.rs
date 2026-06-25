//! 适配器选择、子网/SSID 工具
//!
//! 本模块保留 adapter.rs 的历史职责（适配器选择、子网/SSID），
//! 适配器发现已迁移到 `network::discovery`，缓存访问已迁移到 `network::adapter_cache`，
//! DHCP/MAC 重置已迁移到 `network::dhcp`。

use std::sync::atomic::AtomicBool;
use tauri::AppHandle;
use crate::config::model::Config;
use crate::infra::events::EventBus;

// 从 discovery re-export 公共类型与函数，保持外部调用方不变
pub use crate::network::discovery::{
    Adapter, AdapterDetail, AdapterStatus, DisabledAdapter,
    is_blacklisted, new_command,
};

// 从 adapter_cache re-export 缓存访问 API，保持外部调用方不变
pub use crate::network::adapter_cache::{
    get_adapters_cached, get_adapters_cached_async, get_adapters_force,
    get_disabled_adapters_cached, get_adapter_details_cached,
    get_all_adapters_force, enable_adapter, wait_for_adapter,
    validate_adapter_name, poll_adapter_ip_quick,
};

// 从 dhcp re-export DHCP/MAC 相关函数，保持外部调用方不变
pub use crate::network::dhcp::{
    dhcp_renew, dhcp_release, dhcp_renew_wired_only,
    dhcp_release_renew_all, dhcp_release_renew_single,
    set_mac_via_registry, remove_mac_from_registry,
    netsh_disable, netsh_enable,
    poll_ip_change, poll_adapter_has_ip,
    escape_ps_single_quote,
};

pub fn resolve_adapter_names(adapters: &[Adapter], config: &crate::config::Config) -> (String, String) {
    // 自动检测：优先选有线网卡，其次任意有 IP 的网卡，最后任意第一个
    let auto_detect_a1 = || -> String {
        adapters.iter()
            .find(|a| !a.wireless && !a.ip.is_empty())
            .or_else(|| adapters.iter().find(|a| !a.ip.is_empty()))
            .or_else(|| adapters.first())
            .map(|a| a.name.clone())
            .unwrap_or_default()
    };

    let adapter1 = if config.adapter1.is_empty() || config.adapter1 == crate::config::model::AUTO_DETECT_ADAPTER {
        auto_detect_a1()
    } else if adapters.iter().any(|a| a.name == config.adapter1) {
        config.adapter1.clone()
    } else {
        crate::log_warn!(
            "network",
            "配置中的 adapter1 '{}' 不在当前可见适配器列表中，降级到自动检测",
            config.adapter1
        );
        auto_detect_a1()
    };

    let adapter2 = if config.dual_adapter {
        if config.adapter2.is_empty() || config.adapter2 == crate::config::model::AUTO_DETECT_ADAPTER {
            adapters.iter()
                .find(|a| a.name != adapter1 && !a.wireless && !a.ip.is_empty())
                .or_else(|| adapters.iter().find(|a| a.name != adapter1 && !a.ip.is_empty()))
                .or_else(|| adapters.iter().find(|a| a.name != adapter1))
                .map(|a| a.name.clone())
                .unwrap_or_default()
        } else if adapters.iter().any(|a| a.name == config.adapter2) {
            config.adapter2.clone()
        } else {
            crate::log_warn!(
                "network",
                "配置中的 adapter2 '{}' 不在当前可见适配器列表中，降级到自动检测",
                config.adapter2
            );
            adapters.iter()
                .find(|a| a.name != adapter1 && !a.wireless && !a.ip.is_empty())
                .or_else(|| adapters.iter().find(|a| a.name != adapter1 && !a.ip.is_empty()))
                .or_else(|| adapters.iter().find(|a| a.name != adapter1))
                .map(|a| a.name.clone())
                .unwrap_or_default()
        }
    } else {
        String::new()
    };

    (adapter1, adapter2)
}

pub fn select_adapter(adapters: &[Adapter], config: &crate::config::Config) -> (String, String) {
    if adapters.is_empty() { return (String::new(), String::new()); }

    if !config.adapter1.is_empty() && config.adapter1 != "自动检测" {
        if let Some(a) = adapters.iter().find(|a| a.name == config.adapter1 && !a.ip.is_empty()) {
            return (a.ip.clone(), a.name.clone());
        }
    }

    if let Some(wired) = adapters.iter().find(|a| !a.ip.is_empty() && !a.wireless) {
        return (wired.ip.clone(), wired.name.clone());
    }

    if let Some(with_ip) = adapters.iter().find(|a| !a.ip.is_empty()) {
        return (with_ip.ip.clone(), with_ip.name.clone());
    }

    (String::new(), String::new())
}

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

pub fn ensure_ethernet_ip_for_login(
    app_handle: &AppHandle,
    adapters: &[Adapter],
    config: &Config,
    is_quitting: &AtomicBool,
) {
    let (a1_name, a2_name) = resolve_adapter_names(adapters, config);

    let candidates: Vec<String> = [&a1_name, &a2_name]
        .iter()
        .filter_map(|name| {
            if name.is_empty() {
                return None;
            }
            let adapter = adapters.iter().find(|a| a.name == **name)?;
            if !adapter.wireless && adapter.ip.is_empty() {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect();

    if candidates.is_empty() {
        return;
    }

    for name in &candidates {
        if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
            return;
        }

        let event_bus = EventBus::new(app_handle);
        let _ = event_bus.emit_login_log(
            &format!("检测到以太网 {name} 已连接但未获取IP，正在尝试DHCP续租..."),
            "info",
        );

        let child = new_command("ipconfig")
            .args(["/renew", name])
            .spawn();

        let got_ip = poll_adapter_ip_quick(name, 5000, is_quitting);

        if let Ok(mut c) = child {
            let _ = c.kill();
            let _ = c.wait();
        }

        if got_ip {
            let ip = get_adapters_force()
                .ok()
                .and_then(|list| list.iter().find(|a| a.name == *name).map(|a| a.ip.clone()))
                .unwrap_or_default();
            let event_bus = EventBus::new(app_handle);
            let _ = event_bus.emit_login_log(
                &format!("以太网 {name} DHCP续租成功，IP: {ip}"),
                "success",
            );
        } else {
            let event_bus = EventBus::new(app_handle);
            let _ = event_bus.emit_login_log(
                &format!("以太网 {name} DHCP续租超时仍未获得IP，跳过该网卡"),
                "warning",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_config(adapter1: &str, dual: bool, adapter2: &str) -> crate::config::Config {
        crate::config::Config {
            user: String::new(),
            password: String::new(),
            operator: String::new(),
            adapter1: adapter1.to_string(),
            adapter2: adapter2.to_string(),
            dual_adapter: dual,
            auto_login_on_start: false,
            auto_exit_after_login: false,
            minimize_to_tray: false,
            hidden_start: false,
            auto_launch: false,
            enable_background_check: false,
            background_check_interval: 60,
            auto_login_on_preparation: false,
            auto_exit_on_online: false,
            theme_mode: "light".to_string(),
            enable_notification: false,
            active_account: String::new(),
            enable_latency_test: false,
            latency_test_interval: 300,
            custom_theme_color: String::new(),
            default_panel: "login".to_string(),
            enable_network_quality: false,
            skip_ttfb_in_latency: true,
            skip_content_in_latency: true,
            portal_url: String::new(),
            fixed_gateway: String::new(),
            required_network_name: String::new(),
            enable_network_name_check: false,
            campus_gateway: String::new(),
            campus_exit_on_fail: true,
            campus_check_start_minutes: 480,
            log_retention_days: 7,
            config_version: 2,
        }
    }

    fn make_test_adapter(name: &str, wireless: bool, ip: &str) -> Adapter {
        let status = if ip.is_empty() {
            AdapterStatus::EnabledNoIp
        } else {
            AdapterStatus::Connected
        };
        Adapter {
            name: name.to_string(),
            ip: ip.to_string(),
            wireless,
            guid: format!("{{{name}}}"),
            mac: String::new(),
            if_index: 1,
            status,
        }
    }

    #[test]
    fn resolve_adapter_names_falls_back_when_config_name_missing() {
        let adapters = vec![
            make_test_adapter("以太网", false, "10.2.0.1"),
            make_test_adapter("WLAN", true, ""),
        ];
        let config = make_test_config("本地连接", false, "");
        let (a1, a2) = resolve_adapter_names(&adapters, &config);
        assert_eq!(a1, "以太网");
        assert_eq!(a2, "");
    }

    #[test]
    fn resolve_adapter_names_uses_config_when_present() {
        let adapters = vec![
            make_test_adapter("以太网", false, "10.2.0.1"),
            make_test_adapter("WLAN", true, "10.2.0.2"),
        ];
        let config = make_test_config("WLAN", true, "以太网");
        let (a1, a2) = resolve_adapter_names(&adapters, &config);
        assert_eq!(a1, "WLAN");
        assert_eq!(a2, "以太网");
    }

    #[test]
    fn resolve_adapter_names_auto_detect_prefers_wired_with_ip() {
        let adapters = vec![
            make_test_adapter("WLAN", true, "10.2.0.2"),
            make_test_adapter("以太网", false, "10.2.0.1"),
        ];
        let config = make_test_config("自动检测", false, "");
        let (a1, _) = resolve_adapter_names(&adapters, &config);
        assert_eq!(a1, "以太网");
    }
}
