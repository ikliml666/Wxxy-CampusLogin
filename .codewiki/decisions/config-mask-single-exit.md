---
title: 敏感信息出站唯一出口（统一走 masked_for_display）
type: decision
source_files:
  - tauri-app/src-tauri/src/config/model.rs
  - android/src-tauri/src/config_state.rs
tags: [决策, 安全, 配置, 脱敏]
---

## Overview

### 背景
配置对象（桌面 `Config` / 安卓 `Settings`）包含账号密码等敏感字段，且会被多处发往前端或广播。逐处手工打码必然存在漏项风险，一处漏掉即导致明文泄露。因此确立“单一出口”约束：所有出站路径必须经过统一的掩码函数。当前桌面 `Config` 定义于 `tauri-app/src-tauri/src/config/model.rs` 行6-193，安卓 `Settings` 定义于 `android/src-tauri/src/config_state.rs` 行26-136。

### 决策
一切把配置发往前端的路径**必经** `Config::masked_for_display()`（桌面，model.rs 行341-345）/ `config_state::masked_for_display()`（安卓，config_state.rs 行437-446），**禁止在调用方手工逐字段打码**。日志、错误消息、事件 payload 一律不得携带明文 `password` 或 `vpn_password_enc`（桌面端 VPN 密文同样不出站，见 model.rs 行353-355；安卓端见 config_state.rs 行442-444）。

密码字段的更新语义两端同构：前端传入空串或占位符 `"***"` 表示“未修改”，后端回退已存值；显式清除需通过 `clear` 标志（桌面通过结构体字段语义隐式处理，安卓通过 `resolve_password_field` 的 `clear` 参数显式处理，config_state.rs 行458-467）。

### 理由
1. **安全性**：类型系统无法阻止开发者遗漏某个敏感字段（如未来新增字段），唯有强制单点出口才能确保零泄露。
2. **一致性**：两端使用相同的掩码逻辑（非空则替换为 `"***"`，空则保留空串），避免前端展示逻辑分裂。
3. **可测试性**：单测锁死了出站数据的明文内容，回归成本低（model.rs 行359-424 测试 `masked_for_display` 相关契约，config_state.rs 行543-897 测试涵盖掩码、加密落盘与回退语义）。

### 备选方案
- **手动打码**：在每个 IPC 命令返回前手动构造匿名结构体。缺点：字段多时极易遗漏，且维护成本高。
- **全量加密传输**：前端解密开销大，且密钥管理复杂，不符合 Web 前端安全模型。

## Key Components

### 桌面端（tauri-app/src-tauri/src/config/model.rs）

- **常量定义**：
  - `PASSWORD_MASK: &str = "***"`（行3）：统一掩码字符串。
  - `AUTO_DETECT_ADAPTER: &str = "自动检测"`（行4）：适配器默认值。

- **核心结构体 `Config`**（行6-193，实际字段行10-192）：
  容器级 `#[serde(default)]`（行9）保证旧版本配置或缺失字段时反序列化不失败，而是填充 `Default`（行255-335）。全部字段如下表（行10-192）：

  | Rust 字段 | JSON 名 | 行号 | 类型 | 默认/备注 |
  |---|---|---|---|---|
  | `user` | `user` | 11 | `String` | 无 |
  | `password` | `password` | 12-13 | `String` | `#[serde(default)]` 空串，掩码目标 |
  | `self_hello_enabled` | `selfHelloEnabled` | 14-18 | `bool` | `default_true` (true) |
  | `self_reverify_each_action` | `selfReverifyEachAction` | 19-21 | `bool` | `default` (false) |
  | `operator` | `operator` | 22 | `String` | 无 |
  | `adapter1` | `adapter1` | 23 | `String` | 默认 `AUTO_DETECT_ADAPTER` |
  | `adapter2` | `adapter2` | 24 | `String` | 无 |
  | `adapter1_account` | `adapter1Account` | 25-28 | `String` | 无 |
  | `adapter2_account` | `adapter2Account` | 29-31 | `String` | 无 |
  | `dual_adapter` | `dualAdapter` | 32-33 | `bool` | 无 |
  | `auto_login_on_start` | `autoLoginOnStart` | 34-35 | `bool` | 无 (默认 true, 行268) |
  | `auto_exit_after_login` | `autoExitAfterLogin` | 36-37 | `bool` | 无 (默认 false, 行271) |
  | `minimize_to_tray` | `minimizeToTray` | 38-39 | `bool` | 无 |
  | `lightweight_mode` | `lightweightMode` | 40-43 | `bool` | `default_true` (true, 行273) |
  | `hidden_start` | `hiddenStart` | 44-45 | `bool` | 无 |
  | `auto_launch` | `autoLaunch` | 46-47 | `bool` | 无 (默认 true, 行275) |
  | `enable_background_check` | `enableBackgroundCheck` | 48-49 | `bool` | 无 (默认 true, 行276) |
  | `background_check_interval` | `backgroundCheckInterval` | 50-51 | `u64` | 无 (默认 60000, 行278) |
  | `auto_login_on_preparation` | `autoLoginOnPreparation` | 52-53 | `bool` | 无 (默认 true, 行279) |
  | `enable_night_operator_switch` | `enableNightOperatorSwitch` | 54-57 | `bool` | `default` (false, 行282) |
  | `night_operator_restore` | `nightOperatorRestore` | 58-60 | `String` | `default` 空 |
  | `enable_night_outbound_switch` | `enableNightOutboundSwitch` | 61-64 | `bool` | `default` (false) |
  | `outbound_priority` | `outboundPriority` | 65-70 | `Vec<String>` | `default` 空 |
  | `outbound_metric_restore` | `outboundMetricRestore` | 71-75 | `String` | `default` 空 |
  | `outbound_disabled_adapters` | `outboundDisabledAdapters` | 76-81 | `String` | `default` 空 |
  | `outbound_standby_route` | `outboundStandbyRoute` | 82-86 | `String` | `default` 空 |
  | `outbound_manual_hold_day` | `outboundManualHoldDay` | 87-93 | `i32` | `default` 0 |
  | `outbound_pending_switch` | `outboundPendingSwitch` | 94-99 | `String` | `default` 空 |
  | `night_outbound_restore` | `nightOutboundRestore` | 100-103 | `String` | `default` 空 |
  | `dns_optimize_adapters` | `dnsOptimizeAdapters` | 104-108 | `Vec<String>` | `default` 空 |
  | `device_aliases` | `deviceAliases` | 109-113 | `HashMap<String, String>` | `default` 空 |
  | `auto_exit_on_online` | `autoExitOnOnline` | 114-115 | `bool` | 无 (默认 false, 行295) |
  | `theme_mode` | `themeMode` | 116-117 | `String` | 无 (默认 "dark", 行296) |
  | `enable_notification` | `enableNotification` | 118-119 | `bool` | 无 (默认 true, 行297) |
  | `active_account` | `activeAccount` | 120-121 | `String` | 无 |
  | `display_name` | `displayName` | 122-125 | `String` | 无 |
  | `enable_latency_test` | `enableLatencyTest` | 126-127 | `bool` | 无 (默认 true, 行300) |
  | `latency_test_interval` | `latencyTestInterval` | 128-129 | `u64` | 无 (默认 600000, 行302) |
  | `custom_theme_color` | `customThemeColor` | 130-131 | `String` | 无 (默认 "#6366f1", 行303) |
  | `default_panel` | `defaultPanel` | 132-133 | `String` | 无 |
  | `enable_network_quality` | `enableNetworkQuality` | 134-135 | `bool` | 无 (默认 true, 行305) |
  | `skip_ttfb_in_latency` | `skipTtfbInLatency` | 136-137 | `bool` | `default_true` (true, 行306) |
  | `skip_content_in_latency` | `skipContentInLatency` | 138-139 | `bool` | `default_true` (true, 行307) |
  | `portal_url` | `portalUrl` | 140-141 | `String` | `default_portal_url` (行308) |
  | `fixed_gateway` | `fixedGateway` | 142-143 | `String` | `default_fixed_gateway` (行309) |
  | `required_network_name` | `requiredNetworkName` | 144-145 | `String` | `default_required_network_name`, `deserialize_with` (行310) |
  | `enable_network_name_check` | `enableNetworkNameCheck` | 146-147 | `bool` | `default_true` (true, 行311) |
  | `campus_gateway` | `campusGateway` | 148-149 | `String` | `default_campus_gateway`, `deserialize_with` (行312) |
  | `update_source` | `updateSource` | 150-152 | `String` | `default` ("mirror", 行313) |
  | `campus_exit_on_fail` | `campusExitOnFail` | 153-154 | `bool` | `default_true` (true, 行314) |
  | `campus_exit_start_minutes` | `campusExitStartMinutes` | 155-157 | `u16` | `default_campus_exit_start_minutes` (480, 行315) |
  | `campus_exit_end_minutes` | `campusExitEndMinutes` | 158-160 | `u16` | `default_campus_exit_end_minutes` (1380, 行316) |
  | `campus_check_start_minutes` | `campusCheckStartMinutes` | 161-162 | `u16` | `alias `campusCheckStartHour`, `default_campus_check_start_minutes` (460, 行317) |
  | `campus_check_end_minutes` | `campusCheckEndMinutes` | 163-165 | `u16` | `default_campus_check_end_minutes` (1380, 行318) |
  | `scheduled_login_minutes` | `scheduledLoginMinutes` | 166-168 | `u16` | `default` (1440, 行321) |
  | `scheduled_logout_minutes` | `scheduledLogoutMinutes` | 169-171 | `u16` | `default` (1440, 行322) |
  | `log_retention_days` | `logRetentionDays` | 172-173 | `u32` | `default_log_retention_days` (7, 行323) |
  | `max_disconnect_reconnect` | `maxDisconnectReconnect` | 174-175 | `u32` | `default_max_disconnect_reconnect` (3, 行324) |
  | `auto_login_cooldown_secs` | `autoLoginCooldownSecs` | 176-177 | `u64` | `default_auto_login_cooldown_secs` (60, 行325) |
  | `skip_sha256_when_missing` | `skipSha256WhenMissing` | 178-179 | `bool` | `default` (false, 行326) |
  | `vpn_account` | `vpnAccount` | 180-182 | `String` | `default` 空 (行327) |
  | `vpn_password_enc` | `vpnPasswordEnc` | 183-186 | `String` | `default` 空 (行328)，掩码目标（密文） |
  | `self_service_via_vpn` | `selfServiceViaVpn` | 187-190 | `bool` | `default_true` (true, 行330) |
  | `config_version` | `configVersion` | 191-192 | `u32` | `default` (5, 行332) |

- **核心方法 `impl Config`**（行337-357）：
  - `masked_for_display(&self) -> Config`（行341-345）：**唯一出站入口**。克隆自身后调用 `mask_in_place` 并返回副本，确保内存中的原始 `Config` 不被破坏。
  - `mask_in_place(&mut self)`（行347-356）：就地执行掩码。若 `password` 非空，则替换为 `PASSWORD_MASK`（行348-350）；若 `vpn_password_enc` 非空，同样替换为 `PASSWORD_MASK`（行353-355）。注意此方法为 `pub`，任何调用方均可破坏内存明文，故严禁直接调用，必须通过 `masked_for_display` 的 clone 路径访问。

- **默认值与辅助函数**：
  - `impl Default for Config`（行255-335）：提供全部字段默认值。
  - 多个 `default_*` 函数（行221-253）供 serde 使用，如 `default_true` (行221)、`default_campus_check_start_minutes` (行223)、`default_campus_check_end_minutes` (行227)、`default_campus_exit_start_minutes` (行229)、`default_campus_exit_end_minutes` (行231)、`default_fixed_gateway` (行233-235)、`default_log_retention_days` (行237)、`default_max_disconnect_reconnect` (行239)、`default_auto_login_cooldown_secs` (行241)、`default_portal_url` (行243-245)、`default_required_network_name` (行247-249)、`default_campus_gateway` (行251-253)。
  - 反序列化辅助 `deserialize_non_empty_or` (行195-205) 及特定包装 `deserialize_campus_gateway` (行207-212)、`deserialize_required_network_name` (行214-219)。

- **测试模块**（行359-424）：
  - `default_reconnect_and_cooldown_config` (行363-368)
  - `serde_missing_reconnect_field_uses_default` (行370-380)
  - `serde_custom_reconnect_config` (行382-390)
  - `serde_account_fields_json_names_and_defaults` (行394-423)

### 安卓端（android/src-tauri/src/config_state.rs）

- **常量与静态**：
  - `PASSWORD_MASK: &str = "***"` (行10)
  - `CONFIG_FILE: &str = "config.json"` (行11)
  - `CONFIG_IO_LOCK` (行17-19, `lazy_static` 异步互斥锁)
  - `config_io_lock()` (行22-24)：返回全局异步互斥锁守卫，供账号命令与配置写互斥。

- **核心结构体 `Settings`**（行26-136，`#[serde(rename_all = "camelCase", default)]` 在行27）：
  对齐桌面 `Config` 子集，并额外含 `allow_2d_face_verify`、`background_check_idle_interval`、`enable_boot_autostart` 及 WebVPN 字段（`vpn_account`/`vpn_password_enc`/`self_service_via_vpn`）。全部字段如下表（行30-135）：

  | Rust 字段 | JSON 名 | 行号 | 类型 | 默认/备注 |
  |---|---|---|---|---|
  | `user` | `user` | 30 | `String` | 无 |
  | `password` | `password` | 31 | `String` | 敏感字段，落盘前加密 |
  | `self_hello_enabled` | `selfHelloEnabled` | 32 | `bool` | 默认 true (行143) |
  | `self_reverify_each_action` | `selfReverifyEachAction` | 33 | `bool` | 默认 false (行144) |
  | `allow_2d_face_verify` | `allow2dFaceVerify` | 34-36 | `bool` | 默认 false (行145) |
  | `operator` | `operator` | 37 | `String` | 无 |
  | `enable_night_operator_switch` | `enableNightOperatorSwitch` | 38-41 | `bool` | 默认 false (行149) |
  | `night_operator_restore` | `nightOperatorRestore` | 42-43 | `String` | 默认空 |
  | `enable_night_outbound_switch` | `enableNightOutboundSwitch` | 44-47 | `bool` | 默认 false (行151) |
  | `outbound_priority` | `outboundPriority` | 48-50 | `Vec<String>` | 默认空 |
  | `outbound_metric_restore` | `outboundMetricRestore` | 51-53 | `String` | 默认空 |
  | `outbound_disabled_adapters` | `outboundDisabledAdapters` | 54-56 | `String` | 默认空 |
  | `outbound_standby_route` | `outboundStandbyRoute` | 57-59 | `String` | 默认空 |
  | `night_outbound_restore` | `nightOutboundRestore` | 60-64 | `String` | 默认空 |
  | `dns_optimize_adapters` | `dnsOptimizeAdapters` | 65-67 | `Vec<String>` | 默认空 |
  | `device_aliases` | `deviceAliases` | 68-72 | `HashMap<String, String>` | 行71 `#[serde(default)]` 空 |
  | `auto_login_on_start` | `autoLoginOnStart` | 74 | `bool` | 默认 true (行159) |
  | `enable_background_check` | `enableBackgroundCheck` | 75 | `bool` | 默认 true (行160) |
  | `background_check_interval` | `backgroundCheckInterval` | 76 | `u64` | 默认 60000 (行163) |
  | `background_check_idle_interval` | `backgroundCheckIdleInterval` | 77-81 | `u64` | 默认 300000 (行165) |
  | `auto_login_on_preparation` | `autoLoginOnPreparation` | 82 | `bool` | 默认 true (行166) |
  | `max_disconnect_reconnect` | `maxDisconnectReconnect` | 83 | `u32` | 默认 3 (行167) |
  | `auto_login_cooldown_secs` | `autoLoginCooldownSecs` | 84 | `u64` | 默认 60 (行168) |
  | `theme_mode` | `themeMode` | 86 | `String` | 默认 "dark" (行169) |
  | `enable_notification` | `enableNotification` | 87 | `bool` | 默认 true (行170) |
  | `custom_theme_color` | `customThemeColor` | 88 | `String` | 默认 "#6366f1" (行171) |
  | `default_panel` | `defaultPanel` | 89 | `String` | 默认 "dashboard" (行172) |
  | `display_name` | `displayName` | 90-93 | `String` | 默认空 |
  | `active_account` | `activeAccount` | 94 | `String` | 默认空 |
  | `enable_boot_autostart` | `enableBootAutostart` | 95 | `bool` | 默认 false (行175) |
  | `enable_latency_test` | `enableLatencyTest` | 97 | `bool` | 默认 false (行176) |
  | `latency_test_interval` | `latencyTestInterval` | 98 | `u64` | 默认 600000 (行178) |
  | `enable_network_quality` | `enableNetworkQuality` | 99 | `bool` | 默认 false (行182) |
  | `skip_ttfb_in_latency` | `skipTtfbInLatency` | 100 | `bool` | 默认 true (行183) |
  | `skip_content_in_latency` | `skipContentInLatency` | 101 | `bool` | 默认 true (行184) |
  | `portal_url` | `portalUrl` | 103 | `String` | 默认 "http://10.1.99.100" (行185) |
  | `fixed_gateway` | `fixedGateway` | 104 | `String` | 默认 "10.2.127.254" (行186) |
  | `required_network_name` | `requiredNetworkName` | 105 | `String` | 默认三 SSID (行189) |
  | `enable_network_name_check` | `enableNetworkNameCheck` | 106 | `bool` | 默认 true (行190) |
  | `campus_gateway` | `campusGateway` | 107 | `String` | 默认 "10.2.127.254" (行191) |
  | `campus_check_start_minutes` | `campusCheckStartMinutes` | 108-109 | `u16` | 默认 460 (行193) |
  | `campus_check_end_minutes` | `campusCheckEndMinutes` | 110-111 | `u16` | 默认 1380 (行196) |
  | `scheduled_login_minutes` | `scheduledLoginMinutes` | 112-113 | `u16` | 默认 1440 (行200) |
  | `scheduled_logout_minutes` | `scheduledLogoutMinutes` | 114-115 | `u16` | 默认 1440 (行201) |
  | `update_source` | `updateSource` | 117-118 | `String` | 默认 "mirror" (行202) |
  | `log_retention_days` | `logRetentionDays` | 120 | `u32` | 默认 7 (行203) |
  | `vpn_account` | `vpnAccount` | 124 | `String` | 默认空 (行204) |
  | `vpn_password_enc` | `vpnPasswordEnc` | 125 | `String` | 默认空 (行205)，掩码目标（密文） |
  | `self_service_via_vpn` | `selfServiceViaVpn` | 126-128 | `bool` | 默认 true (行206) |
  | `config_schema_version` | `configSchemaVersion` | 129-135 | `u32` | 默认 7 (行208)，迁移版本 |

- **加密与存储组件**：
  - `struct CryptoBridge` (行214-218, `Clone`) 及 `impl CryptoBridge::from_app` (行220-251)：真机走 `CampusKeystoreExt` (行224)，Host 测试注入假实现 (行245)。
  - `struct EncodedSettings` (行254-259)：磁盘存储形态（含 `settings` 和 `password_cipher`）。落盘时 `settings.password` 被清空 (行304)。
  - `load_file` (行261-274)：负责 JSON 读取与解密。
  - `password_saved_at` (行281-287)：判断密码是否已保存必须查密文位 `password_cipher`，不能看外露 `settings.password`。
  - `save_file` (行289-319)：加密落盘，使用带 PID 的临时文件原子写入 (行311-317)。加密失败时返回错误 (行300-301)。
  - `cleanup_stale_config_tmp` (行324-335)：启动时清理遗留临时文件。
  - `load_from` (行337-347)：加载配置并执行迁移，含 SSID 单值扩三值 (行343-345)。
  - `migrate_legacy_defaults` (行366-428)：根据 `config_schema_version` 执行 v0→v7 迁移。
  - `save_to` (行430-432)：封装 `save_file`。

- **掩码与广播（唯一出口相关）**：
  - `masked_for_display(s: &Settings) -> serde_json::Value` (行437-446)：**唯一出站入口**。克隆并掩码后序列化为 JSON Value；非空 `password` 替换为 `PASSWORD_MASK`（行439-441），非空 `vpn_password_enc` 同样打码（行442-444）。
  - `emit_config_changed(app, s)` (行452-455)：广播 `config-changed` 事件，Payload 为 `{ "config": masked_for_display(s) }`（行454）。
  - `resolve_password_field(incoming, current, clear)` (行458-467)：处理密码更新逻辑。`clear=true` 置空（行459-461）；空或 `"***"` 保留 `current`（行462-464）；否则采用 `incoming`（行465-466）。
  - `#[tauri::command] get_config` (行469-482) 与 `save_config` (行484-520)：命令层，全程持 `config_io_lock`（行493 / 行22-24）并调用掩码出口。
  - `current_settings` (行524-541)：取当前配置供回退。

- **测试模块**（行543-897）：涵盖加密落盘、解密失败、SSID 迁移、默认值语义、掩码出口等，如 `加密落盘_文件不含明文_读回等于原文` (行607-619)、`掩码出口_非空变星号_空保持空` (行797-806)、`掩码出口_vpn密文打码` (行808-818)、`密码字段回退语义_与桌面同构` (行849-861)。

## Architecture

```text
┌──────────────────────────────┐   ┌──────────────────────────────┐
│ 桌面 Config (model.rs:6-193) │   │ 安卓 Settings (cfg_state:26-136)│
└──────────────┬───────────────┘   └──────────────┬───────────────┘
               │ masked_for_display (model.rs:341)│ masked_for_display (cfg_state:437)
               ▼                                  ▼
┌──────────────────────────────┐   ┌──────────────────────────────┐
│ 掩码 Config (password:"***")  │   │ 掩码 JSON (password:"***")     │
└──────────────┬───────────────┘   └──────────────┬───────────────┘
               ▼                                  ▼
┌──────────────────────────────┐   ┌──────────────────────────────┐
│ IPC 返回 / 前端渲染           │   │ emit_config_changed (cfg_state:452)│
└──────────────────────────────┘   └──────────────┬───────────────┘
                                                  │ save_file 加密
                                                  ▼
┌──────────────────────────────┐   ┌──────────────────────────────┐
│                              │◄──│ EncodedSettings (cfg_state:254)│
└──────────────────────────────┘   └──────────────────────────────┘
```

## Data Flow

- **出站脱敏流（桌面）**：任何 IPC 命令需发配置给前端时，调用 `Config::masked_for_display()` (model.rs 行341-345)，内部 `clone` 后 `mask_in_place` (行347-356) 将非空 `password` 及 `vpn_password_enc` 替换为 `PASSWORD_MASK` (行3)，返回新副本，原始内存不变。
- **出站脱敏流（安卓）**：命令层（如 `get_config` 行469-482 或 `save_config` 行484-520）调用 `masked_for_display` (行437-446) 返回 `serde_json::Value`，或经 `emit_config_changed` (行452-455) 广播 `{ "config": masked_for_display(s) }` 事件，前端依赖该事件同步 Store。
- **密码更新流**：前端传 `password` 字段。若为 `"***"` 或空串，安卓经 `resolve_password_field` (行458-467) 判据回退已存值；`clear=true` 显式清除。桌面由结构体内存语义隐式处理。VPN 凭据（安卓 `vpn_password_enc`）不归 `save_config` 管，空或掩码一律回退已存密文 (行506-507)。
- **落盘加密流（安卓）**：`save_file` (行289-319) 中若 `password` 非空，经 `CryptoBridge::encrypt` (行228 / 行298) 加密存入 `password_cipher` (行258)，同时 `settings.password` 清空 (行304)。`load_file` (行261-274) 解密回填外露字段 (行270-272)。

## Connections

- 桌面与安卓配置结构对齐与脱敏契约见 [[tauri-app/src-tauri/src/config/model.rs]] 与 [[android/src-tauri/src/config_state.rs]]。
- 配置广播驱动前端状态同步见 [[frontend-store-sync]]（对应 `config-changed` 事件）。
- 配置 IO 锁与账号命令互斥见 [[account-commands]]（对应 `account_cmds.rs` 迁入的 `CONFIG_IO_LOCK`，config_state.rs 行17-19、行493）。
- 敏感信息保护设计总览见 [[security-architecture]]。

## Known Issues

### 已知漏洞面
1. **桌面端 `mask_in_place` 可见性**：`Config::mask_in_place` 是 `pub` 方法（model.rs 行347-356），类型系统不阻止调用方直接修改内存中的明文配置。虽然当前代码仅通过 `masked_for_display` 间接调用，但若未来有内部逻辑误用该函数，可能导致内存残留明文。建议后续考虑将其降级为 `pub(crate)` 或私有，或通过更严格的封装限制访问。
2. **安卓端密码加密失败**：若 `AndroidKeyStore` 加密失败，`save_file` 会直接返回错误（config_state.rs 行300-301），导致配置保存失败。这在密钥库损坏或硬件异常时可能发生，需确保前端有相应的错误提示。