---
title: Android 开机自启与通知保活机制
type: learning
source_files:
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt
  - android/src-tauri/gen/android/app/src/main/AndroidManifest.xml
tags: [安卓, 开机自启, ROM 限制, 前台服务, 电池优化]
---

## Overview

本模块负责 Android 端应用的后台监控保活、开机自启引导及系统权限设置。核心设计哲学是：**前台服务 (Foreground Service, FGS) 仅提供通知壳与保活锁，不保证界面拉起**。

在 Android 10+ 及各类定制 ROM（MIUI, ColorOS, etc.）下，`BOOT_COMPLETED` 广播后直接 `startActivity` 会被系统拦截。因此，开机自启链路被设计为“最佳努力”模式：确保前台服务启动以维持进程存活和通知展示，界面拉起作为非关键路径。用户首次手动打开应用即可恢复完整的 Rust 监控循环与自动登录流程。

此外，模块提供了针对主流厂商的自启/省电白名单跳转链，以及电池优化白名单的申请接口，旨在降低进程被系统清理的概率，而非强行绕过系统限制。

## Key Components

### MonitorServicePlugin
**文件**: `android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt:26-341`
Tauri 插件类，暴露给前端/Rust 的所有命令入口。

**字段与依赖**:
- `activity`: `android.app.Activity` (构造函数注入，用于启动组件和获取系统服务)

**公开命令 (@Command)**:

1.  **`startMonitor(invoke: Invoke)`** (`MonitorServicePlugin.kt:33-45`)
    -   用途: 启动前台监控服务。
    -   逻辑: 解析 `StartArgs` (title)，构建 Intent 指向 `ForegroundService`，添加额外文本。根据 SDK 版本选择 `startForegroundService` (O+) 或 `startService`。返回 `started: true`。

2.  **`stopMonitor(invoke: Invoke)`** (`MonitorServicePlugin.kt:47-56`)
    -   用途: 停止前台监控服务。
    -   逻辑: 发送带有 `ForegroundService.ACTION_STOP` Action 的 Intent。注释说明使用 `startService` 通知服务自杀，以避免 `stopService` 与 `startForeground` 的竞态条件。返回 `started: false`。

3.  **`updateNotification(invoke: Invoke)`** (`MonitorServicePlugin.kt:58-69`)
    -   用途: 更新前台通知内容。
    -   逻辑: 解析 `TextArgs` (text)，通过 `NotificationManager` 发送标准协议通知（计时器/免重复提醒），通道由服务侧创建。ID 固定为 `ForegroundService.NOTIFICATION_ID`。

4.  **`installApk(invoke: Invoke)`** (`MonitorServicePlugin.kt:71-93`)
    -   用途: 触发 APK 安装流程。
    -   逻辑: 解析 `TextArgs` (文件名)。从 `activity.filesDir/update/` 目录查找对应 APK。使用 `FileProvider` 生成 `content://` URI，避免 Android 7+ 的 `FileUriExposedException`。启动 `ACTION_VIEW` 意图请求读取权限。包含路径逃逸防护（仅取文件名）。

5.  **`beginProbeWindow(invoke: Invoke)`** (`MonitorServicePlugin.kt:95-98`)
    -   用途: 开始网络探针窗口。
    -   逻辑: 静态调用 `ForegroundService.beginProbeWindow()`。用于在 Rust 巡检期间持有 WifiLock 和唤醒锁，窗口外释放以符合 CDD 省电要求。

6.  **`endProbeWindow(invoke: Invoke)`** (`MonitorServicePlugin.kt:100-103`)
    -   用途: 结束网络探针窗口。
    -   逻辑: 静态调用 `ForegroundService.endProbeWindow()`。

7.  **`getPowerState(invoke: Invoke)`** (`MonitorServicePlugin.kt:115-123`)
    -   用途: 查询电源状态，供 Rust 巡检分档。
    -   逻辑: 查询 `PowerManager.isInteractive` (屏幕是否亮屏) 和 `isWifiConnected()` (是否 WiFi 连接)。
    -   策略: 不注册 SCREEN_ON/OFF 广播，依靠巡检唤醒周期动态判断。WiFi+亮屏走基础间隔 (60s)，蜂窝或灭屏走闲时间隔 (默认 5min)。

8.  **`isWifiConnected()`** (`MonitorServicePlugin.kt:127-135`)
    -   用途: 私有方法，实时查询当前活动网络是否为 WiFi。
    -   逻辑: 通过 `ConnectivityManager` 获取 `activeNetwork` 并检查 `TRANSPORT_WIFI`。异常时返回 false。

9.  **`getBatteryOptimizationInfo(invoke: Invoke)`** (`MonitorServicePlugin.kt:165-176`)
    -   用途: 获取电池优化白名单状态及厂商支持信息。
    -   逻辑: 查询 `PowerManager.isIgnoringBatteryOptimizations`。获取 `Build.BRAND` 和 `Build.MANUFACTURER`。检查 `vendorTargets` 映射表判断是否有专用跳转页（含魅族特殊处理）。返回 `ignoring`, `brand`, `manufacturer`, `hasVendorTarget`。

10. **`requestIgnoreBatteryOptimizations(invoke: Invoke)`** (`MonitorServicePlugin.kt:184-194`)
    -   用途: 申请加入电池优化白名单。
    -   逻辑: 构建 `ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` Intent，携带包名 URI。启动该 Intent 会弹出系统确认框。**注意**: 不做静默加入，遵循应用商店审核要求，前端需先展示用户确认 UI。

11. **`openVendorBatterySettings(invoke: Invoke)`** (`MonitorServicePlugin.kt:202-256`)
    -   用途: 跳转至厂商特定的自启/省电管理页面。
    -   逻辑: 降级链实现：
        1.  **厂商专用页**: 根据 `Build.BRAND` 从 `vendorTargets` 获取候选 `ComponentName` 列表，逐个尝试 `startActivity`。成功则返回 `path: "vendor"`, `target`, `tried` 列表。
        2.  **魅族特殊处理**: 若品牌为 meizu，尝试 `com.meizu.safe.security.SHOW_APPSEC` Action。成功返回 `path: "vendor_action"`。
        3.  **应用详情页**: 尝试 `ACTION_APPLICATION_DETAILS_SETTINGS`。成功返回 `path: "app_details"`。
        4.  **通用设置**: 尝试 `ACTION_SETTINGS`。成功返回 `path: "settings"`，否则 `path: "none"`。
    -   策略: 不使用 `resolveActivity` 预探测，因为 Android 11+ 包可见性限制会导致误判。直接启动捕获 `ActivityNotFoundException`。

12. **`openNotificationSettings(invoke: Invoke)`** (`MonitorServicePlugin.kt:264-293`)
    -   用途: 跳转至应用通知设置页。
    -   场景: 应对 Android 13 以下无弹框、13+ 永久拒绝通知、国产 ROM 关闭通知等场景。
    -   逻辑: 降级链：
        1.  API 26+: `ACTION_APP_NOTIFICATION_SETTING` + `EXTRA_APP_PACKAGE`。
        2.  API 21-25: 裸 Action `android.settings.APP_NOTIFICATION_SETTING` + `app_package`/`app_uid` Extra。
        3.  应用详情页: `ACTION_APPLICATION_DETAILS_SETTINGS`。
        4.  通用设置: `ACTION_SETTINGS`。
    -   返回: `opened: boolean`。

13. **`setBootAutostart(invoke: Invoke)`** (`MonitorServicePlugin.kt:299-313`)
    -   用途: 设置开机自启接收者的启用状态。
    -   逻辑: 修改 `BootReceiver` 的组件启用状态 (`COMPONENT_ENABLED_STATE_ENABLED/DISABLED`)。同时在 `campus_settings` SharedPreferences 中持久化 `boot_autostart` 布尔值。

14. **`isBootAutostartEnabled(invoke: Invoke)`** (`MonitorServicePlugin.kt:315-323`)
    -   用途: 查询开机自启接收者是否已启用。
    -   逻辑: 读取 `BootReceiver` 的组件启用状态，判断是否等于 `COMPONENT_ENABLED_STATE_ENABLED`。

### BootReceiver
**文件**: `android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt:328-341`
继承自 `BroadcastReceiver`，处理开机广播。

**方法**:
-   **`onReceive(context: Context, intent: Intent)`** (`MonitorServicePlugin.kt:330-341`)
    -   过滤 `Intent.ACTION_BOOT_COMPLETED`。
    -   检查 SharedPreferences `campus_settings` 中的 `boot_autostart` 标志。若未开启则直接返回。
    -   启动前台服务 `ForegroundService`，附带文本 "开机自启:监控运行中"。
    -   Best-effort 尝试拉起 `MainActivity`。若被 ROM 拦截（常见于 Android 10+ MIUI 等），仅捕获异常忽略，不影响前台服务的启动。

### vendorTargets
**文件**: `android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt:143-163`
私有属性，Map<String, List<ComponentName>>。存储各厂商自启/省电页面的候选组件。
-   **覆盖厂商**: xiaomi, redmi, huawei, honor, oppo, realme, oneplus, vivo, iqoo, samsung。
-   **特点**: 每品牌提供多个候选，应对 ROM 版本间的类名漂移。

## Architecture

开机自启与保活架构采用“服务优先，界面兜底”的设计，以应对 Android 严格的后台限制。

```text
┌─────────────────────────────────────────────────────────────────────┐
│                        Android System / ROM                         │
│  (BOOT_COMPLETED, Background Activity Restrictions, Battery Mgr)    │
└──────────────┬──────────────────────────────────────────────────────┘
               ▼
┌──────────────────────────┐     ┌──────────────────────────────────┐
│      BootReceiver        │     │      MonitorServicePlugin        │
│ (MonitorServicePlugin.kt │     │      (MonitorServicePlugin.kt    │
│  :328-341)               │     │   :26-341)                       │
│                          │     │                                  │
│ 1. Check SP flag         │     │ Exposes Tauri Commands:          │
│ 2. Start FGS (Always)    │     │ - startMonitor / stopMonitor     │
│ 3. startActivity (Best-  │     │ - updateNotification             │
│    effort, often blocked│     │ - installApk                     │
│    by ROM)               │     │ - getPowerState                  │
└──────────┬───────────────┘     │ - openVendorBatterySettings      │
           │                     │ - setBootAutostart               │
           │                     └──────────────────────────────────┘
           ▼
┌──────────────────────────┐
│   ForegroundService      │
│ (Rust Loop + Locks)      │
│                          │
│ - Holds Notification     │
│ - Holds WifiLock/WakeLock│
│ - Drives Network Probes  │
└──────────────────────────┘
```

**数据流与交互**:
1.  **开机流程**: 系统发出 `BOOT_COMPLETED` → `BootReceiver` 截获 → 检查用户授权 (`boot_autostart`) → 启动 `ForegroundService` → 尝试启动 `MainActivity` (可能失败)。
2.  **保活流程**: `ForegroundService` 运行 Rust 监控循环 → 通过 `beginProbeWindow`/`endProbeWindow` 临时持有锁 → 通过 `getPowerState` 获取环境状态调整频率。
3.  **用户引导流程**: 前端检测到进程被杀或权限缺失 → 调用 `openVendorBatterySettings` 或 `requestIgnoreBatteryOptimizations` → 引导用户至系统设置页手动授权。
4.  **恢复流程**: 用户手动点击 App 图标 → `MainActivity` 启动 → Tauri Activity `setup` 执行 `run_startup_tasks` → 恢复完整监控与登录链路。

## Data Flow

**开机自启决策流**:
1.  `BootReceiver.onReceive` 触发。
2.  读取 `SharedPreferences("campus_settings")` 键 `boot_autostart`。
    -   `false`: 退出，不执行任何操作。
    -   `true`: 继续。
3.  构建 `ForegroundService` Intent，Action 为空，Extra `EXTRA_TEXT="开机自启:监控运行中"`。
4.  调用 `startForegroundService` (SDK O+) 或 `startService`。此步骤通常不会被 ROM 阻止，因为前台服务有通知支撑。
5.  构建 `MainActivity` Intent，设置 `FLAG_ACTIVITY_NEW_TASK`。
6.  调用 `startActivity`。
    -   **成功**: 界面拉起，Rust 循环由 Activity 生命周期驱动。
    -   **失败 (Exception)**: 捕获并忽略。前台服务仍在运行，保持进程存活和通知显示。用户后续手动打开 App 即可恢复。

**厂商设置页跳转流**:
1.  前端调用 `openVendorBatterySettings`。
2.  获取 `Build.BRAND`。
3.  遍历 `vendorTargets[brand]` 列表。
4.  对每个 `ComponentName`，构建带 `package_name` Extra 的 Intent。
5.  调用 `startActivity`。
    -   **成功**: 返回 `path="vendor"`, `target=组件名`, `tried=已尝试列表`。
    -   **失败**: 记录到 `tried` 列表，尝试下一个。
6.  若厂商列表耗尽且品牌为 "meizu"，尝试特殊 Action `com.meizu.safe.security.SHOW_APPSEC`。
7.  若仍失败，降级至 `ACTION_APPLICATION_DETAILS_SETTINGS`。
8.  最后降级至 `ACTION_SETTINGS`。
9.  返回最终落点 `path` 及 `tried` 历史，供前端提示用户。

## Connections

-   [[android-persistent-notification-standard]]: 前台服务通知的标准构建与更新协议。
-   [[rust-monitor-loop]]: `monitor_loop.rs` 中描述的 Rust 端监控循环，依赖 Android 端提供的电源状态和探针窗口锁。
-   [[tauri-android-activity-lifecycle]]: Tauri Android Activity 的生命周期管理，特别是 `setup` 和 `run_startup_tasks` 在应用首次手动启动时的作用。

## Known Issues

1.  **开机自启无法强制拉起界面**: Android 10+ 及主流 ROM 严格限制后台启动 Activity。`BootReceiver` 中的 `startActivity` 是被拦常态，非 Bug。设计预期是“前台服务保活 + 用户手动点开恢复”。
2.  **厂商页面类名漂移**: 尽管提供了多组候选组件，但不同 ROM 版本甚至同一 ROM 的不同小版本可能导致类名变化。降级链确保了最终能到达某个设置页，但不一定是最精准的“自启管理页”。
3.  **电池优化白名单审核风险**: `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` 需要用户显式确认。应用商店可能对直接请求此权限进行审查，因此代码层保持了“用户确认前置”的原则，前端必须先展示说明 UI。