---
title: 安卓端配置与账号文件的并发写保护（IO锁）
type: learning
source_files:
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/account_cmds.rs
tags: [教训, 配置, 并发, 锁]
---

## Overview

安卓端（`android/src-tauri`）的主配置文件 `config.json` 与多账号档案 `accounts/*.json` 共用同一套磁盘形态 `EncodedSettings`（`settings` 明文结构 + `password_cipher` 密文位，`android/src-tauri/src/config_state.rs:242-248`），仅目录不同（`android/src-tauri/src/account_cmds.rs:770-771`）。两类文件的并发读-改-写由**一把全局串行化 IO 锁**统一保护。

锁本体 `CONFIG_IO_LOCK` 是 `lazy_static` 的 `tokio::sync::Mutex<()>`，定义在 `android/src-tauri/src/config_state.rs:17-19`。它**自 `account_cmds.rs` 迁入**：`save_config` 自身也要持锁，锁定义在锁的使用者模块里会造成单向依赖扭曲（`android/src-tauri/src/config_state.rs:17-20`）；`account_cmds.rs:18-23` 保留同名转发函数，`monitor_loop` 等既有调用点不受影响。

本篇记录的核心教训：`save_config` 曾是唯一不经锁的配置写路径，与账号命令并发时产生「`save_config` 读旧配置 → 账号命令落盘 → `save_config` 用旧基线覆盖」的丢更新竞态（`android/src-tauri/src/config_state.rs:474-476`）。修复后 `save_config` 全程持锁，guard 跨 `await` 持有，覆盖 `load → merge → save` 全序列（`android/src-tauri/src/config_state.rs:13`）。

## Key Components

### android/src-tauri/src/config_state.rs

#### 全局 IO 锁

- `CONFIG_IO_LOCK`（`android/src-tauri/src/config_state.rs:17-19`）：`lazy_static::lazy_static!` 静态 `tokio::sync::Mutex<()>`，串行化主配置与账号文件的全部读改写。
- `pub async fn config_io_lock() -> tokio::sync::MutexGuard<'static, ()>`（`android/src-tauri/src/config_state.rs:21-24`）：获取锁的唯一入口，doc 注释明确「主配置/账号文件读改写互斥(跨命令共用;account_cmds::config_io_lock 转发至此)」。

#### 常量

- `PASSWORD_MASK: &str = "***"`（`android/src-tauri/src/config_state.rs:10`）：密码出站掩码。
- `CONFIG_FILE: &str = "config.json"`（`android/src-tauri/src/config_state.rs:11`）：主配置文件名。

#### 结构体

- `Settings`（`android/src-tauri/src/config_state.rs:26-128`）：主配置结构，`#[serde(rename_all = "camelCase", default)]`（camelCase IPC 契约 + 容器级缺字段兜底）。全部字段：
  - 账号与凭据：`user`(30)、`password`(31，落盘必经加密)、`self_hello_enabled`(32)、`self_reverify_each_action`(33)、`allow_2d_face_verify`(35，2D 人脸验证开关默认关)、`operator`(36)
  - 夜间运营商/出站切换：`enable_night_operator_switch`(40)、`night_operator_restore`(42，切换前原运营商暂存)、`enable_night_outbound_switch`(46)、`outbound_priority`(49，安卓不消费)、`outbound_metric_restore`(52，安卓不消费)、`outbound_disabled_adapters`(55，安卓不消费)、`outbound_standby_route`(58，安卓不消费)、`night_outbound_restore`(63，安卓出站切换态标记，值恒 `"logged_out"`)、`dns_optimize_adapters`(66，桌面专属)
  - 设备别名：`device_aliases`(72，`#[serde(default)]`，键=12 位 hex MAC)
  - 行为：`auto_login_on_start`(74)、`enable_background_check`(75)、`background_check_interval`(76)、`background_check_idle_interval`(81，闲时巡检间隔)、`auto_login_on_preparation`(82)、`max_disconnect_reconnect`(83)、`auto_login_cooldown_secs`(84)
  - 界面与通知：`theme_mode`(86)、`enable_notification`(87)、`custom_theme_color`(88)、`default_panel`(89)、`display_name`(93)、`active_account`(94)、`enable_boot_autostart`(95)
  - 定时质量测试：`enable_latency_test`(97)、`latency_test_interval`(98)、`enable_network_quality`(99)、`skip_ttfb_in_latency`(100)、`skip_content_in_latency`(101)
  - 网络：`portal_url`(103)、`fixed_gateway`(104)、`required_network_name`(105)、`enable_network_name_check`(106)、`campus_gateway`(107)、`campus_check_start_minutes`(109)、`campus_check_end_minutes`(111)、`scheduled_login_minutes`(113)、`scheduled_logout_minutes`(115)
  - 更新/日志/版本：`update_source`(118)、`log_retention_days`(120)、`config_schema_version`(127，迁移判据，新装即 7)
- `impl Default for Settings`（`android/src-tauri/src/config_state.rs:130-200`）：全字段默认值。关键项：`config_schema_version: 7`(197，新装即当前版本跳过迁移)、`background_check_interval: 60_000`(155)、`background_check_idle_interval: 300_000`(157)、`latency_test_interval: 600_000`(170)、`required_network_name: "i-wxxy、iwxxy-2、iwxxy-3"`(181，2026-10-01 对齐桌面三 SSID)、`campus_check_end_minutes: 1380`(188)、`scheduled_login_minutes`/`scheduled_logout_minutes: 1440`(192/193，禁用哨兵)、`enable_night_operator_switch: false`(141，2026-10-02 改回关闭)。
- `CryptoBridge`（`android/src-tauri/src/config_state.rs:213-251`）：加解密桥。字段 `encrypt`/`decrypt`(205-206，均为 `Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>`)。方法 `from_app(app)`(210-240)：`#[cfg(mobile)]` 分支走 `tauri_plugin_campus_keystore`(213-229)，`#[cfg(not(mobile))]` 分支返回恒 Err 的占位实现「Keystore 仅移动端可用」(231-238)；host 测试另注入可逆假实现。
- `EncodedSettings`（`android/src-tauri/src/config_state.rs:242-248`）：磁盘形态。字段 `settings: Settings`(246) 与 `password_cipher: String`(247)；`save_file` 落盘时刻意把外露 `Settings.password` 清空、只留密文位。

#### 文件 IO 与迁移

- `pub async fn load_file(path, bridge)`（`android/src-tauri/src/config_state.rs:260-273`）：读文件并解密密码；无文件返回 `Settings::default()`(252-253，首次启动)；密文损坏（密钥变更/文件篡改）时 `unwrap_or_default()` 置空而不是让配置不可用(259-260)。
- `pub fn password_saved_at(path)`（`android/src-tauri/src/config_state.rs:265-276`）：账号卡「密码是否已保存」判据。必须落在 disk 形态的密文位 `passwordCipher` 上，不能看外露 `settings.password`——落盘时该字段被刻意清空，取它会永远得到「未保存」（265-269 注释，用户真机反馈）；读取/解析失败返回 `false`。
- `pub async fn save_file(path, bridge, s)`（`android/src-tauri/src/config_state.rs:278-308`）：加密落盘。密码非空则经 `CryptoBridge.encrypt`，成败打 stderr 且不打印明文/密文，失败直接 `Err` 不落盘(284-291)；构造 `EncodedSettings` 时清空外露 password(292-295)；**临时文件名带 pid**(300-304)，避免进程被杀后遗留的同名 tmp 在新一轮 rename 时顶替新写入(297-299 注释)；`write tmp → rename` 原子替换(305-306)。
- `pub async fn cleanup_stale_config_tmp(dir)`（`android/src-tauri/src/config_state.rs:310-324`）：启动时清理崩溃遗留的 `config.json.<pid>.tmp`；能活过一次重启的 tmp 一定是孤儿，不区分 pid 全清(310-312 注释)。
- `pub async fn load_from(dir, bridge)`（`android/src-tauri/src/config_state.rs:326-336`）：`load_file` + `migrate_legacy_defaults` + 旧默认单值 SSID「i-wxxy」扩为三值(332-334，与桌面 validate 2026-09-27 同语义，幂等)。
- `async fn migrate_legacy_defaults(dir, bridge, s)`（`android/src-tauri/src/config_state.rs:338-417`，私有）：旧默认一次性迁移 v0→v7，逐档落盘、失败仅 `log_warn` 下次读盘重迁（幂等）。v0→v1 后台间隔 15s→60s；v1→v2 自动化/验证/跳过项开关刷开；v2→v3 质量检测关；v3→v4 补闲时间隔 300_000；v4→v5 检测终点 0→1380；v5→v6 质量间隔 60s→600s；v6→v7 夜切刷开 + 定时禁用 0→1440。
- `pub async fn save_to(dir, bridge, s)`（`android/src-tauri/src/config_state.rs:419-421`）：`save_file(&dir.join(CONFIG_FILE), ...)` 的目录级封装。

#### 出站掩码与事件广播

- `pub fn masked_for_display(s)`（`android/src-tauri/src/config_state.rs:423-430`）：非空密码 → `"***"`，对齐桌面唯一出口语义；敏感信息出站不得另写序列化。
- `pub async fn emit_config_changed(app, s)`（`android/src-tauri/src/config_state.rs:432-439`）：广播 `config-changed`，payload 为 `{ "config": 掩码后配置 }`(438)。不广播则前端 store 停留旧快照、后续任意保存会把后端刚落盘的值覆盖回去(432-435 注释)。
- `pub fn resolve_password_field(incoming, current, clear)`（`android/src-tauri/src/config_state.rs:441-451`）：密码字段回退语义，与桌面 `save_config` 同构——`clear=true` 置空(443-445)；空串或 `"***"` 视为未修改回退已存值(446-447)；否则采用新值(448-449)。

#### Tauri 命令与缓存读取

- `#[tauri::command] pub async fn get_config(app)`（`android/src-tauri/src/config_state.rs:453-466`）：读配置（不持锁，见 Known Issues）、刷新 `AndroidState.config` 缓存(461-464)、返回掩码值(465)。
- `#[tauri::command] pub async fn save_config(app, config, clear_password)`（`android/src-tauri/src/config_state.rs:494-525`）：**本篇主角**。函数体第一步取 `config_io_lock`(477)，锁注释 474-476 说明防丢更新竞态动机与教训原文「此前 save_config 是唯一不经锁的配置写路径」；随后 `load_from`(483) → `resolve_password_field` 合并密码(484-486) → `save_to`(487) → 刷新缓存(488-491) → `auto_create_account_for_current`(492-494，仍在锁内) → `emit_config_changed`(495-497)。函数返回时 guard drop 释放锁。
- `pub async fn current_settings(app)`（`android/src-tauri/src/config_state.rs:501-520`）：缓存优先读取当前配置，未命中读盘并回填缓存(516-518)；带 `#[allow(dead_code)]`(502，注释「Task 2 协议命令面接线」)。供登录/自助服务等命令回退已存凭据。

#### 测试（`#[cfg(test)] mod tests`，`android/src-tauri/src/config_state.rs:522-827`）

与写路径/密码安全直接相关的用例：

- `加密落盘_文件不含明文_读回等于原文`(587-598)：断言磁盘文件不含明文密码。
- `解密失败时置空_配置仍可加载`(600-614)：换密钥模拟 Keystore 失效，password 置空、非敏感字段不受影响。
- `空密码不写密文位`(793-804)：空密码不得产生密文。
- `密码已保存判据_落盘后仍为已保存`(809-826)：判据必须取密文位（外露 `settings.password` 恒空）。
- `掩码出口_非空变星号_空保持空`(768-777)、`密码字段回退语义_与桌面同构`(779-791)。
- 迁移与默认值系列：`默认值_业务语义`(632-664)、`迁移_旧默认15s升60s且落盘后用户值不被覆盖`(667-711)、`迁移_非旧默认值保持不动`(714-725)、`迁移_v5质量间隔旧默认刷v6且用户值不被覆盖`(728-756)、`旧默认单值SSID_载入时扩为三值`(617-630)、`camelCase_序列化契约`(759-766)。

### android/src-tauri/src/account_cmds.rs

#### 锁转发

- `pub async fn config_io_lock() -> tokio::sync::MutexGuard<'static, ()>`（`android/src-tauri/src/account_cmds.rs:17-22`）：纯转发到 `config_state::config_io_lock`(21)；保留它是为了既有调用点（`monitor_loop` 等）不变(17-19 注释)。
- `ACCOUNT_NAME_RE`(12-15)：账号名白名单正则 `^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$`。

#### 结构体

- `AccountItem`（`android/src-tauri/src/account_cmds.rs:27-40`）：账号列表条目。字段 `id`(29，文件名 stem 稳定不变)、`display_name`(31，serde rename `displayName`)、`user`(35)、`operator`(37)、`password_saved`(39，serde rename `passwordSaved`；密码只出布尔，明文/密文均不出站)。
- `AccountResult`（`android/src-tauri/src/account_cmds.rs:657-682`）：账号操作统一返回。字段 `success`(45)、`message`(47)、`active_account`(49)、`display_name`(52，rename_account 成功时携带 trim 后新名)、`config`(54)；构造器 `ok`(58-60)、`ok_with_account`(61-63)、`err`(64-66)。

#### 校验与消毒

- `pub fn sanitize_account_id(user)`（`android/src-tauri/src/account_cmds.rs:69-83`）：白名单外字符替换为 `_`、按字符截断 32 位(81)；空输入 → 空输出（调用方跳过建号）。
- `pub fn validate_account_name(name)`（`android/src-tauri/src/account_cmds.rs:85-94`）：1-32 字符、仅字母/数字/下划线/中文/连字符，防路径穿越。
- `fn validate_display_name(name)`（`android/src-tauri/src/account_cmds.rs:431-447`，私有）：trim 后 1..=32 字符、禁控制字符，允许空格与 emoji；不做文件名字符集校验（显示名与内部 id 分离）。

#### 目录与列举

- `pub fn accounts_dir(app)`（`android/src-tauri/src/account_cmds.rs:96-102`）：`app_data_dir()/accounts`。
- `fn read_account_card_info(path)`（`android/src-tauri/src/account_cmds.rs:104-121`，私有）：不解密读取展示字段（列举路径无需走 Keystore）；`password_saved` 必须走 `config_state::password_saved_at`(119)。
- `pub fn list_account_items_sync(dir)`（`android/src-tauri/src/account_cmds.rs:123-155`）：按 id 排序、过滤隐藏与非 json、空显示名兜底为 id(142)、读取失败 log_warn 后整条兜底(145-148)。
- `#[tauri::command] pub async fn list_accounts(app)`（`android/src-tauri/src/account_cmds.rs:160-166`）：`spawn_blocking` 包装同步列举(160)。
- `async fn load_account_file(path, bridge)`（`android/src-tauri/src/account_cmds.rs:165-170`，私有）：文件不存在返回 `Ok(None)`。
- `async fn persist_current(app, merged)`（`android/src-tauri/src/account_cmds.rs:172-183`，私有）：`config_state::save_to`(178) + 缓存刷新(179-181) + 返回掩码配置(182)。

#### Tauri 命令（写命令全部持锁）

| 命令 | 定义区间 | 取锁位置 | 要点 |
| --- | --- | --- | --- |
| `switch_account` | `account_cmds.rs:186-227` | 198 | 合并 `user/password/operator` 到主配置(206-208)；清 `night_operator_restore`(211，防 A 账号运营商恢复到 B 账号) 与 `night_outbound_restore`(214，残留标记会让监控巡检整轮跳过)；`persist_current`(222) 后收尾 avoid_bad_wifi 快照(225) |
| `save_current_as_account` | `account_cmds.rs:230-257` | 242 | `merge_save_as_target` 合并(248)后 `save_file`(249)；主配置同步 `active_account`/`display_name` 防串号(252-255) |
| `create_account` | `account_cmds.rs:260-294` | 278 | 重名拒绝(279-281)；新建仅登录字段+显示名，不切换、不动主配置(283-291) |
| `delete_account` | `account_cmds.rs:334-362` | 345 | 删文件(346-348)；删的是当前账号则清 `active_account`(352) 与 `night_outbound_restore`(355)，并收尾 avoid_bad_wifi 快照(358) |
| `get_active_account` | `account_cmds.rs:365-367` | — | 只读（`current_settings` 缓存优先），不取 IO 锁 |
| `rename_account` | `account_cmds.rs:370-401` | 385 | 核心 `rename_account_core`(386)；被改名账号是激活账号时同步主配置 `display_name`(391-393)；业务错误走 IPC `Err(String)`(375-380 注释，与 switch 的 `AccountResult::err` 通道不同) |

#### 纯函数与自动建号

- `fn merge_save_as_target(current, existing, safe_name)`（`android/src-tauri/src/account_cmds.rs:296-331`，私有）：另存合并规则。另存场景（`current.active_account != safe_name`，307）落盘副本 `display_name` 置空防串号(310-312)；目标档案已存在则保留其非登录字段、登录字段用当前配置覆盖(314-323)；主配置最终显示名档案非空优先、否则兜底 id(325-329)。
- `async fn rename_account_core(dir, bridge, account_id, display_name)`（`android/src-tauri/src/account_cmds.rs:403-429`，私有）：校验显示名(411) → 存在性(413-415) → 重名检查（按 id 排除自身，420-424）→ 只改 `displayName` 写回(426-427)，其余字段一律保留。
- `pub async fn auto_create_account_for_current(app, current)`（`android/src-tauri/src/account_cmds.rs:449-460`）：R2 自动建号入口。**不经命令层**（避免递归）、失败仅 `log_warn` 不让保存失败(449-450 注释)。**自身不持锁**，依赖调用方 `save_config` 已持锁（`config_state.rs:492-494`）。
- `async fn auto_create_account_in(dir, bridge, config)`（`android/src-tauri/src/account_cmds.rs:462-510`，私有）：user/password 空 → 跳过(478-480)；`id = sanitize_account_id(user)`，为空 → 跳过(481-484)；档案不存在 → 以当前配置快照建号、`display_name` = 原始 user 文本(486-491)；已存在且 user 原文不同 → 撞库，跳过并告警、绝不覆盖他人凭据(493-501)；user 一致且 password/operator 相同 → 幂等短路不写盘(502-504)；有差异 → 只更新 user/password/operator 三字段、保留自定义显示名(505-509)。

#### 测试（`#[cfg(test)] mod tests`，`android/src-tauri/src/account_cmds.rs:512-838`）

覆盖：账号名消毒(517-537)、列举兜底(539-559)、id 消毒(588-601)、显示名校验(603-618)、自动建号（无凭据跳过 620-629、新号 631-651、幂等不写盘 653-673、差异更新保留显示名 675-698、撞库 817-837）、改名核心（成功 700-719、拒绝 721-757）、IPC JSON 形状(759-778)、另存显示名不串号(780-815)。假加密桥 `test_bridge`(566-571) 可逆且输出带前缀非明文。

## Architecture

```text
┌─────────────────────────────────────────────────────────┐
│              Tauri 命令层（并发写入口）                   │
│ save_config(config_state.rs:469) / switch_account /     │
│ save_current_as_account / create_account /              │
│ delete_account / rename_account(account_cmds.rs)        │
└───────────────────────┬─────────────────────────────────┘
                        │ ① 全部先获取同一把全局串行锁
                        ▼
┌─────────────────────────────────────────────────────────┐
│ CONFIG_IO_LOCK: tokio::sync::Mutex<()> (lazy_static)    │
│ config_state.rs:17-19 定义 · account_cmds.rs:20-22 转发 │
└───────────────────────┬─────────────────────────────────┘
                        │ ② guard 跨 await 持有（覆盖全序列）
                        ▼
┌─────────────────────────────────────────────────────────┐
│ load_from(解密) → merge(resolve_password_field) →       │
│ save_file(Keystore 加密 · 带 pid tmp · rename 原子落盘) │
│ → auto_create_account_for_current → emit_config_changed│
└─────────────────────────────────────────────────────────┘
```

锁是**单实例粗粒度串行**：主配置与全部账号档案共用一把锁，属于写频率低、正确性优先的取舍（`android/src-tauri/src/config_state.rs:21`「主配置/账号文件读改写互斥(跨命令共用)」）。

## Data Flow

1. **写入流程（`save_config`，全程持锁，`android/src-tauri/src/config_state.rs:468-499`）**：
   1. `let _io_guard = config_io_lock().await;`(477)——此后直到函数返回，其他持锁命令全部等待。
   2. `load_from` 读当前配置(483)。
   3. `resolve_password_field` 合并密码字段(484-486)：空串/掩码回退已存值，`clear_password == Some(true)` 显式置空。
   4. `save_to` 加密原子落盘(487)。
   5. 刷新 `AndroidState.config` 内存缓存(488-491)。
   6. `auto_create_account_for_current` 同步账号档案(492-494，仍在锁内)。
   7. `emit_config_changed` 广播(495-497)；返回，guard drop 释放锁。

2. **账号切换（`switch_account`，`android/src-tauri/src/account_cmds.rs:186-227`）**：校验账号名(190-193) → 取锁(198) → `load_account_file` 读目标档案，不存在则业务失败(199-201) → `current_settings` 取当前配置并合并凭据(205-208) → 清空 `night_operator_restore`(211) 与 `night_outbound_restore`(214) → `display_name` 兜底(215-219) → `active_account = safe_name`(220) → `persist_current`(222) → `restore_avoid_bad_wifi`(225) → 释放锁。

3. **另存/新建/删除/改名**：取锁点分别为 `account_cmds.rs:242 / 278 / 345 / 385`；各自内部走 `save_file`/`persist_current`，锁横跨整个读改写序列。

4. **自动建号（锁范围的特例）**：`save_config` 在锁内调用 `auto_create_account_for_current`（`config_state.rs:494-496`），该函数与核心 `auto_create_account_in`（`account_cmds.rs:451-510`）都**不再取锁**——对同一把 `tokio::sync::Mutex` 重复 `.lock().await` 会死锁，且它只调底层 `save_file`、不经命令层（无递归风险，`account_cmds.rs:472`）。这是「锁内调 helper、helper 不重入」的隐式约定。

5. **读路径（不持锁）**：`get_config`（`config_state.rs:454-467`）与 `current_settings`(501-520) 直接读盘/读缓存。单次读的完整性由 `save_file` 的 tmp+rename 原子替换(305-306)保证，不会读到半成品。

## Connections

- [[mask-placeholder-persisted-as-plaintext]] —— 密码掩码占位符与落盘明文风险的配套语义（`resolve_password_field` / `masked_for_display`）。
- [[config-missing-field-load-failure]] —— 配置缺字段/损坏时的加载行为（容器级 `serde(default)` 与首次启动默认）。
- [[dual-platform-sharing]] —— 本文多处「与桌面同构/同语义」的双端共享背景。

## Known Issues

- **锁纪律完全依赖调用方自觉**：模块没有任何机制阻止绕过锁直接调用 `save_file`/`save_to` 写盘。核心教训（`android/src-tauri/src/config_state.rs:474-476` 注释原话）：**此前 `save_config` 是唯一不经锁的配置写路径**，与账号命令并发时出现「`save_config` 读旧配置 → 账号命令落盘 → `save_config` 用旧基线覆盖」的丢更新。新增任何写 `config.json` 或 `accounts/*.json` 的路径，第一步必须取 `config_io_lock`。
- **`auto_create_account_for_current` 的锁范围是隐式契约**：它自身不持锁（`android/src-tauri/src/account_cmds.rs:449-450`），安全性完全依赖「在 `save_config` 的锁语境中被调用」（`config_state.rs:494-496`）。未来若有其他裸路径直接调用它，要么先取锁、要么重新评估并发安全；反之若它在持锁语境外被调用后再取锁，需注意与 `save_config` 的锁内调用互斥。
- **读路径不持锁**：`get_config`（`config_state.rs:454-466`）与 `current_settings`(503-520) 不取 IO 锁。完整性靠 rename 原子替换(305-306)；但读到的可能是任一「已完成落盘」的快照，且两者都会写 `AndroidState.config` 缓存(461-464、516-518)——并发写之后若读先于写完成，缓存可能短暂滞留旧值，直到下一次写路径刷新。
- **锁作用域是单进程**：`CONFIG_IO_LOCK` 是进程内 `lazy_static` 的 `tokio::sync::Mutex`（`config_state.rs:17-19`），不提供跨进程互斥；在安卓单进程应用形态下成立。
- **锁本体位置的历史包袱**：锁曾定义在 `account_cmds.rs`，因 `save_config` 也要持锁而迁至 `config_state.rs`（`config_state.rs:12-15`）；`account_cmds.rs:17-22` 的转发函数仅为兼容既有调用点（`monitor_loop` 等）保留——新代码应直接使用 `config_state::config_io_lock`。