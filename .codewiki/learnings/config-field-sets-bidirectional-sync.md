---
title: 两端配置字段集不是子集关系，而是交集 + 双向差集
type: learning
source_files:
  - tauri-app/src-tauri/src/config/model.rs
  - android/src-tauri/src/config_state.rs
tags: [教训, 双端, 配置, 同步]
---

## Overview

新增配置字段时无法照抄某一端——按“双端同步”的直觉去抄会抄错。实读当前代码（提供的 `tauri-app/src-tauri/src/config/model.rs` 与 `android/src-tauri/src/config_state.rs`），桌面端 `Config`（model.rs:10-193）与安卓端 `Settings`（config_state.rs:26-136）是两个独立定义的结构体，字段集既不是子集也不是超集，而是交集叠加双向差集。桌面共 64 字段，安卓共 50 字段，交集 46 字段，桌面独有 18 字段，安卓独有 4 字段。任何一端新增字段都必须先判断平台归属与消费侧差异，否则会导致双端契约不一致、反序列化失败或功能静默失效。

## Key Components

### tauri-app/src-tauri/src/config/model.rs

常量与函数：

- `PASSWORD_MASK` (model.rs:3)：密码掩码占位符 `"***"`。
- `AUTO_DETECT_ADAPTER` (model.rs:4)：自动检测适配器常量 `"自动检测"`。
- `deserialize_non_empty_or` (model.rs:195-205)：反序列化辅助，空串回退默认函数。
- `deserialize_campus_gateway` (model.rs:207-212)：campus_gateway 空串回退。
- `deserialize_required_network_name` (model.rs:214-219)：required_network_name 空串回退。
- `default_true` (model.rs:221)：返回 `true` 的 serde 默认函数。
- `default_campus_check_start_minutes` (model.rs:223)：返回 `460`。
- `default_campus_check_end_minutes` (model.rs:227)：返回 `1380`（2026-09-13 起）。
- `default_campus_exit_start_minutes` (model.rs:229)：返回 `480`。
- `default_campus_exit_end_minutes` (model.rs:231)：返回 `1380`。
- `default_fixed_gateway` (model.rs:233-235)：返回 `"10.2.127.254"`。
- `default_log_retention_days` (model.rs:237)：返回 `7`。
- `default_max_disconnect_reconnect` (model.rs:239)：返回 `3`。
- `default_auto_login_cooldown_secs` (model.rs:241)：返回 `60`。
- `default_portal_url` (model.rs:243-245)：返回 `"http://10.1.99.100"`。
- `default_required_network_name` (model.rs:247-249)：返回 `"i-wxxy、iwxxy-2、iwxxy-3"`。
- `default_campus_gateway` (model.rs:251-253)：返回 `"10.2.127.254"`。
- `impl Default for Config` (model.rs:255-335)：桌面全量默认值（config_version=5 在 model.rs:332）。
- `impl Config` (model.rs:341-361)：`masked_for_display` (model.rs:341-345) 与 `mask_in_place` (model.rs:347-356) 密码掩码唯一出口。
- `#[cfg(test)] mod tests` (model.rs:359-424)：含 serde 契约锁测试（model.rs:394-423 锁 displayName/adapter1Account/adapter2Account）。

`Config` struct 全字段（model.rs:10-193，容器级 `#[serde(default)]` 在 model.rs:9，共 64 字段）：

| 行号 | 字段 | 双端归属 |
| --- | --- | --- |
| 11 | user | 交集 |
| 12-13 | password | 交集 |
| 14-18 | self_hello_enabled | 交集 |
| 19-21 | self_reverify_each_action | 交集 |
| 22 | operator | 交集 |
| 23 | adapter1 | 桌面独有 |
| 24 | adapter2 | 桌面独有 |
| 25-28 | adapter1_account | 桌面独有 |
| 29-31 | adapter2_account | 桌面独有 |
| 32-33 | dual_adapter | 桌面独有 |
| 34-35 | auto_login_on_start | 交集 |
| 36-37 | auto_exit_after_login | 桌面独有 |
| 38-39 | minimize_to_tray | 桌面独有 |
| 40-43 | lightweight_mode | 桌面独有 |
| 44-45 | hidden_start | 桌面独有 |
| 46-47 | auto_launch | 桌面独有 |
| 48-49 | enable_background_check | 交集 |
| 50-51 | background_check_interval | 交集 |
| 52-53 | auto_login_on_preparation | 交集 |
| 54-57 | enable_night_operator_switch | 交集 |
| 58-60 | night_operator_restore | 交集 |
| 61-64 | enable_night_outbound_switch | 交集 |
| 65-70 | outbound_priority | 交集 |
| 71-75 | outbound_metric_restore | 交集 |
| 76-81 | outbound_disabled_adapters | 交集 |
| 82-86 | outbound_standby_route | 交集 |
| 87-93 | outbound_manual_hold_day | 桌面独有 |
| 94-99 | outbound_pending_switch | 桌面独有 |
| 100-103 | night_outbound_restore | 交集 |
| 104-108 | dns_optimize_adapters | 交集 |
| 109-113 | device_aliases | 交集 |
| 114-115 | auto_exit_on_online | 桌面独有 |
| 116-117 | theme_mode | 交集 |
| 118-119 | enable_notification | 交集 |
| 120-121 | active_account | 交集 |
| 122-125 | display_name | 交集 |
| 126-127 | enable_latency_test | 交集 |
| 128-129 | latency_test_interval | 交集 |
| 130-131 | custom_theme_color | 交集 |
| 132-133 | default_panel | 交集 |
| 134-135 | enable_network_quality | 交集 |
| 136-137 | skip_ttfb_in_latency | 交集 |
| 138-139 | skip_content_in_latency | 交集 |
| 140-141 | portal_url | 交集 |
| 142-143 | fixed_gateway | 交集 |
| 144-145 | required_network_name | 交集 |
| 146-147 | enable_network_name_check | 交集 |
| 148-149 | campus_gateway | 交集 |
| 150-152 | update_source | 交集 |
| 153-154 | campus_exit_on_fail | 桌面独有 |
| 155-157 | campus_exit_start_minutes | 桌面独有 |
| 158-160 | campus_exit_end_minutes | 桌面独有 |
| 161-162 | campus_check_start_minutes | 交集 |
| 163-165 | campus_check_end_minutes | 交集 |
| 166-168 | scheduled_login_minutes | 交集 |
| 169-171 | scheduled_logout_minutes | 交集 |
| 172-173 | log_retention_days | 交集 |
| 174-175 | max_disconnect_reconnect | 交集 |
| 176-177 | auto_login_cooldown_secs | 交集 |
| 178-179 | skip_sha256_when_missing | 桌面独有 |
| 180-182 | vpn_account | 交集 |
| 183-186 | vpn_password_enc | 交集 |
| 187-190 | self_service_via_vpn | 交集 |
| 191-192 | config_version | 桌面独有 |

### android/src-tauri/src/config_state.rs

常量与函数：

- `PASSWORD_MASK` (config_state.rs:10)、`CONFIG_FILE` (config_state.rs:11)。
- `CONFIG_IO_LOCK` (config_state.rs:17-19)：tokio 异步互斥锁。
- `config_io_lock` (config_state.rs:22-24)：获取 IO 锁 guard。
- `Settings` struct (config_state.rs:26-136)，`#[serde(rename_all = "camelCase", default)]` (config_state.rs:27)。
- `impl Default for Settings` (config_state.rs:138-211)：安卓全量默认值（config_schema_version=7 在 config_state.rs:208）。
- `CryptoBridge` (config_state.rs:214-218) 与 `from_app` (config_state.rs:221-251)：AndroidKeyStore 加解密桥。
- `EncodedSettings` (config_state.rs:254-259)：落盘形态，密码只留密文。
- `load_file` (config_state.rs:261-274)：读盘解密。
- `password_saved_at` (config_state.rs:281-287)：判据落在密文位。
- `save_file` (config_state.rs:289-319)：加密落盘、临时文件 rename。
- `cleanup_stale_config_tmp` (config_state.rs:324-335)：清理崩溃遗留 tmp。
- `load_from` (config_state.rs:337-347)：触发迁移与 SSID 单值扩三值。
- `migrate_legacy_defaults` (config_state.rs:366-428)：v0→v7 一次性迁移。
- `save_to` (config_state.rs:430-432)。
- `masked_for_display` (config_state.rs:437-446)：掩码出口。
- `emit_config_changed` (config_state.rs:452-455)：广播事件。
- `resolve_password_field` (config_state.rs:458-467)：空串/MASK 回退已存值。
- `#[tauri::command] get_config` (config_state.rs:470-482)。
- `#[tauri::command] save_config` (config_state.rs:485-520)。
- `current_settings` (config_state.rs:524-541，仅 `#[allow(dead_code)]` 非 `#[tauri::command]`)。
- `#[cfg(test)] mod tests` (config_state.rs:543-897)：含加密落盘、迁移、SSID 扩值等测试。

`Settings` struct 全字段（config_state.rs:26-136，共 50 字段，其中 46 个交集、4 个独有）：

| 行号 | 字段 | 双端归属 |
| --- | --- | --- |
| 30 | user | 交集 |
| 31 | password | 交集 |
| 32 | self_hello_enabled | 交集 |
| 33 | self_reverify_each_action | 交集 |
| 34-36 | allow_2d_face_verify | 安卓独有 |
| 37 | operator | 交集 |
| 38-41 | enable_night_operator_switch | 交集 |
| 42-43 | night_operator_restore | 交集 |
| 44-47 | enable_night_outbound_switch | 交集 |
| 48-50 | outbound_priority | 交集 |
| 51-53 | outbound_metric_restore | 交集 |
| 54-56 | outbound_disabled_adapters | 交集 |
| 57-59 | outbound_standby_route | 交集 |
| 60-64 | night_outbound_restore | 交集 |
| 65-67 | dns_optimize_adapters | 交集 |
| 68-72 | device_aliases | 交集 |
| 73-74 | auto_login_on_start | 交集 |
| 75 | enable_background_check | 交集 |
| 76 | background_check_interval | 交集 |
| 77-81 | background_check_idle_interval | 安卓独有 |
| 82 | auto_login_on_preparation | 交集 |
| 83 | max_disconnect_reconnect | 交集 |
| 84 | auto_login_cooldown_secs | 交集 |
| 85-86 | theme_mode | 交集 |
| 87 | enable_notification | 交集 |
| 88 | custom_theme_color | 交集 |
| 89 | default_panel | 交集 |
| 90-93 | display_name | 交集 |
| 94 | active_account | 交集 |
| 95 | enable_boot_autostart | 安卓独有 |
| 96-97 | enable_latency_test | 交集 |
| 98 | latency_test_interval | 交集 |
| 99 | enable_network_quality | 交集 |
| 100 | skip_ttfb_in_latency | 交集 |
| 101 | skip_content_in_latency | 交集 |
| 102-103 | portal_url | 交集 |
| 104 | fixed_gateway | 交集 |
| 105 | required_network_name | 交集 |
| 106 | enable_network_name_check | 交集 |
| 107 | campus_gateway | 交集 |
| 108-109 | campus_check_start_minutes | 交集 |
| 110-111 | campus_check_end_minutes | 交集 |
| 112-113 | scheduled_login_minutes | 交集 |
| 114-115 | scheduled_logout_minutes | 交集 |
| 116-118 | update_source | 交集 |
| 119-120 | log_retention_days | 交集 |
| 121-124 | vpn_account | 交集 |
| 125 | vpn_password_enc | 交集 |
| 126-128 | self_service_via_vpn | 交集 |
| 129-135 | config_schema_version | 安卓独有 |

## Architecture

```text
┌────────────────────────────┐         ┌────────────────────────────┐
│ 桌面 Config (model.rs:10)  │         │ 安卓 Settings              │
│ 64 字段                    │         │ (config_state.rs:26) 50    │
└─────────────┬──────────────┘         └─────────────┬──────────────┘
              │ 交集 46 字段 (camelCase 同契约)        │
              └──────────────►◄───────────────────────┘
              │                                      │
┌─────────────▼──────────────┐         ┌─────────────▼──────────────┐
│ 桌面独有 18 字段             │         │ 安卓独有 4 字段             │
│ adapter1, lightweight_mode, │         │ allow_2d_face_verify,      │
│ config_version...          │         │ background_check_idle...   │
└────────────────────────────┘         └────────────────────────────┘
```

夜切/出站切换镜像字段族 9 件双端同加但多不消费：`enable_night_operator_switch` (model.rs:54-57 / config_state.rs:38-41)、`night_operator_restore` (model.rs:58-60 / config_state.rs:42-43)、`enable_night_outbound_switch` (model.rs:61-64 / config_state.rs:44-47)、`outbound_priority` (model.rs:65-70 / config_state.rs:48-50)、`outbound_metric_restore` (model.rs:71-75 / config_state.rs:51-53)、`outbound_disabled_adapters` (model.rs:76-81 / config_state.rs:54-56)、`outbound_standby_route` (model.rs:82-86 / config_state.rs:57-59)、`night_outbound_restore` (model.rs:100-103 / config_state.rs:60-64)、`dns_optimize_adapters` (model.rs:104-108 / config_state.rs:65-67)。字段存在 ≠ 字段消费：安卓端不消费 `outbound_priority` (config_state.rs:48-50)、`outbound_metric_restore` (config_state.rs:51-53)、`outbound_disabled_adapters` (config_state.rs:54-56)、`outbound_standby_route` (config_state.rs:57-59)、`dns_optimize_adapters` (config_state.rs:65-67)；`night_outbound_restore` 反向——桌面落盘但桌面端不消费 (model.rs:100-103)。此外 VPN 字段族 `vpn_account`/`vpn_password_enc`/`self_service_via_vpn` 双端均有且消费（安卓经 AndroidKeyStore 密文，桌面经 DPAPI 密文）。

## Data Flow

双端配置读写路径：

- 安卓读取：`get_config` (config_state.rs:470) → `load_from` (config_state.rs:337) → `load_file` (config_state.rs:261) 解密 → `migrate_legacy_defaults` (config_state.rs:366) 及 SSID 单值扩三值 (config_state.rs:470-472) → `masked_for_display` (config_state.rs:437)。
- 安卓保存：`save_config` (config_state.rs:485) 持 `config_io_lock` (config_state.rs:22) → `load_from` 合并 → `resolve_password_field` (config_state.rs:458) → `save_to` (config_state.rs:430) → `save_file` 加密落盘 `EncodedSettings` (config_state.rs:254, 289) → `emit_config_changed` (config_state.rs:452)。
- 桌面读取/保存：`Config` 经 serde 容器级 `#[serde(default)]` (model.rs:341) 兜底反序列化，缺字段整体不失败；掩码经 `masked_for_display` (model.rs:341)。

历史脉络：2026-09-13 双端同加 `scheduled_login_minutes`/`scheduled_logout_minutes` (model.rs:166-171 / config_state.rs:112-115)；2026-09-14 双端同加 `display_name` (model.rs:122-125 / config_state.rs:90-93)、桌面另加 `adapter1_account`/`adapter2_account` (model.rs:25-31)；2026-10-01 安卓 `required_network_name` 对齐桌面三 SSID (config_state.rs:187-189 ↔ model.rs:247-249)；2026-10-02 双端同改夜切开关默认回关 (model.rs:282 / config_state.rs:149，存量已落盘 true 不迁移)；2026-10-25 桌面新增 VPN 相关字段 `vpn_account`/`vpn_password_enc`/`self_service_via_vpn` (model.rs:180-190)，后续安卓也加入对齐（当前 config_state.rs:121-128 同契约，默认 self_service_via_vpn: true 在 config_state.rs:206）。

“双端同步”按**双向增量同步**执行：先判断字段属于两端通用还是平台专属，再决定是否两端各加一份。通用字段（如夜切族）双端同加但注明消费侧差异；平台专属字段（如 `lightweight_mode` 仅桌面 model.rs:40-43、`allow_2d_face_verify` 仅安卓 config_state.rs:34-36）只加一端。

## Connections

[[dual-tree-sync-human-discipline]]、[[android-interval-default-schema-migration]]、[[config-missing-field-load-failure]]、[[adapter-account-binding]]

## Known Issues

① 新增配置字段时对照 `config/model.rs` 与 `config_state.rs` 两份定义，别假设一端是另一端的超集（现状 64 vs 50，交集 46，桌面独有 18，安卓独有 4）。

② 改默认值要走后端 `Settings::default` +（安卓）schema 迁移（`migrate_legacy_defaults` 在 config_state.rs:366-428），且两端默认值允许合理分叉：`default_panel` 安卓 `"dashboard"` (config_state.rs:172) vs 桌面空串 (model.rs:304)；`enable_latency_test` 桌面 true (model.rs:300) vs 安卓 false (config_state.rs:176)；`enable_network_quality` 桌面 true (model.rs:305) vs 安卓 false (config_state.rs:182)；`lightweight_mode` 桌面 true (model.rs:273) 安卓无；其余如 `background_check_interval` 两端均为 60000 (model.rs:278 / config_state.rs:163)，`latency_test_interval` 两端均为 600000 (model.rs:302 / config_state.rs:178)，VPN 开关 `self_service_via_vpn` 两端均默认 true (model.rs:330 / config_state.rs:206)。

③ 不要混淆 `config_version`（桌面，当前默认 5，model.rs:332，迁移在 validate.rs）与 `config_schema_version`（安卓，当前默认 7，config_state.rs:208，迁移 `migrate_legacy_defaults` 在 config_state.rs:366-428），两套版本号各自独立演进、迁移阶梯互不相同。

④ 镜像字段“存在但不消费”是常态（安卓镜像桌面夜切状态、桌面忽略安卓标记），判断字段语义必须读字段注释，不能只看字段名在不在两端。