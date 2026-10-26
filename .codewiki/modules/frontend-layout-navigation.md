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

`frontend-layout-navigation` 模块负责应用在不同设备形态（手机、平板）下的核心布局与导航结构。移动端使用底部磨砂胶囊导航栏 `BottomNav` 与悬浮于其上的快捷操作 `MobileQuickActions`（目的地与动作分离，M3 规范）；平板/桌面形态复用 `DockNav`（镜头弹簧系统导航栏）、`TitleBar` 系统标题栏与 `RightPanel` 运行时日志面板，并由 `TabletShell` 外壳协调，处理横竖屏布局调整（如竖屏隐藏右侧日志栏）。移动端页面内容由 `MobileDashboard`（注入移动专属卡）与 `MobileMore`（聚合子页）承载。

## Key Components

### BottomNav.tsx (Mobile Bottom Navigation)
**文件**: `android/frontend/src/components/layout/BottomNav.tsx`

*   **Type `MobileTab`** (Line 18): 页签联合类型 `'dashboard' | 'account' | 'selfservice' | 'quality' | 'monitor' | 'more'`。
*   **Constant `TABS`** (Lines 20-25): 基础页签数组，含 `dashboard`, `account`, `selfservice`, `more` 及对应 Lucide 图标。
*   **Constant `QUALITY_TAB`** (Line 27): 质量检测页签配置（`Signal` 图标）。
*   **Constant `MONITOR_TAB`** (Line 28): 监控页签配置（`Activity` 图标）。
*   **Function `useNavTabs`** (Lines 30-35): Hook，读取 `useConfigStore` 中 `enableNetworkQuality !== false`（Line 31），在基础数组索引 3 处插入 `QUALITY_TAB` 或 `MONITOR_TAB`。
*   **Component `BottomNav`** (Lines 37-151): Props 接口 Lines 37-40（`tab: MobileTab`, `onChange`）。内部使用 `useGlassRefraction`（Line 46）挂折射玻璃，使用 `useDockLens`（Lines 60-65）实现镜头弹簧系统（启用条件 Line 51：`glassLevel !== 'off' && !reducedMotion`）。渲染绝对定位 `nav`（Lines 67-71，避让 `safe-area-inset-bottom`），玻璃容器 `.glass-nav`（Line 78），镜头底 `data-dock-lens`（Lines 86-94），页签条 `data-dock-strip`（Line 99）按钮等宽 `w-16 h-14` 图标 26px（Lines 112-123），放大层 `data-dock-magnify`（Lines 130-148）。

### DockNav.tsx (Tablet/Desktop Dock)
**文件**: `android/frontend/src/components/layout/DockNav.tsx`

*   **Constant `ICON_MAP`** (Lines 30-39): 字符串图标名到 Lucide 组件的映射。
*   **Component `DockItem`** (Lines 41-85): Props 接口 Lines 41-49（`id`, `label`, `icon`, `isActive`, `ghost?`, `onPanelChange?`）。渲染按钮，激活时显示文字标签（Lines 72-74），hover/focus Tooltip（Lines 75-82），`data-dock-item` 供镜头挖洞（Line 54）。
*   **Component `ActionButton`** (Lines 88-169): Props 接口 Lines 96-104。登录/注销按钮，GSAP 旋转 spinner（Lines 114-122），loadingPulse 边框（Lines 150-156），`m.button` 点击缩放（Line 134），主色/描边变体（Lines 140-147）。
*   **Interface `DockNavProps`** (Lines 171-174): `onPanelChange`, `outerRef?`。
*   **Component `DockNav`** (Lines 176-295): memo 组件。Line 185 过滤 `NAV_ITEMS` 排除 quality（当 `enableNetworkQuality` 关闭）。Lines 189-191 玻璃折射（economy 档强制 off）。Lines 194-209 镜头系统 `useDockLens`（启用条件 Line 195：`glassLevel !== 'off' && profile.tier !== 'economy' && !reducedMotion`）。JSX：外层 fixed 定位宽度 `calc(100vw - var(--right-panel-width, 288px))`（Line 215），`nav.glass-dock`（Line 219），镜头底（Lines 227-235），项容器（Line 239），放大层（Lines 254-269），分隔线（Line 271），两个 `ActionButton`（Lines 273-291）。

### RightPanel.tsx (Runtime Log Panel)
**文件**: `android/frontend/src/components/layout/RightPanel.tsx`

*   **Interface `RightPanelProps`** (Lines 14-18): `logs`, `onClearLogs?`, `outerRef?`。
*   **Constants `LOG_ICONS`/`LOG_COLORS`/`LOG_BG_COLORS`/`LOG_BAR_COLORS`** (Lines 20-46): 日志类型到图标与颜色的映射。
*   **Constants `RIGHT_PANEL_ANIM_THRESHOLD`** (Line 51, 值 50), **`RIGHT_PANEL_ANIM_KEEP_COUNT`** (Line 52, 值 30): 性能分片阈值。
*   **Component `RightPanel`** (Lines 54-279): memo 组件。
    *   `handleClearWithAnimation` (Lines 64-125): GSAP stagger 隐藏条目，动态间隔（Line 75），`failsafe` setTimeout 防 rAF 挂起（Lines 112-117），`onInterrupt` 复位（Lines 99-105）。
    *   `handleScroll` (Lines 127-131): 自动滚动检测（距底 <40px 视为自动）。
    *   Auto-scroll effect (Lines 134-138)。
    *   New log detection (Lines 140-144)。
    *   Log splitting `useMemo` (Lines 148-157): `logs.length > 50` 时前段 `staticLogs` 普通 div，最近 30 条 `animLogs` 用 `m.div`+`AnimatePresence`。
    *   JSX (Lines 159-278): 标题栏计数与清除按钮（Lines 166-187），看板娘背景（Lines 190-197），日志列表渲染（Lines 216-273）。

### TitleBar.tsx (Window Title Bar)
**文件**: `android/frontend/src/components/layout/TitleBar.tsx`

*   **Interface `TitleBarProps`** (Lines 11-24): 含 `showWindowControls?` 默认 true。
*   **Inline SVG Icons** (Lines 26-50): `MinimizeIcon`, `MaximizeIcon`, `RestoreIcon`, `CloseIcon`。
*   **Component `TitleBar`** (Lines 52-278): memo 组件。
    *   `handleTitleBarMouseDown` (Lines 73-85): 拖动窗口（调用 `getCurrentWindow().startDragging()`），排除按钮与 `showWindowControls=false` 情况。
    *   `handleTitleBarDoubleClick` (Lines 87-90): 最大化切换（仅 `showWindowControls`）。
    *   JSX: 左侧 Logo/版本/更新提示（Lines 100-142），右侧工具栏（光暗 Line 145-157、语言 159-172、通知 174-190、主题 192-204、赞助 206-217、关于 219-230），窗口控制按钮仅当 `showWindowControls` 渲染（Lines 232-273）。

### MobileDashboard.tsx (Mobile Dashboard Page)
**文件**: `android/frontend/src/components/mobile/MobileDashboard.tsx`

*   **Type `LampState`** (Line 22): `'online' | 'offline' | 'loading' | 'error'`。
*   **Constant `LAMP_COLOR`** (Lines 24-29): 状态灯颜色类。
*   **Constant `noopAsync`** (Line 32): 异步空函数，供排除卡片 handler。
*   **Constant `EXCLUDED_CARDS`** (Line 35): `['quickActions']`，移动端排除 DHCP 续租（Windows 专属）。
*   **Component `MobileStatusCard`** (Lines 38-96): 呼吸灯状态卡，仅 `formFactor === 'phone'` 显示运营商徽标（Lines 81-91）。
*   **Component `MobileMonitorCard`** (Lines 99-139): 监控摘要卡，Radix Switch 开关（Line 120）与刷新按钮。
*   **Component `MobileDashboard`** (Lines 141-168): 注入 `extraCards`（`mobileHero`, `mobileMonitor`，Lines 149-152）到 `DashboardPanel`，传 `excludeCards={EXCLUDED_CARDS}`（Line 163）。

### MobileMore.tsx (Mobile "More" Page)
**文件**: `android/frontend/src/components/mobile/MobileMore.tsx`

*   **Type `MoreTab`** (Line 19): `'monitor' | 'speedtest' | 'log' | 'settings'`。
*   **Constant `TABS`** (Lines 21-26): 子页签，其中 `monitor` 标 `qualityOnly: true`。
*   **Interface `MobileMoreProps`** (Lines 28-31): `onShowOnboarding?`。
*   **Component `MobileMore`** (Lines 33-84): 根据 `qualityEnabled`（Line 36）过滤 tabs（Line 37）；若 `subRaw === 'monitor'` 且质量关闭则降级到 `'settings'`（Line 40）；chips 切换渲染 `MonitorPanel`/`SpeedTestPanel`/`LogPanel`/`SettingsPanel`（Lines 66-81）。

### MobileQuickActions.tsx (Mobile Quick Actions)
**文件**: `android/frontend/src/components/mobile/MobileQuickActions.tsx`

*   **Component `MobileQuickActions`** (Lines 14-78):
    *   `handleQuickLogin` (Lines 24-32): 先 `api.bindToWifi()` 再 `doLogin()`。
    *   绝对定位悬浮于底栏上方（Lines 35-39，bottom `safe-area + 100px`）。
    *   注销按钮 `btn-tonal-glass` 磨砂（Lines 41-57），登录按钮实心主色（Lines 58-74），视觉等重阴影。

### TabletShell.tsx (Tablet Shell Component)
**文件**: `android/frontend/src/components/tablet/TabletShell.tsx`

*   **Constant `PANEL_TITLES`** (Lines 54-63): 面板标题/描述 key 映射（无 `network`）。
*   **Constant `PANEL_CONTAINER_STYLE`** (Line 66): `{ contain: 'layout style' }`。
*   **Constant `TOPBAR_ZOOM`** (Line 73, 值 1.3): 顶部栏缩放提升触屏点性；`CONTENT_ZOOM` 已移除（Lines 70-72 注释说明）。
*   **Function `PanelSkeleton`** (Lines 77-87): 懒加载骨架。
*   **Component `TabletShellInner`** (Lines 89-408):
    *   Store hooks (Lines 94-124)，`useOrientation` 横竖屏（Line 140），竖屏设 `--right-panel-width: 0px` 隐藏右栏（Lines 141-144）。
    *   `handlePanelChange` (Lines 148-156) 带锁防止动画冲突。
    *   首次无账号弹 Onboarding（Lines 161-166）。
    *   `handleClearLogs` (Lines 168-170)。
    *   Switch `deferredPanel` 渲染面板（Lines 174-237）：dashboard→MobileDashboard，account→AccountPanel，等。
    *   JSX: TitleBar/StatusBar 包 zoom（Lines 246-269），主区 AnimatePresence（Lines 275-321），横屏 RightPanel（Lines 323-328），侧边看板娘断点（Lines 332-350），DockNav（Lines 352-354），全局对话框（Lines 356-405）。
*   **Component `TabletShell`** (Lines 410-416): ErrorBoundary 包装。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│ TabletShell (TabletShell.tsx)                                │
│ ┌────────────┐ ┌────────────┐ ┌────────────────────────────┐ │
│ │ TitleBar   │ │ StatusBar  │ │ Main Content (zoom 1.0)    │ │
│ │ zoom 1.3   │ │ zoom 1.3   │ │ AnimatePresence m.div      │ │
│ └────────────┘ └────────────┘ │ - MobileDashboard etc.     │ │
│                               └────────────────────────────┘ │
│ ┌──────────────────────┐  ┌───────────────────────────────┐ │
│ │ RightPanel (landscape│  │ DockNav (DockNav.tsx)          │ │
│ │ only, logs split)    │  │ glass-dock + useDockLens       │ │
│ └──────────────────────┘  │ ActionButton login/logout     │ │
│                           └───────────────────────────────┘ │
│ Phone shell uses BottomNav + MobileQuickActions (separate)  │
└──────────────────────────────────────────────────────────────┘
   DockNav onPanelChange ► TabletShell.handlePanelChange ► activePanel
```

## Data Flow

1. **State Initialization**: `TabletShellInner` 通过 Zustand stores（`useAdapterStore`, `useConfigStore`, `useAuthStore`, `useLogToastStore` 等，Lines 94-124）获取 `activePanel`, `logs`, `accounts`, `isLoggingIn`。
2. **Navigation Trigger**:
   * 用户点击 `DockNav` 项 → `onLensSelect` (DockNav Lines 197-203) → `onPanelChange(panelName)` prop。
   * `TabletShell.handlePanelChange` (Lines 148-156) 更新 `useAdapterStore.activePanel` 并写 `safeStorage`。
   * `useDeferredValue(activePanel)` (Line 95) 生成 `deferredPanel` 避免阻塞。
   * 手机端 `BottomNav` 类似通过 `onChange` 回调（BottomNav Lines 37-40, 107）。
3. **Content Rendering**: `TabletShell` switch `deferredPanel` (Lines 174-237) 渲染对应面板；`MobileDashboard` 注入 `extraCards` 至 `DashboardPanel` (MobileDashboard Lines 154-167)。
4. **Log Updates**: 后端事件更新 `useLogToastStore.logs` → `RightPanel` 收 `logs` props → 超过阈值分片 (RightPanel Lines 148-157) → 自动滚动 (Lines 134-138)。
5. **Authentication**: `MobileQuickActions` 或 `DockNav ActionButton` 触发 `doLogin`/`doLogout`；`MobileQuickActions.handleQuickLogin` 先绑 WLAN (Lines 24-32)；认证状态反映按钮 loading。

## Connections

* [[android/frontend/src/hooks/useAdapterStore]]: 管理 `activePanel` 导航核心状态（DockNav Line 178, TabletShell Line 94）。
* [[android/frontend/src/hooks/useConfigStore]]: 提供 `enableNetworkQuality`, `accounts`, `language` 等（BottomNav Line 31, DockNav Line 181）。
* [[android/frontend/src/hooks/useAuthStore]]: 处理登录/注销状态与动作（DockNav Lines 179-183, MobileQuickActions Lines 17-20）。
* [[android/frontend/src/shared/ui-constants]]: 导出 `NAV_ITEMS` 供 DockNav 过滤（DockNav Line 15, 185）。
* [[android/frontend/src/auth/DashboardPanel]]: 被 MobileDashboard 复用注入卡片（MobileDashboard Line 17, 155）。
* [[android/frontend/src/monitor/MonitorPanel]]: 用于 TabletShell 与 MobileMore（TabletShell Line 41, MobileMore Line 8）。
* [[android/frontend/src/settings/SettingsPanel]]: 用于 TabletShell 与 MobileMore（TabletShell Line 44, MobileMore Line 10）。
* [[android/frontend/src/shared/LogPanel]]: 用于 MobileMore 与 TabletShell（MobileMore Line 11, TabletShell Line 29）。
* [[android/frontend/src/monitor/StatusBar]]: 平板外壳引入（TabletShell Line 35, 265）。
* [[android/frontend/src/lib/dock-lens]]: `useDockLens` 镜头系统被 BottomNav/DockNav 共用（BottomNav Line 15, DockNav Line 28）。
* [[android/frontend/src/lib/glass-dock]]: `useGlassRefraction` 玻璃折射同源（BottomNav Line 14, DockNav Line 27）。

## Known Issues

* **RightPanel 清空动画在 WebView 后台卡死**: GSAP 回调依赖 rAF ticker，安卓 WebView 后台/遮挡时 rAF 停摆，tween 冻结导致 `onComplete`/`onInterrupt` 不触发，`isClearing` 永久 true（RightPanel.tsx Lines 108-111）。已用 `setTimeout` failsafe 兜底（Lines 112-117），且 `onInterrupt` 复位避免永久禁用（Lines 99-105）。
* **BottomNav 玻璃样式优先级冲突**: 注释明确禁止在 JSX 写 inline 背景样式覆盖 `.glass-nav` CSS，否则切档时死代码（BottomNav.tsx Lines 73-75）。
* **DockNav 按钮自缩放与镜片冲突**: 已移除 hover 上浮放大，避免 scale 撑高 nav 导致镜片尺寸抖动（DockNav.tsx Lines 131-133）。
* **TabletShell 内容缩放移除**: 原 `CONTENT_ZOOM=0.9` 让平板观感憋屈，已移除，与桌面 1:1（TabletShell.tsx Lines 70-72）。
* **MobileDashboard 运营商徽标重复**: 仅 `formFactor === 'phone'` 渲染运营商徽标，避免与平板 StatusBar 重复（MobileDashboard.tsx Lines 49-50, 81-91）。
* **MobileMore 质量关闭派生降级**: `config` 晚到导致 `qualityEnabled` 翻转时，已存的 `monitor` sub 需降级到 `settings` 保持 chips 一致（MobileMore.tsx Lines 39-40）。
* **动画降级条件**: DockNav 镜头系统在非玻璃档或 economy 档或 reducedMotion 时禁用（DockNav.tsx Line 195）；BottomNav 同理（BottomNav.tsx Line 51）。
