---
title: android-frontend-mock-tauri
type: module
source_files:
  - android/frontend/public/mock-tauri.js
  - android/frontend/public/mock-taddoon.js
  - android/frontend/tauri-mock.js
tags: [mock, tauri, frontend, testing, browser-shim]
---

## Overview

`android-frontend-mock-tauri` 模块由三个 JavaScript 文件组成，旨在为 Android 前端应用（基于 Tauri + React）提供浏览器环境下的 Tauri API 模拟层。由于 Tauri 依赖原生 WebView 和 Rust 后端，在纯浏览器环境（如 Vite 开发服务器、Headless Chrome 截图验证）中无法直接运行。

该模块通过拦截 `window.__TAURI_INTERNALS__` 对象，将前端对 Tauri 的异步调用（invoke）和事件监听（listen）重定向到内存中的模拟实现。它支持多种场景注入（如运营商切换、夜间模式、双网卡、离线状态），并包含针对自动化截图的辅助功能（如自动点击按钮、模拟窄屏宽度、禁用动画）。

**注意：** 所有 mock 文件均标记为临时用途（`TEMP-UI-MOCK`），在正式构建产物中应被排除，仅在开发或测试环境中加载。

## Key Components

### 1. android/frontend/public/mock-tauri.js
基础 Shim 层，定义在 `public/mock-tauri.js`。当 `window.__TAURI_INTERNALS__` 不存在时注入。

#### 全局配置与状态
- **mockConfig**: (行 38-57) 默认配置对象。
    - 字段包括：`user` ('23331150'), `password`, `selfPassword`, `operator` ('cmcc'), `themeMode` ('dark'), `requiredNetworkName` ('i-wxxy') 等。
    - 包含网络检查间隔 (`backgroundCheckInterval`: 30000)、日志保留天数 (`logRetentionDays`: 7) 等。
- **campusStatus**: (行 59-64) 校园网状态对象。
    - 字段：`onCampusNetwork` (true), `currentSsid` ('i-wxxy'), `campusWifi` (嵌套对象), `campusWired` (null)。
- **backgroundStatus**: (行 66-72) 后台检查状态。
    - 字段：`isRunning` (false), `checkCount` (0), `online` (false), `interval` (30000)。
- **mockAccounts**: (行 76-80) 预置账号列表。
    - 结构：`{ id, displayName, user, operator, passwordSaved }`。
    - 包含 'mock-001' (演示账号), 'mock-002' (室友号), 'mock-003' (备用)。
- **initData**: (行 82-88) 初始化数据聚合体，包含 config, version ('2.3.9-mock'), adapters, accounts 等。

#### 核心函数
- **transformCallback(callback, once)**: (行 24-36)
    - 作用：将 JS 回调函数包装为带唯一 ID 的属性挂在 `window` 上，以便 Tauri 事件系统查找。
    - 逻辑：生成 `_cb{id}` 属性，设置 `writable: false`。若 `once` 为真，执行后删除属性。
- **handlers**: (行 90-153) 命令分发映射表。
    - `get_config`: 返回 `mockConfig`。
    - `save_config`: 返回 `{ success: true }`。
    - `get_init_data`: 返回 `initData`。
    - `list_accounts`: 返回 `mockAccounts`。
    - `create_account(args)`: (行 98-109) 创建新账号。校验名称是否存在，推入 `mockAccounts`。
    - `save_current_as_account(args)`: (行 110-119) 保存当前配置为指定账号档案。更新 `acc.user` 和 `acc.operator`。
    - `rename_account(args)`: (行 120-125) 修改账号显示名称。
    - `delete_account(args)`: (行 126-130) 从 `mockAccounts` 过滤掉指定 ID。
    - `get_adapters`: 返回空数组 `[]`。
    - `check_campus_status`: 返回 `campusStatus`。
    - `do_login`: (行 149) 返回失败消息 "演示模式不执行登录"。
    - `do_logout`: (行 150) 返回成功消息 "演示模式"。
- **window.__TAURI_INTERNALS__**: (行 155-177)
    - `metadata`: 固定为 `{ currentWindow: { label: 'main' }, ... }`。
    - `transformCallback`: 引用上述函数。
    - `invoke(cmd, args)`: (行 161-176) 核心路由。
        - 处理 `plugin:event|listen`：注册事件监听器 ID。
        - 处理 `plugin:event|unlisten` / `plugin:window|*`：返回 null。
        - 匹配 `handlers[cmd]` 并执行。
        - 未匹配命令打印警告并返回 null。
- **window.__TAURI_EVENT_PLUGIN_INTERNALS__**: (行 180-184)
    - `unregisterListener(eventId)`: 清理 `window['_cb'+eventId]`。
- **window.__mockEmitEvent(name, payload)**: (行 189-195)
    - 作用：手动触发事件，用于测试背景检查等异步流程。遍历 `eventHandlers[name]` 并执行对应回调。
- **localStorage 初始化**: (行 10-19)
    - 强制设置 `campus-onboarding-done='1'` 跳过新手引导。
    - 根据 URL 参数 `?theme=dark` 控制浅色模式开关。

### 2. android/frontend/public/mock-tauri-addon.js
场景注入层，叠加在 `mock-tauri.js` 之上。定义在 `public/mock-tauri-addon.js`。

#### URL 参数解析
- **op**: (行 6) 运营商后缀，默认 '@telecom'。
- **dual**: (行 7) 是否启用双网卡（以太网），值为 '1' 时生效。
- **night**: (行 8) 是否启用夜切模式，值为 '1' 时生效。
- **online**: (行 9) 在线状态，`offline=1` 时为离线。
- **tab**: (行 10) 截图辅助，加载后自动点击指定 Dock 页签按钮。
- **emuw**: (行 71) 模拟窗口宽度，注入 CSS 强制 body 宽度。
- **probe**: (行 77) 诊断模式，将超出视口的元素信息写入 `<title>`。

#### 核心覆盖逻辑
- **invoke 重载**: (行 13-44)
    - 捕获原始 `origInvoke`。
    - `check_campus_status`: (行 14-20) 强制返回在线状态，SSID 为 'i-wxxy'。
    - `get_adapters`: (行 21-27)
        - 默认返回 WLAN 适配器。
        - 若 `dual` 为真，追加以太网适配器。
    - `check_portal_status`: (行 28-30) 根据 `online` 变量返回在线/离线状态。
    - `do_login`: (行 31) 返回演示模式提示。
    - `get_background_status`: (行 32) 调用内部 `bgStatus()` 函数。
    - `get_init_data`: (行 33-42)
        - 先调用 `origInvoke` 获取真实初始数据。
        - 若 `night` 为真，清空 `config.operator` 并设置 `nightOperatorRestore` 为反向运营商（unicom <-> telecom）。
- **bgStatus()**: (行 46-56)
    - 返回动态后台状态，包含 `onlineOperator` (取决于 `online` 和 `op`)。
- **定时器与事件发射**:
    - (行 59) 600ms 后首次发射 `background-check-result` 事件。
    - (行 66-69) 每 3000ms 重复发射 `background-check-result`，递增 `checkCount`。
- **截图辅助**:
    - (行 60-65) 若 `tab` 存在，查找 `aria-label="${tab}"` 的最后一个按钮并点击。
    - (行 72-76) 若 `emuw` 存在，注入 CSS 样式 `width:${emuw}px !important`。
    - (行 77-84) 若 `probe=1`，2500ms 后计算宽元素坐标并写入 `document.title`。

### 3. android/frontend/tauri-mock.js
独立 Mock 层，通常由 `vite.mock.config.ts` 注入。定义在 `android/frontend/tauri-mock.js`。此版本数据更丰富，支持更多业务场景（账单、在线设备、绑定状态）。

#### 全局配置与状态
- **config**: (行 32-42)
    - `user`: '24385214', `operator`: '@cmcc'.
    - `deviceAliases`: 映射 MAC 到别名，如 `B025AA6CA580: '实验室台式机'`.
    - 支持 URL 参数 `?panel=` 覆盖 `defaultPanel`，`?theme=` 覆盖 `themeMode`。
- **mockAccounts**: (行 45-49)
    - 包含 'acc-main', 'acc-dorm', 'acc-lab'。
- **activeAccountName**: (行 51)
    - 当前激活账号 ID，默认 'acc-main'。
    - 可通过 URL `?active=<id>` 覆盖。
- **onlineList**: (行 59-65)
    - 在线设备列表，包含 `loginTime`, `ip`, `mac`, `useTime`, `downFlow`, `upFlow`, `sessionId` 等。
    - 示例数据涵盖 PC 和 Mobile 终端。
- **loginHistory**: (行 67-70)
    - 历史登录记录，时间戳格式 `[start, end, ip, mac, time, flow, cost, ...]`。
- **billRows & billSummary**: (行 72-76)
    - 账单明细与汇总，包含流量分解 (INTERNET/CHINANET UP/DOWN)。
- **mockAdapters**: (行 82-84)
    - 单个以太网适配器，IP '10.2.103.66'。

#### 核心函数
- **ok(data, message)**: (行 78)
    - 统一响应格式包装器：`{ success: true, message, data }`。
- **sleep(ms)**: (行 80)
    - 模拟异步延迟。
- **handlers**: (行 87-214)
    - `get_init_data`: (行 88-97)
        - 返回完整初始化数据，包含 `adapterDetails` (含 subnetMask, gateway, dnsServers)。
        - `backgroundStatus` 初始为 `isRunning: false`。
    - `save_config(args)`: (行 99-110)
        - 深拷贝更新 `config`，忽略掩码值 `'***'`。
        - 0ms 后广播 `config-changed` 事件。
    - `query_self_dashboard`: (行 111) 返回 `onlineList` 和 `loginHistory`。
    - `self_offline_session(args)`: (行 112-118)
        - 延时 800ms 后从 `onlineList` 移除指定 `sessionId` 的设备。
    - `query_self_online_log`: (行 119) 返回 `billRows` 和 `billSummary`。
    - `change_self_password`: (行 121) 延时 800ms 返回成功。
    - `verify_windows_identity` / `verify_biometric_identity`: (行 122-123) 延时 600ms 返回成功。
    - `query_bind_status`: (行 126-136)
        - 支持 `?bound=` 参数覆盖绑定场景。
        - 返回各运营商 (`cmcc`, `telecom`, `unicom`) 的绑定状态及掩码账号。
    - `bind_operator`: (行 138-141)
        - 支持 `?bindfail=1` 模拟失败路径。
    - `check_campus_status`: (行 144)
        - **关键差异**：直接返回平铺对象，不使用 `ok()` 包裹，以符合 `CampusStatusResult` 契约。
        - 包含 `campusWifi` 和 `campusWired` 详细状态。
    - `check_portal_status`: (行 145) 返回 Portal 可达状态。
    - `render_heartbeat`: (行 146) 返回心跳状态。
    - `switch_account(args)`: (行 182-190)
        - 切换 `activeAccountName`，更新 `config.user` 和 `config.operator`。
        - 广播 `config-changed` 事件。
    - `delete_account(args)`: (行 191-196)
        - 从 `mockAccounts` 移除，若删除的是当前激活账号，则自动切换至第一个可用账号。
    - `get_adapters`: (行 197) 返回 `mockAdapters`。
    - `check_dns_doh_status`: (行 198) 返回空适配器列表。
    - `show_window` / `minimize_window` / `close_window`: (行 208-210) 返回 null。
- **invoke(cmd, args)**: (行 216-235)
    - 路由逻辑同 `mock-tauri.js`，但事件监听存储于 `eventListeners` 对象。
    - 未知命令打印警告。
- **emitEvent(event, payload)**: (行 239-247)
    - 遍历 `eventListeners[event]`，执行对应的 `window['__cb_'+id]` 回调。
- **window.__TAURI_INTERNALS__**: (行 249-257)
    - `invoke`: 引用上述函数。
    - `transformCallback`: 将回调存入 `window['__cb_'+id]` 并返回 ID。
- **截图辅助**: (行 263-277)
    - 支持 `?dialog=create|edit|bind`。
    - 4000ms 后查找包含对应文本（'添加登录信息' 等）的按钮并点击。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                    Browser Environment                       │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                  React Frontend                         │ │
│  │  @tauri-apps/api (invoke / listen)                      │ │
│  └──────────────────────┬──────────────────────────────────┘ │
│                         │                                    │
│                         ▼                                    │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │              window.__TAURI_INTERNALS__                 │ │
│  │  (Injected by one of the following mocks)               │ │
│  │                                                         │ │
│  │  Case A: Basic Mock (mock-tauri.js)                     │ │
│  │  ├── handlers: Simple CRUD for accounts                  │ │
│  │  └── State: Static mockConfig, empty adapters           │ │
│  │                                                         │ │
│  │  Case B: Scenario Addon (mock-tauri-addon.js)           │ │
│  │  ├── Wraps invoke to override check_campus/adapters     │ │
│  │  ├── Handles URL params (?op, ?dual, ?night)            │ │
│  │  └── Emits periodic background-check-result events      │ │
│  │                                                         │ │
│  │  Case C: Full Feature Mock (tauri-mock.js)              │ │
│  │  ├── Rich Data: onlineList, bills, history              │ │
│  │  ├── Account Switching: switch_account, save_current    │ │
│  │  ├── Bind Status: query_bind_status with scenarios      │ │
│  │  └── Event System: emitEvent with listener registry     │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **Initialization**:
    *   The application loads in the browser.
    *   One of the mock scripts (`mock-tauri.js`, `mock-tauri-addon.js`, or `tauri-mock.js`) is injected via HTML script tag or Vite plugin.
    *   The script checks `if (!window.__TAURI_INTERNALS__)`. If present (real Tauri or another mock), it exits early.
    *   It defines `window.__TAURI_INTERNALS__` with `invoke` and `transformCallback`.
    *   LocalStorage is initialized to skip onboarding (`campus-onboarding-done='1'`).

2.  **Command Invocation (invoke)**:
    *   Frontend calls `invoke('get_init_data', {})`.
    *   `__TAURI_INTERNALS__.invoke` routes to the specific handler (e.g., `handlers.get_init_data`).
    *   The handler returns a Promise resolving to the mock data object.
    *   In `mock-tauri-addon.js`, if the command is `get_init_data`, it first calls the original invoke (if available) and then mutates the returned `config` based on URL parameters like `?night=1`.

3.  **Event Listening (listen)**:
    *   Frontend calls `listen('background-check-result', callback)`.
    *   `transformCallback` stores the callback as `window['_cb{id}']` and returns `id`.
    *   `invoke('plugin:event|listen', { event: '...', handler: id })` registers the ID in `eventHandlers` or `eventListeners`.
    *   The mock script (e.g., `mock-tauri-addon.js` line 66) uses `setInterval` to call `__mockEmitEvent` or `emitEvent`.
    *   `emitEvent` finds all registered callbacks for the event name and executes them with the payload.

4.  **Scenario Injection**:
    *   `mock-tauri-addon.js` parses URL search params (`?op=@telecom&dual=1`).
    *   It overrides specific commands (`get_adapters`, `check_portal_status`) to return data reflecting these parameters.
    *   It injects CSS styles if `?emuw` is present to simulate narrow screens.

## Connections

*   [[android-frontend-core]]: The main React application that relies on `@tauri-apps/api`.
*   [[android-frontend-hooks]]: Contains `tauriApi.ts` which defines the TypeScript interfaces for the Tauri commands mocked here.
*   [[android-frontend-config]]: Configuration files that might inject these mock scripts during development.

## Known Issues

1.  **Mock Drift**: The three mock files have diverging data structures. For example, `mock-tauri.js` uses simple arrays for accounts, while `tauri-mock.js` includes rich billing and session data. Developers must ensure they are using the correct mock file for their testing context.
2.  **State Persistence**: Mock state is stored in memory (JS variables) and sometimes `localStorage`. Reloading the page resets in-memory state unless re-initialized by the script's startup logic.
3.  **Event Listener Cleanup**: While `unlisten` is implemented, it relies on string matching for IDs (`'_cb' + eventId`). If IDs collide or are not unique across different mock instances, listeners might fire incorrectly.
4.  **Hardcoded Values**: Many values (SSIDs, IPs, User IDs) are hardcoded. Changes in the real backend schema require manual updates in all three mock files.
5.  **`check_campus_status` Wrapper Issue**: In `tauri-mock.js` (line 143-144), there is a comment warning against wrapping the response in `ok()`. This is a known fragile point where incorrect formatting leads to `undefined` fields in the frontend.