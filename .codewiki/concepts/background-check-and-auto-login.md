---
title: Background Check and Auto Login
type: module
source_files:
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/app/lightweight.rs
  - tauri-app/src-tauri/src/monitor/watcher.rs
  - tauri-app/src-tauri/src/monitor/background_task.rs
  - tauri-app/src-tauri/src/monitor/background_check.rs
  - tauri-app/src-tauri/src/monitor/background_emit.rs
  - tauri-app/src-tauri/src/monitor/auto_auth.rs
  - tauri-app/src-tauri/src/monitor/portal_check.rs
  - tauri-app/src-tauri/src/monitor/campus_check.rs
  - tauri-app/src-tauri/src/monitor/outbound_night.rs
  - tauri-app/src-tauri/src/monitor/scheduled.rs
  - tauri-app/src-tauri/src/infra/notification.rs
  - tauri-app/src-tauri/src/infra/state/network.rs
  - tauri-app/src-tauri/src/infra/lifecycle.rs
  - tauri-app/src-tauri/src/commands/login.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/outbound_switch.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/config_state.rs
tags: [background-check, auto-login, monitor, auth, tauri, android]
---

## Overview

本文档描述后台检查（Background Check）与自动登录（Auto Login）相关模块。该模块在桌面端（tauri-app）与安卓端（android）均有实现。桌面端源码均已成功读取并核对：启动阶段由 `tauri-app/src-tauri/src/app/startup.rs` 的 `setup_app` (L155) 调用 `watcher::run_startup_tasks` (L242) 拉起后台任务；核心逻辑分布在 `monitor/` 目录下的 `background_task`、`background_check`、`background_emit`、`auto_auth`、`portal_check`、`campus_check` 等文件中，涵盖网络适配器枚举、校园网与 Portal 探测、自动登录与断线重连、状态发射与生命周期退出等完整业务逻辑。

## Key Components

以下为各源文件中的关键公开函数/结构体及其行号与用途：

- `tauri-app/src-tauri/src/app/startup.rs`：`setup_app` (L155) 应用初始化并调用 `run_startup_tasks` 启动监控；`run` (L23) 注册 `background` 模块命令（L98-L101）。
- `tauri-app/src-tauri/src/app/lightweight.rs`：轻量化模式运行态，`effective_background_interval_ms` (L77) 定义轻量化期间后台检测间隔下限 300s。
- `tauri-app/src-tauri/src/monitor/watcher.rs`：`run_startup_tasks` (L16) 根据配置启动后台检测、延迟测试、自动登录与定时动作；re-export `start_background_check_inner`、`check_campus_network` 等。
- `tauri-app/src-tauri/src/monitor/background_task.rs`：`start_background_check_inner` (L7) 启动后台检测循环并持久化配置；`spawn_background_check_once` (L78) 统一发起一次性巡检；`spawn_background_check_after_outbound_restore` (L149) 夜间出站还原后等待 IP 就绪并补检。
- `tauri-app/src-tauri/src/monitor/background_check.rs`：`run_background_check` (L510) 异步巡检入口；`run_background_check_blocking` (L29) 阻塞巡检主体（适配器/校园网/Portal/重连）；`should_reset_logged_online` (L25) 会话失效复位判定；`trigger_fast_quality_check` (L436) 门户不可达快排查联动。
- `tauri-app/src-tauri/src/monitor/background_emit.rs`：`BackgroundCheckResult` (L14) 结果聚合结构体；`emit_background_check_result` (L109) 构造并发射检测结果事件；`handle_status_change` (L68) 状态变更通知；`update_network_state` (L166) 更新网络状态。
- `tauri-app/src-tauri/src/monitor/auto_auth.rs`：`try_auto_login_on_preparation` (L94) 准备自动登录（含退避）；`try_disconnect_reconnect` (L206) 断线重连；`run_auto_login_on_start` (L317) 开机自启登录流程。
- `tauri-app/src-tauri/src/monitor/portal_check.rs`：`PortalCheckResult` (L6) 枚举；`check_adapter_portal` (L59) 单适配器 Portal 探测。
- `tauri-app/src-tauri/src/monitor/campus_check.rs`：`CampusCheckResult` (L15) 结构体；`check_campus_network` (L44) 校园网环境验证；`is_campus_check_silent` (L306) 静默期判定。
- `tauri-app/src-tauri/src/monitor/outbound_night.rs`：夜间出站切换/还原实现，还原成功后通过 `background_task::spawn_background_check_after_outbound_restore` 钩子补检。
- `tauri-app/src-tauri/src/monitor/scheduled.rs`：`run_scheduled_action_loop` (L52) 每日定时登录/注销与夜间出站编排。
- `tauri-app/src-tauri/src/infra/notification.rs`：`emit_notification` (L16) 系统通知发送（受启用开关与窗口可见性抑制）。
- `tauri-app/src-tauri/src/infra/state/network.rs`：`NetworkSnapshot` (L7) 网络状态快照；`NetworkState` (L63) 基于 ArcSwap 的原子存储；`update_with_result` (L106) 原子读-改-判定。
- `tauri-app/src-tauri/src/infra/lifecycle.rs`：`start_campus_exit` (L23) 非校园网退出；`cancel_campus_exit` (L146) 取消退出；`start_auto_exit` (L191) 登录后自动退出。
- `tauri-app/src-tauri/src/commands/login.rs`：`do_login` (L63) 与 `do_logout` (L92) 前端登录/注销命令入口；`perform_full_logout_sync` (L105) 注销核心。
- `tauri-app/src-tauri/src/config/model.rs`：`Config` (L10) 配置模型，含 `enable_background_check` (L49)、`background_check_interval` (L51)、`auto_login_on_preparation` (L53) 等字段。
- `tauri-app/src-tauri/src/config/outbound_switch.rs`：`outbound_restore_active` (L68) 切换态判定；`evaluate_night_outbound` (L27) 出站动作决策。
- `android/src-tauri/src/monitor_loop.rs` 与 `android/src-tauri/src/config_state.rs`：安卓端监控循环与配置状态实现，与桌面端 `monitor` 及 `config` 模块同语义（本次核对未内联其源码，行号从略）。

## Architecture

组件关系如下：`startup.rs` 的 `setup_app` (L155) 在应用启动时调用 `watcher::run_startup_tasks` (L242)。`watcher` 依据配置拉起 `background_task::start_background_check_inner`（后台巡检循环）、`auto_auth::run_auto_login_on_start`（开机登录）、`scheduled::run_scheduled_action_loop`（定时动作）等独立任务。

后台巡检循环（`background_task.rs`）按动态间隔调用 `background_check::run_background_check` (L510)，后者在阻塞上下文中依次执行：
1. `campus_check::check_campus_network` 验证校园网环境；
2. `portal_check::check_adapter_portal` 探测 Portal 可达性与在线状态；
3. `auto_auth::try_auto_login_on_preparation` 与 `try_disconnect_reconnect` 执行自动登录与重连；
4. `background_emit` 中的 `update_network_state` 更新 `network::NetworkState` 快照，`emit_background_check_result` 向前端发射事件。
若检测到非校园网且配置允许，`lifecycle::start_campus_exit` 触发退出；登录成功后可经 `lifecycle::start_auto_exit` 自动退出。

## Data Flow

配置加载（`config/model.rs` 的 `Config`）→ `watcher::run_startup_tasks` 分发 → `background_task` 按动态间隔（轻量化下受 `lightweight::effective_background_interval_ms` 下限约束）触发 → `background_check` 获取适配器缓存（`network` 模块） → 校园网检测（`campus_check`）→ Portal 检测（`portal_check`）→ 自动登录/重连决策（`auto_auth`）→ `background_emit` 更新 `NetworkSnapshot`（`infra/state/network.rs`）并 emit 前端事件 → 按需调用 `notification::emit_notification` 或 `lifecycle` 退出流程。夜间出站还原（`outbound_night.rs`）完成后通过 `spawn_background_check_after_outbound_restore` 钩子立即补检，缩短还原后自动登录等待。

## Connections

本模块与以下概念存在代码级关联：
- [[monitor]]：后台任务调度与巡检主体。
- [[auth]]：自动登录、Portal 认证与失败追踪。
- [[tauri-app]]：桌面端 Tauri 命令与生命周期。
- [[android]]：安卓端 `monitor_loop` 与 `config_state` 同构实现。

## Known Issues

- 轻量化模式下后台检测间隔下限为 300s（`lightweight.rs` L77/L78），夜间出站还原后若依赖周期巡检会出现最长数分钟的补检延迟；代码已通过 `background_task::spawn_background_check_after_outbound_restore` (L149) 主动等待 IP 就绪（上限 45s）并补检来缓解。
- 历史缺陷修复留存于代码注释中：如 `background_check::should_reset_logged_online` (L25) 修复整夜休眠唤醒后 `has_logged_online` 永久门控导致不自动登录的问题；`auto_auth::try_disconnect_reconnect` 修复返回 false 导致重连成功仍被旧快照覆盖为离线的问题。
- `background_check` 在出站切换态（三份快照任一非空，`outbound_switch::outbound_restore_active` L68）下整体跳过巡检，避免 Portal 误判触发 MAC 重置。
