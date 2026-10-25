---
title: 凭据单一来源（登录信息为自助服务与运营商绑定唯一凭据源）
type: decision
source_files:
  - tauri-app/src-tauri/src/commands/self_service.rs
  - tauri-app/src-tauri/src/config/model.rs
  - android/src-tauri/src/self_service_cmds.rs
tags: [决策, 凭据, 配置, 安全, 引导]
---

## Overview

本决策确立：**校园网登录信息中的 `password` 字段是自助服务系统（Self-Service）与运营商账号绑定的唯一密码来源**。

历史背景中，系统曾并存两套凭据：登录信息 `config.user`/`config.password` 与自助服务专用 `config.self_password`。由于校园网（Dr.COM/Portal）登录与自助服务系统在实际业务逻辑中共用同一套学号密码，双字段导致了用户需在两处分别输入、保存同一密码的冗余，且极易产生“自助密码、登录密码、账号档案密码”三态不一致的问题。

本决策删除了独立的自助密码字段，后端所有涉及自助服务的命令均通过“前端传入优先，空/MASK 占位符回退已存登录密码”的逻辑获取凭据。此设计消除了凭据不一致类 Bug，并收敛了敏感信息的存储与传输路径。当前代码（见 `model.rs` 与双端命令文件）已无任何 `self_password` 字段，凭据解析统一由 `resolve_credential_password` (`self_service.rs:51-62`) / `resolve_self_password` (`self_service_cmds.rs:27-36`) 完成。

## Key Components

### 1. 桌面端 (Tauri Desktop)

**文件**: `tauri-app/src-tauri/src/config/model.rs`

*   **常量**:
    *   `pub const PASSWORD_MASK: &str = "***"` (`model.rs:3`): 密码掩码标准值。
    *   `pub const AUTO_DETECT_ADAPTER: &str = "自动检测"` (`model.rs:4`): 网卡自动检测标识。
*   **结构体 `Config`** (`model.rs:10-182`):
    *   `pub user: String` (`model.rs:11`)
    *   `pub password: String` (`model.rs:13`，`#[serde(default)]` 于 `model.rs:12`)：**唯一的密码存储字段**，加载时由 DPAPI 解密，内存中为明文。
    *   `pub self_hello_enabled: bool` (`model.rs:17`，重命名 `selfHelloEnabled`，默认 `true`)：是否启用 Windows Hello 操作验证。
    *   `pub self_reverify_each_action: bool` (`model.rs:20`，重命名 `selfReverifyEachAction`，默认 `false`)：自助面板每次操作二次验证。
    *   `pub operator: String` (`model.rs:22`)
    *   `pub adapter1: String` (`model.rs:23`)
    *   `pub adapter2: String` (`model.rs:24`)
    *   `pub adapter1_account: String` (`model.rs:28`，重命名 `adapter1Account`)
    *   `pub adapter2_account: String` (`model.rs:31`，重命名 `adapter2Account`)
    *   `pub dual_adapter: bool` (`model.rs:32`，重命名 `dualAdapter`)
    *   `pub auto_login_on_start: bool` (`model.rs:34`，重命名 `autoLoginOnStart`)
    *   `pub auto_exit_after_login: bool` (`model.rs:36`，重命名 `autoExitAfterLogin`)
    *   `pub minimize_to_tray: bool` (`model.rs:38`，重命名 `minimizeToTray`)
    *   `pub lightweight_mode: bool` (`model.rs:42`，重命名 `lightweightMode`，默认 `true`)
    *   `pub hidden_start: bool` (`model.rs:44`，重命名 `hiddenStart`)
    *   `pub auto_launch: bool` (`model.rs:46`，重命名 `autoLaunch`)
    *   `pub enable_background_check: bool` (`model.rs:48`，重命名 `enableBackgroundCheck`)
    *   `pub background_check_interval: u64` (`model.rs:50`，重命名 `backgroundCheckInterval`)
    *   `pub auto_login_on_preparation: bool` (`model.rs:52`，重命名 `autoLoginOnPreparation`)
    *   `pub enable_night_operator_switch: bool` (`model.rs:56`，重命名 `enableNightOperatorSwitch`)
    *   `pub night_operator_restore: String` (`model.rs:59`，重命名 `nightOperatorRestore`)
    *   `pub enable_night_outbound_switch: bool` (`model.rs:63`，重命名 `enableNightOutboundSwitch`)
    *   `pub outbound_priority: Vec<String>` (`model.rs:69`，重命名 `outboundPriority`)
    *   `pub outbound_metric_restore: String` (`model.rs:74`，重命名 `outboundMetricRestore`)
    *   `pub outbound_disabled_adapters: String` (`model.rs:80`，重命名 `outboundDisabledAdapters`)
    *   `pub outbound_standby_route: String` (`model.rs:85`，重命名 `outboundStandbyRoute`)
    *   `pub outbound_manual_hold_day: i32` (`model.rs:92`，重命名 `outboundManualHoldDay`)
    *   `pub outbound_pending_switch: String` (`model.rs:98`，重命名 `outboundPendingSwitch`)
    *   `pub night_outbound_restore: String` (`model.rs:102`，重命名 `nightOutboundRestore`)
    *   `pub dns_optimize_adapters: Vec<String>` (`model.rs:107`，重命名 `dnsOptimizeAdapters`)
    *   `pub device_aliases: HashMap<String, String>` (`model.rs:113`，重命名 `deviceAliases`)
    *   `pub auto_exit_on_online: bool` (`model.rs:114`，重命名 `autoExitOnOnline`)
    *   `pub theme_mode: String` (`model.rs:116`，重命名 `themeMode`)
    *   `pub enable_notification: bool` (`model.rs:118`，重命名 `enableNotification`)
    *   `pub active_account: String` (`model.rs:120`，重命名 `activeAccount`)
    *   `pub display_name: String` (`model.rs:125`，重命名 `displayName`)
    *   `pub enable_latency_test: bool` (`model.rs:126`，重命名 `enableLatencyTest`)
    *   `pub latency_test_interval: u64` (`model.rs:128`，重命名 `latencyTestInterval`)
    *   `pub custom_theme_color: String` (`model.rs:130`，重命名 `customThemeColor`)
    *   `pub default_panel: String` (`model.rs:132`，重命名 `defaultPanel`)
    *   `pub enable_network_quality: bool` (`model.rs:134`，重命名 `enableNetworkQuality`)
    *   `pub skip_ttfb_in_latency: bool` (`model.rs:136`，重命名 `skipTtfbInLatency`，默认 `true`)
    *   `pub skip_content_in_latency: bool` (`model.rs:138`，重命名 `skipContentInLatency`，默认 `true`)
    *   `pub portal_url: String` (`model.rs:140`，重命名 `portalUrl`)
    *   `pub fixed_gateway: String` (`model.rs:142`，重命名 `fixedGateway`)
    *   `pub required_network_name: String` (`model.rs:144`，重命名 `requiredNetworkName`)
    *   `pub enable_network_name_check: bool` (`model.rs:146`，重命名 `enableNetworkNameCheck`，默认 `true`)
    *   `pub campus_gateway: String` (`model.rs:148`，重命名 `campusGateway`)
    *   `pub update_source: String` (`model.rs:151`，重命名 `updateSource`)
    *   `pub campus_exit_on_fail: bool` (`model.rs:153`，重命名 `campusExitOnFail`，默认 `true`)
    *   `pub campus_exit_start_minutes: u16` (`model.rs:156`，重命名 `campusExitStartMinutes`)
    *   `pub campus_exit_end_minutes: u16` (`model.rs:159`，重命名 `campusExitEndMinutes`)
    *   `pub campus_check_start_minutes: u16` (`model.rs:161`，重命名 `campusCheckStartMinutes`)
    *   `pub campus_check_end_minutes: u16` (`model.rs:164`，重命名 `campusCheckEndMinutes`)
    *   `pub scheduled_login_minutes: u16` (`model.rs:167`，重命名 `scheduledLoginMinutes`)
    *   `pub scheduled_logout_minutes: u16` (`model.rs:170`，重命名 `scheduledLogoutMinutes`)
    *   `pub log_retention_days: u32` (`model.rs:172`，重命名 `logRetentionDays`)
    *   `pub max_disconnect_reconnect: u32` (`model.rs:175`，重命名 `maxDisconnectReconnect`)
    *   `pub auto_login_cooldown_secs: u64` (`model.rs:177`，重命名 `autoLoginCooldownSecs`)
    *   `pub skip_sha256_when_missing: bool` (`model.rs:178`，重命名 `skipSha256WhenMissing`)
    *   `pub config_version: u32` (`model.rs:180`，重命名 `configVersion`)
*   **方法**:
    *   `pub fn masked_for_display(&self) -> Config` (`model.rs:326-330`)：出站掩码出口。克隆配置并调用 `mask_in_place`。
    *   `pub fn mask_in_place(&mut self)` (`model.rs:332-336`)：原地掩码。仅当 `self.password` 非空时替换为 `PASSWORD_MASK`。这是敏感信息出站的唯一出口。
    *   `impl Default for Config` (`model.rs:244-320`)：默认值初始化。`password` 默认为空字符串，各字段按注释默认。
*   **自由函数**（反序列化/默认辅助，私有）：
    *   `deserialize_non_empty_or` (`model.rs:184-194`)
    *   `deserialize_campus_gateway` (`model.rs:196-201`)
    *   `deserialize_required_network_name` (`model.rs:203-208`)
    *   `default_true` (`model.rs:210`)
    *   `default_campus_check_start_minutes` (`model.rs:212`)
    *   `default_campus_check_end_minutes` (`model.rs:216`)
    *   `default_campus_exit_start_minutes` (`model.rs:218`)
    *   `default_campus_exit_end_minutes` (`model.rs:220`)
    *   `default_fixed_gateway` (`model.rs:222-224`)
    *   `default_log_retention_days` (`model.rs:226`)
    *   `default_max_disconnect_reconnect` (`model.rs:228`)
    *   `default_auto_login_cooldown_secs` (`model.rs:230`)
    *   `default_portal_url` (`model.rs:232-234`)
    *   `default_required_network_name` (`model.rs:236-238`)
    *   `default_campus_gateway` (`model.rs:240-242`)
*   **测试模块** (`model.rs:339-403`)：验证默认重连/冷却、serde 缺字段回退、账号字段契约。

**文件**: `tauri-app/src-tauri/src/commands/self_service.rs`

*   **辅助函数**:
    *   `fn ensure_identity_gate(state: &AppState) -> Option<String>` (`self_service.rs:16-24`)：身份验证门控。若 `self_hello_enabled` 为 false 放行；否则检查 `crate::platform::identity::identity_verified_recently()`，未验证返回拦截文案。
    *   `fn resolve_campus_bind_addr(state: &AppState) -> Option<IpAddr>` (`self_service.rs:27-33`)：解析校园网适配器源 IP（配置名有效→有线优先→任意有 IP）。
    *   `fn is_iso_date(s: &str) -> bool` (`self_service.rs:37-45`)：严格 YYYY-MM-DD 校验（月份 01-12、日期 01-31，旧宽松实现已替换）。
    *   `fn resolve_credential_password(state: &AppState, password: &str) -> Option<String>` (`self_service.rs:51-62`)：**核心凭据解析逻辑**。前端传入非 MASK 明文优先；空/`PASSWORD_MASK` 回退 `state.config.load_full().password`（已解密内存明文），皆无返回 `None`。
*   **Tauri 命令 (`#[tauri::command]`)**:
    *   `pub async fn bind_operator(...)` (`self_service.rs:66-113`)：绑定运营商账号。需通过 `ensure_identity_gate`（行83-85）。凭据仅本次内存传递。
    *   `pub async fn query_bind_status(...)` (`self_service.rs:116-149`)：查询各运营商绑定状态。**不设验证门**。密码回退（行126-128）。
    *   `pub async fn query_self_dashboard(...)` (`self_service.rs:153-179`)：查询在线信息及上网记录。**不设验证门**。
    *   `pub async fn query_self_online_log(...)` (`self_service.rs:183-216`)：查询上网记录账单页数据。**不设验证门**，含 `is_iso_date` 校验（行200-202）及起止日期比较。
    *   `pub async fn self_offline_session(...)` (`self_service.rs:220-253`)：注销指定在线会话。需通过 `ensure_identity_gate`（行232-234）。
    *   `pub async fn change_self_password(...)` (`self_service.rs:260-295`)：修改自助服务密码（= 校园网登录密码）。需通过 `ensure_identity_gate`（行272-274）。新密码校验 6-11 位 ASCII 字母数字（行278-282）。
    *   `pub async fn verify_windows_identity(...)` (`self_service.rs:323-349`)：Windows Hello 本地身份验证。成功后 `note_identity_verified()` 记录后端时间戳。
    *   `pub async fn reveal_operator_credential(...)` (`self_service.rs:334-364`)：查看运营商明文凭据。**无论总开关如何，强制要求近期身份验证**（行349-353，防 webview 绕过）。
*   **测试模块** (`self_service.rs:371-384`)：`iso_date_strict` 验证 `is_iso_date` 严格性。

### 2. 安卓端 (Tauri Android)

**文件**: `android/src-tauri/src/self_service_cmds.rs`

*   **结构体与实现**:
    *   `pub struct CommandResult` (`self_service_cmds.rs:150-157`)：字段 `success`, `message`, `data`。
    *   `impl CommandResult` (`self_service_cmds.rs:16-23`)：`ok_msg`, `err` 构造器。
*   **辅助函数**:
    *   `fn resolve_self_password(incoming: &str, settings: &crate::config_state::Settings) -> Option<String>` (`self_service_cmds.rs:27-36`)：**核心凭据解析逻辑**。与桌面同构：前端传非 MASK 明文优先，空/MASK 回退 `settings.password`，皆无返回 `None`。
    *   `fn ensure_identity_gate(settings: &crate::config_state::Settings) -> Option<String>` (`self_service_cmds.rs:39-48`)：验证门控。总开关关闭放行，否则检查 `identity_verified_recently()`。
    *   `async fn load_settings(app: &tauri::AppHandle) -> Result<crate::config_state::Settings, String>` (`self_service_cmds.rs:50-52`)：加载设置。
    *   `fn local_addr_of(state: &tauri::State<'_, crate::android_state::AndroidState>) -> Option<IpAddr>` (`self_service_cmds.rs:54-60`)：获取缓存源 IP。
    *   `fn is_iso_date(s: &str) -> bool` (`self_service_cmds.rs:187-199`)：严格 YYYY-MM-DD 日期校验（同构桌面）。
*   **Tauri 命令**:
    *   `pub async fn verify_biometric_identity(success: bool, method: String, _consent_message: Option<String>)` (`self_service_cmds.rs:66-82`)：生物识别验证。仅 `success == true` 解锁验证门。
    *   `pub async fn bind_operator(...)` (`self_service_cmds.rs:84-127`)：绑定运营商账号。需验证门（行103-105）。
    *   `pub async fn query_bind_status(...)` (`self_service_cmds.rs:129-159`)：查询绑定状态。**不设验证门**。
    *   `pub async fn query_self_dashboard(...)` (`self_service_cmds.rs:161-184`)：查询 Dashboard。**不设验证门**。
    *   `pub async fn query_self_online_log(...)` (`self_service_cmds.rs:201-228`)：查询上网记录。**不设验证门**，含 `is_iso_date` 校验。
    *   `pub async fn self_offline_session(...)` (`self_service_cmds.rs:230-256`)：注销会话。需验证门（行243-245）。
    *   `pub async fn change_self_password(...)` (`self_service_cmds.rs:262-297`)：修改密码。需验证门（行276-278），新密码校验同桌面。
    *   `pub async fn reveal_operator_credential(...)` (`self_service_cmds.rs:300-328`)：揭示明文凭据。强制验证门（行314-316）。
*   **测试模块** (`self_service_cmds.rs:330-374`)：覆盖密码回退、验证门、日期校验。

## Architecture

```text
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│ Onboarding   │  │ Self-Service │  │ Account Form │
└──────┬───────┘  └──────┬───────┘  └──────┬───────┘
       │                 │                 │
       ▼                 ▼                 ▼
┌──────────────────────────────────────────────────────┐
│ Tauri Backend: Config/Settings.password (model.rs:13)│
│ resolve_credential_password (self_service.rs:51-62)  │
│ resolve_self_password (self_service_cmds.rs:27-36)   │
│ Commands: bind/query/reveal (self_service.rs:66+)    │
│   modify/reveal → ensure_identity_gate (15/39)       │
│   query → no gate                                    │
│ mask_in_place (model.rs:332-336) for display         │
└───────────────────────────┬──────────────────────────┘
                            ▼
┌──────────────────────────────────────────────────────┐
│ campus_login_lib::self_service (HTTP Client)         │
└──────────────────────────────────────────────────────┘
```

## Data Flow

1.  **凭据存储**:
    *   用户在“登录信息”或“新手引导”中输入学号和密码。
    *   桌面端：密码存入 `Config.password` (`model.rs:13`)，落盘时通过 DPAPI 加密（内存明文见 `self_service.rs:48-49` 注释）。
    *   安卓端：密码存入 `Settings.password`（由 `resolve_self_password` 注释 `self_service_cmds.rs:25-26` 知为凭据唯一源），落盘加密由 `config_state` 处理。
    *   **关键点**: 没有任何地方单独存储“自助服务密码”。

2.  **凭据获取 (Command Execution)**:
    *   前端发起自助服务请求，传递 `account` 和 `password`（若表单留空或显示为 `***` 则传空串/掩码）。
    *   后端调用 `resolve_credential_password` (桌面 `self_service.rs:51-62`) 或 `resolve_self_password` (安卓 `self_service_cmds.rs:27-36`)。
    *   **解析逻辑**:
        *   传入 `password` 非空且不为 `***` → 使用该明文。
        *   传入空/`***` → 从内存 `Config/Settings.password` 加载已解密密码。
        *   内存也为空 → 返回 `None`，前端提示“请先在登录信息填写并保存密码”（桌面 `self_service.rs:88`，安卓 `self_service_cmds.rs:107`）。

3.  **出站安全**:
    *   配置发前端显示必经 `masked_for_display` (`model.rs:326-330`) → `mask_in_place` (`model.rs:332-336`)。
    *   仅 `password` 非空替换为 `***`，确保前端缓存无明文。

4.  **身份验证门控 (Identity Gate)**:
    *   写操作（绑定 `self_service.rs:83`、注销 `self_service.rs:232`、改密 `self_service.rs:272`）和敏感读（揭示凭据 `self_service.rs:349`）检查 `ensure_identity_gate` 或近期验证。
    *   若 `self_hello_enabled` true 且超 TTL，拒绝；`reveal_operator_credential` 忽略总开关始终要求验证。
    *   查询命令（`query_bind_status`/`query_self_dashboard`/`query_self_online_log`）无门控，依赖本机数据掩码渲染。

## Connections

- [[page:config-mask-single-exit]] — 掩码出站唯一出口，`mask_in_place` (`model.rs:332-336`) 现仅 password 单字段。
- [[page:security-model]] — 凭据出站、落盘加密与验证门全景。
- [[page:config-and-persistence]] — 配置结构、三态语义与原子落盘（`Config` 字段见 `model.rs:10-182`）。
- [[page:desktop-account-selfservice]] — 自助命令面与验证门交互时序（`self_service.rs` 命令群）。
- [[page:query-bind-status-no-gate-by-design]] — 查询命令不设门的有意设计（`self_service.rs:116-149` 等）。

## Known Issues

- **旧数据兼容**: 旧版本配置文件或账号档案中可能存在的 `selfPassword` / `self_password` 键值，由于双端 serde 均未设置 `deny_unknown_fields`（`model.rs:10` 仅 `#[serde(default)]`），会在反序列化时被自动忽略。无需专门迁移脚本，但用户若曾独立设自助密码，该值丢失，后续回退登录密码或提示输入。
- **密码一致性**: 若用户曾手动将“登录密码”和“自助密码”设为不同值（业务上应相同），本决策实施后自助服务统一用“登录密码”，可能导致期望不符，除非重新同步。
- **安卓验证门文案差异**: 桌面拦截文案“Windows 身份验证已过期…”（`self_service.rs:22,350`），安卓为“生物识别验证已过期…”（`self_service_cmds.rs:46,315`），属平台术语差异但逻辑同构。