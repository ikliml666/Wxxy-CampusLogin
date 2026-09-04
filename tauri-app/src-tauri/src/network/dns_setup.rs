//! DNS+DoH 一键设置（管理员 / 提权 helper 共用）
//!
//! 从 commands/network_cmd.rs 的 setup_dns_doh 管理员分支抽出，
//! 供主进程（管理员路径）与 helper 子进程（提权后，见 crate::helper）复用，
//! 避免两处逻辑漂移。管理员路径与 helper 路径行为一致：
//! 按调用方传入的目标适配器名单（resolve 后的主/副适配器）→ WiFi 走配置文件级、
//! 有线走接口级 Win32 设置 → 注册全局 DoH 服务器（netsh dns add encryption）→ 清 DNS 缓存。

#[cfg(target_os = "windows")]
/// `targets` 为操作范围白名单（resolve 后的主/副适配器名）：只对这些适配器
/// 设置 DNS/DoH，不再触碰系统里其他活跃适配器。全局 DoH 注册与 flushdns 不受名单限制。
pub fn setup_dns_doh_admin(targets: &[String]) -> serde_json::Value {
    use crate::network::{get_adapters_force, is_blacklisted, Adapter};
    use crate::platform::dns_config;

    let adapters = get_adapters_force().unwrap_or_default();
    let active: Vec<&Adapter> = adapters
        .iter()
        .filter(|a| !a.ip.is_empty() && !is_blacklisted(&a.name) && targets.iter().any(|t| t == &a.name))
        .collect();

    if active.is_empty() {
        return serde_json::json!({
            "success": false,
            "message": "未找到目标网络适配器（主/副适配器均无活跃连接）".to_string(),
        });
    }

    let mut api_success: Vec<String> = Vec::new();
    let mut api_fail: Vec<String> = Vec::new();

    for adapter in &active {
        // IPv4 + IPv6 混合写入 NameServer（逗号分隔，SetInterfaceDnsSettings 支持双栈列表）：
        // 阿里 v4+v6 + 腾讯 v4+v6
        let dns_list: Vec<&str> = vec![
            dns_config::PRIMARY_DNS,
            dns_config::SECONDARY_DNS,
            dns_config::PRIMARY_DNS_V6,
            dns_config::SECONDARY_DNS_V6,
        ];
        let doh_list: Vec<(&str, &str)> = dns_config::DOH_SERVERS.to_vec();

        // WiFi 适配器：先清除适配器级 DNS，再设置配置文件级 DNS
        // 有线适配器：保持适配器级 DNS
        if adapter.wireless {
            if let Err(e) = dns_config::clear_adapter_dns_via_api(&adapter.guid) {
                crate::log_warn!("dns", "清除适配器级DNS失败: {} - {}", adapter.name, e);
            }
            match dns_config::set_profile_dns_via_api(&adapter.guid, &dns_list, &doh_list) {
                Ok(()) => {
                    crate::log_info!("dns", "配置文件级DNS+DoH设置成功: {}", adapter.name);
                    api_success.push(adapter.name.clone());
                }
                Err(e) => {
                    crate::log_warn!(
                        "dns",
                        "配置文件级DNS设置失败: {} - {}, 降级到适配器级",
                        adapter.name,
                        e
                    );
                    match dns_config::set_dns_via_api(&adapter.guid, &dns_list, &doh_list) {
                        Ok(()) => {
                            crate::log_info!("dns", "降级适配器级DNS+DoH成功: {}", adapter.name);
                            api_success.push(adapter.name.clone());
                        }
                        Err(e2) => {
                            crate::log_warn!("dns", "适配器级DNS也失败: {} - {}", adapter.name, e2);
                            api_fail.push(format!("{}: {}", adapter.name, e2));
                        }
                    }
                }
            }
        } else {
            match dns_config::set_dns_via_api(&adapter.guid, &dns_list, &doh_list) {
                Ok(()) => {
                    crate::log_info!("dns", "Win32 API设置DNS+DoH成功: {}", adapter.name);
                    api_success.push(adapter.name.clone());
                }
                Err(e) => {
                    crate::log_warn!("dns", "Win32 API设置DNS失败: {} - {}", adapter.name, e);
                    api_fail.push(format!("{}: {}", adapter.name, e));
                }
            }
        }
    }

    // 注册全局 DoH 服务器（对齐原 PowerShell/cmd 路径行为）
    for (ip, template) in dns_config::DOH_SERVERS {
        let _ = crate::network::discovery::new_command("netsh")
            .args([
                "dns",
                "add",
                "encryption",
                &format!("server={ip}"),
                &format!("dohtemplate={template}"),
                "autoupgrade=yes",
                "udpfallback=yes",
            ])
            .output();
    }

    let _ = crate::network::discovery::new_command("ipconfig")
        .args(["/flushdns"])
        .output();

    if !api_success.is_empty() {
        let mut parts = Vec::new();
        parts.push(format!(
            "已为 {} 设置DNS({}+{})并启用DoH",
            api_success.join("、"),
            dns_config::PRIMARY_DNS,
            dns_config::SECONDARY_DNS
        ));
        if !api_fail.is_empty() {
            parts.push(format!("{}个适配器设置失败", api_fail.len()));
        }
        return serde_json::json!({
            "success": api_fail.is_empty(),
            "message": parts.join("，"),
            "dnsSuccess": api_success,
            "dnsFailed": api_fail,
            "dohAdded": dns_config::DOH_SERVERS.iter().map(|(ip, _)| ip.to_string()).collect::<Vec<_>>(),
            "dohFailed": [],
        });
    }

    serde_json::json!({
        "success": false,
        "message": "设置DNS失败".to_string(),
        "dnsFailed": api_fail,
    })
}

#[cfg(not(target_os = "windows"))]
pub fn setup_dns_doh_admin(_targets: &[String]) -> serde_json::Value {
    serde_json::json!({ "success": false, "message": "仅支持Windows".to_string() })
}
