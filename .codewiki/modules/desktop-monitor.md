---
title: 后台巡检：连通性检测、自动登录/断线重连与网络质量调度
type: module
source_files:
  - tauri-app/src-tauri/src/monitor/mod.rs
  - tauri-app/src-tauri/src/monitor/dynamic_interval.rs
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

本模块是桌面端后台巡检中枢。`monitor/mod.rs:1-14` 声明子模块：`watcher`(行1)、`auto_auth`(行2)、`latency`(行3)、`adapter_watch`(行4)、`campus_check`(行5)、`outbound_switch`(行6)、`portal_check`(行7)、`quality_scheduler`(行8)、`background_emit`(行9)、`background_check`(行10)、`background_task`(行11)、`outbound_night`(行12)、`scheduled`(行13)、`dynamic_interval`(行14)。启动期由 `watcher::run_startup_tasks`（`watcher.rs:4-60`）统一编排后台巡检、质量测试、自动登录与定时动作循环。核心巡检逻辑在 `background_check` 中独占连通性/Portal/重连，质量检测由 `latency`+`quality_scheduler` 独立循环负责，定时登录/注销与夜间出站切换编排在 `scheduled` 中 30s 循环，底层动作层 `outbound_switch` 与 `outbound_night` 提供候选卡选择、快照序列化与提权执行。模块整体在桌面端编译，非 Windows 平台对部分提权/路由能力做 stub 或空实现（如 `outbound_switch.rs:139-142` 的 `standby_route_cleanup` 非 Windows 返回 `AlreadyGone`）。

## Key Components

### 模块索引与启动编排（`monitor/mod.rs`、`monitor/watcher.rs`）

- `monitor/mod.rs:1-14` 声明子模块（含 `dynamic_interval` 于行14）。
- `pub use super::background_task::start_background_check_inner;` — `monitor/watcher.rs:10`。
- `pub use super::campus_check::check_campus_network;` — `monitor/watcher.rs:13`。
- `pub use super::background_emit::{adapter_status_entry, adapter_disabled_entry, adapter_disconnected_entry};` — `monitor/watcher.rs:14`。
- `pub fn run_startup_tasks(app_handle: &AppHandle)` — `monitor/watcher.rs:16-72`，启动期统一编排入口：
  - `config.enable_background_check` 真时 spawn `startup_bg_check` 调 `start_background_check_inner`（`watcher.rs:21-31`）。
  - `config.enable_network_quality` 时 spawn `startup_quality_once` 调 `run_startup_quality_once`（`watcher.rs:36-43`）。
  - `enable_network_quality && enable_latency_test` 时 spawn `startup_latency` 调 `spawn_latency_test_loop`（`watcher.rs:45-55`）。
  - 无条件 spawn `startup_auto_login` 调 `run_auto_login_on_start`（`watcher.rs:57-62`）。
  - 无条件 spawn `scheduled_actions` 调 `scheduled::run_scheduled_action_loop`（`watcher.rs:66-71`）。
  - 各任务注册失败仅 `log_warn!` 不阻断（`watcher.rs:29,41,50,61,70`）。

### 动态检测间隔状态机（`monitor/dynamic_interval.rs`）

- 常量 `BACKOFF_CAP_MS: u64 = 300_000`（`dynamic_interval.rs:14`）、`ONLINE_INTERVAL_MS: u64 = 600_000`（`dynamic_interval.rs:16`）、`FAST_INTERVAL_MS: u64 = 60_000`（`dynamic_interval.rs:18`）、`PORTAL_RESET_START_MINUTES: u16 = 8 * 60 + 30`（`dynamic_interval.rs:20`）。
- `pub fn portal_reset_window_open(now_min: u16) -> bool` — `dynamic_interval.rs:23-25`，判定 `now_min >= PORTAL_RESET_START_MINUTES`（即每日 08:30 之后启用 MAC 重置检测）。
- `pub enum CheckOutcome` — `dynamic_interval.rs:29-39`：变体 `NotOnline`（未在线，行31）、`Online`（在线且门户可达，行33）、`OnlineButPortalUnreachable`（此前在线但本轮门户不可达，行35）、`Skipped`（未执行有效检测，行38）。
- `pub struct DynamicInterval` — `dynamic_interval.rs:43-47`：字段 `offline_streak: u32`（连续未在线轮数，行44）、`last_ms: Option<u64>`（上一拍计算出的间隔，行46）。
- `impl DynamicInterval::next_ms` — `dynamic_interval.rs:52-73`：根据本轮结局与配置基础间隔给出下一拍间隔；`OnlineButPortalUnreachable` 复位并返回 `FAST_INTERVAL_MS`（`:54-57`）；`Online` 复位并返回 `ONLINE_INTERVAL_MS.max(base_ms)`（`:58-62`）；`NotOnline` 累计 `offline_streak` 走 `backoff_ms`（`:63-66`）；`Skipped` 沿用 `last_ms`（`:67`）。
- `pub fn backoff_ms(base_ms: u64, streak: u32) -> u64` — `dynamic_interval.rs:78-88`：未登录退避计算（基础间隔 ×2^(streak-1)，封顶 `BACKOFF_CAP_MS`，尊重用户值）。
- 测试模块（`dynamic_interval.rs:143-163`）：`未登录按2的幂退避封顶5分钟`（`:95-102`）、`用户基础间隔大于封顶时不缩短`（`:105-107`）、`已登录间隔10分钟且复位退避`（`:110-119`）、`已登录但门户不可达降为1分钟`（`:122-131`）、`mac重置检测窗口8点30分整点开启`（`:134-139`）；`skipped_tests` 模块（`dynamic_interval.rs:143-163`）：`skipped沿用上一拍且不累计退避`（`:147-155`）、`已登录档尊重更大的用户配置`（`:158-162`）。

### 巡检任务生命周期（`monitor/background_task.rs`）

- `pub fn start_background_check_inner(app_handle: &AppHandle, state: &AppState) -> Result<CommandResult, String>` — `background_task.rs:7-68`：置 `enable_background_check=true`，间隔 `<10000` 提至 `15000`（`background_task.rs:10-13`）；spawn `background_check` 任务首启直检，循环动态读 `effective_background_interval_ms`（轻量化下限300s）与 `DynamicInterval::next_ms` 作间隔（`background_task.rs:16-59`，其中 `:27` 创建 `DynamicInterval`、`:31-42` 读取间隔、`:56` 迭代）；落盘 `save_config_and_broadcast`（`background_task.rs:63`）。
- `pub fn spawn_background_check_once(app_handle: &AppHandle)` — `background_task.rs:78-98`：spawn `background_check_once` 调 `run_background_check`（`background_task.rs:80-85`），同名已跑复用 `cancel_token` 补跑（`background_task.rs:86-97`）。
- 常量 `POST_RESTORE_IP_WAIT_MAX_MS: u64 = 45_000`（`background_task.rs:112`）、`POST_RESTORE_IP_POLL_MS: u64 = 3_000`（`background_task.rs:113`）。
- `fn post_restore_wait_should_continue(waited_ms: u64, max_ms: u64) -> bool` — `background_task.rs:119-121` 纯函数，`waited_ms < max_ms`。
- `fn post_restore_adapters_have_ip(app_handle: &AppHandle) -> bool` — `background_task.rs:127-137` 用 `get_adapters_force` 枚举主副适配器是否拿到 IP。
- `pub fn spawn_background_check_after_outbound_restore(app_handle: &AppHandle)` — `background_task.rs:149-183`：阻塞线程池轮询等 IP 就绪（最长45s），结束后调 `spawn_background_check_once`（`background_task.rs:181`）；join 失败按等满上限处理（`:166`）。
- 测试：`post_restore_wait_stops_at_cap`（`background_task.rs:191-196`）、`post_restore_wait_zero_budget_never_waits`（`background_task.rs:200-202`）。

### 巡检主体与 Portal 检测（`monitor/background_check.rs`、`monitor/portal_check.rs`）

- `pub(crate) fn should_reset_logged_online(reachable: bool, any_online: bool, has_logged_online: bool) -> bool` — `background_check.rs:25-27` 纯函数，`reachable && !any_online && has_logged_online`。
- `static RESTORE_SILENT_PASS`（`background_check.rs:42-43`）；`pub(crate) fn request_check_after_outbound_restore()` — `background_check.rs:46-48` 置静默期持续放行标记。
- `pub(crate) fn run_background_check_blocking(app_handle: &AppHandle, state: &AppState, cancel_token: &CancellationToken) -> CheckOutcome` — `background_check.rs:54-471`：退出/取消即返 `Skipped`（`:51-53`）；`is_checking.try_acquire()` 并发跳过（`:54-56`）；适配器取 `get_adapters_cached` 空则降级 `get_adapters_force`（`:63-73`）；出站切换态 `outbound_restore_active` 跳过本轮（`:84-91`）；校园网静默期 `is_campus_check_silent` 整体跳过（`:119-127`），还原后补检 `forced` 放行（`:128-133`）；校园网检测走 `check_campus_network`（`:136`）后更新 `on_campus_network`（`:140-152`）；校园网失败触发 `start_campus_exit`/`cancel_campus_exit`（`:120,199`）；双适配器并行 Portal 检测用 `Handle::current().block_on`+`spawn_blocking`+`tokio::join!`（`:212-224`）；Portal 请求失败调 `handle_portal_request_failure` 累加计数（`:248-290`，窗口外清零）；成功重置失败计数（`:299-317`）；`should_reset_logged_online` 判定重置 `has_logged_online`（`:390-396`）；调 `handle_status_change`（`:397-402`）与 `emit_background_check_result` 使用 `BackgroundCheckResult` 结构（`:413-434`）；调 `try_auto_login_on_preparation`（`:436`）与 `try_disconnect_reconnect`（`:438-442`）；仅未重连时 `update_network_state`（`:446-448`）；`OnlineButPortalUnreachable` 结局联动 `trigger_fast_quality_check`（`:461-463`）。
- `fn trigger_fast_quality_check(...)` — `background_check.rs:472-541`：已登录但门户不可达时降为1分钟快排查，并联动一次网络质量检测（5分钟节流 `:482`），`quality_scheduler::run_quality_check` 执行（`:504`），结论写日志/通知（`:516-539`）。
- `pub async fn run_background_check(app_handle: &AppHandle, cancel_token: Arc<CancellationToken>) -> CheckOutcome` — `background_check.rs:546-555`：`spawn_blocking` 调 blocking 版，异常记日志返 `Skipped`。
- 测试：`no_reset_while_online`（`:563-567`）、`no_reset_when_unreachable`（`:571-573`）、`no_reset_when_never_logged_in`（`:577-579`）、`reset_when_reachable_offline_and_logged_before`（`:583-585`）。
- `pub(super) enum PortalCheckResult` — `portal_check.rs:6-19`：变体 `Success{online:bool, message:String, reachable:bool, login_available:bool, uid:Option<String>}`（`:7-14`）、`Error{is_request_failed:bool}`（`:15-17`）、`NotFound`（`:18`）。
- `impl PortalCheckResult` — `portal_check.rs:21-57`：方法 `online()`（`:22-27`）、`message()`（`:29-35`）、`reachable()`（`:37-42`）、`login_available()`（`:44-49`）、`uid()`（`:51-56`）。
- `pub(super) fn check_adapter_portal(adapter: &Adapter, app_handle: &AppHandle) -> PortalCheckResult` — `portal_check.rs:59-93`：调 `check_portal_full`，`error_kind=="request_failed"` 返回 `Error{is_request_failed:true}` 并 emit 日志（`:65-72`）；异常返回 `Error{is_request_failed:false}`（`:83-91`）。

### 自动登录与断线重连（`monitor/auto_auth.rs`）

- 常量 `RECONNECT_REMINDER_INTERVAL: u32 = 10`（`auto_auth.rs:13`）、`PREP_LOGIN_MAX_FAILURES: u32 = 5`（`auto_auth.rs:21`）、`PREP_LOGIN_BACKOFF_CAP_SECS: u64 = 300`（`auto_auth.rs:27`）。
- `pub fn reconnect_attempt_allowed(reconnect_count: u32, max_reconnect: u32) -> bool` — `auto_auth.rs:32-35`：`reconnect_count <= max_reconnect || (reconnect_count - max_reconnect) % RECONNECT_REMINDER_INTERVAL == 0`。
- `pub fn prep_login_backoff_secs(failures: u32, base_cooldown_secs: u64) -> u64` — `auto_auth.rs:40-50`：未达上限返 base，达上限指数翻倍封顶 `PREP_LOGIN_BACKOFF_CAP_SECS`。
- `pub fn prep_login_cooldown_ready(failures: u32, elapsed_secs: u64, base_cooldown_secs: u64) -> bool` — `auto_auth.rs:53-55`：`elapsed_secs >= prep_login_backoff_secs(...)`。
- `pub fn is_time_window_refusal(msg: &str) -> bool` — `auto_auth.rs:62-73`：识别"时间段/时段/时间范围/服务时间/时间未到/不在允许"等中文关键词（`:67`）及英文 time range（`:72`）。
- `pub fn prep_failures_after_failure(failures: u32, failure_msg: &str) -> u32` — `auto_auth.rs:78-84`：时间窗拒绝不累加，其余失败累加。
- `pub fn reconnect_should_report(login_success: bool, within_limit: bool) -> bool` — `auto_auth.rs:90-92` 返回 `login_success && within_limit`。
- `pub fn try_auto_login_on_preparation(app_handle, state, login_available: bool, online: bool, config: &Config)` — `auto_auth.rs:94-203`：先同步 `login_available` 沿检测与上升沿复位失败计数（`:104-117`）；门控 `!login_available||online||!auto_login_on_preparation`（`:119-121`）；`has_logged_online` 跳过（`:125-127`）；连续失败 `prep_login_failures>=PREP_LOGIN_MAX_FAILURES` 退避（`:131-138`）；注销保护期/冷却期检查（`:140-157`）；`is_logging_in.try_acquire()` 后 `full_login`（`:160-163`）；成功置 `has_logged_online=true` 且失败计数清零（`:179-183`）；失败累加或按时间窗拒绝不计（`:187-198`）。
- `pub fn try_disconnect_reconnect(app_handle, state, online: bool, secondary_online: Option<bool>, a1: Option<&Adapter>, adapter1_name: &str, adapter2_name: &str, reachable: bool, login_available: bool, config: &Config) -> bool` — `auto_auth.rs:206-315`：非 `any_adapter_online` 等条件返 false（`:221-224`）；获登录锁后 `disconnect_reconnect_count` 自增（`:247-250`）；超限释放锁并通知（`:293-307`）；返回 `reconnect_should_report`（`:314`）。
- `pub fn run_auto_login_on_start(app_handle: &AppHandle)` — `auto_auth.rs:317-574`：`auto_login_on_start` 门控（`:321-323`）；`--autostart` 决定延迟 5000/1500ms（`:325-326`）；自启时适配器未就绪重试3次（`:363-378`）；校园网静默/检测逻辑（`:384-443`）；双适配器并行 Portal 检测（`:458-471`）；已在线跳过（`:486-511`）；`full_login` 执行（`:526-537`）；成功可 `start_auto_exit`（`:565-568`）。
- 测试模块（`auto_auth.rs:581-672`）：`reconnect_success_within_limit_reports_true`（`:581-584`）、`time_window_refusal_classification`（`:587-599`）、`time_window_refusal_does_not_grow_backoff`（`:602-609`）、`reconnect_failure_never_reports_true`（`:612-615`）、`reconnect_success_over_limit_reports_false`（`:618-621`）、`reconnect_allowed_within_limit`（`:625-628`）、`reconnect_allowed_periodically_after_limit`（`:632-637`）、`prep_login_backoff_below_limit_uses_base_cooldown`（`:641-644`）、`prep_login_backoff_grows_exponentially_after_limit`（`:648-652`）、`prep_login_backoff_capped`（`:657-660`）、`prep_login_ready_after_backoff_elapsed_even_at_limit`（`:665-671`）。

### 延迟与质量调度（`monitor/latency.rs`、`monitor/quality_scheduler.rs`）

- 常量 `BAD_LEVELS: &[&str] = &["poor","bad"]`（`latency.rs:12`）、`GOOD_LEVELS: &[&str] = &["excellent","great","good"]`（`latency.rs:13`）。
- `pub(super) fn classify_quality_change(last: Option<&str>, current: &str, enable_notification: bool) -> Option<&'static str>` — `latency.rs:18-38` 返回 `Some("bad")`/`Some("good")`/`None`。
- `pub(super) fn record_last_quality(state: &AppState, current: &str)` — `latency.rs:40-42`。
- `pub(super) fn notify_quality_change(app_handle: &AppHandle, kind: &str)` — `latency.rs:44-52` bad/good 发不同通知。
- `pub(super) async fn wait_for_adapter_ready(app_handle, cancel_token) -> Option<(String, String)>` — `latency.rs:67-85` 每2s重试等 IP 与 `any_adapter_online`，取消返 None。
- `pub fn spawn_latency_test_loop(app_handle: &AppHandle) -> Result<(), String>` — `latency.rs:78-139`：动态间隔（轻量化下限1800s），每轮重读配置与轻量化系数（`:91-98`），首 tick 吞掉，调 `run_quality_check`（`latency.rs:135`）。
- `pub(super) async fn run_startup_quality_once(app_handle: AppHandle, cancel_token: Arc<CancellationToken>)` — `latency.rs:144-160`：启动即测一次，等待就绪与1s稳定后调 `run_quality_check`（`:159`）。
- 测试（`latency.rs:167-199`）：`degraded_to_bad_level_triggers_pending_bad`（`:167-170`）、`recovery_from_bad_level_triggers_good`（`:173-176`）、`same_level_or_unknown_last_never_notifies`（`:179-183`）、`middle_levels_crossing_never_notifies`（`:186-192`）、`notification_disabled_never_classifies`（`:195-198`）。
- 常量 `SPIKE_CONFIRM_COUNT: usize = 2`（`quality_scheduler.rs:12`）、`SPIKE_CONFIRM_INTERVAL_SECS: u64 = 15`（`quality_scheduler.rs:13`）。
- `async fn perform_quality_check(app_handle, adapter_name: &str, adapter_ip: &str, lightweight: bool) -> Option<serde_json::Value>` — `quality_scheduler.rs:25-51`：`is_quality_checking.try_acquire()` 互斥（`:25-28`），调 `check_network_quality_async`（`:29`），落盘历史与 emit 结果（`:38-43`）。
- `pub(super) async fn run_quality_check(app_handle, adapter_name: &str, adapter_ip: &str, cancel: Option<&CancellationToken>)` — `quality_scheduler.rs:56-122`：首次检测后 `classify_quality_change` 若 `Some("bad")` 触发复核：15s 间隔轻量化再测2次（`:73-106`），全确认才 `notify_quality_change("bad")`（`:107-109`）；其余 kind 直接通知并记录（`:114-119`）。

### 校园网检测与结果发射（`monitor/campus_check.rs`、`monitor/background_emit.rs`）

- `pub struct ConnectionCampusStatus` — `campus_check.rs:6-12`：字段 `on_campus:bool`（行9）、`name:Option<String>`（行10）、`message:String`（行11）（derive Serialize, camelCase）。
- `pub struct AdapterCampusStatus` — `campus_check.rs:22-31`：字段 `name:String`（行25）、`wireless:bool`（行26）、`on_campus:bool`（行27）、`matched_by:Option<String>`（行29）、`message:String`（行30）。
- `pub struct CampusCheckResult` — `campus_check.rs:33-43`：字段 `wifi:Option<ConnectionCampusStatus>`（行36）、`wired:Option<ConnectionCampusStatus>`（行37）、`on_campus:bool`（行38）、`current_ssid:Option<String>`（行39）、`message:String`（行40）、`adapters:Vec<AdapterCampusStatus>`（行42）。
- `pub(super) fn adapter_campus_status<'a>(...)` — `campus_check.rs:45-49` 按适配器名取逐卡状态。
- `pub(super) fn adapter_campus_message(...)` — `campus_check.rs:51-53`。
- `fn campus_name_list(value: &str) -> Vec<&str>` — `campus_check.rs:57-63` 支持多分隔符解析。
- `struct CampusContext` — `campus_check.rs:192-202`：字段 `enable_name_check`、`name_list`、`campus_gw`、`portal_host`、`wifi_ssid`、`wired_profile`。
- `impl CampusContext::name_hit` — `campus_check.rs:81-98` 名称腿按卡类型套用。
- `fn judge_adapter(...)` — `campus_check.rs:106-162` 逐卡判定：无IP判非校园（`:120-122`）→名称（`:125-127`）→网段（`:130-136`）→网关（`:139-145`）→Portal（`:148-154`），全失败返 `None`（`:157-161`）。
- `fn aggregate_class(...)` — `campus_check.cs:166-173`；`fn join_messages(...)` — `campus_check.cs:176-186`。
- `pub fn check_campus_network(config: &Config, adapters: &[Adapter]) -> CampusCheckResult` — `campus_check.rs:350-435`：名称检查禁用时不调 netsh（`:195-202`）；逐卡 `judge_adapter`（`:236-239`）；聚兼容字段 `wifi`/`wired`（`:241-246`）；汇总 `on_campus` 与 `message`（`:248-259`）。
- `pub use crate::config::schedule::is_campus_check_silent;` — `campus_check.cs:285` 静默期判定转交共享面。
- 测试（`campus_check.cs:287-435`）：`名称名单_解析多分隔符与空项`（`:317-325`）、`无ip的卡_直接判非校园且不探测`（`:328-339`）、`名称命中_按卡类型套用且短路不探测`（`:342-358`）、`网段命中_零探测短路`（`:361-373`）、`路由器私网_网关腿绑源命中`（`:376-388`）、`网关失败但portal命中_判校园`（`:391-397`）、`网关与portal都失败_判非校园`（`:400-407`）、`名称检查禁用时_名称腿不参与`（`:410-417`）、`聚合_命中卡文案优先_无卡返回none`（`:420-431`）。
- `pub(super) struct BackgroundCheckResult<'a>` — `background_emit.rs:14-34`：字段 `online:bool`（行15）、`reachable:bool`（行16）、`login_available:bool`（行17）、`message:&'a str`（行18）、`adapter1_name:&'a str`（行19）、`adapter2_name:&'a str`（行20）、`secondary_online:Option<bool>`（行21）、`secondary_message:&'a str`（行22）、`online_operator:Option<String>`（行24）、`secondary_online_operator:Option<String>`（行26）、`dual_adapter:bool`（行27）、`config:&'a Config`（行28）、`campus_result:&'a CampusCheckResult`（行29）、`a1_campus_msg:Option<&'a str>`（行30）、`a2_campus_msg:Option<&'a str>`（行31）、`a1_on_campus:Option<bool>`（行32）、`a2_on_campus:Option<bool>`（行33）。
- `pub fn adapter_status_entry(...)` — `background_emit.rs:43-48`；`adapter_disabled_entry`（`:43-45`）；`adapter_disconnected_entry`（`:47-49`）。
- `pub(super) fn build_adapter_details(...)` — `background_emit.rs:51-65`。
- `pub(super) fn handle_status_change(...)` — `background_emit.rs:67-107`：状态变更发通知（60s 去抖 `:94-101`）。
- `pub(super) fn emit_background_check_result(...)` — `background_emit.rs:109-167`：递增计数（`:116-119`）、注销保护期强制 offline（`:127-134`）、emit 前端事件（`:136-164`）。
- `pub(super) fn update_network_state(...)` — `background_emit.rs:169-205`：更新 `any_adapter_online` 等，注销保护期跳过（`:183-186`）。

### 定时动作与夜间出站编排（`monitor/scheduled.rs`、`monitor/outbound_switch.rs`、`monitor/outbound_night.rs`）

- 常量 `SCHEDULED_TICK_MS: u64 = 30_000`（`scheduled.rs:44`）、`NIGHT_VERIFY_WAIT_SECS: u64 = 15`（`scheduled.rs:48`）、`NIGHT_VERIFY_RETRY_SECS: u64 = 5`（`scheduled.rs:49`）、`NIGHT_VERIFY_ATTEMPTS: u32 = 3`（`scheduled.rs:50`）、`NIGHT_VERIFY_HTTP_TIMEOUT_SECS: u64 = 15`（`scheduled.rs:51`）、`SCHEDULED_LOGIN_MAX_RETRIES: u32 = 20`（`scheduled.rs:346`）。
- `pub async fn run_scheduled_action_loop` — `scheduled.rs:174-318`：启动前 `reconcile_outbound_on_startup`（`:61`）与 `reconcile_scheduled_logout_on_startup`（`:63`）；循环内 tokio::select 等 tick/cancel（`:66-72`）；`is_quitting` 判断退出（`:77-79`）；算 now_minutes/today_day/config_snapshot（`:80-86`）；用拍前 `outbound_switch_active`（`:90`）门控 `evaluate_scheduled`（`:91-94`）得 (fire_login,fire_logout) 调 `run_scheduled_login`/`run_scheduled_logout`（`:95-100`）；再算 restore_active 调 `evaluate_outbound_action_with_restore`（`:114-123`）匹配 `NightOutboundAction` 三分支经 `run_outbound_blocking` 执行 `apply_outbound_switch`/`apply_outbound_restore` 或 replay/watchdog（`:124-171`）；拍后取 `outbound_active`（`:174`）门控 `evaluate_night_switch_gated_with_restore`（`:183-193`）得 night_action 调 `apply_night_switch_action`（`:194-196`）。
- `fn is_quitting` — `scheduled.rs:202-204` 抽离避免跨 await 持 State 守卫。
- `fn reconcile_scheduled_logout_on_startup` — `scheduled.rs:214-229` 冷启动若 now_minutes>=scheduled_logout_minutes 置 logout_day 消耗额度不执行。
- `fn evaluate_scheduled` — `scheduled.rs:238-249` 装配 AppState 调共享纯函数 `evaluate_scheduled_actions`。
- `fn mark_scheduled_day_fired` — `scheduled.rs:256-263`（`#[cfg_attr(not(test), allow(dead_code))]`）置 login_day/logout_day。
- `fn format_minutes` — `scheduled.rs:265-267` 补零格式化。
- `fn run_scheduled_login` — `scheduled.rs:272-279` 调 `spawn_login_action` 传 Some(today_day)。
- `fn spawn_login_action` — `scheduled.rs:285-341`：抢 `tasks.is_logging_in` 锁（`:291`），抢到后若 login_quota_day 有值置额度（`:306-308`），调 `auth::service::full_login`（`:309`）与 `post_login_handler`（`:319`），emit 日志（`:311-314`）；失败补试回退额度（`:320-339`）。
- `fn apply_night_switch_action` — `scheduled.rs:366-393`：改内存 config（SwitchToCampus 将原 operator 存 night_operator_restore 并置空串=无锡学院，Restore 反之）（`:356-364`），调 `save_config_and_broadcast` 落盘（`:366`）失败则不 store 返回；成功 store 后调 `spawn_login_action`（None 额度，`:373`）并 spawn `verify_night_switch`（`:377`）。
- `fn run_scheduled_logout` — `scheduled.rs:385-389` spawn_blocking 调 `perform_logout`。
- `fn perform_logout` — `scheduled.rs:396-430`：抢 `tasks.is_logging_out` 锁（`:399`），成功后置 logout_day 额度（`:408-410`），调 `full_logout`（`:411`）；成功则取消自动退出、重置 failure_tracker、启动 60s 注销保护期（`:418-429`）。
- `async fn verify_night_switch` — `scheduled.rs:465-502`：等 15s，`night_verify_config` 检查 operator 仍匹配否则跳过（`:438`）；`night_switch_verify_round` 重试 3 次间隔 5s（`:441`）；不符则注销重登再验（`:453-456`），最终不符发告警日志+`emit_notification`（`:463-471`）。
- `fn night_verify_config` — `scheduled.rs:477-481` operator != expected_operator 返 None 跳过验证。
- `async fn night_switch_verify_round` — `scheduled.rs:487-505` 用 `create_safe_http_client` 做 NIGHT_VERIFY_ATTEMPTS 轮。
- `async fn night_switch_check_once` — `scheduled.rs:506-530` 构造 chkstatus URL（经 `portal_origin`），parse_chkstatus+uid_matches 核对。
- `pub(crate) fn portal_origin` — `scheduled.rs:534-544` 取 portal_url 的 origin，空值回退 default_portal_url。
- `fn expected_operator_display` — `scheduled.rs:547-553` 空串显"无锡学院"。
- 测试模块（`scheduled.rs:555-676`）：`禁用目标_不触发_不置标记`（`:563-570`）、`到点触发_置当日标记`（`:573-592`）、`跨天重置_次日再触发`（`:595-602`）、`切换态_跳过定时登录且不消耗当日标记_注销照常`（`:605-631`）、`format_minutes_补零`（`:634-637`）、`定时登录补试计数_累加清零跨天归零`（`:641-654`）、`定时登录失败回退额度_补试用尽后当日停止`（`:659-675`）。

- `pub fn is_campus_adapter(ip, gateway, campus_gateway, portal_host, gateway_probe: Fn(&str,&str)->bool) -> bool` — `outbound_switch.rs:17-39`：IP 空否（`:24-26`）；同 /18 真（`:27-29`）；否则绑源探校园网关或 portal 两发去抖任一可达即真（`:35-38`）。
- `pub fn portal_probe_host(portal_url) -> String` — `outbound_switch.rs:44-56` 从 portal_url 提取私网 IPv4 主机，域名/公网/空返空串。
- `pub fn effective_outbound_priority(priority: &[String], details: &[(Adapter,String)]) -> Vec<String>` — `outbound_switch.rs:63-82` 非空透传，空则无线优先后有线保发现序。
- `pub fn select_outbound_candidate` — `outbound_switch.rs:87-111` 按优先级选首张非校园、有 IP、有网关且自身网关可达卡 -> Option<Adapter>。
- `struct SnapshotRow` — `outbound_switch.rs:115-121` 含字段 `guid:String`/`family:u16`/`automatic:bool`/`metric:u32`，序列化时补 guid。
- `pub fn snapshot_json` — `outbound_switch.rs:125-136` 序列化选中卡跃点快照，兜底 `"[]"`。
- `pub struct DisabledRow` — `outbound_switch.rs:141-145` 含字段 `guid:String`/`name:String`，记录本功能亲手禁用卡。
- `pub fn disabled_adapters_json` — `outbound_switch.rs:148-150` 序列化禁用名单，兜底 `"[]"`。
- `pub fn parse_disabled_adapters` — `outbound_switch.rs:156-161` 空串->空名单 Ok，非法 JSON->Err。
- `pub struct StandbyRoute` — `outbound_switch.rs:168-176` 含字段 `dest:String`/`mask:String`/`gateway:String`/`metric:u32`/`if_index:u32`，历史兜底默认路由快照。
- `pub fn parse_standby_route` — `outbound_switch.rs:179-181` 空/非法->None。
- `pub enum StandbyCleanup` — `outbound_switch.rs:186-196` 变体 `AlreadyGone`/`ForeignRowKept`/`LastDefaultPath`/`Safe`。
- `pub fn standby_cleanup_decision` — `outbound_switch.rs:203-220` 纯函数，按 same_key_metrics/standby_metric/rows_outside_exact 判。
- `pub fn standby_route_cleanup` — `outbound_switch.rs:224-264` Windows 实现读路由表判定，非 Windows 返 `AlreadyGone`（`:267-270`）。
- `pub(crate) fn unsafe_to_disable` (cfg windows) — `outbound_switch.rs:278-280` 实例 ID 读不到返 true 保守跳过。
- `pub fn select_campus_to_disable(priority, details, campus_gateway, portal_host, exclude_guid, gateway_probe, bus_guard) -> Vec<DisabledRow>` — `outbound_switch.rs:285-318` 过滤目标卡/无 IP/非校园/guard 命中，同名去重。
- 测试模块（`outbound_switch.rs:320-691`）：覆盖 `is_campus_adapter`、`select_outbound_candidate`、`effective_outbound_priority`、`snapshot_json`、`disabled_adapters_json`、`parse_standby_route`、`select_campus_to_disable`、`portal_probe_host`、`standby_cleanup_decision` 等。

- `outbound_night.rs` 常量与辅助：`OUTBOUND_HELPER_TIMEOUT_SECS: u64 = 30`(`:20`)、`OUTBOUND_BACKOFF_START_MS: u64 = 60_000`(`:24`)、`OUTBOUND_BACKOFF_MAX_MS: u64 = 300_000`(`:25`)、`OUTBOUND_RESTORE_ALERT_FAILS: u32 = 3`(`:30`)、`OUTBOUND_RESTORE_ALERT_REMIND_EVERY: u32 = 20`(`:32`)、`OUTBOUND_RESTORE_GIVE_UP_FAILS: u32 = 40`(`:36`)。
- `fn outbound_restore_alert_due(count: u32) -> bool` — `:39-42`；`fn notify_outbound_issue_once_per_day` — `:46-52`；静态原子变量退避记账 — `:55-72`；`static OUTBOUND_ACTION_BUSY` — `:78`；`struct OutboundActionGuard` RAII 锁 — `:81-96`；`pub(super) async fn run_outbound_blocking` — `:100-112`；`pub(super) fn outbound_switch_active` — `:116-123`；`fn in_outbound_restore_window` — `:128-131`；`fn outbound_backoff_ms` — `:141-148`；`fn backoff_gate` — `:152-157`；`fn switch_gate_open`/`restore_gate_open` — `:159-173`；`fn mark_outbound_failure`/`clear_outbound_failure` — `:176-185`；`fn outbound_allow_uac` — `:188-190`；`fn note_channel_failure`/`clear_outbound_channel_failure` — `:193-202`；`pub(super) fn needs_replay` — `:207-209`；`struct OutboundSnapshotRow` — `:214-219`；`fn parse_outbound_snapshot` — `:222-227`；`struct HelperFailure` — `:230-237`；`fn run_helper_op` — `:243-269`；`fn enable_campus_adapter` (cfg desktop `:280-291` / not desktop `:294-300`)；`fn write_outbound_metric` — `:308-369`；`OUTBOUND_PROBE_HOST` 常量 (`:399`)；`fn probe_target_internet` — `:414-419`；`fn persist_config_update` — `:425-431`；`fn set_pending_switch` — `:436-444`；`pub(super) fn drive_pending_switch` — `:453-467`；`pub(super) fn apply_outbound_switch` — `:469-691`；`pub(super) fn replay_outbound_switch` — `:708-796`；`pub(super) fn watchdog_re_disable_campus` — `:805-872`；`pub(super) fn apply_outbound_restore` — `:886-1199`；`fn finish_outbound_restore` — `:1206-1227`；`fn clear_outbound_snapshot` — `:1231-1251`；`fn set_outbound_manual_hold`/`clear_outbound_manual_hold` — `:1257-1270`；`pub(crate) fn manual_outbound_switch` — `:1276-1297`；`pub(crate) fn manual_outbound_restore` — `:1301-1314`；`pub(super) fn reconcile_outbound_on_startup` — `:1321-1379`。
- 测试（`outbound_night.rs:1381-1491`）：`还原失败告警_首达阈值触发_此后周期重复`(`:1386-1398`)、`重放闸_仅切换态且留有失败历史时才补齐`(`:1401-1411`)、`退避窗_起步六十秒_每失败翻倍封顶三百秒`(`:1414-1421`)、`退避闸_无失败历史恒放行`(`:1424-1429`)、`退避闸_首败六十秒内压制_六十秒后放行`(`:1432-1436`)、`退避闸_第二败退避翻倍`(`:1439-1443`)、`退避闸_封顶后三百秒内压制_三百秒后放行`(`:1446-1452`)、`退避闸_时钟回拨按压制处理`(`:1455-1457`)、`快照解析_空串为空表_非法json报错`(`:1460-1470`)、`单飞闸_同一时刻只放一个出站动作_释放后可再取`(`:1475-1487`)。

### 适配器监控（`monitor/adapter_watch.rs`）

- 常量 `ADAPTER_WATCH_INTERVAL: u64 = 15000`（`adapter_watch.rs:9`）、`CLASS_SUBKEY_REFRESH_ROUNDS: u32 = 4`（`adapter_watch.rs:15`）。
- `pub fn start_adapter_watch(app_handle: &AppHandle) -> Result<(), String>` — `adapter_watch.rs:17-288`：15s 循环检测适配器集合/IP 变化与禁用状态变化，首轮强制推送（`adapter_watch.rs:26`）；60s 刷新 class subkey 缓存（`:57-62`）；读缓存适配器（`:68`）；变更时 emit 事件（`:91-100`）；适配器恢复时触发一次性巡检 `spawn_background_check_once`（`:115`）；配置的卡被禁用发警告（`:118-163`）；夜间出站切换守护窗内跳过自动启用（`:184-215`）；手选适配器被禁用时自动启用（`:237-269`）。
- `fn auto_enable_backoff_ms(failure_count: u32) -> u64` — `adapter_watch.rs:292-299`：0→0,1→60s,2→120s,≥3→300s。
- 测试：`auto_enable_backoff_ladder`（`adapter_watch.rs:306-313`）。

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
│ → background_check.rs:50     │ │ → quality_scheduler.rs:56    │
│ run_background_check_blocking│ │ run_quality_check            │
└──────────┬───────────────────┘ └──────────────────────────────┘
           │ calls
           ▼
┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐
│ portal_check.rs:59│ │ auto_auth.rs:94  │ │ scheduled.rs:54  │
│ check_adapter_    │ │ try_auto_login_  │ │ run_scheduled_   │
│ portal           │ │ on_preparation   │ │ action_loop      │
└──────────────────┘ └──────────────────┘ └────────┬─────────┘
                                                   │ uses
                                                   ▼
                                         ┌──────────────────────┐
                                         │ outbound_night.rs:469│
                                         │ apply_outbound_*     │
                                         │ + outbound_switch.rs │
                                         │ is_campus_adapter    │
                                         └──────────────────────┘
```

## Data Flow

### 启动挂载
- `watcher.rs:16` `run_startup_tasks` 依据配置 spawn：`startup_bg_check`（`watcher.rs:16-26` → `background_task.rs:7`）、`startup_quality_once`（`watcher.rs:16-23` → `latency.rs:144`）、`startup_latency`（`watcher.rs:16-26` → `latency.rs:144`）、`startup_auto_login`（`watcher.rs:16-21` → `auto_auth.rs:317`）、`scheduled_actions`（`watcher.rs:16-21` → `scheduled.rs:54`）。

### 单轮巡检主链
- `background_task.rs:30-57` 循环调用 `background_check::run_background_check`（`background_check.rs:546` → `:50`），`DynamicInterval::next_ms` 决定下一拍间隔（`background_task.rs:41`）。
- `background_check.rs:54` `is_checking.try_acquire()` 互斥（`:54-56`）；`:63-73` 适配器 cached/force；`:84-91` `outbound_restore_active` 跳过；`:119-127` 静默期跳过并强制 `on_campus=true`；`:212-224` 双适配器并行 `portal_check::check_adapter_portal`（`:59`）；`:248-290` 失败计数；`:436` `auto_auth::try_auto_login_on_preparation`；`:438-442` `auto_auth::try_disconnect_reconnect`；`:446-448` 未重连时 `update_network_state`（经 `background_emit.rs:169`）。
- `should_reset_logged_online`（`background_check.cs:25`, `:390-396`）修复休眠会话失效。

### 质量链
- `latency.rs:78` `spawn_latency_test_loop` 就绪等待后调 `quality_scheduler.rs:56` `run_quality_check`；`:73-106` bad 复核 2×15s；`perform_quality_check`（`quality_scheduler.rs:19`）调 `check_network_quality_async` 落盘 emit。

### 定时动作循环
- `scheduled.rs:54` 30s 循环：`:91-100` 定时登录/注销；`:102-171` 出站切换编排调 `super::outbound_night` 动作（底层 `outbound_switch.rs` 选择）；`:178-196` 运营商夜切 `apply_night_switch_action` 改配置并 `verify_night_switch`（`:436`）。

### 夜间出站还原补检
- `background_task.rs:149` `spawn_background_check_after_outbound_restore` 阻塞轮询等 IP 就绪（最长45s）后 `spawn_background_check_once`（`:181`）；该钩子由 `outbound_night.rs:1206` `finish_outbound_restore` 在还原收尾时统一调用（先 `request_check_after_outbound_restore` 置 `RESTORE_SILENT_PASS` 标记，`:1224`）。

## Connections

- [[desktop-auth]]：`auto_auth.rs:163`（内部 `full_login` 调用）、`scheduled.rs:309` 同；`portal_check.rs:63` 调 `auth::portal::check_portal_full`；`background_check.rs:260,268` 调 `handle_portal_request_failure`（阈值见 failure_tracker）。
- [[desktop-network-quality]]：`quality_scheduler.rs:29` 调 `check_network_quality_async` 并发 `emit_network_quality_result`。
- [[desktop-config]]：依赖 `config::persist::save_config_and_broadcast`（`background_task.rs:63`、`scheduled.rs:366`、`outbound_night.rs:427`）、共享纯函数 `evaluate_scheduled_actions`/`evaluate_outbound_action_with_restore`/`evaluate_night_switch_gated_with_restore`（`scheduled.rs:93,114,183` 等）。
- [[desktop-infra]]：`infra::events::EventBus` 与 `infra::notification::emit_notification`（`scheduled.rs` 使用）、`CancellationToken` 跨任务传递。
- [[night-outbound-switch]]：`scheduled.rs` 与 `outbound_switch.rs`/`outbound_night.rs` 为桌面动作层，`outbound_night` 提供 `apply_outbound_switch`/`apply_outbound_restore` 等（`scheduled.rs:36-40` 引用）。
- [[desktop-network-core]]：`background_check.rs:64` 调 `get_adapters_cached`/`get_adapters_force`，`outbound_switch.rs:11` 用 `crate::network::discovery::Adapter`。

## Known Issues

基于观察，无未解决缺陷报告；以下为已处理的边界条件与非显然行为（均已在代码中实现对应逻辑）：

1. **并发互斥**：`background_check.cs:54` `is_checking.try_acquire()` 安全跳过叠加轮次；`quality_scheduler.rs:25` `is_quality_checking`；`auto_auth.rs:160,238` `is_logging_in` 锁。
2. **适配器获取降级**：cached 空降级 force（`background_check.cs:63-73`）；夜间补检特意用 force 避旧缓存（`background_task.rs:130`）。
3. **出站切换态跳过**：`outbound_restore_active` 真时整轮巡检跳过（`background_check.cs:84-91`）。
4. **校园网静默**：静默期跳过验证但强制 `on_campus=true`（`background_check.cs:119-127`）；`auto_auth` 开机自启同理（`auto_auth.rs:388-400` 静默置 true）；还原后补检 `forced` 可无视静默期（`background_check.cs:128-133`）。
5. **登录标记复位（关键修复）**：`should_reset_logged_online`（`background_check.cs:25,390-396`）复位 `has_logged_online`，解决整夜休眠后自动登录被永久门控；校园网检测失败也重置（`:143-150`）。
6. **自动登录防无限重试**：`PREP_LOGIN_MAX_FAILURES=5`（`auto_auth.rs:206`）；达上限指数退避封顶 300s（`auto_auth.rs:40-50` 的 `prep_login_backoff_secs`）；时间窗拒绝不计入退避（`auto_auth.rs:78-84` 的 `prep_failures_after_failure`）；`try_disconnect_reconnect`（`auto_auth.rs:206-315`）超限后每 10 拍放行一次（`auto_auth.rs:206-220` 中 `RECONNECT_REMINDER_INTERVAL` 逻辑）。
7. **重连状态覆盖修复**：`try_disconnect_reconnect` 返回真实 `reconnect_should_report`（`auto_auth.rs:314`），调用方依此跳过旧 Portal 快照覆盖（`background_check.cs:446-448`）。
8. **Portal 请求失败容错**：`Error{is_request_failed:true}` 累计阈值（`background_check.cs:248-290`），成功重置（`background_check.cs:299-317`）；MAC 重置检测仅每日 08:30 后启用（`dynamic_interval.rs:20,23-25`，`background_check.cs:252`）。
9. **夜间出站还原补检缺口**：新增 `spawn_background_check_after_outbound_restore`（`background_task.rs:149-183`）最长等45s轮询IP；还原收尾 `finish_outbound_restore`（`outbound_night.rs:1206-1227`）先置 `RESTORE_SILENT_PASS`（经 `request_check_after_outbound_restore`，`:1224`）再调补检。
10. **延迟测试动态间隔**：轻量化下限1800s，每轮重读配置（`latency.rs:91-98`），吞首 tick（`latency.rs:86-114`）；启动即测一次不再以定时开关为前提（`watcher.rs:36-43`）。
11. **质量抖动复核**：poor/bad 需15s×2次轻量化复核全达标才通知（`quality_scheduler.rs:68-112`）；中间档互转不通知（`latency.rs:18-38` 函数逻辑）。
12. **就绪等待**：`wait_for_adapter_ready` 每2s等IP与在线（`latency.rs:58-76`）。
13. **任务注册失败处理**：watcher 各 spawn 失败仅 `log_warn`（`watcher.rs:29,41,50,61,70`）；`spawn_background_check_once` 复用令牌补跑（`background_task.rs:86-97`）。
14. **双适配器并行实现**：`run_background_check_blocking` 在 spawn_blocking 内用 `Handle::current().block_on`+`join!`（`background_check.cs:212-224`）。
15. **出站与运营商夜切互斥**：`scheduled.rs:90` 拍前出站门控登录，`scheduled.rs:174` 拍后切换态门控夜切；`outbound_switch_active` 快照非空即切换态。
16. **历史路由清理**：`outbound_switch.rs:168-220` 不再创建 StandbyRoute，仅解析历史并安全决策（2026-10-18 事故后）；`outbound_night.rs` 还原时按四元组精确清理（`apply_outbound_restore` `:935-1021`）。
17. **适配器监控与自动启用**：`adapter_watch.rs:17-288` 检测禁用并自动启用，夜间守护窗跳过（`adapter_watch.rs:184-215`）；切换态亲手禁用名单不触发告警与自动启用（`:146-149,207-209`）。
