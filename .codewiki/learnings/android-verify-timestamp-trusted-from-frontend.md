---
title: 跨平台生物识别验证门（Identity Gate）
type: module
source_files:
  - android/src-tauri/src/self_service_cmds.rs
  - tauri-app/src-tauri/src/platform/identity.rs
tags: [安全, 验证门, 生物识别, Windows Hello, TTL]
---

## Overview

本模块实现了跨平台的“身份验证门”（Identity Gate），用于在用户执行敏感自助服务操作（如解绑运营商、修改密码、注销会话）前，强制要求通过生物识别或系统级身份验证。

核心机制是一个基于内存的时间戳校验器：
1. **写入端**：由前端触发后端命令，后端在确认生物识别成功后，调用 `note_identity_verified()` 记录当前 Epoch 秒。
2. **读取端**：敏感操作命令在执行前调用 `ensure_identity_gate()` / `identity_verified_recently()`，检查时间戳是否在 TTL（600秒）内。
3. **防御目标**：防止 Webview 层绕过生物识别流程，直接调用后端 API 执行敏感操作。

**Android vs Desktop 差异**：
- **Desktop (Windows)**：命令 `verify_windows_identity` 内部直接调用 WinRT `UserConsentVerifier`，本地完成指纹/面部/PIN 验证后写时间戳。强度最高。
- **Android**：命令 `verify_biometric_identity` **仅负责写时间戳**。实际的生物识别逻辑由 Android 原生层（Kotlin/Java）处理，验证通过后调用此 Tauri 命令。Tauri 侧不直接执行生物识别算法。

## Key Components

### 1. 通用验证门逻辑 (`self_service_cmds.rs`)

位于 `android/src-tauri/src/self_service_cmds.rs`。

*   **`ensure_identity_gate(settings: &Settings) -> Option<String>`** (Line 48-55)
    *   验证门拦截器。
    *   若 `settings.self_hello_enabled` 为 `false`，返回 `None`（放行）。
    *   若 `identity_verified_recently()` 为 `true`，返回 `None`（放行）。
    *   否则返回错误文案 `"生物识别验证已过期,请重新验证"`。
*   **`resolve_self_password(incoming: &str, settings: &Settings) -> Option<String>`** (Line 32-40)
    *   密码解析辅助函数。
    *   优先使用前端传入的密码；若为空或掩码（`PASSWORD_MASK`），则回退到 `settings.password`（持久化存储的登录密码）。
    *   若两者皆无，返回 `None`（提示用户输入）。
*   **`is_iso_date(s: &str) -> bool`** (Line 135-144)
    *   严格的 YYYY-MM-DD 日期校验，防止注入。

### 2. Android 特定命令 (`self_service_cmds.rs`)

位于 `android/src-tauri/src/self_service_cmds.rs`。

*   **`verify_biometric_identity(success: bool, method: String, _consent_message: Option<String>)`** (Line 67-82)
    *   **关键安全点**：接收 `success` 布尔值。
    *   仅当 `success == true` 时调用 `crate::identity_gate::note_identity_verified()`。
    *   若 `success == false`，记录警告日志并返回错误，**不**更新时间戳。
    *   注：此处 `method` 参数用于审计日志，记录是 `biometric` 还是 `face2d`。
*   **受验证门保护的写操作**：
    *   `bind_operator` (Line 87-125)：调用 `ensure_identity_gate`。
    *   `self_offline_session` (Line 196-216)：调用 `ensure_identity_gate`。
    *   `change_self_password` (Line 229-263)：调用 `ensure_identity_gate`。
    *   `reveal_operator_credential` (Line 268-289)：**强制验证门**，无视总开关 `self_hello_enabled`，直接检查 `identity_verified_recently()`。
*   **不受验证门保护的读操作**：
    *   `query_bind_status` (Line 130-156)
    *   `query_self_dashboard` (Line 161-180)
    *   `query_self_online_log` (Line 185-193)

### 3. Windows 身份验证实现 (`identity.rs`)

位于 `tauri-app/src-tauri/src/platform/identity.rs`。

*   **常量与状态**：
    *   `LAST_VERIFY_EPOCH_SECS: AtomicU64` (Line 30)：静态原子变量，存储最近一次验证通过的 Epoch 秒。初始值为 0。
    *   `IDENTITY_VERIFY_TTL_SECS: u64 = 600` (Line 33)：验证有效期 10 分钟。
    *   `DEFAULT_CONSENT_MESSAGE: &str` (Line 26)：默认弹窗文案。
*   **核心函数**：
    *   `note_identity_verified()` (Line 36-38)：写入当前时间戳到 `LAST_VERIFY_EPOCH_SECS`。
    *   `identity_verified_recently() -> bool` (Line 41-44)：加载时间戳，调用 `is_within_ttl` 判断是否有效。
    *   `is_within_ttl(verified_at, now, ttl_secs) -> bool` (Line 47-49)：纯函数。检查 `verified_at > 0` 且差值 <= TTL。防时钟回拨。
    *   `verify_identity(consent_message, owner_hwnd)` (Line 61-73)：入口函数。处理空消息兜底，调用 `verify_hello`。
    *   `verify_hello(consent_message, owner_hwnd)` (Line 78-116)：
        1.  查询 `UserConsentVerifier::CheckAvailabilityAsync`。不可用则报错引导配置。
        2.  **主路径 (Win11 Build 22000+)**：若提供 `owner_hwnd`，尝试通过 `IUserConsentVerifierInterop::RequestVerificationForWindowAsync` 绑定窗口句柄，确保 Consent 弹窗在前台。
        3.  **兜底路径 (Win10/失败)**：调用 `spawn_consent_focus_nudger()` 启动后台线程轮询窗口类名 `"Credential Dialog Xaml Host"` 并强制置前。随后调用 `RequestVerificationAsync`。
    *   `try_verification_for_window(hwnd, message)` (Line 121-146)：InterOp 接口调用封装。
    *   `spawn_consent_focus_nudger()` (Line 151-167)：后台线程，每 250ms 轮询 FindWindowW，最多 12 次（共 3 秒）。
    *   `await_winrt_operation<T>(op)` (Line 172-196)：非阻塞等待 WinRT IAsyncOperation，使用 tokio oneshot channel。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│                         Frontend (Webview)                          │
│  ┌──────────────┐       ┌──────────────────────────────────────┐   │
│  │ Biometric UI │──────►│ invoke('verify_biometric_identity')  │   │
│  │ (Native/KT)  │   OK  │ success=true, method='biometric'     │   │
│  └──────────────┘       └──────────────┬───────────────────────┘   │
│                                        │                           │
│  ┌──────────────┐       ┌──────────────▼───────────────────────┐   │
│  │ Sensitive Op │──────►│ invoke('bind_operator' / 'offline')  │   │
│  │ (Bind/Off)   │   OK  │                                      │   │
│  └──────────────┘       └──────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      Tauri Backend (Rust)                           │
│                                                                     │
│  ┌─────────────────────────┐    ┌──────────────────────────────┐   │
│  │ self_service_cmds.rs    │    │ identity_gate (Static State) │   │
│  │                         │    │                              │   │
│  │ verify_biometric_       │    │ note_identity_verified()     │   │
│  │ identity(success)       │───►│ LAST_VERIFY_EPOCH_SECS = now │   │
│  │                         │    │                              │   │
│  │ bind_operator()         │    │ identity_verified_recently() │   │
│  │ ├─ ensure_identity_     │◄───│ is_within_ttl(600s)          │   │
│  │ │ gate()                │    └──────────────────────────────┘   │
│  │ └─ campus_login_lib...  │                                       │
│  │                         │    ┌──────────────────────────────┐   │
│  │ reveal_operator_        │    │ platform/identity.rs (Win)   │   │
│  │ credential()            │    │                              │   │
│  │ └─ direct check gate    │───►│ verify_identity()            │   │
│  │ (no switch override)    │    │ ├─ UserConsentVerifier       │   │
│  └─────────────────────────┘    │ └─ Focus Nudger (Win10)      │   │
│                                 └──────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **验证阶段 (Verification)**：
    *   Android: 用户点击生物识别 → Android Native 层验证成功 → 调用 `verify_biometric_identity(success=true)` → Rust 后端调用 `note_identity_verified()` → 写入 `LAST_VERIFY_EPOCH_SECS`。
    *   Windows: 用户点击 "Verify Identity" → Tauri 命令 `verify_windows_identity` → `verify_hello()` → WinRT `UserConsentVerifier` 弹出 Hello 对话框 → 用户验证成功 → 回调触发 `note_identity_verified()`。

2.  **操作阶段 (Action)**：
    *   用户发起敏感操作（如解绑）→ 前端调用 `bind_operator(...)`。
    *   Rust 后端 `bind_operator` 首先调用 `ensure_identity_gate(&settings)`。
    *   `ensure_identity_gate` 检查 `self_hello_enabled` 开关和 `identity_verified_recently()`。
    *   若未通过，返回错误 `"生物识别验证已过期..."`，操作终止。
    *   若通过，继续执行 `campus_login_lib::self_service::bind_operator` 网络请求。

3.  **特殊路径 (Reveal)**：
    *   `reveal_operator_credential` 不使用 `ensure_identity_gate`（因为该函数会检查总开关），而是直接硬编码检查 `!crate::identity_gate::identity_verified_recently()`。这确保了无论总开关如何，查看明文凭证必须经过生物识别。

## Connections

[[verification-gate-tiers]]
[[windows-hello-only-identity]]
[[self-service-protocol]]