---
title: 安卓端后端
type: module
source_files:
  - android/src-tauri/Cargo.toml
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/android_state.rs
  - android/src-tauri/src/battery_cmds.rs
  - android/src-tauri/src/campus_detect.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/cpu_affinity.rs
  - android/src-tauri/src/identity_gate.rs
  - android/src-tauri/src/login_history.rs
  - android/src-tauri/src/protocol_cmds.rs
  - android/src-tauri/src/monitor_loop.rs
tags: [安卓, Tauri, 命令面, 配置加密, 网络绑定]
---

## Overview

安卓端 Tauri 后端（crate `campus-login-android`，lib 名 `campus_login_android_lib`，见 `android/src-tauri/Cargo.toml:8-16`）是桌面协议核心 crate `campus-login` 的**薄壳**：通过 Cargo path 依赖（`android/src-tauri/Cargo.toml:8`，`campus-login = { path = "../../tauri-app/src-tauri" }`）复用桌面全部协议实现，**不复制任何协议逻辑**。

安卓侧只做三件事：**平台探针**（校园网判定与 wlan0 源 IP 选取，`campus_detect.rs`）、**状态管理**（Keystore 加密配置、验证门 TTL，`config_state.rs`/`identity_gate.rs`/`android_state.rs`）、**命令面包装**（`protocol_cmds.rs`、`campus_detect.rs`、`config_state.rs`、`battery_cmds.rs` 等暴露 `#[tauri::command]` 供前端调用）。

平台专属能力（AndroidKeyStore 加密、前台服务保活、进程绑 WiFi、绑小核、电池优化白名单）由 `android/plugins/` 下三个手写插件承接（`tauri-plugin-campus-network-bind` `Cargo.toml:34`、`tauri-plugin-campus-keystore` `Cargo.toml:37`、`tauri-plugin-campus-monitor-service` `Cargo.toml:40`）。

## Key Components

### 模块与入口（lib.rs）

| 名称 | 位置 | 用途 |
|------|------|------|
| 模块声明 | `android/src-tauri/src/lib.rs:11-26` | campus_detect / android_state / config_state / cpu_affinity / identity_gate / login_history / protocol_cmds / self_service_cmds / monitor_loop / account_cmds / system_cmds / quality_cmds / quality_history / update_cmds / face_model_cmds / battery_cmds |
| `pub fn run()` | `lib.rs:31` | `#[cfg_attr(mobile, tauri::mobile_entry_point)]` 入口；mobile 下依次初始化 network_bind/keystore/monitor_service/biometric/notification 插件并 `manage(AndroidState::default())`（`lib.rs:33-40`），非 mobile 仅 manage（`lib.rs:41-42`） |
| `setup` 闭包 | `lib.rs:45-63` | 日志目录 `app_data_dir()/logs`，初始化失败仅 eprintln；调用 `monitor_loop::run_startup_tasks`（`lib.rs:45`）；spawn `config_state::cleanup_stale_config_tmp`（`lib.rs:45`） |
| `invoke_handler` 注册命令 | `lib.rs:66-122` | 注册 `campus_detect::check_campus_status`、`campus_detect::request_wifi_ssid_permission`、`config_state::get_config`、`config_state::save_config`、`battery_cmds::*` 等 |
| `RunEvent::ExitRequested` 处理 | `lib.rs:130-132` | 调 `api.prevent_exit()` 防止划掉任务杀前台服务 |

### android_state.rs

| 名称 | 位置 | 用途 |
|------|------|------|
| `pub(crate) static WATCHER_FAILED: AtomicBool` | `android_state.rs:14` | WiFi watcher 注册失败降级标志，独立静态因 State 早期不可得 |
| `pub struct AndroidState` | `android_state.rs:17-21` | 全局状态容器：`cached_source_ip: Mutex<Option<Ipv4Addr>>`（检测阶段写入供登录复用）、`config: Mutex<Option<Settings>>`（明文配置内存态，密码仅存内存） |

### battery_cmds.rs

| 名称 | 位置 | 用途 |
|------|------|------|
| `pub struct BatteryOptimizationInfo` | `battery_cmds.rs:10-17` | 字段 `ignoring: bool`、`brand: String`、`has_vendor_target: bool` |
| `#[tauri::command] get_battery_optimization_info` | `battery_cmds.rs:20` | 调 `campus_monitor_service().get_battery_optimization_info()` 解析 JSON；非 mobile 返回 Err |
| `#[tauri::command] request_ignore_battery_optimizations` | `battery_cmds.rs:46` | 返回系统确认框是否弹出 |
| `#[tauri::command] open_vendor_battery_settings` | `battery_cmds.rs:65` | 跳厂商自启/省电页降级链 |
| `#[tauri::command] open_notification_settings` | `battery_cmds.rs:87` | 跳通知设置页，13+ 以外场景覆盖 |

### campus_detect.rs

| 名称 | 位置 | 用途 |
|------|------|------|
| `const PORTAL_PORT: u16 = 80` | `campus_detect.rs:9` | Portal 端口 |
| `const TCP_TIMEOUT: Duration = Duration::from_secs(3)` | `campus_detect.rs:10` | TCP 超时 |
| `pub fn pick_campus_source_ip` | `campus_detect.rs:15` | 排除蜂窝/link-local/loopback，wlan0 优先 |
| `pub async fn portal_reachable` | `campus_detect.rs:44` | 用 `tcp_connect_bounded` 经 `tokio::time::timeout`，仅 `Ok(Ok(_))` 为 true |
| `pub struct CampusProbe` | `campus_detect.rs:80-87` | 字段 `source`、`portal_ok`、`on_campus`、`wifi_off` |
| `pub async fn probe_campus` | `campus_detect.rs:65` | 枚举网卡；`wifi_off = source.is_none()` 短路；`on_campus = subnet || gateway_ok || portal_ok` |
| `pub fn campus_status_message` | `campus_detect.rs:129` | 三态文案 |
| `pub fn portal_host_of` | `campus_detect.rs:140` | 去 scheme/端口/路径 |
| `pub fn ssid_matches` | `campus_detect.rs:153` | 相等或 required="i-wxxy" 特判 iwxxy-2/3 |
| `async fn current_wifi_ssid` | `campus_detect.rs:163` | 经 network-bind 插件取，空串 None |
| `pub(crate) async fn probe_campus_with_ssid` | `campus_detect.rs:185` | SSID 感知版，名称命中直接 `on_campus=true` |
| `pub fn cache_source_ip` | `campus_detect.rs:207` | 写 `AndroidState.cached_source_ip` |
| `#[tauri::command] check_campus_status` | `campus_detect.rs:216` | 先 `ensure_wifi_bound`，取配置，`probe_campus_with_ssid`，`cache_source_ip`，返回 JSON |
| `#[tauri::command] request_wifi_ssid_permission` | `campus_detect.rs:244` | 调插件请求 NEARBY_WIFI_DEVICES 权限 |
| 测试模块 | `campus_detect.rs:251-321` | 覆盖选 IP/短路/SSID 匹配等 |

### config_state.rs

| 名称 | 位置 | 用途 |
|------|------|------|
| `pub const PASSWORD_MASK: &str = "***"` | `config_state.rs:10` | 掩码占位符 |
| `const CONFIG_FILE: &str = "config.json"` | `config_state.rs:11` | 配置文件名 |
| `lazy_static! CONFIG_IO_LOCK` | `config_state.rs:17-19` | 跨命令互斥 |
| `pub async fn config_io_lock()` | `config_state.rs:22` | 获取锁 |
| `pub struct Settings` | `config_state.rs:28-128` | 全量配置（camelCase 序列化），字段见下文结构体 |
| `impl Default for Settings` | `config_state.rs:130-200` | 默认值：background_check_interval=60_000、portal_url="http://10.1.99.100"、campus_gateway="10.2.127.254"、required_network_name="i-wxxy、iwxxy-2、iwxxy-3"、config_schema_version=7 等 |
| `pub struct CryptoBridge` | `config_state.rs:204-207` | 含 `encrypt`/`decrypt` Arc<dyn Fn> |
| `from_app` | `config_state.rs:210` | mobile 走 keystore 插件，not(mobile) 返回 Err |
| `struct EncodedSettings` | `config_state.rs:243-248` | disk 形态 settings + password_cipher |
| `pub async fn load_file` | `config_state.rs:250` | 无文件返 Default，损坏报错，密文解密失败置空不报错 |
| `pub fn password_saved_at` | `config_state.rs:270` | 查密文位 password_cipher 判据 |
| `pub async fn save_file` | `config_state.rs:278` | 加密失败 Err；写带 pid 临时文件后 rename；空密码不写密文 |
| `pub async fn cleanup_stale_config_tmp` | `config_state.rs:313` | 启清 `*.json.<pid>.tmp` 孤儿 |
| `pub async fn load_from` | `config_state.rs:326` | load_file + migrate_legacy_defaults + 旧 SSID 单值扩三值 |
| `async fn migrate_legacy_defaults` | `config_state.rs:355-417` | v0→v7 迁移，落盘失败静默幂等 |
| `pub async fn save_to` | `config_state.rs:419` | 写盘 |
| `pub fn masked_for_display` | `config_state.rs:424` | 密码非空变 *** 唯一出站出口 |
| `pub async fn emit_config_changed` | `config_state.rs:436` | emit "config-changed" 事件 |
| `pub fn resolve_password_field` | `config_state.rs:442` | 空/掩码回退当前，clear 置空 |
| `#[tauri::command] get_config` | `config_state.rs:454` | load_from 写缓存返 masked |
| `#[tauri::command] save_config` | `config_state.rs:469` | 持 IO 锁，load→merge→save，写缓存，调 `account_cmds::auto_create_account_for_current`，emit_config_changed |
| `pub async fn current_settings` | `config_state.rs:503` | 缓存优先否则读盘 |
| 测试模块 | `config_state.rs:522-827` | 覆盖加密往返、迁移、掩码、判据等 |

### cpu_affinity.rs

| 名称 | 位置 | 用途 |
|------|------|------|
| `pub fn little_core_ids() -> Vec<usize>` | `cpu_affinity.rs:8` | 读 cpufreq，取最低频簇；同构返空 |
| `pub fn pin_current_thread_to_little_cores() -> bool` | `cpu_affinity.rs:34` | android 下调 sched_setaffinity，失败静默 false，host 恒 false |

### identity_gate.rs

| 名称 | 位置 | 用途 |
|------|------|------|
| `static LAST_VERIFY_EPOCH_SECS: AtomicU64` | `identity_gate.rs:6` | 最近验证时间戳 |
| `pub const IDENTITY_VERIFY_TTL_SECS: u64 = 600` | `identity_gate.rs:8` | TTL |
| `pub fn note_identity_verified()` | `identity_gate.rs:18` | 写时间戳 |
| `pub fn identity_verified_recently() -> bool` | `identity_gate.rs:23` | TTL 内且时钟回拨视为过期 |
| `pub(crate) fn reset_for_tests()` | `identity_gate.rs:36` | 测试复位 |

### login_history.rs

| 名称 | 位置 | 用途 |
|------|------|------|
| `static LOGIN_HISTORY_LOCK: Mutex<()>` | `login_history.rs:8` | 追加串行 |
| `pub const LOGIN_HISTORY_MAX: usize = 100` | `login_history.rs:10` | 上限 |
| `pub struct LoginHistoryEntry` | `login_history.rs:13-21` | 字段 time, success, message, adapter(固定 "wlan0"), user, login_type(序列化 "type") |
| `pub fn read` | `login_history.rs:28` | 损坏备份 `.corrupt-*.bak` 后重置 |
| `pub fn append` | `login_history.rs:52` | 头插、truncate 100、tmp+rename 原子写；锁中毒上抛 Err |

### protocol_cmds.rs

| 名称 | 位置 | 用途 |
|------|------|------|
| `#[tauri::command] do_login` | `protocol_cmds.rs:10` | 手动登录；`adapter` 忽略；`ensure_wifi_bound`；凭据回退配置；`run_login`；历史落盘；成功清 MONITOR 熔断/保护 |
| `pub async fn run_login` | `protocol_cmds.rs:66` | `spawn_blocking` 桥接 `do_login_with_retry`，重试 3 |
| `#[tauri::command] do_logout` | `protocol_cmds.rs:115` | 注销；`ensure_wifi_bound`；`spawn_blocking` 调 `do_logout_with_retry`；历史；设 60s 保护期 |
| `#[tauri::command] check_portal_status` | `protocol_cmds.rs:147` | `ensure_wifi_bound`；`spawn_blocking` 调 `check_portal_full` |
| `cached_adapter_ip` | `protocol_cmds.rs:166` | 从 state 取缓存 IP |
| `#[tauri::command] ping_test` | `protocol_cmds.rs:177` | 返回 `"pong"` |
| `#[tauri::command] bind_to_wifi` | `protocol_cmds.rs:183` | cfg(mobile) 调插件；非 mobile Err |
| `#[tauri::command] accept_wifi_network` | `protocol_cmds.rs:201` | 同上 |
| `#[tauri::command] get_avoid_bad_wifi_status` | `protocol_cmds.rs:220` | cfg(mobile) 查 WRITE_SECURE_SETTINGS 状态 |
| `#[tauri::command] restore_written_settings` | `protocol_cmds.rs:240` | cfg(mobile) 还原系统网络设置 |
| `pub(crate) async fn ensure_wifi_bound` | `protocol_cmds.rs:269` | 全链路前置绑 WiFi；旁路代理 Supported 跳过；成功清池条件；调 accept_campus_wifi |
| `pub(crate) async fn accept_campus_wifi` | `protocol_cmds.rs:350` | 代劳系统确认，失败仅记日志 |

### 结构体字段详情

#### AndroidState (android_state.rs:17-21)
| 字段 | 类型 | 含义 |
|------|------|------|
| `cached_source_ip` | `Mutex<Option<Ipv4Addr>>` | 检测阶段选出的 wlan0 源 IP |
| `config` | `Mutex<Option<Settings>>` | 明文配置内存态（密码仅存内存） |

#### Settings (config_state.rs:28-128)
全量字段（camelCase 序列化）：
`user`, `password`, `self_hello_enabled`, `self_reverify_each_action`, `allow_2d_face_verify`, `operator`, `enable_night_operator_switch`, `night_operator_restore`, `enable_night_outbound_switch`, `outbound_priority`, `outbound_metric_restore`, `outbound_disabled_adapters`, `outbound_standby_route`, `night_outbound_restore`, `dns_optimize_adapters`, `device_aliases(HashMap)`, `auto_login_on_start`, `enable_background_check`, `background_check_interval`, `background_check_idle_interval`, `auto_login_on_preparation`, `max_disconnect_reconnect`, `auto_login_cooldown_secs`, `theme_mode`, `enable_notification`, `custom_theme_color`, `default_panel`, `display_name`, `active_account`, `enable_boot_autostart`, `enable_latency_test`, `latency_test_interval`, `enable_network_quality`, `skip_ttfb_in_latency`, `skip_content_in_latency`, `portal_url`, `fixed_gateway`, `required_network_name`, `enable_network_name_check`, `campus_gateway`, `campus_check_start_minutes`, `campus_check_end_minutes`, `scheduled_login_minutes`, `scheduled_logout_minutes`, `update_source`, `log_retention_days`, `config_schema_version`。

关键默认值（`impl Default` config_state.rs:130-200）：`background_check_interval=60_000`、`background_check_idle_interval=300_000`、`portal_url="http://10.1.99.100"`、`campus_gateway="10.2.127.254"`、`required_network_name="i-wxxy、iwxxy-2、iwxxy-3"`、`campus_check_start_minutes=460`、`campus_check_end_minutes=1380`、`scheduled_login_minutes=1440`、`scheduled_logout_minutes=1440`、`enable_night_operator_switch=false`、`enable_network_quality=false`、`latency_test_interval=600_000`、`config_schema_version=7`。

#### CryptoBridge (config_state.rs:204-207)
| 字段 | 类型 | 含义 |
|------|------|------|
| `encrypt` | `Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>` | 明文→密文 |
| `decrypt` | `Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>` | 密文→明文 |

#### EncodedSettings (config_state.rs:243-248)
| 字段 | 类型 | 含义 |
|------|------|------|
| `settings` | `Settings` | 其余配置（其内 password 恒空） |
| `password_cipher` | `String` | 登录密码密文 |

#### LoginHistoryEntry (login_history.rs:13-21)
| 字段 | 类型 | 含义 |
|------|------|------|
| `time` | `String` | 本地时间 |
| `success` | `bool` | 成功与否 |
| `message` | `String` | 协议消息 |
| `adapter` | `String` | 恒 `"wlan0"` |
| `user` | `String` | 学号 |
| `login_type` | `String` | 序列化 "type": `"manual"`/`"auto"` |

#### CampusProbe (campus_detect.rs:56-63)
| 字段 | 类型 | 含义 |
|------|------|------|
| `source` | `Option<Ipv4Addr>` | 选中源 IP |
| `portal_ok` | `bool` | Portal TCP 可达 |
| `on_campus` | `bool` | 综合判定 |
| `wifi_off` | `bool` | WiFi 未连接短路 |

#### BatteryOptimizationInfo (battery_cmds.rs:10-17)
| 字段 | 类型 | 含义 |
|------|------|------|
| `ignoring` | `bool` | 已在白名单 |
| `brand` | `String` | 品牌小写 |
| `has_vendor_target` | `bool` | 有专用跳转页 |

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                     Frontend (Android UI)                    │
│              invokes via tauri::invoke(...)                  │
└──────────────────────────┬───────────────────────────────────┘
                           │
                           ▼
┌──────────────────────────────────────────────────────────────┐
│                   lib.rs: run()                              │
│  ┌─────────────┐ ┌──────────────┐ ┌───────────────────────┐ │
│  │ Plugins     │ │ State        │ │ invoke_handler!       │ │
│  │ (network,   │ │ AndroidState │ │ commands:             │ │
│  │  keystore,  │ │ (IP/cache,  │ │ campus_detect,        │ │
│  │  monitor,   │ │  config)     │ │ config_state,         │ │
│  │  biometric, │ └──────────────┘ │ battery_cmds,         │ │
│  │  opener)    │                  │ protocol_cmds         │ │
│  └─────────────┘                  └───────────────────────┘ │
└──────────────────────────┬───────────────────────────────────┘
                           │
      ┌────────────────────┼────────────────────┐
      │                    │                    │
      ▼                    ▼                    ▼
┌──────────────┐  ┌──────────────────┐  ┌──────────────────┐
│ campus-login │  │ Android Plugins  │  │ Android OS APIs  │
│ (path dep)   │  │ (network-bind,   │  │ (Wi-Fi, Battery, │
│ auth::proto  │  │  keystore,       │  │  ForegroundSvc) │
│ portal       │  │  monitor-service)│  │                  │
│ subnet       │  └──────────────────┘  └──────────────────┘
└──────────────┘
```

## Data Flow

### 1. 复用桌面协议核心的方式（path 依赖）

`android/src-tauri/Cargo.toml:31` 声明 `campus-login = { path = "../../tauri-app/src-tauri" }`，编译期直接链接桌面 crate（lib 名 `campus_login_lib`）。安卓代码调用桌面实现如 `campus_login_lib::network::bound_socket::tcp_connect_bounded`（`campus_detect.rs:44`）、`auth::protocol::do_login_with_retry`（`protocol_cmds.rs:79`）、`auth::portal::check_portal_full`（`protocol_cmds.rs:160`）、`network::client::clear_client_pool`（`protocol_cmds.rs:294`）、`network::bound_socket::bind_capability`（`protocol_cmds.rs:275`）等。

### 2. 关键调用链

**启动（`lib.rs:47-61`）**
```text
tauri setup → 日志目录 app_data_dir()/logs
            → monitor_loop::run_startup_tasks(app.handle()) (本批未给实现)
            → spawn config_state::cleanup_stale_config_tmp(app_data_dir())
```

**校园网检测（`campus_detect.rs:216`）**
```text
invoke check_campus_status
 → protocol_cmds::ensure_wifi_bound(&app) (protocol_cmds.rs:269)
 → config_state::current_settings(&app) (config_state.rs:503)
 → probe_campus_with_ssid(&app, &settings) (campus_detect.rs:185)
 → cache_source_ip(&state, probe.source) (campus_detect.rs:203)
 → 返回 JSON {onCampusNetwork, currentSsid, campusMessage, enableNetworkNameCheck, requiredNetworkName, sourceIp, portalReachable}
```

**配置读写（`config_state.rs:454` / `:469`）**
```text
save_config{config, clear_password}
 → 持 config_io_lock() (config_state.rs:22,469)
 → CryptoBridge::from_app(&app) (config_state.rs:210)
 → load_from → resolve_password_field → save_to → save_file (加密失败 Err)
 → AndroidState.config 缓存刷新
 → account_cmds::auto_create_account_for_current (本批未给)
 → emit_config_changed (config_state.rs:436)
出站 masked_for_display (config_state.rs:424)
```

**登录（`protocol_cmds.rs:10`）**
```text
invoke do_login{user,password,operator,adapter}
 → ensure_wifi_bound (绑定成功清客户端池 + accept_campus_wifi)
 → 凭据回退 current_settings
 → run_login → spawn_blocking(auth::protocol::do_login_with_retry(...,3,...))
 → login_history::append(type="manual") (protocol_cmds.rs:46-52)
 → 成功：清 MONITOR.consecutive_failures、logout_protected_until_ms，置 was_online
```

**电池优化查询（`battery_cmds.rs:20`）**
```text
invoke get_battery_optimization_info
 → CampusMonitorServiceExt::get_battery_optimization_info()
 → 解析 JSON 为 BatteryOptimizationInfo
非 mobile 返回 Err
```

## Connections

- [[campus-login]]：桌面协议核心 crate，通过 path 依赖被安卓复用（`Cargo.toml:31`）。
- [[android-plugins]]：三个手写插件 `tauri-plugin-campus-network-bind`、`tauri-plugin-campus-keystore`、`tauri-plugin-campus-monitor-service` 承接平台专属能力。
- [[monitor-loop]]：`lib.rs:58` 调用 `monitor_loop::run_startup_tasks`，本批未给文件但模块已声明。
- [[account-cmds]]：`config_state::save_config` 调用 `account_cmds::auto_create_account_for_current`（声明于 lib.rs:11-26）。

## Known Issues

1. **同步 IO 在 async 上下文**：`do_login`（`protocol_cmds.rs:46-52`）与 `do_logout`（`protocol_cmds.rs:125-131`）直接在 async 任务里调用 `login_history::append`（内部 `std::fs` 同步读写，`login_history.rs:52`）。文件小、有 `LOGIN_HISTORY_LOCK` 串行，影响有限但会短时占用 async 线程。
2. **host 环境多处命令返回 `Err`**：`battery_cmds.rs:20/46/65/87` 在非 `mobile` 分支返回错误字符串；`protocol_cmds.rs:183-196/201-214/220-235/240-255` 在 `cfg(mobile)` 外返回 `Err("仅安卓端支持")`；`CryptoBridge::from_app` 在 host 下加解密恒失败（`config_state.rs:210`），host 测试需注入假桥。
3. **绑核/状态失败静默**：`cpu_affinity.rs:34` 绑核失败返回 false，`WATCHER_FAILED` 静态仅作降级标志（`android_state.rs:14`），不影响正确性只影响功耗/保活。
4. **配置迁移幂等静默**：`migrate_legacy_defaults`（`config_state.rs:355-417`）落盘失败静默，用户显式改回的值不被覆盖，但可能导致旧版默认值残留内存直至下次写盘。