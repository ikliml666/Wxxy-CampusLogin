---
type: concept
title: event-and-error-handling
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
1. **非阻断与连续性**：网络/IPC 异常通过重试（`withRetry` L338-354）、串行锁、降级显示维持可用；配置保存失败不上抛静默吞没，而是调用方提示（L81-85, L143-166）。
2. **状态一致性**：使用模块级锁（`_checkOnlineLockFlag` L16, `_qualityLockFlag` L11, `switchInFlight` L12）、脏字段集合（`dirtyFields` L20）与失败计数（`dirtyFailureCounts` L25, `DIRTY_FAILURE_LIMIT` L24）防止后端旧快照覆盖本地未确认数据。
3. **生命周期安全**：`useMonitor` 使用 `mountedRef` L31-35 防止卸载后 setState；事件监听通过 `createEventListener` L159-188 的 cancel 机制清理。
4. **幂等与去重**：Toast 同标题去重（L13, L61）、日志重复计数（L37-41）、质量探测节流（L21, L141, L16-20）。

## Involved Modules

涉及以下模块，各自容错职责：

* **hooks-and-stores（基础设施）**：`tauriApi.ts` 提供 `tauriApiWithRetry`（L356-361）仅对 `saveConfig` 重试；`createEventListener` L159-188 管理监听生命周期；`useAppInit.ts` L1-11 编排启动钩子。`useAuthStore.ts` 处理登录/注销超时包装 `withTimeout` L69-78、在线检测串行锁 L196-243、状态稳定更新 `setStatusStable` L82-86。`useConfigStore.ts` 处理配置防覆盖脏标记 L56-119、失败重置 L151-162、待保存追踪 `hasPendingConfig` L188-193 与 `flushPendingConfig` L195-224。`useQualityStore.ts` 质量刷新锁 L54-74 与结果时间戳 L16-20, L78-82。`useLogToastStore.ts` 日志/Toast 上限与去重 L9-13, L32-82。
* **frontend-account（业务错误处理）**：`useAccount.ts` 账号切换去重 `switchInFlight` L12-128；`selfServiceState.ts` 生物识别门控 `useHelloGate` L65-87、`useSelfServiceVerify` L105-127，TTL 时钟回拨防护 `gateFresh` L58-62，错误码映射 `biometricFailMessage` L34-38。
* **frontend-monitor（运行时监控）**：`useMonitor.ts` 后台检查/延迟测试切换重入锁 `bgToggleBusyRef` L36, `latencyToggleBusyRef` L37，卸载守卫 `mountedRef` L31-35，权限请求降级 L56-63。

模块交互框图：

```text
┌────────────┐   ┌──────────────────────┐
│ useAppInit │──►│ useEventListeners等   │
└────────────┘   └──────────────────────┘
       │
       ▼
┌──────────────┐  retry  ┌─────────────┐ invoke ┌──────────────┐
│ Stores       │────────►│ tauriApiWith│──────►│ Rust Backend  │
│ (Auth/Config/│         │ Retry L356  │       │ (IPC events)  │
│  Quality/Log)│◄────────│ L338-354    │◄──────│               │
└──────┬───────┘  events │ tauriApi.ts │       └──────────────┘
       │          └─────────────┘
       ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│ useAccount   │  │ selfService  │  │ useMonitor   │
│ (switchInFly)│  │ State (Hello)│  │ (busyRef)    │
└──────────────┘  └──────────────┘  └──────────────┘
```

## Flow

### 1. 初始化与全局事件流
* 入口 `useAppInit.ts` L1-11：调用 `useEventListeners()`、`useInitialDataLoad()`、`useHeartbeat()`、`useGlobalShortcut()`。
* 配置加载由 `useConfigStore` 的 `configLoaded` L32, L50 信号控制；初始配置从 `getInitData` 加载（依赖后端）。失败降级由调用方处理。

### 2. IPC 通信与重试流
* `tauriApi.ts` 定义 `tauriApi` 对象 L190-324，其中桌面专属能力通过 `desktopOnly` L14-15 拒绝（如 `exportConfig` L193, `getAdapters` L196, `dhcpRenewAll` L269, `checkDnsDohStatus` L315, `renderHeartbeat` L318 等），空监听 `noopListener` L18 用于 `onAdaptersChanged` L254 等。
* `tauriApiWithRetry` L356-361 仅对 `saveConfig` 包装 `withRetry` L338-354（指数退避，可重试错误判断 `isRetryableError` L326-336）。
* 上层捕获：如 `useConfigStore.saveConfigDirect` L128-176 失败时增加 `dirtyFailureCounts` L152-162，超阈值清除脏标记；`useAuthStore.doLogin` 用 `withTimeout` L69-78 防后端阻塞。

### 3. 状态更新与竞态控制流
* **账号切换去重**：`useAccount.ts` `switchInFlight` L12, L95-127 共享 Promise。
* **在线检测串行**：`useAuthStore.ts` `_checkOnlineLockFlag` L16, L197-242 在 finally 释放（非 setTimeout）。
* **质量刷新串行**：`useQualityStore.ts` `_qualityLockFlag` L11, L54-74 锁持有 500ms L69-72。
* **监控重入锁**：`useMonitor.ts` `bgToggleBusyRef` L36, `latencyToggleBusyRef` L37 防止连点；`mountedRef` L31-35 防卸载后状态更新。
* **配置脏字段**：`useConfigStore` `updateConfig` L56-88 标记 `dirtyFields` L62-65；`mergeConfigFromBackend` L103-119 跳过脏字段；`clearDirtyFields` L121-124。

### 4. 业务异常与用户反馈流
* **生物识别门控**：`selfServiceState.ts` `useHelloGate` L65-87 检查 `gateFresh` L58-62（TTL 570s L54），失败通过 `biometricFailMessage` L34-38 映射错误码；`useSelfServiceVerify` L105-127 面板会话门。
* **Toast/Log 降级**：`useLogToastStore.addToast` L52-82 非前台跳过 L56、同标题去重 L61、上限 `MAX_TOASTS=4` L9, L64-68；`addLog` 重复计数 L37-41。
* **登录/注销错误**：`useAuthStore.doLogin` L106-164 捕获异常 set status error L151，登录成功跳过立即复查 L159-161（防闪烁）；`doLogout` L166-194 类似。
* **权限降级**：`useMonitor.handleToggleBackgroundCheck` L39-81 中 `requestWifiSsidPermission` 失败 catch 仅 warn L58-62。

## Implementation Map

| 功能场景 | 模块 | 文件 | 关键代码/行号 | 说明 |
| :--- | :--- | :--- | :--- | :--- |
| **IPC 重试机制** | hooks-and-stores | `tauriApi.ts` | L338-354, L356-361 | `withRetry` 与 `tauriApiWithRetry` 仅 `saveConfig` 重试。 |
| **桌面能力禁用** | hooks-and-stores | `tauriApi.ts` | L14-15, L193-198, L250-251, L269-271, L296-300, L315-318, L334-336 | `desktopOnly` 拒绝安卓不可用 API；`noopListener` 空监听 L18, L254-256, L297-300。 |
| **全局事件监听生命周期** | hooks-and-stores | `tauriApi.ts` | L159-188 | `createEventListener` cancel 清理 listen。 |
| **配置脏数据冲突** | hooks-and-stores | `useConfigStore.ts` | L20-25, L56-119, L121-124, L151-162 | `dirtyFields`+`dirtyFailureCounts`+`DIRTY_FAILURE_LIMIT` 防覆盖与脱节重置。 |
| **待保存配置追踪** | hooks-and-stores | `useConfigStore.ts` | L14-17, L188-224 | `hasPendingConfig` 检查 pending/in-flight；`flushPendingConfig` 合并保存。 |
| **在线检测串行化** | hooks-and-stores | `useAuthStore.ts` | L16, L196-243 | `_checkOnlineLockFlag` finally 释放，非 setTimeout。 |
| **登录注销超时与防闪烁** | hooks-and-stores | `useAuthStore.ts` | L69-78, L106-164, L166-194 | `withTimeout` 包装；登录成功跳过复查 L159-161。 |
| **状态稳定更新** | hooks-and-stores | `useAuthStore.ts` | L82-86 | `setStatusStable` 引用稳定避免无效重渲染。 |
| **质量刷新串行与节流** | hooks-and-stores | `useQualityStore.ts` | L11, L16-20, L54-82 | `_qualityLockFlag`；`lastQualityResultTime` 节流登录后探测。 |
| **日志/Toast 去重上限** | hooks-and-stores | `useLogToastStore.ts` | L9-13, L32-82, L128-134 | `MAX_TOASTS=4`；同标题去重；日志重复计数。 |
| **账号切换竞态合并** | frontend-account | `useAccount.ts` | L12, L94-128 | `switchInFlight` 共享 Promise；`!== undefined` 处理空串 L114。 |
| **生物识别门控/TTL** | frontend-account | `selfServiceState.ts` | L34-38, L54-62, L65-87, L97-127 | `useHelloGate`/`useSelfServiceVerify`；时钟回拨防护 `gateFresh`。 |
| **监控竞态/卸载保护** | frontend-monitor | `useMonitor.ts` | L31-37, L39-115 | `mountedRef` + `bgToggleBusyRef`/`latencyToggleBusyRef`。 |
| **权限请求降级** | frontend-monitor | `useMonitor.ts` | L56-63 | SSID 权限失败 catch 仅 warn。 |
| **启动编排** | hooks-and-stores | `useAppInit.ts` | L1-11 | 组合初始化钩子。 |

注意：所有行号依据提供源码的「行号| 」前缀重新标注，未沿用旧文章行号。