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

# 安全体系（凭据出站、加密与验证门）

## Overview

安全体系围绕三条主线展开，桌面（Tauri + Windows DPAPI）与安卓（Tauri + AndroidKeyStore AES-GCM）双端同构：

1. **凭据出站唯一出口**：任何把配置发往前端/日志的路径，密码字段非空一律替换为 `"***"`（`PASSWORD_MASK`，桌面 `config/model.rs:3`）。收敛点为 `Config::masked_for_display`（`config/model.rs:309-313`）。
2. **落盘加密**：密码写入磁盘前必经加密——桌面 DPAPI（当前用户作用域，`account/crypto.rs:114-130`），安卓 AndroidKeyStore AES-256-GCM（`plugins/keystore/src/lib.rs:26-30` 的 `CampusKeystoreExt`）。"非空即加密，不排除掩码占位符"（`config/persist.rs:318-321`）。
3. **后端验证门**：敏感自助操作前要求本机身份验证，TTL 600 秒、时间戳只存后端内存（桌面 `platform/identity.rs`，安卓 `identity_gate.rs:8-10`）。`reveal_operator_credential` 无论总开关如何都强制设门（桌面 `commands/self_service.rs`）。

配套机制：登录请求错误日志脱敏 `redact_credentials`（`auth/portal.rs`）；状态探测是只读操作、绝不携带密码（`portal.rs`）；登录历史不含密码字段（桌面 `persist.rs`）；账号 id 白名单防路径穿越（桌面 `commands/account.rs`）。

### 术语与不变量

| 术语 | 含义 | 不变量 |
| --- | --- | --- |
| `PASSWORD_MASK` | 出站掩码占位符 `"***"` | 用户真实密码恰为 `***` 时不得排除加密（`persist.rs:318-321`） |
| `masked_for_display` | 敏感信息出站唯一出口 | 空→空（未设置语义），非空→`***`（`model.rs:313-317`） |
| `IDENTITY_VERIFY_TTL_SECS` | 验证门有效期 600s | 时间戳 0=从未验证；时钟回拨视为过期（`identity_gate.rs:10-12`） |
| `redact_credentials` | 日志/错误脱敏 | URL 与明文密码均替换为 `***`（`portal.rs`） |
| `validate_account_name` | 账号 id 白名单 | 拒绝 `../`、`/`、`\` 等路径穿越字符（`account_cmds.rs`） |

## Key Components

### 1. Config 模型 (`tauri-app/src-tauri/src/config/model.rs`)

核心结构体 `Config` (行 13-116)，包含以下主要字段：

*   `user`: String, 用户名。
*   `password`: String, 密码。出站时必须掩码，落盘时必须加密。
*   `self_hello_enabled`: bool, 是否启用 Windows Hello 操作验证（默认 true）。
*   `self_reverify_each_action`: bool, 自助服务面板每次操作都二次验证（默认 false）。
*   `operator`: String, 运营商。
*   `adapter1`, `adapter2`: String, 适配器名称。
*   `adapter1_account`, `adapter2_account`: String, 主副适配器绑定的账号 id。
*   `dual_adapter`: bool, 是否双适配器。
*   `auto_login_on_start`: bool, 启动自动登录。
*   `auto_exit_after_login`: bool, 登录后自动退出。
*   `minimize_to_tray`: bool, 最小化到托盘。
*   `lightweight_mode`: bool, 轻量化模式。
*   `hidden_start`: bool, 隐藏启动。
*   `auto_launch`: bool, 开机自启。
*   `enable_background_check`: bool, 启用后台检查。
*   `background_check_interval`: u64, 后台检查间隔（毫秒）。
*   `auto_login_on_preparation`: bool, 准备阶段自动登录。
*   `enable_night_operator_switch`: bool, 夜间自动切换运营商。
*   `night_operator_restore`: String, 夜间切换恢复目标。
*   `enable_night_outbound_switch`: bool, 夜间出站自动切换。
*   `outbound_priority`: Vec<String>, 出站网卡优先级。
*   `outbound_metric_restore`: String, 切换态快照 JSON。
*   `outbound_disabled_adapters`: String, 夜间切换禁用的校园网卡快照 JSON。
*   `outbound_standby_route`: String, 夜间切换新增的兜底默认路由快照 JSON。
*   `outbound_manual_hold_day`: i32, 「立即切换」当日保持标记。
*   `night_outbound_restore`: String, 安卓切换态标记。
*   `dns_optimize_adapters`: Vec<String>, DNS 优化目标适配器名单。
*   `device_aliases`: HashMap<String, String>, 在线设备自定义名字。
*   `auto_exit_on_online`: bool, 在线时自动退出。
*   `theme_mode`: String, 主题模式。
*   `enable_notification`: bool, 启用通知。
*   `active_account`: String, 当前激活账号 id。
*   `display_name`: String, 本文件所属账号的显示名。
*   `enable_latency_test`: bool, 启用延迟测试。
*   `latency_test_interval`: u64, 延迟测试间隔（毫秒）。
*   `custom_theme_color`: String, 自定义主题颜色。
*   `default_panel`: String, 默认面板。
*   `enable_network_quality`: bool, 启用网络质量。
*   `skip_ttfb_in_latency`: bool, 延迟测试跳过 TTFB。
*   `skip_content_in_latency`: bool, 延迟测试跳过内容下载。
*   `portal_url`: String, Portal 地址。
*   `fixed_gateway`: String, 固定网关。
*   `required_network_name`: String,  required network name。
*   `enable_network_name_check`: bool, 启用 required network name 检查。
*   `campus_gateway`: String, 校园网网关。
*   `update_source`: String, 更新渠道优先级。
*   `campus_exit_on_fail`: bool, 失败时退出校园网。
*   `campus_exit_start_minutes`: u16, 非校园网自动退出生效时段起点。
*   `campus_exit_end_minutes`: u16, 非校园网自动退出生效时段终点。
*   `campus_check_start_minutes`: u16, 校园网检测时段起点。
*   `campus_check_end_minutes`: u16, 校园网检测时段终点。
*   `scheduled_login_minutes`: u16, 每日定时登录时刻。
*   `scheduled_logout_minutes`: u16, 每日定时注销时刻。
*   `log_retention_days`: u32, 日志保留天数。
*   `max_disconnect_reconnect`: u32, 最大断线重连次数。
*   `auto_login_cooldown_secs`: u64, 自动登录冷却时间（秒）。
*   `skip_sha256_when_missing`: bool, 缺失时跳过 SHA256 校验。
*   `config_version`: u32, 配置版本。

关键方法：

*   `Config::masked_for_display(&self) -> Config` (行 309-313): 返回一个密码被掩码的新 Config 实例。这是所有配置数据发往前端的唯一出口。
*   `Config::mask_in_place(&mut self)` (行 315-319): 将当前实例的 `password` 字段（如果非空）修改为 `PASSWORD_MASK`。

常量：

*   `PASSWORD_MASK: &str = "***"` (行 3): 出站掩码占位符。
*   `AUTO_DETECT_ADAPTER: &str = "自动检测"` (行 4)。

### 2. 持久化层 (`tauri-app/src-tauri/src/config/persist.rs`)

负责配置的原子写入、读取、账号档案管理以及登录/质量历史记录。

关键函数：

*   `atomic_write(path: &Path, content: &str) -> Result<(), String>` (行 12-43): 使用临时文件 + rename 的方式实现原子写入，防止断电或崩溃导致配置文件损坏。
*   `get_data_dir(app_handle: &AppHandle) -> PathBuf` (行 45-58): 获取应用数据目录。
*   `get_config_path(data_dir: &Path) -> PathBuf` (行 60-62): 获取主配置文件路径。
*   `get_accounts_dir(data_dir: &Path) -> PathBuf` (行 64-66): 获取账号档案目录。
*   `get_account_path(data_dir: &Path, account_id: &str) -> PathBuf` (行 68-70): 获取特定账号档案路径。
*   `load_account_config(data_dir: &Path, account_id: &str) -> Result<Config, String>` (行 72-96): 读取并解密账号档案。
*   `save_account_config(data_dir: &Path, account_id: &str, config: &Config) -> Result<(), String>` (行 98-112): 加密并保存账号档案。
*   `list_account_items(data_dir: &Path) -> Vec<AccountItem>` (行 114-160): 枚举所有账号档案，过滤隐藏文件和非法 JSON，按 ID 排序。
*   `append_login_history(...)` (行 180-226): 追加登录历史记录，互斥锁保护，上限 100 条。
*   `append_quality_history(...)` (行 256-302): 追加网络质量历史记录，互斥锁保护，上限 100 条。
*   `save_config_and_broadcast(app_handle: &AppHandle, config: &Config) -> Result<(), String>` (行 304-316): 统一出口，先落盘加密，再广播掩码后的配置事件。
*   `save_config_to_disk_encrypted(data_dir: &Path, config: &Config) -> Result<(), String>` (行 318-333): 将配置中的密码加密后原子写入磁盘。

### 3. 命令层 (`tauri-app/src-tauri/src/commands/config_cmd.rs`)

处理前端发来的配置相关命令。

关键命令函数：

*   `export_config(include_password: Option<bool>)` (行 82-113): 导出配置。默认掩码，可选含加密密码。
*   `import_config(path: String)` (行 115-186): 导入配置。解析 JSON，还原密码（如果是加密导出则先解密），校验，落盘，广播。
*   `show_window()` (行 188-191): 显示主窗口。
*   `get_config()` (行 193-196): 获取当前配置，返回掩码后的副本。
*   `save_config(config: Config, clear_password: Option<bool>)` (行 198-262): 保存配置。校验，处理密码三态语义（明文/掩码/空），落盘，更新内存，触发自动建号。

辅助函数：

*   `build_config_export_payload(current: &Config, include_password: bool) -> Result<serde_json::Value, String>` (行 63-80): 构建导出载荷。
*   `restore_imported_password_field(imported: &mut String, encrypted: bool, current: &str) -> Result<(), String>` (行 82-98): 还原导入文件中的密码字段。
*   `load_config_from_file(app_handle: &AppHandle) -> Result<Config, String>` (行 100-124): 从文件加载配置并解密。
*   `load_config_from_disk_or_default(app_handle: &AppHandle) -> Config` (行 126-152): 加载配置，失败时备份原文件并返回默认配置。

### 4. 系统命令 (`tauri-app/src-tauri/src/commands/system.rs`)

处理系统与 UI 相关的命令。

关键命令函数：

*   `minimize_window(window: Window, state: State<'_, AppState>)` (行 10-19): 根据配置决定最小化到托盘还是任务栏。
*   `close_window(window: Window, state: State<'_, AppState>)` (行 21-42): 根据配置决定隐藏到托盘、轻量级关闭或完全退出。
*   `open_external(url: String)` (行 44-56): 打开外部链接，校验协议和长度。
*   `get_auto_launch()` (行 58-61): 获取开机自启状态。
*   `set_auto_launch(enabled: bool, ...)` (行 63-113): 设置开机自启，注册表操作与配置落盘。
*   `get_notification_enabled(state: State<'_, AppState>)` (行 115-118): 获取通知开关。
*   `set_notification_enabled(enabled: bool, ...)` (行 120-148): 设置通知开关并落盘。
*   `cancel_auto_exit(app_handle: AppHandle, _state: State<'_, AppState>)` (行 150-155): 取消自动退出。
*   `get_logs(app_handle: AppHandle, lines: Option<usize>)` (行 157-161): 获取最近日志。
*   `clear_logs(app_handle: AppHandle)` (行 163-166): 清空日志。
*   `get_init_data(state: State<'_, AppState>, app_handle: AppHandle)` (行 168-213): 获取初始化数据，包括掩码配置、账号列表、适配器信息等。
*   `render_heartbeat(state: State<'_, AppState>)` (行 215-226): 更新前端心跳。
*   `get_gpu_info()` (行 228-231): 获取 GPU 信息。
*   `set_log_retention_days(days: u32)` (行 233-236): 设置日志保留天数。
*   `get_log_retention_days()` (行 238-240): 获取日志保留天数。
*   `export_diagnostics(app_handle: AppHandle, state: State<'_, AppState>, days: Option<u32>)` (行 242-336): 导出诊断包，包含掩码配置、适配器快照、GPU 信息等。
*   `notify_window_ready(window: tauri::Window)` (行 338-341): 通知窗口就绪。

### 5. 自助服务命令 (`tauri-app/src-tauri/src/commands/self_service.rs`)

处理绑定运营商账号、查询状态、注销会话等自助服务操作。

关键命令函数：

*   `bind_operator(account, password, operator, phone, sms_password)` (行 64-106): 绑定运营商账号。需要身份验证门。
*   `query_bind_status(account, password)` (行 108-136): 查询各运营商绑定状态。
*   `query_self_dashboard(account, password)` (行 138-162): 查询自助服务 Dashboard。
*   `query_self_online_log(account, password, start_time, end_time)` (行 164-202): 查询上网记录账单。
*   `self_offline_session(account, password, session_id)` (行 204-232): 注销指定在线会话。需要身份验证门。
*   `change_self_password(account, password, new_password)` (行 234-272): 修改自助服务密码。需要身份验证门。
*   `verify_windows_identity(app, consent_message)` (行 274-297): Windows 本地身份验证。成功后记录时间戳。
*   `reveal_operator_credential(account, password, operator)` (行 299-325): 查看运营商明文凭据。强制身份验证门。

辅助函数：

*   `ensure_identity_gate(state: &AppState) -> Option<String>` (行 14-22): 检查身份验证门。如果开启且未通过或过期，返回错误信息。
*   `resolve_campus_bind_addr(state: &AppState) -> Option<IpAddr>` (行 24-32): 解析校园网适配器源 IP。
*   `is_iso_date(s: &str) -> bool` (行 34-44): 严格校验 ISO 日期格式。
*   `resolve_credential_password(state: &AppState, password: &str) -> Option<String>` (行 46-58): 解析凭据密码，优先使用前端传入，否则回退已保存值。

### 6. 桌面加密 (`tauri-app/src-tauri/src/account/crypto.rs`)

*   `encrypt(text: &str) -> Result<String, String>` (行 114-122): 使用 DPAPI 加密文本，返回 base64 编码。
*   `decrypt(data: &str) -> Result<String, String>` (行 124-130): 解密 base64 编码的 DPAPI 密文。
*   `call_dpapi(input: &[u8], encrypt: bool) -> Result<Vec<u8>, String>` (行 56-73): 底层调用 CryptProtectData/CryptUnprotectData。

### 7. 桌面身份验证 (`tauri-app/src-tauri/src/platform/identity.rs`)

*   `LAST_VERIFY_EPOCH_SECS: AtomicU64`: 存储最后验证成功的时间戳。
*   `note_identity_verified()`: 记录当前时间为最后验证时间。
*   `identity_verified_recently() -> bool`: 检查是否在 TTL 内且无时钟回拨。
*   `verify_identity(message: &str, owner_hwnd: isize) -> impl Future<Output = Result<(), String>>`: 执行 Windows Hello 验证。

### 8. 安卓身份验证门 (`android/src-tauri/src/identity_gate.rs`)

*   `LAST_VERIFY_EPOCH_SECS: AtomicU64` (行 8): 存储最后验证成功的时间戳。
*   `note_identity_verified()` (行 18-20): 记录当前时间为最后验证时间。
*   `identity_verified_recently() -> bool` (行 23-33): 检查是否在 TTL 内且无时钟回拨。
*   `IDENTITY_VERIFY_TTL_SECS: u64 = 600` (行 10): 验证门有效期 600 秒。

### 9. 安卓加密插件 (`android/plugins/keystore/src/lib.rs`)

*   `CampusKeystore<R: Runtime>` (行 22-39): Rust 侧封装 Android Keystore 插件。
*   `encrypt(&self, text: &str) -> Result<String>` (行 26-30): 调用原生插件加密。
*   `decrypt(&self, data: &str) -> Result<String>` (行 32-36): 调用原生插件解密。
*   `init<R: Runtime>() -> TauriPlugin<R>` (行 42-50): 初始化插件。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                        Frontend (Webview)                   │
└──────────────┬──────────────────────────────┬───────────────┘
               │ Tauri Commands               │ Events
               ▼                              ▲
┌─────────────────────────────────────────────────────────────┐
│                     Tauri Backend                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │  Config Cmd  │  │ System Cmd   │  │ SelfService Cmd  │  │
│  └──────┬───────┘  └──────┬───────┘  └────────┬─────────┘  │
│         │                 │                    │            │
│  ┌──────▼───────┐  ┌─────▼──────┐  ┌──────────▼─────────┐  │
│  │  Persist     │  │ Identity   │  │  Identity Gate     │  │
│  │  (Encrypt)   │  │ (Verify)   │  │  (TTL Check)       │  │
│  └──────┬───────┘  └─────┬──────┘  └────────────────────┘  │
│         │                 │                                  │
│  ┌──────▼───────┐  ┌─────▼──────┐                          │
│  │  Crypto      │  │ Platform   │                          │
│  │  (DPAPI/     │  │ (WinHello) │                          │
│  │   Keystore)  │  └────────────┘                          │
│  └──────────────┘                                          │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

### 桌面保存流

1.  前端调用 `save_config`。
2.  `config_cmd.rs` 中的 `save_config` 命令接收配置。
3.  校验配置 (`validate_config`)。
4.  处理密码三态语义：如果 `clear_password` 为 true，置空；如果为空或掩码，保留已存值；否则使用新值。
5.  调用 `persist::save_config_to_disk_encrypted`。
6.  在 `persist.rs` 中，对密码进行 DPAPI 加密。
7.  使用 `atomic_write` 将加密后的配置原子写入磁盘。
8.  返回后，`config_cmd.rs` 更新内存中的 `Config`。
9.  调用 `persist::save_config_and_broadcast`，发射 `config-changed` 事件， payload 为掩码后的配置。

### 桌面导入导出流

*   **导出**: `export_config` 调用 `build_config_export_payload`。如果不包含密码，直接掩码；如果包含密码，对密码进行 DPAPI 加密，并标记 `passwordEncrypted: true`。写入文件。
*   **导入**: `import_config` 读取文件。如果是加密导出的，调用 `restore_imported_password_field` 解密密码。然后校验配置，落盘，广播。

### 安卓保存流

1.  前端调用 `save_config`。
2.  `config_state.rs` 中的逻辑处理密码回退。
3.  调用 `save_file`。
4.  在 `save_file` 中，通过 `CryptoBridge` 调用 Android Keystore 插件加密密码。
5.  将加密后的密文写入独立字段，体内密码字段置空。
6.  使用原子写入保存文件。
7.  更新缓存并发射 `emit_config_changed` 事件，payload 为掩码后的配置。

### 验证门时序

1.  前端调用 `verify_windows_identity` (桌面) 或 `verify_biometric_identity` (安卓)。
2.  桌面调用 `platform/identity.rs` 中的 `verify_identity`，弹出 Windows Hello 对话框。
3.  安卓信任前端结果，直接调用 `note_identity_verified`。
4.  验证成功后，`note_identity_verified` 记录当前时间戳。
5.  后续敏感操作（如 `bind_operator`, `reveal_operator_credential`）调用 `ensure_identity_gate`。
6.  `ensure_identity_gate` 检查 `self_hello_enabled` 开关和 `identity_verified_recently`。
7.  如果验证通过且在 TTL 内，放行；否则拦截。

## Connections

*   [[page:desktop-config]] — 配置模型、`save_config` 三态语义与 `config-changed` 广播的完整链路。
*   [[page:desktop-account-selfservice]] — 自助命令面（绑定/总览/下线/凭据查看）与验证门的交互时序。
*   [[page:android-backend]] — 安卓命令面与状态缓存（`AndroidState`）。
*   [[page:android-plugins]] — keystore 插件的 Rust/Kotlin 双侧接口。
*   [[page:ipc-command-surface]] — 双端命令注册总表（含 `verify_biometric_identity` 注册点 `android/src-tauri/src/lib.rs:71`）。
*   [[page:config-and-persistence]] — 配置结构与原子写、迁移机制。
*   [[page:dual-platform-sharing]] — 双端同构约定（回退语义、验证门、掩码出口的镜像实现）。

## Known Issues

1.  ~~桌面账号档案独立自助密码字段明文落盘~~（已消解）：凭据统一后 `save_account_config` 仅存 `password`（`persist.rs:105-108`），历史双端不一致不复存在。
2.  **`self_reverify_each_action` 无后端消费方**：桌面仅字段定义（`model.rs:21`）、Default false（`:244`）与构造默认值（`network/adapter.rs:234`）三处；安卓仅 `config_state.rs:20`/`:123`。字段存在但任何"每次操作重新验证"的策略都未实现。
3.  **安卓验证门信任前端**：`verify_biometric_identity` 被调用即写时间戳（`self_service_cmds.rs:61-66`），后端不校验 BiometricPrompt 真实结果；桌面走系统 API（`self_service.rs:272-274`）相对可信。webview 侧伪造调用即可解锁 TTL 窗口。
4.  **`redact_credentials` 覆盖面窄**：全仓库唯一调用点在登录请求错误路径（`protocol.rs:166`）；其余网络错误、诊断信息、日志均无自动脱敏。
5.  **密码校验无复杂度要求**：`validate_password` 仅非空且 ≤128 字符（`config/validate.rs:31-39`）。
6.  **查询类命令携带密码出设备**：`query_bind_status`/`query_self_dashboard`/`query_self_online_log` 有意不设门（桌面注释 `self_service.rs:10-13`），凭据经 `resolve_credential_password`（`:51-62`）回退登录密码并随请求发往校园网关——数据本就存于本机，属有意设计而非缺陷，但意味着总览卡自动刷新会周期性使用密码。
7.  **掩码出口类型收敛不对称**：桌面 `masked_for_display` 返回同构 `Config`（`model.rs:313-317`），`mask_in_place` 是 `pub`（`:319-323`）可被绕过直接调用；安卓返回 `serde_json::Value`（`config_state.rs:364-369`），出站即丢失类型。两侧类型系统都无法在编译期强制"必经出口"。
8.  **日志通道无统一脱敏钩子**：`pub fn log()`（`infra/logger.rs:314`）直写不检查内容，依赖调用方自律（登录路径靠 `protocol.rs:163-168` 手动 redact）。