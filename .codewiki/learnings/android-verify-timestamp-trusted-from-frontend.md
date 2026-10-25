---
title: 跨平台生物识别验证门（Identity Gate）
type: module
source_files:
  - android/src-tauri/src/self_service_cmds.rs
  - tauri-app/src-tauri/src/platform/identity.rs
tags: [安全, 验证门, 生物识别, Windows Hello, TTL]
---

## Overview

本模块实现了跨平台的“身份验证门”（Identity Gate），用于在用户执行敏感自助服务操作（如绑定/解绑运营商、修改密码、注销会话、明文揭示凭据）前，强制要求通过生物识别或系统级身份验证。

核心机制是一个基于内存的时间戳校验器：
1. **写入端**：由前端触发后端命令，后端在确认生物识别成功后，调用 `note_identity_verified()`（`tauri-app/src-tauri/src/platform/identity.rs:34-36`）记录当前 Epoch 秒到静态原子变量 `LAST_VERIFY_EPOCH_SECS`（`tauri-app/src-tauri/src/platform/identity.rs:28`）。
2. **读取端**：敏感操作命令在执行前调用 `ensure_identity_gate()`（`android/src-tauri/src/self_service_cmds.rs:42-51`）或 `identity_verified_recently()`（`tauri-app/src-tauri/src/platform/identity.rs:39-42`），检查时间戳是否在 TTL（600秒，`tauri-app/src-tauri/src/platform/identity.rs:39`）内。
3. **防御目标**：防止 Webview 层绕过生物识别流程，直接调用后端 API 执行敏感操作。

**Android vs Desktop 差异**：
- **Desktop (Windows)**：命令 `verify_identity`（`tauri-app/src-tauri/src/platform/identity.rs:60-70`）内部直接调用 WinRT `UserConsentVerifier`，本地完成指纹/面部/PIN 验证后写时间戳。强度最高，无密码回退路径。
- **Android**：命令 `verify_biometric_identity`（`android/src-tauri/src/self_service_cmds.rs:70-86`）**仅负责写时间戳**。实际的生物识别逻辑由 Android 原生层（Kotlin/Java）处理，验证通过后调用此 Tauri 命令并传入 `success=true`。Tauri 侧不直接执行生物识别算法。

## Key Components

### 1. 通用验证门逻辑与命令包装 (`android/src-tauri/src/self_service_cmds.rs`)

- **`CommandResult`** (10-17): `#[derive(Serialize, Clone)]` 标记的网络命令统一返回结构。
  - 字段：
    - `pub success: bool` (11)
    - `pub message: Option<String>` (13-14, 带 `#[serde(skip_serializing_if = "Option::is_none")]`)
    - `pub data: Option<serde_json::Value>` (15-16, 带 `#[serde(skip_serializing_if = "Option::is_none")]`)
  - 方法：
    - `ok_msg(msg: &str) -> Self` (20-22): 构造成功结果（带消息，`data` 为 `None`）。
    - `err(msg: &str) -> Self` (23-25): 构造失败结果（带消息，`data` 为 `None`）。
- **`resolve_self_password`** (30-39): 自助密码回退辅助。前端传入 MASK(`PASSWORD_MASK`)/空时回退至 `settings.password`（持久化登录密码），两者皆无返回 `None`（提示输入）。
- **`ensure_identity_gate`** (42-51): 验证门拦截器。若 `settings.self_hello_enabled` 为 `false` 返回 `None`（放行）；若 `identity_verified_recently()` 为 `true` 返回 `None`（放行）；否则返回 `Some("生物识别验证已过期,请重新验证")`。
- **`load_settings`** (53-55): 异步加载 `AppHandle` 关联的当前配置 `Settings`。
- **`local_addr_of`** (57-63): 从 `AndroidState` 提取缓存的本地源 IP（`cached_source_ip`），供通道自动路由使用。
- **`verify_biometric_identity`** (69-85): **关键安全点**。前端在生物识别/2D 人脸**认证成功后**调用。仅当 `success == true` 时调用 `note_identity_verified()` (76)，返回 `ok_msg("验证通过")`；`success == false` 时记录 `log_warn!` 审计日志并返回 `err("验证未通过")`，**不**更新时间戳。`method: String` 记录验证方式（`biometric` / `face2d`）用于失败审计。`_consent_message: Option<String>` 未使用。
- **受验证门保护的写操作**：
  - `bind_operator` (87-132)：106-108 行调 `ensure_identity_gate`，校验学号/手机/密码后执行 `bind_operator_via`。
  - `self_offline_session` (241-269)：254-256 行调 `ensure_identity_gate`，执行 `offline_session_via` 注销会话。
  - `change_self_password` (275-312)：289-291 行调 `ensure_identity_gate`；295-299 校验新密码 6-11 位 ASCII 字母数字；成功后服务端强制下线，前端负责同步本地登录密码。
  - `reveal_operator_credential` (315-345)：**强制验证门**，无视总开关 `self_hello_enabled`，直接检查 `!crate::identity_gate::identity_verified_recently()` (329-331)，用于明文揭示手机号/短信密码。
- **不受验证门保护的读操作**（查询类不设门）：
  - `query_bind_status` (134-166)
  - `query_self_dashboard` (168-193)
  - `query_self_online_log` (210-239，含 `is_iso_date` 严格 `YYYY-MM-DD` 校验 196-208)
- **`is_iso_date`** (196-208): 纯函数，严格校验日期格式 `YYYY-MM-DD` 防注入。
- **单元测试** (347-391): `settings_with_password` (352-357)、`自助密码回退_掩码与空回退_新值直用` (359-367)、`验证门_总开关关闭放行_开启走TTL` (369-382)、`is_iso_date_严格校验` (384-391)。

### 2. Windows 身份验证实现 (`tauri-app/src-tauri/src/platform/identity.rs`)

- **`DEFAULT_CONSENT_MESSAGE`** (24): 弹窗文案缺省值 `"请完成 Windows 身份验证"`。
- **`LAST_VERIFY_EPOCH_SECS`** (28): `static AtomicU64`，存储最近一次验证通过的 Epoch 秒（初始 0 = 从未验证）。
- **`IDENTITY_VERIFY_TTL_SECS`** (31): `pub const u64 = 600`，验证有效期 10 分钟。
- **`note_identity_verified`** (34-36): 验证通过后以 `Ordering::Release` 写入当前时间戳。
- **`identity_verified_recently`** (39-42): 以 `Ordering::Acquire` 加载时间戳，调用 `is_within_ttl` 判断有效性。
- **`is_within_ttl`** (45-47): 纯函数 TTL 判定：`verified_at > 0 && now >= verified_at && now - verified_at <= ttl_secs`（防时钟回拨）。
- **`epoch_secs_now`** (49-54): 获取当前 `UNIX_EPOCH` 秒，失败兜底 0。
- **`verify_identity`** (60-70): 入口函数。处理空 `consent_message` 兜底，调用 `verify_hello`。
- **`verify_hello`** (78-117): 查询 `UserConsentVerifier::CheckAvailabilityAsync`，不可用则返回引导配置错误；主路径（Win11+）尝试 interop 绑定 `owner_hwnd`；兜底路径调用 `spawn_consent_focus_nudger` + `RequestVerificationAsync`。
- **`try_verification_for_window`** (123-149): Win11 interop 封装 `IUserConsentVerifierInterop::RequestVerificationForWindowAsync`，绑定窗口句柄使弹窗置前；返回 `None` 表示 interop 不可用。
- **`spawn_consent_focus_nudger`** (155-172): 后台线程，每 250ms 轮询 `"Credential Dialog Xaml Host"` 类名并 `SetForegroundWindow`，最多 12 次（3秒）。
- **`await_winrt_operation`** (176-204): 非阻塞等待 WinRT `IAsyncOperation`，通过 `SetCompleted` 回调 + tokio `oneshot` 通道送回结果，避免阻塞 tokio worker。
- **单元测试** (206-222): `identity_ttl_boundary` (210-222) 覆盖 0 哨兵、边界值、超时、时钟回拨拒绝逻辑。

## Architecture

```text
┌───────────────────────┐    ┌──────────────────────────────┐
│ Frontend (Webview)    │    │ Android Backend              │
│ Native Biometric      │───►│ self_service_cmds.rs         │
│ Sensitive Op          │─┐  │ verify_biometric_identity() │
└───────────────────────┘ │  │ (69-85) ─ note_identity ┐  │
                          │  │ bind_operator()(87-132) │ │  │
                          │  │ ensure_identity_gate() │ │  │
                          │  │ (42-51) ◄─ check ───────┼─┘  │
┌───────────────────────┐ │  └────────────────────────┘ │  │
│ Windows Backend       │ │  ┌────────────────────────┐  │  │
│ identity.rs           │ │  │ Identity Gate State    │  │  │
│ verify_identity()(60-70│─┘  │ LAST_VERIFY_EPOCH_SECS│◄─┘  │
│ UserConsentVerifier   │───►│ (28) TTL=600s (31)    │     │
└───────────────────────┘    │ identity_verified_     │     │
                             │ recently()(39-42) ◄────┘     │
                             └────────────────────────┘     │
```

## Data Flow

1. **验证阶段 (Verification)**：
   - Android: 用户点击生物识别 → Android Native 层验证成功 → 调用 `verify_biometric_identity(success=true, method='biometric')` (`android/src-tauri/src/self_service_cmds.rs:70-86`) → Rust 后端调用 `note_identity_verified()` (`tauri-app/src-tauri/src/platform/identity.rs:34-36`) → 写入 `LAST_VERIFY_EPOCH_SECS` (`tauri-app/src-tauri/src/platform/identity.rs:28`)。
   - Windows: 用户点击 "Verify Identity" → Tauri 命令 `verify_identity` (`tauri-app/src-tauri/src/platform/identity.rs:60-70`) → `verify_hello()` (`tauri-app/src-tauri/src/platform/identity.rs:78-117`) → WinRT `UserConsentVerifier` 弹出 Hello 对话框 → 验证成功 → 回调触发 `note_identity_verified()`。

2. **操作阶段 (Action)**：
   - 用户发起敏感操作（如绑定运营商）→ 前端调用 `bind_operator(...)` (`android/src-tauri/src/self_service_cmds.rs:87-132`)。
   - Rust 后端 `bind_operator` 首先调用 `ensure_identity_gate(&settings)` (`android/src-tauri/src/self_service_cmds.rs:106-108`)。
   - `ensure_identity_gate` (`android/src-tauri/src/self_service_cmds.rs:42-51`) 检查 `self_hello_enabled` 开关和 `identity_verified_recently()` (`tauri-app/src-tauri/src/platform/identity.rs:39-42`)。
   - 若未通过，返回错误 `"生物识别验证已过期,请重新验证"`，操作终止。
   - 若通过，解析密码后继续执行 `campus_login_lib::self_service::bind_operator_via` 网络请求 (`android/src-tauri/src/self_service_cmds.rs:128`)。

3. **特殊路径 (Reveal)**：
   - `reveal_operator_credential` (`android/src-tauri/src/self_service_cmds.rs:316-346`) 不使用 `ensure_identity_gate`（因该函数受总开关 `self_hello_enabled` 影响），而是直接硬编码检查 `!crate::identity_gate::identity_verified_recently()` (`android/src-tauri/src/self_service_cmds.rs:316-318`)。这确保了无论总开关如何，查看明文凭证必须经过生物识别。

## Connections

[[verification-gate-tiers]]
[[windows-hello-only-identity]]
[[self-service-protocol]]

## Known Issues

- **Android 历史漏洞修复**：旧版 `verify_biometric_identity` 无 `success` 参数，前端任何 invoke（含异常路径或被篡改调用）都会直接 `note_identity_verified()`，验证门形同虚设。当前实现（`android/src-tauri/src/self_service_cmds.rs:65-68` 注释及 69-85 代码）强制要求 `success == true` 才解锁验证门，`method` 记录验证方式用于失败审计。
- **Windows Hello 弹窗前台 Bug**：Consent 弹窗由独立 broker 进程（Credential Manager UI Host）显示且不抢前台是已知 Windows bug（task.ms/49689617）。当前实现主路径（Win11 Build 22000+）通过 `IUserConsentVerifierInterop` 绑定主窗口 HWND 天然置前（`tauri-app/src-tauri/src/platform/identity.rs:97-102`, `123-149`）；兜底路径（Win10/interop 不可用）启动 `spawn_consent_focus_nudger` (`tauri-app/src-tauri/src/platform/identity.rs:155-172`) 轮询提前台（Chromium/gsudo 同款做法）。
- **Windows Hello 无回退**：设备未配置 Hello 时直接返回错误引导用户配置，**不回退凭据对话框**（2026-09-05 用户要求移除 CredUI 输密码的回退路径，见 `tauri-app/src-tauri/src/platform/identity.rs:3-5` 注释）。