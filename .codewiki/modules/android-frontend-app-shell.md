---
title: android-frontend-app-shell
type: module
source_files:
  - android/frontend/src/App.tsx
  - android/frontend/src/components/layout/BottomNav.tsx
  - android/frontend/src/components/layout/DockNav.tsx
  - android/frontend/src/components/layout/RightPanel.tsx
  - android/frontend/src/components/layout/TitleBar.tsx
  - android/frontend/src/components/mobile/MobileDashboard.tsx
tags: [android, frontend, tauri, responsive, glassmorphism, framer-motion]
---

## Overview

本模块是 Android 前端应用的**外壳与布局系统**，负责根据设备形态（手机 vs 平板）动态渲染不同的应用容器。核心职责包括：

1. **响应式外壳路由**：通过 `useFormFactor()` 判断屏幕形态，手机端渲染 `AppInner`（底部导航+单列卡流），平板端渲染 `TabletShell`（桌面 Dock 布局，面板内容仍为安卓版，见 `App.tsx` 行 361-368 注释与行 363-375 实现）。
2. **触控与视觉适配**：针对安卓 WebView 特性，实现磨砂玻璃拟态（Glassmorphism）导航栏（`.glass-nav` / `.glass-dock`），以及基于 Framer Motion / GSAP 的复杂交互动画（如 Dock Lens 镜头放大系统）。手机外壳视觉延续桌面暗色 surface/主题色/圆角卡（见 `App.tsx` 行 1-5 注释）。
3. **状态管理与持久化**：管理 Tab 切换、账号列表、配置项及新手引导状态，确保跨会话的状态一致性（如 `safeStorage.get('campus-mobile-tab')` 恢复页签，见 `App.tsx` 行 57-62）。
4. **面板内容聚合**：作为容器，将业务逻辑组件（如 `MobileDashboard`, `AccountPanel`, `MonitorPanel`）适配到移动端或平板端的特定布局中。注意：桌面件（`TitleBar`/`StatusBar`/`RightPanel`/`DockNav`/`FluidBackground`/`Onboarding`/`Sponsor`）在手机外壳不再渲染（`App.tsx` 行 4 注释），平板外壳复用桌面件。

## Key Components

### `android/frontend/src/App.tsx` (安卓双外壳入口)

- `function AppInner()` (行 52-359)：手机移动外壳主组件。负责全局初始化（`useAppInit` 行 53）、帧率适配（`useAdaptiveFramePace` 行 54）、Tab 状态管理、面板转场动画及各类弹窗。
  - `useState<MobileTab>` (行 57-62)：从 `safeStorage.get('campus-mobile-tab')` 读取历史页签，白名单 `['dashboard', 'account', 'selfservice', 'quality', 'monitor', 'more']` 校验（KI#14，防止后台检测页杀进程重开回落 dashboard）。
  - `tabOrder` 常量 (行 70-76)：底栏顺序数组，第 4 位根据 `configEnableNetworkQuality !== false` 在 `quality` 与 `monitor` 间动态切换。
  - `handleTabChange(next)` (行 119-127)：处理底栏切换，`markInteraction()` 标记交互，`tabOrder.indexOf` 计算 `navDir` 方向，写入 `safeStorage.set('campus-mobile-tab', next)`。
  - `useEffect` 配置归位 (行 131-136)：配置加载后若质量检测开启且当前 `tab === 'monitor'`，自动 `handleTabChange('quality')` 避免底栏无高亮。
  - `useEffect` 新手向导 (行 138-146)：首次启动无账号（`!useConfigStore.getState().config.user`）且未标记 `campus-onboarding-done` 时，打开 `onboardingOpen`。
  - `switch (deferredTab)` (行 148-192)：根据延迟值 `deferredTab` 渲染 `MobileDashboard` / `AccountPanel` / `SelfServicePanel` / `QualityPanel` / `MonitorPanel` / `MobileMore`。
  - 顶部轻 header JSX (行 202-260)：绝对定位 `backdrop-blur-md`，含状态色点、在线运营商 chip (`IspMark`)、更新胶囊、网络质量胶囊（`NetworkQualityCapsule`，质量关闭时隐藏）、赞助/设置/主题/关于入口。
  - 弹窗管理 JSX (行 301-356)：`ToastContainer`, `SponsorCard`, 懒加载 `AboutDialog` (行 305-316), `UpdateAvailableDialog` (行 318), 懒加载 `ThemeDialog` (行 320-336, 内联 `setThemeName` + `safeStorage.set`), `ConfirmDialog` 删除账号 (行 338-345), 懒加载 `OnboardingWizard` (行 348-356)。

- `export default function App()` (行 363-375)：双外壳路由根组件。
  - `const formFactor = useFormFactor()` (行 364)：获取设备形态。
  - 渲染分支 (行 366-372)：`ErrorBoundary` 内包 `AnimationActiveProvider`，`formFactor === 'tablet' ? <TabletShell /> : <AppInner />`；双外壳共用 `<FaceCaptureDialog />` (行 370) 与 `<NotificationPermissionGate />` (行 371)。

- `function NotificationPermissionGate()` (行 381-405)：首启通知权限引导。仅询问一次（`campus-notification-prompt` 标记），Android 13+ 弹系统框，否则降级跳应用通知设置页（`requestNotificationPermission({ openSettingsIfDenied: true })` 行 400）。

### `android/frontend/src/components/layout/BottomNav.tsx` (移动端底部导航)

- `export type MobileTab` (行 18)：联合类型 `'dashboard' | 'account' | 'selfservice' | 'quality' | 'monitor' | 'more'`，定义手机底栏页签。
- `const TABS` (行 20-25)：基础页签数组，含 `dashboard`(LayoutDashboard), `account`(UserCircle), `selfservice`(Globe), `more`(Ellipsis)，每项结构 `{ id: MobileTab; labelKey: string; Icon: typeof LayoutDashboard }`。
- `const QUALITY_TAB` (行 27) / `const MONITOR_TAB` (行 28)：动态插入的第 4 位页签常量（Signal / Activity 图标）。
- `function useNavTabs()` (行 30-35)：Hook，读取 `useConfigStore` 的 `enableNetworkQuality !== false`，将 `QUALITY_TAB` 或 `MONITOR_TAB` 插入 `TABS` 索引 3 处返回。
- `export function BottomNav({ tab, onChange }: { tab: MobileTab; onChange: (t: MobileTab) => void })` (行 37-152)：底部磨砂玻璃胶囊栏。
  - `useGlassRefraction(navRef, glassLevel)` (行 46)：挂载折射玻璃到 `navRef`（行 44-45 声明）。
  - `useDockLens(navRef, {...})` (行 60-65)：复用桌面镜头系统（弹簧药丸、拖动滑选），`lensEnabled` 依赖 `glassLevel !== 'off' && !reducedMotion`（行 51，手机无 economy 档）。
  - JSX 结构 (行 67-151)：`nav.glass-nav` 容器，`data-dock-lens` 镜片底 (行 86-94)，`data-dock-strip` 真实交互项层 (行 99-126, 等宽 `w-16` 按钮常显标签)，`data-dock-magnify` 放大副本层 (行 130-148) 防残影。

### `android/frontend/src/components/layout/DockNav.tsx` (桌面/平板 Dock 导航)

- `const ICON_MAP` (行 30-39)：字符串图标名到 lucide 组件的映射记录 `Record<string, typeof LayoutDashboard>`。
- `function DockItem({ id, label, icon, isActive, ghost, onPanelChange }: ...)` (行 41-85)：Dock 单项。激活态 (`isActive`) 显示图标+文字标签（行 72-74），非激活仅图标；hover tooltip（行 75-82）；`ghost` 模式用于放大层副本（不交互，行 54-58）。
- `function ActionButton({ label, loadingLabel, icon, isLoading, isDisabled, onAction, variant }: ...)` (行 88-170)：登录/注销按钮。
  - GSAP 旋转 (行 114-122)：`isLoading` 时 `gsap.to` 旋转 spinner。
  - 脉冲边框 `usePulseAnimation` (行 107, 150-156)：加载时显示 `loadingPulse` 边框。
  - `whileTap` 缩放 (行 134)：非加载时按压缩放 0.95。
- `interface DockNavProps` (行 171-174)：字段 `onPanelChange: (panel: PanelName) => void` (行 172)，`outerRef?: (el: HTMLDivElement | null) => void` (行 173)。
- `export const DockNav = memo(function DockNav({ onPanelChange, outerRef }: DockNavProps)` (行 176-296)：平板/横屏底部 Dock。
  - `visibleItems` (行 185)：`NAV_ITEMS.filter(item => enableNetworkQuality || item.id !== 'quality')` 过滤不可见项。
  - `useGlassRefraction(navRef, profile.tier === 'economy' ? 'off' : glassLevel)` (行 191)：economy 档关闭玻璃。
  - `useDockLens` (行 204-209)：镜头系统，`lensEnabled` 在 economy 或 reducedMotion 时禁用 (行 195)。
  - JSX (行 211-293)：固定定位 `fixed z-30` (行 214)，留出右栏宽度 `calc(100vw - var(--right-panel-width, 288px))` (行 215)；渲染 `DockItem` 列表与分隔线、两个 `ActionButton` (登出 outline 行 273-281, 登录 primary 行 283-291)。

### `android/frontend/src/components/layout/RightPanel.tsx` (右侧日志面板 - 桌面/平板共用)

- `interface RightPanelProps` (行 14-18)：字段 `logs: LogEntry[]` (行 15)，`onClearLogs?: () => void` (行 16)，`outerRef?: (el: HTMLDivElement | null) => void` (行 17)。
- `const LOG_ICONS` (行 20-25) / `LOG_COLORS` (行 27-32) / `LOG_BG_COLORS` (行 34-39) / `LOG_BAR_COLORS` (行 41-46)：`LogEntry['type']` 到图标与颜色的映射常量。
- `const RIGHT_PANEL_ANIM_THRESHOLD = 50` (行 51) / `const RIGHT_PANEL_ANIM_KEEP_COUNT = 30` (行 52)：性能分片阈值常量。
- `export const RightPanel = memo(function RightPanel({ logs, onClearLogs, outerRef }: RightPanelProps)` (行 54-278)：
  - `handleClearWithAnimation` (行 64-125)：清空日志 GSAP stagger 退出动画（行 88-107），含 `onInterrupt` 复位（行 99-105）与 `setTimeout` failsafe（行 112-117）应对 WebView 后台 rAF 停摆。
  - `handleScroll` (行 127-131)：判断用户是否手动滚动（`isAutoScrollRef`）。
  - `useEffect` 自动滚动 (行 134-138)：`isAutoScrollRef.current` 为真时滚动到底部。
  - `useMemo` 日志分片 (行 148-157)：`logs.length > 50` 时，前段 `staticLogs` 用普通 `div`，最近 30 条 `animLogs` 保留 `m.div` + `AnimatePresence`（行 240-272）。
  - JSX (行 159-277)：`w-72` 侧栏，看板娘水印 `mascot-bg-tea.webp`（行 190-197），日志列表渲染。

### `android/frontend/src/components/layout/TitleBar.tsx` (顶部标题栏 - 桌面/平板共用)

- `interface TitleBarProps` (行 11-24)：字段含 `notificationEnabled` (行 12)，`onToggleNotification` (行 13)，`onShowTheme` (行 14)，`onShowAbout` (行 15)，`onShowSponsor` (行 16)，`onToggleLightMode` (行 17)，`onMinimize` (行 18)，`onToggleMaximize` (行 19)，`onClose` (行 20)，`isMaximized` (行 21)，以及 `showWindowControls?: boolean` (行 23，无窗口管理平台安卓平板传 false 隐藏三键并禁用拖拽)。
- `MinimizeIcon` (行 26-30) / `MaximizeIcon` (行 32-36) / `RestoreIcon` (行 38-43) / `CloseIcon` (行 45-50)：内置 SVG 图标组件。
- `export const TitleBar = memo(function TitleBar({...}: TitleBarProps)` (行 52-278)：
  - `handleTitleBarMouseDown` (行 73-85)：拖拽窗口，`showWindowControls` 为 false 或点击按钮时跳过（行 74-76）。
  - `handleTitleBarDoubleClick` (行 87-90)：双击最大化/还原，受 `showWindowControls` 限制。
  - JSX (行 92-277)：左侧应用名+版本更新胶囊（行 110-141），右侧图标按钮组（光暗切换行 145-157、语言行 159-172、通知行 174-190、主题行 192-204、赞助行 206-217、关于行 219-230），`showWindowControls` 为真时渲染最小化/最大化/关闭三键（行 232-273）。

### `android/frontend/src/components/mobile/MobileDashboard.tsx` (移动总览页)

- `type LampState = 'online' | 'offline' | 'loading' | 'error'` (行 22)：状态灯类型。
- `const LAMP_COLOR` (行 24-29)：`LampState` 到 Tailwind 颜色类的映射。
- `const noopAsync = async () => {}` (行 32)：占位异步函数，因 `quickActions` 被排除。
- `const EXCLUDED_CARDS = ['quickActions']` (行 35)：传给 `DashboardPanel` 的排除卡 ID 数组。
- `function MobileStatusCard()` (行 38-96)：状态卡（原 hero）。呼吸信号灯（行 60-63），Portal 可达性、检查次数（行 72-80）；仅在 `formFactor === 'phone'` 时渲染在线运营商徽标（行 81-91）。
- `function MobileMonitorCard()` (行 99-139)：监控摘要卡。Radix `Switch` 控制后台检测（行 120-124），立即刷新按钮（行 128-135），间隔读现值 `intervalSec` 防覆盖用户设置（行 105）。
- `export function MobileDashboard()` (行 141-167)：首页仪表盘。
  - `extraCards` (行 149-152)：稳定引用的数组，注入 `mobileHero` (MobileStatusCard) 与 `mobileMonitor` (MobileMonitorCard) 到 `DashboardPanel`。
  - 渲染 `DashboardPanel` (行 155-166)：传 `excludeCards={EXCLUDED_CARDS}`，`mergeSelfServiceEmpty`。

## Architecture

```text
┌──────────────────────────────────────────────────────┐
│ App.tsx (363-375) useFormFactor()                     │
│ formFactor==='tablet' ? <TabletShell/> : <AppInner/>  │
└───────────────────────┬──────────────────────────────┘
                        │
        ┌───────────────▼───────────────┐   ┌──────────────────────────────┐
        │ AppInner (52-359) 手机外壳      │   │ TabletShell 平板外壳           │
        │ - Header (202-260) 绝对定位      │   │ - 复用桌面件(行4注释)          │
        │ - BottomNav (299行调用)         │   │ - DockNav (176-296)           │
        │ - MobileDashboard (151行调用)    │   │ - RightPanel (54-278)         │
        │ - 不渲染 TitleBar/RightPanel    │   │ - TitleBar (52-278)           │
        └───────────────┬───────────────┘   │ - 面板内容为安卓版              │
                        │                   └──────────────────────────────┘
        ┌───────────────▼───────────────┐
        │ BottomNav (37-152)             │
        │ - useNavTabs (30-35)           │
        │ - useDockLens (60-65)          │
        │ - useGlassRefraction (46)      │
        └───────────────────────────────┘
```

## Data Flow

1. **启动与初始化**：
   - `App.tsx` 挂载 (行 363-375)，`useFormFactor()` 决定渲染 `TabletShell` 或 `AppInner`。
   - `AppInner` 从 `safeStorage` 读取 `campus-mobile-tab` (行 57-62)，结合 `configEnableNetworkQuality` 确定 `tabOrder` (行 70-76)。配置就绪后若 quality 开启且 tab 在 monitor 则归位 (行 131-136)。
   - `TabletShell` 内部复用桌面件 (`TitleBar`/`DockNav`/`RightPanel`)，面板内容调用安卓版组件。

2. **导航与面板切换**：
   - **手机端**：用户点击 `BottomNav` 项 (行 103 `onClick={() => onChange(id)}`) → `App.tsx` 的 `handleTabChange` (行 119-127) 更新 `tab` state 并算 `navDir` → `useDeferredValue` (行 63) 触发 `switch (deferredTab)` (行 148-192) → `AnimatePresence` 方向性转场 (行 281-295)。
   - **平板端**：用户点击 `DockNav` 项或镜头滑选 `onLensSelect` (行 197-203) → 调用 `onPanelChange` → `TabletShell` 更新适配 store 并渲染对应安卓版面板。

3. **主题与玻璃效果**：
   - `ThemeDialog` 内联处理 (行 320-336)：`useThemeStore.getState().setThemeName` + `safeStorage.set('campus-theme', name)`。
   - `BottomNav` 行 46 `useGlassRefraction(navRef, glassLevel)` 与 `DockNav` 行 191 `useGlassRefraction(navRef, profile.tier === 'economy' ? 'off' : glassLevel)` 将玻璃折射挂到对应 nav 容器，CSS 类 `.glass-nav` / `.glass-dock` 消费这些变量。

4. **弹窗与反馈**：
   - 删除账号：`BottomNav`/`AccountPanel` 触发 `setConfirmDelete` (行 110, 160) → `ConfirmDialog` (行 338-345) 确认后 `handleDeleteAccount`。
   - 清空日志：`RightPanel` 的 `handleClearWithAnimation` (行 64-125) 执行 GSAP 动画，并依赖 `setTimeout` failsafe (行 112-117) 防止 WebView 后台死锁。

## Connections

- [[modules/android-frontend-auth]]：认证与业务组件模块，提供 `DashboardPanel` (MobileDashboard.tsx 行 17 引用)、`AboutDialogMobile` (App.tsx 行 47 懒加载)。
- [[modules/android-frontend-store]]：状态管理模块，提供 `useConfigStore` (App.tsx 行 9)、`useAuthStore` (行 10)、`useQualityStore` (行 15)、`useThemeStore` (行 16)，被外壳广泛订阅。
- [[modules/android-frontend-hooks]]：Hooks 模块，提供 `useFormFactor` (App.tsx 行 36)、`useAppInit` (行 8)、`useDockLens` (DockNav 行 28)、`useGlassRefraction` (行 27)、`useAnimationProfile` (行 44) 等底层能力。
- [[modules/android-backend-tauri]]：Tauri 后端模块，`tauriApiWithRetry` (App.tsx 行 14) 提供 `openExternal` 等命令，供 `AboutDialog` 等调用。

## Known Issues

1. **Tab 动态性与归位**：`App.tsx` 中 `tabOrder` 是动态的 (行 70-76)。若用户在 `monitor` 页（当 quality 关闭时），随后开启 quality 开关，页面会自动跳回 `quality` 页 (行 131-136)，避免底栏无高亮，但可能导致用户意外离开当前查看的监控页面。
2. **GSAP 动画死锁**：`RightPanel.tsx` 清空日志时 (行 64-125)，若 WebView 后台挂起导致 rAF 停止，GSAP tween 冻结，`onComplete` 不触发，`isClearing` 永久为 true 且按钮禁用。已引入 `setTimeout` 作为 failsafe (行 112-117) 缓解，但极端情况下仍依赖该兜底计时。
3. **手机壳桌面件禁用**：根据 `App.tsx` 行 4 注释与行 202-260 实现，手机外壳不再渲染桌面 `TitleBar`/`StatusBar`/`RightPanel`/`DockNav` 等，相关组件仅在 `TabletShell`（平板形态）下生效；若误在 `AppInner` 中引用会导致布局崩溃。
4. **通知权限时序**：`NotificationPermissionGate` (行 381-405) 为首启弹窗，依赖于 `safeStorage.get('campus-notification-prompt')` 标记。新手向导 `OnboardingWizard` 为后挂载全屏层，天然先于本弹窗展示，无需额外时序协调（见行 377-380 注释），但若存储被手动清除会再次弹出。
