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

一切把配置发往前端的路径**必经** `Config::masked_for_display()`（桌面）/ `config_state::masked_for_display`（安卓），**禁止手工逐字段打码**。日志/错误/事件 payload 一律不得携带 password。

密码语义两端同构：空串或 MASK 表示"未修改，回退已存值"，显式清除走 `clear` 标志。

## 理由

旧文档未展开理由，只给出机制性依据：漏一处即明文出站，因此有回归单测锁死该出口。

## 备选方案

旧文档未记录。

## 影响与约束

### 桌面端（tauri-app/src-tauri/src/config/model.rs）

- `Config::masked_for_display(&self) -> Config`（line 317）：clone + mask，安全路径
- `Config::mask_in_place(&mut self)`（line 323）：就地破坏明文，`pub` 可见——任何调用方都能破坏内存中的明文配置对象，类型系统不阻止误用；`masked_for_display` 走 clone 路径是安全的
- 掩码逻辑：`password` 和 `self_password` 非空时替换为 `PASSWORD_MASK`（`"***"`，line 3）
- `Config` 结构体含 40+ 字段（line 10-173），其中 `password`（line 12-13）和 `self_password`（line 14-16）为敏感字段

### 安卓端（android/src-tauri/src/config_state.rs）

- `masked_for_display(s: &Settings) -> serde_json::Value`（line 373）：返回 JSON Value 而非 Settings clone，内部同样做 mask 后序列化
- `emit_config_changed(app, s)`（line 388）：广播 `config-changed` 事件，payload 为 `{ "config": masked_for_display(s) }`，前端依赖该事件同步 store
- `resolve_password_field(incoming, current, clear)`（line 394）：空串/MASK 视为"未修改"回退已存值，clear=true 则置空
- `current_settings(app)`（line 457）：缓存优先读取，供登录/自助服务等命令回退已存凭据
- `Settings` 结构体含 30+ 字段（line 15-111），`password`（line 18）和 `self_password`（line 19）为敏感字段
- 密码落盘经 AndroidKeyStore AES-GCM 加密（替代桌面 DPAPI），磁盘形态由 `EncodedSettings`（line 228）承载

### 已知漏洞面

`Config::mask_in_place` 是 `pub`（model.rs:323），任何调用方都能就地破坏内存中的明文配置对象，类型系统不阻止误用（`masked_for_display` 走 clone 路径是安全的）。

## Connections

[[mask-placeholder-persisted-as-plaintext]]、[[log-redaction-coverage]]、[[android-identifier-change-breaks-keystore]]
