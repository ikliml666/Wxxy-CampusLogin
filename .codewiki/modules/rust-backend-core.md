---
title: rust-backend-core
type: module
source_files:
  - android/src-tauri/Cargo.toml
  - android/src-tauri/build.rs
  - android/src-tauri/tauri.conf.json
  - android/src-tauri/src/android_state.rs
  - android/src-tauri/src/lib.rs
tags: [Tauri, Rust, Android, Backend, Config, Network, Entry]
---

## Overview
`rust-backend-core` 是校园网登录助手安卓端（CampusLogin）的 Tauri Rust 后端核心模块（包名 `campus-login-android`，版本 `2.4.1`，见 `android/src-tauri/Cargo.toml` L2-L3 与 `android/src-tauri/tauri.conf.json` L3-L4）。它作为应用的启动入口（Entry Point），负责初始化 Tauri 应用上下文、加载插件、注册 IPC 命令面，并管理进程级全局状态。该模块通过 `lib.rs` 协调各个子模块（如 `protocol_cmds`、`campus_detect`、`config_state`、`self_service_cmds`、`vpn_cmds`、`account_cmds`、`system_cmds`、`monitor_loop`、`battery_cmds`、`quality_cmds`、`update_cmds`、`face_model_cmds`、`android_state`、`cpu_affinity`、`identity_gate`、`login_history`、`quality_history`，声明于 `lib.rs` L12-L28），确保在 Android 环境下（前台服务保活、KeyStore 加密、电池优化处理）的稳定运行。核心设计原则包括：前端 UI 退出时保持进程常驻以维持后台巡检（`lib.rs` L137-L138）、密码落盘加密（KeyStore 插件，`lib.rs` L38）、以及基于静态原子变量的监控降级机制（`android_state.rs` L14）。协议核心通过 path 依赖 `campus-login` 单点共享（`Cargo.toml` L31）。

## Key Components

### `lib.rs`: 安卓端 Tauri 入口与命令注册
负责应用的初始化流程、插件挂载、命令暴露及生命周期管理。

*   **Functions**
    *   `run()` (L33-L141): [Main Entry] 安卓端 Tauri 应用的主入口函数。
        *   构建 `tauri::Builder::default()` (L34)。
        *   **Plugin Initialization** (L36-L42 mobile 分支; L44 non-mobile 分支仅 manage; L46 opener 插件):
            *   `tauri_plugin_campus_network_bind::init()` (L37): WLAN 网络绑定插件 (`bindProcessToNetwork`)。
            *   `tauri_plugin_campus_keystore::init()` (L38): Android KeyStore AES-GCM 加解密插件（密码落盘加密）。
            *   `tauri_plugin_campus_monitor_service::init()` (L39): 前台服务保活插件（通知+WifiLock+WakeLock）+开机自启。
            *   `tauri_plugin_biometric::init()` (L40): 生物识别验证门插件（该 crate `#![cfg(mobile)]`，host 编译为空）。
            *   `tauri_plugin_notification::init()` (L41): 通知插件。
            *   `tauri_plugin_opener::init()` (L46): 外链/APK 打开插件。
        *   **State Management** (L42, L44):
            *   `.manage(android_state::AndroidState::default())`: 注入全局状态（mobile 和 non-mobile 均注入）。
        *   **Setup Hook** (L47-L66):
            *   日志目录 (`app_data_dir/logs`) 确定 (L49-L53)。调用 `campus_login_lib::infra::logger::init_logger` (L56)。若失败则 `eprintln` 警告但不阻断启动 (L57-L58)。
            *   执行启动恢复任务：调用 `monitor_loop::run_startup_tasks(app.handle().clone())` 拉起后台检测/定时质量测试循环并执行启动自动登录 (L60)。
            *   清理孤儿临时配置文件：异步 spawn `config_state::cleanup_stale_config_tmp(dir)` (L62-L64)。
        *   **Command Handler** (L68-L129): 注册所有业务命令（见下方列表）。
        *   **Run Event Loop** (L132-L141):
            *   拦截 `tauri::RunEvent::ExitRequested` 事件，调用 `api.prevent_exit()` (L137-L138)。确保划掉任务（UI 关闭）时进程不退出，维持前台服务和后台巡检。彻底退出需通过系统设置。

*   **Registered Commands** (L68-L129)
    *   **Protocol & Network**: `protocol_cmds::ping_test`, `protocol_cmds::bind_to_wifi`, `protocol_cmds::accept_wifi_network`, `protocol_cmds::get_avoid_bad_wifi_status`, `protocol_cmds::restore_written_settings`, `protocol_cmds::do_login`, `protocol_cmds::do_logout`, `protocol_cmds::check_portal_status`.
    *   **Campus Detect**: `campus_detect::check_campus_status`, `campus_detect::request_wifi_ssid_permission`.
    *   **Config**: `config_state::get_config`, `config_state::save_config`.
    *   **Self Service**: `self_service_cmds::verify_biometric_identity`, `self_service_cmds::bind_operator`, `self_service_cmds::query_bind_status`, `self_service_cmds::query_self_dashboard`, `self_service_cmds::query_self_online_log`, `self_service_cmds::self_offline_session`, `self_service_cmds::change_self_password`, `self_service_cmds::reveal_operator_credential`.
    *   **VPN**: `vpn_cmds::vpn_save_credential`, `vpn_cmds::vpn_credential_status`, `vpn_cmds::vpn_clear_credential`, `vpn_cmds::set_self_service_via_vpn`, `vpn_cmds::vpn_test_connection`, `vpn_cmds::vpn_verify_credential`.
    *   **Account**: `account_cmds::list_accounts`, `account_cmds::switch_account`, `account_cmds::rename_account`, `account_cmds::save_current_as_account`, `account_cmds::create_account`, `account_cmds::delete_account`, `account_cmds::get_active_account`.
    *   **System**: `system_cmds::get_init_data`, `system_cmds::get_logs`, `system_cmds::clear_logs`, `system_cmds::get_log_retention_days`, `system_cmds::set_log_retention_days`, `system_cmds::get_debug_mode`, `system_cmds::set_debug_mode`.
    *   **Monitor Loop**: `monitor_loop::start_background_check`, `monitor_loop::stop_background_check`, `monitor_loop::trigger_background_check`, `monitor_loop::get_background_status`, `monitor_loop::get_boot_autostart`, `monitor_loop::set_boot_autostart`, `monitor_loop::get_notification_enabled`, `monitor_loop::set_notification_enabled`.
    *   **Battery**: `battery_cmds::get_battery_optimization_info`, `battery_cmds::request_ignore_battery_optimizations`, `battery_cmds::open_vendor_battery_settings`, `battery_cmds::open_notification_settings`.
    *   **Quality**: `quality_cmds::check_network_quality`, `quality_cmds::start_latency_test`, `quality_cmds::stop_latency_test`.
    *   **Update**: `update_cmds::check_update`, `update_cmds::download_update`, `update_cmds::get_mirror_urls`, `update_cmds::install_update`.
    *   **Face Model**: `face_model_cmds::face_models_state`, `face_model_cmds::face_models_download`.

### `android_state.rs`: 安卓全局状态
维护进程级静态标志和内存中的配置/IP 缓存。

*   **Constants**
    *   `WATCHER_FAILED` (L14): `pub(crate) static WATCHER_FAILED: AtomicBool = AtomicBool::new(false);`。WiFi watcher 注册失败降级标志（退化为纯周期检测）。
        *   逻辑：`start` 成功置 `false`；注册失败置 `true`，经 `get_background_status("watcherFailed")` 暴露给前端。
        *   独立于 `AndroidState` 结构体，以便在 `tauri .manage` 之前（单测/启动早期）被 `status_value` 同步 fn 访问，免锁（`android_state.rs` L1-L3 注释）。

*   **Structs**
    *   `AndroidState` (L17-L21): `#[derive(Default)] pub struct AndroidState`。Tauri State 管理对象。
        *   `cached_source_ip: Mutex<Option<Ipv4Addr>>` (L18): 检测阶段写入的 wlan0 源 IP 缓存（登录/注销复用）。
        *   `config: Mutex<Option<Settings>>` (L20): 明文配置内存态（密码仅存内存，落盘必经 Keystore 加密；`Settings` 引自 `crate::config_state` L9）。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│                              Tauri Frontend                         │
└──────────────────────────┬──────────────────────────────────────────┘
                           │ IPC Calls / Events
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       lib.rs (Entry Point, L33)                     │
│ - run(): Init Plugins (L36-L46), Manage State (L42/L44)             │
│ - setup(): Logger (L56), Startup Tasks (L60), Cleanup (L62)         │
│ - invoke_handler: Register commands (L68-L129)                      │
│ - run_event_loop: Prevent exit (L137-L138)                          │
└──────────────────────────┬──────────────────────────────────────────┘
                           │ manages / uses
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  android_state.rs: AndroidState (L17) / WATCHER_FAILED (L14)        │
│  submodules: protocol_cmds, campus_detect, config_state, monitor_loop│
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### 应用启动流程
1.  `lib.rs::run()` (L33) 被调用。
2.  初始化 Tauri Builder (L34)，挂载插件：mobile 下 CampusNetworkBind (L37), Keystore (L38), MonitorService (L39), Biometric (L40), Notification (L41)；公共 Opener (L46)。non-mobile 仅 manage 状态 (L44)。
3.  注入 `AndroidState` 默认实例 (L42/L44)。
4.  执行 `setup` 回调 (L47-L66)：
    *   确定日志目录 (`app_data_dir/logs`) (L49-L53)。
    *   调用 `campus_login_lib::infra::logger::init_logger` 初始化日志 (L56)。若失败打印错误 (L57-L58)。
    *   调用 `monitor_loop::run_startup_tasks` (L60)：根据持久化配置拉起后台检测循环、定时质量测试，并尝试自动登录。
    *   异步清理残留的临时配置文件 `config_state::cleanup_stale_config_tmp` (L62-L64)。
5.  注册所有业务命令到 `invoke_handler` (L68-L129)。
6.  构建应用 (L130-L131) 并进入运行循环。拦截 `ExitRequested` 事件 (L137-L138) 以防止 UI 关闭导致进程终止。

## Connections
*   [[monitor_loop]]: 由 `lib.rs` 的 `setup` 阶段调用 `run_startup_tasks` (L60) 启动；其状态通过 `WATCHER_FAILED` (`android_state.rs` L14) 暴露。
*   [[config_state]]: 提供 `cleanup_stale_config_tmp` (`lib.rs` L62-L64) 供启动时调用；`AndroidState.config` (`android_state.rs` L20) 缓存其管理的 `Settings`。
*   [[account_cmds]]: 通过 `lib.rs` 注册的命令交互 (L95-L101)，受 `config_state` 中的账号目录约束。
*   [[campus_detect]]: 通过 `lib.rs` 注册的命令交互 (L77-L78)，依赖 `AndroidState.cached_source_ip` (`android_state.rs` L18)。
*   [[protocol_cmds]]: 登录/注销/Portal 探测/WiFi 绑定命令注册于 `lib.rs` L69-L76。
*   [[self_service_cmds]]: 自助服务命令注册于 `lib.rs` L81-L88。
*   [[vpn_cmds]]: WebVPN 凭据与校外自动路由命令注册于 `lib.rs` L89-L94。
*   [[system_cmds]]: 系统配置与日志命令注册于 `lib.rs` L102-L108。
*   [[battery_cmds]]: 电池优化命令注册于 `lib.rs` L117-L120。
*   [[quality_cmds]]: 网络质量命令注册于 `lib.rs` L121-L123。
*   [[update_cmds]]: 更新流命令注册于 `lib.rs` L124-L127。
*   [[face_model_cmds]]: 人脸模型命令注册于 `lib.rs` L128-L129。

## Known Issues
*   **退出护栏限制**: `lib.rs` L137-L138 中强制阻止 `ExitRequested`。用户无法通过常规“划掉任务”方式完全退出应用，必须通过系统设置或特定退出命令。这可能导致进程堆积，需确保后台任务能正确响应停止指令。
*   **日志初始化静默失败**: `lib.rs` L56-L58 中，若 `init_logger` 失败，仅 `eprintln` 到 stderr，不 panic。这可能导致日志文件未创建，后续排障困难，尽管代码注释强调了这一点。
