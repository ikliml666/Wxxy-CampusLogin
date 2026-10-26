---
title: android/src-tauri 安卓端 Rust 模块基线与架构
type: learning
source_files:
  - android/src-tauri/Cargo.toml
  - android/src-tauri/src/lib.rs
tags: [教训, 安卓, 构建, 验证]
---

## Overview

`android/src-tauri` 是校园网登录助手（Campus Login）的 Android 端 Tauri 应用根 crate（包名 `campus-login-android`，版本 `2.4.3`，见 `Cargo.toml` 行 2-3）。它通过 `path` 依赖桌面端的协议核心 `campus-login`（`Cargo.toml` 行 31）单点共享逻辑，并集成了多个安卓专属插件（网络绑定、Keystore、前台服务，见 `Cargo.toml` 行 34、37、40）。

**关键构建特性**：该模块在 Host（桌面开发机，非 Android 目标）上执行 `cargo check` 或 `cargo test` **仍会失败**。当前 `Cargo.toml` 所有依赖均置于普通 `[dependencies]`（`Cargo.toml` 行 14-48），如 `libc`（`Cargo.toml` 行 28）跨平台可用。但 `lib.rs` 中 `#[cfg(mobile)]` 门控（行 35-42）在 Host 被跳过，仅走 `#[cfg(not(mobile))]` 分支（行 43-44）管理状态而不注册插件；而插件 crate（如 `tauri-plugin-campus-network-bind` `Cargo.toml` 行 34）包含安卓特定实现，在 Host 缺少 NDK/JNI 时编译报错。因此安卓 Rust 改动必须使用交叉编译验证（详见 Known Issues）。

## Key Components

### 配置与依赖 (`android/src-tauri/Cargo.toml`)

- **[package]**（行 1-5）：`name = "campus-login-android"`（行 2），`version = "2.4.3"`（行 3），`edition = "2021"`（行 5）。
- **[lib]**（行 7-9）：`name = "campus_login_android_lib"`（行 8），`crate-type = ["cdylib", "rlib"]`（行 9）。
- **[build-dependencies]**（行 11-12）：`tauri-build = { version = "2", features = [] }`（行 12）。
- **[dependencies]**（行 14-48）：
  - 核心 Tauri：`tauri = { version = "2", features = ["protocol-asset"] }`（行 15），`tauri-plugin-notification = "2"`（行 16）。
  - 序列化/异步：`serde`（行 17），`serde_json`（行 18），`tokio`（行 19，启用 `rt-multi-thread` 等），`chrono`（行 20），`lazy_static`（行 21），`regex`（行 22）。
  - 网络与系统：`network-interface = "2"`（行 23），`sha2 = "0.10"`（行 25，APK 下载校验），`libc = "0.2"`（行 28，`sched_setaffinity` 绑核）。
  - 协议核心与插件 path 依赖：`campus-login = { path = "../../tauri-app/src-tauri" }`（行 31），`tauri-plugin-campus-network-bind`（行 34），`tauri-plugin-campus-keystore`（行 37），`tauri-plugin-campus-monitor-service`（行 40），`tauri-plugin-biometric = "2"`（行 43），`tauri-plugin-opener = "2"`（行 44）。
  - 更新流：`reqwest`（行 47，禁用默认特性启用 `rustls-tls` 避免安卓 openssl 交叉编译），`futures-util`（行 48）。
- **[profile.release]**（行 55-58）：`lto = "fat"`（行 56），`codegen-units = 1`（行 57），`strip = true`（行 58）。注释（行 50-54）说明此为 workspace 根 release 档，且不设 `panic="abort"` 以保留前台服务进程。

### 入口与插件注册 (`android/src-tauri/src/lib.rs`)

- **`#[cfg_attr(mobile, tauri::mobile_entry_point)]`**（行 32）：移动端入口属性。
- **`pub fn run()`**（行 33-141）：安卓端 Tauri 入口函数，本文件唯一公开函数。
  - **Builder 初始化**（行 34）：`tauri::Builder::default()`。
  - **`#[cfg(mobile)]` 插件注册块**（行 35-42）：
    - 行 37：`tauri_plugin_campus_network_bind::init()`（WLAN 绑定）
    - 行 38：`tauri_plugin_campus_keystore::init()`（Keystore 加密）
    - 行 39：`tauri_plugin_campus_monitor_service::init()`（前台服务）
    - 行 40：`tauri_plugin_biometric::init()`（注释指明该 crate 内部 `#![cfg(mobile)]`，host 编译为空）
    - 行 41：`tauri_plugin_notification::init()`
    - 行 42：`manage(android_state::AndroidState::default())`
  - **`#[cfg(not(mobile))]` 块**（行 43-44）：仅 `manage(android_state::AndroidState::default())`（行 44），不注册任何插件。
  - **通用插件与 setup**（行 45-66）：
    - 行 46：注册 `tauri_plugin_opener::init()`
    - 行 47-66：`setup` 闭包：计算日志目录 `app_data_dir/logs`（行 49-53），调用 `campus_login_lib::infra::logger::init_logger`（行 56），执行 `monitor_loop::run_startup_tasks`（行 60），异步清理残留 tmp 配置（行 63）。
  - **Invoke Handler**（行 68-130）：注册全部命令处理器（见下节）。
  - **构建与运行**（行 131-140）：`build(...).run(...)`，并在 `RunEvent::ExitRequested`（行 137）时调用 `api.prevent_exit()`（行 138）实现退出护栏。

### 模块声明与分工 (`lib.rs` 行 1-28)

顶部注释（行 1-11）与 `mod` 声明（行 12-28）定义以下模块：

- `campus_detect`（行 12，注释行 3-4）：校园网探针（`check_campus_status` 等）
- `android_state`（行 13，注释行 10）：进程态（源 IP 缓存/配置内存态/监控循环句柄）
- `config_state`（行 14，注释行 5）：全量配置（Keystore 加密落盘/掩码出口）
- `cpu_affinity`（行 15）：绑核逻辑
- `identity_gate`（行 16，注释行 8）：验证门 TTL
- `login_history`（行 17，注释行 8）：登录历史
- `protocol_cmds`（行 18，注释行 3）：登录/注销/Portal 探测/WiFi 绑定
- `self_service_cmds`（行 19，注释行 6）：自助服务七网络命令 + 生物识别验证门
- `vpn_cmds`（行 20，注释行 7）：WebVPN 凭据命令与自助服务校外自动路由
- `monitor_loop`（行 21）：监控循环
- `account_cmds`（行 22）：账户命令
- `system_cmds`（行 23）：系统命令
- `quality_cmds`（行 24）：网络质量命令
- `quality_history`（行 25，注释行 9）：网络质量历史落盘
- `update_cmds`（行 26）：更新命令
- `face_model_cmds`（行 27）：人脸模型命令
- `battery_cmds`（行 28）：电池优化命令

### 公开命令面（按模块归类, `lib.rs` 行 68-129）

以下命令通过 `tauri::generate_handler!` 宏注册，行号对应 `invoke_handler` 列表：

**Protocol (`protocol_cmds`)** – 行 69-76：
- `ping_test`（行 69）
- `bind_to_wifi`（行 70）
- `accept_wifi_network`（行 71）
- `get_avoid_bad_wifi_status`（行 72）
- `restore_written_settings`（行 73）
- `do_login`（行 74）
- `do_logout`（行 75）
- `check_portal_status`（行 76）

**Campus Detect (`campus_detect`)** – 行 77-78：
- `check_campus_status`（行 77）
- `request_wifi_ssid_permission`（行 78）

**Config (`config_state`)** – 行 79-80：
- `get_config`（行 79）
- `save_config`（行 80）

**Self Service (`self_service_cmds`)** – 行 81-88：
- `verify_biometric_identity`（行 81）
- `bind_operator`（行 82）
- `query_bind_status`（行 83）
- `query_self_dashboard`（行 84）
- `query_self_online_log`（行 85）
- `self_offline_session`（行 86）
- `change_self_password`（行 87）
- `reveal_operator_credential`（行 88）

**VPN (`vpn_cmds`)** – 行 89-94：
- `vpn_save_credential`（行 89）
- `vpn_credential_status`（行 90）
- `vpn_clear_credential`（行 91）
- `set_self_service_via_vpn`（行 92）
- `vpn_test_connection`（行 93）
- `vpn_verify_credential`（行 94）

**Account (`account_cmds`)** – 行 95-101：
- `list_accounts`（行 95）
- `switch_account`（行 96）
- `rename_account`（行 97）
- `save_current_as_account`（行 98）
- `create_account`（行 99）
- `delete_account`（行 100）
- `get_active_account`（行 101）

**System (`system_cmds`)** – 行 102-108：
- `get_init_data`（行 102）
- `get_logs`（行 103）
- `clear_logs`（行 104）
- `get_log_retention_days`（行 105）
- `set_log_retention_days`（行 106）
- `get_debug_mode`（行 107）
- `set_debug_mode`（行 108）

**Monitor (`monitor_loop`)** – 行 109-116：
- `start_background_check`（行 109）
- `stop_background_check`（行 110）
- `trigger_background_check`（行 111）
- `get_background_status`（行 112）
- `get_boot_autostart`（行 113）
- `set_boot_autostart`（行 114）
- `get_notification_enabled`（行 115）
- `set_notification_enabled`（行 116）

**Battery (`battery_cmds`)** – 行 117-120：
- `get_battery_optimization_info`（行 117）
- `request_ignore_battery_optimizations`（行 118）
- `open_vendor_battery_settings`（行 119）
- `open_notification_settings`（行 120）

**Quality (`quality_cmds`)** – 行 121-123：
- `check_network_quality`（行 121）
- `start_latency_test`（行 122）
- `stop_latency_test`（行 123）

**Update (`update_cmds`)** – 行 124-127：
- `check_update`（行 124）
- `download_update`（行 125）
- `get_mirror_urls`（行 126）
- `install_update`（行 127）

**Face Model (`face_model_cmds`)** – 行 128-129：
- `face_models_state`（行 128）
- `face_models_download`（行 129）

## Architecture

```text
┌──────────────────────────────────────────────┐
│ android/src-tauri lib.rs run() (行33-141)     │
│ ┌────────────────┐   ┌──────────────────────┐ │
│ │ Tauri Builder  │   │ Invoke Handlers      │ │
│ │ [mobile] plugins│─►│ 命令 行69-129        │ │
│ │ (行37-41)      │   │ (protocol, vpn...)   │ │
│ │ [not mobile]   │   └──────────┬───────────┘ │
│ │ only manage   │              │             │
│ └──────┬────────┘              ▼             │
│        ▼             ┌──────────────────────┐ │
│ ┌──────────────────────────────────────────┐ │
│ │ Shared: AndroidState(42,44),             │ │
│ │ campus-login(Cargo 31), logger(56)      │ │
│ └──────────────────────────────────────────┘ │
│ Exit Guard: prevent_exit() 行137-138         │
└──────────────────────────────────────────────┘
```

## Data Flow

1. **Startup**：`run()`（行 33）被调用 -> 依据 `#[cfg(mobile)]`（行 35）注册安卓插件 -> `setup` 阶段（行 47）初始化日志目录并调用 `init_logger`（行 56）、启动后台任务 `monitor_loop::run_startup_tasks`（行 60）、异步清理孤儿 tmp 文件（行 63）。

2. **Runtime**：前端通过 Tauri Commands 调用已注册命令（如 `protocol_cmds::do_login` 行 74），后端逻辑可能调用 `campus-login` 核心（`Cargo.toml` 行 31）与 `config_state` 读写。

3. **Background**：`monitor_loop` 模块（行 21）在后台运行检测/定时任务，不受 UI 关闭影响（得益于退出护栏）。

4. **Shutdown**：用户划掉任务 -> `RunEvent::ExitRequested`（行 137）触发 -> `api.prevent_exit()`（行 138）-> 进程保留，前台服务继续；仅系统设置彻底退出才终止。

## Connections

[[verification-baseline]]、[[tauri-android-build-report-path-missing]]

## Known Issues

- **Host Build Failure**：在 Host 环境（非 Android 目标）下，`cargo check` 和 `cargo test` **仍会失败**。
  - **原因**：`lib.rs` 中 `#[cfg(mobile)]` 块（行 35-42）在 Host 编译时被忽略，仅执行 `#[cfg(not(mobile))]`（行 43-44）空插件管理；但插件 crate 如 `tauri-plugin-campus-network-bind`（`Cargo.toml` 行 34）、`tauri-plugin-campus-keystore`（`Cargo.toml` 行 37）、`tauri-plugin-campus-monitor-service`（`Cargo.toml` 行 40）等是安卓专属 path 依赖，其内部使用安卓 JNI/NDK 接口，在 Host 缺少对应目标时编译报错，导致整体 `cargo check` 失败。`tauri_plugin_biometric`（`lib.rs` 行 40 注册，`Cargo.toml` 行 43）注释声明自身 `#![cfg(mobile)]` 在 host 编译为空，不是失败主因。
  - **结论**：基线状态下 Host `cargo check` 不可用，不要用它判断安卓改动正确性。
  - **解决方案**：必须使用交叉编译验证：`cargo check --target aarch64-linux-android --all-targets`（需配置 NDK 工具链），或使用构建脚本出包。功能测试需注入 Mock。
