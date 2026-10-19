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

`native-plugin-bridge` 是 Android Tauri 应用中连接前端 JavaScript/TypeScript 运行时与后端 Rust 原生逻辑的核心抽象层。它不仅仅是一个简单的 RPC 调用封装，更是一个包含**类型契约定义**、**错误重试策略**、**平台兼容性降级**以及**开发环境 Mock 模拟**的完整基础设施体系。

该概念的核心价值在于解耦 UI 层与原生实现层，使得前端能够在无真实后端或特定测试场景下独立运行，同时确保在发布版本中能够安全、高效地调用 Android 系统级能力（如 WiFi 绑定、生物识别、后台监控）。

## Key Components

### 1. `tauriApi` (生产环境核心接口)
*   **文件**: `android/frontend/src/hooks/tauriApi.ts`
*   **行号**: L48-L141 (`interface TauriApi`), L190-L324 (`const tauriApi`)
*   **描述**: 定义了所有后端命令的类型签名，并封装了 `invoke` 调用逻辑。这是前端代码中唯一直接感知“原生接口”的地方。
    *   **配置管理**: `getConfig` (L49), `saveConfig` (L50), `exportConfig` (L52, 桌面专属), `importConfig` (L53, 桌面专属).
    *   **账号管理**: `listAccounts` (L80), `createAccount` (L82), `switchAccount` (L83), `saveCurrentAsAccount` (L84), `deleteAccount` (L85), `renameAccount` (L87), `getActiveAccount` (L88).
    *   **网络状态**: `checkPortalStatus` (L59), `checkCampusStatus` (L60), `getAdapters` (L55, 桌面专属), `enableAdapter` (L56, 桌面专属), `getAdapterDetails` (L57, 桌面专属), `bindToWifi` (L58).
    *   **认证与验证**: `doLogin` (L62), `doLogout` (L63), `verifyWindowsIdentity` (L66, 安卓等价物为生物识别), `revealOperatorCredential` (L67).
    *   **后台监控**: `startBackgroundCheck` (L89), `stopBackgroundCheck` (L90), `triggerBackgroundCheck` (L91), `getBackgroundStatus` (L92).
    *   **DHCP/DNS**: `dhcpRenewAll` (L93, 桌面专属), `dhcpReleaseRenew` (L94, 桌面专属), `checkDnsDohStatus` (L129, 桌面专属), `setupDnsDoh` (L130, 桌面专属), `resetDns` (L131, 桌面专属).
    *   **窗口控制**: `minimizeWindow` (L72, 桌面专属), `closeWindow` (L73, 桌面专属), `showWindow` (L115, 桌面专属), `notifyWindowReady` (L134, 桌面专属).
    *   **事件监听**: `onBackgroundCheckResult` (L74), `onAutoLoginResult` (L75), `onAdaptersChanged` (L76, 桌面专属), `onAdapterDetailsChanged` (L77, 桌面专属), `onAdapterDisabledWarning` (L78, 桌面专属), `onLoginLog` (L79), `onNetworkQualityResult` (L97), `onConfigChanged` (L114), `onDownloadProgress` (L125), `onUpdateAvailable` (L126), `onUpdateUnverified` (L128), `onAutoExitCountdown` (L110, 桌面专属), `onAutoExitCancelled` (L111, 桌面专属), `onCampusExitCountdown` (L112, 桌面专属), `onCampusExitCancelled` (L113, 桌面专属).
    *   **其他**: `openExternal` (L100), `getAutoLaunch` (L101), `setAutoLaunch` (L102), `getNotificationEnabled` (L103), `setNotificationEnabled` (L104), `getBatteryOptimizationInfo` (L105), `requestIgnoreBatteryOptimizations` (L106), `openVendorBatterySettings` (L107), `openNotificationSettings` (L108), `cancelAutoExit` (L109, 桌面专属), `getLogs` (L116), `clearLogs` (L117), `getDebugMode` (L118), `setDebugMode` (L119), `getInitData` (L120), `checkUpdate` (L121), `downloadUpdate` (L122), `installUpdate` (L123), `getMirrorUrls` (L124), `renderHeartbeat` (L132, 桌面专属), `getLogRetentionDays` (L135), `setLogRetentionDays` (L136), `getAvoidBadWifiStatus` (L138), `restoreWrittenSettings` (L140).

### 2. `tauriApiWithRetry` (增强版 API)
*   **文件**: `android/frontend/src/hooks/tauriApi.ts`
*   **行号**: L356-L361
*   **描述**: 扩展自 `tauriApi`，仅对 `saveConfig` 添加了重试机制。
    *   **重试逻辑**: `withRetry` 函数 (L338-L354)，最大重试次数 2 次，基础延迟 500ms，指数退避。
    *   **可重试判断**: `isRetryableError` 函数 (L326-L336)，检查错误信息是否包含 'timeout', 'network', 'fetch', 'connection'。

### 3. Mock 基础设施
*   **`mock-tauri.js`**:
    *   **文件**: `android/frontend/public/mock-tauri.js`
    *   **行号**: L1-L197
    *   **描述**: 浏览器实测环境 shim，伪造 `__TAURI_INTERNALS__`，仅供 dev/截图验证 UI。
        *   **配置模拟**: `mockConfig` (L38-L57), `campusStatus` (L59-L64), `backgroundStatus` (L66-L72), `mockAccounts` (L76-L80), `initData` (L82-L88).
        *   **命令处理**: `handlers` 对象 (L90-L153)，处理 `get_config`, `save_config`, `get_init_data`, `get_active_account`, `list_accounts`, `create_account`, `save_current_as_account`, `rename_account`, `delete_account`, `get_adapters`, `get_adapter_details`, `get_disabled_adapters`, `check_campus_status`, `get_background_status`, `get_soc_info`, `get_notification_enabled`, `set_notification_enabled`, `get_boot_autostart`, `get_battery_optimization_info`, `get_debug_mode`, `set_debug_mode`, `get_logs`, `get_log_retention_days`, `check_campus_network`, `check_network_quality`, `check_update`, `get_avoid_bad_wifi_status`, `do_login`, `do_logout`, `bind_to_wifi`, `request_wifi_ssid_permission`.
        *   **事件系统**: `transformCallback` (L24-L36), `eventHandlers` (L22), `__mockEmitEvent` (L189-L195).
*   **`mock-tauri-addon.js`**:
    *   **文件**: `android/frontend/public/mock-tauri-addon.js`
    *   **行号**: L1-L86
    *   **描述**: 叠加在 `mock-tauri.js` 之上的场景注入，用于特定 URL 参数覆盖行为。
        *   **URL 参数解析**: `op` (运营商), `dual` (双网卡), `night` (夜切态), `online` (在线状态), `tab` (截图辅助), `emuw` (模拟器宽度), `probe` (诊断).
        *   **命令覆盖**: 拦截 `check_campus_status` (L14-L20), `get_adapters` (L21-L27), `check_portal_status` (L28-L30), `do_login` (L31), `get_background_status` (L32), `get_init_data` (L33-L42).
*   **`tauri-mock.js`**:
    *   **文件**: `android/frontend/tauri-mock.js`
    *   **行号**: L1-L273
    *   **描述**: Tauri 浏览器 mock，供本地浏览器测试（vite.mock.config.ts 注入），不进入构建产物。提供更完整的模拟数据，支持异步延迟、账号切换模拟。
        *   **配置模拟**: `config` (L32-L42), `mockAccounts` (L45-L49), `activeAccountName` (L51), `onlineList` (L59-L65), `loginHistory` (L67-L70), `billRows` (L72-L75), `billSummary` (L76), `mockAdapters` (L82-L84).
        *   **命令处理**: `handlers` 对象 (L87-L209)，处理 `get_init_data`, `get_config`, `save_config`, `query_self_dashboard`, `self_offline_session`, `query_self_online_log`, `change_self_password`, `verify_windows_identity`, `verify_biometric_identity`, `query_bind_status`, `check_campus_status`, `check_portal_status`, `render_heartbeat`, `get_active_account`, `list_accounts`, `create_account`, `save_current_as_account`, `rename_account`, `switch_account`, `delete_account`, `get_adapters`, `check_dns_doh_status`, `get_debug_mode`, `get_auto_launch`, `get_notification_enabled`, `get_log_retention_days`, `get_logs`, `check_update`, `start_background_check`, `stop_background_check`, `get_background_status`, `show_window`, `minimize_window`, `close_window`, `notify_window_ready`, `cancel_auto_exit`, `open_external`.
        *   **事件系统**: `eventListeners` (L233), `emitEvent` (L234-L242), `transformCallback` (L246-L250).

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                      Frontend (React)                       │
│  ┌───────────────────┐  ┌─────────────────────────────────┐ │
│  │ useAppInit /      │  │  tauriApi /                     │ │
│  │ useConfigStore    │  │  tauriApiWithRetry              │ │
│  └────────┬──────────┘  └──────────┬──────────────────────┘ │
│           │                        │ invoke()               │
├───────────┼────────────────────────┼────────────────────────┤
│           ▼                        ▼                        │
│  ┌─────────────────────────────────────────────────────────┐│
│  │              Mock Infrastructure                        ││
│  │  ┌─────────────────┐  ┌───────────────────────────────┐││
│  │  │ mock-tauri.js   │  │ mock-tauri-addon.js           │││
│  │  │ (Base Shim)     │  │ (Scenario Injection)          │││
│  │  └────────┬────────┘  └───────────┬───────────────────┘││
│  │           │                       │                     ││
│  │  ┌────────▼───────────────────────▼───────────────────┐││
│  │  │                  tauri-mock.js                      │││
│  │  │         (Full Mock with Async & State)             │││
│  │  └────────────────────────────────────────────────────┘││
├───────────┼────────────────────────┼────────────────────────┤
│           │                        │ invoke()               │
├───────────┼────────────────────────┼────────────────────────┤
│           ▼                        ▼                        │
│  ┌─────────────────────────────────────────────────────────┐│
│  │                   Tauri Core                            ││
│  │  ┌─────────────────────────────────────────────────────┐││
│  │  │            Rust Backend Commands                    │││
│  │  │  account_cmds.rs, monitor_loop.rs, etc.             │││
│  │  └─────────────────────────────────────────────────────┘││
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

### 1. 启动与初始化 (Initialization)
*   **构建阶段**: `frontend-build-assets` 中的 `index.html` 注入 `VITE_PLATFORM=android`。若为开发模式，加载 `public/mock-tauri.js` 等文件，劫持 `window.__TAURI_INTERNALS__.invoke`。
*   **运行时**: `frontend-hooks-store` 中的 `useAppInit` 触发 `useInitialDataLoad`。
*   **API 获取**: `useInitialDataLoad` 调用 `frontend-hooks-store` 导出的 `tauriApi.getConfig()`。

### 2. 调用分发 (Dispatch)
*   **生产环境**:
    *   `tauriApi` (L190-L324 in `tauriApi.ts`) 构造一个 invoke 请求。
    *   请求被发送至 Tauri 核心，路由至 `rust-backend-commands` 中对应的 `#[tauri::command]` 函数（如 `account_cmds.rs` 中的 `switch_account`）。
    *   Rust 后端执行 JNI 调用或文件系统操作，返回结果。
*   **Mock 环境**:
    *   `tauriApi` 调用被拦截。
    *   `mock-tauri.js` (L161-L176) 分发命令到 `mock-tauri-addon.js` (L12-L44)。
    *   `mock-tauri-addon.js` 根据 URL 参数（如 `?op=login`）覆盖行为，或直接由 `tauri-mock.js` (L87-L209) 提供预设数据结构。

### 3. 数据回传与状态更新 (Response & State Update)
*   **响应处理**: `tauriApi` 接收 JSON 响应。
*   **重试机制**: 若失败，`tauriApiWithRetry` (L356-L361) 介入，执行指数退避重试。
*   **状态同步**: 成功的数据被写入 `frontend-hooks-store` 中的 Zustand Stores（如 `useConfigStore`, `useAuthStore`）。
*   **UI 渲染**: `frontend-core-lib` 中的 `App.tsx` 读取 Store 变化，驱动 `frontend-mobile-components` 进行重绘。

### 4. 异常与降级 (Error & Fallback)
*   **桌面专属降级**: 当 `desktopOnly<T>` (L14-L15) 标记的方法在 Android 上被调用时，直接返回被拒绝的 Promise，防止崩溃。
*   **存储降级**: `safeStorage` (in `frontend-core-lib`) 在 `localStorage` 不可用时回退到内存 Map。

## Connections

*   [[frontend-hooks-store]]: 使用 `tauriApi` 和 `tauriApiWithRetry` 进行状态管理和数据获取。
*   [[rust-backend-commands]]: 提供 `tauriApi` 调用的实际执行逻辑。
*   [[frontend-core-lib]]: 通过 `main.tsx` 和 `App.tsx` 初始化全局状态，间接依赖桥接层提供的配置加载和数据同步能力。