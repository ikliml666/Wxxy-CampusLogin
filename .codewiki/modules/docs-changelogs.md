---
title: modules/docs-changelogs.md
type: module
source_files:
  - tauri-app/src-tauri/src/monitor/scheduled.rs
  - tauri-app/src-tauri/src/monitor/outbound_switch.rs
  - tauri-app/src-tauri/src/monitor/mod.rs
  - tauri-app/src-tauri/src/app/startup.rs
tags: [changelog, audit, outbound-switch, monitor, account, android, desktop, architecture]
---

## Overview

本模块集合包含桌面端监控调度与应用启动的核心源码，涉及定时登录/注销循环、夜间出站切换编排、纯判定层及 Tauri 启动流程。提供的源文件如下：

- `tauri-app/src-tauri/src/monitor/mod.rs`：声明监控子系统模块树（含 `scheduled`、`outbound_switch`、`outbound_night` 等）。
- `tauri-app/src-tauri/src/monitor/scheduled.rs`：独立轻量循环（30s 一拍），复用跨平台纯函数判定定时动作，编排登录/注销、夜间出站切换与运营商夜切验证。
- `tauri-app/src-tauri/src/monitor/outbound_switch.rs`：夜间出站切换的桌面侧动作纯判定与快照序列化层（目标卡选择、逐卡校园网判定、快照 JSON）。
- `tauri-app/src-tauri/src/app/startup.rs`：构建 Tokio runtime、运行 Tauri 应用、在 `setup_app` 中拉起适配器监听与 `run_startup_tasks`。

## Key Components

### 桌面端调度器 (Desktop Scheduler)

*   **文件**: `tauri-app/src-tauri/src/monitor/scheduled.rs`
*   **职责**: 桌面端定时任务编排中心，负责登录/注销循环、夜间出站切换、夜切验证。
*   **常量**:
    *   `SCHEDULED_TICK_MS` (42): 判定拍间隔 `30_000` ms（分钟粒度判定，30s 保证最多半分钟偏差）。
    *   `NIGHT_VERIFY_WAIT_SECS` (46): 夜切验证时序参数，登录编排完成后等 15s 再查。
    *   `NIGHT_VERIFY_RETRY_SECS` (47): 查询失败重试间隔 5s。
    *   `NIGHT_VERIFY_ATTEMPTS` (48): 最多 3 次。
    *   `NIGHT_VERIFY_HTTP_TIMEOUT_SECS` (49): 单次请求 15s 超时。
    *   `SCHEDULED_LOGIN_MAX_RETRIES` (333): 定时登录当日失败补试上限 3 次。
*   **关键函数**:
    *   `run_scheduled_action_loop` (52-186): 30s 主循环入口。由 `watcher::run_startup_tasks` 经 `task_manager.spawn` 拉起。包含启动对账、定时登录/注销判定、夜间出站切换（spec §2 出站动作先行）、运营商夜切门控逻辑。
    *   `is_quitting` (190-192): App 是否正在退出。抽成独立函数只为让循环体不跨 await 持有 State 守卫。
    *   `reconcile_scheduled_logout_on_startup` (202-217): 启动对账（定时注销防冷启动误触发）：应用启动时若当日注销时刻已过，消耗当日额度不执行动作。
    *   `evaluate_scheduled` (226-237): 单拍判定：命中返回 `(应登录, 应注销)`，不置任何标记。门控用**拍前**切换态（见 `outbound_switch_active` 调用）。
    *   `mark_scheduled_day_fired` (244-251): 审计 A7：当日额度标记，在动作抢锁成功后置位（供单测模拟同一置位语义）。
    *   `format_minutes` (253-255): 格式化分钟数为 `HH:MM` 字符串。
    *   `run_scheduled_login` (260-267): 定时登录：复用 `full_login`，成功后走 `post_login_handler`。
    *   `spawn_login_action` (273-329): 登录动作共用执行体（定时登录与晚间断网切换登录同款）。抢 `is_logging_in` 互斥锁，抢不到本拍跳过发 `scheduled-action-skipped` 事件；`login_quota_day` 传入 `Some` 时抢锁成功后消耗当日额度，失败按 `SCHEDULED_LOGIN_MAX_RETRIES` 补试。
    *   `apply_night_switch_action` (338-365): 晚间断网切换动作：改内存配置 → 落盘（`save_config_and_broadcast`）→ 触发登录（复用 `spawn_login_action`）。切换态由配置自身承载。
    *   `run_scheduled_logout` (372-376): 定时注销：复用 `full_logout`，通过 `perform_logout` 执行。
    *   `perform_logout` (383-417): 注销执行体（定时注销与夜切验证复登前的注销同款）。抢 `is_logging_out` 互斥锁，成功后取消登录后自动退出、重置登录/重连状态并启动 60s 注销保护期。
    *   `verify_night_switch` (423-460): 夜切后的登录生效自动验证（独立 tokio 任务不阻塞监控循环）。等 15s → 查 chkstatus 核对 uid → 仍不符则注销重登一轮再复验。
    *   `night_verify_config` (464-468): 读取验证所需的配置快照；operator 已不等于期望值返回 None 跳过验证。
    *   `night_switch_verify_round` (472-490): 一轮验证（3 次重试、间隔 5s）：Ok(uid)=已生效，Err=最后一轮失败原因。
    *   `night_switch_check_once` (493-517): 单次 chkstatus 查询 + uid 核对（复用 `parse_chkstatus`/`uid_matches`）。
    *   `portal_origin` (521-531): portal_url 的 origin（空值回退默认 portal 地址）。
    *   `expected_operator_display` (534-540): 期望运营商的展示名：空串=无锡学院，其余原样。
*   **测试模块** (542-659): 包含 `禁用目标_不触发_不置标记`、`到点触发_置当日标记`、`跨天重置_次日再触发`、`切换态_跳过定时登录且不消耗当日标记_注销照常`、`format_minutes_补零`、`定时登录补试计数_累加清零跨天归零`、`定时登录失败回退额度_补试用尽后当日停止` 等单元测试，验证判定与补试语义。

### 纯判定层 (Pure Decision Layer)

*   **文件**: `tauri-app/src-tauri/src/monitor/outbound_switch.rs`
*   **职责**: 出站切换的纯判定与序列化薄模块，与 campus_check 同源但按适配器粒度。
*   **关键函数**:
    *   `is_campus_adapter` (17-39): 逐卡判定适配器是否处于校园网内：IP 与校园网关同 /18 网段，或绑该卡源 IP 可达校园网关/portal 主机（probe 注入，两发去抖）。
    *   `portal_probe_host` (44-56): 从 portal_url 提取可作绑源探测目标的私网 IPv4 主机（域名/公网/空值返回空串）。
    *   `effective_outbound_priority` (63-82): 出站优先级的生效列表：用户已排序透传；未排序默认无线卡优先、有线卡随后。
    *   `select_outbound_candidate` (87-111): 按优先级名序选出出站目标卡，跳过不在列表、无 IP、校园网内、网关不可达的卡。
    *   `snapshot_json` (125-136): 序列化选中卡的接口跃点快照为 JSON（补 guid）。
    *   `disabled_adapters_json` (148-150): 序列化待禁用校园网卡名单为 JSON。
    *   `parse_disabled_adapters` (156-161): 解析禁用名单快照。空串→空名单；非法 JSON→Err（快照损坏须告警）。
    *   `parse_standby_route` (179-181): 解析兜底路由快照（空串/非法→None）。
    *   `standby_cleanup_decision` (203-220): 兜底路由删除判定纯函数（跨端可测）。
    *   `standby_route_cleanup` (Windows) (224-264): 读实时路由表判定历史兜底路由状态。
    *   `standby_route_cleanup` (非 Windows) (268-270): 非 Windows 按 AlreadyGone 处理。
    *   `unsafe_to_disable` (278-280): 该卡是否不能参与夜间禁用（实例 ID 读不到保守跳过，USB 不再排除）。
    *   `select_campus_to_disable` (285-318): 选出本次切换要临时禁用的校园网卡（在优先级内、已连接、校园网、非目标卡、总线可判定）。
*   **Struct/Model**:
    *   `SnapshotRow` (115-121): 快照行。字段：`guid: String`, `family: u16`, `automatic: bool`, `metric: u32`。
    *   `DisabledRow` (141-145): 待禁用校园网卡行。字段：`guid: String`, `name: String`。
    *   `StandbyRoute` (168-176): 历史兜底默认路由快照。字段：`dest: String`, `mask: String`, `gateway: String`, `metric: u32`, `if_index: u32` (serde rename "ifIndex")。
    *   `StandbyCleanup` (186-196): 枚举。变体：`AlreadyGone`（表里没有这一行）、`ForeignRowKept`（同网关+接口有默认路由但跃点不符）、`LastDefaultPath`（精确匹配但唯一默认路由）、`Safe`（精确匹配且删除后仍有其他默认路由）。
*   **测试模块** (320-691): 覆盖 `is_campus_adapter` 子网/探测分支、`select_outbound_candidate` 优先级跳过、`effective_outbound_priority` 默认序、`snapshot_json` round-trip、`standby_cleanup_decision` 安全判定等。

### 模块声明 (Module Mod)

*   **文件**: `tauri-app/src-tauri/src/monitor/mod.rs`
*   **内容** (1-13): 声明子模块 `watcher`, `auto_auth`, `latency`, `adapter_watch`, `campus_check`, `outbound_switch`, `portal_check`, `quality_scheduler`, `background_emit`, `background_check`, `background_task`, `outbound_night`, `scheduled`。本文件无逻辑，仅模块树组装。

### 启动入口 (Startup)

*   **文件**: `tauri-app/src-tauri/src/app/startup.rs`
*   **职责**: 构建 Tokio runtime、运行 Tauri 应用、在 setup 中初始化日志/配置/托盘并拉起监控任务。
*   **关键函数**:
    *   `build_runtime` (6-20): 构建多线程 Tokio runtime（`worker_threads` clamp 2-8，`max_blocking_threads` clamp 8-64，线程名 campus-worker）。
    *   `run` (23-153): 运行 Tauri 应用，注册插件（shell/notification/autostart/global_shortcut/single_instance）、`manage(AppState::new())`、`setup` 调用 `setup_app`、窗口事件处理、IPC 命令处理器（含 `outbound_switch_now`/`outbound_restore_now` 等）、`app.run` 拦截轻量化退出。
    *   `setup_app` (155-265): 创建数据/日志目录、初始化 logger、加载配置、应用日志保留天数、静默启动判定、构建托盘、启动 `adapter_watch` 与 `adapter_cache` 刷新、启动更新检查、调用 `crate::monitor::watcher::run_startup_tasks` (242)、记录 WebView2 版本与崩溃订阅、心跳线程、GPU 预热。

## Architecture

```text
┌──────────────────────────────────────────────────────────────────┐
│                     Frontend / Tauri IPC                         │
│  (startup.rs run: invoke_handler 注册 outbound_switch_now 等)     │
└───────────────┬──────────────────────────────────────────────────┘
                │ 启动
                ▼
┌──────────────────────────────────────────────────────────────────┐
│ app/startup.rs (setup_app)                                        │
│  - build_runtime (6-20)                                           │
│  - 调用 monitor::watcher::run_startup_tasks (242)                  │
└───────────────┬──────────────────────────────────────────────────┘
                │ 拉起
                ▼
┌──────────────────────────────────────────────────────────────────┐
│ monitor/mod.rs (1-13) 模块树                                      │
│  ├─ scheduled (循环) ────┐                                         │
│  ├─ outbound_switch (判定)◄┘                                        │
│  ├─ outbound_night (动作)                                          │
│  └─ watcher (启动任务)                                            │
└───────────────┬──────────────────────────────────────────────────┘
                │ 循环内调用
                ▼
┌──────────────────────────────────────────────────────────────────┐
│ scheduled.rs: run_scheduled_action_loop (52-186)                  │
│   │                                                              │
│   ├─ evaluate_scheduled (226-237) ──► outbound_switch::          │
│   │                               evaluate_outbound_action 等    │
│   ├─ apply_outbound_switch (经由 outbound_night)                 │
│   └─ apply_night_switch_action (338-365)                         │
└──────────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **启动拉起**：
   `startup.rs::setup_app` (155-265) → `monitor::watcher::run_startup_tasks` (242) → `scheduled::run_scheduled_action_loop` (52-186) 由 task_manager spawn。
2. **每拍判定**：
   循环 30s tick (42) → `is_quitting` 检查 (190) → 读配置快照 → `outbound_switch_active` 拍前态 → `evaluate_scheduled` (226) 调用共享 `evaluate_scheduled_actions` → 触发 `run_scheduled_login`/`run_scheduled_logout`。
3. **出站切换编排**：
   `evaluate_outbound_action` (112) 返回 `Switch|Restore|None` → `run_outbound_blocking` 调用 `apply_outbound_switch`/`apply_outbound_restore` (来自 outbound_night) → 拍后重读 `outbound_switch_active` 门控运营商夜切 `evaluate_night_switch_gated` (173) → `apply_night_switch_action` (338)。
4. **纯判定层交互**：
   `outbound_switch.rs` 提供 `select_outbound_candidate` (87) / `select_campus_to_disable` (285) / `is_campus_adapter` (17) 供动作层构造快照；序列化经 `snapshot_json` (125) / `disabled_adapters_json` (148)。

## Connections

*   `scheduled.rs` 复用 `[[auth-service]]`（`crate::auth::service::full_login`/`full_logout`）及 `[[config-scheduled-tick]]`（`evaluate_scheduled_actions` 等共享纯函数）。
*   `outbound_switch.rs` 依赖 `[[network-discovery]]`（`Adapter` 结构）与 `[[platform-metric]]`（`MetricRow`）。
*   `startup.rs` 驱动 `[[monitor-watcher]]`（`run_startup_tasks`）并初始化 `[[infra-state]]`（`AppState`）。
*   与 `[[modules/core-network]]` 紧密相关，涉及网络绑定、出站切换及校园判定逻辑（原文档关联）。
*   与 `[[modules/user-auth]]` 交互频繁，涉及账号管理、认证协议（原文档关联）。

## Known Issues

1. **异步线程偏差（非阻塞）**：`scheduled.rs` (72-74) 注释明示——`apply_night_switch_action` 内含同步落盘与托盘刷新，仍直接跑在 async worker 线程上（既有实现），本任务新增出站动作进阻塞线程池，但该偏差本轮不治理。
2. **启动后改注销时刻边界**：`scheduled.rs` (200-201) 已知边界不治理——启动后当天再把注销时刻改成另一个已过时刻，下一拍仍会立即触发，与「改完就想立刻注销」无法区分意图。
3. **冷启动注销对账局限**：`scheduled.rs` (202-217) 仅消耗当日额度不执行动作；但运行中的休眠补触发不受影响。
4. **USB 网卡禁用历史顾虑已解**：`outbound_switch.rs` (272-280) 注释——USB 总线网卡不再排除，因用户要求且运行期对称启用已实证，但实例 ID 读不到仍保守跳过。
5. **历史兜底路由不再创建**：`outbound_switch.rs` (163-170, 184-185) 说明 2026-10-18 事故后 `StandbyRoute` 机制仅留解析清理能力，不再新创建，避免误删用户默认路由。
6. **探测双倍调用**：`outbound_switch.rs` (35-38) `is_campus_adapter` 中对 `gateway_probe` 同参数调用两次（去抖），但逻辑上两发相同闭包，若闭包非幂等或带状态需注意（测试 `campus_probe_single_loss_still_counts_as_campus` 模拟首发丢包次发可达）。
