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

本模块 `android-frontend-hooks-stores` 覆盖安卓前端状态管理与基础设施层，对应目录 `android/frontend/src/hooks/` 下的 12 个文件。它以 **Zustand** 按领域拆分 store（认证、配置、网络质量、主题、面板导航），以 `tauriApi.ts` 统一定义与 Rust 后端通信的 IPC 契约，并以一组 React Hooks 提供应用初始化编排、异步互斥锁、GSAP 自适应帧率、动画配置档案与系统「减少动态效果」适配。

按职责分为四层：

1. **Tauri API 封装与适配**（`tauriApi.ts`）：定义全部 IPC 方法签名，对安卓端不存在的桌面专属能力（适配器/DHCP/DNS/窗口/托盘/心跳）统一显式拒绝或空监听，封装生物识别验证门、外部链接打开白名单与仅针对 `saveConfig` 的指数退避重试。
2. **领域状态管理**：`useAuthStore`（登录/注销/在线检测）、`useConfigStore`（配置去抖保存/脏标记/密码状态）、`useQualityStore`（网络质量/DNS/版本更新）、`useThemeStore`（主题与 DOM 副作用）、`useAdapterStore`（面板导航，网卡数据链已在安卓端删除）。
3. **应用生命周期编排**（`useAppInit.ts`）：组合事件监听、初始数据加载、心跳与全局快捷键四个子 hook；`useAppStore.ts` 仅保留兼容性 re-export。
4. **性能与动画**：`useAdaptiveFramePace`（gsap ticker 动态帧率）、`useAnimationProfile`（恒最高档动画参数）、`useReducedMotion`（系统减少动态效果偏好）；另有通用工具 `useAsyncLock`。

跨 store 协作通过 `useXxxStore.getState()` 直接读取完成（如 `useAuthStore` 读写 `useConfigStore`/`useQualityStore`/`useLogToastStore`），关键副作用（DOM class/CSS 变量同步、localStorage 持久化）集中在 `useThemeStore` 的全局 `subscribe` 与各 store 的模块级变量中。

## Key Components

### 1. Tauri API 封装（`android/frontend/src/hooks/tauriApi.ts`）

#### 类型与接口

| 名称 | 行号 | 字段 / 用途 |
| --- | --- | --- |
| `ConnectionCampusStatus` | `tauriApi.ts:20-24` | `onCampus: boolean`、`name: string \| null`、`message: string`（校园网单链路状态） |
| `BatteryOptimizationInfo` | `tauriApi.ts:26-30` | `ignoring: boolean`、`brand: string`、`hasVendorTarget: boolean`（电池优化白名单状态） |
| `VendorSettingsResult` | `tauriApi.ts:32-36` | `path: 'vendor' \| 'vendor_action' \| 'app_details' \| 'settings' \| 'none'`、`target: string`、`tried: string[]`（厂商省电设置页跳转结果） |
| `CampusStatusResult` | `tauriApi.ts:38-46` | `onCampusNetwork`、`currentSsid`、`campusMessage`、`enableNetworkNameCheck`、`requiredNetworkName`、`campusWifi: ConnectionCampusStatus \| null`、`campusWired: ConnectionCampusStatus \| null` |
| `TauriApi` | `tauriApi.ts:48-141` | 全部 IPC 方法签名（下文分组列出） |
| `AvoidBadWifiStatus` | `tauriApi.ts:143-152` | `granted`（WRITE_SECURE_SETTINGS 是否经 adb 授权）、`avoidBadWifi`（当前值，-1 = 读不到）、`pendingRestoreAvoid`、`pendingRestoreCaptive`（插件快照待还原标记） |
| `RestoreWrittenSettingsResult` | `tauriApi.ts:154-157` | `restored: Record<string, boolean>`、`reason?: string` |

`TauriApi` 接口（`tauriApi.ts:48-141`）方法按端语义分组（行号为该成员在接口中的声明位置）：

- 配置：`getConfig` (49)、`saveConfig` (50)、`exportConfig`/`importConfig`/`exportDiagnostics` (52-54，桌面专属)。
- 网络适配器（桌面遗留）：`getAdapters` (55)、`enableAdapter` (56)、`getAdapterDetails` (57)、`bindToWifi` (58)、`checkPortalStatus` (59)。
- 校园网与认证：`checkCampusStatus` (60)、`requestWifiSsidPermission` (61)、`doLogin` (62)、`doLogout` (63)、`bindOperator` (64)、`getBindStatus` (65)、`verifyWindowsIdentity` (66)、`revealOperatorCredential` (67)、`querySelfDashboard` (68)、`selfOfflineSession` (69)、`querySelfOnlineLog` (70)、`changeSelfPassword` (71)。
- 窗口（桌面专属）：`minimizeWindow` (72)、`closeWindow` (73)、`showWindow` (115)、`renderHeartbeat` (132)、`notifyWindowReady` (134)。
- 事件监听：`onBackgroundCheckResult` (74)、`onAutoLoginResult` (75)、`onAdaptersChanged` (76)、`onAdapterDetailsChanged` (77)、`onAdapterDisabledWarning` (78)、`onLoginLog` (79)、`onNetworkQualityResult` (97)、`onAutoExitCountdown` (110)、`onAutoExitCancelled` (111)、`onCampusExitCountdown` (112)、`onCampusExitCancelled` (113)、`onConfigChanged` (114)、`onDownloadProgress` (125)、`onUpdateAvailable` (126)、`onUpdateUnverified` (127-128)。
- 账号档案：`listAccounts` (80)、`createAccount` (82)、`switchAccount` (83)、`saveCurrentAsAccount` (84)、`deleteAccount` (85)、`renameAccount` (87)、`getActiveAccount` (88)。
- 后台检测：`startBackgroundCheck` (89)、`stopBackgroundCheck` (90)、`triggerBackgroundCheck` (91)、`getBackgroundStatus` (92)。
- DHCP（桌面专属）：`dhcpRenewAll` (93)、`dhcpReleaseRenew` (94)、`dhcpReleaseRenewAdapter` (95)。
- 网络质量：`checkNetworkQuality` (96)、`startLatencyTest` (98)、`stopLatencyTest` (99)。
- 系统设置：`openExternal` (100)、`getAutoLaunch` (101)、`setAutoLaunch` (102)、`getNotificationEnabled` (103)、`setNotificationEnabled` (104)、`getBatteryOptimizationInfo` (105)、`requestIgnoreBatteryOptimizations` (106)、`openVendorBatterySettings` (107)、`openNotificationSettings` (108)、`cancelAutoExit` (109)、`getLogs` (116)、`clearLogs` (117)、`getDebugMode` (118)、`setDebugMode` (119)、`getLogRetentionDays` (135)、`setLogRetentionDays` (136)、`getAvoidBadWifiStatus` (138)、`restoreWrittenSettings` (140)。
- 初始化与更新：`getInitData` (120)、`checkUpdate` (121)、`downloadUpdate` (122)、`installUpdate` (123)、`getMirrorUrls` (124)、`checkDnsDohStatus` (129)、`setupDnsDoh` (130)、`resetDns` (131)。

#### 实现层符号

- `desktopOnly<T>(name)`（`tauriApi.ts:15-16`）：安卓端对桌面专属命令的直接拒绝——`Promise.reject(new Error("桌面专属功能,安卓端不可用: <name>"))`，不发起 invoke。
- `noopListener`（`tauriApi.ts:18`）：返回 no-op 清理函数的空监听器，订阅处零改动且永不触发。
- `createEventListener<T>(eventName)`（`tauriApi.ts:161-190`）：通用事件监听工厂。内部用 `cancelled` 标志与 `unlisten` 槽位处理「listen 尚未完成就卸载」的竞态：回调在 `cancelled` 时丢弃 payload（行 165-166）；清理函数若 `unlisten` 已就绪立即解绑，否则等 `listenPromise` 完成后补解绑（行 177-186）；listen 失败仅在 DEV 打印（行 172-175）。
- `tauriApi` 对象（`tauriApi.ts:190-324`）：按 `TauriApi` 契约实现的 invoke/监听绑定。要点：
  - 桌面专属降级（返回 `desktopOnly`）：`exportConfig`(193)、`importConfig`(194)、`exportDiagnostics`(195)、`getAdapters`(196)、`enableAdapter`(197)、`getAdapterDetails`(198)、`minimizeWindow`(250)、`closeWindow`(251)、`dhcpRenewAll`(269)、`dhcpReleaseRenew`(270)、`dhcpReleaseRenewAdapter`(271)、`cancelAutoExit`(296)、`showWindow`(302)、`checkDnsDohStatus`(315)、`setupDnsDoh`(316)、`resetDns`(317)、`renderHeartbeat`(318)、`notifyWindowReady`(319)。
  - 桌面专属空监听（`noopListener`）：`onAdaptersChanged`(254)、`onAdapterDetailsChanged`(255)、`onAdapterDisabledWarning`(256)、`onAutoExitCountdown`(297)、`onAutoExitCancelled`(298)、`onCampusExitCountdown`(299)、`onCampusExitCancelled`(300)。
  - 真实事件绑定（`createEventListener` + Tauri 事件名）：`background-check-result`(252)、`auto-login-result`(253)、`login-log`(257)、`network-quality-result`(273)、`config-changed`(301)、`update-download-progress`(312)、`update-available`(313)、`update-unverified`(314)。
  - 安卓专属命令：`bindToWifi`(199)、`checkCampusStatus`(201)、`requestWifiSsidPermission`(202)、`getBatteryOptimizationInfo`(292)、`requestIgnoreBatteryOptimizations`(293)、`openVendorBatterySettings`(294)、`openNotificationSettings`(295)、`getAvoidBadWifiStatus`(322)、`restoreWrittenSettings`(323)、`getLogRetentionDays`(320)、`setLogRetentionDays`(321)。
  - `verifyWindowsIdentity`（`tauriApi.ts:207-244`）：生物识别验证门统一出口。内部 `report(success, method)`（行 211-217）把真实结果 invoke 到后端 `verify_biometric_identity`，invoke 异常也归一化为 `{ success: false, message }` 而非抛 IPC 故障。链路顺序：`shouldUseFaceFallback()` 开启且系统生物识别不可用（国产平板 2D 人脸 Class 1 不可达 BiometricPrompt）时走应用内 2D 人脸验证 `useFaceDialogStore.getState().openFaceDialog('verify')`（行 221-235，比对失败/取消一律上报 `face2d` 失败；status 查询异常落回系统链路）；否则 `biometricAuthenticate(params.consentMessage, { allowDeviceCredential: true })`（行 239），取消/失败上报 `biometric` 失败（行 240-243）。
  - `openExternal`（`tauriApi.ts:276-287`）：外部链接打开。校验 http/https 前缀（行 277）、长度 ≤2048（行 278）、`new URL` 解析（行 279），任一失败返回 `false`；通过 `openerOpenUrl` 打开（行 282），异常返回 `false`。
  - `getAutoLaunch`/`setAutoLaunch`（`tauriApi.ts:288-289`）：实际调用 `get_boot_autostart`/`set_boot_autostart`，安卓端使用引导自启；`setAutoLaunch` 把 void 结果包装为 `{ success: true }`。
  - `onUpdateUnverified`（`tauriApi.ts:314`）：更新源 API 不可用时放行下载（无法校验完整性）的警示事件监听。
- `isRetryableError(e)`（`tauriApi.ts:326-336`）：错误字符串或 `Error.message` 小写化后包含 `timeout`/`network`/`fetch`/`connection` 才可重试。
- `withRetry<T>(fn, maxRetries = 2, baseDelay = 500)`（`tauriApi.ts:338-354`）：指数退避重试，延迟 `baseDelay * 2^attempt + random*200`（行 347）；不可重试错误立即抛出。
- `tauriApiWithRetry`（`tauriApi.ts:356-362`）：对外暴露的 API 实例——展开 `tauriApi` 后仅覆盖 `saveConfig` 启用重试（行 361）。注释（行 358-360）说明只保留这一个的原因：保存是一次性关键操作、无其他兜底；`checkPortalStatus` 由 checkOnline 高频调用且后台检测循环周期性重试、`checkNetworkQuality` 有后端 latency loop 事件流兜底，包重试会放大高频流量。

### 2. 领域 Stores（Zustand）

#### `useAuthStore.ts` — 认证领域

职责：登录/注销流程、在线状态检测、后台状态维护。

- 模块级状态：
  - `_checkOnlineLockFlag`（`useAuthStore.ts:16`）：checkOnline 串行锁。
  - `QUALITY_MANUAL_THROTTLE_MS = 60_000`（`useAuthStore.ts:21`）：登录后手动质量探测节流阈值，与后端 `run_quality_check` 的 60s 全局节流对齐（历史缺陷 P2-F5，注释行 18-20）。
- 辅助函数（状态更新统一由 `checkOnline` 处理）：
  - `detectCampusNetwork()`（`useAuthStore.ts:29-35`）：调用 `api.checkCampusStatus()`，失败返回 `null`。
  - `buildCampusBgStatusPatch(bgStatus, campusStatus)`（`useAuthStore.ts:38-49`）：纯函数，构造 `onCampusNetwork`/`campusWifi`/`campusWired`/`enableNetworkNameCheck`/`requiredNetworkName` 的 `Partial<BackgroundStatus>` 补丁。
  - `queryPortalStatus(adapterIp)`（`useAuthStore.ts:56-65`）：查询 Portal 在线状态，失败返回 `{ ok: false }`。注释（行 51-55）记载关键约定：安卓无 adapters 概念（列表恒空、`getAdapters` 必 reject），`adapterIp` 恒传空串，后端 `check_portal_status` 对空 IP 回退缓存源 IP/默认路由；此前 checkOnline 走「解析失败判无网络」路径，是状态点恒被打灰（真机反馈：不手动登录不变绿、过一会又变灰）的直接根因。
  - `withTimeout<T>(promise, ms, timeoutMsg)`（`useAuthStore.ts:69-78`）：登录/注销 invoke 的超时包装，`Promise.race` 超时 reject；`finally` 清理输家定时器（行 77），避免悬挂 60s 定时器。历史缺陷 P2-34：后端阻塞时防止 `isLoggingIn/isLoggingOut` 永久为 true、按钮永久禁用（注释行 67-68）。
  - `setStatusStable(next)`（`useAuthStore.ts:82-86`）：引用稳定性优化——`text`/`state` 均未变化时不写状态，避免 StatusBar 等按引用订阅的组件无效重渲染（注释行 80-81）。
- `AuthStore` 接口（`useAuthStore.ts:88-98`）：`isLoggingIn`、`isLoggingOut`、`status: { text: string; state: StatusState }`、`bgStatus: BackgroundStatus`、`doLogin(): Promise<boolean>`、`doLogout(): Promise<void>`、`checkOnline(cfg?)`、`setStatus(s)`、`setBgStatus(s | fn)`。
- store 初始状态（`useAuthStore.ts:101-104`）：`status` 初始 `{ text: '正在检测...', state: 'loading' }`；`bgStatus` 初始 `{ isRunning: false, checkCount: 0, serverAvailable: false, online: false, onlineOperator: null, currentSsid: null }`。
- `doLogin()`（`useAuthStore.ts:106-164`）：
  1. 防重入：`isLoggingIn || isLoggingOut || !config` 直接返回 `false`（行 107-109）。
  2. 置 `isLoggingIn`、状态 loading，写日志与 toast（行 111-114）。
  3. 先 `saveConfigDirect(loginConfig)` 持久化当前配置，失败仅记 warning 日志并继续用已有配置登录（行 117-121）。
  4. `withTimeout(api.doLogin(), 60000, ...)` 执行登录（行 125）；成功/失败分别更新 `status`（online/offline）并写日志与 toast（行 126-135）。
  5. 登录后质量探测节流（行 136-148）：`config.enableNetworkQuality !== false` 且 `Date.now() - getLastQualityResultTime() > QUALITY_MANUAL_THROTTLE_MS`（行 141）才发起 `checkNetworkQuality`，结果经 `mergeNetworkQuality` 合并进 `useQualityStore`；失败记 warning 日志。注释（行 137-140）记载：后端 60s 节流只作用于循环路径，手动命令不受限，故前端自行节流。
  6. 登录成功**跳过立即 `checkOnline`**（登录结果已是权威判定）；仅失败时复查确认状态（行 159-160）。注释（行 156-158）记载历史缺陷：登录成功立即复查会在 Portal 会话未传播时返回 offline，把「登录成功」覆盖为「未登录」造成状态闪烁。
  7. 复位 `isLoggingIn` 并返回 `success`（行 162-163）。
- `doLogout()`（`useAuthStore.ts:166-194`）：防重入（行 167-168）→ loading + 日志/toast（行 170-172）→ `withTimeout(api.doLogout(), 60000, ...)`（行 175）→ 成功/失败/异常三分支更新状态与日志/toast（行 176-190）→ 无条件 `checkOnline` 确认离线（行 192）→ 复位 `isLoggingOut`（行 193）。
- `checkOnline(cfg?)`（`useAuthStore.ts:196-243`）：核心状态同步，全程持 `_checkOnlineLockFlag` 串行锁（行 197-198；`finally` 释放，行 241）。
  1. campus 检测（行 204-222）：`enableNetworkNameCheck` 开启时 `detectCampusNetwork()`；不在校园网 → 记 warning 日志（仅当先前状态非 offline，行 206-210）、写 `bgStatus` 补丁、`setStatusStable` 为 offline 并 return；campusStatus 可用时也合并补丁（行 217-221）。
  2. Portal 查询（行 224-235）：`queryPortalStatus('')`（空 IP，见上文约定）；`!ok` → offline；portal 有值 → 状态变化时记录 `portal.message` 日志（行 231-233）、`setStatusStable` 为 online/offline（行 234）。
  3. 锁释放策略：注释（行 237-240）记载历史缺陷——用 `setTimeout(500)` 释放锁时，若实际执行超 500ms 锁已提前释放、并发调用产生冗余 invoke；现改为 promise settle 时立即释放，锁持有时间与执行时间一致，串行下任意时刻至多一个执行体、无需 epoch 校验。
- setters：`setStatus`（行 245）、`setBgStatus`（行 246，支持函数式更新）。

#### `useConfigStore.ts` — 配置领域

职责：配置对象、去抖保存、脏标记管理、密码保存状态、账号列表、语言设置。

- 模块级状态（保存管线）：
  - `saveConfigTimer`（`useConfigStore.ts:14`）：500ms 去抖定时器。
  - `saveConfigPending`（`useConfigStore.ts:15`）：待保存的配置增量。
  - `saveConfigInFlight`（`useConfigStore.ts:17`）：正在发送的保存 Promise，关窗前需等待以免数据丢失。
  - `dirtyFields: Set<string>`（`useConfigStore.ts:20`）：本地已修改但尚未被后端确认的字段，`config-changed` 回传时跳过，避免后端旧快照覆盖本地新值。
  - `DIRTY_FAILURE_LIMIT = 3`（`useConfigStore.ts:24`）与 `dirtyFailureCounts: Map`（行 25）：连续失败计数字段，超阈值后放弃脏标记允许后端回传重新同步（历史缺陷：保存持续失败时 dirty 字段被永久跳过、UI 与后端脱节，注释行 21-23）。
- `ConfigStore` 接口（`useConfigStore.ts:27-46`）：`config`、`configLoaded`、`passwordSaved`、`accounts: AccountItem[]`、`activeAccount`、`language`、`updateConfig`、`updateConfigLocal`、`mergeConfigFromBackend`、`clearDirtyFields`、`syncPasswordSaved`、`saveConfigDirect`、`setAccounts`、`setActiveAccount`、`setLanguage`。`configLoaded` 语义：初始配置是否已从后端加载完成（成功或降级都置 true），依赖 config 的启动逻辑应等此信号（注释行 29-32）。
- store 初始状态（`useConfigStore.ts:49-54`）：`config: DEFAULT_CONFIG`、`configLoaded: false`、`passwordSaved: false`、`accounts: []`、`activeAccount: ''`、`language: safeStorage.get('app-language') || 'zh'`。
- `updateConfig(partial)`（`useConfigStore.ts:56-88`）：
  1. 合并 config 并写状态（行 58-59）。
  2. 标记 dirty 字段并重置其失败计数（行 62-65；重新编辑视为新一轮保存）。
  3. 合并 `saveConfigPending`：密码 MASK 保护——pending 已持有真实密码、本次只传 MASK 时保留真实密码（行 66-75）。
  4. 重置去抖定时器，500ms 后调用 `saveConfigDirect(pending)`，失败 DEV 打印 + error toast（行 76-86）。
  5. `partial.customThemeColor` 联动 `useThemeStore.setCustomThemeColor`（行 87）。
- `updateConfigLocal(partial)`（`useConfigStore.ts:90-99`）：仅本地合并 + dirty 标记 + 主题联动，**不**触发后端保存。
- `mergeConfigFromBackend`（`useConfigStore.ts:103-119`）：处理后端 `config-changed` 回传——遍历字段时跳过 `dirtyFields.has(k)` 的项（行 108）；密码字段另有「已保存」口径同步：MASK = 后端确有密码、空 = 后端确实没有（行 110-118）。注释（行 111-113）记载：复位密码的 dirty 标记会把密码钉死在本地，使清除密码后事件回传的空密码被跳过、`passwordSaved` 永久残留 true。
- `clearDirtyFields()`（`useConfigStore.ts:121-124`）：清空脏标记与失败计数。
- `syncPasswordSaved(saved)`（`useConfigStore.ts:126`）：密码保存状态写入口。
- `saveConfigDirect(cfg, clearPassword?)`（`useConfigStore.ts:128-176`）：直接保存（不经去抖）。
  1. 合并全量配置后发起 `api.saveConfig`（行 129-132，此走 `tauriApiWithRetry` 即带重试）。
  2. 成功：清除本次字段的 dirty 标记与失败计数（行 134-137）；`cfg.password` 非空时 `syncPasswordSaved(true)`（行 140-142，注释行 138-139：修复会话内保存密码后 `passwordSaved` 从不置 true、密码框显示空白）。
  3. 失败：记 error 日志（行 144-145）；本次带密码则 `syncPasswordSaved(false)`（行 148-150，注释：后端没存进去，残留 true 会使下次启动跳过密码恢复）；失败计数递增、达 `DIRTY_FAILURE_LIMIT` 的字段放弃脏标记并记 warning（行 152-162）；**失败上抛**，调用方各自走失败分支，不能静默吞掉让 UI 假成功（行 163-165）。
  4. in-flight 跟踪：`saveConfigInFlight = promise`（行 168）；`finally` 中**仅清自己的引用**——并发保存时后写者已覆盖该引用，无条件清空会让 `hasPendingConfig` 漏报、关窗跳过等待丢数据（行 171-175）。
- `setAccounts`（行 178）、`setActiveAccount`（行 179）、`setLanguage`（行 181-185，写 `safeStorage` 并 `i18next.changeLanguage`）。
- `hasPendingConfig()`（`useConfigStore.ts:188-193`）：`saveConfigPending !== null || saveConfigInFlight !== null`（行 192）。注释（行 189-191）记载历史缺陷：仅检查 pending 时，debounce 已触发、保存 in-flight 的情况下返回 false，关窗直接跳过 flush 分支、in-flight 保存随进程丢弃。
- `flushPendingConfig()`（`useConfigStore.ts:195-224`）：关窗/后台化兜底冲刷。
  1. 清去抖定时器（行 196-199）。
  2. 有 pending 时：剔除 MASK 密码（行 205-207）、合并全量配置（行 209，MASK 原样发给后端由其识别并保留原密码）、发出保存并**纳入返回值**（行 214-217；注释行 210-213 记载历史缺陷：本次新发出的保存不经 in-flight 跟踪、返回旧引用，仅 debounce 待存时调用方拿到 null 直接关窗丢数据）。保存失败 catch 后 resolve（关窗不因保存失败中断）但留诊断日志。
  3. 返回 `flushed ?? current`，两者并存时 `Promise.allSettled([flushed, current])`（行 219-223）。

#### `useQualityStore.ts` — 网络质量领域

职责：网络质量、DNS DoH、版本更新状态。核心机制是「手动刷新 / 后台循环事件 / 登录后探测」多来源间的节流与去重（历史缺陷 P2-F5）。

- 模块级状态：
  - `_qualityLockFlag`（`useQualityStore.ts:11`）：`refreshQuality` 同步锁。
  - `lastQualityResultTime`（`useQualityStore.ts:16`）：最近一次质量结果到达时间戳（0 = 从未收到）。任意来源（循环事件/手动刷新/登录后探测）都算新鲜结果，用于登录后手动探测的节流判断（注释行 13-15）。
  - `getLastQualityResultTime(): number`（`useQualityStore.ts:18-20`）：对外暴露该时间戳的读取函数。
- `QualityStore` 接口（`useQualityStore.ts:22-41`）：`networkQuality: NetworkQuality | null`、`dnsDohStatus: DnsDohStatus | null`、`dnsChecking: boolean`、`isRefreshingQuality: boolean`、`updateAvailable: boolean`、`latestVersion: string`、`releaseNotes: string`、`updatePromptOpen: boolean`（发现新版本弹窗可见性，注释行 30）、`refreshRate: number`、`refreshQuality(): Promise<void>`、`setNetworkQuality(q | fn)`、`setDnsDohStatus`、`setDnsChecking`、`setUpdateAvailable`、`setLatestVersion`、`setReleaseNotes`、`setUpdatePromptOpen`。
- store 初始状态（`useQualityStore.ts:44-52`）：全部空/false/0（`refreshRate: 0` 表示未获得设备刷新率）。
- `refreshQuality()`（`useQualityStore.ts:54-74`）：
  1. 从 `useConfigStore.getState()` 读配置；`_qualityLockFlag` 已锁或 `config.enableNetworkQuality === false` 直接返回（行 55-57）。
  2. 加锁 + 置 `isRefreshingQuality`（行 58-59）。
  3. `await api.checkNetworkQuality?.()`（行 61）；有结果时统一走 `get().setNetworkQuality((old) => mergeNetworkQuality(old, q))`（行 63-64），保证 `lastQualityResultTime` 覆盖手动刷新来源。
  4. 异常仅 DEV 打印（行 66-67）。
  5. `finally` 中 `setTimeout(..., 500)` 延迟解锁与复位加载态（行 68-72），防止高频触发。
- `setNetworkQuality(q | fn)`（`useQualityStore.ts:78-82`）：支持函数式更新；**关键逻辑**：`next !== null` 时强制 `lastQualityResultTime = Date.now()`（行 80）——无论来源，收到有效结果即重置节流窗口。
- 其余 setters（行 83-88）：`setDnsDohStatus`/`setDnsChecking`/`setUpdateAvailable`/`setLatestVersion`/`setReleaseNotes`/`setUpdatePromptOpen`，均为简单 `set`。

#### `useThemeStore.ts` — 主题领域

职责：主题名称、浅色模式、自定义主题色、玻璃档位，以及 DOM class/CSS 变量的同步副作用。

- `ThemeStore` 接口（`useThemeStore.ts:9-19`）：`themeName: ThemeName`、`isLightMode: boolean`、`customThemeColor: string`、`glassLevel: GlassLevel`、`setThemeName`、`setIsLightMode`、`setGlassLevel`、`initTheme(cfg: Partial<Config>)`、`setCustomThemeColor`。
- store 初始状态（`useThemeStore.ts:21-28`）：
  - `themeName: 'default'`（行 22）。
  - `isLightMode`：惰性初始化，读 `safeStorage.get('campus-light-mode') === '1'`（行 23）。
  - `customThemeColor: '#6366f1'`（行 24）。
  - `glassLevel`：读 `campus-glass-level` 并校验在 `VALID_GLASS_LEVELS` 中，否则回退 `'standard'`（行 25-28）。
- setters：`setThemeName`/`setIsLightMode`/`setGlassLevel`（行 30-32）、`setCustomThemeColor`（行 56）。
- `initTheme(cfg)`（`useThemeStore.ts:34-54`）：启动时从后端配置 + 本地存储恢复主题。
  1. `campus-theme` 存储值合法则恢复 `themeName`（行 35-36）。
  2. 浅色模式决策链（行 37-52）：存储 `'1'` → true（行 38-39）；存储 `'0'` → false（行 40-41）；无存储时按 `cfg.themeMode`——`'light'` → true 并写回 `'1'`（行 42-44）、`'dark'` → false 并写回 `'0'`（行 45-47）、`'system'` → `window.matchMedia('(prefers-color-scheme: light)').matches` 并写回（行 48-51）。
  3. `cfg.customThemeColor` 存在则应用（行 53）。
- 全局副作用订阅 `useThemeStore.subscribe((state, prev) => ...)`（`useThemeStore.ts:60-97`）：监听状态变化直接操作 DOM。
  - glassLevel（行 62-65）：设 `document.documentElement` 的 `data-glass` 属性并持久化 `campus-glass-level`；三档强度全在 CSS 变量里派生，只切一个 data 属性（注释行 61）。
  - isLightMode（行 66-73）：toggle `.dark` class（`!isLightMode` 时加），并按浅色模式增删 `data-light="1"` 属性。
  - themeName/customThemeColor/isLightMode（行 74-97）：先移除全部主题 class `theme-vibrant/theme-forest/theme-midnight/theme-ocean/theme-cherry/theme-custom`（行 76-77）。custom 分支（行 78-87）：加 `.theme-custom`，`hexToHsl` 转换后用 `root.style.setProperty` 覆盖 `--primary`/`--ring`/`--accent`/`--accent-foreground`（行 84-87；`--accent` 的饱和度封顶 33、亮度按浅色模式取 94/17，`--accent-foreground` 亮度取 20/85）。**历史缺陷修复**（注释行 82-83）：此前 `cssText +=` 累积声明，多次切换主题后 inline style 无界增长，改用 `setProperty` 覆盖同名变量。非 custom 分支（行 88-96）：移除这四个 CSS 变量（行 89-92），非 default 主题加 `theme-${name}` class（行 93-95）。

#### `useAdapterStore.ts` — 面板导航

- 文件头注释（`useAdapterStore.ts:1-2`）：安卓端网卡数据链已删除（adapters 列表恒空、面板不渲染网卡卡），仅保留激活面板状态供 DockNav/TabletShell/SettingsPanel 导航用。
- `AdapterStore` 接口（行 6-9）：`activePanel: PanelName`、`setActivePanel`。
- store（行 11-14）：`activePanel` 默认 `'dashboard'`。

### 3. 应用初始化编排（`useAppInit.ts` / `useAppStore.ts`）

- `useAppInit()`（`useAppInit.ts:6-11`）：应用根部一次性组合四个子 hook——`useEventListeners()`（后端事件监听与关窗前冲刷配置）、`useInitialDataLoad()`（初始配置/背景状态/账号列表加载）、`useHeartbeat()`（渲染链心跳）、`useGlobalShortcut()`。这四个子 hook 源码不在本模块的 12 个源文件清单内，细节见 hooks 目录对应文件。
- `useAppStore.ts`（`useAppStore.ts:1-3`）：`useAppStore` 已拆分为领域 store，此文件仅 re-export `useAppInit` 与 `hasPendingConfig`/`flushPendingConfig` 以兼容旧引用，无业务逻辑。

### 4. 性能与动画 Hooks

#### `useAdaptiveFramePace.ts` — gsap ticker 动态帧率

- 设计注释（`useAdaptiveFramePace.ts:1-10`）：gsap 全局 ticker 是移动版所有常驻氛围动画（glow/呼吸）的单一驱动源，对 `gsap.ticker.fps()` 一处限帧即全量生效。**2026-09-13 功耗修复**（真机 25060RK16C 实测：前台静置 10s 渲染 2438 帧、RenderThread 40~44%）：原实现常驻 rAF 递归轮询 pace，页面永远存在 pending rAF 回调，Chromium 视其为「持续动画」按刷新率满帧派发 BeginFrame、合成器永不休眠；改为 ①档位回落后 setInterval 轮询（不产生 rAF）、②交互起始由事件回调即时提帧（零感知延迟），轮询只负责松手后的静止回落。
- 常量：`ACTIVE_WINDOW_MS = 2000`（行 16）、`PACE_POLL_MS = 250`（行 18，回落最迟延迟一拍 ≤250ms）。
- 模块级状态（行 20-23）：`lastActiveAt`、`idleFps = 30`、`activeFps = 60`、`appliedPace = -1`。
- `markInteraction()`（行 26-29）：更新 `lastActiveAt` 并立即 `applyPace()`；供面板切换等程序性交互标记。
- `currentPace()`（行 31-35）：`document.hidden` → 0（rAF 停转、ticker 自然暂停）；活跃窗口内 → `activeFps`；否则 `idleFps`。
- `applyPace()`（行 38-45）：档位未变则空操作；`gsap.ticker.fps(pace > 0 && pace < 60 ? pace : 60)`（行 44）。注释（行 42-43）：静止档氛围 tween 已被 usePageIdle 暂停、gsap ticker 由 autoSleep（main.tsx:18）停摆；60 表示不限帧（设备恒按旗舰档供帧，v2.4.0 起无档位）。
- `useAdaptiveFramePace()`（行 51-74）：在应用根部挂载一次。读 `useDeviceProfile()` 的 `idleFps`/`activeFps`（行 52-56）；`useEffect` 内监听 `pointerdown/touchstart/wheel/keydown`（passive + capture，行 60-61）、`visibilitychange`（行 65）与 `setInterval(applyPace, PACE_POLL_MS)`（行 64）；清理时移除监听、清定时器并恢复 `gsap.ticker.fps(60)`（行 67-73）。依赖数组 `[profile.idleFps, profile.activeFps]`（行 74）。交互动画（framer-motion/CSS transition）不受影响，天然在活跃窗口内（注释行 48-50）。

#### `useAnimationProfile.ts` — 动画配置档案

- `AnimationTier` 类型（行 6）：`'high' | 'standard' | 'economy'`。
- `AnimationProfile` 接口（行 8-26）字段：`tier`、`willChangeOrbs`、`magneticOffset`、`magneticDuration`、`numberDuration`、`springStiffness`、`springDamping`、`powerPreference`、`prefersCssAnimation`、`enableGpuCompositing`、`enablePageSlide`、`enableTilt`、`enableBackdropBlur`、`startupBoost`、`startupStaggerDelay`、`easing: EasingConfig`、`refreshRate`。
- 注释（行 28-30）：**v2.4.0 删除 standard/economy 分档及 prefers-reduced-motion / GPU 判定链**——档位省下的 GPU 开销换来体验不一致，不值；`tier` 字段与接口形状原样保留，消费方零改动；`refreshRate` 仍驱动 easing 曲线与节流数值。
- `HIGH_PROFILE` 常量（行 31-49）：恒最高档硬编码值——`tier: 'high'`、`willChangeOrbs: true`、`magneticOffset: 5`、`magneticDuration: 0.4`、`numberDuration: 600`、`springStiffness: 400`、`springDamping: 18`、`powerPreference: 'high-performance'`、`prefersCssAnimation: false`、`enableGpuCompositing: true`、`enablePageSlide: true`、`enableTilt: true`、`enableBackdropBlur: true`、`startupBoost: true`、`startupStaggerDelay: 0.05`、`easing: getEasingConfig(60)`、`refreshRate: 60`。
- `useAnimationProfile()`（行 51-57）：从 `useQualityStore` 读 `refreshRate`（行 52）；`effectiveRefreshRate = refreshRate > 0 ? refreshRate : 120`（行 54）；`useMemo` 返回 `{ ...HIGH_PROFILE, easing: getEasingConfig(effectiveRefreshRate), refreshRate: effectiveRefreshRate }`（行 55），依赖 `[refreshRate]`。`tier` 始终 `'high'`。

#### `useReducedMotion.ts` — 减少动态效果

- `QUERY = '(prefers-reduced-motion: reduce)'`（`useReducedMotion.ts:3`）。
- JSDoc（行 5-11）：与 CSS 的 `@media (prefers-reduced-motion: reduce)` 互补——CSS 查询管得住纯 CSS 过渡，管不住 JS 每帧写样式的动画（悬浮栏镜头系统就是这类），两处都要判。
- `useReducedMotion(): boolean`（行 12-30）：`useState` 惰性初始化（行 13-15，`typeof window !== 'undefined'` 守卫）；`useEffect` 中创建 `matchMedia` 监听 `change`（行 17-27），**兼容性处理**（行 20-26）：优先标准 `addEventListener('change')`，旧 WebView 只支持 `addListener` 时加兜底；清理函数对应移除监听。

### 5. 通用工具（`useAsyncLock.ts`）

- `useAsyncLock<T extends (...args: any[]) => Promise<any>>(fn, cooldownMs = 1500): [boolean, T]`（`useAsyncLock.ts:3-27`）：防重复触发的异步互斥锁 hook，返回 `[isRunning, execute]`。
  - `lockRef` 互斥、`mountedRef` 存活标记、`fnRef` 始终指向最新 fn（行 8-11）。
  - **StrictMode 修复**（行 12-17 注释）：setup→cleanup→setup 二次 setup 时恢复 `mountedRef = true`，否则 cleanup 置 false 后 `setTimeout` 内的锁释放被跳过、按钮永久转圈。
  - `execute`（行 18-25）：`lockRef.current` 已锁直接返回；执行完后在 `finally` 里 `setTimeout(cooldownMs)` 冷却，期满且组件仍挂载才释放锁与 `isRunning`（行 22-23）。

## Architecture

```text
┌────────────────────────────────────────────────────────────────┐
│             React 组件层（DockNav/TabletShell/SettingsPanel…）   │
│        订阅 store 状态 / 调用 action（useXxxStore()）            │
└───────────────────────────┬────────────────────────────────────┘
                            ▼
┌────────────────────────────────────────────────────────────────┐
│                  Zustand 领域 Stores（hooks/）                  │
│  useConfigStore ←→ useAuthStore ←→ useQualityStore(getState 协作)│
│  useThemeStore(subscribe→DOM 副作用)   useAdapterStore(面板导航)  │
└───────────────────────────┬────────────────────────────────────┘
                            ▼
┌────────────────────────────────────────────────────────────────┐
│        hooks/tauriApi.ts — tauriApiWithRetry (tauriApi.ts:356) │
│  invoke 封装 · desktopOnly/noopListener 降级 · createEventListener│
│  verifyWindowsIdentity · openExternal · withRetry(仅 saveConfig) │
└───────────────────────────┬────────────────────────────────────┘
                            ▼
┌────────────────────────────────────────────────────────────────┐
│           Rust 后端（Tauri commands + events 双向通道）           │
└────────────────────────────────────────────────────────────────┘
```

分层要点：

- **组件层只与 store 交互**，不直接 invoke；跨 store 协作（如 `doLogin` 读写 `useConfigStore`/`useQualityStore`）通过 `getState()` 在 store 层完成，不引入 React 上下文。
- **tauriApi.ts 是唯一的 IPC 出口**：所有命令调用与事件订阅都经过它，安卓端能力差异（桌面专属拒绝、空监听）在这一层集中收口，上层零感知。
- **副作用收口**：DOM 同步集中在 `useThemeStore` 的模块级 `subscribe`（`useThemeStore.ts:60-97`）；保存兜底集中在 `useConfigStore` 的模块级函数（`flushPendingConfig`/`hasPendingConfig`）；帧率副作用集中在 `useAdaptiveFramePace` 的模块级状态。

## Data Flow

### 1. IPC 调用链路

1. 组件 → store action → `tauriApiWithRetry.xxx` → `invoke(命令名, 参数)`。
2. 仅 `saveConfig` 带 `withRetry` 指数退避重试（`tauriApi.ts:361`，最多 2 次、500ms 基础延迟、仅 timeout/network/fetch/connection 类错误）。
3. 后端事件 → `createEventListener` 工厂（`tauriApi.ts:159-188`）→ 回调进 store。

### 2. 配置保存链路（含关窗兜底）

1. UI 调用 `useConfigStore.updateConfig`：合并 config → 标记 `dirtyFields` → 合并 `saveConfigPending`（密码 MASK 保护）→ 重置 500ms 去抖定时器（`useConfigStore.ts:56-88`）。
2. 定时器到期 → `saveConfigDirect` → `api.saveConfig`（带重试）→ 成功清 dirty 字段并同步 `passwordSaved`；失败上抛、计数、超 3 次放弃脏标记（`useConfigStore.ts:128-176`）。
3. 后端 `config-changed` 回传 → `mergeConfigFromBackend`：跳过 dirty 字段，防旧快照回滚（`useConfigStore.ts:103-119`）。
4. 关窗/后台化兜底：`hasPendingConfig()`（pending 或 in-flight 任一）→ `flushPendingConfig()` 清定时器、剔除 MASK 密码、立即保存并等待全部 settle（`useConfigStore.ts:188-224`）。

### 3. 认证与在线状态链路

1. 登录：`doLogin` 防重入 → 先 `saveConfigDirect` 持久化配置 → `withTimeout(api.doLogin(), 60s)` → 成功置 online（**不再立即复查**，`useAuthStore.ts:117-118`）→ 质量探测按 60s 节流（`useAuthStore.ts:117-129`）；失败才 `checkOnline` 复查。
2. 注销：`doLogout` 同款超时包装，结束后无条件 `checkOnline`（`useAuthStore.ts:192`）。
3. 状态同步：`checkOnline` 持串行锁（`useAuthStore.ts:197-198`）→ campus 检测（不在校园网即拦截）→ `queryPortalStatus('')`（空 IP，后端回退缓存源 IP/默认路由，`useAuthStore.ts:56-60`）→ `setStatusStable` 引用稳定更新。
4. 后端推送：`background-check-result`/`auto-login-result`/`login-log` 事件经 `tauriApi.ts:252-257` 的监听进入 `useAuthStore`（监听侧逻辑在 `useEventListeners`，不在本模块源码清单内）。

### 4. 网络质量节流链路

1. 任意来源获得质量结果（后台循环事件 `network-quality-result`、手动 `refreshQuality`、登录后探测）→ 一律经 `useQualityStore.setNetworkQuality` 写入 → `next !== null` 时 `lastQualityResultTime = Date.now()`（`useQualityStore.ts:78-82`）。
2. 手动 `refreshQuality`：锁 + `enableNetworkQuality` 开关 + 500ms 解锁缓冲（`useQualityStore.ts:54-74`）。
3. 登录后探测：距上次结果不足 60s 则跳过（`useAuthStore.ts:141`），避免与后端 latency loop/后台巡检重复全量检测（P2-F5）。
4. `refreshRate`（设备刷新率）由其他链路写入本 store，供 `useAnimationProfile` 消费。

### 5. 动画与帧率链路

1. 交互事件（pointerdown/touchstart/wheel/keydown/visibilitychange）→ `markInteraction` 推高 2s 活跃窗口并立即提帧（`useAdaptiveFramePace.ts:26-29`）。
2. 250ms setInterval 轮询负责松手后的静止回落：hidden → 0，静止 → `idleFps(30)`，活跃 → `activeFps(60)`（`useAdaptiveFramePace.ts:21-35`）。
3. `useDeviceProfile`（不在本模块清单）提供档位；`useAnimationProfile` 以 `refreshRate`（未获得时回退 120）生成 easing，其余参数恒 `HIGH_PROFILE`（`useAnimationProfile.ts:51-57`）。
4. 主题变化 → `useThemeStore.subscribe` 同步 DOM class 与 CSS 变量（`useThemeStore.ts:60-97`），动画外观随之切换。

## Connections

- [[android-frontend-ui]]：UI 组件（DockNav/TabletShell/SettingsPanel 等）消费各 store 状态并触发 action。
- [[android-backend-ipc]]：`tauriApi.ts` 对接的 Rust 后端命令与事件面。
- [[android-settings]]：`VALID_THEMES`/`VALID_GLASS_LEVELS`/`DEFAULT_CONFIG`/`Config` 类型等设置常量与类型来源。
- [[android-gsap-animation]]：gsap ticker 帧率控制与氛围动画（usePageIdle/useGlowAnimation 等）的配合。

## Known Issues

1. **CSS 变量累积泄漏（已修复）**：`useThemeStore` 全局订阅中 custom 主题曾用 `cssText +=` 累积声明，多次切换主题后 inline style 无界增长；现用 `style.setProperty` 覆盖同名变量、切回预设主题时 `removeProperty` 清理（`useThemeStore.ts:21-31`）。
2. **质量重复探测节流（P2-F5）**：登录后曾无条件手动全量探测，与后端 latency loop/后台巡检重复（后端 60s 节流只作用于循环路径）。现以 `lastQualityResultTime` 为任意来源统一时间戳 + `QUALITY_MANUAL_THROTTLE_MS = 60_000` 节流（`useQualityStore.ts:16-23`、`useAuthStore.ts:18-21`、`useAuthStore.ts:136-148`）。
3. **登录成功后的状态闪烁（已修复）**：登录成功立即 `checkOnline` 会在 Portal 会话未传播时返回 offline，把「登录成功」覆盖为「未登录」；现登录成功跳过立即复查、仅失败时复查（`useAuthStore.ts:156-161`）。
4. **登录/注销永久挂起（P2-34）**：后端阻塞（Portal HTTP 挂起等）曾使 `isLoggingIn/isLoggingOut` 永久为 true；现 `withTimeout` 60s 超时并在 settle 后清理定时器（`useAuthStore.ts:69-80`）。
5. **checkOnline 锁提前释放（已修复）**：曾用 `setTimeout(500)` 释放锁，执行超 500ms 时并发调用产生冗余 invoke；现 promise settle 时立即释放，锁持有时间与执行时间一致（`useAuthStore.ts:236-241`）。
6. **安卓端 Portal 状态恒灰（根因已修）**：checkOnline 曾走「解析失败判无网络」路径；现安卓无 adapters 概念，`adapterIp` 恒传空串、后端对空 IP 回退缓存源 IP/默认路由（`useAuthStore.ts:51-55`、`useAuthStore.ts:224-225`）。
7. **脏字段永久脱节（已修复）**：保存持续失败时 `dirtyFields` 曾被 `mergeConfigFromBackend` 永久跳过；现引入 `DIRTY_FAILURE_LIMIT = 3` 连续失败计数，超阈值放弃脏标记允许后端回传重新同步（`useConfigStore.ts:21-25`、`useConfigStore.ts:20-31`）。
8. **passwordSaved 残留（已修复）**：会话内保存密码后 `passwordSaved` 从不置 true、密码框显示空白；复位密码的 dirty 标记曾使清除密码后事件回传的空密码被跳过、状态永久残留 true。现保存成功/失败双向同步（`useConfigStore.ts:138-150`），后端回传密码字段同步口径（`useConfigStore.ts:110-118`）。
9. **关窗丢保存（已修复，两处）**：`hasPendingConfig` 曾只查 pending、漏掉 in-flight 保存（`useConfigStore.ts:188-193`）；`flushPendingConfig` 曾返回旧引用、新发出的一次保存随窗口销毁丢失（`useConfigStore.ts:188-195`）。另 `saveConfigInFlight` 必须仅清自己的引用，否则并发保存时 `hasPendingConfig` 漏报（`useConfigStore.ts:188-192`）。
10. **帧率轮询功耗（2026-09-13 修复）**：常驻 rAF 递归轮询使 Chromium 满帧派发 BeginFrame（真机 25060RK16C 静置 10s 渲染 2438 帧、RenderThread 40~44%）；改 setInterval 轮询 + 事件回调即时提帧（`useAdaptiveFramePace.ts:1-10`）。
11. **StrictMode 锁死（已修复）**：`useAsyncLock` 的 cleanup 置 `mountedRef=false` 后二次 setup 若不恢复，冷却 setTimeout 内锁永不释放、按钮永久转圈（`useAsyncLock.ts:3-8`）。
12. **旧 WebView matchMedia API 差异**：`useReducedMotion` 对 `addEventListener` 不可用时兜底 `addListener`（`useReducedMotion.ts:20-26`）。
13. **桌面专属 API 的安卓降级**：`tauriApi.ts` 对 18 个桌面专属命令统一 `desktopOnly` 直接拒绝（清单见 Key Components），对 7 个桌面专属事件用 `noopListener` 空监听（`tauriApi.ts:14-19`）；上层代码零改动且不会误调崩溃。
14. **生物识别验证门统一上报**：`verifyWindowsIdentity` 无论系统 BiometricPrompt 还是应用内 2D 人脸回退，真实结果一律上报后端 `verify_biometric_identity`，取消/失败/基础设施异常都归一化为 `{ success: false }` 返回值而非 IPC 故障（`tauriApi.ts:207-244`）。