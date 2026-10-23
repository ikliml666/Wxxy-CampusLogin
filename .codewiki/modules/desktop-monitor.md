---
title: 后台巡检：连通性检测、自动登录/断线重连与网络质量调度
type: module
source_files:
  - tauri-app/src-tauri/src/monitor/mod.rs
  - tauri-app/src-tauri/src/monitor/watcher.rs
  - tauri-app/src-tauri/src/monitor/background_task.rs
  - tauri-app/src-tauri/src/monitor/background_check.rs
  - tauri-app/src-tauri/src/monitor/portal_check.rs
  - tauri-app/src-tauri/src/monitor/auto_auth.rs
  - tauri-app/src-tauri/src/monitor/latency.rs
  - tauri-app/src-tauri/src/monitor/quality_scheduler.rs
  - tauri-app/src-tauri/src/monitor/scheduled.rs
  - tauri-app/src-tauri/src/monitor/outbound_switch.rs
tags: [monitor, background-check, auto-login, reconnect, campus-network, portal, latency, quality, scheduled-actions, night-outbound, tauri-events]
---

## Overview

本模块是桌面端的后台巡检中枢，由 `monitor/mod.rs` 声明 13 个子模块组成（含 `watcher`、`background_task`、`background_check`、`portal_check`、`auto_auth`、`latency`、`quality_scheduler`、`scheduled`、`outbound_switch` 等，以及观察未附源码的 `adapter_watch`、`campus_check`、`background_emit`、`outbound_night`）。启动期由 `watcher::run_startup_tasks` 统一编排后台巡检、质量测试、自动登录与定时动作循环。核心巡检逻辑在 `background_check` 中独占连通性/Portal/重连，质量检测由 `latency`+`quality_scheduler` 独立循环负责，定时登录/注销与夜间出站切换编排在 `scheduled` 中 30s 循环，底层动作层 `outbound_switch` 提供候选卡选择与快照序列化。模块整体在桌面端编译，安卓 target 不编译本模块（由 `#[cfg(desktop)]` 门控于上层 lib.rs，此处观察未覆盖具体行号）。

## Key Components

### 模块索引与启动编排（`monitor/mod.rs`、`monitor/watcher.rs`）

- `monitor/mod.rs:1-13` 声明子模块：`watcher`(1)、`auto_auth`(2)、`latency`(3)、`adapter_watch`(4)、`campus_check`(5)、`outbound_switch`(6)、`portal_check`(7)、`quality_scheduler`(8)、`background_emit`(9)、`background_check`(10)、`background_task`(11)、`outbound_night`(12)、`scheduled`(13)。
- `pub use super::background_task::start_background_check_inner;` — `monitor/watcher.rs:10`。
- `pub use super::campus_check::check_campus_network;` — `monitor/watcher.rs:13`。
- `pub use super::background_emit::{adapter_status_entry, adapter_disabled_entry, adapter_disconnected_entry};` — `monitor/watcher.rs:14`。
- `pub fn run_startup_tasks(app_handle: &AppHandle)` — `monitor/watcher.rs:16`，启动期统一编排入口：
  - `config.enable_background_check` 真时 spawn `startup_bg_check` 调 `start_background_check_inner`（`monitor/watcher.rs:21-25`）。
  - `enable_network_quality && enable_latency_test` 时 spawn `startup_latency` 调 `spawn_latency_test_loop`（`monitor/watcher.rs:33-37`）。
  - 仅 `enable_network_quality` 时 spawn `startup_quality_once` 调 `run_startup_quality_once`（`monitor/watcher.rs:43-48`）。
  - 无条件 spawn `startup_auto_login` 调 `run_auto_login_on_start`（`monitor/watcher.rs:56`）。
  - 无条件 spawn `scheduled_actions` 调 `scheduled::run_scheduled_action_loop`（`monitor/watcher.rs:65`）。
  - 各任务注册失败仅 `log_warn!` 不阻断（`monitor/watcher.rs:29,41,50,58,67`）。

### 巡检任务生命周期（`monitor/background_task.rs`）

- `pub fn start_background_check_inner(app_handle: &AppHandle, state: &AppState) -> Result<CommandResult, String>` — `monitor/background_task.rs:7-64`：置 `enable_background_check=true`，间隔 `<10000` 提至 `15000`（`background_task.rs:10-12`）；spawn `background_check` 任务首启直检，循环动态读 `effective_background_interval_ms`（轻量化下限300s）作间隔（`background_task.rs:59-83`）；落盘 `save_config_and_broadcast`（`background_task.rs:59`）。
- `pub fn spawn_background_check_once(app_handle: &AppHandle)` — `monitor/background_task.rs:74-94`：spawn `background_check_once` 调 `run_background_check`（`background_task.rs:79`），同名已跑复用 cancel_token 补跑（`background_task.rs:82-93`）。
- 常量 `POST_RESTORE_IP_WAIT_MAX_MS: u64 = 45_000`（`background_task.rs:108`）、`POST_RESTORE_IP_POLL_MS: u64 = 3_000`（`background_task.rs:109`）。
- `fn post_restore_wait_should_continue(waited_ms: u64, max_ms: u64) -> bool` — `monitor/background_task.rs:115` 纯函数，`waited_ms < max_ms`。
- `fn post_restore_adapters_have_ip(app_handle: &AppHandle) -> bool` — `monitor/background_task.rs:123` 用 `get_adapters_force` 枚举主副适配器是否拿到 IP。
- `pub fn spawn_background_check_after_outbound_restore(app_handle: &AppHandle)` — `monitor/background_task.rs:145-179`：阻塞线程池轮询等 IP 就绪（最长上限），结束后调 `spawn_background_check_once`（`background_task.rs:177`）；join 失败按等满上限处理。
- 测试：`post_restore_wait_stops_at_cap`（`background_task.rs:187`）、`post_restore_wait_zero_budget_never_waits`（`background_task.rs:196`）。

### 巡检主体与 Portal 检测（`monitor/background_check.rs`、`monitor/portal_check.rs`）

- `pub(crate) fn should_reset_logged_online(reachable: bool, any_online: bool, has_logged_online: bool) -> bool` — `monitor/background_check.rs:25` 纯函数，`reachable && !any_online && has_logged_online`。
- `pub(crate) fn run_background_check_blocking(app_handle: &AppHandle, state: &AppState, cancel_token: &CancellationToken)` — `monitor/background_check.rs:29-395`：退出/取消即返（`:30`）；`is_checking.try_acquire()` 并发跳过（`:33`）；适配器取 `get_adapters_cached` 空则降级 `get_adapters_force`（`:43-52`）；出站切换态 `outbound_restore_active` 跳过本轮（`:64`）；校园网静默期 `is_campus_check_silent` 跳过验证但置 `on_campus=true`（`:72-101`）；失败触发 `start_campus_exit`/`cancel_campus_exit`（`:89,157,164`）；双适配器并行 Portal 检测用 `Handle::current().block_on`+`spawn_blocking`+`tokio::join!`（`:177-189`）；Portal 请求失败调 `handle_portal_request_failure` 累加计数（`:213-234`）；成功重置失败计数（`:243-261`）；`should_reset_logged_online` 判定重置 `has_logged_online`（`:334`）；调 `handle_status_change`（`:341`）与 `emit_background_check_result` 使用 `BackgroundCheckResult` 结构（`:357`）；调 `try_auto_login_on_preparation`（`:380`）与 `try_disconnect_reconnect`（`:382`）；仅未重连时 `update_network_state`（`:390-392`）。
- `pub async fn run_background_check(app_handle: &AppHandle, cancel_token: Arc<CancellationToken>)` — `monitor/background_check.rs:400-408`：`spawn_blocking` 调 blocking 版，异常记日志。
- 测试：`no_reset_while_online`（`:416`）、`no_reset_when_unreachable`（`:424`）、`no_reset_when_never_logged_in`（`:430`）、`reset_when_reachable_offline_and_logged_before`（`:436`）。
- `pub(super) enum PortalCheckResult` — `monitor/portal_check.rs:6-19`：变体 `Success{online,message,reachable,login_available,uid:Option<String>}`（`:7-14`）、`Error{is_request_failed:bool}`（`:15-17`）、`NotFound`（`:18`）。
- `impl PortalCheckResult` — `monitor/portal_check.rs:21-57`：方法 `online()`（`:22`）、`message()`（`:29`）、`reachable()`（`:37`）、`login_available()`（`:44`）、`uid()`（`:51`）。
- `pub(super) fn check_adapter_portal(adapter: &Adapter, app_handle: &AppHandle) -> PortalCheckResult` — `monitor/portal_check.rs:59-93`：调 `check_portal_full`，`error_kind=="request_failed"` 返回 `Error{is_request_failed:true}` 并 emit 日志（`:65-72`）；异常返回 `Error{is_request_failed:false}`（`:90`）。

### 自动登录与断线重连（`monitor/auto_auth.rs`）

- 常量 `RECONNECT_REMINDER_INTERVAL: u32 = 10`（`auto_auth.rs:14`）、`PREP_LOGIN_MAX_FAILURES: u32 = 5`（`auto_auth.rs:19`）。
- `pub fn reconnect_should_report(login_success: bool, within_limit: bool) -> bool` — `auto_auth.rs:25` 返回 `login_success && within_limit`。
- `pub fn try_auto_login_on_preparation(app_handle, state, login_available: bool, online: bool, config: &Config)` — `auto_auth.rs:29-93`：门控 `!login_available||online||!auto_login_on_preparation`（`:36`）；`has_logged_online` 跳过（`:42`）；连续失败 `prep_login_failures>=PREP_LOGIN_MAX_FAILURES` 停（`:48`）；注销保护期/冷却期检查（`:57-67`）；`is_logging_in.try_acquire()` 后 `full_login`（`:70-73`）；成功置 `has_logged_online=true` 且失败计数清零（`:90-93`）。
- `pub fn try_disconnect_reconnect(app_handle, state, online: bool, secondary_online: Option<bool>, a1: Option<&Adapter>, adapter1_name: &str, adapter2_name: &str, reachable: bool, login_available: bool, config: &Config) -> bool` — `auto_auth.rs:110-217`：非 `any_adapter_online` 等条件返 false（`:126`）；获登录锁后 `disconnect_reconnect_count` 自增（`:152`）；超限释放锁并通知（`:196-210`）；返回 `reconnect_should_report`（`:217`）。
- `pub fn run_auto_login_on_start(app_handle: &AppHandle)` — `auto_auth.rs:220-463`：`auto_login_on_start` 门控（`:224`）；`--autostart` 决定延迟 5000/1500ms（`:228-229`）；自启时适配器未就绪重试3次（`:266-281`）；校园网静默/检测逻辑（`:287-338`）；双适配器并行 Portal 检测（`:353-366`）；已在线跳过（`:381-406`）；`full_login` 执行（`:421-432`）；成功可 `start_auto_exit`（`:460-463`）。
- 测试：`reconnect_success_within_limit_reports_true`（`:476`）、`reconnect_failure_never_reports_true`（`:482`）、`reconnect_success_over_limit_reports_false`（`:488`）。

### 延迟与质量调度（`monitor/latency.rs`、`monitor/quality_scheduler.rs`）

- 常量 `BAD_LEVELS: &[&str] = &["poor","bad"]`（`latency.rs:12`）、`GOOD_LEVELS: &[&str] = &["excellent","great","good"]`（`latency.rs:13`）。
- `pub(super) fn classify_quality_change(last: Option<&str>, current: &str, enable_notification: bool) -> Option<&'static str>` — `latency.rs:18` 返回 `Some("bad")`/`Some("good")`/`None`。
- `pub(super) fn record_last_quality(state: &AppState, current: &str)` — `latency.rs:40`。
- `pub(super) fn notify_quality_change(app_handle: &AppHandle, kind: &str)` — `latency.rs:44` bad/good 发不同通知。
- `pub(super) async fn wait_for_adapter_ready(app_handle, cancel_token) -> Option<(String, String)>` — `latency.rs:67` 每2s重试等 IP 与 `any_adapter_online`，取消返 None。
- `pub fn spawn_latency_test_loop(app_handle: &AppHandle) -> Result<(), String>` — `latency.rs:78-139`：动态间隔（轻量化下限1800s），首 tick 吞掉，调 `run_quality_check`（`:135`）。
- `pub(super) async fn run_startup_quality_once(app_handle: AppHandle, cancel_token: Arc<CancellationToken>)` — `latency.rs:144-160`：启动即测一次，等待就绪与1s稳定后调 `run_quality_check`（`:159`）。
- 测试：`degraded_to_bad_level_triggers_pending_bad`（`:167`）、`recovery_from_bad_level_triggers_good`（`:173`）、`same_level_or_unknown_last_never_notifies`（`:179`）、`middle_levels_crossing_never_notifies`（`:186`）、`notification_disabled_never_classifies`（`:195`）。
- 常量 `SPIKE_CONFIRM_COUNT: usize = 2`（`quality_scheduler.rs:12`）、`SPIKE_CONFIRM_INTERVAL_SECS: u64 = 15`（`quality_scheduler.rs:13`）。
- `async fn perform_quality_check(app_handle, adapter_name: &str, adapter_ip: &str, lightweight: bool) -> Option<serde_json::Value>` — `quality_scheduler.rs:25-51`：`is_quality_checking.try_acquire()` 互斥（`:25`），调 `check_network_quality_async`（`:29`），落盘历史与 emit 结果。
- `pub(super) async fn run_quality_check(app_handle, adapter_name: &str, adapter_ip: &str, cancel: Option<&CancellationToken>)` — `quality_scheduler.rs:53-118`：首次检测后 `classify_quality_change` 若 `Some("bad")` 触发复核：15s 间隔轻量化再测2次（`:70-103`），全确认才 `notify_quality_change("bad")`（`:104-105`）；其余 kind 直接通知并记录（`:111-116`）。

### 定时动作与夜间出站编排（`monitor/scheduled.rs`）

- 常量 `SCHEDULED_TICK_MS: u64 = 30_000`（`scheduled.rs:41`）、`NIGHT_VERIFY_WAIT_SECS: u64 = 15`（`scheduled.rs:45`）、`NIGHT_VERIFY_RETRY_SECS: u64 = 5`（`scheduled.rs:46`）、`NIGHT_VERIFY_ATTEMPTS: u32 = 3`（`scheduled.rs:47`）、`NIGHT_VERIFY_HTTP_TIMEOUT_SECS: u64 = 15`（`scheduled.rs:48`）。
- `run_scheduled_action_loop`（pub async）— `scheduled.rs:51-189`：启动前 `reconcile_outbound_on_startup`（来自 super::outbound_night，`:58`）与 `reconcile_scheduled_logout_on_startup`（`:60`）；循环内 tokio::select 等 tick/cancel（`:63-69`）；`is_quitting` 判断退出（`:74`）；算 now_minutes/today_day/config_snapshot；用拍前 `outbound_switch_active`（`:87`）门控 `evaluate_scheduled`（`:88-91`）得 (fire_login,fire_logout) 调 `run_scheduled_login`/`run_scheduled_logout`；再算 restore_active 调 `evaluate_outbound_action`（`:106-118`）匹配 `NightOutboundAction` 三分支经 `run_outbound_blocking` 执行 `apply_outbound_switch`/`apply_outbound_restore` 或 replay/watchdog（`:119-159`）；拍后取 `outbound_active`（`:163`）门控 `evaluate_night_switch_gated`（`:167-180`）得 night_action 调 `apply_night_switch_action`（`:182`）。
- `is_quitting` — `scheduled.rs:189` 抽离避免跨 await 持 State 守卫。
- `reconcile_scheduled_logout_on_startup` — `scheduled.rs:201` 冷启动若 now_minutes>=scheduled_logout_minutes 置 logout_day 消耗额度不执行。
- `evaluate_scheduled` — `scheduled.rs:225-227` 装配 AppState 调共享纯函数 `evaluate_scheduled_actions`。
- `mark_scheduled_day_fired` — `scheduled.rs:243`（`#[cfg_attr(not(test), allow(dead_code))]`）置 login_day/logout_day。
- `format_minutes` — `scheduled.rs:252` 补零格式化 mm:ss。
- `run_scheduled_login` — `scheduled.rs:259` 调 `spawn_login_action` 传 Some(today_day)。
- `spawn_login_action` — `scheduled.rs:272-299`：抢 `tasks.is_logging_in` 锁（`:278`），抢到后若 login_quota_day 有值置额度（`:289`），调 `auth::service::full_login`（`:292`）与 `post_login_handler`（`:299`），emit 日志。
- `apply_night_switch_action` — `scheduled.rs:307-333`：改内存 config（SwitchToCampus 将原 operator 存 night_operator_restore 并置空串=无锡学院，Restore 反之）（`:312-320`），调 `save_config_and_broadcast` 落盘（`:322`）失败则不 store 返回；成功 store 后调 `spawn_login_action`（None 额度）并 spawn `verify_night_switch`（`:329-333`）。
- `run_scheduled_logout` — `scheduled.rs:341` spawn_blocking 调 `perform_logout`。
- `perform_logout` — `scheduled.rs:352-385`：抢 `tasks.is_logging_out` 锁（`:355`），成功后置 logout_day 额度（`:364`），调 `full_logout`（`:367`）；成功则取消自动退出、重置 failure_tracker、启动 60s 注销保护期（`:374-385`）。
- `verify_night_switch` (async) — `scheduled.rs:392-426`：等 15s，`night_verify_config` 检查 operator 仍匹配否则跳过（`:394,433`）；`night_switch_verify_round` 重试 3 次间隔 5s（`:397,441`）；不符则注销重登再验（`:404-411`），最终不符发告警日志+`emit_notification`（`:419-426`）。
- `night_verify_config` — `scheduled.rs:433` operator != expected_operator 返 None 跳过验证。
- `night_switch_verify_round` (async) — `scheduled.rs:441-462` 用 `create_safe_http_client` 做 NIGHT_VERIFY_ATTEMPTS 轮。
- `night_switch_check_once` (async) — `scheduled.rs:462-485` 构造 chkstatus URL（经 `portal_origin`），parse_chkstatus+uid_matches 核对。
- `portal_origin` (pub(crate)) — `scheduled.rs:490` 取 portal_url 的 origin，空值回退 default_portal_url。
- `expected_operator_display` — `scheduled.rs:503` 空串显“无锡学院”。
- 测试模块（`scheduled.rs:511+`）：`禁用目标_不触发_不置标记`（`:519`）、`到点触发_置当日标记`（`:529`）、`跨天重置_次日再触发`（`:551`）、`切换态_跳过定时登录且不消耗当日标记_注销照常`（`:561`）、`format_minutes_补零`（`:590`）。

### 夜间出站切换动作层（`monitor/outbound_switch.rs`）

- `pub fn is_campus_adapter(ip, gateway, campus_gateway, portal_host, gateway_probe: Fn(&str,&str)->bool) -> bool` — `outbound_switch.rs:17-38`：IP 空否（`:24`）；同 /18 真；否则绑源探校园网关或 portal 两发去抖任一可达即真（`:30-38`）。
- `pub fn portal_probe_host(portal_url) -> String` — `outbound_switch.rs:44-56` 从 portal_url 提取私网 IPv4 主机，域名/公网/空返空串。
- `pub fn effective_outbound_priority(priority: &[String], details: &[(Adapter,String)]) -> Vec<String>` — `outbound_switch.rs:63-82` 非空透传，空则无线优先后有线保发现序。
- `pub fn select_outbound_candidate` — `outbound_switch.rs:87-111` 按优先级选首张非校园、有 IP、有网关且自身网关可达卡 -> Option<Adapter>。
- `struct SnapshotRow` (内部 derive Serialize/Deserialize) — `outbound_switch.rs:115` 含字段 `guid/family/automatic/metric`，序列化时补 guid。
- `pub fn snapshot_json` — `outbound_switch.rs:125-135` 序列化选中卡跃点快照，兜底 `"[]"`。
- `pub struct DisabledRow` — `outbound_switch.rs:142` 含字段 `guid/name`，记录本功能亲手禁用卡。
- `pub fn disabled_adapters_json` — `outbound_switch.rs:148` 序列化禁用名单，兜底 `"[]"`。
- `pub fn parse_disabled_adapters` — `outbound_switch.rs:156-161` 空串->空名单 Ok，非法 JSON->Err。
- `pub struct StandbyRoute` — `outbound_switch.rs:169` 含字段 `dest/mask/gateway/metric/if_index`，历史兜底默认路由快照。
- `pub fn parse_standby_route` — `outbound_switch.rs:179-181` 空/非法->None。
- `pub enum StandbyCleanup` — `outbound_switch.rs:187` 变体 `AlreadyGone/ForeignRowKept/LastDefaultPath/Safe`。
- `pub fn standby_cleanup_decision` — `outbound_switch.rs:203-220` 纯函数，按 same_key_metrics/standby_metric/rows_outside_exact 判。
- `pub fn standby_route_cleanup` — `outbound_switch.rs:224 (windows) / :268 (not windows)`：非 windows 返 AlreadyGone（`:268-270`）。
- `pub(crate) fn unsafe_to_disable` (cfg windows) — `outbound_switch.rs:278-280` 实例 ID 读不到返 true 保守跳过。
- `pub fn select_campus_to_disable(priority, details, campus_gateway, portal_host, exclude_guid, gateway_probe, bus_guard) -> Vec<DisabledRow>` — `outbound_switch.rs:285-318` 过滤目标卡/无 IP/非校园/guard 命中，同名去重。
- 测试模块（`outbound_switch.rs:346+`）：`campus_adapter_by_subnet`(`:346`)、`campus_adapter_off_subnet_probe_reachable`(`:352`)、`campus_adapter_off_subnet_probe_unreachable`(`:358`)、`campus_adapter_empty_ip_is_false`(`:364`)、`campus_adapter_empty_gateway_skips_probe`(`:369`)、`candidate_skips_campus_and_ipless_and_out_of_list`(`:375`)、`candidate_probe_reachable_means_campus_skipped`(`:392`)、`candidate_priority_name_absent_from_details_skips`(`:399`)、`candidate_empty_priority_returns_none`(`:409`)、`effective_priority_non_empty_passthrough`(`:417`)、`effective_priority_empty_defaults_wireless_first`(`:428`)、`effective_priority_empty_multiple_wireless_keep_discovery_order`(`:442`)、`effective_priority_empty_no_adapters_is_empty`(`:456`)、`candidate_skips_gatewayless_and_dead_gateway_cards`(`:462`)、`campus_probe_single_loss_still_counts_as_campus`(`:481`)、`snapshot_json_round_trip_with_guid`(`:492`)、`snapshot_json_empty_rows`(`:507`)、`disabled_adapters_json_round_trip`(`:513`)、`standby_route_snapshot_parses_legacy_config_json`(`:527`)、`select_campus_to_disable_filters_everything_non_campus`(`:547`)、`select_campus_to_disable_probe_reachable_counts_and_dedups`(`:577`)、`select_campus_to_disable_unknown_bus_is_skipped`(`:595`)、`campus_adapter_portal_probe_reachable`(`:611`)、`campus_adapter_portal_unreachable_stays_non_campus`(`:625`)、`campus_adapter_portal_probe_single_loss_still_counts`(`:638`)、`campus_adapter_empty_gateway_portal_reachable`(`:657`)、`portal_probe_host_extracts_private_ipv4`(`:663`)、`standby_cleanup_never_deletes_foreign_or_last_default_route`(`:674`)。

## Architecture

```text
┌───────────────────────┐
│ watcher.rs:16         │ run_startup_tasks
│ (monitor/watcher.rs)  │ spawns 5 tasks
└──────────┬────────────┘
           │
           ▼
┌──────────────────────────────┐ ┌──────────────────────────────┐
│ background_task.rs:7         │ │ latency.rs:78                │
│ start_background_check_inner │ │ spawn_latency_test_loop      │
│ → background_check.rs:29      │ │ → quality_scheduler.rs:53    │
│ run_background_check_blocking│ │ run_quality_check            │
└──────────┬───────────────────┘ └──────────────────────────────┘
           │ calls
           ▼
┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐
│ portal_check.rs:59│ │ auto_auth.rs:29  │ │ scheduled.rs:51  │
│ check_adapter_    │ │ try_auto_login_  │ │ run_scheduled_   │
│ portal           │ │ on_preparation   │ │ action_loop      │
└──────────────────┘ └──────────────────┘ └────────┬─────────┘
                                                   │ uses
                                                   ▼
                                         ┌──────────────────────┐
                                         │ outbound_switch.rs:17│
                                         │ is_campus_adapter etc│
                                         └──────────────────────┘
```

## Data Flow

### 启动挂载
- `watcher.rs:16` `run_startup_tasks` 依据配置 spawn：`startup_bg_check`（`watcher.rs:16` → `background_task.rs:7`）、`startup_latency`（`watcher.rs:16` → `latency.rs:144`）、`startup_quality_once`（`watcher.rs:48` → `latency.rs:144`）、`startup_auto_login`（`watcher.rs:16` → `auto_auth.rs:220`）、`scheduled_actions`（`watcher.rs:16` → `scheduled.rs:51`）。

### 单轮巡检主链
- `background_task.rs:29-53` 循环调用 `background_check::run_background_check`（`background_check.rs:400` → `:29`）。
- `background_check.rs:33` `is_checking.try_acquire()` 互斥；`:43-52` 适配器 cached/force；`:64` `outbound_restore_active` 跳过；`:72-101` 静默期跳验证置 `on_campus=true`；`:177-189` 双适配器并行 `portal_check::check_adapter_portal`（`:183-184`）；`:213-234` 失败计数；`:380` `auto_auth::try_auto_login_on_preparation`；`:382` `auto_auth::try_disconnect_reconnect`；`:390-392` 未重连时 `update_network_state`（经 `background_emit`）。
- `should_reset_logged_online`（`background_check.rs:25`, `:334`）修复休眠会话失效。

### 质量链
- `latency.rs:78` `spawn_latency_test_loop` 就绪等待后调 `quality_scheduler.rs:53` `run_quality_check`；`:70-103` bad 复核 2×15s；`perform_quality_check`（`:19`）调 `check_network_quality_async` 落盘 emit。

### 定时动作循环
- `scheduled.rs:51` 30s 循环：`:87-91` 定时登录/注销；`:106-159` 出站切换编排调 `super::outbound_night` 动作（底层 `outbound_switch.rs` 选择）；`:163-182` 运营商夜切 `apply_night_switch_action` 改配置并 `verify_night_switch`（`:392`）。

### 夜间出站还原补检
- `background_task.rs:145` `spawn_background_check_after_outbound_restore` 阻塞轮询等 IP 就绪（最长45s）后 `spawn_background_check_once`（`:177`）。

## Connections

- [[desktop-auth]]：`auto_auth.rs:292` 调 `auth::service::full_login`，`scheduled.rs:292` 同；`portal_check.rs:59` 调 `auth::portal::check_portal_full`；`background_check.rs:213` 调 `handle_portal_request_failure`（阈值见 failure_tracker）。
- [[desktop-network-quality]]：`quality_scheduler.rs:29` 调 `check_network_quality_async` 并发 `emit_network_quality_result`。
- [[desktop-config]]：依赖 `config::persist::save_config_and_broadcast`（`background_task.rs:59`、`scheduled.rs:322`）、共享纯函数 `evaluate_scheduled_actions`/`evaluate_outbound_action`/`evaluate_night_switch_gated`（`scheduled.rs:227` 等）。
- [[desktop-infra]]：`infra::events::EventBus` 与 `infra::notification::emit_notification`（`scheduled.rs` 使用）、`CancellationToken` 跨任务传递。
- [[night-outbound-switch]]：`scheduled.rs` 与 `outbound_switch.rs` 为桌面动作层，`outbound_night` 为声明的兄弟模块提供 `apply_outbound_switch`/`apply_outbound_restore` 等（`scheduled.rs:33-37`）。
- [[desktop-network-core]]：`background_check.rs:43` 调 `get_adapters_cached`/`get_adapters_force`，`outbound_switch.rs:11` 用 `crate::network::discovery::Adapter`。

## Known Issues

基于观察，无未解决缺陷报告；以下为已处理的边界条件与非显然行为（均已在代码中实现对应逻辑）：

1. **并发互斥**：`background_check.rs:33` `is_checking.try_acquire()` 安全跳过叠加轮次；`quality_scheduler.rs:25` `is_quality_checking`；`auto_auth.rs:70,142` `is_logging_in` 锁。
2. **适配器获取降级**：cached 空降级 force（`background_check.rs:43-52`）；夜间补检特意用 force 避旧缓存（`background_task.rs:123`）。
3. **出站切换态跳过**：`outbound_restore_active` 真时整轮巡检跳过（`background_check.rs:64-71`）。
4. **校园网静默**：静默期跳过验证但强制 `on_campus=true`（`background_check.rs:72-101`）；`auto_auth` 开机自启同理（观察指 `auto_auth.rs:295` 静默置 true）。
5. **登录标记复位（关键修复）**：`should_reset_logged_online`（`background_check.rs:25,334`）复位 `has_logged_online`，解决整夜休眠后自动登录被永久门控。
6. **自动登录防无限重试**：`PREP_LOGIN_MAX_FAILURES=5`（`auto_auth.rs:110,48`）；`try_disconnect_reconnect` 超限停止（`auto_auth.rs:110-124`）。
7. **重连状态覆盖修复**：`try_disconnect_reconnect` 返回真实 `reconnect_should_report`（`auto_auth.rs:217`），调用方依此跳过旧 Portal 快照覆盖（`background_check.rs:390`）。
8. **Portal 请求失败容错**：`Error{is_request_failed:true}` 累计阈值（`background_check.rs:208-234`），成功重置（`background_check.rs:243-261`）。
9. **夜间出站还原补检缺口**：新增 `spawn_background_check_after_outbound_restore`（`background_task.rs:145-179`）最长等45s轮询IP。
10. **延迟测试动态间隔**：轻量化下限1800s，吞首 tick（`latency.rs:78-139`）。
11. **质量抖动复核**：poor/bad 需15s×2次轻量化复核全达标才通知（`quality_scheduler.rs:65-103`）；中间档互转不通知（`latency.rs:186-191`）。
12. **就绪等待**：`wait_for_adapter_ready` 每2s等IP与在线（`latency.rs:58-76`）。
13. **任务注册失败处理**：watcher 各 spawn 失败仅 `log_warn`（`watcher.rs:29,41,50,58,67`）；`spawn_background_check_once` 复用令牌补跑（`background_task.rs:82-93`）。
14. **双适配器并行实现**：`run_background_check_blocking` 在 spawn_blocking 内用 `Handle::current().block_on`+`join!`（`background_check.rs:177-189`）。
15. **出站与运营商夜切互斥**：`scheduled.rs:87` 拍前出站门控登录，`scheduled.rs:163` 拍后切换态门控夜切。
16. **历史路由清理**：`outbound_switch.rs:203-220` 不再创建 StandbyRoute，仅解析历史并安全决策（2026-10-18 事故后）。