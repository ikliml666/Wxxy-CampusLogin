---
title: rust-backend-core
type: module
source_files:
  - android/src-tauri/src/account_cmds.rs
  - android/src-tauri/src/android_state.rs
  - android/src-tauri/src/battery_cmds.rs
  - android/src-tauri/src/campus_detect.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/cpu_affinity.rs
tags: [Tauri, Rust, Android, Backend, Config, Account, Network]
---

## Overview
`rust-backend-core` 是 Tauri Android 应用的 Rust 后端核心模块。它负责管理应用的全局配置、多账号体系、校园网状态检测、电池优化跳转以及底层 CPU 亲和性绑定。该模块通过 `lib.rs` 暴露命令面，与前端交互，并直接操作 Android 系统层（KeyStore、WifiManager、Sysfs）。核心设计原则包括：密码落盘加密、配置 IO 互斥锁防止竞态、自动建号幂等性以及严格的账号 ID 消毒。

## Key Components

### `config_state.rs`: 配置状态与持久化
管理全局设置 (`Settings`) 的加载、保存、迁移和内存缓存。

*   **Structs & Enums**
    *   `Settings` (L19-L138): 全量配置结构体。
        *   `user`, `password`, `operator`: 账号凭据。
        *   `self_hello_enabled`, `self_reverify_each_action`, `allow_2d_face_verify`: 验证开关。
        *   `enable_night_operator_switch`, `night_operator_restore`: 夜间运营商切换相关。
        *   `enable_night_outbound_switch`, `night_outbound_restore`: 夜间出站切换相关。
        *   `outbound_priority`, `outbound_metric_restore`, `outbound_disabled_adapters`, `outbound_standby_route`: 桌面端出站切换遗留字段，安卓端不消费但保留结构。
        *   `dns_optimize_adapters`: DNS 优化适配器名单。
        *   `device_aliases`: 在线设备自定义名映射。
        *   `auto_login_on_start`, `auto_login_on_preparation`: 自动登录开关。
        *   `enable_background_check`, `background_check_interval`, `background_check_idle_interval`: 后台检测间隔。
        *   `theme_mode`, `enable_notification`, `custom_theme_color`, `default_panel`: UI/通知设置。
        *   `display_name`, `active_account`: 显示名与当前激活账号 ID。
        *   `enable_boot_autostart`: 开机自启。
        *   `enable_latency_test`, `latency_test_interval`, `skip_ttfb_in_latency`, `skip_content_in_latency`: 延迟测试配置。
        *   `enable_network_quality`: 网络质量检测开关。
        *   `portal_url`, `fixed_gateway`, `required_network_name`, `enable_network_name_check`, `campus_gateway`: 网络检测参数。
        *   `campus_check_start_minutes`, `campus_check_end_minutes`: 检测时段。
        *   `scheduled_login_minutes`, `scheduled_logout_minutes`: 定时登录/注销。
        *   `update_source`: 更新渠道。
        *   `log_retention_days`: 日志保留天数。
        *   `config_schema_version`: 配置版本 (默认 7)。
    *   `CryptoBridge` (L140-L163): 加解密桥接器。
        *   `encrypt`: Arc<dyn Fn(&str) -> Result<String, String>>。
        *   `decrypt`: Arc<dyn Fn(&str) -> Result<String, String>>。
    *   `EncodedSettings` (L165-L169): 磁盘存储格式。
        *   `settings`: Settings 副本。
        *   `password_cipher`: 加密后的密码字符串。

*   **Functions & Commands**
    *   `config_io_lock()` (L28-L31): 获取全局配置 IO 互斥锁。
    *   `load_file(path, bridge)` (L171-L183): 读取 JSON 文件，解密密码。失败返回默认值或错误。
    *   `save_file(path, bridge, s)` (L185-L213): 加密密码，原子写入 (tmp + rename)。
    *   `cleanup_stale_config_tmp(dir)` (L215-L228): 清理启动时遗留的 `.tmp` 配置文件。
    *   `load_from(dir, bridge)` (L230-L244): 加载主配置并执行版本迁移。
    *   `migrate_legacy_defaults(dir, bridge, s)` (L246-L306): 根据 `config_schema_version` 执行一次性默认值迁移 (v0-v7)。
    *   `save_to(dir, bridge, s)` (L308-L310): 快捷调用 `save_file` 保存主配置。
    *   `masked_for_display(s)` (L312-L317): 将密码替换为 `***` 用于序列化输出。
    *   `emit_config_changed(app, s)` (L319-L323): 广播 `config-changed` 事件给前端。
    *   `resolve_password_field(incoming, current, clear)` (L325-L331): 处理密码字段的回退逻辑 (空串/掩码占位符保留旧值)。
    *   `get_config(app)` (L333-L343): [Command] 获取当前配置，更新内存缓存。
    *   `save_config(app, config, clear_password)` (L345-L370): [Command] 保存配置。持锁，合并密码，触发自动建号，广播事件。
    *   `current_settings(app)` (L372-L383): 优先从内存缓存读取，未命中则读盘。

### `account_cmds.rs`: 多账号管理
实现账号的创建、切换、删除、重命名和列表列举。

*   **Structs & Enums**
    *   `AccountItem` (L24-L28): 账号列表条目。
        *   `id`: 账号文件名 stem。
        *   `displayName`: 可读显示名。
    *   `AccountResult` (L30-L40): 账号操作返回结果。
        *   `success`: bool。
        *   `message`: Option<String>。
        *   `active_account`: Option<String>。
        *   `display_name`: Option<String>。
        *   `config`: Option<serde_json::Value>。

*   **Functions & Commands**
    *   `sanitize_account_id(user)` (L63-L73): 账号 ID 消毒，非法字符替换为 `_`，截断至 32 字符。
    *   `validate_account_name(name)` (L75-L82): 校验账号名称 (字母/数字/下划线/中文/连字符，1-32 字符)。
    *   `accounts_dir(app)` (L84-L88): 获取账号目录路径。
    *   `read_display_name(path)` (L90-L97): 从 JSON 读取 `displayName`，失败兜底。
    *   `list_account_items_sync(dir)` (L99-L123): 同步列举账号，过滤隐藏文件，排序，空显示名兜底为 ID。
    *   `list_accounts(app)` (L125-L129): [Command] 异步列举账号。
    *   `switch_account(app, account_name)` (L147-L189): [Command] 切换账号。合并登录字段，清空夜切恢复标记，收尾 `avoid_bad_wifi` 快照。
    *   `save_current_as_account(app, account_name)` (L191-L218): [Command] 另存当前配置为账号。处理覆盖写逻辑，防止显示名串号。
    *   `merge_save_as_target(current, existing, safe_name)` (L220-L251): 纯函数，决定另存时的合并规则。
    *   `delete_account(app, account_name)` (L253-L282): [Command] 删除账号。若为当前账号，清空 `active_account` 和夜切标记。
    *   `get_active_account(app)` (L284-L286): [Command] 获取当前激活账号 ID。
    *   `rename_account(app, account_id, display_name)` (L288-L313): [Command] 重命名账号。校验显示名，检查重名，更新档案及主配置。
    *   `rename_account_core(dir, bridge, account_id, display_name)` (L315-L343): 改名核心逻辑，不含 AppHandle 依赖。
    *   `validate_display_name(name)` (L345-L358): 校验显示名 (trim 后 1-32 字符，无控制字符)。
    *   `auto_create_account_for_current(app, current)` (L360-L370): R2 自动建号入口，`save_config` 成功后调用。
    *   `auto_create_account_in(dir, bridge, config)` (L372-L408): 自动建号核心。处理撞库防护、幂等短路、差异更新。

### `android_state.rs`: 安卓全局状态
维护进程级静态标志和内存中的配置/IP 缓存。

*   **Structs & Enums**
    *   `WATCHER_FAILED` (L13): `AtomicBool`，WiFi watcher 注册失败降级标志。
    *   `AndroidState` (L17-L23): Tauri State 管理。
        *   `cached_source_ip`: `Mutex<Option<Ipv4Addr>>`，源 IP 缓存。
        *   `config`: `Mutex<Option<Settings>>`，明文配置内存态。

### `campus_detect.rs`: 校园网探针
实现 IPv4 枚举、子网判定、Portal TCP 可达性检测和 SSID 匹配。

*   **Structs & Enums**
    *   `CampusProbe` (L65-L71): 探测结果。
        *   `source`: Option<Ipv4Addr>。
        *   `portal_ok`: bool。
        *   `on_campus`: bool。
        *   `wifi_off`: bool。

*   **Functions & Commands**
    *   `pick_campus_source_ip(interfaces)` (L18-L36): 选择源 IP，优先 wlan0，排除蜂窝接口。
    *   `portal_reachable(host, port, timeout)` (L38-L46): TCP 连接探测替代 ICMP。
    *   `probe_campus(campus_gateway, portal_url)` (L48-L93): 完整探测流程。包含 WiFi 短路、子网判定、并行网关/Portal 探测。
    *   `campus_status_message(wifi_off, on_campus)` (L95-L102): 生成状态文案 ("WiFi 未连接", "已连接校园网", "未连接校园网")。
    *   `portal_host_of(url)` (L104-L113): 提取 URL 中的主机名。
    *   `ssid_matches(ssid, required)` (L115-L122): SSID 匹配，支持历史同源特判。
    *   `current_wifi_ssid(app)` (L124-L137): 获取当前 WiFi SSID (JNI 阻塞调用)。
    *   `probe_campus_with_ssid(app, settings)` (L139-L153): 带 SSID 感知的探测增强版。
    *   `cache_source_ip(state, source)` (L155-L159): 缓存源 IP。
    *   `check_campus_status(app, state)` (L161-L183): [Command] 触发探测并返回 JSON 状态。
    *   `request_wifi_ssid_permission(app)` (L185-L190): [Command] 请求 WiFi SSID 权限。

### `battery_cmds.rs`: 电池优化命令
提供电池优化白名单查询/申请及厂商设置页跳转。

*   **Structs & Enums**
    *   `BatteryOptimizationInfo` (L11-L16): 电池优化信息。
        *   `ignoring`: bool。
        *   `brand`: String。
        *   `has_vendor_target`: bool。

*   **Commands**
    *   `get_battery_optimization_info(app)` (L18-L36): [Command] 获取优化信息。
    *   `request_ignore_battery_optimizations(app)` (L38-L50): [Command] 申请忽略优化。
    *   `open_vendor_battery_settings(app)` (L52-L63): [Command] 打开厂商自启页。
    *   `open_notification_settings(app)` (L65-L79): [Command] 打开通知设置页。

### `cpu_affinity.rs`: CPU 亲和性
识别大小核拓扑并将线程绑定到小核以优化功耗。

*   **Functions**
    *   `little_core_ids()` (L13-L32): 解析 `/sys/devices/system/cpu/cpu*/cpufreq/cpuinfo_max_freq`，返回最低频簇 ID。
    *   `pin_current_thread_to_little_cores()` (L34-L48): 使用 `sched_setaffinity` 绑定当前线程。仅安卓生效。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Tauri Frontend                                 │
└──────────────────────────┬──────────────────────────────────────────────────┘
                           │ IPC Calls / Events
                           ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                            lib.rs (Entry Point)                             │
│  Registers Commands: get_config, save_config, list_accounts, switch...      │
│  Manages State: AndroidState                                                  │
└──────────────────┬──────────────────────┬──────────────────────┬────────────┘
                   │                      │                      │
                   ▼                      ▼                      ▼
┌─────────────────────────────┐ ┌────────────────────────┐ ┌────────────────────────────┐
│     config_state.rs         │ │     account_cmds.rs    │ │    campus_detect.rs        │
│ - Settings Model            │ │ - sanitize_account_id  │ │ - pick_campus_source_ip    │
│ - CryptoBridge (Keystore)   │ │ - switch_account       │ │ - probe_campus             │
│ - load/save/migrate         │ │ - auto_create_account  │ │ - ssid_matches             │
│ - CONFIG_IO_LOCK            │ │ - rename_account       │ │ - check_campus_status      │
└─────────────────────────────┘ └────────────────────────┘ └────────────────────────────┘
                   │                      │                      │
                   ▼                      ▼                      ▼
┌─────────────────────────────┐ ┌────────────────────────┐ ┌────────────────────────────┐
│    android_state.rs         │ │     battery_cmds.rs    │ │    cpu_affinity.rs         │
│ - WATCHER_FAILED (Static)   │ │ - get_battery_info     │ │ - little_core_ids          │
│ - cached_source_ip          │ │ - open_vendor_settings │ │ - pin_current_thread       │
│ - config (Memory Cache)     │ │                        │ │                          │
└─────────────────────────────┘ └────────────────────────┘ └────────────────────────────┘
```

## Data Flow

### 配置保存与自动建号流程
1.  前端调用 `save_config`。
2.  `config_state::save_config` 获取 `CONFIG_IO_LOCK`。
3.  加载现有配置，合并新配置，处理密码字段 (`resolve_password_field`)。
4.  调用 `save_file` 进行原子写入 (加密密码，tmp + rename)。
5.  更新 `AndroidState.config` 内存缓存。
6.  调用 `account_cmds::auto_create_account_for_current`：
    *   检查 user/password 非空。
    *   生成 sanitized ID。
    *   若文件不存在，创建新账号文件 (display_name = 原始 user)。
    *   若存在且 user 不同，跳过 (撞库保护)。
    *   若存在且 user 相同，比对 password/operator，有差异则更新，一致则幂等短路。
7.  广播 `config-changed` 事件。

### 校园网探测流程
1.  前端调用 `check_campus_status`。
2.  `protocol_cmds::ensure_wifi_bound` 确保 WiFi 绑定。
3.  获取当前配置。
4.  调用 `probe_campus_with_ssid`：
    *   基础探测 `probe_campus`：枚举网卡，排除蜂窝，选择源 IP。
    *   WiFi 短路：若无有效 IP，返回 `wifi_off=true`。
    *   子网判定：检查源 IP 与 `campus_gateway` 是否同 /18 子网。
    *   并行探测：若非子网命中，并行探测 Portal TCP 和网关 TCP。
    *   SSID 增强：若开启名称检查，获取 SSID 并匹配 `required_network_name`。
5.  生成 `CampusProbe` 结果，转换为 JSON 返回。
6.  缓存源 IP 到 `AndroidState.cached_source_ip`。

## Connections
*   [[lib.rs]]: 模块入口，注册所有命令。
*   [[monitor_loop.rs]]: 消费 `config_state` 中的定时任务配置，调用 `campus_detect` 进行周期检测。
*   [[identity_gate.rs]]: 可能依赖 `account_cmds` 获取当前账号凭据进行验证。
*   [[face_model_cmds.rs]]: 依赖 `config_state` 获取 `allow_2d_face_verify` 配置。
*   [[system_cmds.rs]]: 可能调用 `cpu_affinity` 进行性能调优或状态查询。
*   [[update_cmds.rs]]: 独立模块，依赖 `config_state` 获取 `update_source`。