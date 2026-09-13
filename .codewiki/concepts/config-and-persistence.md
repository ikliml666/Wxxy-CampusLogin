---
title: 配置模型与持久化
type: concept
source_files:
  - tauri-app/src-tauri/src/config/mod.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/persist.rs
  - tauri-app/src-tauri/src/config/validate.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
  - tauri-app/src-tauri/src/commands/account.rs
  - tauri-app/src-tauri/src/commands/system.rs
  - tauri-app/src-tauri/src/commands/background.rs
  - tauri-app/src-tauri/src/commands/network_cmd.rs
  - tauri-app/src-tauri/src/infra/state/mod.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/account_cmds.rs
  - android/src-tauri/src/login_history.rs
  - android/src-tauri/src/monitor_loop.rs
  - tauri-app/frontend/src/hooks/useConfigStore.ts
  - tauri-app/frontend/src/hooks/useInitialDataLoad.ts
  - android/frontend/src/hooks/useConfigStore.ts
  - android/frontend/src/settings/types.ts
  - android/frontend/src/settings/constants.ts
tags: [概念, 配置, 持久化, 迁移, 原子写, config-changed]
---

## Overview

配置是横跨双端的核心数据：桌面 `config::Config`（44 字段，单文件 `config.json` + `accounts/*.json` + `login-history.json`），安卓 `config_state::Settings`（35 字段，同结构但密文外置为 `EncodedSettings`）。落盘走"临时文件 + rename"原子写，密码字段加密后落盘，读取时经 `validate_config_lenient` 逐字段降级。配置变更经 `config-changed` 事件（桌面）或命令返回值（安卓）回流前端。

## 机制说明

### 桌面配置模型（`config/model.rs`）

`Config` 定义在 `model.rs:10-104`，共 **44 个字段**。关键结构决策：

```rust
// model.rs:7-9 容器级 default：任一字段缺失（旧版本配置/手工编辑）都用 Default 补齐，
// 否则 serde 缺一个字段即整体反序列化失败 → 上层全量重置丢配置
#[serde(default)]
pub struct Config {
```

字段分两类：

| 类别 | 字段 | 说明 |
|---|---|---|
| 必需（无 `default`，缺即整体失败但被容器级 default 兜住） | `user` `operator` `adapter1` `adapter2` `dual_adapter` `auto_login_on_start` `auto_exit_after_login` `minimize_to_tray` `hidden_start` `auto_launch` `enable_background_check` `background_check_interval` `auto_login_on_preparation` `auto_exit_on_online` `theme_mode` `enable_notification` `active_account` `enable_latency_test` `latency_test_interval` `custom_theme_color` `default_panel` `enable_network_quality` | 走容器级 `Default` |
| 字段级 `default = "fn"` | `password` `self_password` `self_hello_enabled`(`default_true`) `self_reverify_each_action` `skip_ttfb_in_latency`(`true`) `skip_content_in_latency`(`true`) `portal_url` `fixed_gateway` `required_network_name`(+`deserialize_with`) `enable_network_name_check`(`true`) `campus_gateway`(+`deserialize_with`) `update_source` `campus_exit_on_fail`(`true`) `campus_exit_start_minutes`(480) `campus_exit_end_minutes`(1380) `campus_check_start_minutes`(460, `alias = "campusCheckStartHour"`) `campus_check_end_minutes`(0) `log_retention_days`(7) `max_disconnect_reconnect`(3) `auto_login_cooldown_secs`(60) `skip_sha256_when_missing` `config_version`(0→Default 里为 2) | 逐字段兜底 |

命名契约：Rust snake_case + `#[serde(rename = "camelCase")]`，与前端 TS 类型逐字对应（例：`self_password` → `selfPassword`、`background_check_interval` → `backgroundCheckInterval`、`campus_check_start_minutes` → `campusCheckStartMinutes`）。

特殊反序列化：`deserialize_non_empty_or`（`model.rs:106-116`）用于 `required_network_name` 与 `campus_gateway`——空串回退默认值（`i-wxxy` / `10.2.127.254`）。

默认值函数集中在 `model.rs:132-160`，`Default for Config` 在 `:162-211`（注意 `config_version: 2`、`background_check_interval: 15000`、`latency_test_interval: 60000`、`theme_mode: "dark"`、`update_source: "mirror"`、`adapter1: AUTO_DETECT_ADAPTER`("自动检测")）。

### 桌面原子写与文件布局（`config/persist.rs`）

```rust
// persist.rs:12-43  atomic_write
let tmp_path = path.with_extension(format!("json.tmp.{}", ...as_nanos()));  // :14-20 纳秒时间戳防并发覆盖
let tmp_file = std::fs::File::create(&tmp_path)?;                          // :23
writer.write_all(content.as_bytes())?; writer.flush()?;                    // :26-30
tmp_file.sync_all()?;                                                      // :31 断电保证内容完整
for attempt in 0..3 { if std::fs::rename(&tmp_path, path).is_ok() { return Ok(()); } 
    std::thread::sleep(100ms); }                                            // :32-39 rename 重试 3 次 × 100ms
let _ = std::fs::remove_file(&tmp_path); Err("重命名临时文件失败（重试3次后）")  // :41-42
```

文件布局（全部落在 `app_data_dir`，见 `get_data_dir` `persist.rs:45-58`）：

| 文件 | 路径函数 | 内容 |
|---|---|---|
| 主配置 | `get_config_path` → `<data>/config.json`（`persist.rs:60-62`） | `Config` JSON，`password` / `selfPassword` 为 DPAPI 密文（base64） |
| 账号 | `get_accounts_dir` → `<data>/accounts/<名字>.json`（`persist.rs:64-66`） | 与主配置同格式的 `Config`；密码密文 |
| 登录历史 | `get_login_history_path` → `<data>/login-history.json`（`persist.rs:95-97`） | JSON 数组，头插，**上限 100 条**（`:141-143`） |

`list_account_names`（`persist.rs:68-93`）过滤 `.` 前缀与空名、结果排序。`append_login_history`（`:99-151`）用 `LOGIN_HISTORY_LOCK`（`:10`）串行化读-改-写，解析失败时把原文件重命名为 `<path>.bak` 后重置。

### 桌面校验与迁移（`config/validate.rs`）

| 校验 | 规则 | 位置 |
|---|---|---|
| `validate_username` | 非空、≤64 字符、`^[a-zA-Z0-9._-]+$` | `:10-21`，正则 `:6` |
| `validate_operator` | 仅 `""` / `@telecom` / `@unicom` / `@cmcc` | `:23-29` |
| `validate_password` | 非空、≤128 字符 | `:31-39` |
| `validate_portal_url` | 仅 http/https；host 必须是内网 IPv4 / loopback / `localhost`，禁域名 | `:42-71` |
| `custom_theme_color` | `^#[0-9a-fA-F]{6}$` | `:97-99`，正则 `:7` |
| `theme_mode` | 必须是 `dark` / `light` / `system` | `:100-102` |

迁移与归一化（都在 `validate_config` `:87-138` 内）：

| 迁移 | 规则 | 位置 |
|---|---|---|
| 运营商别名 | `@ctcc` → `@telecom`；`@cucc` → `@unicom` | `migrate_operator` `:73-79`，调用 `:95/161` |
| Portal URL | `http://10.1.99.100:801` 或空串 → `http://10.1.99.100` | `normalize_portal_url` `:81-85`，调用 `:105/177` |
| 间隔 clamp | `background_check_interval` 与 `latency_test_interval` → `[10000, 3600000]` | `:103-104` |
| 日志保留 | `log_retention_days > 365` → `365`（0 = 永久保留，不重置） | `:119-122` |
| 配置版本 | `config_version < 2` 时，`campus_check_start_minutes` 若在 `(0, 24)` 视为小时值 ×60，并置 `config_version = 2` | `:125-132` |
| 分钟字段上界 | 四个 `*_minutes` 字段 `.min(1439)` | `:133-136` |
| 空值补齐 | `campus_gateway` / `required_network_name` 空 → 默认值 | `:110-118` |

**宽松校验**：`validate_config_lenient`（`:143-207`）逐字段降级（每个失败只回退该字段并 `log_warn`），最后再跑一次严格 `validate_config` 兜底，仍失败才整体用 `Config::default()`（`:200-206`）。启动路径 `commands/config_cmd.rs:55-78` 用它；保存路径用严格版（`:99`）。

### 桌面加载与保存路径

```text
启动：app/startup.rs:158-163
  → commands/config_cmd.rs:55-78  load_config_from_disk_or_default
       → load_config_from_file（:20-53）
            ├─ 文件不存在 → Config::default()（:23-25）
            ├─ serde_json 解析
            ├─ password != 空/非 "***" → crypto::decrypt，失败仅清空该字段（:31-40）
            ├─ self_password 同规则（:42-50）
            └─ 解析/解密异常 → 原文件备份为 config.json.corrupt-<秒级时间戳>.bak 后返回默认（:59-76）
       → validate_config_lenient（:57）
  → state.config.store(config)（startup.rs:176）
  → network::update_portal_url + logger::set_log_retention_days（startup.rs:177-181）

保存：commands/config_cmd.rs:92-139  save_config
  → validate_config（严格）
  → clear_password / 空串 / "***" 三态处理（:109-123）
  → network::update_portal_url（:127） + logger::set_log_retention_days（:130）
  → save_config_to_disk_encrypted（先落盘）（:134）
  → state.config.store（内存）（:135）
```

顺序上"先落盘再更新内存"（`:132-133` 注释）：磁盘失败时命令返回 Err 且运行态不变，避免"保存失败但内存已生效、重启后回退"的错位。

`save_config_to_disk_encrypted`（`:9-18`）是**所有配置写入的公共出口**，职责有二：① 加密切片（委托 `persist::save_config_to_disk_encrypted`）；② 掩码后广播 `config-changed`。调用方覆盖：`save_config`、`switch_account`（`account.rs:43`）、`save_current_as_account`（`:184`）、`delete_account`（`:222`）、`set_auto_launch`（`system.rs:61`）、`set_notification_enabled`（`system.rs:84`）、`start_background_check_inner`（`background_task.rs:55`）、`stop_background_check`（`commands/background.rs:20`）、`set_boot_autostart`（安卓）等。

### 账号与登录历史存储

**桌面**（`commands/account.rs`，5 条命令）：

- 账号名消毒：`infra/state/mod.rs:64-72` `validate_account_name`——非空、≤32 字符、`ACCOUNT_NAME_RE`（字母/数字/下划线/中文/连字符），防路径穿越。
- `switch_account`（`account.rs:15-49`）：读账号文件 → 解密密码 → `state.config.update` 合并 `user`/`password`/`operator`/`adapter1`/`adapter2`/`dual_adapter`/`active_account` → 落盘。
- `save_current_as_account`（`:52-191`）：先把上一个 active 账号的登录字段回写（保留该账号文件里已有的主题等非登录字段，`existing.password` 先清空 `:83`），再写当前账号，密码走 `crypto::encrypt`（`:161-168`）；最后持久化 `active_account`。
- `delete_account`（`:194-233`）：删文件；若删的是当前账号则清 `active_account` 并持久化 + 广播（`:213-225`，注释记录"仅内存更新不落盘，重启后仍指向已删除账号"的历史缺陷）。
- 账号文件格式 = `Config`（`model.rs`），因此也带全部 44 字段与容器级 default。

**安卓**（`android/src-tauri/src/account_cmds.rs`）：

- 账号目录 `<app_data_dir>/accounts`（`account_cmds.rs:60-66`），账号名正则 `^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$`（`:13-14`），与桌面同款约束。
- 账号文件格式 = `EncodedSettings`（复用 `config_state::load_file` / `save_file`，`:94-99/158`），即密码密文外置。
- **`CONFIG_IO_LOCK`**（`:17`）：`tokio::sync::Mutex<()>` 异步锁，覆盖 `load → merge → save` 全序列（`config_io_lock()` `:21-23`），被 `switch_account`（`:127`）、`save_current_as_account`（`:156`）、`delete_account`（`:179`）、`set_boot_autostart`（`monitor_loop.rs:331`）、`set_notification_enabled`（`monitor_loop.rs:355`）共用。
- 合并字段只有三项：`user` / `password` / `operator`（`:134-137`，注释"adapter/双适配器为桌面专属,安卓不存在"）。

**安卓登录历史**（`login_history.rs`）：文件 `<app_data_dir>/login-history.json`（`:23-25`），头插上限 `LOGIN_HISTORY_MAX = 100`（`:10`，截断 `:67`），adapter 字段固定 `wlan0`（`:62`），字段名与桌面契约一致（`type` 而非 `login_type`，`:19-20`），损坏文件重命名为 `login-history.json.corrupt-<毫秒时间戳>.bak`（`:34`）后重置。写入用固定名 tmp + rename（`:72-74`），无 `sync_all`。

### 安卓配置模型与迁移（`config_state.rs`）

`Settings` 定义在 `config_state.rs:15-72`，共 **35 个字段**，序列化属性是 `#[serde(rename_all = "camelCase", default)]`（`:14`）——一次统一 camelCase，而不是桌面那样逐字段 `rename`。

与桌面 `Config` 的字段差异（`model.rs:10-104` vs `config_state.rs:15-72`）：**交集 31 个字段，桌面独有 13 个，安卓独有 4 个**。

| 仅桌面（13） | 仅安卓（4） |
|---|---|
| `adapter1` `adapter2` `dual_adapter` | `allow_2d_face_verify`（2D 人脸回退开关，`:24`） |
| `minimize_to_tray` `hidden_start` `auto_launch` | `background_check_idle_interval`（闲时巡检，`:34`） |
| `auto_exit_after_login` `auto_exit_on_online` | `enable_boot_autostart`（`:44`） |
| `campus_exit_on_fail` `campus_exit_start_minutes` `campus_exit_end_minutes` | `config_schema_version`（`:71`，桌面叫 `config_version`） |
| `skip_sha256_when_missing` `config_version` | |

共有（命名一致）：`password` `self_password` `self_hello_enabled` `self_reverify_each_action` `operator` `auto_login_on_start` `enable_background_check` `background_check_interval` `auto_login_on_preparation` `max_disconnect_reconnect` `auto_login_cooldown_secs` `theme_mode` `enable_notification` `custom_theme_color` `default_panel` `active_account` `enable_latency_test` `latency_test_interval` `enable_network_quality` `skip_ttfb_in_latency` `skip_content_in_latency` `portal_url` `fixed_gateway` `required_network_name` `enable_network_name_check` `campus_gateway` `campus_check_start_minutes` `campus_check_end_minutes` `update_source` `log_retention_days` `user`。

**磁盘格式 `EncodedSettings`**（`:166-172`）：

```rust
struct EncodedSettings {
    settings: Settings,            // password / self_password 已被置空
    password_cipher: String,
    self_password_cipher: String,
}
```

`save_file`（`:192-223`）加密后构造（`:213-217`），写 `.json.tmp` 再 rename（`:219-221`），**没有 `sync_all`、没有 rename 重试、tmp 名固定**——并发保护靠调用方的 `CONFIG_IO_LOCK`。空密码不产生密文位（回归测试 `空密码不写密文位` `:574-586`）。

**安卓迁移**（`migrate_legacy_defaults` `:241-266`，由 `load_from` `:225-229` 调用）：

| schema | 规则 | 位置 |
|---|---|---|
| 0 → 1 | `background_check_interval == 15_000` → `60_000` | `:243-245` |
| < 3（`legacy` 标记） | 强刷 `auto_login_on_start` / `enable_background_check` / `auto_login_on_preparation` / `enable_network_name_check` / `skip_ttfb_in_latency` / `skip_content_in_latency` = `true`；`< 3` 时 `enable_network_quality = false`；置 `config_schema_version = 3` | `:246-258` |
| < 4 | `background_check_idle_interval == 0` → `300_000`；置 4 | `:259-265` |

迁移结果落盘，落盘失败静默（下次读盘重迁，幂等）；迁移后用户主动改回不再被覆盖（回归测试 `迁移_旧默认15s升60s且落盘后用户值不被覆盖` `:483-521`）。

### 配置变更如何触发前端更新

**桌面（事件驱动）**：

```text
任意写配置路径
  → save_config_to_disk_encrypted                                  commands/config_cmd.rs:9-18
    → 加密落盘
    → app_handle.notify_config_changed(config.masked_for_display())  :15-16
      → EventBus.emit_config_changed("config-changed")               infra/events.rs:107-109
        → 前端 api.onConfigChanged                                   hooks/tauriApi.ts:203
          → useConfigStore.mergeConfigFromBackend                    hooks/useConfigStore.ts:110-117
```

`mergeConfigFromBackend` 逐字段合并，**跳过 `dirtyFields` 中的字段**（`:113-115`）——本地已改未确认的值不被后端旧快照回滚。

前端写配置的本地队列机制（`useConfigStore.ts`）：

| 机制 | 值 / 位置 |
|---|---|
| debounce 保存延迟 | 500 ms（`:84-93`） |
| 脏字段标记 | `updateConfig` / `updateConfigLocal` 都写 `dirtyFields`（`:69-72`、`:101-104`） |
| 脏字段失败放弃阈值 | `DIRTY_FAILURE_LIMIT = 3`（`:24`，计数逻辑 `:150-160`） |
| 重试封装 | 只有 `saveConfig` 包 `withRetry`（2 次，500ms 起指数退避）（`hooks/tauriApi.ts:237-261`） |
| 关窗等待 | `flushPendingConfig` + `Promise.race` 最多 2000 ms（`hooks/useEventListeners.ts:50-75`） |
| 启动首载 | `useInitialDataLoad.ts:34` `api.getInitData()`；`password === PASSWORD_MASK` → `syncPasswordSaved(true)`（`:37-44`） |

**安卓（返回值驱动）**：`save_config` 返回 `Result<(), String>`（`config_state.rs:311-338`），不发事件；账号三命令返回 `AccountResult { success, message, activeAccount, config }`（`account_cmds.rs:25-47`），`config` 为 `masked_for_display` 后的 JSON（`:111`）——前端拿返回值直接替换本地配置。

## 关键约束

- **新增配置字段必须同时改 5 处**：桌面 `config/model.rs` 字段 + `Default for Config`；安卓 `config_state.rs` 字段 + `Default for Settings`；两端前端 `settings/types.ts` + `settings/constants.ts` 的 `DEFAULT_CONFIG`。漏掉 `Default` 会让旧配置文件反序列化出 `false`/`0` 而非业务默认值。
- **容器级 `#[serde(default)]` 不可删**（`model.rs:9`）：否则新增字段会让所有存量配置文件整体反序列化失败 → 全量重置丢配置。
- **校验严格/宽松双路径不可混用**：启动读盘用 `validate_config_lenient`（`config_cmd.rs:57`），保存 / 导入用 `validate_config`（`:99`）。把宽松版用在保存路径会让非法输入静默落盘。
- **所有配置写入必须经 `save_config_to_disk_encrypted`**：它同时承担加密与 `config-changed` 广播（`config_cmd.rs:13-14` 注释）。绕过它的写入（直接 `persist::save_config_to_disk_encrypted` 或自己写 JSON）不会通知前端。
- **先落盘再更新内存**（`config_cmd.rs:132-133`）：顺序反了会出现"保存失败但内存已生效"。
- **账号名必须消毒**：桌面 `infra/state/mod.rs:64-72`、安卓 `account_cmds.rs:50-58`，防路径穿越（`../`、`/`、`\` 全被白名单正则拒绝）。
- **安卓配置读改写必须持 `CONFIG_IO_LOCK`**（`account_cmds.rs:21-23`）：`tokio::sync::Mutex` 而非 `parking_lot`，因为 guard 要跨 await 覆盖整个 load→merge→save 序列。
- **空密码不产生密文位**：安卓 `save_file:201-212` 只在非空时加密；桌面 `persist.rs:159-163` 同理。
- **登录历史上限 100 条**：桌面 `persist.rs:141-143`、安卓 `login_history.rs:10/67`。
- **`config_version`（桌面）与 `config_schema_version`（安卓）是两套独立机制**：桌面管小时→分钟迁移（`validate.rs:125-132`），安卓管默认值批次迁移（`config_state.rs:241-266`），两者语义与触发时机都不同，不可互相套用。

## Data Flow

```text
                    ┌── 磁盘 ─────────────────────────────────────────────┐
                    │ 桌面 config.json (Config JSON, password=DPAPI 密文) │
                    │ 安卓 config.json (EncodedSettings, 密文外置两位)     │
                    │ <data>/accounts/<name>.json  （桌面 Config / 安卓 EncodedSettings）│
                    │ <data>/login-history.json    （两端同格式，上限 100）  │
                    └─────────────────────────────────────────────────────┘
                          │ 读                            ↑ 写
                          ↓                                │
  桌面 load_config_from_file → crypto::decrypt → validate_config_lenient → state.config.store
  安卓 load_file → bridge.decrypt → migrate_legacy_defaults → AndroidState.config 缓存
                          │
                          ↓
                   内存明文态（state.config / Settings）
                          │
   ┌──────────────────────┴────────────────────────┐
   │ 出站（读）                                      │ 写入
   ↓                                                ↓
masked_for_display()                       桌面 save_config_to_disk_encrypted
  → get_config / get_init_data               ├─ crypto::encrypt → persist::atomic_write
  → account 三命令返回                        └─ notify_config_changed(masked)
  → config-changed 事件 payload                   → 前端 mergeConfigFromBackend（跳过 dirtyFields）
                                            安卓 save_to（持 CONFIG_IO_LOCK）
                                              └─ 无事件，靠命令返回值
   ↓
  前端 useConfigStore（dirtyFields / 500ms debounce / DIRTY_FAILURE_LIMIT=3）
```

## Connections

- [[desktop-config]] — `config/` 三文件的逐函数详解
- [[security-model]] — 掩码出口、DPAPI 加密、`clear` 标志语义
- [[android-backend]] — `config_state.rs` 的 Keystore 桥与迁移逻辑
- [[ipc-command-surface]] — `save_config` / `get_config` / `config-changed` 的通道侧
- [[desktop-frontend-hooks]] — `useConfigStore` 的脏字段合并与 debounce 保存
- [[dual-platform-sharing]] — 字段同步约定与两端配置面差异
- [[background-check-and-auto-login]] — 间隔 / 阈值 / 时段字段的消费方
- [[desktop-account-selfservice]] — 账号存储与自助服务密码的读取路径

## Known Issues

- **两端字段集不是子集关系**：交集 31 个字段，桌面独有 13 个（双适配器三件、托盘/隐藏启动、退出策略、`campus_exit_*` 三件、`skip_sha256_when_missing`、`config_version`），安卓独有 4 个（`allow_2d_face_verify` / `background_check_idle_interval` / `enable_boot_autostart` / `config_schema_version`）。这意味着"双端同步"实际是双向增量同步，新增字段时无法照抄某一端。
- **`config_version` 与 `config_schema_version` 是两套编号**：桌面是 2（`model.rs:208`），安卓是 4（`config_state.rs:120`），两者语义不同却名字相近，容易被误当同一版本号维护。
- **安卓 tmp 文件名固定且无 fsync**：`config_state.rs:219-221` 用固定 `.json.tmp` 且不做 `sync_all` + rename 重试；`login_history.rs:72-74` 同样。并发保护完全依赖调用方持有 `CONFIG_IO_LOCK`，任何新增写路径忘记加锁就会互相覆盖。
- **桌面 `atomic_write` 用纳秒时间戳生成 tmp 名**（`persist.rs:14-20`）：极端情况下同纳秒并发仍可能撞名，且每分钟残留的 tmp 文件无清理逻辑。
- **安卓配置无校验层**：`Settings` 没有任何 `validate_*` 对应物（桌面有 `config/validate.rs` 677 行），`save_config`（`config_state.rs:311-338`）直接落盘，非法 `portal_url` / `theme_mode` / `custom_theme_color` 可被写入并持久化。
- **`Config` 的字段级 `default` 与容器级 `default` 混用**：`#[serde(default)]` 在容器上（`model.rs:9`）+ 部分字段又有 `default = "fn"`。前者让"缺字段"走 `Default::default()`，后者让"缺字段"走指定函数——两者对同一字段同时存在时以字段级为准，阅读时容易误判某字段的真实兜底值（如 `background_check_interval` 缺字段时是 `15000` 而非 0）。
- **安卓 `default_panel` 默认值与桌面不同**：安卓 `"dashboard"`（`config_state.rs:97`），桌面为空串（`model.rs:189`）——空串时面板选择逻辑落到前端默认值，两端首屏面板因此可能不一致。
- **登录历史写入无 fsync 且桌面用 `Mutex<()>` 静态锁**（`persist.rs:10`）：多进程（如桌面 `--helper` 提权子进程）并发写同一文件时锁无效。
