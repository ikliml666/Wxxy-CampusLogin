---
title: 验证门分级（改变外部状态的命令才设门）
type: decision
source_files:
  - tauri-app/src-tauri/src/platform/identity.rs
  - android/src-tauri/src/identity_gate.rs
  - tauri-app/frontend/src/account/selfServiceState.ts
tags: [决策, 安全, 验证门, 生物识别]
---

## 背景

若所有命令都要身份验证，总览卡自动刷新等只读查询会被验证打断；若都不设门，注销在线设备这类破坏性操作无保护。

## 决策

- 改变外部状态的命令设门（`bind_operator` / `self_offline_session`）；
- 只读查询有意**不设门**（总览卡自动刷新依赖免验证拉取）；
- **明文查看无论开关强制验证**。

TTL：前端门 570s + 后端 600s 复核——**后端 TTL 才是真防线**。

## 理由

旧文档只写了"只读查询不设门是依赖免验证拉取"这一因果；其余权衡未记录。

## 备选方案

旧文档未记录。

## 影响与约束

命令分类时先判断"是否改变外部状态"。已知实现强度不一致：安卓 `verify_biometric_identity`（`android/src-tauri/src/self_service_cmds.rs:62-66`）无条件写时间戳、不校验生物识别是否真的通过，webview 直接 `invoke` 即可拿到 600s 窗口；桌面端在同一位置由命令内亲自调 WinRT（`platform/identity.rs:78-117`）。另有 `self_reverify_each_action` 字段后端从未读取，纯前端编排。

## Connections

[[windows-hello-only-identity]]、[[android-verify-timestamp-trusted-from-frontend]]、[[query-bind-status-no-gate-by-design]]
