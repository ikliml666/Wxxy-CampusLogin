---
title: android-rust-backend
type: module
source_files:
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/cpu_affinity.rs
  - android/src-tauri/src/face_model_cmds.rs
  - android/src-tauri/src/identity_gate.rs
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/login_history.rs
  - android/src-tauri/src/protocol_cmds.rs
  - android/src-tauri/src/account_cmds.rs
  - android/src-tauri/src/android_state.rs
  - android/src-tauri/src/battery_cmds.rs
  - android/src-tauri/src/campus_detect.rs
  - android/src-tauri/src/quality_cmds.rs
  - android/src-tauri/src/quality_history.rs
  - android/src-tauri/src/self_service_cmds.rs
  - android/src-tauri/src/system_cmds.rs
  - android/src-tauri/src/update_cmds.rs
  - android/src-tauri/src/vpn_cmds.rs
tags: [android, rust, tauri, backend, config, account, network, update]
---

## Overview

`android-rust-backend` 是 Android 端 Tauri 应用的核心 Rust 后端模块，源码位于 `android/src-tauri/src/`。它负责处理全量配置持久化（KeyStore 加密）、多账号生命周期管理、校园网探测与 WiFi 绑定、登录/注销协议命令、网络质量监控与历史、自助服务（运营商绑定/查账/改密）含生物识别门控、人脸模型按需下载、APK 更新流、电池优化跳转以及 CPU 大小核绑核等。模块通过 JNI 调用原生插件（`tauri_plugin_campus_network_bind`、`tauri_plugin_campus_keystore`、`tauri_plugin_campus_monitor_service`、`tauri_plugin_biometric`、`tauri_plugin_notification`）封装平台特性为 Tauri 命令。

核心设计原则：
1. **原子写入**：配置文件与历史记录均采用“写临时文件 + rename”策略（如 `config.json.<pid>.tmp`，见 `config_state.rs:311-317`）。
2. **IO 串行锁**：全局 `CONFIG_IO_LOCK`（`tokio::sync::Mutex<()>`，`config_state.rs:17-19`）保护配置读写，防并发竞态。
3. **安全隔离**：密码落盘前经 AndroidKeyStore AES-GCM 加密（`CryptoBridge`，`config_state.rs:214-251`），内存明文；敏感操作需通过 `identity_gate` 的 TTL 门控（`identity_gate.rs:23-33`）。
4. **健壮性**：硬件差异（CPU 亲和性、WiFi 绑定）静默失败不阻断主流程；损坏数据备份重置。

## Key Components

### 配置与状态管理 (`config_state.rs`)

**常量与全局锁：**
- `PASSWORD_MASK: &str = "***"` (config_state.rs:10)
- `CONFIG_FILE: &str = "config.json"` (config_state.rs:11)
- `lazy_static! CONFIG_IO_LOCK: tokio::sync::Mutex<()>` (config_state.rs:17-19)
- `pub async fn config_io_lock() -> tokio::sync::MutexGuard<'static, ()>` (config_state.rs:22-24)

**结构体与字段：**
- `struct Settings` (config_state.rs:26-136)，`#[serde(rename_all = "camelCase", default)]`。全部字段：
  - `user: String` (30)
  - `password: String` (31)
  - `self_hello_enabled: bool` (32)
  - `self_reverify_each_action: bool` (33)
  - `allow_2d_face_verify: bool` (36)
  - `operator: String` (37)
  - `enable_night_operator_switch: bool` (41)
  - `night_operator_restore: String` (43)
  - `enable_night_outbound_switch: bool` (47)
  - `outbound_priority: Vec<String>` (50)
  - `outbound_metric_restore: String` (53)
  - `outbound_disabled_adapters: String` (56)
  - `outbound_standby_route: String` (59)
  - `night_outbound_restore: String` (64)
  - `dns_optimize_adapters: Vec<String>` (67)
  - `device_aliases: std::collections::HashMap<String, String>` (72)
  - `auto_login_on_start: bool` (74)
  - `enable_background_check: bool` (75)
  - `background_check_interval: u64` (76)
  - `background_check_idle_interval: u64` (81)
  - `auto_login_on_preparation: bool` (82)
  - `max_disconnect_reconnect: u32` (83)
  - `auto_login_cooldown_secs: u64` (84)
  - `theme_mode: String` (86)
  - `enable_notification: bool` (87)
  - `custom_theme_color: String` (88)
  - `default_panel: String` (89)
  - `display_name: String` (93)
  - `active_account: String` (94)
  - `enable_boot_autostart: bool` (95)
  - `enable_latency_test: bool` (97)
  - `latency_test_interval: u64` (98)
  - `enable_network_quality: bool` (99)
  - `skip_ttfb_in_latency: bool` (100)
  - `skip_content_in_latency: bool` (101)
  - `portal_url: String` (103)
  - `fixed_gateway: String` (104)
  - `required_network_name: String` (105)
  - `enable_network_name_check: bool` (106)
  - `campus_gateway: String` (107)
  - `campus_check_start_minutes: u16` (109)
  - `campus_check_end_minutes: u16` (111)
  - `scheduled_login_minutes: u16` (113)
  - `scheduled_logout_minutes: u16` (115)
  - `update_source: String` (118)
  - `log_retention_days: u32` (120)
  - `vpn_account: String` (124)
  - `vpn_password_enc: String` (125)
  - `self_service_via_vpn: bool` (128)
  - `config_schema_version: u32` (135)
- `impl Default for Settings` (config_state.rs:138-211) 提供默认值：
  - `enable_night_operator_switch` 默认 `false` (149)
  - `background_check_interval` 默认 `60_000` (163)
  - `background_check_idle_interval` 默认 `300_000` (165)
  - `required_network_name` 默认 `"i-wxxy、iwxxy-2、iwxxy-3"` (189)
  - `campus_check_start_minutes` 默认 `460` (193)
  - `campus_check_end_minutes` 默认 `1380` (196)
  - `scheduled_login_minutes` 默认 `1440` (200)
  - `scheduled_logout_minutes` 默认 `1440` (201)
  - `config_schema_version` 默认 `7` (208)
- `struct CryptoBridge` (config_state.rs:214-218) 含 `encrypt: Arc<dyn Fn(&str)->Result<String,String>>`、`decrypt: Arc<dyn Fn(&str)->Result<String,String>>`；`impl CryptoBridge::from_app` (221-251) mobile 走 KeyStore 插件，非 mobile 返回错误闭包。
- `struct EncodedSettings` (config_state.rs:254-259) 磁盘形态：`settings: Settings` + `password_cipher: String`，camelCase 序列化。
- 测试模块 (config_state.rs:543-897) 覆盖加解密、迁移、掩码、判据等。

**函数与方法：**
- `pub async fn load_file(path, bridge) -> Result<Settings,String>` (config_state.rs:261-274) 无文件返默认；密文损坏解密失败置空密码。
- `pub fn password_saved_at(path) -> bool` (config_state.rs:281-287) 判据基于 `EncodedSettings.password_cipher` 非空。
- `pub async fn save_file(path, bridge, s) -> Result<(),String>` (config_state.rs:289-319) 密码加密后外露清空；临时文件带 pid 原子 rename。
- `pub async fn cleanup_stale_config_tmp(dir)` (config_state.rs:324-335) 清孤儿 `*.json.<pid>.tmp`。
- `pub async fn load_from(dir, bridge) -> Result<Settings,String>` (config_state.rs:337-347) 调 load_file + migrate_legacy_defaults + 旧 SSID 单值扩三值。
- `async fn migrate_legacy_defaults(dir, bridge, s)` (config_state.rs:366-428) v0→v7 迁移，每次落盘失败静默。
- `pub async fn save_to(dir, bridge, s)` (config_state.rs:430-432)
- `pub fn masked_for_display(s) -> serde_json::Value` (config_state.rs:437-446) 密码/VPN密文非空变 `***`。
- `pub async fn emit_config_changed(app, s)` (config_state.rs:452-455) 广播 `config-changed` 事件，payload `{config: 掩码}`。
- `pub fn resolve_password_field(incoming, current, clear) -> String` (config_state.rs:458-467) 空/MASK/clear 语义。
- `#[tauri::command] pub async fn get_config(app)` (config_state.rs:469-482) 读并缓存到 `AndroidState.config`。
- `#[tauri::command] pub async fn save_config(app, config: Settings, clear_password: Option<bool>)` (config_state.rs:515-551) 持 IO 锁，merge 密码，保存，更新缓存，调 `auto_create_account_for_current`，emit 事件。
- `pub async fn current_settings(app) -> Result<Settings,String>` (config_state.rs:524-541) 缓存优先。

### 账号管理 (`account_cmds.rs`)

**常量与正则：**
- `ACCOUNT_NAME_RE` (account_cmds.rs:13) 白名单 `^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$`
- `config_io_lock()` (account_cmds.rs:20) 转发 `config_state::config_io_lock`

**结构体与字段：**
- `struct AccountItem` (account_cmds.rs:27-40) 出站序列化 `id, displayName, user, operator, passwordSaved`（密码只出布尔）。
- `struct AccountResult` (account_cmds.rs:42-55) camelCase 字段 `success, message, activeAccount, displayName, config`；构造器 `ok` (57-60) / `ok_with_account` (61-63) / `err` (64-66)。

**函数与方法：**
- `sanitize_account_id` (account_cmds.rs:72-83) 非法字符替 `_`，截断 32。
- `validate_account_name` (account_cmds.rs:86-94) 1-32 字符，正则防路径穿越。
- `accounts_dir` (account_cmds.rs:96-102) 返回 `app_data_dir/accounts`。
- `read_account_card_info` (account_cmds.rs:111-121) 不解密读展示字段；`password_saved` 走 `config_state::password_saved_at` (119)。
- `list_account_items_sync` (account_cmds.rs:126-155) 过滤非 json/隐藏文件，按 id 排序。
- `load_account_file` (account_cmds.rs:165-170) 不存在返 `Ok(None)`。
- `persist_current` (account_cmds.rs:172-183) 保存主配置并刷新 `AndroidState.config` 缓存，返 `masked_for_display`。
- `#[tauri::command] list_accounts` (account_cmds.rs:157-163)
- `#[tauri::command] switch_account` (account_cmds.rs:211-253) 合并登录字段，清 `night_operator_restore`/`night_outbound_restore` (211-214)，调 `monitor_loop::restore_avoid_bad_wifi` (225)。
- `#[tauri::command] save_current_as_account` (account_cmds.rs:229-257)
- `#[tauri::command] create_account` (account_cmds.rs:259-294)
- `#[tauri::command] delete_account` (account_cmds.rs:333-362)
- `#[tauri::command] get_active_account` (account_cmds.rs:364-367)
- `#[tauri::command] rename_account` (account_cmds.rs:369-401) 业务错返 `Err(String)` (381) 而非 `AccountResult::err`。
- `merge_save_as_target` (account_cmds.rs:302-331) 纯函数：另存场景 `display_name` 置空。
- `rename_account_core` (account_cmds.rs:405-429) 只改 displayName，重名检查排除自身。
- `validate_display_name` (account_cmds.rs:434-447) trim 后 1..=32，禁控制字符。
- `auto_create_account_for_current` (account_cmds.rs:451-460) 保存后调用，失败仅 log_warn。
- `auto_create_account_in` (account_cmds.rs:473-510) 规则：空凭据跳过；id=sanitize；不存在建档；已存在且 user 不同撞库跳过 (493)；一致幂等短路 (502)；差异仅更新保留显示名。
- 测试模块 (account_cmds.rs:512-838)。

### 安卓全局状态 (`android_state.rs`)

**常量与结构体：**
- `WATCHER_FAILED` (android_state.rs:14) `pub(crate) static AtomicBool` 进程级。
- `AndroidState` (android_state.rs:16-20) `#[derive(Default)]` 含 `cached_source_ip: Mutex<Option<Ipv4Addr>>` 与 `config: Mutex<Option<Settings>>`（明文内存态）。

### 电池与通知设置 (`battery_cmds.rs`)

**结构体与命令（均 `#[cfg(mobile)]` 分支，非 mobile 返 Err）：**
- `BatteryOptimizationInfo` (battery_cmds.rs:8-17) 字段 `ignoring, brand, hasVendorTarget` camelCase。
- `get_battery_optimization_info` (battery_cmds.rs:19-41)
- `request_ignore_battery_optimizations` (battery_cmds.rs:45-61) 返 bool
- `open_vendor_battery_settings` (battery_cmds.rs:64-81) 返 json
- `open_notification_settings` (battery_cmds.rs:86-102) 返 bool

### 校园网探测 (`campus_detect.rs`)

**常量与结构体：**
- `PORTAL_PORT: u16 = 80` (campus_detect.rs:9)、`TCP_TIMEOUT: Duration = 3s` (campus_detect.rs:10)
- `CampusProbe` (campus_detect.rs:56-63) 含 `source, portal_ok, on_campus, wifi_off`

**函数与方法：**
- `pick_campus_source_ip` (campus_detect.rs:15-37) 排除蜂窝（`rmnet*`/`ccmni*`），wlan0 优先。
- `portal_reachable` (campus_detect.rs:44-53) 用 `bound_socket::tcp_connect_bounded` 直连。
- `probe_campus` (campus_detect.rs:65-125) 无源 IP 即 `wifi_off` 短路 (85)；三层判定 `on_campus = 子网 || 网关 || portal`。
- `campus_status_message` (campus_detect.rs:129-137) WiFi 未连接优先文案。
- `portal_host_of` (campus_detect.rs:140-149) 去 scheme/端口/路径。
- `ssid_matches` (campus_detect.rs:153-157) 默认 `i-wxxy` 特判 `iwxxy-2/3` 同源。
- `current_wifi_ssid` (campus_detect.rs:163-178) 经 network-bind 插件取，权限不足返 None。
- `probe_campus_with_ssid` (campus_detect.rs:185-200) 名称检查开启时 SSID 命中直接判在，返 `(probe, ssid)`。
- `cache_source_ip` (campus_detect.rs:203-210) 写 `AndroidState.cached_source_ip`。
- `#[tauri::command] check_campus_status` (campus_detect.rs:216-238) 先 `protocol_cmds::ensure_wifi_bound`，返 json。
- `#[tauri::command] request_wifi_ssid_permission` (campus_detect.rs:244-249) 触发 NEARBY_WIFI_DEVICES 权限。
- 测试 (campus_detect.rs:251-321)

### 协议命令面 (`protocol_cmds.rs`)

**函数与命令：**
- `#[tauri::command] pub async fn do_login(user,password,operator,adapter,app,state)` (protocol_cmds.rs:9-63) 先 `ensure_wifi_bound`；凭据空回退 `config_state::current_settings`；调 `run_login`；写历史；成功清熔断/注销保护/置 was_online。
- `pub async fn run_login(user,password,operator,state) -> Result<Value,String>` (protocol_cmds.rs:66-90) `spawn_blocking` 调 `campus_login_lib::auth::protocol::do_login_with_retry`。
- `#[tauri::command] pub async fn do_logout(user,adapter,app,state)` (protocol_cmds.rs:115-165) 绑 WiFi；回退 user；`spawn_blocking` 调 `do_logout_with_retry`；设 `logout_protected_until_ms` +60s。
- `#[tauri::command] pub async fn check_portal_status(adapter_ip,app,state)` (protocol_cmds.rs:146-164) 绑 WiFi；`spawn_blocking` 调 `check_portal_full`。
- `fn cached_adapter_ip(state) -> Option<String>` (protocol_cmds.rs:170-178) 取 `AndroidState.cached_source_ip`。
- `#[tauri::command] pub fn ping_test() -> &'static str` (protocol_cmds.rs:176-179) "pong"
- `#[tauri::command] pub fn bind_to_wifi(app)` (protocol_cmds.rs:182-196) mobile 调插件。
- `#[tauri::command] pub fn accept_wifi_network(app)` (protocol_cmds.rs:200-214)
- `#[tauri::command] pub async fn get_avoid_bad_wifi_status(app)` (protocol_cmds.rs:219-235)
- `#[tauri::command] pub async fn restore_written_settings(app)` (protocol_cmds.rs:239-255)
- `pub(crate) async fn ensure_wifi_bound(app)` (protocol_cmds.rs:269-338) 旁路代理跳过；spawn_blocking 调 `bind_to_wifi`；绑成功清 HTTP 池；调 `accept_campus_wifi`。
- `pub(crate) async fn accept_campus_wifi(app)` (protocol_cmds.rs:350-390)

### 质量监控 (`quality_cmds.rs` & `quality_history.rs`)

**`quality_cmds.rs`：**
- `LATENCY_RUNNING` AtomicBool (quality_cmds.rs:12)、`LATENCY_GENERATION` AtomicU64 (quality_cmds.rs:17)
- `GOOD_LEVELS = ["excellent","great","good"]` (quality_cmds.rs:89)、`BACKOFF_STABLE_STREAK: u32 = 5` (quality_cmds.rs:90)、`BACKOFF_MAX_MS: u64 = 1_800_000` (quality_cmds.rs:91)
- `run_quality_once` (quality_cmds.rs:19-49) 取 `AndroidState.cached_source_ip` 作 adapter_ip，结果写 `quality_history::append`。
- `#[tauri::command] check_network_quality` (quality_cmds.rs:51-54)
- `#[tauri::command] start_latency_test` (quality_cmds.rs:56-74) 幂等（swap true 则返），配置读取失败复位标志 (67)
- `#[tauri::command] stop_latency_test` (quality_cmds.rs:76-83) 先推进代际再清标志
- `next_backoff_interval_ms` (quality_cmds.rs:95-101) 稳定计数每达 5 拍翻倍，封顶 `BACKOFF_MAX_MS`
- `latency_loop` (quality_cmds.rs:103-134) base_ms = `latency_test_interval.max(10_000)`；sleep 放循环尾部；退出时仅代际匹配才清 `LATENCY_RUNNING`。
- 测试 (quality_cmds.rs:136-170)

**`quality_history.rs`：**
- `QUALITY_HISTORY_LOCK` Mutex (quality_history.rs:10)、`QUALITY_HISTORY_MAX: usize = 100` (quality_history.rs:12)
- `QualityHistoryEntry` (quality_history.rs:110-118) camelCase：`timestamp, gatewayLatency, externalLatency, quality`；延迟负则落盘 `None`
- `history_path` (quality_history.rs:24-26)
- `read` (quality_history.rs:29-42) 损坏备 `quality_history.json.corrupt-{epoch}.bak` 后重置空
- `epoch_millis` (quality_history.rs:44-49)
- `append` (quality_history.rs:52-75) 持锁头插、截断、原子写
- 测试 (quality_history.rs:77-157)

### 自助服务 (`self_service_cmds.rs`)

**结构体与函数：**
- `CommandResult` (self_service_cmds.rs:10-26) `success, message, data`（`skip_serializing_if` None）
- `resolve_self_password` (self_service_cmds.rs:30-39) 前端传 `***`/空回退 `config.password`，皆无返 None
- `ensure_identity_gate` (self_service_cmds.rs:42-51) 总开关 `self_hello_enabled` 关放行，否则查 `identity_verified_recently()`
- `load_settings` (self_service_cmds.rs:53-55) 包装 `config_state::current_settings`
- `local_addr_of` (self_service_cmds.rs:57-63) 从 `AndroidState.cached_source_ip` 取 `IpAddr`
- `verify_biometric_identity` (self_service_cmds.rs:70-86) 仅 `success=true` 才 `note_identity_verified`
- `#[tauri::command] bind_operator` (self_service_cmds.rs:87-132) 带门+手机号 11 位校验
- `#[tauri::command] query_bind_status` (self_service_cmds.rs:134-166) 无门，返 cmcc/telecom/unicom 掩码
- `#[tauri::command] query_self_dashboard` (self_service_cmds.rs:168-193) 无门
- `is_iso_date` (self_service_cmds.rs:196-208) 严格 YYYY-MM-DD
- `#[tauri::command] query_self_online_log` (self_service_cmds.rs:210-239) 日期校验+起始不晚于结束
- `#[tauri::command] self_offline_session` (self_service_cmds.rs:241-269) 带门
- `#[tauri::command] change_self_password` (self_service_cmds.rs:275-312) 带门+新密码 6-11 位 ASCII
- `#[tauri::command] reveal_operator_credential` (self_service_cmds.rs:315-345) 强制门无视总开关
- 测试 (self_service_cmds.rs:347-392)

### 系统命令 (`system_cmds.rs`)

**函数与方法：**
- `get_init_data` (system_cmds.rs:6-29) 聚合 `config`(掩码)、`accounts`(list_account_items_sync)、`version`、补空默认 `adapters:[]`/`adapterDetails:[]`/`refreshRate:60` 等、`backgroundStatus` 取 `monitor_loop::status_value()`
- `get_logs` (system_cmds.rs:32-39) `spawn_blocking` 读最近日志（默认 200 行）
- `clear_logs` (system_cmds.rs:41-46)
- `get_log_retention_days` (system_cmds.rs:48-51)
- `set_log_retention_days` (system_cmds.rs:53-56)
- `get_debug_mode` (system_cmds.rs:58-61)
- `set_debug_mode` (system_cmds.rs:63-66)

### 应用更新 (`update_cmds.rs`)

**常量与全局状态：**
- `UPDATE_LOOP_RUNNING` (update_cmds.rs:16) AtomicBool
- `UPDATE_CHECKSUM` (update_cmds.rs:21) Mutex<Option<(String,String)>>
- `LAST_CHECK_ERROR` (update_cmds.rs:35) / `LAST_CHECK_TIME_MS` (update_cmds.rs:37)
- `DOWNLOAD_RUNNING` (update_cmds.rs:398) AtomicBool
- `VERSION_FILE` (update_cmds.rs:24)、`AUTO_CHECK_INTERVAL_SECS` (update_cmds.rs:26)、`BACKOFF_RETRY_SECS` (update_cmds.rs:29)
- `VERSION_MIRRORS` (update_cmds.rs:39-43)、`DOWNLOAD_ALLOWED_HOSTS` (update_cmds.rs:46-55)、`MAX_DOWNLOAD_BYTES` (update_cmds.rs:57)、`APK_STALE_SECS` (update_cmds.rs:62)

**结构体与字段：**
- `ReleaseAsset` (update_cmds.rs:74-78) `name, url, size`
- `UpdateInfo` (update_cmds.rs:80-94, camelCase) `has_update, latest_version, release_notes, assets, sha256_checksum, last_check_error, last_check_time`
- `DownloadProgress` (update_cmds.rs:96-102) `downloaded, total, speed, percent`
- `VersionFile` (update_cmds.rs:223-229, snake_case) `version, notes`

**函数与命令：**
- `update_dir(app)` (update_cmds.rs:66-71) 返回 `app_data_dir/files/update`
- `has_newer_version(current, latest)` (update_cmds.rs:105-125) 去 v 前缀逐段数值比较
- `http_client()` (update_cmds.rs:127-136) rustls 客户端
- `allowed_url(url)` (update_cmds.rs:139-159) 白名单唯一漏斗（pub(crate) 供 face_model_cmds 复用）
- `#[tauri::command] check_update(app)` (update_cmds.rs:163-166)
- `now_epoch_ms()` (update_cmds.rs:168-173)
- `last_check_error_snapshot()` (update_cmds.rs:175-177) / `last_check_time_snapshot()` (update_cmds.rs:179-182)
- `backoff_delay_secs(fail_count)` (update_cmds.rs:187-193)
- `check_and_record(app)` (update_cmds.rs:198-209) 统一记录错误/时间
- `version_urls(mirror_first)` (update_cmds.rs:211-217)
- `fetch_apk_assets(client, latest)` (update_cmds.rs:237-280) 返回 `(Vec<ReleaseAsset>, Option<String>, bool)`
- `check_update_inner(app)` (update_cmds.rs:282-333) 遍历源、解析、记账 checksum
- `start_update_check_loop(app)` (update_cmds.rs:340-382) 启动 5s 延迟 + 周期/退避循环
- `#[tauri::command] get_mirror_urls(github_url)` (update_cmds.rs:384-392)
- `#[tauri::command] download_update(app, url)` (update_cmds.rs:400-408) 外壳原子锁
- `download_update_inner(app, url)` (update_cmds.rs:410-502) 流式下载、进度节流、SHA256 校验
- `download_tag(url)` (update_cmds.rs:506-510) 提取 `/releases/download/<tag>/` 中的 tag
- `verify_file_sha256(path, expected)` (update_cmds.rs:517-544) 堆分配缓冲流式校验（pub(crate) 复用）
- `#[tauri::command] install_update(app, file_path)` (update_cmds.rs:548-570) 路径约束后调插件安装
- `cleanup_stale_apks(app)` (update_cmds.rs:576-610) 启动期清超龄 .apk
- 测试 (update_cmds.rs:612-739)

### 其他辅助模块

**`cpu_affinity.rs`：**
- `pub fn little_core_ids() -> Vec<usize>` (cpu_affinity.rs:8-31) 解析 `/sys/devices/system/cpu/cpu*/cpufreq/cpuinfo_max_freq`，同构返空。
- `pub fn pin_current_thread_to_little_cores() -> bool` (cpu_affinity.rs:34-50) 仅安卓绑核，host 恒 false。
- 测试 (cpu_affinity.rs:52-69)

**`face_model_cmds.rs`：**
- 常量 `FACE_MODEL_BASE` (face_model_cmds.rs:18)，`FACE_MODEL_MIRRORS` (20-24)，`FACE_MODEL_FILES` (26-51) 含 sha256
- `static FACE_DL_RUNNING: AtomicBool` (face_model_cmds.rs:54)
- `struct FaceModelFileState` (56-63)，`struct FaceModelsState` (65-72)，`struct FaceModelProgress` (74-81)
- `fn models_dir(app) -> Result<PathBuf,String>` (86-93) 路径 `app_data_dir/files/face-models`
- `async fn state_inner(app)` (96-118) 逐文件存在且哈希匹配
- `#[tauri::command] pub async fn face_models_state(app)` (120-123)
- `#[tauri::command] pub async fn face_models_download(app)` (125-133) 原子标志防重入
- `async fn download_models(app)` (135-184) 源排序跟随 `update_source`
- `async fn download_one(client, url, dest, name, app)` (187-243) 流式写 `.part` 原子改名，每 200ms 发进度事件
- 测试 (245-274)

**`identity_gate.rs`：**
- `static LAST_VERIFY_EPOCH_SECS: AtomicU64` (identity_gate.rs:6)
- `pub const IDENTITY_VERIFY_TTL_SECS: u64 = 600` (identity_gate.rs:8)
- `fn now_epoch_secs() -> u64` (10-15)
- `pub fn note_identity_verified()` (18-20)
- `pub fn identity_verified_recently() -> bool` (23-33) 从未验证或时钟回拨为假
- `pub(crate) fn reset_for_tests()` (36-38)
- 测试 (40-65)

**`login_history.rs`：**
- `static LOGIN_HISTORY_LOCK: Mutex<()>` (login_history.rs:8)
- `pub const LOGIN_HISTORY_MAX: usize = 100` (login_history.rs:10)
- `struct LoginHistoryEntry` (login_history.rs:12-21) 字段 `time,success,message,adapter,user,type(login_type)`
- `fn history_path(dir)` (23-25)
- `pub fn read(dir) -> Vec<LoginHistoryEntry>` (28-41) 损坏备份 `.corrupt-*.bak` 后重置
- `fn chrono_epoch_millis() -> u128` (43-48)
- `pub fn append(dir, success, message, user, login_type) -> Result<(),String>` (52-77) 头插、截断、原子写；adapter 固定 `wlan0` (63)
- 测试 (79-143)

**`lib.rs`：**
- `mod` 声明 (lib.rs:12-28)
- `pub fn run()` (lib.rs:56-141)：mobile 初始化插件并 `manage(AndroidState::default())` (35-44)；setup 中 `init_logger` (56)，调 `monitor_loop::run_startup_tasks` (60)，spawn `config_state::cleanup_stale_config_tmp` (62-64)；`invoke_handler` 注册命令 (67-129)；退出护栏 `RunEvent::ExitRequested` 调 `api.prevent_exit()` (136-138)

### 监控循环 (`monitor_loop.rs`)

**常量与状态：**
- `MONITOR` (monitor_loop.rs:15-17) `MonitorState`
- `OUTBOUND_RESTORE_FAIL_COUNT` (21), `OUTBOUND_RESTORE_MAX_FAILS` (23), `OUTBOUND_SWITCH_FAIL_COUNT` (26), `OUTBOUND_SWITCH_MAX_FAILS` (28), `MONITOR_GENERATION` (32), `LOGIN_ATTEMPT_LOCK` (37)
- `MonitorState` (39-69) 字段含 running, check_count, consecutive_failures, reconnect_count, last_login_attempt_ms, was_online, last_result, desired_interval_ms, idle_interval_ms, logout_protected_until_ms, wifi_event_ms, notified_online, scheduled_login_day, scheduled_logout_day
- `WIFI_EVENT_DELAY_MS` (74), `WIFI_LOST_EVENT_DELAY_MS` (79), `WIFI_EVENT_DEBOUNCE_MS` (82)
- `NIGHT_VERIFY_WAIT_SECS` 等 (1196-1199)

**函数与命令：**
- `wifi_event_is_burst_start` (87-89)
- `effective_interval_ms` (96-103)
- `should_attempt_login` (107-128)
- `status_value` (131-152)
- `is_running` (155-157)
- `emit_login_log` (166-170)
- `notify_system` (175-192)
- `with_vpn_hint` (198-204)
- `start_background_check` (207-256)
- `stop_background_check` (258-271)
- `start_wifi_watcher` (282-304), `stop_wifi_watcher` (308-320)
- `handle_wifi_event` (325-363)
- `trigger_background_check` (365-374)
- `get_background_status` (376-379)
- `get_boot_autostart` (383-397), `set_boot_autostart` (399-422)
- `get_notification_enabled` (426-428), `set_notification_enabled` (430-446)
- `run_startup_tasks` (458-529)
- `probe_with_retry` (535-553)
- `auto_login_on_start` (569-612)
- `monitor_tick_loop` (643-702)
- `outbound_switch_active` (706-711), `latest_settings` (716-723)
- `logout_for_outbound_switch` (731-756), `switch_logout_unconfirmed` (761-778)
- `portal_probe_on_little_cores` (784-821)
- `run_scheduled_actions` (835-1037)
- `persist_settings` (1042-1065)
- `night_switch_login` (1071-1097)
- `trigger_wifi_recheck` (1103-1124), `ensure_avoid_bad_wifi` (1129-1165), `restore_avoid_bad_wifi` (1168-1192)
- `verify_night_switch` (1229-1283) 等
- `run_check_once` (1339-1574)
- `notify_state` (1578-1586)
- 测试 (1588-1710)

## Architecture

```text
┌──────────────────────────────────────────────────────────┐
│ lib.rs::run (lib.rs:33)                                   │
│ init plugins, AndroidState, cleanup, register commands    │
└───────────────┬──────────────────────────────────────────┘
                │ invokes
┌───────────────▼──────────────┐   ┌──────────────────────┐
│ config_state / account_cmds  │   │ protocol_cmds        │
│ AndroidState cache (ip,conf) │◄──┤ do_login/logout      │
└───────┬───────────┬──────────┘   └───┬──────────────────┘
        │           │                  │ ensure_wifi_bound
┌───────▼──┐  ┌─────▼─────────┐  ┌─────▼──────────┐  ┌──────────────┐
│campus_   │  │quality_cmds   │  │self_service_   │  │update_cmds   │
│detect    │  │+history       │  │cmds+identity_  │  │+face_model_  │
│          │  │               │  │gate            │  │cmds          │
└──────────┘  └───────────────┘  └────────────────┘  └──────────────┘
```

## Data Flow

1. **配置加载与初始化**：
   - `lib.rs::setup` 调 `config_state::cleanup_stale_config_tmp` (lib.rs:62-64)。
   - `config_state::load_from` (config_state.rs:337-347) 读 `config.json`，执行 `migrate_legacy_defaults` (config_state.rs:366-428)，更新 `AndroidState.config` 缓存。
   - `system_cmds::get_init_data` (system_cmds.rs:6-29) 聚合掩码配置、账号列表、版本等返前端。

2. **账号切换流程**：
   - 前端调 `account_cmds::switch_account` (account_cmds.rs:185-227)。
   - 获 `CONFIG_IO_LOCK` (account_cmds.rs:18)。
   - 合并登录字段，清 `night_operator_restore`/`night_outbound_restore` (211-214)。
   - `persist_current` (account_cmds.rs:172-183) 落盘主配置，触发 `auto_create_account_for_current` (account_cmds.rs:451)。
   - 调 `monitor_loop::restore_avoid_bad_wifi` (account_cmds.rs:225)。

3. **网络连接与登录流程**：
   - `protocol_cmds::do_login` (protocol_cmds.rs:9-63) 调 `ensure_wifi_bound` (protocol_cmds.rs:269-338) 绑 WiFi 清 HTTP 池。
   - `run_login` (protocol_cmds.rs:66-90) `spawn_blocking` 调 `campus_login_lib`。
   - 成功清熔断/注销保护，写 `login_history::append` (protocol_cmds.rs:49)。

4. **网络质量监控流程**：
   - `quality_cmds::start_latency_test` (quality_cmds.rs:56-74) 生成 `LATENCY_GENERATION` (quality_cmds.rs:17)。
   - `latency_loop` (quality_cmds.rs:103-134) 循环内 `run_quality_once` (quality_cmds.rs:19-49) 取 `cached_source_ip`，结果追 `quality_history::append`。
   - 根据 `GOOD_LEVELS` 退避，最大 `BACKOFF_MAX_MS` (quality_cmds.rs:91)。

5. **应用更新流程**：
   - `update_cmds::check_update` (update_cmds.rs:163-166) 按 `update_source` 选源 (update_cmds.rs:284)。
   - 发现新版本 emit `update-available` 及通知 (update_cmds.rs:353-364)。
   - `download_update` (update_cmds.rs:400-408) 互斥下载，SHA256 校验 (update_cmds.rs:517)。
   - `install_update` (update_cmds.rs:548-570) 调 `CampusMonitorServiceExt::install_apk`，保留 APK 供 `cleanup_stale_apks` (update_cmds.rs:576) 清理。

6. **自助服务与安全门控**：
   - `self_service_cmds` 命令调 `ensure_identity_gate` (self_service_cmds.rs:42-51) 查 `identity_gate::identity_verified_recently` (identity_gate.rs:23-33)。
   - 未过则前端调 `verify_biometric_identity` (self_service_cmds.rs:70-86) 成功后记 `note_identity_verified` (identity_gate.rs:18-20)。
   - `resolve_self_password` (self_service_cmds.rs:30-39) 解析密码。

## Connections

* [[config-state]]: 配置状态管理，被几乎所有命令依赖。
* [[account-management]]: 账号生命周期管理，与配置状态紧密交互。
* [[network-monitoring]]: 网络监控主循环，依赖配置和账号状态。
* [[biometric-auth]]: 生物识别认证，通过 identity_gate 提供 TTL 门控。
* [[update-system]]: 应用更新系统，独立于业务逻辑，但依赖配置中的更新源设置。
* [[quality-history]]: 质量历史记录，被 quality_cmds 写入。
* [[login-history]]: 登录历史记录，被 protocol_cmds 和 monitor_loop 写入。
* [[campus-detect]]: 校园网探测，被 campus_detect 命令和 monitor_loop 使用。
* [[face-models]]: 人脸识别模型管理，独立模块，按需下载。
* [[battery-settings]]: 电池优化设置，辅助模块，提升应用存活率。
* [[cpu-affinity]]: CPU 亲和性设置，辅助模块，优化性能。

## Known Issues

- **version.json 形状陷阱**：真实为 snake_case `version/notes` (update_cmds.rs:295-301)，曾误用 camelCase 反序列化导致 `latest_version` 恒空 (update_cmds.rs:295-346 中 `VersionFile` 解析)。
- **栈溢出防护**：`verify_file_sha256` 必须用 `vec![0u8; 65536]` 堆缓冲 (update_cmds.rs:531-534)，因 WebView JavaBridge 小栈线程构造 async future 时栈数组致 SIGSEGV。
- **密码展示陷阱**：`password_saved` 必须走 `password_saved_at` 而非 `settings.password`（落盘清空）(account_cmds.rs:111-121, config_state.rs:281-287)。
- **蜂窝接口永不被选**：`pick_campus_source_ip` 排除 `rmnet*`/`ccmni*` (campus_detect.rs:15-37)。
- **更名错误通道歧义**：`rename_account` 业务错返 `Err(String)` (account_cmds.rs:381) 而 `switch_account` 返 `AccountResult::err` (account_cmds.rs:192)。
- **质量代际竞态**：`stop_latency_test` 先推进代际防旧循环清新标志 (quality_cmds.rs:77-84)；配置失败复位 `LATENCY_RUNNING` 防卡死 (quality_cmds.rs:67)。
- **安装删文件竞态**：`install_update` 调起安装器后立即返回不删 APK (update_cmds.rs:565-568)，保留供重试。
- **校验三态语义**：API 在线无 digest 记 `("tag","")` 哨兵拒装；API 不可用降级 emit `update-unverified` (update_cmds.rs:309-315, 481-500)。
- **迁移幂等**：`migrate_legacy_defaults` 失败静默下次重迁，用户主动改回的值不被覆盖 (config_state.rs:366-428)。
- **大小核绑核静默**：同构拓扑（频差<30%）返回空/false 不影响正确性 (cpu_affinity.rs:18-25, 34-50)。
- **出站切换态跳过巡检**：`run_check_once` 中 `night_outbound_restore` 非空则整轮跳过 (monitor_loop.rs:1354-1357)，避免误报掉线。
- **WiFi 事件延迟**：连上方向延迟 2500ms 等 DHCP，丢失方向零延迟 (monitor_loop.rs:74-79, 350)。
