---
title: android-frontend-hooks-stores
type: module
source_files:
  - android/frontend/src/hooks/useQualityStore.ts
  - android/frontend/src/hooks/useReducedMotion.ts
  - android/frontend/src/hooks/useThemeStore.ts
  - android/frontend/src/hooks/tauriApi.ts
  - android/frontend/src/hooks/useAuthStore.ts
  - android/frontend/src/hooks/useConfigStore.ts
  - android/frontend/src/hooks/useAppInit.ts
  - android/frontend/src/hooks/useAppStore.ts
  - android/frontend/src/hooks/useAsyncLock.ts
  - android/frontend/src/hooks/useAdapterStore.ts
  - android/frontend/src/hooks/useAdaptiveFramePace.ts
  - android/frontend/src/hooks/useAnimationProfile.ts
tags: [Android, Tauri, Zustand, Hooks, State Management, Animation, Network Quality]
---

## Overview

本模块 `android-frontend-hooks-stores` 是 Android 前端应用的核心状态管理与基础设施层。它基于 **Zustand** 构建 Store，并通过 React Hooks 暴露状态与副作用。主要职责包括：

1.  **Tauri API 封装与适配**：定义前端与 Rust 后端通信的契约，处理安卓端对桌面专属功能的降级（如 DHCP、DNS DoH），并封装生物识别验证逻辑。
2.  **领域状态管理**：涵盖认证 (`useAuthStore`)、配置 (`useConfigStore`)、网络质量 (`useQualityStore`)、主题 (`useThemeStore`)、日志 Toast (`useLogToastStore`) 及面板导航 (`useAdapterStore`)。
3.  **应用生命周期编排**：通过 `useAppInit` 组合初始化数据加载、事件监听、心跳检测与全局快捷键。
4.  **性能与动画优化**：提供自适应帧率控制 (`useAdaptiveFramePace`)、设备性能分级 (`useDeviceProfile`)、减少动态效果支持 (`useReducedMotion`) 以及基于 GSAP 的呼吸/脉冲/光晕动画 Hook。

所有状态变更均通过 Zustand Store 驱动，关键副作用（如 DOM Class 同步、本地存储持久化）在 Store 内部或专用 Hook 中处理，确保 UI 与底层状态的一致性。

## Key Components

### 1. Tauri API 封装 (`tauriApi.ts`)

负责定义 IPC 契约并提供安全调用机制。

*   **类型与接口**
    *   `TauriApi` (行 48-141)：定义所有 IPC 方法签名，包括配置、网络、账号、系统设置及事件监听。
    *   `AvoidBadWifiStatus` (行 143-152)：安卓端“避免坏 WiFi”系统设置读写状态结构。
    *   `RestoreWrittenSettingsResult` (行 154-157)：恢复写入设置的结果结构。

*   **核心函数**
    *   `desktopOnly<T>` (行 14-15)：用于安卓端拒绝桌面专属功能，直接 Reject Promise。
    *   `noopListener` (行 18)：返回空函数，用于安卓端不支持的事件监听器占位。
    *   `createEventListener<T>` (行 159-188)：通用事件监听工厂，处理 Tauri `listen` 的异步性与取消订阅，防止内存泄漏。
    *   `withRetry<T>` (行 338-354)：指数退避重试机制，默认最大重试 2 次，基础延迟 500ms，仅对包含 'timeout'/'network'/'fetch'/'connection' 关键字的错误重试。
    *   `tauriApiWithRetry` (行 356-362)：基于 `tauriApi` 的扩展对象，仅对 `saveConfig` 启用重试机制。
    *   `verifyWindowsIdentity` (行 207-244)：生物识别验证门。优先检查 `shouldUseFaceFallback()`，若开启且系统生物识别不可用，则调用 `useFaceDialogStore.openFaceDialog` 进行 2D 人脸验证；否则回退至标准 `biometricAuthenticate`。无论成功失败，均通过内部 `report` 函数统一上报后端 `verify_biometric_identity`。
    *   `openExternal` (行 276-287)：外部链接打开。包含协议白名单校验（http/https）、长度限制（2048）及 URL 解析校验，失败时返回 `false`。
    *   `getAutoLaunch` / `setAutoLaunch` (行 288-289)：实际调用后端的 `get_boot_autostart` 和 `set_boot_autostart`，表明安卓端使用引导自启。

### 2. 领域 Stores (Zustand)

#### `useAuthStore.ts`
*   **职责**：认证领域状态管理，处理登录/注销流程、在线状态检测及后台状态维护。
*   **状态字段**：
    *   `isLoggingIn` / `isLoggingOut`：锁状态，防止重复操作。
    *   `status`：当前在线状态文本与 UI 状态码 (`online`/`offline`/`loading`/`error`)。
    *   `bgStatus`：后台检测结果，包含 `serverAvailable`, `online`, `campusWifi` 等。
*   **关键方法**：
    *   `doLogin` (行 106-164)：登录流程。包含配置保存、带超时包装的 `api.doLogin`、登录后的质量探测节流（60s 内不重复全量探测），以及登录失败时的即时状态复查。
    *   `doLogout` (行 166-194)：注销流程。带超时包装的 `api.doLogout`，成功后执行 `checkOnline` 确认离线状态。
    *   `checkOnline` (行 196-243)：核心状态同步。串行锁 `_checkOnlineLockFlag` 保证并发安全。先检测校园网状态（若不在校园网且需检查则拦截），再查询 Portal 状态。使用 `setStatusStable` 避免无数据变化时的重渲染。
    *   `detectCampusNetwork` (行 29-35) & `buildCampusBgStatusPatch` (行 38-49)：处理校园网检测数据的获取与补丁构建。
    *   `queryPortalStatus` (行 56-65)：查询 Portal 状态，安卓端传空 IP 让后端回退。
    *   `withTimeout` (行 69-78)：Promise race 超时控制，防止登录/注销按钮永久禁用。
    *   `setStatusStable` (行 82-86)：引用稳定性优化，仅在 text/state 变化时更新 Zustand 状态。

#### `useConfigStore.ts`
*   **职责**：配置领域状态管理，处理配置的本地缓存、去抖保存、脏标记管理及密码状态同步。
*   **状态字段**：
    *   `config`：当前配置对象。
    *   `configLoaded`：初始化信号，依赖此字段判断是否可执行后续启动逻辑。
    *   `dirtyFields` (行 20)：本地修改未确认字段的集合。
    *   `dirtyFailureCounts` (行 25)：连续失败计数，用于解决后端旧快照覆盖本地新值的问题。
*   **关键方法**：
    *   `updateConfig` (行 56-88)：合并配置，标记 dirty fields，触发 500ms 去抖保存定时器。
    *   `mergeConfigFromBackend` (行 103-110)：处理后端 `config-changed` 事件，跳过 dirty fields，防止回滚。
    *   `saveConfigDirect` (行 119-167)：直接保存配置。处理 `saveConfigInFlight` 跟踪，防止关闭窗口时丢失待保存数据。包含密码保存状态的同步逻辑及 dirty 字段失败阈值重置（`DIRTY_FAILURE_LIMIT = 3`）。
    *   `flushPendingConfig` (行 186-215)：强制冲刷待保存配置，用于关闭页面或后台化场景。支持等待 in-flight 保存完成。
    *   `hasPendingConfig` (行 179-184)：判断是否有待保存数据（pending 或 in-flight）。

#### `useQualityStore.ts`
*   **职责**：管理网络质量探测状态、DNS DoH 状态、版本更新信息及 UI 弹窗控制。核心难点在于解决“手动刷新”与“后台循环/登录后探测”之间的竞态条件，防止重复全量检测（历史缺陷 P2-F5）。
*   **关键导出与常量**：
    *   `export function getLastQualityResultTime(): number` (行 18)：暴露全局变量 `lastQualityResultTime`，供外部判断是否需要跳过手动刷新。
    *   `export const useQualityStore` (行 43)：Zustand Store 实例。
    *   `let _qualityLockFlag = false` (行 11)：同步锁标志，防止并发执行 `refreshQuality`。
    *   `let lastQualityResultTime = 0` (行 16)：记录最近一次质量结果到达时间戳，用于节流逻辑。
*   **状态字段** (行 22-32)：
    *   `networkQuality`: 当前网络质量对象或 null。
    *   `dnsDohStatus`, `dnsChecking`: DNS 相关状态。
    *   `updateAvailable`, `latestVersion`, `releaseNotes`, `updatePromptOpen`: 版本更新相关状态。
    *   `isRefreshingQuality`: 标识是否正在刷新中。
    *   `refreshRate`: 刷新频率配置。
*   **关键方法实现**：
    *   `refreshQuality` (行 54-74)：
        *   检查 `_qualityLockFlag` 和 `config.enableNetworkQuality`，若为假则直接返回 (行 56-57)。
        *   设置锁并开启加载态 (行 58-59)。
        *   调用 `api.checkNetworkQuality?.()` (行 61)。
        *   若获取到数据 `q`，调用 `get().setNetworkQuality((old) => mergeNetworkQuality(old, q))` (行 64)，确保合并旧数据并更新时间戳。
        *   异常处理仅在 DEV 模式下打印错误 (行 66-68)。
        *   `finally` 块中通过 `setTimeout(..., 500)` (行 69-72) 延迟解锁和重置加载态，避免频繁触发。
    *   `setNetworkQuality` (行 78-82)：
        *   支持函数式更新。
        *   **关键逻辑**：当新值 `next !== null` 时，强制更新 `lastQualityResultTime = Date.now()` (行 80)。这确保了无论是后台事件还是手动刷新，只要收到有效结果，都会重置节流窗口。
    *   其他 Setters (行 83-88)：简单的 Zustand `set` 操作，包括 `setDnsDohStatus`, `setDnsChecking`, `setUpdateAvailable`, `setLatestVersion`, `setReleaseNotes`, `setUpdatePromptOpen`。

#### `useThemeStore.ts`
*   **职责**：管理主题名称、浅色模式、自定义颜色、玻璃特效档位。负责将状态同步到 DOM Class 和 CSS 变量，并持久化到存储。
*   **关键导入与常量**：
    *   `VALID_THEMES`, `VALID_GLASS_LEVELS` (行 4)：来自 `@/settings/constants` 的白名单。
    *   `hexToHsl` (行 7)：颜色转换工具。
    *   `safeStorage` (行 6)：安全存储封装。
*   **状态字段** (行 9-13)：
    *   `themeName`: 主题名称，默认 `'default'` (行 22)。
    *   `isLightMode`: 浅色模式开关，初始值从 `safeStorage.get('campus-light-mode') === '1'` 读取 (行 23)。
    *   `customThemeColor`: 自定义颜色，默认 `'#6366f1'` (行 24)。
    *   `glassLevel`: 玻璃档位，初始值从存储读取并校验是否在 `VALID_GLASS_LEVELS` 中，否则回退 `'standard'` (行 25-28)。
*   **关键方法实现**：
    *   `initTheme(cfg)` (行 34-54)：
        *   恢复保存的主题名 (行 35-36)。
        *   处理浅色模式逻辑：
            *   优先读取存储 (`campus-light-mode`) 为 `'1'` 或 `'0'` (行 37-41)。
            *   若无存储记录，根据传入 `cfg.themeMode` 决定：`light` -> true, `dark` -> false, `system` -> 匹配 `window.matchMedia('(prefers-color-scheme: light)').matches` (行 42-51)。
            *   在确定最终值后，写回存储 (行 44, 47, 51)。
        *   应用自定义颜色 (行 53)。
    *   `setCustomThemeColor` (行 56)：简单 setter。
*   **全局副作用订阅** (行 60-97)：
    *   使用 `useThemeStore.subscribe` 监听状态变化并同步 DOM。
    *   **Glass Level 同步** (行 62-65)：变更时设置 `document.documentElement.setAttribute('data-glass', ...)` 并持久化存储。
    *   **Light Mode 同步** (行 66-73)：变更时 toggle `.dark` class，并根据 `isLightMode` 添加/移除 `data-light="1"` attribute。
    *   **Theme Name & Color 同步** (行 74-97)：
        *   先移除所有预设主题 class (`theme-vibrant`, `theme-forest` 等) (行 76-77)。
        *   若为 `custom` 主题 (行 78)：
            *   添加 `.theme-custom` (行 79)。
            *   转换 hex 为 HSL (行 81)。
            *   **关键修复**：使用 `root.style.setProperty` 覆盖 CSS 变量 (`--primary`, `--ring`, `--accent`, `--accent-foreground`)，避免历史缺陷中 `cssText +=` 导致的 inline style 无界增长 (行 84-87)。
            *   注意 `--accent` 和 `--accent-foreground` 的亮度计算依赖于 `state.isLightMode` (行 86-87)。
        *   若非 `custom` (行 88)：
            *   移除自定义 CSS 变量 (行 89-92)。
            *   若主题非 `default`，添加对应的 `theme-${name}` class (行 93-95)。

#### `useAdapterStore.ts`
*   **职责**：面板导航状态管理。安卓端网卡数据链已废弃，此 Store 仅保留 `activePanel` 状态供导航组件使用。
*   **关键实现**：
    *   `useAdapterStore` (行 11-14)：Zustand store，默认面板为 `'dashboard'`，提供 `setActivePanel` 方法。

#### `useLogToastStore.ts`
*   **职责**：日志与 Toast 通知的状态管理。
*   **关键限制**：
    *   `MAX_LOG_ENTRIES`：日志条目上限。
    *   `MAX_TOASTS = 4`：Toast 队列上限，溢出时淘汰最旧者。
*   **关键逻辑**：
    *   `addLog`：支持相同消息的时间戳更新与计数累加。
    *   `addToast` / `addToastWithAction`：支持标题去重。非前台时 `addToast` 静默丢弃（`addToastWithAction` 例外，因含操作入口）。
    *   `cleanupToasts`：清理所有定时器。

### 3. 应用初始化与生命周期 Hooks

#### `useAppInit.ts`
*   **职责**：应用初始化编排器，组合加载初始数据、事件监听、心跳检测和全局快捷键。
*   **关键实现**：
    *   `useAppInit` (行 6-11)：顺序调用 `useEventListeners`, `useInitialDataLoad`, `useHeartbeat`, `useGlobalShortcut`。

#### `useInitialDataLoad.ts`
*   **职责**：应用启动时的初始数据加载与状态初始化。
*   **关键流程**：
    *   调用 `api.getInitData` 获取初始配置、背景状态、账号列表等。
    *   初始化 `useConfigStore` (配置、密码状态)、`useThemeStore` (主题)、`useAdapterStore` (默认面板)、`useAuthStore` (后台状态、在线状态)。
    *   若 `initData.backgroundStatus.online` 为 true，直接设置在线状态，避免竞态导致的状态灰显。
    *   最终设置 `configLoaded: true` 信号。
    *   失败时降级为默认配置并记录错误日志。

#### `useEventListeners.ts`
*   **职责**：集中注册所有来自后端的 Tauri 事件监听器，并处理应用生命周期相关的副作用（如关闭前保存配置）。
*   **关键监听器**：
    *   `onBackgroundCheckResult` (行 82-114)：更新 `bgStatus` 和 `status`，包含低频去重逻辑。
    *   `onAutoLoginResult` (行 117-128)：自动登录结果处理，成功提示，失败记录日志并触发 `checkOnline`。
    *   `onLoginLog` (行 134-140)：登录日志记录。
    *   `onAutoExitCountdown` / `onCampusExitCountdown` (行 142-196)：自动退出倒计时处理，显示带“取消”操作的 Toast。
    *   `onNetworkQualityResult` (行 198-204)：网络质量更新，触发“质量差”告警 Toast。
    *   `onUpdateAvailable` (行 206-218)：新版本发现，更新 Quality Store 并记录日志。
    *   `onConfigChanged` (行 220-241)：配置变更同步，调用 `mergeConfigFromBackend`，并同步激活账号和账号列表。
    *   `onUpdateUnverified` (行 244-255)：更新源不可用警示。
*   **生命周期处理**：
    *   `getCurrentWindow().onCloseRequested` (行 41-66)：关闭前检查 `hasPendingConfig`，若有则 `await flushPendingConfig()` (限时 2s)，防止数据丢失。
    *   `visibilitychange` / `pagehide` (行 76-80)：安卓进程杀除兜底，后台化时冲刷配置。

#### `useHeartbeat.ts`
*   **职责**：渲染链心跳检测。
*   **关键实现**：
    *   每 5s 调用 `api.renderHeartbeat`。若 `document.hidden` 或渲染链失活 (`isRenderLoopAlive` 为 false) 则跳过，允许后端检测到心跳丢失并重载 WebView。

#### `useGlobalShortcut.ts`
*   **职责**：全局快捷键监听。
*   **关键实现**：
    *   `Ctrl+Shift+C`：触发 `api.cancelAutoExit`。忽略可编辑元素（Input/Textarea）以防止误触。

### 4. 性能、动画与设备 Profile Hooks

#### `useAdaptiveFramePace.ts`
*   **职责**：智能帧率控制，基于交互状态动态调整 GSAP ticker 帧率，平衡性能与流畅度。
*   **关键常量**：
    *   `ACTIVE_WINDOW_MS = 2000` (行 16)：交互活跃窗口时长。
    *   `PACE_POLL_MS = 250` (行 18)：静止回落轮询间隔。
*   **关键实现**：
    *   `markInteraction` (行 26-29)：标记交互发生，推高活跃窗口并立即应用帧率。
    *   `useAdaptiveFramePace` (行 51-74)：挂载于应用根部。监听指针/触摸/键盘事件，使用 `setInterval` 轮询判断是否进入空闲态，从而切换 `idleFps` (30) 和 `activeFps` (60)。页面隐藏时帧率归零。

#### `useAnimationProfile.ts`
*   **职责**：提供统一的动画配置档案。安卓端已移除多档位判定，恒定为最高档。
*   **关键实现**：
    *   `HIGH_PROFILE` (行 31-49)：硬编码的最高性能配置，包含弹簧刚度、阻尼、是否启用 GPU 合成等。
    *   `useAnimationProfile` (行 51-57)：根据 `useQualityStore` 中的刷新率动态生成 `easing` 配置，但 `tier` 始终为 `'high'`。

#### `useDeviceProfile.ts`
*   **职责**：设备性能分级 Profile。安卓端已折叠为恒旗舰档。
*   **关键实现**：
    *   `useDeviceProfile` (行 25-27)：直接返回 `HIGH_PROFILE`，包含 `idleFps: 30`, `activeFps: 60`。

#### `useReducedMotion.ts`
*   **职责**：监听系统“减少动态效果”偏好，供 JS 动画逻辑参考。弥补 CSS `@media` 无法控制 JS 逐帧动画的不足。
*   **关键实现**：
    *   `const QUERY = '(prefers-reduced-motion: reduce)'` (行 3)：查询字符串常量。
    *   `export function useReducedMotion(): boolean` (行 12)：React Hook。
    *   **初始化** (行 13-15)：使用惰性初始化 `useState(() => ...)`，在服务端渲染安全地检查 `window.matchMedia(QUERY).matches`。
    *   **副作用监听** (行 17-27)：
        *   创建 `mq` 对象 (行 18)。
        *   定义 `onChange` 回调更新状态 (行 19)。
        *   **兼容性处理** (行 21-26)：优先使用标准的 `addEventListener('change', ...)` 和 `removeEventListener`；若不支持（旧 WebView），降级使用 `addListener` 和 `removeListener`。
        *   清理函数正确移除对应的事件监听器。

#### `usePageIdle.ts`
*   **职责**：页面空闲状态检测与动画活跃性上下文提供。
*   **关键实现**：
    *   `usePageIdle` (行 3-49)：基于 `lastActiveRef` 和 500ms 轮询判定 2s 空闲。更新 body class `anim-idle`。
    *   `usePageVisible` / `useWindowFocused`：分别监听可见性和焦点。
    *   `AnimationActiveProvider` (行 85-93)：Context Provider，聚合 `isVisible && isFocused && !isIdle` 作为全局动画活跃性标志，避免大量重复监听器注册。
    *   `useAnimationActive` (行 95-97)：消费 Context。

#### 动画 Hooks
*   **`useGlowAnimation.ts`**：呼吸光晕动画 Hook，基于 GSAP。创建 yoyo 循环的 scale 和 opacity 动画。受 `useAnimationActive` 控制暂停/恢复。
*   **`useBreatheAnimation.ts`**：通用呼吸动画 Hook，支持透明度、缩放、旋转的多属性混合动画。GSAP tween 配置，受 `useAnimationActive` 控制。
*   **`usePulseAnimation.ts`**：脉冲/心跳动画 Hook，支持三种预设类型 (`heartbeat`, `statusPulse`, `loadingPulse`)。GSAP Timeline 构建，受 `useAnimationActive` 控制。

### 5. 表单因子与其他 Tools

#### `useFormFactor.ts`
*   **职责**：设备形态（手机/平板）与方向判定。
*   **关键实现**：
    *   `useFormFactor` (行 38-40)：基于 `Math.min(innerWidth, innerHeight) >= 600` 判定平板。
    *   `useOrientation` (行 43-45)：基于宽高比判定横竖屏。
    *   `useViewportMeasure` (行 24-36)：通用视口度量 Hook，监听 `resize` 和 `orientationchange`。

#### `useAsyncLock.ts`
*   **职责**：React Hook，提供防抖/互斥锁机制，防止异步函数被快速重复点击触发。
*   **关键实现**：
    *   `useAsyncLock<T>` (行 3-26)：接收异步函数 `fn` 和冷却时间 `cooldownMs`。返回 `[isRunning, execute]`。
    *   使用 `lockRef` 和 `mountedRef` 处理 React StrictMode 下的双重调用问题，确保锁正确释放。

#### `useAppStore.ts`
*   **职责**：兼容性导出文件，仅 re-export `useAppInit` 及相关配置工具函数，无业务逻辑。

## Architecture

本模块采用分层架构，以 Zustand Store 为核心状态源，Hooks 作为状态与副作用的桥梁，Tauri API 作为底层通信通道。

```text
┌──────────────────────────────────────────────────────────────────────┐
│                         Frontend Layer                               │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────┐ │
│  │ useAppInit  │──│useEventList- │──│ useHeartbeat │──│Shortcuts │ │
│  └──────┬──────┘  └──────┬───────┘  └──────┬───────┘  └──────────┘ │
│         │                │                 │                        │
│         ▼                ▼                 ▼                        │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────┐               │
│  │ InitialData │  │ Backend Events│  │ Render Loop  │               │
│  │ Load        │  │ (Config, Auth│  │ Heartbeat    │               │
│  └──────┬──────┘  │ ,Quality...)  │  └──────────────┘               │
│         │         └──────┬───────┘                                 │
│         │                │                                         │
│         ▼                ▼                                         │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                     Zustand Stores                           │  │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐   │  │
│  │  │ Config   │ │  Auth    │ │ Quality  │ │   Theme      │   │  │
│  │  │ Store    │ │  Store   │ │  Store   │ │   Store      │   │  │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────────┘   │  │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐   │  │
│  │  │ Adapter  │ │  Log     │ │ App Init │ │  Page Idle   │   │  │
│  │  │  Store   │ │  Toast   │ │  Store   │ │  Context     │   │  │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────────┘   │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                     │
│                              ▼                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                   Side Effects & DOM Sync                    │  │
│  │  • Theme DOM Class/CSS Var Sync (useThemeStore.subscribe)    │  │
│  │  • SafeStorage Persistence                                   │  │
│  │  • GSAP Animation Control (useAnimationActive)               │  │
│  └──────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────────────┐
│                      Tauri API Bridge                                │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │  tauriApi.ts                                                 │   │
│  │  • Desktop-only Rejection (desktopOnly)                      │   │
│  │  • Retry Logic (withRetry)                                   │   │
│  │  • Event Listener Factory (createEventListener)              │   │
│  │  • Biometric Verification Gate (verifyWindowsIdentity)       │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                              │                                     │
│                              ▼                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                      Rust Backend                            │   │
│  │  • Config Save/Load                                          │   │
│  │  • Network Quality Check                                     │   │
│  │  • Auth Portal/Campus                                        │   │
│  │  • System Settings                                           │   │
│  └──────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### 1. 初始化链路
1.  `useAppInit` 调用 `useInitialDataLoad`。
2.  `useInitialDataLoad` 调用 `api.getInitData` 获取初始配置、背景状态、账号列表等。
3.  初始化各 Store：
    *   `useConfigStore`：加载配置、密码状态。
    *   `useThemeStore`：加载主题、浅色模式、玻璃档位。
    *   `useAdapterStore`：设置默认面板 `'dashboard'`。
    *   `useAuthStore`：加载后台状态、在线状态。若 `initData.backgroundStatus.online` 为 true，直接设置在线状态。
4.  设置 `configLoaded: true` 信号。
5.  `useAppInit` 调用 `useEventListeners`，注册后端事件监听器。
6.  `useAppInit` 调用 `useHeartbeat`，启动心跳检测。
7.  `useAppInit` 调用 `useGlobalShortcut`，注册全局快捷键。

### 2. 配置保存链路
1.  UI 组件调用 `useConfigStore.updateConfig`。
2.  `updateConfig` 合并配置，标记 `dirtyFields`，触发 500ms 去抖保存定时器。
3.  定时器到期后调用 `useConfigStore.saveConfigDirect`。
4.  `saveConfigDirect` 调用 `api.saveConfig` (带重试机制)。
5.  若保存失败，增加 `dirtyFailureCounts`。达到 `DIRTY_FAILURE_LIMIT` (3次) 后放弃该字段的脏标记，允许后端回传的旧值覆盖。
6.  **兜底保存**：
    *   `useEventListeners` 中的 `onCloseRequested` (行 41-66) 或 `visibilitychange`/`pagehide` (行 76-80) 触发。
    *   检查 `hasPendingConfig`，若有则调用 `flushPendingConfig` (限时 2s) 强制保存。

### 3. 认证与状态链路
1.  用户点击登录 -> `useAuthStore.doLogin`。
2.  `doLogin` 调用 `api.doLogin` (带超时保护)。
3.  登录成功 -> 更新 `status` 和 `bgStatus`，并可能触发 `api.checkNetworkQuality` (节流 60s)。
4.  登录失败 -> 即时状态复查。
5.  **后端推送**：
    *   `background-check-result` -> `useEventListeners` 捕获 -> 更新 `useAuthStore.setBgStatus` 和 `useAuthStore.setStatus`。
    *   `config-changed` -> `useEventListeners` 捕获 -> `useConfigStore.mergeConfigFromBackend` -> 更新 `useConfigStore.config` 和 `activeAccount`。
    *   `onNetworkQualityResult` -> 更新 `useQualityStore`，若质量差则触发 Toast 告警。
    *   `onUpdateAvailable` -> 更新 `useQualityStore` 版本信息。

### 4. 动画与性能链路
1.  `usePageIdle` 监听交互事件，更新 `lastActiveAt`，判定页面空闲状态 (2s 无交互)。
2.  `usePageIdle` 提供 `AnimationActiveContext`，聚合 `isVisible && isFocused && !isIdle`。
3.  所有动画 Hooks (`useGlowAnimation`, `useBreatheAnimation`, `usePulseAnimation`) 消费此 Context，决定是否暂停/恢复 GSAP 动画。
4.  `useAdaptiveFramePace` 监听交互事件，动态调整 GSAP ticker 帧率 (空闲 30fps，活跃 60fps)。
5.  `useDeviceProfile` 和 `useAnimationProfile` 提供静态的性能参数 (`HIGH_PROFILE`)，供 UI 渲染决策参考。

## Connections

*   [[android-frontend-ui]]：UI 组件消费各 Store 状态，触发 Actions。
*   [[android-backend-ipc]]：Tauri API 对接的后端服务，处理配置、认证、网络质量等业务逻辑。
*   [[android-settings]]：主题、配置等设置的持久化存储与常量定义。
*   [[android-gsap-animation]]：GSAP 动画库的使用与性能优化策略。

## Known Issues

1.  **CSS 内存泄漏修复 (历史)**：`useThemeStore` 的全局订阅中，明确指出了历史缺陷：之前使用 `cssText +=` 导致 inline style 累积。现改为 `style.setProperty` 覆盖同名变量，并在切换回非自定义主题时使用 `style.removeProperty` 清理 (行 84-92)。这是防止 DOM 节点 style 属性无限增长的必要措施。
2.  **竞态与节流 (P2-F5)**：`useQualityStore` 中的 `lastQualityResultTime` 机制是关键。任何来源（后台 loop、手动 refresh、登录后探测）获得的有效结果都会更新时间戳。`refreshQuality` 虽有自己的 `_qualityLockFlag` 防止单次执行重叠，但更高层的逻辑应利用 `getLastQualityResultTime()` 判断是否需要发起本次请求，从而避免与后台巡检重复。`refreshQuality` 完成后有 500ms 的 `setTimeout` 缓冲期才解锁 (行 69-72)，这是一种防御性编程，防止高频点击导致的瞬间多次请求。
3.  **WebView 兼容性**：`useReducedMotion` 显式处理了 `matchMedia` 在不同 WebView 版本中的 API 差异（`addEventListener` vs `addListener`），确保在老旧环境下的可用性 (行 21-26)。
4.  **桌面专属功能的安卓降级**：`tauriApi.ts` 中大量方法（如 `exportConfig`, `getAdapters`, `dhcpRenewAll` 等）被替换为 `desktopOnly`，直接 Reject Promise。这确保了安卓端代码不会意外调用桌面特有 API 导致崩溃，同时也意味着后端对应命令在安卓端不会被执行。
5.  **生物识别验证门的回退逻辑**：`verifyWindowsIdentity` 实现了复杂的回退策略：如果 `shouldUseFaceFallback` 为真且系统 Biometric 不可用，则尝试应用内 2D 人脸验证；若人脸验证也失败或异常，则回退到系统 BiometricPrompt。所有结果均统一上报后端，确保后端视角的一致性。
6.  **配置保存的脏标记与失败重试**：`useConfigStore` 引入了 `dirtyFields` 和 `dirtyFailureCounts`。当保存失败时，脏标记不会立即清除，而是累计失败次数。达到 `DIRTY_FAILURE_LIMIT` (3次) 后放弃该字段的脏标记，允许后端回传的旧值覆盖，防止 UI 与后端永久脱节。
7.  **初始化竞态处理**：`useInitialDataLoad` 和 `useEventListeners` 都使用了 `mountedRef` 来防止 React StrictMode 下二次 setup 导致的异步回调执行在已卸载组件上，或者处理 cleanup/setup 时序问题。