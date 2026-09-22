---
title: 注销协议改为 Radius 注销先行、成功即止，MAC 解绑收尾
type: decision
source_files:
  - tauri-app/src-tauri/src/auth/protocol.rs
tags: [决策, 注销, 协议, radius, mac]
---

## 背景

对"注销后打不开登录页"的现象，曾怀疑需要多发注销请求 / 先解绑 MAC。

## 决策

2026-09-11：注销改为 **Radius 注销先行、成功即止，MAC 解绑收尾**。即先 logout、后 unbind，保持防御性正确。

## 理由

实测证实 **unbind 不踢在线会话、连发无增益**；"注销后打不开登录页"属间歇性服务端/环境异常（23 次注销 0 僵尸）。主流实现（cqu-net-auth / eptools / Meirs）均"一次到位绝不连发"。

## 备选方案

MAC 解绑先行 / 连发多轮注销——因实测无增益被弃用。

## 影响与约束

注销调用点不应再加"连发补偿"。已知脆弱点：注销请求携带硬编码占位凭据（`protocol.rs:3-4` 的 `drcom`/`123`），只依赖 `wlan_user_ip`，Portal 收紧校验会静默退化；注销失败结果缺 `retryable` 字段，`unwrap_or(true)` 兜底导致恒可重试。

**2026-09-22 复核增注**：①"缺 `retryable`"指 `do_logout_with_retry` 的 `Err` 分支与 `max_retries` 兜底 JSON——`do_logout_request` 的正常产出自 2026-09 起已带 `retryable`，该表述仍然准确；②结果 JSON 新增 `radiusOk`/`unbindOk` 两个子步骤信号（`protocol.rs:379-380`），"离线已生效"判据取 `radiusOk || unbindOk`（MAC 解绑同样是破坏性踢下线），不改变本决策"Radius 先行、成功即止"的顺序语义；③`do_logout_with_retry` 只保留最后一次结果，交替型失败场景的 `unbindOk` 可能失真——见 [[night-outbound-switch]] 已知限制。

## Connections

[[portal-port-semantics]]、[[logout-placeholder-credentials]]、[[night-outbound-switch]]
