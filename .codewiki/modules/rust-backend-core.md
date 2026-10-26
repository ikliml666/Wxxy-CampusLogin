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
`rust-backend-core` 是校园网登录助手安卓端（CampusLogin）的 Tauri Rust 后端核心模块（包名 `campus-login-android`，版本 `2.4.2`，见 `android/src-tauri/Cargo.toml` L2-L3 与 `android/src-tauri/tauri.conf.json` L3-L4）。它作为应用的启动入口（Entry Point），负责初始化 Tauri 应用上下文、加载插件、注册 IPC 命令面，并管理进程级全局状态。该模块通过 `lib.rs` 协调各个子模块（`campus_detect`、`android_state`、`config_state`、`cpu_affinity`、`identity_gate`、`login_history`、`protocol_cmds`、`self_service_cmds`、`vpn_cmds`、`monitor_loop`、`account_cmds`、`system_cmds`、`quality_cmds`、`quality_history`、`update_cmds`、`face_model_cmds`、`battery_cmds`，声明于 `lib.rs` L12-L28），确保在 Android 环境下（前台服务保活、KeyStore 加密、电池优化处理）的稳定运行。核心设计原则包括：前端 UI 退出时保持进程常驻以维持后台巡检（`lib.rs` L137-L138）、密码落盘加密（KeyStore 插件，`lib.rs` L38）、以及基于静态原子变量的监控降级机制（`android_state.rs` L14）。协议核心通过 path 依赖 `campus-login` 单点共享（`Cargo.toml` L31）。发布配置采用 `lto = "fat"`、`codegen-units = 1`、`strip = true`（`Cargo.toml` L55-L58），且故意不设 `panic = "abort"` 以允许 async 任务 panic 被 unwind 捕获（`Cargo.toml` L53-L54）。

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
        *   **Command Handler** (L68-L130): 注册所有业务命令（见下方列表）。
        *   **Run Event Loop** (L131-L140):
            *   拦截 `tauri::RunEvent::ExitRequested` 事件，调用 `api.prevent_exit()` (L137-L138)。确保划掉任务（UI 关闭）时进程不退出，维持前台服务和后台巡检。彻底退出需通过系统设置。

*   **Registered Commands** (L68-L130)
    *   **Protocol & Network**: `protocol_cmds::ping_test` (L69), `protocol_cmds::bind_to_wifi` (L70), `protocol_cmds::accept_wifi_network` (L71), `protocol_cmds::get_avoid_bad_wifi_status` (L72), `protocol_cmds::restore_written_settings` (L73), `protocol_cmds::do_login` (L74), `protocol_cmds::do_logout` (L75), `protocol_cmds::check_portal_status` (L76).
    *   **Campus Detect**: `campus_detect::check_campus_status` (L77), `campus_detect::request_wifi_ssid_permission` (L78).
    *   **Config**: `config_state::get_config` (L79), `config_state::save_config` (L80).
    *   **Self Service**: `self_service_cmds::verify_biometric_identity` (L81), `self_service_cmds::bind_operator` (L82), `self_service_cmds::query_bind_status` (L83), `self_service_cmds::query_self_dashboard` (L84), `self_service_cmds::query_self_online_log` (L85), `self_service_cmds::self_offline_session` (L86), `self_service_cmds::change_self_password` (L87), `self_service_cmds::reveal_operator_credential` (L88).
    *   **VPN**: `vpn_cmds::vpn_save_credential` (L89), `vpn_cmds::vpn_credential_status` (L90), `vpn_cmds::vpn_clear_credential` (L91), `vpn_cmds::set_self_service_via_vpn` (L92), `vpn_cmds::vpn_test_connection` (L93), `vpn_cmds::vpn_verify_credential` (L94).
    *   **Account**: `account_cmds::list_accounts` (L95), `account_cmds::switch_account` (L96), `account_cmds::rename_account` (L97), `account_cmds::save_current_as_account` (L98), `account_cmds::create_account` (L99), `account_cmds::delete_account` (L100), `account_cmds::get_active_account` (L101).
    *   **System**: `system_cmds::get_init_data` (L102), `system_cmds::get_logs` (L103), `system_cmds::clear_logs` (L104), `system_cmds::get_log_retention_days` (L105), `system_cmds::set_log_retention_days` (L106), `system_cmds::get_debug_mode` (L107), `system_cmds::set_debug_mode` (L108).
    *   **Monitor Loop**: `monitor_loop::start_background_check` (L109), `monitor_loop::stop_background_check` (L110), `monitor_loop::trigger_background_check` (L111), `monitor_loop::get_background_status` (L112), `monitor_loop::get_boot_autostart` (L113), `monitor_loop::set_boot_autostart` (L114), `monitor_loop::get_notification_enabled` (L115), `monitor_loop::set_notification_enabled` (L116).
    *   **Battery**: `battery_cmds::get_battery_optimization_info` (L117), `battery_cmds::request_ignore_battery_optimizations` (L118), `battery_cmds::open_vendor_battery_settings` (L119), `battery_cmds::open_notification_settings` (L120).
    *   **Quality**: `quality_cmds::check_network_quality` (L121), `quality_cmds::start_latency_test` (L122), `quality_cmds::stop_latency_test` (L123).
    *   **Update**: `update_cmds::check_update` (L124), `update_cmds::download_update` (L125), `update_cmds::get_mirror_urls` (L126), `update_cmds::install_update` (L127).
    *   **Face Model**: `face_model_cmds::face_models_state` (L128), `face_model_cmds::face_models_download` (L129).

### `android_state.rs`: 安卓全局状态
维护进程级静态标志和内存中的配置/IP 缓存。

*   **Constants**
    *   `WATCHER_FAILED` (L14): `pub(crate) static WATCHER_FAILED: AtomicBool = AtomicBool::new(false);`。WiFi watcher 注册失败降级标志（退化为纯周期检测）。
        *   逻辑：`start` 成功置 `false`；注册失败置 `true`，经 `get_background_status("watcherFailed")` 暴露给前端（`android_state.rs` L11-L13 注释）。
        *   独立于 `AndroidState` 结构体，以便在 `tauri .manage` 之前（单测/启动早期）被 `status_value` 同步 fn 访问，免锁（`android_state.rs` L1-L3 注释）。

*   **Structs**
    *   `AndroidState` (L16-L21): `#[derive(Default)] pub struct AndroidState`。Tauri State 管理对象。
        *   `cached_source_ip: Mutex<Option<Ipv4Addr>>` (L18): 检测阶段写入的 wlan0 源 IP 缓存（登录/注销复用）。
        *   `config: Mutex<Option<Settings>>` (L20): 明文配置内存态（密码仅存内存，落盘必经 Keystore 加密；`Settings` 引自 `crate::config_state` L9）。

### `build.rs`: 构建脚本
*   **Functions**
    *   `main()` (L1-L3): 调用 `tauri_build::build()` 执行 Tauri 构建时配置（如生成上下文、Android 权限合并等）。

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
│ - invoke_handler: Register commands (L68-L130)                      │
│ - run_event_loop: Prevent exit (L137-L138)                          │
└──────────────────────────┬──────────────────────────────────────────┘
                           │ manages / uses
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  android_state.rs: AndroidState (L16) / WATCHER_FAILED (L14)        │
│  submodules: protocol_cmds, campus_detect, config_state, monitor_loop│
│  (declared lib.rs L12-L28)                                          │
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
5.  注册所有业务命令到 `invoke_handler` (L68-L130)。
6.  构建应用 (L131) 并进入运行循环。拦截 `ExitRequested` 事件 (L137-L138) 以防止 UI 关闭导致进程终止。

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
*   [[identity_gate]]: 验证门 TTL 模块，声明于 `lib.rs` L16，被 self_service_cmds 等使用。
*   [[login_history]]: 登录历史模块，声明于 `lib.rs` L17。
*   [[quality_history]]: 网络质量历史落盘模块，声明于 `lib.rs` L25。
*   [[cpu_affinity]]: 绑小核模块，声明于 `lib.rs` L15，依赖 libc sched_setaffinity（见 `Cargo.toml` L27-L28）。

## Known Issues
*   **退出护栏限制**: `lib.rs` L137-L138 中强制阻止 `ExitRequested`。用户无法通过常规“划掉任务”方式完全退出应用，必须通过系统设置或特定退出命令。这可能导致进程堆积，需确保后台任务能正确响应停止指令。
*   **日志初始化静默失败**: `lib.rs` L56-L58 中，若 `init_logger` 失败，仅 `eprintln` 到 stderr，不 panic。这可能导致日志文件未创建，后续排障困难，尽管代码注释强调了这一点（L54-L55 注释说明初始化失败不得静默但仅打印）。
*   **panic 策略权衡**: `Cargo.toml` L53-L54 注释明确指出未设置 `panic = "abort"`，因为 Tauri 移动入口 async 任务 panic 时 unwind 捕获为任务失败，abort 会杀死整个进程导致前台服务/保活陪葬；这是可用性换体积的选择，但意味着 panic 可能留下部分状态不一致。
*   **Watcher 失败降级**: `android_state.rs` L14 的 `WATCHER_FAILED` 标志在 WiFi watcher 注册失败时置 true，退化为纯周期检测；若前端未正确处理 `watcherFailed` 状态，用户可能误以为实时监听生效。
