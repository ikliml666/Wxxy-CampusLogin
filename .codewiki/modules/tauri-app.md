---
title: tauri-app
type: module
source_files:
  - tauri-app/src-tauri/Cargo.toml
  - tauri-app/src-tauri/src/main.rs
  - tauri-app/src-tauri/src/lib.rs
  - tauri-app/src-tauri/tauri.conf.json
  - tauri-app/src-tauri/src/app/mod.rs
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/infra/mod.rs
  - tauri-app/src-tauri/src/infra/state/mod.rs
  - tauri-app/src-tauri/src/infra/state/store.rs
  - tauri-app/src-tauri/src/infra/command_context.rs
  - tauri-app/src-tauri/src/infra/events.rs
  - tauri-app/src-tauri/src/infra/lifecycle.rs
  - tauri-app/src-tauri/src/commands/mod.rs
  - tauri-app/src-tauri/src/commands/login.rs
  - tauri-app/src-tauri/src/commands/account.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
  - tauri-app/src-tauri/src/config/mod.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/validate.rs
  - tauri-app/src-tauri/src/config/persist.rs
  - tauri-app/src-tauri/src/auth/mod.rs
  - tauri-app/src-tauri/src/auth/service.rs
  - tauri-app/src-tauri/src/monitor/mod.rs
  - tauri-app/src-tauri/src/monitor/watcher.rs
  - tauri-app/src-tauri/src/network/mod.rs
  - tauri-app/src-tauri/src/network/client.rs
  - tauri-app/src-tauri/src/account/crypto.rs
  - tauri-app/src-tauri/src/update/updater.rs
  - tauri-app/src-tauri/src/vpn/mod.rs
  - tauri-app/src-tauri/src/platform/mod.rs
tags: [tauri, desktop, rust, vpn, network-auth, campus-login]
---

## Overview

`tauri-app` 是「校园网登录助手」（campus-login，包名 `com.campus.login`，版本 `2.4.3`，见 `tauri-app/src-tauri/tauri.conf.json` 第 2–4 行）的桌面应用工程，采用 Tauri v2 框架，分两层：

- **Rust 后端**（`tauri-app/src-tauri/`）：核心业务逻辑，跨平台（Windows 桌面 + Android，见 `lib.rs` 第 11–21 行按 `#[cfg(desktop)]` 门控桌面专属模块）。还额外提供一个无头核心二进制 `campus-login-core`（`Cargo.toml` 第 16–18 行），由 SYSTEM 计划任务在开机未登录 Windows 时拉起，复用 lib 的纯函数协议层，不依赖 tauri 运行时。
- **React + TypeScript 前端**（`tauri-app/frontend/`）：WebView2 内渲染的 SPA，通过 Tauri v2 IPC 与后端通信。其结构与职责见子模块文章 [[tauri-app/frontend]]。

后端以单一 `AppState`（`infra/state/mod.rs` 第 184 行）作为进程级共享状态，`ConfigStore`（`infra/state/store.rs` 第 6 行，基于 `ArcSwap`）持有全盘配置，所有 Tauri 命令（`commands/`）通过 `CommandContext`（`infra/command_context.rs` 第 10 行）访问状态，并经 `EventBus`（`infra/events.rs` 第 8 行）向前端异步推送事件。`Config`（`config/model.rs` 第 10 行）是贯穿全栈的核心数据模型，密码经 DPAPI（`account/crypto.rs`）加密落盘，出站一律经 `masked_for_display`（`config/model.rs` 第 350 行）打码。

应用启动入口是 `main.rs` 第 17 行 `main()`：先注册 panic hook（`main.rs` 第 21 行），再在 Tauri Builder 之前拦截两种提权模式（`--helper-task` 第 31 行、`--helper` 第 38 行），设置 WebView2 浏览器参数（`main.rs` 第 57–69 行），最后构建 Tokio runtime（`app/startup.rs` 第 6 行 `build_runtime`）并调用 `app::startup::run`（`app/startup.rs` 第 23 行）。

WebVPN 协议核心（深澜 Srun）作为独立子模块单独成文，见 [[tauri-app/vpn]]。

## Key Components

下列组件均来自本文引用过的源文件（见 `source_files`）。行号以 `read_code` 返回的「行号| 」前缀为准。

### 进程入口与运行时

- `main()` — `tauri-app/src-tauri/src/main.rs:17`：应用主入口。顺序：① 注册 panic hook（`main.rs:21`，写日志并 `flush_quick`）；② 拦截 `--helper-task` 计划任务 worker 模式（`main.rs:31`，优先于 `--helper`）；③ 解析 `--helper` 提权参数（`main.rs:38`，非法即 `exit(2)`）；④ Windows 下 RTSS hook 预检（`main.rs:52`）；⑤ 构建 WebView2 浏览器参数并写入 env（`main.rs:57`–`69`）；⑥ 构建 Tokio runtime 并 `app::startup::run`（`main.rs:75`、`:78`）。
- `app::startup::build_runtime(core_count)` — `tauri-app/src-tauri/src/app/startup.rs:7`：构建多线程 Tokio runtime，`worker_threads` clamp 到 2–8（`startup.rs:8`），`max_blocking_threads` clamp 到 8–64（`:8`）。
- `app::startup::run(core_count)` — `app/startup.rs:23`：Tauri Builder 装配全部插件（`shell`/`notification`/`autostart`/`global_shortcut`/`single_instance`，`:25`–`:48`）、`manage(AppState::new())`（`:49`）、`setup`（`:50` → `setup_app`）、窗口事件处理（`:53`–`:65`）、`generate_handler!` 注册全部前端命令（`:66`–`:137`）、`app.run` 拦截退出（`:144`–`:158`，轻量化常驻逻辑）。
- `app::startup::setup_app(app, core_count)` — `app/startup.rs:161`：创建数据/日志目录（`:165`、`:175`–`:186`）、初始化 logger 并加载配置（`:190`–`:195`）、应用日志保留天数（`:221`）、静默启动判定（`:226`）、构建托盘（`:233`）、启动适配器监听/`adapter_cache` 刷新/`update` 检查循环/启动期清理（`:236`–`:248`）、WebView2 运行时版本记录与崩溃订阅（`:252`–`:254`）、心跳线程（`:256`–`:257`）、GPU 预热线程（`:261`–`:269`）。
- `lib.rs` 模块树 — `tauri-app/src-tauri/src/lib.rs`，桌面专属模块（`app`/`commands`/`helper`/`monitor`/`update`）以 `#[cfg(desktop)]` 门控（`:12`–`:21`）。

### 共享状态层 `infra/state/`

- `AUTO_EXIT_DELAY_MS: u64 = 20000` — `infra/state/mod.rs:13`（自动退出默认延迟）。
- `CANCEL_EXIT_SHORTCUT: &str = "CommandOrControl+Shift+C"` — `infra/state/mod.rs:14`。
- `TaskLock` — `infra/state/mod.rs:16`：基于 `AtomicBool` 的互斥锁；方法 `new`（`:31`）、`try_acquire`（`:35`，CAS 获取 `TaskGuard`）、`is_active`（`:43`）、`force_release`（`:49`，仅测试）。
- `TaskGuard<'a>` — `infra/state/mod.rs:20`；`Drop`（`:54`）释放锁。
- `ACCOUNT_NAME_RE` 与 `validate_account_name(name)` — `infra/state/mod.rs:61`/`:64`（账号文件名白名单，长度 1–32，允许字母/数字/下划线/中文/连字符）。
- `sanitize_account_id(user)` — `infra/state/mod.rs:77`：R2 自动建号 id 生成（非法字符替换 `_`、截断 32）。
- `TaskFlags` — `infra/state/mod.rs:90`：5 个 `TaskLock`（`is_checking`/`is_logging_in`/`is_logging_out`/`is_quality_checking`/`is_downloading`）。
- `ScheduledFired` — `infra/state/mod.rs:101`：定时动作的当日触发标记，`AtomicI32` 字段 `login_day`（`:102`）、`logout_day`（`:103`）、`login_retry_day`（`:108`）、`login_retry_count`（`:109`）；方法 `new`（`:119`）、`bump_login_retry(day)`（`:129`）、`reset_login_retry`（`:138`）。
- `UpdateStats` — `infra/state/mod.rs:147`：更新与通知统计原子字段（`update_notified`/`last_disabled_notification_ms`/`last_network_change_notification_ms`/`last_render_heartbeat_ms`/`webview_recovery_window_start_ms`/`webview_recovery_count`/`auto_enable_last_attempt_ms`/`auto_enable_failure_count`）；方法 `new`（`:170`）。
- `AppState` — `infra/state/mod.rs:184`：进程级状态聚合。字段：`config: ConfigStore`（`:185`）、`tasks: TaskFlags`（`:186`）、`task_manager: BackgroundTaskManager`（`:187`）、`network: NetworkState`（`:188`）、`exit: ExitStateStore`（`:189`）、`update_stats: UpdateStats`（`:190`）、`scheduled: ScheduledFired`（`:191`）；`new()`（`:201`）。
- `CommandResult` — `infra/state/mod.rs:292`：命令统一返回 `{ success, message?, data? }`，`serde(skip_serializing_if = "Option::is_none")`；方法 `ok`（`:301`）、`ok_msg`（`:304`）、`err`（`:307`）。
- `AccountItem` — `infra/state/mod.rs:314`：`{ id, displayName, user, operator, passwordSaved }`，密码只出布尔。
- `AccountResult` — `infra/state/mod.rs:331`：`{ success, message?, activeAccount?, displayName?, config? }`；方法 `ok`（`:344`）、`ok_with_account`（`:347`）、`err`（`:350`）。
- `ConfigStore` — `infra/state/store.rs:6`：基于 `ArcSwap<Config>` 的配置存储；方法 `new`（`:11`）、`load`（`:18`）、`load_full`（`:23`，`load` 别名）、`store`（`:28`）、`update`（`:35`，CAS 循环原子更新，避免 TOCTOU）。

### 命令上下文与事件总线 `infra/`

- `CommandContext<'a>` — `infra/command_context.rs:10`：`{ app, state }`；`new`（`:16`）、`from_app`（`:22`，自动取 `AppState`）；`Deref`（`:30`）使其可像 `AppState` 一样访问字段。
- `AppHandleExt` trait — `infra/command_context.rs:38`：`notify_config_changed`（`:44`）、`notify_update_download_progress`（`:48`），均委托 `EventBus`。
- `EventBus<'a>` — `infra/events.rs:8`：封装所有 `app_handle.emit`；`new`（`:13`）、`emit`（`:17`）；事件方法：`emit_login_log`（`:22`）、`emit_background_check_result`（`:30`）、`emit_scheduled_action_skipped`（`:36`）、`emit_mac_reset_cycle`（`:41`）、`emit_auto_login_result`（`:46`）、`emit_network_quality_result`（`:55`）、`emit_update_available`（`:59`）等。
- `infra::local_now_minutes()` — `infra/mod.rs:13`：当日分钟数（0–1439），静默窗/时间窗共用。
- `lifecycle::start_campus_exit(app_handle, state)` — `infra/lifecycle.rs:23`：校园网验证失败时 30s 最小化 + 60s 退出（`CAMPUS_MINIMIZE_DELAY_MS` `:12`、`CAMPUS_EXIT_DELAY_MS` `:13`）；`is_within_campus_exit_window`（`:17`）。`AUTO_EXIT_DELAY_MS` 等也由此模块引用。

### 配置模型 `config/`

- `PASSWORD_MASK: &str = "***"` — `config/model.rs:3`；`AUTO_DETECT_ADAPTER: &str = "自动检测"` — `config/model.rs:4`。
- `Config` — `config/model.rs:10`（`#[serde(default)]` 容器级容错）。全部字段（含行号）如下：
  - `user`（`:11`）、`password`（`:13`，默认空）、`self_hello_enabled`（`:17`，默认 true）、`self_reverify_each_action`（`:21`）、`operator`（`:22`）、`adapter1`（`:23`）、`adapter2`（`:24`）、`adapter1_account`（`:27`，设备级主适配器绑定账号）、`adapter2_account`（`:30`，副适配器绑定账号）、`dual_adapter`（`:33`）、`auto_login_on_start`（`:35`）、`auto_exit_after_login`（`:37`）、`minimize_to_tray`（`:39`）、`lightweight_mode`（`:43`，默认 true）、`hidden_start`（`:45`）、`auto_launch`（`:47`）、`enable_background_check`（`:49`）、`background_check_interval`（`:51`）、`auto_login_on_preparation`（`:53`）、`enable_night_operator_switch`（`:57`）、`night_operator_restore`（`:60`）、`enable_night_outbound_switch`（`:64`）、`night_restore_minutes`（`:70`）、`outbound_priority`（`:76`）、`outbound_metric_restore`（`:81`）、`outbound_disabled_adapters`（`:87`）、`outbound_standby_route`（`:92`）、`outbound_manual_hold_day`（`:99`，`i32`）、`outbound_pending_switch`（`:106`）、`night_outbound_restore`（`:110`，安卓切换态标记）、`dns_optimize_adapters`（`:115`）、`device_aliases`（`:120`，`HashMap<String,String>`）、`auto_exit_on_online`（`:122`）、`theme_mode`（`:124`）、`enable_notification`（`:126`）、`active_account`（`:128`）、`display_name`（`:132`）、`enable_latency_test`（`:134`）、`latency_test_interval`（`:136`）、`custom_theme_color`（`:138`）、`default_panel`（`:140`）、`enable_network_quality`（`:142`）、`skip_ttfb_in_latency`（`:144`，默认 true）、`skip_content_in_latency`（`:146`，默认 true）、`portal_url`（`:148`）、`fixed_gateway`（`:150`）、`required_network_name`（`:152`）、`enable_network_name_check`（`:154`，默认 true）、`campus_gateway`（`:156`）、`update_source`（`:159`，`"mirror"`/`"github"`）、`campus_exit_on_fail`（`:161`，默认 true）、`campus_exit_start_minutes`（`:164`，默认 480）、`campus_exit_end_minutes`（`:167`，默认 1380）、`campus_check_start_minutes`（`:169`，默认 460）、`campus_check_end_minutes`（`:172`，默认 1380）、`scheduled_login_minutes`（`:175`，默认 1440=禁用哨兵）、`scheduled_logout_minutes`（`:178`，默认 1440）、`log_retention_days`（`:180`，默认 7）、`max_disconnect_reconnect`（`:182`，默认 3）、`auto_login_cooldown_secs`（`:184`，默认 60）、`skip_sha256_when_missing`（`:186`）、`vpn_account`（`:189`）、`vpn_password_enc`（`:193`，DPAPI 密文）、`self_service_via_vpn`（`:197`，默认 true）、`config_version`（`:199`，默认 5）。
  - 默认实现 `impl Default`（`:262`）；出站掩码 `masked_for_display`（`:350`）与 `mask_in_place`（`:356`）（密码与 `vpn_password_enc` 非空即替换为 `PASSWORD_MASK`）。
- `config/validate.rs`：校验器 `USERNAME_RE`（`:6`）、`CUSTOM_COLOR_RE`（`:7`）；`validate_username`（`:10`）、`validate_operator`（`:23`，允许 `""/@telecom/@unicom/@cmcc`）、`validate_password`（`:31`）、`validate_portal_url`（`:42`，仅内网 IP/localhost 的 http/https）。
- `config/persist.rs`：持久化层（见下「命令/持久化」）。

### 命令层 `commands/`

- `commands/login.rs`：
  - `AdapterOnlineStatus`（`login.rs:9`：`any_online`/`a1_online`/`a2_online`/`probe_ok`）。
  - `check_any_adapter_online(state)`（`login.rs:18`）：并行探测双适配器 Portal 状态（`:40` `std::thread::scope`），`probe_ok` 标志离线结论是否可信（审计 A8）。
  - `do_login(...)`（`login.rs:63`）：`@tauri::command`，经 `spawn_blocking` 调 `auth::service::full_login`（`:80`），成功后 `post_login_handler`（`:85`）。
  - `do_logout(...)`（`login.rs:92`）：`@tauri::command`，调 `perform_full_logout_sync`。
  - `perform_full_logout_sync(app_handle, adapter_name)`（`login.rs:105`）：注销核心（命令与托盘菜单共用），含 1s 复检、状态重置、60s 注销保护期。
- `commands/account.rs`：账号档案 CRUD（R2/R3/R4）。
  - `ACCOUNT_OP_LOCK`（`account.rs:10`）+ `try_account_op_lock`（`account.rs:13`）：switch/rename/delete 单飞互斥。
  - `list_accounts`（`account.rs:18`）、`switch_account`（`account.rs:26`）、`perform_switch_account_sync`（`account.rs:52`）、`merge_account_into_config`（`account.rs:85`，6 个登录字段 + displayName + active_account，排除设备级绑定）、`save_current_as_account`（`account.rs:104`）、`merge_save_as_target`（`account.rs:233`）、`create_account`（`account.rs:262`）、`delete_account`（`account.rs:310`）、`get_active_account`（`account.rs:364`）、`rename_account`（`account.rs:370`）、`perform_rename_account_sync`（`account.rs:389`）、`rename_account_core`（`account.rs:426`）、`validate_display_name`（`account.rs:455`，1–32 字符、禁控制字符）、`auto_create_account_for_current`（`account.rs:473`）、`auto_create_account_in`（`account.rs:496`，R2 自动建号，含撞库防护）。
- `commands/config_cmd.rs`：
  - `save_config_to_disk_encrypted(app_handle, config)`（`config_cmd.rs:8`）：落盘 + 掩码广播 + 刷新托盘（委托 `persist`）。
  - `load_config_from_file`（`config_cmd.rs:17`）、`load_config_from_disk_or_default`（`config_cmd.rs:42`，解析失败备份原文件后回退默认）、`build_config_export_payload`（`config_cmd.rs:73`，导出含 `passwordEncrypted` 标志，绝不明文）。

### 持久化层 `config::persist`

- `LOGIN_HISTORY_LOCK`（`persist.rs:11`）/ `QUALITY_HISTORY_LOCK`（`persist.rs:229`）：读-改-写全程互斥。
- `atomic_write(path, content)`（`persist.rs:13`）：临时文件 + `sync_all` + 重命名（防半截文件）。
- `get_data_dir`（`persist.rs:46`）、`get_config_path`（`persist.rs:61`）、`get_accounts_dir`（`persist.rs:65`）、`get_account_path`（`persist.rs:70`）、`get_login_history_path`（`persist.rs:170`）、`get_quality_history_path`（`persist.rs:231`）。
- `load_account_config(data_dir, account_id)`（`persist.rs:76`：读档并解密密码）、`save_account_config`（`persist.rs:101`：非空密码 DPAPI 加密落盘）、`list_account_items`（`persist.rs:118`：枚举账号档案为 `AccountItem`，空 displayName 兜底为 id）。
- `append_login_history`（`persist.rs:174`：头插、上限 100、损坏备份重置）、`append_quality_history`/`append_quality_history_to`（`persist.rs:237`/`241`：负延迟落盘为 null）。
- `save_config_and_broadcast(app_handle, config)`（`persist.rs:296`：落盘 + `masked_for_display` 广播 `config-changed` + Windows 下无头核心凭据跟随）、`save_config_to_disk_encrypted`（`persist.rs:324`：非空密码 DPAPI 加密）。

### 认证业务层 `auth/`

- `auth::service::credentials_missing(config)`（`auth/service.rs:24`）、`resolve_manual_adapter_account_id`（`:33`）、`credentials_config`（`:46`）、`load_adapter_account_config`（`:58`）、`adapter_account_from_dir`（`:66`）、`adapter_credentials`（`:77`）。
- `auth::service::full_login(state, app_handle, adapter_name)`（`auth/service.rs:81`）：登录编排——凭据校验、取适配器、DHCP 续租、单/双适配器按角色取绑定账号、并行错峰登录、失败计数。
- `auth::service::logout_adapter_with_log`（`auth/service.rs:193`）、`full_logout`（`auth/service.rs:208`）。
- `auth::service::post_login_handler(app_handle, state)`（`auth/service.rs:296`）：登录成功公共后处理（解保护期、延迟后台检测、按需自动退出）。
- `auth/mod.rs`（`auth/mod.rs`）：`failure_tracker`/`portal`/`protocol` 全平台；`dual_adapter_executor`/`session`/`service` 仅 `#[cfg(desktop)]`（`:6`–`:11`）。

### 监控层 `monitor/`

- `monitor::watcher::run_startup_tasks(app_handle)`（`monitor/watcher.rs:16`）：启动期注册 5 类后台任务——`startup_bg_check`（`:23`）、`startup_quality_once`（`:38`）、`startup_latency`（`:47`）、`startup_auto_login`（`:58`）、`scheduled_actions`（`:67`，每日定时登录/注销循环）。
- `monitor/mod.rs`：子模块 `watcher`/`auto_auth`/`latency`/`adapter_watch`/`campus_check`/`outbound_switch`/`portal_check`/`quality_scheduler`/`background_emit`/`background_check`/`background_task`/`outbound_night`/`scheduled`/`dynamic_interval`（`:1`–`:14`）。

### 网络层 `network/`

- `network/mod.rs`：模块树（`network/mod.rs:1`–`:13`），re-export `Adapter`/`AdapterDetail`/`DisabledAdapter`/`is_blacklisted`、`dhcp_*`、`subnet::*`、`adapter::*`、`adapter_cache::*`、`update_portal_url`、`check_network_quality_async`。
- `network::client::PORTAL_URL`（`network/client.rs:8`：`ArcSwap<String>`，全局 Portal 地址）、`CLIENT_POOL`（`:9`：`DashMap` 客户端池）、`ClientPoolKey`（`:14`）、`tls_version_id`（`:18`）、`CLIENT_POOL_TTL_SECS = 600`（`:29`）、`CLIENT_POOL_MAX_ENTRIES = 32`（`:31`）、`update_portal_url(url)`（`:33`）、`build_client`（`:43`）。

### 凭据加密 `account::crypto`

- `account/crypto.rs`：Windows 下 `dpapi` 模块（`:1`–`:2`）经 `CryptProtectData`/`CryptUnprotectData`（`crypt32`）加密，附加 entropy（审计 A10，`:39`）；提供 `encrypt`/`decrypt`（本文未展开全行，仅读前 40 行）。密码落盘/读回统一经此层，前端出站经 `Config::masked_for_display` 打码，明文/密文均不出站。

### 更新层 `update/`

- `update/updater.rs`：`GITHUB_REPO = "ikliml666/Wxxy-CampusLogin"`（`:9`）、`AUTO_CHECK_INTERVAL_SECS = 86400`（`:10`）、`BACKOFF_RETRY_SECS`（`:13`，5min/15min/1h 退避）、`STARTUP_CHECK_DELAY_SECS = 5`（`:15`）、`VERSION_FILE_URL`（`:24`）、`VERSION_MIRRORS`（`:27`，3 个 GitHub 代理降级源）。
- `LAST_CHECK_ERROR`（`updater.rs:21`）、`LAST_CHECK_TIME_MS`（`:23`）静态量。
- `ReleaseAsset`（`updater.rs:33`）、`UpdateInfo`（`:42`：`has_update`/`latest_version`/`release_notes`/`assets`/`sha256_checksum`/`last_check_error`/`last_check_time`）、`DownloadProgress`（`:59`）。

### WebVPN 协议核心 `vpn/`

深澜 Srun WebVPN 协议核心（11 个子模块），独立成文见 [[tauri-app/vpn]]。本模块只暴露 crate 级错误 `WebVpnError`（`vpn/mod.rs:46`）与 re-export（`route`/`WebVpnSession`/`wrap_url`/`GATEWAY`，`vpn/mod.rs:38`–`:41`）。

## Architecture

后端以 `AppState` 为中枢，Tauri 命令层（`commands/`）受理前端 IPC，调用认证/网络/配置/监控业务层；业务层通过 `EventBus` 向前端反向推送事件；Windows 桌面专属的提权动作走 `helper` 子进程。`Config` 经 `ConfigStore`/`ArcSwap` 全局共享，密码 DPAPI 落盘、出站经 `masked_for_display` 打码。

```text
┌──────────────────────── frontend (WebView2 SPA) ────────────────────────┐
│  React panels/hooks  ──IPC──►  commands/*  (do_login / save_config …)    │
└───────────────┬───────────────────────────────────────────────┬─────────┘
                │ invoke                                        │ emit events
                ▼                                                ▼
┌─────────────────────────── Tauri App (app/startup) ─────────────────────┐
│  AppState ──config:ConfigStore(ArcSwap) ── network:NetworkState ──…      │
│     │ commands/*  │ auth/service  │ monitor/watcher                      │
│     │ config/persist │ network/* │ account/crypto(DPAPI) │ update/updater│
│     └── EventBus ──► login-log / background-check-result / network-*     │
└───────────────┬───────────────────────────────────────────────┬─────────┘
                │ spawn_blocking(full_login/full_logout)          │ cfg(switch/rename/delete)
                ▼                                                ▼
┌───────── auth/session·protocol ──► Portal (10.1.99.100) ──┐  ┌── helper (elevated subproc,
└───────── vpn/* (WebVPN, see [[tauri-app/vpn]]) ──────────┤     netsh/DNS/MAC reset)
                                                          └── campus-login-core (headless
                                                              SYSTEM task, no Tauri runtime)
```

组件规模较大（命令层 10 个文件、config 8 个、auth 8 个、monitor 15 个、network 14 个、vpn 11 个、app 10 个、infra 8 个、platform 20 个、account/self_service/update 各 1–2 个），完整列表见各 `mod.rs` 模块树。前端与 WebVPN 协议分别见子模块 [[tauri-app/frontend]]、[[tauri-app/vpn]]。

## Data Flow

1. **启动**：`main()`（`main.rs:17`）→ 提权模式拦截 → 设置 WebView2 参数 → `app::startup::run`（`startup.rs:23`）装配插件并 `setup_app`（`startup.rs:161`）：初始化 logger、加载配置（`config_cmd.rs:42`）、构建托盘（`startup.rs:233`）、启动 `monitor::watcher::run_startup_tasks`（`watcher.rs:16`，注册 5 类后台任务）、心跳与 WebView2 崩溃订阅。
2. **登录**：前端 `do_login`（`login.rs:63`）→ `spawn_blocking` → `auth::service::full_login`（`service.rs:147`）→ 取适配器（`network::get_adapters_cached`）、按角色解析账号凭据（`service.rs:122`）、单/双适配器登录（`login_adapter_with_log`）→ 成功后经 `post_login_handler`（`service.rs:296`）延迟触发后台检测/自动退出。
3. **配置写路径**：`commands/config_cmd::save_config_to_disk_encrypted`（`config_cmd.rs:8`）→ `config::persist::save_config_and_broadcast`（`persist.rs:301`，DPAPI 加密非空密码 + `masked_for_display` 经 `EventBus::emit_config_changed` 广播 `config-changed` 事件）→ 刷新托盘。开关变更（如 `outbound_switch_now`）走同一落盘+广播出口。
4. **账号档案读写**：`list_accounts`/`switch_account`/`create_account`/`delete_account`/`rename_account`（`account.rs`）经 `config::persist::load/save_account_config`（`persist.rs:76`/`101`，DPAPI 解密/加密）、`auto_create_account_in`（R2 自动建号，`account.rs:496`）。`switch/delete/rename` 受 `ACCOUNT_OP_LOCK`（`account.rs:10`）单飞互斥。
5. **后台监控**：`monitor/watcher::run_startup_tasks`（`watcher.rs:19`）注册 `startup_bg_check`、`startup_quality_once`、`startup_latency`、`startup_auto_login`、`scheduled_actions` 五个 `task_manager` 任务，各自周期性读配置、调用 `auth`/`network` 层并 `EventBus` 回写前端。
6. **更新检查**：`update/updater.rs` 的 `start_update_check_loop`（`startup.rs:243`）每 24h 查 `version.json`（GitHub + 3 个镜像降级源，`updater.rs:374`/`27`），结果经 `EventBus::emit_update_available` 推送前端。
7. **提权动作**：Windows 下改 MAC / 设 DNS+DoH / 启用禁用适配器走 `helper` 子进程（`main.rs:31`/`38`），由主进程以管理员身份重启自身执行，`startup.rs` 在 Tauri Builder 之前拦截以免进入正常 UI 流程。
8. **无头核心**：`campus-login-core` 二进制（`Cargo.toml:16`）在开机未登录 Windows 时由 SYSTEM 计划任务拉起，复用 `lib` 的纯函数协议层（`vpn`、`account::crypto`、网络探测），无 Tauri 运行时；启用时凭据经 `core_autostart::refresh_core_config_if_changed`（`persist.rs:312`）跟随主配置变化。

## Connections

- [[tauri-app/vpn]] — 深澜 WebVPN 协议核心（`vpn/`），认证/自助服务校外回退经其 `wrap`/`cas`/`session`/`route`。
- [[tauri-app/frontend]] — React/TS 前端 SPA，通过 Tauri v2 IPC 调用本模块全部 `commands/*`。
- 平台专属能力集中在 `platform/`（提权 `elevation`、DNS `dns_config`、路由表 `route_table`、RTSS 兼容、GPU 检测、任务代理 `task_proxy`、生态模式 `ecoqos` 等），桌面编译、部分仅 Windows。
- 无头核心 `campus-login-core`（`src/bin/campus-login-core.rs`）与 `helper`（`helper/mod.rs`）共享 `lib` 协议层，被 SYSTEM 计划任务/`task_proxy` 拉起。
- 测试见 `tauri-app/src-tauri/tests/`：`captcha_solve.rs`、`repro_dns_runtime_drop.rs`、`repro_logout_panic.rs`、`task_proxy_smoke.rs`，以及各 `mod.rs` 内 `#[cfg(test)]` 单测。

## Known Issues

- **日志目录回退**（`app/startup.rs:173`–`:186`）：per-machine 安装到 Program Files 时安装目录 `logs` 不可写，writer=None 会静默丢失日志，已 fallback 到用户数据目录并在日志告警；NSIS POSTINSTALL hook 以管理员身份兜底。
- **`logRetentionDays` 仅保存不生效**（历史缺陷，注释见 `startup.rs:218`）：启动时已按配置 `set_log_retention_days`（`startup.rs:221`）补应用。
- **v2.4.2 黑屏强拉缺陷**（注释见 `startup.rs:227`–`:231`）：`--autostart` + `hidden_start` 时标记 `mark_hidden_by_design(true)` 避免主窗口被误拉前台。
- **RTSS hook 注入风险**（`main.rs:17`–`:53`）：浏览器参数须在 WebView2 环境创建前设置，setup 阶段处理已为时过晚（6 个 WebView2 进程 3 个已被注入），故预检提前到 `main` 入口。
- **注销崩溃事故根因**（`login.rs:29`–`:33`）：裸 `std::thread::scope` 子线程无 Tokio runtime context，`reqwest` 构造超时计时器需 `Handle::current()`，失败即 panic（A1 起 panic 改 unwind）；现经 `tauri::async_runtime::handle().inner().enter()` 进入 context 修复。
- **`auto_exit_after_login` 默认 false**（`config/model.rs:278`）：夜切后进程需常驻至次日 07:30 恢复，存量显式 `true` 不迁移。
- **`outbound_pending_switch` 历史残留**（`config/model.rs:100`）：旧版本登录闸残留行，本版本不再产生，仅由排程循环每拍清空，前端徽标依赖它（留着会一直显示过期状态）。
- **认证失败 panic 降级**（审计 A1，`Cargo.toml:116`）：不使用 `panic="abort"`，改 unwind，第三方 panic 仅降级为单任务失败而非整进程终止。
- **DPAPI 解密失败兜底**（`config_cmd.rs:31`）：加载配置时密码解密失败仅清空该字段保留其他配置，避免全量配置丢失；文件解析/解密失败还会备份原文件（`.corrupt-<ts>.bak`，`config_cmd.rs:55`）。
- **`literal "***"` 密码明文落盘**（`persist.rs:327`–`:331`）：保存时不得因密码等于 `PASSWORD_MASK` 而跳过 DPAPI 加密（用户真实密码恰为 `***` 时会明文落盘），已修复为「非空即加密」。
