---
title: 适配器速度显示 18446744073 Gbps
type: learning
source_files:
  - tauri-app/src-tauri/src/network/discovery/windows.rs
tags: [教训, windows, 适配器, 哨兵值]
---

## 现象

UI 上适配器速度显示为 18446744073 Gbps。

## 根因

Windows 在**未连接**时 `LinkSpeed` 返回 `u64::MAX`（内部 -1 哨兵，即 2^64-1 换算后的数值）。

## 解决

发现层把该情况归 0，表示"未知"。

## 教训

Win32 返回的无符号字段可能携带 -1 哨兵；网络相关字段解析必须在发现层先做哨兵归一，不要把它透传到 UI。

## Connections

[[adapter-visibility-cache-staleness]]
