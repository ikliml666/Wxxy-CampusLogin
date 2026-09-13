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
  - tauri-app/src-tauri/src/monitor/auto_auth.rs
  - tauri-app/src-tauri/src/monitor/latency.rs
  - tauri-app/src-tauri/src/monitor/quality_scheduler.rs
  - tauri-app/src-tauri/src/monitor/adapter_watch.rs
tags: [monitor, background-check, auto-login, reconnect, campus-network, portal, latency, quality, adapter-watch, tauri-events]
---

## Overview

本模块是桌面端的"后台巡检中枢"：一个可配置周期（默认 15000ms）的巡检循环先做校园网环境判定（静默期内跳过），再做主/副适配器的 Portal 连通性检测，据结果驱动"准备自动登录""断线重连""注销保护""校园网退出倒计时""网络状态变更通知"等后续动作；另有两个独立循环——15000ms 的适配器监听（变更事件、被禁用手选适配器的自动启用）与用户可配置周期的网络质量定时测试（含 poor/bad 档位复核与拥堵通知）。

`monitor` 整体在 `lib.rs:17-18` 以 `#[cfg(desktop)]` 门控，安卓 target 完全不编译本模块（安卓有自己的 `android/src-tauri/src/monitor_loop.rs`）。本模块内部**没有任何 `#[cfg]` 平台门控**：`campus_check.rs` 里直接调用 `get_wireless_ssid()` / `get_wired_network_profile()` / `check_gateway_reachable()` 等，这些平台相关的实现由 `network` 层自行门控，`monitor` 只消费跨平台接口。

本模块全部 `文件:行号` 引用均相对于 `tauri-app/src-tauri/src/`。

## Key Components

### 模块装配（`monitor/mod.rs`、`monitor/watcher.rs`）

- `monitor/mod.rs:1-10` 仅做子模块声明，顺序为 `watcher`、`auto_auth`、`latency`、`adapter_watch`、`campus_check`、`portal_check`、`quality_scheduler`、`background_emit`、`background_check`、`background_task`；无任何公开项。
- `pub use super::background_check::run_background_check;` — `monitor/watcher.rs:8`，re-export 保持旧路径 `watcher::run_background_check` 可用。
- `pub use super::background_task::start_background_check_inner;` — `monitor/watcher.rs:9`。
- `pub use super::campus_check::check_campus_network;` — `monitor/watcher.rs:12`（`auto_auth.rs:302` 就是经此路径调用的）。
- `pub use super::background_emit::{adapter_status_entry, adapter_disabled_entry, adapter_disconnected_entry};` — `monitor/watcher.rs:13`。
- `pub fn run_startup_tasks(app_handle: &AppHandle) -> ()` — `monitor/watcher.rs:15-54`，启动期唯一入口，注册三个跟踪任务：

| 条件 | 任务名 | 动作 | 位置 |
|---|---|---|---|
| `config.enable_background_check` | `startup_bg_check` | `start_background_check_inner(&app_h, &s)` | `monitor/watcher.rs:20-30` |
| `config.enable_network_quality && config.enable_latency_test` | `startup_latency` | 取 `latency_test_interval`，`< 10000` 时回退 `30000`，调 `spawn_latency_test_loop` | `monitor/watcher.rs:32-46` |
| 无条件 | `startup_auto_login` | `run_auto_login_on_start(&app_h)` | `monitor/watcher.rs:48-53` |

三个 `task_manager.spawn` 注册失败都只 `log_warn!`（`monitor/watcher.rs:28`、`:44`、`:52`），不阻断启动。调用方是 `app/startup.rs:195`。

### 巡检任务的生命周期（`monitor/background_task.rs`）

- `pub fn start_background_check_inner(app_handle: &AppHandle, state: &AppState) -> Result<CommandResult, String>` — `monitor/background_task.rs:7-60`，既是启动入口也是"重新开启"入口（`commands/background.rs:10` 的命令、`monitor/watcher.rs:24` 的启动路径都走这里）。行为：
  - `state.config.update`：置 `enable_background_check = true`；若 `background_check_interval < 10000` 则改写为 `15000`（`monitor/background_task.rs:8-13`）。
  - `task_manager.spawn("background_check", ...)`（`:16`）注册跟踪任务；内部先判 cancel（`:20-22`），然后**立即执行首轮** `run_background_check`（`:24`）。
  - 之后进入循环（`:29-49`）：每轮重新读配置 `background_check_interval.max(10000)`（`:30-34`）——间隔修改即时生效，不需要重启任务；`tokio::time::interval` 先 `tick().await` 吃掉即时首 tick（`:36`），再 `select!` 等下一 tick 或 cancel（`:37-43`）；退出条件为 `!task_manager.is_running("background_check") || exit.is_quitting`（`:44-47`）。
  - 注册成功后走 `commands::config_cmd::save_config_to_disk_encrypted` 落盘（`:55-57`），失败只告警；返回 `CommandResult::ok_msg("后台检测已启动")`（`:59`）。

### 巡检主体（`monitor/background_check.rs`）

- `pub(crate) fn run_background_check_blocking(app_handle: &AppHandle, state: &AppState, cancel_token: &CancellationToken)` — `monitor/background_check.rs:14-335`，同步巡检的全部逻辑。
- `pub async fn run_background_check(app_handle: &AppHandle, cancel_token: Arc<CancellationToken>)` — `monitor/background_check.rs:340-348`，薄封装：`tauri::async_runtime::spawn_blocking` 内取 `AppState` 后调 `run_background_check_blocking`；`spawn_blocking` 的 JoinError 只 `log_error!("background", "后台检测异常: {}", e)`（`:345-347`）。

`run_background_check_blocking` 的完整步骤（按执行顺序）：

| 步骤 | 位置 | 说明 |
|---|---|---|
| 中止检查 | `:15-17` | `exit.is_quitting` 或 token 已取消则直接 return |
| 并发互斥 | `:18-20` | `state.tasks.is_checking.try_acquire()`，抢不到**静默 return**（无日志） |
| 读配置 | `:23` | `state.config.load_full()` |
| 取适配器 | `:28-37` | 先 `get_adapters_cached()`，空或 Err 时 `get_adapters_force()`；两者都失败 → `log_error!` + return |
| 解析主/副 | `:41-43` | `resolve_adapter_names` → `find_dual_adapters` |
| 当前分钟数 | `:45` | `Local::now().hour()*60 + Local::now().minute()` |
| 静默期分支 | `:46-75` | 见下文"校园网检测" |
| 写校园网状态 | `:79-91` | 写 `current_ssid`/`on_campus_network`；`campus_check_failed` 时额外把 `any_adapter_online=false`、`last_a1_online=false`、`has_logged_online=false` |
| 校园网失败路径 | `:93-133` | emit（`online/reachable/login_available` 全 false）→ 若主副适配器都无 IP 则跳过退出（`:124-126`），否则 `start_campus_exit`（`:129`）→ return |
| 取消检查 | `:138-140` | 校园网通过后再次检查 token |
| Portal 检测 | `:143-176` | 双适配器且都解析出时：`Handle::current().block_on` + 两个 `spawn_blocking(check_adapter_portal)` + `tokio::join!`（`:149-161`）；否则串行检测主、副 |
| request_failed 处理 | `:181-206` | 逐适配器调 `handle_portal_request_failure`（阈值见 `auth/failure_tracker.rs:194`） |
| 失败计数重置 | `:211-233` | 任一适配器本次 `Success` 即重置该适配器的 `a1/a2_auth_failure_count` |
| 汇总事件字段 | `:235-275` | `online/reachable/login_available/message`；副适配器 online 与 message 的三分支推导（`:258-273`）；写 `last_a2_online`（`:275`） |
| 状态变更通知 | `:282-288` | `any_online = online \|\| secondary_online == Some(true)`，传入 `handle_status_change` |
| 发射结果 | `:290-318` | 组装 `campus` 明细后 `emit_background_check_result` |
| 自动登录 | `:320` | `try_auto_login_on_preparation(app_handle, state, login_available, online, &config)` |
| 断线重连 | `:322-332` | `try_disconnect_reconnect`；返回 false 时才 `update_network_state` |

文件末尾 `:337-339` 的文档注释明确边界：**后台巡检只负责连通性/Portal/重连，质量检测由 `latency.rs` 的定时测试循环独占**（2026-09-04 收敛）。

### 结果发射、状态更新与通知（`monitor/background_emit.rs`）

- `pub fn adapter_status_entry(name: &str, ip: &str, wireless: bool, online: bool, message: &str) -> serde_json::Value` — `:32-37`，构造 `{name, ip, wireless, online, message}`。
- `pub fn adapter_disabled_entry(name: &str) -> serde_json::Value` — `:39-41`，固定 `("", false, false, "适配器已禁用或未找到")`。
- `pub fn adapter_disconnected_entry(name: &str, wireless: bool) -> serde_json::Value` — `:43-45`，固定 `("", wireless, false, "适配器未连接")`。
- `pub(super) fn build_adapter_details(...) -> String` — `:47-61`，拼接 `"名: 消息"`，副适配器仅在其被 `is_secondary_adapter_enabled` 判定启用时纳入（`:56`）。
- `pub(super) fn handle_status_change(prev_online, current_online, reachable, login_available, adapter1_name, adapter1_message, adapter2_name, adapter2_message: Option<&str>, config, app_handle)` — `:64-103`，10 个参数。状态翻转时 `log_info!`；由在线转离线且 `enable_notification` 时，受 **60000ms** 节流（`:95`）后调 `emit_notification(app_handle, "网络状态变更", &adapter_details, "mascot-portrait")`（`:97`）；未翻转时只 `log_debug!`。
- `pub(super) fn emit_background_check_result(app_handle: &AppHandle, state: &AppState, result: &BackgroundCheckResult)` — `:105-157`，核心发射器：
  - `background_check_count` 自增用 `update_with_result` 单次 CAS 完成并取新值（`:112-115`）；
  - 注销保护判定（`:123-130`）：`Instant::now() < logout_protected_until` 为真时把 `effective_online` 与 `effective_secondary_online` 强制为 `false`；
  - 最终 payload 见 Data Flow 的字段表。
- `pub(super) fn update_network_state(state, online, secondary_online, reachable, app_handle)` — `:159-195`：先写 `server_available = reachable`（`:166`）；`any_online = online || secondary_online == Some(true)`（`:168`）；注销保护期内**整体跳过**状态更新（`:170-176`）；否则写 `any_adapter_online`、`last_a1_online`，`any_online` 时清零 `disconnect_reconnect_count`（`:178-184`）；`reachable && !has_logged_online && online` 时置 `has_logged_online=true`、`prep_login_failures=0`，并在 `config.auto_exit_on_online` 为真时 `start_auto_exit`（`:186-194`）。

### 校园网检测（`monitor/campus_check.rs`）

- `pub(super) fn adapter_campus_status<'a>(adapter_name, adapters, campus_result) -> Option<&'a ConnectionCampusStatus>` — `:22-27`，按适配器名字找 `Adapter`，用 `wireless` 决定取 `campus_result.wifi` 还是 `.wired`。
- `pub(super) fn adapter_campus_message(adapter_name, adapters, campus_result) -> Option<String>` — `:29-31`，取该状态对象的 `message` 克隆。
- `pub fn check_campus_network(config: &Config, adapters: &[Adapter]) -> CampusCheckResult` — `:33-238`，判定入口。
  - `!enable_network_name_check` 分支（`:37-52`）：只看 `check_gateway_reachable(&config.campus_gateway)`，`on_campus = gateway_ok`，`wifi`/`wired`/`current_ssid` 全 `None`，message 为 `"网关{ip}可达"` 或 `"未连接到校园网络(网关不可达)"`。
  - 名称检查开启时：先取 `get_wireless_ssid()` 与 `get_wired_network_profile()`（`:57-58`），并定义带缓存的网关探测闭包 `check_gateway`（`:63-73`，单次调用内只探一次网关）。
  - WiFi 判定（`:75-152`）三种情况：SSID 命中 `required_network_name`（忽略大小写，`:78`）→ 直接 on_campus；SSID 不匹配但某 WiFi 网卡 IP 与网关同 `/18` 网段（`is_same_subnet_18`，`:92`）或网关可达（`:102-107`）→ 仍判 on_campus（message 记录降级理由）；都不成立 → `on_campus=false`。无 SSID 但有 WiFi 网卡时同样走"子网 → 网关"降级（`:118-150`），且**只有至少一个 WiFi 网卡有非空 IP 才信任网关可达性**（`:137`）。
  - 有线判定（`:154-203`）：`wired_profile` 命中名称 → on_campus；否则子网/网关降级；同样要求本类型网卡至少一个非空 IP 才信任网关（`:184`）。
  - 汇总（`:205-226`）：`on_campus = wifi.on_campus || wired.on_campus`；message 用 `；` 拼接所有"与 on_campus 一致"的分项消息；`current_ssid = wifi_ssid.or(wired_profile)`。
- `pub fn is_campus_check_silent(now_minutes: u16, start: u16, end: u16) -> bool` — `:244-252`。`start == 0` → `false`（禁用门控）；`now < start` → `true`；`end > start && now >= end` → `true`。即 **`end <= start` 时退化为"仅开始时间单边门控"**。

静态行为参考（current_ssid 写回位置）：`monitor/background_check.rs:80`（巡检）、`monitor/auto_auth.rs:287-336`（开机自启路径）。

### Portal 检测（`monitor/portal_check.rs`）

- `pub(super) enum PortalCheckResult` — `:6-17`，三变体 `Success { online, message, reachable, login_available }`、`Error { is_request_failed }`、`NotFound`。
- `impl PortalCheckResult` — `:19-48`，四个只读投影：`online()`（`:20-25`）、`message()`（`:27-33`，`Error` → `"检测失败"`、`NotFound` → `"未找到主适配器"`）、`reachable()`（`:35-40`）、`login_available()`（`:42-47`）；非 `Success` 变体的三个布尔投影一律 `false`。
- `pub(super) fn check_adapter_portal(adapter: &Adapter, app_handle: &AppHandle) -> PortalCheckResult` — `:50-83`，调 `auth::portal::check_portal_full(&adapter.ip, Some(&adapter.name))`；`ps.error_kind == Some("request_failed")` 时 `log_warn!` + 发一条 `error` 级 `emit_login_log` 并返回 `Error { is_request_failed: true }`（`:56-63`）；其他 `Ok` → `Success`（`:65-70`）；`Err` → `Error { is_request_failed: false }` 并同样发日志（`:73-81`）。

### 自动登录与断线重连（`monitor/auto_auth.rs`）

- `pub fn reconnect_should_report(login_success: bool, within_limit: bool) -> bool` — `:24-26`，返回 `login_success && within_limit`，纯函数（3 个单测在 `:474-490`）。
- `pub fn try_auto_login_on_preparation(app_handle, state, login_available: bool, online: bool, config: &Config)` — `:28-106`。门槛：`login_available && !online && config.auto_login_on_preparation`（`:35-37`）；`has_logged_online` 为真直接返回（`:41-43`）；`prep_login_failures >= PREP_LOGIN_MAX_FAILURES`（5）时 `log_warn!` 并停止本会话自动登录（`:47-54`）；注销保护期内跳过（`:56-60`）；距上次尝试不足 `config.auto_login_cooldown_secs`（默认 60 秒）跳过（`:62-66`）。抢到 `is_logging_in` 后记录时间戳、调 `auth::service::full_login`（`:72`），发 `emit_auto_login_result`（`:80-86`）；成功置 `has_logged_online=true`、清零失败计数、按 `enable_notification` 发"自动登录成功"通知（`:88-95`）；失败 `saturating_add(1)` 并记录 `[f/5]`（`:96-102`）。
- `pub fn try_disconnect_reconnect(app_handle, state, online: bool, secondary_online: Option<bool>, a1: Option<&Adapter>, adapter1_name: &str, adapter2_name: &str, reachable: bool, login_available: bool, config: &Config) -> bool` — `:109-217`，10 参数。判定链：
  - `any_offline = (!online && a1.is_some()) || secondary_online == Some(false)`（`:121`）；
  - 前置门 `!any_adapter_online || !any_offline || !reachable || !login_available || !config.auto_login_on_preparation` → 返回 false（`:125-127`）；
  - 注销保护（`:129-133`）、冷却（`:135-138`）→ false；
  - 抢登录锁失败 → false（`:141-147`）；
  - 抢到后 `update_with_result` 单次 CAS 自增并取 `reconnect_count`（`:151-154`），`within_limit = reconnect_count <= config.max_disconnect_reconnect`（`:155`，默认 3）；
  - 未超限：确定离线适配器名、发"检测到断线"通知 + warning 日志、`full_login`（`:159-177`）；成功则 `reconnect_success = true`，写 `disconnect_reconnect_count=0`、`any_adapter_online=true`、`has_logged_online=true`、`prep_login_failures=0`（`:181-186`），`append_login_history(..., "reconnect")`（`:187`），`emit_auto_login_result(true, ...)`（`:191`）；
  - 超限：`drop(login_guard)` 释放锁（`:197`）；恰好等于 `max+1` 时发"断线重连失败"通知与 error 日志（`:198-201`）；更大时按 `RECONNECT_REMINDER_INTERVAL`（10）取模，每 10 次重复提醒一次（`:202-208`）；
  - 返回 `reconnect_should_report(reconnect_success, within_limit)`（`:216`）。
- `pub fn run_auto_login_on_start(app_handle: &AppHandle)` — `:219-468`，开机/启动自启登录。`config.auto_login_on_start` 为假直接 return（`:223-225`）；`is_auto_start = args.any(|a| a == "--autostart")`（`:227`），初始延迟 **5000ms**（自启）/ **1500ms**（手动启动）（`:228`）；任务名 `auto_login_on_start`（`:235`）。后续流程：等延迟 → 判 `is_quitting`/`has_logged_online`（`:239-241`）→ `get_adapters_force`（失败则发事件并 return，`:244-255`）→ 自启时最多 **3 次、每次 3 秒**等待适配器出现非空 IP（`:266-280`）→ `resolve_adapter_names`（`:284`）→ 校园网校验（`:286-337`，静默期跳过并直接置 `on_campus_network=true`）→ `find_dual_adapters`（`:339`）→ 双适配器并行 `check_portal_full`（`:352-365`）→ 已在线则写状态并 `emit_auto_login_result(true, msg, true)` 后 return（`:380-405`）→ 未在线则 `spawn_blocking` 内再判 `has_logged_online`、抢锁、调 `full_login`（`:418-431`）→ 成功写状态、按 `enable_notification` 通知、`config.auto_exit_after_login` 时 `start_auto_exit`（`:449-463`）。

### 适配器监听（`monitor/adapter_watch.rs`）

- `const ADAPTER_WATCH_INTERVAL: u64 = 15000;` — `:8`（私有）。
- `pub fn start_adapter_watch(app_handle: &AppHandle) -> Result<(), String>` — `:10-219`，任务名 `adapter_watch`，调用方 `app/startup.rs:186`。循环体（`:22-216`）依次做：
  - `select!` 等 tick 或 cancel（`:23-28`）；`exit.is_quitting` 判退（`:30-33`）；
  - `network::dns::cleanup_expired_dns_cache()`（`:35`）；
  - `spawn_blocking(registry::refresh_class_subkey_cache())` 轻量刷新 HKLM Class 子键缓存（`:43-45`，结果被丢弃）；
  - `spawn_blocking(get_all_adapters_cached)` 取缓存快照（`:51`，历史用 `get_all_adapters_force`，改为缓存降低重复 `GetAdaptersAddresses`）；
  - 变更检测：适配器按 `name` 排序后比 `name`/`ip`（`:54-61`）；禁用列表同样排序后比 `name`/`status`（`:63-72`，注释说明 `GetAdaptersAddresses` 返回顺序不稳定）；
  - `adapters_changed` → `emit_adapters_changed`，`details` 非空时另发 `emit_adapter_details_changed`（`:74-83`）；
  - `disabled_changed` → `emit_disabled_adapters_changed`（`:85-88`）；若存在"上次禁用、本次消失"的适配器（`:89-91`）且当前 `!any_adapter_online`，则以 `task_manager.cancel_token("background_check")`（缺省新建 token）触发**一次性** `run_background_check`（`:92-107`，历史缺陷注释说明原先会误开用户已关闭的巡检）；
  - 禁用通知节流：`last_disabled_notification_ms`，首次 `0` 直接放行，否则要求间隔 **60000ms**（`:109-118`），放行时只对本轮"新增禁用且属于配置中手选适配器"的项发 `emit_adapter_disabled_warning`（`:133-140`）；
  - 自动启用块（`:144-204`）：取 `configured_disabled_adapters(&c, &disabled)`；`targets` 为空则清零 `auto_enable_failure_count`（`:152-157`）；否则按 `auto_enable_backoff_ms` 退避判断（`:163-164`），到点先写 `auto_enable_last_attempt_ms` 再对每个目标 `spawn_blocking(enable_adapter(&name, false))`（`:167-172`，`false` = 不弹 UAC）；成功清零失败计数并延时 5 秒触发一次性巡检（`:174-188`）；失败 `fetch_add(1)` 并 `log_warn!` 下一次退避秒数（`:189-198`）；
  - 查询失败时记警告（`:208-215`，区分 `Ok(Err)` 与 `JoinError`）。
- `fn auto_enable_backoff_ms(failure_count: u32) -> u64` — `:223-230`（私有）：`0 → 0`、`1 → 60_000`、`2 → 120_000`、`>=3 → 300_000`，单测在 `:232-245`。

### 网络质量定时测试（`monitor/latency.rs`、`monitor/quality_scheduler.rs`）

- `pub(super) const BAD_LEVELS: &[&str] = &["poor", "bad"];` — `monitor/latency.rs:10`（`quality_scheduler.rs:82` 复核时复用同一常量）。
- `const GOOD_LEVELS: &[&str] = &["excellent", "great", "good"];` — `monitor/latency.rs:11`（私有）。
- `pub(super) fn classify_quality_change(last: Option<&str>, current: &str, enable_notification: bool) -> Option<&'static str>` — `monitor/latency.rs:16-36`，纯判断：通知关闭或 `last` 为 `None` 或档位未变 → `None`；`is_bad && !was_bad` → `Some("bad")`；`is_good && !was_good && was_bad` → `Some("good")`；其余（含 `fair`/`unknown` 中间档互转）→ `None`。5 个单测在 `:104-141`。
- `pub(super) fn record_last_quality(state: &AppState, current: &str)` — `monitor/latency.rs:38-40`，写 `network.last_network_quality`。
- `pub(super) fn notify_quality_change(app_handle: &AppHandle, kind: &str)` — `monitor/latency.rs:42-50`，`"bad"` → 通知"网络拥堵"/"校园网延迟升高，网络可能拥堵"`mascot-busy` + warning 日志；否则 → "网络恢复"/"校园网延迟已恢复正常"`mascot-celebrate` + info 日志。
- `pub fn spawn_latency_test_loop(app_handle: &AppHandle, interval: u64) -> Result<(), String>` — `monitor/latency.rs:52-102`，任务名 `latency_test`。`tokio::time::interval` + `MissedTickBehavior::Delay`（`:59`，避免耗时超周期时连发补 tick）；首轮不等待（`first_run` 标志，`:60-68`）；每轮检查 `!is_running("latency_test") || is_quitting` 退出（`:70-73`）；**就绪等待内循环**（`:79-90`）：每 2 秒重试 `select_adapter`，直到 `ip` 非空且 `any_adapter_online` 为真（cancel 即 return），期间不消耗周期 tick；随后固定等 1 秒（`:92-95`，规避网络未稳定时 HTTPS 延迟异常）；最后 `run_quality_check(&app_h, &adapter_name, &adapter_ip, Some(&cancel_token))`（`:98`）。
- `const SPIKE_CONFIRM_COUNT: usize = 2;` — `monitor/quality_scheduler.rs:12`；`const SPIKE_CONFIRM_INTERVAL_SECS: u64 = 15;` — `:13`。
- `async fn perform_quality_check(app_handle, adapter_name, adapter_ip) -> Option<serde_json::Value>` — `monitor/quality_scheduler.rs:17-39`（私有）：读 `skip_ttfb_in_latency`/`skip_content_in_latency`/`fixed_gateway`，抢 `is_quality_checking` 信号量（抢不到返回 `None`），调 `check_network_quality_async(adapter_name, adapter_ip, skip_ttfb, skip_content, &fixed_gateway, exit.is_quitting, Some(app_handle))`，序列化失败返回 `None`，随后 `emit_network_quality_result`。
- `pub(super) async fn run_quality_check(app_handle, adapter_name, adapter_ip, cancel: Option<&CancellationToken>)` — `monitor/quality_scheduler.rs:47-111`，模块唯一对外入口（`latency.rs:98` 调用）。逻辑：取消预检（`:50-52`）→ 首测（`:53-55`）→ 取 `quality` 字段（缺省 `"unknown"`）与 `last_network_quality` → `classify_quality_change`；命中 `"bad"` 时进入复核：最多 `SPIKE_CONFIRM_COUNT`（2）次、每次先等 `SPIKE_CONFIRM_INTERVAL_SECS`（15 秒，可被 cancel 打断，`:66-75`），再 `perform_quality_check`；任一非 bad 或复核失败（`None`）即 `confirmed = false` 并 break（`:84-95`）；`confirmed` 才 `notify_quality_change(app_handle, "bad")`（`:97-99`），最后用 `record` 更新档位（`:100-102`）。非 bad 分支：`Some(k)` 时直接通知，并记录当前档位（`:104-109`）。

### `#[cfg(test)]` 测试辅助项

- `monitor/campus_check.rs:254-282`：`mod tests`，3 个用例覆盖 `is_campus_check_silent` 的单边/双边/退化语义。
- `monitor/latency.rs:104-141`：5 个用例覆盖 `classify_quality_change` 的恶化、恢复、同档、中间档、通知关闭五种路径。
- `monitor/adapter_watch.rs:232-245`：`auto_enable_backoff_ladder` 覆盖退避阶梯与 `u32::MAX` 封顶。
- `monitor/auto_auth.rs:470-491`：3 个用例覆盖 `reconnect_should_report` 的成功/失败/超限组合。

## 结构体与字段

### `ConnectionCampusStatus`（`monitor/campus_check.rs:4-10`）

`#[derive(Debug, Clone, Serialize)]` + `#[serde(rename_all = "camelCase")]`。

| 字段 | 类型 | 含义 |
|---|---|---|
| `on_campus` | `bool` | 该网络类型（WiFi 或有线）是否判定为校园网 |
| `name` | `Option<String>` | 命中的 SSID 或网络配置文件；未取到时为 `None` |
| `message` | `String` | 面向用户的中文说明（如 `已连接到校园WiFi(xxx)`、`当前WiFi"x"非校园网络`） |

### `CampusCheckResult`（`monitor/campus_check.rs:12-20`）

`#[derive(Debug, Clone, Serialize)]` + `camelCase`。

| 字段 | 类型 | 含义 |
|---|---|---|
| `wifi` | `Option<ConnectionCampusStatus>` | WiFi 侧判定；无 WiFi 网卡时为 `None` |
| `wired` | `Option<ConnectionCampusStatus>` | 有线侧判定；无有线网卡时为 `None` |
| `on_campus` | `bool` | `wifi.on_campus \|\| wired.on_campus` |
| `current_ssid` | `Option<String>` | `wifi_ssid.or(wired_profile)`，写回 `network.current_ssid` |
| `message` | `String` | 汇总消息，分项以 `；` 拼接 |

### `BackgroundCheckResult<'a>`（`monitor/background_emit.rs:14-30`）

`pub(super)`，非 `Serialize`，仅用于把 16 个参数收拢成一个结构体传给 `emit_background_check_result`。

| 字段 | 类型 | 含义 |
|---|---|---|
| `online` | `bool` | 主适配器是否在线（主适配器无 IP 时调用方已置 false） |
| `reachable` | `bool` | Portal 服务可达 |
| `login_available` | `bool` | 当前是否可发起登录 |
| `message` | `&'a str` | 主适配器消息（无 IP 时用校园网消息兜底） |
| `adapter1_name` | `&'a str` | 主适配器名 |
| `adapter2_name` | `&'a str` | 副适配器名 |
| `secondary_online` | `Option<bool>` | 副适配器在线态；`None` = 未知/未启用 |
| `secondary_message` | `&'a str` | 副适配器消息 |
| `dual_adapter` | `bool` | 是否双适配器模式，决定 payload 里 `adapter2Name` 是否为空串 |
| `config` | `&'a Config` | 配置快照（读 `enable_network_name_check`、`required_network_name`） |
| `campus_result` | `&'a CampusCheckResult` | 校园网判定结果（读 `wifi`/`wired`） |
| `a1_campus_msg` | `Option<&'a str>` | 主适配器侧的校园网消息 |
| `a2_campus_msg` | `Option<&'a str>` | 副适配器侧的校园网消息 |
| `a1_on_campus` | `Option<bool>` | 主适配器是否在校园网 |
| `a2_on_campus` | `Option<bool>` | 副适配器是否在校园网 |

### `PortalCheckResult`（`monitor/portal_check.rs:6-17`）

`pub(super)`，无 `derive`（因为含 `String`，仅内部消费）。

| 变体 | 字段 | 类型 | 含义 |
|---|---|---|---|
| `Success` | `online` | `bool` | Portal 判定在线 |
| | `message` | `String` | 原始检测消息 |
| | `reachable` | `bool` | Portal 服务可达 |
| | `login_available` | `bool` | 可发起登录 |
| `Error` | `is_request_failed` | `bool` | 是否为"请求失败"（计入 MAC 重置失败计数）；其他异常为 `false` |
| `NotFound` | — | — | 未解析出主适配器 |

### 本模块依赖的状态结构（定义在 `infra/`，此处列本模块实际读写的字段）

`NetworkSnapshot`（`infra/state/network.rs:7-26`，`NetworkState` 内部 `ArcSwap` 快照，`load()`/`update()`/`update_with_result()` 见 `:63-112`）：

| 字段 | 类型 | 本模块用途 |
|---|---|---|
| `server_available` | `bool` | `background_emit.rs:166` 写 |
| `any_adapter_online` | `bool` | `:168-184` 读写；`auto_auth.rs:125` 当前置门；`adapter_watch.rs:94` 判断是否触发一次性巡检 |
| `last_a1_online` | `bool` | `background_emit.rs:180` 写；`background_check.rs:84` 失败时置 false |
| `last_a2_online` | `bool` | `background_check.rs:275` 写 |
| `has_logged_online` | `bool` | `auto_auth.rs:41`/`:239`/`:422` 前置；`:89`、`:184`、`:397`、`:452` 置真；`background_check.rs:89` 校园网失败时重置 |
| `disconnect_reconnect_count` | `u32` | `auto_auth.rs:151-154` CAS 自增、`:182` 清零；`background_emit.rs:183` 在线时清零 |
| `background_check_count` | `u32` | `background_emit.rs:112-115` CAS 自增，进 payload `checkCount` |
| `last_auto_login_attempt` | `Instant` | `auto_auth.rs:62`/`:135` 读，`:71`、`:172`、`:429` 写 |
| `last_network_quality` | `Option<String>` | `latency.rs:38-40` 写，`quality_scheduler.rs:57` 读 |
| `current_ssid` | `Option<String>` | `background_check.rs:80`、`auto_auth.rs:314`/`:332` 写 |
| `on_campus_network` | `bool` | `background_check.rs:81`、`auto_auth.rs:294`/`:315`/`:333` 写 |
| `logout_protected_until` | `Instant` | `background_emit.rs:123`/`:170`、`auto_auth.rs:56`/`:129` 读，用于注销保护期门控 |
| `a1_auth_failure_count` / `a2_auth_failure_count` | `u32` | `background_check.rs:217-232` 读旧值、按适配器独立清零 |
| `prep_login_failures` | `u32` | `auto_auth.rs:47` 判上限、`:99` 累加、`:91`/`:185`/`:398`/`:453` 清零 |

`UpdateStats`（`infra/state/mod.rs:86-101`）本模块仅用两个字段：`last_network_change_notification_ms`（`background_emit.rs:95-96`，60s 节流）、`last_disabled_notification_ms` 与 `auto_enable_last_attempt_ms`/`auto_enable_failure_count`（`adapter_watch.rs:111-122`、`:154-166`、`:177`、`:191`）。

`TaskFlags`（`infra/state/mod.rs:74-80`）本模块用 `is_checking`（`background_check.rs:18`）、`is_logging_in`（`auto_auth.rs:69`、`:141`、`:425`）、`is_quality_checking`（`quality_scheduler.rs:23`）。

## Data Flow

### 启动挂载

```
app/startup.rs:186  start_adapter_watch                  → 任务 "adapter_watch"（15s 循环）
app/startup.rs:195  monitor::watcher::run_startup_tasks
  ├─ enable_background_check        → 任务 "startup_bg_check" → start_background_check_inner
  │                                                          → 任务 "background_check"（默认 15s 循环）
  ├─ enable_network_quality && enable_latency_test
  │                                 → 任务 "startup_latency" → spawn_latency_test_loop
  │                                                          → 任务 "latency_test"（默认 60s 循环）
  └─ 无条件                          → 任务 "startup_auto_login" → run_auto_login_on_start
```

### 单轮巡检主链

```
tick（background_check_interval，下限 10000ms）
  → is_checking.try_acquire（抢不到静默 return）              background_check.rs:18
  → get_adapters_cached → fallback get_adapters_force        background_check.rs:28-37
  → resolve_adapter_names / find_dual_adapters               background_check.rs:41-43
  → is_campus_check_silent(now_min, start, end)              campus_check.rs:244  ← 静默期则跳过校园网验证
  ├─ 静默期：cancel_campus_exit + on_campus=true             background_check.rs:62-70
  └─ 否则：check_campus_network(过滤后的主/副适配器)          background_check.rs:73-74
  → 写 current_ssid / on_campus_network                       background_check.rs:79-91
     └─ enable_network_name_check && !on_campus
        → any_adapter_online=false / last_a1_online=false / has_logged_online=false
        → emit（online=false, reachable=false, loginAvailable=false）
        → 主副均无 IP ? 跳过退出 : start_campus_exit           background_check.rs:123-130
  → cancel_campus_exit                                        background_check.rs:136
  → Portal 检测（双适配器并行 / 单适配器串行）                background_check.rs:143-176
     → check_adapter_portal → check_portal_full
  → request_failed ? handle_portal_request_failure（阈值 5）   background_check.rs:185-206
  → 任一 Success ? 重置对应 a1/a2_auth_failure_count           background_check.rs:215-233
  → any_online = online || secondary_online == Some(true)      background_check.rs:282
  → handle_status_change（翻转 → 60s 节流系统通知）            background_emit.rs:64
  → emit_background_check_result                               background_emit.rs:105
  → try_auto_login_on_preparation                              auto_auth.rs:28
  → try_disconnect_reconnect                                   auto_auth.rs:109
     └─ 返回 false 时才 update_network_state（写 server_available /
        any_adapter_online / last_a1_online；reachable && 首次在线 →
        has_logged_online=true + prep_login_failures=0 + auto_exit_on_online 时 start_auto_exit）
```

**注销保护期（`logout_protected_until`）的全局影响**：`emit_background_check_result` 强制 `online=false`（`background_emit.rs:123-130`）、`update_network_state` 整体跳过（`:170-176`）、两条自动登录路径都提前返回（`auto_auth.rs:56-60`、`:129-133`）。

### `emit_background_check_result` 的事件载荷（`background_emit.rs:132-154`）

| JSON 字段 | 来源 | 备注 |
|---|---|---|
| `serverAvailable` | `result.reachable` | |
| `loginAvailable` | `result.login_available` | |
| `online` | `effective_online` | 注销保护期内强制 `false` |
| `message` | `result.message` | |
| `adapter1Name` | `result.adapter1_name` | |
| `adapter2Name` | 双适配器时 `result.adapter2_name`，否则 `""` | |
| `secondaryOnline` | `effective_secondary_online` | 注销保护期内强制 `Some(false)` |
| `secondaryMessage` | `result.secondary_message` | |
| `timestamp` | `chrono::Utc::now().timestamp_millis()` | UTC 毫秒 |
| `checkCount` | `background_check_count` 自增后的新值 | |
| `isRunning` | `task_manager.is_running("background_check")` | 一次性巡检时为 `false` |
| `currentSsid` | `snap.current_ssid` | |
| `onCampusNetwork` | `snap.on_campus_network` | |
| `enableNetworkNameCheck` | 配置 | |
| `requiredNetworkName` | 配置 | |
| `campusWifi` | `campus_result.wifi` | 整个 `ConnectionCampusStatus` 序列化 |
| `campusWired` | `campus_result.wired` | 同上 |
| `a1CampusMessage` / `a2CampusMessage` | 计算结果 | |
| `a1OnCampus` / `a2OnCampus` | 计算结果 | |

### 自动登录两阶段

```
阶段1 try_auto_login_on_preparation（巡检内，登录可用且当前离线）
  login_available && !online && auto_login_on_preparation
  → !has_logged_online → prep_login_failures < 5 → 非注销保护 → 过冷却(默认 60s)
  → is_logging_in.try_acquire → full_login → emit_auto_login_result
     ├─ 成功：has_logged_online=true, prep_login_failures=0, 通知 "自动登录成功"
     └─ 失败：prep_login_failures += 1（达 5 本会话停止）

阶段2 try_disconnect_reconnect（巡检内，已在线后掉线）
  any_adapter_online && any_offline && reachable && login_available && auto_login_on_preparation
  → 非注销保护 → 过冷却 → is_logging_in.try_acquire
  → disconnect_reconnect_count CAS 自增 → reconnect_count <= max_disconnect_reconnect(默认 3)
     ├─ 是：通知"检测到断线 (n/3)" + full_login
     │        成功 → 计数清零、any_adapter_online=true、has_logged_online=true、
     │               prep_login_failures=0、append_login_history("reconnect")
     └─ 否：释放锁；== max+1 发"断线重连失败"；之后每 10 次（RECONNECT_REMINDER_INTERVAL）重复提醒
  → 返回 reconnect_should_report(success, within_limit)
```

### 网络质量链

```
任务 "latency_test"（interval = latency_test_interval，缺省 60000；watcher 传参 <10000 时用 30000）
  → MissedTickBehavior::Delay
  → 就绪内循环：每 2s 重试 select_adapter 直到 ip 非空 && any_adapter_online
  → 固定 sleep 1s
  → run_quality_check(cancel = 本循环 token)
     → perform_quality_check（is_quality_checking 互斥；emit_network_quality_result）
     → classify_quality_change(last, current, enable_notification)
        ├─ "bad" → 复核 2 次 × 15s（可取消）
        │          全部仍为 poor/bad → notify_quality_change("bad") → record_last_quality(最后一次结果)
        └─ "good" → notify_quality_change("good") → record_last_quality(当前)
```

### 适配器监听链

```
任务 "adapter_watch"（ADAPTER_WATCH_INTERVAL = 15000ms，Delay）
  → cleanup_expired_dns_cache
  → refresh_class_subkey_cache（spawn_blocking）
  → get_all_adapters_cached（spawn_blocking）
  → 按 name 排序比较 → emit_adapters_changed / emit_adapter_details_changed / emit_disabled_adapters_changed
  → 禁用恢复 && !any_adapter_online → 一次性 run_background_check（不重开巡检）
  → 新增禁用（60s 节流）且属手选适配器 → emit_adapter_disabled_warning
  → configured_disabled_adapters 非空 → 按退避 enable_adapter(name, false)
       成功 → auto_enable_failure_count=0 → 延时 5s 一次性 run_background_check
       失败 → auto_enable_failure_count += 1 → 退避 60s/120s/300s
```

### 周期与阈值常量

| 名称 | 值 | 位置 |
|---|---|---|
| `background_check_interval` 默认 | `15000` ms | `config/model.rs:180` |
| 巡检间隔下限（启动时钳制） | `< 10000` → `15000` | `monitor/background_task.rs:10-12` |
| 巡检间隔下限（每轮读取时钳制） | `.max(10000)` | `monitor/background_task.rs:33` |
| 巡检间隔校验钳制 | `clamp(10000, 3600000)` | `config/validate.rs:103` |
| `ADAPTER_WATCH_INTERVAL` | `15000` ms | `monitor/adapter_watch.rs:8` |
| `latency_test_interval` 默认 / 校验 | `60000` / `clamp(10000, 3600000)` | `config/model.rs:187`、`config/validate.rs:104` |
| 质量循环 interval 回退 | `< 10000` → `30000` ms | `monitor/watcher.rs:38` |
| `AUTO_EXIT_DELAY_MS` | `20000` ms | `infra/state/mod.rs:13` |
| `CAMPUS_MINIMIZE_DELAY_MS` / `CAMPUS_EXIT_DELAY_MS` | `30000` / `60000` ms | `infra/lifecycle.rs:12-13` |
| `PREP_LOGIN_MAX_FAILURES` | `5` | `monitor/auto_auth.rs:18` |
| `RECONNECT_REMINDER_INTERVAL` | `10` | `monitor/auto_auth.rs:13` |
| `max_disconnect_reconnect` 默认 | `3` | `config/model.rs:205` |
| `auto_login_cooldown_secs` 默认 | `60` | `config/model.rs:206` |
| 自启初始延迟（`--autostart` / 手动） | `5000` / `1500` ms | `monitor/auto_auth.rs:228` |
| 自启适配器重试 | `3` 次 × `3` s | `monitor/auto_auth.rs:266-270` |
| Portal 请求失败阈值 | `5` 次 | `auth/failure_tracker.rs:194` |
| 网络变更通知节流 | `60000` ms | `monitor/background_emit.rs:95` |
| 禁用适配器通知节流 | `60000` ms | `monitor/adapter_watch.rs:116` |
| 自动启用退避阶梯 | `0` / `60000` / `120000` / `300000` ms | `monitor/adapter_watch.rs:223-230` |
| `SPIKE_CONFIRM_COUNT` / `SPIKE_CONFIRM_INTERVAL_SECS` | `2` / `15` s | `monitor/quality_scheduler.rs:12-13` |
| 质量就绪重试间隔 / 检测前等待 | `2` s / `1` s | `monitor/latency.rs:87`、`:93` |
| campus 静默期默认窗口 | `start=460`（07:40）、`end=0`（单边） | `config/model.rs:202-203` |

## Connections

- [[desktop-auth]]：`auto_auth.rs` 调 `auth::service::full_login`、`auth::portal::check_portal_full`；`background_check.rs` 调 `auth::failure_tracker::{handle_portal_request_failure, AdapterFailureCounter}`（阈值 5、MAC 重置）。
- [[desktop-network-core]]：适配器枚举与缓存（`get_adapters_cached`/`get_adapters_force`/`get_all_adapters_cached`）、`resolve_adapter_names`/`find_dual_adapters`/`filter_operation_adapters`/`select_adapter`、`configured_disabled_adapters`/`enable_adapter`、`is_secondary_adapter_enabled`。
- [[desktop-network-dns]]：`adapter_watch.rs` 每轮调 `cleanup_expired_dns_cache`；`discovery::registry::refresh_class_subkey_cache`。
- [[desktop-network-quality]]：`quality_scheduler::perform_quality_check` 调 `check_network_quality_async`，并把结果发 `emit_network_quality_result`。
- [[desktop-config]]：本模块读取的全部配置字段（`enable_background_check`、`background_check_interval`、`enable_latency_test`、`latency_test_interval`、`enable_network_quality`、`auto_login_on_preparation`、`auto_login_on_start`、`auto_login_cooldown_secs`、`max_disconnect_reconnect`、`enable_network_name_check`、`required_network_name`、`campus_check_start_minutes`/`campus_check_end_minutes`、`enable_notification`、`auto_exit_on_online`、`auto_exit_after_login`、`skip_ttfb_in_latency`、`skip_content_in_latency`、`fixed_gateway`）。
- [[desktop-commands]]：`commands/background.rs`（启动巡检）、`commands/config_cmd.rs::save_config_to_disk_encrypted`（配置落盘并广播 `config-changed`）。
- [[desktop-app-lifecycle]]：`infra::lifecycle::{start_campus_exit, cancel_campus_exit, start_auto_exit}` 的 30s 最小化 / 60s 退出 / 20s 自动退出倒计时；`app/startup.rs` 的挂载点。
- [[desktop-infra]]：`infra::state`（`AppState`/`NetworkSnapshot`/`UpdateStats`/`TaskFlags`）、`infra::task_manager`、`infra::events::EventBus`、`infra::notification::emit_notification`、`infra::command_context::CommandContext`。
- [[desktop-platform]]：托盘与系统通知的落地实现（本模块只调 `emit_notification`）。
- [[desktop-frontend-hooks]]：前端消费 `background-check-result`、`auto-login-result`、`network-quality-result`、`adapters-changed`、`disabled-adapters-changed`、`adapter-disabled-warning`、`login-log` 等事件。
- [[android-backend]]：安卓侧等价物是 `android/src-tauri/src/monitor_loop.rs` 与 `quality_cmds.rs`，**不复用本模块**（`lib.rs:17-18` 的 `#[cfg(desktop)]` 将其排除在安卓编译外）。

## Known Issues

按严重程度排序，全部带 `文件:行号`。

1. **`monitor/background_check.rs:45` 单表达式内两次取 `Local::now()`**：`chrono::Local::now().hour() * 60 + chrono::Local::now().minute()` 跨分钟边界时会得到不一致的分钟数，影响仅限 `is_campus_check_silent` 的边界判定（错判静默/非静默一档）。
2. **`monitor/background_check.rs:149-161` 的双适配器并行分支强依赖 Tokio 上下文**：`:149` 取 `tokio::runtime::Handle::current()`、`:154` 用 `Handle::block_on` 驱动 `tauri::async_runtime::spawn_blocking`。当前唯一调用点是 `:342` 的 `spawn_blocking`，安全；若将来从纯 std 线程或非 Tokio 线程调用本函数会 panic（`Handle::current()` 在无运行时时 panic）。
3. **`monitor/background_check.rs:158-159` 吞掉 JoinError**：两个 `spawn_blocking` 的结果用 `unwrap_or(PortalCheckResult::Error { is_request_failed: false })` 兜底，被 panic 的适配器检测静默降级为"检测失败"，且**不计入** `request_failed` 计数（不会触发 MAC 重置），只留下一行 Portal 消息。
4. **`monitor/background_check.rs:18-20` 巡检互斥静默跳过**：`is_checking.try_acquire()` 抢不到时直接 `return`，**无任何日志**。用户把 `background_check_interval` 配到接近单轮耗时（Portal 超时 + 双适配器并行）时，会出现"计数不涨但没有任何诊断线索"。
5. **`monitor/watcher.rs:38` 与 `monitor/background_task.rs:11-12` 的回退值不一致**：质量循环对 `< 10000` 回退 `30000`，巡检回退 `15000`；而 `config/validate.rs:103-104` 已把两者下限统一钳为 `10000`，所以 `30000` 分支只在配置未经 validate 的路径（如直接改配置文件后未走校验）可达——同一语义两套常量。
6. **`monitor/background_emit.rs:112-116` 的 `isRunning` 语义偏差**：该字段来自 `is_running("background_check")`。当巡检只由 `adapter_watch` 触发一次性检查（`monitor/adapter_watch.rs:104-106`、`:184-187`）而用户并未开启后台巡检时，事件仍会发出但 `isRunning=false`，前端若据此显示运行态需自行区分。
7. **`monitor/latency.rs:79-90` 就绪等待是无限循环**：`any_adapter_online` 长期为 `false`（例如用户关闭自动登录且未手动登录）时，`latency_test` 任务每 2 秒空转、永不产出质量数据，也没有超时告警或状态上报。
8. **`monitor/quality_scheduler.rs:53-55` 首测被占用即整轮放弃**：`perform_quality_check` 返回 `None`（`is_quality_checking` 被手动检测占用）时直接 `return`，本轮不重试也不记录，下一轮才重新开始。
9. **`monitor/quality_scheduler.rs:64-96` 复核窗口占用调用者最长 30 秒**：`run_quality_check` 在 `latency_test` 循环内同步跑完 2×15s 复核；单轮最坏耗时 = 检测耗时 + 30s，若不小于 `interval` 就会挤掉后续轮次的节奏（`MissedTickBehavior::Delay` 保证不连发，但会导致实际周期变长）。
10. **`monitor/quality_scheduler.rs:89-95` 复核中断时不落状态**：`perform_quality_check` 返回 `None` 时 `record` 仍为 `None`，`record_last_quality` 不执行（`:100-102`），于是 `last_network_quality` 保持旧值，下一轮会再次从 `"good" → "bad"` 触发一次完整复核。
11. **`monitor/adapter_watch.rs:111-122` 禁用通知节流用 `Ordering::Relaxed`**：读写 `last_disabled_notification_ms` 均为 `Relaxed`（对比 `monitor/background_emit.rs:95-96` 的 `Acquire`/`Release`），并发下节流可能失效，表现为 60 秒内重复弹禁用警告。
12. **`monitor/adapter_watch.rs:167-201` 多目标共享同一失败计数**：`for da in targets` 为每个被禁用手选适配器各起一个 `spawn_blocking` 且互不等待，成功/失败都作用在同一个 `auto_enable_failure_count` 上（`:177`、`:191`）——双适配器同时被禁用时，退避阶梯会比单适配器场景更快爬到 300s。
13. **`monitor/adapter_watch.rs:31` 循环缺 `is_running` 检查**：与 `monitor/background_check.rs:44-47`、`monitor/latency.rs:70-73` 不同，`adapter_watch` 只检查 `exit.is_quitting`，任务停止完全依赖 cancel token；若 token 未触发而任务表已被移除，循环会继续跑。
14. **`monitor/campus_check.rs:63-73` 网关可达性在 WiFi/有线间共享缓存**：单次 `check_campus_network` 只探一次网关并复用（`gateway_checked`），代码用"对应类型网卡至少有一个非空 IP"作折衷护栏（`:137`、`:184`），但当 WiFi 与有线同时有 IP 时，网关可达性可能来自另一类网卡而被归因到当前类型。
15. **`monitor/portal_check.rs:50-83` 同步阻塞的 Portal 检测**：`check_adapter_portal` 直接调同步 `check_portal_full`，安全性依赖调用方在 `spawn_blocking` 内（`monitor/background_check.rs:155-156`）；单适配器路径 `monitor/background_check.rs:164`、`:167` 与 `monitor/auto_auth.rs:357-363` 未再加包装，一旦调用链改变（例如挪进 async 任务）会阻塞 runtime 线程。
16. **被主动跳过的逻辑清单**（易被误认为缺陷，实为设计）：
    - 校园网名称检查关闭时只做网关探测、`wifi`/`wired`/`current_ssid` 全 `None` — `monitor/campus_check.rs:37-52`；
    - 校园网检测静默期内跳过验证并强制 `on_campus=true`、同时 `cancel_campus_exit` — `monitor/background_check.rs:46-70`；
    - 校园网不通过但主副适配器均无 IP 时不退出、等待网络恢复 — `monitor/background_check.rs:124-126`、`monitor/auto_auth.rs:322-324`；
    - 后台巡检不再触发全量质量检测（2026-09-04 收敛）— `monitor/background_check.rs:337-339`；
    - "自动检测"模式（适配器名为空或哨兵值）不参与自动启用 — `monitor/adapter_watch.rs:145-151`，过滤在 `network/adapter.rs:53`；
    - 提权自动启用不弹 UAC（`enable_adapter(..., false)`）— `monitor/adapter_watch.rs:173`。
17. **`monitor/watcher.rs:28`/`:44`/`:52`、`monitor/background_task.rs:56` 的失败只告警**：任务注册或配置落盘失败不会阻断启动、也不会向 UI 反馈，用户可能"以为开了巡检但没开"。
18. **`monitor/auto_auth.rs:228`、`:266-270` 魔法数内联**：自启初始延迟 `5000`/`1500` 与重试 `3` 次 / `3` 秒都是字面量，未像 `PREP_LOGIN_MAX_FAILURES`（`:18`）那样提为命名常量。
19. **`monitor/auto_auth.rs:227` 依赖命令行字面量 `--autostart`**：延迟档位完全由参数决定，若自启注册方式变化（注册表/计划任务参数不同）会静默退化成 1500ms 长延迟外的另一种节奏，且没有断言或日志校验该参数来源。
