---
title: 桌面前端业务面板
type: module
source_files:
  - tauri-app/frontend/src/App.tsx
  - tauri-app/frontend/src/main.tsx
  - tauri-app/frontend/src/auth/AboutDialog.tsx
  - tauri-app/frontend/src/auth/DashboardPanel.selfCards.test.tsx
  - tauri-app/frontend/src/auth/index.ts
  - tauri-app/frontend/src/auth/types.ts
  - tauri-app/frontend/src/auth/useAuth.ts
  - tauri-app/frontend/src/account/SelfServicePanel.tsx
  - tauri-app/frontend/src/account/DeviceAliasDialog.tsx
  - tauri-app/frontend/src/account/index.ts
  - tauri-app/frontend/src/account/AccountPanel.helloGate.test.tsx
  - tauri-app/frontend/src/monitor/index.ts
  - tauri-app/frontend/src/settings/ThemeDialog.tsx
  - tauri-app/frontend/src/settings/OnboardingWizard.tsx
  - tauri-app/frontend/src/monitor/StatusBar.tsx
  - tauri-app/frontend/src/settings/constants.ts
  - tauri-app/frontend/src/lib/renderLiveness.ts
  - tauri-app/frontend/src/lib/utils.ts
  - tauri-app/frontend/src/components/layout/RightPanel.tsx
tags: [前端, 面板, react]
---

## Overview

本模块是桌面端前端的**业务面板层与外壳**。`App.tsx` 负责业务面板的路由、转场动画、对话框与全局弹层编排；`main.tsx` 负责渲染根、主题初始化与渲染进程崩溃自恢复。五个业务域目录（`auth/`、`account/`、`monitor/`、`network/`、`settings/`）各自提供「面板组件 + `useXxx` 编排 hook + 领域类型」，本次提供的源码覆盖 `auth/`、`account/` 的部分面板与 `monitor/` 的导出索引，以及两个测试文件。`auth/index.ts` 导出 `DashboardPanel`、`AboutDialog`、`useAuth`；`account/index.ts` 导出 `AccountPanel`、`ChangePasswordDialog`、`useAccount`（实现未全提供，仅有测试）；`monitor/index.ts` 导出 `MonitorPanel` 等（实现未提供）。前端私有状态一律通过 `@/hooks/*Store`（zustand）与 `tauriApiWithRetry` 与 Rust 后端交互。

面板自身不做 IPC 拼装：`useAuth` / `useAccount` / `useMonitor` / `useNetwork` / `useSettings` 五个 hook 把 store action 与 `invoke` 调用收口成可直接绑到 JSX 的处理函数，`App.tsx` 再把这些函数作为 props 下发给面板，形成「App 编排 → hook 执行 → store 落状态 → invoke 过 IPC」的单向链。本次源码中 `useAuth` 直接定义在 `auth/useAuth.ts`，`useAccount` 等未提供实现但由 `account/index.ts` 导出。

## Key Components

### 应用外壳：App.tsx (`tauri-app/frontend/src/App.tsx`)

`AppInner` 是全部装配逻辑所在（第 114-576 行）；导出入口 `App` 只包三层：`ErrorBoundary` → `AnimationActiveProvider` → `AppInner`（第 578-586 行）。

- **启动与懒加载 loader**：`loadAboutDialog`（第 50 行）、`loadThemeDialog`（第 51 行）、`loadOnboardingWizard`（第 52 行）、`lazy(...)` 三处（第 54-56 行）、`preloadPanels`（第 59-62 行，并行预加载剩余懒加载 chunk）。注意第 30 行注释称「仅低频的 LogPanel/对话框保留懒加载」，但第 21 行已静态导入 `LogPanel`，第 47-49 行注释解释 `LogPanel` 从 lazy 回归静态导入，第 30 行注释为残留旧结论。
- **面板元数据**：`PANEL_TITLES`（第 64-74 行），9 个面板各带 `titleKey` / `descKey`。
- **赞助外挂窗常量**：`SPONSOR_SHOW_DELAY_MS = 1_000`（第 77 行）、`SPONSOR_SHOW_INTERVAL_MS = 7 * 24 * 60 * 60 * 1000`（第 78 行）。
- **面板容器样式**（限制 `contain` 到 `layout style` 而非 `paint`，避免裁掉编辑态卡片右上角删除按钮）：`PANEL_CONTAINER_STYLE`（第 82 行）。
- **侧边看板娘每日随机**：`SIDE_MASCOT_POOL`（第 85-89 行，14 项）、`pickDailySideMascots`（第 91-99 行，14×13=182 种有序组合按天序号轮转）。
- **懒加载骨架**：`PanelSkeleton`（第 102-112 行）。
- **壳层状态订阅与下发**：`useAppInit()`（第 115 行）；`dailySideMascots` 状态与每分钟跨天比对（第 118-128 行）；`activePanel`（第 130 行）；`deferredPanel = useDeferredValue(activePanel)`（第 133 行）；`adapters`（第 134 行）；`accounts` / `activeAccount`（第 135-136 行）；`isLoggingIn`（第 137 行）；粒度订阅的 store 字段：`configUser`（第 142 行）、`configEnableNetworkQuality`（第 143 行）、`configAutoLaunch`（第 144 行）、`initDebugMode`（第 145 行）、`initLogRetentionDays`（第 146 行）、`configEnableNotification`（第 147 行）；`api` 静态导入 `tauriApiWithRetry`（第 8 行）。
- **store action 订阅**：`updateConfig`（第 155 行）、`setActivePanel`（第 156 行）、`setUpdateAvailable` / `setLatestVersion` / `setReleaseNotes`（第 157-159 行）、响应式读 `latestVersion` / `releaseNotes` / `qualityUpdateAvailable`（第 161-163 行）、`addToast`（第 164 行）、`doLogin`（第 165 行）、`refreshQuality`（第 166 行）。
- **五个业务 hook 解构**（面板 props 的唯一来源）：`useAuth`（第 168 行）、`useMonitor`（第 169 行）、`useNetwork`（第 170 行）、`useAccount`（第 171 行）、`useSettings`（第 172 行）。
- **日志区 `useShallow` 订阅** `logs` / `toasts` / `removeToast` / `setLogs`（第 174-181 行）。
- **面板切换锁与本地弹层状态**：`panelChangeLock`（第 183 行）、`aboutOpen`（第 189 行）、`themeOpen`（第 190 行）、`confirmDelete`（第 191 行）、`onboardingOpen`（第 192 行）、`sponsorOpen`（第 193 行）、`isMaximized`（第 194 行）。
- **动画与启动**：`useAnimationProfile()`（第 196 行）；`panelVariants = createPanelAppleVariants(profile.easing)`（第 197 行）；`useStartupBoost` 的 `setRef` / `runStartupSequence`（第 198 行）。
- **副作用**：双 rAF 启动序列 + 并行预热懒加载 chunk（第 199-209 行）；窗口 resize/maximized 监听（cleanup 里顺带 `cleanupToasts()`）（第 211-235 行）；系统通知点击开关于框（第 238-241 行）；首次引导自动弹出（第 243-249 行）；赞助浮层自动弹出（7 天频控 + 窗口可见性等待 + 弹出即写时间戳）（第 255-277 行）。
- **回调**：`handleToggleMaximize`（第 279-287 行）、`handleCloseWindow`（第 295-314 行，关窗前 `flushPendingConfig` 防抖配置）、`handleClearLogs`（第 316-318 行）、`handlePanelChange`（60ms 重入锁 + 持久化 `campus-active-panel`）（第 320-329 行）。
- **标题文案**：`panelInfo = PANEL_TITLES[deferredPanel] || PANEL_TITLES.dashboard`（第 331 行）。

**面板路由完整分支（`switch (deferredPanel)` 第 334-419 行）**

| 分支 | 渲染的面板 | 传入 props | 渲染条件 |
|---|---|---|---|
| `'dashboard'` | `DashboardPanel` | `accounts`、`activeAccount`、`onUpdateConfig`、`onSwitchAccount`、`onDhcpRenew`、`onDhcpReleaseRenew`、`onDhcpReleaseRenewAdapter`、`onRefreshQuality` | 无额外条件（第 335-348 行） |
| `'account'` | `AccountPanel` | `adapters`、`accounts`、`activeAccount`、`onUpdateConfig`、`onAddAccount`、`onDeleteAccount`、`onSwitchAccount`、`onRenameAccount` | 无额外条件（第 349-362 行） |
| `'selfservice'` | `SelfServicePanel` | 无 props（自订阅 store） | 无额外条件（第 363-365 行） |
| `'network'` | `NetworkPanel` | `adapters`、`onUpdateConfig` | 无额外条件（第 366-373 行） |
| `'monitor'` | `MonitorPanel` | `onUpdateConfig`、`onToggleBackgroundCheck`、`onTriggerCheck` | 无额外条件（第 374-382 行） |
| `'quality'` | `QualityPanel` | `onUpdateConfig`、`onRefreshQuality`、`onToggleLatencyTest` | `configEnableNetworkQuality !== false`，否则渲染 `null`（第 383-391 行，条件判断在第 384 行） |
| `'settings'` | `SettingsPanel` | `autoLaunch`、`onUpdateConfig`、`onSetAutoLaunch`、`onShowOnboarding` | 无额外条件（第 392-401 行） |
| `'log'` | `LogPanel`（静态导入，第 21 行） | `api`、`addToast`、`initialDebugMode`、`initialLogRetentionDays` | 无额外条件（第 402-411 行） |
| `'speedtest'` | `SpeedTestPanel` | `openExternal` | 无额外条件（第 412-419 行） |
| 默认 | `null` | — | 当 `deferredPanel` 不在上述 9 个键内时渲染空白 |

**转场与渲染管线**：外层 `AnimatePresence mode="wait"` + `m.div key={deferredPanel}`，`variants={panelVariants}`，`className="panel-content"` 与 `PANEL_CONTAINER_STYLE`，内部依次包 `ErrorBoundary` 与 `Suspense fallback={<PanelSkeleton />}`（第 472-486 行）。标题与描述同样包进一组 `AnimatePresence mode="wait"` + `m.div key={deferredPanel}`，复用同一个 `panelVariants` 实现标题/描述与内容同向滑入（第 458-469 行）。

**外壳其余结构**：根容器（含 `isMaximized && 'app-maximized'`）（第 422 行）；`FluidBackground`（第 425 行）；`TitleBar`（第 428-439 行）；`StatusBar`（第 443-446 行）；主区 `main`（`max-w-[880px]` / `max-w-[720px]` 随最大化切换）（第 454-455 行）；`RightPanel`（第 490-494 行）；两张侧边看板娘 `img`（`min-[1360px]` 起显示、右侧偏移 304px 避让 `w-72`）（第 500-515 行）；`DockNav`（第 517-520 行）；`ToastContainer`（第 522 行）；`SponsorCard`（第 524 行）；`AboutDialog`（`Suspense` + 缓存版本号 + `onUpdateAvailable` 回调写 store/发 toast/写日志）（第 526-544 行）；`ThemeDialog`（第 546-553 行）；删除账号 `ConfirmDialog`（确认后 `await handleDeleteAccount(name)` 再关框）（第 555-562 行）；`OnboardingWizard`（第 564-573 行）。

### 渲染根：main.tsx (`tauri-app/frontend/src/main.tsx`)

- **gsap 全局默认**（仅 `ease` 与 ticker，不设全局 `force3D`）（第 17-19 行）；`prefers-reduced-motion` 下把 `duration` 归零并关掉 lagSmoothing（第 21-25 行）。
- **`initTheme`**（第 27-44 行）：读 `campus-light-mode` 决定 `dark` class / `data-light`，读 `campus-theme` 并在 `VALID_THEMES` 内且非 `default` 时加 `theme-${theme}`；读 `campus-glass-level` 设 `data-glass`；模块加载即调用（第 46 行）。
- **`setupCrashRecovery`**（第 48-114 行）：
  - 重载计数与上限：`crashCount` / `MAX_CRASH_RELOADS = 3`（第 49-50 行），`tryRecover` 延迟 1s 重载（第 52-60 行）。
  - 可见时 `#root` 无子节点视为渲染异常（第 62-69 行）。
  - `error` 事件里匹配 `GPU` / `WebGL` / `SharedArrayBuffer` 触发恢复（第 71-77 行）。
  - 心跳：1s 间隔刷新 `lastHeartbeatTime`，但 `!isRenderLoopAlive()`（rAF 停滞）时跳过心跳更新（第 93-103 行）；2s 间隔检查 `elapsed > 10000` 触发恢复（阈值由 `FE-A-11` 从 5s 放宽）（第 105-113 行）。
  - 可见性切换时暂停/恢复 `gsap.globalTimeline`（第 82-91 行）。
  - 模块加载即调用（第 116 行）。
- **根渲染**（第 118-132 行）：DEV 包 `React.StrictMode`、生产用 `React.Fragment`；`ReactDOM.createRoot` → `ErrorBoundary` → `LazyMotion features={domMax} strict` → `MotionConfig reducedMotion="user"` → `App`。

### auth 域

`auth/index.ts` (`tauri-app/frontend/src/auth/index.ts`) 导出 `DashboardPanel`、`AboutDialog`、`useAuth` 与全部 types（第 1-5 行）。

**AboutDialog.tsx (`tauri-app/frontend/src/auth/AboutDialog.tsx`)**

- **props 接口** `AboutDialogProps`（第 24-33 行）：字段 `open`、`onClose`、`openExternal?`、`onUpdateAvailable?`、`initialLatestVersion?`、`initialReleaseNotes?`、`initialUpdateAvailable?`。
- 仓库常量 `GITHUB_REPO = 'ikliml666/Wxxy-CampusLogin'`（第 34 行）；下载状态机类型 `DownloadState = 'idle' | 'selecting' | 'downloading' | 'done' | 'error'`（第 36 行）；核心优势数据 `CORE_FEATURES`（第 39-43 行，含 `icon`、`titleKey`、`descKey`）。
- 格式化工具 `formatSize`（第 45-50 行）、`formatSpeed`（第 52-56 行）、行内 markdown `renderInlineMarkdown`（第 58-69 行）。
- **组件状态**：`updateSource`（第 72 行，来自 store）、`checking`（第 75 行）、`showSponsor`（第 77 行）、`updateInfo`（第 78 行）、`downloadState`（第 79 行）、`progress`（第 80 行）、`mirrors`（第 81 行）、`downloadError`（第 82 行）、`installError`（第 83 行）、`downloadedFile`（第 84 行）、`checkError`（第 85 行）、`showMirrorList`（第 86 行）、`selectedMirror`（第 87 行）、`unlistenRef`（第 88 行）、`autoCheckedRef`（第 91 行）；`hasCachedResult`（第 93 行）。
- **行为回调**：
  - `handleCheckUpdate`（第 95-115 行，403/404 分特殊文案，依赖数组 `[api, onUpdateAvailable]` 缺 `t`）。
  - 切赞助页 effect（第 117-119 行）；缓存注入 effect（补 `lastCheckError: null` / `lastCheckTime: null`）（第 121-134 行）；单次自动检查 effect（第 136-141 行）；卸载清理监听 effect（第 143-147 行）。
  - `handleDownload`（第 149-174 行，注册 `onDownloadProgress` 再下载、finally 里注销）；`handleCancelDownload`（第 177-183 行）；`ensureFullUpdateInfo`（第 188-198 行，通知缓存路径缺 `sha256Checksum` 时重查）；`handleInstall`（第 200-209 行，失败显式写入 `installError`）；`openGithub`（第 211-213 行）；`windowsAsset`（第 215-217 行，只认 `.exe` / `.msi`）；`renderNotesLines`（第 220-246 行，前缀判断从长到短）；兜底资产 URL `defaultAssetUrl`（第 250 行，硬编码 `Wxxy-CampusLogin_${version}_x64-setup.exe`）；一键下载 `handleQuickDownload`（第 253-277 行，自动选非 GitHub 镜像，失败回退官方源）。
- **视图**：左栏含图标/版本/更新泡泡、`MascotFigure`、更新渠道二选一（写 `config.updateSource`）、检查按钮四态文案（检查失败态读 `updateInfo.lastCheckError || checkError`）、赞助与仓库链接；右栏含赞助内嵌页、idle 无更新态（含 `lastCheckError` 失败原因展示与重试按钮；其余情况为已最新态）、idle 有更新态（含通知全文渲染与镜像切换浮层）、`selecting`、`downloading`、`done`、`error`、返回按钮。

**useAuth.ts (`tauri-app/frontend/src/auth/useAuth.ts`)**：`useAuth`（第 7-33 行），`useShallow` 合并 `useAuthStore` 的 `isLoggingIn`/`isLoggingOut`/`status`/`doLogin`/`doLogout`/`checkOnline`（第 8-15 行），`configPortalUrl`（第 18 行），`handleOpenPortal`（第 20-23 行，缺省 `http://10.1.99.100`），`handleOpenSelfService`（第 25-27 行，固定 `http://10.1.80.200:8080/Self/login/?302=LI`）。

**types.ts (`tauri-app/frontend/src/auth/types.ts`)**：
- `PortalStatusResult`（第 1-6 行）：字段 `online: boolean`、`message?: string`、`reachable?: boolean`、`loginAvailable?: boolean`。
- `CommandResult`（第 8-12 行）：字段 `success: boolean`、`message?: string`、`data?: Record<string, unknown>`。
- `LoginResult extends CommandResult`（第 14-15 行，无新增字段）。

**测试**：`DashboardPanel.selfCards.test.tsx` (`tauri-app/frontend/src/auth/DashboardPanel.selfCards.test.tsx`) 4 个用例（第 105-172 行）——凭据齐备自动查询 + 掩码（第 106-124 行）、点眼睛一次验证后两卡共用且各卡独立显隐（第 126-152 行）、验证失败保持掩码（第 154-162 行）、无凭据不查询（第 164-172 行）。模块级门靠 `vi.resetModules()` 重置（第 79 行 beforeEach 中调用）。

### account 域

`account/index.ts` (`tauri-app/frontend/src/account/index.ts`) 导出 `AccountPanel`、`ChangePasswordDialog`、`useAccount` 与 types（第 1-5 行）。

**SelfServicePanel.tsx (`tauri-app/frontend/src/account/SelfServicePanel.tsx`)**

- **协议类型**：`SelfOnlineItem`（第 33-43 行，字段 `loginTime: string`、`ip: string`、`mac: string`、`useTime: string`、`downFlow: string`、`upFlow: string`、`hostName: string`、`terminalType: string`、`sessionId: string`）；`VpnStatus`（第 46-50 行，字段 `saved: boolean`、`accountMasked: string`、`viaVpn: boolean`）。
- **状态**：`addToast`（第 54 行）、`config`（第 57 行）、`passwordSaved`（第 58 行）、`onlineList` / `history` / `querying`（第 59-61 行）、`offlineSessionId` / `confirmTarget`（第 62-63 行）、`aliasTarget` / `deviceAliases`（第 65-66 行）、`vpnStatus` / `vpnCredOpen` / `vpnManageOpen` / `vpnClearConfirmOpen`（第 68-71 行）、日志卡日期范围与结果（第 73-77 行）、`mountedRef`（第 78 行）、`ensureSelfVerified = useSelfServiceVerify()`（第 80 行）、`autoRefreshedRef`（第 81 行）、`configLoaded`（第 83 行）。
- **生命周期**：卸载时 `resetSelfSessionGate()` 与清 `autoRefreshedRef`（第 85-93 行）。
- **凭据门槛**：`hasCred = config.user.trim().length > 0 && passwordSaved`（第 96 行）。
- **命令**：`fetchDashboard`（第 98-122 行，`querySelfDashboard`，验证门，静默降级）；切入面板自动验证并刷新（等 `configLoaded`）（第 128-133 行）；`handleOffline`（第 135-160 行，`selfOfflineSession`，门，成功后本地过滤）；`handleSaveAlias`（第 163-180 行，写 `config.deviceAliases`）；`refreshVpnStatus`（第 183-194 行）；`handleVpnTest`（第 201-210 行）；`handleVpnSave`（第 214-224 行）；`handleVpnToggle`（第 227-244 行）；`handleVpnClear`（第 247-263 行）；`fetchOnlineLog`（第 265-289 行，`querySelfOnlineLog`，门）。
- **视图**：在线信息卡（查询按钮、无凭据提示、在线设备 `OnlineDeviceCardStack`/`OnlineDeviceCard` 堆叠，支持别名改名）、空/待查询态、单位注释；历史与上网记录卡拆分为子组件 `<SelfOnlineRecords>`；注销确认框与设备改名弹窗 `DeviceAliasDialog`；VPN 凭据/管理/清除对话框。

**DeviceAliasDialog.tsx (`tauri-app/frontend/src/account/DeviceAliasDialog.tsx`)**

- **props 接口** `DeviceAliasDialogProps`（第 15-23 行）：`open: boolean`、`mac: string`（原始 12 位 hex MAC）、`initialName: string`、`onSave: (name: string) => Promise<void>`、`onCancel: () => void`。
- **状态**：`name`（第 27 行）、`saving`（第 28 行）。
- **行为**：打开时同步草稿；关闭时复位；提交防双击（第 30-46 行）；Dialog `onOpenChange` 在 saving 时阻止取消（第 49 行）。

**测试**：`AccountPanel.helloGate.test.tsx` (`tauri-app/frontend/src/account/AccountPanel.helloGate.test.tsx`) 3 个用例（第 90-148 行）——首次查询即验证（第 91-107 行）、通过后查看明文不再二次验证（第 109-130 行）、验证失败不执行且下次仍需验证（第 132-148 行）。模块级门靠 `vi.resetModules()` 重置（第 53 行 beforeEach 中调用）。

### monitor 域

`monitor/index.ts` (`tauri-app/frontend/src/monitor/index.ts`) 导出 `MonitorPanel`、`QualityPanel`、`SpeedTestPanel`、`LatencyPair`、`LatencyTimeline`、`NetworkQualityCapsule`、`StatusBar`、`useMonitor` 与全部 types（第 1-10 行）。具体面板实现未在本批源码中提供。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│                         App.tsx (AppInner)                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐           │
│  │ TitleBar │  │ StatusBar│  │ DockNav  │  │ RightPanel│           │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘           │
│                     ▼                                              │
│              AnimatePresence mode="wait"                           │
│              m.div key={deferredPanel}                             │
│              ┌──────────────────────────────────────────────────┐  │
│              │ ErrorBoundary                                    │  │
│              │ Suspense fallback={<PanelSkeleton />}            │  │
│              │ switch (deferredPanel):                          │  │
│              │ ├─ DashboardPanel (auth)                         │  │
│              │ ├─ AccountPanel (account)                        │  │
│              │ ├─ SelfServicePanel (account)                    │  │
│              │ ├─ NetworkPanel (network)                        │  │
│              │ ├─ MonitorPanel (monitor)                        │  │
│              │ ├─ QualityPanel (monitor)                        │  │
│              │ ├─ SettingsPanel (settings)                      │  │
│              │ ├─ LogPanel (shared)                             │  │
│              │ └─ SpeedTestPanel (monitor)                      │  │
│              └──────────────────────────────────────────────────┘  │
│  Dialogs (Suspense lazy loaded):                                   │
│  AboutDialog, ThemeDialog, OnboardingWizard, ConfirmDialog         │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### 流程一：面板切换与渲染

1. 用户点击 `DockNav` 触发 `handlePanelChange`（第 320-329 行）→ 置 `panelChangeLock` 防重入（60ms）、`setActivePanel(p)`、`safeStorage.set('campus-active-panel', p)`。
2. `activePanel` 经 `useDeferredValue` 得 `deferredPanel`（第 133 行），用于渲染标题与面板内容（低优先级，并发跳中间挂载）。
3. `switch (deferredPanel)`（第 334-419 行）选择面板组件，注入 props（来自 `useAuth` 等 hook 解构）。
4. 面板内容包 `ErrorBoundary` + `Suspense fallback={<PanelSkeleton />}`，外层 `AnimatePresence mode="wait"` 以 `panelVariants` 做滑入滑出（第 472-486 行）。

### 流程二：关于对话框更新检查与下载

1. `AboutDialog` 打开时若已有缓存版本 `initialLatestVersion` 则注入 `updateInfo`（第 121-134 行），否则自动 `handleCheckUpdate`（第 136-141 行）。
2. `handleCheckUpdate` 调用 `api.checkUpdate()`（第 99 行），成功则 `onUpdateAvailable(...)` 回写 `App.tsx` 的 store（第 534-542 行）；失败按 403/404 分类文案。
3. 一键下载 `handleQuickDownload` → `ensureFullUpdateInfo`（补 `sha256Checksum`）→ `api.getMirrorUrls` 选最优镜像 → `handleDownload` 注册进度监听并 `api.downloadUpdate`（第 253-277 行、149-174 行）。
4. 下载完成 `handleInstall` 调 `api.installUpdate`（第 200-209 行）。

### 流程三：自助服务查询与 Windows Hello 门

1. `SelfServicePanel` 挂载后若 `configLoaded && hasCred` 则自动 `fetchDashboard({ silent: true })`（第 128-133 行）。
2. `fetchDashboard` 先 `ensureSelfVerified()`（第 80 行，模块级会话门）通过后才 `tauriApiWithRetry.querySelfDashboard`（第 103 行）。
3. 查询成功写 `onlineList` / `history`；失败静默（silent）或 toast。
4. 注销设备 `handleOffline` 同样经门验证后 `selfOfflineSession`（第 140 行），本地过滤 session。
5. 设备改名 `handleSaveAlias` 写 `config.deviceAliases` 并 `saveConfigDirect`（第 163-180 行）。
6. VPN 凭据状态 `refreshVpnStatus`（第 183-194 行），保存/清除/切换经对应 API（第 214-263 行）。

### 流程四：测试中的验证门流程

- `DashboardPanel.selfCards.test.tsx`：模拟 `verifyWindowsIdentity` 与 `querySelfDashboard`/`querySelfOnlineLog`，验证凭据齐备自动查询、掩码、点眼睛触发验证后两卡共用、验证失败保持掩码（第 105-172 行）。
- `AccountPanel.helloGate.test.tsx`：模拟 `verifyWindowsIdentity` 与绑定查询，验证首次操作弹验证、通过后共用、失败不执行（第 90-148 行）。

## Connections

- 面板依赖的 store 与 hook 基础设施：[[desktop-frontend-hooks]]
- 面板复用的共享组件（`AnimatedCard` / `ConfirmDialog` / `MascotFigure` / `panel-content` 动画类）：[[desktop-frontend-shared]]
- 登录/注销与 Portal 协议侧实现：[[desktop-auth]]
- 配置字段语义、默认值与落盘语义：[[desktop-config]]
- 账号自助服务（`SelfServicePanel` 对应的后端命令实现）：[[desktop-account-selfservice]]
- 网络质量与延迟测试后端：[[desktop-network-quality]]
- 后台检测循环与校园网判定后端：[[desktop-monitor]]
- 更新检查/下载/安装与镜像源后端：[[desktop-helper-update]]
- 前端基建（i18n、主题、动画档位、渲染活性）：[[desktop-infra]]
- 应用生命周期（托盘、退出、关窗 flush 配置）：[[desktop-app-lifecycle]]
- IPC 命令面总览与事件清单：[[desktop-commands]]
- 平台差异（Windows 桌面专属能力）：[[desktop-platform]]

## Known Issues

### 路由与外壳

1. **关闭质量检测后 quality 面板渲染 `null`**：`App.tsx` 第 384 行在 `configEnableNetworkQuality === false` 时返回 `null`，若当前正停留在 `quality` 主区域空白。当前靠 `SettingsPanel` 联动切走兜底，但外部改配置关闭开关时仍会空白。
2. **`deferredPanel` 与 `activePanel` 的瞬时分歧**：主区内容与标题用 `deferredPanel`，而 `DockNav` 高亮用 `activePanel`——并发渲染延迟期间会出现「Dock 已切换、内容还是旧面板」，属预期取舍但排查视觉问题时需先想到。
3. **切换锁时长与动画时长硬耦合**：`App.tsx` 第 328 行的 60ms 锁硬编码，注释明确只覆盖 `mode="wait"` 退出动画 0.04s；任一侧改动会静默失配。
4. **`LogPanel` 注释与实现不一致**：`App.tsx` 第 21 行已静态导入 `LogPanel`，第 30 行注释称「仅低频的 LogPanel/对话框保留懒加载」，而第 47-49 行注释解释 `LogPanel` 从 lazy 回归静态导入的原因——第 30 行注释残留旧结论。
5. **生产环境不启用 `StrictMode`**：`main.tsx` 第 118-120 行仅 DEV 包 `StrictMode`，面板里 StrictMode 防御在生产不受考验。
6. **崩溃自恢复只处理三类特征**：`main.tsx` 第 71-77 行只匹配 `GPU` / `WebGL` / `SharedArrayBuffer`；其他渲染进程错误只靠心跳（阈值 10s 放宽权衡）。

### auth 域

7. **更新兜底资产名硬编码**：`auth/AboutDialog.tsx` 第 250 行兜底 URL 拼 `Wxxy-CampusLogin_${version}_x64-setup.exe`，与 Release 资产命名强耦合；缓存路径缺 `assets` 时依赖它命中，命名变更即 404。
8. **`windowsAsset` 只识别 `.exe` / `.msi`**：`auth/AboutDialog.tsx` 第 215-217 行，Release 若只发便携版 zip 或 APK，一键下载取不到资产。
9. **`handleCheckUpdate` 依赖数组缺 `t`**：`auth/AboutDialog.tsx` 第 115 行依赖为 `[api, onUpdateAvailable]`，但内部用 `t(...)`——切换语言后该回调仍用旧语言。
10. **`LoginResult` 为空扩展接口**：`auth/types.ts` 第 14-15 行 `interface LoginResult extends CommandResult {}` 无新增字段，部分 lint 规则会报 `no-empty-interface`。

### account 域

11. **会话门只按面板卸载重置**：`account/SelfServicePanel.tsx` 第 90 行卸载时 `resetSelfSessionGate()`；该面板静态导入但仅选中时挂载，切走再切回必然重验；若因 `ErrorBoundary` 兜底被替换，门不会被重置而状态丢失。
12. **Hello 门是模块级单例，无显式重置入口**：绑定门与会话门通过时间戳在 `account/selfServiceState.ts` 模块级变量维护，生产代码只能等 TTL 过期；测试必须 `vi.resetModules()` 重置（见 `AccountPanel.helloGate.test.tsx` 第 53 行）。向导内绑定验证与账户面板绑定卡共用同一门，任一处验证通过后另一处不再验证。

### monitor 域

13. **导出面板未提供实现**：`monitor/index.ts` 导出 `MonitorPanel` 等，但本批源码未包含其实现，相关已知问题暂不收录。
