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

自助服务的查询路径（绑定状态 `query_bind_status`、Dashboard `query_self_dashboard`、上网记录 `query_self_online_log`）需要携带凭据向自助服务系统（Dr.COM Self）发起认证请求。在项目的通用安全模型中，验证门体系（桌面端 `ensure_identity_gate` 见 `tauri-app/src-tauri/src/commands/self_service.rs:15-23`、安卓端 `ensure_identity_gate` 见 `android/src-tauri/src/self_service_cmds.rs:39-48`）原则上要求改变外部状态的命令（如 `bind_operator`、`self_offline_session`、`change_self_password`）必须通过 Windows Hello / Android Biometric 验证。然而，查询类命令在设计上被明确排除在验证门外。

## 决策

自助服务查询类命令**不设验证门**。

同时，查询命令中的密码参数若为空或掩码占位符，会经由后端回退至已保存的登录密码：
- **桌面端**：由 `tauri-app/src-tauri/src/commands/self_service.rs:51-62` 的 `resolve_credential_password` 函数实现。
- **安卓端**：由 `android/src-tauri/src/self_service_cmds.rs:27-36` 的 `resolve_self_password` 函数实现。

两者逻辑同构，均遵循“前端传入非 MASK 明文优先；空/MASK 时回退 config.password”的语义（桌面端回退 `state.config.load_full().password`，见行56-60；安卓端回退 `settings.password`，见行31-34）。这确保了凭据统一后作为唯一凭据源，无需用户在每次查询时重新输入密码。

## 理由

1. **自动刷新体验需求**：总览卡等 UI 组件依赖免验证拉取数据以支持自动刷新。桌面端源码注释（`tauri-app/src-tauri/src/commands/self_service.rs:11-14`）明确指出：“查询类命令有意不设门——总览卡自动刷新依赖免验证拉取（数据本就存于本机，掩码属渲染层），补门会破坏该体验且不构成实际防线”。
2. **风险收益评估**：查询类命令返回的数据（如在线列表、掩码后的绑定状态，见 `tauri-app/src-tauri/src/commands/self_service.rs:133-141`）本身存于本机或由服务端按会话权限返回，掩码处理在渲染层完成，不构成额外的信息泄露防线缺口。因此，补门带来的安全增益不足以抵消对用户体验的破坏。
3. **跨端一致性**：安卓端命令面注释（`android/src-tauri/src/self_service_cmds.rs:1-3`）同样确认“查询类不设门”，与桌面端行为同构。

## 备选方案

旧文档未记录其他备选方案。当前设计为经过实测与代码审查后确定的最终方案，并在 `tauri-app/src-tauri/src/self_service/mod.rs:1-52` 的协议逆向注释与各端命令实现中固化。

## 影响与约束

1. **Webview 触发风险**：不设门意味着任何拥有 Webview 执行权限的脚本均可高频触发对自助服务系统的认证请求。由于查询命令每次都重新建立 HTTP 会话并登录（见 `tauri-app/src-tauri/src/self_service/mod.rs:352-407` 的 `login_session` 流程，以及 `login_and_fetch_bind_page` 行455-475），这可能被服务端视为异常行为。
2. **风控与锁定风险**：每次面板刷新或查询都会导致一次完整的登录往返（3 次请求：获取 checkcode 行360-367、预热 randomCode 行370-376、verify 登录行379-405）。如果前端频繁调用，存在触发服务端风控或锁定计数的风险。代码层通过 `login_session_with_retry`（行429-451）对传输层错误进行最多 3 次重试，但不缓解业务层高频触发。
3. **凭据暴露面**：虽然密码不直接输出给前端（查询绑定状态仅返回掩码账号与是否设密，见行133-141），但查询过程中密码（或其 MD5 值，见行383）会在内存中传递并用于 HTTP 请求。由于没有验证门拦截，恶意脚本可以轻易利用已保存的登录密码发起这些请求。
4. **敏感操作界限**：明文揭示凭据 `reveal_operator_credential` 仍强制验证门（桌面端行349检查 `identity_verified_recently()`，安卓端行314），与查询类命令形成明确隔离。

## Connections

- `[[verification-gate-tiers]]`：定义了验证门的分级保护策略，本文查询命令为明确例外。
- `[[credential-single-source]]`：确立了登录密码作为唯一凭据源的架构原则，查询回退逻辑依赖此设计。
- `[[android-verify-timestamp-trusted-from-frontend]]`：涉及安卓端生物识别验证的时间戳信任机制（注：查询命令不受此门限制，但 `reveal_operator_credential` 等敏感命令受其限制，见 `android/src-tauri/src/self_service_cmds.rs:301-328`）。
