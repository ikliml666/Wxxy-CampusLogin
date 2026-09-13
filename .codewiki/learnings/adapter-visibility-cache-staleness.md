---
title: 适配器可见性/禁用分类缓存陈旧（只在 enable_adapter 刷新）
type: learning
source_files:
  - tauri-app/src-tauri/src/network/discovery/registry.rs
  - tauri-app/src-tauri/src/monitor/adapter_watch.rs
tags: [教训, 缓存, 适配器, 时效性, 历史缺陷]
---

## 现象

运行期在设备管理器禁用/拔插网卡后，适配器的**可见性与禁用分类永久陈旧**（列表不反映真实状态）。

## 根因

`CLASS_SUBKEY_CACHE` 不会自动过期：`ensure_cache_initialized` 只在首次访问构建，重建只由 `refresh_class_subkey_cache` 触发，而当时**唯一调用点是 `enable_adapter`**（启用网卡才刷新）。

## 解决

补 15s 周期刷新（`monitor/adapter_watch.rs:43-45`，已包 `spawn_blocking`），并在注释里记录该历史缺陷（`monitor/adapter_watch.rs:37-42`）。

## 教训

设备状态类缓存必须有**周期性刷新**，不能只挂在"某个操作路径"上——只靠操作触发时，用户在系统侧做的变更永远不会进入缓存。新增此类缓存时同时确定：TTL、刷新驱动者、以及失败时的行为。

## Connections

[[adapter-linkspeed-u64-max]]、[[adapter-operation-scope]]
