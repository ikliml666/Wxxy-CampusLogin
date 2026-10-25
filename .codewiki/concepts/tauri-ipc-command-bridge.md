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

`tauri-ipc-command-bridge` 是安卓端 Tauri 前后端通信契约层，连接 `android/frontend/src/hooks/tauriApi.ts`（TypeScript）与 `android/src-tauri/src/lib.rs` 注册的 Rust 命令（如 `account_cmds.rs`）。它定义了 `TauriApi` 接口（tauriApi.ts L48-151）枚举全部 IPC 方法，由 `tauriApi` 对象（L200-339）通过 `@tauri-apps/api/core` 的 `invoke` 实现，部分命令在安卓端显式拒绝（桌面专属，L14-15 `desktopOnly`、L203-208、L265-266、L284-286、L311、L317、L330-334 等）。开发环境由 `mock-tauri.js`（L155-177）与 `mock-tauri-addon.js`（L13-44）注入 `window.__TAURI_INTERNALS__` 拦截 `invoke`，实现无原生后端的 UI 验证。Rust 侧通过 `lib.rs` L66-122 的 `generate_handler!` 注册命令，账号相关命令在 `account_cmds.rs` L157-401 实现，并返回 `AccountResult`（L42-55）等结构化类型。

## Key Components

### android/frontend/src/hooks/tauriApi.ts

- **常量/辅助函数**
  - `desktopOnly<T>(name: string)` (L14-15): 返回 `Promise.reject`，提示桌面专属功能安卓不可用。
  - `noopListener` (L18): 返回 `() => {}` 的空监听工厂，用于桌面专属事件的安卓空实现。
  - `createEventListener<T>(eventName: string)` (L169-198): 封装 `@tauri-apps/api/event` 的 `listen`，返回带清理函数的订阅器，处理未完成监听的取消。
  - `isRetryableError(e: unknown)` (L341-351): 判断错误是否含 timeout/network/fetch/connection 关键字。
  - `withRetry<T>(fn, maxRetries=2, baseDelay=500)` (L353-369): 指数退避重试，仅用于可重试错误。

- **接口/类型**
  - `ConnectionCampusStatus` (L20-24): 字段 `onCampus: boolean`, `name: string | null`, `message: string`。
  - `BatteryOptimizationInfo` (L26-30, export): 字段 `ignoring: boolean`, `brand: string`, `hasVendorTarget: boolean`。
  - `VendorSettingsResult` (L32-36, export): 字段 `path: 'vendor'|'vendor_action'|'app_details'|'settings'|'none'`, `target: string`, `tried: string[]`。
  - `CampusStatusResult` (L38-46): 字段 `onCampusNetwork: boolean`, `currentSsid: string|null`, `campusMessage: string`, `enableNetworkNameCheck: boolean`, `requiredNetworkName: string`, `campusWifi: ConnectionCampusStatus|null`, `campusWired: ConnectionCampusStatus|null`。
  - `TauriApi` (L48-151): 定义全部 IPC 方法签名。关键项：
    - `getConfig` (L49) / `saveConfig` (L50)
    - `exportConfig` (L52) / `importConfig` (L53) / `exportDiagnostics` (L54) — 桌面专属，实现中 reject
    - `getAdapters` (L55) / `enableAdapter` (L56) / `getAdapterDetails` (L57) — 桌面专属 reject
    - `bindToWifi` (L58), `checkPortalStatus` (L59), `checkCampusStatus` (L60), `requestWifiSsidPermission` (L61)
    - `doLogin` (L62), `doLogout` (L63), `bindOperator` (L64), `getBindStatus` (L65)
    - `verifyWindowsIdentity` (L66) — 安卓走生物识别/人脸
    - `revealOperatorCredential` (L67), `querySelfDashboard` (L68), `selfOfflineSession` (L69), `querySelfOnlineLog` (L70), `changeSelfPassword` (L71)
    - `vpnSaveCredential` (L73), `vpnCredentialStatus` (L75), `vpnClearCredential` (L77), `setSelfServiceViaVpn` (L79), `vpnTestConnection` (L81)
    - `minimizeWindow` (L82) / `closeWindow` (L83) — 桌面专属 reject
    - `onBackgroundCheckResult` (L84), `onAutoLoginResult` (L85), `onAdaptersChanged` (L86, noop), `onAdapterDetailsChanged` (L87, noop), `onAdapterDisabledWarning` (L88, noop), `onLoginLog` (L89)
    - `listAccounts` (L90), `createAccount` (L92), `switchAccount` (L93), `saveCurrentAsAccount` (L94), `deleteAccount` (L95), `renameAccount` (L97), `getActiveAccount` (L98)
    - `startBackgroundCheck` (L99), `stopBackgroundCheck` (L100), `triggerBackgroundCheck` (L101), `getBackgroundStatus` (L102)
    - `dhcpRenewAll` (L103) / `dhcpReleaseRenew` (L104) / `dhcpReleaseRenewAdapter` (L105) — 桌面专属 reject
    - `checkNetworkQuality` (L106), `onNetworkQualityResult` (L107), `startLatencyTest` (L108), `stopLatencyTest` (L109)
    - `openExternal` (L110), `getAutoLaunch` (L111), `setAutoLaunch` (L112), `getNotificationEnabled` (L113), `setNotificationEnabled` (L114)
    - `getBatteryOptimizationInfo` (L115), `requestIgnoreBatteryOptimizations` (L116), `openVendorBatterySettings` (L117), `openNotificationSettings` (L118)
    - `cancelAutoExit` (L119) / `onAutoExitCountdown` (L120) / `onAutoExitCancelled` (L121) / `onCampusExitCountdown` (L122) / `onCampusExitCancelled` (L123) — 桌面专属或 noop
    - `onConfigChanged` (L124), `showWindow` (L125) — 桌面专属 reject
    - `getLogs` (L126), `clearLogs` (L127), `getDebugMode` (L128), `setDebugMode` (L129), `getInitData` (L130)
    - `checkUpdate` (L131), `downloadUpdate` (L132), `installUpdate` (L133), `getMirrorUrls` (L134), `onDownloadProgress` (L135), `onUpdateAvailable` (L136), `onUpdateUnverified` (L138)
    - `checkDnsDohStatus` (L139) / `setupDnsDoh` (L140) / `resetDns` (L141) — 桌面专属 reject
    - `renderHeartbeat` (L142) / `notifyWindowReady` (L144) — 桌面专属 reject
    - `getLogRetentionDays` (L145), `setLogRetentionDays` (L146)
    - `getAvoidBadWifiStatus` (L148), `restoreWrittenSettings` (L150)
  - `AvoidBadWifiStatus` (L153-162, export): 字段 `granted: boolean` (WRITE_SECURE_SETTINGS 授权), `avoidBadWifi: number` (当前值, -1 读不到), `pendingRestoreAvoid: boolean` (快照待还原), `pendingRestoreCaptive: boolean` (快照待还原)。
  - `RestoreWrittenSettingsResult` (L164-167, export): 字段 `restored: Record<string, boolean>`, `reason?: string`。

- **实现对象**
  - `tauriApi: TauriApi` (L200-339): 具体 `invoke` 映射。
    - `getConfig`→`invoke('get_config')` (L201), `saveConfig`→`invoke('save_config')` (L202)
    - 桌面专属 reject: `exportConfig` (L203), `importConfig` (L204), `exportDiagnostics` (L205), `getAdapters` (L206), `enableAdapter` (L207), `getAdapterDetails` (L208), `minimizeWindow` (L265), `closeWindow` (L266), `dhcpRenewAll` (L284), `dhcpReleaseRenew` (L285), `dhcpReleaseRenewAdapter` (L286), `cancelAutoExit` (L311), `showWindow` (L317), `checkDnsDohStatus` (L330), `setupDnsDoh` (L331), `resetDns` (L332), `renderHeartbeat` (L333), `notifyWindowReady` (L334)
    - noop 事件: `onAdaptersChanged` (L269), `onAdapterDetailsChanged` (L270), `onAdapterDisabledWarning` (L271), `onAutoExitCountdown` (L312), `onAutoExitCancelled` (L313), `onCampusExitCountdown` (L314), `onCampusExitCancelled` (L315)
    - `verifyWindowsIdentity` (L217-254): 集成 `biometricAuthenticate` 与 2D 人脸回退 `useFaceDialogStore`，失败归一化 `{success:false}` 不上抛
    - `openExternal` (L291-302): URL 校验后调 `openerOpenUrl`
    - `getAutoLaunch`→`invoke('get_boot_autostart')` (L303), `setAutoLaunch` (L304)
    - 账号相关: `listAccounts` (L273), `createAccount` (L274), `switchAccount` (L275), `saveCurrentAsAccount` (L276), `deleteAccount` (L277), `renameAccount` (L278), `getActiveAccount` (L279)
    - 其他 invoke 映射见 L209-264, L267-268, L272, L280-283, L287-290, L295-310, L316, L318-329, L335-338
  - `tauriApiWithRetry: TauriApi` (L371-377): 展开 `tauriApi` 并仅对 `saveConfig` (L376) 套 `withRetry`，其余高频命令不重试避免流量放大。

### android/frontend/public/mock-tauri.js

- `transformCallback(callback, once)` (L24-36): 在 window 上定义 `_cb{N}` 属性供回调。
- `mockConfig` (L38-57): 模拟配置，含 `user`, `operator`, `activeAccount`, `logRetentionDays: 7`, `configSchemaVersion: 7` 等。
- `campusStatus` (L59-64) / `backgroundStatus` (L66-72) / `mockAccounts` (L76-80) / `initData` (L82-88): 静态模拟数据。
- `handlers` (L90-153): 命令处理函数映射。
  - `get_config` (L91), `save_config` (L92), `get_init_data` (L93), `get_active_account` (L94), `list_accounts` (L95)
  - `create_account` (L98-109): 校验重名并 push
  - `save_current_as_account` (L110-119), `rename_account` (L120-125), `delete_account` (L126-130)
  - `get_adapters` (L131), `get_adapter_details` (L132), `check_campus_status` (L134), `get_background_status` (L135)
  - `get_avoid_bad_wifi_status` (L148) 返回 `{ authorized: false, currentValue: null, pendingRestore: false }` —— 字段与真实 `AvoidBadWifiStatus` 不符
  - `do_login` (L149 返回演示不执行), `do_logout` (L150), `bind_to_wifi` (L151), `request_wifi_ssid_permission` (L152)
- `window.__TAURI_INTERNALS__` (L155-177): 伪造 `invoke` (L161-176)，对 `plugin:event|listen` 特殊处理 (L162-169)，未列命令兜底返回 `null` (L174-175)。
- `window.__mockEmitEvent` (L189-195): 手动发射事件给监听者。

### android/frontend/public/mock-tauri-addon.js

- 覆盖 `invoke` (L13-44): 劫持 `check_campus_status` (L14-20), `get_adapters` (L21-27), `check_portal_status` (L28-30), `do_login` (L31), `get_background_status` (L32), `get_init_data` (L33-42 注入 operator/night 逻辑)。
- `bgStatus()` (L46-56): 构造背景状态。
- 定时器 (L58-69): 周期性 emit `background-check-result` 事件；tab 切换 (L60-65)；emuw 宽度模拟 (L71-76)；probe 诊断 (L77-84)。

### android/src-tauri/src/lib.rs

- `mod` 声明 (L11-26): 包含 `account_cmds`, `config_state`, `monitor_loop`, `self_service_cmds`, `identity_gate` 等。
- `run()` (L31-132): 构建 Tauri 应用，移动端注册插件 (L34-40)，`setup` 初始化日志与启动任务 (L45-64)，`invoke_handler` 通过 `generate_handler!` 注册全部命令 (L66-122)，包含 `account_cmds::list_accounts` (L87), `switch_account` (L88), `rename_account` (L89), `save_current_as_account` (L90), `create_account` (L91), `delete_account` (L92), `get_active_account` (L93) 等。退出护栏 `ExitRequested` 阻止默认退出 (L129-131)。

### android/src-tauri/src/account_cmds.rs

- `ACCOUNT_NAME_RE` (L13-15): 账号名白名单正则 `^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$`。
- `config_io_lock()` (L20-22): 转发 `config_state::config_io_lock` 保证 IO 互斥。
- `AccountItem` struct (L27-40): 字段 `id: String` (L29), `#[serde(rename="displayName")] display_name: String` (L30-31), `user: String` (L34-35), `operator: String` (L36-37), `#[serde(rename="passwordSaved")] password_saved: bool` (L38-39)。用于账号列表出站。
- `AccountResult` struct (L42-55): `#[serde(rename_all="camelCase")]` 字段 `success: bool` (L45), `message: Option<String>` (L46-47), `active_account: Option<String>` (L48-49), `display_name: Option<String>` (L50-52), `config: Option<serde_json::Value>` (L53-54)。
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
- 测试模块 `#[cfg(test)] mod tests` (L512-838): 含 `账号名消毒_放行合法字符` (L517-521), `账号名消毒_拒绝路径穿越与非法字符` (L523-531), `账号名消毒_长度边界` (L533-537), `列举账号_过滤隐藏与排序_空显示名兜底为id` (L539-559), `账号id消毒_边界与桌面同构` (L588-601), `显示名校验_边界` (L603-618), `自动建号_无凭据跳过` (L620-629), `自动建号_新号_原始user作显示名_文件内active为id` (L631-651), `自动建号_幂等_一致不写盘` (L653-673), `自动建号_差异更新凭据_保留显示名` (L675-698), `改名核心_成功_文件名与凭据保留` (L700-719), `改名核心_拒绝_空_超长_重名_不存在` (L721-757), `账号条目_json形状` (L761-778), `另存账号_显示名不串号_主配置兜底新id` (L782-799), `另存账号_覆盖同id档案_保留自定义显示名` (L802-815), `自动建号_撞库_同id不同user原文_跳过不覆盖` (L819-837) 等单元测试。

## Architecture

```text
┌──────────────────────┐   invoke    ┌──────────────────────┐
│ tauriApi.ts          │ ──────────► │ lib.rs              │
│ (L200-339)           │             │ generate_handler    │
│ TauriApi 接口 L48-151│             │ (L66-122)           │
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

1. 前端 Store/Hook 调用 `tauriApi` 方法（如 L273 `listAccounts` → `invoke('list_accounts')`）。
2. Tauri 序列化经 JNI 桥接至 Rust `lib.rs` 注册的命令（L87 `account_cmds::list_accounts`）。
3. Rust 侧持 `config_io_lock` (L198 等) 读写 `accounts_dir` (L96-102) 下 JSON，经 `CryptoBridge` 加解密（L165-170）。
4. 返回 `AccountResult`/`AccountItem` 等 Serde 序列化结构，前端更新 Zustand。

### Mock 环境调用流

1. 浏览器加载 `mock-tauri.js` 注入 `window.__TAURI_INTERNALS__`（L155）。
2. `tauriApi.ts` 的 `invoke` 被 Mock 拦截，命中 `handlers` 返回静态数据（L172）。
3. `mock-tauri-addon.js` 进一步覆盖 `invoke`（L13）注入场景参数（?op=&night=）。
4. 通过 `__mockEmitEvent`（L189）模拟后台事件流。

### 异常与降级流

- 桌面专属命令（如 `exportConfig` L203, `getAdapters` L206）在安卓走 `desktopOnly` 直接 reject，前端需 catch 或类型上禁用。
- `verifyWindowsIdentity` (L217-254) 在安卓优先 2D 人脸回退（`shouldUseFaceFallback`），否则 `biometricAuthenticate` 系统生物识别，失败归一化为 `{success:false}` 不上抛 IPC 故障。
- `withRetry` (L353-369) 仅包裹 `saveConfig` (L376)，防磁盘写入瞬时失败。

## Connections

后端模块依赖见 `lib.rs` L11-26 声明：[[config_state]]（账号加密落盘）、[[monitor_loop]]（切账号后调 `restore_avoid_bad_wifi` L225, L358）、[[identity_gate]]（验证门在 `self_service_cmds` 触发）、[[self_service_cmds]]（六命令同模块面）。前端依赖 [[android-frontend-hooks-stores]]（Zustand 调 `tauriApi`）、[[android-tauri-shell-and-plugins]]（Biometric/Keystore 插件）。

## Known Issues

- **Mock 字段契约不一致**：`mock-tauri.js` L148 `get_avoid_bad_wifi_status` 返回 `{ authorized: false, currentValue: null, pendingRestore: false }`，但真实前端类型 `AvoidBadWifiStatus`（tauriApi.ts L153-162）字段为 `granted`/`avoidBadWifi`/`pendingRestoreAvoid`/`pendingRestoreCaptive`，Mock 形状错误若被前端读取会导致 `undefined`。
- **Mock 缺失命令**：`mock-tauri.js` 未实现 `check_portal_status`（仅 addon L28-30 覆盖），纯基础 Mock 下该命令返回 `null`（L174-175 兜底），而前端 `checkPortalStatus` 期望 `PortalStatusResult`。
- **桌面专属接口暴露**：`TauriApi` 接口仍声明 `exportConfig`/`getAdapters` 等桌面方法（L52-57, L55 等），安卓实现直接 reject，若调用方未检查 `desktopOnly` 错误将触发 unhandled rejection。
- **命名遗留**：`verifyWindowsIdentity`（tauriApi.ts L66, L217）名称含 Windows，但实际在安卓走 `biometricAuthenticate` 与 `useFaceDialogStore` 人脸，语义已泛化但命名未改。
- **测试编译隔离**：`account_cmds.rs` 测试（L512-838）仅在 `#[cfg(test)]` 下编译，不进生产二进制，但 `auto_create_account_in` 等核心逻辑依赖测试覆盖保证正确性。