---
title: rust-backend-commands
type: module
source_files:
  - android/src-tauri/src/cpu_affinity.rs
  - android/src-tauri/src/face_model_cmds.rs
  - android/src-tauri/src/identity_gate.rs
  - android/src-tauri/src/login_history.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/account_cmds.rs
  - android/src-tauri/src/battery_cmds.rs
  - android/src-tauri/src/campus_detect.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/protocol_cmds.rs
  - android/src-tauri/src/quality_cmds.rs
  - android/src-tauri/src/quality_history.rs
  - android/src-tauri/src/self_service_cmds.rs
  - android/src-tauri/src/system_cmds.rs
  - android/src-tauri/src/update_cmds.rs
tags: [tauri, backend, android, protocol, monitor, security]
---

## Overview
本模块是 Tauri Android 后端的核心业务实现层，负责处理账户管理、网络协议认证、后台监控循环、校园网探测、生物识别门控、质量历史及自动更新逻辑。它通过 `#[tauri::command]` 暴露接口给前端，并依赖 Kotlin 侧的 JNI 插件（如 `CampusNetworkBindExt`, `CampusMonitorServiceExt`）处理 Android 系统级操作（WiFi 绑定、电池优化、APK 安装）。

核心设计原则包括：
1. **原子性与安全性**：所有文件 I/O 采用“写临时文件 + rename”策略；密码字段加密存储且出站脱敏；更新下载强制 SHA256 校验。
2. **状态隔离与竞态防护**：使用代际 Token (Generation) 和互斥锁防止后台循环启动/停止时的状态污染；登录/注销操作串行化执行。
3. **Android 适配**：针对双网路由漂移提供 WiFi 强制绑定；利用小核调度降低功耗；处理 Captive Portal 自动接受。

## Key Components

### 1. 底层系统与配置 (`config_state.rs`, `cpu_affinity.rs`)

#### `cpu_affinity.rs`
*   **职责**: 解析 CPU 拓扑并将当前线程绑定至小核以节能。
*   **函数**:
    *   `little_core_ids()` (L17): 遍历 `/sys/devices/system/cpu/cpu*/cpufreq/cpuinfo_max_freq`，构建 `BTreeMap<usize, u64>`。若最大频率 < 最小频率 * 1.3，视为同构返回空 Vec。过滤保留最小频率 ID。
    *   `pin_current_thread_to_little_cores()` (L40): `#[cfg(target_os = "android")]` 下调用 `libc::sched_setaffinity` 绑定线程。Host 端恒返回 `false`。

#### `config_state.rs`
*   **职责**: 全局配置管理，含持久化、加密、版本迁移。
*   **常量**:
    *   `PASSWORD_MASK`: `"***"` (L10)。
    *   `CONFIG_FILE`: `"config.json"` (L12)。
    *   `Settings::default()`: 默认值如 `background_check_interval: 60_000`, `config_schema_version: 7` (L18-L30)。
*   **结构体**:
    *   `struct Settings`: 全量配置，包含账号、网络、UI、定时任务等。注意 `night_outbound_restore` 为纯标记 ("logged_out")。
    *   `struct CryptoBridge`: 加解密桥接器，封装 `encrypt`/`decrypt` 闭包。生产环境调用 `CampusKeystoreExt`。
*   **函数**:
    *   `load_file(path, bridge)` (L45): 读取 JSON -> 反序列化 `EncodedSettings` -> 解密密码。失败则密码置空。
    *   `save_file(path, bridge, s)` (L60): 原子写入 `<filename>.<pid>.tmp` -> `rename`。落盘前清空明文密码，存入 `password_cipher`。
    *   `migrate_legacy_defaults(dir, bridge, s)` (L80): v0-v7 版本迁移。仅当 `schema_version < N` 且值等于历史旧默认时才更新。
    *   `resolve_password_field(incoming, current, clear)` (L100): 密码回填逻辑。传入 `***` 或空串保留当前值；`clear=true` 则置空。
    *   `masked_for_display(s)` (L115): 非空密码替换为 `"***"` 后序列化。
    *   `emit_config_changed(app, s)` (L125): 广播 `config-changed` 事件。
    *   `current_settings(app)` (L135): 缓存优先读取。
    *   `get_config` (Command) (L145): 返回掩码后配置。
    *   `save_config(config, clear_password)` (Command) (L155): 持有全局 IO 锁，合并配置 -> 落盘 -> 触发自动建号 -> 广播事件。

### 2. 账户管理 (`account_cmds.rs`)

*   **职责**: 多账号创建、切换、删除、重命名及自动同步。
*   **结构体**:
    *   `AccountItem`: `id` (文件名 stem), `displayName`, `user`, `operator`, `passwordSaved`。
    *   `AccountResult`: `success`, `message`, `active_account`, `display_name`, `config`。
*   **函数**:
    *   `sanitize_account_id(user)` (L20): 保留 `[a-zA-Z0-9_\u{4e00}-\u{9fff}-]`，截断至 32 字符。
    *   `validate_account_name(name)` (L30): 长度 1-32，白名单校验。
    *   `list_account_items_sync(dir)` (L40): 过滤非 json/隐藏文件，按 ID 排序。
    *   `switch_account(app, account_name)` (Command) (L50): 持 IO 锁。加载目标账号 -> 合并凭据到主配置 -> 清空夜间恢复标记 -> 更新 `active_account` -> 落盘 -> 清理快照。
    *   `save_current_as_account(app, account_name)` (Command) (L70): 另存为。保留已有自定义字段，覆盖凭据。
    *   `create_account(...)` (Command) (L85): 新建账号。仅保存凭据和显示名，不切换主配置。
    *   `delete_account(app, account_name)` (Command) (L95): 删除文件。若删当前账号，清除主配置对应字段。
    *   `rename_account(app, account_id, display_name)` (Command) (L105): 校验新名 -> 检查重名 -> 更新账号文件 `display_name`。
    *   `auto_create_account_for_current(app, current)` (L115): 自动建号。凭据非空 -> 生成 ID -> 若存在则比对 user (防撞库) 和凭据 (幂等更新)。
    *   `merge_save_as_target(current, existing, safe_name)` (L130): 纯函数，处理另存时字段合并。

### 3. 生物识别与历史记录 (`identity_gate.rs`, `login_history.rs`)

#### `identity_gate.rs`
*   **职责**: 进程内生物识别 TTL 管理。
*   **常量**:
    *   `LAST_VERIFY_EPOCH_SECS`: `AtomicU64` (L10)。
    *   `IDENTITY_VERIFY_TTL_SECS`: 600 秒 (L12)。
*   **函数**:
    *   `note_identity_verified()` (L23): 写入当前时间戳。
    *   `identity_verified_recently()` (L28): 时钟回拨保护 (`now < last` 视为过期)。

#### `login_history.rs`
*   **职责**: 维护 `login-history.json`，最近 100 条记录。
*   **常量**:
    *   `LOGIN_HISTORY_LOCK`: `Mutex<()>` (L10)。
    *   `LOGIN_HISTORY_MAX`: 100 (隐含在 truncate 逻辑)。
*   **结构体**:
    *   `LoginHistoryEntry`: `time`, `success`, `message`, `adapter` (固定 `wlan0`), `user`, `login_type`。
*   **函数**:
    *   `read(dir)` (L32): 损坏备份 `.corrupt-{timestamp}.bak`。
    *   `append(dir, ...)` (L58): 头插法，truncate 至 100，原子写 `.tmp` -> `rename`。

### 4. 人脸模型 (`face_model_cmds.rs`)

*   **职责**: 下载并校验 6 个人脸识别模型 (~8.9MB)。
*   **常量**:
    *   `FACE_MODEL_BASE`, `FACE_MODEL_MIRRORS` (代理列表) (L19-21)。
    *   `FACE_MODEL_FILES`: 6 个文件的 `(name, sha256)` 映射 (L24-43)。
    *   `FACE_DL_RUNNING`: `AtomicBool` 互斥锁 (L46)。
*   **结构体**:
    *   `FaceModelFileState`: `name`, `size`, `ok`。
    *   `FaceModelsState`: `ready`, `dir`, `files`。
    *   `FaceModelProgress`: `file`, `downloaded`, `total`, `percent`。
*   **函数**:
    *   `models_dir(app)` (L53): 路径 `app_data_dir/files/face-models`。
    *   `state_inner(app)` (L61): 异步检查文件存在性及 SHA256。
    *   `face_models_state` / `face_models_download` (Commands) (L78, L83): CAS 互斥防止并发下载。
    *   `download_models(app)` (L90): 源排序 -> 逐文件下载 -> 最终 SHA256 校验。
    *   `download_one(...)` (L130): 流式下载，`.part` 临时文件，每 200ms 发射进度事件。

### 5. 校园网探测 (`campus_detect.rs`)

*   **职责**: TCP 连接 Portal 和网关，替代 ICMP Ping。
*   **常量**:
    *   `PORTAL_PORT`: 80 (L10)。
    *   `TCP_TIMEOUT`: 3s (L12)。
*   **函数**:
    *   `pick_campus_source_ip(interfaces)` (L20): 优选源 IP，排除蜂窝接口。
    *   `portal_reachable(host, port, timeout)` (L35): 绑定物理网卡 TCP 探测。
    *   `probe_campus(campus_gateway, portal_url)` (L50): 子网判定 -> 并行 Portal/Gateway 探测。
    *   `probe_campus_with_ssid(app, settings)` (L70): 增强版，SSID 命中强制成功。
    *   `check_campus_status(app, state)` (Command) (L90): 强制绑 WiFi -> 执行探针 -> 返回 JSON。
    *   `ssid_matches(ssid, required)` (L110): 大小写不敏感，兼容 `i-wxxy`。
    *   `request_wifi_ssid_permission` (Command) (L120): 请求权限。

### 6. 电池与通知 (`battery_cmds.rs`)

*   **职责**: 跳转电池优化/通知设置。
*   **函数**:
    *   `get_battery_optimization_info` (Command): 查询忽略状态。
    *   `request_ignore_battery_optimizations` (Command): 申请白名单。
    *   `open_vendor_battery_settings` (Command): 跳转厂商页面。
    *   `open_notification_settings` (Command): 跳转通知设置。
    *   *边界*: 非 `mobile` 编译直接报错。

### 7. 后台监控循环 (`monitor_loop.rs`)

*   **职责**: 维持监控生命周期，执行检测、登录、夜间切换。
*   **结构体**:
    *   `pub struct MonitorState` (L28-L65): `running`, `check_count`, `consecutive_failures`, `reconnect_count`, `last_login_attempt_ms`, `was_online`, `last_result`, `desired_interval_ms`, `idle_interval_ms`, `logout_protected_until_ms`, `wifi_event_ms`, `notified_online`, `scheduled_login_day`, `scheduled_logout_day`。
    *   `MONITOR`: `lazy_static!` 全局单例 (L17-L19)。
    *   `MONITOR_GENERATION`: `AtomicU64` 代际 Token (L38)。
    *   `LOGIN_ATTEMPT_LOCK`: `tokio::sync::Mutex<()>` (L43)。
*   **函数**:
    *   `effective_interval_ms` (L98): 巡检分档计算。
    *   `should_attempt_login` (L115): 自动登录判定（在线=false, on_campus=true, 冷却期检查）。
    *   `wifi_event_is_burst_start` (L86): WiFi 去抖判断。
    *   `start_background_check` (Command) (L165): 启动 FGS，注册 WiFi 监听，推进代际。
    *   `stop_background_check` (Command) (L208): 推进代际，清标志。
    *   `trigger_background_check` (Command) (L276): 手动触发。
    *   `get_background_status` (Command) (L285): 返回展平状态。
    *   `get/set_boot_autostart` / `get/set_notification_enabled` (Commands) (L290-L345): 配置读写。
    *   `monitor_tick_loop(app, interval_ms, gen)` (L458): 主循环。代际校验 -> 定时任务 -> 间隔热更新 -> 分档跳拍 -> `run_check_once`。
    *   `run_check_once(app)` (L623): 单次检测核心。前置闸（配置/夜间态/静默期）-> 探针窗口 -> 校园网判定 -> Portal 探测（小核线程）-> 三态护栏 -> 状态机更新 -> 自动登录 -> 结果 Emit。
    *   `run_scheduled_actions` (内部): 夜间出站切换逻辑。Switch (注销+设 system setting) / Restore (登录+还原 setting)。连续失败 3 次放弃自动切换。
    *   `verify_night_switch` (内部): 验证运营商切换结果。
    *   `portal_probe_on_little_cores` (L600): 专用线程绑定小核执行 Portal 探测。
    *   `notify_system` (L146): 系统通知。
    *   `with_vpn_hint` (L161): VPN 提示文案。

### 8. 协议与网络绑定 (`protocol_cmds.rs`)

*   **职责**: 登录、注销、Portal 探测、Android 网络绑定。
*   **函数**:
    *   `do_login(user, password, operator, adapter, app, state)` (L20): 手动登录入口。重置熔断，落盘历史。
    *   `run_login(user, password, operator, state)` (L68): 实际登录执行，`spawn_blocking` 调用 lib。
    *   `do_logout(user, adapter, app, state)` (L96): 注销入口。设置 60s 保护期。
    *   `check_portal_status(adapter_ip, app, state)` (L136): Portal 状态探测。
    *   `ping_test()` (L158): 健康检查，返回 "pong"。
    *   `bind_to_wifi(app)` (L164): JNI 调用 `CampusNetworkBindExt::bind_to_wifi()`。
    *   `accept_wifi_network(app)` (L178): JNI 调用，自动接受无互联网 WiFi。
    *   `get_avoid_bad_wifi_status(app)` (L192): 查询系统安全设置。
    *   `restore_written_settings(app)` (L208): 还原系统网络设置。
    *   `ensure_wifi_bound(app)` (L226): 核心辅助。旁路代理跳过 -> 绑定 WiFi -> 清连接池 -> 接受 Portal。
    *   `accept_campus_wifi(app)` (L286): 辅助，尝试反射 API 或写 Settings 接受 WiFi。
    *   `cached_adapter_ip(state)` (L150): 读取缓存 IP。

### 9. 质量监测 (`quality_cmds.rs`, `quality_history.rs`)

#### `quality_cmds.rs`
*   **职责**: 网络质量检测与退避算法。
*   **常量**:
    *   `LATENCY_RUNNING`: `AtomicBool`。
    *   `LATENCY_GENERATION`: `AtomicU64`。
    *   `GOOD_LEVELS`: ["excellent", "great", "good"]。
    *   `BACKOFF_STABLE_STREAK`: 5。
    *   `BACKOFF_MAX_MS`: 1,800,000。
*   **函数**:
    *   `check_network_quality(app)` (Command) (L66): 单次检测。
    *   `start_latency_test(app)` (Command) (L76): 启动后台循环。
    *   `stop_latency_test()` (Command) (L93): 停止循环。
    *   `next_backoff_interval_ms(current_ms, stable_good_streak, base_ms)` (L116): 退避计算。连续 5 次 good 翻倍，上限 1800s。
    *   `latency_loop(app, settings, gen)` (L136): 后台循环。尾部 sleep 支持动态间隔。
    *   `run_quality_once(app)` (L40): 内部执行，落盘 `quality_history::append`。

#### `quality_history.rs`
*   **职责**: 质量历史持久化。
*   **常量**:
    *   `QUALITY_HISTORY_LOCK`: `Mutex<()>`。
    *   `QUALITY_HISTORY_MAX`: 100。
*   **结构体**:
    *   `QualityHistoryEntry`: `timestamp`, `gateway_latency` (Option<i64>), `external_latency` (Option<i64>), `quality`。
*   **函数**:
    *   `read(dir)` (L30): 损坏备份。
    *   `append(dir, result)` (L45): 头插，truncate，原子写。

### 10. 自助服务 (`self_service_cmds.rs`)

*   **职责**: 运营商绑定、离线会话、改密等，实施生物识别门控。
*   **结构体**:
    *   `CommandResult`: `success`, `message?`, `data?`。
*   **函数**:
    *   `resolve_self_password(incoming, settings)` (L20): 密码解析，回退掩码。
    *   `ensure_identity_gate(settings)` (L30): 验证门。`self_hello_enabled` 关闭则放行，否则检查 TTL。
    *   `verify_biometric_identity(success, method, _consent_message)` (L40): 记录验证。**必须** `success == true`。
    *   `bind_operator(...)`, `self_offline_session(...)`, `change_self_password(...)`, `reveal_operator_credential(...)` (Commands): 需经过验证门。
    *   `query_bind_status(...)`, `query_self_dashboard(...)`, `query_self_online_log(...)` (Commands): 查询类，不经过验证门。
    *   `is_iso_date(s)` (L60): YYYY-MM-DD 校验。
    *   `local_addr_of(state)` (L70): 获取本地 IP。

### 11. 系统数据 (`system_cmds.rs`)

*   **职责**: 初始数据聚合，日志管理。
*   **函数**:
    *   `get_init_data(app)` (Command) (L10): 聚合 config, accounts, version, autoLaunch 等。
    *   `get_logs(app, lines)` (Command) (L20): 异步读取日志。
    *   `clear_logs(app)` (Command) (L30): 清空日志。
    *   `get/set_log_retention_days(days)` (Commands) (L40): 日志保留天数。
    *   `get/set_debug_mode(enabled)` (Commands) (L50): 调试模式开关。

### 12. 自动更新 (`update_cmds.rs`)

*   **职责**: 版本检查、流式下载 APK、SHA256 校验、安装。
*   **常量**:
    *   `VERSION_FILE`, `VERSION_MIRRORS` (L10-12)。
    *   `DOWNLOAD_ALLOWED_HOSTS` ([]&str) (L15): 域名白名单。
    *   `MAX_DOWNLOAD_BYTES`: 500MB (L18)。
    *   `APK_STALE_SECS`: 24h (L20)。
    *   `UPDATE_CHECKSUM`: `Mutex<Option<(tag, digest)>>` (L25)。
    *   `LAST_CHECK_ERROR`, `LAST_CHECK_TIME_MS` (L30)。
*   **结构体**:
    *   `UpdateInfo`: `has_update`, `latest_version`, `assets`, `sha256_checksum`, `last_check_error`, `last_check_time`。
    *   `VersionFile`: `version` (snake_case), `notes`。
*   **函数**:
    *   `has_newer_version(current, latest)` (L40): 语义版本号比较。
    *   `allowed_url(url)` (L50): Host 白名单校验。
    *   `check_update(app)` (Command) (L60): 入口，调用 `check_and_record`。
    *   `check_update_inner(app)` (L70): 拉取 `version.json`，获取 APK assets/digest。三态记录 checksum。
    *   `start_update_check_loop(app)` (Command) (L90): 后台循环，启动延迟 5s，之后每 24h。失败退避。
    *   `download_update(app, url)` (Command) (L110): 流式下载，互斥锁，SHA256 校验。
    *   `verify_file_sha256(path, expected)` (L130): 堆分配缓冲区防栈溢出，严格 hex 格式校验。
    *   `install_update(app, file_path)` (Command) (L150): 路径安全校验，JNI 调用安装。不立即删除 APK。
    *   `cleanup_stale_apks(app)` (L170): 清理 >24h 的 APK，跳过下载中任务。
    *   `http_client()` (L180): 构建 reqwest Client。
    *   `get_mirror_urls(github_url)` (L190): 返回镜像 URL。

## Architecture

```text
┌──────────────────────────────────────────────────────────────────────┐
│                        Frontend (WebView)                            │
└──────────────────────────┬───────────────────────────────────────────┘
                           │ [[tauri::command]]
                           ▼
┌──────────────────────────────────────────────────────────────────────┐
│                   Rust Backend Commands                              │
│                                                                      │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────┐  ┌───────────┐ │
│  │ config_state │  │ account_cmds │  │ protocol_cmds│  │ monitor_  │ │
│  │              │  │              │  │              │  │ loop      │ │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └─────┬─────┘ │
│         │                 │                  │                │       │
│  ┌──────▼───────┐  ┌──────▼───────┐  ┌───────▼───────┐  ┌─────▼─────┐ │
│  │ identity_    │  │ login_       │  │ campus_       │  │ quality_  │ │
│  │ gate         │  │ history      │  │ detect        │  │ cmds      │ │
│  └──────────────┘  └──────────────┘  └───────────────┘  └───────────┘ │
│                                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌───────────┐ │
│  │ self_service │  │ update_cmds  │  │ face_model_  │  │ battery_  │ │
│  │ _cmds        │  │              │  │ cmds         │  │ cmds      │ │
│  └──────────────┘  └──────────────┘  └──────────────┘  └───────────┘ │
│                                                                      │
│  ┌──────────────┐  ┌──────────────┐                                  │
│  │ quality_     │  │ system_cmds  │                                  │
│  │ history      │  │              │                                  │
│  └──────────────┘  └──────────────┘                                  │
└──────────────────────────┬───────────────────────────────────────────┘
                           │ JNI / Plugin Calls
                           ▼
┌──────────────────────────────────────────────────────────────────────┐
│                     Android Native (Kotlin)                          │
│  CampusNetworkBindExt  │ CampusMonitorServiceExt │ CampusKeystoreExt │
└──────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **配置生命周期**:
    *   `system_cmds::get_init_data` 调用 `config_state::current_settings` 获取掩码配置。
    *   `config_state::save_config` 持有 `CONFIG_IO_LOCK`，合并配置 -> `save_file` (加密落盘) -> `emit_config_changed`。
    *   `save_config` 成功后异步触发 `account_cmds::auto_create_account_for_current`。

2.  **账号切换流程 (`switch_account`)**:
    *   输入 `account_name` -> `validate_account_name` -> 持 `CONFIG_IO_LOCK` -> `load_account_file` (解密) -> `current_settings` (基线) -> 合并凭据/清空夜间标记 -> `persist_current` (落盘) -> 清理 `avoid_bad_wifi` 快照。

3.  **校园网探测流程 (`check_campus_status`)**:
    *   `protocol_cmds::ensure_wifi_bound` 确保 WiFi 绑定。
    *   `config_state::current_settings` 获取 Gateway/Portal/SSID 配置。
    *   `campus_detect::probe_campus_with_ssid`: 基础 TCP 探测 -> SSID 增强匹配。
    *   缓存源 IP 供后续登录使用。

4.  **后台监控循环 (`monitor_tick_loop`)**:
    *   启动时推进 `MONITOR_GENERATION`。
    *   主循环：代际校验 -> `run_scheduled_actions` (夜间切换) -> 间隔热更新 -> 分档跳拍 -> `run_check_once`。
    *   `run_check_once`: 前置闸 -> 探针窗口 -> `probe_campus` -> `portal_probe_on_little_cores` -> 三态护栏 -> `should_attempt_login` -> `run_login` -> 结果 Emit。

5.  **自动更新流程**:
    *   `start_update_check_loop` 定期调用 `check_update_inner`。
    *   若有新版本，`download_update` 流式下载 APK -> `verify_file_sha256` -> `install_update` (JNI 安装)。
    *   `cleanup_stale_apks` 定期清理残留。

## Connections

*   `config_state.rs` 被几乎所有命令模块依赖，作为配置源。
*   `monitor_loop.rs` 依赖 `protocol_cmds` (登录/注销), `campus_detect` (探测), `config_state` (配置), `login_history` (日志)。
*   `protocol_cmds.rs` 依赖 `config_state`, `monitor_loop` (状态变量), `login_history`, `CampusNetworkBindExt` (JNI)。
*   `self_service_cmds.rs` 依赖 `identity_gate` (验证门), `config_state`。
*   `update_cmds.rs` 依赖 `config_state`, `CampusMonitorServiceExt` (JNI), `infra::logger`。
*   `face_model_cmds.rs` 依赖 `config_state` (更新源), `update_cmds` (HTTP/SHA256 工具)。
*   `account_cmds.rs` 依赖 `config_state`, `login_history`。

## Known Issues

1.  **SSID 读取权限**: `campus_detect` 中的 SSID 读取需要 `NEARBY_WIFI_DEVICES` 运行时权限，且 JNI 调用必须 `spawn_blocking`，否则可能阻塞或报错。
2.  **时钟回拨保护**: `identity_gate` 中若系统时钟被手动调回过去，旧的时间戳会显得“未来”，代码强制将其视为过期。这可能导致用户频繁重新进行生物识别，但在安全上是必要的。
3.  **VPN 干扰**: 虽然 `protocol_cmds` 提供了 `ensure_wifi_bound` 和 `with_vpn_hint`，但在某些复杂的 VPN 配置下，强制绑定 WiFi 可能仍无法完全绕过 VPN 隧道对 Portal 探测的影响，需用户手动排除应用。
4.  **代际竞态**: `monitor_loop` 和 `quality_cmds` 均使用代际 Token 防止竞态。若 `stop` 和 `start` 发生极快的竞速，旧循环可能在退出前误读新循环的状态。目前实现通过递增 Generation 并在循环顶部校验来缓解，但在极端情况下仍需观察。
5.  **历史文件损坏**: `login_history` 和 `quality_history` 在读取损坏 JSON 时会备份并重置为空。虽然保证了功能可用性，但会导致部分历史数据丢失。