---
title: 安卓端配置与账号文件的并发写保护（IO锁）
type: learning
source_files:
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/account_cmds.rs
tags: [教训, 配置, 并发, 锁]
---

## Overview

安卓端 (`android/src-tauri`) 的配置 (`config.json`) 与多账号档案 (`accounts/*.json`) 采用**全局串行化 IO 锁** (`CONFIG_IO_LOCK`) 来防止并发读-改-写竞态。与桌面端 (`tauri-app`) 不同，安卓端的锁被定义为 `lazy_static` 静态变量并集中在 `config_state` 模块中，所有涉及配置或账号文件读写的命令都必须显式获取此锁。

核心设计原则是“锁在锁的使用者模块里定义”，因为 `save_config` 自身也需要持锁。这种集中管理避免了单向依赖扭曲，并确保从 `load` → `merge` → `save` 的全序列原子性。

## Key Components

### `config_state.rs`: 配置状态与 IO 锁

- **常量**:
    - `PASSWORD_MASK`: `"***"` (行 15)
    - `CONFIG_FILE`: `"config.json"` (行 16)
- **全局锁**:
    - `CONFIG_IO_LOCK`: `tokio::sync::Mutex<()>` (行 24)，用于配置与账号文件的互斥读写。
- **函数**:
    - `config_io_lock()` (行 30): 异步获取配置 IO 锁，供跨命令共用。
    - `masked_for_display(s: &Settings)` (行 286): 掩码出口，非空密码替换为 `***`。
    - `resolve_password_field(...)` (行 300): 密码字段回退语义：空串或 `***` 视为未修改，回退已存值；`clear=true` 时置空。
    - `get_config(app)` (行 310): Tauri 命令，读取配置、更新缓存、返回掩码值。
    - `save_config(app, config, clear_password)` (行 323): Tauri 命令。**全程持锁**，执行 `load` → `merge` → `save` → `auto_create_account` → `emit_config_changed`。
    - `current_settings(app)` (行 351): 缓存优先读取当前设置。
    - `load_file(path, bridge)` (行 147): 加载并解密配置文件。
    - `save_file(path, bridge, s)` (行 164): 加密落盘，使用带 PID 的临时文件名防竞争。
    - `cleanup_stale_config_tmp(dir)` (行 200): 清理进程崩溃遗留的 `.tmp` 文件。
    - `load_from(dir, bridge)` (行 214): 加载配置并执行 `migrate_legacy_defaults`。
    - `migrate_legacy_defaults(dir, bridge, s)` (行 232): 旧版本默认值一次性迁移 (v0→v7)。
    - `save_to(dir, bridge, s)` (行 279): 保存配置到指定目录。
    - `emit_config_changed(app, s)` (行 290): 广播 `config-changed` 事件。
- **结构体**:
    - `Settings` (行 20): 主配置结构，包含账号凭据、网络设置、定时任务等。
    - `CryptoBridge` (行 130): 加解密桥接器，真机走 AndroidKeyStore，host 测试可注入假实现。
    - `EncodedSettings` (行 142): 磁盘存储格式，分离 `settings` 与 `password_cipher`。

### `account_cmds.rs`: 多账号命令面

- **函数**:
    - `config_io_lock()` (行 23): 转发至 `config_state::config_io_lock`。
    - `sanitize_account_id(user)` (行 54): 账号 ID 消毒，替换非法字符为 `_`，截断至 32 位。
    - `validate_account_name(name)` (行 67): 账号名校验 (1-32 字符，仅字母/数字/下划线/中文/连字符)。
    - `list_accounts(app)` (行 127): Tauri 命令，列举账号条目。
    - `switch_account(app, account_name)` (行 143): Tauri 命令。**持锁**切换账号，合并登录字段，清空夜切恢复目标与出站切换态标记。
    - `save_current_as_account(app, account_name)` (行 174): Tauri 命令。**持锁**将当前配置另存为新账号。
    - `delete_account(app, account_name)` (行 206): Tauri 命令。**持锁**删除账号文件，若为当前账号则清除 `active_account` 与出站切换态标记。
    - `get_active_account(app)` (行 240): Tauri 命令，获取当前激活账号 ID。
    - `rename_account(app, account_id, display_name)` (行 245): Tauri 命令。**持锁**重命名账号显示名。
    - `auto_create_account_for_current(app, current)` (行 313): R2 自动建号入口，**不经命令层**，失败仅告警。
- **结构体**:
    - `AccountItem` (行 33): 账号列表条目 (`id`, `displayName`)。
    - `AccountResult` (行 39): 账号操作结果统一返回结构。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                      Tauri Commands                          │
│  save_config / switch / delete / rename / save_current_as    │
└──────────────────┬───────────────────────────────────────────┘
                   │ 1. 获取全局串行锁
                   ▼
┌──────────────────────────────────────────────────────────────┐
│              CONFIG_IO_LOCK (config_state.rs:24)             │
│          tokio::sync::Mutex<()> (static ref)                 │
└──────────────────┬───────────────────────────────────────────┘
                   │ 2. 持锁执行
                   ▼
┌──────────────────────────────────────────────────────────────┐
│                  Config State Layer                          │
│  load_file() → merge() → save_file()                         │
│  (加密/解密 via CryptoBridge)                                 │
└──────────────────┬───────────────────────────────────────────┘
                   │ 3. 释放锁
                   ▼
┌──────────────────────────────────────────────────────────────┐
│                   Disk / Cache                               │
│  config.json / accounts/*.json                               │
│  + emit_config_changed() broadcast                           │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **写入流程 (`save_config`)**:
   - 调用方获取 `config_io_lock` (行 330)。
   - 读取当前配置 (`load_from`)。
   - 合并新配置与密码字段 (`resolve_password_field`)。
   - 加密并原子写入 (`save_file`)。
   - 触发自动建号 (`auto_create_account_for_current`)。
   - 广播 `config-changed` 事件。
   - 释放锁。

2. **账号切换 (`switch_account`)**:
   - 获取锁。
   - 加载目标账号文件。
   - 合并凭据到当前配置，清空夜切恢复 (`night_operator_restore`) 与出站切换态 (`night_outbound_restore`)。
   - 持久化当前配置。
   - 释放锁。

3. **自动建号 (`auto_create_account_for_current`)**:
   - 在 `save_config` 成功后调用。
   - 检查凭据是否变化，若一致则幂等短路；若有差异则更新凭据但保留自定义显示名。
   - **注意**：此函数内部不持锁，依赖调用方 (`save_config`) 已持锁。

## Known Issues

- **锁纪律依赖**: 安卓端的并发保护完全依赖调用方显式获取 `config_io_lock`。虽然 `save_config` 和主要账号命令都已持锁，但新增写路径时必须严格遵守这一纪律，否则仍可能发生竞态。
- **自动建号的锁范围**: `auto_create_account_for_current` 本身不持锁，其安全性完全依赖于它在 `save_config` 的锁范围内被调用。如果未来有其他路径直接调用此函数，需要重新评估锁策略。
- **桌面端对比**: 桌面端 (`persist.rs`) 使用 `LOGIN_HISTORY_LOCK` 保护登录历史追加，且账号档案读写没有对应的静态锁，依赖命令层的 `spawn_blocking` 串行化。安卓端的集中锁机制更为严格和统一。

## Connections

[[mask-placeholder-persisted-as-plaintext]], [[config-missing-field-load-failure]]