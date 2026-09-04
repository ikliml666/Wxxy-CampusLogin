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

/// NameServer 条目按地址族分组：含 ':' 即 IPv6，否则按 IPv4 处理。
/// Win32 契约（DNS_INTERFACE_SETTINGS3.Flags）：一次调用只作用于一个栈——
/// 默认仅 IPv4 栈，带 DNS_SETTING_IPV6(0x0001) 时仅 IPv6 栈，且 NameServer
/// 地址族必须与目标栈一致，混合串写入会失败或被静默丢弃。
fn split_families<'a>(dns_servers: &[&'a str]) -> (Vec<&'a str>, Vec<&'a str>) {
    (
        dns_servers.iter().filter(|s| !s.contains(':')).copied().collect(),
        dns_servers.iter().filter(|s| s.contains(':')).copied().collect(),
    )
}

#[cfg(target_os = "windows")]
fn set_dns_inner(
    target: DnsTarget,
    adapter_guid: &str,
    dns_servers: &[&str],
    doh_templates: &[(&str, &str)],
    err_label: &str,
) -> Result<(), String> {
    let (v4_servers, v6_servers) = split_families(dns_servers);
    let mut errs: Vec<String> = Vec::new();
    if !v4_servers.is_empty() {
        if let Err(e) = set_dns_stack(target, false, adapter_guid, &v4_servers, doh_templates, err_label) {
            errs.push(e);
        }
    }
    if !v6_servers.is_empty() {
        if let Err(e) = set_dns_stack(target, true, adapter_guid, &v6_servers, doh_templates, err_label) {
            errs.push(e);
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs.join("；")) }
}

/// 设置单个协议栈的 DNS（+DoH）。`ipv6` 决定 flags 是否带 DNS_SETTING_IPV6，
/// NameServer 只接收该栈的地址；DoH 属性按 target 挂到与 flag 对应的字段：
/// Interface → ServerProperties（DNS_SETTING_DOH），
/// Profile → ProfileServerProperties（DNS_SETTING_DOH_PROFILE）。
#[cfg(target_os = "windows")]
fn set_dns_stack(
    target: DnsTarget,
    ipv6: bool,
    adapter_guid: &str,
    dns_servers: &[&str],
    doh_templates: &[(&str, &str)],
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
    doh_settings.reserve(dns_servers.len());
    doh_props.reserve(dns_servers.len());

    // ServerIndex 按 Win32 契约必须索引本栈 NameServer 列表中的实际位置：
    // 仅为列表中实际存在且配置了模板的服务器生成 DoH 属性，按服务器 IP 匹配模板。
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

    let ns_ptr = ns_wide.as_mut_ptr();
    let (nameserver, profile_nameserver) = match target {
        DnsTarget::Interface => (PWSTR(ns_ptr), PWSTR::null()),
        DnsTarget::Profile => (PWSTR::null(), PWSTR(ns_ptr)),
    };

    let ns_flag: u64 = match target {
        DnsTarget::Interface => DNS_SETTING_NAMESERVER as u64,
        DnsTarget::Profile => DNS_SETTING_PROFILE_NAMESERVER as u64,
    };
    let mut flags = ns_flag;
    if ipv6 {
        flags |= DNS_SETTING_IPV6 as u64;
    }
    if !doh_props.is_empty() {
        flags |= match target {
            DnsTarget::Interface => DNS_SETTING_DOH as u64,
            DnsTarget::Profile => DNS_SETTING_DOH_PROFILE as u64,
        };
    }

    // DoH 属性槽与 flag 一一对应，未使用的槽必须为 NULL
    let (c_server_props, server_props, c_profile_props, profile_props) = match target {
        DnsTarget::Interface => (doh_props.len() as u32, doh_props.as_mut_ptr(), 0u32, std::ptr::null_mut()),
        DnsTarget::Profile => (0u32, std::ptr::null_mut(), doh_props.len() as u32, doh_props.as_mut_ptr()),
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
        cServerProperties: c_server_props,
        ServerProperties: server_props,
        cProfileServerProperties: c_profile_props,
        ProfileServerProperties: profile_props,
    };

    unsafe {
        let result = SetInterfaceDnsSettings(
            guid,
            &settings as *const _ as *const DNS_INTERFACE_SETTINGS,
        );
        if result != windows::Win32::Foundation::WIN32_ERROR(0) {
            let stack = if ipv6 { "IPv6" } else { "IPv4" };
            return Err(format!("SetInterfaceDnsSettings({},{}) 失败: 错误码 {}", err_label, stack, result.0));
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
    set_dns_inner(DnsTarget::Interface, adapter_guid, dns_servers, doh_templates, "DNS+DoH")
}

/// 设置按配置文件（per-profile）的 DNS + DoH
/// 仅对当前 WiFi 配置文件生效，切换 WiFi 后自动切换 DNS
#[cfg(target_os = "windows")]
pub fn set_profile_dns_via_api(
    adapter_guid: &str,
    dns_servers: &[&str],
    doh_templates: &[(&str, &str)],
) -> Result<(), String> {
    set_dns_inner(DnsTarget::Profile, adapter_guid, dns_servers, doh_templates, "ProfileDNS")
}

/// 清除适配器级 DNS 设置（NameServer），恢复 DHCP 获取。
/// IPv4/IPv6 两个栈分别清除：只清 v4 栈时，旧的静态 v6 配置会残留。
/// v4 清除失败必须报错（接口级残留会覆盖 profile DNS）；v6 清除的
/// "IPV6 flag + 空串"组合的 API 接受性未经 Win11 实测确证（文档字面要求
/// IPV6 时 NameServer 必须为 v6 地址），失败降级为警告，避免整个 clear
/// 恒失败导致 WiFi 分支全部退化为接口级设置。
#[cfg(target_os = "windows")]
pub fn clear_adapter_dns_via_api(adapter_guid: &str) -> Result<(), String> {
    if let Err(e) = clear_dns_stack(adapter_guid, true) {
        crate::log_warn!("dns", "清除适配器级DNS(IPv6)失败（忽略）: {}", e);
    }
    clear_dns_stack(adapter_guid, false)
}

#[cfg(target_os = "windows")]
fn clear_dns_stack(adapter_guid: &str, ipv6: bool) -> Result<(), String> {
    use windows::Win32::NetworkManagement::IpHelper::*;
    use windows::core::PWSTR;

    let guid = crate::platform::elevation::parse_guid(adapter_guid)?;

    // 设置 NameServer 为空字符串，清除该栈的适配器级 DNS
    let mut empty_ns: Vec<u16> = vec![0u16];

    let flags = DNS_SETTING_NAMESERVER as u64
        | if ipv6 { DNS_SETTING_IPV6 as u64 } else { 0 };

    let settings = DNS_INTERFACE_SETTINGS3 {
        Version: DNS_INTERFACE_SETTINGS_VERSION3,
        Flags: flags,
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
            let stack = if ipv6 { "IPv6" } else { "IPv4" };
            return Err(format!("清除适配器级DNS({})失败: 错误码 {}", stack, result.0));
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
            let text = crate::platform::console_output::decode_console_bytes(&out.stdout);
            let mut current_ip: Option<String> = None;
            let mut current_template: Option<String> = None;
            let mut current_autoupgrade: bool = false;

            for line in text.lines() {
                let trimmed = line.trim();

                if trimmed.starts_with('-') || trimmed.is_empty() {
                    continue;
                }

                // 服务器 IP 行按"可解析为合法地址"识别：IPv4 或 IPv6。
                // 此前仅匹配点分十进制，IPv6 条目解析不到，且其模板行会被
                // 错误记到上一个 IPv4 条目名下（串染）。
                let ip_match = trimmed.split_whitespace()
                    .find(|s| s.parse::<std::net::Ipv4Addr>().is_ok() || s.parse::<std::net::Ipv6Addr>().is_ok())
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
    fn split_families_partitions_by_colon() {
        let mixed = ["223.5.5.5", "2400:3200::1", "1.12.12.12", "2402:4e00::"];
        let (v4, v6) = split_families(&mixed);
        assert_eq!(v4, vec!["223.5.5.5", "1.12.12.12"]);
        assert_eq!(v6, vec!["2400:3200::1", "2402:4e00::"]);
    }

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
