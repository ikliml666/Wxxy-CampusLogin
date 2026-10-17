m01156–m01243 重写 zustand-state-management.md
TASK AS OF THIS BLOCK: 已根据最新源码（`useAuthStore.ts`, `useConfigStore.ts`, `useQualityStore.ts`, `useLogToastStore.ts`, `useThemeStore.ts`, `useAdapterStore.ts`, `tauriApi.ts`, `useDnsSetup.ts`）重写了 `concepts/zustand-state-management.md`。文章详细记录了各 Store 的状态结构、关键 Action 逻辑（如防抖保存、脏标记、节流探测）、IPC 封装及 DNS 优化流程，并补充了架构框图与已知历史缺陷修复记录。

---
type: concept
title: zustand-state-management
source_files:
  - android/frontend/src/hooks/useAuthStore.ts
  - android/frontend/src/hooks/useConfigStore.ts
  - android/frontend/src/hooks/useQualityStore.ts
  - android/frontend/src/hooks/useLogToastStore.ts
  - android/frontend/src/hooks/useThemeStore.ts
  - android/frontend/src/hooks/useAdapterStore.ts
  - android/frontend/src/hooks/tauriApi.ts
  - android/frontend/src/network/useDnsSetup.ts
tags:
  - state-management
  - zustand
  - frontend-architecture
  - hooks
---

## Overview

`zustand-state-management` 是 Android 前端应用的核心状态管理范式，基于 Zustand 库构建。它负责将应用的全局状态（如认证、配置、网络质量、日志通知等）与 UI 渲染逻辑解耦，提供单一数据源（Single Source of Truth）。该概念不仅定义了数据的存储结构，还规定了数据如何通过 Hooks 在组件间流动，以及如何处理与后端 Rust/Tauri 的异步交互和持久化。

核心原则包括：
1.  **原子化 Store**：按领域（Domain）拆分 Store，避免单体臃肿。
2.  **响应式订阅**：UI 组件通过 Hook 订阅特定状态切片，实现细粒度重渲染控制。
3.  **副作用隔离**：状态变更逻辑与 Tauri IPC 调用封装在 Actions 中，保持 Store 纯净。
4.  **性能优化**：利用引用稳定检查（如 `setStatusStable`）、防抖（debounce）、节流（throttle）及去重逻辑减少不必要的组件更新和资源浪费。

## Involved Modules

该概念贯穿前端应用的多个模块，主要涉及以下核心文件与职责：

### 1. 核心状态定义层 (`frontend-hooks-store`)
这是状态管理的基石，定义了所有全局 Store 及其初始状态。
*   **`android/frontend/src/hooks/useAuthStore.ts`**: 管理用户认证状态、在线检测及后台检查。包含登录/注销流程、校园网/Portal 状态检测、登录后质量探测节流等逻辑。
*   **`android/frontend/src/hooks/useConfigStore.ts`**: 管理应用配置、密码保存状态、账号列表及语言设置。内置防抖保存、脏标记（Dirty Fields）机制以防止后端旧快照覆盖本地新值，以及连续失败重置逻辑。
*   **`android/frontend/src/hooks/useQualityStore.ts`**: 管理网络质量数据、DNS DoH 状态、版本更新信息及刷新速率。记录最近一次质量结果时间戳用于节流判断。
*   **`android/frontend/src/hooks/useLogToastStore.ts`**: 管理日志条目队列与 Toast 通知堆栈。限制最大日志数和 Toast 数，支持同题去重和时间窗口自动淘汰。
*   **`android/frontend/src/hooks/useThemeStore.ts`**: 管理主题名称、亮暗模式及自定义颜色。处理 DOM class 切换与 CSS 变量注入。
*   **`android/frontend/src/hooks/useAdapterStore.ts`**: 管理面板导航状态（安卓端仅保留 `activePanel`，网卡相关功能已移除）。

### 2. 业务逻辑集成层
这些模块消费上述 Store，并将业务逻辑与状态更新绑定。
*   **`android/frontend/src/network/useDnsSetup.ts`**: DNS 设置逻辑，依赖 `useConfigStore` 和 `useQualityStore` 进行读写。提供 DNS 检测与一键优化（DoH 启用）两动作。
*   **`android/frontend/src/hooks/tauriApi.ts`**: 提供 `tauriApi` 对象及 `tauriApiWithRetry`，作为 Store Actions 中执行后端 RPC 调用的统一入口。对桌面专属功能进行显式拒绝或空监听适配。

## Flow

数据与控制流在 `zustand-state-management` 中遵循“单向数据流”与“异步副作用”相结合的模式：

### 1. 初始化与加载
*   **入口**: `App.tsx` (via `useAppInit`) 或 `useInitialDataLoad`。
*   **流程**:
    1.  应用启动时，Hooks 从 `safeStorage` 或 Tauri API 获取初始数据。
    2.  通过 Store 的 Action（如 `setConfig`, `setAuthStatus`）将数据写入 Zustand Store。
    3.  依赖该状态的组件（如 `DashboardPanel`）自动接收新状态并重新渲染。

### 2. 用户交互驱动的状态变更
*   **场景**: 用户在 `SettingsPanel` 切换主题或在 `AccountPanel` 修改密码。
*   **流程**:
    1.  UI 组件捕获用户输入（通常先存入本地 Draft State，如 `passwordDraft`）。
    2.  触发提交事件（如 `blur` 或点击保存）。
    3.  调用对应的 Store Action（如 `updateConfig`）。
    4.  Action 内部可能调用 `tauriApi.save_config` 进行持久化。
    5.  成功后，更新本地 Store 状态并触发 `useLogToastStore` 显示成功提示。
    6.  其他订阅了相关状态的组件（如 `StatusBar`）同步更新。

### 3. 后台监控与数据推送
*   **场景**: 网络质量实时监控。
*   **流程**:
    1.  `monitor-module` 中的后台任务（或 Tauri 侧的 `monitor_loop`）定期获取网络数据。
    2.  通过 Tauri Event Listener 或轮询机制将数据传递给前端。
    3.  `useQualityStore` 的 Action（如 `updateNetworkQuality`）接收增量数据。
    4.  利用 `mergeNetworkQuality` 逻辑合并新旧数据，防止状态冲突。
    5.  依赖 `useQualityStore` 的组件（如 `QualityPanel`, `NetworkQualityCapsule`）实时刷新 UI。

### 4. 状态订阅与性能优化
*   **常规订阅**: 组件直接使用 `const status = useAuthStore(s => s.status)`。
*   **引用稳定**: `useAuthStore` 中的 `setStatusStable` 函数在 text/state 未变化时不换新对象，保持引用稳定，避免 StatusBar 等组件无效重渲染。
*   **防抖与节流**: `useConfigStore` 的 `updateConfig` 内置 500ms 防抖逻辑，配合 `dirtyFields` 防止高频操作导致的状态覆盖或后端旧快照问题。`useAuthStore` 登录后手动质量探测引入 60s 节流，避免与后端 latency loop 重复全量检测。

## Implementation Map

具体实现位置如下：

| 概念要素 | 文件路径 | 行号/范围 | 说明 |
| :--- | :--- | :--- | :--- |
| **Store 定义** | `android/frontend/src/hooks/useAuthStore.ts` | L68-L201 | 认证状态定义，包含 `checkOnline` 等异步 Action。 |
| **Store 定义** | `android/frontend/src/hooks/useConfigStore.ts` | L46-L190 | 配置状态定义，包含 `updateConfig` 防抖逻辑。 |
| **Store 定义** | `android/frontend/src/hooks/useQualityStore.ts` | L30-L68 | 网络质量状态定义。 |
| **Store 定义** | `android/frontend/src/hooks/useLogToastStore.ts` | L26-L121 | 日志与通知状态定义，限制最大 Toast 数量。 |
| **Store 定义** | `android/frontend/src/hooks/useThemeStore.ts` | L30-L57 | 主题状态定义，处理亮暗模式切换。 |
| **IPC 桥接** | `android/frontend/src/hooks/tauriApi.ts` | L103-L183 | 统一后端 RPC 接口，供 Store Actions 调用。 |
| **重试封装** | `android/frontend/src/hooks/tauriApi.ts` | L217-L221 | `tauriApiWithRetry`，增强 IPC 稳定性。 |
| **消费示例** | `android/frontend/src/auth/DashboardPanel.tsx` | N/A | 通过 `useAuthStore` 获取认证状态渲染 UI。 |
| **消费示例** | `android/frontend/src/account/AccountPanel.tsx` | N/A | 使用 `useShallow` 优化配置状态订阅。 |
| **业务逻辑** | `android/frontend/src/network/useDnsSetup.ts` | L18-L82 | DNS 设置 Hook，协调 Store 与 Tauri API。 |

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│                         Frontend Application                        │
│                                                                     │
│  ┌──────────────────┐   ┌──────────────────┐   ┌────────────────┐  │
│  │   UI Components  │   │   Business Logic │   │   Global Store │  │
│  │                  │   │                  │   │                │  │
│  │ DashboardPanel   │──►│ useDnsSetup      │   │ useAuthStore   │  │
│  │ SettingsPanel    │   │                  │   │ useConfigStore │  │
│  │ AccountPanel     │──►│                  │   │ useQualityStore│  │
│  │ QualityPanel     │   │                  │   │ useLogToastStore│  │
│  └────────┬─────────┘   └────────┬─────────┘   │ useThemeStore  │  │
│           │                     │              │ useAdapterStore│  │
│           │                     │              └───────┬────────┘  │
│           │                     │                      │           │
│           │                     │              ┌───────▼────────┐  │
│           │                     │              │  tauriApi      │  │
│           │                     │              │ withRetry      │  │
│           │                     │              └───────┬────────┘  │
│           │                     │                      │           │
│           │                     │              ┌───────▼────────┐  │
│           │                     │              │  Tauri Backend │  │
│           │                     │              │  (Rust)        │  │
│           │                     │              └────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### 1. 认证流程 (`useAuthStore`)
*   **登录 (`doLogin`)**:
    1.  检查锁状态 (`isLoggingIn`/`isLoggingOut`)。
    2.  尝试保存配置（失败则警告并使用已有配置）。
    3.  调用 `api.doLogin()`，带 60s 超时保护。
    4.  若成功，更新状态为 `online`，记录日志，并在 60s 节流窗口外触发一次网络质量探测。
    5.  若失败，触发 `checkOnline()` 复查确认状态。
*   **注销 (`doLogout`)**:
    1.  调用 `api.doLogout()`，带 60s 超时保护。
    2.  更新状态为 `offline`，记录日志。
    3.  触发 `checkOnline()` 复查确认状态。
*   **在线检测 (`checkOnline`)**:
    1.  使用 `_checkOnlineLockFlag` 防止并发冗余 invoke。
    2.  若启用校园网检测 (`enableNetworkNameCheck`)，调用 `detectCampusNetwork()`。若不在校园网，直接返回 `offline` 并更新 `bgStatus`。
    3.  调用 `queryPortalStatus('')`（安卓无 adapterIp，后端回退缓存源 IP/默认路由）。
    4.  根据 Portal 返回结果更新 `status`，使用 `setStatusStable` 避免无效重渲染。

### 2. 配置保存流程 (`useConfigStore`)
*   **更新配置 (`updateConfig`)**:
    1.  合并新配置到当前 `config`，更新 Store 状态。
    2.  将修改字段加入 `dirtyFields`，重置其连续失败计数。
    3.  清除现有定时器，设置新的 500ms 防抖定时器。
    4.  定时器触发时，调用 `saveConfigDirect`。
*   **直接保存 (`saveConfigDirect`)**:
    1.  调用 `api.saveConfig()`。
    2.  若成功，清除 `dirtyFields` 中对应字段的标记，若含密码则同步 `passwordSaved` 为 `true`。
    3.  若失败，增加 `dirtyFailureCounts`，若连续失败达 3 次则放弃脏标记，允许后端回传重新同步。
*   **脏标记机制**: `mergeConfigFromBackend` 会跳过 `dirtyFields` 中的字段，防止后端旧快照覆盖本地未确认的新值。

### 3. 网络质量流程 (`useQualityStore`)
*   **刷新质量 (`refreshQuality`)**:
    1.  检查 `_qualityLockFlag` 和 `config.enableNetworkQuality`。
    2.  调用 `api.checkNetworkQuality()`。
    3.  若返回数据，调用 `setNetworkQuality` 合并并更新 `lastQualityResultTime`。
*   **设置质量 (`setNetworkQuality`)**:
    1.  若新值为非 null，更新 `lastQualityResultTime` 为当前时间戳。
    2.  更新 `networkQuality` 状态。

### 4. 日志与通知流程 (`useLogToastStore`)
*   **添加日志 (`addLog`)**:
    1.  若最后一条日志内容与类型相同，更新时间戳并增加 `count`。
    2.  否则追加新条目，保持最多 `MAX_LOG_ENTRIES` 条。
*   **添加 Toast (`addToast`)**:
    1.  若非前台且无操作按钮，直接跳过。
    2.  若标题重复，跳过新增。
    3.  若超过 `MAX_TOASTS`，淘汰最旧 Toast 及其定时器。
    4.  设置新定时器，到期后移除 Toast。

### 5. 主题切换流程 (`useThemeStore`)
*   **初始化 (`initTheme`)**:
    1.  读取 `safeStorage` 中的 `campus-theme` 和 `campus-light-mode`。
    2.  若配置中有 `themeMode`，则覆盖并持久化到 `safeStorage`。
*   **订阅副作用**:
    1.  监听 `isLightMode`、`themeName`、`customThemeColor` 变化。
    2.  切换 `document.documentElement` 的 `dark` class 和 `data-light` attribute。
    3.  若为自定义主题，计算 HSL 值并设置 CSS 变量 (`--primary`, `--ring`, `--accent`, `--accent-foreground`)。

## Connections

*   [[auth-flow]]: 认证流程依赖于 `useAuthStore` 和 `useConfigStore`。
*   [[network-monitoring]]: 网络监控依赖于 `useQualityStore` 和 `useAuthStore`。
*   [[ui-theming]]: UI 主题依赖于 `useThemeStore` 和 `useConfigStore`。
*   [[backend-ipc]]: 所有 Store 的异步操作均通过 `tauriApi` 与后端通信。