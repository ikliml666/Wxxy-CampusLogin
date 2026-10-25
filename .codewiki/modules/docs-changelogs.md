---
title: Docs Changelogs
type: module
source_files:
  - tauri-app/src-tauri/src/monitor/scheduled.rs
  - tauri-app/src-tauri/src/monitor/outbound_switch.rs
  - tauri-app/src-tauri/src/monitor/mod.rs
  - tauri-app/src-tauri/src/app/startup.rs
tags: [tauri, monitor, app, startup, docs-changelogs]
---

## Overview

本模块维护 Tauri 应用中监控与启动相关逻辑，包括定时登录/注销与夜间出站切换编排（`scheduled.rs`）、出站切换动作与快照（`outbound_switch.rs`）、monitor 模块声明（`mod.rs`）以及应用启动初始化（`startup.rs`）。所有关联源文件均已成功读取，无缺失。

## Key Components

实际源码提取的主要函数与结构体（行号以 `行号| ` 前缀为准）：

- `tauri-app/src-tauri/src/monitor/scheduled.rs`
  - `SCHEDULED_TICK_MS` (行 42，30_000)。
  - `run_scheduled_action_loop` (行 52)：主循环，编排定时动作与出站/夜切。
  - `evaluate_scheduled` (行 234)、`mark_scheduled_day_fired` (行 252)、`run_scheduled_login` (行 268)、`spawn_login_action` (行 281)、`SCHEDULED_LOGIN_MAX_RETRIES` (行 342，20)。
  - `apply_night_switch_action` (行 347)、`run_scheduled_logout` (行 381)、`perform_logout` (行 392)、`verify_night_switch` (行 432)、`portal_origin` (行 530)。
- `tauri-app/src-tauri/src/monitor/outbound_switch.rs`
  - `is_campus_adapter` (行 17)、`portal_probe_host` (行 44)、`effective_outbound_priority` (行 63)、`select_outbound_candidate` (行 87)。
  - `SnapshotRow` (行 116)、`snapshot_json` (行 125)；`DisabledRow` (行 142)、`disabled_adapters_json` (行 148)、`parse_disabled_adapters` (行 156)。
  - `PendingSwitchRow` (行 171)、`pending_switch_json` (行 179)、`parse_pending_switch` (行 190)。
  - `monitored_adapter_online` (行 200)、`StandbyRoute` (行 222)、`StandbyCleanup` (行 240)、`standby_cleanup_decision` (行 256)、`standby_route_cleanup` (行 277)。
  - `unsafe_to_disable` (行 331)、`select_campus_to_disable` (行 338)。
- `tauri-app/src-tauri/src/monitor/mod.rs`
  - 行 1-14：声明 `watcher`、`auto_auth`、`latency`、`adapter_watch`、`campus_check`、`outbound_switch`、`portal_check`、`quality_scheduler`、`background_emit`、`background_check`、`background_task`、`outbound_night`、`scheduled`、`dynamic_interval` 子模块。
- `tauri-app/src-tauri/src/app/startup.rs`
  - `build_runtime` (行 6)：构建 Tokio 多线程 runtime。
  - `run` (行 23)：Tauri 应用入口，注册插件与命令处理器。
  - `setup_app` (行 155)：初始化日志、配置、托盘，并启动 `start_adapter_watch` (行 230) 与 `run_startup_tasks` (行 242)。

## Architecture

架构涵盖 `monitor` 模块（含 `scheduled` 与 `outbound_switch` 子模块）及 `app::startup` 引导逻辑。`startup.rs` 在 `setup_app` 中拉起 `adapter_watch` 与 `scheduled` 循环；`scheduled` 循环内调用 `outbound_switch` 提供的纯函数与动作函数完成切换/还原，并复用 `auth::service` 登录注销。

## Data Flow

启动流程：`startup::run` → `setup_app` (行 155) 初始化状态并调用 `start_adapter_watch` (行 230) 与 `run_startup_tasks` (行 242)。`scheduled::run_scheduled_action_loop` (行 52) 每 30s 判定定时登录/注销，并先执行出站切换动作（`outbound_switch` 函数），随后在切换态门控下处理运营商夜切，动作最终调用 `auth::service::full_login` / `full_logout`。

## Connections

- [[monitor]]：monitor 模块声明见 `mod.rs`。
- [[app-startup]]：启动初始化见 `startup.rs`。

## Known Issues

- 原文档误称所有源文件读取失败，实际均已读取并提取到行号与函数定义，本文已重写。
- `outbound_switch.rs` 中历史兜底路由 `StandbyRoute` 相关逻辑仅用于清理旧版残留，新版本不再创建（见行 216-323），文档已如实反映。
