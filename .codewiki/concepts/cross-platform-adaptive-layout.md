---
title: cross-platform-adaptive-layout
type: concept
source_files:
  - android/frontend/src/main.tsx
  - android/frontend/src/App.tsx
  - android/frontend/src/components/layout/BottomNav.tsx
  - android/frontend/src/components/layout/DockNav.tsx
  - android/frontend/src/components/layout/RightPanel.tsx
  - android/frontend/src/components/mobile/MobileDashboard.tsx
  - android/frontend/src/components/mobile/MobileMore.tsx
  - android/frontend/src/hooks/useFormFactor.ts
  - android/frontend/src/settings/OnboardingWizard.tsx
tags:
  - responsive-design
  - form-factor
  - mobile-first
  - tablet-adaptation
---

## Overview

`cross-platform-adaptive-layout` 是 Android 前端应用的核心布局策略，旨在通过统一代码库适配不同设备形态（手机与平板）。该机制基于 `useFormFactor` Hook (`android/frontend/src/hooks/useFormFactor.ts|38-40`) 在运行时动态判定屏幕短边尺寸（sw600dp 语义），从而在 `AppInner`（移动端视图，定义在 `App.tsx|52-359`）和 `TabletShell`（平板端外壳，在 `App.tsx|363-375` 中条件渲染）之间切换。

移动端采用底部标签栏 (`BottomNav`) 与全屏卡片流，平板端采用侧边 Dock 栏 (`DockNav`)、右侧日志面板 (`RightPanel`) 以及桌面件同源复用的布局。业务逻辑层（认证、设置、监控）保持状态一致，仅由不同容器包装展示。

## Key Components

### 1. 入口与全局配置 (`android/frontend/src/main.tsx`)

**职责**：应用入口，初始化 GSAP、主题、崩溃恢复，挂载 React 根。

- `gsap.defaults` / `gsap.config` / `gsap.ticker.lagSmoothing` (17-19)：默认 ease 为 `expo.out`，`autoSleep: 5`，关闭 nullTargetWarn，设置 lagSmoothing。
- `prefers-reduced-motion` 处理 (21-25)：匹配时强制 duration 0 并关闭 lagSmoothing。
- `initTheme()` (27-44)：读取 `safeStorage` 的 `campus-light-mode`、`campus-theme`、`campus-glass-level`，设置 `dark` 类、`theme-${theme}` 类及 `data-glass` 属性，避免启动闪烁。
- `setupCrashRecovery()` (48-114)：可见性检测 (62-69)、GPU/WebGL 错误监听 (71-77)、心跳机制 (79-113，阈值 10s 判定渲染丢失)。调用位于 116 行。
- `window.__onTrimMemory` (123-129)：安卓内存压力钩子，高压力级别时释放人脸引擎。
- `AppWrapper` (131-133)：DEV 包 StrictMode，生产用 Fragment。
- `ReactDOM.createRoot` 渲染树 (135-145)：`ErrorBoundary` -> `LazyMotion(domMax, strict)` -> `MotionConfig(reducedMotion="user")` -> `<App />`。

### 2. 核心路由与外壳分发 (`android/frontend/src/App.tsx`)

**职责**：双外壳决策，管理移动端状态、布局与全局弹窗。

- `lazy` 导入 (47-50)：`AboutDialogMobile`、`MobileQuickActions`、`ThemeDialog`、`OnboardingWizard` 均按需懒加载。
- `AppInner` 函数 (52-359)：
  - Tab 状态 (57-62)：从 `safeStorage` 读 `campus-mobile-tab`，白名单含 `dashboard/account/selfservice/quality/monitor/more`。
  - `tabOrder` 动态数组 (70-76)：依据 `configEnableNetworkQuality` 决定第 4 位为 `quality` 或 `monitor`。
  - `handleTabChange` (119-127)：计算导航方向 `navDir`，持久化 tab。
  - 面板内容映射 `switch (deferredTab)` (148-192)：渲染 `MobileDashboard`/`AccountPanel`/`SelfServicePanel`/`QualityPanel`/`MonitorPanel`/`MobileMore`。
  - 顶部轻 header (202-260)：绝对定位磨砂玻璃，含状态点、运营商 chip、更新胶囊、网络质量胶囊、主题/关于入口。
  - 底部 `<BottomNav>` (299)；弹窗管理 (303-356)；通知权限门控 `NotificationPermissionGate` (381-405)。
- `App` 默认导出 (363-375)：`useFormFactor()` 返回 `'tablet'` 时渲染 `<TabletShell />`，否则 `<AppInner />`；共享 `<FaceCaptureDialog />` 与 `<NotificationPermissionGate />`。
- `NotificationPermissionGate` (381-405)：首次启动询问通知权限，标记 `campus-notification-prompt`。

### 3. 设备形态判定 Hook (`android/frontend/src/hooks/useFormFactor.ts`)

- 类型 `FormFactor = 'phone' | 'tablet'` (9)；`Orientation = 'portrait' | 'landscape'` (10)。
- 常量 `TABLET_MIN_EDGE_PX = 600` (13)。
- `detectFormFactor()` (15-17)：`Math.min(innerWidth, innerHeight) >= 600` 判为平板。
- `detectOrientation()` (19-21)：宽≥高为 landscape。
- `useViewportMeasure<T>()` (24-36)：订阅 `resize` 与 `orientationchange` 重算。
- `useFormFactor()` (38-40) 与 `useOrientation()` (43-45)：导出 Hook。

### 4. 移动端导航 (`android/frontend/src/components/layout/BottomNav.tsx`)

- 类型 `MobileTab` (18)；`TABS` 基础列表 (20-25)；`QUALITY_TAB`/`MONITOR_TAB` (27-28)。
- `useNavTabs()` (30-35)：依据 `enableNetworkQuality` 插入对应 tab。
- `BottomNav` 组件 (37-151)：磨砂玻璃胶囊 (`glass-nav`)，等宽按钮 `w-16`，集成 `useGlassRefraction` 与 `useDockLens` 镜头系统 (60-65)，含药丸指示与放大层副本 (130-148)。

### 5. 平板端 Dock 导航 (`android/frontend/src/components/layout/DockNav.tsx`)

- `ICON_MAP` (30-39)：图标映射。
- `DockItem` (41-85)：激活项显示图标+标签，hover tooltip。
- `ActionButton` (88-169)：登录/注销按钮，loading 旋转 (`gsap.to` 118)，脉冲边框，primary/outline 样式。
- `DockNavProps` 接口 (171-174)。
- `DockNav` 组件 (176-295)：`memo` 包装，过滤 `NAV_ITEMS` (185)，固定底部居中 (211-216)，集成 `useGlassRefraction` 与 `useDockLens` (191-209)，渲染镜头底、项容器、放大层及分隔线后的动作按钮。

### 6. 平板端右侧面板 (`android/frontend/src/components/layout/RightPanel.tsx`)

- 常量 `LOG_ICONS` (20-25)、`LOG_COLORS` (27-32)、`LOG_BG_COLORS` (34-39)、`LOG_BAR_COLORS` (41-46)。
- 性能阈值 `RIGHT_PANEL_ANIM_THRESHOLD = 50` (51)、`RIGHT_PANEL_ANIM_KEEP_COUNT = 30` (52)。
- `RightPanel` 组件 (54-279，`memo`)：
  - `handleClearWithAnimation` (64-125)：GSAP stagger 清理，failsafe setTimeout (112-117) 防 rAF 停摆卡死。
  - 日志分片 (148-157)：超阈值前段静态 `div`，最近 30 条 `m.div`+`AnimatePresence`。
  - 自动滚动 (127-138) 与空态呼吸动画 (58, 209)。

### 7. 移动端仪表盘 (`android/frontend/src/components/mobile/MobileDashboard.tsx`)

- 类型 `LampState` (22)；`LAMP_COLOR` (24-29)；`noopAsync` (32)；`EXCLUDED_CARDS = ['quickActions']` (35)。
- `MobileStatusCard` (38-96)：状态灯、Portal 可达性、检查次数、运营商标签（仅 phone 形态，81 行）。
- `MobileMonitorCard` (99-139)：Radix Switch 控制后台检测，立即刷新按钮。
- `MobileDashboard` 导出 (141-168)：通过 `extraCards` (149-152) 将两张移动卡注入桌面 `DashboardPanel`，排除 `quickActions`。

### 8. 移动端“更多”聚合页 (`android/frontend/src/components/mobile/MobileMore.tsx`)

- 类型 `MoreTab` (19)；`TABS` (21-26，含 `qualityOnly` 标记)；`MobileMoreProps` (28-31)。
- `MobileMore` 组件 (33-84)：依据 `qualityEnabled` 过滤 monitor 子页 (37)，`sub` 派生降级 (40)，渲染 `MonitorPanel`/`SpeedTestPanel`/`LogPanel`/`SettingsPanel`。

### 9. 引导向导（响应式 Dialog，`android/frontend/src/settings/OnboardingWizard.tsx`）

> 注：旧文档描述的独立手机版 `OnboardingWizardMobile.tsx` 在当前源码中不存在；手机与平板共用本文件中的 `OnboardingWizard`，通过内部 CSS 断点（如 `min-[520px]:hidden` 等，见 544-576 行）实现响应式布局。

- 常量 `STEP_TITLE_KEYS` (42-43)、`OPERATOR_VALUE_BY_KEY` (46-50)、`STACK_LAYERS` (55-60)、`EXIT_UP_MS` (63)、`DECK_CARD_HEIGHT_FALLBACK` (65)、`STACK_TRANSITION` (66)、`deckTransform` (67-68)、`EXIT_UP_TRANSFORM` (69)、`ENTER_FROM_TRANSFORM` (71)、`STACK_TRANSITION_WITH_FADE` (73)、`STEP_ICONS` (75)。
- `DeckArrow` (77-100)：上一步/下一步箭头，禁用态处理。
- `OnboardingWizard` 主组件 (107-628)：
  - 接收 `open/onClose/onUpdateConfig/onLogin/isLoggingIn` (34-40, 107)。
  - 卡片堆动画状态 `exitingStep`/`enteringStep` (127-129) 与 `deckAdvance` (178-194)、`deckGoNext` (198-205)。
  - 高度测量 `measureDeck` (138-158) 与 `useLayoutEffect` 重测 (162-168)。
  - 渲染循环 `Array.from({ length: STEP_COUNT })` (234-599)，`inert` 防聚焦 (247)。
  - 步骤内容：欢迎(279-302)、账号(304-367)、绑定运营商(369-501)、完成(503-537)。
  - 底部导航网格响应式三列/窄屏两行 (544-595)。
  - 关闭确认嵌套 Dialog (605-626)。

## Architecture

```text
┌───────────────────────────────────────────────────────────┐
│ main.tsx                                                   │
│  初始化主题/GSAP/崩溃恢复 (27-116)                          │
│  ReactDOM.createRoot -> <App /> (135-145)                  │
└───────────────────────────────────────────────────────────┘
                           │
                           ▼
┌───────────────────────────────────────────────────────────┐
│ App.tsx :: App (363-375)                                   │
│   useFormFactor() ───────┬──────────────┐                  │
│                        phone           tablet              │
│                          ▼              ▼                  │
│                   AppInner (52-359)  TabletShell           │
│                   ├ BottomNav (37-151)  ├ DockNav (176-295)│
│                   ├ MobileDashboard     ├ RightPanel       │
│                   │ (141-168)           │ (54-279)         │
│                   ├ MobileMore (33-84)  └ 桌面件复用        │
│                   └ 弹窗/向导                            │
│   共享: FaceCaptureDialog, NotificationPermissionGate       │
│   共享向导: OnboardingWizard (107-628, 响应式 Dialog)       │
└───────────────────────────────────────────────────────────┘
```

## Data Flow

1. **初始化与形态判定**：`main.tsx` 执行 `initTheme` (27-44) 与 `setupCrashRecovery` (48-116)；`App.tsx` 的 `App` 组件 (363-375) 调用 `useFormFactor()` (`useFormFactor.ts|38-40`) 获取 `'phone'` 或 `'tablet'`。
2. **外壳分发**：`'tablet'` 渲染 `TabletShell`（平板 Dock 布局），`'phone'` 渲染 `AppInner` (52-359)。
3. **移动端导航同步**：`BottomNav` (37-151) 触发 `handleTabChange` (`App.tsx|119-127`) 更新 `tab` 状态并写入 `safeStorage`；`deferredTab` 驱动 `AnimatePresence` 面板切换 (`App.tsx|148-192, 281-295`)。
4. **平板端导航同步**：`DockNav` (176-295) 的 `DockItem` 点击调用 `onPanelChange` 更新 `useAdapterStore.activePanel`，`RightPanel` (54-279) 常驻展示日志。
5. **引导流**：首次启动无账号时，`AppInner` 的 effect (`App.tsx|138-146`) 打开 `OnboardingWizard` (`App.tsx|348-356`)，其内通过 `useOnboardingFlow` 管理步骤，CSS 响应式适配手机/平板。
6. **响应式重算**：旋转或分屏触发 `resize`/`orientationchange` (`useFormFactor.ts|28-33`)，`useFormFactor` 重算并触发 React 重渲染切换外壳。

## Connections

- [[responsive-design]]：本概念是实现响应式设计的具体手段。
- [[form-factor]]：`useFormFactor` Hook 是本概念的核心依赖。
- [[mobile-first]]：移动端布局 (`AppInner`) 是基础实现，平板端在此基础上扩展。
- [[tablet-adaptation]]：平板端布局 (`TabletShell`/`DockNav`) 是对移动端布局的增强。

## Known Issues

- **日志清空动画卡死风险**：安卓 WebView 后台时 rAF 停摆，GSAP `onComplete` 不触发会导致 `isClearing` 永久 true；已用 `setTimeout` 兜底 (`RightPanel.tsx|108-117`)。
- **GPU 崩溃检测局限**：历史缺陷记录，GPU 崩溃时 JS 存活导致 interval 心跳正常，`isRenderLoopAlive()` 用于弥补 (`main.tsx|95-98`)。
- **废弃文档引用**：旧文章引用的 `android/frontend/src/settings/OnboardingWizardMobile.tsx` 在当前仓库中不存在/读取失败；手机端向导已整合进 `OnboardingWizard.tsx` 的响应式布局（见 `OnboardingWizard.tsx|544-576`），本文已更正。
