---
title: event-and-error-handling
type: concept
source_files:
  - android/frontend/src/hooks/useAppInit.ts
  - android/frontend/src/hooks/useAuthStore.ts
  - android/frontend/src/hooks/useConfigStore.ts
  - android/frontend/src/hooks/useQualityStore.ts
  - android/frontend/src/hooks/useLogToastStore.ts
  - android/frontend/src/hooks/tauriApi.ts
  - android/frontend/src/account/useAccount.ts
  - android/frontend/src/account/selfServiceState.ts
  - android/frontend/src/monitor/useMonitor.ts
tags:
  - error-handling
  - event-loop
  - state-management
  - lifecycle
  - retry-strategy
---

## Overview

在 Android 前端（Tauri + React + Zustand）中，`event-and-error-handling` 是横切关注点，覆盖从 IPC 重试、全局事件监听生命周期、Store 并发锁与脏字段冲突解决，到生物识别门控 TTL、UI 错误降级与去重。

核心设计哲学：
1. **非阻断与连续性**：网络/IPC 异常通过重试（`withRetry` 见 `tauriApi.ts` L341-369）、串行锁、降级显示维持可用；配置保存失败上抛由调用方提示（`useConfigStore.ts` L81-85, L143-166）。
2. **状态一致性**：使用模块级锁（`_checkOnlineLockFlag` `useAuthStore.ts` L16, `_qualityLockFlag` `useQualityStore.ts` L11, `switchInFlight` `useAccount.ts` L12）、脏字段集合（`dirtyFields` `useConfigStore.ts` L20）与失败计数（`dirtyFailureCounts` L25, `DIRTY_FAILURE_LIMIT` L24）防止后端旧快照覆盖本地未确认数据。
3. **生命周期安全**：`useMonitor` 使用 `mountedRef`（`useMonitor.ts` L31-35）防止卸载后 setState；事件监听通过 `createEventListener`（`tauriApi.ts` L169-198）的 cancel 机制清理。
4. **幂等与去重**：Toast 同标题去重（`useLogToastStore.ts` L13, L61）、日志重复计数（L37-41）、质量探测节流（`QUALITY_MANUAL_THROTTLE_MS` `useAuthStore.ts` L21, `lastQualityResultTime` `useQualityStore.ts` L16-20）。

## Key Components

### hooks-and-stores（基础设施）

- **`useAppInit.ts` L1-11**：`useAppInit()` 组合 `useEventListeners()`、`useInitialDataLoad()`、`useHeartbeat()`、`useGlobalShortcut()` 四个启动钩子。
- **`tauriApi.ts`**
  - `desktopOnly<T>(name)` L14-15：返回 reject 的 Promise，标记桌面专属功能安卓不可用。
  - `noopListener` L18：返回 no-op 清理函数的空监听。
  - `createEventListener<T>(eventName)` L169-198：封装 Tauri `listen`，支持 cancel 清理未完成的 listen Promise。
  - `tauriApi` 对象 L200-339：定义全部 IPC 调用；桌面专属如 `exportConfig` L203、`getAdapters` L206、`dhcpRenewAll` L284、`checkDnsDohStatus` L330、`renderHeartbeat` L333 经 `desktopOnly` 拒绝；`onAdaptersChanged` L269 等经 `noopListener` 空监听。
  - `isRetryableError(e)` L341-351：按字符串/Error 判断 timeout/network/fetch/connection。
  - `withRetry<T>(fn, maxRetries, baseDelay)` L353-369：指数退避重试。
  - `tauriApiWithRetry` L371-377：展开 `tauriApi`，仅 `saveConfig` 包 `withRetry`。
  - `verifyWindowsIdentity` L217-254：生物识别验证门，支持 2D 人脸回退（`shouldUseFaceFallback`），失败归一化为 `{success:false}`。
- **`useAuthStore.ts`**
  - `_checkOnlineLockFlag` L16：在线检测串行锁。
  - `QUALITY_MANUAL_THROTTLE_MS` L21：登录后手动质量探测节流阈值 60s。
  - `detectCampusNetwork()` L29-35：campus 检测，失败返 null。
  - `buildCampusBgStatusPatch()` L38-49：构造 bgStatus 补丁纯函数。
  - `queryPortalStatus(adapterIp)` L56-65：portal 状态查询，空 IP 后端回退。
  - `withTimeout<T>(promise, ms, timeoutMsg)` L69-78：Promise.race 超时包装，finally 清定时器。
  - `setStatusStable(next)` L82-86：状态引用稳定更新。
  - `AuthStore` 接口 L88-98：`isLoggingIn`, `isLoggingOut`, `status`, `bgStatus`, `doLogin`, `doLogout`, `checkOnline`, `setStatus`, `setBgStatus`。
  - `useAuthStore` L100-247：`doLogin` L106-164（成功跳过复查 L159-161）、`doLogout` L166-194、`checkOnline` L196-243（finally 释放锁 L241）、`setStatus`/`setBgStatus` L245-246。
- **`useConfigStore.ts`**
  - `saveConfigTimer` L14, `saveConfigPending` L15, `saveConfigInFlight` L17, `dirtyFields` L20, `DIRTY_FAILURE_LIMIT` L24, `dirtyFailureCounts` L25。
  - `ConfigStore` 接口 L27-46：`config`, `configLoaded`, `passwordSaved`, `accounts`, `activeAccount`, `language`, `updateConfig`, `updateConfigLocal`, `mergeConfigFromBackend`, `clearDirtyFields`, `syncPasswordSaved`, `saveConfigDirect`, `setAccounts`, `setActiveAccount`, `setLanguage`。
  - `useConfigStore` L48-186：`updateConfig` L56-88（防抖 500ms L77）、`updateConfigLocal` L90-99、`mergeConfigFromBackend` L103-119（跳过脏字段）、`clearDirtyFields` L121-124、`saveConfigDirect` L128-176（失败计数重置 L151-162）、`setLanguage` L181-185。
  - `hasPendingConfig()` L188-193：检查 pending 或 in-flight。
  - `flushPendingConfig()` L195-224：合并 flush 保存。
- **`useQualityStore.ts`**
  - `_qualityLockFlag` L11, `lastQualityResultTime` L16, `getLastQualityResultTime()` L18-20。
  - `QualityStore` 接口 L22-41。
  - `useQualityStore` L43-89：`refreshQuality` L54-74（锁 500ms L69-72）、`setNetworkQuality` L78-82（更新时间戳）。
- **`useLogToastStore.ts`**
  - `toastTimers` L5, `MAX_TOASTS` L9, `isDuplicateTitle` L13。
  - `LogToastStore` 接口 L15-26。
  - `useLogToastStore` L28-135：`addLog` L32-50（重复计数 L37-41）、`addToast` L52-82（非前台跳过 L56、去重 L61、上限 L64-68）、`addToastWithAction` L84-108、`removeToast` L110-114、`removeToastsByPrefix` L116-126、`setLogs` L128、`cleanupToasts` L130-135。

### frontend-account（业务错误处理）

- **`useAccount.ts`**
  - `switchInFlight` L12：切换账号去重 Promise。
  - `useAccount()` L14-166：`refreshAccounts` L30-37、`handleAddAccount` L40-65、`handleDeleteAccount` L67-92、`handleSwitchAccount` L94-128（空串判断 L114）、`handleRenameAccount` L132-157。
- **`selfServiceState.ts`**
  - `helloEnabled()` L10：Hello 总开关。
  - `BIOMETRIC_ERROR_KEY` L19-32：错误码映射。
  - `biometricFailMessage(err, t)` L34-38。
  - `VERIFY_TTL_MS` L54, `gateFresh(verifiedAt)` L58-62（时钟回拨防护）。
  - `helloGateVerifiedAt` L63, `useHelloGate(options)` L65-87。
  - `selfSessionVerifiedAt` L97, `resetSelfSessionGate()` L100-102, `useSelfServiceVerify()` L105-127。

### frontend-monitor（运行时监控）

- **`useMonitor.ts`**
  - `mountedRef` L31-35, `bgToggleBusyRef` L36, `latencyToggleBusyRef` L37。
  - `useMonitor()` L12-123：`handleToggleBackgroundCheck` L39-81（权限降级 L58-63）、`handleTriggerCheck` L83-85、`handleToggleLatencyTest` L87-115。

## Architecture

```text
┌────────────┐   ┌──────────────────────┐
│ useAppInit │──►│ useEventListeners等   │
└────────────┘   └──────────────────────┘
       │
       ▼
┌──────────────┐ retry ┌─────────────┐ invoke ┌──────────────┐
│ Stores       │──────►│tauriApiWith │───────►│ Rust Backend  │
│ (Auth/Config/│       │Retry L371   │       │ (IPC events)  │
│ Quality/Log) │◄──────│ tauriApi.ts │◄───────│               │
└──────┬───────┘ event └─────────────┘       └──────────────┘
       │
       ▼
┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│ useAccount   │ │selfService   │ │ useMonitor   │
│ (switchInFly)│ │State (Hello) │ │ (busyRef)    │
└──────────────┘ └──────────────┘ └──────────────┘
```

## Data Flow

### 1. 初始化与全局事件流
- 入口 `useAppInit.ts` L1-11：组合 `useEventListeners()`、`useInitialDataLoad()`、`useHeartbeat()`、`useGlobalShortcut()`。
- 配置加载由 `useConfigStore` 的 `configLoaded`（`useConfigStore.ts` L32, L50）控制；初始配置从 `getInitData` 加载（依赖后端）。

### 2. IPC 通信与重试流
- `tauriApi.ts` L200-339 定义全部调用；桌面专属经 `desktopOnly` L14-15 拒绝（如 `exportConfig` L203、`getAdapters` L206、`dhcpRenewAll` L284、`checkDnsDohStatus` L330、`renderHeartbeat` L333）；空监听 `noopListener` L18 用于 `onAdaptersChanged` L269 等。
- `tauriApiWithRetry` L371-377 仅对 `saveConfig` 包 `withRetry` L353-369（指数退避，可重试判断 `isRetryableError` L341-351）。
- 上层捕获：`useConfigStore.saveConfigDirect` L128-176 失败增 `dirtyFailureCounts` L152-162 超阈值清脏标记；`useAuthStore.doLogin` 用 `withTimeout` L69-78 防阻塞。

### 3. 状态更新与竞态控制流
- 账号切换去重：`useAccount.ts` `switchInFlight` L12, L95-127 共享 Promise。
- 在线检测串行：`useAuthStore.ts` `_checkOnlineLockFlag` L16, L197-242 finally 释放。
- 质量刷新串行：`useQualityStore.ts` `_qualityLockFlag` L11, L54-74 锁 500ms L69-72。
- 监控重入锁：`useMonitor.ts` `bgToggleBusyRef` L36, `latencyToggleBusyRef` L37；`mountedRef` L31-35 防卸载更新。
- 配置脏字段：`useConfigStore` `updateConfig` L56-88 标记 `dirtyFields` L62-65；`mergeConfigFromBackend` L103-119 跳过；`clearDirtyFields` L121-124。

### 4. 业务异常与用户反馈流
- 生物识别门控：`selfServiceState.ts` `useHelloGate` L65-87 检 `gateFresh` L58-62（TTL 570s L54），失败映射 `biometricFailMessage` L34-38；`useSelfServiceVerify` L105-127 面板会话门。
- Toast/Log 降级：`useLogToastStore.addToast` L52-82 非前台跳过 L56、同标题去重 L61、上限 `MAX_TOASTS=4` L9 L64-68；`addLog` 重复计数 L37-41。
- 登录/注销错误：`useAuthStore.doLogin` L106-164 捕获异常 set error L151，成功跳过复查 L159-161；`doLogout` L166-194 类似。
- 权限降级：`useMonitor.handleToggleBackgroundCheck` L39-81 中 `requestWifiSsidPermission` 失败 catch 仅 warn L58-62。

## Connections

- [[useAppInit]] 启动编排调用各 store 与监听。
- [[useAuthStore]] 依赖 [[useConfigStore]] 配置与 [[useQualityStore]] 质量数据，错误经 [[useLogToastStore]] 反馈。
- [[useConfigStore]] 与 [[tauriApi]] 重试机制直接交互，脏字段逻辑影响 [[useAccount]] 切号回传落地。
- [[useAccount]] 与 [[selfServiceState]] 共享生物识别门控与账号配置。
- [[useMonitor]] 调用 [[useAuthStore]]/[[useConfigStore]]/[[useQualityStore]] 及 [[tauriApi]] 启停服务。

## Known Issues

- 历史缺陷 P2-F5：登录后无条件手动全量质量探测，与后端 latency loop 重复；已通过 `QUALITY_MANUAL_THROTTLE_MS`（`useAuthStore.ts` L21）与 `lastQualityResultTime`（`useQualityStore.ts` L16-20）节流修复。
- 历史缺陷 P2-34：登录/注销 invoke 无超时致按钮永久禁用；已用 `withTimeout`（`useAuthStore.ts` L69-78）修复。
- 旧版 `_checkOnlineLockFlag` 用 setTimeout(500) 释放致并发冗余 invoke；改为 finally 释放（`useAuthStore.ts` L236-242）。
- `dirtyFields` 永久跳过致 UI 脱节；引入 `DIRTY_FAILURE_LIMIT=3` 连续失败重置（`useConfigStore.ts` L24-25, L151-162）。
- `hasPendingConfig` 仅检 pending 漏 in-flight；改为两者皆检（`useConfigStore.ts` L188-193）。
- 登录成功立即 `checkOnline` 致状态闪烁；改为失败才复查（`useAuthStore.ts` L159-161）。
- 删除/切换账号回传空串 activeAccount 被跳过；改 `!== undefined` 判断（`useAccount.ts` L82, L114）。
- Toast 无上限累积；加 `MAX_TOASTS=4` 与同标题去重（`useLogToastStore.ts` L9, L13, L61-68）。
- 前端 Hello 门缓存永真撞后端 TTL 过期；加 `gateFresh` 时钟回拨防护（`selfServiceState.ts` L54-62）。