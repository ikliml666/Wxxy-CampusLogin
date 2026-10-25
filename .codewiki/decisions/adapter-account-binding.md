---
title: 适配器-账号绑定（凭据副本方案）
type: decision
source_files:
  - tauri-app/src-tauri/src/auth/service.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/frontend/src/settings/types.ts
  - tauri-app/src-tauri/src/commands/account.rs
tags: [决策, 账号, 适配器, 双适配器, 凭据]

## Overview

在桌面端多网卡场景下，允许不同网络适配器绑定不同的校园网账号进行 Portal 登录。该功能通过 `Config` 模型新增设备级字段（`adapter1_account` / `adapter2_account`）记录“网卡→账号”映射，并在登录编排层按角色构造独立的凭据副本传递给登录闭包。核心设计原则是**设备级配置与账号级数据解耦**：切账号不改变绑定关系，且未指定绑定时保持零拷贝共享 `Arc<Config>`（R1 规则）。

## Key Components

### Config 模型 (`tauri-app/src-tauri/src/config/model.rs`)

`Config` 结构体定义于 `model.rs:6-182`，其中新增两个设备级字段，用于存储适配器绑定的账号 ID：

- `adapter1_account` (`model.rs:27-28`)：主适配器绑定的账号 id（R1，设备级配置，仅桌面）；JSON Key 为 `adapter1Account`（行27 `#[serde(rename = "adapter1Account")]`）。空 = 跟随当前激活账号。注释明确要求切换账号时不得被账号档案覆盖。
- `adapter2_account` (`model.rs:30-31`)：副适配器绑定的账号 id，语义同 `adapter1_account`；JSON Key 为 `adapter2Account`（行30 `#[serde(rename = "adapter2Account")]`）。

*   **契约约束**：受容器级 `#[serde(default)]` (`model.rs:9`) 保护，旧版本配置文件缺失字段时回退为空串，默认值见 `model.rs:254-255`（`adapter1_account: String::new()` 与 `adapter2_account: String::new()` 于 `Default` 实现中），无需 schema 迁移。测试 `serde_account_fields_json_names_and_defaults` (`model.rs:375-403`) 锁死了 JSON 字段名与默认空串行为。
*   **前端同步**：`tauri-app/frontend/src/settings/types.ts` 的 `Config` 接口增加同名可选字段（行93 `adapter1Account?: string` 与行95 `adapter2Account?: string`），注释明确“仅桌面设备级配置，切账号不改动”。

### 认证服务层 (`tauri-app/src-tauri/src/auth/service.rs`)

*   **`credentials_missing(config: &Config) -> bool`** (`service.rs:24-28`)：前置校验纯函数。全局凭据缺失**且**主/副适配器都未指定账号时才返回 `true`；若存在适配器绑定，则放行由后续逻辑接管（R1：适配器指定账号的凭据在账号档案里，全局为空不再提前失败）。单元测试 `credentials_missing_requires_global_and_no_binding` (`service.rs:399-419`) 验证了该规则。
*   **`resolve_manual_adapter_account_id<'a>(name: &str, config: &'a Config) -> &'a str`** (`service.rs:33-41`)：手动指定网卡登录时的账号匹配规则纯函数（按 §4.5 规则 4）。网卡名 == 主适配器 → 返回 `adapter1_account`；== 副适配器 → 返回 `adapter2_account`；都不匹配 → 返回空串（跟随全局账号）。测试 `manual_adapter_account_matches_role` (`service.rs:389-395`) 覆盖此逻辑。
*   **`credentials_config(base: &Config, account: Option<&Config>) -> Config`** (`service.rs:46-54`)：凭据覆盖纯函数。`account` 为 `None` 时返回 base 克隆；为 `Some` 时仅覆盖 `user`/`password`/`operator` 三个凭据字段，其余所有字段（含 `adapter1_account`/`adapter2_account`）保持 `base` 原值，防止账号档案污染设备映射。测试 `credentials_config_none_returns_base_clone` (`service.rs:359-370`) 与 `credentials_config_some_overrides_only_credentials` (`service.rs:374-386`) 验证此隔离。
*   **`load_adapter_account_config(app_handle: &AppHandle, account_id: &str) -> Option<Config>`** (`service.rs:58-63`)：薄封装。空 id 直接返回 `None`；文件不存在/读取/解析/解密失败 → `log_warn` 后返回 `None`（回退语义，不阻断登录）。
*   **`adapter_account_from_dir(data_dir: &Path, account_id: &str) -> Option<Config>`** (`service.rs:66-74`)：纯函数回退逻辑，便于单测（如 `adapter_account_missing_file_falls_back_to_none` 行423-429）。
*   **`adapter_credentials(base: &Config, app_handle: &AppHandle, account_id: &str) -> Config`** (`service.rs:77-79`)：组装器。组合上述两步，返回最终的有效凭据副本（未指定账号时凭据与全局一致）。
*   **`full_login(state: &AppState, app_handle: &AppHandle, adapter_name: Option<&str>) -> CommandResult`** (`service.rs:81-191`)：统一登录入口。R1: 全局凭据缺失不再直接拒绝（行87-90）。手动指定网卡（行115-131）按角色取绑定账号后调用 `adapter_credentials`；双适配器错峰并行登录（行141-181），为每个适配器构造 `Arc<Config>` 凭据副本（`config_shared1`/`config_shared2`，行154-163，空则 `Arc::clone(&config)` 零拷贝，非空则 `Arc::new(adapter_credentials(...))` 深拷）；主适配器路径（行183-190）同样按角色取绑定账号。
*   **`logout_adapter_with_log(...)`** (`service.rs:193-206`)：适配器注销薄封装，仅依赖 `config.user` 与适配器 IP 发起请求（行204 `do_logout_with_retry(&config.user, Some(adapter_ip.as_str()), 2, is_quitting)`）。
*   **`full_logout(state: &AppState, app_handle: &AppHandle, adapter_name: Option<&str>) -> CommandResult`** (`service.rs:208-292`)：统一注销入口。双适配器注销并行（行262-283），注意：直接 `let config_shared1 = config; let config_shared2 = config_shared1.clone();`（行270-271），复用全局 `Arc<Config>`，**未**为各适配器分别构造绑定账号的凭据副本。
*   **`post_login_handler(app_handle: &AppHandle, state: &AppState)`** (`service.rs:296-327`)：登录成功后公共后处理（解除注销保护期、延迟后台检测、按需触发自动退出）。

### 账户管理命令 (`tauri-app/src-tauri/src/commands/account.rs`)

*   **`merge_account_into_config(current: &mut Config, account: &Config, safe_name: &str)`** (`account.rs:85-101`)：切换账号时的字段合并规则。**明确排除** `adapter1_account` 和 `adapter2_account`（行84注释说明排除设备级配置；行92-94 仅赋值 `adapter1`/`adapter2` 而不动绑定 id），确保切账号只改变用户凭据与网卡选择，不漂移“网卡→账号”的设备级拓扑映射。
*   **`perform_switch_account_sync(...)`** (`account.rs:52-79`)：切换账号核心同步逻辑，调用 `merge_account_into_config` 并经 `save_config_to_disk_encrypted` 落盘。
*   **`auto_create_account_in(data_dir: &Path, config: &Config) -> Result<bool, String>`** (`account.rs:496-530`)：自动建号核心，只更新 `user`/`password`/`operator` 与 `display_name`/`active_account`，不触碰绑定字段。
*   **契约测试**：`merge_account_fields_and_exclude_device_level_binding` (`account.rs:794-815`) 断言切账号后 `adapter1_account`/`adapter2_account` 保持不变；`merge_account_uses_account_display_name_when_present` (`account.rs:816-823`) 验证显示名合并。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                      Config Model                            │
│  model.rs (行27-31)                                          │
│  ├─ user / password / operator (账号级)                       │
│  └─ adapter1_account / adapter2_account (设备级)              │
└──────────────────────────┬───────────────────────────────────┘
                           │ load_full() (service.rs:84)
                           ▼
┌──────────────────────────────────────────────────────────────┐
│                  Auth Service Layer                          │
│  service.rs:81-191 (full_login)                              │
│                                                              │
│  1. resolve_manual_adapter_account_id() (行33-41)            │
│     └─ 匹配网卡名 -> 获取 adapterX_account                   │
│                                                              │
│  2. load_adapter_account_config() (行58-63)                 │
│     └─ 读取 accounts/<id>.json (解密回退 None)               │
│                                                              │
│  3. credentials_config() (行46-54)                           │
│     └─ 覆盖 user/pass/op，保留 base 的设备字段               │
│                                                              │
│  4. Arc<Config> 分发 (行154-163)                            │
│     ├─ 若 account_id 为空 -> Arc::clone (零拷贝)             │
│     └─ 若 account_id 非空 -> Arc::new (深拷贝副本)           │
└──────────────────────────┬───────────────────────────────────┘
                           │ move || login_adapter_with_log(...)
                           ▼
┌──────────────────────────────────────────────────────────────┐
│                 Dual Adapter Executor                        │
│  dual_adapter_executor::execute_dual (service.rs:168-172)     │
│  ├─ spawn_blocking(a1_action)                                │
│  └─ sleep(1s) -> spawn_blocking(a2_action)                   │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

### 1. 登录凭据解析流程
1.  **全局加载**：`full_login` 调用 `state.config.load_full()` 获取全局 `Arc<Config>`（`service.rs:84`）。
2.  **角色解析**：根据当前路径（手动指定/双适配器/单适配器），调用 `resolve_manual_adapter_account_id`（`service.rs:122`）或直接使用配置中的 `adapter1_account`/`adapter2_account`（`service.rs:154-163`）获取目标账号 ID。
3.  **档案读取**：若账号 ID 非空，调用 `load_adapter_account_config`（`service.rs:157` / `service.rs:162`）从磁盘读取对应账号的完整配置。
4.  **凭据融合**：调用 `credentials_config`（`service.rs:46-54`，经 `adapter_credentials` 行77-79 封装），将读取到的账号凭据（user/password/operator）覆盖到全局配置的副本上；此步骤保证全局配置中的 `adapter1_account` 等字段不被覆盖。
5.  **Arc 包装**：
    *   若未绑定账号（ID 为空）：直接 `Arc::clone(&config)`，实现零拷贝（`service.rs:155` / `service.rs:160`）。
    *   若已绑定账号：`Arc::new(adapter_credentials(...))`，构造独立副本（`service.rs:157` / `service.rs:162`）。

### 2. 双适配器并发分发
1.  `full_login` 在检测到双适配器时（`service.rs:141-181`），分别为 `a1` 和 `a2` 构造 `config_shared1` 和 `config_shared2`（均为 `Arc<Config>`）。
2.  将这两个 `Arc` 移动进 `execute_dual` 的两个闭包中（`service.rs:168-172`）。
3.  `execute_dual` 启动 `spawn_blocking` 并行执行登录（`service.rs:169-170`），闭包内调用 `login_adapter_with_log(&a_clone, &config_shared, ...)`。

### 3. 切账号隔离机制
当用户执行 `switch_account`（命令入口 `account.rs:26`，核心 `perform_switch_account_sync` `account.rs:52-79`）时，`merge_account_into_config`（`account.rs:85-101`）仅更新 `user`、`password`、`operator`、`night_operator_restore`、`adapter1`、`adapter2`、`dual_adapter` 以及 `display_name`/`active_account`。**绝对不触碰** `adapter1_account` 和 `adapter2_account`（行92-94 未涉及绑定 id）。这确保了即使 A 账号档案里记录了某网卡偏好，切换到 B 账号后，以太网依然绑定原设备映射的账号（因为绑定关系存储在设备配置中，而非账号档案中）。

### 4. 注销流程的凭据共享差异
在 `full_logout` 的双适配器注销分支（`service.rs:262-283`）中，代码直接将全局 `config`（`Arc<Config>`）赋值给 `config_shared1` 并 clone 给 `config_shared2`（`service.rs:270-271`），未调用 `adapter_credentials` 构造按绑定账号区分的凭据副本。由于 `logout_adapter_with_log` 仅使用 `config.user` 与适配器 IP 构造注销请求（`service.rs:204`），此处全局共享 `Arc<Config>` 的行为与登录时按适配器分发绑定账号凭据（`service.rs:154-163`）形成实现上的对比，属于当前代码事实。

## Connections

[[desktop-auth]]、[[desktop-config]]、[[account-display-name-id-separation]]、[[dual-platform-sharing]]