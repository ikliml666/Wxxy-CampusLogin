---
title: 命令面两端同名对齐（新增命令三处同改）
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

前端需要同时消费桌面端与安卓端的后端能力，两端命令若各自命名，前端就要按平台分叉。

## 决策

前端 `tauriApi` 一套接口两端消费；安卓命令与桌面同名（`do_login`/`save_config`…），差异只在实现——同名方法的参数允许按平台裁剪（如 `do_login` 桌面带 `adapterName?: string`（`tauri-app/frontend/src/hooks/tauriApi.ts:42`），安卓无参（`android/frontend/src/hooks/tauriApi.ts:71`））。

新增命令时：桌面在 `tauri-app/src-tauri/src/app/startup.rs` 注册，安卓在 `android/src-tauri/src/lib.rs` 注册，并在两端 `frontend/src/hooks/tauriApi.ts` 各加一份方法（同一提交内双端同改）——共三处（其中 `tauriApi.ts` 桌面/安卓各一份落点）。

## 理由

旧文档未记录（"差异只在实现"是结果，未写权衡过程）。

## 备选方案

旧文档未记录。

## 影响与约束

"新增命令必须三处同改"是硬约束。当前规模（2026-10-03 实读）：安卓 53 条（`android/src-tauri/src/lib.rs:57` 宏，条目 `:58-110`）、桌面 65 条（`tauri-app/src-tauri/src/app/startup.rs:62` 宏，条目 `:63-127`）；两端共同命令 38 条，差值全部是平台专属命令面——桌面独有 27 条（适配器/DHCP/DNS/窗口/托盘/出站切换/导出导入与诊断等），安卓独有 15 条（WiFi 绑定与系统设置类 `bind_to_wifi`/`accept_wifi_network`/`get_avoid_bad_wifi_status`/`restore_written_settings`/`request_wifi_ssid_permission`/`detect_campus`/`ping_test`、开机自启 `get_boot_autostart`/`set_boot_autostart`、`get_soc_info`、生物识别 `verify_biometric_identity`、电池/通知设置页四命令），均仅单端注册，不进"三处同改"。

**例外（平台专属能力）**：安卓电池优化白名单与系统设置页跳转四命令 `get_battery_optimization_info` / `request_ignore_battery_optimizations` / `open_vendor_battery_settings` / `open_notification_settings`（`android/src-tauri/src/battery_cmds.rs:20/46/65/87`，实现在 Kotlin 侧 foreground-service 插件，桌面无对应 API）只在安卓 `lib.rs:100-103` 与安卓树 `tauriApi.ts` 各注册一处，不要求桌面同名占位。反向同理：桌面专属的导出/导入类命令 `export_config`/`import_config`/`export_diagnostics`（2026-09-13，DPAPI 密文与本地文件系统依赖，安卓 `crypto::encrypt` 非 Windows 恒 Err）只在桌面 `startup.rs:66/67/113` 注册，安卓树 `tauriApi.ts:200-202` 走 `desktopOnly` 占位。

**前端同名、后端不同名的已知特例**（"差异只在实现"的边界）：`verifyWindowsIdentity` 桌面直接 invoke `verify_windows_identity`（`tauri-app/frontend/src/hooks/tauriApi.ts:165`），安卓包一层系统 BiometricPrompt/2D 人脸回退后 invoke `verify_biometric_identity`（`android/frontend/src/hooks/tauriApi.ts:215-246`）；`getAutoLaunch`/`setAutoLaunch` 桌面 invoke `get_auto_launch`/`set_auto_launch`（`tauri-app/frontend/src/hooks/tauriApi.ts:217-218`），安卓 invoke `get_boot_autostart`/`set_boot_autostart` 并在 set 侧补装返回值（`android/frontend/src/hooks/tauriApi.ts:289-290`）。

注意前端 `TauriApi` 类型在安卓侧仍保留桌面方法并以 `desktopOnly` 统一恒 reject（helper 定义 `android/frontend/src/hooks/tauriApi.ts:14-15`，如导出导入与适配器族 `:200-206`、窗口族 `:251-252`/`:303`），桌面专属事件以 `noopListener` 空监听消化（`:18`，适配器事件 `:255-258`、自动退出/课堂退出事件 `:298-301`）；但两份 interface 已不互为超集——桌面 interface 的 `outboundSwitchNow`/`outboundRestoreNow`/`getCurrentOutboundName`/`onUpdateNotificationClick`（`tauri-app/frontend/src/hooks/tauriApi.ts:75-79`、`:107`）未进安卓 interface，安卓专属的 `getSocInfo`/`getAvoidBadWifiStatus`/`restoreWrittenSettings`（`android/frontend/src/hooks/tauriApi.ts:128`、`:145-147`）亦只存在于安卓侧。同名对齐只约束共同方法面；调用方对 `desktopOnly` 恒 reject 在编译期无法察觉，实际强度低于命名对齐的表面承诺。

## Connections

[[protocol-core-single-source]]、[[ipc-command-surface]]
