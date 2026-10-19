---
type: concept
title: cross-platform-adaptive-layout
source_files:
  - android/frontend/src/main.tsx
  - android/frontend/src/App.tsx
  - android/frontend/src/components/layout/BottomNav.tsx
  - android/frontend/src/components/layout/DockNav.tsx
  - android/frontend/src/components/layout/RightPanel.tsx
  - android/frontend/src/components/mobile/MobileDashboard.tsx
  - android/frontend/src/components/mobile/MobileMore.tsx
  - android/frontend/src/hooks/useFormFactor.ts
tags:
  - responsive-design
  - form-factor
  - mobile-first
  - tablet-adaptation
---

## Overview

`cross-platform-adaptive-layout` 是 Android 前端应用的核心布局策略，旨在通过统一的代码库适配不同的设备形态（手机与平板/桌面）。该概念不依赖传统的媒体查询断点硬编码，而是基于 `useFormFactor` Hook 在运行时动态判定屏幕宽度（如 sw600dp），从而在 `AppInner`（移动端视图）和 `TabletShell`（平板端外壳）之间进行切换。

这一机制确保了 UI 组件（如导航栏、面板、向导对话框）能够根据当前设备形态渲染出最适合的交互范式：移动端采用底部标签栏 (`BottomNav`) 和全屏卡片流，而平板端则采用侧边 Dock 栏 (`DockNav`)、右侧面板 (`RightPanel`) 以及分屏或独立窗口模式。同时，它要求业务逻辑层（如认证、设置、监控）保持状态一致性，仅在不同形态下通过不同的容器组件进行包装和展示。

## Key Components

### 1. 入口与全局配置 (`android/frontend/src/main.tsx`)

**职责**：应用入口，初始化 GSAP 全局配置、主题系统、崩溃恢复机制，挂载 React 根组件。

*   **GSAP 配置** (`main.tsx|17-25`)：
    *   默认 ease 设为 `expo.out`。
    *   开启 `autoSleep: 5` (空闲 5s 后休眠时间线) 和 `nullTargetWarn: false`。
    *   监听 `prefers-reduced-motion`，若匹配则强制 `duration: 0` 并关闭 lagSmoothing。
*   **主题初始化** (`main.tsx|27-41`)：
    *   `initTheme()`：读取 `safeStorage` 中的 `campus-light-mode` 和 `campus-theme`。
    *   逻辑：`light-mode='1'` 移除 `dark` 类并设 `data-light="1"`；否则加 `dark`。
    *   若 theme 在 `VALID_THEMES` 中且非 `default`，添加 `theme-${theme}` 类。
*   **崩溃恢复** (`main.tsx|45-111`)：
    *   `setupCrashRecovery()`：
        *   **可见性检测** (`main.tsx|59-66`)：监听 `visibilitychange`，若变为 visible 且 root 无子节点，触发重载（上限 `MAX_CRASH_RELOADS=3`）。
        *   **GPU/WebGL 错误** (`main.tsx|68-74`)：监听 window error，消息含 GPU/WebGL/SharedArrayBuffer 时触发重载。
        *   **心跳机制** (`main.tsx|76-110`)：
            *   维护 `lastHeartbeatTime`。
            *   1s 间隔更新心跳（若 `isRenderLoopAlive()` 为假则跳过）(`main.tsx|90-100`)。
            *   2s 间隔检测 elapsed，若 >10000ms 视为 GPU 崩溃触发重载 (`main.tsx|102-110`)。
            *   注意：`isVisible` 状态用于暂停/恢复 `gsap.globalTimeline` (`main.tsx|77-86`)。
*   **渲染挂载** (`main.tsx|115-129`)：
    *   DEV 模式包裹 `React.StrictMode`，生产用 `React.Fragment`。
    *   结构：`ErrorBoundary` -> `LazyMotion(domMax, strict)` -> `MotionConfig(reducedMotion="user")` -> `<App />`。

### 2. 核心路由与外壳分发 (`android/frontend/src/App.tsx`)

**职责**：双外壳路由决策（手机 vs 平板），管理移动端核心状态、UI 布局及全局弹窗。

*   **外壳决策** (`App.tsx|364-368`)：
    *   `useFormFactor()` 返回 `'tablet'` 时渲染 `<TabletShell />`，否则渲染 `<AppInner />`。
    *   共享 `<FaceCaptureDialog />` 和 `<NotificationPermissionGate />`。
*   **AppInner 状态管理** (`App.tsx|52-117`)：
    *   **Tab 状态** (`App.tsx|57-62`)：从 `safeStorage` 读取 `campus-mobile-tab`，白名单校验（含 `monitor`）。
    *   **导航方向** (`App.tsx|70-77`)：`tabOrder` 数组动态生成（根据 `configEnableNetworkQuality` 决定第 4 位是 `quality` 还是 `monitor`）。计算 `navDir` (-1/0/1) 用于面板转场动画。
    *   **Store 订阅**：订阅 `useConfigStore`, `useAuthStore`, `useQualityStore`, `useMonitor`, `useAccount` 等。
    *   **面板内容映射** (`App.tsx|149-193`)：`switch (deferredTab)` 渲染对应 Panel 组件。
        *   `dashboard`: `<MobileDashboard />` + 悬浮 `<MobileQuickActions />` (`App.tsx|263-267`)。
        *   `account`: `<AccountPanel />`。
        *   `selfservice`: `<SelfServicePanel />`。
        *   `quality`: `<QualityPanel />` (条件渲染)。
        *   `monitor`: `<MonitorPanel />` (默认显示，质量开启时隐藏 tab 但此处仍保留逻辑兼容)。
        *   `more`: `<MobileMore />`。
*   **顶部 Header** (`App.tsx|202-260`)：
    *   绝对定位，`backdrop-blur-md`。
    *   包含状态点、运营商 Chip (`IspMark`)、更新胶囊、网络质量胶囊 (`NetworkQualityCapsule`)、设置/主题/关于按钮。
*   **底部 Nav** (`App.tsx|299`)：渲染 `<BottomNav />`。
*   **弹窗管理** (`App.tsx|303-356`)：
    *   Sponsor, About, UpdateAvailable, Theme, ConfirmDelete, OnboardingWizardMobile。
    *   所有弹窗均包裹在 `Suspense` 中懒加载。
*   **通知权限门控** (`App.tsx|381-405`)：
    *   `NotificationPermissionGate`：仅 Android 13+ 有效，标记 `campus-notification-prompt` 后不再询问。

### 3. 设备形态判定 Hook (`android/frontend/src/hooks/useFormFactor.ts`)

**职责与关键实现**：
*   **核心逻辑**：基于 CSS `window` 尺寸判定设备形态（Form Factor）与屏幕方向（Orientation）。
*   **判定标准**：
    *   **平板阈值**：短边（`Math.min(width, height)`）≥ 600px 视为平板，否则为手机。这对应 Android `smallestScreenWidthDp >= 600` (sw600dp) 语义，确保旋转时结论一致（手机横屏仍判为手机，平板竖屏仍判为平板）[13-17]。
    *   **方向判定**：宽 ≥ 高为横屏 (`landscape`)，否则为竖屏 (`portrait`) [20]。
*   **常量**：`TABLET_MIN_EDGE_PX = 600` [13]。
*   **类型定义**：`FormFactor = 'phone' | 'tablet'` [9]；`Orientation = 'portrait' | 'landscape'` [10]。
*   **核心 Hook**：
    *   `useViewportMeasure<T>(calc: () => T): T`：通用视口度量订阅器。内部使用 `useState` 存储计算值，通过 `useEffect` 监听 `resize` 和 `orientationchange` 事件触发重算并更新状态 [24-36]。
    *   `useFormFactor(): FormFactor`：导出 Hook，调用 `useViewportMeasure(detectFormFactor)` [38-40]。
    *   `useOrientation(): Orientation`：导出 Hook，调用 `useViewportMeasure(detectOrientation)` [43-45]。

**文件间调用关系与数据流**：
*   被 `OnboardingWizard.tsx` 和 `OnboardingWizardMobile.tsx` 间接依赖（虽然本批源码未直接展示 import 行，但根据架构惯例及后续文件逻辑，这两个 Hook 是决定渲染哪个 Wizard 组件的基础依据）。
*   数据流向：`window` 尺寸变化 → `resize/orientationchange` 事件 → `useViewportMeasure` 重算 → 返回最新 `FormFactor`/`Orientation`。

**错误处理、边界条件与非显然行为**：
*   **旋转一致性**：特意使用短边而非宽高比，防止手机横屏时被误判为平板 [4-5]。
*   **事件清理**：在 `useViewportMeasure` 的 cleanup 函数中正确移除 `resize` 和 `orientationchange` 监听器，防止内存泄漏 [30-33]。

### 4. 移动端导航 (`android/frontend/src/components/layout/BottomNav.tsx`)

**职责**：移动端底部导航栏，支持动态 Tab 配置与弹簧动画选中态。

*   **类型与常量** (`BottomNav.tsx|14-25`)：
    *   `MobileTab` 联合类型。
    *   `TABS` 基础列表，`QUALITY_TAB` / `MONITOR_TAB` 动态插入。
*   **动态 Tab 获取** (`BottomNav.tsx|26-31`)：
    *   `useNavTabs()`：根据 `config.enableNetworkQuality` 决定插入 `quality` 或 `monitor`。
*   **UI 实现** (`BottomNav.tsx|33-96`)：
    *   磨砂玻璃胶囊容器 (`backdrop-filter: blur(24px)`)。
    *   每个 Tab 按钮等宽 `w-16`，确保 `layoutId="nav-active-pill"` 滑动纯平移。
    *   选中态使用 `m.span` 配合 `spring` 动画 (`stiffness: 250, damping: 28`)。

### 5. 平板端 Dock 导航 (`android/frontend/src/components/layout/DockNav.tsx`)

**职责**：桌面/平板 Dock 导航，实现磁吸放大效果与登录/注销动作按钮。

*   **磁吸逻辑** (`DockNav.tsx|42-112`)：
    *   `DockItem` 组件：监听 `mouseX` (via `useMotionValue`)。
    *   距离 < `MAGNETIC_RANGE (80)` 时，线性插值计算 `scale` (max 1.35) 和 `lift` (max -14)。
    *   使用 `gsap.quickTo` 进行高性能变换，RAF 节流鼠标移动事件。
*   **动作按钮** (`DockNav.tsx|156-235`)：
    *   `ActionButton`：支持 loading 旋转动画 (`gsap.to ... rotation: 360`) 和脉冲边框 (`usePulseAnimation`)。
    *   样式区分 primary (渐变背景) 和 outline。
*   **主组件** (`DockNav.tsx|242-323`)：
    *   过滤 `NAV_ITEMS` (基于 `enableNetworkQuality`)。
    *   固定定位底部居中，左侧留空，右侧分隔线后接 Logout/Login 按钮。

### 6. 平板端右侧面板 (`android/frontend/src/components/layout/RightPanel.tsx`)

**职责**：右侧运行时日志面板，处理大量日志条目时的性能降级与清理动画。

*   **性能优化** (`RightPanel.tsx|48-52, 148-157`)：
    *   `RIGHT_PANEL_ANIM_THRESHOLD = 50`。
    *   日志超过 50 条时，前段静态日志用普通 `div`，仅最近 30 条 (`RIGHT_PANEL_ANIM_KEEP_COUNT`) 使用 `m.div` + `AnimatePresence`。
*   **清理动画** (`RightPanel.tsx|64-125`)：
    *   `handleClearWithAnimation`：
        *   使用 GSAP `stagger` 逐条淡出/位移。
        *   **防卡死机制** (`RightPanel.tsx|87-96` 注释逻辑在此体现)：设置 `failsafe` setTimeout，超时后强制调用 `onClearLogs` 并 `ctx.revert()`，防止 rAF 停滞导致 `isClearing` 永久锁定。
        *   监听 `onInterrupt` 以处理组件卸载中断。
*   **自动滚动** (`RightPanel.tsx|127-138`)：
    *   监听 scroll 位置，若距底部 <40px 则保持自动滚动到底部。

### 7. 移动端仪表盘 (`android/frontend/src/components/mobile/MobileDashboard.tsx`)

**职责**：移动端首页总览，复用桌面 `DashboardPanel` 并通过 `extraCards` 注入移动专属卡片。

*   **移动专属卡片** (`MobileDashboard.tsx|38-139`)：
    *   `MobileStatusCard`：状态灯 + Portal 可达性 + 检查次数 + 运营商标识 (仅手机形态)。
    *   `MobileMonitorCard`：后台检测开关 (Radix Switch) + 立即刷新按钮。
*   **集成** (`MobileDashboard.tsx|141-168`)：
    *   将上述两张卡定义为 `ExtraCardDef`，传入 `DashboardPanel`。
    *   排除 `quickActions` 卡片 (移动端独立悬浮)。

### 8. 移动端“更多”聚合页 (`android/frontend/src/components/mobile/MobileMore.tsx`)

**职责**：“更多”聚合页，通过 Chips 切换子面板 (Settings/Speedtest/Log/Monitor)。

*   **逻辑** (`MobileMore.tsx|33-83`)：
    *   根据 `qualityEnabled` 过滤 Monitor tab。
    *   若 `subRaw` 为 monitor 但 quality 关闭，派生降级为 settings。
    *   渲染对应的 `SettingsPanel`, `SpeedTestPanel`, `LogPanel`, `MonitorPanel`。

### 9. 引导向导 - 平板/桌面版 (`android/frontend/src/settings/OnboardingWizard.tsx`)

**职责**：平板/宽屏设备的引导向导，以 Dialog（对话框）形态呈现。

*   **角色与关键实现**：
    *   **角色**：平板/宽屏设备的引导向导，以 Dialog（对话框）形态呈现。
    *   **核心逻辑**：通过 `useOnboardingFlow` 管理状态，UI 层负责渲染卡片堆叠动画和表单交互。
    *   **常量定义**：
        *   `STEP_TITLE_KEYS`: 4个步骤的国际化键 (`onboarding.welcome`, `accountInfo`, `bindOperatorTitle`, `setupComplete`) (行:44)。
        *   `STACK_LAYERS`: 卡片堆叠样式配置，包含 `y` (偏移), `scale` (缩放), `z` (层级) (行:57-62)。
        *   `EXIT_UP_TRANSFORM`: 退出动画的变换字符串，向上飞起并旋转 (行:69)。
        *   `OPERATOR_VALUE_BY_KEY`: 运营商键值映射 (行:48-52)。
    *   **组件结构**：
        *   `DeckArrow`: 导航箭头组件，支持 `prev`/`next`，禁用态处理 (行:73-96)。
        *   `OnboardingWizard`: 主组件，接收 `open`, `onClose`, `onUpdateConfig`, `onLogin`, `isLoggingIn` props (行:103)。
        *   **动画状态**：使用 `exitingStep` 和 `exitTimerRef` 控制当前卡片退出动画 (行:119-126)。
        *   **前进逻辑 (`deckAdvance`)**：前进时触发退出动画，240ms后切换步骤；后退直接切换 (行:129-141)。
        *   **校验逻辑 (`deckGoNext`)**：在步骤1中，若 `canProceedAccount` 为假，则阻止前进，防止卡片滑走但校验失败 (行:143-148)。
        *   **渲染循环**：使用 `Array.from({ length: STEP_COUNT })` 渲染所有步骤卡片，通过 `pos` 计算相对位置，应用 `STACK_LAYERS` 样式 (行:173-176)。
        *   **无障碍**：非顶层卡片设置 `inert` 属性，防止聚焦 (行:185)。
        *   **步骤内容**：
            *   Step 0: 欢迎页，含吉祥物、安全提示、语言切换按钮 (行:206-229)。
            *   Step 1: 账号信息，用户名、密码（含显示/隐藏）、运营商选择 (行:231-294)。
            *   Step 2: 绑定运营商，包含运营商状态查询面板、手机号、短信密码输入 (行:296-436)。
            *   Step 3: 完成页，展示已填信息摘要，提供登录/跳过入口 (行:438-531)。
        *   **关闭确认**：嵌套 `Dialog` 组件，用于用户尝试关闭时的二次确认 (行:540-561)。

### 10. 引导向导 - 手机版 (`android/frontend/src/settings/OnboardingWizardMobile.tsx`)

**职责**：手机端的引导向导，全屏铺满，内容区独立滚动，底部操作区固定。

*   **核心差异**：
    *   不使用 Dialog，而是 `fixed inset-0` 的全屏容器 (行:146)。
    *   使用 `framer-motion` (`AnimatePresence`, `m.div`) 实现卡片进出场动画 (行:39, 187-196)。
    *   卡片高度自适应，非绝对定位等尺寸，避免长表单被裁切 (行:172-197)。
    *   触控目标放大至 48px (行:102-125, 139-140)。
*   **常量定义**：
    *   `ONBOARDING_STEP_COUNT`: 步骤总数 (行:42)。
    *   `cardVariants`: Framer Motion 变体，定义 `enter`, `center`, `exit` 状态 (行:68-72)。
    *   `StepTrack`: 段式进度条组件，比圆点更省空间 (行:77-99)。
    *   `NavArrow`: 移动端专用导航箭头，48px 触控区 (行:102-125)。
*   **组件结构**：
    *   `OnboardingWizardMobile`: 主组件，结构与桌面端类似，但布局完全不同 (行:127-593)。
    *   **早期返回**：若 `!open` 则不渲染 (行:137)。
    *   **布局结构**：
        *   `header`: 顶部跳过按钮 + 进度轨 (行:154-168)。
        *   `main`: 可滚动内容区，包含装饰性背景卡片和实际步骤卡片 (行:171-519)。
        *   `footer`: 仅最后一步显示，包含登录按钮 (行:522-545)。
    *   **动画实现**：
        *   使用 `AnimatePresence mode="wait"` 确保动画串行执行 (行:187)。
        *   卡片使用 `m.div`，根据 `f.direction.current` 决定进入/退出方向 (行:188-196)。
    *   **步骤内容**：与桌面端逻辑一致，但 UI 细节调整（如 ID 前缀 `m-`，类名调整）(行:203-486)。
    *   **跳过确认**：使用自定义的 `AnimatePresence` 模态框，而非 Radix Dialog，以避免安全区错位 (行:548-591)。

## Architecture

跨平台自适应布局的核心在于“形态感知”与“外壳分发”。`useFormFactor` Hook 作为唯一的形态来源，驱动顶层组件树的选择。

```text
┌──────────────────────────────────────────────────────────────┐
│                        main.tsx                              │
│  (Init Theme, Crash Recovery, GSAP Config)                   │
│                          │                                   │
│                          ▼                                   │
│                     <App />                                   │
│                          │                                   │
│                  useFormFactor()                              │
│              ┌───────────┴───────────┐                       │
│              │                       │                       │
│         'phone' (Mobile)       'tablet' (Tablet/Desktop)     │
│              │                       │                       │
│              ▼                       ▼                       │
│      <AppInner />             <TabletShell />                 │
│      ├── BottomNav           ├── DockNav (Left)              │
│      ├── MobileDashboard     ├── Main Content                │
│      └── QuickActions        └── RightPanel (Logs/Info)      │
│                                                              │
│  Shared Modules:                                             │
│  ├── OnboardingWizard (Dialog) / OnboardingWizardMobile       │
│  └── AnimatedCard, TitleBar                                 │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

数据与控制流在跨平台自适应布局中的流动路径如下：

1.  **形态判定与初始化**:
    *   应用启动时，`main.tsx` 执行初始化。
    *   `App.tsx` 中的 `App` 组件挂载，内部调用 `useFormFactor()` Hook (源自 `frontend-hooks-store`)。
    *   `useFormFactor` 基于屏幕宽度（sw600dp）返回当前形态标识 (`'mobile'` 或 `'tablet'`)。

2.  **外壳分发 (Shell Dispatching)**:
    *   根据形态标识，`App.tsx` 渲染对应的顶层结构：
        *   **Mobile**: 渲染 `AppInner`，包含 `BottomNav` 和 `MobileDashboard`。
        *   **Tablet**: 渲染 `TabletShell`（在文档中隐含于 App 逻辑或相关布局组件），包含 `DockNav` (左侧)、主内容区和 `RightPanel` (右侧)。

3.  **导航与状态同步**:
    *   在 Mobile 模式下，`BottomNav` 触发路由变化，更新 `useAdapterStore` 中的 `activePanel`。
    *   在 Tablet 模式下，`DockNav` 的 `DockItem` 点击同样更新 `activePanel`，但视觉上保留侧边栏高亮。
    *   `RightPanel` 监听 `useLogToastStore` 和特定业务状态，在 Tablet 模式下常驻显示，而在 Mobile 模式下可能通过抽屉或独立页面展示（视具体实现而定，文档指出 RightPanel 为平板侧边栏组件）。

4.  **内容渲染与降级**:
    *   主内容区根据 `activePanel` 加载相应的功能模块（如 `NetworkPanel`, `DashboardPanel` 等）。
    *   对于首次启动用户，`SettingsModule` 中的 `useOnboardingFlow` 被触发。
    *   若为 Mobile，渲染 `OnboardingWizardMobile` (全屏模态)；若为 Tablet，渲染 `OnboardingWizard` (对话框形式)。两者共享相同的业务逻辑推进步骤。

5.  **响应式调整**:
    *   当用户改变窗口大小（如在平板上分屏）时，`useFormFactor` 重新计算，触发 React 重渲染，切换至对应的外壳组件，确保布局始终适配当前可用空间。

## Connections

*   [[responsive-design]]: 本概念是实现响应式设计的具体手段。
*   [[form-factor]]: `useFormFactor` Hook 是本概念的核心依赖。
*   [[mobile-first]]: 移动端布局 (`AppInner`) 是基础实现，平板端在此基础上扩展。
*   [[tablet-adaptation]]: 平板端布局 (`TabletShell`) 是对移动端布局的增强和适应。