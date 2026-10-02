---
title: 桌面端 Windows 平台交互层
type: module
source_files:
  - tauri-app/src-tauri/src/platform/mod.rs
  - tauri-app/src-tauri/src/platform/console_output.rs
  - tauri-app/src-tauri/src/platform/autostart.rs
  - tauri-app/src-tauri/src/platform/dns_config.rs
  - tauri-app/src-tauri/src/platform/elevation.rs
  - tauri-app/src-tauri/src/platform/gpu.rs
  - tauri-app/src-tauri/src/platform/helper_spawn.rs
  - tauri-app/src-tauri/src/platform/task_proxy.rs
  - tauri-app/src-tauri/src/platform/metric.rs
  - tauri-app/src-tauri/windows/hooks.nsh
  - tauri-app/src-tauri/src/platform/identity.rs
  - tauri-app/src-tauri/src/platform/toast.rs
tags: [desktop, windows, win32, winrt, registry, uac, 平台层]
---

## Overview

`tauri-app/src-tauri/src/platform/` 是桌面端全部"与操作系统直接打交道"的代码：控制台输出与 HTTP 响应体解码（GBK/OEM 代码页）、注册表读写（开机自启、适配器 DNS 读取、Toast AUMID 注册）、Win32 DNS 接口调用（`SetInterfaceDnsSettings` + DoH 属性）、接口跃点（metric）读写（`GetIpInterfaceTable` 读 + helper `SetIpInterfaceEntry` 写）、UAC 提权（COM `ICMLuaUtil` 静默提权、`ShellExecuteW runas` 降级、计划任务提权代理三级通道）、提权 helper 子进程的启动与结果轮询、DXGI/GDI 硬件信息探测、Windows Hello 身份验证、WinRT Toast 通知。

该层不持有业务状态（唯一例外是 `identity.rs` 的进程内验证时间戳与 `task_proxy.rs` 的通道状态缓存），被 `commands/`、`network/`、`auth/`、`update/`、`app/`、`monitor/` 调用；`platform/mod.rs:1-36` 用 `#[cfg(desktop)]` 把除 `console_output` 外的模块整体排除在安卓构建之外（安卓通过 Cargo path 依赖桌面 crate，只继承 `console_output` 等跨平台部分），其中 `task_proxy`（19-20）、`metric`（24-25）、`best_route`（27-28）、`icmp_probe`（30-31）、`rtss_compat`（33-34）、`toast`（35-36）六个模块额外锁 `target_os = "windows"`。`best_route` / `icmp_probe` 是夜间出站切换的路由级验证与网关可达性探测底座（模块注释见 `platform/mod.rs:26-31`，业务侧见 [[outbound-switch]]）。

## Key Components

### mod.rs — 模块门控

| 行号 | 内容 | cfg |
|---|---|---|
| `platform/mod.rs:1-2` | `pub mod console_output;`（注释：协议响应/子网查询共用，安卓侧同样需要 GBK 解码） | 无（全平台编译） |
| `platform/mod.rs:5-6` | `pub mod autostart;` | `#[cfg(desktop)]` |
| `platform/mod.rs:8-9` | `pub mod ecoqos;`（Windows 效率模式） | `#[cfg(desktop)]` |
| `platform/mod.rs:10-11` | `pub mod dns_config;` | `#[cfg(desktop)]` |
| `platform/mod.rs:12-13` | `pub mod elevation;` | `#[cfg(desktop)]` |
| `platform/mod.rs:14-15` | `pub mod gpu;` | `#[cfg(desktop)]` |
| `platform/mod.rs:16-17` | `pub mod helper_spawn;` | `#[cfg(desktop)]` |
| `platform/mod.rs:19-20` | `pub mod task_proxy;`（计划任务提权代理，注释 18：提权通道的首选层） | `#[cfg(all(desktop, target_os = "windows"))]` |
| `platform/mod.rs:21-22` | `pub mod identity;` | `#[cfg(desktop)]` |
| `platform/mod.rs:24-25` | `pub mod metric;`（接口跃点读写，注释 23：全量 Win32 IpHelper） | `#[cfg(all(desktop, target_os = "windows"))]` |
| `platform/mod.rs:27-28` | `pub mod best_route;`（路由级切换验证：GetBestRoute，夜间出站切换完成后校验禁用效果） | `#[cfg(all(desktop, target_os = "windows"))]` |
| `platform/mod.rs:30-31` | `pub mod icmp_probe;`（ICMP 绑源探测：IcmpSendEcho2Ex，网关可达性判定） | `#[cfg(all(desktop, target_os = "windows"))]` |
| `platform/mod.rs:33-34` | `pub mod rtss_compat;`（RTSS hook 白屏预防） | `#[cfg(all(desktop, target_os = "windows"))]` |
| `platform/mod.rs:35-36` | `pub mod toast;` | `#[cfg(all(desktop, target_os = "windows"))]` |

注意：`autostart` / `gpu` / `helper_spawn` / `identity` 只有模块级 `#[cfg(desktop)]`，文件内部**没有任何 `#[cfg]` 属性**（见下方逐文件清单），它们只在 Windows 桌面可用是"事实约束"而非编译期约束；`dns_config` / `elevation` / `console_output` 在函数级带 `target_os = "windows"` 分支；`task_proxy` / `metric` / `best_route` / `icmp_probe` / `rtss_compat` / `toast` 则在模块级就已锁 Windows。

### console_output.rs — 输出编码解码（全平台）

| 行号 | 项 | 签名 / 值 | cfg |
|---|---|---|---|
| `platform/console_output.rs:11-29` | `decode_with_code_page` | `fn(bytes: &[u8], code_page: u32) -> Option<String>`（私有；调 `MultiByteToWideChar` 18/23 + `String::from_utf16_lossy` 27） | `#[cfg(target_os = "windows")]` |
| `platform/console_output.rs:31-48` | `decode_console_bytes` | `pub fn(bytes: &[u8]) -> String`：严格 UTF-8 成功即原样返回（38-40），否则取 `GetOEMCP()`（41）解码，最后兜底 `from_utf8_lossy`（47） | `#[cfg(target_os = "windows")]` |
| `platform/console_output.rs:50-53` | `decode_console_bytes` | `pub fn(bytes: &[u8]) -> String`：直接 `from_utf8_lossy` | `#[cfg(not(target_os = "windows"))]` |
| `platform/console_output.rs:58-72` | `decode_charset_bytes` | `pub fn(bytes: &[u8], charset: Option<&str>) -> String`：charset 为 `gb*` / 含 `936` / `csgb2312` 时**硬编码按 936** 解码（63-69，匹配 65、解码 66），否则转 `decode_console_bytes`（71） | 无 cfg（内部按 `target_os` 分支；非 Windows 用 60-61 行显式消费 `charset` 避免 unused 警告） |
| `platform/console_output.rs:74-113` | `mod tests` | 3 个用例：`gbk_bytes_decode_to_expected_keyword`（83-94，UTF-8 系统上 OEM=65001 解出 U+FFFD 时只验不 panic，88-93）、`charset_gbk_forces_code_page_936`（100-105，断言 `gbk`/`GB2312`/`gb18030` 三种写法 102-104）、`utf8_bytes_pass_through`（108-112） | `#[cfg(test)]`（前两个用例内层再带 `#[cfg(target_os = "windows")]`） |

### autostart.rs — 开机自启（HKCU 注册表）

| 行号 | 项 | 签名 / 值 | cfg |
|---|---|---|---|
| `platform/autostart.rs:1` | `AUTOSTART_REG_KEY` | `const &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run"`（私有） | 无 |
| `platform/autostart.rs:2` | `AUTOSTART_REG_VALUE` | `const &str = "CampusLogin"`（私有） | 无 |
| `platform/autostart.rs:4-15` | `get_auto_launch_enabled` | `pub fn() -> bool`：`HKEY_CURRENT_USER` 下读 `CampusLogin` 值，非空即启用 | 无 |
| `platform/autostart.rs:17-30` | `set_auto_start` | `pub fn(exe_path: &str) -> Result<(), String>`：写值 `"<exe_path>" --autostart`（25 行） | 无 |
| `platform/autostart.rs:32-45` | `remove_auto_start` | `pub fn() -> Result<(), String>`：删除值；`NotFound` 视为成功（42 行） | 无 |

实现依赖 `winreg`（HKCU，无需管理员）。命令行尾部追加 `--autostart`。命令层调用：`commands/system.rs:40`（`get_auto_launch`）、`commands/system.rs:51/53`（`set_auto_launch` 写/删，先注册表后配置，56-63）、`commands/system.rs:125`（`get_init_data` 聚合 `isAutoStart`）。

### dns_config.rs — DNS / DoH（Windows）

公开常量（全部 `#[cfg(target_os = "windows")]`）：

| 行号 | 常量 | 值 |
|---|---|---|
| `platform/dns_config.rs:2` | `PRIMARY_DNS` | `"223.5.5.5"`（阿里公共 DNS） |
| `platform/dns_config.rs:4` | `SECONDARY_DNS` | `"1.12.12.12"`（DNSPod） |
| `platform/dns_config.rs:7` | `PRIMARY_DNS_V6` | `"2400:3200::1"` |
| `platform/dns_config.rs:9` | `SECONDARY_DNS_V6` | `"2402:4e00::"`（腾讯 DNSPod IPv6） |
| `platform/dns_config.rs:12-21` | `DOH_SERVERS: &[(&str, &str)]` | 7 组服务器 IP → DoH 模板映射：`223.5.5.5`/`223.6.6.6`/`2400:3200::1`/`2400:3200:baba::1` → `https://dns.alidns.com/dns-query`；`1.12.12.12`/`120.53.53.53`/`2402:4e00::` → `https://doh.pub/dns-query` |
| `platform/dns_config.rs:24` | `DNS_PROPERTY_TYPE_DOH` | `const i32 = 1`（私有） |
| `platform/dns_config.rs:26-28` | `DNS_SETTING_PROFILE_NAMESERVER` | `const u64 = 0x0200`（私有，带 `#[allow(dead_code)]` + 注释"在 set_profile_dns_via_api 中使用，编译器因条件编译误报"；实际在 `set_dns_stack` 的 profile 分支使用） |
| `platform/dns_config.rs:29-31` | `DNS_SETTING_DOH_PROFILE` | `const u64 = 0x2000`（私有，同上） |

私有函数与枚举：

| 行号 | 项 | 签名 | cfg |
|---|---|---|---|
| `platform/dns_config.rs:39-50` | `doh_bindings` | `fn(dns_servers: &[&str], doh_templates: &[(&str, &str)]) -> Vec<(usize, &str)>`：只为"实际存在于 NameServer 列表且配了模板"的服务器生成 `(ServerIndex, 模板)` | 无 |
| `platform/dns_config.rs:54-59` | `enum DnsTarget` | `{ Interface, Profile }`（`#[derive(Clone, Copy)]`，私有） | `#[cfg(target_os = "windows")]` |
| `platform/dns_config.rs:65-70` | `split_families` | `fn(dns_servers: &[&str]) -> (Vec<&str>, Vec<&str>)`：按是否含 `:` 分 IPv4/IPv6 | 无 |
| `platform/dns_config.rs:72-93` | `set_dns_inner` | `fn(target: DnsTarget, adapter_guid: &str, dns_servers: &[&str], doh_templates: &[(&str, &str)], err_label: &str) -> Result<(), String>`：两个栈分别调用 `set_dns_stack`，错误用 `；` 拼接（92） | `#[cfg(target_os = "windows")]` |
| `platform/dns_config.rs:99-212` | `set_dns_stack` | `fn(target, ipv6: bool, adapter_guid, dns_servers, doh_templates, err_label) -> Result<(), String>`：`elevation::parse_guid`（111）→ 组 `DNS_INTERFACE_SETTINGS3` 并调 `SetInterfaceDnsSettings`（201-208） | `#[cfg(target_os = "windows")]` |
| `platform/dns_config.rs:248-292` | `clear_dns_stack` | `fn(adapter_guid: &str, ipv6: bool) -> Result<(), String>`：NameServer 置空串清栈（`SetInterfaceDnsSettings` 281-288） | `#[cfg(target_os = "windows")]` |

公开函数：

| 行号 | 函数 | 签名 | cfg |
|---|---|---|---|
| `platform/dns_config.rs:214-221` | `set_dns_via_api` | `pub fn(adapter_guid: &str, dns_servers: &[&str], doh_templates: &[(&str, &str)]) -> Result<(), String>`：接口级 DNS+DoH（`DnsTarget::Interface`，err_label `"DNS+DoH"`） | `#[cfg(target_os = "windows")]` |
| `platform/dns_config.rs:223-232` | `set_profile_dns_via_api` | `pub fn(...)` 同上但 `DnsTarget::Profile`（err_label `"ProfileDNS"`）：仅对当前 WiFi 配置文件生效，切换 WiFi 自动切换 DNS | `#[cfg(target_os = "windows")]` |
| `platform/dns_config.rs:234-246` | `clear_adapter_dns_via_api` | `pub fn(adapter_guid: &str) -> Result<(), String>`：先清 IPv6 栈（失败仅警告，242-244）、再清 IPv4 栈（失败上报，245） | `#[cfg(target_os = "windows")]` |
| `platform/dns_config.rs:297-544` | `read_adapter_dns_from_registry` | `pub fn() -> Result<serde_json::Value, String>`：枚举网卡 → 读 DNS 来源与地址 → 解析 `netsh dns show encryption` → 输出 JSON | `#[cfg(target_os = "windows")]` |

`read_adapter_dns_from_registry` 内部的嵌套函数：

| 行号 | 项 | 说明 |
|---|---|---|
| `platform/dns_config.rs:311-331` | `should_filter_ip` | 过滤空串、非点分四段、末段 0/255、`127.*`、`169.254.*`、`198.18.*`/`198.19.*` |
| `platform/dns_config.rs:333-340` | `parse_dns_list` | 按 `,`/空格/`;` 切分并过滤 |
| `platform/dns_config.rs:342-428` | `check_doh_for_ips` | 调 `netsh dns show encryption`（345-348），逐行解析服务器 IP 与模板/自动升级字段，与 `DOH_SERVERS` 内置表合并（403-425），返回 `HashMap<String, (available, enabled, template)>`。IP 识别按"可解析为 `Ipv4Addr`/`Ipv6Addr` 的 token"匹配（365-367，注释 362-364 记录 IPv6 条目解析不到与模板串染的修复），DoH 属性字段名中英双语匹配（386-388），行结尾 flush（395-398） |

DNS 读优先级为 manual > profile > dhcp（456-464）；`ProfileNameServer` 单独读取（454）并计入 profile 地址列表（477-482）。产物：适配器级 JSON 含 `name`/`dnsSource`/`dnsServers`/`profileDnsServers`/`adapterDnsOverridesProfile`（530-536，535 行：manual 且 profile 非空时 true）；顶层 `adapters[]`、`dohSupported: true`（541）、`autoDohEnabled`（539-543）。

`platform/dns_config.rs:546-597` 为 `mod tests`（5 个纯函数用例）：`split_families_partitions_by_colon`（550-556）、`doh_bindings_only_for_present_servers`（558-570，注释 560-561 记录历史缺陷：曾对不在 NameServer 里的服务器也生成槽位；569 断言 `1.12.12.12 → doh.pub`）、`doh_bindings_skips_servers_without_template`（572-579）、`doh_bindings_support_ipv6_servers`（581-589）、`doh_bindings_empty_when_no_match`（591-596）。

### elevation.rs — 管理员判定与 UAC 提权（Windows）

| 行号 | 项 | 签名 | cfg |
|---|---|---|---|
| `platform/elevation.rs:1-24` | `is_admin` | `pub fn() -> bool`：`OpenProcessToken`（9）+ `GetTokenInformation(TokenElevation)`（14-20），`result.is_ok() && elevation != 0`（22）即管理员 | `#[cfg(target_os = "windows")]` |
| `platform/elevation.rs:26-50` | `run_elevated` | `pub fn(cmd: &str, args: &str) -> Result<(), String>`：`ShellExecuteW(None, "runas", cmd, args, None, SW_HIDE)`（36-43），返回值 ≤32 视为失败（45-47） | `#[cfg(target_os = "windows")]` |
| `platform/elevation.rs:52-129` | `shell_exec_elevated` | `pub fn(file: &str, params: &str, hide_window: bool) -> Result<(), String>`：COM 静默提权主路径 | `#[cfg(target_os = "windows")]` |
| `platform/elevation.rs:131-149` | `co_get_object_raw` | `unsafe fn(pszname, pbindoptions, riid, ppv) -> i32`（私有；`#[link(name = "ole32")]` 手写 `CoGetObject` 绑定 139-148） | `#[cfg(target_os = "windows")]` |
| `platform/elevation.rs:151-171` | `parse_guid` | `pub(crate) fn(s: &str) -> Result<windows::core::GUID, String>`：解析带/不带花括号的 GUID（5 段校验 155-157、data4 16 hex 162-164），供 `dns_config.rs:111`、`dns_config.rs:253` 使用 | `#[cfg(target_os = "windows")]` |
| `platform/elevation.rs:174-185` | `struct ICMLuaUtilVtbl` | `#[repr(C)]` 手工 vtable（私有，见下节字段表） | 无 cfg |
| `platform/elevation.rs:188-212` | `mod tests` | `vtbl_shell_exec_slot_is_9`（197-201，`offset_of!` 断言 ShellExec 在 slot 9，UACMe elvint.h 权威布局）、`shell_exec_elevated_smoke`（206-211，`#[ignore]` 真实提权冒烟 `cmd.exe /c exit`，本机手动 `cargo test -- --ignored`） | `#[cfg(test)]` |

COM 提权链细节（`shell_exec_elevated`，53-129）：`CoInitializeEx(COINIT_APARTMENTTHREADED)`（65，按成功与否决定是否配对 `CoUninitialize`，失败路径 93-95/116-119/123-125）→ moniker `Elevation:Administrator!new:{3E5FC7F9-9A51-4367-9063-A120244FBEC7}`（67）→ IID `{6EDD6D74-C007-4E75-B76A-E5740995E24C}`（70-72）→ **`BIND_OPTS3{cbStruct=sizeof(BIND_OPTS3)（79-80）, dwClassContext=CLSCTX_LOCAL_SERVER（81）}`**（历史缺陷：曾传 `BIND_OPTS` 16 字节，cbStruct 过小被 appinfo 按"提升激活配置无效"拒绝 0x80080017，静默提权从未通过；注释 74-77）→ `CoGetObject`（85-90）→ 取 vtable 调 `shell_exec(file, params, NULL, 0, n_show)`（105-112）→ `release`（114）。详见 `learnings/cmstplua-elevation-bind-opts3-and-vtable-slot`。

### gpu.rs — 硬件探测与 WebView2 参数

| 行号 | 项 | 签名 / 值 | cfg |
|---|---|---|---|
| `platform/gpu.rs:4-12` | `struct GpuInfo` | `#[derive(Serialize)]`，6 字段（见下节） | 无 |
| `platform/gpu.rs:14` | `GPU_CACHE` | `static OnceLock<GpuInfo>`（私有；进程内一次探测） | 无 |
| `platform/gpu.rs:16-69` | `determine_tier` | `fn(vendor: &str, model: &str, _vram_mb: u64) -> String`：返回 `discrete` / `high-igpu` / `mid-igpu` / `low-igpu` / `unknown`。NVIDIA→`discrete`（20-22）；Intel：`arc`→`discrete`（24-27）、`iris`+`xe`→`mid-igpu`（28-30）、UHD 770/768/765/750/730→`mid-igpu`（31-36）、其余 UHD→`low-igpu`（37-39）、HD→`low-igpu`（40-42）、兜底 `low-igpu`（43）；AMD：`rx`/`pro`/`radeon pro`/`radeon rx`→`discrete`（46-49）、780m/760m/880m/890m→`high-igpu`（50-52）、680m/660m→`mid-igpu`（53-55）、Vega 10/11→`mid-igpu`（56-58）、`radeon graphics`→`mid-igpu`（59-61）、其余 Vega→`low-igpu`（62-64）、兜底 `mid-igpu`（65）；未知厂商兜底 `unknown`（68） | 无 |
| `platform/gpu.rs:71-73` | `is_integrated_tier` | `fn(tier: &str) -> bool`：`matches!(tier, "low-igpu" \| "mid-igpu" \| "high-igpu")` | 无 |
| `platform/gpu.rs:75-102` | `read_gpu_preference` | `fn() -> u8`：读 `HKCU\Software\Microsoft\DirectX\UserGpuPreferences`（80）中当前 exe 路径的值，解析 `GpuPreference=<digit>`（93-99），无则 0 | 无 |
| `platform/gpu.rs:104-230` | `detect_gpu_info_inner` | `fn() -> GpuInfo`：`CreateDXGIFactory1`（127）+ `EnumAdapters1` 循环（140-201） | 无 |
| `platform/gpu.rs:232-234` | `detect_gpu_info` | `pub fn() -> &'static GpuInfo`：`GPU_CACHE.get_or_init(detect_gpu_info_inner)` | 无 |
| `platform/gpu.rs:236-248` | `build_browser_args` | `pub fn() -> String`：恒返回 `"--js-flags=--max-old-space-size=512"`（247） | 无 |
| `platform/gpu.rs:250-277` | `detect_display_refresh_rate` | `pub fn() -> u32`：`EnumDisplaySettingsW(ENUM_CURRENT_SETTINGS)`（261-267）取 `dmDisplayFrequency`，失败返回 0（273-276） | 无 |

`detect_gpu_info_inner` 的选择逻辑：跳过 `DXGI_ADAPTER_FLAG_SOFTWARE`（151-154），按 `VendorId` 归类（164-169：`0x10DE` NVIDIA、`0x8086` Intel、`0x1002`/`0x1022` AMD、其余 `Unknown({vendor_id:#06X})`），分别记录 `best_integrated` / `best_nvidia` / `best_amd_discrete` / `best_other`（174-200），再按 `gpu_preference` 选主（203-207：1=核显优先 other→nvidia→amd，2 与默认同为独显优先 nvidia→amd→other→integrated），最后 `determine_tier` 定级并算 `is_integrated`（217-218）。日志穿插在 111-116（偏好）、130（工厂失败，返回 fallback 结构 118-125）、211-214（无已知厂商）、220（探测结果）。

### helper_spawn.rs — 提权 helper 三级通道与结果轮询

模块注释 1-13 记录三级通道设计与"结果文件名统一由 `new_result_name` 生成、worker 侧经 `resolve_result_path` 收口到固定目录"的协议约束。

| 行号 | 项 | 签名 / 说明 |
|---|---|---|
| `platform/helper_spawn.rs:21-28` | `new_result_name` | `pub fn() -> String`：`r-<pid>-<纳秒>.json`（纯文件名，写入 `%ProgramData%\CampusLogin\results\` 固定目录；P0-2 收口——高权限 worker 绝不接受任意路径写） |
| `platform/helper_spawn.rs:31-43` | `read_helper_result` | 私有 `fn(content, result_path)`：解析结果 JSON、`logs` 并入日志（34-40）、删除结果文件（41） |
| `platform/helper_spawn.rs:51-92` | `spawn_elevated_helper` | `pub fn(op, args: &[&str], result_name: &str, timeout, allow_uac_prompt: bool) -> Result<Value, String>`：**三级通道**——①代理可用（`task_proxy::proxy_usable` 59）时直走 `run_via_task`（62），失败失效代理缓存（67）；②未就绪时 `ensure_registered`（75，经提权注册+自检）后再试 `run_via_task`（78），失败失效缓存（82）；③兜底 `spawn_elevated_raw`（91） |
| `platform/helper_spawn.rs:98-143` | `spawn_elevated_raw` | 直走「CMSTPLUA 静默 → runas 弹 UAC」链（代理注册动作自身 96 注释、与代理不可用兜底用，防递归）：拼 `--helper <op>` 引号参数（110-115）+ ` --result "<result_name>"`（117）→ `elevation::shell_exec_elevated`（120）失败时 `allow_uac_prompt=false` 直接报错（122-124）→ `run_elevated`（125）→ `helper::resolve_result_path`（129）→ 100ms 轮询结果文件（131-136）→ 超时兜底再查一次（137-141）→ Err"提权操作超时，未收到helper结果"（142） |

调用方：`commands/network_cmd.rs:352`（`setup_dns_doh` 的 `dns` op）与 `:449`（`reset_dns` 的 `clear_dns` op）、`network/dhcp.rs:290`（MAC 重置）、`network/adapter_cache.rs:176`（enable_adapter op）、`network/discovery/devnode.rs:125`（enable_device op）、`monitor/scheduled.rs:421`（出站切换动作通道 `run_helper_op`，414-417 注释：六期起出站动作不止写跃点，禁用网卡/加删路由同走此通道）。helper 侧实现见 `tauri-app/src-tauri/src/helper/mod.rs`（结果目录 `helper_results_dir` 33 / 请求目录 `helper_requests_dir` 38 / 路径收口 `resolve_result_path` 46-57）。

### task_proxy.rs — 计划任务提权代理（Windows，提权首选层）

SYSTEM 主体 + RunLevel=Highest 哑任务 `CampusLoginPowerOps`，action 固定 `自身exe --helper-task`；主进程「写请求文件（`%ProgramData%\CampusLogin\requests\`，唯一名 + 同目录 tmp rename 原子落盘）→ `schtasks /run`（普通权限）→ 轮询 results 目录」，全程零 UAC。模块注释 1-16 记录设计决策与安全边界（显式 SDDL、零触发器、路径收口 `helper::resolve_result_path`、退避策略），完整版见 `decisions/windows-task-proxy-elevation`。

| 行号 | 项 | 说明 |
|---|---|---|
| `platform/task_proxy.rs:30/32` | `TASK_NAME` / `TASK_ARGS` | `pub const "CampusLoginPowerOps"` / `"--helper-task"` |
| `platform/task_proxy.rs:35` | `TASK_SDDL` | `D:P(A;;GRGX;;;BU)(A;;FA;;;BA)(A;;FA;;;SY)`（Users 仅可触发，/change /delete 被拒——评审实测） |
| `platform/task_proxy.rs:37-38` | `CLSID_TASK_SCHEDULER` | COM 类 ID |
| `platform/task_proxy.rs:40/42` | `CHECK_TTL_MS` / `REGISTER_RETRY_BACKOFF_MS` | 30_000 / 600_000 |
| `platform/task_proxy.rs:45-55` | `enum ProxyState` + `PROXY_STATE` | `Unknown` / `Ready` / `Disabled{until_ms}`，`OnceLock<Mutex<...>>` 进程内缓存 |
| `platform/task_proxy.rs:69-75` | `unique_file_name` | `pub fn(prefix) -> "{prefix}-<pid>-<纳秒>.json"`（请求/结果文件唯一名） |
| `platform/task_proxy.rs:80-101` | `proxy_usable` | `pub fn() -> bool`：Unknown 时只读 `check_task_action`（90-98）检测并缓存 30s，不触发注册 |
| `platform/task_proxy.rs:107-152` | `ensure_registered` | `pub fn`：管理员直跑 COM 注册（108-109）；否则经提权链跑 `register_task` op（115-121，60s，`allow_uac_prompt=true`）→ `run_via_task("selfcheck", [], 20s)` 自检（135）→ 成功置 Ready（137）、失败退避（141/148） |
| `platform/task_proxy.rs:156-163` | `enum RegistrationProbe` | `Ready` / `NeedsRegister` / `Disabled`（注册失败 10min 退避，防 UAC 弹窗风暴） |
| `platform/task_proxy.rs:166-189` | `check_registration_state` | 只读检测（`query_task` + `check_task_action`） |
| `platform/task_proxy.rs:192-198` | `invalidate_proxy_cache` / `disable_for_backoff` | `pub` 缓存失效 / +600s 禁用 |
| `platform/task_proxy.rs:202-252` | `run_via_task` | `pub fn(op, args, timeout)`：写请求 `{op, args, result}`（206），唯一名 + tmp rename（208-211），`schtasks /run /tn`（214-217，失败删请求并带 stderr 详情 218-227，220 行 `decode_console_bytes`），100ms 轮询 `helper_results_dir().join(result_name)`（230-251，超时删请求 247-248） |
| `platform/task_proxy.rs:255-290` | `check_task_action` | 校验任务 action == 当前 exe（路径不区分大小写）且参数 `--helper-task`（283）——路径漂移防护 |
| `platform/task_proxy.rs:293-308` | `query_task` | 取 `IRegisteredTask` |
| `platform/task_proxy.rs:312-378` | `register_task_via_com` | `ITaskFolder::RegisterTaskDefinition`（366-374）：SYSTEM 主体 S-1-5-18（323）、`TASK_LOGON_SERVICE_ACCOUNT`（326）、`TASK_RUNLEVEL_HIGHEST`（329）、**零触发器**、`AllowDemandStart`（334）、电池条件关闭（337/340）、`ExecutionTimeLimit=PT5M`（343）、`MultipleInstances=IgnoreNew`（346）、ExecAction `SetPath`/`SetArguments`（351-360）、显式 SDDL |
| `platform/task_proxy.rs:381-416` | `ComScope` | RAII：`CoInitializeEx(APARTMENTTHREADED)`（403-408，S_FALSE 也算成功），Drop 配对 `CoUninitialize`（410-416） |
| `platform/task_proxy.rs:385-400` | `mod tests` | `task_proxy_register_and_selfcheck_smoke`（395-399，`#[ignore]` 真机注册+自检冒烟） |

worker 端（`helper/mod.rs` `--helper-task` 模式，`main.rs:30` 最先拦截）：扫描请求目录取最旧请求 → 读入内存后立即删请求（防重复执行）→ `build_op_from_args`（与 `--helper` 命令行共用 op 构造，helper/mod.rs:179/181）→ 执行 → 结果写固定目录。op 白名单：`dns` / `clear_dns` / `mac` / `enable_adapter`（适配器名双校验+存在性）/ `enable_device`（实例 ID 字符集白名单防 pnputil 开关注入 + devnode 存在性 + problem 22 解除复核）/ `register_task`（helper/mod.rs:964 `run_register_task` → `task_proxy::register_task_via_com` 966）/ `selfcheck`。

### identity.rs — Windows Hello 身份验证

| 行号 | 项 | 签名 / 值 | cfg |
|---|---|---|---|
| `platform/identity.rs:24` | `DEFAULT_CONSENT_MESSAGE` | `const &str = "请完成 Windows 身份验证"`（私有） | 无 |
| `platform/identity.rs:28` | `LAST_VERIFY_EPOCH_SECS` | `static AtomicU64`（私有；0 = 从未验证） | 无 |
| `platform/identity.rs:31` | `IDENTITY_VERIFY_TTL_SECS` | `pub const u64 = 600` | 无 |
| `platform/identity.rs:34-36` | `note_identity_verified` | `pub fn()`：写入当前 epoch 秒 | 无 |
| `platform/identity.rs:39-42` | `identity_verified_recently` | `pub fn() -> bool` | 无 |
| `platform/identity.rs:45-47` | `is_within_ttl` | `fn(verified_at: u64, now: u64, ttl_secs: u64) -> bool`（私有纯函数；要求 `verified_at > 0 && now >= verified_at`，防时钟回拨） | 无 |
| `platform/identity.rs:49-54` | `epoch_secs_now` | `fn() -> u64`（私有） | 无 |
| `platform/identity.rs:60-70` | `verify_identity` | `pub async fn(consent_message: &str, owner_hwnd: Option<isize>) -> Result<(), String>`：空文案回退 `DEFAULT_CONSENT_MESSAGE`（64-68） | 无 |
| `platform/identity.rs:78-117` | `verify_hello` | `async fn(...)`（私有）：`CheckAvailabilityAsync`（83-88）→ interop 主路径（97-102）→ 兜底路径（104-116） | 无 |
| `platform/identity.rs:123-149` | `try_verification_for_window` | `async fn(hwnd: isize, message: &str) -> Option<Result<(), String>>`（私有）：`None` = interop 不可用需回退；`Some` = 最终结论（含用户取消）。interop 块作用域 131-142 不跨 await，`RequestVerificationForWindowAsync(HWND, &HSTRING)`（139） | 无 |
| `platform/identity.rs:155-172` | `spawn_consent_focus_nudger` | `fn()`（私有）：起线程，12 次 × 250ms（160-161）轮询窗口类名 `"Credential Dialog Xaml Host"` 并 `SetForegroundWindow`（`FindWindowW` 164 + `is_invalid` 165 + 166） | 无 |
| `platform/identity.rs:176-204` | `await_winrt_operation` | `async fn<T: RuntimeType + Send>(op: IAsyncOperation<T>) -> Result<T, String>`（私有）：`SetCompleted` 回调 + `tokio::sync::oneshot`（182-199），非阻塞等待 | 无 |
| `platform/identity.rs:206-223` | `mod tests` | `identity_ttl_boundary`（211-222）：0 哨兵拒绝（214）、TTL 边界内通过（216-217）、超时 1 秒拒绝（219）、时钟回拨拒绝（221） | `#[cfg(test)]` |

`verify_hello` 调用点：`UserConsentVerifier::CheckAvailabilityAsync()`（83-88）、`RequestVerificationAsync(&HSTRING)`（107）、interop 的 `RequestVerificationForWindowAsync(HWND, &HSTRING)`（139）。设备未配置 Hello 时直接返回引导文案且**不回退凭据对话框**（89-94，模块注释记录 2026-09-05 用户要求移除 CredUI 回退）。线程安全（注释 73-77）：不手动 `CoInitializeEx`——本文件所有 COM 入口都经 `windows::core::factory()`，windows-core 0.58 的 factory_cache 在撞到 `CO_E_NOTINITIALIZED` 时自动 `CoIncrementMTAUsage` 重试自愈（windows-rs#1169）。

### metric.rs — 接口跃点（metric）读写（Windows）

夜间出站自动切换的读写底座：读走 Iphlpapi 运行时值（无需提权），写经 helper `SetMetric` 提权执行。模块注释 1-5 明确：**写路径 `SetIpInterfaceEntry` 落在接口的持久配置上**（与 `netsh set interface` 的 persistent 存储同层，重启不会自动还原），因此切换态快照与还原是必需步骤而非可选优化；切换/还原之间的崩溃残留由启动对账 `apply_outbound_restore`（`monitor/scheduled.rs:965`，启动/多次兜底调用 1297-1357）收敛。快照随配置保存（`outbound_metric_restore`，见 [[desktop-config]]）。

| 行号 | 项 | 签名 / 值 | cfg |
|---|---|---|---|
| `platform/metric.rs:15-20` | `MetricRow` | `pub struct { family: u16, automatic: bool, metric: u32 }`（`#[derive(Debug, Clone, Serialize)]`；`family` 取 `AF_INET=2` / `AF_INET6=23`，17 行注释） | 模块级 `all(desktop, target_os = "windows")` |
| `platform/metric.rs:25-53` | `interface_rows_for_guid` | `pub(crate) fn(guid: &str) -> Result<Vec<MIB_IPINTERFACE_ROW>, String>`：`elevation::parse_guid` 解析入参 → `GetIpInterfaceTable(AF_UNSPEC)`（30）取全表 → 逐行 `ConvertInterfaceLuidToGuid`（42）与目标 GUID 比对（转换失败的行跳过 43）→ `FreeMibTable`（50）释放 → 返回**原始 Win32 行**。GUID 无匹配行返回空表（非错误） | 同上 |
| `platform/metric.rs:56-65` | `read_interface_metrics` | `pub fn(guid: &str) -> Result<Vec<MetricRow>, String>`：把原始行映射为 `MetricRow`（`family = row.Family.0`、`automatic = row.UseAutomaticMetric.0 != 0`） | 同上 |
| `platform/metric.rs:67-84` | `mod tests` | 1 个 `#[ignore]` 真机冒烟 `read_interface_metrics_smoke`（76-83）：断言非空（81）且 `family ∈ {2, 23}`（82）；`cargo test read_interface_metrics_smoke -- --ignored --nocapture` | `#[cfg(test)]` |

读与写共用 `interface_rows_for_guid`：helper 取到行后**整行改副本**（只覆写 `UseAutomaticMetric` / `Metric` / `SitePrefixLength`）再 `SetIpInterfaceEntry`，等价 mullvad 的 `GetIpInterfaceEntry` + `Set` 两步——`Family` / `InterfaceLuid` / `InterfaceIndex` 等必须为接口当前值，构造残缺结构体会参数校验失败。写入侧实现见 `helper/mod.rs:564-635`（`run_set_metric`），写入侧的坑见 [[set-ip-interface-entry-metric|SetIpInterfaceEntry 写 metric 的必踩点与字段对照]]。

### toast.rs — WinRT Toast（Windows 桌面）

| 行号 | 项 | 签名 / 值 | cfg |
|---|---|---|---|
| `platform/toast.rs:25-31` | AUMID 常量组 | `AUMID="com.campus.login"`（自有）、`FALLBACK_APP_ID`（PowerShell AUMID 兜底）、`APP_DISPLAY_NAME="校园网登录助手"`、`AUMID_ICON_RESOURCE="icons/128x128.png"` | 无（模块级 `cfg(all(desktop, target_os = "windows"))`） |
| `platform/toast.rs:34-46` | `NOTIFIER_AUMID` + `notifier_aumid` | `static OnceLock<&'static str>` + `fn(app_handle) -> &'static str`：进程内只探一次，`register_aumid` 成功用自有 AUMID、失败 log_warn 回退 FALLBACK | 无 |
| `platform/toast.rs:51-67` | `register_aumid` | `fn(app_handle) -> Result<(), String>`：HKCU `Software\Classes\AppUserModelId\<AUMID>`（54）写 `DisplayName`（56）+ `IconUri`（仅 resource_dir 下 icons/128x128.png 存在才写，58-64；dev 无图标只缺角标），免提权 | 无 |
| `platform/toast.rs:70-77` | `resolve_resource_image` | `fn(app_handle: &AppHandle, relative: &str) -> Option<String>`（私有）：`resource_dir().join(relative)` 存在则转 `file:///` URL（反斜杠替换为正斜杠，75-76），否则 `None` | 无 |
| `platform/toast.rs:81-94` | `show_system_toast` | `pub fn(app_handle: &AppHandle, title: &str, body: &str, mascot: &str) -> Result<(), String>`：`resources/mascot-toast/<mascot>.png`（82）作 `appLogoOverride hint-crop="circle"`（83），`XmlDocument` + `LoadXml`（88-89）→ `ToastNotification::CreateToastNotification`（90）→ `CreateToastNotifierWithId(notifier_aumid(..))`（91-92）→ `Show`（93） | 无 |
| `platform/toast.rs:98-129` | `show_update_toast` | `pub fn(app_handle: &AppHandle, version: &str) -> Result<(), String>`：`resources/mascot-update-toast.png`（101）作头像，固定文案"发现新版本 / 新版本 v{version} 可用，前往关于界面进行更新"（105），注册 `Activated` 回调（112-124）唤起主窗口并向前端发 `update-notification-click` 事件 | 无 |

## 结构体与字段

### `GpuInfo`（`platform/gpu.rs:4-12`）

| 字段 | 类型 | 含义 |
|---|---|---|
| `vendor` | `String` | 厂商名：`"NVIDIA"` / `"Intel"` / `"AMD"` / `"Unknown(0xXXXX)"`（164-169）/ `"unknown"`（兜底，118-125） |
| `model` | `String` | DXGI `Description` 去尾部 `\0`（157-159） |
| `vram_mb` | `u64` | `DedicatedVideoMemory / 1024 / 1024`（160-161） |
| `is_integrated` | `bool` | 由 `tier` 推导（71-73、218） |
| `tier` | `String` | `discrete` / `high-igpu` / `mid-igpu` / `low-igpu` / `unknown` |
| `gpu_preference` | `u8` | 读自注册表：0=系统默认、1=节能(核显)、2=高性能(独显)（111-116 的日志映射） |

序列化后即 `get_gpu_info` / `get_init_data.gpuInfo` 的返回体。

### `ICMLuaUtilVtbl`（`platform/elevation.rs:174-185`）

| 字段 | 类型 | 说明 |
|---|---|---|
| `_query_interface` | `usize` | slot 0（未调用） |
| `_add_ref` | `usize` | slot 1（未调用） |
| `release` | `unsafe extern "system" fn(*mut c_void) -> u32` | slot 2（177） |
| `_slots_3_to_8` | `[usize; 6]` | slot 3~8 占位对齐：SetRasCredentials / SetRasEntryProperties / DeleteRasEntry / LaunchInfSection / LaunchInfSectionEx / CreateLayerDirectory（183，注释 178-182） |
| `shell_exec` | `unsafe extern "system" fn(*mut c_void, *const u16, *const u16, *const u16, u32, u32) -> HRESULT` | **slot 9**（184；真实布局以 UACMe elvint.h 的 ICMLuaUtilVtbl 为准） |

历史缺陷（注释 178-182 记录）：slot 3~8 曾被当作“纯占位”砍掉，`shell_exec` 错排到 slot 4——slot 4 实为 SetRasEntryProperties，参数 marshaling 不匹配报 `RPC_X_BAD_STUB_DATA`（0x800706F4）。`vtbl_shell_exec_slot_is_9`（`platform/elevation.rs:197-201`）用 `offset_of!` 锁死该布局。

### `DnsTarget`（`platform/dns_config.rs:54-59`）

| 变体 | 说明 |
|---|---|
| `Interface` | 写 `NameServer` + `DNS_SETTING_NAMESERVER` flags，DoH 属性进 `ServerProperties`（`DNS_SETTING_DOH`） |
| `Profile` | 写 `ProfileNameServer` + `DNS_SETTING_PROFILE_NAMESERVER` flags，DoH 属性进 `ProfileServerProperties`（`DNS_SETTING_DOH_PROFILE`） |

### `DNS_INTERFACE_SETTINGS3` 填充字段（`platform/dns_config.rs:181-198`）

| 字段 | 置值 | 说明 |
|---|---|---|
| `Version` | `DNS_INTERFACE_SETTINGS_VERSION3` | 必须匹配结构体版本 |
| `Flags` | `ns_flag`（按 target，160-163）\| 可能的 `DNS_SETTING_IPV6`（165-167）\| 可能的 DoH flag（168-173） | 一次调用只作用一个协议栈 |
| `Domain` | `PWSTR::null()` | 不改 |
| `NameServer` | Interface 时为 `ns_wide` 指针，否则 null（154-158） | 逗号分隔的地址串（113-114） |
| `SearchList` | `PWSTR::null()` | 不改 |
| `RegistrationEnabled` / `RegisterAdapterName` / `EnableLLMNR` / `QueryAdapterName` | 全 0 | 不改 |
| `ProfileNameServer` | Profile 时为 `ns_wide` 指针，否则 null | 按 WiFi profile 生效 |
| `DisableUnconstrainedQueries` | 0 | 不改 |
| `SupplementalSearchList` | `PWSTR::null()` | 不改 |
| `cServerProperties` / `ServerProperties` | 按 target 二选一赋 `doh_bindings` 指针与长度（175-179） | 未使用的槽恒为 NULL |
| `cProfileServerProperties` / `ProfileServerProperties` | 同上互补 | — |

`clear_dns_stack` 里的同结构体只需 `Flags = DNS_SETTING_NAMESERVER [| DNS_SETTING_IPV6]` 与 `NameServer = 空串`（248-292 内）。

### DNS/DoH 相关的进程内枚举与常量

`platform/dns_config.rs:132-135` 构造的 `DNS_DOH_SERVER_SETTINGS.Flags` 固定为 `DNS_DOH_SERVER_SETTINGS_ENABLE_AUTO | DNS_DOH_SERVER_SETTINGS_ENABLE | DNS_DOH_SERVER_SETTINGS_FALLBACK_TO_UDP`（自动升级 + 启用 + 回退 UDP）；两阶段组参保证宽字符串指针稳定性（注释 139-141）。

## Data Flow

### 控制台输出解码（多模块共用入口）

```text
netsh / ipconfig / wmic / route / pnputil 子进程 stdout/stderr（OEM 代码页 936 或 65001）
  → platform/console_output.rs:32 decode_console_bytes
       ├─ std::str::from_utf8 成功 → 原样返回（38-40）
       ├─ 失败 → GetOEMCP()（41）→ MultiByteToWideChar（18/23）→ from_utf16_lossy
       └─ 再失败 → from_utf8_lossy 兜底（47）
```

调用方：`network/subnet.rs:58/100`（stdout）、`network/adapter_cache.rs:163`（stderr）、`network/dhcp.rs:27-64`、`network/dns_setup.rs:126-127`、`platform/dns_config.rs:350`（`netsh dns show encryption`）、`platform/task_proxy.rs:220`（schtasks stderr）、`network/discovery/devnode.rs:147-149`、`helper/mod.rs:684/739/850-851`（helper 子进程自身也解码）。HTTP 响应体走 `decode_charset_bytes`（`auth/protocol.rs:33/60/183`，按 `Content-Type` 声明优先 936）。

### DNS + DoH 设置链（含三级提权通道）

```text
前端 invoke('setup_dns_doh', {family})
  → commands/network_cmd.rs:300 setup_dns_doh（参数规整 + 目标白名单）
  → platform/elevation.rs:2 is_admin()
      ├─ 管理员：network::dns_setup::setup_dns_doh_admin（network_cmd.rs:343）
      │     → platform/dns_config.rs:214 set_dns_via_api / 223 set_profile_dns_via_api
      │         → 72 set_dns_inner → 65 split_families → 99 set_dns_stack → 201 SetInterfaceDnsSettings
      └─ 非管理员：platform/helper_spawn.rs:21 new_result_name()
            → 51 spawn_elevated_helper("dns", [adapters…, "--family", family], name, 30s)
                 ├─ ① 59 proxy_usable → 62 task_proxy::run_via_task
                 │     （写请求文件 → schtasks /run → 轮询结果，全链零 UAC）
                 ├─ ② 75 ensure_registered（提权注册+selfcheck 自检）→ 78 run_via_task
                 └─ ③ 91 spawn_elevated_raw（98-143）
                      ├─ 120 elevation::shell_exec_elevated(exe, params, true)
                      │     → CoInitializeEx（65）→ CoGetObject（85-90）
                      │     → vtable.shell_exec（105-112）→ release（114）→ CoUninitialize
                      └─ 失败 → 125 elevation::run_elevated(exe, params) → ShellExecuteW "runas"（36-43）
            → 子进程 --helper dns（helper/mod.rs HelperOp 分发）写结果文件
            → 131-136 轮询/137-141 补查结果文件 → 31 read_helper_result 解析并删除结果文件
```

`reset_dns`（`commands/network_cmd.rs:388`）的非管理员路径同构：`new_result_name`（447）→ `spawn_elevated_helper("clear_dns", ...)`（449-455，30s）。恢复 DNS 自动获取的管理员路径调 `clear_adapter_dns_via_api`（`network_cmd.rs:424`、`network/dns_setup.rs:52`、`helper/mod.rs:437`）。

DNS 状态读取链：`invoke('check_dns_doh_status')` → `commands/network_cmd.rs:285` → `platform/dns_config.rs:297 read_adapter_dns_from_registry` → 遍历 `HKLM\SYSTEM\CurrentControlSet\Control\Network\{4D36E972-…}` 取网卡名 → 用网卡 GUID 打开 `HKLM\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\<guid>` 读 `NameServer` / `ProfileNameServer` / `DhcpNameServer`（优先级 manual > profile > dhcp，456-464）→ `netsh dns show encryption` 交叉核对 DoH（342-428）→ JSON（530-543）。

### 注册表调用点汇总

| 注册表路径 | 读/写 | 代码位置 |
|---|---|---|
| `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run` 值 `CampusLogin` | 读写删 | `platform/autostart.rs:4-15`、`17-30`、`32-45` |
| `HKCU\Software\Microsoft\DirectX\UserGpuPreferences` 值 `<exe 路径>` | 读 | `platform/gpu.rs:75-102` |
| `HKCU\Software\Classes\AppUserModelId\<AUMID>`（`DisplayName`、`IconUri`） | 写 | `platform/toast.rs:51-67` |
| `HKLM\SYSTEM\CurrentControlSet\Control\Network\{4D36E972-E325-11CE-BFC1-08002BE10318}\<guid>\Connection`（`Name`、`PnpInstanceID`） | 读 | `platform/dns_config.rs:297-544` 内 |
| `HKLM\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\<guid>`（`NameServer`、`DhcpNameServer`、`ProfileNameServer`） | 读 | `platform/dns_config.rs:454`、`456-464`、`477-482` |

### Win32 / WinRT API 调用点汇总

| API | 位置 | 用途 |
|---|---|---|
| `MultiByteToWideChar` / `GetOEMCP` | `platform/console_output.rs:18`、`23`、`41` | 代码页解码 |
| `OpenProcessToken` / `GetTokenInformation(TokenElevation)` / `CloseHandle` | `platform/elevation.rs:9`、`14-20`、`21` | 管理员判定 |
| `ShellExecuteW`（verb `runas`，`SW_HIDE`） | `platform/elevation.rs:36-43` | 降级 UAC 提权 |
| `CoInitializeEx` / `CoUninitialize` / `CoGetObject`（ole32） | `platform/elevation.rs:65`、`94/118/124`、`141-148` | COM 静默提权 |
| `ITaskService` / `RegisterTaskDefinition`（COM 计划任务） | `platform/task_proxy.rs:312-378` | SYSTEM 哑任务注册 |
| `CreateDXGIFactory1` / `EnumAdapters1` / `GetDesc1` | `platform/gpu.rs:127`、`140-201` | GPU 枚举 |
| `EnumDisplaySettingsW(ENUM_CURRENT_SETTINGS)` | `platform/gpu.rs:261-267` | 刷新率 |
| `SetInterfaceDnsSettings`（IpHelper） | `platform/dns_config.rs:201-208`、`281-288` | 写/清 DNS + DoH |
| `GetIpInterfaceTable` / `ConvertInterfaceLuidToGuid` / `FreeMibTable`（IpHelper） | `platform/metric.rs:30`、`42`、`50` | 枚举接口表并按 LUID→GUID 匹配目标接口（读 metric） |
| `SetIpInterfaceEntry`（IpHelper） | `helper/mod.rs:617` | 写 metric（helper 提权上下文，`run_set_metric` 564-635） |
| `UserConsentVerifier::CheckAvailabilityAsync` / `RequestVerificationAsync` | `platform/identity.rs:83-88`、`107` | Windows Hello |
| `IUserConsentVerifierInterop::RequestVerificationForWindowAsync` | `platform/identity.rs:139` | Win11 主路径：Consent 绑定主窗口 HWND |
| `FindWindowW` + `SetForegroundWindow` | `platform/identity.rs:164`、`166` | 兜底：Consent 对话框提前台 |
| `XmlDocument::LoadXml` / `ToastNotification` / `ToastNotificationManager` | `platform/toast.rs:89`、`90/109`、`91-92/126` | 自组 Toast XML 并显示 |

### GPU / WebView2 参数流

```text
main.rs:56   build_browser_args() → "--js-flags=--max-old-space-size=512"（platform/gpu.rs:247）
main.rs:64   追加 Crashpad 参数 --enable-crash-reporter --crash-dumps-dir="<Roaming>\com.campus.login\crashdumps"
main.rs:68   std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", ...)（必须在 Tokio runtime 创建前）
app/startup.rs:201  把 env 实际值写日志
```

GPU 信息流：`app/startup.rs:240-243` 后台 `gpu-warmup` 线程先调 `detect_gpu_info()`（242）与 `detect_display_refresh_rate()`（243）；此后 `commands/system.rs:126`（`get_init_data`）、`:176`（`get_gpu_info`）、`:255` 直接读 `OnceLock` 缓存。

### 身份验证门数据流

```text
前端 invoke('verify_windows_identity', {consentMessage})
  → commands/self_service.rs:260
  → 主窗口 show/set_focus + hwnd() → platform/identity.rs:60 verify_identity(msg, Some(hwnd))
      ├─ 83-88 CheckAvailabilityAsync → 非 Available 直接 Err（89-94）
      ├─ 97-102 interop 主路径 → try_verification_for_window（123-149）
      └─ 104-116 兜底路径：spawn_consent_focus_nudger（155-172）+ RequestVerificationAsync（107）
  → 成功：commands/self_service.rs:274 note_identity_verified() → platform/identity.rs:34 写时间戳
后续敏感命令（reveal 等）→ platform/identity.rs:39 identity_verified_recently()（TTL 600s）
```

敏感命令门实例：`commands/self_service.rs:286 reveal_operator_credential` 在执行前校验 `identity_verified_recently()`（300-304，过期提示"Windows 身份验证已过期，请重新验证后再查看"）。

### Toast 数据流

```text
infra/notification.rs:46 → platform/toast.rs:81 show_system_toast(...)
  失败 → 降级 tauri_plugin_notification 纯文本通知
发现新版本：update/updater.rs:365 → platform/toast.rs:98 show_update_toast
  → 点击 Activated（112-124）→ 显示主窗口（114-117）
    + EventBus::new(..).emit_update_notification_click()（119，infra/events.rs:58）
```

## Connections

- [[desktop-commands]]：命令层是本层的直接上游（`set_auto_launch`→`autostart`、`get_gpu_info`→`gpu`、`setup_dns_doh`/`reset_dns`/`check_dns_doh_status`→`dns_config`+`elevation`+`helper_spawn`、`verify_windows_identity`→`identity`）。
- [[desktop-network-dns]]：DNS/DoH 的业务编排（`network::dns_setup`）与本层的 `dns_config` 的分工边界。
- [[desktop-network-core]]：`network/adapter_cache.rs`、`network/dhcp.rs` 对 `elevation` 与 `helper_spawn` 的另外两处调用（netsh 提权、MAC 重置）。
- [[desktop-helper-update]]：helper 子进程协议（`helper/mod.rs` 的 `HelperOp`、结果文件格式、`resolve_result_path` 收口）与更新包下载/校验的上层流程。
- [[outbound-switch]]：夜间出站切换的判定纯函数层，桌面侧动作经 `monitor/scheduled.rs:418 run_helper_op` 消费本层 `metric.rs` 的读与 helper `SetMetric` 的写；`best_route` / `icmp_probe`（`platform/mod.rs:27-31`）为其路由验证与网关探测底座。
- [[desktop-auth]]：`auth/protocol.rs` 通过 `decode_charset_bytes` 解码 GBK 响应体做成败关键词判定。
- [[desktop-infra]]：`crate::log_*` 宏、`EventBus`（`infra/events.rs:58 emit_update_notification_click`）、`AppHandle` 资源的生命周期。
- [[desktop-app-lifecycle]]：主窗口创建、`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 注入时机（`main.rs:68`）、`gpu-warmup` 预热线程与心跳线程。
- [[desktop-config]]：`outbound_metric_restore` 等配置项如何驱动本层行为（自启、通知、DNS 目标解析、metric 快照还原）。
- [[android-plugins]]：安卓端不编译本层模块（仅继承 `console_output`），等价的 Keystore / 前台服务 / 网络绑定能力由 `android/plugins/*` 提供。

## Known Issues

1. **模块门控只到 `desktop`，不锁 Windows**：`platform/mod.rs:1-36` 对 `autostart` / `ecoqos` / `dns_config` / `elevation` / `gpu` / `helper_spawn` / `identity` 只加 `#[cfg(desktop)]`（带 `target_os = "windows"` 的是 `task_proxy`、`metric`、`best_route`、`icmp_probe`、`rtss_compat`、`toast` 六个模块，`platform/mod.rs:19-36`），而 `autostart.rs`、`gpu.rs`、`helper_spawn.rs`、`identity.rs` **文件内部没有任何 `#[cfg]`**（见逐文件清单），它们直接使用 `winreg` / `windows` crate；项目实际只支持 Windows 桌面 + 安卓，但该约束是事实约定而非编译期保证。
2. ~~**`toast.rs` 借用 PowerShell 的 AUMID**~~（2026-09-27 已解决）：自有 AUMID `com.campus.login` 注册到 HKCU `Software\Classes\AppUserModelId\`（DisplayName + IconUri，免提权），通知中心来源显示应用名与应用图标；注册失败自动回退 PowerShell AUMID。
3. **`identity.rs` 无 CredUI 回退**：`platform/identity.rs:89-94` 设备未配置 Windows Hello 时直接报错退出（模块注释记录 2026-09-05 用户要求移除输密码回退），未配置 Hello 的用户无法执行 reveal 等敏感操作。
4. **兜底路径的焦点轮询只有 3 秒**：`platform/identity.rs:155-172` 固定 12 次 × 250ms 后线程自行结束，若 Consent 对话框出现更晚（慢机/UAC 排队），就没有任何提前台兜底。
5. **身份验证时间戳是进程内全局**：`platform/identity.rs:28` 的 `AtomicU64` 随进程重启归零（0 = 从未验证），且是单用户单进程语义，多实例/多用户场景不共享。
6. **`clear_adapter_dns_via_api` 的 IPv6 清理失败被吞**：`platform/dns_config.rs:242-244` 清除 v6 栈失败仅记 `log_warn`（doc 234-239 说明 "IPV6 flag + 空串" 的 API 接受性未经 Win11 实测确证），可能导致旧静态 v6 DNS 残留而接口级设置覆盖 profile DNS。
7. **`build_browser_args` 只剩单个参数**：`platform/gpu.rs:236-248` 原先 11 个 Chromium 参数被精简为 `--js-flags=--max-old-space-size=512`（247），注释（237-246）逐项记录了删除理由；若日后确有性能增益需按 `ponytail:` 注释（244 行）逐项实测后加回。
8. **`GPU_CACHE` 进程内不可刷新**：`platform/gpu.rs:14` 的 `OnceLock` 意味着运行期切换显卡偏好/热插拔不会反映到 `get_gpu_info` 结果，需重启应用。
9. **`detect_gpu_info_inner` 的偏好分支冗余**：`platform/gpu.rs:203-207` 中 `gpu_preference == 2` 与默认分支（`_`）的选择顺序完全相同，实际只有 1（核显优先）与"其他"两种行为。
10. **注册表读取依赖 `netsh` 输出格式**：`platform/dns_config.rs:342-428` 逐行解析中英双语字段名（386-388 的 `autoupgrade`/`自动升级`），`netsh` 输出格式或系统语言变化会直接影响 `dohEnabled` 判定。IPv6 条目的 token 解析已修复（365-367，按 `Ipv4Addr`/`Ipv6Addr` 可解析匹配），但整体格式依赖仍在。
11. **`helper_spawn` 无进程存活检查**：`platform/helper_spawn.rs:131-141` 只轮询结果文件，不感知提权子进程提前退出（例如用户在 UAC 弹窗点"否"），会一直等到超时（调用方 30s）才返回错误。
12. **COM 提权依赖未公开接口**：`platform/elevation.rs:67-72` 使用 `Elevation:Administrator!new:{3E5FC7F9-…}` 与手写 IID `{6EDD6D74-…}`，属未文档化 COM 提权路径；Windows 更新后失效时的表现是降级为弹 UAC（`platform/helper_spawn.rs:120-125`），不会静默失败。
13. **`should_filter_ip` 会丢弃合法的 `198.18/198.19` 内网 DNS**：`platform/dns_config.rs:311-331` 把 `198.18.*`、`198.19.*` 一并过滤（原意是剔除基准测试网段），特殊校园网部署可能显示不出真实 DNS。
14. **`read_adapter_dns_from_registry` 的 `dohSupported` 恒为 `true`**：`platform/dns_config.rs:541` 硬编码，未做系统能力探测。
15. **写 metric 必须把 `SitePrefixLength` 置 0，且只能整行改副本**：`helper/mod.rs:564-635` 从 `interface_rows_for_guid` 取整行 → 覆写 `UseAutomaticMetric` / `Metric` / `SitePrefixLength`（613-616）→ `SetIpInterfaceEntry`（617）。`SitePrefixLength` 非 0 时 Win32 直接报 `ERROR_INVALID_PARAMETER`（615 行注释），错误信息不指向任何具体字段；`Family` / `InterfaceLuid` / `InterfaceIndex` 等若由调用方自行拼装（而非拷贝当前行）同样会参数校验失败。细节与字段对照见 [[set-ip-interface-entry-metric]]。
16. **`read_interface_metrics` 对不存在的 GUID 返回空表而非错误**：`platform/metric.rs:25-53` 匹配不到即返回 `Ok(vec![])`，调用方须自行区分"该接口无此行"与"接口枚举失败"，否则会把空表当成"metric 已还原"。
