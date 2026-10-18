---
title: frontend-settings-onboarding
type: module
source_files:
  - android/frontend/src/settings/constants.ts
  - android/frontend/src/settings/index.ts
  - android/frontend/src/settings/KeepAliveSettingsCard.tsx
  - android/frontend/src/settings/OnboardingWizard.tsx
  - android/frontend/src/settings/OnboardingWizardMobile.tsx
  - android/frontend/src/settings/SettingsPanel.tsx
  - android/frontend/src/settings/ThemeDialog.tsx
  - android/frontend/src/settings/types.ts
  - android/frontend/src/settings/useOnboardingFlow.ts
  - android/frontend/src/settings/useSettings.ts
tags: [settings, onboarding, theme, ui, android]
---

## Overview
该模块负责应用设置面板（Settings Panel）、首次使用引导向导（Onboarding Wizard）及主题配置对话框（Theme Dialog）。核心目标是为安卓端提供跨设备（手机/平板）一致的引导体验，同时管理应用的全局配置、网络质量检测参数、安全验证开关及电池保活策略。

模块通过 `useOnboardingFlow` Hook 抽离了向导的状态机逻辑，确保手机端全屏向导与平板端对话框向导共享同一套业务逻辑。设置面板采用响应式网格布局，适配不同屏幕尺寸。

## Key Components

### Constants & Types (`constants.ts`, `types.ts`)

**常量定义 (`constants.ts`)**
*   **`DEFAULT_CONFIG` (L10-75)**: 默认配置对象，包含所有配置字段的初始值。
    *   `configSchemaVersion`: L74, 值为 7。
    *   `backgroundCheckInterval`: L36, 60000ms。
    *   `backgroundCheckIdleInterval`: L40, 300000ms。
    *   `latencyTestInterval`: L52, 600000ms。
    *   `campusCheckEndMinutes`: L68, 1380 (23:00)。
    *   `scheduledLoginMinutes`: L71, 1440 (禁用)。
    *   `scheduledLogoutMinutes`: L72, 1440 (禁用)。
*   **`ISP_OPTIONS` (L77-82)**: 运营商选项列表，包含无锡学院、电信、联通、移动。
*   **`operatorLabelKey` (L91-95)**: 根据运营商后缀获取 i18n labelKey 的辅助函数。
*   **`THEME_OPTIONS` (L97-105)**: 主题选项列表，包含 default, vibrant, forest, midnight, ocean, cherry, custom。
*   **`VALID_THEMES` (L107)**: 有效主题名称数组。
*   **`DEFAULT_PANEL_OPTIONS` (L109-111)**: 默认面板选项，基于 `NAV_ITEMS` 生成。

**类型定义 (`types.ts`)**
*   **`AccountItem` (L3-13)**: 账号列表项接口。
    *   `id`: string, 账号文件名 stem。
    *   `displayName`: string, 显示名。
    *   `user?`: string, 档案内保存的学号。
    *   `operator?`: string, 档案内保存的运营商后缀。
    *   `passwordSaved?`: boolean, 是否已保存密码。
*   **`Config` (L15-95)**: 全局配置接口。
    *   包含用户凭据、生物识别开关、网络出口策略、定时任务、日志保留等字段。
    *   `selfHelloEnabled`: L20, Windows Hello 总开关。
    *   `allow2dFaceVerify`: L24, 2D 人脸验证开关。
    *   `enableBootAutostart`: L38, 开机自启。
    *   `themeMode`: L44, 'light' | 'dark' | 'system'。
    *   `defaultPanel`: L48, 默认面板名称。
    *   `updateSource`: L62, 'mirror' | 'github'。
    *   `configSchemaVersion`: L93, 配置 schema 版本。
*   **`InitData` (L97-113)**: 初始化数据接口，包含 config, adapters, accounts 等。

### Hooks (`useOnboardingFlow.ts`, `useSettings.ts`)

**`useOnboardingFlow` (`useOnboardingFlow.ts`)**
*   **导出常量**:
    *   `ONBOARDING_STEP_COUNT` (L34): 4。
    *   `BIND_OPERATOR_NONE` (L36): '__none__'。
    *   `DEFAULT_OPERATOR` (L38): '__default__'。
*   **`useOnboardingFlow` (L46-166)**: 向导核心状态机 Hook。
    *   **输入**: `open`, `onUpdateConfig`, `onLogin`, `onClose`。
    *   **状态**:
        *   `step` (L54): 当前步骤 (0-3)。
        *   `username`, `password`, `operator` (L55-57): 登录凭据。
        *   `bindOperatorValue`, `phone`, `smsPassword` (L66-68): 绑定运营商信息。
        *   `bindState` (L69): 'idle' | 'loading' | 'success' | 'error'。
        *   `loginSuccess` (L58): 登录成功标志。
    *   **方法**:
        *   `advance(nextStep)` (L85): 切换步骤并记录方向。
        *   `handleBind()` (L94-122): 执行运营商绑定，调用 `tauriApiWithRetry.bindOperator`。
        *   `goNext()` (L135): 进入下一步，若为第 1 步则落盘账号信息。
        *   `handleSkip()` (L127-133): 跳过向导，写入本地存储 `campus-onboarding-done`。
        *   `handleLoginAndFinish()` (L138-163): 执行登录并结束向导。
    *   **派生值**:
        *   `canProceedAccount` (L128): 账号步骤是否可继续。
        *   `canBind` (L90): 绑定步骤是否可提交。
        *   `passwordSaved` (L165): 密码是否已保存。

**`useSettings` (`useSettings.ts`)**
*   **`useSettings` (L10-63)**: 设置相关副作用 Hook。
    *   `handleToggleLightMode` (L28): 切换亮/暗模式。
    *   `handleToggleNotification` (L35): 切换通知权限。
    *   `handleSetAutoLaunch` (L43): 设置开机自启，调用 `api.setAutoLaunch`。
    *   `handleSetTheme` (L52): 设置主题名称。

### UI Components

**`OnboardingWizard` (`OnboardingWizard.tsx`)**
*   **用途**: 平板/宽屏端的向导组件，以 Dialog 形式呈现。
*   **Props** (L30-36):
    *   `open`: boolean
    *   `onClose`: () => void
    *   `onUpdateConfig`: (partial: Partial<Config>) => void
    *   `onLogin`: () => Promise<boolean>
    *   `isLoggingIn`: boolean
*   **子组件**:
    *   `StepIndicator` (L56-84): 步骤指示器，显示圆点和连接线。
*   **渲染逻辑** (L90-260):
    *   Step 0: 欢迎页，显示吉祥物和安全提示，支持语言切换。
    *   Step 1: 账号信息页，输入用户名、密码，选择运营商。
    *   Step 2: 绑定运营商页，输入手机号和短信密码。
    *   Step 3: 完成页，展示摘要信息，执行登录。
*   **动画**: 使用 `framer-motion` 实现步骤切换滑入滑出效果。

**`OnboardingWizardMobile` (`OnboardingWizardMobile.tsx`)**
*   **用途**: 手机端的新手指引，全屏分步向导。
*   **Props** (L53-59): 同 `OnboardingWizard`。
*   **结构**:
    *   Header (L95-108): 跳过按钮 + 段式进度轨 (`StepTrack`) + 步序文本。
    *   Main (L112-235): 滚动内容区，包含各步骤表单。
    *   Footer (L239-275): 固定底部操作区，包含上一步/下一步/开始登录按钮。
    *   Skip Confirm (L279-307): 全屏遮罩确认跳过。
*   **差异点**:
    *   触控目标放大 (h-12)。
    *   移除 `autoFocus` 避免键盘遮挡。
    *   使用 `safe-area-inset` 适配刘海屏/手势条。

**`SettingsPanel` (`SettingsPanel.tsx`)**
*   **用途**: 主设置面板容器。
*   **Props** (L30-34):
    *   `onUpdateConfig`: (partial: Partial<Config>) => void
    *   `onSetAutoLaunch`: (enabled: boolean) => Promise<void>
    *   `onShowOnboarding?`: () => void
*   **区域划分**:
    *   **新手指引卡片** (L46-65): 仅当 `onShowOnboarding` 存在时显示。
    *   **启动与通知** (L73-105): 开机自启、自动登录、通知开关。
    *   **自动化** (L109-133): 准备就绪自动登录、夜间运营商切换。
    *   **保活设置** (L137-143): 仅安卓 (`isAndroid`) 渲染 `KeepAliveSettingsCard`。
    *   **安全设置** (L149-206): Windows Hello、二次验证、2D 人脸。
    *   **网络设置** (L210-233): 质量检测开关、延迟计算选项入口。
    *   **延迟计算选项弹窗** (L237-285): 包含 TTFB 和内容传输跳过开关。
    *   **背景看板娘** (L290-297): 装饰性图片。

**`KeepAliveSettingsCard` (`KeepAliveSettingsCard.tsx`)**
*   **用途**: 安卓专属保活设置卡片。
*   **逻辑**:
    *   首次进入弹出确认框 (`ConfirmDialog`)，写入 `campus-keepalive-prompted` 标记。
    *   请求忽略电池优化 (`api.requestIgnoreBatteryOptimizations`)。
    *   跳转厂商特定设置页 (`api.openVendorBatterySettings`)。

**`ThemeDialog` (`ThemeDialog.tsx`)**
*   **用途**: 主题配置对话框。
*   **Props** (L17-21):
    *   `open`, `onClose`, `onSetTheme`, `onToggleLightMode`。
*   **功能**: 选择主题色块，切换亮/暗模式开关。

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
1.  **安卓自启字段映射**: 在 `useSettings.ts` L45 中，前端配置项 `enableBootAutostart` 直接映射到后端 API `setAutoLaunch`。注释指出后端实有字段为 `enable_boot_autostart`，但 API 调用可能依赖特定的 Tauri 命令封装。需确保 Tauri 侧命令正确解析此布尔值。
2.  **向导状态重置**: `useOnboardingFlow` L73-84 仅在 `open` 从 false 变为 true 时重置状态。如果向导关闭后再次打开，且期间 `config` 发生了变化（如在其他设置页修改了密码），向导会重新读取最新配置。但如果用户在向导中途关闭，下次打开时应从 Step 0 重新开始，当前逻辑符合预期。
3.  **2D 人脸模板清除**: `SettingsPanel.tsx` L186 在关闭 2D 人脸开关时调用 `clearTemplate()`。这会导致用户必须重新录入人脸才能再次开启，属于设计上的安全最小化原则，非 Bug，但需注意用户体验。
4.  **延迟计算选项联动**: `SettingsPanel.tsx` L220-228 在关闭 `enableNetworkQuality` 时，会联动关闭 `enableLatencyTest` 并停止测试任务。这是为了防止开关与任务分叉，逻辑正确。