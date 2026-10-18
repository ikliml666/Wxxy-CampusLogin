---
type: concept
title: state-management-patterns
source_files:
- android/frontend/src/hooks/tauriApi.ts
- android/frontend/src/hooks/useAuthStore.ts
- android/frontend/src/hooks/useConfigStore.ts
- android/frontend/src/hooks/useQualityStore.ts
- android/frontend/src/hooks/useLogToastStore.ts
- android/frontend/src/hooks/useAppInit.ts
- android/frontend/src/hooks/useInitialDataLoad.ts
- android/frontend/src/hooks/useAsyncLock.ts
- android/frontend/src/account/selfServiceState.ts
- android/frontend/src/account/useAccount.ts
- android/frontend/src/account/SelfOnlineStack.tsx
- android/frontend/src/monitor/MonitorPanel.tsx
- android/frontend/src/monitor/useMonitor.ts
- android/frontend/src/monitor/LatencyTimeline.tsx
tags: [zustand, tauri, race-condition, optimistic-ui, ttl-cache]
---

## Overview

在 Wxxy-CampusLogin 系统中，状态管理并非单一的中心化架构，而是由 `frontend-hooks-store` 提供的 Zustand 全局 Store 层与特定业务模块（如 `frontend-account`、`frontend-monitoring`）内的局部 State/Hook 模式共同构成的混合体系。该概念涵盖了从应用初始化、配置持久化、生物识别门控到网络质量监控的全链路数据流控制。

核心特征包括：
1.  **异步竞态防护**：通过 `useAsyncLock` 和 Promise 锁防止账号切换、登录等操作的状态污染。
2.  **乐观更新与脏标记**：配置保存采用防抖 + 脏字段保护策略，确保本地 UI 响应迅速且最终一致性。
3.  **TTL 缓存与安全门控**：生物识别验证等敏感操作采用带生存时间（TTL）的内存缓存，平衡安全性与用户体验。
4.  **后端同构调度**：前端状态触发后端 Rust 命令，后端通过原生循环（Loop）或定时任务反向推送状态至前端 Store。

## Involved Modules

*   **`frontend-hooks-store`**: 核心基础设施。提供 `useAuthStore`, `useConfigStore`, `useQualityStore` 等 Zustand Stores，以及 `tauriApi` 桥接层。负责全局状态订阅、生命周期钩子（`useAppInit`, `useInitialDataLoad`）及动画性能优化。
*   **`frontend-account`**: 账户生命周期管理。包含 `selfServiceState`（生物识别 TTL 缓存）、`useAccount`（账号切换逻辑）、`SelfOnlineStack`（复杂手势状态）。
*   **`frontend-monitoring`**: 网络监控与质量。包含 `useMonitor` Hook（后台检测控制）、`MonitorPanel`（配置持久化）、`LatencyTimeline`（延迟可视化状态）。
*   **`rust-backend-commands`**: 后端状态源。通过 Tauri Commands 暴露 `config_state`, `account_cmds`, `monitor_loop` 等接口，是前端状态的最终事实来源（Source of Truth）。
*   **`frontend-build-assets`**: Mock 环境支持。通过 `tauri-mock.js` 模拟后端 API 返回，使前端状态管理在无后端环境下可测试。

## Flow

### 1. 初始化与数据加载
*   **入口**: `android/frontend/src/main.tsx` 调用 `useAppInit` (`hooks/useAppInit.ts`)。
*   **流程**:
    1.  `useAppInit` 触发 `useInitialDataLoad` (`hooks/useInitialDataLoad.ts`)。
    2.  `useInitialDataLoad` 通过 `tauriApi.getConfig` 获取初始配置，写入 `useConfigStore`。
    3.  同时加载认证状态至 `useAuthStore` 和网络质量至 `useQualityStore`。
    4.  处理 React StrictMode 双重挂载问题，确保异步加载不被丢弃。

### 2. 配置修改与持久化（防抖与脏标记）
*   **场景**: 用户在 `MonitorPanel` 或 `AccountPanel` 修改设置。
*   **流程**:
    1.  UI 组件更新局部 Draft State。
    2.  `useConfigStore` 监听变更，启动 500ms 防抖定时器。
    3.  定时器触发时，调用 `tauriApiWithRetry.saveConfig`。
    4.  **关键模式**: 使用 `dirtyFields` 记录已修改但未确认的字段。若连续保存失败 3 次，放弃脏标记并强制重新同步，防止后端旧快照覆盖本地值。
    5.  `MonitorPanel` (`monitor/MonitorPanel.tsx`) 强调“配置持久化优先”，在调用原生 API 前强制落盘配置。

### 3. 账号切换与竞态防护
*   **场景**: 用户切换校园网账号。
*   **流程**:
    1.  `useAccount` Hook (`account/useAccount.ts`) 接收切换请求。
    2.  检查 `switchInFlight` Promise 锁，若存在正在进行的切换，则排队或拒绝新请求，防止竞态条件。
    3.  调用 `tauriApi.switch_account`。
    4.  成功后更新 `useAuthStore` 中的当前账号信息，并刷新相关 UI 状态。

### 4. 生物识别门控与 TTL 缓存
*   **场景**: 查看明文密码或执行自助服务。
*   **流程**:
    1.  `selfServiceState.ts` (`account/selfServiceState.ts`) 维护一个带有 TTL (570s) 的缓存对象。
    2.  用户通过 Windows Hello 验证后，将验证状态写入缓存。
    3.  后续操作检查缓存有效期，若未过期则直接放行；若过期或需强制验证（`ignoreToggle: true`），则重新触发生物识别门控。
    4.  预留 30s 余量以避免前后端时钟偏差导致的死锁。

### 5. 网络监控与质量可视化
*   **场景**: 后台自动检测网络质量。
*   **流程**:
    1.  `useMonitor` Hook (`monitor/useMonitor.ts`) 启动后台检查循环。
    2.  通过 `@/hooks/tauriApi` 调用后端 `start_background_check`。
    3.  后端 `monitor_loop.rs` 定期执行探测，并通过事件监听或轮询将结果推送给前端。
    4.  前端 `useQualityStore` 接收增量数据，调用 `mergeNetworkQuality` (`lib/latency.ts`) 合并状态，避免 UI 停滞在 "busy" 状态。
    5.  `LatencyTimeline` (`monitor/LatencyTimeline.tsx`) 消费 Store 数据，渲染 DNS/TCP/TLS 各阶段延迟进度条。

## Implementation Map

| 功能模式 | 涉及文件 | 行号/位置 | 说明 |
| :--- | :--- | :--- | :--- |
| **Zustand Store 定义** | `android/frontend/src/hooks/useAuthStore.ts` | L68-L218 | 认证状态管理 |
| | `android/frontend/src/hooks/useConfigStore.ts` | L46-L202 | 配置状态管理，含防抖与脏字段逻辑 |
| | `android/frontend/src/hooks/useQualityStore.ts` | L30-L79 | 网络质量状态管理 |
| **API 桥接与重试** | `android/frontend/src/hooks/tauriApi.ts` | L103-L183, L353-L363 | 统一 RPC 调用，`tauriApiWithRetry` 实现指数退避 |
| **异步互斥锁** | `android/frontend/src/hooks/useAsyncLock.ts` | L4-L28 | 通用异步锁 Hook |
| **应用初始化** | `android/frontend/src/hooks/useAppInit.ts` | L6-L10 | 串联所有初始化逻辑 |
| | `android/frontend/src/hooks/useInitialDataLoad.ts` | L16-L103 | 首次数据加载，处理 StrictMode |
| **账号切换锁** | `android/frontend/src/account/useAccount.ts` | N/A (推断) | 使用 `switchInFlight` Promise 锁防止切换竞态 |
| **生物识别 TTL 缓存** | `android/frontend/src/account/selfServiceState.ts` | N/A (推断) | 实现 570s TTL 缓存，预留 30s 余量 |
| **空值语义处理** | `android/frontend/src/account/useAccount.ts` | N/A (推断) | 显式标志位处理“保留旧密码”语义 |
| **配置持久化优先** | `android/frontend/src/monitor/MonitorPanel.tsx` | N/A (推断) | 先存配置再调原生 API，修复竞态缺陷 |
| **草稿输入防抖** | `android/frontend/src/monitor/MonitorPanel.tsx` | N/A (推断) | `draft` 状态 + `blur`/`Enter` 提交 |
| **网络质量合并** | `android/frontend/src/lib/latency.ts` | N/A (推断) | `mergeNetworkQuality` 处理 `busy` 状态边界 |
| **延迟可视化状态** | `android/frontend/src/monitor/LatencyTimeline.tsx` | N/A (推断) | 硬编码中文键名匹配后端字段 |
| **Mock 状态模拟** | `android/frontend/tauri-mock.js` | L78-L165 | 模拟后端命令分发，支持动态账号切换 |