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

`responsive-form-factor` 是 Android 前端（Tauri + React）的核心自适应架构概念：通过 `useFormFactor` Hook 在运行时依据短边尺寸（sw600dp 语义）判定设备为 `phone` 或 `tablet`，在应用入口 `App.tsx` 动态分发至两套同源但布局范式迥异的外壳——手机端 `AppInner`（底部导航 + 单列卡流）与平板端 `TabletShell`（桌面 Dock 布局 + 多栏面板）。两套外壳复用同一批业务面板组件（如 `MobileDashboard`、`AccountPanel`、`MonitorPanel`），仅通过容器差异、导航结构、触控缩放系数及动画策略适配不同屏幕，共享 Zustand 状态 Stores，确保逻辑一致性与空间利用率最大化。

## Key Components

- **android/frontend/src/hooks/useFormFactor.ts**
  - `FormFactor` 类型（行9）：`'phone' | 'tablet'`，设备形态标识。
  - `Orientation` 类型（行10）：`'portrait' | 'landscape'`，屏幕方向。
  - `TABLET_MIN_EDGE_PX` 常量（行13）：`600`，平板短边 CSS px 阈值。
  - `detectFormFactor()`（行15-17）：取 `Math.min(innerWidth, innerHeight) >= 600` 返回 `FormFactor`。
  - `detectOrientation()`（行19-21）：宽≥高返回 `'landscape'`。
  - `useViewportMeasure<T>(calc)`（行24-36）：订阅 `resize`/`orientationchange` 重算度量。
  - `useFormFactor()`（行38-40）：导出 Hook，返回 `FormFactor`。
  - `useOrientation()`（行43-45）：导出 Hook，返回 `Orientation`。

- **android/frontend/src/App.tsx**
  - `AppInner` 组件（行52-359）：手机外壳，含绝对定位轻 header（行202-260）、`main` 滚动区（行270-297）、`BottomNav`（行299）、各类弹窗及 `MobileQuickActions` 挂载（行263-267）。
  - `tab` state（行57-62）：`MobileTab` 类型，白名单 `['dashboard','account','selfservice','quality','monitor','more']` 持久化于 `safeStorage`。
  - `tabOrder`（行70-76）：依 `configEnableNetworkQuality` 决定第4位为 `quality` 或 `monitor`。
  - `handleTabChange(next)`（行119-127）：计算方向 `navDir` 并写入存储。
  - `App` 默认导出（行363-375）：`useFormFactor()` 分支渲染 `TabletShell` 或 `AppInner`，共用 `FaceCaptureDialog` 与 `NotificationPermissionGate`。
  - `NotificationPermissionGate`（行381-405）：首启请求通知权限。

- **android/frontend/src/components/tablet/TabletShell.tsx**
  - `PANEL_TITLES`（行54-63）：面板标题/描述 i18n key 映射（不含 network）。
  - `PANEL_CONTAINER_STYLE`（行66）：`contain: 'layout style'`。
  - `TOPBAR_ZOOM`（行71）=1.3、`CONTENT_ZOOM`（行72）=0.9：平板触屏缩放系数。
  - `TabletShellInner`（行74-380）：平板外壳，使用 `useAdapterStore` 的 `activePanel`（行79）、`useOrientation` 控制右栏（行125-129），渲染 `TitleBar`/`StatusBar`（zoom 1.3）、`main`（zoom 0.9）、`RightPanel`（仅横屏）、`DockNav`、看板娘及弹窗。
  - `handlePanelChange`（行133-141）：带锁切换面板并持久化。
  - `TabletShell` 导出（行382-388）：`ErrorBoundary` 包裹 `TabletShellInner`。

- **android/frontend/src/components/mobile/MobileDashboard.tsx**
  - `LampState` 类型（行22）：`'online'|'offline'|'loading'|'error'`。
  - `LAMP_COLOR`（行24-29）：状态灯颜色映射。
  - `noopAsync`（行32）：异步空操作（排除 DHCP 续租）。
  - `EXCLUDED_CARDS`（行35）：`['quickActions']`。
  - `MobileStatusCard`（行38-96）：手机状态卡，仅 `formFactor==='phone'` 显示运营商（行81）。
  - `MobileMonitorCard`（行99-139）：监控摘要卡，含 Radix Switch。
  - `MobileDashboard`（行141-168）：注入 `extraCards` 至 `DashboardPanel`，排除 `quickActions`。

- **android/frontend/src/components/mobile/MobileMore.tsx**
  - `MoreTab` 类型（行19）：`'monitor'|'speedtest'|'log'|'settings'`。
  - `TABS`（行21-26）：chips 定义，`monitor` 标记 `qualityOnly`。
  - `MobileMore`（行33-83）：聚合页，依 `qualityEnabled` 过滤 tabs（行37），降级 `monitor` 子页（行40）。

- **android/frontend/src/components/mobile/MobileQuickActions.tsx**
  - `MobileQuickActions`（行14-78）：首页悬浮登录/注销双胶囊，先绑 WLAN 再 `doLogin`。

- **android/frontend/src/components/layout/BottomNav.tsx**
  - `MobileTab` 类型（行18）：`'dashboard'|'account'|'selfservice'|'quality'|'monitor'|'more'`。
  - `TABS`（行20-25）：基础底栏项（含 more）。
  - `QUALITY_TAB`/`MONITOR_TAB`（行27-28）：动态第4位。
  - `useNavTabs()`（行30-35）：依 `qualityEnabled` 插入对应项。
  - `BottomNav`（行37-152）：磨砂玻璃胶囊栏，集成 `useGlassRefraction` 与 `useDockLens` 镜头放大。

- **android/frontend/src/components/layout/DockNav.tsx**
  - `ICON_MAP`（行30-39）：面板 id 至图标映射。
  - `DockItem`（行41-85）：Dock 项，激活显标签，hover tooltip。
  - `ActionButton`（行88-169）：登录/注销按钮，GSAP 旋转与脉冲。
  - `DockNav`（行176-295）：memo 组件，过滤 `NAV_ITEMS`（行185），集成玻璃与镜头，渲染登录/注销。

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
```

## Data Flow

1. **形态检测**：`App.tsx` 行364 调用 `useFormFactor()`（useFormFactor.ts 行38），返回 `phone`/`tablet`。
2. **外壳分支**：行368 三元渲染 `TabletShell` 或 `AppInner`。
3. **手机导航流**：`BottomNav` 点击 → `handleTabChange`（App.tsx 行119）设 `tab` state → `deferredTab` 驱动 `panelContent` switch（行149-192）与 `AnimatePresence` 方向转场（行281）。
4. **平板导航流**：`DockNav` 点击 → `handlePanelChange`（TabletShell.tsx 行133）设 `useAdapterStore.activePanel` → `deferredPanel` 驱动面板 switch（行160-222）与淡入淡出。
5. **配置依赖**：`useConfigStore.config.enableNetworkQuality` 控制手机底栏第4位（App.tsx 行74）与平板 `NAV_ITEMS` 过滤（DockNav.tsx 行185），并在配置晚到时降级错误页签（App.tsx 行131-136，MobileMore.tsx 行40）。
6. **横竖屏适配**：平板 `useOrientation()`（TabletShell.tsx 行125）切换右栏显隐与看板娘偏移。

## Connections

相关模块：[[android-frontend-app-shell]] [[android-frontend-hooks-and-stores]] [[android-frontend-auth]] [[android-frontend-settings]] [[android-frontend-entry-config]]

## Known Issues

- **KI#14 白名单遗漏**：手机 `tab` 初始白名单必须覆盖 `handleTabChange` 可写全部页签（含 `monitor`），否则后台杀进程重开回落 `dashboard`（App.tsx 行59-61）。质量开启后跨会话恢复的 `monitor` 需在配置就绪后归位 `quality`（行131-136）。
- **平板竖屏右栏隐藏**：`isLandscape` 为假时 `--right-panel-width` 置 0，右栏不渲染（TabletShell.tsx 行125-129, 295-300），日志仅能经 Dock 面板访问。
- **CSS zoom 副作用**：平板通过 `TOPBAR_ZOOM=1.3`/`CONTENT_ZOOM=0.9` 触屏适配（行71-72, 232, 249, 256），仅 WebView 支持，不影响手机外壳。
- **质量默认关闭**：手机顶栏质量胶囊隐藏（App.tsx 行237），底栏第4位由 `monitor` 补位（行74）；`MobileMore` 中 `monitor` 子页因 `qualityOnly` 被过滤（MobileMore.tsx 行25,37）。
- **首启引导时序**：无账号时手机（App.tsx 行138-146）与平板（TabletShell.tsx 行146-151）均弹 `OnboardingWizard`，标记由向导写 `campus-onboarding-done`，未完继续引导。
