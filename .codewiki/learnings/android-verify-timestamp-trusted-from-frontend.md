---
title: 安卓验证门的时间戳信任前端
type: learning
source_files:
  - android/src-tauri/src/self_service_cmds.rs
  - tauri-app/src-tauri/src/platform/identity.rs
tags: [教训, 安全, 安卓, 验证门, 生物识别]
---

## 现象

安卓端在 webview 内直接 `invoke('verify_biometric_identity')` 即可拿到 600s 验证窗口，无需真的通过生物识别。

## 根因

`verify_biometric_identity`（`android/src-tauri/src/self_service_cmds.rs:62-66`）**无条件写时间戳**，不校验生物识别是否真的通过。桌面端在同一位置是命令内亲自调 WinRT（`platform/identity.rs:78-117`），两端强度不对等。

## 解决

当前未修（作为已知强度差异记录）。

## 教训

① 安全门的时间戳必须由**执行验证的那一侧**写，不能提供"只写时间戳"的独立命令；② 评估门禁时看两端的实现强度是否对等，别以为同名命令等效。

## Connections

[[verification-gate-tiers]]、[[query-bind-status-no-gate-by-design]]、[[windows-hello-only-identity]]
