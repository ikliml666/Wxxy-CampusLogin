---
title: desktop-frontend-mock-tauri
type: module
source_files:
  - tauri-app/frontend/public/mock-tauri.js
  - tauri-app/frontend/public/mock-tauri-addon.js
tags: [tauri, mock, frontend, dev-tools, shim]
---

## Overview
`desktop-frontend-mock-tauri` 是一套专为桌面端前端（基于 Tauri 框架）设计的浏览器调试模拟层。它通过在全局作用域伪造 `__TAURI_INTERNALS__` 对象，使得原本依赖原生 WebView 和 Rust 后端命令的 Web 前端代码能够在标准浏览器环境中运行。该模块主要用于 UI 截图验证、功能演示以及开发阶段的独立调试，**严禁入库或用于生产环境**。

核心由两个文件组成：
1. `mock-tauri.js`：基础 Shim 层，提供完整的 Tauri API 模拟、状态管理、事件系统初始化及默认 Mock 数据。
2. `mock-tauri-addon.js`：场景注入层，叠加在基础层之上，通过 URL 参数动态修改配置、网络状态、运营商信息等，支持自动化截图辅助功能。

## Key Components

### 1. Base Mock Layer (`mock-tauri.js`)
位于 `tauri-app/frontend/public/mock-tauri.js`。

#### 全局变量与状态
- **`mockConfig` (Line 38-58)**: 默认应用配置对象。包含用户信息、网络策略、主题模式、后台检查间隔等。
    - 关键字段：`operator`, `adapter1`, `dualAdapter`, `themeMode`, `enableBackgroundCheck`。
- **`campusStatus` (Line 60-65)**: 校园网状态快照。包含 SSID、连接状态、网关信息等。
- **`backgroundStatus` (Line 67-73)**: 后台检测服务状态。初始化为未运行 (`isRunning: false`)。
- **`mockAdapters` (Line 75-79)**: 模拟网卡列表。包含 WLAN、以太网、以太网 2 三种网卡及其 IP/MAC/速率。
- **`mockAccounts` (Line 83-87)**: 模拟账号列表。包含 `id`, `displayName`, `user`, `operator`, `passwordSaved`。
- **`initData` (Line 89-96)**: 启动时返回的完整初始化数据结构，聚合上述所有 Mock 数据。
- **`eventHandlers` (Line 22)**: 存储事件监听器的映射表，结构为 `{ eventName: [callbackId, ...] }`。
- **`callbackSeq` (Line 21)**: 回调函数 ID 计数器。

#### 核心函数
- **`transformCallback(callback, once)` (Line 24-36)**: 
    - 将 JS 回调函数挂载到 `window` 对象上（属性名 `_cb{id}`），返回唯一 ID。
    - 用于兼容 Tauri 的异步回调机制。
- **`handlers` 对象 (Line 98-164)**: 定义所有支持的 Tauri 命令处理逻辑。
    - `get_config`: 返回 `mockConfig`。
    - `save_config`: 返回成功。
    - `get_init_data`: 返回 `initData`。
    - `list_accounts` / `create_account` / `rename_account` / `delete_account`: 模拟账号 CRUD 操作。
        - `create_account` (Line 106): 校验名称唯一性，追加至 `mockAccounts`。
        - `save_current_as_account` (Line 118): 将当前 `mockConfig` 同步到指定账号。
    - `get_adapters`: 返回 `mockAdapters`。
    - `check_campus_status`: 返回 `campusStatus`。
    - `do_login` / `do_logout`: 返回演示模式消息，不执行真实登录。
    - `plugin:event|listen` (Line 175): 注册事件监听，调用 `transformCallback` 获取 ID 并存入 `eventHandlers`。
    - `plugin:event|unlisten` (Line 183): 注销事件监听。
- **`window.__TAURI_INTERNALS__` (Line 166-190)**: 
    - 核心入口。包含 `metadata`, `__TAURI_PATTERN__`, `transformCallback`, `invoke`。
    - `invoke(cmd, args)`: 路由分派器。匹配 `handlers` 中的键，执行对应函数并返回 Promise。若命令以 `plugin:` 开头且未在 handlers 中，则静默返回 null。
- **`window.__TAURI_EVENT_PLUGIN_INTERNALS__` (Line 194-198)**: 
    - 提供 `unregisterListener(event, eventId)`，删除对应的 `window['_cb' + eventId]`。
- **`window.__mockEmitEvent(name, payload)` (Line 199-205)**: 
    - 手动触发事件分发。遍历 `eventHandlers[name]` 中的所有 callbackId，找到对应函数并执行，传入标准化 payload `{ event, id, payload }`。

### 2. Scenario Addon (`mock-tauri-addon.js`)
位于 `tauri-app/frontend/public/mock-tauri-addon.js`。

#### 配置解析 (Line 5-11)
从 URL Query Params 读取场景参数：
- `op`: 主运营商后缀 (默认 `@telecom`)。
- `op2`: 副运营商后缀 (需 `dual=1`)。
- `dual`: 是否启用双网卡模式 (`'1'`)。
- `night`: 是否启用夜切模式 (`'1'`)。
- `offline`: 是否离线 (`'1'`)。
- `tab`: 自动切换的 Dock 页签名称。
- `emuw`: 模拟窗口宽度 (px)。
- `probe`: 开启布局诊断输出。

#### 核心逻辑
- **`invoke` 劫持 (Line 13-48)**:
    - 保存原始 `origInvoke`。
    - 重写 `window.__TAURI_INTERNALS__.invoke`。
    - **`check_campus_status` (Line 15-21)**: 强制返回在线状态，SSID 固定为 `i-wxxy`。
    - **`get_adapters` (Line 22-28)**: 根据 `dual` 参数决定是否添加“以太网”网卡。
    - **`check_portal_status` (Line 29-31)**: 根据 `online` 参数返回在线/离线状态。
    - **`do_login` (Line 32)**: 始终返回成功但提示演示模式。
    - **`get_background_status` (Line 33)**: 调用内部 `bgStatus()` 生成动态状态。
    - **`get_init_data` (Line 34-46)**: 先调用 `origInvoke` 获取基础数据，然后覆盖 `config` 字段：
        - 设置 `operator` (受 `night` 影响)。
        - 设置 `nightOperatorRestore` (夜切回退目标)。
        - 设置 `adapter1`, `adapter2`, `dualAdapter`。
- **`bgStatus()` 函数 (Line 50-67)**:
    - 构建动态后台状态对象。
    - 包含 `adapterStatuses`，根据 `dual` 和 `op2` 动态生成以太网状态。
    - 包含 `currentSsid`, `campusWifi` 等硬编码的演示数据。
- **事件发射定时器 (Line 70-80)**:
    - Line 70: 600ms 后发射一次 `background-check-result`。
    - Line 77-80: 每 3000ms 循环发射 `background-check-result`，递增 `checkCount`。
- **UI 辅助功能**:
    - **Tab 自动点击 (Line 71-76)**: 若 `tab` 参数存在，1500ms 后查找对应 `aria-label` 的按钮并点击。优先选择最后一个匹配项以应对 DOM 结构问题。
    - **窄屏模拟 (Line 82-87)**: 若 `emuw` 参数存在，注入 CSS 强制 `html,body` 宽度。
    - **布局探针 (Line 88-95)**: 若 `probe=1`，2500ms 后计算超出视口的元素，将其信息写入 `document.title`，便于无头浏览器抓取诊断。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                    Browser Environment                       │
│                                                             │
│  ┌──────────────────┐       ┌────────────────────────────┐  │
│  │   index.html     │       │   App.tsx (React/Vue/etc)  │  │
│  │                  │       │                            │  │
│  │  <script>         │       │  window.__TAURI_INTERNALS__│  │
│  │  mock-tauri.js   │──────►│  .invoke(...)              │  │
│  │  (Base Shim)     │       │                            │  │
│  └────────┬─────────┘       └────────────▲───────────────┘  │
│           │                              │                  │
│           │  Injects                     │ Calls            │
│           ▼                              │                  │
│  ┌──────────────────┐       ┌────────────────────────────┐  │
│  │ mock-tauri-addon │       │  window.__TAURI_INTERNALS__│  │
│  │ (Scenario Overlay)│      │  .invoke(...) [Overridden] │  │
│  │                  │       │                            │  │
│  │ URL Params Parse │       │  Routes to:                │  │
│  │ & Hook invoke    │       │  1. origInvoke (if needed) │  │
│  │ & Emit Events    │       │  2. Static Mock Data       │  │
│  └──────────────────┘       │  3. Dynamic bgStatus()     │  │
│                             └────────────────────────────┘  │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  Global State & Handlers                             │  │
│  │  - eventHandlers (Map)                               │  │
│  │  - mockConfig, initData, mockAccounts                │  │
│  │  - __mockEmitEvent (Dispatcher)                      │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **Initialization**:
   - `mock-tauri.js` 立即执行，创建 `window.__TAURI_INTERNALS__` 和 `handlers`。
   - `mock-tauri-addon.js` 随后加载，检测 `__TAURI_INTERNALS__` 是否存在。
   - Addon 解析 URL 参数，保存原始 `invoke` 引用，并替换全局 `invoke` 方法。

2. **Configuration Loading (`get_init_data`)**:
   - 前端请求 `get_init_data`。
   - Addon 拦截调用，委托给 `origInvoke` 获取基础结构。
   - Addon 修改返回对象的 `config` 字段，应用 `op`, `dual`, `night` 等参数。
   - 最终包含特定场景配置的 `initData` 返回给前端。

3. **Runtime Status Updates**:
   - 前端订阅 `background-check-result` 事件。
   - Addon 启动 `setInterval` (每 3s) 和 `setTimeout` (600ms)。
   - 每次触发时，调用 `bgStatus()` 生成最新状态（包含实时计算的 `checkCount` 和基于参数的在线状态）。
   - 通过 `__mockEmitEvent` 分发事件，更新前端 UI。

4. **User Interaction (Login/Logout)**:
   - 前端调用 `do_login`。
   - Addon 拦截，直接返回 `{ success: true, message: '...' }`，不执行任何网络请求。

## Connections

- [[tauri-app-frontend]]: 被此模块模拟的前端应用主体。
- [[android-frontend-hooks]]: `mock-tauri.js` 的注释指出其命令名与 Android 前端的 `tauriApi.ts` 一一对应，保持接口一致性。
- [[desktop-backend-rust]]: 实际 Tauri 后端实现，本模块旨在绕过其进行纯前端测试。

## Known Issues

1. **非侵入式限制**: Addon 仅劫持 `invoke` 方法，未完全模拟 Tauri 的所有插件行为（如文件系统、系统托盘交互）。某些依赖底层插件的命令可能失效或返回空值。
2. **事件模拟延迟**: `__mockEmitEvent` 是手动触发的。虽然 Addon 设置了定时器，但如果前端逻辑依赖特定时间戳或精确的事件序列，可能与真实环境有偏差。
3. **CSS 样式冲突**: `emuw` 参数通过注入 `<style>` 标签强制宽度，可能破坏原有响应式布局的媒体查询逻辑，仅适用于固定宽度截图场景。
4. **数据持久化缺失**: `mockConfig` 和 `mockAccounts` 均为内存变量。刷新页面后所有更改丢失，且不会写回 `localStorage` 或后端。
5. **禁用功能**:
   - `do_login` / `do_logout`: 在 `mock-tauri.js` Line 160-161 和 `mock-tauri-addon.js` Line 32 中被硬编码为演示消息，不执行真实认证。
   - `outbound_switch_now` / `outbound_restore_now`: 在 `mock-tauri.js` Line 143-144 中返回错误消息，不支持真实网卡切换。