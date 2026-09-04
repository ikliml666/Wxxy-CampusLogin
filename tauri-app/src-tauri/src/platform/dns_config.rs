#[cfg(target_os = "windows")]
pub const PRIMARY_DNS: &str = "223.5.5.5";
#[cfg(target_os = "windows")]
pub const SECONDARY_DNS: &str = "1.12.12.12";
/// 阿里公共 DNS IPv6（官方公布：2400:3200::1 / 2400:3200:baba::1）
#[cfg(target_os = "windows")]
pub const PRIMARY_DNS_V6: &str = "2400:3200::1";
#[cfg(target_os = "windows")]
pub const SECONDARY_DNS_V6: &str = "2402:4e00::";
/// 腾讯 DNSPod IPv6（官方公布：2402:4e00::）
#[cfg(target_os = "windows")]
pub const DOH_SERVERS: &[(&str, &str)] = &[
    ("223.5.5.5", "https://dns.alidns.com/dns-query"),
    ("223.6.6.6", "https://dns.alidns.com/dns-query"),
    ("1.12.12.12", "https://doh.pub/dns-query"),
    ("120.53.53.53", "https://doh.pub/dns-query"),
    // IPv6：DoH 模板按域名，dns.alidns.com / doh.pub 双栈，v6 服务器复用同一模板
    ("2400:3200::1", "https://dns.alidns.com/dns-query"),
    ("2400:3200:baba::1", "https://dns.alidns.com/dns-query"),
    ("2402:4e00::", "https://doh.pub/dns-query"),
];

#[cfg(target_os = "windows")]
const DNS_PROPERTY_TYPE_DOH: i32 = 1;

#[cfg(target_os = "windows")]
#[allow(dead_code)] // 在 set_profile_dns_via_api 中使用，编译器因条件编译误报
const DNS_SETTING_PROFILE_NAMESERVER: u64 = 0x0200;
#[cfg(target_os = "windows")]
#[allow(dead_code)] // 在 set_profile_dns_via_api 中使用，编译器因条件编译误报
const DNS_SETTING_DOH_PROFILE: u64 = 0x2000;

/// 计算 NameServer 列表各服务器与 DoH 模板的绑定关系。
///
/// 返回 (ServerIndex, 模板URL) 列表，ServerIndex 是服务器在 `dns_servers` 中的下标。
/// 仅为实际存在且配置了模板的服务器生成绑定；模板不匹配的服务器不参与 DoH。
/// 历史缺陷：曾对 doh_templates 全量生成 ServerIndex 0..3，与仅含 2 台服务器的
/// NameServer 列表不匹配（越界 + 模板错配）。此纯函数供单测覆盖。
fn doh_bindings<'a>(dns_servers: &[&str], doh_templates: &'a [(&str, &str)]) -> Vec<(usize, &'a str)> {
    dns_servers
        .iter()
        .enumerate()
        .filter_map(|(idx, server_ip)| {
            doh_templates
                .iter()
                .find(|(ip, _)| ip == server_ip)
                .map(|(_, tpl)| (idx, *tpl))
        })
        .collect()
}

/// DNS 设置目标：Interface 写入 NameServer + NAMESERVER 类 flags，
/// Profile 写入 ProfileNameServer + PROFILE_NAMESERVER 类 flags。
#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
enum DnsTarget {
    Interface,
    Profile,
}

#[cfg(target_os = "windows")]
fn set_dns_inner(
    target: DnsTarget,
    adapter_guid: &str,
    dns_servers: &[&str],
    doh_templates: &[(&str, &str)],
    include_doh: bool,
    err_label: &str,
) -> Result<(), String> {
    use windows::Win32::NetworkManagement::IpHelper::*;
    use windows::core::PWSTR;

    let guid = crate::platform::elevation::parse_guid(adapter_guid)?;

    let ns_str: String = dns_servers.join(",");
    let mut ns_wide: Vec<u16> = ns_str.encode_utf16().chain(std::iter::once(0)).collect();

    let mut doh_props: Vec<DNS_SERVER_PROPERTY> = Vec::new();
    let mut doh_settings: Vec<DNS_DOH_SERVER_SETTINGS> = Vec::new();
    let mut doh_templates_wide: Vec<Vec<u16>> = Vec::new();
    doh_settings.reserve(doh_templates.len());
    doh_props.reserve(doh_templates.len());

    // 历史缺陷：对 doh_templates（4 台 DoH 服务器）全量生成 ServerIndex 0..3，
    // 但 NameServer 仅含 dns_servers（2 台）。ServerIndex 按 Win32 契约必须索引
    // NameServer 列表中的实际位置：索引 2/3 引用不存在的服务器导致
    // SetInterfaceDnsSettings 报 ERROR_INVALID_PARAMETER，且索引 1 的 DoH 模板
    // 错配（223.6.6.6 模板套到 1.12.12.12）。
    // 修复：仅为 NameServer 中实际存在的服务器生成 DoH 属性，按服务器 IP 匹配模板。
    for (idx, template) in doh_bindings(dns_servers, doh_templates) {
        let tpl_wide: Vec<u16> = template.encode_utf16().chain(std::iter::once(0)).collect();
        doh_templates_wide.push(tpl_wide);

        let doh_setting = DNS_DOH_SERVER_SETTINGS {
            Template: PWSTR(doh_templates_wide.last_mut().unwrap().as_mut_ptr()),
            Flags: (DNS_DOH_SERVER_SETTINGS_ENABLE_AUTO | DNS_DOH_SERVER_SETTINGS_ENABLE | DNS_DOH_SERVER_SETTINGS_FALLBACK_TO_UDP) as u64,
        };
        let cur_idx = doh_settings.len();
        doh_settings.push(doh_setting);

        let prop = DNS_SERVER_PROPERTY {
            Version: DNS_SERVER_PROPERTY_VERSION1,
            ServerIndex: idx as u32,
            Type: DNS_SERVER_PROPERTY_TYPE(DNS_PROPERTY_TYPE_DOH),
            Property: DNS_SERVER_PROPERTY_TYPES {
                DohSettings: &mut doh_settings[cur_idx],
            },
        };
        doh_props.push(prop);
    }

    // 目标字段与 flags 差异：Interface 写入 NameServer + NAMESERVER|DOH，
    // Profile 写入 ProfileNameServer + PROFILE_NAMESERVER|DOH_PROFILE
    let ns_ptr = ns_wide.as_mut_ptr();
    let (nameserver, profile_nameserver) = match target {
        DnsTarget::Interface => (PWSTR(ns_ptr), PWSTR::null()),
        DnsTarget::Profile => (PWSTR::null(), PWSTR(ns_ptr)),
    };
    let flags = match target {
        DnsTarget::Interface => {
            if include_doh && !doh_props.is_empty() {
                (DNS_SETTING_NAMESERVER | DNS_SETTING_DOH) as u64
            } else {
                DNS_SETTING_NAMESERVER as u64
            }
        }
        DnsTarget::Profile => {
            if !doh_props.is_empty() {
                (DNS_SETTING_PROFILE_NAMESERVER | DNS_SETTING_DOH_PROFILE) as u64
            } else {
                DNS_SETTING_PROFILE_NAMESERVER as u64
            }
        }
    };

    let settings = DNS_INTERFACE_SETTINGS3 {
        Version: DNS_INTERFACE_SETTINGS_VERSION3,
        Flags: flags,
        Domain: PWSTR::null(),
        NameServer: nameserver,
        SearchList: PWSTR::null(),
        RegistrationEnabled: 0,
        RegisterAdapterName: 0,
        EnableLLMNR: 0,
        QueryAdapterName: 0,
        ProfileNameServer: profile_nameserver,
        DisableUnconstrainedQueries: 0,
        SupplementalSearchList: PWSTR::null(),
        cServerProperties: doh_props.len() as u32,
        ServerProperties: doh_props.as_mut_ptr(),
        cProfileServerProperties: 0,
        ProfileServerProperties: std::ptr::null_mut(),
    };

    unsafe {
        let result = SetInterfaceDnsSettings(
            guid,
            &settings as *const _ as *const DNS_INTERFACE_SETTINGS,
        );
        if result != windows::Win32::Foundation::WIN32_ERROR(0) {
            return Err(format!("SetInterfaceDnsSettings({}) 失败: 错误码 {}", err_label, result.0));
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
pub fn set_dns_via_api(
    adapter_guid: &str,
    dns_servers: &[&str],
    doh_templates: &[(&str, &str)],
) -> Result<(), String> {
    set_dns_inner(DnsTarget::Interface, adapter_guid, dns_servers, doh_templates, true, "DNS+DoH")
}

/// 设置按配置文件（per-profile）的 DNS + DoH
/// 仅对当前 WiFi 配置文件生效，切换 WiFi 后自动切换 DNS
#[cfg(target_os = "windows")]
pub fn set_profile_dns_via_api(
    adapter_guid: &str,
    dns_servers: &[&str],
    doh_templates: &[(&str, &str)],
) -> Result<(), String> {
    set_dns_inner(DnsTarget::Profile, adapter_guid, dns_servers, doh_templates, true, "ProfileDNS")
}

/// 清除适配器级 DNS 设置（NameServer），使配置文件级 DNS 生效
#[cfg(target_os = "windows")]
pub fn clear_adapter_dns_via_api(adapter_guid: &str) -> Result<(), String> {
    use windows::Win32::NetworkManagement::IpHelper::*;
    use windows::core::PWSTR;

    let guid = crate::platform::elevation::parse_guid(adapter_guid)?;

    // 设置 NameServer 为空字符串，清除适配器级 DNS
    let mut empty_ns: Vec<u16> = [0u16].to_vec();

    let settings = DNS_INTERFACE_SETTINGS3 {
        Version: DNS_INTERFACE_SETTINGS_VERSION3,
        Flags: DNS_SETTING_NAMESERVER as u64,
        Domain: PWSTR::null(),
        NameServer: PWSTR(empty_ns.as_mut_ptr()),
        SearchList: PWSTR::null(),
        RegistrationEnabled: 0,
        RegisterAdapterName: 0,
        EnableLLMNR: 0,
        QueryAdapterName: 0,
        ProfileNameServer: PWSTR::null(),
        DisableUnconstrainedQueries: 0,
        SupplementalSearchList: PWSTR::null(),
        cServerProperties: 0,
        ServerProperties: std::ptr::null_mut(),
        cProfileServerProperties: 0,
        ProfileServerProperties: std::ptr::null_mut(),
    };

    unsafe {
        let result = SetInterfaceDnsSettings(
            guid,
            &settings as *const _ as *const DNS_INTERFACE_SETTINGS,
        );
        if result != windows::Win32::Foundation::WIN32_ERROR(0) {
            return Err(format!("清除适配器级DNS失败: 错误码 {}", result.0));
        }
    }

    Ok(())
}




#[cfg(target_os = "windows")]
pub fn read_adapter_dns_from_registry() -> Result<serde_json::Value, String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let net_key = hklm
        .open_subkey(r"SYSTEM\CurrentControlSet\Control\Network\{4D36E972-E325-11CE-BFC1-08002BE10318}")
        .map_err(|e| format!("打开网络注册表失败: {e}"))?;

    let tcpip_key = hklm
        .open_subkey(r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces")
        .map_err(|e| format!("打开TCP/IP注册表失败: {e}"))?;

    fn should_filter_ip(ip: &str) -> bool {
        let trimmed = ip.trim();
        if trimmed.is_empty() { return true; }
        let p: Vec<&str> = trimmed.split('.').collect();
        if p.len() != 4 { return false; }
        let o3: u8 = match p[3].parse::<u8>() { Ok(v) => v, Err(_) => return false };
        let o0 = p[0];
        if o3 == 0 || o3 == 255 { return true; }
        if o0 == "127" { return true; }
        if o0 == "169" {
            if let Ok(o1) = p[1].parse::<u8>() {
                if o1 == 254 { return true; }
            }
        }
        if o0 == "198" {
            if let Ok(o1) = p[1].parse::<u8>() {
                if o1 == 18 || o1 == 19 { return true; }
            }
        }
        false
    }

    fn parse_dns_list(raw: &str) -> Vec<String> {
        raw.split([',', ' ', ';'])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .filter(|s| !should_filter_ip(s))
            .map(|s| s.to_string())
            .collect()
    }

    fn check_doh_for_ips(dns_ips: &[String], _hklm: &winreg::RegKey) -> std::collections::HashMap<String, (bool, bool, String)> {
        let mut netsh_doh: std::collections::HashMap<String, (bool, String)> = std::collections::HashMap::new();

        let output = crate::network::discovery::new_command("netsh")
            .args(["dns", "show", "encryption"])
            .output();

        if let Ok(out) = output {
            let text = String::from_utf8_lossy(&out.stdout);
            let mut current_ip: Option<String> = None;
            let mut current_template: Option<String> = None;
            let mut current_autoupgrade: bool = false;

            for line in text.lines() {
                let trimmed = line.trim();

                if trimmed.starts_with('-') || trimmed.is_empty() {
                    continue;
                }

                let ip_match = trimmed.split_whitespace()
                    .find(|s| s.chars().all(|c| c.is_ascii_digit() || c == '.') && s.contains('.') && s.parse::<std::net::Ipv4Addr>().is_ok())
                    .map(|s| s.to_string());

                if let Some(ip) = ip_match {
                    if let Some(old_ip) = current_ip.take() {
                        let template = current_template.take().unwrap_or_default();
                        netsh_doh.insert(old_ip, (current_autoupgrade, template));
                        current_autoupgrade = false;
                    }
                    current_ip = Some(ip);
                    continue;
                }

                if current_ip.is_none() { continue; }

                if let Some(colon_pos) = trimmed.find(':') {
                    let field_name = trimmed[..colon_pos].trim().to_ascii_lowercase();
                    let val = trimmed[colon_pos + 1..].trim();
                    if val.starts_with("https://") {
                        current_template = Some(val.to_string());
                    } else if (field_name.contains("autoupgrade") || field_name.contains("自动升级"))
                        && (val.eq_ignore_ascii_case("yes") || val.eq_ignore_ascii_case("true")
                            || val.eq_ignore_ascii_case("是") || val.contains("yes") || val.contains("是"))
                    {
                        current_autoupgrade = true;
                    }
                }
            }

            if let Some(ip) = current_ip.take() {
                let template = current_template.take().unwrap_or_default();
                netsh_doh.insert(ip, (current_autoupgrade, template));
            }
        }

        crate::log_debug!("doh", "netsh检测结果: {:?}", netsh_doh);

        let builtin_doh: &[(&str, &str)] = DOH_SERVERS;

        let mut result: std::collections::HashMap<String, (bool, bool, String)> = std::collections::HashMap::new();
        for dns in dns_ips {
            let in_netsh = netsh_doh.get(dns);
            let in_builtin = builtin_doh.iter().find(|(ip, _)| *ip == dns);

            let (doh_available, doh_enabled, doh_template) = match (in_netsh, in_builtin) {
                (Some((autoupgrade, template)), _) => {
                    let tpl = if template.is_empty() {
                        in_builtin.map(|(_, t)| t.to_string()).unwrap_or_default()
                    } else {
                        template.clone()
                    };
                    (true, *autoupgrade, tpl)
                }
                (None, Some((_, tpl))) => (true, false, tpl.to_string()),
                (None, None) => (false, false, String::new()),
            };

            crate::log_debug!("doh", "{} available={} enabled={} template={}", dns, doh_available, doh_enabled, doh_template);
            result.insert(dns.to_string(), (doh_available, doh_enabled, doh_template));
        }

        result
    }

    let mut adapters_result: Vec<serde_json::Value> = Vec::new();
    let mut all_dns_ips: Vec<String> = Vec::new();
    type AdapterDnsRaw = (String, String, Vec<String>, Option<Vec<String>>);
    let mut adapter_dns_raw: Vec<AdapterDnsRaw> = Vec::new();

    for guid_entry in net_key.enum_keys().flatten() {
        let conn_path = format!(r"{guid_entry}\Connection");
        if let Ok(conn_key) = net_key.open_subkey(&conn_path) {
            let name: String = conn_key.get_value("Name").unwrap_or_default();
            if name.is_empty() { continue; }

            if crate::network::is_blacklisted(&name) { continue; }

            let pnp_id: String = conn_key.get_value("PnpInstanceID").unwrap_or_default();
            if !pnp_id.is_empty() {
                let d = pnp_id.to_lowercase();
                if d.contains("vethernet") || d.contains("vpci") || d.contains("vmbus")
                    || d.contains("tun") || d.contains("tap") || d.contains("wintun")
                { continue; }
            }

            if let Ok(iface_key) = tcpip_key.open_subkey(&guid_entry) {
                let ns: String = iface_key.get_value("NameServer").unwrap_or_default();
                let dhcp_ns: String = iface_key.get_value("DhcpNameServer").unwrap_or_default();
                let profile_ns: String = iface_key.get_value("ProfileNameServer").unwrap_or_default();

                let (source, raw) = if !ns.is_empty() {
                    ("manual", ns)
                } else if !profile_ns.is_empty() {
                    ("profile", profile_ns.clone())
                } else if !dhcp_ns.is_empty() {
                    ("dhcp", dhcp_ns)
                } else {
                    continue;
                };

                let addrs = parse_dns_list(&raw);
                crate::log_debug!("dns", "{} source={} raw:[{}] → [{:?}]", name, source, raw, addrs);

                if addrs.is_empty() { continue; }

                for ip in &addrs {
                    if !all_dns_ips.contains(ip) {
                        all_dns_ips.push(ip.clone());
                    }
                }

                let profile_addrs = if !profile_ns.is_empty() && source != "profile" {
                    let parsed = parse_dns_list(&profile_ns);
                    if parsed.is_empty() { None } else { Some(parsed) }
                } else {
                    None
                };

                if let Some(ref p_addrs) = profile_addrs {
                    for ip in p_addrs {
                        if !all_dns_ips.contains(ip) {
                            all_dns_ips.push(ip.clone());
                        }
                    }
                }

                adapter_dns_raw.push((name, source.to_string(), addrs, profile_addrs));
            }
        }
    }

    let doh_map = check_doh_for_ips(&all_dns_ips, &hklm);
    let any_doh_enabled = doh_map.values().any(|(_, enabled, _)| *enabled);

    for (name, source, addrs, profile_addrs) in adapter_dns_raw {
        let mut dns_list: Vec<serde_json::Value> = Vec::new();
        for dns in &addrs {
            let (doh_available, doh_enabled, doh_template) = doh_map.get(dns)
                .cloned()
                .unwrap_or((false, false, String::new()));
            dns_list.push(serde_json::json!({
                "address": dns,
                "dohAvailable": doh_available,
                "dohEnabled": doh_enabled,
                "dohTemplate": doh_template,
            }));
        }

        let profile_dns_list: Vec<serde_json::Value> = if let Some(ref p_addrs) = profile_addrs {
            p_addrs.iter().map(|dns| {
                let (doh_available, doh_enabled, doh_template) = doh_map.get(dns)
                    .cloned()
                    .unwrap_or((false, false, String::new()));
                serde_json::json!({
                    "address": dns,
                    "dohAvailable": doh_available,
                    "dohEnabled": doh_enabled,
                    "dohTemplate": doh_template,
                })
            }).collect()
        } else {
            vec![]
        };

        adapters_result.push(serde_json::json!({
            "name": name,
            "dnsSource": source,
            "dnsServers": dns_list,
            "profileDnsServers": profile_dns_list,
            "adapterDnsOverridesProfile": source == "manual" && profile_addrs.is_some(),
        }));
    }

    Ok(serde_json::json!({
        "adapters": adapters_result,
        "dohSupported": true,
        "autoDohEnabled": any_doh_enabled,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doh_bindings_only_for_present_servers() {
        // NameServer 仅含 2 台服务器，doh_templates 有 4 台。
        // 历史缺陷：生成 4 条属性（ServerIndex 0..3），索引 2/3 越界。
        let servers = ["223.5.5.5", "1.12.12.12"];
        let templates = DOH_SERVERS;
        let bindings = doh_bindings(&servers, templates);
        assert_eq!(bindings.len(), 2, "只为实际存在的服务器生成绑定");
        assert_eq!(bindings[0].0, 0);
        assert_eq!(bindings[0].1, "https://dns.alidns.com/dns-query");
        assert_eq!(bindings[1].0, 1);
        assert_eq!(bindings[1].1, "https://doh.pub/dns-query", "1.12.12.12 必须配对 doh.pub 而非 dns.alidns.com");
    }

    #[test]
    fn doh_bindings_skips_servers_without_template() {
        let servers = ["223.5.5.5", "9.9.9.9"]; // 9.9.9.9 无模板
        let templates = DOH_SERVERS;
        let bindings = doh_bindings(&servers, templates);
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].0, 0);
    }

    #[test]
    fn doh_bindings_support_ipv6_servers() {
        // IPv6 服务器按 IP 精确匹配模板：阿里 v6 配 dns.alidns.com、腾讯 v6 配 doh.pub
        let servers = ["2400:3200::1", "2402:4e00::"];
        let bindings = doh_bindings(&servers, DOH_SERVERS);
        assert_eq!(bindings.len(), 2, "v4+v6 混合 NameServer 中 IPv6 服务器也要参与 DoH 绑定");
        assert_eq!(bindings[0].1, "https://dns.alidns.com/dns-query");
        assert_eq!(bindings[1].1, "https://doh.pub/dns-query");
    }

    #[test]
    fn doh_bindings_empty_when_no_match() {
        let servers = ["8.8.8.8"];
        let bindings = doh_bindings(&servers, DOH_SERVERS);
        assert!(bindings.is_empty());
    }
}
