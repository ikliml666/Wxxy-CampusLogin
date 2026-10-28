---
title: app_root
type: module
source_files:
  - tauri-app/frontend/src/App.tsx
  - tauri-app/frontend/src/main.tsx
  - tauri-app/frontend/src/hooks/useAppInit.ts
  - tauri-app/frontend/src/hooks/useAdapterStore.ts
  - tauri-app/frontend/src/hooks/useConfigStore.ts
  - tauri-app/frontend/src/hooks/useLogToastStore.ts
  - tauri-app/frontend/src/hooks/useQualityStore.ts
  - tauri-app/frontend/src/hooks/useThemeStore.ts
  - tauri-app/frontend/src/hooks/useAuthStore.ts
  - tauri-app/frontend/src/hooks/useInitialDataLoad.ts
  - tauri-app/frontend/src/hooks/useEventListeners.ts
  - tauri-app/frontend/src/hooks/useHeartbeat.ts
  - tauri-app/frontend/src/hooks/useGlobalShortcut.ts
  - tauri-app/frontend/src/hooks/useStartupBoost.ts
  - tauri-app/frontend/src/hooks/usePageIdle.ts
  - tauri-app/frontend/src/hooks/useAnimationProfile.ts
  - tauri-app/frontend/src/auth/useAuth.ts
  - tauri-app/frontend/src/settings/useSettings.ts
  - tauri-app/frontend/src/monitor/useMonitor.ts
  - tauri-app/frontend/src/network/useNetwork.ts
  - tauri-app/frontend/src/account/useAccount.ts
  - tauri-app/frontend/src/components/layout/TitleBar.tsx
  - tauri-app/frontend/src/components/layout/DockNav.tsx
  - tauri-app/frontend/src/components/layout/RightPanel.tsx
  - tauri-app/frontend/src/monitor/StatusBar.tsx
  - tauri-app/frontend/src/shared/ConfirmDialog.tsx
  - tauri-app/frontend/src/shared/SponsorCard.tsx
  - tauri-app/frontend/src/shared/ToastContainer.tsx
  - tauri-app/frontend/src/shared/ErrorBoundary.tsx
  - tauri-app/frontend/src/shared/FluidBackground.tsx
  - tauri-app/frontend/src/shared/index.ts
  - tauri-app/frontend/src/shared/ui-constants.ts
  - tauri-app/frontend/src/shared/ui-types.ts
  - tauri-app/frontend/src/shared/LogPanel.tsx
  - tauri-app/frontend/src/auth/AboutDialog.tsx
  - tauri-app/frontend/src/settings/ThemeDialog.tsx
  - tauri-app/frontend/src/settings/OnboardingWizard.tsx
tags: [app-root, global-state, dialogs, shell, zustand, tauri]
---

## Overview

`app_root` 是前端单页应用的根聚合层：它把「全局 Zustand 状态」「领域动作编排 hooks」「平板外壳（标题栏 / 状态栏 / 侧栏 / Dock 导航）」「对话框与浮层（关于 / 主题 / 引导 / 确认删除 / 赞助 / Toast）」全部铆接在一起。

入口链为 `main.tsx` → `App`（默认导出）→ `AppInner`（实际渲染体）。

- `main.tsx`（第 1–133 行）在挂载 React 之前完成主题初值落定（`initTheme`，第 27–44 行）、GPU 崩溃自愈（`setupCrashRecovery`，第 48–114 行），再用 `ErrorBoundary` + `LazyMotion(domMax)` + `MotionConfig(reducedMotion="user")` 包裹 `<App/>`（第 122–132 行）。注意 DEV 模式用 `React.StrictMode`、生产用 `React.Fragment`（第 118–120 行）——这影响 `useInitialDataLoad` 里 effect 的 setup→cleanup→setup 行为。
- `App.tsx` 的默认导出 `App`（第 580–588 行）再套一层 `ErrorBoundary` 与 `AnimationActiveProvider`（来自 `usePageIdle.ts`）。
- `AppInner`（第 114–578 行）是核心聚合体：调用 `useAppInit`（启动时的事件/数据/心跳/快捷键装配）、从各 store 粒度订阅所需字段、从领域 hooks（`useAuth`/`useMonitor`/`useNetwork`/`useAccount`/`useSettings`）取得动作回调，渲染外壳并切换面板。

面板的切换与渲染是 `AppInner` 的关键职责：
- `activePanel` 来自 `useAdapterStore`（第 130 行），经 `useDeferredValue` 得到 `deferredPanel`（第 133 行），使快速连点时 React 并发跳过中间面板的 mount（注释见第 131–133 行）。
- `PANEL_TITLES`（第 64–74 行）定义 9 个面板的中文/英文标题与描述 i18n key。
- 第 334–419 行的 `switch (deferredPanel)` 把每个面板组件实例化并传入 props。
- 三个低频对话框（`AboutDialog`/`ThemeDialog`/`OnboardingWizard`）用 `lazy` 分包（第 50–56 行），并在启动动画开始后并行预加载（`preloadPanels`，第 59–62 行；由 `useEffect` 在 `requestAnimationFrame` 内触发，第 199–209 行）。

应用常驻状态管理的关键约定（贯穿多个文件）：`App.tsx` 不注册 `onCloseRequested`，窗口关闭的 config flush 由关闭发起方就地完成（见 `handleCloseWindow` 第 295–314 行 与 `useEventListeners.ts` 第 49–57 行 注释），窗口销毁控制权回归 Rust。

## Key Components

### 根组件与入口

- **`App`**（默认导出，`App.tsx` 第 580–588 行）：组合 `ErrorBoundary` + `AnimationActiveProvider` + `AppInner` 三层包裹组件。
- **`AppInner`**（`App.tsx` 第 114–578 行）：核心聚合函数组件。关键导出/常量：
  - `loadAboutDialog` / `loadThemeDialog` / `loadOnboardingWizard`（第 50–52 行）：返回 `import().then(m => ({ default }))` 的 loader，供 `lazy` 与 `preloadPanels` 复用。
  - `PANEL_TITLES`（第 64–74 行）：`Record<string, { titleKey; descKey }>`，面板标题映射。
  - `SPONSOR_SHOW_DELAY_MS = 1000` / `SPONSOR_SHOW_INTERVAL_MS = 7*24*3600*1000`（第 77–78 行）：赞助浮层延迟与 7 天频控。
  - `PANEL_CONTAINER_STYLE`（第 82 行）：面板容器 `contain: 'layout style'` + `willChange/translateZ`，避免编辑模式删除按钮被裁剪。
  - `SIDE_MASCOT_POOL` / `pickDailySideMascots(dayKey)`（第 85–99 行）：侧边看板娘每日种子随机，跨天轮换。
  - `PanelSkeleton`（第 102–112 行）：面板 chunk 加载中的轻量骨架。
  - `handlePanelChange`（第 320–329 行）：带 60ms 互斥锁（`panelChangeLock`）的面板切换，写 `safeStorage.set('campus-active-panel', p)`（第 324 行）。
  - `handleCloseWindow`（第 295–314 行）：关窗前 `flushPendingConfig()` 最多等 2s，再 `api.closeWindow?.()`。
  - `handleToggleMaximize`（第 279–287 行）、`handleClearLogs`（第 316–318 行）。
- **`initTheme`**（`main.tsx` 第 27–44 行）：首帧前落定 `dark`/`data-light` class、主题 class、`data-glass` 档位。
- **`setupCrashRecovery`**（`main.tsx` 第 48–114 行）：GPU/WebGL 崩溃检测 + 渲染心跳（rAF 停滞 10s 判超时）自动重载，上限 3 次（第 50 行 `MAX_CRASH_RELOADS`）。

### 全局状态（Zustand Stores）

- **`useAdapterStore`**（`useAdapterStore.ts` 第 45–71 行）：适配器列表与激活面板。
  - 模块级 `refreshAdapterData(options)`（第 16–32 行）：并行拉 `getAdapters`/`getAdapterDetails` 后写回 store。
  - 接口 `AdapterStore`（第 34–43 行）：`adapters`、`adapterDetails`、`isRefreshingAdapters`、`activePanel`、`refreshAdapters`、`setAdapters`、`setAdapterDetails`、`setActivePanel`。
  - `_adapterLockFlag`（第 9 行）：刷新互斥锁；`refreshAdapters`（第 51–66 行）保证锁最短保持 500ms 且工作完成后才释放。
- **`useConfigStore`**（`useConfigStore.ts` 第 51–178 行）：配置对象、账号、语言、密码保存态。
  - 接口 `ConfigStore`（第 27–49 行）：`config`、`configLoaded`、`passwordSaved`、`initDebugMode?`、`initLogRetentionDays?`、`accounts`、`activeAccount`、`language`、`updateConfig`、`updateConfigLocal`、`mergeConfigFromBackend`、`clearDirtyFields`、`syncPasswordSaved`、`saveConfigDirect`、`setAccounts`、`setActiveAccount`、`setLanguage`。
  - 模块级：`saveConfigTimer`/`saveConfigPending`/`saveConfigInFlight`（第 14–17 行，500ms 防抖保存）、`dirtyFields` + `DIRTY_FAILURE_LIMIT=3`（第 20–25 行，脏字段跳过后端回传与失败计数）、`hasPendingConfig()`（第 180–185 行）、`flushPendingConfig()`（第 187–216 行）。
- **`useLogToastStore`**（`useLogToastStore.ts` 第 28–136 行）：日志与 Toast。
  - 接口 `LogToastStore`（第 15–26 行）：`logs`、`toasts`、`addLog`、`addToast`、`addToastWithAction`、`removeToast`、`removeToastsByPrefix`、`setLogs`、`cleanupToasts`。
  - 常量 `MAX_TOASTS = 4`（第 9 行）、`MAX_LOG_ENTRIES` 来自 `ui-constants`、`toastTimers`/`logIdCounter`/`toastIdCounter`（第 5–7 行）。`addLog` 连续重复折叠（第 39–51 行）；`addToast` 同题去重、非前台跳过（第 53–83 行）。
- **`useQualityStore`**（`useQualityStore.ts` 第 40–84 行）：网络质量、DNS DoH、版本更新。
  - 接口 `QualityStore`（第 22–38 行）：`networkQuality`、`dnsDohStatus`、`dnsChecking`、`isRefreshingQuality`、`updateAvailable`、`latestVersion`、`releaseNotes`、`refreshRate`、`refreshQuality`、`setNetworkQuality`、`setDnsDohStatus`、`setDnsChecking`、`setUpdateAvailable`、`setLatestVersion`、`setReleaseNotes`。
  - 模块级 `lastQualityResultTime` + `getLastQualityResultTime()`（第 16–20 行，登录后探测节流判定）；`_qualityLockFlag`（第 11 行）。
- **`useThemeStore`**（`useThemeStore.ts` 第 21–100 行）：主题名、浅色模式、自定义色、玻璃档位 + DOM 副作用。
  - 接口 `ThemeStore`（第 9–19 行）：`themeName`、`isLightMode`、`customThemeColor`、`glassLevel`、`setThemeName`、`setIsLightMode`、`setGlassLevel`、`initTheme`、`setCustomThemeColor`。
  - 模块级 `subscribe`（第 62–100 行）：监听四字段变化切 DOM class / CSS 变量，自定义主题用 `setProperty` 覆盖（修复 `cssText +=` 累积缺陷，第 84–87 行）。
- **`useAuthStore`**（`useAuthStore.ts` 第 149–319 行）：登录/注销、在线状态、后台状态。
  - 接口 `AuthStore`（第 137–147 行）：`isLoggingIn`、`isLoggingOut`、`status`、`bgStatus`、`doLogin`、`doLogout`、`checkOnline`、`setStatus`、`setBgStatus`。
  - 模块级 `withTimeout`（第 118–127 行，登录/注销 60s 超时包装，修复 P2-34）、`setStatusStable`（第 131–135 行，引用稳定不触发无效重渲染）、`_checkOnlineLockFlag`（第 19 行）、`QUALITY_MANUAL_THROTTLE_MS=60000`（第 24 行）。
- **`useAnimationProfile`**（`useAnimationProfile.ts` 第 51–57 行）：返回恒定 `HIGH_PROFILE`（第 31–49 行，已删除 standard/economy 分档），`refreshRate` 驱动 easing 曲线。
- **`AnimationActiveProvider` / `useAnimationActive`**（`usePageIdle.ts` 第 85–97 行）：应用顶层仅一次组合可见性/聚焦/空闲状态，`createElement` 注入 context（`.ts` 不能用 JSX）。

### 行为聚合 Hooks（领域动作编排）

### 行为聚合 Hooks（领域动作编排）

- **`useAppInit`**（`useAppInit.ts` 第 6–11 行）：聚合 `useEventListeners` + `useInitialDataLoad` + `useHeartbeat` + `useGlobalShortcut` 四个启动副作用，被 `AppInner` 首行调用（第 115 行）。
- **`useInitialDataLoad`**（`useInitialDataLoad.ts` 第 18–167 行）：异步拉 `getInitData`（第 32 行），落配置（`useConfigStore.setState`，第 41–47 行）、初始化主题（`useThemeStore.initTheme`，第 49 行）、恢复激活面板（第 55–60 行，质量检测禁用时跳过 `quality`）、显示窗口（第 64–66 行，静默启动则隐藏）、填适配器/背景状态/账号（第 68–101 行）、`checkOnline`（第 103 行）、DNS DoH 状态（第 109–137 行）、最后置 `configLoaded: true`（第 152 行）。失败降级 DEFAULT_CONFIG（第 160 行）。
- **`useEventListeners`**（`useEventListeners.ts` 第 18–384 行）：注册后端事件回调（`onBackgroundCheckResult`/`onAutoLoginResult`/`onAdaptersChanged`/`onAdapterDetailsChanged`/`onAdapterDisabledWarning`/`onLoginLog`/`onAutoExitCountdown`/`onNetworkQualityResult`/`onUpdateAvailable`/`onConfigChanged`/`onScheduledActionSkipped`/`onMacResetCycle` 等），统一写各 store。**明确不注册 `onCloseRequested`**（注释见第 49–57 行），仅用 `window blur` 兜底 flush（第 55–57 行）。
- **`useHeartbeat`**（`useHeartbeat.ts` 第 5–20 行）：每 5s 调 `api.renderHeartbeat?.()`，渲染链失活（`isRenderLoopAlive()`）时跳过，供后端按心跳丢失重载 WebView。
- **`useGlobalShortcut`**（`useGlobalShortcut.ts` 第 4–25 行）：Ctrl+Shift+C 取消自动退出，忽略可编辑元素目标。
- **`useStartupBoost`**（`useStartupBoost.ts` 第 15–143 行）：GSAP 启动入场时间线。`setRef(key)`（第 29–37 行）返回稳定 ref 回调给外壳挂载点（`titleBar`/`statusBar`/`title`/`dockNav`/`rightPanel`）；`runStartupSequence`（第 64–132 行）错峰入场，`dockNav` 仅位移不叠 opacity（防玻璃膜发黑，注释第 44–45、121–124 行）。
- **`useAuth`**（`useAuth.ts` 第 7–34 行）：暴露 `doLogin`/`doLogout`/`checkOnline`/`status` 与 `handleOpenPortal`/`handleOpenSelfService`（打开门户/自助服务 URL）。
- **`useMonitor`**（`useMonitor.ts` 第 11–105 行）：`handleToggleBackgroundCheck`/`handleTriggerCheck`/`handleToggleLatencyTest`，用 `bgToggleBusyRef`/`latencyToggleBusyRef`（第 35–36 行）防 in-flight 交错覆盖。
- **`useNetwork`**（`useNetwork.ts` 第 39–100 行）：`refreshAdapterInfo`/`handleDhcpRenew`/`handleDhcpReleaseRenew`/`handleDhcpReleaseRenewAdapter`；模块级 `normalizeDhcpResults`/`announceDhcpResults`（第 15–37 行）统一 DHCP 结果归一化与 toast 文案。
- **`useAccount`**（`useAccount.ts` 第 13–137 行）：`handleAddAccount`/`handleDeleteAccount`/`handleSwitchAccount`/`handleRenameAccount`；模块级 `switchInFlight`（第 11 行）防并发切换交错；删除/改名后应用后端回传的 `activeAccount`/`config`（修复 R4，第 70–91 行）。
- **`useSettings`**（`useSettings.ts` 第 11–70 行）：`handleToggleLightMode`/`handleToggleNotification`/`handleSetAutoLaunch`/`handleSetTheme`。

### 平板外壳组件

- **`TitleBar`**（`TitleBar.tsx` 第 50–271 行，`memo`）：标题栏，含应用名、版本、浅色/语言/通知/主题/赞助/关于/最小化/最大化/关闭按钮。鼠标拖拽移动窗口（`handleTitleBarMouseDown`，第 70–82 行）、双击最大化（第 84–86 行）。订阅 `useThemeStore.isLightMode`、`useQualityStore.updateAvailable/latestVersion`、`useConfigStore.language`。停靠点回调由 `AppInner` 注入：`notificationEnabled`、`onToggleNotification`、`onShowTheme`、`onShowAbout`、`onShowSponsor`、`onToggleLightMode`、`onMinimize`、`onToggleMaximize`、`onClose`、`isMaximized`（接口第 11–22 行）。赞助按钮带 `data-sponsor-anchor="true"`（第 206 行），供 `SponsorCard` 实测定位。
- **`StatusBar`**（`StatusBar.tsx` 第 26–268 行，`memo`）：状态条。订阅 `useAuthStore.status/bgStatus`、`useConfigStore` 三适配器字段 + `config.user/operator/nightOperatorRestore`、`useQualityStore` 质量相关。粒度订阅避免任意 config 变化触发整条重渲染（注释第 29–33 行）。`EMPTY_ADAPTER_STATUSES` 模块级常量（第 19 行）避免空数组新引用。渲染在线状态徽标、运营商胶囊、`NetworkQualityCapsule`、刷新/自助/门户按钮（接口 `StatusBarProps` 第 21–24 行：`onOpenPortal`/`onOpenSelfService`）。
- **`DockNav`**（`DockNav.tsx` 第 331–477 行，`memo`）：底部悬浮导航。订阅 `useAdapterStore.activePanel/adapters`、`useAuthStore.isLoggingIn/isLoggingOut`、`useConfigStore.enableNetworkQuality` 与 `adapter1/adapter2/dualAdapter`、玻璃档位。`NAV_ITEMS`（来自 `ui-constants`）过滤 `quality`（第 354 行）。含登录/注销 `ActionButtonWithMenu`（第 177–324 行，单卡直执行、双卡弹 `AdapterMenu` 选择），以及 `useGlassRefraction` + `useDockLens` 镜头/折射玻璃系统（第 358–379 行）。接口 `DockNavProps`（第 326–329 行）：`onPanelChange`/`outerRef`。
- **`RightPanel`**（`RightPanel.tsx` 第 82–432 行，`memo`）：右侧日志 + 适配器信息栏。订阅 `useAdapterStore.adapterDetails/adapters/isRefreshingAdapters/refreshAdapters`、`useConfigStore` 三适配器字段。日志分片动画（第 56–57 行 `RIGHT_PANEL_ANIM_THRESHOLD=50`/`KEEP_COUNT=30`，第 196–207 行），清空走 GSAP 一条条动画 + rAF 兜底（第 102–163 行）。`getAdapterInfo`（第 59–80 行）解析主/副适配器详情。接口 `RightPanelProps`（第 19–23 行）：`logs`/`onClearLogs`/`outerRef`。
- **`FluidBackground`**（`FluidBackground.tsx` 第 41–59 行）：CSS-only 慢速光斑，仅 `profile.willChangeOrbs`（high 档）时渲染（第 50 行）。
- **`ErrorBoundary`**（`ErrorBoundary.tsx` 第 15–62 行）：class 组件，捕获渲染错误，折叠原始报错到 `<details>`，提供重载按钮。

### 对话框与浮层

### 对话框与浮层

## Architecture

## Data Flow

## Connections

## Known Issues
