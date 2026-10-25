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
  - tauri-app/src-tauri/src/monitor/campus_check.rs
  - tauri-app/src-tauri/src/monitor/background_emit.rs
  - tauri-app/src-tauri/src/monitor/outbound_night.rs
  - tauri-app/src-tauri/src/monitor/adapter_watch.rs
tags: [monitor, background-check, auto-login, reconnect, campus-network, portal, latency, quality, scheduled-actions, night-outbound, tauri-events]
---

## Overview

本模块是桌面端的后台巡检中枢。`monitor/mod.rs:1-13` 声明子模块：`watcher`(1)、`auto_auth`(2)、`latency`(3)、`adapter_watch`(4)、`campus_check`(5)、`outbound_switch`(6)、`portal_check`(7)、`quality_scheduler`(8)、`background_emit`(9)、`background_check`(10)、`background_task`(11)、`outbound_night`(12)、`scheduled`(13)；`background_task` 模块在 `mod.rs:11` 逐行列出，且由 `watcher.rs:10` 的 `pub use super::background_task::start_background_check_inner;` 引用并提供独立源文件 `background_task.rs`。启动期由 `watcher::run_startup_tasks`（`watcher.rs:4`）统一编排后台巡检、质量测试、自动登录与定时动作循环。核心巡检逻辑在 `background_check` 中独占连通性/Portal/重连，质量检测由 `latency`+`quality_scheduler` 独立循环负责，定时登录/注销与夜间出站切换编排在 `scheduled` 中 30s 循环，底层动作层 `outbound_switch` 与 `outbound_night` 提供候选卡选择、快照序列化与提权执行。模块整体在桌面端编译，非 Windows 平台对部分提权/路由能力做 stub 或空实现（如 `outbound_switch.rs:139-141`、`outbound_night.rs:1-6`）。

## Key Components

### 模块索引与启动编排（`monitor/mod.rs`、`monitor/watcher.rs`）

- `monitor/mod.rs:1-13` 声明子模块（见 Overview 列表）。
- `pub use super::background_task::start_background_check_inner;` — `monitor/watcher.rs:10`。
- `pub use super::campus_check::check_campus_network;` — `monitor/watcher.rs:13`。
- `pub use super::background_emit::{adapter_status_entry, adapter_disabled_entry, adapter_disconnected_entry};` — `monitor/watcher.rs:14`。
- `pub fn run_startup_tasks(app_handle: &AppHandle)` — `monitor/watcher.rs:16-69`，启动期统一编排入口：
  - `config.enable_background_check` 真时 spawn `startup_bg_check` 调 `start_background_check_inner`（`watcher.rs:21-31`）。
  - `enable_network_quality && enable_latency_test` 时 spawn `startup_latency` 调 `spawn_latency_test_loop`（`watcher.rs:33-42`）。
  - 仅 `enable_network_quality` 时 spawn `startup_quality_once` 调 `run_startup_quality_once`（`watcher.rs:43-52`）。
  - 无条件 spawn `startup_auto_login` 调 `run_auto_login_on_start`（`watcher.rs:54-59`）。
  - 无条件 spawn `scheduled_actions` 调 `scheduled::run_scheduled_action_loop`（`watcher.rs:61-68`）。
  - 各任务注册失败仅 `log_warn!` 不阻断（`watcher.rs:29,41,50,58,67`）。

### 巡检任务生命周期（`monitor/background_task.rs`）

- `pub fn start_background_check_inner(app_handle: &AppHandle, state: &AppState) -> Result<CommandResult, String>` — `background_task.rs:7-64`：置 `enable_background_check=true`，间隔 `<10000` 提至 `15000`（`background_task.rs:10-13`）；spawn `background_check` 任务首启直检，循环动态读 `effective_background_interval_ms`（轻量化下限300s）作间隔（`background_task.rs:61-85`，其中 `:34-37` 读取）；落盘 `save_config_and_broadcast`（`background_task.rs:61`）。
- `pub fn spawn_background_check_once(app_handle: &AppHandle)` — `background_task.rs:74-94`：spawn `background_check_once` 调 `run_background_check`（`background_task.rs:79`），同名已跑复用 cancel_token 补跑（`background_task.rs:82-93`）。
- 常量 `POST_RESTORE_IP_WAIT_MAX_MS: u64 = 45_000`（`background_task.rs:108`）、`POST_RESTORE_IP_POLL_MS: u64 = 3_000`（`background_task.rs:109`）。
- `fn post_restore_wait_should_continue(waited_ms: u64, max_ms: u64) -> bool` — `background_task.rs:115-117` 纯函数，`waited_ms < max_ms`。
- `fn post_restore_adapters_have_ip(app_handle: &AppHandle) -> bool` — `background_task.rs:123-133` 用 `get_adapters_force` 枚举主副适配器是否拿到 IP。
- `pub fn spawn_background_check_after_outbound_restore(app_handle: &AppHandle)` — `background_task.rs:145-179`：阻塞线程池轮询等 IP 就绪（最长上限），结束后调 `spawn_background_check_once`（`background_task.rs:177`）；join 失败按等满上限处理。
- 测试：`post_restore_wait_stops_at_cap`（`background_task.rs:189-194`）、`post_restore_wait_zero_budget_never_waits`（`background_task.rs:189-191`）。

### 巡检主体与 Portal 检测（`monitor/background_check.rs`、`monitor/portal_check.rs`）

- `pub(crate) fn should_reset_logged_online(reachable: bool, any_online: bool, has_logged_online: bool) -> bool` — `background_check.rs:25-27` 纯函数，`reachable && !any_online && has_logged_online`。
- `pub(crate) fn run_background_check_blocking(app_handle: &AppHandle, state: &AppState, cancel_token: &CancellationToken)` — `background_check.rs:34-400`：退出/取消即返（`:30-32`）；`is_checking.try_acquire()` 并发跳过（`:33-35`）；适配器取 `get_adapters_cached` 空则降级 `get_adapters_force`（`:43-52`）；出站切换态 `outbound_restore_active` 跳过本轮（`:64-71`）；校园网静默期 `is_campus_check_silent` 跳过验证但置 `on_campus=true`（`:72-101`）；失败触发 `start_campus_exit`/`cancel_campus_exit`（`:89,157,164`）；双适配器并行 Portal 检测用 `Handle::current().block_on`+`spawn_blocking`+`tokio::join!`（`:177-189`）；Portal 请求失败调 `handle_portal_request_failure` 累加计数（`:213-234`）；成功重置失败计数（`:243-261`）；`should_reset_logged_online` 判定重置 `has_logged_online`（`:334-340`）；调 `handle_status_change`（`:341-346`）与 `emit_background_check_result` 使用 `BackgroundCheckResult` 结构（`:357-378`）；调 `try_auto_login_on_preparation`（`:380`）与 `try_disconnect_reconnect`（`:382-386`）；仅未重连时 `update_network_state`（`:390-392`）。
- `pub async fn run_background_check(app_handle: &AppHandle, cancel_token: Arc<CancellationToken>)` — `background_check.rs:400-408`：`spawn_blocking` 调 blocking 版，异常记日志。
- 测试：`no_reset_while_online`（`:416-420`）、`no_reset_when_unreachable`（`:424-426`）、`no_reset_when_never_logged_in`（`:430-432`）、`reset_when_reachable_offline_and_logged_before`（`:436-438`）。
- `pub(super) enum PortalCheckResult` — `portal_check.rs:6-19`：变体 `Success{online:bool, message:String, reachable:bool, login_available:bool, uid:Option<String>}`（`:7-14`）、`Error{is_request_failed:bool}`（`:15-17`）、`NotFound`（`:18`）。
- `impl PortalCheckResult` — `portal_check.rs:21-57`：方法 `online()`（`:22-27`）、`message()`（`:29-35`）、`reachable()`（`:37-42`）、`login_available()`（`:44-49`）、`uid()`（`:51-56`）。
- `pub(super) fn check_adapter_portal(adapter: &Adapter, app_handle: &AppHandle) -> PortalCheckResult` — `portal_check.rs:59-93`：调 `check_portal_full`，`error_kind=="request_failed"` 返回 `Error{is_request_failed:true}` 并 emit 日志（`:65-72`）；异常返回 `Error{is_request_failed:false}`（`:83-91`）。

### 自动登录与断线重连（`monitor/auto_auth.rs`）

- 常量 `RECONNECT_REMINDER_INTERVAL: u32 = 10`（`auto_auth.rs:14`）、`PREP_LOGIN_MAX_FAILURES: u32 = 5`（`auto_auth.rs:22`）、`PREP_LOGIN_BACKOFF_CAP_SECS: u64 = 300`（`auto_auth.rs:28`）。
- `pub fn reconnect_attempt_allowed(reconnect_count: u32, max_reconnect: u32) -> bool` — `auto_auth.rs:33-36`：`reconnect_count <= max_reconnect || (reconnect_count - max_reconnect) % RECONNECT_REMINDER_INTERVAL == 0`。
- `pub fn prep_login_backoff_secs(failures: u32, base_cooldown_secs: u64) -> u64` — `auto_auth.rs:41-51`：未达上限返 base，达上限指数翻倍封顶。
- `pub fn prep_login_cooldown_ready(failures: u32, elapsed_secs: u64, base_cooldown_secs: u64) -> bool` — `auto_auth.rs:54-56`：`elapsed_secs >= prep_login_backoff_secs(...)`。
- `pub fn is_time_window_refusal(msg: &str) -> bool` — `auto_auth.rs:63-74`：识别“时间段/时段/时间范围/服务时间/时间未到/不在允许”等中文关键词及英文 time range。
- `pub fn prep_failures_after_failure(failures: u32, failure_msg: &str) -> u32` — `auto_auth.rs:79-85`：时间窗拒绝不累加，其余失败累加。
- `pub fn reconnect_should_report(login_success: bool, within_limit: bool) -> bool` — `auto_auth.rs:91-93` 返回 `login_success && within_limit`。
- `pub fn try_auto_login_on_preparation(app_handle, state, login_available: bool, online: bool, config: &Config)` — `auto_auth.rs:95-204`：门控 `!login_available||online||!auto_login_on_preparation`（`:120-121`）；`has_logged_online` 跳过（`:126-128`）；连续失败 `prep_login_failures>=PREP_LOGIN_MAX_FAILURES` 退避（`:132-139`）；注销保护期/冷却期检查（`:141-158`）；`is_logging_in.try_acquire()` 后 `full_login`（`:161-164`）；成功置 `has_logged_online=true` 且失败计数清零（`:180-184`）。
- `pub fn try_disconnect_reconnect(app_handle, state, online: bool, secondary_online: Option<bool>, a1: Option<&Adapter>, adapter1_name: &str, adapter2_name: &str, reachable: bool, login_available: bool, config: &Config) -> bool` — `auto_auth.rs:207-316`：非 `any_adapter_online` 等条件返 false（`:223-225`）；获登录锁后 `disconnect_reconnect_count` 自增（`:248-251`）；超限释放锁并通知（`:294-307`）；返回 `reconnect_should_report`（`:315`）。
- `pub fn run_auto_login_on_start(app_handle: &AppHandle)` — `auto_auth.rs:318-567`：`auto_login_on_start` 门控（`:322-324`）；`--autostart` 决定延迟 5000/1500ms（`:326-327`）；自启时适配器未就绪重试3次（`:364-378`）；校园网静默/检测逻辑（`:385-436`）；双适配器并行 Portal 检测（`:448-464`）；已在线跳过（`:479-504`）；`full_login` 执行（`:519-530`）；成功可 `start_auto_exit`（`:558-561`）。
- 测试：`reconnect_success_within_limit_reports_true`（`:574-577`）、`time_window_refusal_classification`（`:580-592`）、`time_window_refusal_does_not_grow_backoff`（`:595-602`）、`reconnect_failure_never_reports_true`（`:605-608`）、`reconnect_success_over_limit_reports_false`（`:611-614`）、`reconnect_allowed_within_limit`（`:618-621`）、`reconnect_allowed_periodically_after_limit`（`:625-630`）、`prep_login_backoff_below_limit_uses_base_cooldown`（`:634-637`）、`prep_login_backoff_grows_exponentially_after_limit`（`:641-645`）、`prep_login_backoff_capped`（`:650-653`）、`prep_login_ready_after_backoff_elapsed_even_at_limit`（`:658-664`）。

### 延迟与质量调度（`monitor/latency.rs`、`monitor/quality_scheduler.rs`）

- 常量 `BAD_LEVELS: &[&str] = &["poor","bad"]`（`latency.rs:12`）、`GOOD_LEVELS: &[&str] = &["excellent","great","good"]`（`latency.rs:13`）。
- `pub(super) fn classify_quality_change(last: Option<&str>, current: &str, enable_notification: bool) -> Option<&'static str>` — `latency.rs:18-38` 返回 `Some("bad")`/`Some("good")`/`None`。
- `pub(super) fn record_last_quality(state: &AppState, current: &str)` — `latency.rs:40-42`。
- `pub(super) fn notify_quality_change(app_handle: &AppHandle, kind: &str)` — `latency.rs:44-52` bad/good 发不同通知。
- `pub(super) async fn wait_for_adapter_ready(app_handle, cancel_token) -> Option<(String, String)>` — `latency.rs:67-85` 每2s重试等 IP 与 `any_adapter_online`，取消返 None。
- `pub fn spawn_latency_test_loop(app_handle: &AppHandle) -> Result<(), String>` — `latency.rs:78-139`：动态间隔（轻量化下限1800s），首 tick 吞掉，调 `run_quality_check`（`latency.rs:135`）。
- `pub(super) async fn run_startup_quality_once(app_handle: AppHandle, cancel_token: Arc<CancellationToken>)` — `latency.rs:144-160`：启动即测一次，等待就绪与1s稳定后调 `run_quality_check`（`:159`）。
- 测试：`degraded_to_bad_level_triggers_pending_bad`（`:167-170`）、`recovery_from_bad_level_triggers_good`（`:173-176`）、`same_level_or_unknown_last_never_notifies`（`:179-183`）、`middle_levels_crossing_never_notifies`（`:186-192`）、`notification_disabled_never_classifies`（`:195-198`）。
- 常量 `SPIKE_CONFIRM_COUNT: usize = 2`（`quality_scheduler.rs:12`）、`SPIKE_CONFIRM_INTERVAL_SECS: u64 = 15`（`quality_scheduler.rs:13`）。
- `async fn perform_quality_check(app_handle, adapter_name: &str, adapter_ip: &str, lightweight: bool) -> Option<serde_json::Value>` — `quality_scheduler.rs:25-51`：`is_quality_checking.try_acquire()` 互斥（`:25`），调 `check_network_quality_async`（`:29`），落盘历史与 emit 结果。
- `pub(super) async fn run_quality_check(app_handle, adapter_name: &str, adapter_ip: &str, cancel: Option<&CancellationToken>)` — `quality_scheduler.rs:56-121`：首次检测后 `classify_quality_change` 若 `Some("bad")` 触发复核：15s 间隔轻量化再测2次（`:70-103`），全确认才 `notify_quality_change("bad")`（`:104-105`）；其余 kind 直接通知并记录（`:111-116`）。

### 校园网检测与结果发射（`monitor/campus_check.rs`、`monitor/background_emit.rs`）

- `pub struct ConnectionCampusStatus` — `campus_check.rs:5-11`：字段 `on_campus:bool`、`name:Option<String>`、`message:String`（derive Serialize, camelCase）。
- `pub struct CampusCheckResult` — `campus_check.rs:13-21`：字段 `wifi:Option<ConnectionCampusStatus>`、`wired:Option<ConnectionCampusStatus>`、`on_campus:bool`、`current_ssid:Option<String>`、`message:String`。
- `pub(super) fn adapter_campus_status<'a>(...)` — `campus_check.rs:23-28` 按无线/有线取状态。
- `pub(super) fn adapter_campus_message(...)` — `campus_check.rs:30-32`。
- `fn campus_name_list(value: &str) -> Vec<&str>` — `campus_check.rs:36-42` 支持多分隔符解析。
- `pub fn check_campus_network(config: &Config, adapters: &[Adapter]) -> CampusCheckResult` — `campus_check.rs:261-355`：名称检查禁用时走网关/portal 可达（`:48-75`）；启用时 WiFi/有线分别匹配名称、子网、网关、portal（`:121-259`）；汇总 `on_campus` 与 `message`（`:261-299`）。
- `pub fn is_campus_check_silent(now_minutes: u16, start: u16, end: u16) -> bool` — `campus_check.rs:306-314` 静默期判定。
- 测试：`名称名单_解析多分隔符与空项`（`:321-329`）、`静默期判定_开始时间单边`（`:332-338`）、`静默期判定_时间段双边`（`:341-347`）、`静默期判定_结束不晚于开始时退化为单边`（`:350-354`）。
- `pub(super) struct BackgroundCheckResult<'a>` — `background_emit.rs:14-34`：字段 `online:bool`、`reachable:bool`、`login_available:bool`、`message:&'a str`、`adapter1_name:&'a str`、`adapter2_name:&'a str`、`secondary_online:Option<bool>`、`secondary_message:&'a str`、`online_operator:Option<String>`、`secondary_online_operator:Option<String>`、`dual_adapter:bool`、`config:&'a Config`、`campus_result:&'a CampusCheckResult`、`a1_campus_msg:Option<&'a str>`、`a2_campus_msg:Option<&'a str>`、`a1_on_campus:Option<bool>`、`a2_on_campus:Option<bool>`。
- `pub fn adapter_status_entry(...)` — `background_emit.rs:43-48`；`adapter_disabled_entry`（`:43-45`）；`adapter_disconnected_entry`（`:47-49`）。
- `pub(super) fn build_adapter_details(...)` — `background_emit.rs:51-65`。
- `pub(super) fn handle_status_change(...)` — `background_emit.rs:68-107`：状态变更发通知（60s 去抖）。
- `pub(super) fn emit_background_check_result(...)` — `background_emit.rs:109-164`：递增计数、注销保护期强制 offline、emit 前端事件。
- `pub(super) fn update_network_state(...)` — `background_emit.rs:166-202`：更新 `any_adapter_online` 等，注销保护期跳过。

### 定时动作与夜间出站编排（`monitor/scheduled.rs`、`monitor/outbound_switch.rs`、`monitor/outbound_night.rs`）

- 常量 `SCHEDULED_TICK_MS: u64 = 30_000`（`scheduled.rs:42`）、`NIGHT_VERIFY_WAIT_SECS: u64 = 15`（`scheduled.rs:46`）、`NIGHT_VERIFY_RETRY_SECS: u64 = 5`（`scheduled.rs:47`）、`NIGHT_VERIFY_ATTEMPTS: u32 = 3`（`scheduled.rs:48`）、`NIGHT_VERIFY_HTTP_TIMEOUT_SECS: u64 = 15`（`scheduled.rs:49`）、`SCHEDULED_LOGIN_MAX_RETRIES: u32 = 3`（`scheduled.rs:333`）。
- `pub async fn run_scheduled_action_loop` — `scheduled.rs:164-298`：启动前 `reconcile_outbound_on_startup`（来自 super::outbound_night，`:59`）与 `reconcile_scheduled_logout_on_startup`（`:61`）；循环内 tokio::select 等 tick/cancel（`:63-70`）；`is_quitting` 判断退出（`:75-77`）；算 now_minutes/today_day/config_snapshot；用拍前 `outbound_switch_active`（`:88`）门控 `evaluate_scheduled`（`:89-92`）得 (fire_login,fire_logout) 调 `run_scheduled_login`/`run_scheduled_logout`；再算 restore_active 调 `evaluate_outbound_action`（`:107-119`）匹配 `NightOutboundAction` 三分支经 `run_outbound_blocking` 执行 `apply_outbound_switch`/`apply_outbound_restore` 或 replay/watchdog（`:120-161`）；拍后取 `outbound_active`（`:164`）门控 `evaluate_night_switch_gated`（`:168-181`）得 night_action 调 `apply_night_switch_action`（`:182-184`）。
- `fn is_quitting` — `scheduled.rs:190-192` 抽离避免跨 await 持 State 守卫。
- `fn reconcile_scheduled_logout_on_startup` — `scheduled.rs:202-217` 冷启动若 now_minutes>=scheduled_logout_minutes 置 logout_day 消耗额度不执行。
- `fn evaluate_scheduled` — `scheduled.rs:226-237` 装配 AppState 调共享纯函数 `evaluate_scheduled_actions`。
- `fn mark_scheduled_day_fired` — `scheduled.rs:243-251`（`#[cfg_attr(not(test), allow(dead_code))]`）置 login_day/logout_day。
- `fn format_minutes` — `scheduled.rs:253-255` 补零格式化 mm:ss。
- `fn run_scheduled_login` — `scheduled.rs:260-267` 调 `spawn_login_action` 传 Some(today_day)。
- `fn spawn_login_action` — `scheduled.rs:273-329`：抢 `tasks.is_logging_in` 锁（`:279`），抢到后若 login_quota_day 有值置额度（`:294-296`），调 `auth::service::full_login`（`:297`）与 `post_login_handler`（`:307`），emit 日志；失败补试逻辑（`:308-327`）。
- `fn apply_night_switch_action` — `scheduled.rs:353-380`：改内存 config（SwitchToCampus 将原 operator 存 night_operator_restore 并置空串=无锡学院，Restore 反之）（`:343-352`），调 `save_config_and_broadcast` 落盘（`:353`）失败则不 store 返回；成功 store 后调 `spawn_login_action`（None 额度）并 spawn `verify_night_switch`（`:364`）。
- `fn run_scheduled_logout` — `scheduled.rs:372-376` spawn_blocking 调 `perform_logout`。
- `fn perform_logout` — `scheduled.rs:383-417`：抢 `tasks.is_logging_out` 锁（`:386`），成功后置 logout_day 额度（`:395-397`），调 `full_logout`（`:398`）；成功则取消自动退出、重置 failure_tracker、启动 60s 注销保护期（`:405-416`）。
- `async fn verify_night_switch` — `scheduled.rs:452-489`：等 15s，`night_verify_config` 检查 operator 仍匹配否则跳过（`:425,437`）；`night_switch_verify_round` 重试 3 次间隔 5s（`:428,444`）；不符则注销重登再验（`:435-442`），最终不符发告警日志+`emit_notification`（`:449-458`）。
- `fn night_verify_config` — `scheduled.rs:464-468` operator != expected_operator 返 None 跳过验证。
- `async fn night_switch_verify_round` — `scheduled.rs:474-492` 用 `create_safe_http_client` 做 NIGHT_VERIFY_ATTEMPTS 轮。
- `async fn night_switch_check_once` — `scheduled.rs:493-517` 构造 chkstatus URL（经 `portal_origin`），parse_chkstatus+uid_matches 核对。
- `pub(crate) fn portal_origin` — `scheduled.rs:521-531` 取 portal_url 的 origin，空值回退 default_portal_url。
- `fn expected_operator_display` — `scheduled.rs:534-540` 空串显“无锡学院”。
- 测试模块（`scheduled.rs:542-658`）：`禁用目标_不触发_不置标记`（`:549-557`）、`到点触发_置当日标记`（`:560-579`）、`跨天重置_次日再触发`（`:582-589`）、`切换态_跳过定时登录且不消耗当日标记_注销照常`（`:592-618`）、`format_minutes_补零`（`:621-624`）、`定时登录补试计数_累加清零跨天归零`（`:628-638`）、`定时登录失败回退额度_补试用尽后当日停止`（`:643-658`）。

- `pub fn is_campus_adapter(ip, gateway, campus_gateway, portal_host, gateway_probe: Fn(&str,&str)->bool) -> bool` — `outbound_switch.rs:17-39`：IP 空否（`:24`）；同 /18 真；否则绑源探校园网关或 portal 两发去抖任一可达即真（`:35-38`）。
- `pub fn portal_probe_host(portal_url) -> String` — `outbound_switch.rs:44-56` 从 portal_url 提取私网 IPv4 主机，域名/公网/空返空串。
- `pub fn effective_outbound_priority(priority: &[String], details: &[(Adapter,String)]) -> Vec<String>` — `outbound_switch.rs:63-82` 非空透传，空则无线优先后有线保发现序。
- `pub fn select_outbound_candidate` — `outbound_switch.rs:87-111` 按优先级选首张非校园、有 IP、有网关且自身网关可达卡 -> Option<Adapter>。
- `struct SnapshotRow` — `outbound_switch.rs:115-121` 含字段 `guid:String`/`family:u16`/`automatic:bool`/`metric:u32`，序列化时补 guid。
- `pub fn snapshot_json` — `outbound_switch.rs:125-136` 序列化选中卡跃点快照，兜底 `"[]"`。
- `pub struct DisabledRow` — `outbound_switch.rs:142-145` 含字段 `guid:String`/`name:String`，记录本功能亲手禁用卡。
- `pub fn disabled_adapters_json` — `outbound_switch.rs:148-150` 序列化禁用名单，兜底 `"[]"`。
- `pub fn parse_disabled_adapters` — `outbound_switch.rs:156-161` 空串->空名单 Ok，非法 JSON->Err。
- `pub struct StandbyRoute` — `outbound_switch.rs:168-176` 含字段 `dest:String`/`mask:String`/`gateway:String`/`metric:u32`/`if_index:u32`，历史兜底默认路由快照。
- `pub fn parse_standby_route` — `outbound_switch.rs:179-181` 空/非法->None。
- `pub enum StandbyCleanup` — `outbound_switch.rs:186-196` 变体 `AlreadyGone`/`ForeignRowKept`/`LastDefaultPath`/`Safe`。
- `pub fn standby_cleanup_decision` — `outbound_switch.rs:203-220` 纯函数，按 same_key_metrics/standby_metric/rows_outside_exact 判。
- `pub fn standby_route_cleanup` — `outbound_switch.rs:224-264 (windows) / :268-270 (not windows)`：非 windows 返 AlreadyGone（`:268-270`）。
- `pub(crate) fn unsafe_to_disable` (cfg windows) — `outbound_switch.rs:278-280` 实例 ID 读不到返 true 保守跳过。
- `pub fn select_campus_to_disable(priority, details, campus_gateway, portal_host, exclude_guid, gateway_probe, bus_guard) -> Vec<DisabledRow>` — `outbound_switch.rs:285-318` 过滤目标卡/无 IP/非校园/guard 命中，同名去重。
- 测试模块（`outbound_switch.rs:346-691`）：`campus_adapter_by_subnet`(`:345-349`)、`campus_adapter_off_subnet_probe_reachable`(`:351-355`)、`campus_adapter_off_subnet_probe_unreachable`(`:357-361`)、`campus_adapter_empty_ip_is_false`(`:363-366`)、`campus_adapter_empty_gateway_skips_probe`(`:368-372`)、`candidate_skips_campus_and_ipless_and_out_of_list`(`:374-389`)、`candidate_probe_reachable_means_campus_skipped`(`:391-396`)、`candidate_priority_name_absent_from_details_skips`(`:398-406`)、`candidate_empty_priority_returns_none`(`:408-414`)、`effective_priority_non_empty_passthrough`(`:416-425`)、`effective_priority_empty_defaults_wireless_first`(`:427-439`)、`effective_priority_empty_multiple_wireless_keep_discovery_order`(`:441-453`)、`effective_priority_empty_no_adapters_is_empty`(`:455-459`)、`candidate_skips_gatewayless_and_dead_gateway_cards`(`:461-478`)、`campus_probe_single_loss_still_counts_as_campus`(`:480-489`)、`snapshot_json_round_trip_with_guid`(`:491-504`)、`snapshot_json_empty_rows`(`:506-510`)、`disabled_adapters_json_round_trip`(`:512-524`)、`standby_route_snapshot_parses_legacy_config_json`(`:526-544`)、`select_campus_to_disable_filters_everything_non_campus`(`:546-574`)、`select_campus_to_disable_probe_reachable_counts_and_dedups`(`:576-592`)、`select_campus_to_disable_unknown_bus_is_skipped`(`:594-608`)、`campus_adapter_portal_probe_reachable`(`:610-622`)、`campus_adapter_portal_unreachable_stays_non_campus`(`:624-635`)、`campus_adapter_portal_probe_single_loss_still_counts`(`:637-654`)、`campus_adapter_empty_gateway_portal_reachable`(`:656-660`)、`portal_probe_host_extracts_private_ipv4`(`:662-671`)、`standby_cleanup_never_deletes_foreign_or_last_default_route`(`:673-691`)。

- `outbound_night.rs` 常量与辅助：`OUTBOUND_HELPER_TIMEOUT_SECS: u64 = 30`(`:20`)、`OUTBOUND_BACKOFF_START_MS: u64 = 60_000`(`:24`)、`OUTBOUND_BACKOFF_MAX_MS: u64 = 300_000`(`:25`)、`OUTBOUND_RESTORE_ALERT_FAILS: u32 = 3`(`:30`)、`OUTBOUND_RESTORE_ALERT_REMIND_EVERY: u32 = 20`(`:32`)、`OUTBOUND_RESTORE_GIVE_UP_FAILS: u32 = 40`(`:36`)。
- `fn outbound_restore_alert_due(count: u32) -> bool` — `:39-42`；`fn notify_outbound_issue_once_per_day` — `:46-52`；静态原子变量退避记账 — `:55-70`；`static OUTBOUND_ACTION_BUSY` — `:76`；`struct OutboundActionGuard` RAII 锁 — `:79-94`；`pub(super) async fn run_outbound_blocking` — `:98-110`；`pub(super) fn outbound_switch_active` — `:114-121`；`fn in_outbound_restore_window` — `:125-127`；`fn outbound_backoff_ms` — `:137-144`；`fn backoff_gate` — `:148-153`；`fn switch_gate_open`/`restore_gate_open` — `:155-169`；`fn mark_outbound_failure`/`clear_outbound_failure` — `:172-181`；`fn outbound_allow_uac` — `:184-186`；`fn note_channel_failure`/`clear_outbound_channel_failure` — `:189-198`；`pub(super) fn needs_replay` — `:203-205`；`struct OutboundSnapshotRow` — `:209-215`；`fn parse_outbound_snapshot` — `:218-223`；`struct HelperFailure` — `:226-233`；`fn run_helper_op` — `:239-265`；`fn enable_campus_adapter` (cfg desktop `:276-287` / not desktop `:291-296`)；`fn write_outbound_metric` — `:304-365`；`pub(super) fn apply_outbound_switch` — `:391-583`；`pub(super) fn replay_outbound_switch` — `:600-688`；`pub(super) fn watchdog_re_disable_campus` — `:697-762`；`pub(super) fn apply_outbound_restore` — `:776-1089`；`fn finish_outbound_restore` — `:1096-1112`；`fn clear_outbound_snapshot` — `:1116-1134`；`fn set_outbound_manual_hold`/`clear_outbound_manual_hold` — `:1140-1156`；`pub(crate) fn manual_outbound_switch` — `:1162-1180`；`pub(crate) fn manual_outbound_restore` — `:1184-1197`；`pub(super) fn reconcile_outbound_on_startup` — `:1204-1262`。
- 测试（`outbound_night.rs:1264-1373`）：`还原失败告警_首达阈值触发_此后周期重复`(`:1268-1281`)、`重放闸_仅切换态且留有失败历史时才补齐`(`:1283-1294`)、`退避窗_起步六十秒_每失败翻倍封顶三百秒`(`:1296-1304`)、`退避闸_无失败历史恒放行`(`:1306-1312`)、`退避闸_首败六十秒内压制_六十秒后放行`(`:1314-1319`)、`退避闸_第二败退避翻倍`(`:1321-1326`)、`退避闸_封顶后三百秒内压制_三百秒后放行`(`:1328-1335`)、`退避闸_时钟回拨按压制处理`(`:1337-1340`)、`快照解析_空串为空表_非法json报错`(`:1342-1353`)、`单飞闸_同一时刻只放一个出站动作_释放后可再取`(`:1357-1370`)。

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
│ portal_check.rs:59│ │ auto_auth.rs:95  │ │ scheduled.rs:52  │
│ check_adapter_    │ │ try_auto_login_  │ │ run_scheduled_   │
│ portal           │ │ on_preparation   │ │ action_loop      │
└──────────────────┘ └──────────────────┘ └────────┬─────────┘
                                                   │ uses
                                                   ▼
                                         ┌──────────────────────┐
                                         │ outbound_night.rs:391│
                                         │ apply_outbound_*     │
                                         │ + outbound_switch.rs│
                                         │ is_campus_adapter   │
                                         └──────────────────────┘
```

## Data Flow

### 启动挂载
- `watcher.rs:16` `run_startup_tasks` 依据配置 spawn：`startup_bg_check`（`watcher.rs:16` → `background_task.rs:7`）、`startup_latency`（`watcher.rs:16` → `latency.rs:144`）、`startup_quality_once`（`watcher.rs:16` → `latency.rs:144`）、`startup_auto_login`（`watcher.rs:16` → `auto_auth.rs:318`）、`scheduled_actions`（`watcher.rs:16` → `scheduled.rs:52`）。

### 单轮巡检主链
- `background_task.rs:29-53` 循环调用 `background_check::run_background_check`（`background_check.rs:400` → `:29`）。
- `background_check.rs:34` `is_checking.try_acquire()` 互斥；`:43-52` 适配器 cached/force；`:64-71` `outbound_restore_active` 跳过；`:72-101` 静默期跳验证置 `on_campus=true`；`:177-189` 双适配器并行 `portal_check::check_adapter_portal`（`:59`）；`:213-234` 失败计数；`:380` `auto_auth::try_auto_login_on_preparation`；`:382-386` `auto_auth::try_disconnect_reconnect`；`:390-392` 未重连时 `update_network_state`（经 `background_emit.rs:166`）。
- `should_reset_logged_online`（`background_check.rs:25`, `:334-340`）修复休眠会话失效。

### 质量链
- `latency.rs:78` `spawn_latency_test_loop` 就绪等待后调 `quality_scheduler.rs:47` `run_quality_check`；`:70-103` bad 复核 2×15s；`perform_quality_check`（`:19-45`）调 `check_network_quality_async` 落盘 emit。

### 定时动作循环
- `scheduled.rs:52` 30s 循环：`:89-92` 定时登录/注销；`:107-161` 出站切换编排调 `super::outbound_night` 动作（底层 `outbound_switch.rs` 选择）；`:164-184` 运营商夜切 `apply_night_switch_action` 改配置并 `verify_night_switch`（`:423`）。

### 夜间出站还原补检
- `background_task.rs:147` `spawn_background_check_after_outbound_restore` 阻塞轮询等 IP 就绪（最长45s）后 `spawn_background_check_once`（`:177`）；该钩子由 `outbound_night.rs:1110` `finish_outbound_restore` 在还原收尾时统一调用。

## Connections

- [[desktop-auth]]：`auto_auth.rs:164`（内部 `full_login` 调用）、`scheduled.rs:297` 同；`portal_check.rs:59` 调 `auth::portal::check_portal_full`；`background_check.rs:213` 调 `handle_portal_request_failure`（阈值见 failure_tracker）。
- [[desktop-network-quality]]：`quality_scheduler.rs:29` 调 `check_network_quality_async` 并发 `emit_network_quality_result`。
- [[desktop-config]]：依赖 `config::persist::save_config_and_broadcast`（`background_task.rs:59`、`scheduled.rs:353`、`outbound_night.rs:504`）、共享纯函数 `evaluate_scheduled_actions`/`evaluate_outbound_action`/`evaluate_night_switch_gated`（`scheduled.rs:226` 等）。
- [[desktop-infra]]：`infra::events::EventBus` 与 `infra::notification::emit_notification`（`scheduled.rs` 使用）、`CancellationToken` 跨任务传递。
- [[night-outbound-switch]]：`scheduled.rs` 与 `outbound_switch.rs`/`outbound_night.rs` 为桌面动作层，`outbound_night` 提供 `apply_outbound_switch`/`apply_outbound_restore` 等（`scheduled.rs:34-38` 引用）。
- [[desktop-network-core]]：`background_check.rs:43` 调 `get_adapters_cached`/`get_adapters_force`，`outbound_switch.rs:11` 用 `crate::network::discovery::Adapter`。

## Known Issues

基于观察，无未解决缺陷报告；以下为已处理的边界条件与非显然行为（均已在代码中实现对应逻辑）：

1. **并发互斥**：`background_check.rs:34` `is_checking.try_acquire()` 安全跳过叠加轮次；`quality_scheduler.rs:25` `is_quality_checking`；`auto_auth.rs:161,239` `is_logging_in` 锁。
2. **适配器获取降级**：cached 空降级 force（`background_check.rs:43-52`）；夜间补检特意用 force 避旧缓存（`background_task.rs:123`）。
3. **出站切换态跳过**：`outbound_restore_active` 真时整轮巡检跳过（`background_check.rs:65-72`）。
4. **校园网静默**：静默期跳过验证但强制 `on_campus=true`（`background_check.rs:72-101`）；`auto_auth` 开机自启同理（`auto_auth.rs:389-393` 静默置 true）。
5. **登录标记复位（关键修复）**：`should_reset_logged_online`（`background_check.rs:25,334-340`）复位 `has_logged_online`，解决整夜休眠后自动登录被永久门控。
6. **自动登录防无限重试**：`PREP_LOGIN_MAX_FAILURES=5`（`auto_auth.rs:207`）；`try_disconnect_reconnect` 超限停止（`auto_auth.rs:207` 门控、`auto_auth.rs:207` 通知）。
7. **重连状态覆盖修复**：`try_disconnect_reconnect` 返回真实 `reconnect_should_report`（`auto_auth.rs:315`），调用方依此跳过旧 Portal 快照覆盖（`background_check.rs:390`）。
8. **Portal 请求失败容错**：`Error{is_request_failed:true}` 累计阈值（`background_check.rs:209-234`），成功重置（`background_check.rs:243-261`）。
9. **夜间出站还原补检缺口**：新增 `spawn_background_check_after_outbound_restore`（`background_task.rs:147-181`）最长等45s轮询IP。
10. **延迟测试动态间隔**：轻量化下限1800s，吞首 tick（`latency.rs:78-139`）。
11. **质量抖动复核**：poor/bad 需15s×2次轻量化复核全达标才通知（`quality_scheduler.rs:70-105`）；中间档互转不通知（`latency.rs:18-38` 函数逻辑，测试 `:186-192`）。
12. **就绪等待**：`wait_for_adapter_ready` 每2s等IP与在线（`latency.rs:58-76`）。
13. **任务注册失败处理**：watcher 各 spawn 失败仅 `log_warn`（`watcher.rs:29,41,50,58,67`）；`spawn_background_check_once` 复用令牌补跑（`background_task.rs:82-93`）。
14. **双适配器并行实现**：`run_background_check_blocking` 在 spawn_blocking 内用 `Handle::current().block_on`+`join!`（`background_check.rs:177-189`）。
15. **出站与运营商夜切互斥**：`scheduled.rs:88` 拍前出站门控登录，`scheduled.rs:164` 拍后切换态门控夜切。
16. **历史路由清理**：`outbound_switch.rs:203-220` 不再创建 StandbyRoute，仅解析历史并安全决策（2026-10-18 事故后）。
