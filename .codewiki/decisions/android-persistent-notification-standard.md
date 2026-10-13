---
title: 安卓前台服务与常驻通知标准协议实现
type: module
source_files:
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/ForegroundService.kt
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt
tags: [决策, 安卓, 通知, 前台服务, 厂商适配]
---

## Overview

本模块负责 Android 端的前台服务保活、常驻通知展示及系统级资源管理。核心设计原则是**“通知走标准安卓协议，不做厂商私有 extras”**，通过 `ongoing` + `Chronometer` + `CATEGORY_SERVICE` 触发 Android 16+ 的 promoted ongoing 特性，以兼容 ColorOS 流体云、小米 HyperOS 等原生通道。

服务由 `ForegroundService` 实现，仅负责保活（常驻通知 + WifiLock + WakeLock）。监控逻辑在 Rust 侧循环，服务被系统回收时进程死亡，状态一致。Rust 通过 `MonitorServicePlugin` 与 Java 层交互，控制服务的启停、通知更新及探针窗口加锁。

## Key Components

### ForegroundService (`ForegroundService.kt`)

前台服务类，继承自 `android.app.Service`。

**常量 (Companion Object)**
- `CHANNEL_ID = "campus_monitor_v2"` (`:38`): 通知通道 ID。v2 版本使用 `IMPORTANCE_DEFAULT`，避免旧版 `IMPORTANCE_LOW` 被国产 ROM 折叠进“更多消息”。
- `NOTIFICATION_ID = 0xCAFE` (`:39`): 通知 ID。
- `EXTRA_TEXT = "text"` (`:40`): 启动服务时传递的通知文案 extra key。
- `ACTION_UPDATE = "com.campuslogin.plugin.monitorservice.UPDATE"` (`:41`): 更新通知文案的动作。
- `ACTION_STOP = "com.campuslogin.plugin.monitorservice.STOP"` (`:42`): 停止服务的动作。
- `NUDGE_THROTTLE_MS = 5000L` (`:57`): nudge 唤醒锁最小间隔，防止 WiFi 波动高频连发导致反复抑制 suspend。
- `NUDGE_WAKE_MS = 3000L` (`:61`): nudge 唤醒锁持有时长，覆盖 Rust 侧 WiFi 事件延迟 + 一次探针。
- `PROBE_WINDOW_TIMEOUT_MS = 30_000L` (`:65`): 探针窗口唤醒锁上限，防泄漏。
- `REFRESH_INTERVAL_MS = 12 * 60 * 60 * 1000L` (`:96`): 常驻通知定时刷新间隔，每 12h 重建一次以重置 Chronometer 计时起点，避免被 ROM 按陈旧度折叠。

**静态入口点 (Companion Object)**
- `beginProbeWindow()` (`:76`): 进入探针窗口，调用实例的 `acquireProbeLocks`。
- `endProbeWindow()` (`:79`): 退出探针窗口，调用实例的 `releaseProbeLocks`。
- `buildNotification(context: Context, text: String): Notification` (`:101-121`): 构建标准安卓协议通知。
    - 设置 `setOngoing(true)` (`:118`)。
    - 设置 `setOnlyAlertOnce(true)` (`:119`)。
    - 设置 `setCategory(Notification.CATEGORY_SERVICE)` (`:120`)。
    - 设置 `setUsesChronometer(true)` (`:121`)。
    - 设置 `setWhen` 基于 `startAtMs` (`:122`)。
    - 点击通知回到应用 (`mainActivityIntent`, `:127-136`)。

**实例字段**
- `wifiLock: WifiManager.WifiLock?` (`:140`): 探针窗口持有的 WifiLock。
- `nudgeWakeLock: PowerManager.WakeLock?` (`:141`): 网络变化触发的短持唤醒锁。
- `probeWakeLock: PowerManager.WakeLock?` (`:142`): 探针窗口持有的 PARTIAL_WAKE_LOCK。
- `networkCallback: ConnectivityManager.NetworkCallback?` (`:143`): 网络变化回调。
- `refreshHandler` & `refreshTick` (`:149-160`): 定时刷新通知的 Handler 和 Runnable。

**方法**
- `onCreate()` (`:163-173`): 初始化服务。
    - 设置 `startAtMs` 为当前时间 (`:164`)。
    - 调用 `startForegroundWithText("校园网监控运行中")` (`:165`)。
    - 注册网络监听 `registerNetworkWatcher()` (`:170`)。
    - 启动定时刷新 `refreshHandler.postDelayed(refreshTick, REFRESH_INTERVAL_MS)` (`:172`)。
- `acquireProbeLocks()` (`:180-199`): 探针窗口加锁。
    - 创建并 acquire `WifiLock` (`:185-191`)。
    - 创建并 acquire `PARTIAL_WAKE_LOCK`，带超时 `PROBE_WINDOW_TIMEOUT_MS` (`:193-199`)。
- `releaseProbeLocks()` (`:204-214`): 探针窗口解锁，幂等。
- `registerNetworkWatcher()` (`:221-265`): 注册网络变化监听。
    - 监听 `TRANSPORT_WIFI`, `TRANSPORT_CELLULAR`, `NET_CAPABILITY_VALIDATED` 位掩码翻转 (`:250-255`)。
    - 翻转时调用 `nudge()` 获取短持唤醒锁 (`:257`)。
    - `nudge()` 内部有 `NUDGE_THROTTLE_MS` 节流 (`:240-242`)。
- `startForegroundWithText(text: String)` (`:310-341`): 启动前台服务。
    - 创建 `IMPORTANCE_DEFAULT` 通道 (`:322-326`)。
    - 删除旧 `campus_monitor` 通道 (`:333`)。
    - 调用 `startForeground`，Android 14+ 使用 `FOREGROUND_SERVICE_TYPE_SPECIAL_USE` (`:337-339`)。
- `onStartCommand(intent: Intent?, flags: Int, startId: Int): Int` (`:296-307`): 处理启动命令。
    - `ACTION_STOP`: 调用 `stopSelf()` (`:299`)。
    - 其他: 解析 `EXTRA_TEXT` 并调用 `startForegroundWithText` (`:302`)。
- `onDestroy()` (`:309-316`): 清理资源。移除刷新回调、释放锁、注销网络回调、置空实例。

### MonitorServicePlugin (`MonitorServicePlugin.kt`)

Tauri 插件类，提供 Rust 侧调用的 Command 接口。

**Command 方法**
- `startMonitor(invoke: Invoke)` (`:34-47`): 启动前台服务。
    - 发送 `Intent` 到 `ForegroundService`，携带 `EXTRA_TEXT` (`:37-38`)。
    - Android O+ 使用 `startForegroundService` (`:41`)。
- `stopMonitor(invoke: Invoke)` (`:49-58`): 停止前台服务。
    - 发送带 `ACTION_STOP` 的 `Intent` (`:52`)。
- `updateNotification(invoke: Invoke)` (`:60-71`): 更新通知文案。
    - 直接使用 `ForegroundService.buildNotification` 构建通知 (`:67`)。
    - 调用 `NotificationManager.notify` (`:68-71`)。
    - **注意**: 依赖服务已创建的通道，若服务未启动，Android 8+ 会静默丢弃。
- `installApk(invoke: Invoke)` (`:74-93`): 安装 APK。
    - 使用 `FileProvider` 生成 `content://` URI (`:84`)。
    - 启动 `ACTION_VIEW` 安装 (`:86-90`)。
- `beginProbeWindow(invoke: Invoke)` (`:99-103`): 调用 `ForegroundService.beginProbeWindow()`。
- `endProbeWindow(invoke: Invoke)` (`:105-109`): 调用 `ForegroundService.endProbeWindow()`。
- `getPowerState(invoke: Invoke)` (`:116-125`): 获取电源状态。
    - `screenOn`: `pm.isInteractive` (`:120`)。
    - `wifiConnected`: 查询活动网络是否含 `TRANSPORT_WIFI` (`:121-124`)。
- `getBatteryOptimizationInfo(invoke: Invoke)` (`:156-167`): 获取电池优化信息。
    - `ignoring`: `pm.isIgnoringBatteryOptimizations` (`:161`)。
    - `brand` / `manufacturer`: 设备品牌 (`:162-163`)。
    - `hasVendorTarget`: 是否有厂商专用跳转页 (`:164`)。
- `requestIgnoreBatteryOptimizations(invoke: Invoke)` (`:174-183`): 申请电池优化白名单。
    - 启动 `ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` (`:178`)。
- `openVendorBatterySettings(invoke: Invoke)` (`:189-236`): 打开厂商自启/省电设置页。
    - 降级链: 厂商专用页 → 应用详情页 → 通用设置。
    - 逐个尝试 `vendorTargets` 中的 `ComponentName` (`:202-210`)。
    - 魅族特殊处理 `SHOW_APPSEC` action (`:214-222`)。
- `openNotificationSettings(invoke: Invoke)` (`:243-273`): 打开通知设置页。
    - 降级链: API 26+ `ACTION_APP_NOTIFICATION_SETTINGS` → API 21+ 裸 action → 应用详情页 → 通用设置。
- `setBootAutostart(invoke: Invoke)` (`:279-292`): 设置开机自启。
    - 启用/禁用 `BootReceiver` (`:284-288`)。
    - 持久化到 SharedPreferences (`:289-291`)。
- `isBootAutostartEnabled(invoke: Invoke)` (`:294-302`): 检查开机自启状态。
    - 查询 `BootReceiver` 组件启用状态 (`:297-299`)。

### BootReceiver (`MonitorServicePlugin.kt` 底部)

开机广播接收器。
- `onReceive(context: Context, intent: Intent)` (`:308-323`):
    - 检查 `ACTION_BOOT_COMPLETED` (`:310`)。
    - 检查 SharedPreferences 中 `boot_autstart` 标志 (`:312`)。
    - 启动 `ForegroundService` (`:314-320`)。
    - Best-effort 拉起 `MainActivity` (`:322-325`)，捕获异常忽略。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Android Application Layer                          │
│                                                                             │
│  ┌──────────────────────┐     ┌──────────────────────────────────────────┐  │
│  │   Tauri Core (Rust)  │────►│         MonitorServicePlugin             │  │
│  │                      │     │  (app.tauri.plugin.Plugin)               │  │
│  │  monitor_loop.rs     │     └──────────────┬───────────────────────────┘  │
│  │                      │                    │ Commands                     │
│  │  - state tracking    │                    ▼                              │
│  │  - notify_state      │     ┌──────────────────────────────────────────┐  │
│  │  - probe logic       │     │           ForegroundService              │  │
│  └──────────────────────┘     │  (android.app.Service)                   │  │
│                               │                                          │  │
│                               │  ┌────────────────────────────────────┐   │  │
│                               │  │  Notification Manager              │   │  │
│                               │  │  - buildNotification (Std Protocol)│   │  │
│                               │  │  - refreshTick (12h interval)      │   │  │
│                               │  └────────────────────────────────────┘   │  │
│                               │                                          │  │
│                               │  ┌────────────────────────────────────┐   │  │
│                               │  │  Lock Management                   │   │  │
│                               │  │  - acquireProbeLocks (WifiLock +   │   │  │
│                               │  │    PARTIAL_WAKE_LOCK)              │   │  │
│                               │  │  - registerNetworkWatcher (Nudge)  │   │  │
│                               │  └────────────────────────────────────┘   │  │
│                               └──────────────────────────────────────────┘  │
│                                                                             │
│  ┌──────────────────────┐     ┌──────────────────────────────────────────┐  │
│  │   BootReceiver       │────►│  ForegroundService                       │  │
│  │  (BOOT_COMPLETED)    │     │  (startForeground)                       │  │
│  └──────────────────────┘     └──────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **启动监控**:
    -   Rust `monitor_loop.rs` 调用 `start_monitor` Command。
    -   `MonitorServicePlugin.startMonitor` 发送 `Intent` 启动 `ForegroundService`。
    -   `ForegroundService.onCreate` 创建 `IMPORTANCE_DEFAULT` 通道，调用 `startForegroundWithText` 展示初始通知。
    -   注册 `refreshTick` 定时任务。

2.  **状态更新**:
    -   Rust 检测到网络状态翻转，调用 `update_notification` Command。
    -   `MonitorServicePlugin.updateNotification` 调用 `ForegroundService.buildNotification` 构建新通知。
    -   通过 `NotificationManager.notify` 更新通知文案。
    -   `startAtMs` 保持不变，Chronometer 继续计时。

3.  **探针窗口**:
    -   Rust 巡检开始时调用 `beginProbeWindow`。
    -   `ForegroundService.beginProbeWindow` 调用 `acquireProbeLocks`。
    -   持有 `WifiLock` (抑制 WiFi 省电) 和 `PARTIAL_WAKE_LOCK` (CPU 唤醒)。
    -   Rust 巡检结束后调用 `endProbeWindow`。
    -   `ForegroundService.endProbeWindow` 调用 `releaseProbeLocks` 释放锁。

4.  **网络变化 Nudge**:
    -   `ConnectivityManager` 触发 `onCapabilitiesChanged`。
    -   若关注字段 (WiFi/蜂窝/VALIDATED) 发生翻转，且超过 `NUDGE_THROTTLE_MS`，则调用 `nudge()`。
    -   `nudge()` 获取短持 `PARTIAL_WAKE_LOCK` (`NUDGE_WAKE_MS`)，确保 Rust 轮询 tick 能准时执行。

5.  **通知刷新**:
    -   每 `REFRESH_INTERVAL_MS` (12h)，`refreshTick` 触发。
    -   重置 `startAtMs` 为当前时间，重建通知。
    -   防止因通知长期不更新被 ROM 折叠进“更多消息”。

6.  **开机自启**:
    -   系统发送 `ACTION_BOOT_COMPLETED`。
    -   `BootReceiver` 检查 SharedPreferences 中 `boot_autstart` 标志。
    -   若开启，启动 `ForegroundService` 并尝试拉起 `MainActivity`。

## Connections

[[android-persistent-notification-standard]] (决策依据：标准协议优先)
[[single-notification-channel]] (通道管理策略)