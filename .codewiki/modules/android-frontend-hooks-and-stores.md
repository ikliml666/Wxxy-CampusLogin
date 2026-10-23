---
title: android-frontend-hooks-and-stores
type: module
source_files:
  - android/frontend/src/hooks/useQualityStore.ts
  - android/frontend/src/hooks/useReducedMotion.ts
  - android/frontend/src/hooks/useThemeStore.ts
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
  - android/frontend/src/hooks/useFormFactor.ts
  - android/frontend/src/hooks/useEventListeners.ts
  - android/frontend/src/hooks/useGlobalShortcut.ts
  - android/frontend/src/hooks/useGlowAnimation.ts
  - android/frontend/src/hooks/useHeartbeat.ts
  - android/frontend/src/hooks/useInitialDataLoad.ts
  - android/frontend/src/hooks/useLogToastStore.ts
  - android/frontend/src/hooks/usePageIdle.ts
  - android/frontend/src/hooks/usePulseAnimation.ts
tags: [android, frontend, hooks, zustand, tauri-api, state-management, animations]
---

## Overview
本模块是 Android 前端的核心基础设施层，负责管理应用状态、用户交互反馈、动画性能优化以及与 Rust 后端的 IPC 通信。由于 Android 端缺乏桌面端的硬件抽象（如物理网卡、窗口管理），该模块通过 `tauriApi` 实现了特定的降级策略和兼容性适配。

核心职责包括：
1. **状态管理**：基于 Zustand 的多个 Store（`useConfigStore`、`useAuthStore`、`useThemeStore`、`useLogToastStore`、`useQualityStore`、`useAdapterStore` 等），处理本地持久化、后端同步及脏数据冲突解决。
2. **IPC 适配**：封装 Tauri API（`tauriApi.ts`），拦截桌面专属功能，实现安卓特有的生物识别回退和事件监听。
3. **动画与性能**：动态帧率控制（`useAdaptiveFramePace`）、设备性能分级（`useDeviceProfile`）、空闲检测（`usePageIdle`）及 GSAP 动画实例管理（`usePulseAnimation`、`useBreatheAnimation`、`useGlowAnimation`）。
4. **初始化编排**：协调启动流程（`useAppInit`），确保数据加载（`useInitialDataLoad`）、事件订阅（`useEventListeners`）和心跳保活（`useHeartbeat`）的正确顺序。

## Key Components

### `tauriApi.ts`: Tauri API 封装与 IPC 适配
作为 Android 前端与 Rust 后端通信的唯一桥梁。
* **常量与工具函数**
    * `desktopOnly<T>(name)` (行14)：返回 rejected Promise，用于拦截所有桌面专属功能调用（如 `exportConfig`, `getAdapters`, `minimizeWindow`）。
    * `noopListener` (行18)：返回空操作清理函数，用于安卓端不存在的适配器事件订阅。
    * `isRetryableError` (行326)：识别 timeout/network/fetch/connection 错误。
    * `withRetry<T>` (行338)：指数退避重试逻辑，最多 2 次。
* **类型定义**
    * `ConnectionCampusStatus` (行20)、`BatteryOptimizationInfo` (行26)、`VendorSettingsResult` (行32)、`CampusStatusResult` (行38)。
    * `TauriApi` 接口 (行48-141)：声明所有 IPC 方法签名（含桌面专属 reject 及 noop 分配）。
    * `AvoidBadWifiStatus` (行143)、`RestoreWrittenSettingsResult` (行154)。
* **API 实现细节**
    * `createEventListener<T>(eventName)` (行159)：封装 `listen`，处理 cancelled 与异步清理。
    * `const tauriApi: TauriApi` (行190)：原始 API 映射表，桌面专属均调 `desktopOnly` 或 `noopListener`。
    * `verifyWindowsIdentity` (行207)：含人脸回退 `shouldUseFaceFallback()` 逻辑，调 `verify_biometric_identity` 并归一化失败。
    * `openExternal` (行276)：校验 http(s)/长度≤2048/URL 解析后调 `openerOpenUrl`。
    * `setAutoLaunch` (行289)：包成 `{success:true}`。
    * `export const tauriApiWithRetry: TauriApi` (行356)：扩展原对象，仅 `saveConfig` 保留重试 (行361)。

### `useConfigStore.ts`: 配置持久化与脏数据管理
管理全局配置 `Config`，处理本地修改与后端同步的冲突。
* **模块变量**：`saveConfigTimer`、`saveConfigPending`、`saveConfigInFlight`、`dirtyFields`、`DIRTY_FAILURE_LIMIT=3`、`dirtyFailureCounts` (行14-25)。
* **状态接口**：`interface ConfigStore` (行27)：含 `config`、`configLoaded`、`passwordSaved`、`accounts`、`activeAccount`、`language` 及 update/merge/set 方法。
* **状态实例**：`useConfigStore` (行48)：默认 config DEFAULT_CONFIG，configLoaded false，language 取 storage 或 'zh' (行54)。
* **操作方法**：
    * `updateConfig` (行56)：防抖500ms (行77)，标 dirtyFields (行62)，处理 PASSWORD_MASK 合并 (行69-75)，调 `saveConfigDirect`，同步主题色 (行87)。
    * `updateConfigLocal` (行90)：无防抖保存。
    * `mergeConfigFromBackend` (行103)：跳过 dirtyFields，密码 MASK 同步 passwordSaved (行116-118)。
    * `saveConfigDirect` (行128)：全量保存，成功清 dirty；失败若带密码则 passwordSaved false (行148)，连续失败达阈值清 dirty (行152-162)；in-flight 跟踪 (行168-174)。
    * `hasPendingConfig()` (行188)：返 pending||inFlight。
    * `flushPendingConfig()` (行195)：清定时器立即存 pending（去 MASK），返回包含 inFlight 的 Promise (行219-223)。

### `useAuthStore.ts`: 认证流程与在线状态检测
管理登录/注销、在线状态探测及后台状态维护。
* **模块变量**：`_checkOnlineLockFlag` (行16)、`QUALITY_MANUAL_THROTTLE_MS=60_000` (行21)。
* **类型**：`CampusStatus` (行23)、`PortalStatus` (行24)。
* **辅助函数**：`detectCampusNetwork()` (行29) 失败返 null；`buildCampusBgStatusPatch` (行38) 纯函数补丁；`queryPortalStatus(adapterIp)` (行56) 空串 IP 后端回退；`withTimeout<T>` (行69) Promise.race 超时清理；`setStatusStable` (行82) 引用稳定更新 status。
* **状态接口**：`interface AuthStore` (行88)：含 `isLoggingIn`、`isLoggingOut`、`status`、`bgStatus` 及 `doLogin`、`doLogout`、`checkOnline`、`setStatus`、`setBgStatus`。
* **状态实例**：`useAuthStore` (行100)：默认 status `{text:'正在检测...',state:'loading'}` (行103)，bgStatus 默认 (行104)。
* **操作方法**：
    * `doLogin` (行106)：防重入 (行109)；`saveConfigDirect` (行117) 失败仅日志；`withTimeout(api.doLogin(),60000)` (行125)；成功置状态，若启用质量且距 `getLastQualityResultTime()` 超阈值才手动探测 (行141-147)；成功不立即 checkOnline 防闪烁 (行159)，失败才复查；复位锁 (行162)。
    * `doLogout` (行166)：注销后必 checkOnline (行192)。
    * `checkOnline(cfg?)` (行196)：加锁 (行197)，campus 检测启用时调 detect (行204)，非校园网则设状态返 (行206-215)；portal 查空IP (行225)，稳定更新状态；finally 立即释放锁 (行241)。

### `useLogToastStore.ts`: 日志与通知管理
管理日志列表 `logs` 和 Toast 通知队列 `toasts`。
* **模块变量**：`toastTimers = new Map<string, ReturnType<typeof setTimeout>>()` (行5)；`toastIdCounter` (行6)、`logIdCounter` (行7) 自增 ID；`MAX_TOASTS = 4` (行9)；`isDuplicateTitle` (行13) 按 `title` 判重。
* **状态接口**：`interface LogToastStore` (行15-26)：定义状态 `logs: LogEntry[]`、`toasts: ToastMessage[]` 及方法签名。
* **状态实例**：导出 `useLogToastStore = create<LogToastStore>((set) => ({...}))` (行28)，初始 `logs: []`、`toasts: []` (行29-30)。
* **操作方法**：
    * `addLog: (message, type = 'info')` (行32)：取本地时间字符串 (行33)；若与末条日志同 `message`+`type` 则仅更新 `time` 且 `count` 累加 (行37-41)；否则新建 `LogEntry` (行44)，超限 `MAX_LOG_ENTRIES` 时保留最近 `MAX_LOG_ENTRIES-1` 条加新条 (行45-47)。
    * `addToast: (title, type='info', description, duration=4000, mascot)` (行52)：若 `document.visibilityState !== 'visible'` 直接返回 (行56)；同题跳过 (行61)；生成 `newId` (行62)，超限淘汰最旧 `evictedId` (行64-66)；入队 (行67-68)；清理被淘汰定时器 (行71-74)；设自动移除定时器 (行77-81)。
    * `addToastWithAction: (toast: ToastMessage)` (行84)：`effectiveDuration = toast.duration ?? 8000` (行85)；非前台也入队 (行86)；同题跳过 (行90)；淘汰与入队逻辑同上 (行92-96)；定时器用 `toast.id` 管理 (行103-107)。
    * `removeToast: (id)` (行110)：过滤并清定时器 (行111-113)。
    * `removeToastsByPrefix: (prefix)` (行116)：按 `id.startsWith(prefix)` 移除 toasts 与对应定时器 (行117-126)。
    * `setLogs: (logs)` (行128)：超 `MAX_LOG_ENTRIES` 则截断保留末尾。
    * `cleanupToasts: ()` (行130)：清所有定时器并空 toasts。

### `useQualityStore.ts`: 网络质量与版本更新状态
* **模块变量**：`_qualityLockFlag` (行11，刷新互斥锁)，`lastQualityResultTime` (行16，最近质量结果到达时间戳，初始0)。
* **导出函数**：`getLastQualityResultTime(): number` (行18)。
* **状态接口**：`interface QualityStore` (行22) 状态含 `networkQuality: NetworkQuality|null`、`dnsDohStatus: DnsDohStatus|null`、`dnsChecking`、`isRefreshingQuality`、`updateAvailable`、`latestVersion`、`releaseNotes`、`updatePromptOpen`、`refreshRate` 及对应 setters 与 `refreshQuality`。
* **状态实例**：`useQualityStore` (行43) 默认值行44-52 全部置空/假/0。
* **操作方法**：
    * `refreshQuality` (行54)：取 `useConfigStore` 的 `config` (行55)，若锁或 `enableNetworkQuality===false` 则返回 (行56-57)；加锁后置 `isRefreshingQuality:true` (行59)；调 `api.checkNetworkQuality?.()` (行61)，成功则经 `mergeNetworkQuality` 走 `setNetworkQuality` (行64)；catch 仅 DEV 打印 (行66-67)；finally 中 setTimeout 500ms 解锁并复位 (行69-72)。
    * `setNetworkQuality` (行78) 支持函数式入参，若结果非 null 则写 `lastQualityResultTime=Date.now()` (行80)；其余 `setDnsDohStatus` (行83) 等简单 setter 行83-88。

### `useThemeStore.ts`: 主题管理与 DOM 同步
* **状态接口**：`interface ThemeStore` (行9) 含 `themeName`、`isLightMode`、`customThemeColor`、`glassLevel` 与 setters、`initTheme`。
* **状态实例**：`useThemeStore` (行21) 默认：`themeName:'default'`；`isLightMode` 从 `safeStorage.get('campus-light-mode')==='1'` 读 (行23)；`customThemeColor:'#6366f1'` (行24)；`glassLevel` 从 storage 校验 `VALID_GLASS_LEVELS` 否则 `'standard'` (行25-28)。
* **操作方法**：
    * setters：`setThemeName` (行30)、`setIsLightMode` (行31)、`setGlassLevel` (行32)、`setCustomThemeColor` (行56)。
    * `initTheme(cfg)` (行34)：从 storage 或 `cfg.themeMode` 决定浅色并写 storage (行35-52)；`cfg.customThemeColor` 直接设 (行53)。
    * 模块订阅 `useThemeStore.subscribe` (行60)：`glassLevel` 变则改 `data-glass` 并写 storage (行62-65)；`isLightMode` 变则 toggle `dark` class 与 `data-light` (行66-73)；`themeName/customThemeColor/isLightMode` 变则重算主题 class，custom 主题经 `hexToHsl` 用 `style.setProperty` 写 `--primary` 等变量 (行74-97)。

### `useReducedMotion.ts`: 减少动态效果偏好检测
* **常量**：`QUERY = '(prefers-reduced-motion: reduce)'` (行3)。
* **导出函数**：`useReducedMotion(): boolean` (行12)：初始 state 用 `window.matchMedia(QUERY).matches` (行13-15)；useEffect 监听 change，新 WebView 用 `addEventListener` 否则降级 `addListener` (行17-27)。

### `useAdaptiveFramePace.ts`: 动态帧率控制
* **常量**：`ACTIVE_WINDOW_MS=2000` (行16)、`PACE_POLL_MS=250` (行18)。
* **模块变量**：`lastActiveAt`、`idleFps`、`activeFps`、`appliedPace` (行20-23)。
* **函数**：`markInteraction()` (行26) 推活跃窗口并 `applyPace()`；`currentPace()` (行31) 后台返0否则活跃返 `activeFps` 否则 `idleFps`；`applyPace()` (行38) 设 `gsap.ticker.fps(pace>0&&pace<60?pace:60)` (行44)。
* **Hook**：`useAdaptiveFramePace()` (行51)：取 `useDeviceProfile` 赋 `idleFps/activeFps` (行55-56)；监听 passive 事件 mark (行60-61)；setInterval 轮询 `applyPace` (行64)；visibilitychange 触发 mark (行65)；清理恢复60fps (行67-73)。

### `useAnimationProfile.ts`: 动画参数配置
* **类型**：`type AnimationTier` (行6)；`interface AnimationProfile` (行8) 含 tier、各动画参数、`easing`、`refreshRate` 等。
* **常量**：`const HIGH_PROFILE` (行31) 默认值：tier 'high'、magneticOffset 5、numberDuration 600、springStiffness 400、refreshRate 60 等。
* **Hook**：`useAnimationProfile()` (行51) 从 `useQualityStore` 取 `refreshRate` (行52)，useMemo 中 `effectiveRefreshRate = refreshRate>0?refreshRate:120` (行54)，返回 `{...HIGH_PROFILE, easing:getEasingConfig(effectiveRefreshRate), refreshRate:effectiveRefreshRate}` (行55)。

### `useAppInit.ts`: 应用启动编排
* **Hook**：`useAppInit()` (行6) 依次调用 `useEventListeners()`、`useInitialDataLoad()`、`useHeartbeat()`、`useGlobalShortcut()` (行7-10)。

### `useAppStore.ts`: 兼容性 Re-export
* 导出 `useAppInit` (行2)、`hasPendingConfig` 与 `flushPendingConfig` (行3) 来自 useConfigStore。

### `useAsyncLock.ts`: 异步操作互斥锁
* **Hook**：`useAsyncLock<T>(fn, cooldownMs=1500): [boolean, T]` (行3)：用 `lockRef`、`mountedRef` (StrictMode 修复，行12-17)、`fnRef`；`execute` (行18) 加锁 setIsRunning，finally 中 setTimeout 冷却释放 (行22-24)。

### `usePageIdle.ts`: 空闲检测与 Provider
* **常量/变量**：`IDLE_TIMEOUT = 2_000` (行6)、`IDLE_CHECK_INTERVAL = 500` (行8)；`lastActiveRef` (行5)。
* **Hooks**：
    * `usePageIdle()` (行3)：返回 `isIdle`，单 `interval` 轮询判定空闲 (行17-20)；监听 `mousemove/mousedown/keydown/touchstart/scroll` (行26-27)；`visibilitychange` 隐藏即 `setIsIdle(true)` (行29-30)，显示重置 (行32-33)；切换 `document.body.classList` 的 `anim-idle` (行45)。
    * `usePageVisible()` (行51)：返回 `isVisible`，初始 `!document.hidden` (行52)，监听 `visibilitychange` (行54-58)。
    * `useWindowFocused()` (行63)：返回 `isFocused`，初始 `document.hasFocus()` (行64)，监听 `focus/blur` (行66-75)。
* **Context/Provider**：`AnimationActiveContext = createContext<boolean>(true)` (行83)；`AnimationActiveProvider({children})` (行85) 组合三 hook，`useMemo` 计算 `value = isVisible && isFocused && !isIdle` (行90)；`useAnimationActive(): boolean` (行95) `useContext` 消费 (行96)。

### 动画专用 Hooks (`usePulseAnimation`, `useBreatheAnimation`, `useGlowAnimation`)
* **`usePulseAnimation.ts`**：
    * 导入 `useAnimationActive` (行3)。
    * 接口 `PulseOptions { type: 'heartbeat' | 'statusPulse' | 'loadingPulse' }` (行5-7)。
    * 导出 `usePulseAnimation(options)` (行15)：`ref` (行16)；`animActive` (行17)；`timelineRef` (行18)。构建 GSAP `timeline` (行28-50)，存 `timelineRef` (行54)，清理时 `timeline.kill()` (行57)；`animActive` 变化时 `resume()/pause()` (行66-70)。返回 `ref` (行73)。
* **`useBreatheAnimation.ts`**：
    * 接口 `BreatheOptions` (行5) 默认值 minOpacity0.6/max1/duration4/scale1/rotation0。
    * `useBreatheAnimation(options)` (行15) 返 ref；useEffect 建 gsap.set (行34) 与 yoyo repeat tween (行36-45)；第二 effect 依 `useAnimationActive()` (行17,行55-63) resume/pause。
* **`useGlowAnimation.ts`**：
    * 接口 `GlowOptions` (行5) 默认 duration4/maxScale1.15/maxOpacity0.6。
    * `useGlowAnimation(options)` (行11) 返 ref；建 tween (行24-32)；依 `useAnimationActive()` (行13,42-50) 暂停/恢复。

### `useAdapterStore.ts`: 面板导航状态
* **状态接口**：`interface AdapterStore` (行6)：`activePanel: PanelName`、`setActivePanel`。
* **状态实例**：`useAdapterStore` (行11) 默认 `activePanel:'dashboard'` (行12)，`setActivePanel` (行13)。

### `useDeviceProfile.ts`: 设备性能分级
* **类型/常量**：`DeviceTier` (行6)、`DeviceProfile` (行8)；`HIGH_PROFILE` (行17) idleFps30 activeFps60。
* **Hook**：`useDeviceProfile()` (行25) 返 HIGH_PROFILE。

### `useEventListeners.ts`: 全局事件总线订阅
* **Hook**：`useEventListeners()` (行14)：
    * `handleQualityBadAlert` (行25) 质量变坏 toast。
    * `getCurrentWindow().onCloseRequested` (行41) 拦截关窗，flushPendingConfig await 2s 再关 (行46-60)。
    * `flushPendingNow` 可见性/pagehide 冲刷 (行71-80)。
    * 订阅 `onBackgroundCheckResult` (行82)、`onAutoLoginResult` (行117)、`onLoginLog` (行134)、`onAutoExitCountdown` (行142)、`onAutoExitCancelled` (行163)、`onCampusExitCountdown` (行170)、`onCampusExitCancelled` (行191)、`onNetworkQualityResult` (行198)、`onUpdateAvailable` (行206)、`onConfigChanged` (行220)、`onUpdateUnverified` (行244)。
    * 清理 (行257-262)。适配器相关 noop 不注册 (行130-132)。

### `useFormFactor.ts`: 设备形态判定
* **类型/常量**：`FormFactor`/`Orientation` (行9-10)；`TABLET_MIN_EDGE_PX=600` (行13)。
* **函数/Hooks**：`detectFormFactor` (行15)、`detectOrientation` (行19)；`useViewportMeasure<T>` (行24) 响应 resize/orientationchange；`useFormFactor` (行38)、`useOrientation` (行43)。

### `useGlobalShortcut.ts`: 全局快捷键
* **Hook**：`useGlobalShortcut()` (行4)：keydown 忽略可编辑元素 (行10-15)，调 `api.cancelAutoExit?.()` (行19)。

### `useHeartbeat.ts`: 心跳保活
* **Hook**：`useHeartbeat()` (行5)：setInterval 5s (行10)，`document.hidden` 暂停 (行7)，`isRenderLoopAlive()` 为真才调 `api.renderHeartbeat?.()` (行12，安卓端该 API 为 desktopOnly reject 但被 catch)。

### `useInitialDataLoad.ts`: 首次数据加载
* **常量**：`VALID_PANELS` (行15) 从 NAV_ITEMS 取。
* **Hook**：`useInitialDataLoad()` (行17)：async 取 `api.getInitData()` (行31)；合并 DEFAULT_CONFIG，密码 MASK 同步 (行35-39)；`useThemeStore.initTheme` (行42)；恢复 activePanel 跳过 quality 禁用 (行44-53)；bgStatus 补 (行57-78)，在线则设 status (行72-77)；accounts/active (行80-84)；`checkOnline(cfg)` (行86)；refreshRate 写 qualityStore (行91-93)；置 configLoaded (行101)；失败降级 DEFAULT_CONFIG (行107)。StrictMode mountedRef 处理 (行25,111-113)。

## Architecture

```text
┌──────────────────────────────────────────────┐
│ useAppInit: useEventListeners►useInitialDataLoad►useHeartbeat │
│ ┌─────────────────────────────────────────┐  │
│ │ Global Stores (Auth/Config/Quality/...) │  │
│ └───────────────────┬─────────────────────┘  │
│ ┌────────┐ ┌──────────────┐ ┌────────────┐  │
│ │useGlobal│ │AnimationActive│ │ThemeStore  │  │
│ │Shortcut │ │(usePageIdle)  │ │            │  │
│ └───┬────┘ └──────┬───────┘ └─────┬──────┘  │
└─────┼─────────────┼───────────────┼────────┘
      ▼             ▼               ▼
┌──────────────────────────────────────────────┐
│ tauriApi.ts (Bridge: desktopOnly, retry, bio) │
└──────────────────────────────────────────────┘
```

## Data Flow

1. **配置同步流**：
    * 用户修改配置 -> `updateConfig` (行56) -> 标记 `dirtyFields` (行62) -> 500ms Debounce (行77) -> `saveConfigDirect` (行128) -> IPC 调用 `tauriApiWithRetry.saveConfig` (行361)。
    * 后端推送配置变更 -> `onConfigChanged` (行220) -> `mergeConfigFromBackend` (行103) -> 跳过 `dirtyFields` -> 更新 `useConfigStore`。
    * 应用关闭 -> `onCloseRequested` (行41) -> `hasPendingConfig` 检查 (行188) -> `flushPendingConfig` (行195) -> 强制保存。

2. **认证与在线状态流**：
    * 用户登录 -> `doLogin` (行106) -> `api.doLogin` (行125) -> 成功 -> 更新 Auth Store -> 触发质量探测 (若距 `getLastQualityResultTime` 超 60s，行18/141)。
    * 后台检测 -> `onBackgroundCheckResult` (行82) -> 更新 `bgStatus` -> `checkOnline` (行196) -> 串行化调用 `queryPortalStatus` (行56/225) -> 更新 `status`。

3. **动画性能流**：
    * 用户交互 -> `markInteraction` (行26) -> `usePageIdle` 重置空闲计时器 -> `useAdaptiveFramePace` 提升帧率 (行44)。
    * 空闲超时 -> `usePageIdle` 标记 Idle -> `useAnimationActive` Context 变化 (行90) -> 动画 Hooks (`useBreatheAnimation` 行55、`useGlowAnimation` 行42、`usePulseAnimation` 行66) 暂停 GSAP 实例。
    * 后台切换 -> `visibilitychange` -> 暂停心跳 (行7) 和动画。

4. **主题同步流**：
    * 初始化 -> `initTheme` (行34) -> 读取 Storage/Config -> 设置 State。
    * State 变化 -> `subscribe` (行60) -> 修改 DOM Class/CSS Variables -> 持久化 Storage。

5. **日志与 Toast 流**：
    * 各模块调用 `addLog` (行32) 或 `addToast` (行52) -> `useLogToastStore` 状态变更 -> 组件订阅渲染。
    * 后台时普通 `addToast` 直接返回 (行56)，但 `addToastWithAction` 仍入队 (行86)。

## Connections

* [[android-backend-rust]]: Tauri IPC 通信的目标模块（`tauriApi.ts` 中 `invoke` 调用）。
* [[android-ui-components]]: 消费 `useThemeStore`, `useAuthStore`, `useAdapterStore`, `useLogToastStore` 等状态的 UI 组件。
* [[android-shared-types]]: 共享的类型定义，如 `Config`, `NetworkQuality`, `ThemeName`（被 `tauriApi.ts` 和各 store 引用）。
* [[android-face-verify]]: `tauriApi.ts` 中 `verifyWindowsIdentity` (行207) 反向依赖 `@/face/faceVerifyStore` 的 `shouldUseFaceFallback` 与 `useFaceDialogStore`。

## Known Issues

1. **Android 特定降级**：
    * 所有涉及物理网卡、DHCP、DNS DoH、窗口最小化/关闭、托盘的功能均通过 `desktopOnly` (行14) 显式拒绝，或 `noopListener` (行18) 空订阅，避免 IPC 报错（如 `getAdapters` 行196、`dhcp*` 行269、`exportConfig` 行193、`minimizeWindow` 行250）。
    * `checkPortalStatus` 传入空 IP 字符串 (`queryPortalStatus('')` 行56/225)，依赖后端回退逻辑修复此前状态点恒灰。
    * 适配器相关事件 (`onAdaptersChanged` 等) 在 `tauriApi` 中映射为 `noopListener`，`useEventListeners` 中跳过订阅 (行130-132)。

2. **状态一致性保护**：
    * **配置同步**：`dirtyFields` (行20) 机制防止后端旧数据覆盖本地未保存的新数据；失败计数 `DIRTY_FAILURE_LIMIT=3` (行24-25) 机制防止永久脱节。
    * **登录状态**：`doLogin` 成功后不立即 `checkOnline` (行159) 避免状态闪烁；`checkOnline` 使用 `_checkOnlineLockFlag` (行16) 串行化，并在 promise settle 立即释放 (行241) 修复并发。
    * **UI 渲染优化**：`setStatusStable` (行82) 比较引用/值避免无效重渲染；`useAnimationActive` (行95) 全局 Context 减少监听器开销，`AnimationActiveProvider` 用 `useMemo` 稳定 `value` (行90)。

3. **资源泄漏防护**：
    * **定时器清理**：所有 `setTimeout`/`setInterval` 均在 cleanup 函数中清除（`useLogToastStore` 的 `toastTimers` 行5 清理；`useAdaptiveFramePace` 行67-73；`useQualityStore` 的 500ms 解锁定时器行69）。
    * **GSAP 动画**：`usePulseAnimation` 在 unmount 时 `timeline.kill()` (行57)；`useBreatheAnimation`/`useGlowAnimation` 同理清理 tween。
    * **监听器清理**：`createEventListener` (行159) 处理异步注册期间的取消；`useEventListeners` 收集所有 unlisten 函数统一调用 (行257-262)。

4. **StrictMode 兼容**：
    * `useAsyncLock` (行12-17)、`useEventListeners` (行21)、`useInitialDataLoad` (行25,111-113) 等均使用 `mountedRef` 并在 effect setup 阶段重置，防止 React 18 StrictMode 下二次执行导致的逻辑失效或锁永久不释放。

5. **安全性与非显然行为**：
    * `openExternal` (行276) 严格校验 URL 协议(http/https)、长度(≤2048)和内容解析合法性。
    * 密码处理中使用 `PASSWORD_MASK` 标识，并在保存/加载时特殊处理 (`useConfigStore` 行69-75, 116-118, 140-149)。
    * `usePageIdle` (行17) 改为单 `interval` 轮询判定空闲，而非事件内重建定时器；隐藏页签立即判空闲 (行29)，最迟 2.5s 内判定。
    * `useHeartbeat` (行12) 中 `isRenderLoopAlive()` 为假时跳过 `renderHeartbeat` 让后端重载（安卓端 `renderHeartbeat` 为 desktopOnly reject 但被 catch）。
    * `useLogToastStore` (行37-41) 的 `addLog` 仅当末条日志完全同消息同类型才累加 `count` 并刷时间戳，并非完全去重，以保留频次感知风暴；`MAX_TOASTS=4` (行9) 超出时 FIFO 淘汰最旧并清定时器。
    * `useAnimationProfile` (行31) 与 `useDeviceProfile` (行17) 恒返回最高档（HIGH_PROFILE），但保留接口形状供消费方零改。
    * `statusPulse` 注释声称“重复 2 次，1.5s”但代码为 `repeat: -1` 无限循环 (行38)；`loadingPulse` 未设 `force3D` (行46-50)。