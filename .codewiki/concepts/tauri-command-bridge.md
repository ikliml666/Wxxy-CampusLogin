---
frontmatter:
  type: concept
  title: tauri-command-bridge
  source_files:
    - android/src-tauri/src/protocol_cmds.rs
    - android/src-tauri/src/config_state.rs
    - android/src-tauri/src/account_cmds.rs
    - android/src-tauri/src/monitor_loop.rs
    - android/src-tauri/src/quality_cmds.rs
    - android/src-tauri/src/update_cmds.rs
    - android/frontend/src/hooks/tauriApi.ts
    - android/frontend/src/auth/useAuth.ts
    - android/frontend/src/settings/useSettings.ts
    - android/frontend/src/account/useAccount.ts
    - android/frontend/src/account/useOperatorBind.ts
    - android/frontend/src/monitor/useMonitor.ts
    - android/frontend/src/hooks/useConfigStore.ts
    - android/frontend/src/hooks/useAuthStore.ts
    - android/frontend/src/hooks/useQualityStore.ts
    - android/frontend/src/hooks/useAppInit.ts
    - android/src-tauri/build.rs
    - android/src-tauri/capabilities/default.json
    - android/src-tauri/gen/schemas/acl-manifests.json
    - android/src-tauri/gen/schemas/capabilities.json
  tags: [Tauri, IPC, Bridge, Android, Security, State-Sync]

## Overview

`tauri-command-bridge` 是连接 Android 前端（React/Vite）与 Tauri 后端（Rust/Cargo）的核心通信桥梁。它并非单一代码文件，而是由**前端 API 封装层**、**后端命令注册面**以及**运行时权限校验面**共同构成的跨模块概念体系。

该桥接层解决了 WebView 与原生环境之间的上下文隔离问题，将复杂的 Rust 业务逻辑（如网络探针、账号管理、配置持久化）抽象为前端可调用的异步函数。同时，它引入了重试机制、状态合并策略及生物识别门控，确保在移动端不稳定网络和高频交互场景下的数据一致性与安全性。

## Involved Modules

1.  **android-frontend-hooks-and-stores**: 前端基础设施层，负责维护 `tauriApi` 实例，定义所有向后端发起的 IPC 调用契约，并处理本地状态（Zustand Stores）与后端响应的同步。
2.  **desktop-rust-core**: 后端核心骨架，通过 `#[tauri::command]` 宏暴露所有业务命令，实现具体的网络协议、配置读写及后台监控逻辑。
3.  **desktop-tauri-shell-config**: 构建壳层，负责编译时生成 ACL (Access Control List) 和 Capability 文件，决定哪些命令在前端可见且被允许执行。
4.  **android-frontend-auth / account / settings / monitor**: 业务功能模块，作为 `tauri-command-bridge` 的主要消费者，通过 Hook 或直接调用 API 触发后端行为。

## Flow

数据与控制流在 `tauri-command-bridge` 中遵循以下路径：

### 1. 声明与授权 (Build Time)
*   **后端注册**: 在 `desktop-rust-core` (`src-tauri/src/*.rs`) 中定义 `pub fn command_name()` 函数。
*   **权限映射**: `desktop-tauri-shell-config` (`build.rs`, `capabilities/default.json`) 将这些函数映射到具体的 Capability 权限集。若命令未在 `default.json` 或生成的 `acl-manifests.json` 中声明，Tauri 运行时会拒绝调用。

### 2. 发起调用 (Frontend Runtime)
*   **入口**: 业务模块（如 `useAuth` in `android-frontend-auth`）调用 `tauriApi` 中的对应方法。
*   **封装**: `tauriApi` (`android-frontend-hooks-and-stores`) 提供 `invoke` 包装器。对于关键操作（如 `saveConfig`），使用 `tauriApiWithRetry` 进行自动重试。
*   **参数传递**: 前端将 JSON 序列化的对象通过 Tauri IPC 通道发送至 Rust 侧。

### 3. 执行与验证 (Backend Runtime)
*   **路由分发**: Rust 侧根据命令名分发至对应的 Handler (`config_state.rs`, `account_cmds.rs` 等)。
*   **安全门控**: 敏感操作（如运营商绑定、密码修改）在 Rust 侧或前端 Hook 中集成生物识别验证（Windows Hello/Android Biometric），通过后才允许执行核心逻辑。
*   **状态变更**: 后端执行逻辑（如写入配置文件、发起网络请求），并通过 `emit` 事件向前端广播状态更新（如 `CONFIG_UPDATED`）。

### 4. 响应与同步 (State Sync)
*   **直接返回**: 大多数命令返回 Promise，前端在 `.then()` 中更新 Zustand Store（如 `useAuthStore`, `useConfigStore`）。
*   **脏数据冲突解决**: `useConfigStore` 监听后端广播，结合 `dirtyFields` 机制，防止后端旧快照覆盖前端未落盘的修改。
*   **降级处理**: 若 IPC 失败，前端 Hook（如 `useMonitor`）利用 `busyRef` 和超时机制进行降级或错误提示。

## Implementation Map

| 层级 | 关键文件 | 行号/位置 | 职责描述 |
| :--- | :--- | :--- | :--- |
| **Backend Commands** | `android/src-tauri/src/protocol_cmds.rs` | Global | 定义登录、注销、Portal 探测、WiFi 绑定等网络协议相关命令。 |
| | `android/src-tauri/src/config_state.rs` | L440-512 | 实现配置的加密读取 (`get_config`) 与原子写入 (`save_config`)，包含全局锁保护。 |
| | `android/src-tauri/src/account_cmds.rs` | Global | 处理账号列表 CRUD、切换及别名管理。 |
| | `android/src-tauri/src/monitor_loop.rs` | Global | 启动/停止后台监控循环，触发网络质量检查。 |
| | `android/src-tauri/src/quality_cmds.rs` | Global | 执行实时的网络延迟测试 (`check_network_quality`)。 |
| | `android/src-tauri/src/update_cmds.rs` | Global | 处理应用版本检查、下载及安装流程。 |
| **Frontend API Layer** | `android/frontend/src/hooks/tauriApi.ts` | L190-324 | **核心桥接点**。定义 `tauriApi` 对象，将所有后端命令映射为前端 Promise 方法。 |
| | `android/frontend/src/hooks/tauriApi.ts` | L356-361 | 定义 `tauriApiWithRetry`，专门用于 `saveConfig` 等高可靠性要求的操作。 |
| **Frontend Consumers** | `android/frontend/src/auth/useAuth.ts` | Global | 调用 `doLogin`/`doLogout`，处理认证状态流转。 |
| | `android/frontend/src/account/useAccount.ts` | Global | 调用账号相关命令，处理 `switchInFlight` 并发去重。 |
| | `android/frontend/src/account/useOperatorBind.ts` | Global | 调用运营商绑定命令，集成二次验证逻辑。 |
| | `android/frontend/src/settings/useSettings.ts` | L13-65 | 调用设置保存命令，处理平台差异化字段映射。 |
| | `android/frontend/src/monitor/useMonitor.ts` | L12 | 调用后台检查命令，实施生命周期保护。 |
| **Frontend State Sync** | `android/frontend/src/hooks/useConfigStore.ts` | L56-99 | 监听配置变更，处理 `mergeConfigFromBackend` 与本地脏数据的冲突解决。 |
| | `android/frontend/src/hooks/useAuthStore.ts` | L106-164 | 响应登录/登出事件，更新认证状态。 |
| **Permission/Schema** | `android/src-tauri/capabilities/default.json` | L1-14 | 声明前端可访问的命令权限白名单。 |
| | `android/src-tauri/gen/schemas/acl-manifests.json` | L1 | 运行时 ACL 引擎读取的清单，决定 IPC 拦截或放行。 |