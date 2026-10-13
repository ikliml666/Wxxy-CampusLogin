---
title: 安卓前端 - 面板层（总览 / 账号 / 自助 / 监控 / 质量 / 设置 / 日志）
type: module
source_files:
  - android/frontend/src/auth/AboutDialog.tsx
  - android/frontend/src/auth/index.ts
  - android/frontend/src/auth/types.ts
  - android/frontend/src/auth/useAuth.ts
  - android/frontend/src/account/index.ts
  - android/frontend/src/account/types.ts
  - android/frontend/src/monitor/index.ts
  - android/frontend/src/monitor/types.ts
  - android/frontend/src/network/constants.ts
  - android/frontend/src/network/index.ts
tags: [安卓, 前端, 面板, 总览, 账号, 自助服务, 网络质量, 设置, 日志, 新手向导]
---

## Overview

本模块覆盖安卓前端的业务面板层：关于与更新（`auth/AboutDialog.tsx`）、账号管理（`account/*`），以及适配器/DNS 面板的类型与常量定义（`network/*`，安卓无渲染入口但有类型输出）。

与桌面前端的关系：本层文件多为**复刻同构**（同名文件同名组件、逐行近似），但面板装配、可见项与平台门控完全不同——手机壳只渲染 `MobileDashboard`/`AccountPanel`/`SelfServicePanel`/`QualityPanel`/`MonitorPanel`/`More 聚合页`，平板壳渲染八个 Dock 面板且不含 `network`。

## Key Components

### auth/ — 关于与更新

| 导出 | 位置 | 用途 |
| --- | --- | --- |
| `AboutDialog` | `auth/AboutDialog.tsx:99` | 平板/宽屏关于弹窗：320px 侧栏 + 更新仪表盘（releaseNotes 全文渲染）+ 赞助内嵌页；支持 `initialLatestVersion`/`initialReleaseNotes`/`initialUpdateAvailable` 缓存入参 |
| `useAuth` | `auth/useAuth.ts:6` | 组合 `useAuthStore` 与 `config.api`，派生 `handleOpenPortal`（读 `config.portalUrl`，兜底 `http://10.1.99.100`，`useAuth.ts:22-25`）与 `handleOpenSelfService`（硬编码 `http://10.1.80.200:8080/Self/login/?302=LI`，`useAuth.ts:27-29`） |

`auth/index.ts` 只导出 `DashboardPanel`(1)、`AboutDialog`(2)、`useAuth`(3)、`export * from './types'`(5)。`AboutDialogMobile` 不在 barrel（`App.tsx:42` 按路径懒加载）。

`auth/types.ts`：`PortalStatusResult`(1)、`CommandResult`(8)、`LoginResult`(14)。

### account/ — 账号信息、运营商绑定与自助服务

| 导出 | 位置 | 用途 |
| --- | --- | --- |
| `AccountPanel` | `account/AccountPanel.tsx:44` | 账号面板（`memo`）：登录信息卡、自动化开关卡（含 2026-09-19 新增的夜切开关 `enableNightOperatorSwitch`，两端可见，见 [[decisions/night-operator-switch]]）、运营商绑定卡（绑定状态查询/查看明文/绑定表单，其下拉**故意排除无锡学院**，见 [[learnings/binding-operator-select-excludes-campus]]）、账号管理卡（新增/**堆叠卡 narrow**：2026-10-11 起手机/平板统一 `OnlineDeviceStackH variant="narrow"` 窄卡封面流，卡宽 min 213px、姿势同 coverflow 侧卡可见压暗、单卡居中禁拖禁切，双端同构见 [[learnings/translatex-sum-requires-calc]]/切换/改名/删除） |
| `useAccount` | `account/useAccount.ts:8` | 账号动作集合：`refreshAccounts`(23)、`handleAddAccount`(33，返回是否成功)、`handleDeleteAccount`(53)、`handleSwitchAccount`(73，守卫用 `!== undefined` 而非真值判断——R4 修复)、`handleRenameAccount`(95，成功返回是否成功供退出内联编辑) |

`account/index.ts`：`AccountPanel`(1)、`ChangePasswordDialog`(2)、`useAccount`(3)、`export * from './types'`(5)；`SelfServicePanel` 与 `selfServiceState` 不在 barrel。

`account/types.ts`：`AccountResult`(12)、`SwitchAccountResult`(18)、`DeleteAccountResult`(25)、`SaveAccountResult`(32)。

### monitor/ — 状态栏、巡检、质量与测速

| 导出 | 位置 | 用途 |
| --- | --- | --- |
| `StatusBar` | `monitor/StatusBar.tsx:23` | 平板顶栏状态条：状态胶囊（含离线/恢复动画 class）+ 质量胶囊 + 刷新 + 自助服务/门户外链（`onOpenSelfService` 可选） |
| `MonitorPanel` | `monitor/MonitorPanel.tsx:80` | 后台检测面板：启停/立即检测/间隔输入、适配器在线状态卡列表、验证设置（自动检测、可登录即登录、校园网名校校验、检测时间窗）。校园网验证 Label 旁的问号 `HelpCircle`（`:298-319`）把检测逻辑四步说明收进 Tooltip（步骤① 文案：匹配 `i-wxxy`、`iwxxy-2` 或 `iwxxy-3`）（沿用 `QualityPanel` 指标问号同款模式，不占常驻空间）。检测时间段行（`:390-434`）窄屏（<640px）纵向堆叠——右侧两个 `w-24` 的 `<input type="time">` 合计约 200px 不可压缩，同行布局会把左侧 `min-w-0` 标题挤成逐字竖排（360-430px 手机实测标题 14×164px），容器用 `flex-col gap-2 sm:flex-row sm:items-center sm:justify-between`，≥640px 恢复同一行；每日定时登录/注销两行（`:438-497`）同款堆叠，且单 time Input 外包 `flex shrink-0 items-center` 容器（Input 组件 time 类型自带 `relative w-full` 包装层会吃满剩余空间，见 `learnings/input-time-wrapper-w-full`） |
| `QualityPanel` | `monitor/QualityPanel.tsx:108` | 网络质量面板：质量指纹卡、定时测试卡、测试明细卡（5 类 tab + 每项时间线） |
| `SpeedTestPanel` | `monitor/SpeedTestPanel.tsx:109` | 测速站点集合（8 个硬编码站点，3 个分类），点击经 `openExternal` 外开 |
| `NetworkQualityCapsule` | `monitor/NetworkQualityCapsule.tsx:53` | 质量胶囊：延迟数字 + 悬停 portal 明细（网关/外网/DNS 三行），busy/unknown 时按延迟推断等级（`NetworkQualityCapsule.tsx:96-98`） |
| `LatencyPair` | `monitor/LatencyComponents.tsx:170` | 网关/外网双列延迟卡（含信号条 `SignalBars`） |
| `LatencyTimeline` | `monitor/LatencyTimeline.tsx:42` | 单目标分段时间线（UDP/DNS/TCP/TLS/TTFB/内容/网络 + 总时长） |
| `useMonitor` | `monitor/useMonitor.ts:7` | 巡检与延迟测试动作：`handleToggleBackgroundCheck`(25)、`handleTriggerCheck`(52)、`handleToggleLatencyTest`(56) |

`monitor/index.ts`：`MonitorPanel`(1)、`QualityPanel`(2)、`SpeedTestPanel`(3)、`LatencyPair`(4)、`LatencyTimeline`(5)、`NetworkQualityCapsule`(6)、`StatusBar`(7)、`useMonitor`(8)、`export * from './types'`(10)。

### network/ — 适配器与 DNS（安卓无入口）

| 导出 | 位置 | 用途 |
| --- | --- | --- |
| `NetworkPanel` | `network/NetworkPanel.tsx:47` | 适配器列表 + 适配器设置 + DNS 优化三卡面板；**安卓全仓无渲染点** |
| `QUALITY_CONFIG` | `network/constants.ts:1` | 9 档质量展示配置（`excellent/great/good/fair/poor/bad/unknown/disabled/busy`，每档 `label`/`labelKey`/`color`/`bg`/`border`/`borderBg`/`icon`/`hex`/`activeBars`/`glow`） |

`network/index.ts`：`NetworkPanel`(1)、`export * from './types'`(2)、`export * from './constants'`(3)。

## Architecture

```text
┌───────────────────────────────────────────────────────────────────────┐
│                         Android Frontend Panels                        │
├──────────────┬──────────────┬──────────────┬──────────────────────────┤
│   Auth       │   Account    │   Monitor    │      Network             │
│              │              │              │                          │
│ AboutDialog  │ AccountPanel │ MonitorPanel │ NetworkPanel (dead code) │
│ useAuth      │ ChangePwdDlg │ QualityPanel │ QUALITY_CONFIG           │
│ types        │ useAccount   │ SpeedTestPnl │ types                    │
│              │ types        │ LatencyComp  │                          │
└──────────────┴──────────────┴──────────────┴──────────────────────────┘
         │              │              │
         ▼              ▼              ▼
   Tauri IPC        Tauri IPC      Tauri IPC
   check_update     save_config    start_bg_check
   download_update  switch_acct    check_quality
   install_update   bind_operator  get_logs
```

## Data Flow

### 面板装配（谁渲染哪个面板）

| 外壳 | 装配点 | 面板清单 |
| --- | --- | --- |
| 手机外壳 | `App.tsx:111-154`（switch `deferredTab`） | `dashboard` → `MobileDashboard`(113)；`account` → `AccountPanel`(115-127)；`selfservice` → `SelfServicePanel`(129)；`quality` → `QualityPanel`(132-140，`enableNetworkQuality === false` 时返回 `null`)；`monitor` → `MonitorPanel`(142-150)；`more` → `MobileMore`(152) |
| 手机「更多」页 | `components/mobile/MobileMore.tsx:67-85` | chips 顺序 2026-09-19 起为 **settings → speedtest → log → monitor**（默认选中 `settings`）：`settings` → `SettingsPanel`(77)；`speedtest` → `SpeedTestPanel`(74)；`log` → `LogPanel`(75)；`monitor` → `MonitorPanel`(68，`qualityOnly`) |
| 平板外壳 | `components/tablet/TabletShell.tsx:162-229`（switch `deferredPanel`） | `dashboard` → `MobileDashboard`(164)；`account` → `AccountPanel`(167-179)；`selfservice` → `SelfServicePanel`(180)；`monitor` → `MonitorPanel`(183-189)；`quality` → `QualityPanel`(192-198)；`speedtest` → `SpeedTestPanel`(201-205)；`settings` → `SettingsPanel`(208-217)；`log` → `LogPanel`(220-223，`case 'log'` 在 220) |

手机端底栏位置是动态的：质量检测开启时第 4 位是「网络质量」（`BottomNav.tsx:30` 插 `QUALITY_TAB`），关闭时插 `MONITOR_TAB`（后台检测）；与之对称，`MobileMore` 的 monitor 子页带 `qualityOnly: true`（`MobileMore.tsx:21`）并按 `qualityEnabled` 过滤（`MobileMore.tsx:36`），关闭质量时「更多」页若停留在 monitor 派生降级为 settings（第一个 chip，`MobileMore.tsx:39`）。

### 面板 → store → `tauriApi.invoke` → 安卓 Rust 命令（真实调用点）

| 用户动作 | 组件调用点 | store/hook | IPC 实现 | 后端命令 |
| --- | --- | --- | --- | --- |
| 总览：切换账号 | `DashboardPanel.tsx:316` | `useAccount.handleSwitchAccount`（`useAccount.ts:75`） | `tauriApi.ts:226` | `switch_account` |
| 总览：在线设备卡自动查询 | `DashboardPanel.tsx:500` | 直调 `tauriApiWithRetry` | `tauriApi.ts:213` | `query_self_dashboard` |
| 总览：踢设备下线 | `DashboardPanel.tsx:520` | 直调 | `tauriApi.ts:214` | `self_offline_session` |
| 总览：今日上网记录 | `DashboardPanel.tsx:629` | 直调 | `tauriApi.ts:215` | `query_self_online_log` |
| 总览：质量卡刷新 | `DashboardPanel.tsx:395` → `onRefreshQuality` | `useQualityStore.refreshQuality`（`useQualityStore.ts:58`） | `tauriApi.ts:237` | `check_network_quality` |
| 账号：保存新账号 | `AccountPanel.tsx:131` | `useAccount.handleAddAccount`（`useAccount.ts:33`） | `tauriApi.ts:227` | `save_current_as_account` |
| 账号：删除账号 | `AccountPanel.tsx:820` → 外壳确认框（`App.tsx:278`） | `useAccount.handleDeleteAccount`（`useAccount.ts:51`） | `tauriApi.ts:228` | `delete_account` |
| 账号：绑定状态查询 | `AccountPanel.tsx:265`（先过 `useHelloGate`） | 直调 | `tauriApi.ts:179` | `query_bind_status` |
| 账号：查看运营商明文密码 | `AccountPanel.tsx:296`（`ensureRevealVerified`，`ignoreToggle`） | `useHelloGate({ignoreToggle:true})`（`AccountPanel.tsx:295`） | `tauriApi.ts:212` | `reveal_operator_credential` |
| 账号/向导：绑定运营商 | `AccountPanel.tsx:322`；`useOnboardingFlow.ts:120` | `useHelloGate`（`AccountPanel.tsx:263`；`useOnboardingFlow.ts:71`） | `tauriApi.ts:178` | `bind_operator` |
| 账号：清除已保存密码 | `AccountPanel.tsx:100-105`（`handleClearPassword`，`:102`） | `saveConfigDirect({password:''}, true)`（凭据统一后仅登录密码一个清除动作，见 [[credential-single-source]]） | `tauriApi.ts:199` | `save_config`（`clearPassword`） |
| 自助：查询在线信息 | `SelfServicePanel.tsx:204` | `useSelfServiceVerify`（`SelfServicePanel.tsx:141`） | `tauriApi.ts:213` | `query_self_dashboard` |
| 自助：踢下线 | `SelfServicePanel.tsx:243` | 同上 | `tauriApi.ts:214` | `self_offline_session` |
| 自助：使用记录查询 | `SelfServicePanel.tsx:273` | 同上 | `tauriApi.ts:215` | `query_self_online_log` |
| 自助：密码失焦保存 | `SelfServicePanel.tsx:180` | `saveConfigDirect`（`useConfigStore.ts:128`） | `tauriApi.ts:168` | `save_config` |
| 巡检：启停/立即检测 | `MonitorPanel.tsx:166` / `MonitorPanel.tsx:156` | `useMonitor.handleToggleBackgroundCheck`（`useMonitor.ts:29`）/ `handleTriggerCheck`（`useMonitor.ts:53`） | `tauriApi.ts:230` / `tauriApi.ts:232` | `start_background_check` / `trigger_background_check` |
| 巡检：间隔提交 | `MonitorPanel.tsx:106` | `onUpdateConfig` → debounce | `tauriApi.ts:168` | `save_config` |
| 质量：定时测试启停 | `QualityPanel.tsx:282` | `useMonitor.handleToggleLatencyTest`（`useMonitor.ts:60`） | `tauriApi.ts:239` / `tauriApi.ts:240` | `start_latency_test` / `stop_latency_test` |
| 设置：开机自启 | `SettingsPanel.tsx:285` | `useSettings.handleSetAutoLaunch`（`useSettings.ts:52`） | `tauriApi.ts:254` | `set_boot_autostart` |
| 设置：通知开关 | `SettingsPanel.tsx:408` | `updateConfig`；标题栏/平板走 `useSettings.handleToggleNotification`（`useSettings.ts:46`） | `tauriApi.ts:256` | `set_notification_enabled` |
| 设置：关闭质量检测（联动） | `SettingsPanel.tsx:586-604` | 直接 `tauriApiWithRetry.stopLatencyTest()`（`SettingsPanel.tsx:599`，2026-10-03 重构后不再经 store 的 api 字段） | `tauriApi.ts:240` | `stop_latency_test` |
| 设置：2D 人脸录入 | `SettingsPanel.tsx:504` | `useFaceDialogStore.openFaceDialog('enroll')` | `faceService.ts:155`（本地，无 IPC） | —— |
| 保活：申请电池白名单 | `KeepAliveSettingsCard.tsx:52` | 直调 | `tauriApi.ts:258` | `request_ignore_battery_optimizations` |
| 保活：厂商省电页 | `KeepAliveSettingsCard.tsx:65` | 直调 | `tauriApi.ts:259` | `open_vendor_battery_settings` |
| 保活：状态读取 | `KeepAliveSettingsCard.tsx:34` | 直调 | `tauriApi.ts:257` | `get_battery_optimization_info` |
| 日志：拉取/清空/调试/保留 | `LogPanel.tsx:125` / `329` / `184` / `200`（另有 `158` 读调试模式、`162` 读保留天数） | 直调 `api`：外壳静态导入 `tauriApiWithRetry` 注入（手机经 `components/mobile/MobileMore.tsx:42`，平板经 `components/tablet/TabletShell.tsx:86`） | `tauriApi.ts:267` / `268` / `269` / `270` / `283` / `284` | `get_logs` / `clear_logs` / `get_debug_mode` / `set_debug_mode` / `get_log_retention_days` / `set_log_retention_days` |
| 关于：检查更新 / 下载 / 安装 | `AboutDialogMobile.tsx:54` / `87` / `106`；`AboutDialog.tsx:126` / `189` / `220` | 直调 | `tauriApi.ts:273` / `274` / `275` | `check_update` / `download_update` / `install_update` |
| 关于：镜像源列表 | `AboutDialogMobile.tsx:78`；`AboutDialog.tsx:250`/`476` | 直调 | `tauriApi.ts:276` | `get_mirror_urls` |
| 向导：绑定 + 登录 | `useOnboardingFlow.ts:120` / `193` | `onLogin` 由外壳传入 `useAuthStore.doLogin`（`App.tsx:288`、`TabletShell.tsx:380`） | `tauriApi.ts:178` / `176` | `bind_operator` / `do_login` |

### 事件进入面板的路径

- `background-check-result` → `useEventListeners.ts:77` 写 `useAuthStore.bgStatus` → `MonitorPanel`（`MonitorPanel.tsx:82`）与 `MobileDashboard` 的 `MobileMonitorCard`（`components/mobile/MobileDashboard.tsx:81`）消费；`StatusBar` 也订阅 `bgStatus.campusWifi/campusWired/adapterStatuses`（`StatusBar.tsx:35-38`）。
- `network-quality-result` → `useEventListeners.ts:327` → `handleQualityBadAlert`（`useEventListeners.ts:34`，质量转差时弹警告 toast）→ `useQualityStore.setNetworkQuality` → `QualityPanel`（`QualityPanel.tsx:111`）/`NetworkQualityCapsule`（`NetworkQualityCapsule.tsx:92`）/`MobileDashboard` 的 `NetworkQualityCard`（`DashboardPanel.tsx:365`）。
- `update-available` → `useEventListeners.ts:335` → `useQualityStore.setUpdatePromptOpen(true)` → `UpdateAvailableDialog`（`shared/UpdateAvailableDialog.tsx:20`）→ `onGoUpdate` 打开关于页（`App.tsx:253`、`TabletShell.tsx:354`）。
- `login-log` → `useEventListeners.ts:263` → `useLogToastStore.addLog` → `LogPanel`/`RightPanel`/`ToastContainer` 的日志与 toast 面。
- `config-changed` → `useEventListeners.ts:350` → `mergeConfigFromBackend`（`useConfigStore.ts:109`）→ 各面板受控输入回读（脏字段跳过）。

### 向导流程（两端共用状态机）

`useOnboardingFlow`（`useOnboardingFlow.ts:45`）在 `open` 由假转真时重置全部字段（`useOnboardingFlow.ts:78-94`）→ step0 欢迎（语言切换 `setLanguage`）→ step1 绑定：`handleBind`（114-143）先过 `ensureBindHello`（`useOnboardingFlow.ts:71` → `useHelloGate`），成功后 `BIND_SUCCESS_ADVANCE_MS=1200` 自动进 step2（130-134）→ step2 账号：`goNext`（164）内部 `handleNext`（148）在 step2 落盘 `user`/`operator`（仅非空密码才写 `password`，155-158）→ step3 完成页：`handleLoginAndFinish`（177）做最终校验（179）后 `onUpdateConfig` → `onLogin()`（193）→ 成功后 `LOGIN_SUCCESS_ADVANCE_MS=1500` 写 `campus-onboarding-done` 并 `onClose`（197-200）；「跳过」走 `handleSkip`（168）直接写标记并关闭。

手机外壳的首次启动引导由 `App.tsx:102-108` 判定（无 `campus-onboarding-done` 且 `config.user` 为空时打开），平板外壳同逻辑在 `TabletShell.tsx:149-154`。

## Connections

- 运行时骨架、store 契约与 IPC 面：[[android-frontend-core]]（`tauriApi` 全部方法与行号、六个领域 store 的 state 字段、`DesktopPanel` 之外的钩子）。
- 后端命令与事件实现：[[android-backend]]（`do_login`/`query_self_dashboard`/`start_background_check`/`check_update`/`get_battery_optimization_info`、`background-check-result`/`network-quality-result`/`update-available` 载荷）。
- 插件与平台门控：[[android-plugins]]（`plugin-biometric` 用于 `useHelloGate`、`plugin-notification` 用于巡检前台服务、`plugin-opener` 用于 `openExternal`）。
- 桌面同构对照（复刻分叉需双端同步）：[[desktop-frontend-panels]]（桌面 `DashboardPanel` 全量卡片、桌面 `SettingsPanel`/`MonitorPanel`/`QualityPanel`/`LogPanel`）、[[desktop-frontend-shared]]、[[desktop-frontend-hooks]]。
- 被安卓裁剪的面板与能力：[[desktop-network-core]]（`network/NetworkPanel` 在安卓有文件无入口）、[[desktop-network-dns]]（DNS 优化为 Windows 注册表能力）、[[desktop-network-quality]]（质量检测后端与前端展示契约）、[[desktop-monitor]]、[[desktop-config]]（`Config` 字段与默认值全表）、[[desktop-auth]]、[[desktop-account-selfservice]]、[[desktop-helper-update]]（更新/镜像/安装链路）、[[desktop-platform]]（窗口控制与托盘开关，安卓一律不渲染）。

## Known Issues

1. **`network/` 整个面板层在安卓是死代码**：`NetworkPanel`（`network/NetworkPanel.tsx:47`）与 `useNetwork`（`network/useNetwork.ts:39`）全仓无 import（仅 `network/index.ts:1-2` barrel 导出）；`NAV_ITEMS`（`shared/ui-constants.ts:7-15`）不含 `network`，`TabletShell.tsx:10` 注释亦明确「network 面板不接入」，而 `PanelName`（`shared/ui-types.ts:2`）仍保留 `'network'`，`DEFAULT_PANEL_OPTIONS`（`settings/constants.ts:74`）由 `NAV_ITEMS` 派生故不含它。若将来接入，该面板依赖的 `checkDnsDohStatus`/`setupDnsDoh`/`dhcpReleaseRenewAdapter`/`enableAdapter` 在安卓全是 `desktopOnly` reject（`hooks/tauriApi.ts:279` / `280` / `236` / `171`）。
2. **平板端关于弹窗没有 APK 兜底**：平板壳用 `AboutDialog`（`TabletShell.tsx:48`），其默认资产 URL 兜底是 Windows 安装包 `Wxxy-CampusLogin_{版本}_x64-setup.exe`（`auth/AboutDialog.tsx:237`），资产筛选也只认 `.exe`/`.msi`（`auth/AboutDialog.tsx:231-233`）；手机壳用的 `AboutDialogMobile` 才按 `.apk` 筛选（`auth/AboutDialogMobile.tsx:68`）并带「外链 Releases」兜底（`auth/AboutDialogMobile.tsx:240-249`）。Release 缺 APK 资产时平板端会走到一个下不动 .exe 的分支。
3. **`DashboardPanel` 的 `quickActions` 卡被排除但仍在编辑体系内**：手机/平板都走 `MobileDashboard`，其 `EXCLUDED_CARDS = ['quickActions']`（`components/mobile/MobileDashboard.tsx:32`）通过 `excludeCards` 传入（`MobileDashboard.tsx:144`）；`visibleCards`（`DashboardPanel.tsx:897-904`）与 `availableCards`（`DashboardPanel.tsx:887-895`，889-891 行按 `excluded` 过滤）都不会列出它，因此该卡既不可见也无法重新添加，却仍占据 `cards` 数组（`DashboardPanel.tsx:907-917` 的 hidden 尾部分支把它留在尾部）。三个必填的 DHCP prop 因此只能传 `noopAsync`（`MobileDashboard.tsx:29`、`MobileDashboard.tsx:140-142`）。
4. **`QuickActionsCard` 里 `resolveAdapterNames` 的结果在安卓恒为空**：`DashboardPanel.tsx:176-178` 依赖 `adapters`，而安卓 `adapters` 恒为 `[]`（`useAdapterStore.ts:49`，`getAdapters` reject），因此 `resolved.primary`/`resolved.secondary` 为空、菜单项（`DashboardPanel.tsx:255-282`）永不渲染。该卡在安卓本不可见（见上条），但代码仍留在这里，属复刻载荷。
5. **平台门控写成运行时常量（三处），安卓分支为死代码**：`AccountPanel.tsx:59`、`MonitorPanel.tsx:24`、`SettingsPanel.tsx:31` 各自 `const isAndroid = import.meta.env.VITE_PLATFORM === 'android'`，配套的 `!isAndroid &&` 区块（`AccountPanel.tsx:489-515` 自动退出/上线退出、`MonitorPanel.tsx:247-268` 上线退出、`MonitorPanel.tsx:365-386` 非校园网退出、`SettingsPanel.tsx:302-380` 静默启动/最小化托盘/默认面板/自动退出）在安卓构建中恒不渲染。这些块与桌面前端逐字同源（桌面同一文件里 `isAndroid` 为假），因此**改一端必须同步另一端**，否则两端 `isAndroid` 语义分叉；`VITE_PLATFORM` 未在 `vite-env.d.ts` 中声明类型（`src/vite-env.d.ts:1` 只有一行 vite/client 引用），拼错字符串不会被类型检查拦下。
6. **`SettingsPanel` 的间隔默认值三处不一致**：`QualityPanel` 的延迟测试间隔回退值是 30000ms（`monitor/QualityPanel.tsx:158`，`config.latencyTestInterval || 30000`），而 `DEFAULT_CONFIG.latencyTestInterval` 是 60000（`settings/constants.ts:33`）；`MonitorPanel` 的巡检间隔回退 60000（`monitor/MonitorPanel.tsx:86`）与默认一致，但 `MobileDashboard` 的监控摘要卡用 `Math.max(5, ...)` 下限 5s（`components/mobile/MobileDashboard.tsx:85`），而 `MonitorPanel` 的 `commitInterval` 下限是 10s（`monitor/MonitorPanel.tsx:104`）——同一个 config 字段在两处 UI 的下限不同。
7. **`config.backgroundCheckInterval` 被 UI 以秒呈现、以毫秒落盘，跨处转换点分散**：`MonitorPanel.tsx:86`（读）、`MonitorPanel.tsx:106`（写）、`MobileDashboard.tsx:85`（读）、`MobileDashboard.tsx:102`（写）、`useMonitor.ts:31`（写）共 5 处 `/1000`、`*1000` 换算，任一处遗漏即出现「设 60 秒实际 60000 秒」类偏差。
8. **`campusCheckEndMinutes = 0`（用户手选 00:00）的 UI 呈现有歧义**：`MonitorPanel.tsx:405-428` 把 0 渲染为 `00:00`，与「仅开始时间限制」语义无视觉区分。默认值已在 2026-09-13 从 0 改为 1380=23:00（`settings/constants.ts:48` + 后端 v4→v5 迁移双源刷新），UI 空值兜底也已在 2026-09-14 与后端默认对齐（`campusCheckStartMinutes ?? 460` / `campusCheckEndMinutes ?? 1380`，`MonitorPanel.tsx:405` / `:422`，双端同构），但歧义本身仍在：用户把手选终点拨到 00:00 时实际含义是「只有开始时间限制」，界面上与"午夜截止"无法区分。时间窗判定在后端，前端只透传。
9. **`StatusBar` 的 `wasOffline` 在渲染期读 ref**：`monitor/StatusBar.tsx:42` 在 render 中读取 `prevStatusRef.current`，而该 ref 在 `useEffect`（`StatusBar.tsx:44-46`）里更新；React 严格模式下渲染可能被丢弃重放，此时基于 ref 的「刚从离线恢复」判定可能与实际提交节奏不一致（只影响 `status-enter-from-offline` 动画类，不影响状态语义）。另外手机外壳自绘 header 的状态点（`App.tsx:168-175`）与 `StatusBar` 的胶囊（`StatusBar.tsx:110-130`）是两套实现，颜色映射写了两遍。
10. **`about` 两个弹窗重复实现同一后端链路**：`AboutDialog`（平板）与 `AboutDialogMobile`（手机）各自实现 `checkUpdate` → `getMirrorUrls` → `downloadUpdate` → `installUpdate`（`auth/AboutDialog.tsx:122-264`、`auth/AboutDialogMobile.tsx:50-110`），错误文案映射（403/404 分支）也各写一份（`auth/AboutDialog.tsx:133-137`、`auth/AboutDialogMobile.tsx:59-61`）；`AboutDialogMobile` 的镜像顺序由 `updateSource` 决定（`auth/AboutDialogMobile.tsx:79-81`），`AboutDialog` 无此逻辑（`auth/AboutDialog.tsx:253` 固定选非 GitHub 源）。
11. **2D 人脸开关与模板的持久化不一致会产生不可达功能**：`allow2dFaceVerify` 存于后端配置，模板存于 `safeStorage`（`face/faceService.ts:20` 的 `campus-2d-face-template`）。关闭开关时前端清模板（`settings/SettingsPanel.tsx:487-489`），但若 WebView 数据被清（或换设备恢复配置），会出现 `allow2dFaceVerify = true` 而 `hasTemplate() = false`，此时 `shouldUseFaceFallback()`（`face/faceVerifyStore.ts:44-46`）返回 false，验证落到系统 `BiometricPrompt`（`tauriApi.ts:209`）——在只有 Class 1 2D 人脸的设备上该链路不可用（`face/faceService.ts:5-8` 注释说明的场景），用户无从察觉，只能去设置页重录。
12. **自助面板的「切入自动验证」有多重前置条件，条件不全时用户看不到任何提示**：`SelfServicePanel.tsx:139-149`（注释 `:139-143`）要求 `configLoaded && hasCred`（`config.user.trim() && passwordSaved`，`SelfServicePanel.tsx:113`）才自动查询；任一不满足就静默等待手动刷新。其中 `configLoaded`（`useConfigStore.ts`）在 `getInitData` 失败降级时也置 true（`hooks/useInitialDataLoad.ts:113`），此时面板会以默认空凭据渲染而非报错。（自动验证原有的额外前置条件已随凭据统一移除，现仅剩双状态判定，见 [[credential-single-source]]）
13. **`useSelfCardReveal` 的验证门与查看明文门是同一个状态机、但语义不同**：`auth/DashboardPanel.tsx:312-323` 走 `useHelloGate()`（默认无 `ignoreToggle`，总开关关闭时直接放行），而 `AccountPanel` 查看明文走 `useHelloGate({ignoreToggle:true})`（`account/AccountPanel.tsx:244`，总开关关闭仍强制验证）。两处「点眼睛查看」的验证强度不同，容易被误认为同一个门。
14. **`useHelloGate` 的门是模块级单例，跨面板共享一份时间戳**：`account/selfServiceState.ts:63`（绑定门）与 `account/selfServiceState.ts:97`（自助会话门）是两个独立变量，但**绑定门只有一个**：账号面板绑定卡（`AccountPanel.tsx:212`）与两个向导（`useOnboardingFlow.ts:70`）共用 `helloGateVerifiedAt`，所以在向导里验证过一次后，账号面板的绑定/查询在 TTL 内不再验证；反之亦然。注释（`account/selfServiceState.ts:40-47`）只声明「与自助服务面板的门独立」，未声明面板与向导之间共享。
15. **`MonitorPanel` 与 `MobileDashboard` 对同一后台检测状态有两套展示与两套动作**：`MonitorPanel.tsx:166` 与 `MobileDashboard.tsx:102` 都调 `handleToggleBackgroundCheck`，但前者传 `intervalSec = config/1000`（`MonitorPanel.tsx:86`，可能为 60），后者传 `Math.max(5, ...)`（`MobileDashboard.tsx:85`）——从总览卡切开关会用「下限 5s 的值」覆盖配置，可能与监控页显示值不同。
16. **`LogPanel` 的调试模式与保留天数在安卓行为依赖后端命令**：`getLogRetentionDays`/`setLogRetentionDays` 是安卓可用命令（`hooks/tauriApi.ts:283-284`），但 `AboutDialog`/`LogPanel` 之外无其他入口；`LogPanel` 的失败回滚逻辑（`shared/LogPanel.tsx:196-207`）依赖 `setLogRetentionDays` reject，若后端静默成功但未落盘则 UI 与磁盘不一致（无可验证前端的确认信号）。
17. **`SpeedTestPanel` 的 8 个测速站是硬编码常量**：`monitor/SpeedTestPanel.tsx:28-101`，桌面端若为同一份数据，站点增删需双端各改一次（本文件在两端都是复刻副本，不属于共享 crate）。
18. **`QualityPanel` 依赖后端 detail key 名做 i18n 查表**：`monitor/QualityPanel.tsx:401` 用 `t(\`quality.names.${item.name}\`)`，`item.name` 来自 `DETAIL_CATEGORIES` 硬编码列表（`QualityPanel.tsx:36-87`）；`LatencyTimeline` 的 `SEGMENT_INFO` 键（`monitor/LatencyTimeline.tsx:23-31`）包含后端原始中文 key `内容`/`网络`，注释（`monitor/LatencyTimeline.tsx:21-22`）声明这是跨语言契约——后端改 key 名会直接导致前端显示原始 token。
19. **手机端在默认配置下没有独立的「后台检测」底栏入口**：`BottomNav.tsx:28-30` 在质量检测开启（默认，`settings/constants.ts:36`）时第 4 位是质量页、monitor 项被挤出；monitor 只能经「更多 → 后台检测」到达（`MobileMore.tsx:21` 的 `qualityOnly: true`）。这是设计如此（注释 `BottomNav.tsx:11-12`），但意味着**质量开关是底栏结构的隐式开关**，调整默认值时需同时检查 `App.tsx:52-55` 的 tab 白名单（不含 `monitor`，见 [[android-frontend-core]] 的 Known Issues 第 14 条）与 `MobileMore.tsx:36-39` 的派生降级。