---
title: Config 模型与持久化机制
type: learning
source_files:
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/persist.rs
tags: [配置, serde, 兼容性, 安全, 原子写入]
---

## Overview

本文档记录了 `Config` 模块的设计哲学、数据结构定义及持久化机制。核心目标是解决**旧版本配置文件因字段缺失导致反序列化失败**的问题，同时确保敏感信息（密码）的安全存储以及配置文件写入的原子性。

该模块由两部分组成：
1. **模型层 (`model.rs`)**：定义 `Config` 结构体，通过容器级 `#[serde(default)]` 实现向后兼容，并定义了脱敏显示逻辑。
2. **持久化层 (`persist.rs`)**：负责配置的读写、加密/解密、原子写入以及登录/质量历史的维护。

## Key Components

### tauri-app/src-tauri/src/config/model.rs

#### 常量
- `PASSWORD_MASK`: `"***"` (L3) - 密码脱敏占位符。
- `AUTO_DETECT_ADAPTER`: `"自动检测"` (L4) - 适配器自动检测的显示名。

#### 结构体: Config
- **定义**: `pub struct Config` (L10-L182)
- **容器级默认值**: `#[serde(default)]` (L9)
    - **作用**: 允许旧版本配置文件或缺失字段的 JSON 在反序列化时自动使用 `Default::default()` 填充缺失字段，防止因单个字段缺失导致整个配置加载失败（见 L6-L9 注释）。
- **字段列表**（全部字段，含重命名与默认值）：
  - `user: String` (L11): 用户名。
  - `password: String` (L13，字段级 `#[serde(default)]`): 密码。
  - `self_hello_enabled: bool` (L17-L18，重命名 `selfHelloEnabled`，默认 `default_true`): 是否启用 Windows Hello 操作验证。
  - `self_reverify_each_action: bool` (L20-L21，重命名 `selfReverifyEachAction`，默认 `false`): 自助服务面板每次操作二次验证。
  - `operator: String` (L22): 运营商名称。
  - `adapter1: String` (L23): 主适配器名称。
  - `adapter2: String` (L24): 副适配器名称。
  - `adapter1_account: String` (L27-L28，重命名 `adapter1Account`): 主适配器绑定的账号 ID。
  - `adapter2_account: String` (L30-L31，重命名 `adapter2Account`): 副适配器绑定的账号 ID。
  - `dual_adapter: bool` (L32-L33，重命名 `dualAdapter`): 双适配器模式。
  - `auto_login_on_start: bool` (L34-L35，重命名 `autoLoginOnStart`): 启动时自动登录。
  - `auto_exit_after_login: bool` (L36-L37，重命名 `autoExitAfterLogin`): 登录后自动退出。
  - `minimize_to_tray: bool` (L38-L39，重命名 `minimizeToTray`): 最小化到托盘。
  - `lightweight_mode: bool` (L42-L43，重命名 `lightweightMode`，默认 `default_true`): 轻量化模式。
  - `hidden_start: bool` (L44-L45，重命名 `hiddenStart`): 隐藏启动。
  - `auto_launch: bool` (L46-L47，重命名 `autoLaunch`): 开机自启。
  - `enable_background_check: bool` (L48-L49，重命名 `enableBackgroundCheck`): 启用后台检查。
  - `background_check_interval: u64` (L50-L51，重命名 `backgroundCheckInterval`): 后台检查间隔（毫秒）。
  - `auto_login_on_preparation: bool` (L52-L53，重命名 `autoLoginOnPreparation`): 准备阶段自动登录。
  - `enable_night_operator_switch: bool` (L56-L57，重命名 `enableNightOperatorSwitch`，默认 `false`): 夜间自动切换运营商开关。
  - `night_operator_restore: String` (L59-L60，重命名 `nightOperatorRestore`，默认空): 夜间切换前暂存的运营商。
  - `enable_night_outbound_switch: bool` (L63-L64，重命名 `enableNightOutboundSwitch`，默认 `false`): 夜间出站自动切换开关。
  - `outbound_priority: Vec<String>` (L69-L70，重命名 `outboundPriority`，默认空): 出站网卡优先级列表。
  - `outbound_metric_restore: String` (L74-L75，重命名 `outboundMetricRestore`，默认空): 切换态快照（JSON 字符串）。
  - `outbound_disabled_adapters: String` (L80-L81，重命名 `outboundDisabledAdapters`，默认空): 夜间切换禁用的校园网卡快照（JSON 字符串）。
  - `outbound_standby_route: String` (L85-L86，重命名 `outboundStandbyRoute`，默认空): 夜间切换新增的兜底默认路由快照（JSON 字符串）。
  - `outbound_manual_hold_day: i32` (L92-L93，重命名 `outboundManualHoldDay`，默认 0): "立即切换"当日保持标记。
  - `outbound_pending_switch: String` (L98-L99，重命名 `outboundPendingSwitch`，默认空): 待切换状态 JSON。
  - `night_outbound_restore: String` (L102-L103，重命名 `nightOutboundRestore`，默认空): 安卓切换态标记。
  - `dns_optimize_adapters: Vec<String>` (L107-L108，重命名 `dnsOptimizeAdapters`，默认空): DNS 优化目标适配器名单。
  - `device_aliases: HashMap<String, String>` (L112-L113，重命名 `deviceAliases`，默认空): 在线设备自定义名字映射。
  - `auto_exit_on_online: bool` (L114-L115，重命名 `autoExitOnOnline`): 在线后自动退出。
  - `theme_mode: String` (L116-L117，重命名 `themeMode`): 主题模式。
  - `enable_notification: bool` (L118-L119，重命名 `enableNotification`): 启用通知。
  - `active_account: String` (L120-L121，重命名 `activeAccount`): 当前激活账号 ID。
  - `display_name: String` (L124-L125，重命名 `displayName`): 本文件所属账号的显示名。
  - `enable_latency_test: bool` (L126-L127，重命名 `enableLatencyTest`): 启用延迟测试。
  - `latency_test_interval: u64` (L128-L129，重命名 `latencyTestInterval`): 延迟测试间隔（毫秒）。
  - `custom_theme_color: String` (L130-L131，重命名 `customThemeColor`): 自定义主题颜色。
  - `default_panel: String` (L132-L133，重命名 `defaultPanel`): 默认面板。
  - `enable_network_quality: bool` (L134-L135，重命名 `enableNetworkQuality`): 启用网络质量检测。
  - `skip_ttfb_in_latency: bool` (L136-L137，重命名 `skipTtfbInLatency`，默认 `default_true`): 延迟测试中跳过 TTFB。
  - `skip_content_in_latency: bool` (L138-L139，重命名 `skipContentInLatency`，默认 `default_true`): 延迟测试中跳过内容下载。
  - `portal_url: String` (L140-L141，重命名 `portalUrl`，默认 `default_portal_url`): Portal URL。
  - `fixed_gateway: String` (L142-L143，重命名 `fixedGateway`，默认 `default_fixed_gateway`): 固定网关。
  - `required_network_name: String` (L144-L145，重命名 `requiredNetworkName`，默认 `default_required_network_name`，自定义反序列化): 必需的网络名称。
  - `enable_network_name_check: bool` (L146-L147，重命名 `enableNetworkNameCheck`，默认 `default_true`): 启用网络名称检查。
  - `campus_gateway: String` (L148-L149，重命名 `campusGateway`，默认 `default_campus_gateway`，自定义反序列化): 校园网网关。
  - `update_source: String` (L151-L152，重命名 `updateSource`，默认来自 impl 为 `"mirror"`): 更新渠道优先级。
  - `campus_exit_on_fail: bool` (L153-L154，重命名 `campusExitOnFail`，默认 `default_true`): 校园网连接失败时退出。
  - `campus_exit_start_minutes: u16` (L156-L157，重命名 `campusExitStartMinutes`，默认 `default_campus_exit_start_minutes`): 非校园网自动退出生效时段起点。
  - `campus_exit_end_minutes: u16` (L159-L160，重命名 `campusExitEndMinutes`，默认 `default_campus_exit_end_minutes`): 非校园网自动退出生效时段终点。
  - `campus_check_start_minutes: u16` (L161-L162，重命名 `campusCheckStartMinutes`，别名 `campusCheckStartHour`，默认 `default_campus_check_start_minutes`): 校园网检测时段起点。
  - `campus_check_end_minutes: u16` (L164-L165，重命名 `campusCheckEndMinutes`，默认 `default_campus_check_end_minutes`): 校园网检测时段终点。
  - `scheduled_login_minutes: u16` (L167-L168，重命名 `scheduledLoginMinutes`，默认 `1440`): 每日定时登录时刻（分钟数）。
  - `scheduled_logout_minutes: u16` (L170-L171，重命名 `scheduledLogoutMinutes`，默认 `1440`): 每日定时注销时刻（分钟数）。
  - `log_retention_days: u32` (L172-L173，重命名 `logRetentionDays`，默认 `default_log_retention_days`): 日志保留天数。
  - `max_disconnect_reconnect: u32` (L174-L175，重命名 `maxDisconnectReconnect`，默认 `default_max_disconnect_reconnect`): 最大断线重连次数。
  - `auto_login_cooldown_secs: u64` (L176-L177，重命名 `autoLoginCooldownSecs`，默认 `default_auto_login_cooldown_secs`): 自动登录冷却时间（秒）。
  - `skip_sha256_when_missing: bool` (L178-L179，重命名 `skipSha256WhenMissing`，默认 `false`): 缺失时跳过 SHA256 校验。
  - `config_version: u32` (L180-L181，重命名 `configVersion`，默认来自 impl 为 `5`): 配置版本号。

#### 辅助函数
- `deserialize_non_empty_or` (L184-L194): 通用反序列化辅助，若字符串为空则调用默认函数返回默认值。
- `deserialize_campus_gateway` (L196-L201): 校园网关的反序列化器，调用 `deserialize_non_empty_or` 和 `default_campus_gateway`。
- `deserialize_required_network_name` (L203-L208): 必需网络名称的反序列化器，调用 `deserialize_non_empty_or` 和 `default_required_network_name`。
- `default_true` (L210): 返回 `true`。
- `default_campus_check_start_minutes` (L212): 返回 `460`（约 7:40）。
- `default_campus_check_end_minutes` (L216): 返回 `1380`（23:00），注释提及 2026-09-13 起变更。
- `default_campus_exit_start_minutes` (L218): 返回 `480`（8:00）。
- `default_campus_exit_end_minutes` (L220): 返回 `1380`（23:00）。
- `default_fixed_gateway` (L222-L224): 返回 `"10.2.127.254"`。
- `default_log_retention_days` (L226): 返回 `7`。
- `default_max_disconnect_reconnect` (L228): 返回 `3`。
- `default_auto_login_cooldown_secs` (L230): 返回 `60`。
- `default_portal_url` (L232-L234): 返回 `"http://10.1.99.100"`。
- `default_required_network_name` (L236-L238): 返回 `"i-wxxy、iwxxy-2、iwxxy-3"`。
- `default_campus_gateway` (L240-L242): 返回 `"10.2.127.254"`。

#### Default 实现
- `impl Default for Config` (L244-L320):
    - 提供所有字段的默认值。
    - 关键默认值变更注释（源码行号）：
        - `auto_exit_after_login`: 默认 `false` (L260)，配合夜切功能。
        - `background_check_interval`: 默认 `60000` ms (L267)，后台留存优化。
        - `enable_night_operator_switch`: 默认 `false` (L271)，2026-10-02 起改回关闭。
        - `auto_exit_on_online`: 默认 `false` (L284)，与 `auto_exit_after_login` 同因。
        - `latency_test_interval`: 默认 `600000` ms (L291)，后台留存优化。
        - `scheduled_login_minutes` / `scheduled_logout_minutes`: 默认 `1440` (L310-L311，禁用哨兵)。
        - `config_version`: 默认 `5` (L317)。

#### Config 方法
- `masked_for_display` (L326-L330): 返回一个脱敏后的 `Config` 副本。密码非空时替换为 `PASSWORD_MASK`。
- `mask_in_place` (L332-L337): 原地修改，将非空密码替换为 `PASSWORD_MASK`。

#### 测试用例
- `default_reconnect_and_cooldown_config` (L343-L348): 验证默认的重连和冷却时间。
- `serde_missing_reconnect_field_uses_default` (L350-L360): 验证缺少新字段时反序列化能正确使用默认值。
- `serde_custom_reconnect_config` (L362-L370): 验证自定义值的正确解析。
- `serde_account_fields_json_names_and_defaults` (L374-L403): 验证账号相关字段的 JSON 键名和默认值，特别是旧配置缺失这些字段时的兼容性。

### tauri-app/src-tauri/src/config/persist.rs

#### 全局锁
- `LOGIN_HISTORY_LOCK` (L11): 登录历史读-改-写互斥锁。
- `QUALITY_HISTORY_LOCK` (L229): 质量历史读-改-写互斥锁。

#### 路径获取函数
- `get_data_dir` (L46-L59): 获取应用数据目录。优先使用 Tauri 提供的路径，否则回退到 `dirs::data_dir()` 或当前目录。
- `get_config_path` (L61-L63): 获取配置文件路径 `<data_dir>/config.json`。
- `get_accounts_dir` (L65-L67): 获取账号档案目录 `<data_dir>/accounts`。
- `get_account_path` (L70-L72): 获取特定账号档案路径 `<data_dir>/accounts/<id>.json`。
- `get_login_history_path` (L170-L172): 获取登录历史路径 `<data_dir>/login-history.json`。
- `get_quality_history_path` (L231-L233): 获取质量历史路径 `<data_dir>/quality_history.json`。

#### 原子写入
- `atomic_write` (L13-L44):
    - **目的**: 确保配置文件写入的原子性，防止断电或崩溃导致文件损坏。
    - **机制**: 先写入带纳秒时间戳的临时文件 `.json.tmp.<nanos>` (L15-L21)，`sync_all` 后 (L32) 尝试重命名为正式文件名 (L34)。重试 3 次 (L33-L40)，失败则清理临时文件 (L42)。

#### 账号档案操作
- `load_account_config` (L76-L97):
    - **功能**: 读取账号档案，解密密码 (L87-L94)。
    - **错误处理**: 文件不存在 (L78-L80)、读取失败 (L81-L82)、解析失败 (L83-L84)、解密失败 (L89-L92) 均返回 `Err`。
- `save_account_config` (L101-L112):
    - **功能**: 写入账号档案，加密密码 (L105-L108)。
    - **加密**: 非空密码使用 `crypto::encrypt` DPAPI 加密。
    - **写入**: 使用 `atomic_write` (L111)。
- `list_account_items` (L118-L168):
    - **功能**: 枚举账号档案为 `AccountItem` 列表。
    - **过滤**: 跳过 `.` 前缀和空名 (L135-L137)，非 json 扩展名跳过 (L128-L130)，按 ID 排序 (L166)。
    - **兜底**: `display_name` 为空或文件解析失败时兜底为 ID (L144-L154)。
    - **注意**: 此处不解密密码，仅判断是否保存过密码 (L149)。

#### 登录历史操作
- `append_login_history` (L174-L226):
    - **功能**: 追加一条登录历史记录 (L207-L214)。
    - **互斥**: 使用 `LOGIN_HISTORY_LOCK` (L175)。
    - **上限**: 最多保留 100 条 (L216-L218)。
    - **损坏处理**: 读取或解析失败时备份为 `.bak` 并重置为空 (L183-L188, L194-L199)。

#### 质量历史操作
- `append_quality_history` (L237-L239): 包装 `append_quality_history_to`。
- `append_quality_history_to` (L241-L288):
    - **功能**: 追加一条网络质量历史记录 (L271-L276)。
    - **互斥**: 使用 `QUALITY_HISTORY_LOCK` (L242)。
    - **上限**: 最多保留 100 条 (L278-L280)。
    - **数据转换**: 延迟 <0 落盘为 `null` (L273-L274)。
    - **损坏处理**: 同登录历史 (L249-L265)。

#### 配置保存与广播
- `save_config_and_broadcast` (L296-L320):
    - **功能**: 统一出口，保存配置到磁盘并广播 `config-changed` 事件 (L298-L302)。
    - **广播内容**: 必须使用 `masked_for_display()` 脱敏后的配置，包裹在 `{ "config": ... }` 中 (L301-L302)。
    - **额外**: 后台线程刷新无头核心凭据 (L306-L318)。
- `save_config_to_disk_encrypted` (L322-L334):
    - **功能**: 将配置加密后原子写入磁盘 (L328-L333)。
    - **加密逻辑**: 任何非空密码一律加密，包括 `PASSWORD_MASK` ("***") (L328-L330)，防止真实密码恰好为占位符时明文落盘。

#### 测试用例
- `literal_star_password_is_encrypted_on_disk` (L347-L363): 验证真实密码为 "***" 时也被加密。
- `empty_password_stays_empty` (L365-L376): 验证空密码原样落盘。
- `quality_history_head_insert_and_shape` (L391-L406): 验证质量历史头插法和数据形状。
- `quality_history_caps_at_100` (L408-L420): 验证质量历史上限 100 条。
- `quality_history_corrupt_backup_and_reset` (L422-L435): 验证损坏文件备份后重置。
- `list_account_items_missing_dir_is_empty` (L438-L443): 验证账号目录不存在时返回空列表。
- `list_account_items_fallback_and_filter` (L447-L474): 验证 `display_name` 兜底规则和过滤逻辑。
- `account_config_roundtrip_encrypts_password` (L477-L499，仅 Windows): 验证账号档案读写往返中密码的加解密。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────────────────┐
│                                   Config Module                                 │
├──────────────────────┬──────────────────────────────────────────────────────────┤
│      model.rs        │                      persist.rs                          │
│                      │                                                          │
│  ┌────────────────┐  │   ┌──────────────────────────────────────────────────┐   │
│  │  struct Config │  │   │  atomic_write(path, content) (L13)               │   │
│  │  - serde(default)│  │   │  1. Write to .tmp.<nanos>                        │   │
│  │  - masked_for_display()│  │  2. sync_all()                                 │   │
│  │  - mask_in_place()     │  │  3. rename to path                             │   │
│  └────────────────┘  │   └──────────────────────────────────────────────────┘   │
│                      │                                                          │
│  ┌────────────────┐  │   ┌──────────────────────────────────────────────────┐   │
│  │  Default impl  │  │   │  load/save_account_config (L76/L101)             │   │
│  │  - all defaults│  │   │  - encrypt/decrypt password                      │   │
│  │  - config_ver=5│  │   │  - use atomic_write                              │   │
│  └────────────────┘  │   └──────────────────────────────────────────────────┘   │
│                      │                                                          │
│                      │   ┌──────────────────────────────────────────────────┐   │
│                      │   │  save_config_and_broadcast (L296)                │   │
│                      │   │  1. save_config_to_disk_encrypted (L322)        │   │
│                      │   │  2. notify_config_changed(masked_config)         │   │
│                      │   └──────────────────────────────────────────────────┘   │
│                      │                                                          │
│                      │   ┌──────────────────────────────────────────────────┐   │
│                      │   │  append_login/quality_history (L174/L241)        │   │
│                      │   │  - Mutex guard                                   │   │
│                      │   │  - Head insert, cap 100                          │   │
│                      │   │  - Corrupt backup & reset                        │   │
│                      │   └──────────────────────────────────────────────────┘   │
└──────────────────────┴──────────────────────────────────────────────────────────┘
```

## Data Flow

1. **加载配置**:
    - `persist.rs` 中的 `load_account_config` (L76) 或主配置加载逻辑读取 JSON 文件。
    - `serde` 反序列化时，由于 `Config` 结构体上有 `#[serde(default)]` (L9)，任何缺失的字段都会自动使用 `Config::default()` (L244) 中的值填充，从而避免旧版本配置文件因缺少新字段而加载失败。
    - 如果密码非空，则调用 `crypto::decrypt` (L87) 解密。

2. **保存配置**:
    - 调用 `save_config_and_broadcast` (L296)。
    - 内部调用 `save_config_to_disk_encrypted` (L322)，将非空密码加密后 (L328-L330)，通过 `atomic_write` (L13) 原子写入磁盘。
    - 成功后，调用 `config.masked_for_display()` (L301) 获取脱敏配置，并通过 Tauri 事件系统广播 `config-changed` 事件，前端监听此事件更新 UI。

3. **账号档案管理**:
    - `list_account_items` (L118) 扫描 `accounts` 目录，逐个读取 JSON 文件，解析为 `Config` 对象，提取 `display_name` 等信息，构建 `AccountItem` 列表。
    - `save_account_config` (L101) 和 `load_account_config` (L76) 分别用于账号档案的加密保存和解密加载。

4. **历史记录**:
    - 登录和质量历史通过 `append_login_history` (L174) 和 `append_quality_history` (L237) 追加。
    - 两者都使用独立的 Mutex (`LOGIN_HISTORY_LOCK` L11, `QUALITY_HISTORY_LOCK` L229) 保证并发安全，采用头插法，限制最大 100 条，并在文件损坏时备份并重置。

## Connections

[[config-field-sets-bidirectional-sync]]
[[mask-placeholder-persisted-as-plaintext]]
