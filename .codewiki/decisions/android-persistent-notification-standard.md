---
title: 安卓常驻通知走标准安卓协议，不做厂商私有 extras
type: decision
source_files:
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/ForegroundService.kt
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt
  - android/src-tauri/src/monitor_loop.rs
tags: [决策, 安卓, 通知, 前台服务, 厂商适配]
---

## 背景

各家 ROM 对"岛态/常驻"通知有自己的私有字段与提升开关，逐厂商适配成本高且不统一。

## 决策

2026-09-08：安卓常驻通知走**标准安卓协议**（ongoing + Chronometer + CATEGORY_SERVICE，即 promoted ongoing 特征），**不做厂商私有 extras**。

## 理由

统一行为优先于逐厂商适配；厂商岛态由 ROM 决定，用户侧需开"实时通知提升"类权限，这是已知体验边界。该特征组合即 Android 16 promoted ongoing 的触发特征，ColorOS 16 流体云/小米 HyperOS 等按标准 Live Updates 自动识别（`ForegroundService.kt:96-101` 注释）。

## 备选方案

逐厂商私有 extras 适配——因统一行为优先被弃用；厂商岛态交由 ROM 决定。

## 影响与约束

- 形态单点：服务侧与 `updateNotification` 命令共用 `ForegroundService.buildNotification`（`ForegroundService.kt:103-121`）：`setOngoing(true)`（`:115`）+ `setOnlyAlertOnce(true)`（`:116`）+ `setCategory(Notification.CATEGORY_SERVICE)`（`:117`）+ `setUsesChronometer(true)`（`:118`）+ `setWhen(if (startAtMs == 0L) ... else startAtMs)`（`:119`）；插件侧以 `NOTIFICATION_ID = 0xCAFE`（`:35`）+ 同一构建 notify（`MonitorServicePlugin.kt:72-75`）。通道由 `startForegroundWithText` 创建（`ForegroundService.kt:317-338`），IMPORTANCE_DEFAULT（`:324`），旧 "campus_monitor" IMPORTANCE_LOW 通道在升级后删除（`:330`）；插件侧不建通道（`MonitorServicePlugin.kt:67-77`）。

- Chronometer 起点：存于 `startAtMs`（`ForegroundService.kt:78-80`），`onCreate` 置为服务启动时刻（`:161`）；Rust 侧状态翻转重建通知不改起点；服务侧每 12h（`REFRESH_INTERVAL_MS = 12 * 60 * 60 * 1000L`，`:93`）定时刷新把起点拨到 now、计时随之重计（`refreshTick`，`:150-157`，`onCreate` 经 `:169` 注册，复用当前文案 `lastText :84`）；`onDestroy` 清零（`:311`）。

- Rust 侧只在状态翻转时 notify：`MONITOR.notified_online`（`monitor_loop.rs:46-49`）记录已展示状态码，`notify_state`（`monitor_loop.rs:1483-1491`）给出 1=在线 / 2=未连接 / 4=WiFi 未连接（3 被"非检测时段"占用，`monitor_loop.rs:1286-1296`）；仅翻转时经 `update_notification` 重建文案"监控运行中 · <状态>"（`monitor_loop.rs:1473-1478`）。启动监控时 `start_monitor("校园网监控运行中")` 并清零状态记忆（`monitor_loop.rs:212`、`:219`）。

- 常驻通知**不带大图**——看板娘大图只属系统通知路径（`notify_system` 的 `large_icon`，`monitor_loop.rs:161-178`，大图在 `:173`，见 [[system-notification-mascot-avatar]]）。

- 已知边界：`updateNotification` 依赖服务已建通道（通道仅由 `startForegroundWithText` 创建，`ForegroundService.kt:317-338`；插件侧不建通道，`MonitorServicePlugin.kt:67-77`），服务未启动时 Android 8+ 会静默丢弃。

## Connections

[[android-keepalive-fgs-architecture]]、[[single-notification-channel]]
