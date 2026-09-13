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
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/protocol_cmds.rs
  - android/src-tauri/src/quality_cmds.rs
  - android/src-tauri/src/self_service_cmds.rs
  - android/src-tauri/src/account_cmds.rs
  - android/src-tauri/src/system_cmds.rs
  - android/src-tauri/src/update_cmds.rs
  - android/plugins/foreground-service/src/lib.rs
tags: [安卓, Tauri, 命令面, 配置加密, 后台监控, 网络绑定]
---

## Overview

安卓端 Tauri 后端（crate `campus-login-android`，lib 名 `campus_login_android_lib`，见 `android/src-tauri/Cargo.toml:2` 与 `:8`）是桌面协议核心 crate `campus-login` 的**薄壳**：它通过 Cargo path 依赖（`android/src-tauri/Cargo.toml:34`，`campus-login = { path = "../../tauri-app/src-tauri" }`）复用桌面全部协议实现，**不复制任何协议逻辑**；协议核心 crate 的 `lib.rs:1-20` 把 `account/auth/config/infra/network/platform/self_service` 设为跨平台可见面，而 `app/commands/helper/monitor/update` 用 `#[cfg(desktop)]` 门控（`tauri-app/src-tauri/src/lib.rs:11-20`），安卓 target 不编译。

安卓侧只做三件事：**平台探针**（校园网判定与 wlan0 源 IP 选取，`campus_detect.rs`）、**状态管理**（Keystore 加密配置、监控状态机、验证门 TTL、多账号，`config_state.rs`/`monitor_loop.rs`/`identity_gate.rs`/`account_cmds.rs`）、**命令面包装**（48 个 `#[tauri::command]`，与桌面同名对齐，前端 `tauriApi` 两端一致）。

平台专属能力（AndroidKeyStore 加密、前台服务保活、进程绑 WiFi、绑小核、电池优化白名单、APK 安装）由 `android/plugins/` 下三个手写插件承接，见 [[android-plugins]]。

## Key Components

### 模块与入口（lib.rs）

| 名称 | 位置 | 用途 |
|------|------|------|
| `mod` 声明 14 个模块 | `android/src-tauri/src/lib.rs:10-23` | campus_detect / android_state / config_state / cpu_affinity / identity_gate / login_history / protocol_cmds / self_service_cmds / monitor_loop / account_cmds / system_cmds / quality_cmds / update_cmds / battery_cmds |
| `pub fn run()` | `lib.rs:28` | `#[cfg_attr(mobile, tauri::mobile_entry_point)]` 入口；注册插件、`manage(AndroidState)`、setup、`generate_handler!`、`run` |
| 插件注册（mobile） | `lib.rs:31-37` | 顺序：`campus-network-bind` → `campus-keystore` → `campus-monitor-service` → `tauri_plugin_biometric`（该 crate `#![cfg(mobile)]`，host 编译为空）→ `tauri_plugin_notification`；`manage(android_state::AndroidState::default())` |
| 插件注册（host） | `lib.rs:38-39` | `#[cfg(not(mobile))]`：只 `manage(AndroidState)`，三个自定义插件不注册 |
| `tauri_plugin_opener::init()` | `lib.rs:41` | 外链/APK 打开（非 mobile 门控，两端都注册） |
| `setup` 闭包 | `lib.rs:42-53` | 日志目录 = `app_data_dir()/logs`（与桌面 `infra::logger::get_log_dir` 的 android 分支一致）→ `init_logger`（`lib.rs:49`）→ `monitor_loop::run_startup_tasks(app.handle().clone())`（`lib.rs:51`） |
| `generate_handler!` 命令注册 | `lib.rs:55-104` | 共 **48** 个命令（清单见下节） |

### 命令总表（48 个，逐一）

“桌面对应物”列中 `startup.rs:N` 指注册位置 `tauri-app/src-tauri/src/app/startup.rs:N`。

| # | 命令 | 定义位置 | 用途 | 桌面对应物 |
|---|------|----------|------|-----------|
| 1 | `ping_test` | `protocol_cmds.rs:169` | 返回 `"pong"` 的连通性自检 | 无（安卓独有） |
| 2 | `bind_to_wifi` | `protocol_cmds.rs:175` | 进程网络绑定 WLAN；`cfg(mobile)` 调 network-bind 插件返回 `{"bound":..}` | 无（安卓独有） |
| 3 | `accept_wifi_network` | `protocol_cmds.rs:193` | 让系统接受“无互联网”的 WiFi（captive portal） | 无（安卓独有） |
| 4 | `do_login` | `protocol_cmds.rs:10` | 手动登录；凭据空/掩码回退已存配置；写登录历史；成功清熔断与注销保护 | `commands/login.rs::do_login`（`startup.rs:67`） |
| 5 | `do_logout` | `protocol_cmds.rs:88` | 两步注销；成功后设 60s 注销保护期 | `commands/login.rs::do_logout`（`startup.rs:68`） |
| 6 | `check_portal_status` | `protocol_cmds.rs:139` | Portal 状态页探测（端口 80 页面特征判在线） | `commands/network_cmd.rs::check_portal_status`（`startup.rs:72`） |
| 7 | `detect_campus` | `campus_detect.rs:124` | 阶段 1 检测卡（`onCampus`/`sourceIp`/`portalReachable`/`detail`） | 无（安卓独有；调用方见 Known Issues） |
| 8 | `check_campus_status` | `campus_detect.rs:152` | 校园网判定，形状对齐桌面 `network_cmd.rs` | `commands/network_cmd.rs::check_campus_status`（`startup.rs:71`） |
| 9 | `get_config` | `config_state.rs:297` | 读配置并刷新内存态，出站掩码（`***`） | `commands/config_cmd.rs::get_config`（`startup.rs:64`） |
| 10 | `save_config` | `config_state.rs:312` | 写配置；密码走 Keystore 密文落盘；空/掩码回退已存值 | `commands/config_cmd.rs::save_config`（`startup.rs:66`） |
| 11 | `verify_biometric_identity` | `self_service_cmds.rs:63` | 前端 BiometricPrompt 成功后写验证门时间戳 | `commands/self_service.rs::verify_windows_identity`（`startup.rs:113`，名字不同、语义对应） |
| 12 | `bind_operator` | `self_service_cmds.rs:69` | 运营商绑定（设验证门） | `commands/self_service.rs::bind_operator`（`startup.rs:111`） |
| 13 | `query_bind_status` | `self_service_cmds.rs:114` | 查询三运营商绑定状态（不设门） | `commands/self_service.rs::query_bind_status`（`startup.rs:112`） |
| 14 | `query_self_dashboard` | `self_service_cmds.rs:146` | 在线列表 + 登录历史（不设门） | `commands/self_service.rs::query_self_dashboard`（`startup.rs:115`） |
| 15 | `query_self_online_log` | `self_service_cmds.rs:186` | 指定日期区间在线日志（不设门） | `commands/self_service.rs::query_self_online_log`（`startup.rs:116`） |
| 16 | `self_offline_session` | `self_service_cmds.rs:215` | 强制下线会话（设验证门） | `commands/self_service.rs::self_offline_session`（`startup.rs:117`） |
| 17 | `reveal_operator_credential` | `self_service_cmds.rs:244` | 明文揭示手机号/短信密码（**无论开关强制验证门**） | `commands/self_service.rs::reveal_operator_credential`（`startup.rs:114`） |
| 18 | `list_accounts` | `account_cmds.rs:87` | 列举账号文件（过滤隐藏、排序） | `commands/account.rs::list_accounts`（`startup.rs:83`） |
| 19 | `switch_account` | `account_cmds.rs:115` | 切换账号（合并 user/password/operator/activeAccount 到主配置） | `commands/account.rs::switch_account`（`startup.rs:84`） |
| 20 | `save_current_as_account` | `account_cmds.rs:144` | 当前配置另存为账号 | `commands/account.rs::save_current_as_account`（`startup.rs:85`） |
| 21 | `delete_account` | `account_cmds.rs:168` | 删除账号文件；删的是当前账号则清 `activeAccount` | `commands/account.rs::delete_account`（`startup.rs:86`） |
| 22 | `get_active_account` | `account_cmds.rs:194` | 取 `activeAccount` | `commands/account.rs::get_active_account`（`startup.rs:87`） |
| 23 | `get_init_data` | `system_cmds.rs:6` | 启动聚合：config（掩码）+ accounts + version + backgroundStatus + 桌面专属字段空默认 | `commands/system.rs::get_init_data`（`startup.rs:102`） |
| 24 | `get_soc_info` | `system_cmds.rs:71` | 设备 SoC 型号与性能分档 tier 0-3 | `commands/system.rs::get_gpu_info`（`startup.rs:104`，语义对应、名字不同） |
| 25 | `get_logs` | `system_cmds.rs:30` | 读最近 N 行日志（默认 200） | `commands/system.rs::get_logs`（`startup.rs:100`） |
| 26 | `clear_logs` | `system_cmds.rs:40` | 清空日志 | `commands/system.rs::clear_logs`（`startup.rs:101`） |
| 27 | `get_log_retention_days` | `system_cmds.rs:47` | 读日志保留天数 | `commands/system.rs::get_log_retention_days`（`startup.rs:106`） |
| 28 | `set_log_retention_days` | `system_cmds.rs:52` | 设日志保留天数 | `commands/system.rs::set_log_retention_days`（`startup.rs:105`） |
| 29 | `get_debug_mode` | `system_cmds.rs:57` | 读调试模式 | `infra/logger.rs::get_debug_mode`（`startup.rs:119`） |
| 30 | `set_debug_mode` | `system_cmds.rs:62` | 设调试模式 | `infra/logger.rs::set_debug_mode`（`startup.rs:118`） |
| 31 | `start_background_check` | `monitor_loop.rs:160` | 启动后台监控（前台服务 + tokio 循环 + WiFi 监听），幂等；已跑则只刷新间隔 | `commands/background.rs::start_background_check`（`startup.rs:88`） |
| 32 | `stop_background_check` | `monitor_loop.rs:197` | 停止监控（置 `running=false`、注销 WiFi 监听、停前台服务） | `commands/background.rs::stop_background_check`（`startup.rs:89`） |
| 33 | `trigger_background_check` | `monitor_loop.rs:294` | 立即执行一次 `run_check_once` | `commands/background.rs::trigger_background_check`（`startup.rs:90`） |
| 34 | `get_background_status` | `monitor_loop.rs:300` | 取监控状态快照（`status_value()`） | `commands/background.rs::get_background_status`（`startup.rs:91`） |
| 35 | `get_boot_autostart` | `monitor_loop.rs:306` | 读开机自启（mobile 走插件，host 恒 `false`） | `commands/system.rs::get_auto_launch`（`startup.rs:92`，名字不同） |
| 36 | `set_boot_autostart` | `monitor_loop.rs:323` | 设开机自启（插件组件启停 + 配置落盘，受 `config_io_lock`） | `commands/system.rs::set_auto_launch`（`startup.rs:93`） |
| 37 | `get_notification_enabled` | `monitor_loop.rs:349` | 读通知开关 | `commands/system.rs::get_notification_enabled`（`startup.rs:94`） |
| 38 | `set_notification_enabled` | `monitor_loop.rs:354` | 设通知开关（落盘 + 刷内存态） | `commands/system.rs::set_notification_enabled`（`startup.rs:95`） |
| 39 | `get_battery_optimization_info` | `battery_cmds.rs:20` | 电池优化白名单状态（`ignoring`/`brand`/`hasVendorTarget`） | 无（平台专属，Windows 无对应 API） |
| 40 | `request_ignore_battery_optimizations` | `battery_cmds.rs:46` | 一次性申请加入白名单（返回确认框是否弹出） | 无（平台专属） |
| 41 | `open_vendor_battery_settings` | `battery_cmds.rs:65` | 跳厂商自启/省电页（降级链，返回 `{path,target,tried}`） | 无（平台专属） |
| 42 | `check_network_quality` | `quality_cmds.rs:40` | 单次网络质量检测 | `commands/network_cmd.rs::check_network_quality`（`startup.rs:78`） |
| 43 | `start_latency_test` | `quality_cmds.rs:45` | 启动定时质量测试循环（幂等） | `commands/network_cmd.rs::start_latency_test`（`startup.rs:79`） |
| 44 | `stop_latency_test` | `quality_cmds.rs:56` | 停止定时质量测试 | `commands/network_cmd.rs::stop_latency_test`（`startup.rs:80`） |
| 45 | `check_update` | `update_cmds.rs:126` | 检查更新（version.json 4 源降级 + GitHub API 拉 APK 资产） | `commands/updater.rs::check_update`（`startup.rs:107`） |
| 46 | `download_update` | `update_cmds.rs:287` | 流式下载 APK（白名单 + 500MB 上限 + SHA256 校验） | `commands/updater.rs::download_update`（`startup.rs:108`） |
| 47 | `get_mirror_urls` | `update_cmds.rs:271` | 生成 4 个下载源候选 | `commands/updater.rs::get_mirror_urls`（`startup.rs:110`） |
| 48 | `install_update` | `update_cmds.rs:405` | 交系统包安装器安装 APK（路径限定更新目录） | `commands/updater.rs::install_update`（`startup.rs:109`） |

### 公开函数 / 结构体 / 常量清单（按模块）

#### android_state.rs（13 行）

| 名称 | 位置 | 用途 |
|------|------|------|
| `struct AndroidState` | `android_state.rs:8-13` | `#[derive(Default)]`，Tauri `manage` 的进程级状态 |
| `pub cached_source_ip: Mutex<Option<Ipv4Addr>>` | `android_state.rs:10` | 检测阶段写入的 wlan0 源 IP，登录/注销/质量/自助服务共用 |
| `pub config: Mutex<Option<Settings>>` | `android_state.rs:12` | 明文配置内存态（密码仅存内存，落盘必经 Keystore 加密） |

#### cpu_affinity.rs（69 行）

| 名称 | 位置 | 用途 |
|------|------|------|
| `pub fn little_core_ids() -> Vec<usize>` | `cpu_affinity.rs:8` | 读 `/sys/devices/system/cpu/cpu{0..16}/cpufreq/cpuinfo_max_freq`，按最低频簇判小核；频率差 <30% 视为同构返回空 |
| `pub fn pin_current_thread_to_little_cores() -> bool` | `cpu_affinity.rs:34` | 仅 `target_os = "android"` 实做（`libc::sched_setaffinity`，`cpu_affinity.rs:41-46`）；host 恒 `false`（`cpu_affinity.rs:48-49`） |
| 单测 `host_环境_恒不绑定` / `小核识别_同构拓扑返回空` | `cpu_affinity.rs:58`、`:64` | 失败静默语义与边界 |

#### identity_gate.rs（66 行）

| 名称 | 位置 | 用途 |
|------|------|------|
| `static LAST_VERIFY_EPOCH_SECS: AtomicU64` | `identity_gate.rs:6` | 最近一次生物识别通过时间戳（epoch 秒） |
| `pub const IDENTITY_VERIFY_TTL_SECS: u64 = 600` | `identity_gate.rs:8` | 验证门 TTL，与桌面 `platform/identity.rs:31` 同值 |
| `fn now_epoch_secs() -> u64` | `identity_gate.rs:10` | 取当前 epoch 秒（失败回 0） |
| `pub fn note_identity_verified()` | `identity_gate.rs:18` | 写时间戳 |
| `pub fn identity_verified_recently() -> bool` | `identity_gate.rs:23` | 从未验证（0）或时钟回拨（`now < last`，`identity_gate.rs:29-31`）→ false；TTL 内 → true |
| `pub(crate) fn reset_for_tests()` | `identity_gate.rs:36` | `#[cfg(test)]` 复位 |

#### login_history.rs（142 行）

| 名称 | 位置 | 用途 |
|------|------|------|
| `static LOGIN_HISTORY_LOCK: Mutex<()>` | `login_history.rs:8` | 追加串行 |
| `pub const LOGIN_HISTORY_MAX: usize = 100` | `login_history.rs:10` | 记录上限 |
| `struct LoginHistoryEntry` | `login_history.rs:12-21` | 历史条目 |
| `fn history_path(dir) -> PathBuf` | `login_history.rs:23` | `dir/login-history.json` |
| `pub fn read(dir) -> Vec<LoginHistoryEntry>` | `login_history.rs:28` | 读；损坏时重命名 `.corrupt-<epoch_ms>.bak` 后重置（`login_history.rs:33-37`） |
| `fn chrono_epoch_millis() -> u128` | `login_history.rs:43` | 备份名时间戳 |
| `pub fn append(dir, success, message, user, login_type)` | `login_history.rs:51` | 头插 + 截断 100 + tmp/rename 原子写；`adapter` 固定 `"wlan0"`（`login_history.rs:62`） |

#### campus_detect.rs（224 行）

| 名称 | 位置 | 用途 |
|------|------|------|
| `const PORTAL_PORT: u16 = 80` / `const TCP_TIMEOUT = 3s` | `campus_detect.rs:8`、`:9` | 可达性探测参数 |
| `pub fn pick_campus_source_ip(interfaces) -> Option<Ipv4Addr>` | `campus_detect.rs:14` | 排除 `rmnet*`/`ccmni*` 蜂窝、link-local/回环/unspecified；wlan0 优先，否则首个合法接口 |
| `pub async fn portal_reachable(host, port, timeout) -> bool` | `campus_detect.rs:40` | TCP 连接可达（替代 ICMP） |
| `struct CampusProbe` | `campus_detect.rs:48-53` | 一次探测结果 |
| `pub async fn probe_campus(campus_gateway, portal_url) -> Result<CampusProbe, String>` | `campus_detect.rs:55` | 网卡枚举 → 源 IP → /18 子网判定（复用桌面 `network::subnet::is_same_subnet_18`，`campus_detect.rs:73`）→ Portal TCP → 网关 TCP 兜底 → `on_campus` 或运算 |
| `pub fn portal_host_of(url) -> String` | `campus_detect.rs:101` | 从 URL 抠 host（去 scheme/端口/路径） |
| `pub fn cache_source_ip(state, source)` | `campus_detect.rs:113` | 写入 `AndroidState.cached_source_ip` |
| `#[tauri::command] detect_campus` | `campus_detect.rs:124` | 绑 WiFi → 读配置 → `probe_campus` → 缓存源 IP → 返回 `{onCampus, sourceIp, portalReachable, detail}` |
| `#[tauri::command] check_campus_status` | `campus_detect.rs:152` | 同上探测，返回桌面形状 `{onCampusNetwork, currentSsid:"", campusMessage, enableNetworkNameCheck, requiredNetworkName, sourceIp, portalReachable}` |

#### protocol_cmds.rs（331 行）

| 名称 | 位置 | 用途 |
|------|------|------|
| `#[tauri::command] do_login` | `protocol_cmds.rs:10` | 参数 `user/password/operator/adapter`（`adapter` 用 `let _ = adapter;` 丢弃，`protocol_cmds.rs:18`）；`ensure_wifi_bound` → 凭据回退配置 → `run_login` → 写历史 → 成功清 `consecutive_failures`/`logout_protected_until_ms` |
| `pub async fn run_login(user, password, operator, state)` | `protocol_cmds.rs:60` | `spawn_blocking` 调桌面 `auth::protocol::do_login_with_retry(..., 3, ...)`（`protocol_cmds.rs:71-81`）；monitor_loop 自动重登复用 |
| `#[tauri::command] do_logout` | `protocol_cmds.rs:88` | `spawn_blocking` 调 `do_logout_with_retry`（`protocol_cmds.rs:107-117`）→ 写历史 → 设 60s 保护期（`protocol_cmds.rs:127-133`） |
| `#[tauri::command] check_portal_status` | `protocol_cmds.rs:139` | `spawn_blocking` 调桌面 `auth::portal::check_portal_full`（`protocol_cmds.rs:151-152`） |
| `fn cached_adapter_ip(state) -> Option<String>` | `protocol_cmds.rs:158` | 读缓存源 IP 为字符串 |
| `#[tauri::command] ping_test` | `protocol_cmds.rs:169` | `"pong"` |
| `#[tauri::command] bind_to_wifi` | `protocol_cmds.rs:175` | mobile 调插件；host 返回 Err（`protocol_cmds.rs:183-187`） |
| `#[tauri::command] accept_wifi_network` | `protocol_cmds.rs:193` | 同上模式 |
| `pub(crate) async fn ensure_wifi_bound(app)` | `protocol_cmds.rs:220` | 全链路前置绑 WiFi；成功且 `path != "already_bound"` 时清 HTTP 客户端池（`protocol_cmds.rs:232-236`）；日志走 `log_info!/log_warn!` + `eprintln!`；成功后调 `accept_campus_wifi` |
| `pub(crate) async fn accept_campus_wifi(app)` | `protocol_cmds.rs:291` | 代劳系统“仍然连接”确认，失败仅记日志 |

#### config_state.rs（587 行）

| 名称 | 位置 | 用途 |
|------|------|------|
| `pub const PASSWORD_MASK: &str = "***"` | `config_state.rs:10` | 掩码占位符（与桌面同款） |
| `const CONFIG_FILE: &str = "config.json"` | `config_state.rs:11` | 配置文件名 |
| `struct Settings` | `config_state.rs:15-72` | 全量配置（35 字段，见下节） |
| `impl Default for Settings` | `config_state.rs:74-123` | 默认值（含 2026-09-09/12/13 的三次默认值调整） |
| `struct CryptoBridge` | `config_state.rs:127-130` | 加解密桥（`Arc<dyn Fn>` 便于 host 注入假实现） |
| `impl CryptoBridge::from_app(app)` | `config_state.rs:133` | mobile 绑 keystore 插件；host 返回恒 Err 的实现（`config_state.rs:158-159`） |
| `struct EncodedSettings` | `config_state.rs:168-172` | 落盘形态：`settings` + `passwordCipher` + `selfPasswordCipher` |
| `pub async fn load_file(path, bridge)` | `config_state.rs:174` | 文件缺失 → `Settings::default()`；解密失败置空继续（`config_state.rs:183-188`） |
| `pub async fn save_file(path, bridge, s)` | `config_state.rs:192` | 加密两密码 → `Settings` 内密码清空 → tmp + rename 原子写；**加密失败直接 Err 不落明文**（`config_state.rs:204`、`:210`） |
| `pub async fn load_from(dir, bridge)` | `config_state.rs:225` | `load_file` + 迁移 |
| `async fn migrate_legacy_defaults(dir, bridge, s)` | `config_state.rs:241` | schema v0→v3、v3→v4 一次性迁移并落盘 |
| `pub async fn save_to(dir, bridge, s)` | `config_state.rs:268` | 写 `dir/config.json` |
| `pub fn masked_for_display(s) -> serde_json::Value` | `config_state.rs:273` | 非空密码 → `***`；唯一出站出口 |
| `pub fn resolve_password_field(incoming, current, clear) -> String` | `config_state.rs:285` | 空/掩码保留已存值，`clear` 显式清除 |
| `#[tauri::command] get_config` | `config_state.rs:297` | 读盘 + 刷内存态 + 掩码出站 |
| `#[tauri::command] save_config` | `config_state.rs:312` | 读当前 → 合并密码 → 落盘 → 刷内存态（**不联动后台/质量循环起停**，见 Known Issues） |
| `pub async fn current_settings(app) -> Result<Settings, String>` | `config_state.rs:342` | 缓存优先，未命中读盘（仍带 `#[allow(dead_code)] // Task 2 协议命令面接线` 陈旧标注） |

#### self_service_cmds.rs（318 行）

| 名称 | 位置 | 用途 |
|------|------|------|
| `struct CommandResult` | `self_service_cmds.rs:7-14` | `{success, message?, data?}`（`skip_serializing_if`） |
| `impl CommandResult::{ok_msg, err}` | `self_service_cmds.rs:17`、`:20` | 构造器 |
| `fn resolve_self_password(incoming, settings) -> Option<String>` | `self_service_cmds.rs:26` | 空/`***` 回退 `self_password`，皆无返回 None |
| `fn ensure_identity_gate(settings) -> Option<String>` | `self_service_cmds.rs:38` | `self_hello_enabled=false` 放行；否则要求 TTL 内已验证 |
| `async fn load_settings(app)` | `self_service_cmds.rs:49` | 包装 `current_settings` |
| `fn local_addr_of(state) -> Option<IpAddr>` | `self_service_cmds.rs:53` | 缓存源 IP → `IpAddr` |
| `#[tauri::command] verify_biometric_identity` | `self_service_cmds.rs:63` | 写 TTL 时间戳（`_consent_message` 不收） |
| `#[tauri::command] bind_operator` | `self_service_cmds.rs:69` | 校验学号/11 位手机号/短信密码 → 设门 → 调桌面 `self_service::bind_operator` |
| `#[tauri::command] query_bind_status` | `self_service_cmds.rs:114` | 三运营商结果转 `{cmcc,telecom,unicom}` JSON（`self_service_cmds.rs:130-138`） |
| `#[tauri::command] query_self_dashboard` | `self_service_cmds.rs:146` | 返回 `{onlineList, loginHistory}` |
| `fn is_iso_date(s) -> bool` | `self_service_cmds.rs:171` | `YYYY-MM-DD` 严格校验（桌面同构） |
| `#[tauri::command] query_self_online_log` | `self_service_cmds.rs:186` | 日期格式与顺序校验后调协议 |
| `#[tauri::command] self_offline_session` | `self_service_cmds.rs:215` | 设门 + `session_id` 非空校验 |
| `#[tauri::command] reveal_operator_credential` | `self_service_cmds.rs:244` | 无条件验门（`self_service_cmds.rs:257-259`） |

#### account_cmds.rs（238 行）

| 名称 | 位置 | 用途 |
|------|------|------|
| `static ACCOUNT_NAME_RE: Regex` | `account_cmds.rs:13` | `^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$`（防路径穿越） |
| `static CONFIG_IO_LOCK: tokio::sync::Mutex<()>` | `account_cmds.rs:17` | 配置读改写串行 |
| `pub async fn config_io_lock()` | `account_cmds.rs:21` | 跨命令共用（`save_config` 未使用，见 Known Issues） |
| `struct AccountResult` | `account_cmds.rs:25-35` | `{success, message?, activeAccount?, config?}` |
| `impl AccountResult::{ok, ok_with_account, err}` | `account_cmds.rs:38`、`:41`、`:44` | 构造器 |
| `pub fn validate_account_name(name)` | `account_cmds.rs:50` | 1-32 字符 + 白名单正则 |
| `pub fn accounts_dir(app) -> Result<PathBuf, String>` | `account_cmds.rs:60` | `app_data_dir()/accounts` |
| `pub fn list_account_names_sync(dir) -> Vec<String>` | `account_cmds.rs:68` | 过滤 `.` 前缀与空名，排序 |
| `#[tauri::command] list_accounts` | `account_cmds.rs:87` | `spawn_blocking` 包装 |
| `async fn load_account_file(path, bridge)` | `account_cmds.rs:94` | 不存在返回 None |
| `async fn persist_current(app, merged)` | `account_cmds.rs:101` | 落盘 + 刷内存态 + 返回掩码 JSON |
| `#[tauri::command] switch_account` | `account_cmds.rs:115` | 消毒 → `config_io_lock` → 载入账号 → 合并 `user/password/operator/activeAccount` |
| `#[tauri::command] save_current_as_account` | `account_cmds.rs:144` | 当前配置写成账号文件 + 更新 `activeAccount` |
| `#[tauri::command] delete_account` | `account_cmds.rs:168` | 删文件；命中当前账号则清字段 |
| `#[tauri::command] get_active_account` | `account_cmds.rs:194` | 读 `activeAccount` |

#### system_cmds.rs（160 行）

| 名称 | 位置 | 用途 |
|------|------|------|
| `#[tauri::command] get_init_data` | `system_cmds.rs:6` | 聚合返回；桌面专属字段补空默认（`gpuInfo: null`、`refreshRate: 60`、`adapters: []` 等，`system_cmds.rs:16-25`）防前端读 `undefined` 崩溃 |
| `#[tauri::command] get_logs` | `system_cmds.rs:30` | `spawn_blocking` 调桌面 `infra::logger::read_recent_logs` |
| `#[tauri::command] clear_logs` | `system_cmds.rs:40` | 桌面 `logger::clear_logs` |
| `#[tauri::command] get_log_retention_days` | `system_cmds.rs:47` | 桌面 `logger::get_log_retention_days` |
| `#[tauri::command] set_log_retention_days` | `system_cmds.rs:52` | 桌面同名 |
| `#[tauri::command] get_debug_mode` | `system_cmds.rs:57` | 桌面同名 |
| `#[tauri::command] set_debug_mode` | `system_cmds.rs:62` | 桌面同名 |
| `#[tauri::command] get_soc_info` | `system_cmds.rs:71` | `spawn_blocking(read_soc_info)` |
| `fn read_soc_info()` | `system_cmds.rs:77` | android 读 `ro.soc.model`/`ro.product.model`（`android_system_properties`），host 置空 |
| `fn count_big_cores() -> u32` | `system_cmds.rs:100` | ≥1.8GHz 计大核 |
| `fn read_mem_total_mb() -> u64` | `system_cmds.rs:115` | 读 `/proc/meminfo` 首行 |
| `fn tier_of(soc, big_cores, mem_mb) -> u32` | `system_cmds.rs:133` | 型号映射优先（骁龙 8 系 7 型 + 天玑 9300/9400 = 3；骁龙 7 系/天玑 8-9 系 = 2），空型号走启发式 |

#### monitor_loop.rs（858 行）

| 名称 | 位置 | 用途 |
|------|------|------|
| `static MONITOR: MonitorState` | `monitor_loop.rs:12` | `lazy_static` 全局监控状态 |
| `struct MonitorState` | `monitor_loop.rs:17-39` | 12 个原子/互斥字段（见下节） |
| `const WIFI_EVENT_DELAY_MS: u64 = 2500` | `monitor_loop.rs:44` | WiFi 事件触发检测的延迟（等 DHCP/路由就绪，兼风暴合并窗口） |
| `const WIFI_EVENT_DEBOUNCE_MS: u64 = 1000` | `monitor_loop.rs:47` | 事件去抖窗口 |
| `fn wifi_event_is_burst_start(last, now, debounce) -> bool` | `monitor_loop.rs:52` | 纯函数：首事件或窗口外为风暴起点 |
| `pub fn effective_interval_ms(base, idle, screen_on, wifi_connected) -> u64` | `monitor_loop.rs:61` | 巡检分档：亮屏 + WiFi 用 base（下限 5s），否则 idle（不足 base 时取 base） |
| `pub fn should_attempt_login(online, was_online, on_campus, auto_login_on_preparation, reconnect_count, max_reconnect, millis_since_last_attempt, cooldown_secs) -> bool` | `monitor_loop.rs:72` | 纯函数：在线/非校园网不试；非掉线场景需显式开关；重连上限；cooldown |
| `pub fn status_value() -> serde_json::Value` | `monitor_loop.rs:96` | 出站状态；把 `lastResult` 的 `online/message/serverAvailable/onCampusNetwork` 展平到顶层（`monitor_loop.rs:108-114`） |
| `pub fn is_running() -> bool` | `monitor_loop.rs:119` | 读 `running` |
| `fn epoch_ms() -> u64` | `monitor_loop.rs:123` | 当前毫秒时间戳 |
| `fn emit_login_log(app, message, log_type)` | `monitor_loop.rs:130` | emit `login-log` |
| `pub(crate) fn notify_system(app, enabled, title, body, mascot)` | `monitor_loop.rs:139` | 系统通知，`mascot` 为 drawable 资源名（大图失败降级纯文本，`monitor_loop.rs:153-155`） |
| `#[tauri::command] start_background_check` | `monitor_loop.rs:160` | 刷新 `desired_interval_ms`/`idle_interval_ms`；已跑直接返回；否则起前台服务（失败即 Err 不留假运行态）→ `running=true` → 重置 `notified_online` → 起 WiFi 监听 → spawn `monitor_tick_loop` |
| `#[tauri::command] stop_background_check` | `monitor_loop.rs:197` | `running=false` → 注销 WiFi 监听 → 停插件服务 → emit 日志 |
| `static WIFI_WATCHER_CHANNEL_ID: AtomicU32` / `WIFI_WATCHER_ACTIVE: AtomicBool` | `monitor_loop.rs:211`、`:213` | mobile 专用监听句柄 |
| `pub(crate) fn start_wifi_watcher(app)` | `monitor_loop.rs:218` | 注册 `tauri::ipc::Channel` 回调 → 插件 `start_wifi_watcher`；失败仅记日志（退化纯周期检测） |
| `pub(crate) fn stop_wifi_watcher(app)` | `monitor_loop.rs:240` | `removeListener` + `stopWifiWatcher` |
| `fn handle_wifi_event(app, body)` | `monitor_loop.rs:257` | 去抖 → 记录 `wifi_event_ms` → 延迟 2.5s → 时间戳未变才 `run_check_once`（风暴内最后安排者生效） |
| `#[tauri::command] trigger_background_check` | `monitor_loop.rs:294` | 立即单次检测 |
| `#[tauri::command] get_background_status` | `monitor_loop.rs:300` | `status_value()` |
| `#[tauri::command] get_boot_autostart` | `monitor_loop.rs:306` | mobile 插件 / host `Ok(false)` |
| `#[tauri::command] set_boot_autostart` | `monitor_loop.rs:323` | 插件组件启停 + `config_io_lock` 下落盘 + 刷内存态 |
| `#[tauri::command] get_notification_enabled` | `monitor_loop.rs:349` | 读配置 |
| `#[tauri::command] set_notification_enabled` | `monitor_loop.rs:354` | 落盘 + 刷内存态 |
| `pub fn run_startup_tasks(app)` | `monitor_loop.rs:381` | 启动恢复：500ms 就绪窗口 → 读配置 → **并行 spawn** 三条链（后台检测 / 质量首测或定时测试，`monitor_loop.rs:394-406`）+ 更新检查循环（`monitor_loop.rs:408`）+ 启动自动登录（`monitor_loop.rs:409-417`） |
| `async fn probe_with_retry(settings)` | `monitor_loop.rs:424` | 未确认校园网时 3s 后重试一次，仍失败返回最后一次结果 |
| `fn emit_auto_login_result(app, success, message)` | `monitor_loop.rs:444` | emit `auto-login-result`（应用内 toast；系统通知只留失败场景） |
| `async fn auto_login_on_start(app, settings)` | `monitor_loop.rs:454` | 无凭据返回；`ensure_wifi_bound` → 探测 + 缓存源 IP → 非校园网跳过 → `run_login` → 成功预置 `was_online=true` 并清保护期（`monitor_loop.rs:481-482`） |
| `struct ProbeWindowGuard` + `Drop` | `monitor_loop.rs:500`、`:503` | 探针窗口锁 guard（mobile），Drop 必释放 |
| `fn power_state(app) -> (bool, bool)` | `monitor_loop.rs:514`（mobile）/ `:522`（host 恒 `(true,true)`） | `(屏幕交互中, WiFi 连接中)`，查询失败按保守值 |
| `async fn monitor_tick_loop(app, interval_ms)` | `monitor_loop.rs:526` | tick 循环：间隔热更新（`monitor_loop.rs:538-543`）、分档跳拍（`:546-552`）、`run_check_once` |
| `async fn portal_probe_on_little_cores(ip)` | `monitor_loop.rs:563` | 裸线程 `portal-probe` + `Handle::enter()`（`:573`、`:577`）+ 绑小核（`:578`）+ `catch_unwind`（`:582-585`）；线程创建失败降级 `spawn_blocking`（`:593-597`） |
| `pub async fn run_check_once(app)` | `monitor_loop.rs:603` | 单次检测五步（静默期门控 → 校园网判定 → Portal 探测与三态消费 → 自动重登 → emit），详见 Data Flow |

#### battery_cmds.rs（81 行）

| 名称 | 位置 | 用途 |
|------|------|------|
| `struct BatteryOptimizationInfo` | `battery_cmds.rs:10-17` | `{ignoring, brand, hasVendorTarget}`（camelCase） |
| `#[tauri::command] get_battery_optimization_info` | `battery_cmds.rs:20` | 调插件 `get_battery_optimization_info`，取 `ignoring/brand/hasVendorTarget`；host 返回 Err（`:39`） |
| `#[tauri::command] request_ignore_battery_optimizations` | `battery_cmds.rs:46` | 调插件，返回 `opened`（前端需延迟重查刷新状态） |
| `#[tauri::command] open_vendor_battery_settings` | `battery_cmds.rs:65` | 透传插件 `{path,target,tried}` |

#### quality_cmds.rs（73 行）

| 名称 | 位置 | 用途 |
|------|------|------|
| `static LATENCY_RUNNING: AtomicBool` | `quality_cmds.rs:12` | 定时测试运行标志 |
| `async fn run_quality_once(app) -> NetworkQualityResult` | `quality_cmds.rs:15` | 读配置 + 缓存源 IP → 调桌面 `network::quality::check_network_quality_async("wlan0", ...)`（`quality_cmds.rs:27-36`） |
| `#[tauri::command] check_network_quality` | `quality_cmds.rs:40` | 单次检测 |
| `#[tauri::command] start_latency_test` | `quality_cmds.rs:45` | `swap(true)` 幂等 → spawn `latency_loop` |
| `#[tauri::command] stop_latency_test` | `quality_cmds.rs:56` | 置 false |
| `async fn latency_loop(app, settings)` | `quality_cmds.rs:61` | 间隔 `latency_test_interval.max(10_000)`（`quality_cmds.rs:63`）；每拍 `run_quality_once`（内部经 EventBus emit `network-quality-result`） |

#### update_cmds.rs（481 行）

| 名称 | 位置 | 用途 |
|------|------|------|
| `static UPDATE_LOOP_RUNNING: AtomicBool` | `update_cmds.rs:14` | 24h 循环单例 |
| `static UPDATE_CHECKSUM: Mutex<Option<String>>` | `update_cmds.rs:17` | 最近一次检查拿到的 SHA256（下载后比对） |
| `const VERSION_FILE: &str` | `update_cmds.rs:20` | GitHub raw main 分支 version.json |
| `const VERSION_MIRRORS: &[&str]` | `update_cmds.rs:22-26` | ghfast.top / gh-proxy.com / ghproxy.net 三个镜像 |
| `const DOWNLOAD_ALLOWED_HOSTS: &[&str]` | `update_cmds.rs:29-38` | 8 个下载域名白名单（防 SSRF） |
| `const MAX_DOWNLOAD_BYTES: u64 = 500 * 1024 * 1024` | `update_cmds.rs:40` | 下载上限 |
| `struct ReleaseAsset` | `update_cmds.rs:42-47` | `{name, url, size}` |
| `struct UpdateInfo` | `update_cmds.rs:49-57` | `{hasUpdate, latestVersion, releaseNotes, assets, sha256Checksum?}` |
| `struct DownloadProgress` | `update_cmds.rs:59-65` | `{downloaded, total, speed, percent}` |
| `pub fn has_newer_version(current, latest) -> bool` | `update_cmds.rs:68` | 去 `v` 前缀逐段数值比较（桌面 `compare_versions` 同语义） |
| `fn http_client()` | `update_cmds.rs:90` | rustls；`connect_timeout=5s` + `timeout=30s`（不设连接超时会吃满总超时） |
| `fn allowed_url(url)` | `update_cmds.rs:101` | 抠 host 并查白名单 |
| `#[tauri::command] check_update` | `update_cmds.rs:126` | 包装 `check_update_inner` |
| `fn version_urls(mirror_first) -> Vec<&'static str>` | `update_cmds.rs:130` | 按渠道排序 4 源 |
| `struct VersionFile` | `update_cmds.rs:142-148` | **真身形状**：snake_case `version`/`notes`（不能用 `UpdateInfo` 反序列化） |
| `async fn fetch_apk_assets(client, latest) -> (Vec<ReleaseAsset>, Option<String>)` | `update_cmds.rs:153` | GitHub API release 取 `.apk` 资产与 `digest` 的 `sha256:` 值；10s 超时、失败返回空 |
| `async fn check_update_inner(app)` | `update_cmds.rs:198` | 按 `update_source` 排序逐源尝试 → 解析 `VersionFile` → 比较版本 → 拉资产 → 写 `UPDATE_CHECKSUM` |
| `pub fn start_update_check_loop(app)` | `update_cmds.rs:242` | 启动后 5s 首查，之后每 24h；有新版 emit `update-available` + 系统通知 |
| `#[tauri::command] get_mirror_urls` | `update_cmds.rs:271` | 返回 GitHub + 三镜像候选 |
| `static DOWNLOAD_RUNNING: AtomicBool` | `update_cmds.rs:284` | 下载互斥 |
| `#[tauri::command] download_update` | `update_cmds.rs:287` | 互斥包装 + 全路径复位标志 |
| `async fn download_update_inner(app, url)` | `update_cmds.rs:296` | 白名单 → 流式写盘 → 200ms 节流 emit `update-download-progress` → SHA256 比对（失败删文件） |
| `async fn verify_file_sha256(path, expected) -> Result<bool, String>` | `update_cmds.rs:376` | 流式 64KiB 分块；非 64 位 hex 视为未提供返回 `true` |
| `#[tauri::command] install_update` | `update_cmds.rs:405` | `canonicalize` 限定 `app_data_dir/update/` 内 → 插件 `install_apk` |

## 结构体与字段

### `AndroidState`（`android_state.rs:8-13`）

| 字段 | 类型 | 含义 |
|------|------|------|
| `cached_source_ip` | `Mutex<Option<Ipv4Addr>>` | 检测阶段选出的 wlan0 源 IP；登录/注销/Portal/质量/自助服务共用 |
| `config` | `Mutex<Option<Settings>>` | 明文配置内存态（含明文密码，仅内存，不落盘明文） |

### `Settings`（`config_state.rs:15-72`，35 字段，`#[serde(rename_all = "camelCase", default)]`）

| 字段（Rust） | 类型 | 含义 / 默认值 |
|--------------|------|---------------|
| `user` | `String` | 学号（默认空） |
| `password` | `String` | 登录密码（内存明文，落盘为密文） |
| `self_password` | `String` | 自助服务系统密码（同上） |
| `self_hello_enabled` | `bool` | 生物识别验证门总开关（默认 `true`） |
| `self_reverify_each_action` | `bool` | 每动作重验证（默认 `false`） |
| `allow_2d_face_verify` | `bool` | 2D 人脸回退（低安全，默认 `false`，注释见 `config_state.rs:22-23`） |
| `operator` | `String` | 运营商后缀（如 `@cmcc`） |
| `auto_login_on_start` | `bool` | 启动自动登录（默认 `true`） |
| `enable_background_check` | `bool` | 后台检测总开关（默认 `true`） |
| `background_check_interval` | `u64` | 基础巡检间隔 ms（默认 `60_000`，2026-09-09 由 15s 上调） |
| `background_check_idle_interval` | `u64` | 闲时巡检间隔 ms（默认 `300_000`，蜂窝/灭屏生效，2026-09-13 新增） |
| `auto_login_on_preparation` | `bool` | 启动即离线场景的自动登录（默认 `true`） |
| `max_disconnect_reconnect` | `u32` | 单在线周期重连上限（默认 `3`） |
| `auto_login_cooldown_secs` | `u64` | 重登冷却秒（默认 `60`） |
| `theme_mode` | `String` | 主题（默认 `"dark"`） |
| `enable_notification` | `bool` | 通知开关（默认 `true`） |
| `custom_theme_color` | `String` | 自定义主题色（默认 `"#6366f1"`） |
| `default_panel` | `String` | 默认面板（默认 `"dashboard"`） |
| `active_account` | `String` | 当前账号名（默认空） |
| `enable_boot_autostart` | `bool` | 开机自启（默认 `false`，不属登录自动化） |
| `enable_latency_test` | `bool` | 定时质量测试（默认 `false`） |
| `latency_test_interval` | `u64` | 定时测试间隔 ms（默认 `60_000`，运行时下限 10s） |
| `enable_network_quality` | `bool` | 启动即跑质量检测（默认 `false`，2026-09-12 关闭省电） |
| `skip_ttfb_in_latency` | `bool` | 跳过 TTFB（默认 `true`） |
| `skip_content_in_latency` | `bool` | 跳过内容下载（默认 `true`） |
| `portal_url` | `String` | Portal 地址（默认 `http://10.1.99.100`） |
| `fixed_gateway` | `String` | 固定网关（默认 `10.2.127.254`） |
| `required_network_name` | `String` | 要求的 SSID（默认 `"i-wxxy"`，安卓取不到 SSID） |
| `enable_network_name_check` | `bool` | SSID 校验开关（默认 `true`） |
| `campus_gateway` | `String` | 校园网关（默认 `10.2.127.254`） |
| `campus_check_start_minutes` | `u16` | 检测时段起点（当日分钟，`0`=禁用，默认 `460`=07:40） |
| `campus_check_end_minutes` | `u16` | 检测时段终点（`0`/≤起点 = 不限制，默认 `0`） |
| `update_source` | `String` | 更新渠道 `"mirror"`/`"github"`（默认 `"mirror"`） |
| `log_retention_days` | `u32` | 日志保留天数（默认 `7`） |
| `config_schema_version` | `u32` | 配置结构版本（新装 `4`，旧文件缺省反序列化为 `0` 触发迁移） |

### `CryptoBridge`（`config_state.rs:127-130`）

| 字段 | 类型 | 含义 |
|------|------|------|
| `encrypt` | `Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>` | 明文 → 密文（真机为 Keystore AES-GCM，host 测试为 base64 假桥） |
| `decrypt` | `Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>` | 密文 → 明文 |

### `EncodedSettings`（`config_state.rs:168-172`，落盘形态）

| 字段 | 类型 | 含义 |
|------|------|------|
| `settings` | `Settings` | 其余配置（其内 `password`/`self_password` 恒为空串，见 `config_state.rs:214`） |
| `password_cipher` | `String` | 登录密码密文（camelCase `passwordCipher`） |
| `self_password_cipher` | `String` | 自助密码密文（camelCase `selfPasswordCipher`） |

### `LoginHistoryEntry`（`login_history.rs:12-21`）

| 字段 | 类型 | 含义 |
|------|------|------|
| `time` | `String` | `%Y-%m-%d %H:%M:%S` 本地时间 |
| `success` | `bool` | 成功与否 |
| `message` | `String` | 协议返回消息 |
| `adapter` | `String` | 恒 `"wlan0"`（安卓无多适配器，对齐桌面契约） |
| `user` | `String` | 学号 |
| `login_type` | `String` | 序列化为 `"type"`：`"manual"` / `"auto"` |

### `CampusProbe`（`campus_detect.rs:48-53`）

| 字段 | 类型 | 含义 |
|------|------|------|
| `source` | `Option<Ipv4Addr>` | 选中的源 IP |
| `on_campus_by_subnet` | `bool` | `/18` 子网命中 |
| `portal_ok` | `bool` | Portal TCP 可达 |
| `on_campus` | `bool` | 综合判定（子网 ∨ 网关 TCP ∨ Portal TCP） |

### `CommandResult`（`self_service_cmds.rs:7-14`）

| 字段 | 类型 | 含义 |
|------|------|------|
| `success` | `bool` | 成功标志 |
| `message` | `Option<String>` | 消息（`None` 不序列化） |
| `data` | `Option<serde_json::Value>` | 业务数据（同上） |

### `AccountResult`（`account_cmds.rs:25-35`）

| 字段 | 类型 | 含义 |
|------|------|------|
| `success` | `bool` | 成功标志 |
| `message` | `Option<String>` | 错误消息 |
| `active_account` | `Option<String>` | 变更后的当前账号 |
| `config` | `Option<serde_json::Value>` | 变更后的配置（已掩码） |

### `MonitorState`（`monitor_loop.rs:17-39`）

| 字段 | 类型 | 含义 |
|------|------|------|
| `running` | `AtomicBool` | 循环运行标志 |
| `check_count` | `AtomicU64` | 累计检测次数 |
| `consecutive_failures` | `AtomicU32` | 连续登录失败（≥5 熔断本会话自动重登） |
| `reconnect_count` | `AtomicU32` | 本在线周期重连次数（回在线清零） |
| `last_login_attempt_ms` | `AtomicU64` | 上次登录尝试时间（cooldown 基准） |
| `was_online` | `AtomicBool` | 上一拍在线状态（三态护栏的记忆位） |
| `last_result` | `Mutex<Option<serde_json::Value>>` | 最近一次检测 payload |
| `desired_interval_ms` | `AtomicU64` | 目标基础间隔（`start_background_check` 刷新，循环热更新） |
| `idle_interval_ms` | `AtomicU64` | 闲时间隔（从配置刷新） |
| `logout_protected_until_ms` | `AtomicU64` | 注销保护期截止（epoch ms） |
| `wifi_event_ms` | `AtomicU64` | 最近 WiFi 事件时间（去抖与“最后事件生效”） |
| `notified_online` | `AtomicU8` | 常驻通知已展示状态（`0` 未展示 / `1` 在线 / `2` 未连接），仅翻转时重建通知 |

### `BatteryOptimizationInfo`（`battery_cmds.rs:10-17`）

| 字段 | 类型 | 含义 |
|------|------|------|
| `ignoring` | `bool` | 已在“不优化电池”白名单 |
| `brand` | `String` | `Build.BRAND` 小写（前端只显示对应厂商项） |
| `has_vendor_target` | `bool` | 该品牌是否有专用跳转页 |

### `ReleaseAsset` / `UpdateInfo` / `DownloadProgress` / `VersionFile`（update_cmds.rs）

| 结构体 | 位置 | 字段 |
|--------|------|------|
| `ReleaseAsset` | `update_cmds.rs:42-47` | `name: String`、`url: String`、`size: u64` |
| `UpdateInfo` | `update_cmds.rs:49-57` | `has_update: bool`（`hasUpdate`）、`latest_version: String`、`release_notes: String`、`assets: Vec<ReleaseAsset>`、`sha256_checksum: Option<String>` |
| `DownloadProgress` | `update_cmds.rs:59-65` | `downloaded: u64`、`total: u64`、`speed: u64`（字节/秒）、`percent: f64` |
| `VersionFile` | `update_cmds.rs:142-148` | `version: String`（`#[serde(default)]`）、`notes: String`（同）——version.json 真实形状 |

## Data Flow

### 1. 复用桌面协议核心的方式（path 依赖）

```text
android/src-tauri/Cargo.toml:34   campus-login = { path = "../../tauri-app/src-tauri" }
        ↓ 编译期直接链接桌面 crate（lib 名 campus_login_lib）
tauri-app/src-tauri/src/lib.rs:1-8    跨平台可见：account / auth / config / infra / network / platform / self_service
tauri-app/src-tauri/src/lib.rs:11-20  #[cfg(desktop)] 门控：app / commands / helper / monitor / update（安卓不可见）
```

安卓代码实际调用的桌面实现（全部是 `campus_login_lib::` 前缀）：

| 安卓调用点 | 桌面实现位置 |
|-----------|-------------|
| `protocol_cmds.rs:73` `auth::protocol::do_login_with_retry` | `tauri-app/src-tauri/src/auth/protocol.rs:155` |
| `protocol_cmds.rs:109` `auth::protocol::do_logout_with_retry` | `tauri-app/src-tauri/src/auth/protocol.rs:377` |
| `protocol_cmds.rs:152` `auth::portal::check_portal_full` | `tauri-app/src-tauri/src/auth/portal.rs:95`（返回 `PortalStatus`，`auth/portal.rs:85-93`） |
| `protocol_cmds.rs:235` `network::client::clear_client_pool` | `tauri-app/src-tauri/src/network/client.rs:137` |
| `protocol_cmds.rs:239` `log_info!` / `:265` `log_warn!` | `tauri-app/src-tauri/src/infra/logger.rs:355`、`:362` |
| `campus_detect.rs:73` `network::is_same_subnet_18` | `tauri-app/src-tauri/src/network/subnet.rs:174` |
| `quality_cmds.rs:27` `network::quality::check_network_quality_async` | `tauri-app/src-tauri/src/network/quality.rs:451`（`NetworkQualityResult` 定义于 `network/quality.rs:9-18`） |
| `self_service_cmds.rs:107/128/160/208/236/263` `self_service::{bind_operator,query_bind_status,query_dashboard,query_online_log,offline_session,reveal_credential}` | `tauri-app/src-tauri/src/self_service/mod.rs:148`、`:207`、`:377`、`:391`、`:431`、`:223`（`BindParams` `:64`、`OperatorBinding` `:198`） |
| `system_cmds.rs:33/41/48/53/58/63` `infra::logger::{read_recent_logs,clear_logs,get/set_log_retention_days,get/set_debug_mode}` | `tauri-app/src-tauri/src/infra/logger.rs:399`、`:479`、`:22`、`:18`、`:343`、`:336` |
| `lib.rs:49` `infra::logger::init_logger` | `tauri-app/src-tauri/src/infra/logger.rs:65` |

**注意**：桌面侧 `commands/login.rs`、`config/persist.rs`（`append_login_history`，`config/persist.rs:99`）、`platform/identity.rs`（`identity.rs:31/34/39`）属门控或跨平台模块，安卓**不直接调用桌面命令层**，而是复刻同语义的安卓实现（`login_history.rs`、`identity_gate.rs`）——这是"同构"而非"复用"，改桌面这两处时安卓需同步检查语义。

### 2. 安卓独有实现（桌面无对应）

| 能力 | 实现位置 | 关键点 |
|------|----------|--------|
| Keystore 加密落盘 | `config_state.rs:192-223` + keystore 插件 | 桌面用 DPAPI，安卓用 AndroidKeyStore AES-GCM；加密失败 Err、解密失败置空 |
| 进程绑 WiFi | `protocol_cmds.rs:220-279` + network-bind 插件 | `bindProcessToNetwork` 进程级 fwmark；`already_bound` 时不清客户端池 |
| 前台服务保活 | `monitor_loop.rs:160-207` + foreground-service 插件 | 常驻通知；WifiLock/WakeLock 改为探针窗口内按需持有（`begin_probe_window`/`end_probe_window`，见下行），nudge 唤醒锁按关注字段翻转去重 |
| 探针窗口锁 | `monitor_loop.rs:499-508`、`:630-635` | `ProbeWindowGuard` Drop 必释放 |
| 绑小核 | `cpu_affinity.rs:34` + `monitor_loop.rs:578` | 只绑专用短命线程，不绑共享 worker |
| 验证门 TTL | `identity_gate.rs:6-33` | 进程内 `AtomicU64`，600s，时钟回拨视为过期 |
| 监控状态机 | `monitor_loop.rs:17-39`、`:603-775` | 三态消费、双闸、分档巡检 |
| WiFi 事件驱动检测 | `monitor_loop.rs:218-291` | 去抖 + 2.5s 延迟 + 最后事件生效 |
| 电池优化白名单 | `battery_cmds.rs:20-81` | 平台专属 API |
| 设备分档 | `system_cmds.rs:77-160` | `ro.soc.model` + 大核/内存启发式 |
| APK 安装 | `update_cmds.rs:405-422` | FileProvider content URI（桌面用 exe/msi） |

### 3. 关键调用链

**启动（`lib.rs:51` → `monitor_loop.rs:381`）**

```text
tauri setup → init_logger(app_data_dir/logs)
            → run_startup_tasks: sleep 500ms → current_settings
              ├─ enable_background_check   → spawn start_background_check
              │    └─ 插件 start_monitor(前台服务) → MONITOR.running=true
              │       → start_wifi_watcher → spawn monitor_tick_loop
              ├─ enable_network_quality    → spawn start_latency_test 或 check_network_quality
              ├─ start_update_check_loop   → 5s 首查 → 每 24h
              └─ auto_login_on_start       → ensure_wifi_bound → probe_with_retry
                                           → cache_source_ip → run_login → emit auto-login-result
```

**单次检测（`monitor_loop.rs:603`）**

```text
check_count += 1 → current_settings
 ⓪ 检测静默期门控（campus_check_start/end_minutes，monitor_loop.rs:617-626）整拍 return
 探针窗口 guard（mobile，monitor_loop.rs:630-635）
 → ensure_wifi_bound（monitor_loop.rs:639）
 ① probe_campus（源 IP 缓存刷新，monitor_loop.rs:643-653）
 ② portal_probe_on_little_cores（裸线程 enter + 绑小核，monitor_loop.rs:657）
 ③ 三态消费：error_kind=None 才采用 s.online，否则沿用 prev_online（monitor_loop.rs:668-672）
    在线 → 清 reconnect/consecutive_failures/保护期（monitor_loop.rs:675-679）
    was_online→offline 且 on_campus → notify_system("校园网连接掉线")（monitor_loop.rs:682-684）
 ④ should_attempt_login ∧ !logout_protected ∧ !failures_capped ∧ 凭据非空
    → last_login_attempt_ms / reconnect_count++ → run_login → 历史落盘 → emit
      （monitor_loop.rs:690-739）
 ⑤ 组装 payload → MONITOR.last_result → emit "background-check-result"（monitor_loop.rs:743-760）
    → notified_online 翻转时插件 update_notification（monitor_loop.rs:764-774）
```

**登录（`protocol_cmds.rs:10`）**

```text
invoke do_login{user,password,operator,adapter}
 → ensure_wifi_bound（绑定成功后清客户端池 + accept_campus_wifi）
 → 凭据回退 current_settings
 → run_login → spawn_blocking(auth::protocol::do_login_with_retry(user,password,operator,adapter_ip,3,&is_quitting))
 → login_history::append(type="manual")
 → 成功：consecutive_failures=0、logout_protected_until_ms=0
```

**配置读写（`config_state.rs:297` / `:312`）**

```text
save_config{config,clearPassword?,clearSelfPassword?}
 → load_from（含迁移）→ resolve_password_field 合并两密码
 → save_to → save_file：encrypt(password) / encrypt(selfPassword)（失败即 Err 不落明文）
 → Settings 内密码清空 + 密文位 → tmp + rename
 → AndroidState.config 内存态刷新
出站：masked_for_display（password/selfPassword 非空 → "***"）
```

**监控状态迁移（`monitor_loop.rs`）**

```text
未验证 → identity_verified_recently=false → reveal/bind/offline 被拦
         verify_biometric_identity（self_service_cmds.rs:63）→ 写时间戳 → TTL 600s 内放行
在线 ←(Portal 确定判定)→ 掉线：清 reconnect_count；was_online→false
掉线 → 自动重连：reconnect_count++（达 max_disconnect_reconnect 通知上限）
     → 连续失败 ≥5 熔断；手动登录成功（protocol_cmds.rs:51-55）清零熔断与保护期
手动注销 → logout_protected_until_ms = now+60s（protocol_cmds.rs:127-133）
```

**事件出口（前端监听）**：`login-log`（`monitor_loop.rs:133`）、`auto-login-result`（`:446`）、`background-check-result`（`:760`）、`network-quality-result`（由桌面 `network::quality` 内部 emit）、`update-available`（`update_cmds.rs:251`）、`update-download-progress`（`:355`）。

## Connections

- [[desktop-auth]]：`do_login`/`do_logout`/`check_portal_status` 三个命令的实现在桌面 `auth/protocol.rs`、`auth/portal.rs`；安卓仅包装。
- [[desktop-config]]：`Settings` 为桌面 `config::Config` 的可适用子集；`masked_for_display`、`resolve_password_field`、schema 迁移语义同构。
- [[desktop-account-selfservice]]：多账号与自助服务六命令的桌面同构物。
- [[desktop-network-core]]：`is_same_subnet_18`、`clear_client_pool` 的复用点。
- [[desktop-network-quality]]：`check_network_quality_async` 与 `NetworkQualityResult` 的复用点。
- [[desktop-monitor]]：`should_attempt_login`、三态消费、注销保护期、熔断等状态机语义的桌面参考。
- [[desktop-commands]]：桌面 48 个同名命令的注册与实现对照。
- [[desktop-infra]]：`infra::logger`（日志读写、debug mode、retention）。
- [[desktop-platform]]：`platform/identity.rs` 是 `identity_gate.rs` 的同构物（DPAPI 侧对应 Keystore）。
- [[desktop-app-lifecycle]]：桌面启动编排（`watcher::run_startup_tasks`）是 `run_startup_tasks` 的对齐目标。
- [[desktop-helper-update]]：更新检查/下载/安装的桌面版本（exe/msi vs APK）。
- [[android-plugins]]：三个手写插件的 Rust 壳与 Kotlin 实现、权限文件。
- [[android-frontend]]：调用本模块 48 个命令的安卓前端。

## Known Issues

1. **配置保存不联动循环起停**：`save_config`（`config_state.rs:312-338`）只落盘与刷内存态，不根据 `enable_background_check`/`enable_latency_test` 调用 `start/stop_background_check`、`start/stop_latency_test`。用户关掉开关后循环是否停止完全依赖前端另行发起命令（`android/frontend/src/hooks/tauriApi.ts:230-240` 提供了对应封装，但后端无兜底），后端层面存在"配置关、循环仍在跑"的窗口。
2. **`config_io_lock` 覆盖不全**：锁在 `account_cmds.rs:17/21` 定义，`switch_account`/`save_current_as_account`/`delete_account`（`account_cmds.rs:127/156/179`）、`set_boot_autostart`（`monitor_loop.rs:331`）、`set_notification_enabled`（`monitor_loop.rs:355`）有取锁，但 **`save_config`（`config_state.rs:312`）完全没取锁**——正是注释里点名的并发读改写竞态对象之一。
3. **`detect_campus` 已成死命令**：`campus_detect.rs:122` 注释称"前端旧 UI 仍在用"，但安卓前端全仓（`android/frontend/src/`）已无 `detect_campus`、`accept_wifi_network`、`ping_test` 的调用点（grep 仅命中 `set_boot_autostart`），三个命令与注释均属陈旧残留。
4. **`UPDATE_CHECKSUM` 与下载目标未绑定**：校验值在 `check_update_inner`（`update_cmds.rs:221-223`）写入全局静态，`download_update`（`update_cmds.rs:364-370`）读取时**不校验该值属于哪个版本/资产**；前端若先手动 `check_update` 再下载其他 URL（或反过来先用旧校验值下载新包），要么误杀要么放行；校验值缺失时 fail-open（跳过校验）。
5. **下载校验 fail-open**：`verify_file_sha256`（`update_cmds.rs:384-386`）在期望值不是 64 位 hex 时返回 `true`（视为未提供）；`update_cmds.rs:365` 的 `if let Some(expected)` 在 `None` 时直接跳过校验。发布流程未补 `digest` 时下载链路无完整性保护。
6. **`install_update` 的时间窗边界**：`update_cmds.rs:414` 在 `update` 目录 canonicalize 失败时回退未规范化路径，此时 `starts_with` 校验基于非绝对规范化路径；目录不存在（从未下载过）属该分支的常规触发场景。
7. **`check_campus_status` 的 SSID 语义错位**：安卓恒返回 `currentSsid: ""`（`campus_detect.rs:170`），却把 `enableNetworkNameCheck` 与 `requiredNetworkName` 原样透传（`campus_detect.rs:172-173`）。若前端按"开关开 + SSID 必须匹配"消费，会得到与 `onCampusNetwork` 矛盾的结果；本项目前端当前是否规避需在 [[android-frontend]] 侧确认。
8. **`enable_network_name_check` 在安卓无效**：`probe_campus`（`campus_detect.rs:55-98`）完全未读取该字段，探测链只有 /18 子网 + 两个 TCP 兜底；配置项在安卓为死配置。
9. **Portal 探测兜底可误判**：`campus_detect.rs:85-90` 在子网未命中时用 `portal_reachable(campus_gateway, 80)` 兜底，若 `portal_url` 配成公网域名，家宽环境也可能连通（源码注释 `campus_detect.rs:79-84` 自述为已知边界）。
10. **`gateway_ok` 采用同端口语义**：网关 TCP 探测复用 `PORTAL_PORT = 80`（`campus_detect.rs:88`），网关不监听 80 的校园网环境下该兜底恒 false。
11. **写历史的同步 IO 在 async 上下文**：`do_login`（`protocol_cmds.rs:48`）与 `do_logout`（`protocol_cmds.rs:122`）、`run_check_once`（`monitor_loop.rs:721`）直接在 async 任务里调用 `login_history::append`（内部 `std::fs` 同步读写，`login_history.rs:73-74`）。文件小、有 `LOGIN_HISTORY_LOCK` 串行，影响有限但会短时占用 async 线程。
12. **`identity_gate` 用进程级静态**：`LAST_VERIFY_EPOCH_SECS`（`identity_gate.rs:6`）是全局静态而非 `AppHandle` 状态，验证门与"哪个 app 实例"无关；当前单实例架构下无问题，与桌面 `platform/identity.rs` 的实现选择一致。
13. **陈旧 `#[allow(dead_code)]` 与注释**：`config_state.rs:341` 标 `#[allow(dead_code)] // Task 2 协议命令面接线`，但 `current_settings` 已被全部命令使用；`monitor_loop.rs:16/71/118` 标注 `// Task 4 循环体接线`，而循环体早已接线（`monitor_loop.rs:697` 调用 `should_attempt_login`）。`android_state.rs:1` 与 `lib.rs:8` 注释称 `AndroidState` 含"监控循环句柄"，实际只有两个字段（`android_state.rs:10-12`）——监控状态在 `monitor_loop.rs:12` 的 `MONITOR` 静态里。
14. **`get_power_state` 查询失败按保守值**：`monitor_loop.rs:514-519` 失败回落 `(true, true)` 即"亮屏 + WiFi"，会持续按基础间隔巡检（源码注释说明这是刻意取舍：省电是优化、漏检是缺陷）。
15. **`start_latency_test` 与 `check_network_quality` 共用全局标志**：`LATENCY_RUNNING`（`quality_cmds.rs:12`）是进程级 `static`，`stop_latency_test`（`quality_cmds.rs:56`）无 `app` 参数——多 app 实例/多 webview 场景下无法区分调用方；当前单实例架构下可接受。
16. **前台服务启动失败已正确处理但顺序敏感**：`start_background_check`（`monitor_loop.rs:172-183`）先起 FGS 成功后才置 `running=true`，属刻意设计；但 `MONITOR.running` 与插件服务的状态在进程被杀后不同步恢复，只能靠 `run_startup_tasks`（`monitor_loop.rs:381`）重新对齐。
17. **host 环境多处命令返回 `Err`**：`battery_cmds.rs:39/59/79`、`protocol_cmds.rs:186/204` 在 `not(mobile)` 分支返回错误字符串；`CryptoBridge::from_app` 在 host 下加解密恒失败（`config_state.rs:158-159`），host 上跑测试必须注入假桥（`config_state.rs:367`）。
