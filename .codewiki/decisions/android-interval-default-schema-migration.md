---
title: 安卓配置状态管理 config_state.rs
type: module
source_files: []
tags: [安卓, 配置, 状态机, 加密, Tauri]
---

## Overview

`config_state.rs` 实现了安卓端的全量配置持久化与运行时状态管理。其核心职责包括：从磁盘加载/保存 `Settings` 结构体、通过 `CryptoBridge` 对密码字段进行加解密（真机使用 AndroidKeyStore，Host 测试使用可逆假实现）、执行基于 `config_schema_version` 的一次性旧默认值迁移、以及处理前端 IPC 命令（`get_config`, `save_config`）。

该模块设计为桌面端 `config::Config` 的子集适配层，遵循相同的 camelCase IPC 契约。所有出站配置数据必须经过 `masked_for_display` 掩码处理，确保密码等敏感信息永不以明文形式暴露给前端。

## Key Components

### 常量与类型定义

*   **`PASSWORD_MASK`** (`android/src-tauri/src/config_state.rs:13`): 字符串 `"***"`，用于在显示层掩盖密码。
*   **`CONFIG_FILE`** (`android/src-tauri/src/config_state.rs:14`): 字符串 `"config.json"`，配置文件名。
*   **`CONFIG_IO_LOCK`** (`android/src-tauri/src/config_state.rs:20-22`): `lazy_static` 定义的 `tokio::sync::Mutex<()>` 全局串行锁。用于防止 `save_config` / `switch` / `delete` 并发读改写竞态。
*   **`Settings`** (`android/src-tauri/src/config_state.rs:26-182`): 安卓端全量配置结构体。
    *   派生属性: `Serialize, Deserialize, Clone, Debug`
    *   Serde 属性: `#[serde(rename_all = "camelCase", default)]`
    *   **账号与凭据**:
        *   `user`: String
        *   `password`: String (落盘前必加密)
        *   `self_hello_enabled`: bool
        *   `self_reverify_each_action`: bool
        *   `allow_2d_face_verify`: bool (2D 人脸验证开关)
        *   `operator`: String
        *   `enable_night_operator_switch`: bool (夜间自动切换运营商开关)
        *   `night_operator_restore`: String (夜切前原运营商暂存)
        *   `enable_night_outbound_switch`: bool (夜间出站自动切换总开关)
        *   `outbound_priority`: Vec<String> (桌面消费，安卓镜像)
        *   `outbound_metric_restore`: String (桌面消费，安卓镜像)
        *   `outbound_disabled_adapters`: String (桌面消费，安卓镜像)
        *   `outbound_standby_route`: String (桌面消费，安卓镜像)
        *   `night_outbound_restore`: String (安卓出站切换态标记，值恒 "logged_out")
        *   `dns_optimize_adapters`: Vec<String> (桌面消费，安卓镜像)
        *   `device_aliases`: HashMap<String, String> (在线设备自定义名字)
    *   **行为**:
        *   `auto_login_on_start`: bool
        *   `enable_background_check`: bool
        *   `background_check_interval`: u64 (基础检测间隔，默认 60s)
        *   `background_check_idle_interval`: u64 (闲时巡检间隔，默认 5min)
        *   `auto_login_on_preparation`: bool
        *   `max_disconnect_reconnect`: u32
        *   `auto_login_cooldown_secs`: u64
    *   **界面与通知**:
        *   `theme_mode`: String
        *   `enable_notification`: bool
        *   `custom_theme_color`: String
        *   `default_panel`: String
        *   `display_name`: String (账号显示名)
        *   `active_account`: String
        *   `enable_boot_autostart`: bool
    *   **定时质量测试**:
        *   `enable_latency_test`: bool
        *   `latency_test_interval`: u64 (默认 600s)
        *   `enable_network_quality`: bool (默认 false)
        *   `skip_ttfb_in_latency`: bool
        *   `skip_content_in_latency`: bool
    *   **网络**:
        *   `portal_url`: String
        *   `fixed_gateway`: String
        *   `required_network_name`: String (SSID 名单)
        *   `enable_network_name_check`: bool
        *   `campus_gateway`: String
        *   `campus_check_start_minutes`: u16
        *   `campus_check_end_minutes`: u16
        *   `scheduled_login_minutes`: u16
        *   `scheduled_logout_minutes`: u16
    *   **更新**:
        *   `update_source`: String
    *   **日志**:
        *   `log_retention_days`: u32
        *   `config_schema_version`: u32 (当前版本 7)
*   **`Default for Settings`** (`android/src-tauri/src/config_state.rs:184-235`): 提供所有字段的默认值。
    *   `background_check_interval`: 60_000 ms
    *   `background_check_idle_interval`: 300_000 ms
    *   `latency_test_interval`: 600_000 ms
    *   `config_schema_version`: 7
    *   `enable_night_operator_switch`: false (2026-10-02 起改回关闭)
    *   `scheduled_login/logout_minutes`: 1440 (禁用哨兵)
    *   `campus_check_end_minutes`: 1380 (23:00)
*   **`CryptoBridge`** (`android/src-tauri/src/config_state.rs:238-265`): 加解密桥接器。
    *   `encrypt`: Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>
    *   `decrypt`: Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>
    *   `from_app(app: &tauri::AppHandle)`: 构造函数。真机 (`#[cfg(mobile)]`) 调用 `CampusKeystoreExt`；Host 测试返回错误。
*   **`EncodedSettings`** (`android/src-tauri/src/config_state.rs:269-273`): 磁盘存储形态。
    *   `settings`: Settings
    *   `password_cipher`: String

### 核心函数

*   **`config_io_lock`** (`android/src-tauri/src/config_state.rs:25`): 获取配置 IO 串行锁的 Guard，供外部命令持有以防竞态。
*   **`load_file`** (`android/src-tauri/src/config_state.rs:275-287`): 异步读取并解密单个配置文件。
    *   若文件不存在，返回 `Settings::default()`。
    *   解析 JSON 为 `EncodedSettings`。
    *   调用 `bridge.decrypt` 解密密码；若解密失败（如密钥变更），密码置空而非报错。
*   **`save_file`** (`android/src-tauri/src/config_state.rs:289-310`): 异步加密并保存配置。
    *   创建父目录。
    *   若密码非空，调用 `bridge.encrypt`；失败则返回错误。
    *   构造 `EncodedSettings`，将明文密码替换为空字符串，仅保留 `password_cipher`。
    *   原子写入：先写 `.json.tmp` (带 PID)，再 `rename` 覆盖目标文件。
*   **`cleanup_stale_config_tmp`** (`android/src-tauri/src/config_state.rs:312-327`): 清理进程崩溃遗留的配置临时文件 (启动时调用一次)。
*   **`load_from`** (`android/src-tauri/src/config_state.rs:329-344`): 加载配置并执行迁移。
    *   调用 `load_file`。
    *   调用 `migrate_legacy_defaults` 执行一次性默认值迁移。
    *   SSID 迁移：若 `required_network_name` 为旧单值 `"i-wxxy"`，幂等扩为三值 `"i-wxxy、iwxxy-2、iwxxy-3"`。
*   **`save_to`** (`android/src-tauri/src/config_state.rs:346-348`): 便捷封装，调用 `save_file`。
*   **`migrate_legacy_defaults`** (`android/src-tauri/src/config_state.rs:350-400`): 旧默认值一次性迁移逻辑。
    *   根据 `config_schema_version` 执行链式迁移，每次迁移后落盘。
    *   **v0→v1**: `background_check_interval` 15_000 → 60_000。
    *   **v1→v2**: 批量开启自动化登录/检测/验证/跳过项开关 (`auto_login_on_start`, `enable_background_check` 等)。
    *   **v2→v3**: 关闭质量检测 (`enable_network_quality = false`)。
    *   **v3→v4**: 补全 `background_check_idle_interval` 默认 300_000 (仅当值为 0 时)。
    *   **v4→v5**: 刷 `campus_check_end_minutes` 旧默认 0 → 1380 (仅当值为 0 时)。
    *   **v5→v6**: 刷 `latency_test_interval` 旧默认 60_000 → 600_000 (仅当值为 60_000 时)。
    *   **v6→v7**: 开启夜切开关 (`enable_night_operator_switch = true`)；刷定时动作禁用值 0 → 1440。
    *   迁移结果落盘，失败静默（下次读盘重迁，幂等）。
*   **`masked_for_display`** (`android/src-tauri/src/config_state.rs:403-409`): 生成供前端显示的 JSON Value。
    *   克隆 Settings，若密码非空则替换为 `PASSWORD_MASK` ("***")。
    *   序列化为 `serde_json::Value`。
*   **`emit_config_changed`** (`android/src-tauri/src/config_state.rs:411-418`): 广播 `config-changed` 事件。
    *   Payload: `{ "config": masked_for_display(s) }`。
    *   确保前端 Store 同步，防止后续保存覆盖后端新值。
*   **`resolve_password_field`** (`android/src-tauri/src/config_state.rs:420-428`): 密码字段合并语义。
    *   `clear=true`: 返回空串。
    *   `incoming` 为空或 `"***"`: 返回 `current` (保留已存值)。
    *   其他: 返回 `incoming` (采用新值)。
*   **Tauri Commands**:
    *   **`get_config`** (`android/src-tauri/src/config_state.rs:430-444`): 获取当前配置。
        *   从缓存或磁盘加载。
        *   更新 `AndroidState` 中的配置缓存。
        *   返回 `masked_for_display` 结果。
    *   **`save_config`** (`android/src-tauri/src/config_state.rs:446-476`): 保存配置。
        *   全程持配置 IO 锁 (`config_io_lock`)。
        *   加载当前配置作为基准。
        *   调用 `resolve_password_field` 合并密码。
        *   调用 `save_to` 落盘。
        *   更新 `AndroidState` 缓存。
        *   触发 `auto_create_account_for_current` (R2 自动建号)。
        *   调用 `emit_config_changed` 广播。
    *   **`current_settings`** (`android/src-tauri/src/config_state.rs:478-494`): 取当前配置（缓存优先，未命中读盘）。供登录/自助服务等命令回退已存凭据。

### 测试模块 (`mod tests`)

*   **`fake_bridge`** (`android/src-tauri/src/config_state.rs:502-509`): 构造基于 Base64 的可逆假加密桥，用于 Host 测试。
*   **`base64_encode` / `base64_decode`** (`android/src-tauri/src/config_state.rs:511-544`): 纯 Rust 实现的 Base64 编解码，无外部依赖。
*   **`tmp_dir`** (`android/src-tauri/src/config_state.rs:546-550`): 生成临时测试目录。
*   **`sample_settings`** (`android/src-tauri/src/config_state.rs:552-559`): 构造包含明文的测试用 Settings。
*   **测试用例**:
    *   `加密落盘_文件不含明文_读回等于原文`: 验证 roundtrip 加密/解密正确性。
    *   `解密失败时置空_配置仍可加载`: 验证密钥失效时密码置空，非敏感字段正常。
    *   `旧默认单值SSID_载入时扩为三值`: 验证 SSID 迁移逻辑。
    *   `默认值_业务语义`: 断言 `Settings::default` 所有关键字段默认值。
    *   `迁移_旧默认15s升60s且落盘后用户值不被覆盖`: 验证完整迁移链及"不覆盖用户显式值"原则。
    *   `迁移_非旧默认值保持不动`: 验证用户自定义值不受迁移影响。
    *   `迁移_v5质量间隔旧默认刷v6且用户值不被覆盖`: 验证 v5→v6 迁移及用户回滚保护。
    *   `camelCase_序列化契约`: 验证 JSON 键名符合 IPC 约定。
    *   `掩码出口_非空变星号_空保持空`: 验证 `masked_for_display`。
    *   `密码字段回退语义_与桌面同构`: 验证 `resolve_password_field` 的四种情形。
    *   `空密码不写密文位`: 验证空密码时 `passwordCipher` 字段为空串。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                     Tauri Frontend                          │
│  (IPC: get_config / save_config)                            │
└───────────────────────┬─────────────────────────────────────┘
                        │ emit("config-changed")
                        ▼
┌─────────────────────────────────────────────────────────────┐
│                  Android State Cache                        │
│  (state.config: Option<Settings>)                           │
└───────────────────────┬─────────────────────────────────────┘
                        │
┌───────────────────────▼─────────────────────────────────────┐
│                 config_state.rs                             │
│                                                             │
│  ┌──────────────┐     ┌──────────────────┐                  │
│  │  Commands    │────▶│ migrate_legacy   │                  │
│  │ get_config   │     │ defaults         │                  │
│  │ save_config  │     │ (v0→v7 chain)    │                  │
│  └──────────────┘     └────────┬─────────┘                  │
│          │                     │                             │
│          ▼                     ▼                             │
│  ┌──────────────┐     ┌──────────────────┐                  │
│  │ load_file    │     │ save_file        │                  │
│  │ (JSON+Decrypt)│    │ (Encrypt+JSON)    │                  │
│  └──────────────┘     └────────┬─────────┘                  │
│                                │                             │
│                       ┌────────▼─────────┐                  │
│                       │  CryptoBridge    │                  │
│                       │  (AndroidKeyStore│                  │
│                       │   / Fake Base64) │                  │
│                       └──────────────────┘                  │
└───────────────────────┬─────────────────────────────────────┘
                        │ read/write
                        ▼
┌─────────────────────────────────────────────────────────────┐
│                   Disk Storage                              │
│              config.json (Encoded)                          │
│  { settings: {...}, password_cipher: "..." }                │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **启动/读取**: `get_config` 命令触发 `load_from`。首先尝试从 `AndroidState` 缓存读取，若未命中则调用 `load_file` 读取磁盘 JSON。`load_file` 解析 `EncodedSettings`，调用 `CryptoBridge.decrypt` 还原密码。随后调用 `migrate_legacy_defaults` 检查 `config_schema_version`，若低于当前版本（7）且字段等于历史旧默认值，则执行相应迁移并落盘。最后调用 `masked_for_display` 掩码密码，更新缓存，返回前端。
2.  **保存/写入**: `save_config` 命令接收前端传来的 `Settings`。全程持有 `CONFIG_IO_LOCK` 以防止竞态。首先调用 `load_from` 获取当前磁盘配置作为基准。调用 `resolve_password_field` 决定是保留旧密码、采用新密码还是清空密码。合并后的 `Settings` 传入 `save_to`。`save_to` 调用 `save_file`，其中 `CryptoBridge.encrypt` 对密码加密，生成 `EncodedSettings`。文件先写入 `.tmp` 后缀 (带 PID)，成功后原子重命名为 `config.json`。
3.  **广播**: 保存成功后，`save_config` 调用 `emit_config_changed`，向所有前端监听者广播掩码后的配置，确保 UI 状态与后端一致。同时触发 `auto_create_account_for_current` 辅助函数。

## Connections

*   [[android-interval-default-schema-migration]]: 本文档是 `android-interval-default-schema-migration` 决策的具体实现模块。
*   [[config-field-sets-bidirectional-sync]]: `Settings` 结构与桌面 `Config` 保持字段对齐，但编号体系独立。