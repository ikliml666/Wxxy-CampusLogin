---
title: adaptive-form-factor-layout
type: concept
source_files:
  - android/frontend/src/App.tsx
  - android/frontend/src/components/tablet/TabletShell.tsx
  - android/frontend/src/hooks/useFormFactor.ts
tags: [responsive, layout, tablet, mobile, state-management]
---

## Overview

`adaptive-form-factor-layout` 是一种跨设备形态的响应式布局架构模式，通过感知设备物理维度（短边 `sw600dp` 语义）动态切换整个应用的渲染外壳（Shell）与导航范式。核心目标是在手机与平板形态间建立过渡：手机采用单栏流式布局 + 底部固定导航（`BottomNav`），平板采用桌面同源多栏布局（复用 `TitleBar`/`StatusBar`/`DockNav`/`RightPanel`）并搭配安卓版面板内容。状态管理通过 Zustand stores 与视图解耦，业务逻辑保持一致。

## Key Components

### `android/frontend/src/hooks/useFormFactor.ts`

- **`FormFactor`** (L9): 导出类型，联合 `'phone' | 'tablet'`。
- **`Orientation`** (L10): 导出类型，联合 `'portrait' | 'landscape'`。
- **`TABLET_MIN_EDGE_PX`** (L13): 常量，平板短边阈值 `600`（CSS px）。
- **`detectFormFactor`** (L15-17): 函数，返回 `Math.min(window.innerWidth, window.innerHeight) >= TABLET_MIN_EDGE_PX ? 'tablet' : 'phone'`。
- **`detectOrientation`** (L19-21): 函数，返回 `window.innerWidth >= window.innerHeight ? 'landscape' : 'portrait'`。
- **`useViewportMeasure`** (L24-36): 内部泛型 Hook `useViewportMeasure<T>(calc: () => T): T`，订阅 `resize` 与 `orientationchange` 事件重算值。
- **`useFormFactor`** (L38-40): 导出 Hook，调用 `useViewportMeasure(detectFormFactor)` 返回 `FormFactor`。
- **`useOrientation`** (L43-45): 导出 Hook，调用 `useViewportMeasure(detectOrientation)` 返回 `Orientation`。

### `android/frontend/src/App.tsx`

- **`AppInner`** (L59-366): 手机外壳主组件。管理 `tab` 状态（L64-69，白名单 `['dashboard','account','selfservice','quality','monitor','more']`），根据 `deferredTab` 切换 `panelContent`（L155-199）；渲染绝对定位轻 header（L209-267）、快捷登录浮条（L269-274）、`main` 滚动区（L277-304）、`BottomNav`（L306）、赞助/关于/主题弹窗及 `OnboardingWizard`（L308-363）。桌面件（TitleBar 等）不渲染（见 L3-4 注释）。
- **`App`** (L374-400): 默认导出函数组件。使用 `useFormFactor()`（L375）决定渲染 `<TabletShell />`（平板，L389）或 `<AppInner />`（手机，L392），并挂载共用的 `FaceCaptureDialog` 与 `NotificationPermissionGate`，外层包 `AnimationActiveProvider` 与 `ErrorBoundary`。
- **`NotificationPermissionGate`** (L406-430): 内部组件，首次启动（`!safeStorage.get('campus-notification-prompt')`）弹确认框请求通知权限（L409-411, L417-428），仅询问一次。
- 懒加载模块常量：`AboutDialog`（L54）、`MobileQuickActions`（L55）、`ThemeDialog`（L56）、`OnboardingWizard`（L57）。
- 内部状态形状：`confirmDelete` 为 `{ open: boolean; name: string; displayName: string }`（L117）。

### `android/frontend/src/components/tablet/TabletShell.tsx`

- **`PANEL_TITLES`** (L54-63): 常量记录，键为面板名（`'dashboard'|'account'|'selfservice'|'monitor'|'quality'|'speedtest'|'settings'|'log'`），值为对象含 `titleKey: string` 与 `descKey: string` 字段（无 `network`）。
- **`PANEL_CONTAINER_STYLE`** (L66): `React.CSSProperties` 常量，`{ contain: 'layout style' }`。
- **`TOPBAR_ZOOM`** (L71): 常量 `1.3`，平板顶栏 CSS zoom 系数。
- **`CONTENT_ZOOM`** (L72): 常量 `0.9`，内容区 CSS zoom 系数。
- **`TabletShellInner`** (L74-380): 平板外壳主组件。使用 `useAdapterStore` 的 `activePanel`（L79）切换面板内容（L159-222），渲染 `TitleBar`（L233-245，无窗口控制）、`StatusBar`（L250-253）、主区与 `RightPanel`（横屏时 L295-300）、侧边看板娘（L304-322）、`DockNav`（L324-326）及各类弹窗（L328-377）。竖屏时通过 `--right-panel-width: 0px` 隐藏右栏（L125-129）。
- **`TabletShell`** (L382-388): 导出组件，用 `ErrorBoundary` 包裹 `TabletShellInner`。
- 内部状态形状：`confirmDelete` 为 `{ open: boolean; name: string; displayName: string }`（L116）。

## Architecture

```text
┌─────────────────────────────┐
│ useFormFactor.ts (L38-45)   │
│ useFormFactor()->FormFactor │
│ useOrientation()->Orient.   │
└───────────────┬─────────────┘
                │ formFactor
                ▼
┌─────────────────────────────┐
│ App.tsx L374 App()          │
│ if formFactor==='tablet'    │
│   render TabletShell        │
│ else render AppInner        │
└───────────────┬─────────────┘
       ┌────────┴────────┐
       ▼                 ▼
┌──────────────────────┐  ┌──────────────────────┐
│ AppInner L59          │  │ TabletShell L382      │
│ BottomNav    (L306)   │  │ TitleBar/StatusBar    │
│ Mobile panels         │  │ DockNav/RightPanel    │
│ (dashboard..)         │  │ Android panels        │
└──────────────────────┘  └──────────────────────┘
```

## Data Flow

1. **状态采集**：`useFormFactor` (L38-40) 与 `useOrientation` (L43-45) 在挂载时通过 `useViewportMeasure` 读取 `window.innerWidth/innerHeight`，短边≥600 判定 `tablet` (L16)，宽≥高判定 `landscape` (L20)；监听 `resize`/`orientationchange` 实时更新 (L26-33)。
2. **外壳切换**：`App` 组件 (L374-400) 订阅 `formFactor`，`formFactor === 'tablet'` 渲染 `TabletShell` (L389)，否则 `AppInner` (L392)。
3. **导航适配**：
   - 手机：`AppInner` 内部 `tab` 状态驱动 `BottomNav` (L306) 与 `panelContent` (L155-199)；`tabOrder` 依据 `configEnableNetworkQuality` 动态将第4位设为 `quality` 或 `monitor` (L77-83)。
   - 平板：`TabletShellInner` 使用 `useAdapterStore.activePanel` (L79) 驱动 `DockNav` (L324) 与面板渲染；横屏 (`isLandscape`) 才显示 `RightPanel` (L125-129, L295-300)。
4. **持久化与恢复**：手机 `tab` 存入 `safeStorage('campus-mobile-tab')` (L65, L133)；平板 `activePanel` 存入 `safeStorage('campus-active-panel')` (L138)。配置加载后纠正越界 tab (L138-143 KI#14)。

## Connections

- 状态管理依赖 [[useConfigStore]]、[[useAdapterStore]] (TabletShell L79, L87)、[[useQualityStore]]、[[useAuthStore]] 等（见各文件 import）。
- UI 组件关联 [[BottomNav]] (App.tsx L31, L306)、[[DockNav]] (TabletShell L36, L324)、[[TitleBar]]、[[StatusBar]]、[[RightPanel]] 等。
- 初始化与动画依赖 [[useAppInit]] (App.tsx L8, TabletShell L13)、[[useAdaptiveFramePace]] (L43, L54, L76)、[[useAnimationProfile]] (L44, L115)。
- 面板内容组件如 [[MobileDashboard]]、[[AccountPanel]]、[[MonitorPanel]] 等跨外壳复用。

## Known Issues

- **KI#14 页签白名单与恢复**：`App.tsx` L64-69 定义 `tab` 初始白名单须覆盖 `handleTabChange` 可写入全部页签（含 `monitor`），否则杀进程重开会回落 `dashboard`。L138-143 在配置就绪后若 `configEnableNetworkQuality !== false` 且 `tab === 'monitor'` 则强制切到 `quality`，避免底栏无高亮。
- **平板竖屏右栏隐藏**：`TabletShell.tsx` L125-129 设置 `--right-panel-width: 0px` 使 `DockNav` 全视口居中，L295-300 仅在 `isLandscape` 渲染 `RightPanel`；日志功能仍可由 Dock "日志" 面板到达。
- **质量检测关闭时 monitor 补位**：`App.tsx` L74 当 `configEnableNetworkQuality !== false` 为假时 `tabOrder` 第4位为 `monitor`；L186-194 渲染 `MonitorPanel` 而非 `QualityPanel`。默认关闭省电。
- **桌面件在手机外壳不渲染**：`App.tsx` L3-4 注释明确手机外壳不渲染桌面件（TitleBar/StatusBar/RightPanel/DockNav/FluidBackground/Onboarding/Sponsor 等），但 `SponsorCard` 与 `OnboardingWizard` 以移动适配版仍渲染（L310, L354-363），需区分。
- **平板外壳注释与代码不一致**：`TabletShell.tsx` L7 注释称“无 Onboarding 向导”，但 L51 导入、L146-151 首次无账号弹向导、L368-377 渲染 `OnboardingWizard`，实际已实现。
- **network 面板未接入**：`TabletShell.tsx` L10 注释明确 `network` 面板不接入（安卓 NAV_ITEMS 无此项，DNS 优化为 Windows 能力），`PANEL_TITLES` (L54-63) 也未列 `network`。
- **通知权限单次弹窗**：`App.tsx` L406-430 `NotificationPermissionGate` 仅当 `safeStorage` 无 `campus-notification-prompt` 时弹出，点任意按钮标记后不再弹。
