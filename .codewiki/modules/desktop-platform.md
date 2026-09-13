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
  - tauri-app/src-tauri/src/platform/identity.rs
  - tauri-app/src-tauri/src/platform/toast.rs
tags: [desktop, windows, win32, winrt, registry, uac, 平台层]
---

## Overview

`tauri-app/src-tauri/src/platform/` 是桌面端全部"与操作系统直接打交道"的代码：控制台输出与 HTTP 响应体解码（GBK/OEM 代码页）、注册表读写（开机自启、适配器 DNS 读取）、Win32 DNS 接口调用（`SetInterfaceDnsSettings` + DoH 属性）、UAC 提权（COM `ICMLuaUtil` 静默提权与 `ShellExecuteW runas` 降级）、提权 helper 子进程的启动与结果轮询、DXGI/GDI 硬件信息探测、Windows Hello 身份验证、WinRT Toast 通知。

该层不持有业务状态（唯一例外是 `identity.rs` 的进程内验证时间戳），被 `commands/`、`network/`、`auth/`、`update/`、`app/` 调用；`platform/mod.rs:1-8` 用 `#[cfg(desktop)]` 把除 `console_output` 外的模块整体排除在安卓构建之外（安卓通过 Cargo path 依赖桌面 crate，只继承 `console_output` 等跨平台部分）。

## Key Components

### mod.rs — 模块门控

| 行号 | 内容 | cfg |
|---|---|---|
| `platform/mod.rs:1-2` | `pub mod console_output;`（注释：协议响应/子网查询共用，安卓侧同样需要 GBK 解码） | 无（全平台编译） |
| `platform/mod.rs:5-6` | `pub mod autostart;` | `#[cfg(desktop)]` |
| `platform/mod.rs:7-8` | `pub mod dns_config;` | `#[cfg(desktop)]` |
| `platform/mod.rs:9-10` | `pub mod elevation;` | `#[cfg(desktop)]` |
| `platform/mod.rs:11-12` | `pub mod gpu;` | `#[cfg(desktop)]` |
| `platform/mod.rs:13-14` | `pub mod helper_spawn;` | `#[cfg(desktop)]` |
| `platform/mod.rs:15-16` | `pub mod identity;` | `#[cfg(desktop)]` |
| `platform/mod.rs:17-18` | `pub mod toast;` | `#[cfg(all(desktop, target_os = "windows"))]` |

注意：模块级门控只到 `desktop`；`autostart` / `gpu` / `helper_spawn` / `identity` / `toast` 内部**没有任何 `#[cfg]` 属性**（见下方逐文件清单），它们只在 Windows 桌面可用是"事实约束"而非编译期约束；`dns_config` / `elevation` / `console_output` 则在函数级带 `target_os = "windows"` 分支。

### console_output.rs — 输出编码解码（全平台）

| 行号 | 项 | 签名 / 值 | cfg |
|---|---|---|---|
| `platform/console_output.rs:11-29` | `decode_with_code_page` | `fn(bytes: &[u8], code_page: u32) -> Option<String>`（私有；调 `MultiByteToWideChar` + `String::from_utf16_lossy`） | `#[cfg(target_os = "windows")]` |
| `platform/console_output.rs:31-48` | `decode_console_bytes` | `pub fn(bytes: &[u8]) -> String`：严格 UTF-8 成功即原样返回，否则取 `GetOEMCP()` 解码，最后兜底 `from_utf8_lossy` | `#[cfg(target_os = "windows")]` |
| `platform/console_output.rs:50-53` | `decode_console_bytes` | `pub fn(bytes: &[u8]) -> String`：直接 `from_utf8_lossy` | `#[cfg(not(target_os = "windows"))]` |
| `platform/console_output.rs:58-72` | `decode_charset_bytes` | `pub fn(bytes: &[u8], charset: Option<&str>) -> String`：charset 为 `gb*` / 含 `936` / `csgb2312` 时**硬编码按 936** 解码（62-70），否则转 `decode_console_bytes`（71） | 无 cfg（内部按 `target_os` 分支；非 Windows 用 60-61 行显式消费 `charset` 避免 unused 警告） |
| `platform/console_output.rs:74-113` | `mod tests` | 3 个用例：`gbk_bytes_decode_to_expected_keyword`（82-94）、`charset_gbk_forces_code_page_936`（100-105）、`utf8_bytes_pass_through`（108-112） | `#[cfg(test)]`（前两个用例内层再带 `#[cfg(target_os = "windows")]`） |

### autostart.rs — 开机自启（HKCU 注册表）

| 行号 | 项 | 签名 / 值 | cfg |
|---|---|---|---|
| `platform/autostart.rs:1` | `AUTOSTART_REG_KEY` | `const &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run"`（私有） | 无 |
| `platform/autostart.rs:2` | `AUTOSTART_REG_VALUE` | `const &str = "CampusLogin"`（私有） | 无 |
| `platform/autostart.rs:4-15` | `get_auto_launch_enabled` | `pub fn() -> bool`：`HKEY_CURRENT_USER` 下读 `CampusLogin` 值，非空即启用 | 无 |
| `platform/autostart.rs:17-30` | `set_auto_start` | `pub fn(exe_path: &str) -> Result<(), String>`：写值 `"<exe_path>" --autostart`（25 行） | 无 |
| `platform/autostart.rs:32-45` | `remove_auto_start` | `pub fn() -> Result<(), String>`：删除值；`NotFound` 视为成功（42 行） | 无 |

实现依赖 `winreg`（HKCU，无需管理员）。命令行尾部追加 `--autostart`，该标记由 `commands/system.rs:122` 读取后作为 `isAutoStart` 返回前端（`commands/system.rs:148`）。

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
| `platform/dns_config.rs:26-28` | `DNS_SETTING_PROFILE_NAMESERVER` | `const u64 = 0x0200`（私有，带 `#[allow(dead_code)]` + 注释"编译器因条件编译误报"；实际在 `set_dns_stack:162` 使用） |
| `platform/dns_config.rs:29-31` | `DNS_SETTING_DOH_PROFILE` | `const u64 = 0x2000`（私有，同上；实际在 `set_dns_stack:171` 使用） |

私有函数与枚举：

| 行号 | 项 | 签名 | cfg |
|---|---|---|---|
| `platform/dns_config.rs:39-50` | `doh_bindings` | `fn<'a>(dns_servers: &[&str], doh_templates: &'a [(&str, &str)]) -> Vec<(usize, &'a str)>`：只为"实际存在于 NameServer 列表且配了模板"的服务器生成 `(ServerIndex, 模板)` | 无 |
| `platform/dns_config.rs:54-59` | `enum DnsTarget` | `{ Interface, Profile }`（`#[derive(Clone, Copy)]`，私有） | `#[cfg(target_os = "windows")]` |
| `platform/dns_config.rs:65-70` | `split_families` | `fn<'a>(dns_servers: &[&'a str]) -> (Vec<&'a str>, Vec<&'a str>)`：按是否含 `:` 分 IPv4/IPv6 | 无 |
| `platform/dns_config.rs:72-93` | `set_dns_inner` | `fn(target: DnsTarget, adapter_guid: &str, dns_servers: &[&str], doh_templates: &[(&str, &str)], err_label: &str) -> Result<(), String>`：两个栈分别调用 `set_dns_stack`，错误用 `；` 拼接 | `#[cfg(target_os = "windows")]` |
| `platform/dns_config.rs:99-212` | `set_dns_stack` | `fn(target, ipv6: bool, adapter_guid, dns_servers, doh_templates, err_label) -> Result<(), String>`：组 `DNS_INTERFACE_SETTINGS3` 并调 `SetInterfaceDnsSettings`（201-208） | `#[cfg(target_os = "windows")]` |
| `platform/dns_config.rs:248-292` | `clear_dns_stack` | `fn(adapter_guid: &str, ipv6: bool) -> Result<(), String>`：NameServer 置空串清栈 | `#[cfg(target_os = "windows")]` |

公开函数：

| 行号 | 函数 | 签名 | cfg |
|---|---|---|---|
| `platform/dns_config.rs:214-221` | `set_dns_via_api` | `pub fn(adapter_guid: &str, dns_servers: &[&str], doh_templates: &[(&str, &str)]) -> Result<(), String>`：接口级 DNS+DoH（`DnsTarget::Interface`） | `#[cfg(target_os = "windows")]` |
| `platform/dns_config.rs:225-232` | `set_profile_dns_via_api` | `pub fn(...)` 同上但 `DnsTarget::Profile`：仅对当前 WiFi 配置文件生效，切换 WiFi 自动切换 DNS | `#[cfg(target_os = "windows")]` |
| `platform/dns_config.rs:240-246` | `clear_adapter_dns_via_api` | `pub fn(adapter_guid: &str) -> Result<(), String>`：先清 IPv6 栈（失败仅警告，242-244）、再清 IPv4 栈（失败上报，245） | `#[cfg(target_os = "windows")]` |
| `platform/dns_config.rs:297-544` | `read_adapter_dns_from_registry` | `pub fn() -> Result<serde_json::Value, String>`：枚举网卡 → 读 DNS 来源与地址 → 解析 `netsh dns show encryption` → 输出 JSON | `#[cfg(target_os = "windows")]` |

`read_adapter_dns_from_registry` 内部的嵌套函数：

| 行号 | 项 | 说明 |
|---|---|---|
| `platform/dns_config.rs:311-331` | `should_filter_ip` | 过滤空串、非点分四段、末段 0/255、`127.*`、`169.254.*`、`198.18.*`/`198.19.*` |
| `platform/dns_config.rs:333-340` | `parse_dns_list` | 按 `,`/空格/`;` 切分并过滤 |
| `platform/dns_config.rs:342-428` | `check_doh_for_ips` | 调 `netsh dns show encryption`（345-348），逐行解析服务器 IP 与模板/自动升级字段（355-398，字段名中英双语匹配 386-389），与 `DOH_SERVERS` 内置表合并（403-425），返回 `HashMap<String, (available, enabled, template)>` |

注册表路径与产物（539-543）：`adapters[]`、`dohSupported: true`、`autoDohEnabled`。

### elevation.rs — 管理员判定与 UAC 提权（Windows）

| 行号 | 项 | 签名 | cfg |
|---|---|---|---|
| `platform/elevation.rs:1-24` | `is_admin` | `pub fn() -> bool`：`OpenProcessToken` + `GetTokenInformation(TokenElevation)`，`elevation != 0` 即管理员 | `#[cfg(target_os = "windows")]` |
| `platform/elevation.rs:26-50` | `run_elevated` | `pub fn(cmd: &str, args: &str) -> Result<(), String>`：`ShellExecuteW(None, "runas", cmd, args, None, SW_HIDE)`，返回值 ≤32 视为失败（45-47） | `#[cfg(target_os = "windows")]` |
| `platform/elevation.rs:52-123` | `shell_exec_elevated` | `pub fn(file: &str, params: &str, hide_window: bool) -> Result<(), String>`：COM 静默提权主路径 | `#[cfg(target_os = "windows")]` |
| `platform/elevation.rs:125-143` | `co_get_object_raw` | `unsafe fn(pszname, pbindoptions, riid, ppv) -> i32`（私有；`#[link(name = "ole32")]` 手写 `CoGetObject` 绑定） | `#[cfg(target_os = "windows")]` |
| `platform/elevation.rs:145-165` | `parse_guid` | `pub(crate) fn(s: &str) -> Result<windows::core::GUID, String>`：解析带/不带花括号的 GUID，供 `dns_config.rs:111`、`dns_config.rs:253` 使用 | `#[cfg(target_os = "windows")]` |
| `platform/elevation.rs:167-174` | `struct ICMLuaUtilVtbl` | `#[repr(C)]` 手工 vtable：`_query_interface`、`_add_ref`、`release`、`set_call_state`、`shell_exec`（私有） | 无 cfg |
| `platform/elevation.rs:176-193` | `mod tests` | `vtbl_shell_exec_slot_is_4`（182-186，用 `offset_of!` 断言 slot 4）、`vtbl_set_call_state_slot_is_3`（188-192） | `#[cfg(test)]` |

COM 提权链细节（`shell_exec_elevated`）：`CoInitializeEx(COINIT_APARTMENTTHREADED)`（65，按成功与否决定是否配对 `CoUninitialize`）→ moniker `Elevation:Administrator!new:{3E5FC7F9-9A51-4367-9063-A120244FBEC7}`（67）→ IID `{6EDD6D74-C007-4E75-B76A-E5740995E24C}`（70-72）→ `CoGetObject`（79-84）→ 取 vtable 调 `shell_exec(file, params, NULL, 0, n_show)`（99-106）→ `release`（108）→ 每个失败路径都配对 `CoUninitialize`（86-91、110-115、117-119）。

### gpu.rs — 硬件探测与 WebView2 参数

| 行号 | 项 | 签名 / 值 | cfg |
|---|---|---|---|
| `platform/gpu.rs:4-12` | `struct GpuInfo` | `#[derive(Serialize)]`，6 字段（见下节） | 无 |
| `platform/gpu.rs:14` | `GPU_CACHE` | `static OnceLock<GpuInfo>`（私有；进程内一次探测） | 无 |
| `platform/gpu.rs:16-69` | `determine_tier` | `fn(vendor: &str, model: &str, _vram_mb: u64) -> String`：返回 `discrete` / `high-igpu` / `mid-igpu` / `low-igpu` / `unknown` | 无 |
| `platform/gpu.rs:71-73` | `is_integrated_tier` | `fn(tier: &str) -> bool`：`matches!(tier, "low-igpu" \| "mid-igpu" \| "high-igpu")` | 无 |
| `platform/gpu.rs:75-102` | `read_gpu_preference` | `fn() -> u8`：读 `HKCU\Software\Microsoft\DirectX\UserGpuPreferences` 中当前 exe 路径的值，解析 `GpuPreference=<digit>`（93-99），无则 0 | 无 |
| `platform/gpu.rs:104-230` | `detect_gpu_info_inner` | `fn() -> GpuInfo`：`CreateDXGIFactory1`（127）+ `EnumAdapters1` 循环（140-201） | 无 |
| `platform/gpu.rs:232-234` | `detect_gpu_info` | `pub fn() -> &'static GpuInfo`：`GPU_CACHE.get_or_init(detect_gpu_info_inner)` | 无 |
| `platform/gpu.rs:236-248` | `build_browser_args` | `pub fn() -> String`：恒返回 `"--js-flags=--max-old-space-size=512"` | 无 |
| `platform/gpu.rs:250-277` | `detect_display_refresh_rate` | `pub fn() -> u32`：`EnumDisplaySettingsW(ENUM_CURRENT_SETTINGS)` 取 `dmDisplayFrequency`，失败返回 0 | 无 |

`detect_gpu_info_inner` 的选择逻辑：跳过 `DXGI_ADAPTER_FLAG_SOFTWARE`（151-154），按 `VendorId` 归类（164-169：`0x10DE` NVIDIA、`0x8086` Intel、`0x1002`/`0x1022` AMD），分别记录 `best_integrated` / `best_nvidia` / `best_amd_discrete` / `best_other`，再按 `gpu_preference` 选主（203-207：1=核显优先，2 与默认同为独显优先），最后 `determine_tier` 定级并算 `is_integrated`（217-218）。日志穿插在 111-116（偏好）、130（工厂失败）、212（无已知厂商）、220（探测结果）。

### helper_spawn.rs — 提权 helper 子进程与结果轮询

| 行号 | 项 | 签名 | cfg |
|---|---|---|---|
| `platform/helper_spawn.rs:13-20` | `unique_result_path` | `pub fn() -> PathBuf`：`%TEMP%/campus-login-helper-<pid>-<ms>.json` | 无 |
| `platform/helper_spawn.rs:23-35` | `read_helper_result` | `fn(content: &str, result_path: &Path) -> Result<serde_json::Value, String>`（私有）：解析 JSON、把 `logs` 数组逐条并入 `crate::log_info!("helper", ...)`（26-32）、删除结果文件（33） | 无 |
| `platform/helper_spawn.rs:41-80` | `spawn_elevated_helper` | `pub fn(op: &str, args: &[&str], result_path: &Path, timeout: Duration) -> Result<serde_json::Value, String>` | 无 |

`spawn_elevated_helper` 的步骤：取 `current_exe`（47）→ 拼参数 `--helper <op> "<arg>"… --result "<path>"`，每个位置参数用双引号包裹以适应含空格适配器名（52-58）→ 先试 `elevation::shell_exec_elevated(exe, params, true)`（61），失败则记警告并降级 `elevation::run_elevated`（62-63）→ 每 100ms 轮询结果文件直到 `deadline`（67-73）→ 超时后再补查一次（74-78）→ 仍无则 `Err("提权操作超时，未收到helper结果")`（79）。

调用方：`commands/network_cmd.rs:303`（DNS，30s 超时）、`network/dhcp.rs:288-290`（MAC 重置）。helper 侧实现见 `tauri-app/src-tauri/src/helper/mod.rs`（`HelperOp::Dns` / `HelperOp::Mac`，`helper/mod.rs:28-34`），入口在 `main.rs:13` / `lib.rs:16` 注册。

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
| `platform/identity.rs:123-149` | `try_verification_for_window` | `async fn(hwnd: isize, message: &str) -> Option<Result<(), String>>`（私有）：`None` = interop 不可用需回退；`Some` = 最终结论（含用户取消） | 无 |
| `platform/identity.rs:155-172` | `spawn_consent_focus_nudger` | `fn()`（私有）：起线程，12 次 × 250ms 轮询窗口类名 `"Credential Dialog Xaml Host"` 并 `SetForegroundWindow`（159-170） | 无 |
| `platform/identity.rs:176-204` | `await_winrt_operation` | `async fn<T: RuntimeType + Send>(op: IAsyncOperation<T>) -> Result<T, String>`（私有）：`SetCompleted` 回调 + `tokio::sync::oneshot`，非阻塞等待 | 无 |

`verify_hello` 调用点：`UserConsentVerifier::CheckAvailabilityAsync()`（84）、`RequestVerificationAsync(&HSTRING)`（107）、interop 的 `RequestVerificationForWindowAsync(HWND, &HSTRING)`（139）。设备未配置 Hello 时直接返回引导文案且**不回退凭据对话框**（89-94，模块注释 3-5 行记录 2026-09-05 用户要求移除 CredUI 回退）。

### toast.rs — WinRT Toast（Windows 桌面）

| 行号 | 项 | 签名 / 值 | cfg |
|---|---|---|---|
| `platform/toast.rs:16` | `APP_ID` | `const &str = "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe"`（私有；沿用 notify-rust 默认 AUMID，未打包应用才能进通知中心） | 无（模块级 `cfg(all(desktop, target_os = "windows"))`） |
| `platform/toast.rs:19-26` | `resolve_resource_image` | `fn(app_handle: &AppHandle, relative: &str) -> Option<String>`（私有）：`resource_dir().join(relative)` 存在则转 `file:///` URL（反斜杠替换为正斜杠），否则 `None` | 无 |
| `platform/toast.rs:30-43` | `show_system_toast` | `pub fn(app_handle: &AppHandle, title: &str, body: &str, mascot: &str) -> Result<(), String>`：`resources/mascot-toast/<mascot>.png` 作 `appLogoOverride`，`ToastNotificationManager::CreateToastNotifierWithId(APP_ID)` 后 `Show` | 无 |
| `platform/toast.rs:47-78` | `show_update_toast` | `pub fn(app_handle: &AppHandle, version: &str) -> Result<(), String>`：固定文案"发现新版本 / 新版本 v{version} 可用，前往关于界面进行更新"，注册 `Activated` 回调（62-72）唤起主窗口并发 `update-notification-click` 事件 | 无 |

## 结构体与字段

### `GpuInfo`（`platform/gpu.rs:4-12`）

| 字段 | 类型 | 含义 |
|---|---|---|
| `vendor` | `String` | 厂商名：`"NVIDIA"` / `"Intel"` / `"AMD"` / `"Unknown(0xXXXX)"` / `"unknown"`（兜底，118-125） |
| `model` | `String` | DXGI `Description` 去尾部 `\0`（157-159） |
| `vram_mb` | `u64` | `DedicatedVideoMemory / 1024 / 1024`（160-161） |
| `is_integrated` | `bool` | 由 `tier` 推导（71-73、218） |
| `tier` | `String` | `discrete` / `high-igpu` / `mid-igpu` / `low-igpu` / `unknown` |
| `gpu_preference` | `u8` | 读自注册表：0=系统默认、1=节能(核显)、2=高性能(独显)（111-116 的日志映射） |

序列化后即 `get_gpu_info` / `get_init_data.gpuInfo` 的返回体。

### `ICMLuaUtilVtbl`（`platform/elevation.rs:167-174`）

| 字段 | 类型 | 说明 |
|---|---|---|
| `_query_interface` | `usize` | vtable slot 0（未调用） |
| `_add_ref` | `usize` | slot 1（未调用） |
| `release` | `unsafe extern "system" fn(*mut c_void) -> u32` | slot 2 |
| `set_call_state` | `unsafe extern "system" fn(*mut c_void, u32) -> HRESULT` | slot 3（测试锁定，188-192） |
| `shell_exec` | `unsafe extern "system" fn(*mut c_void, *const u16, *const u16, *const u16, u32, u32) -> HRESULT` | slot 4（测试锁定，182-186；历史版本曾因多声明 6 个占位方法把 `shell_exec` 挤到 slot 9 造成越界 UB） |

### `DnsTarget`（`platform/dns_config.rs:54-59`）

| 变体 | 说明 |
|---|---|
| `Interface` | 写 `NameServer` + `DNS_SETTING_NAMESERVER` flags，DoH 属性进 `ServerProperties`（`DNS_SETTING_DOH`） |
| `Profile` | 写 `ProfileNameServer` + `DNS_SETTING_PROFILE_NAMESERVER` flags，DoH 属性进 `ProfileServerProperties`（`DNS_SETTING_DOH_PROFILE`） |

### `DNS_INTERFACE_SETTINGS3` 填充字段（`platform/dns_config.rs:181-198`）

| 字段 | 置值 | 说明 |
|---|---|---|
| `Version` | `DNS_INTERFACE_SETTINGS_VERSION3` | 必须匹配结构体版本 |
| `Flags` | `ns_flag`（按 target）\| 可能的 `DNS_SETTING_IPV6`（165-167）\| 可能的 DoH flag（168-173） | 一次调用只作用一个协议栈 |
| `Domain` | `PWSTR::null()` | 不改 |
| `NameServer` | Interface 时为 `ns_wide` 指针，否则 null（155-158） | 逗号分隔的地址串（113-114） |
| `SearchList` | `PWSTR::null()` | 不改 |
| `RegistrationEnabled` / `RegisterAdapterName` / `EnableLLMNR` / `QueryAdapterName` | 全 0 | 不改 |
| `ProfileNameServer` | Profile 时为 `ns_wide` 指针，否则 null | 按 WiFi profile 生效 |
| `DisableUnconstrainedQueries` | 0 | 不改 |
| `SupplementalSearchList` | `PWSTR::null()` | 不改 |
| `cServerProperties` / `ServerProperties` | 按 target 二选一赋 `doh_props` 指针与长度（176-179） | 未使用的槽恒为 NULL |
| `cProfileServerProperties` / `ProfileServerProperties` | 同上互补 | — |

`clear_dns_stack` 里的同结构体只需 `Flags = DNS_SETTING_NAMESERVER [| DNS_SETTING_IPV6]` 与 `NameServer = 空串`（256-278）。

### DNS/DoH 相关的进程内枚举与常量

`platform/dns_config.rs:132-135` 构造的 `DNS_DOH_SERVER_SETTINGS.Flags` 固定为 `DNS_DOH_SERVER_SETTINGS_ENABLE_AUTO | DNS_DOH_SERVER_SETTINGS_ENABLE | DNS_DOH_SERVER_SETTINGS_FALLBACK_TO_UDP`（自动升级 + 启用 + 回退 UDP）。

## Data Flow

### 控制台输出解码（多模块共用入口）

```text
netsh / ipconfig / wmic 子进程 stdout/stderr（OEM 代码页 936 或 65001）
  → platform/console_output.rs:32 decode_console_bytes
       ├─ std::str::from_utf8 成功 → 原样返回（38-40）
       ├─ 失败 → GetOEMCP()（41）→ MultiByteToWideChar（18/23）→ from_utf16_lossy
       └─ 再失败 → from_utf8_lossy 兜底（47）
```

调用方：`network/subnet.rs`（stdout）、`network/adapter_cache.rs`（stderr）、`network/dhcp.rs`、`network/dns_setup.rs`（stdout/stderr）、`platform/dns_config.rs:350`（`netsh dns show encryption`）。HTTP 响应体走 `decode_charset_bytes`（`auth/protocol.rs`，按 `Content-Type` 声明优先 936）。

### DNS + DoH 设置链（含 UAC）

```text
前端 invoke('setup_dns_doh', {family})
  → commands/network_cmd.rs:261（参数规整 + 目标白名单）
  → platform/elevation.rs:2 is_admin()
      ├─ 管理员：network::dns_setup::setup_dns_doh_admin
      │     → platform/dns_config.rs:214 set_dns_via_api / 225 set_profile_dns_via_api
      │         → 72 set_dns_inner → 65 split_families → 99 set_dns_stack → 201 SetInterfaceDnsSettings
      └─ 非管理员：platform/helper_spawn.rs:303 unique_result_path()
            → 41 spawn_elevated_helper("dns", [adapters…, "--family", family], path, 30s)
                 ├─ 61 elevation::shell_exec_elevated(exe, params, true)
                 │     → CoInitializeEx → CoGetObject(Elevation:Administrator!new:{3E5FC7F9-…})（79）
                 │     → vtable.shell_exec（99）→ release（108）→ CoUninitialize
                 └─ 失败 → 63 elevation::run_elevated(exe, params) → ShellExecuteW "runas"（36）
            → 子进程 --helper dns（helper/mod.rs:106 run_dns）写结果文件
            → 74-78 轮询/补查结果文件 → 23 read_helper_result 解析并删除结果文件
```

DNS 状态读取链：`invoke('check_dns_doh_status')` → `commands/network_cmd.rs:246` → `platform/dns_config.rs:298 read_adapter_dns_from_registry` → 遍历 `HKLM\SYSTEM\CurrentControlSet\Control\Network\{4D36E972-…}` 取网卡名（435-441）→ 用网卡 GUID 打开 `HKLM\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\<guid>`（307-309、451）读 `NameServer` / `ProfileNameServer` / `DhcpNameServer`（452-464，优先级 manual > profile > dhcp）→ `netsh dns show encryption` 交叉核对 DoH（342-399）→ JSON。

### 注册表调用点汇总

| 注册表路径 | 读/写 | 代码位置 |
|---|---|---|
| `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run` 值 `CampusLogin` | 读写删 | `platform/autostart.rs:8-14`、`21-27`、`36-43` |
| `HKCU\Software\Microsoft\DirectX\UserGpuPreferences` 值 `<exe 路径>` | 读 | `platform/gpu.rs:79-99` |
| `HKLM\SYSTEM\CurrentControlSet\Control\Network\{4D36E972-E325-11CE-BFC1-08002BE10318}\<guid>\Connection`（`Name`、`PnpInstanceID`） | 读 | `platform/dns_config.rs:302-305`、`435-449` |
| `HKLM\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\<guid>`（`NameServer`、`DhcpNameServer`、`ProfileNameServer`） | 读 | `platform/dns_config.rs:307-309`、`451-464` |

### Win32 / WinRT API 调用点汇总

| API | 位置 | 用途 |
|---|---|---|
| `MultiByteToWideChar` / `GetOEMCP` | `platform/console_output.rs:18`、`23`、`41` | 代码页解码 |
| `OpenProcessToken` / `GetTokenInformation(TokenElevation)` / `CloseHandle` | `platform/elevation.rs:9`、`14-20`、`21` | 管理员判定 |
| `ShellExecuteW`（verb `runas`，`SW_HIDE`） | `platform/elevation.rs:36-43` | 降级 UAC 提权 |
| `CoInitializeEx` / `CoUninitialize` / `CoGetObject`（ole32） | `platform/elevation.rs:65`、`118`、`136-142` | COM 静默提权 |
| `CreateDXGIFactory1` / `EnumAdapters1` / `GetDesc1` | `platform/gpu.rs:127`、`141`、`146` | GPU 枚举 |
| `EnumDisplaySettingsW(ENUM_CURRENT_SETTINGS)` | `platform/gpu.rs:262-266` | 刷新率 |
| `SetInterfaceDnsSettings`（IpHelper） | `platform/dns_config.rs:201`、`281` | 写/清 DNS + DoH |
| `UserConsentVerifier::CheckAvailabilityAsync` / `RequestVerificationAsync` | `platform/identity.rs:84`、`107` | Windows Hello |
| `IUserConsentVerifierInterop::RequestVerificationForWindowAsync` | `platform/identity.rs:139` | Win11 主路径：Consent 绑定主窗口 HWND |
| `FindWindowW` + `SetForegroundWindow` | `platform/identity.rs:164`、`166` | 兜底：Consent 对话框提前台 |
| `XmlDocument::LoadXml` / `ToastNotification` / `ToastNotificationManager` | `platform/toast.rs:38`、`39`、`40`、`75` | 自组 Toast XML 并显示 |

### GPU / WebView2 参数流

```text
main.rs:43  build_browser_args() → "--js-flags=--max-old-space-size=512"（platform/gpu.rs:247）
main.rs:50-53  追加 Crashpad 参数 --enable-crash-reporter --crash-dumps-dir="<Roaming>\com.campus.login\crashdumps"
main.rs:55  std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", ...)（必须在 Tokio runtime 创建前）
app/startup.rs:171  把 env 实际值写日志
```

GPU 信息流：`app/startup.rs:207-218` 后台 `gpu-warmup` 线程先调 `detect_gpu_info()`（211）与 `detect_display_refresh_rate()`（212）；此后 `commands/system.rs:123-124`（`get_init_data`）与 `commands/system.rs:172`（`get_gpu_info`）直接读 `OnceLock` 缓存。

### 身份验证门数据流

```text
前端 invoke('verify_windows_identity', {consentMessage})
  → commands/self_service.rs:260
  → 主窗口 show/set_focus + hwnd()（266-270）→ platform/identity.rs:60 verify_identity(msg, Some(hwnd))
      ├─ 83 CheckAvailabilityAsync → 非 Available 直接 Err（89-94）
      ├─ 97-102 interop 主路径（Win11）→ try_verification_for_window（123）
      └─ 104-116 兜底路径：spawn_consent_focus_nudger（155）+ RequestVerificationAsync
  → 成功：commands/self_service.rs:273 note_identity_verified() → platform/identity.rs:35 写时间戳
后续敏感命令（reveal / bind / offline）→ platform/identity.rs:39 identity_verified_recently()（TTL 600s）
```

### Toast 数据流

```text
infra/notification.rs:46（cfg(all(desktop, target_os="windows"))）
  → platform/toast.rs:30 show_system_toast(...)
      失败 → 降级 tauri_plugin_notification 纯文本（infra/notification.rs:52-56）
非 Windows 桌面/安卓 → 直接走插件通知（infra/notification.rs:57-62）
发现新版本：update/updater.rs:296 → platform/toast.rs:47 show_update_toast
  → 点击 → 显示主窗口 + EventBus::emit_update_notification_click（platform/toast.rs:63-70）
```

## Connections

- [[desktop-commands]]：命令层是本层的直接上游（`set_auto_launch`→`autostart`、`get_gpu_info`→`gpu`、`setup_dns_doh`/`check_dns_doh_status`→`dns_config`+`elevation`+`helper_spawn`、`verify_windows_identity`→`identity`）。
- [[desktop-network-dns]]：DNS/DoH 的业务编排（`network::dns_setup`）与本层的 `dns_config` 的分工边界。
- [[desktop-network-core]]：`network/adapter_cache.rs`、`network/dhcp.rs` 对 `elevation` 与 `helper_spawn` 的另外两处调用（netsh 提权、MAC 重置）。
- [[desktop-helper-update]]：helper 子进程协议（`helper/mod.rs` 的 `HelperOp`、结果文件格式）与更新包下载/校验的上层流程。
- [[desktop-auth]]：`auth/protocol.rs` 通过 `decode_charset_bytes` 解码 GBK 响应体做成败关键词判定。
- [[desktop-infra]]：`crate::log_*` 宏、`EventBus`、`AppHandle` 资源的生命周期。
- [[desktop-app-lifecycle]]：主窗口创建、`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 注入时机、`gpu-warmup` 预热线程与心跳线程。
- [[desktop-config]]：`log_retention_days` 等配置项如何驱动本层行为（自启、通知、DNS 目标解析）。
- [[android-plugins]]：安卓端不编译本层模块，等价的 Keystore / 前台服务 / 网络绑定能力由 `android/plugins/*` 提供。

## Known Issues

1. **模块门控只到 `desktop`，不锁 Windows**：`platform/mod.rs:4-16` 对 `autostart` / `dns_config` / `elevation` / `gpu` / `helper_spawn` / `identity` 只加 `#[cfg(desktop)]`（唯一带 `target_os = "windows"` 的是 `toast`，`platform/mod.rs:17`），而 `autostart.rs`、`gpu.rs`、`helper_spawn.rs`、`identity.rs` **文件内部没有任何 `#[cfg]`**（见逐文件清单），它们直接使用 `winreg` / `windows` crate；项目实际只支持 Windows 桌面 + 安卓，但该约束是事实约定而非编译期保证。
2. **`toast.rs` 借用 PowerShell 的 AUMID**：`platform/toast.rs:16` 使用 `{1AC14E77-...}\WindowsPowerShell\v1.0\powershell.exe`，是未打包应用进通知中心的取巧做法；应用改签名/打包方式或系统策略变化时通知可能静默不显示。
3. **`identity.rs` 无 CredUI 回退**：`platform/identity.rs:89-94` 设备未配置 Windows Hello 时直接报错退出（模块注释 1-5 行说明 2026-09-05 用户要求移除输密码回退），未配置 Hello 的用户无法执行 reveal / bind 等敏感操作。
4. **兜底路径的焦点轮询只有 3 秒**：`platform/identity.rs:159-170` 固定 12 次 × 250ms 后线程自行结束，若 Consent 对话框出现更晚（慢机/UAC 排队），就没有任何提前台兜底。
5. **身份验证时间戳是进程内全局**：`platform/identity.rs:28` 的 `AtomicU64` 随进程重启归零（0 = 从未验证），且是单用户单进程语义，多实例/多用户场景不共享。
6. **`clear_adapter_dns_via_api` 的 IPv6 清理失败被吞**：`platform/dns_config.rs:242-244` 清除 v6 栈失败仅记 `log_warn`（注释说明 "IPV6 flag + 空串" 的 API 接受性未经 Win11 实测确证），可能导致旧静态 v6 DNS 残留而接口级设置覆盖 profile DNS。
7. **`build_browser_args` 只剩单个参数**：`platform/gpu.rs:236-248` 原先 11 个 Chromium 参数被精简为 `--js-flags=--max-old-space-size=512`，注释（239-246）逐项记录了删除理由；若日后确有性能增益需按 `ponytail:` 注释（244 行）逐项实测后加回。
8. **`GPU_CACHE` 进程内不可刷新**：`platform/gpu.rs:14` 的 `OnceLock` 意味着运行期切换显卡偏好/热插拔不会反映到 `get_gpu_info` 结果，需重启应用。
9. **`detect_gpu_info_inner` 的偏好分支冗余**：`platform/gpu.rs:203-207` 中 `gpu_preference == 2` 与默认分支（`_`）的选择顺序完全相同，实际只有 1（核显优先）与"其他"两种行为。
10. **注册表读取依赖 `netsh` 输出格式**：`platform/dns_config.rs:355-398` 逐行解析中英双语字段名（386-389 的 `autoupgrade`/`自动升级`），`netsh` 输出格式或系统语言变化会直接影响 `dohEnabled` 判定。
11. **`helper_spawn` 无进程存活检查**：`platform/helper_spawn.rs:67-78` 只轮询结果文件，不提权子进程提前退出（例如用户在 UAC 弹窗点"否"）时会一直等到超时（调用方 30s）才返回错误。
12. **COM 提权依赖未公开接口**：`platform/elevation.rs:67-72` 使用 `Elevation:Administrator!new:{3E5FC7F9-…}` 与手写 IID `{6EDD6D74-…}`，属未文档化 COM 提权路径；Windows 更新后失效时的表现是降级为弹 UAC（`platform/helper_spawn.rs:61-64`），不会静默失败。
13. **`should_filter_ip` 会丢弃合法的 `198.18/198.19` 内网 DNS**：`platform/dns_config.rs:325-329` 把 `198.18.*`、`198.19.*` 一并过滤（原意是剔除基准测试网段），特殊校园网部署可能显示不出真实 DNS。
14. **`read_adapter_dns_from_registry` 的 `dohSupported` 恒为 `true`**：`platform/dns_config.rs:541` 硬编码，未做系统能力探测。
