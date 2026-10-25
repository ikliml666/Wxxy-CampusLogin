---
title: desktop-rust-support
type: module
source_files:
  - android/src-tauri/src/campus_detect.rs
  - android/src-tauri/src/self_service_cmds.rs
  - android/src-tauri/src/face_model_cmds.rs
  - android/src-tauri/src/login_history.rs
  - android/src-tauri/src/quality_history.rs
  - android/src-tauri/src/battery_cmds.rs
  - android/src-tauri/src/cpu_affinity.rs
  - android/src-tauri/src/system_cmds.rs
  - android/src-tauri/src/identity_gate.rs
  - android/src-tauri/src/android_state.rs
tags: [android, campus-login, self-service, face-model, battery, history, identity-gate]
---

## Overview

本模块涵盖 Android 端 Tauri 应用的 Rust 侧核心逻辑，是桌面端功能在移动端的适配与扩展。包含校园网探测（`campus_detect`）、自助服务命令（`self_service_cmds`）、人脸识别模型按需下载（`face_model_cmds`）、登录与网络质量历史持久化（`login_history` / `quality_history`）、电池优化与通知设置跳转（`battery_cmds`）、CPU 大小核绑核（`cpu_affinity`）、系统聚合初始化与日志命令（`system_cmds`）、生物识别验证门（`identity_gate`）以及全局状态（`android_state`）。由于 Android 平台限制（SELinux 禁止 ICMP 原始 socket、无 root），网络探测改用 TCP 可达性与 SSID 判定，并针对厂商省电策略提供降级链支持。

## Key Components

### `campus_detect` (校园网探测)
**文件**: `android/src-tauri/src/campus_detect.rs`

核心功能：枚举网卡、判定校园网连通性、处理 WiFi SSID 与权限。

*   **常量**
    *   `PORTAL_PORT`: `u16 = 80` (第 9 行)
    *   `TCP_TIMEOUT`: `Duration::from_secs(3)` (第 10 行)

*   **Structs**
    *   `CampusProbe` (第 56-63 行): 探测结果结构体
        *   `source: Option<Ipv4Addr>` - 源 IP
        *   `portal_ok: bool` - Portal TCP 可达性
        *   `on_campus: bool` - 是否在校内
        *   `wifi_off: bool` - WiFi 是否关闭（短路标志）

*   **Functions**
    *   `pub fn pick_campus_source_ip(interfaces: &[(String, IpAddr)]) -> Option<Ipv4Addr>` (第 15-37 行) 从网卡列表筛选 IPv4，排除蜂窝接口 (`rmnet*`/`ccmni*`)、link-local、回环、未指定；优先 `wlan0`。
    *   `pub async fn portal_reachable(host: &str, port: u16, timeout: Duration) -> bool` (第 44-53 行) 替代 ICMP Ping，使用 `bound_socket::tcp_connect_bounded` 做 TCP 连通测试，支持 VPN 接管下物理网卡直连。
    *   `pub async fn probe_campus(campus_gateway: &str, portal_url: &str) -> Result<CampusProbe, String>` (第 65-125 行) 主探测函数；含 WiFi 未连接短路 (第 85-93 行)；三层判定：子网 (/18) → 网关 TCP → Portal TCP（并行/串行见第 109-116 行）。
    *   `pub fn campus_status_message(wifi_off: bool, on_campus: bool) -> &'static str` (第 129-137 行) 生成状态文案（`"WiFi 未连接"` / `"已连接校园网"` / `"未连接校园网"`）。
    *   `pub fn portal_host_of(url: &str) -> String` (第 140-149 行) 提取 URL 中的 Host 部分。
    *   `pub fn ssid_matches(ssid: &str, required: &str) -> bool` (第 153-157 行) SSID 匹配，默认 `i-wxxy` 特判历史同源 `iwxxy-2`/`iwxxy-3`。
    *   `async fn current_wifi_ssid(app: &tauri::AppHandle) -> Option<String>` (第 163-178 行) 经 network-bind 插件 JNI 阻塞调用获取当前 SSID。
    *   `pub(crate) async fn probe_campus_with_ssid(app: &tauri::AppHandle, settings: &crate::config_state::Settings) -> (Result<CampusProbe, String>, Option<String>)` (第 185-200 行) SSID 感知探测版。
    *   `pub fn cache_source_ip(state: &tauri::State<'_, crate::android_state::AndroidState>, source: Option<Ipv4Addr>)` (第 203-210 行) 缓存源 IP 供登录绑定。
    *   `#[tauri::command] pub async fn check_campus_status(app: tauri::AppHandle, state: tauri::State<'_, crate::android_state::AndroidState>) -> Result<serde_json::Value, String>` (第 216-238 行) Tauri 命令，返回 JSON 状态。
    *   `#[tauri::command] pub fn request_wifi_ssid_permission(app: tauri::AppHandle) -> Result<serde_json::Value, String>` (第 244-249 行) 触发 `NEARBY_WIFI_DEVICES` 权限请求。

*   **Tests** (mod tests 第 251-321 行)
    *   `wlan0_campus_ip_被优先选中` (第 261-269 行)
    *   `蜂窝接口永不返回` (第 271-275 行)
    *   `link_local_地址被排除` (第 277-281 行)
    *   `无_wlan0_时取任意合法内网ipv4` (第 283-287 行)
    *   `portal_host_提取` (第 289-295 行)
    *   `campus_状态文案_wifi短路优先` (第 297-304 行)
    *   `ssid_匹配语义对齐桌面` (第 306-320 行)

### `self_service_cmds` (自助服务命令)
**文件**: `android/src-tauri/src/self_service_cmds.rs`

核心功能：包装 `campus_login_lib::self_service`，实现绑定、查询、注销、改密等自助服务，含生物识别验证门。

*   **Structs**
    *   `CommandResult` (第 10-17 行) `#[derive(Serialize, Clone)]`
        *   `success: bool`
        *   `message: Option<String>` (`skip_serializing_if = "Option::is_none"`)
        *   `data: Option<serde_json::Value>` (`skip_serializing_if = "Option::is_none"`)
        *   方法 `ok_msg(msg: &str) -> Self` (第 20-22 行)
        *   方法 `err(msg: &str) -> Self` (第 23-25 行)

*   **内部/私有函数**
    *   `fn resolve_self_password(incoming: &str, settings: &crate::config_state::Settings) -> Option<String>` (第 30-39 行) 密码回退逻辑（掩码/空回退至 `config.password`）。
    *   `pub(crate) fn ensure_identity_gate(settings: &crate::config_state::Settings) -> Option<String>` (第 42-51 行) 验证门：总开关关放行，否则查 TTL。
    *   `pub(crate) async fn load_settings(app: &tauri::AppHandle) -> Result<crate::config_state::Settings, String>` (第 53-55 行)
    *   `fn local_addr_of(state: &tauri::State<'_, crate::android_state::AndroidState>) -> Option<std::net::IpAddr>` (第 57-63 行) 取缓存源 IP。
    *   `fn is_iso_date(s: &str) -> bool` (第 196-208 行) 严格校验 `YYYY-MM-DD`。

*   **Commands**
    *   `#[tauri::command] pub async fn verify_biometric_identity(success: bool, method: String, _consent_message: Option<String>) -> Result<CommandResult, String>` (第 70-85 行) 仅 `success==true` 时记验证时间戳。
    *   `#[tauri::command] pub async fn bind_operator(...)` (第 88-132 行) 运营商绑定，需验证门。
    *   `#[tauri::command] pub async fn query_bind_status(...)` (第 135-166 行) 查询绑定状态，无验证门。
    *   `#[tauri::command] pub async fn query_self_dashboard(...)` (第 169-193 行) 查询仪表盘，无验证门。
    *   `#[tauri::command] pub async fn query_self_online_log(...)` (第 211-239 行) 查询在线日志，无验证门。
    *   `#[tauri::command] pub async fn self_offline_session(...)` (第 242-269 行) 注销会话，需验证门。
    *   `#[tauri::command] pub async fn change_self_password(...)` (第 276-312 行) 改密，需验证门，新密码 6-11 位 ASCII 字母数字。
    *   `#[tauri::command] pub async fn reveal_operator_credential(...)` (第 316-345 行) 揭示手机号/短信密码，强制验证门。

*   **Tests** (mod tests 第 347-391 行)
    *   `自助密码回退_掩码与空回退_新值直用` (第 359-367 行)
    *   `验证门_总开关关闭放行_开启走TTL` (第 369-382 行)
    *   `is_iso_date_严格校验` (第 384-391 行)

### `face_model_cmds` (面部模型管理)
**文件**: `android/src-tauri/src/face_model_cmds.rs`

核心功能：按需下载人脸识别模型，SHA256 校验，进度上报。

*   **Constants / Statics**
    *   `FACE_MODEL_BASE`: GitHub Raw URL (第 18 行)
    *   `FACE_MODEL_MIRRORS`: 镜像代理列表 (第 20-24 行)
    *   `FACE_MODEL_FILES`: 文件名与 SHA256 映射 (第 26-51 行)，共 6 文件。
    *   `static FACE_DL_RUNNING: AtomicBool` (第 54 行) 并发互斥。

*   **Structs**
    *   `FaceModelFileState` (第 57-63 行): `name: String`, `size: u64`, `ok: bool` (存在且哈希匹配)
    *   `FaceModelsState` (第 66-72 行): `ready: bool`, `dir: String`, `files: Vec<FaceModelFileState>`
    *   `FaceModelProgress` (第 75-81 行): `file: String`, `downloaded: u64`, `total: u64`, `percent: f64`

*   **Functions**
    *   `fn models_dir(app: &tauri::AppHandle) -> Result<PathBuf, String>` (第 86-93 行) 目录 `app_data_dir/files/face-models`。
    *   `async fn state_inner(app: &tauri::AppHandle) -> Result<FaceModelsState, String>` (第 96-118 行) 检查文件状态。
    *   `#[tauri::command] pub async fn face_models_state(app: tauri::AppHandle) -> Result<FaceModelsState, String>` (第 121-123 行)
    *   `#[tauri::command] pub async fn face_models_download(app: tauri::AppHandle) -> Result<FaceModelsState, String>` (第 126-133 行) 互斥防并发。
    *   `async fn download_models(app: &tauri::AppHandle) -> Result<FaceModelsState, String>` (第 135-184 行) 镜像降级、逐文件校验、原子写。
    *   `async fn download_one(client: &reqwest::Client, url: &str, dest: &Path, name: &str, app: &tauri::AppHandle) -> Result<(), String>` (第 187-243 行) 流式下载，每 200ms 发射 `face-models-download-progress` 事件。

*   **Tests** (mod tests 第 246-274 行)
    *   `mirror_url_keeps_prefix_form` (第 250-256 行)
    *   `sources_order_follows_mirror_first` (第 258-274 行)

### `login_history` (登录历史)
**文件**: `android/src-tauri/src/login_history.rs`

核心功能：持久化登录记录，头插法，上限 100，损坏备份。

*   **Constants / Statics**
    *   `static LOGIN_HISTORY_LOCK: Mutex<()>` (第 8 行)
    *   `pub const LOGIN_HISTORY_MAX: usize = 100` (第 10 行)

*   **Structs**
    *   `LoginHistoryEntry` (第 12-21 行):
        *   `time: String`
        *   `success: bool`
        *   `message: String`
        *   `adapter: String` (固定 `"wlan0"`)
        *   `user: String`
        *   `login_type: String` (序列化名 `"type"`)

*   **Functions**
    *   `fn history_path(dir: &Path) -> PathBuf` (第 23-25 行)
    *   `pub fn read(dir: &Path) -> Vec<LoginHistoryEntry>` (第 28-41 行) 损坏备份 `.corrupt-*.bak` 后重置。
    *   `fn chrono_epoch_millis() -> u128` (第 43-48 行)
    *   `pub fn append(dir: &Path, success: bool, message: &str, user: &str, login_type: &str) -> Result<(), String>` (第 52-77 行) 头插、截断、原子写。

*   **Tests** (mod tests 第 79-142 行)
    *   `头插法_最新在前` (第 90-101 行)
    *   `上限_100条截断` (第 103-114 行)
    *   `损坏文件_备份后重置` (第 116-130 行)
    *   `记录字段_序列化形状对齐桌面` (第 132-142 行)

### `quality_history` (网络质量历史)
**文件**: `android/src-tauri/src/quality_history.rs`

核心功能：持久化网络质量数据，结构双端一致。

*   **Constants / Statics**
    *   `static QUALITY_HISTORY_LOCK: Mutex<()>` (第 10 行)
    *   `pub const QUALITY_HISTORY_MAX: usize = 100` (第 12 行)

*   **Structs**
    *   `QualityHistoryEntry` (第 14-22 行):
        *   `timestamp: u64`
        *   `gateway_latency: Option<i64>` (负延迟落盘为 `null`)
        *   `external_latency: Option<i64>`
        *   `quality: String`

*   **Functions**
    *   `fn history_path(dir: &Path) -> PathBuf` (第 24-26 行)
    *   `pub fn read(dir: &Path) -> Vec<QualityHistoryEntry>` (第 29-42 行) 损坏备份。
    *   `fn epoch_millis() -> u128` (第 44-49 行)
    *   `pub fn append(dir: &Path, result: &NetworkQualityResult) -> Result<(), String>` (第 52-75 行) 追加并将 `-1` 延迟转 `None`。

*   **Tests** (mod tests 第 77-156 行)
    *   `头插法_最新在前` (第 101-114 行)
    *   `上限_100条截断` (第 116-127 行)
    *   `损坏文件_备份后重置` (第 129-143 行)
    *   `记录字段_序列化形状双端一致` (第 145-156 行)

### `battery_cmds` (电池与系统设置)
**文件**: `android/src-tauri/src/battery_cmds.rs`

核心功能：安卓专属电池优化白名单与通知设置跳转。

*   **Structs**
    *   `BatteryOptimizationInfo` (第 8-17 行): `ignoring: bool`, `brand: String`, `has_vendor_target: bool`

*   **Commands** (均 `#[cfg(mobile)]` 有效，否则返回 Err)
    *   `get_battery_optimization_info(app)` (第 20-41 行)
    *   `request_ignore_battery_optimizations(app)` (第 46-61 行)
    *   `open_vendor_battery_settings(app)` (第 65-81 行)
    *   `open_notification_settings(app)` (第 87-102 行)

### `cpu_affinity` (CPU 绑核)
**文件**: `android/src-tauri/src/cpu_affinity.rs`

核心功能：识别小核并绑核省功耗。

*   **Functions**
    *   `pub fn little_core_ids() -> Vec<usize>` (第 8-31 行) 解析 `/sys/devices/system/cpu/cpu*/cpufreq/cpuinfo_max_freq`，频率差 <30% 返回空。
    *   `pub fn pin_current_thread_to_little_cores() -> bool` (第 34-50 行) `sched_setaffinity` 绑核；非 Android 恒 `false`。

*   **Tests** (mod tests 第 52-68 行)
    *   `host_环境_恒不绑定` (第 57-61 行)
    *   `小核识别_同构拓扑返回空` (第 63-68 行)

### `system_cmds` (系统聚合命令)
**文件**: `android/src-tauri/src/system_cmds.rs`

核心功能：聚合初始化数据、日志管理。

*   **Commands**
    *   `get_init_data(app)` (第 6-29 行) 一次返回配置、账号、版本、日志设置等，补桌面字段默认值。
    *   `get_logs(app, lines)` (第 32-39 行)
    *   `clear_logs(app)` (第 42-46 行)
    *   `get_log_retention_days()` (第 49-51 行)
    *   `set_log_retention_days(days)` (第 54-56 行)
    *   `get_debug_mode()` (第 59-61 行)
    *   `set_debug_mode(enabled)` (第 64-66 行)

### `identity_gate` (生物识别验证门)
**文件**: `android/src-tauri/src/identity_gate.rs`

核心功能：进程内 TTL 时间戳管理。

*   **Constants / Statics**
    *   `static LAST_VERIFY_EPOCH_SECS: AtomicU64` (第 6 行)
    *   `pub const IDENTITY_VERIFY_TTL_SECS: u64 = 600` (第 8 行)

*   **Functions**
    *   `fn now_epoch_secs() -> u64` (第 10-15 行)
    *   `pub fn note_identity_verified()` (第 18-20 行) 写验证时间戳。
    *   `pub fn identity_verified_recently() -> bool` (第 23-33 行) TTL 检查，时钟回拨视为过期。
    *   `pub(crate) fn reset_for_tests()` (第 36-38 行)

*   **Tests** (mod tests 第 40-65 行)
    *   `从未验证_为假` (第 45-49 行)
    *   `TTL_内为真_外为假` (第 51-58 行)
    *   `时钟回拨_视为过期` (第 60-65 行)

### `android_state` (全局状态)
**文件**: `android/src-tauri/src/android_state.rs`

核心功能：维护 Android 端全局内存状态。

*   **Statics / Structs**
    *   `pub(crate) static WATCHER_FAILED: AtomicBool` (第 14 行) WiFi watcher 注册失败标志。
    *   `struct AndroidState` (第 16-21 行) `#[derive(Default)]`:
        *   `cached_source_ip: Mutex<Option<Ipv4Addr>>`
        *   `config: Mutex<Option<Settings>>`

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                    Tauri App Handle                          │
│  ┌─────────────────┐  ┌──────────────────┐  ┌──────────────┐ │
│  │ AndroidState    │  │ IdentityGate     │  │ ConfigState  │ │
│  │ (android_state) │  │ (identity_gate)  │  │ (config_state)│ │
│  └────────┬────────┘  └────────┬─────────┘  └──────┬───────┘ │
│           │                   │                    │         │
│  ┌────────▼───────────────────▼────────────────────▼───────┐ │
│  │            campus_detect & self_service_cmds            │ │
│  │  probe_campus_with_ssid ─► check_campus_status         │ │
│  │  bind/change/offline ──► ensure_identity_gate          │ │
│  └─────────────────────────────────────────────────────────┘ │
│           ▲                   ▲                    ▲         │
│  ┌────────┴────────┐  ┌──────┴──────┐      ┌──────┴──────┐  │
│  │ face_model_cmds │  │ login/quality│      │ battery/cpu │  │
│  │ _history        │  │ (history)    │      │ (affinity)  │  │
│  └─────────────────┘  └─────────────┘      └─────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **校园网探测**:
   * `check_campus_status` (`campus_detect.rs` 第 216-238 行) 调用 `probe_campus_with_ssid` (第 185-200 行)。
   * `probe_campus` (第 65-125 行) 枚举网卡，调用 `pick_campus_source_ip` (第 15-37 行) 获取源 IP。
   * 若 `wifi_off` 为真 (第 85-93 行) 直接短路返回。
   * 否则并行/串行执行 `portal_reachable` (第 44-53 行) 对网关与 Portal 的 TCP 探测。
   * 结果封装 `CampusProbe` (第 56-63 行)，`cache_source_ip` (第 203-210 行) 缓存。
   * `campus_status_message` (第 129-137 行) 生成文案。

2. **自助服务与安全门**:
   * 前端发起 `bind_operator` 等 (`self_service_cmds.rs` 第 88-132 行等)，内部 `ensure_identity_gate` (第 42-51 行) 检查 `identity_verified_recently` (`identity_gate.rs` 第 23-33 行)。
   * 拦截则返回错误；否则继续 HTTP 请求。
   * 前端生物识别成功调 `verify_biometric_identity` (第 70-85 行)，成功时 `note_identity_verified` (`identity_gate.rs` 第 18-20 行) 更新时间戳。

3. **面部模型下载**:
   * `face_models_download` (`face_model_cmds.rs` 第 126-133 行) 查 `FACE_DL_RUNNING` (第 54 行) 互斥。
   * `download_models` (第 135-184 行) 按镜像优先级遍历 `FACE_MODEL_FILES` (第 26-51 行)。
   * `download_one` (第 187-243 行) 流式下载，每 200ms 发射 `face-models-download-progress` 事件，完成后 SHA256 校验。

4. **历史记录持久化**:
   * 登录成功调 `login_history::append` (`login_history.rs` 第 52-77 行)：头插、截断 100、原子写；读取损坏由 `read` (第 28-41 行) 备份重置。
   * 质量记录调 `quality_history::append` (`quality_history.rs` 第 52-75 行) 类似，负延迟转 `None`。

## Connections

* [[account-cmds]]: `system_cmds::get_init_data` (`system_cmds.rs` 第 6-29 行) 依赖 `account_cmds::list_account_items_sync`。
* [[config-state]]: 几乎所有命令依赖 `config_state::current_settings`（如 `campus_detect.rs` 第 222 行、`self_service_cmds.rs` 第 54 行）。
* [[monitor-loop]]: `system_cmds::get_init_data` 调用 `monitor_loop::status_value` (第 27 行)。
* [[update-cmds]]: `face_model_cmds` 复用 `update_cmds::allowed_url`, `http_client`, `verify_file_sha256` (第 16 行 use)。
* [[network-bind-plugin]]: `campus_detect` 依赖 `tauri_plugin_campus_network_bind` 获取 SSID（第 164 行）与权限请求（第 245 行）。

## Known Issues

* Android 平台 SELinux 禁止 ICMP 原始 socket（`campus_detect.rs` 第 1-3 行注释），校园网探测改用 TCP 可达性；若 `portal_url` 配置公网域名，家宽也可能连通，属已知边界（第 105-106 行）。
* `cpu_affinity::pin_current_thread_to_little_cores` 在非安卓恒返回 `false`，且读不到频率节点或同构拓扑时静默返回空（第 18-25、48-49 行）。
* `battery_cmds` 中所有命令在非 `mobile` 平台返回 `Err("...仅安卓端可用")`（`battery_cmds.rs` 第 36-40、56-60 等）。
* 人脸模型哈希锁定 npm 3.3.6（`face_model_cmds.rs` 第 5-6 行），上游更新模型会哈希失配，升级依赖时需同步更新 `FACE_MODEL_FILES` 中的 sha256。
* `AndroidState` 的 `config` 仅存内存，落盘必经 Keystore 加密（`android_state.rs` 第 19 行注释）。
