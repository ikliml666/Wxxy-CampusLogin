---
title: 身份验证仅用 Windows Hello，删除 CredUI/SSPI 回退
type: decision
source_files:
  - tauri-app/src-tauri/src/platform/identity.rs
tags: [决策, 安全, 身份验证, windows-hello]
---

## 背景

身份验证原本还有输密码回退（CredUI/SSPI）。

## 决策

2026-09-05：身份验证**仅 Windows Hello**，删除 CredUI/SSPI 回退；设备未配置时返回**引导文案**而非降级到密码框。

## 理由

用户明确只要 Hello。

## 备选方案

CredUI/SSPI 密码回退——按用户要求删除（`platform/identity.rs:1-5` 注释记载 2026-09-05 用户要求移除输密码回退）。

## 影响与约束

未配置 Hello 的用户**无法执行 reveal / bind 等敏感操作**，这是设计后果而非缺陷。安全提示：错误信息不得暗示可降级。

## Connections

[[verification-gate-tiers]]、[[windows-hello-gate-module-singleton]]
