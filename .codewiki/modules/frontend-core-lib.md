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
`frontend-core-lib` 是 Android 前端应用的核心基础设施模块，负责应用的启动入口、双外壳分发、全局状态与 UI 布局适配、国际化配置、动画引擎及通用工具能力。该模块基于 React + Tauri 技术栈，通过 `App.tsx` 中的 `useFormFactor` 实现手机移动端底部导航外壳（`AppInner`）与平板桌面 Dock 布局（`TabletShell`）的无缝切换；利用 `i18n` 目录管理多语言资源；通过 `lib/` 下的零依赖工具函数（缓动配置、颜色转换、通知权限请求）保障在低端设备及复杂 ROM 环境下的稳定性与性能。

## Key Components

### Entry & Shell (`android/frontend/src/App.tsx`)
**职责**：安卓端应用双外壳入口（手机/平板），负责顶层状态管理、路由切换、UI 布局适配及全局弹窗控制。

*   **核心组件 `AppInner`** (行 52-359)
    *   **初始化**：调用 `useAppInit` (行 53) 和 `useAdaptiveFramePace` (行 54)，获取 `useTranslation` (行 55)。
    *   **Tab 状态管理**：
        *   `tab`: 当前页签，默认从 `safeStorage.get('campus-mobile-tab')` 读取，白名单校验 `['dashboard', 'account', 'selfservice', 'quality', 'monitor', 'more']` (行 57-62)。
        *   `deferredTab`: `useDeferredValue(tab)` 优化渲染 (行 63)。
        *   `tabOrder`: 根据 `configEnableNetworkQuality` 决定第 4 位为 `quality` 或 `monitor` (行 70-76)。
        *   `navDir`: 导航方向状态 (-1/0/1)，用于转场动画 (行 77)。
        *   `handleTabChange(next: MobileTab)`: 计算方向、更新状态、持久化 `safeStorage.set` (行 119-127)。
    *   **配置同步逻辑**：`useEffect` 监测若 `configLoaded` 且 `configEnableNetworkQuality !== false` 且 `tab === 'monitor'`，强制 `handleTabChange('quality')` (行 131-136)。
    *   **新手引导逻辑**：`useEffect` 检查 `campus-onboarding-done` 标记及 `useConfigStore.getState().config.user`，若无则弹出向导 (行 138-146)。
    *   **布局结构**：
        *   **Header** (行 202-260)：绝对定位覆盖层，含状态点、在线运营商 Chip (行 215-223)、更新胶囊 (行 225-235)、网络质量胶囊 (行 237-246)、赞助/设置/主题/关于按钮 (行 248-259)。
        *   **MobileQuickActions** (行 263-267)：仅 `dashboard` 页签时懒加载渲染。
        *   **Main** (行 270-297)：滚动区，`paddingTop` 适配 Header 安全区，`paddingBottom` 适配底栏或快捷浮条。
        *   **面板内容 Switch** (行 148-192)：基于 `deferredTab` 分发 `MobileDashboard` / `AccountPanel` / `SelfServicePanel` / `QualityPanel` / `MonitorPanel` / `MobileMore`，包裹于 `AnimatePresence` 与 `m.div` (行 281-295)。
        *   **BottomNav** (行 299)：底部固定导航。
    *   **全局弹窗/对话框**：
        *   `SponsorCard` (行 303)。
        *   `AboutDialog` (懒加载自 `AboutDialogMobile`，行 47, 306-316)。
        *   `UpdateAvailableDialog` (行 318)。
        *   `ThemeDialog` (懒加载，行 49, 321-336，内联 `onSetTheme` 和 `onToggleLightMode` 直接操作 `useThemeStore` 和 `safeStorage`)。
        *   `ConfirmDialog` 账号删除 (行 338-345)。
        *   `OnboardingWizard` (懒加载，行 50, 348-356)。
*   **外壳分发 `App`** (行 363-375)
    *   使用 `useFormFactor()` 判断 `tablet` 或手机 (行 364)。
    *   平板渲染 `TabletShell` (行 368)，手机渲染 `AppInner` (行 368)。
    *   共享 `FaceCaptureDialog` (行 370) 与 `NotificationPermissionGate` (行 371)。
*   **辅助组件 `NotificationPermissionGate`** (行 381-405)
    *   检查 `campus-notification-prompt` 标记 (行 385)。
    *   确认后调用 `requestNotificationPermission({ openSettingsIfDenied: true })` 并标记 (行 398-401)。

### Internationalization (`android/frontend/src/i18n/`)
**职责**：i18next 初始化及翻译资源提供。

*   **`index.ts`** (行 1-28)
    *   插件链：`LanguageDetector` (行 9) + `initReactI18next` (行 10)。
    *   静态导入 `zh.json` / `en.json` (行 5-6)。
    *   `init` 配置：`fallbackLng: 'zh'` (行 16)，`lng` 取自 `localStorage.getItem('app-language')` (行 17)，`detection.order: ['localStorage', 'navigator']` 且 `lookupLocalStorage: 'app-language'` (行 18-22)，`interpolation.escapeValue: false` (行 23-25)。
*   **`locales/en.json`** (行 1-917)
    *   嵌套 JSON 结构，包含 24 个命名空间：`notify`, `common`, `nav`, `titlebar`, `auth`, `account`, `network`, `monitor`, `quality`, `speedtest`, `settings`, `log`, `onboarding`, `updatePrompt`, `about`, `statusbar`, `dashboard`, `confirmDialog`, `panel`, `themeDialog`, `rightPanel`, `sponsor`, `mobile`, `face`。提供全量英文 UI 文本、错误提示及模板变量（如 `{{version}}`, `{{seconds}}`）。

### Animation & Easing (`android/frontend/src/lib/animations.ts`, `easing-config.ts`)
**职责**：提供 Framer Motion 动画变体工厂函数及缓动曲线配置。

*   **`animations.ts`**
    *   `createLogEntryVariants(easing)` (行 3-17): 返回日志条目 `initial`/`animate`/`exit` 动画变体（x 轴位移 + 透明度）。
    *   `createPanelAppleVariants(easing)` (行 22-42): 返回面板转场变体，支持方向感知 `direction: -1 | 0 | 1`，弹簧动画参数 `stiffness: 320, damping: 24, mass: 0.7`，退出时长 `duration: 0.04`。
*   **`easing-config.ts`**
    *   `EasingConfig` 接口 (行 3-9)：定义 `enter`, `exit`, `smooth`, `snappy`, `overshoot` 五种贝塞尔曲线元组 `[number, number, number, number]`。
    *   `EASING_60HZ` 常量 (行 11-17)。
    *   `EASING_120HZ` 常量 (行 19-25)。
    *   `getEasingConfig(refreshRate)` 函数 (行 27-29)：`refreshRate >= 120` 返回 `EASING_120HZ` 否则 `EASING_60HZ`。

### Utilities & Logic (`android/frontend/src/lib/`)
**职责**：通用前端工具函数、颜色转换、权限请求及构建环境类型。

*   **`color.ts`**
    *   `hexToHsl(hex)` (行 1-33): HEX 转 HSL `{h, s, l}`，输入非法时回退默认值 `{ h: 230, s: 70, l: 55 }`。
*   **`notificationPermission.ts`**
    *   `requestNotificationPermission(opts?: { openSettingsIfDenied?: boolean })` (行 9-21): 动态导入 `@tauri-apps/plugin-notification` 请求权限；若被拒且 `opts.openSettingsIfDenied` 为真则调用 `tauriApiWithRetry.openNotificationSettings?.()` 跳系统设置页。
*   **`vite-env.d.ts`** (行 1)
    *   引入 Vite 客户端全局类型 `vite/client`。

## Architecture

```text
┌──────────────────────────────────────────────────┐
│ App (App.tsx:363)                                 │
│  useFormFactor() ─ tablet? ► TabletShell         │
│                   └ mobile? ► AppInner           │
│  Shared: FaceCaptureDialog, NotificationPermission│
└───────────────────────┬──────────────────────────┘
                        ▼
┌──────────────────────────────────────────────────┐
│ AppInner (App.tsx:52)                            │
│  ┌────────┐ ┌─────────┐ ┌─────────┐              │
│  │ Header │ │ Main    │ │BottomNav│              │
│  │(202)   │ │(270)    │ │(299)    │              │
│  └────────┘ └────┬────┘ └─────────┘              │
│                  ▼                                │
│  ┌────────────────────────────────────────────┐  │
│  │ Content Switch (148) + createPanelApple     │  │
│  │ Variants (animations.ts:22) + AnimatePresen │  │
│  └────────────────────────────────────────────┘  │
└───────────────────────┬──────────────────────────┘
                        ▼
┌──────────────────────────────────────────────────┐
│ Libs: easing-config.ts, color.ts, notification    │
│ Permission.ts, i18n/index.ts, locales/en.json    │
└──────────────────────────────────────────────────┘
```

## Data Flow

1.  **启动链路**：
    *   `App` 组件渲染 (行 363)，内部 `useFormFactor` 分发到 `TabletShell` 或 `AppInner`。
    *   `AppInner` 初始化调用 `useAppInit` 和 `useAdaptiveFramePace` (行 53-54)。
    *   面板切换通过 `framer-motion` 的 `AnimatePresence` 结合 `animations.ts` 的 `createPanelAppleVariants` (行 117, 281-295) 实现方向感知动画。
2.  **UI 渲染与样式**：
    *   `AppInner` 依赖 Tailwind 类与 CSS 变量（如 `bg-background`, `var(--surface-main)`，行 196-197）。
    *   动画组件使用 `createPanelAppleVariants` 生成属性，其缓动参数来源于 `useAnimationProfile` (行 115) 映射至 `easing-config.ts` 的 `getEasingConfig`。
3.  **国际化数据流**：
    *   `i18n/index.ts` 初始化 i18next (行 8-28)。
    *   组件通过 `useTranslation` (行 55) 获取 `t` 函数，从 `locales/en.json` (或 `zh.json`) 读取对应命名空间文本。
4.  **权限与存储**：
    *   需要通知权限时，`NotificationPermissionGate` (行 381) 调用 `notificationPermission.ts` 的 `requestNotificationPermission`。
    *   所有持久化存储操作通过 `safeStorage` 代理（如 `campus-mobile-tab` 行 58/126、`campus-onboarding-done` 行 142、`campus-notification-prompt` 行 385/388），确保即使 `localStorage` 异常也不崩溃。

## Connections

*   [[frontend-mobile-components]]: `App.tsx` 向 `MobileDashboard` (行 32, 151), `AccountPanel` (行 37, 155), `SelfServicePanel` (行 38, 167), `QualityPanel` (行 39, 172), `MonitorPanel` (行 40, 182), `MobileMore` (行 33, 190), `BottomNav` (行 31, 299), `TabletShell` (行 34, 368) 传递回调与状态。
*   [[frontend-stores]]: `App.tsx` 从 `useConfigStore` (行 9, 65-68, 86), `useAuthStore` (行 10, 81-84, 112-113), `useQualityStore` (行 15, 87-94), `useLogToastStore` (行 13, 99), `useAccount` (行 19, 97), `useMonitor` (行 18, 96), `useThemeStore` (行 16, 326-331), `useFormFactor` (行 36, 364) 读取或写入状态。
*   [[frontend-dialogs]]: `App.tsx` 控制 `SponsorCard` (行 25, 303), `AboutDialog` (行 47, 306), `UpdateAvailableDialog` (行 29, 318), `ThemeDialog` (行 49, 321), `ConfirmDialog` (行 28, 338, 392), `OnboardingWizard` (行 50, 349), `FaceCaptureDialog` (行 35, 370) 的 `open` 状态及回调。

## Known Issues

1.  **应用名移除与顶栏重组** (`android/frontend/src/App.tsx:236`)：用户要求移除应用名，原位置放置桌面版同款网络质量胶囊（点击进质量页看明细）。
2.  **质量检测默认关闭及页签退避** (`android/frontend/src/App.tsx:67-73`, `129-136`, `179-188`)：`configEnableNetworkQuality` 默认关闭（省电）。关闭时底部导航第 4 位由 `monitor`（`MonitorPanel`）替代 `quality`（`QualityPanel`）补位；跨会话若从存储恢复出 `monitor` 且无质量开启，会导致底栏无高亮，故在 `configLoaded` 后强制归位 `quality`（KI#14 配套逻辑）。
3.  **新手向导标记时机修复** (`android/frontend/src/App.tsx:138-146`)：原实现首次启动无账号时直奔账号页并立即写 `campus-onboarding-done` 等于无引导；现改为向导内“跳过”或“登录成功”时才写标记，未完成下次启动继续引导。
4.  **移动外壳主题设置内联** (`android/frontend/src/App.tsx:324-327`)：移动外壳无 `useSettings` 实例，故 `ThemeDialog` 的 `onSetTheme` 和 `onToggleLightMode` 直接内联调用 `useThemeStore.getState()` 和 `safeStorage.set` 进行状态与持久化，与桌面版 `useSettings.handleSetTheme` 同构。
5.  **通知权限国产 ROM 降级** (`android/frontend/src/lib/notificationPermission.ts:1-3`, `15-17`)：Android 13 以下或国产 ROM 无系统弹框（`requestPermission` 为空转），被拒后唯一路径是 `openSettingsIfDenied` 为 true 时调用 `tauriApiWithRetry.openNotificationSettings?.()` 跳系统设置页。
6.  **动画退出等待税优化** (`android/frontend/src/lib/animations.ts:38`)：面板退出 `duration` 为 `0.04s`，注释明确说明 0.08s 会导致每次切换多等 80ms，0.04s 切换更跟手。
