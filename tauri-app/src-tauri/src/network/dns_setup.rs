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
/// `family` 为优化目标："ipv4" 只写 v4、"ipv6" 只写 v6、"both"（默认）写 v4+v6 混合列表。
pub fn setup_dns_doh_admin(targets: &[String], family: &str) -> serde_json::Value {
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

    // 按 family 决定 NameServer 列表；DoH 绑定由 doh_bindings 按列表实际内容配对
    let dns_list: Vec<&str> = match family {
        "ipv4" => vec![dns_config::PRIMARY_DNS, dns_config::SECONDARY_DNS],
        "ipv6" => vec![dns_config::PRIMARY_DNS_V6, dns_config::SECONDARY_DNS_V6],
        _ => vec![
            dns_config::PRIMARY_DNS,
            dns_config::SECONDARY_DNS,
            dns_config::PRIMARY_DNS_V6,
            dns_config::SECONDARY_DNS_V6,
        ],
    };
    let doh_list: Vec<(&str, &str)> = dns_config::DOH_SERVERS.to_vec();

    let mut api_success: Vec<String> = Vec::new();
    let mut api_fail: Vec<String> = Vec::new();

    for adapter in &active {
        // WiFi 适配器：先清除适配器级 DNS，再设置配置文件级 DNS；
        // 清除失败时接口级残留会覆盖 profile DNS（接口级优先级更高），
        // 此时改走接口级设置而非写 profile
        // 有线适配器：保持适配器级 DNS
        if adapter.wireless {
            match dns_config::clear_adapter_dns_via_api(&adapter.guid) {
                Ok(()) => match dns_config::set_profile_dns_via_api(&adapter.guid, &dns_list, &doh_list) {
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
                },
                Err(e) => {
                    crate::log_warn!("dns", "清除适配器级DNS失败: {} - {}, 改用接口级设置", adapter.name, e);
                    match dns_config::set_dns_via_api(&adapter.guid, &dns_list, &doh_list) {
                        Ok(()) => {
                            crate::log_info!("dns", "接口级DNS+DoH设置成功: {}", adapter.name);
                            api_success.push(adapter.name.clone());
                        }
                        Err(e2) => {
                            crate::log_warn!("dns", "接口级DNS设置失败: {} - {}", adapter.name, e2);
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

    // 注册全局 DoH 服务器（对齐原 PowerShell/cmd 路径行为）。
    // 失败必须如实上报：历史实现 let _ 吞掉且 dohFailed 恒空，DoH 未生效
    // 仍向用户展示"并启用DoH"
    let mut doh_failed: Vec<String> = Vec::new();
    for (ip, template) in dns_config::DOH_SERVERS {
        match crate::network::discovery::new_command("netsh")
            .args([
                "dns",
                "add",
                "encryption",
                &format!("server={ip}"),
                &format!("dohtemplate={template}"),
                "autoupgrade=yes",
                "udpfallback=yes",
            ])
            .output()
        {
            Ok(out) if out.status.success() => {}
            Ok(out) => {
                // 重复添加已注册条目按幂等成功处理（netsh 退出码语义未实测确证，
                // 无论退出码如何，"已存在"都不是失败）
                let stderr_text = crate::platform::console_output::decode_console_bytes(&out.stderr);
                let stdout_text = crate::platform::console_output::decode_console_bytes(&out.stdout);
                let combined = format!("{} {}", stderr_text, stdout_text);
                if combined.contains("已存在") || combined.to_lowercase().contains("already exists") {
                    crate::log_debug!("dns", "DoH加密服务器已注册: {}", ip);
                    continue;
                }
                let detail = stderr_text.trim();
                crate::log_warn!("dns", "注册DoH加密服务器失败: {} - {}（退出码 {:?}）",
                    ip, if detail.is_empty() { "无输出" } else { detail }, out.status.code());
                doh_failed.push(ip.to_string());
            }
            Err(e) => {
                crate::log_warn!("dns", "注册DoH加密服务器失败: {} - {}", ip, e);
                doh_failed.push(ip.to_string());
            }
        }
    }

    let _ = crate::network::discovery::new_command("ipconfig")
        .args(["/flushdns"])
        .output();

    if !api_success.is_empty() {
        // 文案按 family 反映实际写入的服务器（此前硬编码 v4 地址，IPv6 档误导）
        let servers_desc = match family {
            "ipv6" => format!("IPv6 {}/{}", dns_config::PRIMARY_DNS_V6, dns_config::SECONDARY_DNS_V6),
            "ipv4" => format!("IPv4 {}/{}", dns_config::PRIMARY_DNS, dns_config::SECONDARY_DNS),
            _ => format!(
                "IPv4 {}/{}、IPv6 {}/{}",
                dns_config::PRIMARY_DNS,
                dns_config::SECONDARY_DNS,
                dns_config::PRIMARY_DNS_V6,
                dns_config::SECONDARY_DNS_V6
            ),
        };
        let mut parts = Vec::new();
        if doh_failed.is_empty() {
            parts.push(format!(
                "已为 {} 设置DNS（{}）并启用DoH",
                api_success.join("、"),
                servers_desc
            ));
        } else {
            parts.push(format!(
                "已为 {} 设置DNS（{}），{}项DoH注册失败",
                api_success.join("、"),
                servers_desc,
                doh_failed.len()
            ));
        }
        if family == "ipv6" {
            parts.push("仅IPv6模式：请确保当前网络支持IPv6出口，否则域名解析可能失败".to_string());
        }
        if !api_fail.is_empty() {
            parts.push(format!("{}个适配器设置失败", api_fail.len()));
        }
        let doh_ok: Vec<&str> = dns_config::DOH_SERVERS.iter()
            .map(|(ip, _)| *ip)
            .filter(|ip| !doh_failed.iter().any(|f| f == ip))
            .collect();
        return serde_json::json!({
            "success": api_fail.is_empty() && doh_failed.is_empty(),
            "message": parts.join("，"),
            "dnsSuccess": api_success,
            "dnsFailed": api_fail,
            "dohAdded": doh_ok,
            "dohFailed": doh_failed,
        });
    }

    serde_json::json!({
        "success": false,
        "message": "设置DNS失败".to_string(),
        "dnsFailed": api_fail,
    })
}

#[cfg(not(target_os = "windows"))]
pub fn setup_dns_doh_admin(_targets: &[String], _family: &str) -> serde_json::Value {
    serde_json::json!({ "success": false, "message": "仅支持Windows".to_string() })
}
