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

`frontend-layout-navigation` 模块负责应用在不同设备形态（手机、平板）下的核心布局与导航结构。移动端使用底部磨砂胶囊导航栏 `BottomNav`（行 37-151）与悬浮于其上的快捷操作 `MobileQuickActions`（行 14-78，目的地与动作分离，M3 规范）；平板/桌面形态复用 `DockNav`（行 176-295，镜头弹簧系统导航栏）、`TitleBar` 系统标题栏（行 52-278）与 `RightPanel` 运行时日志面板（行 54-279），并由 `TabletShell` 外壳（`TabletShellInner` 行 74-380，`TabletShell` 行 382-388）协调，处理横竖屏布局调整（如竖屏通过 CSS 变量 `--right-panel-width: 0px` 隐藏右侧日志栏，行 126-129）。移动端页面内容由 `MobileDashboard`（行 141-168，注入移动专属卡）与 `MobileMore`（行 33-84，聚合子页）承载。

## Key Components

### BottomNav.tsx (Mobile Bottom Navigation)
**文件**: `android/frontend/src/components/layout/BottomNav.tsx`

*   **Type `MobileTab`** (行 18): 页签联合类型 `'dashboard' | 'account' | 'selfservice' | 'quality' | 'monitor' | 'more'`。
*   **Constant `TABS`** (行 20-25): 基础页签数组，含 `dashboard`, `account`, `selfservice`, `more` 及对应 Lucide 图标（LayoutDashboard, UserCircle, Globe, Ellipsis）。
*   **Constant `QUALITY_TAB`** (行 27): 质量检测页签配置（Signal 图标）。
*   **Constant `MONITOR_TAB`** (行 28): 监控页签配置（Activity 图标）。
*   **Function `useNavTabs`** (行 30-35): Hook，读取 `useConfigStore` 中 `enableNetworkQuality !== false`（行 31），在基础数组索引 3 处插入 `QUALITY_TAB` 或 `MONITOR_TAB`（行 33）。
*   **Component `BottomNav`** (行 37-151): Props 接口（行 37-40）含 `tab: MobileTab`, `onChange`。内部使用 `useGlassRefraction`（行 46）挂折射玻璃，使用 `useDockLens`（行 60-65）实现镜头弹簧系统（启用条件行 51：`glassLevel !== 'off' && !reducedMotion`）。渲染绝对定位 `nav`（行 67-71，避让 `safe-area-inset-bottom`），玻璃容器 `.glass-nav`（行 76-84，径向渐变通过 CSS 变量驱动，行 81-82），镜头底 `data-dock-lens`（行 86-94），页签条 `data-dock-strip`（行 99-126）按钮等宽 `w-16 h-14` 图标 26px（行 112-123），放大层 `data-dock-magnify` 与内层 `data-dock-magnify-inner`（行 130-148，渲染 `aria-hidden` 副本）。

### DockNav.tsx (Tablet/Desktop Dock)
**文件**: `android/frontend/src/components/layout/DockNav.tsx`

*   **Constant `ICON_MAP`** (行 30-39): 字符串图标名到 Lucide 组件的映射。
*   **Component `DockItem`** (行 41-85): Props 接口（行 41-49）含 `id: PanelName`, `label: string`, `icon: string`, `isActive: boolean`, `ghost?: boolean`, `onPanelChange?: (id: PanelName) => void`。渲染按钮，激活时显示文字标签（行 72-74），hover/focus Tooltip（行 75-82），`data-dock-item` 供镜头挖洞（行 54）。非激活仅显图标（行 70，大小 `h-[18px] w-[18px]`）。
*   **Component `ActionButton`** (行 88-169): Props 接口（行 96-104）含 `label`, `loadingLabel`, `icon`, `isLoading`, `isDisabled`, `onAction`, `variant: 'primary' | 'outline'`。登录/注销按钮，GSAP 旋转 spinner（行 114-122），loadingPulse 边框（行 150-156），`m.button` 点击缩放 `whileTap`（行 134），主色/描边变体（行 140-147，主色使用 `linear-gradient` 行 145）。
*   **Interface `DockNavProps`** (行 171-174): 字段 `onPanelChange: (panel: PanelName) => void`, `outerRef?: (el: HTMLDivElement | null) => void`。
*   **Component `DockNav`** (行 176-295): memo 组件。行 185 过滤 `NAV_ITEMS` 排除 quality（当 `enableNetworkQuality` 关闭）。行 189-191 玻璃折射（economy 档强制 off）。行 194-209 镜头系统 `useDockLens`（启用条件行 195：`glassLevel !== 'off' && profile.tier !== 'economy' && !reducedMotion`）。JSX：外层 fixed 定位宽度 `calc(100vw - var(--right-panel-width, 288px))`（行 215，避让 `safe-area-inset-bottom`），`nav.glass-dock`（行 217-225），镜头底（行 227-235），项容器（行 239-250），放大层（行 254-269），分隔线（行 271），两个 `ActionButton` 注销与登录（行 273-291）。

### RightPanel.tsx (Runtime Log Panel)
**文件**: `android/frontend/src/components/layout/RightPanel.tsx`

*   **Interface `RightPanelProps`** (行 14-18): 字段 `logs: LogEntry[]`, `onClearLogs?: () => void`, `outerRef?: (el: HTMLDivElement | null) => void`。
*   **Constants `LOG_ICONS`/`LOG_COLORS`/`LOG_BG_COLORS`/`LOG_BAR_COLORS`** (行 20-46): 日志类型（`info`, `success`, `error`, `warning`）到图标与颜色的映射。
*   **Constants `RIGHT_PANEL_ANIM_THRESHOLD`** (行 51, 值 50), **`RIGHT_PANEL_ANIM_KEEP_COUNT`** (行 52, 值 30): 性能分片阈值（FP-4 策略）。
*   **Component `RightPanel`** (行 54-279): memo 组件。
    *   `handleClearWithAnimation` (行 64-125): GSAP stagger 隐藏条目，动态间隔（行 75），`failsafe` setTimeout 防 rAF 挂起（行 112-117），`onInterrupt` 复位（行 99-105）。
    *   `handleScroll` (行 127-131): 自动滚动检测（距底 <40px 视为自动，行 130）。
    *   Auto-scroll effect (行 134-138): 依赖 `logs` 变化时若 `isAutoScrollRef` 为真则滚动到底。
    *   New log detection (行 140-144): `isNewLog` state 用于标记最新条目动画。
    *   Log splitting `useMemo` (行 148-157): `logs.length > 50` 时前段 `staticLogs` 普通 div，最近 30 条 `animLogs` 用 `m.div`+`AnimatePresence`。
    *   JSX (行 159-278): 宽度 `w-72` 侧边栏（行 162），标题栏计数与清除按钮（行 166-187），看板娘背景（行 190-197），日志列表渲染（行 216-273）。

### TitleBar.tsx (Window Title Bar)
**文件**: `android/frontend/src/components/layout/TitleBar.tsx`

*   **Interface `TitleBarProps`** (行 11-24): 字段含 `notificationEnabled: boolean`, `onToggleNotification: () => void`, `onShowTheme: () => void`, `onShowAbout: () => void`, `onShowSponsor: () => void`, `onToggleLightMode: () => void`, `onMinimize: () => void`, `onToggleMaximize: () => void`, `onClose: () => void`, `isMaximized: boolean`, `showWindowControls?: boolean`（默认 true，行 23）。
*   **Inline SVG Icons** (行 26-50): `MinimizeIcon`, `MaximizeIcon`, `RestoreIcon`, `CloseIcon`。
*   **Component `TitleBar`** (行 52-278): memo 组件。
    *   `handleTitleBarMouseDown` (行 73-85): 拖动窗口（调用 `getCurrentWindow().startDragging()`，行 84），排除按钮与 `showWindowControls=false` 情况（行 74, 82）。
    *   `handleTitleBarDoubleClick` (行 87-90): 最大化切换（仅 `showWindowControls` 为真时生效，行 88）。
    *   JSX: 左侧 Logo/版本/更新提示（行 100-142），右侧工具栏（光暗行 145-157、语言 159-172、通知 174-190、主题 192-204、赞助 206-217、关于 219-230），窗口控制按钮仅当 `showWindowControls` 渲染（行 232-273）。

### MobileDashboard.tsx (Mobile Dashboard Page)
**文件**: `android/frontend/src/components/mobile/MobileDashboard.tsx`

*   **Type `LampState`** (行 22): `'online' | 'offline' | 'loading' | 'error'`。
*   **Constant `LAMP_COLOR`** (行 24-29): 状态灯颜色类映射。
*   **Constant `noopAsync`** (行 32): 异步空函数，供排除卡片 handler 使用。
*   **Constant `EXCLUDED_CARDS`** (行 35): `['quickActions']`，移动端排除 DHCP 续租（Windows 专属）。
*   **Component `MobileStatusCard`** (行 38-96): 呼吸灯状态卡，仅 `formFactor === 'phone'` 显示运营商徽标（行 81-91），避免与平板 StatusBar 重复。
*   **Component `MobileMonitorCard`** (行 99-139): 监控摘要卡，Radix Switch 开关（行 120-124）与刷新按钮（行 128-135）。
*   **Component `MobileDashboard`** (行 141-168): 注入 `extraCards`（`mobileHero`, `mobileMonitor`，行 149-152）到 `DashboardPanel`，传 `excludeCards={EXCLUDED_CARDS}`（行 163），`onDhcpRenew` 等传 `noopAsync`（行 160-161），`mergeSelfServiceEmpty` 开启（行 165）。

### MobileMore.tsx (Mobile "More" Page)
**文件**: `android/frontend/src/components/mobile/MobileMore.tsx`

*   **Type `MoreTab`** (行 19): `'monitor' | 'speedtest' | 'log' | 'settings'`。
*   **Constant `TABS`** (行 21-26): 子页签，其中 `monitor` 标 `qualityOnly: true`（行 25）。
*   **Interface `MobileMoreProps`** (行 28-31): 字段 `onShowOnboarding?: () => void`。
*   **Component `MobileMore`** (行 33-84): 根据 `qualityEnabled`（行 36，读取 `enableNetworkQuality !== false`）过滤 tabs（行 37）；若 `subRaw === 'monitor'` 且质量关闭则降级到 `'settings'`（行 40）；chips 切换渲染 `MonitorPanel`/`SpeedTestPanel`/`LogPanel`/`SettingsPanel`（行 66-81）。

### MobileQuickActions.tsx (Mobile Quick Actions)
**文件**: `android/frontend/src/components/mobile/MobileQuickActions.tsx`

*   **Component `MobileQuickActions`** (行 14-78):
    *   `handleQuickLogin` (行 24-32): 先 `api.bindToWifi()` 再 `doLogin()`。
    *   绝对定位悬浮于底栏上方（行 35-39，bottom `safe-area + 100px`）。
    *   注销按钮 `btn-tonal-glass` 磨砂（行 41-57），登录按钮实心主色（行 58-74），视觉等重阴影（行 49, 66）。

### TabletShell.tsx (Tablet Shell Component)
**文件**: `android/frontend/src/components/tablet/TabletShell.tsx`

*   **Constant `PANEL_TITLES`** (行 54-63): 面板标题/描述 key 映射（无 `network`）。
*   **Constant `PANEL_CONTAINER_STYLE`** (行 66): `{ contain: 'layout style' }`。
*   **Constant `TOPBAR_ZOOM`** (行 71, 值 1.3): 顶部栏缩放提升触屏点性；**Constant `CONTENT_ZOOM`** (行 72, 值 0.9): 主区缩放平衡屏占比（与桌面不同，非 1:1）。
*   **Component `TabletShellInner`** (行 74-380):
    *   Store hooks (行 79-109)，`useOrientation` 横竖屏（行 125），竖屏设 `--right-panel-width: 0px` 隐藏右栏（行 126-129）。
    *   `handlePanelChange` (行 133-141) 带锁防止动画冲突，写入 `safeStorage`。
    *   首次无账号弹 Onboarding（行 146-151）。
    *   `handleClearLogs` (行 153-155)。
    *   Switch `deferredPanel` 渲染面板（行 160-222）：dashboard→MobileDashboard（行 161-163），account→AccountPanel（行 164-176），selfservice→SelfServicePanel（行 177-179），monitor→MonitorPanel（行 180-188），quality→QualityPanel（行 189-197），speedtest→SpeedTestPanel（行 198-204），settings→SettingsPanel（行 205-213），log→LogPanel（行 214-221）。
    *   JSX: TitleBar/StatusBar 包 zoom（行 231-254，TitleBar 的 `showWindowControls={false}` 行 234），主区 `<main>` 带 `pb-28` 类（行 257），AnimatePresence（行 276-291），横屏 RightPanel（行 295-300），侧边看板娘断点（行 304-322），DockNav（行 324-326），全局对话框（行 332-377）。
*   **Component `TabletShell`** (行 382-388): ErrorBoundary 包装。

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

1. **State Initialization**: `TabletShellInner` 通过 Zustand stores（`useAdapterStore`, `useConfigStore`, `useAuthStore`, `useLogToastStore` 等，行 79-109）获取 `activePanel`, `logs`, `accounts`, `isLoggingIn`。
2. **Navigation Trigger**:
   * 用户点击 `DockNav` 项 → `onLensSelect` (DockNav 行 197-203) → `onPanelChange(panelName)` prop。
   * `TabletShell.handlePanelChange` (行 133-141) 更新 `useAdapterStore.activePanel` 并写 `safeStorage`。
   * `useDeferredValue(activePanel)` (行 80) 生成 `deferredPanel` 避免阻塞。
   * 手机端 `BottomNav` 类似通过 `onChange` 回调（BottomNav 行 37-40, 107）。
3. **Content Rendering**: `TabletShell` switch `deferredPanel` (行 160-222) 渲染对应面板；`MobileDashboard` 注入 `extraCards` 至 `DashboardPanel` (MobileDashboard 行 154-167)。
4. **Log Updates**: 后端事件更新 `useLogToastStore.logs` → `RightPanel` 收 `logs` props → 超过阈值分片 (RightPanel 行 148-157) → 自动滚动 (行 134-138)。
5. **Authentication**: `MobileQuickActions` 或 `DockNav ActionButton` 触发 `doLogin`/`doLogout`；`MobileQuickActions.handleQuickLogin` 先绑 WLAN (行 24-32)；认证状态反映按钮 loading。

## Connections

* [[android/frontend/src/hooks/useAdapterStore]]: 管理 `activePanel` 导航核心状态（DockNav 行 178, TabletShell 行 79）。
* [[android/frontend/src/hooks/useConfigStore]]: 提供 `enableNetworkQuality`, `accounts`, `language` 等（BottomNav 行 31, DockNav 行 181）。
* [[android/frontend/src/hooks/useAuthStore]]: 处理登录/注销状态与动作（DockNav 行 179-183, MobileQuickActions 行 17-20）。
* [[android/frontend/src/shared/ui-constants]]: 导出 `NAV_ITEMS` 供 DockNav 过滤（DockNav 行 15, 185）。
* [[android/frontend/src/auth/DashboardPanel]]: 被 MobileDashboard 复用注入卡片（MobileDashboard 行 17, 155）。
* [[android/frontend/src/monitor/MonitorPanel]]: 用于 TabletShell 与 MobileMore（TabletShell 行 41, MobileMore 行 8）。
* [[android/frontend/src/settings/SettingsPanel]]: 用于 TabletShell 与 MobileMore（TabletShell 行 44, MobileMore 行 10）。
* [[android/frontend/src/shared/LogPanel]]: 用于 MobileMore 与 TabletShell（MobileMore 行 11, TabletShell 行 29）。
* [[android/frontend/src/monitor/StatusBar]]: 平板外壳引入（TabletShell 行 35, 250）。
* [[android/frontend/src/lib/dock-lens]]: `useDockLens` 镜头系统被 BottomNav/DockNav 共用（BottomNav 行 15, DockNav 行 28）。
* [[android/frontend/src/lib/glass-dock]]: `useGlassRefraction` 玻璃折射同源（BottomNav 行 14, DockNav 行 27）。

## Known Issues

* **RightPanel 清空动画在 WebView 后台卡死**: GSAP 回调依赖 rAF ticker，安卓 WebView 后台/遮挡时 rAF 停摆，tween 冻结导致 `onComplete`/`onInterrupt` 不触发，`isClearing` 永久 true（RightPanel.tsx 行 108-111）。已用 `setTimeout` failsafe 兜底（行 112-117），且 `onInterrupt` 复位避免永久禁用（行 99-105）。
* **BottomNav 玻璃样式优先级冲突**: 注释明确禁止在 JSX 写 inline 背景样式覆盖 `.glass-nav` CSS，否则切档时死代码（BottomNav.tsx 行 73-75）。
* **DockNav 按钮自缩放与镜片冲突**: 已移除 hover 上浮放大（原 `whileHover` scale），避免 scale 撑高 nav 导致镜片尺寸抖动（DockNav.tsx 行 131-133 注释说明）。
* **TabletShell 主区缩放**: 主区使用 `CONTENT_ZOOM=0.9` 缩放（`TabletShell.tsx` 行 72 定义、行 256 使用），与桌面 1:1 不同，旨在平衡平板触屏屏占比。
* **MobileDashboard 运营商徽标重复**: 仅 `formFactor === 'phone'` 渲染运营商徽标，避免与平板 StatusBar 重复（MobileDashboard.tsx 行 49-50, 81-91）。
* **MobileMore 质量关闭派生降级**: `config` 晚到导致 `qualityEnabled` 翻转时，已存的 `monitor` sub 需降级到 `settings` 保持 chips 一致（MobileMore.tsx 行 39-40）。
* **动画降级条件**: DockNav 镜头系统在非玻璃档或 economy 档或 reducedMotion 时禁用（DockNav.tsx 行 195：`glassLevel !== 'off' && profile.tier !== 'economy' && !reducedMotion`）；BottomNav 同理（BottomNav.tsx 行 51：`glassLevel !== 'off' && !reducedMotion`）。
* **TitleBar 窗口控制在平板上禁用**: 安卓平板无窗口管理 API，TabletShell 向 TitleBar 传入 `showWindowControls={false}`（TabletShell.tsx 行 234），此时标题栏拖动、双击最大化及最小化/关闭按钮均被隐藏或禁用（TitleBar.tsx 行 74, 82, 88, 232）。
