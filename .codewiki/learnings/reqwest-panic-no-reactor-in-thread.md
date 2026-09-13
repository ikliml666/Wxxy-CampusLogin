---
title: 子线程 reqwest panic "there is no reactor running"
type: learning
source_files:
  - tauri-app/src-tauri/src/infra/async_util.rs
  - tauri-app/src-tauri/src/network/client.rs
tags: [教训, rust, async, tokio, 协议]
---

## 现象

裸子线程里发起 reqwest 请求直接 panic：`there is no reactor running`。

## 根因

裸子线程没有 Tokio reactor 上下文。

## 解决

同步桥接统一走 `block_on_http` / `spawn_blocking`。

## 教训

**禁止在 async 上下文直接调用同步协议函数**；同步协议函数必须在阻塞线程（`spawn_blocking`）内调用，异步上下文走 `block_on_*` 封装。相关约束：`block_on_sync` 在 async worker 线程上会 panic/死锁。

## Connections

[[android-gateway-icmp-unavailable]]、[[remove-auth-layer-traits]]
