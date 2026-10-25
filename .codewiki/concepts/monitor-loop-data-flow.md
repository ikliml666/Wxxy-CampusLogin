---
title: monitor-loop-data-flow
type: concept
source_files:
  - android/frontend/src/monitor/useMonitor.ts
  - android/frontend/src/monitor/index.ts
  - android/frontend/src/hooks/useQualityStore.ts
  - android/frontend/src/hooks/tauriApi.ts
  - android/src-tauri/src/lib.rs
tags:
  - monitoring
  - data-flow
  - android-frontend
  - backend-rust
---

## Overview

`monitor-loop-data-flow` 描述 Android 客户端网络监控功能的完整控制与数据闭环：从前端 `useMonitor` Hook 接收用户操作，经由 Zustand store 与 `tauriApi` IPC 层调用 Tauri 命令，到后端 Rust（`lib.rs` 注册、`monitor_loop` / `quality_cmds` 模块）执行后台巡检与延迟测试循环，再通过事件监听或状态拉取将质量数据回传前端 store 驱动 UI 更新。当前实现强化了配置先落盘再启停、重入锁与卸载守卫、权限请求幂等、质量结果节流等机制，修复了多处历史竞态与状态丢失缺陷。

## Key Components

### android/frontend/src/monitor/useMonitor.ts

- `useMonitor()` 函数，行 12-123：聚合 `authStore`（行 13-16）、`qualityStore`（行 17-22）、`configStore`（行 23-26）的浅订阅，返回 store 字段与三个回调 `handleToggleBackgroundCheck`、`handleTriggerCheck`、`handleToggleLatencyTest`。
- `mountedRef` 行 31-35：卸载守卫，`useEffect` 内设置 `true`，清理时设 `false`，用于阻止卸载后 `setState`。
- `bgToggleBusyRef` 行 36、`latencyToggleBusyRef` 行 37：重入锁（`useRef(false)`），防止快速连点并发 start/stop。
- `handleToggleBackgroundCheck(enabled, intervalSec)` 行 39-81：先 `store.saveConfigDirect` 持久化 `enableBackgroundCheck` 与 `backgroundCheckInterval`（毫秒，行 45-48），再依据 `mountedRef` 检查（行 49）后，若 `enabled` 则 `requestNotificationPermission()`（行 53）及在名称检查开启时顺带 SSID 权限 `api.requestWifiSsidPermission?.()`（行 57-63），然后 `api.startBackgroundCheck?.()`（行 64）或 `api.stopBackgroundCheck?.()`（行 66），成功后 `store.updateConfigLocal`（行 69）+ `store.setBgStatus`（行 70）；失败走 `useLogToastStore.addToast`（行 74-76）。
- `handleTriggerCheck()` 行 83-85：调用 `api.triggerBackgroundCheck?.()`。
- `handleToggleLatencyTest(enabled, intervalSec)` 行 87-115：先 `store.saveConfigDirect` 落盘 `enableLatencyTest` 与 `latencyTestInterval`（行 93-96），依据 `mountedRef` 检查（行 97）后调 `api.startLatencyTest?.()`（行 99）或 `api.stopLatencyTest?.()`（行 101），本地更新 `updateConfigLocal`（行 104）。

### android/frontend/src/monitor/index.ts

- 导出列表行 1-8：`MonitorPanel`、`QualityPanel`、`SpeedTestPanel`、`LatencyPair`、`LatencyTimeline`、`NetworkQualityCapsule`、`StatusBar`、`useMonitor`；行 10 `export * from './types'` 通配导出类型。

### android/frontend/src/hooks/useQualityStore.ts

- `_qualityLockFlag` 行 11：模块级 `boolean` 刷新锁标志。
- `lastQualityResultTime` 行 16：模块级 `number` 类型，最近质量结果到达时间戳（0=从未），用于登录后节流（P2-F5）。
- `getLastQualityResultTime()` 行 18-20：返回上述时间戳。
- `QualityStore` 接口 行 22-41，字段：
  - `networkQuality: NetworkQuality | null`
  - `dnsDohStatus: DnsDohStatus | null`
  - `dnsChecking: boolean`
  - `isRefreshingQuality: boolean`
  - `updateAvailable: boolean`
  - `latestVersion: string`
  - `releaseNotes: string`
  - `updatePromptOpen: boolean`（收到 update-available 时置 true，用户关闭/前往后置 false）
  - `refreshRate: number`
  - 方法 `refreshQuality: () => Promise<void>`
  - `setNetworkQuality: (q: NetworkQuality | null | ((prev: NetworkQuality | null) => NetworkQuality | null)) => void`
  - `setDnsDohStatus: (s: DnsDohStatus | null) => void`
  - `setDnsChecking: (v: boolean) => void`
  - `setUpdateAvailable: (v: boolean) => void`
  - `setLatestVersion: (v: string) => void`
  - `setReleaseNotes: (v: string) => void`
  - `setUpdatePromptOpen: (v: boolean) => void`
- `useQualityStore` 创建 行 43-89：初始状态全默认（行 44-52）；`refreshQuality` 行 54-74 加锁（行 56-58）、读 `config.enableNetworkQuality`（行 57），调 `api.checkNetworkQuality?.()`（行 61），经 `setNetworkQuality` 合并（行 64），500ms 后解锁（行 69-72）；`setNetworkQuality` 行 78-82 函数式更新并写 `lastQualityResultTime = Date.now()`（行 80）；其余 setter 行 83-88。

### android/frontend/src/hooks/tauriApi.ts

- `desktopOnly<T>(name)` 行 14-15：安卓端拒绝桌面专属能力，统一 `Promise.reject`。
- `noopListener` 行 18：空监听返回 no-op 清理函数。
- `ConnectionCampusStatus` 行 20-24（接口）、`BatteryOptimizationInfo` 行 26-30（接口）、`VendorSettingsResult` 行 32-36（接口）、`CampusStatusResult` 行 38-46（接口）、`AvoidBadWifiStatus` 行 155-164（接口）、`RestoreWrittenSettingsResult` 行 166-169（接口）等类型定义。
- `TauriApi` 接口 行 48-153，监控相关成员：
  - `requestWifiSsidPermission: () => Promise<unknown>` 行 61
  - `onBackgroundCheckResult: (cb) => () => void` 行 86
  - `onAutoLoginResult: (cb) => () => void` 行 87
  - `startBackgroundCheck: () => Promise<CommandResult>` 行 101
  - `stopBackgroundCheck: () => Promise<CommandResult>` 行 102
  - `triggerBackgroundCheck: () => Promise<CommandResult>` 行 103
  - `getBackgroundStatus: () => Promise<BackgroundStatus>` 行 104
  - `checkNetworkQuality: () => Promise<NetworkQuality>` 行 108
  - `onNetworkQualityResult: (cb) => () => void` 行 109
  - `startLatencyTest: () => Promise<CommandResult>` 行 110
  - `stopLatencyTest: () => Promise<CommandResult>` 行 111
- `createEventListener<T>` 行 171-200：通用 Tauri 事件封装，支持取消与未完成监听清理。
- `tauriApi` 对象 行 202-342：实现上述接口；监控命令实际 `invoke` 映射：
  - `requestWifiSsidPermission` → `invoke('request_wifi_ssid_permission')` 行 214
  - `onBackgroundCheckResult` → `createEventListener('background-check-result')` 行 270
  - `onAutoLoginResult` → `createEventListener('auto-login-result')` 行 271
  - `startBackgroundCheck` → `invoke('start_background_check')` 行 283
  - `stopBackgroundCheck` → `invoke('stop_background_check')` 行 284
  - `triggerBackgroundCheck` → `invoke('trigger_background_check')` 行 285
  - `getBackgroundStatus` → `invoke('get_background_status')` 行 286
  - `checkNetworkQuality` → `invoke('check_network_quality')` 行 290
  - `onNetworkQualityResult` → `createEventListener('network-quality-result')` 行 291
  - `startLatencyTest` → `invoke('start_latency_test')` 行 292
  - `stopLatencyTest` → `invoke('stop_latency_test')` 行 293
- `tauriApiWithRetry` 行 374-380：仅 `saveConfig` 包 `withRetry`（行 379），其余直连；注释说明 `checkNetworkQuality` 有后端 loop 兜底故不重试（行 377-378）。

### android/src-tauri/src/lib.rs

- `run()` 行 33-140：构建 Tauri 应用，移动端挂载 `campus_monitor_service` 等插件（行 39），`setup` 中调 `monitor_loop::run_startup_tasks(app.handle().clone())` 行 60 拉起持久化配置对应的循环与启动自动登录；`invoke_handler` 注册命令 行 68-130：
  - `campus_detect::request_wifi_ssid_permission` 行 78
  - `monitor_loop::start_background_check` 行 109
  - `monitor_loop::stop_background_check` 行 110
  - `monitor_loop::trigger_background_check` 行 111
  - `monitor_loop::get_background_status` 行 112
  - `quality_cmds::check_network_quality` 行 121
  - `quality_cmds::start_latency_test` 行 122
  - `quality_cmds::stop_latency_test` 行 123
- 退出护栏：`RunEvent::ExitRequested` 阻止默认退出（行 137-139），保证前台服务常驻。

## Architecture

```text
┌─────────────────────┐   ┌────────────────────┐   ┌────────────────────┐
│ useMonitor (Hook)   │   │ useQualityStore    │   │ tauriApi (IPC)     │
│ useMonitor.ts:12    │-->│ useQualityStore.ts │-->│ tauriApi.ts:48     │
└─────────────────────┘   │ :43               │   └─────────┬──────────┘
                          └────────────────────┘             │ invoke
                                                             ▼
┌─────────────────────┐   ┌────────────────────┐   ┌────────────────────┐
│ lib.rs cmd register │<--│ monitor_loop (Rust)│<--│ frontend listeners │
│ lib.rs:68-130       │   │ run_startup_tasks  │   │ onNetworkQuality…  │
└─────────────────────┘   │ :60                │   │ tauriApi.ts:109    │
                          └────────────────────┘   └────────────────────┘
```

## Data Flow

1. **初始化与订阅**：`useMonitor` 挂载时（行 12-35）通过 `useQualityStore` 浅订阅 `networkQuality`、`isRefreshingQuality` 等；`mountedRef` 防卸载后写状态。后端 `lib.rs` `setup` 调 `run_startup_tasks`（行 60）按持久化配置自动拉起循环。

2. **控制指令下发**：用户操作 → `handleToggleBackgroundCheck`（行 39）或 `handleToggleLatencyTest`（行 87）。两者均先 `store.saveConfigDirect`（行 45 / 93）落盘开关与间隔（毫秒），再依据 `mountedRef` 检查（行 49 / 97）后调 `api.startBackgroundCheck?.()`（行 64）或 `api.startLatencyTest?.()`（行 99）。`handleTriggerCheck`（行 83）直接调 `triggerBackgroundCheck`。

3. **IPC 与后端执行**：`tauriApi` 将调用映射为 `invoke('start_background_check')` 等（行 283-293），`lib.rs` 行 109-112 路由到 `monitor_loop`、行 121-123 路由到 `quality_cmds` 模块维持 Tokio 循环，执行探针与延迟测试；`checkNetworkQuality` 可由 `refreshQuality`（行 61）手动触发或后端 loop 事件流推送。

4. **数据回传与渲染**：后端通过 `network-quality-result` 事件（行 109, 291）或 `getBackgroundStatus` 回传；`useQualityStore.setNetworkQuality`（行 78-82）更新并置 `lastQualityResultTime`（行 80），订阅组件重渲染。`refreshQuality` 内置 `_qualityLockFlag` 与 500ms 解锁（行 56-72）避免高频重复探测（P2-F5 节流）。失败路径：`useMonitor` 捕获异常经 `extractErrorMessage` 转 `addToast`（行 74-76），DEV 环境 `console.error`。

## Connections

- 后端循环模块详见 [[monitor-loop-rust]]
- 配置加密落盘与启动恢复见 [[config-state]]
- 质量探测命令见 [[quality-cmds]]
- 前端 IPC 封装层见 [[tauri-ipc-android]]
- 前端监控 UI 组件集合见 [[android-frontend-monitor]]
- 账户与生物验证门见 [[identity-gate]]

## Known Issues

- **历史缺陷：先 start 后存配置导致首次用旧间隔** —— 已在 `useMonitor.ts` 行 43-44 注释说明，改为先 `saveConfigDirect` 再启动。
- **历史缺陷：延迟测试开关仅前端本地更新，重启丢失** —— 行 91-92 改为先 `saveConfigDirect` 持久化 `enableLatencyTest`。
- **历史缺陷 P2-F5：登录后手动质量探测与循环重复** —— `useQualityStore.ts` 行 13-16、76-77 引入 `lastQualityResultTime` 节流，任意来源结果均刷新时间戳（实现见行 78-82）。
- **快速连点竞态** —— `bgToggleBusyRef` / `latencyToggleBusyRef` 重入锁（`useMonitor.ts` 行 36-37, 40, 88）拦截并发。
- **卸载后 setState** —— `mountedRef` 守卫（`useMonitor.ts` 行 31-35, 49, 54, 68, 73）阻止卸载后状态写。
- **Android 13+ 通知权限** —— 启动后台检查前 `requestNotificationPermission()`（行 53）仅弹框不跳设置页。
- **存量用户 SSID 权限未请求** —— 名称检查已开时顺带 `requestWifiSsidPermission?.()`（行 57-63），幂等捕获异常。
- **桌面专属能力在安卓不可用** —— `desktopOnly` 拒绝适配器/DHCP/DNS/窗口等（`tauriApi.ts` 行 14-15, 205-208, 268-269, 287-289, 311, 320, 333-337），相关事件 `noopListener`（行 18, 272-274, 315-318）。
- **重试策略** —— `tauriApiWithRetry` 仅 `saveConfig` 重试（`tauriApi.ts` 行 374-380 中行 379 实现），`checkNetworkQuality` 因后端 loop 兜底不重试以免放大流量。
