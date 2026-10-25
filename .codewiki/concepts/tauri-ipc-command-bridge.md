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

`tauri-ipc-command-bridge` 是安卓端 Tauri 前后端通信契约层，连接 `android/frontend/src/hooks/tauriApi.ts`（TypeScript）与 `android/src-tauri/src/lib.rs` 注册的 Rust 命令（如 `android/src-tauri/src/account_cmds.rs`）。它定义了 `TauriApi` 接口（tauriApi.ts 第 48-153 行）枚举全部 IPC 方法，由 `tauriApi` 对象（第 202-342 行）通过 `@tauri-apps/api/core` 的 `invoke` 实现，部分命令在安卓端显式拒绝（桌面专属，第 14-15 行 `desktopOnly`、第 205-208 行、第 268-269 行、第 287-289 行、第 314 行、第 320 行、第 333-337 行等）。开发环境由 `mock-tauri.js`（第 155-177 行）与 `mock-tauri-addon.js`（第 13-44 行）注入 `window.__TAURI_INTERNALS__` 拦截 `invoke`，实现无原生后端的 UI 验证。Rust 侧通过 `lib.rs` 第 68-130 行的 `generate_handler!` 注册命令，账号相关命令在 `account_cmds.rs` 第 157-401 行实现，并返回 `AccountResult`（第 42-55 行）等结构化类型。

## Key Components

### android/frontend/src/hooks/tauriApi.ts

- **常量/辅助函数**
  - `desktopOnly<T>(name: string)` (第 14-15 行): 返回 `Promise.reject`，提示桌面专属功能安卓不可用。
  - `noopListener` (第 18 行): 返回 `() => {}` 的空监听工厂，用于桌面专属事件的安卓空实现。
  - `createEventListener<T>(eventName: string)` (第 171-200 行): 封装 `@tauri-apps/api/event` 的 `listen`，返回带清理函数的订阅器，处理未完成监听的取消。
  - `isRetryableError(e: unknown)` (第 344-354 行): 判断错误是否含 timeout/network/fetch/connection 关键字。
  - `withRetry<T>(fn, maxRetries=2, baseDelay=500)` (第 356-372 行): 指数退避重试，仅用于可重试错误。

- **接口/类型**
  - `ConnectionCampusStatus` (第 20-24 行): 字段 `onCampus: boolean`, `name: string | null`, `message: string`。
  - `BatteryOptimizationInfo` (第 26-30 行, export): 字段 `ignoring: boolean`, `brand: string`, `hasVendorTarget: boolean`。
  - `VendorSettingsResult` (第 32-36 行, export): 字段 `path: 'vendor'|'vendor_action'|'app_details'|'settings'|'none'`, `target: string`, `tried: string[]`。
  - `CampusStatusResult` (第 38-46 行): 字段 `onCampusNetwork: boolean`, `currentSsid: string|null`, `campusMessage: string`, `enableNetworkNameCheck: boolean`, `requiredNetworkName: string`, `campusWifi: ConnectionCampusStatus|null`, `campusWired: ConnectionCampusStatus|null`。
  - `TauriApi` (第 48-153 行): 定义全部 IPC 方法签名。关键项：
    - `getConfig` (第 49 行) / `saveConfig` (第 50 行)
    - `exportConfig` (第 52 行) / `importConfig` (第 53 行) / `exportDiagnostics` (第 54 行) — 桌面专属，实现中 reject
    - `getAdapters` (第 55 行) / `enableAdapter` (第 56 行) / `getAdapterDetails` (第 57 行) — 桌面专属 reject
    - `bindToWifi` (第 58 行), `checkPortalStatus` (第 59 行), `checkCampusStatus` (第 60 行), `requestWifiSsidPermission` (第 61 行)
    - `doLogin` (第 62 行), `doLogout` (第 63 行), `bindOperator` (第 64 行), `getBindStatus` (第 65 行)
    - `verifyWindowsIdentity` (第 66 行) — 安卓走生物识别/人脸
    - `revealOperatorCredential` (第 67 行), `querySelfDashboard` (第 68 行), `selfOfflineSession` (第 69 行), `querySelfOnlineLog` (第 70 行), `changeSelfPassword` (第 71 行)
    - `vpnSaveCredential` (第 73 行), `vpnCredentialStatus` (第 75 行), `vpnClearCredential` (第 77 行), `setSelfServiceViaVpn` (第 79 行), `vpnTestConnection` (第 81 行), `vpnVerifyCredential` (第 83 行)
    - `minimizeWindow` (第 84 行) / `closeWindow` (第 85 行) — 桌面专属 reject
    - `onBackgroundCheckResult` (第 86 行), `onAutoLoginResult` (第 87 行), `onAdaptersChanged` (第 88 行, noop), `onAdapterDetailsChanged` (第 89 行, noop), `onAdapterDisabledWarning` (第 90 行, noop), `onLoginLog` (第 91 行)
    - `listAccounts` (第 92 行), `createAccount` (第 94 行), `switchAccount` (第 95 行), `saveCurrentAsAccount` (第 96 行), `deleteAccount` (第 97 行), `renameAccount` (第 99 行), `getActiveAccount` (第 100 行)
    - `startBackgroundCheck` (第 101 行), `stopBackgroundCheck` (第 102 行), `triggerBackgroundCheck` (第 103 行), `getBackgroundStatus` (第 104 行)
    - `dhcpRenewAll` (第 105 行) / `dhcpReleaseRenew` (第 106 行) / `dhcpReleaseRenewAdapter` (第 107 行) — 桌面专属 reject
    - `checkNetworkQuality` (第 108 行), `onNetworkQualityResult` (第 109 行), `startLatencyTest` (第 110 行), `stopLatencyTest` (第 111 行)
    - `openExternal` (第 112 行), `getAutoLaunch` (第 113 行), `setAutoLaunch` (第 114 行), `getNotificationEnabled` (第 115 行), `setNotificationEnabled` (第 116 行)
    - `getBatteryOptimizationInfo` (第 117 行), `requestIgnoreBatteryOptimizations` (第 118 行), `openVendorBatterySettings` (第 119 行), `openNotificationSettings` (第 120 行)
    - `cancelAutoExit` (第 121 行) / `onAutoExitCountdown` (第 122 行) / `onAutoExitCancelled` (第 123 行) / `onCampusExitCountdown` (第 124 行) / `onCampusExitCancelled` (第 125 行) — 桌面专属或 noop
    - `onConfigChanged` (第 126 行), `showWindow` (第 127 行) — 桌面专属 reject
    - `getLogs` (第 128 行), `clearLogs` (第 129 行), `getDebugMode` (第 130 行), `setDebugMode` (第 131 行), `getInitData` (第 132 行)
    - `checkUpdate` (第 133 行), `downloadUpdate` (第 134 行), `installUpdate` (第 135 行), `getMirrorUrls` (第 136 行), `onDownloadProgress` (第 137 行), `onUpdateAvailable` (第 138 行), `onUpdateUnverified` (第 140 行)
    - `checkDnsDohStatus` (第 141 行) / `setupDnsDoh` (第 142 行) / `resetDns` (第 143 行) — 桌面专属 reject
    - `renderHeartbeat` (第 144 行) / `notifyWindowReady` (第 146 行) — 桌面专属 reject
    - `getLogRetentionDays` (第 147 行), `setLogRetentionDays` (第 148 行)
    - `getAvoidBadWifiStatus` (第 150 行), `restoreWrittenSettings` (第 152 行)
  - `AvoidBadWifiStatus` (第 155-164 行, export): 字段 `granted: boolean` (WRITE_SECURE_SETTINGS 授权), `avoidBadWifi: number` (当前值, -1 读不到), `pendingRestoreAvoid: boolean` (快照待还原), `pendingRestoreCaptive: boolean` (快照待还原)。
  - `RestoreWrittenSettingsResult` (第 166-169 行, export): 字段 `restored: Record<string, boolean>`, `reason?: string`。

- **实现对象**
  - `tauriApi: TauriApi` (第 202-342 行): 具体 `invoke` 映射。
    - `getConfig`→`invoke('get_config')` (第 203 行), `saveConfig`→`invoke('save_config')` (第 204 行)
    - 桌面专属 reject: `exportConfig` (第 205 行), `importConfig` (第 206 行), `exportDiagnostics` (第 207 行), `getAdapters` (第 208 行), `enableAdapter` (第 209 行), `getAdapterDetails` (第 210 行), `minimizeWindow` (第 268 行), `closeWindow` (第 269 行), `dhcpRenewAll` (第 287 行), `dhcpReleaseRenew` (第 288 行), `dhcpReleaseRenewAdapter` (第 289 行), `cancelAutoExit` (第 314 行), `showWindow` (第 320 行), `checkDnsDohStatus` (第 333 行), `setupDnsDoh` (第 334 行), `resetDns` (第 335 行), `renderHeartbeat` (第 336 行), `notifyWindowReady` (第 337 行)
    - noop 事件: `onAdaptersChanged` (第 272 行), `onAdapterDetailsChanged` (第 273 行), `onAdapterDisabledWarning` (第 274 行), `onAutoExitCountdown` (第 315 行), `onAutoExitCancelled` (第 316 行), `onCampusExitCountdown` (第 317 行), `onCampusExitCancelled` (第 318 行)
    - `verifyWindowsIdentity` (第 219-256 行): 集成 `biometricAuthenticate` 与 2D 人脸回退 `useFaceDialogStore`，失败归一化 `{success:false}` 不上抛
    - `openExternal` (第 294-305 行): URL 校验后调 `openerOpenUrl`
    - `getAutoLaunch`→`invoke('get_boot_autostart')` (第 306 行), `setAutoLaunch` (第 307 行)
    - 账号相关: `listAccounts` (第 276 行), `createAccount` (第 277 行), `switchAccount` (第 278 行), `saveCurrentAsAccount` (第 279 行), `deleteAccount` (第 280 行), `renameAccount` (第 281 行), `getActiveAccount` (第 282 行)
    - VPN 相关: `vpnSaveCredential` (第 262 行), `vpnCredentialStatus` (第 263 行), `vpnClearCredential` (第 264 行), `setSelfServiceViaVpn` (第 265 行), `vpnTestConnection` (第 266 行), `vpnVerifyCredential` (第 267 行)
    - 其他 invoke 映射见第 211-218 行, 第 257-261 行, 第 270-271 行, 第 275 行, 第 283-286 行, 第 290-293 行, 第 308-313 行, 第 319 行, 第 321-332 行, 第 338-342 行
  - `tauriApiWithRetry: TauriApi` (第 374-380 行): 展开 `tauriApi` 并仅对 `saveConfig` (第 379 行) 套 `withRetry`，其余高频命令不重试避免流量放大。

### android/frontend/public/mock-tauri.js

- `transformCallback(callback, once)` (第 24-36 行): 在 window 上定义 `_cb{N}` 属性供回调。
- `mockConfig` (第 38-57 行): 模拟配置，含 `user`, `operator`, `activeAccount`, `logRetentionDays: 7`, `configSchemaVersion: 7` 等。
- `campusStatus` (第 59-64 行) / `backgroundStatus` (第 66-72 行) / `mockAccounts` (第 76-80 行) / `initData` (第 82-88 行): 静态模拟数据。
- `handlers` (第 90-153 行): 命令处理函数映射。
  - `get_config` (第 91 行), `save_config` (第 92 行), `get_init_data` (第 93 行), `get_active_account` (第 94 行), `list_accounts` (第 95 行)
  - `create_account` (第 98-109 行): 校验重名并 push
  - `save_current_as_account` (第 110-119 行), `rename_account` (第 120-125 行), `delete_account` (第 126-130 行)
  - `get_adapters` (第 131 行), `get_adapter_details` (第 132 行), `check_campus_status` (第 134 行), `get_background_status` (第 135 行)
  - `get_avoid_bad_wifi_status` (第 148 行) 返回 `{ authorized: false, currentValue: null, pendingRestore: false }` —— 字段与真实 `AvoidBadWifiStatus` 不符
  - `do_login` (第 149 行 返回演示不执行), `do_logout` (第 150 行), `bind_to_wifi` (第 151 行), `request_wifi_ssid_permission` (第 152 行)
- `window.__TAURI_INTERNALS__` (第 155-177 行): 伪造 `invoke` (第 161-176 行)，对 `plugin:event|listen` 特殊处理 (第 162-169 行)，未列命令兜底返回 `null` (第 174-175 行)。
- `window.__mockEmitEvent` (第 189-195 行): 手动发射事件给监听者。

### android/frontend/public/mock-tauri-addon.js

- 覆盖 `invoke` (第 13-44 行): 劫持 `check_campus_status` (第 14-20 行), `get_adapters` (第 21-27 行), `check_portal_status` (第 28-30 行), `do_login` (第 31 行), `get_background_status` (第 32 行), `get_init_data` (第 33-42 行 注入 operator/night 逻辑)。
- `bgStatus()` (第 46-56 行): 构造背景状态。
- 定时器 (第 58-69 行): 周期性 emit `background-check-result` 事件；tab 切换 (第 60-65 行)；emuw 宽度模拟 (第 71-76 行)；probe 诊断 (第 77-84 行)。

### android/src-tauri/src/lib.rs

- `mod` 声明 (第 12-28 行): 包含 `account_cmds`, `config_state`, `monitor_loop`, `self_service_cmds`, `identity_gate` 等。
- `run()` (第 33-140 行): 构建 Tauri 应用，移动端注册插件 (第 35-42 行)，`setup` 初始化日志与启动任务 (第 47-64 行)，`invoke_handler` 通过 `generate_handler!` 注册全部命令 (第 68-130 行)，包含 `account_cmds::list_accounts` (第 95 行), `switch_account` (第 96 行), `rename_account` (第 97 行), `save_current_as_account` (第 98 行), `create_account` (第 99 行), `delete_account` (第 100 行), `get_active_account` (第 101 行) 等。退出护栏 `ExitRequested` 阻止默认退出 (第 137-139 行)。

### android/src-tauri/src/account_cmds.rs

- `ACCOUNT_NAME_RE` (第 12-15 行): 账号名白名单正则 `^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$`。
- `config_io_lock()` (第 20-22 行): 转发 `config_state::config_io_lock` 保证 IO 互斥。
- `AccountItem` struct (第 27-40 行): 字段 `id: String` (第 29 行), `#[serde(rename="displayName")] display_name: String` (第 30-31 行), `user: String` (第 34-35 行), `operator: String` (第 36-37 行), `#[serde(rename="passwordSaved")] password_saved: bool` (第 38-39 行)。用于账号列表出站。
- `AccountResult` struct (第 42-55 行): `#[serde(rename_all="camelCase")]` 字段 `success: bool` (第 45 行), `message: Option<String>` (第 46-47 行), `active_account: Option<String>` (第 48-49 行), `display_name: Option<String>` (第 50-52 行), `config: Option<serde_json::Value>` (第 53-54 行)。
  - 方法 `ok(config)` (第 58-60 行): 成功无账号。
  - 方法 `ok_with_account(account, config)` (第 61-63 行): 成功带激活账号。
  - 方法 `err(msg)` (第 64-66 行): 失败带消息。
- `sanitize_account_id(user)` (第 72-83 行): 非法字符转 `_`，截断 32。
- `validate_account_name(name)` (第 86-94 行): 长度 1-32 且匹配正则。
- `accounts_dir(app)` (第 96-102 行): 返回 `app_data_dir/accounts`。
- `read_account_card_info(path)` (第 111-121 行): 读展示字段，密码保存态走 `config_state::password_saved_at`。
- `list_account_items_sync(dir)` (第 126-155 行): 同步列举，按 id 排序，空显示名兜底 id。
- `#[tauri::command] list_accounts(app)` (第 157-163 行): Spawn blocking 调用上述同步函数。
- `load_account_file(path, bridge)` (第 165-170 行): 加载解密账号文件。
- `persist_current(app, merged)` (第 172-183 行): 保存主配置并刷缓存，返回掩码配置。
- `#[tauri::command] switch_account(app, account_name)` (第 185-227 行): 校验名、合并登录字段、清夜切标记、调 `restore_avoid_bad_wifi`。
- `#[tauri::command] save_current_as_account(app, account_name)` (第 229-257 行): 调用 `merge_save_as_target` 落盘并同步主配置。
- `#[tauri::command] create_account(app, account_name, user, password, operator)` (第 259-294 行): 新建档案不切换，已存在报错。
- `merge_save_as_target(current, existing, safe_name)` (第 302-331 行): 纯函数，处理另存/覆盖逻辑，返回 `(Settings, String)`。
- `#[tauri::command] delete_account(app, account_name)` (第 333-362 行): 删文件，若删当前则清 active。
- `#[tauri::command] get_active_account(app)` (第 364-367 行): 返回主配置 active_account。
- `#[tauri::command] rename_account(app, account_id, display_name)` (第 369-401 行): 调核心函数，若改当前则同步主配置。
- `rename_account_core(dir, bridge, account_id, display_name)` (第 405-429 行): 异步核心，校验重名、只改 displayName。
- `validate_display_name(name)` (第 434-447 行): trim 后 1-32 字符，禁控制字符。
- `auto_create_account_for_current(app, current)` (第 451-460 行): 保存后自动建号入口。
- `auto_create_account_in(dir, bridge, config)` (第 473-510 行): 自动建号/撞库防护/幂等更新。
- 测试模块 `#[cfg(test)] mod tests` (第 512-838 行): 含 `账号名消毒_放行合法字符` (第 517-521 行), `账号名消毒_拒绝路径穿越与非法字符` (第 523-531 行), `账号名消毒_长度边界` (第 533-537 行), `列举账号_过滤隐藏与排序_空显示名兜底为id` (第 539-559 行), `账号id消毒_边界与桌面同构` (第 588-601 行), `显示名校验_边界` (第 603-618 行), `自动建号_无凭据跳过` (第 620-629 行), `自动建号_新号_原始user作显示名_文件内active为id` (第 631-651 行), `自动建号_幂等_一致不写盘` (第 653-673 行), `自动建号_差异更新凭据_保留显示名` (第 675-698 行), `改名核心_成功_文件名与凭据保留` (第 700-719 行), `改名核心_拒绝_空_超长_重名_不存在` (第 721-757 行), `账号条目_json形状` (第 761-778 行), `另存账号_显示名不串号_主配置兜底新id` (第 782-799 行), `另存账号_覆盖同id档案_保留自定义显示名` (第 802-815 行), `自动建号_撞库_同id不同user原文_跳过不覆盖` (第 819-837 行) 等单元测试。

## Architecture

```text
┌──────────────────────┐   invoke    ┌──────────────────────┐
│ tauriApi.ts          │ ──────────► │ lib.rs              │
│ (第202-342行)        │             │ generate_handler    │
│ TauriApi 接口第48-153│             │ (第68-130行)        │
└──────────────────────┘             └──────────────────────┘
         │ 开发 Mock 拦截                     │ 分派命令
         ▼                                   ▼
┌──────────────────────┐             ┌──────────────────────┐
│ mock-tauri.js        │             │ account_cmds.rs      │
│ (第155-177行 inject) │             │ (第157-401行 commands│
│ mock-tauri-addon.js  │             │ 读写 accounts/*.json │
│ (第13-44行 override) │             └──────────────────────┘
└──────────────────────┘                      │
         │                                   ▼
         └──────────────────────────────────►│ config_state         │
                                             │ (CryptoBridge 加密)  │
                                             └──────────────────────┘
```

## Data Flow

### 生产环境调用流

1. 前端 Store/Hook 调用 `tauriApi` 方法（如第 276 行 `listAccounts` → `invoke('list_accounts')`）。
2. Tauri 序列化经 JNI 桥接至 Rust `lib.rs` 注册的命令（第 95 行 `account_cmds::list_accounts`）。
3. Rust 侧持 `config_io_lock` (第 198 行等) 读写 `accounts_dir` (第 96-102 行) 下 JSON，经 `CryptoBridge` 加解密（第 165-170 行）。
4. 返回 `AccountResult`/`AccountItem` 等 Serde 序列化结构，前端更新 Zustand。

### Mock 环境调用流

1. 浏览器加载 `mock-tauri.js` 注入 `window.__TAURI_INTERNALS__`（第 155 行）。
2. `tauriApi.ts` 的 `invoke` 被 Mock 拦截，命中 `handlers` 返回静态数据（第 172 行）。
3. `mock-tauri-addon.js` 进一步覆盖 `invoke`（第 13 行）注入场景参数（?op=&night=）。
4. 通过 `__mockEmitEvent`（第 189 行）模拟后台事件流。

### 异常与降级流

- 桌面专属命令（如 `exportConfig` 第 205 行, `getAdapters` 第 208 行）在安卓走 `desktopOnly` 直接 reject，前端需 catch 或类型上禁用。
- `verifyWindowsIdentity` (第 219-256 行) 在安卓优先 2D 人脸回退（`shouldUseFaceFallback`），否则 `biometricAuthenticate` 系统生物识别，失败归一化为 `{success:false}` 不上抛 IPC 故障。
- `withRetry` (第 356-372 行) 仅包裹 `saveConfig` (第 379 行)，防磁盘写入瞬时失败。

## Connections

后端模块依赖见 `lib.rs` 第 12-28 行声明：[[config_state]]（账号加密落盘）、[[monitor_loop]]（切账号后调 `restore_avoid_bad_wifi` 第 225 行, 第 358 行）、[[identity_gate]]（验证门在 `self_service_cmds` 触发）、[[self_service_cmds]]（六命令同模块面）。前端依赖 [[android-frontend-hooks-stores]]（Zustand 调 `tauriApi`）、[[android-tauri-shell-and-plugins]]（Biometric/Keystore 插件）。

## Known Issues

- **Mock 字段契约不一致**：`mock-tauri.js` 第 148 行 `get_avoid_bad_wifi_status` 返回 `{ authorized: false, currentValue: null, pendingRestore: false }`，但真实前端类型 `AvoidBadWifiStatus`（tauriApi.ts 第 155-164 行）字段为 `granted`/`avoidBadWifi`/`pendingRestoreAvoid`/`pendingRestoreCaptive`，Mock 形状错误若被前端读取会导致 `undefined`。
- **Mock 缺失命令**：`mock-tauri.js` 未实现 `check_portal_status`（仅 addon 第 28-30 行覆盖），纯基础 Mock 下该命令返回 `null`（第 174-175 行兜底），而前端 `checkPortalStatus` 期望 `PortalStatusResult`。
- **桌面专属接口暴露**：`TauriApi` 接口仍声明 `exportConfig`/`getAdapters` 等桌面方法（第 52-57 行等），安卓实现直接 reject，若调用方未检查 `desktopOnly` 错误将触发 unhandled rejection。
- **命名遗留**：`verifyWindowsIdentity`（tauriApi.ts 第 66 行, 第 219 行）名称含 Windows，但实际在安卓走 `biometricAuthenticate` 与 `useFaceDialogStore` 人脸，语义已泛化但命名未改。
- **测试编译隔离**：`account_cmds.rs` 测试（第 512-838 行）仅在 `#[cfg(test)]` 下编译，不进生产二进制，但 `auto_create_account_in` 等核心逻辑依赖测试覆盖保证正确性。