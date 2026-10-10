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

自助服务的查询路径需要带密码向自助服务系统认证，而验证门体系（见 [[verification-gate-tiers]]）原则上保护敏感操作。

## 决策

自助服务查询类命令**不设验证门**，且密码经 `resolve_credential_password`（桌面；安卓同名 `resolve_self_password`）**回退登录密码**（桌面与安卓同设计；凭据统一后唯一凭据源见 [[credential-single-source]]）。

## 理由

桌面源码注释明确标注此为**有意为之**（`tauri-app/src-tauri/src/commands/self_service.rs:10-13`）。

## 备选方案

旧文档未记录。

## 影响与约束

不设门意味着：当用户已保存登录密码时，**任意 webview 脚本可高频触发对自助服务系统的认证**（`android/src-tauri/src/self_service_cmds.rs:115-146`）。另有风控风险：每条命令都重新登录一次，一次面板刷新 = N ×（3 次登录往返 + 1~2 次业务请求），每次都要提交一次 MD5 密码，存在被服务端风控/锁定计数的风险。

## Connections

[[verification-gate-tiers]]、[[android-verify-timestamp-trusted-from-frontend]]
