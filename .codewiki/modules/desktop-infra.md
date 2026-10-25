---
title: 桌面端全局基础设施（状态/日志/事件/任务/退出/通知）
type: module
source_files:
  - tauri-app/src-tauri/src/infra/mod.rs
  - tauri-app/src-tauri/src/infra/logger.rs
  - tauri-app/src-tauri/src/infra/events.rs
  - tauri-app/src-tauri/src/infra/task_manager.rs
  - tauri-app/src-tauri/src/infra/lifecycle.rs
  - tauri-app/src-tauri/src/infra/notification.rs
  - tauri-app/src-tauri/src/infra/command_context.rs
  - tauri-app/src-tauri/src/infra/async_util.rs
  - tauri-app/src-tauri/src/infra/state/mod.rs
  - tauri-app/src-tauri/src/infra/state/store.rs
  - tauri-app/src-tauri/src/infra/state/network.rs
  - tauri-app/src-tauri/src/infra/state/exit.rs
tags: [基础设施, 状态, 日志, 事件总线, 后台任务, 退出生命周期, 通知, 跨平台]
---

## Overview

本模块是双端应用共享的运行时基座：`state/` 提供配置（`ArcSwap` + CAS）、网络快照、退出状态三组原子状态与任务互斥锁；`logger.rs` 提供异步落盘的有界日志通道（审计 A17 改为有界 8192 条，发送端丢弃计数并周期汇总）；`events.rs` 是唯一的事件发射出口（17 个 `emit_xxx` 方法）；`task_manager.rs` 统一托管周期性后台任务的取消与等待；`lifecycle.rs` 实现“非校园网自动退出 / 登录后自动退出”两条倒计时状态机与统一退出流程；`notification.rs` 是系统通知的唯一出口；`command_context.rs` 提供命令层上下文与 `AppHandle` 扩展 trait；`async_util.rs` 提供同步上下文驱动 async future 的统一入口。

模块声明于 `tauri-app/src-tauri/src/infra/mod.rs:2-8`，由上级核心 crate 编入跨平台协议核心。安卓端经 cfg 属性复用 `logger`（证据：`logger.rs:421-430` 的 `#[cfg(target_os = "android")]` `get_log_dir`，及 `logger.rs:432-443` 非安卓分支），`AppState` 等状态结构体在当前源码中未见安卓端引用，但 `logger` 的日志目录策略已按平台分离。

## Key Components

### infra/mod.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `infra/mod.rs:1` | pub mod | `state` | 全局状态子模块 |
| `infra/mod.rs:2` | pub mod | `logger` | 异步日志 |
| `infra/mod.rs:3` | pub mod | `notification` | 系统通知出口 |
| `infra/mod.rs:4` | pub mod | `lifecycle` | 退出生命周期 |
| `infra/mod.rs:5` | pub mod | `events` | 事件总线 |
| `infra/mod.rs:6` | pub mod | `command_context` | 命令层上下文与 AppHandle 扩展 |
| `infra/mod.rs:7` | pub mod | `task_manager` | 后台任务管理 |
| `infra/mod.rs:8` | pub mod | `async_util` | 同步上下文驱动 async |

### infra/state/mod.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `state/mod.rs:1` | pub mod | `store` | 配置 ArcSwap 存储 |
| `state/mod.rs:2` | pub mod | `network` | 网络状态快照 |
| `state/mod.rs:3` | pub mod | `exit` | 退出状态与截止时间 |
| `state/mod.rs:13` | pub const | `AUTO_EXIT_DELAY_MS: u64 = 20000` | 登录后自动退出倒计时（20s），`lifecycle::start_auto_exit` 等使用 |
| `state/mod.rs:14` | pub const | `CANCEL_EXIT_SHORTCUT: &str = "CommandOrControl+Shift+C"` | 取消退出快捷键文本；`lifecycle.rs` 注册/注销 |
| `state/mod.rs:16-18` | pub struct | `TaskLock` | 单标志任务互斥锁 |
| `state/mod.rs:20-22` | pub struct | `TaskGuard<'a>` | 锁的 RAII 句柄，Drop 时释放 |
| `state/mod.rs:24-28` | impl | `Default for TaskLock` | 委托 `new` |
| `state/mod.rs:30-52` | impl | `TaskLock` | `new`/`try_acquire`/`is_active`/`force_release`(test) |
| `state/mod.rs:54-58` | impl | `Drop for TaskGuard<'_>` | `Release` 清标志 |
| `state/mod.rs:60-62` | 私有 static | `ACCOUNT_NAME_RE: regex::Regex` | `^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$` |
| `state/mod.rs:64-72` | pub fn | `validate_account_name(&str) -> Result<String, String>` | 账号名 1-32 字符 + 字符集校验 |
| `state/mod.rs:74-88` | pub fn | `sanitize_account_id(user: &str) -> String` | 自动建号 id 生成（R2）：非法字符替换 `_`、按字符截断 32 |
| `state/mod.rs:90-96` | pub struct | `TaskFlags` | 5 个任务互斥锁聚合 |
| `state/mod.rs:98-110` | pub struct | `ScheduledFired` | 定时动作当日触发标记与补试计数 |
| `state/mod.rs:112-116` | impl | `Default for ScheduledFired` | 委托 `new` |
| `state/mod.rs:118-141` | impl | `ScheduledFired` | `new`/`bump_login_retry`/`reset_login_retry` |
| `state/mod.rs:143-161` | pub struct | `UpdateStats` | 更新/通知/心跳/恢复/自动启用原子统计字段 |
| `state/mod.rs:163-167` | impl | `Default for UpdateStats` | 委托 `new` |
| `state/mod.rs:169-182` | impl | `UpdateStats` | `new` |
| `state/mod.rs:184-192` | pub struct | `AppState` | 全局状态根，Tauri 托管对象 |
| `state/mod.rs:194-198` | impl | `Default for AppState` | 委托 `new` |
| `state/mod.rs:200-218` | impl | `AppState` | `new` 装配全部子状态 |
| `state/mod.rs:220-289` | `#[cfg(test)]` mod | `tests` | TaskLock/账号名/sanitize 测试 |
| `state/mod.rs:291-298` | pub struct | `CommandResult` | 通用命令返回（`Serialize`） |
| `state/mod.rs:300-310` | impl | `CommandResult` | `ok`/`ok_msg`/`err` |
| `state/mod.rs:314-327` | pub struct | `AccountItem` | 账号列表条目（R3） |
| `state/mod.rs:329-342` | pub struct | `AccountResult` | 账号操作返回（camelCase） |
| `state/mod.rs:343-353` | impl | `AccountResult` | `ok`/`ok_with_account`/`err` |

**结构体字段与方法明细：**

- `TaskLock` (`state/mod.rs:16-18`)：字段 `flag: AtomicBool`。方法 `new` (31)，`try_acquire` (35) CAS 抢锁返回 `Option<TaskGuard>`，`is_active` (43) `Acquire` 读，`force_release` (49, `#[cfg(test)]`)。`TaskGuard` (20-22) 字段 `lock`；`Drop` (54-58) 释放。
- `TaskFlags` (`state/mod.rs:90-96`)：字段 `is_checking`, `is_logging_in`, `is_logging_out`, `is_quality_checking`, `is_downloading`，均为 `TaskLock`。
- `ScheduledFired` (`state/mod.rs:98-110`)：字段 `login_day: AtomicI32`, `logout_day: AtomicI32`, `login_retry_day: AtomicI32`, `login_retry_count: AtomicU32`。方法 `bump_login_retry` (129) 跨天归零后计数，`reset_login_retry` (138) 清零。
- `UpdateStats` (`state/mod.rs:143-161`)：字段 `update_notified: AtomicBool`, `last_disabled_notification_ms: AtomicU64`, `last_network_change_notification_ms: AtomicU64`, `last_render_heartbeat_ms: AtomicU64`, `webview_recovery_window_start_ms: AtomicU64`, `webview_recovery_count: AtomicU32`, `auto_enable_last_attempt_ms: AtomicU64`, `auto_enable_failure_count: AtomicU32`。仅 `new` (170)。
- `AppState` (`state/mod.rs:184-192`)：字段 `config: ConfigStore`, `tasks: TaskFlags`, `task_manager: BackgroundTaskManager`, `network: NetworkState`, `exit: ExitStateStore`, `update_stats: UpdateStats`, `scheduled: ScheduledFired`。方法 `new` (201)。
- `CommandResult` (`state/mod.rs:291-298`)：字段 `success: bool`, `message: Option<String>` (`skip_serializing_if`), `data: Option<serde_json::Value>` (`skip_serializing_if`)。方法 `ok` (301), `ok_msg` (304), `err` (307)。
- `AccountItem` (`state/mod.rs:314-327`)：字段 `id: String`, `display_name: String` (rename), `user: String` (default), `operator: String` (default), `password_saved: bool` (rename, default)。
- `AccountResult` (`state/mod.rs:329-342`)：字段 `success: bool`, `message: Option<String>`, `active_account: Option<String>`, `display_name: Option<String>` (rename), `config: Option<Config>`。方法 `ok` (344), `ok_with_account` (347), `err` (350)。

### infra/state/store.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `state/store.rs:6-8` | pub struct | `ConfigStore` | 封装 `ArcSwap<Config>` |
| `state/store.rs:11` | pub fn | `new(config: Config)` | 建初值 |
| `state/store.rs:18` | pub fn | `load(&self) -> Arc<Config>` | 取不可变快照 |
| `state/store.rs:23` | pub fn | `load_full(&self) -> Arc<Config>` | `load` 兼容别名 |
| `state/store.rs:28` | pub fn | `store(&self, Config) -> Arc<Config>` | 整体替换 |
| `state/store.rs:35-49` | pub fn | `update<F>(&self, f) -> Arc<Config>` | CAS 循环改配置 |
| `state/store.rs:52-69` | `#[cfg(test)]` mod | `tests` | 默认值与原子更新测试 |

### infra/state/network.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `state/network.rs:7-31` | pub struct | `NetworkSnapshot` (`Clone`) | 21 字段网络状态快照（含 `Arc<str>` 字段减少拷贝） |
| `state/network.rs:33-56` | impl | `Default for NetworkSnapshot` | 时间字段取 `Instant::now()` |
| `state/network.rs:59-61` | pub struct | `NetworkState` | 封装 `ArcSwap<NetworkSnapshot>` |
| `state/network.rs:63-67` | impl | `Default for NetworkState` | 委托 `new` |
| `state/network.rs:70` | pub fn | `new() -> Self` | 建初值 |
| `state/network.rs:77` | pub fn | `load(&self) -> Arc<NetworkSnapshot>` | 取一致快照 |
| `state/network.rs:82-96` | pub fn | `update<F>(&self, f)` | CAS 循环写 |
| `state/network.rs:102-117` | pub fn | `update_with_result<T, F>(&self, f) -> T` | CAS 写并返回闭包结果（避免 TOCTOU） |
| `state/network.rs:120-143` | `#[cfg(test)]` mod | `tests` | 默认快照与原子更新测试 |

**`NetworkSnapshot` 字段** (`state/network.rs:7-31`)：`server_available`, `any_adapter_online`, `last_a1_online`, `last_a2_online`, `has_logged_online`, `disconnect_reconnect_count`, `background_check_count`, `last_auto_login_attempt: Instant`, `last_network_quality: Option<Arc<str>>`, `current_ssid: Option<Arc<str>>`, `on_campus_network`, `logout_protected_until: Instant`, `portal_failure_count`, `a1_auth_failure_count`, `a2_auth_failure_count`, `prep_login_failures: u32`, `last_login_available: bool`。

### infra/state/exit.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `state/exit.rs:7-13` | pub struct | `ExitStateStore` | 退出状态（5 个 pub 字段） |
| `state/exit.rs:15-19` | impl | `Default for ExitStateStore` | 委托 `new` |
| `state/exit.rs:22` | pub fn | `new() -> Self` | 全 false / None |
| `state/exit.rs:32` | pub fn | `deadline(&self) -> Option<Instant>` | 读自动退出截止 |
| `state/exit.rs:36` | pub fn | `set_deadline(&self, Option<Instant>)` | 写自动退出截止 |
| `state/exit.rs:40` | pub fn | `campus_exit_deadline(&self) -> Option<Instant>` | 读校园网退出截止 |
| `state/exit.rs:44` | pub fn | `set_campus_exit_deadline(&self, Option<Instant>)` | 写校园网退出截止 |

字段：`is_quitting: Arc<AtomicBool>`, `auto_exit_deadline: Mutex<Option<Instant>>`, `auto_exit_cancelled: AtomicBool`, `campus_exit_started: AtomicBool`, `campus_exit_deadline: Mutex<Option<Instant>>`。

### infra/logger.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `logger.rs:11` | 私有 const | `MAX_LOG_SIZE: u64 = 5*1024*1024` | 单文件 5MB 触发轮转 |
| `logger.rs:12` | 私有 const | `MAX_LOG_FILES: usize = 5` | 轮转时按 mtime 保留最新 5 个 |
| `logger.rs:13` | 私有 const | `FLUSH_INTERVAL_MS: u64 = 2000` | worker `recv_timeout` 与批量落盘间隔 |
| `logger.rs:14` | 私有 const | `CHANNEL_CAPACITY: usize = 1024` | `drain_channel` 单次排空上限 |
| `logger.rs:17` | 私有 const | `LOG_CHANNEL_CAPACITY: usize = 8192` | 审计 A17：有界通道容量 |
| `logger.rs:20-21` | 私有 const | `DROP_REPORT_INTERVAL` / `DROP_REPORT_THRESHOLD` | 丢弃计数周期汇总阈值 |
| `logger.rs:22` | 私有 static | `LOG_DROPPED: AtomicU64` | 发送端丢弃计数 |
| `logger.rs:24` | 私有 static | `LOG_RETENTION_DAYS: AtomicU32 = 7` | 进程级保留天数 |
| `logger.rs:27` | 私有 static | `LOG_DIR: OnceLock<PathBuf>` | init_logger 记录目录供即清 |
| `logger.rs:29-37` | pub fn | `set_log_retention_days(days: u32)` | 写保留天数（0=永久），立即清理一次 |
| `logger.rs:39-41` | pub fn | `get_log_retention_days() -> u32` | 读保留天数 |
| `logger.rs:43-49` | pub enum | `LogLevel { Debug, Info, Warn, Error }` | `PartialOrd/Ord` 派生 |
| `logger.rs:51-60` | 私有 impl | `LogLevel::as_str` | 级别文本 |
| `logger.rs:62-66` | 私有 enum | `LogMessage { Entry, Flush, Shutdown }` | 通道消息 |
| `logger.rs:68-73` | 私有 struct | `LoggerState` | worker 私有状态 |
| `logger.rs:75-80` | 私有 static | `LOGGER_SENDER`/`CLEAR_LOGS_MUTEX`/`LOGGER_THREAD`/`MIN_LOG_LEVEL` | 全局发送端、锁、线程、级别 |
| `logger.rs:82-135` | pub fn | `init_logger(log_dir: PathBuf) -> Result<(), String>` | 建目录、开当天文件、起 worker；重复调用先 Shutdown 旧线程 |
| `logger.rs:137-203` | 私有 fn | `logger_worker(state, receiver)` | 消费循环：超时分支小时级清理，drain+条件落盘 |
| `logger.rs:142` | 私有 const (在 worker 内) | `CLEANUP_INTERVAL = 3600s` | worker 内清理间隔 |
| `logger.rs:145` | 私有 const (在 worker 内) | `BATCH_FLUSH_THRESHOLD = 32` | 批量落盘条数阈值 |
| `logger.rs:146` | 私有 const (在 worker 内) | `FLUSH_INTERVAL = 2000ms` | 距上次落盘时间阈值 |
| `logger.rs:205-221` | 私有 fn | `drain_channel(receiver, buffer)` | `try_recv` 排空（Shutdown 丢弃） |
| `logger.rs:223-252` | 私有 fn | `flush_messages(state, buffer)` | 先 `rotate_if_needed`，写 Entry/Flush 回 ack |
| `logger.rs:254-296` | 私有 fn | `rotate_if_needed(state)` | 跨天换文件；超 5MB 改名轮转 |
| `logger.rs:298-316` | 私有 fn | `cleanup_old_logs(log_dir)` | 轮转后按 mtime 保留最新 5 个 `.log` |
| `logger.rs:318-342` | 私有 fn | `cleanup_old_logs_by_time(log_dir, retention_days)` | 按保留天数删；0 返回 |
| `logger.rs:344-370` | pub fn | `log(level, module, message)` | 级别过滤→组行→try_send；满则丢计数，Warn+ eprint 兜底 |
| `logger.rs:372-374` | pub fn | `set_log_level(level)` | 改最低级别 |
| `logger.rs:376-378` | pub fn | `get_log_level() -> LogLevel` | 读最低级别 |
| `logger.rs:381-386` | `#[tauri::command]` | `set_debug_mode(enabled: bool)` | Debug/Info 切换 |
| `logger.rs:387-391` | pub fn | `get_debug_mode() -> Result<bool, String>` | 查询是否 Debug |
| `logger.rs:393-419` | `#[macro_export]` | `log_debug!`/`log_info!`/`log_warn!`/`log_error!` | 展开为 `log` |
| `logger.rs:421-430` | pub fn (`#[cfg(android)]`) | `get_log_dir(&AppHandle) -> PathBuf` | 安卓 `app_data_dir()/logs` |
| `logger.rs:432-443` | pub fn (`#[cfg(not(android))]` | `get_log_dir(&AppHandle) -> PathBuf` | 桌面 `current_exe()/logs` |
| `logger.rs:445-487` | pub fn | `read_recent_logs(&AppHandle, lines) -> Result<String, String>` | 尾部按 8KB×2ⁿ 窗口倒读取尾 N 行 |
| `logger.rs:489-498` | pub fn | `flush()` | 发 `Flush` 等 ack，超时 5s |
| `logger.rs:501-509` | pub fn | `flush_quick()` | panic hook 专用，ack 超时 500ms |
| `logger.rs:511-526` | pub fn | `shutdown()` | swap sender、发 Shutdown、join 线程（500ms 超时） |
| `logger.rs:528-533` | pub fn | `is_app_log_file(name: &str) -> bool` | `app-` 前缀 + `.log` 后缀 |
| `logger.rs:535-591` | pub fn | `clear_logs(&AppHandle) -> Result<(), String>` | 先删应用日志文件，再换新 sender+线程 |
| `logger.rs:594-614` | `#[cfg(test)]` mod | `tests` | `is_app_log_file` 正负用例 |

### infra/events.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `events.rs:8-10` | pub struct | `EventBus<'a>` | 事件发射器，仅持有 `&AppHandle` |
| `events.rs:13` | pub fn | `EventBus::new(&AppHandle)` | 构造 |
| `events.rs:17-19` | 私有 fn | `emit<S>(&self, &str, S) -> Result<(), String>` | 唯一底层发射 |
| `events.rs:22-27` | pub fn | `emit_login_log(&self, message: &str, log_type: &str)` | 事件 `login-log` |
| `events.rs:30-32` | pub fn | `emit_background_check_result<S>(&self, payload: S)` | 事件 `background-check-result` |
| `events.rs:36-38` | pub fn | `emit_scheduled_action_skipped(&self, kind: &str)` | 事件 `scheduled-action-skipped`（A7） |
| `events.rs:41-43` | pub fn | `emit_mac_reset_cycle(&self)` | 事件 `mac-reset-cycle`（A16） |
| `events.rs:46-52` | pub fn | `emit_auto_login_result(&self, success, message, skipped)` | 事件 `auto-login-result` |
| `events.rs:55-57` | pub fn | `emit_network_quality_result<S>(&self, payload: S)` | 事件 `network-quality-result` |
| `events.rs:60-66` | pub fn | `emit_update_available(&self, has_update, latest_version, release_notes)` | 事件 `update-available` |
| `events.rs:69-71` | pub fn | `emit_update_notification_click(&self)` | 事件 `update-notification-click` |
| `events.rs:74-76` | pub fn | `emit_adapter_details_changed<S>(&self, details: S)` | 事件 `adapter-details-changed` |
| `events.rs:79-84` | pub fn | `emit_adapter_disabled_warning(&self, name: &str, message: &str)` | 事件 `adapter-disabled-warning` |
| `events.rs:87-92` | pub fn | `emit_campus_exit_countdown(&self, minimize_delay, exit_delay)` | 事件 `campus-exit-countdown` |
| `events.rs:95-97` | pub fn | `emit_campus_exit_cancelled(&self)` | 事件 `campus-exit-cancelled` |
| `events.rs:100-105` | pub fn | `emit_auto_exit_countdown(&self, delay, shortcut)` | 事件 `auto-exit-countdown` |
| `events.rs:108-110` | pub fn | `emit_auto_exit_cancelled(&self)` | 事件 `auto-exit-cancelled` |
| `events.rs:113-115` | pub fn | `emit_config_changed<S>(&self, payload: S)` | 事件 `config-changed` |
| `events.rs:118-120` | pub fn | `emit_adapters_changed<S>(&self, adapters: S)` | 事件 `adapters-changed` |
| `events.rs:123-125` | pub fn | `emit_update_download_progress<S>(&self, payload: S)` | 事件 `update-download-progress` |
| `events.rs:129-141` | `#[cfg(test)]` mod | `tests` | 仅 `size_of` 编译期断言 |

### infra/task_manager.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `task_manager.rs:8-11` | pub struct | `TaskHandle` | 单任务句柄（公开 `cancel_token`，私有 `join_handle`） |
| `task_manager.rs:14-17` | pub struct (`Clone`) | `BackgroundTaskManager` | 名称→句柄表 |
| `task_manager.rs:19-23` | impl | `Default` | 委托 `new` |
| `task_manager.rs:26` | pub fn | `new() -> Self` | 空表 |
| `task_manager.rs:41-72` | pub fn | `spawn<F, Fut>(&self, name: &str, build_future: F) -> Result<(), String>` | 同名已在跑则 `Err`；结束自动摘除 |
| `task_manager.rs:67-75` | pub fn | `cancel(&self, name: &str) -> bool` | 摘除并 `cancel()` 令牌 |
| `task_manager.rs:81-83` | pub fn | `detach(&self, name: &str) -> bool` | 只摘除不取消（防 shutdown 死锁） |
| `task_manager.rs:86-98` | pub async fn | `shutdown(&self)` | 排空→全部 cancel→全部 await |
| `task_manager.rs:101-103` | pub fn | `is_running(&self, name: &str) -> bool` | 查询 |
| `task_manager.rs:106-108` | pub fn | `cancel_token(&self, name: &str) -> Option<Arc<CancellationToken>>` | 取令牌 |
| `task_manager.rs:112-147` | `#[cfg(test)]` mod | `tests` | 重复拒绝/cancel/shutdown 测试 |

### infra/lifecycle.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `lifecycle.rs:12` | 私有 const | `CAMPUS_MINIMIZE_DELAY_MS: u64 = 30000` | 非校园网：30s 后最小化 |
| `lifecycle.rs:13` | 私有 const | `CAMPUS_EXIT_DELAY_MS: u64 = 60000` | 非校园网：60s 后退出 |
| `lifecycle.rs:17-19` | 私有 fn | `is_within_campus_exit_window(now, start, end) -> bool` | 当日分钟数判窗；`end<=start` 退化仅 start |
| `lifecycle.rs:23-143` | pub fn | `start_campus_exit(&AppHandle, &AppState)` | 校园网验证不通过启动倒计时（配置+时间窗门控） |
| `lifecycle.rs:146-162` | pub fn | `cancel_campus_exit(&AppHandle, &AppState)` | 静默取消（不发通知/事件） |
| `lifecycle.rs:165-189` | pub fn | `cancel_campus_exit_with_notification(&AppHandle, &AppState)` | 带通知+`campus-exit-cancelled` 取消 |
| `lifecycle.rs:191-272` | pub fn | `start_auto_exit(&AppHandle, &AppState)` | 登录后 20s 自动退出；用户已取消则跳过 |
| `lifecycle.rs:274-298` | pub fn | `cancel_auto_exit_inner(&AppHandle, &AppState) -> Result<CommandResult, String>` | 取消自动退出 |
| `lifecycle.rs:301-310` | 私有 fn (`#[cfg(desktop)]`) | `try_unregister_cancel_exit_shortcut(&AppHandle, bool)` | 条件注销快捷键 |
| `lifecycle.rs:313-314` | 私有 fn (`#[cfg(not(desktop))]`) | `try_unregister_cancel_exit_shortcut(_, _)` | 空实现 |
| `lifecycle.rs:319-328` | pub async fn | `shutdown_and_exit(&AppHandle, &AppState)` | 置 quitting→10s 上限等 shutdown→exit(0) |
| `lifecycle.rs:331-352` | `#[cfg(test)]` mod | `tests` | 时间窗判定测试 |

### infra/notification.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `notification.rs:16` | pub fn | `emit_notification(&AppHandle, title: &str, body: &str, mascot: &str)` | 系统通知唯一出口；双重门控后异步发通知 |

### infra/command_context.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `command_context.rs:10-13` | pub struct | `CommandContext<'a>` | 命令层 AppHandle + AppState 打包 |
| `command_context.rs:16` | pub fn | `CommandContext::new(&AppHandle, &AppState)` | 显式构造 |
| `command_context.rs:24-27` | pub fn | `CommandContext::from_app(&AppHandle)` | 内部取 state `inner()` |
| `command_context.rs:29-32` | impl | `Deref for CommandContext<'a>` (`Target = AppState`) | `ctx.config` 等价 `ctx.state.config` |
| `command_context.rs:38-41` | pub trait | `AppHandleExt` | 定义 `notify_config_changed`/`notify_update_download_progress` |
| `command_context.rs:43-51` | impl | `AppHandleExt for AppHandle` | 均委托 `EventBus` |

### infra/async_util.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `async_util.rs:26-41` | 私有 fn | `fallback_runtime() -> &'static Runtime` | `OnceLock` 多线程 Runtime 兜底 |
| `async_util.rs:48-53` | pub fn | `block_on_sync<F: Future>(future: F) -> F::Output` | 有 runtime context 走 `Handle::block_on`，否则兜底 Runtime |

## Architecture

```text
┌───────────────────────┐     ┌───────────────────────┐
│   infra/state         │     │   infra/logger         │
│  mod.rs:184 AppState  │     │  logger.rs:82 init     │
│  store/network/exit   │     │  logger-worker:137     │
└───────┬───────────────┘     └───────────┬───────────┘
        │                                 │
        ▼                                 ▼
┌───────────────────────┐     ┌───────────────────────┐
│  infra/task_manager   │     │  infra/events         │
│  task_manager.rs:15   │     │  events.rs:8 EventBus │
│  spawn/cancel/detach  │     │  emit_xxx (17 methods)│
└───────┬───────────────┘     └───────────┬───────────┘
        │                                 │
        ▼                                 ▼
┌───────────────────────┐     ┌───────────────────────┐
│  infra/lifecycle      │     │  infra/notification   │
│  lifecycle.rs:23,191  │     │  notification.rs:16   │
│  shutdown_and_exit:319│     │  emit_notification    │
└───────────────────────┘     └───────────────────────┘
        │
        ▼
┌───────────────────────┐
│ infra/command_context │
│ command_context.rs:10 │
│ AppHandleExt:38       │
└───────────────────────┘
```

## Data Flow

### 状态进入/变换/离开

- 装配入口：`AppState::new()` (`state/mod.rs:201`) 构造全部子状态；命令层通过 `CommandContext::from_app` (`command_context.rs:22`) 或 `app_handle.state::<AppState>()` 获取。
- 配置变换统一走 CAS：`ConfigStore::update` (`state/store.rs:35-49`) → `ArcSwap::compare_and_swap` → 胜出返回新快照；读路径 `load()` (`state/store.rs:18`)。
- 网络状态变换走 `NetworkState::update_with_result` (`state/network.rs:102-117`)，"读-改-判定"同一 CAS 循环内。
- 退出状态：`ExitStateStore` 方法 (`state/exit.rs:32-46`) 提供 deadline 读写。
- 离开路径：仅通过 `EventBus` / `#[tauri::command]` 返回值 / `emit_notification` 三条出口。

### 日志链路

```text
log_info!/log_warn!/... (logger.rs:393-419)
  → logger::log(level, module, message) (logger.rs:344)  —— MIN_LOG_LEVEL 过滤 (logger.rs:345)
  → LOGGER_SENDER.load().try_send(LogMessage::Entry) (logger.rs:352,356)
  → mpsc 有界通道（容量 8192，满则丢弃计数 LOG_DROPPED，Warn 以上 eprint 兜底）(logger.rs:17,22,356-365)
  → logger-worker 线程 logger_worker (logger.rs:137)
      recv_timeout(2s) → push buffer → drain_channel(:205)
      → 条件落盘 flush_messages(:200)：含 Flush / ≥32 条 / 距上次 ≥2s (logger.rs:195-201)
      → rotate_if_needed(:254)：跨天换文件 / 超 5MB 轮转 + cleanup_old_logs(:277)
      → BufWriter.write_all + flush (logger.rs:234,243)
  超时分支每 3600s 调 cleanup_old_logs_by_time(:164-168)
```

收尾：`flush()` (`logger.rs:489`) → `shutdown()` (`logger.rs:511`)；panic 走 `flush_quick()` (`logger.rs:501`)。前端读日志 `read_recent_logs` (`logger.rs:445`)，清日志 `clear_logs` (`logger.rs:535`)。

### 事件总线（全部 17 个事件）

所有桌面端 emit 收敛在 `events.rs` 内——除 `events.rs:17` 私有 `emit` 外，全仓事件发射应通过 `EventBus` 方法。

| 事件名 | payload 字段 | 发出点 (events.rs) | 备注 |
| --- | --- | --- | --- |
| `login-log` | `message`, `type` | `:22-27` | |
| `background-check-result` | 调用方传入 | `:30-32` | |
| `scheduled-action-skipped` | `kind` | `:36-38` | 审计 A7 |
| `mac-reset-cycle` | `{}` | `:41-43` | 审计 A16 |
| `auto-login-result` | `success`, `message`, `skipped` | `:46-52` | |
| `network-quality-result` | 调用方传入 | `:55-57` | |
| `update-available` | `hasUpdate`, `latestVersion`, `releaseNotes` | `:60-66` | |
| `update-notification-click` | `{}` | `:69-71` | `platform/toast.rs` 调用 |
| `adapter-details-changed` | 调用方传入 | `:74-76` | |
| `adapter-disabled-warning` | `name`, `message` | `:79-84` | |
| `campus-exit-countdown` | `minimizeDelay`, `exitDelay` | `lifecycle.rs:59` 调用 `:87-92` | |
| `campus-exit-cancelled` | `{}` | `lifecycle.rs:186` 调用 `:95-97` | |
| `auto-exit-countdown` | `delay`, `shortcut` | `lifecycle.rs:212` 调用 `:100-105` | |
| `auto-exit-cancelled` | `{}` | `lifecycle.rs:293` 调用 `:108-110` | |
| `config-changed` | 调用方传入（完整 config） | `:113-115`（`command_context.rs:45` 转发） | |
| `adapters-changed` | 调用方传入 | `:118-120` | |
| `update-download-progress` | 调用方传入 | `:123-125`（`command_context.rs:49` 转发） | |

### 退出生命周期状态机

自动退出（登录成功后）：

```text
start_auto_exit (lifecycle.rs:191)
  → auto_exit_cancelled 真则 return (:193)
  → 锁 auto_exit_deadline：Some 则 return；None 置 now+20s (:197-206)
  → emit auto-exit-countdown (:212) + 系统通知 (:216)
  → 注册 Ctrl+Shift+C (:219-226)
  → task_manager.spawn("auto_exit", ...) (:236)
        sleep(deadline-now) / cancel 二选一 (:245-248)
        → 二次校验 deadline 到期 (:250-259)
        → 仅 campus_exit 未启动则注销快捷键 (:261-262)
        → detach("auto_exit") (:264) —— 防 shutdown 等自己
        → shutdown_and_exit (:265)
  → spawn 失败回滚 deadline (:269)
```

取消：`cancel_auto_exit_inner` (`lifecycle.rs:274`) → 清 deadline、置 `auto_exit_cancelled`、`cancel("auto_exit")`、注销快捷键、通知 + `auto-exit-cancelled`。

非校园网退出：`start_campus_exit` (`lifecycle.rs:23`) → 配置门控 (`:25`) → 时间窗门控 (`:32`) → `campus_exit_started` CAS (`:51`) → 写 deadline (`:54`) → 事件+通知+注册快捷键 → `spawn("campus_exit")` (`:78`) → 30s 后 `window.hide()` (`:91-101`) → 再 30s → deadline 终校验 (`:104-121`) → `detach` + `shutdown_and_exit`。

统一退出：`shutdown_and_exit` (`lifecycle.rs:319-328`) → `is_quitting=true` → `timeout(10s, task_manager.shutdown())` → `app_handle.exit(0)`。

### 通知链路

```text
emit_notification(app, title, body, mascot) (notification.rs:16)
  → 配置门控：config.enable_notification false 则 return (:17-23)
  → 可见性门控：窗口 is_visible && !is_minimized → return（用户能看到界面）(:29-34)
  → spawn 异步 (:40)
      Windows 桌面：platform::toast::show_system_toast (mascot 圆形头像) (:46)
        失败 → tauri_plugin_notification 纯文本降级 (:50-53)
      其他平台：直接用 tauri_plugin_notification（mascot 被丢弃）(:56-63)
```

### 扩展点（改动前必读）

- **新增事件**：只在 `infra/events.rs` 内追加 `emit_xxx` 方法（现有最后方法在 `:123-125`），保持事件名/payload 契约；不要绕过 `EventBus` 直接 `emit`。
- **新增后台周期任务**：`state.task_manager.spawn("唯一名字", move |cancel_token| async move { ... })`（`task_manager.rs:33`）；任务名重复返回 `Err`；`tokio::select!` 必须处理 `cancel_token.cancelled()`。
- **读取全局状态**：命令里用 `CommandContext::from_app(&app_handle)`（`command_context.rs:22`）或 `app_handle.state::<AppState>()`；两者同实例。
- **新增配置字段**：改 `config/model.rs` 后需同步 `ConfigStore::update` 调用方。

## Connections

- [[desktop-app-lifecycle]]：`infra::state::AppState` 由 `lifecycle.rs` 使用（`start_campus_exit` 等签名含 `&AppState`）；`lifecycle::shutdown_and_exit` 调用 `state.task_manager.shutdown()`（`lifecycle.rs:323`）；`BackgroundTaskManager` 在 `state/mod.rs:187` 持有。
- [[desktop-config]]：`ConfigStore`（`state/store.rs`）承载 `config::model::Config`；`AppState.config` 字段 `state/mod.rs:185`。
- [[desktop-auth]]：登录/注销命令拿 `TaskFlags`（`state/mod.rs:90`）；`lifecycle::start_auto_exit` 由登录成功路径触发。
- [[desktop-monitor]]：`start_campus_exit` / `cancel_campus_exit` 主要调用方在 monitor 模块；`UpdateStats` 节流字段（`state/mod.rs:147`）由 monitor 使用。
- [[desktop-platform]]：`notification.rs:46` 依赖 `platform::toast`；`logger.rs:421-443` 平台目录策略。
- [[android-backend]]：安卓端仅复用 `infra::logger`（`logger.rs:421-430` 的 `#[cfg(target_os = "android")]` `get_log_dir`），不使用 `AppState` / `EventBus` / `BackgroundTaskManager`。
- [[desktop-frontend-hooks]]：事件消费点对应前端监听，事件名见上方表格（发出点 `events.rs`）。

## Known Issues

1. **日志按天清理可能长期不执行**：`cleanup_old_logs_by_time` 只在 `logger_worker` 的 `recv_timeout` **超时分支**被检查（`logger.rs:148-170`，清理调用 `:164-168`）。日志持续写入（间隔 < 2s）时该分支永不进入，`CLEANUP_INTERVAL`（`logger.rs:142`）形同虚设，只能靠 5MB 轮转路径的 `cleanup_old_logs`（`logger.rs:277`）做文件数控制。
2. **`clear_logs` 新建的 `LoggerState` 的 `current_date` 是空串、`current_writer` 是 `None`**（`logger.rs:554-559`），首次写入依赖 `rotate_if_needed` 在 `today != ""` 时创建文件（`logger.rs:256-267`）；这中间的 `write_all` 分支不会执行，但与注释声明的删除顺序耦合，改动时需一并考虑。
3. **`LOG_RETENTION_DAYS` 是进程级 static**（`logger.rs:24`），不属于 `AppState`，多实例/测试并发时共享；`set_log_retention_days` 无同步，写入可见性仅靠 `Relaxed`（`logger.rs:30`）。
4. **保留天数超过 `SystemTime` 可表示范围时跳过清理**（`logger.rs:322-330`），`retention_days == 0` 表示永久保留（`logger.rs:319-321`）。
5. **`TaskLock` 无 owner、不可重入**（`state/mod.rs:30-52`）：`try_acquire` 返回 `None` 只表示"忙"，调用方必须自己决定提示还是静默。`force_release` 仅在 `#[cfg(test)]` 下存在（`state/mod.rs:49`）。
6. **`start_campus_exit` 的 CAS 与 `set_deadline` 之间存在窗口**（`lifecycle.rs:51-54`）：注释自述若在该窗口内发生取消，会出现"一次取消无效"；用极小概率失效窗口换取避免确定性永久卡死。
7. **`spawn("campus_exit")` 失败时的回滚依赖显式语句**（`lifecycle.rs:137-142`）：若后续新增提前 return 分支而漏写回滚，`campus_exit_started` 会永久为 `true` 让后续调用短路。
8. **`cancel_campus_exit` 与 `cancel_campus_exit_with_notification` 行为不一致**（`lifecycle.rs:146` vs `:165`）：前者不发通知/事件，后者发。调用方必须按需要选择。
9. **快捷键注销依赖锁临界区**（`lifecycle.rs:128-131`、`157-160`、`179-181`）：`is_none()` 检查与 `unregister` 必须同锁，若把 `guard` 提前 drop 会重新引入 TOCTOU。
10. **`block_on_sync` 在异步上下文中会 panic/死锁**（`async_util.rs:48-53` 注释）：`Ok` 分支走 `Handle::block_on`，调用者必须经 `spawn_blocking` 或在同步线程调用。
11. **`block_on_sync` 的兜底 Runtime 一旦初始化即常驻**（`async_util.rs:48-53`）：`OnceLock<Runtime>` 不会关闭，进程退出时由 Drop 收尾。
12. **`emit_notification` 的 `mascot` 参数在非 Windows 桌面被丢弃**（`notification.rs:58` 的 `let _ = &mascot;`），安卓端看板娘大图改由其他模块 `large_icon` 负责。
13. **`emit_notification` 已不再向前端发事件**（`notification.rs:16-19` 注释）：应用内 toast 与日志由各业务事件负责，双通道重复是已根除的历史缺陷，不要在本函数内补 emit。
14. **`EventBus` 无单元测试**（`events.rs:139-141`）：只有 `size_of` 断言（`events.rs:139`），事件名/payload 契约靠人工核对，改名不会触发测试失败。
15. **`shutdown_and_exit` 的 10s 上限可能截断后台任务**（`lifecycle.rs:323-325`）：超时后仅记 WARN 就 `exit(0)`；DHCP 子进程、长 HTTP 请求可能被留下。
16. **`logger::flush()` 的 5s 超时静默返回**（`logger.rs:496`），日志线程卡死时调用方无法感知。
