---
title: frontend-hooks-store
type: module
source_files:
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
tags: [frontend, hooks, zustand, tauri, state-management, animation, auth, config]
---

## Overview
`frontend-hooks-store` 模块是 Android 前端应用的核心状态管理与基础设施层。它通过 Zustand stores 管理应用初始化、配置、认证、网络质量、日志通知、动画与设备性能等关键领域，并通过 `tauriApi` 桥接 UI 与后端 Rust 能力。该模块解决了安卓端特有的生命周期管理（如后台保活、配置冲刷）、性能优化（自适应帧率、动画节流）以及跨平台兼容性（桌面专属功能在安卓端的降级处理，见 `tauriApi.ts:14-15` `desktopOnly` 与 `tauriApi.ts:18` `noopListener`）。

## Key Components

### 1. Tauri API Bridge (`tauriApi.ts`)
提供与后端 Rust 逻辑交互的统一接口。由于安卓端缺乏桌面端的硬件抽象（如网卡、窗口管理），大部分桌面专属功能被显式拒绝或返回空操作。

- **类型与接口**:
  - `BatteryOptimizationInfo`(26-30)
  - `VendorSettingsResult`(32-36)
  - `AvoidBadWifiStatus`(143-152)
  - `RestoreWrittenSettingsResult`(154-157)
  - `TauriApi`(48-141) 定义全部方法签名。
- **辅助函数**:
  - `desktopOnly<T>(name)`(14-15) 返回 `Promise.reject`，标记安卓端不可用桌面功能。
  - `noopListener`(18) 返回空清理函数，用于安卓端不存在的桌面事件监听。
  - `createEventListener<T>(eventName)`(159-188) 封装 `listen` 并处理取消竞态。
  - `isRetryableError(e)`(326-336) 按 timeout/network/fetch/connection 关键词判定可重试错误。
  - `withRetry<T>(fn, maxRetries=2, baseDelay=500)`(338-354) 指数退避重试。
- **导出对象 `tauriApi: TauriApi`**(190-324):
  - 绝大多数字段经 `invoke` 调后端（如 `getConfig`(191)、`doLogin`(203)、`checkPortalStatus`(200)）。
  - 桌面专属（`exportConfig`(193)、`getAdapters`(196)、`minimizeWindow`(250)、`dhcpRenewAll`(269)、`showWindow`(302)、`renderHeartbeat`(318) 等）走 `desktopOnly`。
  - `onAdaptersChanged`(254)、`onAdapterDisabledWarning`(256)、`onAutoExitCountdown`(297) 等事件走 `noopListener`。
  - `verifyWindowsIdentity`(207-244)：融合系统生物识别与 2D 人脸回退，归一化调 `invoke('verify_biometric_identity')`，失败回传 `{success:false}`。
  - `openExternal`(276-287)：校验 http/https 协议、长度≤2048、URL 解析，失败返回 false。
  - `setAutoLaunch`(289)：invoke `set_boot_autostart` 后强制返回 `{success:true}`。
- **导出对象 `tauriApiWithRetry: TauriApi`**(356-362)：展开 `tauriApi`，仅对 `saveConfig` 套用 `withRetry`。

### 2. State Stores (Zustand)

#### `useAdapterStore` (`useAdapterStore.ts`)
面板导航 store（网卡数据链已删，仅留激活面板态）。
- 接口 `AdapterStore`(6-9)
- 导出 `useAdapterStore`(11-14)：`activePanel` 默认 `'dashboard'`(12)，`setActivePanel`(13)

#### `useAuthStore` (`useAuthStore.ts`)
认证领域 store（登录/注销/在线检测/后台状态）。
- 模块变量 `_checkOnlineLockFlag`(16)、`QUALITY_MANUAL_THROTTLE_MS = 60_000`(21)
- 类型 `CampusStatus`(23)、`PortalStatus`(24)
- 辅助函数：
  - `detectCampusNetwork()`(29-35) 失败返 null
  - `buildCampusBgStatusPatch()`(38-49) 纯函数构造补丁
  - `queryPortalStatus(adapterIp)`(56-65) 安卓无 adapters 恒传空串(53-54)
  - `withTimeout<T>()`(69-78) 竞态超时清理防按钮永久禁用
  - `setStatusStable(next)`(82-86) 引用稳定更新防无效重渲染
- 接口 `AuthStore`(88-98)
- 导出 `useAuthStore`(100-247)：
  - `doLogin`(106-164)：校验锁与 config，先 `saveConfigDirect`(117)，`withTimeout(api.doLogin(), 60000)`(125)；成功更新状态并节流调 `api.checkNetworkQuality`(141-147)；仅失败才 `checkOnline`(159-161)
  - `doLogout`(166-194)：注销后无条件 `checkOnline`(192)
  - `checkOnline`(196-243)：`_checkOnlineLockFlag` 串行化(197-198,241)；campus 检测(204-222)；portal 空 IP 查询(225)；稳定更新 status
  - `setStatus`(245)、`setBgStatus`(246)

#### `useConfigStore` (`useConfigStore.ts`)
配置领域 store（配置对象、密码状态、账号、语言）。
- 模块变量 `saveConfigTimer`(14)、`saveConfigPending`(15)、`saveConfigInFlight`(17)、`dirtyFields`(20)、`DIRTY_FAILURE_LIMIT = 3`(24)、`dirtyFailureCounts`(25)
- 接口 `ConfigStore`(27-46)
- 导出 `useConfigStore`(48-186)：
  - `updateConfig`(56-88)：合并并标 dirty，防抖 500ms(77) 调 `saveConfigDirect`，处理 MASK 密码(69-71) 与主题色同步(87)
  - `updateConfigLocal`(90-99)：纯本地更新并标 dirty
  - `mergeConfigFromBackend`(103-119)：跳过 dirty 字段，同步 `passwordSaved`(116-118)
  - `saveConfigDirect`(128-176)：拼全量配置 invoke `saveConfig`；成功清 dirty 并标密码已存(140-142)；失败清密码标记(148-150)且连续失败达限弃 dirty(152-162)；`saveConfigInFlight` 跟踪在途(168-175)
  - `setAccounts`(178)、`setActiveAccount`(179)、`setLanguage`(181-185) 写 `safeStorage` 并切 i18next
- 导出 `hasPendingConfig()`(188-193)：pending 或 in-flight 任一存在即真
- 导出 `flushPendingConfig()`(195-224)：清 timer，发 pending（MASK 处理），纳入 in-flight 返回 Promise 供关窗等待

#### `useQualityStore` (`useQualityStore.ts`)
网络质量领域 store，管理网络质量、DNS DoH、版本更新状态。
- 模块变量 `_qualityLockFlag`(11)、`lastQualityResultTime`(16)
- 导出 `getLastQualityResultTime()`(18)
- 接口 `QualityStore`(22-41)
- 导出 `useQualityStore`(43) 默认 state(44-52)：`networkQuality: null`, `dnsDohStatus: null`, `dnsChecking: false`, `isRefreshingQuality: false`, `updateAvailable: false`, `latestVersion: ''`, `releaseNotes: ''`, `updatePromptOpen: false`, `refreshRate: 0`
- 关键方法：
  - `refreshQuality`(54-74)：取 `useConfigStore.getState().config`(55)；锁或 `config.enableNetworkQuality === false` 时 return(56-57)；置锁并 `set({ isRefreshingQuality: true })`；`await api.checkNetworkQuality?.()`(61)；结果经 `mergeNetworkQuality(old, q)` 由 `setNetworkQuality` 写入(64)；catch 仅 DEV `console.error`(66-67)；finally 中 500ms 后解锁(68-73)
  - `setNetworkQuality`(78-82)：支持函数/值入参，若 `next !== null` 则更新 `lastQualityResultTime = Date.now()`
  - 其余 setter：`setDnsDohStatus`, `setDnsChecking`, `setUpdateAvailable`, `setLatestVersion`, `setReleaseNotes`, `setUpdatePromptOpen`(83-88)

#### `useLogToastStore` (`useLogToastStore.ts`)
日志与 Toast 全局 store。
- 常量 `MAX_TOASTS = 4`(9)、`toastTimers` Map(5)、`isDuplicateTitle`(13)
- 接口 `LogToastStore`(15-26)
- 导出 `useLogToastStore`(28-135)：
  - `addLog`(32-50)：同内容刷新时间戳并 count++(37-42)，超 `MAX_LOG_ENTRIES` 截断(45-47)
  - `addToast`(52-82)：非前台跳过(56)，同题跳过(61)，超 4 淘汰最旧(64-68)，定时移除(77-81)
  - `addToastWithAction`(84-108)：默认 duration 8000，非前台也入队
  - `removeToast`(110-114)、`removeToastsByPrefix`(116-126)、`setLogs`(128)、`cleanupToasts`(130-135)

#### `useThemeStore` (`useThemeStore.ts`)
主题领域 store，管理主题名、浅色模式、自定义色、玻璃档位，并注入 DOM 副作用。
- 接口 `ThemeStore`(9-19)
- 导出 `useThemeStore`(21) 初始 state(22-28)：`themeName: 'default'`(22), `isLightMode` 从 storage(23), `customThemeColor: '#6366f1'`(24), `glassLevel` 初始取 storage 回退 `'standard'`(25-28)
- 方法：
  - `initTheme(cfg)`(34-54)：从 storage 恢复 `themeName`(35-36)；依 storage 或 `cfg.themeMode` 决定 `isLightMode`(37-52)；应用 `cfg.customThemeColor`(53)
  - 简单 setter：`setThemeName`, `setIsLightMode`, `setGlassLevel`, `setCustomThemeColor`(30-32,56)
- 模块级 DOM 副作用 `useThemeStore.subscribe`(60-97)：
  - `glassLevel` 变：设 `data-glass` 并写 storage(62-65)
  - `isLightMode` 变：toggle `dark` class 与 `data-light`(66-73)
  - `themeName/customThemeColor/isLightMode` 变：移除预定义 `themeClasses` 列表(76-77)；`custom` 主题用 `hexToHsl` 转色并 `style.setProperty` 写变量(78-87)；非 custom 移除变量并按名加 class(88-96)

#### `useDeviceProfile` (`useDeviceProfile.ts`)
设备性能分级（v2.4.0 折叠为恒旗舰档）。
- 类型 `DeviceTier`(6)、`DeviceProfile`(8-15)
- 常量 `HIGH_PROFILE`(17-23) idleFps=30、activeFps=60
- 导出 `useDeviceProfile()`(25-27) 返回 HIGH_PROFILE

#### `useAppStore` (`useAppStore.ts`)
旧引用兼容 re-export。
- 导出 `useAppInit`(2)
- `hasPendingConfig`、`flushPendingConfig`(3)（来自 useConfigStore）

### 3. Animation & Performance Hooks

#### `useAnimationProfile` (`useAnimationProfile.ts`)
动画配置档（v2.4.0 起恒最高档，保留接口形状兼容消费方）。
- 类型 `AnimationTier`(6)、接口 `AnimationProfile`(8-26)
- 常量 `HIGH_PROFILE`(31-49) 含 `magneticOffset=5`、`numberDuration=600`、`springStiffness=400`、`refreshRate=60` 等
- 导出 `useAnimationProfile()`(51-57)：从 `useQualityStore` 取 `refreshRate`，`useMemo` 中若 ≤0 取 120(54)，覆盖 `easing` 与 `refreshRate`

#### `useAdaptiveFramePace` (`useAdaptiveFramePace.ts`)
智能帧率控制（LTPO 思路），驱动 gsap 全局 ticker 动态限帧。
- 常量 `ACTIVE_WINDOW_MS = 2000`(16)、`PACE_POLL_MS = 250`(18)；模块级 `idleFps=30`、`activeFps=60`(21-22)
- `markInteraction()`(26-29) 推活跃窗口并即时提帧
- `currentPace()`(31-35) 后台返 0、活跃返 activeFps 否则 idleFps
- `applyPace()`(38-45) 调 `gsap.ticker.fps`（非 60 才限帧）
- 导出 `useAdaptiveFramePace()`(51-75)：依赖 `useDeviceProfile`(52) 取 fps，挂载 passive 事件(59-61)，`setInterval` 轮询回落(64)，卸载还原 fps(72)

#### `useBreatheAnimation` (`useBreatheAnimation.ts`)
呼吸动画 Hook（gsap）。
- 接口 `BreatheOptions`(5-13)
- 导出 `useBreatheAnimation(options)`(15-66)：默认 minOpacity=0.6、maxOpacity=1、duration=4(19-27)；建 yoyo/repeat tween(36-46)；依赖 `useAnimationActive`(17,55-63) 暂停/恢复

#### `useGlowAnimation` (`useGlowAnimation.ts`)
发光动画 Hook（gsap）。
- 接口 `GlowOptions`(5-9)
- 导出 `useGlowAnimation(options)`(11-53)：默认 duration=4、maxScale=1.15、maxOpacity=0.6(15)；yoyo tween(24-32)；`useAnimationActive`(13,42-50) 控暂停/恢复

#### `usePulseAnimation` (`usePulseAnimation.ts`)
GSAP 驱动的脉冲/心跳动画 Hook，替代 CSS @keyframes，返回绑定目标 DOM 的 ref。
- 接口 `PulseOptions { type: 'heartbeat' | 'statusPulse' | 'loadingPulse' }`(5)
- 导出 `usePulseAnimation(options)`(15)
  - 主 `useEffect`(21-60)：按 `type` 分支构建 GSAP timeline（`heartbeat`(28-35), `statusPulse`(37-44), `loadingPulse`(46-50)），清理 `timeline?.kill()`(56-59)
  - 次 `useEffect`(63-71)：依 `animActive` 调 `tl.resume()/pause()`
  - 返回 `ref`(73)

#### `usePageIdle` (`usePageIdle.ts`)
页面空闲/可见/聚焦状态及动画激活 Context。
- `usePageIdle()`(3-49)：`IDLE_TIMEOUT=2000`(6)、`IDLE_CHECK_INTERVAL=500`(8)；interval 轮询判定(17-20)，事件 mousemove 等(26-27)，visibilitychange 置 idle(28-36)，body 切 `anim-idle` class(44-46)
- `usePageVisible()`(51-61)、`useWindowFocused()`(63-78)
- `AnimationActiveContext`(83)
- 导出 `AnimationActiveProvider`(85-93) 用 useMemo 稳值并以 `createElement` 提供
- 导出 `useAnimationActive()`(95-97) 消费 Context

### 4. Initialization & Lifecycle Hooks

#### `useAppInit` (`useAppInit.ts`)
组合应用初始化钩子。
- 导出 `useAppInit()`(6-11)：依次调用 `useEventListeners`、`useInitialDataLoad`、`useHeartbeat`、`useGlobalShortcut`

#### `useInitialDataLoad` (`useInitialDataLoad.ts`)
启动初始化数据加载。
- `VALID_PANELS`(15) 来自 NAV_ITEMS
- 导出 `useInitialDataLoad()`(17-114)：`mountedRef` 处理 StrictMode(18,25,112)
  - `api.getInitData()`(31) 失败降级 DEFAULT_CONFIG 并置 `configLoaded`(107)；成功合并配置(34)，MASK 密码判定(35-39)，`useThemeStore.initTheme`(42)
  - 恢复面板 `setActivePanel`(44-53)，质量禁用时跳过 quality 面板(47-51)
  - 初始化 bgStatus(57-78) 并直接置在线绿态(72-77)；账号列表/活跃账号写入(80-84)；触发 `checkOnline`(86)；`refreshRate` 写入 QualityStore(91-93)；置 `configLoaded: true`(101)

#### `useEventListeners` (`useEventListeners.ts`)
全局事件监听装配与生命周期兜底。
- 导出 `useEventListeners()`(14-264)：`lastBgCheckTimeRef` 节流 1s(15,86-87)
  - `getCurrentWindow().onCloseRequested`(41-66)：有未存配置则 `preventDefault` + `flushPendingConfig` 限时 2s 等待(42-60)
  - `flushPendingNow`(71-75) 绑 `visibilitychange`/`pagehide`(76-80)
  - 注册 `api.onBackgroundCheckResult`(82-115) 更新 bgStatus/status；`onAutoLoginResult`(117-128) 触发 `checkOnline`；`onLoginLog`(134-140)；`onAutoExitCountdown/Cancelled`(142-168)；`onCampusExitCountdown/Cancelled`(170-196)；`onNetworkQualityResult`(198-204) 调内部 `handleQualityBadAlert`(25-39) 与 `mergeNetworkQuality`；`onUpdateAvailable`(206-218)；`onConfigChanged`(220-241) 调 `mergeConfigFromBackend` 并同步账号；`onUpdateUnverified`(244-255)
  - `mountedRef`(16,21,258) 处理 StrictMode

#### `useHeartbeat` (`useHeartbeat.ts`)
渲染心跳上报。
- 导出 `useHeartbeat()`(5-20)：`document.hidden` 暂停(7-9)；`setInterval` 5s 调 `api.renderHeartbeat`(10-13)，经 `isRenderLoopAlive()` 校验跳过 GPU 崩溃(12)

#### `useGlobalShortcut` (`useGlobalShortcut.ts`)
全局快捷键（Ctrl+Shift+C 取消自动退出）。
- 导出 `useGlobalShortcut()`(4-25)：keydown 中判定 `isEditable`(10-15) 忽略输入框，调 `api.cancelAutoExit`(19)

#### `useAsyncLock` (`useAsyncLock.ts`)
异步操作冷却锁 Hook。
- 导出 `useAsyncLock<T>(fn, cooldownMs=1500)`(3-27)：返回 `[isRunning, execute]`。`lockRef`(8) 防重入，`mountedRef`(9,12-17) 处理 StrictMode 二次 setup 恢复；`execute`(18-25) finally 中 `setTimeout` 释放

#### `useFormFactor` (`useFormFactor.ts`)
设备形态与方向判定（sw600dp 语义）。
- 类型 `FormFactor`(9)、`Orientation`(10)；常量 `TABLET_MIN_EDGE_PX = 600`(13)
- `detectFormFactor`(15-17) 短边≥600 为 tablet；`detectOrientation`(19-21) 宽≥高为 landscape
- `useViewportMeasure<T>(calc)`(24-36) 监听 resize/orientationchange
- 导出 `useFormFactor()`(38-40)、`useOrientation()`(43-45)

（注：`useStartupBoost.ts` 在观察批2中确认文件不存在或读取失败，无源码可观察，故不提供组件细节。）

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                       App Root (main.tsx)                    │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │           useAppInit() (useAppInit.ts:6)                │ │
│  │  ► useEventListeners (useEventListeners.ts:14)         │ │
│  │  ► useInitialDataLoad (useInitialDataLoad.ts:17)       │ │
│  │  ► useHeartbeat (useHeartbeat.ts:5)                    │ │
│  │  ► useGlobalShortcut (useGlobalShortcut.ts:4)         │ │
│  └─────────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │              Zustand Stores (State)                     │ │
│  │  • useAuthStore (useAuthStore.ts:100)                  │ │
│  │  • useConfigStore (useConfigStore.ts:48)               │ │
│  │  • useQualityStore (useQualityStore.ts:43)             │ │
│  │  • useLogToastStore (useLogToastStore.ts:28)           │ │
│  │  • useThemeStore (useThemeStore.ts:21)                 │ │
│  │  • useAdapterStore (useAdapterStore.ts:11)             │ │
│  └─────────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │         Animation & Performance Layer                   │ │
│  │  • AnimationActiveProvider (usePageIdle.ts:85)         │ │
│  │  • useAdaptiveFramePace (useAdaptiveFramePace.ts:51)   │ │
│  │  • useAnimationProfile (useAnimationProfile.ts:51)     │ │
│  │  • useBreathe/Glow/Pulse (gsap tweens)                 │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌──────────────────────────────────────────────────────────────┐
│                   Tauri API Bridge (tauriApi.ts)             │
│  • invoke<T>(cmd, args) → Backend Rust                      │
│  • listen<T>(event, cb) → Backend Events                    │
│  • desktopOnly/noopListener → Android Degradation           │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **Initialization**:
   - `useAppInit`(useAppInit.ts:6) 挂载 `useEventListeners`、`useInitialDataLoad`、`useHeartbeat`、`useGlobalShortcut`。
   - `useInitialDataLoad`(useInitialDataLoad.ts:17) 调 `api.getInitData()`(31) 拉取 `InitData`，分别写入 `useConfigStore`(config/accounts/activeAccount/configLoaded)、`useAuthStore`(bgStatus/status)、`useAdapterStore`(activePanel)、`useQualityStore`(refreshRate)、`useThemeStore`(initTheme)，随后调 `useAuthStore.checkOnline`(86) 触发在线检测。

2. **Authentication & Online Check**:
   - 用户点击登录 -> `useAuthStore.doLogin()`(106-164)。先 `saveConfigDirect`(117)，`withTimeout(api.doLogin(), 60000)`(125)；成功更新状态并节流调 `api.checkNetworkQuality`(141-147)；仅失败才 `checkOnline`(159-161)。
   - 后台循环 `api.onBackgroundCheckResult`(82-115) 驱 `useAuthStore`(setBgStatus/setStatus/checkOnline)；`onConfigChanged`(220-241) 驱 `useConfigStore`(mergeConfigFromBackend/setAccounts/setActiveAccount)。

3. **Configuration Management**:
   - UI 调 `useConfigStore.updateConfig`(56-88) → 防抖 500ms(77) → `saveConfigDirect`(128-176) → `tauriApiWithRetry.saveConfig`(356-362)（带 `withRetry`(338-354)）。
   - 后端 `config-changed` 事件回传时，`mergeConfigFromBackend`(103-119) 跳过本地 `dirtyFields`(20)。
   - 关窗/隐藏时 `useEventListeners`(41-80) 调 `hasPendingConfig`(188-193)/`flushPendingConfig`(195-224) 等待在途保存。

4. **Performance & Animation**:
   - `useAdaptiveFramePace`(51-75) 依赖 `useDeviceProfile`(25-27) 的 fps 驱 `gsap.ticker`；`useBreatheAnimation`/`useGlowAnimation`/`usePulseAnimation` 依赖 `usePageIdle.useAnimationActive`(95-97) 控 gsap tween 暂停。
   - `useAnimationProfile`(51-57) 依赖 `useQualityStore.refreshRate` 算缓动；`AnimationActiveProvider`(85-93) 在顶层组合 `usePageIdle/Visible/Focused` 供消费者读取。

## Connections
*   [[backend-rust-api]]: Tauri commands invoked via `tauriApi.ts` (e.g., `invoke` in `tauriApi.ts:190-324`).
*   [[ui-components]]: Consumes stores (`useAuthStore`, `useConfigStore`, etc.) for rendering.
*   [[settings-module]]: Defines `Config` type used by `useThemeStore.initTheme` (useThemeStore.ts:34) and `useConfigStore`.
*   [[monitor-module]]: Defines `BackgroundStatus`, `NetworkQuality` types used by `useAuthStore` and `useQualityStore`.
*   [[shared-ui-constants]]: Defines `NAV_ITEMS` (used in useInitialDataLoad.ts:15), `MAX_LOG_ENTRIES` (useLogToastStore.ts), `PASSWORD_MASK` (useConfigStore.ts:69).
*   [[lib-latency]]: Provides `mergeNetworkQuality` used in `useQualityStore.ts:64`.
*   [[lib-color]]: Provides `hexToHsl` used in `useThemeStore.ts:78-87`.
*   [[lib-utils]]: Provides `safeStorage` used in `useThemeStore.ts` and `useConfigStore.ts`.

## Known Issues

1. **Desktop-Only Features on Android**:
   - Functions like `getAdapters`(tauriApi.ts:196), `minimizeWindow`(250), `dhcpRenewAll`(269), `exportConfig`(193) are explicitly rejected via `desktopOnly`(tauriApi.ts:14-15) or `noopListener`(18). This is a deliberate degradation, not a bug, but requires UI components to handle potential errors or hide related features.

2. **StrictMode Double Invocation**:
   - `useInitialDataLoad`(18,25,112), `useEventListeners`(16,21,258), `useAsyncLock`(12-17) explicitly handle React StrictMode's double mount/unmount cycle using `mountedRef` to prevent async operations from being silently dropped or locks from permanent engagement.

3. **Toast Accumulation Boundary**:
   - `useLogToastStore.ts` enforces `MAX_TOASTS = 4`(9) and evicts oldest entries(64-68). Non-foreground normal toasts are skipped(56) but action toasts are queued(84-108). Duplicate titles are ignored(13,61).

4. **Inline Style Accumulation Fix**:
   - `useThemeStore.ts`(78-87) uses `style.setProperty` and `removeProperty` for `custom` theme HSL variables, replacing historical `cssText +=` growth defect.

5. **Race Conditions in `checkOnline` Serialization**:
   - `_checkOnlineLockFlag`(useAuthStore.ts:16,197-198,241) serializes calls and releases upon promise settlement, preventing concurrent redundant invokes.

6. **Lost Auto-Login Status Prevention**:
   - `useInitialDataLoad.ts`(72-77) proactively sets online green status from `initData.backgroundStatus.online` before listeners establish, avoiding race loss.

7. **Quality Result Timestamp Unified**:
   - `useQualityStore.ts`(78-82) `setNetworkQuality` updates `lastQualityResultTime` from any source, fixing P2-F5 redundant detection. `refreshQuality` lock released after 500ms(68-73).

8. **Frame Pace Polling**:
   - `useAdaptiveFramePace.ts`(64) uses `setInterval` (PACE_POLL_MS=250) instead of rAF recursion to avoid Chromium persistent full-frame rendering. `usePageIdle.ts`(17-20) similarly uses interval polling.

9. **Biometric Fallback**:
   - `verifyWindowsIdentity`(tauriApi.ts:207-244) normalizes failure to `{success:false}` without throwing IPC error, using 2D face fallback when system unavailable.

10. **Config Save Failure Degradation**:
   - `useConfigStore.ts`(152-162) abandons `dirtyFields` after `DIRTY_FAILURE_LIMIT = 3`(24) consecutive save failures to resync with backend.