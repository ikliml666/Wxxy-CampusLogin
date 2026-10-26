---
title: responsive-form-factor
type: concept
source_files:
  - android/frontend/src/App.tsx
  - android/frontend/src/components/tablet/TabletShell.tsx
  - android/frontend/src/components/mobile/MobileDashboard.tsx
  - android/frontend/src/components/mobile/MobileMore.tsx
  - android/frontend/src/components/mobile/MobileQuickActions.tsx
  - android/frontend/src/components/layout/BottomNav.tsx
  - android/frontend/src/components/layout/DockNav.tsx
  - android/frontend/src/hooks/useFormFactor.ts
tags:
  - responsive
  - layout
  - mobile
  - tablet
  - tauri
---

## Overview

`responsive-form-factor` 是 Android 前端（Tauri + React）的核心自适应架构概念：通过 `useFormFactor` Hook 在运行时依据短边尺寸（sw600dp 语义）判定设备为 `phone` 或 `tablet`，在应用入口 `App.tsx` 动态分发至两套同源但布局范式迥异的外壳——手机端 `AppInner`（顶部轻 header + 底部导航 `BottomNav` + 单列卡流）与平板端 `TabletShell`（桌面 Dock 布局 `TitleBar`/`StatusBar`/`DockNav`/`RightPanel` 同源复用、面板用安卓版）。两套外壳复用同一批业务面板组件（如 `MobileDashboard`、`AccountPanel`、`MonitorPanel`、`QualityPanel`、`SpeedTestPanel`、`SettingsPanel`、`LogPanel`），仅通过容器差异、导航结构、触控缩放系数（仅平板顶栏 zoom 1.3）及动画策略适配不同屏幕，共享 Zustand 状态 Stores，确保逻辑一致性。平板外壳 `TabletShell` 通过 `lazy` 动态导入，手机端永不求值该模块树（见 App.tsx 行34-41、379-381）。

## Key Components

- **android/frontend/src/hooks/useFormFactor.ts**
  - `FormFactor` 类型（行9）：`'phone' | 'tablet'`，设备形态标识。
  - `Orientation` 类型（行10）：`'portrait' | 'landscape'`，屏幕方向。
  - `TABLET_MIN_EDGE_PX` 常量（行13）：`600`，平板短边 CSS px 阈值。
  - `detectFormFactor()`（行15-17）：取 `Math.min(window.innerWidth, window.innerHeight) >= 600` 返回 `FormFactor`。
  - `detectOrientation()`（行19-21）：宽≥高返回 `'landscape'`，否则 `'portrait'`。
  - `useViewportMeasure<T>(calc)`（行24-36）：订阅 `resize`/`orientationchange` 重算度量。
  - `useFormFactor()`（行38-40）：导出 Hook，返回 `FormFactor`。
  - `useOrientation()`（行43-45）：导出 Hook，返回 `Orientation`。

- **android/frontend/src/App.tsx**
  - `TabletShell` 懒加载常量（行39-41）：`lazy(() => import('@/components/tablet/TabletShell'))`，手机端不求值。
  - `AboutDialog`/`MobileQuickActions`/`ThemeDialog`/`OnboardingWizard` 懒加载（行54-57）。
  - `AppInner` 组件（行59-366）：手机外壳。包含：
    - `tab` state（行64-69）：`MobileTab` 类型，白名单 `['dashboard','account','selfservice','quality','monitor','more']` 持久化于 `safeStorage`（键 `campus-mobile-tab`）。
    - `deferredTab`（行70）：`useDeferredValue(tab)`。
    - `tabOrder`（行77-83）：依 `configEnableNetworkQuality` 决定第4位为 `quality` 或 `monitor`。
    - `navDir` state（行84）与 `tabRef`（行85-86）。
    - `handleTabChange(next)`（行126-134）：计算方向 `navDir`、更新 `tabRef`、写存储、调用 `markInteraction()`。
    - 配置就绪归位效果（行138-143）：质量开启后 `monitor`‑tab 归位 `quality`。
    - 首启向导效果（行145-153）：无账号且未标记 `campus-onboarding-done` 时开 `OnboardingWizard`。
    - `panelContent` switch（行155-199）：依 `deferredTab` 渲染对应面板（dashboard→`MobileDashboard`，account→`AccountPanel`，selfservice→`SelfServicePanel`，quality→`QualityPanel`，monitor→`MonitorPanel`，more→`MobileMore`）。
    - 顶部轻 header（行209-267）：绝对定位、状态点、运营商 chip（行222-230）、更新胶囊（行232-242）、质量胶囊（行244-253，质量关时隐藏）、主题/赞助/设置/关于图标按钮。
    - `MobileQuickActions` 挂载（行270-274，仅 dashboard）。
    - 主滚动区与 `AnimatePresence` 方向转场（行277-304）。
    - `BottomNav`（行306）、弹窗（`SponsorCard`/`AboutDialog`/`UpdateAvailableDialog`/`ThemeDialog`/`ConfirmDialog`/`OnboardingWizard`）及 `ToastContainer`（行308-363）。
  - `App` 默认导出（行374-400）：`useFormFactor()` 分支渲染 `TabletShell` 或 `AppInner`，平板预取效果（行379-381），共用 `ErrorBoundary`+`AnimationActiveProvider`+`FaceCaptureDialog`+`NotificationPermissionGate`。
  - `NotificationPermissionGate` 组件（行406-430）：首启请求通知权限，标记 `campus-notification-prompt`。

- **android/frontend/src/components/tablet/TabletShell.tsx**
  - `PANEL_TITLES`（行54-63）：面板标题/描述 i18n key 映射（不含 network）。
  - `PANEL_CONTAINER_STYLE`（行66）：`contain: 'layout style'`。
  - `TOPBAR_ZOOM` 常量（行73）=1.3：平板触屏顶栏缩放系数；`CONTENT_ZOOM` 已移除（行70-72 注释明确内容区不再缩放）。
  - `PanelSkeleton` 组件（行77-87）：面板加载骨架。
  - `TabletShellInner` 组件（行89-408）：
    - `activePanel` 来自 `useAdapterStore`（行94-95），`deferredPanel` 驱动面板。
    - `isLandscape` 来自 `useOrientation()`（行140），`shellStyle` 竖屏置 `--right-panel-width:0px`（行141-144）。
    - `panelVariants` 本地定义（行146）。
    - `handlePanelChange`（行148-156）：带锁（60ms）切换面板并持久化 `campus-active-panel`。
    - 首启向导效果（行161-166）。
    - `panelContent` switch（行174-237）：dashboard→`MobileDashboard`，account→`AccountPanel`，selfservice→`SelfServicePanel`，monitor→`MonitorPanel`，quality→`QualityPanel`，speedtest→`SpeedTestPanel`，settings→`SettingsPanel`，log→`LogPanel`。
    - JSX（行239-407）：`TitleBar`/`StatusBar` 包 zoom（行247,264），`main` 区（行275-321）含标题与面板转场，`RightPanel` 仅 `isLandscape` 渲染（行323-328），侧边看板娘（行332-350），`DockNav`（行352-354），弹窗与 `ToastContainer`。
  - `TabletShell` 导出（行410-416）：`ErrorBoundary` 包裹 `TabletShellInner`。

- **android/frontend/src/components/mobile/MobileDashboard.tsx**
  - `LampState` 类型（行22）：`'online'|'offline'|'loading'|'error'`。
  - `LAMP_COLOR` 常量（行24-29）：状态灯颜色映射。
  - `noopAsync`（行32）：异步空操作（排除 DHCP 续租）。
  - `EXCLUDED_CARDS`（行35）：`['quickActions']`。
  - `MobileStatusCard` 组件（行38-96）：手机状态卡，仅 `formFactor==='phone'` 显示运营商（行81-91）。
  - `MobileMonitorCard` 组件（行99-139）：监控摘要卡，含 Radix `Switch`。
  - `MobileDashboard` 组件（行141-168）：注入 `extraCards` 至 `DashboardPanel`，排除 `quickActions`，`mergeSelfServiceEmpty`。

- **android/frontend/src/components/mobile/MobileMore.tsx**
  - `MoreTab` 类型（行19）：`'monitor'|'speedtest'|'log'|'settings'`。
  - `TABS` 常量（行21-26）：chips 定义，`monitor` 标记 `qualityOnly: true`。
  - `MobileMoreProps` 接口（行28-31）：`onShowOnboarding?: () => void`。
  - `MobileMore` 组件（行33-83）：聚合页，依 `qualityEnabled`（行36）过滤 tabs（行37），`sub` 状态降级 monitor→settings（行38-40），渲染对应子面板。

- **android/frontend/src/components/mobile/MobileQuickActions.tsx**
  - `MobileQuickActions` 组件（行14-78）：首页悬浮登录/注销双胶囊（绝对定位底栏上方），先 `api.bindToWifi()` 再 `doLogin()`（行24-32），视觉等重磨砂/实心。

- **android/frontend/src/components/layout/BottomNav.tsx**
  - `MobileTab` 类型（行18）：`'dashboard'|'account'|'selfservice'|'quality'|'monitor'|'more'`。
  - `TABS` 常量（行20-25）：基础底栏项（dashboard/account/selfservice/more）。
  - `QUALITY_TAB`/`MONITOR_TAB`（行27-28）：动态第4位定义。
  - `useNavTabs()`（行30-35）：依 `qualityEnabled` 插入对应项到索引3。
  - `BottomNav` 组件（行37-152）：磨砂玻璃胶囊栏（`glass-nav`），集成 `useGlassRefraction`（行46）与 `useDockLens`（行60-65）镜头放大，渲染页签按钮与放大层。

- **android/frontend/src/components/layout/DockNav.tsx**
  - `ICON_MAP` 常量（行30-39）：面板 id 至图标映射。
  - `DockItem` 组件（行41-85）：Dock 项，激活显标签，hover tooltip，支持 `ghost` 放大层副本。
  - `ActionButton` 组件（行88-169）：登录/注销按钮，GSAP 旋转（行114-122）与 `usePulseAnimation` 加载脉冲，物理按钮样式。
  - `DockNavProps` 接口（行171-174）：`onPanelChange: (panel: PanelName) => void`、`outerRef?`。
  - `DockNav` 导出 memo 组件（行176-295）：`visibleItems` 过滤 `NAV_ITEMS`（行185，质量关时去 quality），集成玻璃折射（行191）与镜头（行204-209），渲染 `DockItem` 列表、分隔线、两个 `ActionButton`。

## Architecture

```text
┌───────────────────────┐     ┌──────────────────────┐
│ useFormFactor.ts      │     │ App.tsx (App)        │
│ useFormFactor()       │────►│ formFactor === tablet│
└───────────────────────┘     └──────────────────────┘
                                  │
                    ┌─────────────┴─────────────┐
                    ▼                           ▼
┌───────────────────────┐     ┌──────────────────────┐
│ AppInner (phone)      │     │ TabletShell (tablet)  │
│ BottomNav + Main      │     │ TitleBar/StatusBar    │
│ MobileDashboard etc.  │     │ DockNav + RightPanel  │
│ MobileQuickActions    │     │ MobileDashboard etc.  │
└───────────────────────┘     └──────────────────────┘
        │                                   │
        └─────── shared Zustand stores ─────┘
   (useConfigStore/useAuthStore/useAdapterStore/useQualityStore...)
```

## Data Flow

1. **形态检测**：`App.tsx` 行375 调用 `useFormFactor()`（useFormFactor.ts 行38），返回 `phone`/`tablet`。
2. **外壳分支与预取**：行376-381 若 `isTablet` 则预取 `TabletShell` 模块；行385-393 三元渲染 `TabletShell` 或 `AppInner`。
3. **手机导航流**：`BottomNav` 点击（BottomNav.tsx 行107）→ `handleTabChange`（App.tsx 行126）设 `tab` state → `deferredTab` 驱动 `panelContent` switch（行156-199）与 `AnimatePresence` 方向转场（行288-302）。
4. **平板导航流**：`DockNav` 点击（DockNav.tsx 行247 `onPanelChange`）→ `handlePanelChange`（TabletShell.tsx 行148）设 `useAdapterStore.activePanel` → `deferredPanel` 驱动面板 switch（行174-237）与淡入淡出（行304-319）。
5. **配置依赖**：`useConfigStore.config.enableNetworkQuality` 控制手机底栏第4位（App.tsx 行77-83）、平板 `NAV_ITEMS` 过滤（DockNav.tsx 行185），并在配置晚到时降级错误页签（App.tsx 行138-143 手机 monitor→quality；MobileMore.tsx 行38-40 平板派生降级）。
6. **横竖屏适配**：平板 `useOrientation()`（TabletShell.tsx 行140）切换右栏显隐（行323-328）与看板娘偏移（行341-349），并调整 `--right-panel-width` CSS 变量（行141-144）。

## Connections

相关模块：[[android-frontend-app-shell]] [[android-frontend-hooks-and-stores]] [[android-frontend-auth]] [[android-frontend-settings]] [[android-frontend-entry-config]]

## Known Issues

- **KI#14 白名单遗漏**：手机 `tab` 初始白名单必须覆盖 `handleTabChange` 可写全部页签（含 `monitor`），否则后台杀进程重开回落 `dashboard`（App.tsx 行64-69）。质量开启后跨会话恢复的 `monitor` 需在配置就绪后归位 `quality`（行138-143）。
- **平板竖屏右栏隐藏**：`isLandscape` 为假时 `--right-panel-width` 置 0，右栏 `RightPanel` 不渲染（TabletShell.tsx 行140-144, 323-328），日志仅能经 Dock 的 "log" 面板访问。
- **CSS zoom 副作用**：平板通过 `TOPBAR_ZOOM=1.3` 触屏适配（TabletShell.tsx 行73, 应用于 TitleBar/StatusBar 行247,264），仅 WebView 支持；`CONTENT_ZOOM` 已移除（行70-72 注释），内容区与桌面 1:1 不再缩放。
- **质量默认关闭**：手机顶栏质量胶囊隐藏（App.tsx 行244-253），底栏第4位由 `monitor` 补位（行77-83）；`MobileMore` 中 `monitor` 子页因 `qualityOnly` 被过滤（MobileMore.tsx 行21-26,37）。
- **首启引导时序**：无账号时手机（App.tsx 行145-153）与平板（TabletShell.tsx 行161-166）均弹 `OnboardingWizard`，标记由向导写 `campus-onboarding-done`，未完继续引导；新手向导为 Portal 渲染不受外壳布局裁剪。
