---
title: credential-encryption-and-identity
type: concept
source_files:
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/protocol_cmds.rs
  - android/src-tauri/src/self_service_cmds.rs
  - android/src-tauri/src/identity_gate.rs
  - android/plugins/keystore/android/src/main/java/com/campuslogin/plugin/keystore/KeystorePlugin.kt
  - android/frontend/src/auth/useAuth.ts
  - android/frontend/src/account/selfServiceState.ts
  - android/frontend/src/account/SelfServicePanel.tsx
  - android/frontend/src/settings/SettingsPanel.tsx
  - android/frontend/src/settings/useSettings.ts
tags:
  - security
  - encryption
  - identity
  - biometric
---

## Overview

`credential-encryption-and-identity` 是安卓端处理用户敏感凭证（登录密码、运营商短信密码等）静态加密存储与高权限操作动态身份门控的安全链路。它横跨前端 React Hook/组件、Rust Tauri 命令、Android 原生 Keystore 插件，解决两个维度问题：

1. **静态数据保护**：登录密码在落盘前经 `KeystorePlugin.kt` 的 AndroidKeyStore AES-256-GCM 加密，磁盘形态 `EncodedSettings.passwordCipher` 存密文，`Settings.password` 内存态仅短暂存在，出站一律掩码 `***`（见 `config_state.rs:10` 的 `PASSWORD_MASK`）。桌面端使用 DPAPI，安卓以 AndroidKeyStore 替代。
2. **动态身份门控**：执行踢下线、查记录、改密码、揭示凭据等高危操作前，必须经过生物识别验证门（`identity_gate.rs` 的进程内 TTL 状态，或前端 `selfServiceState.ts` 的 `useHelloGate`/`useSelfServiceVerify`），防止未授权访问。总开关 `selfHelloEnabled` 关闭时放行，但 `reveal_operator_credential` 强制验证无视总开关。

## Key Components

### android/src-tauri/src/config_state.rs

- **常量**
  - `PASSWORD_MASK: &str = "***"`（config_state.rs:10）
  - `CONFIG_FILE: &str = "config.json"`（config_state.rs:11）
  - `CONFIG_IO_LOCK: tokio::sync::Mutex<()>`（lazy_static，config_state.rs:17-19），跨命令串行化配置 IO（防 save/switch/delete 并发竞态）
  - `pub async fn config_io_lock() -> MutexGuard<'static,()>`（config_state.rs:22-24）供 account_cmds 转发调用

- **结构体 `Settings`**（config_state.rs:26-128），`#[serde(rename_all="camelCase", default)]`，全部字段：
  - `user: String`（config_state.rs:30）账号
  - `password: String`（config_state.rs:31）落盘必加密
  - `self_hello_enabled: bool`（config_state.rs:32）生物验证总开关，默认 true
  - `self_reverify_each_action: bool`（config_state.rs:33）每次操作重验，默认 false
  - `allow_2d_face_verify: bool`（config_state.rs:36）2D 人脸验证开关，默认 false
  - `operator: String`（config_state.rs:37）
  - `enable_night_operator_switch: bool`（config_state.rs:41）夜间运营商切换，默认 false（config_state.rs:141）
  - `night_operator_restore: String`（config_state.rs:43）夜切前原运营商暂存
  - `enable_night_outbound_switch: bool`（config_state.rs:47）夜间出站切换总开关，默认 false
  - `outbound_priority: Vec<String>`（config_state.rs:50）出站网卡优先级，安卓不消费
  - `outbound_metric_restore: String`（config_state.rs:53）桌面出站切换态快照
  - `outbound_disabled_adapters: String`（config_state.rs:56）桌面禁用网卡名单
  - `outbound_standby_route: String`（config_state.rs:59）桌面兜底路由
  - `night_outbound_restore: String`（config_state.rs:64）安卓出站切换态标记，值恒 "logged_out"
  - `dns_optimize_adapters: Vec<String>`（config_state.rs:67）桌面 DNS 优化，安卓不消费
  - `device_aliases: HashMap<String,String>`（config_state.rs:72）设备自定义名，serde(default) 兼容旧配置
  - `auto_login_on_start: bool`（config_state.rs:74）默认 true
  - `enable_background_check: bool`（config_state.rs:75）默认 true
  - `background_check_interval: u64`（config_state.rs:76）默认 60000（config_state.rs:155）
  - `background_check_idle_interval: u64`（config_state.rs:81）闲时巡检默认 300000（config_state.rs:157）
  - `auto_login_on_preparation: bool`（config_state.rs:82）默认 true
  - `max_disconnect_reconnect: u32`（config_state.rs:83）默认 3
  - `auto_login_cooldown_secs: u64`（config_state.rs:84）默认 60
  - `theme_mode: String`（config_state.rs:86）默认 "dark"
  - `enable_notification: bool`（config_state.rs:87）默认 true
  - `custom_theme_color: String`（config_state.rs:88）默认 "#6366f1"
  - `default_panel: String`（config_state.rs:89）默认 "dashboard"
  - `display_name: String`（config_state.rs:93）显示名，默认空
  - `active_account: String`（config_state.rs:94）
  - `enable_boot_autostart: bool`（config_state.rs:95）默认 false
  - `enable_latency_test: bool`（config_state.rs:97）默认 false
  - `latency_test_interval: u64`（config_state.rs:98）默认 600000（config_state.rs:170）
  - `enable_network_quality: bool`（config_state.rs:99）默认 false（config_state.rs:174）
  - `skip_ttfb_in_latency: bool`（config_state.rs:100）默认 true
  - `skip_content_in_latency: bool`（config_state.rs:101）默认 true
  - `portal_url: String`（config_state.rs:103）默认 "http://10.1.99.100"
  - `fixed_gateway: String`（config_state.rs:104）默认 "10.2.127.254"
  - `required_network_name: String`（config_state.rs:105）默认 "i-wxxy、iwxxy-2、iwxxy-3"（config_state.rs:181）
  - `enable_network_name_check: bool`（config_state.rs:106）默认 true
  - `campus_gateway: String`（config_state.rs:107）默认 "10.2.127.254"
  - `campus_check_start_minutes: u16`（config_state.rs:109）默认 460
  - `campus_check_end_minutes: u16`（config_state.rs:111）默认 1380
  - `scheduled_login_minutes: u16`（config_state.rs:113）默认 1440
  - `scheduled_logout_minutes: u16`（config_state.rs:115）默认 1440
  - `update_source: String`（config_state.rs:118）默认 "mirror"
  - `log_retention_days: u32`（config_state.rs:120）默认 7
  - `config_schema_version: u32`（config_state.rs:127）默认 7（config_state.rs:197）

- **`impl Default for Settings`**（config_state.rs:130-200）给出上述默认值。

- **结构体 `CryptoBridge`**（config_state.rs:213-217）含 `encrypt: Arc<dyn Fn(&str)->Result<String,String>>` 与 `decrypt: Arc<dyn Fn(&str)->Result<String,String>>`；`from_app`（config_state.rs:210-240）移动端用 `tauri_plugin_campus_keystore` 插件，非移动端返回 Err（config_state.rs:213-214）。

- **结构体 `EncodedSettings`**（config_state.rs:243-248）磁盘形态：`settings: Settings` + `password_cipher: String`，密码字段落盘清空。

- **函数/命令**
  - `load_file`（config_state.rs:250-263）：读文件，缺失返默认；解析 `EncodedSettings`；密文非空则解密，失败置空（config_state.rs:260）。
  - `password_saved_at`（config_state.rs:270-276）：读盘判断 `password_cipher` 非空。
  - `save_file`（config_state.rs:278-308）：建目录；密码非空则加密，失败返回 Err（config_state.rs:289）；写 `EncodedSettings` 且 `password` 置空（config_state.rs:293）；临时文件带 pid（config_state.rs:300-304）后 rename。
  - `cleanup_stale_config_tmp`（config_state.rs:313-324）：启动清孤儿 `*.json.<pid>.tmp`。
  - `load_from`（config_state.rs:326-336）：调 `load_file` → `migrate_legacy_defaults` → 若 `required_network_name=="i-wxxy"` 扩为三值（config_state.rs:332-334）。
  - `migrate_legacy_defaults`（config_state.rs:355-417）：v0→v7 一次性迁移（v0→v1 间隔 15s→60s；v1→v3 自动化开/质量关；v3→v4 闲时间隔；v4→v5 检测终点；v5→v6 质量间隔；v6→v7 夜切开/定时禁用值 0→1440）。
  - `save_to`（config_state.rs:419-421）：包 `save_file`。
  - `masked_for_display`（config_state.rs:424-430）：克隆后密码非空变 `***`，唯一出站出口。
  - `emit_config_changed`（config_state.rs:436-439）：emit `config-changed` 事件，payload `{config: masked}`。
  - `resolve_password_field`（config_state.rs:442-451）：clear→空；incoming 空或 `***`→current；否则 incoming。
  - `#[tauri::command] get_config`（config_state.rs:454-466）：载配置、缓存 `AndroidState.config`、返掩码。
  - `#[tauri::command] save_config`（config_state.rs:469-499）：持 IO 锁，合密码，落盘，缓存，调 `account_cmds::auto_create_account_for_current`，emit 事件。
  - `current_settings`（config_state.rs:503-520）：缓存优先，未命中读盘。
  - 测试模块（config_state.rs:522-827）覆盖加解密、迁移、掩码、判据等。

### android/src-tauri/src/protocol_cmds.rs

- `#[tauri::command] do_login`（protocol_cmds.rs:9-63）：参 user/password/operator/adapter Option；先 `ensure_wifi_bound`；缺失回退 `current_settings`；空则拒；仅打长度（protocol_cmds.rs:37）；成功写 `login_history` 并清 `MONITOR` 熔断（protocol_cmds.rs:56-61）。
- `pub async fn run_login`（protocol_cmds.rs:66-90）：取 `cached_adapter_ip`，`spawn_blocking` 调 `do_login_with_retry`（重试 3）。
- `#[tauri::command] do_logout`（protocol_cmds.rs:93-143）：类似，设 `logout_protected_until_ms` +60s（protocol_cmds.rs:140）。
- `#[tauri::command] check_portal_status`（protocol_cmds.rs:146-164）：绑 WiFi 后 `spawn_blocking` `check_portal_full`。
- `cached_adapter_ip`（protocol_cmds.rs:166-174）：读 `state.cached_source_ip`。
- `#[tauri::command] ping_test`（protocol_cmds.rs:176-179）：返 "pong"。
- `#[tauri::command] bind_to_wifi` / `accept_wifi_network`（protocol_cmds.rs:182-214）：移动端调插件，非移动端 Err。
- `#[tauri::command] get_avoid_bad_wifi_status` / `restore_written_settings`（protocol_cmds.rs:219-255）：移动端 `spawn_blocking` 查/还原系统设置。
- `pub(crate) async fn ensure_wifi_bound`（protocol_cmds.rs:294-363）：旁路代理 `bind_capability()==Supported` 时跳过（protocol_cmds.rs:294-299）；否则 `spawn_blocking` 绑 WiFi；绑成功且非 `already_bound` 才 `clear_client_pool`（protocol_cmds.rs:291-295），随后 `accept_campus_wifi`；失败仅日志不阻断。
- `pub(crate) async fn accept_campus_wifi`（protocol_cmds.rs:350-389）：调 `accept_wifi_network` 并记录。

### android/src-tauri/src/self_service_cmds.rs

- `struct CommandResult`（self_service_cmds.rs:7-23）：字段 `success: bool`, `message?: Option<String>`, `data?: Option<Value>`；`ok_msg`/`err` 构造。
- `resolve_self_password`（self_service_cmds.rs:27-36）：incoming 非空且非 `PASSWORD_MASK`→用新；否则回退 `settings.password`，皆空返 None。
- `ensure_identity_gate`（self_service_cmds.rs:39-48）：`self_hello_enabled` 关→放行；否则查 `identity_verified_recently()`。
- `load_settings`（self_service_cmds.rs:50-52）：调 `current_settings`。
- `local_addr_of`（self_service_cmds.rs:54-60）：取 `cached_source_ip`。
- `verify_biometric_identity`（self_service_cmds.rs:67-83）：仅 `success==true` 才 `note_identity_verified`。
- `bind_operator`（self_service_cmds.rs:116-159）：验账号非空、门、密码解析、手机号 11 数字、sms 非空；构 `BindParams` 调 `bind_operator`。
- `query_bind_status`（self_service_cmds.rs:147-177）：无门，返 cmcc/telecom/unicom 的 `masked_account`/`password_set`。
- `query_self_dashboard`（self_service_cmds.rs:162-185）：返 `onlineList`/`loginHistory`。
- `is_iso_date`（self_service_cmds.rs:187-199）：严格 `YYYY-MM-DD`。
- `query_self_online_log`（self_service_cmds.rs:202-229）：验日期格式与顺序，调 `query_online_log`。
- `self_offline_session`（self_service_cmds.rs:231-257）：走门，验 session_id，调 `offline_session`。
- `change_self_password`（self_service_cmds.rs:287-322）：走门；新密码 6-11 位 ASCII 字母数字（self_service_cmds.rs:287-288）；调 `change_password`。
- `reveal_operator_credential`（self_service_cmds.rs:301-329）：**强制** `identity_verified_recently` 不依总开关（self_service_cmds.rs:314）；返 phone/smsPassword。
- 测试模块（self_service_cmds.rs:330-375）覆盖密码回退、验证门、日期校验。

### android/src-tauri/src/identity_gate.rs

- `static LAST_VERIFY_EPOCH_SECS: AtomicU64`（identity_gate.rs:6）
- `const IDENTITY_VERIFY_TTL_SECS: u64 = 600`（identity_gate.rs:8）
- `note_identity_verified`（identity_gate.rs:18-20）：写当前秒。
- `identity_verified_recently`（identity_gate.rs:23-33）：last==0 或时钟回拨（`now<last`）判过期；否则差 ≤600s 有效。
- `reset_for_tests`（identity_gate.rs:36-38）。
- 测试模块（identity_gate.rs:40-66）覆盖未验证/TTL/回拨。

### android/plugins/keystore/.../KeystorePlugin.kt

- `companion`：`KEY_ALIAS="campus_login_master"`, `IV_LEN=12`, `TAG="CampusKeystore"`（KeystorePlugin.kt:33-37）
- `getOrCreateKey`（KeystorePlugin.kt:39-54）：取或生成 AES-256 GCM NoPadding 密钥。
- `@Command encrypt`（KeystorePlugin.kt:56-71）：`Cipher AES/GCM/NoPadding`，IV+密文拼接后 Base64 置 `data` 字段。
- `@Command decrypt`（KeystorePlugin.kt:73-89）：Base64 解，要求 `raw.size>IV_LEN`，拆 IV(12) 与密文，`GCMParameterSpec(128, iv)` 解密返 `text`；异常 `reject`。

### android/frontend/src/auth/useAuth.ts

- `useAuth`（useAuth.ts:7-34）：通过 `useShallow` 选取 `useAuthStore` 的 `isLoggingIn`、`isLoggingOut`、`status`、`doLogin`、`doLogout`、`checkOnline`。
- `handleOpenPortal`（useAuth.ts:20-23）：取 `config.portalUrl`，缺省回退 `'http://10.1.99.100'`，调 `api.openExternal?.(url)`。
- `handleOpenSelfService`（useAuth.ts:25-27）：直接 `api.openExternal?.('http://10.1.80.200:8080/Self/login/?302=LI')`。

### android/frontend/src/account/selfServiceState.ts

- `helloEnabled`（selfServiceState.ts:10）：读取 `config.selfHelloEnabled !== false`，缺失视为开启。
- `BIOMETRIC_ERROR_KEY`（selfServiceState.ts:19-32）：错误码到 i18n key 映射表。
- `biometricFailMessage(err, t)`（selfServiceState.ts:34-38）：提取 `err.code` 翻译。
- `VERIFY_TTL_MS = 570_000`（selfServiceState.ts:54）：对齐后端 `IDENTITY_VERIFY_TTL_SECS=600` 留 30s 余量。
- `gateFresh(verifiedAt)`（selfServiceState.ts:58-62）：TTL 内且 `elapsed >= 0`（防时钟回拨）。
- 模块单例 `helloGateVerifiedAt`（selfServiceState.ts:63）与 `selfSessionVerifiedAt`（selfServiceState.ts:97）。
- `useHelloGate(options?: { ignoreToggle?: boolean })`（selfServiceState.ts:65-87）：异步验证；门新鲜直接放行；非 `ignoreToggle` 且总开关关则放行；否则调 `tauriApiWithRetry.verifyWindowsIdentity`，成功写时间戳；`ignoreToggle:true` 强制真实验证。
- `resetSelfSessionGate()`（selfServiceState.ts:100-102）：面板卸载重置会话门。
- `useSelfServiceVerify()`（selfServiceState.ts:105-127）：面板会话门；总开关关放行；`config.selfReverifyEachAction === true` 时每次验证，否则 TTL 内共用；成功写 `selfSessionVerifiedAt`。

### android/frontend/src/account/SelfServicePanel.tsx

- 接口 `SelfOnlineItem`（SelfServicePanel.tsx:28-38）：字段 `loginTime`、`ip`、`mac`、`useTime`、`downFlow`、`upFlow`、`hostName`、`terminalType`、`sessionId`。
- `toInt`（SelfServicePanel.tsx:49-52，失败返 null）
- `formatTerminalType`（SelfServicePanel.tsx:55-58，去 `#` 前缀）
- `formatUseTimeMinutes`（SelfServicePanel.tsx:61-64，秒转分 floor）
- `formatFlowMb`（SelfServicePanel.tsx:67-71，KB 转 MB toFixed(3)）
- `localDateStr`（SelfServicePanel.tsx:75-78，本地日期防 UTC 偏移）
- 主组件 `SelfServicePanel`（SelfServicePanel.tsx:80-480）：凭据唯一源 `config.user` + `passwordSaved`（SelfServicePanel.tsx:85-86, 125）；`ensureSelfVerified = useSelfServiceVerify()`（SelfServicePanel.tsx:80）；卸载重置会话门（SelfServicePanel.tsx:80-88）。
- `fetchDashboard`（SelfServicePanel.tsx:127-151）：守卫 `hasCred && !querying`；验证失败中止；调 `querySelfDashboard({account, password:''})`；`mountedRef` 防卸载后 setState。
- `handleOffline`（SelfServicePanel.tsx:165-190）：`offlineSessionId` 防重入；调 `selfOfflineSession`。
- `handleSaveAlias`（SelfServicePanel.tsx:193-210）：写 `config.deviceAliases[mac]`（空名删键），`saveConfigDirect` 落盘。
- `fetchOnlineLog`（SelfServicePanel.tsx:295-319）：调 `querySelfOnlineLog({account, password:'', startTime, endTime})`。

### android/frontend/src/settings/SettingsPanel.tsx

- `isAndroid` 常量（SettingsPanel.tsx:29）由 `import.meta.env.VITE_PLATFORM` 判断。
- `SettingsPanelProps`（SettingsPanel.tsx:31-35）：`onUpdateConfig`、`onSetAutoLaunch`、`onShowOnboarding?`。
- `handleSecurityDisable(apply)`（SettingsPanel.tsx:56-70）：调 `verifyWindowsIdentity({consentMessage})`，成功执行 `apply(true)`，失败 toast。
- 安全组开关（SettingsPanel.tsx:229-318）：
  - `self-hello-enabled`：关时走 `handleSecurityDisable`（SettingsPanel.tsx:56-64），`checked={config.selfHelloEnabled !== false}`。
  - `self-reverify` 与 `allow-2d-face`：在 `selfHelloEnabled === false` 时 disabled（SettingsPanel.tsx:266, 284）；2D Face 开启弹风险确认（SettingsPanel.tsx:287），关闭调 `clearTemplate()` 清模板（SettingsPanel.tsx:289）。
  - 录入走 `openFaceDialog('enroll')`（SettingsPanel.tsx:50-55）。
- 网络组（SettingsPanel.tsx:370-426）：关 `enable-quality` 联动停 `stopLatencyTest`、若 `defaultPanel==='quality'` 切 `dashboard`（SettingsPanel.tsx:370-388）。
- 延迟弹窗 `Dialog`（SettingsPanel.tsx:393-439）：`skip-ttfb`、`skip-content` 开关。

### android/frontend/src/settings/useSettings.ts

- `useSettings`（useSettings.ts:12-74）：浅层取 `useConfigStore`、`useThemeStore`、`useLogToastStore`。
- `handleToggleLightMode`（useSettings.ts:36-42）：翻 `isLightMode`，`updateConfig({themeMode})`，`safeStorage.set('campus-light-mode', ...)`。
- `handleToggleNotification`（useSettings.ts:44-50）：翻 `enableNotification`，`api.setNotificationEnabled?` 容错，开启时 `requestNotificationPermission`。
- `handleSetAutoLaunch`（useSettings.ts:52-60）：安卓映射 `enableBootAutostart` 字段，`api.setAutoLaunch?` 失败 toast。
- `handleSetTheme`（useSettings.ts:62-65）：`setThemeName` + `safeStorage.set('campus-theme', name)`。

## Architecture

```text
┌──────────────────────┐   ┌──────────────────────────┐   ┌──────────────────────┐
│ Frontend UI          │   │ selfServiceState.ts       │   │ Rust backend         │
│ SettingsPanel.tsx    │►  │ useHelloGate/             │►  │ self_service_cmds.rs  │
│ SelfServicePanel.tsx │   │ useSelfServiceVerify      │   │ identity_gate.rs      │
└──────────────────────┘   └──────────────────────────┘   └──────────┬───────────┘
                                                                     │ calls
                                                          ┌──────────▼───────────┐
                                                          │ config_state.rs       │
                                                          │ CryptoBridge.from_app │
                                                          └──────────┬───────────┘
                                                                     │ tauri plugin
                                                          ┌──────────▼───────────┐
                                                          │ KeystorePlugin.kt     │
                                                          │ AES-GCM encrypt/decrypt│
                                                          └──────────────────────┘
```

## Data Flow

### 1. 敏感操作身份验证（Identity Gate）
1. 前端 `SelfServicePanel` 等操作触发 `useSelfServiceVerify()`（selfServiceState.ts:105-127），检查 `selfSessionVerifiedAt` TTL（570s，对齐后端 600s）。
2. TTL 失效则调用 `tauriApiWithRetry.verifyWindowsIdentity`，后端 `verify_biometric_identity`（self_service_cmds.rs:67-83）仅 `success==true` 才 `note_identity_verified`（identity_gate.rs:18-20）。
3. 后端 `ensure_identity_gate`（self_service_cmds.rs:39-48）查 `identity_verified_recently()`（identity_gate.rs:23-33），`reveal_operator_credential` 无视总开关强制验（self_service_cmds.rs:314）。
4. 验证通过后命令执行业务（如 `query_self_online_log`、`self_offline_session`）。

### 2. 凭据加密与存储（Credential Encryption）
1. 前端 `save_config` 或 `saveConfigDirect` 收集密码，Rust `save_config`（config_state.rs:469-499）持 `CONFIG_IO_LOCK`，`resolve_password_field` 合并。
2. `save_file`（config_state.rs:278-308）调 `CryptoBridge::from_app` → `KeystorePlugin.kt.encrypt`（IV+密文 Base64），失败 Err 不写盘；成功写 `EncodedSettings` 且 `password` 置空。
3. 读取时 `load_file` 解密填内存态，`masked_for_display` 出站掩码。
4. 自助命令 `resolve_self_password`（self_service_cmds.rs:27-36）回退 `settings.password` 或 NULL。

## Connections

- 后端加密桥接 [[KeystorePlugin.kt]] 由 [[config_state.rs]] 的 `CryptoBridge` 调用。
- 身份门状态由 [[identity_gate.rs]] 支撑，被 [[self_service_cmds.rs]] 及前端 [[selfServiceState.ts]] 依赖。
- 配置读写与 `account_cmds::auto_create_account_for_current`（见 [[config_state.rs]] `save_config`）及 `monitor_loop::MONITOR`、`login_history::append`（见 [[protocol_cmds.rs]]）联动。
- 业务委托 `campus_login_lib::self_service::*`（见 [[self_service_cmds.rs]]）。
- 前端状态来自 `useConfigStore`/`useAuthStore`（见 [[useSettings.ts]]、[[useAuth.ts]]）。

## Known Issues

- **密码永不落明文**：`save_file` 加密失败直接 Err（config_state.rs:289），不写盘；解密失败置空而非崩（config_state.rs:260）。`masked_for_display` 是唯一出站出口。
- **密码已保存判据**：必须查 `password_cipher`（disk 形态），因落盘时 `settings.password` 被清（config_state.rs:270-276）。
- **配置并发**：`CONFIG_IO_LOCK` 覆盖 load→merge→save 全序列（config_state.rs:477）。
- **迁移幂等**：`migrate_legacy_defaults` 仅当等于旧默认时才刷，用户显式改后不再覆盖；落盘失败下次重迁（config_state.rs:355-417）。
- **SSID 名单扩展**：旧值 `"i-wxxy"` 载入扩三值，幂等（config_state.rs:332-334）。
- **临时文件竞争**：写盘用 `config.json.<pid>.tmp` 再 rename，孤立 tmp 由 `cleanup_stale_config_tmp` 启清（config_state.rs:300-324）。
- **WiFi 绑定旁路**：`ensure_wifi_bound` 在旁路代理 `BindCapability::Supported` 时跳过（protocol_cmds.rs:274-279）；绑定成功但 `already_bound` 不清 HTTP 池；绑定失败不阻断。
- **敏感日志纪律**：operator 只打长度（protocol_cmds.rs:37），密码绝不进日志/错误。
- **验证门语义**：总开关关则 `ensure_identity_gate` 放行；`reveal_operator_credential` 无视总开关强制验（self_service_cmds.rs:301-329）；`verify_biometric_identity` 必须 `success=true` 才解锁。
- **时钟回拨守卫**：`identity_verified_recently` 中 `now<last` 判过期（identity_gate.rs:29-31）；前端 `gateFresh` 要求 `elapsed >= 0`（selfServiceState.ts:58-62）。
- **Keystore 边界**：解密要求 `raw.size>IV_LEN`（KeystorePlugin.kt:78）；GCM 认证失败转 reject；应用卸载重装后密钥丢失则旧密文不可解密（config_state 置空）。
- **自助密码回退**：incoming 为 MASK/空且存储空时返 None，前端须提示输入（self_service_cmds.rs:27-36）。
- **日期严格性**：`is_iso_date` 拒 `2026-9-07`/`2026/09/07` 等（self_service_cmds.rs:187-199）。
- **新密码规则**：6–11 位 ASCII 字母数字（self_service_cmds.rs:282-283）。
- **前端 TTL 对齐**：`VERIFY_TTL_MS=570_000` 留 30s 余量防死锁（selfServiceState.ts:54）。
- **卸载保护与静默降级**：`mountedRef` 守卫 setState（SelfServicePanel.tsx:107, 136 等）；外网不可达失败静默。
- **安全开关联动**：关 2D Face 立即 `clearTemplate()`（SettingsPanel.tsx:289）；关质量检测联动停延迟测试并切面板（SettingsPanel.tsx:357-375）；`self-reverify`/`allow-2d-face` 在 Hello 总开关关时 disabled（SettingsPanel.tsx:266, 284）。
- **安卓差异化**：`useSettings` 中安卓自启真实字段为 `enableBootAutostart`（useSettings.ts:53-54）；API 失败仅 DEV 日志或 toast 不回滚 UI。