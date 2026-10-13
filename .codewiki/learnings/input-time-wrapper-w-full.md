---
title: MonitorPanel 双端组件架构与布局约束
type: learning
source_files:
  - tauri-app/frontend/src/components/ui/input.tsx
  - tauri-app/frontend/src/monitor/MonitorPanel.tsx
  - android/frontend/src/monitor/MonitorPanel.tsx
tags: [frontend, react, layout, dual-platform, state-management]
---

## Overview

`MonitorPanel` 是监控面板的核心 UI 组件，负责展示网络状态检测控制、适配器在线状态以及自动化配置（定时登录/注销、校园网校验等）。该模块在 Tauri（桌面）和 Android 两个平台间存在显著的功能与实现差异：桌面端功能完整，包含适配器状态卡片、退出生效时段、非校园网退出等高级设置；Android 端功能精简，仅保留基础自动化与校园网校验，且使用本地定义的 `MonitorSettingRow` 替代桌面端的 `SettingRow`。

组件采用 `memo` 包裹以避免不必要的重渲染，并通过 `useConfigStore` 的 `useShallow` 订阅特定字段以隔离状态更新。所有输入控件均经过防抖或草稿模式处理，以防止用户输入时的级联渲染问题。

## Key Components

### Tauri (Desktop) Implementation

**File:** `tauri-app/frontend/src/monitor/MonitorPanel.tsx`

- **`AdapterStatusCard`**: 适配器在线状态迷你卡。
    - Props: `{ status: AdapterOnlineStatus; isPrimary: boolean }`
    - 用途: 横向小卡展示适配器名称、IP、连接类型（有线/无线）及在线状态。
    - 逻辑: 根据 `status.online` 切换背景色（emerald/rose），显示无线 (`Wifi`) 或有线 (`Cable`) 图标。
- **`GroupCardHeader`**: 设置卡片头组件。
    - Props: `{ icon: React.ReactNode; titleKey: string; descKey: string }`
    - 用途: 统一两张设置卡的头部结构，包含图标、标题和描述。
- **`MonitorPanel`**: 主组件。
    - Props: `{ onUpdateConfig, onToggleBackgroundCheck, onTriggerCheck }`
    - State:
        - `bgStatus`: 来自 `useAuthStore`，包含运行状态、检查次数、适配器状态列表等。
        - `config`: 来自 `useConfigStore(useShallow(...))`，浅比较订阅配置。
        - `intervalDraft`: 数字输入本地草稿，防止 clamp 值回灌导致输入阻塞。
        - `isRefreshing`, `isTogglingDetection`: 来自 `useAsyncLock`，防止并发操作。
    - Methods:
        - `commitInterval`: 提交间隔时间，进行 clamp (10-600秒) 并更新 store。
        - `handleTriggerCheck`: 手动触发一次检测。
        - `toggleBackgroundCheck`: 启动/停止后台检测。

### Android Implementation

**File:** `android/frontend/src/monitor/MonitorPanel.tsx`

- **`MonitorSettingRow`**: 安卓专用设置行组件。
    - Props: `{ htmlFor?, icon, label, description?, children }`
    - 用途: 替代桌面端的 `SettingRow`，实现左图标+标签/描述，右 children 的布局。
    - 样式: 固定宽度图标容器 (`w-8 h-8`)，无分隔线。
- **`MonitorPanel`**: 安卓主组件。
    - Props: 同桌面端。
    - Differences from Desktop:
        - 缺少 `AdapterStatusCard` 和 `GroupCardHeader`，使用内联布局。
        - 缺少 "自动退出"、"非校园网退出"、"退出生效时段" 设置项。
        - 增加 `Tooltip` 用于说明检测逻辑。
        - 增加 `requestWifiSsidPermission` 调用，开启网络名校验时请求权限。
        - 使用 `tauriApiWithRetry` 包装 API 调用。
        - 新增 `WatcherFailed` 降级提示逻辑。

### Shared/Input Component

**File:** `tauri-app/frontend/src/components/ui/input.tsx`

- **`Input`**: 通用输入框组件。
    - Props: `InputProps` extends `React.InputHTMLAttributes<HTMLInputElement>`, adds `icon?: React.ReactNode`, `error?: string`.
    - Structure:
        - Outer: `<div className="relative w-full">`.
        - Icon: Absolute positioned left.
        - Input: `<input>` with shared styles.
        - Error: `<p>` below input.
    - Note: The outer wrapper applies `relative w-full`, which can interfere with flex layouts if not handled correctly by parent containers.

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                     MonitorPanel                           │
│  (tauri-app/.../MonitorPanel.tsx / android/.../MonitorPanel)│
│                                                             │
│  ┌─────────────────────┐   ┌──────────────────────────────┐ │
│  │  Control Bar Card   │   │  Settings Cards Grid         │ │
│  │  (AnimatedCard)     │   │                              │ │
│  │  - Radar Icon       │   │  ┌────────────────────────┐  │ │
│  │  - Switch (On/Off)  │   │  │ Automation Card        │  │ │
│  │  - Interval Input   │   │  │ (Rocket Icon)          │  │ │
│  │  - Trigger Button   │   │  │ - Auto Start           │  │ │
│  └─────────────────────┘   │  │ - Auto Login           │  │ │
│                             │  └────────────────────────┘  │ │
│  ┌─────────────────────┐   │  ┌────────────────────────┐  │ │
│  │ Adapter Status Cards│   │  │ Campus Check Card      │  │ │
│  │ (Desktop Only)      │   │  │ (Settings2 Icon)       │  │ │
│  │ - Adapter 1         │   │  │ - Network Name Check   │  │ │
│  │ - Adapter 2         │   │  │ - Time Ranges          │  │ │
│  └─────────────────────┘   │  │ - Scheduled Login      │  │ │
│                             │  │ - Scheduled Logout     │  │ │
│  ┌─────────────────────┐   │  └────────────────────────┘  │ │
│  │ Config Store        │   └──────────────────────────────┘ │
│  │ (useShallow)        │                                    │
│  └─────────────────────┘                                    │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **State Subscription**:
    - `MonitorPanel` subscribes to `authStore.bgStatus` for real-time monitoring data.
    - It subscribes to `configStore.config` using `useShallow` to minimize re-renders when unrelated config fields change.
2.  **User Interaction**:
    - **Switches**: Directly call `onUpdateConfig` to persist changes to the store/backend.
    - **Interval Input**: Uses local draft state (`intervalDraft`). On `blur` or `Enter`, `commitInterval` clamps the value and updates the store. This prevents input blocking due to immediate store updates.
    - **Buttons**: Wrapped in `useAsyncLock` to prevent concurrent execution of `triggerCheck` and `toggleBackgroundCheck`.
3.  **Platform Specifics**:
    - **Desktop**: Renders `AdapterStatusCard` based on `bgStatus.adapterStatuses`. Includes advanced settings like campus exit windows.
    - **Android**: Skips adapter cards. Includes a tooltip for detection logic explanation. Requests Wi-Fi SSID permission upon enabling network check.

## Connections

- [[Config Store]]: Manages application configuration, subscribed by `MonitorPanel`.
- [[Auth Store]]: Provides background check status and adapter information.
- [[Input Component]]: Used for interval and time inputs; note the `relative w-full` wrapper constraint.
- [[Setting Row Component]]: Desktop uses shared `SettingRow`; Android implements `MonitorSettingRow`.

## Known Issues

1.  **Flex Layout Constraint with Input Wrapper**:
    - **Issue**: The `Input` component wraps its content in `<div className="relative w-full">`. When used directly as a child of a `flex items-center justify-between` container, the `w-full` wrapper can cause the input to stretch unexpectedly or push other elements out of alignment, especially on narrow screens.
    - **Workaround**: In `MonitorPanel.tsx` (both platforms), time inputs are wrapped in a `<div className="flex shrink-0 items-center gap-2">` (or similar) to constrain their width and prevent them from consuming available flex space. This ensures they align correctly next to labels or switches.
    - **Reference**: See comments in `tauri-app/frontend/src/monitor/MonitorPanel.tsx` around line 280 and `android/frontend/src/monitor/MonitorPanel.tsx` around line 290 regarding "time input" and "shrink-0".
2.  **Input Draft State Complexity**:
    - The use of `intervalDraft` introduces local state management that must be carefully synchronized with the global store on blur/enter. Any failure to commit the draft may lead to inconsistent UI state.
3.  **Android Permission Request**:
    - Enabling network name check on Android triggers a permission request (`requestWifiSsidPermission`). If denied, the backend will not receive the SSID, potentially causing detection failures. The UI does not explicitly guide the user through the permission denial flow.