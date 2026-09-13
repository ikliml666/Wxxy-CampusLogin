---
title: 安卓前端 - 运行时核心（入口 / 领域 store / 钩子 / 外壳组件）
type: module
source_files:
  - android/frontend/src/App.tsx
  - android/frontend/src/main.tsx
  - android/frontend/src/index.css
  - android/frontend/src/vite-env.d.ts
  - android/frontend/src/hooks/tauriApi.ts
  - android/frontend/src/hooks/useAdapterStore.ts
  - android/frontend/src/hooks/useAdaptiveFramePace.ts
  - android/frontend/src/hooks/useAnimationProfile.ts
  - android/frontend/src/hooks/useAppInit.ts
  - android/frontend/src/hooks/useAppStore.ts
  - android/frontend/src/hooks/useAsyncLock.ts
  - android/frontend/src/hooks/useAuthStore.ts
  - android/frontend/src/hooks/useBreatheAnimation.ts
  - android/frontend/src/hooks/useConfigStore.ts
  - android/frontend/src/hooks/useDeviceProfile.ts
  - android/frontend/src/hooks/useEventListeners.ts
  - android/frontend/src/hooks/useFormFactor.ts
  - android/frontend/src/hooks/useGlobalShortcut.ts
  - android/frontend/src/hooks/useGlowAnimation.ts
  - android/frontend/src/hooks/useHeartbeat.ts
  - android/frontend/src/hooks/useInitialDataLoad.ts
  - android/frontend/src/hooks/useLogToastStore.ts
  - android/frontend/src/hooks/usePageIdle.ts
  - android/frontend/src/hooks/usePulseAnimation.ts
  - android/frontend/src/hooks/useQualityStore.ts
  - android/frontend/src/hooks/useStartupBoost.ts
  - android/frontend/src/hooks/useThemeStore.ts
  - android/frontend/src/lib/animations.ts
  - android/frontend/src/lib/color.ts
  - android/frontend/src/lib/easing-config.ts
  - android/frontend/src/lib/latency.ts
  - android/frontend/src/lib/renderLiveness.ts
  - android/frontend/src/lib/utils.ts
  - android/frontend/src/i18n/index.ts
  - android/frontend/src/components/ui/animated-card.tsx
  - android/frontend/src/components/ui/badge.tsx
  - android/frontend/src/components/ui/button.tsx
  - android/frontend/src/components/ui/card.tsx
  - android/frontend/src/components/ui/dialog.tsx
  - android/frontend/src/components/ui/input.tsx
  - android/frontend/src/components/ui/label.tsx
  - android/frontend/src/components/ui/select.tsx
  - android/frontend/src/components/ui/separator.tsx
  - android/frontend/src/components/ui/switch.tsx
  - android/frontend/src/components/ui/tooltip.tsx
  - android/frontend/src/components/layout/BottomNav.tsx
  - android/frontend/src/components/layout/DockNav.tsx
  - android/frontend/src/components/layout/RightPanel.tsx
  - android/frontend/src/components/layout/TitleBar.tsx
  - android/frontend/src/components/mobile/MobileDashboard.tsx
  - android/frontend/src/components/mobile/MobileMore.tsx
  - android/frontend/src/components/mobile/MobileQuickActions.tsx
  - android/frontend/src/components/tablet/TabletShell.tsx
  - android/frontend/src/shared/AnimatedNumber.tsx
  - android/frontend/src/shared/ConfirmDialog.tsx
  - android/frontend/src/shared/ErrorBoundary.tsx
  - android/frontend/src/shared/FluidBackground.tsx
  - android/frontend/src/shared/MascotFigure.tsx
  - android/frontend/src/shared/RefreshButton.tsx
  - android/frontend/src/shared/SegmentTabs.tsx
  - android/frontend/src/shared/SponsorCard.tsx
  - android/frontend/src/shared/ToastContainer.tsx
  - android/frontend/src/shared/UpdateAvailableDialog.tsx
  - android/frontend/src/shared/dailyMascot.ts
  - android/frontend/src/shared/index.ts
  - android/frontend/src/shared/types.ts
  - android/frontend/src/shared/ui-constants.ts
  - android/frontend/src/shared/ui-types.ts
  - android/frontend/src/face/FaceCaptureDialog.tsx
  - android/frontend/src/face/faceService.ts
  - android/frontend/src/face/faceVerifyStore.ts
tags: [安卓, 前端, 运行时核心, store, 钩子, 外壳, 人脸验证, 双端同构]
---

## Overview

本模块是安卓前端（`android/frontend`）的运行时骨架：`main.tsx` 负责渲染前初始化（主题、崩溃自恢复、gsap 全局配置），`App.tsx` 按短边 ≥600 CSS px 在「手机移动外壳」与「平板 Dock 外壳」之间二选一，`hooks/` 提供六个 zustand 领域 store、IPC 封装 `tauriApi` 与全部生命周期钩子，`components/{ui,layout,mobile,tablet}` 与 `shared/` 提供组件库与看板娘/弹窗/日志等通用件。

与桌面前端的根本差异：安卓前端是 `tauri-app/frontend/src` 的**独立复刻代码库**（同构不等于共享代码），文件级差异见 Known Issues；安卓独有目录为 `components/{mobile,tablet}`、`face/`、`hooks/{useDeviceProfile,useAdaptiveFramePace,useFormFactor}`、`shared/dailyMascot.ts`、`shared/UpdateAvailableDialog.tsx`、`settings/OnboardingWizardMobile.tsx`、`settings/useOnboardingFlow.ts`、`settings/KeepAliveSettingsCard.tsx`。

IPC 面同样是分叉的：`hooks/tauriApi.ts` 把桌面专有能力（适配器枚举、DHCP、DNS DoH、窗口与托盘、GPU 信息）统一改成显式 reject 或 no-op 监听（`desktopOnly` / `noopListener`，`tauriApi.ts:14` / `tauriApi.ts:18`），并新增 `getSocInfo` / 电池优化 / 厂商省电页等安卓专属命令。

## Key Components

### 入口与根组件

| 导出 | 位置 | 用途 |
| --- | --- | --- |
| `App`（default export） | `App.tsx:296` | 根组件。调用 `useFormFactor()`（`App.tsx:297`）判定形态，`tablet` 渲染 `<TabletShell />`、否则 `<AppInner />`（`App.tsx:301`）；外层包 `ErrorBoundary` + `AnimationActiveProvider`，并在双外壳之外单例挂载 `<FaceCaptureDialog />`（`App.tsx:303`） |
| `AppInner`（未导出，模块内组件） | `App.tsx:47` | 手机移动外壳：顶部轻 header（安全点 + 质量胶囊 + 心跳赞助/设置/主题/关于四图标，`App.tsx:164-200`）、单列滚动主区（`App.tsx:203-224`）、首页快捷登录条（`App.tsx:227-231`）、`BottomNav`（`App.tsx:233`）、概览/账号/自助/质量/更多 五 tab 切换（`App.tsx:110-154`） |

`main.tsx` 无导出，执行顺序即其职责：

| 位置 | 内容 |
| --- | --- |
| `main.tsx:17-19` | `gsap.defaults({ease:'expo.out'})`、`gsap.config({autoSleep:5,nullTargetWarn:false})`、`gsap.ticker.lagSmoothing(500,33)`；注释说明不设全局 `force3D` |
| `main.tsx:21-25` | `prefers-reduced-motion` 命中时 `duration:0` + `lagSmoothing(0)` |
| `main.tsx:27-41` / `main.tsx:43` | `initTheme()`：读 `campus-light-mode` 切 `dark` class 与 `data-light` 属性，读 `campus-theme` 加 `theme-<name>` class；模块加载即执行（防主题闪烁） |
| `main.tsx:45-111` / `main.tsx:113` | `setupCrashRecovery()`：`root` 空且可见时重载（≤3 次，`main.tsx:59-66`）；`GPU/WebGL/SharedArrayBuffer` 报错重载（`main.tsx:68-74`）；可见性切换停/启 `gsap.globalTimeline`（`main.tsx:79-88`）；1s 心跳更新（渲染链失活则跳过，`main.tsx:90-100`）；2s 检测心跳丢失 >10s 触发恢复（`main.tsx:102-110`） |
| `main.tsx:115-117` | `AppWrapper`：DEV 用 `React.StrictMode`，生产用 `React.Fragment` |
| `main.tsx:119-129` | 渲染树：`ErrorBoundary` → `LazyMotion features={domMax} strict` → `MotionConfig reducedMotion="user"` → `App` |

### hooks/ — IPC 封装

`tauriApi.ts` 的导出与其内部结构：

| 导出 | 位置 | 说明 |
| --- | --- | --- |
| `SocInfo` | `tauriApi.ts:27` | SoC 分级信息接口 |
| `BatteryOptimizationInfo` | `tauriApi.ts:34` | 电池优化白名单状态接口 |
| `VendorSettingsResult` | `tauriApi.ts:40` | 厂商省电页跳转结果接口 |
| `tauriApiWithRetry` | `tauriApi.ts:317` | 生产使用的 API 对象：`...tauriApi` 展开后仅覆写 `saveConfig` 包重试（`tauriApi.ts:322`） |

内部（未导出，但被全仓引用的是 `tauriApiWithRetry`）：`desktopOnly`（`tauriApi.ts:14`，桌面专属命令统一 reject）、`noopListener`（`tauriApi.ts:18`，桌面专属事件返回 no-op 清理函数）、`ConnectionCampusStatus`（`tauriApi.ts:20`）、`CampusStatusResult`（`tauriApi.ts:46`）、`TauriApi` 接口（`tauriApi.ts:56-133`）、`tauriApi` 常量实现对象（`tauriApi.ts:166-285`）、`createEventListener`（`tauriApi.ts:135`，具名事件订阅工厂，处理「listen 未完成即取消」）、`isRetryableError`（`tauriApi.ts:287`）、`withRetry`（`tauriApi.ts:299`）。

`TauriApi` 全部方法（声明在 `tauriApi.ts:56-133`，实现在 `tauriApi.ts:166-285`）：

| 方法 | 声明行 | 实现行 | 安卓行为 |
| --- | --- | --- | --- |
| `getConfig` | 57 | 167 | `invoke('get_config')` |
| `saveConfig(config, clearPassword?, clearSelfPassword?)` | 58 | 168 | `invoke('save_config', {config, clearPassword, clearSelfPassword})` |
| `getAdapters(force?)` | 59 | 169 | `desktopOnly` → reject |
| `getDisabledAdapters` | 60 | 170 | `desktopOnly` → reject |
| `enableAdapter(adapterName)` | 61 | 171 | `desktopOnly` → reject |
| `getAdapterDetails` | 62 | 172 | `desktopOnly` → reject |
| `bindToWifi` | 63 | 173 | `invoke('bind_to_wifi')`，返回 `{bound}` |
| `checkPortalStatus(adapterIp)` | 64 | 174 | `invoke('check_portal_status',{adapterIp})`（安卓传空串走缓存源 IP） |
| `checkCampusStatus` | 65 | 175 | `invoke('check_campus_status')` |
| `doLogin(adapterName?)` | 66 | 176 | `invoke('do_login',{adapterName})` |
| `doLogout(adapterName?)` | 67 | 177 | `invoke('do_logout',{adapterName})` |
| `bindOperator(params)` | 68 | 178 | `invoke('bind_operator', ...params)` |
| `getBindStatus(params)` | 69 | 179 | `invoke('query_bind_status', ...params)` |
| `verifyWindowsIdentity(params)` | 70 | 180-211 | 生物识别门：`allow2dFaceVerify && hasTemplate()` 且系统生物识别不可用时走 2D 人脸（`tauriApi.ts:184-207`），否则 `biometricAuthenticate(msg,{allowDeviceCredential:true})`（`tauriApi.ts:209`）后 `invoke('verify_biometric_identity')`（`tauriApi.ts:210`） |
| `revealOperatorCredential(params)` | 71 | 212 | `invoke('reveal_operator_credential', ...params)` |
| `querySelfDashboard(params)` | 72 | 213 | `invoke('query_self_dashboard', ...params)` |
| `selfOfflineSession(params)` | 73 | 214 | `invoke('self_offline_session', ...params)` |
| `querySelfOnlineLog(params)` | 74 | 215 | `invoke('query_self_online_log', ...params)` |
| `minimizeWindow` | 75 | 216 | `desktopOnly` → reject |
| `closeWindow` | 76 | 217 | `desktopOnly` → reject |
| `onBackgroundCheckResult(cb)` | 77 | 218 | `listen('background-check-result')` |
| `onAutoLoginResult(cb)` | 78 | 219 | `listen('auto-login-result')` |
| `onAdaptersChanged(cb)` | 79 | 220 | `noopListener`（永不触发） |
| `onAdapterDetailsChanged(cb)` | 80 | 221 | `noopListener` |
| `onDisabledAdaptersChanged(cb)` | 81 | 222 | `noopListener` |
| `onAdapterDisabledWarning(cb)` | 82 | 223 | `noopListener` |
| `onLoginLog(cb)` | 83 | 224 | `listen('login-log')` |
| `listAccounts` | 84 | 225 | `invoke('list_accounts')` |
| `switchAccount(accountName)` | 85 | 226 | `invoke('switch_account',{accountName})` |
| `saveCurrentAsAccount(accountName)` | 86 | 227 | `invoke('save_current_as_account',{accountName})` |
| `deleteAccount(accountName)` | 87 | 228 | `invoke('delete_account',{accountName})` |
| `getActiveAccount` | 88 | 229 | `invoke('get_active_account')` |
| `startBackgroundCheck` | 89 | 230 | `invoke('start_background_check')` |
| `stopBackgroundCheck` | 90 | 231 | `invoke('stop_background_check')` |
| `triggerBackgroundCheck` | 91 | 232 | `invoke('trigger_background_check')` |
| `getBackgroundStatus` | 92 | 233 | `invoke('get_background_status')` |
| `dhcpRenewAll` | 93 | 234 | `desktopOnly` → reject |
| `dhcpReleaseRenew` | 94 | 235 | `desktopOnly` → reject |
| `dhcpReleaseRenewAdapter(adapterName)` | 95 | 236 | `desktopOnly` → reject |
| `checkNetworkQuality` | 96 | 237 | `invoke('check_network_quality')` |
| `onNetworkQualityResult(cb)` | 97 | 238 | `listen('network-quality-result')` |
| `startLatencyTest` | 98 | 239 | `invoke('start_latency_test')` |
| `stopLatencyTest` | 99 | 240 | `invoke('stop_latency_test')` |
| `openExternal(url)` | 100 | 241-252 | 校验协议（仅 http/https）、长度 ≤2048、`new URL` 可解析后调 `@tauri-apps/plugin-opener` 的 `openUrl` |
| `getAutoLaunch` | 101 | 253 | `invoke('get_boot_autostart')` |
| `setAutoLaunch(enabled)` | 102 | 254 | `invoke('set_boot_autostart')` 后包成 `{success:true}` |
| `getNotificationEnabled` | 103 | 255 | `invoke('get_notification_enabled')` |
| `setNotificationEnabled(enabled)` | 104 | 256 | `invoke('set_notification_enabled')` |
| `getBatteryOptimizationInfo` | 105 | 257 | `invoke('get_battery_optimization_info')`（安卓专属） |
| `requestIgnoreBatteryOptimizations` | 106 | 258 | `invoke('request_ignore_battery_optimizations')`（安卓专属） |
| `openVendorBatterySettings` | 107 | 259 | `invoke('open_vendor_battery_settings')`（安卓专属） |
| `cancelAutoExit` | 108 | 260 | `desktopOnly` → reject |
| `onAutoExitCountdown(cb)` | 109 | 261 | `noopListener` |
| `onAutoExitCancelled(cb)` | 110 | 262 | `noopListener` |
| `onCampusExitCountdown(cb)` | 111 | 263 | `noopListener` |
| `onCampusExitCancelled(cb)` | 112 | 264 | `noopListener` |
| `onConfigChanged(cb)` | 113 | 265 | `listen('config-changed')` |
| `showWindow` | 114 | 266 | `desktopOnly` → reject |
| `getLogs(lines?)` | 115 | 267 | `invoke('get_logs',{lines})` |
| `clearLogs` | 116 | 268 | `invoke('clear_logs')` |
| `getDebugMode` | 117 | 269 | `invoke('get_debug_mode')` |
| `setDebugMode(enabled)` | 118 | 270 | `invoke('set_debug_mode')` |
| `getInitData` | 119 | 271 | `invoke('get_init_data')` |
| `getSocInfo` | 120 | 272 | `invoke('get_soc_info')`（安卓专属） |
| `checkUpdate` | 121 | 273 | `invoke('check_update')` |
| `downloadUpdate(url)` | 122 | 274 | `invoke('download_update',{url})` |
| `installUpdate(filePath, checksumUrl?)` | 123 | 275 | `invoke('install_update',{filePath, checksumUrl})` |
| `getMirrorUrls(githubUrl)` | 124 | 276 | `invoke('get_mirror_urls',{githubUrl})` |
| `onDownloadProgress(cb)` | 125 | 277 | `listen('update-download-progress')` |
| `onUpdateAvailable(cb)` | 126 | 278 | `listen('update-available')` |
| `checkDnsDohStatus` | 127 | 279 | `desktopOnly` → reject |
| `setupDnsDoh(family?)` | 128 | 280 | `desktopOnly` → reject |
| `renderHeartbeat` | 129 | 281 | `desktopOnly` → reject（安卓无后端重载 WebView 机制） |
| `getGpuInfo` | 130 | 282 | `desktopOnly` → reject |
| `getLogRetentionDays` | 131 | 283 | `invoke('get_log_retention_days')` |
| `setLogRetentionDays(days)` | 132 | 284 | `invoke('set_log_retention_days',{days})` |

### hooks/ — 六个领域 store

`useConfigStore`（配置域）：

| 导出 | 位置 | 说明 |
| --- | --- | --- |
| `useConfigStore` | `useConfigStore.ts:53` | 状态：`config` / `configLoaded` / `passwordSaved` / `selfPasswordSaved` / `accounts` / `activeAccount` / `language` / `api`；动作：`updateConfig`(63) / `updateConfigLocal`(97) / `mergeConfigFromBackend`(110) / `clearDirtyFields`(119) / `syncPasswordSaved`(124) / `syncSelfPasswordSaved`(126) / `saveConfigDirect`(128) / `setAccounts`(173) / `setActiveAccount`(174) / `setLanguage`(176) |
| `hasPendingConfig` | `useConfigStore.ts:183` | debounce 待存或 in-flight 保存存在即 true（关窗前判定） |
| `flushPendingConfig` | `useConfigStore.ts:190` | 清 debounce 定时器、立即发出待存配置、返回需等待的 Promise（含 in-flight） |

模块级私有状态：`saveConfigTimer`(14) / `saveConfigPending`(15) / `saveConfigInFlight`(17) / `dirtyFields`(20) / `DIRTY_FAILURE_LIMIT=3`(24) / `dirtyFailureCounts`(25)。

`useAuthStore`（认证域）：导出 `useAuthStore`（`useAuthStore.ts:103`），动作 `doLogin`(109) / `doLogout`(170) / `checkOnline`(201) / `setStatus`(254) / `setBgStatus`(255)；模块级私有：`_checkOnlineLockFlag`(18)、`QUALITY_MANUAL_THROTTLE_MS=60000`(23)、`detectCampusNetwork`(31)、`buildCampusBgStatusPatch`(40)、`queryPortalStatus`(71)、`withTimeout`(84)。

`useAdapterStore`（适配器域）：导出 `refreshAdapterData`（`useAdapterStore.ts:16`，并行拉 `getAdapters`/`getAdapterDetails`/可选 `getDisabledAdapters` 后写回 store，安卓下端三个命令均 reject 并被 `.catch(() => undefined)` 吞掉）、`useAdapterStore`（`useAdapterStore.ts:48`，状态 `adapters` / `disabledAdapters` / `adapterDetails` / `isRefreshingAdapters` / `activePanel`，动作 `refreshAdapters`(55) / `setAdapters`(72) / `setActivePanel`(73)）。

`useQualityStore`（质量与更新域）：导出 `getLastQualityResultTime`（`useQualityStore.ts:19`）、`useQualityStore`（`useQualityStore.ts:46`，状态 `networkQuality` / `dnsDohStatus` / `dnsChecking` / `isRefreshingQuality` / `updateAvailable` / `latestVersion` / `releaseNotes` / `updatePromptOpen` / `gpuInfo` / `refreshRate`，动作 `refreshQuality`(58) / `setNetworkQuality`(82) / `setDnsDohStatus`(87) / `setDnsChecking`(88) / `setUpdateAvailable`(89) / `setLatestVersion`(90) / `setReleaseNotes`(91) / `setUpdatePromptOpen`(92) / `setGpuInfo`(94)）。

`useLogToastStore`（日志与 toast 域）：导出 `useLogToastStore`（`useLogToastStore.ts:28`），状态 `logs` / `toasts`，动作 `addLog`(32) / `addToast`(52) / `addToastWithAction`(84) / `removeToast`(110) / `removeToastsByPrefix`(116) / `setLogs`(128) / `cleanupToasts`(130)；模块级 `toastTimers`(5) / `MAX_TOASTS=4`(9) / `isDuplicateTitle`(13)。

`useThemeStore`（主题域）：导出 `useThemeStore`（`useThemeStore.ts:19`），状态 `themeName` / `isLightMode` / `customThemeColor`，动作 `setThemeName`(24) / `setIsLightMode`(25) / `initTheme`(27) / `setCustomThemeColor`(49)；**模块级副作用订阅在 `useThemeStore.ts:53-86`**（切 `dark` class / `data-light` 属性、按 `theme-custom` 用 `hexToHsl` 写 `--primary`/`--ring`/`--accent`/`--accent-foreground`）。

`useAppStore.ts` 只剩兼容 re-export：`useAppInit`（`useAppStore.ts:2`）、`hasPendingConfig` 与 `flushPendingConfig`（`useAppStore.ts:3`）。

### hooks/ — 生命周期与动画钩子

| 导出 | 位置 | 用途 |
| --- | --- | --- |
| `useAppInit` | `useAppInit.ts:6` | 顺序调用 `useEventListeners` → `useInitialDataLoad` → `useHeartbeat` → `useGlobalShortcut`（`useAppInit.ts:7-10`） |
| `useEventListeners` | `useEventListeners.ts:18` | 集中注册全部后端事件订阅（单 useEffect），含窗口关闭 flush（`useEventListeners.ts:50-75`） |
| `useInitialDataLoad` | `useInitialDataLoad.ts:16` | 启动唯一编排入口（见 Data Flow） |
| `useHeartbeat` | `useHeartbeat.ts:5` | 5s 一次 `renderHeartbeat`，隐藏或 `isRenderLoopAlive()` 为假时跳过（`useHeartbeat.ts:13`） |
| `useGlobalShortcut` | `useGlobalShortcut.ts:4` | 全局 `Ctrl+Shift+C` → `cancelAutoExit`，可编辑元素内忽略（`useGlobalShortcut.ts:10-16`） |
| `useFormFactor` | `useFormFactor.ts:38` | 短边 ≥600px 判平板（`useFormFactor.ts:13`），resize/orientationchange 重算（`useFormFactor.ts:24-36`） |
| `useOrientation` | `useFormFactor.ts:43` | 宽≥高为 `landscape`（`useFormFactor.ts:19`） |
| `useDeviceProfile` | `useDeviceProfile.ts:93` | 设备性能分级：同步用 WebGL renderer + 核数/内存（`useDeviceProfile.ts:43-67`），异步用 `getSocInfo` 只升不降（`useDeviceProfile.ts:72-91`） |
| `useAdaptiveFramePace` | `useAdaptiveFramePace.ts:51` | 把 `useDeviceProfile` 的 idle/active fps 落到 `gsap.ticker.fps()`，250ms 轮询回落、事件回调即时提帧 |
| `markInteraction` | `useAdaptiveFramePace.ts:26` | 程序性交互（面板切换）主动推活跃窗口 |
| `useAnimationProfile` | `useAnimationProfile.ts:73` | 由 `refreshRate`/`gpuInfo`/reduced-motion 解析动画档（`useAnimationProfile.ts:85-94`） |
| `AnimationActiveProvider` | `usePageIdle.ts:85` | 全局单例注册「可见 + 有焦点 + 未空闲」判定，`useMemo` 稳定 value |
| `useAnimationActive` | `usePageIdle.ts:95` | 消费上述 context |
| `useBreatheAnimation` | `useBreatheAnimation.ts:15` | gsap yoyo 呼吸动画 ref，空闲暂停（`useBreatheAnimation.ts:55-63`） |
| `useGlowAnimation` | `useGlowAnimation.ts:11` | 发光脉冲动画 ref，空闲暂停（`useGlowAnimation.ts:42-50`） |
| `usePulseAnimation` | `usePulseAnimation.ts:15` | 三种脉冲时间线（heartbeat/statusPulse/loadingPulse，`usePulseAnimation.ts:27-52`），空闲暂停 |
| `useAsyncLock` | `useAsyncLock.ts:3` | 防重复提交锁，返回 `[isRunning, execute]`，`cooldownMs` 默认 1500 |
| `useStartupBoost` | `useStartupBoost.ts:15` | 桌面启动入场序列（标题栏/状态栏/标题/Dock/右栏 gsap timeline）。**安卓无调用方**，见 Known Issues |

`useAnimationProfile` 的档位解析：`HIGH_PROFILE`（`useAnimationProfile.ts:30`）、`ECONOMY_OVERRIDES`（`useAnimationProfile.ts:52`）、`resolveTier`（`useAnimationProfile.ts:60`，reduced-motion 或 `low-igpu` → economy，`discrete`/`high-igpu` → high，其余 standard）、模块级 `reducedMotionQuery`（`useAnimationProfile.ts:69`）。

### lib/

| 导出 | 位置 | 用途 |
| --- | --- | --- |
| `cn` | `lib/utils.ts:4` | `clsx` + `tailwind-merge` |
| `extractErrorMessage` | `lib/utils.ts:8` | 字符串/Error/`{message}` 对象三态取消息（tauri 插件 reject 普通对象） |
| `safeStorage` | `lib/utils.ts:31` | localStorage 包装，异常落内存 Map（`lib/utils.ts:20`） |
| `hexToHsl` | `lib/color.ts:1` | 十六进制 → HSL，非法输入回退 `{h:230,s:70,l:55}` |
| `EasingConfig` | `lib/easing-config.ts:3` | 5 组贝塞尔缓动字段接口 |
| `EASING_120HZ` | `lib/easing-config.ts:19` | 120Hz 档缓动表 |
| `getEasingConfig` | `lib/easing-config.ts:27` | `refreshRate >= 120` 取 120Hz 表，否则 60Hz 表（`EASING_60HZ` 定义在 `lib/easing-config.ts:11`，未导出） |
| `LatencyType` | `lib/latency.ts:5` | `'gateway' \| 'external'` |
| `getLatencyLevel` | `lib/latency.ts:7` | 延迟 → 六档等级（<0 → bad） |
| `getLatencyColor` | `lib/latency.ts:17` | 延迟 → `{text,bg,borderBg}` |
| `mergeNetworkQuality` | `lib/latency.ts:25` | 增量推送合并（disabled/busy 特判，`lib/latency.ts:26-33`） |
| `extractGatewayLatency` | `lib/latency.ts:38` | 取网关延迟，回退 `details.gateway` |
| `extractExternalLatency` | `lib/latency.ts:45` | 取外网延迟（均值优先，其次中位数） |
| `resolveQualityDisplay` | `lib/latency.ts:66` | 卡片展示统一解析，`unknown`/`busy` 时按延迟推断等级 |
| `isRenderLoopAlive` | `lib/renderLiveness.ts:36` | rAF 按需短探测判存活（阈值 `RENDER_STALL_THRESHOLD_MS=10000`，`lib/renderLiveness.ts:21`） |
| `createLogEntryVariants` | `lib/animations.ts:3` | 日志条目进出场 variants |
| `getPanelDirection` | `lib/animations.ts:21` | 面板序方向（**安卓无调用方**） |
| `createPanelAppleVariants` | `lib/animations.ts:28` | 面板 Apple 风 variants（**安卓无调用方**） |

### i18n

| 导出 | 位置 | 说明 |
| --- | --- | --- |
| default（`i18n` 实例） | `i18n/index.ts:28` | `LanguageDetector` + `initReactI18next`，资源 `zh`/`en`（`i18n/index.ts:12-15`），`fallbackLng:'zh'`，localStorage key `app-language`（`i18n/index.ts:17-22`）；由 `main.tsx:13` 侧载 |

### components/ui/（Radix + CVA 基础件）

| 导出 | 定义位置 | 导出位置 | 说明 |
| --- | --- | --- | --- |
| `AnimatedCard` | `animated-card.tsx:31` | 同左 | 卡片容器：倾斜（gsap quickTo rotateX/Y）、入场 class、`noHover`/`noAnimation`/`noEnterAnimation`/`enableTilt`/`staggerIndex` |
| `Button` | `button.tsx:43` | `button.tsx:173` | 变体 8 种（`default/destructive/outline/secondary/ghost/link/glass/soft`，`button.tsx:10-19`）、尺寸 6 种（`button.tsx:20-27`）、`asChild`/`isLoading`、鼠标坐标写 CSS 变量 |
| `buttonVariants` | `button.tsx:6` | `button.tsx:173` | CVA 变体表 |
| `Badge` | `badge.tsx:36` | `badge.tsx:42` | 变体 8 种（`badge.tsx:9-18`）、尺寸 3 种（`badge.tsx:19-23`） |
| `badgeVariants` | `badge.tsx:5` | `badge.tsx:42` | CVA 变体表 |
| `CardHeader` | `card.tsx:4` | `card.tsx:48` | `p-5` 头容器 |
| `CardTitle` | `card.tsx:16` | `card.tsx:48` | `h3` 标题 |
| `CardDescription` | `card.tsx:28` | `card.tsx:48` | `p` 描述 |
| `CardContent` | `card.tsx:40` | `card.tsx:48` | `p-5 pt-0` 内容 |
| `Dialog` | `dialog.tsx:7` | `dialog.tsx:97` | Radix Root 别名 |
| `DialogPortal` | `dialog.tsx:9` | **未导出** | Radix Portal 别名 |
| `DialogOverlay` | `dialog.tsx:11` | **未导出** | 遮罩（仅内部用于 `DialogContent`） |
| `DialogContent` | `dialog.tsx:26` | `dialog.tsx:97` | 自定义居中容器（`dialog.tsx:32-33`）+ 关闭钮（`dialog.tsx:43-48`，`showClose` 可关） |
| `DialogHeader` | `dialog.tsx:56` | `dialog.tsx:97` | 头部布局 |
| `DialogTitle` | `dialog.tsx:70` | `dialog.tsx:97` | Radix Title |
| `DialogDescription` | `dialog.tsx:85` | `dialog.tsx:97` | Radix Description |
| `Input` | `input.tsx:10` | `input.tsx:40` | 带 `icon` 与 `error`（错误态补边框宽度，`input.tsx:24-25`） |
| `Label` | `label.tsx:10` | `label.tsx:26` | 支持 `required` 星号（`label.tsx:21`） |
| `Select` | `select.tsx:6` | `select.tsx:100` | Radix Root 别名 |
| `SelectValue` | `select.tsx:8` | `select.tsx:100` | Radix Value 别名 |
| `SelectTrigger` | `select.tsx:10` | `select.tsx:100` | 支持 `icon`（`select.tsx:23-27`） |
| `SelectContent` | `select.tsx:36` | `select.tsx:100` | Portal + popper 定位 |
| `SelectItem` | `select.tsx:66` | `select.tsx:100` | 带选中勾 |
| `SelectSeparator` | `select.tsx:88` | `select.tsx:100` | 分组分隔线 |
| `Separator` | `separator.tsx:5` | `separator.tsx:28` | 水平/垂直 |
| `Switch` | `switch.tsx:5` | `switch.tsx:34` | 尺寸 `default/sm/lg`（`switch.tsx:14-16`） |
| `TooltipProvider` | `tooltip.tsx:5` | `tooltip.tsx:31` | Radix Provider 别名 |
| `Tooltip` | `tooltip.tsx:7` | `tooltip.tsx:31` | Radix Root 别名 |
| `TooltipTrigger` | `tooltip.tsx:9` | `tooltip.tsx:31` | Radix Trigger 别名 |
| `TooltipPortal` | `tooltip.tsx:11` | **未导出** | Radix Portal 别名 |
| `TooltipContent` | `tooltip.tsx:13` | `tooltip.tsx:31` | 内容容器 + 进出场动画 |

`animated-card.tsx` 内部私有：`AnimatedCardConfig`(6)、`AnimatedCardProps`(14)、`REST_SHADOW`(23)、模块级 `reducedMotionQuery`(27)。

### components/layout/、components/mobile/、components/tablet/

| 导出 | 位置 | 用途 |
| --- | --- | --- |
| `MobileTab` | `BottomNav.tsx:9` | `'dashboard' \| 'account' \| 'selfservice' \| 'quality' \| 'monitor' \| 'more'` |
| `BottomNav` | `BottomNav.tsx:23` | 移动底栏（五列 grid，`BottomNav.tsx:42`）；第 4 位动态：质量开启插 `QUALITY_TAB`，关闭插 `MONITOR_TAB`（`BottomNav.tsx:29-30`）；半透明 + backdrop-blur、`env(safe-area-inset-bottom)`（`BottomNav.tsx:35-39`） |
| `DockNav` | `DockNav.tsx:437` | 平板悬浮 Dock（磁吸放大、指示条、注销/登录按钮带适配器菜单），`memo` 包裹。内部私有：`DockItem`(50)、`IS_TOUCH`(161)、`AdapterMenu`(172)、`ActionButtonWithMenu`(281)、`MAGNETIC_RANGE/MAX_SCALE/MAX_LIFT`(46-48)、`ICON_MAP`(34) |
| `RightPanel` | `RightPanel.tsx:83` | 平板横屏右侧栏：运行日志卡 + 网络适配器卡。内部私有：`LOG_ICONS`(26)、`LOG_COLORS`(33)、`LOG_BG_COLORS`(40)、`LOG_BAR_COLORS`(47)、`RIGHT_PANEL_ANIM_THRESHOLD=50`(57)、`RIGHT_PANEL_ANIM_KEEP_COUNT=30`(58)、`getAdapterInfo`(60) |
| `TitleBar` | `TitleBar.tsx:52` | 平板顶栏：应用名/版本徽标、浅色/语言/通知/主题/赞助/关于按钮；`showWindowControls=false` 时隐藏最小化/最大化/关闭并禁用拖动与双击最大化（`TitleBar.tsx:73-90`）。内部私有图标组件 `MinimizeIcon`(26)/`MaximizeIcon`(32)/`RestoreIcon`(38)/`CloseIcon`(45) |
| `MobileDashboard` | `MobileDashboard.tsx:121` | 手机总览：复用 `DashboardPanel`，注入两张移动专属卡（`MobileDashboard.tsx:129-132`），排除 `quickActions`（`MobileDashboard.tsx:32`）。内部私有：`MobileStatusCard`(35)、`MobileMonitorCard`(79)、`LAMP_COLOR`(21)、`noopAsync`(29) |
| `MobileMore` | `MobileMore.tsx:32` | 「更多」聚合页：chips 切换 monitor/speedtest/log/settings 四个子页；质量关闭时移除 monitor 子页并派生降级到 speedtest（`MobileMore.tsx:36-39`）。私有 `MoreTab`(18)、`TABS`(20) |
| `MobileQuickActions` | `MobileQuickActions.tsx:11` | 首页底部固定快捷登录/注销条；快捷登录先 `bindToWifi` 再 `doLogin`（`MobileQuickActions.tsx:25-26`） |
| `TabletShell` | `TabletShell.tsx:380` | 平板外壳；`TabletShellInner`(73) 渲染 TitleBar（`TabletShell.tsx:238`）/ StatusBar（255）/ 主区（262-292）/ 横屏 RightPanel（294-299）/ 侧边看板娘（303-321）/ DockNav（323）/ 弹窗组。私有：`PANEL_TITLES`(53)、`PANEL_CONTAINER_STYLE`(65)、`TOPBAR_ZOOM=1.3`(70)、`CONTENT_ZOOM=0.9`(71) |

### shared/（通用组件与常量）

| 导出 | 位置 | 用途 |
| --- | --- | --- |
| `AnimatedNumber` | `AnimatedNumber.tsx:14` | gsap quickTo 数字滚动 + 缩放弹跳（economy 档禁用弹跳，`AnimatedNumber.tsx:83`） |
| `ConfirmDialog` | `ConfirmDialog.tsx:20` | 二次确认弹窗，确认期间禁用按钮防双击（`ConfirmDialog.tsx:22-23`） |
| `ErrorBoundary` | `ErrorBoundary.tsx:15` | 类组件错误边界，渲染 `MascotFigure variant="alert"` + 重载按钮（`ErrorBoundary.tsx:38-46`） |
| `FluidBackground` | `FluidBackground.tsx:1` | 纯静态背景层（`var(--surface-main)`）。**安卓无调用方** |
| `MascotVariant` | `MascotFigure.tsx:5` | 7 种看板娘变体 |
| `MascotFigure` | `MascotFigure.tsx:7` | `/girl/mascot-<variant>.webp` 统一出口，尺寸 `sm/md/lg` |
| `SIDE_MASCOT_POOL` | `dailyMascot.ts:5` | 14 张竖版立绘池 |
| `pickDailyMascot` | `dailyMascot.ts:11` | 按 UTC 天数 + offset 取图 |
| `useDailyMascots` | `dailyMascot.ts:18` | 多 offset 每日轮换，60s 轮询跨天切换 |
| `RefreshButton` | `RefreshButton.tsx:13` | 刷新按钮（旋转/回弹 shake/可选对勾） |
| `getRefreshIconClass` | `RefreshButton.tsx:71` | 仅图标版刷新 class 生成器（供卡片内联使用） |
| `SegmentTabs` | `SegmentTabs.tsx:19` | 分段标签（`useId` 前缀避免跨实例 layoutId 冲突，`SegmentTabs.tsx:20-22`） |
| `TabContent` | `SegmentTabs.tsx:60` | `AnimatePresence mode="wait"` 包装 |
| `SponsorCard` | `SponsorCard.tsx:13` | 移动端赞助弹窗（Radix Dialog，替代桌面锚定浮层） |
| `ToastContainer` | `ToastContainer.tsx:30` | 左上角 toast 栈（`TOAST_MASCOTS`(16) / `TOAST_STYLES`(23)，economy 档换轻量转场，`ToastContainer.tsx:33-36`） |
| `UpdateAvailableDialog` | `UpdateAvailableDialog.tsx:18` | 新版本提醒弹窗，`onGoUpdate` 跳关于页；开合由 `useQualityStore.updatePromptOpen` 驱动 |
| `UpdateAvailableData` | `types.ts:1` | 新版本事件载荷 |
| `UpdateInfo` | `types.ts:7` | 检查更新结果 |
| `DownloadProgress` | `types.ts:15` | 下载进度事件 |
| `MirrorSource` | `types.ts:22` | 镜像源项 |
| `MAX_LOG_ENTRIES` | `ui-constants.ts:1` | 300，日志条数上限 |
| `APP_VERSION` | `ui-constants.ts:2` | `'2.3.6'`（与 `android/frontend/package.json` 的 `version` 同步处） |
| `APP_NAME` | `ui-constants.ts:3` | `'校园网登录助手'` |
| `PASSWORD_MASK` | `ui-constants.ts:4` | `'***'`，后端掩码占位 |
| `NAV_ITEMS` | `ui-constants.ts:7` | **8 项且不含 `network`**：dashboard(8)/account(9)/selfservice(10)/monitor(11)/quality(12)/speedtest(13)/settings(14)/log(15)；桌面版同数组含 network 项，安卓此处被裁剪 |
| `StatusState` | `ui-types.ts:1` | `'loading' \| 'online' \| 'offline' \| 'error'` |
| `PanelName` | `ui-types.ts:2` | **仍含 `'network'`**（类型层未裁剪，运行期无入口） |
| `ThemeName` | `ui-types.ts:3` | 7 种主题 |
| `LogType` | `ui-types.ts:4` | 4 种日志级别 |
| `GpuTier` | `ui-types.ts:5` | 桌面 GPU 分级（安卓恒 `unknown`/未使用） |
| `GpuInfo` | `ui-types.ts:7` | GPU 信息（安卓 `getGpuInfo` reject，store 里恒 null） |
| `LogEntry` | `ui-types.ts:16` | 日志条目（含重复折叠 `count`，`ui-types.ts:22`） |
| `ToastMessage` | `ui-types.ts:25` | toast（含 `mascot`、`action`） |
| `AdapterDisabledWarningData` | `ui-types.ts:39` | 网卡停用告警（安卓事件为 no-op） |
| `AutoExitCountdownData` | `ui-types.ts:44` | 自动退出倒计时（安卓事件为 no-op） |
| `SaveConfigResult` | `ui-types.ts:49` | 保存配置返回 |

`shared/index.ts` 只导出 `LogPanel`(1)、`ConfirmDialog`(2)、`AnimatedNumber`(3)、`RefreshButton` 与 `getRefreshIconClass`(4)、`SegmentTabs` 与 `TabContent`(5)、`ToastContainer`(6)、`FluidBackground`(7)、`ErrorBoundary`(8)，并 `export *` 三个类型/常量文件（10-12）。**`MascotFigure`、`SponsorCard`、`UpdateAvailableDialog`、`dailyMascot` 不在 barrel 内**，调用方按文件路径导入（如 `App.tsx:22` 的 `@/shared/SponsorCard`），`main.tsx:8` 也刻意按路径导入 `ErrorBoundary` 以避免经 barrel 静态引入懒加载面板。

### face/（安卓独有：应用内 2D 人脸验证）

| 导出 | 位置 | 用途 |
| --- | --- | --- |
| `FaceDialogMode` | `faceVerifyStore.ts:14` | `'enroll' \| 'verify'` |
| `FaceDialogResult` | `faceVerifyStore.ts:15` | `{ok, reason?}`，失败 reason 传给验证门翻译 |
| `useFaceDialogStore` | `faceVerifyStore.ts:25` | 命令式弹窗桥：`open`(18)/`mode`(19)/`resolve`(20)/`openFaceDialog`(21，返回 Promise；并发触发按取消处理，`faceVerifyStore.ts:30-33`)/`closeFaceDialog`(22) |
| `shouldUseFaceFallback` | `faceVerifyStore.ts:44` | `config.allow2dFaceVerify === true && hasTemplate()` |
| `FaceChallenge` | `faceService.ts:35` | `'blink' \| 'turn'` |
| `FaceFailReason` | `faceService.ts:37` | `'timeout' \| 'challenge' \| 'mismatch' \| 'camera'` |
| `FaceVerifyResult` | `faceService.ts:38` | 判别联合结果 |
| `openCamera` | `faceService.ts:80` | getUserMedia 前置摄像头 640×480 |
| `closeCamera` | `faceService.ts:89` | 停全部 track 并清 `srcObject` |
| `loadTemplate` | `faceService.ts:131` | 读 `campus-2d-face-template` 描述子 |
| `hasTemplate` | `faceService.ts:142` | 是否已录入 |
| `clearTemplate` | `faceService.ts:146` | 清空模板（开关关闭时调用） |
| `enrollFace` | `faceService.ts:155` | 质量门控采 8 帧取均值存模板（`ENROLL_FRAMES=8` / `ENROLL_TIMEOUT_MS=20000` / `DETECT_SCORE_MIN=0.85`，`faceService.ts:26/28/33`） |
| `verifyFace` | `faceService.ts:184` | 随机动作挑战（眨眼/转头）→ 同人比对 3 次机会（`MATCH_THRESHOLD=0.55`，`faceService.ts:23`） |
| `FaceCaptureDialog` | `FaceCaptureDialog.tsx:28` | 单例弹窗（`Dialog` + `CaptureFlow`）；`CaptureFlow` 定义在 `FaceCaptureDialog.tsx:48`，必须放 `DialogContent` 内以拿到 video ref（`FaceCaptureDialog.tsx:5-8` 注释） |

`faceService.ts` 内部私有：`getHuman`(44，单例懒加载 + warmup)、`detectOnce`(104，含 human mesh 索引判眨眼)、`saveTemplate`(150)、各常量 `TEMPLATE_KEY`(20)/`POLL_INTERVAL_MS`(31)/`CHALLENGE_TIMEOUT_MS`(29)。

### index.css（无导出）

`@tailwind base/components/utilities`（`index.css:1-3`）；`@layer base`（5）内含浅色 `--surface-top/--surface-main/--surface-side`（`index.css:34-36`）、深色同名变量（68-70）、6 套主题变量（75-84）、`.force-light-dialog`（90，桌面 AboutDialog 复用）；`@layer components`（170）内含卡片/玻璃 Dock（`glass-dock` 210）/圆角贴面类（`app-outer-square` 284、`surface-top-square` 295、`surface-side-square` 300、`surface-main-square` 304）/按钮类（`btn-physical` 346、`btn-press` 382、`soft-btn` 420）/卡片交互（`animated-card-interactive` 473）/日志类（`log-entry-flash` 328、`log-entry-hover` 522）/`panel-content`（547）/`card-enter`（566）/对话框进出场（603-617）/状态点动画（`status-enter` 682、`status-offline-shake` 688）/数字与标签弹入（`label-bounce-in` 697、`num-scale-in` 705）；`@layer utilities`（710）内含 `.anim-idle .animate-pulse` 停动画（718）、`.scrollbar-none`（723）、信号条动画（`signal-bar-enter` 746、`signal-glow-idle` 751、`signal-glow-active` 765）、列表项交互（`list-item-interactive` 770），并有一段 `prefers-reduced-motion` 降级块（`index.css:799-830`，逐类置 `animation: none`）。

## 结构体与字段（前端即 props 与 state 类型）

### 各 store 的 state 字段

`ConfigStore`（声明 `useConfigStore.ts:27-51`）：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `config` | `Config` | 全量配置，初值 `DEFAULT_CONFIG`（`useConfigStore.ts:54`） |
| `configLoaded` | `boolean` | `getInitData` 成功或降级完成（`useInitialDataLoad.ts:107` / `useInitialDataLoad.ts:113`） |
| `passwordSaved` | `boolean` | 登录密码已落盘（后端掩码态） |
| `selfPasswordSaved` | `boolean` | 自助服务密码已落盘（独立布尔，避免读 `config.selfPassword` 的三态竞态，`useConfigStore.ts:34-36`） |
| `accounts` | `string[]` | 账号名列表 |
| `activeAccount` | `string` | 当前账号 |
| `language` | `string` | 初值 `safeStorage.get('app-language') \|\| 'zh'`（`useConfigStore.ts:60`） |
| `api` | `typeof tauriApiWithRetry` | store 内直通 IPC，供 `useXxx` 钩子复用 |

`AuthStore`（`useAuthStore.ts:91-101`）：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `isLoggingIn` / `isLoggingOut` | `boolean` | 登录/注销进行中（按钮禁用与并发互斥） |
| `status` | `{text: string; state: StatusState}` | 顶栏状态点文案与状态 |
| `bgStatus` | `BackgroundStatus` | 后台巡检状态（初值 `useAuthStore.ts:107`：`isRunning/checkCount/serverAvailable/online/adapterStatuses/currentSsid`） |
| `doLogin(adapterName?)` | `=> Promise<boolean>` | 登录，返回是否成功 |
| `doLogout(adapterName?)` | `=> Promise<void>` | 注销 |
| `checkOnline(cfg?, adps?)` | `=> Promise<void>` | 校园网 + Portal 状态检测（有模块级锁 `_checkOnlineLockFlag`） |
| `setStatus` / `setBgStatus` | setter | `setBgStatus` 支持函数式更新（`useAuthStore.ts:255`） |

`AdapterStore`（`useAdapterStore.ts:37-46`）：`adapters: Adapter[]`、`disabledAdapters: DisabledAdapter[]`、`adapterDetails: AdapterDetail[]`、`isRefreshingAdapters: boolean`、`activePanel: PanelName`（初值 `'dashboard'`，`useAdapterStore.ts:53`）、`refreshAdapters(): Promise<void>`、`setAdapters(a)`、`setActivePanel(p)`。

`QualityStore`（`useQualityStore.ts:23-44`）：`networkQuality: NetworkQuality | null`、`dnsDohStatus: DnsDohStatus | null`、`dnsChecking: boolean`、`isRefreshingQuality: boolean`、`updateAvailable: boolean`、`latestVersion: string`、`releaseNotes: string`、`updatePromptOpen: boolean`、`gpuInfo: GpuInfo | null`、`refreshRate: number`（初值 0，`useQualityStore.ts:56`）。

`LogToastStore`（`useLogToastStore.ts:15-26`）：`logs: LogEntry[]`、`toasts: ToastMessage[]`、`addLog(message, type?)`、`addToast(title, type?, description?, duration?, mascot?)`、`addToastWithAction(toast)`、`removeToast(id)`、`removeToastsByPrefix(prefix)`、`setLogs(logs)`、`cleanupToasts()`。

`ThemeStore`（`useThemeStore.ts:9-17`）：`themeName: ThemeName`（`'default'`）、`isLightMode: boolean`（初值由 `campus-light-mode` 推导，`useThemeStore.ts:21`）、`customThemeColor: string`（`'#6366f1'`）。

`FaceDialogState`（`faceVerifyStore.ts:17-23`）：`open: boolean`、`mode: FaceDialogMode`、`resolve: ((r) => void) | null`、`openFaceDialog(mode)`、`closeFaceDialog(r)`。

### 组件 props

| 类型 | 位置 | 字段 |
| --- | --- | --- |
| `AnimatedCardProps`（未导出） | `animated-card.tsx:14` | `animationConfig?`、`noHover?`、`noAnimation?`、`noEnterAnimation?`、`enableTilt?`、`staggerIndex?`（继承 `HTMLAttributes<HTMLDivElement>`） |
| `AnimatedCardConfig`（未导出） | `animated-card.tsx:6` | `glowIntensity?`、`hoverScale?`、`stiffness?`、`damping?`、`mass?` |
| `ButtonProps` | `button.tsx:36` | `asChild?`、`isLoading?` + HTML button 属性 + variant/size |
| `BadgeProps` | `badge.tsx:32` | HTML div 属性 + variant/size |
| `InputProps` | `input.tsx:4` | `icon?: ReactNode`、`error?: string` + HTML input 属性 |
| `TitleBarProps`（未导出） | `TitleBar.tsx:11` | `notificationEnabled`、`onToggleNotification`、`onShowTheme`、`onShowAbout`、`onShowSponsor`、`onToggleLightMode`、`onMinimize`、`onToggleMaximize`、`onClose`、`isMaximized`、`showWindowControls?`（默认 true） |
| `DockNavProps`（未导出） | `DockNav.tsx:432` | `onPanelChange(panel)`、`outerRef?` |
| `AdapterMenuProps`（未导出） | `DockNav.tsx:163` | `adapters`、`selectedAdapter?`、`onSelect`、`actionLabel`、`autoDetectName?` |
| `RightPanelProps`（未导出） | `RightPanel.tsx:20` | `logs: LogEntry[]`、`onClearLogs?`、`outerRef?` |
| `MobileMoreProps`（未导出） | `MobileMore.tsx:27` | `onShowOnboarding?` |
| `SegmentTabs` 的 `TabItem`（未导出） | `SegmentTabs.tsx:5` | `key`、`label`、`icon`、`color`、`bg` |
| `ToastContainerProps`（未导出） | `ToastContainer.tsx:10` | `toasts`、`onRemove` |
| `ConfirmDialogProps`（未导出） | `ConfirmDialog.tsx:12` | `open`、`title`、`message`、`onConfirm`、`onCancel` |
| `FaceCaptureDialog` 的 `CaptureFlow` props | `FaceCaptureDialog.tsx:48` | `mode`、`onClose(r)` |
| `MascotFigure` props | `MascotFigure.tsx:8` | `variant`、`size?`、`className?` |

### 领域类型

`Config`（`settings/types.ts:3-52`，**43 个字段**；口径：接口本体（第 4-51 行）的字段总数，含 `configVersion` 标记字段，不含同文件另两个接口 `AutoLaunchResult` / `InitData`；安卓侧隐藏项见表内加粗标注）：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `user` / `password` | `string` | 登录账号/密码（密码后端存掩码） |
| `selfPassword` | `string` | 自助服务密码（DPAPI/Keystore 落盘，前端见掩码） |
| `selfHelloEnabled` | `boolean` | 生物验证总开关（默认 true） |
| `selfReverifyEachAction` | `boolean` | 自助面板每次操作都验证（默认 false） |
| `allow2dFaceVerify` | `boolean` | 2D 人脸回退开关（默认 false） |
| `operator` | `string` | 运营商后缀（`''`/`@telecom`/`@unicom`/`@cmcc`） |
| `adapter1` / `adapter2` / `dualAdapter` | `string`/`string`/`boolean` | 主/副适配器（安卓无枚举，恒 `自动检测` + 空；`DockNav` 仍按此解析作用域） |
| `autoLoginOnStart` | `boolean` | 启动自动登录 |
| `autoExitAfterLogin` | `boolean` | 登录后退（**安卓 UI 隐藏**） |
| `minimizeToTray` / `hiddenStart` | `boolean` | 托盘/静默启动（**安卓 UI 隐藏**） |
| `autoLaunch` | `boolean` | 开机自启 |
| `enableBackgroundCheck` | `boolean` | 后台巡检开关 |
| `backgroundCheckInterval` | `number` | 稳态巡检间隔 ms（默认 60000，`settings/constants.ts:24`） |
| `backgroundCheckIdleInterval` | `number` | 闲时巡检间隔 ms（默认 300000，`settings/constants.ts:26`） |
| `autoLoginOnPreparation` | `boolean` | 「可登录即自动登录」 |
| `autoExitOnOnline` | `boolean` | 上线后自动退出（**安卓 UI 隐藏**） |
| `themeMode` | `'light' \| 'dark' \| 'system'` | 主题模式 |
| `enableNotification` | `boolean` | 通知总开关 |
| `activeAccount` | `string` | 当前账号名 |
| `enableLatencyTest` / `latencyTestInterval` | `boolean`/`number` | 定时延迟测试与间隔 |
| `customThemeColor` | `string` | 自定义主题色 |
| `defaultPanel` | `PanelName \| ''` | 默认面板（**安卓 UI 隐藏**） |
| `enableNetworkQuality` | `boolean` | 质量检测总开关（默认 true；前端按 `!== false` 判定，桌面/安卓后端默认可能为关） |
| `skipTtfbInLatency` / `skipContentInLatency` | `boolean` | 延迟计算跳过项 |
| `portalUrl` | `string` | Portal 地址（默认 `http://10.1.99.100`） |
| `fixedGateway` | `string` | 固定网关（桌面 DNS 优化用） |
| `requiredNetworkName` | `string` | 校园网 SSID（默认 `i-wxxy`） |
| `enableNetworkNameCheck` | `boolean` | 校园网名校验 |
| `campusGateway` | `string` | 校园网网关（默认 `10.2.127.254`） |
| `updateSource` | `'mirror' \| 'github'` | 更新渠道优先级 |
| `campusExitOnFail` | `boolean` | 非校园网时退出（**安卓 UI 隐藏**） |
| `campusCheckStartMinutes` / `campusCheckEndMinutes` | `number` | 检测时间窗（分钟制，460=07:40，0=00:00） |
| `maxDisconnectReconnect` | `number` | 断连重连上限 |
| `autoLoginCooldownSecs` | `number` | 自动登录冷却 |
| `logRetentionDays` | `number` | 日志保留天数 |
| `configVersion` | `number` | 配置 schema 版本（当前 4） |

`InitData`（`settings/types.ts:59-72`）：`config: Partial<Config>`、`version: string`、`adapters: Adapter[]`、`adapterDetails: AdapterDetail[]`、`disabledAdapters: DisabledAdapter[]`、`accounts: string[]`、`activeAccount: string`、`backgroundStatus: BackgroundStatus`、`isAutoStart: boolean`、`autoLaunch: boolean`、`notificationEnabled: boolean`、`gpuInfo?: GpuInfo`、`refreshRate?: number`（安卓仅消费 `config`/`backgroundStatus`/`accounts`/`activeAccount`/`refreshRate`）。

`AuthStore` 相关类型：`PortalStatusResult`（`auth/types.ts:1`：`online`、`message?`、`reachable?`、`loginAvailable?`）、`CommandResult`（`auth/types.ts:8`：`success`、`message?`、`data?`）、`LoginResult extends CommandResult`（`auth/types.ts:14`）。

`BackgroundStatus`（`monitor/types.ts:15-36`）：`isRunning`、`checkCount`、`serverAvailable`、`online`、`message?`、`adapterStatuses?: AdapterOnlineStatus[]`、`currentSsid: string | null`、`onCampusNetwork?`、`enableNetworkNameCheck?`、`requiredNetworkName?`、`campusWifi?`、`campusWired?`（`ConnectionCampusStatus`，私有接口 `monitor/types.ts:9`：`onCampus`/`name`/`message`）、`a1CampusMessage?`、`a2CampusMessage?`、`a1OnCampus?`、`a2OnCampus?`、`loginPreparationMode?`、`interval?`、`enabled?`。

`BackgroundCheckEventData = BackgroundStatus & {...}`（`monitor/types.ts:38-49`）：追加 `timestamp?`、`checkCount?`、`secondaryOnline?`、`secondaryMessage?`、`message?`、`online?`、`adapter1Name?`、`adapter2Name?`、`loginAvailable?`、`serverAvailable?`。

`AdapterOnlineStatus`（`monitor/types.ts:1`）：`name`、`ip`、`wireless`、`online`、`message`。

`NetworkQuality`（`monitor/types.ts:69-78`）：`gatewayLatency`、`externalLatency`、`averageExternalLatency`、`gateway`、`quality`（9 值含 `disabled`/`busy`）、`timestamp`、`details?`（`NetworkQualityDetail` 私有 `monitor/types.ts:51`：`target`/`latency`/`type` + `dnsLatency?`/`tcpLatency?`/`tlsLatency?`/`udpLatency?`/`networkLatency?`/`ttfbLatency?`/`contentLatency?`）、`metrics?`（私有 `NetworkQualityMetrics` `monitor/types.ts:64`：`totalElapsed`/`tests`）。

`AutoLoginEventData`（`monitor/types.ts:80`）：`success`、`message`、`skipped?`。

`Adapter`（`network/types.ts:3`）：`name`、`ip`、`wireless`、`guid?`、`mac`、`ifIndex`、`status: AdapterStatus`、`linkSpeed?`；`AdapterStatus`（`network/types.ts:1`）= `'disabled'|'disconnected'|'enabledNoIp'|'connected'`；`DisabledAdapter`（15）、`AdapterDetail`（21，追加 `subnetMask`/`gateway`/`dhcpServer`）、`DnsServerInfo`（35，私有）、`DnsAdapterInfo`（42：`name`/`dnsSource`/`dnsServers`/`profileDnsServers`/`adapterDnsOverridesProfile`）、`DnsDohStatus`（50）、`DhcpRenewResult`（57）、`DhcpReleaseRenewResult`（62，含 `skipped`/`reason`）、`DnsSetupResult`（67）、`EnableAdapterResult`（76）——后五者安卓侧仅剩类型定义（命令已 reject）。

`SocInfo`（`hooks/tauriApi.ts:27`）：`socModel`、`deviceModel`、`tier`（0 未知/1 入门/2 中高/3 旗舰）；`BatteryOptimizationInfo`（`hooks/tauriApi.ts:34`）：`ignoring`、`brand`、`hasVendorTarget`；`VendorSettingsResult`（`hooks/tauriApi.ts:40`）：`path`（`'vendor'|'vendor_action'|'app_details'|'settings'|'none'`）、`target`、`tried: string[]`。

`DeviceProfile`（`hooks/useDeviceProfile.ts:12`）：`tier: DeviceTier`（`'high'|'mid'|'low'`）、`gpuVendor`（`'adreno'|'mali'|'immortalis'|'power vr'|'other'`）、`idleFps`、`activeFps`。

`AnimationProfile`（未导出，`hooks/useAnimationProfile.ts:9-27`）：`tier`（`'high'|'standard'|'economy'`）、`willChangeOrbs`、`magneticOffset`、`magneticDuration`、`numberDuration`、`springStiffness`、`springDamping`、`powerPreference`、`prefersCssAnimation`、`enableGpuCompositing`、`enablePageSlide`、`enableTilt`、`enableBackdropBlur`、`startupBoost`、`startupStaggerDelay`、`easing: EasingConfig`、`refreshRate`。

`EasingConfig`（`lib/easing-config.ts:3`）：`enter`、`exit`、`smooth`、`snappy`、`overshoot`，均为 4 元贝塞尔数组。

`UpdateInfo`（`shared/types.ts:7`）：`hasUpdate`、`latestVersion`、`releaseNotes`、`assets: {name,url,size}[]`、`sha256Checksum?`；`DownloadProgress`（15）：`downloaded`、`total`、`speed`、`percent`；`MirrorSource`（22）：`name`、`url`、`description`；`UpdateAvailableData`（1）：`hasUpdate`、`latestVersion`、`releaseNotes?`。

`LogEntry` / `ToastMessage`：见上文 `shared/ui-types.ts` 条目（`ToastMessage.action = { label, onClick }`，`shared/ui-types.ts:33-36`）。

## Data Flow

### 启动编排顺序（真实调用点）

1. `main.tsx` 模块求值：gsap 全局配置（`main.tsx:17-19`）→ reduced-motion 降级（`main.tsx:21-25`）→ `initTheme()` 同步写 DOM class（`main.tsx:43`）→ `setupCrashRecovery()` 注册监听与定时器（`main.tsx:113`）→ `ReactDOM.createRoot(...).render`（`main.tsx:119`）。
2. `App`（`App.tsx:296`）→ `useFormFactor()`（`App.tsx:297`）→ `AppInner`（`App.tsx:302`）或 `TabletShell`（`App.tsx:301`）。
3. 双外壳第一句都是 `useAppInit()`（`App.tsx:48` / `TabletShell.tsx:74`），其内部固定顺序（`useAppInit.ts:7-10`）：
   - `useEventListeners()`（`useEventListeners.ts:18`）：`api = useConfigStore.getState().api`（`useEventListeners.ts:31`）→ 注册 `getCurrentWindow().onCloseRequested`（`useEventListeners.ts:50`，安卓无窗口关闭语义）→ 依次订阅 `onBackgroundCheckResult`(77) / `onAutoLoginResult`(199) / `onAdaptersChanged`(215，no-op) / `onAdapterDetailsChanged`(242，no-op) / `onDisabledAdaptersChanged`(248，no-op) / `onAdapterDisabledWarning`(254，no-op) / `onLoginLog`(263) / `onAutoExitCountdown`(271，no-op) / `onAutoExitCancelled`(292，no-op) / `onCampusExitCountdown`(299，no-op) / `onCampusExitExitCancelled`(320，no-op) / `onNetworkQualityResult`(327) / `onUpdateAvailable`(335) / `onConfigChanged`(350)。
   - `useInitialDataLoad()`（`useInitialDataLoad.ts:16`）：`api.getInitData()`（`useInitialDataLoad.ts:31`，→ `tauriApi.ts:271` `invoke('get_init_data')`）→ 合并 `DEFAULT_CONFIG`（`useInitialDataLoad.ts:34`）→ 由 `PASSWORD_MASK` 同步 `passwordSaved`/`selfPasswordSaved`（`useInitialDataLoad.ts:35-42`）→ `useConfigStore.setState({config})`（43）→ `useThemeStore.initTheme(cfg)`（45）→ 恢复上次面板（47-56，质量关闭时跳过 quality）→ 从 `initData.backgroundStatus` 灌 `bgStatus` 并在 `online === true` 时直接置在线（`useInitialDataLoad.ts:62-84`）→ 灌 `accounts`/`activeAccount`（86-90）→ `useAuthStore.getState().checkOnline(cfg, adps)`（92）→ 灌 `refreshRate`（97-99）→ 置 `configLoaded: true`（107）。失败降级：`config: DEFAULT_CONFIG` + `configLoaded: true` + 错误 toast（`useInitialDataLoad.ts:108-114`）。
   - `useHeartbeat()`（`useHeartbeat.ts:5`）：立即一次 + 每 5s `api.renderHeartbeat()`（安卓为 `desktopOnly` reject，见 Known Issues）。
   - `useGlobalShortcut()`（`useGlobalShortcut.ts:4`）：注册 keydown。

`checkOnline` 的内部数据流（`useAuthStore.ts:201-252`）：按 `enableNetworkNameCheck` 先 `checkCampusStatus`（`useAuthStore.ts:210` → `detectCampusNetwork` 31 → `tauriApi.ts:175`），不在校园网则写 `bgStatus` 补丁（`buildCampusBgStatusPatch` 40，调用点 219/228）并把 `status` 置 offline 后 return（221-222）；随后 `queryPortalStatus('')`（234 → `useAuthStore.ts:71` → `tauriApi.ts:174`）→ 用 `portal.online` 设 `status.state`（239-243）；结尾 `finally` 释放锁（250）。

### 状态点与后台巡检链路

`background-check-result` 事件 → `useEventListeners.ts:77` 回调：1s 节流（81）→ 主/副适配器在线翻转日志（87-134）→ `useAuthStore.setBgStatus`（136-181，逐字段 `??` 合并）→ 仅在 text/state 变化时 `setStatus`（190-193）。

### 登录/注销链路

`MobileQuickActions`（`MobileQuickActions.tsx:11`）→ `api.bindToWifi()`（25 → `tauriApi.ts:173`）→ `doLogin()`（27）→ `useAuthStore.doLogin`（`useAuthStore.ts:109`）→ `saveConfigDirect`（121）→ `withTimeout(api.doLogin(adapterName), 60000, ...)`（129 → `tauriApi.ts:176` `invoke('do_login')`）→ 成功置 online + 日志/toast（131-134）→ 质量联动：节流窗口外才 `checkNetworkQuality`（145-151 → `tauriApi.ts:237`）→ 失败才补一次 `checkOnline`（163-165）。平板 Dock 的登录/注销走 `DockNav.tsx:449-450` 的同一 store 动作，按 `resolveAdapterNames`（`network/adapters.ts:15`）限定菜单候选。

审批/明文链路：`AccountPanel` 绑定卡与 `SelfServicePanel` 通过 `useHelloGate`（`account/selfServiceState.ts:85`）或 `useSelfServiceVerify`（125）→ `tauriApiWithRetry.verifyWindowsIdentity`（93/133 → `tauriApi.ts:180-211`）→ 2D 人脸回退走 `useFaceDialogStore.openFaceDialog('verify')`（`tauriApi.ts:189`）→ `FaceCaptureDialog`（`App.tsx:303` 挂载）→ `faceService.verifyFace`（`faceService.ts:184`）。

### 配置写回链路

组件 → `useConfigStore.updateConfig(partial)`（`useConfigStore.ts:63`）：合并进 `config`（65）→ 标脏 `dirtyFields`（69-72）→ 累积 `saveConfigPending`（74-82）→ 500ms debounce 后 `saveConfigDirect(pending)`（84-93）→ `tauriApiWithRetry.saveConfig`（`useConfigStore.ts:132` → `tauriApi.ts:322` → `tauriApi.ts:168` `invoke('save_config')`）→ 成功后清脏（134-137）；失败累计到 3 次放弃脏标记（150-160）。后端回传经 `onConfigChanged`（`useEventListeners.ts:350`）→ `mergeConfigFromBackend`（`useConfigStore.ts:110`，跳过脏字段）。关窗前 `hasPendingConfig`（183）/`flushPendingConfig`（190）保证不丢（`useEventListeners.ts:50-75`）。

### 主题与 CSS 变量链路

`initTheme`（`main.tsx:27`，首帧防闪）与 `useThemeStore.initTheme`（`useThemeStore.ts:27`，读存储/配置）→ store 订阅副作用（`useThemeStore.ts:53-86`）→ 切 `dark` class / 主题 class / `--primary` 等变量；`App.tsx:258-268` 与 `settings/useSettings.ts:35-41`、`settings/useSettings.ts:58-61` 是两个入口（手机外壳内联、平板走 `useSettings`）。

## Connections

- 面板层与业务 UI：[[android-frontend-panels]]（auth/account/monitor/settings/network 面板、`shared/LogPanel`）。
- 后端命令与事件契约：[[android-backend]]（`invoke` 命令名、`background-check-result`/`network-quality-result`/`config-changed` 等事件载荷）。
- 安卓专属插件与门控：[[android-plugins]]（`plugin-biometric`、`plugin-notification`、`plugin-opener`、`get_soc_info`/电池优化命令）。
- 桌面同构对照：[[desktop-frontend-hooks]]（桌面 store/钩子集合，含安卓没有的 `useGpuCorrection`）、[[desktop-frontend-shared]]（共享组件在桌面的同名实现）、[[desktop-frontend-panels]]（桌面面板与 `DashboardPanel` 全量卡片）、[[desktop-app-lifecycle]]（桌面启动编排、托盘与窗口控制）。
- 被安卓裁剪的桌面能力来源：[[desktop-platform]]（窗口/托盘/单实例）、[[desktop-network-core]]（适配器与 DHCP）、[[desktop-network-dns]]（DNS DoH）、[[desktop-commands]]（命令面全表）、[[desktop-config]]（配置字段与默认值）、[[desktop-monitor]]（质量检测与延迟测试后端）、[[desktop-auth]]、[[desktop-account-selfservice]]、[[desktop-helper-update]]（更新/镜像后端）、[[desktop-infra]]（日志与存储）。

## Known Issues

1. **`useStartupBoost` 是死代码**：`hooks/useStartupBoost.ts:15` 定义后全仓无 import（仅 `hooks/useAnimationProfile.ts:51` 注释提及）。其 `StartupRefs` 字段（`useStartupBoost.ts:5-11`）指向桌面 TitleBar/StatusBar/DockNav/RightPanel，手机外壳根本不渲染这些元素——即便被调用也只会 `gsap.set` 到 null。
2. **`FluidBackground` 是死代码**：`shared/FluidBackground.tsx:1` 仅经 `shared/index.ts:7` 导出，全仓无渲染点；`App.tsx:4` 注释明确「桌面件（…/FluidBackground/…）在手机外壳不再渲染」。
3. **`useAppStore` 兼容壳无人引用**：`hooks/useAppStore.ts:2-3` 只做 re-export，全仓无 import；引用清理后可直接删除。
4. **`lib/animations.ts` 两个函数在安卓无调用方**：`getPanelDirection`（`lib/animations.ts:21`）与 `createPanelAppleVariants`（`lib/animations.ts:28`）；安卓面板过渡用 `App.tsx:92` 与 `TabletShell.tsx:133` 各自内联的轻量 variants（0.18s easeOut）。`PANEL_ORDER`（`lib/animations.ts:19`）还含 `'network'`。
5. **`useQualityStore.gpuInfo` 在安卓永为 null**：唯一 setter `setGpuInfo`（`useQualityStore.ts:94`）全仓无调用；`getGpuInfo` 在安卓是 `desktopOnly` reject（`tauriApi.ts:282`），`useInitialDataLoad.ts:94-96` 注释也说明启动不再请求。因此 `useAnimationProfile` 的 `resolveTier(gpuInfo?.tier, reducedMotion)`（`hooks/useAnimationProfile.ts:88`）在安卓只会落 `'standard'` 或（reduced-motion 时）`'economy'`——设备性能分级实际由另一条链路 `useDeviceProfile` → `useAdaptiveFramePace` 承担（`useAdaptiveFramePace.ts:52`），两条分级互不打通。
6. **`renderHeartbeat` 在安卓是 reject**：`tauriApi.ts:281` `desktopOnly('render_heartbeat')`，而 `useHeartbeat.ts:13/15` 无条件调用（仅吞掉 DEV 日志）。安卓没有后端按心跳丢失重载 WebView 的机制，`lib/renderLiveness.ts` 的探测只剩「配合 `main.tsx:95` 跳过心跳更新」这一处意义。
7. **窗口关闭 flush 在安卓不生效**：`useEventListeners.ts:50` 依赖 `getCurrentWindow().onCloseRequested`，安卓进程被杀时不会有该事件；`hasPendingConfig`/`flushPendingConfig`（`useConfigStore.ts:183/190`）在安卓缺少触发时机（仅 `App.tsx:276` 删除账号等场景间接依赖 debounce），存在「最后一次配置改动未落盘」的窗口。
8. **`useGlobalShortcut` 是桌面遗留**：`useGlobalShortcut.ts:17-21` 的 `Ctrl+Shift+C` → `cancelAutoExit`，而安卓 `cancelAutoExit` 为 `desktopOnly` reject（`tauriApi.ts:260`）、`onAutoExitCountdown` 为 no-op（`tauriApi.ts:261`）。键盘事件在手机上也不会触发。
9. **`verifyWindowsIdentity` 的 catch 吞掉非人脸错误**：`tauriApi.ts:201-206` 只在 `code` 以 `face` 开头或等于 `userCancel` 时重抛；`plugin-biometric` 的 `biometryLockout`/`authenticationFailed` 等码会静默滑落到系统验证链路（`tauriApi.ts:209`），可能造成「一次人脸失败后立刻又弹一次系统验证」的双弹体验。
10. **`TauriApi` 类型仍声明了安卓不可用的方法**：`TauriApi`（`tauriApi.ts:56-133`）保留 `getAdapters`/`dhcpRenewAll`/`setupDnsDoh`/`getGpuInfo`/`minimizeWindow`/`showWindow` 等桌面方法，调用方编译期无法察觉其恒 reject；例如 `useAdapterStore.refreshAdapters`（`useAdapterStore.ts:55`）在安卓每次都会走一遍 reject + `.catch(() => undefined)`（`useAdapterStore.ts:24-26`）的空转，`useNetwork.handleDhcpRenew`（`network/useNetwork.ts:65`）同因。
11. **`NoopListener` 订阅方拿到的清理函数不安全**：`tauriApi.ts:18` 的 `noopListener` 每次调用返回**同一个无操作函数**（闭包常量），`useEventListeners.ts:215-264` 逐个 `unlisteners.push` 后逐个调用没有问题，但如果未来有人把返回值当 key 去重会失效。
12. **`index.css` 仍带桌面专用类**：`index.css:90` 的 `.force-light-dialog`、`index.css:310-326` 的 `.app-maximized` 系列（桌面最大化态）、`index.css:505` 的 `.animate-window-reveal`、`index.css:633-652` 的 titlebar 按钮类在安卓无使用点（`TitleBar` 以 `showWindowControls={false}` 渲染，`TabletShell.tsx:239`）。
13. **`gpuInfo`/`dnsDohStatus`/`isRefreshingAdapters` 等字段在安卓恒为初值**：`useQualityStore.ts:48-49`（`dnsDohStatus: null`、`dnsChecking: false`）只有已废弃的 `network/NetworkPanel.tsx:82/95/112` 会写；`isRefreshingAdapters`（`useAdapterStore.ts:52`）只被桌面遗留的 `refreshAdapters`/`RightPanel.tsx:95` 使用。
14. **手机外壳的 tab 持久化键未随面板裁剪迁移**：`App.tsx:52-55` 只接受 `dashboard/account/selfservice/quality/more`，历史存过 `monitor`（当前底栏动态 tab 之一，`BottomNav.tsx:21`）或 `speedtest` 的值会被丢弃回落 `dashboard`；而底栏点击 `monitor` 时 `handleTabChange` 会把 `'monitor'` 写回存储（`App.tsx:97`），下次启动该值又不在白名单里——一处跨会话不一致。
15. **`main.tsx` 的崩溃恢复计数不持久**：`crashCount` 是模块级变量（`main.tsx:46`），`window.location.reload()` 后归零，因此 `MAX_CRASH_RELOADS = 3`（`main.tsx:47`）实际退化为「单次页面生命周期内最多 3 次」；持续崩溃场景下会形成重载循环（`main.tsx:53`），只有「心跳丢失 > 10s」的 2s 检测（`main.tsx:102-110`）在反复触发。
16. **安卓无任何单元测试**：`tauri-app/frontend/src` 有 13 个 `.test.ts(x)` 与 `test-setup.ts`，`android/frontend` 目录下**不存在** `test-setup.ts` 与任何测试文件（文件树对比），`android/frontend/package.json` 也没有 `test` 脚本；复刻分叉只能靠 `tsc --noEmit` + 真机验证发现。
17. **`vite-env.d.ts` 极简**：`android/frontend/src/vite-env.d.ts:1` 只有 `/// <reference types="vite/client" />`，未声明 `VITE_PLATFORM` 等自定义环境变量类型，`import.meta.env.VITE_PLATFORM` 的取值依赖 `android/frontend/.env`（`VITE_PLATFORM=android`）与各处的 `=== 'android'` 字符串比较（`AccountPanel.tsx:55`、`MonitorPanel.tsx:23`、`SettingsPanel.tsx:31`、`LogPanel.tsx:63`）——写错字符串不会有类型报错。
