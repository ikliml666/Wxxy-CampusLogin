---
title: 桌面端 Tauri 命令层
type: module
source_files:
  - tauri-app/src-tauri/src/commands/mod.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
  - tauri-app/src-tauri/src/commands/login.rs
  - tauri-app/src-tauri/src/commands/background.rs
  - tauri-app/src-tauri/src/commands/network_cmd.rs
  - tauri-app/src-tauri/src/commands/system.rs
  - tauri-app/src-tauri/src/commands/account.rs
  - tauri-app/src-tauri/src/commands/self_service.rs
  - tauri-app/src-tauri/src/commands/updater.rs
tags: [desktop, tauri, ipc, commands, 命令面]
---

## Overview

`tauri-app/src-tauri/src/commands/` 是桌面端 Rust 后端对 webview 暴露的全部 IPC 命令面：前端经 `@tauri-apps/api/core` 的 `invoke('<snake_case 命令名>')` 调用，命令函数负责参数校验、并发互斥、状态读写，再转发到 `auth` / `network` / `config` / `monitor` / `self_service` / `update` / `platform` 等下层模块。

本模块共 **54 条 `#[tauri::command]`**（按文件：`config_cmd.rs` 3、`login.rs` 2、`background.rs` 4、`network_cmd.rs` 14、`system.rs` 15、`account.rs` 5、`self_service.rs` 7、`updater.rs` 4），另有 2 条日志命令定义在 `infra/logger.rs` 并一同注册到桌面命令表——桌面端 `generate_handler!` 合计注册 **56 项**。

## Key Components

### 命令注册位置（先读这一节）

| 端 | 注册位置 | 注册项数 | 说明 |
|---|---|---|---|
| 桌面 | `tauri-app/src-tauri/src/app/startup.rs:63-120` | 56 | 54 条 `commands::*` + `infra::logger::set_debug_mode`（118 行）/ `get_debug_mode`（119 行） |
| 安卓 | `android/src-tauri/src/lib.rs:55-104` | 48 | 同名对齐的独立实现（`protocol_cmds` / `campus_detect` / `config_state` / `self_service_cmds` / `account_cmds` / `system_cmds` / `monitor_loop` / `quality_cmds` / `update_cmds`），不含桌面专属命令 |

模块声明见 `tauri-app/src-tauri/src/commands/mod.rs:1-8`（8 个 `pub mod`，`self_service` 在 8 行）。

命令命名约定：Rust 函数名即前端 `invoke` 名，保持 snake_case（例如 `invoke<Config>('get_config')`，见 `tauri-app/frontend/src/hooks/tauriApi.ts:134`）；命令**参数**在 IPC 上按 camelCase 传递（Tauri 2 默认行为），例如 `save_config(config, clearPassword, clearSelfPassword)`（`tauriApi.ts:135`）。`tauriApi.ts` 覆盖了上述全部 56 个命令名。

### 命令总表（逐条，共 54 条）

#### config_cmd.rs（3 条）

| 命令 | 位置 | 参数 | 返回 | 用途 | 前置门 / 校验 |
|---|---|---|---|---|---|
| `show_window` | `commands/config_cmd.rs:80-84` | `app_handle: AppHandle` | `Result<(), String>` | 显示并聚焦主窗口（转发 `app::window::show_and_focus_main`） | 无 |
| `get_config` | `commands/config_cmd.rs:86-89` | `state: State<AppState>` | `Result<Config, String>` | 返回当前内存配置，出站前 `masked_for_display()` 掩码两个密码字段 | 无（掩码是出站唯一出口） |
| `save_config` | `commands/config_cmd.rs:91-139` | `state`、`app_handle`、`config: Config`、`clear_password: Option<bool>`、`clear_self_password: Option<bool>` | `Result<CommandResult, String>` | 校验配置 → 处理密码保留/清除语义 → 同步全局 `PORTAL_URL` 与日志保留天数 → 先落盘再更新内存 | `validate_config`（99 行）；`clear_password == Some(true)` 跳过兜底置空（109-110）；空/MASK 时回填当前内存密码（111-115、118-123） |

同文件非命令的公开辅助函数：

- `save_config_to_disk_encrypted(app_handle: &AppHandle, config: &Config) -> Result<(), String>`（`commands/config_cmd.rs:9-18`）：落盘 + 统一发射 `config-changed` 事件（掩码后发射，15-16 行）。**所有改写配置的命令最终都经此路径通知前端**。
- `load_config_from_disk_or_default(app_handle: &AppHandle) -> Config`（`commands/config_cmd.rs:55-78`）：启动/受损恢复入口，解析失败时把原文件备份为 `*.json.corrupt-<ts>.bak`（63-73 行）后返回默认配置。

#### login.rs（2 条）

| 命令 | 位置 | 参数 | 返回 | 用途 | 前置门 / 校验 |
|---|---|---|---|---|---|
| `do_login` | `commands/login.rs:56-83` | `state: State<AppState>`、`app_handle: AppHandle`、`adapter_name: Option<String>` | `Result<CommandResult, String>` | 取消残留自动退出倒计时（58-60）后经 `spawn_blocking` 调用 `auth::service::full_login`，成功再跑 `post_login_handler`（78-80） | **并发互斥门** `tasks.is_logging_in.try_acquire()`（67-73）：已在登录则 `CommandResult::err("登录正在进行中")` |
| `do_logout` | `commands/login.rs:85-180` | `_state`、`app_handle`、`adapter_name: Option<String>` | `Result<CommandResult, String>` | 调用 `auth::service::full_logout`，成功后 sleep 1s 复查各适配器 Portal 在线状态并写登录日志（102-121），再重置运行态 | **并发互斥门** `tasks.is_logging_out.try_acquire()`（92-98） |

#### background.rs（4 条）

| 命令 | 位置 | 参数 | 返回 | 用途 | 前置门 / 校验 |
|---|---|---|---|---|---|
| `start_background_check` | `commands/background.rs:7-11` | `app_handle`、`state` | `Result<CommandResult, String>` | 转发 `monitor::watcher::start_background_check_inner` 启动周期巡检 | 由 `watcher` 内部判定重复启动 |
| `stop_background_check` | `commands/background.rs:13-25` | `_state`、`app_handle` | `Result<CommandResult, String>` | 取消 `background_check` 任务令牌 + 置 `enable_background_check=false` 并落盘（17-22） | 无 |
| `trigger_background_check` | `commands/background.rs:27-41` | `_state`、`app_handle` | `Result<CommandResult, String>` | 手动触发一次后台检测（`watcher::run_background_check`），在 tokio 任务中异步跑 | **并发门** `tasks.is_checking.is_active()`（30-32）：进行中直接返回 `err("检测正在进行中")` |
| `get_background_status` | `commands/background.rs:112-116` | `app_handle` | `Result<serde_json::Value, String>` | 返回巡检状态快照（组装逻辑在同文件 `get_background_status_value`，43-110 行） | 无 |

#### network_cmd.rs（14 条）

| 命令 | 位置 | 参数 | 返回 | 用途 | 前置门 / 校验 |
|---|---|---|---|---|---|
| `get_adapters` | `commands/network_cmd.rs:19-25` | `force: Option<bool>` | `Result<Vec<Adapter>, String>` | 取网卡列表（`force=true` 走 `get_adapters_force` 绕缓存） | 无 |
| `get_disabled_adapters` | `commands/network_cmd.rs:27-30` | — | `Result<Vec<DisabledAdapter>, String>` | 取被禁用的网卡列表 | 无 |
| `enable_adapter` | `commands/network_cmd.rs:32-41` | `adapter_name: String` | `Result<CommandResult, String>` | 启用指定网卡（提权级操作） | `adapter_cache::validate_adapter_name`（35 行）；`enable_adapter_inner(name, true)` 允许 COM 提权失败后降级弹 UAC（37-38 行注释） |
| `get_adapter_details` | `commands/network_cmd.rs:43-46` | — | `Result<Vec<AdapterDetail>, String>` | 取网卡详情（IP/掩码/网关/DNS 等） | 无 |
| `check_campus_status` | `commands/network_cmd.rs:48-68` | `app_handle` | `Result<serde_json::Value, String>` | 强制刷新网卡后调用 `monitor::watcher::check_campus_network` 判定是否在校园网 | 无（内部强制刷新网卡列表，绕过缓存） |
| `check_portal_status` | `commands/network_cmd.rs:70-100` | `adapter_ip: String`、`app_handle` | `Result<serde_json::Value, String>` | 探测 Portal 在线状态（只读，不带凭据） | **注销保护期门**（81-88）：`logout_protected_until` 未到期时直接返回 `online=false, message="已注销"`，避免 Portal 延迟误判；`adapter_ip` 为空提前返回（72-77） |
| `dhcp_renew_all` | `commands/network_cmd.rs:112-129` | `app_handle` | `Result<serde_json::Value, String>` | 对 resolve 后的主/副适配器中有线者做 DHCP 续租（`dhcp_renew_wired_only`） | `filter_operation_adapters`（120-121）限定操作范围，不触碰其他网卡 |
| `dhcp_release_renew` | `commands/network_cmd.rs:131-152` | `app_handle` | `Result<serde_json::Value, String>` | MAC 重置 + 释放/续租全部目标适配器（`dhcp_release_renew_all`），网关取 `get_campus_gateway`（102-106） | 同上（143-144） |
| `dhcp_release_renew_adapter` | `commands/network_cmd.rs:154-170` | `adapter_name: String`、`app_handle` | `Result<serde_json::Value, String>` | 单个适配器释放/续租 | `validate_adapter_name`（157 行） |
| `check_network_quality` | `commands/network_cmd.rs:172-198` | `app_handle` | `Result<serde_json::Value, String>` | 网络质量检测（网关/外网延迟等） | **总开关门**（176-178：`enable_network_quality=false` 返回 `empty_quality_json("disabled")`）+ **并发门** `is_quality_checking`（179-182，忙时返回 `"busy"`）；无可用 IP 返回 `"unknown"`（192-194） |
| `start_latency_test` | `commands/network_cmd.rs:200-230` | `app_handle`、`_state` | `Result<CommandResult, String>` | 启动定时延迟测试循环（`monitor::latency::spawn_latency_test_loop`），间隔 <10000ms 时兜底 30000ms（214-217） | **总开关门**（207-213）：质量总开关关闭时把 `enable_latency_test` 落盘为 false 并返回提示，防 UI 与运行态分叉 |
| `stop_latency_test` | `commands/network_cmd.rs:232-243` | `app_handle`、`state` | `Result<CommandResult, String>` | 取消 `latency_test` 任务 + 落盘 `enable_latency_test=false` | 无 |
| `check_dns_doh_status` | `commands/network_cmd.rs:245-258` | — | `Result<serde_json::Value, String>` | 读注册表汇总各适配器 DNS 来源与 DoH 状态；非 Windows 返回空结构（253-256） | 无 |
| `setup_dns_doh` | `commands/network_cmd.rs:260-340` | `app_handle`、`family: Option<String>`（`"ipv4"`/`"ipv6"`/`"both"`，非法回退 `both`，263-267） | `Result<serde_json::Value, String>` | 一键设置 DNS + DoH：管理员直调 `dns_setup::setup_dns_doh_admin`（298-300），否则经 `--helper dns` 提权（302-337） | 目标白名单：`resolve_adapter_names` + `filter_operation_adapters` + 非空 IP + `!is_blacklisted`（284-289）；无目标返回失败（291-296）；非 Windows 返回"仅支持Windows"（270-274） |

#### system.rs（15 条）

| 命令 | 位置 | 参数 | 返回 | 用途 | 前置门 / 校验 |
|---|---|---|---|---|---|
| `minimize_window` | `commands/system.rs:7-10` | `window: Window` | `Result<(), String>` | 最小化当前窗口 | 无 |
| `close_window` | `commands/system.rs:12-21` | `window`、`state` | `Result<(), String>` | 按 `minimize_to_tray` 隐藏到托盘；否则置 `exit.is_quitting=true` 后真正关闭（14-20） | 读取 `config.minimize_to_tray`（14 行） |
| `open_external` | `commands/system.rs:23-36` | `url: String` | `Result<bool, String>` | 用系统默认浏览器打开外链 | 仅 http/https（25-27）、长度 ≤2048（28-30）、URL 不得含用户名/密码（32-34） |
| `get_auto_launch` | `commands/system.rs:38-42` | — | `Result<serde_json::Value, String>` | 读注册表判断开机自启是否开启 | 无 |
| `set_auto_launch` | `commands/system.rs:44-72` | `enabled: bool`、`app_handle`、`state` | `Result<serde_json::Value, String>` | 写/删 HKCU Run 注册表项，成功后更新配置并落盘（57-66） | 先注册表后配置，注册表失败不动配置（49-54） |
| `get_notification_enabled` | `commands/system.rs:74-77` | `state` | `Result<bool, String>` | 读 `enable_notification` | 无 |
| `set_notification_enabled` | `commands/system.rs:79-89` | `enabled: bool`、`state`、`app_handle` | `Result<bool, String>` | 更新通知开关并落盘 | 无 |
| `cancel_auto_exit` | `commands/system.rs:91-98` | `app_handle`、`_state` | `Result<CommandResult, String>` | 统一取消自动退出 + 校园网退出（95-96） | 无 |
| `get_logs` | `commands/system.rs:100-104` | `app_handle`、`lines: Option<usize>` | `Result<String, String>` | 读最近日志，默认 200 行 | 无 |
| `clear_logs` | `commands/system.rs:106-110` | `app_handle` | `Result<bool, String>` | 清空日志文件 | 无 |
| `get_init_data` | `commands/system.rs:112-151` | `state`、`app_handle` | `Result<serde_json::Value, String>` | 首屏聚合数据：掩码配置、账号名列表、版本、自启、GPU、刷新率、网卡三列表、激活账号、通知开关、`--autostart` 判定（122 行）、后台状态（134 行复用 `background::get_background_status_value`） | `masked_for_display()` 是出站唯一出口（117 行，注释记录了 2026-09-06 漏掩 `self_password` 的真机缺陷） |
| `render_heartbeat` | `commands/system.rs:153-168` | `state` | `Result<serde_json::Value, String>` | 前端心跳：写 `update_stats.last_render_heartbeat_ms` 供 main.rs 心跳线程检测 WebView 崩溃；返回 `online`/`checking` | 无 |
| `get_gpu_info` | `commands/system.rs:170-174` | — | `Result<serde_json::Value, String>` | 单独取 GPU 信息（内部 `OnceLock` 缓存） | 无 |
| `set_log_retention_days` | `commands/system.rs:176-180` | `days: u32` | `Result<(), String>` | 更新运行期日志保留天数 | 无 |
| `get_log_retention_days` | `commands/system.rs:182-185` | — | `u32` | 读日志保留天数（**唯一不返回 Result 的命令**） | 无 |

#### account.rs（5 条）

| 命令 | 位置 | 参数 | 返回 | 用途 | 前置门 / 校验 |
|---|---|---|---|---|---|
| `list_accounts` | `commands/account.rs:7-12` | `app_handle` | `Result<Vec<String>, String>` | 列出账号目录下的账号名 | 无 |
| `switch_account` | `commands/account.rs:14-49` | `account_name: String`、`app_handle`、`state` | `Result<AccountResult, String>` | 加载账号文件、合并登录字段到内存配置、落盘，返回掩码配置 | `validate_account_name`（16-19，失败返回 `AccountResult::err`）；账号不存在返回 `"账号不存在"`（29） |
| `save_current_as_account` | `commands/account.rs:51-191` | `account_name: String`、`app_handle`、`state` | `Result<AccountResult, String>` | 另存为账号：先回存旧账号（60-123），再写新账号文件（125-175，密码 DPAPI 加密），最后落盘 `active_account`（177-186） | `validate_account_name`（53-56） |
| `delete_account` | `commands/account.rs:193-233` | `account_name: String`、`app_handle`、`state` | `Result<AccountResult, String>` | 删除账号文件；若删的是激活账号则清空并落盘（213-225） | `validate_account_name`（195-196）：此处校验失败**直接 `Err` 传播**（与另两条命令返回 `AccountResult::err` 不一致） |
| `get_active_account` | `commands/account.rs:235-239` | `state` | `Result<String, String>` | 读当前激活账号名 | 无 |

#### self_service.rs（7 条）

| 命令 | 位置 | 参数 | 返回 | 用途 | 前置门 / 校验 |
|---|---|---|---|---|---|
| `bind_operator` | `commands/self_service.rs:65-112` | `state`、`account: String`、`password: String`、`operator: String`、`phone: String`、`sms_password: String` | `Result<CommandResult, String>` | 绑定运营商账号（凭据仅本次请求内存传递，不落盘不写日志） | **身份验证门** `ensure_identity_gate`（82-84）；手机号必须 11 位纯数字（89-91）；`sms_password` 非空（92-94）；学号非空（79-81）；自助密码经 `resolve_self_password` 回退（86-88） |
| `query_bind_status` | `commands/self_service.rs:115-148` | `state`、`account`、`password` | `Result<CommandResult, String>` | 查各运营商绑定状态（手机号掩码 + `passwordSet`） | **有意不设身份门**（模块注释 13-14）：查询类命令依赖免验证自动刷新；学号/密码非空校验（122-127） |
| `query_self_dashboard` | `commands/self_service.rs:152-178` | `state`、`account`、`password` | `Result<CommandResult, String>` | 查在线信息 + 近期上网记录（原始数组透传前端格式化） | 同上，无身份门；学号/密码校验（158-164） |
| `query_self_online_log` | `commands/self_service.rs:182-215` | `state`、`account`、`password`、`start_time: String`、`end_time: String` | `Result<CommandResult, String>` | 查"上网记录"账单页数据 | 无身份门；**严格 ISO 日期校验** `is_iso_date`（37-45、199-201）+ 起止先后比较（203-205） |
| `self_offline_session` | `commands/self_service.rs:219-252` | `state`、`account`、`password`、`session_id: String` | `Result<CommandResult, String>` | 注销指定自助服务在线会话（改变外部状态） | **身份验证门**（231-233）；`session_id` 非空（237-239） |
| `verify_windows_identity` | `commands/self_service.rs:259-279` | `app: AppHandle`、`consent_message: Option<String>` | `Result<CommandResult, String>` | 触发 Windows Hello 验证；主窗口带到前台并取 HWND 绑定 Consent UI（266-270），成功后 `note_identity_verified()` 写时间戳（273-276） | 本身即验证动作；`consent_message` 由前端按 i18n 场景传入 |
| `reveal_operator_credential` | `commands/self_service.rs:285-315` | `state`、`account`、`password`、`operator: String` | `Result<CommandResult, String>` | 返回指定运营商明文凭据（`phone` + `smsPassword`） | **身份验证门** `identity_verified_recently()`（300-304）：TTL 600s 内才能查看，webview 无法绕过 |

#### updater.rs（4 条）

| 命令 | 位置 | 参数 | 返回 | 用途 | 前置门 / 校验 |
|---|---|---|---|---|---|
| `check_update` | `commands/updater.rs:7-21` | `app_handle`、`_state` | `Result<serde_json::Value, String>` | 检查更新（`update_source != "github"` 时镜像优先，10-11），并写 `last_update_check_epoch_ms` | 无 |
| `download_update` | `commands/updater.rs:23-192` | `app_handle`、`url: String`、`_state` | `Result<String, String>` | 流式下载更新包到 `%TEMP%/campus-login-update/`，每 ≥200ms 或下载完成时发进度事件；返回本地文件路径 | **并发门** `is_downloading.try_acquire()`（31-32）；必须 https（33-35）；**主机白名单 13 项**（39-56）；文件名清洗非法字符与 `..`（60-72）；大小上限 500MB（74、97-100、125-129，写盘前判定） |
| `install_update` | `commands/updater.rs:194-291` | `app_handle`、`file_path: String`、`checksum_url: Option<String>` | `Result<bool, String>` | 校验后启动安装：`.exe` 走 `open::that`（251-256），`.msi` 走 `msiexec /i`（257-287），成功后 `schedule_update_cleanup()` | 文件存在（197-199）；**canonical 路径必须落在临时目录内**（202-209）；**SHA256 强制校验**：`checksum_url` 为 `None`/空串直接拒绝（211-243），校验失败即删文件；`skipSha256WhenMissing` 默认关闭（220-225） |
| `get_mirror_urls` | `commands/updater.rs:293-328` | `github_url: String` | `Result<Vec<serde_json::Value>, String>` | 生成 4 个镜像下载地址（官方 / ghfast.top / gh-proxy.com / ghproxy.net） | URL 必须 `https://github.com/` 或 `http://github.com/` 前缀（295-297）、不得含 `..` 或 `\`（298-300） |

## 结构体与字段

### 命令返回值类型（定义在 `infra/state/mod.rs`，命令层大量复用）

`CommandResult`（`infra/state/mod.rs:213-233`）：

| 字段 | 类型 | 说明 |
|---|---|---|
| `success` | `bool` | 业务是否成功（与 `Err(String)` 语义分离：校验失败也用 `ok(CommandResult::err)` 返回） |
| `message` | `Option<String>` | `#[serde(skip_serializing_if = "Option::is_none")]`，提示文案 |
| `data` | `Option<serde_json::Value>` | 同上跳过序列化，附加数据 |

构造器：`ok()`（222）、`ok_msg(&str)`（225）、`err(&str)`（228）。

`AccountResult`（`infra/state/mod.rs:235-255`）：

| 字段 | 类型 | 说明 |
|---|---|---|
| `success` | `bool` | 同 `CommandResult` |
| `message` | `Option<String>` | 失败原因 |
| `active_account` | `Option<String>` | 变更后的激活账号名（`delete_account` 清空时显式返回空串，`commands/account.rs:229-231`） |
| `config` | `Option<Config>` | 掩码后的最新配置（前端直接替换本地配置） |

构造器：`ok(Config)`（246）、`ok_with_account(String, Config)`（249）、`err(&str)`（252）。

### 命令层私有结构体

`AdapterOnlineStatus`（`commands/login.rs:10-14`，仅 `login.rs` 内部使用）：

| 字段 | 类型 | 说明 |
|---|---|---|
| `any_online` | `bool` | 任一适配器 Portal 在线 |
| `a1_online` | `bool` | 主适配器在线 |
| `a2_online` | `bool` | 副适配器在线（副适配器未启用时为 false） |

### 命令参数涉及的跨模块结构体

- `Config`（`config/model.rs`）：`save_config` / `get_config` / `AccountResult.config` 的载体，字段全集见 [[desktop-config]]。命令层只关心两个密码字段：`password`、`self_password`（空/MASK 语义见 `config_cmd.rs:111-123`）。
- `BindParams<'a>`（`self_service/mod.rs:64-75`）：`bind_operator` 传给协议层的参数集合，字段为 `account`、`password`、`operator`、`phone`、`sms_password`（全为 `&'a str` 借用）。
- `OperatorBinding`（`self_service/mod.rs:198-203`）：`query_bind_status` 的返回项，字段为 `masked_account: String`（手机号前三后二掩码）、`password_set: bool`；命令层把它转成 `{account, passwordSet}` JSON（`commands/self_service.rs:132-140`）。
- `DownloadProgress`（`update/updater.rs:39-44`）：`download_update` 每 200ms 通过事件发给前端的进度结构，字段为 `downloaded: u64`、`total: u64`、`speed: u64`、`percent: f64`（`commands/updater.rs:150-155`、`181-188`）。

### 命令返回的 JSON 结构（内联构造，无 Rust 结构体）

`get_background_status`（构造点 `commands/background.rs:90-109`）：

| key | 来源 | 说明 |
|---|---|---|
| `serverAvailable` | `network.server_available` | 服务器可达性 |
| `loginPreparationMode` | `config.auto_login_on_preparation` | 准备期自动登录模式 |
| `checkCount` | `network.background_check_count` | 巡检次数 |
| `isRunning` | `task_manager.is_running("background_check")` | 巡检是否在运行 |
| `interval` | `config.background_check_interval` | 巡检间隔 |
| `enabled` | `config.enable_background_check` | 总开关 |
| `adapterStatuses` | 逐适配器组装（52-82） | 由 `watcher::adapter_status_entry` / `adapter_disconnected_entry` / `adapter_disabled_entry` 生成 |
| `online` | 由 `adapterStatuses` 任一 `online=true` 推导（84） | 整体在线判定 |
| `currentSsid` | `network.current_ssid` | 当前 SSID |
| `onCampusNetwork` | `network.on_campus_network` | 是否校园网 |
| `enableNetworkNameCheck` | `config.enable_network_name_check` | 网络名校验开关 |
| `requiredNetworkName` | `config.required_network_name` | 要求的网络名 |
| `campusWifi` / `campusWired` / `a1OnCampus` / `a2OnCampus` / `a1CampusMessage` / `a2CampusMessage` | 全部硬编码 `serde_json::Value::Null`（103-108） | 历史遗留占位，桌面端未填充（见 Known Issues） |

`get_init_data`（构造点 `commands/system.rs:136-150`）：`config`（掩码）、`accounts`、`version`（编译期 `env!("APP_VERSION")`）、`autoLaunch`、`gpuInfo`、`refreshRate`、`adapters`、`adapterDetails`、`disabledAdapters`、`activeAccount`、`notificationEnabled`、`isAutoStart`（进程参数含 `--autostart`）、`backgroundStatus`（内嵌上面的后台状态）。

`check_campus_status`（`commands/network_cmd.rs:59-67`）：`onCampusNetwork`、`currentSsid`、`campusMessage`、`enableNetworkNameCheck`、`requiredNetworkName`、`campusWifi`、`campusWired`。

`check_portal_status`（`commands/network_cmd.rs:73-77` 与 `93-98`）：`online`、`message`、`reachable`、`loginAvailable`（提前返回分支只有前两个 key）。

`check_network_quality`：成功时序列化 `check_network_quality_async` 的结果结构；未运行分支由 `empty_quality_json(quality)` 生成（`commands/network_cmd.rs:15-17`），字段为 `gatewayLatency`、`externalLatency`、`averageExternalLatency`（均 -1）、`gateway`、`quality`、`timestamp`、`details`、`metrics`；`quality` 取值 `"disabled"` / `"busy"` / `"unknown"`。

`check_dns_doh_status`（`platform/dns_config.rs:539-543` 为 Windows 分支产物）：`adapters`（数组，每项 `name`、`dnsSource`、`dnsServers[]`、`profileDnsServers[]`、`adapterDnsOverridesProfile`）、`dohSupported`、`autoDohEnabled`；非 Windows 为 `{adapters: [], dohSupported: false}`。

`setup_dns_doh`：管理员路径直接返回 `dns_setup::setup_dns_doh_admin` 的 JSON（`network/dns_setup.rs:13`，非 Windows 版本在 205-207 恒返回失败）；helper 路径把结果文件 `details` 对象提升到顶层（`commands/network_cmd.rs:315-328`），保证两种路径结构一致。

`get_mirror_urls`：数组，每项 `{name, url, description}`（`commands/updater.rs:304-325`）。

## Data Flow

### 通用入站与出站

```text
前端 invoke('<snake_case 命令名>', { camelCase 参数 })
  → tauri::generate_handler!（app/startup.rs:63-120）
  → commands/<模块>.rs 的 #[tauri::command] 函数
      ├─ 参数校验（validate_* / is_iso_date / URL 前缀 …）
      ├─ 并发门（TaskLock::try_acquire / is_active）或身份门（identity_verified_recently）
      ├─ 状态读写：AppState { config, tasks, task_manager, network, exit, update_stats }（infra/state/mod.rs:125-132）
      └─ 转发到 auth / network / config / monitor / self_service / update / platform
  → Result<T, String>（Tauri 把 Err 变 rejected Promise，Ok 变 resolved）
```

出站事件不经过命令返回值，而是 `AppHandleExt`（`infra/command_context.rs:38-52`）：

- `notify_config_changed(config)`：由 `save_config_to_disk_encrypted` 统一发射（`commands/config_cmd.rs:16`），保证 `save_config` / `switch_account` / `save_current_as_account` / `delete_account` / `set_auto_launch` / `set_notification_enabled` / `stop_background_check` / `start|stop_latency_test` 全部经同一出口通知前端，且发送的是**掩码后**配置。
- `notify_update_download_progress(progress)`：`commands/updater.rs:150`、`181`。

只读命令常通过 `CommandContext::from_app(&app_handle)`（`infra/command_context.rs:22-25`）在 `spawn_blocking` 闭包内重新取 `AppState`（因为 `State<'_, AppState>` 不能跨线程 move），例如 `commands/login.rs:66`、`commands/background.rs:15`、`commands/network_cmd.rs:118`。

### 关键调用链

登录：

```text
invoke('do_login', {adapterName})
  → commands/login.rs:57 do_login
  → exit.auto_exit_cancelled = false；exit.set_deadline(None)（58-60）
  → spawn_blocking（65）→ tasks.is_logging_in.try_acquire()（67）
  → auth::service::full_login(&state, &app_handle, adapter)（74）
  → 成功后 commands/login.rs:79 → auth::service::post_login_handler
```

注销与状态复位：

```text
invoke('do_logout', {adapterName})
  → commands/login.rs:86 → tasks.is_logging_out.try_acquire()（92）
  → auth::service::full_logout（100）
  → 成功：sleep 1s（103）→ check_any_adapter_online（104，见下）→ EventBus::emit_login_log（107/113）
  → 返回后：adapter_name 为 None 时走"全量注销"（130-159：auto_exit_cancelled=true、failure_tracker::reset_all、
     network 状态重置、logout_protected_until = now+60s）
            否则走"单适配器注销"（160-177：只按检测结果重置 per-adapter 标志）
```

`check_any_adapter_online`（`commands/login.rs:16-54`）是命令层唯一的自建并发探测：两个裸 `std::thread::spawn` 并行 `check_portal_full`（38-47），每个线程先 `tauri::async_runtime::handle().inner().enter()`（30）以获得 Tokio runtime context（否则 reqwest 的 per-request 超时计时器会 panic，注释记录了 2026-09-05 注销崩溃事故）。

DNS / DoH 提权链：

```text
invoke('setup_dns_doh', {family})
  → commands/network_cmd.rs:261
  → resolve_adapter_names + filter_operation_adapters + is_blacklisted（284-289）
  → elevation::is_admin()（298）
      ├─ 是：network::dns_setup::setup_dns_doh_admin(&targets, &family)（299）
      └─ 否：helper_spawn::unique_result_path()（303）
             → helper_spawn::spawn_elevated_helper("dns", [targets…, "--family", family], &result_path, 30s)（308-313）
             → 子进程 --helper 模式（helper/mod.rs）写结果文件
             → 结果 details 提升顶层后返回（315-328）
```

更新下载与安装：

```text
invoke('download_update', {url}) → commands/updater.rs:24
  → tasks.is_downloading.try_acquire（31）→ https + host 白名单（33-56）→ 文件名清洗（60-72）
  → reqwest 流式下载到 %TEMP%/campus-login-update/<file>（104-171），每 200ms 发 DownloadProgress
  → 返回路径字符串

invoke('install_update', {filePath, checksumUrl}) → commands/updater.rs:195
  → canonical 校验在临时目录内（202-209）→ verify_download_sha256（227）→ .exe/.msi 启动安装（251-287）
```

账户命令的状态一致性：三条改写配置的账户命令都遵循"写文件 → 更新内存 → 经 `save_config_to_disk_encrypted` 落盘并广播"（`commands/account.rs:42-43`、`183-186`、`221-224`）。

## Connections

- [[desktop-auth]]：`do_login` / `do_logout` 转发的 `auth::service::full_login` / `full_logout` / `post_login_handler`，`check_portal_status` 依赖的 `auth::portal::check_portal_full`。
- [[desktop-config]]：`Config` 字段全集、`masked_for_display` / `PASSWORD_MASK`、`validate_config` 与 `validate_config_lenient`、`persist` 读写与加密。
- [[desktop-account-selfservice]]：`account` / `self_service` 协议实现、`BindParams`、`OperatorBinding`、身份门的上层语义。
- [[desktop-network-core]]：`Adapter` / `AdapterDetail` / `DisabledAdapter`、`resolve_adapter_names`、`filter_operation_adapters`、`is_blacklisted`、`select_adapter`、`check_network_quality_async`。
- [[desktop-network-dns]]：`network::dns_setup::setup_dns_doh_admin` 与注册表读取的语义。
- [[desktop-network-quality]]：`start_latency_test` / `stop_latency_test` / `check_network_quality` 与 `monitor::latency` 的关系。
- [[desktop-monitor]]：`watcher::start_background_check_inner` / `run_background_check` / `check_campus_network` 与 `get_background_status_value` 的适配器条目构造。
- [[desktop-helper-update]]：`helper/mod.rs` 的 `--helper dns|mac` 子进程实现、结果文件协议、`crate::update::updater` 的检查/校验/清理。
- [[desktop-infra]]：`AppState`、`CommandResult` / `AccountResult`、`TaskLock` / `TaskGuard` / `BackgroundTaskManager`、`CommandContext` / `AppHandleExt`、`EventBus`、`logger`、`lifecycle::cancel_auto_exit_inner`。
- [[desktop-app-lifecycle]]：窗口的 `minimize_window` / `close_window`、托盘行为、`exit` 状态机与心跳线程。
- [[desktop-platform]]：`platform::autostart` / `gpu` / `identity` / `helper_spawn` / `dns_config` / `elevation` 的 Win32 细节。
- [[desktop-frontend-hooks]]：`tauri-app/frontend/src/hooks/tauriApi.ts` 的命令名与参数映射。
- [[android-backend]]：安卓端同名命令的独立实现（`android/src-tauri/src/lib.rs:55-104`）。

## Known Issues

1. **后台状态 6 个字段恒为 `null`**：`commands/background.rs:103-108` 硬编码 `campusWifi` / `campusWired` / `a1OnCampus` / `a2OnCampus` / `a1CampusMessage` / `a2CampusMessage` 为 `serde_json::Value::Null`；安卓侧对应字段有实现（`android/src-tauri/src/monitor_loop.rs`），桌面端消费者必须容忍 null。
2. **`check_any_adapter_online` 静默吞错**：`commands/login.rs:17-20`，`get_adapters_cached()` 失败直接返回"全部离线"，注销后的状态复位会据此把 `any_adapter_online` 写 false，可能掩盖真实在线状态。
3. **`trigger_background_check` 的令牌可能游离**：`commands/background.rs:34-36`，`task_manager.cancel_token("background_check")` 不存在时新建一个 `CancellationToken` 并直接使用，该 token 未被登记到 `task_manager`；这类手动触发产生的任务无法被 `stop_background_check` 取消。
4. **`delete_account` 的错误返回形态不一致**：`commands/account.rs:195-196` 校验失败直接 `Err(String)`，而 `switch_account`（16-19）/ `save_current_as_account`（53-56）返回 `Ok(AccountResult::err(...))`；前端两种失败路径需要分别处理。
5. **`save_current_as_account` 回存旧账号失败仅告警**：`commands/account.rs:118-122`，旧账号写盘失败不阻断新账号保存，可能出现"旧账号未更新但已切换"的静默数据陈旧。
6. **`disable` 后的 `install_update` 不可达分支**：`commands/updater.rs:276-287` 的 `#[cfg(not(target_os = "windows"))]` 分支仍调用 `msiexec`，非 Windows 桌面下必失败（该组合不在项目支持范围，但分支存在）。
7. **`get_mirror_urls` 与下载白名单不一致**：镜像生成只给 4 个域名（`commands/updater.rs:304-325`），而 `download_update` 白名单有 13 项（39-53）；前端若自造镜像 URL，仍可能被下载命令接受但不在 UI 列表中。
8. **`check_network_quality` 的 `busy` 语义**：`commands/network_cmd.rs:179-182` 忙时返回 `quality="busy"` 的成功响应（`success` 字段不存在），前端需按 `quality` 分支处理，不能只看命令是否 reject。
9. **同步命令中的重活**：`get_init_data`（`commands/system.rs:112-151`）是 `fn`（非 `async`）却在内部做账号目录扫描（119）、网卡三列表枚举（126-128）、`detect_gpu_info()`（123）/`detect_display_refresh_rate()`（124）；GPU 与刷新率的首次 DXGI/GDI 枚举已由 `app/startup.rs:207-218` 的 `gpu-warmup` 后台线程预热（`platform/gpu.rs:14` 用 `OnceLock` 缓存），但网卡枚举与账号目录扫描仍在同步命令路径内执行，冷启动首次调用有阻塞风险。
10. **`get_log_retention_days` 是唯一非 `Result` 返回的命令**（`commands/system.rs:182-185`），错误无法上报，前端类型定义需区别对待。
11. **`download_update` 在 `content_length` 缺失时的上限判断**：`commands/updater.rs:97-100` 的早退只在服务端给长度时生效，实际保护依赖逐 chunk 判定（125-129），这部分逻辑正确但属于"无长度时无预检"。
12. **`do_logout` 的 `_state` 参数未使用**（`commands/login.rs:86`）：函数通过 `app_handle` 重新取 state（91 行），签名保留 `state` 只为与前端调用保持一致。
