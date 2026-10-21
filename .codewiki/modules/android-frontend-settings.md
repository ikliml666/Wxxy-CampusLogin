---
title: android-frontend-settings
type: module
source_files:
  - android/frontend/src/settings/useOnboardingFlow.ts
  - android/frontend/src/settings/useSettings.ts
  - android/frontend/src/settings/constants.ts
  - android/frontend/src/settings/index.ts
  - android/frontend/src/settings/KeepAliveSettingsCard.tsx
  - android/frontend/src/settings/OnboardingWizard.tsx
  - android/frontend/src/settings/ThemeDialog.tsx
  - android/frontend/src/settings/types.ts
tags: [android, settings, onboarding, tauri, zustand]
---

## Overview

本模块 `android-frontend-settings` 负责 Android 客户端（基于 Tauri）的设置面板、新手引导向导、常亮保活卡片及主题管理。核心职责包括：
1.  **设置聚合**：通过 `useSettings` Hook 聚合配置、主题和 Toast 状态，提供统一的设置处理回调。
2.  **新手引导 (Onboarding)**：封装 4 步状态机逻辑，解决安卓端手机/平板 UI 差异，确保流程步骤与后端契约一致。移除网络适配器选择步骤。
3.  **平台差异化功能**：包含安卓专属的电池优化白名单申请（`KeepAliveSettingsCard`）、Windows Hello 安全验证集成、通知权限请求等。
4.  **常量与类型定义**：集中管理默认配置、运营商选项、主题列表及玻璃质感等级。

## Key Components

### Constants & Types (`constants.ts`, `types.ts`)

#### `constants.ts`
*   **`DEFAULT_CONFIG`**: 全局配置默认值对象。
    *   `configSchemaVersion: 7` (L57)：当前配置版本，注释提及 v4→v5, v5→v6 迁移逻辑。
    *   `backgroundCheckInterval: 60000` (L26)：稳态周期检测间隔，对齐后端。
    *   `backgroundCheckIdleInterval: 300000` (L28)：闲时巡检间隔（5分钟）。
    *   `latencyTestInterval: 600000` (L35)：延迟测试间隔（10分钟）。
    *   `campusCheckEndMinutes: 1380` (L49)：校园网检测结束时间（23:00），旧默认 0。
    *   `scheduledLoginMinutes: 1440` (L51)：定时登录时刻，1440 表示禁用/全天有效。
*   **`ISP_OPTIONS`**: 数组，定义运营商选项（无锡学院 `__default__`, 电信 `@telecom` 等）。
*   **`operatorLabelKey`**: 函数 (L72-75)，将运营商后缀转换为 i18n labelKey，处理空值与默认值映射。
*   **`THEME_OPTIONS`**: 数组，定义 7 种主题（default, vibrant, forest, midnight, ocean, cherry, custom）及其颜色代码。
*   **`VALID_THEMES`**: 数组，提取所有合法主题 ID。
*   **`GLASS_OPTIONS`**: 数组，定义三档玻璃质感（off, standard, ultra）。
*   **`VALID_GLASS_LEVELS`**: 数组，提取所有合法玻璃等级。
*   **`DEFAULT_PANEL_OPTIONS`**: 计算属性，基于 `NAV_ITEMS` 生成默认面板选项。

#### `types.ts`
*   **`AccountItem`**: 账号列表项接口 (L4-13)。
    *   字段：`id`, `displayName`, `user`, `operator`, `passwordSaved`。
*   **`Config`**: 核心配置接口 (L15-80)。
    *   关键字段：`selfHelloEnabled`, `allow2dFaceVerify`, `enableNightOperatorSwitch`, `autoLoginOnStart`, `backgroundCheckInterval`, `latencyTestInterval`, `configSchemaVersion` 等。
*   **`AutoLaunchResult`**: 开机自启结果接口 (L82-85)。
*   **`InitData`**: 初始化数据接口，合并了原分散的配置、适配器、账号、状态等信息 (L87-103)。

### Hooks

#### `useOnboardingFlow.ts`
*   **职责**：封装新手指引（Onboarding）的 4 步状态机逻辑。
*   **关键常量与导出**：
    *   `ONBOARDING_STEP_COUNT = 4`：步骤总数（欢迎、账号、绑定运营商、完成）[行:26]。
    *   `BIND_OPERATOR_NONE = '__none__'`：绑定下拉未选择哨兵 [行:28]。
    *   `DEFAULT_OPERATOR = '__default__'`：账号步骤默认运营商哨信 [行:30]。
    *   `BIND_SUCCESS_ADVANCE_MS = 1200` / `LOGIN_SUCCESS_ADVANCE_MS = 1500`：成功提示停留时长 [行:32, 34]。
    *   `OperatorKey` (`cmcc`, `telecom`, `unicom`) 及 `BindStatuses` 类型定义 [行:39-44]。
    *   `OPERATOR_KEY_BY_VALUE`：映射 UI 值到后端键 [行:47-51]。
    *   `OPERATOR_STATUS_ORDER`：展示顺序 [行:53]。
    *   `OnboardingFlowOptions` 接口：包含 `open`, `onUpdateConfig`, `onLogin`, `onClose`, `onAutoAdvance` [行:55-67]。
*   **核心方法/状态**：
    *   **初始化重置**：当 `open` 从 false 变 true 时，重置所有表单字段和绑定状态为当前配置或空值 [行:108-124]。清理定时器防泄漏 [行:126-131]。
    *   **密码掩码态处理**：若 `config.password === PASSWORD_MASK`，视为已保存掩码态，后端回退使用 [行:78, 140, 186]。仅当用户显式输入新密码时才更新 `password` 字段 [行:225-227, 257-259]。
    *   **绑定验证门**：绑定操作前需通过 `ensureBindHello()` (来自 `useHelloGate`) 验证，防止后端拦截 [行:101, 148]。
    *   **自动检测绑定状态**：进入 Step 2 且账号有效时，自动调用 `tauriApiWithRetry.getBindStatus` 查询三个运营商的绑定情况 [行:188-214]。取消机制依赖 `cancelled` 标志 [行:190, 213]。
    *   **最终校验修复**：`handleLoginAndFinish` 中重新校验账号密码非空，修复历史缺陷“完成步不重校验” [行:246-252]。
    *   **返回值结构**：返回大量派生状态、setter 和控制函数（`step`, `advance`, `goNext`, `handleBind`, `handleLoginAndFinish` 等）[行:281-301]。

#### `useSettings.ts`
*   **职责**：设置页的全局逻辑控制器，聚合配置、主题、Toast 存储，并提供具体的设置处理回调。
*   **关键实现**：
    *   **Store 聚合**：通过 `useShallow` 浅比较订阅 `useConfigStore`, `useThemeStore`, `useLogToastStore` 的关键切片 [行:13-31]。合并为 `store` 对象供内部使用 [行:32]。
    *   **`handleToggleLightMode`**：同步更新主题 Store、Config Store 的 `themeMode` 以及本地存储 `campus-light-mode` [行:36-42]。
    *   **`handleToggleNotification`**：先更新 Config，再调用 API `setNotificationEnabled`。开启时需请求系统权限（Android 13+ 弹框，低版本/被拒跳转设置页）[行:44-50]。
    *   **`handleSetAutoLaunch`**：注意安卓后端字段名为 `enableBootAutostart` 而非 `autoLaunch` [行:52-60]。API 失败时通过 Toast 提示用户。
    *   **`handleSetTheme`**：更新主题名并持久化到 `campus-theme` [行:62-65]。

### Components

#### SettingsPanel.tsx
*   **职责**：主设置面板，组织多个功能卡片，处理平台差异化渲染（安卓专属保活卡片）。
*   **关键实现**：
    *   `isAndroid`: 常量，判断是否为安卓平台。
    *   **`handleSecurityDisable`**: 关闭安全开关前需通过 Windows Hello 验证 (`tauriApiWithRetry.verifyWindowsIdentity`)。
    *   **布局**: 两列网格 (md 起)，左列含启动/通知、自动化、保活；右列含安全、网络质量。
    *   **新手指引卡片**: 仅在 `onShowOnboarding` 存在时渲染。
    *   **启动与通知卡片**: 开机自启 (`onSetAutoLaunch`)、自动登录校园网、通知权限 (`requestNotificationPermission`)。
    *   **自动化卡片**: 准备就绪自动登录、夜间运营商切换。
    *   **保活卡片**: 仅安卓平台渲染 `KeepAliveSettingsCard`。
    *   **安全卡片**:
        *   Windows Hello 总开关 (`selfHelloEnabled`)。
        *   每次操作二次验证 (`selfReverifyEachAction`)，依赖 Hello 开关。
        *   2D 人脸验证 (`allow2dFaceVerify`)，开启需风险确认 (`faceRiskConfirmOpen`)，关闭时清除模板 (`clearTemplate`)。提供录入按钮。
    *   **网络质量卡片**:
        *   总开关 (`enableNetworkQuality`)。关闭时联动停止延迟测试、清理面板引用、切换回 Dashboard。
        *   入口按钮打开延迟计算选项 Modal。
    *   **延迟计算选项 Modal**: 包含跳过 TTFB、跳过内容传输两个开关及解释说明。
    *   **背景看板娘**: 底部装饰图片。

#### `OnboardingWizard.tsx`
*   **职责**：平板/宽屏下的新手引导向导（Dialog 形态），复用 `useOnboardingFlow` Hook 处理核心逻辑。
*   **关键实现**：
    *   `STEP_COUNT`: 4 步 (L43)。
    *   `STACK_LAYERS`: 定义卡片堆叠的层级样式（y, scale, z-index）(L55-60)。
    *   `EXIT_UP_MS`: 出场动画时长 240ms (L63)。
    *   `DeckArrow`: 子组件，前后导航箭头按钮。
    *   **`deckAdvanceRef`**: Ref 指向 `deckAdvance` 函数，供 Hook 在绑定成功后自动推进步骤时使用 (L111-112)。
    *   **`measureDeck`**: 测量所有步骤内容高度，取最大值作为统一卡高，防止切步时高度跳动 (L138-158)。使用 `useLayoutEffect` 确保绘制前完成 (L162-168)。
    *   **`deckAdvance`**: 前进时触发出场动画（飞起 -> 延迟 -> 切步 -> 入场浮起）；后退直接切步 (L178-194)。
    *   **`deckGoNext`**: 校验通过后调用 `deckAdvance(step + 1)` (L198-205)。
    *   **步骤内容**：
        *   Step 0 (欢迎): 吉祥物、语言切换按钮 (L279-302)。
        *   Step 1 (账号信息): 用户名、密码（显隐切换）、运营商选择 (L304-367)。
        *   Step 2 (绑定运营商): 显示各运营商绑定状态胶囊、输入手机号和短信验证码进行绑定 (L369-501)。包含重试、加载、成功状态。
        *   Step 3 (完成): 展示摘要信息，提供“开始登录”按钮 (L503-594)。
    *   **关闭确认**: 点击遮罩或关闭按钮时弹出二次确认框，询问是否跳过设置 (L605-625)。

#### `KeepAliveSettingsCard.tsx`
*   **职责**：安卓专属保活设置卡片。管理电池优化白名单状态查询、申请、厂商特定设置跳转。
*   **关键实现**：
    *   `PROMPTED_KEY`: 常量 `'campus-keepalive-prompted'` (L23)，用于本地记忆首次提示。
    *   **`refresh`**: 调用 `api.getBatteryOptimizationInfo()` 获取状态 (L34)。
    *   **`useEffect` (L40-47)**: 初始化时刷新状态；若未提示过 (`!safeStorage.get(PROMPTED_KEY)`)，打开确认弹窗并标记已提示。
    *   **`handleConfirmRequest`**: 请求忽略电池优化 (`api.requestIgnoreBatteryOptimizations`) (L52)，失败则 Toast 错误，成功或返回后 1.5s 延迟刷新状态 (L59)。
    *   **`handleOpenVendor`**: 打开厂商电池设置页 (`api.openVendorBatterySettings`)，若返回路径为 `'none'` 则报错 (L65-66)。
    *   **UI 渲染**: 根据 `info?.ignoring` 显示徽章状态；若 `!ignoring` 显示“申请”按钮；若 `info?.hasVendorTarget` 显示厂商设置跳转按钮。
    *   **`ConfirmDialog`**: 用于首次引导用户授权。

#### `ThemeDialog.tsx`
*   **职责**：主题设置对话框，提供颜色主题、玻璃质感、明暗模式切换。
*   **关键实现**：
    *   `GLASS_ICONS`: 映射玻璃等级到图标组件 (L21)。
    *   **颜色主题**: 网格展示 `THEME_OPTIONS`，选中态高亮 (L49-72)。
    *   **玻璃质感**: 网格展示 `GLASS_OPTIONS`，选中态高亮，下方预留高度显示描述文案以防抖动 (L76-112)。
    *   **明暗模式**: Switch 切换 `isLightMode` (L114-123)。

#### `index.ts`
*   **职责**：模块入口，统一导出所有组件、Hook 和类型/常量。
*   **关键导出**：
    *   `SettingsPanel` (L1)
    *   `ThemeDialog` (L2)
    *   `OnboardingWizard` (L3)
    *   `useSettings` (L4)
    *   `types` (L6)
    *   `constants` (L7)

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                      Settings Module                           │
│                                                              │
│  ┌──────────────┐    ┌──────────────────┐    ┌─────────────┐ │
│  │  SettingsPanel│    │ OnboardingWizard │    │ThemeDialog  │ │
│  │ (Main Layout) │    │ (Step Wizard)    │    │             │ │
│  └──────┬───────┘    └────────┬─────────┘    └──────┬──────┘ │
│         │                     │                      │       │
│         │                     ▼                      │       │
│         │            ┌─────────────────┐             │       │
│         │            │ useOnboardingFlow│             │       │
│         │            │ (State Machine)  │             │       │
│         │            └────────┬────────┘             │       │
│         │                     │                      │       │
│         ▼                     ▼                      ▼       │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                  useSettings Hook                         │ │
│  │  (Aggregates Config, Theme, Toast Stores)                 │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │               External Dependencies                       │ │
│  │  • useConfigStore (Tauri Config)                          │ │
│  │  • useThemeStore (Local Storage)                          │ │
│  │  • api (Tauri Native APIs: Battery, Notification, etc.)   │ │
│  │  • safeStorage (Local Persistence)                        │ │
│  │  • useHelloGate (Security Verification)                   │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **配置更新流**:
    *   `SettingsPanel` 接收 `onUpdateConfig` prop。
    *   用户交互（Switch/Button）触发 `onUpdateConfig(partialConfig)`。
    *   `useConfigStore` 响应变化，驱动 UI 更新。
    *   `constants.ts` 中的 `DEFAULT_CONFIG` 提供初始值或迁移参考。
2.  **主题与状态**:
    *   `ThemeDialog` 直接订阅 `useThemeStore`，独立于 `Config` 存储（可能存储在另一处或同步到 Config）。
    *   `KeepAliveSettingsCard` 通过 `api` (Tauri API) 直接与原生层交互获取电池优化状态，不经过 Redux/Zustand 配置中心。
3.  **新手引导流**:
    *   `OnboardingWizard` 内部使用 `useOnboardingFlow` Hook (import 引用 L32-32)。
    *   `useOnboardingFlow` 管理 4 步状态机，通过 `onUpdateConfig` 暂存用户输入，通过 `onLogin` 触发最终登录。
    *   绑定步骤中，`useOnboardingFlow` 调用 `tauriApiWithRetry.bindOperator` 执行绑定，并通过 `ensureBindHello` 进行前置验证。
4.  **平台差异流**:
    *   `SettingsPanel` 通过 `isAndroid` 常量决定渲染 `KeepAliveSettingsCard`。
    *   `useSettings` 中 `handleSetAutoLaunch` 明确区分了配置字段 `enableBootAutostart` 和 API 方法 `setAutoLaunch`，注释指出安卓后端无 `autoLaunch` 字段 (L53-54)。

## Connections

*   [[android-frontend-config]]: `useConfigStore` 和 `constants.ts` 中的默认配置与此模块紧密相关。
*   [[android-frontend-security]]: `SettingsPanel` 中的 Windows Hello 验证和 2D 人脸验证逻辑与此模块的安全设置部分关联。
*   [[android-frontend-api]]: `KeepAliveSettingsCard` 和 `OnboardingWizard` 调用的 Tauri API (电池优化、绑定运营商等) 与此模块的后端接口定义关联。

## Known Issues

*   **密码掩码态处理**: 前端显示逻辑：若 `config.password === PASSWORD_MASK`，输入框置空但允许提交 [行:78, 112, 186]。后端收到空密码时，若配置中存在掩码态，则使用已保存密码 [行:12, 140, 186]。更新纪律：仅当用户显式输入新密码时才更新 `password` 字段，避免空字符串覆盖已保存值 [行:225-227, 257-259]。
*   **绑定步骤的 Hello 门控**: 绑定操作会修改运营商账号，后端有 TTL 验证门。前端通过 `ensureBindHello` 提前验证，避免操作中途被后端拒绝导致体验中断 [行:99-101, 148]。状态检测（`getBindStatus`）不挂 Hello 门，因为只是查询，且进步骤才弹 Hello 会打断引导流程 [行:93]。
*   **定时器清理**: `bindTimerRef` 和 `finishTimerRef` 必须在组件卸载或状态变更时清除，防止内存泄漏或无效回调 [行:126-131, 162-163, 238-241, 265-266]。
*   **异步取消**: `getBindStatus` 请求使用 `cancelled` 标志位处理组件卸载或快速切换步骤时的竞态条件 [行:190, 195, 207, 213]。
*   **安卓特定逻辑**:
    *   `useSettings` 中 `handleSetAutoLaunch` 明确区分了配置字段 `enableBootAutostart` 和 API 方法 `setAutoLaunch`，注释指出安卓后端无 `autoLaunch` 字段 [行:53-54]。
    *   通知权限请求针对 Android 13+ 及以上版本的不同行为进行处理 [行:48-49]。
*   **UI 动画解耦**: `useOnboardingFlow` 提供 `onAutoAdvance` 钩子，允许宿主（如平板 Dialog 或手机全屏）自定义切步动画，避免原生 `setStep` 导致的瞬间闪现 [行:62-66, 164-169]。
*   **默认运营商处理**: `DEFAULT_OPERATOR` (`__default__`) 在落盘配置时会被转换为空字符串 `''` [行:222, 255]，保持配置整洁。
*   **保活设置**:
    *   **首次提示记忆**: 使用 `safeStorage` 记录 `PROMPTED_KEY`，避免反复弹窗 (L43-45)。
    *   **系统页返回**: 用户可能在系统设置页停留，因此 `handleConfirmRequest` 中使用 `setTimeout` 延迟刷新状态，而非依赖生命周期钩子 (L59)。
    *   **厂商支持检查**: 仅当 `info?.hasVendorTarget` 为真时显示厂商设置按钮 (L111)。
*   **新手引导**:
    *   **卡片高度测量**: 使用 `useLayoutEffect` 和 `requestAnimationFrame` 双重保障测量最大高度，防止内容变化导致布局抖动 (L162-168)。
    *   **步进动画**: 前进时先出场再切步再入场，确保视觉连贯；后退直接切步 (L178-194)。
    *   **关闭确认**: 防止误触关闭，提供二次确认 (L605-625)。
    *   **绑定状态**: 即使查询失败也渲染胶囊，保留手动选择入口，避免流程卡死 (L395-400)。
*   **安全设置**:
    *   **Windows Hello 验证**: 关闭安全相关开关前必须验证身份，防止绕过界面直接修改 (L56-70, L253, L270)。
    *   **2D 人脸风险**: 开启前需确认风险 (`faceRiskConfirmOpen`) (L287, L321-330)。
    *   **模板清除**: 关闭 2D 人脸验证时立即清除本地人脸模板，符合数据最小化原则 (L289)。
*   **网络质量**:
    *   **联动清理**: 关闭质量检测时，不仅关闭开关，还联动停止后台测试任务、清理默认面板引用、切换活跃面板，防止界面残留空白 (L357-375)。
*   **配置版本**:
    *   `configSchemaVersion: 7` (L57)，暗示存在前端配置迁移逻辑，需与后端 schema 保持一致。