---
title: 安卓通知权限主动索取链路（13+ 弹框 / 13- 与国产 ROM 跳设置页）
type: decision
source_files:
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt
  - android/plugins/foreground-service/src/lib.rs
  - android/src-tauri/src/battery_cmds.rs
  - android/src-tauri/src/lib.rs
  - android/frontend/src/lib/notificationPermission.ts
  - android/frontend/src/App.tsx
  - android/frontend/src/settings/useSettings.ts
  - android/frontend/src/settings/SettingsPanel.tsx
  - android/frontend/src/monitor/useMonitor.ts
tags: [安卓, 通知, 权限, 国产ROM, 决策]
---

## 背景

安卓 13+ 通知受 `POST_NOTIFICATIONS` 运行时权限管控；13 以下与国产 ROM（MIUI/EMUI/ColorOS/OriginOS 等）不存在任何能弹框的标准 API——用户在系统设置里关掉通知后，应用侧唯一手段是 `areNotificationsEnabled()` 检测 + 引导跳应用通知设置页（GitHub 调研 2026-09-21：getActivity/XXPermissions `NotificationServicePermission` 的跳转链是业界蓝本；`tauri-plugin-notification` 只覆盖 13+ 弹框，13- 的 `requestPermission()` 是无害空转）。本项目此前只在「启动后台检查」时请求 13+ 弹框，「启用系统通知」开关不请求任何权限。

## 决策

2026-09-21：通知权限索取收敛到单一 helper `android/frontend/src/lib/notificationPermission.ts` 的 `requestNotificationPermission({ openSettingsIfDenied? })`，三条触发路径：

1. **首次进入**：App 顶层 `NotificationPermissionGate`（`App.tsx:L398-L421`），无 `campus-notification-prompt`（safeStorage）标记即弹引导弹窗（`App.tsx:L401-L403`），点「去开启」→ 13+ 系统弹框、仍无权限则跳设置页（`App.tsx:L415-L417`）；点任意按钮即标记 `close()`（`App.tsx:L404-L407`），不重复弹。`ConfirmDialog` 使用 `confirmLabel` + `confirmVariant="default"`（非默认 destructive 红色破坏样式，`App.tsx:L413-L414`）。
2. **通知开关打开**：存在两个独立调用点均传 `openSettingsIfDenied: true`——① 手机壳 `SettingsPanel.tsx:L304-L308` 内联 switch handler；② TabletShell 通过 `useSettings.handleToggleNotification`（`useSettings.ts:L44-L50`，`TabletShell.tsx:L100,L238` 传入 TitleBar 铃铛）。两者逻辑等价，均先更新配置再调 `requestNotificationPermission`。
3. **启动后台检查**（`useMonitor`）：只弹框不跳设置——间接路径不打断操作流（`useMonitor.ts:L37`，调用时不传 `openSettingsIfDenied`）。

跳设置页能力由新命令 `open_notification_settings`（battery_cmds → 插件 `MonitorServicePlugin.openNotificationSettings`）提供，降级链：API 26+ `ACTION_APP_NOTIFICATION_SETTINGS` + `EXTRA_APP_PACKAGE`（`MonitorServicePlugin.kt:L318-L322`）→ API 21-25 裸 action + `app_package`/`app_uid`（`MonitorServicePlugin.kt:L323-L328`）→ 应用详情页 → 通用设置，逐个 try/catch（沿用 `openVendorBatterySettings` 的「不用 resolveActivity 预探测」经验，`MonitorServicePlugin.kt:L307-L314` 注释说明包可见性坑）。

Rust 侧转发链：
- `lib.rs:L95-L97`：`CampusMonitorService::open_notification_settings()` → `run_mobile_plugin("openNotificationSettings")`
- `battery_cmds.rs:L87-L101`：`#[tauri::command] pub async fn open_notification_settings()` → `[cfg(mobile)]` 走插件，`[cfg(not(mobile))]` 返回 Err
- `lib.rs:L104`：Tauri invoke_handler 注册 `battery_cmds::open_notification_settings`

## 理由

- 13+ 拒绝两次即永久拒绝，之后 `requestPermission()` 不再弹框，跳设置页是唯一路径；13- 无弹框可弹，直接跳设置页。
- 首启弹窗与新手向导的时序无需协调：向导是后挂载的全屏覆盖层（fixed z-50），天然先于本弹窗展示。
- 首启弹窗只在用户点过「去开启」/「暂不」后标记，避免 XXPermissions issue #316 式「每次启动骚扰」。
- 不引第三方权限库：跳转链 60 行内可写完，且厂商适配（vendorTargets）已有同款降级链基建。

## 备选方案

- 引入 XXPermissions/AndPermission——被弃用：依赖重、本项目只需通知一项。
- 13+ 拒绝后不跳设置页（只弹框）——被弃用：用户明确要求首启索取，且首次弹窗有应用内说明垫底，先说明再跳符合官方「contextual permission」建议。

## 影响与约束

- 桌面端不涉及（平台专属能力例外，`battery_cmds.rs` 桌面调用返回 Err）。
- 新增跳转类命令归入 battery_cmds（模块定位已扩为「安卓系统设置页跳转命令」）。
- `ConfirmDialog` 增加可选 `confirmLabel`/`confirmVariant`（缺省保持红色破坏样式，现有调用方零改动；Android 首启弹窗显式传 `confirmVariant="default"`，`App.tsx:L414`）。
- 常驻通知协议本身不变（见 [[android-persistent-notification-standard]]）；通知渠道单通道语义不变（见 [[single-notification-channel]]）。

## Connections

[[android-keepalive-fgs-architecture]]、[[android-persistent-notification-standard]]、[[single-notification-channel]]、[[wifi-ssid-permission-route]]
