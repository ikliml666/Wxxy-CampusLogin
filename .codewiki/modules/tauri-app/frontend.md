---
title: tauri-app Frontend (React + TypeScript SPA)
type: module
source_files:
  - tauri-app/frontend/package.json
  - tauri-app/frontend/src/main.tsx
  - tauri-app/frontend/src/App.tsx
  - tauri-app/frontend/src/i18n/index.ts
  - tauri-app/frontend/src/hooks/tauriApi.ts
  - tauri-app/frontend/src/hooks/useStartupBoost.ts
  - tauri-app/frontend/src/hooks/useConfigStore.ts
  - tauri-app/frontend/src/hooks/useAppStore.ts
  - tauri-app/frontend/src/hooks/useAdapterStore.ts
  - tauri-app/frontend/src/hooks/useAuthStore.ts
  - tauri-app/frontend/src/hooks/useQualityStore.ts
  - tauri-app/frontend/src/hooks/useLogToastStore.ts
  - tauri-app/frontend/src/hooks/useThemeStore.ts
  - tauri-app/frontend/src/hooks/useAppInit.ts
  - tauri-app/frontend/src/hooks/useInitialDataLoad.ts
  - tauri-app/frontend/src/hooks/useEventListeners.ts
  - tauri-app/frontend/src/hooks/useHeartbeat.ts
  - tauri-app/frontend/src/hooks/useGlobalShortcut.ts
  - tauri-app/frontend/src/hooks/useAnimationProfile.ts
  - tauri-app/frontend/src/hooks/usePageIdle.ts
  - tauri-app/frontend/src/hooks/useStartupBoost.ts
  - tauri-app/frontend/src/auth/useAuth.ts
  - tauri-app/frontend/src/auth/DashboardPanel.tsx
  - tauri-app/frontend/src/monitor/useMonitor.ts
  - tauri-app/frontend/src/monitor/MonitorPanel.tsx
  - tauri-app/frontend/src/monitor/QualityPanel.tsx
  - tauri-app/frontend/src/monitor/SpeedTestPanel.tsx
  - tauri-app/frontend/src/monitor/StatusBar.tsx
  - tauri-app/frontend/src/network/useNetwork.ts
  - tauri-app/frontend/src/network/NetworkPanel.tsx
  - tauri-app/frontend/src/network/useDnsSetup.ts
  - tauri-app/frontend/src/network/useOutboundManual.ts
  - tauri-app/frontend/src/network/adapters.ts
  - tauri-app/frontend/src/network/constants.ts
  - tauri-app/frontend/src/account/useAccount.ts
  - tauri-app/frontend/src/account/AccountPanel.tsx
  - tauri-app/frontend/src/account/SelfServicePanel.tsx
  - tauri-app/frontend/src/account/selfServiceState.ts
  - tauri-app/frontend/src/account/useOperatorBind.ts
  - tauri-app/frontend/src/account/selfServiceTypes.ts
  - tauri-app/frontend/src/settings/useSettings.ts
  - tauri-app/frontend/src/settings/SettingsPanel.tsx
  - tauri-app/frontend/src/settings/constants.ts
  - tauri-app/frontend/src/components/layout/TitleBar.tsx
  - tauri-app/frontend/src/components/layout/DockNav.tsx
  - tauri-app/frontend/src/components/layout/RightPanel.tsx
  - tauri-app/frontend/src/shared/LogPanel.tsx
  - tauri-app/frontend/src/shared/ToastContainer.tsx
  - tauri-app/frontend/src/shared/ErrorBoundary.tsx
  - tauri-app/frontend/src/shared/ConfirmDialog.tsx
  - tauri-app/frontend/src/shared/FluidBackground.tsx
  - tauri-app/frontend/src/shared/SponsorCard.tsx
  - tauri-app/frontend/src/shared/AnimatedNumber.tsx
  - tauri-app/frontend/src/shared/SegmentTabs.tsx
  - tauri-app/frontend/src/shared/IspMark.tsx
  - tauri-app/frontend/src/shared/MascotFigure.tsx
  - tauri-app/frontend/src/shared/RefreshButton.tsx
  - tauri-app/frontend/src/shared/index.ts
  - tauri-app/frontend/src/shared/types.ts
  - tauri-app/frontend/src/shared/ui-types.ts
  - tauri-app/frontend/src/shared/ui-constants.ts
  - tauri-app/frontend/src/network/types.ts
  - tauri-app/frontend/src/monitor/types.ts
  - tauri-app/frontend/src/account/types.ts
  - tauri-app/frontend/src/settings/types.ts
  - tauri-app/frontend/src/auth/types.ts
  - tauri-app/frontend/src/lib/utils.ts
  - tauri-app/frontend/src/lib/latency.ts
  - tauri-app/frontend/src/lib/animations.ts
tags: [frontend, react, typescript, tauri-v2, zustand, tailwind, framer-motion, react-i18next, ipc]
---

## Overview

`tauri-app/frontend/` 是桌面端（Tauri v2）校园网登录助手的 React 19 + TypeScript 单页应用（SPA），版本号 `2.4.3`（`package.json:3`）。它通过 **Tauri v2 IPC**（`@tauri-apps/api/core` 的 `invoke` + `@tauri-apps/api/event` 的 `listen`）与 Rust 后端通信；状态管理全部基于 **Zustand v5** 的多个领域 store；样式用 **Tailwind CSS v3** + **framer-motion v12**（辅以少量 **GSAP v3** 做金刚玻璃/清空动画）；国际化用 **react-i18next v17**（`i18n/index.ts` 注册 `zh`/`en`，`fallbackLng: 'zh'`）。

应用入口链路：`main.tsx` 先初始化主题（`initTheme`）、崩溃恢复（`setupCrashRecovery`）与 GSAP 全局默认，再用 `LazyMotion+MotionConfig` 包裹 `<App/>`（`main.tsx:48`）。`App.tsx` 的 `<App/>` 外层再套一层 `<ErrorBoundary>`，内层用 `AnimationActiveProvider`（`usePageIdle.ts:85`）统一驱动「页面是否处于动画活跃态」。`AppInner`（`App.tsx:114`）是真正的应用根，负责启动时初始化（`useAppInit`）、订阅各 store、渲染 `TitleBar` / `StatusBar` / 主内容区 / `RightPanel` / `DockNav` / `ToastContainer` / 懒加载对话框（About / Theme / Onboarding）。

要点：

- **面板按业务域拆分而非 `src/panels/`**：题面提到的 `panels(Dashboard/Account/SelfService/Network/Monitor/Quality/SpeedTest/Settings)` 实际分散在 `auth/`、`account/`、`monitor/`、`network/`、`settings/` 五个业务目录里（仓库并不存在 `src/panels/` 目录）。`App.tsx` 通过 `switch (deferredPanel)`（`App.tsx:334`）把激活面板映射到具体组件。`NAV_ITEMS`（`shared/ui-constants.ts:7`）定义 9 个导航项，`quality` 面板在 `enableNetworkQuality === false` 时从 Dock 与 `visibleItems` 中隐藏（`DockNav.tsx:354`、`App.tsx:384`）。
- **常用面板静态导入、低频对话框懒加载**：`App.tsx:54-60` 直接静态导入 7 个面板（零切换等待）；`AboutDialog`/`ThemeDialog`/`OnboardingWizard` 仅在 `App.tsx:50-56` 用 `lazy()` 分包，并在启动后由 `preloadPanels`（`App.tsx:59`）并行预热（`App.tsx:55` 的 `useEffect`）。`LogPanel` 曾懒加载，实测切换延迟从 ~65ms 恶化到 ~366ms，**回归静态导入**（`App.tsx:46-49` 注释）。
- **配置持久化防抖 + 关闭窗口 flush**：`useConfigStore.updateConfig`（`useConfigStore.ts:59`）对每次字段变更做 500ms 防抖落盘；窗口关闭由 `App.handleCloseWindow`（`App.tsx:295`）就地 `flushPendingConfig()`（`useConfigStore.ts:187`），不走 JS 的 `onCloseRequested`（注释解释 Tauri core 一旦检测到该监听就 `prevent_close`，见 `useEventListeners.ts:50`）。
- **动画性能治理**：`useAnimationProfile.ts` 已固定为最高档 `HIGH_PROFILE`（`useAnimationProfile.ts:31`，standard/economy 分档于 v2.4.0 删除）；`usePageIdle.ts:83` 的 `AnimationActiveProvider` 把「可见 + 聚焦 + 未空闲」三态合并为单一 context，避免每处组件各自注册 8-9 个监听器（FP-2）；`RightPanel.tsx:56` 与 `LogPanel.tsx:83` 对超长日志列表做动画降级（仅最近 30/50 条走 framer-motion）。

## Key Components

### 应用外壳与初始化

- **`main.tsx`** — 入口。`initTheme()`（`:27`）在首帧前落定 `dark`/`data-light`/`data-glass`；`setupCrashRecovery()`（`:48`）监听 GPU/WebGL 错误、用 `isRenderLoopAlive()` 检测 rAF 停滞（>10s 视为崩溃）并自动 `location.reload()`（最多 3 次）。
- **`App.tsx:114` `AppInner`** — 应用根。`useAppInit()`（`:115`）挂载全部初始化 hook；粒度订阅各 store（`:130-147`）；渲染标题栏、状态栏、主内容区、右栏、Dock、Toast 与懒加载对话框；`PANEL_TITLES`（`:64`）定义各面板标题/描述 i18n key。
- **`useAppInit.ts:7`** — 组合 `useEventListeners` + `useInitialDataLoad` + `useHeartbeat` + `useGlobalShortcut`。
- **`useInitialDataLoad.ts:43`** — 启动数据加载：调 `get_init_data`（`api.getInitData()`），合并默认配置（`DEFAULT_CONFIG`）、恢复激活面板（`safeStorage` 的 `campus-active-panel` 或 `config.defaultPanel`）、下发适配器/账号/后台状态到各 store，并在质量未禁用时跳过恢复 quality 面板（`:55`）。`getInitData` 失败降级为 `DEFAULT_CONFIG`（`:160`）。
- **`useEventListeners.ts:59`** — 注册全部后端事件监听：`onBackgroundCheckResult`、`onAutoLoginResult`、`onAdaptersChanged`（500ms 节流 + trailing）、`onAdapterDetailsChanged`、`onAdapterDisabledWarning`、`onLoginLog`、`onAutoExitCountdown`/`Cancelled`、`onCampusExitCountdown`/`Cancelled`、`onNetworkQualityResult`、`onUpdateAvailable`、`onConfigChanged`、`onScheduledActionSkipped`、`onMacResetCycle`；窗体失焦 `blur` 时兜底 flush 配置（`:55`）。注意：这里**故意不注册 `onCloseRequested`**（`:49`）。
- **`useHeartbeat.ts:5`** — 每 5s 调 `renderHeartbeat()` 向 Rust 上报渲染存活；渲染链失活（GPU 崩溃）或窗口隐藏时跳过。
- **`useGlobalShortcut.ts:19`** — 全局快捷键 `Ctrl+Shift+C` 触发 `cancelAutoExit`；输入框/可编辑元素内忽略。

### IPC 封装层

- **`hooks/tauriApi.ts`** — 整个前端的 IPC 出入口。导出 `TauriApi` 接口（`:39-144`）与 `tauriApi`（`:177`）/`tauriApiWithRetry`（`:314`）两个实现。`tauriApi` 把每个方法映射到具体 Rust 命令名（如 `doLogin`→`invoke('do_login')`，`:188`；`getAdapters`→`get_adapters`，`:183`；`saveConfig`→`save_config`，`:179`）。所有 `on*Event` 监听统一由 `createEventListener`（`:146`）工厂生成（带 cancel 安全清理）。`tauriApiWithRetry` 仅在 `saveConfig` 上叠加 `withRetry` 指数退避（`:319`），其余高频命令不做重试（注释解释原因）。`openExternal`（`:233`）先做 URL 协议/长度/解析校验，invoke 失败回退 `@tauri-apps/plugin-shell` 的 `shellOpen`。

### 状态管理（Zustand 领域 store）

- **`useConfigStore.ts:51`** — `ConfigStore`：持有 `config`、`configLoaded`、`passwordSaved`、`initDebugMode`/`initLogRetentionDays`、`accounts`、`activeAccount`、`language`。方法：`updateConfig`（防抖 500ms 落盘 + 标记 dirty 字段，`:59`）、`updateConfigLocal`（仅本地、不落盘，`:93`）、`mergeConfigFromBackend`（跳过 dirty 字段，`:106`）、`saveConfigDirect`（即时落盘 + in-flight 跟踪，`:122`）、`setLanguage`（同步 `safeStorage` + `i18next.changeLanguage`，`:173`）。模块级导出 `hasPendingConfig()`（`:180`）与 `flushPendingConfig()`（`:187`）。配套 `useAppStore.ts` 仅 re-export 兼容旧引用（`:2-3`）。
- **`useAdapterStore.ts:45`** — `AdapterStore`：持有 `adapters`、`adapterDetails`、`isRefreshingAdapters`、`activePanel`。`refreshAdapterData`（`:16`，模块级）并行拉取 `get_adapters` + `get_adapter_details`，并被 `refreshAdapters`（`:51`，带 `_adapterLockFlag` 锁 + 最短 500ms 展示）复用；`setActivePanel`（`:70`）。
- **`useAuthStore.ts:149`** — `AuthStore`：认证核心。`doLogin`（`:155`，带 60s 超时 `withTimeout` 与登录后质量探测节流）、`doLogout`（`:216`）、`checkOnline`（`:247`，校内网判定 + 适配器解析 + Portal 状态查询，带 `_checkOnlineLockFlag` 锁）、`setStatus`/`setBgStatus`、`status`、`bgStatus`。大量纯函数子例程：`detectCampusNetwork`、`buildCampusBgStatusPatch`、`pickAdapterIp`、`queryPortalStatus`、`setStatusStable`。
- **`useQualityStore.ts:40`** — `QualityStore`：持有 `networkQuality`、`dnsDohStatus`、`updateAvailable`、`latestVersion`、`releaseNotes`、`refreshRate`。`refreshQuality`（`:50`）、`setNetworkQuality`（统一更新 `lastQualityResultTime` 供登录后节流，`:74`）。`getLastQualityResultTime` 模块级导出（`:18`）。
- **`useLogToastStore.ts:28`** — `LogToastStore`：`logs`（带连续重复折叠 `count`，`:39`）与 `toasts`（上限 `MAX_TOASTS=4`、同题去重，`:13`）。`addLog`、`addToast`、`addToastWithAction`（承载「取消退出」等操作按钮）、`removeToast`、`removeToastsByPrefix`、`setLogs`、`cleanupToasts`。
- **`useThemeStore.ts:21`** — `ThemeStore`：`themeName`、`isLightMode`、`customThemeColor`、`glassLevel` 及对应 setter；`initTheme`（`:36`）。模块级 `subscribe`（`:62`）把状态变化同步到 DOM（`data-glass`/`dark`/`data-light`/主题 class/`--primary` 等 CSS 变量）。

### 业务 Hook（连接 store 与 IPC）

- **`auth/useAuth.ts:7`** — `useAuth`：聚合 `useAuthStore` 与 `config.portalUrl`，导出 `handleOpenPortal`（`http://10.1.99.100` 或配置值）、`handleOpenSelfService`（`http://10.1.80.200:8080/Self/login/?302=LI`）。
- **`monitor/useMonitor.ts:11`** — `useMonitor`：聚合 auth/quality/config store，导出 `handleToggleBackgroundCheck`（先 `saveConfigDirect` 再启停，带 `bgToggleBusyRef`，`:38`）、`handleTriggerCheck`（`:67`）、`handleToggleLatencyTest`（`:71`，同模式）。
- **`network/useNetwork.ts:39`** — `useNetwork`：聚合 adapter/quality/logToast store，导出 `refreshAdapterInfo`、`handleDhcpRenew`/`handleDhcpReleaseRenew`/`handleDhcpReleaseRenewAdapter`，以及模块级 `normalizeDhcpResults`（`:15`）/`announceDhcpResults`（`:23`，统一 DHCP 结果 toast 文案）。
- **`account/useAccount.ts:13`** — `useAccount`：导出 `handleAddAccount`（`saveCurrentAsAccount`，`:37`）、`handleDeleteAccount`（`:57`）、`handleSwitchAccount`（带 `switchInFlight` 并发去重，`:77`）、`handleRenameAccount`（`:110`）。
- **`settings/useSettings.ts:11`** — `useSettings`：导出 `handleToggleLightMode`、`handleToggleNotification`、`handleSetAutoLaunch`、`handleSetTheme`。
- **`account/selfServiceState.ts`** — 自助服务/Hellō 验证门：`useHelloGate`（`:37`，绑定/揭示的 Windows Hello 验证门，TTL `VERIFY_TTL_MS=570_000`）、`useSelfServiceVerify`（`:77`，面板会话门，卸载 `resetSelfSessionGate` 重置）、`helloEnabled`（`:10`）。
- **`account/useOperatorBind.ts:23`** — `useOperatorBind(user, operator, passwordSaved, credOverride?)`：运营商绑定流（绑定/查询状态/揭示明文/改密），被 `AccountPanel` 使用。
- **`network/useDnsSetup.ts:23`** — `useDnsSetup(count)`：DNS 检测/一键优化/恢复，导出 `RECOMMENDED_DNS`、`ALI_DNS`。
- **`network/useOutboundManual.ts:14`** — `useOutboundManual(adaptersCount)`：当前出站查询/轮询与「立即切换/立即还原」。
- **`network/adapters.ts:15`** — `resolveAdapterNames(adapters, config)`：与 Rust 后端 `resolve_adapter_names` 同源规则（配置名失效则自动检测）。

### 布局组件 `components/layout/`

- **`TitleBar.tsx:50`**（`memo`）— 自定义标题栏：拖拽移动（`handleTitleBarMouseDown`，`:70`）、双击最大化（`:84`）；右侧按钮组含浅色/深色、语言、通知、主题、赞助、关于、最小化、最大化、关闭。读 `useThemeStore.isLightMode`、`useQualityStore.updateAvailable`、`useConfigStore.language`，调用 `getCurrentWindow()`。
- **`DockNav.tsx:331`**（`memo`）— 底部玻璃 Dock 导航：从 `NAV_ITEMS` 渲染各 `DockItem`（`:49`），登录/注销 `ActionButtonWithMenu`（`:177`，双卡时弹适配器选择浮层 `AdapterMenu`，`:88`）。接入 `useGlassRefraction`（折射玻璃）与 `useDockLens`（弹簧镜片系统，含选中药丸/放大副本），`lensEnabled` 在玻璃关/economy/减少动态时退回纯静态（`:365`）。`scopedAdapters`（`:349`）按主/副适配器解析登录作用域。
- **`RightPanel.tsx:82`**（`memo`）— 右侧常驻面板：上半部运行日志（`LogEntry` 渲染，`RIGHT_PANEL_ANIM_THRESHOLD=50` 条降级、最近 30 条动画，`:56`）；下半部适配器信息区（`getAdapterInfo`，`:59`，可展开显示 IP/掩码/网关/DHCP/MAC）。`handleClearWithAnimation`（`:102`）用 GSAP 做逐条删除动画 + `setTimeout` 兜底（防 rAF 冻结卡死）。

### 面板组件（业务域）

- **`auth/DashboardPanel.tsx:763`**（`memo`）— 仪表盘：可拖拽重排的卡片网格（`Reorder.Group`，`:832`），卡片含 `QuickActionsCard`（DHCP 续租/换 IP，`:90`）、`AccountManageCard`（账号切换，`:276`）、`SelfOnlineCard`/`SelfLogCard`（自助数据，`:498`/`:657`）、`NetworkQualityCard`（`:379`）。`useSelfCardReveal`（`:424`）与 `useSelfCardFetch`（`:440`）是自助卡共用骨架。`loadLayout`/`saveLayout`（`:63`/`:74`）把布局存到 `safeStorage('campus-dashboard-layout')`。
- **`account/AccountPanel.tsx:52`**（`memo`）— 账号页：登录信息、密码（`handleClearPassword` 走 `clearPassword=true`，`:78`）、运营商绑定（`useOperatorBind`）、账号档案增删改切（props 注入 `onAddAccount` 等）、设备别名。
- **`account/SelfServicePanel.tsx:52`** — 自助服务：在线设备 `onlineList`、上网记录 `history`/`SelfOnlineRecords`、VPN 凭据（`VpnCredentialDialog`/`VpnManageDialog`）、设备别名 `deviceAliases`（持久化在 config）、日期范围上网记录查询。切入验证一次（`ensureSelfVerified`，`:80`），卸载 `resetSelfSessionGate`。
- **`network/NetworkPanel.tsx:217`**（`memo`）— 网络页：适配器列表（`SortableAdapterRow` 长按起拖，`:95`）、出站优先级拖拽排序（`buildOutboundOrder` + `handleDragEnd` 落盘 `outboundPriority`）、夜间出站切换 + 恢复时刻（`formatRestoreTime`/`RESTORE_TIME_*`，`:53-69`）、登录网卡/账号绑定、DNS 优化（`useDnsSetup` + `DnsAdapterCard`/`SegmentTabs`）。
- **`monitor/MonitorPanel.tsx:81`**（`memo`）— 监控页：后台检测开关 + 间隔（`useAsyncLock` 加锁，`:93`/`:97`）、适配器在线状态 `AdapterStatusCard`（`:29`）、自动化设置（自动启动/在线退出/就绪登录/非校园网退出/定时登录注销）、校园网校验（SSID 名/gateway 改为固定展示，`:324`/`:339`）。
- **`monitor/QualityPanel.tsx:1`**（`memo`）— 质量页：网络质量评级 + 网关/DNS、Web/视频/游戏分组的延迟探测明细（537 行，含 `DETAIL_CATEGORIES`，`:38`）、延迟测试开关（`onToggleLatencyTest`）。
- **`monitor/SpeedTestPanel.tsx:1`**（`memo`）— 测速页：`SPEED_TEST_SITES`（`:28`）列出外站测速/教育网/网速测试入口，统一 `openExternal` 外链。
- **`settings/SettingsPanel.tsx:35`**（`memo`）— 设置页：开机自启/通知/主题/默认面板/轻量化/数据导入导出（`exportConfig`/`importConfig`）、安全开关变更需 `verifyWindowsIdentity`（`:47`）。
- **`monitor/StatusBar.tsx:26`**（`memo`）— 状态栏：展示 `status`（在线状态文本）、运营商徽标（`IspMark` + `operatorLabelKey`）、网络质量胶囊（`NetworkQualityCapsule`）、校园网/SSID 徽标，开放 Portal 与自助入口。

### 共享组件 `shared/`

- **`LogPanel.tsx:85`**（`memo`）— 日志面板（独立 `log` 面板）：按行解析（`LOG_LINE_REGEX`，`:62`）、级别/模块/搜索过滤、`LINE_OPTIONS` 行数选择、5s 轮询（`fetchLogs`，`:119`，受可见性门控）、调试开关、日志保留天数、导出诊断包 `exportDiagnostics`（`:270`）、清空（`handleClearConfirm` 用 GSAP + `setTimeout` 兜底，`:285`）。`MAX_DISPLAY_LINES=200`、`displayedLines.length <= 30` 才启用逐条动画（`:584`）。
- **`ToastContainer.tsx:30`**（`memo`）— 右下角 toast 列表（`aria-live="polite"`），按 `type` 选看板娘变体（`TOAST_MASCOTS`，`:16`）。
- **`ErrorBoundary.tsx:15`**（`class extends Component`）— 捕获渲染错误，展示错误 + 折叠详情 + 重载按钮。
- **`ConfirmDialog.tsx:20`** — 确认弹窗（破坏性操作防双击，`confirming` 态，`:23`）。
- **`FluidBackground.tsx:24`** — CSS-only 慢速光斑（仅 transform/opacity）。
- **`SponsorCard.tsx:24`** — 赞助下拉浮层（非模态，锚定 TitleBar 赞助按钮 `data-sponsor-anchor`）。
- **`AnimatedNumber.tsx:14`** — GSAP 数字滚动（质量/延迟展示）。
- **`SegmentTabs.tsx:19`** — 分段切换标签（DNS 协议族 IPv4/IPv6/both）。
- **`IspMark.tsx:18`** — 运营商/校徽 WebP 标识图。
- **`MascotFigure.tsx:7`** — 看板娘图片出口（`MascotVariant`）。
- **`RefreshButton.tsx:13`** — 刷新按钮（刷新完成抖动 `refresh-shake`）。
- **`shared/index.ts`** — barrel 导出 LogPanel/ConfirmDialog/AnimatedNumber/RefreshButton/SegmentTabs/ToastContainer/FluidBackground/ErrorBoundary + `types`/`ui-types`/`ui-constants`。

### 类型与常量

- **`auth/types.ts`** — `PortalStatusResult`、`CommandResult`、`LoginResult`。
- **`network/types.ts`** — `Adapter`、`AdapterDetail`、`DnsAdapterInfo`、`DnsDohStatus`、`DhcpRenewResult`、`DhcpReleaseRenewResult`、`DnsSetupResult`、`EnableAdapterResult`。
- **`monitor/types.ts`** — `AdapterOnlineStatus`、`AdapterCampusStatus`、`BackgroundStatus`（含 `onlineOperator`/`secondaryOnlineOperator`/`campusAdapters` 等）、`BackgroundCheckEventData`、`NetworkQuality`、`AutoLoginEventData`。
- **`account/types.ts`** — `AccountResult`、`SwitchAccountResult`、`DeleteAccountResult`、`SaveAccountResult`（re-export `AccountItem`）。
- **`account/selfServiceTypes.ts`** — 自助服务协议类型（`SelfHistoryRow`/`SelfLogRow`/`SelfLogSummary`）与格式化工具（`formatMac`/`formatEpoch`/`formatFlowMb`/`fmtMoney`/`localDateStr` 等）。
- **`settings/types.ts`** — `Config`（~90 字段，含 `user`/`password`/`adapter1`/`operator`/`enableNetworkQuality`/`outboundPriority` 等）、`AccountItem`、`AutoLaunchResult`、`CoreAutostartResult`、`InitData`。
- **`settings/constants.ts`** — `DEFAULT_CONFIG`（`:6`，所有配置缺省值）、`ISP_OPTIONS`、`operatorLabelKey`、`THEME_OPTIONS`、`GLASS_OPTIONS`、`VALID_THEMES`/`VALID_GLASS_LEVELS`、`DEFAULT_PANEL_OPTIONS`。
- **`shared/ui-types.ts`** — `StatusState`、`PanelName`（9 值）、`ThemeName`、`GlassLevel`、`LogType`、`LogEntry`、`ToastMessage`、`AdapterDisabledWarningData`、`AutoExitCountdownData`、`SaveConfigResult`。
- **`shared/types.ts`** — `UpdateAvailableData`、`UpdateInfo`、`DownloadProgress`、`MirrorSource`。
- **`shared/ui-constants.ts`** — `MAX_LOG_ENTRIES=300`、`APP_VERSION='2.4.3'`、`APP_NAME`、`PASSWORD_MASK='***'`、`NAV_ITEMS`（9 导航项）。
- **`network/constants.ts`** — `QUALITY_CONFIG`（质量评级配色/图标/柱数）。
- **`lib/utils.ts`** — `cn`（clsx+twMerge）、`extractErrorMessage`、`safeStorage`（localStorage + 内存兜底）。
- **`lib/latency.ts`** — `getLatencyLevel`/`getLatencyColor`/`mergeNetworkQuality`（busy 不采纳、disabled 跳过，`:25`）/`extractGatewayLatency`/`extractExternalLatency`。
- **`lib/animations.ts`** — `createLogEntryVariants`/`createPanelAppleVariants`（面板切换 spring 变体）。

## Architecture

前端是一个「领域 store 为中心 + IPC 封装层 + 业务 hook + 布局/面板/共享组件」的分层 SPA。所有跨进程调用集中在 `hooks/tauriApi.ts`，store 不直接 invoke，而是通过业务 hook（`useAuth`/`useMonitor`/`useNetwork`/`useAccount`/`useSettings`）或 `App.tsx` 桥接到 IPC。组件只读 store、调用 hook 返回值。

```text
┌──────────────────────────────────────────────────────────────────────┐
│ App.tsx (AppInner)                                                     │
│  useAppInit() → useEventListeners / useInitialDataLoad / useHeartbeat   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌───────────┐  │
│  │ TitleBar     │  │ StatusBar    │  │ DockNav      │  │ RightPanel│  │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └─────┬─────┘  │
│         └────────┬────────┴────────┬────────┴────────┬───────┘        │
│  主内容区 (switch deferredPanel):                                   │
│  DashboardPanel / AccountPanel / SelfServicePanel / NetworkPanel /  │
│  MonitorPanel / QualityPanel / SpeedTestPanel / SettingsPanel /     │
│  LogPanel                                                          │
│         │  props (onUpdateConfig/onSwitchAccount/...)               │
└─────────┼────────────────────────────────────────────────────────────┘
          ▼
┌──────────────────────────────────────────────────────────────────────┐
│ 业务 hook: useAuth / useMonitor / useNetwork / useAccount / useSettings│
│  account/selfServiceState · useOperatorBind · network/useDnsSetup    │
│  network/useOutboundManual · network/adapters                       │
└─────────┬────────────────────────────────────────────────────────────┘
          ▼  读/调
┌──────────────────────────────────────────────────────────────────────┐
│ Zustand 领域 store:                                                    │
│  useConfigStore · useAdapterStore · useAuthStore · useQualityStore    │
│  useLogToastStore · useThemeStore                                     │
└─────────┬────────────────────────────────────────────────────────────┘
          ▼
┌──────────────────────────────────────────────────────────────────────┐
│ hooks/tauriApi.ts  (invoke + listen 封装; tauriApi / tauriApiWithRetry)│
└─────────┬──────────────────────────────────────────────────────────────┘
          ▼  Tauri v2 IPC (invoke 'command' / listen 'event')
┌──────────────────────────────────────────────────────────────────────┐
│ Rust 后端 (src-tauri)                                                  │
└──────────────────────────────────────────────────────────────────────┘
```

`shared/` 跨面板复用：`LogPanel`/`ToastContainer`/`ErrorBoundary`/`ConfirmDialog`/`FluidBackground`/`SponsorCard`/`AnimatedNumber`/`SegmentTabs`/`IspMark`/`MascotFigure`/`RefreshButton`。`components/ui/` 为 Radix 基础组件（card/button/dialog/switch/select 等，无 barrel，按文件直引——见 `App.tsx:524` 注释）。

## Data Flow

1. **启动加载**：`useInitialDataLoad` → `api.getInitData()`（`tauriApi.ts:267`，命令 `get_init_data`）一次性取回 `config / adapters / adapterDetails / accounts / activeAccount / backgroundStatus / isAutoStart / logRetentionDays / debugMode`；合并 `DEFAULT_CONFIG` 写入 `useConfigStore.config`，并下发到 adapter/auth/quality/theme store（`useInitialDataLoad.ts:41-101`）。随后 `checkOnline()` 做首屏在线判定。
2. **用户改配置**：组件调用业务 hook → `useConfigStore.updateConfig(partial)`（`useConfigStore.ts:59`）：本地 `set` + 标记 dirty 字段 + 500ms 防抖触发 `saveConfigDirect` → `api.saveConfig`（`invoke('save_config')`，`tauriApi.ts:179`）。
3. **后端主动推送**：`useEventListeners` 注册的事件（如 `onBackgroundCheckResult`、`onNetworkQualityResult`、`onConfigChanged`、`onAdaptersChanged`）回调里直接 `useXxxStore.getState().setYyy(...)`，把后端状态同步进 store；组件因订阅而重渲染。`onConfigChanged` 回写时跳過本地 dirty 字段（`mergeConfigFromBackend`，`useConfigStore.ts:106`）。
4. **登录/注销**：`DockNav` 按钮 → `useAuthStore.doLogin/doLogout`（`useAuthStore.ts:155`/`:216`）→ `api.doLogin/doLogout`（`invoke('do_login')`/`('do_logout')`）→ 结果写 `status`/`bgStatus` 与日志/toast。
5. **面板切换**：`DockNav` 点击 → `App.handlePanelChange`（`App.tsx:320`）→ `useAdapterStore.setActivePanel` + `safeStorage.set('campus-active-panel')`；`App` 用 `useDeferredValue` 派生 `deferredPanel`，经 `AnimatePresence` 切换面板（`App.tsx:133`）。
6. **日志/Toast**：各业务处调 `useLogToastStore.addLog/addToast`，`RightPanel` 与 `ToastContainer` 订阅渲染；`LogPanel` 独立轮询 `api.getLogs`（`tauriApi.ts:264`，`get_logs`）。
7. **关闭窗口**：`App.handleCloseWindow`（`App.tsx:295`）→ `flushPendingConfig()`（`useConfigStore.ts:187`）等待 in-flight/debounce 保存完成（最多 2s）→ `api.closeWindow()`（`invoke('close_window')`）。

## Connections

- 后端 IPC 命令与事件映射见下方「前端↔后端 IPC 调用映射」。
- 后端模块（Rust 命令实现、事件发射）：见 Rust 端 `[[src-tauri-commands]]` 与 `[[src-tauri-events]]`（本仓库 `src-tauri/src/`）——前端 `tauriApi.ts` 的命令名需与 Rust `#[tauri::command]` 一一对应。
- 配置结构与默认值的唯一真相在 `settings/types.ts` 的 `Config` 与 `settings/constants.ts` 的 `DEFAULT_CONFIG`，与 Rust `Config` struct 对应（字段增减需双端同步）。
- 适配器解析需与 Rust `resolve_adapter_names`（`network/adapters.ts:15` 注释指明 `src-tauri/src/network/adapter.rs`）保持同源规则。
- 质量评级/延迟阈值由 `network/constants.ts`（`QUALITY_CONFIG`）与 `lib/latency.ts`（`getLatencyLevel`）定义，与后端 `run_quality_check` 的 60s 全局节流对齐。
- 国际化文案位于 `i18n/locales/zh.json`、`en.json`，组件仅引用 key（如 `nav.dashboard`、`monitor.start`）。

## 前端↔后端 IPC 调用映射

下表列出 `hooks/tauriApi.ts` 中每个方法对应的 Tauri v2 命令名（或事件名）与用途。命令来自 `invoke('<cmd>', args)`，事件来自 `listen('<event>')`。

### 命令（invoke）

| 前端方法 (tauriApi.ts) | Rust 命令 | 参数 | 说明 |
|---|---|---|---|
| `getConfig` | `get_config` | — | 取完整配置 |
| `saveConfig` | `save_config` | `{config, clearPassword?}` | 落盘配置（带重试） |
| `exportConfig` | `export_config` | `{includePassword?}` | 导出配置到 `<dataDir>/exports/` |
| `importConfig` | `import_config` | `{path}` | 从 JSON 导入 |
| `exportDiagnostics` | `export_diagnostics` | `{days?}` | 导出诊断包 |
| `getAdapters` | `get_adapters` | `{force?}` | 适配器列表 |
| `enableAdapter` | `enable_adapter` | `{adapterName}` | 启用禁用卡 |
| `getAdapterDetails` | `get_adapter_details` | — | 适配器详情 |
| `checkPortalStatus` | `check_portal_status` | `{adapterIp}` | Portal 在线状态 |
| `checkCampusStatus` | `check_campus_status` | — | 校园网判定（逐卡） |
| `doLogin` | `do_login` | `{adapterName?}` | 登录 |
| `doLogout` | `do_logout` | `{adapterName?}` | 注销 |
| `bindOperator` | `bind_operator` | `{account,password,operator,phone,smsPassword}` | 绑定运营商 |
| `getBindStatus` | `query_bind_status` | `{account,password}` | 查绑定状态 |
| `verifyWindowsIdentity` | `verify_windows_identity` | `{consentMessage}` | Windows Hello 验证门 |
| `revealOperatorCredential` | `reveal_operator_credential` | `{account,password,operator}` | 揭示明文密码 |
| `querySelfDashboard` | `query_self_dashboard` | `{account,password}` | 自助在线设备 |
| `selfOfflineSession` | `self_offline_session` | `{account,password,sessionId}` | 踢下线 |
| `querySelfOnlineLog` | `query_self_online_log` | `{account,password,startTime,endTime}` | 上网记录 |
| `changeSelfPassword` | `change_self_password` | `{account,password,newPassword}` | 改自助密码 |
| `vpnSaveCredential` | `vpn_save_credential` | `{account,password}` | 存 VPN 凭据 |
| `vpnCredentialStatus` | `vpn_credential_status` | — | VPN 凭据状态 |
| `vpnClearCredential` | `vpn_clear_credential` | — | 清 VPN 凭据 |
| `setSelfServiceViaVpn` | `set_self_service_via_vpn` | `{enabled}` | 自助经 VPN 出站 |
| `vpnTestConnection` | `vpn_test_connection` | — | 测 VPN 连通性 |
| `vpnVerifyCredential` | `vpn_verify_credential` | `{account,password}` | 校验 VPN 凭据 |
| `minimizeWindow` | `minimize_window` | — | 最小化 |
| `closeWindow` | `close_window` | — | 关闭（flush 后调） |
| `listAccounts` | `list_accounts` | — | 账号档案列表 |
| `createAccount` | `create_account` | `{accountName,user,password,operator}` | 新建档案 |
| `switchAccount` | `switch_account` | `{accountName}` | 切换账号 |
| `saveCurrentAsAccount` | `save_current_as_account` | `{accountName}` | 当前登录信息存为档案 |
| `deleteAccount` | `delete_account` | `{accountName}` | 删除档案 |
| `renameAccount` | `rename_account` | `{accountId,displayName}` | 改名（仅显示名） |
| `getActiveAccount` | `get_active_account` | — | 当前激活账号 |
| `startBackgroundCheck` | `start_background_check` | — | 启后台检测 |
| `stopBackgroundCheck` | `stop_background_check` | — | 停后台检测 |
| `triggerBackgroundCheck` | `trigger_background_check` | — | 立即检测一次 |
| `getBackgroundStatus` | `get_background_status` | — | 后台状态 |
| `dhcpRenewAll` | `dhcp_renew_all` | — | 全部 DHCP 续租 |
| `dhcpReleaseRenew` | `dhcp_release_renew` | — | 全部换 IP |
| `dhcpReleaseRenewAdapter` | `dhcp_release_renew_adapter` | `{adapterName}` | 单卡换 IP |
| `outboundSwitchNow` | `outbound_switch_now` | — | 立即切换出站 |
| `outboundRestoreNow` | `outbound_restore_now` | — | 立即还原出站 |
| `getCurrentOutboundName` | `get_current_outbound_name` | — | 当前真实出站接口 |
| `checkNetworkQuality` | `check_network_quality` | — | 网络质量探测 |
| `startLatencyTest` | `start_latency_test` | — | 启延迟测试 |
| `stopLatencyTest` | `stop_latency_test` | — | 停延迟测试 |
| `openExternal` | `open_external` | `{url}` | 打开外链（失败回退 shell 插件） |
| `setAutoLaunch` | `set_auto_launch` | `{enabled}` | 开机自启注册表 |
| `getCoreAutostart` | `get_core_autostart` | — | SYSTEM 计划任务状态 |
| `setCoreAutostart` | `set_core_autostart` | `{enabled}` | 设 SYSTEM 计划任务 |
| `setNotificationEnabled` | `set_notification_enabled` | `{enabled}` | 通知开关 |
| `cancelAutoExit` | `cancel_auto_exit` | — | 取消自动退出倒计时 |
| `showWindow` | `show_window` | — | 显示窗口 |
| `getLogs` | `get_logs` | `{lines?}` | 取日志行 |
| `clearLogs` | `clear_logs` | — | 清空日志 |
| `setDebugMode` | `set_debug_mode` | `{enabled}` | 调试开关 |
| `getInitData` | `get_init_data` | — | 启动聚合数据 |
| `checkUpdate` | `check_update` | — | 查更新 |
| `downloadUpdate` | `download_update` | `{url}` | 下载更新包 |
| `cancelDownloadUpdate` | `cancel_download_update` | — | 取消下载 |
| `installUpdate` | `install_update` | `{filePath,checksumUrl?}` | 安装更新 |
| `getMirrorUrls` | `get_mirror_urls` | `{githubUrl}` | 镜像源列表 |
| `checkDnsDohStatus` | `check_dns_doh_status` | — | DNS/DoH 状态 |
| `setupDnsDoh` | `setup_dns_doh` | `{family?}` | DNS/DoH 优化 |
| `resetDns` | `reset_dns` | — | 恢复自动 DNS |
| `renderHeartbeat` | `render_heartbeat` | — | 渲染存活心跳 |
| `notifyWindowReady` | `notify_window_ready` | — | 挂载完成信号 |
| `setLogRetentionDays` | `set_log_retention_days` | `{days}` | 日志保留天数 |

### 事件（listen）

| 前端方法 | 事件名 | 回调数据类型 | 用途 |
|---|---|---|---|
| `onBackgroundCheckResult` | `background-check-result` | `BackgroundCheckEventData` | 后台检测/适配器在线状态 |
| `onAutoLoginResult` | `auto-login-result` | `AutoLoginEventData` | 定时自动登录结果 |
| `onAdaptersChanged` | `adapters-changed` | `Adapter[]` | 适配器增删（500ms 节流） |
| `onAdapterDetailsChanged` | `adapter-details-changed` | `AdapterDetail[]` | 适配器详情更新 |
| `onAdapterDisabledWarning` | `adapter-disabled-warning` | `AdapterDisabledWarningData` | 禁用卡警告 |
| `onLoginLog` | `login-log` | `{message,type}` | 登录日志 |
| `onNetworkQualityResult` | `network-quality-result` | `NetworkQuality` | 质量结果（循环推送） |
| `onAutoExitCountdown` | `auto-exit-countdown` | `AutoExitCountdownData` | 自动退出倒计时（带取消操作） |
| `onAutoExitCancelled` | `auto-exit-cancelled` | — | 自动退出已取消 |
| `onCampusExitCountdown` | `campus-exit-countdown` | `{minimizeDelay,exitDelay}` | 非校园网退出倒计时 |
| `onCampusExitCancelled` | `campus-exit-cancelled` | — | 非校园网退出已取消 |
| `onScheduledActionSkipped` | `scheduled-action-skipped` | `{kind:'login'\|'logout'}` | 静默窗外定时动作跳过 |
| `onMacResetCycle` | `mac-reset-cycle` | — | MAC 重置循环 |
| `onConfigChanged` | `config-changed` | `{config}` | 配置被后端改动（托盘切账号等） |
| `onDownloadProgress` | `update-download-progress` | `DownloadProgress` | 更新下载进度 |
| `onUpdateAvailable` | `update-available` | `UpdateAvailableData` | 发现新版本 |
| `onUpdateNotificationClick` | `update-notification-click` | `unknown` | 系统通知被点击（开关于关于页） |

## Known Issues

- **`useConfigStore.ts:132-135`**：历史缺陷——保存密码后 `config-changed` 回写 MASK，但 `passwordSaved` 从不置 true；已修复为 `saveConfigDirect` 成功即 `syncPasswordSaved(true)`。
- **`useEventListeners.ts:49-57`**：前端**刻意不注册 `onCloseRequested`**——Tauri core 检测到该 JS 监听就无条件 `prevent_close`，会导致窗口销毁被转嫁到 JS 异步收尾链，v2.4.2 实测关闭按钮 6 连击无效。配置 flush 改为关闭发起方（App 的 `handleCloseWindow`）就地完成。
- **`App.tsx:46-49`**：`LogPanel` 曾懒加载，实测切换延迟从 ~65ms 恶化到 ~366ms（因 `React.lazy+Suspense` 多一轮挂载 + 动画重入），已回归静态导入。
- **`RightPanel.tsx:56` / `LogPanel.tsx:83`**：日志超过 50/200 条或 `displayedLines > 30` 时降级为纯 `div` 渲染，避免 framer-motion 组件大量 reconciliation（FP-4 / FE-B-10）。
- **`LogPanel.tsx:137`、`RightPanel.tsx:135`**：GSAP 清空动画依赖 rAF ticker；WebView2/安卓 WebView 在窗口被遮挡时 rAF 停摆，tween 的 `onComplete/onInterrupt` 不触发，`isClearing` 会卡死——两处均用 `setTimeout` 兜底复位（与动画时序解耦）。
- **`useAuthStore.ts:19`**：`checkOnline` 的 `_checkOnlineLockFlag` 改为「promise 真正 settle 即释放」，旧实现用 `setTimeout(500)` 释放，执行超 500ms 时锁提前释放可重入。
- **`useAccount.ts:88-91`**：`handleSwitchAccount` 用 `result?.activeAccount !== undefined` 判空（而非真值），修复 R4 缺陷——后端返回空串时跳过 `setActiveAccount` 导致高亮不切换。
- **`account/selfServiceState.ts:26`**：Hello 门 TTL 为 `VERIFY_TTL_MS=570_000`，与后端 600s 对齐并留 30s 余量；旧实现缓存永真，10 分钟后撞上后端过期错误而功能死锁。
- **`App.tsx:289-294`**：v2.4.2 关闭缺陷——此前依赖全局 `onCloseRequested` JS 监听收尾，窗口销毁被拖延；现约定前端不注册该监听，flush 由关闭发起方就地完成，销毁控制权回归 Rust（`app/shutdown.rs` 三分支）。
- **`DashboardPanel.tsx` 等面板**：`config` 改为各面板用 `useConfigStore(useShallow(s=>s.config))` 自订阅（`DashboardPanel.tsx:773`、`NetworkPanel.tsx:230`、`MonitorPanel.tsx:86`、`SettingsPanel.tsx:43` 等），避免 App 外壳因任意 config 字段（主题拖拽、文本输入）变化而级联重渲染。
- **`quality` 面板在 `enableNetworkQuality===false` 时完全不渲染**（Dock 入口与恢复面板逻辑都已隐藏，`App.tsx:384`、`DockNav.tsx:354`、`useInitialDataLoad.ts:55`），重启后不会落到空白的 quality 面板。

