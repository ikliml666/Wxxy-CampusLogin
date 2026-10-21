---
type: concept
title: monitor-loop-data-flow
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

`monitor-loop-data-flow` 描述了 Android 客户端网络监控功能的完整数据与控制流闭环。该概念涵盖了从前端 UI 触发、状态管理、Tauri IPC 通信到后端 Rust 守护进程执行，最终将结果回传至前端可视化的全过程。

核心流程依赖于“控制指令下发”与“状态数据上行”的双向通道。前端通过 Hook 发起启动/停止/触发指令，后端通过 `lib.rs` 注册命令并调用 `monitor_loop` 模块维持后台检测循环（含夜间出站切换逻辑），并将实时质量数据写入共享状态或配置状态，前端 Store 订阅变化并驱动 UI 更新。

## Involved Modules

1.  **android-frontend-monitor**: 监控功能的前端核心模块，负责 UI 组件渲染（胶囊、面板）、用户交互入口及本地状态聚合。
2.  **android-frontend-hooks-and-stores**: 基础设施层，提供 `useQualityStore` 用于存储和订阅网络质量数据，以及 `tauriApi` 用于底层 IPC 调用。
3.  **desktop-rust-core**: Tauri 后端核心模块，包含 `lib.rs` 中的命令注册与路由，以及 `monitor_loop` 模块实现的实际后台监控循环逻辑。

## Flow

数据与控制流的交互遵循以下阶段：

### 1. 初始化与状态订阅 (Initialization & Subscription)
*   **前端**: `useMonitor` Hook (`android/frontend/src/monitor/useMonitor.ts`) 在组件挂载时初始化。它依赖 `useQualityStore` (`android/frontend/src/hooks/useQualityStore.ts`) 来读取当前的网络质量快照。
*   **Store 机制**: `useQualityStore` 通过 `refreshQuality` 方法监听来自后端的更新。该 Store 是连接后端数据与前端的桥梁。

### 2. 控制指令下发 (Control Command Dispatch)
*   **触发**: 用户在 `NetworkQualityCapsule` 或 `MonitorPanel` 中点击“开始检测”、“暂停”或“立即测试”。
*   **封装**: `useMonitor` 捕获事件，通过 `tauriApi` (`android/frontend/src/hooks/tauriApi.ts`) 调用对应的 Tauri 命令。
    *   启动后台循环: `startBackgroundCheck`
    *   停止后台循环: `stopBackgroundCheck`
    *   手动触发单次检测: `triggerBackgroundCheck`
*   **IPC 路由**: 这些调用被路由到后端 `desktop-rust-core` 模块中注册的命令处理器。

### 3. 后端处理与执行 (Backend Processing)
*   **命令接收**: 在 `lib.rs` 中，Tauri 命令接收前端的控制信号并分发给 `monitor_loop` 模块。
*   **循环管理**:
    *   后端维护一个后台任务（通常基于 `tokio` 或类似异步运行时）。
    *   `startBackgroundCheck` 启动一个持续运行的循环，定期执行网络探针（Ping, 延迟测试等）及可能的“夜间出站切换”逻辑（涉及网卡 Metric 修改）。
    *   `stopBackgroundCheck` 发送终止信号，清理资源。
    *   `triggerBackgroundCheck` 强制立即执行一次检测周期。
*   **状态更新**: 检测完成后，后端将最新的质量数据（延迟、丢包率、节点信息等）写入共享状态（如 `AndroidState` 或通过配置广播机制）。

### 4. 数据回传与前端渲染 (Data Return & Rendering)
*   **数据同步**: 后端更新的状态被 `useQualityStore` 感知。这可能通过 Tauri 的事件系统（Event Listener）或配置变更的副作用触发。
*   **UI 响应**:
    *   `useQualityStore` 更新其内部 Zustand 状态。
    *   依赖该 Store 的 UI 组件（如 `LatencyTimeline`, `StatusBar`, `QualityPanel`）自动重渲染。
    *   `LatencyTimeline` 使用归一化算法展示历史延迟曲线；`NetworkQualityCapsule` 显示当前质量等级图标。
*   **异常处理**: 如果后端检测失败或超时，前端通过 `extractErrorMessage` 提取错误信息，并在 `MonitorPanel` 中展示降级状态或错误提示，同时保持 UI 的鲁棒性（非阻断式权限请求策略）。

## Implementation Map

| 环节 | 模块 | 文件 | 行号/位置 | 说明 |
| :--- | :--- | :--- | :--- | :--- |
| **前端入口** | android-frontend-monitor | `src/monitor/useMonitor.ts` | Line 12 | 核心 Hook，协调 UI 状态与后端命令 |
| **前端入口** | android-frontend-monitor | `src/monitor/index.ts` | Line 1-10 | 导出 `useMonitor`, `NetworkQualityCapsule` 等 |
| **UI 组件** | android-frontend-monitor | `src/monitor/NetworkQualityCapsule.tsx` | Line 85-90 | 胶囊视图，触发检测操作的 UI 入口 |
| **UI 组件** | android-frontend-monitor | `src/monitor/MonitorPanel.tsx` | Line 50 | 详细监控面板 |
| **状态存储** | android-frontend-hooks-and-stores | `src/hooks/useQualityStore.ts` | Line 54-73 | `refreshQuality` 方法，更新质量状态 |
| **IPC 调用** | android-frontend-hooks-and-stores | `src/hooks/tauriApi.ts` | Line 190-324 | 封装所有 Tauri 后端调用，包括监控相关命令 |
| **后端注册** | desktop-rust-core | `src/lib.rs` | Line 66-122 | `invoke_handler` 中注册所有 Tauri 命令，包括监控循环命令 |