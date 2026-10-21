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

在 Android 前端应用（基于 Tauri + React）中，`event-and-error-handling` 并非单一模块的职责，而是贯穿基础设施、状态管理、业务逻辑与 UI 渲染的横切关注点。该概念涵盖了从底层 IPC 通信的重试机制、全局事件监听的生命周期管理，到业务层面的竞态条件处理、生物识别门控失败恢复，以及 UI 层的错误吞没与降级展示。

核心设计哲学在于**“防御性编程”**与**“用户体验连续性”**：
1.  **非阻断式体验**：网络抖动或后台服务异常不应导致应用崩溃或强制弹窗，而是通过静默重试、降级显示或本地缓存维持界面可用。
2.  **状态一致性**：通过代际标记（Generation）、串行锁（Serial Lock）和脏数据冲突解决机制，确保异步操作不会破坏全局 Store 的状态一致性。
3.  **生命周期安全**：严格绑定组件挂载/卸载状态，防止内存泄漏及无效的后端调用。

## Involved Modules

该概念涉及以下关键模块，各自承担不同的容错与事件处理职责：

*   **`android-frontend-hooks-and-stores`**: 核心基础设施。提供 `tauriApiWithRetry` 重试机制、`useAppInit` 启动编排中的事件订阅清理、`useAuthStore`/`useConfigStore` 中的脏数据冲突解决与并发去重逻辑。
*   **`android-frontend-account`**: 业务级错误处理。处理账号切换的竞态条件（`switchInFlight`），实施生物识别验证的门控策略（TTL 失效处理），并在弹窗中显式吞没提交错误以支持用户重试。
*   **`android-frontend-monitor`**: 运行时监控与降级。处理网络探测过程中的 `busy` 状态忽略，权限请求失败的静默降级，以及监控循环的代际保护。

## Flow

数据与控制流在错误处理和事件响应中的流动路径如下：

### 1. 初始化与全局事件流 (Bootstrap & Global Events)
*   **入口**：`android-frontend-hooks-and-stores` (`useAppInit.ts`)。
*   **流程**：
    1.  应用启动时，`useAppInit` 订阅全局事件（如 `useEventListeners`）。
    2.  正常流程下，初始化配置加载 (`useInitialDataLoad`) 和心跳保活 (`useHeartbeat`)。若加载失败，`useConfigStore` 尝试使用本地缓存或默认值，而非直接报错阻断。

### 2. IPC 通信与重试流 (IPC & Retry)
*   **入口**：`android-frontend-hooks-and-stores` (`tauriApi.ts`) -> `android-backend-rust` (Rust Backend)。
*   **流程**：
    1.  前端发起 API 调用（如 `listAccounts`, `checkOnline`）。
    2.  若使用 `tauriApiWithRetry`（仅 `saveConfig` 等关键操作），内部实现指数退避重试。
    3.  普通 `tauriApi` 调用若失败，通常由上层 Hook 捕获。例如 `useConfigStore` 检测到保存失败后，增加 `dirtyFailureCounts`，连续失败则放弃同步以防脱节。
    4.  后端 Rust 模块通过原子文件落盘防止配置损坏，前端无感知。

### 3. 状态更新与竞态控制流 (State & Concurrency)
*   **入口**：各业务 Hook (`useAccount.ts`, `useMonitor.ts`, `useAuthStore.ts`)。
*   **流程**：
    1.  **去重/串行化**：`useAccount.ts` 使用 `switchInFlight` Promise 合并重复的账号切换请求；`useAuthStore.ts` 和 `useQualityStore.ts` 使用 `_checkOnlineLockFlag` / `_qualityLockFlag` 串行化在线检测和质量刷新。
    2.  **生命周期检查**：所有异步回调前检查 `mountedRef.current`。若组件已卸载，取消状态更新，防止内存泄漏。

### 4. 业务异常与用户反馈流 (Business Error & Feedback)
*   **入口**：`android-frontend-account` (`selfServiceState.ts`, `useAccount.ts`)。
*   **流程**：
    1.  **生物识别失败**：`useHelloGate` 检测到时钟回拨或验证失败时，重置 TTL 并抛出特定错误码。
    2.  **UI 层处理**：`AccountPanel` 捕获此错误，不直接崩溃，而是提示用户重新验证或降级为密码输入。
    3.  **静默降级**：`fetchDashboard` 自动刷新失败时，静默忽略（不弹 Toast），仅在手动触发失败时通过 `useLogToastStore.addToast` 通知用户。
    4.  **错误吞没**：`DeviceAliasDialog` 提交失败时 `catch` 错误但不关闭弹窗，允许用户修正后重试，保持上下文。

## Implementation Map

以下是具体实现位置及行号参考（基于提供的模块文章）：

| 功能场景 | 模块 | 文件 | 关键代码/行号 | 说明 |
| :--- | :--- | :--- | :--- | :--- |
| **IPC 重试机制** | hooks-and-stores | `tauriApi.ts` | L356-361 | `tauriApiWithRetry` 定义，用于 `saveConfig` 等关键操作的自动重试。 |
| **全局事件清理** | hooks-and-stores | `useAppInit.ts` | (依赖 `useEventListeners`) | 应用初始化时注册全局监听，并在卸载时清理，防止泄漏。 |
| **配置脏数据冲突** | hooks-and-stores | `useConfigStore.ts` | L56-99, L103-110 | `dirtyFields` + `dirtyFailureCounts` 机制，防止后端旧快照覆盖本地未落盘数据。 |
| **在线检测串行化** | hooks-and-stores | `useAuthStore.ts` | L196-243 | `_checkOnlineLockFlag` 防止并发 invoke 导致状态异常。 |
| **质量刷新串行化** | hooks-and-stores | `useQualityStore.ts` | L54-73 | `_qualityLockFlag` 防止并发 refresh 导致状态异常。 |
| **登录成功后延迟检测** | hooks-and-stores | `useAuthStore.ts` | L106-164, L166-194 | `doLogin` 成功后延迟在线检测，避免 Portal 会话传播闪烁。 |
| **账号切换竞态合并** | frontend-account | `useAccount.ts` | L1-126 | `switchInFlight` Promise 机制合并重复请求；`!== undefined` 处理空字符串兼容。 |
| **生物识别门控/TTL** | frontend-account | `selfServiceState.ts` | L65-87, L105-127 | `useHelloGate` 和 `useSelfServiceVerify`；L58-62 时钟回拨防护。 |
| **监控竞态/卸载保护** | frontend-monitor | `useMonitor.ts` | L12 | `mountedRef` 和 `bgToggleBusyRef` 双重锁，防止卸载后内存泄漏及连点冲突。 |
| **权限请求降级** | frontend-monitor | `useMonitor.ts` | L56-63 | WiFi SSID 权限请求包裹在 try-catch 中，失败仅降级显示。 |