---
title: IPC 命令面两端同名对齐
type: decision
source_files:
  - tauri-app/frontend/src/hooks/tauriApi.ts
  - android/frontend/src/hooks/tauriApi.ts
  - tauri-app/src-tauri/src/app/startup.rs
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/battery_cmds.rs
tags: [决策, ipc, 双端, 命令面]
---

## 背景

前端需要同时消费桌面端（Windows/macOS）与安卓端的后端能力。若两端 Rust 后端的 IPC 命令各自命名，前端 `TauriApi` 就必须按平台分叉维护两套接口，增加维护成本与出错概率。

## 决策

**核心原则：前端 `TauriApi` 一套接口两端消费；安卓命令与桌面同名（例外见后）。**

1. **统一前端接口**：在 `tauri-app/frontend/src/hooks/tauriApi.ts`（接口定义见行 27-130）和 `android/frontend/src/hooks/tauriApi.ts`（接口定义见行 48-151）中定义相同的 `TauriApi` 方法名（如 `getConfig`、`doLogin`、`saveConfig` 等）。
2. **后端同名对齐**：安卓端在 `android/src-tauri/src/lib.rs` 注册的命令名（行 66-122 宏内条目）必须与桌面端 `tauri-app/src-tauri/src/app/startup.rs`（行 66-136 宏内条目）保持一致，例如 `do_login` 桌面端行 72 / 安卓端行 72，`save_config` 桌面端行 69 / 安卓端行 78，`get_config` 桌面端行 67 / 安卓端行 77。
3. **差异仅在实现**：同名方法允许参数按平台裁剪或类型不同。
   - 例：`doLogin` 桌面端带可选参数 `adapterName?: string`（`tauri-app/frontend/src/hooks/tauriApi.ts:41` 接口，实现行 174 传 `{ adapterName }`），安卓端无参（接口 `android/frontend/src/hooks/tauriApi.ts:62`，实现行 213 传 `{}`）。
   - 例：`verifyWindowsIdentity` 桌面直接 invoke `verify_windows_identity`（`tauri-app/frontend/src/hooks/tauriApi.ts:178`），安卓包一层生物识别逻辑后 invoke `verify_biometric_identity`（见“例外”章节）。
4. **新增命令流程**：必须在同一提交内完成“三处同改”：
   - 桌面后端注册（`tauri-app/src-tauri/src/app/startup.rs` 的 `generate_handler!` 宏，行 66-136）
   - 安卓后端注册（`android/src-tauri/src/lib.rs` 的 `generate_handler!` 宏，行 66-122）
   - 两端前端接口声明与实现（`tauri-app/frontend/src/hooks/tauriApi.ts` 与 `android/frontend/src/hooks/tauriApi.ts`）

## 理由

- **降低前端复杂度**：业务代码无需感知底层 IPC 命令名的平台差异，统一通过 `invoke` 调用。
- **强制同步**：通过“三处同改”的硬约束，防止出现后端命令缺失或前端接口不一致的情况。
- **明确边界**：将“平台专属能力”与“共同能力”区分开，避免强行对齐导致的空壳实现。

## 备选方案

- **前端分叉**：为每个平台维护独立的 API 层。*弃用原因*：大量重复代码，且容易因不同步导致功能缺失。
- **后端自动适配**：由后端根据平台自动映射命令。*弃用原因*：增加后端路由复杂度，且无法利用 TypeScript 类型系统在前端编译期发现错误。

## 影响与约束

### 硬约束：新增命令必须三处同改

当前命令规模（基于实读源码）：
- **安卓端**：55 条命令（`android/src-tauri/src/lib.rs:66` 宏，条目行 67-121）。
- **桌面端**：69 条命令（`tauri-app/src-tauri/src/app/startup.rs:66` 宏，条目行 67-135）。
- **共同命令**：37 条（同名且两端均注册，如 `get_config`、`do_login`、`save_config`、`check_portal_status`、`set_notification_enabled` 等）。
- **差值**：全部为平台专属命令面，仅单端注册，不进“三处同改”。桌面后端独有 32 条，安卓后端独有 18 条。

### 平台专属命令分类

#### 1. 桌面后端独有 (32 条)
这些命令在安卓后端未注册；前端处理分两类：安卓 `TauriApi` 中通过 `desktopOnly` helper 恒 reject（定义于 `android/frontend/src/hooks/tauriApi.ts:14-15`），或安卓接口未声明该方法。

| 类别 | 命令名 | 桌面后端注册 | 安卓前端处理 |
| :--- | :--- | :--- | :--- |
| **窗口/轻量化** | `show_window`, `minimize_window`, `close_window`, `notify_window_ready` | `startup.rs:68,107,108,116` | `desktopOnly` (`android/frontend/src/hooks/tauriApi.ts:317,265,266,334`) |
| **适配器族** | `get_adapters`, `get_adapter_details`, `enable_adapter` | `startup.rs:74,75,78` | `desktopOnly` (`android/frontend/src/hooks/tauriApi.ts:206,208,207`) |
| **DHCP** | `dhcp_renew_all`, `dhcp_release_renew`, `dhcp_release_renew_adapter` | `startup.rs:79,80,81` | `desktopOnly` (`android/frontend/src/hooks/tauriApi.ts:284,285,286`) |
| **DNS/DOH** | `check_dns_doh_status`, `setup_dns_doh`, `reset_dns` | `startup.rs:85,86,87` | `desktopOnly` (`android/frontend/src/hooks/tauriApi.ts:330,331,332`) |
| **导出/诊断** | `export_config`, `import_config`, `export_diagnostics` | `startup.rs:70,71,115` | `desktopOnly` (`android/frontend/src/hooks/tauriApi.ts:203,204,205`) |
| **出站切换** | `outbound_switch_now`, `outbound_restore_now`, `get_current_outbound_name` | `startup.rs:88,89,90` | 安卓接口未声明（桌面前端 `tauri-app/frontend/src/hooks/tauriApi.ts:211-215` 实现） |
| **托盘/退出** | `cancel_auto_exit` | `startup.rs:106` | `desktopOnly` (`android/frontend/src/hooks/tauriApi.ts:311`) |
| **心跳/渲染** | `render_heartbeat` | `startup.rs:113` | `desktopOnly` (`android/frontend/src/hooks/tauriApi.ts:333`) |
| **自启/核心** | `set_auto_launch`, `set_core_autostart`, `get_core_autostart` | `startup.rs:102,103,104` | 安卓使用不同名 `set_boot_autostart`/`get_boot_autostart`（见例外） |
| **更新取消** | `cancel_download_update` | `startup.rs:119` | 安卓接口未声明（桌面前端 `tauri-app/frontend/src/hooks/tauriApi.ts:255,255`） |
| **VPN 系列** | `vpn_save_credential`, `vpn_credential_status`, `vpn_clear_credential`, `set_self_service_via_vpn`, `vpn_test_connection` | `startup.rs:130-134` | 安卓前端声明并 invoke 同名命令（`android/frontend/src/hooks/tauriApi.ts:260-264`），但安卓后端 `lib.rs` 未注册 → **已知缺陷** |
| **验证门** | `verify_windows_identity` | `startup.rs:124` | 安卓前端方法同名但 invoke `verify_biometric_identity`（例外） |
| **外部打开** | `open_external` | `startup.rs:109` | 安卓前端 `openExternal` 不走 invoke，改用 `openerOpenUrl`（例外） |

#### 2. 安卓后端独有 (18 条)
这些命令仅注册于安卓后端，桌面端无对应物。

| 类别 | 命令名 | 文件/位置 | 前端声明（安卓） |
| :--- | :--- | :--- | :--- |
| **WiFi/网络** | `bind_to_wifi`, `request_wifi_ssid_permission` | `lib.rs:68,76` | `bindToWifi` (`android/frontend/src/hooks/tauriApi.ts:58,209`), `requestWifiSsidPermission` (`:61,212`) |
| **防劣网** | `get_avoid_bad_wifi_status`, `restore_written_settings` | `lib.rs:70,71` | `getAvoidBadWifiStatus` (`:148,337`), `restoreWrittenSettings` (`:150,338`) |
| **探针/其他协议** | `ping_test`, `accept_wifi_network` | `lib.rs:67,69` | 前端接口未声明 |
| **自启** | `get_boot_autostart`, `set_boot_autostart` | `lib.rs:105,106` | `getAutoLaunch`/`setAutoLaunch` (`:111-112,303-304`) |
| **通知/电池** | `get_notification_enabled`, `get_battery_optimization_info`, `request_ignore_battery_optimizations`, `open_vendor_battery_settings`, `open_notification_settings` | `lib.rs:107,109-112` | `getNotificationEnabled` (`:113,305`), `getBatteryOptimizationInfo` (`:115,307`), `requestIgnoreBatteryOptimizations` (`:116,308`), `openVendorBatterySettings` (`:117,309`), `openNotificationSettings` (`:118,310`)；实现见 `battery_cmds.rs:20,46,65,87` |
| **配置/调试 getter** | `get_log_retention_days`, `get_debug_mode` | `lib.rs:97,99` | `getLogRetentionDays` (`:145,335`), `getDebugMode` (`:128,320`) |
| **人脸模型** | `face_models_state`, `face_models_download` | `lib.rs:120-121` | 前端接口未声明 |
| **生物识别验证门** | `verify_biometric_identity` | `lib.rs:79` | 安卓前端 `verifyWindowsIdentity` 内部 invoke (`:223`) |

*注：桌面端 `TauriApi` 接口（行 27-130）不包含上述安卓专属方法。*

### 例外：前端同名、后端不同名或实现分叉

尽管遵循“后端同名”原则，以下场景存在已知特例：

1. **`verifyWindowsIdentity`**
   - **桌面**：直接 invoke `verify_windows_identity`（`tauri-app/frontend/src/hooks/tauriApi.ts:178`），后端注册于 `startup.rs:124`。
   - **安卓**：前端方法名仍为 `verifyWindowsIdentity`（接口 `android/frontend/src/hooks/tauriApi.ts:66`），但内部逻辑包裹系统 `BiometricPrompt` 或 2D 人脸回退（实现行 217-254），最终 invoke `verify_biometric_identity`（行 223），后端注册于 `lib.rs:79`。

2. **`getAutoLaunch` / `setAutoLaunch`**
   - **桌面**：invoke `set_auto_launch` 与 `get_core_autostart`/`set_core_autostart`（`tauri-app/frontend/src/hooks/tauriApi.ts:236-238`）；桌面无 `getAutoLaunch` 方法，仅有 `setAutoLaunch` 及 core autostart 对。
   - **安卓**：前端 `getAutoLaunch`/`setAutoLaunch`（接口 `:111-112`）invoke `get_boot_autostart` / `set_boot_autostart`（`android/frontend/src/hooks/tauriApi.ts:303-304`），并在 set 侧补装返回值以匹配桌面 `AutoLaunchResult` 类型（行 304 `.then(() => ({ success: true }))`）。

3. **`openExternal`**
   - **桌面**：invoke `open_external`（`tauri-app/frontend/src/hooks/tauriApi.ts:223`），失败时回退到 shell 插件（行 227-233）。
   - **安卓**：后端无 `open_external` 命令，前端直接调用 `@tauri-apps/plugin-opener` 的 `openerOpenUrl`（行 296），不走 `invoke`（实现行 291-302）。

4. **前端接口非互为超集**
   - 桌面 `TauriApi` 中的 `outboundSwitchNow`, `outboundRestoreNow`, `getCurrentOutboundName`, `onScheduledActionSkipped`, `onMacResetCycle`, `onUpdateNotificationClick`, `cancelDownloadUpdate`（`tauri-app/frontend/src/hooks/tauriApi.ts:86-90,106-108,118,245-246,260`）**未进入**安卓接口。
   - 安卓专属的 `bindToWifi`, `requestWifiSsidPermission`, `getAvoidBadWifiStatus`, `restoreWrittenSettings`, `getBatteryOptimizationInfo`, `onUpdateUnverified` 等（接口 `android/frontend/src/hooks/tauriApi.ts:58,61,138,148-150,115,329`）**不存在于**桌面接口。
   - 这意味着“同名对齐”仅约束**共同方法面**。对于 `desktopOnly` 恒 reject 的方法，编译期无法察觉调用方是否误用，实际强度低于命名对齐的表面承诺。

## Connections

[[protocol-core-single-source]]、[[ipc-command-surface]]