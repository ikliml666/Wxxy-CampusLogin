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

安卓 13+ 通知受 `POST_NOTIFICATIONS` 运行时权限管控；13 以下与国产 ROM（MIUI/EMUI/ColorOS/OriginOS 等）不存在任何能弹框的标准 API——用户在系统设置里关掉通知后，应用侧唯一手段是引导跳应用通知设置页。本项目当前代码已统一收敛到单一 helper 并覆盖所有入口，不再出现「启用系统通知开关不请求任何权限」的情况。

## 决策

通知权限索取收敛到单一 helper `android/frontend/src/lib/notificationPermission.ts` 的 `requestNotificationPermission({ openSettingsIfDenied? })`（定义于第 9-21 行），三条触发路径：

1. **首次进入**：App 顶层 `NotificationPermissionGate`（定义于 `android/frontend/src/App.tsx:406-430`，在默认导出 `App` 中渲染于第 396 行）。无 `campus-notification-prompt`（safeStorage）标记即弹引导弹窗（判断在 `android/frontend/src/App.tsx:406`），点「去开启」→ 调用 `requestNotificationPermission({ openSettingsIfDenied: true })`（第 425 行）；点任意按钮即标记 `close()`（写 `campus-notification-prompt` 于第 413 行），不重复弹。`ConfirmDialog` 使用 `confirmLabel` + `confirmVariant="default"`（第 422 行）。该 Gate 与新手向导（`OnboardingWizard` 经 lazy 引入于第 57 行、在 `AppInner` 内 `Suspense` 渲染于第 355-363 行）相互独立，均挂载于 `App` 返回树但无时序依赖。

2. **通知开关打开**：存在两个独立调用点均传 `openSettingsIfDenied: true`——① `SettingsPanel.tsx` 内联 switch handler（位于 `android/frontend/src/settings/SettingsPanel.tsx:37-52`，具体请求在 162 行：`if (checked) void requestNotificationPermission({ openSettingsIfDenied: true })`）；② `useSettings.handleToggleNotification`（`android/frontend/src/settings/useSettings.ts:44-50`，先更新配置并调 `api.setNotificationEnabled?.`（第 47 行），再请求权限（第 49 行））。两者逻辑等价，均先更新配置再调 `requestNotificationPermission`。

3. **启动后台检查**（`useMonitor`）：只弹框不跳设置——间接路径不打断操作流（`android/frontend/src/monitor/useMonitor.ts:12-54`，其中权限请求在第 53 行，调用时不传 `openSettingsIfDenied`；注释第 51-52 行说明仅弹框）。

跳设置页能力由命令 `open_notification_settings` 提供，降级链在 `MonitorServicePlugin.openNotificationSettings`（`android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt:349-380`）：API 26+ `ACTION_APP_NOTIFICATION_SETTINGS` + `EXTRA_APP_PACKAGE`（第 351-355 行）→ API 21-25 裸 action + `app_package`/`app_uid`（第 356-361 行）→ 应用详情页（第 363-366 行）→ 通用设置（第 367 行），逐个 try/catch 通过 `startSafely`（`MonitorServicePlugin.kt:240-245`）启动。沿用 `openVendorBatterySettings` 的「不用 resolveActivity 预探测」经验（`MonitorServicePlugin.kt:349-356` 注释说明包可见性坑）。

Rust 侧转发链：
- `android/src-tauri/src/lib.rs:120`：Tauri invoke_handler 注册 `battery_cmds::open_notification_settings`（在 68-130 的 generate_handler 列表中）
- `android/src-tauri/src/battery_cmds.rs:86-101`：`#[tauri::command] pub async fn open_notification_settings()` → `[cfg(mobile)]` 走插件（第 88-95 行），`[cfg(not(mobile))]` 返回 Err（第 97-101 行）
- `android/plugins/foreground-service/src/lib.rs:95-97`：`CampusMonitorService::open_notification_settings()` → `run_mobile_plugin("openNotificationSettings", ())`
- 插件初始化参见 `android/src-tauri/src/lib.rs:39` 注册 `tauri_plugin_campus_monitor_service` 及第 41 行 `tauri_plugin_notification`

## 理由

- 13+ 拒绝两次即永久拒绝，之后 `requestPermission()` 不再弹框，跳设置页是唯一路径；13- 无弹框可弹，直接跳设置页（依据 `android/frontend/src/lib/notificationPermission.ts:11-17` 逻辑：先 `isPermissionGranted`，否则 `requestPermission`，再判断 `opts?.openSettingsIfDenied` 调 `openNotificationSettings`）。
- 首启弹窗与新手向导的时序无需协调：向导是后挂载的全屏覆盖层（`android/frontend/src/App.tsx:57-65` 的 `OnboardingWizard` 在 `AppInner` 中 Suspense 渲染，且 `NotificationPermissionGate` 在 `android/frontend/src/App.tsx:396` 渲染于外层 `App`，两者独立）。
- 首启弹窗只在用户点过「去开启」/「暂不」后标记（`android/frontend/src/App.tsx:413` 写 `campus-notification-prompt`），避免每次启动骚扰。
- 不引第三方权限库：跳转链在 Kotlin 侧约 30 行写完（`MonitorServicePlugin.kt:348-379`），且厂商适配（`vendorTargets` 第 189-230 行）已有同款降级链基建。

## 备选方案

- 引入 XXPermissions/AndPermission——被弃用：依赖重、本项目只需通知一项，且已有自建降级链。
- 13+ 拒绝后不跳设置页（只弹框）——被弃用：用户明确要求首启索取，且首次弹窗有应用内说明垫底，先说明再跳符合官方「contextual permission」建议。

## 影响与约束

- 桌面端不涉及（平台专属能力例外，`android/src-tauri/src/battery_cmds.rs:97-101` 桌面调用返回 Err）。
- 跳转类命令归入 battery_cmds（模块定位已扩为「安卓系统设置页跳转命令」，见 `android/src-tauri/src/battery_cmds.rs:1-4`）。
- `ConfirmDialog` 增加可选 `confirmLabel`/`confirmVariant`（缺省保持红色破坏样式，现有调用方零改动；Android 首启弹窗显式传 `confirmVariant="default"`，`android/frontend/src/App.tsx:422`）。
- 常驻通知协议本身不变（见 [[android-persistent-notification-standard]]）；通知渠道单通道语义不变（见 [[single-notification-channel]]）。
- 后台检查启动链路中请求权限不跳设置页，避免打断用户操作（`android/frontend/src/monitor/useMonitor.ts:51-53` 注释与调用）。

## Connections

[[android-keepalive-fgs-architecture]]、[[android-persistent-notification-standard]]、[[single-notification-channel]]、[[wifi-ssid-permission-route]]