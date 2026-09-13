---
title: 删除 auth 层 trait 抽象（AdapterResolver/PortalChecker/ProtocolClient）
type: decision
source_files:
  - tauri-app/src-tauri/src/auth/mod.rs
tags: [决策, 重构, 抽象, 测试]
---

## 背景

早期为 auth 层设计了 trait 抽象（AdapterResolver / PortalChecker / ProtocolClient），配套 mock 实现。

## 决策

（早期重构）**删除 auth 层 trait 抽象**：直接调自由函数，测试用真函数。

## 理由

单实现 trait + mock 属**无意义抽象**。

## 备选方案

保留 trait + mock 以便测试替换——因"单实现"这一事实被判定为无收益，弃用。

## 影响与约束

不要在只有单一实现的场景重新引入 trait 层；测试直接调用真函数。

## Connections

[[login-response-retry-semantics]]
