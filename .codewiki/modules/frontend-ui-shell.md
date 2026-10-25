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

- **应用入口**：`main.tsx` 处理 React 根节点挂载、framer-motion 懒加载配置、GSAP 全局配置及 GPU/WebGL 崩溃恢复逻辑，并挂载安卓内存压力钩子。
- **路由与状态管理**：`App.tsx` 作为双壳入口，通过 `useFormFactor` 钩子判断屏幕宽度（sw600dp 语义），决定渲染 `TabletShell`（桌面 Dock 布局）还是 `AppInner`（移动端底部导航布局）。
- **布局组件**：提供 `BottomNav`（手机底栏）、`DockNav`（平板/桌面 Dock）、`RightPanel`（右侧日志面板）、`TitleBar`（桌面标题栏）等共享布局件。
- **移动专属视图**：`MobileDashboard` 通过 `DashboardPanel` 注入移动状态卡与监控卡，复用桌面编辑体系。

## Key Components

### Entry Point & Initialization

#### `android/frontend/src/main.tsx`

- **`initTheme()` (L27-L44)**: 读取 `safeStorage` 中的 `campus-light-mode`、`campus-theme`、`campus-glass-level`，操作 `document.documentElement` 的 class 与 `data-light`/`data-glass` 属性以应用深色模式、主题色与玻璃档位，避免启动闪烁。
- **`setupCrashRecovery()` (L48-L115)**:
  - `tryRecover` (L52-L60): 崩溃计数与重载逻辑，最多 `MAX_CRASH_RELOADS=3` 次。
  - `visibilitychange` 监听 (L62-L69): 页面可见时若 `#root` 为空触发恢复。
  - `window.error` 监听 (L71-L77): 捕获含 "GPU"/"WebGL"/"SharedArrayBuffer" 错误并恢复。
  - 第二个 `visibilitychange` 监听 (L82-L91): 可见时恢复 `gsap.globalTimeline`，不可见时暂停。
  - 心跳 `setInterval` (L93-L103): 每 1s 若 `isRenderLoopAlive()` 正常则更新 `lastHeartbeatTime`。
  - 判定 `setInterval` (L105-L113): 每 2s 检查心跳间隔，超过 10s 判定渲染链失活触发恢复。
- **`window.__onTrimMemory` (L123-L129)**: 安卓内存压力钩子（Kotlin 侧调用），高压力级别下若人脸引擎已加载且非会话中，动态 `import('./face/faceService').then(m => m.releaseFaceEngine())` 释放缓存。
- **`AppWrapper` (L131-L133)**: 开发环境为 `React.StrictMode`，生产为 `React.Fragment`。
- **`ReactDOM.createRoot` (L135-L145)**: 挂载根节点，包裹 `ErrorBoundary` + `LazyMotion(features={domMax} strict)` + `MotionConfig(reducedMotion="user")` 后渲染 `<App />`。

#### `android/frontend/src/App.tsx`

- **懒加载模块 (L47-L50)**: `AboutDialog`(来自 `@/auth/AboutDialogMobile`)、`MobileQuickActions`、`ThemeDialog`、`OnboardingWizard` 均通过 `lazy()` 动态导入。
- **`AppInner` (L52-L359)**: 移动端主视图容器。
  - **状态与 Hook**:
    - `tab` (L57-L62): `MobileTab` 类型，初始从 `safeStorage.get('campus-mobile-tab')` 读取并白名单校验（含 `monitor`），默认 `'dashboard'`。
    - `deferredTab` (L63): `useDeferredValue(tab)` 降低渲染优先级。
    - `tabOrder` (L70-L76): 底栏顺序数组，第 4 位根据 `configEnableNetworkQuality !== false` 在 `'quality'`/`'monitor'` 间切换。
    - `navDir` (L77): 面板转场方向 `-1|0|1`。
    - `panelVariants` (L115-L117): 由 `createPanelAppleVariants(easing)` 生成。
  - **`handleTabChange` (L119-L127)**: 标记交互、计算 `navDir`、更新 `tab` 并持久化到 `safeStorage`。
  - **配置归位 Effect (L131-L136)**: 配置加载后若质量开启且当前为 `monitor` 则切回 `quality`（KI#14 配套）。
  - **新手引导 Effect (L138-L146)**: 无 `campus-onboarding-done` 且无账号时打开 `OnboardingWizard`。
  - **面板内容 switch (L148-L192)**: 依据 `deferredTab` 渲染 `MobileDashboard`/`AccountPanel`/`SelfServicePanel`/`QualityPanel`/`MonitorPanel`/`MobileMore`。
  - **顶部轻 Header (L202-L260)**: 绝对定位 `backdrop-blur-md`，含状态点 (L206-L213)、在线运营商 chip (L215-L223)、更新胶囊 (L225-L235)、网络质量胶囊按钮 (L237-L246)、赞助/设置/主题/关于图标按钮 (L248-L259)。
  - **主滚动区 (L270-L297)**: `paddingTop` 计算避开 header，`AnimatePresence mode="wait" custom={navDir}` 内 `m.div` 使用 `panelVariants` 实现方向转场。
  - **底部与弹层**: `BottomNav` (L299)、`ToastContainer` (L301)、`SponsorCard` (L303)、`AboutDialog` (L305-L316)、`UpdateAvailableDialog` (L318)、`ThemeDialog` (L320-L336)、`ConfirmDialog` 删除确认 (L338-L345)、`OnboardingWizard` (L347-L356)。
- **`App` (L363-L375)**: 默认导出。调用 `useFormFactor()`，在 `ErrorBoundary`+`AnimationActiveProvider` 内根据 `formFactor==='tablet'` 渲染 `<TabletShell />` 否则 `<AppInner />`；公共挂载 `<FaceCaptureDialog />` 与 `<NotificationPermissionGate />`。
- **`NotificationPermissionGate` (L381-L405)**: 首次启动（无 `campus-notification-prompt` 标记）弹确认框引导通知权限，确认或取消均写标记并调用 `requestNotificationPermission`。

### Layout Shells

#### `android/frontend/src/components/layout/BottomNav.tsx`

- **`MobileTab` 类型 (L18)**: `'dashboard' | 'account' | 'selfservice' | 'quality' | 'monitor' | 'more'`。
- **`TABS` 常量 (L20-L25)**: 默认四项 `[dashboard, account, selfservice, more]` 含 `labelKey` 与 `Icon`。
- **`QUALITY_TAB` / `MONITOR_TAB` (L27-L28)**: 动态插入项定义。
- **`useNavTabs()` (L30-L35)**: 根据 `useConfigStore` 中 `config.enableNetworkQuality !== false` 在索引 3 处插入 `QUALITY_TAB` 或 `MONITOR_TAB`。
- **`BottomNav` 组件 (L37-L152)**:
  - Props (L37-L40): `{ tab: MobileTab; onChange: (t: MobileTab) => void }`。
  - 内部使用 `useGlassRefraction(navRef, glassLevel)` (L46)、`useReducedMotion`、`useDockLens` (L60-L65) 实现镜片放大系统。
  - 渲染结构 (L67-L151): 绝对定位 `nav` → `glass-nav` 容器（含 `data-dock-lens` 底 (L86-L94)、`data-dock-strip` 原项 (L99-L126)、`data-dock-magnify` 放大层副本 (L130-L148)）。

#### `android/frontend/src/components/layout/DockNav.tsx`

- **`ICON_MAP` (L30-L39)**: 名称到 lucide 图标的映射。
- **`DockItem` (L41-L85)**:
  - Props (L41-L49): `{ id: PanelName; label: string; icon: string; isActive: boolean; ghost?: boolean; onPanelChange?: (id: PanelName) => void }`。
  - 激活项显示图标+标签，非激活仅图标；含 hover tooltip (L75-L82)。
- **`ActionButton` (L87-L169)**:
  - Props (L88-L104): `{ label, loadingLabel, icon, isLoading, isDisabled, onAction, variant: 'primary'|'outline' }`。
  - 加载时 GSAP 旋转 spinner (L114-L122) 与脉冲边框 `loadingPulseRef` (L107)；`whileTap` 缩放 (L134)。
- **`DockNavProps` 接口 (L171-L174)**: `{ onPanelChange: (panel: PanelName) => void; outerRef?: (el: HTMLDivElement|null)=>void }`。
- **`DockNav` (L176-L295)**:
  - `visibleItems` (L185): `NAV_ITEMS` 过滤掉 `quality`（当 `enableNetworkQuality` 为 false）。
  - 玻璃与镜片 (L189-L209): `useGlassRefraction` 与 `useDockLens`（economy 档或 reducedMotion 时禁用）。
  - 渲染固定底部中央 `nav.glass-dock` (L217-L292)，含 `DockItem` 列表、放大层、分隔线及两个 `ActionButton`（注销/登录）。

#### `android/frontend/src/components/layout/RightPanel.tsx`

- **`RightPanelProps` 接口 (L14-L18)**: `{ logs: LogEntry[]; onClearLogs?: () => void; outerRef?: (el: HTMLDivElement|null)=>void }`。
- **日志样式映射 (L20-L46)**: `LOG_ICONS`、`LOG_COLORS`、`LOG_BG_COLORS`、`LOG_BAR_COLORS` 按 `LogEntry['type']` 分类。
- **性能常量 (L51-L52)**: `RIGHT_PANEL_ANIM_THRESHOLD=50`、`RIGHT_PANEL_ANIM_KEEP_COUNT=30`（FP-4 策略）。
- **`RightPanel` (L54-L279)**:
  - `handleClearWithAnimation` (L64-L125): GSAP stagger 逐条退出，动态间隔；`onInterrupt` 复位 (L99-L105)；`setTimeout` failsafe (L112-L117) 防 rAF 停摆死锁。
  - 自动滚动 (L127-L138): `isAutoScrollRef` 跟踪是否贴近底部。
  - 分片渲染 (L148-L157): `logs.length>50` 时前段 `staticLogs` 用普通 `div`，最近 30 条 `animLogs` 用 `m.div`+`AnimatePresence`。
  - JSX (L159-L278): `AnimatedCard` 包裹头部计数与清空按钮、看板娘水印图、滚动日志列表。

#### `android/frontend/src/components/layout/TitleBar.tsx`

- **`TitleBarProps` 接口 (L11-L24)**: 含 `notificationEnabled, onToggleNotification, onShowTheme, onShowAbout, onShowSponsor, onToggleLightMode, onMinimize, onToggleMaximize, onClose, isMaximized, showWindowControls?`（无窗口管理平台传 false 隐藏三键）。
- **图标组件 (L26-L50)**: `MinimizeIcon`/`MaximizeIcon`/`RestoreIcon`/`CloseIcon` 内联 SVG。
- **`TitleBar` (L52-L278)**:
  - `handleTitleBarMouseDown` (L73-L85): 非按钮区左键拖拽 `getCurrentWindow().startDragging()`（需 `showWindowControls`）。
  - `handleTitleBarDoubleClick` (L87-L90): 双击最大化切换。
  - 渲染 (L92-L276): 左侧 mascot+应用名+版本/更新提示；右侧 tooltip 按钮组（亮暗、语言、通知、主题、赞助、关于）及条件渲染的窗口控制三键。

### Mobile Specific Views

#### `android/frontend/src/components/mobile/MobileDashboard.tsx`

- **`LampState` 类型与 `LAMP_COLOR` (L22-L29)**: `'online'|'offline'|'loading'|'error'` 及对应颜色类。
- **`noopAsync` (L32)**: 异步空函数，供排除的 quickActions 占位。
- **`EXCLUDED_CARDS` (L35)**: 常量 `['quickActions']` 排除桌面 DHCP 续租卡。
- **`MobileStatusCard` (L38-L96)**: 标准 `AnimatedCard` 展示连接灯、Portal 可达性、检查次数、运营商（仅手机形态 `formFactor==='phone'` 显示，L81）。
- **`MobileMonitorCard` (L99-L139)**: 标准卡 + Radix `Switch` 控制后台检测，按钮手动刷新；间隔从 `config.backgroundCheckInterval` 读取 (L105)。
- **`MobileDashboard` (L141-L168)**: 通过 `useMemo` 稳定 `extraCards` 注入 `DashboardPanel`，传入 `excludeCards={EXCLUDED_CARDS}` 与 `mergeSelfServiceEmpty`。

## Architecture

```text
┌─────────────────────────┐
│ main.tsx (L135-L145)    │
│ ReactDOM.createRoot     │
└───────────┬─────────────┘
            ▼
┌─────────────────────────┐
│ App (App.tsx L363-L375) │
│ useFormFactor()         │
└───────────┬─────────────┘
      ┌──────┴──────┐
      ▼             ▼
┌──────────────┐ ┌──────────────────────┐
│ AppInner     │ │ TabletShell (external)│
│ (L52-L359)   │ │ 桌面 Dock 布局         │
└──────┬───────┘ └───┬──────────┬────────┘
       ▼             ▼          ▼
┌──────────────┐ ┌──────────┐ ┌──────────┐
│ BottomNav    │ │ DockNav  │ │ TitleBar │
│ (L37-L152)   │ │(L176-295)│ │(L52-278) │
└──────┬───────┘ └────┬─────┘ └────┬─────┘
       ▼             ▼          ▼
┌──────────────┐ ┌──────────┐ ┌──────────┐
│ Header+Main  │ │RightPanel│ │窗口控制   │
│ (L202-L297)  │ │(L54-279) │ │          │
└──────┬───────┘ └──────────┘ └──────────┘
       ▼
┌──────────────────────┐
│ MobileDashboard      │
│ (L141-L168)          │
│  └─ DashboardPanel   │
└──────────────────────┘
```

## Data Flow

- **主题与玻璃初始化**：`main.tsx` 的 `initTheme()` (L27-L44) 在首屏前从 `safeStorage` 读取并写 DOM 属性；运行时 `ThemeDialog` (App.tsx L320-L336) 通过 `useThemeStore` 与 `safeStorage` 更新。
- **页签状态持久化**：`AppInner` 的 `tab` 状态在 `handleTabChange` (L119-L127) 写入 `campus-mobile-tab`；重启时读回并白名单校验 (L57-L62)。
- **配置驱动布局**：`useConfigStore.config.enableNetworkQuality` 同时决定 `BottomNav` 的 `useNavTabs` (BottomNav L31) 与 `DockNav` 的 `visibleItems` (DockNav L181, L185) 插入/隐藏 quality 页签。
- **全局 Store 注入**：`useAuthStore`(状态/运营商)、`useQualityStore`(更新/日志)、`useLogToastStore`(toast) 等通过 selector 将字段传入 `AppInner`、`RightPanel`、`TitleBar` 等。
- **崩溃恢复信号**：`setupCrashRecovery` 依赖 `isRenderLoopAlive()` (main.tsx L98) 与 `performance.now()` 差值判定 GPU 失活；`visibilitychange` 联动 `gsap.globalTimeline` 暂停/恢复。

## Connections

- 共享 UI 与类型来自 [[frontend-shared]]（`@/shared` 的 `ErrorBoundary`、`IspMark`、`SponsorCard`、`LogEntry` 等）。
- 全局状态钩子依赖 [[frontend-hooks]]（`@/hooks/useConfigStore`、`useAuthStore`、`useQualityStore`、`useThemeStore`、`useFormFactor`、`useAppInit` 等）。
- 动画工具与配置见 [[frontend-lib]]（`@/lib/animations`、`@/lib/glass-dock`、`@/lib/dock-lens`、`@/lib/utils`）。
- 常量与设置来自 [[frontend-settings]]（`@/settings/constants` 的 `VALID_THEMES`、`operatorLabelKey` 等）。
- 监控与账号面板连接 [[frontend-monitor]]（`@/monitor/useMonitor`、`QualityPanel`、`MonitorPanel`）与 [[frontend-account]]（`@/account/useAccount`、`AccountPanel`）。
- 人脸认证弹窗共用 [[frontend-face]]（`@/face/FaceCaptureDialog`、`faceService`）。
- 通用组件库 [[frontend-components]]（`@/components/ui/*`、`@/components/layout/*`、`@/components/mobile/*`、`@/components/tablet/TabletShell`）。

## Known Issues

- **GPU 崩溃检测历史缺陷** (`main.tsx` L95-L101): 旧逻辑仅靠 interval 心跳，GPU 崩溃但 JS 存活时 rAF 停滞 interval 仍跳动，检测不到；已改用 `isRenderLoopAlive()` 跳过心跳更新。
- **误判阈值** (`main.tsx` L108-L109): 原 5s 阈值在低端机长 GC 阻塞会误判，FE-A-11 放宽到 10s。
- **页签白名单遗漏** (`App.tsx` L59-L61, KI#14): `campus-mobile-tab` 白名单须覆盖 `handleTabChange` 可写全部页签（含 `monitor`），否则后台杀进程重开回落 `dashboard`。
- **质量开关页签归位** (`App.tsx` L129-L136): 质量开启后 `monitor` 不再是底栏页签，跨会话恢复出的 `monitor` 使底栏无高亮，配置就绪后归位 `quality`。
- **原新手引导失效** (`App.tsx` L138-L146): 旧实现直奔账号页并写 `campus-onboarding-done`，等于无引导；现改为向导内写标记。
- **日志清空死锁** (`RightPanel.tsx` L96-L105): GSAP tween 被中断时 `onComplete` 不执行，`isClearing` 永久 true；已加 `onInterrupt` 复位。
- **WebView 后台 rAF 冻结** (`RightPanel.tsx` L108-L111): 安卓 WebView 后台时 rAF 停摆，GSAP 回调不触发导致清空卡死；用 `setTimeout` 兜底 (L112-L117)。
- **日志动画降级** (`RightPanel.tsx` L48-L51, FP-4): 超过 50 条时旧条目用普通 `div`，避免 300 条 reconciliation 成本。
- **监控间隔覆盖 bug** (`MobileDashboard.tsx` L104-L105): 硬编码旧默认 15s 会覆盖用户自定义间隔，已改为读 `config.backgroundCheckInterval` 并 `Math.max(5, ...)`。
- **内存释放前置条件** (`main.tsx` L118-L125): `__onTrimMemory` 必须检查 `__faceEngineLoaded` 与 `__faceSessionActive`，否则白拉 chunk 或释放进行中实例导致轮询异常。
