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
  - android/src-tauri/src/account_cmds.rs
tags: [安卓, Tauri, 命令面, 配置加密, 网络绑定]
---

## Overview

安卓端 Tauri 后端（crate `campus-login-android`，lib 名 `campus_login_android_lib`，见 `android/src-tauri/Cargo.toml:8-9`）是桌面协议核心 crate `campus-login` 的**薄壳**：通过 Cargo path 依赖（`android/src-tauri/Cargo.toml:8`，`campus-login = { path = "../../tauri-app/src-tauri" }`）复用桌面全部协议实现，**不复制任何协议逻辑**。

安卓侧只做三件事：**平台探针**（校园网判定与 wlan0 源 IP 选取，`campus_detect.rs`）、**状态管理**（Keystore 加密配置、验证门 TTL，`config_state.rs`/`identity_gate.rs`/`android_state.rs`）、**命令面包装**（`protocol_cmds.rs`、`campus_detect.rs`、`config_state.rs`、`battery_cmds.rs`、`monitor_loop.rs` 等暴露 `#[tauri::command]` 供前端调用）。

平台专属能力（AndroidKeyStore 加密、前台服务保活、进程绑 WiFi、绑小核、电池优化白名单）由 `android/plugins/` 下三个手写插件承接（`tauri-plugin-campus-network-bind` `Cargo.toml:34`、`tauri-plugin-campus-keystore` `Cargo.toml:37`、`tauri-plugin-campus-monitor-service` `Cargo.toml:40`）。

## Key Components

### 模块与入口（lib.rs）

| 名称 | 位置 | 用途 |
|------|------|------|
| 模块声明 | `android/src-tauri/src/lib.rs:12-28` | campus_detect / android_state / config_state / cpu_affinity / identity_gate / login_history / protocol_cmds / self_service_cmds / vpn_cmds / monitor_loop / account_cmds / system_cmds / quality_cmds / quality_history / update_cmds / face_model_cmds / battery_cmds |
| `pub fn run()` | `lib.rs:32-33` | `#[cfg_attr(mobile, tauri::mobile_entry_point)]` 入口；mobile 下依次初始化 network_bind/keystore/monitor_service/biometric/notification 插件并 `manage(AndroidState::default())`（`lib.rs:35-42`），非 mobile 仅 manage（`lib.rs:43-44`） |
| `setup` 闭包 | `lib.rs:47-66` | 日志目录 `app_data_dir()/logs`（`lib.rs:47-52`），初始化失败仅 eprintln（`lib.rs:47-49`）；调用 `monitor_loop::run_startup_tasks`（`lib.rs:47`）；spawn `config_state::cleanup_stale_config_tmp`（`lib.rs:47-49`） |
| `invoke_handler` 注册命令 | `lib.rs:68-132` | 注册 `campus_detect::check_campus_status`、`campus_detect::request_wifi_ssid_permission`、`config_state::get_config`、`config_state::save_config`、`battery_cmds::*`、`protocol_cmds::*`、`monitor_loop::*` 等 |
| `RunEvent::ExitRequested` 处理 | `lib.rs:138-141` | 调 `api.prevent_exit()` 防止划掉任务杀前台服务 |

### android_state.rs

| 名称 | 位置 | 用途 |
|------|------|------|
| `pub(crate) static WATCHER_FAILED: AtomicBool` | `android_state.rs:14` | WiFi watcher 注册失败降级标志，独立静态因 State 早期不可得 |
| `pub struct AndroidState` | `android_state.rs:16-21` | 全局状态容器：`cached_source_ip: Mutex<Option<Ipv4Addr>>`（检测阶段写入供登录复用）、`config: Mutex<Option<Settings>>`（明文配置内存态，密码仅存内存） |

### battery_cmds.rs

| 名称 | 位置 | 用途 |
|------|------|------|
| `pub struct BatteryOptimizationInfo` | `battery_cmds.rs:8-17` | 字段 `ignoring: bool`、`brand: String`、`has_vendor_target: bool` |
| `#[tauri::command] get_battery_optimization_info` | `battery_cmds.rs:19-41` | 调 `campus_monitor_service().get_battery_optimization_info()` 解析 JSON；非 mobile 返回 Err（`battery_cmds.rs:36-40`） |
| `#[tauri::command] request_ignore_battery_optimizations` | `battery_cmds.rs:45-61` | 返回系统确认框是否弹出；非 mobile 返回 Err（`battery_cmds.rs:56-60`） |
| `#[tauri::command] open_vendor_battery_settings` | `battery_cmds.rs:63-81` | 跳厂商自启/省电页降级链；非 mobile 返回 Err（`battery_cmds.rs:76-80`） |
| `#[tauri::command] open_notification_settings` | `battery_cmds.rs:83-102` | 跳通知设置页，13+ 以外场景覆盖；非 mobile 返回 Err（`battery_cmds.rs:97-101`） |

### campus_detect.rs

| 名称 | 位置 | 用途 |
|------|------|------|
| `const PORTAL_PORT: u16 = 80` | `campus_detect.rs:9` | Portal 端口 |
| `const TCP_TIMEOUT: Duration = Duration::from_secs(3)` | `campus_detect.rs:10` | TCP 超时 |
| `pub fn pick_campus_source_ip` | `campus_detect.rs:15-37` | 排除蜂窝/link-local/loopback，wlan0 优先 |
| `pub async fn portal_reachable` | `campus_detect.rs:44-53` | 用 `tcp_connect_bounded` 经 `tokio::time::timeout`，仅 `Ok(Ok(_))` 为 true |
| `pub struct CampusProbe` | `campus_detect.rs:80-88` | 字段 `source`、`portal_ok`、`on_campus`、`wifi_off` |
| `pub async fn probe_campus` | `campus_detect.rs:65-125` | 枚举网卡；`wifi_off = source.is_none()` 短路；`on_campus = subnet || gateway_ok || portal_ok` |
| `pub fn campus_status_message` | `campus_detect.rs:129-137` | 三态文案（WiFi 未连接优先） |
| `pub fn portal_host_of` | `campus_detect.rs:140-149` | 去 scheme/端口/路径 |
| `pub fn ssid_matches` | `campus_detect.rs:153-157` | 相等或 required="i-wxxy" 特判 iwxxy-2/3 |
| `async fn current_wifi_ssid` | `campus_detect.rs:163-178` | 经 network-bind 插件取，空串 None |
| `pub(crate) async fn probe_campus_with_ssid` | `campus_detect.rs:185-200` | SSID 感知版，名称命中直接 `on_campus=true` |
| `pub fn cache_source_ip` | `campus_detect.rs:207-214` | 写 `AndroidState.cached_source_ip` |
| `#[tauri::command] check_campus_status` | `campus_detect.rs:215-238` | 先 `ensure_wifi_bound`，取配置，`probe_campus_with_ssid`，`cache_source_ip`，返回 JSON |
| `#[tauri::command] request_wifi_ssid_permission` | `campus_detect.rs:243-249` | 调插件请求 NEARBY_WIFI_DEVICES 权限 |
| 测试模块 | `campus_detect.rs:251-321` | 覆盖选 IP/短路/SSID 匹配等 |

### config_state.rs

| 名称 | 位置 | 用途 |
|------|------|------|
| `pub const PASSWORD_MASK: &str = "***"` | `config_state.rs:10` | 掩码占位符 |
| `const CONFIG_FILE: &str = "config.json"` | `config_state.rs:11` | 配置文件名 |
| `lazy_static! CONFIG_IO_LOCK` | `config_state.rs:17-19` | 跨命令互斥 |
| `pub async fn config_io_lock()` | `config_state.rs:22-24` | 获取锁 |
| `pub struct Settings` | `config_state.rs:26-136` | 全量配置（camelCase 序列化），字段见下文结构体 |
| `impl Default for Settings` | `config_state.rs:138-211` | 默认值：background_check_interval=60_000、portal_url="http://10.1.99.100"、campus_gateway="10.2.127.254"、required_network_name="i-wxxy、iwxxy-2、iwxxy-3"、config_schema_version=7 等 |
| `pub struct CryptoBridge` | `config_state.rs:213-218` | 含 `encrypt`/`decrypt` Arc<dyn Fn> |
| `from_app` | `config_state.rs:220-251` | mobile 走 keystore 插件，not(mobile) 返回 Err（`config_state.rs:242-249`） |
| `struct EncodedSettings` | `config_state.rs:253-259` | disk 形态 settings + password_cipher |
| `pub async fn load_file` | `config_state.rs:261-274` | 无文件返 Default，损坏报错，密文解密失败置空不报错 |
| `pub fn password_saved_at` | `config_state.rs:281-287` | 查密文位 password_cipher 判据 |
| `pub async fn save_file` | `config_state.rs:289-319` | 加密失败 Err；写带 pid 临时文件后 rename；空密码不写密文 |
| `pub async fn cleanup_stale_config_tmp` | `config_state.rs:324-335` | 启清 `*.json.<pid>.tmp` 孤儿 |
| `pub async fn load_from` | `config_state.rs:337-347` | load_file + migrate_legacy_defaults + 旧 SSID 单值扩三值 |
| `async fn migrate_legacy_defaults` | `config_state.rs:366-428` | v0→v7 迁移，落盘失败静默幂等 |
| `pub async fn save_to` | `config_state.rs:430-432` | 写盘 |
| `pub fn masked_for_display` | `config_state.rs:437-446` | 密码非空变 *** 唯一出站出口 |
| `pub async fn emit_config_changed` | `config_state.rs:452-455` | emit "config-changed" 事件 |
| `pub fn resolve_password_field` | `config_state.rs:458-467` | 空/掩码回退当前，clear 置空 |
| `#[tauri::command] get_config` | `config_state.rs:469-482` | load_from 写缓存返 masked |
| `#[tauri::command] save_config` | `config_state.rs:484-520` | 持 IO 锁，load→merge→save，写缓存，调 `account_cmds::auto_create_account_for_current`，emit_config_changed |
| `pub async fn current_settings` | `config_state.rs:522-541` | 缓存优先否则读盘 |
| 测试模块 | `config_state.rs:543-897` | 覆盖加密往返、迁移、掩码、判据等 |

### cpu_affinity.rs

| 名称 | 位置 | 用途 |
|------|------|------|
| `pub fn little_core_ids() -> Vec<usize>` | `cpu_affinity.rs:8-31` | 读 cpufreq，取最低频簇；同构返空 |
| `pub fn pin_current_thread_to_little_cores() -> bool` | `cpu_affinity.rs:34-50` | android 下调 sched_setaffinity，失败静默 false，host 恒 false（`cpu_affinity.rs:48-49`） |
| 测试模块 | `cpu_affinity.rs:52-69` | host 环境恒不绑定等 |

### identity_gate.rs

| 名称 | 位置 | 用途 |
|------|------|------|
| `static LAST_VERIFY_EPOCH_SECS: AtomicU64` | `identity_gate.rs:6` | 最近验证时间戳 |
| `pub const IDENTITY_VERIFY_TTL_SECS: u64 = 600` | `identity_gate.rs:8` | TTL |
| `pub fn note_identity_verified()` | `identity_gate.rs:18-20` | 写时间戳 |
| `pub fn identity_verified_recently() -> bool` | `identity_gate.rs:23-33` | TTL 内且时钟回拨视为过期 |
| `pub(crate) fn reset_for_tests()` | `identity_gate.rs:36-38` | 测试复位 |
| 测试模块 | `identity_gate.rs:40-66` | 从未验证/回拨守卫等 |

### login_history.rs

| 名称 | 位置 | 用途 |
|------|------|------|
| `static LOGIN_HISTORY_LOCK: Mutex<()>` | `login_history.rs:8` | 追加串行 |
| `pub const LOGIN_HISTORY_MAX: usize = 100` | `login_history.rs:10` | 上限 |
| `pub struct LoginHistoryEntry` | `login_history.rs:12-21` | 字段 time, success, message, adapter(固定 "wlan0"), user, login_type(序列化 "type") |
| `pub fn read` | `login_history.rs:28-41` | 损坏备份 `.corrupt-*.bak` 后重置 |
| `pub fn append` | `login_history.rs:52-77` | 头插、truncate 100、tmp+rename 原子写；锁中毒上抛 Err |
| 测试模块 | `login_history.rs:79-143` | 头插法/截断/损坏重置等 |

### protocol_cmds.rs

| 名称 | 位置 | 用途 |
|------|------|------|
| `#[tauri::command] do_login` | `protocol_cmds.rs:9-63` | 手动登录；`adapter` 忽略；`ensure_wifi_bound`；凭据回退配置；`run_login`；历史落盘；成功清 MONITOR 熔断/保护 |
| `pub async fn run_login` | `protocol_cmds.rs:66-90` | `spawn_blocking` 桥接 `do_login_with_retry`，重试 3 |
| `#[tauri::command] do_logout` | `protocol_cmds.rs:115-165` | 注销；`ensure_wifi_bound`；`spawn_blocking` 调 `do_logout_with_retry`；历史；设 60s 保护期 |
| `#[tauri::command] check_portal_status` | `protocol_cmds.rs:146-164` | `ensure_wifi_bound`；`spawn_blocking` 调 `check_portal_full` |
| `cached_adapter_ip` | `protocol_cmds.rs:166-174` | 从 state 取缓存 IP |
| `#[tauri::command] ping_test` | `protocol_cmds.rs:176-179` | 返回 `"pong"` |
| `#[tauri::command] bind_to_wifi` | `protocol_cmds.rs:182-196` | cfg(mobile) 调插件；非 mobile Err（`protocol_cmds.rs:191-195`） |
| `#[tauri::command] accept_wifi_network` | `protocol_cmds.rs:200-214` | 同上（`protocol_cmds.rs:209-213` 非 mobile Err） |
| `#[tauri::command] get_avoid_bad_wifi_status` | `protocol_cmds.rs:219-235` | cfg(mobile) 查 WRITE_SECURE_SETTINGS 状态；非 mobile Err（`protocol_cmds.rs:230-234`） |
| `#[tauri::command] restore_written_settings` | `protocol_cmds.rs:239-255` | cfg(mobile) 还原系统网络设置；非 mobile Err（`protocol_cmds.rs:250-254`） |
| `pub(crate) async fn ensure_wifi_bound` | `protocol_cmds.rs:269-338` | 全链路前置绑 WiFi；旁路代理 Supported 跳过；成功清池条件；调 accept_campus_wifi |
| `pub(crate) async fn accept_campus_wifi` | `protocol_cmds.rs:350-389` | 代劳系统确认，失败仅记日志 |

### monitor_loop.rs

| 名称 | 位置 | 用途 |
|------|------|------|
| `lazy_static! MONITOR` | `monitor_loop.rs:15-17` | 全局 `MonitorState` 实例 |
| `MonitorState` | `monitor_loop.rs:39-69` | 监控状态容器（见结构体详情） |
| `wifi_event_is_burst_start` | `monitor_loop.rs:84-89` | WiFi 事件风暴起点判定（纯函数） |
| `effective_interval_ms` | `monitor_loop.rs:96-103` | 巡检分档：WiFi+亮屏走基础间隔，其余走闲时 |
| `should_attempt_login` | `monitor_loop.rs:107-128` | 本 tick 是否应尝试登录（纯函数） |
| `status_value` | `monitor_loop.rs:131-152` | backgroundStatus 出站形状，展平 last_result 关键字段 |
| `start_background_check` | `monitor_loop.rs:207-256` | 启停 FGS + tokio 循环，刷间隔，启 WiFi watcher |
| `stop_background_check` | `monitor_loop.rs:258-271` | 停 FGS + WiFi watcher，推进代际 |
| `start_wifi_watcher` / `stop_wifi_watcher` | `monitor_loop.rs:282-320` | WiFi 变化监听注册/注销，失败置 `WATCHER_FAILED` |
| `handle_wifi_event` | `monitor_loop.rs:324-363` | WiFi 事件去抖+延迟触发 `run_check_once` |
| `trigger_background_check` | `monitor_loop.rs:365-374` | 手动触发检测（仅运行中） |
| `get_background_status` | `monitor_loop.rs:376-379` | 返回 `status_value()` |
| `run_startup_tasks` | `monitor_loop.rs:458-529` | 启动恢复：并行拉起后台检测/质量测试/自动登录/更新检查 |
| `run_check_once` | `monitor_loop.rs:1339-1574` | 单次检测：校园网判定→Portal探测→状态机→自动重登→emit |
| `notify_state` | `monitor_loop.rs:1578-1586` | 常驻通知状态码（1在线/2未连接/4WiFi未连接） |
| 测试模块 | `monitor_loop.rs:1588-1710` | 覆盖通知状态/登录判定/分档等 |

### 结构体字段详情

#### AndroidState (android_state.rs:16-21)
| 字段 | 类型 | 含义 |
|------|------|------|
| `cached_source_ip` | `Mutex<Option<Ipv4Addr>>` | 检测阶段选出的 wlan0 源 IP |
| `config` | `Mutex<Option<Settings>>` | 明文配置内存态（密码仅存内存） |

#### Settings (config_state.rs:26-136)
全量字段（camelCase 序列化，`config_state.rs:27`）：
`user` (`config_state.rs:30`)、`password` (`config_state.rs:31`)、`self_hello_enabled` (`config_state.rs:32`)、`self_reverify_each_action` (`config_state.rs:33`)、`allow_2d_face_verify` (`config_state.rs:34-36`)、`operator` (`config_state.rs:37`)、`enable_night_operator_switch` (`config_state.rs:38-41`)、`night_operator_restore` (`config_state.rs:42-43`)、`enable_night_outbound_switch` (`config_state.rs:44-47`)、`outbound_priority` (`config_state.rs:48-50`)、`outbound_metric_restore` (`config_state.rs:51-53`)、`outbound_disabled_adapters` (`config_state.rs:54-56`)、`outbound_standby_route` (`config_state.rs:57-59`)、`night_outbound_restore` (`config_state.rs:60-64`)、`dns_optimize_adapters` (`config_state.rs:65-67`)、`device_aliases` (`config_state.rs:68-72`, HashMap)、`auto_login_on_start` (`config_state.rs:74`)、`enable_background_check` (`config_state.rs:75`)、`background_check_interval` (`config_state.rs:76`)、`background_check_idle_interval` (`config_state.rs:77-81`)、`auto_login_on_preparation` (`config_state.rs:82`)、`max_disconnect_reconnect` (`config_state.rs:83`)、`auto_login_cooldown_secs` (`config_state.rs:84`)、`theme_mode` (`config_state.rs:86`)、`enable_notification` (`config_state.rs:87`)、`custom_theme_color` (`config_state.rs:88`)、`default_panel` (`config_state.rs:89`)、`display_name` (`config_state.rs:90-93`)、`active_account` (`config_state.rs:94`)、`enable_boot_autostart` (`config_state.rs:95`)、`enable_latency_test` (`config_state.rs:97`)、`latency_test_interval` (`config_state.rs:98`)、`enable_network_quality` (`config_state.rs:99`)、`skip_ttfb_in_latency` (`config_state.rs:100`)、`skip_content_in_latency` (`config_state.rs:101`)、`portal_url` (`config_state.rs:103`)、`fixed_gateway` (`config_state.rs:104`)、`required_network_name` (`config_state.rs:105`)、`enable_network_name_check` (`config_state.rs:106`)、`campus_gateway` (`config_state.rs:107`)、`campus_check_start_minutes` (`config_state.rs:108-110`)、`campus_check_end_minutes` (`config_state.rs:110-112`)、`scheduled_login_minutes` (`config_state.rs:112-115`)、`scheduled_logout_minutes` (`config_state.rs:115`)、`update_source` (`config_state.rs:117-118`)、`log_retention_days` (`config_state.rs:120`)、`vpn_account` (`config_state.rs:124`)、`vpn_password_enc` (`config_state.rs:125`)、`self_service_via_vpn` (`config_state.rs:126-128`)、`config_schema_version` (`config_state.rs:129-135`)。

关键默认值（`impl Default` `config_state.rs:138-211`）：`background_check_interval=60_000` (`config_state.rs:163`)、`background_check_idle_interval=300_000` (`config_state.rs:165`)、`portal_url="http://10.1.99.100"` (`config_state.rs:185`)、`campus_gateway="10.2.127.254"` (`config_state.rs:191`)、`required_network_name="i-wxxy、iwxxy-2、iwxxy-3"` (`config_state.rs:189`)、`campus_check_start_minutes=460` (`config_state.rs:193`)、`campus_check_end_minutes=1380` (`config_state.rs:196`)、`scheduled_login_minutes=1440` (`config_state.rs:200`)、`scheduled_logout_minutes=1440` (`config_state.rs:201`)、`enable_night_operator_switch=false` (`config_state.rs:149`)、`enable_network_quality=false` (`config_state.rs:182`)、`latency_test_interval=600_000` (`config_state.rs:178`)、`config_schema_version=7` (`config_state.rs:208`)。

#### CryptoBridge (config_state.rs:213-218)
| 字段 | 类型 | 含义 |
|------|------|------|
| `encrypt` | `Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>` | 明文→密文 (`config_state.rs:216`) |
| `decrypt` | `Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>` | 密文→明文 (`config_state.rs:217`) |

#### EncodedSettings (config_state.rs:253-259)
| 字段 | 类型 | 含义 |
|------|------|------|
| `settings` | `Settings` | 其余配置（其内 password 恒空，`config_state.rs:304`） |
| `password_cipher` | `String` | 登录密码密文 (`config_state.rs:258`) |

#### LoginHistoryEntry (login_history.rs:12-21)
| 字段 | 类型 | 含义 |
|------|------|------|
| `time` | `String` | 本地时间 (`login_history.rs:14`) |
| `success` | `bool` | 成功与否 (`login_history.rs:15`) |
| `message` | `String` | 协议消息 (`login_history.rs:16`) |
| `adapter` | `String` | 恒 `"wlan0"` (`login_history.rs:17`) |
| `user` | `String` | 学号 (`login_history.rs:18`) |
| `login_type` | `String` | 序列化 "type": `"manual"`/`"auto"` (`login_history.rs:19-20`) |

#### CampusProbe (campus_detect.rs:55-63)
| 字段 | 类型 | 含义 |
|------|------|------|
| `source` | `Option<Ipv4Addr>` | 选中源 IP (`campus_detect.rs:57`) |
| `portal_ok` | `bool` | Portal TCP 可达 (`campus_detect.rs:58`) |
| `on_campus` | `bool` | 综合判定 (`campus_detect.rs:59`) |
| `wifi_off` | `bool` | WiFi 未连接短路 (`campus_detect.rs:62`) |

#### BatteryOptimizationInfo (battery_cmds.rs:8-17)
| 字段 | 类型 | 含义 |
|------|------|------|
| `ignoring` | `bool` | 已在白名单 (`battery_cmds.rs:11-12`) |
| `brand` | `String` | 品牌小写 (`battery_cmds.rs:13-14`) |
| `has_vendor_target` | `bool` | 有专用跳转页 (`battery_cmds.rs:15-16`) |

#### MonitorState (monitor_loop.rs:39-69)
| 字段 | 类型 | 含义 |
|------|------|------|
| `running` | `AtomicBool` | 循环运行标志 (`monitor_loop.rs:42`) |
| `check_count` | `AtomicU64` | 检测计数 (`monitor_loop.rs:43`) |
| `consecutive_failures` | `AtomicU32` | 连续失败熔断计数 (`monitor_loop.rs:44`) |
| `reconnect_count` | `AtomicU32` | 在线周期内自动重连次数 (`monitor_loop.rs:45-46`) |
| `last_login_attempt_ms` | `AtomicU64` | 上次登录尝试时间戳 (`monitor_loop.rs:47`) |
| `was_online` | `AtomicBool` | 上一拍在线记忆 (`monitor_loop.rs:48`) |
| `last_result` | `Mutex<Option<serde_json::Value>>` | 最近一次检测结果 (`monitor_loop.rs:49`) |
| `desired_interval_ms` | `AtomicU64` | 目标检测间隔 (`monitor_loop.rs:50-52`) |
| `idle_interval_ms` | `AtomicU64` | 闲时巡检间隔 (`monitor_loop.rs:53-54`) |
| `logout_protected_until_ms` | `AtomicU64` | 注销保护期截止 (`monitor_loop.rs:55-56`) |
| `wifi_event_ms` | `AtomicU64` | 最近 WiFi 事件时间戳 (`monitor_loop.rs:57-58`) |
| `notified_online` | `AtomicU8` | 常驻通知展示状态 (1在线/2未连接/3非检测时段/4WiFi未连接) (`monitor_loop.rs:59-62`) |
| `scheduled_login_day` | `AtomicI32` | 定时登录当日触发标记 (`monitor_loop.rs:63-66`) |
| `scheduled_logout_day` | `AtomicI32` | 定时注销当日触发标记 (`monitor_loop.rs:67-68`) |

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
│  │  opener)    │                  │ protocol_cmds,        │ │
│  └─────────────┘                  │ monitor_loop          │ │
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

`android/src-tauri/Cargo.toml:31` 声明 `campus-login = { path = "../../tauri-app/src-tauri" }`，编译期直接链接桌面 crate（lib 名 `campus_login_lib`）。安卓代码调用桌面实现如 `campus_login_lib::network::bound_socket::tcp_connect_bounded`（`campus_detect.rs:48`）、`auth::protocol::do_login_with_retry`（`protocol_cmds.rs:79`）、`auth::portal::check_portal_full`（`protocol_cmds.rs:160`）、`network::client::clear_client_pool`（`protocol_cmds.rs:294`）、`network::bound_socket::bind_capability`（`protocol_cmds.rs:275`）等。

### 2. 关键调用链

**启动（`lib.rs:47-66`）**
```text
tauri setup → 日志目录 app_data_dir()/logs (lib.rs:48-53)
            → monitor_loop::run_startup_tasks(app.handle()) (lib.rs:60)
            → spawn config_state::cleanup_stale_config_tmp(app_data_dir()) (lib.rs:62-64)
```

**校园网检测（`campus_detect.rs:215-238`）**
```text
invoke check_campus_status
 → protocol_cmds::ensure_wifi_bound(&app) (protocol_cmds.rs:269)
 → config_state::current_settings(&app) (config_state.rs:522)
 → probe_campus_with_ssid(&app, &settings) (campus_detect.rs:185)
 → cache_source_ip(&state, probe.source) (campus_detect.rs:203)
 → 返回 JSON {onCampusNetwork, currentSsid, campusMessage, enableNetworkNameCheck, requiredNetworkName, sourceIp, portalReachable}
```

**配置读写（`config_state.rs:484-520`）**
```text
save_config{config, clear_password}
 → 持 config_io_lock() (config_state.rs:22,493)
 → CryptoBridge::from_app(&app) (config_state.rs:220)
 → load_from → resolve_password_field → save_to → save_file (加密失败 Err)
 → AndroidState.config 缓存刷新 (config_state.rs:509-512)
 → account_cmds::auto_create_account_for_current (lib.rs:22 模块声明, 此处调用 config_state.rs:515)
 → emit_config_changed (config_state.rs:452)
出站 masked_for_display (config_state.rs:437)
```

**登录（`protocol_cmds.rs:9-63`）**
```text
invoke do_login{user,password,operator,adapter}
 → ensure_wifi_bound (绑定成功清客户端池 + accept_campus_wifi)
 → 凭据回退 current_settings
 → run_login → spawn_blocking(auth::protocol::do_login_with_retry(...,3,...)) (protocol_cmds.rs:77-89)
 → login_history::append(type="manual") (protocol_cmds.rs:46-52)
 → 成功：清 MONITOR.consecutive_failures、logout_protected_until_ms，置 was_online (protocol_cmds.rs:56-61)
```

**电池优化查询（`battery_cmds.rs:19-41`）**
```text
invoke get_battery_optimization_info
 → CampusMonitorServiceExt::get_battery_optimization_info()
 → 解析 JSON 为 BatteryOptimizationInfo
非 mobile 返回 Err (battery_cmds.rs:36-40)
```

**后台监控循环（`monitor_loop.rs:1339-1574`）**
```text
run_check_once(app)
 → 出站切换态跳过 (monitor_loop.rs:1354-1357)
 → 校园网检测静默期判断 (monitor_loop.rs:1363-1383)
 → ensure_wifi_bound (monitor_loop.rs:1396)
 → probe_campus_with_ssid + cache_source_ip (monitor_loop.rs:1401-1404)
 → portal_probe_on_little_cores (绑小核探测, monitor_loop.rs:1421)
 → 状态机翻在线/掉线通知 (monitor_loop.rs:1448-1457)
 → should_attempt_login + run_login 自动重登 (monitor_loop.rs:1468-1523)
 → emit "background-check-result" (monitor_loop.rs:1560)
```

## Connections

- [[campus-login]]：桌面协议核心 crate，通过 path 依赖被安卓复用（`Cargo.toml:31`）。
- [[android-plugins]]：三个手写插件 `tauri-plugin-campus-network-bind`、`tauri-plugin-campus-keystore`、`tauri-plugin-campus-monitor-service` 承接平台专属能力（`Cargo.toml:34-40`）。
- [[monitor-loop]]：`lib.rs:60` 调用 `monitor_loop::run_startup_tasks`，模块在 `lib.rs:21` 声明。
- [[account-cmds]]：`config_state::save_config` 调用 `account_cmds::auto_create_account_for_current`（`config_state.rs:515`，声明于 `lib.rs:22`）。

## Known Issues

1. **同步 IO 在 async 上下文**：`do_login`（`protocol_cmds.rs:46-52`）与 `do_logout`（`protocol_cmds.rs:125-131`）直接在 async 任务里调用 `login_history::append`（内部 `std::fs` 同步读写，`login_history.rs:52-77`）。文件小、有 `LOGIN_HISTORY_LOCK` 串行，影响有限但会短时占用 async 线程。
2. **host 环境多处命令返回 `Err`**：`battery_cmds.rs:36-40/56-60/76-80/97-101` 在非 `mobile` 分支返回错误字符串；`protocol_cmds.rs:191-195/209-213/230-234/250-254` 在 `cfg(mobile)` 外返回 `Err("仅安卓端支持")`；`CryptoBridge::from_app` 在 host 下加解密恒失败（`config_state.rs:242-249`），host 测试需注入假桥（见 `config_state.rs:549-558`）。
3. **绑核/状态失败静默**：`cpu_affinity.rs:34-50` 绑核失败返回 false，`WATCHER_FAILED` 静态仅作降级标志（`android_state.rs:14`），不影响正确性只影响功耗/保活。
4. **配置迁移幂等静默**：`migrate_legacy_defaults`（`config_state.rs:366-428`）落盘失败静默，用户显式改回的值不被覆盖，但可能导致旧版默认值残留内存直至下次写盘。
5. **Release 档配置必须显式声明**：`Cargo.toml:50-58` 注释说明安卓构建的 workspace 根必须在此声明 `[profile.release]`（lto/codegen/strip），否则 .so 不 strip、无 LTO，体积可达两倍；且故意不设 `panic="abort"` 以避免前台服务随进程陪葬。
