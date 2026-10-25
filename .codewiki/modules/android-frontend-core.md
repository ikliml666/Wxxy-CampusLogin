---
title: 安卓前端 - 运行时核心（入口 / 领域 store / 钩子 / 外壳组件）
type: module
source_files:
  - android/frontend/src/App.tsx
  - android/frontend/src/main.tsx
  - android/frontend/src/vite-env.d.ts
  - android/frontend/src/hooks/useAdapterStore.ts
  - android/frontend/src/hooks/useAdaptiveFramePace.ts
  - android/frontend/src/hooks/useAnimationProfile.ts
  - android/frontend/src/hooks/useAppInit.ts
  - android/frontend/src/hooks/useAppStore.ts
  - android/frontend/src/hooks/useAsyncLock.ts
  - android/frontend/src/hooks/useBreatheAnimation.ts
  - android/frontend/src/hooks/useDeviceProfile.ts
  - android/frontend/src/hooks/useFormFactor.ts
  - android/frontend/src/hooks/tauriApi.ts
tags: [安卓, 前端, 运行时核心, store, 钩子, 外壳]

## Overview

本模块是安卓前端（`android/frontend`）的运行时骨架：`main.tsx` 负责渲染前初始化（主题、崩溃自恢复、gsap 全局配置、内存压力钩子），`App.tsx` 按短边 ≥600 CSS px 在「手机移动外壳（`AppInner`）」与「平板 Dock 外壳（`TabletShell`）」之间二选一，`hooks/` 提供 zustand 领域 store（`useAdapterStore`）、IPC 封装 `tauriApi` 与全部生命周期/动画钩子。

与桌面前端的根本差异：安卓前端是 `tauri-app/frontend/src` 的**独立复刻代码库**（同构不等于共享代码）。安卓独有文件包括 `hooks/{useDeviceProfile,useAdaptiveFramePace,useFormFactor}.ts`、`face/`、`components/{mobile,tablet}` 等。

## Key Components

### 入口与根组件

| 导出 / 函数 | 位置 | 用途 |
| --- | --- | --- |
| `App`（default export） | `App.tsx:363` | 根组件。调用 `useFormFactor()`（`App.tsx:364`）判定形态，`tablet` 渲染 `<TabletShell />`、否则 `<AppInner />`（`App.tsx:368`）；外层包 `ErrorBoundary` + `AnimationActiveProvider`，并在双外壳之外单例挂载 `<FaceCaptureDialog />`（`App.tsx:370`）以及通知权限引导 `NotificationPermissionGate`（`App.tsx:371`） |
| `AppInner`（模块内组件） | `App.tsx:52` | 手机移动外壳：顶部轻 header（状态点 + 运营商 chip + 更新胶囊 + 质量胶囊 + 赞助/设置/主题/关于图标，`App.tsx:202-260`）、首页快捷登录条（`App.tsx:263-267`）、单列滚动主区（`App.tsx:270-297`）、`BottomNav`（`App.tsx:299`）、概览/账号/自助/质量/后台检测/更多 tab 切换（`App.tsx:57-192`） |
| `NotificationPermissionGate`（模块内组件） | `App.tsx:381` | 首启通知权限引导：仅询问一次（`campus-notification-prompt` 标记，`App.tsx:385`），调用 `requestNotificationPermission`（`App.tsx:400`） |
| `initTheme` | `main.tsx:27` | 读 `campus-light-mode` 切 `dark` class 与 `data-light` 属性，读 `campus-theme` 加 `theme-<name>` class，读 `campus-glass-level` 设 `data-glass`（`main.tsx:27-44`）；模块加载即执行防闪烁 |
| `setupCrashRecovery` | `main.tsx:48` | `root` 空且可见时重载（≤3 次，`main.tsx:49-60`）；`GPU/WebGL/SharedArrayBuffer` 报错重载（`main.tsx:71-77`）；可见性切换停/启 `gsap.globalTimeline`（`main.tsx:82-91`）；1s 心跳更新（`main.tsx:93-103`）；2s 检测心跳丢失 >10s 触发恢复（`main.tsx:105-113`） |
| `window.__onTrimMemory` | `main.tsx:123` | 安卓内存压力钩子：高压力级别（`level >= 10`）且无人脸会话时动态 import `faceService` 释放引擎（`main.tsx:123-129`） |
| `AppWrapper` | `main.tsx:131` | DEV 用 `React.StrictMode`，生产用 `React.Fragment`（`main.tsx:131-133`） |

`main.tsx` 渲染树（`main.tsx:135-145`）：`ErrorBoundary` → `LazyMotion features={domMax} strict` → `MotionConfig reducedMotion="user"` → `App`。
`vite-env.d.ts:1` 仅包含 `/// <reference types="vite/client" />`，未声明自定义环境变量类型。

### hooks/ — IPC 封装（tauriApi.ts）

| 导出 / 接口 | 位置 | 说明 |
| --- | --- | --- |
| `tauriApiWithRetry` | `tauriApi.ts:371` | 生产使用的 API 对象：`...tauriApi` 展开后仅覆写 `saveConfig` 包重试（`tauriApi.ts:376`） |
| `BatteryOptimizationInfo` | `tauriApi.ts:26` | 字段：`ignoring`(boolean)、`brand`(string)、`hasVendorTarget`(boolean) |
| `VendorSettingsResult` | `tauriApi.ts:32` | 字段：`path`(`'vendor'\|'vendor_action'\|'app_details'\|'settings'\|'none'`)、`target`(string)、`tried`(string[]) |
| `AvoidBadWifiStatus` | `tauriApi.ts:153` | 字段：`granted`(boolean)、`avoidBadWifi`(number)、`pendingRestoreAvoid`(boolean)、`pendingRestoreCaptive`(boolean) |
| `RestoreWrittenSettingsResult` | `tauriApi.ts:164` | 字段：`restored`(Record<string, boolean>)、`reason?`(string) |

内部符号：
- `desktopOnly`（`tauriApi.ts:14`）：桌面专属命令统一 reject。
- `noopListener`（`tauriApi.ts:18`）：桌面专属事件返回 no-op 清理函数。
- `ConnectionCampusStatus`（`tauriApi.ts:20`）：字段 `onCampus`/`name`/`message`。
- `CampusStatusResult`（`tauriApi.ts:38`）：字段 `onCampusNetwork`/`currentSsid`/`campusMessage`/`enableNetworkNameCheck`/`requiredNetworkName`/`campusWifi`/`campusWired`。
- `TauriApi` 接口（`tauriApi.ts:48-151`）：声明全部 IPC 方法（含安卓恒 reject 的桌面专属方法）。
- `tauriApi` 常量实现对象（`tauriApi.ts:200-339`）：安卓可用命令走 `invoke`，桌面专属（`exportConfig`/`importConfig`/`exportDiagnostics`/`getAdapters`/`enableAdapter`/`getAdapterDetails`/`dhcpRenewAll`/`dhcpReleaseRenew`/`dhcpReleaseRenewAdapter`/`minimizeWindow`/`closeWindow`/`cancelAutoExit`/`showWindow`/`checkDnsDohStatus`/`setupDnsDoh`/`resetDns`/`renderHeartbeat`/`notifyWindowReady`）均赋值 `desktopOnly` reject（`tauriApi.ts:203-334`）；`onAdaptersChanged`/`onAdapterDetailsChanged`/`onAdapterDisabledWarning`/`onAutoExitCountdown`/`onAutoExitCancelled`/`onCampusExitCountdown`/`onCampusExitCancelled` 使用 `noopListener`（`tauriApi.ts:269-315`）。
- `createEventListener`（`tauriApi.ts:169`）：具名事件订阅工厂，处理「listen 未完成即取消」。
- `isRetryableError`（`tauriApi.ts:341`）、`withRetry`（`tauriApi.ts:353`）：指数退避重试工具。

`TauriApi` 典型方法（声明/实现）：

| 方法 | 声明 | 实现 | 安卓行为 |
| --- | --- | --- | --- |
| `getConfig` | 49 | 201 | `invoke('get_config')` |
| `saveConfig` | 50 | 202（覆写376） | `invoke('save_config')` + 重试 |
| `bindToWifi` | 58 | 209 | `invoke('bind_to_wifi')` |
| `checkPortalStatus` | 59 | 210 | `invoke('check_portal_status')` |
| `doLogin` | 62 | 213 | `invoke('do_login')` |
| `verifyWindowsIdentity` | 66 | 217-254 | 生物识别门：2D 人脸回退走 `useFaceDialogStore.openFaceDialog`（`tauriApi.ts:235`），否则 `biometricAuthenticate`（`tauriApi.ts:249`） |
| `listAccounts` | 90 | 273 | `invoke('list_accounts')` |
| `checkNetworkQuality` | 106 | 287 | `invoke('check_network_quality')` |
| `openExternal` | 110 | 291-302 | 协议/长度/URL 校验后 `openerOpenUrl` |
| `getAvoidBadWifiStatus` | 148 | 337 | `invoke('get_avoid_bad_wifi_status')` |
| `restoreWrittenSettings` | 150 | 338 | `invoke('restore_written_settings')` |

### hooks/ — 领域 store

| 导出 | 位置 | 说明 |
| --- | --- | --- |
| `useAdapterStore` | `useAdapterStore.ts:11` | 状态：`activePanel`（`PanelName`，`useAdapterStore.ts:7`）；动作：`setActivePanel`（`useAdapterStore.ts:8`）。安卓端网卡数据链已删除（adapters 恒空），仅保留激活面板状态供导航用（`useAdapterStore.ts:1-2`） |
| `useAppInit` re-export | `useAppStore.ts:2` | 兼容 re-export |
| `hasPendingConfig` / `flushPendingConfig` re-export | `useAppStore.ts:3` | 从 `./useConfigStore` 兼容 re-export（`useAppStore.ts:1-3` 注释说明已拆分） |

### hooks/ — 生命周期与动画钩子

| 导出 | 位置 | 用途 |
| --- | --- | --- |
| `useAppInit` | `useAppInit.ts:6` | 顺序调用 `useEventListeners` → `useInitialDataLoad` → `useHeartbeat` → `useGlobalShortcut`（`useAppInit.ts:7-10`） |
| `useFormFactor` | `useFormFactor.ts:38` | 短边 ≥600px 判平板（`useFormFactor.ts:15-16`），resize/orientationchange 重算（`useFormFactor.ts:24-36`） |
| `useOrientation` | `useFormFactor.ts:43` | 宽≥高为 `landscape`（`useFormFactor.ts:19-20`） |
| `FormFactor` / `Orientation` 类型 | `useFormFactor.ts:9-10` | `'phone' \| 'tablet'` / `'portrait' \| 'landscape'` |
| `TABLET_MIN_EDGE_PX` | `useFormFactor.ts:13` | 600 |
| `useDeviceProfile` | `useDeviceProfile.ts:25` | 恒返旗舰档常量 `HIGH_PROFILE`（`idleFps:30`/`activeFps:60`，`useDeviceProfile.ts:17-23`）作氛围动画节流；GPU/核数/内存三档判定已删（`useDeviceProfile.ts:1-4`） |
| `DeviceTier` / `DeviceProfile` | `useDeviceProfile.ts:6,8` | `DeviceProfile` 字段：`tier`(`DeviceTier`)、`gpuVendor`(`'adreno'\|'mali'\|'immortalis'\|'power vr'\|'other'`)、`idleFps`、`activeFps` |
| `useAdaptiveFramePace` | `useAdaptiveFramePace.ts:51` | 把 `useDeviceProfile` 的 idle/active fps 落到 `gsap.ticker.fps()`，250ms 轮询回落（`PACE_POLL_MS` `useAdaptiveFramePace.ts:18`）、事件回调即时提帧 |
| `markInteraction` | `useAdaptiveFramePace.ts:26` | 程序性交互主动推活跃窗口（`ACTIVE_WINDOW_MS=2000` `useAdaptiveFramePace.ts:16`） |
| `useAnimationProfile` | `useAnimationProfile.ts:51` | 恒返 `HIGH_PROFILE` 满配档（`useAnimationProfile.ts:31-49`，仅 `refreshRate` 驱动 easing/节流数值）；standard/economy 分档已删（`useAnimationProfile.ts:28-30`） |
| `AnimationTier` / `AnimationProfile` | `useAnimationProfile.ts:6,8` | `AnimationProfile` 字段：`tier`、`willChangeOrbs`、`magneticOffset`、`magneticDuration`、`numberDuration`、`springStiffness`、`springDamping`、`powerPreference`、`prefersCssAnimation`、`enableGpuCompositing`、`enablePageSlide`、`enableTilt`、`enableBackdropBlur`、`startupBoost`、`startupStaggerDelay`、`easing`、`refreshRate` |
| `useBreatheAnimation` | `useBreatheAnimation.ts:15` | gsap yoyo 呼吸动画 ref，空闲暂停（`useBreatheAnimation.ts:29-63`，依赖 `useAnimationActive`） |
| `BreatheOptions` | `useBreatheAnimation.ts:5` | 选项接口字段：`minOpacity`/`maxOpacity`/`duration`/`minScale`/`maxScale`/`minRotation`/`maxRotation`（均可选） |
| `useAsyncLock` | `useAsyncLock.ts:3` | 防重复提交锁，返回 `[isRunning, execute]`（`useAsyncLock.ts:26`），`cooldownMs` 默认 1500（`useAsyncLock.ts:5`） |

## Architecture

```text
┌──────────────────────────────────────────────┐
│            main.tsx (Entry)                  │
│ initTheme/setupCrashRecovery/__onTrimMemory  │
└───────────────────────┬──────────────────────┘
                        │ render (main.tsx:135)
                        ▼
┌──────────────────────────────────────────────┐
│                 App (Root)                   │
│ useFormFactor() ──► tablet? TabletShell      │
│                    : AppInner (App.tsx:363)  │
└───────────────────────┬──────────────────────┘
                        │
      ┌─────────────────┴─────────────────┐
      ▼                                   ▼
┌──────────────┐                   ┌──────────────┐
│  AppInner    │                   │  TabletShell │
│ (Phone Shell)│                   │ (Desktop Dock│
│ App.tsx:52   │                   │  Layout)     │
├──────────────┤                   └──────────────┘
│ Header       │
│ Main (Scroll)│
│ BottomNav    │
│ Panels:      │
│ Dashboard    │
│ Account      │
│ SelfService  │
│ Quality/     │
│ Monitor/More │
└──────────────┘
      ▲
      │ useAppInit() App.tsx:53
      ├── useEventListeners()
      ├── useInitialDataLoad()
      ├── useHeartbeat()
      └── useGlobalShortcut()
```

## Data Flow

### 启动编排顺序（基于提供源码）

1. `main.tsx` 模块求值：gsap 全局配置（`main.tsx:17-19`）→ reduced-motion 降级（`main.tsx:21-25`）→ `initTheme()` 同步写 DOM（`main.tsx:27-46`）→ `setupCrashRecovery()` 注册监听与定时器（`main.tsx:48-116`）→ `window.__onTrimMemory` 赋值（`main.tsx:123-129`）→ `ReactDOM.createRoot(...).render`（`main.tsx:135-145`）。
2. `App`（`App.tsx:363`）→ `useFormFactor()`（`App.tsx:364`）→ `AppInner`（`App.tsx:368`）或 `TabletShell`（`App.tsx:368`）。
3. `AppInner` 首句 `useAppInit()`（`App.tsx:53`），其内部固定顺序（`useAppInit.ts:6-10`）：`useEventListeners` → `useInitialDataLoad` → `useHeartbeat` → `useGlobalShortcut`（具体实现文件未在本批次提供，但由 `useAppInit` 顺序调用）。
4. 面板切换：`tab` 状态初始化自 `safeStorage` 白名单校验（`App.tsx:57-62`），`deferredTab` 降频渲染（`App.tsx:63`）；`handleTabChange`（`App.tsx:119-127`）计算方向 `navDir` 并持久化；`panelContent` 按 `deferredTab` 切换六面板（`App.tsx:149-192`），外裹 `AnimatePresence` + `m.div` 方向转场（`App.tsx:281-295`）。
5. 帧率适配：`useAdaptiveFramePace()`（`App.tsx:54`）挂载后由 `useDeviceProfile` 提供帧率档，`markInteraction` 在 `handleTabChange`（`App.tsx:120`）被调用提帧；`useAnimationProfile`（`App.tsx:115`）提供 `easing` 给 `createPanelAppleVariants`（`App.tsx:117`）。
6. IPC 封装：`tauriApiWithRetry`（`App.tsx:14` 导入）供全应用调用，仅 `saveConfig` 带重试（`tauriApi.ts:376`）。

## Connections

- 面板层与业务 UI：[[android-frontend-panels]]（auth/account/monitor/settings 面板、`shared/LogPanel`）。
- 后端命令与事件契约：[[android-backend]]（`invoke` 命令名、`background-check-result`/`network-quality-result`/`config-changed` 等事件载荷）。
- 安卓专属插件与门控：[[android-plugins]]（`plugin-biometric`、`plugin-notification`、`plugin-opener`、电池优化命令）。
- 桌面同构对照：[[desktop-frontend-hooks]]（桌面 store/钩子集合）、[[desktop-frontend-shared]]（共享组件）、[[desktop-app-lifecycle]]（桌面启动编排、托盘与窗口控制）。
- 被安卓裁剪的桌面能力来源：[[desktop-platform]]（窗口/托盘）、[[desktop-network-core]]（适配器与 DHCP）、[[desktop-network-dns]]（DNS DoH）。

## Known Issues

1. **`useAppStore` 兼容壳无人引用**：`useAppStore.ts:2-3` 只做 re-export（`useAppInit` 与 `hasPendingConfig`/`flushPendingConfig`），全仓若清理后可直接删除。
2. **`TauriApi` 类型仍声明了安卓不可用的方法**：`TauriApi`（`tauriApi.ts:48-151`）保留 `getAdapters`/`dhcpRenewAll`/`setupDnsDoh`/`minimizeWindow`/`showWindow` 等桌面方法，调用方编译期无法察觉其恒 reject（`desktopOnly` 实现于 `tauriApi.ts:14`，赋值于 `tauriApi.ts:200-339`）。
3. **`main.tsx` 崩溃恢复计数不持久**：`crashCount` 是模块级变量（`main.tsx:49`），`window.location.reload()` 后归零，因此 `MAX_CRASH_RELOADS = 3`（`main.tsx:50`）实际退化为单次页面生命周期内最多 3 次；持续崩溃会形成重载循环（`main.tsx:52-60`）。
4. **`vite-env.d.ts` 极简**：`vite-env.d.ts:1` 只有 `/// <reference types="vite/client" />`，未声明 `VITE_PLATFORM` 等自定义环境变量类型，各处 `=== 'android'` 字符串比较无类型保护。
5. **桌面件渲染状态注释与实现不一致**：`App.tsx:4` 注释列举桌面件清单时提及 `FluidBackground`/`Onboarding`/`Sponsor` 在手机外壳不再渲染，但 `OnboardingWizard` 与 `SponsorCard` 在手机外壳实际以懒加载弹窗形式挂载（`App.tsx:50`/`App.tsx:348`、`App.tsx:25`/`App.tsx:303`），仅 `FluidBackground` 等纯桌面件确无调用点。
6. **`useAdapterStore` 网卡数据链已删**：`useAdapterStore.ts:1-2` 注释说明 adapters 恒空、面板不渲染网卡卡，仅保留 `activePanel` 导航状态，历史 `refreshAdapters` 类逻辑若残留会走空转。