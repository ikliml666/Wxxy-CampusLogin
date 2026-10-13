---
title: android/src-tauri 安卓端 Rust 模块基线与架构
type: learning
source_files:
  - android/src-tauri/Cargo.toml
  - android/src-tauri/src/lib.rs
tags: [教训, 安卓, 构建, 验证]
---

## Overview

`android/src-tauri` 是校园网登录助手（Campus Login）的 Android 端 Tauri 应用根 crate。它通过 `path` 依赖桌面端的协议核心 `campus-login`，并集成了多个安卓专属插件（网络绑定、Keystore、前台服务）。

**关键构建特性**：该模块在 Host（桌面开发机）上执行 `cargo check` 或 `cargo test` **必然失败**。这是因为其入口逻辑重度依赖 `#[cfg(mobile)]` 门控的平台特定插件与目标属性依赖。安卓 Rust 改动必须使用交叉编译验证。

## Key Components

### 配置与依赖 (`Cargo.toml`)

- **Package**: `campus-login-android`, version `2.4.1`.
- **Protocol Core**: `campus-login = { path = "../../tauri-app/src-tauri" }` (行 34)。单点共享协议逻辑。
- **Android-Specific Dependencies** (行 28-31):
    - `android_system_properties = "0.1"`: 读取 `ro.soc.model` 等系统属性。
    - `libc = "0.2"`: 用于 `sched_setaffinity` 绑核。
    - 仅当 `target_os = "android"` 时编译。
- **Plugins**:
    - `tauri-plugin-campus-network-bind`: WLAN 网络绑定 (`bindProcessToNetwork`)。
    - `tauri-plugin-campus-keystore`: AndroidKeyStore AES-GCM 加解密。
    - `tauri-plugin-campus-monitor-service`: 前台服务保活 (通知+WifiLock+WakeLock)。
    - `tauri-plugin-biometric` & `tauri-plugin-opener`: 官方生物识别与外链支持。
- **Release Profile** (行 56-59):
    - `lto = "fat"`, `codegen-units = 1`, `strip = true`。
    - **注意**: 未设置 `panic="abort"`。因为 Tauri 移动入口的 async 任务 panic 时 unwind 捕获为任务失败，若设 abort 则直接杀死进程（前台服务/保活随进程陪葬），可用性换体积不划算。

### 入口与插件注册 (`src/lib.rs`)

- **`run()`** (行 27-96): 安卓端 Tauri 入口函数。
    - **Builder 配置** (行 32-43):
        - `#[cfg(mobile)]` (行 33-39): 注册 `campus_network_bind`, `campus_keystore`, `campus_monitor_service`, `biometric`, `notification` 插件，并 `manage` `AndroidState`。
        - `#[cfg(not(mobile))]` (行 41-42): **Host 编译路径**。仅 `manage` `AndroidState`，**不注册任何插件**。这是导致 Host `cargo check` 失败的主因之一。
    - **Setup Hook** (行 44-58):
        - 初始化日志到 `app_data_dir/logs`。
        - 调用 `monitor_loop::run_startup_tasks` 启动后台检测/定时质量测试循环及自动登录。
        - 异步清理上次崩溃遗留的配置临时文件 (`config_state::cleanup_stale_config_tmp`)。
    - **Invoke Handler** (行 60-92): 注册所有命令面处理器。
    - **Run Event Loop** (行 93-96):
        - **退出护栏**: 拦截 `ExitRequested` 事件 (`api.prevent_exit()`)。
        - **目的**: Tauri 默认行为是最后窗口关闭时 `process::exit(0)`，这会带走前台服务。安卓语义要求划掉任务只关 UI，进程随前台服务常驻以继续巡检/夜切/定时动作；彻底退出需走系统设置。

### 模块分工 (`lib.rs` 顶部注释, 行 5-13)

- `protocol_cmds`: 登录/注销/Portal 探测/WiFi 绑定。
- `campus_detect`: 校园网探针 (检测卡 + check_campus_status)。
- `config_state`: 全量配置 (Keystore 加密落盘/掩码出口)。
- `self_service_cmds`: 自助服务六命令 + 生物识别验证门。
- `identity_gate` / `login_history`: 验证门 TTL 与登录历史。
- `quality_history`: 网络质量历史落盘 (形状与桌面端一致)。
- `android_state`: 进程态 (源 IP 缓存/配置内存态/监控循环句柄)。
- 其他导入模块: `cpu_affinity`, `monitor_loop`, `account_cmds`, `system_cmds`, `quality_cmds`, `update_cmds`, `face_model_cmds`, `battery_cmds`。

### 公开命令面 (按模块归类)

**Protocol (`protocol_cmds`)**:
- `ping_test`
- `bind_to_wifi`
- `accept_wifi_network`
- `get_avoid_bad_wifi_status`
- `restore_written_settings`
- `do_login`
- `do_logout`
- `check_portal_status`

**Campus Detect (`campus_detect`)**:
- `detect_campus`
- `check_campus_status`
- `request_wifi_ssid_permission`

**Config (`config_state`)**:
- `get_config`
- `save_config`

**Self Service (`self_service_cmds`)**:
- `verify_biometric_identity`
- `bind_operator`
- `query_bind_status`
- `query_self_dashboard`
- `query_self_online_log`
- `self_offline_session`
- `change_self_password`
- `reveal_operator_credential`

**Account (`account_cmds`)**:
- `list_accounts`
- `switch_account`
- `rename_account`
- `save_current_as_account`
- `delete_account`
- `get_active_account`

**System (`system_cmds`)**:
- `get_init_data`
- `get_soc_info`
- `get_logs`
- `clear_logs`
- `get_log_retention_days`
- `set_log_retention_days`
- `get_debug_mode`
- `set_debug_mode`

**Monitor (`monitor_loop`)**:
- `start_background_check`
- `stop_background_check`
- `trigger_background_check`
- `get_background_status`
- `get_boot_autostart`
- `set_boot_autostart`
- `get_notification_enabled`
- `set_notification_enabled`

**Battery (`battery_cmds`)**:
- `get_battery_optimization_info`
- `request_ignore_battery_optimizations`
- `open_vendor_battery_settings`
- `open_notification_settings`

**Quality (`quality_cmds`)**:
- `check_network_quality`
- `start_latency_test`
- `stop_latency_test`

**Update (`update_cmds`)**:
- `check_update`
- `download_update`
- `get_mirror_urls`
- `install_update`

**Face Model (`face_model_cmds`)**:
- `face_models_state`
- `face_models_download`

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                   android/src-tauri                         │
│                                                             │
│  ┌──────────────────┐     ┌──────────────────────────────┐  │
│  │   Tauri Builder  │     │      Invoke Handlers         │  │
│  │                  │     │                              │  │
│  │ [mobile] Plugins │     │ protocol_cmds                │  │
│  │ • network-bind   │────►│ campus_detect                │  │
│  │ • keystore       │     │ config_state                 │  │
│  │ • monitor-svc    │     │ self_service_cmds            │  │
│  │ • biometric      │     │ account_cmds                 │  │
│  │ • notification   │     │ system_cmds                  │  │
│  │                  │     │ monitor_loop                 │  │
│  │ [not mobile]     │     │ quality_cmds                 │  │
│  │ (Empty Plugins)  │     │ update_cmds                  │  │
│  │                  │     │ face_model_cmds              │  │
│  └────────┬─────────┘     │ battery_cmds                 │  │
│           │               └──────────┬───────────────────┘  │
│           │                          │                      │
│           ▼                          ▼                      │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              Shared State & Logic                     │   │
│  │                                                      │   │
│  │ • android_state (IP cache, mem config, monitor handle)│   │
│  │ • campus-login (path dep from ../../tauri-app)        │   │
│  │ • infra::logger (init at startup)                     │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                             │
│  Exit Guard: prevents_exit() on window close to keep        │
│  foreground service alive for monitoring/auto-switch.       │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **Startup**: `run()` 被调用 -> 根据 `#[cfg(mobile)]` 注册插件 -> Setup 阶段初始化日志、启动后台监控循环 (`monitor_loop::run_startup_tasks`)、清理孤儿 tmp 文件。
2.  **Runtime**: 前端通过 Tauri Commands 调用后端逻辑。例如 `do_login` 触发 `protocol_cmds::do_login`，可能涉及 `campus-login` 核心逻辑和 `config_state` 读写。
3.  **Background**: `monitor_loop` 在后台运行检测和定时任务，不受 UI 窗口关闭影响（得益于 Exit Guard）。
4.  **Shutdown**: 用户划掉任务 -> `ExitRequested` 事件触发 -> `api.prevent_exit()` -> 进程保留，前台服务继续运行。只有用户通过系统设置彻底退出时，进程才会终止。

## Known Issues

- **Host Build Failure**: 在 Host 环境（非 Android 目标）下，`cargo check` 和 `cargo test` **必然失败**。
    - **原因**: `lib.rs` 中的 `#[cfg(mobile)]` 块在 Host 上被跳过，导致平台特定插件未注册；同时 `Cargo.toml` 中的 `[target.'cfg(target_os = "android")'.dependencies]` 段（如 `android_system_properties`, `libc`）在 Host 上不可用。
    - **结论**: 基线状态下 Host `cargo check` 就是失败的，不要用它来判断安卓改动是否有问题。
    - **解决方案**: 必须使用交叉编译进行验证：`cargo check --target aarch64-linux-android --all-targets`（需配置 NDK 工具链环境变量），或使用构建脚本 `pwsh android/build-apk.ps1` 一键出包。在 Host 上进行功能测试需要注入假桥（Mock）。

## Connections

[[verification-baseline]]、[[tauri-android-build-report-path-missing]]