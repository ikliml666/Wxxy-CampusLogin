---
title: IPC 命令面与事件总线
type: concept
source_files:
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/commands/mod.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
  - tauri-app/src-tauri/src/commands/system.rs
  - tauri-app/src-tauri/src/commands/network_cmd.rs
  - tauri-app/src-tauri/src/commands/account.rs
  - tauri-app/src-tauri/src/commands/background.rs
  - tauri-app/src-tauri/src/app/tray.rs
  - tauri-app/src-tauri/src/monitor/scheduled.rs
  - tauri-app/src-tauri/src/infra/events.rs
  - tauri-app/src-tauri/src/infra/command_context.rs
  - tauri-app/src-tauri/src/infra/logger.rs
  - tauri-app/src-tauri/src/monitor/adapter_watch.rs
  - tauri-app/src-tauri/src/update/updater.rs
  - tauri-app/src-tauri/src/platform/toast.rs
  - tauri-app/src-tauri/src/commands/updater.rs
  - tauri-app/src-tauri/capabilities/default.json
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/system_cmds.rs
  - android/src-tauri/src/update_cmds.rs
  - android/src-tauri/src/battery_cmds.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/protocol_cmds.rs
  - android/src-tauri/src/campus_detect.rs
  - android/src-tauri/src/self_service_cmds.rs
  - android/src-tauri/src/account_cmds.rs
  - android/src-tauri/src/quality_cmds.rs
  - tauri-app/frontend/src/hooks/tauriApi.ts
  - tauri-app/frontend/src/hooks/useEventListeners.ts
  - tauri-app/frontend/src/App.tsx
  - tauri-app/frontend/src/auth/AboutDialog.tsx
  - android/frontend/src/hooks/tauriApi.ts
  - android/frontend/src/hooks/useEventListeners.ts
tags: [概念, ipc, tauri, 命令, 事件总线, 双端]
---

# IPC 命令面与事件总线

## Overview

双端（Windows 桌面 / Android）通过 Tauri IPC 暴露命令面，通过事件总线向前端推送后端状态变化。

- **桌面 65 条命令**：`app/startup.rs:62-128` 的 `generate_handler!` 注册块，其中 `commands/` 目录 8 个文件贡献 63 条，`infra/logger.rs` 贡献 2 条（`set_debug_mode`/`get_debug_mode`，logger.rs:345-355）。
- **安卓 53 条命令**：`android/src-tauri/src/lib.rs:57-111` 的 `generate_handler!`，分布在 15 个模块（lib.rs:11-25）。
- **两端同名命令 38 条**；另有 **3 对跨端映射**（功能对应但名字不同）：`get_auto_launch↔get_boot_autostart`、`set_auto_launch↔set_boot_autostart`、`verify_windows_identity↔verify_biometric_identity`。因此**桌面独有 24 条、安卓独有 12 条**（38+3+24=65；38+3+12=53）。
- **桌面事件总线**：`infra/events.rs` 的 `EventBus` 提供 16 个 `emit_*` 方法，是桌面后端向前端发事件的唯一正规出口；安卓没有 `events.rs`，直接 `use tauri::Emitter` 裸调 `app.emit`。
- **安卓真实事件是桌面的子集**：只有 7 个事件真实发射（login-log、background-check-result、auto-login-result、network-quality-result、config-changed、update-download-progress、update-available），不含 `update-notification-click`、适配器族 4 个与退出倒计时族 4 个；其中 `config-changed` 已覆盖。

## 机制说明

### 命令注册点（先读这一节）

**桌面**：`#[tauri::command]` 函数写在 `commands/` 各文件中，由 `app/startup.rs:62-128` 的 `generate_handler!` 一次性注册。模块清单见 `commands/mod.rs:1-8`（login/background/network_cmd/system/config_cmd/account/updater/self_service 共 8 个）。注册分布：

| 模块 | 条数 | startup.rs 注册行 | 命令 |
| --- | --- | --- | --- |
| config_cmd | 5 | 63-67 | get_config、show_window、save_config、export_config、import_config |
| login | 2 | 68-69 | do_login、do_logout |
| network_cmd | 18 | 70-87 | get_adapters、get_adapter_details、check_campus_status、check_portal_status、get_disabled_adapters、enable_adapter、dhcp_renew_all、dhcp_release_renew、dhcp_release_renew_adapter、check_network_quality、start_latency_test、stop_latency_test、check_dns_doh_status、setup_dns_doh、reset_dns、outbound_switch_now、outbound_restore_now、get_current_outbound_name |
| account | 6 | 88-93 | list_accounts、switch_account、rename_account、save_current_as_account、delete_account、get_active_account |
| background | 4 | 94-97 | start_background_check、stop_background_check、trigger_background_check、get_background_status |
| system | 17 | 98-114 | get_auto_launch、set_auto_launch、get_notification_enabled、set_notification_enabled、cancel_auto_exit、minimize_window、close_window、open_external、get_logs、clear_logs、get_init_data、render_heartbeat、get_gpu_info、set_log_retention_days、get_log_retention_days、export_diagnostics、notify_window_ready |
| updater | 4 | 115-118 | check_update、download_update、install_update、get_mirror_urls |
| self_service | 7 | 119-125 | bind_operator、query_bind_status、verify_windows_identity、reveal_operator_credential、query_self_dashboard、query_self_online_log、self_offline_session |
| logger（infra） | 2 | 126-127 | set_debug_mode、get_debug_mode |

`startup.rs:135-149` 在 `RunEvent::ExitRequested` 中用轻量化退出守卫（`take_lightweight_exit_guard`）决定是否 `api.prevent_exit()`，实现关窗常驻托盘、托盘退出/系统关机放行。

**安卓**：命令按功能分 15 个模块（lib.rs:11-25：campus_detect、android_state、config_state、cpu_affinity、identity_gate、login_history、protocol_cmds、self_service_cmds、monitor_loop、account_cmds、system_cmds、quality_cmds、quality_history、update_cmds、battery_cmds），mobile 插件在 lib.rs:33-38 注册（campus_network_bind/campus_keystore/campus_monitor_service/biometric/notification）。注册分布：

| 模块 | 条数 | lib.rs 注册行 | 命令 |
| --- | --- | --- | --- |
| protocol_cmds | 8 | 58-65 | ping_test、bind_to_wifi、accept_wifi_network、get_avoid_bad_wifi_status、restore_written_settings、do_login、do_logout、check_portal_status |
| campus_detect | 3 | 66-68 | detect_campus、check_campus_status、request_wifi_ssid_permission |
| config_state | 2 | 69-70 | get_config、save_config |
| self_service_cmds | 7 | 71-77 | verify_biometric_identity、bind_operator、query_bind_status、query_self_dashboard、query_self_online_log、self_offline_session、reveal_operator_credential |
| account_cmds | 6 | 78-83 | list_accounts、switch_account、rename_account、save_current_as_account、delete_account、get_active_account |
| system_cmds | 8 | 84-91 | get_init_data、get_soc_info、get_logs、clear_logs、get_log_retention_days、set_log_retention_days、get_debug_mode、set_debug_mode |
| monitor_loop | 8 | 92-99 | start_background_check、stop_background_check、trigger_background_check、get_background_status、get_boot_autostart、set_boot_autostart、get_notification_enabled、set_notification_enabled |
| battery_cmds | 4 | 100-103 | get_battery_optimization_info、request_ignore_battery_optimizations、open_vendor_battery_settings、open_notification_settings |
| quality_cmds | 3 | 104-106 | check_network_quality、start_latency_test、stop_latency_test |
| update_cmds | 4 | 107-110 | check_update、download_update、get_mirror_urls、install_update |

安卓同样在 `lib.rs:114-121` 的 `RunEvent::ExitRequested` 中 `api.prevent_exit()`，保活常驻（划掉任务只关 UI，进程随前台服务常驻）。

**注册即全部暴露面**：没进 `generate_handler!` 的函数，前端 invoke 会得到 `command not found`，`#[tauri::command]` 属性本身不产生任何暴露。

### 命令命名与参数契约

- **命令名 = Rust 函数名的 snake_case**。前端 JS 侧用 camelCase 包装（如 `tauriApi.ts` 里 `doLogin`、`saveConfig`）。
- **参数自动映射**：前端 camelCase 实参由 Tauri 自动映射回 Rust snake_case 形参。例如安卓 `saveConfig(config, clearPassword, clearSelfPassword)` → invoke `save_config` 带 `{ config, clearPassword, clearSelfPassword }`（android/frontend/src/hooks/tauriApi.ts:199），对应 `config_state.rs:420-453` 的 `clear_password`/`clear_self_password` 形参。
- 桌面前端入口集中在 `tauri-app/frontend/src/hooks/tauriApi.ts`（`invoke('do_login', ...)` 在 161 行）；安卓在 `android/frontend/src/hooks/tauriApi.ts`。
- **统一返回结构 `CommandResult { success, message?, data? }`**：桌面定义在 `infra/state/mod.rs:272`；安卓同形定义在 `android/src-tauri/src/self_service_cmds.rs:7-23`（struct 7-14，`message`/`data` 带 `skip_serializing_if = "Option::is_none"`，`ok/err` 构造器在 impl 16-23：`ok_msg` 17、`err` 20）。账号命令另有 `AccountResult`，携带可选 `activeAccount`/`displayName`，其 IPC 出站形状由 `commands/account.rs` 测试锁定（720-744：`switch_success_result_carries_active_account`、`account_item_json_shape`）。
- **能力门控不覆盖自有命令**：`tauri-app/src-tauri/capabilities/default.json`（14 行，7 项权限）只声明 `core:default`、`core:window:allow-start-dragging`、`core:window:allow-toggle-maximize` 与 notification 系列（`notification:default` + allow-is-permission-granted/allow-request-permission/allow-notify），全部 65 条自有命令不经能力系统，注册即可调用。
- **敏感字段掩码统一走 `masked_for_display`**：桌面在 `config/model.rs:315`（`pub fn masked_for_display(&self) -> Config`），安卓在 `config_state.rs:373`（`pub fn masked_for_display(s: &Settings) -> serde_json::Value`，仅替换 `password`/`self_password` 为 `"***"`，见 372-382）。任何出站配置都必须经过它，不得另写序列化（安卓 386-387 行注释为唯一出口语义）。

### 两端命令名不完全一致的三处

| # | 桌面命令 | 安卓命令 | 前端处理 |
| --- | --- | --- | --- |
| 1 | `verify_windows_identity`（startup.rs:121） | `verify_biometric_identity`（lib.rs:71） | 安卓 `tauriApi.ts:215-246`：先走 2D 人脸回退（`shouldUseFaceFallback`，219-242）或系统 BiometricPrompt（244，`allowDeviceCredential`），再 `invoke('verify_biometric_identity')`——同一封装内有两处 invoke（234、245）。桌面前端直接 invoke `verify_windows_identity` |
| 2 | `get_auto_launch` / `set_auto_launch`（startup.rs:98-99） | `get_boot_autostart` / `set_boot_autostart`（lib.rs:96-97） | 安卓 `tauriApi.ts:289-290`：前端成员仍叫 `getAutoLaunch`/`setAutoLaunch`，内部换名 invoke；set 侧把安卓返回包装成桌面形状 `AutoLaunchResult` |
| 3 | `get_config` 掩码 | `get_config` 同名 | 双端同名但都各自掩码：桌面 `commands/config_cmd.rs:215-218`（`masked_for_display`），安卓 `config_state.rs:405-418`（417 行 `Ok(masked_for_display(&s))`） |

### 桌面独有与安卓独有命令

**桌面独有 24 条**（安卓前端用 `desktopOnly` 桩显式拒绝，见下文）：

- 窗口/托盘类：`show_window`（config_cmd.rs:209-213）、`minimize_window`（system.rs:7-10）、`close_window`（system.rs:12-21）、`notify_window_ready`（system.rs:284-288）、`render_heartbeat`（system.rs:157-172）、`cancel_auto_exit`（system.rs:91-98）
- 配置导出/导入/诊断：`export_config`（config_cmd.rs:135-150）、`import_config`（config_cmd.rs:156-207）、`export_diagnostics`（system.rs:195-280）
- 网卡 4 条：`get_adapters`（network_cmd.rs:19-25）、`get_adapter_details`（43-46）、`get_disabled_adapters`（27-30）、`enable_adapter`（32-41）
- DHCP 3 条：`dhcp_renew_all`（151-168）、`dhcp_release_renew`（170-191）、`dhcp_release_renew_adapter`（193-209）
- DNS/DoH 3 条：`check_dns_doh_status`（284-297）、`setup_dns_doh`（299-385）、`reset_dns`（387-478）
- 出站切换 3 条：`outbound_switch_now`（49-58）、`outbound_restore_now`（61-70）、`get_current_outbound_name`（74-85），分别转调 `monitor::scheduled::manual_outbound_switch`（scheduled.rs:1271-1289）/`manual_outbound_restore`（1293-1306）与 `best_route::current_outbound_v4`（platform/best_route.rs:56，network_cmd.rs:77 调用）
- 其他：`open_external`（system.rs:23-36）、`get_gpu_info`（system.rs:174-178）

**安卓独有 12 条**：

- 协议/网络：`ping_test`（protocol_cmds.rs:171-174）、`bind_to_wifi`（177-191）、`accept_wifi_network`（195-209）、`get_avoid_bad_wifi_status`（214-230）、`restore_written_settings`（234-250）
- 校园网检测：`detect_campus`（campus_detect.rs:216-244）、`request_wifi_ssid_permission`（277-283）
- 设备信息：`get_soc_info`（system_cmds.rs:71-76）
- 电池 4 条：`get_battery_optimization_info`（battery_cmds.rs:19-41）、`request_ignore_battery_optimizations`（45-61）、`open_vendor_battery_settings`（64-81）、`open_notification_settings`（86-102）

其中 `ping_test`、`detect_campus`、`accept_wifi_network` **没有前端封装**（见 Known Issues ④）。

### 前端封装覆盖关系

**桌面 `tauri-app/frontend/src/hooks/tauriApi.ts`（286 行）**：

- `interface TauriApi` 27-116，共 **81 成员 = 65 条命令封装 + 16 个事件监听器**。
- `createEventListener` 118-147：处理 `cancelled` 标志与 `unlisten` 的竞态（listen 未完成时取消，要等 promise 完成后再清理）。
- 实现 `tauriApi` 149-248：65 个 invoke。事件监听行号：`onBackgroundCheckResult` 172、`onAutoLoginResult` 173、`onAdaptersChanged` 174、`onAdapterDetailsChanged` 175、`onDisabledAdaptersChanged` 176、`onAdapterDisabledWarning` 177、`onLoginLog` 178、`onNetworkQualityResult` 196、`onAutoExitCountdown` 222、`onAutoExitCancelled` 223、`onCampusExitCountdown` 224、`onCampusExitCancelled` 225、`onConfigChanged` 226、`onDownloadProgress` 237、`onUpdateAvailable` 238、`onUpdateNotificationClick` 239。
- `openExternal` 199-216：先 invoke `open_external`（204，后端做 http/https、长度、凭据校验），失败兜底 opener 插件 `shellOpen`（209）。
- `withRetry` 262-278（指数退避+抖动），`tauriApiWithRetry` 280-286 只覆写 `saveConfig`（282-284 注释解释 `checkPortalStatus`/`checkNetworkQuality` 因高频与后端兜底不重试）。

**安卓 `android/frontend/src/hooks/tauriApi.ts`（364 行）**：

- `interface TauriApi` 56-148，共 **86 成员 = 50 条真实命令封装（51 个 invoke 位点，`verify_biometric_identity` 在 234/245 两处）+ `openExternal`（opener 插件真实现）+ 20 个 `desktopOnly` 桩 + 15 个事件监听器**。
- `desktopOnly` helper 14-15：统一拒绝 `桌面专属功能,安卓端不可用: <name>`；`noopListener` 18：桌面专属事件的空监听。
- 20 个 desktopOnly 桩（200-206、251-252、270-272、297、303、316-321）：exportConfig、importConfig、exportDiagnostics、getAdapters、getDisabledAdapters、enableAdapter、getAdapterDetails、minimizeWindow、closeWindow、dhcpRenewAll、dhcpReleaseRenew、dhcpReleaseRenewAdapter、cancelAutoExit、showWindow、checkDnsDohStatus、setupDnsDoh、resetDns、renderHeartbeat、notifyWindowReady、getGpuInfo。
- **`openExternal` 277-288 不是桩**：前端用 `@tauri-apps/plugin-opener` 的 `openUrl`（282）真实现，带 http/https 校验（278）、2048 长度上限（279）、URL 解析校验（280）；安卓后端没有 `open_external` 命令——与桌面（invoke 命令 + shell 兜底）机制不同、API 形状一致。
- 安卓独有封装 9 个：`bindToWifi`（67/207）、`requestWifiSsidPermission`（70/210）、`getSocInfo`（128/309）、`getBatteryOptimizationInfo`（112/293）、`requestIgnoreBatteryOptimizations`（113/294）、`openVendorBatterySettings`（114/295）、`openNotificationSettings`（115/296）、`getAvoidBadWifiStatus`（145/324）、`restoreWrittenSettings`（147/325）。
- 事件监听 15 个 = 7 真 + 8 noop：真实 `createEventListener` 在 253（background-check-result）、254（auto-login-result）、259（login-log）、274（network-quality-result）、302（config-changed）、314（update-download-progress）、315（update-available）；noop 在 255-258（适配器族 4 个）与 298-301（退出倒计时族 4 个）。
- `isRetryableError` 328-338、`withRetry` 340-356、`tauriApiWithRetry` 358-364 同样只覆写 `saveConfig`（360-362 注释）。

**消费端（useEventListeners / 组件）**：

- 桌面 `useEventListeners.ts`（386 行）：`onCloseRequested` 50-75（有待存配置时 `preventDefault` → `await` 冲刷 promise 与 2s 定时器 `Promise.race`（61-64）→ 再 close（69）；否则直接冲刷 71）。事件订阅 14 个：background-check-result 77（1s 节流 80-82）、auto-login-result 201（**204 行读 `skipped`**）、adapters-changed 217（500ms 节流）、adapter-details-changed 244（JSON 相等跳过 249）、disabled-adapters-changed 254、adapter-disabled-warning 260、login-log 269、auto-exit-countdown 277、auto-exit-cancelled 298、campus-exit-countdown 305、campus-exit-cancelled 326、network-quality-result 333、update-available 341-352（仅记日志+写 store，不弹窗）、config-changed 354（`mergeConfigFromBackend` 跳过本地脏字段、同步 activeAccount 365-368、刷新账号列表 369-372）；cleanup 377-384。`onDownloadProgress` 与 `onUpdateNotificationClick` 不在本文件。
- 桌面 `App.tsx:230-234`：232 行订阅 `onUpdateNotificationClick` → 打开关于界面（系统通知点击回传）。桌面 `auth/AboutDialog.tsx`：`handleDownload`（149-174）下载期间动态注册 `onDownloadProgress`（155-156 先清旧监听，159-161 注册）。
- 安卓 `useEventListeners.ts`（251 行）：`onCloseRequested` 41-66（同样 await in-flight 保存 + 2s race 50-58）；另有 `visibilitychange`/`pagehide` 兜底 68-80（进程被杀不触发 onCloseRequested 时切后台冲刷）。事件订阅 10 个：background-check-result 82、auto-login-result 117（不读 skipped）、login-log 134、auto-exit-countdown 142、auto-exit-cancelled 163、campus-exit-countdown 170、campus-exit-cancelled 191、network-quality-result 198、update-available 206（**215 行 `setUpdatePromptOpen(true)` 弹更新窗**，与桌面只记日志不同）、config-changed 221（mergeConfigFromBackend 227、activeAccount 同步 231-235、listAccounts 刷新 236-239）。适配器族 4 个订阅已清理（130-132 注释说明），退出倒计时 4 个 handler 仍注册但对着 noopListener（见 KI①）；cleanup 244-249（含移除 visibilitychange/pagehide）。
- 安卓 `onDownloadProgress` 的消费在 `android/frontend/src/auth/AboutDialog.tsx:185` 与 `AboutDialogMobile.tsx:86`（均为下载期间动态注册）。

### 事件总线

**桌面**：`infra/events.rs`（135 行）定义 `EventBus<'a>`（8-15），私有 `emit` 17-19 调 `app_handle.emit(event, payload)`。16 个 `emit_*` 方法与行号：

| 事件 | events.rs | payload | 后端发射点 | 桌面前端 | 安卓 |
| --- | --- | --- | --- | --- | --- |
| login-log | 22-27 | `{message, type}` | auth/session.rs:28/38/45/62/125、auth/failure_tracker.rs:76/157/256、commands/login.rs:115/121、monitor/auto_auth.rs:163/202/205、monitor/latency.rs:46/49、monitor/portal_check.rs:68/86、network/adapter.rs:177/210/216、app/tray.rs:160/183/195、monitor/scheduled.rs:1451/1518/1547/1553/1564/1568 | useEventListeners.ts:269 | monitor_loop.rs:152-156 封装 + 34 个调用点（226、239、516、522、525、529、539、544、649、684、797、813、828、831、868、881、912、931、945、947、960、965、986、996、1006、1151、1162、1172、1177、1179、1250、1314、1398、1415） |
| background-check-result | 30-32 | BackgroundStatus 快照 | monitor/background_emit.rs:136（包装函数定义 109；由 monitor/background_check.rs:115/327 调用） | useEventListeners.ts:77 | monitor_loop.rs:1457（payload 1432-1452） |
| auto-login-result | 35-41 | `{success, message, skipped}` | monitor/auto_auth.rs:81/192/251/318/404/442、app/tray.rs:134/143 | useEventListeners.ts:201（204 读 skipped） | monitor_loop.rs:496-502 封装**仅 2 字段**，调用点 535/541/546/1404/1409/1417 |
| network-quality-result | 44-46 | NetworkQuality | network/quality.rs:511/591/645、monitor/quality_scheduler.rs:41 | useEventListeners.ts:333 | 复用共享 crate（android/src-tauri/src/quality_cmds.rs:98 注释：run_quality_once 内部经 EventBus emit） |
| update-available | 49-55 | `{hasUpdate, latestVersion, releaseNotes}` | update/updater.rs:354（do_update_check；hasUpdate 时 WinRT show_update_toast 365，失败降级 emit_notification 367/370） | useEventListeners.ts:341（仅记日志+store） | update_cmds.rs:323（start_update_check_loop） |
| update-notification-click | 58-60 | `{}` | platform/toast.rs:119（WinRT Activated 处理器 113-123；show/unminimize/set_focus 114-117） | App.tsx:232 | **无**（monitor_loop.rs:161-178 notify_system 用 large_icon 173，无点击回传） |
| adapter-details-changed | 63-65 | AdapterDetail[] | monitor/adapter_watch.rs:92（details 非空才发） | useEventListeners.ts:244 | noop（tauriApi.ts:256） |
| disabled-adapters-changed | 68-70 | DisabledAdapter[] | adapter_watch.rs:99 | 254 | noop（257） |
| adapter-disabled-warning | 73-78 | `{name, message}` | adapter_watch.rs:160（新禁用∧配置卡∧非夜切亲手禁用才发） | 260 | noop（258） |
| campus-exit-countdown | 81-86 | `{minimizeDelay, exitDelay}` | infra/lifecycle.rs:59 | 305 | noop（300） |
| campus-exit-cancelled | 89-91 | `{}` | infra/lifecycle.rs:178 | 326 | noop（301） |
| auto-exit-countdown | 94-99 | `{delay, shortcut}` | infra/lifecycle.rs:204（快捷键 "Ctrl+Shift+C"） | 277 | noop（298） |
| auto-exit-cancelled | 102-104 | `{}` | infra/lifecycle.rs:285 | 298 | noop（299） |
| config-changed | 107-109 | `{config: 掩码后配置}` | 统一出口 commands/config_cmd.rs:18（`save_config_to_disk_encrypted` 9-23 内经 `AppHandleExt::notify_config_changed`）；account.rs 的 switch/delete 落盘、system.rs 的 set_auto_launch/set_notification_enabled 也经同一函数 | useEventListeners.ts:354 | config_state.rs:388-391 `emit_config_changed`（emit 390）；save_config 451 与 monitor_loop.rs:976（persist_settings）调用 |
| adapters-changed | 112-114 | Adapter[] | adapter_watch.rs:88 | 217 | noop（255） |
| update-download-progress | 117-119 | 下载进度 | commands/updater.rs:147/178（200ms 节流 + 完成 100%，经 `AppHandleExt::notify_update_download_progress`） | AboutDialog.tsx:159（下载期动态注册） | update_cmds.rs:444-447 直发；消费在 android AboutDialog.tsx:185 / AboutDialogMobile.tsx:86 |

`AppHandleExt`（`infra/command_context.rs:38-51`）是两个事件在业务代码里的语义出口：`notify_config_changed`（39 声明，44-46 → `EventBus::emit_config_changed`）、`notify_update_download_progress`（40 声明，48-50 → `emit_update_download_progress`）。

**安卓没有 events.rs**：`monitor_loop.rs` 与 `update_cmds.rs` 直接 `use tauri::Emitter` 后 `app.emit`。少量复用性封装：`emit_login_log` 包装（monitor_loop.rs:152-156，34 个调用点）、`emit_auto_login_result` 包装（496-502）、`emit_config_changed`（config_state.rs:388-391）；其余（update-available 323、update-download-progress 444-447、background-check-result 1457）为裸 emit。

**update-available 的消费差异**：桌面 useEventListeners.ts:341-352 只记日志并写 store；安卓 useEventListeners.ts:206-219 会在 `hasUpdate` 时 `setUpdatePromptOpen(true)`（215）主动弹更新窗。

### 新增一条命令要改哪几处

**桌面（5 步）**：

1. `commands/<module>.rs` 写 `#[tauri::command]` 函数；
2. 新模块时在 `commands/mod.rs:1-8` 加 `pub mod`；
3. `app/startup.rs:62-128` 的 `generate_handler!` 登记命令名（漏掉即 command not found）；
4. 桌面前端 `tauriApi.ts`：接口 `TauriApi`（27-116）加成员声明 + 实现（149-248）加 invoke；
5. 若命令改配置：走 `save_config_to_disk_encrypted`（config_cmd.rs:9-23）落盘——它在第 17-18 行先 `masked_for_display()` 再发 `config-changed`，前端才能同步 store。

**安卓（5 步）**：

1. 对应模块写 `#[tauri::command]` 函数；
2. `android/src-tauri/src/lib.rs:11-25` 挂模块（新模块时）；
3. `lib.rs:57-111` `generate_handler!` 登记；
4. 安卓 `tauriApi.ts`：接口（56-148）+ 实现（197-326）；桌面专属能力就写成 `desktopOnly`（14-15）桩，不要留 undefined；
5. 若命令改配置：走 `config_state::save_to` + 更新 `AndroidState.config` 缓存 + **显式调用** `emit_config_changed`（config_state.rs:388-391）——安卓没有统一落盘出口，参考 `save_config`（420-453）与 `monitor_loop.rs:955-978`（persist_settings：save_to 964、缓存写回 968-971、自动建号 972、广播 976）。

**新增事件（3 步）**：`events.rs` 加 `emit_*` 方法 → 桌面前端 `createEventListener`（或 useEventListeners.ts 订阅）→ 安卓侧单独裸 emit（payload 必须对齐桌面契约）或接 `noopListener`（android tauriApi.ts:18）。

## 关键约束

1. **注册即全部暴露面**：`generate_handler!` 之外一律 `command not found`；桌面 65 条、安卓 53 条，两端注册表是命令面的唯一权威清单。
2. **两端命令名逐字相同**，仅 3 对跨端映射例外（见上表）。前端成员名可以跨端一致（如 `verifyWindowsIdentity`），但 invoke 的命令字符串必须与后端函数名一致。
3. **事件名是字符串常量、无编译期校验**：`events.rs` 方法内的事件名、前端 `createEventListener('...')` 的字符串、安卓裸 emit 的字面量，三处必须手工同步；安卓同一事件可能有多个字面量点，新代码应优先走包装函数（如 `emit_login_log`）收敛。
4. **桌面事件必须经 EventBus**：业务代码不得直接 `app.emit`；配置与下载进度必须经 `AppHandleExt` 的 `notify_config_changed`/`notify_update_download_progress`（command_context.rs:38-51）。安卓无此约束，但 payload 形状必须与桌面前端契约一致。
5. **配置变更必须先掩码再广播**：桌面唯一落盘出口 `save_config_to_disk_encrypted`（config_cmd.rs:9-23，emit 在 17-18）；`get_config`（215-218）与 `get_init_data`（system.rs:112-155，掩码在 117）同样掩码。安卓唯一出口 `masked_for_display`（config_state.rs:373，386-387 注释强调不得另写序列化）；`get_config`（405-418）、`get_init_data`（system_cmds.rs:5-28，掩码在 13）、账号命令出站（account_cmds.rs:161/303/338）全部复用它。
6. **安卓 `get_init_data` 必须补齐桌面专属字段**（system_cmds.rs:16-22 注释）：`autoLaunch`、`gpuInfo: null`、`refreshRate: 60`、`adapters/adapterDetails/disabledAdapters` 空数组——否则前端 `useInitialDataLoad` 读到 undefined 崩溃。
7. **前端必须处理「监听建立前丢事件」**：安卓把当前状态展平进 `status_value`（monitor_loop.rs:118-138），随 `get_init_data` 的 `backgroundStatus`（system_cmds.rs:26）首拉；桌面 `get_init_data` 同样携带快照。事件只做增量，初值靠拉。

## Data Flow

**配置写路径（双端对称）**：

```text
React 组件 → zustand store（debounce 聚合）→ tauriApiWithRetry.saveConfig
  → invoke('save_config') → save_config 命令
    桌面: config_cmd.rs save_config(220-276) → save_config_to_disk_encrypted(9-23)
          → 加密落盘 + notify_config_changed → EventBus::emit_config_changed({config: 掩码})
    安卓: config_state.rs save_config(420-453) → resolve_password_field(394-403) → save_to(441)
          → AndroidState.config 缓存(442-445) → auto_create_account_for_current(448)
          → emit_config_changed(451, emit 390)
  → 前端 onConfigChanged（桌面 tauriApi.ts:226 / 安卓 302）
  → useEventListeners.ts:354 / 221 → mergeConfigFromBackend（跳过本地脏字段）→ store → UI
```

**状态读路径**：UI 挂载 → `get_init_data` 一次拉全（config 掩码 + accounts + 快照状态）→ 渲染初值；此后事件做增量。

**监控推送路径**：桌面 monitor 体系（scheduled.rs / background_check.rs / auto_auth.rs / adapter_watch.rs）→ `EventBus::emit_*` → 前端 `createEventListener` → `useEventListeners` 各 handler → zustand → UI。安卓由 `monitor_loop.rs` 的 `run_check_once`（1245-1471）驱动，`background-check-result` 在 1457 发射（payload 1432-1452，含 onlineOperator/timestamp/checkCount/isRunning/onCampusNetwork/campusMessage/currentSsid/sourceIp/autoLogin）；login-log 经包装函数散布 34 个调用点。

## Connections

- [[desktop-commands]] —— 桌面 65 条命令在各 commands/*.rs 的领域实现与校验规则。
- [[android-backend]] —— 安卓 15 模块的命令实现、前台服务与 WiFi watcher。
- [[desktop-infra]] —— AppState/CommandContext/EventBus/AppHandleExt/logger 等基础设施。
- [[desktop-frontend-hooks]] —— 桌面 tauriApi.ts / useEventListeners.ts / zustand store 全家桶。
- [[android-frontend]] —— 安卓 tauriApi.ts / useEventListeners.ts 与移动端 store 差异。
- [[config-and-persistence]] —— 配置加密落盘、掩码出站、导入导出与双端同步。
- [[background-check-and-auto-login]] —— 后台检测循环与自动登录，是 login-log/auto-login-result/background-check-result 的主要生产者。
- [[security-model]] —— DPAPI/keystore 加密、凭据掩码、export/import 的安全边界。

## Known Issues

1. **安卓 4 个退出倒计时监听是空壳**：handler 逻辑完整（android useEventListeners.ts:142-196），但 `tauriApi.ts:298-301` 的对应成员全是 `noopListener`，永不触发；且 `cancelAutoExit`（tauriApi.ts:297）也是 `desktopOnly` 桩，handler 内的取消动作同样不可达。适配器族 4 个同类空壳已清理（useEventListeners.ts:130-132 注释，不再注册），但 `tauriApi.ts:255-258` 的监听桩仍在。
2. **命令计数易漂移**：本文以当前代码为准——桌面 65 / 安卓 53 / 同名 38 / 映射 3 对 / 桌面独有 24 / 安卓独有 12。旧版文档出现过 49/61、安卓独有 14（把 `get/set_boot_autostart` 计入独有而未折入映射对）等过时口径。核对计数必须在主仓库工作区进行（滞后分支缺新命令文件）。
3. **auto-login-result 载荷双端不对齐**：桌面 3 字段（events.rs:35-41，`skipped` 有语义——auto_auth.rs:318/404 传 true 表示"跳过"）；安卓包装（monitor_loop.rs:496-502）只发 `success`/`message` 2 字段。桌面前端 useEventListeners.ts:204 读 `skipped`，安卓前端 117-128 不读。安卓调用点现为 6 处（535/541/546/1404/1409/1417）。
4. **安卓 3 条命令无前端出口**：`ping_test`（protocol_cmds.rs:171-174）、`detect_campus`（campus_detect.rs:216-244）、`accept_wifi_network`（protocol_cmds.rs:195-209）在安卓 `TauriApi` 接口中没有成员（useAuthStore.ts 里的 `detectCampusNetwork` 是本地辅助函数名，与命令无关）。
5. **update-notification-click 安卓无对应**：桌面 WinRT toast 的 Activated 回调（platform/toast.rs:113-123）在 119 行 emit，App.tsx:232 据此打开关于页；安卓 `notify_system`（monitor_loop.rs:161-178）用 `large_icon`（173）无点击回传，事件也不在安卓 TauriApi 中（安卓 15 个监听器 vs 桌面 16 个）。
6. **安卓关窗冲刷是双路径兜底、桌面单路径**：安卓 `onCloseRequested`（useEventListeners.ts:41-66，await in-flight 保存 + 2s race 50-58）+ `visibilitychange`/`pagehide`（68-80，进程被杀不触发 onCloseRequested 时兜底冲刷）；桌面只有 `onCloseRequested`（useEventListeners.ts:50-75，同款 2s race 61-64）。
