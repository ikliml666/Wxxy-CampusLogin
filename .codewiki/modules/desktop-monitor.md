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

本模块是桌面端后台巡检中枢。`monitor/mod.rs:1-14` 声明 14 个子模块：`watcher`(行1)、`auto_auth`(行2)、`latency`(行3)、`adapter_watch`(行4)、`campus_check`(行5)、`outbound_switch`(行6)、`portal_check`(行7)、`quality_scheduler`(行8)、`background_emit`(行9)、`background_check`(行10)、`background_task`(行11)、`outbound_night`(行12)、`scheduled`(行13)、`dynamic_interval`(行14)。启动期由 `watcher::run_startup_tasks`（`watcher.rs:4-60`）统一编排后台巡检、启动首测、定时测试循环、开机自启登录与定时动作循环。

核心巡检逻辑在 `background_check`：独占连通性/Portal/重连；质量检测由 `latency` + `quality_scheduler` 的独立循环负责（2026-09-04 收敛，后台巡检不再触发全量质量检测）；巡检间隔由 `dynamic_interval` 状态机自适应（未登录按配置基础间隔 ×2 退避封顶 5 分钟、已登录 10 分钟、已登录但门户不可达 1 分钟快排查，见 `dynamic_interval.rs:1-11` 模块文档）；定时登录/注销与夜间出站切换编排在 `scheduled` 的 30s 循环；底层动作层 `outbound_switch`（纯函数判定与快照序列化）与 `outbound_night`（提权执行、退避闸、预判定计划）。

模块主要面向 Windows 桌面端；非 Windows 平台对部分提权/路由能力做 stub：`outbound_switch.rs:267-270` 非 Windows 的 `standby_route_cleanup` 恒返回 `StandbyCleanup::AlreadyGone`，`outbound_night.rs:295-300` 非 desktop 的 `enable_campus_adapter` 恒返回 `Err`（`#[cfg(desktop)]` 版本在 `:280-291`）。

## Key Components

### 模块索引与启动编排（`monitor/mod.rs`、`monitor/watcher.rs`）

- `monitor/mod.rs:1-14` 声明 14 个子模块。
- `pub use super::background_task::start_background_check_inner;` — `watcher.rs:8`（审计 A9 注释见 `:8-9`：`run_background_check` 的外部调用统一走 `background_task::spawn_background_check_once`，原 re-export 已移除）。
- `pub use super::campus_check::check_campus_network;` — `watcher.rs:13`。
- `pub use super::background_emit::{adapter_status_entry, adapter_disabled_entry, adapter_disconnected_entry};` — `watcher.rs:14`。
- `pub fn run_startup_tasks(app_handle: &AppHandle)` — `watcher.rs:16-72`，启动期统一编排入口：
  - `config.enable_background_check` 为真时 spawn `startup_bg_check` 调 `start_background_check_inner`（`watcher.rs:21-31`）。
  - `config.enable_network_quality` 时 spawn `startup_quality_once` 调 `run_startup_quality_once`（2026-10-25 修订：只要质量总开关开启即启动首测，不再以定时测试开关为前提；`watcher.rs:36-43`）。
  - `enable_network_quality && enable_latency_test` 时 spawn `startup_latency` 调 `spawn_latency_test_loop`（`watcher.rs:45-55`）。
  - 无条件 spawn `startup_auto_login` 调 `run_auto_login_on_start`（`watcher.rs:57-62`）。
  - 无条件 spawn `scheduled_actions` 调 `scheduled::run_scheduled_action_loop`（独立轻量循环，不受 `enable_background_check` 开关管辖；`watcher.rs:64-71`）。
  - 各任务注册失败仅 `log_warn!` 不阻断（`watcher.rs:29,41,53,61,70`）。

### 动态检测间隔状态机（`monitor/dynamic_interval.rs`）

- 常量 `BACKOFF_CAP_MS: u64 = 300_000`（`dynamic_interval.rs:14`）、`ONLINE_INTERVAL_MS: u64 = 600_000`（`:16`）、`FAST_INTERVAL_MS: u64 = 60_000`（`:18`）、`PORTAL_RESET_START_MINUTES: u16 = 8 * 60 + 30`（`:20`）。
- `pub fn portal_reset_window_open(now_min: u16) -> bool` — `dynamic_interval.rs:23-25`，判定 `now_min >= PORTAL_RESET_START_MINUTES`（即每日 08:30 之后启用 MAC 重置检测）。
- `pub enum CheckOutcome` — `dynamic_interval.rs:28-39`：变体 `NotOnline`（未在线，行31）、`Online`（在线且门户可达，行33）、`OnlineButPortalUnreachable`（此前在线但本轮门户不可达，行35）、`Skipped`（本轮未执行有效检测：退出中/抢锁失败/取适配器失败/出站切换态等早退，行38）。
- `pub struct DynamicInterval` — `dynamic_interval.rs:43-47`：字段 `offline_streak: u32`（连续未在线轮数，行44）、`last_ms: Option<u64>`（上一拍计算出的间隔，Skipped 时沿用，行46）。
- `impl DynamicInterval::next_ms(&mut self, outcome: CheckOutcome, base_ms: u64) -> u64` — `dynamic_interval.rs:52-73`：`OnlineButPortalUnreachable` 复位并返回 `FAST_INTERVAL_MS`（`:54-57`）；`Online` 复位并返回 `ONLINE_INTERVAL_MS.max(base_ms)`（已登录档尊重更大的用户配置，`:58-62`）；`NotOnline` 累计 `offline_streak` 走 `backoff_ms`（`:63-66`）；`Skipped` 沿用 `last_ms`、既不累计退避也不复位（`:67`）；非 Skipped 时记录 `last_ms`（`:69-71`）。
- `pub fn backoff_ms(base_ms: u64, streak: u32) -> u64` — `dynamic_interval.rs:78-88`：未登录退避（基础间隔下限 10s，×2^(streak-1)，封顶 `BACKOFF_CAP_MS`，用户基础间隔大于封顶时不缩短）。
- 测试模块（`dynamic_interval.rs:143-163`）：`未登录按2的幂退避封顶5分钟`（`:94-102`）、`用户基础间隔大于封顶时不缩短`（`:104-107`）、`已登录间隔10分钟且复位退避`（`:109-119`）、`已登录但门户不可达降为1分钟`（`:121-131`）、`mac重置检测窗口8点30分整点开启`（`:133-139`）；`skipped_tests` 模块（`:142-163`）：`skipped沿用上一拍且不累计退避`（`:146-155`）、`已登录档尊重更大的用户配置`（`:157-162`）。

### 巡检任务生命周期（`monitor/background_task.rs`）

- `pub fn start_background_check_inner(app_handle: &AppHandle, state: &AppState) -> Result<CommandResult, String>` — `background_task.rs:7-68`：置 `enable_background_check=true`、间隔 `<10000` 提至 `15000`（`:8-13`）；spawn `background_check` 任务首启直检（`:19-28`），循环内每轮重读配置基础间隔 `cfg.background_check_interval.max(10000)`（`:31-34`），轻量化下限 300s 只作用于退避基础值（`effective_background_interval_ms`，快排查/已登录档不被抬回，`:36-40`），经 `DynamicInterval::next_ms` 得出下一拍间隔（`:41`）；`tokio::select!` 支持取消令牌（`:43-51`）；`is_running("background_check")` 与 `is_quitting` 退出（`:52-55`）；迭代执行 `run_background_check`（`:27-28,56`）；落盘走 `config::persist::save_config_and_broadcast`（`:63-65`）。
- `pub fn spawn_background_check_once(app_handle: &AppHandle)` — `background_task.rs:78-98`（审计 A9 统一发起入口）：spawn `background_check_once` 调 `run_background_check`（`:80-85`）；同名一次性检测已在跑时复用其注册令牌补跑一轮，`is_checking` 信号量兜底并发（`:86-97`）。
- 常量 `POST_RESTORE_IP_WAIT_MAX_MS: u64 = 45_000`（`background_task.rs:112`）、`POST_RESTORE_IP_POLL_MS: u64 = 3_000`（`:113`）。
- `fn post_restore_wait_should_continue(waited_ms: u64, max_ms: u64) -> bool` — `background_task.rs:119-121` 纯函数，`waited_ms < max_ms`（达到上限即收尾，不多睡一轮）。
- `fn post_restore_adapters_have_ip(app_handle: &AppHandle) -> bool` — `background_task.rs:127-137`：用 `get_adapters_force` 枚举（cached 带 TTL 会返回启用前旧快照），主副适配器至少一张拿到 IP。
- `pub fn spawn_background_check_after_outbound_restore(app_handle: &AppHandle)` — `background_task.rs:149-183`：阻塞线程池轮询等 IP 就绪（最长 45s，`:153-162`），join 失败按等满上限处理（`:166`），未读到 IP 也照发巡检（`:168-180`），结束后调 `spawn_background_check_once`（`:181`）。
- 测试：`post_restore_wait_stops_at_cap`（`background_task.rs:191-197`）、`post_restore_wait_zero_budget_never_waits`（`:199-202`）。

### 巡检主体与 Portal 检测（`monitor/background_check.rs`、`monitor/portal_check.rs`）

- `pub(crate) fn should_reset_logged_online(reachable: bool, any_online: bool, has_logged_online: bool) -> bool` — `background_check.rs:25-27` 纯函数，`reachable && !any_online && has_logged_online`（三条件缺一不可：探测失败/无 IP 不判会话失效）。
- `static RESTORE_SILENT_PASS: AtomicBool` — `background_check.rs:42-43`；`pub(crate) fn request_check_after_outbound_restore()` — `:46-48` 置「还原后在静默期内持续放行」标记（2026-10-26 语义：放行持续到本轮静默期结束，离开静默期即清除，不跨日残留）。
- `pub(crate) fn run_background_check_blocking(app_handle, state, cancel_token) -> CheckOutcome` — `background_check.rs:50-467`：
  - 退出/取消即返 `Skipped`（`:51-53`）；`is_checking.try_acquire()` 并发跳过（`:54-56`）。
  - 适配器取 `get_adapters_cached`，空则降级 `get_adapters_force`（`:63-73`）；`resolve_adapter_names` + `find_dual_adapters`（`:77-79`）。
  - 出站切换态（三份快照任一非空）`outbound_restore_active` 整轮跳过（`:84-91`）；校园网静默期判定 `is_campus_check_silent`（`:92-96`）；还原后放行标记读取与清除（`:98-102`）；静默期且未放行时跳过本轮（清 `on_campus_network`、取消最小化+退出流程、按 `Skipped` 交还节奏，`:119-127`）；静默期且放行时照常巡检（`:128-133`）。
  - 校园网检测走 `check_campus_network`（`:135-136`），更新 `on_campus_network`/`current_ssid`，失败时重置 `any_adapter_online`/`last_a1_online`/`has_logged_online`（`:139-152`）；失败分支 emit 结果，配置的适配器均无 IP 时跳过退出等待网络恢复，否则 `start_campus_exit`（`:154-196`，`start_campus_exit` 在行192）；验证通过 `cancel_campus_exit`（`:199`）。
  - Portal 检测（`:205-239`）：双适配器且两卡齐备时用 `Handle::current().block_on` + `tauri::async_runtime::spawn_blocking` + `tokio::join!` 并行（`:212-224`）；否则串行/NotFound 兜底（`:225-239`）。
  - Portal 请求失败容错（`:243-290`）：任一适配器 `is_request_failed` 时，仅当 `portal_reset_window_open(now_min)`（行252）才调 `handle_portal_request_failure` 按适配器累加计数（A1 `:259-264`、A2 `:267-272`）；窗口未开则清零残留计数（`:273-289`）。
  - 成功按适配器独立重置失败计数（`:295-317`）。
  - 组装 `online`/`online_operator`（Portal uid 推导运营商后缀，`:324-336`）、`secondary_online`/`secondary_message`/`secondary_online_operator`（`:347-373`）、`any_online`（行380）。
  - `should_reset_logged_online` 判定重置 `has_logged_online`（必须在 `try_auto_login_on_preparation` 之前，本次巡检即可登录；`:381-396`）。
  - 调 `handle_status_change`（`:397-402`）与 `emit_background_check_result`（`:413-434`）；调 `try_auto_login_on_preparation`（`:436`）与 `try_disconnect_reconnect`（`:438-442`）；仅未重连时 `update_network_state`（`:446-448`）。
  - 结局分类（`:450-460`）：门户可达且任一卡在线 → `Online`；门户不可达但此前在线 → `OnlineButPortalUnreachable`（联动 `trigger_fast_quality_check`，`:461-463`）；否则 `NotOnline`。
- `fn trigger_fast_quality_check(...)` — `background_check.rs:492-561`：快排查联动质量检测，5 分钟节流（`last_fast_quality_at`，`:478-487`）；`select_adapter_for_quality` 选卡（`:492`）；`quality_scheduler::run_quality_check` 执行（`:504`）；按 `averageExternalLatency > 0` 判定互联网可达性（`:508-513`），结论写登录日志、外网中断时发系统通知（`:516-539`）。
- `pub async fn run_background_check(app_handle, cancel_token: Arc<CancellationToken>) -> CheckOutcome` — `background_check.rs:546-555`：`spawn_blocking` 调 blocking 版，join 异常记日志返 `Skipped`。
- 测试（`background_check.rs:563-586`）：`no_reset_while_online`（`:562-567`）、`no_reset_when_unreachable`（`:569-573`）、`no_reset_when_never_logged_in`（`:575-579`）、`reset_when_reachable_offline_and_logged_before`（`:581-585`）。
- `pub(super) enum PortalCheckResult` — `portal_check.rs:6-19`：变体 `Success { online: bool, message: String, reachable: bool, login_available: bool, uid: Option<String> }`（`:7-14`，uid 为 Portal 页携带的完整在线账号=账号+运营商后缀，行12-13）、`Error { is_request_failed: bool }`（`:15-17`）、`NotFound`（`:18`）。
- `impl PortalCheckResult` — `portal_check.rs:21-57`：方法 `online()`（`:22-27`）、`message()`（`:29-35`，Error→"检测失败"、NotFound→"未找到主适配器"）、`reachable()`（`:37-42`）、`login_available()`（`:44-49`）、`uid()`（`:51-56`）。
- `pub(super) fn check_adapter_portal(adapter: &Adapter, app_handle: &AppHandle) -> PortalCheckResult` — `portal_check.rs:59-93`：调 `check_portal_full`，`error_kind=="request_failed"` 返回 `Error{is_request_failed:true}` 并 emit 登录日志（`:65-72`）；异常返回 `Error{is_request_failed:false}`（`:83-91`）。

### 自动登录与断线重连（`monitor/auto_auth.rs`）

- 常量 `RECONNECT_REMINDER_INTERVAL: u32 = 10`（`auto_auth.rs:13`）、`PREP_LOGIN_MAX_FAILURES: u32 = 5`（`:21`）、`PREP_LOGIN_BACKOFF_CAP_SECS: u64 = 300`（`:27`）。
- `pub fn reconnect_attempt_allowed(reconnect_count: u32, max_reconnect: u32) -> bool` — `auto_auth.rs:32-35`：`reconnect_count <= max_reconnect || (reconnect_count - max_reconnect) % RECONNECT_REMINDER_INTERVAL == 0`（超限后降频重试而非终态放弃）。
- `pub fn prep_login_backoff_secs(failures: u32, base_cooldown_secs: u64) -> u64` — `auto_auth.rs:40-50`：未达上限返 base；达上限按超出次数指数翻倍（excess 上限 11），封顶 `PREP_LOGIN_BACKOFF_CAP_SECS` 且不低于 base。
- `pub fn prep_login_cooldown_ready(failures: u32, elapsed_secs: u64, base_cooldown_secs: u64) -> bool` — `auto_auth.rs:53-55`：`elapsed_secs >= prep_login_backoff_secs(...)`。
- `pub fn is_time_window_refusal(msg: &str) -> bool` — `auto_auth.rs:62-73`：识别中文关键词「时间段/时段/时间范围/服务时间/时间未到/不在允许」（`:67`）及英文 time range（`:72`）。
- `pub fn prep_failures_after_failure(failures: u32, failure_msg: &str) -> u32` — `auto_auth.rs:78-84`：时间窗拒绝不累加（保持基础冷却轻探测），其余失败累加。
- `pub fn reconnect_should_report(login_success: bool, within_limit: bool) -> bool` — `auto_auth.rs:90-92`：返回 `login_success && within_limit`。
- `pub fn try_auto_login_on_preparation(app_handle, state, login_available: bool, online: bool, config: &Config)` — `auto_auth.rs:94-203`：先同步 `login_available` 沿检测，上升沿（门户重新可达）复位 `prep_login_failures`（`:104-117`）；门控 `!login_available || online || !auto_login_on_preparation`（`:119-121`）；`has_logged_online` 跳过（`:123-127`）；连续失败达上限进入退避日志（`:129-138`）；注销保护期跳过（`:140-144`）；冷却按失败计数退避检查（`:146-157`）；`is_logging_in.try_acquire()` 后调 `auth::service::full_login`（`:160-163`）；成功置 `has_logged_online=true` 且失败计数清零（`:179-186`）；失败按 `prep_failures_after_failure` 累加（时间窗拒绝豁免，`:187-199`）。
- `pub fn try_disconnect_reconnect(app_handle, state, online: bool, secondary_online: Option<bool>, a1: Option<&Adapter>, adapter1_name: &str, adapter2_name: &str, reachable: bool, login_available: bool, config: &Config) -> bool` — `auto_auth.rs:218-327`：`any_offline` 判定（行218）；`any_adapter_online`/注销保护/冷却门控（`:221-235`）；先抢登录锁再自增 `disconnect_reconnect_count`（`:238-250`）；`reconnect_attempt_allowed` 判定（`:253`）；上限内发断线通知、`full_login`、成功则清零计数/置在线/记重连历史（`:256-292`）；超限释放锁并按提醒节律通知（`:293-307`）；返回 `reconnect_should_report`（`:314`）。
- `pub fn run_auto_login_on_start(app_handle: &AppHandle)` — `auto_auth.rs:317-574`：`auto_login_on_start` 门控（`:321-323`）；`--autostart` 决定延迟 5000/1500ms（`:325-326`）；任务内先查 `is_quitting`/`has_logged_online`（`:337-339`）；`spawn_blocking` 强制枚举适配器（`:342-353`）；自启时适配器未就绪重试 3 次（`:363-378`）；校园网静默期连自动登录一并跳过（`:384-400`）、检测未通过触发 `start_campus_exit`（`:401-435`）；双适配器并行 Portal 检测（`:458-471`）；已在线跳过登录（`:486-511`）；`spawn_blocking` 内抢锁执行 `full_login`（`:524-537`）；成功置登录标记、可通知、`auto_exit_after_login` 时 `start_auto_exit`（`:555-569`）。
- 测试模块（`auto_auth.rs:581-672`）：`reconnect_success_within_limit_reports_true`（`:580-584`）、`time_window_refusal_classification`（`:586-599`）、`time_window_refusal_does_not_grow_backoff`（`:601-609`）、`reconnect_failure_never_reports_true`（`:611-615`）、`reconnect_success_over_limit_reports_false`（`:617-621`）、`reconnect_allowed_within_limit`（`:623-628`）、`reconnect_allowed_periodically_after_limit`（`:630-637`）、`prep_login_backoff_below_limit_uses_base_cooldown`（`:639-644`）、`prep_login_backoff_grows_exponentially_after_limit`（`:646-652`）、`prep_login_backoff_capped`（`:654-660`）、`prep_login_ready_after_backoff_elapsed_even_at_limit`（`:662-671`）。

### 延迟与质量调度（`monitor/latency.rs`、`monitor/quality_scheduler.rs`）

- 常量 `pub(super) BAD_LEVELS: &[&str] = &["poor", "bad"]`（`latency.rs:12`）、`GOOD_LEVELS: &[&str] = &["excellent", "great", "good"]`（`:13`）。
- `pub(super) fn classify_quality_change(last: Option<&str>, current: &str, enable_notification: bool) -> Option<&'static str>` — `latency.rs:18-38`：恶化到 poor/bad 返 `Some("bad")`、从 poor/bad 恢复返 `Some("good")`，同档/未知/中间档互转/通知关闭均返 `None`。
- `pub(super) fn record_last_quality(state: &AppState, current: &str)` — `latency.rs:40-42`。
- `pub(super) fn notify_quality_change(app_handle, kind: &str)` — `latency.rs:44-52`：bad→「网络拥堵」、good→「网络恢复」通知 + 登录日志。
- `pub(super) async fn wait_for_adapter_ready(app_handle, cancel_token) -> Option<(String, String)>` — `latency.rs:67-85`：每 2s 短重试等适配器解析出 IP 且 `any_adapter_online`（Portal 未认证时外网 HTTPS 必被拦截），取消返 `None`。
- `pub fn spawn_latency_test_loop(app_handle: &AppHandle) -> Result<(), String>` — `latency.rs:78-139`：spawn `latency_test`；间隔每轮重读配置与轻量化系数（轻量化下限 1800s，`effective_quality_interval_ms`，`:91-98`），计时器按需重建、`MissedTickBehavior::Delay` 错过不补发、重建首 tick 吞掉（`:86-107`）；`tokio::select!` 支持取消（`:108-114`）；`is_running("latency_test")`/`is_quitting` 退出（`:115-119`）；就绪等待复用 `wait_for_adapter_ready`（`:121-127`）；检测前等 1s（`:128-132`）；调 `run_quality_check` 传循环取消令牌（`:135`）。
- `pub(super) async fn run_startup_quality_once(app_handle: AppHandle, cancel_token: Arc<CancellationToken>)` — `latency.rs:144-160`：启动即测一次（v2.4.2），就绪等待与 1s 稳定后调 `run_quality_check`（`:159`），任务即退出。
- 测试（`latency.rs:167-199`）：`degraded_to_bad_level_triggers_pending_bad`（`:166-170`）、`recovery_from_bad_level_triggers_good`（`:172-176`）、`same_level_or_unknown_last_never_notifies`（`:178-183`）、`middle_levels_crossing_never_notifies`（`:185-192`）、`notification_disabled_never_classifies`（`:194-198`）。
- 常量 `SPIKE_CONFIRM_COUNT: usize = 2`（`quality_scheduler.rs:12`）、`SPIKE_CONFIRM_INTERVAL_SECS: u64 = 15`（`:13`）。
- `async fn perform_quality_check(app_handle, adapter_name: &str, adapter_ip: &str, lightweight: bool) -> Option<serde_json::Value>` — `quality_scheduler.rs:23-49`：读 `skip_ttfb_in_latency`/`skip_content_in_latency`/`fixed_gateway`（`:21-24`）；`is_quality_checking.try_acquire()` 互斥（`:25-28`）；`check_network_quality_async`（lightweight 复核轮只测网关+1 个外网站点，`:29`）；`append_quality_history` 落盘（`:37-40`）；`emit_network_quality_result` 推前端（`:41-43`）；信号量被占用/序列化失败返 `None`。
- `pub(super) async fn run_quality_check(app_handle, adapter_name: &str, adapter_ip: &str, cancel: Option<&CancellationToken>) -> Option<serde_json::Value>` — `quality_scheduler.rs:56-122`：全量质量检测统一入口（周期驱动者只剩定时测试循环，手动命令不经过本函数）；取消即返 `None`（`:59-61`）；首轮全量检测（`:62-64`）；`classify_quality_change` 为 `Some("bad")` 时复核确认：15s 间隔再测 2 次（轻量化），复核等待可被取消，任一次未达标即放弃，全确认才 `notify_quality_change("bad")`（`:68-113`，复核轮轻量化在行89）；其余 kind 直接通知并记录（`:114-119`）；返回首轮结果（`:121`）。

### 校园网检测与结果发射（`monitor/campus_check.rs`、`monitor/background_emit.rs`）

- `pub struct ConnectionCampusStatus` — `campus_check.rs:8-12`（derive `Debug, Clone, Serialize` 行6、`serde(rename_all="camelCase")` 行7）：字段 `on_campus: bool`（行9）、`name: Option<String>`（行10）、`message: String`（行11）。
- `pub struct AdapterCampusStatus` — `campus_check.rs:350-357`（derive 行22-23）：字段 `name: String`（行25）、`wireless: bool`（行26）、`on_campus: bool`（行27）、`matched_by: Option<String>`（命中判定腿 `name`/`subnet`/`gateway`/`portal`，全部失败为 None，行29）、`message: String`（行30）。2026-10-26 改造：替代原 `wifi`/`wired` 整类聚合的错误归因。
- `pub struct CampusCheckResult` — `campus_check.rs:35-43`（derive 行33-34）：字段 `wifi: Option<ConnectionCampusStatus>`（行36）、`wired: Option<ConnectionCampusStatus>`（行37）、`on_campus: bool`（行38）、`current_ssid: Option<String>`（行39）、`message: String`（行40）、`adapters: Vec<AdapterCampusStatus>`（逐卡明细，行42）。
- `pub(super) fn adapter_campus_status<'a>(adapter_name, adapters, campus_result) -> Option<&'a AdapterCampusStatus>` — `campus_check.rs:45-49`：以适配器列表为权威，卡已不存在时返 None。
- `pub(super) fn adapter_campus_message(...)` — `campus_check.rs:51-53`。
- `fn campus_name_list(value: &str) -> Vec<&str>` — `campus_check.rs:57-63`：支持顿号/全半角逗号/分号/空白分隔。
- `struct CampusContext<'a>` — `campus_check.rs:66-76`：字段 `enable_name_check: bool`、`name_list: Vec<&'a str>`、`campus_gw: &'a str`、`portal_host: String`、`wifi_ssid: Option<String>`、`wired_profile: Option<String>`。
- `impl CampusContext::name_hit(&self, adapter: &Adapter) -> Option<String>` — `campus_check.rs:81-98`：名称腿按卡类型套用（SSID 只对无线卡、有线 profile 只对有线卡），关闭名称检查时整体不参与。
- `fn judge_adapter<F: FnMut(&str, &str) -> bool>(adapter, ctx, probe) -> AdapterCampusStatus` — `campus_check.rs:106-162`：逐卡按成本短路判定——无 IP 直接判非校园且不探测（`:120-122`）→ 名称（`:125-127`）→ 网段 /18 纯位运算（`:130-136`）→ 网关绑源 ICMP（`:139-145`）→ Portal 绑源（`:148-154`），全失败判非校园（`:157-161`）。
- `fn aggregate_class(cards, name) -> Option<ConnectionCampusStatus>` — `campus_check.rs:166-173`（按卡类型聚合兼容字段，无卡返 None）；`fn join_messages(cards)` — `:176-186`（命中卡文案优先）。
- `pub fn check_campus_network(config: &Config, adapters: &[Adapter]) -> CampusCheckResult` — `campus_check.rs:350-435`：名称检查禁用时不付 netsh 调用成本（`:195-202`）；`(target, source)` 维度探测缓存避免重复绑源探测（`:216-234`）；逐卡 `judge_adapter`（`:236-239`）；聚合 `wifi`/`wired` 兼容字段（`:241-246`）；汇总 `on_campus` 与 `message`（`:248-259`）。
- `pub use crate::config::schedule::is_campus_check_silent;` — `campus_check.rs:285`（静默期判定下沉双端共享面，crate 内转出）。
- 测试（`campus_check.rs:287-434`）：`名称名单_解析多分隔符与空项`（`:316-325`）、`无ip的卡_直接判非校园且不探测`（`:327-339`）、`名称命中_按卡类型套用且短路不探测`（`:341-358`）、`网段命中_零探测短路`（`:360-373`）、`路由器私网_网关腿绑源命中`（`:375-388`）、`网关失败但portal命中_判校园`（`:390-397`）、`网关与portal都失败_判非校园`（`:399-407`）、`名称检查禁用时_名称腿不参与`（`:409-417`）、`聚合_命中卡文案优先_无卡返回none`（`:419-431`）。
- `pub(super) struct BackgroundCheckResult<'a>` — `background_emit.rs:14-34`：字段 `online: bool`（行15）、`reachable: bool`（行16）、`login_available: bool`（行17）、`message: &'a str`（行18）、`adapter1_name: &'a str`（行19）、`adapter2_name: &'a str`（行20）、`secondary_online: Option<bool>`（行21）、`secondary_message: &'a str`（行22）、`online_operator: Option<String>`（主适配器在线时从 Portal uid 推导的运营商后缀，行24）、`secondary_online_operator: Option<String>`（行26）、`dual_adapter: bool`（行27）、`config: &'a Config`（行28）、`campus_result: &'a CampusCheckResult`（行29）、`a1_campus_msg: Option<&'a str>`（行30）、`a2_campus_msg: Option<&'a str>`（行31）、`a1_on_campus: Option<bool>`（行32）、`a2_on_campus: Option<bool>`（行33）。
- `pub fn adapter_status_entry(name, ip, wireless, online, message) -> serde_json::Value` — `background_emit.rs:36-41`；`pub fn adapter_disabled_entry(name)` — `:43-45`；`pub fn adapter_disconnected_entry(name, wireless)` — `:47-49`。
- `pub(super) fn build_adapter_details(...)` — `background_emit.rs:51-65`。
- `pub(super) fn handle_status_change(...)` — `background_emit.rs:68-107`（`#[allow(clippy::too_many_arguments)]` 行67）：状态变更记日志；离线且开启通知时 60s 去抖弹系统通知（`:92-103`）。
- `pub(super) fn emit_background_check_result(app_handle, state, result: &BackgroundCheckResult)` — `background_emit.rs:109-167`：`update_with_result` CAS 内自增 `background_check_count`（BE-A-10 修复，`:116-119`）；`is_running("background_check")`（行120）；注销保护期内强制 `online=false`/`secondaryOnline=Some(false)`/运营商字段 `null`（`:127-134,146-147`）；emit 前端事件含 `campusAdapters` 逐卡明细（`:136-164`）。
- `pub(super) fn update_network_state(state, online, secondary_online, reachable, app_handle)` — `background_emit.rs:169-205`：更新 `server_available`（行176）；注销保护期跳过（`:183-186`）；更新 `any_adapter_online`/`last_a1_online`、在线时清零重连计数（`:188-194`）；门户可达且主卡在线但未登录过时置 `has_logged_online=true` 并按 `auto_exit_on_online` 触发 `start_auto_exit`（`:196-204`）。

### 定时动作与夜间出站编排（`monitor/scheduled.rs`、`monitor/outbound_switch.rs`、`monitor/outbound_night.rs`）

**scheduled.rs**（每日定时登录/注销 P2-32 + 夜间出站切换编排 Task 5，模块文档 `:1-22`）：

- 常量 `SCHEDULED_TICK_MS: u64 = 30_000`（`scheduled.rs:46`）、`NIGHT_VERIFY_WAIT_SECS: u64 = 15`（`:50`）、`NIGHT_VERIFY_RETRY_SECS: u64 = 5`（`:51`）、`NIGHT_VERIFY_ATTEMPTS: u32 = 3`（`:52`）、`NIGHT_VERIFY_HTTP_TIMEOUT_SECS: u64 = 15`（`:53`）、`SCHEDULED_LOGIN_MAX_RETRIES: u32 = 20`（`:368`）。
- `pub async fn run_scheduled_action_loop(app_handle, cancel_token)` — `scheduled.rs:196-360`：启动对账 `reconcile_outbound_on_startup`（送阻塞线程池，`:62-63`）与 `reconcile_scheduled_logout_on_startup`（`:65`）；30s 拍循环 + `tokio::select!` 取消（`:66-74`）；`is_quitting` 退出（`:79-81`）；拍前取 `outbound_switch_active` 门控 `evaluate_scheduled`（`:92-96`），命中调 `run_scheduled_login`/`run_scheduled_logout`（`:97-102`）；出站编排 `evaluate_outbound_action_with_restore`（`:116-125`）匹配 `NightOutboundAction`：`Switch` → `run_outbound_blocking(apply_outbound_switch)`（`:126-133`）、`Restore` → `apply_outbound_restore`（`:134-141`）、`None` → T-1 分钟预判定 `evaluate_outbound_prep_with_restore` + `prepare_outbound_switch`（`:142-162`）、`needs_replay` 补齐重放（`:163-179`）、待切换残留清理 `drive_pending_switch`（`:180-185`）、夜间看门狗 `watchdog_re_disable_campus`（`:186-191`）；拍后取 `outbound_active`（行196）门控 `evaluate_night_switch_gated_with_restore`（`:200-215`，含可配置 `night_restore_minutes`）得 `night_action` 调 `apply_night_switch_action`（`:216-218`）。
- `fn is_quitting(app_handle) -> bool` — `scheduled.rs:224-226`（抽离避免循环体跨 await 持 State 守卫）。
- `fn reconcile_scheduled_logout_on_startup(app_handle)` — `scheduled.rs:236-251`：冷启动若当日注销时刻已过，只消耗额度不执行（防「刚打开应用就被踢下线」）。
- `fn evaluate_scheduled(state, now_minutes, today_day, outbound_active) -> (bool, bool)` — `scheduled.rs:260-271`：装配输入调共享纯函数 `evaluate_scheduled_actions`（双端单点）。
- `fn mark_scheduled_day_fired(state, fire_login, fire_logout, today_day)` — `scheduled.rs:278-285`（`#[cfg_attr(not(test), allow(dead_code))]` 行277；生产路径在 `spawn_login_action`/`perform_logout` 闭包内置位）。
- `fn format_minutes(minutes: u16) -> String` — `scheduled.rs:287-289`。
- `fn run_scheduled_login(app_handle, target_minutes, today_day)` — `scheduled.rs:294-301`：调 `spawn_login_action` 传 `Some(today_day)`。
- `fn spawn_login_action(app_handle, trigger_log, result_prefix, login_quota_day: Option<i32>)` — `scheduled.rs:307-363`：抢 `tasks.is_logging_in` 锁，抢不到发 `scheduled-action-skipped` 事件且不耗额度（`:313-322`）；抢到后置当日额度标记（审计 A7，`:328-330`）；调 `auth::service::full_login`（行331）与 `post_login_handler`（行341）；失败补试：`bump_login_retry` 未达 `SCHEDULED_LOGIN_MAX_RETRIES` 时回退额度标记下一拍重试，用尽则保留当日标记（`:342-361`）。
- `fn apply_night_switch_action(app_handle, action: NightSwitchAction)` — `scheduled.rs:388-415`：`SwitchToCampus` 将原 operator 存 `night_operator_restore` 并置空串（=无锡学院），`Restore` 反向（`:376-387`）；`save_config_and_broadcast` 落盘失败不 store 内存下拍重试（`:388-392`）；成功后 `spawn_login_action`（None 额度，`:395`）并 spawn `verify_night_switch`（`:399`）。
- `fn run_scheduled_logout(app_handle, target_minutes, today_day)` — `scheduled.rs:407-411`。
- `fn perform_logout(app_handle, result_prefix, logout_quota_day: Option<i32>)` — `scheduled.rs:443-477`：抢 `tasks.is_logging_out` 锁（`:421-428`）；调 `auth::service::full_logout`（行433）；成功后取消自动退出、重置 `failure_tracker`、重置登录/重连状态并启动 60s 注销保护期（`:440-451`）。
- `async fn verify_night_switch(app_handle, expected_operator: String)` — `scheduled.rs:487-524`：等 15s → `night_switch_verify_round` 核对 → 未生效则注销重登一轮再复验 → 最终不符发告警日志 + `emit_notification`（`:484-494`）。
- `fn night_verify_config(app_handle, expected_operator) -> Option<Config>` — `scheduled.rs:499-503`（operator 已不等于期望值时返 None 跳过验证）。
- `async fn night_switch_verify_round(config, expected_operator) -> Result<String, String>` — `scheduled.rs:509-527`：`create_safe_http_client` 做 3 轮重试、间隔 5s。
- `async fn night_switch_check_once(client, portal_url, user, expected_operator) -> Result<String, String>` — `scheduled.rs:528-552`：构造 `{origin}/drcom/chkstatus?callback=dr1003`，`parse_chkstatus` + `uid_matches` 核对。
- `pub(crate) fn portal_origin(portal_url: &str) -> String` — `scheduled.rs:558-568`（空值回退 `default_portal_url`）；`fn expected_operator_display(expected_operator: &str) -> &str` — `:569-575`（空串显「无锡学院」）。
- 测试（`scheduled.rs:577-698`）：`禁用目标_不触发_不置标记`（`:584-592`）、`到点触发_置当日标记`（`:594-614`）、`跨天重置_次日再触发`（`:616-624`）、`切换态_跳过定时登录且不消耗当日标记_注销照常`（`:626-653`）、`format_minutes_补零`（`:655-659`）、`定时登录补试计数_累加清零跨天归零`（`:663-676`）、`定时登录失败回退额度_补试用尽后当日停止`（`:681-697`）。

**outbound_switch.rs**（夜间出站切换的桌面侧动作层：目标卡选择、逐卡校园网判定、快照序列化，模块文档 `:1-9`）：

- `pub fn is_campus_adapter(ip, gateway, campus_gateway, portal_host, gateway_probe: impl Fn(&str,&str)->bool) -> bool` — `outbound_switch.rs:17-39`：IP 空判否（`:24-26`）；同 /18 网段判真（`:27-29`）；否则绑源探校园网关两发去抖或绑源探 portal 主机（`portal_probe_host` 保证仅私网 IPv4，`:35-38`）。
- `pub fn portal_probe_host(portal_url: &str) -> String` — `outbound_switch.rs:44-56`：从 portal_url 提取可作绑源探测目标的私网 IPv4 主机；域名/公网/IPv6/空值返空串。
- `pub fn effective_outbound_priority(priority: &[String], details: &[(Adapter, String)]) -> Vec<String>` — `outbound_switch.rs:63-82`：用户已排序按原序透传；未排序默认「无线卡优先、有线卡随后」组内保发现序。
- `pub fn select_outbound_candidate(priority, details, campus_gateway, portal_host, gateway_probe) -> Option<Adapter>` — `outbound_switch.rs:87-111`：跳过无 IP/无网关/校园网卡/自身网关不可达卡，按优先级选首张可用卡。
- `struct SnapshotRow` — `outbound_switch.rs:115-121`：字段 `guid: String`（行117）、`family: u16`（行118）、`automatic: bool`（行119）、`metric: u32`（行120）。
- `pub fn snapshot_json(guid: &str, rows: &[MetricRow]) -> String` — `outbound_switch.rs:125-136`：序列化跃点快照（每行补 guid），兜底 `"[]"`。
- `pub struct DisabledRow` — `outbound_switch.rs:142-145`：字段 `pub guid: String`（行143）、`pub name: String`（行144）——名单只记本功能亲手禁用的卡，绝不按状态推断。
- `pub fn disabled_adapters_json(rows: &[DisabledRow]) -> String` — `outbound_switch.rs:148-150`；`pub fn parse_disabled_adapters(json: &str) -> Result<Vec<DisabledRow>, String>` — `:156-161`（空串→空名单；非法 JSON→Err，调用方须区分处理）。
- `pub struct StandbyRoute` — `outbound_switch.rs:169-176`：字段 `pub dest: String`（行170）、`pub mask: String`（行171）、`pub gateway: String`（行172）、`pub metric: u32`（行173）、`pub if_index: u32`（serde rename `"ifIndex"`，行175）——历史兜底默认路由快照（2026-10-18 事故后不再创建，仅保留解析能力）。
- `pub fn parse_standby_route(json: &str) -> Option<StandbyRoute>` — `outbound_switch.rs:179-181`。
- `pub enum StandbyCleanup` — `outbound_switch.rs:186-196`：变体 `AlreadyGone`（行189）、`ForeignRowKept`（行191）、`LastDefaultPath`（行193）、`Safe`（行195）。
- `pub fn standby_cleanup_decision(same_key_metrics: &[u32], standby_metric: u32, rows_outside_exact: usize) -> StandbyCleanup` — `outbound_switch.rs:203-220`：按跃点列表判定「哪一行是我们的」（route.exe 与 DeleteIpForwardEntry2 的匹配键都不含跃点，正是误删根源）。
- `pub fn standby_route_cleanup(route: &StandbyRoute) -> Result<StandbyCleanup, String>` — `outbound_switch.rs:224-264`（`#[cfg(target_os = "windows")]` 行223，读实时路由表判定）；非 Windows 版 `:267-270` 恒返回 `AlreadyGone`。
- `pub(crate) fn unsafe_to_disable(guid: &str) -> bool` — `outbound_switch.rs:278-280`（cfg windows）：实例 ID 都读不到的卡保守跳过；USB 总线网卡不再排除（用户确认夜间禁用副适配器）。
- `pub fn select_campus_to_disable(priority, details, campus_gateway, portal_host, exclude_guid, gateway_probe, bus_guard) -> Vec<DisabledRow>` — `outbound_switch.rs:285-318`：过滤目标卡/无 IP/非校园/guard 命中，同名去重。
- 测试模块（`outbound_switch.rs:346-691`）：`campus_adapter_by_subnet`（`:345-349`）、`campus_adapter_off_subnet_probe_reachable`（`:351-355`）、`candidate_skips_campus_and_ipless_and_out_of_list`（`:374-389`）、`effective_priority_empty_defaults_wireless_first`（`:427-439`）、`candidate_skips_gatewayless_and_dead_gateway_cards`（`:461-478`）、`campus_probe_single_loss_still_counts_as_campus`（`:480-489`）、`snapshot_json_round_trip_with_guid`（`:491-504`）、`disabled_adapters_json_round_trip`（`:512-524`）、`standby_route_snapshot_parses_legacy_config_json`（`:526-544`）、`select_campus_to_disable_filters_everything_non_campus`（`:546-574`）、`campus_adapter_portal_probe_reachable`（`:610-622`）、`portal_probe_host_extracts_private_ipv4`（`:662-671`）、`standby_cleanup_never_deletes_foreign_or_last_default_route`（`:673-690`）等。

**outbound_night.rs**（夜间出站实现：提权执行、退避、预判定计划，模块文档 `:1`）：

- 常量：`OUTBOUND_HELPER_TIMEOUT_SECS: u64 = 30`（`:20`）、`OUTBOUND_BACKOFF_START_MS: u64 = 60_000`（`:24`）、`OUTBOUND_BACKOFF_MAX_MS: u64 = 300_000`（`:25`）、`OUTBOUND_RESTORE_ALERT_FAILS: u32 = 3`（`:30`）、`OUTBOUND_RESTORE_ALERT_REMIND_EVERY: u32 = 20`（`:32`）、`OUTBOUND_RESTORE_GIVE_UP_FAILS: u32 = 40`（`:36`）、`pub(super) OUTBOUND_PROBE_HOST: &str = "223.5.5.5"`（`:399`）、`OUTBOUND_PLAN_MAX_AGE_MS: u64 = 180_000`（`:471`）。
- `fn outbound_restore_alert_due(count: u32) -> bool` — `:39-42`；`fn notify_outbound_issue_once_per_day(app_handle, last_day: &AtomicI32, title, body)` — `:46-52`（swap 当日去重）。
- 退避记账静态原子：`OUTBOUND_SWITCH_LAST_FAIL_MS`（`:55`）、`pub(super) OUTBOUND_SWITCH_FAIL_COUNT`（`:56`）、`OUTBOUND_RESTORE_LAST_FAIL_MS`（`:58`）、`OUTBOUND_RESTORE_FAIL_COUNT`（`:59`）、`OUTBOUND_CHANNEL_FAIL_COUNT`（`:63`）；五枚每日提醒去重标记（`:67-72`）。
- `static OUTBOUND_ACTION_BUSY: AtomicBool` — `:78`；`struct OutboundActionGuard` RAII 单飞锁 — `:81-96`（`try_acquire` `:84-90`、`Drop` 释放 `:92-96`）。
- `pub(super) async fn run_outbound_blocking(app_h, config, action: fn(&AppHandle, &Config) -> Result<(), String>)` — `:100-112`（出站动作送阻塞线程池）。
- `pub(super) fn outbound_switch_active(app_handle) -> bool` — `:116-123`；`pub(super) fn in_outbound_restore_window(config, now_minutes) -> bool` — `:128-131`（恢复窗口 `[起点, 23:00)`，起点 `nightRestoreMinutes` 可配置、默认 07:30）。
- `fn now_epoch_ms` — `:133-138`；`fn outbound_backoff_ms(fail_count: u32) -> u64` — `:141-148`（60s 起步、每失败翻倍、封顶 300s）；`fn backoff_gate(last_fail_ms, fail_count, now_ms) -> bool` — `:152-157`（时钟回拨按压制处理）；`fn switch_gate_open` — `:159-165`；`fn restore_gate_open` — `:167-173`；`fn mark_outbound_failure` — `:176-179`；`fn clear_outbound_failure` — `:182-185`；`fn outbound_allow_uac` — `:188-190`；`fn note_channel_failure` — `:193-197`；`fn clear_outbound_channel_failure` — `:200-202`。
- `pub(super) fn needs_replay(metric_snapshot, disabled_adapters, standby_route, fail_count: u32) -> bool` — `:207-209`（切换态 ∧ 有失败历史才补齐，稳态不重放）。
- `struct OutboundSnapshotRow` — `:213-219`：字段 `guid: String`（行215）、`family: u16`（行216）、`automatic: bool`（行217）、`metric: u32`（行218）；`fn parse_outbound_snapshot(snapshot: &str) -> Result<Vec<OutboundSnapshotRow>, String>` — `:222-227`。
- `struct HelperFailure` — `:230-237`：字段 `reason: String`（行232）、`channel: bool`（是否提权/执行通道失败，行236）；`fn run_helper_op(op: &str, args: &[String], allow_uac_prompt: bool) -> Result<(), HelperFailure>` — `:243-269`（调提权 helper，超时 30s）。
- `fn enable_campus_adapter(name: &str, allow_uac_prompt: bool)` — `#[cfg(desktop)]` 版 `:280-291`（走 `adapter_cache::enable_adapter`，含 problem 22 分流 pnputil 设备级启用）；非 desktop stub `:295-300` 恒 Err。
- `fn write_outbound_metric(app_handle, name, guid, families, notify_success, notify_suffix)` — `:308-369`（写 Metric=1，幂等，成功清退避、失败记退避）。
- `fn probe_target_internet(target: &Adapter) -> bool` — `:414-419`：绑源 ICMP 打 `223.5.5.5`，**只作诊断信号不阻断切换**（2026-10-26 取代旧登录闸——校园网 23:00 断网时要求热点登录校园 Portal 的条件恒假）。
- `fn persist_config_update` — `:425-431`；`fn set_pending_switch` — `:436-444`；`pub(super) fn drive_pending_switch(app_handle, config)` — `:453-467`（本版本夜间切换不再产生挂起，仅清理历史版本 `waiting-login` 残留行）。
- 预判定计划：`struct OutboundSwitchPlan` — `:480-495`（字段 `target_name: String` 行482、`target_guid: String` 行483、`rows: Vec<MetricRow>` 行485、`campus_to_disable: Vec<DisabledRow>` 行487、`empty_disable_warn: bool` 行489、`probe_ok: bool` 行491、`prepared_ms: u64` 行493、`day: i32` 行494）；`static OUTBOUND_PREPARED_PLAN: Mutex<Option<OutboundSwitchPlan>>` — `:500-501`（进程内内存、不落盘）；`fn take_prepared_plan` — `:505-518`（跨日/超 3 分钟即丢弃，一次性消费）；`fn plan_target_still_valid(plan)` — `:523-533`；`fn build_outbound_plan(app_handle, config, notify)` — `:541-672`（**纯只读**：选卡绑源探测、读跃点、算禁用名单、外网探测、空禁用名单告警判定）；`pub(super) fn prepare_outbound_switch` — `:682-699`（T-1 分钟拍入口，失败不覆盖已有计划）。
- `pub(super) fn apply_outbound_switch(app_handle, config)` — `:701-870`：单飞闸 + 退避闸（`:702-709`）→ 消费预判定计划或现场判定（`:711-731`）→ 跃点现值复核（`:732-756`）→ 诊断提醒（`:757-776`）→ 快照先落盘（切换态成立唯一凭据，`:777-800`）→ helper 写 Metric=1 + 禁用校园网卡（`:801-829`）→ 路由级验证（GetBestRoute 到校园网关的最优路由若仍指向校园卡判失败，`:830-864`）。
- `pub(super) fn replay_outbound_switch(app_handle, config)` — `:887-975`：补齐/重放，分支①跃点补写 + 分支②禁用补做（原③兜底路由补加已删除），全幂等，无新增失败时清切换失败计数。
- `pub(super) fn watchdog_re_disable_campus(app_handle, config)` — `:984-1051`：切换态 ∧ 守护窗口内复核禁用名单，名单内校园网卡被外部重新启用时重新禁用（P2-3）。
- `pub(super) fn apply_outbound_restore(app_handle, config)` — `:1065-1378`：反向顺序还原——① 清理历史兜底路由（四元组精确匹配、绝不误删用户路由、唯一默认路径保留，`:1105-1205`）→ ② 跃点写回（只写「快照有原值+现值不一致」的协议栈，目标卡/跃点行消失视为已还原，`:1206-1266`）→ ③ 重新启用名单内校园网卡（名单快照损坏走告警+保留状态，放弃阀 40 次后按空名单放行收尾，`:1267-1344`）；收尾通知区分放弃阀放行与真实还原（`:1345-1372`）；`finish_outbound_restore` 清快照（`:1373-1377`）。
- `fn finish_outbound_restore(app_handle) -> bool` — `:1385-1406`：先清切换态快照（落盘成功才清退避），再清「立即切换」hold（`:1393`）；**还原收尾钩子**：`request_check_after_outbound_restore()`（行1403）+ `spawn_background_check_after_outbound_restore`（行1404），所有还原路径（自动/手动/启动对账）都汇到此处。
- `fn clear_outbound_snapshot(app_handle) -> bool` — `:1410-1430`；`fn set_outbound_manual_hold` — `:1436-1444`；`fn clear_outbound_manual_hold` — `:1447-1449`；`pub(crate) fn manual_outbound_switch` — `:1455-1476`（用户显式触发，清退避闸，已处于切换态时拒绝）；`pub(crate) fn manual_outbound_restore` — `:1480-1493`；`pub(super) fn reconcile_outbound_on_startup(app_handle)` — `:1500-1558`（启动对账：恢复窗口内立即还原；夜间窗口内目标卡仍可用且判非校园才重放切换）。
- 测试（`:1560-1670`）：`还原失败告警_首达阈值触发_此后周期重复`（`:1564-1577`）、`重放闸_仅切换态且留有失败历史时才补齐`（`:1579-1590`）、`退避窗_起步六十秒_每失败翻倍封顶三百秒`（`:1592-1600`）、`退避闸_无失败历史恒放行`（`:1602-1608`）、`退避闸_首败六十秒内压制_六十秒后放行`（`:1610-1615`）、`退避闸_第二败退避翻倍`（`:1617-1622`）、`退避闸_封顶后三百秒内压制_三百秒后放行`（`:1624-1631`）、`退避闸_时钟回拨按压制处理`（`:1633-1636`）、`快照解析_空串为空表_非法json报错`（`:1638-1649`）、`单飞闸_同一时刻只放一个出站动作_释放后可再取`（`:1651-1666`）、`SNAPSHOT_ONE_ROW` 常量（行1669）。

### 适配器监控（`monitor/adapter_watch.rs`）

- 常量 `ADAPTER_WATCH_INTERVAL: u64 = 15000`（`adapter_watch.rs:9`）、`CLASS_SUBKEY_REFRESH_ROUNDS: u32 = 4`（`:15`，60s 刷新 class subkey 缓存）。
- `pub fn start_adapter_watch(app_handle: &AppHandle) -> Result<(), String>` — `adapter_watch.rs:17-288`：spawn `adapter_watch` 任务，15s 循环（`MissedTickBehavior::Delay`，`:28-32`）；首轮无条件推送一次（启动竞态，行26）；每 4 轮（60s）刷新 class subkey 缓存（包 spawn_blocking，`:56-62`）；读 `get_all_adapters_cached` 缓存（`:64-68`）；适配器集合/IP 变化与禁用状态变化按 name 排序后比较（`:71-89`），变更 emit `adapters_changed`/`adapter_details_changed` 事件（`:91-100`）；适配器从禁用恢复且当前不在线时经 `spawn_background_check_once` 触发一次性巡检（不重开用户已停止的巡检，`:102-117`）；用户配置的卡被新禁用时发警告（60s 去抖，夜间出站切换亲手禁用的卡不发，`:118-163`）；自动启用块（`:166-272`）：夜间出站切换守护窗（时间窗 ∧ 切换态）内一律跳过（`:183-203`）、切换态亲手禁用名单拦截（快照损坏按全部拦截保守处理，`:204-215`）、按 `auto_enable_backoff_ms` 退避执行，提权首试静默、失败后允许降级弹 UAC（`:216-271`），启用成功 5s 后触发一次性巡检（`:251-255`）。
- `fn auto_enable_backoff_ms(failure_count: u32) -> u64` — `adapter_watch.rs:292-299`：0→0、1→60s、2→120s、≥3→300s 封顶。
- 测试：`auto_enable_backoff_ladder`（`adapter_watch.rs:306-314`）。

## Architecture

```text
┌──────────────────────────┐
│ watcher.rs:16            │ run_startup_tasks（spawn 5 任务）
└──────────┬───────────────┘
           ▼
┌──────────────────────────────┐ ┌─────────────────────────────┐
│ background_task.rs:7         │ │ latency.rs:78               │
│ start_background_check_inner │ │ spawn_latency_test_loop     │
│ （dynamic_interval 动态间隔） │ │ → quality_scheduler.rs:56   │
│ → background_check.rs:50     │ │ run_quality_check           │
└──────────┬───────────────────┘ └─────────────────────────────┘
           ▼
┌────────────────────┐ ┌──────────────────┐ ┌────────────────────┐
│ portal_check.rs:59 │ │ auto_auth.rs:94  │ │ scheduled.rs:56    │
│ check_adapter_     │ │ try_auto_login_  │ │ run_scheduled_     │
│ portal             │ │ on_preparation   │ │ action_loop        │
└────────────────────┘ └──────────────────┘ └─────────┬──────────┘
                                                      ▼
                                            ┌───────────────────────┐
                                            │ outbound_night.rs:701 │
                                            │ apply_outbound_switch │
                                            │ /apply_outbound_      │
                                            │ restore（动作层        │
                                            │ outbound_switch.rs）  │
                                            └───────────────────────┘
```

## Data Flow

### 启动挂载
- `watcher.rs:16` `run_startup_tasks` 依据配置 spawn：`startup_bg_check`（`watcher.rs:16-26` → `background_task.rs:7`）、`startup_quality_once`（`watcher.rs:16-23` → `latency.rs:144`）、`startup_latency`（`watcher.rs:16-26` → `latency.rs:144`）、`startup_auto_login`（`watcher.rs:16-21` → `auto_auth.rs:317`）、`scheduled_actions`（`watcher.rs:16-23` → `scheduled.rs:56`）。

### 单轮巡检主链
- `background_task.rs:30-57` 循环调用 `background_check::run_background_check`（`background_check.rs:546` → `:50`），`DynamicInterval::next_ms` 决定下一拍间隔（`background_task.rs:41`）。
- `background_check.rs:54` `is_checking.try_acquire()` 互斥（`:54-56`）；`:63-73` 适配器 cached/force 降级；`:84-91` `outbound_restore_active` 切换态跳过；`:92-96` 静默期判定；`:119-127` 静默期跳过并强制 `on_campus=true`；`:128-133` 还原后 `forced` 放行；`:212-224` 双适配器并行 `portal_check::check_adapter_portal`（`:59`）；`:248-290` 失败计数（MAC 重置窗口 `:252`）；`:390-396` `should_reset_logged_online` 复位登录标记；`:436` `auto_auth::try_auto_login_on_preparation`；`:438-442` `auto_auth::try_disconnect_reconnect`；`:446-448` 未重连时 `update_network_state`（经 `background_emit.rs:169`）；`:450-463` 结局分类与快排查联动质量检测。

### 质量链
- `latency.rs:78` `spawn_latency_test_loop` 就绪等待（`wait_for_adapter_ready` `:58-76`）后调 `quality_scheduler.rs:56` `run_quality_check`；`:68-113` bad 档复核 2×15s 轻量化确认；`perform_quality_check`（`quality_scheduler.rs:19`）调 `check_network_quality_async`，落盘 `append_quality_history`（`:38`）并 emit（`:41`）。

### 定时动作循环
- `scheduled.rs:56` 30s 循环：`:93-102` 定时登录/注销；`:111-193` 夜间出站编排调 `super::outbound_night` 动作（`apply_outbound_switch` `:701`、`apply_outbound_restore` `:1065`、`prepare_outbound_switch` `:682`、`replay_outbound_switch` `:887`、`watchdog_re_disable_campus` `:984`；判定与选择在 `outbound_switch.rs`）；`:196-218` 运营商夜切 `apply_night_switch_action`（`:373`）改配置并 `verify_night_switch`（`:458`）。

### 夜间出站预判定与还原补检
- 预判定：T-1 分钟那一拍 `evaluate_outbound_prep_with_restore` 命中后 `prepare_outbound_switch`（`outbound_night.rs:682`）纯只读构建计划（`build_outbound_plan` `:541`）存内存槽位（`:500-501`）；切点拍 `apply_outbound_switch`（`:701`）经 `take_prepared_plan`（`:505`）+ `plan_target_still_valid`（`:523`）消费或现场重建。
- 还原补检：`background_task.rs:149` `spawn_background_check_after_outbound_restore` 阻塞轮询等 IP 就绪（最长 45s）后 `spawn_background_check_once`（`:181`）；该钩子由 `outbound_night.rs:1385` `finish_outbound_restore` 在还原收尾统一调用（先 `request_check_after_outbound_restore` 置 `RESTORE_SILENT_PASS` 于 `background_check.rs:46-48`，行1403；再 spawn 补检，行1404），所有还原路径（自动/手动/启动对账）都汇到此处。

## Connections

- [[desktop-auth]]：`auto_auth.rs:163`（`try_auto_login_on_preparation` 内 `full_login`）、`auto_auth.rs:271`（断线重连）、`auto_auth.rs:536`（开机自启）、`scheduled.rs:331`（定时登录）与 `scheduled.rs:433`（`full_logout`）；`portal_check.rs:63` 调 `auth::portal::check_portal_full`；`background_check.rs:260,268` 调 `handle_portal_request_failure`（阈值见 `failure_tracker.rs::PORTAL_REQUEST_FAILURE_THRESHOLD`，`background_check.rs:243`）。
- [[desktop-network-quality]]：`quality_scheduler.rs:29` 调 `check_network_quality_async`，`:41` 发 `emit_network_quality_result`；`background_check.rs:504` 快排查联动调 `run_quality_check`。
- [[desktop-config]]：依赖 `config::persist::save_config_and_broadcast`（`background_task.rs:63`、`scheduled.rs:388`、`outbound_night.rs:427`）；共享纯函数 `evaluate_scheduled_actions`/`evaluate_outbound_action_with_restore`/`evaluate_night_switch_gated_with_restore`/`is_campus_check_silent`（`scheduled.rs:95,116,205`，`campus_check.rs:285`）；`night_switch`/`outbound_switch`/`scheduled_tick` 共享判定（`scheduled.rs:29-34`）。
- [[desktop-infra]]：`infra::events::EventBus`（登录日志/结果事件，如 `background_emit.rs:136`、`scheduled.rs:56`）与 `infra::notification::emit_notification`（`scheduled.rs:56`、`outbound_night.rs` 多处）；`CancellationToken` 跨任务传递（`background_task.rs:16`、`scheduled.rs:56`）。
- [[night-outbound-switch]]：`scheduled.rs:37-42` 引入 `outbound_night` 的 `apply_outbound_switch`/`apply_outbound_restore`/`prepare_outbound_switch`/`replay_outbound_switch`/`watchdog_re_disable_campus` 等；`outbound_switch.rs` 提供纯函数判定层；`background_check.rs:84-91` 切换态跳过巡检、`adapter_watch.rs:183-215` 守护窗拦自动启用。
- [[desktop-network-core]]：`background_check.rs:64-66` 调 `get_adapters_cached`/`get_adapters_force`；`campus_check.rs:226` 调 `check_gateway_reachable_from`（绑源探测）；`outbound_switch.rs:11` 用 `crate::network::discovery::Adapter`；`adapter_watch.rs:68` 调 `get_all_adapters_cached`。

## Known Issues

基于观察，无未解决缺陷报告；以下为已处理的边界条件与非显然行为（均已在代码中实现对应逻辑）：

1. **并发互斥**：`background_check.rs:54` `is_checking.try_acquire()` 安全跳过叠加轮次；`quality_scheduler.rs:25` `is_quality_checking`；`auto_auth.rs:160,238` `is_logging_in` 锁；`scheduled.rs:313,421` 定时动作抢锁失败发 `scheduled-action-skipped` 事件且不耗当日额度（审计 A7）。
2. **适配器获取降级**：cached 空降级 force（`background_check.rs:63-73`）；夜间补检特意用 force 避 TTL 旧缓存（`background_task.rs:123-126` 注释）。
3. **出站切换态跳过巡检**：`outbound_restore_active` 三份快照任一非空即整轮跳过（`background_check.rs:84-91`），避免 portal 检测走热点出站必失败而累计触发 MAC 重置与整夜告警。
4. **校园网静默期语义**：静默期整体跳过本轮巡检（Portal 检测/自动登录/断连重连均不执行，`background_check.rs:119-127`，2026-10-25 修复整夜空转认证）；还原后**在本轮静默期剩余时段持续放行**（`RESTORE_SILENT_PASS`，`background_check.rs:98-102,128-133`，离开静默期即清除，2026-10-26）；开机自启同口径跳过（`auto_auth.rs:388-400`）。
5. **登录标记复位（会话失效判定）**：`should_reset_logged_online`（`background_check.rs:25-27`，调用 `:390-396`）修复整夜休眠唤醒后自动登录被 `has_logged_online` 永久门控；校园网检测失败也随离线重置（`:143-150`）。
6. **自动登录防无限重试**：`PREP_LOGIN_MAX_FAILURES=5`（`auto_auth.rs:206`）；达上限指数退避封顶 300s（`auto_auth.rs:206-216`），退避期满必须放行重试（`:662-671` 测试）；时间窗拒绝不计入退避（`auto_auth.rs:206-212`）；`try_disconnect_reconnect`（`:206-315`）超限后每 10 拍放行一次（`RECONNECT_REMINDER_INTERVAL`，`:32-35`）。
7. **重连状态覆盖修复**：`try_disconnect_reconnect` 返回真实 `reconnect_should_report`（`auto_auth.rs:90-92,314`），调用方 `background_check.rs:446-448` 依此跳过用重连前旧 Portal 快照覆盖 `any_adapter_online`。
8. **Portal 请求失败容错**：`Error{is_request_failed:true}` 累计阈值（`background_check.rs:248-290`），成功按适配器独立重置（`:295-317`）；MAC 重置检测仅每日 08:30 后启用（`dynamic_interval.rs:20,23-25`，`background_check.rs:252`），窗口未开清零残留计数防夜间失败在 8:31 首拍触发（`:273-289`）。
9. **夜间出站还原补检缺口**：`spawn_background_check_after_outbound_restore`（`background_task.rs:149-183`）最长等 45s 轮询 IP 后发起一次性巡检；还原收尾 `finish_outbound_restore`（`outbound_night.rs:1385-1406`）先置静默期放行标记再调补检，修复「轻量化下限 300s 导致到点不自动登录」（`:100-113` 实证注释）。
10. **延迟测试动态间隔与启动首测**：每轮重读配置与轻量化系数（下限 1800s），计时器按需重建、`Delay` 错过不补发、首 tick 吞掉（`latency.rs:86-107`）；启动即测一次不再以定时开关为前提（`watcher.rs:36-43`、`latency.rs:144-160`）。
11. **质量抖动复核**：poor/bad 需 15s×2 次轻量化复核全达标才通知（`quality_scheduler.rs:68-113`），复核轮被互斥抢占则不通知不落状态（`:99-104`）；中间档互转不通知（`latency.rs:18-38`）。
12. **就绪等待**：`wait_for_adapter_ready` 每 2s 等 IP 与 `any_adapter_online`（Portal 未认证时外网 HTTPS 全超时会误报拥堵，`latency.rs:58-76`）。
13. **任务注册失败处理**：watcher 各 spawn 失败仅 `log_warn`（`watcher.rs:29,41,53,61,70`）；`spawn_background_check_once` 复用注册令牌补跑（`background_task.rs:86-97`）。
14. **双适配器并行实现**：`run_background_check_blocking` 在 spawn_blocking 线程内用 `Handle::current().block_on` + `spawn_blocking` + `tokio::join!`（`background_check.rs:212-224`）。
15. **出站切换与运营商夜切互斥**：`scheduled.rs:92` 拍前出站态门控定时登录（跳过不耗额度，还原后下一拍过点补触发，`:626-653` 测试）；`:196` 拍后切换态门控运营商夜切；出站动作先行（spec §2）。
16. **历史兜底路由清理**：`outbound_switch.rs:168-220` 不再创建 StandbyRoute，仅解析历史快照并按跃点列表安全决策（2026-10-18 事故后）；还原时四元组精确删除、唯一默认路径保留（`outbound_night.rs:1105-1146`）。
17. **适配器监控与自动启用**：`adapter_watch.rs:17-288` 检测禁用并自动启用（首试静默、失败后允许 UAC、退避阶梯 `:292-299`）；夜间守护窗跳过（`:183-203`）；切换态亲手禁用名单拦告警与自动启用（`:146-153,204-215`）。
18. **巡检间隔三档自适应**：未登录 ×2 退避封顶 5 分钟、已登录 10 分钟（尊重更大用户配置）、门户不可达 1 分钟快排查（`dynamic_interval.rs:52-73`）；`Skipped` 沿用上一拍不累计退避（`:67` 与 `:146-155` 测试）；快排查联动质量检测 5 分钟节流（`background_check.rs:478-487`）。
19. **夜间切换预判定计划**：T-1 分钟只读建计划（`outbound_night.rs:541-699`），切点消费省掉绑源探测耗时；计划不落盘、跨日/超时丢弃（`:505-518`）、执行时刻跃点现值复核（`:732-756`）；目标卡外网探测只作诊断不阻断（`:414-419`，取代恒假的登录闸）；待切换挂起机制已移除，仅清理历史残留（`:453-467`）。
20. **定时登录失败补试**：到点失败按 30s 拍有限补试 20 次（`scheduled.rs:368`，约 10 分钟窗口），失败回退当日额度标记下一拍重试（`:342-361`），补试用尽才当日放弃；注销仍单次语义。
