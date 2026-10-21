---
title: desktop-rust-core（桌面端 Tauri 后端核心）
type: module
source_files:
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/account_cmds.rs
  - android/src-tauri/src/protocol_cmds.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/quality_cmds.rs
  - android/src-tauri/src/update_cmds.rs
tags: [tauri, rust, ipc, config, monitor-loop, updater, desktop, android-twin]
---

## Overview

本模块是 Tauri 后端的「命令面 + 状态面」骨架：应用装配（插件、`manage` 状态、`setup` 启动钩子、`invoke_handler` 命令注册）、全量配置的加密落盘与旧默认迁移、多账号命令、登录/注销/Portal 探测命令、后台监控循环（巡检 + 定时动作 + 夜间出站切换）、网络质量命令与自更新命令。

需要先说明一处与源码范围有关的事实，避免后来者误读：`source_files` 里列出的 7 个文件全部位于 `android/src-tauri/src/`，而以 `campus_login_lib::` 开头的路径（协议核心 `auth::protocol`、`auth::portal`、`network::quality`、`config::schedule`/`config::night_switch`/`config::outbound_switch`/`config::scheduled_tick`、`infra::logger`）是双端共享的桌面 crate（安卓端经 Cargo path 依赖直接继承）。这些安卓侧文件在自身文档注释里明确声明与桌面版「同名对齐/同构」：命令名与参数对齐（`android/src-tauri/src/protocol_cmds.rs:1-3`）、账号命令与桌面 `commands/account.rs` 同构（`android/src-tauri/src/account_cmds.rs:1-2`）、配置结构与桌面 `config::Config` 对齐（`android/src-tauri/src/config_state.rs:1-3`）、启动恢复对齐桌面 `startup.rs → watcher::run_startup_tasks`（`android/src-tauri/src/monitor_loop.rs:448-451`）。因此本文所写的命令面、状态机与不变量，就是桌面端同名后端的契约镜像；凡涉及桌面专属实现（DPAPI 落盘、网卡 metric/禁用/兜底路由、DNS 优化的实际执行）本文只按注释所述的名义边界标注，不复述未给出的桌面源码。

三条贯穿全模块的纪律，读代码时处处能看到：

1. **敏感信息只有一个出站出口**：`masked_for_display`（`android/src-tauri/src/config_state.rs:410-417`）是密码掩码唯一实现，`emit_config_changed` 明确要求复用它、不得另写序列化（`android/src-tauri/src/config_state.rs:419-426`）；日志里运营商名只打长度（`android/src-tauri/src/protocol_cmds.rs:36-37`）。
2. **配置读改写必须串行**：`CONFIG_IO_LOCK` 从账号命令迁到配置模块，因为 `save_config` 自己也要持锁（`android/src-tauri/src/config_state.rs:13-24`），`account_cmds::config_io_lock` 只做转发以保持既有调用点不变（`android/src-tauri/src/account_cmds.rs:18-23`）。
3. **循环生命周期用代际 token 而不是布尔标志**：`MONITOR_GENERATION`（`android/src-tauri/src/monitor_loop.rs:30-32`）、`LATENCY_GENERATION`（`android/src-tauri/src/quality_cmds.rs:14-17`）都是为了防 `stop → start` 竞速时旧循环误清新循环的运行标志。

## Key Components

### 应用装配 — `android/src-tauri/src/lib.rs`

| 项目 | 行号 | 说明 |
| --- | --- | --- |
| 模块清单（16 个子模块） | `android/src-tauri/src/lib.rs:11-26` | `campus_detect`(11)、`android_state`(12)、`config_state`(13)、`cpu_affinity`(14)、`identity_gate`(15)、`login_history`(16)、`protocol_cmds`(17)、`self_service_cmds`(18)、`monitor_loop`(19)、`account_cmds`(20)、`system_cmds`(21)、`quality_cmds`(22)、`quality_history`(23)、`update_cmds`(24)、`face_model_cmds`(25)、`battery_cmds`(26) |
| `pub fn run()` | `android/src-tauri/src/lib.rs:30-133` | 唯一入口，`#[cfg_attr(mobile, tauri::mobile_entry_point)]` |
| mobile 插件注册 | `android/src-tauri/src/lib.rs:35-42` | `campus_network_bind`(35)、`campus_keystore`(36)、`campus_monitor_service`(37)、`biometric`(38)、`notification`(39)，随后 `manage(AndroidState::default())`(40) |
| 非移动端装配 | `android/src-tauri/src/lib.rs:41-42` | host 编译下只 `manage(AndroidState)`，移动插件整段编译期消除 |
| `setup` 钩子 | `android/src-tauri/src/lib.rs:45-64` | 日志目录 `app_data_dir/logs`(47-51) → `init_logger`，失败必须 `eprintln!` 而非静默(52-56) → `monitor_loop::run_startup_tasks`(58) → `spawn(cleanup_stale_config_tmp(dir))`(60-62) |
| `invoke_handler` 命令表 | `android/src-tauri/src/lib.rs:66-122` | 共 56 条命令，覆盖协议(67-74)、校园探测(75-76)、配置(77-78)、自助服务(79-86)、账号(87-93)、系统/日志(94-100)、监控与自启(101-108)、电量(109-112)、质量(113-115)、更新(116-119)、人脸模型(120-121) |
| `build(...)` + `run(...)` | `android/src-tauri/src/lib.rs:123-132` | 构建失败 `panic!`(124)；`RunEvent::ExitRequested` 里 `api.prevent_exit()`(129-131)，防「最后窗口关闭 → `process::exit(0)`」把前台服务一起带走 |

### 配置状态 — `android/src-tauri/src/config_state.rs`

常量与锁：

| 项目 | 行号 | 说明 |
| --- | --- | --- |
| `PASSWORD_MASK: &str = "***"` | `android/src-tauri/src/config_state.rs:10` | 出站掩码占位符，也是 `resolve_password_field` 的「未修改」哨兵 |
| `CONFIG_FILE: &str = "config.json"` | `android/src-tauri/src/config_state.rs:11` | 主配置文件与账号档案共用同一文件名约定（仅目录不同） |
| `CONFIG_IO_LOCK` | `android/src-tauri/src/config_state.rs:17-19` | `tokio::sync::Mutex<()>`；用异步锁是因为 guard 需跨 `await` 持有，覆盖 `load → merge → save` 全序列(13-16) |
| `pub async fn config_io_lock() -> MutexGuard<'static, ()>` | `android/src-tauri/src/config_state.rs:22-24` | 锁的唯一获取入口，账号命令与监控循环都经此转发 |

`pub struct Settings`（`android/src-tauri/src/config_state.rs:26-128`，`#[serde(rename_all = "camelCase", default)]`）全部字段：

| 字段 | 行号 | 语义 |
| --- | --- | --- |
| `user: String` | 30 | 学号 |
| `password: String` | 31 | 明文仅存在于内存；落盘必经加密 |
| `self_hello_enabled: bool` | 32 | 自助服务问候开关 |
| `self_reverify_each_action: bool` | 33 | 每个自助动作前重新验证 |
| `allow_2d_face_verify: bool` | 34-36 | 2D 人脸回退验证（默认关，注释声明为低安全、照片可欺骗） |
| `operator: String` | 37 | 运营商后缀（`@cmcc` 等），空串=校方默认线路（无锡学院） |
| `enable_night_operator_switch: bool` | 38-41 | 运营商夜切开关（判定复用共享纯函数 `config::night_switch`） |
| `night_operator_restore: String` | 42-43 | 切至校方线路时暂存的原运营商 |
| `enable_night_outbound_switch: bool` | 44-47 | 夜间出站自动切蜂窝总开关（默认关，改系统路由属侵入性动作） |
| `outbound_priority: Vec<String>` | 48-50 | 桌面专属：出站网卡优先级，安卓不消费 |
| `outbound_metric_restore: String` | 51-53 | 桌面专属：出站切换态 metric JSON 快照 |
| `outbound_disabled_adapters: String` | 54-56 | 桌面专属：被禁用的校园网卡名单 |
| `outbound_standby_route: String` | 57-59 | 桌面专属：兜底默认路由 |
| `night_outbound_restore: String` | 60-64 | 安卓侧切换态标记，值恒为 `"logged_out"`；非空即切换态 |
| `dns_optimize_adapters: Vec<String>` | 65-67 | 桌面专属：DNS 优化目标网卡名单 |
| `device_aliases: HashMap<String, String>` | 68-72 | 12 位 hex MAC → 显示别名，容器级 `serde(default)` 保证向后兼容 |
| `auto_login_on_start: bool` | 74 | 启动自动登录 |
| `enable_background_check: bool` | 75 | 后台检测总开关 |
| `background_check_interval: u64` | 76 | 基础巡检间隔（ms，默认 60000） |
| `background_check_idle_interval: u64` | 77-81 | 闲时间隔（蜂窝/灭屏，默认 300000） |
| `auto_login_on_preparation: bool` | 82 | 非掉线场景（启动即离线）是否也自动登录 |
| `max_disconnect_reconnect: u32` | 83 | 单次在线周期内重连次数上限（默认 3） |
| `auto_login_cooldown_secs: u64` | 84 | 重登冷却（默认 60s） |
| `theme_mode: String` | 86 | 主题（默认 `"dark"`） |
| `enable_notification: bool` | 87 | 系统通知总闸 |
| `custom_theme_color: String` | 88 | 主色（默认 `#6366f1`） |
| `default_panel: String` | 89 | 默认面板（默认 `"dashboard"`） |
| `display_name: String` | 90-93 | 当前账号显示名；空 → 读取阶段兜底为账号 id |
| `active_account: String` | 94 | 当前激活账号 id（文件名 stem） |
| `enable_boot_autostart: bool` | 95 | 开机自启（默认 false，明确不属于登录自动化） |
| `enable_latency_test: bool` | 97 | 定时质量测试开关 |
| `latency_test_interval: u64` | 98 | 质量测试间隔（默认 600000，2026-09-20 由 60000 提升） |
| `enable_network_quality: bool` | 99 | 网络质量检测开关（默认 false，省电） |
| `skip_ttfb_in_latency: bool` | 100 | 跳过 TTFB 分段 |
| `skip_content_in_latency: bool` | 101 | 跳过内容分段 |
| `portal_url: String` | 103 | Portal 地址（默认 `http://10.1.99.100`） |
| `fixed_gateway: String` | 104 | 固定网关（默认 `10.2.127.254`） |
| `required_network_name: String` | 105 | SSID 名单（默认三值 `i-wxxy`/`iwxxy-2`/`iwxxy-3`） |
| `enable_network_name_check: bool` | 106 | SSID 名称检查开关 |
| `campus_gateway: String` | 107 | 校园网关（默认 `10.2.127.254`） |
| `campus_check_start_minutes: u16` | 108-109 | 检测时段起点（当日分钟数，0=禁用，默认 460=07:40） |
| `campus_check_end_minutes: u16` | 110-111 | 检测时段终点（默认 1380=23:00；`<=` 起点时退化为仅起点限制） |
| `scheduled_login_minutes: u16` | 112-113 | 定时登录时刻（1440=禁用哨兵，0=真实的 00:00） |
| `scheduled_logout_minutes: u16` | 114-115 | 定时注销时刻（同上语义） |
| `update_source: String` | 116-118 | 更新渠道优先级 `"mirror"`(默认)/`"github"` |
| `log_retention_days: u32` | 120 | 日志保留天数（默认 7） |
| `config_schema_version: u32` | 121-127 | 配置结构版本；旧文件缺字段反序列化为 0 触发迁移，新装即 7 |

方法与函数：

| 项目 | 行号 | 说明 |
| --- | --- | --- |
| `impl Default for Settings` → `fn default()` | `android/src-tauri/src/config_state.rs:130-200` | 全部默认值；关键项：`background_check_interval=60_000`(155)、`background_check_idle_interval=300_000`(157)、`latency_test_interval=600_000`(170)、`scheduled_*=1440`(192-193)、`config_schema_version=7`(197) |
| `pub struct CryptoBridge` | `android/src-tauri/src/config_state.rs:202-207` | 两字段：`encrypt: Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>`、`decrypt: Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>`；用函数指针桥接是为了 host 测试可注入可逆假实现 |
| `CryptoBridge::from_app(&tauri::AppHandle) -> Self` | `android/src-tauri/src/config_state.rs:209-240` | mobile：走 `campus_keystore()` 的 `encrypt/decrypt`(213-229)；非 mobile：两个闭包恒 `Err("Keystore 仅移动端可用")`(235-237) |
| `struct EncodedSettings`（私有） | `android/src-tauri/src/config_state.rs:242-248` | 磁盘形态两字段：`settings: Settings`、`password_cipher: String`（`camelCase`） |
| `pub async fn load_file(path, bridge) -> Result<Settings, String>` | `android/src-tauri/src/config_state.rs:250-263` | 文件不存在返回 `Settings::default()`(253)；解析失败 `Err("配置损坏")`(255-256)；解密密文失败**置空而非报错**(259-261) |
| `pub async fn save_file(path, bridge, s) -> Result<(), String>` | `android/src-tauri/src/config_state.rs:292-322` | 建目录(266-270) → 非空密码加密、失败即返回 `Err`(271-278) → 落盘时 `settings.password` 显式置空(280) → 带 pid 的临时文件名(284-291) → `write` + `rename`(292-293) |
| `pub async fn cleanup_stale_config_tmp(dir: PathBuf)` | `android/src-tauri/src/config_state.rs:297-311` | 删 `config.json.*.tmp` 形状的崩溃遗留；不区分 pid(298-299) |
| `pub async fn load_from(dir, bridge) -> Result<Settings, String>` | `android/src-tauri/src/config_state.rs:313-323` | `load_file` + `migrate_legacy_defaults`(314-315) + 旧默认单值 SSID 扩三值(319-321，幂等) |
| `async fn migrate_legacy_defaults(dir, bridge, s)`（私有） | `android/src-tauri/src/config_state.rs:325-404` | v0→1 间隔 15s→60s(344-346)；v1→3 批量刷开自动化/验证/跳过项并关质量检测(347-361)；v4 补 `background_check_idle_interval`(362-370)；v5 检测终点 0→1380(371-379)；v6 质量间隔 60s→600s(380-388)；v7 夜切刷开 + 定时 0→1440(389-403)。每次落盘失败只 `log_warn!`，下次读盘重迁（幂等） |
| `pub async fn save_to(dir, bridge, s)` | `android/src-tauri/src/config_state.rs:406-408` | `dir/config.json` 的 `save_file` 薄包装 |
| `pub fn masked_for_display(s: &Settings) -> serde_json::Value` | `android/src-tauri/src/config_state.rs:410-417` | 密码非空 → `"***"`，序列化失败返回 `Value::Null` |
| `pub async fn emit_config_changed(app, s)` | `android/src-tauri/src/config_state.rs:419-426` | 广播 `config-changed`，payload `{ "config": 掩码后配置 }` |
| `pub fn resolve_password_field(incoming, current, clear) -> String` | `android/src-tauri/src/config_state.rs:428-438` | `clear=true` → 空；空串或 `"***"` → 回退已存值；否则取新值 |
| `#[tauri::command] pub async fn get_config(app)` | `android/src-tauri/src/config_state.rs:440-453` | 读盘 + 写回内存缓存(448-451) + 返回掩码值(452) |
| `#[tauri::command] pub async fn save_config(app, config: Settings, clear_password: Option<bool>)` | `android/src-tauri/src/config_state.rs:481-512` | 全程持锁(461-464) → `load_from`(470) → `resolve_password_field`(471-473) → `save_to`(474) → 刷缓存(475-478) → `auto_create_account_for_current`(481) → `emit_config_changed`(484) |
| `pub async fn current_settings(app) -> Result<Settings, String>` | `android/src-tauri/src/config_state.rs:488-507` | 缓存优先(492-496)，未命中读盘并回填(497-505)；带 `#[allow(dead_code)]`(489) |

### 多账号命令 — `android/src-tauri/src/account_cmds.rs`

| 项目 | 行号 | 说明 |
| --- | --- | --- |
| `ACCOUNT_NAME_RE` | `android/src-tauri/src/account_cmds.rs:12-15` | `^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$`，账号文件名白名单 |
| `pub async fn config_io_lock()` | `android/src-tauri/src/account_cmds.rs:20-22` | 转发 `config_state::config_io_lock`，保持既有调用点（`monitor_loop` 等）不变 |
| `pub struct AccountItem` | `android/src-tauri/src/account_cmds.rs:27-40` | 字段：`id: String`(29)、`display_name: String`（`#[serde(rename = "displayName")]`，30-31）、`user: String`(34-35)、`operator: String`(36-37)、`password_saved: bool`（`rename = "passwordSaved"`，38-39） |
| `pub struct AccountResult` | `android/src-tauri/src/account_cmds.rs:42-55` | 字段：`success: bool`(45)、`message: Option<String>`(46-47)、`active_account: Option<String>`(48-49)、`display_name: Option<String>`(50-52)、`config: Option<serde_json::Value>`(53-54)；全部 Option 带 `skip_serializing_if` |
| `AccountResult::ok` / `ok_with_account` / `err` | `android/src-tauri/src/account_cmds.rs:57-67` | 三个构造器(58-60/61-63/64-66)，`err` 只带 `message` |
| `pub fn sanitize_account_id(user: &str) -> String` | `android/src-tauri/src/account_cmds.rs:69-83` | 白名单字符外的字符 → `_`，按字符截断 32(81)；空输入 → 空输出，全非法 → 全下划线 |
| `pub fn validate_account_name(name: &str) -> Result<String, String>` | `android/src-tauri/src/account_cmds.rs:85-94` | 1..=32 字符且匹配正则，防路径穿越 |
| `pub fn accounts_dir(app) -> Result<PathBuf, String>` | `android/src-tauri/src/account_cmds.rs:96-102` | `app_data_dir/accounts` |
| `fn read_account_card_info(path)`（私有） | `android/src-tauri/src/account_cmds.rs:104-117` | 不解密读取 `settings.displayName/user/operator` 与 `password` 是否非空(111-116) |
| `pub fn list_account_items_sync(dir) -> Vec<AccountItem>` | `android/src-tauri/src/account_cmds.rs:119-151` | 只认 `.json`(126)、跳过隐藏与空 id(133-135)、空 `displayName` 兜底为 id(138)、读取失败 `log_warn` 后兜底空字段(141-145)、按 id 排序(149) |
| `#[tauri::command] pub async fn list_accounts(app)` | `android/src-tauri/src/account_cmds.rs:156-162` | `spawn_blocking` 包同步列举 |
| `async fn load_account_file(path, bridge)` | `android/src-tauri/src/account_cmds.rs:161-166` | 不存在返回 `Ok(None)`，存在则 `load_file` |
| `async fn persist_current(app, merged) -> Result<Value, String>` | `android/src-tauri/src/account_cmds.rs:174-185` | `save_to` + 刷内存缓存 + 返回掩码值 |
| `#[tauri::command] pub async fn switch_account(app, account_name)` | `android/src-tauri/src/account_cmds.rs:207-249` | 校验名(186-189) → 持锁(194) → 合并 `user/password/operator`(202-204) → 清 `night_operator_restore`(207) 与 `night_outbound_restore`(210) → `display_name` 兜底(211-215) → `active_account`(216) → 落盘(218) → 收尾 `restore_avoid_bad_wifi`(221) |
| `#[tauri::command] pub async fn save_current_as_account(app, account_name)` | `android/src-tauri/src/account_cmds.rs:225-253` | 持锁(238) → `merge_save_as_target`(243) → 写档案(244) → 主配置同步 `active_account`/`display_name`(248-251) |
| `#[tauri::command] pub async fn create_account(app, account_name, user, password, operator)` | `android/src-tauri/src/account_cmds.rs:255-290` | 学号非空校验(267-269)、重名拒绝(275-277)、新建档案只含登录字段+显示名且不切换主配置(278-287) |
| `fn merge_save_as_target(current, existing, safe_name) -> (Settings, String)`（私有） | `android/src-tauri/src/account_cmds.rs:292-327` | 另存（`active_account != safe_name`）时档案 `display_name` 置空以避串号(303-308)；目标已存在则保留非登录字段、仅覆盖登录字段(310-319)；返回主配置最终显示名(321-325) |
| `#[tauri::command] pub async fn delete_account(app, account_name)` | `android/src-tauri/src/account_cmds.rs:329-358` | 持锁(341) → 存在则删文件(342-344) → 若删的是当前账号：清 `active_account`(348)、清出站标记(351)、落盘、收尾 `restore_avoid_bad_wifi`(354) |
| `#[tauri::command] pub async fn get_active_account(app)` | `android/src-tauri/src/account_cmds.rs:360-363` | 返回 `active_account` |
| `#[tauri::command] pub async fn rename_account(app, account_id, display_name)` | `android/src-tauri/src/account_cmds.rs:365-397` | 业务错误走 `Err(String)` 通道（与 switch 的 `AccountResult::err` 不同，373-376）；改名后若为激活账号则同步主配置 `display_name`(387-390) |
| `async fn rename_account_core(dir, bridge, account_id, display_name)`（私有） | `android/src-tauri/src/account_cmds.rs:399-425` | 校验显示名(407) → 存在性(409-411) → 重名检查排除自身(416-420) → 只改 `display_name` 写回(422-424) |
| `fn validate_display_name(name) -> Result<String, String>`（私有） | `android/src-tauri/src/account_cmds.rs:427-443` | trim 后 1..=32 字符(431-438)、拒绝控制字符(439-441)、允许空格与 emoji；**不复用**文件名白名单(428-429) |
| `pub async fn auto_create_account_for_current(app, current)` | `android/src-tauri/src/account_cmds.rs:445-456` | `save_config` 落盘成功后调用，失败只 `log_warn`，不影响保存结果 |
| `async fn auto_create_account_in(dir, bridge, config)`（私有） | `android/src-tauri/src/account_cmds.rs:458-506` | 五条规则见注释(459-467)：无凭据跳过、id 空跳过、不存在则建、同 id 不同 user 原文视为撞库跳过、已存在且 user 一致则幂等短路或仅更新三字段保留显示名 |

### 登录/注销/Portal 探测 — `android/src-tauri/src/protocol_cmds.rs`

| 项目 | 行号 | 说明 |
| --- | --- | --- |
| `#[tauri::command] pub async fn do_login(user, password, operator, adapter, app, state)` | `android/src-tauri/src/protocol_cmds.rs:9-63` | 先 `ensure_wifi_bound`(21)；参数为空回退配置(23-31)；空凭据拒绝(32-35)；日志只打长度(36-37)；`run_login`(44)；`login_history::append(..., "manual")`(46-52)；成功后清熔断/注销保护、置 `was_online`(56-61) |
| `pub async fn run_login(user, password, operator, state) -> Result<Value, String>` | `android/src-tauri/src/protocol_cmds.rs:65-90` | 取缓存源 IP(72) → `spawn_blocking` 调同步的 `campus_login_lib::auth::protocol::do_login_with_retry`（重试 3 次，76-87） |
| `#[tauri::command] pub async fn do_logout(user, adapter, app, state)` | `android/src-tauri/src/protocol_cmds.rs:115-166` | 先绑 WiFi(101)；user 空回退配置(103-107)；`do_logout_with_retry`（3 次，113-123）；写历史(125-131)；置 60s 注销保护期(134-141) |
| `#[tauri::command] pub async fn check_portal_status(adapter_ip, app, state)` | `android/src-tauri/src/protocol_cmds.rs:145-164` | 先绑 WiFi(153)；IP 回退缓存(154-157)；`spawn_blocking(check_portal_full)`(159-163) |
| `fn cached_adapter_ip(state) -> Option<String>`（私有） | `android/src-tauri/src/protocol_cmds.rs:166-174` | 读 `AndroidState::cached_source_ip` |
| `#[tauri::command] pub fn ping_test() -> &'static str` | `android/src-tauri/src/protocol_cmds.rs:176-179` | 返回 `"pong"`，前端探活 |
| `#[tauri::command] pub fn bind_to_wifi(app)` | `android/src-tauri/src/protocol_cmds.rs:181-196` | mobile 走插件 `bind_to_wifi`(184-190)；非 mobile 恒 `Err("网络绑定仅安卓端支持")`(191-195) |
| `#[tauri::command] pub fn accept_wifi_network(app)` | `android/src-tauri/src/protocol_cmds.rs:198-214` | 同上，让系统接受无互联网 WiFi；非 mobile 恒 Err(209-213) |
| `#[tauri::command] pub async fn get_avoid_bad_wifi_status(app)` | `android/src-tauri/src/protocol_cmds.rs:217-236` | 查 `WRITE_SECURE_SETTINGS` 授权、`avoid_bad_wifi` 值、插件是否有待还原快照；JNI 走 `spawn_blocking`(224-228)；非 mobile 恒 Err(230-234) |
| `#[tauri::command] pub async fn restore_written_settings(app)` | `android/src-tauri/src/protocol_cmds.rs:237-255` | 按插件快照写回 `captive_portal_mode`/`network_avoid_bad_wifi`；非 mobile 恒 Err(250-254) |
| `pub(crate) async fn ensure_wifi_bound(app)` | `android/src-tauri/src/protocol_cmds.rs:257-338` | 旁路代理生效（`bind_capability()==Supported`）时跳过(274-279)；绑定成功后**清空 HTTP 客户端池**(291-295)，但 `path=="already_bound"` 时不清(288-292)；顺带 `accept_campus_wifi`(316)；结果只记日志不阻断(286-337) |
| `pub(crate) async fn accept_campus_wifi(app)` | `android/src-tauri/src/protocol_cmds.rs:345-390` | 三路径尝试（反射 hidden API `setAcceptUnvalidated` → 写 `Settings.Global`），无动作时返回 `already_validated`(342-346)；失败只记日志(348-349) |

### 后台监控循环 — `android/src-tauri/src/monitor_loop.rs`

进程级状态与常量：

| 项目 | 行号 | 说明 |
| --- | --- | --- |
| `MONITOR: MonitorState` | `android/src-tauri/src/monitor_loop.rs:15-17` | `lazy_static` 全局单例 |
| `OUTBOUND_RESTORE_FAIL_COUNT` / `OUTBOUND_RESTORE_MAX_FAILS = 3` | `android/src-tauri/src/monitor_loop.rs:19-23` | 夜间出站还原失败上限 |
| `OUTBOUND_SWITCH_FAIL_COUNT` / `OUTBOUND_SWITCH_MAX_FAILS = 3` | `android/src-tauri/src/monitor_loop.rs:24-28` | 夜间出站注销失败上限 |
| `MONITOR_GENERATION: AtomicU64` | `android/src-tauri/src/monitor_loop.rs:30-32` | 循环代际 token |
| `LOGIN_ATTEMPT_LOCK` | `android/src-tauri/src/monitor_loop.rs:34-37` | 「判在线 → 判定 → 登录 → 计数」段互斥；探测 IO 不持锁 |
| `struct MonitorState` | `android/src-tauri/src/monitor_loop.rs:39-69` | 字段见下 |
| `WIFI_EVENT_DELAY_MS = 2500` / `WIFI_LOST_EVENT_DELAY_MS = 0` / `WIFI_EVENT_DEBOUNCE_MS = 1000` | `android/src-tauri/src/monitor_loop.rs:71-82` | 连上方向延迟等 DHCP；断开方向零延迟；去抖窗口 |

`MonitorState` 全部字段（`android/src-tauri/src/monitor_loop.rs:39-69`）：

| 字段 | 行号 | 说明 |
| --- | --- | --- |
| `running: AtomicBool` | 42 | 循环运行标志 |
| `check_count: AtomicU64` | 43 | 累计检测次数 |
| `consecutive_failures: AtomicU32` | 44 | 连续登录失败（熔断阈值 5） |
| `reconnect_count: AtomicU32` | 45-46 | 本次在线周期内自动重连次数 |
| `last_login_attempt_ms: AtomicU64` | 47 | 上次登录尝试（cooldown 用） |
| `was_online: AtomicBool` | 48 | 上一拍在线记忆 |
| `last_result: Mutex<Option<serde_json::Value>>` | 49 | 最近一次检测 payload |
| `desired_interval_ms: AtomicU64` | 50-52 | 目标基础间隔，支持「改间隔立即生效」 |
| `idle_interval_ms: AtomicU64` | 53-54 | 闲时间隔 |
| `logout_protected_until_ms: AtomicU64` | 55-56 | 注销保护期截止（epoch ms） |
| `wifi_event_ms: AtomicU64` | 57-58 | 最近 WiFi 事件时间（去抖 + 风暴内最后事件生效） |
| `notified_online: AtomicU8` | 59-62 | 常驻通知已展示状态码（0/1/2/3/4） |
| `scheduled_login_day: AtomicI32` | 63-66 | 定时登录当日已触发标记（`num_days_from_ce`） |
| `scheduled_logout_day: AtomicI32` | 67-68 | 定时注销当日已触发标记 |

纯函数与状态出口：

| 项目 | 行号 | 说明 |
| --- | --- | --- |
| `fn wifi_event_is_burst_start(last_event_ms, now_ms, debounce_ms) -> bool`（私有） | `android/src-tauri/src/monitor_loop.rs:84-89` | 首事件或超出窗口才是风暴起点 |
| `pub fn effective_interval_ms(base_ms, idle_ms, screen_on, wifi_connected) -> u64` | `android/src-tauri/src/monitor_loop.rs:687-699` | 亮屏+WiFi → 基础间隔（下限 5000，97）；其余 → `idle_ms.max(base)` |
| `pub fn should_attempt_login(online, was_online, on_campus, auto_login_on_preparation, reconnect_count, max_reconnect, millis_since_last_attempt, cooldown_secs) -> bool` | `android/src-tauri/src/monitor_loop.rs:105-128` | 四道判定：在线/非校园网(117-119)、启动即离线需显式开关(120-123)、次数上限(124-126)、cooldown(127) |
| `pub fn status_value() -> serde_json::Value` | `android/src-tauri/src/monitor_loop.rs:130-152` | 出站形状 `isRunning/checkCount/consecutiveFailures/reconnectCount/lastResult/watcherFailed`，并把 `online/message/onlineOperator/serverAvailable/onCampusNetwork` 展平到顶层(144-150) |
| `pub fn is_running() -> bool` | `android/src-tauri/src/monitor_loop.rs:154-157` | 读 `running`（带 `dead_code` 标注） |
| `fn epoch_ms() -> u64`（私有） | `android/src-tauri/src/monitor_loop.rs:159-164` | 系统时间 → ms |
| `fn emit_login_log(app, message, log_type)`（私有） | `android/src-tauri/src/monitor_loop.rs:166-170` | 广播 `login-log` |
| `pub(crate) fn notify_system(app, enabled, title, body, mascot)` | `android/src-tauri/src/monitor_loop.rs:172-192` | 系统通知；`mascot` 大图失败降级为纯文本(182-191) |
| `fn with_vpn_hint(body) -> String`（私有） | `android/src-tauri/src/monitor_loop.rs:194-204` | 检测到 VPN tun 时追加「请排除本应用」提示 |
| `fn notify_state(online, wifi_off) -> (u8, &'static str)`（私有） | `android/src-tauri/src/monitor_loop.rs:1576-1586` | 1=在线、4=WiFi 未连接、2=未连接（3 被「非检测时段」占用） |

命令与生命周期：

| 项目 | 行号 | 说明 |
| --- | --- | --- |
| `#[tauri::command] pub async fn start_background_check(app)` | `android/src-tauri/src/monitor_loop.rs:206-256` | 刷新 `desired/idle` 间隔(210-214)；已在跑则仅刷间隔并重发 FGS 保活(215-228)；否则先起 FGS 再置 `running`(231-242)、重置通知记忆(244)、起 WiFi watcher(247-248)、推进代际并 spawn `monitor_tick_loop`(250-253) |
| `#[tauri::command] pub async fn stop_background_check(app)` | `android/src-tauri/src/monitor_loop.rs:258-271` | **先推进代际再清标志**(261-262)，停 watcher(263)、停 FGS(264-268) |
| `WIFI_WATCHER_CHANNEL_ID` / `WIFI_WATCHER_ACTIVE` | `android/src-tauri/src/monitor_loop.rs:273-277` | watcher channel id 与存活标志（均 `#[cfg(mobile)]`） |
| `pub(crate) fn start_wifi_watcher(app)` | `android/src-tauri/src/monitor_loop.rs:279-304` | 注册失败置 `WATCHER_FAILED=true` 降级为纯周期检测(297-302)，经 `get_background_status` 暴露 |
| `pub(crate) fn stop_wifi_watcher(app)` | `android/src-tauri/src/monitor_loop.rs:306-320` | `swap(false)` 幂等退出，按 channel id 注销 |
| `fn handle_wifi_event(app, body) -> tauri::Result<()>`（私有） | `android/src-tauri/src/monitor_loop.rs:322-363` | 未运行时忽略(330-332)；解析 `event`(333-339)；去抖(341-347)；`lost` 零延迟(350)；延迟后校验时间戳未被更新才执行 `run_check_once`(355-360) |
| `#[tauri::command] pub async fn trigger_background_check(app)` | `android/src-tauri/src/monitor_loop.rs:365-374` | 未运行时拒绝，避免孤立检测(369-371) |
| `#[tauri::command] pub fn get_background_status() -> Value` | `android/src-tauri/src/monitor_loop.rs:376-379` | `status_value()` 直出 |
| `#[tauri::command] pub async fn get_boot_autostart(app)` | `android/src-tauri/src/monitor_loop.rs:381-397` | mobile 查插件(384-390)；非 mobile 恒 `Ok(false)`(392-396) |
| `#[tauri::command] pub async fn set_boot_autostart(app, enabled)` | `android/src-tauri/src/monitor_loop.rs:399-422` | 插件组件启停(401-407) + 持锁改配置落盘(408-416) + 刷缓存(418-420) |
| `#[tauri::command] pub async fn get_notification_enabled(app)` | `android/src-tauri/src/monitor_loop.rs:424-428` | 读 `enable_notification` |
| `#[tauri::command] pub async fn set_notification_enabled(app, enabled)` | `android/src-tauri/src/monitor_loop.rs:430-446` | 持锁改配置落盘 + 刷缓存 |

启动与巡检编排：

| 项目 | 行号 | 说明 |
| --- | --- | --- |
| `pub fn run_startup_tasks(app)` | `android/src-tauri/src/monitor_loop.rs:448-529` | 500ms 就绪窗口(460) → 读配置，失败则 `log_error` + emit `startup-failed` + 停 FGS(461-484) → 应用日志保留天数(488) → 并行 spawn 后台检测(489-494)/avoid_bad_wifi 快照兜底(498-500)/质量首测或定时测试(501-513)/更新检查循环(515)/清理残留安装包(518)/启动自动登录(519-527) |
| `async fn probe_with_retry(app, settings)`（私有） | `android/src-tauri/src/monitor_loop.rs:531-553` | 首次不通过则 3s 后重试一次；retry 为 Err 时回退首次结果(548-552) |
| `fn emit_auto_login_result(app, success, message)`（私有） | `android/src-tauri/src/monitor_loop.rs:555-565` | 广播 `auto-login-result`（成功与失败都发） |
| `async fn auto_login_on_start(app, settings)`（私有） | `android/src-tauri/src/monitor_loop.rs:1453-1498` | 无凭据直接返回(570-572) → 绑 WiFi(574) → 探测(576-582) → 缓存源 IP(583) → 非校园网跳过(584-587) → `run_login`(589)，成功置 `was_online` 并清注销保护(596-597)，失败走系统通知 + 应用内 toast(600-610) |
| `struct ProbeWindowGuard(AppHandle)` + `impl Drop` | `android/src-tauri/src/monitor_loop.rs:614-625` | 探针窗口 WifiLock/WakeLock，Drop（含 early return 与 panic unwind）必释放(620-625) |
| `fn power_state(app) -> (bool, bool)` | `android/src-tauri/src/monitor_loop.rs:627-641` | mobile 查插件、失败保守按 `(true, true)`(631-636)；非 mobile 恒 `(true, true)`(638-641) |
| `async fn monitor_tick_loop(app, interval_ms, gen)`（私有） | `android/src-tauri/src/monitor_loop.rs:643-702` | 下限 5s(645)；启动对账：当日注销时刻已过则消耗额度不执行(648-665)；每拍校验代际(668-671) → `run_scheduled_actions`(676) → 间隔热更新重建计时器(677-684) → 分档跳拍(685-693) → `run_check_once`(695)；退出时仅代际匹配才清 `running`(697-701) |
| `async fn outbound_switch_active(app) -> bool`（私有） | `android/src-tauri/src/monitor_loop.rs:704-711` | `night_outbound_restore` 非空即切换态；读取失败按未切换 |
| `async fn latest_settings(app, fallback) -> Settings`（私有） | `android/src-tauri/src/monitor_loop.rs:713-723` | 重读配置防旧快照覆盖新值 |
| `async fn logout_for_outbound_switch(app) -> bool`（私有） | `android/src-tauri/src/monitor_loop.rs:725-756` | 前置在线检查(732-738)；判据取 `radiusOk || unbindOk`(742-752) |
| `fn switch_logout_unconfirmed(app, reason) -> bool`（私有） | `android/src-tauri/src/monitor_loop.rs:758-778` | 未达 3 次 → false 下一拍重试；达上限 → true 并 warn(763-769) |
| `async fn portal_probe_on_little_cores(ip)`（私有） | `android/src-tauri/src/monitor_loop.rs:803-844` | 独立短命线程 + 绑小核(795-799)；**线程内需 `Handle::enter()`** 才有 reactor context(789-793)；`catch_unwind` 把 panic 转为 Err(800-805)；线程创建失败降级 `spawn_blocking`(811-819) |
| `async fn run_scheduled_actions(app)`（私有） | `android/src-tauri/src/monitor_loop.rs:943-1157` | 夜间出站切换（`evaluate_outbound_action`，849-926）→ 重读配置(929) → 运营商夜切（`evaluate_night_switch_gated`，939-980）→ 双禁用早退(981-984) → 定时登录/注销判定（`evaluate_scheduled_actions`，993-1001）与执行(1002-1036) |
| `async fn persist_settings(app, s) -> bool`（私有） | `android/src-tauri/src/monitor_loop.rs:1059-1085` | 落盘 + 刷缓存 + `auto_create_account_for_current` + `emit_config_changed`(1063) |
| `async fn night_switch_login(app, settings) -> bool`（私有） | `android/src-tauri/src/monitor_loop.rs:1453-1483` | 夜切专用登录内核：无校园网探测闸(1067-1070)，成功置 `was_online`、清注销保护(1086-1088) |
| `async fn trigger_wifi_recheck(app)`（私有） | `android/src-tauri/src/monitor_loop.rs:1107-1132` | `report_wifi_unusable()` 触发系统重检；`reported=false` 按 warn 并带 reason(1114-1120) |
| `async fn ensure_avoid_bad_wifi(app)`（私有） | `android/src-tauri/src/monitor_loop.rs:1126-1165` | 程序化确保 `network_avoid_bad_wifi=1`（需 `WRITE_SECURE_SETTINGS`），插件快照原值 |
| `pub(crate) async fn restore_avoid_bad_wifi(app)` | `android/src-tauri/src/monitor_loop.rs:1167-1192` | 按快照还原；无快照是正常路径不打日志(1183-1184) |
| `NIGHT_VERIFY_WAIT_SECS=15` / `RETRY_SECS=5` / `ATTEMPTS=3` / `HTTP_TIMEOUT_SECS=15` | `android/src-tauri/src/monitor_loop.rs:1194-1199` | 夜切验证时序参数 |
| `fn portal_origin(portal_url) -> String`（私有） | `android/src-tauri/src/monitor_loop.rs:1201-1214` | 取 scheme+host；空值回退 `config::model::default_portal_url()`(1205-1208) |
| `fn expected_operator_display(expected) -> &str`（私有） | `android/src-tauri/src/monitor_loop.rs:1216-1223` | 空串 → 「无锡学院」 |
| `async fn verify_night_switch(app, expected_operator)`（私有） | `android/src-tauri/src/monitor_loop.rs:1225-1283` | 两处出站切换态守卫(1232-1234/1252-1254)；`operator` 漂移守卫(1241-1243/1261-1263)；未生效则注销→重登→复验(1264-1269)；仍不符发 warn + 系统通知(1272-1281) |
| `async fn night_switch_verify_round(settings, expected) -> Result<String, String>`（私有） | `android/src-tauri/src/monitor_loop.rs:1285-1304` | 3 次重试、间隔 5s，返回最后错误 |
| `async fn night_switch_check_once(client, portal_url, user, expected) -> Result<String, String>`（私有） | `android/src-tauri/src/monitor_loop.rs:1327-1357` | `GET {origin}/drcom/chkstatus?callback=dr1003`(1313-1316)，复用共享纯函数 `parse_chkstatus`/`uid_matches`(1327-1334) |
| `pub async fn run_check_once(app)` | `android/src-tauri/src/monitor_loop.rs:1338-1574` | 全入口共用的单拍检测，流程见 Data Flow 第 2 节 |

### 网络质量 — `android/src-tauri/src/quality_cmds.rs`

| 项目 | 行号 | 说明 |
| --- | --- | --- |
| `LATENCY_RUNNING: AtomicBool` | `android/src-tauri/src/quality_cmds.rs:11-13` | 幂等运行标志 |
| `LATENCY_GENERATION: AtomicU64` | `android/src-tauri/src/quality_cmds.rs:14-17` | 代际 token，防旧循环误清新标志 |
| `async fn run_quality_once(app) -> NetworkQualityResult`（私有） | `android/src-tauri/src/quality_cmds.rs:19-49` | 读配置、取缓存源 IP(23-29)，调共享 `check_network_quality_async("wlan0", ...)`(31-41)，并落一条质量历史(42-47) |
| `#[tauri::command] pub async fn check_network_quality(app)` | `android/src-tauri/src/quality_cmds.rs:51-54` | 立即检测一次 |
| `#[tauri::command] pub async fn start_latency_test(app)` | `android/src-tauri/src/quality_cmds.rs:56-74` | 幂等闸(59-61)；读配置失败**必须复位标志**(65-68，否则永久卡 true)；推进代际并 spawn `latency_loop`(71-72) |
| `#[tauri::command] pub fn stop_latency_test()` | `android/src-tauri/src/quality_cmds.rs:76-83` | 先推进代际再清标志(80-81) |
| `GOOD_LEVELS = ["excellent","great","good"]` / `BACKOFF_STABLE_STREAK = 5` / `BACKOFF_MAX_MS = 1_800_000` | `android/src-tauri/src/quality_cmds.rs:85-91` | 稳态退避参数 |
| `fn next_backoff_interval_ms(current_ms, stable_good_streak, base_ms) -> u64`（私有） | `android/src-tauri/src/quality_cmds.rs:93-101` | 稳定计数每达 5 拍翻倍，封顶 1800s 且不低于基础间隔 |
| `async fn latency_loop(app, settings, gen)`（私有） | `android/src-tauri/src/quality_cmds.rs:103-134` | 基础间隔下限 10s(104)；**先检测后 sleep**（107-109 说明原 `interval` 首 tick 立即到期的缺陷）；good 及以上累加、波动清零(116-126)；退出仅代际匹配才清标志(131-133) |

### 自更新 — `android/src-tauri/src/update_cmds.rs`

常量与全局：

| 项目 | 行号 | 说明 |
| --- | --- | --- |
| `UPDATE_LOOP_RUNNING: AtomicBool` | `android/src-tauri/src/update_cmds.rs:15-16` | 检查循环幂等标志 |
| `UPDATE_CHECKSUM: Mutex<Option<(String, String)>>` | `android/src-tauri/src/update_cmds.rs:17-21` | 记账 `(release tag, digest)`；哨兵 `(tag, "")` 表示「API 在线但无 digest」 |
| `VERSION_FILE` | `android/src-tauri/src/update_cmds.rs:24` | `version.json` 官方地址 |
| `AUTO_CHECK_INTERVAL_SECS = 24*3600` | `android/src-tauri/src/update_cmds.rs:26` | 24h 周期 |
| `BACKOFF_RETRY_SECS = [5min, 15min, 1h]` | `android/src-tauri/src/update_cmds.rs:27-29` | 失败退避档位 |
| `LAST_CHECK_ERROR: Mutex<Option<String>>` | `android/src-tauri/src/update_cmds.rs:31-35` | 最近检查失败原因（原实现静默吞 Err） |
| `LAST_CHECK_TIME_MS: AtomicU64` | `android/src-tauri/src/update_cmds.rs:36-37` | 最近检查完成时间 |
| `VERSION_MIRRORS`（3 个） | `android/src-tauri/src/update_cmds.rs:38-43` | `ghfast.top` / `gh-proxy.com` / `ghproxy.net` |
| `DOWNLOAD_ALLOWED_HOSTS`（8 个） | `android/src-tauri/src/update_cmds.rs:45-55` | 防 SSRF 的域名白名单 |
| `MAX_DOWNLOAD_BYTES = 500MiB` | `android/src-tauri/src/update_cmds.rs:57` | 下载上限 |
| `APK_STALE_SECS = 24h` | `android/src-tauri/src/update_cmds.rs:59-62` | 残留安装包保留时长 |

类型（全部字段）：

| 类型 | 行号 | 字段 |
| --- | --- | --- |
| `pub struct ReleaseAsset` | `android/src-tauri/src/update_cmds.rs:73-78` | `name: String`(75)、`url: String`(76)、`size: u64`(77) |
| `pub struct UpdateInfo` | `android/src-tauri/src/update_cmds.rs:80-94` | `has_update: bool`(83)、`latest_version: String`(84)、`release_notes: String`(85)、`assets: Vec<ReleaseAsset>`(86)、`sha256_checksum: Option<String>`(87)、`last_check_error: Option<String>`(88-91)、`last_check_time: Option<u64>`(92-93)；`camelCase + default`(81) |
| `pub struct DownloadProgress` | `android/src-tauri/src/update_cmds.rs:96-102` | `downloaded: u64`(98)、`total: u64`(99)、`speed: u64`(100)、`percent: f64`(101) |
| `struct VersionFile`（私有） | `android/src-tauri/src/update_cmds.rs:219-229` | `version: String`(225-226)、`notes: String`(227-228)；**snake_case 真实形状**，不能复用 `UpdateInfo` 反序列化(219-222) |

函数与方法：

| 项目 | 行号 | 说明 |
| --- | --- | --- |
| `pub(crate) fn update_dir(app) -> Result<PathBuf, String>` | `android/src-tauri/src/update_cmds.rs:64-71` | `app_data_dir/files/update`，三处拼接约定集中于此 |
| `pub fn has_newer_version(current, latest) -> bool` | `android/src-tauri/src/update_cmds.rs:104-125` | 去 `v` 前缀、逐段数值比较、短版按 0 补齐；预发布后缀段解析为 0 |
| `pub(crate) fn http_client() -> Result<reqwest::Client, String>` | `android/src-tauri/src/update_cmds.rs:127-136` | `connect_timeout=5s` + 总超时 30s |
| `pub(crate) fn allowed_url(url) -> Result<(), String>` | `android/src-tauri/src/update_cmds.rs:138-159` | 白名单唯一漏斗（人脸模型下载复用，勿复制，138） |
| `#[tauri::command] pub async fn check_update(app)` | `android/src-tauri/src/update_cmds.rs:161-166` | 转 `check_and_record` |
| `fn now_epoch_ms()` / `last_check_error_snapshot()` / `last_check_time_snapshot()`（私有） | `android/src-tauri/src/update_cmds.rs:168-182` | 时间与快照读取(175-182) |
| `fn backoff_delay_secs(fail_count) -> u64`（私有） | `android/src-tauri/src/update_cmds.rs:184-193` | 0 → 24h；1..=3 → 5min/15min/1h；>3 → 24h |
| `async fn check_and_record(app) -> Result<UpdateInfo, String>`（私有） | `android/src-tauri/src/update_cmds.rs:195-209` | 执行检查后写 `LAST_CHECK_ERROR`/`LAST_CHECK_TIME_MS`，手动与自动路径共用(196-197) |
| `fn version_urls(mirror_first) -> Vec<&'static str>`（私有） | `android/src-tauri/src/update_cmds.rs:211-217` | 按渠道设置排序，另一侧留作降级 |
| `async fn fetch_apk_assets(client, latest) -> (Vec<ReleaseAsset>, Option<String>, bool)`（私有） | `android/src-tauri/src/update_cmds.rs:231-280` | GitHub API 取 `.apk` 资产与 `digest`（`sha256:` 前缀剥离，265-271）；返回 `api_ok` 区分「API 可用无 digest」与「API 不可用」(232-236) |
| `async fn check_update_inner(app) -> Result<UpdateInfo, String>`（私有） | `android/src-tauri/src/update_cmds.rs:282-333` | 4 源按序降级(290-331)；空 `version` 跳过该源(296-299)；`has_newer_version(env!("CARGO_PKG_VERSION"), latest)`(300-301)；三态记账(309-315) |
| `pub fn start_update_check_loop(app)` | `android/src-tauri/src/update_cmds.rs:335-382` | 5s 后首查，之后按退避；有新版 emit `update-available` + 系统通知(352-365)；退避用尽复位计数(374-378) |
| `#[tauri::command] pub fn get_mirror_urls(github_url) -> Result<Vec<Value>, String>` | `android/src-tauri/src/update_cmds.rs:384-392` | 生成 GitHub + 3 镜像候选 |
| `#[tauri::command] pub async fn download_update(app, url) -> Result<String, String>` | `android/src-tauri/src/update_cmds.rs:394-408` | 原子标志互斥(402-406)，内部函数保证所有提前 return 都复位 |
| `async fn download_update_inner(app, url) -> Result<String, String>`（私有） | `android/src-tauri/src/update_cmds.rs:410-502` | 白名单(411) → 流式写盘、超 500MB 先判后写并删文件(451-455) → 200ms 节流 emit `update-download-progress`(460-473) → 三态 SHA256 校验(476-500) |
| `fn download_tag(url) -> Option<String>`（私有） | `android/src-tauri/src/update_cmds.rs:504-510` | 从 `/releases/download/<tag>/` 取 tag（镜像仅前缀拼接） |
| `pub(crate) async fn verify_file_sha256(path, expected) -> Result<bool, String>` | `android/src-tauri/src/update_cmds.rs:512-544` | 非 64 位 hex 直接 `Err`(525-527)；缓冲区**必须堆分配**(531-534，否则 async future 撑爆 JavaBridge 线程栈) |
| `#[tauri::command] pub async fn install_update(app, file_path) -> Result<bool, String>` | `android/src-tauri/src/update_cmds.rs:546-570` | canonicalize 后必须位于更新目录内(556-561)；交插件 `install_apk` 走 FileProvider(562-564)；**不立刻删 APK**(565-568) |
| `pub async fn cleanup_stale_apks(app)` | `android/src-tauri/src/update_cmds.rs:572-610` | 下载进行中跳过(581-584)；只删扩展名 `.apk`(590-596) 且 mtime 超 24h(597-603) |

## Architecture

```text
┌────────────────────────── android/src-tauri/src/lib.rs::run (30-133) ─────────────────────────┐
│ Builder ► [mobile] 5 插件 + manage(AndroidState) ► setup: init_logger / run_startup_tasks     │
│           [host]   仅 manage(AndroidState)         ► invoke_handler(66-122, 56 条命令)       │
│           build() ► run(): ExitRequested ⇒ api.prevent_exit()(129-131)                       │
└──────────┬──────────────────────────────────────────────────────┬───────────────────────────┘
           │ setup 启动链                                          │ IPC 命令面
           ▼                                                      ▼
  monitor_loop::run_startup_tasks (448-529)              account_cmds (AccountItem)
           │                                                        │
           │ current_settings(488-507) / save_to                    │ config_io_lock
           ▼                                                        ▼
  config_state ──◄── CONFIG_IO_LOCK(17-24) ──► CryptoBridge(Keystore) ──► config.json
   Settings/迁移/掩码出口
           ▲
           │ monitor_tick_loop(643-702) → run_scheduled_actions(823-1037)
           │                          → run_check_once(1338-1574)
           ▼
  protocol_cmds (ensure_wifi_bound / run_login / do_logout)
           │                                   │
           ▼                                   ▼
  campus_login_lib::auth::protocol     campus_login_lib::auth::portal
  campus_login_lib::config::{schedule, night_switch, outbound_switch, scheduled_tick}
           │
  quality_cmds(19-134) ─► network::quality    update_cmds(335-610) ─► GitHub/镜像 + 插件装 APK
```

装配的四个决定性细节：

1. `AndroidState` 在 mobile 与 host 两条支路都 `manage`，因此命令体内的 `app.state::<AndroidState>()` 无需配对 `cfg` 门控（`android/src-tauri/src/lib.rs:40-42`）。
2. 移动插件整段被 `#[cfg(mobile)]` 包住（`android/src-tauri/src/lib.rs:33-40`），host 编译下这些能力退化为编译期空实现，调用点不用成对门控（`android/src-tauri/src/protocol_cmds.rs:281-283` 注释显式说明这个约定）。
3. 退出护栏把「最后窗口关闭 → 进程退出」拦下（`android/src-tauri/src/lib.rs:125-132`），安卓语义是「划掉任务只关 UI，进程随前台服务常驻」。
4. 日志初始化失败必须可见：`init_logger` 失败只 `eprintln!`，因为 `log_*` 宏全依赖它（`android/src-tauri/src/lib.rs:54-58`）。

## Data Flow

### 1. 启动恢复链

`lib.rs:45-64` 的 `setup` 先建日志目录并初始化 logger，再调 `run_startup_tasks`（`android/src-tauri/src/monitor_loop.rs:448-529`）：

1. 等 500ms 就绪窗口（`android/src-tauri/src/monitor_loop.rs:460`）。
2. 读配置。失败 → `log_error` + emit `startup-failed` + 尽力停 FGS，然后整体退出（`android/src-tauri/src/monitor_loop.rs:461-484`）。这一步是「开关开着却不运行、且无任何解释」问题的根因修复。
3. `set_log_retention_days(settings.log_retention_days)`（`android/src-tauri/src/monitor_loop.rs:488`）。
4. 三条互不依赖的链**并行 spawn**（2026-09-09 提速，注释在 453-457）：后台检测（489-494）、质量首测或定时测试（501-513）、启动自动登录（519-527）；同一函数里还串了 avoid_bad_wifi 快照兜底（498-500）、更新检查循环（515）、清理残留安装包（518）。

### 2. 巡检单拍 `run_check_once`

`android/src-tauri/src/monitor_loop.rs:1338-1574`，顺序即代码顺序：

1. `check_count += 1`（1340），读配置，失败即 emit error 并返回（1341-1347）。
2. **出站切换态闸**：`night_outbound_restore` 非空则整轮跳过（1349-1357）。闸放在 `run_check_once` 而不是 tick 循环，是因为周期拍、WiFi 事件、手动触发三条入口共用这一个闸；而 `run_scheduled_actions` 不经过此闸，晨间还原才能照常跑（1353-1354）。
3. **检测静默期闸**：早于开始时间或晚于终点则跳拍，并把常驻通知置为状态码 3「非检测时段」（1363-1383）。
4. 探针窗口 `ProbeWindowGuard`（持有 WifiLock/WakeLock，Drop 必释放）（1385-1392）。
5. `ensure_wifi_bound` 强制绑 WiFi（1394-1396）。
6. 校园网判定 + 缓存源 IP（1398-1411），SSID 感知版探针 `probe_campus_with_ssid`。
7. Portal 探测三态短路（1413-1428）：有源 IP 且在校园网才真探；非校园网、无源 IP 直接给原因，避免空转 3~8s。
8. **三态消费**：仅 `error_kind.is_none()`（确定判定）才翻转在线状态；在校园网但探针失配 → 保持 `prev_online`；不在校园网 → 直接 `false`（1429-1445）。这一段是「状态点每拍打灰又变绿」误报的根治。
9. 状态机：在线则清零 `reconnect_count`/`consecutive_failures`/注销保护（1447-1452）；`was_online.swap(online)`；从在线翻离线且在校园网 → 发掉线通知（1454-1457）。
10. 自动重登判定 `should`（1459-1479）叠加两道调用侧闸：注销保护期（1461）与连续失败熔断（`failures >= 5`，1462/1467）。放行且凭据非空时持 `LOGIN_ATTEMPT_LOCK`（1481-1483）记 `last_login_attempt_ms`、`reconnect_count += 1`（1484-1487），未达上限才真正 `run_login`（1490-1493），失败计入熔断并 `notify_system`（1507-1519）。
11. 组装 payload 并 `emit("background-check-result")`（1525-1560），字段对齐桌面同名事件的可适用子集；`onlineOperator` 由 Portal uid 后缀推导（1527-1534）。
12. 常驻通知仅在线状态翻转时才 `update_notification`（1562-1573），避免每拍重建的稳态功耗。

### 3. 配置写路径（`save_config`）

`android/src-tauri/src/config_state.rs:481-512`：持 `CONFIG_IO_LOCK` → `load_from`（含迁移与 SSID 扩三值）→ `resolve_password_field`（空串/`"***"` 回退已存值，`clear_password` 显式清除）→ `save_to` →（`save_file` 内：非空密码加密、`settings.password` 置空、pid 唯一 tmp、write+rename）→ 刷内存缓存 → `auto_create_account_for_current` → `emit_config_changed`。

监控侧的写路径是另一条同构链路 `persist_settings`（`android/src-tauri/src/monitor_loop.rs:1039-1065`）：落盘 + 刷缓存 + 自动建号 + 广播 `config-changed`，保证夜切改的 `operator/restore` 立刻推到前端，避免 UI 随后保存时用旧快照覆盖回去。

### 4. 更新下载与校验

`check_update_inner`（`android/src-tauri/src/update_cmds.rs:282-333`）先按渠道排序取 `version.json`（snake_case `version/notes`），再打 GitHub API 取 APK 资产与 `digest`，把 `(tag, digest)` 记进 `UPDATE_CHECKSUM`，其中 `(tag, "")` 是「API 在线但无 digest」的哨兵。`download_update_inner`（410-502）按下载 URL 的 tag 匹配记账：有 digest → 强制比对，不匹配即删文件报错；空 digest → 拒绝安装；无记账或 tag 不匹配 → 放行但 emit `update-unverified`（476-500）。这条三态设计是为了把「完整性未校验」从静默变成可见。

## Connections

- [[campus-detect]]：`probe_campus_with_ssid` / `cache_source_ip` / `campus_status_message` 由 `monitor_loop` 与 `protocol_cmds` 共同消费（`android/src-tauri/src/monitor_loop.rs:1526-1539`）。
- [[android-state]]：`AndroidState.config` 内存缓存与 `cached_source_ip`（`android/src-tauri/src/config_state.rs:448-451`、`android/src-tauri/src/protocol_cmds.rs:170-178`）；`WATCHER_FAILED` 由 `monitor_loop` 记账（`android/src-tauri/src/monitor_loop.rs:11-13`）。
- [[login-history]]：手动登录/注销与自动重登都落一条历史（`android/src-tauri/src/protocol_cmds.rs:46-52`、`android/src-tauri/src/monitor_loop.rs:1497-1502`）。
- [[quality-history]]：每次真实质量检测落一条（`android/src-tauri/src/quality_cmds.rs:42-47`）。
- [[system-cmds]]：日志面板/调试模式命令与 `get_init_data` 共用 `monitor_loop::status_value`（`android/src-tauri/src/monitor_loop.rs:130-152`）。
- [[self-service-cmds]] / [[identity-gate]]：自助服务六命令与验证门 TTL，共用同一份 `Settings` 与 `config_io_lock`（`android/src-tauri/src/account_cmds.rs:20-22`）。
- [[battery-cmds]] / [[face-model-cmds]]：电量优化引导命令与 2D 人脸模型下载（复用 `update_cmds::allowed_url` 与 `verify_file_sha256`，见 `android/src-tauri/src/update_cmds.rs:138`、`515-517` 的复用声明）。
- 共享协议核心（桌面 crate）：`campus_login_lib::auth::protocol`、`auth::portal`、`network::quality`、`config::{schedule, night_switch, outbound_switch, scheduled_tick}`、`infra::logger` —— 单点实现，禁止在安卓侧复制。
- 相关决策/经验文章：夜切语义（`android/src-tauri/src/monitor_loop.rs:1350` 提到的 `decisions/night-outbound-switch`）、保持活性架构（`android/src-tauri/src/quality_cmds.rs:87-88` 提到的 `decisions/android-keepalive-fgs-architecture`）。

## Known Issues

1. **`AccountItem.password_saved` 恒为 false（源码内自相矛盾）**：`read_account_card_info` 用 `settings.password` 是否非空判定（`android/src-tauri/src/account_cmds.rs:115`），但 `save_file` 落盘时**强制**把 `settings.password` 置为空串、真值只进 `password_cipher`（`android/src-tauri/src/config_state.rs:788-791`）。因此从磁盘读回的 `password` 永远为空，出站布尔恒 false；现有单测也只断言了 false（`android/src-tauri/src/account_cmds.rs:549-550`）。若前端据此显示「密码已保存」，应改为判 `passwordCipher` 非空。
2. **`Settings::default()` 与 v7 迁移的夜切默认值不一致**：新装默认 `enable_night_operator_switch: false`（`android/src-tauri/src/config_state.rs:141`），而 v6→v7 迁移把存量刷成 `true`（`android/src-tauri/src/config_state.rs:389-392`）。新装默认关、旧用户迁移后默认开，是刻意保留的历史取舍，但两处注释（139-140 与 338-339）对「存量是否迁移」的说法需要核对统一。
3. **`night_outbound_restore` 用魔法字符串做状态位**：值恒为 `"logged_out"`（`android/src-tauri/src/config_state.rs:60-64`），切换态判定全靠「非空」（`android/src-tauri/src/monitor_loop.rs:704-711`）。任何误写该字段的路径都会静默压住整轮巡检。
4. **桌面专属字段在安卓侧只镜像不消费**：`outbound_priority` / `outbound_metric_restore` / `outbound_disabled_adapters` / `outbound_standby_route` / `dns_optimize_adapters`（`android/src-tauri/src/config_state.rs:48-67`）。安卓消费的是 `night_outbound_restore` + 系统 `network_avoid_bad_wifi`，两者状态机不互通，跨端同步配置时要意识到这一点。
5. **host 编译下多处能力被编译期禁用**（设计边界，非运行期开关）：`CryptoBridge::from_app` 返回恒 Err 的桥（`android/src-tauri/src/config_state.rs:231-239`）；`bind_to_wifi`/`accept_wifi_network`/`get_avoid_bad_wifi_status`/`restore_written_settings` 恒 Err（`android/src-tauri/src/protocol_cmds.rs:220-224, 209-213, 230-234, 250-254`）；`power_state` 恒 `(true, true)`（`android/src-tauri/src/monitor_loop.rs:877-880`）；`get_boot_autostart` 恒 `Ok(false)`（`android/src-tauri/src/monitor_loop.rs:877-881`）；`start_wifi_watcher` 等整段 `#[cfg(mobile)]`（`android/src-tauri/src/monitor_loop.rs:877-886`）。据此，host 上 `cargo test` 只覆盖纯函数与文件 IO，移动路径的行为只能在真机/交叉编译目标上验证。
6. **仍有 `#[allow(dead_code)]` 的未接线标注**：`current_settings`（`android/src-tauri/src/config_state.rs:490`，注释「Task 2 协议命令面接线」）、`MonitorState`（`android/src-tauri/src/monitor_loop.rs:40`）、`should_attempt_login`（`android/src-tauri/src/monitor_loop.rs:106`）、`is_running`（`android/src-tauri/src/monitor_loop.rs:154`，注释「Task 4 循环体接线」）。注意这几个符号在当前源码里实际已被 `run_check_once`/`monitor_tick_loop` 调用，标注是历史遗留，清理时别误删。
7. **`delete_account` 不清 `night_operator_restore`**：切账号时清了运营商夜切的 restore 暂存（`android/src-tauri/src/account_cmds.rs:207`），删除当前账号时只清了 `active_account` 与 `night_outbound_restore`（`android/src-tauri/src/account_cmds.rs:330-338`），运营商 restore 残留会在下一次夜切恢复窗口里被写回已删除账号的 ISP 后缀。
8. **`run_login` 的 `operator` 回退时机**：`do_login` 在参数空值检查**之后**才回退 `operator`（`android/src-tauri/src/protocol_cmds.rs:38-42`），因此「入参空 + 配置也空」是合法路径（校方默认线路），但日志里的 `operator_len` 记录的是回退前长度（`android/src-tauri/src/protocol_cmds.rs:37-38`），排障时会看到 0 而实际用了配置值。
9. **`MAX_DOWNLOAD_BYTES` 与 `content_length` 无联动**：响应没有 `Content-Length` 时 `percent` 恒为 0（`android/src-tauri/src/update_cmds.rs:468`），进度条停在 0% 直到结束；大小上限只靠逐 chunk 判定兜底（451-455）。
10. **夜切验证时序硬编码不可配**：`NIGHT_VERIFY_*`（`android/src-tauri/src/monitor_loop.rs:1194-1199`）写死，弱网环境下「未生效 → 注销重登 → 复验」整轮最长可达数十秒，期间与巡检、定时动作并发。