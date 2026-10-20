---
title: frontend-ui-shell
type: module
source_files:
  - android/frontend/src/main.tsx
  - android/frontend/src/App.tsx
  - android/frontend/src/components/layout/BottomNav.tsx
  - android/frontend/src/components/layout/DockNav.tsx
  - android/frontend/src/components/layout/RightPanel.tsx
  - android/frontend/src/components/layout/TitleBar.tsx
  - android/frontend/src/components/mobile/MobileDashboard.tsx
  - android/frontend/src/components/mobile/MobileMore.tsx
  - android/frontend/src/components/mobile/MobileQuickActions.tsx
tags: [frontend, shell, responsive, layout, react]
---

## Overview

`frontend-ui-shell` 是 Android Tauri 应用的核心 UI 入口与布局层。它负责初始化全局运行时环境（主题、GSAP 动画引擎、崩溃恢复机制），并根据设备形态（手机 vs 平板）动态渲染不同的外壳（Shell）。

- **应用入口**：`main.tsx` 处理 React 根节点挂载、framer-motion 懒加载、GSAP 全局配置及 GPU/WebGL 崩溃恢复逻辑。
- **路由与状态管理**：`App.tsx` 作为双壳入口，通过 `useFormFactor` 钩子判断屏幕宽度（sw600dp 语义），决定渲染 `TabletShell`（桌面 Dock 布局）还是 `AppInner`（移动端底部导航布局）。
- **共享组件库**：提供通用的 UI 基元，如 `AnimatedCard`（带 3D 倾斜效果）、`Badge`、`Button`（带鼠标跟随光效）、`EmptyState` 等。

## Key Components

### Entry Point & Initialization

#### `android/frontend/src/main.tsx`

- **`initTheme()` (L27-L41)**: 读取 `safeStorage` 中的 `campus-light-mode` 和 `campus-theme`，操作 `document.documentElement` 的 class 属性以应用深色模式或特定主题。
- **`setupCrashRecovery()` (L45-L111)**: 
    - L59-L66: 监听 `visibilitychange`，当页面从后台恢复且 `#root` 为空时触发重载。
    - L68-L74: 监听 `window.error`，捕获包含 "GPU"、"WebGL"、"SharedArrayBuffer" 的错误并触发重载。
    - L76-L110: 心跳检测机制。每 1s 更新 `lastHeartbeatTime`（若 rAF 存活），每 2s 检查心跳间隔。若超过 10s 未更新，判定为渲染链失活（疑似 GPU 崩溃），触发重载。
- **`ReactDOM.createRoot` (L119-L129)**: 挂载应用。在开发环境下包裹 `React.StrictMode`。使用 `LazyMotion` 和 `MotionConfig` 优化 framer-motion 性能。

#### `android/frontend/src/App.tsx`

- **`AppInner` (L52-L359)**: 移动端主视图容器。
    - **Hooks**: `useAppInit`, `useAdaptiveFramePace`, `useTranslation`, `useFormFactor` (隐含在导出中)。
    - **State**: 
        - `tab`: 当前底栏页签 (`MobileTab`)，持久化到 `safeStorage`。
        - `navDir`: 面板切换方向 (-1, 0, 1)，用于 framer-motion 转场动画。
        - Dialog States: `aboutOpen`, `themeOpen`, `sponsorOpen`, `onboardingOpen`, `confirmDelete`。
    - **Logic**:
        - **Tab Routing (L148-L192)**: 根据 `deferredTab` 渲染对应的面板组件 (`MobileDashboard`, `AccountPanel`, `SelfServicePanel`, `QualityPanel`, `MonitorPanel`, `MobileMore`)。
        - **Header (L199-L260)**: 绝对定位的顶部轻 Header。包含状态指示灯、运营商 Chip、更新提醒胶囊、网络质量胶囊、以及设置/主题/关于按钮。
        - **Main Content (L269-L297)**: 滚动区域，使用 `AnimatePresence` 和 `m.div` 配合 `panelVariants` 实现基于 `navDir` 的方向性面板切换动画。
        - **Footer**: `BottomNav` 组件。
    - **Dialogs**: 渲染 `SponsorCard`, `AboutDialog`, `UpdateAvailableDialog`, `ThemeDialog`, `ConfirmDialog`, `OnboardingWizardMobile`。
- **`App` (L363-L375)**: 顶层组件。
    - 调用 `useFormFactor()` 获取设备形态。
    - 若为 `'tablet'`，渲染 `<TabletShell />`；否则渲染 `<AppInner />`。
    - 始终渲染 `FaceCaptureDialog` 和 `NotificationPermissionGate`。
- **`NotificationPermissionGate` (L381-L404)**: 首次启动时引导通知权限。仅询问一次，标记写入 `safeStorage`。
- **`EXCLUDED_CARDS` (MobileDashboard.tsx L36)**: 常量数组 `['quickActions']`，用于在移动端 Dashboard 中排除桌面版的快速操作卡片。

### Layout Shells

#### `android/frontend/src/components/layout/BottomNav.tsx`

- **`MobileTab` Type (L14)**: 联合类型 `'dashboard' | 'account' | 'selfservice' | 'quality' | 'monitor' | 'more'`。
- **`TABS` (L16-L21)**: 默认页签配置列表。
- **`QUALITY_TAB` / `MONITOR_TAB` (L23-L24)**: 质量检测开关控制的动态页签。
- **`useNavTabs()` (L26-L31)**: Hook。根据 `config.enableNetworkQuality` 动态插入 `quality` 或 `monitor` 到 TABS 列表中。
- **`BottomNav` Component (L33-L96)**:
    - 磨砂玻璃胶囊样式 (`backdrop-blur-md`)。
    - 使用 `layoutId="nav-active-pill"` 实现选中指示器的平滑滑动动画。
    - 图标尺寸 26px，标签 11px。

#### `android/frontend/src/components/layout/DockNav.tsx`

- **`DockItem` (L42-L153)**: Dock 单个图标项。
    - **Magnetic Effect (L76-L112)**: 使用 GSAP `quickTo` 实现鼠标靠近时的缩放 (`scale`) 和上浮 (`y`) 效果。范围 `MAGNETIC_RANGE = 80`。
    - **Active State**: 使用 `layoutId="dock-active-pill"` 实现药丸背景在图标间的平滑过渡。
- **`ActionButton` (L156-L235)**: 登录/注销按钮。
    - 支持加载状态旋转动画 (`gsap.to(... rotation: 360)`)。
    - 视觉上有脉冲边框 (`loadingPulseRef`)。
- **`DockNav` (L242-L322)**:
    - 固定定位在底部中央。
    - 使用 `useMotionValue` 和 RAF 节流跟踪鼠标 X 坐标以驱动磁吸效果。
    - 过滤显示项：若 `enableNetworkQuality` 为 false，隐藏 `quality` 面板项。

#### `android/frontend/src/components/layout/RightPanel.tsx`

- **`RightPanel` (L54-L278)**: 右侧运行日志面板。
    - **Performance Optimization (FP-4)**: 
        - `RIGHT_PANEL_ANIM_THRESHOLD = 50`: 日志超过 50 条时，前段静态渲染 (`staticLogs`)，仅最近 30 条 (`animLogs`) 使用 framer-motion 动画。
    - **Clear Animation (L64-L125)**: 清空日志时使用 GSAP stagger 动画逐条移除，并配备 `setTimeout` 兜底防止 rAF 停滞导致死锁。
    - **Auto-scroll**: 监听 `scroll` 事件，新日志到来时自动滚动到底部。

#### `android/frontend/src/components/layout/TitleBar.tsx`

- **`TitleBar` (L52-L277)**: 桌面端标题栏。
    - 包含应用图标、版本号、更新提示、语言切换、主题切换、通知开关、窗口控制按钮（最小化/最大化/关闭）。
    - **Window Controls**: 仅在 `showWindowControls=true` 时渲染。支持拖拽 (`startDragging`) 和双击最大化。

### Mobile Specific Views

#### `android/frontend/src/components/mobile/MobileDashboard.tsx`

- **`MobileStatusCard` (L38-L96)**: 移动总览状态卡。显示连接状态灯、Portal 可达性、检查次数、运营商信息。
- **`MobileMonitorCard` (L99-L139)**: 移动监控摘要卡。包含后台检测开关 (`Switch`) 和手动刷新按钮。
- **`MobileDashboard` (L141-L167)**: 组合上述卡片，并通过 `extraCards` 注入 `DashboardPanel`。

#### `android/frontend/src/components/mobile/MobileMore.tsx`

- **`MobileMore` (L33-L83)**: "更多"聚合页。
    - 顶部 Chips 切换子页：设置、测速、日志、监控（可选）。
    - 复用桌面版 `SettingsPanel`, `SpeedTestPanel`, `LogPanel`, `MonitorPanel`。

#### `android/frontend/src/components/mobile/MobileQuickActions.tsx`

- **`MobileQuickActions` (L14-L83)**: 首页悬浮快捷操作条。
    - 包含“绑定 WLAN 并登录”和“注销”两个胶囊按钮。
    - 绝对定位在底栏上方，不随内容滚动。

### Shared UI Library

#### `android/frontend/src/components/ui/animated-card.tsx`

- **`AnimatedCard` (L26-L166)**:
    - **Tilt Effect**: 鼠标悬停时产生 3D 倾斜 (`rotateX`, `rotateY`)。使用 GSAP `quickTo` 和 RAF 节流。
    - **Entry Animation**: 支持入场动画 (`card-enter` class)。
    - **Props**: `noHover`, `noAnimation`, `enableTilt`, `staggerIndex`。

#### `android/frontend/src/components/ui/badge.tsx`

- **`Badge` (L23-L29)**: 基于 `cva` 的状态徽章。支持多种变体 (`default`, `destructive`, `success` 等) 和尺寸。

#### `android/frontend/src/components/ui/button.tsx`

- **`Button` (L38-L138)**:
    - **Mouse Follow Glow**: 鼠标移动时更新 CSS 变量 `--mouse-x`, `--mouse-y`，配合 CSS 实现光效跟随。
    - **Loading State**: 内置旋转 SVG 加载器。
    - **AsChild**: 支持 Radix Slot 透传。

#### `android/frontend/src/components/ui/empty-state.tsx`

- **`EmptyState` (L14-L32)**: 统一空状态展示组件。支持紧凑模式 (`compact`)。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                      main.tsx (Entry)                        │
│  ├─ initTheme()                                              │
│  ├─ setupCrashRecovery() (rAF Heartbeat + Error Catch)       │
│  └─ <ErrorBoundary> -> <LazyMotion> -> <App />               │
└──────────────────────┬───────────────────────────────────────┘
                       │
┌──────────────────────▼───────────────────────────────────────┐
│                         App.tsx                              │
│  ├─ useFormFactor()                                          │
│  │   ├─ 'phone'  ► <AppInner />                             │
│  │   │     ├─ Header (Absolute)                             │
│  │   │     ├─ Main (Scrollable + AnimatePresence)           │
│  │   │     │     ├─ MobileDashboard                         │
│  │   │     │     ├─ AccountPanel                            │
│  │   │     │     ├─ SelfServicePanel                        │
│  │   │     │     ├─ QualityPanel / MonitorPanel             │
│  │   │     │     └─ MobileMore (Chips Sub-nav)              │
│  │   │     └─ BottomNav (Capsule + layoutId)                │
│  │   └─ 'tablet' ► <TabletShell />                          │
│  │         ├─ TitleBar                                      │
│  │         ├─ DockNav (Magnetic Items)                      │
│  │         ├─ RightPanel (Logs)                             │
│  │         └─ Panel Content                                 │
│  └─ Global Overlays: FaceCaptureDialog, NotificationGate     │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **Initialization**: `main.tsx` 初始化主题和崩溃恢复。`App.tsx` 加载用户配置、账户信息和网络质量状态。
2.  **Form Factor Detection**: `useFormFactor` 返回 `'phone'` 或 `'tablet'`。
3.  **Routing (Phone)**:
    - `AppInner` 维护 `tab` 状态。
    - `handleTabChange` 计算导航方向 `navDir`，更新 `safeStorage`。
    - `AnimatePresence` 根据 `navDir` 播放面板进出动画。
    - 顶部的 `MobileQuickActions` 仅在 `dashboard` tab 显示。
4.  **Routing (Tablet)**:
    - `TabletShell` 渲染左侧主内容和右侧 `RightPanel`。
    - `DockNav` 管理 `activePanel` 状态，点击触发面板切换。
5.  **State Management**:
    - 账户相关：`useConfigStore`, `useAuthStore`, `useAccount`。
    - 网络质量：`useQualityStore`, `useMonitor`。
    - 设置：`useThemeStore`, `useSettings`。

## Connections

- [[shared-store]]: 所有状态管理依赖 Zustand stores。
- [[tauri-api]]: 登录、绑定 WiFi、打开外部链接等操作通过 Tauri API 调用。
- [[animations]]: 全局动画配置由 `framer-motion` 和 `gsap` 共同管理。
- [[settings]]: 主题、语言、新手向导等设置逻辑。

## Known Issues

1.  **Crash Recovery False Positives**: `setupCrashRecovery` 中的心跳检测阈值放宽至 10s (L93)，以避免低端机长 GC 误判。但在极端负载下仍可能漏检或误检。
2.  **GSAP Context Revert**: 在 `RightPanel` 的 `handleClearWithAnimation` 中，使用了 `setTimeout` 兜底来防止 rAF 停滞导致的 `isClearing` 死锁。如果动画被中断，`onInterrupt` 会重置状态，但需确保 `onComplete` 和 `onInterrupt` 的逻辑互斥 (L106-L112)。
3.  **Android WebView Background**: 安卓 WebView 在后台时 rAF 可能暂停，导致 GSAP ticker 冻结。`main.tsx` 中的 `setupCrashRecovery` 尝试通过 `visibilitychange` 恢复 ticker (L75)，但部分 ROM 可能不触发此事件。