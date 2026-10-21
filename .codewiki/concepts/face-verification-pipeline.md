---
type: concept
title: face-verification-pipeline
source_files:
  - android/frontend/src/auth/useAuth.ts
  - android/frontend/src/account/selfServiceState.ts
  - android/src-tauri/src/face_model_cmds.rs
  - android/src-tauri/src/identity_gate.rs
tags:
  - security
  - biometric
  - pipeline
  - android
---

## Overview

`face-verification-pipeline` 是系统内用于处理生物识别身份验证的核心概念，主要应用于 Android 端。它并非单一模块，而是由前端 UI 交互、本地状态门控（TTL）、Rust 后端命令执行以及底层原生插件共同组成的完整流水线。该流程旨在确保在用户执行敏感操作（如查看在线记录、修改密码、踢下线）前，必须通过 Windows Hello 或 Android 原生 BiometricPrompt 进行人脸/指纹验证。

该概念涉及两个主要的验证场景：
1.  **自助服务验证**：针对账号管理面板中的高权限操作，采用基于 TTL（Time-To-Live）的门控机制。
2.  **绑定/特权操作验证**：针对绑定运营商、查看明文密码等敏感操作，同样采用基于 TTL 的门控机制。

## Involved Modules

*   **android-frontend-auth**: 提供 `DashboardPanel` 和 `AboutDialog` 等入口，并在 `useAuth.ts` 中集成验证逻辑。它是用户发起验证请求的前端触点。
*   **android-frontend-account**: 包含 `SelfOnlineRecords`, `ChangePasswordDialog` 等组件，这些组件在执行数据查询或修改时，需调用 `selfServiceState` 中的验证逻辑。
*   **android-frontend-hooks-stores**: 提供底层的 Tauri API 封装 (`tauriApi.ts`) 和状态管理，负责将验证请求发送至后端。
*   **android-rust-backend**: 
    *   `identity_gate.rs`: 实现安全门控逻辑，校验生物识别的有效期（TTL）。
    *   `face_model_cmds.rs`: 处理人脸识别模型的下载与管理，为验证提供基础模型支持。
*   **android-tauri-shell-and-plugins**: 提供 Android 原生能力，特别是通过 JNI 桥接调用 Android 系统的 `BiometricPrompt` 或 Keystore 进行实际的硬件级验证。

## Flow

1.  **触发阶段 (Frontend)**:
    *   用户在 `android-frontend-account` 中的 `SelfOnlineRecords` 或 `ChangePasswordDialog` 执行敏感操作。
    *   或者在 `android-frontend-auth` 的全局逻辑中需要刷新认证状态。
    *   前端 Hook (`useAuth.ts` 或 `selfServiceState.ts`) 检测到未验证状态或 TTL 过期。

2.  **门控检查 (Backend Logic)**:
    *   前端调用 `verifyWindowsIdentity` (来自 `tauriApiWithRetry`)。
    *   `selfServiceState.ts` 首先检查内存中的 TTL 缓存。如果验证仍在有效期内（默认约 10 分钟），则直接返回成功，跳过硬件交互。
    *   如果 TTL 失效或强制要求重新验证，流程进入硬件验证步骤。

3.  **原生验证 (Native Execution)**:
    *   Rust 后端通过 JNI 调用 Android 原生插件 (`KeystorePlugin` 或系统 Biometric API)。
    *   Android 系统弹出生物识别对话框（人脸/指纹）。
    *   用户完成验证后，原生层返回结果给 Rust 层。

4.  **状态更新 (State Update)**:
    *   验证成功后，`identity_gate.rs` 更新内部 TTL 时间戳。
    *   前端 Store (`useAuthStore` 或 `selfServiceState`) 接收成功信号，解锁后续业务逻辑（如加载在线记录、发送改密请求）。
    *   若验证失败或取消，前端根据策略决定是重试还是报错。

5.  **模型依赖 (Model Dependency)**:
    *   在首次使用或模型过期时，`face_model_cmds.rs` 可能异步触发 `face_models_download`，确保本地有足够的特征数据支持快速匹配（尽管主要验证通常依赖系统级 API，但模型下载是潜在的前置依赖）。

## Implementation Map

*   **前端触发点**:
    *   `android/frontend/src/auth/useAuth.ts`: 定义 `doLogin` 和 `checkOnline` 中的验证分支，行 106-164, 196-243。
    *   `android/frontend/src/account/selfServiceState.ts`: 实现 TTL 门控逻辑，包括时钟回拨防护和强制验证标志，行 65-87, 105-127。
*   **后端核心逻辑**:
    *   `android/src-tauri/src/identity_gate.rs`: 核心门控实现，管理 TTL 和安全上下文。
    *   `android/src-tauri/src/face_model_cmds.rs`: 处理模型下载与状态检查，行 126。
*   **原生桥接**:
    *   `android/plugins/keystore/android/src/main/java/com/campuslogin/plugin/keystore/KeystorePlugin.kt`: 处理加密和解密及可能的生物识别辅助存储，行 34-89。