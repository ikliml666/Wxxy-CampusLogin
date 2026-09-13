---
title: 敏感信息出站唯一出口（统一走 masked_for_display）
type: decision
source_files:
  - tauri-app/src-tauri/src/config/model.rs
  - android/src-tauri/src/config_state.rs
tags: [决策, 安全, 配置, 脱敏]
---

## 背景

配置对象里有账号密码等敏感字段，而它会被多处发往前端；逐处手工打码必然漏项，一处漏掉就是明文出站。

## 决策

一切把 `Config` 发往前端的路径**必经** `Config::masked_for_display()`（桌面）/ `config_state::masked_for_display`（安卓），**禁止手工逐字段打码**。日志/错误/事件 payload 一律不得携带 password。

密码语义两端同构：空串或 MASK 表示"未修改，回退已存值"，显式清除走 `clear` 标志。

## 理由

旧文档未展开理由，只给出机制性依据：漏一处即明文出站，因此有回归单测锁死该出口。

## 备选方案

旧文档未记录。

## 影响与约束

新增任何向前端返回 Config 的命令都必须走该函数。已知漏洞面：`Config::mask_in_place` 是 `pub`（`model.rs:223`），任何调用方都能就地破坏内存中的明文配置对象，类型系统不阻止误用（`masked_for_display` 走 clone 路径是安全的）。

## Connections

[[mask-placeholder-persisted-as-plaintext]]、[[log-redaction-coverage]]、[[android-identifier-change-breaks-keystore]]
