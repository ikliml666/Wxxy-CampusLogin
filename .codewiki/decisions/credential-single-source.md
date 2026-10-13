---
title: 凭据单一来源（登录信息为自助服务与运营商绑定唯一凭据源）
type: decision
source_files:
  - tauri-app/src-tauri/src/commands/self_service.rs
  - tauri-app/src-tauri/src/config/model.rs
  - android/src-tauri/src/self_service_cmds.rs
tags: [决策, 凭据, 配置, 安全, 引导]
---

## Overview

本决策确立：**校园网登录信息中的 `password` 字段是自助服务系统（Self-Service）与运营商账号绑定的唯一密码来源**。

历史背景中，系统曾并存两套凭据：登录信息 `config.user`/`config.password` 与自助服务专用 `config.self_password`。由于校园网（Dr.COM/Portal）登录与自助服务系统在实际业务逻辑中共用同一套学号密码，双字段导致了用户需在两处分别输入、保存同一密码的冗余，且极易产生“自助密码、登录密码、账号档案密码”三态不一致的问题。

本决策删除了独立的自助密码字段，后端所有涉及自助服务的命令均通过“前端传入优先，空/MASK 占位符回退已存登录密码”的逻辑获取凭据。此设计消除了凭据不一致类 Bug，并收敛了敏感信息的存储与传输路径。

## Key Components

### 1. 桌面端 (Tauri Desktop)

**文件**: `tauri-app/src-tauri/src/config/model.rs`

*   **常量**:
    *   `pub const PASSWORD_MASK: &str = "***"` (`model.rs:3`): 密码掩码标准值。
*   **结构体 `Config`**:
    *   `pub password: String` (`model.rs:14`): 唯一的密码存储字段。
    *   `pub self_hello_enabled: bool` (`model.rs:20`): 是否启用 Windows Hello 操作验证（默认 `true`）。
    *   `pub self_reverify_each_action: bool` (`model.rs:25`): 自助服务面板每次操作都二次验证（默认 `false`）。
    *   *注*: 旧的 `self_password` 字段已完全移除。
*   **方法**:
    *   `pub fn masked_for_display(&self) -> Config` (`model.rs:321-326`): 出站掩码出口。克隆配置并调用 `mask_in_place`。
    *   `pub fn mask_in_place(&mut self)` (`model.rs:328-331`): 原地掩码。仅当 `self.password` 非空时替换为 `PASSWORD_MASK`。这是敏感信息出站的唯一出口。
    *   `impl Default for Config` (`model.rs:337-409`): 默认值初始化。`password` 默认为空字符串。

**文件**: `tauri-app/src-tauri/src/commands/self_service.rs`

*   **辅助函数**:
    *   `fn resolve_credential_password(state: &AppState, password: &str) -> Option<String>` (`self_service.rs:46-60`): **核心凭据解析逻辑**。
        *   逻辑：若前端传入的 `password` 非空且不为 `***`，直接返回；否则尝试从 `state.config.load_full().password` 获取已保存的登录密码；若两者皆无，返回 `None`。
    *   `fn ensure_identity_gate(state: &AppState) -> Option<String>` (`self_service.rs:15-23`): 身份验证门控。检查 `self_hello_enabled` 开关及近期验证状态。
    *   `fn is_iso_date(s: &str) -> bool` (`self_service.rs:31-40`): 严格 YYYY-MM-DD 日期校验。
    *   `fn resolve_campus_bind_addr(state: &AppState) -> Option<IpAddr>` (`self_service.rs:26-31`): 解析校园网适配器源 IP。
*   **Tauri 命令 (`#[tauri::command]`)**:
    *   `pub async fn bind_operator(...)` (`self_service.rs:64-103`): 绑定运营商账号。需通过 `ensure_identity_gate` 验证门。
    *   `pub async fn query_bind_status(...)` (`self_service.rs:106-136`): 查询各运营商绑定状态。**不设验证门**。
    *   `pub async fn query_self_dashboard(...)` (`self_service.rs:139-162`): 查询在线信息及上网记录。**不设验证门**。
    *   `pub async fn query_self_online_log(...)` (`self_service.rs:165-198`): 查询上网记录账单页数据。**不设验证门**。
    *   `pub async fn self_offline_session(...)` (`self_service.rs:201-230`): 注销指定在线会话。需通过 `ensure_identity_gate` 验证门。
    *   `pub async fn change_self_password(...)` (`self_service.rs:233-272`): 修改自助服务密码（即校园网登录密码）。需通过 `ensure_identity_gate` 验证门。新密码需满足 6-11 位 ASCII 字母数字。
    *   `pub async fn verify_windows_identity(...)` (`self_service.rs:275-296`): Windows Hello 本地身份验证。通过后记录后端验证时间戳。
    *   `pub async fn reveal_operator_credential(...)` (`self_service.rs:299-325`): 查看运营商明文凭据。**无论总开关如何，强制要求近期身份验证**（防 webview 绕过）。

### 2. 安卓端 (Tauri Android)

**文件**: `android/src-tauri/src/self_service_cmds.rs`

*   **辅助函数**:
    *   `fn resolve_self_password(incoming: &str, settings: &crate::config_state::Settings) -> Option<String>` (`self_service_cmds.rs:27-36`): **核心凭据解析逻辑**。与桌面 `resolve_credential_password` 同构：前端传非 MASK 明文优先，空/MASK 回退 `settings.password`，皆无返回 `None`。
    *   `fn ensure_identity_gate(settings: &crate::config_state::Settings) -> Option<String>` (`self_service_cmds.rs:39-47`): 验证门控。逻辑同桌面。
    *   `fn is_iso_date(s: &str) -> bool` (`self_service_cmds.rs:116-124`): 严格 YYYY-MM-DD 日期校验。
    *   `fn local_addr_of(...) -> Option<std::net::IpAddr>` (`self_service_cmds.rs:53-57`): 获取缓存的源 IP。
*   **Tauri 命令**:
    *   `pub async fn verify_biometric_identity(success: bool, method: String, ...)`: 生物识别验证。仅当 `success == true` 时解锁验证门（修复了原实现无 success 参数导致任何调用都能解锁的安全漏洞）。
    *   `pub async fn bind_operator(...)`: 绑定运营商账号。
    *   `pub async fn query_bind_status(...)`: 查询绑定状态。
    *   `pub async fn query_self_dashboard(...)`: 查询 Dashboard。
    *   `pub async fn query_self_online_log(...)`: 查询上网记录。
    *   `pub async fn self_offline_session(...)`: 注销会话。
    *   `pub async fn change_self_password(...)`: 修改密码。
    *   `pub async fn reveal_operator_credential(...)`: 揭示明文凭据（强制验证门）。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│                        Frontend (Vue/React)                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │ Onboarding   │  │ Self-Service │  │ Account Info / Login     │  │
│  │ Wizard       │  │ Panel        │  │ Form                     │  │
│  └──────┬───────┘  └──────┬───────┘  └──────────┬───────────────┘  │
│         │                 │                      │                  │
│         │  Sends Account  │  Sends Account +     │  Saves User/     │
│         │  + Password     │  Password (or empty) │  Password to     │
│         │  (masked if     │                      │  Config/Settings │
│         │   saved)        │                      │                  │
└─────────┼─────────────────┼──────────────────────┼──────────────────┘
          │                 │                      │
          ▼                 ▼                      ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       Tauri Backend (Rust)                          │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                   config_state.rs / model.rs                │   │
│  │  ┌───────────────────────────────────────────────────────┐  │   │
│  │  │  Settings / Config                                    │  │   │
│  │  │  - user: String                                       │  │   │
│  │  │  - password: String  ◄──► [Encrypted on Disk]         │  │   │
│  │  │  - self_hello_enabled: bool                           │  │   │
│  │  └───────────────────────────────────────────────────────┘  │   │
│  │                                                             │   │
│  │  ┌───────────────────────────────────────────────────────┐  │   │
│  │  │  Credential Resolution Logic                          │  │   │
│  │  │  resolve_credential_password() / resolve_self_password│  │   │
│  │  │  1. If incoming != "" && incoming != "***" → Use it   │  │   │
│  │  │  2. Else → Load from Config.password (Decrypted)      │  │   │
│  │  │  3. Else → Return None (Prompt User)                  │  │   │
│  │  └───────────────────────────────────────────────────────┘  │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │              self_service.rs / self_service_cmds.rs         │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │   │
│  │  │ Modify/Binds │  │  Reveal      │  │ Query Commands   │  │   │
│  │  │ (Offline,    │  │ (Credentials)│  │ (Status, Log,    │  │   │
│  │  │  Bind,       │  │              │  │  Dashboard)      │  │   │
│  │  │  ChangePwd)  │  │ Gate: Always │  │ Gate: Disabled   │  │   │
│  │  │ Gate: TTL    │  │ Checked      │  │                  │  │   │
│  │  └──────┬───────┘  └──────┬───────┘  └────────┬─────────┘  │   │
│  └─────────┼─────────────────┼────────────────────┼────────────┘   │
│            │                 │                    │                │
│            ▼                 ▼                    ▼                │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │              campus_login_lib::self_service                 │   │
│  │  HTTP Client → Campus Network APIs                          │   │
│  └─────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **凭据存储**:
    *   用户在“登录信息”或“新手引导”中输入学号和密码。
    *   桌面端：密码存入 `Config.password`，落盘时通过 DPAPI 加密（`persist.rs` 逻辑，虽未直接展示但文档提及）。
    *   安卓端：密码存入 `Settings.password`，落盘时通过 `CryptoBridge` 使用 AndroidKeyStore AES-GCM 加密，存入 `EncodedSettings.password_cipher`。
    *   **关键点**: 没有任何地方单独存储“自助服务密码”。

2.  **凭据获取 (Command Execution)**:
    *   前端发起自助服务请求（如绑定运营商、查询在线状态）。
    *   前端传递 `account` 和 `password`。如果用户已在该表单填写了新密码，则传递新密码；如果留空或显示为 `***`，则传递空串或掩码。
    *   后端收到请求后，调用 `resolve_credential_password` (桌面) 或 `resolve_self_password` (安卓)。
    *   **解析逻辑**:
        *   如果传入的 `password` 是有效明文 → 使用该明文。
        *   如果传入的是空串或 `***` → 从内存中的 `Config/Settings` 加载已解密的 `password` 字段。
        *   如果内存中也为空 → 返回 `None`，前端提示“请先在登录信息填写并保存密码”。

3.  **出站安全**:
    *   当配置需要发送给前端显示时，必须经过 `masked_for_display`。
    *   该函数会检查 `password` 字段，若非空则替换为 `***`。
    *   这确保了即使前端 store 缓存了配置，也不会泄露明文密码。

4.  **身份验证门控 (Identity Gate)**:
    *   对于写操作（绑定、注销、改密）和敏感读操作（揭示凭据），后端会检查 `ensure_identity_gate`。
    *   如果 `self_hello_enabled` 为 `true`，且距离上次 `verify_windows_identity` (桌面) 或 `verify_biometric_identity` (安卓) 成功超过 TTL (600s)，则拒绝请求。
    *   `reveal_operator_credential` 忽略总开关，始终要求近期验证。

## Connections

- [[page:config-mask-single-exit]] — 掩码出站唯一出口，`mask_in_place` 现仅 password 单字段。
- [[page:security-model]] — 凭据出站、落盘加密与验证门全景。
- [[page:config-and-persistence]] — 配置结构、三态语义与原子落盘。
- [[page:desktop-account-selfservice]] — 自助命令面与验证门交互时序。
- [[page:query-bind-status-no-gate-by-design]] — 查询命令不设门的有意设计。

## Known Issues

- **旧数据兼容**: 旧版本配置文件或账号档案中可能存在的 `selfPassword` / `self_password` 键值，由于双端 serde 均未设置 `deny_unknown_fields`，会在反序列化时被自动忽略。无需专门的迁移脚本，但用户如果之前独立设置了自助密码，该值将丢失，后续操作将回退到登录密码（或提示输入）。
- **密码一致性**: 如果用户曾经手动将“登录密码”和“自助密码”设置为不同值（虽然业务上它们应该相同），本决策实施后，自助服务系统将不再使用那个独立的“自助密码”，而是统一使用“登录密码”。这可能导致用户期望的行为与实际生效的凭据不符，除非用户重新同步两者。