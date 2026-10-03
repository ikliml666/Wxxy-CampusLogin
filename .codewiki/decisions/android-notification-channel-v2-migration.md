---
title: 安卓常驻通知通道 v2：IMPORTANCE_DEFAULT + 版本化换 ID 迁移
type: decision
source_files:
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/ForegroundService.kt
tags: [决策, 安卓, 通知, 前台服务, 通知通道]
---

## 背景

2026-10-01 曾决策「不做通道 importance 迁移（留作后备）」（v2.3.8 changelog），后续用户报告常驻通知仍被折叠进「更多消息」，要求从底层解决。根因：`NotificationChannel` 的 importance 创建后**不可变**，现役通道 `campus_monitor` 建于 `IMPORTANCE_LOW`——importance 低于 DEFAULT 在 AOSP 即静默通知桶（无横幅无提示音），也是国产 ROM（澎湃OS 等）折叠「更多消息」的底层判据；通道档位是评分模型（文本/点击率因子）之上的先决条件，2026-10-01 的 12h 重建与可点击优化动不了它。

## 决策

2026-10-03：通道 ID 版本化 `campus_monitor` → `campus_monitor_v2`（`ForegroundService.kt:34`），新通道 `IMPORTANCE_DEFAULT` + `setSound(null, null)` + `setShowBadge(false)`（`startForegroundWithText`，`ForegroundService.kt:320-330`），随后 `deleteNotificationChannel("campus_monitor")` 删除旧低档通道。**撤销 2026-10-01「不做通道 importance 迁移（留作后备）」决策**——后备启用即其失效。

## 理由

importance 不可变 ⇒ 想升档只能换 ID 重建；DEFAULT 不触发 heads-up 横幅（那需要 HIGH），无铃声无角标，「安静常驻」体验不变但归入常规列表，不再折叠。

## 备选方案

保留 LOW 档只调文本/点击率（2026-10-01 方案）——已被折叠投诉证伪；升 HIGH——会触发横幅，过度；厂商私有 extras——违反 [[android-persistent-notification-standard]]。

## 影响与约束

- 后续调通道行为（声音/震动/横幅）都必须换新 ID 再迁移，**不要原地改 v2 通道参数**（不可变只会静默忽略）。
- foregroundServiceType 不动（Android 15 BOOT_COMPLETED FGS 限制红线，见 [[android-keepalive-fgs-architecture]]）。
- 12h 通知重建、`buildNotification` 形态单点不变。

## Connections

[[android-persistent-notification-standard]]、[[android-keepalive-fgs-architecture]]
