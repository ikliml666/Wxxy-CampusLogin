---
title: android-rust-backend
type: module
source_files:
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/cpu_affinity.rs
  - android/src-tauri/src/face_model_cmds.rs
  - android/src-tauri/src/identity_gate.rs
  - android/src-tauri/src/lib.rs
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
tags: [android, rust, tauri, backend, config, account, network, update]
---

## Overview
`android-rust-backend` 是 Android 端 Tauri 应用的核心 Rust 后端模块。它负责处理账号生命周期管理、配置状态持久化、校园网连接探测、网络质量监控、自助服务（运营商绑定/查账）、应用更新流程以及生物识别门控等关键功能。该模块通过 JNI 调用 Android 原生插件（如 `campus_network_bind`, `campus_keystore`, `campus_monitor_service`），将 Android 平台特性封装为 Tauri 命令供前端调用。

核心设计原则包括：
1. **原子性写入**：所有配置文件和历史记录均采用“写临时文件 + rename”策略，防止进程崩溃导致数据损坏。
2. **IO 串行锁**：使用全局 `tokio::sync::Mutex<()>` 保护配置读写，避免并发竞态。
3. **安全隔离**：密码字段落盘前加密，内存中明文；敏感操作需通过生物识别 TTL 门控。
4. **健壮性**：硬件差异（如 CPU 亲和性、WiFi 绑定）静默失败，不阻断主流程。

## Key Components

### 配置与状态管理 (`config_state.rs`)

**结构体与常量：**
*   `Settings` (L28): 配置模型，对齐桌面 `Config`。
    *   字段：`user`, `password` (加密存储), `operator`, `display_name`, `active_account`, `background_check_interval` (默认 60_000 ms, L155), `background_check_idle_interval` (默认 300_000 ms, L157), `latency_test_interval` (默认 600_000 ms, L170), `required_network_name` (默认 `"i-wxxy、iwxxy-2、iwxxy-3"`, L181), `config_schema_version` (默认 7, L197), `scheduled_login/logout_minutes` (默认 1440, L192-193), `campus_check_end_minutes` (默认 1380, L188) 等。
    *   序列化：`#[serde(rename_all = "camelCase", default)]`。
*   `CONFIG_IO_LOCK` (L18): 全局 `tokio::sync::Mutex<()>`，用于串行化 IO 操作。
*   `PASSWORD_MASK`: 来自 `config_state`，通常为 `"***"` (L10)。

**函数与方法：**
*   `config_io_lock()` (L22): 导出锁实例。
*   `load_file(dir)` (L250): 读取配置文件。若解密失败，密码置空而非报错 (L260)。
*   `save_file(dir, settings)` (L265): 原子保存。先写 `config.json.<pid>.tmp` (L287)，再 `rename` 为 `config.json` (L287-293)。
*   `cleanup_stale_config_tmp(dir)` (L300): 启动时清理孤儿 `.tmp` 文件。
*   `load_from(dir)` (L313): 加载配置并执行迁移。
*   `migrate_legacy_defaults(settings)` (L342-404): 根据 `config_schema_version` 执行一次性升级：
    *   v0→v1: 后台检测间隔 15s→60s。
    *   v0→v3: 开启自动化登录/检测/验证/质量跳过项；关闭质量检测。
    *   v3→v4: 补全 `background_check_idle_interval` 默认 300_000。
    *   v4→v5: `campus_check_end_minutes` 默认 0→1380。
    *   v5→v6: `latency_test_interval` 默认 60s→600s。
    *   v6→v7: 夜切开关默认 false→true；定时登录/注销禁用值 0→1440。
    *   SSID 扩容 (L319): 若 `required_network_name` 为旧单值 `"i-wxxy"`，强制扩为三值列表。
*   `get_config(app)` (L441): Tauri 命令。加载配置，更新内存缓存，返回掩码后 JSON。
*   `save_config(app, new_settings)` (L456): Tauri 命令。持 IO 锁，合并密码字段 (`resolve_password_field`)，落盘，触发自动建号 (`auto_create_account_for_current`)，广播 `config-changed` 事件。
*   `current_settings(app)` (L490): Tauri 命令。缓存优先读取。
*   `masked_for_display(settings)` (L411): 将非空密码替换为 `"***"`。
*   `resolve_password_field(incoming, settings)` (L429): 处理前端传来的空串/掩码占位符，回退已存值或清除。

### 账号管理 (`account_cmds.rs`)

**结构体与常量：**
*   `AccountItem` (L28-40):
    *   字段：`id` (文件名stem), `displayName`, `user`, `operator`, `passwordSaved` (布尔，默认 false)。
*   `AccountResult` (L44-55):
    *   字段：`success`, `message`, `active_account`, `display_name`, `config`。
    *   辅助构造器：`ok`, `ok_with_account`, `err` (L57-67)。
*   `ACCOUNT_NAME_RE` (L13-15): 正则 `^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$`。
*   `accounts_dir` (L96-102): 路径 `{app_data_dir}/accounts`。

**函数与方法：**
*   `sanitize_account_id(id)` (L72-83): 消毒账号 ID。保留字母数字、下划线、中文、连字符；非法字符替换为 `_`；截断至 32 字符。
*   `validate_account_name(name)` (L86-94): 校验长度 1-32，严格匹配 `ACCOUNT_NAME_RE`。
*   `list_accounts(app)` (L110): Tauri 命令。列举账号。
*   `list_account_items_sync(dir)` (L122-151): 同步列举。过滤隐藏文件，解析卡片信息，排序。
*   `read_account_card_info(dir, id)` (L107-117): 直接读 JSON，不解密敏感字段。
*   `switch_account(app, id)` (L182-223): Tauri 命令。切换账号。加 IO 锁，合并配置，清空恢复标记 (`night_operator_restore`, `night_outbound_restore`)，落盘主配置。
*   `save_current_as_account(app, id)` (L226-253): Tauri 命令。保存当前配置为账号。
*   `merge_save_as_target(existing, incoming, is_new)` (L298-327): 合并策略。防串号：另存为新账号时强制清空 `display_name`。
*   `create_account(app, id, user, password, operator, display_name)` (L256-290): Tauri 命令。创建新账号。幂等检查，新建 `Settings`。
*   `delete_account(app, id)` (L330-358): Tauri 命令。删除账号。若为当前激活账号，清空主配置相关字段。
*   `rename_account(app, id, new_display_name)` (L366-397): Tauri 命令。改名。校验显示名，重名检查，仅修改 `display_name`。
*   `rename_account_core(dir, id, new_display_name)` (L401-425): 核心改名逻辑。
*   `validate_display_name(name)` (L430-443): 校验显示名 Trim 后 1-32 字符，无控制字符。
*   `auto_create_account_for_current(app)` (L447-456): 由 `save_config` 成功后调用，不经命令层。
*   `auto_create_account_in(dir, settings)` (L469-506): 自动建号核心。撞库防护：若 ID 存在但 `user` 不同则跳过。

### 安卓全局状态 (`android_state.rs`)

**结构体与常量：**
*   `AndroidState` (L17-21):
    *   字段：`cached_source_ip` (`Mutex<Option<Ipv4Addr>>`), `config` (`Mutex<Option<Settings>>`)。
*   `WATCHER_FAILED` (L14): `AtomicBool`，进程级静态变量，记录 WiFi watcher 注册是否失败。

### 电池与通知设置 (`battery_cmds.rs`)

**结构体与常量：**
*   `BatteryOptimizationInfo` (L10-17):
    *   字段：`ignoring` (是否在白名单), `brand` (Build.BRAND 小写), `has_vendor_target` (是否有厂商专用页)。

**函数与方法：**
*   `get_battery_optimization_info(app)` (L20-41): Tauri 命令。查询白名单状态。
*   `request_ignore_battery_optimizations(app)` (L46-61): Tauri 命令。申请白名单，返回 `true` 表示弹窗已弹出。
*   `open_vendor_battery_settings(app)` (L65-81): Tauri 命令。跳转厂商自启/省电页。
*   `open_notification_settings(app)` (L87-101): Tauri 命令。跳转通知设置页。

### 校园网探测 (`campus_detect.rs`)

**函数与方法：**
*   `pick_campus_source_ip()` (L15-37): 优选源 IP。排除蜂窝接口，优先 `wlan0`。
*   `portal_reachable(host, port)` (L44-53): TCP 可达性检测。绑定物理网卡直连，超时 3 秒。
*   `probe_campus(gateway, portal_url)` (L65-125): 综合探测。WiFi 短路机制；子网判定 `/18`；三层判定逻辑 (`subnet_hit || gateway_ok || portal_ok`)。
*   `portal_host_of(url)` (L140-149): 从 URL 提取 Host。
*   `ssid_matches(ssid, required)` (L153-157): SSID 匹配。大小写不敏感，兼容 `i-wxxy` 变体。
*   `campus_status_message(wifi_off, on_campus)` (L129-137): 生成状态文案。
*   `probe_campus_with_ssid(gateway, portal_url, enable_network_name_check, required_network_name)` (L185-200): 增强探测。若 SSID 命中，强制 `on_campus = true`。
*   `check_campus_status(app)` (L216-238): Tauri 命令。前置 `ensure_wifi_bound`，缓存源 IP，返回 JSON。
*   `request_wifi_ssid_permission(app)` (L244-249): Tauri 命令。请求 NEARBY_WIFI_DEVICES 权限。

### 协议命令面 (`protocol_cmds.rs`)

**函数与方法：**
*   `do_login(app, user, password, operator)` (L15): Tauri 命令。回退配置凭据，确保 WiFi 绑定，调用 `run_login`，清除失败计数，记录历史。
*   `do_logout(app)` (L100): Tauri 命令。确保 WiFi 绑定，调用 `run_logout`，设置 `logout_protected_until_ms` (60s 保护期)，记录历史。
*   `check_portal_status(app)` (L150): Tauri 命令。确保 WiFi 绑定，调用 `check_portal_full`。
*   `ensure_wifi_bound(app)` (L270): 辅助函数。旁路优化：若已绑定且支持，跳过重复绑定。绑定成功后清空 HTTP 客户端池。附带调用 `accept_campus_wifi`。
*   `bind_to_wifi()` (L183): JNI 调用。
*   `accept_wifi_network()` (L201): JNI 调用。
*   `get_avoid_bad_wifi_status()` (L220): JNI 调用。
*   `restore_written_settings()` (L240): JNI 调用。

### 质量监控 (`quality_cmds.rs` & `quality_history.rs`)

**`quality_cmds.rs`:**
*   `LATENCY_RUNNING` (L12): `AtomicBool`。
*   `LATENCY_GENERATION` (L17): `AtomicU64`。
*   `GOOD_LEVELS` (L89): `["excellent", "great", "good"]`。
*   `BACKOFF_STABLE_STREAK` (L90): 5。
*   `BACKOFF_MAX_MS` (L91): 1_800_000。
*   `check_network_quality(app)` (L52): Tauri 命令。单次质量检查，记录历史。
*   `start_latency_test(app)` (L57): Tauri 命令。启动后台循环。幂等检查，生成代际 Token。
*   `stop_latency_test()` (L77): Tauri 命令。停止循环。原子递增 Generation。
*   `run_quality_once(app)` (L19): 核心执行。获取设置和缓存 IP，调用 `check_network_quality_async`，追加历史。
*   `latency_loop(app, settings, gen)` (L103): 稳态退避循环。检查 Generation 匹配性，执行检测，指数退避。
*   `next_backoff_interval_ms(current_ms, stable_good_streak, base_ms)` (L95): 纯函数。每 5 拍翻倍，上限 1.8M ms。

**`quality_history.rs`:**
*   `QUALITY_HISTORY_MAX` (L12): 100。
*   `QUALITY_HISTORY_LOCK` (L10): `Mutex<()>`。
*   `QualityHistoryEntry` (L16): 结构体，字段 `timestamp`, `gateway_latency`, `external_latency`, `quality`。
*   `read(dir)` (L29): 读取历史。损坏时备份重置。
*   `append(dir, result)` (L52): 追加一条。头插法，截断，原子写。
*   `history_path(dir)` (L24): 拼接路径。
*   `epoch_millis()` (L44): 获取毫秒时间戳。

### 自助服务 (`self_service_cmds.rs`)

**结构体与常量：**
*   `CommandResult` (L8): `{success, message?, data?}`。辅助方法 `ok_msg` (L17), `err` (L20)。

**函数与方法：**
*   `resolve_self_password(incoming, settings)` (L27): 密码解析。若 incoming 非空且非 MASK 则直接用，否则回退设置，皆无则 None。
*   `ensure_identity_gate(settings)` (L39): 验证门检查。若 `self_hello_enabled` 为 false 放行，否则检查 `identity_gate`。
*   `verify_biometric_identity(success, method, _consent_message)` (L67): Tauri 命令。接收前端生物识别结果。仅当 `success == true` 时调用 `note_identity_verified`。
*   `load_settings(app)` (L50): 异步加载配置。
*   `local_addr_of(state)` (L54): 提取源 IP。
*   `bind_operator(...)` (L85): Tauri 命令。绑定运营商。校验学号、验证门、密码、手机号、短信密码。
*   `query_bind_status(...)` (L130): Tauri 命令。查询绑定状态。格式化数据。
*   `query_self_dashboard(...)` (L162): Tauri 命令。查询仪表盘。
*   `is_iso_date(s)` (L187): 严格 YYYY-MM-DD 校验。
*   `query_self_online_log(...)` (L202): Tauri 命令。查询在线日志。校验日期格式和顺序。
*   `self_offline_session(...)` (L231): Tauri 命令。注销会话。需验证门。
*   `change_self_password(...)` (L263): Tauri 命令。修改密码。需验证门。新密码 6-11 位 ASCII。
*   `reveal_operator_credential(...)` (L301): Tauri 命令。揭示凭证。**强制验证门**。

### 系统命令 (`system_cmds.rs`)

**函数与方法：**
*   `get_init_data(app)` (L6): Tauri 命令。聚合配置、账号列表、版本、状态等。
*   `get_logs(app, lines)` (L32): Tauri 命令。异步阻塞读取最近 N 行日志。
*   `clear_logs(app)` (L42): Tauri 命令。清理日志。
*   `get_log_retention_days()` (L49) / `set_log_retention_days(days)` (L54): 日志保留天数读写。
*   `get_debug_mode()` (L59) / `set_debug_mode(enabled)` (L64): 调试模式读写。

### 应用更新 (`update_cmds.rs`)

**结构体与常量：**
*   `UpdateInfo` (L82): `{has_update, latest_version, release_notes, assets, sha256_checksum, last_check_error, last_check_time}`。
*   `ReleaseAsset` (L74): `{name, url, size}`。
*   `DownloadProgress` (L97): `{downloaded, total, speed, percent}`。
*   `VersionFile` (L224): `{version, notes}` (snake_case)。
*   `UPDATE_LOOP_RUNNING` (L16): `AtomicBool`。
*   `UPDATE_CHECKSUM` (L21): `Mutex<Option<(String, String)>>`。
*   `LAST_CHECK_ERROR` (L35): `Mutex<Option<String>>`。
*   `LAST_CHECK_TIME_MS` (L37): `AtomicU64`。
*   `DOWNLOAD_RUNNING` (L398): `AtomicBool`。
*   `VERSION_FILE` (L24): GitHub raw URL。
*   `AUTO_CHECK_INTERVAL_SECS` (L26): 24h。
*   `BACKOFF_RETRY_SECS` (L29): `[300, 900, 3600]`。
*   `VERSION_MIRRORS` (L39): 三个代理地址。
*   `DOWNLOAD_ALLOWED_HOSTS` (L46): 白名单域名。
*   `MAX_DOWNLOAD_BYTES` (L57): 500MB。
*   `APK_STALE_SECS` (L62): 24h。

**函数与方法：**
*   `check_update(app)` (L164): Tauri 命令。入口，调用 `check_and_record`。
*   `check_and_record(app)` (L198): 内部检查，记录错误和时间。
*   `check_update_inner(app)` (L282): 核心检查。按 `update_source` 决定镜像优先级，遍历 URL，解析 `VersionFile`，获取资产。
*   `fetch_apk_assets(client, latest)` (L237): 请求 GitHub API 获取 assets 和 digest。超时 10s。
*   `has_newer_version(current, latest)` (L105): 语义化版本比较。处理短版本补齐和前缀 'v'。
*   `http_client()` (L127): 构建 reqwest Client。
*   `allowed_url(url)` (L139): 域名白名单校验。
*   `download_update(app, url)` (L401): Tauri 命令。下载入口。互斥锁检查。
*   `download_update_inner(app, url)` (L410): 流式下载。校验 URL，解析文件名，创建文件，循环读取 chunk，实时进度 emit，大小上限检查，SHA256 校验。
*   `verify_file_sha256(path, expected)` (L517): 异步 SHA256 校验。堆分配 buffer，严格 hex 格式校验。
*   `install_update(app, file_path)` (L549): Tauri 命令。安装 APK。路径必须在 `update_dir` 内。调用 `CampusMonitorServiceExt::install_apk`。
*   `cleanup_stale_apks(app)` (L576): 清理超过 24h 的 APK。若 `DOWNLOAD_RUNNING` 为真则跳过。
*   `start_update_check_loop(app)` (L340): 自动检查循环。首次延迟 5s，之后每 24h。失败退避。有新版本 emit 事件并发送通知。
*   `get_mirror_urls(github_url)` (L385): 返回镜像 URL 列表。
*   `download_tag(url)` (L506): 从 URL 提取 release tag。
*   `update_dir(app)` (L66): 获取更新目录 `app_data_dir/files/update`。

### 其他辅助模块

**`cpu_affinity.rs`:**
*   `little_core_ids` (L8): 解析 `/sys/devices/system/cpu/cpu*/cpufreq/cpuinfo_max_freq`，过滤频率缺失，若同构返回空。
*   `pin_current_thread_to_little_cores` (L34): 绑定线程到小核。Android 端调用 `libc::sched_setaffinity`，Host 端返回 false。静默失败。

**`face_model_cmds.rs`:**
*   `FACE_MODEL_FILES` (L26): 硬编码 SHA256。
*   `FACE_MODEL_MIRRORS` (L20): 代理镜像列表。
*   `face_models_state` (L121): 检查文件存在性及 SHA256。
*   `face_models_download` (L126): Tauri 命令。互斥锁 `FACE_DL_RUNNING`。遍历文件下载，流式下载至 `.part`，成功后 rename，发射进度事件。
*   `download_one` (L187): 单个文件下载。
*   `models_dir` (L86): 路径 `app_data_dir/files/face-models`。

**`identity_gate.rs`:**
*   `LAST_VERIFY_EPOCH_SECS` (L6): 最近一次验证成功的时间戳。
*   `IDENTITY_VERIFY_TTL_SECS` (L8): 600 (10分钟)。
*   `note_identity_verified` (L18): 更新当前时间戳。
*   `identity_verified_recently` (L23): 检查 TTL。时钟回拨守卫：若 `now < last` 视为过期。

**`login_history.rs`:**
*   `LOGIN_HISTORY_LOCK` (L10): `Mutex<()>`。
*   `LoginHistoryEntry` (L13): 结构体，字段 `timestamp`, `success`, `message`, `adapter`, `user`, `type`。
*   `read(dir)` (L25): 读取历史。损坏时备份重置。
*   `append(dir, entry)` (L45): 追加一条。头插法，上限 100 条，原子写。

**`lib.rs`:**
*   `run` (L31): 入口点。构建 Tauri 应用。
*   `setup` (L43): 初始化。注册插件，注入 `AndroidState`，初始化日志目录，启动后台监控循环，清理陈旧配置临时文件。
*   `invoke_handler` (L66-122): 注册所有子模块的命令。
*   `ExitRequested` 拦截 (L130): 调用 `api.prevent_exit()`，确保前台服务常驻。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                         android-rust-backend                                │
│                                                                             │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐   ┌──────────────┐ │
│  │ Config State │   │ Account Cmds │   │ Protocol Cmds│   │ Self Service │ │
│  │              │   │              │   │              │   │              │ │
│  │ • Settings   │   │ • CRUD       │   │ • Login      │   │ • Bind       │ │
│  │ • Migration  │   │ • Merge      │   │ • Logout     │   │ • Query      │ │
│  │ • Crypto     │   │ • AutoCreate │   │ • Portal     │   │ • ChangePwd  │ │
│  └──────┬───────┘   └──────┬───────┘   └──────┬───────┘   └──────┬───────┘ │
│         │                  │                  │                  │          │
│         ▼                  ▼                  ▼                  ▼          │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                    Shared Infrastructure                             │  │
│  │                                                                      │  │
│  │  • CONFIG_IO_LOCK (Tokio Mutex)                                      │  │
│  │  • AndroidState (Cached IP, Config)                                  │  │
│  │  • IdentityGate (Biometric TTL)                                      │  │
│  │  • LoginHistory / QualityHistory                                     │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                              ▲                                             │
│                              │                                              │
│  ┌──────────────┐   ┌──────┴───────┐   ┌──────────────┐   ┌──────────────┐ │
│  │ Campus Detect│   │ Quality Cmds │   │ Update Cmds  │   │ System Cmds  │ │
│  │              │   │              │   │              │   │              │ │
│  │ • Probe      │   │ • Latency    │   │ • Check      │   │ • InitData   │ │
│  │ • SSID Match │   │ • Backoff    │   │ • Download   │   │ • Logs       │ │
│  └──────────────┘   └──────────────┘   └──────────────┘   └──────────────┘ │
│                                                                             │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐                   │
│  │ Face Models  │   │ Battery Cmds │   │ CPU Affinity │                   │
│  │              │   │              │   │              │                   │
│  │ • Download   │   │ • Whitelist  │   │ • Pin Core   │                   │
│  │ • Verify SHA │   │ • Settings   │   │              │                   │
│  └──────────────┘   └──────────────┘   └──────────────┘                   │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **配置加载与初始化**:
    *   `lib.rs::setup` 调用 `config_state::cleanup_stale_config_tmp` 清理临时文件。
    *   `config_state::load_from` 读取 `config.json`，执行 `migrate_legacy_defaults` 进行版本迁移，更新 `AndroidState.config` 缓存。
    *   `system_cmds::get_init_data` 聚合当前配置（掩码后）、账号列表、版本号等信息返回前端。

2.  **账号切换流程**:
    *   前端调用 `account_cmds::switch_account`。
    *   获取 `CONFIG_IO_LOCK`。
    *   加载目标账号 JSON，合并到 `Settings`。
    *   清空 `night_operator_restore` 和 `night_outbound_restore`。
    *   调用 `config_state::persist_current` 落盘主配置，触发 `auto_create_account_for_current`。
    *   调用 `monitor_loop::restore_avoid_bad_wifi` 清理网络快照。
    *   广播 `config-changed` 事件。

3.  **网络连接与登录流程**:
    *   前端调用 `protocol_cmds::do_login`。
    *   调用 `ensure_wifi_bound` 绑定 WiFi 进程，清空 HTTP 客户端池。
    *   调用 `campus_login_lib::network::login` 执行登录。
    *   成功后清除连续失败计数，调用 `login_history::append` 记录手动登录。
    *   若自动检测触发，`monitor_loop` 也会调用登录并记录历史。

4.  **网络质量监控流程**:
    *   前端调用 `quality_cmds::start_latency_test`。
    *   生成 `LATENCY_GENERATION` token，启动 `latency_loop`。
    *   循环内调用 `run_quality_once`，获取 `cached_source_ip`，执行 `check_network_quality_async`。
    *   结果追加到 `quality_history::append`，头插法，上限 100 条。
    *   根据 `GOOD_LEVELS` 进行指数退避，最大间隔 30 分钟。

5.  **应用更新流程**:
    *   前端调用 `update_cmds::check_update`。
    *   `check_update_inner` 按 `update_source` 配置选择镜像源，请求 `VersionFile` 和 `ReleaseAssets`。
    *   若发现新版本，emit `update-available` 事件并发送系统通知。
    *   前端调用 `download_update`，互斥锁保护，流式下载 APK 至 `update_dir`。
    *   下载完成后进行 `verify_file_sha256` 校验。
    *   调用 `install_update` 触发系统安装器，不立即删除 APK，由 `cleanup_stale_apks` 定期清理。

6.  **自助服务与安全门控**:
    *   前端发起敏感操作（如改密、查账）。
    *   `self_service_cmds` 调用 `ensure_identity_gate` 检查 `identity_gate::identity_verified_recently`。
    *   若未通过，要求前端调用 `verify_biometric_identity`。
    *   前端验证成功后，Rust 端调用 `identity_gate::note_identity_verified` 更新 TTL。
    *   后续操作继续执行，密码通过 `resolve_self_password` 解析。

## Connections

*   [[config-state]]: 配置状态管理，被几乎所有命令依赖。
*   [[account-management]]: 账号生命周期管理，与配置状态紧密交互。
*   [[network-monitoring]]: 网络监控主循环，依赖配置和账号状态。
*   [[biometric-auth]]: 生物识别认证，通过 identity_gate 提供 TTL 门控。
*   [[update-system]]: 应用更新系统，独立于业务逻辑，但依赖配置中的更新源设置。
*   [[quality-history]]: 质量历史记录，被 quality_cmds 写入。
*   [[login-history]]: 登录历史记录，被 protocol_cmds 和 monitor_loop 写入。
*   [[campus-detect]]: 校园网探测，被 campus_detect 命令和 monitor_loop 使用。
*   [[face-models]]: 人脸识别模型管理，独立模块，按需下载。
*   [[battery-settings]]: 电池优化设置，辅助模块，提升应用存活率。
*   [[cpu-affinity]]: CPU 亲和性设置，辅助模块，优化性能。