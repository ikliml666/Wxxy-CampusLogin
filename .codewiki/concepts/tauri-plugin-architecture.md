---
title: tauri-plugin-architecture
type: concept
source_files:
  - android/frontend/src/hooks/tauriApi.ts
  - android/frontend/src/network/useDnsSetup.ts
  - android/frontend/src/auth/DashboardPanel.tsx
  - android/frontend/src/auth/useAuth.ts
  - android/frontend/src/account/SelfOnlineStack.tsx
  - android/frontend/src/account/selfServiceState.ts
  - android/frontend/src/settings/SettingsPanel.tsx
  - android/frontend/src/settings/useOnboardingFlow.ts
  - android/plugins/keystore/android/src/main/java/com/campuslogin/plugin/keystore/KeystorePlugin.kt
  - android/plugins/keystore/src/lib.rs
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt
  - android/plugins/foreground-service/src/lib.rs
  - android/plugins/network-bind/android/src/main/java/com/campuslogin/plugin/networkbind/NetworkBindPlugin.kt
  - android/plugins/network-bind/src/lib.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/campus_detect.rs
tags: [Tauri, IPC, Plugin, Android, Architecture]
---

## Overview

`tauri-plugin-architecture` 定义了 Campus Login Assistant Android 应用的核心交互架构。该架构基于 Tauri v2 插件系统，采用“Rust 桥接 + Kotlin 原生实现”的双轨模式，通过 IPC 命令面暴露功能。其核心职责是将复杂的 Android 系统级操作（如 Wi-Fi 绑定、前台保活、生物识别密钥存储）封装为标准化的异步 API，并通过前端的 `tauriApi` 统一入口进行调用，同时处理网络不稳定下的重试逻辑与安全门控。安卓端对桌面专属功能（适配器/DHCP/DNS/窗口/托盘等）通过 `desktopOnly` 显式拒绝（tauriApi.ts:14-15, 205-207, 208-210, 268-269, 287-289, 314, 333-337），对桌面专属事件通过 `noopListener` 返回空清理函数（tauriApi.ts:18, 272-274, 315-318）。密码落盘经 AndroidKeyStore AES-GCM 加密（替代桌面 DPAPI，config_state.rs:220-257, KeystorePlugin.kt:39-89），校园网探测改用 TCP 可达替代 ICMP（campus_detect.rs:1-3, 44-53），网络绑定解决 captive portal 场景（NetworkBindPlugin.kt:24-47）。

## Key Components

### Frontend Hooks & Store (`android/frontend/src/hooks/tauriApi.ts`)

构建安卓端 Tauri IPC 适配层。将 Rust 后端命令映射为前端调用，并处理桌面端专属功能在安卓端的降级。提供带重试机制的 API 包装。

- **核心常量与类型**：
  - `desktopOnly<T>(name: string)` (14-15): 返回 `Promise.reject(new Error('桌面专属功能,安卓端不可用: '+name))` 的辅助函数，用于标记安卓不支持的功能。
  - `noopListener` (18): 返回 `() => {}` 的辅助函数，用于安卓端无对应事件的监听器。
  - 类型 `ConnectionCampusStatus` (20-24)、`BatteryOptimizationInfo` (26-30)、`VendorSettingsResult` (32-36)、`CampusStatusResult` (38-46)、`AvoidBadWifiStatus` (155-164)、`RestoreWrittenSettingsResult` (166-169)。
  - `TauriApi` 接口 (48-153): 定义所有可用的 IPC 方法签名，包含配置、网络、认证、账号、监控、更新、系统设置等模块。
  - `ALI_DNS` / `RECOMMENDED_DNS` 逻辑关联 `useDnsSetup`（但常量实际定义在 useDnsSetup.ts）。

- **关键实现细节**：
  - **事件监听封装 `createEventListener`** (171-200): 解决 Tauri `listen` 异步注册问题，返回清理函数支持在 `listen` Promise 完成前取消订阅（通过 `cancelled` 标志和 `.then` 链式清理，174-196）。DEV 模式打印失败日志但不阻断主流程 (185)。
  - **生物识别验证门 `verifyWindowsIdentity`** (219-256): 优先检查 `shouldUseFaceFallback()` (233)，若开启且系统生物识别不可用则调用应用内人脸 `openFaceDialog('verify')` (237) 上报 `face2d`，否则 `biometricAuthenticate` (251) 上报 `biometric`；失败时归一化为 `{success:false, message}` 而非抛出异常 (223-229, 238-255)；`biometricCheckStatus` 查询异常不阻断主流程落回系统链路 (244-246)。
  - **外部链接打开 `openExternal`** (294-305): 严格校验 URL 协议（仅 http/https，295）、长度（≤2048，296）及格式，调用 `@tauri-apps/plugin-opener` 的 `openerOpenUrl` 返回 boolean。
  - **重试机制 `withRetry` & `tauriApiWithRetry`** (344-379): `isRetryableError` 识别包含 'timeout', 'network', 'fetch', 'connection' 的错误 (344-354)；`withRetry` 指数退避重试（基础 500ms，最大 2 次，356-372）；**差异化重试策略**：仅 `saveConfig` 启用重试 (379)，`checkPortalStatus`/`checkNetworkQuality` 因高频调用由业务层自行兜底。
  - **对象 `tauriApi: TauriApi`** (202-342): 封装所有命令。`getConfig`(203)/`saveConfig`(204)；`exportConfig`/`importConfig`/`exportDiagnostics` 走 `desktopOnly`(205-207)；`getAdapters`/`enableAdapter`/`getAdapterDetails` 走 `desktopOnly`(208-210)；`bindToWifi`(211)/`checkPortalStatus`(212)/`checkCampusStatus`(213)/`requestWifiSsidPermission`(214)；`doLogin`(215)/`doLogout`(216)/`bindOperator`(217)/`getBindStatus`(218)；`verifyWindowsIdentity`(219-256)；`revealOperatorCredential`(257)/`querySelfDashboard`(258)/`selfOfflineSession`(259)/`querySelfOnlineLog`(260)/`changeSelfPassword`(261)；VPN 凭据相关 `vpnSaveCredential` 等 (262-267)；`minimizeWindow`/`closeWindow` 走 `desktopOnly`(268-269)；事件监听 `onBackgroundCheckResult`(270) 等用 `createEventListener`，`onAdaptersChanged`(272) 等用 `noopListener`；`listAccounts`(276) 等账号命令；`dhcpRenewAll` 等走 `desktopOnly`(287-289)；`checkNetworkQuality`(290)/`startLatencyTest`(292)；`openExternal`(294-305)；`getAutoLaunch`→`get_boot_autostart`(306)，`setAutoLaunch`→包装 `{success:true}`(307)；`getBatteryOptimizationInfo`(310) 等保活相关；`cancelAutoExit` 走 `desktopOnly`(314)；`onAutoExitCountdown` 等用 `noopListener`(315-318)；`getLogs`(321) 等调试；`checkUpdate`(326) 等更新；`checkDnsDohStatus` 等走 `desktopOnly`(333-335)；`getAvoidBadWifiStatus`(340)/`restoreWrittenSettings`(341)；`renderHeartbeat`/`notifyWindowReady` 走 `desktopOnly`(336-337)。

### Network Module (`android/frontend/src/network/useDnsSetup.ts`)

DNS DoH 检测与优化的一键操作 Hook。纯逻辑层（UI 由 NetworkPanel 渲染），无恢复自动获取、无适配器门槛、busy 锁仅 `dohEnabling`（21-22 注释）。

- **关键常量**：
  - `ALI_DNS` (11，Set: '223.5.5.5','223.6.6.6','2400:3200::1','2400:3200:baba::1')；`TENCENT_DNS`(12)；`RECOMMENDED_DNS`(13，两者并集)；类型 `DnsFamily = 'both'|'ipv4'|'ipv6'`(15)。
- **关键状态与方法** (`useDnsSetup()` 22-87)：
  - `ipc = tauriApiWithRetry`(24)；`mountedRef` 处理 StrictMode(25,30-35)。
  - state：`dohEnabling`(26)，`dnsFamily` 默认 `'both'`(28)。
  - `handleCheckDns` (37-59): 调 `ipc.checkDnsDohStatus()`(40) → `setDnsDohStatus`；判定 `hasRecommendedDns`(43) 和 `dohNotEnabled`(44-46)；warning toast(48-50)；catch 置 null(54)。
  - `handleSetupDnsDoh` (61-85): 调 `ipc.setupDnsDoh(dnsFamily)`(64)；成功 toast + 独立 try-catch 刷新状态(69-75)；失败/异常 toast(77,81)；finally 按 mountedRef 复位(83)。

### Auth & Dashboard (`android/frontend/src/auth/DashboardPanel.tsx`)

校园网总览面板组件。支持卡片化布局、自定义卡片注入、拖拽排序、编辑模式；移动端空态合并。

- **核心数据结构**：
  - 类型：`BuiltinCardId`(43), `CardId`(45), `CardDef`(47-51), `ExtraCardDef`(54-61), `DashboardPanelProps`(102-117，含 `accounts`, `activeAccount`, `onDhcpRenew`, `onDhcpReleaseRenew`, `excludeCards`, `mergeSelfServiceEmpty`, `extraCards` 等)。
  - `ALL_CARDS`(63-69), `DEFAULT_LAYOUT`(71), `EXTRA_REMOVED_KEY`='campus-dashboard-extra-removed'(91)。
  - `loadLayout(valid, defaults)`(73-82) 从 `safeStorage` 读 `campus-dashboard-layout` 校验返回；`saveLayout(cards)`(84-86)；`loadRemovedExtras()`(93-100) 读删除黑名单。
  - 接口 `SelfOnlineItem`(406-416)、`SelfLogRow`(596)。
- **关键子组件与 Hooks** (均 memo)：
  - `QuickActionsCard`(119-191)：`useAsyncLock` 包装 `onDhcpRenew`(136,5000ms) 和 `onDhcpReleaseRenew`(140,0ms)；网络差光晕(134,146)。
  - `AccountManageCard`(193-297)：`handleSwitchAccount`(210-217) 用 `switchTimerRef`(215) 和 `mountedRef`(203)；渲染 `OnlineDeviceStack`(235)。
  - `NetworkQualityCard`(299-339)：`resolveQualityDisplay`(303) + `QUALITY_CONFIG`(305)；`LatencyPair`(332)。
  - `useSelfCardReveal()`(344-356)：调 `useHelloGate()`(347)；`hasCred = config.user && passwordSaved`(348)。
  - `useSelfCardFetch<T>(fetcher, account)`(360-401)：`fetchLockRef`(367) 防重入，`fetchedForRef`(368) 按 account 查一次；错误 toast(386)。
  - `SelfOnlineCard`(418-592)：`fetchOnline` 调 `querySelfDashboard`(448) 返回 `onlineList: SelfOnlineItem[]`；60s 轮询 `refetch`(457-461)；`handleOffline` 调 `selfOfflineSession`(468)；`handleSaveAlias` 调 `saveConfigDirect`(439) 失败 throw(442)；掩码渲染(509-511)。`ConfirmDialog`(578), `DeviceAliasDialog`(583)。
  - `SelfLogCard`(599-677)：`SELF_LOG_LIMIT=5`(597)；`fetchLog` 调 `querySelfOnlineLog`(606) 取今日日志截断(610)。
  - `SelfServiceMergedEmptyCard`(681-701)：合并空态。
  - `renderCard(id, props, ...)`(703-726)：按 id 分发。
  - `EditableCardItem`(737-831)：编辑拖拽。`LONG_PRESS_MS=350`(735)；长按激活(770-777)；`handlePointerMove` 越过半高阈值 `onSwap`(794-798)；`navigator.vibrate`(773,798)；卡内 `pointer-events-none`(822)。
- **主组件 `DashboardPanel`** (memo, 833-1005)：
  - `allDefs`(836) 合并 extra；`cards` 初始化含迁移补 start 位 extra 卡(839-856) 且避黑名单(850)；`editing`(857)。
  - 订阅 `bgStatus`(858), `networkQuality`(859), `config`(863), `passwordSaved`(864)。
  - `mergeSelfEmpty`(866) = `props.mergeSelfServiceEmpty && !hasCred`。
  - `handleAddCard`(870) 清黑名单(874)；`handleRemoveCard`(878) 记黑名单(882)。
  - `availableCards`(887) 排除 `excludeCards` 和 `enableNetworkQuality===false`(891-892)；`visibleCards`(897) 类似过滤。
  - `handleSwapCard`(907) 仅可见卡交换，隐藏卡留尾。

### Auth Hook (`android/frontend/src/auth/useAuth.ts`)

认证相关的 UI 辅助 Hook，聚合 `useAuthStore` 状态/动作，提供打开 Portal / 自助服务页回调。

- **关键方法** `useAuth()` (7-34)：
  - `authStore` 浅比较选 `isLoggingIn`, `isLoggingOut`, `status`, `doLogin`, `doLogout`, `checkOnline`(8-15)。
  - `configPortalUrl` 来自 `useConfigStore`(18)。
  - `handleOpenPortal`(20-23): 默认 `http://10.1.99.100`(21)，调 `api.openExternal`。
  - `handleOpenSelfService`(25-27): 打开 `http://10.1.80.200:8080/Self/login/?302=LI`。

### Account Stack (`android/frontend/src/account/SelfOnlineStack.tsx`)

提供设备在线状态的可视化堆叠组件，分为手机端竖排（`OnlineDeviceStack`）和平板横排（`OnlineDeviceStackH`），以及单张卡片内容渲染（`OnlineDeviceStackCard`）。核心交互为“整副牌连续滚动”，所有卡片常挂载，通过 `transform` 切换位置。

- **导出与接口**：
  - `OnlineDeviceStackProps`(43-48)：含 `cards: React.ReactNode[]`、`variant?: 'coverflow' | 'narrow'`、`focusIndex?: number`。
  - `OnlineDeviceStack`(50-230)：竖排堆叠主组件。
  - `OnlineDeviceStackCardProps`(233-248) 与 `OnlineDeviceStackCard`(250-336)：含 `loginTime/ip/mac/flowMb/durationMin/terminalType/alias/onRename/offlineDisabled/offlineLoading/onOffline`。
  - `OnlineDeviceStackH`(342-519)：横排堆叠。
- **常量与默认值**：
  - 竖排：`PEEK_TOP=44`(14)、`PEEK_BOTTOM=44`(15)、`DRAG_THRESHOLD=60`(16)、`DRAG_MIN=-110`/`DRAG_MAX=70`(18-19)、`SLIDE_MS=500`(20)、`EASE='cubic-bezier(0.22, 1, 0.36, 1)'`(21)、`V_SIDE_SCALE=0.75`(23)、`V_CARD_INSET='inset-x-[6%]'`(27)。
  - 横排：`CF_X=[60,85]`(32)、`CF_INSET='27%'`(33)、`SIDE_SCALE=0.6`/`SIDE_SCALE_2=0.45`(34-35)、`DRAG_CLAMP=80`(36)、`NARROW_MIN_W=213`(41)。
- **关键实现**：所有槽位常挂载靠 `transform`/`opacity` 过渡(95-97, 378)；`rel`/`sd` 算相对位置(94, 377)，`go` 取模换页(98-102, 379-383)；用 `ResizeObserver` 量最大自然高度防跳变(74-89, 359-374)，jsdom 无 RO 退化一次性量高；单卡 `count<2` 禁拖禁点(105, 140, 386, 421)；横排 `narrow` 用 `left:50%`+`minWidth:213` 居中(469-471)，`translateX` 合并 `-50%` 必须 `calc()` 包裹(473-485)；卡面 `p-3` 同构桌面端(267)。

### Self Service State (`android/frontend/src/account/selfServiceState.ts`)

管理生物识别验证门（Hello Gate）和自助服务会话门（Session Gate），控制敏感操作的权限校验，与后端 TTL 对齐防死锁。

- **`helloEnabled()`** (10)：`config.selfHelloEnabled !== false` 视为开启。
- **`BIOMETRIC_ERROR_KEY`** (19-32)：错误码到 i18n key 映射（含 faceService 业务码 faceTimeout 等）。
- **`biometricFailMessage(err, t)`** (34-38)：按 `err.code` 翻译，未知回退 `extractErrorMessage`。
- **`VERIFY_TTL_MS=570_000`** (54，对齐后端 `IDENTITY_VERIFY_TTL_SECS=600s` 留 30s 余量)；**`gateFresh`** (58-62) 查 TTL 且防时钟回拨（`elapsed >= 0`）；模块级 `helloGateVerifiedAt`(63)。
- **`useHelloGate(options?)`** (65-87)：绑定/明文门，`ignoreToggle:true` 无视总开关强验(71)。
- **`selfSessionVerifiedAt`**(97)；**`resetSelfSessionGate()`**(100-102) 面板卸载重置；**`useSelfServiceVerify()`**(105-127) 受 `selfReverifyEachAction` 控制每次验证。

### Settings Panel (`android/frontend/src/settings/SettingsPanel.tsx`)

设置面板 UI，包含启动、自动化、安全、网络质量、保活等模块。安卓端差异化渲染（如保活设置、无退出应用选项）。

- **导出**：`SettingsPanelProps`(31-35，含 `onUpdateConfig`/`onSetAutoLaunch`/`onShowOnboarding`)；`SettingsPanel`(memo, 37-452)。
- **常量**：`isAndroid = import.meta.env.VITE_PLATFORM === 'android'`(29)。
- **关键实现**：自订阅 `config` 用 `useShallow`(45) 防级联重渲染；`handleSecurityDisable`(56-70) 关安全开关前强弹 Hello，失败受控回弹；复用 `biometricFailMessage`(15, 68)；2D 人脸开启弹 `ConfirmDialog` 风险确认(321-330)，关闭调 `clearTemplate()` 数据最小化(289)；关闭质量检测联动停 `stopLatencyTest` 并若 `defaultPanel==='quality'` 改空切 dashboard 防空白(357-375)；安卓专属 `KeepAliveSettingsCard`(26, 217-223)；两列布局 md 起 grid(109)。

### Onboarding Flow (`android/frontend/src/settings/useOnboardingFlow.ts`)

新手指引 4 步状态机（欢迎→账号→绑定→完成），抽 hook 供手机/平板复用，修复完成步不重校验历史缺陷。

- **导出常量**：`ONBOARDING_STEP_COUNT=4`(26)、`BIND_OPERATOR_NONE='__none__'`(28)、`DEFAULT_OPERATOR='__default__'`(30)、`BIND_SUCCESS_ADVANCE_MS=1200`/`LOGIN_SUCCESS_ADVANCE_MS=1500`(32-34)、`OPERATOR_KEY_BY_VALUE`(47-51)、`OPERATOR_STATUS_ORDER`(53)。
- **类型**：`BindState`(36)、`OperatorKey`/`OperatorBindingInfo`/`BindStatuses`(39-44)、`OnboardingFlowOptions`(55-67，含 `onAutoAdvance` 宿主动画接管)、`OnboardingFlow`(303)。
- **关键实现**：无适配器步骤（安卓后端丢弃 adapter 参数，8-9）；凭据复用账号步草稿，密码掩码 `PASSWORD_MASK` 留空回退已存(78, 186, 248, 279)；`useHelloGate` 用于绑定步(101, 148)；`handleBind`(145-180) 调 `bindOperator`，成功 `setBindRefreshTick` 触发重查(161)；`getBindStatus` 在 step===2 且账号合法自动查(188-214) 无 Hello 门；`handleLoginAndFinish`(246-276) 完成步重校验账号密码必填。

### Keystore Plugin (`android/plugins/keystore/...`)

Android KeyStore AES-GCM 加解密插件，用于密码等敏感数据落盘加密（同构桌面 DPAPI）。

- **Kotlin (`KeystorePlugin.kt`)**：
  - `EncryptArgs`(20-23)、`DecryptArgs`(25-28)、`KeystorePlugin`(30-89)。
  - 常量 `KEY_ALIAS="campus_login_master"`(34)、`IV_LEN=12`(35)、`TAG="CampusKeystore"`(36)。
  - `getOrCreateKey()`(39-54) 创建/复用 AES-256 GCM 无填充密钥；`@Command encrypt`(56-71) 输出 `base64(iv+ct)`；`@Command decrypt`(73-89) 要求 `raw.size > IV_LEN`(78)，用 `GCMParameterSpec(128, raw, 0, IV_LEN)` 解密。错误 catch Exception 打 Log 并 `invoke.reject` 中文(67-70, 85-88)。
- **Rust (`lib.rs`)**：
  - `#![cfg(mobile)]`，`PLUGIN_IDENTIFIER="com.campuslogin.plugin.keystore"`(12)。
  - `EncryptResult`/`DecryptResult`(14-22)、`CampusKeystore<R>`(24) 含 `encrypt`(30-33)/`decrypt`(36-39)、`CampusKeystoreExt` trait(42-50)、`init`(52-60) 注册 `KeystorePlugin`(56) 并 `app.manage` 状态(57)；`run_mobile_plugin` 传 `text`/`data` 字段。

### Monitor Service Plugin (`android/plugins/foreground-service/...`)

前台服务保活（常驻通知 + 锁）、APK 安装、探针窗口锁、电源状态、电池优化与厂商自启页跳转、通知设置页跳转、开机自启开关等；同文件顶层类 `BootReceiver` 处理开机广播。

- **Kotlin (`MonitorServicePlugin.kt`)**：
  - `TAG = "CampusMonitorService"`(27)；`@InvokeArg` 可空安全数据类 `StartArgs`(31-34)、`TextArgs`(36-39)、`BoolArgs`(41-44)；`MonitorServicePlugin(activity)`(47)。
  - `startMonitor`(50-76)：title 空则 `reject`(53-57)；`startForegroundService`/`startService`(63-67)；catch `IllegalStateException` 后 reject(68-72)；成功返 `{started:true}`。
  - `stopMonitor`(79-86)：发 `ACTION_STOP` 的 Intent 让服务自杀。
  - `updateNotification`(89-104)：`NotificationManager` 调 `ForegroundService.buildNotification` 以 `NOTIFICATION_ID` notify(97-102)。
  - `installApk`(110-137)：拼 `activity.filesDir/update/` 防路径逃逸(119-120)；`FileProvider.getUriForFile` authority=`${packageName}.fileprovider` 起安装器(125-132)。
  - `beginProbeWindow`/`endProbeWindow`(143-152) 直调 `ForegroundService` 静态方法。
  - `getPowerState`(161-167)：返 `{screenOn, wifiConnected}`；私有 `isWifiConnected`(170-178)。
  - `vendorTargets`(189-230)；`buildVendorIntent`(234-238)；`startSafely`(240-245)。
  - `getBatteryOptimizationInfo`(252-261)；`requestIgnoreBatteryOptimizations`(270-277)；`openVendorBatterySettings`(285-338) 降级链；`openNotificationSettings`(349-379) 降级链。
  - `setBootAutostart`(382-398) 启停 `BootReceiver` 组件 + SharedPreferences；`isBootAutostartEnabled`(401-408)。
  - `BootReceiver : BroadcastReceiver()`(416-442)：`onReceive`(417-441) 非 `ACTION_BOOT_COMPLETED` 或 prefs 假则 return；try/catch 启 FGS(425-433) 拉 `MainActivity`(434-440)。
- **Rust (`lib.rs`)**：
  - `PLUGIN_IDENTIFIER="com.campuslogin.plugin.monitorservice"`(12)。
  - `#[derive(Deserialize)] struct BoolResult`(15-17)、`PowerStateResult`(20-25)。
  - `CampusMonitorService<R>(PluginHandle<R>)`(27)；`start_monitor`(33-35)、`stop_monitor`(37-39)、`update_notification`(42-44)、`begin_probe_window`(47-49)、`end_probe_window`(52-54)、`get_power_state`(58-61)、`set_boot_autostart`(64-66)、`is_boot_autostart_enabled`(68-71)、`install_apk`(75-77)、`get_battery_optimization_info`(80-82)、`request_ignore_battery_optimizations`(85-87)、`open_vendor_battery_settings`(90-92)、`open_notification_settings`(95-97)。
  - `CampusMonitorServiceExt`(100-108)；`init`(110-119) `Builder::new("campus-monitor-service")` 注册 `MonitorServicePlugin`(114) 并 `app.manage`(117)。

### Network Bind Plugin (`android/plugins/network-bind/...`)

Android 原生网络绑定插件，核心解决校园网 Captive Portal 下的进程级流量劫持问题。提供 WiFi 绑定、SSID 读取、系统设置读写及 WiFi 状态监听。

- **Kotlin (`NetworkBindPlugin.kt`)**：
  - `@TauriPlugin(permissions=[Permission(strings=[Manifest.permission.NEARBY_WIFI_DEVICES], alias="wifiSsid")])`(48-55)；`NetworkBindPlugin(activity)`(56)；`heldCallback`(59)、`lastBoundNetwork`(63)、`cm` getter(65-66)、`watcherCallback`(488)、`lastValidated`(491)、`guardPrefs`(309-311)、`KEY_AVOID_BAD_WIFI`(314)、`KEY_CAPTIVE_PORTAL_MODE`(315)、`PREF_PREFIX`(316)、`REQUEST_TIMEOUT_MS=3000`(659)、`WIFI_SSID_PERMISSION_ALIAS="wifiSsid"`(662)。
  - `bindToWifi`(69-165)：`AtomicBoolean done` 单次 resolve 保护(71-81)；直绑成功返 `allNetworks`/`already_bound`(98-103)；无 WiFi 返 `no_wifi_network`(109-112)；回退 `requestNetwork`(125-128)。
  - `unbind`(168-175)；`acceptWifiNetwork`(192-238)：`canReachSetAcceptUnvalidated`(241-251) 无副作用；`trySetAcceptUnvalidated`(254-268)；`applyNetworkSettingsCompat`(284-298) 写 Global；`snapshotGlobalKey`(322-331)；`hasWriteSecureSettings`(334-336)；`hasPendingRestore`(339-341)；`restoreGlobalKey`(347-358)；`ensureAvoidBadWifi`(366-402)；`restoreAvoidBadWifi`(409-421)；`restoreWrittenSettings`(429-446)；`getSecureSettingsStatus`(454-468)；`describe`(472-476)；`hasVpn`(479-485)；`emitWifiEvent`(494-498)；`startWifiWatcher`(506-541) VALIDATED 翻转发 `wifiChanged`；`stopWifiWatcher`(544-554)；`getWifiSsid`(569-577) API33+ 查授权；`reportWifiUnusable`(586-618)；`requestWifiSsidPermission`(627-636)；`wifiSsidPermissionCallback`(639-642)；`readSsid`(646-655) 去引号归一空串。
- **Rust (`lib.rs`)**：
  - `PLUGIN_IDENTIFIER="com.campuslogin.plugin.networkbind"`(11)；`CampusNetworkBind<R>(PluginHandle<R>)`(13)。
  - `bind_to_wifi`(19-21)、`accept_wifi_network`(26-28)、`start_wifi_watcher`(32-43) 先 `registerListener` 建 Channel、`stop_wifi_watcher`(47-54) 先 `removeListener`、`get_wifi_ssid`(59-61)、`request_wifi_ssid_permission`(65-67)、`report_wifi_unusable`(73-75)、`ensure_avoid_bad_wifi`(82-84)、`restore_avoid_bad_wifi`(89-91)、`restore_written_settings`(96-98)、`get_secure_settings_status`(104-106)；注释强调 JNI 阻塞调用须 `spawn_blocking`。
  - `CampusNetworkBindExt`(110-118)；`init`(121-130) `Builder::new("campus-network-bind")` 注册 `NetworkBindPlugin`(125) 并 `app.manage`(127)。

### Config State (`android/src-tauri/src/config_state.rs`)

安卓端配置持久化、加密存储、版本迁移及 IO 锁管理。结构对齐桌面端 `Config` 可适用子集（camelCase IPC 契约）。

- **常量与锁**：
  - `PASSWORD_MASK="***"`(10)、`CONFIG_FILE="config.json"`(11)、`CONFIG_IO_LOCK: tokio::sync::Mutex<()>`(17-19)、`config_io_lock()`(22-24)。
- **`struct Settings`** (26-140, `#[serde(rename_all = "camelCase", default)]`) 字段：
  - `user`(30), `password`(31), `self_hello_enabled`(32), `self_reverify_each_action`(33), `allow_2d_face_verify`(36), `operator`(37), `enable_night_operator_switch`(41), `night_operator_restore`(43), `enable_night_outbound_switch`(47), `night_restore_minutes`(51), `outbound_priority`(54), `outbound_metric_restore`(57), `outbound_disabled_adapters`(60), `outbound_standby_route`(63), `night_outbound_restore`(68), `dns_optimize_adapters`(71), `device_aliases`(76), `auto_login_on_start`(78), `enable_background_check`(79), `background_check_interval`(80), `background_check_idle_interval`(85), `auto_login_on_preparation`(86), `max_disconnect_reconnect`(87), `auto_login_cooldown_secs`(88), `theme_mode`(90), `enable_notification`(91), `custom_theme_color`(92), `default_panel`(93), `display_name`(97), `active_account`(98), `enable_boot_autostart`(99), `enable_latency_test`(101), `latency_test_interval`(102), `enable_network_quality`(103), `skip_ttfb_in_latency`(104), `skip_content_in_latency`(105), `portal_url`(107), `fixed_gateway`(108), `required_network_name`(109), `enable_network_name_check`(110), `campus_gateway`(111), `campus_check_start_minutes`(113), `campus_check_end_minutes`(115), `scheduled_login_minutes`(117), `scheduled_logout_minutes`(119), `update_source`(122), `log_retention_days`(124), `vpn_account`(128), `vpn_password_enc`(129), `self_service_via_vpn`(132), `config_schema_version`(139)。
- **`impl Default for Settings`** (142-217)：`self_hello_enabled=true`(147)、`enable_night_operator_switch=false`(153)、`background_check_interval=60_000`(169)、`background_check_idle_interval=300_000`(171)、`max_disconnect_reconnect=3`(173)、`portal_url="http://10.1.99.100"`(191)、`fixed_gateway`/`campus_gateway="10.2.127.254"`(192-197)、`required_network_name="i-wxxy、iwxxy-2、iwxxy-3"`(195)、`campus_check_start_minutes=460`(199)、`campus_check_end_minutes=1380`(202)、`scheduled_login_minutes`/`scheduled_logout_minutes=1440`(206-207)、`update_source="mirror"`(208)、`log_retention_days=7`(209)、`config_schema_version=7`(214)、`enable_network_quality=false`(188)、`latency_test_interval=600_000`(184)。
- **`struct CryptoBridge`** (220-257, Clone)：含 `encrypt`/`decrypt` 的 `Arc<dyn Fn>`；`from_app`(227-256) mobile 走 `tauri_plugin_campus_keystore`(230)，非 mobile 返回 Err(252-254)。
- **`struct EncodedSettings`** (260-265, 私有)：`settings: Settings` + `password_cipher: String`(264)。
- **核心 IO 函数**：
  - `load_file`(267-280)：无文件返回默认(270)，json 损坏报错(273)，密文非空才解密失败置空不报错(277-278)。
  - `password_saved_at`(287-293)：查 `EncodedSettings.password_cipher` 非空判据(291)。
  - `save_file`(295-325)：空密码不写密文(303-308)，加密失败返回 Err 绝不落明文(304-307)；带 pid 唯一 tmp 文件写再 rename(317-323)。
  - `cleanup_stale_config_tmp`(330-341)；`load_from`(343-353) 调 `load_file` + `migrate_legacy_defaults` + 旧单值 "i-wxxy" 扩三值(349-351)；`migrate_legacy_defaults`(372-434) v0→v1(374-376), v<3(377-391), v<4(392-400), v<5(401-409), v<6(410-418), v<7(419-433)；`save_to`(436-438)；`masked_for_display`(443-452)；`emit_config_changed`(458-461)；`resolve_password_field`(464-473)。
- **Tauri 命令与缓存**：
  - `#[tauri::command] get_config`(475-488)；`save_config`(490-526) 持 `CONFIG_IO_LOCK`(499)，调 `auto_create_account_for_current`(521)，`emit_config_changed`(524)；`current_settings`(530-547, `#[allow(dead_code)]`)。
  - 测试模块(549-903)：覆盖加密落盘无明文、解密失败置空、SSID 迁移、默认值语义、版本迁移幂等等。

### Campus Detect (`android/src-tauri/src/campus_detect.rs`)

安卓侧校园网探针：本机 IPv4 枚举 + /18 子网判定（复用桌面纯函数）+ Portal TCP 可达 + WiFi SSID 判定。ICMP 原始 socket 被 SELinux 禁止，网关可达改用 Portal TCP 可达表达。

- **常量**：`PORTAL_PORT=80`(9)、`TCP_TIMEOUT=Duration::from_secs(3)`(10)。
- **源 IP 与可达性**：
  - `pick_campus_source_ip`(15-37)：排除蜂窝 `rmnet*`/`ccmni*`(17) 等。
  - `portal_reachable`(44-53) 走 `tcp_connect_bounded`(48)。
  - `struct CampusProbe`(56-63)：`source`, `portal_ok`, `on_campus`, `wifi_off`。
  - `probe_campus`(65-125)：枚举网卡(66-79)，pick source(80)；wifi_off 短路(85-93)；`is_same_subnet_18`(96)；`portal_host_of`(99)；三层判定(109-116)，`on_campus`(117)。
- **文案与解析**：
  - `campus_status_message`(129-137)；`portal_host_of`(140-149)；`ssid_matches`(153-157) 默认名特判同源。
- **SSID 与命令**：
  - `current_wifi_ssid`(163-178) 经 `tauri_plugin_campus_network_bind`；`probe_campus_with_ssid`(185-200)；`cache_source_ip`(203-210)；`#[tauri::command] check_campus_status`(215-238) 调 `ensure_wifi_bound`(221) 返 JSON；`#[tauri::command] request_wifi_ssid_permission`(243-249)。
  - 测试(251-321)。

## Architecture

```text
┌──────────────────────────────────────────────────────────────────────┐
│                        Frontend Layer (React)                        │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ │
│  │ Dashboard    │ │ Settings     │ │ Onboarding   │ │ Self Online  │ │
│  │ Panel        │ │ Panel        │ │ Flow         │ │ Stack        │ │
│  └──────┬───────┘ └──────┬───────┘ └──────┬───────┘ └──────┬───────┘ │
│         ▼               ▼               ▼               ▼          │
│        tauriApi / tauriApiWithRetry (Unified IPC Entry)              │
└───────────────────────────────┬──────────────────────────────────────┘
                                │ Tauri IPC
                                ▼
┌──────────────────────────────────────────────────────────────────────┐
│                      Rust Backend Core + Plugins Wrapper              │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ │
│  │ config_state │ │ campus_detect│ │ Keystore     │ │ Monitor/Net  │ │
│  │ (CryptoBridge│ │ (TCP Probe)  │ │ (AES-GCM)    │ │ Bind Plugins │ │
│  │  IO Lock)    │ │              │ │              │ │              │
│  └──────┬───────┘ └──────┬───────┘ └──────┬───────┘ └──────┬───────┘ │
└───────────────────────────────┼──────────────────────────────────────┘
                                │ JNI
                                ▼
┌──────────────────────────────────────────────────────────────────────┐
│                   Kotlin Native Implementation                        │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐                   │
│  │ Keystore     │ │ Monitor      │ │ Network      │                   │
│  │ Plugin       │ │ Service      │ │ Bind         │                   │
│  └──────────────┘ └──────────────┘ └──────────────┘                   │
└──────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **初始化与桥接建立**：
   - 应用启动时，前端 Store 加载初始配置。
   - Rust 后端注册 Commands，Kotlin 插件通过 JNI 暴露原生方法。
   - 前端通过 `tauriApi` 建立与后端的 IPC 通道，其中 `KeystorePlugin.kt` 与 `lib.rs` 配套：Rust `init` 注册 Kotlin 侧 `KeystorePlugin`，前端经 Tauri bridge 调 `encrypt`/`decrypt`（config_state.rs:227-256）。

2. **UI 触发与指令转发**：
   - 用户在 `DashboardPanel` 或 `SettingsPanel` 执行操作，控制流进入 `useAuth()` 或相关 Hooks，调用 `tauriApi.invoke()`。
   - `save_config` 入参 `config: Settings` + `clear_password: Option<bool>` (config_state.rs:490-495) → 合并后 `save_to` 落盘 `EncodedSettings`（`password_cipher` 密文）(config_state.rs:436-438) → 广播 `config-changed` 事件 payload `{config: masked Value}` (config_state.rs:458-461)。
   - 网络质量/DNS 配置由 `useDnsSetup()` 介入，强制使用 `tauriApiWithRetry` 封装调用（useDnsSetup.ts:24）。

3. **原生执行与安全门控**：
   - **普通操作**：IPC 消息直达 Rust Wrapper。例如 Wi-Fi 绑定由 `CampusNetworkBind` 接收，转发至 Kotlin `NetworkBindPlugin.kt`；保活服务由 `CampusMonitorService` 处理。
   - **敏感操作**：涉及密码读写时，流程指向 `CampusKeystore`，Rust 层调用 Kotlin `KeystorePlugin.kt` 利用 `AndroidKeyStore` 进行 AES-GCM 加解密，明文不落地（KeystorePlugin.kt:36-86）。
   - **生物识别门控**：在 `selfServiceState.ts` 中，`useHelloGate` 用于绑定/明文门；若未通过验证则拦截 IPC 请求（selfServiceState.ts:65-87）。
   - **校园网探测**：`check_campus_status` (campus_detect.rs:187-210) 先调用 `ensure_wifi_bound` 绑 WiFi，取 `config_state::current_settings` 获取 `Settings` 以读 `campus_gateway`/`portal_url` 等，调 `probe_campus_with_ssid` 后 cache_source_ip，返回 `serde_json::Value` 含探测结果与 SSID。

4. **事件反向流与状态同步**：
   - `NetworkBindPlugin` 的 WiFi 变化经 `trigger("wifiChanged")` 发事件，Rust 侧 `start_wifi_watcher` 先 `registerListener` 建 Channel 接收 `{"event":...}` 负载，形成反向事件流；`stop_wifi_watcher` 经 `removeListener`+`stopWifiWatcher` 注销（network-bind/src/lib.rs:32-54）。
   - 原生层执行完成后，通过 Tauri Event Listener 或 Command Response 将结果返回前端 Store（如 `useAuthStore`, `useQualityStore`）并更新 UI。

## Connections

- [[concepts/tauri-plugin-architecture]]

## Known Issues

- **配置与加密边界** (config_state.rs)：无配置文件不报错返默认(270)；密文损坏/密钥变更密码置空而非配置不可用(277-278)；加密失败返回 Err 且绝不落明文(304-307)；`password_saved_at` 必须查密文位，外露 `settings.password` 落盘恒空(287-293)；迁移幂等：仅旧默认值被刷，用户显式改过的值保留(372-433)；新装 `config_schema_version=7` 不触发迁移(214)；非 mobile 构建 `CryptoBridge` 加解密均返 Err(252-254)，保证 host 测试需注入假桥；`enable_night_operator_switch` 默认 false(153) 但 v7 迁移刷存量 true(419-422)。
- **校园网探测边界** (campus_detect.rs)：ICMP 被 SELinux 禁，以 Portal TCP 可达替代 ping(1-3)；蜂窝接口绝不入选 source IP(17)；WiFi 未连接（无非蜂窝 IPv4）短路返回 "WiFi 未连接" 不再空转 2×3s 探测(85-93)；`portal_host_of` 解析失败回退原串(140-149)；`current_wifi_ssid` 未授权/API<33/未连 WiFi 返 None(163-178)。
- **前台服务与权限边界** (MonitorServicePlugin.kt)：`@InvokeArg` 字段全可空，命令内显式判空 `reject` 避免炸插件线程(31-44)；catch `IllegalStateException` 兼容低版本(68)；`stopMonitor` 用 `startService(ACTION_STOP)` 让服务自身退出避免竞态(82)；`installApk` 仅取文件名拼 `filesDir/update/` 拒绝目录穿越(120)；厂商页跳转故意不用 `resolveActivity` 预探测，直接 `startActivity` catch `ActivityNotFoundException`(185-187)；`BootReceiver` 开机瞬间启 FGS 失败只记日志不炸广播接收器(431)。
- **网络绑定陷阱** (NetworkBindPlugin.kt)：`bindToWifi` 不能仅用 `NET_CAPABILITY_INTERNET` 筛 WiFi（captive portal 会被漏掉，27-33）；`requestNetwork` 不能带 INTERNET 能力否则被 WifiNetworkFactory 拒绝(34-37)；`AtomicBoolean done` 防 `onAvailable`/`onUnavailable` 多次触发重复 resolve(71-81)；`lastBoundNetwork` 同网络重绑返 `already_bound` 使 Rust 侧跳过清空连接池(62-63)；`applyNetworkSettingsCompat` 直接试写 Global 不再预检 `canWrite`（此前误判，273-298）；普通签名无 `WRITE_SECURE_SETTINGS` 恒失败(368)；`startWifiWatcher` 的 `onCapabilitiesChanged` 只在 `VALIDATED` 翻转时上报(524-531)；SSID 读取 API33+ 用 `NEARBY_WIFI_DEVICES`（免定位），低版本因不引入定位权限恒 `granted=false`(571-573)；`getWifiSsid` 绝不弹框(560-561)，权限请求须前端显式触发(627-636)。
- **前端状态与交互边界**：堆叠卡 `count===0` 返 null(91, 376)，`count===2` 无底露卡 `peekBottom=0`(93)；拖拽冲突容器 `touch-action:none` 防 WebView 滚动冲突(163, 440)；`gateFresh` 拒时钟回拨防前端永真后端拒死锁(selfServiceState.ts:58-62)；引导密码纪律掩码态留空后端回退已存，仅新密码非空才写 `password`(useOnboardingFlow.ts:224-227)；`mergeSelfEmpty` 仅无凭据且移动端时合并 selfOnline/selfLog 为空态卡(DashboardPanel.tsx:866,969-976)；桌面专属功能在安卓端经 `desktopOnly` 拒绝(tauriApi.ts:14-15, 205 等)，事件经 `noopListener` 空订阅(tauriApi.ts:18, 272-274)。
