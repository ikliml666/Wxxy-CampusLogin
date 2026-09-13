---
title: 注销请求携带硬编码占位凭据
type: learning
source_files:
  - tauri-app/src-tauri/src/auth/protocol.rs
tags: [教训, 协议, 注销, 脆弱性]
---

## 现象

注销不校验身份凭据，只依赖 `wlan_user_ip`。

## 根因

`protocol.rs:3-4` 定义 `LOGOUT_PLACEHOLDER_ACCOUNT = "drcom"`、`LOGOUT_PLACEHOLDER_PASSWORD = "123"`，`do_logout_request` 在 `protocol.rs:273-274` 把它们填入 `user_account` / `user_password`。

## 解决

当前属设计现状（协议侧不校验）。

## 教训

① 注销链路的正确性建立在一个**外部未校验**的假设上，若 Portal 侧收紧校验会**静默退化**，做协议变更评估时要把它列为风险点；② 相关脆弱点一并注意：注销失败结果缺 `retryable` 字段（`unwrap_or(true)` 兜底 → 恒可重试）、注销轮次间隔只覆盖第 1 轮。

## Connections

[[logout-radius-first]]
