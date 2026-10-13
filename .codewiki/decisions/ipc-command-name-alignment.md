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

1.  **统一前端接口**：在 `tauri-app/frontend/src/hooks/tauriApi.ts` 和 `android/frontend/src/hooks/tauriApi.ts` 中定义相同的 `TauriApi` 接口。
2.  **后端同名对齐**：安卓端在 `android/src-tauri/src/lib.rs` 注册的命令名必须与桌面端 `tauri-app/src-tauri/src/app/startup.rs` 保持一致（例如 `do_login`, `save_config`, `get_config`）。
3.  **差异仅在实现**：同名方法允许参数按平台裁剪或类型不同。
    *   例：`doLogin` 桌面端带可选参数 `adapterName?: string`（`tauri-app/frontend/src/hooks/tauriApi.ts:42`），安卓端无参（`android/frontend/src/hooks/tauriApi.ts:71`）。
    *   例：`verifyWindowsIdentity` 桌面直接 invoke `verify_windows_identity`，安卓包一层生物识别逻辑后 invoke `verify_biometric_identity`（见“例外”章节）。
4.  **新增命令流程**：必须在同一提交内完成“三处同改”：
    *   桌面后端注册（`tauri-app/src-tauri/src/app/startup.rs`）
    *   安卓后端注册（`android/src-tauri/src/lib.rs`）
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
*   **安卓端**：50 条命令（`android/src-tauri/src/lib.rs:68` 宏，条目 `:69-118`）。
*   **桌面端**：65 条命令（`tauri-app/src-tauri/src/app/startup.rs:62` 宏，条目 `:63-127`）。
*   **共同命令**：38 条。
*   **差值**：全部为平台专属命令面，仅单端注册，不进“三处同改”。

### 平台专属命令分类

#### 1. 桌面独有 (27 条)
这些命令在安卓端通过 `desktopOnly` helper 恒 reject，或在安卓 `TauriApi` 接口中不存在。

| 类别 | 命令名 | 说明 |
| :--- | :--- | :--- |
| **适配器族** | `get_adapters`, `get_adapter_details`, `get_disabled_adapters`, `enable_adapter` | 桌面网络适配器管理 |
| **DHCP** | `dhcp_renew_all`, `dhcp_release_renew`, `dhcp_release_renew_adapter` | DHCP 续租/释放 |
| **DNS/DHCP** | `check_dns_doh_status`, `setup_dns_doh`, `reset_dns` | DNS-over-HTTPS 设置 |
| **窗口/轻量化** | `show_window`, `minimize_window`, `close_window`, `notify_window_ready` | 窗口管理与轻量化退出机制 |
| **托盘/退出** | `cancel_auto_exit` | 取消自动退出倒计时 |
| **出站切换** | `outbound_switch_now`, `outbound_restore_now`, `get_current_outbound_name` | 智能出站切换控制 |
| **导出/诊断** | `export_config`, `import_config`, `export_diagnostics` | 依赖 DPAPI 密文与本地文件系统，安卓无对应实现 |
| **其他** | `open_external`, `render_heartbeat`, `get_gpu_info`, `verify_windows_identity`, `get_auto_launch`, `set_auto_launch` | 桌面特定系统交互 |

*   **安卓端处理**：
    *   接口声明存在于 `android/frontend/src/hooks/tauriApi.ts`（如 `exportConfig` `:93`）。
    *   实现统一走 `desktopOnly` helper（定义于 `:14-15`），例如 `exportConfig` 实现于 `:200`。
    *   部分桌面专属事件（如 `onAdaptersChanged`）在安卓端使用 `noopListener`（`:18`，适配器事件 `:255-258`）消化，返回 no-op 清理函数。

#### 2. 安卓独有 (15+ 条)
这些命令仅注册于安卓后端，桌面端无对应物。

| 类别 | 命令名 | 文件/位置 | 说明 |
| :--- | :--- | :--- | :--- |
| **WiFi/网络** | `bind_to_wifi`, `accept_wifi_network`, `request_wifi_ssid_permission` | `lib.rs:69-71` | WiFi 绑定与权限请求 |
| **防劣网** | `get_avoid_bad_wifi_status`, `restore_written_settings` | `lib.rs:72-73` | 系统设置读写通道状态 |
| **探针** | `ping_test`, `detect_campus` | `lib.rs:68, 76` | 协议测试与校园网检测 |
| **自启** | `get_boot_autostart`, `set_boot_autostart` | `lib.rs:98-99` | 开机自启（安卓语义） |
| **SOC/GPU** | `get_soc_info` | `lib.rs:88` | 设备 SoC 性能信息 |
| **生物识别** | `verify_biometric_identity` | `lib.rs:77` | 安卓生物识别验证门 |
| **电池/通知** | `get_battery_optimization_info`, `request_ignore_battery_optimizations`, `open_vendor_battery_settings`, `open_notification_settings` | `battery_cmds.rs:20/46/65/87` | 电池优化白名单与系统设置页跳转 |

*   **前端处理**：
    *   仅在 `android/frontend/src/hooks/tauriApi.ts` 中声明与实现（如 `getSocInfo` `:128`，实现 `:309`；`getAvoidBadWifiStatus` `:145`，实现 `:324`）。
    *   桌面端 `TauriApi` 接口中不包含这些方法。

### 例外：前端同名、后端不同名

尽管遵循“后端同名”原则，以下场景存在已知特例，前端保持方法名一致，但底层 invoke 的命令名或逻辑不同：

1.  **`verifyWindowsIdentity`**
    *   **桌面**：直接 invoke `verify_windows_identity`（`tauri-app/frontend/src/hooks/tauriApi.ts:165`）。
    *   **安卓**：前端方法名仍为 `verifyWindowsIdentity`，但内部逻辑包裹了系统 `BiometricPrompt` 或 2D 人脸回退，最终 invoke `verify_biometric_identity`（`android/frontend/src/hooks/tauriApi.ts:215-246`）。

2.  **`getAutoLaunch` / `setAutoLaunch`**
    *   **桌面**：invoke `get_auto_launch` / `set_auto_launch`（`tauri-app/frontend/src/hooks/tauriApi.ts:217-218`）。
    *   **安卓**：invoke `get_boot_autostart` / `set_boot_autostart`（`android/frontend/src/hooks/tauriApi.ts:289-290`），并在 set 侧补装返回值以匹配桌面类型。

3.  **`openExternal`**
    *   **桌面**：invoke `open_external`（`tauri-app/frontend/src/hooks/tauriApi.ts:204`），失败时回退到 shell 插件（`:209`）。
    *   **安卓**：后端无 `open_external` 命令，前端直接调用 `@tauri-apps/plugin-opener` 的 `openerOpenUrl`（`:277-288`）。

4.  **前端接口非互为超集**
    *   桌面 `TauriApi` 中的 `outboundSwitchNow`, `outboundRestoreNow`, `getCurrentOutboundName`, `onUpdateNotificationClick`（`tauri-app/frontend/src/hooks/tauriApi.ts:75/77/79/107`）**未进入**安卓接口。
    *   安卓专属的 `getSocInfo`, `getAvoidBadWifiStatus`, `restoreWrittenSettings` **不存在于**桌面接口。
    *   这意味着“同名对齐”仅约束**共同方法面**。对于 `desktopOnly` 恒 reject 的方法，编译期无法察觉调用方是否误用，实际强度低于命名对齐的表面承诺。

## Connections

[[protocol-core-single-source]]、[[ipc-command-surface]]