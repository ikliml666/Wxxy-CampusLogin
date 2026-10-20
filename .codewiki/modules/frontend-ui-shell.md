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

- **`initTheme()` (L27-L44)**: 读取 `safeStorage` 中的 `campus-light-mode` 和 `campus-theme`，操作 `document.documentElement` 的 class 属性以应用深色模式或特定主题。同时设置 `data-glass` 属性以控制玻璃档位，避免启动闪烁。
- **`setupCrashRecovery()` (L48-L116)**: 
    - L62-L69: 监听 `visibilitychange`，当页面从后台恢复且 `#root` 为空时触发重载。
    - L71-L77: 监听 `window.error`，捕获包含 "GPU"、"WebGL"、"SharedArrayBuffer" 的错误并触发重载。
    - L79-L113: 心跳检测机制。每 1s 更新 `lastHeartbeatTime`（若 rAF 存活），每 2s 检查心跳间隔。若超过 10s 未更新，判定为渲染链失活（疑似 GPU 崩溃），触发重载。
- **`ReactDOM.createRoot` (L122-L131)**: 挂载应用。在开发环境下包裹 `React.StrictMode`。使用 `LazyMotion` 和 `MotionConfig` 优化 framer-motion 性能。

#### `android/frontend/src/App.tsx`

- **`AppInner` (L53-L369)**: 移动端主视图容器。
    - **Hooks**: `useAppInit`, `useAdaptiveFramePace`, `useTranslation`, `useFormFactor` (隐含在导出中)。
    - **State**: 
        - `tab`: 当前底栏页签 (`MobileTab`)，持久化到 `safeStorage`。
        - `navDir`: 面板切换方向 (-1, 0, 1)，用于 framer-motion 转场动画。
        - Dialog States: `aboutOpen`, `themeOpen`, `sponsorOpen`, `onboardingOpen`, `confirmDelete`。
    - **Logic**:
        - **Tab Routing (L149-L193)**: 根据 `deferredTab` 渲染对应的面板组件 (`MobileDashboard`, `AccountPanel`, `SelfServicePanel`, `QualityPanel`, `MonitorPanel`, `MobileMore`)。
        - **Header (L206-L270)**: 绝对定位的顶部轻 Header。包含状态指示灯、运营商 Chip、更新提醒胶囊、网络质量胶囊、以及设置/主题/关于按钮。
        - **Main Content (L280-L307)**: 滚动区域，使用 `AnimatePresence` 和 `m.div` 配合 `panelVariants` 实现基于 `navDir` 的方向性面板切换动画。
        - **Footer**: `BottomNav` 组件。
    - **Dialogs**: 渲染 `SponsorCard`, `AboutDialog`, `UpdateAvailableDialog`, `ThemeDialog`, `ConfirmDialog`, `OnboardingWizardMobile`。
- **`App` (L373-L385)**: 顶层组件。
    - 调用 `useFormFactor()` 获取设备形态。
    - 若为 `'tablet'`，渲染 `<TabletShell />`；否则渲染 `<AppInner />`。
    - 始终渲染 `FaceCaptureDialog` 和 `NotificationPermissionGate`。
- **`NotificationPermissionGate` (L391-L415)**: 首次启动时引导通知权限。仅询问一次，标记写入 `safeStorage`。
- **`EXCLUDED_CARDS` (MobileDashboard.tsx L36)**: 常量数组 `['quickActions']`，用于在移动端 Dashboard 中排除桌面版的快速操作卡片。

### Layout Shells

#### `android/frontend/src/components/layout/BottomNav.tsx`

- **`MobileTab` Type (L14)**: 联合类型 `'dashboard' | 'account' | 'selfservice' | 'quality' | 'monitor' | 'more'`。
- **`TABS` (L16-L21)**: 默认页签配置列表。
- **`QUALITY_TAB` / `MONITOR_TAB` (L23-L24)**: 质量检测开关控制的动态页签。
- **`useNavTabs()` (L26-L31)**: Hook。根据 `config.enableNetworkQuality` 动态插入 `quality` 或 `monitor` 到 TABS 列表中。
- **`BottomNav` Component (L33-L91)**:
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