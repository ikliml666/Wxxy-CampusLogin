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
1.  **响应式外壳路由**：通过 `useFormFactor()` 判断屏幕形态，手机端渲染 `AppInner`（底部导航+单列流），平板端渲染 `TabletShell`（Dock 导航+多栏布局）。
2.  **触控与视觉适配**：针对安卓 WebView 特性，实现 CSS `zoom` 缩放优化触屏点击区域、磨砂玻璃拟态（Glassmorphism）导航栏、以及基于 Framer Motion/GSAP 的复杂交互动画。
3.  **状态管理与持久化**：管理 Tab 切换、账号列表、配置项及新手引导状态，确保跨会话的状态一致性。
4.  **面板内容聚合**：作为容器，将业务逻辑组件（如 Dashboard, Account, Monitor）适配到移动端或平板端的特定布局中。

## Key Components

### `src/App.tsx` (安卓双外壳入口)
*   **职责**：根据屏幕形态渲染不同的外壳布局。手机使用底部导航+单列卡流，平板使用桌面 Dock 布局。负责全局状态初始化、新手引导、通知权限、主题切换及各类弹窗管理。
*   **关键实现**：
    *   **外壳路由**：`useFormFactor()` 决定渲染 `<TabletShell />` 还是 `<AppInner />` (`364-368`)。两者共享 `FaceCaptureDialog` 和 `NotificationPermissionGate`。
    *   **Tab 状态与持久化**：`MobileTab` 类型包含 `dashboard`, `account`, `selfservice`, `quality`, `monitor`, `more`。从 `safeStorage` 读取上次页签，白名单校验后恢复 (`57-62`)。使用 `useDeferredValue` 延迟 Tab 更新以优化动画性能 (`63`)。
    *   **动态 Tab 顺序**：底栏第 4 个位置根据 `configEnableNetworkQuality` 在 `quality` 和 `monitor` 间切换 (`70-76`)。若配置加载后开关开启但当前在 `monitor`，自动跳转至 `quality` (`131-136`)。
    *   **面板转场动画**：基于 `navDir` (-1/0/1) 计算滑动方向，配合 `framer-motion` 的 `AnimatePresence` 和 `createPanelAppleVariants` 实现方向性面板切换 (`117-118`, `281-295`)。
    *   **顶栏设计**：绝对定位覆盖在内容上方，利用 `backdrop-blur-md` 实现磨砂效果。包含状态指示灯、运营商 Chip、更新提醒胶囊、网络质量胶囊及设置/主题/关于按钮 (`199-260`)。
    *   **快捷登录浮条**：仅在 Dashboard 页签显示，悬浮于底栏上方 (`263-267`)。
    *   **弹窗管理**：
        *   `AboutDialog`: 懒加载，处理版本更新回调 (`306-316`)。
        *   `ThemeDialog`: 内联处理主题切换逻辑，同步更新 Store 和本地存储 (`321-336`)。
        *   `ConfirmDialog`: 用于账号删除确认 (`338-345`)。
        *   `OnboardingWizard`: 首次启动无用户时弹出，标记由向导内部完成 (`348-356`)。
    *   **通知权限门控**：`NotificationPermissionGate` 组件，仅询问一次，Android 13+ 请求系统权限，否则降级至应用设置页 (`381-404`)。

### `src/components/layout/BottomNav.tsx` (移动端底部导航)
*   **职责**：提供移动端的底部导航栏，支持“镜头”放大效果（Dock Lens），视觉风格为磨砂玻璃圆角胶囊。
*   **关键实现**：
    *   **Tab 定义**：导出 `MobileTab` 类型。基础 Tab 数组 `TABS` 加上动态插入的 `quality` 或 `monitor` (`20-35`)。
    *   **镜头系统**：复用桌面 `useDockLens` hook，实现弹簧驱动的药丸选中指示器、按下抬起、果冻形变及拖动滑选 (`50-65`)。
    *   **玻璃折射**：通过 `useGlassRefraction` 将玻璃折射效果挂载到导航栏容器上 (`46`)。
    *   **结构**：包含真实交互项层 (`data-dock-strip`) 和隐藏的放大副本层 (`data-dock-magnify`)，防止放大时产生残影 (`99-148`)。
    *   **样式细节**：等宽按钮 `w-16`，图标 26px，标签始终显示（非激活态也显示标签，区别于桌面的仅激活显示标签）。

### `src/components/layout/DockNav.tsx` (桌面/平板 Dock 导航)
*   **职责**：用于平板横屏或桌面模式的底部 Dock 导航，功能比 BottomNav 更丰富，包含登录/注销按钮。
*   **关键实现**：
    *   **组件结构**：`DockItem` 处理单个图标/标签渲染，激活态显示标签和非激活态隐藏标签；`ActionButton` 处理登录/注销按钮，带加载旋转动画 (`gsap`) 和脉冲边框 (`usePulseAnimation`) (`41-169`)。
    *   **导航逻辑**：根据 `enableNetworkQuality` 过滤 `NAV_ITEMS`，排除不可见的 quality tab (`185`)。
    *   **镜头与玻璃**：同样使用 `useDockLens` 和 `useGlassRefraction`，但在 'economy' 动画档位下禁用镜头效果 (`191-195`)。
    *   **布局**：固定定位，左侧留出右侧面板宽度，底部留安全区 (`214-215`)。

### `src/components/layout/RightPanel.tsx` (右侧日志面板 - 桌面/平板共用)
*   **职责**：展示运行时日志，支持自动滚动、清空动画、性能降级。
*   **关键实现**：
    *   **性能分片**：当日志超过 50 条 (`RIGHT_PANEL_ANIM_THRESHOLD`) 时，前段静态日志用普通 `div` 渲染，仅保留最近 30 条 (`RIGHT_PANEL_ANIM_KEEP_COUNT`) 使用 `m.div` + `AnimatePresence` 进行进出场动画 (`148-157`)。
    *   **清空动画**：使用 `gsap` 对现有 DOM 元素执行 stagger 退出动画。包含复杂的防死锁机制：`onInterrupt` 重置状态，以及 `setTimeout` failsafe 应对 WebView 后台 rAF 停摆导致的动画冻结 (`64-125`)。
    *   **自动滚动**：监听滚动事件判断用户是否手动滚动，仅在自动模式下跟随最新日志滚动 (`127-138`)。
    *   **视觉**：不同日志级别 (`info/success/error/warning`) 对应不同的颜色、背景色和左侧条颜色 (`20-46`)。

### `src/components/layout/TitleBar.tsx` (顶部标题栏 - 桌面/平板共用)
*   **职责**：提供窗口控制（最小化/最大化/关闭）、应用信息、版本更新提示、主题/语言/通知/赞助入口。
*   **关键实现**：
    *   **窗口拖拽与缩放**：通过 `handleTitleBarMouseDown` 和 `handleTitleBarDoubleClick` 调用 Tauri API 实现拖拽和最大化/还原，需区分 `showWindowControls` 属性以适配无窗口管理平台的安卓平板 (`73-90`)。
    *   **版本更新 UI**：显示当前版本，若有新版本则显示带红点的更新按钮和详情 Tooltip (`110-141`)。
    *   **图标按钮**：所有操作按钮均包裹在 `Tooltip` 中，支持悬停旋转效果 (`145-230`)。
    *   **自定义 SVG 图标**：内置 Minimize, Maximize, Restore, Close 的 SVG 路径，避免依赖外部库 (`26-50`)。

### `src/components/mobile/MobileDashboard.tsx` (移动总览页)
*   **职责**：移动端的首页仪表盘，复用桌面 `DashboardPanel` 的核心逻辑，注入移动端专属卡片。
*   **关键实现**：
    *   **卡片注入**：定义 `extraCards` 数组，包含 `mobileHero` (状态卡) 和 `mobileMonitor` (监控摘要卡)，并通过 `excludeCards: ['quickActions']` 移除桌面版的 DHCP 续租等非移动端功能 (`149-164`)。
    *   **MobileStatusCard**：显示网络状态灯（呼吸动画）、Portal 可达性、检查次数。仅在手机形态下显示运营商徽标 (`38-96`)。
    *   **MobileMonitorCard**：显示后台检测开关（使用 Radix Switch）和立即刷新按钮，读取最新的 `backgroundCheckInterval` 避免覆盖用户设置 (`99-139`)。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                        App.tsx                              │
│  (Form Factor Detection via useFormFactor())                │
├──────────────────────┬──────────────────────────────────────┤
│       TabletShell    │              AppInner                │
│  (Left/Right Split,  │  (Mobile Layout, BottomNav, Header)  │
│   Zoom Factors)      │                                      │
├──────────────────────┼──────────────────────────────────────┤
│  TitleBar/StatusBar  │  TitleBar (Absolute, Glassy)         │
│  Main Content Area   │  Main Content (Scrollable)           │
│  RightPanel (Logs)   │  BottomNav (Glass, Lens)             │
│  DockNav             │  QuickActions (Floating)             │
├──────────────────────┼──────────────────────────────────────┤
│  MobileDashboard     │  MobileDashboard                     │
│  AccountPanel        │  AccountPanel                        │
│  More (Chips)        │  More (Chips)                        │
└──────────────────────┴──────────────────────────────────────┘
                       │ Shared Stores & Hooks
                       ▼
┌─────────────────────────────────────────────────────────────┐
│  useConfigStore | useAuthStore | useQualityStore | Logs     │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **启动与初始化**：
    *   `App.tsx` 挂载，通过 `useFormFactor()` 决定渲染 `TabletShell` 或 `AppInner`。
    *   `AppInner` 从 `safeStorage` 读取 `campus-mobile-tab`，结合 `configEnableNetworkQuality` 确定 `tabOrder` 和 `navDir`。
    *   `TabletShell` 初始化 Store 订阅，设置 `panelChangeLock` 和 Onboarding 检查。

2.  **导航与面板切换**：
    *   **手机端**：用户点击 `BottomNav` 项 -> `App.tsx` 更新 `tab` state -> 计算 `navDir` -> `AnimatePresence` 触发动画 -> 渲染对应 Panel (`MobileDashboard`, `MobileMore` 等)。
    *   **平板端**：用户点击 `DockNav` 项 -> `TabletShell.tsx` 调用 `handlePanelChange` -> 锁定 `panelChangeLock` -> 更新 `useAdapterStore` -> `deferredPanel` 变化 -> Switch Case 渲染对应 Panel。

3.  **主题与玻璃效果**：
    *   `index.html` 或 `main.tsx` 设置 `data-glass` 属性。
    *   `src/index.css` 根据 `[data-glass="..."]` 选择器应用对应的 `--glass-blur`, `--glass-tint` 等变量。
    *   `.glass-dock` / `.glass-nav` 利用这些变量进行多层渐变和 backdrop-filter 渲染。

4.  **弹窗与反馈**：
    *   用户操作（如删除账号）触发 `ConfirmDialog` -> 确认后调用 Hook Handler (`handleDeleteAccount`) -> 更新 Store -> UI 重渲染。
    *   `RightPanel` 清空日志 -> GSAP 动画 -> `setTimeout` Failsafe 防止死锁。

## Connections

*   [[modules/android-frontend-auth]]: 认证模块，包含 `DashboardPanel`, `AboutDialog` 等核心业务组件。
*   [[modules/android-frontend-store]]: 状态管理模块，包含 `useConfigStore`, `useAuthStore`, `useQualityStore`，被 `App.tsx` 及各类面板广泛引用。
*   [[modules/android-frontend-hooks]]: Hooks 模块，包含 `useAuth`, `useHelloGate`, `useDockLens` 等，被 UI 组件调用。
*   [[modules/android-backend-tauri]]: Tauri 后端模块，提供真实的 `invoke` 命令实现，前端 Mock 旨在模拟其行为。

## Known Issues

1.  **崩溃恢复阈值**：`main.tsx` 中将渲染心跳超时阈值设为 **10s** (行 109)，而非默认的 5s，以防止低端机 GC 导致误判重启。但若真发生 GPU 崩溃，恢复可能较慢。
2.  **Tab 动态性与归位**：`App.tsx` 中 `tabOrder` 是动态的。如果用户在 `monitor` 页（当 quality 关闭时），随后开启了 quality 开关，页面会自动跳回 `quality` 页，避免底栏无高亮 (行 131-136)。此行为可能导致用户意外离开当前查看的监控页面。
3.  **GSAP 动画死锁**：`RightPanel` 清空日志时，若 WebView 后台挂起导致 rAF 停止，GSAP tween 冻结，`onComplete` 不触发，`isClearing` 永久为 true，按钮禁用。引入 `setTimeout` 作为 failsafe (行 112-117) 缓解此问题。