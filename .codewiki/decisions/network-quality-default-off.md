---
title: 网络质量检测默认关闭（schema v3），质量页由后台检测状态代替
type: decision
source_files:
  - android/src-tauri/src/config_state.rs
  - android/frontend/src/settings/constants.ts
  - tauri-app/frontend/src/App.tsx
  - android/frontend/src/App.tsx
tags: [决策, 质量检测, 默认值, 安卓, 双端同语义]
---

## 背景

质量检测链路（12+ 外网目标 / 60s）是最大功耗主力；安卓端质量页在默认配置下占据底栏第 4 位。

## 决策

2026-09-12：`enable_network_quality` **默认 false**，并用 v2→v3 一次性迁移（存量刷 false，用户开回落盘不覆盖，沿用 v1→v2 先例）。前端行为与桌面同语义：

- 关闭即删除质量 tab，其余 tab 补位；
- 平板 DockNav 恢复按开关过滤 quality；
- 手机 BottomNav 第 4 位动态互换（质量开→"网络质量"，关→"后台检测/网络状态检测" MonitorPanel，`App.tsx` 补 monitor case）；
- 手机顶栏胶囊关闭态改显后台在线状态（原 `NetworkQualityCapsule` 恒显"未知"误导）、点击跳后台检测面板；
- 启动/循环链路仍受该开关门控。

## 理由

旧文档未记录（只写了关闭后各端 UI 的替代行为）。

## 备选方案

旧文档未记录。

## 影响与约束

**语义对照**：`enable_network_quality` = 质量链路总开关；`enable_latency_test` = 定时循环开关（默认本就 false）。

**改默认值必须走后端 `Settings::default` + schema 迁移（双源覆盖，只改前端无效）**——前端 `DEFAULT_CONFIG.configVersion` 也要与后端 `config_schema_version` 对齐（本轮由漂移值 2 同步到 4，`constants.ts:52`，见 [[android-interval-default-schema-migration]]）。副作用：质量开关是手机底栏结构的隐式开关，调整默认值需同时检查手机 tab 白名单（不含 `monitor`）与派生降级。

## Connections

[[android-interval-default-schema-migration]]、[[quality-detail-key-contract]]、[[tablet-layout-alignment-audit]]
