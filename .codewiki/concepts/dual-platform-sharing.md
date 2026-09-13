---
title: 双端同构与协议单点共享
type: concept
source_files:
  - tauri-app/src-tauri/Cargo.toml
  - android/src-tauri/Cargo.toml
  - tauri-app/src-tauri/src/lib.rs
  - tauri-app/src-tauri/src/main.rs
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/platform/mod.rs
  - tauri-app/src-tauri/src/auth/mod.rs
  - tauri-app/src-tauri/src/network/mod.rs
  - tauri-app/src-tauri/src/config/mod.rs
  - tauri-app/src-tauri/src/infra/mod.rs
  - tauri-app/src-tauri/src/infra/logger.rs
  - tauri-app/src-tauri/src/infra/lifecycle.rs
  - tauri-app/src-tauri/src/account/crypto.rs
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/android_state.rs
  - android/src-tauri/src/cpu_affinity.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/protocol_cmds.rs
  - android/src-tauri/build.rs
  - android/plugins/keystore/Cargo.toml
  - android/plugins/keystore/src/lib.rs
  - android/plugins/network-bind/src/lib.rs
  - android/plugins/foreground-service/src/lib.rs
tags: [概念, 双端同构, 协议共享, cfg 门控, cargo path 依赖]
---

## Overview

项目是 Windows 桌面端与安卓端的同构应用：登录 / 注销 / Portal 探测 / 自助服务 / 网络质量等协议实现**只存在于桌面 crate**（`tauri-app/src-tauri`，package `campus-login`，lib `campus_login_lib`），安卓以 Cargo path 依赖复用，禁止复制。桌面侧与平台耦合的模块用 `#[cfg(desktop)]` 整体门控，安卓侧只补三件事：平台探针、状态管理、命令面包装。前端是两套独立复刻树，通过同形的 `tauriApi.ts` 接口保持行为一致。

## 机制说明

### 共享边界：Cargo path 依赖的实际配置

安卓后端 `android/src-tauri/Cargo.toml` 的依赖段：

```toml
# 协议核心:与桌面端单点共享(阶段 3 收敛为 git 依赖 + tag)
campus-login = { path = "../../tauri-app/src-tauri" }

tauri-plugin-campus-network-bind = { path = "../plugins/network-bind" }
tauri-plugin-campus-keystore = { path = "../plugins/keystore" }
tauri-plugin-campus-monitor-service = { path = "../plugins/foreground-service" }
```

对应桌面 `tauri-app/src-tauri/Cargo.toml` 的 `[lib] name = "campus_login_lib"`。安卓代码里所有 `campus_login_lib::...` 引用都是这份共享 crate（例：`monitor_loop.rs:583` 用 `campus_login_lib::auth::portal::check_portal_full`，`quality_cmds.rs:9` 用 `campus_login_lib::network::quality::check_network_quality_async`）。

三个手写 Tauri 插件都以 `#![cfg(mobile)]` 开头（`keystore/src/lib.rs`、`network-bind/src/lib.rs:1`、`foreground-service/src/lib.rs:1`），在 host 编译为空。

### 哪些是共享的

`tauri-app/src-tauri/src/lib.rs` 是共享 crate 的可见面（安卓只能看到这个文件导出的东西）：

| 模块 | 状态 | 说明 |
|---|---|---|
| `account` | 共享 | 仅 `crypto` 子模块，内部再按 `cfg(target_os = "windows")` 分支（`lib.rs:3`） |
| `auth` | 部分共享 | `failure_tracker` / `portal` / `protocol` 共享；`dual_adapter_executor` / `session` / `service` 为 `#[cfg(desktop)]`（`auth/mod.rs:6-10`） |
| `config` | 共享 | `model` / `persist` / `validate` 全部跨平台 |
| `infra` | 共享 | `state` / `logger` / `notification` / `lifecycle` / `events` / `command_context` / `task_manager` / `async_util`（`infra/mod.rs`） |
| `network` | 部分共享 | `adapter` / `adapter_cache` / `client` / `dhcp` / `discovery` / `dns` / `dns_setup` / `quality` / `subnet` / `timing` 跨平台；`dhcp_release_renew_all` 与 `dhcp_release_renew_single` 为 `cfg(target_os = "windows")`（`network/mod.rs:17-22`），非 Windows 仅有 stub 版 `dhcp_release_renew_single`（`network/mod.rs:24-25`），`enable_adapter` 为 `cfg(desktop)`（`network/mod.rs:45`） |
| `platform` | 部分共享 | 仅 `console_output` 跨平台（GBK 解码，安卓也要）；`autostart` / `dns_config` / `elevation` / `gpu` / `helper_spawn` / `identity` 为 `cfg(desktop)`，`toast` 为 `cfg(all(desktop, target_os = "windows"))`（`platform/mod.rs:5-15`） |
| `self_service` | 共享 | 纯 HTTP 协议 |
| `app` / `commands` / `helper` / `monitor` / `update` | **桌面专属** | `lib.rs:11-19` 五个 `#[cfg(desktop)] pub mod` |

`app/startup.rs:165-172`、`commands/system.rs:121`、`update/updater.rs:374` 里的 `env!("APP_VERSION")` 由**桌面专属** `build.rs` 从 `tauri.conf.json` 注入；安卓 `build.rs` 只有一行 `tauri_build::build()`，所以安卓只能用 `env!("CARGO_PKG_VERSION")`（`system_cmds.rs:14`、`update_cmds.rs:216/314`）。

### 哪些是各端独有

**桌面独有（cfg 门控 + 平台插件）**：

```rust
// app/startup.rs:24-49 桌面插件装配
.plugin(tauri_plugin_shell::init())
.plugin(tauri_plugin_notification::init())
.plugin(tauri_plugin_autostart::init(...))
.plugin(tauri_plugin_global_shortcut::Builder::new()...)
.plugin(tauri_plugin_single_instance::init(...))
```

对应 `Cargo.toml` 的 `[target.'cfg(not(any(target_os = "android", target_os = "ios")))'.dependencies]`：shell / autostart / global-shortcut / single-instance。另有 `[target.'cfg(target_os = "windows")'.dependencies]`（winreg、webview2-com、windows 全套）；托盘、WebView2 崩溃恢复、GPU/刷新率探测、提权 helper、全局快捷键、DPI/窗口行为全在桌面侧。

**安卓独有**：

| 能力 | 位置 | 说明 |
|---|---|---|
| Keystore 加解密 | `plugins/keystore/`，桥在 `config_state.rs:132-163` 的 `CryptoBridge::from_app` | `#[cfg(mobile)]` 走真插件，`#[cfg(not(mobile))]` 返回 `Err("Keystore 仅移动端可用")`，使 host 测试可注入假桥 |
| 前台服务保活 | `plugins/foreground-service/`（`CampusMonitorService`） | `start_monitor` / `begin_probe_window` / `get_power_state` / `set_boot_autostart` / `install_apk` 等（`foreground-service/src/lib.rs:33-92`） |
| 进程绑 WLAN | `plugins/network-bind/`（`CampusNetworkBind`） | `bind_to_wifi` / `accept_wifi_network` / WiFi 变化 Channel 监听（`network-bind/src/lib.rs:19-54`） |
| 绑小核省电 | `cpu_affinity.rs` | 解析 `cpuinfo_max_freq` 分簇，最低频簇视为小核；频率差 <30% 视为同构不绑（`cpu_affinity.rs:15-19`） |
| 系统生物识别 | `identity_gate.rs` + 官方 `tauri-plugin-biometric` | TTL 600s，时钟回拨视为过期（`identity_gate.rs:8/23-33`） |
| 电池优化白名单 | `battery_cmds.rs`（3 条命令） | 桌面无对应物 |
| SoC/性能分档 | `system_cmds.rs:71` `get_soc_info` | tier 0/1/2/3 驱动帧率档 |
| 监控状态机 | `monitor_loop.rs` | 桌面由 `monitor/` 模块群承担 |
| 进程态缓存 | `android_state.rs` | 仅 `cached_source_ip` + `config` 内存态两个字段 |

安卓专属依赖：`[target.'cfg(target_os = "android")'.dependencies]` 的 `android_system_properties` 与 `libc`。

### 双端同步的硬性约定

项目 `AGENTS.md` 第 3 条（"通用改进双端同步"）规定：前端 UI / 交互行为、文案与 i18n、图标与看板娘素材、IPC 命令面、配置字段与默认值、事件与日志类型等**通用改进必须同一次提交双端各改一份**，提交前用 `git diff --stat` 自检是否同时触及 `tauri-app/` 与 `android/`。两类例外：

1. **协议实现单点存在桌面 crate**：安卓经 Cargo path 依赖自动继承，**禁止复制**（`lib.rs:1` 与 `android/src-tauri/Cargo.toml` 注释均明确）。
2. **平台专属能力各端自理**：DPAPI / Keystore、托盘 / 前台服务等。

### 两端命令面差异

| 维度 | 桌面 | 安卓 |
|---|---|---|
| 注册条数 | 56（`app/startup.rs:63-120`） | 48（`lib.rs:55-104`） |
| 独有命令 | `show_window` / `minimize_window` / `close_window` / `open_external` / `cancel_auto_exit` / `render_heartbeat` / `get_gpu_info` / `check_dns_doh_status` / `setup_dns_doh` / 适配器 4 条 / `dhcp_*` 3 条 | `ping_test` / `bind_to_wifi` / `accept_wifi_network` / `detect_campus` / `get_soc_info` / 电池 3 条 / `get_boot_autostart` / `set_boot_autostart` |
| 同名异实现 | `get_config` 返回 `Config` | `get_config` 返回掩码后的 `serde_json::Value`（`config_state.rs:296-309`） |
| 语义等价异名 | `verify_windows_identity`、`get/set_auto_launch` | `verify_biometric_identity`、`get/set_boot_autostart` |
| 配置字段数 | 44（`config/model.rs:10-104`） | 35（`config_state.rs:15-72`） |
| 双适配器 | 支持（`dual_adapter` / `adapter1` / `adapter2`） | 不支持，`Settings` 无这些字段 |

## 关键约束

- **协议逻辑禁止复制到安卓**：唯一合法复用方式是 path 依赖（`android/src-tauri/Cargo.toml`），`lib.rs:1` 注释 "协议核心 path 依赖 campus-login 单点共享"。
- **共享 crate 的新模块必须判断安卓可见性**：加进 `lib.rs` 顶层即在安卓编译（可能因 Windows API 失败），必须 `#[cfg(desktop)]` 门控或保证跨平台。
- **安卓插件必须 `#![cfg(mobile)]`**：否则 host `cargo test` 会因缺 Android SDK 符号失败（三个插件首行均如此）。
- **Keystore host 降级必须显式可测**：`CryptoBridge::from_app` 的 `#[cfg(not(mobile))]` 分支返回 Err，测试用 `fake_bridge()` 注入（`config_state.rs:367-376`）。
- **改包名 = 密码密文作废**：Keystore key alias 固定 `campus_login_master`（`KeystorePlugin.kt:34`），密钥按包名隔离；`android/src-tauri/tauri.conf.json` 的 `identifier` 为 `com.campuslogin.client`，桌面为 `com.campus.login`。
- **安卓日志目录必须走 `app_data_dir`**：`infra/logger.rs:375-383` 的 `cfg(target_os = "android")` 分支，因为安卓 `current_exe` 指向只读 APK 安装目录。
- **配置字段双端同步**：新增配置字段要同时改 `config/model.rs`（camelCase `rename`）与 `config_state.rs`（`#[serde(rename_all = "camelCase")]`），并各自补迁移逻辑（桌面 `validate.rs:125-132`，安卓 `config_state.rs:241-266`）。

## Data Flow

```text
android/src-tauri (campus-login-android)
  ├─ protocol_cmds / campus_detect / config_state / monitor_loop / *_cmds   ← 安卓自有：平台探针 + 状态 + 命令包装
  ├─ plugins/keystore, network-bind, foreground-service                     ← Kotlin 侧 + Rust 桥
  └─ campus-login = { path = "../../tauri-app/src-tauri" } ─────┐
                                                                 │
tauri-app/src-tauri (campus-login, lib campus_login_lib)  ◄──────┘
  ├─ 共享面: auth(3/6 文件) · config · infra · network(大部) · platform::console_output · self_service
  └─ #[cfg(desktop)] 面: app · commands · helper · monitor · update · platform 其余 · auth::session/service
                                                                 │
                                                         桌面二进制 main.rs
```

## Connections

- [[ipc-command-surface]] — 两端命令注册点与命名对齐规则
- [[android-backend]] — 安卓侧模块分工与三个插件的实现细节
- [[android-plugins]] — Keystore / network-bind / foreground-service 插件专章
- [[desktop-platform]] — 桌面 `platform` 模块的 Windows 专属能力
- [[config-and-persistence]] — 双端配置模型字段差异与迁移
- [[security-model]] — DPAPI 与 Android Keystore 的对应关系与验证门差异
- [[background-check-and-auto-login]] — 桌面 `monitor/` 与安卓 `monitor_loop.rs` 的分工
- [[desktop-app-lifecycle]] — 桌面启动装配与插件初始化

## Known Issues

- **`Cargo.toml` 注释与实现不一致**：`android/src-tauri/Cargo.toml` 写着"阶段 3 收敛为 git 依赖 + tag"，实际仍是相对路径 `../../tauri-app/src-tauri`，路径一旦重命名即断。
- **版本号有两条独立来源**：桌面 `env!("APP_VERSION")` 由 `build.rs` 从 `tauri.conf.json` 注入（`tauri-app/src-tauri/build.rs:29`），安卓 `env!("CARGO_PKG_VERSION")` 取自 `Cargo.toml`。当前两边都是 `2.3.6`（两个 `Cargo.toml` `version` 字段 + 两个 `tauri.conf.json`），发布时需人工四处同步，无自动校验。
- **两端配置面是"交集 + 双向差集"而非子集**：44（桌面）与 35（安卓）字段中只有 31 个共有；桌面独有 13 个（`adapter1`/`adapter2`/`dual_adapter`、`minimize_to_tray`/`hidden_start`/`auto_launch`、`auto_exit_after_login`/`auto_exit_on_online`、`campus_exit_on_fail`/`campus_exit_start_minutes`/`campus_exit_end_minutes`、`skip_sha256_when_missing`、`config_version`），安卓独有 4 个（`allow_2d_face_verify`、`background_check_idle_interval`、`enable_boot_autostart`、`config_schema_version`）。比较 `config/model.rs:10-104` 与 `config_state.rs:15-72` 可见，"双端同步"实际需要双向增量维护。
- **共享 crate 的 `infra::lifecycle` 带桌面语义但被安卓编译**：`lifecycle.rs:6` 只把 `CANCEL_EXIT_SHORTCUT` 做了 `#[cfg(desktop)]`，其余循环退出逻辑保留，安卓调用点为空实现（`lifecycle.rs:305-306`），属"编得过但用不上"的死代码。
- **`config_state::current_settings` 标 `#[allow(dead_code)]` 却已被大量使用**：`config_state.rs:341` 的注释"Task 2 协议命令面接线"已过期，实际 `monitor_loop.rs`、`self_service_cmds.rs` 等都在调用。
- **双端同步依赖人工纪律**：`AGENTS.md` 第 3 条要求同提交双端各改一份，但仓库中没有 CI 检查；`CODE_WIKI.md:289-290` 只把它写成约定，无法自动拦截只改一端的提交。
