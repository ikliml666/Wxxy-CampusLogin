---
title: 安卓前端 - 面板层（总览 / 账号 / 自助 / 监控 / 质量 / 设置 / 日志）
type: module
source_files:
  - android/frontend/src/auth/AboutDialog.tsx
  - android/frontend/src/auth/AboutDialogMobile.tsx
  - android/frontend/src/auth/DashboardPanel.tsx
  - android/frontend/src/auth/index.ts
  - android/frontend/src/auth/types.ts
  - android/frontend/src/auth/useAuth.ts
  - android/frontend/src/account/AccountPanel.tsx
  - android/frontend/src/account/SelfServicePanel.tsx
  - android/frontend/src/account/index.ts
  - android/frontend/src/account/selfServiceState.ts
  - android/frontend/src/account/types.ts
  - android/frontend/src/account/useAccount.ts
  - android/frontend/src/monitor/LatencyComponents.tsx
  - android/frontend/src/monitor/LatencyTimeline.tsx
  - android/frontend/src/monitor/MonitorPanel.tsx
  - android/frontend/src/monitor/NetworkQualityCapsule.tsx
  - android/frontend/src/monitor/QualityPanel.tsx
  - android/frontend/src/monitor/SpeedTestPanel.tsx
  - android/frontend/src/monitor/StatusBar.tsx
  - android/frontend/src/monitor/index.ts
  - android/frontend/src/monitor/types.ts
  - android/frontend/src/monitor/useMonitor.ts
  - android/frontend/src/network/NetworkPanel.tsx
  - android/frontend/src/network/adapters.ts
  - android/frontend/src/network/constants.ts
  - android/frontend/src/network/index.ts
  - android/frontend/src/network/types.ts
  - android/frontend/src/network/useNetwork.ts
  - android/frontend/src/settings/KeepAliveSettingsCard.tsx
  - android/frontend/src/settings/OnboardingWizard.tsx
  - android/frontend/src/settings/OnboardingWizardMobile.tsx
  - android/frontend/src/settings/SettingsPanel.tsx
  - android/frontend/src/settings/ThemeDialog.tsx
  - android/frontend/src/settings/constants.ts
  - android/frontend/src/settings/index.ts
  - android/frontend/src/settings/types.ts
  - android/frontend/src/settings/useOnboardingFlow.ts
  - android/frontend/src/settings/useSettings.ts
  - android/frontend/src/shared/LogPanel.tsx
  - android/frontend/src/i18n/locales/zh.json
  - android/frontend/src/i18n/locales/en.json
tags: [安卓, 前端, 面板, 总览, 账号, 自助服务, 网络质量, 设置, 日志, 新手向导]
---

## Overview

本模块覆盖安卓前端的业务面板层：总览看板（`auth/DashboardPanel.tsx`，安卓经 `MobileDashboard` 复用）、账号与运营商绑定（`account/AccountPanel.tsx`）、自助服务（`account/SelfServicePanel.tsx` 与 `account/selfServiceState.ts` 验证门）、后台巡检与网络质量（`monitor/*`）、设置与新手向导（`settings/*`）、运行日志（`shared/LogPanel.tsx`），以及适配器/DNS 面板（`network/*`，安卓无入口但有类型与常量输出）。

与桌面前端的关系：本层文件多为**复刻同构**（同名文件同名组件、逐行近似），但面板装配、可见项与平台门控完全不同——手机壳只渲染 `MobileDashboard`/`AccountPanel`/`SelfServicePanel`/`QualityPanel`/`MonitorPanel`/`More 聚合页`（`App.tsx:110-154`、`components/mobile/MobileMore.tsx:67-85`），平板壳渲染八个 Dock 面板且不含 `network`（`components/tablet/TabletShell.tsx:161-227`）。

## Key Components

### auth/ — 总览与关于

| 导出 | 位置 | 用途 |
| --- | --- | --- |
| `ExtraCardDef` | `auth/DashboardPanel.tsx:51` | 宿主注入卡片的契约：`id` / `label` / `icon` / `defaultPosition?: 'start' \| 'end'` / `render(editing)`；`MobileDashboard` 用它注入状态卡与监控摘要卡 |
| `DashboardPanel` | `auth/DashboardPanel.tsx:829` | 总览看板（`memo`）：五张内置卡 + 宿主 extraCards 的统一编辑体系（增删/长按拖动排序/持久化） |

`auth/DashboardPanel.tsx` 内部私有（读者最常需要的）：`BuiltinCardId`(40)、`CardId`(42，`string & {}` 泛化)、`CardDef`(44)、`ALL_CARDS`(60，five 张内置卡：`quickActions`/`accountManage`/`selfOnline`/`selfLog`/`networkQuality`)、`DEFAULT_LAYOUT`(68)、`loadLayout`(70)、`saveLayout`(81)、`EXTRA_REMOVED_KEY`(88，`campus-dashboard-extra-removed` 黑名单键)、`loadRemovedExtras`(90)、`DashboardPanelProps`(99)、`QuickActionsCard`(115，DHCP 续租/换 IP，含 `createPortal` 适配器菜单 230-286)、`AccountManageCard`(294)、`NetworkQualityCard`(365)、`useSelfCardReveal`(410，Hello 门 + 掩码切换)、`useSelfCardFetch`(426，自动查询 + 锁 + 错误态)、`SelfOnlineItem`(472)、`SelfOnlineCard`(484，60s 轮询 503-507)、`SelfLogRow`(613)、`SELF_LOG_LIMIT=5`(614)、`SelfLogCard`(616)、`renderCard`(696)、`LONG_PRESS_MS=350`(731)、`EditableCardItem`(733，自研长按拖动换序)。

| 导出 | 位置 | 用途 |
| --- | --- | --- |
| `AboutDialog` | `auth/AboutDialog.tsx:99` | 平板/宽屏关于弹窗：320px 侧栏 + 更新仪表盘（releaseNotes 全文渲染）+ 赞助内嵌页；支持 `initialLatestVersion`/`initialReleaseNotes`/`initialUpdateAvailable` 缓存入参 |
| `AboutDialogMobile` | `auth/AboutDialogMobile.tsx:32` | 手机竖版关于弹窗：应用信息 + 更新渠道切换 + 检查更新 + 应用内下载 APK（镜像依次重试）+ 安装；无 APK 资产时外链 Releases |
| `useAuth` | `auth/useAuth.ts:6` | 组合 `useAuthStore` 与 `config.api`，派生 `handleOpenPortal`（读 `config.portalUrl`，兜底 `http://10.1.99.100`，`useAuth.ts:22-25`）与 `handleOpenSelfService`（硬编码 `http://10.1.80.200:8080/Self/login/?302=LI`，`useAuth.ts:27-29`） |

`auth/index.ts` 只导出 `DashboardPanel`(1)、`AboutDialog`(2)、`useAuth`(3)、`export * from './types'`(5)。`AboutDialogMobile` 不在 barrel（`App.tsx:42` 按路径懒加载）。

`auth/types.ts`：`PortalStatusResult`(1)、`CommandResult`(8)、`LoginResult`(14)。

### account/ — 账号信息、运营商绑定与自助服务

| 导出 | 位置 | 用途 |
| --- | --- | --- |
| `AccountPanel` | `account/AccountPanel.tsx:44` | 账号面板（`memo`）：登录信息卡、自动化开关卡、运营商绑定卡（绑定状态查询/查看明文/绑定表单）、账号管理卡（新增/切换/删除） |
| `useAccount` | `account/useAccount.ts:8` | 账号动作集合：`handleAddAccount`(23，返回是否成功)、`handleDeleteAccount`(48)、`handleSwitchAccount`(73) |
| `useSelfCredStore` | `account/selfServiceState.ts:21` | 自助服务凭据共享 store（`account` / `password` + setter），账号面板绑定卡与自助面板共用，仅内存不落盘 |
| `biometricFailMessage` | `account/selfServiceState.ts:54` | 按插件 reject 的 `code` 翻译验证失败文案（错误码表 `BIOMETRIC_ERROR_KEY`，`selfServiceState.ts:39-52`） |
| `useHelloGate` | `account/selfServiceState.ts:85` | 绑定/查看类操作的验证门（TTL 570s，`selfServiceState.ts:74`；`ignoreToggle` 语义见 `selfServiceState.ts:60-73`） |
| `resetSelfSessionGate` | `account/selfServiceState.ts:120` | 重置自助面板会话门（面板卸载时调用） |
| `useSelfServiceVerify` | `account/selfServiceState.ts:125` | 自助面板操作验证门（尊重 `selfReverifyEachAction`） |
| `formatMac` | `account/SelfServicePanel.tsx:59` | 12 位 hex → `AA-BB-...` |
| `formatEpoch` | `account/SelfServicePanel.tsx:64` | epoch ms → `YYYY-MM-DD HH:mm:ss`（非法返回 `-`） |
| `formatUseTimeMinutes` | `account/SelfServicePanel.tsx:83` | 秒 → 分钟（floor，与原站一致） |
| `formatFlowMb` | `account/SelfServicePanel.tsx:89` | (下行+上行) KB → M（3 位小数） |
| `localDateStr` | `account/SelfServicePanel.tsx:105` | 本地 `YYYY-MM-DD`（避免 `toISOString` 的 UTC 偏一天） |
| `SelfServicePanel` | `account/SelfServicePanel.tsx:110` | 自助服务面板：凭据区 + 在线设备表（可踢下线）+ 使用记录表（可查日期范围）+ 汇总数据卡 |

`account/SelfServicePanel.tsx` 内部私有：`SelfOnlineItem`(17)、`SelfHistoryRow`(29，元组类型)、`SelfLogRow`(33)、`SelfLogSummary`(48)、`formatTerminalType`(72)、`toInt`(77)、`fmt2`(96)、`fmtMoney`(99)、`thClass`/`tdClass`(265/266)。

`account/AccountPanel.tsx` 内部私有：`AccountPanelProps`(34)、`isAndroid`(55)、`BIND_OPERATOR_NONE='__none__'`(152)、`OperatorBindingInfo`(174)、`BindStatuses`(175)。

`account/index.ts`：`AccountPanel`(1)、`useAccount`(2)、`export * from './types'`(4)；`SelfServicePanel` 与 `selfServiceState` 不在 barrel。

`account/types.ts`：`SwitchAccountResult`(3)、`DeleteAccountResult`(10)、`SaveAccountResult`(17)。

### monitor/ — 状态栏、巡检、质量与测速

| 导出 | 位置 | 用途 |
| --- | --- | --- |
| `StatusBar` | `monitor/StatusBar.tsx:23` | 平板顶栏状态条：状态胶囊（含离线/恢复动画 class）+ 质量胶囊 + 刷新 + 自助服务/门户外链（`onOpenSelfService` 可选） |
| `MonitorPanel` | `monitor/MonitorPanel.tsx:79` | 后台检测面板：启停/立即检测/间隔输入、适配器在线状态卡列表、验证设置（自动检测、可登录即登录、校园网名校验、检测时间窗） |
| `QualityPanel` | `monitor/QualityPanel.tsx:108` | 网络质量面板：质量指纹卡、定时测试卡、测试明细卡（5 类 tab + 每项时间线） |
| `SpeedTestPanel` | `monitor/SpeedTestPanel.tsx:109` | 测速站点集合（8 个硬编码站点，3 个分类），点击经 `openExternal` 外开 |
| `NetworkQualityCapsule` | `monitor/NetworkQualityCapsule.tsx:53` | 质量胶囊：延迟数字 + 悬停 portal 明细（网关/外网/DNS 三行），busy/unknown 时按延迟推断等级（`NetworkQualityCapsule.tsx:96-98`） |
| `LatencyPair` | `monitor/LatencyComponents.tsx:170` | 网关/外网双列延迟卡（含信号条 `SignalBars`） |
| `LatencyTimeline` | `monitor/LatencyTimeline.tsx:42` | 单目标分段时间线（UDP/DNS/TCP/TLS/TTFB/内容/网络 + 总时长） |
| `useMonitor` | `monitor/useMonitor.ts:7` | 巡检与延迟测试动作：`handleToggleBackgroundCheck`(25)、`handleTriggerCheck`(52)、`handleToggleLatencyTest`(56) |

`monitor/MonitorPanel.tsx` 内部私有：`isAndroid`(23)、`MonitorPanelProps`(25)、`AdapterStatusCard`(31)。

`monitor/QualityPanel.tsx` 内部私有：`QualityPanelProps`(30)、`DETAIL_CATEGORIES`(36-87，5 类明细分组名与后端 detail key 的映射)、`tabContainerVariants`(89)。

`monitor/SpeedTestPanel.tsx` 内部私有：`SpeedTestSite`(18)、`SPEED_TEST_SITES`(28-101)、`SITE_CATEGORY_KEYS`(103)、`SpeedTestPanelProps`(105)。

`monitor/NetworkQualityCapsule.tsx` 内部私有：`NetworkQualityCapsuleProps`(13)、`LatencyRowProps`(17)、`LatencyRow`(24)、`getQualityCapsuleBg`(44)。

`monitor/LatencyComponents.tsx` 内部私有：`getSignalCfg`(11)、`BAR_SPECS`(24)、`SignalGlowDot`(33)、`SignalBars`(75)。

`monitor/LatencyTimeline.tsx` 内部私有：`LatencyTimelineProps`(7)、`TimelineSegment`(19)、`SEGMENT_INFO`(23，键含后端原始 key `内容`/`网络`)、`LEVEL_BAR_COLOR`(33)。

`monitor/index.ts`：`MonitorPanel`(1)、`QualityPanel`(2)、`SpeedTestPanel`(3)、`LatencyPair`(4)、`LatencyTimeline`(5)、`NetworkQualityCapsule`(6)、`StatusBar`(7)、`useMonitor`(8)、`export * from './types'`(10)。

### network/ — 适配器与 DNS（安卓无入口）

| 导出 | 位置 | 用途 |
| --- | --- | --- |
| `NetworkPanel` | `network/NetworkPanel.tsx:47` | 适配器列表 + 适配器设置 + DNS 优化三卡面板；**安卓全仓无渲染点** |
| `useNetwork` | `network/useNetwork.ts:39` | DHCP 动作集合（续租/换 IP/按适配器换 IP）+ `refreshAdapterInfo`(59，转调 `refreshAdapterData`) |
| `normalizeDhcpResults` | `network/useNetwork.ts:15` | 单条/批量结果归一化为数组 |
| `announceDhcpResults` | `network/useNetwork.ts:23` | DHCP 结果 → 成功/跳过/失败三类 toast |
| `AUTO_DETECT_ADAPTER` | `network/adapters.ts:3` | `'自动检测'` 哨兵值 |
| `AdapterScope` | `network/adapters.ts:5` | `{adapter1, adapter2, dualAdapter}` |
| `resolveAdapterNames` | `network/adapters.ts:15` | 与后端 `resolve_adapter_names` 同源规则解析主/副适配器（有线有 IP > 任意有 IP > 第一个） |
| `QUALITY_CONFIG` | `network/constants.ts:1` | 9 档质量展示配置（`excellent/great/good/fair/poor/bad/unknown/disabled/busy`，每档 `label`/`labelKey`/`color`/`bg`/`border`/`borderBg`/`icon`/`hex`/`activeBars`/`glow`） |

`network/NetworkPanel.tsx` 内部私有：`NetworkPanelProps`(31)、`ALI_DNS`(36)、`TENCENT_DNS`(37)、`RECOMMENDED_DNS`(38)、`formatSpeed`(41)。

`network/useNetwork.ts` 内部私有：`DhcpResultItem`(11)、`AddToastFn`(12)。`network/index.ts`：`NetworkPanel`(1)、`useNetwork`(2)、`export * from './types'`(4)、`export * from './constants'`(5)。

### settings/ — 设置、主题与新手向导

| 导出 | 位置 | 用途 |
| --- | --- | --- |
| `SettingsPanel` | `settings/SettingsPanel.tsx:63` | 设置面板：外观（主题网格、自定义取色、浅色模式）、启动设置、通知、保活（安卓专属卡）、安全（生物开关、2D 人脸）、引导入口、质量检测（总开关、跳过项、固定网关） |
| `ThemeDialog` | `settings/ThemeDialog.tsx:27` | 主题弹窗（手机 header 的调色板入口，`App.tsx:255`） |
| `OnboardingWizard` | `settings/OnboardingWizard.tsx:93` | 平板/宽屏向导（Dialog 形态，四步 + 顶部圆点进度） |
| `OnboardingWizardMobile` | `settings/OnboardingWizardMobile.tsx:82` | 手机全屏向导（段式进度轨、底部固定操作区、输入高 48px、不含 autoFocus） |
| `useSettings` | `settings/useSettings.ts:10` | 设置动作：`handleToggleLightMode`(35)、`handleToggleNotification`(43)、`handleSetAutoLaunch`(49)、`handleSetTheme`(58) |
| `KeepAliveSettingsCard` | `settings/KeepAliveSettingsCard.tsx:25` | 安卓专属保活卡：电池优化白名单状态、一键申请（自家确认框 → 系统页）、厂商省电页跳转 |
| `ONBOARDING_STEP_COUNT` | `settings/useOnboardingFlow.ts:25` | 4 步（欢迎 → 绑定运营商 → 账号 → 完成） |
| `BIND_OPERATOR_NONE` | `settings/useOnboardingFlow.ts:27` | `'__none__'`（Radix SelectItem 禁空串） |
| `DEFAULT_OPERATOR` | `settings/useOnboardingFlow.ts:29` | `'__default__'`（账号步骤运营商默认哨兵） |
| `BindState` | `settings/useOnboardingFlow.ts:35` | `'idle' \| 'loading' \| 'success' \| 'error'` |
| `OnboardingFlowOptions` | `settings/useOnboardingFlow.ts:37` | `{open, onUpdateConfig, onLogin, onClose}` |
| `useOnboardingFlow` | `settings/useOnboardingFlow.ts:45` | 向导流程状态机（两套外壳共用的逻辑单点） |
| `OnboardingFlow` | `settings/useOnboardingFlow.ts:233` | `ReturnType<typeof useOnboardingFlow>` |
| `DEFAULT_CONFIG` | `settings/constants.ts:6` | 全部配置默认值（53 行，含 `backgroundCheckInterval:60000`、`backgroundCheckIdleInterval:300000`、`configVersion:4`） |
| `ISP_OPTIONS` | `settings/constants.ts:55` | 运营商 4 项（`__default__`/`@telecom`/`@unicom`/`@cmcc`） |
| `THEME_OPTIONS` | `settings/constants.ts:62` | 主题 7 项（含 `custom`） |
| `VALID_THEMES` | `settings/constants.ts:72` | 主题名白名单（`main.tsx:38`、`useInitialDataLoad.ts:51` 用它校验存储值） |
| `DEFAULT_PANEL_OPTIONS` | `settings/constants.ts:74` | 由 `NAV_ITEMS` 派生（因此**不含 `network`**） |

`settings/SettingsPanel.tsx` 内部私有：`isAndroid`(31)、`SettingsPanelProps`(33)、`PRESET_COLORS`(42-46)、`PRESET_COLOR_NAMES`(48-61)。

`settings/OnboardingWizard.tsx` 内部私有：`OnboardingWizardProps`(33)、`STEP_TITLE_KEYS`(41)、`slideVariants`(43)、`StepIndicator`(49)。

`settings/OnboardingWizardMobile.tsx` 内部私有：`OnboardingWizardMobileProps`(43)、`slideVariants`(51)、`StepTrack`(58)。

`settings/useOnboardingFlow.ts` 内部私有：`BIND_SUCCESS_ADVANCE_MS=1200`(31)、`LOGIN_SUCCESS_ADVANCE_MS=1500`(33)。

`settings/ThemeDialog.tsx` 内部私有：`ThemeDialogProps`(20)。`settings/KeepAliveSettingsCard.tsx` 内部私有：`PROMPTED_KEY='campus-keepalive-prompted'`(23)。

`settings/index.ts`：`SettingsPanel`(1)、`ThemeDialog`(2)、`OnboardingWizard`(3)、`OnboardingWizardMobile`(4)、`useSettings`(5)、`export * from './types'`(7)、`export * from './constants'`(8)。`KeepAliveSettingsCard` 与 `useOnboardingFlow` 不在 barrel（前者按相对路径导入，`SettingsPanel.tsx:28`；后者被两个向导相对导入）。

### shared/LogPanel.tsx

| 导出 | 位置 | 用途 |
| --- | --- | --- |
| `LogPanel` | `shared/LogPanel.tsx:86` | 运行日志面板（`memo`）：行数选择、保留天数、手动刷新、调试模式、搜索、模块/级别筛选、逐条清空动画、5s 轮询（可见性 + IntersectionObserver 双重门控） |

内部私有：`LogPanelProps`(28，只依赖 `api` 子集与 `addToast`，便于复用)、`LogLevel`(40)、`ParsedLogLine`(42)、`DEFAULT_LEVEL_CONFIG`(50)、`LEVEL_CONFIG`(52)、`LOG_LINE_REGEX`(59)、`isAndroidBuild`(63，安卓改两行布局)、`parseLogLine`(65)、`LINE_OPTIONS`(77)、`MAX_DISPLAY_LINES=200`(84)。

## 结构体与字段（前端即 props 与 state 类型）

### `Config`（`settings/types.ts:3-52`，43 字段；口径：接口本体（第 4-51 行）字段总数，含 `configVersion` 标记字段，不含同文件 `AutoLaunchResult` / `InitData`）

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `user` / `password` | `string` | 登录账号 / 密码（密码后端存掩码 `***`，`shared/ui-constants.ts:4`） |
| `selfPassword` | `string` | 自助服务密码（加密落盘，前端只见掩码） |
| `selfHelloEnabled` | `boolean` | 生物/Hello 操作验证总开关（默认 `true`） |
| `selfReverifyEachAction` | `boolean` | 自助面板每次操作都验证（默认 `false`） |
| `allow2dFaceVerify` | `boolean` | 2D 人脸回退开关（默认 `false`） |
| `operator` | `string` | 运营商后缀，`''` 表示默认（无锡学院） |
| `adapter1` / `adapter2` | `string` | 主/副适配器名（安卓无枚举，恒 `自动检测` / 空） |
| `dualAdapter` | `boolean` | 启用备用适配器 |
| `autoLoginOnStart` | `boolean` | 启动自动登录 |
| `autoExitAfterLogin` | `boolean` | 登录成功后自动退出（安卓 UI 隐藏） |
| `minimizeToTray` / `hiddenStart` | `boolean` | 最小化到托盘 / 静默启动（安卓 UI 隐藏） |
| `autoLaunch` | `boolean` | 开机自启（安卓走 `set_boot_autostart`） |
| `enableBackgroundCheck` | `boolean` | 后台巡检自动启动 |
| `backgroundCheckInterval` | `number` | 稳态巡检间隔 ms（默认 60000） |
| `backgroundCheckIdleInterval` | `number` | 闲时巡检间隔 ms（默认 300000） |
| `autoLoginOnPreparation` | `boolean` | 可登录即自动登录 |
| `autoExitOnOnline` | `boolean` | 上线后自动退出（安卓 UI 隐藏） |
| `themeMode` | `'light' \| 'dark' \| 'system'` | 主题模式（默认 `dark`） |
| `enableNotification` | `boolean` | 通知开关 |
| `activeAccount` | `string` | 当前账号名 |
| `enableLatencyTest` / `latencyTestInterval` | `boolean` / `number` | 定时延迟测试开关与间隔（默认 60000） |
| `customThemeColor` | `string` | 自定义主题色（默认 `#6366f1`） |
| `defaultPanel` | `PanelName \| ''` | 默认面板（安卓 UI 隐藏） |
| `enableNetworkQuality` | `boolean` | 质量检测总开关（默认 true；前端一律按 `!== false` 判定） |
| `skipTtfbInLatency` / `skipContentInLatency` | `boolean` | 延迟计算跳过 TTFB / 内容传输 |
| `portalUrl` | `string` | 门户地址（默认 `http://10.1.99.100`） |
| `fixedGateway` | `string` | 固定网关（默认 `10.2.127.254`） |
| `requiredNetworkName` | `string` | 校园网 SSID（默认 `i-wxxy`） |
| `enableNetworkNameCheck` | `boolean` | 校园网名校验 |
| `campusGateway` | `string` | 校园网网关（默认 `10.2.127.254`） |
| `updateSource` | `'mirror' \| 'github'` | 更新渠道优先级（默认 `mirror`） |
| `campusExitOnFail` | `boolean` | 非校园网自动退出（安卓 UI 隐藏） |
| `campusCheckStartMinutes` / `campusCheckEndMinutes` | `number` | 检测时间窗（分钟制；默认 460=07:40 / 0=00:00） |
| `maxDisconnectReconnect` | `number` | 断连重连上限（默认 3） |
| `autoLoginCooldownSecs` | `number` | 自动登录冷却秒（默认 60） |
| `logRetentionDays` | `number` | 日志保留天数（默认 7） |
| `configVersion` | `number` | 配置 schema 版本（默认 4） |

### 其余 settings/ 类型

| 类型 | 位置 | 字段 |
| --- | --- | --- |
| `AutoLaunchResult` | `settings/types.ts:54` | `success`、`message?` |
| `InitData` | `settings/types.ts:59` | `config: Partial<Config>`、`version`、`adapters`、`adapterDetails`、`disabledAdapters`、`accounts`、`activeAccount`、`backgroundStatus`、`isAutoStart`、`autoLaunch`、`notificationEnabled`、`gpuInfo?`、`refreshRate?` |

### auth/ 类型

| 类型 | 位置 | 字段 |
| --- | --- | --- |
| `PortalStatusResult` | `auth/types.ts:1` | `online`、`message?`、`reachable?`、`loginAvailable?` |
| `CommandResult` | `auth/types.ts:8` | `success`、`message?`、`data?: Record<string, unknown>` |
| `LoginResult` | `auth/types.ts:14` | `extends CommandResult`（无新字段） |

### account/ 类型与凭据 store

| 类型 | 位置 | 字段 |
| --- | --- | --- |
| `SwitchAccountResult` | `account/types.ts:3` | `success`、`message?`、`activeAccount?`、`config?: Config` |
| `DeleteAccountResult` | `account/types.ts:10` | `success`、`message?`、`activeAccount?`、`config?: Config` |
| `SaveAccountResult` | `account/types.ts:17` | `success`、`activeAccount?`、`config?`、`message?` |
| `SelfCredState`（私有） | `account/selfServiceState.ts:14` | `account`、`password`、`setAccount`、`setPassword` |

### monitor/ 类型

| 类型 | 位置 | 字段 |
| --- | --- | --- |
| `AdapterOnlineStatus` | `monitor/types.ts:1` | `name`、`ip`、`wireless`、`online`、`message` |
| `BackgroundStatus` | `monitor/types.ts:15` | `isRunning`、`checkCount`、`serverAvailable`、`online`、`message?`、`adapterStatuses?`、`currentSsid`、`onCampusNetwork?`、`enableNetworkNameCheck?`、`requiredNetworkName?`、`campusWifi?`、`campusWired?`、`a1CampusMessage?`、`a2CampusMessage?`、`a1OnCampus?`、`a2OnCampus?`、`loginPreparationMode?`、`interval?`、`enabled?` |
| `BackgroundCheckEventData` | `monitor/types.ts:38` | `BackgroundStatus & { timestamp?, checkCount?, secondaryOnline?, secondaryMessage?, message?, online?, adapter1Name?, adapter2Name?, loginAvailable?, serverAvailable? }` |
| `NetworkQuality` | `monitor/types.ts:69` | `gatewayLatency`、`externalLatency`、`averageExternalLatency`、`gateway`、`quality`（`excellent/great/good/fair/poor/bad/unknown/disabled/busy`）、`timestamp`、`details?`、`metrics?` |
| `AutoLoginEventData` | `monitor/types.ts:80` | `success`、`message`、`skipped?` |
| `ConnectionCampusStatus`（私有） | `monitor/types.ts:9` | `onCampus`、`name: string \| null`、`message` |
| `NetworkQualityDetail`（私有） | `monitor/types.ts:51` | `target`、`latency`、`type` + `dnsLatency?`/`tcpLatency?`/`tlsLatency?`/`udpLatency?`/`networkLatency?`/`ttfbLatency?`/`contentLatency?` |
| `NetworkQualityMetrics`（私有） | `monitor/types.ts:64` | `totalElapsed`、`tests: Record<string, {latency, type, elapsed}>` |

### network/ 类型

| 类型 | 位置 | 字段 |
| --- | --- | --- |
| `AdapterStatus` | `network/types.ts:1` | `'disabled' \| 'disconnected' \| 'enabledNoIp' \| 'connected'` |
| `Adapter` | `network/types.ts:3` | `name`、`ip`、`wireless`、`guid?`、`mac`、`ifIndex`、`status`、`linkSpeed?` |
| `DisabledAdapter` | `network/types.ts:15` | `name`、`status`、`description` |
| `AdapterDetail` | `network/types.ts:21` | `name`、`ip`、`wireless`、`subnetMask`、`gateway`、`dhcpServer`、`mac`、`ifIndex`、`status`、`linkSpeed?` |
| `DnsAdapterInfo` | `network/types.ts:42` | `name`、`dnsSource`、`dnsServers`、`profileDnsServers`、`adapterDnsOverridesProfile` |
| `DnsDohStatus` | `network/types.ts:50` | `adapters`、`dohSupported`、`autoDohEnabled`、`dnsSource?` |
| `DhcpRenewResult` | `network/types.ts:57` | `success`、`results: {name, success}[]` |
| `DhcpReleaseRenewResult` | `network/types.ts:62` | `success`、`results: {name, wireless, ip, regOk, success, skipped, reason}[]` |
| `DnsSetupResult` | `network/types.ts:67` | `success`、`message`、`dnsSuccess?`、`dnsFailed?`、`dohAdded?`、`dohFailed?` |
| `EnableAdapterResult` | `network/types.ts:76` | `success`、`message?` |
| `DnsServerInfo`（私有） | `network/types.ts:35` | `address`、`dohAvailable`、`dohEnabled`、`dohTemplate` |

### 面板组件 props

| 类型 | 位置 | 字段 |
| --- | --- | --- |
| `DashboardPanelProps`（私有） | `auth/DashboardPanel.tsx:99` | `accounts`、`activeAccount`、`onUpdateConfig`、`onSwitchAccount`、`onDhcpRenew`、`onDhcpReleaseRenew`、`onDhcpReleaseRenewAdapter`、`onRefreshQuality?`、`onToggleBackgroundCheck?`、`excludeCards?`、`extraCards?` |
| `AboutDialogProps`（私有） | `auth/AboutDialog.tsx:23` | `open`、`onClose`、`openExternal?`、`onUpdateAvailable?`、`initialLatestVersion?`、`initialReleaseNotes?`、`initialUpdateAvailable?` |
| `AboutDialogMobileProps`（私有） | `auth/AboutDialogMobile.tsx:23` | `open`、`onClose`、`openExternal?`、`onUpdateAvailable?` |
| `AccountPanelProps`（私有） | `account/AccountPanel.tsx:34` | `adapters`、`accounts`、`activeAccount`、`onUpdateConfig`、`onAddAccount`（返回 `Promise<boolean>`）、`onDeleteAccount`、`onSwitchAccount` |
| `MonitorPanelProps`（私有） | `monitor/MonitorPanel.tsx:25` | `onUpdateConfig`、`onToggleBackgroundCheck(enabled, interval)`、`onTriggerCheck()` |
| `QualityPanelProps`（私有） | `monitor/QualityPanel.tsx:30` | `onUpdateConfig`、`onRefreshQuality?`、`onToggleLatencyTest?` |
| `SpeedTestPanelProps`（私有） | `monitor/SpeedTestPanel.tsx:105` | `openExternal(url)` |
| `StatusBarProps`（私有） | `monitor/StatusBar.tsx:18` | `onOpenPortal()`、`onOpenSelfService?()` |
| `SettingsPanelProps`（私有） | `settings/SettingsPanel.tsx:33` | `autoLaunch`、`onUpdateConfig`、`onSetAutoLaunch`、`onToggleLightMode`、`onSetTheme`、`onShowOnboarding?` |
| `ThemeDialogProps`（私有） | `settings/ThemeDialog.tsx:20` | `open`、`onClose`、`onSetTheme(name)`、`onToggleLightMode()` |
| `OnboardingWizardProps`（私有） | `settings/OnboardingWizard.tsx:33` | `open`、`onClose`、`onUpdateConfig`、`onLogin(adapterName?)`、`isLoggingIn` |
| `OnboardingWizardMobileProps`（私有） | `settings/OnboardingWizardMobile.tsx:43` | 同上 |
| `NetworkPanelProps`（私有） | `network/NetworkPanel.tsx:31` | `adapters`、`onUpdateConfig` |
| `LogPanelProps`（私有） | `shared/LogPanel.tsx:28` | `api`（`getLogs`/`clearLogs`/`getDebugMode`/`setDebugMode`/`getLogRetentionDays?`/`setLogRetentionDays?`）、`addToast(message, type, description?)` |

### `useOnboardingFlow` 返回字段（`settings/useOnboardingFlow.ts:212-230`）

| 字段 | 类型/含义 |
| --- | --- |
| `step` | `number`，当前步（0-3） |
| `direction` | `Ref<1 \| -1>`，滑动方向（`useOnboardingFlow.ts:75`，供 `custom={direction.current}`） |
| `canProceedAccount` | `boolean`，账号步骤是否可继续（`useOnboardingFlow.ts:145`） |
| `canBind` | `boolean`，绑定表单完整性（`useOnboardingFlow.ts:108-112`，含手机号 `/^1\d{10}$/`） |
| `passwordSaved` | `boolean`，完成页显示 `••••••••` 还是 `-`（`useOnboardingFlow.ts:210`） |
| `username` / `setUsername` | `string` |
| `password` / `setPassword` / `showPassword` / `setShowPassword` | `string` / `boolean` |
| `operator` / `setOperator` | `string` |
| `selfAccount` / `setSelfAccount` | `string`（自助凭据，仅内存） |
| `selfPassword` / `setSelfPassword` | `string`（同上） |
| `bindOperatorValue` / `setBindOperatorValue` | `string` |
| `phone` / `setPhone` | `string` |
| `smsPassword` / `setSmsPassword` | `string` |
| `bindState` / `bindError` / `handleBind` | `BindState` / `string` / `() => Promise<void>` |
| `language` / `setLanguage` | `string` / `(lang) => void`（读写 `useConfigStore` 的 `language`） |
| `loginSuccess` / `showCloseConfirm` / `setShowCloseConfirm` | `boolean` |
| `advance` / `goNext` / `handleSkip` / `handleLoginAndFinish` | 流程动作（`useOnboardingFlow.ts:103` / `164` / `168` / `177`） |

## Data Flow

### 面板装配（谁渲染哪个面板）

| 外壳 | 装配点 | 面板清单 |
| --- | --- | --- |
| 手机外壳 | `App.tsx:110-154`（switch `deferredTab`） | `dashboard` → `MobileDashboard`(113)；`account` → `AccountPanel`(117-126)；`selfservice` → `SelfServicePanel`(129)；`quality` → `QualityPanel`(134-138，`enableNetworkQuality === false` 时返回 `null`)；`monitor` → `MonitorPanel`(144-148)；`more` → `MobileMore`(152) |
| 手机「更多」页 | `components/mobile/MobileMore.tsx:67-85` | `monitor` → `MonitorPanel`(68)；`speedtest` → `SpeedTestPanel`(74)；`log` → `LogPanel`(75)；`settings` → `SettingsPanel`(77) |
| 平板外壳 | `components/tablet/TabletShell.tsx:161-227`（switch `deferredPanel`） | `dashboard` → `MobileDashboard`(164)；`account` → `AccountPanel`(167-177)；`selfservice` → `SelfServicePanel`(180)；`monitor` → `MonitorPanel`(183-189)；`quality` → `QualityPanel`(192-198)；`speedtest` → `SpeedTestPanel`(201-205)；`settings` → `SettingsPanel`(208-217)；`log` → `LogPanel`(221-224，`case 'log'` 在 219) |

手机端底栏位置是动态的：质量检测开启时第 4 位是「网络质量」（`BottomNav.tsx:30` 插 `QUALITY_TAB`），关闭时插 `MONITOR_TAB`（后台检测）；与之对称，`MobileMore` 的 monitor 子页带 `qualityOnly: true`（`MobileMore.tsx:21`）并按 `qualityEnabled` 过滤（`MobileMore.tsx:36`），关闭质量时「更多」页从 monitor 派生降级为 speedtest（`MobileMore.tsx:39`）。

### 面板 → store → `tauriApi.invoke` → 安卓 Rust 命令（真实调用点）

| 用户动作 | 组件调用点 | store/hook | IPC 实现 | 后端命令 |
| --- | --- | --- | --- | --- |
| 总览：切换账号 | `DashboardPanel.tsx:315` | `useAccount.handleSwitchAccount`（`useAccount.ts:75`） | `tauriApi.ts:226` | `switch_account` |
| 总览：在线设备卡自动查询 | `DashboardPanel.tsx:494` | 直调 `tauriApiWithRetry` | `tauriApi.ts:213` | `query_self_dashboard` |
| 总览：踢设备下线 | `DashboardPanel.tsx:514` | 直调 | `tauriApi.ts:214` | `self_offline_session` |
| 总览：今日上网记录 | `DashboardPanel.tsx:623` | 直调 | `tauriApi.ts:215` | `query_self_online_log` |
| 总览：质量卡刷新 | `DashboardPanel.tsx:389` → `onRefreshQuality` | `useQualityStore.refreshQuality`（`useQualityStore.ts:58`） | `tauriApi.ts:237` | `check_network_quality` |
| 账号：保存新账号 | `AccountPanel.tsx:136` | `useAccount.handleAddAccount`（`useAccount.ts:26`） | `tauriApi.ts:227` | `save_current_as_account` |
| 账号：删除账号 | `AccountPanel.tsx:745` → 外壳确认框（`App.tsx:276`） | `useAccount.handleDeleteAccount`（`useAccount.ts:51`） | `tauriApi.ts:228` | `delete_account` |
| 账号：绑定状态查询 | `AccountPanel.tsx:229`（先过 `useHelloGate`） | 直调 | `tauriApi.ts:179` | `query_bind_status` |
| 账号：查看运营商明文密码 | `AccountPanel.tsx:263`（`ensureRevealVerified`，`ignoreToggle`） | `useHelloGate({ignoreToggle:true})`（`AccountPanel.tsx:254`） | `tauriApi.ts:212` | `reveal_operator_credential` |
| 账号/向导：绑定运营商 | `AccountPanel.tsx:286`；`useOnboardingFlow.ts:120` | `useHelloGate`（`AccountPanel.tsx:222`；`useOnboardingFlow.ts:71`） | `tauriApi.ts:178` | `bind_operator` |
| 账号：清除已保存密码 | `AccountPanel.tsx:104` / `AccountPanel.tsx:114` | `saveConfigDirect({password:''}, true)` / `({selfPassword:''}, undefined, true)` | `tauriApi.ts:168` | `save_config`（`clearPassword` / `clearSelfPassword`） |
| 自助：查询在线信息 | `SelfServicePanel.tsx:204` | `useSelfServiceVerify`（`SelfServicePanel.tsx:141`） | `tauriApi.ts:213` | `query_self_dashboard` |
| 自助：踢下线 | `SelfServicePanel.tsx:243` | 同上 | `tauriApi.ts:214` | `self_offline_session` |
| 自助：使用记录查询 | `SelfServicePanel.tsx:273` | 同上 | `tauriApi.ts:215` | `query_self_online_log` |
| 自助：密码失焦保存 | `SelfServicePanel.tsx:180` | `saveConfigDirect`（`useConfigStore.ts:128`） | `tauriApi.ts:168` | `save_config` |
| 巡检：启停/立即检测 | `MonitorPanel.tsx:165` / `MonitorPanel.tsx:155` | `useMonitor.handleToggleBackgroundCheck`（`useMonitor.ts:29`）/ `handleTriggerCheck`（`useMonitor.ts:53`） | `tauriApi.ts:230` / `tauriApi.ts:232` | `start_background_check` / `trigger_background_check` |
| 巡检：间隔提交 | `MonitorPanel.tsx:105` | `onUpdateConfig` → debounce | `tauriApi.ts:168` | `save_config` |
| 质量：定时测试启停 | `QualityPanel.tsx:282` | `useMonitor.handleToggleLatencyTest`（`useMonitor.ts:60`） | `tauriApi.ts:239` / `tauriApi.ts:240` | `start_latency_test` / `stop_latency_test` |
| 设置：开机自启 | `SettingsPanel.tsx:285` | `useSettings.handleSetAutoLaunch`（`useSettings.ts:52`） | `tauriApi.ts:254` | `set_boot_autostart` |
| 设置：通知开关 | `SettingsPanel.tsx:408` | `updateConfig`；标题栏/平板走 `useSettings.handleToggleNotification`（`useSettings.ts:46`） | `tauriApi.ts:256` | `set_notification_enabled` |
| 设置：关闭质量检测（联动） | `SettingsPanel.tsx:586-604` | 直接 `useConfigStore.getState().api.stopLatencyTest()`（`SettingsPanel.tsx:599`） | `tauriApi.ts:240` | `stop_latency_test` |
| 设置：2D 人脸录入 | `SettingsPanel.tsx:504` | `useFaceDialogStore.openFaceDialog('enroll')` | `faceService.ts:155`（本地，无 IPC） | —— |
| 保活：申请电池白名单 | `KeepAliveSettingsCard.tsx:52` | 直调 | `tauriApi.ts:258` | `request_ignore_battery_optimizations` |
| 保活：厂商省电页 | `KeepAliveSettingsCard.tsx:65` | 直调 | `tauriApi.ts:259` | `open_vendor_battery_settings` |
| 保活：状态读取 | `KeepAliveSettingsCard.tsx:34` | 直调 | `tauriApi.ts:257` | `get_battery_optimization_info` |
| 日志：拉取/清空/调试/保留 | `LogPanel.tsx:125` / `329` / `184` / `200`（另有 `158` 读调试模式、`162` 读保留天数） | 直调 `api`：外壳注入 `useConfigStore.getState().api`（手机经 `components/mobile/MobileMore.tsx:42`，平板经 `components/tablet/TabletShell.tsx:86`） | `tauriApi.ts:267` / `268` / `269` / `270` / `283` / `284` | `get_logs` / `clear_logs` / `get_debug_mode` / `set_debug_mode` / `get_log_retention_days` / `set_log_retention_days` |
| 关于：检查更新 / 下载 / 安装 | `AboutDialogMobile.tsx:54` / `87` / `106`；`AboutDialog.tsx:126` / `189` / `220` | 直调 | `tauriApi.ts:273` / `274` / `275` | `check_update` / `download_update` / `install_update` |
| 关于：镜像源列表 | `AboutDialogMobile.tsx:78`；`AboutDialog.tsx:250`/`476` | 直调 | `tauriApi.ts:276` | `get_mirror_urls` |
| 向导：绑定 + 登录 | `useOnboardingFlow.ts:120` / `193` | `onLogin` 由外壳传入 `useAuthStore.doLogin`（`App.tsx:286`、`TabletShell.tsx:373`） | `tauriApi.ts:178` / `176` | `bind_operator` / `do_login` |

### 事件进入面板的路径

- `background-check-result` → `useEventListeners.ts:77` 写 `useAuthStore.bgStatus` → `MonitorPanel`（`MonitorPanel.tsx:81`）与 `MobileDashboard` 的 `MobileMonitorCard`（`components/mobile/MobileDashboard.tsx:81`）消费；`StatusBar` 也订阅 `bgStatus.campusWifi/campusWired/adapterStatuses`（`StatusBar.tsx:35-38`）。
- `network-quality-result` → `useEventListeners.ts:327` → `handleQualityBadAlert`（`useEventListeners.ts:34`，质量转差时弹警告 toast）→ `useQualityStore.setNetworkQuality` → `QualityPanel`（`QualityPanel.tsx:111`）/`NetworkQualityCapsule`（`NetworkQualityCapsule.tsx:92`）/`MobileDashboard` 的 `NetworkQualityCard`（`DashboardPanel.tsx:365`）。
- `update-available` → `useEventListeners.ts:335` → `useQualityStore.setUpdatePromptOpen(true)` → `UpdateAvailableDialog`（`shared/UpdateAvailableDialog.tsx:20`）→ `onGoUpdate` 打开关于页（`App.tsx:252`、`TabletShell.tsx:347`）。
- `login-log` → `useEventListeners.ts:263` → `useLogToastStore.addLog` → `LogPanel`/`RightPanel`/`ToastContainer` 的日志与 toast 面。
- `config-changed` → `useEventListeners.ts:350` → `mergeConfigFromBackend`（`useConfigStore.ts:110`）→ 各面板受控输入回读（脏字段跳过）。

### 向导流程（两端共用状态机）

`useOnboardingFlow`（`useOnboardingFlow.ts:45`）在 `open` 由假转真时重置全部字段（`useOnboardingFlow.ts:78-94`）→ step0 欢迎（语言切换 `setLanguage`）→ step1 绑定：`handleBind`（114-143）先过 `ensureBindHello`（`useOnboardingFlow.ts:71` → `useHelloGate`），成功后 `BIND_SUCCESS_ADVANCE_MS=1200` 自动进 step2（130-134）→ step2 账号：`goNext`（164）内部 `handleNext`（148）在 step2 落盘 `user`/`operator`（仅非空密码才写 `password`，155-158）→ step3 完成页：`handleLoginAndFinish`（177）做最终校验（179）后 `onUpdateConfig` → `onLogin()`（193）→ 成功后 `LOGIN_SUCCESS_ADVANCE_MS=1500` 写 `campus-onboarding-done` 并 `onClose`（197-200）；「跳过」走 `handleSkip`（168）直接写标记并关闭。

手机外壳的首次启动引导由 `App.tsx:100-108` 判定（无 `campus-onboarding-done` 且 `config.user` 为空时打开），平板外壳同逻辑在 `TabletShell.tsx:148-153`。

## Connections

- 运行时骨架、store 契约与 IPC 面：[[android-frontend-core]]（`tauriApi` 全部方法与行号、六个领域 store 的 state 字段、`DesktopPanel` 之外的钩子）。
- 后端命令与事件实现：[[android-backend]]（`do_login`/`query_self_dashboard`/`start_background_check`/`check_update`/`get_battery_optimization_info`、`background-check-result`/`network-quality-result`/`update-available` 载荷）。
- 插件与平台门控：[[android-plugins]]（`plugin-biometric` 用于 `useHelloGate`、`plugin-notification` 用于巡检前台服务、`plugin-opener` 用于 `openExternal`）。
- 桌面同构对照（复刻分叉需双端同步）：[[desktop-frontend-panels]]（桌面 `DashboardPanel` 全量卡片、桌面 `SettingsPanel`/`MonitorPanel`/`QualityPanel`/`LogPanel`）、[[desktop-frontend-shared]]、[[desktop-frontend-hooks]]。
- 被安卓裁剪的面板与能力：[[desktop-network-core]]（`network/NetworkPanel` 在安卓有文件无入口）、[[desktop-network-dns]]（DNS 优化为 Windows 注册表能力）、[[desktop-network-quality]]（质量检测后端与前端展示契约）、[[desktop-monitor]]、[[desktop-config]]（`Config` 字段与默认值全表）、[[desktop-auth]]、[[desktop-account-selfservice]]、[[desktop-helper-update]]（更新/镜像/安装链路）、[[desktop-platform]]（窗口控制与托盘开关，安卓一律不渲染）。

## Known Issues

1. **`network/` 整个面板层在安卓是死代码**：`NetworkPanel`（`network/NetworkPanel.tsx:47`）与 `useNetwork`（`network/useNetwork.ts:39`）全仓无 import（仅 `network/index.ts:1-2` barrel 导出）；`NAV_ITEMS`（`shared/ui-constants.ts:7-15`）不含 `network`，`TabletShell.tsx:10` 注释亦明确「network 面板不接入」，而 `PanelName`（`shared/ui-types.ts:2`）仍保留 `'network'`，`DEFAULT_PANEL_OPTIONS`（`settings/constants.ts:74`）由 `NAV_ITEMS` 派生故不含它。若将来接入，该面板依赖的 `checkDnsDohStatus`/`setupDnsDoh`/`dhcpReleaseRenewAdapter`/`enableAdapter` 在安卓全是 `desktopOnly` reject（`hooks/tauriApi.ts:279` / `280` / `236` / `171`）。
2. **平板端关于弹窗没有 APK 兜底**：平板壳用 `AboutDialog`（`TabletShell.tsx:48`），其默认资产 URL 兜底是 Windows 安装包 `Wxxy-CampusLogin_{版本}_x64-setup.exe`（`auth/AboutDialog.tsx:237`），资产筛选也只认 `.exe`/`.msi`（`auth/AboutDialog.tsx:231-233`）；手机壳用的 `AboutDialogMobile` 才按 `.apk` 筛选（`auth/AboutDialogMobile.tsx:68`）并带「外链 Releases」兜底（`auth/AboutDialogMobile.tsx:240-249`）。Release 缺 APK 资产时平板端会走到一个下不动 .exe 的分支。
3. **`DashboardPanel` 的 `quickActions` 卡被排除但仍在编辑体系内**：手机/平板都走 `MobileDashboard`，其 `EXCLUDED_CARDS = ['quickActions']`（`components/mobile/MobileDashboard.tsx:32`）通过 `excludeCards` 传入（`MobileDashboard.tsx:144`）；`visibleCards`（`DashboardPanel.tsx:891-898`）与 `availableCards`（`DashboardPanel.tsx:881-889`，883-885 行按 `excluded` 过滤）都不会列出它，因此该卡既不可见也无法重新添加，却仍占据 `cards` 数组（`DashboardPanel.tsx:901-911` 的 hidden 尾部分支把它留在尾部）。三个必填的 DHCP prop 因此只能传 `noopAsync`（`MobileDashboard.tsx:29`、`MobileDashboard.tsx:140-142`）。
4. **`QuickActionsCard` 里 `resolveAdapterNames` 的结果在安卓恒为空**：`DashboardPanel.tsx:175-177` 依赖 `adapters`，而安卓 `adapters` 恒为 `[]`（`useAdapterStore.ts:49`，`getAdapters` reject），因此 `resolved.primary`/`resolved.secondary` 为空、菜单项（`DashboardPanel.tsx:254-281`）永不渲染。该卡在安卓本不可见（见上条），但代码仍留在这里，属复刻载荷。
5. **平台门控写成运行时常量（三处），安卓分支为死代码**：`AccountPanel.tsx:55`、`MonitorPanel.tsx:23`、`SettingsPanel.tsx:31` 各自 `const isAndroid = import.meta.env.VITE_PLATFORM === 'android'`，配套的 `!isAndroid &&` 区块（`AccountPanel.tsx:448-474` 自动退出/上线退出、`MonitorPanel.tsx:246-267` 上线退出、`MonitorPanel.tsx:346-367` 非校园网退出、`SettingsPanel.tsx:302-380` 静默启动/最小化托盘/默认面板/自动退出）在安卓构建中恒不渲染。这些块与桌面前端逐字同源（桌面同一文件里 `isAndroid` 为假），因此**改一端必须同步另一端**，否则两端 `isAndroid` 语义分叉；`VITE_PLATFORM` 未在 `vite-env.d.ts` 中声明类型（`src/vite-env.d.ts:1` 只有一行 vite/client 引用），拼错字符串不会被类型检查拦下。
6. **`SettingsPanel` 的间隔默认值三处不一致**：`QualityPanel` 的延迟测试间隔回退值是 30000ms（`monitor/QualityPanel.tsx:158`，`config.latencyTestInterval || 30000`），而 `DEFAULT_CONFIG.latencyTestInterval` 是 60000（`settings/constants.ts:33`）；`MonitorPanel` 的巡检间隔回退 60000（`monitor/MonitorPanel.tsx:85`）与默认一致，但 `MobileDashboard` 的监控摘要卡用 `Math.max(5, ...)` 下限 5s（`components/mobile/MobileDashboard.tsx:85`），而 `MonitorPanel` 的 `commitInterval` 下限是 10s（`monitor/MonitorPanel.tsx:103`）——同一个 config 字段在两处 UI 的下限不同。
7. **`config.backgroundCheckInterval` 被 UI 以秒呈现、以毫秒落盘，跨处转换点分散**：`MonitorPanel.tsx:85`（读）、`MonitorPanel.tsx:105`（写）、`MobileDashboard.tsx:85`（读）、`MobileDashboard.tsx:102`（写）、`useMonitor.ts:31`（写）共 5 处 `/1000`、`*1000` 换算，任一处遗漏即出现「设 60 秒实际 60000 秒」类偏差。
8. **`campusCheckEndMinutes = 0` 的 UI 呈现有歧义**：`MonitorPanel.tsx:400-403` 把 0 渲染为 `00:00`，与「不限制/未设置」语义无视觉区分（默认值即 0，`settings/constants.ts:47`）；时间窗判定在后端，前端只透传。
9. **`StatusBar` 的 `wasOffline` 在渲染期读 ref**：`monitor/StatusBar.tsx:42` 在 render 中读取 `prevStatusRef.current`，而该 ref 在 `useEffect`（`StatusBar.tsx:44-46`）里更新；React 严格模式下渲染可能被丢弃重放，此时基于 ref 的「刚从离线恢复」判定可能与实际提交节奏不一致（只影响 `status-enter-from-offline` 动画类，不影响状态语义）。另外手机外壳自绘 header 的状态点（`App.tsx:168-175`）与 `StatusBar` 的胶囊（`StatusBar.tsx:110-130`）是两套实现，颜色映射写了两遍。
10. **`about` 两个弹窗重复实现同一后端链路**：`AboutDialog`（平板）与 `AboutDialogMobile`（手机）各自实现 `checkUpdate` → `getMirrorUrls` → `downloadUpdate` → `installUpdate`（`auth/AboutDialog.tsx:122-264`、`auth/AboutDialogMobile.tsx:50-110`），错误文案映射（403/404 分支）也各写一份（`auth/AboutDialog.tsx:133-137`、`auth/AboutDialogMobile.tsx:59-61`）；`AboutDialogMobile` 的镜像顺序由 `updateSource` 决定（`auth/AboutDialogMobile.tsx:79-81`），`AboutDialog` 无此逻辑（`auth/AboutDialog.tsx:253` 固定选非 GitHub 源）。
11. **2D 人脸开关与模板的持久化不一致会产生不可达功能**：`allow2dFaceVerify` 存于后端配置，模板存于 `safeStorage`（`face/faceService.ts:20` 的 `campus-2d-face-template`）。关闭开关时前端清模板（`settings/SettingsPanel.tsx:487-489`），但若 WebView 数据被清（或换设备恢复配置），会出现 `allow2dFaceVerify = true` 而 `hasTemplate() = false`，此时 `shouldUseFaceFallback()`（`face/faceVerifyStore.ts:44-46`）返回 false，验证落到系统 `BiometricPrompt`（`tauriApi.ts:209`）——在只有 Class 1 2D 人脸的设备上该链路不可用（`face/faceService.ts:5-8` 注释说明的场景），用户无从察觉，只能去设置页重录。
12. **自助面板的「切入自动验证」有多重前置条件，条件不全时用户看不到任何提示**：`SelfServicePanel.tsx:227-236` 要求 `configLoaded && !pwdTouchedRef.current && account.trim() && selfPasswordSaved` 才自动查询；任一不满足就静默等待手动刷新（`SelfServicePanel.tsx:313`）。其中 `configLoaded`（`useConfigStore.ts:32`）在 `getInitData` 失败降级时也置 true（`hooks/useInitialDataLoad.ts:113`），此时面板会以默认空凭据渲染而非报错。
13. **`useSelfCardReveal` 的验证门与查看明文门是同一个状态机、但语义不同**：`auth/DashboardPanel.tsx:410-422` 走 `useHelloGate()`（默认无 `ignoreToggle`，总开关关闭时直接放行），而 `AccountPanel` 查看明文走 `useHelloGate({ignoreToggle:true})`（`account/AccountPanel.tsx:254`，总开关关闭仍强制验证）。两处「点眼睛查看」的验证强度不同，容易被误认为同一个门。
14. **`useHelloGate` 的门是模块级单例，跨面板共享一份时间戳**：`account/selfServiceState.ts:83`（绑定门）与 `account/selfServiceState.ts:117`（自助会话门）是两个独立变量，但**绑定门只有一个**：账号面板绑定卡（`AccountPanel.tsx:222`）与两个向导（`useOnboardingFlow.ts:71`）共用 `helloGateVerifiedAt`，所以在向导里验证过一次后，账号面板的绑定/查询在 TTL 内不再验证；反之亦然。注释（`account/selfServiceState.ts:60-64`）只声明「与自助服务面板的门独立」，未声明面板与向导之间共享。
15. **`MonitorPanel` 与 `MobileDashboard` 对同一后台检测状态有两套展示与两套动作**：`MonitorPanel.tsx:165` 与 `MobileDashboard.tsx:102` 都调 `handleToggleBackgroundCheck`，但前者传 `intervalSec = config/1000`（`MonitorPanel.tsx:85`，可能为 60），后者传 `Math.max(5, ...)`（`MobileDashboard.tsx:85`）——从总览卡切开关会用「下限 5s 的值」覆盖配置，可能与监控页显示值不同。
16. **`LogPanel` 的调试模式与保留天数在安卓行为依赖后端命令**：`getLogRetentionDays`/`setLogRetentionDays` 是安卓可用命令（`hooks/tauriApi.ts:283-284`），但 `AboutDialog`/`LogPanel` 之外无其他入口；`LogPanel` 的失败回滚逻辑（`shared/LogPanel.tsx:196-207`）依赖 `setLogRetentionDays` reject，若后端静默成功但未落盘则 UI 与磁盘不一致（无可验证前端的确认信号）。
17. **`SpeedTestPanel` 的 8 个测速站是硬编码常量**：`monitor/SpeedTestPanel.tsx:28-101`，桌面端若为同一份数据，站点增删需双端各改一次（本文件在两端都是复刻副本，不属于共享 crate）。
18. **`QualityPanel` 依赖后端 detail key 名做 i18n 查表**：`monitor/QualityPanel.tsx:401` 用 `t(\`quality.names.${item.name}\`)`，`item.name` 来自 `DETAIL_CATEGORIES` 硬编码列表（`QualityPanel.tsx:36-87`）；`LatencyTimeline` 的 `SEGMENT_INFO` 键（`monitor/LatencyTimeline.tsx:23-31`）包含后端原始中文 key `内容`/`网络`，注释（`monitor/LatencyTimeline.tsx:21-22`）声明这是跨语言契约——后端改 key 名会直接导致前端显示原始 token。
19. **手机端在默认配置下没有独立的「后台检测」底栏入口**：`BottomNav.tsx:28-30` 在质量检测开启（默认，`settings/constants.ts:36`）时第 4 位是质量页、monitor 项被挤出；monitor 只能经「更多 → 后台检测」到达（`MobileMore.tsx:21` 的 `qualityOnly: true`）。这是设计如此（注释 `BottomNav.tsx:11-12`），但意味着**质量开关是底栏结构的隐式开关**，调整默认值时需同时检查 `App.tsx:52-55` 的 tab 白名单（不含 `monitor`，见 [[android-frontend-core]] 的 Known Issues 第 14 条）与 `MobileMore.tsx:36-39` 的派生降级。
