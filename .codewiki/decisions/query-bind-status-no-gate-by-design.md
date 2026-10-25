---
title: 自助服务查询命令的"无验证门 + 密码逐个回退"是有意设计
type: decision
source_files:
  - tauri-app/src-tauri/src/commands/self_service.rs
  - tauri-app/src-tauri/src/self_service/mod.rs
  - android/src-tauri/src/self_service_cmds.rs
tags: [决策, 安全, 自助服务, 验证门]
---

## 背景

自助服务的查询路径（绑定状态 `query_bind_status`、Dashboard `query_self_dashboard`、上网记录 `query_self_online_log`）需要携带凭据向自助服务系统（Dr.COM Self）发起认证请求。在项目的通用安全模型中，验证门体系原则上要求改变外部状态的命令必须通过本地身份验证：

- 桌面端 `ensure_identity_gate`（`tauri-app/src-tauri/src/commands/self_service.rs:15-23`）：当 `self_hello_enabled` 开启且 `identity_verified_recently()` 为否时返回拦截文案；被 `bind_operator`（行83-85）、`self_offline_session`（行236-238）、`change_self_password`（行277-279）调用。
- 安卓端 `ensure_identity_gate`（`android/src-tauri/src/self_service_cmds.rs:42-51`）：同构逻辑，被 `bind_operator`（行106-108）、`self_offline_session`（行254-256）、`change_self_password`（行289-291）调用。

然而，查询类命令在设计上被明确排除在验证门外。

### 查询命令调用架构

```text
┌─────────────────────────┐
│ Webview 查询触发         │
└───────────┬─────────────┘
            ▼
┌─────────────────────────┐
│ query_bind_status        │
│ self_service.rs:117-151  │
│ (无验证门)                │
└───────────┬─────────────┘
            │ resolve_credential_password (行51-62)
            ▼
┌─────────────────────────┐
│ resolve_self_service_    │
│ channel (VPN/直连路由)    │
└───────────┬─────────────┘
            │ query_bind_status_via (mod.rs:297-309)
            ▼
┌─────────────────────────┐
│ login_and_fetch_bind_page│
│ mod.rs:562-582           │
│   └ login_session_       │
│     with_retry:534-558   │
│       └ login_session    │
│         :453-510          │
└───────────┬─────────────┘
            │ extract_fld_values + binding_from
            ▼
┌─────────────────────────┐
│ 掩码 JSON 返回前端       │
└─────────────────────────┘
```

## 决策

自助服务查询类命令**不设验证门**。具体命令与核心组件包括：

- **桌面端查询命令**：
  - `query_bind_status`（`tauri-app/src-tauri/src/commands/self_service.rs:140-174`）：参数 `account`/`password`，无 `ensure_identity_gate` 调用；仅做 `account` 空校验（行124-126）与密码回退（行127-129），返回 `cmcc`/`telecom`/`unicom` 掩码 JSON（行135-143）。
  - `query_self_dashboard`（行156-182）：无验证门，密码回退（行165-167），透传在线列表与记录。
  - `query_self_online_log`（行187-220）：无验证门，增加 `is_iso_date` 日期校验（行203，函数定义行37-45）。
- **安卓端查询命令**：
  - `query_bind_status`（`android/src-tauri/src/self_service_cmds.rs:135-166`）：无验证门，密码回退（行146-148）。
  - `query_self_dashboard`（行169-193）：无验证门。
  - `query_self_online_log`（行211-239）：无验证门，含 `is_iso_date`（行196-208）。
- **密码回退函数（凭据统一语义）**：
  - 桌面端 `resolve_credential_password`（`tauri-app/src-tauri/src/commands/self_service.rs:51-62`）：前端非 MASK 明文优先；空/MASK 时回退 `state.config.load_full().password`（行56-60）。
  - 安卓端 `resolve_self_password`（`android/src-tauri/src/self_service_cmds.rs:30-39`）：同构，回退 `settings.password`（行31-37）。
- **协议层结构体**：
  - `OperatorBinding`（`tauri-app/src-tauri/src/self_service/mod.rs:280-285`）：字段 `masked_account: String`（掩码账号）、`password_set: bool`（密码是否设置）；由 `binding_from`（行441-449）基于 `extract_fld_values` 构建，供 `query_bind_status_via`（行297-309）返回 `[Option<OperatorBinding>; 3]`。

## 理由

1. **自动刷新体验需求**：总览卡等 UI 组件依赖免验证拉取数据以支持自动刷新。桌面端源码注释（`tauri-app/src-tauri/src/commands/self_service.rs:11-14`）明确指出：“查询类命令有意不设门——总览卡自动刷新依赖免验证拉取（数据本就存于本机，掩码属渲染层），补门会破坏该体验且不构成实际防线”。
2. **风险收益评估**：查询类命令返回的数据（如在线列表、掩码后的绑定状态，见桌面端 `tauri-app/src-tauri/src/commands/self_service.rs:135-143` 与安卓端 `android/src-tauri/src/self_service_cmds.rs:153-161`）本身存于本机或由服务端按会话权限返回，掩码处理在协议层完成（`mask_account` 见 `tauri-app/src-tauri/src/self_service/mod.rs:424-439`，`binding_from` 行441-449），不构成额外的信息泄露防线缺口。因此，补门带来的安全增益不足以抵消对用户体验的破坏。
3. **跨端一致性**：安卓端命令面注释（`android/src-tauri/src/self_service_cmds.rs:1-3`）同样确认“查询类不设门”，与桌面端行为同构。

## 备选方案

当前设计为经过实测与代码审查后确定的最终方案，并在协议逆向注释与各端命令实现中固化。查询绑定状态协议实现见 `query_bind_status_via`（`tauri-app/src-tauri/src/self_service/mod.rs:280-292`），其登录流程复用 `login_and_fetch_bind_page`（行562-582）与 `login_session_with_retry`（行534-558），底层 `login_session`（行453-510）执行获取 checkcode（行459-467）、预热 randomCode（行470-477）、verify 登录（行480-508）。

## 影响与约束

1. **Webview 触发风险**：不设门意味着任何拥有 Webview 执行权限的脚本均可高频触发对自助服务系统的认证请求。由于查询命令每次都重新建立 HTTP 会话并登录（见 `tauri-app/src-tauri/src/self_service/mod.rs:562-582` 的 `login_and_fetch_bind_page`，其调用 `login_session_with_retry` 行534-558，进而 `login_session` 行453-510），这可能被服务端视为异常行为。
2. **风控与锁定风险**：每次面板刷新或查询都会导致一次完整的登录往返（3 次请求：获取 checkcode 行459-467、预热 randomCode 行470-477、verify 登录行480-508）。如果前端频繁调用，存在触发服务端风控或锁定计数的风险。代码层通过 `login_session_with_retry`（行534-558）对传输层错误进行最多 3 次重试，但不缓解业务层高频触发。
3. **凭据暴露面**：虽然密码不直接输出给前端（查询绑定状态仅返回掩码账号与是否设密，见桌面端行135-143、安卓端行153-161），但查询过程中密码（或其 MD5 值，见 `tauri-app/src-tauri/src/self_service/mod.rs:485` 的 `md5_hex(password)`）会在内存中传递并用于 HTTP 请求。由于没有验证门拦截，恶意脚本可以轻易利用已保存的登录密码发起这些请求。
4. **敏感操作界限**：明文揭示凭据 `reveal_operator_credential` 仍强制验证门（桌面端 `tauri-app/src-tauri/src/commands/self_service.rs:355-359` 检查 `identity_verified_recently()`，安卓端 `android/src-tauri/src/self_service_cmds.rs:329-331`），与查询类命令形成明确隔离。注意 reveal 路径不受总开关 `self_hello_enabled` 影响（桌面端 `reveal_operator_credential` 行341-371 未调用 `ensure_identity_gate` 故跳过开关判断；安卓端同理行316-345），属于刻意设计。

## Connections

- `[[verification-gate-tiers]]`：定义了验证门的分级保护策略，本文查询命令为明确例外（桌面端 `ensure_identity_gate` 仅用于变更状态命令，见 `tauri-app/src-tauri/src/commands/self_service.rs:15-23,83-85,236-238,277-279`；安卓端见 `android/src-tauri/src/self_service_cmds.rs:42-51,106-108,254-256,289-291`）。
- `[[credential-single-source]]`：确立了登录密码作为唯一凭据源的架构原则，查询回退逻辑依赖此设计（桌面 `resolve_credential_password` 行51-62，安卓 `resolve_self_password` 行30-39）。
- `[[android-verify-timestamp-trusted-from-frontend]]`：涉及安卓端生物识别验证的时间戳信任机制（注：查询命令不受此门限制，但 `reveal_operator_credential` 等敏感命令受其限制，见 `android/src-tauri/src/self_service_cmds.rs:316-345`，其中验证检查位于行329-331）。