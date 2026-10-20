---
title: frontend-core-lib
type: module
source_files:
  - android/frontend/src/App.tsx
  - android/frontend/src/i18n/index.ts
  - android/frontend/src/i18n/locales/en.json
  - android/frontend/src/lib/animations.ts
  - android/frontend/src/lib/color.ts
  - android/frontend/src/lib/easing-config.ts
  - android/frontend/src/lib/notificationPermission.ts
  - android/frontend/src/vite-env.d.ts
tags: [frontend, core, entry, i18n, styling, utils, build]
---

## Overview
`frontend-core-lib` 是 Android 前端应用的核心基础设施模块，负责应用的启动入口、全局状态管理、国际化配置、样式系统、动画引擎及构建环境。该模块基于 React + Tauri 技术栈，通过 `App.tsx` 实现移动端与平板端的自适应布局（双外壳机制），利用 `i18n` 目录管理多语言资源，并通过 `lib/` 下的工具函数库提供缓动配置、颜色转换及通知权限请求等通用能力。此外，模块包含 Vite 构建配置及 TypeScript 编译选项，确保应用在复杂网络环境和低端设备上的稳定性与性能。

## Key Components

### Entry & Shell (`android/frontend/src/App.tsx`)
**职责**：安卓端应用双外壳入口（手机/平板），负责顶层状态管理、路由切换、UI 布局适配及全局弹窗控制。

*   **核心组件 `AppInner`** (行 52-359)
    *   **初始化**：调用 `useAppInit` (行 53) 和 `useAdaptiveFramePace` (行 54)。
    *   **Tab 状态管理**：
        *   `tab`: 当前页签，默认从 `safeStorage` 读取，白名单校验包括 `dashboard`, `account`, `selfservice`, `quality`, `monitor`, `more` (行 57-62)。
        *   `deferredTab`: 使用 `useDeferredValue` 优化渲染性能 (行 63)。
        *   `navDir`: 导航方向 (-1/0/1)，用于面板转场动画 (行 77)。
        *   `handleTabChange`: 记录方向、更新状态、持久化存储 (行 119-127)。
        *   **配置同步逻辑**：若 `configEnableNetworkQuality` 开启且当前在 `monitor` 页，强制跳转至 `quality` (行 131-136)。
    *   **新手引导 (`OnboardingWizardMobile`)**：
        *   检查 `campus-onboarding-done` 标记及用户是否存在，若无则弹出向导 (行 138-146)。
    *   **布局结构**：
        *   **Header**：绝对定位覆盖层，包含状态点、运营商 Chip、更新胶囊、网络质量胶囊、设置/主题/关于按钮 (行 202-260)。
        *   **Main**：滚动区域，动态计算 Padding 以适配悬浮底栏或快捷操作条 (行 270-297)。
        *   **Content**：基于 `deferredTab` 的 `switch` 渲染，包裹 `AnimatePresence` 和 `ErrorBoundary` (行 148-192, 281-295)。
        *   **BottomNav**：底部固定导航 (行 299)。
    *   **全局弹窗/对话框**：
        *   `SponsorCard` (行 303)。
        *   `AboutDialog` (懒加载，处理版本信息回传) (行 306-316)。
        *   `UpdateAvailableDialog` (行 318)。
        *   `ThemeDialog` (内联处理主题切换逻辑，直接操作 `useThemeStore` 和 `safeStorage`) (行 321-336)。
        *   `ConfirmDialog` (账号删除确认) (行 338-345)。
        *   `OnboardingWizardMobile` (懒加载，全屏覆盖) (行 348-356)。
*   **外壳分发 `App`** (行 363-375)
    *   使用 `useFormFactor` 判断是否为平板 (`tablet`)。
    *   平板渲染 `TabletShell`，手机渲染 `AppInner` (行 368)。
    *   共享组件：`FaceCaptureDialog` (人脸录入/验证), `NotificationPermissionGate` (通知权限门控) (行 370-371)。
*   **辅助组件 `NotificationPermissionGate`** (行 381-404)
    *   检查 `campus-notification-prompt` 标记，未询问则弹出确认框 (行 384-386)。
    *   确认后调用 `requestNotificationPermission` 并标记完成 (行 398-401)。

### Internationalization (`android/frontend/src/i18n/`)
**职责**：i18next 国际化库的全局初始化配置及资源文件。

*   **`index.ts`** (行 1-27)
    *   **插件链**：
        *   `LanguageDetector`：检测语言顺序为 `localStorage` -> `navigator` (行 9-10)。
        *   `initReactI18next`：注入 React 上下文 (行 10)。
    *   **资源加载**：静态导入 `zh.json` 和 `en.json` (行 5-6)。
    *   **初始化配置 `init`**：
        *   `resources`：定义 `zh` 和 `en` 翻译对象 (行 12-15)。
        *   `fallbackLng`：默认为 `'zh'` (行 16)。
        *   `lng`：优先读取 `localStorage.getItem('app-language')`，捕获异常返回 `undefined` (行 17)。
        *   `detection`：查找 localStorage key 为 `app-language`，缓存策略为 `localStorage` (行 18-22)。
        *   `interpolation`：关闭 HTML 转义 (`escapeValue: false`) (行 23-25)。
*   **`locales/en.json`**
    *   **职责**：英文翻译资源文件。
    *   **结构**：扁平化 JSON 对象，涵盖 `notify`, `common`, `nav`, `titlebar`, `auth`, `account`, `network`, `monitor`, `quality`, `speedtest`, `settings`, `log`, `onboarding`, `updatePrompt`, `about`, `statusbar`, `dashboard`, `confirmDialog`, `panel`, `themeDialog`, `rightPanel`, `sponsor`, `mobile`, `face` 等命名空间。
    *   **关键内容**：定义了所有 UI 文本、错误提示、自动登录/退出倒计时文案、DNS 优化说明、生物识别风险提示等。
*   **`locales/zh.json`**
    *   **职责**：全量中文国际化资源文件，定义所有 UI 文本。
    *   **注意**：包含大量模板字符串变量占位符（如 `{{version}}`, `{{seconds}}`），需确保后端数据源格式匹配。

### Animation & Easing (`android/frontend/src/lib/animations.ts`, `easing-config.ts`)
**职责**：提供 Framer Motion 动画变体工厂函数及缓动曲线配置。

*   **`animations.ts`**
    *   **关键导出**：
        *   `createLogEntryVariants(easing)` (行 3-17): 日志条目入场/退场动画（x轴位移+淡入淡出）。
        *   `createPanelAppleVariants(easing)` (行 22-41): 面板转场动画，支持方向感知（`direction: -1 | 0 | 1`），模拟 iOS 页面切换效果（弹簧动画参数 `stiffness: 320, damping: 24`）。
*   **`easing-config.ts`**
    *   **职责**：集中管理缓动曲线配置，适配不同刷新率。
    *   **关键接口**：`EasingConfig` 包含 `enter`, `exit`, `smooth`, `snappy`, `overshoot` 五种贝塞尔曲线 (行 3-9)。
    *   **关键常量**：
        *   `EASING_60HZ` (行 11-17): 标准 60Hz 配置。
        *   `EASING_120HZ` (行 19-25): 高刷优化配置（更短的持续时间，更紧凑的曲线）。
        *   `getEasingConfig(refreshRate)` (行 27-29): 根据屏幕刷新率动态返回对应配置。

### Utilities & Logic (`android/frontend/src/lib/`)
**职责**：通用前端工具函数、颜色转换、权限请求。

*   **`color.ts`**
    *   **职责**：颜色格式转换工具。
    *   **关键导出**：
        *   `hexToHsl(hex)` (行 1-32): 将 HEX 颜色转换为 HSL 对象 `{h, s, l}`。包含输入校验与默认值回退（`{ h: 230, s: 70, l: 55 }`）。
*   **`notificationPermission.ts`**
    *   **职责**：Android 13+ 通知权限的统一请求入口。
    *   **关键导出**：
        *   `requestNotificationPermission(opts)` (行 9-20): 尝试获取权限；若被拒且 `opts.openSettingsIfDenied` 为真，引导用户至系统设置页。兼容国产 ROM 无弹框场景。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                     Main Entry Point                         │
│                   App.tsx                                    │
│  ┌───────────────────────────────────────────────────────┐   │
│  │  useFormFactor                                         │   │
│  │       Tablet? ► TabletShell                            │   │
│  │       Mobile? ► AppInner                               │   │
│  └───────────────────────┬───────────────────────────────┘   │
│                          ▼                                    │
│  ┌───────────────────────────────────────────────────────┐   │
│  │                  AppInner                              │   │
│  │  ┌────────────┐ ┌────────────┐ ┌────────────┐         │   │
│  │  │   Header   │ │   Main     │ │ BottomNav  │         │   │
│  │  └────────────┘ └─────┬──────┘ └────────────┘         │   │
│  │                      ▼                                 │   │
│  │  ┌──────────────────────────────────────────┐          │   │
│  │  │          Content Switch                   │          │   │
│  │  │  Dashboard / Account / Quality / Monitor  │          │   │
│  │  └──────────────────────────────────────────┘          │   │
│  └───────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
           ▲                    ▲                    ▲
           │                    │                    │
┌──────────┴──────────┐ ┌──────┴──────────┐ ┌──────┴──────────┐
│   i18n / Locales    │ │     Libs        │ │     Config      │
│ zh.json / en.json   │ │ animations.ts   │ │ vite.config.ts  │
│ i18n/index.ts       │ │ easing-config.ts│ │ tsconfig.json   │
└─────────────────────┘ │ color.ts        │                   │
                        │ notification... │                   │
                        └─────────────────┘                   │
```

## Data Flow

1.  **启动链路**：
    *   `App` 组件渲染，内部通过 `useFormFactor` 分发到 `TabletShell` 或 `AppInner`。
    *   `AppInner` 初始化时调用 `useAppInit` 和 `useAdaptiveFramePace`。
    *   通过 `framer-motion` 的 `AnimatePresence` 结合 `animations.ts` 提供的变体进行面板切换。

2.  **UI 渲染与样式**：
    *   所有 React 组件隐式依赖 `index.css` 中的 CSS 变量和 Tailwind 类。
    *   动画组件使用 `animations.ts` 的 `createPanelAppleVariants` 等函数生成 Framer Motion 属性。

3.  **网络质量数据流**：
    *   后端/监控模块产生 `NetworkQuality` 原始数据。
    *   展示组件调用 `resolveQualityDisplay` 获取最终展示的延迟值和等级。
    *   调用 `getLatencyLevel` 和 `getLatencyColor` 决定 UI 颜色反馈。

4.  **权限与存储**：
    *   需要通知权限的组件调用 `notificationPermission.ts` 的 `requestNotificationPermission`。
    *   所有持久化存储操作通过 `utils.ts` 的 `safeStorage` 代理，确保即使 `localStorage` 失败也不会导致 JS 报错崩溃。

## Connections

*   [[frontend-mobile-components]]: `App.tsx` 向 `MobileDashboard`, `AccountPanel`, `SelfServicePanel`, `QualityPanel`, `MonitorPanel`, `MobileMore` 传递回调函数。
*   [[frontend-stores]]: `App.tsx` 从 `useConfigStore`, `useAuthStore`, `useQualityStore`, `useLogToastStore`, `useAccount`, `useMonitor` 等读取状态。
*   [[frontend-dialogs]]: `App.tsx` 控制 `AboutDialog`, `UpdateAvailableDialog`, `ConfirmDialog`, `OnboardingWizardMobile` 的 `open` 状态及回调。

## Known Issues

1.  **IDE 临时文件干扰**：
    *   `vite.config.ts` 明确忽略了 `**/.*.tmpdir/**`，这是为了应对 VS Code 或其他 IDE 在保存文件时产生的临时目录，防止 Vite 的 HMR 服务因文件系统事件过多而崩溃或报错。
2.  **Windows 杀软拦截 esbuild**：
    *   `vite.mock.config.ts` 的注释详细描述了在 Windows 上，Defender 或其他杀软可能会拦截 `esbuild` 子进程写入 `node_modules/.vite` 缓存目录。
    *   **解决方案**：将 `cacheDir` 移动到系统临时目录 (`os.tmpdir()`)，这是一个非显然但关键的兼容性修复。
    *   **替代方案**：如果移动缓存目录无效，可以通过环境变量 `ESBUILD_BINARY_PATH` 指定一个重命名的 esbuild 二进制文件副本。
3.  **Tauri Mock 注入时机**：
    *   Mock 脚本通过 `injectTo: 'head-prepend'` 注入，确保它在任何 ES Module 脚本之前执行。这对于正确 mock `__TAURI_INTERNALS__` 至关重要，因为许多 Tauri API 可能在模块初始化时被访问。
4.  **代码分割粒度**：
    *   `vite.config.ts` 将 React、Framer Motion、GSAP 和 Radix UI 单独拆分为 chunk。这是一种优化策略，因为这些库体积较大且更新频率相对较低，有助于利用浏览器缓存。但这也可能增加初始请求数量，需权衡。
5.  **渲染循环存活检测 (非显然行为)**：
    *   `renderLiveness.ts` 采用**按需探测**而非持续 rAF，以解决前台静置时的高功耗问题（实测降低 RenderThread 负载）。
    *   必须配合 `main.tsx` 中的可见性检查，因为浏览器会节流隐藏标签页的 rAF，此处需跳过探测以免误报。
6.  **网络质量状态合并 (边界条件)**：
    *   `latency.ts` 中的 `mergeNetworkQuality` 专门处理 `busy` 状态。在慢网络下，后端可能长时间推送 `busy`，若直接采纳会导致 UI 永远显示“检测中”而无法显示任何延迟数值。逻辑改为：若旧状态非 busy，则忽略新状态的 busy 标记，仅合并明细数据，直到收到终态。
7.  **崩溃恢复阈值**：
    *   `main.tsx` 将心跳超时阈值设为 10s，而非更敏感的 5s，以容忍低端机型的长 GC 停顿或主线程阻塞，避免频繁误重启。
    *   最多允许 3 次自动重载，防止陷入崩溃循环。
8.  **主题强制亮色 (CSS Hack)**：
    *   `index.css` 中的 `.force-light-dialog` 不仅重定义变量，还显式设置 `color` 属性，因为 CSS 变量是继承的，而 `color` 在某些上下文下可能已被计算为暗色值，直接重定义变量不足以覆盖已计算的继承颜色。
9.  **存储容错**：
    *   `utils.ts` 的 `safeStorage` 在 `localStorage` 抛出异常（如配额满）时，回退到内存 Map。这保证了即使在极端存储环境下，应用至少能在当前会话中保持基本功能，而不是直接 Crash。