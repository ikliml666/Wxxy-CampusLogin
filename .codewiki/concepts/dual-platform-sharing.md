---
title: 双端同构与协议单点共享
type: concept
source_files:
  - tauri-app/src-tauri/Cargo.toml
  - android/src-tauri/Cargo.toml
  - tauri-app/src-tauri/src/lib.rs
  - tauri-app/src-tauri/src/main.rs
  - tauri-app/src-tauri/build.rs
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/platform/mod.rs
  - tauri-app/src-tauri/src/auth/mod.rs
  - tauri-app/src-tauri/src/network/mod.rs
  - tauri-app/src-tauri/src/config/mod.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/validate.rs
  - tauri-app/src-tauri/src/commands/system.rs
  - tauri-app/src-tauri/src/update/updater.rs
  - tauri-app/src-tauri/src/infra/mod.rs
  - tauri-app/src-tauri/src/infra/logger.rs
  - tauri-app/src-tauri/src/infra/lifecycle.rs
  - tauri-app/src-tauri/src/account/crypto.rs
  - tauri-app/frontend/src/settings/types.ts
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/android_state.rs
  - android/src-tauri/src/cpu_affinity.rs
  - android/src-tauri/src/identity_gate.rs
  - android/src-tauri/src/login_history.rs
  - android/src-tauri/src/quality_history.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/protocol_cmds.rs
  - android/src-tauri/src/self_service_cmds.rs
  - android/src-tauri/src/account_cmds.rs
  - android/src-tauri/src/campus_detect.rs
  - android/src-tauri/src/battery_cmds.rs
  - android/src-tauri/src/quality_cmds.rs
  - android/src-tauri/src/system_cmds.rs
  - android/src-tauri/src/update_cmds.rs
  - android/src-tauri/build.rs
  - android/plugins/keystore/Cargo.toml
  - android/plugins/keystore/src/lib.rs
  - android/plugins/network-bind/src/lib.rs
  - android/plugins/foreground-service/src/lib.rs
  - android/frontend/src/settings/types.ts
tags: [概念, 双端同构, 协议共享, cfg 门控, cargo path 依赖]
---

## Overview

项目是 Windows 桌面端与安卓端的同构应用：登录 / 注销 / Portal 探测 / 配置校验与迁移 / 夜间切换判定 / 自助服务 / 网络质量分段计时等协议实现**只存在于桌面 crate**（`tauri-app/src-tauri`，package `campus-login`，lib `campus_login_lib`，版本 2.3.9），安卓壳工程（`android/src-tauri`，package `campus-login-android`，lib `campus_login_android_lib`）以 Cargo path 依赖复用同一份代码，禁止复制。桌面侧与平台耦合的模块用 `#[cfg(desktop)]` 整体门控，安卓侧只补四件事：平台探针（三个自研插件 + `campus_detect` + `monitor_loop` + `cpu_affinity`）、状态管理（`android_state` / `config_state`）、命令面包装（`*_cmds`，共 53 条命令）、双端同构的落盘记录（`login_history` / `quality_history`）。前端是两套独立复刻树，通过同形的类型契约（`frontend/src/settings/types.ts` 双端各一份）保持行为一致。

## 机制说明

### 共享边界：Cargo path 依赖的实际配置

安卓后端 `android/src-tauri/Cargo.toml:33-43` 的依赖段：

```toml
# 协议核心:与桌面端单点共享(阶段 3 收敛为 git 依赖 + tag)          # :33
campus-login = { path = "../../tauri-app/src-tauri" }               # :34

tauri-plugin-campus-network-bind = { path = "../plugins/network-bind" }          # :37
tauri-plugin-campus-keystore = { path = "../plugins/keystore" }                  # :40
tauri-plugin-campus-monitor-service = { path = "../plugins/foreground-service" } # :43
```

对应桌面 `tauri-app/src-tauri/Cargo.toml:8-10` 的 `[lib] name = "campus_login_lib"`（两 crate 的 `version` 均为 2.3.9）。共享 crate 编到安卓目标所需的平台依赖（socket2 / libc / network-interface，供 `bound_socket` 的 SO_BINDTODEVICE 探测）由**桌面** `Cargo.toml:43-50` 的 `cfg(target_os = "android")` 依赖段声明——path 依赖连目标专属依赖一起继承。安卓代码里所有 `campus_login_lib::...` 引用都是这份共享 crate，例如：`protocol_cmds.rs:63-87` 登录内核 `run_login` 在 `spawn_blocking` 中调用 `auth::protocol::do_login_with_retry`（`:76`），`protocol_cmds.rs:112` 注销用 `do_logout_with_retry`，`protocol_cmds.rs:153-155` Portal 探测用 `auth::portal::check_portal_full`，`quality_cmds.rs:27-37` 质量检测用 `network::quality::check_network_quality_async`，`system_cmds.rs:30-65` 日志与保留期命令直通 `infra::logger`，`monitor_loop.rs:856` 夜间运营商切换判定复用 `config::night_switch::evaluate_night_switch`、`monitor_loop.rs:1233-1234` 夜切验证复用 `config::night_switch::parse_chkstatus/uid_matches`，`self_service_cmds.rs:62-271` 七个自助命令包装 `campus_login_lib::self_service` 纯 HTTP 协议（命令名/参数/返回形状与桌面同构），`campus_detect.rs:48` 网关/Portal 可达性用 `bound_socket::tcp_connect_bounded`。

三个手写 Tauri 插件都以 `#![cfg(mobile)]` 开头（`keystore/src/lib.rs:1`、`network-bind/src/lib.rs:1`、`foreground-service/src/lib.rs:1`），在 host 编译为空。

release profile 各归各的 workspace：安卓壳工程是 workspace 根，`[profile.release]` `lto="fat"`（`android/src-tauri/Cargo.toml:56-60`）；桌面的 `lto="thin"`（`tauri-app/src-tauri/Cargo.toml:89-94`）不随 path 依赖传递到安卓侧编译。

版本注入两条路：桌面专属 `build.rs:12-32` 从 `tauri.conf.json` 解析 version 并注入 `APP_VERSION`（`build.rs:37` `println!("cargo:rustc-env=APP_VERSION={version}")`），`startup.rs:189` 与 `commands/system.rs:124` 用 `env!("APP_VERSION")`；安卓 `build.rs:1-3` 只有一行 `tauri_build::build()`，所以安卓只能用 `env!("CARGO_PKG_VERSION")`（`system_cmds.rs:15`、`update_cmds.rs` 的 `check_update_inner` 内 `:278`）。

### 哪些是共享的

`tauri-app/src-tauri/src/lib.rs` 是共享 crate 的可见面（安卓只能看到这个文件导出的东西）：

| 模块 | 状态 | 说明 |
|---|---|---|
| `account` | 共享 | 仅 `crypto` 子模块，内部再按 `cfg(target_os = "windows")` 分支（`lib.rs:2`）：Windows 走 DPAPI（`crypto.rs:1-130`），非 Windows stub 返回 `Err("加密存储仅桌面端支持")`（`crypto.rs:133-141`）；安卓密码加密不走此模块，走 Keystore 插件 |
| `auth` | 部分共享 | `failure_tracker` / `portal` / `protocol` 共享（`auth/mod.rs:1-3`）；`dual_adapter_executor` / `session` / `service` 为 `#[cfg(desktop)]`（`auth/mod.rs:6-11`） |
| `config` | 共享 | `model` / `night_switch` / `outbound_switch` / `persist` / `schedule` / `validate` 全部跨平台（`config/mod.rs:1-6`，re-export `:8`）；夜间运营商切换判定与 chkstatus 解析（`night_switch::evaluate_night_switch` / `parse_chkstatus` / `uid_matches`）被安卓 `monitor_loop.rs:856/1233-1234`、`config_state.rs:26-28`（注释声明复用）单点消费 |
| `infra` | 共享 | `state` / `logger` / `notification` / `lifecycle` / `events` / `command_context` / `task_manager` / `async_util`（`infra/mod.rs:1-8`）；安卓日志命令直通 `infra::logger`（`system_cmds.rs:30-65`），`get_log_dir` 的 android 分支落 `app_data_dir/logs`（`infra/logger.rs:385-394`） |
| `network` | 部分共享 | `adapter` / `adapter_cache` / `bound_socket` / `client` / `dhcp` / `discovery` / `dns` / `dns_setup` / `quality` / `subnet` / `timing` 跨平台（`network/mod.rs:1-13`）；`bound_socket` 纯函数全平台编译、socket 探测仅 Android（`network/mod.rs:5`）；`dhcp_release_renew_all` / `dhcp_release_renew_single` 为 `cfg(windows)`（`network/mod.rs:22-23`），非 Windows 仅有 stub 版 `dhcp_release_renew_single`（`network/mod.rs:25-26`）；`enable_adapter` 为 `cfg(desktop)`（`network/mod.rs:48-49`） |
| `platform` | 部分共享 | 仅 `console_output` 跨平台（GBK 解码，协议响应/子网查询安卓同样需要，`platform/mod.rs:2`）；`autostart` / `ecoqos` / `dns_config` / `elevation` / `gpu` / `helper_spawn` / `task_proxy` / `identity` / `metric` / `best_route` / `icmp_probe` / `rtss_compat` / `toast` 全部桌面门控（`platform/mod.rs:5-36`） |
| `self_service` | 共享 | 纯 HTTP 协议；桌面 `startup.rs:62-128` 注册其中 7 条，安卓 `self_service_cmds.rs:62-271` 同名同参 7 条 |
| `app` / `commands` / `helper` / `monitor` / `update` | **桌面专属** | `lib.rs:10-20` 五个 `#[cfg(desktop)] pub mod`，安卓编译期不可见 |

### 哪些是各端独有

**桌面独有（cfg 门控 + 平台插件）**：

```rust
// app/startup.rs:25-48 桌面插件装配
.plugin(tauri_plugin_shell::init())
.plugin(tauri_plugin_notification::init())
.plugin(tauri_plugin_autostart::init(...))
.plugin(tauri_plugin_global_shortcut::Builder::new()...)
.plugin(tauri_plugin_single_instance::init(...))
```

对应 `tauri-app/src-tauri/Cargo.toml:53-57` 的 `cfg(not(any(target_os = "android", target_os = "ios")))` 依赖段；另有 `cfg(target_os = "windows")` 段的 winreg / webview2-com / windows 全套（`Cargo.toml:59-87`）。桌面还有：helper 子进程（`--helper-task` 拦截 `main.rs:30-32`、参数解析 `main.rs:37-46`）、RTSS 预初始化（`main.rs:51-52`）、WebView2 crashdumps 启动参数（`main.rs:56-68`）、托盘 / WebView2 崩溃恢复订阅 / GPU 预热线程（`startup.rs:214/233-235/242-250`）、轻量化模式（`RunEvent::ExitRequested` 拦截 `startup.rs:135-149` + `notify_window_ready` 就绪门 `commands/system.rs:284-288`）、DPAPI（`crypto.rs:1-130`）、exe 更新链（`update/updater.rs:1-651`）、DHCP 释放续租 / 计划任务提权代理 / EcoQoS 等 `platform` 桌面模块。

**安卓独有**：

| 能力 | 位置 | 说明 |
|---|---|---|
| Keystore 加解密 | `plugins/keystore`（`CampusKeystore::encrypt/decrypt`，base64(iv+ciphertext)，`keystore/src/lib.rs:30-39`） | 桥在 `config_state.rs:187-223` `CryptoBridge`：mobile 走真插件，host 注入 Err |
| 前台服务保活 | `plugins/foreground-service`（`CampusMonitorService`，`foreground-service/src/lib.rs:31-98`） | `start_monitor` / `begin_probe_window` / `get_power_state` / `set_boot_autostart` / `install_apk` 等；只负责"让进程活着"，监控/自动重登业务全在 Rust 侧 `monitor_loop`（协议不双实现） |
| 进程绑 WLAN | `plugins/network-bind`（`CampusNetworkBind`，`network-bind/src/lib.rs:17-107`） | 11 个方法：`bind_to_wifi` / `accept_wifi_network` / `start_wifi_watcher` / `stop_wifi_watcher` / `get_wifi_ssid` / `request_wifi_ssid_permission` / `report_wifi_unusable` / `ensure_avoid_bad_wifi` / `restore_avoid_bad_wifi` / `restore_written_settings` / `get_secure_settings_status` |
| 绑小核省电 | `cpu_affinity.rs:8-50` | 解析 `cpuinfo_max_freq` 按频率分簇，最低频簇视为小核；频率差 <30% 判同构不绑；`sched_setaffinity`（host 恒 false） |
| 系统生物识别 | `identity_gate.rs` + 官方 `tauri-plugin-biometric` | TTL 600s（`identity_gate.rs:8`），时钟回拨视为过期（`identity_gate.rs:23-33`） |
| 电池优化白名单 | `battery_cmds.rs`（4 条命令，`:19-102`） | 桌面无对应物；跳转候选表/降级链在 Kotlin 侧（`battery_cmds.rs:1-4`） |
| SoC/性能分档 | `system_cmds.rs:71-161` `get_soc_info` | `ro.soc.model` + 大核数（≥1.8GHz）+ 内存 → tier 0/1/2/3 |
| 监控状态机 | `monitor_loop.rs`（1607 行） | 桌面由 `monitor/` 模块群承担；安卓为巡检分档 + 夜切 + 定时动作 + 常驻通知 + 自动重登一体的 tick 循环（`run_check_once` `monitor_loop.rs:1245-1471`） |
| 登录/质量历史落盘 | `login_history.rs`（142 行）/ `quality_history.rs`（157 行） | 与桌面 `config::persist` 的历史记录同构：`LoginHistoryEntry` 头插上限 100 条、adapter 恒 `wlan0`（`login_history.rs:51-76`）；质量条目 camelCase 落盘、负延迟写 null（`quality_history.rs:52-75`），复用共享 crate `network::quality::NetworkQualityResult`（`quality_history.rs:5`） |
| 进程态缓存 | `android_state.rs:9-13` | 仅 `cached_source_ip` + `config` 内存态两个字段 |
| 常驻进程 | `lib.rs:114-121` | `RunEvent::ExitRequested` → `prevent_exit()`：划掉任务只关 UI，进程随前台服务常驻（tauri #15671 根因） |

安卓专属依赖：`android_system_properties` 与 `libc`（`Cargo.toml:28-31`）、官方 `biometric` / `opener`（`Cargo.toml:46-47`）。

### 双端同步的硬性约定

项目 `AGENTS.md` 第 3 条（"通用改进双端同步"）规定：前端 UI / 交互行为、文案与 i18n、图标与看板娘素材、IPC 命令面、配置字段与默认值、事件与日志类型等**通用改进必须同一次提交双端各改一份**，提交前用 `git diff --stat` 自检是否同时触及 `tauri-app/` 与 `android/`。两类例外：

1. **协议实现单点存在桌面 crate**：安卓经 Cargo path 依赖自动继承，**禁止复制**（`tauri-app/src-tauri/src/lib.rs:1` 与 `android/src-tauri/Cargo.toml:33` 注释均明确）。
2. **平台专属能力各端自理**：DPAPI / Keystore、托盘 / 前台服务等。

**账号体系（2026-09-14 落地）的同步归类**：

- **双端通用项**（同一次提交双端各改一份）：账号显示名与 id 分离（`displayName` 字段、`AccountItem {id, displayName}` 出站契约、`rename_account` 命令、`list_accounts` 返回结构、重名/校验语义）；输入账号密码自动建号（桌面 `save_config` 与安卓 `account_cmds.rs:393` `auto_create_account_for_current`——主配置落盘后由 `config_state.rs:448` 与巡检落盘 `monitor_loop.rs:972` 调用同一入口）；切换账号后 UI 刷新（返回必带 `activeAccount` + `config-changed` 事件补齐独立字段同步）——见 [[account-display-name-id-separation]] 与 [[account-switch-ui-state-desync]]。
- **桌面专属例外**：主/副适配器各自指定账号（`Config.adapter1Account`/`adapter2Account` 设备级字段 + 登录编排凭据副本 + 网络面板下拉）——双适配器与"网卡→账号"映射是桌面专属能力，见 [[adapter-account-binding]]；切账号合并明确排除这两个字段，故安卓 `Settings` 无需引入。

### 两端命令面差异

| 维度 | 桌面 | 安卓 |
|---|---|---|
| 注册条数 | 65（`app/startup.rs:62-128`） | 53（`lib.rs:57-111`） |
| 独有命令 | `show_window` / `minimize_window` / `close_window` / `open_external` / `cancel_auto_exit` / `render_heartbeat` / `get_gpu_info` / `export_config` / `import_config` / `export_diagnostics` / `notify_window_ready` / `get_auto_launch` / `set_auto_launch` / 通知开关 2 条 / DHCP 与适配器操作等 | `ping_test` / `bind_to_wifi` / `accept_wifi_network` / `get_avoid_bad_wifi_status` / `restore_written_settings` / `detect_campus` 系列 3 条 / `get_soc_info` / 电池 4 条 / `get_boot_autostart` / `set_boot_autostart` / `verify_biometric_identity` |
| 同名异实现 | `get_config` 返回 `Config` | `get_config` 返回掩码后的 `serde_json::Value`（`config_state.rs:406-418`） |
| 语义等价异名 | `verify_windows_identity`、`get/set_auto_launch` | `verify_biometric_identity`、`get/set_boot_autostart` |
| 配置字段数 | 60（`config/model.rs:10-172`，`config_version = 5`） | 47（`config_state.rs:15-111`，`config_schema_version = 7`） |
| 双适配器 | 支持（`dual_adapter` / `adapter1` / `adapter2` / `adapter1Account` / `adapter2Account`） | 不支持，`Settings` 无这些字段 |

安卓 53 条的构成（2026-10 起命令包装按域拆分为独立文件）：`protocol_cmds` 8（`ping_test`/`bind_to_wifi`/`accept_wifi_network`/`get_avoid_bad_wifi_status`/`restore_written_settings`/`do_login`/`do_logout`/`check_portal_status`）+ `campus_detect` 3（`campus_detect.rs:217-283`，含 `request_wifi_ssid_permission`）+ `config_state` 2 + `self_service_cmds` 7（`self_service_cmds.rs:62-271`）+ `account_cmds` 6（`account_cmds.rs:136-343`）+ `system` 8 + `monitor_loop` 8 + `battery_cmds` 4（`battery_cmds.rs:19-102`）+ `quality` 3（`quality_cmds.rs:47-67`）+ `update` 4（`lib.rs:57-111`）。

## 关键约束

- **协议逻辑禁止复制到安卓**：唯一合法复用方式是 path 依赖（`android/src-tauri/Cargo.toml:34`），`lib.rs:1` 注释"跨平台协议核心:安卓端(path 依赖)唯一可见面"。
- **共享 crate 的新模块必须判断安卓可见性**：加进 `lib.rs` 顶层即在安卓编译（可能因 Windows API 失败），必须 `#[cfg(desktop)]` 门控或保证跨平台（现状：`lib.rs:2-8` 七个 `pub mod` 全部跨平台安全，`lib.rs:10-20` 五个桌面模块全部门控）。
- **安卓插件必须 `#![cfg(mobile)]`，JNI 阻塞调用必须 `spawn_blocking`**：否则 host `cargo test` 会因缺 Android SDK 符号失败（三个插件首行均如此）；`network-bind` 各方法 doc 注释显式标注"JNI 阻塞调用，调用方须走 spawn_blocking"（`network-bind/src/lib.rs:58/72/81/88/95/103`）。
- **密码三纪律**：落盘必加密——桌面 DPAPI / 安卓 AndroidKeyStore AES-GCM，且加密失败绝不落明文（安卓 `save_file` tmp+rename 原子写、加密失败直接返回 Err，`config_state.rs:252-283`）；日志/payload 不含 password（`protocol_cmds.rs:3`）；出站掩码唯一出口 `masked_for_display`（安卓 `config_state.rs:373-382`，桌面 `config/model.rs:315-319`）。
- **改包名 = 密码密文作废**：Keystore key alias 固定 `campus_login_master`（Kotlin 侧 `KeystorePlugin.kt:34`），密钥按包名隔离；`android/src-tauri/tauri.conf.json:5` 的 `identifier` 为 `com.campuslogin.client`，桌面为 `com.campus.login`（`tauri-app/src-tauri/tauri.conf.json:4`）。
- **安卓日志目录必须走 `app_data_dir`**：`infra/logger.rs:385-394` 的 `cfg(target_os = "android")` 分支，因为安卓 `current_exe` 指向只读 APK 安装目录。
- **配置字段双端同步**：新增配置字段要同时改 `config/model.rs`（60 字段）与 `config_state.rs`（47 字段，`#[serde(rename_all = "camelCase")]`），并各自补迁移逻辑（桌面 `validate.rs:87-183` 迁移链 v1→v5，`v1→v2` 在 `:130-137` 至 `v4→v5` 在 `:163-174`；安卓 `migrate_legacy_defaults` `config_state.rs:314-366` v0→v7，迁移结果落盘幂等；另有 `config_state.rs:291-293` 旧默认单值 SSID 载入时扩为三值）。

## Data Flow

```text
android/src-tauri (campus-login-android, lib campus_login_android_lib, 53 条命令)
  ├─ protocol_cmds / campus_detect / config_state / self_service_cmds / account_cmds
  │   / monitor_loop / battery_cmds / quality_cmds / system_cmds / update_cmds        ← 安卓自有：平台探针 + 状态 + 命令包装
  ├─ login_history / quality_history                                                  ← 双端同构落盘记录（adapter 恒 wlan0）
  ├─ plugins/keystore, network-bind, foreground-service                               ← Kotlin 侧 + Rust 桥（#![cfg(mobile)]）
  └─ campus-login = { path = "../../tauri-app/src-tauri" } ─────┐
                                                                 │
tauri-app/src-tauri (campus-login, lib campus_login_lib, v2.3.9) ◄┘
  ├─ 共享面: auth(3/6 文件) · config(6 子模块) · infra(8 子模块) · network(大部 + bound_socket) · platform::console_output · self_service · account::crypto(cfg 分支)
  └─ #[cfg(desktop)] 面: app · commands · helper · monitor · update · platform 其余 · auth::session/service/dual_adapter_executor
                                                                 │
                                                         桌面二进制 main.rs（65 条命令）
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

- **`Cargo.toml` 注释与实现不一致**：`android/src-tauri/Cargo.toml:33` 写着"阶段 3 收敛为 git 依赖 + tag"，实际仍是相对路径 `../../tauri-app/src-tauri`（`:34`），路径一旦重命名即断。
- **`self_service_cmds.rs` 头注释与实现不一致**：`self_service_cmds.rs:1` 写"六个命令包装"，实际注册 **7** 条（多出的 `verify_biometric_identity` `:62-66` 是安卓独有的验证门命令，前端 BiometricPrompt 成功后调用；桌面命令面无对应物，桌面用 `verify_windows_identity`）。
- **版本号五处人工同步**：桌面 `env!("APP_VERSION")` 由 `build.rs:12-32/37` 从 `tauri.conf.json` 注入，安卓 `env!("CARGO_PKG_VERSION")` 取自 `Cargo.toml`。当前两 Cargo.toml `version` + 桌面 `tauri.conf.json:3` / 安卓 `tauri.conf.json:4` + 更新源 `version.json:2` 五处均为 `2.3.9`，发布时人工同步，无自动校验。
- **两端配置面是"交集 + 双向差集"而非子集**：60（桌面 `config/model.rs:10-172`）与 47（安卓 `config_state.rs:15-111`）字段中共有 43 个；桌面独有 17 个（`adapter1`/`adapter2`/`dual_adapter`、`adapter1_account`/`adapter2_account`（设备级账号绑定，[[adapter-account-binding]]）、`auto_exit_after_login`/`auto_exit_on_online`、`minimize_to_tray`、`lightweight_mode`、`hidden_start`、`auto_launch`、`outbound_manual_hold_day`、`campus_exit_on_fail`/`campus_exit_start_minutes`/`campus_exit_end_minutes`、`skip_sha256_when_missing`、`config_version`）；安卓独有 4 个（`allow_2d_face_verify`、`background_check_idle_interval`、`enable_boot_autostart`、`config_schema_version`）。"双端同步"实际需要双向增量维护。另注意：桌面前端 `types.ts` 的 `Config` 接口只有 59 字段（`tauri-app/frontend/src/settings/types.ts:9-87`），Rust 侧 `skip_sha256_when_missing`（`config/model.rs:169`）未暴露给前端；安卓前端 `types.ts` 的 `Config` 47 字段（`android/frontend/src/settings/types.ts:9-73`）与 Rust `Settings` 一一对应。
- **安卓侧网络探测存在平台性缺口**：`surge_ping` ICMP 兜底在安卓运行时静默失败，质量结果依赖 TCP 优先策略（`quality_cmds.rs:1-2`）；校园网判定已改为 Portal TCP 可达表达（ICMP 原始 socket 被 SELinux 禁止，`campus_detect.rs:3-10`），且经 `bound_socket` 的 SO_BINDTODEVICE 直连物理网卡绕开 VPN 全量接管（`campus_detect.rs:41-53`）；`bindProcessToNetwork` 在 VPN 全接管时仍会被 netd 以 EPERM 拒绝，故 `bound_socket::bind_capability()==Supported` 时 `ensure_wifi_bound` 直接旁路（`protocol_cmds.rs:269-274`），旁路生效前提下清连接池与否依赖 `already_bound` 判定（`protocol_cmds.rs:286-290`）。
- **双端同步依赖人工纪律**：`AGENTS.md` 第 3 条要求同提交双端各改一份，但仓库中没有 CI 检查，无法自动拦截只改一端的提交（约定现落在 `AGENTS.md`「必守约定」第 3 条，由仓库内 `.codewiki/` 活 wiki 记载）。
