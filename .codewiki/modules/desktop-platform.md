---
title: 桌面端 Windows 平台交互层
type: module
source_files:
  - tauri-app/src-tauri/src/platform/mod.rs
  - tauri-app/src-tauri/src/platform/console_output.rs
  - tauri-app/src-tauri/src/platform/autostart.rs
  - tauri-app/src-tauri/src/platform/dns_config.rs
  - tauri-app/src-tauri/src/platform/elevation.rs
  - tauri-app/src-tauri/windows/hooks.nsh
tags: [desktop, windows, win32, winrt, registry, uac, 平台层]
---

## Overview

`tauri-app/src-tauri/src/platform/` 是桌面端全部“与操作系统直接打交道”的代码。根据 `tauri-app/src-tauri/src/platform/mod.rs`，`console_output` 模块无 cfg 约束全平台编译（安卓侧同样需要 GBK 解码，见 mod.rs:2-3）；其余模块均受 `#[cfg(desktop)]` 门控（mod.rs:2-22），其中 `task_proxy`、`core_autostart`、`metric`、`best_route`、`route_table`、`icmp_probe`、`rtss_compat`、`toast` 进一步锁 `target_os = "windows"`（mod.rs:2-25）。本轮提供源码的模块包括：`console_output.rs`（跨平台控制台/HTTP 输出解码）、`autostart.rs`（HKCU 注册表自启）、`dns_config.rs`（Windows DNS/DoH 设置与注册表读取）、`elevation.rs`（UAC 提权与 GUID 解析），以及 `windows/hooks.nsh`（NSIS 卸载钩子清理计划任务代理）。该层被命令层、网络层等调用，本身不持有业务状态。

## Key Components

### mod.rs — 模块门控

| 行号 | 内容 | cfg |
|---|---|---|
| `tauri-app/src-tauri/src/platform/mod.rs:1-2` | `pub mod console_output;`（注释：协议响应/子网查询共用，安卓侧同样需要 GBK 解码） | 无（全平台编译） |
| `tauri-app/src-tauri/src/platform/mod.rs:5-6` | `pub mod autostart;` | `#[cfg(desktop)]` |
| `tauri-app/src-tauri/src/platform/mod.rs:8-9` | `pub mod ecoqos;`（Windows 效率模式 EcoQoS） | `#[cfg(desktop)]` |
| `tauri-app/src-tauri/src/platform/mod.rs:10-11` | `pub mod dns_config;` | `#[cfg(desktop)]` |
| `tauri-app/src-tauri/src/platform/mod.rs:12-13` | `pub mod elevation;` | `#[cfg(desktop)]` |
| `tauri-app/src-tauri/src/platform/mod.rs:14-15` | `pub mod gpu;` | `#[cfg(desktop)]` |
| `tauri-app/src-tauri/src/platform/mod.rs:16-17` | `pub mod helper_spawn;` | `#[cfg(desktop)]` |
| `tauri-app/src-tauri/src/platform/mod.rs:19-20` | `pub mod task_proxy;`（计划任务提权代理首选层） | `#[cfg(all(desktop, target_os = "windows"))]` |
| `tauri-app/src-tauri/src/platform/mod.rs:21-22` | `pub mod core_autostart;`（未登录 Windows 系统级自动登录） | `#[cfg(all(desktop, target_os = "windows"))]` |
| `tauri-app/src-tauri/src/platform/mod.rs:24-25` | `pub mod identity;` | `#[cfg(desktop)]` |
| `tauri-app/src-tauri/src/platform/mod.rs:27-28` | `pub mod metric;`（接口跃点读写，全量 Win32 IpHelper） | `#[cfg(all(desktop, target_os = "windows"))]` |
| `tauri-app/src-tauri/src/platform/mod.rs:30-31` | `pub mod best_route;`（路由级切换验证 GetBestRoute） | `#[cfg(all(desktop, target_os = "windows"))]` |
| `tauri-app/src-tauri/src/platform/mod.rs:33-34` | `pub mod route_table;`（IPv4 路由表读取+四元组精确删除） | `#[cfg(all(desktop, target_os = "windows"))]` |
| `tauri-app/src-tauri/src/platform/mod.rs:36-37` | `pub mod icmp_probe;`（ICMP 绑源探测 IcmpSendEcho2Ex） | `#[cfg(all(desktop, target_os = "windows"))]` |
| `tauri-app/src-tauri/src/platform/mod.rs:39-40` | `pub mod rtss_compat;`（RTSS hook 白屏预防） | `#[cfg(all(desktop, target_os = "windows"))]` |
| `tauri-app/src-tauri/src/platform/mod.rs:41-42` | `pub mod toast;` | `#[cfg(all(desktop, target_os = "windows"))]` |

注意：`autostart.rs` 等文件内部无 `#[cfg]` 属性（见逐文件清单），其仅在 Windows 桌面可用是“事实约束”而非编译期约束；`dns_config.rs`/`elevation.rs`/`console_output.rs` 在函数级带 `target_os = "windows"` 分支。

### console_output.rs — 输出编码解码（全平台）

| 行号 | 项 | 签名 / 值 | cfg |
|---|---|---|---|
| `tauri-app/src-tauri/src/platform/console_output.rs:1-8` | 模块文档 | 说明 Windows 控制台 OEM/UTF-8 代码页与 GBK 解码必要性 | 无 |
| `tauri-app/src-tauri/src/platform/console_output.rs:10-29` | `decode_with_code_page` | `fn(bytes: &[u8], code_page: u32) -> Option<String>`（私有；调 `MultiByteToWideChar` 18/23 + `String::from_utf16_lossy` 27） | `#[cfg(target_os = "windows")]` |
| `tauri-app/src-tauri/src/platform/console_output.rs:31-48` | `decode_console_bytes` | `pub fn(bytes: &[u8]) -> String`：严格 UTF-8 成功即原样返回（38-40），否则取 `GetOEMCP()`（41）解码，最后兜底 `from_utf8_lossy`（47） | `#[cfg(target_os = "windows")]` |
| `tauri-app/src-tauri/src/platform/console_output.rs:50-53` | `decode_console_bytes` | `pub fn(bytes: &[u8]) -> String`：直接 `from_utf8_lossy` | `#[cfg(not(target_os = "windows"))]` |
| `tauri-app/src-tauri/src/platform/console_output.rs:58-72` | `decode_charset_bytes` | `pub fn(bytes: &[u8], charset: Option<&str>) -> String`：charset 为 `gb*` / 含 `936` / `csgb2312` 时**硬编码按 936** 解码（63-69，匹配 65、解码 66），否则转 `decode_console_bytes`（71） | 无 cfg（内部按 `target_os` 分支；非 Windows 用 60-61 行显式消费 `charset` 避免 unused 警告） |
| `tauri-app/src-tauri/src/platform/console_output.rs:83-113` | `mod tests` | 3 个用例：`gbk_bytes_decode_to_expected_keyword`（83-94，UTF-8 系统上 OEM=65001 解出 U+FFFD 时只验不 panic，88-93）、`charset_gbk_forces_code_page_936`（100-105，断言 `gbk`/`GB2312`/`gb18030` 三种写法 102-104）、`utf8_bytes_pass_through`（108-112） | `#[cfg(test)]`（前两个用例内层再带 `#[cfg(target_os = "windows")]`） |

### autostart.rs — 开机自启（HKCU 注册表）

| 行号 | 项 | 签名 / 值 | cfg |
|---|---|---|---|
| `tauri-app/src-tauri/src/platform/autostart.rs:1` | `AUTOSTART_REG_KEY` | `const &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run"`（私有） | 无 |
| `tauri-app/src-tauri/src/platform/autostart.rs:2` | `AUTOSTART_REG_VALUE` | `const &str = "CampusLogin"`（私有） | 无 |
| `tauri-app/src-tauri/src/platform/autostart.rs:4-15` | `get_auto_launch_enabled` | `pub fn() -> bool`：`HKEY_CURRENT_USER` 下读 `CampusLogin` 值，非空即启用 | 无 |
| `tauri-app/src-tauri/src/platform/autostart.rs:17-30` | `set_auto_start` | `pub fn(exe_path: &str) -> Result<(), String>`：写值 `"<exe_path>" --autostart`（25 行） | 无 |
| `tauri-app/src-tauri/src/platform/autostart.rs:32-45` | `remove_auto_start` | `pub fn() -> Result<(), String>`：删除值；`NotFound` 视为成功（42 行） | 无 |

实现依赖 `winreg`（HKCU，无需管理员）。命令行尾部追加 `--autostart`。

### dns_config.rs — DNS / DoH（Windows）

公开常量（全部 `#[cfg(target_os = "windows")]`）：

| 行号 | 常量 | 值 |
|---|---|---|
| `tauri-app/src-tauri/src/platform/dns_config.rs:2` | `PRIMARY_DNS` | `"223.5.5.5"`（阿里公共 DNS） |
| `tauri-app/src-tauri/src/platform/dns_config.rs:4` | `SECONDARY_DNS` | `"1.12.12.12"`（DNSPod） |
| `tauri-app/src-tauri/src/platform/dns_config.rs:7` | `PRIMARY_DNS_V6` | `"2400:3200::1"` |
| `tauri-app/src-tauri/src/platform/dns_config.rs:9` | `SECONDARY_DNS_V6` | `"2402:4e00::"`（腾讯 DNSPod IPv6） |
| `tauri-app/src-tauri/src/platform/dns_config.rs:12-21` | `DOH_SERVERS: &[(&str, &str)]` | 7 组服务器 IP → DoH 模板映射：`223.5.5.5`/`223.6.6.6`/`2400:3200::1`/`2400:3200:baba::1` → `https://dns.alidns.com/dns-query`；`1.12.12.12`/`120.53.53.53`/`2402:4e00::` → `https://doh.pub/dns-query` |
| `tauri-app/src-tauri/src/platform/dns_config.rs:24` | `DNS_PROPERTY_TYPE_DOH` | `const i32 = 1`（私有） |
| `tauri-app/src-tauri/src/platform/dns_config.rs:27-28` | `DNS_SETTING_PROFILE_NAMESERVER` | `const u64 = 0x0200`（私有，带 `#[allow(dead_code)]`） |
| `tauri-app/src-tauri/src/platform/dns_config.rs:30-31` | `DNS_SETTING_DOH_PROFILE` | `const u64 = 0x2000`（私有，同上） |

私有函数与枚举：

| 行号 | 项 | 签名 | cfg |
|---|---|---|---|
| `tauri-app/src-tauri/src/platform/dns_config.rs:39-50` | `doh_bindings` | `fn(dns_servers: &[&str], doh_templates: &[(&str, &str)]) -> Vec<(usize, &str)>`：只为“实际存在于 NameServer 列表且配了模板”的服务器生成 `(ServerIndex, 模板)` | 无 |
| `tauri-app/src-tauri/src/platform/dns_config.rs:54-57` | `enum DnsTarget` | `{ Interface, Profile }`（`#[derive(Clone, Copy)]`，私有） | `#[cfg(target_os = "windows")]` |
| `tauri-app/src-tauri/src/platform/dns_config.rs:65-70` | `split_families` | `fn(dns_servers: &[&str]) -> (Vec<&str>, Vec<&str>)`：按是否含 `:` 分 IPv4/IPv6 | 无 |
| `tauri-app/src-tauri/src/platform/dns_config.rs:72-93` | `set_dns_inner` | `fn(target: DnsTarget, adapter_guid: &str, dns_servers: &[&str], doh_templates: &[(&str, &str)], err_label: &str) -> Result<(), String>`：两个栈分别调用 `set_dns_stack`，错误用 `；` 拼接（92） | `#[cfg(target_os = "windows")]` |
| `tauri-app/src-tauri/src/platform/dns_config.rs:99-212` | `set_dns_stack` | `fn(target, ipv6: bool, adapter_guid, dns_servers, doh_templates, err_label) -> Result<(), String>`：`elevation::parse_guid`（111）→ 组 `DNS_INTERFACE_SETTINGS3` 并调 `SetInterfaceDnsSettings`（201-208） | `#[cfg(target_os = "windows")]` |
| `tauri-app/src-tauri/src/platform/dns_config.rs:248-292` | `clear_dns_stack` | `fn(adapter_guid: &str, ipv6: bool) -> Result<(), String>`：NameServer 置空串清栈（`SetInterfaceDnsSettings` 281-288） | `#[cfg(target_os = "windows")]` |

公开函数：

| 行号 | 函数 | 签名 | cfg |
|---|---|---|---|
| `tauri-app/src-tauri/src/platform/dns_config.rs:215-222` | `set_dns_via_api` | `pub fn(adapter_guid: &str, dns_servers: &[&str], doh_templates: &[(&str, &str)]) -> Result<(), String>`：接口级 DNS+DoH（`DnsTarget::Interface`，err_label `"DNS+DoH"`） | `#[cfg(target_os = "windows")]` |
| `tauri-app/src-tauri/src/platform/dns_config.rs:226-232` | `set_profile_dns_via_api` | `pub fn(...)` 同上但 `DnsTarget::Profile`（err_label `"ProfileDNS"`）：仅对当前 WiFi 配置文件生效，切换 WiFi 自动切换 DNS | `#[cfg(target_os = "windows")]` |
| `tauri-app/src-tauri/src/platform/dns_config.rs:241-246` | `clear_adapter_dns_via_api` | `pub fn(adapter_guid: &str) -> Result<(), String>`：先清 IPv6 栈（失败仅警告，242-244）、再清 IPv4 栈（失败上报，245） | `#[cfg(target_os = "windows")]` |
| `tauri-app/src-tauri/src/platform/dns_config.rs:298-544` | `read_adapter_dns_from_registry` | `pub fn() -> Result<serde_json::Value, String>`：枚举网卡 → 读 DNS 来源与地址 → 解析 `netsh dns show encryption` → 输出 JSON | `#[cfg(target_os = "windows")]` |

`read_adapter_dns_from_registry` 内部的嵌套函数：

| 行号 | 项 | 说明 |
|---|---|---|
| `tauri-app/src-tauri/src/platform/dns_config.rs:311-331` | `should_filter_ip` | 过滤空串、非点分四段、末段 0/255、`127.*`、`169.254.*`、`198.18.*`/`198.19.*` |
| `tauri-app/src-tauri/src/platform/dns_config.rs:333-340` | `parse_dns_list` | 按 `,`/空格/`;` 切分并过滤 |
| `tauri-app/src-tauri/src/platform/dns_config.rs:342-428` | `check_doh_for_ips` | 调 `netsh dns show encryption`（345-348），逐行解析服务器 IP 与模板/自动升级字段，与 `DOH_SERVERS` 内置表合并（403-425），返回 `HashMap<String, (available, enabled, template)>`。IP 识别按“可解析为 `Ipv4Addr`/`Ipv6Addr` 的 token”匹配（365-367），DoH 属性字段名中英双语匹配（386-388），行结尾 flush（395-398） |

DNS 读优先级为 manual > profile > dhcp（456-464）；`ProfileNameServer` 单独读取（454）并计入 profile 地址列表（477-482）。产物：适配器级 JSON 含 `name`/`dnsSource`/`dnsServers`/`profileDnsServers`/`adapterDnsOverridesProfile`（530-536，535 行：manual 且 profile 非空时 true）；顶层 `adapters[]`、`dohSupported: true`（541）、`autoDohEnabled`（539-543）。

`tauri-app/src-tauri/src/platform/dns_config.rs:551-597` 为 `mod tests`（5 个纯函数用例）：`split_families_partitions_by_colon`（550-556）、`doh_bindings_only_for_present_servers`（558-570，注释 560-561 记录历史缺陷：曾对不在 NameServer 里的服务器也生成槽位；569 断言 `1.12.12.12 → doh.pub`）、`doh_bindings_skips_servers_without_template`（572-579）、`doh_bindings_support_ipv6_servers`（581-589）、`doh_bindings_empty_when_no_match`（591-596）。

### elevation.rs — 管理员判定与 UAC 提权（Windows）

| 行号 | 项 | 签名 | cfg |
|---|---|---|---|
| `tauri-app/src-tauri/src/platform/elevation.rs:2-24` | `is_admin` | `pub fn() -> bool`：`OpenProcessToken`（9）+ `GetTokenInformation(TokenElevation)`（14-20），`result.is_ok() && elevation != 0`（22）即管理员 | `#[cfg(target_os = "windows")]` |
| `tauri-app/src-tauri/src/platform/elevation.rs:27-50` | `run_elevated` | `pub fn(cmd: &str, args: &str) -> Result<(), String>`：`ShellExecuteW(None, "runas", cmd, args, None, SW_HIDE)`（36-43），返回值 ≤32 视为失败（45-47） | `#[cfg(target_os = "windows")]` |
| `tauri-app/src-tauri/src/platform/elevation.rs:52-129` | `shell_exec_elevated` | `pub fn(file: &str, params: &str, hide_window: bool) -> Result<(), String>`：COM 静默提权主路径 | `#[cfg(target_os = "windows")]` |
| `tauri-app/src-tauri/src/platform/elevation.rs:131-149` | `co_get_object_raw` | `unsafe fn(pszname, pbindoptions, riid, ppv) -> i32`（私有；`#[link(name = "ole32")]` 手写 `CoGetObject` 绑定 139-148） | `#[cfg(target_os = "windows")]` |
| `tauri-app/src-tauri/src/platform/elevation.rs:152-171` | `parse_guid` | `pub(crate) fn(s: &str) -> Result<windows::core::GUID, String>`：解析带/不带花括号的 GUID（5 段校验 155-157、data4 16 hex 162-164），供 `dns_config.rs:111`、`dns_config.rs:253` 使用 | `#[cfg(target_os = "windows")]` |
| `tauri-app/src-tauri/src/platform/elevation.rs:173-185` | `struct ICMLuaUtilVtbl` | `#[repr(C)]` 手工 vtable（私有，shell_exec 在 slot 9，见下节） | 无 cfg |
| `tauri-app/src-tauri/src/platform/elevation.rs:198-212` | `mod tests` | `vtbl_shell_exec_slot_is_9`（197-201，`offset_of!` 断言 ShellExec 在 slot 9，UACMe elvint.h 权威布局）、`shell_exec_elevated_smoke`（206-211，`#[ignore]` 真实提权冒烟 `cmd.exe /c exit`，本机手动 `cargo test -- --ignored`） | `#[cfg(test)]` |

COM 提权链细节（`shell_exec_elevated`，52-129）：`CoInitializeEx(COINIT_APARTMENTTHREADED)`（65，按成功与否决定是否配对 `CoUninitialize`，失败路径 93-95/116-119/123-125）→ moniker `Elevation:Administrator!new:{3E5FC7F9-9A51-4367-9063-A120244FBEC7}`（67）→ IID `{6EDD6D74-C007-4E75-B76A-E5740995E24C}`（70-72）→ **`BIND_OPTS3{cbStruct=sizeof(BIND_OPTS3)（79-80）, dwClassContext=CLSCTX_LOCAL_SERVER（81）}`**（历史缺陷：曾传 `BIND_OPTS` 16 字节，cbStruct 过小被 appinfo 按“提升激活配置无效”拒绝 0x80080017，静默提权从未通过；注释 74-77）→ `CoGetObject`（85-90）→ 取 vtable 调 `shell_exec(file, params, NULL, 0, n_show)`（105-112）→ `release`（114）。

### windows/hooks.nsh — NSIS 安装器钩子

| 行号 | 项 | 说明 |
|---|---|---|
| `tauri-app/src-tauri/windows/hooks.nsh:6-9` | `NSIS_HOOK_POSTUNINSTALL` | 卸载时清理计划任务提权代理与代理数据目录：`schtasks /delete /tn "CampusLoginPowerOps" /f`（7）删 SYSTEM 主体哑任务；`RMDir /r "$COMMONPROGRAMDATA\CampusLogin"`（8）删请求/结果文件目录（关联 `task_proxy` 模块，后者源码未在本轮提供） |

## Architecture

```text
┌──────────────────────────────────────────────────────────┐
│                      命令层 / 网络层                       │
│  (commands/*, network/* 调用 platform 公开 API)            │
└───────────┬───────────────┬───────────────┬──────────────┘
            │               │               │
            ▼               ▼               ▼
┌────────────────┐ ┌──────────────┐ ┌──────────────────┐
│   autostart    │ │  dns_config  │ │    elevation     │
│ (HKCU reg)     │ │ (Win32 DNS)  │ │ (UAC/COM/Token)  │
└────────────────┘ └──────┬───────┘ └─────────┬────────┘
                          │                  │
                          ▼                  ▼
┌────────────────────────┐ ┌────────────────────────────┐
│ console_output.decode_* │ │ elevation::parse_guid       │
│ (OEM/UTF-8/GBK)         │ │ (GUID 字符串→windows::GUID) │
└────────────────────────┘ └────────────────────────────┘
            │                  │
            ▼                  ▼
┌──────────────────────────────────────────────────────────┐
│ OS: Registry, IpHelper SetInterfaceDnsSettings,          │
│ Win32 Token, COM ICMLuaUtil, Task Scheduler (hooks.nsh)  │
└──────────────────────────────────────────────────────────┘
```

## Data Flow

### 控制台输出解码（多模块共用入口）

```text
netsh / ipconfig 等子进程 stdout/stderr（OEM 代码页 936 或 65001）
  → platform/console_output.rs:32 decode_console_bytes
       ├─ std::str::from_utf8 成功 → 原样返回（38-40）
       ├─ 失败 → GetOEMCP()（41）→ MultiByteToWideChar（18/23）→ from_utf16_lossy
       └─ 再失败 → from_utf8_lossy 兜底（47）
HTTP 响应体 → platform/console_output.rs:58 decode_charset_bytes
       ├─ charset 含 gb/936/csgb2312 → 硬编码 936 解码（66）
       └─ 否则转 decode_console_bytes（71）
```

调用方示例：`dns_config.rs:350` 在 `check_doh_for_ips` 中调 `decode_console_bytes` 解析 `netsh dns show encryption` 输出。

### DNS + DoH 设置与读取链（Windows）

```text
设置：前端 invoke → commands → platform/dns_config.rs:214 set_dns_via_api
      / 226 set_profile_dns_via_api
       → 72 set_dns_inner → 65 split_families → 99 set_dns_stack
       → 111 elevation::parse_guid → 201 SetInterfaceDnsSettings
清除：241 clear_adapter_dns_via_api → 249 clear_dns_stack (先 v6 后 v4)
读取：298 read_adapter_dns_from_registry
       → 435 枚举 HKLM\SYSTEM\CurrentControlSet\Control\Network\{4D36E972-…}
       → 451 读 Tcpip\Parameters\Interfaces\<guid> NameServer/ProfileNameServer/DhcpNameServer
       → 342 check_doh_for_ips 调 345 netsh dns show encryption
       → 530-543 输出适配器 JSON 与顶层 dohSupported/autoDohEnabled
```

### 提权流（elevation.rs）

```text
shell_exec_elevated (52)
  → 65 CoInitializeEx → 85 CoGetObject(moniker, BIND_OPTS3)
  → 99 取 vtable → 105 vtable.shell_exec → 114 release → 123 CoUninitialize
run_elevated (26) → 36 ShellExecuteW "runas"
is_admin (1) → 9 OpenProcessToken → 14 GetTokenInformation(TokenElevation)
```

### NSIS 卸载清理流（hooks.nsh）

```text
安装器卸载后触发 NSIS_HOOK_POSTUNINSTALL (6)
  → 7 schtasks /delete /tn "CampusLoginPowerOps" /f
  → 8 RMDir /r "$COMMONPROGRAMDATA\CampusLogin"
```

## Connections

- [[desktop-commands]]：命令层是本层的直接上游（`set_auto_launch`→`autostart`、`setup_dns_doh`/`reset_dns`/`check_dns_doh_status`→`dns_config`+`elevation`）。
- [[desktop-network-dns]]：DNS/DoH 的业务编排与本层 `dns_config` 的分工边界；`dns_config` 内部使用 `crate::network::discovery::new_command`（345）与 `console_output`。
- [[desktop-auth]]：`auth/protocol.rs` 通过 `console_output::decode_charset_bytes` 解码 GBK 响应体做成败关键词判定。
- [[android-plugins]]：安卓端不编译本层模块（仅继承 `console_output`），等价的 Keystore / 前台服务 / 网络绑定能力由安卓插件提供。
- [[desktop-infra]]：`dns_config` 使用 `crate::log_warn`/`log_debug` 宏（243,401,423,467 等），`elevation` 使用 COM 接口，生命周期依赖 `AppHandle` 资源（未在本轮文件直接出现，但 NSIS 钩子关联任务计划）。
- [[desktop-app-lifecycle]]：自启注册表写入的 `--autostart` 参数由主进程启动时解析。

## Known Issues

1. **模块门控只到 `desktop`，不锁 Windows**：`platform/mod.rs:5-25` 对 `autostart` / `dns_config` / `elevation` 等只加 `#[cfg(desktop)]`，而 `autostart.rs` 文件内部没有任何 `#[cfg]`（见逐文件清单），直接使用 `winreg`；项目实际只支持 Windows 桌面 + 安卓，但该约束是事实约定而非编译期保证。
2. **`clear_adapter_dns_via_api` 的 IPv6 清理失败被吞**：`tauri-app/src-tauri/src/platform/dns_config.rs:241-243` 清除 v6 栈失败仅记 `log_warn`（doc 234-239 说明 "IPV6 flag + 空串" 的 API 接受性未经 Win11 实测确证），可能导致旧静态 v6 DNS 残留而接口级设置覆盖 profile DNS。
3. **`should_filter_ip` 会丢弃合法的 `198.18/198.19` 内网 DNS**：`tauri-app/src-tauri/src/platform/dns_config.rs:311-331` 把 `198.18.*`、`198.19.*` 一并过滤（原意是剔除基准测试网段），特殊校园网部署可能显示不出真实 DNS。
4. **`read_adapter_dns_from_registry` 的 `dohSupported` 恒为 `true`**：`tauri-app/src-tauri/src/platform/dns_config.rs:541` 硬编码，未做系统能力探测。
5. **注册表读取依赖 `netsh` 输出格式**：`tauri-app/src-tauri/src/platform/dns_config.rs:366-452` 逐行解析中英双语字段名（386-388 的 `autoupgrade`/`自动升级`），`netsh` 输出格式或系统语言变化会直接影响 `dohEnabled` 判定。IPv6 条目的 token 解析已修复（365-367，按 `Ipv4Addr`/`Ipv6Addr` 可解析匹配），但整体格式依赖仍在。
6. **COM 提权依赖未公开接口**：`tauri-app/src-tauri/src/platform/elevation.rs:27-32` 使用 `Elevation:Administrator!new:{3E5FC7F9-…}` 与手写 IID `{6EDD6D74-…}`，属未文档化 COM 提权路径；Windows 更新后失效时的表现是降级为弹 UAC（`run_elevated`），不会静默失败。
7. **`hooks.nsh` 清理依赖任务名硬编码**：`tauri-app/src-tauri/windows/hooks.nsh:7` 固定删除 `CampusLoginPowerOps`，若 Rust 侧 `task_proxy::TASK_NAME` 变更需同步修改，否则卸载残留提权任务。
