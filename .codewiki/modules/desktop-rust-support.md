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

本模块是桌面端 Rust 辅助模块，负责 Android 侧的校园网探测、自助服务命令、面部模型管理、历史记录持久化、电池优化与系统设置跳转、CPU 大小核绑核以及系统状态初始化。由于 Android 平台限制（SELinux 禁止 ICMP 原始 Socket、无 root 权限），本模块对网络探测逻辑进行了深度适配，并提供了针对厂商省电策略的降级链支持。

## Key Components

### `campus_detect` (校园网探测)
**文件**: `android/src-tauri/src/campus_detect.rs`

核心功能：枚举网卡、判定校园网连通性、处理 WiFi SSID。

*   **常量**
    *   `PORTAL_PORT`: `80` (第 9 行)
    *   `TCP_TIMEOUT`: `Duration::from_secs(3)` (第 10 行)

*   **Structs / Enums**
    *   `CampusProbe` (第 56-63 行): 探测结果结构体
        *   `source`: `Option<Ipv4Addr>` - 源 IP
        *   `portal_ok`: `bool` - Portal TCP 可达性
        *   `on_campus`: `bool` - 是否在校内
        *   `wifi_off`: `bool` - WiFi 是否关闭（短路标志）

*   **Functions**
    *   `pick_campus_source_ip(interfaces)`: (第 15-37 行) 从网卡列表中筛选 IPv4。排除蜂窝接口 (`rmnet`, `ccmni`)、link-local、回环地址。优先选择 `wlan0`。
    *   `portal_reachable(host, port, timeout)`: (第 44-53 行) 替代 ICMP Ping。使用 `bound_socket::tcp_connect_bounded` 进行 TCP 连接测试，支持 VPN 接管下的物理网卡直连。
    *   `probe_campus(campus_gateway, portal_url)`: (第 65-125 行) 主探测函数。
        *   包含“WiFi 未连接短路”逻辑 (第 82-93 行)。
        *   三层判定：子网 (/18) -> 网关 TCP -> Portal TCP。
    *   `campus_status_message(wifi_off, on_campus)`: (第 129-137 行) 生成状态文案 ("WiFi 未连接", "已连接校园网", "未连接校园网")。
    *   `portal_host_of(url)`: (第 140-149 行) 提取 URL 中的 Host 部分。
    *   `ssid_matches(ssid, required)`: (第 153-157 行) SSID 匹配逻辑，支持历史同源 SSID (`iwxxy-2`, `iwxxy-3`) 的特判。
    *   `current_wifi_ssid(app)`: (第 163-178 行) 异步获取当前 WiFi SSID，通过 JNI 阻塞调用。
    *   `probe_campus_with_ssid(app, settings)`: (第 185-200 行) 结合 SSID 感知的探测版本。
    *   `cache_source_ip(state, source)`: (第 203-210 行) 缓存源 IP 供后续登录绑定使用。
    *   `check_campus_status(app, state)`: (第 216-238 行) Tauri 命令，返回 JSON 状态。
    *   `request_wifi_ssid_permission(app)`: (第 244-249 行) 触发 `NEARBY_WIFI_DEVICES` 权限请求。

*   **Tests**
    *   `wlan0_campus_ip_被优先选中` (第 262-269 行)
    *   `蜂窝接口永不返回` (第 271-275 行)
    *   `link_local_地址被排除` (第 277-281 行)
    *   `无_wlan0_时取任意合法内网ipv4` (第 283-287 行)
    *   `portal_host_提取` (第 289-295 行)
    *   `campus_状态文案_wifi短路优先` (第 297-304 行)
    *   `ssid_匹配语义对齐桌面` (第 306-320 行)

### `self_service_cmds` (自助服务命令)
**文件**: `android/src-tauri/src/self_service_cmds.rs`

核心功能：包装 `campus_login_lib::self_service`，实现绑定、查询、注销、改密等自助服务操作，包含生物识别验证门。

*   **Structs**
    *   `CommandResult` (第 7-14 行):
        *   `success`: `bool`
        *   `message`: `Option<String>`
        *   `data`: `Option<serde_json::Value>`
    *   `resolve_self_password` (内部函数, 第 27-36 行): 密码回退逻辑。

*   **Functions**
    *   `ensure_identity_gate(settings)`: (第 39-48 行) 验证门检查。若总开关关闭或未在 TTL 内验证，返回错误信息。
    *   `load_settings(app)`: (第 50-52 行) 加载配置。
    *   `local_addr_of(state)`: (第 54-60 行) 从状态中获取缓存的本地 IP。
    *   `verify_biometric_identity(success, method, _consent_message)`: (第 67-82 行) 记录生物识别验证成功时间戳。仅当 `success == true` 时解锁验证门。
    *   `bind_operator(...)`: (第 85-127 行) 运营商绑定。需通过验证门。
    *   `query_bind_status(...)`: (第 130-159 行) 查询绑定状态。不设验证门。
    *   `query_self_dashboard(...)`: (第 162-184 行) 查询仪表盘（在线列表、登录历史）。不设验证门。
    *   `is_iso_date(s)`: (第 187-199 行) 严格校验 YYYY-MM-DD 格式。
    *   `query_self_online_log(...)`: (第 202-228 行) 查询在线日志。不设验证门。
    *   `self_offline_session(...)`: (第 231-256 行) 主动注销会话。需通过验证门。
    *   `change_self_password(...)`: (第 263-297 行) 修改密码。需通过验证门，新密码校验 6-11 位 ASCII 字母数字。
    *   `reveal_operator_credential(...)`: (第 301-328 行) 揭示手机号/短信密码。**强制**验证门，不受总开关影响。

*   **Tests**
    *   `自助密码回退_掩码与空回退_新值直用` (第 343-350 行)
    *   `验证门_总开关关闭放行_开启走TTL` (第 352-365 行)
    *   `is_iso_date_严格校验` (第 367-374 行)

### `face_model_cmds` (面部模型管理)
**文件**: `android/src-tauri/src/face_model_cmds.rs`

核心功能：按需下载人脸识别模型文件，确保完整性（SHA256 校验），提供进度上报。

*   **Constants**
    *   `FACE_MODEL_BASE`: GitHub Raw URL (第 18 行)
    *   `FACE_MODEL_MIRRORS`: 镜像代理列表 (第 20-24 行)
    *   `FACE_MODEL_FILES`: 文件名与 SHA256 哈希映射 (第 26-51 行)，共 6 个文件。
    *   `FACE_DL_RUNNING`: `AtomicBool` 并发互斥锁 (第 54 行)

*   **Structs**
    *   `FaceModelFileState` (第 56-63 行):
        *   `name`: `String`
        *   `size`: `u64`
        *   `ok`: `bool` (存在且哈希匹配)
    *   `FaceModelsState` (第 65-72 行):
        *   `ready`: `bool`
        *   `dir`: `String` (绝对路径)
        *   `files`: `Vec<FaceModelFileState>`
    *   `FaceModelProgress` (第 74-81 行):
        *   `file`: `String`
        *   `downloaded`: `u64`
        *   `total`: `u64`
        *   `percent`: `f64`

*   **Functions**
    *   `models_dir(app)`: (第 86-93 行) 获取模型存储目录 `dataDir/files/face-models`。
    *   `state_inner(app)`: (第 96-118 行) 检查所有模型文件的存存与哈希状态。
    *   `face_models_state(app)`: (第 121-123 行) Tauri 命令，返回当前状态。
    *   `face_models_download(app)`: (第 126-133 行) Tauri 命令，触发下载。使用 `FACE_DL_RUNNING` 防止并发。
    *   `download_models(app)`: (第 135-184 行) 实际下载逻辑。支持镜像降级，逐文件校验，原子写入 (`.part` -> 重命名)。
    *   `download_one(client, url, dest, name, app)`: (第 187-243 行) 单文件流式下载，每 200ms 发射 `face-models-download-progress` 事件。

*   **Tests**
    *   `mirror_url_keeps_prefix_form` (第 251-256 行)
    *   `sources_order_follows_mirror_first` (第 258-274 行)

### `login_history` (登录历史)
**文件**: `android/src-tauri/src/login_history.rs`

核心功能：持久化登录记录，头插法，上限 100 条，损坏备份。

*   **Constants**
    *   `LOGIN_HISTORY_MAX`: `100` (第 10 行)
    *   `LOGIN_HISTORY_LOCK`: `Mutex<()>` (第 8 行)

*   **Structs**
    *   `LoginHistoryEntry` (第 12-21 行):
        *   `time`: `String`
        *   `success`: `bool`
        *   `message`: `String`
        *   `adapter`: `String` (固定为 `wlan0`)
        *   `user`: `String`
        *   `login_type`: `String` (JSON key: `type`)

*   **Functions**
    *   `history_path(dir)`: (第 23-25 行) 路径拼接。
    *   `read(dir)`: (第 28-41 行) 读取历史。损坏时备份为 `.corrupt-*.bak` 并重置。
    *   `chrono_epoch_millis()`: (第 43-48 行) 时间戳辅助函数。
    *   `append(dir, success, message, user, login_type)`: (第 52-77 行) 追加记录。原子写 (`tmp` + `rename`)。

*   **Tests**
    *   `头插法_最新在前` (第 91-101 行)
    *   `上限_100条截断` (第 103-114 行)
    *   `损坏文件_备份后重置` (第 116-130 行)
    *   `记录字段_序列化形状对齐桌面` (第 132-142 行)

### `quality_history` (网络质量历史)
**文件**: `android/src-tauri/src/quality_history.rs`

核心功能：持久化网络质量检测数据，结构与桌面一致。

*   **Constants**
    *   `QUALITY_HISTORY_MAX`: `100` (第 12 行)
    *   `QUALITY_HISTORY_LOCK`: `Mutex<()>` (第 10 行)

*   **Structs**
    *   `QualityHistoryEntry` (第 14-22 行):
        *   `timestamp`: `u64`
        *   `gateway_latency`: `Option<i64>` (负值落盘为 null)
        *   `external_latency`: `Option<i64>`
        *   `quality`: `String`

*   **Functions**
    *   `read(dir)`: (第 29-42 行) 读取并处理损坏文件。
    *   `epoch_millis()`: (第 44-49 行) 时间戳辅助函数。
    *   `append(dir, result)`: (第 52-75 行) 追加质量记录。将 `-1` 延迟转为 `None`。

*   **Tests**
    *   `头插法_最新在前` (第 101-114 行)
    *   `上限_100条截断` (第 116-127 行)
    *   `损坏文件_备份后重置` (第 129-143 行)
    *   `记录字段_序列化形状双端一致` (第 145-156 行)

### `battery_cmds` (电池与系统设置)
**文件**: `android/src-tauri/src/battery_cmds.rs`

核心功能：安卓专属的电池优化白名单管理与通知设置跳转。

*   **Structs**
    *   `BatteryOptimizationInfo` (第 8-17 行):
        *   `ignoring`: `bool`
        *   `brand`: `String`
        *   `has_vendor_target`: `bool`

*   **Functions**
    *   `get_battery_optimization_info(app)`: (第 20-41 行) 查询电池优化状态。仅在 `#[cfg(mobile)]` 下有效。
    *   `request_ignore_battery_optimizations(app)`: (第 46-61 行) 申请忽略电池优化。
    *   `open_vendor_battery_settings(app)`: (第 65-81 行) 打开厂商特定省电页。
    *   `open_notification_settings(app)`: (第 87-102 行) 打开应用通知设置页。

### `cpu_affinity` (CPU 绑核)
**文件**: `android/src-tauri/src/cpu_affinity.rs`

核心功能：识别小核并将当前线程绑定至小核，以节省功耗。

*   **Functions**
    *   `little_core_ids()`: (第 8-31 行) 解析 `/sys/devices/system/cpu/cpu*/cpufreq/cpuinfo_max_freq`。频率差 <30% 视为同构，返回空。
    *   `pin_current_thread_to_little_cores()`: (第 34-50 行) 使用 `sched_setaffinity` 绑核。非 Android 平台恒返回 `false`。

*   **Tests**
    *   `host_环境_恒不绑定` (第 57-61 行)
    *   `小核识别_同构拓扑返回空` (第 63-68 行)

### `system_cmds` (系统聚合命令)
**文件**: `android/src-tauri/src/system_cmds.rs`

核心功能：聚合初始化数据、日志管理。

*   **Functions**
    *   `get_init_data(app)`: (第 6-29 行) 一次性返回配置、账号、版本、日志设置等初始数据。补全桌面专属字段的默认值。
    *   `get_logs(app, lines)`: (第 32-39 行) 读取最近日志。
    *   `clear_logs(app)`: (第 42-46 行) 清理日志。
    *   `get_log_retention_days()`: (第 49-51 行)
    *   `set_log_retention_days(days)`: (第 54-56 行)
    *   `get_debug_mode()`: (第 59-61 行)
    *   `set_debug_mode(enabled)`: (第 64-66 行)

### `identity_gate` (生物识别验证门)
**文件**: `android/src-tauri/src/identity_gate.rs`

核心功能：进程内 TTL 时间戳管理，用于控制敏感操作的访问权限。

*   **Constants**
    *   `IDENTITY_VERIFY_TTL_SECS`: `600` (第 8 行)

*   **Static Variables**
    *   `LAST_VERIFY_EPOCH_SECS`: `AtomicU64` (第 6 行)

*   **Functions**
    *   `now_epoch_secs()`: (第 10-15 行) 当前纪元秒数。
    *   `note_identity_verified()`: (第 18-20 行) 更新验证时间戳。
    *   `identity_verified_recently()`: (第 23-33 行) 检查是否在 TTL 内。处理时钟回拨（视为过期）。
    *   `reset_for_tests()`: (第 36-38 行) 测试用重置。

*   **Tests**
    *   `从未验证_为假` (第 45-49 行)
    *   `TTL_内为真_外为假` (第 51-58 行)
    *   `时钟回拨_视为过期` (第 60-65 行)

### `android_state` (全局状态)
**文件**: `android/src-tauri/src/android_state.rs`

核心功能：维护 Android 端的全局内存状态。

*   **Static Variables**
    *   `WATCHER_FAILED`: `AtomicBool` (第 14 行) - WiFi watcher 注册失败标志。

*   **Structs**
    *   `AndroidState` (第 16-21 行):
        *   `cached_source_ip`: `Mutex<Option<Ipv4Addr>>`
        *   `config`: `Mutex<Option<Settings>>`

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                    Tauri App Handle                          │
│  ┌─────────────────┐  ┌──────────────────┐  ┌──────────────┐ │
│  │ AndroidState    │  │ IdentityGate     │  │ ConfigState  │ │
│  │ (src-ip/cache)  │  │ (TTL/Verify)     │  │ (Settings)   │ │
│  └────────┬────────┘  └────────┬─────────┘  └──────┬───────┘ │
│           │                   │                    │         │
│  ┌────────▼───────────────────▼────────────────────▼───────┐ │
│  │                  Campus Detect & Self Service            │ │
│  │  probe_campus() ──► check_campus_status()               │ │
│  │  bind/change/offline ──► ensure_identity_gate()         │ │
│  └─────────────────────────────────────────────────────────┘ │
│           ▲                   ▲                    ▲         │
│  ┌────────┴────────┐  ┌──────┴──────┐      ┌──────┴──────┐  │
│  │ Face Models     │  │ History     │      │ Battery/CPU │  │
│  │ (Download/Hash) │  │ (Persist)   │      │ (Vendor/API)│  │
│  └─────────────────┘  └─────────────┘      └─────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **校园网探测**:
    *   `check_campus_status` 调用 `probe_campus_with_ssid`。
    *   `probe_campus` 枚举网卡，调用 `pick_campus_source_ip` 获取源 IP。
    *   若 WiFi 关闭 (`wifi_off=true`)，直接返回短路状态。
    *   否则并行执行 `portal_reachable` (Portal TCP) 和 `portal_reachable` (Gateway TCP)。
    *   结果封装为 `CampusProbe`，并通过 `cache_source_ip` 缓存源 IP。
    *   最终通过 `campus_status_message` 生成用户可见文案。

2.  **自助服务与安全门**:
    *   前端发起 `bind_operator` 或 `change_self_password`。
    *   后端调用 `ensure_identity_gate` 检查 `identity_verified_recently`。
    *   若验证门拦截，返回错误；否则继续 HTTP 请求。
    *   前端在生物识别成功后调用 `verify_biometric_identity(success=true)`，更新 `LAST_VERIFY_EPOCH_SECS`。

3.  **面部模型下载**:
    *   前端调用 `face_models_download`。
    *   后端检查 `FACE_DL_RUNNING` 互斥锁。
    *   遍历 `FACE_MODEL_FILES`，按镜像优先级下载。
    *   下载完成后立即进行 SHA256 校验，失败则删除文件并报错。
    *   下载过程中每 200ms 发射 `face-models-download-progress` 事件。

4.  **历史记录持久化**:
    *   登录成功后调用 `login_history::append`。
    *   获取锁，读取现有 JSON，头插新条目，截断至 100 条。
    *   原子写入 (`tmp` 文件 + `rename`)。
    *   若读取时 JSON 损坏，备份为 `.corrupt-*.bak` 并清空。

## Connections

*   [[account-cmds]]: `system_cmds::get_init_data` 依赖 `account_cmds::list_account_items_sync`。
*   [[config-state]]: 几乎所有命令都依赖 `config_state::current_settings` 获取配置。
*   [[monitor-loop]]: `system_cmds::get_init_data` 依赖 `monitor_loop::status_value`。
*   [[update-cmds]]: `face_model_cmds` 复用 `update_cmds::allowed_url`, `http_client`, `verify_file_sha256`。
*   [[network-bind-plugin]]: `campus_detect` 依赖 `tauri_plugin_campus_network_bind` 获取 SSID 和权限。