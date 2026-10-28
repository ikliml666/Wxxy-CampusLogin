---
title: 安卓端 React 前端壳 (android/frontend)
type: module
source_files:
  - android/frontend/package.json
  - android/frontend/vite.config.ts
  - android/frontend/src/hooks/tauriApi.ts
  - android/frontend/src/App.tsx
  - android/frontend/src/main.tsx
  - android/frontend/src/hooks/useAppInit.ts
  - android/frontend/src/hooks/useInitialDataLoad.ts
  - android/frontend/src/hooks/useFormFactor.ts
  - android/frontend/src/account/AccountPanel.tsx
  - android/frontend/src/account/SelfServicePanel.tsx
  - android/frontend/src/account/useAccount.ts
  - android/frontend/src/monitor/MonitorPanel.tsx
  - android/frontend/src/monitor/QualityPanel.tsx
  - android/frontend/src/monitor/useMonitor.ts
  - android/frontend/src/face/faceVerifyStore.ts
  - android/frontend/src/face/FaceCaptureDialog.tsx
  - android/frontend/src/face/faceService.ts
  - android/frontend/src/settings/SettingsPanel.tsx
  - android/frontend/src/settings/OnboardingWizard.tsx
  - android/frontend/src/settings/constants.ts
  - android/src-tauri/src/lib.rs
  - android/README.md
tags: [android, react, tauri, frontend, ipc, mobile, tablet, face-verification]
---

## Overview

安卓端（`android/frontend`）是一层用 React + Vite + Tauri v2 构建的「前端壳」，与桌面版（`../tauri-app`，见 [[desktop-tauri-app]] 与 [[desktop-frontend]]）**同构命令/IPC 契约层**，但重新为移动形态（手机单列卡流 + 平板 Dock 布局）做过布局重写，并显式拒绝/空监听所有桌面专属能力。

核心设计要点（均来自源码，非推测）：

- **命令契约层集中在一处**：`src/hooks/tauriApi.ts`（第 1–381 行）导出 `tauriApi` 与 `tauriApiWithRetry` 两个对象，封装 `@tauri-apps/api` 的 `invoke`，是前端与安卓后端（`android/src-tauri/src/lib.rs` `invoke_handler`，第 68–130 行）之间唯一的 IPC 边界。
- **桌面专属命令显式拒绝**：`desktopOnly<T>(name)`（第 14–15 行）把所有桌面专属命令（`export_config`、`import_config`、`export_diagnostics`、`get_adapters`、`enable_adapter`、`get_adapter_details`、`dhcp_renew_all`、`minimize_window`、`close_window`、`cancel_auto_exit`、`show_window`、`dhcp_release_renew`、`dhcp_release_renew_adapter`、`check_dns_doh_status`、`setup_dns_doh`、`reset_dns`、`render_heartbeat`、`notify_window_ready` 等）统一 `Promise.reject(new Error('桌面专属功能,安卓端不可用: <name>'))`。
- **桌面专属事件走空监听**：`noopListener`（第 18 行）返回 `() => {}` 清理函数，挂载于 `onAdaptersChanged`/`onAdapterDetailsChanged`/`onAdapterDisabledWarning`/`onAutoExitCountdown`/`onAutoExitCancelled`/`onCampusExitCountdown`/`onCampusExitCancelled`（第 272–318 行），订阅代码零改动且永不触发。
- **双外壳入口**：`src/App.tsx`（第 376–402 行）按 `useFormFactor()` 在 `phone`/`tablet` 间二选一渲染 `AppInner`（手机）或 `TabletShell`（平板，`lazy` 懒加载）。
- **启动初始化与崩溃恢复**：`src/main.tsx`（第 1–146 行）负责 GSAP 初始化、首帧主题预置、渲染崩溃自动恢复（最多 3 次 reload）、安卓 `onTrimMemory` 兜底释放人脸引擎。

`@shared` 别名说明：README（第 8 行）称「`@shared` alias 指向主仓库前端共享层」，但本模块 `vite.config.ts`（第 11–13 行）实际只声明了 `@` → `./src`；共享层（`@/shared`、`@/components/ui` 等）是通过 `android/frontend/src/shared` 等本地同构副本 + path 依赖主仓库协议核心实现，并非运行时 alias（详见 [[android-frontend-shared-note]]）。

## Key Components

### 命令契约层：`src/hooks/tauriApi.ts`（第 1–381 行）

安卓 IPC 边界的唯一封装处。所有前端→后端调用、所有事件订阅都经此文件。

- `desktopOnly<T>(name)` — 第 14–15 行。桌面专属命令统一 reject：`Promise.reject(new Error('桌面专属功能,安卓端不可用: ' + name))`。
- `noopListener` — 第 18 行。返回 `() => {}` 的空监听工厂，桌面专属事件订阅点零改动。
- `interface TauriApi` — 第 48–153 行。完整契约声明（约 95 个方法），`desktopOnly` 命中的方法在此处有注释标明，例如 `exportConfig`（第 51 行注释）、`getAdapters`/`enableAdapter`/`getAdapterDetails`（适配器）、`minimizeWindow`/`closeWindow`（窗口）、`dhcpRenewAll` 等（DHCP/DNS）、`renderHeartbeat`（第 145 行「轻量化重建窗口的 ready 门信号」）、`notifyWindowReady`（第 146 行）。
- `interface BatteryOptimizationInfo` — 第 26–30 行。`{ ignoring, brand, hasVendorTarget }`，对应 `get_battery_optimization_info`。
- `interface VendorSettingsResult` — 第 32–36 行。`{ path, target, tried }`，对应 `open_vendor_battery_settings`。
- `interface CampusStatusResult` — 第 38–46 行。校园网探针结果（WiFi/有线双通道）。
- `interface AvoidBadWifiStatus` — 第 155–164 行。`{ granted, avoidBadWifi, pendingRestoreAvoid, pendingRestoreCaptive }`，对应 `get_avoid_bad_wifi_status`。
- `interface RestoreWrittenSettingsResult` — 第 166–169 行。`{ restored: Record<string,boolean>, reason? }`，对应 `restore_written_settings`。
- `createEventListener<T>(eventName)` — 第 171–200 行。带 `cancelled` 守卫的通用事件订阅工厂，返回 `(cb) => 清理函数`；`listen` 未完成时取消也能正确清理。
- `const tauriApi: TauriApi` — 第 202–342 行。契约实现。逐方法映射：
  - 真实落地的命令（非 desktopOnly）：`getConfig`（203）、`saveConfig`（204）、`bindToWifi`（211）、`checkPortalStatus`（212）、`checkCampusStatus`（213）、`requestWifiSsidPermission`（214）、`doLogin`/`doLogout`（215–216）、`bindOperator`/`getBindStatus`/`revealOperatorCredential`（217–218、257）、`verifyWindowsIdentity`（219–256，含人脸回退逻辑）、`querySelfDashboard`/`selfOfflineSession`/`querySelfOnlineLog`/`changeSelfPassword`（258–261）、`vpnSaveCredential`/`vpnCredentialStatus`/`vpnClearCredential`/`setSelfServiceViaVpn`/`vpnTestConnection`/`vpnVerifyCredential`（262–267）、`listAccounts`/`createAccount`/`switchAccount`/`saveCurrentAsAccount`/`renameAccount`/`getActiveAccount`（276–282）、`startBackgroundCheck`/`stopBackgroundCheck`/`triggerBackgroundCheck`/`getBackgroundStatus`（283–286）、`checkNetworkQuality`/`startLatencyTest`/`stopLatencyTest`（290–293）、`openExternal`（294–305，http(s) 校验后调 `openerOpenUrl`）、`getAutoLaunch`/`setAutoLaunch`（306–307，`set` 返回 `{success:true}`）、`getNotificationEnabled`/`setNotificationEnabled`（308–309）、`getBatteryOptimizationInfo`/`requestIgnoreBatteryOptimizations`/`openVendorBatterySettings`/`openNotificationSettings`（310–313）、`getLogs`/`clearLogs`/`getDebugMode`/`setDebugMode`/`getLogRetentionDays`/`setLogRetentionDays`（321–339）、`getInitData`/`checkUpdate`/`downloadUpdate`/`installUpdate`/`getMirrorUrls`（325–329）、`getAvoidBadWifiStatus`/`restoreWrittenSettings`（340–341）、人脸模型 `faceModelsState`/`faceModelsDownload`（未在 tauriApi 暴露，由 `faceService.ts` 直接 `invoke`）。
  - `verifyWindowsIdentity`（第 219–256 行）：验证门统一出口。若 `shouldUseFaceFallback()` 为真且系统生物识别 `!isAvailable`，则走应用内人脸验证（`useFaceDialogStore.openFaceDialog('verify')`）；否则调 `biometricAuthenticate(consentMessage, { allowDeviceCredential: true })`。任何结果（成功/取消/失败）都归一到 `verify_biometric_identity({ success, method })`，不把失败当 IPC 故障抛出（第 220–256 行注释详述）。
  - `desktopOnly` 命中的命令：`exportConfig`（205）、`importConfig`（206）、`exportDiagnostics`（207）、`getAdapters`（208）、`enableAdapter`（209）、`getAdapterDetails`（210）、`minimizeWindow`（268）、`closeWindow`（269）、`dhcpRenewAll`（287）、`dhcpReleaseRenew`（288）、`dhcpReleaseRenewAdapter`（289）、`cancelAutoExit`（314）、`showWindow`（320）、`checkDnsDohStatus`（333）、`setupDnsDoh`（334）、`resetDns`（335）、`renderHeartbeat`（336）、`notifyWindowReady`（337）。
  - 空监听挂载点（`noopListener`）：`onAdaptersChanged`（272）、`onAdapterDetailsChanged`（273）、`onAdapterDisabledWarning`（274）、`onAutoExitCountdown`（315）、`onAutoExitCancelled`（316）、`onCampusExitCountdown`（317）、`onCampusExitCancelled`（318）。
  - 真实事件订阅（`createEventListener`）：`onBackgroundCheckResult`（270）、`onAutoLoginResult`（271）、`onLoginLog`（275）、`onNetworkQualityResult`（291）、`onConfigChanged`（319）、`onDownloadProgress`（330）、`onUpdateAvailable`（331）、`onUpdateUnverified`（332）。
- `isRetryableError(e)` — 第 344–354 行。判断 timeout/network/fetch/connection 关键字。
- `withRetry<T>(fn, maxRetries=2, baseDelay=500)` — 第 356–372 行。指数退避重试。
- `export const tauriApiWithRetry: TauriApi` — 第 374–380 行。展开 `tauriApi` 仅覆盖 `saveConfig` 加 `withRetry`（仅保存是一次性关键操作）。

### 双外壳入口：`src/App.tsx`（第 1–433 行）

- `const TabletShell = lazy(...)` — 第 39–41 行。平板外壳懒加载（手机端永不求值）。
- 其余 `lazy` 入口：`AboutDialog`（54）、`MobileQuickActions`（55）、`ThemeDialog`（56）、`OnboardingWizard`（57）。
- `function AppInner()` — 第 59–368 行。手机外壳主体：
  - 页签状态 `tab`（第 64–69 行），白名单 `['dashboard','account','selfservice','quality','monitor','more']`（KI#14）。
  - `tabOrder`（第 77–83 行）：质量开关关闭时第 4 位用 `monitor` 替 `quality`。
  - `handleTabChange`（第 126–134 行）：方向性转场，持久化 `campus-mobile-tab`。
  - 首次启动无账号 → 弹新手向导（第 145–153 行）。
  - `panelContent` 分发（第 155–199 行）：dashboard/account/selfservice/quality/monitor/more 六个分支，分别渲染 `MobileDashboard`/`AccountPanel`/`SelfServicePanel`/`QualityPanel`/`MonitorPanel`/`MobileMore`。
  - 顶部轻 header（第 209–267 行）：状态色点、在线运营商标、更新胶囊、`NetworkQualityCapsule`、`SponsorCard`/`Settings`/`Palette`/`Info` 入口。
  - `BottomNav`（第 306 行）、`ToastContainer`、`SponsorCard`、`AboutDialog`、`UpdateAvailableDialog`、`ThemeDialog`、`ConfirmDialog`（删除确认）、`OnboardingWizard`（第 308–365 行）。
- `export default function App()` — 第 376–402 行。形态分发：`useFormFactor()` 判定 `isTablet`，是则 `Suspense` 懒渲染 `TabletShell`，否则 `AppInner`；平板判定后立刻 `import('@/components/tablet/TabletShell')` 预取（第 381–383 行）。根部常驻 `FaceCaptureDialog`（第 397 行）与 `NotificationPermissionGate`（第 398 行）。
- `function NotificationPermissionGate()` — 第 408–432 行。首启通知权限引导，标记 `campus-notification-prompt` 只问一次。

### 启动初始化与崩溃恢复：`src/main.tsx`（第 1–146 行）

- `import.meta.env` 分支：`DEV` 用 `React.StrictMode`，否则 `React.Fragment`（第 131–133 行）。
- `gsap.defaults/config/ticker`（第 17–25 行），`prefers-reduced-motion` 降级（第 21–25 行）。
- `function initTheme()` — 第 27–44 行。首帧前定下 `dark`/`light` + `theme-*` class + `data-glass` 档位，避免启动闪烁。
- `function setupCrashRecovery()` — 第 48–114 行。
  - `MAX_CRASH_RELOADS = 3`（第 50 行）；`tryRecover`（第 52–60 行）。
  - `visibilitychange` 检测空 `#root`（第 62–69 行）。
  - `window error` 捕获 GPU/WebGL/SharedArrayBuffer（第 71–77 行）。
  - rAF 心跳 `isRenderLoopAlive()`（来自 `@/lib/renderLiveness`），停滞超 10s 触发恢复（第 79–103 行）。
  - 10s 阈值判定（第 105–113 行，FE-A-11 放宽）。
- `window.__onTrimMemory = (level)` — 第 123–129 行。安卓内存压力钩子，高压力（`level >= 10`）且非会话中进行时 `import('./face/faceService').then(m => m.releaseFaceEngine())` 兜底释放人脸引擎。
- 根渲染（第 135–145 行）：`ErrorBoundary` → `LazyMotion(domMax, strict)` → `MotionConfig(reducedMotion="user")` → `App`。

### 启动引导 hook 链

- `useAppInit()` — `src/hooks/useAppInit.ts` 第 6–11 行。组合 `useEventListeners`/`useInitialDataLoad`/`useHeartbeat`/`useGlobalShortcut`。
- `useInitialDataLoad()` — `src/hooks/useInitialDataLoad.ts` 第 17–114 行。调用 `api.getInitData()`（第 31 行），合并 `DEFAULT_CONFIG`（第 34 行），据 `PASSWORD_MASK` 定 `passwordSaved`（第 35–39 行），初始化主题/激活面板/在线状态/账号列表，最后置 `configLoaded=true`（第 101 行）。明确删除了桌面的 `getAdapters`/GPU/DNS DoH 补充请求（第 88–90 行注释）。
- `useFormFactor()` — `src/hooks/useFormFactor.ts` 第 38–40 行。短边 ≥ 600 CSS px（sw600dp）判为 `tablet`（第 13–17 行）。另导出 `useOrientation`（第 43–45 行）。`FormFactor = 'phone' | 'tablet'`（第 9 行）。

### 账号面板：`src/account/AccountPanel.tsx`（第 1–510 行）

- `export const AccountPanel = memo(...)` — 第 50–509 行。`AccountPanelProps`（第 32–41 行）。单列纵排：账号管理卡（上，含 `OnlineDeviceStack` 账号卡序列 + 添加/编辑登录信息 + 绑定运营商快捷操作）+ 自动化设置卡（下，三开关：`autoLoginOnStart`/`autoLoginOnPreparation`/`enableNightOperatorSwitch`）。
  - `handleClearPassword`（第 100–115 行）：走 `store.saveConfigDirect({password:''}, true)` 显式清密。
  - `handleSwitchAccount`（第 119–123 行）：busy 态防重。
  - `handleCardEdit`（第 127–134 行）：非激活卡先切后编辑。
  - `submitRename`（第 149–164 行）：内联改名，校验经 `isValidDisplayName`。
  - `refreshAccounts`（第 168–175 行）：`tauriApiWithRetry.listAccounts()`。
  - `handleSaveLoginInfo`（第 180–234 行）：新建走 `createAccount`，编辑走 `saveConfigDirect` + `saveCurrentAsAccount`。
  - `useOperatorBind`（第 239 行）：绑定运营商状态机（逻辑在 `useOperatorBind.ts`）。

### 自助服务面板：`src/account/SelfServicePanel.tsx`（第 1–484 行）

- `interface SelfOnlineItem`（第 28–38 行）：在线会话字段（loginTime/ip/mac/useTime/.../sessionId）。
- `interface VpnStatus`（第 41–45 行）：`{ saved, accountMasked, viaVpn }`。
- `formatUseTimeMinutes`（第 61–64 行）、`formatFlowMb`（第 67–71 行）、`localDateStr`（第 75–78 行）、`formatTerminalType`（第 55–58 行）、`toInt`（第 49–52 行）：数据格式化导出函数。
- `export function SelfServicePanel()` — 第 80–483 行。初次切入自动 `ensureSelfVerified()`（来自 `selfServiceState.ts`）后拉取 dashboard（第 109、157–163 行）；`fetchDashboard`（第 127–151 行）、`handleOffline`（第 165–190 行）、`handleSaveAlias`（第 193–210 行，写 `config.deviceAliases`）、`refreshVpnStatus`/`handleVpnTest`/`handleVpnSave`/`handleVpnToggle`/`handleVpnClear`/`fetchOnlineLog`（第 213–319 行）。平板用 `OnlineDeviceStackH`，手机用 `OnlineDeviceStack`（第 380–418 行）。

### 账号 hook：`src/account/useAccount.ts`（第 1–167 行）

- `let switchInFlight`（第 12 行）：切号去重共享 Promise。
- `export function useAccount()` — 第 14–166 行。返回 `handleAddAccount`（40–65）、`handleDeleteAccount`（67–92）、`handleSwitchAccount`（94–128，含 `switchInFlight` 去重与 `!== undefined` 判定修复 R4）、`handleRenameAccount`（132–157）。回传的掩码 config 整份落地 + `clearDirtyFields` + `syncPasswordSaved(config.password === PASSWORD_MASK)`（第 48–55 行模式）。

### 监控面板：`src/monitor/MonitorPanel.tsx`（第 1–435 行）

- `MonitorSettingRow`（第 29–48 行）：本地设置行组件（安卓无桌面 `SettingRow`）。
- `export const MonitorPanel = memo(...)` — 第 50–434 行。`MonitorPanelProps`（第 21–25 行）。三卡：控制条（开关 + 间隔 + 触发器）/ 自动化（仅 `enableBackgroundCheck` + `autoLoginOnPreparation`，无在线退出/非校园网退出/退出生效时段能力）/ 校园网校验（`enableNetworkNameCheck`、`requiredNetworkName`、`campusGateway`、`campusCheckStart/EndMinutes`、`scheduledLogin/logoutMinutes`）。`intervalDraft` 本地草稿 + `commitInterval`（第 59–77 行）；`useAsyncLock` 防连点（第 62–68 行）。

### 质量面板：`src/monitor/QualityPanel.tsx`（第 1–548 行）

- `DETAIL_CATEGORIES`（第 39–76 行）：4 类探测指标（gatewayDns/http/stream/game）。
- `PAGE_COUNT`（第 78 行）、`cardItemVariantsNoY`（第 80–83 行）。
- `export const QualityPanel = memo(...)` — 第 85–547 行。`QualityPanelProps`（第 30–34 行）。定时测试 + 测试详情合并卡：启停开关 + 间隔输入 + 周期进度轨 + 4 页横滑分类详情（translateX 平移 + 拖拽 `onDragEnd` 落页，第 148–192 行）。空态四段式（第 254–268 行），UDP 异常提示（第 472–474 行）。

### 监控 hook：`src/monitor/useMonitor.ts`（第 1–124 行）

- `export function useMonitor()` — 第 12–123 行。返回 `handleToggleBackgroundCheck`（第 39–81 行，先存配置再请求通知权限 + SSID 权限，最后 start/stop）、`handleTriggerCheck`（第 83–85 行）、`handleToggleLatencyTest`（第 87–115 行，同样先存配置）。`bgToggleBusyRef`/`latencyToggleBusyRef` 重入锁（第 36–37 行）。

### 人脸验证（应用内 2D 比对）：`src/face/*`

- `src/face/faceVerifyStore.ts`（第 1–47 行）：
  - `type FaceDialogMode = 'enroll' | 'verify'`、`type FaceDialogResult = { ok: boolean; reason?: string }`（第 14–15 行）。
  - `useFaceDialogStore`（第 25–41 行）：zustand store，`open`/`mode`/`resolve`/`openFaceDialog`/`closeFaceDialog`。
  - `shouldUseFaceFallback()`（第 44–46 行）：`config.allow2dFaceVerify === true && hasTemplate()`。
- `src/face/FaceCaptureDialog.tsx`（第 1–197 行）：
  - `FaceCaptureDialog`（第 28–46 行）：常驻单例弹窗，受 `useFaceDialogStore` 驱动。
  - `CaptureFlow`（第 48–196 行）：`enroll`（自动采帧进度 x/8）或 `verify`（随机动作挑战 + 本地比对）；`ensureFaceModels`（第 67 行）→ `openCamera`（第 80 行）→ 流程 → 关闭即 `releaseFaceEngine`（第 120 行）。
- `src/face/faceService.ts`（第 1–354 行）：
  - `TEMPLATE_KEY = 'campus-2d-face-template'`（第 22 行）；`MATCH_THRESHOLD = 0.55`（38）、`ENROLL_FRAMES = 8`（40）、`ENROLL_TIMEOUT_MS`（42）、`CHALLENGE_TIMEOUT_MS`（44）、`POLL_INTERVAL_MS`（46）、`DETECT_SCORE_MIN`（48）。
  - `FaceChallenge = 'blink' | 'turn'`（第 50 行）、`FaceFailReason`（第 52 行）、`FaceVerifyResult`（第 53 行）。
  - `ensureFaceModels(onProgress?)`（第 83–91 行）：共享在途 Promise 下载模型到私有目录（模型不打包 APK，v2.4.0）。
  - `downloadModels`（第 93–112 行）：`invoke('face_models_state')`/`invoke('face_models_download')` + 监听 `face-models-download-progress`。
  - `getHuman()`（第 118–162 行）：`@vladmandic/human` 动态 import + WebGL + warmup，`engineGen` 代际防竞态。
  - `releaseFaceEngine()`（第 179–192 行）：同步置空单例 + reset 孤儿实例。
  - `openCamera`/`closeCamera`（第 194–207 行）。
  - `detectOnce`（第 218–236 行）：human `detect` + 眨眼/转头判据。
  - `loadTemplate`/`hasTemplate`/`clearTemplate`/`saveTemplate`（第 245–266 行）。
  - `enrollFace(video, onProgress, onFacePresence?)`（第 269–300 行）：质量门控采 8 帧均值作模板。
  - `verifyFace(video, onChallenge, onFacePresence?)`（第 303–353 行）：随机挑战 → 同人比对（最多 3 帧，阈值 0.55）。

### 设置面板：`src/settings/SettingsPanel.tsx`（第 1–453 行）

- `const isAndroid = import.meta.env.VITE_PLATFORM === 'android'`（第 29 行）：安卓差异化渲染（Windows 专属设置不渲染）。
- `SettingsPanelProps`（第 31–35 行）。
- `export const SettingsPanel = memo(...)` — 第 37–452 行。新手指引 + 两列区（启动与通知/自动化/保活 + 安全/引导）+ 网络组。
  - `handleSecurityDisable`（第 56–70 行）：关闭安全开关先过 `verifyWindowsIdentity`。
  - 保活设置卡 `KeepAliveSettingsCard` 仅 `isAndroid` 时渲染（第 217–223 行）。
  - 2D 人脸风险确认 `faceRiskConfirmOpen`（第 321–330 行）；关闭即 `clearTemplate()`（第 289 行）。
  - 延迟计算选项 Modal（第 393–439 行）：`skipTtfbInLatency`/`skipContentInLatency`。

### 新手向导：`src/settings/OnboardingWizard.tsx`（第 1–632 行）

- `STEP_TITLE_KEYS`/`STEP_COUNT = 4`（第 42–43 行）；`OPERATOR_VALUE_BY_KEY`（第 46–50 行）；`STACK_LAYERS`（第 55–60 行）；`EXIT_UP_MS = 240`（63）；`deckTransform`/`EXIT_UP_TRANSFORM`/`ENTER_FROM_TRANSFORM`（第 67–71 行）。
- `DeckArrow`（第 77–100 行）：卡堆前进/后退箭头。
- `export function OnboardingWizard({ open, onClose, onUpdateConfig, onLogin, isLoggingIn })` — 第 107–631 行。复用 `useOnboardingFlow`（跨手机/平板共用流程逻辑）。卡堆测量 `measureDeck`（第 138–158 行）、`deckAdvance`（第 178–194 行，前进飞起 + 入场浮起）、`deckGoNext`（第 198–205 行）、4 步内容（i=0 欢迎/i=1 账号/i=2 绑定运营商/i=3 完成，第 282–540 行）、跳过关确认（第 608–628 行）。

### 设置常量：`src/settings/constants.ts`（第 1–108 行）

- `DEFAULT_CONFIG: Config`（第 5–59 行）：安卓默认配置（注意 `configSchemaVersion: 7`、`latencyTestInterval: 600000`、`backgroundCheckInterval: 60000` 等，与后端 `Settings::default` 对齐）。
- `ISP_OPTIONS`（第 61–66 行）：运营商下拉（含 `__default__`=无锡学院）。
- `operatorLabelKey(suffix)`（第 73–76 行）：后缀→i18n labelKey。
- `THEME_OPTIONS`/`VALID_THEMES`（第 78–88 行）、`GLASS_OPTIONS`/`VALID_GLASS_LEVELS`（第 96–102 行）、`DEFAULT_PANEL_OPTIONS`（第 104–107 行）。

### 构建配置：`android/frontend/vite.config.ts`（第 1–45 行）

- `@` → `path.resolve(dirname, './src')`（第 11–13 行）。**仅此一个 alias**，README 所提 `@shared` 非此处声明。
- `manualChunks`（第 35–40 行）：`vendor-react`/`vendor-motion`/`vendor-gsap`/`vendor-radix` 分包。
- `server.port = 5174`、`strictPort`（第 18–19 行）；`watch.ignored` 跳过 `.*.tmpdir`（第 20–23 行）。

## Architecture

```text
┌──────────────────────────────────────────────────────────────────────┐
│  main.tsx (启动/崩溃恢复/TrimMemory)                                   │
│   initTheme → setupCrashRecovery → ReactDOM.render(ErrorBoundary▶App)  │
└───────────────────────────────┬──────────────────────────────────────┘
                                 │ useAppInit
                                 ▼
┌──────────────────────────────────────────────────────────────────────┐
│  App.tsx  (双外壳)                                                    │
│  useFormFactor()→ phone? AppInner : TabletShell(lazy)                  │
│   AppInner: BottomNav(6 tab)▶ AccountPanel / SelfServicePanel /        │
│            QualityPanel / MonitorPanel / MobileDashboard / MobileMore  │
│   根部常驻: FaceCaptureDialog + NotificationPermissionGate             │
└───────────────────────────────┬──────────────────────────────────────┘
                                 │ 全部 IPC 经
                                 ▼
┌──────────────────────────────────────────────────────────────────────┐
│  hooks/tauriApi.ts  (契约层)                                          │
│   tauriApi (invoke) ──真实命令──▶ 安卓后端 lib.rs invoke_handler       │
│   desktopOnly ── reject ──▶ export_config/get_adapters/dhcp/...       │
│   noopListener ── 空监听 ─▶ onAdaptersChanged/onAutoExitCountdown/... │
│   createEventListener ─▶ onNetworkQualityResult/onConfigChanged/...   │
│   tauriApiWithRetry (仅 saveConfig 带指数退避重try)                    │
└───────────────────────────────┬──────────────────────────────────────┘
                                 │ Tauri v2 invoke / listen
                                 ▼
┌──────────────────────────────────────────────────────────────────────┐
│  android/src-tauri/src/lib.rs invoke_handler (第68-130行)              │
│   protocol_cmds/campus_detect/config_state/self_service_cmds/vpn_cmds/ │
│   account_cmds/system_cmds/monitor_loop/battery_cmds/quality_cmds/     │
│   update_cmds/face_model_cmds  (与桌面版同名对齐)                      │
└──────────────────────────────────────────────────────────────────────┘
        ▲ 协议核心 path 依赖主仓库 campus-login ([[desktop-tauri-app]])
```

## Data Flow

1. **启动加载**：`main.tsx` 先 `initTheme()`（第 27–44 行）预置主题/玻璃档位 → `setupCrashRecovery()`（第 48–114 行）→ 渲染 `App`。`AppInner` 调 `useAppInit()`（组合 `useInitialDataLoad`）→ `api.getInitData()`（`useInitialDataLoad.ts` 第 31 行）拉全量配置 + 掩码密码 + 后台状态 + 账号列表，合并 `DEFAULT_CONFIG` 后写入各 store，最后置 `configLoaded=true`（第 101 行）。桌面遗留的 `getAdapters`/GPU/DNS DoH 补充请求被显式删除（第 88–90 行）。
2. **命令调用**：组件经 `tauriApiWithRetry`（或 `tauriApi`）调后端。例如 `AccountPanel.handleSaveLoginInfo`（第 180 行）→ `createAccount`/`saveConfigDirect`/`saveCurrentAsAccount`；`useMonitor.handleToggleBackgroundCheck`（第 39 行）→ 先 `saveConfigDirect` 再 `startBackgroundCheck`；`SelfServicePanel.fetchDashboard`（第 127 行）→ `querySelfDashboard`。
3. **事件回流**：后端 `emit` 的事件经 `createEventListener` 订阅（`onNetworkQualityResult`/`onConfigChanged`/`onBackgroundCheckResult`/`onLoginLog`/`onUpdateAvailable` 等）；桌面专属事件（`onAdaptersChanged` 等）走 `noopListener` 永不触发。模型下载进度走 `face-models-download-progress`（faceService.ts 第 99 行）。
4. **桌面专属命令拒绝路径**：任何调 `exportConfig`/`getAdapters`/`dhcp*`/`minimizeWindow`/`closeWindow`/`showWindow`/`check_dns_doh_status`/`reset_dns`/`render_heartbeat`/`notifyWindowReady`/`cancel_auto_exit` 的入口（若未来被误引）会同步 `reject('桌面专属功能,安卓端不可用: <name>')`；上层 try/catch 将其当作错误而非静默成功处理。
5. **人脸验证门**：`SettingsPanel` 开启 `allow2dFaceVerify` → `SettingsPanel` 触发 `openFaceDialog('enroll')`（`faceVerifyStore`）；`verifyWindowsIdentity`（tauriApi 第 219 行）在系统生物识别不可用时经 `shouldUseFaceFallback()` 落到 `openFaceDialog('verify')` → `FaceCaptureDialog`/`faceService` 跑 human 比对 → `closeFaceDialog` 回传 `{ok,reason}` → `report()` 调 `verify_biometric_identity`。
6. **形态切换**：`useFormFactor()` 据短边 ≥600dp 判平板，`App` 懒加载 `TabletShell`（面板仍用安卓版组件）。

## Connections

- [[desktop-tauri-app]]：安卓后端 `lib.rs` 的协议核心（登录/注销/自助服务）经 Cargo path 依赖主仓库 `tauri-app/src-tauri` 单点维护，命令名同名对齐。
- [[desktop-frontend]]：桌面端 React 前端是这些面板的「同构源」，本模块据此做安卓布局重写与 desktopOnly 拒绝。
- [[android-backend]]（即 `android/src-tauri`）：`invoke_handler` 注册的命令面，本模块 `tauriApi.ts` 逐一对应。
- [[android-frontend-shared-note]]：`@shared` alias 与本地共享层（`src/shared`、`src/components/ui`）的真实来源说明（vite 只声明 `@`，README 的 `@shared` 描述需校正）。
- 相关 Rust 模块：`protocol_cmds`、`campus_detect`、`config_state`、`self_service_cmds`、`vpn_cmds`、`account_cmds`、`system_cmds`、`monitor_loop`、`battery_cmds`、`quality_cmds`、`update_cmds`、`face_model_cmds`（均见 `android/src-tauri/src/`）。

## Known Issues

- **`desktopOnly` 命令若被误引会直接 reject**：`export_config`/`import_config`/`export_diagnostics`/`get_adapters`/`enable_adapter`/`get_adapter_details`/`dhcp_renew_all`/`dhcp_release_renew`/`dhcp_release_renew_adapter`/`minimize_window`/`close_window`/`cancel_auto_exit`/`show_window`/`check_dns_doh_status`/`setup_dns_doh`/`reset_dns`/`render_heartbeat`/`notify_window_ready`（tauriApi.ts 第 205–337 行）——桌面专属，安卓无对应实现，调用即 `Promise.reject`。
- **桌面专属事件空监听**：`onAdaptersChanged`/`onAdapterDetailsChanged`/`onAdapterDisabledWarning`/`onAutoExitCountdown`/`onAutoExitCancelled`/`onCampusExitCountdown`/`onCampusExitCancelled`（tauriApi.ts 第 272–318 行）挂载 `noopListener`，永不触发回调。
- **README `@shared` 描述与实际不符**：`android/README.md` 第 8 行称「`@shared` alias 指向主仓库前端共享层」，但 `vite.config.ts` 第 11–13 行只声明 `@` → `./src`，并无 `@shared` alias；共享 UI/类型经本地同构副本 + path 依赖实现。
- **崩溃恢复阈值/行为**：`main.tsx` 第 105–113 行 rAF 心跳阈值放宽到 10s（FE-A-11），避免低端机长 GC 误判；超出 `MAX_CRASH_RELOADS=3`（第 50 行）后停止自动恢复。
- **`getInitData` 失败降级**：`useInitialDataLoad.ts` 第 102–108 行，拉取失败时降级为 `DEFAULT_CONFIG` 并写日志（旧实现静默吞错）。
- **`setSelfServiceViaVpn` 等回写依赖 `viaVpn` 字段**：`SelfServicePanel` 中 `handleVpnToggle`（第 257–274 行）在失败时用本地 `rollback()` 回滚 `viaVpn` 显示，避免与后端状态分叉。
- **人脸引擎释放竞态**：`faceService.ts` 的 `releaseFaceEngine`（第 179 行）用 `engineGen` 代际处理「取消后立刻重开」；`main.tsx` 的 `__onTrimMemory`（第 123 行）仅在 `level >= 10` 且非 `__faceSessionActive` 时释放，会话中释放会让轮询循环打到已 reset 实例直至静默超时。
