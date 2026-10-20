---
title: frontend-layout-navigation
type: module
source_files:
  - android/frontend/src/components/layout/BottomNav.tsx
  - android/frontend/src/components/layout/DockNav.tsx
  - android/frontend/src/components/layout/RightPanel.tsx
  - android/frontend/src/components/layout/TitleBar.tsx
  - android/frontend/src/components/mobile/MobileDashboard.tsx
  - android/frontend/src/components/mobile/MobileMore.tsx
  - android/frontend/src/components/mobile/MobileQuickActions.tsx
  - android/frontend/src/components/tablet/TabletShell.tsx
tags: [layout, navigation, responsive, mobile, tablet, framer-motion, gsap]
---

## Overview

`frontend-layout-navigation` 模块负责处理应用在不同设备形态（手机、平板、桌面）下的核心布局与导航结构。它通过条件渲染和响应式 CSS 变量，为移动端提供底部胶囊导航栏 (`BottomNav`) 和悬浮快捷操作 (`MobileQuickActions`)，为桌面/平板端提供磁性吸附 Dock 导航 (`DockNav`)、系统标题栏 (`TitleBar`) 以及右侧日志面板 (`RightPanel`)。

该模块还包含了针对平板设备的外壳组件 `TabletShell`，它协调了上述所有子组件，并处理横竖屏切换时的布局调整（如隐藏右侧日志栏）。移动端的核心页面内容则由 `MobileDashboard` 和 `MobileMore` 承载。

## Key Components

### BottomNav.tsx (Mobile Bottom Navigation)
**文件**: `android/frontend/src/components/layout/BottomNav.tsx`

用于移动端的底部导航栏，采用磨砂玻璃效果的圆角胶囊设计。

*   **Type `MobileTab`** (Line 14): 定义页签类型 `'dashboard' | 'account' | 'selfservice' | 'quality' | 'monitor' | 'more'`。
*   **Constant `TABS`** (Line 16-21): 基础页签配置数组，包含 dashboard, account, selfservice, more。
*   **Constant `QUALITY_TAB`** (Line 23): 质量检测页签配置。
*   **Constant `MONITOR_TAB`** (Line 24): 监控页签配置。
*   **Function `useNavTabs()`** (Line 26-31): Hook，根据 `enableNetworkQuality` 配置动态决定在第三个位置插入 `QUALITY_TAB` 还是 `MONITOR_TAB`。
*   **Component `BottomNav`** (Line 33-91):
    *   Props: `tab: MobileTab`, `onChange: (t: MobileTab) => void`。
    *   Line 40-45: 根容器，使用 `absolute inset-x-0` 定位，底部留出安全区域间距。
    *   Line 47-51: 磨砂玻璃容器，使用 `.glass-nav` CSS 类实现样式。
    *   Line 54-86: 页签列表循环。每个按钮宽 `w-16`，高 `h-14`。
    *   Line 70-76: 激活指示器，使用 `m.span` 配合 `layoutId="nav-active-pill"` 实现平滑滑动效果。
    *   Line 77-80: 图标尺寸固定为 `26px`。

### DockNav.tsx (Desktop/Tablet Dock)
**文件**: `android/frontend/src/components/layout/DockNav.tsx`

用于桌面和平板底部的 Dock 导航栏，支持鼠标磁吸放大效果。

*   **Constant `ICON_MAP`** (Line 27-36): 将图标名称映射到对应的 Lucide React 组件。
*   **Constants `MAGNETIC_RANGE`** (Line 38), `MAX_SCALE` (Line 39), `MAX_LIFT` (Line 40): 磁吸效果参数。
*   **Component `DockItem`** (Line 42-153):
    *   Props: `id`, `label`, `icon`, `isActive`, `visibleCount`, `onPanelChange`, `mouseX`。
    *   Line 62-74: `useEffect` 初始化 GSAP `quickTo` tween 用于缩放和平移。
    *   Line 76-112: `useEffect` 监听 `mouseX` motion value，计算距离中心点的距离，应用磁吸动画。如果距离小于 `MAGNETIC_RANGE`，则按比例缩放和平移。
    *   Line 132-138: 激活状态药丸，使用 `m.div` 配合 `layoutId="dock-active-pill"`。
    *   Line 145-150: 视觉 Tooltip，仅在 hover/focus 时显示。
*   **Component `ActionButton`** (Line 156-235):
    *   Props: `label`, `loadingLabel`, `icon`, `isLoading`, `isDisabled`, `onAction`, `variant`。
    *   Line 182-190: 加载状态旋转动画，使用 GSAP context。
    *   Line 194-234: 按钮样式，主色或描边变体，支持 hover/tap 动画。
*   **Component `DockNav`** (Line 242-323):
    *   Props: `onPanelChange`, `outerRef`。
    *   Line 244-249: 获取当前激活面板、登录状态和网络质量配置。
    *   Line 251: 过滤可见项，如果网络质量关闭则排除 quality 面板。
    *   Line 257-265: `handleMouseMove` 使用 RAF 节流更新 `mouseX`，仅在 `animActive` 且非 `economy` 模式时启用磁吸。
    *   Line 276-280: 根容器定位，宽度计算基于 `--right-panel-width` CSS 变量，确保 Dock 居中。
    *   Line 286-319: 渲染 `DockItem` 列表，分隔线，以及登录/注销 `ActionButton`。

### RightPanel.tsx (Runtime Log Panel)
**文件**: `android/frontend/src/components/layout/RightPanel.tsx`

桌面/平板右侧的运行时日志面板。

*   **Constants `LOG_ICONS`**, `LOG_COLORS` (Line 20-32), `LOG_BG_COLORS` (Line 34-39), `LOG_BAR_COLORS` (Line 41-46): 日志类型的视觉映射。
*   **Constants `RIGHT_PANEL_ANIM_THRESHOLD`** (Line 51, value 50), `RIGHT_PANEL_ANIM_KEEP_COUNT` (Line 52, value 30): 性能优化阈值。
*   **Component `RightPanel`** (Line 54-279):
    *   Props: `logs`, `onClearLogs`, `outerRef`。
    *   Line 64-125: `handleClearWithAnimation`。使用 GSAP stagger 动画逐条隐藏日志条目。包含 `failsafe` setTimeout 防止 rAF 挂起导致的状态卡死。
    *   Line 127-138: 自动滚动逻辑，当用户未手动滚动超过 40px 时自动到底。
    *   Line 148-157: `useMemo` 分片日志。如果 `logs.length > 50`，前段日志使用普通 `div` 渲染，最近 30 条使用 `m.div` + `AnimatePresence` 以保留进出场动画。这是为了降低 Framer Motion 在大量数据下的 Reconciliation 成本。
    *   Line 166-187: 标题栏，显示日志数量和清除按钮。
    *   Line 190-213: 空状态展示，带有呼吸动画的看板娘背景。
    *   Line 216-273: 日志列表渲染。`staticLogs` 无动画，`animLogs` 有动画。

### TitleBar.tsx (Window Title Bar)
**文件**: `android/frontend/src/components/layout/TitleBar.tsx`

窗口标题栏，包含应用信息、控制按钮和系统控件。

*   **Icon Components** (Line 26-50): `MinimizeIcon`, `MaximizeIcon`, `RestoreIcon`, `CloseIcon` 的内联 SVG 组件。
*   **Component `TitleBar`** (Line 52-278):
    *   Props: `notificationEnabled`, `onToggleNotification`, `onShowTheme`, `onShowAbout`, `onShowSponsor`, `onToggleLightMode`, `onMinimize`, `onToggleMaximize`, `onClose`, `isMaximized`, `showWindowControls`。
    *   Line 73-85: `handleTitleBarMouseDown`。实现拖动功能，排除点击按钮等交互元素，双击检测防抖。调用 `getCurrentWindow().startDragging()`。
    *   Line 87-90: `handleTitleBarDoubleClick`。最大化/还原切换。
    *   Line 100-142: 左侧 Logo 和版本号。如果有更新，显示带红点的版本按钮和新版本提示。
    *   Line 144-231: 右侧工具栏。包括深色模式切换、语言切换、通知开关、主题设置、赞助、关于。
    *   Line 232-273: 窗口控制按钮（最小化、最大化、关闭），仅当 `showWindowControls` 为 true 时渲染。

### MobileDashboard.tsx (Mobile Dashboard Page)
**文件**: `android/frontend/src/components/mobile/MobileDashboard.tsx`

移动端的首页仪表盘，复用桌面 `DashboardPanel` 但注入移动专属卡片。

*   **Constant `EXCLUDED_CARDS`** (Line 35): `['quickActions']`，移动端不渲染 DHCP 续租等桌面专属动作。
*   **Component `MobileStatusCard`** (Line 38-96):
    *   替代原 Hero 卡。展示网络状态指示灯（呼吸灯效果）、Portal 可达性、检查次数。
    *   Line 81-91: 仅在 `formFactor === 'phone'` 时显示运营商徽标。
*   **Component `MobileMonitorCard`** (Line 99-139):
    *   监控摘要卡。包含 Radix Switch 开关和手动刷新按钮。
*   **Component `MobileDashboard`** (Line 141-168):
    *   Line 149-152: 定义 `extraCards`，包含 `mobileHero` 和 `mobileMonitor`。
    *   Line 154-167: 渲染 `DashboardPanel`，传入账号管理 props 和排除/额外卡片配置。

### MobileMore.tsx (Mobile "More" Page)
**文件**: `android/frontend/src/components/mobile/MobileMore.tsx`

移动端“更多”聚合页，包含设置、测速、日志、后台检测等子页面。

*   **Constant `TABS`** (Line 21-26): 定义子页签，其中 `monitor` 标记为 `qualityOnly`。
*   **Component `MobileMore`** (Line 33-84):
    *   Line 37: 根据 `enableNetworkQuality` 过滤 tabs。如果质量功能关闭，`monitor` tab 被移除，且如果当前处于 monitor 状态则降级回 settings。
    *   Line 48-81: 渲染 Chips 切换器和对应的面板组件 (`MonitorPanel`, `SpeedTestPanel`, `LogPanel`, `SettingsPanel`)。

### MobileQuickActions.tsx (Mobile Quick Actions)
**文件**: `android/frontend/src/components/mobile/MobileQuickActions.tsx`

悬浮于底部导航栏上方的快捷登录/注销胶囊。

*   **Component `MobileQuickActions`** (Line 14-78):
    *   Line 24-32: `handleQuickLogin`。先执行 `api.bindToWifi()` 绑定 WLAN，再执行 `doLogin()`。
    *   Line 35-39: 绝对定位，底部间距计算为 `safe-area-inset-bottom + 100px`。
    *   Line 41-57: 注销按钮，磨砂 tonal 风格 (`btn-tonal-glass`)。
    *   Line 58-74: 登录按钮，实心主色风格。

### TabletShell.tsx (Tablet Shell Component)
**文件**: `android/frontend/src/components/tablet/TabletShell.tsx`

平板设备的主外壳组件，协调 TitleBar, StatusBar, DockNav, RightPanel 和主内容区。

*   **Constants `PANEL_TITLES`** (Line 54-63): 各面板的标题和描述 key。
*   **Constants `TOPBAR_ZOOM`** (Line 71, 1.3), `CONTENT_ZOOM` (Line 72, 0.9): 平板触屏适配系数。
*   **Component `TabletShellInner`** (Line 74-380):
    *   Line 79-95: Store hooks 获取状态。
    *   Line 125-129: `useOrientation` 检测横竖屏。`shellStyle` 中，竖屏时将 `--right-panel-width` 置为 0 以隐藏右栏并使 Dock 居中。
    *   Line 133-141: `handlePanelChange`。带锁机制防止动画冲突。
    *   Line 146-151: 首次启动无账号时触发 Onboarding。
    *   Line 153-155: `handleClearLogs`。
    *   Line 160-222: 根据 `deferredPanel` switch-case 渲染对应面板内容 (`MobileDashboard`, `AccountPanel`, `SelfServicePanel`, `MonitorPanel`, `QualityPanel`, `SpeedTestPanel`, `SettingsPanel`, `LogPanel`)。
    *   Line 232-246: TitleBar 包裹在 zoom 容器中以提升可点性。
    *   Line 249-254: StatusBar 包裹在 zoom 容器中。
    *   Line 256-301: 主内容区。使用 `AnimatePresence` 和 `m.div` 实现面板切换动画。
    *   Line 295-300: 横屏时渲染 `RightPanel`。
    *   Line 304-322: 侧边看板娘图片，根据屏幕宽度断点 (`min-[1008px]`, `min-[1330px]`) 显示。
    *   Line 324-326: 渲染 `DockNav`。
    *   Line 328-377: 渲染全局对话框 (`ToastContainer`, `SponsorCard`, `AboutDialog`, `UpdateAvailableDialog`, `ConfirmDialog`, `OnboardingWizard`)。
*   **Component `TabletShell`** (Line 382-388): 错误边界包装。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                      TabletShell / App Root                  │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │ TitleBar (zoom:1.3)                                     │ │
│  ├─────────────────────────────────────────────────────────┤ │
│  │ StatusBar (zoom:1.3)                                    │ │
│  ├─────────────────────────────────────────────────────────┤ │
│  │                                                         │
│  │  ┌──────────────────────┐  ┌────────────────────────┐   │
│  │  │ Main Content Area    │  │ RightPanel (Logs)      │   │
│  │  │ (zoom:0.9)           │  │                        │   │
│  │  │                      │  │ Visible in Landscape   │   │
│  │  │ AnimatePresence:     │  │ Hidden in Portrait     │   │
│  │  │ m.div (panelVariants)│  │                        │   │
│  │  │                      │  │                        │   │
│  │  │ Case:                 │  │                        │   │
│  │  │ - MobileDashboard    │  │                        │   │
│  │  │ - AccountPanel       │  │                        │   │
│  │  │ - SettingsPanel etc. │  │                        │   │
│  │  └──────────────────────┘  └────────────────────────┘   │
│  │                                                         │
│  │  ┌─────────────────────────────────────────────────────┐│
│  │  │ DockNav (Fixed Bottom)                              ││
│  │  │ - Magnetic Items (GSAP quickTo)                     ││
│  │  │ - Active Pill (Framer Motion layoutId)              ││
│  │  └─────────────────────────────────────────────────────┘│
│  └─────────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │ MobileQuickActions (Absolute, above Dock on Phone only) │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **State Initialization**: `TabletShellInner` 通过 Zustand stores (`useAdapterStore`, `useConfigStore`, `useAuthStore` 等) 获取全局状态，如 `activePanel`, `logs`, `accounts`, `isLoggingIn`。
2.  **Navigation Trigger**:
    *   User clicks an item in `DockNav`.
    *   `DockNav` calls `onPanelChange(panelName)` prop.
    *   `TabletShell.handlePanelChange` updates `activePanel` in `useAdapterStore`.
    *   `useDeferredValue(activePanel)` creates a deferred version to allow UI updates without blocking input.
3.  **Content Rendering**:
    *   `TabletShell` switches on `deferredPanel` to render the specific panel component (e.g., `MobileDashboard`).
    *   For `MobileDashboard`, it passes props to `DashboardPanel` and injects `extraCards` (`MobileStatusCard`, `MobileMonitorCard`).
4.  **Log Updates**:
    *   Backend events update `useLogToastStore` logs.
    *   `RightPanel` receives `logs`.
    *   If `logs.length > 50`, `RightPanel` splits logs into `staticLogs` (no animation) and `animLogs` (framer-motion animations).
    *   Auto-scroll logic keeps view at bottom if user hasn't scrolled up.
5.  **Authentication Flow**:
    *   `MobileQuickActions` or `DockNav` ActionButton triggers `doLogin` or `doLogout`.
    *   `MobileQuickActions` may pre-bind WiFi via Tauri API before login.
    *   Auth state changes reflect in UI (buttons disable during loading, show spinners).

## Connections

*   [[android/frontend/src/hooks/useAdapterStore]]: Manages `activePanel` state, central to navigation.
*   [[android/frontend/src/hooks/useConfigStore]]: Provides configuration for network quality, accounts, language, etc.
*   [[android/frontend/src/hooks/useAuthStore]]: Handles login/logout state and actions.
*   [[android/frontend/src/shared/ui-constants]]: Defines `NAV_ITEMS` used by `DockNav`.
*   [[android/frontend/src/auth/DashboardPanel]]: Reused by `MobileDashboard` for desktop-like card structure.
*   [[android/frontend/src/monitor/MonitorPanel]]: Used in `TabletShell` and `MobileMore`.
*   [[android/frontend/src/settings/SettingsPanel]]: Used in `TabletShell` and `MobileMore`.
*   [[android/frontend/src/shared/LogPanel]]: Used in `MobileMore` and `TabletShell`.