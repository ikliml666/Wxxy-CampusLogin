---
title: 账号显示名与内部 id 分离（含自动建号契约）
type: decision
source_files:
  - tauri-app/src-tauri/src/commands/account.rs
  - tauri-app/src-tauri/src/config/persist.rs
  - android/src-tauri/src/account_cmds.rs
  - tauri-app/frontend/src/account/useAccount.ts
tags: [决策, 账号, 显示名, 自动建号, 双端同构]
---

## 背景

账号的内部标识曾是"账号文件名 stem"兼做 UI 显示名：文件名受白名单字符集约束（字母数字/下划线/连字符/中文），用户无法把账号显示成"我的 账号😀"这类可读名。同时"存账号"要先去账号面板手动另存，输入账号密码登录时不会自动留下账号档案。

## 决策

**id 与 displayName 分离**：

- id = 账号文件名 stem（`<data_dir>/accounts/<id>.json`，`persist.rs:69` `get_account_path`），**稳定不变**，仍受 `validate_account_name` 白名单约束（防路径穿越：id 直接拼文件路径）；
- displayName 为新增持久化字段（桌面 `Config.displayName`、安卓 `Settings.displayName`，camelCase，默认空串），trim 后 1..=32 码点、允许空格与 emoji、禁止控制字符（`account.rs:345` `validate_display_name`；安卓同构 `account_cmds.rs:362`）；
- **读取阶段兜底**：displayName 为空（含纯空白）或账号文件读取/解析失败时，列表项显示名兜底为 id（`persist.rs:117` `list_account_items`；安卓 `account_cmds.rs:99` `read_display_name` + `:107` `list_account_items_sync`），出站一律非空；
- **命令面变化**：`list_accounts` 返回由 `Vec<String>` 改为 `Vec<AccountItem>`（`{id, displayName}`，`state/mod.rs:295`）；`get_init_data.accounts` 同步（`commands/system.rs:121`）；新增 `rename_account(accountId, displayName)` 命令（桌面 `account.rs:267`、安卓 `account_cmds.rs:298`，两端注册于 `app/startup.rs:88` 与 `lib.rs:77`）。

**rename 的校验与边界**——只改一个字段：

1. id 先过文件名白名单（`perform_rename_account_sync` 入口与 `rename_account_core` 内各校验一次，后者不依赖 AppHandle 可单测）；
2. `validate_display_name` 校验新显示名（trim 后返回，空/超 32 码点/含控制字符拒绝）；
3. 账号必须存在（不存在/读盘/解析失败以 `Err` 返回，不 panic、不新建文件）；
4. 重名拒绝：与其他账号的 displayName 重复即拒绝（按 id 排除自身；对方显示名为空时以其兜底 id 参与比较）；
5. 成功时只改档案的 `displayName` 字段写回，**不动文件名、不动档案内其他字段、不动 `active_account`**；若被改名账号是当前激活账号，同步主配置 `display_name` 并经 `save_config_to_disk_encrypted` 落盘（广播 `config-changed`，前端即时可见）；
6. 错误通道与 `switch_account` 不同：rename 的业务错误（校验失败/重名/不存在）以 IPC `Err(String)` 返回，由前端 `useAccount` 的 try/catch toast（两端注释同构）。

**为什么不做"重命名文件"**：id 被 `activeAccount`（主配置）、`adapter1Account`/`adapter2Account`（设备级绑定，见 [[adapter-account-binding]]）、托盘切换菜单的事件 id（`SWITCH_ITEM_PREFIX + id`）、前端列表 key 等多处引用；改文件名意味着原子地重写档案文件 + 主配置 + 所有引用，且在多写路径并发（见 [[config-write-lock-coverage]]）下有丢改窗口。显示名分离后 id 永不变化，rename 只是一次字段级写盘。

**为什么不需要 schema 迁移**：两端账号/配置结构都是"无 `deny_unknown_fields` + 容器级 `serde(default)`"（桌面 `Config`、安卓 `Settings`），旧配置文件缺 `displayName`/`adapter1Account` 等新字段时 serde 自动补默认值，**未升 schema 版本、无迁移**（契约锁测试 `config/model.rs` `serde_account_fields_json_names_and_defaults`；安卓见 [[android-interval-default-schema-migration]] 的既有结论）。

**自动建号契约（R2）**——输入账号密码即自动留下账号档案：

- 触发点在 `save_config` **落盘成功后**（桌面 `commands/config_cmd.rs:260`；安卓 `config_state.rs:363`），不是新增命令层命令；内部只调底层落盘（`persist::save_account_config` / 安卓 helper `save_file`），不经命令层避免事件回环/递归；失败仅 `log_warn`，不得让本次保存失败。
- 规则（桌面 `account.rs:386` `auto_create_account_in`，安卓同构 `account_cmds.rs:401`）：
  1. `user` 或 `password` 为空 → 跳过；
  2. id = `sanitize_account_id(user)`（`infra/state/mod.rs:77`：白名单字符保留、其余替换 `_`、按字符截断 32），为空 → 跳过；
  3. 档案不存在 → 以当前配置快照建号，`displayName` = 原始 user 文本（保留原文含非法字符），快照 `active_account` = id；
  4. 档案已存在且 `existing.user != config.user` → **撞库**（不同用户名 sanitize 出同一 id），跳过并 `log_warn`，绝不覆盖他人凭据；
  5. 档案已存在且 user 一致：`password`/`operator` 均一致 → **幂等短路**不写盘（防每次防抖保存都落盘）；有差异 → 只更新这 3 个字段，**保留自定义 displayName 与其他字段**；
  6. 返回"是否实际写盘"：桌面据此**补刷托盘菜单**——`save_config_to_disk_encrypted` 内部的托盘刷新发生在自动建号之前，不补刷则新账号要等下一次落盘才进托盘子菜单（安卓无托盘，无此步）。
- 自动建号拿到的内存 config 已过 `save_config` 的掩码兜底（`config_cmd.rs:210-236`：空/`***` 占位符回填已保存值），写入档案前经 `save_account_config` 对非空密码 DPAPI 加密（安卓走 Keystore 桥），不落明文。

**前端**：`useAccount` 新增 `handleRenameAccount`（成功后 `setActiveAccount` + `updateConfig` + 刷新列表）与共享的 `refreshAccounts`；`accounts` 状态由 `string[]` 改为 `AccountItem[]`（`useConfigStore.ts`）；删除确认弹窗展示显示名、调用仍传 id（`App.tsx`）；仪表盘账号卡片按 `id` 匹配激活态、显示 `displayName`（`DashboardPanel.tsx`）。两端同构（安卓 `android/frontend/src/account/*`）。

## 影响与约束

- 改动 id 的生成/校验规则必须同步 `sanitize_account_id`（宽松替换版）与 `validate_account_name`（严格白名单版）两个字符类，`state/mod.rs` 注释已互指。
- `list_accounts` 出站契约变化是**破坏性 IPC 变更**：前后端必须同一次提交内同步（两端 `tauriApi.ts` 的 `listAccounts` 返回类型已同步）。
- 重名检查是 O(N) 全量列举（N = 账号数，本地文件量级），不做索引。
- 绑定运营商功能未改动（用户明确要求保持现状）。

## Connections

[[adapter-account-binding]]、[[desktop-account-selfservice]]、[[android-backend]]、[[config-write-lock-coverage]]、[[config-mask-single-exit]]
