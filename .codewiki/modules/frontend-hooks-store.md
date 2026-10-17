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
  - android/frontend/src/hooks/useThemeStore.ts
tags: [frontend, hooks, zustand, tauri, state-management, animation, auth, config]
---

## Overview
`frontend-hooks-store` 模块是 Android 前端应用的核心状态管理与基础设施层。它通过 Zustand stores 管理应用初始化、配置、认证、网络质量、日志通知、动画与设备性能等关键领域，并通过 `tauriApi` 桥接 UI 与后端 Rust 能力。该模块解决了安卓端特有的生命周期管理（如后台保活、配置冲刷）、性能优化（自适应帧率、动画节流）以及跨平台兼容性（桌面专属功能在安卓端的降级处理）。

## Key Components

### 1. Tauri API Bridge (`tauriApi.ts`)
提供与后端 Rust 逻辑交互的统一接口。由于安卓端缺乏桌面端的硬件抽象（如网卡、窗口管理），大部分桌面专属功能被显式拒绝或返回空操作。

- **常量与辅助函数**:
  - `desktopOnly<T>(name: string)`: (L38) 返回被拒绝的 Promise，用于标记安卓端不可用的桌面功能。
  - `noopListener()`: (L42) 返回无操作的清理函数，用于安卓端不存在的桌面事件监听。
  - `createEventListener<T>()`: (L79-L101) 创建安全的 Tauri 事件监听器，处理异步 listen 和取消订阅。
  - `isRetryableError(e: unknown)`: (L185-L196) 判断错误是否可重试（包含 timeout/network/fetch/connection 关键字）。
  - `withRetry<T>()`: (L198-L215) 指数退避重试机制，仅用于 `saveConfig`。

- **接口定义**:
  - `interface TauriApi`: (L44-L128) 定义了所有后端 RPC 方法的类型签名，包括配置、认证、网络、更新等。
  - `interface AvoidBadWifiStatus`: (L130-L139) 描述系统网络设置写入权限及快照状态。
  - `interface RestoreWrittenSettingsResult`: (L141-L144) 还原系统设置的结果。

- **实现对象 `tauriApi`**: (L103-L183)
  - `getConfig`, `saveConfig`: (L104-L105) 配置读写。
  - `verifyWindowsIdentity`: (L118-L150) 生物识别验证门。安卓端使用 `biometricAuthenticate`，若失败或回退条件满足则调用 `useFaceDialogStore` 进行 2D 人脸验证。
  - `openExternal`: (L165-L176) 安全地打开外部 URL，校验协议和长度。
  - `onBackgroundCheckResult`, `onLoginLog`, `onNetworkQualityResult` 等: (L151-L154, L160-L161) 使用 `createEventListener` 注册事件。
  - 桌面专属方法（如 `getAdapters`, `minimizeWindow`, `dhcpRenewAll` 等）均指向 `desktopOnly`。

- **导出对象 `tauriApiWithRetry`**: (L217-L221)
  - 扩展 `tauriApi`，仅对 `saveConfig` 应用 `withRetry` 逻辑。

### 2. State Stores (Zustand)

#### `useAuthStore` (`useAuthStore.ts`)
管理登录状态、在线检测及后台检查状态。

- **State**:
  - `isLoggingIn`: (L68) 登录中标志。
  - `isLoggingOut`: (L69) 注销中标志。
  - `status`: (L70) `{ text: string; state: StatusState }`，当前状态文本和视觉状态。
  - `bgStatus`: (L71) `BackgroundStatus`，后台检测的详细状态。

- **Actions**:
  - `doLogin()`: (L74-L128) 执行登录。包含超时包装 (`withTimeout`, 60s)、配置保存、质量探测节流（60s 内不重复全量检测）、状态稳定更新。登录成功跳过立即复查以防状态闪烁。
  - `doLogout()`: (L130-L156) 执行注销。同样包含超时包装和状态更新。
  - `checkOnline(cfg?)`: (L158-L198) 在线状态检测。包含校园网检测 (`detectCampusNetwork`) 和 Portal 状态查询 (`queryPortalStatus`)。使用 `_checkOnlineLockFlag` 串行化请求，防止并发冗余 invoke。
  - `setStatus`, `setBgStatus`: (L200-L201) 状态设置器。

#### `useConfigStore` (`useConfigStore.ts`)
管理应用配置、密码状态、账号列表及语言。

- **State**:
  - `config`: (L46) 当前配置对象。
  - `configLoaded`: (L48) 配置是否已从后端加载完成。
  - `passwordSaved`: (L49) 密码是否已保存（用于 UI 显示占位符）。
  - `accounts`: (L50) 账号列表。
  - `activeAccount`: (L51) 当前激活账号 ID。
  - `language`: (L52) 当前语言。

- **Actions**:
  - `updateConfig(partial)`: (L56-L82) 合并配置并触发防抖保存（500ms）。标记 `dirtyFields` 以防止后端旧快照覆盖本地未确认值。
  - `updateConfigLocal(partial)`: (L84-L92) 仅本地更新配置，不触发保存。
  - `mergeConfigFromBackend(incoming)`: (L94-L101) 合并后端推送的配置，跳过脏字段。
  - `saveConfigDirect(cfg, clearPassword?)`: (L108-L153) 直接调用后端保存。处理密码掩码、脏字段重置、连续失败计数（阈值 3 次后放弃脏标记以重新同步）。
  - `hasPendingConfig()`: (L159-L162) 检查是否有待保存配置（pending 或 in-flight）。
  - `flushPendingConfig()`: (L164-L190) 冲刷待保存配置，用于关闭窗口或切后台时。

#### `useQualityStore` (`useQualityStore.ts`)
管理网络质量、DNS DoH 状态及版本更新信息。

- **State**:
  - `networkQuality`: (L30) `NetworkQuality | null`。
  - `dnsDohStatus`: (L31) `DnsDohStatus | null`。
  - `refreshRate`: (L38) 刷新率，驱动动画性能。
  - `updateAvailable`, `latestVersion`, `releaseNotes`, `updatePromptOpen`: (L39-L42) 更新相关状态。

- **Actions**:
  - `refreshQuality()`: (L45-L62) 手动刷新网络质量，受锁和配置开关保护。
  - `setNetworkQuality(q)`: (L64-L68) 设置质量并更新时间戳 `lastQualityResultTime`，用于登录后节流判断。

#### `useLogToastStore` (`useLogToastStore.ts`)
管理日志条目和 Toast 通知。

- **State**:
  - `logs`: (L26) `LogEntry[]`，最多保留 `MAX_LOG_ENTRIES`。
  - `toasts`: (L27) `ToastMessage[]`，最多保留 `MAX_TOASTS` (4)。

- **Actions**:
  - `addLog(message, type)`: (L30-L50) 添加日志，支持去重和计数累计。
  - `addToast(title, type, ...)`: (L52-L82) 添加 Toast，支持同题去重、容量淘汰、非前台过滤。
  - `addToastWithAction(toast)`: (L84-L107) 添加带操作按钮的 Toast，非前台也入队。
  - `removeToastsByPrefix(prefix)`: (L112-L121) 按前缀移除 Toast 及其定时器。

#### `useThemeStore` (`useThemeStore.ts`)
管理主题名称、亮暗模式及自定义颜色。

- **Side Effects**: (L68-L96)
  - 订阅状态变化，动态修改 `document.documentElement` 的 class 和 CSS 变量。
  - 处理 `theme-custom` 时的 HSL 转换和内联样式注入。

#### `useAdapterStore` (`useAdapterStore.ts`)
面板导航状态。安卓端网卡数据链已删除，仅保留 `activePanel` 供导航组件使用。

#### `useDeviceProfile` (`useDeviceProfile.ts`)
设备性能分级。安卓端已折叠为恒旗舰档 (`HIGH_PROFILE`)，`idleFps: 30`, `activeFps: 60`。

### 3. Animation & Performance Hooks

#### `useAdaptiveFramePace` (`useAdaptiveFramePace.ts`)
智能帧率控制，基于 GSAP ticker。

- **Logic**:
  - `markInteraction()`: (L33-L36) 标记交互发生，推高活跃窗口时间并立即提帧。
  - `currentPace()`: (L38-L42) 根据 `lastActiveAt` 和页面可见性计算当前帧率（0/30/60）。
  - `applyPace()`: (L44-L50) 应用帧率到 `gsap.ticker.fps()`。
  - Hook 挂载: (L56-L78) 监听 `pointerdown`, `touchstart` 等事件重置活跃计时器，并使用 `setInterval` (250ms) 轮询回落帧率，避免常驻 rAF 导致的功耗问题。

#### `useAnimationProfile` (`useAnimationProfile.ts`)
动画参数配置。恒最高档 (`HIGH_PROFILE`)，根据 `refreshRate` 调整 easing 曲线。

#### `usePageIdle` (`usePageIdle.ts`)
页面空闲检测与动画活性上下文。

- `usePageIdle()`: (L3-L36) 检测用户空闲（2s 无交互），使用 interval 轮询而非频繁定时器重建。
- `AnimationActiveProvider`: (L75-L83) 组合 `isVisible`, `isFocused`, `isIdle`，通过 Context 提供 `useAnimationActive()`。解决原实现中多个实例导致的大量监听器注册问题。

#### 具体动画 Hooks
- `useBreatheAnimation` (`useBreatheAnimation.ts`, L1-L45): 呼吸效果（opacity/scale/rotation 循环），受 `useAnimationActive` 控制暂停/恢复。
- `useGlowAnimation` (`useGlowAnimation.ts`, L1-L45): 发光效果（scale/opacity 循环），受 `useAnimationActive` 控制。
- `usePulseAnimation` (`usePulseAnimation.ts`, L1-L82): 脉冲/心跳动画，支持 `heartbeat`, `statusPulse`, `loadingPulse` 三种类型。

### 4. Initialization & Lifecycle Hooks

#### `useAppInit` (`useAppInit.ts`)
应用初始化入口。

- `useAppInit()`: (L6-L10) 依次调用：
  - `useEventListeners()`: 注册全局事件监听。
  - `useInitialDataLoad()`: 加载初始数据。
  - `useHeartbeat()`: 启动心跳检测。
  - `useGlobalShortcut()`: 注册全局快捷键。

#### `useInitialDataLoad` (`useInitialDataLoad.ts`)
首次数据加载。

- **Logic**: (L16-L96)
  - 调用 `api.getInitData()` 获取配置、背景状态、账号列表、刷新率。
  - 处理密码掩码同步 `passwordSaved`。
  - 初始化主题和默认面板。
  - 如果后端已知在线，提前设置状态以避免竞态丢失。
  - 最终调用 `checkOnline` 并标记 `configLoaded: true`。
  - 失败时降级为默认配置并记录日志。

#### `useEventListeners` (`useEventListeners.ts`)
全局事件监听管理器。

- **Logic**: (L12-L198)
  - **Close Request**: (L30-L46) 拦截窗口关闭，若有待保存配置则 `await flushPendingConfig()` (限时 2s)。
  - **Visibility/PageHide**: (L48-L54) 切后台或卸载时冲刷配置。
  - **Tauri Events**:
    - `onBackgroundCheckResult`: (L56-L82) 更新 `bgStatus` 和 `status`，保持引用稳定。
    - `onAutoLoginResult`: (L84-L94) 自动登录结果处理。
    - `onLoginLog`: (L96-L102) 登录日志处理。
    - `onAutoExitCountdown` / `onCampusExitCountdown`: (L104-L136) 倒计时通知，提供“取消退出”操作。
    - `onNetworkQualityResult`: (L138-L145) 更新质量并处理低质量告警。
    - `onUpdateAvailable`: (L147-L159) 更新可用通知。
    - `onConfigChanged`: (L161-L178) 合并后端配置，同步激活账号和刷新账号列表。
    - `onUpdateUnverified`: (L180-L190) 更新源不可用警示。

#### `useHeartbeat` (`useHeartbeat.ts`)
渲染存活检测。

- **Logic**: (L6-L16)
  - 每 5s 调用 `api.renderHeartbeat()`。
  - 若页面隐藏或渲染链失活 (`isRenderLoopAlive` 返回 false) 则跳过，让后端重载 WebView。

#### `useGlobalShortcut` (`useGlobalShortcut.ts`)
全局快捷键。

- **Logic**: (L6-L20)
  - 监听 `Ctrl+Shift+C`，忽略可编辑元素目标，调用 `api.cancelAutoExit()`。

#### `useAsyncLock` (`useAsyncLock.ts`)
异步操作互斥锁。

- **Logic**: (L4-L24)
  - 防止同一操作并发执行，包含冷却时间 (`cooldownMs`)。
  - 处理 StrictMode 下的双重挂载问题。

#### `useFormFactor` (`useFormFactor.ts`)
设备形态判定。

- **Logic**: (L12-L38)
  - 基于 `window.innerWidth/Height` 短边 >= 600px 判定平板。
  - 响应 `resize` 和 `orientationchange` 事件。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                       App Root (main.tsx)                    │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │           useAppInit() Hook Chain                       │ │
│  │  ► useEventListeners (tauriApi events + lifecycle)      │ │
│  │  ► useInitialDataLoad (getInitData + checkOnline)       │ │
│  │  ► useHeartbeat (renderLiveness check)                  │ │
│  │  ► useGlobalShortcut (keyboard listener)                │ │
│  └─────────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │              Zustand Stores (State)                     │ │
│  │  • useAuthStore (login/logout/status)                   │ │
│  │  • useConfigStore (config/password/accounts)            │ │
│  │  • useQualityStore (network/dns/update)                 │ │
│  │  • useLogToastStore (logs/toasts)                       │ │
│  │  • useThemeStore (theme/lightMode/color)                │ │
│  │  • useAdapterStore (activePanel)                        │ │
│  └─────────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │         Animation & Performance Layer                   │ │
│  │  • AnimationActiveProvider (Context for idle/focus)     │ │
│  │  • useAdaptiveFramePace (GSAP ticker control)           │ │
│  │  • useAnimationProfile (easing/spring params)           │ │
│  │  • useBreathe/Glow/Pulse (GSAP tweens)                  │ │
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

1.  **Initialization**:
    *   `useAppInit` triggers `useInitialDataLoad`.
    *   `useInitialDataLoad` calls `api.getInitData()`.
    *   Data is merged into `useConfigStore`, `useAuthStore`, `useQualityStore`, and `useThemeStore`.
    *   `configLoaded` becomes `true`, signaling downstream logic (e.g., self-service panel) that config is ready.

2.  **Authentication & Online Check**:
    *   User clicks Login -> `useAuthStore.doLogin()`.
    *   `doLogin` saves config, calls `api.doLogin()`.
    *   On success, updates status to `online`. Skips immediate `checkOnline` to prevent flicker.
    *   Triggers throttled `checkNetworkQuality` if enabled.
    *   Background loop (`api.onBackgroundCheckResult`) continuously updates `bgStatus` and `status` in `useAuthStore`.

3.  **Configuration Management**:
    *   User changes setting -> `useConfigStore.updateConfig()`.
    *   Debounced save (500ms) -> `useConfigStore.saveConfigDirect()`.
    *   `saveConfigDirect` calls `api.saveConfig()` with retry logic.
    *   On success, clears `dirtyFields`. On failure, increments failure count; if limit reached, clears dirty flag to allow backend sync.
    *   Window close / background transition -> `flushPendingConfig()` ensures data persistence before process termination.

4.  **Performance & Animation**:
    *   `useAdaptiveFramePace` monitors user interaction.
    *   Interaction resets `lastActiveAt`, triggering `gsap.ticker.fps(60)`.
    *   Idle detection (via `usePageIdle` and polling) drops FPS to 30 or 0 (background).
    *   `AnimationActiveProvider` exposes `useAnimationActive()` to all animation hooks (`useBreathe`, `useGlow`, etc.) to pause/resume tweens based on visibility, focus, and idle state.

## Connections
*   [[backend-rust-api]]: Tauri commands invoked via `tauriApi`.
*   [[ui-components]]: Consumes stores (`useAuthStore`, `useConfigStore`, etc.) for rendering.
*   [[settings-module]]: Defines `Config`, `AccountItem` types used by stores.
*   [[monitor-module]]: Defines `BackgroundStatus`, `NetworkQuality` types.
*   [[shared-ui-constants]]: Defines `NAV_ITEMS`, `PASSWORD_MASK`, `MAX_LOG_ENTRIES`.

## Known Issues

1.  **Desktop-Only Features on Android**:
    *   Functions like `getAdapters`, `minimizeWindow`, `dhcpRenewAll`, `setupDnsDoh` are explicitly rejected via `desktopOnly` in `tauriApi.ts` (L106-L108, L151-L153, L162-L164, L179-L180). This is a deliberate degradation, not a bug, but requires UI components to handle potential errors or hide related features.

2.  **StrictMode Double Invocation**:
    *   `useInitialDataLoad`, `useEventListeners`, and `useAsyncLock` explicitly handle React StrictMode's double mount/unmount cycle using `mountedRef` to prevent async operations from being silently dropped during the second setup phase (see comments in `useInitialDataLoad.ts` L18-L22, `useEventListeners.ts` L15-L17, `useAsyncLock.ts` L11-L14).

3.  **Toast Accumulation**:
    *   Historically, `toasts` array had no limit. Fixed in `useLogToastStore.ts` (L27) by enforcing `MAX_TOASTS = 4` and evicting oldest entries.

4.  **Inline Style Accumulation**:
    *   `useThemeStore.ts` (L88-L93) previously used `cssText +=` which caused unbounded growth. Fixed by using `style.setProperty` and `style.removeProperty`.

5.  **Race Conditions in `checkOnline`**:
    *   Previously used `setTimeout(500)` to release lock, allowing concurrent invocations. Fixed in `useAuthStore.ts` (L194-L197) by releasing lock immediately upon promise settlement.

6.  **Lost Auto-Login Status**:
    *   If `checkOnline` ran before WebView listeners were established, initial online status could be lost. Fixed in `useInitialDataLoad.ts` (L66-L73) by checking `initData.backgroundStatus.online` and setting status proactively.