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

本决策删除了独立的自助密码字段，后端所有涉及自助服务的命令均通过“前端传入优先，空/MASK 占位符回退已存登录密码”的逻辑获取凭据。此设计消除了凭据不一致类 Bug，并收敛了敏感信息的存储与传输路径。当前代码（见 `model.rs` 与双端命令文件）已无任何 `self_password` 字段，凭据解析统一由 `resolve_credential_password`（`self_service.rs:51-62`） / `resolve_self_password`（`self_service_cmds.rs:30-39`）完成。注意 `Config` 中仍存在 `vpn_password_enc`（`model.rs:185-186`）用于存储 WebVPN CAS 密码密文，但其仅用于自助服务校外 VPN 回退通道，不干扰校园网登录密码作为自助服务主凭据的单一性。

## Key Components

### 1. 桌面端 (Tauri Desktop)

**文件**: `tauri-app/src-tauri/src/config/model.rs`

*   **常量**:
    *   `pub const PASSWORD_MASK: &str = "***"` (`model.rs:3`)：密码掩码标准值。
    *   `pub const AUTO_DETECT_ADAPTER: &str = "自动检测"` (`model.rs:4`)：网卡自动检测标识。
*   **结构体 `Config`** (`model.rs:10-193`)：
    *   `pub user: String` (`model.rs:11`)
    *   `#[serde(default)] pub password: String` (`model.rs:12-13`)：**唯一的登录密码存储字段**，加载时由 DPAPI 解密，内存中为明文。
    *   `#[serde(rename = "selfHelloEnabled", default = "default_true")] pub self_hello_enabled: bool` (`model.rs:17-18`)：是否启用 Windows Hello 操作验证。
    *   `#[serde(rename = "selfReverifyEachAction", default)] pub self_reverify_each_action: bool` (`model.rs:20-21`)：自助面板每次操作二次验证。
    *   `pub operator: String` (`model.rs:22`)
    *   `pub adapter1: String` (`model.rs:23`)
    *   `pub adapter2: String` (`model.rs:24`)
    *   `#[serde(rename = "adapter1Account")] pub adapter1_account: String` (`model.rs:27-28`)
    *   `#[serde(rename = "adapter2Account")] pub adapter2_account: String` (`model.rs:30-31`)
    *   `#[serde(rename = "dualAdapter")] pub dual_adapter: bool` (`model.rs:32-33`)
    *   `#[serde(rename = "autoLoginOnStart")] pub auto_login_on_start: bool` (`model.rs:34-35`)
    *   `#[serde(rename = "autoExitAfterLogin")] pub auto_exit_after_login: bool` (`model.rs:36-37`)
    *   `#[serde(rename = "minimizeToTray")] pub minimize_to_tray: bool` (`model.rs:38-39`)
    *   `#[serde(rename = "lightweightMode", default = "default_true")] pub lightweight_mode: bool` (`model.rs:42-43`)
    *   `#[serde(rename = "hiddenStart")] pub hidden_start: bool` (`model.rs:44-45`)
    *   `#[serde(rename = "autoLaunch")] pub auto_launch: bool` (`model.rs:46-47`)
    *   `#[serde(rename = "enableBackgroundCheck")] pub enable_background_check: bool` (`model.rs:48-49`)
    *   `#[serde(rename = "backgroundCheckInterval")] pub background_check_interval: u64` (`model.rs:50-51`)
    *   `#[serde(rename = "autoLoginOnPreparation")] pub auto_login_on_preparation: bool` (`model.rs:52-53`)
    *   `#[serde(rename = "enableNightOperatorSwitch", default)] pub enable_night_operator_switch: bool` (`model.rs:56-57`)
    *   `#[serde(rename = "nightOperatorRestore", default)] pub night_operator_restore: String` (`model.rs:59-60`)
    *   `#[serde(rename = "enableNightOutboundSwitch", default)] pub enable_night_outbound_switch: bool` (`model.rs:63-64`)
    *   `#[serde(rename = "outboundPriority", default)] pub outbound_priority: Vec<String>` (`model.rs:69-70`)
    *   `#[serde(rename = "outboundMetricRestore", default)] pub outbound_metric_restore: String` (`model.rs:74-75`)
    *   `#[serde(rename = "outboundDisabledAdapters", default)] pub outbound_disabled_adapters: String` (`model.rs:80-81`)
    *   `#[serde(rename = "outboundStandbyRoute", default)] pub outbound_standby_route: String` (`model.rs:85-86`)
    *   `#[serde(rename = "outboundManualHoldDay", default)] pub outbound_manual_hold_day: i32` (`model.rs:92-93`)
    *   `#[serde(rename = "outboundPendingSwitch", default)] pub outbound_pending_switch: String` (`model.rs:98-99`)
    *   `#[serde(rename = "nightOutboundRestore", default)] pub night_outbound_restore: String` (`model.rs:102-103`)
    *   `#[serde(rename = "dnsOptimizeAdapters", default)] pub dns_optimize_adapters: Vec<String>` (`model.rs:107-108`)
    *   `#[serde(rename = "deviceAliases", default)] pub device_aliases: std::collections::HashMap<String, String>` (`model.rs:112-113`)
    *   `#[serde(rename = "autoExitOnOnline")] pub auto_exit_on_online: bool` (`model.rs:114-115`)
    *   `#[serde(rename = "themeMode")] pub theme_mode: String` (`model.rs:116-117`)
    *   `#[serde(rename = "enableNotification")] pub enable_notification: bool` (`model.rs:118-119`)
    *   `#[serde(rename = "activeAccount")] pub active_account: String` (`model.rs:120-121`)
    *   `#[serde(rename = "displayName")] pub display_name: String` (`model.rs:124-125`)
    *   `#[serde(rename = "enableLatencyTest")] pub enable_latency_test: bool` (`model.rs:126-127`)
    *   `#[serde(rename = "latencyTestInterval")] pub latency_test_interval: u64` (`model.rs:128-129`)
    *   `#[serde(rename = "customThemeColor")] pub custom_theme_color: String` (`model.rs:130-131`)
    *   `#[serde(rename = "defaultPanel")] pub default_panel: String` (`model.rs:132-133`)
    *   `#[serde(rename = "enableNetworkQuality")] pub enable_network_quality: bool` (`model.rs:134-135`)
    *   `#[serde(rename = "skipTtfbInLatency", default = "default_true")] pub skip_ttfb_in_latency: bool` (`model.rs:136-137`)
    *   `#[serde(rename = "skipContentInLatency", default = "default_true")] pub skip_content_in_latency: bool` (`model.rs:138-139`)
    *   `#[serde(rename = "portalUrl", default = "default_portal_url")] pub portal_url: String` (`model.rs:140-141`)
    *   `#[serde(rename = "fixedGateway", default = "default_fixed_gateway")] pub fixed_gateway: String` (`model.rs:142-143`)
    *   `#[serde(rename = "requiredNetworkName", default = "default_required_network_name", deserialize_with = "deserialize_required_network_name")] pub required_network_name: String` (`model.rs:144-145`)
    *   `#[serde(rename = "enableNetworkNameCheck", default = "default_true")] pub enable_network_name_check: bool` (`model.rs:146-147`)
    *   `#[serde(rename = "campusGateway", default = "default_campus_gateway", deserialize_with = "deserialize_campus_gateway")] pub campus_gateway: String` (`model.rs:148-149`)
    *   `#[serde(rename = "updateSource", default)] pub update_source: String` (`model.rs:151-152`)
    *   `#[serde(rename = "campusExitOnFail", default = "default_true")] pub campus_exit_on_fail: bool` (`model.rs:153-154`)
    *   `#[serde(rename = "campusExitStartMinutes", default = "default_campus_exit_start_minutes")] pub campus_exit_start_minutes: u16` (`model.rs:156-157`)
    *   `#[serde(rename = "campusExitEndMinutes", default = "default_campus_exit_end_minutes")] pub campus_exit_end_minutes: u16` (`model.rs:159-160`)
    *   `#[serde(rename = "campusCheckStartMinutes", alias = "campusCheckStartHour", default = "default_campus_check_start_minutes")] pub campus_check_start_minutes: u16` (`model.rs:161-162`)
    *   `#[serde(rename = "campusCheckEndMinutes", default = "default_campus_check_end_minutes")] pub campus_check_end_minutes: u16` (`model.rs:164-165`)
    *   `#[serde(rename = "scheduledLoginMinutes", default)] pub scheduled_login_minutes: u16` (`model.rs:167-168`)
    *   `#[serde(rename = "scheduledLogoutMinutes", default)] pub scheduled_logout_minutes: u16` (`model.rs:170-171`)
    *   `#[serde(rename = "logRetentionDays", default = "default_log_retention_days")] pub log_retention_days: u32` (`model.rs:172-173`)
    *   `#[serde(rename = "maxDisconnectReconnect", default = "default_max_disconnect_reconnect")] pub max_disconnect_reconnect: u32` (`model.rs:174-175`)
    *   `#[serde(rename = "autoLoginCooldownSecs", default = "default_auto_login_cooldown_secs")] pub auto_login_cooldown_secs: u64` (`model.rs:176-177`)
    *   `#[serde(rename = "skipSha256WhenMissing", default)] pub skip_sha256_when_missing: bool` (`model.rs:178-179`)
    *   `#[serde(rename = "vpnAccount", default)] pub vpn_account: String` (`model.rs:181-182`)：WebVPN 登录账号（学号，明文；与校园网登录 user 允许不同）。
    *   `#[serde(rename = "vpnPasswordEnc", default)] pub vpn_password_enc: String` (`model.rs:185-186`)：WebVPN CAS 登录密码（DPAPI 密文）。
    *   `#[serde(rename = "selfServiceViaVpn", default = "default_true")] pub self_service_via_vpn: bool` (`model.rs:189-190`)：自助服务自动 VPN 回退总开关。
    *   `#[serde(rename = "configVersion", default)] pub config_version: u32` (`model.rs:191-192`)
*   **方法**:
    *   `pub fn masked_for_display(&self) -> Config` (`model.rs:341-345`)：出站掩码出口。克隆配置并调用 `mask_in_place`。
    *   `pub fn mask_in_place(&mut self)` (`model.rs:347-356`)：原地掩码。仅当 `self.password` 非空时替换为 `PASSWORD_MASK`；同样将非空的 `self.vpn_password_enc` 替换为 `PASSWORD_MASK`（密文出站即泄露攻击面）。这是敏感信息出站的唯一出口。
    *   `impl Default for Config` (`model.rs:255-335`)：默认值初始化。`password` 默认为空字符串，各字段按注释默认。
*   **自由函数**（反序列化/默认辅助，私有）：
    *   `deserialize_non_empty_or` (`model.rs:195-205`)
    *   `deserialize_campus_gateway` (`model.rs:207-212`)
    *   `deserialize_required_network_name` (`model.rs:214-219`)
    *   `default_true` (`model.rs:221`)
    *   `default_campus_check_start_minutes` (`model.rs:223`)
    *   `default_campus_check_end_minutes` (`model.rs:227`)
    *   `default_campus_exit_start_minutes` (`model.rs:229`)
    *   `default_campus_exit_end_minutes` (`model.rs:231`)
    *   `default_fixed_gateway` (`model.rs:233-235`)
    *   `default_log_retention_days` (`model.rs:237`)
    *   `default_max_disconnect_reconnect` (`model.rs:239`)
    *   `default_auto_login_cooldown_secs` (`model.rs:241`)
    *   `default_portal_url` (`model.rs:243-245`)
    *   `default_required_network_name` (`model.rs:247-249`)
    *   `default_campus_gateway` (`model.rs:251-253`)
*   **测试模块** (`model.rs:359-424`)：验证默认重连/冷却、serde 缺字段回退、账号字段契约。

**文件**: `tauri-app/src-tauri/src/commands/self_service.rs`

*   **辅助函数**:
    *   `pub(crate) fn ensure_identity_gate(state: &AppState) -> Option<String>` (`self_service.rs:16-24`)：身份验证门控。若 `self_hello_enabled` 为 false 放行；否则检查 `crate::platform::identity::identity_verified_recently()`，未验证返回拦截文案（行22）。
    *   `fn resolve_campus_bind_addr(state: &AppState) -> Option<IpAddr>` (`self_service.rs:27-33`)：解析校园网适配器源 IP（配置名有效→有线优先→任意有 IP）。
    *   `fn is_iso_date(s: &str) -> bool` (`self_service.rs:37-45`)：严格 YYYY-MM-DD 校验（月份 01-12、日期 01-31，旧宽松实现已替换）。
    *   `fn resolve_credential_password(state: &AppState, password: &str) -> Option<String>` (`self_service.rs:51-62`)：**核心凭据解析逻辑**。前端传入非 MASK 明文优先；空/`PASSWORD_MASK` 回退 `state.config.load_full().password`（已解密内存明文），皆无返回 `None`。
*   **Tauri 命令 (`#[tauri::command]`)**:
    *   `pub async fn bind_operator(...)` (`self_service.rs:67-114`)：绑定运营商账号。需通过 `ensure_identity_gate`（行83-85）。凭据仅本次内存传递。
    *   `pub async fn query_bind_status(...)` (`self_service.rs:118-151`)：查询各运营商绑定状态。**不设验证门**。密码回退（行127-129）。
    *   `pub async fn query_self_dashboard(...)` (`self_service.rs:156-182`)：查询在线信息及上网记录。**不设验证门**。
    *   `pub async fn query_self_online_log(...)` (`self_service.rs:187-220`)：查询上网记录账单页数据。**不设验证门**，含 `is_iso_date` 校验（行203-205）及起止日期比较。
    *   `pub async fn self_offline_session(...)` (`self_service.rs:225-258`)：注销指定在线会话。需通过 `ensure_identity_gate`（行236-238）。
    *   `pub async fn change_self_password(...)` (`self_service.rs:266-301`)：修改自助服务密码（= 校园网登录密码）。需通过 `ensure_identity_gate`（行277-279）。新密码校验 6-11 位 ASCII 字母数字（行283-287）。
    *   `pub async fn verify_windows_identity(...)` (`self_service.rs:329-354`)：Windows Hello 本地身份验证。成功后 `note_identity_verified()` 记录后端时间戳。
    *   `pub async fn reveal_operator_credential(...)` (`self_service.rs:341-371`)：查看运营商明文凭据。**无论总开关如何，强制要求近期身份验证**（行355-359，防 webview 绕过）。
*   **测试模块** (`self_service.rs:378-391`)：`iso_date_strict` 验证 `is_iso_date` 严格性（行378-390）。

### 2. 安卓端 (Tauri Android)

**文件**: `android/src-tauri/src/self_service_cmds.rs`

*   **结构体与实现**:
    *   `#[derive(Serialize, Clone)] pub struct CommandResult` (`self_service_cmds.rs:157-163`)：字段 `success`, `message`, `data`。
    *   `impl CommandResult` (`self_service_cmds.rs:19-26`)：`ok_msg`, `err` 构造器。
*   **辅助函数**:
    *   `fn resolve_self_password(incoming: &str, settings: &crate::config_state::Settings) -> Option<String>` (`self_service_cmds.rs:30-39`)：**核心凭据解析逻辑**。与桌面同构：前端传非 MASK 明文优先，空/MASK 回退 `settings.password`，皆无返回 `None`。
    *   `pub(crate) fn ensure_identity_gate(settings: &crate::config_state::Settings) -> Option<String>` (`self_service_cmds.rs:42-51`)：验证门控。总开关关闭放行，否则检查 `identity_verified_recently()`。
    *   `pub(crate) async fn load_settings(app: &tauri::AppHandle) -> Result<crate::config_state::Settings, String>` (`self_service_cmds.rs:53-55`)：加载设置。
    *   `fn local_addr_of(state: &tauri::State<'_, crate::android_state::AndroidState>) -> Option<std::net::IpAddr>` (`self_service_cmds.rs:57-63`)：获取缓存源 IP。
    *   `fn is_iso_date(s: &str) -> bool` (`self_service_cmds.rs:196-208`)：严格 YYYY-MM-DD 日期校验（同构桌面）。
*   **Tauri 命令**:
    *   `pub async fn verify_biometric_identity(success: bool, method: String, _consent_message: Option<String>)` (`self_service_cmds.rs:69-85`)：生物识别验证。仅 `success == true` 解锁验证门。
    *   `pub async fn bind_operator(...)` (`self_service_cmds.rs:87-132`)：绑定运营商账号。需验证门（行106-108），空密码回退失败时提示“请输入自助服务系统密码”（行110）。
    *   `pub async fn query_bind_status(...)` (`self_service_cmds.rs:134-166`)：查询绑定状态。**不设验证门**，密码回退失败提示“请输入自助服务系统密码”（行147）。
    *   `pub async fn query_self_dashboard(...)` (`self_service_cmds.rs:168-193`)：查询 Dashboard。**不设验证门**。
    *   `pub async fn query_self_online_log(...)` (`self_service_cmds.rs:210-239`)：查询上网记录。**不设验证门**，含 `is_iso_date` 校验。
    *   `pub async fn self_offline_session(...)` (`self_service_cmds.rs:241-269`)：注销会话。需验证门（行254-256）。
    *   `pub async fn change_self_password(...)` (`self_service_cmds.rs:275-312`)：修改密码。需验证门（行289-291），新密码校验同桌面。
    *   `pub async fn reveal_operator_credential(...)` (`self_service_cmds.rs:315-345`)：揭示明文凭据。强制验证门（行329-331）。
*   **测试模块** (`self_service_cmds.rs:347-391`)：覆盖密码回退、验证门、日期校验。

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
│ resolve_self_password (self_service_cmds.rs:30-39)   │
│ Commands: bind/query/reveal (self_service.rs:67+)    │
│   modify/reveal → ensure_identity_gate (15/42)       │
│   query → no gate                                    │
│ mask_in_place (model.rs:347-356) for display         │
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
    *   安卓端：密码存入 `Settings.password`（由 `resolve_self_password` 注释 `self_service_cmds.rs:28-29` 知为凭据唯一源），落盘加密由 `config_state` 处理。
    *   **关键点**: 没有任何地方单独存储“自助服务密码”。

2.  **凭据获取 (Command Execution)**:
    *   前端发起自助服务请求，传递 `account` 和 `password`（若表单留空或显示为 `***` 则传空串/掩码）。
    *   后端调用 `resolve_credential_password` (桌面 `self_service.rs:51-62`) 或 `resolve_self_password` (安卓 `self_service_cmds.rs:30-39`)。
    *   **解析逻辑**:
        *   传入 `password` 非空且不为 `***` → 使用该明文。
        *   传入空/`***` → 从内存 `Config/Settings.password` 加载已解密密码。
        *   内存也为空 → 返回 `None`，桌面前端提示“请先在登录信息填写并保存密码”（`self_service.rs:88`），安卓前端提示“请输入自助服务系统密码”（`self_service_cmds.rs:110`）。

3.  **出站安全**:
    *   配置发前端显示必经 `masked_for_display` (`model.rs:341`) → `mask_in_place` (`model.rs:347-356`)。
    *   `password` 非空替换为 `***`，`vpn_password_enc` 非空同样替换为 `***`，确保前端缓存无明文。

4.  **身份验证门控 (Identity Gate)**:
    *   写操作（绑定 `self_service.rs:83`、注销 `self_service.rs:236`、改密 `self_service.rs:277`）和敏感读（揭示凭据 `self_service.rs:355`）检查 `ensure_identity_gate` 或近期验证。
    *   若 `self_hello_enabled` true 且超 TTL，拒绝；`reveal_operator_credential` 忽略总开关始终要求验证。
    *   查询命令（`query_bind_status`/`query_self_dashboard`/`query_self_online_log`）无门控，依赖本机数据掩码渲染。

## Connections

- [[page:config-mask-single-exit]] — 掩码出站唯一出口，`mask_in_place` (`model.rs:347-356`) 掩码 `password` 与 `vpn_password_enc` 双字段。
- [[page:security-model]] — 凭据出站、落盘加密与验证门全景。
- [[page:config-and-persistence]] — 配置结构、三态语义与原子落盘（`Config` 字段见 `model.rs:10-193`）。
- [[page:desktop-account-selfservice]] — 自助命令面与验证门交互时序（`self_service.rs` 命令群）。
- [[page:query-bind-status-no-gate-by-design]] — 查询命令不设门的有意设计（`self_service.rs:118-151` 等）。

## Known Issues

- **旧数据兼容**: 旧版本配置文件或账号档案中可能存在的 `selfPassword` / `self_password` 键值，由于双端 serde 均未设置 `deny_unknown_fields`（`model.rs:9-10` 仅 `#[serde(default)]`），会在反序列化时被自动忽略。无需专门迁移脚本，但用户若曾独立设自助密码，该值丢失，后续回退登录密码或提示输入。
- **密码一致性**: 若用户曾手动将“登录密码”和“自助密码”设为不同值（业务上应相同），本决策实施后自助服务统一用“登录密码”，可能导致期望不符，除非重新同步。
- **安卓验证门文案差异**: 桌面拦截文案“Windows 身份验证已过期，请重新验证后再操作”（`self_service.rs:22`）与揭示处“Windows 身份验证已过期，请重新验证后再查看”（`self_service.rs:357`），安卓为“生物识别验证已过期,请重新验证”（`self_service_cmds.rs:49` 及 `self_service_cmds.rs:330`），属平台术语差异但逻辑同构。