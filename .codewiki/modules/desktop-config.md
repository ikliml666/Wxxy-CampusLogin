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

本模块定义应用的全量配置数据结构 `Config`（70+ 字段，camelCase IPC 契约）、磁盘持久化（原子写 + 密码 DPAPI 加密 + 登录历史追加 + 质量历史追加 + 账号档案读写）、以及两条校验通道：严格版 `validate_config`（保存/导入入口，非法即拒绝）与宽松版 `validate_config_lenient`（加载入口，逐字段降级到默认值）。配置同时承载"内存明文、磁盘密文"的唯一敏感字段（`password`，凭据统一后为唯一密码字段）与一套配置版本迁移逻辑（`config_version < 2` 时把小时值折算为分钟值；`< 3` 时把 `campus_check_end_minutes` 旧默认 0 刷为 1380；`< 4` 刷新后台检测与延迟测试间隔默认值；`< 5` 处理夜切开关默认值变化与定时动作禁用哨兵更新）。

本模块在 `mod.rs:1-7` 声明子模块，并于 `mod.rs:9` 对外单点导出 `model::Config`。

## Key Components

### config/mod.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `mod.rs:1-7` | pub mod | `model`, `night_switch`, `outbound_switch`, `persist`, `schedule`, `scheduled_tick`, `validate` | 子模块声明 |
| `mod.rs:9` | pub use | `model::Config` | 对外单点导出 `model::Config` |

### config/model.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `model.rs:3` | pub const | `PASSWORD_MASK: &str = "***"` | 出站掩码占位符；前端回传该值表示"未修改密码" |
| `model.rs:4` | pub const | `AUTO_DETECT_ADAPTER: &str = "自动检测"` | 适配器自动选择的哨兵值 |
| `model.rs:6` | derive | `#[derive(Debug, Clone, Serialize, Deserialize)]` | 派生序列化/反序列化 |
| `model.rs:9` | 属性 | `#[serde(default)]`（容器级） | 任一字段缺失（旧版/手工编辑）都用 `Default` 补齐，否则整份反序列化失败 |
| `model.rs:10-182` | pub struct | `Config` | 全量配置（70+ 字段，见下表） |
| `model.rs:184-194` | 私有 fn | `deserialize_non_empty_or<D>(deserializer, default_fn) -> Result<String, D::Error>` | 反序列化时把空串替换为指定默认值 |
| `model.rs:196-201` | 私有 fn | `deserialize_campus_gateway<D>` | 组合上者，空串 → `default_campus_gateway()` |
| `model.rs:203-208` | 私有 fn | `deserialize_required_network_name<D>` | 组合上者，空串 → `default_required_network_name()` |
| `model.rs:210` | 私有 fn | `default_true() -> bool` | 返回 `true` |
| `model.rs:212` | 私有 fn | `default_campus_check_start_minutes() -> u16` | 460（07:40） |
| `model.rs:214-216` | 私有 fn | `default_campus_check_end_minutes() -> u16` | 1380（23:00，2026-09-13 起旧默认 0，由 v2→v3 迁移刷新） |
| `model.rs:218` | 私有 fn | `default_campus_exit_start_minutes() -> u16` | 480（08:00） |
| `model.rs:220` | 私有 fn | `default_campus_exit_end_minutes() -> u16` | 1380（23:00） |
| `model.rs:222-224` | pub fn | `default_fixed_gateway() -> String` | `"10.2.127.254"` |
| `model.rs:226` | pub fn | `default_log_retention_days() -> u32` | 7 |
| `model.rs:228` | 私有 fn | `default_max_disconnect_reconnect() -> u32` | 3 |
| `model.rs:230` | 私有 fn | `default_auto_login_cooldown_secs() -> u64` | 60 |
| `model.rs:232-234` | pub fn | `default_portal_url() -> String` | `"http://10.1.99.100"` |
| `model.rs:236-238` | pub fn | `default_required_network_name() -> String` | `"i-wxxy、iwxxy-2、iwxxy-3"` |
| `model.rs:240-242` | pub fn | `default_campus_gateway() -> String` | `"10.2.127.254"` |
| `model.rs:244-320` | impl | `Default for Config` | 70+ 个字段的默认值（见表） |
| `model.rs:322-330` | pub fn | `Config::masked_for_display(&self) -> Config` | 克隆后掩码，原 struct 不变；**所有把 Config 发往前端的路径必须经此方法** |
| `model.rs:332-336` | pub fn | `Config::mask_in_place(&mut self)` | 就地掩码：`password` 非空则置为 `PASSWORD_MASK` |

### config/persist.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `persist.rs:11` | 私有 static | `LOGIN_HISTORY_LOCK: Mutex<()>` | 登录历史读-改-写全程互斥（防并发追加互相覆盖） |
| `persist.rs:13-44` | pub fn | `atomic_write(path: &Path, content: &str) -> Result<(), String>` | 原子写：临时文件（纳秒时间戳后缀）→ BufWriter + flush → `sync_all` → `rename` 重试 3 次（间隔 100ms），失败清理临时文件并返回 Err |
| `persist.rs:47-60` | pub fn | `get_data_dir(app_handle) -> PathBuf` | Tauri `app_data_dir()`；失败回退 `dirs::data_dir()/campus-login`，再回退 `"."`；不存在则 `create_dir_all` |
| `persist.rs:61-63` | pub fn | `get_config_path(data_dir) -> PathBuf` | `{data_dir}/config.json` |
| `persist.rs:65-67` | pub fn | `get_accounts_dir(data_dir) -> PathBuf` | `{data_dir}/accounts` |
| `persist.rs:70-72` | pub fn | `get_account_path(data_dir, account_id) -> PathBuf` | 账号档案文件路径 `{accounts_dir}/<id>.json`（id = 文件名 stem） |
| `persist.rs:90-111` | pub fn | `load_account_config(data_dir, account_id) -> Result<Config, String>` | 读账号档案（扁平 Config JSON）：文件不存在返回 `Err("账号不存在")`；读盘/解析失败返回 Err；非空密码 DPAPI 解密（失败 `log_error` + Err） |
| `persist.rs:101-112` | pub fn | `save_account_config(data_dir, account_id, config) -> Result<(), String>` | 写账号档案：创建目录 → 非空密码 DPAPI 加密 → `to_string_pretty` → `atomic_write`（与 `save_config_to_disk_encrypted` 同一加密约定） |
| `persist.rs:118-168` | pub fn | `list_account_items(data_dir) -> Vec<AccountItem>` | 枚举 `accounts/*.json` 为 `{id, display_name, user, operator, password_saved}`（过滤 `.` 前缀与空名，按 id 排序）；display_name 为空/文件损坏时兜底为 id 并 `log_warn` |
| `persist.rs:170-172` | pub fn | `get_login_history_path(data_dir) -> PathBuf` | `{data_dir}/login-history.json` |
| `persist.rs:174-226` | pub fn | `append_login_history(app_handle, success: bool, message: &str, adapter: &str, user: &str, login_type: &str) -> Result<(), String>` | 加锁 → 读现有历史（读/解析失败 rename 为 `.bak` 后重置）→ 头部插入新记录 → 截断至 100 条 → `atomic_write` |
| `persist.rs:229` | 私有 static | `QUALITY_HISTORY_LOCK: Mutex<()>` | 质量历史读-改-写全程互斥 |
| `persist.rs:231-233` | pub fn | `get_quality_history_path(data_dir) -> PathBuf` | `{data_dir}/quality_history.json` |
| `persist.rs:237-239` | pub fn | `append_quality_history(app_handle, result: &NetworkQualityResult) -> Result<(), String>` | 委托给 `append_quality_history_to(get_data_dir(...), result)` |
| `persist.rs:241-288` | pub fn | `append_quality_history_to(data_dir, result: &NetworkQualityResult) -> Result<(), String>` | 加锁 → 读现有历史（读/解析失败 rename 为 `.bak` 后重置）→ 头插法加入 `{timestamp, gatewayLatency, externalLatency, quality}`（延迟 <0 落盘为 null）→ 截断至 100 条 → `atomic_write` |
| `persist.rs:302-328` | pub fn | `save_config_and_broadcast(app_handle: &AppHandle, config: &Config) -> Result<(), String>` | `get_data_dir` → `save_config_to_disk_encrypted` → 掩码后 `notify_config_changed`（`{"config": …}` 包裹）；不刷托盘；后台线程触发 `refresh_core_config_if_changed` 实现无头核心凭据跟随 |
| `persist.rs:324-336` | pub fn | `save_config_to_disk_encrypted(data_dir: &Path, config: &Config) -> Result<(), String>` | 克隆配置 → 非空 `password` → `crypto::encrypt` → `serde_json::to_string_pretty` → `atomic_write`；**不做任何校验** |

### config/validate.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `validate.rs:5-8` | lazy_static | `USERNAME_RE = ^[a-zA-Z0-9._-]+$` / `CUSTOM_COLOR_RE = ^#[0-9a-fA-F]{6}$` | 用户名与主题色格式白名单 |
| `validate.rs:10-21` | pub fn | `validate_username(user: &str) -> Result<&str, String>` | 非空 + ≤64 字节 + 字符白名单 |
| `validate.rs:23-29` | pub fn | `validate_operator(op: &str) -> Result<&str, String>` | 仅允许 `""`/`"@telecom"`/`"@unicom"`/`"@cmcc"` |
| `validate.rs:31-39` | pub fn | `validate_password(password: &str) -> Result<(), String>` | 非空 + ≤128 字节 |
| `validate.rs:42-71` | 私有 fn | `validate_portal_url(url: &str) -> Result<(), String>` | 协议限 http/https；host 必须是内网 IPv4 / 回环 IPv6 / 字面量 `localhost`；**域名一律拒绝** |
| `validate.rs:73-79` | 私有 fn | `migrate_operator(op: &mut String)` | `@ctcc`→`@telecom`、`@cucc`→`@unicom` |
| `validate.rs:81-85` | 私有 fn | `normalize_portal_url(url: &mut String)` | 空串或字面值 `"http://10.1.99.100:801"` → `"http://10.1.99.100"` |
| `validate.rs:87-183` | pub fn | `validate_config(config: Config) -> Result<Config, String>` | 严格校验 + 迁移（v1→v2 小时→分钟、v2→v3 检测时段终点旧默认、v3→v4 间隔默认值、v4→v5 夜切默认与定时哨兵）+ clamp；任一项非法即 `Err` |
| `validate.rs:188-255` | pub fn | `validate_config_lenient(mut config: Config) -> Config` | 逐字段降级：无效字段回退 `Config::default()` 对应值并 `log_warn!`；末尾再跑一次严格版，仍失败则整份返回 `Config::default()` |

## Architecture

```text
┌──────────────────────────────────────────────────────────────────────┐
│                        Config Module (config/)                       │
│                                                                      │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────────────────┐ │
│  │  model.rs    │   │  persist.rs  │   │      validate.rs         │ │
│  │  struct      │──►│  atomic_write│──►│  validate_config         │ │
│  │  Config      │   │  save_*_enc  │   │  validate_config_lenient │ │
│  │  Default     │   │  load_*_cfg  │   │  migrate_*               │ │
│  │  masked_*    │   │  append_*_hist│  │  USERNAME_RE             │ │
│  └──────────────┘   └──────────────┘   │  CUSTOM_COLOR_RE         │ │
│                                        └──────────────────────────┘ │
│                                                                      │
│  Data Flow:                                                          │
│  Load: disk ► serde ► lenient ► strict ► state.store                 │
│  Save: frontend ► strict ► encrypt ► atomic_write ► broadcast        │
└──────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### 加载（启动 / 账号切换 / 配置导入）

```text
磁盘配置文件 → get_data_dir (persist.rs:46) → get_config_path (persist.rs:61)
  → serde_json::from_str::<Config> (容器级 default 补齐缺失字段，model.rs:9)
  → password 非空且 != PASSWORD_MASK → account::crypto::decrypt (persist.rs:87)
  → validate_config_lenient (validate.rs:188)
      逐字段降级 → 末尾 validate_config (validate.rs:248) 兜底 → 仍失败则 Config::default() (model.rs:244)
```

### 保存

```text
前端 save_config IPC
  → validate_config(config) (validate.rs:87)  非法即拒绝，不落盘
  → persist::save_config_and_broadcast(app_handle, &config) (persist.rs:296)
      ├─ save_config_to_disk_encrypted (persist.rs:324)
      │     ├─ 非空 password → account::crypto::encrypt (DPAPI，persist.rs:331)
      │     ├─ serde_json::to_string_pretty
      │     └─ atomic_write (persist.rs:13)
      ├─ config.masked_for_display() (model.rs:322) → notify_config_changed (persist.rs:302)
      └─ 后台线程 refresh_core_config_if_changed (persist.rs:311)
```

### 登录历史追加

```text
auth / monitor 层调用
  → config::persist::append_login_history (persist.rs:174)
      LOGIN_HISTORY_LOCK (persist.rs:11,175) → 读 login-history.json (坏文件 → *.bak 重置)
      → history.insert(0, {time, success, message, adapter, user, type})
      → 超过 100 条则 truncate(100) → atomic_write (persist.rs:223)
```

### 网络质量历史追加

```text
network::quality 检测完成后
  → config::persist::append_quality_history (persist.rs:237)
      QUALITY_HISTORY_LOCK (persist.rs:229,242) → 读 quality_history.json（坏文件 → *.bak 重置）
      → history.insert(0, {timestamp, gatewayLatency, externalLatency, quality})
      → 超过 100 条则 truncate(100) → atomic_write (persist.rs:285)
```

### 出站掩码（唯一出口）

```text
任意把 Config 发往前端的路径
  → Config::masked_for_display() (model.rs:322)
       password 非空 → "***"（凭据统一后唯一敏感字段）；空值保留（"未设置"语义）
```

## Connections

- [[desktop-commands]] — `commands/config_cmd.rs`（`save_config` 等经由 `persist::save_config_and_broadcast`，见 `persist.rs:296-322`）、`commands/account.rs`（账号档案读写 + `crypto::encrypt/decrypt` + `masked_for_display`，对应 `persist.rs:301-337, 296-336`、`model.rs:326-340`）、`commands/updater.rs`（`skip_sha256_when_missing`，`model.rs:326-327`）。
- [[desktop-auth]] — `config::validate::{validate_username, validate_operator, validate_password}`（`validate.rs:10-39`）被协议层校验复用；`config::persist::append_login_history`（`persist.rs:174-226`）被 session 调用。
- [[desktop-monitor]] — `enable_background_check` / `background_check_interval`（`model.rs:48-51`）/ `auto_login_on_preparation`（`model.rs:52-53`）/ `auto_exit_on_online`（`model.rs:114-115`）/ `enable_network_name_check`（`model.rs:146-147`）/ `required_network_name`（`model.rs:144-145`）/ `max_disconnect_reconnect`（`model.rs:174-175`）/ `auto_login_cooldown_secs`（`model.rs:176-177`）/ `enable_network_quality`（`model.rs:134-135`）的消费点。
- [[desktop-network-core]] — `network::client::PORTAL_URL`（用 `default_portal_url()` `model.rs:232` 初始化）、`network::quality` 消费 `fixed_gateway`（`model.rs:142-143`）/`skip_*_in_latency`（`model.rs:136-139`）；`AUTO_DETECT_ADAPTER`（`model.rs:4`）用于适配器比较。
- [[desktop-infra]] — `infra::state::store`（`list_account_items` 返回 `AccountItem` 见 `persist.rs:118-168`）、`infra::logger`（`set_log_retention_days` 消费 `log_retention_days` `model.rs:172-173`）。
- [[desktop-account-selfservice]] — `config/persist.rs:7` 依赖 `account::crypto`；`self_hello_enabled`（`model.rs:17-18`）与 `self_reverify_each_action`（`model.rs:20-21`）是自助服务面板的配置面。

## Known Issues

1. **密码校验面收敛到单字段**：凭据统一后唯一密码字段是 `password`，`validate_config` 只在 `validate.rs:92-94` 校验它。
2. **`update_source` 无取值校验**：`model.rs:151-152` 只声明 `default`，`validate.rs` 不校验其取值；消费点用 `!= "github"` 判定，任何拼写错误都会静默走镜像优先。
3. **`normalize_portal_url` 只认一个字面值**：`validate.rs:81-85` 硬比较 `"http://10.1.99.100:801"` 与空串。用户填其他带 `:801` 的地址不会被归一化。
4. **容器级 `serde(default)` 让字段名错误静默降级**：`model.rs:9` 无 `deny_unknown_fields`，前端若传拼错键，反序列化不报错、值静默丢成默认值。同因，`Config` 的 JSON 键名靠手写 `rename` 维护（仅 `user`/`password`/`operator`/`adapter1`/`adapter2` 用原名），新增字段漏加 `rename` 无编译期保护——新账号字段的 JSON 名有契约锁测试 `serde_account_fields_json_names_and_defaults`（`model.rs:375-404`）兜底，但其余字段仍裸奔。
5. **`validate_config` 对 `adapter1`/`adapter2`/`adapter1Account`/`adapter2Account`/`active_account`/`display_name`/`default_panel` 均无校验**：`validate.rs:87-183` 的检查清单只覆盖 user/password/operator/颜色/主题/两个 interval/portal_url/两个 gateway/required_network_name/log_retention_days/四个分钟字段；账号档案名另有 `infra::state::validate_account_name` 把关，但配置内的 `active_account`、适配器绑定的账号 id 本身不受校验。
6. **`log_retention_days` 只限上限不限下限**：`validate.rs:125-127` 仅 `> 365` 时截到 365；`0` 有"永久保留"语义，但无下界保护。
7. **宽松校验的最终兜底会丢全部配置**：`validate.rs:248-254`，降级后仍失败时直接返回 `Config::default()`（仅 `log_warn!` 告警，用户无感知）。
8. **`validate_config_lenient` 不做 interval clamp 之外的数值归一**：`validate.rs:188-255` 未处理 `background_check_interval` / `latency_test_interval` / `log_retention_days`，依赖末尾的 `validate_config` 兜底完成 clamp（`validate.rs:103-104`、`validate.rs:125-127`）。
9. **`atomic_write` 的临时文件名会替换原扩展名**：`persist.rs:15-21` 用 `path.with_extension("json.tmp.{nanos}")`，对 `config.json` 得到 `config.json.tmp.123`；若传入无扩展名的路径，语义会变成"替换整个文件名"。另 `rename` 仅重试 3×100ms（`persist.rs:15-22`），Windows 上被索引/杀软占用时失败即删除临时文件并返回 Err（原文件保持完整）。
10. **`save_config_to_disk_encrypted` 不做校验且会对 MASK 加密**：`persist.rs:324-336` 直接落盘传入的 `Config`；`persist.rs:330-332` 明确不排除 `PASSWORD_MASK`（注释 `persist.rs:327-329` 说明：若真实密码恰为 `"***"`，排除判断会让它明文落盘）。代价是：任何把 MASK 占位符直接落盘的调用方，重启后会得到明文密码 `"***"`（当前调用方需在落盘前还原真值，属依赖调用方正确性）。`save_account_config`（`persist.rs:101-112`）沿用同一约定。
11. **登录历史锁只覆盖 `append_login_history`**：`persist.rs:11` 的 `LOGIN_HISTORY_LOCK` 仅在 `persist.rs:175` 加锁；历史为"全量读 → 写"（`persist.rs:180-223`），每次追加都是 O(100) 次序列化。账号档案的读写（`load_account_config`/`save_account_config`/`list_account_items`）没有对应的静态锁，靠命令层串行化与调用方纪律。
12. **登录历史读取失败/解析失败只做 `.bak` 改名后重置**：`persist.rs:170`、`persist.rs:181`/`184-185`/`195-196` 的备份路径用 `format!("{}.bak", ...)`（固定名，第二次失败会覆盖上一次备份），且失败被隐式忽略（`let _ =`）。
13. **配置迁移是链式的 `config_version` 递增，且 v4→v5 与新默认分歧**：`validate.rs:130-174` 现有四段——v1→v2（`validate.rs:130-137`）：`campus_check_start_minutes` 在 `(0, 24)` 视为小时值 ×60；v2→v3（`validate.rs:141-146`）：`campus_check_end_minutes == 0`（旧默认）刷为 1380；v3→v4（`validate.rs:151-159`）：等于旧默认的间隔刷为新默认；v4→v5（`validate.rs:163-174`）：`validate.rs:164-166` 强制把 `enable_night_operator_switch` 置 `true`（夜切默认开）+ 定时禁用哨兵 1440。但 `Config::default()`（`model.rs:271`）自 2026-10-02 起已将 `enable_night_operator_switch` 改回 `false`；新建配置 `config_version = 5`（`model.rs:317`）不触发迁移，导致存量升级与新默认行为不一致，仅依赖版本号区分。
14. **`validate_portal_url` 拒绝域名**：`validate.rs:63-65`，host 既非 IP 也非字面量 `localhost` 即报"仅允许IP地址，不支持域名"。校园网改用域名 Portal 时需改代码。
15. **`validate_portal_url` 的 IPv6 分支只允许回环**：`validate.rs:57-61`，非 `::1` 的 IPv6 一律拒绝，错误文案却写"仅允许内网IPv4或localhost"。
16. **`campus_check_*` / `campus_exit_*` / `scheduled_*_minutes` 只做 `min(1439)` 或 `min(1440)` 钳制**：`validate.rs:175-181`，无"起点晚于终点"的纠正（语义上允许，退化为仅受起点限制，见 `model.rs:155-166` 注释），但 UI 与日志可能展示出反直觉的时段。
17. **`fixed_gateway` 允许空串**：`validate.rs:107-109` 仅在非空时校验 IP，空值语义（是否启用固定网关）没有显式开关，由消费方 `network::quality` 自行解释。

## 结构体与字段

### `Config`（`model.rs:10-182`）

`#[derive(Debug, Clone, Serialize, Deserialize)]`（`model.rs:6`）+ 容器级 `#[serde(default)]`（`model.rs:9`）。JSON 键名无统一 `rename_all`，未标 `rename` 的字段保持 Rust 原名（`user`、`password`、`operator`、`adapter1`、`adapter2`），其余均为显式 camelCase。

| 位置 | Rust 字段 | JSON 键 | 类型 | 默认值 | 含义 |
| --- | --- | --- | --- | --- | --- |
| `model.rs:11` | `user` | `user` | `String` | `""` | 校园网登录账号（学号） |
| `model.rs:13` | `password` | `password` | `String` | `""` | 登录密码；内存明文 / 磁盘 DPAPI 密文 / 出站 MASK；**唯一密码字段** |
| `model.rs:17-18` | `self_hello_enabled` | `selfHelloEnabled` | `bool` | `true`（`default_true`） | 是否启用 Windows Hello 操作验证门；关闭后"查看运营商账号密码"仍强制验证（后端 TTL 独立） |
| `model.rs:20-21` | `self_reverify_each_action` | `selfReverifyEachAction` | `bool` | `false` | 自助服务面板是否每次操作都二次验证（否则切入面板验证一次后共用）；**仅前端消费** |
| `model.rs:22` | `operator` | `operator` | `String` | `""` | 运营商后缀：`""`/`@telecom`/`@unicom`/`@cmcc` |
| `model.rs:23` | `adapter1` | `adapter1` | `String` | `"自动检测"` | 主适配器名或自动检测哨兵 |
| `model.rs:24` | `adapter2` | `adapter2` | `String` | `""` | 副适配器名（双适配器模式使用） |
| `model.rs:27-28` | `adapter1_account` | `adapter1Account` | `String` | `""` | 主适配器绑定的账号 id（设备级，仅桌面；空=跟随当前激活账号；切账号不覆盖） |
| `model.rs:30-31` | `adapter2_account` | `adapter2Account` | `String` | `""` | 副适配器绑定的账号 id，语义同上 |
| `model.rs:32-33` | `dual_adapter` | `dualAdapter` | `bool` | `false` | 是否启用双适配器同时登录 |
| `model.rs:34-35` | `auto_login_on_start` | `autoLoginOnStart` | `bool` | `true` | 启动时自动登录 |
| `model.rs:36-37` | `auto_exit_after_login` | `autoExitAfterLogin` | `bool` | `false`（2026-09-20 起默认关闭；存量显式值不迁移） | 登录成功后自动退出程序 |
| `model.rs:38-39` | `minimize_to_tray` | `minimizeToTray` | `bool` | `false` | 关闭窗口时最小化到托盘 |
| `model.rs:42-43` | `lightweight_mode` | `lightweightMode` | `bool` | `true`（`default_true`） | 轻量化模式：关闭按钮销毁 WebView 仅留后端与托盘 |
| `model.rs:44-45` | `hidden_start` | `hiddenStart` | `bool` | `false` | 启动时不显示主窗口 |
| `model.rs:46-47` | `auto_launch` | `autoLaunch` | `bool` | `true` | 开机自启 |
| `model.rs:48-49` | `enable_background_check` | `enableBackgroundCheck` | `bool` | `true` | 是否启用后台巡检 |
| `model.rs:50-51` | `background_check_interval` | `backgroundCheckInterval` | `u64` | `60000`（2026-09-20 起 15s → 60s，存量旧默认由 v3→v4 迁移刷新） | 后台巡检间隔（毫秒），clamp 到 `[10000, 3600000]` |
| `model.rs:52-53` | `auto_login_on_preparation` | `autoLoginOnPreparation` | `bool` | `true` | "登录准备"模式：检测到未登录且可登录时自动登录 |
| `model.rs:56-57` | `enable_night_operator_switch` | `enableNightOperatorSwitch` | `bool` | `false`（2026-10-02 起默认改回关闭；但 v4→v5 迁移在 `validate.rs:164-166` 会强制刷 true，存量已落盘 true 不迁移） | 晚间断网自动切换运营商总开关 |
| `model.rs:59-60` | `night_operator_restore` | `nightOperatorRestore` | `String` | `""` | 切至无锡学院前暂存的原运营商；空 = 未处于切换态。恢复后清空 |
| `model.rs:63-64` | `enable_night_outbound_switch` | `enableNightOutboundSwitch` | `bool` | `false` | 夜间出站自动切换总开关 |
| `model.rs:69-70` | `outbound_priority` | `outboundPriority` | `Vec<String>` | `[]` | 出站网卡优先级（友好名有序列表，首项 = 夜间出站目标）；空 = 未排序。安卓端不消费此字段 |
| `model.rs:74-75` | `outbound_metric_restore` | `outboundMetricRestore` | `String` | `""` | 切换态快照：`[{guid, family, automatic, metric}]` JSON；非空 = 桌面处于出站切换态。导入配置时清空。安卓端不消费此字段 |
| `model.rs:80-81` | `outbound_disabled_adapters` | `outboundDisabledAdapters` | `String` | `""` | 夜间切换禁用的校园网卡快照：`[{guid, name}]` JSON；非空 = 存在待还原的禁用动作。导入配置时保留本机值。安卓端不消费此字段 |
| `model.rs:85-86` | `outbound_standby_route` | `outboundStandbyRoute` | `String` | `""` | 夜间切换新增的兜底默认路由快照：`{dest, mask, gateway, metric, ifIndex}` JSON；空 = 未加路由。导入配置时清空。安卓端不消费此字段 |
| `model.rs:92-93` | `outbound_manual_hold_day` | `outboundManualHoldDay` | `i32` | `0` | 「立即切换」当日保持标记（num_days_from_ce；0=无）；次日 7:30 自动还原或手动还原成功后清零。导入配置时保留本机值。安卓端不消费此字段 |
| `model.rs:98-99` | `outbound_pending_switch` | `outboundPendingSwitch` | `String` | `""` | 待切换状态：`{target, tag}` JSON；非空 = 目标网卡尚未登录校园网、切换被挂起；监测确认目标卡在线后由排程循环自动完成切换。导入配置时保留本机值。安卓端不消费此字段 |
| `model.rs:102-103` | `night_outbound_restore` | `nightOutboundRestore` | `String` | `""` | 安卓切换态标记：值恒 `"logged_out"`；非空 = 安卓处于出站切换态。切账号时清空。桌面端不消费此字段 |
| `model.rs:107-108` | `dns_optimize_adapters` | `dnsOptimizeAdapters` | `Vec<String>` | `[]` | DNS 优化目标适配器名单（网卡名）。安卓端不消费此字段（DNS 优化为桌面专属功能） |
| `model.rs:112-113` | `device_aliases` | `deviceAliases` | `HashMap<String, String>` | `{}` | 在线设备自定义名字（键 = 12 位 hex MAC，值 = 显示别名） |
| `model.rs:114-115` | `auto_exit_on_online` | `autoExitOnOnline` | `bool` | `false`（2026-09-20 起默认关闭，存量不迁移） | 检测到在线后自动退出 |
| `model.rs:116-117` | `theme_mode` | `themeMode` | `String` | `"dark"` | 主题：`"dark"`/`"light"`/`"system"` |
| `model.rs:118-119` | `enable_notification` | `enableNotification` | `bool` | `true` | 是否发系统通知 |
| `model.rs:120-121` | `active_account` | `activeAccount` | `String` | `""` | 当前生效的账号档案名（`accounts/*.json` 的文件名词干） |
| `model.rs:124-125` | `display_name` | `displayName` | `String` | `""` | 本文件所属账号的显示名（与内部 id 分离，可含空格/emoji 等任意字符）；空 → 读取阶段兜底用账号 id |
| `model.rs:126-127` | `enable_latency_test` | `enableLatencyTest` | `bool` | `true` | 是否启用延迟测试 |
| `model.rs:128-129` | `latency_test_interval` | `latencyTestInterval` | `u64` | `600000`（2026-09-20 起 60s → 600s，存量旧默认由 v3→v4 迁移刷新） | 延迟测试间隔（毫秒），clamp 到 `[10000, 3600000]` |
| `model.rs:130-131` | `custom_theme_color` | `customThemeColor` | `String` | `"#6366f1"` | 自定义主题色（`#RRGGBB`） |
| `model.rs:132-133` | `default_panel` | `defaultPanel` | `String` | `""` | 启动时默认显示的面板；**仅前端消费，生产代码 Rust 侧不消费** |
| `model.rs:134-135` | `enable_network_quality` | `enableNetworkQuality` | `bool` | `true` | 是否启用网络质量检测 |
| `model.rs:136-137` | `skip_ttfb_in_latency` | `skipTtfbInLatency` | `bool` | `true`（`default_true`） | 延迟测试跳过 TTFB 指标 |
| `model.rs:138-139` | `skip_content_in_latency` | `skipContentInLatency` | `bool` | `true`（`default_true`） | 延迟测试跳过内容下载指标 |
| `model.rs:140-141` | `portal_url` | `portalUrl` | `String` | `"http://10.1.99.100"`（`default_portal_url`） | Portal 基地址；保存时同步到全局 `network::client::PORTAL_URL` |
| `model.rs:142-143` | `fixed_gateway` | `fixedGateway` | `String` | `"10.2.127.254"`（`default_fixed_gateway`） | 固定网关地址（延迟/质量测试目标） |
| `model.rs:144-145` | `required_network_name` | `requiredNetworkName` | `String` | `"i-wxxy、iwxxy-2、iwxxy-3"`（`default_required_network_name`） | 校园 WiFi SSID；空串反序列化回默认 |
| `model.rs:146-147` | `enable_network_name_check` | `enableNetworkNameCheck` | `bool` | `true`（`default_true`） | 是否启用 SSID 判定 |
| `model.rs:148-149` | `campus_gateway` | `campusGateway` | `String` | `"10.2.127.254"`（`default_campus_gateway`） | 校园网关（MAC 重置 / 可达性判定的目标）；空串反序列化回默认 |
| `model.rs:151-152` | `update_source` | `updateSource` | `String` | `"mirror"`（仅 `#[serde(default)]`，由 `Config::default` 赋 `"mirror"`） | 更新渠道优先级：`"mirror"`=镜像优先 / `"github"`=官方优先 |
| `model.rs:153-154` | `campus_exit_on_fail` | `campusExitOnFail` | `bool` | `true`（`default_true`） | 非校园网时是否自动退出 |
| `model.rs:156-157` | `campus_exit_start_minutes` | `campusExitStartMinutes` | `u16` | `480`（08:00） | 非校园网自动退出生效时段起点（当日分钟数） |
| `model.rs:159-160` | `campus_exit_end_minutes` | `campusExitEndMinutes` | `u16` | `1380`（23:00） | 生效时段终点（不含该时刻；结束 ≤ 起点时退化为仅受起点限制） |
| `model.rs:161-162` | `campus_check_start_minutes` | `campusCheckStartMinutes`（alias `campusCheckStartHour`） | `u16` | `460`（07:40） | 校园网检测时段起点 |
| `model.rs:164-165` | `campus_check_end_minutes` | `campusCheckEndMinutes` | `u16` | `1380`（23:00，旧默认 `0` 由 v2→v3 迁移刷为 1380） | 检测时段终点；≤ 起点时退化为仅起点限制 |
| `model.rs:167-168` | `scheduled_login_minutes` | `scheduledLoginMinutes` | `u16` | `1440`（每日定时登录时刻，1440=禁用哨兵、0=真实的 00:00；存量 0 由 v4→v5 迁移刷为 1440） | 到点即触发含过点补触发，判定见 `config::schedule` |
| `model.rs:170-171` | `scheduled_logout_minutes` | `scheduledLogoutMinutes` | `u16` | `1440`（每日定时注销时刻，1440=禁用哨兵；语义同 scheduled_login_minutes） | 到点即触发含过点补触发，判定见 `config::schedule` |
| `model.rs:172-173` | `log_retention_days` | `logRetentionDays` | `u32` | `7`（`default_log_retention_days`） | 日志保留天数；`0`=永久保留；上限 365 |
| `model.rs:174-175` | `max_disconnect_reconnect` | `maxDisconnectReconnect` | `u32` | `3`（`default_max_disconnect_reconnect`） | 断线重连次数上限 |
| `model.rs:176-177` | `auto_login_cooldown_secs` | `autoLoginCooldownSecs` | `u64` | `60`（`default_auto_login_cooldown_secs`） | 自动登录冷却秒数 |
| `model.rs:178-179` | `skip_sha256_when_missing` | `skipSha256WhenMissing` | `bool` | `false`（`#[serde(default)]`） | 更新包缺少 sha256 时是否跳过校验 |
| `model.rs:180-181` | `config_version` | `configVersion` | `u32` | `5`（v5：见 `validate.rs` 迁移逻辑） | 配置版本号，驱动 v1→v2、v2→v3、v3→v4、v4→v5 迁移 |

### `Config` 的 serde 属性要点

| 位置 | 属性 | 作用 |
| --- | --- | --- |
| `model.rs:9` | `#[serde(default)]`（容器级） | 任一字段缺失即用 `Default` 补齐；否则单字段缺失会导致整份配置反序列化失败 |
| `model.rs:12` | `password` 上的 `#[serde(default)]` | 与容器级重复，冗余但无害 |
| `model.rs:17` | `default = "default_true"` | `self_hello_enabled` 默认 `true` |
| `model.rs:42` | `default = "default_true"` | `lightweight_mode` 默认 `true` |
| `model.rs:136`、`model.rs:138` | `default = "default_true"` | 两个 skip 开关默认 `true` |
| `model.rs:146`、`model.rs:153` | `default = "default_true"` | `enable_network_name_check` / `campus_exit_on_fail` 默认 `true` |
| `model.rs:140`、`model.rs:142` | `default = "default_portal_url"` / `default_fixed_gateway` | 空值落盘时回退对应默认值 |
| `model.rs:144`、`model.rs:148` | `deserialize_with = "deserialize_..."` | 空串替换为 `"i-wxxy、iwxxy-2、iwxxy-3"` / `"10.2.127.254"` |
| `model.rs:151`、`model.rs:178` | `#[serde(default)]` | 无自定义 fn，依赖容器级 `Default` 补默认值（`"mirror"` / `false`） |
| `model.rs:161` | `alias = "campusCheckStartHour"` | 兼容旧版字段名（配合 `config_version < 2` 的小时→分钟迁移） |
| `model.rs:164` | `default = "default_campus_check_end_minutes"` | `campus_check_end_minutes` 缺字段时取 1380 |
| `model.rs:27-31`、`model.rs:124-125` | `rename = "adapter1Account"/"adapter2Account"/"displayName"` | 新字段手写 rename；缺字段靠容器级 `serde(default)` 补空串（测试 `serde_account_fields_json_names_and_defaults` 锁契约） |
| `model.rs:56-103` | `rename = "enableNightOperatorSwitch"/.../"outboundPendingSwitch"/"nightOutboundRestore"` | 夜间出站/运营商切换多字段手写 rename；缺字段靠容器级 `serde(default)` 兜底，纯新增、无 schema 迁移 |