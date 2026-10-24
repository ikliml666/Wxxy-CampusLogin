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

**核心原则：前端 `TauriApi` 一套接口两端消费；安卓命令与桌面同名。**

1.  **统一前端接口**：在 `tauri-app/frontend/src/hooks/tauriApi.ts`（接口定义见行 27-120）和 `android/frontend/src/hooks/tauriApi.ts`（接口定义见行 48-141）中定义相同的 `TauriApi` 接口方法名。
2.  **后端同名对齐**：安卓端在 `android/src-tauri/src/lib.rs` 注册的命令名（行 66-122 宏内条目）必须与桌面端 `tauri-app/src-tauri/src/app/startup.rs` 保持一致（例如 `do_login` 行 72/72，`save_config` 行 69/78，`get_config` 行 67/77）。
3.  **差异仅在实现**：同名方法允许参数按平台裁剪或类型不同。
    *   例：`doLogin` 桌面端带可选参数 `adapterName?: string`（`tauri-app/frontend/src/hooks/tauriApi.ts:41` 接口，实现 `:164` 传 `{ adapterName }`），安卓端无参（接口 `android/frontend/src/hooks/tauriApi.ts:62`，实现 `:203` 传 `{}`）。
    *   例：`verifyWindowsIdentity` 桌面直接 invoke `verify_windows_identity`（`tauri-app/frontend/src/hooks/tauriApi.ts:168`），安卓包一层生物识别逻辑后 invoke `verify_biometric_identity`（见“例外”章节）。
4.  **新增命令流程**：必须在同一提交内完成“三处同改”：
    *   桌面后端注册（`tauri-app/src-tauri/src/app/startup.rs` 的 `generate_handler!` 宏，行 66-131）
    *   安卓后端注册（`android/src-tauri/src/lib.rs` 的 `generate_handler!` 宏，行 66-122）
    *   两端前端接口声明与实现（`tauri-app/frontend/src/hooks/tauriApi.ts` 与 `android/frontend/src/hooks/tauriApi.ts`）

## 理由

*   **降低前端复杂度**：业务代码无需感知底层 IPC 命令名的平台差异，统一通过 `invoke` 调用。
*   **强制同步**：通过“三处同改”的硬约束，防止出现后端命令缺失或前端接口不一致的情况。
*   **明确边界**：将“平台专属能力”与“共同能力”区分开，避免强行对齐导致的空壳实现。

## 备选方案

*   **前端分叉**：为每个平台维护独立的 API 层。*弃用原因*：大量重复代码，且容易因不同步导致功能缺失。
*   **后端自动适配**：由后端根据平台自动映射命令。*弃用原因*：增加后端路由复杂度，且无法利用 TypeScript 类型系统在前端编译期发现错误。

## 影响与约束

### 硬约束：新增命令必须三处同改

当前命令规模（基于实读源码）：
*   **安卓端**：55 条命令（`android/src-tauri/src/lib.rs:66` 宏，条目 `:67-121`）。
*   **桌面端**：64 条命令（`tauri-app/src-tauri/src/app/startup.rs:66` 宏，条目 `:67-130`）。
*   **共同命令**：37 条（同名且两端均注册，如 `get_config`、`do_login`、`save_config`、`check_portal_status`、`set_notification_enabled` 等）。
*   **差值**：全部为平台专属命令面，仅单端注册，不进“三处同改”。桌面独有 27 条，安卓独有 18 条。

### 平台专属命令分类

#### 1. 桌面独有 (27 条)
这些命令在安卓端通过 `desktopOnly` helper 恒 reject（定义于 `android/frontend/src/hooks/tauriApi.ts:14-15`），或在安卓 `TauriApi` 接口中不存在（如 outbound 系列）。

| 类别 | 命令名 | 说明 |
| :--- | :--- | :--- |
| **适配器族** | `get_adapters`, `get_adapter_details`, `enable_adapter` | 桌面网络适配器管理（前端 `android/frontend/src/hooks/tauriApi.ts:196-198` 走 `desktopOnly`） |
| **DHCP** | `dhcp_renew_all`, `dhcp_release_renew`, `dhcp_release_renew_adapter` | DHCP 续租/释放（前端 `:269-271` 走 `desktopOnly`） |
| **DNS/DOH** | `check_dns_doh_status`, `setup_dns_doh`, `reset_dns` | DNS-over-HTTPS 设置（前端 `:315-317` 走 `desktopOnly`） |
| **窗口/轻量化** | `show_window`, `minimize_window`, `close_window`, `notify_window_ready` | 窗口管理与轻量化退出机制（前端 `:250-251`, `:302`, `:319` 走 `desktopOnly`） |
| **托盘/退出** | `cancel_auto_exit` | 取消自动退出倒计时（前端 `:296` 走 `desktopOnly`） |
| **出站切换** | `outbound_switch_now`, `outbound_restore_now`, `get_current_outbound_name` | 智能出站切换控制（桌面前端 `tauri-app/frontend/src/hooks/tauriApi.ts:196-198` 实现；安卓接口未声明这些方法） |
| **导出/诊断** | `export_config`, `import_config`, `export_diagnostics` | 依赖 DPAPI 密文与本地文件系统，安卓无对应实现（前端 `:193-195` 走 `desktopOnly`） |
| **其他** | `open_external`, `render_heartbeat`, `verify_windows_identity`, `set_auto_launch`, `get_core_autostart`, `set_core_autostart`, `cancel_download_update` | 桌面特定系统交互（前端 `openExternal` 调 `open_external` 行 208；`verifyWindowsIdentity` 调 `verify_windows_identity` 行 168；`setAutoLaunch` 调 `set_auto_launch` 行 221；`getCoreAutostart`/`setCoreAutostart` 行 222-223；`cancelDownloadUpdate` 行 240） |

*   **安卓端处理**：
    *   接口声明存在于 `android/frontend/src/hooks/tauriApi.ts`（如 `exportConfig` 接口行 52，实现 `:193`）。
    *   实现统一走 `desktopOnly` helper（定义于 `:14-15`），例如 `exportConfig` 实现于 `:193`。
    *   部分桌面专属事件（如 `onAdaptersChanged`、`onAdapterDetailsChanged`、`onAdapterDisabledWarning`、`onAutoExitCountdown` 等）在安卓端使用 `noopListener`（`:18`，适配器事件 `:254-256`，退出事件 `:297-300`）消化，返回 no-op 清理函数。

#### 2. 安卓独有 (18 条)
这些命令仅注册于安卓后端，桌面端无对应物。

| 类别 | 命令名 | 文件/位置 | 说明 |
| :--- | :--- | :--- | :--- |
| **WiFi/网络** | `bind_to_wifi`, `accept_wifi_network`, `request_wifi_ssid_permission` | `lib.rs:68-69,76` | WiFi 绑定与权限请求 |
| **防劣网** | `get_avoid_bad_wifi_status`, `restore_written_settings` | `lib.rs:70-71` | 系统设置读写通道状态（前端 `android/frontend/src/hooks/tauriApi.ts:322-324,322-323`） |
| **探针** | `ping_test` | `lib.rs:67` | 协议测试 |
| **自启** | `get_boot_autostart`, `set_boot_autostart` | `lib.rs:105-106` | 开机自启（安卓语义，对应桌面 `set_auto_launch` 等但不同名） |
| **通知/电池** | `get_notification_enabled`, `get_battery_optimization_info`, `request_ignore_battery_optimizations`, `open_vendor_battery_settings`, `open_notification_settings` | `lib.rs:107,109-112` | 电池优化白名单与系统设置页跳转（`battery_cmds.rs:20,46,65,87` 实现） |
| **配置/调试** | `get_log_retention_days`, `get_debug_mode` | `lib.rs:97,99` | 安卓端补充的 getter（桌面端仅有 setter或无） |
| **人脸模型** | `face_models_state`, `face_models_download` | `lib.rs:120-121` | 2D 人脸回退模型管理 |
| **生物识别验证门** | `verify_biometric_identity` | `lib.rs:79` | 安卓生物识别验证门（前端同名方法 `verifyWindowsIdentity` 调它，见例外） |

*   **前端处理**：
    *   仅在 `android/frontend/src/hooks/tauriApi.ts` 中声明与实现（如 `bindToWifi` 行 58/199；`getAvoidBadWifiStatus` 行 138/322；`getBatteryOptimizationInfo` 行 105/292）。
    *   桌面端 `TauriApi` 接口中不包含这些方法（桌面接口行 27-120 无对应项）。

### 例外：前端同名、后端不同名

尽管遵循“后端同名”原则，以下场景存在已知特例，前端保持方法名一致，但底层 invoke 的命令名或逻辑不同：

1.  **`verifyWindowsIdentity`**
    *   **桌面**：直接 invoke `verify_windows_identity`（`tauri-app/frontend/src/hooks/tauriApi.ts:168`），后端注册于 `startup.rs:124`。
    *   **安卓**：前端方法名仍为 `verifyWindowsIdentity`（接口 `android/frontend/src/hooks/tauriApi.ts:66`），但内部逻辑包裹了系统 `BiometricPrompt` 或 2D 人脸回退（实现 `:207-244`），最终 invoke `verify_biometric_identity`（`:213`），后端注册于 `lib.rs:79`。

2.  **`getAutoLaunch` / `setAutoLaunch`**
    *   **桌面**：invoke `set_auto_launch` 与 `get_core_autostart`/`set_core_autostart`（`tauri-app/frontend/src/hooks/tauriApi.ts:221-223`；注意桌面无 `getAutoLaunch` 方法，仅有 `setAutoLaunch` 及 core autostart 对）。
    *   **安卓**：前端 `getAutoLaunch`/`setAutoLaunch`（接口 `:101-102`）invoke `get_boot_autostart` / `set_boot_autostart`（`android/frontend/src/hooks/tauriApi.ts:288-289`），并在 set 侧补装返回值以匹配桌面 `AutoLaunchResult` 类型（`:289` `.then(() => ({ success: true }))`）。

3.  **`openExternal`**
    *   **桌面**：invoke `open_external`（`tauri-app/frontend/src/hooks/tauriApi.ts:208`），失败时回退到 shell 插件（`:212-218`）。
    *   **安卓**：后端无 `open_external` 命令，前端直接调用 `@tauri-apps/plugin-opener` 的 `openerOpenUrl`（`:281`），不走 `invoke`（实现 `:276-287`）。

4.  **前端接口非互为超集**
    *   桌面 `TauriApi` 中的 `outboundSwitchNow`, `outboundRestoreNow`, `getCurrentOutboundName`, `onScheduledActionSkipped`, `onMacResetCycle`, `onUpdateNotificationClick`, `cancelDownloadUpdate`（`tauri-app/frontend/src/hooks/tauriApi.ts:76,78,80,96,98,113,108/240`）**未进入**安卓接口。
    *   安卓专属的 `bindToWifi`, `requestWifiSsidPermission`, `getAvoidBadWifiStatus`, `restoreWrittenSettings`, `getBatteryOptimizationInfo`, `onUpdateUnverified` 等（见接口 `android/frontend/src/hooks/tauriApi.ts:58,61,138,106-108,128/314`）**不存在于**桌面接口。
    *   这意味着“同名对齐”仅约束**共同方法面**。对于 `desktopOnly` 恒 reject 的方法，编译期无法察觉调用方是否误用，实际强度低于命名对齐的表面承诺。

## Connections

[[protocol-core-single-source]]、[[ipc-command-surface]]
