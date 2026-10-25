---
title: Config 模型与持久化机制
type: learning
source_files:
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/persist.rs
tags: [配置, serde, 兼容性, 安全, 原子写入]
---

## Overview

本文档记录 `Config` 模块的设计哲学、数据结构定义及持久化机制。核心目标是解决**旧版本配置文件因字段缺失导致反序列化失败**的问题，同时确保敏感信息（密码、VPN 密文）的安全存储以及配置文件写入的原子性。

模块由两个源文件组成：
1. **模型层 (`tauri-app/src-tauri/src/config/model.rs`)**：定义 `Config` 结构体，通过容器级 `#[serde(default)]` 实现向后兼容，并定义脱敏显示逻辑 `masked_for_display` / `mask_in_place`。
2. **持久化层 (`tauri-app/src-tauri/src/config/persist.rs`)**：负责配置与账号档案的读写、加密/解密、原子写入、登录/质量历史维护，以及统一保存广播出口 `save_config_and_broadcast`。

## Key Components

### tauri-app/src-tauri/src/config/model.rs

#### 常量
- `PASSWORD_MASK: &str` (L3): 密码脱敏占位符 `"***"`。
- `AUTO_DETECT_ADAPTER: &str` (L4): 适配器自动检测的显示名 `"自动检测"`。

#### 结构体: Config
- **定义**: `pub struct Config` (L10–L193)
- **容器级默认值**: `#[serde(default)]` (L9) 置于结构体上，任一字段缺失（旧版本配置/手工编辑）都用 `Default::default()` 补齐，避免 serde 缺字段即整体反序列化失败导致上层全量重置丢配置（见 L6–L9 注释）。
- **字段列表**（全部字段，含重命名、默认值与用途）：
  - `user: String` (L11): 校园网登录用户名。
  - `password: String` (L12–L13，字段级 `#[serde(default)]`): 密码（内存明文，磁盘 DPAPI 密文）。
  - `self_hello_enabled: bool` (L14–L18，重命名 `selfHelloEnabled`，默认 `default_true`): 是否启用 Windows Hello 操作验证；关闭后自助服务面板与绑定操作不再弹验证，但查看运营商账户密码仍强制验证。
  - `self_reverify_each_action: bool` (L19–L21，重命名 `selfReverifyEachAction`，默认 `false`): 自助服务面板每次操作都二次验证（默认 false：切入面板验证一次后操作共用）。
  - `operator: String` (L22): 运营商名称。
  - `adapter1: String` (L23): 主适配器名称（默认 `AUTO_DETECT_ADAPTER`）。
  - `adapter2: String` (L24): 副适配器名称。
  - `adapter1_account: String` (L25–L28，重命名 `adapter1Account`): 主适配器绑定的账号 id（设备级，切账号不被覆盖）。
  - `adapter2_account: String` (L29–L31，重命名 `adapter2Account`): 副适配器绑定的账号 id。
  - `dual_adapter: bool` (L32–L33，重命名 `dualAdapter`): 双适配器模式。
  - `auto_login_on_start: bool` (L34–L35，重命名 `autoLoginOnStart`): 启动时自动登录。
  - `auto_exit_after_login: bool` (L36–L37，重命名 `autoExitAfterLogin`): 登录后自动退出（默认 false，配合夜切）。
  - `minimize_to_tray: bool` (L38–L39，重命名 `minimizeToTray`): 最小化到托盘。
  - `lightweight_mode: bool` (L40–L43，重命名 `lightweightMode`，默认 `default_true`): 轻量化模式（桌面专属）。
  - `hidden_start: bool` (L44–L45，重命名 `hiddenStart`): 隐藏启动。
  - `auto_launch: bool` (L46–L47，重命名 `autoLaunch`): 开机自启。
  - `enable_background_check: bool` (L48–L49，重命名 `enableBackgroundCheck`): 启用后台检查。
  - `background_check_interval: u64` (L50–L51，重命名 `backgroundCheckInterval`): 后台检查间隔（毫秒，默认 60000）。
  - `auto_login_on_preparation: bool` (L52–L53，重命名 `autoLoginOnPreparation`): 准备阶段自动登录。
  - `enable_night_operator_switch: bool` (L54–L57，重命名 `enableNightOperatorSwitch`，默认 `false`): 夜间自动切换运营商总开关。
  - `night_operator_restore: String` (L58–L60，重命名 `nightOperatorRestore`，默认空): 切至无锡学院前暂存的原运营商。
  - `enable_night_outbound_switch: bool` (L61–L64，重命名 `enableNightOutboundSwitch`，默认 `false`): 夜间出站自动切换总开关。
  - `outbound_priority: Vec<String>` (L65–L70，重命名 `outboundPriority`，默认空): 出站网卡优先级列表（首项=夜间出站目标）。
  - `outbound_metric_restore: String` (L71–L75，重命名 `outboundMetricRestore`，默认空): 切换态快照 JSON，导入配置时清空。
  - `outbound_disabled_adapters: String` (L76–L81，重命名 `outboundDisabledAdapters`，默认空): 夜间切换禁用的校园网卡快照 JSON，导入配置时保留本机值。
  - `outbound_standby_route: String` (L82–L86，重命名 `outboundStandbyRoute`，默认空): 兜底默认路由快照 JSON，导入配置时清空。
  - `outbound_manual_hold_day: i32` (L87–L93，重命名 `outboundManualHoldDay`，默认 0): “立即切换”当日保持标记（num_days_from_ce），导入配置时保留本机值。
  - `outbound_pending_switch: String` (L94–L99，重命名 `outboundPendingSwitch`，默认空): 待切换状态 JSON，导入配置时保留本机值。
  - `night_outbound_restore: String` (L100–L103，重命名 `nightOutboundRestore`，默认空): 安卓切换态标记（恒 `"logged_out"`），桌面端不消费。
  - `dns_optimize_adapters: Vec<String>` (L104–L108，重命名 `dnsOptimizeAdapters`，默认空): DNS 优化目标适配器名单，桌面专属。
  - `device_aliases: HashMap<String, String>` (L109–L113，重命名 `deviceAliases`，默认空): 在线设备自定义名字映射。
  - `auto_exit_on_online: bool` (L114–L115，重命名 `autoExitOnOnline`): 在线后自动退出（默认 false）。
  - `theme_mode: String` (L116–L117，重命名 `themeMode`): 主题模式（默认 `"dark"`）。
  - `enable_notification: bool` (L118–L119，重命名 `enableNotification`): 启用通知。
  - `active_account: String` (L120–L121，重命名 `activeAccount`): 当前激活账号 ID。
  - `display_name: String` (L122–L125，重命名 `displayName`): 本文件所属账号的显示名。
  - `enable_latency_test: bool` (L126–L127，重命名 `enableLatencyTest`): 启用延迟测试。
  - `latency_test_interval: u64` (L128–L129，重命名 `latencyTestInterval`): 延迟测试间隔（毫秒，默认 600000）。
  - `custom_theme_color: String` (L130–L131，重命名 `customThemeColor`): 自定义主题颜色（默认 `"#6366f1"`）。
  - `default_panel: String` (L132–L133，重命名 `defaultPanel`): 默认面板。
  - `enable_network_quality: bool` (L134–L135，重命名 `enableNetworkQuality`): 启用网络质量检测。
  - `skip_ttfb_in_latency: bool` (L136–L137，重命名 `skipTtfbInLatency`，默认 `default_true`): 延迟测试跳过 TTFB。
  - `skip_content_in_latency: bool` (L138–L139，重命名 `skipContentInLatency`，默认 `default_true`): 延迟测试跳过内容下载。
  - `portal_url: String` (L140–L141，重命名 `portalUrl`，默认 `default_portal_url`): Portal URL（默认 `"http://10.1.99.100"`）。
  - `fixed_gateway: String` (L142–L143，重命名 `fixedGateway`，默认 `default_fixed_gateway`): 固定网关（默认 `"10.2.127.254"`）。
  - `required_network_name: String` (L144–L145，重命名 `requiredNetworkName`，默认 `default_required_network_name`，自定义反序列化 `deserialize_required_network_name`): 必需网络名称（默认 `"i-wxxy、iwxxy-2、iwxxy-3"`）。
  - `enable_network_name_check: bool` (L146–L147，重命名 `enableNetworkNameCheck`，默认 `default_true`): 启用网络名称检查。
  - `campus_gateway: String` (L148–L149，重命名 `campusGateway`，默认 `default_campus_gateway`，自定义反序列化 `deserialize_campus_gateway`): 校园网网关（默认 `"10.2.127.254"`）。
  - `update_source: String` (L150–L152，重命名 `updateSource`，默认 `"mirror"`): 更新渠道优先级（`"mirror"`|`"github"`）。
  - `campus_exit_on_fail: bool` (L153–L154，重命名 `campusExitOnFail`，默认 `default_true`): 校园网连接失败时退出。
  - `campus_exit_start_minutes: u16` (L155–L157，重命名 `campusExitStartMinutes`，默认 `default_campus_exit_start_minutes`=480): 非校园网自动退出生效时段起点（分钟）。
  - `campus_exit_end_minutes: u16` (L158–L160，重命名 `campusExitEndMinutes`，默认 `default_campus_exit_end_minutes`=1380): 非校园网自动退出生效时段终点。
  - `campus_check_start_minutes: u16` (L161–L162，重命名 `campusCheckStartMinutes`，别名 `campusCheckStartHour`，默认 `default_campus_check_start_minutes`=460): 校园网检测时段起点。
  - `campus_check_end_minutes: u16` (L163–L165，重命名 `campusCheckEndMinutes`，默认 `default_campus_check_end_minutes`=1380): 校园网检测时段终点。
  - `scheduled_login_minutes: u16` (L166–L168，重命名 `scheduledLoginMinutes`，默认 1440): 每日定时登录时刻（分钟，1440=禁用）。
  - `scheduled_logout_minutes: u16` (L169–L171，重命名 `scheduledLogoutMinutes`，默认 1440): 每日定时注销时刻（分钟）。
  - `log_retention_days: u32` (L172–L173，重命名 `logRetentionDays`，默认 `default_log_retention_days`=7): 日志保留天数。
  - `max_disconnect_reconnect: u32` (L174–L175，重命名 `maxDisconnectReconnect`，默认 `default_max_disconnect_reconnect`=3): 最大断线重连次数。
  - `auto_login_cooldown_secs: u64` (L176–L177，重命名 `autoLoginCooldownSecs`，默认 `default_auto_login_cooldown_secs`=60): 自动登录冷却秒数。
  - `skip_sha256_when_missing: bool` (L178–L179，重命名 `skipSha256WhenMissing`，默认 `false`): 缺失时跳过 SHA256 校验。
  - `vpn_account: String` (L180–L182，重命名 `vpnAccount`，默认空): WebVPN 登录账号（学号明文）。
  - `vpn_password_enc: String` (L183–L186，重命名 `vpnPasswordEnc`，默认空): WebVPN CAS 密码 DPAPI 密文。
  - `self_service_via_vpn: bool` (L187–L190，重命名 `selfServiceViaVpn`，默认 `default_true`): 自助服务自动 VPN 回退总开关。
  - `config_version: u32` (L191–L192，重命名 `configVersion`，默认由 `Default` impl 赋 5): 配置版本号。

#### 辅助函数
- `deserialize_non_empty_or<'de, D>(deserializer: D, default_fn: fn() -> String) -> Result<String, D::Error>` (L195–L205): 通用反序列化辅助，字符串为空则调默认函数。
- `deserialize_campus_gateway<'de, D>(deserializer: D) -> Result<String, D::Error>` (L207–L212): 校园网关反序列化器，调用上述辅助与 `default_campus_gateway`。
- `deserialize_required_network_name<'de, D>(deserializer: D) -> Result<String, D::Error>` (L214–L219): 必需网络名反序列化器。
- `default_true() -> bool` (L221): 返回 `true`。
- `default_campus_check_start_minutes() -> u16` (L223): 返回 `460`。
- `default_campus_check_end_minutes() -> u16` (L227): 返回 `1380`（2026-09-13 起变更）。
- `default_campus_exit_start_minutes() -> u16` (L229): 返回 `480`。
- `default_campus_exit_end_minutes() -> u16` (L231): 返回 `1380`。
- `default_fixed_gateway() -> String` (L233–L235): 返回 `"10.2.127.254"`。
- `default_log_retention_days() -> u32` (L237): 返回 `7`。
- `default_max_disconnect_reconnect() -> u32` (L239): 返回 `3`。
- `default_auto_login_cooldown_secs() -> u64` (L241): 返回 `60`。
- `default_portal_url() -> String` (L243–L245): 返回 `"http://10.1.99.100"`。
- `default_required_network_name() -> String` (L247–L249): 返回 `"i-wxxy、iwxxy-2、iwxxy-3"`。
- `default_campus_gateway() -> String` (L251–L253): 返回 `"10.2.127.254"`。

#### Default 实现
- `impl Default for Config` (L255–L335):
  - `user`/`password`/`operator`/`adapter2`/`adapter1_account`/`adapter2_account`/`night_operator_restore`/`outbound_priority` 等字符串字段默认空串（L258–L283 等）。
  - `adapter1` 默认 `AUTO_DETECT_ADAPTER` (L263)。
  - `self_hello_enabled` 默认 `true` (L260)；`self_reverify_each_action` 默认 `false` (L261)。
  - `auto_login_on_start` 默认 `true` (L268)；`auto_exit_after_login` 默认 `false` (L271，2026-09-20 起配合夜切)。
  - `lightweight_mode` 默认 `true` (L273)；`auto_launch` 默认 `true` (L275)；`enable_background_check` 默认 `true` (L276)。
  - `background_check_interval` 默认 `60000` (L278，2026-09-20 起 15s→60s)。
  - `enable_night_operator_switch` 默认 `false` (L282，2026-10-02 起改回关闭)。
  - `auto_exit_on_online` 默认 `false` (L295)。
  - `theme_mode` 默认 `"dark"` (L296)；`enable_notification` 默认 `true` (L297)。
  - `latency_test_interval` 默认 `600000` (L302，2026-09-20 起 60s→600s)。
  - `custom_theme_color` 默认 `"#6366f1"` (L303)；`enable_network_quality` 默认 `true` (L305)。
  - `skip_ttfb_in_latency`/`skip_content_in_latency` 默认 `true` (L306–L307)。
  - `portal_url`/`fixed_gateway`/`campus_gateway` 默认对应常量 (L308–L312)。
  - `update_source` 默认 `"mirror"` (L313)；`campus_exit_on_fail` 默认 `true` (L314)。
  - `campus_exit_start_minutes` 480 / `campus_exit_end_minutes` 1380 / `campus_check_start_minutes` 460 / `campus_check_end_minutes` 1380 (L315–L318)。
  - `scheduled_login_minutes` / `scheduled_logout_minutes` 默认 `1440` (L321–L322，禁用哨兵)。
  - `log_retention_days` 7 / `max_disconnect_reconnect` 3 / `auto_login_cooldown_secs` 60 (L323–L325)。
  - `self_service_via_vpn` 默认 `true` (L330，2026-10-25 起)。
  - `config_version` 默认 `5` (L332)。

#### Config 方法
- `masked_for_display(&self) -> Config` (L341–L345): 返回脱敏副本，调用 `mask_in_place`。
- `mask_in_place(&mut self)` (L347–L356): 原地脱敏，非空 `password` 与 `vpn_password_enc` 替换为 `PASSWORD_MASK`。

#### 测试用例（同文件 `mod tests` L359–L423）
- `default_reconnect_and_cooldown_config` (L363–L368): 验证默认重连次数与冷却。
- `serde_missing_reconnect_field_uses_default` (L370–L380): 删除新字段后反序列化回退默认。
- `serde_custom_reconnect_config` (L382–L390): 自定义值正确解析。
- `serde_account_fields_json_names_and_defaults` (L394–L423): 验证 `displayName`/`adapter1Account`/`adapter2Account` 键名、缺省空串兼容与序列化回读。

### tauri-app/src-tauri/src/config/persist.rs

#### 全局锁
- `LOGIN_HISTORY_LOCK: Mutex<()>` (L11): 登录历史读-改-写互斥锁。
- `QUALITY_HISTORY_LOCK: Mutex<()>` (L229): 质量历史读-改-写互斥锁。

#### 路径获取函数
- `get_data_dir(app_handle: &tauri::AppHandle) -> PathBuf` (L46–L59): 获取应用数据目录，优先 Tauri 路径，回退 `dirs::data_dir()` 或当前目录。
- `get_config_path(data_dir: &Path) -> PathBuf` (L61–L63): `<data_dir>/config.json`。
- `get_accounts_dir(data_dir: &Path) -> PathBuf` (L65–L67): `<data_dir>/accounts`。
- `get_account_path(data_dir: &Path, account_id: &str) -> PathBuf` (L70–L72): `<data_dir>/accounts/<id>.json`。
- `get_login_history_path(data_dir: &Path) -> PathBuf` (L170–L172): `<data_dir>/login-history.json`。
- `get_quality_history_path(data_dir: &Path) -> PathBuf` (L231–L233): `<data_dir>/quality_history.json`。

#### 原子写入
- `atomic_write(path: &Path, content: &str) -> Result<(), String>` (L13–L44): 写带纳秒时间戳临时文件 `.json.tmp.<nanos>` (L15–L21)，`sync_all` 后 (L32) 重试 3 次重命名 (L33–L40)，失败清理临时文件 (L42)。

#### 账号档案操作
- `load_account_config(data_dir: &Path, account_id: &str) -> Result<Config, String>` (L76–L97): 读档案，非空密码 `crypto::decrypt` 解密 (L86–L94)；文件不存在/读盘/解析/解密失败均返 `Err`。
- `save_account_config(data_dir: &Path, account_id: &str, config: &Config) -> Result<(), String>` (L101–L112): 非空密码 DPAPI 加密 (L105–L108)，`atomic_write` 落盘 (L111)。
- `list_account_items(data_dir: &Path) -> Vec<AccountItem>` (L118–L168): 枚举账号档案；过滤 `.` 前缀/空名/非 json (L128–L137)；按 id 排序 (L166)；`display_name` 空或解析失败兜底 id (L144–L154)；不解密密码仅判非空 (L149)。

#### 登录历史操作
- `append_login_history(app_handle: &tauri::AppHandle, success: bool, message: &str, adapter: &str, user: &str, login_type: &str) -> Result<(), String>` (L174–L226): 持 `LOGIN_HISTORY_LOCK` (L175)，头插记录 (L207–L214)，上限 100 (L216–L218)，损坏备份 `.bak` 重置 (L183–L199)。

#### 质量历史操作
- `append_quality_history(app_handle: &tauri::AppHandle, result: &NetworkQualityResult) -> Result<(), String>` (L237–L239): 包装 `append_quality_history_to`。
- `append_quality_history_to(data_dir: &Path, result: &NetworkQualityResult) -> Result<(), String>` (L241–L288): 持 `QUALITY_HISTORY_LOCK` (L242)，头插 (L271–L276)，延迟 <0 落盘 `null` (L273–L274)，上限 100 (L278–L280)，损坏备份重置 (L249–L265)。

#### 配置保存与广播
- `save_config_and_broadcast(app_handle: &AppHandle, config: &Config) -> Result<(), String>` (L296–L322): 调 `save_config_to_disk_encrypted` (L298)，掩码后 `notify_config_changed({config: emit_cfg})` 广播 (L301–L302)；Windows 桌面后台线程刷新无头核心凭据 (L307–L320)。
- `save_config_to_disk_encrypted(data_dir: &Path, config: &Config) -> Result<(), String>` (L324–L336): 非空密码一律 DPAPI 加密（含真实 `"***"` 不豁免，L330–L332），`atomic_write` 落盘 (L335)。

#### 测试用例（同文件 `mod tests` L338–L502）
- `literal_star_password_is_encrypted_on_disk` (L349–L365): 真实密码 `"***"` 也加密落盘。
- `empty_password_stays_empty` (L367–L378): 空密码原样。
- `quality_history_head_insert_and_shape` (L393–L408): 质量历史头插与形状（负延迟 null）。
- `quality_history_caps_at_100` (L410–L422): 上限 100。
- `quality_history_corrupt_backup_and_reset` (L424–L437): 损坏备份重置。
- `list_account_items_missing_dir_is_empty` (L440–L445): 目录不存在返空。
- `list_account_items_fallback_and_filter` (L449–L476): 兜底与过滤规则。
- `account_config_roundtrip_encrypts_password` (L478–L501，仅 Windows): 账号档案密码加解密往返。

## Architecture

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│                                Config Module                                 │
├────────────────────────┬─────────────────────────────────────────────────────┤
│       model.rs         │                       persist.rs                    │
│                        │                                                     │
│  ┌──────────────────┐  │   ┌─────────────────────────────────────────────┐   │
│  │ struct Config    │  │   │ atomic_write (L13)                          │   │
│  │ serde(default)   │  │   │  tmp.<nanos> → sync_all → rename            │   │
│  │ masked_for_display│  │   └─────────────────────────────────────────────┘   │
│  │ mask_in_place    │  │   ┌─────────────────────────────────────────────┐   │
│  └──────────────────┘  │   │ load/save_account_config (L76/L101)         │   │
│                        │   │  crypto encrypt/decrypt + atomic_write      │   │
│  ┌──────────────────┐  │   └─────────────────────────────────────────────┘   │
│  │ Default impl     │  │   ┌─────────────────────────────────────────────┐   │
│  │ config_version=5 │  │   │ save_config_and_broadcast (L296)           │   │
│  └──────────────────┘  │   │  save_encrypted → masked notify_config_changed│ │
│                        │   └─────────────────────────────────────────────┘   │
│                        │   ┌─────────────────────────────────────────────┐   │
│                        │   │ append_login/quality_history (L174/L241)    │   │
│                        │   │  Mutex, head insert cap 100, corrupt backup │   │
│                        │   └─────────────────────────────────────────────┘   │
└────────────────────────┴─────────────────────────────────────────────────────┘
```

## Data Flow

1. **加载配置**:
   - `persist.rs` 的 `load_account_config` (L76) 或主配置加载读取 JSON。
   - `serde` 反序列化因 `Config` 上 `#[serde(default)]` (L9) 自动用 `Config::default()` (L255) 补缺失字段，避免旧配置加载失败。
   - 非空密码调 `crypto::decrypt` (L87) 解密。

2. **保存配置**:
   - 调 `save_config_and_broadcast` (L296)。
   - 内调 `save_config_to_disk_encrypted` (L324)，非空密码加密 (L330–L332) 后 `atomic_write` (L13) 落盘。
   - 成功后将 `config.masked_for_display()` (L301) 包裹 `{ "config": ... }` 广播 `config-changed`，前端更新 UI。

3. **账号档案管理**:
   - `list_account_items` (L118) 扫 `accounts` 目录解析 `Config` 提取 `display_name` 等构建列表。
   - `save_account_config` (L101) / `load_account_config` (L76) 加密保存与解密加载。

4. **历史记录**:
   - `append_login_history` (L174) 与 `append_quality_history` (L237) 追加，独立 Mutex (L11/L229) 保并发安全，头插法上限 100，损坏备份重置。

## Connections

[[config-field-sets-bidirectional-sync]]
[[mask-placeholder-persisted-as-plaintext]]

## Known Issues

- **本机状态字段导入语义不一**：`outbound_metric_restore`/`outbound_standby_route` 导入配置时清空，而 `outbound_disabled_adapters`/`outbound_manual_hold_day`/`outbound_pending_switch`/`night_outbound_restore` 保留本机值（见 model.rs L71–L103 注释），跨设备迁移配置需注意。
- **真实密码恰为 `***` 时必须加密**：`save_config_to_disk_encrypted` (L330) 不豁免占位符，防止明文落盘；前端回传空/MASK 时由上层 `save_config` 保留当前值（见 model.rs L183–L186 注释）。
- **定时动作哨兵值**：`scheduled_login_minutes`/`scheduled_logout_minutes` 默认 `1440` 表示禁用（L321–L322），`0` 才是真实 00:00，存量 `0` 由迁移刷为 `1440`。