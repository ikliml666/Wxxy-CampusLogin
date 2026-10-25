---
title: native-plugin-bridge
type: concept
source_files:
  - android/frontend/src/hooks/tauriApi.ts
  - android/frontend/public/mock-tauri.js
  - android/frontend/public/mock-tauri-addon.js
  - android/frontend/tauri-mock.js
tags: [Tauri, IPC, Bridge, Mocking, Android]
---

## Overview

`native-plugin-bridge` 是 Android Tauri 应用中连接前端 TypeScript 运行时与后端 Rust 原生逻辑的核心抽象层。它不仅封装了 `invoke` RPC 调用，还定义了前后端类型契约、错误重试策略、平台兼容性降级（桌面专属能力在安卓显式拒绝）以及开发环境 Mock 模拟体系。

该层解耦 UI 与原生实现，使前端可在无真实后端时独立运行（通过 `mock-tauri.js`、`mock-tauri-addon.js`、`tauri-mock.js` 注入内存实现），并在发布版本中安全调用安卓系统能力（WiFi 绑定、生物识别、后台监控、电池优化设置等）。

## Key Components

### 1. `tauriApi` (生产环境核心接口与实现)
*   **文件**: `android/frontend/src/hooks/tauriApi.ts`
*   **类型与常量**:
    *   `desktopOnly<T>` (L14-L15): 返回拒绝 Promise，提示“桌面专属功能,安卓端不可用”，用于标记安卓无对应物的桌面能力。
    *   `noopListener` (L18): 返回 no-op 清理函数，用于桌面专属事件的空监听。
    *   `BatteryOptimizationInfo` 接口 (L26-L30): 字段 `ignoring: boolean`, `brand: string`, `hasVendorTarget: boolean`。
    *   `VendorSettingsResult` 接口 (L32-L36): 字段 `path: 'vendor'|'vendor_action'|'app_details'|'settings'|'none'`, `target: string`, `tried: string[]`。
    *   `ConnectionCampusStatus` 接口 (L20-L24): 字段 `onCampus: boolean`, `name: string|null`, `message: string`。
    *   `CampusStatusResult` 接口 (L38-L46): 字段 `onCampusNetwork: boolean`, `currentSsid: string|null`, `campusMessage: string`, `enableNetworkNameCheck: boolean`, `requiredNetworkName: string`, `campusWifi: ConnectionCampusStatus|null`, `campusWired: ConnectionCampusStatus|null`。
    *   `AvoidBadWifiStatus` 导出接口 (L153-L162): 字段 `granted: boolean` (WRITE_SECURE_SETTINGS 是否授权), `avoidBadWifi: number` (当前值, -1读不到), `pendingRestoreAvoid: boolean`, `pendingRestoreCaptive: boolean`。
    *   `RestoreWrittenSettingsResult` 导出接口 (L164-L167): 字段 `restored: Record<string, boolean>`, `reason?: string`。
    *   `TauriApi` 接口 (L48-L151): 定义全部后端命令签名。
*   **接口方法（行号指接口声明，实现见 `const tauriApi` L200-L339）**:
    *   **配置管理**: `getConfig` (L49, 实现 L201), `saveConfig` (L50, L202), `exportConfig` (L52, L203 桌面专属拒绝), `importConfig` (L53, L204 桌面专属), `exportDiagnostics` (L54, L205 桌面专属)。
    *   **网络适配器（桌面专属）**: `getAdapters` (L55, L206 拒绝), `enableAdapter` (L56, L207 拒绝), `getAdapterDetails` (L57, L208 拒绝), `bindToWifi` (L58, L209 真实调用), `dhcpRenewAll` (L103, L284 拒绝), `dhcpReleaseRenew` (L104, L285 拒绝), `dhcpReleaseRenewAdapter` (L105, L286 拒绝)。
    *   **校园网状态**: `checkPortalStatus` (L59, L210), `checkCampusStatus` (L60, L211), `requestWifiSsidPermission` (L61, L212)。
    *   **认证登录**: `doLogin` (L62, L213), `doLogout` (L63, L214), `bindOperator` (L64, L215), `getBindStatus` (L65, L216 命令 `query_bind_status`), `verifyWindowsIdentity` (L66, L217-L254 安卓等价物为生物识别/2D人脸回退), `revealOperatorCredential` (L67, L255), `querySelfDashboard` (L68, L256), `selfOfflineSession` (L69, L257), `querySelfOnlineLog` (L70, L258), `changeSelfPassword` (L71, L259)。
    *   **VPN 自助凭据**: `vpnSaveCredential` (L73, L260), `vpnCredentialStatus` (L75, L261), `vpnClearCredential` (L77, L262), `setSelfServiceViaVpn` (L79, L263), `vpnTestConnection` (L81, L264)。
    *   **账号管理**: `listAccounts` (L90, L273), `createAccount` (L92, L274), `switchAccount` (L93, L275), `saveCurrentAsAccount` (L94, L276), `deleteAccount` (L95, L277), `renameAccount` (L97, L278), `getActiveAccount` (L98, L279)。
    *   **后台监控**: `startBackgroundCheck` (L99, L280), `stopBackgroundCheck` (L100, L281), `triggerBackgroundCheck` (L101, L282), `getBackgroundStatus` (L102, L283), `checkNetworkQuality` (L106, L287), `startLatencyTest` (L108, L289), `stopLatencyTest` (L109, L290)。
    *   **窗口控制（桌面专属）**: `minimizeWindow` (L82, L265 拒绝), `closeWindow` (L83, L266 拒绝), `showWindow` (L125, L317 拒绝), `notifyWindowReady` (L143-L144, L334 拒绝), `cancelAutoExit` (L119, L311 拒绝), `renderHeartbeat` (L142, L333 拒绝)。
    *   **事件监听**: `onBackgroundCheckResult` (L84, L267), `onAutoLoginResult` (L85, L268), `onAdaptersChanged` (L86, L269 noop), `onAdapterDetailsChanged` (L87, L270 noop), `onAdapterDisabledWarning` (L88, L271 noop), `onLoginLog` (L89, L272), `onNetworkQualityResult` (L107, L288), `onAutoExitCountdown` (L120, L312 noop), `onAutoExitCancelled` (L121, L313 noop), `onCampusExitCountdown` (L122, L314 noop), `onCampusExitCancelled` (L123, L315 noop), `onConfigChanged` (L124, L316), `onDownloadProgress` (L135, L327), `onUpdateAvailable` (L136, L328), `onUpdateUnverified` (L138, L329)。
    *   **系统/其他**: `openExternal` (L110, L291-L302 校验 http/https 并调用 opener), `getAutoLaunch` (L111, L303 命令 `get_boot_autostart`), `setAutoLaunch` (L112, L304), `getNotificationEnabled` (L113, L305), `setNotificationEnabled` (L114, L306), `getBatteryOptimizationInfo` (L115, L307), `requestIgnoreBatteryOptimizations` (L116, L308), `openVendorBatterySettings` (L117, L309), `openNotificationSettings` (L118, L310), `getLogs` (L126, L318), `clearLogs` (L127, L319), `getDebugMode` (L128, L320), `setDebugMode` (L129, L321), `getInitData` (L130, L322), `checkUpdate` (L131, L323), `downloadUpdate` (L132, L324), `installUpdate` (L133, L325), `getMirrorUrls` (L134, L326), `checkDnsDohStatus` (L139, L330 拒绝), `setupDnsDoh` (L140, L331 拒绝), `resetDns` (L141, L332 拒绝), `getLogRetentionDays` (L145, L335), `setLogRetentionDays` (L146, L336), `getAvoidBadWifiStatus` (L148, L337), `restoreWrittenSettings` (L150, L338)。

### 2. `tauriApiWithRetry` (增强版 API)
*   **文件**: `android/frontend/src/hooks/tauriApi.ts`
*   **行号**: L371-L377
*   **描述**: 通过展开 `tauriApi` 并覆盖 `saveConfig` 添加重试。仅 `saveConfig` 保留重试 (L376)，因为保存是关键一次性操作无其他兜底。
*   **辅助函数**:
    *   `isRetryableError` (L341-L351): 检查错误字符串或 Error.message 是否包含 'timeout','network','fetch','connection'（大小写不敏感）。
    *   `withRetry<T>` (L353-L369): 指数退避重试，最大重试 2 次，基础延迟 500ms，随机 jitter 200ms。

### 3. Mock 基础设施
*   **`mock-tauri.js`** (浏览器实测 shim，绝不入库)
    *   **文件**: `android/frontend/public/mock-tauri.js`
    *   **行号**: L1-L197
    *   **关键数据**: `mockConfig` (L38-L57), `campusStatus` (L59-L64), `backgroundStatus` (L66-L72), `mockAccounts` (L76-L80, 含 id/displayName/user/operator/passwordSaved), `initData` (L82-L88)。
    *   **命令处理**: `handlers` 对象 (L90-L153) 覆盖 `get_config`, `save_config`, `get_init_data`, `get_active_account`, `list_accounts`, `create_account` (L98-L109), `save_current_as_account` (L110-L119), `rename_account` (L120-L125), `delete_account` (L126-L130), `get_adapters` (L131), `get_adapter_details` (L132), `check_campus_status` (L134), `get_background_status` (L135), 及多个桌面/系统命令。
    *   **事件系统**: `transformCallback` (L24-L36), `eventHandlers` 映射 (L22), `window.__TAURI_INTERNALS__.invoke` shim (L161-L176) 路由命令, `__mockEmitEvent` (L189-L195) 发射事件。
*   **`mock-tauri-addon.js`** (场景注入，绝不入库)
    *   **文件**: `android/frontend/public/mock-tauri-addon.js`
    *   **行号**: L1-L86
    *   **描述**: 叠加在 base shim 上，解析 URL 参数 `op` (L6), `dual` (L7), `night` (L8), `online` (L9), `tab` (L10), `emuw` (L71), `probe` (L77)。重写 `invoke` (L13-L44) 拦截 `check_campus_status` (L14-L20), `get_adapters` (L21-L27), `check_portal_status` (L28-L30), `do_login` (L31), `get_background_status` (L32), `get_init_data` (L33-L42)。提供 `bgStatus` (L46-L56) 与定时器发射事件 (L59, L66-L69)。
*   **`tauri-mock.js`** (vite 注入浏览器 mock，不进构建产物)
    *   **文件**: `android/frontend/tauri-mock.js`
    *   **行号**: L1-L278
    *   **关键数据**: `config` (L32-L42), `mockAccounts` (L45-L49), `activeAccountName` (L51), `onlineList` (L59-L65), `loginHistory` (L67-L70), `billRows` (L72-L75), `billSummary` (L76), `mockAdapters` (L82-L84)。
    *   **命令处理**: `handlers` 对象 (L87-L214) 实现 `get_init_data`, `get_config`, `save_config` (L99-L110, 会 emit `config-changed`), `query_self_dashboard`, `self_offline_session`, `query_self_online_log`, `change_self_password`, `verify_windows_identity`, `verify_biometric_identity`, `query_bind_status` (L126-L136), `bind_operator` (L138-L141), `check_campus_status` (L144 平铺形状), `check_portal_status` (L145), `render_heartbeat`, 账号管理 `create_account` (L150-L163), `save_current_as_account` (L165-L175), `rename_account` (L176-L181), `switch_account` (L182-L190), `delete_account` (L191-L196), 以及适配器/调试/更新等。
    *   **事件系统**: `invoke` 分发 (L216-L235), `eventListeners` (L238), `emitEvent` (L239-L247), `transformCallback` (L251-L255)。支持 `?dialog=` 自动点击 (L263-L277)。

## Architecture

```text
┌──────────────────────────────────────────────────────────┐
│                    Frontend (React)                      │
│  useAppInit / Zustand Stores ──► tauriApi /              │
│                               tauriApiWithRetry          │
└───────────────┬───────────────────────────┬─────────────┘
                │ invoke()                  │ invoke()
┌───────────────▼──────────────┐ ┌──────────▼──────────────┐
│     Mock Infrastructure      │ │   Tauri Core (Android)  │
│ ┌─────────────────────────┐ │ │  Rust Backend Commands  │
│ │ mock-tauri.js (L1-L197) │ │ │  (account_cmds, etc.)   │
│ │ + mock-tauri-addon.js   │ │ └──────────────────────────┘
│ │   (L1-L86)              │ │
│ │ + tauri-mock.js (L1-L278)│ │
│ └─────────────────────────┘ │
└──────────────────────────────┘
```

## Data Flow

### 1. 启动与初始化
*   开发/截图环境：浏览器加载 `mock-tauri.js` (L1-L197) 伪造 `__TAURI_INTERNALS__`，若带 addon 则 `mock-tauri-addon.js` 重写 invoke；`tauri-mock.js` 由 vite 配置注入（L1-L278），均在 React 前执行。
*   生产环境：Tauri Android 提供真实 `__TAURI_INTERNALS__`，`tauriApi.ts` 直接 `invoke` 至 Rust。

### 2. 调用分发
*   生产：`tauriApi` 方法 (L200-L339) 调用 `invoke<Type>(cmd, args)`，路由到 Rust `#[tauri::command]`。例如 `switchAccount` (L275) 调用 `switch_account` 命令。
*   Mock：`window.__TAURI_INTERNALS__.invoke` 被 shim 替换（`mock-tauri.js` L161-L176 或 `tauri-mock.js` L216-L235），根据 cmd 名查 `handlers` 返回内存数据；`addon` 可拦截覆盖。

### 3. 数据回传与状态更新
*   `tauriApi` 接收 Promise 解析值。若 `saveConfig` 失败且错误可重试，`tauriApiWithRetry` (L376) 通过 `withRetry` (L353-L369) 指数退避重试。
*   成功数据写入前端 Zustand stores，驱动 UI 重绘。
*   Mock 中 `save_config` 等会 `setTimeout` 发射 `config-changed` 事件（`tauri-mock.js` L107, L173, L188），`onConfigChanged` (L316) 监听同步。

### 4. 异常与降级
*   桌面专属方法调用 `desktopOnly` (L14-L15) 直接 reject，防止安卓端误操作。
*   桌面专属事件监听绑定 `noopListener` (L18) 返回空清理函数。
*   `verifyWindowsIdentity` (L217-L254) 在安卓走生物识别或 2D 人脸回退，失败归一化上报 `{success:false}` 而非 IPC 故障。

## Connections

*   [[frontend-hooks-store]]: 使用 `tauriApi` 与 `tauriApiWithRetry` 获取/保存配置、账号、状态。
*   [[rust-backend-commands]]: 提供 `tauriApi` 调用的实际 Rust 命令实现（如 `switch_account`, `query_bind_status`）。
*   [[frontend-core-lib]]: 通过 `App.tsx` 初始化，依赖桥接层加载 `getInitData` (L322) 与配置同步。

## Known Issues

*   Mock 文件（`mock-tauri.js` L2, `mock-tauri-addon.js` L1, `tauri-mock.js` L1）明确标注“绝不入库”或“不进入构建产物”，仅用于本地浏览器验证，与生产 IPC 契约可能偏离。
*   桌面专属命令在安卓端统一拒绝（L14-L15），若前端误调用将获 rejection 而非兜底数据。
*   `mock-tauri.js` 中 `get_avoid_bad_wifi_status` 返回字段为 `authorized/currentValue/pendingRestore` (L148)，而真实 `AvoidBadWifiStatus` 接口字段为 `granted/avoidBadWifi/pendingRestoreAvoid/pendingRestoreCaptive` (L153-L162)，Mock 形状与类型契约不一致。