---
title: 日志与错误信息没有统一脱敏钩子
type: learning
source_files:
  - tauri-app/src-tauri/src/infra/logger.rs
  - tauri-app/src-tauri/src/auth/portal.rs
tags: [教训, 安全, 日志, 脱敏]
---

## 现象

`redact_credentials` 只覆盖**登录请求一条路径**（唯一调用点 `auth/protocol.rs:129`），其余路径可能把请求细节原样带出去：Portal 探测把 reqwest 的 `Display`（可能含完整请求 URL）写进日志与 `login-log` 事件（`monitor/portal_check.rs:56/74`），自助服务把同类错误串原样塞进 `CommandResult.message` 返回前端（`self_service/mod.rs:179/186/308` 等十余处 `format!("...: {e}")`）。

## 根因

日志层零过滤：`infra/logger.rs:304` 的 `log(level, module, message)` 与四个宏对内容不做处理，只有调用方纪律。

## 解决

无统一钩子（靠调用方纪律）。

## 教训

① 新增日志语句**不得**直接打 `config.password`（不会有任何拦截）；② 错误信息出口（日志 / 事件 payload / `CommandResult.message`）都要当作可能的泄露面；③ 新增涉及凭据的请求路径时，复用 `redact_credentials` 而不是自己拼错误串模板。

## Connections

[[config-mask-single-exit]]、[[mask-placeholder-persisted-as-plaintext]]
