---
title: tauri-ipc-command-bridge
type: concept
source_files:
  - android/frontend/src/hooks/tauriApi.ts
  - android/frontend/public/mock-tauri.js
  - android/frontend/public/mock-tauri-addon.js
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/account_cmds.rs
tags:
  - Tauri
  - IPC
  - Bridge
  - Mock
  - Rust
---

## Overview

`tauri-ipc-command-bridge` 是安卓端 Tauri 前后端通信契约层，连接 `android/frontend/src/hooks/tauriApi.ts`（TypeScript）与 `android/src-tauri/src/lib.rs` 注册的 Rust 命令（如 `account_cmds.rs`）。它定义了 `TauriApi` 接口（tauriApi.ts L48-141）枚举全部 IPC 方法，由 `tauriApi` 对象（L190-324）通过 `@tauri-apps/api/core` 的 `invoke` 实现，部分命令在安卓端显式拒绝（桌面专属，L14-15 `desktopOnly`、L196-198 等）。开发环境由 `mock-tauri.js`（L155-177）与 `mock-tauri-addon.js`（L4-44）注入 `window.__TAURI_INTERNALS__` 拦截 `invoke`，实现无原生后端的 UI 验证。Rust 侧通过 `lib.rs` L66-122 的 `generate_handler!` 注册命令，账号相关命令在 `account_cmds.rs` L157-401 实现，并返回 `AccountResult`（L42-55）等结构化类型。

## Key Components

### android/frontend/src/hooks/tauriApi.ts

- **常量/辅助函数**
  - `desktopOnly<T>(name: string)` (L14-15): 返回 `Promise.reject`，提示桌面专属功能安卓不可用。
  - `noopListener` (L18): 返回 `() => {}` 的空监听工厂，用于桌面专属事件的安卓空实现。
  - `isRetryableError(e: unknown)` (L326-336): 判断错误是否含 timeout/network/fetch/connection 关键字。
  - `withRetry<T>(fn, maxRetries=2, baseDelay=500)` (L338-354): 指数退避重试，仅用于可重试错误。
  - `createEventListener<T>(eventName: string)` (L159-188): 封装 `@tauri-apps/api/event` 的 `listen`，返回带清理函数的订阅器，处理未完成监听的取消。

- **接口/类型（Struct 等价）**
  - `ConnectionCampusStatus` (L20-24): 字段 `onCampus: boolean`, `name: string | null`, `message: string`。
  - `BatteryOptimizationInfo` (L26-30): 字段 `ignoring: boolean`, `brand: string`, `hasVendorTarget: boolean`。
  - `VendorSettingsResult` (L32-36): 字段 `path: 'vendor'|'vendor_action'|'app_details'|'settings'|'none'`, `target: string`, `tried: string[]`。
  - `CampusStatusResult` (L38-46): 字段 `onCampusNetwork: boolean`, `currentSsid: string|null`, `campusMessage: string`, `enableNetworkNameCheck: boolean`, `requiredNetworkName: string`, `campusWifi: ConnectionCampusStatus|null`, `campusWired: ConnectionCampusStatus|null`。
  - `AvoidBadWifiStatus` (L143-152): 字段 `granted: boolean`(WRITE_SECURE_SETTINGS 授权), `avoidBadWifi: number`(当前值,-1读不到), `pendingRestoreAvoid: boolean`(快照待还原), `pendingRestoreCaptive: boolean`(快照待还原)。
  - `RestoreWrittenSettingsResult` (L154-157): 字段 `restored: Record<string, boolean>`, `reason?: string`。
  - `TauriApi` (L48-141): 定义全部 IPC 方法签名，涵盖配置、适配器(桌面专属拒绝)、校园网、登录、账号、背景检测、DNS(桌面专属拒绝)、更新、电池优化等。例如 `getConfig`(L49), `saveConfig`(L50), `exportConfig`(L52 桌面专属), `listAccounts`(L80), `createAccount`(L82), `renameAccount`(L87), `verifyWindowsIdentity`(L66, 安卓走生物识别/人脸), `getAvoidBadWifiStatus`(L138) 等。

- **实现对象**
  - `tauriApi: TauriApi` (L190-324): 具体 `invoke` 映射。如 `getConfig`→`invoke('get_config')`(L191), `saveConfig`→`invoke('save_config')`(L192), `verifyWindowsIdentity`(L207-244) 集成 `biometricAuthenticate` 与 2D 人脸回退 `useFaceDialogStore`, `openExternal`(L276-287) 做 URL 校验后调 `openerOpenUrl`, `getAutoLaunch`→`invoke('get_boot_autostart')`(L288) 等。桌面专属命令统一调 `desktopOnly` 拒绝（L193-195, L196-198, L250-251, L269-271, L296, L302, L315-318, L319）。
  - `tauriApiWithRetry: TauriApi` (L356-362): 展开 `tauriApi` 并仅对 `saveConfig` 套 `withRetry`（L361），其余高频命令不重试避免流量放大。

### android/frontend/public/mock-tauri.js

- `transformCallback(callback, once)` (L24-36): 在 window 上定义 `_cb{N}` 属性供 Rust 侧回调。
- `mockConfig` (L38-57): 模拟配置对象，含 `user`, `operator`, `activeAccount`, `logRetentionDays: 7`, `configSchemaVersion: 7` 等。
- `campusStatus` (L59-64) / `backgroundStatus` (L66-72) / `mockAccounts` (L76-80) / `initData` (L82-88): 静态模拟数据。
- `handlers` (L90-153): 命令处理函数映射。如 `get_config`(L91), `save_config`(L92), `list_accounts`(L95), `create_account`(L98-109 校验重名并 push), `save_current_as_account`(L110-119), `rename_account`(L120-125), `delete_account`(L126-130), `check_campus_status`(L134), `get_background_status`(L135), `do_login`(L149 返回演示不执行), `get_avoid_bad_wifi_status`(L148 返回 `{authorized, currentValue, pendingRestore}` 注意字段与真实 `AvoidBadWifiStatus` 不符)。
- `window.__TAURI_INTERNALS__` (L155-177): 伪造 `invoke` (L161-176)，对 `plugin:event|listen` 等特殊处理，未列命令兜底返回 `null`。
- `window.__mockEmitEvent` (L189-195): 手动发射事件给监听者。

### android/frontend/public/mock-tauri-addon.js

- 覆盖 `invoke` (L13-44): 劫持 `check_campus_status`(L14-20), `get_adapters`(L21-27), `check_portal_status`(L28-30), `do_login`(L31), `get_background_status`(L32), `get_init_data`(L33-42 注入 operator/night 逻辑)。
- `bgStatus()` (L46-56): 构造背景状态。
- 定时器 (L58-69): 周期性 emit `background-check-result` 事件。

### android/src-tauri/src/lib.rs

- `mod` 声明 (L11-26): 包含 `account_cmds`, `config_state`, `monitor_loop`, `self_service_cmds` 等。
- `run()` (L31-132): 构建 Tauri 应用，移动端注册插件 (L34-40)，`setup` 初始化日志与启动任务 (L45-64)，`invoke_handler` 通过 `generate_handler!` 注册全部命令 (L66-122)，包含 `account_cmds::list_accounts`(L87), `switch_account`(L88), `rename_account`(L89), `save_current_as_account`(L90), `create_account`(L91), `delete_account`(L92), `get_active_account`(L93) 等。退出护栏 `ExitRequested` 阻止默认退出 (L129-131)。

### android/src-tauri/src/account_cmds.rs

- `ACCOUNT_NAME_RE` (L13-15): 账号名白名单正则 `^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$`。
- `config_io_lock()` (L20-22): 转发 `config_state::config_io_lock` 保证 IO 互斥。
- `AccountItem` struct (L27-40): 字段 `id: String`(L29), `#[serde(rename="displayName")] display_name: String`(L30-31), `user: String`(L34-35), `operator: String`(L36-37), `#[serde(rename="passwordSaved")] password_saved: bool`(L38-39)。用于账号列表出站。
- `AccountResult` struct (L42-55): `#[serde(rename_all="camelCase")]` 字段 `success: bool`(L45), `message: Option<String>`(L46-47), `active_account: Option<String>`(L48-49), `display_name: Option<String>`(L50-52), `config: Option<serde_json::Value>`(L53-54)。
  - 方法 `ok(config)` (L58-60): 成功无账号。
  - 方法 `ok_with_account(account, config)` (L61-63): 成功带激活账号。
  - 方法 `err(msg)` (L64-66): 失败带消息。
- `sanitize_account_id(user)` (L72-83): 非法字符转 `_`，截断 32。
- `validate_account_name(name)` (L86-94): 长度 1-32 且匹配正则。
- `accounts_dir(app)` (L96-102): 返回 `app_data_dir/accounts`。
- `read_account_card_info(path)` (L111-121): 读展示字段，密码保存态走 `config_state::password_saved_at`。
- `list_account_items_sync(dir)` (L126-155): 同步列举，按 id 排序，空显示名兜底 id。
- `#[tauri::command] list_accounts(app)` (L157-163): Spawn blocking 调用上述同步函数。
- `load_account_file(path, bridge)` (L165-170): 加载解密账号文件。
- `persist_current(app, merged)` (L172-183): 保存主配置并刷缓存，返回掩码配置。
- `#[tauri::command] switch_account(app, account_name)` (L185-227): 校验名、合并登录字段、清夜切标记、调 `restore_avoid_bad_wifi`。
- `#[tauri::command] save_current_as_account(app, account_name)` (L229-257): 调用 `merge_save_as_target` 落盘并同步主配置。
- `#[tauri::command] create_account(app, account_name, user, password, operator)` (L259-294): 新建档案不切换，已存在报错。
- `merge_save_as_target(current, existing, safe_name)` (L302-331): 纯函数，处理另存/覆盖逻辑，返回 `(Settings, String)`。
- `#[tauri::command] delete_account(app, account_name)` (L333-362): 删文件，若删当前则清 active。
- `#[tauri::command] get_active_account(app)` (L364-367): 返回主配置 active_account。
- `#[tauri::command] rename_account(app, account_id, display_name)` (L369-401): 调核心函数，若改当前则同步主配置。
- `rename_account_core(dir, bridge, account_id, display_name)` (L405-429): 异步核心，校验重名、只改 displayName。
- `validate_display_name(name)` (L434-447): trim 后 1-32 字符，禁控制字符。
- `auto_create_account_for_current(app, current)` (L451-460): 保存后自动建号入口。
- `auto_create_account_in(dir, bridge, config)` (L473-510): 自动建号/撞库防护/幂等更新。
- 测试模块 `#[cfg(test)] mod tests` (L512-838): 含 `账号名消毒`(L517), `列举账号`(L540), `账号id消毒`(L588), `显示名校验`(L604), `自动建号_无凭据跳过`(L621), `自动建号_新号`(L632), `自动建号_幂等`(L654), `自动建号_差异更新`(L676), `改名核心_成功`(L701), `改名核心_拒绝`(L722), `账号条目_json形状`(L762), `另存账号_显示名不串号`(L783), `另存账号_覆盖`(L803), `自动建号_撞库`(L820) 等单元测试。

## Architecture

```text
┌──────────────────────┐   invoke    ┌──────────────────────┐
│ tauriApi.ts          │ ──────────► │ lib.rs              │
│ (L190-324)           │             │ generate_handler    │
│ TauriApi 接口 L48-141│             │ (L66-122)           │
└──────────────────────┘             └──────────────────────┘
         │ 开发 Mock 拦截                     │ 分派命令
         ▼                                   ▼
┌──────────────────────┐             ┌──────────────────────┐
│ mock-tauri.js        │             │ account_cmds.rs      │
│ (L155-177 inject)    │             │ (L157-401 commands)  │
│ mock-tauri-addon.js  │             │ 读写 accounts/*.json │
│ (L13-44 override)    │             └──────────────────────┘
└──────────────────────┘                      │
         │                                   ▼
         └──────────────────────────────────►│ config_state         │
                                             │ (CryptoBridge 加密)  │
                                             └──────────────────────┘
```

## Data Flow

### 生产环境调用流

1. 前端 Store/Hook 调用 `tauriApi` 方法（如 L258 `listAccounts` → `invoke('list_accounts')`）。
2. Tauri 序列化经 JNI 桥接至 Rust `lib.rs` 注册的命令（L87 `account_cmds::list_accounts`）。
3. Rust 侧持 `config_io_lock` (L198 等) 读写 `accounts_dir` 下 JSON，经 `CryptoBridge` 加解密（L165-170）。
4. 返回 `AccountResult`/`AccountItem` 等 Serde 序列化结构，前端更新 Zustand。

### Mock 环境调用流

1. 浏览器加载 `mock-tauri.js` 注入 `window.__TAURI_INTERNALS__`（L155）。
2. `tauriApi.ts` 的 `invoke` 被 Mock 拦截，命中 `handlers` 返回静态数据（L172）。
3. `mock-tauri-addon.js` 进一步覆盖 `invoke`（L13）注入场景参数（?op=&night=）。
4. 通过 `__mockEmitEvent`（L189）模拟后台事件流。

### 异常与降级流

- 桌面专属命令（如 `exportConfig` L193, `getAdapters` L196）在安卓走 `desktopOnly` 直接 reject，前端需 catch 或类型上禁用。
- `verifyWindowsIdentity` (L207-244) 在安卓优先 2D 人脸回退（`shouldUseFaceFallback`），否则 `biometricAuthenticate` 系统生物识别，失败归一化为 `{success:false}` 不上抛 IPC 故障。
- `withRetry` (L338-354) 仅包裹 `saveConfig` (L361)，防磁盘写入瞬时失败。

## Connections

后端模块依赖见 `lib.rs` L11-26 声明：[[config_state]]（账号加密落盘）、[[monitor_loop]]（切账号后调 `restore_avoid_bad_wifi` L225, L358）、[[identity_gate]]（验证门在 `self_service_cmds` 触发）、[[self_service_cmds]]（六命令同模块面）。前端依赖 [[android-frontend-hooks-stores]]（Zustand 调 `tauriApi`）、[[android-tauri-shell-and-plugins]]（Biometric/Keystore 插件）。

## Known Issues

- **Mock 字段契约不一致**：`mock-tauri.js` L148 `get_avoid_bad_wifi_status` 返回 `{ authorized: false, currentValue: null, pendingRestore: false }`，但真实前端类型 `AvoidBadWifiStatus`（tauriApi.ts L143-152）字段为 `granted`/`avoidBadWifi`/`pendingRestoreAvoid`/`pendingRestoreCaptive`，Mock 形状错误若被前端读取会导致 `undefined`。
- **Mock 缺失命令**：`mock-tauri.js` 未实现 `check_portal_status`（仅 addon L28-30 覆盖），纯基础 Mock 下该命令返回 `null`（L175 兜底），而前端 `checkPortalStatus` 期望 `PortalStatusResult`。
- **桌面专属接口暴露**：`TauriApi` 接口仍声明 `exportConfig`/`getAdapters` 等桌面方法（L52-57），安卓实现直接 reject，若调用方未检查 `desktopOnly` 错误将触发 unhandled rejection。
- **命名遗留**：`verifyWindowsIdentity`（tauriApi.ts L66, L207）名称含 Windows，但实际在安卓走 `biometricAuthenticate` 与 `useFaceDialogStore` 人脸，语义已泛化但命名未改。
- **测试编译隔离**：`account_cmds.rs` 测试（L512-838）仅在 `#[cfg(test)]` 下编译，不进生产二进制，但 `auto_create_account_in` 等核心逻辑依赖测试覆盖保证正确性。