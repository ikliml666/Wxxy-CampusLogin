---
title: 适配器操作范围只作用于主/副适配器（UI 展示遍历全部）
type: decision
source_files:
  - tauri-app/src-tauri/src/network/adapter.rs
  - tauri-app/frontend/src/network/adapters.ts
tags: [决策, 网络, 适配器, 双端同源]
---

## 背景

一台机器上可能有多张网卡（虚拟网卡、代理 TUN、蓝牙等），若对所有网卡执行检测/登录/注销/DNS 设置/DHCP，会误伤无关网卡。

## 决策

- 操作类流程（检测/登录/注销/DNS 设置/DHCP）**只作用于** `resolve_adapter_names` 解析出的主/副适配器；
- UI 展示类**遍历全部**。

前端 `network/adapters.ts::resolveAdapterNames` 与后端**同源规则**，改任一侧必须同步另一侧（`adapters.test.ts` 锁行为）。

## 理由

旧文档未记录。

## 备选方案

旧文档未记录。

## 影响与约束

改适配器解析规则必须两端同步，且靠 `adapters.test.ts` 锁行为。注意解析结果在安卓恒为空（`adapters` 恒为 `[]`），消费方需容忍。

## Connections

[[protocol-core-single-source]]、[[adapter-visibility-cache-staleness]]
