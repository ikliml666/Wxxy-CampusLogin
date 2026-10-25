---
title: deliberate-background-check-skips
type: module
source_files:
  - tauri-app/src-tauri/src/monitor/background_check.rs
  - tauri-app/src-tauri/src/monitor/campus_check.rs
  - tauri-app/src-tauri/src/monitor/adapter_watch.rs
tags: [monitor, background-check, decisions, tauri]
---

## Overview

本模块涉及在 Tauri 应用中监控（monitor）相关的逻辑，特别是与背景检查（background check）跳过（skips）相关的决策。源码文件 `tauri-app/src-tauri/src/monitor/background_check.rs`、`tauri-app/src-tauri/src/monitor/campus_check.rs` 与 `tauri-app/src-tauri/src/monitor/adapter_watch.rs` 均存在且已成功读取。核心逻辑是在特定条件下（退出/取消/并发检测/适配器获取失败/出站切换态等）主动跳过整轮后台巡检，或在校园网检测静默期内跳过校园网环境验证；同时在适配器从禁用恢复时仅触发一次性检查而非重启常驻后台巡检。

## Key Components

基于实际源码提取的关键函数与结构体（行号以源码 `行号| ` 前缀为准）：

- `tauri-app/src-tauri/src/monitor/background_check.rs`
  - `should_reset_logged_online` (行 25)：纯函数，判定是否需要复位「已登录过」标记（可达且主副均不在线且曾登录）。
  - `run_background_check_blocking` (行 29)：同步执行后台检测，返回 `CheckOutcome`；在退出/取消令牌触发(行 30-32)、已有检测在跑(行 33-35)、适配器列表获取失败(行 45-50)、出站切换态(行 63-70)、取消(行 165-166)等场景返回 `CheckOutcome::Skipped`。
  - `trigger_fast_quality_check` (行 436)：门户不可达快排查联动质量检测。
  - `run_background_check` (行 510)：async 包装，通过 `spawn_blocking` 调用上述阻塞函数。
- `tauri-app/src-tauri/src/monitor/campus_check.rs`
  - `ConnectionCampusStatus` (行 7) 与 `CampusCheckResult` (行 15) 结构体。
  - `adapter_campus_status` (行 23) 与 `adapter_campus_message` (行 30)：按适配器名提取校园网状态消息。
  - `check_campus_network` (行 44)：校园网检测主函数，依据名称/网关/portal 可达性判定。
  - `is_campus_check_silent` (行 306)：静默期判定；在 `background_check.rs` 行 71 处若处于静默期则跳过校园网环境验证（仍继续执行后续 Portal 检测）。
- `tauri-app/src-tauri/src/monitor/adapter_watch.rs`
  - `ADAPTER_WATCH_INTERVAL` (行 9，15000ms) 与 `CLASS_SUBKEY_REFRESH_ROUNDS` (行 15，4轮) 常量。
  - `start_adapter_watch` (行 17)：监听适配器变化；当适配器从禁用恢复时调用 `crate::monitor::background_task::spawn_background_check_once` (行 115) 仅触发一次性检查，避免重启用户已停止的后台巡检。
  - `auto_enable_backoff_ms` (行 289)：自动启用退避纯函数（0/60s/120s/300s 封顶）。

## Architecture

实际组件框图如下（基于源码归属）：

```text
┌────────────────────────┐
│ adapter_watch.rs       │
│ (start_adapter_watch)  │
└───────────┬───────────┘
            │ 适配器恢复时调用 spawn_background_check_once
┌───────────▼───────────┐
│ background_check.rs   │
│ (run_background_check│
│  _blocking)          │
└───────────┬───────────┘
            │ 调用
┌───────────▼───────────┐
│ campus_check.rs       │
│ (check_campus_network│
│  /is_campus_check_    │
│   silent)            │
└───────────────────────┘
```

## Data Flow

真实数据流：
1. `adapter_watch` 监听适配器集合/禁用状态变化，恢复时触发一次性后台检查（`background_task::spawn_background_check_once`）。
2. `background_check::run_background_check_blocking` 先判断是否退出、并发信号量获取失败、适配器获取失败或出站切换态，满足则直接返回 `Skipped`。
3. 获取适配器后调用 `campus_check::is_campus_check_silent` 判定静默期；若静默则构造 `on_campus=true` 的跳过结果，否则调用 `check_campus_network` 验证。
4. 校园网验证通过后继续 Portal 检测、状态变更与自动登录准备；若需跳过则早已返回。

## Connections

- [[tauri-app-monitor]]
- [[background-check-system]]
- [[campus-network-adapter]]

## Known Issues

- 原文档误称三个源文件读取失败，实际均已读取并提取到具体函数与行号，本文已重写。
- 代码中历史缺陷（如 `has_logged_online` 永不复位导致整夜休眠后自动登录被拦截）已在 `background_check.rs` 行 354-360 与行 114 等处修复，文档不再标记源码缺失。
