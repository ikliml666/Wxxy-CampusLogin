---
title: frontend-settings-module
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
tags: [settings, onboarding, theme, keep-alive, configuration]
---

## Overview

`frontend-settings-module` 是前端应用的核心配置与用户引导模块。它负责管理应用程序的全局状态（包括网络、安全、主题、自动化策略），提供首次安装后的新手指引（Onboarding Wizard），以及处理平台特定的保活机制（Android Keep-Alive）。

该模块通过 `useConfigStore` 和 `useThemeStore` 集中管理配置数据，确保 UI 组件与后端逻辑的一致性。它支持双端适配：在桌面/平板端使用对话框形态的向导，在移动端使用全屏分步向导。

## Key Components

### Constants & Types (`constants.ts`, `types.ts`)

**常量定义**:
- `DEFAULT_CONFIG`: 定义 `Config` 接口的默认值。包含所有设置项的初始状态，如 `autoLoginOnStart` (true), `themeMode` ('dark'), `backgroundCheckInterval` (60000ms) 等。
- `ISP_OPTIONS`: 运营商选项列表，包含 `__default__` (无锡学院), `@telecom`, `@unicom`, `@cmcc`。
- `operatorLabelKey`: 将运营商后缀映射为 i18n labelKey 的工具函数。
- `THEME_OPTIONS`: 主题选项列表，包含 'default', 'vibrant', 'forest', 'midnight', 'ocean', 'cherry', 'custom' 及其对应的颜色代码。
- `VALID_THEMES`: 合法主题名称数组。
- `DEFAULT_PANEL_OPTIONS`: 基于 `NAV_ITEMS` 生成的默认面板选项。

**类型定义**:
- `AccountItem` (types.ts): 账号列表项，包含 `id` 和 `displayName`。
- `Config` (types.ts): 核心配置接口。关键字段包括：
    - `selfHelloEnabled`: Windows Hello 验证开关。
    - `allow2dFaceVerify`: 2D 人脸验证开关。
    - `enableNightOperatorSwitch`: 夜间运营商自动切换开关。
    - `autoLoginOnStart`: 开机自启登录。
    - `enableBootAutostart`: 安卓开机自启注册。
    - `backgroundCheckInterval`: 稳态周期检测间隔 (60s)。
    - `latencyTestInterval`: 延迟测试间隔 (600s)。
    - `configSchemaVersion`: 配置 Schema 版本 (当前为 7)。
- `InitData` (types.ts): 初始化数据结构，包含 config, version, adapters, accounts 等。

### Hooks (`useSettings.ts`, `useOnboardingFlow.ts`)

**`useSettings`** (index.ts 导出, useSettings.ts 实现):
- `handleToggleLightMode`: 切换明暗模式，更新 `themeMode` 并持久化到 `safeStorage`。
- `handleToggleNotification`: 切换通知权限，请求系统授权。
- `handleSetAutoLaunch`: 设置开机自启，调用 Tauri API `setAutoLaunch`。
- `handleSetTheme`: 设置主题名称并持久化。

**`useOnboardingFlow`** (useOnboardingFlow.ts 实现):
- **状态变量**:
    - `step`: 当前步骤 (0-3)。
    - `username`, `password`, `operator`: 账号凭据。
    - `bindOperatorValue`, `phone`, `smsPassword`: 绑定运营商所需信息。
    - `bindState`: 绑定状态 ('idle', 'loading', 'success', 'error')。
    - `loginSuccess`: 登录成功标志。
- **核心方法**:
    - `advance`: 切换步骤方向。
    - `handleBind`: 执行运营商绑定，调用 `tauriApiWithRetry.bindOperator`。需通过 `ensureBindHello` 验证。
    - `goNext`: 进入下一步，先校验账号信息并落盘。
    - `handleSkip`: 跳过向导，标记 `campus-onboarding-done` 并关闭。
    - `handleLoginAndFinish`: 最终登录流程，校验账号，调用 `onLogin`，成功后标记完成并关闭。
    - `canProceedAccount` (派生属性): 账号步骤是否可继续（用户名非空且密码有效）。
    - `canBind` (派生属性): 绑定步骤是否可提交。

### UI Components

**`SettingsPanel`** (SettingsPanel.tsx):
- **功能**: 主设置面板，展示各类配置开关。
- **结构**:
    - **启动与通知组**: 包含 `autoLaunch` (开机自启), `autoLoginCampus` (开机自登录), `enableNotification` (通知)。
    - **自动化组**: 包含 `autoLoginWhenReady` (准备就绪自动登录), `nightOperatorSwitch` (夜间运营商切换)。
    - **保活设置卡片**: 仅在 Android 平台渲染 (`isAndroid && <KeepAliveSettingsCard />`)。
    - **安全设置组**: 包含 `selfHello` (Windows Hello), `selfReverify` (二次验证), `allow2dFace` (2D 人脸)。关闭安全开关需通过 Hello 验证。
    - **网络质量组**: 包含 `enableQualityDetection` (质量检测开关), `latencyCalcOptions` (延迟计算选项入口)。
    - **延迟计算弹窗**: 显示 `skipTtfb` 和 `skipContent` 选项及说明。
- **特殊逻辑**:
    - 关闭 `enableNetworkQuality` 时联动关闭 `enableLatencyTest` 并清理面板引用。
    - 关闭 `selfHello` 或 `selfReverify` 时需调用 `handleSecurityDisable` 进行生物识别验证。

**`KeepAliveSettingsCard`** (KeepAliveSettingsCard.tsx):
- **功能**: Android 专属电池优化白名单申请与厂商设置跳转。
- **逻辑**:
    - 首次进入弹出确认框 (`ConfirmDialog`)，仅提示一次 (通过 `safeStorage` 键 `campus-keepalive-prompted` 记忆)。
    - 获取电池优化状态 `getBatteryOptimizationInfo`。
    - 点击“申请”按钮调用 `requestIgnoreBatteryOptimizations`。
    - 若有厂商特定目标，提供跳转按钮 `openVendorBatterySettings`。

**`OnboardingWizard`** (OnboardingWizard.tsx):
- **功能**: 平板/桌面端的新手指引对话框。
- **步骤**:
    0. 欢迎页 (MascotFigure, 语言切换)。
    1. 账号信息 (Username, Password, Operator)。
    2. 绑定运营商 (Phone, SMS Password, ISP Select)。
    3. 完成页 (Summary, Login Button)。
- **动画**: 使用 `framer-motion` 实现步骤间滑动过渡。

**`OnboardingWizardMobile`** (OnboardingWizardMobile.tsx):
- **功能**: Android 手机端全屏新手指引。
- **差异**:
    - 全屏布局，独立滚动内容区，固定底部操作栏。
    - 使用段式进度轨 (`StepTrack`) 替代圆点指示器。
    - 触控目标放大 (h-12)，移除 autoFocus 以避免键盘遮挡。
    - 复用 `useOnboardingFlow` 逻辑。

**`ThemeDialog`** (ThemeDialog.tsx):
- **功能**: 主题选择对话框。
- **内容**: 网格展示 7 种预设主题色，明暗模式切换开关。

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