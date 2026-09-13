---
title: 桌面端配置（模型/持久化/校验）
type: module
source_files:
  - tauri-app/src-tauri/src/config/mod.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/persist.rs
  - tauri-app/src-tauri/src/config/validate.rs
tags: [配置, 持久化, 原子写, DPAPI, 校验, 迁移, 跨平台]
---

## Overview

本模块定义应用的全量配置数据结构 `Config`（44 个字段，camelCase IPC 契约）、磁盘持久化（原子写 + 密码 DPAPI 加密 + 登录历史追加）、以及两条校验通道：严格版 `validate_config`（保存/导入入口，非法即拒绝）与宽松版 `validate_config_lenient`（加载入口，逐字段降级到默认值）。配置同时承载"内存明文、磁盘密文"的敏感字段（`password`、`self_password`）与一套配置版本迁移逻辑（`config_version < 2` 时把小时值折算为分钟值）。

本模块在 `tauri-app/src-tauri/src/lib.rs:4` 被声明为跨平台模块（安卓端经 path 依赖可见），但安卓端另有自己的配置结构 `android/src-tauri/src/config_state.rs`（注释声明"结构对齐桌面 config::Config 可适用子集"），并不复用 `persist.rs` 的落盘路径。

## Key Components

### config/mod.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `config/mod.rs:1` | pub | `mod model` | 配置模型与默认值 |
| `config/mod.rs:2` | pub | `mod persist` | 路径解析、原子写、加密落盘、登录历史 |
| `config/mod.rs:3` | pub | `mod validate` | 严格/宽松校验与字段迁移 |
| `config/mod.rs:5` | pub use | `Config` | 对外单点导出 `model::Config` |

### config/model.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `model.rs:3` | pub const | `PASSWORD_MASK: &str = "***"` | 出站掩码占位符；前端回传该值表示"未修改密码" |
| `model.rs:4` | pub const | `AUTO_DETECT_ADAPTER: &str = "自动检测"` | 适配器自动选择的哨兵值（`network::adapter` 与 `monitor::adapter_watch` 都用它做比较） |
| `model.rs:6-104` | pub struct | `Config` | 全量配置（44 字段，见下表）；容器级 `#[serde(default)]` |
| `model.rs:106-116` | 私有 fn | `deserialize_non_empty_or<D>(deserializer, default_fn) -> Result<String, D::Error>` | 反序列化时把空串替换为指定默认值 |
| `model.rs:118-123` | 私有 fn | `deserialize_campus_gateway<D>` | 组合上者，空串 → `default_campus_gateway()` |
| `model.rs:125-130` | 私有 fn | `deserialize_required_network_name<D>` | 组合上者，空串 → `default_required_network_name()` |
| `model.rs:132` | 私有 fn | `default_true() -> bool` | 返回 `true` |
| `model.rs:134` | 私有 fn | `default_campus_check_start_minutes() -> u16` | 460（07:40） |
| `model.rs:136` | 私有 fn | `default_campus_exit_start_minutes() -> u16` | 480（08:00） |
| `model.rs:138` | 私有 fn | `default_campus_exit_end_minutes() -> u16` | 1380（23:00） |
| `model.rs:140-142` | pub fn | `default_fixed_gateway() -> String` | `"10.2.127.254"` |
| `model.rs:144` | pub fn | `default_log_retention_days() -> u32` | 7 |
| `model.rs:146` | 私有 fn | `default_max_disconnect_reconnect() -> u32` | 3 |
| `model.rs:148` | 私有 fn | `default_auto_login_cooldown_secs() -> u64` | 60 |
| `model.rs:150-152` | pub fn | `default_portal_url() -> String` | `"http://10.1.99.100"`（`network::client::PORTAL_URL` 的初值也取它） |
| `model.rs:154-156` | pub fn | `default_required_network_name() -> String` | `"i-wxxy"` |
| `model.rs:158-160` | pub fn | `default_campus_gateway() -> String` | `"10.2.127.254"`（`commands/network_cmd.rs:105` 用作兜底） |
| `model.rs:162-211` | impl | `Default for Config` | 44 个字段的默认值（见表） |
| `model.rs:217-221` | pub fn | `Config::masked_for_display(&self) -> Config` | 克隆后掩码，原 struct 不变；**所有把 Config 发往前端的路径必须经此方法** |
| `model.rs:223-230` | pub fn | `Config::mask_in_place(&mut self)` | 就地掩码：`password`/`self_password` 非空则置为 `PASSWORD_MASK` |

本模块未定义任何 `trait`（只实现对 `Default` 的 `impl`）。

### config/persist.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `persist.rs:10` | 私有 static | `LOGIN_HISTORY_LOCK: Mutex<()>` | 登录历史读-改-写全程互斥（防并发追加互相覆盖） |
| `persist.rs:12-43` | pub fn | `atomic_write(path: &Path, content: &str) -> Result<(), String>` | 原子写：临时文件（纳秒时间戳后缀）→ BufWriter + flush → `sync_all` → `rename` 重试 3 次（间隔 100ms），失败清理临时文件并返回 Err |
| `persist.rs:45-58` | pub fn | `get_data_dir(app_handle) -> PathBuf` | Tauri `app_data_dir()`；失败回退 `dirs::data_dir()/campus-login`，再回退 `"."`；不存在则 `create_dir_all` |
| `persist.rs:60-62` | pub fn | `get_config_path(data_dir) -> PathBuf` | `{data_dir}/config.json` |
| `persist.rs:64-66` | pub fn | `get_accounts_dir(data_dir) -> PathBuf` | `{data_dir}/accounts` |
| `persist.rs:68-93` | pub fn | `list_account_names(app_handle) -> Vec<String>` | 列 `accounts/*.json` 的文件名词干（过滤 `.` 前缀与空名），排序返回 |
| `persist.rs:95-97` | pub fn | `get_login_history_path(data_dir) -> PathBuf` | `{data_dir}/login-history.json` |
| `persist.rs:99-151` | pub fn | `append_login_history(app_handle, success: bool, message: &str, adapter: &str, user: &str, login_type: &str) -> Result<(), String>` | 加锁 → 读现有历史（读/解析失败 rename 为 `.bak` 后重置）→ 头部插入新记录 → 截断至 100 条 → `atomic_write` |
| `persist.rs:153-168` | pub fn | `save_config_to_disk_encrypted(data_dir: &Path, config: &Config) -> Result<(), String>` | 克隆配置 → 非空 `password`/`self_password` 逐一 `crypto::encrypt` → `serde_json::to_string_pretty` → `atomic_write`；**不做任何校验** |

### config/validate.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `validate.rs:6` | 私有 static | `USERNAME_RE = ^[a-zA-Z0-9._-]+$` | 用户名字符白名单 |
| `validate.rs:7` | 私有 static | `CUSTOM_COLOR_RE = ^#[0-9a-fA-F]{6}$` | 自定义主题色格式 |
| `validate.rs:10-21` | pub fn | `validate_username(user: &str) -> Result<&str, String>` | 非空 + ≤64 字节 + 字符白名单 |
| `validate.rs:23-29` | pub fn | `validate_operator(op: &str) -> Result<&str, String>` | 仅允许 `""`/`"@telecom"`/`"@unicom"`/`"@cmcc"` |
| `validate.rs:31-39` | pub fn | `validate_password(password: &str) -> Result<(), String>` | 非空 + ≤128 字节 |
| `validate.rs:42-71` | 私有 fn | `validate_portal_url(url: &str) -> Result<(), String>` | 协议限 http/https；host 必须是内网 IPv4 / 回环 IPv6 / 字面量 `localhost`；**域名一律拒绝** |
| `validate.rs:73-79` | 私有 fn | `migrate_operator(op: &mut String)` | `@ctcc`→`@telecom`、`@cucc`→`@unicom` |
| `validate.rs:81-85` | 私有 fn | `normalize_portal_url(url: &mut String)` | 空串或字面值 `"http://10.1.99.100:801"` → `"http://10.1.99.100"` |
| `validate.rs:87-138` | pub fn | `validate_config(config: Config) -> Result<Config, String>` | 严格校验 + 迁移 + clamp；任一项非法即 `Err` |
| `validate.rs:143-207` | pub fn | `validate_config_lenient(mut config: Config) -> Config` | 逐字段降级：无效字段回退 `Config::default()` 对应值并 `log_warn!`；末尾再跑一次严格版，仍失败则整份返回 `Config::default()` |

## 结构体与字段

### `Config`（`model.rs:6-104`）

`#[derive(Debug, Clone, Serialize, Deserialize)]`（`model.rs:6`）+ 容器级 `#[serde(default)]`（`model.rs:9`）。JSON 键名无统一 `rename_all`，未标 `rename` 的字段保持 Rust 原名（`user`、`password`、`operator`、`adapter1`、`adapter2`），其余均为显式 camelCase。

| 位置 | Rust 字段 | JSON 键 | 类型 | 默认值 | 含义 |
| --- | --- | --- | --- | --- | --- |
| `model.rs:11` | `user` | `user` | `String` | `""` | 校园网登录账号（学号） |
| `model.rs:13` | `password` | `password` | `String` | `""` | 登录密码；内存明文 / 磁盘 DPAPI 密文 / 出站 MASK |
| `model.rs:16` | `self_password` | `selfPassword` | `String` | `""` | 自助服务系统登录密码（同上三态） |
| `model.rs:21` | `self_hello_enabled` | `selfHelloEnabled` | `bool` | `true`（`default_true`） | 是否启用 Windows Hello 操作验证门；关闭后"查看运营商账号密码"仍强制验证（后端 TTL 独立） |
| `model.rs:24` | `self_reverify_each_action` | `selfReverifyEachAction` | `bool` | `false` | 自助服务面板是否每次操作都二次验证（否则切入面板验证一次后共用）；**仅前端消费** |
| `model.rs:25` | `operator` | `operator` | `String` | `""` | 运营商后缀：`""`/`@telecom`/`@unicom`/`@cmcc` |
| `model.rs:26` | `adapter1` | `adapter1` | `String` | `"自动检测"` | 主适配器名或自动检测哨兵 |
| `model.rs:27` | `adapter2` | `adapter2` | `String` | `""` | 副适配器名（双适配器模式使用） |
| `model.rs:29` | `dual_adapter` | `dualAdapter` | `bool` | `false` | 是否启用双适配器同时登录 |
| `model.rs:31` | `auto_login_on_start` | `autoLoginOnStart` | `bool` | `true` | 启动时自动登录 |
| `model.rs:33` | `auto_exit_after_login` | `autoExitAfterLogin` | `bool` | `true` | 登录成功后自动退出程序（由 `auth::service::post_login_handler` 消费） |
| `model.rs:35` | `minimize_to_tray` | `minimizeToTray` | `bool` | `false` | 关闭窗口时最小化到托盘 |
| `model.rs:37` | `hidden_start` | `hiddenStart` | `bool` | `false` | 启动时不显示主窗口 |
| `model.rs:39` | `auto_launch` | `autoLaunch` | `bool` | `true` | 开机自启 |
| `model.rs:41` | `enable_background_check` | `enableBackgroundCheck` | `bool` | `true` | 是否启用后台巡检（关闭后登录成功不再触发后台检查） |
| `model.rs:43` | `background_check_interval` | `backgroundCheckInterval` | `u64` | `15000` | 后台巡检间隔（毫秒），clamp 到 `[10000, 3600000]` |
| `model.rs:45` | `auto_login_on_preparation` | `autoLoginOnPreparation` | `bool` | `true` | "登录准备"模式：检测到未登录且可登录时自动登录（`monitor/auto_auth.rs:35,125`） |
| `model.rs:47` | `auto_exit_on_online` | `autoExitOnOnline` | `bool` | `true` | 检测到在线后自动退出（`monitor/background_emit.rs:191`） |
| `model.rs:49` | `theme_mode` | `themeMode` | `String` | `"dark"` | 主题：`"dark"`/`"light"`/`"system"` |
| `model.rs:51` | `enable_notification` | `enableNotification` | `bool` | `true` | 是否发系统通知 |
| `model.rs:53` | `active_account` | `activeAccount` | `String` | `""` | 当前生效的账号档案名（`accounts/*.json` 的文件名词干） |
| `model.rs:55` | `enable_latency_test` | `enableLatencyTest` | `bool` | `true` | 是否启用延迟测试 |
| `model.rs:57` | `latency_test_interval` | `latencyTestInterval` | `u64` | `60000` | 延迟测试间隔（毫秒），clamp 到 `[10000, 3600000]` |
| `model.rs:59` | `custom_theme_color` | `customThemeColor` | `String` | `"#6366f1"` | 自定义主题色（`#RRGGBB`） |
| `model.rs:61` | `default_panel` | `defaultPanel` | `String` | `""` | 启动时默认显示的面板；**仅前端消费，生产代码 Rust 侧不消费**（`grep -rn "default_panel" src/` 命中 `model.rs:61` 字段声明与 `model.rs:189` 的 `impl Default` 之外，另命中 `network/adapter.rs:255`——位于 `#[cfg(test)] mod tests` 的 `make_test_config` 夹具内，非生产路径；安卓侧同名独立字段见 `android/src-tauri/src/config_state.rs:42/97`） |
| `model.rs:63` | `enable_network_quality` | `enableNetworkQuality` | `bool` | `true` | 是否启用网络质量检测（`monitor/watcher.rs:32`） |
| `model.rs:65` | `skip_ttfb_in_latency` | `skipTtfbInLatency` | `bool` | `true` | 延迟测试跳过 TTFB 指标 |
| `model.rs:67` | `skip_content_in_latency` | `skipContentInLatency` | `bool` | `true` | 延迟测试跳过内容下载指标 |
| `model.rs:69` | `portal_url` | `portalUrl` | `String` | `"http://10.1.99.100"` | Portal 基地址；保存时同步到全局 `network::client::PORTAL_URL` |
| `model.rs:71` | `fixed_gateway` | `fixedGateway` | `String` | `"10.2.127.254"` | 固定网关地址（延迟/质量测试目标，`network::quality`） |
| `model.rs:73` | `required_network_name` | `requiredNetworkName` | `String` | `"i-wxxy"` | 校园 WiFi SSID；空串反序列化回默认 |
| `model.rs:75` | `enable_network_name_check` | `enableNetworkNameCheck` | `bool` | `true` | 是否启用 SSID 判定（`monitor/background_check.rs:78`、`auto_auth.rs:286`） |
| `model.rs:77` | `campus_gateway` | `campusGateway` | `String` | `"10.2.127.254"` | 校园网关（MAC 重置 / 可达性判定的目标）；空串反序列化回默认 |
| `model.rs:80` | `update_source` | `updateSource` | `String` | `"mirror"` | 更新渠道优先级：`"mirror"`=镜像优先 / `"github"`=官方优先 |
| `model.rs:82` | `campus_exit_on_fail` | `campusExitOnFail` | `bool` | `true` | 非校园网时是否自动退出（`infra/lifecycle.rs:25`） |
| `model.rs:85` | `campus_exit_start_minutes` | `campusExitStartMinutes` | `u16` | `480`（08:00） | 非校园网自动退出生效时段起点（当日分钟数） |
| `model.rs:88` | `campus_exit_end_minutes` | `campusExitEndMinutes` | `u16` | `1380`（23:00） | 生效时段终点（不含该时刻；结束 ≤ 起点时退化为仅受起点限制） |
| `model.rs:90` | `campus_check_start_minutes` | `campusCheckStartMinutes`（alias `campusCheckStartHour`） | `u16` | `460`（07:40） | 校园网检测时段起点 |
| `model.rs:93` | `campus_check_end_minutes` | `campusCheckEndMinutes` | `u16` | `0` | 检测时段终点；`0`=不限制；≤ 起点时退化为仅起点限制 |
| `model.rs:95` | `log_retention_days` | `logRetentionDays` | `u32` | `7` | 日志保留天数；`0`=永久保留；上限 365 |
| `model.rs:97` | `max_disconnect_reconnect` | `maxDisconnectReconnect` | `u32` | `3` | 断线重连次数上限（`monitor/auto_auth.rs:155`） |
| `model.rs:99` | `auto_login_cooldown_secs` | `autoLoginCooldownSecs` | `u64` | `60` | 自动登录冷却秒数（`monitor/auto_auth.rs:63`） |
| `model.rs:101` | `skip_sha256_when_missing` | `skipSha256WhenMissing` | `bool` | `false` | 更新包缺少 sha256 时是否跳过校验（`commands/updater.rs:225`） |
| `model.rs:103` | `config_version` | `configVersion` | `u32` | `2` | 配置版本号，驱动 `campus_check_start_minutes` 小时→分钟迁移 |

### `Config` 的 serde 属性要点

| 位置 | 属性 | 作用 |
| --- | --- | --- |
| `model.rs:9` | `#[serde(default)]`（容器级） | 任一字段缺失即用 `Default` 补齐；否则单字段缺失会导致整份配置反序列化失败 |
| `model.rs:12` | `password` 上的 `#[serde(default)]` | 与容器级重复，冗余但无害 |
| `model.rs:20` | `default = "default_true"` | `self_hello_enabled` 默认 `true` |
| `model.rs:64`、`model.rs:66` | `default = "default_true"` | 两个 skip 开关默认 `true` |
| `model.rs:72`、`model.rs:76` | `deserialize_with = "deserialize_...` | 空串替换为 `"i-wxxy"` / `"10.2.127.254"` |
| `model.rs:89` | `alias = "campusCheckStartHour"` | 兼容旧版字段名（配合 `config_version < 2` 的小时→分钟迁移） |

## Data Flow

### 加载（启动 / 账号切换 / 配置导入）

```
app/startup.rs:162（安卓端另有自实现 config_state.rs）
  → commands::config_cmd::load_config_from_disk_or_default(app_handle)
      ├─ load_config_from_file: get_data_dir → get_config_path → 读文件
      │     ├─ 文件不存在 → Config::default()
      │     ├─ serde_json::from_str::<Config>（容器级 default 补齐缺失字段）
      │     ├─ password 非空且 != PASSWORD_MASK → account::crypto::decrypt
      │     │     失败 → 仅清空 password，保留其余配置                     (config_cmd.rs:31-40)
      │     └─ self_password 同上                                        (config_cmd.rs:42-50)
      └─ 成功 → config::validate::validate_config_lenient(config)
            逐字段降级 → 末尾 validate_config 兜底 → 仍失败则 Config::default()
      ← 失败（解析/读取异常）→ 原文件复制为 config.json.corrupt-{epoch}.bak + Config::default()
  → state.config.store(config)（app/startup.rs:177）
  → network::update_portal_url(&config.portal_url)（app/startup.rs:178）
  → infra::logger::set_log_retention_days(config.log_retention_days)（app/startup.rs:184）
```

### 保存

```
前端 save_config IPC / commands::{account,background,network_cmd,system,config_cmd}.rs
  → commands::config_cmd::save_config
      ├─ validate_config(config)  非法 → CommandResult::err，不落盘         (config_cmd.rs:99-105)
      ├─ clear_password / MASK 占位符处理：保留 state.config 中的旧密码      (config_cmd.rs:109-123)
      ├─ network::update_portal_url(&config.portal_url)                    (config_cmd.rs:127)
      ├─ infra::logger::set_log_retention_days(config.log_retention_days)  (config_cmd.rs:130)
      ├─ save_config_to_disk_encrypted(app_handle, &config)
      │     ├─ 非空 password/self_password → account::crypto::encrypt（DPAPI → base64）
      │     ├─ serde_json::to_string_pretty
      │     └─ persist::atomic_write：临时文件 → sync_all → rename（3 次重试）
      ├─ state.config.store(config.clone())   先落盘再更新内存               (config_cmd.rs:134-135)
      └─ notify_config_changed(config.masked_for_display())  掩码后发事件    (config_cmd.rs:15-16)
```

### 登录历史追加

```
auth::session::adapter_action_with_log / monitor::auto_auth.rs:187
  → config::persist::append_login_history(app_handle, success, message, adapter, user, type)
      LOGIN_HISTORY_LOCK → 读 login-history.json（坏文件 → *.bak 重置）
      → history.insert(0, {time, success, message, adapter, user, type})
      → 超过 100 条则 truncate(100) → atomic_write
```

### 出站掩码（唯一出口）

```
任意把 Config 发往前端的路径
  → Config::masked_for_display()
       password 非空 → "***"；self_password 非空 → "***"；空值保留（"未设置"语义）
使用点：commands/config_cmd.rs:15,88、commands/account.rs:47,188,227、
        commands/system.rs:117、commands/config_cmd.rs 回归测试 :149-164
```

## Connections

- [[desktop-commands]] — `commands/config_cmd.rs`（`save_config` / `get_config` / `load_config_from_disk_or_default` / `save_config_to_disk_encrypted`）、`commands/account.rs`（账号档案读写 + `crypto::encrypt/decrypt` + `masked_for_display`）、`commands/background.rs`、`commands/network_cmd.rs`、`commands/system.rs`、`commands/updater.rs`（`skip_sha256_when_missing`）。
- [[desktop-auth]] — `config::validate::{validate_username, validate_operator, validate_password}` 在 `auth/protocol.rs:99-101,242` 被复用为协议层校验；`config::persist::append_login_history` 被 `auth/session.rs:41,51` 调用。
- [[desktop-monitor]] — `enable_background_check` / `background_check_interval` / `auto_login_on_preparation` / `auto_exit_on_online` / `enable_network_name_check` / `required_network_name` / `max_disconnect_reconnect` / `auto_login_cooldown_secs` / `enable_network_quality` 的全部消费点。
- [[desktop-network-core]] — `network::client::PORTAL_URL`（`client.rs:8` 用 `default_portal_url()` 初始化）、`network::update_portal_url`、`network::adapter` 中的 `AUTO_DETECT_ADAPTER` 比较、`network::quality` 消费 `fixed_gateway`/`skip_*_in_latency`。
- [[desktop-infra]] — `infra::state::store`（`state.config.load()/load_full()/update()`）、`infra::logger`（`set_log_retention_days`、坏配置 `log_warn!`）、`infra::lifecycle`（`campus_exit_on_fail` / `campus_exit_*_minutes`）。
- [[desktop-account-selfservice]] — `config/persist.rs:6` 依赖 `account::crypto`；`self_password` / `self_hello_enabled` / `self_reverify_each_action` 是自助服务面板的配置面。

## Known Issues

1. **`self_password` 完全没有校验**：`validate.rs` 全文无 `self_password` 出现（`grep -rn "self_password" src/config/validate.rs` 零命中），既无长度上限也无格式检查；`validate_config` 只在 `validate.rs:92-94` 校验 `password`。
2. **`update_source` 无取值校验**：`model.rs:78-80` 只声明 `default`，`validate.rs` 不校验其取值；消费点 `commands/updater.rs:10` 与 `update/updater.rs:282` 用 `!= "github"` 判定，任何拼写错误（如 `"Github"`）都会静默走镜像优先。
3. **`normalize_portal_url` 只认一个字面值**：`validate.rs:81-85` 硬比较 `"http://10.1.99.100:801"` 与空串。用户填其他带 `:801` 的地址（例如校园网改版后的 EPortal 地址）不会被归一化，会与 `auth/portal.rs:206-213` 记录的"页面探测不能探 `:801`"约束冲突。
4. **容器级 `serde(default)` 让字段名错误静默降级**：`model.rs:9` 无 `deny_unknown_fields`，前端若传 `"portalURL"` 这类拼错键，反序列化不报错、值静默丢成默认值。同因，`Config` 的 JSON 键名靠 43 处手写 `rename` 维护（仅 `user`/`password`/`operator`/`adapter1`/`adapter2` 用原名），新增字段漏加 `rename` 无编译期保护。
5. **`validate_config` 对 `adapter1`/`adapter2`/`active_account`/`default_panel` 均无校验**：`validate.rs:87-138` 的检查清单只覆盖 user/password/operator/颜色/主题/两个 interval/portal_url/两个 gateway/required_network_name/log_retention_days/四个分钟字段；账号档案名另有 `infra::state::validate_account_name` 把关（见 `commands/account.rs:16,53,195`），但配置内的 `active_account` 本身不受校验。
6. **`log_retention_days` 只限上限不限下限**：`validate.rs:120-122` 仅 `> 365` 时截到 365；`0` 有"永久保留"语义（注释指向 `logger.rs cleanup_old_logs_by_time`），但无下界保护。
7. **宽松校验的最终兜底会丢全部配置**：`validate.rs:200-206`，降级后仍失败时直接返回 `Config::default()`（仅 `log_warn!` 告警，用户无感知）。这是"逐字段降级"设计之外的整份回退路径。
8. **`validate_config_lenient` 不做 interval clamp 之外的数值归一**：`validate.rs:143-207` 未处理 `background_check_interval` / `latency_test_interval` / `log_retention_days`，依赖末尾的 `validate_config` 兜底完成 clamp（`validate.rs:103-104`、`validate.rs:120-122`）。
9. **`atomic_write` 的临时文件名会替换原扩展名**：`persist.rs:14-20` 用 `path.with_extension("json.tmp.{nanos}")`，对 `config.json` 得到 `config.json.tmp.123`；若传入无扩展名的路径，语义会变成"替换整个文件名"。另 `rename` 仅重试 3×100ms（`persist.rs:32-39`），Windows 上被索引/杀软占用时失败即删除临时文件并返回 Err（原文件保持完整）。
10. **`save_config_to_disk_encrypted` 不做校验且会对 MASK 加密**：`persist.rs:153-168` 直接落盘传入的 `Config`；`persist.rs:159-164` 明确不排除 `PASSWORD_MASK`（注释 `persist.rs:155-158` 说明：若真实密码恰为 `"***"`，排除判断会让它明文落盘）。代价是：任何把 MASK 占位符直接落盘的调用方，重启后会得到明文密码 `"***"`（当前调用方 `commands/config_cmd.rs:107-123` 已在落盘前还原真值，属依赖调用方正确性）。
11. **登录历史锁只覆盖 `append_login_history`**：`persist.rs:10` 的 `LOGIN_HISTORY_LOCK` 仅在 `persist.rs:100` 加锁；`commands/system.rs` 等若存在其他读写 `login-history.json` 的路径，将不受保护。另历史为"全量读 → 写"（`persist.rs:105-148`），每次追加都是 O(100) 次序列化。
12. **登录历史读取失败/解析失败只做 `.bak` 改名后重置**：`persist.rs:110`、`persist.rs:121` 的备份路径用 `format!("{}.bak", ...)`（固定名，第二次失败会覆盖上一次备份），且失败被静默忽略（`let _ =`）。
13. **配置迁移只覆盖 `config_version < 2` 的单向场景**：`validate.rs:125-132`，条件 `campus_check_start_minutes > 0 && < 24` 才视为小时值；若旧版用户恰好把该值设为 24 以上（理论上不该发生），会跳过迁移并被 `validate.rs:133` 截到 1439。迁移完成后无条件写 `config_version = 2`。
14. **`validate_portal_url` 拒绝域名**：`validate.rs:63-65`，host 既非 IP 也非字面量 `localhost` 即报"仅允许IP地址，不支持域名"。校园网改用域名 Portal 时需改代码。
15. **`validate_portal_url` 的 IPv6 分支只允许回环**：`validate.rs:57-61`，非 `::1` 的 IPv6 一律拒绝，错误文案却写"仅允许内网IPv4或localhost"。
16. **`campus_check_*` / `campus_exit_*` 只做 `min(1439)` 钳制**：`validate.rs:133-136`，无"起点晚于终点"的纠正（语义上允许，退化为仅受起点限制，见 `model.rs:83-93` 注释），但 UI 与日志可能展示出反直觉的时段。
17. **`fixed_gateway` 允许空串**：`validate.rs:107-109` 仅在非空时校验 IP，空值语义（是否启用固定网关）没有显式开关，由消费方 `network::quality` 自行解释。
