---
title: _patterns
type: pattern
source_files:
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/android_state.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/campus_detect.rs
  - android/src-tauri/src/protocol_cmds.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/account_cmds.rs
  - android/src-tauri/src/system_cmds.rs
  - android/src-tauri/src/battery_cmds.rs
  - android/src-tauri/src/cpu_affinity.rs
  - android/src-tauri/src/quality_cmds.rs
  - android/src-tauri/src/quality_history.rs
  - android/src-tauri/src/update_cmds.rs
  - android/src-tauri/src/login_history.rs
  - android/src-tauri/src/identity_gate.rs
  - android/src-tauri/src/vpn_cmds.rs
  - android/src-tauri/src/self_service_cmds.rs
  - android/src-tauri/src/face_model_cmds.rs
  - android/plugins/network-bind/src/lib.rs
  - android/plugins/keystore/src/lib.rs
  - android/plugins/foreground-service/src/lib.rs
  - android/src-tauri/tauri.conf.json
  - android/src-tauri/src/config_state.rs (tests)
  - android/src-tauri/src/campus_detect.rs (tests)
  - android/src-tauri/src/monitor_loop.rs (tests)
  - android/src-tauri/src/account_cmds.rs (tests)
  - android/src-tauri/src/quality_cmds.rs (tests)
  - android/src-tauri/src/quality_history.rs (tests)
  - android/src-tauri/src/update_cmds.rs (tests)
  - android/src-tauri/src/login_history.rs (tests)
  - android/src-tauri/src/identity_gate.rs (tests)
  - android/src-tauri/src/vpn_cmds.rs (tests)
  - android/src-tauri/src/self_service_cmds.rs (tests)
  - android/src-tauri/src/face_model_cmds.rs (tests)
  - android/src-tauri/src/cpu_affinity.rs (tests)
  - tauri-app/src-tauri/Cargo.toml
  - tauri-app/src-tauri/src/main.rs
  - tauri-app/src-tauri/src/lib.rs
  - tauri-app/src-tauri/src/build.rs
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
tags: [pattern, cross-cutting, tauri, rust, android, desktop, campus-login]
---

## Overview

本仓库包含 Android 端（`android/`）与桌面端（`tauri-app/`）两套 Tauri 应用，二者通过 Cargo 依赖共享跨平台核心库 `campus_login_lib`，在命令面、配置模型、加密落盘、后台监控等方面呈现出高度一致的实现模式。以下总结反复出现的横切模式。

## Patterns

### 跨平台协议共享与命令面同构
两端均禁止在各自平台代码中复制协议逻辑，而是复用 `campus_login_lib` 中的纯函数；Tauri 命令名与 IPC 契约刻意对齐，使前端近乎零适配。
证据：`android/src-tauri/src/lib.rs` 的 `invoke_handler!` 注册命令（如 `get_config`、`save_config`、`check_campus_status`、`start_background_check`）与 `tauri-app/src-tauri/src/lib.rs` 及 `tauri-app/src-tauri/src/commands/mod.rs` 同名；`android/src-tauri/src/protocol_cmds.rs` 的 `do_login` 复用 `campus_login_lib::auth::protocol::do_login_with_retry`；`android/src-tauri/src/account_cmds.rs` 与 `tauri-app/src-tauri/src/commands/account.rs` 均包含 `auto_create_account_for_current`、`sanitize_account_id`、`merge_save_as_target` 等同构函数。

### 敏感信息掩码与平台加密落盘
密码等凭据在内存与磁盘均受保护：出站一律经掩码函数替换为 `***`；落盘时通过平台专属加密器（AndroidKeyStore / Windows DPAPI）加密，且明文密码永不出站。
证据：`android/src-tauri/src/config_state.rs` 定义 `PASSWORD_MASK = "***"` 与 `masked_for_display`（第443–452行），`CryptoBridge` 经 `CampusKeystoreExt` 调用 `android/plugins/keystore/src/lib.rs` 的 AES-GCM；`tauri-app/src-tauri/src/config/model.rs` 同样定义 `PASSWORD_MASK` 与 `masked_for_display`（第350行），`tauri-app/src-tauri/src/account/crypto.rs` 使用 DPAPI 加解密；两端 `vpn_password_enc` 字段均为平台密文互不可解。

### 配置与历史数据的原子写与损坏恢复
所有持久化文件采用临时文件+重命名（或 pid 标记）的原子写；历史记录（登录、质量）采用头插法上限 100，解析损坏时备份原文件并重置，避免半截文件或崩溃导致数据丢失。
证据：`android/src-tauri/src/config_state.rs` 的 `save_file`（临时文件名带 pid）、`login_history.rs` 与 `quality_history.rs` 的 `append`（头插法上限 100、损坏备份重置）；`tauri-app/src-tauri/src/config/persist.rs` 的 `atomic_write`（临时文件+sync_all+重命名）、`append_login_history`（第174行，头插上限100）、`append_quality_history`（第237行），以及 `config_cmd.rs` 加载失败时 `.corrupt-<ts>.bak` 备份。

### 并发互斥与代际令牌防竞速
跨命令的共享资源（配置 IO、账号操作、历史写、后台循环）均通过显式锁或原子标志串行化；长期循环使用代际 token 防止 stop/start 竞速误清状态。
证据：`android/src-tauri/src/config_state.rs` 的 `config_io_lock`（跨命令串行锁）、`monitor_loop.rs` 的 `MONITOR_GENERATION`、`quality_cmds.rs` 的 `LATENCY_GENERATION`；`tauri-app/src-tauri/src/infra/state/mod.rs` 的 `TaskLock`/`TaskFlags`、`commands/account.rs` 的 `ACCOUNT_OP_LOCK`、`config/persist.rs` 的 `LOGIN_HISTORY_LOCK`/`QUALITY_HISTORY_LOCK`。

### 平台条件编译隔离
移动端与桌面端专属代码通过 `#[cfg(mobile)]` / `#[cfg(desktop)]` 在编译期隔离，非目标平台编译为空实现或剔除模块，保证单一代码库多端构建。
证据：`android/src-tauri/src/lib.rs` 在 `#[cfg(mobile)]` 下挂载三个插件（`tauri_plugin_campus_network_bind` 等），`android/plugins/*/src/lib.rs` 均 `#![cfg(mobile)]`；`tauri-app/src-tauri/src/lib.rs` 以 `#[cfg(desktop)]` 门控 `app`/`commands`/`monitor` 等模块，`auth/mod.rs` 中 `dual_adapter_executor` 等仅 `#[cfg(desktop)]`。

### 统一命令返回结构与事件总线
命令返回统一为 `{success, message?, data?}` 形状；状态变更经事件总线向前端 emit 固定事件名（如 `config-changed`、`login-log`、`background-check-result`），前后端契约稳定。
证据：`tauri-app/src-tauri/src/infra/state/mod.rs` 的 `CommandResult`/`AccountResult`，`infra/events.rs` 的 `EventBus` 方法 `emit_login_log`/`emit_background_check_result`/`emit_config_changed`；`android/src-tauri/src/account_cmds.rs` 的 `AccountResult`（`ok`/`ok_with_account`/`err` 构造器）、`config_state.rs` 的 `emit_config_changed`、`monitor_loop.rs` 的 `emit_login_log` 与 `emit_auto_login_result`。

### 后台监控循环与退避重试
两端均实现后台检测/更新循环，采用分档间隔、稳态退避、失败上限熔断，且启动恢复任务将核心逻辑抽为纯函数便于测试。
证据：`android/src-tauri/src/monitor_loop.rs` 的 `effective_interval_ms`（分档）、`run_check_once`（连续失败熔断>=5）、`quality_cmds.rs` 的 `next_backoff_interval_ms`（翻倍上限1800s）；`tauri-app/src-tauri/src/monitor/watcher.rs` 的 `run_startup_tasks`（注册5类任务）、`update/updater.rs` 的 `BACKOFF_RETRY_SECS = [300, 900, 3600]`、`start_update_check_loop` 退避。

### 更新检查与下载安全校验
更新流程均使用版本文件多源降级、白名单域名防 SSRF、流式下载配合 SHA256 校验、残留文件清理。
证据：`android/src-tauri/src/update_cmds.rs` 的 `DOWNLOAD_ALLOWED_HOSTS`、`verify_file_sha256`、`cleanup_stale_apks`、`VERSION_MIRRORS`；`tauri-app/src-tauri/src/update/updater.rs` 的 `VERSION_MIRRORS`、`sha256_checksum`、`GITHUB_REPO` 与 `BACKOFF_RETRY_SECS`。

### 账号档案管理与自动建号
多账号系统采用一账号一 JSON、密码密文落盘、切换合并登录字段；R2 自动建号具备撞库防护与幂等短路，id 消毒白名单。
证据：`android/src-tauri/src/account_cmds.rs` 的 `sanitize_account_id`、`validate_account_name`、`auto_create_account_in`（撞库防护）、`merge_save_as_target`；`tauri-app/src-tauri/src/commands/account.rs` 的同名 `sanitize_account_id`、`validate_display_name`、`auto_create_account_in`（第496行）、`merge_account_into_config`。