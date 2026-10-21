---
frontmatter:
  type: concept
  title: self-service-account-flow
  source_files:
    - android/frontend/src/account/SelfServicePanel.tsx
    - android/frontend/src/account/selfServiceState.ts
    - android/frontend/src/account/useSelfServiceVerify.ts
    - android/frontend/src/account/useHelloGate.ts
    - android/frontend/src/account/AccountPanel.tsx
    - android/frontend/src/account/ChangePasswordDialog.tsx
    - android/frontend/src/account/BindOperatorDialog.tsx
    - android/frontend/src/account/index.ts
    - android/src-tauri/src/self_service_cmds.rs
    - android/src-tauri/src/identity_gate.rs
    - android/src-tauri/src/lib.rs
    - android/frontend/src/settings/useSettings.ts
    - android/frontend/src/settings/KeepAliveSettingsCard.tsx
tags: [self-service, account-management, security-gate, biometric-auth, ttl-verification]

## Overview

`self-service-account-flow` 是校园网登录应用中用于处理高敏感度个人账号操作的核心业务概念。它涵盖了自助服务面板（Self Service）中的身份验证、运营商绑定查询、在线状态监控、密码修改及凭据揭示等关键流程。该概念的核心特征在于其严格的**安全门控机制**：所有敏感操作均强制经过生物识别或系统级身份验证（如 Windows Hello / Android Biometric），并受 TTL（Time-To-Live）时间窗口约束，以防止未授权访问和凭证泄露。

该流程贯穿了前端 UI 交互层、本地状态管理层、Rust 后端逻辑层以及 Android 原生插件层，形成了一个从“用户意图”到“身份核验”再到“数据执行”的闭环。

## Involved Modules

该概念涉及以下核心模块，按数据与控制流向排列：

1.  **`android-frontend-account`**: 概念的主要载体。包含 `SelfServicePanel` 组件、`useSelfServiceVerify` Hook 以及 `selfServiceState.ts` 状态管理文件。负责收集用户输入、展示自助服务结果、维护生物识别验证的时间戳状态。
2.  **`android-rust-backend`**: 业务逻辑与数据持久化核心。`self_service_cmds.rs` 提供了 Tauri Commands 入口，`identity_gate.rs` 实现了后端的 TTL 校验与安全上下文隔离。
3.  **`android-frontend-hooks-stores`**: 基础设施层。提供 `tauriApiWithRetry` 封装，负责将前端的 API 调用转发至 Rust 后端，并处理网络重试逻辑。
4.  **`android-frontend-settings`**: 前置依赖模块。在用户尝试进行某些自助服务操作前，可能需要通过 `useSettings` 或 `KeepAliveSettingsCard` 触发初始的身份验证门控（如 `ensureBindHello`），确保设备环境安全。
5.  **`android-tauri-shell-and-plugins`**: 底层能力支持。虽然主要涉及 Keystore 插件（用于密码加密存储），但在自助服务流程中，密码的解密与重新加密依赖于底层的 `KeystorePlugin` 提供的 AES-GCM 能力。

## Flow

`self-service-account-flow` 的数据与控制流动遵循以下路径：

### 1. 入口与前置检查 (UI -> Store)
*   用户在 `SelfServicePanel` (行 1-348) 中发起操作（如查看在线记录、修改密码）。
*   `SelfServicePanel` 调用 `useSelfServiceVerify` Hook (行 105-127)。
*   **TTL 校验**: `useSelfServiceVerify` 读取 `selfServiceState.ts` 中的状态。如果当前时间与上次成功验证时间差超过 TTL（默认 10 分钟，含时钟回拨防护），则触发新的验证流程；否则直接复用缓存的验证令牌。

### 2. 身份验证门控 (Store -> Backend/Native)
*   若需重新验证，`useSelfServiceVerify` 调用 `verifyWindowsIdentity` (源自 `hooks/tauriApi.ts`)。
*   **跨平台适配**:
    *   在 Android 端，此调用可能下沉至 `android-frontend-settings` 中定义的 `ensureBindHello` 逻辑，或通过 JNI 桥接调用 Android 原生的 BiometricPrompt。
    *   验证成功后，`selfServiceState` 更新 `lastVerifiedAt` 时间戳。

### 3. 业务指令下发 (Frontend -> Rust Backend)
*   验证通过后，前端组件（如 `SelfServicePanel` 或 `ChangePasswordDialog`）通过 `tauriApiWithRetry` 调用具体的 Tauri Command：
    *   `query_self_dashboard`: 获取在线设备与流量统计。
    *   `change_self_password`: 提交新密码。
    *   `reveal_operator_credential`: 揭示运营商账号密码。
*   请求载荷中包含由前端生成的临时 Token 或会话标识，用于后端二次确认。

### 4. 后端安全拦截与执行 (Rust Backend)
*   `lib.rs` (行 52-60) 路由请求至 `self_service_cmds.rs`。
*   **Identity Gate**: `self_service_cmds` 内部调用 `identity_gate` (行 67 等)。
    *   `identity_gate.rs` 检查请求携带的安全上下文是否与 `selfServiceState` 中记录的最近一次生物识别验证匹配。
    *   若匹配且未过期，允许执行；否则返回权限错误。
*   **数据操作**:
    *   对于密码修改，调用 `campus_keystore` (通过 JNI) 进行加解密。
    *   对于查询操作，读取 `login_history` 或实时网络状态。

### 5. 结果反馈与状态同步 (Backend -> Frontend)
*   Rust 后端返回 JSON 结果。
*   前端 `tauriApiWithRetry` 接收响应。
*   `SelfServicePanel` 或相关 Dialog 更新 Zustand Store (`useConfigStore`, `useAuthStore`)。
*   UI 刷新显示最新状态（如“密码修改成功”、“当前在线设备列表”）。

## Implementation Map

以下是实现该概念的关键代码位置：

### 前端状态与验证逻辑
*   **TTL 状态管理**: `android/frontend/src/account/selfServiceState.ts`
    *   定义 `SelfServiceState` 接口，管理 `lastVerifiedAt`。
    *   实现 `isVerificationExpired` 逻辑，包含 `elapsed >= 0` 防回拨保护。
*   **验证 Hook**: `android/frontend/src/account/useSelfServiceVerify.ts`
    *   行 105-127: 核心验证逻辑，协调 `selfServiceState` 与后端 API。
*   **Hello Gate 集成**: `android/frontend/src/account/useHelloGate.ts`
    *   行 65-87: 处理 Windows Hello / Android Biometric 的具体调用封装。

### 前端 UI 组件
*   **自助服务主面板**: `android/frontend/src/account/SelfServicePanel.tsx`
    *   行 1-348: 渲染自助服务入口，调用 Hooks 获取数据。
*   **密码修改弹窗**: `android/frontend/src/account/ChangePasswordDialog.tsx`
    *   行 1-110: 处理密码输入表单，调用 `change_self_password` 命令。
*   **运营商绑定弹窗**: `android/frontend/src/account/BindOperatorDialog.tsx`
    *   行 1-275: 处理运营商信息查看与绑定逻辑。

### Rust 后端实现
*   **命令入口**: `android/src-tauri/src/self_service_cmds.rs`
    *   行 67: `verify_biometric_identity` (若存在独立验证命令)。
    *   行 85: `bind_operator`。
    *   行 162: `query_self_dashboard`。
    *   行 263: `change_self_password`。
*   **安全门控**: `android/src-tauri/src/identity_gate.rs`
    *   实现 `check_identity` 函数，比对请求上下文与内存中的验证状态。

### 配置与设置联动
*   **设置模块联动**: `android/frontend/src/settings/KeepAliveSettingsCard.tsx`
    *   可能在保活设置变更时触发全局重置或重新验证，影响自助服务的可用性。