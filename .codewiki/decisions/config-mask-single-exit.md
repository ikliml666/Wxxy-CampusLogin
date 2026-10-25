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
配置对象（桌面 `Config` / 安卓 `Settings`）包含账号密码等敏感字段，且会被多处发往前端或广播。逐处手工打码必然存在漏项风险，一处漏掉即导致明文泄露。因此确立“单一出口”约束：所有出站路径必须经过统一的掩码函数。当前桌面 `Config` 定义于 `tauri-app/src-tauri/src/config/model.rs` 行6-182，安卓 `Settings` 定义于 `android/src-tauri/src/config_state.rs` 行26-128。

### 决策
一切把配置发往前端的路径**必经** `Config::masked_for_display()`（桌面，model.rs 行326-330）/ `config_state::masked_for_display()`（安卓，config_state.rs 行424-430），**禁止在调用方手工逐字段打码**。日志、错误消息、事件 payload 一律不得携带明文 `password`。

密码字段的更新语义两端同构：前端传入空串或占位符 `"***"` 表示“未修改”，后端回退已存值；显式清除需通过 `clear` 标志（桌面通过结构体字段语义隐式处理，安卓通过 `resolve_password_field` 的 `clear` 参数显式处理，config_state.rs 行442-451）。

### 理由
1. **安全性**：类型系统无法阻止开发者遗漏某个敏感字段（如未来新增字段），唯有强制单点出口才能确保零泄露。
2. **一致性**：两端使用相同的掩码逻辑（非空则替换为 `"***"`，空则保留空串），避免前端展示逻辑分裂。
3. **可测试性**：单测锁死了出站数据的明文内容，回归成本低（model.rs 行339-404 测试 `masked_for_display` 相关契约，config_state.rs 行522-827 测试涵盖掩码、加密落盘与回退语义）。

### 备选方案
- **手动打码**：在每个 IPC 命令返回前手动构造匿名结构体。缺点：字段多时极易遗漏，且维护成本高。
- **全量加密传输**：前端解密开销大，且密钥管理复杂，不符合 Web 前端安全模型。

## Key Components

### 桌面端（tauri-app/src-tauri/src/config/model.rs）

- **常量定义**：
  - `PASSWORD_MASK: &str = "***"`（行3）：统一掩码字符串。
  - `AUTO_DETECT_ADAPTER: &str = "自动检测"`（行4）：适配器默认值。

- **核心结构体 `Config`**（行6-182）：
  容器级 `#[serde(default)]`（行9）保证旧版本配置或缺失字段时反序列化不失败，而是填充 `Default`（行244-320）。全部字段如下表（行10-182）：

  | Rust 字段 | JSON 名 | 行号 | 类型 | 默认/备注 |
  |---|---|---|---|---|
  | `user` | `user` | 11 | `String` | 无 |
  | `password` | `password` | 12-13 | `String` | `#[serde(default)]` 空串，掩码目标 |
  | `self_hello_enabled` | `selfHelloEnabled` | 17-18 | `bool` | `default_true` (true) |
  | `self_reverify_each_action` | `selfReverifyEachAction` | 20-21 | `bool` | `default` (false) |
  | `operator` | `operator` | 22 | `String` | 无 |
  | `adapter1` | `adapter1` | 23 | `String` | 默认 `AUTO_DETECT_ADAPTER` (行252) |
  | `adapter2` | `adapter2` | 24 | `String` | 无 |
  | `adapter1_account` | `adapter1Account` | 27-28 | `String` | 无 |
  | `adapter2_account` | `adapter2Account` | 30-31 | `String` | 无 |
  | `dual_adapter` | `dualAdapter` | 32-33 | `bool` | 无 (默认 false, 行256) |
  | `auto_login_on_start` | `autoLoginOnStart` | 34-35 | `bool` | 无 (默认 true, 行257) |
  | `auto_exit_after_login` | `autoExitAfterLogin` | 36-37 | `bool` | 无 (默认 false, 行260) |
  | `minimize_to_tray` | `minimizeToTray` | 38-39 | `bool` | 无 (默认 false, 行261) |
  | `lightweight_mode` | `lightweightMode` | 42-43 | `bool` | `default_true` (true, 行262) |
  | `hidden_start` | `hiddenStart` | 44-45 | `bool` | 无 (默认 false, 行263) |
  | `auto_launch` | `autoLaunch` | 46-47 | `bool` | 无 (默认 true, 行264) |
  | `enable_background_check` | `enableBackgroundCheck` | 48-49 | `bool` | 无 (默认 true, 行265) |
  | `background_check_interval` | `backgroundCheckInterval` | 50-51 | `u64` | 无 (默认 60000, 行267) |
  | `auto_login_on_preparation` | `autoLoginOnPreparation` | 52-53 | `bool` | 无 (默认 true, 行268) |
  | `enable_night_operator_switch` | `enableNightOperatorSwitch` | 56-57 | `bool` | `default` (false, 行271) |
  | `night_operator_restore` | `nightOperatorRestore` | 59-60 | `String` | `default` 空 (行272) |
  | `enable_night_outbound_switch` | `enableNightOutboundSwitch` | 63-64 | `bool` | `default` (false, 行273) |
  | `outbound_priority` | `outboundPriority` | 69-70 | `Vec<String>` | `default` 空 (行274) |
  | `outbound_metric_restore` | `outboundMetricRestore` | 74-75 | `String` | `default` 空 (行275) |
  | `outbound_disabled_adapters` | `outboundDisabledAdapters` | 80-81 | `String` | `default` 空 (行276) |
  | `outbound_standby_route` | `outboundStandbyRoute` | 85-86 | `String` | `default` 空 (行277) |
  | `outbound_manual_hold_day` | `outboundManualHoldDay` | 92-93 | `i32` | `default` 0 (行278) |
  | `outbound_pending_switch` | `outboundPendingSwitch` | 98-99 | `String` | `default` 空 (行279) |
  | `night_outbound_restore` | `nightOutboundRestore` | 102-103 | `String` | `default` 空 (行282) |
  | `dns_optimize_adapters` | `dnsOptimizeAdapters` | 107-108 | `Vec<String>` | `default` 空 (行280) |
  | `device_aliases` | `deviceAliases` | 112-113 | `HashMap<String, String>` | `default` 空 (行281) |
  | `auto_exit_on_online` | `autoExitOnOnline` | 114-115 | `bool` | 无 (默认 false, 行284) |
  | `theme_mode` | `themeMode` | 116-117 | `String` | 无 (默认 "dark", 行285) |
  | `enable_notification` | `enableNotification` | 118-119 | `bool` | 无 (默认 true, 行286) |
  | `active_account` | `activeAccount` | 120-121 | `String` | 无 (默认空, 行287) |
  | `display_name` | `displayName` | 124-125 | `String` | 无 (默认空, 行288) |
  | `enable_latency_test` | `enableLatencyTest` | 126-127 | `bool` | 无 (默认 true, 行289) |
  | `latency_test_interval` | `latencyTestInterval` | 128-129 | `u64` | 无 (默认 600000, 行291) |
  | `custom_theme_color` | `customThemeColor` | 130-131 | `String` | 无 (默认 "#6366f1", 行292) |
  | `default_panel` | `defaultPanel` | 132-133 | `String` | 无 (默认空, 行293) |
  | `enable_network_quality` | `enableNetworkQuality` | 134-135 | `bool` | 无 (默认 true, 行294) |
  | `skip_ttfb_in_latency` | `skipTtfbInLatency` | 136-137 | `bool` | `default_true` (true, 行295) |
  | `skip_content_in_latency` | `skipContentInLatency` | 138-139 | `bool` | `default_true` (true, 行296) |
  | `portal_url` | `portalUrl` | 140-141 | `String` | `default_portal_url` (行297) |
  | `fixed_gateway` | `fixedGateway` | 142-143 | `String` | `default_fixed_gateway` (行298) |
  | `required_network_name` | `requiredNetworkName` | 144-145 | `String` | `default_required_network_name`, `deserialize_with` (行299) |
  | `enable_network_name_check` | `enableNetworkNameCheck` | 146-147 | `bool` | `default_true` (true, 行300) |
  | `campus_gateway` | `campusGateway` | 148-149 | `String` | `default_campus_gateway`, `deserialize_with` (行301) |
  | `update_source` | `updateSource` | 151-152 | `String` | `default` ("mirror", 行302) |
  | `campus_exit_on_fail` | `campusExitOnFail` | 153-154 | `bool` | `default_true` (true, 行303) |
  | `campus_exit_start_minutes` | `campusExitStartMinutes` | 156-157 | `u16` | `default_campus_exit_start_minutes` (480, 行304) |
  | `campus_exit_end_minutes` | `campusExitEndMinutes` | 159-160 | `u16` | `default_campus_exit_end_minutes` (1380, 行305) |
  | `campus_check_start_minutes` | `campusCheckStartMinutes` | 161-162 | `u16` | `alias="campusCheckStartHour"`, `default_campus_check_start_minutes` (460, 行306) |
  | `campus_check_end_minutes` | `campusCheckEndMinutes` | 164-165 | `u16` | `default_campus_check_end_minutes` (1380, 行307) |
  | `scheduled_login_minutes` | `scheduledLoginMinutes` | 167-168 | `u16` | `default` (1440=禁用哨兵, 行310) |
  | `scheduled_logout_minutes` | `scheduledLogoutMinutes` | 170-171 | `u16` | `default` (1440=禁用哨兵, 行311) |
  | `log_retention_days` | `logRetentionDays` | 172-173 | `u32` | `default_log_retention_days` (7, 行312) |
  | `max_disconnect_reconnect` | `maxDisconnectReconnect` | 174-175 | `u32` | `default_max_disconnect_reconnect` (3, 行313) |
  | `auto_login_cooldown_secs` | `autoLoginCooldownSecs` | 176-177 | `u64` | `default_auto_login_cooldown_secs` (60, 行314) |
  | `skip_sha256_when_missing` | `skipSha256WhenMissing` | 178-179 | `bool` | `default` (false, 行315) |
  | `config_version` | `configVersion` | 180-181 | `u32` | `default` (5, 行317) |

- **核心方法 `impl Config`**（行322-337）：
  - `masked_for_display(&self) -> Config`（行326-330）：**唯一出站入口**。克隆自身后调用 `mask_in_place` 并返回副本，确保内存中的原始 `Config` 不被破坏。
  - `mask_in_place(&mut self)`（行332-336）：就地执行掩码。若 `password` 非空，则替换为 `PASSWORD_MASK`（行3）。注意此方法为 `pub`，任何调用方均可破坏内存明文，故严禁直接调用，必须通过 `masked_for_display` 的 clone 路径访问。

- **默认值与辅助函数**：
  - `impl Default for Config`（行244-320）：提供全部字段默认值。
  - 多个 `default_*` 函数（行210-242）供 serde 使用，如 `default_true` (行210)、`default_campus_check_start_minutes` (行212)、`default_campus_check_end_minutes` (行216，注释提及 2026-09-13 起 1380=23:00)、`default_campus_exit_start_minutes` (行218)、`default_campus_exit_end_minutes` (行220)、`default_fixed_gateway` (行222-224)、`default_log_retention_days` (行226)、`default_max_disconnect_reconnect` (行228)、`default_auto_login_cooldown_secs` (行230)、`default_portal_url` (行232-234)、`default_required_network_name` (行236-238)、`default_campus_gateway` (行240-242)。
  - 反序列化辅助 `deserialize_non_empty_or` (行184-194) 及特定包装 `deserialize_campus_gateway` (行196-201)、`deserialize_required_network_name` (行203-208)。

- **测试模块**（行339-404）：
  - `default_reconnect_and_cooldown_config` (行343-348)
  - `serde_missing_reconnect_field_uses_default` (行350-360)
  - `serde_custom_reconnect_config` (行362-370)
  - `serde_account_fields_json_names_and_defaults` (行374-403)

### 安卓端（android/src-tauri/src/config_state.rs）

- **常量与静态**：
  - `PASSWORD_MASK: &str = "***"` (行10)
  - `CONFIG_FILE: &str = "config.json"` (行11)
  - `CONFIG_IO_LOCK` (行17-19, `lazy_static` 异步互斥锁，定义在 config_state 模块，供 save_config / switch / delete 等并发保护)

- **核心结构体 `Settings`**（行26-128，`#[serde(rename_all = "camelCase", default)]`）：
  对齐桌面 `Config` 子集。全部字段如下表：

  | Rust 字段 | JSON 名 | 行号 | 类型 | 默认/备注 |
  |---|---|---|---|---|
  | `user` | `user` | 30 | `String` | 无 (默认空, 行133) |
  | `password` | `password` | 31 | `String` | 敏感字段，落盘前加密 (默认空, 行134) |
  | `self_hello_enabled` | `selfHelloEnabled` | 32 | `bool` | 默认 true (行135) |
  | `self_reverify_each_action` | `selfReverifyEachAction` | 33 | `bool` | 默认 false (行136) |
  | `allow_2d_face_verify` | `allow2dFaceVerify` | 36 | `bool` | 默认 false (行137) |
  | `operator` | `operator` | 37 | `String` | 无 (默认空, 行138) |
  | `enable_night_operator_switch` | `enableNightOperatorSwitch` | 41 | `bool` | 默认 false (行141，注释提及 2026-10-02 起改回关闭) |
  | `night_operator_restore` | `nightOperatorRestore` | 43 | `String` | 默认空 (行142) |
  | `enable_night_outbound_switch` | `enableNightOutboundSwitch` | 47 | `bool` | 默认 false (行143) |
  | `outbound_priority` | `outboundPriority` | 50 | `Vec<String>` | 默认空 (行144) |
  | `outbound_metric_restore` | `outboundMetricRestore` | 53 | `String` | 默认空 (行145) |
  | `outbound_disabled_adapters` | `outboundDisabledAdapters` | 56 | `String` | 默认空 (行146) |
  | `outbound_standby_route` | `outboundStandbyRoute` | 59 | `String` | 默认空 (行147) |
  | `night_outbound_restore` | `nightOutboundRestore` | 64 | `String` | 默认空 (行148) |
  | `dns_optimize_adapters` | `dnsOptimizeAdapters` | 67 | `Vec<String>` | 默认空 (行149) |
  | `device_aliases` | `deviceAliases` | 71-72 | `HashMap<String, String>` | `serde(default)` 空 (行150) |
  | `auto_login_on_start` | `autoLoginOnStart` | 74 | `bool` | 默认 true (行151) |
  | `enable_background_check` | `enableBackgroundCheck` | 75 | `bool` | 默认 true (行152) |
  | `background_check_interval` | `backgroundCheckInterval` | 76 | `u64` | 默认 60000 (行155) |
  | `background_check_idle_interval` | `backgroundCheckIdleInterval` | 81 | `u64` | 默认 300000 (行157) |
  | `auto_login_on_preparation` | `autoLoginOnPreparation` | 82 | `bool` | 默认 true (行158) |
  | `max_disconnect_reconnect` | `maxDisconnectReconnect` | 83 | `u32` | 默认 3 (行159) |
  | `auto_login_cooldown_secs` | `autoLoginCooldownSecs` | 84 | `u64` | 默认 60 (行160) |
  | `theme_mode` | `themeMode` | 86 | `String` | 默认 "dark" (行161) |
  | `enable_notification` | `enableNotification` | 87 | `bool` | 默认 true (行162) |
  | `custom_theme_color` | `customThemeColor` | 88 | `String` | 默认 "#6366f1" (行163) |
  | `default_panel` | `defaultPanel` | 89 | `String` | 默认 "dashboard" (行164) |
  | `display_name` | `displayName` | 93 | `String` | 默认空 (行165) |
  | `active_account` | `activeAccount` | 94 | `String` | 默认空 (行166) |
  | `enable_boot_autostart` | `enableBootAutostart` | 95 | `bool` | 默认 false (行167) |
  | `enable_latency_test` | `enableLatencyTest` | 97 | `bool` | 默认 false (行168) |
  | `latency_test_interval` | `latencyTestInterval` | 98 | `u64` | 默认 600000 (行170) |
  | `enable_network_quality` | `enableNetworkQuality` | 99 | `bool` | 默认 false (行174) |
  | `skip_ttfb_in_latency` | `skipTtfbInLatency` | 100 | `bool` | 默认 true (行175) |
  | `skip_content_in_latency` | `skipContentInLatency` | 101 | `bool` | 默认 true (行176) |
  | `portal_url` | `portalUrl` | 103 | `String` | 默认 "http://10.1.99.100" (行177) |
  | `fixed_gateway` | `fixedGateway` | 104 | `String` | 默认 "10.2.127.254" (行178) |
  | `required_network_name` | `requiredNetworkName` | 105 | `String` | 默认三 SSID "i-wxxy、iwxxy-2、iwxxy-3" (行181) |
  | `enable_network_name_check` | `enableNetworkNameCheck` | 106 | `bool` | 默认 true (行182) |
  | `campus_gateway` | `campusGateway` | 107 | `String` | 默认 "10.2.127.254" (行183) |
  | `campus_check_start_minutes` | `campusCheckStartMinutes` | 109 | `u16` | 默认 460 (行185) |
  | `campus_check_end_minutes` | `campusCheckEndMinutes` | 111 | `u16` | 默认 1380 (行188) |
  | `scheduled_login_minutes` | `scheduledLoginMinutes` | 113 | `u16` | 默认 1440=禁用哨兵 (行192) |
  | `scheduled_logout_minutes` | `scheduledLogoutMinutes` | 115 | `u16` | 默认 1440=禁用哨兵 (行193) |
  | `update_source` | `updateSource` | 118 | `String` | 默认 "mirror" (行194) |
  | `log_retention_days` | `logRetentionDays` | 120 | `u32` | 默认 7 (行195) |
  | `config_schema_version` | `configSchemaVersion` | 127 | `u32` | 默认 7 (行197)，迁移版本 |

- **加密与存储组件**：
  - `struct CryptoBridge` (行203-207, `Clone`) 及 `impl CryptoBridge::from_app` (行210-239)：真机走 `CampusKeystoreExt` (行213)，Host 测试注入假实现 (行235)。
  - `struct EncodedSettings` (行243-248)：磁盘存储形态，含 `settings` (行246) 和 `password_cipher` (行247)。落盘时 `settings.password` 被清空 (行293)。
  - `load_file` (行250-263)：负责 JSON 读取与解密。
  - `password_saved_at` (行270-276)：判断密码是否已保存必须查密文位 `password_cipher`，不能看外露 `settings.password`。
  - `save_file` (行278-308)：加密落盘，使用带 PID 的临时文件原子写入 (行300-306)。
  - `cleanup_stale_config_tmp` (行313-324)：启动时清理遗留临时文件。
  - `load_from` (行326-336)：加载配置并执行迁移，含 SSID 单值扩三值 (行332-334)。
  - `migrate_legacy_defaults` (行355-417)：根据 `config_schema_version` 执行 v0→v7 迁移。
  - `save_to` (行419-421)：封装 `save_file`。

- **掩码与广播（唯一出口相关）**：
  - `masked_for_display(s: &Settings) -> serde_json::Value` (行424-430)：**唯一出站入口**。克隆并掩码后序列化为 JSON Value。
  - `emit_config_changed(app, s)` (行436-439)：广播 `config-changed` 事件，Payload 为 `{ "config": masked_for_display(s) }`。
  - `resolve_password_field(incoming, current, clear)` (行442-451)：处理密码更新逻辑。`clear=true` 置空；空或 `"***"` 保留 `current`；否则采用 `incoming`。
  - `#[tauri::command] get_config` (行453-466) 与 `save_config` (行468-499)：命令层，全程持 `config_io_lock` (行477) 并调用掩码出口。
  - `current_settings` (行503-520)：取当前配置供回退。

- **测试模块**（行522-827）：涵盖加密落盘、解密失败、SSID 迁移、默认值语义、掩码出口等，如 `加密落盘_文件不含明文_读回等于原文` (行586-598)、`掩码出口_非空变星号_空保持空` (行768-777)、`密码已保存判据_落盘后仍为已保存` (行809-826)。

## Architecture

```text
┌──────────────────────────────┐   ┌──────────────────────────────┐
│ 桌面 Config (model.rs:6-182) │   │ 安卓 Settings (cfg_state:26-128)│
└──────────────┬───────────────┘   └──────────────┬───────────────┘
               │ masked_for_display (model.rs:326)│ masked_for_display (cfg_state:424)
               ▼                                  ▼
┌──────────────────────────────┐   ┌──────────────────────────────┐
│ 掩码 Config (password:"***")  │   │ 掩码 JSON (password:"***")     │
└──────────────┬───────────────┘   └──────────────┬───────────────┘
               ▼                                  ▼
┌──────────────────────────────┐   ┌──────────────────────────────┐
│ IPC 返回 / 前端渲染           │   │ emit_config_changed (cfg_state:436)│
└──────────────────────────────┘   └──────────────┬───────────────┘
                                                  │ save_file 加密
                                                  ▼
┌──────────────────────────────┐   ┌──────────────────────────────┐
│                              │◄──│ EncodedSettings (cfg_state:243)│
└──────────────────────────────┘   └──────────────────────────────┘
```

## Data Flow

- **出站脱敏流（桌面）**：任何 IPC 命令需发配置给前端时，调用 `Config::masked_for_display()` (model.rs 行326-330)，内部 `clone` 后 `mask_in_place` (行332-336) 将非空 `password` 替换为 `PASSWORD_MASK` (行3)，返回新副本，原始内存不变。
- **出站脱敏流（安卓）**：命令层（如 `get_config` 行453-466 或 `save_config` 行468-499）调用 `masked_for_display` (行424-430) 返回 `serde_json::Value`，或经 `emit_config_changed` (行436-439) 广播 `{ "config": masked_for_display(s) }` 事件，前端依赖该事件同步 Store。
- **密码更新流**：前端传 `password` 字段。若为 `"***"` 或空串，安卓经 `resolve_password_field` (行442-451) 判据回退已存值；`clear=true` 显式清除。桌面由结构体内存语义隐式处理。
- **落盘加密流（安卓）**：`save_file` (行278-308) 中若 `password` 非空，经 `CryptoBridge::encrypt` (行217) 加密存入 `password_cipher` (行247)，同时 `settings.password` 清空 (行293)。`load_file` (行250-263) 解密回填外露字段。

## Connections

- 桌面与安卓配置结构对齐与脱敏契约见 [[tauri-app/src-tauri/src/config/model.rs]] 与 [[android/src-tauri/src/config_state.rs]]。
- 配置广播驱动前端状态同步见 [[frontend-store-sync]]（对应 `config-changed` 事件）。
- 配置 IO 锁与账号命令互斥见 [[account-commands]]（对应 `account_cmds.rs` 迁入的 `CONFIG_IO_LOCK`）。
- 敏感信息保护设计总览见 [[security-architecture]]。

## Known Issues

### 已知漏洞面
1. **桌面端 `mask_in_place` 可见性**：`Config::mask_in_place` 是 `pub` 方法（model.rs 行332-336），类型系统不阻止调用方直接修改内存中的明文配置。虽然当前代码仅通过 `masked_for_display` 间接调用，但若未来有内部逻辑误用该函数，可能导致内存残留明文。建议后续考虑将其降级为 `pub(crate)` 或私有，或通过更严格的封装限制访问。
2. **安卓端密码加密失败**：若 `AndroidKeyStore` 加密失败，`save_file` 会直接返回错误（config_state.rs 行289，匹配 `Err(e)` 分支打 stderr 并返回 `Err(format!("password encrypt: {e}"))`），导致配置保存失败。这在密钥库损坏或硬件异常时可能发生，需确保前端有相应的错误提示。