---
title: 桌面前端 hooks 层（IPC 网关 / 领域 store / 事件监听 / 初始化编排）
type: module
source_files:
  - tauri-app/frontend/src/hooks/tauriApi.ts
  - tauri-app/frontend/src/hooks/useAdapterStore.ts
  - tauri-app/frontend/src/hooks/useAnimationProfile.ts
  - tauri-app/frontend/src/hooks/useAppInit.ts
  - tauri-app/frontend/src/hooks/useAppStore.ts
  - tauri-app/frontend/src/hooks/useAsyncLock.ts
  - tauri-app/frontend/src/hooks/useAsyncLock.test.tsx
  - tauri-app/frontend/src/hooks/useAuthStore.ts
  - tauri-app/frontend/src/hooks/useBreatheAnimation.ts
  - tauri-app/frontend/src/hooks/useConfigStore.ts
  - tauri-app/frontend/src/hooks/useConfigStore.selfPassword.test.ts
  - tauri-app/frontend/src/hooks/useEventListeners.ts
  - tauri-app/frontend/src/hooks/useGlobalShortcut.ts
  - tauri-app/frontend/src/hooks/useGlowAnimation.ts
  - tauri-app/frontend/src/hooks/useGpuCorrection.ts
  - tauri-app/frontend/src/hooks/useHeartbeat.ts
  - tauri-app/frontend/src/hooks/useInitialDataLoad.ts
  - tauri-app/frontend/src/hooks/useLogToastStore.ts
  - tauri-app/frontend/src/hooks/useLogToastStore.test.ts
  - tauri-app/frontend/src/hooks/usePageIdle.ts
  - tauri-app/frontend/src/hooks/usePulseAnimation.ts
  - tauri-app/frontend/src/hooks/useQualityStore.ts
  - tauri-app/frontend/src/hooks/useStartupBoost.ts
  - tauri-app/frontend/src/hooks/useThemeStore.ts
tags: [前端, hooks, zustand, IPC, Tauri, 事件监听, 状态管理, 初始化编排, 动画档位]
---

## Overview

`tauri-app/frontend/src/hooks/` 是桌面前端的"业务内核"：`tauriApi.ts` 是全应用唯一的 IPC 出口（56 个 `invoke` 命令 + 16 个事件订阅）；其余文件按领域拆成 6 个 zustand store（auth / config / adapter / quality / log-toast / theme），加上动画档位、GPU 校正、渲染心跳、初始化编排等横切 hook。

本目录不渲染 UI，只负责三件事：把 Rust 后端的能力包装成 Promise API、把后端推送的事件折叠成 store 状态、在启动时按固定顺序组装这些 store（`useAppInit`）。所有 UI 组件（见 [[desktop-frontend-shared]]）都通过 store 订阅或 `tauriApiWithRetry` 直接调用来消费本模块。

## Key Components

### 目录总览（24 个文件）

| 文件 | 类别 | 主要导出（行号） |
|---|---|---|
| `tauriApi.ts` | IPC 网关 | `tauriApi`(133)、`tauriApiWithRetry`(255)、`createEventListener`(102)、`withRetry`(237)、`isRetryableError`(225)、`interface TauriApi`(27) |
| `useAuthStore.ts` | 领域 store | `useAuthStore`(127) |
| `useConfigStore.ts` | 领域 store | `useConfigStore`(53)、`hasPendingConfig`(183)、`flushPendingConfig`(190) |
| `useAdapterStore.ts` | 领域 store | `useAdapterStore`(48)、`refreshAdapterData`(16) |
| `useQualityStore.ts` | 领域 store | `useQualityStore`(43)、`getLastQualityResultTime`(19) |
| `useLogToastStore.ts` | 领域 store | `useLogToastStore`(28) |
| `useThemeStore.ts` | 领域 store | `useThemeStore`(19) |
| `useAppInit.ts` | 编排 | `useAppInit`(6) |
| `useInitialDataLoad.ts` | 编排 | `useInitialDataLoad`(18) |
| `useEventListeners.ts` | 编排 | `useEventListeners`(18) |
| `useHeartbeat.ts` | 全局副作用 | `useHeartbeat`(5) |
| `useGlobalShortcut.ts` | 全局副作用 | `useGlobalShortcut`(4) |
| `useAnimationProfile.ts` | 性能/动画 | `useAnimationProfile`(73) |
| `usePageIdle.ts` | 性能/动画 | `AnimationActiveProvider`(85)、`useAnimationActive`(95) |
| `useStartupBoost.ts` | 性能/动画 | `useStartupBoost`(15) |
| `usePulseAnimation.ts` | 动画 | `usePulseAnimation`(15) |
| `useBreatheAnimation.ts` | 动画 | `useBreatheAnimation`(15) |
| `useGlowAnimation.ts` | 动画 | `useGlowAnimation`(11) |
| `useGpuCorrection.ts` | 硬件探测 | `useGpuCorrection`(80) |
| `useAsyncLock.ts` | 工具 | `useAsyncLock`(3) |
| `useAppStore.ts` | 兼容壳 | 仅 re-export（1-3） |
| `useAsyncLock.test.tsx` | 测试 | 1 个用例（20-40） |
| `useConfigStore.selfPassword.test.ts` | 测试 | 3 个用例（24-46） |
| `useLogToastStore.test.ts` | 测试 | 7 个用例（9-95） |

### tauriApi.ts — 唯一 IPC 出口

| 位置 | 项 | 说明 |
|---|---|---|
| `tauriApi.ts:11-15` | `interface ConnectionCampusStatus` | 单个连接（WiFi / 有线）的校园网判定：`onCampus`、`name`、`message` |
| `tauriApi.ts:17-25` | `interface CampusStatusResult` | `check_campus_status` 回包：`onCampusNetwork`、`currentSsid`、`campusMessage`、`enableNetworkNameCheck`、`requiredNetworkName`、`campusWifi`、`campusWired` |
| `tauriApi.ts:27-100` | `interface TauriApi` | 全部 72 个成员的签名（56 个请求-响应 + 16 个事件订阅） |
| `tauriApi.ts:102-131` | `createEventListener<T>(eventName)` | 事件订阅工厂：返回 `(cb) => unlisten`；内部用 `cancelled` 标志处理"注册未完成即取消"的竞态（`listen` 的 Promise 尚未 resolve 时调用 unlisten，则在 Promise 完成后补调 `fn?.()`，120-129）；注册失败仅 DEV 下 `console.error` 并返回 `null`（115-118） |
| `tauriApi.ts:133-223` | `const tauriApi: TauriApi` | 逐方法实现：`invoke<返回类型>('命令名', { 参数 })` 或 `createEventListener<载荷>('事件名')` |
| `tauriApi.ts:225-235` | `isRetryableError(e)` | 小写化后匹配 `timeout`/`network`/`fetch`/`connection` 四个子串 |
| `tauriApi.ts:237-253` | `withRetry(fn, maxRetries=2, baseDelay=500)` | 指数退避 + 0-200ms 抖动（245） |
| `tauriApi.ts:255-261` | `tauriApiWithRetry` | `{...tauriApi}` 后**只**覆盖 `saveConfig`（260）。257-259 注释明确说明为何不再包 `checkPortalStatus`/`checkNetworkQuality` |

#### 请求-响应 API（56 个，全部逐条列出）

| 行号 | 前端方法 | IPC 命令 | 参数 / 返回 |
|---|---|---|---|
| 134 | `getConfig()` | `get_config` | → `Config` |
| 135 | `saveConfig(config, clearPassword?, clearSelfPassword?)` | `save_config` | → `SaveConfigResult` |
| 136 | `getAdapters(force?)` | `get_adapters` | → `Adapter[]` |
| 137 | `getDisabledAdapters()` | `get_disabled_adapters` | → `DisabledAdapter[]` |
| 138 | `enableAdapter(adapterName)` | `enable_adapter` | → `EnableAdapterResult` |
| 139 | `getAdapterDetails()` | `get_adapter_details` | → `AdapterDetail[]` |
| 140 | `checkPortalStatus(adapterIp)` | `check_portal_status` | → `PortalStatusResult` |
| 141 | `checkCampusStatus()` | `check_campus_status` | → `CampusStatusResult` |
| 142 | `doLogin(adapterName?)` | `do_login` | → `LoginResult` |
| 143 | `doLogout(adapterName?)` | `do_logout` | → `LoginResult` |
| 144 | `bindOperator({account,password,operator,phone,smsPassword})` | `bind_operator` | → `CommandResult` |
| 145 | `getBindStatus({account,password})` | `query_bind_status` | → `CommandResult` |
| 146 | `verifyWindowsIdentity({consentMessage})` | `verify_windows_identity` | → `CommandResult` |
| 147 | `revealOperatorCredential({account,password,operator})` | `reveal_operator_credential` | → `CommandResult` |
| 148 | `querySelfDashboard({account,password})` | `query_self_dashboard` | → `CommandResult` |
| 149 | `selfOfflineSession({account,password,sessionId})` | `self_offline_session` | → `CommandResult` |
| 150 | `querySelfOnlineLog({account,password,startTime,endTime})` | `query_self_online_log` | → `CommandResult` |
| 151 | `minimizeWindow()` | `minimize_window` | → `void` |
| 152 | `closeWindow()` | `close_window` | → `void` |
| 160 | `listAccounts()` | `list_accounts` | → `string[]` |
| 161 | `switchAccount(accountName)` | `switch_account` | → `SwitchAccountResult` |
| 162 | `saveCurrentAsAccount(accountName)` | `save_current_as_account` | → `SaveAccountResult` |
| 163 | `deleteAccount(accountName)` | `delete_account` | → `DeleteAccountResult` |
| 164 | `getActiveAccount()` | `get_active_account` | → `string` |
| 165 | `startBackgroundCheck()` | `start_background_check` | → `CommandResult` |
| 166 | `stopBackgroundCheck()` | `stop_background_check` | → `CommandResult` |
| 167 | `triggerBackgroundCheck()` | `trigger_background_check` | → `CommandResult` |
| 168 | `getBackgroundStatus()` | `get_background_status` | → `BackgroundStatus` |
| 169 | `dhcpRenewAll()` | `dhcp_renew_all` | → `DhcpRenewResult` |
| 170 | `dhcpReleaseRenew()` | `dhcp_release_renew` | → `DhcpReleaseRenewResult` |
| 171 | `dhcpReleaseRenewAdapter(adapterName)` | `dhcp_release_renew_adapter` | → `DhcpReleaseRenewResult` |
| 172 | `checkNetworkQuality()` | `check_network_quality` | → `NetworkQuality` |
| 174 | `startLatencyTest()` | `start_latency_test` | → `CommandResult` |
| 175 | `stopLatencyTest()` | `stop_latency_test` | → `CommandResult` |
| 176-193 | `openExternal(url)` | `open_external` | 前置校验：仅 `http://`/`https://`（177）、长度 ≤2048（178）、`new URL()` 可解析（179）；invoke 失败时回退 `@tauri-apps/plugin-shell` 的 `open`（185-191），两者都失败返回 `false` |
| 194 | `getAutoLaunch()` | `get_auto_launch` | → `{ enabled: boolean }` |
| 195 | `setAutoLaunch(enabled)` | `set_auto_launch` | → `AutoLaunchResult` |
| 196 | `getNotificationEnabled()` | `get_notification_enabled` | → `boolean` |
| 197 | `setNotificationEnabled(enabled)` | `set_notification_enabled` | → `boolean` |
| 198 | `cancelAutoExit()` | `cancel_auto_exit` | → `CommandResult` |
| 204 | `showWindow()` | `show_window` | → `void` |
| 205 | `getLogs(lines?)` | `get_logs` | → `string`（整块文本） |
| 206 | `clearLogs()` | `clear_logs` | → `boolean` |
| 207 | `getDebugMode()` | `get_debug_mode` | → `boolean` |
| 208 | `setDebugMode(enabled)` | `set_debug_mode` | → `boolean` |
| 209 | `getInitData()` | `get_init_data` | → `InitData` |
| 210 | `checkUpdate()` | `check_update` | → `UpdateInfo` |
| 211 | `downloadUpdate(url)` | `download_update` | → `string`（落盘路径） |
| 212 | `installUpdate(filePath, checksumUrl?)` | `install_update` | → `boolean` |
| 213 | `getMirrorUrls(githubUrl)` | `get_mirror_urls` | → `MirrorSource[]` |
| 217 | `checkDnsDohStatus()` | `check_dns_doh_status` | → `DnsDohStatus` |
| 218 | `setupDnsDoh(family?)` | `setup_dns_doh` | `family?: 'ipv4'\|'ipv6'\|'both'` → `DnsSetupResult` |
| 219 | `renderHeartbeat()` | `render_heartbeat` | → `{ online: boolean; checking: boolean }` |
| 220 | `getGpuInfo()` | `get_gpu_info` | → `GpuInfo` |
| 221 | `getLogRetentionDays()` | `get_log_retention_days` | → `number` |
| 222 | `setLogRetentionDays(days)` | `set_log_retention_days` | → `void` |

#### 事件订阅（16 个，全部逐条列出）

| 行号 | 前端方法 | 事件名 | 载荷类型 | 主要消费点 |
|---|---|---|---|---|
| 153 | `onBackgroundCheckResult` | `background-check-result` | `BackgroundCheckEventData` | `useEventListeners.ts:77` |
| 154 | `onAutoLoginResult` | `auto-login-result` | `AutoLoginEventData` | `useEventListeners.ts:199` |
| 155 | `onAdaptersChanged` | `adapters-changed` | `Adapter[]` | `useEventListeners.ts:215` |
| 156 | `onAdapterDetailsChanged` | `adapter-details-changed` | `AdapterDetail[]` | `useEventListeners.ts:242` |
| 157 | `onDisabledAdaptersChanged` | `disabled-adapters-changed` | `DisabledAdapter[]` | `useEventListeners.ts:248` |
| 158 | `onAdapterDisabledWarning` | `adapter-disabled-warning` | `AdapterDisabledWarningData` | `useEventListeners.ts:254` |
| 159 | `onLoginLog` | `login-log` | `{ message: string; type: string }` | `useEventListeners.ts:263` |
| 173 | `onNetworkQualityResult` | `network-quality-result` | `NetworkQuality` | `useEventListeners.ts:327` |
| 199 | `onAutoExitCountdown` | `auto-exit-countdown` | `AutoExitCountdownData` | `useEventListeners.ts:271` |
| 200 | `onAutoExitCancelled` | `auto-exit-cancelled` | `Record<string, never>`（无载荷） | `useEventListeners.ts:292` |
| 201 | `onCampusExitCountdown` | `campus-exit-countdown` | `{ minimizeDelay: number; exitDelay: number }` | `useEventListeners.ts:299` |
| 202 | `onCampusExitCancelled` | `campus-exit-cancelled` | `Record<string, never>`（无载荷） | `useEventListeners.ts:320` |
| 203 | `onConfigChanged` | `config-changed` | `{ config: Config }` | `useEventListeners.ts:348` |
| 214 | `onDownloadProgress` | `update-download-progress` | `DownloadProgress` | `auth/AboutDialog.tsx:157` |
| 215 | `onUpdateAvailable` | `update-available` | `UpdateAvailableData` | `useEventListeners.ts:335` |
| 216 | `onUpdateNotificationClick` | `update-notification-click` | `unknown` | `App.tsx:223` |

> 16 个事件中 14 个由 `useEventListeners` 统一注册；`onDownloadProgress`（弹窗内进度）与 `onUpdateNotificationClick`（点击系统通知后打开"关于"）在各自组件内按需注册。

### 领域 store

#### useAuthStore.ts（认证 / 在线状态 / 后台巡检状态）

| 位置 | 项 | 说明 |
|---|---|---|
| `useAuthStore.ts:19` | `_checkOnlineLockFlag` | 模块级 `checkOnline` 互斥锁（非 React state） |
| `useAuthStore.ts:24` | `QUALITY_MANUAL_THROTTLE_MS = 60_000` | 登录后手动质量探测节流阈值，与后端 `run_quality_check` 60s 节流对齐 |
| `useAuthStore.ts:26` | `type CampusStatus` | `Awaited<ReturnType<typeof api.checkCampusStatus>>` |
| `useAuthStore.ts:27` | `type PortalStatus` | `Awaited<ReturnType<typeof api.checkPortalStatus>>` |
| `useAuthStore.ts:32-38` | `detectCampusNetwork()` | 调 `check_campus_status`，异常返回 `null` |
| `useAuthStore.ts:41-65` | `buildCampusBgStatusPatch(...)` | 纯函数：按主/副适配器是否无线选择 `campusWifi`/`campusWired` 拼 `bgStatus` 补丁，含 `?? bgStatus.xxx` 兜底 |
| `useAuthStore.ts:68-79` | `pickAdapterIp(adapters, adapter1)` | 指定适配器有 IP 则用；否则优先有线、再无线、再第一个；都没有返回 `''` |
| `useAuthStore.ts:82-92` | `refreshAdaptersForIp(adapter1)` | 列表无 IP 时 `getAdapters(true)` 强制重探 |
| `useAuthStore.ts:95-104` | `queryPortalStatus(adapterIp)` | 返回判别联合 `{ok:true,portal} \| {ok:false}` |
| `useAuthStore.ts:108-113` | `withTimeout(promise, ms, timeoutMsg)` | `Promise.race` 超时包装，防 `isLoggingIn/isLoggingOut` 永久为 true（历史缺陷 P2-34） |
| `useAuthStore.ts:115-125` | `interface AuthStore` | store 契约 |
| `useAuthStore.ts:127-297` | `useAuthStore` | store 实体 |

#### useConfigStore.ts（配置 / 密码保存态 / 账号列表 / 语言）

模块级可变状态（不触发订阅，但被 `hasPendingConfig`/`flushPendingConfig` 读写）：

| 位置 | 项 | 说明 |
|---|---|---|
| `useConfigStore.ts:14` | `saveConfigTimer` | 500ms debounce 定时器 |
| `useConfigStore.ts:15` | `saveConfigPending` | debounce 待发配置 |
| `useConfigStore.ts:17` | `saveConfigInFlight` | 正在飞的保存 Promise（关窗等待用） |
| `useConfigStore.ts:20` | `dirtyFields: Set<string>` | 本地已改未确认字段，`config-changed` 回传时跳过 |
| `useConfigStore.ts:24` | `DIRTY_FAILURE_LIMIT = 3` | 连续失败阈值，超限放弃脏标记 |
| `useConfigStore.ts:25` | `dirtyFailureCounts` | 每字段连续失败计数 |
| `useConfigStore.ts:27-51` | `interface ConfigStore` | store 契约 |
| `useConfigStore.ts:53-181` | `useConfigStore` | store 实体 |
| `useConfigStore.ts:183-188` | `hasPendingConfig()` | `saveConfigPending !== null \|\| saveConfigInFlight !== null` |
| `useConfigStore.ts:190-220` | `flushPendingConfig()` | 清 debounce 定时器 → 发出待存配置（`PASSWORD_MASK` 字段删除后再拼全量，200-204）→ 与 in-flight 一起 `Promise.allSettled` 返回 |

#### useAdapterStore.ts（适配器数据 / 当前面板）

| 位置 | 项 | 说明 |
|---|---|---|
| `useAdapterStore.ts:9` | `_adapterLockFlag` | `refreshAdapters` 互斥锁 |
| `useAdapterStore.ts:16-35` | `refreshAdapterData({force, includeDisabled, triggerCheck})` | 公共刷新动作：并行 `getAdapters`/`getAdapterDetails`/`getDisabledAdapters`（23-27），写回 store（28-30），可选触发后台检测（31）。收敛了历史上三处重复实现（P2-F10） |
| `useAdapterStore.ts:37-46` | `interface AdapterStore` | store 契约 |
| `useAdapterStore.ts:48-74` | `useAdapterStore` | store 实体 |

`refreshAdapterData` 的调用方：`network/NetworkPanel.tsx:141,161`、`network/useNetwork.ts:61`、`useAdapterStore.ts:61`。

#### useQualityStore.ts（质量 / DNS DoH / 更新 / GPU 信息）

| 位置 | 项 | 说明 |
|---|---|---|
| `useQualityStore.ts:12` | `_qualityLockFlag` | `refreshQuality` 互斥锁 |
| `useQualityStore.ts:17` | `lastQualityResultTime` | 最近一次质量结果到达时间戳（毫秒，0=从未） |
| `useQualityStore.ts:19-21` | `getLastQualityResultTime()` | 供 `useAuthStore` 做登录后节流判断 |
| `useQualityStore.ts:23-41` | `interface QualityStore` | store 契约 |
| `useQualityStore.ts:43-90` | `useQualityStore` | store 实体 |

#### useLogToastStore.ts（应用内日志 + toast）

| 位置 | 项 | 说明 |
|---|---|---|
| `useLogToastStore.ts:5` | `toastTimers` | toast 自动消失定时器表，`removeToast`/`cleanupToasts` 需同步清理 |
| `useLogToastStore.ts:6,7` | `toastIdCounter` / `logIdCounter` | 自增 ID |
| `useLogToastStore.ts:9` | `MAX_TOASTS = 4` | toast 上限，超出淘汰最旧 |
| `useLogToastStore.ts:13` | `isDuplicateTitle(toasts, title)` | 同标题去重（防"专用事件 + system-notification"双通道重复弹） |
| `useLogToastStore.ts:15-26` | `interface LogToastStore` | store 契约 |
| `useLogToastStore.ts:28-136` | `useLogToastStore` | store 实体 |

#### useThemeStore.ts（主题名 / 浅色模式 / 自定义主题色）

| 位置 | 项 | 说明 |
|---|---|---|
| `useThemeStore.ts:9-17` | `interface ThemeStore` | store 契约 |
| `useThemeStore.ts:19-50` | `useThemeStore` | store 实体 |
| `useThemeStore.ts:53-86` | `useThemeStore.subscribe(...)` | 模块级副作用：同步 `<html>` 的 `dark` class、`data-light` 属性、`theme-*` class 与 `--primary/--ring/--accent/--accent-foreground` CSS 变量 |

### 动画与渲染性能 hook

| 位置 | 项 | 说明 |
|---|---|---|
| `useAnimationProfile.ts:7` | `type AnimationTier` | `'high' \| 'standard' \| 'economy'` |
| `useAnimationProfile.ts:9-27` | `interface AnimationProfile` | 动画档位契约（字段见"结构体与字段"节） |
| `useAnimationProfile.ts:30-48` | `HIGH_PROFILE` | 高档基线（满配） |
| `useAnimationProfile.ts:52-58` | `ECONOMY_OVERRIDES` | 经济档覆盖项：`willChangeOrbs`/`enableTilt`/`startupBoost`/`numberDuration`（50-51 注释明确只覆盖当前被组件消费的字段） |
| `useAnimationProfile.ts:60-65` | `resolveTier(gpuTier, reducedMotion)` | reduced-motion 或 `low-igpu` → economy；`discrete`/`high-igpu` → high；其余 → standard |
| `useAnimationProfile.ts:69-71` | `reducedMotionQuery` | 模块级 `MediaQueryList`，由 `change` 事件驱动重算 |
| `useAnimationProfile.ts:73-95` | `useAnimationProfile()` | 组合 `refreshRate`/`gpuInfo`（74-75）+ reduced-motion 状态 → `AnimationProfile` |
| `usePageIdle.ts:3-49` | `usePageIdle()`（**未导出**） | 2s 空闲判定（`IDLE_TIMEOUT=2000`、500ms 轮询 18-20），切 `document.body` 的 `anim-idle` class（44-46） |
| `usePageIdle.ts:51-61` | `usePageVisible()`（**未导出**） | `visibilitychange` → 是否可见 |
| `usePageIdle.ts:63-78` | `useWindowFocused()`（**未导出**） | `focus`/`blur` → 是否聚焦 |
| `usePageIdle.ts:83` | `AnimationActiveContext` | `createContext<boolean>(true)` |
| `usePageIdle.ts:85-93` | `AnimationActiveProvider` | 顶层唯一实例，`useMemo` 稳定 value（90），用 `createElement` 表达 Provider（92，`.ts` 无 JSX） |
| `usePageIdle.ts:95-97` | `useAnimationActive()` | `useContext(AnimationActiveContext)` |
| `useStartupBoost.ts:5-11` | `interface StartupRefs` | `titleBar`/`statusBar`/`title`/`dockNav`/`rightPanel` |
| `useStartupBoost.ts:13` | `TRANSFORM_KEYS` | 上述 5 个 key 的常量数组 |
| `useStartupBoost.ts:15-137` | `useStartupBoost()` | 返回 `{ setRef, runStartupSequence, refs }`；入场时间线见 85-123 |
| `usePulseAnimation.ts:5-7` | `interface PulseOptions` | `type: 'heartbeat' \| 'statusPulse' \| 'loadingPulse'` |
| `usePulseAnimation.ts:15-74` | `usePulseAnimation(options)` | 三种 GSAP 时间线（27-52）；空闲时 `pause`/活跃时 `resume`（63-71） |
| `useBreatheAnimation.ts:5-13` | `interface BreatheOptions` | 透明度/缩放/旋转区间与时长 |
| `useBreatheAnimation.ts:15-66` | `useBreatheAnimation(options)` | `yoyo + repeat:-1` 呼吸动画 |
| `useGlowAnimation.ts:5-9` | `interface GlowOptions` | 时长、最大缩放、最大透明度 |
| `useGlowAnimation.ts:11-53` | `useGlowAnimation(options)` | 光晕脉冲 |
| `useGpuCorrection.ts:5` | `cachedWebGlRenderer` | 会话级缓存（WebGL context 有数量上限） |
| `useGpuCorrection.ts:7-24` | `getWebGlRenderer()` | 优先 `webgl`、回退 `experimental-webgl`，读 `WEBGL_debug_renderer_info` |
| `useGpuCorrection.ts:26-30` | `parseWebGlGpu(renderer)` | 从 `ANGLE (vendor, model...)` 中正则提取 |
| `useGpuCorrection.ts:32-54` | `classifyTierFromWebGl(vendor, model)` | 按厂商/型号关键字映射到 `GpuTier` |
| `useGpuCorrection.ts:56-78` | `correctGpuInfoWithWebGl(wmiInfo)` | WMI 报独显但 WebGL 报别家（集显）时以 WebGL 结果为准 |
| `useGpuCorrection.ts:80-82` | `useGpuCorrection()` | 返回 `correctGpuInfoWithWebGl`（无状态，纯函数包装） |
| `useAsyncLock.ts:3-27` | `useAsyncLock(fn, cooldownMs = 1500)` | 返回 `[isRunning, execute]`；`lockRef` 短路重入（19），`finally` 内 `setTimeout` 释放（22-24）；effect 二次 setup 恢复 `mountedRef`（12-17） |

### 初始化与全局副作用

| 位置 | 项 | 说明 |
|---|---|---|
| `useAppInit.ts:6-11` | `useAppInit()` | 固定顺序调用 `useEventListeners()`(7) → `useInitialDataLoad()`(8) → `useHeartbeat()`(9) → `useGlobalShortcut()`(10)。调用点：`App.tsx:114` |
| `useInitialDataLoad.ts:16` | `VALID_PANELS` | 由 `NAV_ITEMS.map(i => i.id)` 派生，用于校验 `localStorage` 里的面板名 |
| `useInitialDataLoad.ts:18-173` | `useInitialDataLoad()` | 启动数据装载（细节见 Data Flow） |
| `useEventListeners.ts:15` | `EMPTY_ADAPTER_STATUSES` | 模块级空数组常量，避免 `adapterStatuses` 为空时每次产生新引用（P2-F6） |
| `useEventListeners.ts:18-368` | `useEventListeners()` | 注册 14 个后端事件 + 窗口关闭拦截（50-75） |
| `useHeartbeat.ts:5-21` | `useHeartbeat()` | 挂载时立即发一次 + 每 5s 一次 `render_heartbeat`（11-14）；`document.hidden` 时跳过（8-10）；`isRenderLoopAlive()` 为假时跳过（13） |
| `useGlobalShortcut.ts:4-26` | `useGlobalShortcut()` | 全局 `Ctrl+Shift+C` → `cancel_auto_exit`；`INPUT`/`TEXTAREA`/`contentEditable` 上不触发（10-16） |

### 兼容壳与测试

| 位置 | 项 | 说明 |
|---|---|---|
| `useAppStore.ts:1-3` | re-export | 仅 `useAppInit`、`hasPendingConfig`、`flushPendingConfig` 三个 re-export，文件首行自述"已拆分为领域 store，此文件仅保留 re-export 以兼容旧引用" |
| `useAsyncLock.test.tsx:20-40` | 用例 | StrictMode 下锁仍能释放、可重复触发（验证 `mountedRef` 二次 setup 恢复） |
| `useConfigStore.selfPassword.test.ts:8-13` | `vi.mock('@/hooks/tauriApi')` | 只 mock `tauriApiWithRetry.saveConfig` |
| `useConfigStore.selfPassword.test.ts:24-46` | 3 个用例 | 非空 `selfPassword` 成功置 `true`(24)；空串不置位(35)；失败不置位(40) |
| `useLogToastStore.test.ts:9-35` | 3 个用例 | 连续重复折叠计数 / 中断连续性 / 级别参与判定 |
| `useLogToastStore.test.ts:43-95` | 4 个用例 | 同 title 去重(43)、带按钮版互斥(52)、超 `MAX_TOASTS` 淘汰(67)、窗口非前台普通 toast 不入队(76) |

## 结构体与字段（前端即类型与 state 结构）

### `AuthStore`（`useAuthStore.ts:115-125`）

| 字段 / action | 类型 | 含义 |
|---|---|---|
| `isLoggingIn` | `boolean` | 登录进行中（初值 `false`，`128`） |
| `isLoggingOut` | `boolean` | 注销进行中（初值 `false`，`129`） |
| `status` | `{ text: string; state: StatusState }` | 状态栏文案与语义态；初值 `{ text: '正在检测...', state: 'loading' }`（130） |
| `bgStatus` | `BackgroundStatus` | 后台巡检状态；初值 `{ isRunning: false, checkCount: 0, serverAvailable: false, online: false, adapterStatuses: [], currentSsid: null }`（131） |
| `doLogin(adapterName?)` | `Promise<boolean>` | 133-192 |
| `doLogout(adapterName?)` | `Promise<void>` | 194-223 |
| `checkOnline(cfg?, adps?)` | `Promise<void>` | 225-293 |
| `setStatus(s)` | `(s) => void` | 295 |
| `setBgStatus(s)` | 值或更新函数 | 296 |

### `ConfigStore`（`useConfigStore.ts:27-51`）

| 字段 / action | 类型 | 含义 |
|---|---|---|
| `config` | `Config` | 初值 `DEFAULT_CONFIG`（54）；`Config` 定义在 `settings/types.ts:3-51`，含 43 个字段（口径：接口本体第 4-50 行的字段总数，含 `configVersion` 标记字段，不含同文件 `AutoLaunchResult` / `InitData`），例如 `user`/`password`/`selfPassword`/`adapter1`/`adapter2`/`dualAdapter`/`themeMode`/`defaultPanel`/`enableNetworkQuality`/`logRetentionDays` 等 |
| `configLoaded` | `boolean` | 初始配置是否装载完成（成功或降级都置 true，见 `useInitialDataLoad.ts:157,165`） |
| `passwordSaved` | `boolean` | 登录密码已保存（决定密码框显示"已保存"占位） |
| `selfPasswordSaved` | `boolean` | 自助服务密码已保存（**独立布尔**，不依赖 `config.selfPassword` 的值，见 34-35 注释） |
| `accounts` | `string[]` | 账号列表（初值 `[]`，58） |
| `activeAccount` | `string` | 当前账号（初值 `''`，59） |
| `language` | `string` | 初值 `safeStorage.get('app-language') \|\| 'zh'`（60） |
| `api` | `typeof api`（即 `TauriApi`） | store 内缓存 IPC 对象（61），供 `useInitialDataLoad`/`useEventListeners`/`useHeartbeat`/`useGlobalShortcut` 取用 |
| `updateConfig(partial)` | `(p: Partial<Config>) => void` | 63-95：立即 `set` + 标脏 + 500ms debounce 保存（84-93）；`customThemeColor` 同步给 `useThemeStore`（94） |
| `updateConfigLocal(partial)` | 同上 | 97-106：只改本地与标脏，不落盘 |
| `mergeConfigFromBackend(incoming)` | `(p: Partial<Config>) => void` | 110-117：跳过 `dirtyFields` 中的键 |
| `clearDirtyFields()` | `() => void` | 119-122 |
| `syncPasswordSaved(saved)` | `(b: boolean) => void` | 124 |
| `syncSelfPasswordSaved(saved)` | `(b: boolean) => void` | 126 |
| `saveConfigDirect(cfg, clearPassword?, clearSelfPassword?)` | `=> Promise<void>` | 128-171：全量合并后调 `api.saveConfig`；成功清脏（134-137）并按需置 `passwordSaved`/`selfPasswordSaved`（140-145）；失败记日志，连续失败达 `DIRTY_FAILURE_LIMIT` 时放弃脏标记（150-160） |
| `setAccounts(a)` / `setActiveAccount(a)` | `=> void` | 173-174 |
| `setLanguage(lang)` | `=> void` | 176-180：写 `safeStorage` + `i18next.changeLanguage` |

### `AdapterStore`（`useAdapterStore.ts:37-46`）

| 字段 / action | 类型 | 含义 |
|---|---|---|
| `adapters` | `Adapter[]` | 初值 `[]`（49） |
| `disabledAdapters` | `DisabledAdapter[]` | 50 |
| `adapterDetails` | `AdapterDetail[]` | 51 |
| `isRefreshingAdapters` | `boolean` | 52；`refreshAdapters` 最少保持 true 500ms（67） |
| `activePanel` | `PanelName` | 初值 `'dashboard'`（53） |
| `refreshAdapters()` | `=> Promise<void>` | 55-70：锁 → `refreshAdapterData({force:true, triggerCheck:true})` → 并行等 500ms → 释放锁 |
| `setAdapters(a)` | `=> void` | 72 |
| `setActivePanel(p)` | `=> void` | 73 |

### `QualityStore`（`useQualityStore.ts:23-41`）

| 字段 / action | 类型 | 含义 |
|---|---|---|
| `networkQuality` | `NetworkQuality \| null` | 初值 `null`（44） |
| `dnsDohStatus` | `DnsDohStatus \| null` | 45 |
| `dnsChecking` | `boolean` | 46 |
| `isRefreshingQuality` | `boolean` | 47 |
| `updateAvailable` | `boolean` | 48 |
| `latestVersion` | `string` | 49 |
| `releaseNotes` | `string` | 50 |
| `gpuInfo` | `GpuInfo \| null` | 51 |
| `refreshRate` | `number` | 初值 0（52），由 `useInitialDataLoad.ts:122-124` 注入 |
| `refreshQuality()` | `=> Promise<void>` | 54-74：`enableNetworkQuality === false` 短路（57）；锁 + `isRefreshingQuality`；成功后走 `setNetworkQuality`（64） |
| `setNetworkQuality(q)` | 值或更新函数 | 78-82：非 null 结果刷新 `lastQualityResultTime` |
| `setDnsDohStatus(s)` | `=> void` | 83 |
| `setDnsChecking(v)` | `=> void` | 84 |
| `setUpdateAvailable(v)` | `=> void` | 85 |
| `setLatestVersion(v)` | `=> void` | 86 |
| `setReleaseNotes(v)` | `=> void` | 87 |
| `setGpuInfo(info)` | `=> void` | 89 |

### `LogToastStore`（`useLogToastStore.ts:15-26`）

| 字段 / action | 类型 | 含义 |
|---|---|---|
| `logs` | `LogEntry[]` | 初值 `[]`（29）；上限 `MAX_LOG_ENTRIES`（`shared/ui-constants.ts:1` = 300） |
| `toasts` | `ToastMessage[]` | 初值 `[]`（30）；上限 `MAX_TOASTS = 4` |
| `addLog(message, type='info')` | `=> void` | 32-51：与最后一条同内容同级别时折叠为 `count + 1`（39-43）；否则追加并在超 `MAX_LOG_ENTRIES` 时裁掉最旧（46-48） |
| `addToast(title, type='info', description?, duration=4000, mascot?)` | `=> void` | 53-83：`document.visibilityState !== 'visible'` 直接 return（57）；同 title 去重（62）；超限淘汰最旧并清其定时器（65-74） |
| `addToastWithAction(toast)` | `=> void` | 85-109：缺省 `duration` 8000（86）；非前台**也**入队（承载"取消退出"入口） |
| `removeToast(id)` | `=> void` | 111-115 |
| `removeToastsByPrefix(prefix)` | `=> void` | 117-127（用于清 `auto-exit-cancel-` / `campus-exit-cancel-` 前缀） |
| `setLogs(logs)` | `=> void` | 129（超限时截尾） |
| `cleanupToasts()` | `=> void` | 131-135 |

### `ThemeStore`（`useThemeStore.ts:9-17`）

| 字段 / action | 类型 | 含义 |
|---|---|---|
| `themeName` | `ThemeName` | 初值 `'default'`（20）；取值见 `shared/ui-types.ts:3` |
| `isLightMode` | `boolean` | 初值由 `safeStorage.get('campus-light-mode') === '1'` 决定（21，IIFE） |
| `customThemeColor` | `string` | 初值 `'#6366f1'`（22） |
| `setThemeName(name)` | `=> void` | 24 |
| `setIsLightMode(v)` | `=> void` | 25 |
| `initTheme(cfg)` | `(cfg: Partial<Config>) => void` | 27-47：`campus-theme` 校验（`VALID_THEMES`，`settings/constants.ts:69`）→ 浅色模式四级回退（localStorage `'1'`/`'0'` → `cfg.themeMode` → `system` 时读 `matchMedia('(prefers-color-scheme: light)')`）→ `cfg.customThemeColor` |
| `setCustomThemeColor(color)` | `=> void` | 49 |

### `AnimationProfile`（`useAnimationProfile.ts:9-27`）

| 字段 | 类型 | 含义 |
|---|---|---|
| `tier` | `AnimationTier` | high / standard / economy |
| `willChangeOrbs` | `boolean` | 是否给光晕加 `will-change`（消费者：`monitor/LatencyComponents.tsx`） |
| `magneticOffset` | `number` | 磁吸位移量（high 档 5） |
| `magneticDuration` | `number` | 磁吸动画时长（0.4） |
| `numberDuration` | `number` | 数字滚动时长（high 600 / economy 350） |
| `springStiffness` / `springDamping` | `number` | 弹簧参数（400 / 18） |
| `powerPreference` | `'low-power' \| 'high-performance'` | WebGL 上下文偏好（high 档 `high-performance`） |
| `prefersCssAnimation` | `boolean` | 是否优先 CSS 动画（high 档 false） |
| `enableGpuCompositing` | `boolean` | GPU 合成开关（true） |
| `enablePageSlide` | `boolean` | 面板滑动（true） |
| `enableTilt` | `boolean` | 卡片 3D 倾斜（economy 覆盖为 false，消费者 `components/ui/animated-card.tsx`） |
| `enableBackdropBlur` | `boolean` | 背景模糊（true） |
| `startupBoost` | `boolean` | 启动入场动画（economy 覆盖为 false，消费者 `useStartupBoost`） |
| `startupStaggerDelay` | `number` | 入场错峰间隔（0.05） |
| `easing` | `EasingConfig` | 由 `refreshRate` 派生（见 [[desktop-frontend-shared]] 的 `lib/easing-config.ts`） |
| `refreshRate` | `number` | 生效刷新率（`refreshRate > 0 ? refreshRate : 120`，86） |

### tauriApi 的局部接口

| 接口 | 行号 | 字段 |
|---|---|---|
| `ConnectionCampusStatus` | 11-15 | `onCampus: boolean`、`name: string \| null`、`message: string` |
| `CampusStatusResult` | 17-25 | `onCampusNetwork: boolean`、`currentSsid: string \| null`、`campusMessage: string`、`enableNetworkNameCheck: boolean`、`requiredNetworkName: string`、`campusWifi: ConnectionCampusStatus \| null`、`campusWired: ConnectionCampusStatus \| null` |

`StartupRefs`（`useStartupBoost.ts:5-11`）与 `PulseOptions`/`BreatheOptions`/`GlowOptions` 字段见上文"动画与渲染性能 hook"表。

## Data Flow

### 链路一：组件 → store → tauriApi.invoke → IPC → Rust

1. **取 api**：store 内统一 `const api = tauriApiWithRetry`（`useAuthStore.ts:17`、`useConfigStore.ts:12`、`useAdapterStore.ts:7`、`useQualityStore.ts:10`）；hook/组件侧从 `useConfigStore.getState().api` 取（`useInitialDataLoad.ts:30`、`useEventListeners.ts:31`、`useHeartbeat.ts:7`、`useGlobalShortcut.ts:6`），或直接静态导入（`account/AccountPanel.tsx:26`、`account/selfServiceState.ts:5`、`auth/DashboardPanel.tsx:32`）。
2. **组件触发**：`DockNav` 取 `useAuthStore(s => s.doLogin)`（`components/layout/DockNav.tsx:413`）并交给登录按钮 `onAction`（539）。
3. **store 内编排**：`doLogin`（`useAuthStore.ts:133-192`）→ 先 `saveConfigDirect`（145）→ `withTimeout(api.doLogin(adapterName), 60000, ...)`（153）→ 写 `status` 与日志/toast（155-163）→ 按 `QUALITY_MANUAL_THROTTLE_MS` 节流后调 `api.checkNetworkQuality()`（169-175）→ 失败才复查 `checkOnline()`（187-189）。
4. **出 IPC**：`tauriApi.doLogin` → `invoke<LoginResult>('do_login', { adapterName })`（`tauriApi.ts:142`）。Rust 侧的 `do_login` 命令见 [[desktop-commands]] 与 [[desktop-auth]]。
5. **返回后写回 store**，组件通过选择器订阅（如 `useAuthStore((s) => s.isLoggingIn)`，`DockNav.tsx:404`）重渲染。

配置写盘的完整链路（防抖 + 脏字段 + in-flight + 关窗 flush）：`updateConfig`（`useConfigStore.ts:63-95`）→ 500ms debounce → `saveConfigDirect`（128）→ `api.saveConfig`（`tauriApi.ts:135`，带重试 260）→ Rust `save_config` 落盘；失败时 `dirtyFailureCounts` 累计，达 3 次放弃脏标记（150-160）。

### 链路二：Rust emit → listen → store → UI

1. **注册**：`useEventListeners` 在 effect 内（`useEventListeners.ts:26-367`）对 14 个事件各调一次 `api.onX(cb)`，unlisten 收进 `unlisteners` 数组（32，逐个 push），cleanup 时统一调用（359-366）。
2. **订阅原语**：`createEventListener`（`tauriApi.ts:102-131`）内部 `listen<T>(eventName, e => { if (cancelled) return; cb(e.payload) })`（107-109）。
3. **写 store**（按事件分类）：
   - `background-check-result`（`useEventListeners.ts:77-196`）：1s 节流（81）→ 主/副适配器在线状态变化检测（87-134，含"已在线"日志 5s 节流 126）→ `useAuthStore.getState().setBgStatus(prev => ...)`（136-181）→ 文本未变则不写 `status`（190-193）。
   - `auto-login-result`（199-212）：写日志/toast 后 `checkOnline()`（211）。
   - `adapters-changed`（215-238）：500ms 节流 + trailing 定时器（226-237）→ 写 `useAdapterStore.setState({ adapters })`（219）；当 `status.state` 为 `offline`/`loading` 时补一次 `checkOnline(undefined, next)`（220-223）。
   - `adapter-details-changed`（242-245）/`disabled-adapters-changed`（248-251）：直接 `useAdapterStore.setState`。
   - `adapter-disabled-warning`（254-260）：warning toast + 日志。
   - `login-log`（263-268）：`useLogToastStore.getState().addLog(data.message, data.type ?? 'info')`。
   - `auto-exit-countdown`（271-289）：日志 + `addToastWithAction`，`id` 前缀 `auto-exit-cancel-`，动作按钮调 `api.cancelAutoExit()`（284）。
   - `auto-exit-cancelled`（292-296）：成功 toast + `removeToastsByPrefix('auto-exit-cancel-')`。
   - `campus-exit-countdown`（299-317）/`campus-exit-cancelled`（320-324）：同上，前缀 `campus-exit-cancel-`。
   - `network-quality-result`（327-332）：先 `handleQualityBadAlert`（34-48，bad 状态跃迁时提示）→ `setNetworkQuality(mergeNetworkQuality(prev, data))`（331）。
   - `update-available`（335-345）：写 `updateAvailable`/`latestVersion`/`releaseNotes` 到 `useQualityStore`（`TitleBar` 消费：`components/layout/TitleBar.tsx:64-65`）。
   - `config-changed`（348-355）：`mergeConfigFromBackend`（**不是** `updateConfigLocal`），避免后端旧快照回滚本地新值。
4. **UI 渲染**：`RightPanel` 订阅 `useLogToastStore` 的 `logs`（由 `App.tsx` 传入 props）、`StatusBar` 订阅 `useAuthStore` 的 `bgStatus`/`status`。

后端 emit 侧的事件名与载荷定义见 [[desktop-infra]]（`events.rs` 是唯一发射出口）。

### 初始化编排顺序

`App.tsx:114` 调 `useAppInit()`（`useAppInit.ts:6-11`），Hook 顺序即注册顺序：

1. `useEventListeners()`（`useAppInit.ts:7`）：**先**注册事件与窗口关闭拦截，保证后续初始化过程中的后端推送不丢。
2. `useInitialDataLoad()`（8），在 `useEffect` 内（`useInitialDataLoad.ts:22-172`）按下列顺序执行：
   1. `api.getInitData()`（34）；
   2. `configLoaded` 之前先按 `PASSWORD_MASK` 判定 `passwordSaved`/`selfPasswordSaved`（38-45）；
   3. `useConfigStore.setState({ config: {...DEFAULT_CONFIG, ...initData.config} })`（37,46）；
   4. `useThemeStore.getState().initTheme(cfg)`（48）；
   5. 恢复/应用活动面板（50-59）：`localStorage` 的 `campus-active-panel` 或 `cfg.defaultPanel`，`enableNetworkQuality === false` 时跳过 `quality`；
   6. 窗口显示（61-65）：`isAutoStart && cfg.hiddenStart` 时不 `showWindow`；
   7. 适配器（67-76）：`initData.adapters` 为空才回退 `getAdapters(false)`；
   8. 后台状态（78-91）、适配器详情（93-94）、禁用列表（96-100）、账号（102-106）；
   9. `useAuthStore.getState().checkOnline(cfg, adps)`（108）；
   10. GPU 信息（110-120，经 `useGpuCorrection` 校正）、`refreshRate`（122-124）；
   11. DNS/DoH 后台探测（126-147，失败或已有值即跳过）；
   12. 末尾置 `configLoaded: true`（157）；异常路径 `showWindow()` + 默认配置 + `configLoaded: true`（162-165）。
3. `useHeartbeat()`（`useAppInit.ts:9`）：立即发一次心跳 + 每 5s 一次。
4. `useGlobalShortcut()`（10）：注册 `Ctrl+Shift+C`。

`useStartupBoost().runStartupSequence` 的调用时机在 `App.tsx:185`，与数据装载并行（不阻塞）。

### 新增一个 IPC 命令 / 事件要改哪几处

新增**请求-响应命令**：

1. `tauri-app/frontend/src/hooks/tauriApi.ts:27-100`：在 `interface TauriApi` 增加方法签名。

   ```ts
   getXxx: (arg: string) => Promise<XxxResult>
   ```

2. `tauri-app/frontend/src/hooks/tauriApi.ts:133-223`：在 `tauriApi` 对象增加实现。

   ```ts
   getXxx: (arg) => invoke<XxxResult>('get_xxx', { arg }),
   ```
3. 若该命令需要重试：`tauri-app/frontend/src/hooks/tauriApi.ts:255-261` 的 `tauriApiWithRetry` 里覆盖（当前仅 `saveConfig`）。
4. 消费侧 store：在目标 store 的 `interface`（如 `useConfigStore.ts:27-51`、`useAuthStore.ts:115-125`）加 action 签名，并在 store 实体（`useConfigStore.ts:53-181`、`useAuthStore.ts:127-297`）内实现。
5. 只读数据若属于启动批次：在 `useInitialDataLoad.ts:32-167` 的 `getInitData` 分支内 `set` 到对应 store；若 Rust 侧 `init_data` 不含该字段，则在此追加一次独立 `api.xxx()` 调用（参照 `getDisabledAdapters` 的写法 96-100）。
6. Rust 侧需在 `invoke_handler` 注册（见 [[desktop-commands]]）。

新增**事件**：

1. `tauriApi.ts:27-100` 加 `onXxx: (cb: (data: T) => void) => () => void`；`tauriApi.ts:133-223` 加 `onXxx: createEventListener<T>('xxx-event')`。
2. `useEventListeners.ts:26-367` 的 effect 内注册并 `unlisteners.push(...)`（如 153-159 的写法）。
3. store 写回点：`setBgStatus`（`useEventListeners.ts:136`）/`useAdapterStore.setState`（219、244、250）/`useQualityStore.getState().setXxx`（331、338-340）/`mergeConfigFromBackend`（354）。
4. 在 `mountedRef.current` 检查与（必要时）节流（参照 81、226）后写状态；Rust 侧 emit 见 [[desktop-infra]]。

## Connections

- [[desktop-frontend-shared]]：本模块消费其 UI 组件（`ToastContainer` 渲染 `useLogToastStore.toasts`）、类型（`shared/ui-types.ts` 的 `PanelName`/`StatusState`/`GpuInfo`）与常量（`MAX_LOG_ENTRIES`、`PASSWORD_MASK`、`NAV_ITEMS`）。
- [[desktop-config]]：`useConfigStore` 与 Rust 配置命令（`get_config`/`save_config`/`config-changed`）一一对应，脏字段与 `PASSWORD_MASK` 语义在此闭环。
- [[desktop-auth]]：`useAuthStore` 的 `doLogin`/`doLogout`/`checkOnline` 对应后端登录链路与 `PortalStatusResult`。
- [[desktop-monitor]]：`background-check-result`、`network-quality-result`、`BackgroundStatus`、`NetworkQuality` 的载荷定义与写入点。
- [[desktop-network-core]] / [[desktop-network-dns]]：`useAdapterStore`、`refreshAdapterData`、`check_dns_doh_status`/`setup_dns_doh`。
- [[desktop-account-selfservice]]：`list_accounts`/`switch_account`/`save_current_as_account`/`delete_account` 与自助服务 6 个命令（`bind_operator`、`query_self_*`、`self_offline_session` 等）。
- [[desktop-helper-update]]：`check_update`/`download_update`/`install_update`/`get_mirror_urls` 与 `update-available`/`update-download-progress`/`update-notification-click` 事件。
- [[desktop-commands]]：本模块是全部 IPC 命令的前端唯一入口，命令面清单与之对齐。
- [[desktop-app-lifecycle]]：`render_heartbeat`（`useHeartbeat.ts`）与关窗 flush（`useEventListeners.ts:50-75` + `useConfigStore.ts:190-220`）对应后端生命周期与 WebView 重载。
- [[desktop-platform]]：`open_external` 的 shell 插件回退（`tauriApi.ts:185-191`）与平台能力相关。
- [[desktop-infra]]：事件名/payload 契约由后端 `events.rs` 定义，前端在此镜像。

## Known Issues

1. **`useAppStore.ts` 是零消费者的死兼容壳**：文件仅 3 行 re-export（`useAppStore.ts:1-3`），全仓库除自身外无任何引用（`grep -rn "useAppStore"` 只命中该文件第 1 行）。可安全删除，但删前需确认没有外部/动态引用。
2. **`usePageIdle` / `usePageVisible` / `useWindowFocused` 未导出**（`usePageIdle.ts:3`、`51`、`63` 均为不带 `export` 的 `function`）：文件名暗示的 `usePageIdle` 对外不可用，只能经 `AnimationActiveProvider`（85）消费；新增消费方必须走 `useAnimationActive`（95），否则会退回"每个调用点注册 8-9 个监听器"的旧缺陷（80-82 注释记录了 FP-2）。
3. **`useQualityStore.refreshQuality` 的锁释放仍用 `setTimeout(500)`**（`useQualityStore.ts:69-72`）：与 `checkOnline`（`useAuthStore.ts:286-292`）和 `refreshAdapters`（`useAdapterStore.ts:65-69`）已改为"实际完成即释放"的模式不一致；若 `check_network_quality` 耗时超过 500ms，锁可能提前释放并允许并发进入。
4. **`_checkOnlineLockFlag` / `_adapterLockFlag` / `_qualityLockFlag` 是模块级变量**（`useAuthStore.ts:19`、`useAdapterStore.ts:9`、`useQualityStore.ts:12`）：多窗口或多 store 实例场景下共享，且窗口销毁不会复位。当前为单窗口设计（`App.tsx` 单实例），扩展多窗口时必须改造。
5. **`addToastWithAction` 的定时器以 `toast.id` 为键登记**（`useLogToastStore.ts:108`）：若调用方传入的 `id` 与已有 toast 重复，`toastTimers.set` 会覆盖旧定时器引用，旧定时器不再被 `removeToast` 清理（`useEventListeners.ts:275`、`303` 用 `Date.now()` 组 id 规避，但约定未在 store 层强制）。
6. **`addToast` 在窗口非前台时静默丢弃**（`useLogToastStore.ts:57`）：信息依赖调用方同步写日志兜底；`addToastWithAction` 例外（承载操作入口）。若新增"必须让用户看到"的普通 toast，会在后台被静默吞掉。
7. **`onAutoExitCancelled` / `onCampusExitCancelled` 的 handler 缺 `mountedRef.current` 检查**（`useEventListeners.ts:292-296`、`320-325`）：其余 handler 一律以 `if (!mountedRef.current) return` 开头（如 271、299、327、335），这两个没有；卸载后事件到达仍会写 `useLogToastStore`（`addLog`/`addToast`/`removeToastsByPrefix`）。单人单窗口下影响仅是"卸载后仍弹 toast"，但这是同文件内唯一的不一致点。
8. **api 方法缺失时静默失效**：所有注册点写成 `api.onX?.(...) ?? (() => {})`（如 `useEventListeners.ts:77/196`、`199/212`、`215/239`），若 `TauriApi` 中某方法被改名或删除，`unlisteners` 里推入的是空函数，无任何报错、也无事件——只有实际推送时才会发现"UI 不更新"。
9. **`withTimeout` 不会取消底层 invoke**（`useAuthStore.ts:108-113`）：超时只是让前端提前解禁按钮，`do_login` 仍在后端继续执行，可能出现"前端已显示超时失败、后端实际登录成功"的错位（由随后的 `checkOnline` 纠正显示）。
10. **`useGpuCorrection` 的 WebGL 结果会话级缓存**（`useGpuCorrection.ts:5`）：`cachedWebGlRenderer` 一旦为 `null`（首次取 context 失败）便永久返回 `null`，驱动/GPU 恢复后不会再探测。
11. **`useGlobalShortcut` 的快捷键硬编码**（`useGlobalShortcut.ts:17`）：`Ctrl+Shift+C` 不支持配置，`Config` 中也没有快捷键字段；前端侧 `cancel_auto_exit` 仅三个入口——`useGlobalShortcut.ts:20`、`useEventListeners.ts:284`（自动退出 toast 按钮）、`useEventListeners.ts:312`（校园网退出 toast 按钮）。
12. **`AnimationProfile` 近半数字段没有消费方**：实测被消费的只有 `tier`、`easing`、`willChangeOrbs`（`monitor/LatencyComponents.tsx:135,144,161`）、`enableTilt`（`components/ui/animated-card.tsx:36`）、`startupBoost`（`useStartupBoost.ts:40,48,69`）、`numberDuration`（`shared/AnimatedNumber.tsx:23`）、`startupStaggerDelay`（`useStartupBoost.ts:66`）。以下字段全仓库仅出现在 `useAnimationProfile.ts` 定义处：`magneticOffset`(12)、`magneticDuration`(13)、`springStiffness`(15)、`springDamping`(16)、`powerPreference`(17)、`prefersCssAnimation`(18)、`enableGpuCompositing`(19)、`enablePageSlide`(20)、`enableBackdropBlur`(22)。50-51 行的注释只声明 `ECONOMY_OVERRIDES` 覆盖"当前被组件消费的高开销字段"，但未声明这些字段是死字段——新消费方接入前，改它们不会有任何效果。同理 `AnimationProfile.refreshRate`(26) 也无外部消费方（仅 `useAnimationProfile.ts:86-89` 内部用于算 `easing`）。
13. **`useStartupBoost` 的 `boostedRef` 只在卸载时复位**（`useStartupBoost.ts:128-134`）：`StrictMode` 下 setup→cleanup→setup 会复位并允许重跑，但生产环境一旦 `runStartupSequence` 执行过，再次调用即被短路（62-63）——面板切换不会重播入场动画属预期行为。
14. **`useInitialDataLoad` 的 DNS 探测失败静默**（`useInitialDataLoad.ts:146`）：仅 DEV 下 `console.error`，没有用户可见提示（与 `getInitData` 失败路径不同，后者写日志 164）。
15. **`useEventListeners` 的 `mountedRef` 依赖手工复位**（`useEventListeners.ts:29`，注释 27-28）：任何新增 effect 若忘记在 setup 时置 `true`，dev 模式下所有事件 handler 会静默失效（`useAsyncLock.ts:12-17` 是同类修复的第二处，`shared/LogPanel.tsx:106-113` 是第三处）。
