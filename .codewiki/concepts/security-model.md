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
  - tauri-app/src-tauri/src/commands/system.rs
  - tauri-app/src-tauri/src/commands/self_service.rs
  - tauri-app/src-tauri/src/commands/account.rs
  - tauri-app/src-tauri/src/account/crypto.rs
  - tauri-app/src-tauri/src/platform/identity.rs
  - tauri-app/src-tauri/src/auth/portal.rs
  - tauri-app/src-tauri/src/auth/protocol.rs
  - tauri-app/src-tauri/src/infra/events.rs
  - tauri-app/src-tauri/src/infra/logger.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/identity_gate.rs
  - android/src-tauri/src/self_service_cmds.rs
  - android/src-tauri/src/account_cmds.rs
  - android/src-tauri/src/login_history.rs
  - android/plugins/keystore/src/lib.rs
  - android/plugins/keystore/android/src/main/java/com/campuslogin/plugin/keystore/KeystorePlugin.kt
---

# 安全体系（凭据出站、加密与验证门）

## Overview

安全体系围绕三条主线展开，桌面（Tauri + Windows）与安卓（Tauri + AndroidKeyStore）双端同构：

1. **凭据出站唯一出口**：任何把配置发往前端/日志的路径，密码字段非空一律替换为 `"***"`（`PASSWORD_MASK`，桌面 `config/model.rs:3`、安卓 `config_state.rs:10`）。桌面收敛点为 `Config::masked_for_display`（`config/model.rs:315-319`），安卓收敛点为 `masked_for_display(&Settings) -> serde_json::Value`（`config_state.rs:372-382`）。
2. **落盘加密**：密码写入磁盘前必经加密——桌面 DPAPI（当前用户作用域，`account/crypto.rs:75-111`），安卓 AndroidKeyStore AES-256-GCM（`KeystorePlugin.kt:39-89`）。"非空即加密，不排除掩码占位符"（`config/persist.rs:278-281`）。
3. **后端验证门**：敏感自助操作前要求本机身份验证，TTL 600 秒、时间戳只存后端内存（桌面 `platform/identity.rs:28-31`，安卓 `identity_gate.rs:6-8`）。`reveal_operator_credential` 无论总开关如何都强制设门（桌面 `commands/self_service.rs:300-304`，安卓 `self_service_cmds.rs:257-259`）。

配套机制：登录请求错误日志脱敏 `redact_credentials`（`auth/portal.rs:67-81`）；状态探测是只读操作、绝不携带密码（`portal.rs:125-143`）；登录历史不含密码字段（桌面 `persist.rs:160-212`、安卓 `login_history.rs:51-76` 且测试锁定 `:139`）；账号 id 白名单防路径穿越（桌面 `commands/account.rs:620-632`，安卓 `account_cmds.rs:79-87`）。

### 术语与不变量

| 术语 | 含义 | 不变量 |
| --- | --- | --- |
| `PASSWORD_MASK` | 出站掩码占位符 `"***"` | 用户真实密码恰为 `***` 时不得排除加密（`persist.rs:278-281`） |
| `masked_for_display` | 敏感信息出站唯一出口 | 空→空（未设置语义），非空→`***`（`model.rs:312-319`） |
| `EncodedSettings` | 安卓密文容器 | `passwordCipher`/`selfPasswordCipher` 之外体内字段置空，settings 体内不含明文（`config_state.rs:226-232`、`:274`） |
| `IDENTITY_VERIFY_TTL_SECS` | 验证门有效期 600s | 时间戳 0=从未验证；时钟回拨视为过期（`identity.rs:45-47`、`identity_gate.rs:29-31`） |
| `redact_credentials` | 日志/错误脱敏 | URL 与明文密码均替换为 `***`（`portal.rs:70-80`） |
| `validate_account_name` | 账号 id 白名单 | 拒绝 `../`、`/`、`\` 等路径穿越字符（`account_cmds.rs:79-87`） |

## 机制说明（Key Components）

### 1. 掩码与出站唯一出口

桌面 `Config::masked_for_display`（`config/model.rs:315-319`，clone 后调 `mask_in_place`）是约定上的唯一出口，doc 注释（`model.rs:312-314`）明确"漏一个字段就是一次明文泄露，account 三命令即前车之鉴"。`mask_in_place`（`model.rs:321-328`）对 `password` 与 `self_password` 非空 → `"***"`。

实际出站调用点（已逐一核对）：

| 出站路径 | 掩码调用点 |
| --- | --- |
| `get_config` | `commands/config_cmd.rs:217` |
| 配置变更广播 | `config_cmd.rs:17-18`（`save_config_to_disk_encrypted` 内统一掩码 + `notify_config_changed`，事件体 `{"config": …}` 包裹） |
| `get_init_data` | `commands/system.rs:117`（注释 `:114-116` 记 2026-09-06 真机缺陷：漏掩码 → 前端 `selfPasswordSaved` 永为 false → 重启后密码框空、自动 Hello 永不触发） |
| 配置导出（不含密码态） | `config_cmd.rs:103` |
| 诊断包 | `system.rs:239-243`（`config-masked.json`） |
| 账号命令出站 | `account.rs:28`（switch）、`:181-183`（save_as）、`:252-253`（delete）、`:309-311`（rename） |
| 自助查询出站 | `self_service.rs:132-140`（`query_bind_status` 只回 `masked_account` + `password_set`，不回密码本身） |

安卓出口收敛为 `serde_json::Value`（`config_state.rs:372-382`），调用点：`get_config`（`config_state.rs:417`）、`emit_config_changed`（`:384-391`，注释 `:384-387` 要求"掩码必须复用 masked_for_display，不得另写序列化"）、账号命令出站（`account_cmds.rs:161`、`:303`、`:338`）。事件通道 `events.rs:107-109` 的 `emit_config_changed` 对 payload 原样透传——掩码责任在调用方。

### 2. 密码回退与清除语义

三态输入（明文 / `***` / 空）+ `clear` 开关，双端同构：

- 桌面 `save_config`（`config_cmd.rs:220-276`）：`clear_password==Some(true)` → 置空（`:238-239`）；空或 `***` → 保留已存值（`:240-244`）；`self_password` 同构（`:247-252`）。`resolve_self_password`（`commands/self_service.rs:47-61`）同语义：非空非掩码用前端传入，否则回退已存值，皆无 → `None`。
- 安卓 `resolve_password_field`（`config_state.rs:393-403`）：clear → 空、空/掩码 → 已存、否则新值；`save_config`（`config_state.rs:420-453`）在 `:434-440` 对两个字段执行 resolve。
- 导入还原 `restore_imported_password_field`（`config_cmd.rs:115-131`）：空/掩码 → 保留本机当前值（`:122-125`）；密文 → 解密还原，失败报错 `"密码密文解密失败（含密码导出仅限本机导入）: {e}"`（`:126-129`）；doc（`:115-120`）引用 learnings/mask-placeholder-persisted-as-plaintext——MASK 占位符不得直接落盘。
- 解密失败宽容：桌面 `load_config_from_file` 解密失败清空该字段保留其他配置（`config_cmd.rs:36-45`，条件 `!empty && != PASSWORD_MASK`）；安卓 `load_file` 解密失败 `unwrap_or_default` 置空（`config_state.rs:243-248`，测试 `:557-570`）。
- 密码校验 `validate_password`（`config/validate.rs:31-39`）：仅要求非空且 ≤128 字符，无复杂度要求；掩码密码跳过校验（`validate.rs:92`、`:199`，测试 `validate_config_skips_password_when_masked` `:734`）。

### 3. 落盘加密

**桌面 DPAPI**（`account/crypto.rs`）：`CryptProtectData/CryptUnprotectData`（`:11-32`，`#[link(name="crypt32")]`，flags 传 0 = 当前用户作用域 `:85`/`:104`），`LocalFree` 释放缓冲（`:34-37`），`call_dpapi` 失败路径判空释放（`:57-64`）。对外 `encrypt`（`:114-122`，base64 STANDARD）/`decrypt`（`:124-130`）；非 Windows 桩返回 Err（`:133-141`）。

- 主配置 `save_config_to_disk_encrypted`（`config/persist.rs:276-291`）：注释（`:278-281`）明确"任何非空密码一律 DPAPI 加密落盘。注意：不得排除 `PASSWORD_MASK("***")`——若用户真实密码恰为 `***`，排除判断会使其明文落盘"；`password` 加密 `:282-284`、`self_password` `:285-287`，经 `atomic_write`（`:12-43`，临时文件 + rename 重试）落盘。
- 账号档案 `save_account_config`（`persist.rs:98-112`）：仅加密 `password`（`:105-108`），`self_password` 透传（Known Issues 1）。
- 测试：掩码字面量必须加密 `persist.rs:304-320`、空密码保持空 `:322-333`、roundtrip `:434-456`；导出载荷无明文 `config_cmd.rs:308-334`；导入还原语义 `config_cmd.rs:338-363`。

**安卓 AndroidKeyStore**（`KeystorePlugin.kt`）：密钥别名 `campus_login_master`（`:34`），AES/GCM/NoPadding、256 位（`:39-54`），未设置 `setUserAuthenticationRequired`——密钥不绑定生物识别；`encrypt` 输出 `base64(iv + ciphertext)`（`:56-71`），`decrypt` 校验长度并拒绝 GCM 认证失败（`:73-89`）。Rust 侧经 `CryptoBridge`（`config_state.rs:186-223`）接入插件（`plugins/keystore/src/lib.rs:42-50` 的 `CampusKeystoreExt`，注册名 `com.campuslogin.plugin.keystore` `:11-12`）。

- `save_file`（`config_state.rs:252-283`）：`password`/`self_password` 分别加密（`:261-272`），**加密失败 → Err 拒绝落盘**（`:260` 的 eprintln 不含明文）；`EncodedSettings`（`:226-232`）体内两密码字段置空（`:274`），密文放独立字段。
- 迁移路径 `migrate_legacy_defaults`（`config_state.rs:314-366`）落盘同样经 `save_file` 加密。
- 测试：落盘文件不含明文（`config_state.rs:541-555`，断言 `!raw.contains("secret_pass")`/`"self_secret"`）、空密码不写密文位（`:752-764`）。

### 4. 后端验证门（TTL 600s）

桌面 `platform/identity.rs`：模块 doc（`:1-17`）记录决策——仅 Windows Hello、不回退 CredUI（`:3-5`，2026-09-05）；Win11 interop 主路径（`:97-102`，`RequestVerificationForWindowAsync` `:139`）、Win10 兜底轮询（`:104-116`）；非阻塞 `SetCompleted` + oneshot（`:176-204`）。状态：`LAST_VERIFY_EPOCH_SECS`（`:28`，0=从未验证）、`IDENTITY_VERIFY_TTL_SECS = 600`（`:31`）、`note_identity_verified`（`:34-36`）、`identity_verified_recently`（`:39-42`）、`is_within_ttl`（`:45-47`，纯函数：0 哨兵拒绝 + 时钟回拨拒绝，测试 `identity_ttl_boundary` `:206-223`）。`verify_windows_identity`（`commands/self_service.rs:254-279`）取主窗口 HWND（`:264-270`）→ `verify_identity`（`:272`）→ `note_identity_verified`（`:274`）；空文案兜底 `DEFAULT_CONSENT_MESSAGE`（`identity.rs:24`、`:64-68`）。

安卓 `identity_gate.rs`：同构实现（模块注释 `:1-2`），`note_identity_verified`（`:18-20`）、`identity_verified_recently`（`:23-33`，回拨视为过期 `:29-31`）、TTL `:8`。

设门策略（双端一致）：

| 命令 | 门 | 位置 |
| --- | --- | --- |
| `bind_operator` | 需门 | 桌面 `self_service.rs:82-84`；安卓 `self_service_cmds.rs:87-89` |
| `self_offline_session` | 需门 | 桌面 `:231-233`；安卓 `:227-229` |
| `reveal_operator_credential` | **强制门**（无论 `self_hello_enabled`） | 桌面 `:300-304`；安卓 `:257-259`（直接检查 `identity_verified_recently`，不经 `ensure_identity_gate`） |
| 查询类（bind_status/dashboard/online_log） | 有意不设门 | 桌面注释 `self_service.rs:11-14`（总览卡自动刷新依赖免验证拉取）；安卓同样不设门 |
| 验证本身 | 记录时间戳 | 桌面 `verify_windows_identity` `:272-274`（走系统 API）；安卓 `verify_biometric_identity` `self_service_cmds.rs:61-66`（信任前端结果，Known Issues 3） |

`self_hello_enabled` 总开关关闭时 `ensure_identity_gate` 直接放行（桌面 `self_service.rs:16-18`，安卓 `self_service_cmds.rs:39-41`）；开启但未验证/过期时拦截，文案分别为"Windows 身份验证已过期…"（桌面 `:22`）与"生物识别验证已过期,请重新验证"（安卓 `:45`）。

### 5. 登录请求脱敏与只读探测

- `redact_credentials`（`auth/portal.rs:67-81`）：完整 URL → `{base_url}?***`（`:70-72`）、URL 编码密码 → `***`（`:73-76`）、明文密码 → `***`（`:77-80`）。doc（`:67-68`）说明动因：reqwest 错误的 Display 会携带完整请求 URL（含 `user_password`）。
- `do_login_request`（`auth/protocol.rs:124-190`）：URL 携带 `user_password={urlencode(password)}`（`:132-138`），但开始日志只打 user/operator/adapterIp（`:142-143`）；错误消息必经 `redact_credentials`（`:163-168`，全仓库唯一调用点）+ `safe_truncate` 200；完成日志只打 `safe_url`（`:140`、`:186-187`）。
- 响应体 1MB 上限：`MAX_HTTP_BODY`（`protocol.rs:14`）、同步 `:21-41`、异步 `:46-63`、登录路径内联 `:173-182`。
- 只读探测 `handle_unknown_page_status`（`portal.rs:125-143`）：doc（`:125-131`）安全约束"状态探测是只读操作，绝不允许携带用户密码调用登录端点"（两类风险：错误凭据轮询触发账号锁定；正确凭据被静默登录误报未登录），签名（`:132`）不含密码，测试锁定 `handle_unknown_page_status_is_readonly_no_credentials`（`:459-466`）。
- 登出使用占位凭据 `drcom`/`123`（`protocol.rs:3-4`，拼进 URL `:317-318`）。
- `open_external`（`commands/system.rs:24-36`）：仅允许 http/https（`:25-27`）、长度 ≤2048（`:28-30`）、拒绝携带用户名/密码的 URL（`:32-34`）。
- 防路径穿越：桌面 `validate_account_name` + 测试 `rename_account_core_rejects_illegal_id_without_disk_writes`（`account.rs:620-632`，`../config`、`a/b` 拒绝且不落盘）；安卓 `ACCOUNT_NAME_RE`（`account_cmds.rs:13-14`，`^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$`）+ `validate_account_name`（`:79-87`），测试 `:466-473`。
- 登录历史不含密码：桌面 `append_login_history`（`persist.rs:160-212`，字段仅 time/success/message/adapter/user/type `:193-200`，上限 100 `:202-204`）；安卓 `login_history.rs:51-76`（adapter 固定 `wlan0` `:62`），测试 `:139` 断言 `!raw.contains("password")`。

### 6. 事件与日志通道

- `emit_config_changed`（`infra/events.rs:107-109`）原样透传 payload；`save_config_to_disk_encrypted`（`config_cmd.rs:9-23`）统一负责掩码（`:17`）与 `{"config": …}` 包裹（`:18`，注释 `:13-16`："直发裸 Config 会让前端监听静默失效"），随后刷新托盘（`:19-21`）。安卓同构：`emit_config_changed`（`config_state.rs:384-391`）。
- `infra/logger.rs`：单文件 5MB × 5 个（`:11-12`），`rotate_if_needed`（`:224`，`app-<stamp>.log`）、按保留天数清理 `cleanup_old_logs_by_time`（`:288`，0=跳过）、`pub fn log()`（`:314`）直写、`read_recent_logs`（`:409`）、`clear_logs`（`:496`）。日志通道无统一脱敏钩子（Known Issues 8）。
- 诊断包导出（`system.rs:191-280`）中配置以掩码形态写 `config-masked.json`（`:239-243`），manifest 记录导出清单（`:262-276`）。

## 关键约束

1. 任何 `Config`/`Settings` 出站（事件、`get_config`、`get_init_data`、导出、诊断）必经 `masked_for_display`；事件体必须 `{"config": …}` 包裹（`config_cmd.rs:13-18`）。
2. 落盘加密"非空即加密"，不得排除 `PASSWORD_MASK`（`persist.rs:278-281`）；安卓加密失败必须 Err 拒绝落盘，不得明文兜底（`config_state.rs:260-272`）。
3. 密码输入三态语义：明文直用、`***`/空回退已存值、`clear` 置空；掩码占位符绝不落盘（`config_cmd.rs:238-252`、`config_state.rs:393-403`、导入还原 `config_cmd.rs:115-131`）。
4. 验证门时间戳只存后端进程内存，TTL 600s；时钟回拨视为过期（`identity.rs:45-47`、`identity_gate.rs:29-31`）。
5. `reveal_operator_credential` 强制设门且与明文返回在同一后端函数内关联，不依赖前端编排（桌面 `self_service.rs:281-315`，安卓 `self_service_cmds.rs:242-271`）。
6. 状态探测不带密码（`portal.rs:125-143`）；登录错误日志必经 `redact_credentials`（`protocol.rs:163-168`）。
7. 账号 id 必须过白名单（防路径穿越），显示名单独校验（1-32 字符、允许空格 emoji）；登录历史与质量历史不含密码。
8. 含密码导出仅限本机导入还原，密文解密失败即拒绝导入（`config_cmd.rs:126-129`）；导入大小上限 1MB（`config_cmd.rs:158-163`）。

## Architecture（Data Flow）

**桌面保存流**：前端 `save_config` → `validate_config`（`config_cmd.rs:228-234`，掩码密码跳过 `validate.rs:92`）→ 三态回退（`:238-252`）→ 先落盘后更新内存（`:261-264`）→ `save_config_to_disk_encrypted`：DPAPI 加密 → `atomic_write` → 掩码广播 `config-changed` → 托盘刷新（`persist.rs:276-291` + `config_cmd.rs:9-23`）→ R2 自动建号（`config_cmd.rs:271-273` → `account.rs:366-376`/`:389-423`，撞库跳过绝不覆盖）。

**桌面导入导出流**：导出 `build_config_export_payload`（`config_cmd.rs:85-113`）——不含密码态直接掩码（`:103`），含密码态两字段 DPAPI 密文 + `passwordEncrypted: true` 标志（`:94-100`、`:110`）；导入 `import_config`（`:152-207`）——1MB 上限（`:158-163`）→ 还原密码先于校验（`:176-180`）→ 五个切换态字段保留本机（`outbound_metric_restore`/`outbound_disabled_adapters`/`outbound_standby_route`/`outbound_manual_hold_day`/`night_outbound_restore`，`:181-189`）→ 严格校验（`:191-195`）→ 落盘广播（`:197-203`）。

**桌面账号切换流**：`switch_account`（`account.rs:15-35`）→ `perform_switch_account_sync`（`:42-60`）→ `load_account_config`（`persist.rs:73-96`，不存在 Err"账号不存在"，解密失败 Err"账号密码解密失败"）→ `merge_account_into_config`（`account.rs:62-82`，6 个登录字段 + 夜切恢复目标置空 `:70-72`，排除 `adapter1/2_account` 设备级字段）→ 加密落盘（`:56`）→ 掩码出站（`:28`）。另存 `save_current_as_account`（`:85-184`）旧账号回存后经 `merge_save_as_target`（`:194-216`，解密失败可宽松整档覆盖自救 `:205-213`，测试 `account.rs:637-658`）。

**安卓保存流**：前端 `save_config` → `resolve_password_field` ×2（`config_state.rs:434-440`）→ `save_file`（`:252-283`）：两密码字段经 `CryptoBridge`（Keystore AES-GCM）加密，失败 Err 拒绝落盘 → `EncodedSettings`（体内置空、密文独立字段）→ tmp+rename → 缓存更新 → R2 自动建号（`:446-448`）→ `emit_config_changed` 掩码广播（`:449-451`）。

**安卓账号流**：`switch_account`（`account_cmds.rs:164-206`，tokio 异步 I/O 锁 `:17`/`:177`）→ `load_account_file`（Keystore 解密）→ 合并登录字段 + 夜切两恢复字段置空（`:185-193`）→ `persist_current`（`:151-162`，掩码出站 `:161`）。改名 `rename_account`（`:311-343`）核心 `:347-371`（重名 Err"名称已存在"，只改 display_name）；读 `read_display_name`（`:99-103`）不解密、不走 Keystore。

**验证门时序**：前端调 `verify_windows_identity`/`verify_biometric_identity` → 系统 Hello / BiometricPrompt → 成功后 `note_identity_verified` 写时间戳（桌面 `self_service.rs:272-274`，安卓 `self_service_cmds.rs:63-64`）→ 后续敏感命令经 `ensure_identity_gate`：`self_hello_enabled` 关 → 放行；`identity_verified_recently`（TTL 600s 内且无回拨）→ 放行；否则拦截。`reveal_operator_credential` 绕过开关强制检查（桌面 `:300-304`，安卓 `:257-259`）。

**双端加密对照**：

| 维度 | 桌面 | 安卓 |
| --- | --- | --- |
| 算法 | DPAPI（CryptProtectData，当前用户作用域） | AndroidKeyStore AES-256-GCM |
| 落盘形态 | `config.json` 字段内存 base64 密文 | `passwordCipher`/`selfPasswordCipher` 独立密文字段 |
| 加密失败 | 报错（DPAPI 失败即 Err） | Err 拒绝落盘（不落明文） |
| 空密码 | 保持空串（`persist.rs:322-333`） | 不写密文位（`config_state.rs:752-764`） |
| 解密失败（读取时） | 清空字段保留其余配置（`config_cmd.rs:36-45`） | `unwrap_or_default` 置空（`config_state.rs:243-248`） |
| `self_password`（账号档案） | **明文透传**（`persist.rs:105-108`，Known Issues 1） | 密文（与主配置一致） |

## Connections

- [[page:desktop-config]] — 配置模型、`save_config` 三态语义与 `config-changed` 广播的完整链路。
- [[page:desktop-account-selfservice]] — 自助命令面（绑定/总览/下线/凭据查看）与验证门的交互时序。
- [[page:android-backend]] — 安卓命令面与状态缓存（`AndroidState`）。
- [[page:android-plugins]] — keystore 插件的 Rust/Kotlin 双侧接口。
- [[page:ipc-command-surface]] — 双端命令注册总表（含 `verify_biometric_identity` 注册点 `android/src-tauri/src/lib.rs:71`）。
- [[page:config-and-persistence]] — 配置结构与原子写、迁移机制。
- [[page:dual-platform-sharing]] — 双端同构约定（回退语义、验证门、掩码出口的镜像实现）。

## Known Issues

1. **桌面账号档案 `self_password` 明文落盘**：`save_account_config` 仅加密 `password`（`persist.rs:105-108`），`self_password` 透传明文——与主配置 `save_config_to_disk_encrypted`（`:282-287`）不一致；安卓侧两字段均加密（`config_state.rs:261-272`），无双端对齐。
2. **`self_reverify_each_action` 无后端消费方**：桌面仅字段定义（`model.rs:24`）、Default false（`:241`）与构造默认值（`network/adapter.rs:235`）三处；安卓仅 `config_state.rs:21`/`:120`。字段存在但任何"每次操作重新验证"的策略都未实现。
3. **安卓验证门信任前端**：`verify_biometric_identity` 被调用即写时间戳（`self_service_cmds.rs:61-66`），后端不校验 BiometricPrompt 真实结果；桌面走系统 API（`self_service.rs:272-274`）相对可信。webview 侧伪造调用即可解锁 TTL 窗口。
4. **`redact_credentials` 覆盖面窄**：全仓库唯一调用点在登录请求错误路径（`protocol.rs:166`）；其余网络错误、诊断信息、日志均无自动脱敏。
5. **密码校验无复杂度要求**：`validate_password` 仅非空且 ≤128 字符（`config/validate.rs:31-39`）。
6. **查询类命令携带密码出设备**：`query_bind_status`/`query_self_dashboard`/`query_self_online_log` 有意不设门（桌面注释 `self_service.rs:11-14`），凭据经 `resolve_self_password`（`:47-61`）随请求发往校园网关——数据本就存于本机，属有意设计而非缺陷，但意味着总览卡自动刷新会周期性使用密码。
7. **掩码出口类型收敛不对称**：桌面 `masked_for_display` 返回同构 `Config`（`model.rs:315-319`），`mask_in_place` 是 `pub`（`:321-328`）可被绕过直接调用；安卓返回 `serde_json::Value`（`config_state.rs:372-382`），出站即丢失类型。两侧类型系统都无法在编译期强制"必经出口"。
8. **日志通道无统一脱敏钩子**：`pub fn log()`（`infra/logger.rs:314`）直写不检查内容，依赖调用方自律（登录路径靠 `protocol.rs:163-168` 手动 redact）。
