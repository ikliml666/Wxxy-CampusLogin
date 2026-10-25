---
title: 安全体系（凭据出站、加密与验证门）
type: concept
tags:
  - 概念
  - 安全
  - 掩码
  - dpapi
  - keystore
  - 验证门
  - 凭据
source_files:
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/persist.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
  - tauri-app/src-tauri/src/account/crypto.rs
  - android/src-tauri/src/identity_gate.rs
  - android/plugins/keystore/src/lib.rs
---

## Overview

安全体系围绕三条主线展开，桌面（Tauri + Windows DPAPI）与安卓（Tauri + AndroidKeyStore AES-GCM）双端同构：

1. **凭据出站唯一出口**：任何把配置发往前端/事件的路径，密码字段非空一律替换为 `"***"`（`PASSWORD_MASK`，`config/model.rs:3`）。收敛点为 `Config::masked_for_display`（`config/model.rs:326-330`），所有前端读取路径（`get_config` 命令 `commands/config_cmd.rs:226-229`、广播 `config/persist.rs:296-322` 中 `notify_config_changed` 包裹 `{config: emit_cfg}` 于行301-302）均经此出口。
2. **落盘加密**：密码写入磁盘前必经加密——桌面 DPAPI（用户作用域 + additional entropy，`account/crypto.rs:42,88-106,221-229`），安卓 AndroidKeyStore AES-GCM（`plugins/keystore/src/lib.rs:28-40` 的 `CampusKeystore`）。"非空即加密，不排除掩码占位符"（`config/persist.rs:324-336`，注释 326-330 明确用户真实密码恰为 `***` 也须加密）。
3. **后端验证门**：敏感自助操作前要求本机身份验证，安卓端 TTL 600 秒、时间戳只存后端内存（`android/src-tauri/src/identity_gate.rs:6-33`，时钟回拨视为过期）。桌面端 `platform/identity.rs` 未在本文源码范围内，此处不展开。

配套机制：登录/质量历史记录不含密码字段且损坏自动备份（`config/persist.rs:174-226,237-288`）；导入配置时切换态快照保留本机值以防系统残留（`commands/config_cmd.rs:177-182`）；账号档案读写加解密往返（`config/persist.rs:76-112`）；原子写防断电损坏（`config/persist.rs:13-44`）。

### 术语与不变量

| 术语 | 含义 | 不变量 |
| --- | --- | --- |
| `PASSWORD_MASK` | 出站掩码占位符 `"***"` | 用户真实密码恰为 `***` 时不得排除加密（`persist.rs:326-330`） |
| `masked_for_display` | 敏感信息出站唯一出口 | 空→空（未设置语义），非空→`***`（`model.rs:326-330`） |
| `IDENTITY_VERIFY_TTL_SECS` | 验证门有效期 600s | 时间戳 0=从未验证；时钟回拨视为过期（`identity_gate.rs:8,23-33`） |
| `atomic_write` | 临时文件+rename 原子写 | 断电/崩溃不残留半成品配置（`persist.rs:13-44`） |
| `DPAPI_ENTROPY` | DPAPI 附加熵 | 机器+用户作用域叠加，拷出密文无法直接解（`crypto.rs:42`） |

## Key Components

### 1. Config 模型 (`tauri-app/src-tauri/src/config/model.rs`)

核心结构体 `Config` (行 10-182)，容器级 `#[serde(default)]` (行 9) 保证旧版本缺字段不整体失败。关键常量：

*   `PASSWORD_MASK: &str = "***"` (行 3)
*   `AUTO_DETECT_ADAPTER: &str = "自动检测"` (行 4)

全部字段（serde rename 见源码注释）：

*   `user`: String (行 11)
*   `password`: String (行 13, 前导 `#[serde(default)]` 行 12) — 出站掩码、落盘加密
*   `self_hello_enabled`: bool (行 18, rename `selfHelloEnabled`, default `default_true` 行 210) — Windows Hello 验证开关；关闭后查看运营商密码仍强制验证（行 14-16 注释）
*   `self_reverify_each_action`: bool (行 20-21, rename `selfReverifyEachAction`, default false) — 每次操作二次验证（无消费方，见 Known Issues）
*   `operator`: String (行 22)
*   `adapter1`: String (行 23, default `自动检测` 见 Default 行 252)
*   `adapter2`: String (行 24)
*   `adapter1_account`: String (行 28, rename `adapter1Account`) — 设备级配置，切账号不覆盖（行 25-26）
*   `adapter2_account`: String (行 31, rename `adapter2Account`)
*   `dual_adapter`: bool (行 33, rename `dualAdapter`)
*   `auto_login_on_start`: bool (行 35, rename `autoLoginOnStart`, default true 行 257)
*   `auto_exit_after_login`: bool (行 37, rename `autoExitAfterLogin`, default false 行 260)
*   `minimize_to_tray`: bool (行 39, rename `minimizeToTray`, default false 行 261)
*   `lightweight_mode`: bool (行 43, rename `lightweightMode`, default true 行 262)
*   `hidden_start`: bool (行 45, rename `hiddenStart`, default false 行 263)
*   `auto_launch`: bool (行 47, rename `autoLaunch`, default true 行 264)
*   `enable_background_check`: bool (行 49, rename `enableBackgroundCheck`, default true 行 265)
*   `background_check_interval`: u64 (行 51, rename `backgroundCheckInterval`, default 60000 行 267)
*   `auto_login_on_preparation`: bool (行 53, rename `autoLoginOnPreparation`, default true 行 268)
*   `enable_night_operator_switch`: bool (行 57, rename `enableNightOperatorSwitch`, default false 行 271)
*   `night_operator_restore`: String (行 60, rename `nightOperatorRestore`, default 空 行 272)
*   `enable_night_outbound_switch`: bool (行 64, rename `enableNightOutboundSwitch`, default false 行 273)
*   `outbound_priority`: Vec<String> (行 70, rename `outboundPriority`, default 空 行 274)
*   `outbound_metric_restore`: String (行 75, rename `outboundMetricRestore`, default 空 行 275) — 切换态快照，导入保留本机值
*   `outbound_disabled_adapters`: String (行 81, rename `outboundDisabledAdapters`, default 空 行 276)
*   `outbound_standby_route`: String (行 86, rename `outboundStandbyRoute`, default 空 行 277)
*   `outbound_manual_hold_day`: i32 (行 93, rename `outboundManualHoldDay`, default 0 行 278)
*   `outbound_pending_switch`: String (行 99, rename `outboundPendingSwitch`, default 空 行 279) — 待切换状态快照，导入保留本机值
*   `night_outbound_restore`: String (行 103, rename `nightOutboundRestore`, default 空 行 282) — 安卓切换态标记
*   `dns_optimize_adapters`: Vec<String> (行 108, rename `dnsOptimizeAdapters`, default 空 行 280)
*   `device_aliases`: HashMap<String, String> (行 113, rename `deviceAliases`, default 空 行 281)
*   `auto_exit_on_online`: bool (行 115, rename `autoExitOnOnline`, default false 行 284)
*   `theme_mode`: String (行 117, rename `themeMode`, default "dark" 行 285)
*   `enable_notification`: bool (行 119, rename `enableNotification`, default true 行 286)
*   `active_account`: String (行 121, rename `activeAccount`, default 空 行 287)
*   `display_name`: String (行 125, rename `displayName`, default 空 行 288) — 账号显示名，空兜底 id
*   `enable_latency_test`: bool (行 127, rename `enableLatencyTest`, default true 行 289)
*   `latency_test_interval`: u64 (行 129, rename `latencyTestInterval`, default 600000 行 291)
*   `custom_theme_color`: String (行 131, rename `customThemeColor`, default "#6366f1" 行 292)
*   `default_panel`: String (行 133, rename `defaultPanel`, default 空 行 293)
*   `enable_network_quality`: bool (行 135, rename `enableNetworkQuality`, default true 行 294)
*   `skip_ttfb_in_latency`: bool (行 137, rename `skipTtfbInLatency`, default true 行 295)
*   `skip_content_in_latency`: bool (行 139, rename `skipContentInLatency`, default true 行 296)
*   `portal_url`: String (行 141, rename `portalUrl`, default `http://10.1.99.100` 行 297)
*   `fixed_gateway`: String (行 143, rename `fixedGateway`, default `10.2.127.254` 行 298)
*   `required_network_name`: String (行 145, rename `requiredNetworkName`, deserialize_with 非空回退默认 行 299)
*   `enable_network_name_check`: bool (行 147, rename `enableNetworkNameCheck`, default true 行 300)
*   `campus_gateway`: String (行 149, rename `campusGateway`, default `10.2.127.254` 行 301)
*   `update_source`: String (行 152, rename `updateSource`, default "mirror" 行 302)
*   `campus_exit_on_fail`: bool (行 154, rename `campusExitOnFail`, default true 行 303)
*   `campus_exit_start_minutes`: u16 (行 157, rename `campusExitStartMinutes`, default 480 行 304)
*   `campus_exit_end_minutes`: u16 (行 160, rename `campusExitEndMinutes`, default 1380 行 305)
*   `campus_check_start_minutes`: u16 (行 162, rename `campusCheckStartMinutes`, alias `campusCheckStartHour`, default 460 行 306)
*   `campus_check_end_minutes`: u16 (行 165, rename `campusCheckEndMinutes`, default 1380 行 307)
*   `scheduled_login_minutes`: u16 (行 168, rename `scheduledLoginMinutes`, default 1440 行 310)
*   `scheduled_logout_minutes`: u16 (行 171, rename `scheduledLogoutMinutes`, default 1440 行 311)
*   `log_retention_days`: u32 (行 173, rename `logRetentionDays`, default 7 行 312)
*   `max_disconnect_reconnect`: u32 (行 175, rename `maxDisconnectReconnect`, default 3 行 313)
*   `auto_login_cooldown_secs`: u64 (行 177, rename `autoLoginCooldownSecs`, default 60 行 314)
*   `skip_sha256_when_missing`: bool (行 179, rename `skipSha256WhenMissing`, default false 行 315)
*   `config_version`: u32 (行 181, rename `configVersion`, default 5 行 317)

关键方法：

*   `Config::masked_for_display(&self) -> Config` (行 326-330): 返回密码被掩码的新 Config。唯一出站出口，空值保留。
*   `Config::mask_in_place(&mut self)` (行 332-336): 就地掩码非空 password 为 `PASSWORD_MASK`。`pub` 可被绕过（见 Known Issues）。

辅助函数与 Default：

*   `deserialize_non_empty_or` (行 184-194), `deserialize_campus_gateway` (行 196-201), `deserialize_required_network_name` (行 203-208) — 空串回退默认
*   `default_true` (行 210), `default_campus_check_start_minutes` (行 212), `default_campus_check_end_minutes` (行 216), `default_campus_exit_start_minutes` (行 218), `default_campus_exit_end_minutes` (行 220), `default_fixed_gateway` (行 222), `default_log_retention_days` (行 226), `default_max_disconnect_reconnect` (行 228), `default_auto_login_cooldown_secs` (行 230), `default_portal_url` (行 232), `default_required_network_name` (行 236), `default_campus_gateway` (行 240)
*   `impl Default for Config` (行 244-320)

测试（行 339-403）：`default_reconnect_and_cooldown_config` (行 344), `serde_missing_reconnect_field_uses_default` (行 351), `serde_custom_reconnect_config` (行 363), `serde_account_fields_json_names_and_defaults` (行 375) 锁定 serde 契约。

### 2. 持久化层 (`tauri-app/src-tauri/src/config/persist.rs`)

关键函数：

*   `LOGIN_HISTORY_LOCK: Mutex<()>` (行 11) — 登录历史读写互斥
*   `atomic_write(path: &Path, content: &str)` (行 13-44) — 临时文件(纳秒时间戳)+sync_all+rename，重试 3 次
*   `get_data_dir(app_handle: &AppHandle) -> PathBuf` (行 46-59) — 极端回退 `campus-login` 子目录
*   `get_config_path(data_dir: &Path) -> PathBuf` (行 61-63) — `<dir>/config.json`
*   `get_accounts_dir(data_dir: &Path) -> PathBuf` (行 65-67) — `<dir>/accounts`
*   `get_account_path(data_dir: &Path, account_id: &str) -> PathBuf` (行 70-72) — `<dir>/accounts/<id>.json`
*   `load_account_config(data_dir: &Path, account_id: &str) -> Result<Config, String>` (行 76-97) — 读取并 `crypto::decrypt` 密码
*   `save_account_config(data_dir: &Path, account_id: &str, config: &Config) -> Result<(), String>` (行 101-112) — 非空密码 `crypto::encrypt` 后落盘
*   `list_account_items(data_dir: &Path) -> Vec<AccountItem>` (行 118-168) — 枚举账号，过滤 `.` 前缀/非 json，按 id 排序，display_name 空兜底 id
*   `get_login_history_path` (行 170-172)
*   `append_login_history(...)` (行 174-226) — 头插，上限 100，损坏备份 `.bak` 后重置
*   `QUALITY_HISTORY_LOCK: Mutex<()>` (行 229)
*   `get_quality_history_path` (行 231-233)
*   `append_quality_history(...)` (行 237-239) — 委托 `append_quality_history_to`
*   `append_quality_history_to(data_dir: &Path, result: &...)` (行 241-288) — 头插，延迟 <0 落盘 null，上限 100
*   `save_config_and_broadcast(app_handle: &AppHandle, config: &Config)` (行 296-322) — 调 `save_config_to_disk_encrypted` 后掩码发射 `config-changed` 事件（`notify_config_changed` 包裹 `{config: emit_cfg}` 行 301-302），并后台刷新无头核心凭据
*   `save_config_to_disk_encrypted(data_dir: &Path, config: &Config)` (行 324-336) — 非空密码一律 `crypto::encrypt`（不排除 MASK），`atomic_write` 落盘

测试（行 338-502）：`literal_star_password_is_encrypted_on_disk` (行 350), `empty_password_stays_empty` (行 368), `quality_history_head_insert_and_shape` (行 394), `quality_history_caps_at_100` (行 411), `quality_history_corrupt_backup_and_reset` (行 425), `list_account_items_missing_dir_is_empty` (行 441), `list_account_items_fallback_and_filter` (行 450), `account_config_roundtrip_encrypts_password` (行 481, Windows) 锁定加密契约。

### 3. 命令层 (`tauri-app/src-tauri/src/commands/config_cmd.rs`)

关键命令：

*   `save_config_to_disk_encrypted(app_handle: &AppHandle, config: &Config)` (行 8-15) — 调 `persist::save_config_and_broadcast` 后刷托盘菜单
*   `load_config_from_file(app_handle: &AppHandle) -> Result<Config, String>` (行 17-40) — 读取并解密（排除 MASK 占位，行 28）
*   `load_config_from_disk_or_default(app_handle: &AppHandle) -> Config` (行 42-65) — 失败备份 `.corrupt-<stamp>.bak` 后返回默认
*   `build_config_export_payload(current: &Config, include_password: bool) -> Result<serde_json::Value, String>` (行 73-94) — 掩码态或 DPAPI 加密态（`passwordEncrypted` 标志），绝不写明文
*   `restore_imported_password_field(imported: &mut String, encrypted: bool, current: &str) -> Result<(), String>` (行 102-112) — 空/MASK 保留当前值，密文 decrypt，失败报错
*   `export_config(...)` (行 117-137) — 阻塞 IO 移入 `spawn_blocking`，原子写 `<data_dir>/exports/config-<时间戳>.json`
*   `import_config(...)` (行 144-206) — 大小防呆 1MB，解析 wrapper/裸 Config，密码还原先于校验，切换态六字段+安卓标记保留本机值（行 177-182），落盘同 `save_config` 路径
*   `show_window(app_handle: AppHandle)` (行 208-224) — 用户最小化保护
*   `get_config(state: State<'_, AppState>) -> Result<Config, String>` (行 226-229) — 返回 `masked_for_display()`
*   `save_config(...)` (行 234-286) — 校验，密码三态（clear/空或MASK保留当前/新值），同步 `update_portal_url` 与 `set_log_retention_days`，先落盘再更新内存，R2 自动建号

测试（行 288-365）：`masked_for_display_masks_password` (行 298), `export_payload_never_contains_plaintext_password` (行 316, Windows), `import_password_restore_semantics` (行 343, Windows)。

### 4. 桌面加密 (`tauri-app/src-tauri/src/account/crypto.rs`)

Windows DPAPI 模块 `dpapi` (行 1-219)：

*   `DPAPI_ENTROPY: &[u8] = b"Wxxy-CampusLogin::dpapi-v1"` (行 42) — 附加熵
*   `call_dpapi<F>(input_data: &[u8], dpapi_call: F, ...)` (行 46-78) — 通用 helper，LocalFree 释放
*   `encrypt(plaintext: &[u8])` (行 88-106) — 带 entropy 的 `CryptProtectData`
*   `decrypt(data: &[u8])` (行 108-147) — 先带 entropy 解密，失败回退无 entropy 兼容旧密文
*   `CRYPTPROTECT_LOCAL_MACHINE: u32 = 0x0400` (行 152)
*   `encrypt_machine(plaintext: &[u8])` (行 156-174) — 机器作用域加密（SYSTEM 可解）
*   `decrypt_machine(data: &[u8])` (行 179-197) — 机器作用域解密（dead_code，供无头核心）
*   `encrypt_legacy_no_entropy(plaintext: &[u8])` (行 201-218) — 测试用旧格式

跨平台 pub 封装：

*   `encrypt(plaintext: &str) -> Result<String, String>` (Windows 行 221-229, 非 Windows 行 241-243 返回 Err)
*   `decrypt(encrypted_base64: &str) -> Result<String, String>` (Windows 行 232-237, 非 Windows 行 277-279 返回 Err)
*   `encrypt_machine(plaintext: &str)` (Windows 行 248-255, 非 Windows 行 267-269 Err)
*   `decrypt_machine(encrypted_base64: &str)` (Windows 行 259-264, 非 Windows 行 272-274 Err)

测试（行 281-317, Windows）：`dpapi_roundtrip_with_entropy` (行 287), `dpapi_decrypt_falls_back_to_legacy_no_entropy` (行 296), `dpapi_machine_scope_roundtrip` (行 306)。

### 5. 安卓身份验证门 (`android/src-tauri/src/identity_gate.rs`)

*   `LAST_VERIFY_EPOCH_SECS: AtomicU64` (行 6) — 最后验证时间戳
*   `IDENTITY_VERIFY_TTL_SECS: u64 = 600` (行 8)
*   `now_epoch_secs() -> u64` (行 10-15)
*   `note_identity_verified()` (行 18-20) — 写入当前时间戳
*   `identity_verified_recently() -> bool` (行 23-33) — TTL 内且非时钟回拨才 true（回拨守卫行 29-31）
*   `reset_for_tests()` (行 36-38)

测试（行 40-65）：从未验证为假、TTL 内/外、时钟回拨过期。

### 6. 安卓加密插件 (`android/plugins/keystore/src/lib.rs`)

*   `PLUGIN_IDENTIFIER: &str = "com.campuslogin.plugin.keystore"` (行 12)
*   `EncryptResult { data: String }` (行 15-17)
*   `DecryptResult { text: String }` (行 19-22)
*   `CampusKeystore<R: Runtime>(PluginHandle<R>)` (行 24)
*   `CampusKeystore::encrypt(&self, text: &str) -> Result<String>` (行 30-33) — 调移动插件 `encrypt`，返回 base64(iv+ciphertext)
*   `CampusKeystore::decrypt(&self, data: &str) -> Result<String>` (行 36-39) — GCM 认证失败拒绝
*   `CampusKeystoreExt<R: Runtime>` trait (行 42-50) — `campus_keystore()` 获取句柄
*   `init<R: Runtime>() -> TauriPlugin<R>` (行 52-61) — 注册 Android 插件

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                        Frontend (Webview)                   │
└──────┬──────────────────────────────┬───────────────────────┘
       │ Tauri Commands               │ Events (config-changed)
       ▼                              ▲
┌─────────────────────────────────────────────────────────────┐
│                     Tauri Backend                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │ Config Cmd   │  │ Identity Gate│  │ Keystore Plugin  │  │
│  │ (config_cmd) │  │ (android)    │  │ (android only)   │  │
│  └──────┬───────┘  └─────┬────────┘  └────────┬─────────┘  │
│         │                 │                    │            │
│  ┌──────▼───────┐  ┌─────▼──────┐  ┌──────────▼─────────┐  │
│  │  Persist     │  │ Masked     │  │  Crypto (DPAPI)    │  │
│  │  (Encrypt)   │  │ Display    │  │  / Keystore        │  │
│  └──────┬───────┘  └─────┬──────┘  └────────────────────┘  │
│         │                 │                                  │
│  ┌──────▼───────┐  ┌─────▼──────┐                          │
│  │  Config Model│  │ atomic_write│                          │
│  │  (mask API)  │  └────────────┘                          │
│  └──────────────┘                                          │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

### 桌面保存流

1. 前端调用 `save_config` (`commands/config_cmd.rs:234-286`)。
2. `validate_config` 校验（行 241）。
3. 密码三态：clear_password 置空（行 251-252）；空或 MASK 保留当前（`state.config.load()` 行 255-256）；否则新值。
4. 同步全局 `update_portal_url` (行 261) 与 `set_log_retention_days` (行 264)。
5. `spawn_blocking` 调 `save_config_to_disk_encrypted` (行 271) → `persist::save_config_and_broadcast` (`persist.rs:296-322`) → `save_config_to_disk_encrypted` (`persist.rs:324-336`) 加密非空密码 → `atomic_write` (`persist.rs:13-44`)。
6. 更新内存 `state.config.store` (`config_cmd.rs:274`)。
7. R2 自动建号 `auto_create_account_for_current` (行 281)。

### 桌面导入导出流

* **导出** (`export_config` 行 117-137)：`build_config_export_payload` (行 73-94) 掩码态或 DPAPI 加密态（`passwordEncrypted` 标志），写入 `<data_dir>/exports/`。
* **导入** (`import_config` 行 144-206)：大小防呆 1MB，解析 wrapper/裸 Config，密码还原 `restore_imported_password_field` (行 172)，切换态六字段保留本机值 (行 177-182)，校验后同保存路径落盘。

### 安卓保存流（基于提供插件接口）

1. 前端调用保存命令（未提供命令层，仅插件）。
2. 密码经 `CampusKeystore::encrypt` (`keystore/src/lib.rs:30-33`) 调 Android Keystore AES-GCM 加密为 base64(iv+ciphertext)。
3. 密文落盘（具体持久化未提供）。
4. 出站配置经 `masked_for_display` 同构逻辑掩码（桌面侧 `model.rs:326-330`；安卓侧约定同构但未提供源码）。

### 验证门时序（安卓）

1. 前端 BiometricPrompt 成功后调用后端写入时间戳（调用 `note_identity_verified` `identity_gate.rs:18-20`，具体命令未提供）。
2. 后续敏感操作调用 `identity_verified_recently` (`identity_gate.rs:23-33`) 检查 TTL 600s 与时钟回拨。
3. TTL 内且无回拨 → 放行；否则拦截。

## Connections

* [[page:desktop-config]] — 配置模型、`save_config` 三态语义与 `config-changed` 广播链路。
* [[page:desktop-account-selfservice]] — 自助命令面与验证门交互（桌面端未在提供源码，参见历史约定）。
* [[page:android-backend]] — 安卓命令面与状态缓存（identity_gate 模块）。
* [[page:android-plugins]] — keystore 插件的 Rust/Kotlin 双侧接口。
* [[page:config-and-persistence]] — 配置结构与原子写、迁移机制（persist.rs）。
* [[page:dual-platform-sharing]] — 双端同构约定（回退语义、验证门、掩码出口）。

## Known Issues

1. ~~桌面账号档案独立自助密码字段明文落盘~~（已消解）：`save_account_config` (`persist.rs:101-112`) 统一加密 `password` 字段，历史双端不一致不复存在。
2. **`self_reverify_each_action` 无后端消费方**：仅字段定义（`model.rs:20-21`）、Default false（`model.rs:250`）；提供源码中 `config_cmd.rs`/`persist.rs` 均未引用，任何"每次操作重新验证"策略未实现。
3. **安卓验证门信任前端**：`note_identity_verified` (`identity_gate.rs:18-20`) 被调用即写时间戳，提供源码未含校验 BiometricPrompt 真实结果的后端逻辑；前端伪造调用即可解锁 TTL 窗口（桌面走系统 API 相对可信但未提供源码）。
4. **掩码出口类型收敛不对称**：桌面 `masked_for_display` 返回同构 `Config` (`model.rs:326-330`)，但 `mask_in_place` 为 `pub` (`model.rs:332-336`) 可直接调用绕过封装；注释 (`model.rs:322-324`) 强调所有前端路径必须经由 `masked_for_display`，漏调即泄露。安卓侧未提供 `config_state.rs` 无法确认。
5. **非 Windows 加密函数全为 Err 占位**：`crypto.rs:241-243,267-269,272-274,277-279` 直接返回 `"加密存储仅桌面端支持"`，安卓走独立 keystore 插件（`keystore/src/lib.rs`），双端加密实现不共享代码路径，维护需双边同步。
6. **导入切换态保留逻辑硬编码**：`config_cmd.rs:181-186` 将六个切换态字段（含 `outbound_pending_switch` 与 `night_outbound_restore`）强制覆盖为本机值，若未来新增切换态字段易遗漏（当前靠注释 k2.8 审计提示）。