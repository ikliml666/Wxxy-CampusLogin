---
title: 适配器-账号绑定（凭据副本方案）
type: decision
source_files:
  - tauri-app/src-tauri/src/auth/service.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/frontend/src/settings/types.ts
  - tauri-app/src-tauri/src/commands/account.rs
tags: [决策, 账号, 适配器, 双适配器, 凭据]
---

## Overview

在桌面端多网卡场景下，允许不同网络适配器绑定不同的校园网账号进行 Portal 登录。该功能通过 `Config` 模型新增设备级字段记录"网卡→账号"映射，并在登录编排层按角色构造独立的凭据副本传递给登录闭包。核心设计原则是**设备级配置与账号级数据解耦**：切账号不改变绑定关系，且未指定绑定时保持零拷贝共享 `Arc<Config>`。

## Key Components

### Config 模型 (`tauri-app/src-tauri/src/config/model.rs`)

新增两个设备级字段，用于存储适配器绑定的账号 ID：

| 字段 (Rust) | JSON Key | 类型 | 默认值 | 用途 |
| :--- | :--- | :--- | :--- | :--- |
| `adapter1_account` | `adapter1Account` | `String` | `""` | 主适配器绑定的账号 id（空=跟随当前激活账号） |
| `adapter2_account` | `adapter2Account` | `String` | `""` | 副适配器绑定的账号 id（空=跟随当前激活账号） |

*   **契约约束**：受容器级 `#[serde(default)]` 保护，旧版本配置文件缺失字段时回退为空串，无需 schema 迁移 (`model.rs` 结构体定义处)。
*   **前端同步**：`tauri-app/frontend/src/settings/types.ts` 增加同名可选字段，注释明确"仅桌面设备级配置，切账号不改动"。

### 认证服务层 (`tauri-app/src-tauri/src/auth/service.rs`)

提供凭据解析、覆盖与登录编排的核心逻辑：

*   **`credentials_missing(config: &Config) -> bool`** (`service.rs:24-28`): 前置校验纯函数。全局凭据缺失**且**任一适配器未指定账号时才返回 `true`；若存在适配器绑定，则放行由后续逻辑接管。
*   **`resolve_manual_adapter_account_id<'a>(name: &str, config: &'a Config) -> &'a str`** (`service.rs:33-39`): 手动指定网卡时的角色匹配规则。网卡名 == 主适配器 → 返回 `adapter1_account`；== 副适配器 → 返回 `adapter2_account`；否则返回空串（跟随全局）。
*   **`credentials_config(base: &Config, account: Option<&Config>) -> Config`** (`service.rs:44-52`): 凭据覆盖纯函数。若 `account` 为 `Some`，仅覆盖 `user/password/operator` 三个凭据字段，其余所有字段（含 `adapter1_account`/`adapter2_account`）保持 `base` 原值，防止账号档案污染设备映射。
*   **`load_adapter_account_config(app_handle: &AppHandle, account_id: &str) -> Option<Config>`** (`service.rs:57-63`): 薄封装。读取指定账号的加密档案；空 id 直接返回 `None`；文件不存在或解密失败时 `log_warn` 并返回 `None`（回退语义）。
*   **`adapter_credentials(base: &Config, app_handle: &AppHandle, account_id: &str) -> Config`** (`service.rs:68-70`): 组装器。组合上述两步，返回最终的有效凭据副本。
*   **`full_login(state, app_handle, adapter_name)`** (`service.rs:75-200`): 统一登录入口。处理单/双适配器分支，为每个适配器构造专属的 `Arc<Config>` 凭据副本后下发给 `login_adapter_with_log`。
*   **`post_login_handler(app_handle, state)`** (`service.rs:235-265`): 登录成功后的公共后处理（解除注销保护期、触发后台检测等）。

### 账户管理命令 (`tauri-app/src-tauri/src/commands/account.rs`)

*   **`merge_account_into_config`** (`account.rs:66-81`): 切换账号时的字段合并规则。**明确排除** `adapter1_account` 和 `adapter2_account`，确保切账号只改变用户凭据，不漂移"网卡→账号"的设备级拓扑映射。
*   **`perform_switch_account_sync`** (`account.rs:44-63`): 切换账号的核心同步逻辑。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                      Config Model                            │
│  model.rs                                                      │
│  ├─ user / password / operator (账号级)                       │
│  └─ adapter1_account / adapter2_account (设备级)              │
└──────────────────────────┬───────────────────────────────────┘
                           │ load_full()
                           ▼
┌──────────────────────────────────────────────────────────────┐
│                  Auth Service Layer                          │
│  service.rs:75-200 (full_login)                              │
│                                                              │
│  1. resolve_manual_adapter_account_id() [纯函数]             │
│     └─ 匹配网卡名 -> 获取 adapterX_account                   │
│                                                              │
│  2. load_adapter_account_config() [IO]                       │
│     └─ 读取 accounts/<id>.json (解密回退 None)               │
│                                                              │
│  3. credentials_config() [纯函数]                            │
│     └─ 覆盖 user/pass/op，保留 base 的设备字段               │
│                                                              │
│  4. Arc<Config> 分发                                         │
│     ├─ 若 account_id 为空 -> Arc::clone (零拷贝)             │
│     └─ 若 account_id 非空 -> Arc::new (深拷贝副本)           │
└──────────────────────────┬───────────────────────────────────┘
                           │ move || login_adapter_with_log(...)
                           ▼
┌──────────────────────────────────────────────────────────────┐
│                 Dual Adapter Executor                        │
│  dual_adapter_executor.rs                                    │
│  execute_dual(a1_action, a2_action, is_quitting)             │
│  ├─ spawn_blocking(a1_action)                                │
│  └─ sleep(1s) -> spawn_blocking(a2_action)                   │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

### 1. 登录凭据解析流程
1.  **全局加载**：`full_login` 调用 `state.config.load_full()` 获取全局 `Arc<Config>`。
2.  **角色解析**：根据当前路径（手动指定/双适配器/单适配器），调用 `resolve_manual_adapter_account_id` 或直接使用配置中的 `adapter1_account`/`adapter2_account` 获取目标账号 ID。
3.  **档案读取**：若账号 ID 非空，调用 `load_adapter_account_config` 从磁盘读取对应账号的完整配置（含密码）。
4.  **凭据融合**：调用 `credentials_config`，将读取到的账号凭据（user/password/operator）覆盖到全局配置的副本上。此步骤保证全局配置中的 `adapter1_account` 等字段不被覆盖。
5.  **Arc 包装**：
    *   若未绑定账号（ID 为空）：直接 `Arc::clone(&config)`，实现零拷贝。
    *   若已绑定账号：`Arc::new(adapter_credentials(...))`，构造独立副本。

### 2. 双适配器并发分发
1.  `full_login` 在检测到双适配器时，分别为 `a1` 和 `a2` 构造 `config_shared1` 和 `config_shared2`（均为 `Arc<Config>`）。
2.  将这两个 `Arc` 移动进 `execute_dual` 的两个闭包中。
3.  `execute_dual` 启动 `spawn_blocking` 并行执行登录，闭包内调用 `login_adapter_with_log(&a_clone, &config_shared, ...)`。

### 3. 切账号隔离机制
当用户执行 `switch_account` 时，`merge_account_into_config` 仅更新 `user`, `password`, `operator`, `adapter1`, `adapter2`, `dual_adapter` 以及 `display_name`/`active_account`。**绝对不触碰** `adapter1_account` 和 `adapter2_account`。这确保了即使 A 账号档案里记录了"以太网用账号A"，切换到 B 账号后，以太网依然绑定账号 B（因为绑定关系存储在设备配置中，而非账号档案中）。

## Connections

[[desktop-auth]]、[[desktop-config]]、[[account-display-name-id-separation]]、[[dual-platform-sharing]]