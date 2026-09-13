---
title: 安卓 app 白屏：get_init_data 少字段，前端直接读不判空
type: learning
source_files:
  - android/src-tauri/src/system_cmds.rs
  - android/src-tauri/src/lib.rs
  - android/frontend/src/hooks/useInitialDataLoad.ts
tags: [教训, 安卓, 白屏, 双端同步, 初始化]
---

## 现象

安卓端启动后白屏。

## 根因

`useInitialDataLoad` 直接读 `get_init_data` 返回的字段、不判空，而安卓命令缺了桌面专属字段（`gpuInfo` / `adapters` 等）。

## 解决

安卓命令补桌面专属字段的空默认值：`android/src-tauri/src/system_cmds.rs:15-21`（`gpuInfo: null` :17、`refreshRate: 60` :18、`adapters` / `adapterDetails` / `disabledAdapters` 空数组 :19-21），命令注册在 `android/src-tauri/src/lib.rs:78`。前端消费侧对平台差异字段判空：`android/frontend/src/hooks/useInitialDataLoad.ts:60`（`initData.adapters || []`）。

2026-09-13 复核：两端 `get_init_data` 的键集已一致（各 13 个键：`config` / `accounts` / `version` / `autoLaunch` / `gpuInfo` / `refreshRate` / `adapters` / `adapterDetails` / `disabledAdapters` / `activeAccount` / `notificationEnabled` / `isAutoStart` / `backgroundStatus`）。

## 教训

**新增 `get_init_data` 字段必须两端同步**：安卓侧即使功能不可用，也要补空默认值，否则前端取到 `undefined` 直接白屏。前端读初始化数据时对平台差异字段要判空。

## Connections

[[android-version-json-camelcase-parse-drift]]、[[config-field-sets-bidirectional-sync]]
