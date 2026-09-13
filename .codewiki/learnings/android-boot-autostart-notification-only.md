---
title: Android 10+ 开机自启只有通知没有界面
type: learning
source_files:
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt
  - android/src-tauri/gen/android/app/src/main/AndroidManifest.xml
  - android/src-tauri/src/monitor_loop.rs
tags: [教训, 安卓, 开机自启, rom 限制]
---

## 现象

Android 10+ 开机自启后只弹了通知，界面没有起来。

## 根因

后台启 Activity 受 ROM 限制（MIUI 需"后台弹出界面"权限）。开机自启链路是：`BOOT_COMPLETED`（manifest 声明在 `AndroidManifest.xml:51-58`，权限 :7）→ `BootReceiver`（`MonitorServicePlugin.kt:342-362`）先 `startForegroundService`（:349-353）再 best-effort `startActivity`（:354-360，`FLAG_ACTIVITY_NEW_TASK`）——第二步会被 ROM 拦掉，通知（第一步）照常。

## 解决

**被拦时用户点开 app 一次即恢复完整链路——这是设计内行为**，不做绕过（注释即写在 `MonitorServicePlugin.kt:337-341`：FGS 只提供通知壳与保活锁，Rust 监控循环由 Activity 的 `run_startup_tasks` 驱动，故必须拉起 MainActivity）。

2026-09-13 补充：本轮保活改造（电池优化白名单 `battery_cmds.rs`、厂商自启/省电页跳转）只降低进程被系统/ROM 清掉的概率，**不解除"后台弹 Activity 受限"**，本结论不变。

## 教训

不要试图用非常规手段强制后台弹 Activity；把"点一次即恢复"写进设计预期，避免被当缺陷反复"修"。

## Connections

[[android-persistent-notification-standard]]
