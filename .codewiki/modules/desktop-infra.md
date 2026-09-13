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

本模块是双端应用共享的运行时基座：`state/` 提供配置（ArcSwap + CAS）、网络快照、退出状态三组原子状态与任务互斥锁；`logger.rs` 提供异步落盘的日志通道；`events.rs` 是唯一的事件发射出口（16 个事件）；`task_manager.rs` 统一托管周期性后台任务的取消与等待；`lifecycle.rs` 实现"非校园网自动退出 / 登录后自动退出"两条倒计时状态机与统一退出流程；`notification.rs` 是系统通知的唯一出口。

模块声明于 `tauri-app/src-tauri/src/infra/mod.rs:1-8`，并按 `tauri-app/src-tauri/src/lib.rs:5` 中的 `pub mod infra;` 编入跨平台协议核心 crate（lib 名 `campus_login_lib`，见 `tauri-app/src-tauri/Cargo.toml:8-10`）。安卓端经 path 依赖可见本模块，但实际只用到 `logger`（证据：`android/src-tauri/src/lib.rs:48`、`android/src-tauri/src/system_cmds.rs:33,41,48,53,58,63`），`AppState` 在安卓端完全没有被引用（安卓有自己的 `android/src-tauri/src/android_state.rs`、`config_state.rs`）。

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
| `state/mod.rs:13` | pub const | `AUTO_EXIT_DELAY_MS: u64 = 20000` | 登录后自动退出倒计时（20s），`lifecycle::start_auto_exit` 与 `state/mod.rs:195` 使用 |
| `state/mod.rs:14` | pub const | `CANCEL_EXIT_SHORTCUT: &str = "CommandOrControl+Shift+C"` | 取消退出快捷键文本；`app/shortcut.rs:16` 解析、`lifecycle.rs:69,213,299` 注册/注销 |
| `state/mod.rs:16-18` | pub struct | `TaskLock` | 单标志任务互斥锁 |
| `state/mod.rs:20-22` | pub struct | `TaskGuard<'a>` | 锁的 RAII 句柄，Drop 时释放 |
| `state/mod.rs:24-28` | impl | `Default for TaskLock` | 委托 `new` |
| `state/mod.rs:31` | pub fn | `TaskLock::new() -> Self` | 初始 `false` |
| `state/mod.rs:35` | pub fn | `TaskLock::try_acquire(&self) -> Option<TaskGuard<'_>>` | CAS(false→true, Acquire) 抢锁 |
| `state/mod.rs:43` | pub fn | `TaskLock::is_active(&self) -> bool` | `Acquire` 读当前标志（供 UI 状态显示） |
| `state/mod.rs:49` | pub fn（`#[cfg(test)]`） | `TaskLock::force_release(&self)` | 仅测试可用，`Release` 清标志 |
| `state/mod.rs:54-58` | impl | `Drop for TaskGuard<'_>` | `Release` 清标志 |
| `state/mod.rs:60-62` | 私有 static | `ACCOUNT_NAME_RE: regex::Regex` | `^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$` |
| `state/mod.rs:64` | pub fn | `validate_account_name(&str) -> Result<String, String>` | 账号名 1-32 字符 + 字符集校验 |
| `state/mod.rs:74-80` | pub struct | `TaskFlags` | 5 个任务互斥锁的聚合 |
| `state/mod.rs:86-101` | pub struct | `UpdateStats` | 更新/通知/心跳/恢复/自动启用的 9 个原子统计字段 |
| `state/mod.rs:103-107` | impl | `Default for UpdateStats` | 委托 `new` |
| `state/mod.rs:110` | pub fn | `UpdateStats::new() -> Self` | 全部置 0/false |
| `state/mod.rs:125-132` | pub struct | `AppState` | 全局状态根，Tauri 托管对象 |
| `state/mod.rs:134-138` | impl | `Default for AppState` | 委托 `new` |
| `state/mod.rs:141` | pub fn | `AppState::new() -> Self` | 装配全部子状态；`app/startup.rs:50` 的 `.manage(AppState::new())` 是唯一生产构造点 |
| `state/mod.rs:159-210` | `#[cfg(test)]` mod | `tests` | 5 个 TaskLock 测试 + 2 个账号名校验测试 |
| `state/mod.rs:212-219` | pub struct | `CommandResult` | 通用命令返回（`Serialize`，字段 `skip_serializing_if = "Option::is_none"`） |
| `state/mod.rs:222/225/228` | pub fn | `CommandResult::ok()` / `ok_msg(&str)` / `err(&str)` | 三个构造器 |
| `state/mod.rs:233-243` | pub struct | `AccountResult` | 账号操作返回（`#[serde(rename_all = "camelCase")]`） |
| `state/mod.rs:246/249/252` | pub fn | `AccountResult::ok(Config)` / `ok_with_account(String, Config)` / `err(&str)` | 三个构造器 |

### infra/state/store.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `state/store.rs:6-8` | pub struct | `ConfigStore` | 封装 `ArcSwap<Config>` |
| `state/store.rs:11` | pub fn | `new(config: Config)` | 建初值 |
| `state/store.rs:18` | pub fn | `load(&self) -> Arc<Config>` | 取不可变快照 |
| `state/store.rs:23` | pub fn | `load_full(&self) -> Arc<Config>` | `load` 的兼容别名 |
| `state/store.rs:28` | pub fn | `store(&self, Config) -> Arc<Config>` | 整体替换 |
| `state/store.rs:35-49` | pub fn | `update<F: Fn(&mut Config)>(&self, f) -> Arc<Config>` | CAS 循环改配置，`Arc::ptr_eq` 判成功 |
| `state/store.rs:52-69` | `#[cfg(test)]` mod | `tests` | 默认值与原子更新测试 |

### infra/state/network.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `state/network.rs:7-26` | pub struct | `NetworkSnapshot`（`#[derive(Clone)]`） | 16 字段网络状态快照 |
| `state/network.rs:28-50` | impl | `Default for NetworkSnapshot` | 时间字段取 `Instant::now()` |
| `state/network.rs:53-55` | pub struct | `NetworkState` | 封装 `ArcSwap<NetworkSnapshot>` |
| `state/network.rs:57-61` | impl | `Default for NetworkState` | 委托 `new` |
| `state/network.rs:64` | pub fn | `NetworkState::new() -> Self` | 建初值 |
| `state/network.rs:71` | pub fn | `load(&self) -> Arc<NetworkSnapshot>` | 取一致快照 |
| `state/network.rs:76-90` | pub fn | `update<F: FnMut(&mut NetworkSnapshot)>(&self, f)` | CAS 循环写 |
| `state/network.rs:96-111` | pub fn | `update_with_result<T: Clone, F>(&self, f) -> T` | CAS 写并返回闭包结果（"读-改-判定"场景，避免 TOCTOU） |
| `state/network.rs:114-137` | `#[cfg(test)]` mod | `tests` | 默认快照与原子更新测试 |

### infra/state/exit.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `state/exit.rs:7-13` | pub struct | `ExitStateStore` | 退出相关状态（5 个 pub 字段） |
| `state/exit.rs:15-19` | impl | `Default for ExitStateStore` | 委托 `new` |
| `state/exit.rs:22` | pub fn | `new() -> Self` | 全 false / None |
| `state/exit.rs:32` | pub fn | `deadline(&self) -> Option<Instant>` | 读自动退出截止时间 |
| `state/exit.rs:36` | pub fn | `set_deadline(&self, Option<Instant>)` | 写自动退出截止时间 |
| `state/exit.rs:40` | pub fn | `campus_exit_deadline(&self) -> Option<Instant>` | 读校园网退出截止时间 |
| `state/exit.rs:44` | pub fn | `set_campus_exit_deadline(&self, Option<Instant>)` | 写校园网退出截止时间 |

### infra/logger.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `logger.rs:11` | 私有 const | `MAX_LOG_SIZE: u64 = 5 * 1024 * 1024` | 单文件 5MB 触发轮转 |
| `logger.rs:12` | 私有 const | `MAX_LOG_FILES: usize = 5` | 轮转时按 mtime 保留最新 5 个 |
| `logger.rs:13` | 私有 const | `FLUSH_INTERVAL_MS: u64 = 2000` | worker `recv_timeout` 与批量落盘间隔 |
| `logger.rs:14` | 私有 const | `CHANNEL_CAPACITY: usize = 1024` | `drain_channel` 单次排空上限 |
| `logger.rs:16` | 私有 static | `LOG_RETENTION_DAYS: AtomicU32 = 7` | 进程级保留天数 |
| `logger.rs:18` | pub fn | `set_log_retention_days(days: u32)` | 写保留天数（0 = 永久保留） |
| `logger.rs:22` | pub fn | `get_log_retention_days() -> u32` | 读保留天数 |
| `logger.rs:26-32` | pub enum | `LogLevel { Debug, Info, Warn, Error }` | `PartialOrd/Ord` 派生用于阈值比较 |
| `logger.rs:34-43` | 私有 impl | `LogLevel::as_str(&self) -> &'static str` | 日志行中的级别文本 |
| `logger.rs:45-49` | 私有 enum | `LogMessage` | 通道消息：`Entry` / `Flush` / `Shutdown` |
| `logger.rs:51-56` | 私有 struct | `LoggerState` | worker 线程私有状态 |
| `logger.rs:59` | 私有 static | `LOGGER_SENDER: ArcSwap<Option<Sender<LogMessage>>>` | 全局发送端（`None` = 未初始化/已关闭） |
| `logger.rs:60` | 私有 static | `CLEAR_LOGS_MUTEX: Mutex<()>` | `clear_logs` 互斥 |
| `logger.rs:61` | 私有 static | `LOGGER_THREAD: Mutex<Option<JoinHandle<()>>>` | worker 线程句柄 |
| `logger.rs:62` | 私有 static | `MIN_LOG_LEVEL: ArcSwap<LogLevel> = Info` | 全局最低级别 |
| `logger.rs:65` | pub fn | `init_logger(log_dir: PathBuf) -> Result<(), String>` | 建目录、开当天文件、起 `logger-worker` 线程；重复调用会先给旧 sender 发 `Shutdown` 并 join 旧线程（`logger.rs:85-113`） |
| `logger.rs:118` | 私有 fn | `logger_worker(state, receiver)` | 消费循环：超时分支做小时级清理（`logger.rs:142-148`），随后 `drain_channel` + 条件落盘（`logger.rs:151-161`） |
| `logger.rs:121` | 私有 const | `CLEANUP_INTERVAL = 3600s` | worker 内清理间隔 |
| `logger.rs:124` | 私有 const | `BATCH_FLUSH_THRESHOLD = 32` | 批量落盘条数阈值（BE-C-01） |
| `logger.rs:125` | 私有 const | `FLUSH_INTERVAL = 2000ms` | 距上次落盘时间阈值 |
| `logger.rs:165` | 私有 fn | `drain_channel(receiver, buffer)` | `try_recv` 排空到 buffer（`Shutdown` 丢弃） |
| `logger.rs:183` | 私有 fn | `flush_messages(state, buffer)` | 先 `rotate_if_needed`，写 `Entry`、`Flush` 回 ack，最后 `flush()` |
| `logger.rs:214` | 私有 fn | `rotate_if_needed(state)` | 跨天换文件；超 5MB 改名 `app-<YYYYMMDDHHMMSS>.log`（失败则原地截断，`logger.rs:239-243`） |
| `logger.rs:258` | 私有 fn | `cleanup_old_logs(log_dir: &PathBuf)` | 轮转后按 mtime 保留最新 5 个 `.log` |
| `logger.rs:278` | 私有 fn | `cleanup_old_logs_by_time(log_dir: &Path, retention_days: u32)` | 按保留天数删文件；`retention_days == 0` 直接返回 |
| `logger.rs:304` | pub fn | `log(level, module, message)` | 级别过滤 → 组行 → 发通道；send 失败时 Warn 以上直接 `eprint!`（`logger.rs:315-324`） |
| `logger.rs:327` | pub fn | `set_log_level(level: LogLevel)` | 改全局最低级别 |
| `logger.rs:331` | pub fn | `get_log_level() -> LogLevel` | 读全局最低级别 |
| `logger.rs:335-340` | `#[tauri::command]` pub fn | `set_debug_mode(enabled: bool) -> Result<bool, String>` | Debug/Info 切换；注册于 `app/startup.rs:118` |
| `logger.rs:342-345` | `#[tauri::command]` pub fn | `get_debug_mode() -> Result<bool, String>` | 查询是否 Debug 级；注册于 `app/startup.rs:119` |
| `logger.rs:347-352` | `#[macro_export]` macro | `log_debug!($module, $($arg)*)` | 展开为 `log(LogLevel::Debug, ...)` |
| `logger.rs:354-359` | `#[macro_export]` macro | `log_info!` | 同上，Info |
| `logger.rs:361-366` | `#[macro_export]` macro | `log_warn!` | 同上，Warn |
| `logger.rs:368-373` | `#[macro_export]` macro | `log_error!` | 同上，Error |
| `logger.rs:375-384` | pub fn（`#[cfg(target_os = "android")]`） | `get_log_dir(&AppHandle) -> PathBuf` | 安卓落在 `app_data_dir()/logs`（APK 安装目录只读） |
| `logger.rs:386-397` | pub fn（`#[cfg(not(target_os = "android"))]`） | `get_log_dir(&AppHandle) -> PathBuf` | 桌面落在 `current_exe()` 同级 `logs/` |
| `logger.rs:399` | pub fn | `read_recent_logs(&AppHandle, lines: usize) -> Result<String, String>` | 从当天文件尾部按 8KB×2ⁿ 窗口倒读，取尾 N 行（BE-C-01） |
| `logger.rs:420` | 私有 const | `BASE_WINDOW: u64 = 8192` | 倒读初始窗口 |
| `logger.rs:443` | pub fn | `flush()` | 发 `Flush` 并等 ack，超时 5s |
| `logger.rs:453` | pub fn | `flush_quick()` | panic hook 专用，ack 超时 500ms |
| `logger.rs:462` | pub fn | `shutdown()` | swap 掉 sender、发 `Shutdown`、join 线程（500ms 超时） |
| `logger.rs:479` | pub fn | `clear_logs(&AppHandle) -> Result<(), String>` | 先删日志目录内全部文件，再换新 sender + 新线程（顺序注释见 `logger.rs:482`） |

### infra/events.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `events.rs:8-10` | pub struct | `EventBus<'a>` | 事件发射器，仅持有 `&AppHandle` |
| `events.rs:13` | pub fn | `EventBus::new(&AppHandle)` | 构造 |
| `events.rs:17-19` | 私有 fn | `emit<S: Serialize + Clone>(&self, &str, S) -> Result<(), String>` | 唯一底层发射，错误转 `String` |
| `events.rs:22` | pub fn | `emit_login_log(&self, message: &str, log_type: &str)` | 事件 `login-log` |
| `events.rs:30` | pub fn | `emit_background_check_result<S>(&self, payload: S)` | 事件 `background-check-result` |
| `events.rs:35` | pub fn | `emit_auto_login_result(&self, success: bool, message: &str, skipped: bool)` | 事件 `auto-login-result` |
| `events.rs:44` | pub fn | `emit_network_quality_result<S>(&self, payload: S)` | 事件 `network-quality-result` |
| `events.rs:49` | pub fn | `emit_update_available(&self, has_update, latest_version, release_notes)` | 事件 `update-available` |
| `events.rs:58` | pub fn | `emit_update_notification_click(&self)` | 事件 `update-notification-click`（`platform/toast.rs` 的 WinRT Activated 回调调用） |
| `events.rs:63` | pub fn | `emit_adapter_details_changed<S>(&self, details: S)` | 事件 `adapter-details-changed` |
| `events.rs:68` | pub fn | `emit_disabled_adapters_changed<S>(&self, disabled: S)` | 事件 `disabled-adapters-changed` |
| `events.rs:73` | pub fn | `emit_adapter_disabled_warning(&self, name: &str, message: &str)` | 事件 `adapter-disabled-warning` |
| `events.rs:81` | pub fn | `emit_campus_exit_countdown(&self, minimize_delay: u64, exit_delay: u64)` | 事件 `campus-exit-countdown` |
| `events.rs:89` | pub fn | `emit_campus_exit_cancelled(&self)` | 事件 `campus-exit-cancelled` |
| `events.rs:94` | pub fn | `emit_auto_exit_countdown(&self, delay: u64, shortcut: &str)` | 事件 `auto-exit-countdown` |
| `events.rs:102` | pub fn | `emit_auto_exit_cancelled(&self)` | 事件 `auto-exit-cancelled` |
| `events.rs:107` | pub fn | `emit_config_changed<S>(&self, payload: S)` | 事件 `config-changed` |
| `events.rs:112` | pub fn | `emit_adapters_changed<S>(&self, adapters: S)` | 事件 `adapters-changed` |
| `events.rs:117` | pub fn | `emit_update_download_progress<S>(&self, payload: S)` | 事件 `update-download-progress` |
| `events.rs:122-135` | `#[cfg(test)]` mod | `tests` | 仅一个 `size_of` 编译期断言（`events.rs:131-134`），无行为测试 |

### infra/task_manager.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `task_manager.rs:8-11` | pub struct | `TaskHandle` | 单任务句柄（公开字段仅取消令牌） |
| `task_manager.rs:14-17` | pub struct（`#[derive(Clone)]`） | `BackgroundTaskManager` | 名称 → 句柄表 |
| `task_manager.rs:19-23` | impl | `Default` | 委托 `new` |
| `task_manager.rs:26` | pub fn | `new() -> Self` | 空表 |
| `task_manager.rs:33` | pub fn | `spawn<F, Fut>(&self, name: &str, build_future: F) -> Result<(), String>` | 同名已在跑则返回 `Err("任务 {name} 已在运行")`；任务结束/被取消后自动摘除 |
| `task_manager.rs:67` | pub fn | `cancel(&self, name: &str) -> bool` | 摘除并 `cancel()` 令牌 |
| `task_manager.rs:81` | pub fn | `detach(&self, name: &str) -> bool` | 只摘除不取消、不等待（任务自身即将 `shutdown_and_exit` 时防死锁） |
| `task_manager.rs:86` | pub async fn | `shutdown(&self)` | 排空表 → 全部 `cancel()` → 全部 `await` |
| `task_manager.rs:101` | pub fn | `is_running(&self, name: &str) -> bool` | 查询 |
| `task_manager.rs:106` | pub fn | `cancel_token(&self, name: &str) -> Option<Arc<CancellationToken>>` | 取令牌 |
| `task_manager.rs:111-147` | `#[cfg(test)]` mod | `tests` | 重复 spawn 拒绝 / cancel 摘除 / shutdown 等待 |

### infra/lifecycle.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `lifecycle.rs:12` | 私有 const | `CAMPUS_MINIMIZE_DELAY_MS: u64 = 30000` | 非校园网：30s 后最小化 |
| `lifecycle.rs:13` | 私有 const | `CAMPUS_EXIT_DELAY_MS: u64 = 60000` | 非校园网：60s 后退出 |
| `lifecycle.rs:17-19` | 私有 fn | `is_within_campus_exit_window(now_minutes, start, end) -> bool` | 当日分钟数判窗；`end <= start` 退化为只受 start 限制 |
| `lifecycle.rs:23` | pub fn | `start_campus_exit(&AppHandle, &AppState)` | 校园网验证不通过时启动倒计时（受 `campus_exit_on_fail` 与时间窗双重门控） |
| `lifecycle.rs:138` | pub fn | `cancel_campus_exit(&AppHandle, &AppState)` | 静默取消（不发通知/事件），由 `monitor/background_check.rs:63,136` 调用 |
| `lifecycle.rs:157` | pub fn | `cancel_campus_exit_with_notification(&AppHandle, &AppState)` | 带系统通知 + `campus-exit-cancelled` 事件的取消 |
| `lifecycle.rs:183` | pub fn | `start_auto_exit(&AppHandle, &AppState)` | 登录成功后 20s 自动退出倒计时；用户已取消过则跳过（`lifecycle.rs:185-188`） |
| `lifecycle.rs:266` | pub fn | `cancel_auto_exit_inner(&AppHandle, &AppState) -> Result<CommandResult, String>` | 取消自动退出（无 deadline 时返回 `ok_msg("无需取消")`） |
| `lifecycle.rs:294-302` | 私有 fn（`#[cfg(desktop)]`） | `try_unregister_cancel_exit_shortcut(&AppHandle, bool)` | 条件注销全局快捷键 |
| `lifecycle.rs:305-306` | 私有 fn（`#[cfg(not(desktop))]`） | `try_unregister_cancel_exit_shortcut(_, _)` | 空实现，保持调用点不变 |
| `lifecycle.rs:311` | pub async fn | `shutdown_and_exit(&AppHandle, &AppState)` | 置 `is_quitting` → 10s 上限等待 `task_manager.shutdown()` → `app_handle.exit(0)` |
| `lifecycle.rs:322-344` | `#[cfg(test)]` mod | `tests` | 时间窗判定：默认 8:00–23:00 与 `end<=start` 退化两种 |

### infra/notification.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `notification.rs:16` | pub fn | `emit_notification(&AppHandle, title: &str, body: &str, mascot: &str)` | 系统通知唯一出口；双重门控（`enable_notification` 配置 + 窗口不可见）后才异步发通知 |

### infra/command_context.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `command_context.rs:10-13` | pub struct | `CommandContext<'a>` | 命令层 AppHandle + AppState 打包 |
| `command_context.rs:16` | pub fn | `CommandContext::new(&AppHandle, &AppState)` | 显式构造 |
| `command_context.rs:22` | pub fn | `CommandContext::from_app(&AppHandle)` | 内部 `app.state::<AppState>()` 后取 `inner()` |
| `command_context.rs:30-33` | impl | `Deref for CommandContext<'a>`（`Target = AppState`） | `ctx.config` 等价 `ctx.state.config` |
| `command_context.rs:38-41` | pub trait | `AppHandleExt` | 定义 `notify_config_changed` / `notify_update_download_progress` |
| `command_context.rs:43-51` | impl | `AppHandleExt for AppHandle` | 两个方法均委托 `EventBus` |

### infra/async_util.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `async_util.rs:24-32` | 私有 fn | `fallback_runtime() -> &'static Runtime` | `OnceLock` 多线程 Runtime（`enable_all`）兜底 |
| `async_util.rs:39-44` | pub fn | `block_on_sync<F: Future>(future: F) -> F::Output` | 有 runtime context 走 `Handle::block_on`，否则走兜底 Runtime |

## 结构体与字段

### TaskLock（`state/mod.rs:16-18`）

| 字段 | 类型 | 行号 | 含义 |
| --- | --- | --- | --- |
| `flag` | `AtomicBool`（私有） | `state/mod.rs:17` | `false` 空闲 / `true` 已占用；`compare_exchange(false, true, Acquire, Relaxed)` 抢锁，`Drop` 时 `Release` 清 |

`TaskGuard<'a>`（`state/mod.rs:20-22`）唯一字段 `lock: &'a TaskLock`（`state/mod.rs:21`），是自动释放的依据；无 owner 标识、无重入（`mem::forget` 掉 guard 即永久占用）。

### TaskFlags（`state/mod.rs:74-80`）

| 字段 | 类型 | 行号 | 含义与使用点 |
| --- | --- | --- | --- |
| `is_checking` | `TaskLock` | `state/mod.rs:75` | 后台周期检测锁；`monitor/background_check.rs:18`、`commands/background.rs:30`、`commands/system.rs:163`（读 `is_active` 供前端显示） |
| `is_logging_in` | `TaskLock` | `state/mod.rs:76` | 登录锁；`commands/login.rs:67`、`monitor/auto_auth.rs:69,141,425`、`app/tray.rs:58` |
| `is_logging_out` | `TaskLock` | `state/mod.rs:77` | 注销锁；`commands/login.rs:92` |
| `is_quality_checking` | `TaskLock` | `state/mod.rs:78` | 网络质量检测锁；`commands/network_cmd.rs:179`、`monitor/quality_scheduler.rs:23` |
| `is_downloading` | `TaskLock` | `state/mod.rs:79` | 更新包下载锁；`commands/updater.rs:31` |

### UpdateStats（`state/mod.rs:86-101`）

| 字段 | 类型 | 行号 | 含义与使用点 |
| --- | --- | --- | --- |
| `last_update_check_epoch_ms` | `AtomicU64` | `state/mod.rs:87` | 上次更新检查时间；`commands/updater.rs:18`、`update/updater.rs:307` 写 |
| `update_notified` | `AtomicBool` | `state/mod.rs:88` | 本会话是否已弹过"发现新版本"（CAS 去重）；`update/updater.rs:292` |
| `last_disabled_notification_ms` | `AtomicU64` | `state/mod.rs:89` | 适配器被禁用的通知节流；`monitor/adapter_watch.rs:111,122` |
| `last_network_change_notification_ms` | `AtomicU64` | `state/mod.rs:90` | 网络状态变更通知节流（60s）；`monitor/background_emit.rs:95-96` |
| `last_render_heartbeat_ms` | `AtomicU64` | `state/mod.rs:91` | 前端心跳时间戳；写 `commands/system.rs:160`，读 `app/heartbeat.rs:38` |
| `webview_recovery_window_start_ms` | `AtomicU64` | `state/mod.rs:94` | WebView 恢复滑动窗口起点（0 = 未触发过）；`app/webview_recovery.rs:62-74` |
| `webview_recovery_count` | `AtomicU32` | `state/mod.rs:96` | 当前窗口内已 reload 次数；`app/webview_recovery.rs:66-78` |
| `auto_enable_last_attempt_ms` | `AtomicU64` | `state/mod.rs:98` | 自动启用被禁适配器的上次尝试时间；`monitor/adapter_watch.rs:164,166` |
| `auto_enable_failure_count` | `AtomicU32` | `state/mod.rs:100` | 自动启用连续失败数（退避输入）；`monitor/adapter_watch.rs:154-191` |

### AppState（`state/mod.rs:125-132`）

| 字段 | 类型 | 行号 | 含义 |
| --- | --- | --- | --- |
| `config` | `ConfigStore` | `state/mod.rs:126` | 配置快照（ArcSwap） |
| `tasks` | `TaskFlags` | `state/mod.rs:127` | 5 个任务互斥锁 |
| `task_manager` | `BackgroundTaskManager` | `state/mod.rs:128` | 后台任务表（`Clone` 后可跨线程持有） |
| `network` | `NetworkState` | `state/mod.rs:129` | 网络状态快照 |
| `exit` | `ExitStateStore` | `state/mod.rs:130` | 退出状态与截止时间 |
| `update_stats` | `UpdateStats` | `state/mod.rs:131` | 更新/通知/心跳/恢复统计 |

### ConfigStore（`state/store.rs:6-8`）

| 字段 | 类型 | 行号 | 含义 |
| --- | --- | --- | --- |
| `inner` | `ArcSwap<Config>`（私有） | `state/store.rs:7` | 配置的原子指针容器；`update` 用 `compare_and_swap` + `Arc::ptr_eq` 判胜出（`state/store.rs:44-48`） |

### NetworkSnapshot（`state/network.rs:7-26`）

| 字段 | 类型 | 行号 | 含义 |
| --- | --- | --- | --- |
| `server_available` | `bool` | `state/network.rs:8` | 校园网服务端是否可达 |
| `any_adapter_online` | `bool` | `state/network.rs:9` | 是否有适配器在线（前端 `render_heartbeat` 读它） |
| `last_a1_online` | `bool` | `state/network.rs:10` | 适配器 1 上次在线状态 |
| `last_a2_online` | `bool` | `state/network.rs:11` | 适配器 2 上次在线状态 |
| `has_logged_online` | `bool` | `state/network.rs:12` | 本会话是否曾登录成功 |
| `disconnect_reconnect_count` | `u32` | `state/network.rs:13` | 断线重连已试次数 |
| `background_check_count` | `u32` | `state/network.rs:14` | 后台检测累计次数 |
| `last_auto_login_attempt` | `Instant` | `state/network.rs:15` | 上次自动登录尝试时刻（默认 `Instant::now()`，`state/network.rs:39`） |
| `last_network_quality` | `Option<String>` | `state/network.rs:16` | 上次网络质量等级 |
| `current_ssid` | `Option<String>` | `state/network.rs:17` | 当前 WiFi SSID |
| `on_campus_network` | `bool` | `state/network.rs:18` | 是否在校园网 |
| `logout_protected_until` | `Instant` | `state/network.rs:19` | 注销保护截止时刻（默认 `Instant::now()`，`state/network.rs:43`） |
| `portal_failure_count` | `u32` | `state/network.rs:20` | Portal 失败计数 |
| `a1_auth_failure_count` | `u32` | `state/network.rs:21` | 适配器 1 认证失败计数 |
| `a2_auth_failure_count` | `u32` | `state/network.rs:22` | 适配器 2 认证失败计数 |
| `prep_login_failures` | `u32` | `state/network.rs:25` | 准备自动登录连续失败数（会话内，成功/重置清零；达上限停自动登录） |

`NetworkState`（`state/network.rs:53-55`）唯一字段 `snapshot: ArcSwap<NetworkSnapshot>`（私有，`state/network.rs:54`）。

### ExitStateStore（`state/exit.rs:7-13`）

| 字段 | 类型 | 行号 | 含义 |
| --- | --- | --- | --- |
| `is_quitting` | `Arc<AtomicBool>` | `state/exit.rs:8` | 退出中标志（`shutdown_and_exit` 置位；`lifecycle.rs:87` 用它跳过最小化） |
| `auto_exit_deadline` | `Mutex<Option<Instant>>` | `state/exit.rs:9` | 自动退出截止时间；同时充当 `start_auto_exit` 的"是否已在跑"判据（`lifecycle.rs:189-198`）与快捷键 TOCTOU 临界区（`lifecycle.rs:121,150,172`） |
| `auto_exit_cancelled` | `AtomicBool` | `state/exit.rs:10` | 用户已取消自动退出（`lifecycle.rs:185` 读、`274` 写、`194` 复位） |
| `campus_exit_started` | `AtomicBool` | `state/exit.rs:11` | 校园网退出是否已启动（CAS 防重复触发，`lifecycle.rs:51,131,139,158`） |
| `campus_exit_deadline` | `Mutex<Option<Instant>>` | `state/exit.rs:12` | 校园网退出截止时间（任务体二次/最终校验，`lifecycle.rs:86,104`） |

### TaskHandle / BackgroundTaskManager（`task_manager.rs:8-17`）

| 结构体 | 字段 | 类型 | 行号 | 含义 |
| --- | --- | --- | --- | --- |
| `TaskHandle` | `cancel_token` | `Arc<CancellationToken>` | `task_manager.rs:9` | pub，取消用 |
| `TaskHandle` | `join_handle` | `tauri::async_runtime::JoinHandle<()>`（私有） | `task_manager.rs:10` | `shutdown` 时 `await` 用 |
| `BackgroundTaskManager` | `inner` | `Arc<Mutex<HashMap<String, TaskHandle>>>`（私有） | `task_manager.rs:16` | 任务名 → 句柄；`Clone` 共享同一表 |

### logger 内部类型

| 结构体/枚举 | 字段/变体 | 类型 | 行号 | 含义 |
| --- | --- | --- | --- | --- |
| `LogLevel` | `Debug` / `Info` / `Warn` / `Error` | — | `logger.rs:28-31` | 派生 `PartialOrd/Ord`，`logger.rs:305` 用它做阈值过滤；默认阈值 `Info`（`logger.rs:62`） |
| `LogMessage` | `Entry { line: String }` | — | `logger.rs:46` | 待落盘的一整行（已含时间戳/级别/模块） |
| `LogMessage` | `Flush { ack: Sender<()> }` | — | `logger.rs:47` | 要求立即 flush 并回 ack |
| `LogMessage` | `Shutdown` | — | `logger.rs:48` | 让 worker 排空后退出线程 |
| `LoggerState` | `log_dir` | `PathBuf` | `logger.rs:52` | 日志目录 |
| `LoggerState` | `current_writer` | `Option<BufWriter<File>>` | `logger.rs:53` | `None` = 打不开（后续 `flush_messages` 走丢弃分支，`logger.rs:204-210`） |
| `LoggerState` | `current_date` | `String` | `logger.rs:54` | 当前文件日期，跨天比对换文件 |
| `LoggerState` | `last_flush` | `Instant` | `logger.rs:55` | 上次落盘时刻 |

### EventBus / CommandContext

| 结构体 | 字段 | 类型 | 行号 | 含义 |
| --- | --- | --- | --- | --- |
| `EventBus<'a>` | `app_handle` | `&'a AppHandle`（私有） | `events.rs:9` | 仅持引用；`events.rs:133` 编译期断言其大小等于 `&AppHandle` |
| `CommandContext<'a>` | `app` | `&'a AppHandle` | `command_context.rs:11` | pub |
| `CommandContext<'a>` | `state` | `&'a AppState` | `command_context.rs:12` | pub，`Deref` 目标 |

### 返回体结构（`state/mod.rs`）

| 结构体 | 字段 | 类型 | 行号 | 含义 |
| --- | --- | --- | --- | --- |
| `CommandResult` | `success` | `bool` | `state/mod.rs:214` | 恒序列化 |
| `CommandResult` | `message` | `Option<String>` | `state/mod.rs:216` | `skip_serializing_if = "Option::is_none"` |
| `CommandResult` | `data` | `Option<serde_json::Value>` | `state/mod.rs:218` | 同上 |
| `AccountResult` | `success` | `bool` | `state/mod.rs:236` | 恒序列化 |
| `AccountResult` | `message` | `Option<String>` | `state/mod.rs:238` | 可选 |
| `AccountResult` | `active_account` | `Option<String>` | `state/mod.rs:240` | 当前激活账号名 |
| `AccountResult` | `config` | `Option<Config>` | `state/mod.rs:242` | 变更后的配置（注意：由调用方负责掩码，见 `desktop-config`） |

## Data Flow

### 状态进入/变换/离开

- 装配入口：`app/startup.rs:50` `AppState::new()` → `app/startup.rs:174` `CommandContext::from_app` → `app/startup.rs:176` `state.config.store(config.clone())` 把磁盘配置灌入内存。
- 配置变换统一走 CAS：`ConfigStore::update`（`state/store.rs:35-49`）→ `ArcSwap::compare_and_swap` → 胜出后返回新快照；读路径 `load()`（`state/store.rs:18`）。
- 网络状态变换走 `NetworkState::update_with_result`（`state/network.rs:96-111`），"读-改-判定"在同一 CAS 循环内，避免基于旧快照做决策。
- 离开路径：仅通过 `EventBus` / `#[tauri::command]` 返回值 / `emit_notification` 三条出口到前端或系统。

### 日志链路

```text
log_info!/log_warn!/... (logger.rs:347-373)
  → logger::log(level, module, message) (logger.rs:304)  —— MIN_LOG_LEVEL 过滤 (logger.rs:305)
  → LOGGER_SENDER.send(LogMessage::Entry) (logger.rs:315)
  → mpsc channel（容量无界，worker 侧 CHANNEL_CAPACITY=1024 控制单轮排空）
  → logger-worker 线程 logger_worker (logger.rs:118)
      recv_timeout(2s) → push buffer → drain_channel(:151)
      → 条件落盘 flush_messages(:158-161)：含 Flush 消息 / ≥32 条 / 距上次 ≥2s
      → rotate_if_needed(:188)：跨天换文件 / 超 5MB 轮转 + cleanup_old_logs(:237)
      → BufWriter.write_all + flush (logger.rs:194,203)
  超时分支每 3600s 调 cleanup_old_logs_by_time(:145)
```

收尾链路：`main.rs:65-66` `flush()` → `shutdown()`（`logger.rs:443,462`）；panic 走 `main.rs:20-24` 的 hook → `flush_quick()`（`logger.rs:453`）。前端读日志经 `read_recent_logs`（`logger.rs:399`），清日志经 `clear_logs`（`logger.rs:479`）。

### 事件总线（全部 16 个事件）

所有桌面端 emit 都收敛在 `events.rs` 内——除 `events.rs:18` 外，全仓不存在其他 `.emit(` 调用（`grep -rn "\.emit(" src` 仅命中 `events.rs`）。前端消费点在 `tauri-app/frontend/src/hooks/tauriApi.ts`（监听装配在 `tauri-app/frontend/src/hooks/useEventListeners.ts`）。

| 事件名 | payload 字段 | 发出点 | 前端消费点 |
| --- | --- | --- | --- |
| `login-log` | `message`, `type` | `events.rs:22-27` | `tauriApi.ts:159` |
| `background-check-result` | 由调用方传入（`BackgroundStatus` 类） | `events.rs:30-32` | `tauriApi.ts:153` |
| `auto-login-result` | `success`, `message`, `skipped` | `events.rs:35-41` | `tauriApi.ts:154` |
| `network-quality-result` | 由调用方传入（`NetworkQuality`） | `events.rs:44-46` | `tauriApi.ts:173` |
| `update-available` | `hasUpdate`, `latestVersion`, `releaseNotes` | `events.rs:49-55` | `tauriApi.ts:215` |
| `update-notification-click` | `{}` | `events.rs:58-60`（`platform/toast.rs` 调用） | `tauriApi.ts:216` |
| `adapter-details-changed` | 由调用方传入（`AdapterDetail[]`） | `events.rs:63-65` | `tauriApi.ts:156` |
| `disabled-adapters-changed` | 由调用方传入（禁用适配器列表） | `events.rs:68-70` | `tauriApi.ts:157` |
| `adapter-disabled-warning` | `name`, `message` | `events.rs:73-78` | `tauriApi.ts:158` |
| `campus-exit-countdown` | `minimizeDelay`, `exitDelay` | `lifecycle.rs:59` | `tauriApi.ts:201` |
| `campus-exit-cancelled` | `{}` | `lifecycle.rs:178` | `tauriApi.ts:202` |
| `auto-exit-countdown` | `delay`, `shortcut` | `lifecycle.rs:204` | `tauriApi.ts:199` |
| `auto-exit-cancelled` | `{}` | `lifecycle.rs:285` | `tauriApi.ts:200` |
| `config-changed` | 由调用方传入（含完整 config） | `events.rs:107-109`（`command_context.rs:45` 是其中一条转发） | `tauriApi.ts:203` |
| `adapters-changed` | 由调用方传入（`Adapter[]`） | `events.rs:112-114` | `tauriApi.ts:155` |
| `update-download-progress` | 由调用方传入（`DownloadProgress`） | `events.rs:117-119`（`command_context.rs:49` 转发） | `tauriApi.ts:214` |

### 退出生命周期状态机

自动退出（登录成功后）：

```text
auth/monitor 调用 start_auto_exit (lifecycle.rs:183)
  → auto_exit_cancelled 为真则 return (:185)
  → 锁 auto_exit_deadline：Some 则 return；None 则置 now+20s (:189-198)
  → emit auto-exit-countdown (:204) + 系统通知 (:208)
  → 注册 Ctrl+Shift+C (:210-218)
  → task_manager.spawn("auto_exit", ...) (:228)
        sleep(deadline-now) / cancel 二选一 (:237-240)
        → 二次校验 deadline 是否到期 (:242-251)
        → 仅在 campus_exit 未启动时注销快捷键 (:253-254)
        → detach("auto_exit") (:256) —— 防止 shutdown 等自己
        → shutdown_and_exit (:257)
  → spawn 失败则回滚 deadline (:261)
```

取消：`commands/system.rs:95` / `app/shortcut.rs:29` → `cancel_auto_exit_inner`（`lifecycle.rs:266`）→ 清 deadline、置 `auto_exit_cancelled`、`cancel("auto_exit")`、注销快捷键、通知 + `auto-exit-cancelled`。

非校园网退出：`monitor/background_check.rs:129`、`monitor/auto_auth.rs:326` → `start_campus_exit`（`lifecycle.rs:23`）→ 配置门控（`:25`）→ 时间窗门控（`:32`）→ `campus_exit_started` CAS（`:51`）→ 写 deadline（`:54`）→ 事件 + 通知 + 注册快捷键 → `spawn("campus_exit")`（`:78`）→ 30s 后 `window.hide()`（`:91-94`）→ 再 30s → deadline 终校验（`:104-113`）→ `detach` + `shutdown_and_exit`。

统一退出：`lifecycle.rs:311-320` `shutdown_and_exit` → `is_quitting=true` → `tokio::time::timeout(10s, task_manager.shutdown())` → `app_handle.exit(0)`。

### 通知链路

```text
emit_notification(app, title, body, mascot) (notification.rs:16)
  → 配置门控：config.enable_notification 为 false 直接 return (:17-23)
  → 可见性门控：窗口 is_visible && !is_minimized → return（视为用户能看到界面）(:29-34)
  → spawn 异步 (:40)
      Windows 桌面：platform::toast::show_system_toast (mascot 作为圆形头像) (:46)
        失败 → tauri_plugin_notification 纯文本降级 (:50-53)
      其他平台：直接用 tauri_plugin_notification（mascot 被丢弃）(:56-63)
```

### 扩展点（改动前必读）

- **新增事件**：只允许在 `infra/events.rs:117` 之后追加 `emit_xxx` 方法，并在 `tauri-app/frontend/src/hooks/tauriApi.ts:153-216` 与 `useEventListeners.ts` 增加监听；不要绕过 `EventBus` 直接 `emit`（当前全仓无此先例）。
- **新增 `#[tauri::command]`**：命令体写在 `tauri-app/src-tauri/src/commands/` 下，函数上标 `#[tauri::command]`，然后必须加入 `tauri-app/src-tauri/src/app/startup.rs:63-120` 的 `invoke_handler(tauri::generate_handler![...])`；安卓端命令是独立注册（`android/src-tauri/src/lib.rs`），必须双端各注册一次。
- **新增后台周期任务**：`state.task_manager.spawn("唯一名字", move |cancel_token| async move { ... })`，参考 `app/heartbeat.rs:11-58`；任务名重复会返回 `Err`；`tokio::select!` 里必须处理 `cancel_token.cancelled()`，否则退出时会等满 10s 上限。
- **读取全局状态**：命令里用 `CommandContext::from_app(&app_handle)`（`command_context.rs:22`）或 `app_handle.state::<AppState>()`；两者都指向同一个 `.manage` 实例。
- **新增配置字段**：改 `config/model.rs` 后需同步 `ConfigStore::update` 调用方、i18n 与双端（见 `desktop-config` 与 `AGENTS.md` 第 3 条）。

## Connections

- [[desktop-app-lifecycle]]：`infra::state::AppState` 由 `app/startup.rs:50` 托管；`lifecycle::shutdown_and_exit` 由 `app/shutdown.rs` 调用；`app/heartbeat.rs` 写 `UpdateStats::last_render_heartbeat_ms` 并复用 `BackgroundTaskManager`。
- [[desktop-config]]：`ConfigStore` 承载 `config::model::Config`；`Config::masked_for_display` 决定 `AccountResult.config` 的掩码语义。
- [[desktop-auth]]：登录/注销命令拿 `TaskFlags::{is_logging_in, is_logging_out}`；`auth/service.rs:246`、`monitor/auto_auth.rs:461` 触发 `start_auto_exit`。
- [[desktop-monitor]]：`start_campus_exit` / `cancel_campus_exit` 的主要调用方（`monitor/background_check.rs`、`monitor/auto_auth.rs`）；`UpdateStats` 的节流字段在 `monitor/adapter_watch.rs`、`monitor/background_emit.rs` 使用。
- [[desktop-helper-update]]：`update/updater.rs:292,307` 用 `update_notified` / `last_update_check_epoch_ms`；`TaskFlags::is_downloading` 由 `commands/updater.rs:31` 占用。
- [[desktop-network-core]]：`NetworkState` / `NetworkSnapshot` 由网络模块读写。
- [[desktop-network-quality]]：`TaskFlags::is_quality_checking` 与 `network-quality-result` 事件。
- [[desktop-account-selfservice]]：`AccountResult`、`validate_account_name` 服务于账号与自助服务命令。
- [[desktop-commands]]：`CommandContext`、`CommandResult`、`AppHandleExt` 是命令层通用设施。
- [[desktop-platform]]：`notification.rs:46` 依赖 `platform::toast`；`logger.rs:375-397` 与平台目录策略相关。
- [[android-backend]]：安卓端仅复用 `infra::logger`（`android/src-tauri/src/lib.rs:48`、`system_cmds.rs:33-63`），不使用 `AppState` / `EventBus` / `BackgroundTaskManager`。
- [[desktop-frontend-hooks]]：事件消费点 `tauriApi.ts:153-216` 与 `useEventListeners.ts`。

## Known Issues

1. **日志按天清理可能长期不执行**：`cleanup_old_logs_by_time` 只在 `logger_worker` 的 `recv_timeout` **超时分支**被检查（`logger.rs:142-148`）。日志持续写入（间隔 < 2s）时该分支永不进入，`CLEANUP_INTERVAL`（`logger.rs:121`）形同虚设，只能靠 5MB 轮转路径的 `cleanup_old_logs`（`logger.rs:237`）做文件数控制。
2. **`clear_logs` 会删除日志目录下的全部文件**，不筛选扩展名（`logger.rs:485-489` 对 `read_dir` 的每个 entry 都 `remove_file`）；只有当轮转/清理路径才按 `extension() == "log"` 过滤（`logger.rs:262-264`）。
3. **`clear_logs` 新建的 `LoggerState` 的 `current_date` 是空串、`current_writer` 是 `None`**（`logger.rs:498-499`），首次写入依赖 `rotate_if_needed` 在 `today != ""` 时创建文件（`logger.rs:216-224`）；这中间的 `write_all` 分支不会执行，但与注释声明的删除顺序（`logger.rs:482`）耦合，改动时需一并考虑。
4. **`LOG_RETENTION_DAYS` 是进程级 static**（`logger.rs:16`），不属于 `AppState`，多实例/测试并发时共享；`set_log_retention_days` 无同步，写入可见性仅靠 `Relaxed`（`logger.rs:19`）。
5. **保留天数超过 `SystemTime` 可表示范围时跳过清理**（`logger.rs:282-290`），`retention_days == 0` 表示永久保留（`logger.rs:279-281`）。
6. **`TaskLock` 无 owner、不可重入**（`state/mod.rs:35-41`）：`try_acquire` 返回 `None` 只表示"忙"，调用方必须自己决定提示还是静默（`app/tray.rs:60-68` 做了提示，`monitor/quality_scheduler.rs:23` 为静默跳过）。`force_release` 仅在 `#[cfg(test)]` 下存在（`state/mod.rs:48-51`）。
7. **`start_campus_exit` 的 CAS 与 `set_deadline` 之间存在窗口**（`lifecycle.rs:51-54`）：注释自述若在该窗口内发生取消，会出现"一次取消无效"；这是用极小概率的失效窗口换取"避免确定性永久卡死"的历史修复取舍。
8. **`spawn("campus_exit")` 失败时的回滚依赖显式语句**（`lifecycle.rs:130-133`）：若后续新增提前 return 分支而漏写回滚，`campus_exit_started` 会永久为 `true` 并让后续调用全部短路（注释已标注为"历史缺陷"）。
9. **`cancel_campus_exit` 与 `cancel_campus_exit_with_notification` 行为不一致**（`lifecycle.rs:138` vs `:157`）：前者不发通知/事件，后者发。调用方必须按需要选择，误用会导致前端倒计时界面不消失。
10. **快捷键注销依赖 `GuardedMutexGuard` 的临界区**（`lifecycle.rs:121-123`、`150-152`、`172-174`）：`is_none()` 检查与 `unregister` 必须同锁，若把 `guard` 提前 drop 会重新引入 TOCTOU（注释已明确）。
11. **`block_on_sync` 在异步上下文中会 panic/死锁**（`async_util.rs:36-38` 注释）：`Ok` 分支走 `Handle::block_on`，调用者必须经 `spawn_blocking` 或在同步线程调用。
12. **`block_on_sync` 的兜底 Runtime 一旦初始化即常驻**（`async_util.rs:24-32`）：`OnceLock<Runtime>` 不会关闭，进程退出时由 `Runtime` 的 Drop 收尾。
13. **`emit_notification` 的 `mascot` 参数在非 Windows 桌面被丢弃**（`notification.rs:57-58` 的 `let _ = &mascot;`），安卓端看板娘大图改由 `monitor_loop::notify_system` 的 `large_icon` 负责（`notification.rs:44`）。
14. **`emit_notification` 已不再向前端发事件**（`notification.rs:11-14` 注释）：应用内 toast 与日志由各业务事件负责，双通道重复是已根除的历史缺陷，不要在本函数内补 emit。
15. **`EventBus` 无单元测试**（`events.rs:126-128` 注释）：只有 `size_of` 断言（`events.rs:131-134`），事件名/payload 契约靠人工核对，改名不会触发测试失败。
16. **`shutdown_and_exit` 的 10s 上限可能截断后台任务**（`lifecycle.rs:315-317`）：超时后仅记 WARN 就 `exit(0)`；DHCP 子进程、长 HTTP 请求可能被留下。
17. **`logger::flush()` 的 5s 超时静默返回**（`logger.rs:448`），日志线程卡死时调用方无法感知，`main.rs:65` 的收尾会继续往下走。

