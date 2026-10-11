---
title: 桌面前端业务面板
type: module
source_files:
  - tauri-app/frontend/src/App.tsx
  - tauri-app/frontend/src/main.tsx
  - tauri-app/frontend/src/auth/AboutDialog.tsx
  - tauri-app/frontend/src/auth/DashboardPanel.tsx
  - tauri-app/frontend/src/auth/DashboardPanel.selfCards.test.tsx
  - tauri-app/frontend/src/auth/index.ts
  - tauri-app/frontend/src/auth/types.ts
  - tauri-app/frontend/src/auth/useAuth.ts
  - tauri-app/frontend/src/account/AccountPanel.tsx
  - tauri-app/frontend/src/account/AccountPanel.helloGate.test.tsx
  - tauri-app/frontend/src/account/SelfServicePanel.tsx
  - tauri-app/frontend/src/account/SelfServicePanel.test.tsx
  - tauri-app/frontend/src/account/DeviceAliasDialog.tsx
  - tauri-app/frontend/src/account/SelfOnlineCards.tsx
  - tauri-app/frontend/src/account/SelfOnlineRecords.tsx
  - tauri-app/frontend/src/account/selfServiceTypes.ts
  - tauri-app/frontend/src/account/index.ts
  - tauri-app/frontend/src/account/selfServiceState.ts
  - tauri-app/frontend/src/account/types.ts
  - tauri-app/frontend/src/account/useAccount.ts
  - tauri-app/frontend/src/monitor/LatencyComponents.tsx
  - tauri-app/frontend/src/monitor/LatencyTimeline.tsx
  - tauri-app/frontend/src/monitor/MonitorPanel.tsx
  - tauri-app/frontend/src/monitor/NetworkQualityCapsule.tsx
  - tauri-app/frontend/src/monitor/QualityPanel.tsx
  - tauri-app/frontend/src/monitor/SpeedTestPanel.tsx
  - tauri-app/frontend/src/monitor/StatusBar.tsx
  - tauri-app/frontend/src/monitor/index.ts
  - tauri-app/frontend/src/monitor/types.ts
  - tauri-app/frontend/src/monitor/useMonitor.ts
  - tauri-app/frontend/src/network/NetworkPanel.tsx
  - tauri-app/frontend/src/network/adapters.test.ts
  - tauri-app/frontend/src/network/adapters.ts
  - tauri-app/frontend/src/network/constants.ts
  - tauri-app/frontend/src/network/index.ts
  - tauri-app/frontend/src/network/types.ts
  - tauri-app/frontend/src/network/useNetwork.test.ts
  - tauri-app/frontend/src/network/useNetwork.ts
  - tauri-app/frontend/src/settings/OnboardingWizard.tsx
  - tauri-app/frontend/src/settings/SettingsPanel.tsx
  - tauri-app/frontend/src/settings/ThemeDialog.tsx
  - tauri-app/frontend/src/settings/constants.ts
  - tauri-app/frontend/src/settings/index.ts
  - tauri-app/frontend/src/settings/types.ts
  - tauri-app/frontend/src/settings/useSettings.ts
tags: [前端, 面板, react]
---

## Overview

本模块是桌面端前端的**业务面板层与外壳**：`App.tsx` 负责 9 个业务面板的路由、转场动画、对话框与全局弹层编排，`main.tsx` 负责渲染根、主题初始化与渲染进程崩溃自恢复。五个业务域目录（`auth/`、`account/`、`monitor/`、`network/`、`settings/`）各自提供「面板组件 + `useXxx` 编排 hook + 领域类型」，前端私有状态一律通过 `@/hooks/*Store`（zustand）与 `tauriApiWithRetry` 与 Rust 后端交互。

面板自身不做 IPC 拼装：`useAuth` / `useAccount` / `useMonitor` / `useNetwork` / `useSettings` 五个 hook 把 store action 与 `invoke` 调用收口成可直接绑到 JSX 的处理函数，`App.tsx` 再把这些函数作为 props 下发给面板，形成「App 编排 → hook 执行 → store 落状态 → invoke 过 IPC」的单向链。

## Key Components

### 应用外壳：App.tsx

`App.tsx:113` 的 `AppInner` 是全部装配逻辑所在；导出入口 `App`（`App.tsx:529-537`）只包三层：`ErrorBoundary` → `AnimationActiveProvider` → `AppInner`。

- 启动与懒加载 loader：`loadAboutDialog`（`App.tsx:49`）、`loadThemeDialog`（`App.tsx:50`）、`loadOnboardingWizard`（`App.tsx:51`）、`lazy(...)` 三处（`App.tsx:53-55`）、`preloadPanels`（`App.tsx:58-61`）。
- 面板元数据：`PANEL_TITLES`（`App.tsx:63-73`），9 个面板各带 `titleKey` / `descKey`。
- 赞助外挂窗常量：`SPONSOR_SHOW_DELAY_MS = 1_000`（`App.tsx:76`）、`SPONSOR_SHOW_INTERVAL_MS = 7 天`（`App.tsx:77`）。
- 面板容器样式（限制 `contain` 到 `layout style` 而非 `paint`，避免裁掉编辑态卡片右上角删除按钮）：`PANEL_CONTAINER_STYLE`（`App.tsx:81`）。
- 侧边看板娘每日随机：`SIDE_MASCOT_POOL`（`App.tsx:84-88`，14 项）、`pickDailySideMascots`（`App.tsx:90-98`，14×13=182 种有序组合按天序号轮转）。
- 懒加载骨架：`PanelSkeleton`（`App.tsx:101-111`）。
- 壳层状态订阅与下发：`useAppInit()`（`App.tsx:114`）；`dailySideMascots` 状态与每分钟跨天比对（`App.tsx:117-127`）；`activePanel`（`App.tsx:129`）；`deferredPanel = useDeferredValue(activePanel)`（`App.tsx:132`）；`adapters`（`App.tsx:133`）；`accounts` / `activeAccount`（`App.tsx:134-135`）；`isLoggingIn`（`App.tsx:136`）；粒度订阅的 4 个 config 字段 `configUser` / `configEnableNetworkQuality` / `configAutoLaunch` / `configEnableNotification`（`App.tsx:141-144`）；`api` 静态导入 `tauriApiWithRetry`（2026-10-03 重构删除 store 的 api 字段）。
- store action 订阅：`updateConfig`（`App.tsx:147`）、`setActivePanel`（`App.tsx:148`）、`setUpdateAvailable` / `setLatestVersion` / `setReleaseNotes`（`App.tsx:149-151`）、响应式读 `latestVersion` / `releaseNotes` / `qualityUpdateAvailable`（`App.tsx:153-155`）、`addToast`（`App.tsx:156`）、`doLogin`（`App.tsx:157`）、`refreshQuality`（`App.tsx:158`）。
- 五个业务 hook 解构（面板 props 的唯一来源）：`useAuth`（`App.tsx:160`）、`useMonitor`（`App.tsx:161`）、`useNetwork`（`App.tsx:162`）、`useAccount`（`App.tsx:163`）、`useSettings`（`App.tsx:164`）。
- 日志区 `useShallow` 订阅 `logs` / `toasts` / `removeToast` / `setLogs`（`App.tsx:166-173`）。
- 面板切换锁与本地弹层状态：`panelChangeLock`（`App.tsx:175`）、`aboutOpen`（`App.tsx:176`）、`themeOpen`（`App.tsx:177`）、`confirmDelete`（`App.tsx:178`）、`onboardingOpen`（`App.tsx:179`）、`sponsorOpen`（`App.tsx:180`）、`isMaximized`（`App.tsx:181`）。
- 动画与启动：`useAnimationProfile()`（`App.tsx:183`）、`panelVariants = createPanelAppleVariants(profile.easing)`（`App.tsx:184`）、`useStartupBoost` 的 `setRef` / `runStartupSequence`（`App.tsx:185`）。
- 副作用：双 rAF 启动序列 + 并行预热懒加载 chunk（`App.tsx:186-196`）；窗口 resize/maximized 监听（`App.tsx:198-222`，cleanup 里顺带 `cleanupToasts()`）；系统通知点击开关于框（`App.tsx:224-228`）；首次引导自动弹出（`App.tsx:230-236`，`campus-onboarding-done` 未写且无 `configUser` 时延迟 800ms）；赞助浮层自动弹出（`App.tsx:242-264`，7 天频控 + 窗口可见性等待 + 弹出即写时间戳）。
- 回调：`handleToggleMaximize`（`App.tsx:266-274`）、`handleClearLogs`（`App.tsx:276-278`）、`handlePanelChange`（`App.tsx:280-288`：60ms 重入锁 + 持久化 `campus-active-panel`）。
- 标题文案：`panelInfo = PANEL_TITLES[deferredPanel] || PANEL_TITLES.dashboard`（`App.tsx:290`）。

**面板路由完整分支（`App.tsx:293-378`，`switch (deferredPanel)`）**

| 分支 | 行号 | 渲染的面板 | 传入 props | 渲染条件 |
|---|---|---|---|---|
| `'dashboard'` | `App.tsx:294-306` | `DashboardPanel` | `accounts`（`AccountItem[]`）、`activeAccount`、`onUpdateConfig=updateConfig`、`onSwitchAccount=handleSwitchAccount`、`onDhcpRenew`、`onDhcpReleaseRenew`、`onDhcpReleaseRenewAdapter`、`onRefreshQuality=refreshQuality` | 无额外条件 |
| `'account'` | `App.tsx:308-320` | `AccountPanel` | `adapters`、`accounts`（`AccountItem[]`）、`activeAccount`、`onUpdateConfig`、`onAddAccount=handleAddAccount`、`onDeleteAccount=(id, displayName)=>setConfirmDelete({open:true,name:id,displayName})`、`onSwitchAccount`、`onRenameAccount=handleRenameAccount` | 无额外条件 |
| `'selfservice'` | `App.tsx:322-324` | `SelfServicePanel` | 无 props（自订阅 store） | 无额外条件 |
| `'network'` | `App.tsx:325-332` | `NetworkPanel` | `adapters`、`onUpdateConfig` | 无额外条件 |
| `'monitor'` | `App.tsx:333-341` | `MonitorPanel` | `onUpdateConfig`、`onToggleBackgroundCheck=handleToggleBackgroundCheck`、`onTriggerCheck=handleTriggerCheck` | 无额外条件 |
| `'quality'` | `App.tsx:342-350` | `QualityPanel` | `onUpdateConfig`、`onRefreshQuality`、`onToggleLatencyTest=handleToggleLatencyTest` | `configEnableNetworkQuality !== false`，否则渲染 `null`（主区域空白，靠 `SettingsPanel.tsx:582-592` 联动切走兜底） |
| `'settings'` | `App.tsx:351-362` | `SettingsPanel` | `autoLaunch`（`configAutoLaunch !== false`）、`onUpdateConfig`、`onSetAutoLaunch`、`onToggleLightMode`、`onSetTheme`、`onShowOnboarding` | 无额外条件 |
| `'log'` | `App.tsx:363-370` | `LogPanel`（静态导入） | `api`、`addToast` | 无额外条件 |
| `'speedtest'` | `App.tsx:371-377` | `SpeedTestPanel` | `openExternal=(url)=>api.openExternal?.(url)` | 无额外条件 |
| 默认（`default` 不存在） | — | `panelContent` 初值 `null`（`App.tsx:292`） | — | 当 `deferredPanel` 不在上述 9 个键内时渲染空白 |

**转场与渲染管线**：外层 `AnimatePresence mode="wait"` + `m.div key={deferredPanel}`（`App.tsx:426-440`），`variants={panelVariants}`（`App.tsx:429`，来自 `lib/animations.ts:19-36`），`className="panel-content"` 与 `PANEL_CONTAINER_STYLE`（`App.tsx:433-434`），内部依次包 `ErrorBoundary`（`App.tsx:436`）与 `Suspense fallback={<PanelSkeleton />}`（`App.tsx:437`）。标题与描述同样包进一组 `AnimatePresence mode="wait"` + `m.div key={deferredPanel}`（`App.tsx:411-422`），复用同一个 `panelVariants` 实现标题/描述与内容同向滑入（历史实现是仅用 `key` 强制重挂 + `slideDirection` 方向参数，方向信号已在死代码清理 `1dda320` 中删除）。

**外壳其余结构**：根容器 `App.tsx:381`（含 `isMaximized && 'app-maximized'` 与 `animate-window-reveal`）；`FluidBackground`（`App.tsx:382`）；`TitleBar`（`App.tsx:385-398`，`setRef('titleBar')`，下发通知开关/主题/关于/赞助/浅色/最小化/最大化/关闭）；`StatusBar`（`App.tsx:400-405`，`setRef('statusBar')`）；主区 `main`（`App.tsx:407-442`，`max-w-[880px]` / `max-w-[720px]` 随最大化切换，`App.tsx:408`）；`RightPanel`（`App.tsx:443-447`）；两张侧边看板娘 `img`（`App.tsx:453-468`，`min-[1360px]` 起显示、右侧偏移 304px 避让 `w-72`）；`DockNav`（`App.tsx:470-473`）；`ToastContainer`（`App.tsx:475`）；`SponsorCard`（`App.tsx:477`）；`AboutDialog`（`App.tsx:479-497`，`Suspense` + 缓存版本号 + `onUpdateAvailable` 回调写 store/发 toast/写日志）；`ThemeDialog`（`App.tsx:499-506`）；删除账号 `ConfirmDialog`（`App.tsx:508-515`，确认后 `await handleDeleteAccount(name)` 再关框）；`OnboardingWizard`（`App.tsx:517-526`）。

### 渲染根：main.tsx

- gsap 全局默认（仅 `ease` 与 ticker，不设全局 `force3D`）：`main.tsx:17-19`；`prefers-reduced-motion` 下把 `duration` 归零并关掉 lagSmoothing：`main.tsx:21-25`。
- `initTheme`（`main.tsx:27-41`）：读 `campus-light-mode` 决定 `dark` class / `data-light`，读 `campus-theme` 并在 `VALID_THEMES` 内且非 `default` 时加 `theme-${theme}`；模块加载即调用（`main.tsx:43`）。
- `setupCrashRecovery`（`main.tsx:45-111`）：
  - 重载计数与上限：`crashCount` / `MAX_CRASH_RELOADS = 3`（`main.tsx:46-47`），`tryRecover` 延迟 1s 重载（`main.tsx:49-57`）。
  - 可见时 `#root` 无子节点视为渲染异常（`main.tsx:59-66`）。
  - `error` 事件里匹配 `GPU` / `WebGL` / `SharedArrayBuffer` 触发恢复（`main.tsx:68-74`）。
  - 心跳：1s 间隔刷新 `lastHeartbeatTime`，但 `!isRenderLoopAlive()`（rAF 停滞 >10s）时跳过心跳更新（`main.tsx:90-100`）；2s 间隔检查 `elapsed > 10000` 触发恢复（`main.tsx:102-110`，阈值由 `FE-A-11` 从 5s 放宽）。
  - 可见性切换时暂停/恢复 `gsap.globalTimeline`（`main.tsx:79-88`）。
  - 模块加载即调用（`main.tsx:113`）。
- 根渲染：DEV 包 `React.StrictMode`、生产用 `React.Fragment`（`main.tsx:115-117`）；`ReactDOM.createRoot` → `ErrorBoundary` → `LazyMotion features={domMax} strict` → `MotionConfig reducedMotion="user"` → `App`（`main.tsx:119-129`）。

### auth 域

`auth/index.ts:1-5` 导出 `DashboardPanel`、`AboutDialog`、`useAuth` 与全部 types。

**DashboardPanel.tsx（首页卡片面板，支持编辑/排序/增删）**

- 卡片标识与元数据：`CardId`（`DashboardPanel.tsx:39`，`'quickActions' | 'accountManage' | 'selfOnline' | 'selfLog' | 'networkQuality'`）；`CardDef`（`DashboardPanel.tsx:41-45`）；`ALL_CARDS`（`DashboardPanel.tsx:47-53`，5 张卡）；`CARD_MAP`（`DashboardPanel.tsx:55`）；`DEFAULT_LAYOUT`（`DashboardPanel.tsx:57`）；布局读写 `loadLayout`（`DashboardPanel.tsx:59-68`，非法内容回退默认）/ `saveLayout`（`DashboardPanel.tsx:70-72`，键 `campus-dashboard-layout`）。
- 子卡片组件：
  - `QuickActionsCard`（`DashboardPanel.tsx:86-263`）：DHCP 续租（`DashboardPanel.tsx:112-114`）、获取新 IP-全部（`DashboardPanel.tsx:116-118`）、获取新 IP-指定适配器（`DashboardPanel.tsx:120-122`）；双适配器时的适配器菜单（portal 到 body + 视口坐标 fixed 定位，`DashboardPanel.tsx:128-141`、`DashboardPanel.tsx:201-257`）；主/副适配器解析 `resolved`（`DashboardPanel.tsx:146`，来自 `resolveAdapterNames`）。
  - `AccountManageCard`（`DashboardPanel.tsx:265-339`）：`accounts: AccountItem[]`，列表按 `item.id` 匹配激活态、展示 `item.displayName`（当前账号显示名列表缺失时兜底 id）；`switchingAccount` 单槽位切换中态、`mountedRef` StrictMode 复原、切换后 500ms 清态、`otherAccounts` 按 `id` 派生。
  - `NetworkQualityCard`（`DashboardPanel.tsx:341-381`）：质量态用 `resolveQualityDisplay`（`DashboardPanel.tsx:345`）、`QUALITY_CONFIG` 兜底 `unknown`（`DashboardPanel.tsx:347-349`）、刷新按钮（`DashboardPanel.tsx:364-368`）、`LatencyPair`（`DashboardPanel.tsx:373-377`）。
  - 自助卡共用 hook `useSelfCardReveal`（`DashboardPanel.tsx:397-408`）：凭据判断 `hasCred = !!config.user && passwordSaved`（`DashboardPanel.tsx:401`，凭据统一后以登录密码保存态为判据，见 [[credential-single-source]]）、查看需过 Hello 门。
  - 自助卡自动查询骨架 `useSelfCardFetch<T>`（`DashboardPanel.tsx:413` 起）：`data` / `querying` / `loadError`、`fetchLockRef` 防重入（`DashboardPanel.tsx:420`）、`fetchedForRef` 保证「同一学号只自动查一次」（`DashboardPanel.tsx:421`、`:448-449`）。
  - `SelfOnlineCard`：数据源 `querySelfDashboard({ account, password: '' })`（`DashboardPanel.tsx:494`，凭据统一后密码恒传空串、由后端回退登录密码，见 [[credential-single-source]]）；凭据入口 `useSelfCardReveal`（`:475`）、自动查询 `useSelfCardFetch`（`:499`，`hasCred` 门 effect `:504-507`）；注销单会话 `selfOfflineSession`（`DashboardPanel.tsx:514`）；`hasCred` 渲染门（`:587`、`:596`）；60s 轮询 `setInterval`（`:505`）；在线设备以 `OnlineDeviceCardStack`/`OnlineDeviceCard` 全量卡片堆叠渲染（`:549-568`，掩码由 `revealed` 控制 `:554-556`），支持单台注销确认（`:603`）与设备别名改名 `DeviceAliasDialog`（`:608-614`）。
  - `SelfLogCard`（`DashboardPanel.tsx:628` 起）：今日记录 `querySelfOnlineLog`（`DashboardPanel.tsx:631`，密码恒传空串）、自动查询 `useSelfCardFetch`（`DashboardPanel.tsx:637`）；只取前 `SELF_LOG_LIMIT = 5` 条；`hasCred` 渲染门（`:684`、`:693`）。
- 卡片分发 `renderCard`（`DashboardPanel.tsx:672-696`）：`noAnim = editing`、`noEnter = !editing`（`DashboardPanel.tsx:673-674`）——编辑态关掉动画以免拖拽冲突，非编辑态关入场动画。
- 主组件 `DashboardPanel`（`DashboardPanel.tsx:698-816`）：`cards` 初值 `loadLayout`（`DashboardPanel.tsx:700`）、`editing`（`DashboardPanel.tsx:701`）、订阅 `bgStatus`（`DashboardPanel.tsx:702`，仅传参、卡片内不使用）、`networkQuality` / `isRefreshingQuality`（`DashboardPanel.tsx:703-704`）、`adapters`（`DashboardPanel.tsx:705`）、`config` 浅比较订阅（`DashboardPanel.tsx:708`）、持久化（`DashboardPanel.tsx:710`）、`handleAddCard` / `handleRemoveCard`（`DashboardPanel.tsx:712-718`）、`availableCards`（`DashboardPanel.tsx:720-726`，关闭质量检测时剔除质量卡）、`visibleCards`（`DashboardPanel.tsx:728-733`）、编辑模式 `Reorder.Group` + `Reorder.Item`（`DashboardPanel.tsx:766-795`，含删除按钮 `DashboardPanel.tsx:787-792`）、非编辑模式 `card-enter` 逐张入场（`DashboardPanel.tsx:796-804`）、空态（`DashboardPanel.tsx:806-812`）。

**AboutDialog.tsx（关于/更新/赞助，两栏仪表盘）**

- props 接口 `AboutDialogProps`（`AboutDialog.tsx:24-32`）；仓库常量 `GITHUB_REPO`（`AboutDialog.tsx:34`）；下载状态机类型 `DownloadState = 'idle' | 'selecting' | 'downloading' | 'done' | 'error'`（`AboutDialog.tsx:36`）；核心优势数据 `CORE_FEATURES`（`AboutDialog.tsx:39-43`）；格式化工具 `formatSize`（`AboutDialog.tsx:45-50`）、`formatSpeed`（`AboutDialog.tsx:52-56`）、行内 markdown `renderInlineMarkdown`（`AboutDialog.tsx:58-69`）。
- 状态：`checking`（`AboutDialog.tsx:75`）、`showSponsor`（`AboutDialog.tsx:77`）、`updateInfo`（`AboutDialog.tsx:78`）、`downloadState`（`AboutDialog.tsx:79`）、`progress`（`AboutDialog.tsx:80`）、`mirrors`（`AboutDialog.tsx:81`）、`downloadError`（`AboutDialog.tsx:82`）、`installError`（`AboutDialog.tsx:83`）、`downloadedFile`（`AboutDialog.tsx:84`）、`checkError`（`AboutDialog.tsx:85`）、`showMirrorList`（`AboutDialog.tsx:86`）、`selectedMirror`（`AboutDialog.tsx:87`）、`unlistenRef`（`AboutDialog.tsx:88`）、`autoCheckedRef`（`AboutDialog.tsx:91`）。
- 行为：`hasCachedResult`（`AboutDialog.tsx:93`）；`handleCheckUpdate`（`AboutDialog.tsx:95-115`，403/404 分特殊文案）；切赞助页（`AboutDialog.tsx:117-119`）；缓存注入（`AboutDialog.tsx:121-134`，补 `lastCheckError: null` / `lastCheckTime: null`）；单次自动检查（`AboutDialog.tsx:136-141`）；卸载清理监听（`AboutDialog.tsx:143-147`）；`handleDownload`（`AboutDialog.tsx:149-174`，注册 `onDownloadProgress` 再下载、finally 里注销）；`ensureFullUpdateInfo`（`AboutDialog.tsx:179-189`，通知缓存路径缺 `sha256Checksum` 时重查）；`handleInstall`（`AboutDialog.tsx:191-200`，失败显式写入 `installError`）；`openGithub`（`AboutDialog.tsx:202-204`）；`windowsAsset`（`AboutDialog.tsx:206-208`，只认 `.exe` / `.msi`）；releaseNotes 逐行渲染 `renderNotesLines`（`AboutDialog.tsx:211-237`，前缀判断从长到短）；兜底资产 URL `defaultAssetUrl`（`AboutDialog.tsx:241`）；一键下载 `handleQuickDownload`（`AboutDialog.tsx:244-268`，自动选非 GitHub 镜像，失败回退官方源）。
- 视图：左栏（`AboutDialog.tsx:281-367`）含图标/版本/更新泡泡（`AboutDialog.tsx:293-297`）、`MascotFigure`（`AboutDialog.tsx:299`）、更新渠道二选一（`AboutDialog.tsx:308-330`，写 `config.updateSource`）、检查按钮四态文案（`AboutDialog.tsx:344-346`，检查失败态读 `updateInfo.lastCheckError || checkError`）、赞助与仓库链接（`AboutDialog.tsx:350-366`）；右栏（`AboutDialog.tsx:384-681`）含赞助内嵌页（`AboutDialog.tsx:373-422`）、idle 无更新态（`AboutDialog.tsx:426-469`，含 `lastCheckError` 失败原因展示与重试按钮；其余情况为已最新态）、idle 有更新态（`AboutDialog.tsx:472-573`，含通知全文渲染与镜像切换浮层 `AboutDialog.tsx:509-570`）、`selecting`（`AboutDialog.tsx:576-581`）、`downloading`（`AboutDialog.tsx:584-610`）、`done`（`AboutDialog.tsx:613-637`）、`error`（`AboutDialog.tsx:640-665`）、返回按钮（`AboutDialog.tsx:668-679`）。

**useAuth.ts**：`useAuth`（`useAuth.ts:6-36`），`useShallow` 合并 `useAuthStore`（`useAuth.ts:7-14`）与 `useConfigStore`（`useAuth.ts:15-17`），`configPortalUrl`（`useAuth.ts:20`），`handleOpenPortal`（`useAuth.ts:22-25`，缺省 `http://10.1.99.100`），`handleOpenSelfService`（`useAuth.ts:27-29`，固定 `http://10.1.80.200:8080/Self/login/?302=LI`）。

**types.ts**：`PortalStatusResult`（`auth/types.ts:1-6`）、`CommandResult`（`auth/types.ts:8-12`）、`LoginResult extends CommandResult`（`auth/types.ts:14`）。

**测试**：`DashboardPanel.selfCards.test.tsx` 4 个用例——凭据齐备自动查询 + 掩码（`DashboardPanel.selfCards.test.tsx:106-121`）、点眼睛一次验证后两卡共用且各卡独立显隐（`DashboardPanel.selfCards.test.tsx:123-149`）、验证失败保持掩码（`DashboardPanel.selfCards.test.tsx:151-159`）、无凭据不查询（`DashboardPanel.selfCards.test.tsx:161-169`）。模块级门靠 `vi.resetModules()` 重置（`DashboardPanel.selfCards.test.tsx:79`）。

### account 域

`account/index.ts:1-4` 导出 `AccountPanel`、`useAccount` 与 types。

**AccountPanel.tsx（登录信息 / 自动化开关 / 运营商绑定 / 账号管理）**

- props `AccountPanelProps`（`AccountPanel.tsx:35-45`，2026-09 新增 `onRenameAccount`）。
- 状态与草稿：`passwordSaved`（`AccountPanel.tsx:59`）、`config` 浅比较订阅、`newAccountName` / `showAddInput` / `showPassword` / `passwordFocused`、`passwordDraft`（避免聚焦中被打断）、`usernameDraft`、`mountedRef`。
- 密码显示三态推导 `displayPassword`（`AccountPanel.tsx:84`）；`handlePasswordFocus` / `handlePasswordBlur`（`:90`/:`95` 起）；`handleClearPassword`（`AccountPanel.tsx:105`，走 `saveConfigDirect({password:''}, true)` 显式清空）；`commitUsername`（`AccountPanel.tsx:112`）。历史独立的自助密码清除入口已随凭据统一删除（见 [[credential-single-source]]）。
- 新增账号 `handleAddAccount`（`AccountPanel.tsx:120`，长度 ≤32 且 `^[a-zA-Z0-9_\u4e00-\u9fa5-]+$`）；账号切换 `handleSwitchAccount` 带 `switchingAccount` 防重（`AccountPanel.tsx:137`）。
- **改名（R3，2026-09 新增）**：`renamingId` / `renameDraft` / `renaming` 三态（`AccountPanel.tsx:156-158`）驱动列表内联编辑；提交 trim 后调 `onRenameAccount(id, name)`，显示名校验（trim 后 1..=32、空/超长 toast `account.invalidDisplayName`）由后端 `rename_account` 把关、前端仅预检；成功退出编辑态，账号列表与激活态由 `useAccount.handleRenameAccount` 统一刷新。
- 运营商绑定（凭据统一后）：`BIND_OPERATOR_NONE = '__none__'`（`AccountPanel.tsx:182`）；绑定卡不再有独立凭据输入——学号取 `config.user`、密码由后端回退登录密码（`credReady = config.user.trim() && passwordSaved`，`AccountPanel.tsx:206`，见 [[credential-single-source]]）。
- 绑定与查询条件：`canBind`（`AccountPanel.tsx:208-209`）/ `canQueryStatus`（`AccountPanel.tsx:213`，均以 `credReady` 为前置；手机号 `^1\d{10}$`）。历史的绑定口令草稿、blur 落盘与回退提交路径已随凭据统一删除。
- Hello 门与后端命令：`ensureHelloVerified = useHelloGate()`（`AccountPanel.tsx:217`）；`fetchBindStatus`（`AccountPanel.tsx:219`，`getBindStatus`，门 `:221`）；`ensureRevealVerified = useHelloGate({ ignoreToggle: true })`（`AccountPanel.tsx:249`）；`handleReveal`（`AccountPanel.tsx:250`，`revealOperatorCredential`，门 `:257`）；`handleBindOperator`（`AccountPanel.tsx:276`，`bindOperator`，门 `:278`，成功后清 SMS 口令并自动刷新状态区 `:294`）。
- 视图四块：登录信息卡（标题描述带当前账号**显示名** `activeDisplayName`；用户名草稿输入、密码框与眼睛、运营商选择、主适配器选择）；自动化开关卡（五个开关：`autoLoginOnStart` / `autoExitAfterLogin` / `autoExitOnOnline` / `autoLoginOnPreparation` / `enableNightOperatorSwitch`——2026-09-19 新增夜切开关，安卓同位，见 [[decisions/night-operator-switch]]）；绑定卡（状态区、查询按钮、运营商/手机/短信口令输入、提交按钮——学号/密码不再单独输入，凭据统一取自登录信息）；账号管理卡（新增输入、**堆叠卡 narrow**：账号列表渲染为 `OnlineDeviceCardStack variant="narrow"` 窄卡封面流（2026-10-11 起，卡宽 min 213px、姿势同 coverflow 侧卡可见压暗、点击半区/拖拽/圆点切换无箭头、单卡居中且禁拖禁切，双端同构见 [[learnings/translatex-sum-requires-calc]]），卡内容含**切换/改名/删除**、空态）。

**SelfServicePanel.tsx（自助服务：在线会话 / 上网历史 / 上网记录账单）**

- 协议类型：`SelfOnlineItem`（`SelfServicePanel.tsx:17-27`）、`SelfHistoryRow`（`SelfServicePanel.tsx:29`，位置元组）、`SelfLogRow`（`SelfServicePanel.tsx:33-46`）、`SelfLogSummary`（`SelfServicePanel.tsx:48-57`，大写键 + `COU` 记录数）。
- 导出格式化函数（被 `DashboardPanel.tsx:34` 复用）：`formatMac`（`SelfServicePanel.tsx:59-62`）、`formatEpoch`（`SelfServicePanel.tsx:64-69`）、`formatUseTimeMinutes`（`SelfServicePanel.tsx:83-86`，floor 与原站一致）、`formatFlowMb`（`SelfServicePanel.tsx:89-93`，(下行+上行) KB → M 三位小数）、`localDateStr`（`SelfServicePanel.tsx:105-108`，本地日期而非 `toISOString`）。
- 私有工具：`formatTerminalType`（`SelfServicePanel.tsx:72-75`，去 `#` 前缀）、`toInt`（`SelfServicePanel.tsx:77-80`）、`fmt2`（`SelfServicePanel.tsx:96`）、`fmtMoney`（`SelfServicePanel.tsx:99-103`，保留原始精度不截断）。
- 状态：`passwordSaved`（`SelfServicePanel.tsx:49`，凭据统一后唯一保存态，见 [[credential-single-source]]）、`ensureSelfVerified`（`SelfServicePanel.tsx:66`）、`autoRefreshedRef`（`SelfServicePanel.tsx:67`）、`configLoaded`（`SelfServicePanel.tsx:69`）、`onlineList` / `history` / `querying`、`offlineSessionId` / `confirmTarget`、日志卡日期范围与结果、`mountedRef`（行号随凭据共享 store 删除整体前移）。
- 生命周期：卸载时 `resetSelfSessionGate()` 与清 `autoRefreshedRef`（`SelfServicePanel.tsx:76-77`）。历史的学号预填、密码草稿 focus/blur 与独立密码清除入口均已随凭据统一删除（学号直接取 `config.user`）。
- 命令：`hasCred = config.user.trim() && passwordSaved`（`SelfServicePanel.tsx:82`）；`fetchDashboard`（`SelfServicePanel.tsx:84-106`，`querySelfDashboard`，验证门 `:86`）；切入面板自动验证并刷新（注释 `:110-113`、effect `SelfServicePanel.tsx:114-117`，等 `configLoaded`）；`handleOffline`（`SelfServicePanel.tsx:119`，`selfOfflineSession`，门 `:121`，成功后本地过滤掉该 session）；`fetchOnlineLog`（`SelfServicePanel.tsx:166`，`querySelfOnlineLog`，门 `:168`）；查询按钮禁用判据（`:212`）与无凭据提示 `account.needLoginPassword`（`:214`）。
- 视图：在线信息卡（`SelfServicePanel.tsx:193-263`：查询按钮 `:208-221`——禁用判据 `:212`、无凭据提示 `account.needLoginPassword` `:214`；内容分支 `:226-258`——`configLoaded` 加载态 `:226`、`!hasCred` 提示 `:230`、查询中 `:232`、在线设备 `OnlineDeviceCardStack`/`OnlineDeviceCard` 堆叠（`:236-254`，`key={item.sessionId}` `:240`，支持别名改名）、空/待查询态 `:255-258`、单位注释 `:260`）；历史与上网记录卡拆分为子组件 `<SelfOnlineRecords>`（`SelfServicePanel.tsx:265-277`，实现 `account/SelfOnlineRecords.tsx`）；注销确认框（`:279-285`）与设备改名弹窗 `DeviceAliasDialog`（`:286-292`）。凭据输入区已随凭据统一移除（见 [[credential-single-source]]）。

**selfServiceState.ts（两道验证门）**

- 历史的跨面板共享凭据 store 已随凭据统一删除（绑定与自助查询直接消费登录信息，见 [[credential-single-source]]）。
- Hello 总开关读取：`helloEnabled`（`!== false` 视为开启）。
- 绑定/明文操作门：`VERIFY_TTL_MS = 570_000`（`selfServiceState.ts:26`，对齐后端 600s 留 30s 余量，`:18` 注释）、`gateFresh`（`:33`，时钟回拨视为过期）、模块级 `helloGateVerifiedAt`、`useHelloGate(options?: { ignoreToggle?: boolean })`（`selfServiceState.ts:37` 起：门新鲜直接放行 → 非 `ignoreToggle` 且总开关关闭放行 → 否则 `verifyWindowsIdentity`）。
- 自助服务会话门：`resetSelfSessionGate`（`SelfServicePanel.tsx:76` 卸载调用）、`useSelfServiceVerify`（`selfServiceState.ts:77` 起，`selfReverifyEachAction === true` 时每次重验）。

**useAccount.ts**：`useAccount`（`useAccount.ts:8`）；共享刷新 `refreshAccounts`（`useAccount.ts:23`，新增/删除/切换/改名后统一回填列表）；`handleAddAccount`（`useAccount.ts:33`，返回 boolean 供调用方决定是否清输入）；`handleDeleteAccount`（`useAccount.ts:53`，应用返回的 `activeAccount` / `config`，修历史缺陷）；`handleSwitchAccount`（`useAccount.ts:73`，守卫用 `activeAccount !== undefined` 而非真值判断——R4 修复，见 [[account-switch-ui-state-desync]]）；`handleRenameAccount`（`useAccount.ts:95` 起，成功返回 boolean 供调用方决定是否退出内联编辑；失败透出后端 message）。注意本文件用 `i18next.t` 而非 `useTranslation`，因此 toast 文案随语言即时变化。

**types.ts**：`AccountResult`（`account/types.ts:7`，含 `displayName?`）、`SwitchAccountResult`（`account/types.ts:15`）、`DeleteAccountResult`（`account/types.ts:22`）、`SaveAccountResult`（`account/types.ts:29`）；`AccountItem {id, displayName}` 从 `@/settings` 转出口（`account/types.ts:4`）。

**测试**：`AccountPanel.helloGate.test.tsx` 3 个用例——首次查询即验证、通过后共用（`AccountPanel.helloGate.test.tsx:84-101`）、查询通过后查看明文不再二次验证（`AccountPanel.helloGate.test.tsx:103-123`）、验证失败不执行且下次仍需验证（`AccountPanel.helloGate.test.tsx:125-142`）。`SelfServicePanel.test.tsx` 7 个用例——无凭据禁用刷新（`SelfServicePanel.test.tsx:82-88`）、学号默认取 `config.user` 且格式化正确（含金额不截断，`SelfServicePanel.test.tsx:90-121`）、验证失败不查询（`SelfServicePanel.test.tsx:123-136`）、总开关关闭不弹验证（`SelfServicePanel.test.tsx:138-150`）、验证一次后共用（`SelfServicePanel.test.tsx:152-173`）、眼睛按钮 `mousedown` 必须 preventDefault（`SelfServicePanel.test.tsx:175-192`）、注销需确认且成功后移除行（`SelfServicePanel.test.tsx:194-225`）。

### monitor 域

`monitor/index.ts:1-10` 导出 `MonitorPanel`、`QualityPanel`、`SpeedTestPanel`、`LatencyPair`、`LatencyTimeline`、`NetworkQualityCapsule`、`StatusBar`、`useMonitor` 与全部 types。

**MonitorPanel.tsx（后台检测控制 + 校园网校验设置）**

- props `MonitorPanelProps`（`MonitorPanel.tsx:22-26`）。
- `AdapterStatusCard`（`MonitorPanel.tsx:28-74`）：在线/离线配色与图标、主适配器徽标（`MonitorPanel.tsx:52-54`）、IP 或 `auth.noIp`（`MonitorPanel.tsx:56`）。
- 主组件（`MonitorPanel.tsx:76-553`）：`bgStatus`（`MonitorPanel.tsx:78`）、`config` 浅比较订阅（`MonitorPanel.tsx:81`）、`intervalSec`（`MonitorPanel.tsx:82`，缺省 60000/1000）、三处文本草稿 `intervalDraft` / `networkNameDraft` / `campusGatewayDraft`（`MonitorPanel.tsx:85-89`）；`isRefreshing` 用 `useAsyncLock(…, 2000)`（`MonitorPanel.tsx:90-92`）；`isTogglingDetection` 用默认 1500ms 冷却（`MonitorPanel.tsx:94-96`）；`commitInterval` 钳制 10–600s（`MonitorPanel.tsx:98-105`）、`commitNetworkName`（`MonitorPanel.tsx:107-113`）、`commitCampusGateway`（`MonitorPanel.tsx:115-121`）。
- 检测卡（`MonitorPanel.tsx:125-209`）：运行态图标与描述（`MonitorPanel.tsx:130-141`）、检查次数徽标（`MonitorPanel.tsx:144-147`，>9999 显示 k）、立即检测（`MonitorPanel.tsx:148-157`）、启动/停止（`MonitorPanel.tsx:158-167`）、开关与间隔输入（`MonitorPanel.tsx:172-194`）、适配器在线状态列表（`MonitorPanel.tsx:196-206`）。
- 校验设置卡（`MonitorPanel.tsx:211-550`）：`enableBackgroundCheck`（`MonitorPanel.tsx:235-240`）、`autoExitOnOnline`（`MonitorPanel.tsx:253-258`）、`autoLoginOnPreparation`（`MonitorPanel.tsx:271-276`）、`enableNetworkNameCheck`（`MonitorPanel.tsx:286-321`，Label 旁的问号 `HelpCircle` 按钮（`:289-310`）把检测逻辑四步说明收进 Tooltip——沿用 `QualityPanel` 指标问号同款模式，替代原先展开区常驻的 5 行说明块），展开区（`MonitorPanel.tsx:322-544`）含 SSID 名（`MonitorPanel.tsx:324-336`）、网关（`MonitorPanel.tsx:337-355`，输入期正则 `^(\d{1,3}\.){0,3}\d{0,3}$` 只让合法值进草稿）、`campusExitOnFail`（`MonitorPanel.tsx:367-372`，缺省 true）、退出时段两端点 `campusExitStartMinutes` / `campusExitEndMinutes`（`MonitorPanel.tsx:375-419`，缺省 480/1380）、检查时段 `campusCheckStartMinutes` / `campusCheckEndMinutes`（`MonitorPanel.tsx:421-466`，缺省 460/1380——2026-09-13 起终点旧默认 0 改为 1380=23:00；该行窄屏（<640px）纵向堆叠，容器 `flex-col gap-2 sm:flex-row sm:items-center sm:justify-between`，原因与安卓同构：右侧两个 `w-24` time input 合计约 200px 不可压缩，同行会把左侧 `min-w-0` 标题挤成竖排，见 `MonitorPanel.tsx:421` 注释）、每日定时登录/定时注销时刻 `scheduledLoginMinutes` / `scheduledLogoutMinutes`（`MonitorPanel.tsx:468-527`，分钟制、0=禁用、过点补触发；两行同样窄屏纵向堆叠，且单 time Input 外包 `flex shrink-0 items-center` 容器——Input 组件对 time 类型自带 `relative w-full` 包装层，直接作 flex 子项会吃满剩余空间使选择器悬中，见 `learnings/input-time-wrapper-w-full`）、当前 SSID/有线/校园网徽标（`MonitorPanel.tsx:528-544`）。

**QualityPanel.tsx（网络质量详情：总览 / 定时测试 / 分项明细）**

- props `QualityPanelProps`（`QualityPanel.tsx:29-33`）。
- `DETAIL_CATEGORIES`（`QualityPanel.tsx:35-86`）：5 个子标签 `gateway`（`QualityPanel.tsx:36-45`）、`dns`（`QualityPanel.tsx:46-55`，含 `aliDoh`/`tencentDoh`/`aliDns`/`tencentDns`/`xinfengDns`/`dnsResolve`）、`http`（`QualityPanel.tsx:56-65`，`baidu`/`jd`/`bing`/`railway12306`）、`stream`（`QualityPanel.tsx:66-75`）、`game`（`QualityPanel.tsx:76-85`）；`tabContainerVariants`（`QualityPanel.tsx:88-105`）。
- 主组件（`QualityPanel.tsx:107-487`）：`networkQuality` / `isRefreshingQuality`（`QualityPanel.tsx:109-110`）、`config` 浅比较订阅（`QualityPanel.tsx:113`）、`profile`（`QualityPanel.tsx:114`）、`isPoorQuality` 与危险光晕（`QualityPanel.tsx:115-116`）、`cardItemVariantsNoY` / `tabItemVariants`（`QualityPanel.tsx:118-150`）、`resolveQualityDisplay`（`QualityPanel.tsx:151`）、`qualityConfig`（`QualityPanel.tsx:152-154`）、`intervalSec`（`QualityPanel.tsx:156`）、`intervalDraft` 与 `commitInterval`（`QualityPanel.tsx:158-167`）、`activeTab` / `tabDirection`（`QualityPanel.tsx:169-170`）、`handleTabChange`（`QualityPanel.tsx:172-178`）、`isTogglingLatency`（`QualityPanel.tsx:181-184`）、`details` / `hasData`（`QualityPanel.tsx:186-188`）、`activeItems`（`QualityPanel.tsx:190-197`）。
- 视图：质量卡（`QualityPanel.tsx:201-262`，含刷新按钮与总耗时 tooltip `QualityPanel.tsx:224-246`）、定时测试卡（`QualityPanel.tsx:264-313`）、明细卡（`QualityPanel.tsx:315-483`，`SegmentTabs` `QualityPanel.tsx:357-367`、逐项延迟与 `LatencyTimeline` `QualityPanel.tsx:385-468`、UDP 异常提示 `QualityPanel.tsx:424-426`、目标/类型 tooltip `QualityPanel.tsx:431-465`、检测时间 `QualityPanel.tsx:473-480`）。

**SpeedTestPanel.tsx（纯外链测速站点集合）**

- `SpeedTestSite`（`SpeedTestPanel.tsx:18-26`）；`SPEED_TEST_SITES` 8 个站点（`SpeedTestPanel.tsx:28-101`）；分类键 `SITE_CATEGORY_KEYS`（`SpeedTestPanel.tsx:103`，综合/教育网/轻量）；props `SpeedTestPanelProps`（`SpeedTestPanel.tsx:105-107`）；主组件（`SpeedTestPanel.tsx:109-199`，`handleOpen` `SpeedTestPanel.tsx:111-113`，分类渲染 `SpeedTestPanel.tsx:138-185`，脚注 `SpeedTestPanel.tsx:187-196`）。

**StatusBar.tsx（顶部状态条）**

- `EMPTY_ADAPTER_STATUSES` 模块级空数组（`StatusBar.tsx:16`，避免 `?? []` 造新引用触发重渲染）；props `StatusBarProps`（`StatusBar.tsx:18-21`）；粒度订阅（`StatusBar.tsx:25-38`，只吃 `adapter1` / `adapter2` / `dualAdapter`）；`wasOffline` 派生（`StatusBar.tsx:42`）与 ref 更新（`StatusBar.tsx:44-46`）；`displayText` / `campusTooltip` 推导（`StatusBar.tsx:48-91`，用 `adapterStatuses` 的 `online` 与卡片同源）；状态配色表 `statusConfig`（`StatusBar.tsx:93-100`）；视图与动画类名（`StatusBar.tsx:102-198`），右侧按 `enableNetworkQuality` 决定是否渲染 `NetworkQualityCapsule` + 刷新按钮（`StatusBar.tsx:143-163`）。

**NetworkQualityCapsule.tsx（状态条上的质量胶囊 + 悬浮详情）**

- props（`NetworkQualityCapsule.tsx:13-15`）；`LatencyRowProps`（`NetworkQualityCapsule.tsx:17-22`）；`LatencyRow`（`NetworkQualityCapsule.tsx:24-42`）；`getQualityCapsuleBg`（`NetworkQualityCapsule.tsx:44-51`，由 hex 造 10% 透明底）。
- 主组件（`NetworkQualityCapsule.tsx:53-234`）：网关/外网/DNS 三个延迟源（`NetworkQualityCapsule.tsx:61-63`）、副标题派生（`NetworkQualityCapsule.tsx:65-83`）、`displayLatency` 三级降级（`NetworkQualityCapsule.tsx:85-90`）、动画类型判定（`NetworkQualityCapsule.tsx:104-113`，200ms 以上或恶化 1.5 倍走 heartbeat）、浮层坐标与滚动/resize 跟随（`NetworkQualityCapsule.tsx:115-164`）、胶囊本体（`NetworkQualityCapsule.tsx:168-203`）、`createPortal` 到 body 的详情浮层（`NetworkQualityCapsule.tsx:205-231`）。

**LatencyComponents.tsx（信号条与内/外网延迟对）**

- `getSignalCfg`（`LatencyComponents.tsx:11-22`）；`BAR_SPECS` 5 根柱（`LatencyComponents.tsx:24-30`）；`SignalGlowDot`（`LatencyComponents.tsx:33-73`，CSS `@keyframes` 替代 GSAP）；`SignalBars`（`LatencyComponents.tsx:75-168`，加载态 `LatencyComponents.tsx:100-116`、`activeBars` 变化触发重挂 `LatencyComponents.tsx:83-88`、暗色底推理 `LatencyComponents.tsx:90-96`）；`LatencyPair`（`LatencyComponents.tsx:170-236`）。

**LatencyTimeline.tsx（延迟分段瀑布条）**

- props（`LatencyTimeline.tsx:7-17`）；`TimelineSegment`（`LatencyTimeline.tsx:19`）；`SEGMENT_INFO`（`LatencyTimeline.tsx:23-31`，键名 `内容` / `网络` 为后端 `quality.rs` 原始字典键，属跨语言契约）；`LEVEL_BAR_COLOR`（`LatencyTimeline.tsx:33-40`）；主组件（`LatencyTimeline.tsx:42-160`，分段构造 `LatencyTimeline.tsx:44-78`、无数据灰占位 `LatencyTimeline.tsx:84-89`、宽度归一化避免最小宽度溢出 `LatencyTimeline.tsx:96-119`、图例与 tooltip `LatencyTimeline.tsx:130-156`）。

**types.ts**：`AdapterOnlineStatus`（`monitor/types.ts:1-7`）、`ConnectionCampusStatus`（`monitor/types.ts:9-13`，未导出）、`BackgroundStatus`（`monitor/types.ts:15-34`）、`BackgroundCheckEventData`（`monitor/types.ts:36-47`）、`NetworkQualityDetail`（`monitor/types.ts:49-60`，未导出）、`NetworkQualityMetrics`（`monitor/types.ts:62-65`，未导出）、`NetworkQuality`（`monitor/types.ts:67-76`，`quality` 9 值枚举）、`AutoLoginEventData`（`monitor/types.ts:78-82`）。

**useMonitor.ts**：`useMonitor`（`useMonitor.ts:7-74`）合并 `useAuthStore` / `useQualityStore` / `useConfigStore`；`handleToggleBackgroundCheck`（`useMonitor.ts:25-43`，先 `saveConfigDirect` 再 start/stop，最后 `updateConfigLocal` + `setBgStatus`）；`handleTriggerCheck`（`useMonitor.ts:45-47`）；`handleToggleLatencyTest`（`useMonitor.ts:49-66`，同样先落盘再启停）。

### network 域

`network/index.ts:1-5` 导出 `NetworkPanel`、`useNetwork`、types 与 constants。

**NetworkPanel.tsx（网卡列表 / 主副适配器 / DNS-DoH 优化）**

- props `NetworkPanelProps`（`NetworkPanel.tsx:33-36`）。
- DNS 白名单常量：`ALI_DNS`（`NetworkPanel.tsx:38`）、`TENCENT_DNS`（`NetworkPanel.tsx:39`）、`RECOMMENDED_DNS`（`NetworkPanel.tsx:40`，两者并集）。
- `formatSpeed`（`NetworkPanel.tsx:45`，bit/s → Mbps / Kbps）。
- 主组件 `NetworkPanel = memo(...)`（`NetworkPanel.tsx:171-865`，2026-09-23 三期起与「夜间出站切换」合并为单张适配器卡，见 [[night-outbound-switch]]）：`t`（`:172`）、`disabledAdapters`（`:173`）、`config` 浅比较自订阅（`:176`，useShallow——App 外壳不再因 config 变化级联重渲染）、`accounts`（`:178`）；拖拽常量 `DRAG_LONG_PRESS_MS = 250`（`:52`）/`DRAG_DEAD_ZONE_PX = 8`（`:54`）；可拖拽行 `SortableAdapterRow`（`memo`，`:71-169`，行级长按 pointer 处理器 + 死区 + 按钮区隔离）；拖拽顺序状态：`outboundOrder = buildOutboundOrder(...)`（`:184`）→ 本地 `dragOrder`/`isDraggingRef`（`:186-197`，拖拽中屏蔽外部顺序回显）→ `handleDragStart`（`:203`）/`handleReorder`（`:208`，仅写本地态）/`handleDragEnd`（`:216-224`，顺序变化才一次性提交 `outboundPriority`）；单槽位忙态 `dohEnabling`/`dnsResetting`/`gettingNewIpAdapter`/`enablingAdapter` 与 `ipc = tauriApiWithRetry`、`mountedRef`（`:226-231`）、`refreshAdapters` 来自 store（`:242`）。
- 行为：`handleCheckDns`（`NetworkPanel.tsx:252`，`checkDnsDohStatus` + 两类 warning 日志）；`handleSetupDnsDoh`（`NetworkPanel.tsx:275`，`setupDnsDoh(dnsFamily)`，随后独立 try 刷新状态，避免失败误报）；`handleResetDns`（`NetworkPanel.tsx:301`，`resetDns()` 恢复默认 DNS，同样独立 try 刷新状态；补齐一键设 DNS 后的可逆性）；`handleGetNewIpForAdapter`（`NetworkPanel.tsx:327`，`dhcpReleaseRenewAdapter` + `announceDhcpResults` + `refreshAdapterData()`）；`handleEnableAdapter`（`NetworkPanel.tsx:344`，`enableAdapter` + 带 `includeDisabled` 刷新）；`getDnsQuality`（`NetworkPanel.tsx:364`，四级 `excellent`/`good`/`basic`/`none`）。
- 恢复重探（2026-10-06）：`syncOnRestore`（`NetworkPanel.tsx:298-307`，`lastFocusSyncRef` 5s 节流去重）挂 `visibilitychange`（visible 时）+ `window focus` 两监听（`:308-317`）——WebView2 最小化期间定时器被节流、夜间自动切换只改路由 metric/禁卡不改 `adapters.length`、后端 `outbound_switch_now` 不向前端 emit 事件，三者叠加使恢复窗口的出站徽标/列表陈旧；触发即 `refreshAdapterData({force:true,includeDisabled:true})` + `refreshCurrentOutbound()`。mockIPC 浏览器实测：恢复派发 → `get_adapters:force`/`get_adapter_details`/`get_disabled_adapters`/`get_current_outbound_name` 全数发出。
- 视图（2026-09-23 三期两卡合并、四期撤销拖拽把手，行号为合并后现状）：**合并卡「网络适配器 + 夜间出站切换」**（`NetworkPanel.tsx:387-521`）——卡头左「网络适配器 + 检测数」、右 MoonStar + 开关 `enableNightOutboundSwitch`（默认关，`:404-409`），卡头下整行功能描述（`:413`）；卡体：空态（`:416-421`）或 `Reorder.Group as="div"`（`:423-429`）行列表（`:430-508`），每行 = 有线/无线图标 + 名称/IP(mono)/链路速度 + 徽标（主适配器 `:438-442`/副适配器 `:443-447`/有线无线 `:448-450`/状态 `:451-459`）+ 行按钮（启用 `:460-476`/获取新IP `:477-488`/刷新DHCP `:489-500`）+ **夜间出站目标徽标**（排第一的卡常显 `:502-506`，不受开关影响）；行支持**长按任意处拖拽排序**（长按 250ms 起拖 + 8px 死区，2026-09-23 四期撤销把手通道，见 [[night-outbound-switch]]；原「主适配器排最前」强制排序同时取消，主/副由徽标表达）；列表底部提示（`:511-519`：拖拽提示 `outboundDragHint` + 管理员权限提示 `outboundAdminHint`）；适配器设置卡（`:524-650`：主适配器选择、**主适配器"指定账号"下拉**（哨兵 `FOLLOW_CURRENT_ACCOUNT = '__follow__'`，Radix Select 不允许空串 value，落盘时空串；选项为 `AccountItem` 列表，`:574-587`）、副适配器选择与 `dualAdapter` 联动、**副适配器"指定账号"下拉**（仅已选副适配器时显示）、说明文案）；DNS 优化卡（`:653-843`：检测/优化/重置 DNS 按钮、IP 族切换、未检测/检测中态、逐适配器明细与 profile 级 DNS、DoH 不支持提示）。适配器绑定账号的解析语义见 [[adapter-account-binding]]。

**adapters.ts**：`AUTO_DETECT_ADAPTER = '自动检测'`（`adapters.ts:3`）；`AdapterScope`（`adapters.ts:5-9`）；`resolveAdapterNames`（`adapters.ts:15-30`）——与后端 `resolve_adapter_names`（`src-tauri/src/network/adapter.rs`）保持同一规则：有线有 IP > 任意有 IP > 第一个；副适配器自动检测时排除主适配器。

**constants.ts**：`QUALITY_CONFIG`（`network/constants.ts:1-11`）9 档（`excellent`/`great`/`good`/`fair`/`poor`/`bad`/`unknown`/`disabled`/`busy`），每档含 `label` / `labelKey` / `color` / `bg` / `border` / `borderBg` / `icon` / `hex` / `activeBars` / `glow`。

**types.ts**：`AdapterStatus`（`network/types.ts:1`）、`Adapter`（`network/types.ts:3-13`）、`DisabledAdapter`（`network/types.ts:15-19`）、`AdapterDetail`（`network/types.ts:21-33`）、`DnsServerInfo`（`network/types.ts:35-40`，未导出）、`DnsAdapterInfo`（`network/types.ts:42-48`）、`DnsDohStatus`（`network/types.ts:50-55`）、`DhcpRenewResult`（`network/types.ts:57-60`）、`DhcpReleaseRenewResult`（`network/types.ts:62-65`）、`DnsSetupResult`（`network/types.ts:67-74`）、`EnableAdapterResult`（`network/types.ts:76-79`）。

**useNetwork.ts**：`normalizeDhcpResults`（`useNetwork.ts:15-20`，兼容单条与 `{results}` 批量两形态）；`announceDhcpResults`（`useNetwork.ts:23-37`，成功/跳过/失败三类 toast）；`useNetwork`（`useNetwork.ts:39-104`），`refreshAdapterInfo`（`useNetwork.ts:59-62`，复用 `refreshAdapterData`）、`handleDhcpRenew`（`useNetwork.ts:64-71`，`dhcpRenewAll` + 刷新 + 触发后台检测）、`handleDhcpReleaseRenew`（`useNetwork.ts:73-83`）、`handleDhcpReleaseRenewAdapter`（`useNetwork.ts:85-95`）。

**测试**：`adapters.test.ts` 6 个用例覆盖配置名有效、自动检测降级、失效名降级、单适配器、仅无 IP 适配器、空列表（`adapters.test.ts:22-68`）。`useNetwork.test.ts` 覆盖 `normalizeDhcpResults` 两形态（`useNetwork.test.ts:34-43`）与三类 toast 分类（`useNetwork.test.ts:45-67`）。

### settings 域

`settings/index.ts:1-7` 导出 `SettingsPanel`、`ThemeDialog`、`OnboardingWizard`、`useSettings`、types 与 constants。

**constants.ts**：`DEFAULT_CONFIG`（`settings/constants.ts:6` 起，完整默认值，与 `Config` 接口同口径）；`ISP_OPTIONS`（`settings/constants.ts:54-59`，`__default__`=无锡学院，其余 `@telecom` / `@unicom` / `@cmcc`）；`THEME_OPTIONS`（`settings/constants.ts:61-69`，7 套）；`VALID_THEMES`（`settings/constants.ts:71`，供 `main.tsx:38` 校验）；`DEFAULT_PANEL_OPTIONS`（`settings/constants.ts:73-76`，由 `NAV_ITEMS` 派生）。

**types.ts**：`Config`（`settings/types.ts`——含 Windows Hello 开关、校园网时段、每日定时登录/注销时刻、更新渠道、**适配器指定账号 `adapter1Account` / `adapter2Account`（2026-09 新增，可选）**等）；`AutoLaunchResult`（`settings/types.ts:71-74`）；`InitData`（`settings/types.ts:76-89`，`getInitData` 返回体，含 `config` / `adapters` / `adapterDetails` / `disabledAdapters` / `accounts: AccountItem[]` / `backgroundStatus` / `gpuInfo` 等）。

**SettingsPanel.tsx（外观 / 启动 / 通知 / 安全 / 引导 / 质量检测 / 配置导出导入）**

- props `SettingsPanelProps`（`SettingsPanel.tsx:28-35`）；`PRESET_COLORS` 12 色（`SettingsPanel.tsx:37-41`）；`PRESET_COLOR_NAMES`（`SettingsPanel.tsx:43-56`）。
- 主组件（`SettingsPanel.tsx:123-765`）：`isLightMode` / `themeName`（`SettingsPanel.tsx:66-67`）、`config` 浅比较订阅（`SettingsPanel.tsx:70`）、取色草稿 `colorDraft` + 80ms 节流提交（`SettingsPanel.tsx:74-103`）、固定网关草稿 `fixedGatewayDraft`（`SettingsPanel.tsx:81`、`SettingsPanel.tsx:177-183`）、`customColor`（`SettingsPanel.tsx:83-84`）、`handlePresetColor`（`SettingsPanel.tsx:105-110`，点预设时取消待提交的取色任务）、`handleSecurityDisable`（`SettingsPanel.tsx:122-137`，关闭安全开关前必须过 `verifyWindowsIdentity`）；配置导出/导入 handler（`SettingsPanel.tsx:141-204`，`exportWithPassword` 状态 `:141`、`handleExportConfig` `:148`、`handleImportConfig` `:184`，导出可选 DPAPI 密文携带密码、导入严格校验后走 `save_config` 同路径）。
- 视图：外观卡（`SettingsPanel.tsx:208-314`，7 套主题按钮 `SettingsPanel.tsx:224-254`、自定义取色区 `SettingsPanel.tsx:258-299`、浅色开关 `SettingsPanel.tsx:302-311`）；启动设置卡（`SettingsPanel.tsx:320-435`，`autoLaunch` `SettingsPanel.tsx:338-343`、`autoLoginOnStart` `SettingsPanel.tsx:351-356`、`autoExitAfterLogin` `SettingsPanel.tsx:364-369`、`autoExitOnOnline` `SettingsPanel.tsx:377-382`、`hiddenStart` `SettingsPanel.tsx:390-395`、`minimizeToTray` `SettingsPanel.tsx:403-408`、`defaultPanel` 选择 `SettingsPanel.tsx:418-431`）；通知卡（`SettingsPanel.tsx:439-466`）；安全卡（`SettingsPanel.tsx:471-518`，`selfHelloEnabled` `SettingsPanel.tsx:489-497`、`selfReverifyEachAction` `SettingsPanel.tsx:505-514`）；引导入口（`SettingsPanel.tsx:522-548`）；质量检测卡（`SettingsPanel.tsx:554-665`，`enableNetworkQuality` 及其联动清理 `SettingsPanel.tsx:572-595`、`skipTtfbInLatency` `SettingsPanel.tsx:605-610`、`skipContentInLatency` `SettingsPanel.tsx:618-623`、说明块 `SettingsPanel.tsx:627-637`、`fixedGateway` 输入与清除 `SettingsPanel.tsx:639-664`）；导出/导入配置卡（`SettingsPanel.tsx:672-759`，`exportWithPassword` 开关 `:692`、导出按钮 `:699`、导入确认框 `:759`）。

**OnboardingWizard.tsx（5 步首次引导）**

- props（`OnboardingWizard.tsx:37-44`）；步骤标题键 `STEP_TITLE_KEYS`（`OnboardingWizard.tsx:46`）；哨兵 `BIND_OPERATOR_NONE`（`OnboardingWizard.tsx:49`）；停留时长 `BIND_SUCCESS_ADVANCE_MS = 1200`（`OnboardingWizard.tsx:51`）；绑定状态机 `BindState`（`OnboardingWizard.tsx:53`）；滑动 variants（`OnboardingWizard.tsx:55-59`）；`StepIndicator`（`OnboardingWizard.tsx:61-98`，含 `layoutId="step-indicator"` 共享元素）。
- 主组件（`OnboardingWizard.tsx:100` 起）：语言切换 `language` / `setLanguage`（`OnboardingWizard.tsx:102-103`）、`config` 浅比较订阅（`OnboardingWizard.tsx:106`）、基础向导状态（`OnboardingWizard.tsx:107-116`）、`prevOpenRef`（`OnboardingWizard.tsx:117`）、`ensureBindHello`（`OnboardingWizard.tsx:120`）、绑定步骤状态（`OnboardingWizard.tsx:122-124`，凭据取自账号信息步草稿，见 [[credential-single-source]]）、每次打开重置全部字段（`OnboardingWizard.tsx:132-151`，`:150` 注释「绑定凭据沿用登录信息」）、`canBind`（`OnboardingWizard.tsx:151-153`）、`handleBind`（`OnboardingWizard.tsx:157` 起，`:163` invoke，成功 1.2s 后自动进入网卡步 `:175`/`:177`）、`canProceedAccount`（`OnboardingWizard.tsx:194`）、`handleNext`（`OnboardingWizard.tsx:196` 起，step 1 账号信息写账号 `:197`、step 3 网卡写适配器 `:209`）、`handleSkip`（`OnboardingWizard.tsx:221` 起，写 `campus-onboarding-done` 并关闭）、`handleLoginAndFinish`（`OnboardingWizard.tsx:236` 起，`:240` hasAccount，最终校验 + 写配置 + `onLogin` + 1.5s 后标记完成）、`direction` ref（`OnboardingWizard.tsx:279`）、`advance`（`OnboardingWizard.tsx:281` 起）。
- 视图（**步序已重排**：欢迎 → 账号信息 → 绑定运营商 → 网卡 → 完成，账号信息先行以提供绑定凭据，见 [[credential-single-source]]；安卓端 4 步无网卡步）：step 0 欢迎（`OnboardingWizard.tsx:302-317`）、step 2 绑定运营商（`OnboardingWizard.tsx:318-390`，仅 ISP/手机/短信口令输入——不再有学号/密码输入，凭据沿用账号信息步草稿）、step 1 账号信息（`OnboardingWizard.tsx:391-459`，JSX 位于绑定步代码块之后）、step 3 网卡选择（`OnboardingWizard.tsx:460-537`）、step 4 完成确认（`OnboardingWizard.tsx:538-599`）、语言切换、底部导航与登录按钮（`OnboardingWizard.tsx:600` 起）、关闭二次确认（文件尾部）。

**ThemeDialog.tsx**：props（`ThemeDialog.tsx:20-25`）；主组件（`ThemeDialog.tsx:27-84`），`themeName` / `isLightMode` 订阅（`ThemeDialog.tsx:28-29`）、主题网格（`ThemeDialog.tsx:44-67`）、浅色开关（`ThemeDialog.tsx:70-79`）。

**useSettings.ts**：`useSettings`（`useSettings.ts:10-70`）；`handleToggleLightMode`（`useSettings.ts:35-41`，同步 `useThemeStore` + `config.themeMode` + `campus-light-mode`）；`handleToggleNotification`（`useSettings.ts:43-47`）；`handleSetAutoLaunch`（`useSettings.ts:49-56`，API 失败时提示）；`handleSetTheme`（`useSettings.ts:58-61`，写 `campus-theme`）。

## 结构体与字段（前端即 props 与类型）

### 面板 props

| 组件 | 文件:行号 | 字段 | 类型 | 含义 |
|---|---|---|---|---|
| `DashboardPanel` | `auth/DashboardPanel.tsx:74-84` | `accounts` | `AccountItem[]` | 已保存账号列表（`{id, displayName}`） |
| | | `activeAccount` | `string` | 当前使用账号 |
| | | `onUpdateConfig` | `(partial: Partial<Config>) => void` | 写入配置（debounce 落盘） |
| | | `onSwitchAccount` | `(name: string) => Promise<any>` | 切账号 |
| | | `onDhcpRenew` | `() => Promise<void>` | DHCP 续租 |
| | | `onDhcpReleaseRenew` | `() => Promise<void>` | 全部适配器释放并重新获取 IP |
| | | `onDhcpReleaseRenewAdapter` | `(adapterName: string) => Promise<void>` | 指定适配器获取新 IP |
| | | `onRefreshQuality?` | `() => Promise<void>` | 手动刷新质量 |
| | | `onToggleBackgroundCheck?` | `(enabled: boolean, intervalSec: number) => Promise<void>` | 声明但 `App.tsx:296-305` 未传（`renderCard` 亦未消费） |
| `AccountPanel` | `account/AccountPanel.tsx:35-45` | `adapters` | `Adapter[]` | 主适配器下拉候选 |
| | | `accounts` / `activeAccount` | `AccountItem[]` / `string` | 账号列表与当前账号 |
| | | `onUpdateConfig` | `(partial: Partial<Config>) => void` | 配置写入 |
| | | `onAddAccount` | `(name: string) => Promise<boolean>` | 保存当前为账号，返回是否成功 |
| | | `onDeleteAccount` | `(id: string, displayName: string) => void` | 触发删除（实际由 App 弹确认框） |
| | | `onSwitchAccount` | `(name: string) => Promise<void>` | 切账号 |
| | | `onRenameAccount` | `(accountId: string, displayName: string) => Promise<boolean>` | 账号改名（成功与否决定是否退出内联编辑） |
| `SelfServicePanel` | `account/SelfServicePanel.tsx:110` | — | — | 无 props，自订阅 `useConfigStore`（凭据统一后绑定与自助查询共用登录信息，见 [[credential-single-source]]） |
| `NetworkPanel` | `network/NetworkPanel.tsx:31-34` | `adapters` | `Adapter[]` | 网卡列表 |
| | | `onUpdateConfig` | `(partial: Partial<Config>) => void` | 配置写入 |
| `MonitorPanel` | `monitor/MonitorPanel.tsx:22-26` | `onUpdateConfig` | `(partial: Partial<Config>) => void` | 配置写入 |
| | | `onToggleBackgroundCheck` | `(enabled: boolean, interval: number) => Promise<void>` | 启停后台检测 |
| | | `onTriggerCheck` | `() => Promise<void>` | 立即检测 |
| `QualityPanel` | `monitor/QualityPanel.tsx:29-33` | `onUpdateConfig` | `(partial: Partial<Config>) => void` | 配置写入 |
| | | `onRefreshQuality?` | `() => Promise<void>` | 手动重测（缺省则不渲染刷新按钮） |
| | | `onToggleLatencyTest?` | `(enabled: boolean, intervalSec: number) => Promise<void>` | 启停定时延迟测试 |
| `SpeedTestPanel` | `monitor/SpeedTestPanel.tsx:105-107` | `openExternal` | `(url: string) => void` | 外部浏览器打开 |
| `StatusBar` | `monitor/StatusBar.tsx:18-21` | `onOpenPortal` | `() => void` | 打开 Portal 页 |
| | | `onOpenSelfService?` | `() => void` | 打开自助服务页（缺省则不渲染入口） |
| `SettingsPanel` | `settings/SettingsPanel.tsx:28-35` | `autoLaunch` | `boolean` | 开机自启（由 App 从 `config.autoLaunch` 解出） |
| | | `onUpdateConfig` | `(partial: Partial<Config>) => void` | 配置写入 |
| | | `onSetAutoLaunch` | `(enabled: boolean) => Promise<void>` | 写注册表自启 |
| | | `onToggleLightMode` | `() => void` | 明暗切换 |
| | | `onSetTheme` | `(name: ThemeName) => void` | 切主题 |
| | | `onShowOnboarding?` | `() => void` | 打开引导（缺省则隐藏该卡） |
| `AboutDialog` | `auth/AboutDialog.tsx:24-32` | `open` / `onClose` | `boolean` / `() => void` | 开关 |
| | | `openExternal?` | `(url: string) => void` | 外链 |
| | | `onUpdateAvailable?` | `(hasUpdate: boolean, latestVersion?: string, releaseNotes?: string) => void` | 结果回报给 App（写 store + toast + log） |
| | | `initialLatestVersion?` / `initialReleaseNotes?` / `initialUpdateAvailable?` | `string` / `string` / `boolean` | 通知缓存注入（避免重复检查） |
| `ThemeDialog` | `settings/ThemeDialog.tsx:20-25` | `open` / `onClose` | `boolean` / `() => void` | 开关 |
| | | `onSetTheme` | `(name: ThemeName) => void` | 切主题 |
| | | `onToggleLightMode` | `() => void` | 明暗切换 |
| `OnboardingWizard` | `settings/OnboardingWizard.tsx:37-44` | `open` / `onClose` | `boolean` / `() => void` | 开关 |
| | | `adapters` | `Adapter[]` | 网卡候选 |
| | | `onUpdateConfig` | `(partial: Partial<Config>) => void` | 配置写入 |
| | | `onLogin` | `(adapterName?: string) => Promise<boolean>` | 定向登录 |
| | | `isLoggingIn` | `boolean` | 登录中态（禁用按钮） |
| `AdapterStatusCard` | `monitor/MonitorPanel.tsx:27` | `status` | `AdapterOnlineStatus` | 单网卡在线信息 |
| | | `isPrimary` | `boolean` | 是否主适配器（渲染 `monitor.primary` 徽标） |
| `LatencyPair` | `monitor/LatencyComponents.tsx:170-174` | `gatewayLatency` / `externalLatency` | `number` | 内网/外网延迟，`<0` 表示无数据 |
| | | `loading?` | `boolean` | 骨架态（不显示数值与信号条颜色） |
| `LatencyTimeline` | `monitor/LatencyTimeline.tsx:7-17` | `totalMs` | `number` | 总耗时；`<0` 走灰色占位 |
| | | `dnsMs?` / `tcpMs?` / `tlsMs?` / `udpMs?` / `networkMs?` / `ttfbMs?` / `contentMs?` | `number` | 各分段耗时，仅 `>0` 的段进入瀑布条 |
| | | `className?` | `string` | 透传样式 |
| `LatencyRow` | `monitor/NetworkQualityCapsule.tsx:17-22` | `icon` / `label` / `sub?` / `latency` | `typeof Server` / `string` / `string` / `number` | 浮层单行 |
| `DashboardPanel` 内部卡 | `auth/DashboardPanel.tsx:93-103`、`auth/DashboardPanel.tsx:264-266`、`auth/DashboardPanel.tsx:335-337` | `noAnimation?` / `noEnterAnimation?` | `boolean` | 编辑态关动画 / 非编辑态关入场动画（由 `renderCard` 决定，`auth/DashboardPanel.tsx:667-668`） |
| `QuickActionsCard` | `auth/DashboardPanel.tsx:94-103` | `networkQuality` | `NetworkQuality \| null` | 用于差质量危险光晕 |
| | | `config` / `adapters` | `Config` / `Adapter[]` | 判断双适配器与解析主/副名 |

### 领域类型与常量

| 类型/常量 | 文件:行号 | 字段 | 类型 | 含义 |
|---|---|---|---|---|
| `PortalStatusResult` | `auth/types.ts:1-6` | `online` / `message?` / `reachable?` / `loginAvailable?` | `boolean` / `string` / `boolean` / `boolean` | Portal 在线查询结果 |
| `CommandResult` | `auth/types.ts:8-12` | `success` / `message?` / `data?` | `boolean` / `string` / `Record<string, unknown>` | 通用命令返回 |
| `LoginResult` | `auth/types.ts:14` | 继承 `CommandResult` | — | 登录返回（空扩展接口） |
| `CardId` / `CardDef` | `auth/DashboardPanel.tsx:38` / `:40-44` | `id` / `label` / `icon` | 联合字面量 / `string`（i18n key） / `typeof Zap` | 首页卡片定义 |
| `SelfOnlineItem` | `auth/DashboardPanel.tsx:459`、`account/SelfServicePanel.tsx:31` | `loginTime` / `ip` / `mac` / `useTime` / `downFlow` / `upFlow` / `hostName` / `terminalType` / `sessionId` | `string` | 在线会话（两处各自定义，字段一致） |
| `SelfLogRow`（首页精简版） | `auth/DashboardPanel.tsx:621` | `loginTime` / `time` / `flow` | `number` | 首页近期上网记录行 |
| `SelfLogRow`（账单版） | `account/selfServiceTypes.ts:9` | `loginTime` / `logoutTime` / `time` / `flow` / `costMoney` / `internetUpFlow` / `internetDownFlow` / `chinanetUpFlow` / `chinanetDownFlow` / `userIp` / `nasIp` / `nasPort` | `number` / `string` / `string \| number` | 账单明细行 |
| `SelfLogSummary` | `account/selfServiceTypes.ts:25` | `INTERNETUPFLOW` / `INTERNETDOWNFLOW` / `CHINANETUPFLOW` / `CHINANETDOWNFLOW` / `FLOW` / `TIME` / `COSTMONEY` / `COU` | `number` | 汇总卡（`COU` 为记录数） |
| `SelfHistoryRow` | `account/selfServiceTypes.ts:5` | 位置元组 `[loginTime, logoutTime, ip, mac, time, flow, payStyle, money, hostName, terminalType, ...]` | — | 历史行（按下标消费，`account/SelfOnlineRecords.tsx:63-79`） |
| `SwitchAccountResult` | `account/types.ts:3-8` | `success` / `message?` / `activeAccount?` / `config?` | `boolean` / `string` / `string` / `Config` | 切账号返回 |
| `DeleteAccountResult` | `account/types.ts:10-15` | 同上一组 | — | 删账号返回 |
| `SaveAccountResult` | `account/types.ts:17-22` | 同上一组 | — | 保存账号返回 |
| `AdapterOnlineStatus` | `monitor/types.ts:1-7` | `name` / `ip` / `wireless` / `online` / `message` | `string` / `string` / `boolean` / `boolean` / `string` | 后台检测给出的单网卡在线态 |
| `BackgroundStatus` | `monitor/types.ts:15-34` | `isRunning` / `checkCount` / `serverAvailable` / `online` / `adapterStatuses?` / `currentSsid` / `onCampusNetwork?` / `enableNetworkNameCheck?` / `requiredNetworkName?` / `campusWifi?` / `campusWired?` / `a1CampusMessage?` / `a2CampusMessage?` / `a1OnCampus?` / `a2OnCampus?` / `loginPreparationMode?` / `interval?` / `enabled?` | 混合 | 后台检测全局状态（`useAuthStore` 的 `bgStatus`） |
| `BackgroundCheckEventData` | `monitor/types.ts:36-47` | `BackgroundStatus` + `timestamp?` / `checkCount?` / `secondaryOnline?` / `secondaryMessage?` / `message?` / `online?` / `adapter1Name?` / `adapter2Name?` / `loginAvailable?` / `serverAvailable?` | — | `background-check-result` 事件体 |
| `NetworkQuality` | `monitor/types.ts:67-76` | `gatewayLatency` / `externalLatency` / `averageExternalLatency` / `gateway` / `quality` / `timestamp` | `number` / `string` / 9 值联合 / `number` | 质量结果主体 |
| | | `details?` / `metrics?` | `Record<string, NetworkQualityDetail>` / `NetworkQualityMetrics` | 分项延迟与总耗时 |
| `NetworkQualityDetail` | `monitor/types.ts:49-60` | `target` / `latency` / `type` / `dnsLatency?` / `tcpLatency?` / `tlsLatency?` / `udpLatency?` / `networkLatency?` / `ttfbLatency?` / `contentLatency?` | `string` / `number` | 单项探测明细 |
| `AutoLoginEventData` | `monitor/types.ts:78-82` | `success` / `message` / `skipped?` | `boolean` / `string` / `boolean` | `auto-login-result` 事件体 |
| `QUALITY_CONFIG` | `network/constants.ts:1-11` | 9 档，每档 `label` / `labelKey` / `color` / `bg` / `border` / `borderBg` / `icon` / `hex` / `activeBars` / `glow` | — | 质量档位展示配置 |
| `AUTO_DETECT_ADAPTER` | `network/adapters.ts:3` | `'自动检测'` | `string` | 自动检测哨兵值 |
| `AdapterScope` | `network/adapters.ts:5-9` | `adapter1` / `adapter2` / `dualAdapter` | `string` / `string` / `boolean` | 解析入参 |
| `Adapter` | `network/types.ts:3-13` | `name` / `ip` / `wireless` / `guid?` / `mac` / `ifIndex` / `status` / `linkSpeed?` | `string` / `string` / `boolean` / `string` / `string` / `number` / `AdapterStatus` / `number` | 网卡（`linkSpeed` 单位 bit/s） |
| `AdapterDetail` | `network/types.ts:21-33` | 上述 + `subnetMask` / `gateway` / `dhcpServer` | `string` | 网卡详情 |
| `DisabledAdapter` | `network/types.ts:15-19` | `name` / `status` / `description` | `string` | 禁用网卡 |
| `DnsAdapterInfo` | `network/types.ts:42-48` | `name` / `dnsSource` / `dnsServers` / `profileDnsServers` / `adapterDnsOverridesProfile` | `string` / `string` / `DnsServerInfo[]` / `DnsServerInfo[]` / `boolean` | 单网卡 DNS 状态 |
| `DnsServerInfo` | `network/types.ts:35-40` | `address` / `dohAvailable` / `dohEnabled` / `dohTemplate` | `string` / `boolean` / `boolean` / `string` | 单个 DNS 服务器 |
| `DnsDohStatus` | `network/types.ts:50-55` | `adapters` / `dohSupported` / `autoDohEnabled` / `dnsSource?` | `DnsAdapterInfo[]` / `boolean` / `boolean` / `string` | DNS 检测总结果 |
| `DhcpRenewResult` | `network/types.ts:57-60` | `success` / `results[]`（`name`、`success`） | — | 批量续租结果 |
| `DhcpReleaseRenewResult` | `network/types.ts:62-65` | `success` / `results[]`（`name`、`wireless`、`ip`、`regOk`、`success`、`skipped`、`reason`） | — | 释放重获结果（toast 分类依据） |
| `DnsSetupResult` | `network/types.ts:67-74` | `success` / `message` / `dnsSuccess?` / `dnsFailed?` / `dohAdded?` / `dohFailed?` | `boolean` / `string` / `string[]` | DNS/DoH 设置结果 |
| `EnableAdapterResult` | `network/types.ts:76-79` | `success` / `message?` | `boolean` / `string` | 启用网卡结果 |
| `Config` | `settings/types.ts` | （账号密码、Hello 开关、适配器与**指定账号 `adapter1Account`/`adapter2Account`**、自动化、主题、通知、质量、校园网时段、每日定时登录/注销时刻、更新渠道、日志保留等） | 混合 | 全局配置（默认值见 `settings/constants.ts`） |
| `InitData` | `settings/types.ts:76-89` | `config` / `version` / `adapters` / `adapterDetails` / `disabledAdapters` / `accounts: AccountItem[]` / `activeAccount` / `backgroundStatus` / `isAutoStart` / `autoLaunch` / `notificationEnabled` / `gpuInfo?` / `refreshRate?` | — | `get_init_data` 返回体 |
| `AutoLaunchResult` | `settings/types.ts:71-74` | `success` / `message?` | `boolean` / `string` | 自启设置返回 |
| `ThemeName` | `shared/ui-types.ts:3` | `'default' \| 'vibrant' \| 'forest' \| 'midnight' \| 'ocean' \| 'cherry' \| 'custom'` | — | 主题名（本模块 `main.tsx:38`、`settings/constants.ts:69` 消费） |
| `PanelName` | `shared/ui-types.ts:2` | 9 个面板 id | — | 面板路由键 |
| `NAV_ITEMS` | `shared/ui-constants.ts:7-17` | 每项 `id` / `labelKey` / `icon` / `shortcut` | — | Dock 导航与默认面板选项来源 |

## Data Flow

### 流程一：登录（向导内的启动登录）

1. 用户点「开始登录」（向导底部导航区，`settings/OnboardingWizard.tsx:600` 起）→ `handleLoginAndFinish`（`settings/OnboardingWizard.tsx:236` 起）做最终校验（账号必填 `:240`、双适配器必须选副适配器）。
2. 先写配置：`onUpdateConfig(updateData)`（`settings/OnboardingWizard.tsx:207`）→ `App.tsx:355` 传入的 `updateConfig` → `useConfigStore.updateConfig`（`hooks/useConfigStore.ts:62-94`，标脏 + 500ms debounce）。
3. 再触发登录：`onLogin(adapter1 === AUTO_DETECT_ADAPTER ? undefined : adapter1)`（`settings/OnboardingWizard.tsx:261`）→ `App.tsx:523` 传入的 `doLogin` → `useAuthStore.doLogin`（`hooks/useAuthStore.ts:145-204`）。
4. 登录前落盘：`saveConfigDirect(loginConfig)`（`hooks/useAuthStore.ts:157`）→ `hooks/useConfigStore.ts:119` 起 → `api.saveConfig`（`hooks/tauriApi.ts:285` 的 retry 包装）→ `invoke('save_config', { config, clearPassword })`（`hooks/tauriApi.ts:151`）。
5. 真实登录：`withTimeout(api.doLogin(adapterName), 60000, ...)`（`hooks/useAuthStore.ts:165`）→ `invoke('do_login', { adapterName })`（`hooks/tauriApi.ts:152`）；成功后置 `status = { text: 登录成功, state: 'online' }`（`hooks/useAuthStore.ts:167`）并写日志/toast（`hooks/useAuthStore.ts:168-169`）。
6. 登录后质量：`enableNetworkQuality !== false` 且距上次质量结果 >60s 时才 `checkNetworkQuality`（`hooks/useAuthStore.ts:176-188`）→ 写 `useQualityStore.setNetworkQuality`（`hooks/useQualityStore.ts:78-82`）。
7. 失败分支才复查在线状态：`checkOnline()`（`hooks/useAuthStore.ts:199-201`）→ `check_campus_status`（`hooks/useAuthStore.ts:34`）/`get_adapters`（`hooks/useAuthStore.ts:86`）/`check_portal_status`（`hooks/useAuthStore.ts:99`）→ 写 `status`（`hooks/useAuthStore.ts:258`、`hooks/useAuthStore.ts:289-296`，现在统一经 `setStatusStable` 内容比较后写入，引用未变时不触发订阅方重渲染）。

### 流程二：配置保存（开关类改动）

1. 用户拨动任一 `Switch`，例如 `account/AccountPanel.tsx:481-485`（开机自动登录）、`settings/SettingsPanel.tsx:351-356`（同语义的另一入口）或 `settings/SettingsPanel.tsx:403-408`（最小化到托盘）；开机自启走例外路径：`settings/SettingsPanel.tsx:338-343` 的 `onSetAutoLaunch` 直接 `invoke('set_auto_launch')`，不经过 config debounce。
2. `App.tsx:147` 注入的 `updateConfig` → `useConfigStore.updateConfig`（`hooks/useConfigStore.ts:62`）：先 `set({ config: next })` 与标脏（`hooks/useConfigStore.ts:65-71`），并入 `saveConfigPending`（`hooks/useConfigStore.ts:73-81`），500ms debounce 后调 `saveConfigDirect`（`hooks/useConfigStore.ts:82-92`）。
3. `saveConfigDirect`（`hooks/useConfigStore.ts:119` 起）把 pending 浅合并进完整 config → `api.saveConfig`（`hooks/tauriApi.ts:285`，仅此方法带指数退避重试）→ `invoke('save_config', { config, clearPassword })`（`hooks/tauriApi.ts:151`）；成功后清脏标记并按需置 `passwordSaved`（凭据统一后唯一保存态，见 [[credential-single-source]]）。
4. 反向回流：后端 `config-changed` 事件（`hooks/tauriApi.ts:203`）→ `mergeConfigFromBackend`（`hooks/useConfigStore.ts:109-116`）跳过 `dirtyFields`，避免旧快照覆盖本地新值。
5. **绕过 debounce 的立即保存路径**（清密码、启停检测）：`AccountPanel.tsx:105`（`saveConfigDirect({password:''}, true)`）、`useMonitor.ts:29-32`、`useMonitor.ts:53-56`（历史的绑定口令/自助密码 blur 提交路径已随凭据统一删除，见 [[credential-single-source]]）。
6. 文本/数字输入的「本地草稿 + blur/Enter 提交」模式统一避免每键写 store：`AccountPanel.tsx:117-123`、`AccountPanel.tsx:85-97`、`MonitorPanel.tsx:98-121`、`QualityPanel.tsx:160-167`、`SettingsPanel.tsx:86-118`、`SettingsPanel.tsx:112-118`。

### 流程三：网络质量检测

1. 结果入口（事件流）：后端发 `network-quality-result`（`hooks/tauriApi.ts:183`）→ 事件监听写 `useQualityStore.setNetworkQuality`（`hooks/useQualityStore.ts:78-82`，同时更新模块级 `lastQualityResultTime`）。
2. 手动刷新入口三处：`monitor/StatusBar.tsx:147-156`（状态条刷新按钮）、`monitor/QualityPanel.tsx:224-246`（质量卡刷新按钮）、`auth/DashboardPanel.tsx:358-362`（首页质量卡刷新按钮）→ `useQualityStore.refreshQuality`（`hooks/useQualityStore.ts:54-74`）：模块级 `_qualityLockFlag` 与 `enableNetworkQuality` 双重门（`hooks/useQualityStore.ts:56-57`）→ `api.checkNetworkQuality()`（`hooks/useQualityStore.ts:61`）→ `invoke('check_network_quality')`（`hooks/tauriApi.ts:182`）→ `mergeNetworkQuality(old, q)`（`lib/latency.ts:25-29`）写回 store。
3. 定时循环开关：`QualityPanel.tsx:181-184`（定时测试启停）与 `MonitorPanel.tsx:162` / `MonitorPanel.tsx:176`（后台检测启停，启停按钮与开关两个入口）→ `useMonitor.handleToggleLatencyTest`（`hooks/useMonitor.ts:49-66`）/ `handleToggleBackgroundCheck`（`hooks/useMonitor.ts:25-43`）→ 先 `saveConfigDirect`（`hooks/useMonitor.ts:53-56` / `hooks/useMonitor.ts:29-32`）再 `api.startLatencyTest` / `stopLatencyTest`（`hooks/useMonitor.ts:57-61`，IPC 见 `hooks/tauriApi.ts:184-185`）或 `startBackgroundCheck` / `stopBackgroundCheck`（`hooks/useMonitor.ts:34/36`，IPC 见 `hooks/tauriApi.ts:175-176`）。
4. 展示解析：`resolveQualityDisplay`（`lib/latency.ts:59-68`，把长期停在 `unknown`/`busy` 的 `quality` 按 `displayLatency` 推断档位）→ 消费点 `auth/DashboardPanel.tsx:339`、`monitor/QualityPanel.tsx:151`；胶囊走自己的三级降级 `monitor/NetworkQualityCapsule.tsx:85-90`；信号条走 `getLatencyLevel`（`lib/latency.ts:7-15`）→ `monitor/LatencyComponents.tsx:76`。
5. 状态条展示：`monitor/StatusBar.tsx:31-38` 订阅 `isRefreshingQuality` / `networkQuality` / `enableNetworkQuality` → `monitor/StatusBar.tsx:143-163` 条件渲染胶囊与刷新按钮。

### 流程四：自助服务查询与 Windows Hello 门

1. 凭据来源（凭据统一后）：绑定与自助查询共用登录信息 `config.user` + `config.password`（保存态 `passwordSaved`），判断 `hasCred`/`credReady`（`account/AccountPanel.tsx:206`、`account/SelfServicePanel.tsx:82`）；无面板级凭据输入与 blur 落盘，后端 `resolve_credential_password` 回退登录密码（见 [[credential-single-source]]）。
2. 条件判断：`hasCred`（`account/SelfServicePanel.tsx:82`）/ `canQueryStatus`（`account/AccountPanel.tsx:213`）。
3. 验证门：`ensureSelfVerified = useSelfServiceVerify()`（`account/SelfServicePanel.tsx:66`，实现 `account/selfServiceState.ts:77` 起）或 `ensureHelloVerified = useHelloGate()`（`account/AccountPanel.tsx:217`，实现 `account/selfServiceState.ts:37` 起）→ `api.verifyWindowsIdentity({ consentMessage })` → `invoke('verify_windows_identity', { consentMessage })`（`hooks/tauriApi.ts:165`）；通过后写模块级时间戳。
4. 门通过后执行命令：`querySelfDashboard`（`account/SelfServicePanel.tsx:84` → `invoke('query_self_dashboard')` `hooks/tauriApi.ts:167`）、`selfOfflineSession`（`account/SelfServicePanel.tsx:119` → `invoke('self_offline_session')` `hooks/tauriApi.ts:168`）、`querySelfOnlineLog`（`account/SelfServicePanel.tsx:166` → `invoke('query_self_online_log')` `hooks/tauriApi.ts:169`）、`getBindStatus`（`account/AccountPanel.tsx:219` → `invoke('query_bind_status')` `hooks/tauriApi.ts:164`）、`revealOperatorCredential`（`account/AccountPanel.tsx:250` → `invoke('reveal_operator_credential')` `hooks/tauriApi.ts:166`）、`bindOperator`（`account/AccountPanel.tsx:276` / `settings/OnboardingWizard.tsx:163` → `invoke('bind_operator')` `hooks/tauriApi.ts:163`）。
5. 首页两卡复用同一门与同一批命令：`auth/DashboardPanel.tsx:397` 起（`useSelfCardReveal`）与 `auth/DashboardPanel.tsx:413` 起（`useSelfCardFetch`）。

### 流程五：网卡刷新 / DHCP / DNS 优化

1. 获取新 IP（指定网卡）：`handleGetNewIpForAdapter`（`network/NetworkPanel.tsx:327`）→ `ipc.dhcpReleaseRenewAdapter`（`network/NetworkPanel.tsx:330`）→ `invoke('dhcp_release_renew_adapter', { adapterName })`（`hooks/tauriApi.ts:181`）→ `announceDhcpResults(normalizeDhcpResults(result), addToast)`（`network/NetworkPanel.tsx:333`，实现 `network/useNetwork.ts:15-37`）→ `refreshAdapterData()`（`network/NetworkPanel.tsx:341` → `hooks/useAdapterStore.ts:16-35`，内部并行 `get_adapters` / `get_adapter_details`）。
2. 启用被禁用网卡：`handleEnableAdapter`（`network/NetworkPanel.tsx:344`）→ `ipc.enableAdapter`（`network/NetworkPanel.tsx:347`）→ `invoke('enable_adapter', { adapterName })`（`hooks/tauriApi.ts:148`）→ `refreshAdapterData({ force: true, includeDisabled: true })`（`network/NetworkPanel.tsx:361`）。
3. DNS 检测与一键优化：`handleCheckDns`（`network/NetworkPanel.tsx:252`）→ `ipc.checkDnsDohStatus`（`:255`，IPC `hooks/tauriApi.ts:227`）；`handleSetupDnsDoh`（`network/NetworkPanel.tsx:275`）→ `ipc.setupDnsDoh(dnsFamily)`（`:278`，IPC `hooks/tauriApi.ts:228`），成功后独立 try 再刷状态（`:283-289`）；一键恢复默认 DNS：`handleResetDns`（`network/NetworkPanel.tsx:301`）→ `ipc.resetDns()`（`:304`，IPC `hooks/tauriApi.ts:229`），同样独立 try 再刷状态（`:309-315`）。
4. 面板被动刷新：`useAdapterStore` 由 `useEventListeners` 订阅 `adapters-changed` / `adapter-details-changed` / `disabled-adapters-changed`（`hooks/tauriApi.ts:165-167`）落库，`network/NetworkPanel.tsx:173` 与 `monitor/MonitorPanel.tsx:78` 直接消费。

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

1. **关闭质量检测后 quality 面板渲染 `null`**：`App.tsx:343-351` 在 `configEnableNetworkQuality === false` 时返回 `null`，若当前正停留在 `quality` 主区域空白。当前靠 `SettingsPanel.tsx:582-592` 联动切走（`setActivePanel('dashboard')`）兜底，但任何其他路径（如外部改配置、`config-changed` 回流）关闭该开关时仍会空白。
2. **`deferredPanel` 与 `activePanel` 的瞬时分歧**：主区内容与标题用 `deferredPanel`（`App.tsx:290`、`App.tsx:293`、`App.tsx:427`），而 `DockNav` 高亮用 `activePanel`（`components/layout/DockNav.tsx:405`）——并发渲染延迟期间会出现「Dock 已切换、内容还是旧面板」，属预期取舍但应在排查视觉问题时先想到。
3. **切换锁时长与动画时长硬耦合**：`App.tsx:287` 的 60ms 锁硬编码，注释明确它只覆盖 `mode="wait"` 的退出动画 0.04s（`lib/animations.ts:32`）；任一侧改动都会静默失配（锁过短→快速点击穿插旧内容，过长→吞点击）。
4. **渲染期取 store 快照的非响应式写法（已解决）**：原 `App.tsx:145` 用 `useConfigStore.getState().api` 在 render 中取值——`api` 恒定故无实际缺陷，但同文件 `App.tsx:152` 的注释记录了「此前在 JSX props 里 `getState()` 取快照，非响应式」这一历史问题，本行是同类写法残留；2026-10-03 重构删除 `useConfigStore.api` 字段后，`App.tsx` 改静态导入 `tauriApiWithRetry`，该残留随之消失。
5. **`LogPanel` 已回归静态导入但注释保留旧结论**：`App.tsx:46-48` 的注释说明 `LogPanel` 从 `lazy` 回归静态导入的原因（`React.lazy` + `Suspense` 让切换从 ~65ms 恶化到 ~366ms），而 `App.tsx:20` 仍是静态导入、`App.tsx:29` 注释称「仅低频的 LogPanel/对话框保留懒加载」——注释与实现不一致。
6. **生产环境不启用 `StrictMode`**：`main.tsx:115-117` 仅 DEV 包 `StrictMode`。因此面板里大量「StrictMode setup→cleanup→setup 恢复 mountedRef」的防御（`AccountPanel.tsx:72-77`、`DashboardPanel.tsx:271-279`、`NetworkPanel.tsx:59-64`、`hooks/useAsyncLock.ts:12-17`）在生产不受考验，但也意味着这些 double-invoke 类问题只在 DEV 暴露。
7. **崩溃自恢复只处理三类特征**：`main.tsx:68-74` 只匹配 `GPU` / `WebGL` / `SharedArrayBuffer`；其他渲染进程错误不触发重载，只靠心跳（`main.tsx:90-110`）。心跳阈值 10s（`main.tsx:106`）在长任务阻塞场景下是刻意放宽的权衡。

### auth 域

9. **（已失效）首页「仅展示前 3 台」限制已移除**：首页在线设备改为 `OnlineDeviceCardStack` 全量卡片堆叠（`auth/DashboardPanel.tsx:549-568`），本条原文描述的 `.slice(0, 3)` 截断与 `+N` 计数已不存在，完整列表直接渲染。
10. **60s 轮询不看可见性**：`auth/DashboardPanel.tsx:505` 的 `setInterval` 在窗口最小化/隐藏时仍持续发 `query_self_dashboard` IPC，无 `document.visibilityState` 判断（对比 `App.tsx:248-254` 的赞助弹层有可见性判断）。
11. **布局持久化静默失败**：`auth/DashboardPanel.tsx:60-66` 的 `catch {}` 空块——`campus-dashboard-layout` 内容损坏时直接回退 `DEFAULT_LAYOUT`（`auth/DashboardPanel.tsx:67`），用户自定义布局无提示地丢失。
12. **更新兜底资产名硬编码**：`auth/AboutDialog.tsx:241` 的兜底 URL 拼 `Wxxy-CampusLogin_${version}_x64-setup.exe`，与 Release 资产命名强耦合；缓存路径缺 `assets` 时（`auth/AboutDialog.tsx:176-178` 注释所述）依赖它命中，命名一旦变更即 404。
13. **`windowsAsset` 只识别 `.exe` / `.msi`**：`auth/AboutDialog.tsx:206-208` 与 `auth/AboutDialog.tsx:263-265`，Release 若只发便携版 zip 或 APK，一键下载取不到资产。
14. **`handleCheckUpdate` 依赖数组缺 `t`**：`auth/AboutDialog.tsx:115` 的依赖为 `[api, onUpdateAvailable]`，但内部用 `t(...)`（`auth/AboutDialog.tsx:107-111`）——切换语言后该回调仍用旧语言的 `t`。同类问题见 `network/NetworkPanel.tsx:99`（依赖 `[ipc]`，内部用 `t` 于 `network/NetworkPanel.tsx:89`）。
15. **`LoginResult` 为空扩展接口**：`auth/types.ts:14` `interface LoginResult extends CommandResult {}`，无新增字段，属可合并的写法（部分 lint 规则会报 `no-empty-interface`）。

### account 域

16. **清登录密码按钮缺少 `mousedown` 拦截（竞态）**：`account/AccountPanel.tsx:346-349` 的「清除已保存密码」按钮只有 `onClick={handleClearPassword}`（`account/AccountPanel.tsx:349`），没有 `onMouseDown={(e) => e.preventDefault()}`；若用户此刻在密码框里刚输入了草稿，点击按钮会先触发 `handlePasswordBlur`（`account/AccountPanel.tsx:95` 起）提交 `onUpdateConfig({ password: draft })`（进入 500ms debounce，`hooks/useConfigStore.ts:82-92`），随后才执行清空保存。后果：清除操作被随后触发的 debounce 保存覆盖（后端「空密码=保留旧密码」语义下，密码并没有被真正清除）。历史上同文件的自助密码清除按钮曾以「点击时先清空本地草稿」缓解同类竞态——凭据统一后独立自助凭据输入已整体删除（见 [[credential-single-source]]），本处的登录密码清除按钮成为该竞态的唯一残留点。
17. **`BIND_OPERATOR_NONE` 定义在组件体内**：`account/AccountPanel.tsx:182` 每次渲染重建（值恒定为 `'__none__'`，无功能影响）；同语义常量在 `settings/OnboardingWizard.tsx:49` 是模块级，位置不一致。
18. **`switchingAccount` 清态无 mounted 守卫**：`handleSwitchAccount`（`account/AccountPanel.tsx:137` 起）的 `finally { setSwitchingAccount(null) }` 未判 `mountedRef.current`，而同文件新增账号路径（`handleAddAccount`，`account/AccountPanel.tsx:120` 起）做了守卫——异步返回时组件已卸载会触发一次无意义的 setState。
19. **Hello 门是模块级单例，无显式重置入口**：绑定门与会话门的通过时间戳都是 `account/selfServiceState.ts` 的模块级变量，生产代码只能等 `VERIFY_TTL_MS`（`account/selfServiceState.ts:26`，570s）过期或时钟回拨（`gateFresh`，`account/selfServiceState.ts:33`）；测试必须 `vi.resetModules()` 重置（`account/AccountPanel.helloGate.test.tsx:55`、`auth/DashboardPanel.selfCards.test.tsx:79`）。副作用：向导内绑定验证（`settings/OnboardingWizard.tsx:120`）与账户面板绑定卡共用同一门，任一处验证通过后另一处不再验证。
20. **会话门只按面板卸载重置**：`account/SelfServicePanel.tsx:76` 在卸载时 `resetSelfSessionGate()`；该面板在 `App.tsx` 是静态导入但只在被选中时挂载，因此「切走再切回必然重验」成立；但若面板因 `ErrorBoundary` 兜底被替换（`App.tsx:434`），门不会被重置而状态丢失。
21. **（已失效）账单明细已改用稳定 key**：`account/SelfOnlineRecords.tsx:267`/`:325` 用 `key={row.key}`（行对象自带稳定 key），本条原文描述的 `key={idx}` 下标复用错位风险不再存在。
22. **500 条截断阈值硬编码**：`account/SelfOnlineRecords.tsx:288` 的 `logRows.length >= 500` 直接提示「超出部分未显示」，阈值不来自配置也不来自后端返回。
23. **首页与自助页对同一协议各定义一份类型**：`auth/DashboardPanel.tsx:459` 与 `account/SelfServicePanel.tsx:31` 的 `SelfOnlineItem` 字段一致但重复定义，任一侧协议变更需同步两处。
24. **`fmtMoney` 与 `toInt` 语义分叉在历史行上并存**：`account/SelfOnlineRecords.tsx:273`/`:338` 用 `fmtMoney`（保留小数），`account/SelfOnlineRecords.tsx:63-64`/`:69`/`:79` 的时长/流量用 `toInt`（截断）——同一张表两种数值口径，注释（`account/SelfOnlineRecords.tsx:27-28`）解释了原因（金额可能为小数），阅读时易误判。

### monitor 域

25. **数字/文本输入的钳制只在 commit 生效**：`monitor/MonitorPanel.tsx:98-105`（10–600s）与 `monitor/QualityPanel.tsx:160-167`（10–600s）的 `min` / `max` 属性（`monitor/MonitorPanel.tsx:184-185`、`monitor/QualityPanel.tsx:294-296`）只是浏览器提示，真实值靠 blur/Enter 时的 `Math.min/max` 兜底；`parseInt(intervalDraft) || 60` 意味着输入非数字会回落到缺省值而非报错。
26. **主适配器徽标依赖精确名匹配**：`monitor/MonitorPanel.tsx:203` 传 `isPrimary={s.name === config.adapter1}`；当 `adapter1` 为空或 `'自动检测'`（`network/adapters.ts:3`）时没有任何卡片会被标主，与 `AuthStore.checkOnline` 的自动选择（`hooks/useAuthStore.ts:68-79`）口径不同；`StatusBar` 则用 `AUTO_DETECT_ADAPTER` 显式排除（`monitor/StatusBar.tsx:54-55`）。
27. **`campusExitOnFail` 用 `?? true` 隐式默认**：`monitor/MonitorPanel.tsx:369` 在字段缺失时视为开启（与 `settings/constants.ts:41` 的默认值一致），但 UI 上无法区分「显式开启」与「未配置」。
28. **质量明细在首次无 `details` 时整块占位**：`monitor/QualityPanel.tsx:188` 的 `hasData = !!networkQuality?.details`，只要后端首包不带 `details`（例如只推 `gatewayLatency`），明细区所有项都渲染 `--`（`monitor/QualityPanel.tsx:404-405`）与脉冲占位条（`monitor/QualityPanel.tsx:419-423`）。
29. **定时测试运行徽标只读本地开关**：`monitor/QualityPanel.tsx:305-309` 的「运行中」徽标只看 `config.enableLatencyTest`，不反映后端 `start_latency_test` 的真实状态；若后端因校验拒绝启动（`settings/SettingsPanel.tsx:516-519` 注释提到的同向校验），UI 仍显示运行中。
30. **`StatusBar` 的 `wasOffline` 是「上一次渲染的状态」**：`monitor/StatusBar.tsx:42` 在渲染期读 `prevStatusRef.current`，而 ref 在 `monitor/StatusBar.tsx:44-46` 的 effect 中更新——语义为「上上次状态」，仅用于动画类名选择（`monitor/StatusBar.tsx:116-121`），不参与文案；阅读时勿当作当前状态。
31. **胶囊的两个 `useMemo` 依赖数组刻意收窄**：`monitor/NetworkQualityCapsule.tsx:65-69`（依赖 `networkQuality?.gateway`）与 `monitor/NetworkQualityCapsule.tsx:79-83`（依赖 `networkQuality?.details?.['dnsResolve']`）——依赖列表比闭包实际读取量窄，属有意优化但与 `react-hooks/exhaustive-deps` 冲突。
32. **`QualityPanel` 的子标签切换动画注释保留历史缺陷说明**：`monitor/QualityPanel.tsx:370-374` 解释了必须把 `TooltipProvider` 放进 `m.div` 内（否则 `AnimatePresence mode="wait"` 无法感知 key 变化）；该约束是「改结构就退化为无动画」的脆弱点。

### network 域

33. **推荐 DNS 白名单硬编码在前端**：`network/NetworkPanel.tsx:36-38`（阿里/腾讯 IPv4+IPv6 共 7 个地址）。「推荐」判定（`network/NetworkPanel.tsx:205-209`）与提示文案都基于这份清单，与后端 DNS 优化实际下发的清单可能分叉；新增推荐源需改前端。
34. **两个刷新入口参数不一致**：`network/NetworkPanel.tsx:168` 的 `refreshAdapterData()` 不带 `includeDisabled`，`network/NetworkPanel.tsx:188` 带 `{ force: true, includeDisabled: true }`；前者在网卡被禁用/启用的瞬间可能拿到过期列表（`network/NetworkPanel.tsx:187` 注释仅解释了后者）。
35. **`handleCheckDns` 的依赖数组缺 `t`**：`network/NetworkPanel.tsx:100` 依赖 `[ipc]`，但 `network/NetworkPanel.tsx:90`、`network/NetworkPanel.tsx:92` 用 `t(...)` 写日志——切语言后该回调仍用旧语言。
36. **`getDnsQuality` 在适配器级覆盖 profile 时的判定分支**：`network/NetworkPanel.tsx:202` 的 `adapter.adapterDnsOverridesProfile ? servers : (servers.length > 0 ? servers : profileServers)`——当「适配器级为空且 profile 有值」时用 profile 值；警告 UI（`network/NetworkPanel.tsx:572-577`）与判定分别独立计算，二者条件不同（警告多要求 `profileDnsServers.length > 0`），极端组合下可能出现「判为 excellent 但显示覆盖警告」。
37. **`dnsStatus` 为空与 `dnsChecking` 的组合态**：`network/NetworkPanel.tsx:519-531` 分三个互斥分支渲染（未检测 / 检测中 / 有结果）；当检测失败被 `catch` 吞掉并置 `null`（`network/NetworkPanel.tsx:95-97`）时 UI 回到「点击检测」空态，用户无法区分「没查」与「查失败」。

### settings 域

38. **`handleSecurityDisable` 定义在 `t` 之前（顺序脆弱）**：`settings/SettingsPanel.tsx:122-137` 的闭包引用 `t`，而 `const { t } = useTranslation()` 在 `settings/SettingsPanel.tsx:138` 才声明——依赖「函数只在事件触发时才执行」这一事实避开 TDZ；若将来有人在渲染期调用它会抛 `ReferenceError`。
39. **`OnboardingWizard` 可能把明文密码回填进输入框**：`settings/OnboardingWizard.tsx:109` 与 `settings/OnboardingWizard.tsx:135` 的写法是 `config.password === PASSWORD_MASK ? '' : (config.password || '')`——只有在后端回传 MASK 时才清空；若后端在某窗口期回传明文（`Config.password` 类型为 `string`），明文会被填入 `type="text"` 的可切换输入框（账号信息步视图，`settings/OnboardingWizard.tsx:391` 起）。账户/自助面板均改用独立布尔 `passwordSaved`（凭据统一后唯一保存态，见 [[credential-single-source]]）规避该状态机（`account/AccountPanel.tsx:59`、`account/SelfServicePanel.tsx:49`），向导未同步该修法。
40. **`handleLoginAndFinish` 校验失败时静默返回**：`settings/OnboardingWizard.tsx:236` 起（`:240` 账号必填分支）的两个 `return` 分支只 `setLoginSuccess(false)`，不给 toast/内联错误；用户点「开始登录」无任何反馈（对比 `settings/OnboardingWizard.tsx:194` 的 `canProceedAccount` + 账号信息步（step 1）的禁用态 + `onboarding.pleaseComplete` 提示，视图 `settings/OnboardingWizard.tsx:391` 起）。
41. **关窗二次确认会吞掉首次关闭意图**：`settings/OnboardingWizard.tsx:288` 的 `onPointerDownOutside` 被 preventDefault——点遮罩不会关闭向导，只弹确认框（`showCloseConfirm` 状态 `OnboardingWizard.tsx:116`）；这是有意设计，但若内层确认框的 `Dialog` 与向导同时存在于 DOM，键盘 Esc 的行为需注意（`settings/OnboardingWizard.tsx:668` 的确认框没有显式 Esc 处理）。
42. **`SettingsPanel` 关闭质量检测的联动只覆盖三处**：`settings/SettingsPanel.tsx:583-592` 处理 `enableLatencyTest`、`defaultPanel`、`activePanel`；但不回滚 `speedtest` 面板的默认值，也不清理 `useQualityStore.networkQuality` 里已缓存的结果（关闭后 `StatusBar`（`monitor/StatusBar.tsx:143`）不再显示胶囊，但 store 中的数据仍在）。
43. **`fixedGateway` 清空按钮是原生 `button`**：`settings/SettingsPanel.tsx:656-662` 与输入框（`settings/SettingsPanel.tsx:645-654` 同样是原生 `input`）未走项目 UI 组件；行为一致但样式/可访问性（无 `aria-label`、无 label 关联）与同卡其他控件不齐。
44. **取色器草稿的 80ms 节流与 blur 双通道**：`settings/SettingsPanel.tsx:160-168` 定时提交 + `settings/SettingsPanel.tsx:177-183` 的 `commitColorDraft` 走 `onBlur`——`<input type="color">`（`settings/SettingsPanel.tsx:272-278`）在部分平台不触发 blur，此时仅靠 80ms 定时器提交（`settings/SettingsPanel.tsx:163-167`），逻辑正确但存在两条提交路径需同时维护。
45. **`defaultPanel` 哨兵值与 Radix 约束**：`settings/SettingsPanel.tsx:419-420` 用 `'__remember__'` 映射空串（Radix `SelectItem` 不接受空 `value`）；`settings/SettingsPanel.tsx:427` 过滤 `quality`，但未考虑 `enableNetworkQuality` 之外的其它无效面板组合；同时在 `settings/constants.ts:73-76` 用 `NAV_ITEMS` 派生选项，`NAV_ITEMS` 含全部 9 项（`shared/ui-constants.ts:7-17`）。
