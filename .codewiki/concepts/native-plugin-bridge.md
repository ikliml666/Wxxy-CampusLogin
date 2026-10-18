---
type: concept
title: native-plugin-bridge
source_files:
  - android/frontend/src/hooks/tauriApi.ts
  - android/frontend/public/mock-tauri.js
  - android/frontend/public/mock-tauri-addon.js
  - android/frontend/tauri-mock.js
tags:
  - Tauri
  - IPC
  - Bridge
  - Mocking
  - Android
---

## Overview

`native-plugin-bridge` 是 Android Tauri 应用中连接前端 JavaScript/TypeScript 运行时与后端 Rust 原生逻辑的核心抽象层。它不仅仅是一个简单的 RPC 调用封装，更是一个包含**类型契约定义**、**错误重试策略**、**平台兼容性降级**以及**开发环境 Mock 模拟**的完整基础设施体系。

该概念的核心价值在于解耦 UI 层与原生实现层，使得前端能够在无真实后端或特定测试场景下独立运行，同时确保在发布版本中能够安全、高效地调用 Android 系统级能力（如 WiFi 绑定、生物识别、后台监控）。

## Involved Modules

1.  **frontend-hooks-store**: 提供 `tauriApi` 对象及其扩展方法（如 `tauriApiWithRetry`），定义了对后端命令的类型化调用接口，并处理了异步状态管理和生命周期事件监听。
2.  **frontend-build-assets**: 提供了开发阶段的 Mock 基础设施，通过 `mock-tauri.js`、`mock-tauri-addon.js` 和 `tauri-mock.js` 分层模拟 Tauri 内部 API，确保前端在无后端时的可用性。
3.  **rust-backend-commands**: 定义了所有可用的原生命令（Commands），如 `switch_account`, `start_background_check`, `check_network_quality` 等，是桥接另一端的实际执行者。
4.  **frontend-core-lib**: 作为应用入口，通过 `main.tsx` 和 `App.tsx` 初始化全局状态，间接依赖桥接层提供的配置加载和数据同步能力。

## Flow

数据与控制流在 `native-plugin-bridge` 中遵循以下路径：

### 1. 启动与初始化 (Initialization)
*   **构建阶段**: `frontend-build-assets` 中的 `index.html` 注入 `VITE_PLATFORM=android`。若为开发模式，加载 `public/mock-tauri.js` 等文件，劫持 `window.__TAURI_INTERNALS__.invoke`。
*   **运行时**: `frontend-hooks-store` 中的 `useAppInit` 触发 `useInitialDataLoad`。
*   **API 获取**: `useInitialDataLoad` 调用 `frontend-hooks-store` 导出的 `tauriApi.getConfig()`。

### 2. 调用分发 (Dispatch)
*   **生产环境**:
    *   `tauriApi` (L103-L183 in `tauriApi.ts`) 构造一个 invoke 请求。
    *   请求被发送至 Tauri 核心，路由至 `rust-backend-commands` 中对应的 `#[tauri::command]` 函数（如 `account_cmds.rs` 中的 `switch_account`）。
    *   Rust 后端执行 JNI 调用或文件系统操作，返回结果。
*   **Mock 环境**:
    *   `tauriApi` 调用被拦截。
    *   `mock-tauri.js` (L130-L146) 分发命令到 `mock-tauri-addon.js` (L12-L38)。
    *   `mock-tauri-addon.js` 根据 URL 参数（如 `?op=login`）覆盖行为，或直接由 `tauri-mock.js` (L78-L165) 提供预设数据结构。

### 3. 数据回传与状态更新 (Response & State Update)
*   **响应处理**: `tauriApi` 接收 JSON 响应。
*   **重试机制**: 若失败，`tauriApiWithRetry` (L353-L363) 介入，执行指数退避重试。
*   **状态同步**: 成功的数据被写入 `frontend-hooks-store` 中的 Zustand Stores（如 `useConfigStore`, `useAuthStore`）。
*   **UI 渲染**: `frontend-core-lib` 中的 `App.tsx` 读取 Store 变化，驱动 `frontend-mobile-components` 进行重绘。

### 4. 异常与降级 (Error & Fallback)
*   **桌面专属降级**: 当 `desktopOnly<T>` (L30-L32) 标记的方法在 Android 上被调用时，直接返回被拒绝的 Promise，防止崩溃。
*   **存储降级**: `safeStorage` (in `frontend-core-lib`) 在 `localStorage` 不可用时回退到内存 Map。

## Implementation Map

### 1. 核心 API 定义与调用 (Production)
*   **文件**: `android/frontend/src/hooks/tauriApi.ts`
*   **行号**: L4-L28 (`interface TauriApi`), L103-L183 (`tauriApi` object)
*   **描述**: 定义了所有后端命令的类型签名，并封装了 `invoke` 调用逻辑。这是前端代码中唯一直接感知“原生接口”的地方。

### 2. 重试与增强逻辑
*   **文件**: `android/frontend/src/hooks/tauriApi.ts`
*   **行号**: L353-L363 (`tauriApiWithRetry`), L338-L351 (`withRetry`)
*   **描述**: 为关键操作（如配置保存）添加网络不稳定时的自动重试能力，确保数据一致性。

### 3. 开发环境 Mock 栈 (Development)
*   **基础 Shim**:
    *   **文件**: `android/frontend/public/mock-tauri.js`
    *   **行号**: L130-L146
    *   **描述**: 挂载 `window.__TAURI_INTERNALS__`，模拟 Tauri 底层结构。
*   **场景增强**:
    *   **文件**: `android/frontend/public/mock-tauri-addon.js`
    *   **行号**: L12-L38
    *   **描述**: 根据 URL 参数动态修改特定命令（如 `check_campus_status`）的返回值，用于测试特定网络状态。
*   **生产级 Mock**:
    *   **文件**: `android/frontend/tauri-mock.js`
    *   **行号**: L78-L165
    *   **描述**: 提供更完整的模拟数据，支持异步延迟、账号切换模拟，并严格遵循后端数据结构约定（如不使用 `ok()` 包裹平铺结构体以防解析错误）。

### 4. 后端命令注册 (Backend)
*   **文件**: `android/src-tauri/src/account_cmds.rs`, `monitor_loop.rs`, `quality_cmds.rs` 等
*   **行号**: 各文件中 `#[tauri::command]` 装饰的函数
*   **描述**: 虽然不直接在前端 JS 中实现，但它们是桥接的目标端。例如 `account_cmds.rs` 中的 `switch_account` 对应前端 `tauriApi.switchAccount`。