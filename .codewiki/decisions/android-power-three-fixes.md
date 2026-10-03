---
title: 安卓省电三刀定调（真机归因驱动）
type: decision
source_files:
  - android/frontend/src/hooks/useAdaptiveFramePace.ts
  - android/frontend/src/lib/renderLiveness.ts
  - tauri-app/frontend/src/lib/renderLiveness.ts
  - android/frontend/src/index.css
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/battery_cmds.rs
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/ForegroundService.kt
tags: [决策, 安卓, 省电, 帧率, 后台巡检]
---

## 背景

真机实测前台静置满帧合成（10s 2438 帧、RenderThread 40%+、宿主 ≈0.8 核）是 2.5W / 20% CPU 的主因；后台巡检同时存在常驻唤醒锁。

## 决策

2026-09-13：安卓省电三刀——

1. **帧控改造**：帧控轮询 `rAF` 改 `setInterval(250ms)`（`useAdaptiveFramePace.ts:18` 的 `PACE_POLL_MS`，:64 落地 setInterval，活跃窗 2s :16）；`renderLiveness` 改按需短探测（双端逐行相同：2 帧 rAF 探针 + 500ms 兜底复位，`renderLiveness.ts:24-34`；4s 刷新 :18、10s 失活阈值 :21，`isRenderLoopAlive` :36-40）；修复 `.anim-idle .animate-pulse` 冻结失效（`index.css:738-740`，曾与 `.scrollbar-none` 并进同一逗号规则，现已每个选择器独立成规则）。修复后真机实测静置 10s **0 帧、全线程 <1%**。
2. **后台巡检分档**：`effective_interval_ms` 纯函数（`monitor_loop.rs:83-90`：亮屏 + WiFi = 基础档 60s，灭屏或蜂窝 = idle 档 300s，基础档下限 5000ms）——唤醒周期恒为基础档（timer 恒按基础档建），是否真跑整轮探针由 idle 档判定跳拍（`monitor_loop.rs:602-611`，保证亮屏/回 WiFi 后最迟一拍恢复）；分档语义有内嵌单测覆盖（`monitor_loop.rs:1562-1582`，含下限 5s 用例）。
3. **锁改探针窗口**：`WifiLock`/`WakeLock` 改按需持有——Rust 侧 `begin_probe_window()`/`end_probe_window()`（`plugins/foreground-service/src/lib.rs` 插件命令面转发 Kotlin）+ `ProbeWindowGuard` RAII（`monitor_loop.rs:554-562`，Drop 自动 `end_probe_window`；`run_check_once` 每轮 :1302-1306 开窗）；Kotlin 侧 `beginProbeWindow`/`endProbeWindow`（`ForegroundService.kt:62,65`）→ `acquireProbeLocks`/`releaseProbeLocks`（:179,:204，`PROBE_WINDOW_TIMEOUT_MS=30_000L` :52 超时兜底防泄漏），nudge 按关注字段位掩码翻转去重（bit0=TRANSPORT_WIFI、bit1=TRANSPORT_CELLULAR、bit2=NET_CAPABILITY_VALIDATED，:257-263）+ 5s 节流（`NUDGE_THROTTLE_MS=5000L` :46）——**常驻锁清零**（依据：vitals 2h/24h 异常线 + CDD [C-3-1] WiFi 省电条款）。

## 理由

根因是**页面存在常驻 rAF**（`useAdaptiveFramePace` 的 rAF 轮询 + `renderLiveness` 模块级 rAF 循环）：Chromium 对"有活跃 rAF"的页面持续满帧派发 BeginFrame，合成器永不休眠——**用 rAF 做的帧率控制器自己阻止了合成器休眠**。

巡检分档经 B2 `getPowerState` **查询式**（`MonitorServicePlugin.kt:128-135`：返回 `screenOn` = `pm.isInteractive`、`wifiConnected` = 实时 TRANSPORT_WIFI 查询，不依赖缓存），不做屏幕广播接收器。

## 备选方案

- 常驻 WifiLock/WakeLock —— 改为探针窗口按需持有（常驻锁是 CDD 功耗异常项）；
- 屏幕广播接收器 —— 弃用，改 `getPowerState` 查询式；
- 电池白名单入口的跳转预探测：**直接 `startActivity` + `catch(ActivityNotFoundException)` 降级，禁用 `resolveActivity` 预探测**（Android 11+ 包可见性会误判不存在、直接降级）——这是对本条决策的实现约束，落在 Kotlin 侧 `MonitorServicePlugin.kt`（`getBatteryOptimizationInfo` :219、`openVendorBatterySettings` :252），`battery_cmds.rs` 四个命令（:19/:45/:65/:87）仅做转发。

## 影响与约束

不得重新引入常驻 rAF 帧控与常驻锁；省电相关改动必须真机实测（静置 10s 帧数与全线程占用）。

## Connections

[[android-keepalive-fgs-architecture]]、[[rAF-blocks-compositor-idle]]、[[css-comma-selector-shared-body-pitfall]]、[[airplane-mode-kills-wireless-adb]]
