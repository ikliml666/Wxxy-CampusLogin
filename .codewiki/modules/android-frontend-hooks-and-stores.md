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
  - android/frontend/src/hooks/useEventListeners.ts
  - android/frontend/src/hooks/useFormFactor.ts
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
1.  **状态管理**：基于 Zustand 的多个 Store（配置、认证、主题、日志、质量等），处理本地持久化、后端同步及脏数据冲突解决。
2.  **IPC 适配**：封装 Tauri API，拦截桌面专属功能，实现安卓特有的生物识别回退和事件监听。
3.  **动画与性能**：动态帧率控制 (`useAdaptiveFramePace`)、设备性能分级 (`useDeviceProfile`) 及 GSAP 动画实例管理。
4.  **初始化编排**：协调启动流程，确保数据加载、事件订阅和心跳保活的正确顺序。

## Key Components

### `tauriApi.ts`: Tauri API 封装与 IPC 适配
作为 Android 前端与 Rust 后端通信的唯一桥梁。

*   **常量与工具函数**
    *   `desktopOnly<T>` (行 14-15)：返回 rejected Promise，用于拦截所有桌面专属功能调用（如 `exportConfig`, `getAdapters`, `minimizeWindow`）。
    *   `noopListener` (行 18)：返回空操作清理函数，用于安卓端不存在的适配器事件订阅。
    *   `isRetryableError` (行 326-336)：判断错误是否包含 'timeout', 'network', 'fetch', 'connection' 关键词以决定是否重试。
    *   `withRetry` (行 338-354)：指数退避重试逻辑，默认最大 2 次重试，基础延迟 500ms。

*   **类型定义**
    *   `TauriApi` 接口 (行 48-141)：定义了所有 IPC 方法的签名。
    *   `AvoidBadWifiStatus` (行 143-152)：安卓特有系统设置读写状态结构。
    *   `RestoreWrittenSettingsResult` (行 154-157)：安卓特有系统设置读写结果结构。

*   **API 实现细节**
    *   `createEventListener` (行 159-188)：封装 Tauri `listen` API，处理异步注册过程中的取消逻辑（`cancelled` 标志位），确保组件卸载时能正确清理未完成的监听器。
    *   `verifyWindowsIdentity` (行 207-244)：**关键业务逻辑**。优先检查 `shouldUseFaceFallback()`；若启用人脸回退且系统生物识别不可用，则调用 `useFaceDialogStore.openFaceDialog('verify')`；否则走标准系统 `biometricAuthenticate`。最终通过 `invoke('verify_biometric_identity', ...)` 上报结果，将前端异常归一化为 `{success: false}`。
    *   `openExternal` (行 276-287)：URL 安全校验（协议头、长度 <2048、解析合法性），使用 `openerOpenUrl` 打开。
    *   `setAutoLaunch` (行 289)：包装 `invoke` 结果，强制返回 `{ success: true }` 类型的 `AutoLaunchResult`。

*   **导出对象**
    *   `tauriApi` (行 190-324)：原始 API 映射表。
    *   `tauriApiWithRetry` (行 356-361)：仅对 `saveConfig` 应用重试机制；`checkPortalStatus` 等高频调用方法明确排除重试以防流量放大。

### `useConfigStore.ts`: 配置持久化与脏数据管理
管理全局配置 `Config`，处理本地修改与后端同步的冲突。

*   **状态字段**
    *   `config`: 当前配置对象。
    *   `configLoaded` (行 32)：初始数据加载完成信号。
    *   `dirtyFields` (行 20)：`Set<string>`，记录本地已改但未获后端确认的字段。
    *   `dirtyFailureCounts` (行 24-25)：`Map<string, number>`，连续失败计数，达 `DIRTY_FAILURE_LIMIT` (3) 后放弃脏标记。
    *   `saveConfigInFlight` (行 17)：当前正在进行的保存 Promise 引用。

*   **操作方法**
    *   `updateConfig` / `updateConfigLocal` (行 56-99)：更新本地状态，标记 `dirtyFields`，启动 500ms debounce 定时器触发 `saveConfigDirect`。
    *   `mergeConfigFromBackend` (行 103-110)：合并后端推送的配置。**核心逻辑**：跳过 `dirtyFields` 中的键，防止后端旧快照覆盖本地未落盘的新值。
    *   `saveConfigDirect` (行 119-167)：执行实际 IPC 调用。成功则清除 dirty 标记并置 `passwordSaved=true`；失败则增加失败计数，超限则重置 dirty 标记允许重新同步。
    *   `hasPendingConfig` (行 179-184)：检查 `saveConfigPending` 或 `saveConfigInFlight`，用于关闭前拦截。
    *   `flushPendingConfig` (行 186-214)：立即执行待存配置。处理 `PASSWORD_MASK` 删除逻辑，合并 pending 与 in-flight promise 等待。

### `useAuthStore.ts`: 认证流程与在线状态检测
管理登录/注销、在线状态探测及后台状态维护。

*   **状态字段**
    *   `isLoggingIn` / `isLoggingOut`：防重复提交锁。
    *   `status`: `{ text, state }`，UI 显示的状态文本与颜色状态。
    *   `bgStatus`：后台检测结果聚合。

*   **操作方法**
    *   `doLogin` (行 106-164)：保存配置 -> 调用 `api.doLogin` (带 60s 超时 `withTimeout`)。成功后：若开启质量检测且距上次检测 > 60s (`QUALITY_MANUAL_THROTTLE_MS`)，触发手动质量探测。**修复历史缺陷**：登录成功**不**立即调用 `checkOnline`（避免 Portal 会话未传播导致状态闪烁为 offline）；仅失败时复查。
    *   `doLogout` (行 166-194)：调用 `api.doLogout`，成功后调用 `checkOnline` 刷新状态。
    *   `checkOnline` (行 196-243)：
        *   使用 `_checkOnlineLockFlag` 串行化调用，防止并发 invoke。
        *   若启用校园网检查 (`enableNetworkNameCheck`)，先调 `detectCampusNetwork`。若非校园网且非离线状态，发送警告日志并直接返回 offline。
        *   调用 `queryPortalStatus('')`（安卓端 adapterIp 恒为空串，后端回退默认路由）。
        *   使用 `setStatusStable` 比较 text/state 是否变化，避免无效重渲染。

### `useLogToastStore.ts`: 日志与通知管理
管理日志列表 `logs` 和 Toast 通知队列 `toasts`。

*   **状态字段**
    *   `logs`: 日志列表。
    *   `toasts`: Toast 通知队列。
    *   `toastTimers`：Toast 自动消失定时器管理。

*   **操作方法**
    *   `addLog`：日志去重。最后一条内容/类型相同时，仅更新时间戳并增加 `count` (行 37-41)。
    *   `addToast`：Toast 去重。同标题 (`title`) 重复时跳过新增 (行 61, 90)。在非前台 (`document.visibilityState !== 'visible'`) 时静默丢弃（除非是带操作的 Toast）。

### `useQualityStore.ts`: 网络质量与版本更新状态
管理网络质量、DNS DoH 状态、版本更新信息。

*   **状态字段**
    *   `networkQuality`: 网络质量数据。
    *   `dnsDohStatus`: DNS DoH 状态。
    *   `dnsChecking`: DNS 检测中标志。
    *   `isRefreshingQuality`: 正在刷新质量标志。
    *   `updateAvailable`: 是否有可用更新。
    *   `latestVersion`: 最新版本号。
    *   `releaseNotes`: 更新说明。
    *   `updatePromptOpen`: 更新提示框是否打开。
    *   `refreshRate`: 刷新频率。

*   **操作方法**
    *   `refreshQuality` (行 54-73)：
        1.  检查锁 `_qualityLockFlag` (行 56) 和配置开关 `config.enableNetworkQuality` (行 57)，任一不满足则直接返回。
        2.  加锁 (行 58)，设置 `isRefreshingQuality: true` (行 59)。
        3.  调用 `api.checkNetworkQuality?.()` (行 61)。
        4.  若返回结果 `q`，通过 `get().setNetworkQuality((old) => mergeNetworkQuality(old, q))` (行 64) 合并旧数据与新数据。
        5.  异常处理：仅在 DEV 模式下打印错误 (行 67)。
        6.  释放锁：使用 `setTimeout` (行 69) 延迟 500ms 后重置 `_qualityLockFlag` (行 70) 并清除刷新状态 (行 71)。
    *   `setNetworkQuality` (行 78-82)：支持函数式更新 `(prev) => ...` (行 79)。当新结果非 null 时更新 `lastQualityResultTime` 为 `Date.now()` (行 80)。
    *   `getLastQualityResultTime` (行 18-20)：供外部判断是否需跳过手动全量探测。

### `useThemeStore.ts`: 主题管理与 DOM 同步
管理主题名称、浅色模式、自定义颜色、玻璃特效档位，并同步到 DOM。

*   **状态字段**
    *   `themeName` (行 22)：默认 `'default'`。
    *   `isLightMode` (行 23)：浅色模式标志。
    *   `customThemeColor` (行 24)：默认 `'#6366f1'`。
    *   `glassLevel` (行 25)：玻璃特效档位。

*   **操作方法**
    *   `initTheme` (行 34-53)：
        *   从 `safeStorage` 读取 `campus-theme`，校验是否在 `VALID_THEMES` 中 (行 35-36)。
        *   **浅色模式优先级**：读取 `safeStorage` 的 `campus-light-mode`；若为空，回退到传入的 `cfg.themeMode`（`'light'`->true, `'dark'`->false, `'system'`->`window.matchMedia`）。
        *   若 `cfg.customThemeColor` 存在则更新。
    *   `subscribe` (行 60-97)：监听状态变化并同步 DOM。
        *   **玻璃档位**：设置 `document.documentElement.setAttribute('data-glass', ...)` (行 63) 并持久化存储 (行 64)。
        *   **浅色模式切换**：切换 `dark` class (行 67)；若为浅色，添加 `data-light="1"` (行 69)；否则移除 (行 71)。
        *   **主题与颜色切换**：
            *   清理：移除所有预设主题类名 `['theme-vibrant', ..., 'theme-custom']` (行 76-77)。
            *   **自定义主题处理**：添加 `theme-custom` 类 (行 79)；转换 HEX 为 HSL (行 81)；使用 `style.setProperty` 覆盖而非 `cssText +=`，防止内存泄漏/样式累积 (行 84-87)。具体映射：
                *   `--primary`, `--ring`: HSL(h, s%, l%) (行 84-85)。
                *   `--accent`: HSL(h, s%, min(s, 33)%) + 亮度随 `isLightMode` 变化 (行 86)。
                *   `--accent-foreground`: HSL(h, s%, 亮度随 `isLightMode` 变化) (行 87)。
            *   移除自定义 CSS 变量 (行 89-92)。
            *   **预设主题处理**：若非 `default`，添加对应类名 `theme-${state.themeName}` (行 93-95)。

### `useReducedMotion.ts`: 减少动态效果偏好检测
检测系统“减少动态效果”偏好，用于 JS 动画控制。

*   **关键实现**
    *   `QUERY` (行 3)：`'(prefers-reduced-motion: reduce)'`。
    *   **状态初始化**：惰性初始化 `useState(() => ...)` (行 13-15)，环境检查 `typeof window !== 'undefined'` (行 14)。
    *   **监听器注册**：`useEffect` 依赖空数组 `[]` (行 27)。创建 `mq` 对象 (行 18)。兼容性处理：优先使用标准 `addEventListener('change', onChange)` (行 21-23)；若不支持（旧 WebView），降级使用 `addListener`/`removeListener` (行 25-26)。

### `useAdaptiveFramePace.ts`: 动态帧率控制
基于 GSAP ticker 动态调整帧率，平衡功耗与流畅度。

*   **关键参数**
    *   `ACTIVE_WINDOW_MS = 2000`：交互活跃窗口。
    *   `PACE_POLL_MS = 250`：静止回落轮询间隔。

*   **操作方法**
    *   `markInteraction()`：用户交互时推高活跃窗口并立即应用帧率。
    *   `applyPace()`：根据 `currentPace()` 设置 `gsap.ticker.fps()`。后台时帧率为 0。
    *   **修复历史缺陷**：原实现使用常驻 rAF 递归轮询导致页面永远无法休眠。现改为 `setInterval` 轮询档位变化，交互事件即时提帧。

### `useAnimationProfile.ts`: 动画参数配置
提供统一的动画配置对象。安卓端**恒最高档** (`HIGH_PROFILE`)，删除了 standard/economy 分档。`refreshRate` 驱动 easing 曲线计算。

### `useDeviceProfile.ts`: 设备性能分级
提供设备性能画像。安卓端折叠为单一 `HIGH_PROFILE`，`idleFps=30`, `activeFps=60`。接口形状保留以兼容桌面代码。

### `usePageIdle.ts`: 空闲检测与 Provider
检测页面是否处于“空闲”状态（无交互超过 2s），并提供全局 Context `AnimationActiveContext`。组合逻辑：`isVisible && isFocused && !isIdle` 决定动画是否激活。Provider：`AnimationActiveProvider` 在应用顶层挂载。消费 Hook：`useAnimationActive()` 供各动画 Hook 暂停/恢复使用。

### 动画专用 Hooks
*   `useBreatheAnimation`：呼吸效果 (opacity/scale/rotation 循环)。
*   `useGlowAnimation`：发光效果 (scale/opacity 循环)。
*   `usePulseAnimation`：脉冲效果，支持 `heartbeat`, `statusPulse`, `loadingPulse` 三种模式，使用 Timeline 编排多步动画。
*   **共性**：均使用 `useRef` 缓存 GSAP Tween/Timeline 实例，通过 `useAnimationActive()` 控制 `resume()/pause()`。

### `useAdapterStore.ts`: 面板导航状态
仅保留 `activePanel` 状态，供 DockNav 等导航组件使用。安卓端无真实网卡数据，此 store 不再关联网络适配器列表。

### `useAppInit.ts`: 应用启动编排
组合所有启动必需的 Hooks。调用链：`useEventListeners` -> `useInitialDataLoad` -> `useHeartbeat` -> `useGlobalShortcut`。

### `useInitialDataLoad.ts`: 首次数据加载
*   **流程**：
    1.  调用 `api.getInitData()`。
    2.  合并 `DEFAULT_CONFIG` 与后端配置，处理 `PASSWORD_MASK` 同步 `passwordSaved`。
    3.  初始化主题 `useThemeStore.initTheme`。
    4.  恢复上次停留的面板 `activePanel` (排除禁用的 quality 面板)。
    5.  设置初始后台状态 `bgStatus`，若后端已知在线则直接设置 `status` 为 online (解决竞态丢失问题)。
    6.  设置账号列表与激活账号。
    7.  触发首次 `checkOnline(cfg)`。
    8.  设置 `refreshRate`。
    9.  标记 `configLoaded = true`。
*   **降级处理**：若 `getInitData` 失败，打印错误日志，使用默认配置并标记 `configLoaded = true`。

### `useEventListeners.ts`: 全局事件总线订阅
*   **关键订阅**：
    *   `onBackgroundCheckResult`：更新 `bgStatus` 和 `status`。含 1s 节流防止风暴。
    *   `onAutoLoginResult`：自动登录成功/失败的通知与日志。
    *   `onLoginLog`：通用登录日志。
    *   `onAutoExitCountdown` / `onCampusExitCountdown`：倒计时预警，显示带“取消退出”操作的 Toast。
    *   `onNetworkQualityResult`：更新网络质量，触发“质量差”告警 Toast。
    *   `onUpdateAvailable`：更新可用通知（不自动弹窗，仅更新 Store）。
    *   `onConfigChanged`：后端配置变更。调用 `mergeConfigFromBackend` 并同步激活账号及刷新账号列表。
    *   `onUpdateUnverified`：更新源不可用时警示下载。
*   **关闭保护**：
    *   监听 `getCurrentWindow().onCloseRequested`。若有待存配置 (`hasPendingConfig`)，阻止默认关闭，调用 `flushPendingConfig` 并 await (限时 2s)，完成后手动 `close()`。
    *   监听 `visibilitychange` (hidden) 和 `pagehide`：后台切换时也冲刷待存配置。

### `useAsyncLock.ts`: 异步操作互斥锁
防止同一函数在短时间内被重复调用。`useState` 控制按钮 loading 态，`useRef` 存储锁状态。支持 `cooldownMs` 冷却时间。处理 StrictMode 下的双重渲染问题。

### `useFormFactor.ts`: 设备形态判定
区分手机 (`phone`) 与平板 (`tablet`)。短边 (smallest-width) >= 600px 视为平板。响应 `resize` 和 `orientationchange` 事件实时重算。

### `useGlobalShortcut.ts`: 全局快捷键
监听 `Ctrl+Shift+C` 取消自动退出。边界条件：忽略可编辑元素 (`INPUT`, `TEXTAREA`, `contentEditable`) 中的按键，防止干扰复制操作。

### `useHeartbeat.ts`: 心跳保活
每 5s 调用 `api.renderHeartbeat`。后台时暂停；结合 `isRenderLoopAlive()` 检测 GPU 崩溃，失活时跳过心跳让后端重载 WebView。

### `useAppStore.ts`: 兼容性 Re-export
仅导出 `useAppInit` 和配置相关的工具函数，旧引用迁移至此。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                     App Initialization                      │
│                   (useAppInit.ts)                           │
│                                                             │
│  ┌──────────────┐   ┌──────────────────┐   ┌────────────┐  │
│  │ useEvent     │──►│ useInitialData   │──►│ useHeart-  │  │
│  │ Listeners    │   │ Load             │   │ beat       │  │
│  └──────────────┘   └──────────────────┘   └────────────┘  │
│           ▲                  │                 │             │
│           │                  ▼                 ▼             │
│  ┌──────────────────────────────────────────────────────┐    │
│  │               Global Event Bus & Stores              │    │
│  │  (useAuthStore, useConfigStore, useQualityStore...)  │    │
│  └──────────────────────────────────────────────────────┘    │
│           ▲                  │                 │             │
│           │                  ▼                 ▼             │
│  ┌──────────────┐   ┌──────────────────┐   ┌────────────┐  │
│  │ useGlobal    │   │ Animation        │   │ Theme      │  │
│  │ Shortcut     │   │ Active Context   │   │ Store      │  │
│  └──────────────┘   └──────────────────┘   └────────────┘  │
└─────────────────────────────────────────────────────────────┘
           │                  │                 │
           ▼                  ▼                 ▼
┌─────────────────────────────────────────────────────────────┐
│                   Tauri API Bridge                          │
│                   (tauriApi.ts)                             │
│                                                             │
│  ┌──────────────┐   ┌──────────────────┐   ┌────────────┐  │
│  │ Desktop Only │   │ Retry Logic      │   │ Biometric  │  │
│  │ Interceptor  │   │ (withRetry)      │   │ Fallback   │  │
│  └──────────────┘   └──────────────────┘   └────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **配置同步流**：
    *   用户修改配置 -> `updateConfig` -> 标记 `dirtyFields` -> 500ms Debounce -> `saveConfigDirect` -> IPC 调用 `api.saveConfig`。
    *   后端推送配置变更 -> `onConfigChanged` -> `mergeConfigFromBackend` -> 跳过 `dirtyFields` -> 更新 Store。
    *   应用关闭 -> `onCloseRequested` -> `hasPendingConfig` 检查 -> `flushPendingConfig` -> 强制保存。

2.  **认证与在线状态流**：
    *   用户登录 -> `doLogin` -> `api.doLogin` -> 成功 -> 更新 Auth Store -> 触发质量探测 (若满足阈值)。
    *   后台检测 -> `onBackgroundCheckResult` -> 更新 `bgStatus` -> `checkOnline` -> 串行化调用 `queryPortalStatus` -> 更新 `status`。

3.  **动画性能流**：
    *   用户交互 -> `markInteraction` -> `usePageIdle` 重置空闲计时器 -> `useAdaptiveFramePace` 提升帧率。
    *   空闲超时 -> `usePageIdle` 标记 Idle -> `useAnimationActive` Context 变化 -> 动画 Hooks (`useBreatheAnimation` 等) 暂停 GSAP 实例。
    *   后台切换 -> `visibilitychange` -> 暂停心跳和动画。

4.  **主题同步流**：
    *   初始化 -> `initTheme` -> 读取 Storage/Config -> 设置 State。
    *   State 变化 -> `subscribe` -> 修改 DOM Class/CSS Variables -> 持久化 Storage。

## Connections

*   [[android-backend-rust]]: Tauri IPC 通信的目标模块。
*   [[android-ui-components]]: 消费 `useThemeStore`, `useAuthStore`, `useAdapterStore` 等状态的 UI 组件。
*   [[android-shared-types]]: 共享的类型定义，如 `Config`, `NetworkQuality`, `ThemeName`。

## Known Issues

1.  **Android 特定降级**：
    *   所有涉及物理网卡、DHCP、DNS DoH、窗口最小化/关闭、托盘的功能均通过 `desktopOnly` 显式拒绝，避免 IPC 报错。
    *   `checkPortalStatus` 传入空 IP 字符串，依赖后端回退逻辑。
    *   适配器相关事件 (`onAdaptersChanged` 等) 在 `tauriApi` 中映射为 `noopListener`，`useEventListeners` 中跳过订阅。

2.  **状态一致性保护**：
    *   **配置同步**：`dirtyFields` 机制防止后端旧数据覆盖本地未保存的新数据；失败计数机制防止永久脱节。
    *   **登录状态**：`doLogin` 成功后不立即 `checkOnline` 避免状态闪烁；`checkOnline` 使用锁串行化。
    *   **UI 渲染优化**：`setStatusStable` 比较引用/值避免无效重渲染；`useAnimationActive` 全局 Context 减少监听器开销。

3.  **资源泄漏防护**：
    *   **定时器清理**：所有 `setTimeout`/`setInterval` 均在 cleanup 函数中清除，特别是 `flushPendingConfig` 中的 2s 超时定时器。
    *   **GSAP 动画**：每个动画 Hook 都在 unmount 时调用 `.kill()` 停止动画。
    *   **监听器清理**：`createEventListener` 处理异步注册期间的取消；`useEventListeners` 收集所有 unlisten 函数统一调用。

4.  **StrictMode 兼容**：
    *   `useAsyncLock`, `useEventListeners`, `useInitialDataLoad` 等均使用 `mountedRef` 并在 effect setup 阶段重置，防止 React 18 StrictMode 下二次执行导致的逻辑失效。

5.  **安全性**：
    *   `openExternal` 严格校验 URL 协议和内容。
    *   密码处理中使用 `PASSWORD_MASK` 标识，并在保存/加载时特殊处理。