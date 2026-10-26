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

- **懒加载模块常量**：
  - `TabletShell` (L39-41): `lazy(() => import('@/components/tablet/TabletShell')...)`。
  - `AboutDialog` (L54): 移动端 `AboutDialogMobile` 懒加载。
  - `MobileQuickActions` (L55): 快捷登录浮条懒加载。
  - `ThemeDialog` (L56): 主题弹窗懒加载。
  - `OnboardingWizard` (L57): 新手向导懒加载。
  - `FaceCaptureDialog` (L42): 静态导入，双外壳共用单例。
- **`AppInner`** (L59-366): 手机外壳主组件。
  - `tab` 状态 (L64-69)：白名单 `['dashboard','account','selfservice','quality','monitor','more']`，从 `safeStorage('campus-mobile-tab')` 恢复。
  - `deferredTab` (L70)：`useDeferredValue(tab)`。
  - `tabOrder` (L77-83)：依据 `configEnableNetworkQuality` 动态将第4位设为 `quality` 或 `monitor`。
  - `navDir` (L84) 与 `handleTabChange` (L126-134)：方向性转场与持久化。
  - `panelContent` 切换 (L155-199)：根据 `deferredTab` 渲染 `MobileDashboard`/`AccountPanel`/`SelfServicePanel`/`QualityPanel`/`MonitorPanel`/`MobileMore`。
  - 绝对定位轻 header (L209-267)：状态点、运营商 chip、更新胶囊、质量胶囊、主题/关于/赞助入口。
  - 快捷登录浮条 (L270-274)：仅 dashboard 页签挂载。
  - `main` 滚动区 (L277-304)：`AnimatePresence` 面板转场。
  - `BottomNav` (L306)：底部导航。
  - 弹窗与向导 (L308-363)：赞助卡、关于、更新提示、主题、删除确认、新手向导。
  - 桌面件（TitleBar/StatusBar/RightPanel/DockNav 等）不在手机外壳渲染（见 L3-4、L34-38 注释）。
- **`App`** (L374-400): 默认导出函数组件。
  - `useFormFactor()` (L375) 判定 `isTablet` (L376)。
  - 平板预取 `TabletShell` 模块 (L379-381)。
  - 条件渲染 `<TabletShell />` 或 `<AppInner />` (L385-393)，外层 `AnimationActiveProvider` + `ErrorBoundary`。
  - 共用 `FaceCaptureDialog` (L395) 与 `NotificationPermissionGate` (L396)。
- **`NotificationPermissionGate`** (L406-430): 内部组件，首次启动（`!safeStorage.get('campus-notification-prompt')`）弹确认框请求通知权限，点任意按钮标记后不再弹。

### `android/frontend/src/components/tablet/TabletShell.tsx`

- **懒加载模块常量**：
  - `AboutDialog` (L49)：桌面版关于弹窗。
  - `ThemeDialog` (L50)：主题弹窗。
  - `OnboardingWizard` (L51)：新手向导。
- **`PANEL_TITLES`** (L54-63): 常量记录，键为面板名（`'dashboard'|'account'|'selfservice'|'monitor'|'quality'|'speedtest'|'settings'|'log'`），值为 `{ titleKey, descKey }`，无 `network`。
- **`PANEL_CONTAINER_STYLE`** (L66): `React.CSSProperties` 常量，`{ contain: 'layout style' }`。
- **`TOPBAR_ZOOM`** (L73): 常量 `1.3`，平板顶栏 CSS zoom 系数（内容区不再缩放，原 `CONTENT_ZOOM` 已移除，见 L70-72 注释）。
- **`PanelSkeleton`** (L77-87): 面板加载骨架屏。
- **`TabletShellInner`** (L89-408): 平板外壳主组件。
  - `activePanel` 来自 `useAdapterStore` (L94)，`deferredPanel` (L95)。
  - `handlePanelChange` (L148-156)：带锁切换面板并写 `safeStorage('campus-active-panel')` (L153)。
  - 首次无账号弹向导 effect (L161-166)。
  - `panelContent` 切换 (L174-237)：覆盖 dashboard/account/selfservice/monitor/quality/speedtest/settings/log，全部使用安卓版面板。
  - 渲染 `TitleBar` (L248-261，无窗口控制)、`StatusBar` (L265-268)、主区 (L275-321)、横屏时 `RightPanel` (L323-328)、侧边看板娘 (L332-350)、`DockNav` (L352-354) 及弹窗 (L356-405)。
  - 竖屏通过 `--right-panel-width: 0px` (L140-144) 隐藏右栏并使 Dock 全视口居中。
- **`TabletShell`** (L410-416): 导出组件，用 `ErrorBoundary` 包裹 `TabletShellInner`。
- 内部状态形状：`confirmDelete` 为 `{ open: boolean; name: string; displayName: string }` (App.tsx L117, TabletShell.tsx L131)。

## Architecture

```text
┌─────────────────────────────┐
│ useFormFactor.ts L38-45     │
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
┌──────────────┐  ┌──────────────────────┐
│ AppInner L59 │  │ TabletShell L410      │
│ BottomNav    │  │ TitleBar/StatusBar    │
│ Mobile panels│  │ DockNav/RightPanel    │
│ (dashboard..)│  │ Android panels        │
└──────────────┘  └──────────────────────┘
```

## Data Flow

1. **状态采集**：`useFormFactor` (L38-40) 与 `useOrientation` (L43-45) 在挂载时通过 `useViewportMeasure` (L24-36) 读取 `window.innerWidth/innerHeight`，短边≥600 判定 `tablet` (L16)，宽≥高判定 `landscape` (L20)；监听 `resize`/`orientationchange` 实时更新 (L26-33)。
2. **外壳切换**：`App` 组件 (L374-400) 订阅 `formFactor`，`formFactor === 'tablet'` 渲染 `TabletShell` (L389)，否则 `AppInner` (L392)；平板形态下预取外壳 chunk (L379-381)。
3. **导航适配**：
   - 手机：`AppInner` 内部 `tab` 状态驱动 `BottomNav` (L306) 与 `panelContent` (L155-199)；`tabOrder` 依据 `configEnableNetworkQuality` 动态将第4位设为 `quality` 或 `monitor` (L77-83)。
   - 平板：`TabletShellInner` 使用 `useAdapterStore.activePanel` (L94) 驱动 `DockNav` (L352) 与面板渲染；横屏 (`isLandscape` L140) 才显示 `RightPanel` (L323-328)。
4. **持久化与恢复**：手机 `tab` 存入 `safeStorage('campus-mobile-tab')` (L65, L133)；平板 `activePanel` 存入 `safeStorage('campus-active-panel')` (L153)。配置加载后纠正越界 tab：App.tsx L138-143 在 `configEnableNetworkQuality !== false` 且 `tab === 'monitor'` 时强制切到 `quality`（KI#14 配套）。

## Connections

- 状态管理依赖 [[useConfigStore]] (App.tsx L9,L72-75; TabletShell.tsx L15,L96-101)、[[useAdapterStore]] (TabletShell.tsx L14,L94,L102)、[[useQualityStore]] (App.tsx L15,L94-101; TabletShell.tsx L17,L103-108)、[[useAuthStore]] (App.tsx L10,L88-91)、[[useLogToastStore]] (App.tsx L13,L106-111; TabletShell.tsx L18,L109,L117-124) 等。
- UI 组件关联 [[BottomNav]] (App.tsx L31,L306)、[[DockNav]] (TabletShell.tsx L36,L352)、[[TitleBar]] (TabletShell.tsx L34,L248)、[[StatusBar]] (TabletShell.tsx L35,L265)、[[RightPanel]] (TabletShell.tsx L37,L323) 等。
- 初始化与动画依赖 [[useAppInit]] (App.tsx L8,L60; TabletShell.tsx L13,L90)、[[useAdaptiveFramePace]] (App.tsx L50,L61; TabletShell.tsx L45,L91)、[[useAnimationProfile]] (App.tsx L51,L122)、[[usePageIdle]] (App.tsx L49)。
- 面板内容组件如 [[MobileDashboard]]、[[AccountPanel]]、[[MonitorPanel]]、[[QualityPanel]]、[[SelfServicePanel]] 等跨外壳复用。

## Known Issues

- **KI#14 页签白名单与恢复**：`App.tsx` L64-69 定义 `tab` 初始白名单须覆盖 `handleTabChange` 可写入全部页签（含 `monitor`），否则杀进程重开会回落 `dashboard`。L138-143 在配置就绪后若 `configEnableNetworkQuality !== false` 且 `tab === 'monitor'` 则强制切到 `quality`，避免底栏无高亮。
- **平板竖屏右栏隐藏**：`TabletShell.tsx` L140-144 设置 `--right-panel-width: 0px` 使 `DockNav` 全视口居中，L323-328 仅在 `isLandscape` 渲染 `RightPanel`；日志功能仍可由 Dock "日志" 面板到达。
- **质量检测关闭时 monitor 补位**：`App.tsx` L77-83 当 `configEnableNetworkQuality !== false` 为假时 `tabOrder` 第4位为 `monitor`；L186-195 渲染 `MonitorPanel` 而非 `QualityPanel`。默认关闭省电。
- **桌面件在手机外壳不渲染**：`App.tsx` L3-4 注释明确手机外壳不渲染桌面件（TitleBar/StatusBar/RightPanel/DockNav/FluidBackground/Onboarding/Sponsor 等），但 `SponsorCard` (L310) 与 `OnboardingWizard` (L355-363) 以移动适配版仍渲染，需区分。
- **平板外壳注释与代码不一致**：`TabletShell.tsx` L7 注释称“无 Onboarding 向导”，但 L51 导入、L161-166 首次无账号弹向导、L396-405 渲染 `OnboardingWizard`，实际已实现。
- **network 面板未接入**：`TabletShell.tsx` L10 注释明确 `network` 面板不接入（安卓 NAV_ITEMS 无此项，DNS 优化为 Windows 能力），`PANEL_TITLES` (L54-63) 也未列 `network`。
- **通知权限单次弹窗**：`App.tsx` L406-430 `NotificationPermissionGate` 仅当 `safeStorage` 无 `campus-notification-prompt` 时弹出，点任意按钮标记后不再弹。
