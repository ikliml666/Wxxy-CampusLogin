---
title: modules/docs-changelogs.md
type: module
source_files:
  - tauri-app/src-tauri/src/monitor/mod.rs
  - tauri-app/src-tauri/src/monitor/scheduled.rs
  - tauri-app/src-tauri/src/monitor/outbound_switch.rs
  - tauri-app/src-tauri/src/monitor/outbound_night.rs
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/monitor/watcher.rs
tags: [changelog, audit, outbound-switch, monitor, account, android, desktop, architecture]
---

## Overview

本模块集合包含桌面端监控调度与应用启动的核心源码，涉及定时登录/注销循环、夜间出站切换编排、纯判定层、出站动作层及 Tauri 启动流程。提供的源文件如下：

- `tauri-app/src-tauri/src/monitor/mod.rs`：声明监控子系统模块树（第 1-14 行），含 `scheduled`、`outbound_switch`、`outbound_night`、`watcher` 等子模块。
- `tauri-app/src-tauri/src/monitor/scheduled.rs`：独立轻量循环（30s 一拍，第 44 行常量），复用跨平台纯函数判定定时动作，编排登录/注销、夜间出站切换与运营商夜切验证（第 54-197 行主循环）。
- `tauri-app/src-tauri/src/monitor/outbound_switch.rs`：夜间出站切换的桌面侧动作纯判定与快照序列化层（目标卡选择、逐卡校园网判定、快照 JSON，第 1-735 行）。
- `tauri-app/src-tauri/src/monitor/outbound_night.rs`：夜间出站切换的动作执行层（从 scheduled.rs 原样外移，第 1 行注释），含提权 helper 调用、退避、还原、看门狗等（第 1-1507 行）。
- `tauri-app/src-tauri/src/app/startup.rs`：构建 Tokio runtime、运行 Tauri 应用、在 `setup_app` 中拉起适配器监听与 `run_startup_tasks`（第 6-272 行）。
- `tauri-app/src-tauri/src/monitor/watcher.rs`：启动任务编排，拉起后台检测、质量测试、自动登录及定时动作循环（第 16-72 行）。

## Key Components

### 模块声明 (Module Mod)

*   **文件**: `tauri-app/src-tauri/src/monitor/mod.rs`
*   **内容** (1-14): 声明子模块 `watcher`(1), `auto_auth`(2), `latency`(3), `adapter_watch`(4), `campus_check`(5), `outbound_switch`(6), `portal_check`(7), `quality_scheduler`(8), `background_emit`(9), `background_check`(10), `background_task`(11), `outbound_night`(12), `scheduled`(13), `dynamic_interval`(14)。本文件无逻辑，仅模块树组装。

### 桌面端调度器 (Desktop Scheduler)

*   **文件**: `tauri-app/src-tauri/src/monitor/scheduled.rs`
*   **职责**: 桌面端定时任务编排中心，负责登录/注销循环、夜间出站切换、夜切验证，与安卓端同语义复用纯函数。
*   **常量**:
    *   `SCHEDULED_TICK_MS` (44): 判定拍间隔 `30_000` ms（分钟粒度判定，30s 保证最多半分钟偏差）。
    *   `NIGHT_VERIFY_WAIT_SECS` (48): 夜切验证时序参数，登录编排完成后等 15s 再查。
    *   `NIGHT_VERIFY_RETRY_SECS` (49): 查询失败重试间隔 5s。
    *   `NIGHT_VERIFY_ATTEMPTS` (50): 最多 3 次。
    *   `NIGHT_VERIFY_HTTP_TIMEOUT_SECS` (51): 单次请求 15s 超时。
    *   `SCHEDULED_LOGIN_MAX_RETRIES` (345): 定时登录当日失败补试上限 20 次（第 342-344 行注释说明原 3 次太短，修订为 20 次覆盖约 10 分钟窗口）。
*   **关键函数**:
    *   `run_scheduled_action_loop` (54-197): 30s 主循环入口。由 `watcher::run_startup_tasks` 经 `task_manager.spawn` 拉起（watcher.rs 第 66-71 行）。包含启动对账（第 60-63 行）、定时登录/注销判定（第 90-100 行）、夜间出站切换（spec §2 出站动作先行，第 102-171 行）、运营商夜切门控逻辑（第 172-195 行）。
    *   `is_quitting` (201-203): App 是否正在退出。抽成独立函数只为让循环体不跨 await 持有 State 守卫。
    *   `reconcile_scheduled_logout_on_startup` (213-228): 启动对账（定时注销防冷启动误触发）：应用启动时若当日注销时刻已过，消耗当日额度不执行动作；已知边界第 211-212 行注释启动后改时刻不治理。
    *   `evaluate_scheduled` (237-248): 单拍判定：命中返回 `(应登录, 应注销)`，不置任何标记。门控用**拍前**切换态（第 90 行 `outbound_switch_active` 调用后传入）。
    *   `mark_scheduled_day_fired` (254-262): 审计 A7：当日额度标记，在动作抢锁成功后置位（供单测模拟同一置位语义，`#[cfg_attr(not(test), allow(dead_code))]`）。
    *   `format_minutes` (264-266): 格式化分钟数为 `HH:MM` 字符串。
    *   `run_scheduled_login` (271-278): 定时登录：复用 `full_login`，成功后走 `post_login_handler`，传入当日天数消耗额度。
    *   `spawn_login_action` (284-340): 登录动作共用执行体（定时登录与晚间断网切换登录同款）。抢 `is_logging_in` 互斥锁（第 290 行），抢不到本拍跳过发 `scheduled-action-skipped` 事件；`login_quota_day` 传入 `Some` 时抢锁成功后消耗当日额度（第 305-307 行），失败按 `SCHEDULED_LOGIN_MAX_RETRIES` 补试（第 319-337 行）。
    *   `apply_night_switch_action` (350-377): 晚间断网切换动作：改内存配置 → 落盘（`save_config_and_broadcast`）→ 触发登录（复用 `spawn_login_action`）。切换态由配置自身承载，落盘失败不存内存（第 365-369 行）。
    *   `run_scheduled_logout` (384-388): 定时注销：复用 `full_logout`，通过 `perform_logout` 执行。
    *   `perform_logout` (395-429): 注销执行体（定时注销与夜切验证复登前的注销同款）。抢 `is_logging_out` 互斥锁（第 398 行），成功后取消登录后自动退出、重置登录/重连状态并启动 60s 注销保护期（第 417-427 行）。
    *   `verify_night_switch` (435-472): 夜切后的登录生效自动验证（独立 tokio 任务不阻塞监控循环）。等 15s → 查 chkstatus 核对 uid → 仍不符则注销重登一轮再复验（第 440-471 行）。
    *   `night_verify_config` (476-480): 读取验证所需的配置快照；operator 已不等于期望值返回 None 跳过验证。
    *   `night_switch_verify_round` (484-502): 一轮验证（3 次重试、间隔 5s）：Ok(uid)=已生效，Err=最后一轮失败原因。
    *   `night_switch_check_once` (505-529): 单次 chkstatus 查询 + uid 核对（复用 `parse_chkstatus`/`uid_matches`）。
    *   `portal_origin` (533-543): portal_url 的 origin（空值回退默认 portal 地址）。
    *   `expected_operator_display` (546-552): 期望运营商的展示名：空串=无锡学院，其余原样。
*   **测试模块** (554-675): 包含 `禁用目标_不触发_不置标记` (561-569)、`到点触发_置当日标记` (571-591)、`跨天重置_次日再触发` (593-601)、`切换态_跳过定时登录且不消耗当日标记_注销照常` (603-630)、`format_minutes_补零` (632-636)、`定时登录补试计数_累加清零跨天归零` (639-653)、`定时登录失败回退额度_补试用尽后当日停止` (657-674) 等单元测试，验证判定与补试语义。

### 纯判定层 (Pure Decision Layer)

*   **文件**: `tauri-app/src-tauri/src/monitor/outbound_switch.rs`
*   **职责**: 出站切换的纯判定与序列化薄模块，与 campus_check 同源但按适配器粒度（第 1-9 行注释）。
*   **Struct/Model**:
    *   `SnapshotRow` (115-121): 快照行（派生 Debug, PartialEq, Serialize, Deserialize）。字段：`guid: String`, `family: u16`, `automatic: bool`, `metric: u32`。
    *   `DisabledRow` (141-145): 待禁用校园网卡行（派生 Debug, PartialEq, Serialize, Deserialize）。字段：`guid: String`, `name: String`。
    *   `PendingSwitchRow` (169-175): 待切换状态行（历史遗留，仅解析旧版本残留）。字段：`target: String`, `tag: String`, `day: i32`（`#[serde(default)]` 缺省 0）。
    *   `StandbyRoute` (190-198): 历史兜底默认路由快照。字段：`dest: String`, `mask: String`, `gateway: String`, `metric: u32`, `if_index: u32` (serde rename `"ifIndex"`)。
    *   枚举 `StandbyCleanup` (208-218): 变体 `AlreadyGone`（表里没有这一行）、`ForeignRowKept`（同网关+接口有默认路由但跃点不符）、`LastDefaultPath`（精确匹配但唯一默认路由）、`Safe`（精确匹配且删除后仍有其他默认路由）。
*   **关键函数**:
    *   `is_campus_adapter` (17-39): 逐卡判定适配器是否处于校园网内：IP 与校园网关同 /18 网段，或绑该卡源 IP 可达校园网关/portal 主机（probe 注入，两发去抖，第 35-38 行）。
    *   `portal_probe_host` (44-56): 从 portal_url 提取可作绑源探测目标的私网 IPv4 主机（域名/公网/空值返回空串）。
    *   `effective_outbound_priority` (63-82): 出站优先级的生效列表：用户已排序透传；未排序默认无线卡优先、有线卡随后。
    *   `select_outbound_candidate` (87-111): 按优先级名序选出出站目标卡，跳过不在列表、无 IP、校园网内、网关不可达的卡。
    *   `snapshot_json` (125-136): 序列化选中卡的接口跃点快照为 JSON（补 guid）。
    *   `disabled_adapters_json` (148-150): 序列化待禁用校园网卡名单为 JSON。
    *   `parse_disabled_adapters` (156-161): 解析禁用名单快照。空串→空名单；非法 JSON→Err（快照损坏须告警）。
    *   `parse_pending_switch` (178-183): 解析待切换状态（空串/非法 JSON → `None`），仅用于识别历史版本残留；本版本不再提供序列化入口（第 163-168 行注释）。
    *   `parse_standby_route` (201-203): 解析兜底路由快照（空串/非法 JSON → `None`）。
    *   `standby_cleanup_decision` (225-242): 兜底路由删除判定纯函数（跨端可测），依据同键跃点列表与残余路由数返回 `StandbyCleanup`。
    *   `standby_route_cleanup` (Windows) (246-286): 读实时路由表判定历史兜底路由状态。
    *   `standby_route_cleanup` (非 Windows) (290-292): 非 Windows 按 `AlreadyGone` 处理。
    *   `unsafe_to_disable` (300-302) (cfg windows): 该卡是否不能参与夜间禁用（实例 ID 读不到保守跳过，USB 不再排除）。
    *   `select_campus_to_disable` (307-340): 选出本次切换要临时禁用的校园网卡（在优先级内、已连接、校园网、非目标卡、总线可判定）。
*   **测试模块** (342-735): 覆盖 `is_campus_adapter` 子网/探测分支（如 367-394, 502-511, 654-704）、`select_outbound_candidate` 优先级跳过（396-436, 483-500）、`effective_outbound_priority` 默认序（438-481）、`snapshot_json` round-trip（513-532）、`disabled_adapters_json` (534-546)、`parse_pending_switch` (550-568)、`parse_standby_route` (570-588)、`select_campus_to_disable` 过滤（590-652）、`standby_cleanup_decision` 安全判定（717-734）等。

### 出站动作层 (Outbound Night Action Layer)

*   **文件**: `tauri-app/src-tauri/src/monitor/outbound_night.rs`
*   **职责**: 夜间出站切换的动作执行层（从 scheduled.rs 原样外移，第 1 行注释），负责提权 helper 调用、退避、快照落盘、还原、看门狗、手动入口与启动对账。
*   **常量**:
    *   `OUTBOUND_HELPER_TIMEOUT_SECS` (20): 30s 提权 helper 执行超时。
    *   `OUTBOUND_BACKOFF_START_MS` (24): 60_000 退避起步。
    *   `OUTBOUND_BACKOFF_MAX_MS` (25): 300_000 退避封顶。
    *   `OUTBOUND_RESTORE_ALERT_FAILS` (30): 3 次失败发告警。
    *   `OUTBOUND_RESTORE_ALERT_REMIND_EVERY` (32): 20 次重复提醒间隔。
    *   `OUTBOUND_RESTORE_GIVE_UP_FAILS` (36): 40 次放弃阀。
    *   `OUTBOUND_PROBE_HOST` (399): `"223.5.5.5"` 外网探测目标。
*   **静态变量**:
    *   `OUTBOUND_SWITCH_LAST_FAIL_MS` (55): 切换失败时刻。
    *   `OUTBOUND_SWITCH_FAIL_COUNT` (56): 切换失败计数 pub(super)。
    *   `OUTBOUND_RESTORE_LAST_FAIL_MS` (58): 还原失败时刻。
    *   `OUTBOUND_RESTORE_FAIL_COUNT` (59): 还原失败计数。
    *   `OUTBOUND_CHANNEL_FAIL_COUNT` (63): 提权通道失败计数。
    *   `OUTBOUND_NO_CANDIDATE_WARN_DAY` (67) 等每日提醒去重原子变量 (67-72)。
    *   `OUTBOUND_ACTION_BUSY` (78): 出站动作单飞闸。
*   **Struct**:
    *   `OutboundActionGuard` (81-96): RAII 单飞锁，`try_acquire` (84-89), `Drop` (92-96)。
    *   `OutboundSnapshotRow` (213-219): 切换态快照解析结构，字段 `guid: String`, `family: u16`, `automatic: bool`, `metric: u32`。
    *   `HelperFailure` (230-237): helper 失败分类，字段 `reason: String`, `channel: bool`。
*   **关键函数**:
    *   `run_outbound_blocking` (100-112): 把出站动作送进阻塞线程池执行。
    *   `outbound_switch_active` (116-123): 是否处于出站切换态（三份快照任一非空）。
    *   `in_outbound_restore_window` (128-131): 恢复窗口判定 [恢复起点, 23:00)。
    *   `now_epoch_ms` (133-138): 当前 epoch 毫秒。
    *   `outbound_backoff_ms` (141-148): 退避窗纯函数。
    *   `backoff_gate` (152-157): 退避闸纯函数。
    *   `switch_gate_open` (159-165): 切换退避闸。
    *   `restore_gate_open` (167-173): 还原退避闸。
    *   `mark_outbound_failure` (176-179): 记一次失败。
    *   `clear_outbound_failure` (182-185): 清零退避状态。
    *   `outbound_allow_uac` (188-190): 是否允许降级弹 UAC。
    *   `note_channel_failure` (193-197): 提权通道失败才抬高 UAC 放行计数。
    *   `clear_outbound_channel_failure` (200-202): 通道成功清零。
    *   `needs_replay` (207-209): 是否需要走"未生效补齐"重放纯函数。
    *   `parse_outbound_snapshot` (222-227): 解析切换态快照。
    *   `run_helper_op` (243-269): 调提权 helper 执行单个操作。
    *   `enable_campus_adapter` (desktop) (280-291): 重新启用校园网卡（走 adapter_cache::enable_adapter）。
    *   `enable_campus_adapter` (non-desktop) (295-300): 非桌面 stub 返回错误。
    *   `write_outbound_metric` (308-369): 把目标卡跃点写为 Metric=1，处理退避与通知。
    *   `probe_target_internet` (414-419): 外网可达探测，不阻断切换。
    *   `persist_config_update` (425-431): 写/清本机状态配置并落盘广播。
    *   `set_pending_switch` (436-444): 写待切换状态（仅变化才落盘）。
    *   `drive_pending_switch` (453-467): 清理历史遗留待切换状态（本版本不再挂起）。
    *   `apply_outbound_switch` (469-691): 夜间出站切换动作（候选选择、快照落盘、写跃点、禁用网卡、路由验证）。
    *   `replay_outbound_switch` (708-796): 补齐/重放切换动作（跃点补写、禁用补做）。
    *   `watchdog_re_disable_campus` (805-871): 夜间看门狗，重新禁用被外部启用的校园网卡。
    *   `apply_outbound_restore` (885-1198): 夜间出站还原动作（清理兜底路由、跃点写回、启用网卡、收尾）。
    *   `finish_outbound_restore` (1205-1226): 还原收尾：清快照、清 hold、触发补检。
    *   `clear_outbound_snapshot` (1230-1250): 清空切换态快照（三份）。
    *   `set_outbound_manual_hold` (1256-1264): 「立即切换」当日保持标记落盘。
    *   `clear_outbound_manual_hold` (1267-1269): 清 hold 标记。
    *   `manual_outbound_switch` (1275-1296): 「立即切换」命令入口（桌面）。
    *   `manual_outbound_restore` (1300-1313): 「立即还原」命令入口（桌面）。
    *   `reconcile_outbound_on_startup` (1320-1378): 启动对账（残留切换态还原或重放）。
*   **测试模块** (1380-1507): 包含 `还原失败告警_首达阈值触发_此后周期重复` (1384-1397)、`重放闸_仅切换态且留有失败历史时才补齐` (1399-1410)、`退避窗_起步六十秒_每失败翻倍封顶三百秒` (1412-1420)、`退避闸_无失败历史恒放行` (1422-1428)、`退避闸_首败六十秒内压制_六十秒后放行` (1430-1435)、`退避闸_第二败退避翻倍` (1437-1442)、`退避闸_封顶后三百秒内压制_三百秒后放行` (1444-1451)、`退避闸_时钟回拨按压制处理` (1453-1456)、`快照解析_空串为空表_非法json报错` (1458-1469)、`单飞闸_同一时刻只放一个出站动作_释放后可再取` (1490-1503) 等。

### 启动入口 (Startup)

*   **文件**: `tauri-app/src-tauri/src/app/startup.rs`
*   **职责**: 构建 Tokio runtime、运行 Tauri 应用、在 setup 中初始化日志/配置/托盘并拉起监控任务。
*   **关键函数**:
    *   `build_runtime` (6-20): 构建多线程 Tokio runtime（`worker_threads` clamp 2-8，`max_blocking_threads` clamp 8-64，线程名 campus-worker）。
    *   `run` (23-159): 运行 Tauri 应用，注册插件（shell/notification/autostart/global_shortcut/single_instance）、`manage(AppState::new())`(49)、`setup` 调用 `setup_app`(50-52)、窗口事件处理(53-65)、IPC 命令处理器（含 `outbound_switch_now`(88)/`outbound_restore_now`(89) 等，第 66-137 行）、`app.run` 拦截轻量化退出(144-158)。
    *   `setup_app` (161-272): 创建数据/日志目录(162-186)、初始化 logger(190-195)、加载配置(194)、应用日志保留天数(221)、静默启动判定(226-231)、构建托盘(233)、启动 `adapter_watch`(236) 与 `adapter_cache` 刷新(239)、启动更新检查(243-246)、调用 `crate::monitor::watcher::run_startup_tasks` (248)、记录 WebView2 版本与崩溃订阅(252-254)、心跳线程(256-257)、GPU 预热(261-269)。

### 启动任务编排 (Watcher)

*   **文件**: `tauri-app/src-tauri/src/monitor/watcher.rs`
*   **职责**: 拆分自原 watcher.rs，拉起后台检测、质量测试、自动登录及定时动作循环。
*   **关键函数/重导出**:
    *   `run_startup_tasks` (16-72): 根据配置 spawn 跟踪任务：后台检测(21-31)、启动即测一次质量(36-43)、定时延迟测试循环(45-55)、启动自动登录(57-62)、定时动作循环(66-71) 调用 `super::scheduled::run_scheduled_action_loop`。
    *   重导出 `start_background_check_inner` (10), `check_campus_network` (13), `adapter_status_entry` 等 (14)。

## Architecture

```text
┌──────────────────────────────────────────────────────────┐
│ Frontend / Tauri IPC (startup.rs run:23-159)             │
└───────────────┬──────────────────────────────────────────┘
                │ 启动
                ▼
┌──────────────────────────────────────────────────────────┐
│ app/startup.rs setup_app (161-272)                        │
│  - build_runtime (6-20)                                   │
│  - 调用 monitor::watcher::run_startup_tasks (248)         │
└───────────────┬──────────────────────────────────────────┘
                │ 拉起
                ▼
┌──────────────────────────────────────────────────────────┐
│ monitor/mod.rs (1-14) 模块树                              │
│  ├─ scheduled (循环) ────┐                                 │
│  ├─ outbound_switch (判定)◄┘                               │
│  ├─ outbound_night (动作)                                │
│  └─ watcher (启动任务)                                    │
└───────────────┬──────────────────────────────────────────┘
                │ 循环内调用
                ▼
┌──────────────────────────────────────────────────────────┐
│ scheduled.rs run_scheduled_action_loop (54-197)           │
│   ├─ evaluate_scheduled (237-248) ──► config::scheduled_tick::   │
│   │                       evaluate_outbound_action 等     │
│   ├─ run_outbound_blocking ─► outbound_night::apply_outbound_switch│
│   └─ apply_night_switch_action (350-377)                 │
└──────────────────────────────────────────────────────────┘
```

## Data Flow

1. **启动拉起**：
   `startup.rs::setup_app` (161-272) → `monitor::watcher::run_startup_tasks` (248) → `scheduled::run_scheduled_action_loop` (54-197) 由 task_manager spawn (watcher.rs 66-71)。
2. **每拍判定**：
   循环 30s tick (44) → `is_quitting` 检查 (201) → 读配置快照 (83-86) → `outbound_switch_active` 拍前态 (90) → `evaluate_scheduled` (237-248) 调用共享 `evaluate_scheduled_actions` → 触发 `run_scheduled_login` (271)/`run_scheduled_logout` (384)。
3. **出站切换编排**：
   `evaluate_outbound_action_with_restore` (scheduled.rs 114-123) 返回 `Switch|Restore|None` → `run_outbound_blocking` 调用 `apply_outbound_switch`/`apply_outbound_restore`（定义于 outbound_night.rs 469-691 / 885-1198，于 scheduled.rs 125-133 行调用） → 拍后重读 `outbound_switch_active` (174) 门控运营商夜切 `evaluate_night_switch_gated_with_restore` (183-192) → `apply_night_switch_action` (350-377)。
4. **纯判定层交互**：
   `outbound_switch.rs` 提供 `select_outbound_candidate` (87-111) / `select_campus_to_disable` (307-340) / `is_campus_adapter` (17-39) 供动作层构造快照；序列化经 `snapshot_json` (125-136) / `disabled_adapters_json` (148-150) / `parse_pending_switch` (178-183)。

## Connections

*   `scheduled.rs` 复用 `[[auth-service]]`（`crate::auth::service::full_login`/`full_logout`，第 308、410 行）及 `[[config-scheduled-tick]]`（`evaluate_scheduled_actions` 等共享纯函数，第 31-33 行）。
*   `outbound_switch.rs` 依赖 `[[network-discovery]]`（`Adapter` 结构，第 11 行）与 `[[platform-metric]]`（`MetricRow`，在 `snapshot_json` (125) 等处以全路径 `crate::platform::metric::MetricRow` 引用）。
*   `outbound_night.rs` 依赖 `[[network-subnet]]`（`check_gateway_reachable_from`，第 418、507 行）与 `[[platform-helper-spawn]]`（`spawn_elevated_helper`，第 246 行）。
*   `startup.rs` 驱动 `[[monitor-watcher]]`（`run_startup_tasks` 第 248 行）并初始化 `[[infra-state]]`（`AppState` 第 49、215 行）。
*   与 `[[modules/core-network]]` 紧密相关，涉及网络绑定、出站切换及校园判定逻辑。
*   与 `[[modules/user-auth]]` 交互频繁，涉及账号管理、认证协议。

## Known Issues

1. **异步线程偏差（非阻塞）**：`scheduled.rs` (73-76) 注释明示——`apply_night_switch_action` 内含同步落盘与托盘刷新，仍直接跑在 async worker 线程上（既有实现），本任务新增出站动作进阻塞线程池，但该偏差本轮不治理。
2. **启动后改注销时刻边界**：`scheduled.rs` (211-212) 已知边界不治理——启动后当天再把注销时刻改成另一个已过时刻，下一拍仍会立即触发，与「改完就想立刻注销」无法区分意图。
3. **冷启动注销对账局限**：`scheduled.rs` (213-228) 仅消耗当日额度不执行动作；但运行中的休眠补触发不受影响。
4. **USB 网卡禁用历史顾虑已解**：`outbound_switch.rs` (294-302) 注释——USB 总线网卡不再排除，因用户要求且运行期对称启用已实证，但实例 ID 读不到仍保守跳过（`unsafe_to_disable` 第 300-302 行）。
5. **历史兜底路由不再创建**：`outbound_switch.rs` (185-203) 说明 2026-10-18 事故后 `StandbyRoute` 机制仅留解析清理能力（`standby_route_cleanup` 等），不再新创建，避免误删用户默认路由。
6. **探测双倍调用**：`outbound_switch.rs` (35-38) `is_campus_adapter` 中对 `gateway_probe` 同参数调用两次（去抖），逻辑上两发相同闭包，若闭包非幂等或带状态需注意（测试 `campus_probe_single_loss_still_counts_as_campus` 第 502-511 行模拟首发丢包次发可达）。
7. **快照落盘先行**：`outbound_night.rs` (378-380) 注释——快照在 helper 之前落盘；helper 失败不清快照（裁决 Important-2），切换态保留 + 退避重试写回，重试幂等。
8. **登录闸移除残留**：`outbound_night.rs` (448-452) 注释——2026-10-26 起夜间切换不再产生挂起，`drive_pending_switch` 仅清理历史版本留下的残留行（前端徽标依赖），功能关闭与跨日语义不变。
9. **外网探测不阻断切换**：`outbound_night.rs` (409-413) 注释——探测用绑源 ICMP 直打公网地址，结果不作为切换闸门，只在不可达时记日志+每日提醒，切换照做。
