---
title: frontend-settings-onboarding
type: module
source_files:
  - android/frontend/src/settings/constants.ts
  - android/frontend/src/settings/index.ts
  - android/frontend/src/settings/KeepAliveSettingsCard.tsx
  - android/frontend/src/settings/OnboardingWizard.tsx
  - android/frontend/src/settings/ThemeDialog.tsx
  - android/frontend/src/settings/types.ts
  - android/frontend/src/settings/useOnboardingFlow.ts
  - android/frontend/src/settings/useSettings.ts
  - android/frontend/src/settings/SettingsPanel.tsx
tags: [settings, onboarding, theme, ui, android]
---

## Overview
本模块负责 Android 端应用的全局设置面板（`SettingsPanel`）、首次使用引导向导（`OnboardingWizard` / `OnboardingWizardMobile`）及主题配置对话框（`ThemeDialog`）。核心目标是为安卓端提供跨设备（手机/平板）一致的引导体验，同时管理应用的全局配置、网络质量检测参数、安全验证开关及电池保活策略。

模块通过 `useOnboardingFlow` Hook 抽离了向导的状态机逻辑，确保手机端全屏向导与平板端对话框向导共享同一套业务逻辑。设置面板采用响应式布局，适配不同屏幕尺寸，并针对 Android 平台提供了特定的保活与安全功能。所有 UI 组件均通过 `index.ts` 统一导出。

## Key Components

### Constants & Types (`constants.ts`, `types.ts`)

**常量定义 (`android/frontend/src/settings/constants.ts`)**

*   **`DEFAULT_CONFIG: Config` (5-58)**: 默认配置对象，作为单一事实来源（Single Source of Truth），确保 UI 展示值与后端配置结构一致。
    *   `configSchemaVersion: 7` (57): 配置版本迁移标记，注释提及 v4→v5, v5→6 迁移历史。
    *   `backgroundCheckInterval: 60000` (26) & `backgroundCheckIdleInterval: 300000` (28): 后台检测周期，与后端 `Settings::default` 对齐，用于降功耗。
    *   `latencyTestInterval: 600000` (35): 延迟测试间隔 600s。
    *   `campusCheckStartMinutes: 460` (47) / `EndMinutes: 1380` (49): 校园网检查时间窗口（约 07:40 - 23:00）。
    *   `portalUrl: 'http://10.1.99.100'` (41), `fixedGateway: '10.2.127.254'` (42), `requiredNetworkName: 'i-wxxy'` (43): 硬编码的校园网网络参数。
    *   `enableBootAutostart: false` (23): 注释说明需调用 `set_boot_autostart` 才生效。
*   **`ISP_OPTIONS` (60-65)**: 运营商选项数组。特殊值 `__default__` 对应“无锡学院”（校园网默认线路），其余为 `@telecom`, `@unicom`, `@cmcc`。
*   **`operatorLabelKey` (72-75)**: 工具函数，将后端返回的运营商后缀（如 `''` 或 `'@telecom'`）转换为 i18n 的 `labelKey`。空字符串 `''` 映射为 `__default__` 查找；`null/undefined` 返回 `undefined`（用于隐藏徽标）。
*   **`THEME_OPTIONS` (77-85)**: 主题选项，包含 `id` (ThemeName), `label`, `labelKey`, `color`。包含 `custom` 主题，颜色由 `DEFAULT_CONFIG.customThemeColor` (`#6366f1`) 提供。
*   **`VALID_THEMES` (87)**: 所有有效主题 ID 的数组，用于校验。
*   **`DEFAULT_PANEL_OPTIONS` (89-92)**: 基于 `NAV_ITEMS` 动态生成的默认面板选项，仅提取 `value` 和 `labelKey`。

**类型定义 (`android/frontend/src/settings/types.ts`)**

*   **`AccountItem` (4-13)**: 账号列表项接口。
    *   `id`: string, 文件名 stem。
    *   `displayName`: string, 可编辑显示名。
    *   `user?`: string, 档案内保存的学号。
    *   `operator?`: string, 运营商后缀。
    *   `passwordSaved?`: boolean, 是否已保存密码。
*   **`Config` (15-80)**: 核心配置对象。
    *   **安全相关**: `selfHelloEnabled` (L19), `selfReverifyEachAction` (L21), `allow2dFaceVerify` (L23)。
    *   **运营商相关**: `operator`, `enableNightOperatorSwitch`, `nightOperatorRestore`, `enableNightOutboundSwitch`, `outboundPriority`, `outboundMetricRestore`, `outboundDisabledAdapters` (JSON string), `outboundStandbyRoute` (JSON string), `nightOutboundRestore`, `dnsOptimizeAdapters`。
        *   *注*: `outboundDisabledAdapters` 和 `outboundStandbyRoute` 在安卓端仅同构不消费 (L32-L35)。
    *   **自动化相关**: `autoLoginOnStart`, `enableBootAutostart` (L43), `backgroundCheckInterval`, `backgroundCheckIdleInterval` (L47, 默认 300000ms), `autoLoginOnPreparation`, `scheduledLoginMinutes`, `scheduledLogoutMinutes`。
    *   **网络相关**: `enableLatencyTest`, `latencyTestInterval`, `defaultPanel`, `enableNetworkQuality`, `skipTtfbInLatency` (L57), `skipContentInLatency` (L58), `portalUrl`, `fixedGateway`, `requiredNetworkName`, `enableNetworkNameCheck`, `campusGateway`, `updateSource` ('mirror'|'github'), `campusCheckStartMinutes` (默认 460=07:40), `campusCheckEndMinutes` (默认 1380=23:00)。
    *   **其他**: `themeMode` ('light'|'dark'|'system'), `customThemeColor`, `displayName` (当前激活账号显示名), `logRetentionDays`, `configSchemaVersion`。
*   **`InitData` (87-103)**: 初始化数据接口。包含 `accounts`, `adapterDetails`, `logRetentionDays`, `debugMode` 等。注释说明 `logRetentionDays` 和 `debugMode` 原为一次性命令，现并入 InitData。

### Hooks (`useOnboardingFlow.ts`, `useSettings.ts`)

**`useOnboardingFlow` (`android/frontend/src/settings/useOnboardingFlow.ts`)**

*   **导出常量**:
    *   `ONBOARDING_STEP_COUNT = 4` (26)。
    *   `BIND_OPERATOR_NONE = '__none__'` (28): 绑定步骤未选择运营商。
    *   `DEFAULT_OPERATOR = '__default__'` (30): 账号步骤默认运营商。
    *   `BIND_SUCCESS_ADVANCE_MS = 1200` (32), `LOGIN_SUCCESS_ADVANCE_MS = 1500` (34): 停留时长。
*   **运营商映射**:
    *   `OPERATOR_KEY_BY_VALUE`: `'@telecom'->'telecom'`, `'@unicom'->'unicom'`, `'@cmcc'->'cmcc'` (47-51)。
    *   `OPERATOR_STATUS_ORDER`: `['telecom', 'unicom', 'cmcc']` (53)。
*   **`BindState`**: `'idle' | 'loading' | 'success' | 'error'` (36)。
*   **`OperatorBindingInfo`**: `{ account: string, passwordSet: boolean }` (40-43)。
*   **`BindStatuses`**: `Record<OperatorKey, OperatorBindingInfo | null>` (44)。

*   **`useOnboardingFlow` (63-291)**: 向导核心状态机 Hook。
    *   **输入**: `open`, `onUpdateConfig`, `onLogin`, `onClose`。
    *   **状态**:
        *   `step` (70): 当前步骤 (0-3)。
        *   `username`, `password`, `operator` (71-73): 登录凭据。
        *   `bindOperatorValue`, `phone`, `smsPassword` (79-81): 绑定运营商信息。
        *   `bindState` (82): 'idle' | 'loading' | 'success' | 'error'。
        *   `loginSuccess` (75): 登录成功标志。
    *   **方法**:
        *   `advance(nextStep)` (127-130): 切换步骤并记录方向。
        *   `handleBind()` (139-170): 执行运营商绑定，调用 `tauriApiWithRetry.bindOperator`。前置验证调用 `ensureBindHello()` (142)。成功处理设置 `bindState='success'`，刷新绑定状态检测 tick (`setBindRefreshTick`)，延迟 1.2s 后进入完成步 (157-161)。
        *   `goNext()` (223-225): 进入下一步，若为第 1 步则落盘账号信息。**关键纪律**：仅当用户输入了新密码 (`password.trim()`) 时才更新 `password` 字段，避免空字符串覆盖后端已保存的掩码密码 (215-217)。
        *   `handleSkip()` (227-234): 跳过向导，直接写入 `campus-onboarding-done` 并关闭。
        *   `handleLoginAndFinish()` (236-266): 执行登录并结束向导。**最终校验**：再次检查账号密码有效性 (238)，修复历史缺陷。登录成功后，设置 `loginSuccess=true`，写入 `safeStorage.set('campus-onboarding-done', '1')` (257)，延迟 1.5s 后关闭向导 (258)。
    *   **派生值**:
        *   `canProceedAccount` (172): 账号步骤是否可继续。
        *   `canBind` (133): 绑定步骤是否可提交。
        *   `passwordSaved` (269): 密码是否已保存。
    *   **绑定状态自动检测 (178-204)**: 进入绑定步骤 (step 2) 且账号有效时，自动调用 `tauriApiWithRetry.getBindStatus` (183)。结果缓存到 `bindStatuses`。支持通过 `bindRefreshTick` 手动刷新。

**`useSettings` (`android/frontend/src/settings/useSettings.ts`)**

*   **`useSettings` (10-63)**: 设置相关副作用 Hook。整合 Config Store、Theme Store、Toast Store。
    *   `handleToggleLightMode` (36-42): 切换亮/暗模式。切换 `useThemeStore.isLightMode`，更新 `useConfigStore.themeMode` 为 `'light'` 或 `'dark'`，持久化到 `safeStorage` (`campus-light-mode`)。
    *   `handleToggleNotification` (44-50): 切换通知权限。反转 `enableNotification` 配置，调用 `api.setNotificationEnabled?.(next)` (47)。开启时调用 `requestNotificationPermission` (49)。
    *   `handleSetAutoLaunch` (52-60): 设置开机自启。更新 `enableBootAutostart` 配置 (54)，调用 `api.setAutoLaunch?.(enabled)` (56)。**注意**：代码注释指出安卓后端 Settings 无 `autoLaunch` 字段，真实生效字段为 `enableBootAutostart`，但此处仍调用 `setAutoLaunch` API（可能用于桌面兼容或特定安卓实现），失败时提示 Toast (58)。
    *   `handleSetTheme` (62-65): 设置主题名称。调用 `store.setThemeName`，持久化到 `safeStorage` (`campus-theme`)。

### UI Components

**`OnboardingWizard` (`android/frontend/src/settings/OnboardingWizard.tsx`)**

*   **用途**: 平板/宽屏端的向导组件，以 Dialog 形式呈现。
*   **Props** (34-40):
    *   `open`: boolean
    *   `onClose`: () => void
    *   `onUpdateConfig`: (partial: Partial<import('@/settings').Config>) => void
    *   `onLogin`: () => Promise<boolean>
    *   `isLoggingIn`: boolean
*   **子组件**:
    *   `DeckArrow` (77-100): 导航箭头按钮。
*   **渲染逻辑** (202-623):
    *   Step 0: 欢迎页，显示吉祥物和安全提示，支持语言切换 (`setLanguage`)。
    *   Step 1: 账号信息页，输入用户名、密码（支持显隐），选择运营商。必填项校验实时反馈。
    *   Step 2: 绑定运营商页，复用 Step 1 的凭据。ISP 下拉框过滤掉 `DEFAULT_OPERATOR`。绑定状态面板自动查询三个运营商绑定状态 (`bindStatuses`)。选中运营商后高亮聚焦该行 (`focusedOperatorKey`)。手机号、短信验证码输入。绑定按钮状态机：loading/success/error。
    *   Step 3: 完成页，展示摘要信息（用户名、密码掩码、运营商），提供“开始登录”按钮。
*   **动画**: 使用 `framer-motion` 类似的手动 transform 实现卡片堆叠进出场效果 (`STACK_LAYERS`, `deckTransform`, 55-73)。前进左出，后退右出。
*   **导航**: 底部圆形箭头按钮，步骤指示器。
*   **关闭确认**: `showCloseConfirm` 状态控制是否弹出“跳过设置”确认 Dialog。

**`OnboardingWizardMobile` (`android/frontend/src/settings/OnboardingWizardMobile.tsx`)**

*   **用途**: 手机端的新手指引，全屏分步向导。
*   **Props** (48-54): 同 `OnboardingWizard`。
*   **结构**:
    *   Header (152-169): 跳过按钮 + 段式进度轨 (`StepTrack`) + 步序文本。
    *   Main (172-491): 滚动内容区，包含各步骤表单。
    *   Footer (494-536): 固定底部操作区，包含上一步/下一步/开始登录按钮。
    *   Skip Confirm (539-582): 全屏遮罩确认跳过。
*   **差异点**:
    *   布局：`fixed inset-0` 全屏铺满，非 Dialog。顶部 Header + 中间滚动 Content + 底部固定 Footer (142, 172, 494)。
    *   安全区：Header 和 Footer 使用 `calc(env(safe-area-inset-top/bottom) + ...)` 适配刘海/手势条 (154, 496)。
    *   触控目标：按钮和输入框高度增加至 48px (`h-12`, 136, 112)，图标增大。
    *   进度指示：使用 `StepTrack` 组件（段式进度轨）替代圆点，更节省横向空间 (73-95)。
    *   跳过确认：使用自定义的 `AnimatePresence` 遮罩层而非桌面 `Dialog` 组件，避免尺寸错位 (539-582)。
    *   输入体验：移除 `autoFocus`，避免进入账号步骤时键盘遮挡表单。
*   **逻辑复用**: 完全依赖 `useOnboardingFlow` (131)，确保双端流程一致。

**`SettingsPanel` (`android/frontend/src/settings/SettingsPanel.tsx`)**

*   **用途**: 主设置面板容器。负责渲染启动、自动化、保活、安全、网络五大分组设置项，处理跨平台差异（Android vs Desktop），管理局部状态弹窗（人脸风险、延迟测试选项）。
*   **Props** (30-34):
    *   `onUpdateConfig`: (partial: Partial<Config>) => void
    *   `onSetAutoLaunch`: (enabled: boolean) => Promise<void>
    *   `onShowOnboarding?`: () => void
*   **区域划分**:
    *   **新手指引卡片** (46-65): 仅当 `onShowOnboarding` 存在时显示。
    *   **启动与通知** (73-105): 开机自启、自动登录、通知开关。
    *   **自动化** (109-133): 准备就绪自动登录 (`autoLoginOnPreparation`, 194)、夜间运营商切换 (`enableNightOperatorSwitch`, 207)。
    *   **保活设置** (137-143): 仅安卓 (`isAndroid`) 渲染 `KeepAliveSettingsCard`。
    *   **安全设置** (149-206): Windows Hello (`selfHelloEnabled`, 252)、二次验证 (`selfReverifyEachAction`, 269)、2D 人脸 (`allow2dFaceVerify`, 286-L293)。
        *   `allow2dFaceVerify`: 开启前弹出 `ConfirmDialog` (321-330) 告知风险。关闭时立即调用 `clearTemplate()` (289) 清除本地人脸模板，并更新配置。若已开启且录入过人脸 (`hasTemplate()`)，显示“重新录入”按钮，否则显示“录入”按钮 (300-312)。
    *   **网络设置** (210-233): 质量检测开关 (`enableNetworkQuality`, 357-L375)、延迟计算选项入口。
    *   **延迟计算选项弹窗** (237-285): 包含 TTFB 和内容传输跳过开关 (`skipTtfbInLatency`, 408; `skipContentInLatency`, 421)。
    *   **背景看板娘** (290-297): 装饰性图片。根据 `useDailyMascots([2])` (42) 获取随机 mascot ID，渲染低透明度图片 (442-449)。
*   **平台差异化门控**: `isAndroid = import.meta.env.VITE_PLATFORM === 'android'` (29)。仅当为 Android 时渲染 `KeepAliveSettingsCard` (217-L223)，桌面端不渲染退出应用/托盘等 Windows 专属项。
*   **配置订阅优化**: 使用 `useConfigStore(useShallow((s) => s.config))` (45) 浅比较订阅 config，避免 App 外壳因非相关字段变化级联重渲染。
*   **安全开关验证逻辑 (`handleSecurityDisable`, 56-L70)**: 关闭“Windows Hello”或“二次验证”时，必须先调用 `tauriApiWithRetry.verifyWindowsIdentity` (58) 进行生物识别验证。验证成功则执行 `apply(true)` 更新配置；失败则通过 `addToast` 报错，Switch 受控自动回弹。捕获异常时显示 `biometricFailMessage` (68)。
*   **通知权限联动**: 开启通知开关时 (162)，触发 `requestNotificationPermission({ openSettingsIfDenied: true })`，针对 Android 13+ 请求运行时权限，被拒则跳转系统设置。
*   **开机自启联动**: `onSetAutoLaunch` (133) 直接调用父组件传入的回调，不在此处修改 store。对应后端命令为 `set_boot_autostart`，由 `useSettings` 中的 `handleSetAutoLaunch` 统一处理。
*   **网络组与质量检测联动**: 关闭 `enableNetworkQuality` (357-L375) 时，不仅更新配置，还执行清理逻辑：若 `config.enableLatencyTest` 为真，同步关闭定时测试 (`patch.enableLatencyTest = false`) 并调用 `tauriApiWithRetry.stopLatencyTest?.()` (370)。若 `config.defaultPanel === 'quality'`，重置默认面板为空 (372)。若当前活跃面板为 `quality`，强制切换至 `dashboard` (374)。

**`KeepAliveSettingsCard` (`android/frontend/src/settings/KeepAliveSettingsCard.tsx`)**

*   **用途**: 安卓专属保活设置卡片。处理电池优化白名单状态读取、用户引导申请及厂商自定义省电页跳转。
*   **逻辑**:
    *   首次进入弹出确认框 (`ConfirmDialog`)，写入 `campus-keepalive-prompted` 标记 (23, 43-46)。
    *   请求忽略电池优化 (`api.requestIgnoreBatteryOptimizations`, 49-61)。成功后 1.5s 延迟刷新状态（处理系统页返回不触发事件的问题，59）。失败则通过 Toast 报错。
    *   跳转厂商特定设置页 (`api.openVendorBatterySettings`, 63-71)。若返回 `path === 'none'` 或异常，提示跳转失败。
    *   根据 `ignoring` 状态显示不同 Badge（成功/受限）。仅在 `!ignoring` 时显示“申请”按钮。若 `hasVendorTarget` 为真，显示厂商设置跳转入口。
    *   开发环境日志：关键错误仅在 `import.meta.env.DEV` 下 `console.error`，生产环境静默 (36, 55, 69)。

**`ThemeDialog` (`android/frontend/src/settings/ThemeDialog.tsx`)**

*   **用途**: 主题配置对话框。
*   **Props** (20-25):
    *   `open`, `onClose`, `onSetTheme`, `onToggleLightMode`。
*   **功能**: 选择主题色块，切换亮/暗模式开关。遍历 `THEME_OPTIONS` (14, 45) 渲染网格按钮 (44-67)。选中态样式：`border-primary bg-primary/5 text-primary` (54)。明暗模式切换显示图标随模式变化：亮色 `Sparkles`，暗色 `Moon` (73)。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                      App Shell                              │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                 SettingsPanel                         │  │
│  │  ┌─────────────┐ ┌──────────────┐ ┌───────────────┐  │  │
│  │  │ Startup     │ │ Automation   │ │ Security      │  │  │
│  │  │ & Notif.    │ │              │ │               │  │  │
│  │  └─────────────┘ └──────────────┘ └───────────────┘  │  │
│  │  ┌─────────────┐ ┌──────────────┐                     │  │
│  │  │ KeepAlive   │ │ Network      │                     │  │
│  │  │ (Android)   │ │ Quality      │                     │  │
│  │  └─────────────┘ └──────────────┘                     │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              Onboarding Flow                          │  │
│  │  ┌──────────────┐       ┌──────────────────────┐     │  │
│  │  │ useOnboarding│◄─────►│ OnboardingWizard     │     │  │
│  │  │ Flow (Logic) │       │ (Desktop/Tablet)     │     │  │
│  │  └──────────────┘       └──────────────────────┘     │  │
│  │                     ┌──────────────────────┐         │  │
│  │                     │ OnboardingWizardMobile│         │  │
│  │                     │ (Phone)              │         │  │
│  │                     └──────────────────────┘         │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                 ThemeDialog                           │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **初始化**:
    *   `App` 加载 `init_data`，存入 `useConfigStore` 和 `useThemeStore`。
    *   `SettingsPanel` 订阅 `configStore.config`。

2.  **向导流程 (Onboarding)**:
    *   用户点击“新手指引”或首次启动触发 `OnboardingWizard` / `OnboardingWizardMobile`。
    *   `useOnboardingFlow` 初始化状态：从 `configStore` 读取当前 `user`, `password`, `operator`。
    *   用户在 Step 1 输入凭据，调用 `handleNext` 更新 `configStore` (仅草稿状态，未最终落盘直到 Step 3 或登录)。
    *   用户在 Step 2 绑定运营商，调用 `handleBind` 请求后端 API，成功后自动进入 Step 3。
    *   用户在 Step 3 点击“开始登录”，调用 `onLogin` (由父组件传入，通常是 `do_login` 命令)。
    *   登录成功后，`handleLoginAndFinish` 写入 `campus-onboarding-done` 标记并关闭向导。

3.  **设置变更**:
    *   `SettingsPanel` 中的 Switch/Button 直接调用 `onUpdateConfig` 或 `onSetAutoLaunch`。
    *   `onUpdateConfig` 更新 `useConfigStore`，并持久化到文件系统。
    *   `handleSetAutoLaunch` 额外调用 Tauri API 注册系统自启。
    *   `handleSetTheme` 更新 `useThemeStore` 并写入本地存储。

4.  **保活策略**:
    *   `KeepAliveSettingsCard` 在挂载时检查电池优化状态。
    *   若未请求过，弹出 `ConfirmDialog`。
    *   用户确认后调用 `requestIgnoreBatteryOptimizations` 请求系统白名单。

## Connections
*   [[frontend-account]]: 账号管理模块，`OnboardingWizard` 复用其登录逻辑。
*   [[frontend-monitor]]: 监控模块，`SettingsPanel` 中的网络质量设置影响后台检测任务。
*   [[shared-ui]]: 共享 UI 组件库，`SettingsPanel` 和向导均使用其中的 Card, Button, Switch 等。
*   [[tauri-api]]: Tauri 后端接口，用于执行登录、绑定运营商、设置自启等操作。

## Known Issues
1.  **安卓自启字段映射**: 在 `useSettings.ts` L52-60 中，前端配置项 `enableBootAutostart` 直接映射到后端 API `setAutoLaunch`。注释指出后端实有字段为 `enable_boot_autostart`，但 API 调用可能依赖特定的 Tauri 命令封装。需确保 Tauri 侧命令正确解析此布尔值。
2.  **向导状态重置**: `useOnboardingFlow` L102-118 仅在 `open` 从 false 变为 true 时重置状态。如果向导关闭后再次打开，且期间 `config` 发生了变化（如在其他设置页修改了密码），向导会重新读取最新配置。但如果用户在向导中途关闭，下次打开时应从 Step 0 重新开始，当前逻辑符合预期。
3.  **2D 人脸模板清除**: `SettingsPanel.tsx` L289 在关闭 2D 人脸开关时调用 `clearTemplate()`。这会导致用户必须重新录入人脸才能再次开启，属于设计上的安全最小化原则，非 Bug，但需注意用户体验。
4.  **延迟计算选项联动**: `SettingsPanel.tsx` L357-L375 在关闭 `enableNetworkQuality` 时，会联动关闭 `enableLatencyTest` 并停止测试任务。这是为了防止开关与任务分叉，逻辑正确。