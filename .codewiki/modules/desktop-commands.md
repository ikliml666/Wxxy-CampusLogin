---
title: 桌面端 Tauri 命令层
type: module
source_files:
  - tauri-app/src-tauri/src/commands/mod.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
  - tauri-app/src-tauri/src/commands/login.rs
  - tauri-app/src-tauri/src/commands/background.rs
tags: [desktop, tauri, ipc, commands, 命令面]
---

## Overview

`tauri-app/src-tauri/src/commands/` 是桌面端 Rust 后端对 webview 暴露的 IPC 命令面。模块声明见 `tauri-app/src-tauri/src/commands/mod.rs:1-8`，共声明 8 个子模块：`login`、`background`、`network_cmd`、`system`、`config_cmd`、`account`、`updater`、`self_service`。本文档仅基于当前提供的源码（`mod.rs`、`config_cmd.rs`、`login.rs`、`background.rs`）重写，涵盖其中的命令与辅助函数。

提供的源码中共有 **11 条 `#[tauri::command]`**（`config_cmd.rs` 5、`login.rs` 2、`background.rs` 4）。核心特征包括：阻塞 IO（文件读写、DPAPI、网络探测）通过 `spawn_blocking` 卸载至后台线程；严格的敏感数据掩码策略（`masked_for_display` / DPAPI 密文导出）；基于 `AppState` 状态机的一致性保障（先落盘再更新内存）。

## Key Components

### 命令注册位置（先读这一节）

模块声明见 `tauri-app/src-tauri/src/commands/mod.rs:1-8`（8 个 `pub mod`）。命令命名约定：Rust 函数名即前端 `invoke` 名，保持 snake_case；命令**参数**在 IPC 上按 camelCase 传递（Tauri 2 默认行为），例如 `save_config(config, clearPassword)`（`config_cmd.rs:235` 函数签名为 `config: Config, clear_password: Option<bool>`）。

### 命令总表（逐条，共 11 条）

#### config_cmd.rs（5 条）

| 命令 | 位置 | 参数 | 返回 | 用途 | 前置门 / 校验 |
|---|---|---|---|---|---|
| `show_window` | `commands/config_cmd.rs:209-224` | `app_handle: AppHandle` | `Result<(), String>` | 显示并聚焦主窗口（转发 `app::window::show_or_rebuild_main`），启动期用户最小化保护（`is_user_dismissed` 时直接返回） | 无 |
| `get_config` | `commands/config_cmd.rs:227-229` | `state: State<'_, AppState>` | `Result<Config, String>` | 返回当前内存配置，出站前 `masked_for_display()` 掩码密码字段 | 无（掩码是出站唯一出口） |
| `save_config` | `commands/config_cmd.rs:235-286` | `state`、`app_handle`、`config: Config`、`clear_password: Option<bool>` | `Result<CommandResult, String>` | 校验配置 → 处理密码保留/清除语义 → 同步全局 `PORTAL_URL` 与日志保留天数 → 先落盘再更新内存 → 落盘成功后自动建号/同步账号档案（R2），实际写盘时补刷托盘菜单（281-283） | `validate_config`（241）；`clear_password == Some(true)` 跳过兜底置空（251-252）；空/MASK 时回填当前内存密码（253-257） |
| `export_config` | `commands/config_cmd.rs:117-137` | `state`、`app_handle`、`include_password: Option<bool>` | `Result<String, String>` | 导出配置 JSON 到 `<data_dir>/exports/config-<ts>.json`，返回文件路径；默认掩码态（`masked_for_display`），`include_password=true` 时密码经 `crypto::encrypt` 转本机 DPAPI 密文（wrapper 带 `passwordEncrypted` 标志），任何分支不写明文 | 纯函数 `build_config_export_payload`（73-94）保证明文不出站；原子写 `persist::atomic_write`（129） |
| `import_config` | `commands/config_cmd.rs:145-206` | `state`、`app_handle`、`path: String` | `Result<CommandResult, String>` | 读 JSON（≤1MB）→ 密码还原（空/MASK 回填当前值；`passwordEncrypted=true` 先 `decrypt`，失败明确报错）→ 严格 `validate_config` → 丢弃导入文件中的本机切换态字段 → 与 `save_config` 同路径落盘+发事件+更新内存 | 失败分列：大小防呆（147）、JSON 解析（160）、结构无效（166）、密码密文解密（172）、配置校验（185）；密码还原先于校验（172） |

同文件非命令的公开辅助函数：

- `save_config_to_disk_encrypted(app_handle: &AppHandle, config: &Config) -> Result<(), String>`（`commands/config_cmd.rs:8-15`）：包装 `persist::save_config_and_broadcast`（落盘+掩码广播 `config-changed`）并补刷托盘菜单。所有改写配置的命令最终都经此路径通知前端。
- `load_config_from_file(app_handle: &AppHandle) -> Result<Config, String>`（`commands/config_cmd.rs:17-40`）：从磁盘读配置，解密密码字段，失败清密码保留其他。
- `load_config_from_disk_or_default(app_handle: &AppHandle) -> Config`（`commands/config_cmd.rs:42-65`）：启动/受损恢复入口，解析失败时把原文件备份为 `*.json.corrupt-<ts>.bak`（48-60）后返回默认配置。
- `build_config_export_payload(current: &Config, include_password: bool) -> Result<serde_json::Value, String>`（`commands/config_cmd.rs:73-94`）：纯函数，组装导出 payload，掩码或 DPAPI 密文。
- `restore_imported_password_field(imported: &mut String, encrypted: bool, current: &str) -> Result<(), String>`（`commands/config_cmd.rs:102-112`）：还原导入密码字段，空/MASK 保留当前值，密文 decrypt，明文直传。

测试模块：`commands/config_cmd.rs:288-366` 含 `masked_for_display_masks_password`（297）、`export_payload_never_contains_plaintext_password`（316，windows）、`import_password_restore_semantics`（342，windows）三个测试。

#### login.rs（2 条）

| 命令 | 位置 | 参数 | 返回 | 用途 | 前置门 / 校验 |
|---|---|---|---|---|---|
| `do_login` | `commands/login.rs:63-89` | `state: State<AppState>`、`app_handle: AppHandle`、`adapter_name: Option<String>` | `Result<CommandResult, String>` | 取消残留自动退出倒计时（64-66）后经 `spawn_blocking` 调用 `auth::service::full_login`，成功再跑 `post_login_handler`（85） | **并发互斥门** `tasks.is_logging_in.try_acquire()`（73-79）：已在登录则 `CommandResult::err("登录正在进行中")` |
| `do_logout` | `commands/login.rs:92-100` | `_state: State<'_, AppState>`、`app_handle: AppHandle`、`adapter_name: Option<String>` | `Result<CommandResult, String>` | 调用 `perform_full_logout_sync`（内部执行 `auth::service::full_logout` 及后续状态复位），成功后 sleep 1s 复查各适配器 Portal 在线状态并写登录日志 | **并发互斥门** `tasks.is_logging_out.try_acquire()`（`perform_full_logout_sync` 内 107-113） |

同文件非命令函数与结构体：

- `AdapterOnlineStatus` 结构体（`commands/login.rs:9-16`）：字段 `any_online: bool`、`a1_online: bool`、`a2_online: bool`、`probe_ok: bool`（审计 A8：探测链路是否全部可用，false 表示离线结论不可信）。
- `check_any_adapter_online(state: &AppState) -> AdapterOnlineStatus`（`commands/login.rs:18-60`）：并行检测双适配器 Portal 状态，使用 `std::thread::scope` 与 `tauri::async_runtime::handle().inner().enter()` 修复 reqwest 超时上下文。
- `perform_full_logout_sync(app_handle: &AppHandle, adapter_name: Option<&str>) -> CommandResult`（`commands/login.rs:105-198`）：注销核心，含互斥获取、调用 `full_logout`、1s 复检、状态复位（全量注销重置 `auto_exit_cancelled`/`failure_tracker`/`logout_protected_until` 等，单适配器仅重置 per-adapter 标志）。

#### background.rs（4 条）

| 命令 | 位置 | 参数 | 返回 | 用途 | 前置门 / 校验 |
|---|---|---|---|---|---|
| `start_background_check` | `commands/background.rs:7-10` | `app_handle`、`state` | `Result<CommandResult, String>` | 转发 `monitor::watcher::start_background_check_inner` 启动周期巡检 | 由 `watcher` 内部判定重复启动 |
| `stop_background_check` | `commands/background.rs:13-27` | `_state`、`app_handle` | `Result<CommandResult, String>` | 取消 `background_check` 与 `background_check_once` 任务令牌 + 置 `enable_background_check=false` 并落盘（22） | 无 |
| `trigger_background_check` | `commands/background.rs:30-39` | `_state`、`app_handle` | `Result<CommandResult, String>` | 手动触发一次后台检测（`background_task::spawn_background_check_once`），经 task_manager 统一注册 | **并发门** `tasks.is_checking.is_active()`（32-34）：进行中返回 `err("检测正在进行中")` |
| `get_background_status` | `commands/background.rs:114-118` | `app_handle` | `Result<serde_json::Value, String>` | 返回巡检状态快照（组装逻辑在同文件 `get_background_status_value`） | 无 |

同文件非命令函数：

- `get_background_status_value(state: &AppState, _app_handle: &AppHandle, adapters: &[Adapter]) -> serde_json::Value`（`commands/background.rs:44-111`）：组装后台状态 JSON，接受外部适配器快照避免重复枚举。

### 命令层私有结构体

`AdapterOnlineStatus`（`commands/login.rs:9-16`，仅 `login.rs` 内部使用）：

| 字段 | 类型 | 说明 |
|---|---|---|
| `any_online` | `bool` | 任一适配器 Portal 在线 |
| `a1_online` | `bool` | 主适配器在线 |
| `a2_online` | `bool` | 副适配器在线（副适配器未启用时为 false） |
| `probe_ok` | `bool` | 审计 A8：探测链路是否全部可用（所有已启用适配器的 Portal 探测均成功返回）。false 表示存在探测异常——"离线"结论不可信，不得断言已确认离线 |

### 命令返回的 JSON 结构（内联构造，无 Rust 结构体）

`get_background_status`（构造点 `commands/background.rs:91-110`）：

| key | 来源 | 说明 |
|---|---|---|
| `serverAvailable` | `network.server_available` | 服务器可达性 |
| `loginPreparationMode` | `config.auto_login_on_preparation` | 准备期自动登录模式 |
| `checkCount` | `network.background_check_count` | 巡检次数 |
| `isRunning` | `task_manager.is_running("background_check")` | 巡检是否在运行 |
| `interval` | `config.background_check_interval` | 巡检间隔 |
| `enabled` | `config.enable_background_check` | 总开关 |
| `adapterStatuses` | 逐适配器组装（51-83） | 由 `watcher::adapter_status_entry` / `adapter_disconnected_entry` / `adapter_disabled_entry` 生成 |
| `online` | 由 `adapterStatuses` 任一 `online=true` 推导（85） | 整体在线判定 |
| `currentSsid` | `network.current_ssid` | 当前 SSID |
| `onCampusNetwork` | `network.on_campus_network` | 是否校园网 |
| `enableNetworkNameCheck` | `config.enable_network_name_check` | 网络名校验开关 |
| `requiredNetworkName` | `config.required_network_name` | 要求的网络名 |
| `campusWifi` / `campusWired` / `a1OnCampus` / `a2OnCampus` / `a1CampusMessage` / `a2CampusMessage` | 全部硬编码 `serde_json::Value::Null`（104-109） | 历史遗留占位，桌面端未填充（见 Known Issues） |

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                            Desktop Commands Layer                           │
│  tauri-app/src-tauri/src/commands/                                          │
│                                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                      │
│  │ config_cmd   │  │ login        │  │ background   │                      │
│  │ (5 cmds)     │  │ (2 cmds)     │  │ (4 cmds)     │                      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘                      │
│         │                 │                  │                             │
│  ┌──────▼──────────────────────────────────────────────────────────────┐  │
│  │                         Shared Infrastructure                        │  │
│  │  • TaskLock / CancellationToken (并发互斥)                          │  │
│  │  • spawn_blocking (IO/DPAPI/网络探测隔离)                           │  │
│  │  • save_config_to_disk_encrypted (落盘+广播)                        │  │
│  │  • masked_for_display (密码安全出站)                                │  │
│  └──────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### 通用入站与出站

```text
前端 invoke('<snake_case 命令名>', { camelCase 参数 })
  → commands/<模块>.rs 的 #[tauri::command] 函数
      ├─ 参数校验（validate_config / include_password 等）
      ├─ 并发门（TaskLock::try_acquire / is_active）
      ├─ 状态读写：AppState { config, tasks, task_manager, network, exit, ... }
      └─ 转发到 auth / network / config / monitor 等下层模块
  → Result<T, String>（Tauri 把 Err 变 rejected Promise，Ok 变 resolved）
```

异步命令在 `spawn_blocking` 闭包内经 `app_handle` 重新取状态（因为 `State<'_, AppState>` 不能跨线程 move），例如 `commands/login.rs:72`、`commands/login.rs:106` 通过 `app_h.state::<AppState>()` 获取；`commands/background.rs` 的命令则使用 `CommandContext::from_app` 直接取状态（如 `commands/background.rs:14`、`commands/background.rs:31`）。

### 关键调用链

登录：

```text
invoke('do_login', {adapterName})
  → commands/login.rs:63 do_login
  → exit.auto_exit_cancelled = false；exit.set_deadline(None)（64-66）
  → spawn_blocking（71）→ tasks.is_logging_in.try_acquire()（73）
  → auth::service::full_login(&s, &app_h, adapter)（80）
  → 成功后 commands/login.rs:85 → auth::service::post_login_handler
```

注销与状态复位：

```text
invoke('do_logout', {adapterName})
  → commands/login.rs:92 → perform_full_logout_sync（内部执行 full_logout 及状态复位）
  → auth::service::full_logout（perform_full_logout_sync 内 115）
  → 成功：sleep 1s（118）→ check_any_adapter_online（119，见下）→ EventBus::emit_login_log（122/129/138）
  → 返回后：adapter_name 为 None 时走"全量注销"（148-178：auto_exit_cancelled=true、failure_tracker::reset_all、network 状态重置、logout_protected_until = now+60s）
            否则走"单适配器注销"（179-196：只按检测结果重置 per-adapter 标志）
```

`check_any_adapter_online`（`commands/login.rs:18-60`）并行 `check_portal_full`（35-36），每个线程先 `tauri::async_runtime::handle().inner().enter()`（33）以获得 Tokio runtime context。

配置保存：

```text
invoke('save_config', {config, clearPassword})
  → commands/config_cmd.rs:235 save_config
  → validate_config（241）→ 密码语义处理（251-257）
  → network::update_portal_url（261）→ logger::set_log_retention_days（264）
  → spawn_blocking 调 save_config_to_disk_encrypted（271）→ state.config.store（274）
  → auto_create_account_for_current（281）补刷托盘
```

后台检测触发：

```text
invoke('trigger_background_check')
  → commands/background.rs:30 → tasks.is_checking.is_active() 门（32）
  → monitor::background_task::spawn_background_check_once（37）
```

## Connections

- [[desktop-auth]]：`do_login` / `do_logout` 转发的 `auth::service::full_login` / `full_logout` / `post_login_handler`，`check_any_adapter_online` 依赖的 `auth::portal::check_portal_full`。
- [[desktop-config]]：`Config` 字段全集、`masked_for_display` / `PASSWORD_MASK`、`validate_config` 与 `validate_config_lenient`、`persist` 读写与加密。
- [[desktop-monitor]]：`watcher::start_background_check_inner` / `adapter_status_entry` 等以及 `background_task::spawn_background_check_once` 的关系。
- [[desktop-infra]]：`AppState`、`CommandResult`、`TaskLock` / `BackgroundTaskManager`、`EventBus`、`CommandContext`。

## Known Issues

1. **后台状态 6 个字段恒为 `null`**：`tauri-app/src-tauri/src/commands/background.rs:104-109` 硬编码 `campusWifi` / `campusWired` / `a1OnCampus` / `a2OnCampus` / `a1CampusMessage` / `a2CampusMessage` 为 `serde_json::Value::Null`；消费者必须容忍 null。
2. **`check_any_adapter_online` 适配器枚举失败静默降级**：`tauri-app/src-tauri/src/commands/login.rs:18-23`，`get_adapters_cached()` 失败时直接返回 `probe_ok: false` 且所有在线 false，注销后状态复位会据此写 `any_adapter_online=false`（login.rs:161），可能掩盖真实在线状态（但因 `probe_ok=false` 不会发“已确认离线”日志）。
3. **`do_logout` 的 `_state` 参数未使用**：`tauri-app/src-tauri/src/commands/login.rs:92`，函数通过 `app_handle` 在 `perform_full_logout_sync` 内重新取 state（login.rs:106），签名保留仅为与前端调用一致。
4. **`import_config` 强制丢弃导入文件中的本机切换态**：`tauri-app/src-tauri/src/commands/config_cmd.rs:177-182` 用当前本机值覆盖 `outbound_metric_restore` 等字段，若用户预期迁移这些系统状态则会丢失（设计如此，需文档明确）。
