---
title: 登录"成功"与"可重试"的判定语义（中文文案与 code 字符串耦合）
type: learning
source_files:
  - tauri-app/src-tauri/src/auth/protocol.rs
  - tauri-app/src-tauri/src/auth/session.rs
tags: [教训, 协议, 登录, 重试, 文案耦合]
---

## 现象

① 大响应体错误会被**无谓重试** 3 次；② 若 Portal 改版后 `result=1` 语义变化且不带 msg，会把失败误报为成功。

## 根因

- `result == 1` 且 `msg` 为空被判定为登录成功（`protocol.rs:210-215`，空 msg 落到 `else` 返回 `"Portal协议认证成功"`）。
- 登录大响应体错误（`protocol.rs:138`、`:144` 返回 `Err("登录响应体过大")`）经 `do_login_with_retry` 的 `Err` 分支被包装为 `retryable: true`（`:178`），同一超大响应体会被重复请求 3 次。
- 另有大量**中文文案耦合**："已经在线"靠 `m.contains("已经在线")`（`session.rs:151`）判定；`AUTH_FAILURE_CODES`（`failure_tracker.rs:9` 的 `["ac_auth_failed", "1", "4"]`）必须与 `protocol.rs` 产出的 code 字符串一致，而 `"1"` 同时覆盖"非法/失败/错误/拒绝"四类语义。

## 解决

部分注释已说明"为消除跨文件文案耦合"做过调整（`session.rs:119-120`）；其余作为已知脆弱点保留。

## 教训

① 重试分类要按错误**种类**而不是语法上的 `Err` 分支决定（明确不可重试的错误不要标记 `retryable`）；② 与外部协议耦合的判定优先用结构化字段，中文文案匹配只能作兜底并显式标注；③ 新增/修改 code 字符串时同步检查 `AUTH_FAILURE_CODES` 等跨文件常量。

## Connections

[[remove-auth-layer-traits]]、[[portal-port-semantics]]
