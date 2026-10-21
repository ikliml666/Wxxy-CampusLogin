---
type: concept
title: tauri-ipc-command-bridge
source_files:
  - android/frontend/src/hooks/tauriApi.ts
  - android/frontend/public/mock-tauri.js
  - android/frontend/public/mock-tauri-addon.js
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/account_cmds.rs
tags:
  - Tauri
  - IPC
  - Bridge
  - Mock
  - Rust
---

## Overview

`tauri-ipc-command-bridge` 是连接 Android Tauri 前端（TypeScript/React）与后端（Rust/JNI）的核心通信抽象层。它定义了前后端交互的契约，屏蔽了底层 `@tauri-apps/api` 调用细节及 JNI 桥接差异。该概念不仅涵盖生产环境下的真实 RPC 调用，还包含开发阶段用于模拟后端行为的 Mock 体系，确保前端逻辑在脱离原生环境时的独立运行能力。

其核心职责包括：
1. **命令路由**：将前端的业务请求映射到后端的特定 Rust Command。
2. **类型安全**：通过 TypeScript 接口定义后端返回结构，确保数据一致性。
3. **容错与降级**：集成重试机制、Mock 开关及错误处理策略。
4. **平台适配**：区分桌面专属功能与移动端通用逻辑。

## Involved Modules

*   **android-frontend-hooks-stores**:
    *   提供 `tauriApi.ts` (行 48-141)，定义 `TauriApi` 类型及辅助函数（如 `withRetry`, `verifyWindowsIdentity`）。
    *   所有 Store（如 `useAuthStore`, `useConfigStore`）均依赖此模块发起 IPC 请求。
*   **android-rust-backend**:
    *   实现具体的后端 Command，如 `account_cmds.rs` 等。
    *   通过 `lib.rs` (行 66) 注册这些 Command，使其可通过 Tauri IPC 被前端调用。
*   **android-frontend-app-shell**:
    *   提供 `public/mock-tauri.js` 和 `public/mock-tauri-addon.js`，作为开发环境的 IPC 拦截器。
    *   在浏览器环境中注入全局对象以拦截 `window.__TAURI__` 或相关 API 调用。
*   **android-tauri-shell-and-plugins**:
    *   提供底层的 JNI 桥接实现（如 `MonitorServicePlugin.kt`, `KeystorePlugin.kt`），这些插件的命令最终由 `android-rust-backend` 中的 Rust 代码触发。

## Flow

### 1. 生产环境调用流 (Production Flow)

1.  **前端发起**: UI 组件或 Hook (如 `useAccount`) 调用 `tauriApi.ts` 中封装的方法（例如 `invoke('list_accounts')`）。
2.  **IPC 传输**: `@tauri-apps/api` 库将 JSON 序列化后的指令通过 Tauri 内部通道发送至 Rust 侧。
3.  **后端执行**:
    *   `lib.rs` 接收命令并分派至对应的 Command Handler（如 `account_cmds::list_accounts`）。
    *   Rust 代码执行业务逻辑（如读取 SQLite/JSON 数据库，或通过 JNI 调用 Android 原生 API）。
    *   结果通过原子性持久化模式（如 `rename` 操作）保存状态。
4.  **响应返回**: Rust 将结果序列化回传给前端。
5.  **状态更新**: 前端 Store（如 `useConfigStore`）接收数据，更新 Zustand 状态，触发 UI 重渲染。

### 2. Mock 环境调用流 (Mock Flow)

1.  **开发启动**: 开发者使用浏览器环境启动应用。
2.  **拦截注入**: `public/mock-tauri.js` 注入全局对象 `window.__TAURI_INTERNALS__`，伪造 Tauri 运行时环境。
3.  **本地模拟**:
    *   请求被重定向至 `mock-tauri.js` 中的静态数据或简单逻辑（如 `check_campus_status` 直接返回预设 JSON）。
    *   注意：部分 Mock 数据（如 `mock-tauri.js` 行 143-145）未使用标准 `ok()` 包装，需前端直读平铺结构体。
4.  **透明切换**: 构建时若移除 Mock 配置，则自动回退至真实 IPC 通道，无需修改前端业务代码。

### 3. 异常与降级流

*   **权限拦截**: 若 `capabilities.json` 未声明插件权限（如 `campus-keystore`），Tauri 会直接拒绝调用，前端需通过 `tauriApi.ts` 中的错误捕获机制处理。
*   **生物识别门控**: 敏感操作（如 `self_service_cmds`）强制经过 `identity_gate` 验证，失败则阻断流程并提示用户。

## Implementation Map

### Frontend Interface & Logic
*   **`android/frontend/src/hooks/tauriApi.ts`**:
    *   **L48-141**: `TauriApi` 接口定义，列出了所有可用的 IPC 方法签名。
    *   **L338-354**: `withRetry` 函数，为 IPC 调用提供自动重试逻辑，增强网络不稳定性下的鲁棒性。
    *   **L207-244**: `verifyWindowsIdentity`，封装 Windows Hello 验证逻辑，并在 Android 端提供多级回退（BiometricPrompt）。
    *   **L14-15**: `desktopOnly`，用于在 Android 端静默拒绝桌面专属命令（如 DHCP 查询）。

*   **`android/frontend/public/mock-tauri.js`**:
    *   **L1-100+**: 模拟 `invoke` 行为，返回硬编码的 JSON 数据，用于替代真实后端。
    *   **L143-145**: `check_campus_status` 和 `check_portal_status` 的 Mock 实现，注意其返回结构为平铺而非标准包装。

*   **`android/frontend/public/mock-tauri-addon.js`**:
    *   补充模拟一些特定插件的行为，确保前端在 Mock 模式下不会因缺少原生插件而崩溃。

### Backend Commands (Rust)
*   **`android/src-tauri/src/lib.rs`**:
    *   **L66**: `generate_handler!` 宏，集中注册所有后端 Command Handler，使其可通过 Tauri IPC 被前端调用。

*   **`android/src-tauri/src/account_cmds.rs`**:
    *   **L154, L182, L226, L256, L330, L366**: 定义账号管理相关的 Command (`list_accounts`, `switch_account`, `save_current_as_account`, `create_account`, `delete_account`, `rename_account`)。