---
title: 安卓后台检测间隔默认 15s→60s + config_schema_version 迁移机制
type: decision
source_files:
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/frontend/src/settings/constants.ts
tags: [决策, 安卓, 配置, 默认值, 迁移]
---

## 背景

安卓稳态周期任务按 15s 运行属空转耗电。

## 决策

2026-09-09：后台检测间隔默认值 **15s → 60s**，并引入 `config_schema_version` 做**一次性默认值迁移**。

迁移语义：迁移落盘后用户显式设回**不再覆盖**；后续默认值变更沿用该机制。

2026-09-13：schema **v3→v4** —— 新增**闲时巡检间隔** `background_check_idle_interval`（默认 300_000；旧文件缺该字段反序列化为 0，迁移里补 300_000，`config_state.rs:270-274`）。同时间隔改为**按网络类型/屏幕状态分档**：亮屏 + WiFi 走 `background_check_interval`（60s），蜂窝或灭屏走闲时间隔（5min，闲时间隔小于基础间隔时取基础间隔）——判定为纯函数 `effective_interval_ms`（`monitor_loop.rs:61`），经 `getPowerState` 查询式驱动。新装即 v4，跳过迁移。

同日追加 schema **v4→v5**：**检测时段终点** `campus_check_end_minutes` 旧默认 `0`（仅开始时间限制）→ `1380`（23:00，`config_state.rs:119-121`），迁移把存量等于旧默认 0 的值一并刷为新默认（`config_state.rs:277-281`，测试 `config_state.rs:528` 断言迁移生效、`:539` 断言迁移后用户显式设回 0 不被二次覆盖）。**桌面侧同日做了同义迁移**（`config_version` v2→v3）：`Config::default` 与 serde `default = "default_campus_check_end_minutes"` 均改 1380（`tauri-app/src-tauri/src/config/model.rs:142-144`、`:213`、`:220`），`validate_config` 内 `config_version < 3` 时把等于旧默认 0 的终点刷为 1380（`tauri-app/src-tauri/src/config/validate.rs:133-140`，测试 `validate.rs:536-549`）——桌面此前没有版本化默认值迁移机制，这次是第一次补第二段（v1→v2 之外），语义与安卓 schema 迁移对齐但编号体系独立（桌面 `config_version` 3 ≠ 安卓 `config_schema_version` 5，两套编号不可互推）。

## 理由

稳态周期任务 15s 空转耗电，需要降频；直接改 `Settings::default` 对存量用户无效，因此引入版本化迁移。

v4 追加分档的理由：蜂窝环境探针必失败、灭屏时用户不在看状态，拉长间隔不改正确性（WiFi 事件仍即时触发检测，无明确离线证据时在线状态保持上一拍记忆）。

## 备选方案

旧文档未记录（只记录机制的演进方向）。

## 影响与约束

改默认值必须走后端 `Settings::default` **+** schema 迁移双源覆盖——只改前端无效；前端 `DEFAULT_CONFIG.configVersion` 须与后端 `config_schema_version` 同步（v3→v4 轮由漂移值 2 对齐到 4；v4→v5 轮现为 5，`constants.ts:55`，闲时间隔前端常量 `constants.ts:26`）。已知不一致：桌面 `background_check_interval` 默认 15000（`config/model.rs`），安卓 60000（`config_state.rs:92`）；桌面 Config 无闲时间隔字段——分档是安卓省电专属，不参与双端字段同步。桌面 `config_version`（现 3）与安卓 `config_schema_version`（现 5）仍是两套独立编号，见 [[config-field-sets-bidirectional-sync]]。

## Connections

[[network-quality-default-off]]、[[config-field-sets-bidirectional-sync]]、[[android-power-three-fixes]]
