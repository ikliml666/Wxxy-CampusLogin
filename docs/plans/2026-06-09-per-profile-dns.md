# WLAN DNS 按配置文件设置 - 实施方案

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 WLAN 适配器的 DNS 设置从适配器级（全局）改为配置文件级（per-profile），解决手动关闭全局设置后无法检测、无法单独设置每个 WiFi DNS 的问题。

**Architecture:** WiFi 适配器使用 `DNS_SETTING_PROFILE_NAMESERVER` (0x0200) + `ProfileNameServer` 字段设置按配置文件 DNS，有线适配器保持 `DNS_SETTING_NAMESERVER` (0x0002) + `NameServer` 适配器级设置。设置前自动清除适配器级 DNS（避免优先级覆盖）。DNS 检测同时读取两层注册表值，前端展示完整双层数据。

**Tech Stack:** Rust (windows crate 0.58, winreg), TypeScript/React, Tauri IPC

---

## 文件变更清单

| 文件 | 操作 | 职责 |
|------|------|------|
| `src-tauri/src/platform/dns_config.rs` | 修改 | 核心变更：添加按配置文件 DNS API、清除适配器级 DNS、增强检测 |
| `src-tauri/src/commands/network_cmd.rs` | 修改 | WiFi 适配器使用 profile DNS，有线使用 adapter DNS |
| `frontend/src/network/types.ts` | 修改 | 类型定义增加 profile DNS 字段 |
| `frontend/src/network/NetworkPanel.tsx` | 修改 | 展示双层数据、优先级警告 |
| `frontend/src/i18n/locales/zh.json` | 修改 | 新增翻译 key |
| `frontend/src/i18n/locales/en.json` | 修改 | 新增翻译 key |

---

### Task 1: dns_config.rs - 添加按配置文件 DNS 设置 API

**Files:**
- Modify: `tauri-app/src-tauri/src/platform/dns_config.rs`

- [ ] **Step 1: 添加 DNS_SETTING_PROFILE_NAMESERVER 常量和 set_profile_dns_via_api 函数**

在 `dns_config.rs` 顶部常量区域添加：

```rust
#[cfg(target_os = "windows")]
const DNS_SETTING_PROFILE_NAMESERVER: u64 = 0x0200;
#[cfg(target_os = "windows")]
const DNS_SETTING_DOH_PROFILE: u64 = 0x2000;
```

在 `set_doh_via_api` 函数之后添加新函数：

```rust
/// 设置按配置文件（per-profile）的 DNS + DoH
/// 仅对当前 WiFi 配置文件生效，切换 WiFi 后自动切换 DNS
#[cfg(target_os = "windows")]
pub fn set_profile_dns_via_api(
    adapter_guid: &str,
    dns_servers: &[&str],
    doh_templates: &[(&str, &str)],
) -> Result<(), String> {
    use windows::Win32::NetworkManagement::IpHelper::*;
    use windows::core::PWSTR;

    let guid = crate::platform::elevation::parse_guid(adapter_guid)?;

    let ns_str: String = dns_servers.join(",");
    let mut ns_wide: Vec<u16> = ns_str.encode_utf16().chain(std::iter::once(0)).collect();

    let mut doh_props: Vec<DNS_SERVER_PROPERTY> = Vec::new();
    let mut doh_settings: Vec<DNS_DOH_SERVER_SETTINGS> = Vec::new();
    let mut doh_templates_wide: Vec<Vec<u16>> = Vec::new();

    for (idx, (_ip, template)) in doh_templates.iter().enumerate() {
        let tpl_wide: Vec<u16> = template.encode_utf16().chain(std::iter::once(0)).collect();
        doh_templates_wide.push(tpl_wide);

        let doh_setting = DNS_DOH_SERVER_SETTINGS {
            Template: PWSTR(doh_templates_wide.last_mut().unwrap().as_mut_ptr()),
            Flags: (DNS_DOH_SERVER_SETTINGS_ENABLE_AUTO | DNS_DOH_SERVER_SETTINGS_ENABLE | DNS_DOH_SERVER_SETTINGS_FALLBACK_TO_UDP) as u64,
        };
        doh_settings.push(doh_setting);

        let prop = DNS_SERVER_PROPERTY {
            Version: DNS_SERVER_PROPERTY_VERSION1,
            ServerIndex: idx as u32,
            Type: DNS_SERVER_PROPERTY_TYPE(DNS_PROPERTY_TYPE_DOH),
            Property: DNS_SERVER_PROPERTY_TYPES {
                DohSettings: &mut doh_settings[idx],
            },
        };
        doh_props.push(prop);
    }

    let flags = if !doh_props.is_empty() {
        (DNS_SETTING_PROFILE_NAMESERVER | DNS_SETTING_DOH_PROFILE) as u64
    } else {
        DNS_SETTING_PROFILE_NAMESERVER as u64
    };

    let settings = DNS_INTERFACE_SETTINGS3 {
        Version: DNS_INTERFACE_SETTINGS_VERSION3,
        Flags: flags,
        Domain: PWSTR::null(),
        NameServer: PWSTR::null(),
        SearchList: PWSTR::null(),
        RegistrationEnabled: 0,
        RegisterAdapterName: 0,
        EnableLLMNR: 0,
        QueryAdapterName: 0,
        ProfileNameServer: PWSTR(ns_wide.as_mut_ptr()),
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
            return Err(format!("SetInterfaceDnsSettings(ProfileDNS) 失败: 错误码 {}", result.0));
        }
    }

    Ok(())
}
```

- [ ] **Step 2: 添加清除适配器级 DNS 的函数**

```rust
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
```

- [ ] **Step 3: 编译验证**

Run: `cd tauri-app/src-tauri && cargo check`
Expected: 编译通过（可能有 unused warning，正常）

- [ ] **Step 4: Commit**

```bash
git add tauri-app/src-tauri/src/platform/dns_config.rs
git commit -m "feat(dns): add per-profile DNS API and clear adapter DNS function"
```

---

### Task 2: dns_config.rs - 增强 DNS 检测读取 ProfileNameServer

**Files:**
- Modify: `tauri-app/src-tauri/src/platform/dns_config.rs`

- [ ] **Step 1: 修改 read_adapter_dns_from_registry 函数，同时读取 ProfileNameServer**

在 `read_adapter_dns_from_registry()` 函数中，`if let Ok(iface_key) = tcpip_key.open_subkey(&guid_entry)` 块内，读取 `NameServer` 之后，增加读取 `ProfileNameServer`：

找到这段代码：
```rust
let ns: String = iface_key.get_value("NameServer").unwrap_or_default();
let dhcp_ns: String = iface_key.get_value("DhcpNameServer").unwrap_or_default();

let (source, raw) = if !ns.is_empty() {
    ("manual", ns)
} else if !dhcp_ns.is_empty() {
    ("dhcp", dhcp_ns)
} else {
    continue;
};
```

替换为：
```rust
let ns: String = iface_key.get_value("NameServer").unwrap_or_default();
let dhcp_ns: String = iface_key.get_value("DhcpNameServer").unwrap_or_default();
let profile_ns: String = iface_key.get_value("ProfileNameServer").unwrap_or_default();

let (source, raw) = if !ns.is_empty() {
    ("manual", ns)
} else if !profile_ns.is_empty() {
    ("profile", profile_ns)
} else if !dhcp_ns.is_empty() {
    ("dhcp", dhcp_ns)
} else {
    continue;
};
```

同时修改 `adapter_dns_raw` 的类型，增加 profile DNS 信息：

找到：
```rust
let mut adapter_dns_raw: Vec<(String, String, Vec<String>)> = Vec::new();
```

替换为：
```rust
let mut adapter_dns_raw: Vec<(String, String, Vec<String>, Option<Vec<String>>)> = Vec::new();
```

在填充 `adapter_dns_raw` 的地方，找到：
```rust
adapter_dns_raw.push((name, source.to_string(), addrs));
```

替换为：
```rust
let profile_addrs = if !profile_ns.is_empty() && source != "profile" {
    let parsed = parse_dns_list(&profile_ns);
    if parsed.is_empty() { None } else { Some(parsed) }
} else {
    None
};
adapter_dns_raw.push((name, source.to_string(), addrs, profile_addrs));
```

在构建 `adapters_result` 的循环中，找到：
```rust
for (name, source, addrs) in adapter_dns_raw {
```

替换为：
```rust
for (name, source, addrs, profile_addrs) in adapter_dns_raw {
```

在 `dns_list` 构建完成后、`adapters_result.push` 之前，添加 profile DNS 数据：

```rust
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
```

修改 `adapters_result.push` 的 JSON 对象，增加 `profileDnsServers` 和 `adapterDnsOverridesProfile` 字段：

```rust
adapters_result.push(serde_json::json!({
    "name": name,
    "dnsSource": source,
    "dnsServers": dns_list,
    "profileDnsServers": profile_dns_list,
    "adapterDnsOverridesProfile": source == "manual" && !profile_addrs.is_none(),
}));
```

- [ ] **Step 2: 编译验证**

Run: `cd tauri-app/src-tauri && cargo check`
Expected: 编译通过

- [ ] **Step 3: Commit**

```bash
git add tauri-app/src-tauri/src/platform/dns_config.rs
git commit -m "feat(dns): read ProfileNameServer in DNS detection, show dual-layer data"
```

---

### Task 3: network_cmd.rs - WiFi 适配器使用 profile DNS

**Files:**
- Modify: `tauri-app/src-tauri/src/commands/network_cmd.rs`

- [ ] **Step 1: 修改 setup_dns_doh 命令，WiFi 使用 profile DNS**

在 `setup_dns_doh()` 函数的管理员路径中，找到：

```rust
for adapter in &active {
    let dns_list: Vec<&str> = vec![dns_config::PRIMARY_DNS, dns_config::SECONDARY_DNS];
    let doh_list: Vec<(&str, &str)> = dns_config::DOH_SERVERS.to_vec();

    match dns_config::set_dns_via_api(&adapter.guid, &dns_list, &doh_list) {
```

替换为：

```rust
for adapter in &active {
    let dns_list: Vec<&str> = vec![dns_config::PRIMARY_DNS, dns_config::SECONDARY_DNS];
    let doh_list: Vec<(&str, &str)> = dns_config::DOH_SERVERS.to_vec();

    // WiFi 适配器：先清除适配器级 DNS，再设置配置文件级 DNS
    // 有线适配器：保持适配器级 DNS
    if adapter.wireless {
        // 清除适配器级 DNS，确保配置文件级 DNS 生效
        if let Err(e) = dns_config::clear_adapter_dns_via_api(&adapter.guid) {
            crate::log_warn!("dns", "清除适配器级DNS失败: {} - {}", adapter.name, e);
        }
        match dns_config::set_profile_dns_via_api(&adapter.guid, &dns_list, &doh_list) {
            Ok(()) => {
                crate::log_info!("dns", "配置文件级DNS+DoH设置成功: {}", adapter.name);
                api_success.push(adapter.name.clone());
            }
            Err(e) => {
                crate::log_warn!("dns", "配置文件级DNS设置失败: {} - {}, 降级到适配器级", adapter.name, e);
                // 降级到适配器级 DNS
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
```

注意：`else` 分支保持原有逻辑不变。

- [ ] **Step 2: 修改非管理员路径的 PowerShell 命令**

在 `setup_dns_doh()` 的非管理员路径中，找到构建 `ps_cmds` 的循环：

```rust
for adapter in &active {
    ps_cmds.push(format!(
        "Set-DnsClientServerAddress -InterfaceAlias '{}' -ServerAddresses ('{}','{}') -Confirm:$false",
        crate::network::adapter::escape_ps_single_quote(&adapter.name), dns_config::PRIMARY_DNS, dns_config::SECONDARY_DNS
    ));
}
```

替换为：

```rust
for adapter in &active {
    if adapter.wireless {
        // WiFi: 清除适配器级 DNS，设置配置文件级 DNS
        // Set-DnsClientServerAddress 的 -ResetParameter 没有直接清除 NameServer 的方式
        // 使用 netsh 先清除适配器级 DNS，再通过注册表设置 ProfileNameServer
        ps_cmds.push(format!(
            "netsh interface ip set dns name='{}' dhcp",
            crate::network::adapter::escape_ps_single_quote(&adapter.name)
        ));
        // 设置 ProfileNameServer（通过注册表）
        ps_cmds.push(format!(
            "Set-ItemProperty -Path 'HKLM:\\SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters\\Interfaces\\{}' -Name 'ProfileNameServer' -Value '{},{}'",
            adapter.guid, dns_config::PRIMARY_DNS, dns_config::SECONDARY_DNS
        ));
    } else {
        ps_cmds.push(format!(
            "Set-DnsClientServerAddress -InterfaceAlias '{}' -ServerAddresses ('{}','{}') -Confirm:$false",
            crate::network::adapter::escape_ps_single_quote(&adapter.name), dns_config::PRIMARY_DNS, dns_config::SECONDARY_DNS
        ));
    }
}
```

同样修改 `cmd` 降级路径中的 `all_cmds` 构建，WiFi 适配器使用注册表设置 ProfileNameServer：

找到：
```rust
for adapter in &active {
    all_cmds.push_str(&format!("netsh interface ip set dns name=\"{}\" static {} primary & ", adapter.name, dns_config::PRIMARY_DNS));
    all_cmds.push_str(&format!("netsh interface ip add dns name=\"{}\" {} index=2 & ", adapter.name, dns_config::SECONDARY_DNS));
```

替换为：

```rust
for adapter in &active {
    if adapter.wireless {
        // WiFi: 清除适配器级 DNS (恢复 DHCP)，设置 ProfileNameServer
        all_cmds.push_str(&format!("netsh interface ip set dns name=\"{}\" dhcp & ", adapter.name));
        if !adapter.guid.is_empty() {
            all_cmds.push_str(&format!(
                "reg add \"HKLM\\SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters\\Interfaces\\{}\" /v ProfileNameServer /t REG_SZ /d \"{},{}\" /f & ",
                adapter.guid, dns_config::PRIMARY_DNS, dns_config::SECONDARY_DNS
            ));
        }
    } else {
        all_cmds.push_str(&format!("netsh interface ip set dns name=\"{}\" static {} primary & ", adapter.name, dns_config::PRIMARY_DNS));
        all_cmds.push_str(&format!("netsh interface ip add dns name=\"{}\" {} index=2 & ", adapter.name, dns_config::SECONDARY_DNS));
    }
```

- [ ] **Step 3: 修改 enable_doh_for_dns 命令**

在 `enable_doh_for_dns()` 的管理员路径中，找到：

```rust
for adapter in &active {
    match dns_config::set_doh_via_api(&adapter.guid, &dns_ips, dns_config::DOH_SERVERS) {
```

WiFi 适配器使用 profile DoH（`set_profile_dns_via_api` 传入当前 DNS + DoH），有线适配器保持原有逻辑。但由于 `enable_doh_for_dns` 只启用 DoH 不改变 DNS 服务器，这里保持原有逻辑即可——DoH 加密注册是全局的（`netsh dns add encryption`），不区分适配器/配置文件。

**此步骤无需修改 `enable_doh_for_dns`。**

- [ ] **Step 4: 编译验证**

Run: `cd tauri-app/src-tauri && cargo check`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add tauri-app/src-tauri/src/commands/network_cmd.rs
git commit -m "feat(dns): WiFi adapters use per-profile DNS, wired use per-adapter DNS"
```

---

### Task 4: 前端类型定义和 i18n 更新

**Files:**
- Modify: `tauri-app/frontend/src/network/types.ts`
- Modify: `tauri-app/frontend/src/i18n/locales/zh.json`
- Modify: `tauri-app/frontend/src/i18n/locales/en.json`

- [ ] **Step 1: 更新 TypeScript 类型定义**

在 `types.ts` 中，修改 `DnsAdapterInfo` 接口：

```typescript
interface DnsServerInfo {
  address: string
  dohAvailable: boolean
  dohEnabled: boolean
  dohTemplate: string
}

export interface DnsAdapterInfo {
  name: string
  dnsSource: string
  dnsServers: DnsServerInfo[]
  profileDnsServers: DnsServerInfo[]
  adapterDnsOverridesProfile: boolean
}
```

- [ ] **Step 2: 更新中文翻译**

在 `zh.json` 的 `network` 命名空间中添加：

```json
"profileDns": "配置文件DNS",
"adapterDns": "适配器DNS",
"adapterDnsOverridesProfile": "适配器级DNS已覆盖配置文件级DNS",
"adapterDnsOverridesProfileTip": "适配器级DNS优先级更高，配置文件级DNS不会生效。建议清除适配器级DNS。",
"perProfileDns": "按配置文件",
"perAdapterDns": "按适配器",
"currentWifiOnly": "仅当前WiFi",
"allWifiNetworks": "所有WiFi网络",
"wifiDnsMode": "WiFi DNS模式"
```

- [ ] **Step 3: 更新英文翻译**

在 `en.json` 的 `network` 命名空间中添加对应英文翻译。

- [ ] **Step 4: Commit**

```bash
git add tauri-app/frontend/src/network/types.ts tauri-app/frontend/src/i18n/locales/zh.json tauri-app/frontend/src/i18n/locales/en.json
git commit -m "feat(dns): add profile DNS types and i18n translations"
```

---

### Task 5: NetworkPanel.tsx - 展示双层数据和优先级警告

**Files:**
- Modify: `tauri-app/frontend/src/network/NetworkPanel.tsx`

- [ ] **Step 1: 修改 getDnsQuality 函数，考虑 profile DNS**

找到 `getDnsQuality` 函数，修改逻辑以考虑 profile DNS：

```typescript
const getDnsQuality = (
  adapter: {
    dnsSource?: string;
    dnsServers: { address: string; dohAvailable: boolean; dohEnabled: boolean }[];
    profileDnsServers?: { address: string; dohAvailable: boolean; dohEnabled: boolean }[];
    adapterDnsOverridesProfile?: boolean;
  },
  autoDohEnabled: boolean
) => {
  const servers = adapter.dnsServers || []
  const profileServers = adapter.profileDnsServers || []
  const effectiveServers = adapter.adapterDnsOverridesProfile ? servers : (servers.length > 0 ? servers : profileServers)

  if (effectiveServers.length === 0 || adapter.dnsSource === 'dhcp') return { level: 'none' as const, label: t('network.dnsNotConfigured') }
  const hasRecommended = effectiveServers.some(s => RECOMMENDED_DNS.has(s.address))
  const dohActive = autoDohEnabled || effectiveServers.filter(s => RECOMMENDED_DNS.has(s.address)).every(s => s.dohEnabled)
  if (hasRecommended && dohActive) return { level: 'excellent' as const, label: t('network.dnsRecommendedWithDoh') }
  if (hasRecommended) return { level: 'good' as const, label: t('network.dnsRecommendedNoDoh') }
  return { level: 'basic' as const, label: t('network.dnsNotRecommendedShort') }
}
```

- [ ] **Step 2: 在 DNS 适配器卡片中展示 profile DNS 和优先级警告**

在 `dnsStatus.adapters.map` 回调中，在 `adapter.dnsServers.map` 渲染之后，添加 profile DNS 展示和优先级警告：

找到 `{adapter.dnsServers.length === 0 && (` 之前，插入：

```tsx
{/* 适配器级 DNS 覆盖配置文件级 DNS 警告 */}
{adapter.adapterDnsOverridesProfile && adapter.profileDnsServers && adapter.profileDnsServers.length > 0 && (
  <div className="flex items-center gap-2 p-2 rounded-lg bg-amber-500/5 border border-amber-500/10">
    <AlertTriangle className="h-3.5 w-3.5 text-amber-500 shrink-0" />
    <span className="text-xs text-amber-600">{t('network.adapterDnsOverridesProfileTip')}</span>
  </div>
)}
{/* 配置文件级 DNS */}
{adapter.profileDnsServers && adapter.profileDnsServers.length > 0 && (
  <div className="space-y-1 mt-1 pt-1 border-t border-border/30">
    <span className="text-[10px] text-muted-foreground/60 uppercase tracking-wider">{t('network.profileDns')}</span>
    {adapter.profileDnsServers.map((dns) => (
      <div key={dns.address} className="flex items-center gap-2 text-xs">
        <span className={cn("font-mono", RECOMMENDED_DNS.has(dns.address) ? "text-green-600" : "text-muted-foreground")}>
          {dns.address}
        </span>
        {RECOMMENDED_DNS.has(dns.address) && (
          <span className="text-muted-foreground/60">
            {ALI_DNS.has(dns.address) ? t('network.ali') : t('network.tencent')}
          </span>
        )}
        {dns.dohEnabled ? (
          <CheckCircle2 className="h-3 w-3 text-green-500" />
        ) : dns.dohAvailable ? (
          <XCircle className="h-3 w-3 text-amber-400" />
        ) : (
          <XCircle className="h-3 w-3 text-muted-foreground/30" />
        )}
      </div>
    ))}
  </div>
)}
```

- [ ] **Step 3: 在 DNS 来源标签旁显示模式标识**

在适配器名称旁的 Badge 区域，找到 `{quality.level === 'excellent' && ...}` 等行，在 dnsSource 为 "profile" 时添加模式标识：

```tsx
{adapter.dnsSource === 'profile' && (
  <Badge variant="outline" size="sm" className="border-purple-500/30 text-purple-600">{t('network.perProfileDns')}</Badge>
)}
{adapter.dnsSource === 'manual' && (
  <Badge variant="outline" size="sm" className="border-blue-500/30 text-blue-600">{t('network.perAdapterDns')}</Badge>
)}
```

- [ ] **Step 4: 验证前端编译**

Run: `cd tauri-app/frontend && npx tsc --noEmit`
Expected: 类型检查通过

- [ ] **Step 5: Commit**

```bash
git add tauri-app/frontend/src/network/NetworkPanel.tsx
git commit -m "feat(dns): show dual-layer DNS data and priority warning in UI"
```

---

### Task 6: 集成测试和验证

**Files:**
- 无新文件

- [ ] **Step 1: 完整编译后端**

Run: `cd tauri-app/src-tauri && cargo build`
Expected: 编译成功

- [ ] **Step 2: 完整编译前端**

Run: `cd tauri-app/frontend && npm run build`
Expected: 构建成功

- [ ] **Step 3: 运行已有测试**

Run: `cd tauri-app/src-tauri && cargo test`
Expected: 所有测试通过

- [ ] **Step 4: 手动功能验证清单**

验证项目：
1. 连接校园网 WiFi → 点击"检测DNS" → 应显示 profile DNS 层数据
2. 点击"一键优化" → WiFi 适配器应使用 ProfileNameServer，有线适配器使用 NameServer
3. 优化后再次检测 → WiFi 适配器 dnsSource 应为 "profile"
4. 手动在 Windows 设置中关闭 WiFi 的 DNS → 再次检测 → 应能正确显示状态
5. 连接不同 WiFi → DNS 设置应独立（仅影响当前 WiFi 配置文件）

- [ ] **Step 5: Final Commit**

```bash
git add -A
git commit -m "feat(dns): complete per-profile DNS for WiFi adapters"
```

---

## 风险和注意事项

1. **ProfileNameServer 注册表残留 Bug**：Windows 已知 Bug，手动设置 DNS 后恢复 DHCP 时 ProfileNameServer 不会自动清除。我们的 `clear_adapter_dns_via_api` 只清除 NameServer，不清除 ProfileNameServer，避免触发此 Bug。

2. **适配器级 DNS 优先级**：如果用户在 ncpa.cpl 中手动设置了静态 DNS，配置文件级 DNS 会被忽略。前端已添加优先级警告提示。

3. **Windows 版本兼容**：`DNS_SETTING_PROFILE_NAMESERVER` (0x0200) 最低要求 Win10 Build 19041。`SetInterfaceDnsSettings` API 在旧版本上会返回错误，此时降级到适配器级 DNS。

4. **GUID 安全**：`adapter.guid` 来自 Win32 API，已通过 `parse_guid` 校验格式，不存在注入风险。

5. **WiFi 适配器判断**：使用 `adapter.wireless` 字段（来自 `IfType == IF_TYPE_IEEE80211`），这是 Win32 API 标准判断，不依赖名称模式。
