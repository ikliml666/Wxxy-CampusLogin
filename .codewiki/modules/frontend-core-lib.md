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
`frontend-core-lib` 是 Android 前端应用的核心基础设施模块，负责应用的启动入口、双外壳（手机/平板）自适应布局、国际化配置、动画引擎及通用工具能力。该模块基于 React + Tauri 技术栈，通过 `App.tsx` 实现移动端与平板端的布局分发（双外壳机制），利用 `i18n` 目录管理多语言资源，并通过 `lib/` 下的工具函数库提供缓动配置、颜色转换及通知权限请求等通用能力。此外，模块包含 Vite 构建环境的 TypeScript 声明引用，确保类型安全。

## Key Components

### Entry & Shell (`android/frontend/src/App.tsx`)
**职责**：安卓端应用双外壳入口（手机/平板），负责顶层状态管理、Tab 路由切换、UI 布局适配及全局弹窗控制。

*   **核心组件 `AppInner`** (行 52-359)
    *   **初始化**：调用 `useAppInit` (行 53) 和 `useAdaptiveFramePace` (行 54)，获取 `useTranslation` (行 55)。
    *   **状态字段**：
        *   `tab`: 当前页签，默认从 `safeStorage.get('campus-mobile-tab')` 读取，白名单校验 `dashboard`/`account`/`selfservice`/`quality`/`monitor`/`more` (行 57-62)。
        *   `deferredTab`: 使用 `useDeferredValue` 优化的延迟页签 (行 63)。
        *   `tabOrder`: 页签顺序数组，第 4 位根据 `configEnableNetworkQuality` 在 `quality`/`monitor` 间切换 (行 70-76)。
        *   `navDir`: 导航方向状态 (`-1 | 0 | 1`)，驱动面板转场动画 (行 77)。
        *   `tabRef`: 页签引用，用于在异步回调中读取最新值 (行 78-79)。
        *   弹窗状态：`aboutOpen`, `themeOpen`, `sponsorOpen`, `onboardingOpen`, `confirmDelete` (行 106-110)。
    *   **方法**：
        *   `handleTabChange(next)` (行 119-127)：计算导航方向、更新 `tab` 与 `tabRef`、持久化到 `safeStorage`。
        *   配置同步 `useEffect` (行 131-136)：若 `configEnableNetworkQuality !== false` 且 `tab === 'monitor'`，强制跳转 `quality`（修复跨会话恢复无高亮）。
        *   新手引导 `useEffect` (行 138-146)：检查 `campus-onboarding-done` 标记，无账号则弹出 `OnboardingWizard`。
    *   **渲染结构**：
        *   **Header** (行 202-260)：绝对定位浮层，含状态点、`IspMark` 运营商 Chip、更新胶囊、网络质量胶囊 (`NetworkQualityCapsule`)、赞助/设置/主题/关于按钮。
        *   **QuickActions** (行 263-267)：仅在 `dashboard` 渲染 `MobileQuickActions`。
        *   **Main** (行 270-297)：滚动容器，基于 `deferredTab` 的 `switch` 渲染 `panelContent` (行 148-192)，包裹 `AnimatePresence` 与 `m.div` (`panelVariants`) 实现方向性转场。
        *   **BottomNav** (行 299)：底部固定导航栏。
        *   **全局弹窗**：`ToastContainer` (行 301)、`SponsorCard` (行 303)、懒加载 `AboutDialog` (行 305-316)、`UpdateAvailableDialog` (行 318)、懒加载 `ThemeDialog` (行 320-336，内联主题 store 与 `safeStorage` 逻辑)、`ConfirmDialog` 删除确认 (行 338-345)、懒加载 `OnboardingWizard` (行 348-356)。

*   **外壳分发 `App`** (行 363-375)
    *   使用 `useFormFactor` (行 364) 判断形态。
    *   平板 (`tablet`) 渲染 `TabletShell`，手机渲染 `AppInner` (行 368)。
    *   双外壳共享：`FaceCaptureDialog` (行 370) 与 `NotificationPermissionGate` (行 371)。

*   **辅助组件 `NotificationPermissionGate`** (行 381-405)
    *   检查 `campus-notification-prompt` 标记 (行 384-386)。
    *   确认后调用 `requestNotificationPermission({ openSettingsIfDenied: true })` 并标记完成 (行 398-401)。

### Internationalization (`android/frontend/src/i18n/`)
**职责**：i18next 国际化库的全局初始化配置及英文资源文件。

*   **`index.ts`** (行 1-28)
    *   **插件链**：`LanguageDetector` (localStorage -> navigator) 与 `initReactI18next` (行 8-10)。
    *   **资源加载**：静态导入 `zh.json` 与 `en.json` (行 5-6)。
    *   **初始化 `init`** (行 11-26)：
        *   `fallbackLng: 'zh'` (行 16)。
        *   `lng` 优先读取 `localStorage.getItem('app-language')` (行 17)。
        *   `detection` 配置 `lookupLocalStorage: 'app-language'`，缓存至 localStorage (行 18-22)。
        *   `interpolation.escapeValue: false` (行 23-25)。

*   **`locales/en.json`** (行 1-895)
    *   **职责**：英文翻译资源文件，扁平化 JSON 结构。
    *   **命名空间**：涵盖 `notify`, `common`, `nav` (行 44-55，含 `dashboard`/`account`/`selfservice`/`network`/`monitor`/`quality`/`more` 等), `titlebar`, `auth`, `account`, `network`, `monitor`, `quality`, `speedtest`, `settings`, `log`, `onboarding`, `updatePrompt`, `about`, `statusbar`, `dashboard`, `confirmDialog`, `panel`, `themeDialog`, `rightPanel`, `sponsor`, `mobile` (行 860-869，含移动端专属 `moreMonitor`/`moreSettings` 等), `face`。
    *   **关键内容**：包含自动登录/退出倒计时 (`notify`)、DNS 优化说明 (`network`)、生物识别与人脸风险告知 (`face`/`settings`)、新手向导文案 (`onboarding`) 等，广泛使用 `{{variable}}` 模板占位符。

### Animation & Easing (`android/frontend/src/lib/animations.ts`, `easing-config.ts`)
**职责**：提供 Framer Motion 动画变体工厂及缓动曲线配置。

*   **`animations.ts`**
    *   `createLogEntryVariants(easing)` (行 3-17)：生成日志条目入场/退场变体（x 轴位移 + 淡入淡出），使用 `easing.snappy` 与 `easing.exit`。
    *   `createPanelAppleVariants(easing)` (行 22-41)：生成方向感知面板转场变体，接收 `direction: -1 | 0 | 1`；进场为弹簧动画 (`stiffness: 320, damping: 24, mass: 0.7`)，退场极短 (`duration: 0.04`) 以跟手。

*   **`easing-config.ts`**
    *   **接口 `EasingConfig`** (行 3-9)：字段 `enter`, `exit`, `smooth`, `snappy`, `overshoot`，类型均为 `[number, number, number, number]`（贝塞尔曲线）。
    *   **常量 `EASING_60HZ`** (行 11-17)：60Hz 标准缓动配置。
    *   **常量 `EASING_120HZ`** (行 19-25，导出)：高刷屏优化配置。
    *   **函数 `getEasingConfig(refreshRate)`** (行 27-29)：根据刷新率 `>= 120` 返回 `EASING_120HZ` 否则 `EASING_60HZ`。

### Utilities & Logic (`android/frontend/src/lib/`)
**职责**：通用前端工具函数、颜色转换、权限请求及环境声明。

*   **`color.ts`**
    *   `hexToHsl(hex)` (行 1-33)：将 HEX 字符串转为 HSL 对象 `{ h, s, l }`；含输入校验，解析失败回退默认蓝 `{ h: 230, s: 70, l: 55 }` (行 2)。

*   **`notificationPermission.ts`**
    *   `requestNotificationPermission(opts?)` (行 9-21)：Android 通知权限统一入口。动态导入 `@tauri-apps/plugin-notification`，检查/请求权限；若被拒且 `opts.openSettingsIfDenied` 为真，调 `tauriApiWithRetry.openNotificationSettings?.()` 降级跳系统设置页；弹框异常按未授权静默继续 (行 13)。

*   **`vite-env.d.ts`** (行 1)
    *   引用 Vite Client 类型声明：`/// <reference types="vite/client" />`。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                     Main Entry Point                         │
│                   App.tsx (行 363)                           │
│  ┌───────────────────────────────────────────────────────┐   │
│  │  useFormFactor (行 364)                                │   │
│  │       Tablet? ► TabletShell                            │   │
│  │       Mobile? ► AppInner (行 52)                       │   │
│  └───────────────────────┬───────────────────────────────┘   │
│                          ▼                                    │
│  ┌───────────────────────────────────────────────────────┐   │
│  │                  AppInner                              │   │
│  │  ┌────────────┐ ┌────────────┐ ┌────────────┐         │   │
│  │  │ Header     │ │ Main       │ │ BottomNav  │         │   │
│  │  └────────────┘ └─────┬──────┘ └────────────┘         │   │
│  │                      ▼                                 │   │
│  │  ┌──────────────────────────────────────────┐          │   │
│  │  │  switch(deferredTab) ► panelContent       │          │   │
│  │  │  Dashboard/Account/Self/Quality/Monitor   │          │   │
│  │  └──────────────────────────────────────────┘          │   │
│  └───────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
           ▲                    ▲                    ▲
           │                    │                    │
┌──────────┴──────────┐ ┌──────┴──────────┐ ┌──────┴──────────┐
│   i18n / Locales    │ │     Libs        │ │  Tauri / Perm   │
│ index.ts (行 1)     │ │ animations.ts  │ │ notificationPerm │
│ en.json (行 1)      │ │ easing-config  │ │ (行 9)           │
│ zh.json (未提供)    │ │ color.ts      │ │                  │
└─────────────────────┘ └────────────────┘ └──────────────────┘
```

## Data Flow

1.  **启动链路**：
    *   `App` 组件渲染，通过 `useFormFactor` 分发到 `TabletShell` 或 `AppInner` (行 363-368)。
    *   `AppInner` 挂载时调用 `useAppInit` 与 `useAdaptiveFramePace` 初始化全局状态与帧率调度 (行 53-54)。
    *   面板切换由 `AnimatePresence` 结合 `animations.ts` 的 `createPanelAppleVariants` 驱动方向性转场 (行 117, 281-295)。

2.  **UI 渲染与样式**：
    *   `deferredTab` 决定 `panelContent` 渲染分支 (行 148-192)，通过 `ErrorBoundary` 与 `Suspense` 包裹容错 (行 291-293)。
    *   顶栏状态点、`IspMark` 芯片与更新胶囊依赖 `useAuthStore` / `useQualityStore` 实时状态 (行 81-94, 206-235)。

3.  **国际化加载**：
    *   应用启动时 `i18n/index.ts` 完成初始化，注入 `zh`/`en` 资源 (行 8-26)；组件通过 `useTranslation` 的 `t()` 函数消费 `en.json` / `zh.json` 键值 (如 `App.tsx` 行 55, 218)。

4.  **权限与持久化**：
    *   `NotificationPermissionGate` 首启检查标记，调用 `notificationPermission.ts` 请求权限 (行 381-405)。
    *   页签状态通过 `@/lib/utils` 的 `safeStorage` 代理持久化至 `campus-mobile-tab` (行 58, 126)，账号删除确认依赖 `ConfirmDialog` 回调 (行 338-345)。

## Connections

*   [[frontend-mobile-components]]: `App.tsx` 向 `MobileDashboard` (行 32/151), `AccountPanel` (行 37/154), `SelfServicePanel` (行 38/167), `QualityPanel` (行 39/171), `MonitorPanel` (行 40/181), `MobileMore` (行 33/190) 传递回调与数据。
*   [[frontend-stores]]: `App.tsx` 从 `useConfigStore` (行 65-68), `useAuthStore` (行 81-84), `useQualityStore` (行 87-94), `useLogToastStore` (行 99-104), `useAccount` (行 97), `useMonitor` (行 96) 读取状态并派发动作。
*   [[frontend-dialogs]]: `App.tsx` 控制 `AboutDialog` (行 47/306), `UpdateAvailableDialog` (行 318), `ConfirmDialog` (行 338), `OnboardingWizard` (行 50/349), `ThemeDialog` (行 49/321), `SponsorCard` (行 25/303) 的 `open` 状态及回调。

## Known Issues

1.  **跨会话页签归位 (边界条件)**：
    *   `App.tsx` 行 57-62 与行 129-136：当 `configEnableNetworkQuality` 开启时，底栏第 4 位页签切换为 `quality`，原 `monitor` 页不再可见。若从旧版本升级或后台杀进程前停留在 `monitor`，重启恢复 `tab` 会导致底栏无高亮，需在 `configLoaded` 后通过 `useEffect` 强制归位 `quality`（KI#14 配套逻辑）。

2.  **新手向导标记写入时机变更**：
    *   `App.tsx` 行 138-146：首次启动无账号时弹出向导，标记 `campus-onboarding-done` 改由向导内部在「跳过」或「登录成功」时写入，若未完成则下次启动继续引导，避免旧逻辑中直奔账号页却无引导的问题。

3.  **通知权限弹框异常吞没**：
    *   `notificationPermission.ts` 行 13：调用 `requestPermission()` 时若抛异常，代码按未授权状态静默继续，仅在 `openSettingsIfDenied` 为 true 时尝试跳转系统设置页，不在控制台抛错（DEV 环境仅 `warning` 行 19），可能掩盖部分 ROM 的权限框架崩溃。

4.  **颜色解析静默回退**：
    *   `color.ts` 行 2：当传入非字符串或正则不匹配的 HEX 值时，`hexToHsl` 直接返回默认蓝 `{ h: 230, s: 70, l: 55 }`，调用方需知晓该回退值，避免主题计算出错时难以排查。
