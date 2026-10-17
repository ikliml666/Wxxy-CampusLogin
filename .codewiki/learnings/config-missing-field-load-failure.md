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
- `PASSWORD_MASK`: `"***"` (L4) - 密码脱敏占位符。
- `AUTO_DETECT_ADAPTER`: `"自动检测"` (L5) - 适配器自动检测的显示名。

#### 结构体: Config
- **定义**: `pub struct Config` (L9-L108)
- **容器级默认值**: `#[serde(default)]` (L9)
    - **作用**: 允许旧版本配置文件或缺失字段的 JSON 在反序列化时自动使用 `Default::default()` 填充缺失字段，防止因单个字段缺失导致整个配置加载失败。
- **字段列表**:
    - `user: String` (L11): 用户名。
    - `password: String` (L13): 密码。字段级 `#[serde(default)]`。
    - `self_hello_enabled: bool` (L17): 是否启用 Windows Hello 操作验证。默认 `true`。重命名为 `selfHelloEnabled`。
    - `self_reverify_each_action: bool` (L20): 自助服务面板每次操作二次验证。默认 `false`。重命名为 `selfReverifyEachAction`。
    - `operator: String` (L23): 运营商名称。
    - `adapter1: String` (L24): 主适配器名称。
    - `adapter2: String` (L25): 副适配器名称。
    - `adapter1_account: String` (L29): 主适配器绑定的账号 ID。重命名为 `adapter1Account`。
    - `adapter2_account: String` (L32): 副适配器绑定的账号 ID。重命名为 `adapter2Account`。
    - `dual_adapter: bool` (L34): 双适配器模式。重命名为 `dualAdapter`。
    - `auto_login_on_start: bool` (L36): 启动时自动登录。重命名为 `autoLoginOnStart`。
    - `auto_exit_after_login: bool` (L38): 登录后自动退出。重命名为 `autoExitAfterLogin`。
    - `minimize_to_tray: bool` (L40): 最小化到托盘。重命名为 `minimizeToTray`。
    - `lightweight_mode: bool` (L44): 轻量化模式。重命名为 `lightweightMode`。
    - `hidden_start: bool` (L46): 隐藏启动。重命名为 `hiddenStart`。
    - `auto_launch: bool` (L48): 开机自启。重命名为 `autoLaunch`。
    - `enable_background_check: bool` (L50): 启用后台检查。重命名为 `enableBackgroundCheck`。
    - `background_check_interval: u64` (L52): 后台检查间隔（毫秒）。重命名为 `backgroundCheckInterval`。
    - `auto_login_on_preparation: bool` (L54): 准备阶段自动登录。重命名为 `autoLoginOnPreparation`。
    - `enable_night_operator_switch: bool` (L58): 夜间自动切换运营商开关。重命名为 `enableNightOperatorSwitch`。
    - `night_operator_restore: String` (L61): 夜间切换前暂存的运营商。重命名为 `nightOperatorRestore`。
    - `enable_night_outbound_switch: bool` (L64): 夜间出站自动切换开关。重命名为 `enableNightOutboundSwitch`。
    - `outbound_priority: Vec<String>` (L68): 出站网卡优先级列表。重命名为 `outboundPriority`。
    - `outbound_metric_restore: String` (L73): 切换态快照（JSON 字符串）。重命名为 `outboundMetricRestore`。
    - `outbound_disabled_adapters: String` (L78): 夜间切换禁用的校园网卡快照（JSON 字符串）。重命名为 `outboundDisabledAdapters`。
    - `outbound_standby_route: String` (L83): 夜间切换新增的兜底默认路由快照（JSON 字符串）。重命名为 `outboundStandbyRoute`。
    - `outbound_manual_hold_day: i32` (L88): "立即切换"当日保持标记。重命名为 `outboundManualHoldDay`。
    - `night_outbound_restore: String` (L92): 安卓切换态标记。重命名为 `nightOutboundRestore`。
    - `dns_optimize_adapters: Vec<String>` (L96): DNS 优化目标适配器名单。重命名为 `dnsOptimizeAdapters`。
    - `device_aliases: HashMap<String, String>` (L101): 在线设备自定义名字映射。重命名为 `deviceAliases`。
    - `auto_exit_on_online: bool` (L103): 在线后自动退出。重命名为 `autoExitOnOnline`。
    - `theme_mode: String` (L105): 主题模式。重命名为 `themeMode`。
    - `enable_notification: bool` (L107): 启用通知。重命名为 `enableNotification`。
    - `active_account: String` (L109): 当前激活账号 ID。重命名为 `activeAccount`。
    - `display_name: String` (L113): 本文件所属账号的显示名。重命名为 `displayName`。
    - `enable_latency_test: bool` (L115): 启用延迟测试。重命名为 `enableLatencyTest`。
    - `latency_test_interval: u64` (L117): 延迟测试间隔（毫秒）。重命名为 `latencyTestInterval`。
    - `custom_theme_color: String` (L119): 自定义主题颜色。重命名为 `customThemeColor`。
    - `default_panel: String` (L121): 默认面板。重命名为 `defaultPanel`。
    - `enable_network_quality: bool` (L123): 启用网络质量检测。重命名为 `enableNetworkQuality`。
    - `skip_ttfb_in_latency: bool` (L125): 延迟测试中跳过 TTFB。重命名为 `skipTtfbInLatency`。
    - `skip_content_in_latency: bool` (L127): 延迟测试中跳过内容下载。重命名为 `skipContentInLatency`。
    - `portal_url: String` (L129): Portal URL。重命名为 `portalUrl`。
    - `fixed_gateway: String` (L131): 固定网关。重命名为 `fixedGateway`。
    - `required_network_name: String` (L133): 必需的网络名称。重命名为 `requiredNetworkName`，使用自定义反序列化器。
    - `enable_network_name_check: bool` (L135): 启用网络名称检查。重命名为 `enableNetworkNameCheck`。
    - `campus_gateway: String` (L137): 校园网网关。重命名为 `campusGateway`，使用自定义反序列化器。
    - `update_source: String` (L140): 更新渠道优先级。重命名为 `updateSource`。
    - `campus_exit_on_fail: bool` (L142): 校园网连接失败时退出。重命名为 `campusExitOnFail`。
    - `campus_exit_start_minutes: u16` (L144): 非校园网自动退出生效时段起点。重命名为 `campusExitStartMinutes`。
    - `campus_exit_end_minutes: u16` (L146): 非校园网自动退出生效时段终点。重命名为 `campusExitEndMinutes`。
    - `campus_check_start_minutes: u16` (L148): 校园网检测时段起点。重命名为 `campusCheckStartMinutes`，支持别名 `campusCheckStartHour`。
    - `campus_check_end_minutes: u16` (L150): 校园网检测时段终点。重命名为 `campusCheckEndMinutes`。
    - `scheduled_login_minutes: u16` (L153): 每日定时登录时刻（分钟数）。重命名为 `scheduledLoginMinutes`。
    - `scheduled_logout_minutes: u16` (L155): 每日定时注销时刻（分钟数）。重命名为 `scheduledLogoutMinutes`。
    - `log_retention_days: u32` (L157): 日志保留天数。重命名为 `logRetentionDays`。
    - `max_disconnect_reconnect: u32` (L159): 最大断线重连次数。重命名为 `maxDisconnectReconnect`。
    - `auto_login_cooldown_secs: u64` (L161): 自动登录冷却时间（秒）。重命名为 `autoLoginCooldownSecs`。
    - `skip_sha256_when_missing: bool` (L163): 缺失时跳过 SHA256 校验。重命名为 `skipSha256WhenMissing`。
    - `config_version: u32` (L165): 配置版本号。重命名为 `configVersion`。

#### 辅助函数
- `deserialize_non_empty_or` (L168-L176): 通用反序列化辅助，若字符串为空则调用默认函数返回默认值。
- `deserialize_campus_gateway` (L178-L183): 校园网关的反序列化器，调用 `deserialize_non_empty_or` 和 `default_campus_gateway`。
- `deserialize_required_network_name` (L185-L190): 必需网络名称的反序列化器，调用 `deserialize_non_empty_or` 和 `default_required_network_name`。
- `default_true` (L192): 返回 `true`。
- `default_campus_check_start_minutes` (L194): 返回 `460`（约 7:40）。
- `default_campus_check_end_minutes` (L199): 返回 `1380`（23:00）。
- `default_campus_exit_start_minutes` (L201): 返回 `480`（8:00）。
- `default_campus_exit_end_minutes` (L203): 返回 `1380`（23:00）。
- `default_fixed_gateway` (L205-L207): 返回 `"10.2.127.254"`。
- `default_log_retention_days` (L209-L211): 返回 `7`。
- `default_max_disconnect_reconnect` (L213-L215): 返回 `3`。
- `default_auto_login_cooldown_secs` (L217-L219): 返回 `60`。
- `default_portal_url` (L221-L223): 返回 `"http://10.1.99.100"`。
- `default_required_network_name` (L225-L227): 返回 `"i-wxxy、iwxxy-2、iwxxy-3"`。
- `default_campus_gateway` (L229-L231): 返回 `"10.2.127.254"`。

#### Default 实现
- `impl Default for Config` (L233-L295):
    - 提供所有字段的默认值。
    - 关键默认值变更注释：
        - `auto_exit_after_login`: 默认 `false`（L244），配合夜切功能。
        - `background_check_interval`: 默认 `60000` ms（L249），后台留存优化。
        - `enable_night_operator_switch`: 默认 `false`（L253），2026-10-02 起改回关闭。
        - `auto_exit_on_online`: 默认 `false`（L263），与 `auto_exit_after_login` 同因。
        - `latency_test_interval`: 默认 `600000` ms（L267），后台留存优化。
        - `scheduled_login_minutes` / `scheduled_logout_minutes`: 默认 `1440`（禁用哨兵）（L276）。
        - `config_version`: 默认 `5`（L281）。

#### Config 方法
- `masked_for_display` (L299-L303): 返回一个脱敏后的 `Config` 副本。密码非空时替换为 `PASSWORD_MASK`。
- `mask_in_place` (L305-L309): 原地修改，将非空密码替换为 `PASSWORD_MASK`。

#### 测试用例
- `default_reconnect_and_cooldown_config` (L315-L320): 验证默认的重连和冷却时间。
- `serde_missing_reconnect_field_uses_default` (L322-L332): 验证缺少新字段时反序列化能正确使用默认值。
- `serde_custom_reconnect_config` (L334-L341): 验证自定义值的正确解析。
- `serde_account_fields_json_names_and_defaults` (L344-L372): 验证账号相关字段的 JSON 键名和默认值，特别是旧配置缺失这些字段时的兼容性。

### tauri-app/src-tauri/src/config/persist.rs

#### 全局锁
- `LOGIN_HISTORY_LOCK` (L12): 登录历史读-改-写互斥锁。
- `QUALITY_HISTORY_LOCK` (L175): 质量历史读-改-写互斥锁。

#### 路径获取函数
- `get_data_dir` (L45-L58): 获取应用数据目录。优先使用 Tauri 提供的路径，否则回退到 `dirs::data_dir()` 或当前目录。
- `get_config_path` (L60-L62): 获取配置文件路径 `<data_dir>/config.json`。
- `get_accounts_dir` (L64-L66): 获取账号档案目录 `<data_dir>/accounts`。
- `get_account_path` (L69-L71): 获取特定账号档案路径 `<data_dir>/accounts/<id>.json`。
- `get_login_history_path` (L165-L167): 获取登录历史路径 `<data_dir>/login-history.json`。
- `get_quality_history_path` (L177-L179): 获取质量历史路径 `<data_dir>/quality_history.json`。

#### 原子写入
- `atomic_write` (L15-L43):
    - **目的**: 确保配置文件写入的原子性，防止断电或崩溃导致文件损坏。
    - **机制**: 先写入带纳秒时间戳的临时文件 `.json.tmp.<nanos>`，`sync_all` 后尝试重命名为正式文件名。重试 3 次，失败则清理临时文件。

#### 账号档案操作
- `load_account_config` (L74-L96):
    - **功能**: 读取账号档案，解密密码。
    - **错误处理**: 文件不存在、读取失败、解析失败、解密失败均返回 `Err`。
- `save_account_config` (L99-L115):
    - **功能**: 写入账号档案，加密密码。
    - **加密**: 非空密码使用 DPAPI 加密。
    - **写入**: 使用 `atomic_write`。
- `list_account_items` (L118-L163):
    - **功能**: 枚举账号档案为 `AccountItem` 列表。
    - **过滤**: 跳过 `.` 前缀和空名，按 ID 排序。
    - **兜底**: `display_name` 为空或文件解析失败时兜底为 ID。
    - **注意**: 此处不解密密码，仅判断是否保存过密码。

#### 登录历史操作
- `append_login_history` (L169-L213):
    - **功能**: 追加一条登录历史记录。
    - **互斥**: 使用 `LOGIN_HISTORY_LOCK`。
    - **上限**: 最多保留 100 条。
    - **损坏处理**: 读取或解析失败时备份为 `.bak` 并重置为空。

#### 质量历史操作
- `append_quality_history` (L182-L184): 包装 `append_quality_history_to`。
- `append_quality_history_to` (L186-L234):
    - **功能**: 追加一条网络质量历史记录。
    - **互斥**: 使用 `QUALITY_HISTORY_LOCK`。
    - **上限**: 最多保留 100 条。
    - **数据转换**: 延迟 <0 落盘为 `null`。
    - **损坏处理**: 同登录历史。

#### 配置保存与广播
- `save_config_and_broadcast` (L237-L247):
    - **功能**: 统一出口，保存配置到磁盘并广播 `config-changed` 事件。
    - **广播内容**: 必须使用 `masked_for_display()` 脱敏后的配置，包裹在 `{ "config": ... }` 中。
- `save_config_to_disk_encrypted` (L249-L263):
    - **功能**: 将配置加密后原子写入磁盘。
    - **加密逻辑**: 任何非空密码一律加密，包括 `PASSWORD_MASK` ("***")，防止真实密码恰好为占位符时明文落盘。

#### 测试用例
- `literal_star_password_is_encrypted_on_disk` (L269-L280): 验证真实密码为 "***" 时也被加密。
- `empty_password_stays_empty` (L282-L292): 验证空密码原样落盘。
- `quality_history_head_insert_and_shape` (L294-L313): 验证质量历史头插法和数据形状。
- `quality_history_caps_at_100` (L315-L324): 验证质量历史上限 100 条。
- `quality_history_corrupt_backup_and_reset` (L326-L337): 验证损坏文件备份后重置。
- `list_account_items_missing_dir_is_empty` (L340-L344): 验证账号目录不存在时返回空列表。
- `list_account_items_fallback_and_filter` (L347-L377): 验证 `display_name` 兜底规则和过滤逻辑。
- `account_config_roundtrip_encrypts_password` (L380-L399): (仅 Windows) 验证账号档案读写往返中密码的加解密。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────────────────┐
│                                   Config Module                                 │
├──────────────────────┬──────────────────────────────────────────────────────────┤
│      model.rs        │                      persist.rs                          │
│                      │                                                          │
│  ┌────────────────┐  │   ┌──────────────────────────────────────────────────┐   │
│  │  struct Config │  │   │  atomic_write(path, content)                     │   │
│  │  - serde(default)│  │   │  1. Write to .tmp.<nanos>                        │   │
│  │  - masked_for_display()│  │  2. sync_all()                                 │   │
│  │  - mask_in_place()     │  │  3. rename to path                             │   │
│  └────────────────┘  │   └──────────────────────────────────────────────────┘   │
│                      │                                                          │
│  ┌────────────────┐  │   ┌──────────────────────────────────────────────────┐   │
│  │  Default impl  │  │   │  load/save_account_config                        │   │
│  │  - all defaults│  │   │  - encrypt/decrypt password                      │   │
│  │  - config_ver=5│  │   │  - use atomic_write                              │   │
│  └────────────────┘  │   └──────────────────────────────────────────────────┘   │
│                      │                                                          │
│                      │   ┌──────────────────────────────────────────────────┐   │
│                      │   │  save_config_and_broadcast                       │   │
│                      │   │  1. save_config_to_disk_encrypted                │   │
│                      │   │  2. notify_config_changed(masked_config)         │   │
│                      │   └──────────────────────────────────────────────────┘   │
│                      │                                                          │
│                      │   ┌──────────────────────────────────────────────────┐   │
│                      │   │  append_login/history                            │   │
│                      │   │  - Mutex guard                                   │   │
│                      │   │  - Head insert, cap 100                          │   │
│                      │   │  - Corrupt backup & reset                        │   │
│                      │   └──────────────────────────────────────────────────┘   │
└──────────────────────┴──────────────────────────────────────────────────────────┘
```

## Data Flow

1. **加载配置**:
    - `persist.rs` 中的 `load_account_config` 或主配置加载逻辑读取 JSON 文件。
    - `serde` 反序列化时，由于 `Config` 结构体上有 `#[serde(default)]`，任何缺失的字段都会自动使用 `Config::default()` 中的值填充，从而避免旧版本配置文件因缺少新字段而加载失败。
    - 如果密码非空，则调用 `crypto::decrypt` 解密。

2. **保存配置**:
    - 调用 `save_config_and_broadcast`。
    - 内部调用 `save_config_to_disk_encrypted`，将非空密码加密后，通过 `atomic_write` 原子写入磁盘。
    - 成功后，调用 `config.masked_for_display()` 获取脱敏配置，并通过 Tauri 事件系统广播 `config-changed` 事件，前端监听此事件更新 UI。

3. **账号档案管理**:
    - `list_account_items` 扫描 `accounts` 目录，逐个读取 JSON 文件，解析为 `Config` 对象，提取 `display_name` 等信息，构建 `AccountItem` 列表。
    - `save_account_config` 和 `load_account_config` 分别用于账号档案的加密保存和解密加载。

4. **历史记录**:
    - 登录和质量历史通过 `append_login_history` 和 `append_quality_history` 追加。
    - 两者都使用独立的 Mutex 保证并发安全，采用头插法，限制最大 100 条，并在文件损坏时备份并重置。

## Connections

[[config-field-sets-bidirectional-sync]]
[[mask-placeholder-persisted-as-plaintext]]