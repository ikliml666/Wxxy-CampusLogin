---
title: frontend-settings-module
type: module
source_files:
  - android/frontend/src/settings/constants.ts
  - android/frontend/src/settings/index.ts
  - android/frontend/src/settings/KeepAliveSettingsCard.tsx
  - android/frontend/src/settings/OnboardingWizard.tsx
  - android/frontend/src/settings/ThemeDialog.tsx
tags: [settings, onboarding, theme, keep-alive, configuration]
---

## Overview

`frontend-settings-module` 是前端应用的核心配置与用户引导模块，主要面向 Android 平台（同时保留桌面/平板适配）。它负责管理应用程序的全局状态（包括网络、安全、主题、自动化策略），提供首次安装后的新手指引（Onboarding Wizard），以及处理平台特定的保活机制（Android Keep-Alive）。

该模块通过 `useConfigStore` 和 `useThemeStore` 集中管理配置数据，确保 UI 组件与后端逻辑的一致性。它支持双端适配：在桌面/平板端使用对话框形态的向导，在移动端使用全屏分步向导。所有公共 API 统一由 `index.ts` 导出。

## Key Components

### Constants & Types (`constants.ts`)

**常量定义**:
*   `DEFAULT_CONFIG: Config` (5| L5-L58): 完整的默认配置对象，作为配置的“单一事实来源”。
    *   **网络/连接**: `portalUrl: 'http://10.1.99.100'`, `fixedGateway: '10.2.127.254'`, `requiredNetworkName: 'i-wxxy'` (41| L41-L43)。
    *   **保活/后台**: `backgroundCheckInterval: 60000` (60s), `backgroundCheckIdleInterval: 300000` (5min) (26| L26, 28| L28)。注释明确指出这些值与后端对齐，且自 2026-09-09 起对齐后端稳态周期降功耗。
    *   **时间策略**: `campusCheckStartMinutes: 460` (07:40), `campusCheckEndMinutes: 1380` (23:00, 旧默认0已迁移) (47| L47, 49| L49)。
    *   **Schema版本**: `configSchemaVersion: 7` (57| L57)，用于配置迁移版本控制。
*   `ISP_OPTIONS` (60| L60-L65): 运营商选项数组。包含 `__default__` (无锡学院)、`@telecom`、`@unicom`、`@cmcc`。
*   `operatorLabelKey(suffix)` (72| L72-L75): 工具函数，将运营商后缀映射为 i18n key。处理空字符串到 `__default__` 的转换，返回 `undefined` 表示离线/未知。
*   `THEME_OPTIONS` (77| L77-L85): 主题选项，包含 `default`, `vibrant`, `forest`, `midnight`, `ocean`, `cherry`, `custom` 及其对应的色值。
*   `VALID_THEMES` (87| L87): 有效主题 ID 数组。
*   `DEFAULT_PANEL_OPTIONS` (89| L89-L92): 基于 `NAV_ITEMS` 生成的默认面板选项。

### Hooks (`useSettings.ts`, `useOnboardingFlow.ts`)

**`useSettings`** (useSettings.ts 实现, index.ts 5| L5 导出):
*   **聚合 Store**: 合并 `configStore` (config, updateConfig, saveConfigDirect, passwordSaved, syncPasswordSaved), `themeStore` (themeName, isLightMode, setThemeName, setIsLightMode, initTheme, setCustomThemeColor), `logToastStore` (addToast)。
*   **处理函数**:
    *   `handleToggleLightMode` (36| L36-L42): 切换 `isLightMode`，更新 config `themeMode` ('light'|'dark')，持久化 `safeStorage.set('campus-light-mode', ...)`。
    *   `handleToggleNotification` (44| L44-L50): 取反 `enableNotification`，调用 `api.setNotificationEnabled`，若开启则请求系统权限 `requestNotificationPermission`。
    *   `handleSetAutoLaunch` (52| L52-L60): 更新 `enableBootAutostart`，调用 `api.setAutoLaunch`。**Android 特异性处理**：若 API 失败（注册表未生效），抛出 Error Toast。
    *   `handleSetTheme` (62| L62-L65): 更新 themeName，持久化 `safeStorage.set('campus-theme', name)`。

**`useOnboardingFlow`** (useOnboardingFlow.ts 实现, index.ts 未直接导出但被 Wizard 组件使用):
*   **常量**:
    *   `ONBOARDING_STEP_COUNT = 4` (26| L26): 欢迎 → 账号 → 绑定运营商 → 完成。
    *   `BIND_OPERATOR_NONE = '__none__'` (28| L28), `DEFAULT_OPERATOR = '__default__'` (30| L30)。
    *   `BIND_SUCCESS_ADVANCE_MS = 1200` (32| L32), `LOGIN_SUCCESS_ADVANCE_MS = 1500` (34| L34)。
    *   `OPERATOR_KEY_BY_VALUE` (47| L47-L51): ISP 值到后端键的映射。
    *   `OPERATOR_STATUS_ORDER` (53| L53): 展示顺序 ['telecom', 'unicom', 'cmcc']。
*   **状态管理**:
    *   **账号步**: `username`, `password`, `operator`, `showPassword`。密码处理：若 `config.password === PASSWORD_MASK` 则清空输入框但保留掩码态判断。
    *   **绑定步**: `bindOperatorValue`, `phone`, `smsPassword`, `bindState` ('idle'|'loading'|'success'|'error'), `bindStatuses` (三个运营商的状态查询结果)。
    *   **流程控制**: `step`, `direction`, `loginSuccess`, `showCloseConfirm`。
*   **关键逻辑**:
    *   **重置机制** (102| L102-L118): 当 `open` 从 false 变 true 时，重置所有表单状态为当前 config 值。
    *   **绑定校验** (`canBind`, 133| L133-L137): 需用户名、密码(或掩码)、选定运营商、有效手机号(正则 `/^1\d{10}$/`)、短信验证码。
    *   **绑定操作** (`handleBind`, 139| L139-L170):
        *   前置检查 `ensureBindHello()` (142| L142) 确保生物识别验证通过。
        *   调用 `tauriApiWithRetry.bindOperator` (145| L145)。
        *   成功后刷新 `bindRefreshTick` 触发状态重查，延时 1.2s 进入完成步。
    *   **绑定状态自动检测** (178| L178-L204): 进入 Step 2 且账号有效时，自动调用 `tauriApiWithRetry.getBindStatus` (183| L183) 查询三运营商绑定情况。**非阻塞性**：此过程不经过 Windows Hello 验证，以避免打断用户引导体验。支持取消监听防止竞态。
    *   **账号下一步** (`handleNext`, 207| L207-L221): 仅当 Step 1 时，校验账号密码必填，更新 user/operator/password (若非空才覆盖，保护已保存密码)。
    *   **完成登录** (`handleLoginAndFinish`, 236| L236-L266):
        *   最终校验账号密码存在性 (238| L238)。
        *   更新配置后调用 `onLogin()`。
        *   成功后标记 `loginSuccess=true`，延时 1.5s 写入 `safeStorage.set('campus-onboarding-done', '1')` 并关闭向导。
    *   **跳过** (`handleSkip`, 227| L227-L234): 清除定时器，写入 done 标记，关闭向导。
*   **返回对象**: 包含所有派生状态、表单 setter、操作函数 (`advance`, `goNext`, `handleSkip`, `handleLoginAndFinish`) (271| L271-L291)。

### UI Components

**`SettingsPanel`** (SettingsPanel.tsx, index.ts 1| L1 导出):
*   **职责**: 安卓端主设置面板 UI 组件，负责渲染启动、自动化、保活、安全、网络五大分组配置项。采用两列网格布局（md 起），包含新手指引入口、延迟测试弹窗及看板娘装饰。
*   **平台差异化门控**:
    *   `isAndroid = import.meta.env.VITE_PLATFORM === 'android'` (29| L29): 用于条件渲染 Windows 专属设置（如退出应用等不渲染）及安卓专属保活卡片。
*   **状态订阅优化**:
    *   使用 `useConfigStore(useShallow((s) => s.config))` (45| L45) 浅比较订阅 config，防止 App 外壳因非相关字段变化级联重渲染。
*   **核心交互逻辑**:
    *   **安全开关降级保护** (`handleSecurityDisable`, 56| L56-L70): 关闭 `selfHelloEnabled` 或 `selfReverifyEachAction` 时，必须调用 `tauriApiWithRetry.verifyWindowsIdentity` (58| L58) 通过 Windows Hello 验证。失败则 Toast 报错并回弹 Switch；成功则执行 `apply(true)` 更新配置。
    *   **通知权限联动** (159| L159-L163): 开启通知时，除更新配置外，异步调用 `requestNotificationPermission({ openSettingsIfDenied: true })` (162| L162)，处理 Android 13+ 权限请求及被拒后跳转设置页。
    *   **自动登录时机**:
        *   开机自启：`onSetAutoLaunch` (133| L133) 直接回调父组件处理。
        *   启动自动登录：`autoLoginOnStart` (146| L146)。
        *   准备就绪自动登录：`autoLoginOnPreparation` (194| L194)。
    *   **夜间运营商切换**: `enableNightOperatorSwitch` (207| L207)。
    *   **保活设置**: 仅当 `isAndroid` 为真时渲染 `<KeepAliveSettingsCard />` (220| L220)。
    *   **2D 人脸风险确认**:
        *   开启时弹出 `ConfirmDialog` (321| L321-L330) 告知风险，确认后 `setFaceRiskConfirmOpen(false)` 并更新配置 (327| L327)。
        *   关闭时立即调用 `clearTemplate()` (289| L289) 清除本地人脸模板，并更新配置。
        *   若已开启且 `hasTemplate()` (297| L297) 为真，显示“已录入”状态及重新录入按钮；否则显示“未录入”。
    *   **网络质量检测联动关闭** (357| L357-L375):
        *   关闭 `enableNetworkQuality` 时，若 `config.enableLatencyTest` 为真，同时置空 `enableLatencyTest` 并调用 `tauriApiWithRetry.stopLatencyTest?.()` (370| L370) 停止后端任务。
        *   若当前 `activePanel === 'quality'`，强制切换至 `dashboard` (374| L374) 避免空白面板。
        *   若 `defaultPanel === 'quality'`，重置默认面板 (372| L372)。
    *   **延迟计算选项弹窗**: 通过 `latencyModalOpen` (49| L49) 控制 `<Dialog>` (393| L393)，内含 `skipTtfbInLatency` (408| L408) 和 `skipContentInLatency` (421| L421) 开关及解释说明。

**`KeepAliveSettingsCard`** (KeepAliveSettingsCard.tsx, index.ts 未直接导出但在 SettingsPanel 中使用):
*   **职责**: Android 专属的保活设置卡片。管理电池优化白名单状态查询、申请跳转及厂商特定设置页跳转。
*   **关键实现**:
    *   **首次提示记忆**: 使用 `safeStorage` 键 `PROMPTED_KEY = 'campus-keepalive-prompted'` (23| L23)。仅在首次进入时弹出确认框，拒绝后保留手动入口 (43| L43-L46)。
    *   **状态刷新**: `refresh()` 调用 `api.getBatteryOptimizationInfo()` (32| L32-L38)。
    *   **交互流程**:
        *   用户点击“请求” -> 打开 `ConfirmDialog` (130| L130-L136)。
        *   确认后调用 `api.requestIgnoreBatteryOptimizations()` (52| L52)。失败则 Toast 报错 (54| L54)。
        *   操作后延迟 1.5s 重查状态以应对系统页返回事件不触发的问题 (59| L59)。
        *   厂商设置跳转：调用 `api.openVendorBatterySettings()`，若返回 `path === 'none'` 或异常，则 Toast 报错 (63| L63-L71)。
    *   **UI 逻辑**: 根据 `info?.ignoring` 显示 Badge 状态 (73| L73, 95| L95)。仅当未忽略时显示“请求”按钮 (103| L103-L108)。仅当 `info?.hasVendorTarget` 存在时显示厂商设置按钮 (111| L111-L125)。
    *   **平台限制说明**: 注释指出此组件不参与双端同步，文案风格统一但逻辑安卓专属 (5| L5-L7)。

**`OnboardingWizard`** (OnboardingWizard.tsx, index.ts 2| L2 导出):
*   **职责**: 平板/桌面端的新手指引向导（Dialog 形态）。复用 `useOnboardingFlow` 逻辑，独立处理 UI 布局与动画。
*   **Props**: `open`, `onClose`, `onUpdateConfig`, `onLogin`, `isLoggingIn` (36| L36-L42)。
*   **步骤定义**: 4步 (`STEP_COUNT = 4`)。图标序列 `STEP_ICONS` [Sparkles, UserCircle, Link2, Zap] (44| L44-L45, 75| L75)。
*   **动画系统**: 使用 `framer-motion`。卡片进出场变体 `cardVariants` 包含旋转、缩放、透明度变化 (56| L56-L72)。过渡参数 `CARD_TRANSITION` (73| L73)。
*   **Step 0 (欢迎)**: 展示 MascotFigure，语言切换按钮 (zh/en) (170| L170-L191)。
*   **Step 1 (账号信息)**:
    *   输入 Username/Password，支持密码显隐 (193| L193-L257)。
    *   运营商选择器 `Select`，数据源 `ISP_OPTIONS` (244| L244-L253)。
    *   校验：用户名/密码非空高亮错误 (198| L198-L213)。
*   **Step 2 (绑定运营商)**:
    *   复用登录凭据提示 (264| L264-L267)。
    *   运营商选择：过滤掉 `DEFAULT_OPERATOR` (275| L275-L278)。
    *   绑定状态面板：自动查询三个运营商状态 (`bindStatuses`)，支持手动刷新 (283| L283-L347)。
    *   聚焦逻辑：选中运营商后放大显示详情 (309| L309-L342)。
    *   手机号/SMS 密码输入，正则校验手机号 (349| L349-L376)。
    *   绑定按钮状态：loading/success/disabled (382| L382-L395)。
*   **Step 3 (完成)**: 展示总结信息（用户名、密码掩码、运营商），提供“开始登录”按钮 (400| L400-L436)。
*   **导航与退出**: 底部导航箭头，跳过按钮带二次确认 `showCloseConfirm` (453| L453-L527)。

**`OnboardingWizardMobile`** (OnboardingWizardMobile.tsx, index.ts 3| L3 导出):
*   **职责**: 手机端新手指引（全屏分步向导）。解决窄屏下 Dialog 挤压问题，适配触控与大安全区。
*   **布局差异**: 全屏铺满 `fixed inset-0`，内容区独立滚动 `overflow-y-auto`，底部操作区固定 `shrink-0` (142| L142-L172)。
*   **进度指示**: 自定义 `StepTrack` 段式进度轨，比圆点更省空间 (73| L73-L95)。
*   **触控优化**: NavArrow 尺寸放大至 48px (h-12 w-12)，输入框高度统一 `h-12` (112| L112, 135| L135)。移除 `autoFocus` 防止键盘遮挡 (10| L10-L11)。
*   **逻辑复用**: 完全复用 `useOnboardingFlow` (131| L131)。
*   **UI 结构**:
    *   Header：跳过按钮 + 进度轨 + 步序 (152| L152-L169)。
    *   Main：堆叠卡片装饰层 + `AnimatePresence` 动画卡片 (173| L173-L491)。
    *   Footer：导航箭头 + 最后一步登录按钮 (494| L494-L536)。
    *   跳过确认：全屏遮罩上的轻量卡片，而非 Dialog 组件 (539| L539-L582)。
*   **数据映射**: 内部定义 `OPERATOR_VALUE_BY_KEY` 用于状态面板反向映射 (57| L57-L61)。

**`ThemeDialog`** (ThemeDialog.tsx, index.ts 2| L2 导出):
*   **职责**: 主题选择对话框，提供预设色板选择和明暗模式切换。
*   **Props**: `open`, `onClose`, `onSetTheme(name: ThemeName)`, `onToggleLightMode`。
*   **色板渲染**: 遍历 `THEME_OPTIONS` (14| L14, 45| L45)，高亮选中项，点击触发 `onSetTheme`。
*   **明暗切换**: Switch 绑定 `useThemeStore.isLightMode`，切换调用 `onToggleLightMode`。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                      App Shell / Entry                      │
└──────────────────┬──────────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────────┐
│                   Settings Panel (UI)                       │
│  ┌──────────────┐ ┌──────────────┐ ┌────────────────────┐  │
│  │ Startup/     │ │ Automation   │ │ Security           │  │
│  │ Notification │ │ Group        │ │ (Hello/Face)       │  │
│  └──────────────┘ └──────────────┘ └────────────────────┘  │
│  ┌──────────────┐ ┌──────────────┐                         │
│  │ Network      │ │ Keep-Alive   │ (Android Only)         │  │
│  │ Quality      │ │ Card         │                         │  │
│  └──────────────┘ └──────────────┘                         │
└──────────────────┬──────────────────────────────────────────┘
                   │ Updates Config via onUpdateConfig
                   ▼
┌─────────────────────────────────────────────────────────────┐
│                   State Management                          │
│  ┌──────────────────┐  ┌──────────────────┐                │
│  │ useConfigStore   │  │ useThemeStore    │                │
│  │ (Zustand)        │  │ (Zustand)        │                │
│  └────────┬─────────┘  └────────┬─────────┘                │
└───────────┼────────────────────┼───────────────────────────┘
            │                    │
            ▼                    ▼
┌─────────────────────────────────────────────────────────────┐
│                   Onboarding Flow                           │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              useOnboardingFlow Hook                  │   │
│  │  ├─ Step 0: Welcome                                │   │
│  │  ├─ Step 1: Account Info (Validates canProceed)     │   │
│  │  ├─ Step 2: Bind Operator (Validates canBind)       │   │
│  │  └─ Step 3: Finish & Login                         │   │
│  └──────────────────────────────────────────────────────┘   │
│           ▲                  ▲                              │
│           │                  │                              │
│  ┌────────┴────────┐  ┌─────┴──────────────┐               │
│  │ OnboardingWizard│  │ OnboardingWizardMobile│             │
│  │ (Desktop/Tablet)│  │ (Android Fullscreen) │             │
│  └─────────────────┘  └──────────────────────┘               │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **初始化**:
    *   App 启动时，`InitData` 从后端加载，包含 `Config` 快照。
    *   `useConfigStore` 和 `useThemeStore` 初始化状态。
    *   检查 `safeStorage` 中的 `campus-onboarding-done` 标志，决定是否显示 `OnboardingWizard`。

2.  **设置修改**:
    *   用户在 `SettingsPanel` 中触发 Switch/Button。
    *   调用 `onUpdateConfig(partial)` 或直接调用 `useSettings` 中的 handlers。
    *   `useConfigStore.updateConfig` 更新 Zustand store。
    *   副作用触发：如 `requestNotificationPermission` (通知), `tauriApiWithRetry.setAutoLaunch` (自启), `clearTemplate` (人脸清除)。

3.  **新手指引**:
    *   用户进入向导，`useOnboardingFlow` 重置状态。
    *   **Step 1**: 输入账号密码 -> `handleNext` 校验 `canProceedAccount` -> 更新 `user/operator/password` 到 Store。
    *   **Step 2**: 输入绑定信息 -> `handleBind` 调用 Tauri API `bindOperator` -> 成功后延时跳转 Step 3。
    *   **Step 3**: 点击登录 -> `handleLoginAndFinish` 校验账号 -> 调用 `onLogin` -> 成功后设置 `campus-onboarding-done` 并关闭向导。

4.  **保活机制 (Android)**:
    *   `KeepAliveSettingsCard` 挂载时读取电池优化状态。
    *   若未豁免，用户点击申请 -> 弹出确认框 -> 调用 `requestIgnoreBatteryOptimizations` -> 延迟刷新状态。

## Connections

*   [[use-config-store]]: `SettingsPanel` 和 `useOnboardingFlow` 通过此 Store 读写全局配置。
*   [[use-theme-store]]: `ThemeDialog` 和 `useSettings` 通过此 Store 管理主题和明暗模式。
*   [[tauri-api]]: `useOnboardingFlow` (绑定/登录), `KeepAliveSettingsCard` (电池优化), `useSettings` (自启/通知) 均依赖 Tauri API 与后端交互。
*   [[face-service]]: `SettingsPanel` 中的 2D 人脸录入/清除直接调用 `faceService` 模块。
*   [[notification-permission]]: `SettingsPanel` 和 `useSettings` 在开启通知时调用此模块请求系统权限。

## Known Issues

*   **保活卡片首次提示**: `KeepAliveSettingsCard` 使用 `safeStorage` 键 `campus-keepalive-prompted` (23| L23) 记忆首次提示，但若无清理机制，用户可能永久无法再次看到初始引导，仅能手动进入。
*   **网络质量联动清理**: `SettingsPanel` 中关闭 `enableNetworkQuality` 时，若 `config.enableLatencyTest` 为真，会同时置空 `enableLatencyTest` 并停止后端任务 (357| L357-L375)。此逻辑假设两者强耦合，若未来设计解耦，需调整。
*   **移动端键盘遮挡**: `OnboardingWizardMobile` 移除了 `autoFocus` (10| L10-L11) 以防止键盘遮挡，但这可能导致用户需要手动点击输入框才能唤起键盘，影响流畅度。