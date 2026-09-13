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

统一行为优先于逐厂商适配；厂商岛态由 ROM 决定，用户侧需开"实时通知提升"类权限，这是已知体验边界。

## 备选方案

逐厂商私有 extras 适配——因统一行为优先被弃用；厂商岛态交由 ROM 决定。

## 影响与约束

新增通知能力不得写入厂商私有字段。形态单点：服务侧与 `updateNotification` 命令共用 `ForegroundService.buildNotification`（`ForegroundService.kt:78-93`：`setOngoing` + `setUsesChronometer` + `setCategory(CATEGORY_SERVICE)` + `setOnlyAlertOnce`；Chronometer 起点固定为服务启动时刻，重建通知不重置计时）。常驻通知**不带大图**——看板娘大图只属系统通知路径（`notify_system` 的 `large_icon`，见 [[system-notification-mascot-avatar]]）。已知边界：`updateNotification` 依赖服务已建通道，服务未启动时 Android 8+ 会静默丢弃。

## Connections

[[android-keepalive-fgs-architecture]]、[[single-notification-channel]]
