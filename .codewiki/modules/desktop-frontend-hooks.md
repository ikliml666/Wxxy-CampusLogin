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
  - tauri-app/frontend/src/hooks/useEventListeners.ts
  - tauri-app/frontend/src/hooks/useGlobalShortcut.ts
  - tauri-app/frontend/src/hooks/useGlowAnimation.ts
  - tauri-app/frontend/src/hooks/useHeartbeat.ts
  - tauri-app/frontend/src/hooks/useInitialDataLoad.ts
  - tauri-app/frontend/src/hooks/useLogToastStore.ts
tags: [前端, hooks, zustand, IPC, Tauri, 事件监听, 状态管理, 初始化编排, 动画档位]
---

## Overview

`tauri-app/frontend/src/hooks/` 是桌面前端的“业务内核”：`tauriApi.ts` 是全应用唯一的 IPC 出口（包含请求‑响应命令与事件订阅）；其余文件按领域拆成多个 zustand store（auth / config / adapter / log‑toast），加上动画档位、渲染心跳、初始化编排等横切 hook。

本目录不渲染 UI，只负责三件事：把 Rust 后端的能力包装成 Promise API、把后端推送的事件折叠成 store 状态、在启动时按固定顺序组装这些 store（`useAppInit`）。所有 UI 组件都通过 store 订阅或 `tauriApiWithRetry` 直接调用来消费本模块。

当前代码中，`tauriApiWithRetry`（定义于 `tauriApi.ts:314-319`）仅对 `saveConfig`（`tauriApi.ts:319`）保留指数退避重试；动画档位自 v2.4.0 起恒为最高档（`useAnimationProfile.ts:28-49` 删除分档逻辑，`tier` 字段保留以维持接口形状）；多个历史缺陷（锁释放竞态、StrictMode `mountedRef` 复位、`config-changed` 脏字段合并等）已在各 store 与监听器中修复。

## Key Components

### 目录总览（16 个文件）

| 文件 | 类别 | 主要导出（行号） |
|---|---|---|
| `tauriApi.ts` | IPC 网关 | `tauriApi`(177)、`tauriApiWithRetry`(314)、`createEventListener`(146)、`withRetry`(296)、`isRetryableError`(284) |
| `useAuthStore.ts` | 领域 store | `useAuthStore`(149) |
| `useConfigStore.ts` | 领域 store | `useConfigStore`(51)、`hasPendingConfig`(180)、`flushPendingConfig`(187) |
| `useAdapterStore.ts` | 领域 store | `useAdapterStore`(45)、`refreshAdapterData`(16) |
| `useLogToastStore.ts` | 领域 store | `useLogToastStore`(28) |
| `useAppInit.ts` | 编排 | `useAppInit`(6) |
| `useInitialDataLoad.ts` | 编排 | `useInitialDataLoad`(18) |
| `useEventListeners.ts` | 编排 | `useEventListeners`(18) |
| `useHeartbeat.ts` | 全局副作用 | `useHeartbeat`(5) |
| `useGlobalShortcut.ts` | 全局副作用 | `useGlobalShortcut`(4) |
| `useAnimationProfile.ts` | 性能/动画 | `useAnimationProfile`(51) |
| `useBreatheAnimation.ts` | 动画 | `useBreatheAnimation`(20) |
| `useGlowAnimation.ts` | 动画 | `useGlowAnimation`(15) |
| `useAsyncLock.ts` | 工具 | `useAsyncLock`(3) |
| `useAppStore.ts` | 兼容壳 | 仅 re-export（1-3） |
| `useAsyncLock.test.tsx` | 测试 | 1 个用例（20-40） |

### tauriApi.ts — 唯一 IPC 出口

#### 接口定义

| 位置 | 项 | 说明 |
|---|---|---|
| `tauriApi.ts:11-15` | `interface ConnectionCampusStatus` | 单个连接（WiFi / 有线）的校园网判定：`onCampus`、`name`、`message` |
| `tauriApi.ts:20-27` | `export interface AdapterCampusStatus` | 逐卡校园网判定结果：`name`、`wireless`、`onCampus`、`matchedBy`、`message` |
| `tauriApi.ts:187-197` | `interface CampusStatusResult` | `check_campus_status` 回包：`onCampusNetwork`、`currentSsid`、`campusMessage`、`enableNetworkNameCheck`、`requiredNetworkName`、`campusWifi`、`campusWired`、`adapters?` |
| `tauriApi.ts:39-144` | `interface TauriApi` | 全部成员的签名（请求‑响应 + 事件订阅） |

#### 核心实现

| 位置 | 项 | 说明 |
|---|---|---|
| `tauriApi.ts:148-177` | `const createEventListener<T>(eventName)` | 事件订阅工厂：返回 `(cb) => unlisten`；内部用 `cancelled` 标志处理“注册未完成即取消”的竞态（`listen` 的 Promise 尚未 resolve 时调用 unlisten，则在 Promise 完成后补调 `fn?.()`，`tauriApi.ts:148-173`，补调在 `171`）；注册失败仅 DEV 下 `console.error` 并返回 `null`（`tauriApi.ts:148-151`） |
| `tauriApi.ts:177-282` | `const tauriApi: TauriApi` | 逐方法实现：`invoke<返回类型>('命令名', { 参数 })` 或 `createEventListener<载荷>('事件名')` |
| `tauriApi.ts:284-294` | `function isRetryableError(e)` | 小写化后匹配 `timeout`/`network`/`fetch`/`connection` 四个子串 |
| `tauriApi.ts:296-312` | `async function withRetry(fn, maxRetries=2, baseDelay=500)` | 指数退避 + 0‑200ms 抖动（`tauriApi.ts:304`） |
| `tauriApi.ts:314-319` | `export const tauriApiWithRetry` | `{...tauriApi}` 后**只**覆盖 `saveConfig`（`tauriApi.ts:319`）。注释明确说明为何不再包 `checkPortalStatus`/`checkNetworkQuality`（后者有后端 latency loop 兜底，包重试会放大高频流量） |

#### 请求‑响应 API

| 行号 | 前端方法 | IPC 命令 | 参数 / 返回 |
|---|---|---|---|
| 178 | `getConfig()` | `get_config` | → `Config` |
| 179 | `saveConfig(config, clearPassword?)` | `save_config` | → `SaveConfigResult` |
| 180 | `exportConfig(includePassword?)` | `export_config` | → `string` |
| 181 | `importConfig(path)` | `import_config` | → `CommandResult` |
| 182 | `exportDiagnostics(days?)` | `export_diagnostics` | → `string` |
| 183 | `getAdapters(force?)` | `get_adapters` | → `Adapter[]` |
| 184 | `enableAdapter(adapterName)` | `enable_adapter` | → `EnableAdapterResult` |
| 185 | `getAdapterDetails()` | `get_adapter_details` | → `AdapterDetail[]` |
| 186 | `checkPortalStatus(adapterIp)` | `check_portal_status` | → `PortalStatusResult` |
| 187 | `checkCampusStatus()` | `check_campus_status` | → `CampusStatusResult` |
| 188 | `doLogin(adapterName?)` | `do_login` | → `LoginResult` |
| 189 | `doLogout(adapterName?)` | `do_logout` | → `LoginResult` |
| 190 | `bindOperator({account,password,operator,phone,smsPassword})` | `bind_operator` | → `CommandResult` |
| 191 | `getBindStatus({account,password})` | `query_bind_status` | → `CommandResult` |
| 192 | `verifyWindowsIdentity({consentMessage})` | `verify_windows_identity` | → `CommandResult` |
| 193 | `revealOperatorCredential({account,password,operator})` | `reveal_operator_credential` | → `CommandResult` |
| 194 | `querySelfDashboard({account,password})` | `query_self_dashboard` | → `CommandResult` |
| 195 | `selfOfflineSession({account,password,sessionId})` | `self_offline_session` | → `CommandResult` |
| 196 | `querySelfOnlineLog({account,password,startTime,endTime})` | `query_self_online_log` | → `CommandResult` |
| 197 | `changeSelfPassword({account,password,newPassword})` | `change_self_password` | → `CommandResult` |
| 198 | `vpnSaveCredential({account,password})` | `vpn_save_credential` | → `CommandResult` |
| 199 | `vpnCredentialStatus()` | `vpn_credential_status` | → `CommandResult` |
| 200 | `vpnClearCredential()` | `vpn_clear_credential` | → `CommandResult` |
| 201 | `setSelfServiceViaVpn(enabled)` | `set_self_service_via_vpn` | → `CommandResult` |
| 202 | `vpnTestConnection()` | `vpn_test_connection` | → `CommandResult` |
| 203 | `vpnVerifyCredential({account,password})` | `vpn_verify_credential` | → `CommandResult` |
| 204 | `minimizeWindow()` | `minimize_window` | → `void` |
| 205 | `closeWindow()` | `close_window` | → `void` |
| 212 | `listAccounts()` | `list_accounts` | → `AccountItem[]` |
| 213 | `createAccount({accountName,user,password,operator})` | `create_account` | → `AccountResult` |
| 214 | `switchAccount(accountName)` | `switch_account` | → `SwitchAccountResult` |
| 215 | `saveCurrentAsAccount(accountName)` | `save_current_as_account` | → `SaveAccountResult` |
| 216 | `deleteAccount(accountName)` | `delete_account` | → `DeleteAccountResult` |
| 217 | `renameAccount({accountId,displayName})` | `rename_account` | → `AccountResult`（重名等校验失败 `success=false`） |
| 218 | `getActiveAccount()` | `get_active_account` | → `string` |
| 219 | `startBackgroundCheck()` | `start_background_check` | → `CommandResult` |
| 220 | `stopBackgroundCheck()` | `stop_background_check` | → `CommandResult` |
| 221 | `triggerBackgroundCheck()` | `trigger_background_check` | → `CommandResult` |
| 222 | `getBackgroundStatus()` | `get_background_status` | → `BackgroundStatus` |
| 223 | `dhcpRenewAll()` | `dhcp_renew_all` | → `DhcpRenewResult` |
| 224 | `dhcpReleaseRenew()` | `dhcp_release_renew` | → `DhcpReleaseRenewResult` |
| 225 | `dhcpReleaseRenewAdapter(adapterName)` | `dhcp_release_renew_adapter` | → `DhcpReleaseRenewResult` |
| 226 | `outboundSwitchNow()` | `outbound_switch_now` | → `CommandResult`（绕过自动切换退避闸） |
| 227 | `outboundRestoreNow()` | `outbound_restore_now` | → `CommandResult`（清当日保持标记并还原） |
| 228 | `getCurrentOutboundName()` | `get_current_outbound_name` | → `string \| null`（默认路由 metric 求和最小者） |
| 229 | `checkNetworkQuality()` | `check_network_quality` | → `NetworkQuality` |
| 230 | `onNetworkQualityResult` | 事件订阅（见下） | — |
| 231 | `startLatencyTest()` | `start_latency_test` | → `CommandResult` |
| 232 | `stopLatencyTest()` | `stop_latency_test` | → `CommandResult` |
| 233-250 | `openExternal(url)` | `open_external` | 前置校验：仅 `http://`/`https://`、长度 ≤2048、`new URL()` 可解析；invoke 失败时回退 `@tauri-apps/plugin-shell` 的 `open`，两者都失败返回 `false` |
| 251 | `setAutoLaunch(enabled)` | `set_auto_launch` | → `AutoLaunchResult` |
| 252 | `getCoreAutostart()` | `get_core_autostart` | → `boolean` |
| 253 | `setCoreAutostart(enabled)` | `set_core_autostart` | → `CoreAutostartResult` |
| 254 | `setNotificationEnabled(enabled)` | `set_notification_enabled` | → `boolean` |
| 255 | `cancelAutoExit()` | `cancel_auto_exit` | → `CommandResult` |
| 263 | `showWindow()` | `show_window` | → `void` |
| 264 | `getLogs(lines?)` | `get_logs` | → `string`（整块文本） |
| 265 | `clearLogs()` | `clear_logs` | → `boolean` |
| 266 | `setDebugMode(enabled)` | `set_debug_mode` | → `boolean` |
| 267 | `getInitData()` | `get_init_data` | → `InitData` |
| 268 | `checkUpdate()` | `check_update` | → `UpdateInfo` |
| 269 | `downloadUpdate(url)` | `download_update` | → `string`（落盘路径） |
| 270 | `cancelDownloadUpdate()` | `cancel_download_update` | → `boolean` |
| 271 | `installUpdate(filePath, checksumUrl?)` | `install_update` | → `boolean` |
| 272 | `getMirrorUrls(githubUrl)` | `get_mirror_urls` | → `MirrorSource[]` |
| 276 | `checkDnsDohStatus()` | `check_dns_doh_status` | → `DnsDohStatus` |
| 277 | `setupDnsDoh(family?)` | `setup_dns_doh` | `family?: 'ipv4'\|'ipv6'\|'both'` → `DnsSetupResult` |
| 278 | `resetDns()` | `reset_dns` | → `CommandResult` |
| 279 | `renderHeartbeat()` | `render_heartbeat` | → `{ online: boolean; checking: boolean }` |
| 280 | `notifyWindowReady()` | `notify_window_ready` | → `CommandResult` |
| 281 | `setLogRetentionDays(days)` | `set_log_retention_days` | → `void` |

#### 事件订阅

| 行号 | 前端方法 | 事件名 | 载荷类型 | 主要消费点 |
|---|---|---|---|---|
| 206 | `onBackgroundCheckResult` | `background-check-result` | `BackgroundCheckEventData` | `useEventListeners.ts` |
| 207 | `onAutoLoginResult` | `auto-login-result` | `AutoLoginEventData` | `useEventListeners.ts` |
| 208 | `onAdaptersChanged` | `adapters-changed` | `Adapter[]` | `useEventListeners.ts` |
| 209 | `onAdapterDetailsChanged` | `adapter-details-changed` | `AdapterDetail[]` | `useEventListeners.ts` |
| 210 | `onAdapterDisabledWarning` | `adapter-disabled-warning` | `AdapterDisabledWarningData` | `useEventListeners.ts` |
| 211 | `onLoginLog` | `login-log` | `{ message: string; type: string }` | `useEventListeners.ts` |
| 230 | `onNetworkQualityResult` | `network-quality-result` | `NetworkQuality` | `useEventListeners.ts` |
| 256 | `onAutoExitCountdown` | `auto-exit-countdown` | `AutoExitCountdownData` | `useEventListeners.ts` |
| 257 | `onAutoExitCancelled` | `auto-exit-cancelled` | `Record<string, never>`（无载荷） | `useEventListeners.ts` |
| 258 | `onCampusExitCountdown` | `campus-exit-countdown` | `{ minimizeDelay: number; exitDelay: number }` | `useEventListeners.ts` |
| 259 | `onCampusExitCancelled` | `campus-exit-cancelled` | `Record<string, never>`（无载荷） | `useEventListeners.ts` |
| 260 | `onScheduledActionSkipped` | `scheduled-action-skipped` | `{ kind: 'login' \| 'logout' }` | `useEventListeners.ts` |
| 261 | `onMacResetCycle` | `mac-reset-cycle` | `Record<string, never>`（无载荷） | `useEventListeners.ts` |
| 262 | `onConfigChanged` | `config-changed` | `{ config: Config }` | `useEventListeners.ts` |
| 273 | `onDownloadProgress` | `update-download-progress` | `DownloadProgress` | 组件内（如 AboutDialog） |
| 274 | `onUpdateAvailable` | `update-available` | `UpdateAvailableData` | `useEventListeners.ts` |
| 275 | `onUpdateNotificationClick` | `update-notification-click` | `unknown` | 组件内（App.tsx 等） |

> 17 个事件中 15 个由 `useEventListeners` 统一注册；`onDownloadProgress`（弹窗内进度）与 `onUpdateNotificationClick`（点击系统通知后打开“关于”）在各自组件内按需注册。

### 领域 store

#### useAuthStore.ts（认证 / 在线状态 / 后台巡检状态）

| 位置 | 项 | 说明 |
|---|---|---|
| `useAuthStore.ts:17` | `const api` | `tauriApiWithRetry` 别名 |
| `useAuthStore.ts:19` | `_checkOnlineLockFlag` | 模块级 `checkOnline` 互斥锁（非 React state） |
| `useAuthStore.ts:24` | `QUALITY_MANUAL_THROTTLE_MS = 60_000` | 登录后手动质量探测节流阈值，与后端 `run_quality_check` 60s 节流对齐 |
| `useAuthStore.ts:26-27` | 类型别名 | `CampusStatus`、`PortalStatus` |
| `useAuthStore.ts:32-38` | `detectCampusNetwork()` | 调 `check_campus_status`，异常返回 `null` |
| `useAuthStore.ts:41-75` | `buildCampusBgStatusPatch(...)` | 纯函数：优先取逐卡判定结果，回退整类聚合推断，拼 `bgStatus` 补丁，含 `?? bgStatus.xxx` 兜底 |
| `useAuthStore.ts:78-89` | `pickAdapterIp(adapters, adapter1)` | 指定适配器有 IP 则用；否则优先有线、再无线、再第一个；都没有返回 `''` |
| `useAuthStore.ts:92-102` | `refreshAdaptersForIp(adapter1)` | 列表无 IP 时 `getAdapters(true)` 强制重探 |
| `useAuthStore.ts:105-114` | `queryPortalStatus(adapterIp)` | 返回判别联合 `{ok:true,portal} \| {ok:false}` |
| `useAuthStore.ts:118-127` | `withTimeout(promise, ms, timeoutMsg)` | `Promise.race` 超时包装，防 `isLoggingIn/isLoggingOut` 永久为 true（历史缺陷 P2‑34）；`finally` 清理定时器 |
| `useAuthStore.ts:131-135` | `setStatusStable(next)` | 文本/状态未变时不换新对象，保持引用稳定 |
| `useAuthStore.ts:137-147` | `interface AuthStore` | store 契约 |
| `useAuthStore.ts:149-319` | `useAuthStore` | store 实体（`doLogin` 155‑214、`doLogout` 216‑245、`checkOnline` 247‑315、`setStatus` 317、`setBgStatus` 318） |

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
| `useConfigStore.ts:27-49` | `interface ConfigStore` | store 契约 |
| `useConfigStore.ts:51-178` | `useConfigStore` | store 实体 |
| `useConfigStore.ts:180-185` | `hasPendingConfig()` | `saveConfigPending !== null \|\| saveConfigInFlight !== null` |
| `useConfigStore.ts:187-216` | `flushPendingConfig()` | 清 debounce 定时器 → 发出待存配置（`PASSWORD_MASK` 字段删除后再拼全量）→ 与 in‑flight 一起 `Promise.allSettled` 返回 |

#### useAdapterStore.ts（适配器数据 / 当前面板）

| 位置 | 项 | 说明 |
|---|---|---|
| `useAdapterStore.ts:9` | `_adapterLockFlag` | `refreshAdapters` 互斥锁 |
| `useAdapterStore.ts:16-32` | `refreshAdapterData({force, triggerCheck})` | 公共刷新动作：并行 `getAdapters`/`getAdapterDetails`（22‑25），写回 store（26‑27），可选触发后台检测（28）。收敛了历史上三处重复实现（P2‑F10） |
| `useAdapterStore.ts:34-43` | `interface AdapterStore` | store 契约 |
| `useAdapterStore.ts:45-71` | `useAdapterStore` | store 实体（`refreshAdapters` 51‑66 锁释放与 500ms 最短展示） |

#### useLogToastStore.ts（应用内日志 + toast）

| 位置 | 项 | 说明 |
|---|---|---|
| `useLogToastStore.ts:5` | `toastTimers` | toast 自动消失定时器表，`removeToast`/`cleanupToasts` 需同步清理 |
| `useLogToastStore.ts:6,7` | `toastIdCounter` / `logIdCounter` | 自增 ID |
| `useLogToastStore.ts:9` | `MAX_TOASTS = 4` | toast 上限，超出淘汰最旧 |
| `useLogToastStore.ts:13` | `isDuplicateTitle(toasts, title)` | 同标题去重（防“专用事件 + system‑notification”双通道重复弹） |
| `useLogToastStore.ts:15-26` | `interface LogToastStore` | store 契约 |
| `useLogToastStore.ts:28-136` | `useLogToastStore` | store 实体 |

### 动画与渲染性能 hook

| 位置 | 项 | 说明 |
|---|---|---|
| `useAnimationProfile.ts:6` | `type AnimationTier` | `'high' \| 'standard' \| 'economy'`（现仅 high 实际使用） |
| `useAnimationProfile.ts:8-26` | `interface AnimationProfile` | 动画档位契约（字段见“结构体与字段”节） |
| `useAnimationProfile.ts:28-49` | `HIGH_PROFILE` | 高档基线（满配），v2.4.0 起恒最高档，standard/economy 分档已删除 |
| `useAnimationProfile.ts:51-57` | `useAnimationProfile()` | 组合 `refreshRate` → `AnimationProfile`，`refreshRate>0?refreshRate:120` 驱动 `easing` |
| `useBreatheAnimation.ts:5-13` | `interface BreatheOptions` | 透明度/缩放/旋转区间与时长 |
| `useBreatheAnimation.ts:20-69` | `useBreatheAnimation(options)` | `yoyo + repeat:-1` 呼吸动画；回调 ref 模式支持条件渲染重建（21 行 `setNode` state） |
| `useGlowAnimation.ts:5-9` | `interface GlowOptions` | 时长、最大缩放、最大透明度 |
| `useGlowAnimation.ts:15-56` | `useGlowAnimation(options)` | 光晕脉冲；回调 ref 模式支持条件渲染重建 |
| `useAsyncLock.ts:3-27` | `useAsyncLock(fn, cooldownMs = 1500)` | 返回 `[isRunning, execute]`；`lockRef` 短路重入（19），`finally` 内 `setTimeout` 释放（22‑24）；effect 二次 setup 恢复 `mountedRef`（12‑17） |

### 初始化与全局副作用

| 位置 | 项 | 说明 |
|---|---|---|
| `useAppInit.ts:6-11` | `useAppInit()` | 固定顺序调用 `useEventListeners()`(7) → `useInitialDataLoad()`(8) → `useHeartbeat()`(9) → `useGlobalShortcut()`(10) |
| `useInitialDataLoad.ts:16` | `VALID_PANELS` | 由 `NAV_ITEMS.map(i => i.id)` 派生，用于校验面板名 |
| `useInitialDataLoad.ts:18-167` | `useInitialDataLoad()` | 启动数据装载（细节见 Data Flow） |
| `useEventListeners.ts:15` | `EMPTY_ADAPTER_STATUSES` | 模块级空数组常量，避免 `adapterStatuses` 为空时每次产生新引用（P2‑F6） |
| `useEventListeners.ts:18-383` | `useEventListeners()` | 注册 15 个后端事件 + 窗口 blur 兜底 flush（55‑57），无 `onCloseRequested` 注册（注释 49‑54） |
| `useHeartbeat.ts:5-19` | `useHeartbeat()` | 挂载时立即发一次 + 每 5s 一次 `render_heartbeat`（14）；`document.hidden` 时跳过（8‑9）；`isRenderLoopAlive()` 为假时跳过（12） |
| `useGlobalShortcut.ts:4-24` | `useGlobalShortcut()` | 全局 `Ctrl+Shift+C` → `cancel_auto_exit`；`INPUT`/`TEXTAREA`/`contentEditable` 上不触发（10‑15） |

### 兼容壳与测试

| 位置 | 项 | 说明 |
|---|---|---|
| `useAppStore.ts:2-3` | re-export | 仅 `useAppInit`、`hasPendingConfig`、`flushPendingConfig` 三个 re-export，文件首行自述“已拆分为领域 store，此文件仅保留 re-export 以兼容旧引用” |
| `useAsyncLock.test.tsx:20-40` | 用例 | StrictMode 下锁仍能释放、可重复触发（验证 `mountedRef` 二次 setup 恢复） |

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                        App.tsx                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │useAppInit()   │  │useStartupBoost│  │  UI Components    │  │
│  │              │  └──────────────┘  │  (DockNav, etc.)  │  │
│  │► listeners   │                   │  ┌──────────────┐  │  │
│  │► init data   │                   │  │ zustand Store│  │  │
│  │► heartbeat   │                   │  │ (auth/config │  │  │
│  │► shortcut    │                   │  │  /adapter/etc)│  │  │
│  └──────┬───────┘                   │  └───────┬──────┘  │  │
└─────────┼───────────────────────────┼───────────┼────────┘  │
          │                           │           │           │
          ▼                           ▼           ▼           │
┌─────────────────────────────────────────────────────────────┤
│                    tauriApi.ts                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │ invoke()     │  │ listen()     │  │ createEventListener│ │
│  │ (request)    │  │ (event)      │  │                  │  │
│  └──────┬───────┘  └──────┬───────┘  └────────┬─────────┘  │
└─────────┼─────────────────┼───────────────────┼────────────┘
          │                 │                   │
          ▼                 ▼                   ▼
┌─────────────────────────────────────────────────────────────┐
│                     Tauri / Rust Backend                     │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │ invoke_handler│  │ events.rs   │  │ WebView / Shell  │  │
│  │ (commands)    │  │ (emit)      │  │                  │  │
│  └──────────────┘  └──────────────┘  └──────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

### 链路一：组件 → store → tauriApi.invoke → IPC → Rust

1. **取 api**：store 内统一 `const api = tauriApiWithRetry`（`useAuthStore.ts:17`、`useConfigStore.ts:12`、`useAdapterStore.ts:7`）；hook/组件侧一律静态导入 `import { tauriApiWithRetry as api } from './tauriApi'`（`useInitialDataLoad.ts:52`、`useEventListeners.ts:53`、`useHeartbeat.ts:2`、`useGlobalShortcut.ts:2`、各 `useXxx` 钩子、`App.tsx` 等）；或直接静态导入（如组件内）。
2. **组件触发**：`DockNav` 取 `useAuthStore(s => s.doLogin)` 并交给登录按钮 `onAction`。
3. **store 内编排**：`doLogin`（`useAuthStore.ts:155-214`）→ 先 `saveConfigDirect`（167）→ `withTimeout(api.doLogin(adapterName), 60000, ...)`（175）→ 写 `status` 与日志/toast（177-184）→ 按 `QUALITY_MANUAL_THROTTLE_MS` 节流后调 `api.checkNetworkQuality()`（191-197）→ 失败才复查 `checkOnline()`（209-211）。
4. **出 IPC**：`tauriApi.doLogin` → `invoke<LoginResult>('do_login', { adapterName })`（`tauriApi.ts:188`）。Rust 侧的 `do_login` 命令见 [[desktop-commands]] 与 [[desktop-auth]]。
5. **返回后写回 store**，组件通过选择器订阅（如 `useAuthStore((s) => s.isLoggingIn)`）重渲染。

配置写盘的完整链路（防抖 + 脏字段 + in‑flight + 关窗 flush）：`updateConfig`（`useConfigStore.ts:59-91`）→ 500ms debounce → `saveConfigDirect`（122 起）→ `api.saveConfig`（`tauriApi.ts:179`，带重试 `tauriApi.ts:319`）→ Rust `save_config` 落盘；失败时 `dirtyFailureCounts` 累计，达 3 次放弃脏标记（145-155）。

### 链路二：Rust emit → listen → store → UI

1. **注册**：`useEventListeners` 在 effect 内（`useEventListeners.ts:18-383`）对 15 个事件各调一次 `api.onX(cb)`，unlisten 收进 `unlisteners` 数组（31），cleanup 时统一调用（375-382）。
2. **订阅原语**：`createEventListener`（`tauriApi.ts:146-175`）内部 `listen<T>(eventName, e => { if (cancelled) return; cb(e.payload) })`（151-154）。
3. **写 store**（按事件分类）：
   - `background-check-result`（`useEventListeners.ts:59-185`）：1s 节流（62‑64）→ 主/副适配器在线状态变化检测（69‑91，含“已在线”日志 5s 节流 108）→ `useAuthStore.getState().setBgStatus(prev => ...)`（118‑169）→ 文本未变则不写 `status`（178‑182）。
   - `auto-login-result`（187‑201）：写日志/toast 后 `checkOnline()`（199）。
   - `adapters-changed`（203‑228）：500ms 节流 + trailing 定时器（213‑226）→ 写 `useAdapterStore.getState().setAdapters`（207）；当 `status.state` 为 `offline`/`loading` 时补一次 `checkOnline(undefined, next)`（210）。
   - `adapter-details-changed`（230‑238）：内容未变跳过 `setState`（235），否则 `setAdapterDetails`。
   - `adapter-disabled-warning`（240‑247）：warning toast + 日志。
   - `login-log`（249‑255）：`addLog(data.message, data.type ?? 'info')`。
   - `auto-exit-countdown`（257‑276）：日志 + `addToastWithAction`，`id` 前缀 `auto-exit-cancel-`，动作按钮调 `api.cancelAutoExit()`（270）。
   - `auto-exit-cancelled`（278‑284）：成功 toast + `removeToastsByPrefix('auto-exit-cancel-')`。
   - `campus-exit-countdown`（286‑305）/`campus-exit-cancelled`（307‑313）：同上，前缀 `campus-exit-cancel-`。
   - `network-quality-result`（315‑321）：先 `handleQualityBadAlert`（33‑47，bad 状态跃迁时提示）→ `setNetworkQuality(mergeNetworkQuality(prev, data))`（319）。
   - `update-available`（323‑334）：写 `updateAvailable`/`latestVersion`/`releaseNotes` 到 `useQualityStore`。
   - `config-changed`（336‑357）：`mergeConfigFromBackend`（342，**不是** `updateConfigLocal`），避免后端旧快照回滚本地新值；并从 `data.config.activeAccount` 比对后 `setActiveAccount`（349）、再拉 `listAccounts` 刷新账号列表（351‑354）。
   - `scheduled-action-skipped`（360‑366）：info toast + 日志。
   - `mac-reset-cycle`（368‑373）：warning toast + 日志。
4. **UI 渲染**：`RightPanel` 订阅 `useLogToastStore` 的 `logs`、`StatusBar` 订阅 `useAuthStore` 的 `bgStatus`/`status`。

后端 emit 侧的事件名与载荷定义见 [[desktop-infra]]（`events.rs` 是唯一发射出口）。

### 初始化编排顺序

`App.tsx` 调 `useAppInit()`（`useAppInit.ts:6-11`），Hook 顺序即注册顺序：

1. `useEventListeners()`（`useAppInit.ts:7`）：**先**注册事件与窗口 blur 兜底 flush，保证后续初始化过程中的后端推送不丢。
2. `useInitialDataLoad()`（8），在 `useEffect` 内（`useInitialDataLoad.ts:18-167`）按下列顺序执行：
   1. `api.getInitData()`（32）；
   2. `configLoaded` 之前先按 `PASSWORD_MASK` 判定 `passwordSaved`（36‑40）；
   3. `useConfigStore.setState({ config: {...DEFAULT_CONFIG, ...initData.config} })`（41）；
   4. `useThemeStore.getState().initTheme(cfg)`（49）；
   5. 恢复/应用活动面板（51‑60）：`localStorage` 的 `campus-active-panel` 或 `cfg.defaultPanel`，`enableNetworkQuality === false` 时跳过 `quality`；
   6. 窗口显示（64‑66）：`isAutoStart && cfg.hiddenStart` 时不 `showWindow`；
   7. 适配器（68‑77）：`initData.adapters` 为空才回退 `getAdapters(false)`；
   8. 后台状态（79‑92）、适配器详情（94‑95）、账号（97‑101）；
   9. `useAuthStore.getState().checkOnline(cfg, adps)`（103）；
   10. `refreshRate`（105‑107）；
   11. DNS/DoH 后台探测（109‑137，失败或已有值即跳过，失败有 toast 提示）；
   12. 末尾置 `configLoaded: true`（152）；异常路径 `showWindow()` + 默认配置 + `configLoaded: true`（153‑161）。
3. `useHeartbeat()`（`useAppInit.ts:9`）：立即发一次心跳 + 每 5s 一次。
4. `useGlobalShortcut()`（10）：注册 `Ctrl+Shift+C`。

### 新增一个 IPC 命令 / 事件要改哪几处

新增**请求‑响应命令**：

1. `tauri-app/frontend/src/hooks/tauriApi.ts:39-144`：在 `interface TauriApi` 增加方法签名。
2. `tauri-app/frontend/src/hooks/tauriApi.ts:177-282`：在 `tauriApi` 对象增加实现 `invoke<XxxResult>('get_xxx', { arg })`。
3. 若该命令需要重试：`tauri-app/frontend/src/hooks/tauriApi.ts:314-319` 的 `tauriApiWithRetry` 里覆盖（当前仅 `saveConfig`）。
4. 消费侧 store：在目标 store 的 `interface` 加 action 签名，并在 store 实体内实现。
5. 只读数据若属于启动批次：在 `useInitialDataLoad.ts:43-168` 的 `getInitData` 分支内 `set` 到对应 store；若 Rust 侧 `init_data` 不含该字段，则在此追加一次独立 `api.xxx()` 调用。
6. Rust 侧需在 `invoke_handler` 注册（见 [[desktop-commands]]）。

新增**事件**：

1. `tauriApi.ts:39-144` 加 `onXxx: (cb: (data: T) => void) => () => void`；`tauriApi.ts:177-282` 加 `onXxx: createEventListener<T>('xxx-event')`。
2. `useEventListeners.ts:18-383` 的 effect 内注册并 `unlisteners.push(...)`（参照 59‑185 写法）。
3. store 写回点：`setBgStatus` / `useAdapterStore.getState().setXxx` / `useQualityStore.getState().setXxx` / `mergeConfigFromBackend` + `setActiveAccount` + `setAccounts`。
4. 在 `mountedRef.current` 检查与（必要时）节流后写状态；Rust 侧 emit 见 [[desktop-infra]]。

## Connections

- [[desktop-frontend-shared]]：本模块消费其 UI 组件（`ToastContainer` 渲染 `useLogToastStore.toasts`）、类型（`shared/ui-types.ts` 的 `PanelName`/`StatusState`）与常量（`MAX_LOG_ENTRIES`、`PASSWORD_MASK`、`NAV_ITEMS`）。
- [[desktop-config]]：`useConfigStore` 与 Rust 配置命令（`get_config`/`save_config`/`config-changed`）一一对应，脏字段与 `PASSWORD_MASK` 语义在此闭环。
- [[desktop-auth]]：`useAuthStore` 的 `doLogin`/`doLogout`/`checkOnline` 对应后端登录链路与 `PortalStatusResult`。
- [[desktop-monitor]]：`background-check-result`、`network-quality-result`、`BackgroundStatus`、`NetworkQuality` 的载荷定义与写入点。
- [[desktop-network-core]] / [[desktop-network-dns]]：`useAdapterStore`、`refreshAdapterData`、`check_dns_doh_status`/`setup_dns_doh`。
- [[desktop-account-selfservice]]：`list_accounts`/`switch_account`/`save_current_as_account`/`delete_account` 与自助服务命令（`bind_operator`、`query_self_*`、`self_offline_session` 等）。
- [[desktop-helper-update]]：`check_update`/`download_update`/`install_update`/`get_mirror_urls` 与 `update-available`/`update-download-progress`/`update-notification-click` 事件。
- [[desktop-commands]]：本模块是全部 IPC 命令的前端唯一入口，命令面清单与之对齐。
- [[desktop-app-lifecycle]]：`render_heartbeat`（`useHeartbeat.ts`）与关窗 flush（`useEventListeners.ts:55-57` + `useConfigStore.ts:187-216`）对应后端生命周期与 WebView 重载。
- [[desktop-platform]]：`open_external` 的 shell 插件回退（`tauriApi.ts:238-255`）与平台能力相关。
- [[desktop-infra]]：事件名/payload 契约由后端 `events.rs` 定义，前端在此镜像。

## Known Issues

1. **`useAppStore.ts` 是零消费者的死兼容壳**：文件仅 3 行 re-export（`useAppStore.ts:1-3`），全仓库除自身外无任何引用。可安全删除，但删前需确认没有外部/动态引用。
2. **`_checkOnlineLockFlag` / `_adapterLockFlag` 是模块级变量**（`useAuthStore.ts:19`、`useAdapterStore.ts:9`）：多窗口或多 store 实例场景下共享，且窗口销毁不会复位。当前为单窗口设计，扩展多窗口时必须改造。
3. **`addToastWithAction` 的定时器以 `toast.id` 为键登记**（`useLogToastStore.ts:104-108`）：若调用方传入的 `id` 与已有 toast 重复，`toastTimers.set` 会覆盖旧定时器引用，旧定时器不再被 `removeToast` 清理（`useEventListeners.ts:261`、`291` 用 `Date.now()` 组 id 规避，但约定未在 store 层强制）。
4. **`addToast` 在窗口非前台时静默丢弃**（`useLogToastStore.ts:57`）：信息依赖调用方同步写日志兜底；`addToastWithAction` 例外（承载操作入口）。若新增“必须让用户看到”的普通 toast，会在后台被静默吞掉。
5. **api 方法缺失时静默失效**：所有注册点写成 `api.onX?.(...) ?? (() => {})`（如 `useEventListeners.ts:59`、`187`、`203` 等），若 `TauriApi` 中某方法被改名或删除，`unlisteners` 里推入的是空函数，无任何报错、也无事件——只有实际推送时才会发现“UI 不更新”。
6. **`withTimeout` 不会取消底层 invoke**（`useAuthStore.ts:118-127`）：超时只是让前端提前解禁按钮，`do_login` 仍在后端继续执行，可能出现“前端已显示超时失败、后端实际登录成功”的错位（由随后的 `checkOnline` 纠正显示）。
7. **`useGlobalShortcut` 的快捷键硬编码**（`useGlobalShortcut.ts:4-24`）：`Ctrl+Shift+C` 不支持配置，`Config` 中也没有快捷键字段；前端侧 `cancel_auto_exit` 仅三个入口——`useGlobalShortcut.ts:4`、`useEventListeners.ts:270`（自动退出 toast 按钮）、`useEventListeners.ts:299`（校园网退出 toast 按钮）。
8. **`AnimationProfile` 近半数字段没有消费方**：实测被消费的只有 `tier`、`easing`、`willChangeOrbs`、`enableTilt`、`startupBoost`、`numberDuration`、`startupStaggerDelay` 等；以下字段全仓库仅出现在 `useAnimationProfile.ts` 定义处：`magneticOffset`(11)、`magneticDuration`(12)、`springStiffness`(14)、`springDamping`(15)、`powerPreference`(16)、`prefersCssAnimation`(17)、`enableGpuCompositing`(18)、`enablePageSlide`(19)、`enableBackdropBlur`(21)、`refreshRate`(25，仅内部用于算 `easing`)。改它们不会有任何效果。
9. **`mountedRef` 依赖手工复位**（`useEventListeners.ts:24-27`、`useInitialDataLoad.ts:19-24`、`useAsyncLock.ts:9-14`）：任何新增 effect 若忘记在 setup 时置 `true`，dev 模式下所有事件 handler 会静默失效。
10. **（已修复）原 `onAutoExitCancelled` / `onCampusExitCancelled` 缺 `mountedRef` 检查**：当前代码 `useEventListeners.ts:278` 与 `307` 已有 `if (!mountedRef.current) return`，与同文件其他 handler 一致。
11. **（已修复）原 `useInitialDataLoad` 的 DNS 探测失败静默**：当前代码 `useInitialDataLoad.ts:18-24` 与 `142-147` 在失败时调用 `addToast` 提示用户，不再仅 DEV 下 `console.error`。

## 结构体与字段（前端即类型与 state 结构）

### `AuthStore`（`useAuthStore.ts:137-147`）

| 字段 / action | 类型 | 含义 |
|---|---|---|
| `isLoggingIn` | `boolean` | 登录进行中（初值 `false`，`150`） |
| `isLoggingOut` | `boolean` | 注销进行中（初值 `false`，`150`） |
| `status` | `{ text: string; state: StatusState }` | 状态栏文案与语义态；初值 `{ text: '正在检测...', state: 'loading' }`（152） |
| `bgStatus` | `BackgroundStatus` | 后台巡检状态；初值 `{ isRunning: false, checkCount: 0, serverAvailable: false, online: false, onlineOperator: null, secondaryOnlineOperator: null, adapterStatuses: [], currentSsid: null }`（153） |
| `doLogin(adapterName?)` | `Promise<boolean>` | 155‑214 |
| `doLogout(adapterName?)` | `Promise<void>` | 216‑245 |
| `checkOnline(cfg?, adps?)` | `Promise<void>` | 247‑315 |
| `setStatus(s)` | `(s) => void` | 317 |
| `setBgStatus(s)` | 值或更新函数 | 318 |

### `ConfigStore`（`useConfigStore.ts:27-49`）

| 字段 / action | 类型 | 含义 |
|---|---|---|
| `config` | `Config` | 初值 `DEFAULT_CONFIG`（52） |
| `configLoaded` | `boolean` | 初始配置是否装载完成（成功或降级都置 true，见 `useInitialDataLoad.ts:152,160`） |
| `passwordSaved` | `boolean` | 登录密码已保存（决定密码框显示“已保存”占位） |
| `initDebugMode?` | `boolean` | `get_init_data` 携带的日志面板初始值（35） |
| `initLogRetentionDays?` | `number` | 同上（36） |
| `accounts` | `AccountItem[]` | 账号列表（初值 `[]`，55） |
| `activeAccount` | `string` | 当前账号（初值 `''`，56） |
| `language` | `string` | 初值 `safeStorage.get('app-language') \|\| 'zh'`（57） |
| `updateConfig(partial)` | `(p: Partial<Config>) => void` | 59‑91：立即 `set` + 标脏 + 500ms debounce 保存；`customThemeColor` 同步给 `useThemeStore`（90） |
| `updateConfigLocal(partial)` | 同上 | 93‑102：只改本地与标脏，不落盘 |
| `mergeConfigFromBackend(incoming)` | `(p: Partial<Config>) => void` | 104‑113：跳过 `dirtyFields` 中的键 |
| `clearDirtyFields()` | `() => void` | 115‑118 |
| `syncPasswordSaved(saved)` | `(b: boolean) => void` | 120 |
| `saveConfigDirect(cfg, clearPassword?)` | `=> Promise<void>` | 122‑168：全量合并后调 `api.saveConfig`；成功清脏并按需置 `passwordSaved`（134‑136）；失败记日志，连续失败达 `DIRTY_FAILURE_LIMIT` 时放弃脏标记（145‑155） |
| `setAccounts(a)` / `setActiveAccount(a)` | `=> void` | 170‑171 |
| `setLanguage(lang)` | `=> void` | 173‑177：写 `safeStorage` + `i18next.changeLanguage` |

### `AdapterStore`（`useAdapterStore.ts:34-43`）

| 字段 / action | 类型 | 含义 |
|---|---|---|
| `adapters` | `Adapter[]` | 初值 `[]`（35） |
| `adapterDetails` | `AdapterDetail[]` | 36 |
| `isRefreshingAdapters` | `boolean` | 37；`refreshAdapters` 最少保持 true 500ms（63‑65） |
| `activePanel` | `PanelName` | 初值 `'dashboard'`（38） |
| `refreshAdapters()` | `=> Promise<void>` | 51‑66：锁 → `refreshAdapterData({force:true, triggerCheck:true})` → 并行等 500ms → 释放锁 |
| `setAdapters(a)` | `=> void` | 68 |
| `setAdapterDetails(d)` | `=> void` | 69 |
| `setActivePanel(p)` | `=> void` | 70 |

### `LogToastStore`（`useLogToastStore.ts:15-26`）

| 字段 / action | 类型 | 含义 |
|---|---|---|
| `logs` | `LogEntry[]` | 初值 `[]`（29）；上限 `MAX_LOG_ENTRIES`（外部常量 300） |
| `toasts` | `ToastMessage[]` | 初值 `[]`（30）；上限 `MAX_TOASTS = 4`（9） |
| `addLog(message, type='info')` | `=> void` | 32‑51：与最后一条同内容同级别时折叠为 `count + 1`（39‑43）；否则追加并在超上限时裁掉最旧（46‑48） |
| `addToast(title, type='info', description?, duration=4000, mascot?)` | `=> void` | 53‑83：`document.visibilityState !== 'visible'` 直接 return（57）；同 title 去重（62）；超限淘汰最旧并清其定时器（65‑74） |
| `addToastWithAction(toast)` | `=> void` | 85‑109：缺省 `duration` 8000（86）；非前台**也**入队（承载“取消退出”入口） |
| `removeToast(id)` | `=> void` | 111‑115 |
| `removeToastsByPrefix(prefix)` | `=> void` | 117‑127（用于清 `auto-exit-cancel-` / `campus-exit-cancel-` 前缀） |
| `setLogs(logs)` | `=> void` | 129（超限时截尾） |
| `cleanupToasts()` | `=> void` | 131‑135 |

### `AnimationProfile`（`useAnimationProfile.ts:8-26`）

| 字段 | 类型 | 含义 |
|---|---|---|
| `tier` | `AnimationTier` | high / standard / economy（现恒 high） |
| `willChangeOrbs` | `boolean` | 是否给光晕加 `will-change` |
| `magneticOffset` | `number` | 磁吸位移量（high 档 5） |
| `magneticDuration` | `number` | 磁吸动画时长（0.4） |
| `numberDuration` | `number` | 数字滚动时长（high 600） |
| `springStiffness` / `springDamping` | `number` | 弹簧参数（400 / 18） |
| `powerPreference` | `'low-power' \| 'high-performance'` | WebGL 上下文偏好（high 档 `high-performance`） |
| `prefersCssAnimation` | `boolean` | 是否优先 CSS 动画（high 档 false） |
| `enableGpuCompositing` | `boolean` | GPU 合成开关（true） |
| `enablePageSlide` | `boolean` | 面板滑动（true） |
| `enableTilt` | `boolean` | 卡片 3D 倾斜（true） |
| `enableBackdropBlur` | `boolean` | 背景模糊（true） |
| `startupBoost` | `boolean` | 启动入场动画（true） |
| `startupStaggerDelay` | `number` | 入场错峰间隔（0.05） |
| `easing` | `EasingConfig` | 由 `refreshRate` 派生 |
| `refreshRate` | `number` | 生效刷新率（`refreshRate > 0 ? refreshRate : 120`，54） |

### tauriApi 的局部接口

| 接口 | 行号 | 字段 |
|---|---|---|
| `ConnectionCampusStatus` | 11‑15 | `onCampus: boolean`、`name: string \| null`、`message: string` |
| `AdapterCampusStatus` | 17‑25 | `name: string`、`wireless: boolean`、`onCampus: boolean`、`matchedBy: string \| null`、`message: string` |
| `CampusStatusResult` | 27‑37 | `onCampusNetwork: boolean`、`currentSsid: string \| null`、`campusMessage: string`、`enableNetworkNameCheck: boolean`、`requiredNetworkName: string`、`campusWifi: ConnectionCampusStatus \| null`、`campusWired: ConnectionCampusStatus \| null`、`adapters?: AdapterCampusStatus[]` |
