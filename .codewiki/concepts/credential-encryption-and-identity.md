---
type: concept
title: credential-encryption-and-identity
source_files:
  - android/frontend/src/auth/useAuth.ts
  - android/frontend/src/account/selfServiceState.ts
  - android/frontend/src/account/SelfServicePanel.tsx
  - android/frontend/src/settings/SettingsPanel.tsx
  - android/frontend/src/settings/useSettings.ts
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/protocol_cmds.rs
  - android/src-tauri/src/self_service_cmds.rs
  - android/src-tauri/src/identity_gate.rs
  - android/plugins/keystore/android/src/main/java/com/campuslogin/plugin/keystore/KeystorePlugin.kt
tags:
  - security
  - encryption
  - identity
  - biometric
---

## Overview

`credential-encryption-and-identity` 是系统中处理用户敏感凭证（如密码、运营商凭据）的存储加密以及高权限操作身份验证的核心概念。它并非单一模块，而是横跨前端 UI、状态管理、Rust 后端逻辑与 Android 原生插件的完整安全链路。

该概念主要解决两个维度的安全问题：
1.  **静态数据保护**：确保用户的登录密码和运营商绑定信息在落盘时经过加密，仅在内存中或解密后短暂存在明文。
2.  **动态身份门控**：在执行高危操作（如踢下线、查看在线记录、修改密码、揭示凭据）前，强制要求用户通过生物识别（Windows Hello / Android BiometricPrompt）进行实时身份确认，防止未授权访问。

## Involved Modules

该概念涉及以下关键模块及其交互角色：

*   **android-frontend-account**: 负责展示账号面板、自助服务面板及各类弹窗（BindOperatorDialog, ChangePasswordDialog）。它是触发身份验证流程的前端入口，并处理生物识别门控的状态反馈。
*   **android-frontend-hooks-stores**: 提供底层的 Tauri API 封装 (`tauriApi.ts`) 和状态管理 (`useAuthStore`, `useConfigStore`)。其中 `verifyWindowsIdentity` 函数是连接前端 UI 与系统生物识别能力的桥梁。
*   **android-rust-backend**: 作为核心业务逻辑层，负责调用后端命令。特别是 `self_service_cmds` 中的 `reveal_operator_credential` 等命令，以及 `config_state` 中对密码字段的加解密处理。
*   **android-tauri-shell-and-plugins**: 提供原生的密钥存储服务 (`KeystorePlugin.kt`)，利用 Android Keystore 系统进行 AES-GCM 加解密，为 Rust 后端提供硬件级安全支持。
*   **android-frontend-settings**: 包含设置面板和新手引导，涉及“安全门控前置”逻辑，在敏感配置变更前调用身份验证。

## Flow

数据与控制流在模块间的流动遵循“前端发起 -> 后端门控 -> 原生执行 -> 结果返回”的模式：

### 1. 敏感操作的身份验证 (Identity Gate)
当用户在 `android-frontend-account` 的 `SelfServicePanel` 或 `AccountPanel` 中尝试执行高危操作（如查询在线日志、踢下线）时：
1.  **前端拦截**: `SelfServicePanel` 或相关 Hook 检测到操作请求。
2.  **TTL 检查**: 首先检查 `selfServiceState.ts` 中的 TTL 缓存。若验证未过期且时钟正常，直接放行；否则进入下一步。
3.  **生物识别调用**: 前端调用 `tauriApiWithRetry` 发送 `verifyWindowsIdentity` 命令至 Rust 后端。
4.  **后端门控**: `self_service_cmds.rs` 接收请求，委托给 `identity_gate.rs`。
5.  **原生交互**: Rust 层通过 JNI 调用 Android 原生插件 `KeystorePlugin.kt` 或系统 BiometricPrompt，弹出生物识别对话框。
6.  **结果回传**: 验证通过后，Rust 后端更新内部 TTL 状态，允许后续操作继续执行。

### 2. 凭据加密与存储 (Credential Encryption)
在账号创建或修改密码时：
1.  **前端准备**: `ChangePasswordDialog` 收集用户输入的密码。
2.  **API 调用**: 通过 `useAccount` Hook 调用后端 `create_account` 或 `change_self_password` 命令。
3.  **后端处理**: `account_cmds.rs` 或 `self_service_cmds.rs` 接收到明文密码。
4.  **加密执行**: Rust 后端调用 `KeystorePlugin.kt` 中的 `encrypt` 方法，使用 Android Keystore 生成的密钥对密码进行 AES-GCM 加密。
5.  **持久化**: 加密后的密文写入 `config_state.rs` 管理的配置文件或数据库中。
6.  **揭示凭据**: 当需要向用户展示运营商密码时，`reveal_operator_credential` 命令触发上述逆向流程：从存储读取密文 -> 调用 `decrypt` -> 返回明文至前端展示。

## Implementation Map

具体实现位置如下：

*   **Android Keystore 加解密实现**:
    *   `android/plugins/keystore/android/src/main/java/com/campuslogin/plugin/keystore/KeystorePlugin.kt` (行 39-89): 实现 `getOrCreateKey`, `encrypt` 和 `decrypt` 核心逻辑，依赖 Android KeyStore 系统。

*   **Rust 后端身份门控与命令**:
    *   `android/src-tauri/src/identity_gate.rs`: 管理生物识别验证的 TTL 状态和锁机制。
    *   `android/src-tauri/src/self_service_cmds.rs`: 定义 `verify_biometric_identity`, `query_self_online_log`, `reveal_operator_credential` 等命令，并在执行前调用 `identity_gate`。
    *   `android/src-tauri/src/config_state.rs`: 处理配置的读写，涉及密码字段的加密存储逻辑。

*   **前端 API 封装与状态管理**:
    *   `android/frontend/src/hooks/tauriApi.ts`: 实现 `verifyWindowsIdentity` 函数，处理 Windows Hello 的生物识别调用及多级回退逻辑。
    *   `android/frontend/src/hooks/useAuthStore.ts`: 管理认证状态，可能涉及登录凭据的初步处理。

*   **前端 UI 与安全门控集成**:
    *   `android/frontend/src/account/selfServiceState.ts` (行 105-127): 实现 `useSelfServiceVerify` Hook，处理 TTL 校验、时钟回拨防护及强制验证逻辑。
    *   `android/frontend/src/account/SelfServicePanel.tsx`: 集成生物识别门控，确保在展示敏感数据前完成身份验证。
    *   `android/frontend/src/settings/SettingsPanel.tsx` & `useSettings.ts`: 在涉及安全开关或运营商绑定的设置变更中，前置调用身份验证以符合“防御性编程”约定。