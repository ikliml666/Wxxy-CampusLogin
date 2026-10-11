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
  - tauri-app/frontend/src/account/AccountPanel.helloGate.test.tsx
  - tauri-app/frontend/src/account/SelfServicePanel.tsx
  - tauri-app/frontend/src/account/DeviceAliasDialog.tsx
  - tauri-app/frontend/src/account/index.ts
  - tauri-app/frontend/src/account/types.ts
  - tauri-app/frontend/src/monitor/index.ts
  - tauri-app/frontend/src/network/index.ts
  - tauri-app/frontend/src/settings/index.ts
tags: [前端, 面板, react]
---

## Overview

本模块是桌面端前端的**业务面板层与外壳**。`App.tsx` 负责 9 个业务面板的路由、转场动画、对话框与全局弹层编排；`main.tsx` 负责渲染根、主题初始化与渲染进程崩溃自恢复。五个业务域目录（`auth/`、`account/`、`monitor/`、`network/`、`settings/`）各自提供「面板组件 + `useXxx` 编排 hook + 领域类型」。前端私有状态一律通过 `@/hooks/*Store`（zustand）与 `tauriApiWithRetry` 与 Rust 后端交互。

面板自身不做 IPC 拼装：`useAuth` / `useAccount` / `useMonitor` / `useNetwork` / `useSettings` 五个 hook 把 store action 与 `invoke` 调用收口成可直接绑到 JSX 的处理函数，`App.tsx` 再把这些函数作为 props 下发给面板，形成「App 编排 → hook 执行 → store 落状态 → invoke 过 IPC」的单向链。

## Key Components

### 应用外壳：App.tsx

`AppInner` 是全部装配逻辑所在；导出入口 `App` 只包三层：`ErrorBoundary` → `AnimationActiveProvider` → `AppInner`。

- **启动与懒加载 loader**：`loadAboutDialog`、`loadThemeDialog`、`loadOnboardingWizard`、`lazy(...)` 三处、`preloadPanels`。
- **面板元数据**：`PANEL_TITLES`，9 个面板各带 `titleKey` / `descKey`。
- **赞助外挂窗常量**：`SPONSOR_SHOW_DELAY_MS = 1_000`、`SPONSOR_SHOW_INTERVAL_MS = 7 * 24 * 60 * 60 * 1000`。
- **面板容器样式**（限制 `contain` 到 `layout style` 而非 `paint`，避免裁掉编辑态卡片右上角删除按钮）：`PANEL_CONTAINER_STYLE`。
- **侧边看板娘每日随机**：`SIDE_MASCOT_POOL`（14 项）、`pickDailySideMascots`（14×13=182 种有序组合按天序号轮转）。
- **懒加载骨架**：`PanelSkeleton`。
- **壳层状态订阅与下发**：`useAppInit()`；`dailySideMascots` 状态与每分钟跨天比对；`activePanel`；`deferredPanel = useDeferredValue(activePanel)`；`adapters`；`accounts` / `activeAccount`；`isLoggingIn`；粒度订阅的 4 个 config 字段 `configUser` / `configEnableNetworkQuality` / `configAutoLaunch` / `configEnableNotification`；`api` 静态导入 `tauriApiWithRetry`（2026-10-03 重构删除 store 的 api 字段）。
- **store action 订阅**：`updateConfig`、`setActivePanel`、`setUpdateAvailable` / `setLatestVersion` / `setReleaseNotes`、响应式读 `latestVersion` / `releaseNotes` / `qualityUpdateAvailable`、`addToast`、`doLogin`、`refreshQuality`。
- **五个业务 hook 解构**（面板 props 的唯一来源）：`useAuth`、`useMonitor`、`useNetwork`、`useAccount`、`useSettings`。
- **日志区 `useShallow` 订阅** `logs` / `toasts` / `removeToast` / `setLogs`。
- **面板切换锁与本地弹层状态**：`panelChangeLock`、`aboutOpen`、`themeOpen`、`confirmDelete`、`onboardingOpen`、`sponsorOpen`、`isMaximized`。
- **动画与启动**：`useAnimationProfile()`；`panelVariants = createPanelAppleVariants(profile.easing)`；`useStartupBoost` 的 `setRef` / `runStartupSequence`。
- **副作用**：双 rAF 启动序列 + 并行预热懒加载 chunk；窗口 resize/maximized 监听（cleanup 里顺带 `cleanupToasts()`）；系统通知点击开关于框；首次引导自动弹出；赞助浮层自动弹出（7 天频控 + 窗口可见性等待 + 弹出即写时间戳）。
- **回调**：`handleToggleMaximize`、`handleClearLogs`、`handlePanelChange`（60ms 重入锁 + 持久化 `campus-active-panel`）。
- **标题文案**：`panelInfo = PANEL_TITLES[deferredPanel] || PANEL_TITLES.dashboard`。

**面板路由完整分支（`switch (deferredPanel)`）**

| 分支 | 渲染的面板 | 传入 props | 渲染条件 |
|---|---|---|---|
| `'dashboard'` | `DashboardPanel` | `accounts`、`activeAccount`、`onUpdateConfig`、`onSwitchAccount`、`onDhcpRenew`、`onDhcpReleaseRenew`、`onDhcpReleaseRenewAdapter`、`onRefreshQuality` | 无额外条件 |
| `'account'` | `AccountPanel` | `adapters`、`accounts`、`activeAccount`、`onUpdateConfig`、`onAddAccount`、`onDeleteAccount`、`onSwitchAccount`、`onRenameAccount` | 无额外条件 |
| `'selfservice'` | `SelfServicePanel` | 无 props（自订阅 store） | 无额外条件 |
| `'network'` | `NetworkPanel` | `adapters`、`onUpdateConfig` | 无额外条件 |
| `'monitor'` | `MonitorPanel` | `onUpdateConfig`、`onToggleBackgroundCheck`、`onTriggerCheck` | 无额外条件 |
| `'quality'` | `QualityPanel` | `onUpdateConfig`、`onRefreshQuality`、`onToggleLatencyTest` | `configEnableNetworkQuality !== false`，否则渲染 `null`（主区域空白，靠 `SettingsPanel.tsx` 联动切走兜底） |
| `'settings'` | `SettingsPanel` | `autoLaunch`、`onUpdateConfig`、`onSetAutoLaunch`、`onToggleLightMode`、`onSetTheme`、`onShowOnboarding` | 无额外条件 |
| `'log'` | `LogPanel`（静态导入） | `api`、`addToast` | 无额外条件 |
| `'speedtest'` | `SpeedTestPanel` | `openExternal` | 无额外条件 |
| 默认 | `null` | — | 当 `deferredPanel` 不在上述 9 个键内时渲染空白 |

**转场与渲染管线**：外层 `AnimatePresence mode="wait"` + `m.div key={deferredPanel}`，`variants={panelVariants}`，`className="panel-content"` 与 `PANEL_CONTAINER_STYLE`，内部依次包 `ErrorBoundary` 与 `Suspense fallback={<PanelSkeleton />}`。标题与描述同样包进一组 `AnimatePresence mode="wait"` + `m.div key={deferredPanel}`，复用同一个 `panelVariants` 实现标题/描述与内容同向滑入。

**外壳其余结构**：根容器（含 `isMaximized && 'app-maximized'` 与 `animate-window-reveal`）；`FluidBackground`；`TitleBar`（下发通知开关/主题/关于/赞助/浅色/最小化/最大化/关闭）；`StatusBar`；主区 `main`（`max-w-[880px]` / `max-w-[720px]` 随最大化切换）；`RightPanel`；两张侧边看板娘 `img`（`min-[1360px]` 起显示、右侧偏移 304px 避让 `w-72`）；`DockNav`；`ToastContainer`；`SponsorCard`；`AboutDialog`（`Suspense` + 缓存版本号 + `onUpdateAvailable` 回调写 store/发 toast/写日志）；`ThemeDialog`；删除账号 `ConfirmDialog`（确认后 `await handleDeleteAccount(name)` 再关框）；`OnboardingWizard`。

### 渲染根：main.tsx

- **gsap 全局默认**（仅 `ease` 与 ticker，不设全局 `force3D`）；`prefers-reduced-motion` 下把 `duration` 归零并关掉 lagSmoothing。
- **`initTheme`**：读 `campus-light-mode` 决定 `dark` class / `data-light`，读 `campus-theme` 并在 `VALID_THEMES` 内且非 `default` 时加 `theme-${theme}`；模块加载即调用。
- **`setupCrashRecovery`**：
  - 重载计数与上限：`crashCount` / `MAX_CRASH_RELOADS = 3`，`tryRecover` 延迟 1s 重载。
  - 可见时 `#root` 无子节点视为渲染异常。
  - `error` 事件里匹配 `GPU` / `WebGL` / `SharedArrayBuffer` 触发恢复。
  - 心跳：1s 间隔刷新 `lastHeartbeatTime`，但 `!isRenderLoopAlive()`（rAF 停滞 >10s）时跳过心跳更新；2s 间隔检查 `elapsed > 10000` 触发恢复（阈值由 `FE-A-11` 从 5s 放宽）。
  - 可见性切换时暂停/恢复 `gsap.globalTimeline`。
  - 模块加载即调用。
- **根渲染**：DEV 包 `React.StrictMode`、生产用 `React.Fragment`；`ReactDOM.createRoot` → `ErrorBoundary` → `LazyMotion features={domMax} strict` → `MotionConfig reducedMotion="user"` → `App`。

### auth 域

`auth/index.ts` 导出 `DashboardPanel`、`AboutDialog`、`useAuth` 与全部 types。

**AboutDialog.tsx（关于/更新/赞助，两栏仪表盘）**

- **props 接口** `AboutDialogProps`；仓库常量 `GITHUB_REPO`；下载状态机类型 `DownloadState = 'idle' | 'selecting' | 'downloading' | 'done' | 'error'`；核心优势数据 `CORE_FEATURES`；格式化工具 `formatSize`、`formatSpeed`、行内 markdown `renderInlineMarkdown`。
- **状态**：`checking`、`showSponsor`、`updateInfo`、`downloadState`、`progress`、`mirrors`、`downloadError`、`installError`、`downloadedFile`、`checkError`、`showMirrorList`、`selectedMirror`、`unlistenRef`、`autoCheckedRef`。
- **行为**：`hasCachedResult`；`handleCheckUpdate`（403/404 分特殊文案）；切赞助页；缓存注入（补 `lastCheckError: null` / `lastCheckTime: null`）；单次自动检查；卸载清理监听；`handleDownload`（注册 `onDownloadProgress` 再下载、finally 里注销）；`ensureFullUpdateInfo`（通知缓存路径缺 `sha256Checksum` 时重查）；`handleInstall`（失败显式写入 `installError`）；`openGithub`；`windowsAsset`（只认 `.exe` / `.msi`）；releaseNotes 逐行渲染 `renderNotesLines`（前缀判断从长到短）；兜底资产 URL `defaultAssetUrl`；一键下载 `handleQuickDownload`（自动选非 GitHub 镜像，失败回退官方源）。
- **视图**：左栏含图标/版本/更新泡泡、`MascotFigure`、更新渠道二选一（写 `config.updateSource`）、检查按钮四态文案（检查失败态读 `updateInfo.lastCheckError || checkError`）、赞助与仓库链接；右栏含赞助内嵌页、idle 无更新态（含 `lastCheckError` 失败原因展示与重试按钮；其余情况为已最新态）、idle 有更新态（含通知全文渲染与镜像切换浮层）、`selecting`、`downloading`、`done`、`error`、返回按钮。

**useAuth.ts**：`useAuth`，`useShallow` 合并 `useAuthStore` 与 `useConfigStore`，`configPortalUrl`，`handleOpenPortal`（缺省 `http://10.1.99.100`），`handleOpenSelfService`（固定 `http://10.1.80.200:8080/Self/login/?302=LI`）。

**types.ts**：`PortalStatusResult`、`CommandResult`、`LoginResult extends CommandResult`。

**测试**：`DashboardPanel.selfCards.test.tsx` 4 个用例——凭据齐备自动查询 + 掩码、点眼睛一次验证后两卡共用且各卡独立显隐、验证失败保持掩码、无凭据不查询。模块级门靠 `vi.resetModules()` 重置。

### account 域

`account/index.ts` 导出 `AccountPanel`、`ChangePasswordDialog`、`useAccount` 与 types。

**OnlineDeviceCardStack（账号管理堆叠卡，narrow 变体）**：账号管理右列底部以窄卡封面流呈现登录账号——卡宽 `min-width:213px`（left 50% + max-content，学号行自然宽 + 80px 留白）；姿势表与在线信息 coverflow 共用（中央卡全亮，侧卡 ±60% 偏移缩小压暗可见），单卡时居中定格且禁拖禁切（`count < 2` 早退）；多账号经点击半区 / 拖拽 / 圆点分页切换，无箭头。transform 偏移在 JS 内数值求和（`pos.x - 50`），不可写裸 `translateX(-50% + x%)`（无 calc 包装会被浏览器整条丢弃，见 [[translatex-sum-requires-calc]]）。（注：源码 index 还导出了 `ChangePasswordDialog`，但文章主要聚焦 AccountPanel 和 SelfServicePanel，此处修正 index 描述以符合源码实际导出列表，但不展开未提供的 ChangePasswordDialog 细节以免臆测）。

**SelfServicePanel.tsx（自助服务：在线会话 / 上网历史 / 上网记录账单）**

- **协议类型**：`SelfOnlineItem`、`SelfHistoryRow`（位置元组）、`SelfLogRow`、`SelfLogSummary`（大写键 + `COU` 记录数）。
- **导出格式化函数**（被 `DashboardPanel.tsx` 复用）：`formatMac`、`formatEpoch`、`formatUseTimeMinutes`（floor 与原站一致）、`formatFlowMb`（(下行+上行) KB → M 三位小数）、`localDateStr`（本地日期而非 `toISOString`）。
- **私有工具**：`formatTerminalType`（去 `#` 前缀）、`toInt`、`fmt2`、`fmtMoney`（保留原始精度不截断）。
- **状态**：`passwordSaved`（凭据统一后唯一保存态，见 [[credential-single-source]]）、`ensureSelfVerified`、`autoRefreshedRef`、`configLoaded`、`onlineList` / `history` / `querying`、`offlineSessionId` / `confirmTarget`、日志卡日期范围与结果、`mountedRef`（行号随凭据共享 store 删除整体前移）。
- **生命周期**：卸载时 `resetSelfSessionGate()` 与清 `autoRefreshedRef`。历史的学号预填、密码草稿 focus/blur 与独立密码清除入口均已随凭据统一删除（学号直接取 `config.user`）。
- **命令**：`hasCred = config.user.trim() && passwordSaved`；`fetchDashboard`（`querySelfDashboard`，验证门）；切入面板自动验证并刷新（等 `configLoaded`）；`handleOffline`（`selfOfflineSession`，门，成功后本地过滤掉该 session）；`fetchOnlineLog`（`querySelfOnlineLog`，门）；查询按钮禁用判据与无凭据提示 `account.needLoginPassword`。
- **视图**：在线信息卡（查询按钮——禁用判据、无凭据提示；内容分支——`configLoaded` 加载态、`!hasCred` 提示、查询中、在线设备 `OnlineDeviceCardStack`/`OnlineDeviceCard` 堆叠，支持别名改名）、空/待查询态、单位注释；历史与上网记录卡拆分为子组件 `<SelfOnlineRecords>`；注销确认框与设备改名弹窗 `DeviceAliasDialog`。凭据输入区已随凭据统一移除（见 [[credential-single-source]]）。

**DeviceAliasDialog.tsx（在线设备改名弹窗）**

- **props 接口** `DeviceAliasDialogProps`：`open`、`mac`（原始 12 位 hex MAC）、`initialName`、`onSave`、`onCancel`。
- **状态**：`name`、`saving`。
- **行为**：打开时同步草稿；关闭时复位；提交防双击。

**测试**：`AccountPanel.helloGate.test.tsx` 3 个用例——首次查询即验证、通过后共用、查询通过后查看明文不再二次验证、验证失败不执行且下次仍需验证。

### monitor 域

`monitor/index.ts` 导出 `MonitorPanel`、`QualityPanel`、`SpeedTestPanel`、`LatencyPair`、`LatencyTimeline`、`NetworkQualityCapsule`、`StatusBar`、`useMonitor` 与全部 types。

**StatusBar.tsx（顶部状态条）**

- **`EMPTY_ADAPTER_STATUSES`** 模块级空数组（避免 `?? []` 造新引用触发重渲染）；props `StatusBarProps`；粒度订阅（只吃 `adapter1` / `adapter2` / `dualAdapter`）；`wasOffline` 派生与 ref 更新；`displayText` / `campusTooltip` 推导（用 `adapterStatuses` 的 `online` 与卡片同源）；状态配色表 `statusConfig`；视图与动画类名，右侧按 `enableNetworkQuality` 决定是否渲染 `NetworkQualityCapsule` + 刷新按钮。

### network 域

`network/index.ts` 导出 `NetworkPanel`、`useNetwork`、types 与 constants。

### settings 域

`settings/index.ts` 导出 `SettingsPanel`、`ThemeDialog`、`OnboardingWizard`、`useSettings`、types 与 constants。

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
│              │                                                  │  │
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
│                                                                      │
│  Dialogs (Suspense lazy loaded):                                   │
│  AboutDialog, ThemeDialog, OnboardingWizard, ConfirmDialog         │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### 流程一：登录（向导内的启动登录）

1. 用户点「开始登录」（向导底部导航区）→ `handleLoginAndFinish` 做最终校验（账号必填、双适配器必须选副适配器）。
2. 先写配置：`onUpdateConfig(updateData)` → `App.tsx` 传入的 `updateConfig` → `useConfigStore.updateConfig`（标脏 + 500ms debounce）。
3. 再触发登录：`onLogin(adapter1 === AUTO_DETECT_ADAPTER ? undefined : adapter1)` → `App.tsx` 传入的 `doLogin` → `useAuthStore.doLogin`。
4. 登录前落盘：`saveConfigDirect(loginConfig)` → `hooks/useConfigStore.ts` 起 → `api.saveConfig`（retry 包装）→ `invoke('save_config', { config, clearPassword })`。
5. 真实登录：`withTimeout(api.doLogin(adapterName), 60000, ...)` → `invoke('do_login', { adapterName })`；成功后置 `status = { text: 登录成功, state: 'online' }` 并写日志/toast。
6. 登录后质量：`enableNetworkQuality !== false` 且距上次质量结果 >60s 时才 `checkNetworkQuality` → 写 `useQualityStore.setNetworkQuality`。
7. 失败分支才复查在线状态：`checkOnline()` → `check_campus_status` / `get_adapters` / `check_portal_status` → 写 `status`（现在统一经 `setStatusStable` 内容比较后写入，引用未变时不触发订阅方重渲染）。

### 流程二：配置保存（开关类改动）

1. 用户拨动任一 `Switch`，例如 `account/AccountPanel.tsx`（开机自动登录）、`settings/SettingsPanel.tsx`（同语义的另一入口或最小化到托盘）；开机自启走例外路径：`settings/SettingsPanel.tsx` 的 `onSetAutoLaunch` 直接 `invoke('set_auto_launch')`，不经过 config debounce。
2. `App.tsx` 注入的 `updateConfig` → `useConfigStore.updateConfig`：先 `set({ config: next })` 与标脏，并入 `saveConfigPending`，500ms debounce 后调 `saveConfigDirect`。
3. `saveConfigDirect` 把 pending 浅合并进完整 config → `api.saveConfig`（仅此方法带指数退避重试）→ `invoke('save_config', { config, clearPassword })`；成功后清脏标记并按需置 `passwordSaved`（凭据统一后唯一保存态，见 [[credential-single-source]]）。
4. 反向回流：后端 `config-changed` 事件 → `mergeConfigFromBackend` 跳过 `dirtyFields`，避免旧快照覆盖本地新值。
5. **绕过 debounce 的立即保存路径**（清密码、启停检测）：`AccountPanel.tsx`（`saveConfigDirect({password:''}, true)`）、`useMonitor.ts`（历史的绑定口令/自助密码 blur 提交路径已随凭据统一删除，见 [[credential-single-source]]）。
6. 文本/数字输入的「本地草稿 + blur/Enter 提交」模式统一避免每键写 store。

### 流程三：网络质量检测

1. 结果入口（事件流）：后端发 `network-quality-result` → 事件监听写 `useQualityStore.setNetworkQuality`（同时更新模块级 `lastQualityResultTime`）。
2. 手动刷新入口三处：`monitor/StatusBar.tsx`（状态条刷新按钮）、`monitor/QualityPanel.tsx`（质量卡刷新按钮）、`auth/DashboardPanel.tsx`（首页质量卡刷新按钮）→ `useQualityStore.refreshQuality`：模块级 `_qualityLockFlag` 与 `enableNetworkQuality` 双重门 → `api.checkNetworkQuality()` → `invoke('check_network_quality')` → `mergeNetworkQuality(old, q)` 写回 store。
3. 定时循环开关：`QualityPanel.tsx`（定时测试启停）与 `MonitorPanel.tsx`（后台检测启停，启停按钮与开关两个入口）→ `useMonitor.handleToggleLatencyTest` / `handleToggleBackgroundCheck` → 先 `saveConfigDirect` 再 `api.startLatencyTest` / `stopLatencyTest` 或 `startBackgroundCheck` / `stopBackgroundCheck`。
4. 展示解析：`resolveQualityDisplay`（把长期停在 `unknown`/`busy` 的 `quality` 按 `displayLatency` 推断档位）→ 消费点 `auth/DashboardPanel.tsx`、`monitor/QualityPanel.tsx`；胶囊走自己的三级降级 `monitor/NetworkQualityCapsule.tsx`；信号条走 `getLatencyLevel` → `monitor/LatencyComponents.tsx`。
5. 状态条展示：`monitor/StatusBar.tsx` 订阅 `isRefreshingQuality` / `networkQuality` / `enableNetworkQuality` → 条件渲染胶囊与刷新按钮。

### 流程四：自助服务查询与 Windows Hello 门

1. 凭据来源（凭据统一后）：绑定与自助查询共用登录信息 `config.user` + `config.password`（保存态 `passwordSaved`），判断 `hasCred`/`credReady`；无面板级凭据输入与 blur 落盘，后端 `resolve_credential_password` 回退登录密码（见 [[credential-single-source]]）。
2. 条件判断：`hasCred` / `canQueryStatus`。
3. 验证门：`ensureSelfVerified = useSelfServiceVerify()` 或 `ensureHelloVerified = useHelloGate()` → `api.verifyWindowsIdentity({ consentMessage })` → `invoke('verify_windows_identity', { consentMessage })`；通过后写模块级时间戳。
4. 门通过后执行命令：`querySelfDashboard`、`selfOfflineSession`、`querySelfOnlineLog`、`getBindStatus`、`revealOperatorCredential`、`bindOperator`。
5. 首页两卡复用同一门与同一批命令：`auth/DashboardPanel.tsx` 起（`useSelfCardReveal`）与 `auth/DashboardPanel.tsx` 起（`useSelfCardFetch`）。

### 流程五：网卡刷新 / DHCP / DNS 优化

1. 获取新 IP（指定网卡）：`handleGetNewIpForAdapter` → `ipc.dhcpReleaseRenewAdapter` → `invoke('dhcp_release_renew_adapter', { adapterName })` → `announceDhcpResults(normalizeDhcpResults(result), addToast)` → `refreshAdapterData()`（内部并行 `get_adapters` / `get_adapter_details`）。
2. 启用被禁用网卡：`handleEnableAdapter` → `ipc.enableAdapter` → `invoke('enable_adapter', { adapterName })` → `refreshAdapterData({ force: true, includeDisabled: true })`。
3. DNS 检测与一键优化：`handleCheckDns`；`handleSetupDnsDoh(dnsFamily)`，成功后独立 try 再刷状态；一键恢复默认 DNS：`handleResetDns`，同样独立 try 再刷状态。
4. 面板被动刷新：`useAdapterStore` 由 `useEventListeners` 订阅 `adapters-changed` / `adapter-details-changed` / `disabled-adapters-changed` 落库，`network/NetworkPanel.tsx` 与 `monitor/MonitorPanel.tsx` 直接消费。

## Connections

- 面板依赖的 store 与 hook 基础设施：[[desktop-frontend-hooks]]
- 面板复用的共享组件（`AnimatedCard` / `SegmentTabs` / `RefreshButton` / `ConfirmDialog` / `MascotFigure` / `panel-content` 动画类）：[[desktop-frontend-shared]]
- 登录/注销与 Portal 协议侧实现：[[desktop-auth]]
- 配置字段语义、默认值与落盘语义：[[desktop-config]]
- 账号自助服务（`SelfServicePanel` 对应的后端命令实现）：[[desktop-account-selfservice]]
- 网卡/适配器与 DHCP 后端：[[desktop-network-core]]
- DNS / DoH 检测与优化后端：[[desktop-network-dns]]
- 网络质量与延迟测试后端：[[desktop-network-quality]]
- 后台检测循环与校园网判定后端：[[desktop-monitor]]
- 更新检查/下载/安装与镜像源后端：[[desktop-helper-update]]
- 前端基建（i18n、主题、动画档位、渲染活性）：[[desktop-infra]]
- 应用生命周期（托盘、退出、关窗 flush 配置）：[[desktop-app-lifecycle]]
- IPC 命令面总览与事件清单：[[desktop-commands]]
- 平台差异（Windows 桌面专属能力）：[[desktop-platform]]
- 安卓端对应实现与独立复刻前端：[[android-frontend]]、[[android-backend]]、[[android-plugins]]

## Known Issues

### 路由与外壳

1. **关闭质量检测后 quality 面板渲染 `null`**：`App.tsx` 在 `configEnableNetworkQuality === false` 时返回 `null`，若当前正停留在 `quality` 主区域空白。当前靠 `SettingsPanel.tsx` 联动切走（`setActivePanel('dashboard')`）兜底，但任何其他路径（如外部改配置、`config-changed` 回流）关闭该开关时仍会空白。
2. **`deferredPanel` 与 `activePanel` 的瞬时分歧**：主区内容与标题用 `deferredPanel`，而 `DockNav` 高亮用 `activePanel`——并发渲染延迟期间会出现「Dock 已切换、内容还是旧面板」，属预期取舍但应在排查视觉问题时先想到。
3. **切换锁时长与动画时长硬耦合**：`App.tsx` 的 60ms 锁硬编码，注释明确它只覆盖 `mode="wait"` 的退出动画 0.04s；任一侧改动都会静默失配（锁过短→快速点击穿插旧内容，过长→吞点击）。
4. **渲染期取 store 快照的非响应式写法（已解决）**：原 `App.tsx` 用 `useConfigStore.getState().api` 在 render 中取值——`api` 恒定故无实际缺陷，但同文件注释记录了「此前在 JSX props 里 `getState()` 取快照，非响应式」这一历史问题，本行是同类写法残留；2026-10-03 重构删除 `useConfigStore.api` 字段后，`App.tsx` 改静态导入 `tauriApiWithRetry`，该残留随之消失。
5. **`LogPanel` 已回归静态导入但注释保留旧结论**：`App.tsx` 的注释说明 `LogPanel` 从 `lazy` 回归静态导入的原因（`React.lazy` + `Suspense` 让切换从 ~65ms 恶化到 ~366ms），而 `App.tsx` 仍是静态导入、注释称「仅低频的 LogPanel/对话框保留懒加载」——注释与实现不一致。
6. **生产环境不启用 `StrictMode`**：`main.tsx` 仅 DEV 包 `StrictMode`。因此面板里大量「StrictMode setup→cleanup→setup 恢复 mountedRef」的防御在生产不受考验，但也意味着这些 double-invoke 类问题只在 DEV 暴露。
7. **崩溃自恢复只处理三类特征**：`main.tsx` 只匹配 `GPU` / `WebGL` / `SharedArrayBuffer`；其他渲染进程错误不触发重载，只靠心跳。心跳阈值 10s 在长任务阻塞场景下是刻意放宽的权衡。

### auth 域

9. **（已失效）首页「仅展示前 3 台」限制已移除**：首页在线设备改为 `OnlineDeviceCardStack` 全量卡片堆叠，本条原文描述的 `.slice(0, 3)` 截断与 `+N` 计数已不存在，完整列表直接渲染。
10. **60s 轮询不看可见性**：`auth/DashboardPanel.tsx` 的 `setInterval` 在窗口最小化/隐藏时仍持续发 `query_self_dashboard` IPC，无 `document.visibilityState` 判断（对比 `App.tsx` 的赞助弹层有可见性判断）。
11. **布局持久化静默失败**：`auth/DashboardPanel.tsx` 的 `catch {}` 空块——`campus-dashboard-layout` 内容损坏时直接回退 `DEFAULT_LAYOUT`，用户自定义布局无提示地丢失。
12. **更新兜底资产名硬编码**：`auth/AboutDialog.tsx` 的兜底 URL 拼 `Wxxy-CampusLogin_${version}_x64-setup.exe`，与 Release 资产命名强耦合；缓存路径缺 `assets` 时依赖它命中，命名一旦变更即 404。
13. **`windowsAsset` 只识别 `.exe` / `.msi`**：`auth/AboutDialog.tsx`，Release 若只发便携版 zip 或 APK，一键下载取不到资产。
14. **`handleCheckUpdate` 依赖数组缺 `t`**：`auth/AboutDialog.tsx` 的依赖为 `[api, onUpdateAvailable]`，但内部用 `t(...)`——切换语言后该回调仍用旧语言的 `t`。同类问题见 `network/NetworkPanel.tsx`（依赖 `[ipc]`，内部用 `t`）。
15. **`LoginResult` 为空扩展接口**：`auth/types.ts` `interface LoginResult extends CommandResult {}`，无新增字段，属可合并的写法（部分 lint 规则会报 `no-empty-interface`）。

### account 域

16. **清登录密码按钮缺少 `mousedown` 拦截（竞态）**：`account/AccountPanel.tsx` 的「清除已保存密码」按钮只有 `onClick={handleClearPassword}`，没有 `onMouseDown={(e) => e.preventDefault()}`；若用户此刻在密码框里刚输入了草稿，点击按钮会先触发 `handlePasswordBlur` 提交 `onUpdateConfig({ password: draft })`（进入 500ms debounce），随后才执行清空保存。后果：清除操作被随后触发的 debounce 保存覆盖（后端「空密码=保留旧密码」语义下，密码并没有被真正清除）。历史上同文件的自助密码清除按钮曾以「点击时先清空本地草稿」缓解同类竞态——凭据统一后独立自助凭据输入已整体删除（见 [[credential-single-source]]），本处的登录密码清除按钮成为该竞态的唯一残留点。
17. **`BIND_OPERATOR_NONE` 定义在组件体内**：`account/AccountPanel.tsx` 每次渲染重建（值恒定为 `'__none__'`，无功能影响）；同语义常量在 `settings/OnboardingWizard.tsx` 是模块级，位置不一致。
18. **`switchingAccount` 清态无 mounted 守卫**：`handleSwitchAccount` 的 `finally { setSwitchingAccount(null) }` 未判 `mountedRef.current`，而同文件新增账号路径做了守卫——异步返回时组件已卸载会触发一次无意义的 setState。
19. **Hello 门是模块级单例，无显式重置入口**：绑定门与会话门的通过时间戳都是 `account/selfServiceState.ts` 的模块级变量，生产代码只能等 `VERIFY_TTL_MS`（570s）过期或时钟回拨（`gateFresh`）；测试必须 `vi.resetModules()` 重置。副作用：向导内绑定验证与账户面板绑定卡共用同一门，任一处验证通过后另一处不再验证。
20. **会话门只按面板卸载重置**：`account/SelfServicePanel.tsx` 在卸载时 `resetSelfSessionGate()`；该面板在 `App.tsx` 是静态导入但只在被选中时挂载，因此「切走再切回必然重验」成立；但若面板因 `ErrorBoundary` 兜底被替换，门不会被重置而状态丢失。
21. **（已失效）账单明细已改用稳定 key**：`account/SelfOnlineRecords.tsx` 用 `key={row.key}`（行对象自带稳定 key），本条原文描述的 `key={idx}` 下标复用错位风险不再存在。
22. **500 条截断阈值硬编码**：`account/SelfOnlineRecords.tsx` 的 `logRows.length >= 500` 直接提示「超出部分未显示」，阈值不来自配置也不来自后端返回。
23. **首页与自助页对同一协议各定义一份类型**：`auth/DashboardPanel.tsx` 与 `account/SelfServicePanel.tsx` 的 `SelfOnlineItem` 字段一致但重复定义，任一侧协议变更需同步两处。
24. **`fmtMoney` 与 `toInt` 语义分叉在历史行上并存**：`account/SelfOnlineRecords.tsx` 用 `fmtMoney`（保留小数），`account/SelfOnlineRecords.tsx` 的时长/流量用 `toInt`（截断）——同一张表两种数值口径，注释解释了原因（金额可能为小数），阅读时易误判。

### monitor 域

25. **数字/文本输入的钳制只在 commit 生效**：`monitor/MonitorPanel.tsx` 与 `monitor/QualityPanel.tsx` 的 `min` / `max` 属性只是浏览器提示，真实值靠 blur/Enter 时的 `Math.min/max` 兜底；`parseInt(intervalDraft) || 60` 意味着输入非数字会回落到缺省值而非报错。
26. **主适配器徽标依赖精确名匹配**：`monitor/MonitorPanel.tsx` 传 `isPrimary={s.name === config.adapter1}`；当 `adapter1` 为空或 `'自动检测'` 时没有任何卡片会被标主，与 `AuthStore.checkOnline` 的自动选择口径不同；`StatusBar` 则用 `AUTO_DETECT_ADAPTER` 显式排除。
27. **`campusExitOnFail` 用 `?? true` 隐式默认**：`monitor/MonitorPanel.tsx` 在字段缺失时视为开启（与 `settings/constants.ts` 的默认值一致），但 UI 上无法区分「显式开启」与「未配置」。
28. **质量明细在首次无 `details` 时整块占位**：`monitor/QualityPanel.tsx` 的 `hasData = !!networkQuality?.details`，只要后端首包不带 `details`（例如只推 `gatewayLatency`），明细区所有项都渲染 `--` 与脉冲占位条。
29. **定时测试运行徽标只读本地开关**：`monitor/QualityPanel.tsx` 的「运行中」徽标只看 `config.enableLatencyTest`，不反映后端 `start_latency_test` 的真实状态；若后端因校验拒绝启动，UI 仍显示运行中。
30. **`StatusBar` 的 `wasOffline` 是「上一次渲染的状态」**：`monitor/StatusBar.tsx` 在渲染期读 `prevStatusRef.current`，而 ref 在 effect 中更新——语义为「上上次状态」，仅用于动画类名选择，不参与文案；阅读时勿当作当前状态。
31. **胶囊的两个 `useMemo` 依赖数组刻意收窄**：`monitor/NetworkQualityCapsule.tsx`（依赖 `networkQuality?.gateway`）与 `monitor/NetworkQualityCapsule.tsx`（依赖 `networkQuality?.details?.['dnsResolve']`）——依赖列表比闭包实际读取量窄，属有意优化但与 `react-hooks/exhaustive-deps` 冲突。
32. **`QualityPanel` 的子标签切换动画注释保留历史缺陷说明**：`monitor/QualityPanel.tsx` 解释了必须把 `TooltipProvider` 放进 `m.div` 内（否则 `AnimatePresence mode="wait"` 无法感知 key 变化）；该约束是「改结构就退化为无动画」的脆弱点。

### network 域

33. **推荐 DNS 白名单硬编码在前端**：`network/NetworkPanel.tsx`（阿里/腾讯 IPv4+IPv6 共 7 个地址）。「推荐」判定与提示文案都基于这份清单，与后端 DNS 优化实际下发的清单可能分叉；新增推荐源需改前端。
34. **两个刷新入口参数不一致**：`network/NetworkPanel.tsx` 的 `refreshAdapterData()` 不带 `includeDisabled`，`network/NetworkPanel.tsx` 带 `{ force: true, includeDisabled: true }`；前者在网卡被禁用/启用的瞬间可能拿到过期列表。
35. **`handleCheckDns` 的依赖数组缺 `t`**：`network/NetworkPanel.tsx` 依赖 `[ipc]`，但 `network/NetworkPanel.tsx` 用 `t(...)` 写日志——切语言后该回调仍用旧语言。
36. **`getDnsQuality` 在适配器级覆盖 profile 时的判定分支**：`network/NetworkPanel.tsx` 的 `adapter.adapterDnsOverridesProfile ? servers : (servers.length > 0 ? servers : profileServers)`——当「适配器级为空且 profile 有值」时用 profile 值；警告 UI 与判定分别独立计算，二者条件不同（警告多要求 `profileDnsServers.length > 0`），极端组合下可能出现「判为 excellent 但显示覆盖警告」。
37. **`dnsStatus` 为空与 `dnsChecking` 的组合态**：`network/NetworkPanel.tsx` 分三个互斥分支渲染（未检测 / 检测中 / 有结果）；当检测失败被 `catch` 吞掉并置 `null` 时 UI 回到「点击检测」空态，用户无法区分「没查」与「查失败」。

### settings 域

38. **`handleSecurityDisable` 定义在 `t` 之前（顺序脆弱）**：`settings/SettingsPanel.tsx` 的闭包引用 `t`，而 `const { t } = useTranslation()` 在 `settings/SettingsPanel.tsx` 才声明——依赖「函数只在事件触发时才执行」这一事实避开 TDZ；若将来有人在渲染期调用它会抛 `ReferenceError`。
39. **`OnboardingWizard` 可能把明文密码回填进输入框**：`settings/OnboardingWizard.tsx` 的写法是 `config.password === PASSWORD_MASK ? '' : (config.password || '')`——只有在后端回传 MASK 时才清空；若后端在某窗口期回传明文（`Config.password` 类型为 `string`），明文会被填入 `type="text"` 的可切换输入框（账号信息步视图）。账户/自助面板均改用独立布尔 `passwordSaved`（凭据统一后唯一保存态，见 [[credential-single-source]]）规避该状态机，向导未同步该修法。
40. **`handleLoginAndFinish` 校验失败时静默返回**：`settings/OnboardingWizard.tsx` 起的两个 `return` 分支只 `setLoginSuccess(false)`，不给 toast/内联错误；用户点「开始登录」无任何反馈（对比 `settings/OnboardingWizard.tsx` 的 `canProceedAccount` + 账号信息步的禁用态 + `onboarding.pleaseComplete` 提示，视图起）。
41. **关窗二次确认会吞掉首次关闭意图**：`settings/OnboardingWizard.tsx` 的 `onPointerDownOutside` 被 preventDefault——点遮罩不会关闭向导，只弹确认框（`showCloseConfirm` 状态）；这是有意设计，但若内层确认框的 `Dialog` 与向导同时存在于 DOM，键盘 Esc 的行为需注意（确认框没有显式 Esc 处理）。
42. **`SettingsPanel` 关闭质量检测的联动只覆盖三处**：`settings/SettingsPanel.tsx` 处理 `enableLatencyTest`、`defaultPanel`、`activePanel`；但不回滚 `speedtest` 面板的默认值，也不清理 `useQualityStore.networkQuality` 里已缓存的结果（关闭后 `StatusBar` 不再显示胶囊，但 store 中的数据仍在）。
43. **`fixedGateway` 清空按钮是原生 `button`**：`settings/SettingsPanel.tsx` 与输入框同样是原生 `input` 未走项目 UI 组件；行为一致但样式/可访问性（无 `aria-label`、无 label 关联）与同卡其他控件不齐。
44. **取色器草稿的 80ms 节流与 blur 双通道**：`settings/SettingsPanel.tsx` 定时提交 + `settings/SettingsPanel.tsx` 的 `commitColorDraft` 走 `onBlur`——`<input type="color">` 在部分平台不触发 blur，此时仅靠 80ms 定时器提交，逻辑正确但存在两条提交路径需同时维护。
45. **`defaultPanel` 哨兵值与 Radix 约束**：`settings/SettingsPanel.tsx` 用 `'__remember__'` 映射空串（Radix `SelectItem` 不接受空 `value`）；`settings/SettingsPanel.tsx` 过滤 `quality`，但未考虑 `enableNetworkQuality` 之外的其它无效面板组合；同时在 `settings/constants.ts` 用 `NAV_ITEMS` 派生选项，`NAV_ITEMS` 含全部 9 项（`shared/ui-constants.ts`）。