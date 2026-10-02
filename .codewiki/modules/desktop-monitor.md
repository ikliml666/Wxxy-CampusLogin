---
title: 后台巡检：连通性检测、自动登录/断线重连与网络质量调度
type: module
source_files:
  - tauri-app/src-tauri/src/monitor/mod.rs
  - tauri-app/src-tauri/src/monitor/watcher.rs
  - tauri-app/src-tauri/src/monitor/background_task.rs
  - tauri-app/src-tauri/src/monitor/background_check.rs
  - tauri-app/src-tauri/src/monitor/background_emit.rs
  - tauri-app/src-tauri/src/monitor/campus_check.rs
  - tauri-app/src-tauri/src/monitor/portal_check.rs
  - tauri-app/src-tauri/src/monitor/portal_watch.rs
  - tauri-app/src-tauri/src/monitor/auto_auth.rs
  - tauri-app/src-tauri/src/monitor/latency.rs
  - tauri-app/src-tauri/src/monitor/quality_scheduler.rs
  - tauri-app/src-tauri/src/monitor/adapter_watch.rs
  - tauri-app/src-tauri/src/monitor/scheduled.rs
  - tauri-app/src-tauri/src/monitor/outbound_switch.rs
tags: [monitor, background-check, auto-login, reconnect, campus-network, portal, latency, quality, adapter-watch, scheduled-actions, night-outbound, tauri-events]
---

## Overview

本模块是桌面端的"后台巡检中枢"：一个可配置周期（默认 60000ms、下限 10000ms、轻量态自动放宽）的巡检循环先做校园网环境判定（静默期、出站切换态内跳过），再做主/副适配器的 Portal 连通性检测，据结果驱动"准备自动登录""断线重连""注销保护""校园网退出倒计时""网络状态变更通知"等后续动作；另有四个独立循环——15000ms 的适配器监听（变更事件、被禁用手选适配器的自动启用、class subkey 缓存周期刷新）、用户可配置周期的网络质量定时测试（poor/bad 档位两轮复核与拥堵通知）、30s 一拍的定时动作循环（`scheduled.rs`：定时登录/注销 + 运营商夜切 + 夜间出站切换编排，决策见 [[night-outbound-switch]]、[[night-operator-switch]]），以及 300s 一拍的 Portal 常驻监测（`portal_watch.rs`：逐卡只读探测，JSONL 落盘 + 状态变化事件日志）。

`monitor` 整体在 `lib.rs:17-18` 以 `#[cfg(desktop)]` 门控（17 行属性、18 行 `pub mod monitor;`），安卓 target 完全不编译本模块（安卓有自己的 `android/src-tauri/src/monitor_loop.rs`）。本模块内部**没有任何 `#[cfg]` 平台门控**：`campus_check.rs` 里直接调用 `get_wireless_ssid()` / `get_wired_network_profile()` / `check_gateway_reachable()` 等，这些平台相关的实现由 `network` 层自行门控，`monitor` 只消费跨平台接口；唯一例外是 `outbound_switch.rs`/`scheduled.rs` 里经 `network` 层包装的 Windows 提权与注册表动作（见 Key Components）。

本模块全部 `文件:行号` 引用均相对于 `tauri-app/src-tauri/src/`。

## Key Components

### 模块装配（`monitor/mod.rs`、`monitor/watcher.rs`）

- `monitor/mod.rs:1-13` 仅做子模块声明，顺序为 `watcher:1`、`auto_auth:2`、`latency:3`、`adapter_watch:4`、`campus_check:5`、`outbound_switch:6`、`portal_check:7`、`portal_watch:8`（新增）、`quality_scheduler:9`、`background_emit:10`、`background_check:11`、`background_task:12`、`scheduled:13`；无任何 `pub use`。
- `watcher.rs`（59 行）负责 re-export 与启动挂载：
  - `pub use super::background_check::run_background_check;` — `monitor/watcher.rs:8`，保持旧路径 `watcher::run_background_check` 可用。
  - `pub use super::background_task::start_background_check_inner;` — `monitor/watcher.rs:9`。
  - `pub use super::campus_check::check_campus_network;` — `monitor/watcher.rs:12`（`auto_auth.rs:305` 经此路径调用）。
  - `pub use super::background_emit::{adapter_status_entry, adapter_disabled_entry, adapter_disconnected_entry};` — `monitor/watcher.rs:13`。
- `pub fn run_startup_tasks(app_handle: &AppHandle)` — `monitor/watcher.rs:15-59`，启动期唯一入口，注册四个跟踪任务：

| 条件 | 任务名 | 动作 | 位置 |
| --- | --- | --- | --- |
| `enable_background_check` | `background_check` | `start_background_check_inner(&app_h, state)`；注册失败 `log_warn` | `watcher.rs:22-29` |
| `enable_network_quality && enable_latency_test` | `latency_test` | `spawn_latency_test_loop(&app_h)`；注册失败 `log_warn` | `watcher.rs:34-41` |
| `auto_login_on_start` | `auto_login_on_start` | `run_auto_login_on_start(&app_h)`（函数内部自行判断开关） | `watcher.rs:45-49` |
| 无条件 | `scheduled_actions` | `run_scheduled_action_loop(&app_h, cancel_token)`；**不受 `enable_background_check` 管**（注释 `watcher.rs:51-52`） | `watcher.rs:54-58` |

### 巡检任务生命周期（`monitor/background_task.rs`，64 行）

- `pub(super) fn start_background_check_inner(app_handle, state) -> Result<CommandResult, String>` — `monitor/background_task.rs:7-63`：
  - `config.update`：置 `enable_background_check=true`；`background_check_interval < 10000` 时回写 15000（`background_task.rs:8-13`）。
  - `task_manager.spawn("background_check")`（`background_task.rs:16`）；循环体内：
    - 首拍先取 `cancel_token` 并检查 `is_quitting`（`background_task.rs:20-22`）。
    - 首轮直接 `run_background_check`（`background_task.rs:24`）。
    - 主循环（`background_task.rs:29-53`）：**每拍重读** `interval_ms = effective_background_interval_ms(cfg.background_check_interval.max(10000), is_lightweight_active())`（`background_task.rs:30-38`，轻量态放宽）；重建 `tokio::interval` 计时器；先 `tick` 吞首拍（`background_task.rs:39-40`）；`select!` 等待 tick 或取消令牌（`background_task.rs:41-47`）；退出条件 `!is_running("background_check") || is_quitting`（`background_task.rs:49-51`）；否则执行一轮 `run_background_check`（`background_task.rs:52`）。
  - 成功后 `save_config_to_disk_encrypted`（失败仅 `log_warn`，`background_task.rs:59-61`），返回 `ok_msg("后台检测已启动")`（`background_task.rs:63`）。

### 巡检主体（`monitor/background_check.rs`，378 行）

- `pub async fn run_background_check(app_handle, cancel_token: Arc<CancellationToken>)` — `monitor/background_check.rs:370-378`：`spawn_blocking` 包裹阻塞主体（`background_check.rs:372-374`），join 失败 `log_error`（`background_check.rs:375-377`）。文档注释（`background_check.rs:367-369`）明确职责边界：**巡检只管连通性/Portal/重连，网络质量归 latency 循环**（2026-09-04 收敛）。
- `pub(super) async fn run_background_check_blocking(app_handle, state, cancel_token)` — `monitor/background_check.rs:15-365`，单轮完整流程：
  1. 中止检查（`is_quitting || !is_running("background_check")` 即返回，`background_check.rs:16-18`）；`is_checking.try_acquire` 互斥（`background_check.rs:19-21`）。
  2. 计时 `t_total`（`background_check.rs:22`）；`config.load_full` + debug 日志（`background_check.rs:24-26`）。
  3. 适配器快照：`get_adapters_cached`，为空再 `get_adapters_force`（`background_check.rs:29-38`，计时 `t_adapters`）；`resolve_adapter_names`（`background_check.rs:42`）、`find_dual_adapters`（`background_check.rs:44`）。
  4. `now_min` **单次** `chrono::Local::now()` 取分钟数（`background_check.rs:46-47`，旧版两次取时已修复）。
  5. 出站切换态跳过：`outbound_restore_active(三快照)` 为真则直接短路返回（`background_check.rs:50-57`）。
  6. 静默期分支：`is_campus_check_silent(now_min, start, end)` 为真（`background_check.rs:58-82`）——构造窗口文案（`background_check.rs:63-73`）、`log_info`（`background_check.rs:74`）、`cancel_campus_exit`（`background_check.rs:75`）、写 `current_ssid`/`on_campus_network=true`，产出 `CampusCheckResult { wifi: None, wired: None, on_campus: true }`（`background_check.rs:76-82`）。
  7. 否则 `filter_operation_adapters` + `check_campus_network`（`background_check.rs:85-86`）。
  8. 状态写回 `config.update`：`current_ssid`（`background_check.rs:92`）、`on_campus_network`（`background_check.rs:93`）；校园网判定失败时把 `any_adapter_online`/`last_a1_online`/`has_logged_online` 一并清 false（`background_check.rs:94-102`）。
  9. 校园网失败路径（`campus_check_failed = enable_network_name_check && !on_campus`，`background_check.rs:90`）：`background_check.rs:105-147`——emit 巡检结果（`background_check.rs:115-136`）；`no_configured_ip = a1.is_none() && a2.is_none()`（`background_check.rs:138`），无配置 IP 则跳过退出倒计时（`background_check.rs:139-140`），否则 `start_campus_exit`（`background_check.rs:143`）；`return`（`background_check.rs:146`）。
  10. 校园网通过则 `cancel_campus_exit`（`background_check.rs:150`）；取消点检查（`background_check.rs:152-154`）。
  11. Portal 检测（`background_check.rs:156-190`）：双卡齐全时 `Handle::current().block_on` + 两个 `tauri::async_runtime::spawn_blocking(check_adapter_portal)` + `tokio::join!` 并行（`background_check.rs:163-175`，join 失败按 `PortalCheckResult::Error { is_request_failed: false }` 处理，`background_check.rs:172-173`）；双卡缺一并行退化为串行（`background_check.rs:177-182`）；非 dual 单卡串行（`background_check.rs:185-189`）。
  12. `is_request_failed` 处理（`background_check.rs:194-220`）：判定主/副失败（`background_check.rs:195-197`），分别调 `handle_portal_request_failure(state, app_handle, a1_ip, campus_gw, AdapterFailureCounter::A1, "适配器1")`（`background_check.rs:206-211`）/ `A2`（`background_check.rs:214-219`）。
  13. `PortalCheckResult::Success` 重置该适配器认证失败计数：合并 `config.update` 写 `a1_auth_failure_count=0` / `a2_auth_failure_count=0`（`background_check.rs:225-247`），有恢复时 `log_info`（`background_check.rs:241-246`）。
  14. 汇总（`background_check.rs:249-266`）：`primary_online`/`reachable`/`login_available`（`background_check.rs:249-251`）、`a1_has_ip`（`background_check.rs:252`）、`message` 优先用 campus 消息否则 Portal 消息（`background_check.rs:254-259`）、`online`（`background_check.rs:260`）、`online_operator = uid → operator_suffix_from_uid`（`background_check.rs:262-266`）。
  15. 副适配器三分支（`background_check.rs:277-293`）：Portal `Success` → `(Some(online), portal msg)`；副启用且无 IP → campus 消息或 `(None, "")`；副启用且有 IP → `(None, portal msg)`。`secondary_online_operator` 同法推导（`background_check.rs:295-301`）；`last_a2_online` 写回（`background_check.rs:303`）。
  16. `any_online = online || secondary_online == Some(true)`（`background_check.rs:310`）；`handle_status_change(prev_online, any_online, ...)`（`background_check.rs:311-316`）。
  17. `emit_background_check_result`（`background_check.rs:327-348`）→ `try_auto_login_on_preparation`（`background_check.rs:350`）→ `try_disconnect_reconnect`（`background_check.rs:352-356`）→ **仅当未发生重连**才 `update_network_state`（`background_check.rs:360-362`）→ 完成 `log_debug`（`background_check.rs:364`）。

### 定时动作循环与夜间出站编排（`monitor/scheduled.rs`，1905 行）

模块文档（`scheduled.rs:1-21`）：定时登录/注销（P2-32）+ 夜间出站切换编排（Task 5）。30s 一拍做分钟粒度判定，复用 `config::schedule::should_fire_scheduled_action` 与 `auth::service::full_login/full_logout`；同一拍内**出站动作先行**，再以切换态门控运营商夜切；与后台检测解耦、随 `run_startup_tasks` 无条件启动；与自动登录共用 `is_logging_in` 互斥、独立 `is_logging_out`；当日触发标记存 `state.scheduled`（`ScheduledFired`），到点即置当日标记、不重试。

- 常量（`scheduled.rs:49-81`）：`SCHEDULED_TICK_MS=30_000`（`:49`）、`NIGHT_VERIFY_WAIT_SECS=15`（`:53`）、`NIGHT_VERIFY_RETRY_SECS=5`（`:54`）、`NIGHT_VERIFY_ATTEMPTS=3`（`:55`）、`NIGHT_VERIFY_HTTP_TIMEOUT_SECS=15`（`:56`）、`OUTBOUND_HELPER_TIMEOUT_SECS=30`（`:59`）、`OUTBOUND_STANDBY_ROUTE_METRIC=2`（`:65`）、`OUTBOUND_BACKOFF_START_MS=60_000`（`:69`）、`OUTBOUND_BACKOFF_MAX_MS=300_000`（`:70`）、`OUTBOUND_RESTORE_ALERT_FAILS=3`（`:75`）、`OUTBOUND_RESTORE_ALERT_REMIND_EVERY=20`（`:77`）、`OUTBOUND_RESTORE_GIVE_UP_FAILS=40`（`:81`）。
- 进程级静态原子（`scheduled.rs:100-113`）：`OUTBOUND_SWITCH_LAST_FAIL_MS`/`FAIL_COUNT`（`:100-101`）、`OUTBOUND_RESTORE_LAST_FAIL_MS`/`FAIL_COUNT`（`:103-104`）、`OUTBOUND_CHANNEL_FAIL_COUNT`（`:108`，仅"没拿到 helper 结果"类失败自增，任一 helper 成功清零）、`OUTBOUND_NO_CANDIDATE_WARN_DAY`/`OUTBOUND_EMPTY_DISABLE_WARN_DAY`/`OUTBOUND_METRIC_SNAPSHOT_WARN_DAY`（`:111-113`，按 `num_days_from_ce` 每日一次告警去重）。
- 告警节流：`outbound_restore_alert_due(count)` — `scheduled.rs:84-87`（首次 ≥3 且此后每 20 次提醒一次）；`notify_outbound_issue_once_per_day(app_handle, last_day: &AtomicI32, title, body)` — `scheduled.rs:91-97`（swap 当日去重，`mascot-alert`）。
- `pub fn run_scheduled_action_loop(app_handle, cancel_token)` — `scheduled.rs:116-233`，主循环：
  - 启动先一次性 `spawn_blocking(reconcile_outbound_on_startup)` 对账（`scheduled.rs:122-123`）。
  - 每拍（`tick` 30s，`select!` 可取消，`scheduled.rs:124-132`）：`is_quitting` 退出（`:137-139`）；取 `now_minutes`/`today_day`（`:140-142`）、`config_snapshot`（`:143-146`）、`outbound_active_at_tick = outbound_switch_active()`（`:150`）。
  - `evaluate_and_mark` 判定并落当日标记（`:151-154`）→ `fire_login` → `run_scheduled_login`（`:155-157`）、`fire_logout` → `run_scheduled_logout`（`:158-160`）。
  - 出站动作 `match outbound_action_for(...)`（`:167-209`）：`Switch` → `run_outbound_blocking(apply_outbound_switch)`（`:168-175`）；`Restore` → `apply_outbound_restore`（`:176-183`）；`None` → `needs_replay` 为真时 `replay_outbound_switch` 补齐重放（`:184-201`）+ `watchdog_re_disable_campus` 夜间看门狗（`:205-207`）。
  - 运营商夜切：`outbound_active` 期间保持 `None`，否则 `evaluate_night_switch`（`:216-228`），非 `None` 则 `apply_night_switch_action`（`:229-231`）。
- 出站闸门与退避（`scheduled.rs:305-378`）：`in_outbound_restore_window`（恢复窗 `[07:30,23:00)`，`:305-307`）、`now_epoch_ms`（`:309-314`）、`outbound_backoff_ms`（60s 起步翻倍、封顶 300s，`:317-324`）、`backoff_gate`（无失败恒放行；时钟回拨按压制处理，`:328-333`）、`switch_gate_open`/`restore_gate_open`（`:335-341`/`:343-349`）、`mark_outbound_failure`/`clear_outbound_failure`（`:352-361`）、`outbound_allow_uac`（通道失败过才允许 UAC，`:364-366`）、`note_channel_failure`/`clear_outbound_channel_failure`（`:369-378`）。
- 动作判定：`outbound_action_for(config, weekday, now_minutes, today_day)` — `scheduled.rs:276-301`——**清理分支优先**：切换态残留但功能已关 → `Restore`（不受 hold 冻结，`:290-292`）；否则 `evaluate_night_outbound`（`:293-294`）；`outbound_manual_hold_day == today_day` 且非 0 → `None` 冻结当日自动动作（`:297-299`）。`needs_replay` — `scheduled.rs:383-385`（切换态 && 失败计数 >0）。
- 提权执行：`run_helper_op(op, args, allow_uac_prompt) -> Result<(), HelperFailure>` — `scheduled.rs:419-445`（`spawn_elevated_helper` 30s 超时；`Err` → `channel:true`；`success != true` → `channel:false`）。`HelperFailure { reason, channel }` — `scheduled.rs:406-413`。
- `write_outbound_metric(app_handle, name, guid, families, notify_success, notify_suffix)` — `scheduled.rs:453-514`：families 为空直接失败返回"当前无可用跃点行（网卡已拔出或协议栈缺失）"（不抬 UAC 计数，`:461-471`）；encoded = `guid:family:0:1`（automatic=0、metric=1，`:472-475`）；成功清退避 + 仅 `notify_success` 首次通知（`:477-493`）；失败记退避 + 首败（count==1）通知（`:494-511`）。
- `apply_outbound_switch(app_handle, config) -> Result<(), String>` — `scheduled.rs:538-772`（文档 `:516-537`）：退避闸（`:539-542`）→ 适配器快照（`:543-549`）→ details join 网关（`:551-562`）→ `portal_probe_host`（`:564`）→ `select_outbound_candidate`（`:565-588`，无候选时按"是否配置过优先级"分别 debug/每日一次告警）→ 目标卡 guid 校验（`:589-592`）→ `read_interface_metrics` 过滤 family 2|23（`:594-604`）→ `bus_guard`（Windows 侧为 `unsafe_to_disable`，`:610-613`）→ `select_campus_to_disable`（`:614-622`）→ 空禁用名单但有其他在网优先级卡时每日一次告警"可能未生效"（`:627-640`）→ 组装 `StandbyRoute`（metric=2，`:642-653`）→ **三快照落盘** `save_config_to_disk_encrypted` 后才 `store`，落盘失败不进内存（`:655-664`）→ `write_outbound_metric(notify_success=true)`（`:684-687`）→ 禁用循环 `disable_adapter`（`:690-704`）→ 兜底路由 `route_add`（`:706-733`）→ 路由验证 `best_route_if_index_v4` 仍指校园卡即记失败（`:739-766`）→ `all_ok` 汇总（`:767-771`）。
- `replay_outbound_switch(app_handle, config)` — `scheduled.rs:787-888`：三分支**独立幂等**——① 跃点补写（快照 ∩ 现存协议栈，`:792-829`）；② 禁用补做（`:830-860`）；③ 路由补加（`:861-882`）；无任何新增失败则清切换失败计数（`:885-887`）。
- `watchdog_re_disable_campus` — `scheduled.rs:897-957`（P2-3 夜间看门狗，仅 `None` 拍调用）：guard 窗 ∧ 切换态；名单内卡状态非 `Disabled` → `disable_adapter` + 通知"被重新启用，已自动再次禁用（夜间看门狗）"。
- `apply_outbound_restore(app_handle, config) -> Result<(), String>` — `scheduled.rs:969-1204`（文档 `:959-968`，反向顺序：①删兜底路由 → ②写回跃点 → ③启用网卡；目标卡不存在/跃点行消失视为终态提前出口）：
  - 跃点快照损坏 → 每日一次通知"夜间出站还原不完整"+ 按空继续（`:970-986`）；三快照全空 → `finish_outbound_restore` 收尾（`:987-997`）；`restore_gate_open` + `outbound_allow_uac`（`:998-1001`）。
  - ① 删路由失败 → 记退避 + `outbound_restore_alert_due` 告警 + `route_ok=false` 阻止收尾（`:1005-1043`，快照损坏跳过删除 `:1039-1043`）。
  - ② 跃点写回：目标卡不存在视为已还原（`:1049-1051`）；只写快照中存在且当前协议栈仍在的行，encoded 用**快照原值** `guid:family:automatic:metric`（`:1052-1063`）；`set_metric` 失败 → 告警 + `metric_ok=false`（`:1064-1089`）。
  - ③ 启用：`give_up_corrupt = OUTBOUND_RESTORE_FAIL_COUNT >= 40`（`:1103-1104`）；名单损坏 ∧ give_up → "按空名单放行收尾" + `gave_up_released`（`:1107-1114`）；损坏但未 give_up → 告警 + `enable_ok=false`（`:1115-1129`）；启用循环 `enable_adapter`（`:1131-1170`）。
  - 任一子项失败 → `Err(fail_reasons join "；")`（`:1171-1173`）；收尾通知：`gave_up_released` → 告警"还原流程已收尾，但校园网网卡可能仍处于禁用状态"，正常 → "已还原网络设置（启用 N 张校园网网卡）"/"已还原网卡跃点设置"（`:1174-1198`）；`finish_outbound_restore` → `Ok` / `Err("切换状态快照清理失败，将自动重试")`（`:1199-1203`）。
- 收尾与手动入口：`finish_outbound_restore` — `scheduled.rs:1211-1221`（先清快照，确认清掉才清退避，再清手动保持）；`clear_outbound_snapshot` — `scheduled.rs:1225-1243`（三份一起清，落盘成功才 `store`）；`set_outbound_manual_hold`/`clear_outbound_manual_hold` — `scheduled.rs:1249-1265`。
- `manual_outbound_switch` — `scheduled.rs:1271-1289`（"立即切换"命令入口：未开启/已切换态均 `Err`；清退避 → set hold → 失败则清 hold）；`manual_outbound_restore` — `scheduled.rs:1293-1306`（非切换态 `Err`；先清 hold + 还原退避再还原）。
- `reconcile_outbound_on_startup` — `scheduled.rs:1313-1371`（启动对账，须 `spawn_blocking`）：非切换态直接返回；功能关闭 ∨ 恢复窗内 ∨ 跃点快照不可用 ∨ 目标卡不存在 ∶ 或已回到校园网（IP 为空或 `is_campus_adapter` 绑源探测为真）→ 立即还原；否则 `replay_outbound_switch`。
- 定时登录/注销：`evaluate_and_mark` — `scheduled.rs:1376-1396`（fire → store `scheduled.login_day/logout_day = today_day`）；`gated_night_action` — `scheduled.rs:1403-1417`（`fire_login = !outbound_active && should_fire_scheduled_action(...)`；**注销不受切换态影响**）；`format_minutes` — `scheduled.rs:1419-1421`；`run_scheduled_login` — `scheduled.rs:1425-1431` → `spawn_login_action` — `scheduled.rs:1435-1459`（`is_logging_in.try_acquire`；`full_login`；`emit_login_log`；成功 → `post_login_handler`）；`run_scheduled_logout` — `scheduled.rs:1497-1501` → `perform_logout` — `scheduled.rs:1505-1534`（`is_logging_out.try_acquire`；`full_logout`；成功 → `auto_exit_cancelled=true`、`set_deadline(None)`、`failure_tracker::reset_all`、`network.update(has_logged_online=false, disconnect_reconnect_count=0, last_auto_login_attempt=now, logout_protected_until=now+60s)`）。
- 运营商夜切：`apply_night_switch_action` — `scheduled.rs:1464-1491`（`SwitchToCampus`：`operator → night_operator_restore` 再置空；`Restore` 反向；落盘失败不 `store`、下拍重试；成功 → `spawn_login_action("晚间断网切换")` + `spawn(verify_night_switch)`）；`verify_night_switch` — `scheduled.rs:1540-1577`（等 15s → `night_verify_config`（`operator` 已被改回则跳过）→ `night_switch_verify_round`；未生效 → `perform_logout("夜切验证注销")` + `spawn_login_action("夜切验证复登")` → 等 15s 复验，仍失败 → 通知"夜切验证失败"）；`night_switch_verify_round` — `scheduled.rs:1589-1607`（`create_safe_http_client(15s)`，3 次重试间隔 5s）；`night_switch_check_once` — `scheduled.rs:1610-1634`（GET `{portal_origin}/drcom/chkstatus?callback=dr1003` → `parse_chkstatus` → `uid_matches` 比对）；`portal_origin` — `scheduled.rs:1638-1648`（空回落 `default_portal_url`）；`expected_operator_display` — `scheduled.rs:1651-1657`（空显示"无锡学院"）。
- 测试：`scheduled.rs:1659-1905` 共 18 个（还原失败告警节流 `:1667-1679`、禁用目标不置标记 `:1682-1689`、到点触发置当日标记 `:1692-1709`、跨天重置 `:1712-1719`、切换态跳过登录但注销照常 `:1722-1746`、`gated_night_action` 门控 `:1749-1758`、`format_minutes` 补零 `:1761-1764`、重放闸 `:1769-1779`、退避窗/退避闸 6 例 `:1782-1825`、快照解析 `:1828-1838`、出站动作 3 例 `:1862-1904`）。

### 结果发射、状态更新与通知（`monitor/background_emit.rs`，202 行）

- `pub(super) struct BackgroundCheckResult<'a>` — `monitor/background_emit.rs:14-34`，巡检单轮结果的聚合载体，17 个字段见[结构体与字段](#结构体与字段)。
- 适配器明细条目构造：`adapter_status_entry(name, ip, wireless, online, message) -> serde_json::Value` — `background_emit.rs:36-41`（json 字段 `name/ip/wireless/online/message`）；`adapter_disabled_entry(name)` — `background_emit.rs:43-45`（message = "适配器已禁用或未找到"）；`adapter_disconnected_entry(name, wireless)` — `background_emit.rs:47-49`（"适配器未连接"）。
- `build_adapter_details(...)` — `background_emit.rs:51-65`：主适配器条目必带，副适配器仅在 `is_secondary_adapter_enabled` 时附加。
- `#[allow(clippy::too_many_arguments)] pub(super) fn handle_status_change(...)` — `background_emit.rs:68-107`：状态翻转才 `log_info`（`:87-90`）；离线且 `enable_notification` 时以 `last_network_change_notification_ms` 做 60s 节流（`:94-101`）→ `emit_notification("网络状态变更", details, "mascot-portrait")`；无变化仅 debug（`:105`）。
- `pub(super) fn emit_background_check_result(app_handle, state, result)` — `background_emit.rs:109-164`：CAS `update_with_result` 自增 `background_check_count`（`:116-119`）；`is_running("background_check")`（`:120`）；单次 `load` 快照 `current_ssid`/`on_campus_network`（`:122-124`）；**注销保护期**（`Instant::now() < logout_protected_until`）强制 `effective_online=false`、`effective_secondary_online=Some(false)`（`:127-134`）；`EventBus.emit_background_check_result` 载荷 22 字段（`:136-161`，见 Data Flow）；发射失败 `log_warn`（`:162`）。
- `pub(super) fn update_network_state(state, online, secondary_online, reachable, app_handle)` — `background_emit.rs:166-202`：`server_available = reachable`（`:173`）、`any_online`（`:175`）；注销保护期直接跳过（`:177-183`）；写 `any_adapter_online`/`last_a1_online`，任一在线即清 `disconnect_reconnect_count=0`（`:185-191`）；`reachable && !has_logged_online && online` → `has_logged_online=true`、`prep_login_failures=0`（`:193-197`），若 `auto_exit_on_online` → `start_auto_exit`（`:198-200`）。

### 校园网检测（`monitor/campus_check.rs`，321 行）

- `pub struct ConnectionCampusStatus { on_campus: bool, name: Option<String>, message: String }` — `campus_check.rs:6-10`；`pub struct CampusCheckResult { wifi, wired: Option<ConnectionCampusStatus>, on_campus: bool, current_ssid: Option<String>, message: String }` — `campus_check.rs:14-20`。
- `adapter_campus_status(adapter_name, adapters, campus_result) -> Option<&ConnectionCampusStatus>` — `campus_check.rs:22-27`（按 `info.wireless` 选 wifi/wired 侧）；`adapter_campus_message` — `campus_check.rs:29-31`。
- `campus_name_list(value) -> Vec<&str>` — `campus_check.rs:35-41`：分隔符 `、 ， , ； ;` 空格与 `\t`，trim 后滤空项。
- `pub fn check_campus_network(config, adapters) -> CampusCheckResult` — `campus_check.rs:43-266`：
  - `!enable_network_name_check` 分支（`:47-67`）：只测网关 `check_gateway_reachable(&config.campus_gateway)`，`on_campus = gateway_ok`，wifi/wired/current_ssid 全 `None`，message 区分"网关可达/未连接到校园网络"。
  - 名称检查开启（`:69-75`）：`campus_name_list`、`get_wireless_ssid()`、`get_wired_network_profile()`；`check_gateway` 闭包带 `gateway_checked` **单次探测缓存**（`:79-95`）。
  - WiFi 判定（`:97-174`）：SSID 命中名单即在线（`:100-107`）；不匹配 → 逐卡 `is_same_subnet_18` /18 网段（`:112-122`）→ 网关可达降级（前提该类卡至少一个非空 IP，`:159`）（`:123-129`）→ 否则"当前WiFi非校园网络"（`:130-138`）；无 SSID 时走同样降级链，message = "WiFi未连接校园网"（`:140-172`）。
  - 有线判定（`:176-225`）：profile 命中即在线（`:182-189`）；降级链同上（`:190-222`，网关前提 `wired_adapters` 有非空 IP `:206`），失败文案区分有无 profile（`:216-221`）。
  - 汇总：`on_campus = wifi || wired`（`:227-228`）、message 拼接（`:230-248`）、结果日志在线 debug/离线 warn（`:251-256`）、`current_ssid = wifi_ssid.or(wired_profile)`（`:263`）。
- `is_campus_check_silent(now_minutes: u16, start: u16, end: u16) -> bool` — `campus_check.rs:272-280`：`start == 0` 恒 false；`now < start` 为 true；`end > start && now >= end` 为 true；**`end <= start` 退化为只按 start 门控的单边窗口**。
- 测试：`campus_check.rs:282-321` 共 4 个（名称名单多分隔符 `:287-295`、静默期单边 `:298-304`、静默期双边 `:307-313`、结束不晚于开始退化为单边 `:316-320`）。

### Portal 检测（`monitor/portal_check.rs`，93 行）

- `pub(super) enum PortalCheckResult` — `portal_check.rs:6-19`：`Success { online, message, reachable, login_available, uid: Option<String> }`、`Error { is_request_failed: bool }`、`NotFound`；`impl` 取值器 `:21-57`（`Error` → "检测失败"、`NotFound` → "未找到主适配器"）。
- `pub(super) fn check_adapter_portal(adapter: &Adapter, app_handle) -> PortalCheckResult` — `portal_check.rs:59-93`：调 `check_portal_full(&adapter.ip, Some(&adapter.name))`（`:63`）；`error_kind == Some("request_failed")` → `log_warn` + `emit_login_log(error)` + `Error { is_request_failed: true }`（`:65-72`）；`Ok` → `Success { ps.online, ps.message, ps.reachable, ps.login_available, ps.uid }`（`:74-81`）；其他 `Err` → `Error { is_request_failed: false }`（`:83-91`）。

### Portal 常驻监测（`monitor/portal_watch.rs`，404 行，新增）

模块文档（`portal_watch.rs:1-11`）：Portal 常驻监测——300s 采样、JSONL 落盘 `portal-watch-*.jsonl`（随日志保留天数清理）、状态变化写事件日志；**只读探测绝不携凭据**；ICMP 复用 `check_gateway_reachable_from`（须阻塞线程）；页面/会话复用 `create_safe_http_client`（8s 超时，对齐 `auth::portal` 共享连接池）+ `read_bounded_body`(1MB) + `config::night_switch::parse_chkstatus`；oltime 停摆不发事件。

- 常量：`SAMPLE_INTERVAL_SECS=300`（`portal_watch.rs:25`）、`CLIENT_TIMEOUT_SECS=8`（`:28`）、`REQUEST_TIMEOUT_SECS=3`（`:31`）、`WATCH_FILE_PREFIX="portal-watch-"`（`:33`）。
- `struct Sample` — `portal_watch.rs:37-52`，字段即 JSONL 行字段：`adapter`、`ip`、`icmp`（绑源 ICMP 可达）、`http`（页面 2xx 且非空）、`online`（chkstatus `result==1`）、`uid`、`oltime: Option<i64>`、`carriers`（页面运营商服务列表签名）。
- `pub fn start_portal_watch(app_handle) -> Result<(), String>` — `portal_watch.rs:54-88`：`spawn("portal_watch")`（`:59`）；`interval(300s)` + `MissedTickBehavior::Delay`（`:62-64`）；基线 `last: HashMap<String, Sample>`（`:65`）；循环 `select!` tick/取消（`:67-70`）、`is_quitting` 退出（`:71-78`）；整轮 `spawn_blocking(run_round)` → `apply_events`（`:81-82`，join 失败 `log_warn` `:83`）。
- `fn run_round(app) -> Vec<Sample>` — `portal_watch.rs:92-113`：`portal_url` 为空回落 `default_portal_url`（`:94-98`）；`portal_probe_host` 为空则空轮（`:99-102`）；`get_adapters_cached`（`:103`）；有 IP 的卡逐卡 `sample_adapter`（`:104-108`）；非空则 `write_jsonl(&infra::logger::get_log_dir(app), &samples)`（`:109-111`）。
- `fn sample_adapter(portal_url, portal_host, adapter) -> Sample` — `portal_watch.rs:118-170`：`local_addr = adapter.ip.parse::<IpAddr>()`（`:119`）；`icmp = check_gateway_reachable_from(portal_host, Some(&adapter.ip))`（`:120`）；`create_safe_http_client(8s, local_addr)`（`:122-140`，构造失败 → 全 false 的 Sample `:127-139`）；页面 GET `{base}/`（`:143-148`）；chkstatus GET `{base}/drcom/chkstatus?callback=dr1003`（`:149-154`）；`parse_chkstatus` → `(online, uid, oltime)`，解析失败按 `(false, "", None)`（`:155-158`）；`carriers = carriers_signature(&page)`（`:168`）。
- `fn fetch_text(client, url, label, adapter_name) -> String` — `portal_watch.rs:174-198`：`block_on(send + 3s timeout)`（`:175-180`）；2xx → `auth::protocol::read_bounded_body`（`:182-187`）；其余/失败仅记 debug 返回空串（`:189-197`，避免 5 分钟粒度刷屏）。
- `fn carriers_signature(html) -> String` — `portal_watch.rs:205-215`：`html.split("{\"id\"")` 逐段取 `name`+`suffix` 以 `|` 拼接——Dr.COM 服务列表内嵌在页面 JS 里，运营商（如 `@cmcc`）服务下架会立即反映为签名变化；`json_string_field(chunk, key)` — `portal_watch.rs:218-224`。
- `fn write_jsonl(dir, samples)` — `portal_watch.rs:228-254`：按天文件 `portal-watch-{YYYY-MM-DD}.jsonl` 追加，行字段 `ts/adapter/ip/icmp/http/online/uid/oltime/carriers`（`:234-244`）；`create_dir_all` + `OpenOptions::append`（`:248-252`）；`cleanup_watch_files`（`:253`）。
- `fn cleanup_watch_files(dir)` — `portal_watch.rs:257-285`：按 `get_log_retention_days`，0 = 永久保留；`cutoff = now - days*86400`；只删 `WATCH_FILE_PREFIX` 前缀且 `.jsonl` 后缀的文件（`:271-284`）。
- `fn apply_events(last: &mut HashMap<String, Sample>, samples)` — `portal_watch.rs:289-297`：逐卡写基线 + `diff_events` → `log_info("[监测] {adapter}: {ev}")`（`:290-295`）；`retain` 只留本轮出现的卡（`:296`）。
- `fn diff_events(prev: Option<&Sample>, cur) -> Vec<String>` — `portal_watch.rs:300-325`：首样只建基线不发事件（`:301-303`）；http 断 → "portal 页面不可达（http 断，icmp={}）"（`:305-306`）；恢复 → "portal 页面恢复可达"（`:307-308`）；会话掉线 "会话掉线（uid 空，oltime={:?}）"/检测到在线 "检测到在线会话 {uid}"（`:310-313`）；oltime 回退 → "会话重连（oltime {} → {}）"（`:315-319`）；carriers 双非空且不同 → "portal 运营商配置变更: {} → {}"（`:321-323`）。
- 测试：`portal_watch.rs:327-404` 共 8 个（carriers 签名 3 例 `:335-353`，其中 `PAGE_CARRIERS` 常量 `:332` 为 2026-10-03 实测 4 项服务列表；diff_events 5 例 `:369-403`，helper `sample()` `:355-366`）。

### 自动登录与断线重连（`monitor/auto_auth.rs`，492 行）

- 常量：`RECONNECT_REMINDER_INTERVAL: u32 = 10` — `auto_auth.rs:14`；`PREP_LOGIN_MAX_FAILURES: u32 = 5` — `auto_auth.rs:19`（文档 `:16-18`：防无限重试 + 周期性 DHCP 断网）。
- `reconnect_should_report(login_success: bool, within_limit: bool) -> bool` — `auto_auth.rs:25-27`（纯函数，被测试覆盖）。
- `pub(super) fn try_auto_login_on_preparation(app_handle, state, login_available, online, config)` — `auto_auth.rs:29-107`（"准备登录"阶段）：门槛 `!login_available || online || !auto_login_on_preparation`（`:36-38`）；单次 `load` 快照（`:41`）；`has_logged_online` 已在线跳过（`:42-44`）；`prep_login_failures >= 5` 停止并日志（`:48-55`）；注销保护期跳过（`:57-61`）；冷却 `auto_login_cooldown_secs`（`:63-67`）；`is_logging_in.try_acquire` 抢锁（`:70`）；写 `last_auto_login_attempt`（`:72`）；`full_login(state, app_handle, None)`（`:73`）；耗时日志（`:76-79`）；`emit_auto_login_result(success, message, false)`（`:81-87`）；成功 → `has_logged_online=true`、`prep_login_failures=0`（`:90-93`）+ 可选通知"自动登录成功"（`mascot-celebrate`，`:94-96`）；失败 → 计数 `saturating_add(1)`（`:100-102`）；锁占用仅 debug（`:104-106`）。
- `#[allow(clippy::too_many_arguments)] pub(super) fn try_disconnect_reconnect(...) -> bool` — `auto_auth.rs:110-218`（断线重连）：`any_offline = (!online && a1.is_some()) || secondary_online == Some(false)`（`:122`）；前置门 `!any_adapter_online || !any_offline || !reachable || !login_available || !auto_login_on_preparation`（`:126-128`）；注销保护（`:130-134`）与冷却（`:136-139`）检查；先取 `is_logging_in` 锁再 CAS 计数 `disconnect_reconnect_count`（`:142-155`）；`within_limit = count <= max_disconnect_reconnect`（`:156`）：
  - 未超限（`:159-195`）：`offline_adapter = if !online { adapter1_name } else { adapter2_name }`（`:160`）；通知"检测到断线 ... ({reconnect_count}/{max})"（`mascot-alert`）+ `emit_login_log` warning（`:161-166`）；写 `last_auto_login_attempt`（`:173`）；`full_login`（`:174`）；成功 → `count=0`、`any_adapter_online=true`、`has_logged_online=true`、`prep_login_failures=0`（`:182-187`）+ `append_login_history(..., "断线重连成功", offline_adapter, &config.user, "reconnect")`（`:188-190`）+ `emit_auto_login_result(true, "断线重连成功: ...", false)`（`:191-194`）。
  - 超限（`:196-210`）：先 `drop(login_guard)`（`:198`）；`count == max+1` 时一次性通知"断线重连失败：已达到最大重连次数，请手动登录"（`:199-202`）；此后每 `RECONNECT_REMINDER_INTERVAL`(10) 次提醒"网络仍断线"（`:203-209`）。
  - 返回 `reconnect_should_report(reconnect_success, within_limit)`（`:217`；修复注释 `:212-216`：旧版恒 `false` 导致重连成功结果被旧快照覆盖回离线）。
- `pub fn run_auto_login_on_start(app_handle)` — `auto_auth.rs:220-469`（启动自动登录，两阶段）：
  - 阶段一（`:222-338`）：`config` 检查 `auto_login_on_start`（`:222-226`）；`is_auto_start = std::env::args().any(|a| a == "--autostart")`（`:228`）；`initial_delay = is_auto_start ? 5000ms : 1500ms`（`:229`）；`task_manager.spawn("auto_login_on_start")`（`:236`）；`sleep(initial_delay)`（`:237`）；`is_quitting || has_logged_online` 退出（`:240-242`）；`spawn_blocking(get_adapters_force)`（`:245-256`，失败 `log_error` + `emit_auto_login_result(false, ..., false)` + return `:250-255`）；debug 列表（`:261-264`）；自启场景重试 `0..3` 次、每次等 3s 重查（`:266-281`）；重读 config（`:283-284`）、`resolve_adapter_names`（`:285`）。
  - 校园网校验（`enable_network_name_check`，`:287-338`）：`now_min` 此处**仍是两次** `chrono::Local::now()`（`:288`）；`skip_campus = is_campus_check_silent(...)`（`:289`）；静默期跳过并置 `on_campus_network=true`（`:291-295`）；否则 `spawn_blocking(check_campus_network(filter_operation_adapters(...)))`（`:300-310`，join 异常 `log_error` + return 防误踢 `:306-309`）；不通过 → 写状态（`:314-317`）+ `emit_auto_login_result(false, msg, true)`（`:318`）+ `a1_has_ip/a2_has_ip` 判定（`:320-322`）——均无 IP 仅日志跳过退出（`:323-324`），否则 `start_campus_exit`（`:327`）后 return（`:329`）；通过 → 写 `current_ssid`/`on_campus_network=true`（`:332-335`）。
  - 阶段二 Portal 探测与登录（`:340-469`）：`find_dual_adapters`（`:340`）；a1 分支（`:341-417`）——Portal 探测（注释 `:351-352`：状态探测只读、不携凭据）：有 a2 → 两个 `spawn_blocking(check_portal_full(&ip, Some(&name)))` 先 spawn 后 await 并行（`:353-362`），无 → 单 `spawn_blocking` 串行（`:364`）；`portal_elapsed` 计时（`:368`）；结果日志含 `data_length`（`:370-379`）；`portal_status.online` → 已在线跳过登录：`adapter_names`（副在线则含 n2）组装（`:382-393`）、msg="已在线（...）"（`:394`）、写 `any_adapter_online`/`has_logged_online`/`prep_login_failures=0`（`:396-400`）、`emit_auto_login_result(true, msg, true)`（`:404`）、return（`:405`）；join 异常仅日志（`:407-413`）；未找到主适配器 `log_warn`（`:414-417`）。登录（`:419-469`）：`spawn_blocking` 内 `has_logged_online` → "已在线，跳过登录"（`:423-425`）；`is_logging_in.try_acquire` 抢不到 → "登录正在进行中"（`:426-429`）；写 `last_auto_login_attempt`（`:430`）；`full_login`（`:431`）；结果日志（`:437-440`）；`emit_auto_login_result`（`:442-448`）；成功 → 写 `has_logged_online`/`prep_login_failures=0`（`:452-455`）+ 通知（`:456-458`）+ `auto_exit_after_login` → `start_auto_exit`（`:460-463`）；任务注册失败 `log_warn`（`:467`）。
- 测试：`auto_auth.rs:471-492` 共 3 个（`reconnect_success_within_limit_reports_true:476-479`、`reconnect_failure_never_reports_true:482-485`、`reconnect_success_over_limit_reports_false:488-491`）。

### 适配器监听（`monitor/adapter_watch.rs`，314 行）

- 常量：`ADAPTER_WATCH_INTERVAL: u64 = 15000` — `adapter_watch.rs:9`；`CLASS_SUBKEY_REFRESH_ROUNDS: u32 = 4` — `adapter_watch.rs:15`（class subkey 缓存刷新周期 = 4 × 15s = 60s；文档 `:10-14`：禁用分类检测依赖缓存新鲜度）。
- `pub fn start_adapter_watch(app_handle) -> Result<(), String>` — `adapter_watch.rs:17-288`：`spawn("adapter_watch")`（`:19`）；初始化 `last_adapters`/`last_disabled`/`class_refresh_round`/`interval_timer`(15s + `MissedTickBehavior::Delay`)（`:21-27`）；先 `tick` 吞首拍（`:28`）；循环（`:30`）`select!` tick/取消（`:31-36`）、`is_quitting` 退出（`:38-41`）：
  - 每轮 `cleanup_expired_dns_cache()`（`:43`）；`class_refresh_round += 1`，每 4 轮 `spawn_blocking(registry::refresh_class_subkey_cache)`（`:52-58`）。
  - `spawn_blocking(get_all_adapters_cached)`（`:64`）。
  - 变更检测与事件（`Ok(Ok((adapters, details, disabled)))`，`:66` 起）：`adapters_changed` 按 name 排序比较 len/name/ip（`:67-74`）；`disabled_changed` 按 name 排序比较 len/name/status（`:78-85`）；变更 → `emit_adapters_changed`（`:88-90`）+ details 非空时 `emit_adapter_details_changed`（`:91-95`）；禁用名单变更 → `emit_disabled_adapters_changed`（`:99-101`）+ `adapter_recovered`（上轮禁用、本轮恢复，`:102-104`）→ `!any_adapter_online` 时记日志并**一次性**补跑 `run_background_check`（复用 `task_manager.cancel_token("background_check")`，没有才新建；`adapter_watch.rs:109-119`，注释明确不再无条件重开已停止的巡检）。
  - 禁用告警：60s 节流 `last_disabled_notification_ms`（首次 0 放行，`adapter_watch.rs:122-131`）后 `store`（`:135`）；`configured_names`（`:136-145`：副启用且 `adapter2 != AUTO_DETECT_ADAPTER` → `[a1, a2]`；`a1` 非空且非哨兵 → `[a1]`；否则空）；`night_disabled = parse_disabled_adapters(&c.outbound_disabled_adapters).ok()` → `HashSet`（`:150-153`）；对每个新增禁用 ∧ 在 `configured_names` ∧ 不在 `night_disabled` → `emit_adapter_disabled_warning(name, "适配器{} 当前{}，请检查后重试")`（`:154-164`）。
  - 自动启用块（`:174-273`）：`config`（`:176`）；`guard_window = is_night_outbound_guard_window(enable_night_outbound_switch, weekday, now_minutes) && outbound_restore_active(三快照)`（`:185-194`，文档 `:177-184`：**守护窗 ∧ 切换态内不做自动启用**，防止与夜间还原互相踩）；`candidates = configured_disabled_adapters(&c, &disabled)`（网络侧过滤，"自动检测"哨兵不参与，`:195`）；guard_window 且非空 → debug 跳过（`:196-202`）；`night_disabled`（解析失败时按"全部拦截"保守处理，`:203-209`）；`targets = candidates` 过滤 `!night_disabled.contains && !guard_window`（`:210-214`）；targets 空 → 非零 `auto_enable_failure_count` 清零（`:216-221`）；否则 `now_ms` + 退避判断 `auto_enable_backoff_ms(failure_count)`（`:223-228`）、先 `store last_attempt`（`:230`）；`allow_uac = failure_count > 0`（首试静默、失败后允许 UAC 弹窗——针对 CMSTPLUA 被封堵场景，注释 `:231-234`）；逐卡 `spawn_blocking(enable_adapter(&name, allow_uac))`（`:241-269`）：成功 → 日志 + 计数清零（`:244-246`）+ 等 5s 后一次性 `run_background_check`（`:247-256`）；失败 → `fetch_add` + 按新失败数记退避 + `log_warn`（`:258-267`）。
  - 收尾：更新 `last_adapters`/`last_disabled`（`:275-276`）；查询失败（`Ok(Err)`/`Err` 两分支）仅 `log_warn`（`:277-284`）。
- `auto_enable_backoff_ms(failure_count: u32) -> u64` — `adapter_watch.rs:292-299`：0 → 0；1 → 60s；2 → 120s；≥3 → 300s 封顶。
- 测试：`adapter_watch.rs:301-314` 共 1 个（`auto_enable_backoff_ladder:306-313`，含 `u32::MAX` 封顶断言）。

### 网络质量定时测试（`monitor/latency.rs`、`monitor/quality_scheduler.rs`）

- `latency.rs`（162 行）——循环与档位判定：
  - 常量：`BAD_LEVELS = ["poor","bad"]`（`latency.rs:11`）、`GOOD_LEVELS = ["excellent","great","good"]`（`:12`），均 `pub(super)`。
  - `classify_quality_change(last: Option<&str>, current: &str, enable_notification: bool) -> Option<&'static str>` — `latency.rs:17-37`（纯函数）：仅在 `last` 已知且与 `current` 不同、且跨越"坏档进入"或"从坏档恢复"边界时返回 `Some("bad")`/`Some("good")`；中间档互切、未知 `last`、关闭通知均 `None`。
  - `record_last_quality(state, current)` — `latency.rs:39-41`（写 `last_network_quality`）；`notify_quality_change(app_handle, kind)` — `latency.rs:43-51`（bad → 通知"网络拥堵"+ `emit_login_log` warning；否则"网络恢复" + info）。
  - `pub fn spawn_latency_test_loop(app_handle) -> Result<(), String>` — `latency.rs:53-123`：`spawn("latency_test")`（`:55`）；动态间隔 `desired_ms = effective_quality_interval_ms(cfg.latency_test_interval.max(10_000), is_lightweight_active())`（`:66-73`），变化即重建计时器并吞非首拍 tick（`:74-82`）；`select!` 可取消（`:83-88`）；退出条件 `!is_running("latency_test") || is_quitting`（`:90-94`）；就绪等待循环（`:100-111`）：`config` + `get_adapters_cached_async` → `select_adapter(&adapters, &config)`，`!ip.is_empty() && any_adapter_online` 才继续，否则 2s 重试（可取消，`:107-110`）；检测前等 1s（`:112-116`）；`run_quality_check(&app_h, &adapter_name, &adapter_ip, Some(&cancel_token))`（`:119`，注释 `:117-118`：停止后正在执行的一轮提前返回）。
  - 测试：`latency.rs:125-162` 共 5 个（劣化到坏档 `:130-133`、坏档恢复 `:136-139`、同档/未知不通知 `:142-146`、中间档互切不通知 `:149-155`、关闭通知不分类 `:158-161`）。
- `quality_scheduler.rs`（118 行）——单轮质量检测与复核：
  - 常量：`SPIKE_CONFIRM_COUNT: usize = 2`（`quality_scheduler.rs:12`）、`SPIKE_CONFIRM_INTERVAL_SECS: u64 = 15`（`:13`；文档 `:9-11`：防瞬时抖动误报）。
  - `fn perform_quality_check(app_handle, adapter_name, adapter_ip, lightweight: bool) -> Option<serde_json::Value>` — `quality_scheduler.rs:19-45`：读 `skip_ttfb_in_latency`/`skip_content_in_latency`/`fixed_gateway`（`:21-24`）；`is_quality_checking.try_acquire` 互斥（`:25-28`）；`check_network_quality_async(..., is_quitting, Some(app_handle), lightweight)`（`:29`）；序列化失败 → `log_warn` + `None`（`:30-36`）；`append_quality_history`（`:38-40`）+ `emit_network_quality_result`（`:41-43`）。
  - `pub(super) fn run_quality_check(app_handle, adapter_name, adapter_ip, cancel: Option<&CancellationToken>)` — `quality_scheduler.rs:53-118`（文档 `:47-52`：周期驱动者只剩 `spawn_latency_test_loop`，手动命令不经过本函数）：取消预检（`:56-58`）；首测 `lightweight=false`（`:59-61`）；`current = first["quality"].unwrap_or("unknown")`（`:62`）；`last = network.last_network_quality`（`:63`）；`classify_quality_change`（`:64`）：
    - `Some("bad")` 分支（`:65-110`）：立即记 `last_network_quality` 并进入复核——`SPIKE_CONFIRM_COUNT`(2) 轮、每轮 `sleep(15s)`（可取消，`:72-81`）、`is_quitting || cancel` 提前返回（`:82-84`）；复核轮 `perform_quality_check(lightweight=true)` 只测网关 + 1 个外网站点（`:86`）；结果不在 `BAD_LEVELS` → `confirmed=false` break（`:87-95`）；复核轮拿不到互斥锁（`None`）→ 同样按未确认处理，**不通知不落状态**（`:96-101`）；`confirmed` → `notify_quality_change("bad")`（`:104-106`）；最终 `record_last_quality`（`:107-109`）。
    - else 分支（`:111-117`）：`Some(kind)` → `notify_quality_change(kind)`（good 恢复等）；末尾统一 `record_last_quality(current)`（`:115`）。

### 测试辅助项

各文件 `#[cfg(test)]` 内嵌测试共 59 个：`campus_check.rs:282-321`（4）、`portal_check.rs`（无）、`portal_watch.rs:327-404`（8）、`auto_auth.rs:471-492`（3）、`latency.rs:125-162`（5）、`quality_scheduler.rs`（无）、`adapter_watch.rs:301-314`（1）、`scheduled.rs:1659-1905`（18）、`outbound_switch.rs:214-522`（20）。测试风格：纯函数直接断言；涉状态的用临时 `AppState`/构造 config（如 `scheduled.rs:1841-1859` 的 `SNAPSHOT_ONE_ROW`/`outbound_test_config(_hold)`、`outbound_switch.rs:220-231` 的 `adapter()` 与 `:234-236` 的 `no_probe()` panic 闭包）；涉外发的（emit/通知）不覆盖。

## 结构体与字段

| 结构体 | 位置 | 字段 |
| --- | --- | --- |
| `BackgroundCheckResult<'a>` | `background_emit.rs:14-34` | `online:15`、`reachable:16`、`login_available:17`、`message:18`、`adapter1_name:19`、`adapter2_name:20`、`secondary_online:21`、`secondary_message:22`、`online_operator:23`（Portal uid 推导运营商后缀：空串=无锡学院、`@telecom`/`@unicom`/`@cmcc`；离线时语义上应为 None）、`secondary_online_operator:25`、`dual_adapter:27`、`config:28`、`campus_result:29`、`a1_campus_msg:30`、`a2_campus_msg:31`、`a1_on_campus:32`、`a2_on_campus:33`（共 17 字段） |
| `ConnectionCampusStatus` | `campus_check.rs:6-10` | `on_campus: bool`、`name: Option<String>`、`message: String` |
| `CampusCheckResult` | `campus_check.rs:14-20` | `wifi`/`wired: Option<ConnectionCampusStatus>`、`on_campus: bool`、`current_ssid: Option<String>`、`message: String` |
| `PortalCheckResult` | `portal_check.rs:6-19` | enum：`Success { online, message, reachable, login_available, uid: Option<String> }`、`Error { is_request_failed: bool }`、`NotFound` |
| `Sample` | `portal_watch.rs:37-52` | `adapter`、`ip`、`icmp: bool`、`http: bool`、`online: bool`、`uid: String`、`oltime: Option<i64>`、`carriers: String` |
| `OutboundSnapshotRow` | `scheduled.rs:389-395` | `guid`、`family: u16`、`automatic: bool`、`metric: u32`（跃点快照行，JSON 编码 `guid:family:automatic:metric`） |
| `HelperFailure` | `scheduled.rs:406-413` | `reason: String`、`channel: bool`（true = 提权通道/UAC 层失败） |
| `SnapshotRow` | `outbound_switch.rs:90-95` | 私有：`guid`、`family`、`automatic`、`metric`（序列化经 `snapshot_json`，`:99-110`，空行兜底 `"[]"`） |
| `DisabledRow` | `outbound_switch.rs:116-119` | pub：`guid`、`name`（禁用名单行；`disabled_adapters_json:122-124`、`parse_disabled_adapters:130-135`——空/空白 → 空名单，非法 JSON → `Err("禁用名单快照损坏: {e}")`，**损坏 ≠ 无动作**） |
| `StandbyRoute` | `outbound_switch.rs:143-150` | pub：`dest`、`mask`、`gateway`、`metric`、`if_index`（serde rename `ifIndex`；`standby_route_json:153-158`、`parse_standby_route:161-163`） |

## Data Flow

### 启动挂载

`lib.rs` → `run_startup_tasks`（`watcher.rs:15-59`）按上表注册四个任务；`scheduled_actions` 无条件启动并在首拍前做一次出站对账（`scheduled.rs:122-123`）；`portal_watch` 由 `start_portal_watch`（`portal_watch.rs:54-88`）独立常驻。巡检循环内部每拍动态重读间隔与轻量态（`background_task.rs:30-38`）。

### 单轮巡检主链

```
run_background_check (spawn_blocking, background_check.rs:370-378)
 ├─ 前置：互斥 is_checking / 出站切换态跳过 / 静默期分支
 ├─ check_campus_network (campus_check.rs:43-266) → 写 on_campus_network / current_ssid
 ├─ 失败：emit → 无 IP 则跳过 / 否则 start_campus_exit → return
 ├─ Portal: check_adapter_portal (portal_check.rs:59-93)，双卡并行 tokio::join! / 单卡串行
 │    ├─ is_request_failed → handle_portal_request_failure(A1/A2)
 │    └─ Success → 失败计数清零
 ├─ 汇总 online / secondary_online / online_operator
 ├─ handle_status_change (background_emit.rs:68-107) → 60s 节流通知
 ├─ emit_background_check_result (background_emit.rs:109-164) → 前端事件
 ├─ try_auto_login_on_preparation (auto_auth.rs:29-107)
 ├─ try_disconnect_reconnect (auto_auth.rs:110-218) → bool
 └─ !reconnected 才 update_network_state (background_emit.rs:166-202)
```

### `emit_background_check_result` 事件载荷（`background_emit.rs:136-161`，22 字段）

`serverAvailable`(=reachable)、`loginAvailable`、`online`(=注销保护期改写后的 `effective_online`)、`message`、`adapter1Name`、`adapter2Name`(非 dual 为空串)、`secondaryOnline`、`secondaryMessage`、`onlineOperator`/`secondaryOnlineOperator`（保护期为 None）、`timestamp`、`checkCount`、`isRunning`、`currentSsid`、`onCampusNetwork`、`enableNetworkNameCheck`、`requiredNetworkName`、`campusWifi`、`campusWired`、`a1CampusMessage`、`a2CampusMessage`、`a1OnCampus`、`a2OnCampus`。

### 自动登录两阶段（`auto_auth.rs:220-469`）

启动登录先做环境准备（初始延迟 → 强制刷适配器 → 自启重试 → 校园网校验/静默期跳过），再进入 Portal 探测：已在线则只补状态与 `emit(true, msg, true)`；未在线才走 `full_login`（互斥 + `last_auto_login_attempt`），成功后联动 `start_auto_exit`。准备阶段（巡检触发的 `try_auto_login_on_preparation`）与重连（`try_disconnect_reconnect`）共用 `is_logging_in` 互斥、`prep_login_failures`(上限 5)、冷却与注销保护门。

### 夜间出站切换/还原链（`scheduled.rs`）

`outbound_action_for`（`:276-301`，清理分支优先、hold 冻结）→ `apply_outbound_switch`（`:538-772`，三快照落盘为准：跃点/禁用名单/兜底路由 → 写跃点 → 禁用校园卡 → 加兜底路由 → GetBestRoute 验证）→ 失败留下 `OUTBOUND_SWITCH_FAIL_COUNT` 驱动 `replay_outbound_switch`（`:787-888` 三分支独立幂等）与 `watchdog_re_disable_campus`（`:897-957`）→ 早晨恢复窗或功能关闭时 `apply_outbound_restore`（`:969-1204`：删路由 → 写回快照原值跃点 → 启用网卡 → `finish_outbound_restore` 清快照/退避/hold，`:1211-1221`）。启动时 `reconcile_outbound_on_startup`（`:1313-1371`）对账；手动入口 `manual_outbound_switch`/`manual_outbound_restore`（`:1271-1306`）。退避：60s 起步翻倍封顶 300s（`:317-324`）；还原告警首达 3 次后每 20 次提醒（`:84-87`）；名单损坏失败达 40 次按空名单放行收尾（`:1103-1114`）。

### 网络质量链

`spawn_latency_test_loop`（`latency.rs:53-123`，间隔动态）→ `run_quality_check`（`quality_scheduler.rs:53-118`）→ `perform_quality_check`（`:19-45`，互斥 + `append_quality_history` + `emit_network_quality_result`）→ bad 档两轮 15s 复核确认 → `notify_quality_change`（`latency.rs:43-51`）。巡检循环不碰质量指标（`background_check.rs:367-369`）。

### 适配器监听链

`start_adapter_watch`（`adapter_watch.rs:17-288`）15s 一拍：DNS 缓存清理 → 每 4 轮刷新 class subkey 缓存 → 读适配器快照 → 差分比较 → `emit_adapters_changed`/`emit_adapter_details_changed`/`emit_disabled_adapters_changed` → 恢复卡且离线时补跑一轮巡检 → 禁用告警（60s 节流）→ 自动启用（guard 窗 ∧ 切换态跳过；夜切名单拦截；退避阶梯 0/60s/120s/300s；失败后才允许 UAC）。

### Portal 常驻监测链

`start_portal_watch`（`portal_watch.rs:54-88`）300s 一拍：`run_round`（`:92-113`）逐卡 `sample_adapter`（`:118-170`，绑源 ICMP + 绑源 HTTP 页面 + chkstatus 会话）→ `write_jsonl`（`:228-254`）→ `apply_events`（`:289-297`）→ `diff_events`（`:300-325`）状态变化写事件日志（页面断联/恢复、会话掉线/在线、oltime 回退重连、运营商配置变更）。

### 周期与阈值常量

| 常量 | 值 | 位置 |
| --- | --- | --- |
| 巡检默认/下限间隔 | 60000ms / 10000ms（低于回写 15000） | `background_task.rs:10-12` |
| 适配器监听间隔 | 15000ms | `adapter_watch.rs:9` |
| class subkey 缓存刷新 | 每 4 轮（60s） | `adapter_watch.rs:15` |
| 定时动作 tick | 30000ms | `scheduled.rs:49` |
| 质量循环默认间隔 | 600s（`latency_test_interval` 默认，下限 10s，轻量态放宽） | `latency.rs:66-73` |
| 质量复核 | 2 轮 × 15s | `quality_scheduler.rs:12-13` |
| Portal 常驻监测采样 | 300s（请求 3s / 客户端 8s 超时） | `portal_watch.rs:25,28,31` |
| 出站 helper 超时 | 30s | `scheduled.rs:59` |
| 出站退避 | 60s 起步翻倍封顶 300s | `scheduled.rs:69-70` |
| 出站还原告警 | 首达 3 次、此后每 20 次 | `scheduled.rs:75,77` |
| 出站还原放弃阈值 | 40 次失败（仅限名单损坏场景放行收尾） | `scheduled.rs:81` |
| 夜切验证 | 等 15s、3 次重试间隔 5s、HTTP 15s | `scheduled.rs:53-56` |
| 兜底路由 metric | 2 | `scheduled.rs:65` |
| 自动启用退避 | 0 / 60s / 120s / 300s | `adapter_watch.rs:292-299` |
| 准备登录失败上限 | 5 次 | `auto_auth.rs:19` |
| 重连提醒周期 | 每 10 次 | `auto_auth.rs:14` |
| 离线/禁用通知节流 | 60s | `background_emit.rs:94-101`、`adapter_watch.rs:122-131` |
| 注销保护期 | 注销后 60s | `scheduled.rs:1526-1532` |
| 启动登录延迟 | 自启 5000ms / 手动 1500ms | `auto_auth.rs:229` |

## Connections

- [[desktop-auth]] — `full_login`/`full_logout`、`check_portal_full`、`read_bounded_body`、`post_login_handler`、`append_login_history`、`operator_suffix_from_uid`、`uid_matches`、Portal 探测协议（巡检与启动登录的登录动作全部经此层）。
- [[desktop-network-core]] — 适配器枚举与强制刷新（`get_adapters_cached`/`_force`/`_cached_async`）、`resolve_adapter_names`/`find_dual_adapters`/`filter_operation_adapters`、`enable_adapter`/`disable_adapter`、`read_interface_metrics`/`best_route_if_index_v4`、网关探测（`check_gateway_reachable`/`check_gateway_reachable_from`）、`is_same_subnet_18`、`get_wireless_ssid`/`get_wired_network_profile`。
- [[desktop-network-dns]] — `cleanup_expired_dns_cache`（`adapter_watch.rs:43` 每轮调用）。
- [[desktop-network-quality]] — `check_network_quality_async`、`select_adapter`、`append_quality_history`、`emit_network_quality_result`、轻量态判定 `is_lightweight_active()` 与 `effective_background_interval_ms`/`effective_quality_interval_ms`。
- [[desktop-config]] — `config.load_full`/`update`/`update_with_result`/`save_config_to_disk_encrypted`、`ScheduledFired`（`state.scheduled`）、夜切/出站配置字段（`night_operator_restore`、`outbound_priority`、`outbound_disabled_adapters`、`outbound_manual_hold_day`、恢复窗常量）、`config::schedule::should_fire_scheduled_action`、`config::night_switch::{parse_chkstatus, evaluate_night_outbound, is_night_outbound_guard_window, night_switch_*}`、`default_portal_url`。
- [[desktop-commands]] — `start_background_check_inner` 由命令层触发启停；`manual_outbound_switch`/`manual_outbound_restore`（"立即切换/还原"命令入口）；手动质量检测命令不经过 `run_quality_check`。
- [[desktop-app-lifecycle]] — `is_quitting`、启动挂载 `run_startup_tasks`、`start_auto_exit`/`auto_exit_cancelled`/`set_deadline`。
- [[desktop-infra]] — `task_manager`（`spawn`/`cancel_token`/`is_running`）、`EventBus`（`emit_background_check_result`/`emit_adapters_changed`/`emit_disabled_adapters_changed`/`emit_adapter_details_changed`/`emit_adapter_disabled_warning`/`emit_network_quality_result`/`emit_login_log`）、`emit_notification`、`infra::logger::{get_log_dir, get_log_retention_days}`、`ok_msg`、`failure_tracker::reset_all`、`spawn_elevated_helper`、`CancellationToken`。
- [[desktop-platform]] — Windows 侧提权与注册表动作：`registry::refresh_class_subkey_cache`、`read_pnp_instance_id`（`unsafe_to_disable` 总线判定）、CMSTPLUA/UAC 降级策略；非 Windows 上相关分支为空实现。
- [[desktop-frontend-hooks]] — 前端消费上述事件与轮询状态（巡检结果、适配器变更、质量结果、通知）。
- [[android-backend]] — 安卓侧独立的 `android/src-tauri/src/monitor_loop.rs`，与本模块无共享代码；本模块在安卓 target 不编译。
- [[outbound-switch]] — 夜间出站切换的决策与窗口设计（concepts）；`outbound_switch.rs` 是其桌面侧动作层（目标卡选择、逐卡校园网判定、快照序列化）。

## Known Issues

1. **巡检单轮同步阻塞**：整轮巡检跑在 `spawn_blocking` 线程（`background_check.rs:372-374`），Portal HTTP 请求同步等待（双卡并行 `background_check.rs:163-175`、其余串行），Portal 响应慢或超时会拉长单轮、放大周期抖动。
2. **校园网网关探测缓存共享**：`check_gateway_reachable` 系 network 层共享缓存，巡检轮内 `campus_check.rs:79-95` 只做单轮内单次缓存；缓存 TTL 内网关状态变化感知滞后，静默期/出站态跳过轮次期间尤为明显。
3. **静默期单边退化语义**：`campus_check.rs:272-280` 中 `end <= start` 退化为只按 start 门控的单边窗口（测试 `campus_check.rs:316-320` 锁定该行为）；跨零点停机窗（如 23:00–06:00）无法直接表达。
4. **启动登录仍两次取时**：`run_auto_login_on_start` 校园网校验处 `now_min` 取自两次 `chrono::Local::now()`（`auto_auth.rs:288`），跨分钟边界时静默判定可能与单次语义不一致；巡检侧已在 `background_check.rs:46-47` 改为单次。
5. **`--autostart` 字面量匹配**：`auto_auth.rs:228` 以 `args().any(|a| a == "--autostart")` 精确匹配，`--autostart=1` 或大小写/路径变体不会被识别为自启场景（初始延迟与重试策略随之不同）。
6. **魔法数散布**：启动登录延迟 1500/5000ms（`auto_auth.rs:229`）、自启重试 3 次 × 3s（`auto_auth.rs:266-281`）、注销保护 60s（`scheduled.rs:1526-1532`）、通知节流 60000ms（`background_emit.rs:94-101`、`adapter_watch.rs:122-131`）、恢复后补检 5s（`adapter_watch.rs:247-256`）等以字面量散布，未集中为命名常量。
7. **节流丢事件**：离线变更通知 60s 节流（`background_emit.rs:94-101`）与禁用告警 60s 节流（`adapter_watch.rs:122-131`）在密集翻转时静默丢弃部分事件，仅保证最后一次状态可见。
8. **自动启用失败无放弃阈值**：`enable_adapter` 失败退避阶梯封顶 300s（`adapter_watch.rs:292-299`）但失败计数无上限、无 give-up；UAC 长期被拒时最多每 5 分钟重试一次（对比出站还原有 40 次放弃阈值 `scheduled.rs:81`）。
9. **出站切换以磁盘快照为唯一真源**：`apply_outbound_switch` 三快照落盘失败即整链失败且不进内存（`scheduled.rs:655-664`）；还原收尾同样依赖 `clear_outbound_snapshot` 落盘成功（`scheduled.rs:1225-1243`），`save_config_to_disk_encrypted` 故障期间切换/还原/重放全部受阻。
10. **give-up 放行的残余风险**：还原时禁用名单损坏且失败达 `OUTBOUND_RESTORE_GIVE_UP_FAILS`(40) 次后按空名单放行收尾（`scheduled.rs:1103-1114`），校园网卡可能保持禁用，仅收到一次告警。
11. **路由验证仅 IPv4**：`apply_outbound_switch` 的 GetBestRoute 验证只查 `best_route_if_index_v4`（`scheduled.rs:739-766`），IPv6 默认路由是否切换成功不在验证范围。
12. **Portal 常驻监测的延迟与静默失败**：300s 采样粒度（`portal_watch.rs:25`）使事件最多延迟 5 分钟；`fetch_text` 对非 2xx/请求失败只记 debug（`portal_watch.rs:189-197`），连续故障无告警；JSONL 清理跟随日志保留天数（`portal_watch.rs:257-285`），`retention=0` 时文件永久累积。
13. **质量复核互斥冲突时静默放弃**：复核轮拿不到 `is_quality_checking` 锁按"未确认"处理，不通知也不落状态（`quality_scheduler.rs:96-101`），极端情况下一次真实拥堵可能不触发"网络拥堵"通知。
14. **主动跳过/改写清单**：出站切换态跳过整轮（`background_check.rs:50-57`）、静默期直接置在线（`background_check.rs:58-82`）、注销保护期强制 `online=false` 并对 `onlineOperator` 置 None（`background_emit.rs:127-134`）、`update_network_state` 保护期跳过（`background_emit.rs:177-183`）——前端在这些窗口看到的是受控状态而非实测结果。
15. **helper 通道失败只告警**：`run_helper_op` 的 UAC/超时/结果文件缺失类失败记入 `OUTBOUND_CHANNEL_FAIL_COUNT` 并走退避 + 告警（`scheduled.rs:419-445`、`:369-373`），无跨拍自动重试编排，依赖下一拍重放；适配器查询失败同样只 `log_warn` 等下一轮（`adapter_watch.rs:277-284`）。
