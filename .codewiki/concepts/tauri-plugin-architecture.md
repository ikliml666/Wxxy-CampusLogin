---
type: concept
title: tauri-plugin-architecture
source_files:
  - android/frontend/src/hooks/tauriApi.ts
  - android/frontend/src/network/useDnsSetup.ts
  - android/frontend/src/auth/DashboardPanel.tsx
  - android/frontend/src/auth/useAuth.ts
  - android/frontend/src/account/SelfOnlineStack.tsx
  - android/frontend/src/account/selfServiceState.ts
  - android/frontend/src/settings/SettingsPanel.tsx
  - android/frontend/src/settings/useOnboardingFlow.ts
  - android/plugins/keystore/android/src/main/java/com/campuslogin/plugin/keystore/KeystorePlugin.kt
  - android/plugins/keystore/src/lib.rs
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt
  - android/plugins/foreground-service/src/lib.rs
  - android/plugins/network-bind/android/src/main/java/com/campuslogin/plugin/networkbind/NetworkBindPlugin.kt
  - android/plugins/network-bind/src/lib.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/campus_detect.rs
tags: [Tauri, IPC, Plugin, Android, Architecture]
---

## Overview

`tauri-plugin-architecture` 是 Campus Login Assistant Android 应用的核心交互架构模式，定义了前端 React UI 层与后端 Rust/Kotlin 原生能力之间的通信机制。该架构基于 Tauri v2 的插件系统，采用“Rust 桥接 + Kotlin 原生实现”的双轨模式，通过 IPC (Inter-Process Communication) 命令面暴露功能。其核心职责是将复杂的 Android 系统级操作（如 Wi-Fi 绑定、前台保活、生物识别密钥存储）封装为标准化的异步 API，并通过前端的 `tauriApi` 统一入口进行调用，同时处理网络不稳定下的重试逻辑与安全门控。

## Involved Modules

该概念涉及以下关键模块，它们共同构成了从 UI 到原生的完整调用链：

1.  **frontend-hooks-store**: 提供 `tauriApi` 对象及 `tauriApiWithRetry` 封装，作为前端所有原生调用的唯一出口；管理 `useAuthStore`, `useConfigStore` 等状态，决定何时发起 IPC 请求。
2.  **frontend-network-module**: 依赖 `tauriApi` 执行 DNS 配置、DoH 检测及 Wi-Fi 绑定逻辑，处理网络侧的原生交互。
3.  **frontend-auth-module**: 通过 `useAuth()` Hook 调用登录/登出 API，依赖 `DashboardPanel` 渲染认证状态，处理身份验证生命周期。
4.  **frontend-account-module**: 涉及敏感数据展示（如在线记录），依赖 `selfServiceState` 中的 `useHelloGate` 进行生物识别门控，确保只有验证通过后才触发后续的数据获取或账户切换。
5.  **frontend-settings-module**: 处理开机自启、电池优化引导及运营商绑定，直接调用底层权限申请与系统设置修改接口。
6.  **rust-plugins-keystore**: 提供硬件级密钥存储 (`AndroidKeyStore`)，负责密码加解密，被认证模块间接依赖以保障凭据安全。
7.  **rust-plugins-foreground-service**: 提供前台服务保活、WakeLock/WifiLock 管理及通知更新，支撑后台监控任务。
8.  **rust-plugins-network-bind**: 提供进程级 Wi-Fi 绑定、Captive Portal 自动认证及系统设置快照还原，解决校园网特殊连接需求。
9.  **rust-backend-core**: 处理全局配置持久化、多账号生命周期管理及校园网状态探测，通过 `config_state.rs` 和 `campus_detect.rs` 暴露核心业务命令。

## Flow

数据与控制流在模块间按以下路径流动：

1.  **初始化与桥接建立**：
    *   应用启动时，`frontend-hooks-store` 中的 `useAppInit` 加载初始配置。
    *   Rust 后端 (`rust-backend-core`) 注册 Commands，Kotlin 插件 (`rust-plugins-*`) 通过 JNI 暴露原生方法。
    *   前端通过 `tauriApi` 建立与后端的 IPC 通道。

2.  **UI 触发与指令转发**：
    *   用户在 `DashboardPanel` (frontend-auth-module) 或 `SettingsPanel` (frontend-settings-module) 执行操作。
    *   控制流进入 `useAuth()` 或相关 Settings Hooks，调用 `tauriApi.invoke()`。
    *   若操作涉及网络质量或 DNS 配置，`frontend-network-module` 的 `useDnsSetup()` 介入，强制使用 `tauriApiWithRetry` 封装调用，以应对网络抖动。

3.  **原生执行与安全门控**：
    *   **普通操作**：IPC 消息直达 `rust-backend-core` 或对应 Plugin 的 Rust Wrapper。
        *   例如：Wi-Fi 绑定请求由 `rust-plugins-network-bind` 接收，转发至 Kotlin 的 `NetworkBindPlugin.kt` 执行 JNI 调用。
        *   例如：保活服务启动由 `rust-plugins-foreground-service` 的 `MonitorServicePlugin` 处理，管理 WakeLock。
    *   **敏感操作**：
        *   **密钥访问**：涉及密码读写时，流程指向 `rust-plugins-keystore`。Rust 层调用 Kotlin 的 `KeystorePlugin.kt`，利用 `AndroidKeyStore` 进行 AES-GCM 加解密，明文不落地。
        *   **生物识别门控**：在 `frontend-account-module` 中，`SelfOnlineStack` 或账户切换前，先调用 `useHelloGate`。若未通过 Hello 验证，则拦截 IPC 请求，防止敏感数据泄露。

4.  **结果返回与状态同步**：
    *   原生层执行完成后，通过 Tauri Event Listener 或 Command Response 将结果返回前端。
    *   `frontend-hooks-store` 中的 Store (如 `useAuthStore`, `useQualityStore`) 接收事件并更新状态。
    *   UI 组件（如 `StatusBar`, `NetworkQualityCapsule`）响应 Store 变化，重新渲染可视化界面。

## Implementation Map

关键实现位置如下：

*   **前端统一入口**:
    *   `android/frontend/src/hooks/tauriApi.ts`: 定义 `tauriApi` 对象，封装所有 `invoke` 调用。
    *   `android/frontend/src/hooks/tauriApi.ts`: 定义 `tauriApiWithRetry`，用于网络不稳定场景的重试逻辑。

*   **Rust 插件核心 (Plugin Core)**:
    *   **Keystore**: `android/plugins/keystore/src/lib.rs`: 定义 `encrypt`/`decrypt` 方法，桥接 Kotlin 原生实现。
    *   **Foreground Service**: `android/plugins/foreground-service/src/lib.rs`: 定义 `start_monitor`, `update_notification` 等命令。
    *   **Network Bind**: `android/plugins/network-bind/src/lib.rs`: 定义 `bind_to_wifi`, `accept_wifi_network` 等命令。

*   **Kotlin 原生实现 (Native Implementation)**:
    *   **Keystore**: `android/plugins/keystore/android/src/main/java/com/campuslogin/plugin/keystore/KeystorePlugin.kt`: 实际调用 `AndroidKeyStore` 的加密解密逻辑。
    *   **Foreground Service**: `android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt`: 处理前台服务生命周期、锁管理及权限引导。
    *   **Network Bind**: `android/plugins/network-bind/android/src/main/java/com/campuslogin/plugin/networkbind/NetworkBindPlugin.kt`: 处理 Wi-Fi 绑定、SSID 获取及安全设置修改。

*   **Rust 后端核心 (Backend Core)**:
    *   `android/src-tauri/src/config_state.rs`: `get_config`/`save_config` 命令实现，处理配置持久化。
    *   `android/src-tauri/src/campus_detect.rs`: `check_campus_status`/`request_wifi_ssid_permission` 命令实现。

*   **前端业务集成点**:
    *   `android/frontend/src/network/useDnsSetup.ts`: 网络配置逻辑，调用 `tauriApi`。
    *   `android/frontend/src/auth/useAuth.ts`: 认证逻辑，调用登录/登出 API。
    *   `android/frontend/src/account/selfServiceState.ts`: 生物识别门控 TTL 对齐逻辑。