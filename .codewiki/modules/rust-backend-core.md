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
`rust-backend-core` 是校园网登录助手安卓端（CampusLogin）的 Tauri Rust 后端核心模块。它作为应用的启动入口（Entry Point），负责初始化 Tauri 应用上下文、加载插件、注册 IPC 命令面，并管理进程级全局状态。该模块通过 `lib.rs` 协调各个子模块（如配置、账号、网络探测等），确保在 Android 环境下（前台服务保活、KeyStore 加密、电池优化处理）的稳定运行。核心设计原则包括：前端 UI 退出时保持进程常驻以维持后台巡检、密码落盘加密、以及基于静态原子变量的监控降级机制。

## Key Components

### `lib.rs`: 安卓端 Tauri 入口与命令注册
负责应用的初始化流程、插件挂载、命令暴露及生命周期管理。

*   **Functions**
    *   `run()` (L18-L96): [Main Entry] 安卓端 Tauri 应用的主入口函数。
        *   构建 `tauri::Builder`。
        *   **Plugin Initialization**:
            *   `tauri_plugin_campus_network_bind::init()`: WLAN 网络绑定插件。
            *   `tauri_plugin_campus_keystore::init()`: Android KeyStore AES-GCM 加解密插件。
            *   `tauri_plugin_campus_monitor_service::init()`: 前台服务保活插件。
            *   `tauri_plugin_biometric::init()`: 生物识别验证门插件（仅 mobile 编译）。
            *   `tauri_plugin_notification::init()`: 通知插件。
            *   `tauri_plugin_opener::init()`: 外链/APK 打开插件。
        *   **State Management**:
            *   `.manage(android_state::AndroidState::default())`: 注入全局状态（mobile 和 non-mobile 均注入）。
        *   **Setup Hook** (L30-L47):
            *   初始化日志目录 (`app_data_dir/logs`)。调用 `campus_login_lib::infra::logger::init_logger`。若失败则 `eprintln` 警告但不阻断启动。
            *   执行启动恢复任务：调用 `monitor_loop::run_startup_tasks` 拉起后台检测/定时质量测试循环并执行自动登录。
            *   清理孤儿临时配置文件：异步调用 `config_state::cleanup_stale_config_tmp`。
        *   **Command Handler**: 注册所有业务命令（见下方列表）。
        *   **Run Event Loop** (L85-L92):
            *   拦截 `ExitRequested` 事件，调用 `api.prevent_exit()`。确保划掉任务（UI 关闭）时进程不退出，维持前台服务和后台巡检。彻底退出需通过系统设置。

*   **Registered Commands** (L49-L83)
    *   **Protocol & Network**: `ping_test`, `bind_to_wifi`, `accept_wifi_network`, `get_avoid_bad_wifi_status`, `restore_written_settings`, `do_login`, `do_logout`, `check_portal_status`.
    *   **Campus Detect**: `check_campus_status`, `request_wifi_ssid_permission`.
    *   **Config**: `get_config`, `save_config`.
    *   **Self Service**: `verify_biometric_identity`, `bind_operator`, `query_bind_status`, `query_self_dashboard`, `query_self_online_log`, `self_offline_session`, `change_self_password`, `reveal_operator_credential`.
    *   **Account**: `list_accounts`, `switch_account`, `rename_account`, `save_current_as_account`, `create_account`, `delete_account`, `get_active_account`.
    *   **System**: `get_init_data`, `get_logs`, `clear_logs`, `get_log_retention_days`, `set_log_retention_days`, `get_debug_mode`, `set_debug_mode`.
    *   **Monitor Loop**: `start_background_check`, `stop_background_check`, `trigger_background_check`, `get_background_status`, `get_boot_autostart`, `set_boot_autostart`, `get_notification_enabled`, `set_notification_enabled`.
    *   **Battery**: `get_battery_optimization_info`, `request_ignore_battery_optimizations`, `open_vendor_battery_settings`, `open_notification_settings`.
    *   **Quality**: `check_network_quality`, `start_latency_test`, `stop_latency_test`.
    *   **Update**: `check_update`, `download_update`, `get_mirror_urls`, `install_update`.
    *   **Face Model**: `face_models_state`, `face_models_download`.

### `android_state.rs`: 安卓全局状态
维护进程级静态标志和内存中的配置/IP 缓存。

*   **Constants**
    *   `WATCHER_FAILED` (L13): `pub(crate) static AtomicBool`。WiFi watcher 注册失败降级标志。
        *   初始值 `false`。
        *   逻辑：`start` 成功置 `false`；注册失败置 `true`。通过 `get_background_status("watcherFailed")` 暴露给前端。独立于 `AndroidState` 结构体，以便在 `tauri .manage` 之前（单测/启动早期）被访问。

*   **Structs**
    *   `AndroidState` (L17-L23): `#[derive(Default)]`。Tauri State 管理对象。
        *   `cached_source_ip`: `Mutex<Option<Ipv4Addr>>`。检测阶段写入的 wlan0 源 IP 缓存。
        *   `config`: `Mutex<Option<Settings>>`。明文配置内存态（密码仅存内存，落盘必经 Keystore 加密）。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Tauri Frontend                                 │
└──────────────────────────┬──────────────────────────────────────────────────┘
                           │ IPC Calls / Events
                           ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                            lib.rs (Entry Point)                             │
│  - run(): Init Plugins, Manage State, Setup Logger, Spawn Tasks             │
│  - invoke_handler: Register all commands                                    │
│  - run_event_loop: Prevent exit on window close                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### 应用启动流程
1.  `lib.rs::run()` 被调用。
2.  初始化 Tauri Builder，挂载 CampusNetworkBind, Keystore, MonitorService, Biometric, Notification, Opener 插件。
3.  注入 `AndroidState` 默认实例。
4.  执行 `setup` 回调：
    *   确定日志目录 (`app_data_dir/logs`)。
    *   调用 `campus_login_lib::infra::logger::init_logger` 初始化日志。若失败打印错误。
    *   调用 `monitor_loop::run_startup_tasks`：根据持久化配置拉起后台检测循环、定时质量测试，并尝试自动登录。
    *   异步清理残留的临时配置文件。
5.  注册所有业务命令到 `invoke_handler`。
6.  构建应用并进入运行循环。拦截 `ExitRequested` 事件以防止 UI 关闭导致进程终止。

## Connections
*   [[monitor_loop.rs]]: 由 `lib.rs` 的 `setup` 阶段调用 `run_startup_tasks` 启动。
*   [[config_state.rs]]: 提供 `cleanup_stale_config_tmp` 供启动时调用；`AndroidState.config` 缓存其管理的 Settings。
*   [[account_cmds.rs]]: 通过 `lib.rs` 注册的命令交互，受 `config_state` 中的账号目录约束。
*   [[campus_detect.rs]]: 通过 `lib.rs` 注册的命令交互，依赖 `AndroidState.cached_source_ip`。

## Known Issues
*   **退出护栏限制**: `lib.rs` L85-L92 中强制阻止 `ExitRequested`。用户无法通过常规“划掉任务”方式完全退出应用，必须通过系统设置或特定退出命令。这可能导致进程堆积，需确保后台任务能正确响应停止指令。
*   **日志初始化静默失败**: `lib.rs` L36-L39 中，若 `init_logger` 失败，仅 `eprintln` 到 stderr，不 panic。这可能导致日志文件未创建，后续排障困难，尽管代码注释强调了这一点。