---
title: 桌面前端共享层（shared 组件 / UI 原语 / lib 工具 / i18n）
type: module
source_files:
  - tauri-app/frontend/src/shared/index.ts
  - tauri-app/frontend/src/shared/types.ts
  - tauri-app/frontend/src/shared/ui-types.ts
  - tauri-app/frontend/src/shared/ui-constants.ts
  - tauri-app/frontend/src/shared/AnimatedNumber.tsx
  - tauri-app/frontend/src/shared/ConfirmDialog.tsx
  - tauri-app/frontend/src/shared/ErrorBoundary.tsx
  - tauri-app/frontend/src/shared/FluidBackground.tsx
  - tauri-app/frontend/src/shared/LogPanel.tsx
  - tauri-app/frontend/src/shared/MascotFigure.tsx
  - tauri-app/frontend/src/shared/RefreshButton.tsx
  - tauri-app/frontend/src/shared/SegmentTabs.tsx
  - tauri-app/frontend/src/shared/SponsorCard.tsx
  - tauri-app/frontend/src/shared/ToastContainer.tsx
  - tauri-app/frontend/src/components/layout/DockNav.tsx
  - tauri-app/frontend/src/components/layout/TitleBar.tsx
  - tauri-app/frontend/src/components/ui/animated-card.tsx
  - tauri-app/frontend/src/network/adapters.ts
  - tauri-app/frontend/src/lib/glass-dock.ts
  - tauri-app/frontend/src/lib/dock-lens.ts
  - tauri-app/frontend/src/lib/animations.ts
  - tauri-app/frontend/src/lib/utils.ts
tags: [前端, 共享组件, UI 原语, Radix, Tailwind, framer-motion, GSAP, i18n, 布局]
---

## Overview

本模块是桌面前端（Tauri App）的展示层与复用组件层：`shared/` 存放跨面板复用的业务组件（日志面板、确认框、数字滚动、刷新按钮、段标签、Toast 容器、流体背景、错误边界、看板娘图片、赞助浮层），`components/layout/` 提供应用骨架（标题栏 `TitleBar`、底部 Dock 导航 `DockNav`），`components/ui/` 提供动画卡片原语 `AnimatedCard`。所有组件依赖 `lib/`（被间接引用如 `cn`、`extractErrorMessage`）、`i18n`（通过 `react-i18next` 与 `i18next` 直接调用）以及 [[desktop-frontend-hooks]] 的 store/hook。

依赖方向单向：`components/*` 与 `shared/*` 从 `lib/*`、`i18n` 和 hooks 层取数据，反向不成立。安卓前端为独立复刻树，本模块描述桌面端实现（`tauri-app/frontend/src/`）。

## Key Components

### shared/index.ts 桶文件（1-12）

| 行号 | 导出 |
|---|---|
| 1 | `LogPanel` |
| 2 | `ConfirmDialog` |
| 3 | `AnimatedNumber` |
| 4 | `RefreshButton, getRefreshIconClass` |
| 5 | `SegmentTabs` |
| 6 | `ToastContainer` |
| 7 | `FluidBackground` |
| 8 | `ErrorBoundary` |
| 10 | `export * from './types'` |
| 11 | `export * from './ui-types'` |
| 12 | `export * from './ui-constants'` |

`MascotFigure`（`shared/MascotFigure.tsx`）与 `SponsorCard`（`shared/SponsorCard.tsx`）**不在**桶文件里，只能用 `@/shared/MascotFigure` 深路径导入（见 Known Issues 1）。

### shared/types.ts（IPC 载荷类型）

| 行号 | 类型 | 字段 |
|---|---|---|
| 1-5 | `UpdateAvailableData` | `hasUpdate: boolean`、`latestVersion: string`、`releaseNotes?: string` |
| 7-17 | `UpdateInfo` | `hasUpdate`、`latestVersion`、`releaseNotes: string`、`assets: { name; url; size }[]`、`sha256Checksum?: string`、`lastCheckError: string \| null`（14，最近一次检查失败原因快照）、`lastCheckTime: number \| null`（16，最近一次检查完成时间 unix ms） |
| 19-24 | `DownloadProgress` | `downloaded`、`total`、`speed`、`percent`（全 `number`） |
| 26-30 | `MirrorSource` | `name`、`url`、`description`（全 `string`） |

### shared/ui-types.ts（UI 联合类型与展示模型）

| 行号 | 类型 | 取值 / 字段 |
|---|---|---|
| 1 | `StatusState` | `'loading' \| 'online' \| 'offline' \| 'error'` |
| 2 | `PanelName` | `'dashboard' \| 'account' \| 'selfservice' \| 'network' \| 'monitor' \| 'quality' \| 'settings' \| 'log' \| 'speedtest'`（9 个） |
| 3 | `ThemeName` | `'default' \| 'vibrant' \| 'forest' \| 'midnight' \| 'ocean' \| 'cherry' \| 'custom'` |
| 4-5 | `GlassLevel` | `'off' \| 'standard' \| 'ultra'`（玻璃质感档位，对齐 Apple Liquid Glass / FlClash 三档） |
| 6 | `LogType` | `'info' \| 'success' \| 'error' \| 'warning'` |
| 8-15 | `LogEntry` | `id: string`、`time: string`、`message: string`、`type: LogType`、`count?: number`（14 注释：连续重复折叠累计次数，`undefined` 表示只出现一次） |
| 17-29 | `ToastMessage` | `id`、`title`、`description?`、`type: LogType`、`duration?: number`、`mascot?: 'portrait' \| 'celebrate' \| 'offline' \| 'alert' \| 'update'`（24 注释：缺省按 `type` 取默认变体）、`action?: { label: string; onClick: () => void }` |
| 31-34 | `AdapterDisabledWarningData` | `name`、`message` |
| 36-39 | `AutoExitCountdownData` | `delay: number`、`shortcut: string` |
| 41-44 | `SaveConfigResult` | `success: boolean`、`message?: string`、`data?: Record<string, unknown>` |

### shared/ui-constants.ts（常量）

| 行号 | 常量 | 值 / 用途 |
|---|---|---|
| 1 | `MAX_LOG_ENTRIES` | `300`（日志环形上限） |
| 2 | `APP_VERSION` | `'2.4.3'`（`TitleBar.tsx:112,119` 展示） |
| 3 | `APP_NAME` | `'校园网登录助手'` |
| 4 | `PASSWORD_MASK` | `'***'`（配置回传掩码） |
| 7-17 | `NAV_ITEMS` | 9 项导航定义（`as const`）：`dashboard`(快捷键1)/`account`(2)/`selfservice`(9)/`network`(3)/`monitor`(4)/`quality`(5)/`speedtest`(6)/`settings`(7)/`log`(8)，每项含 `id`、`labelKey`、`icon`（lucide 组件名字符串）、`shortcut` |

### shared 业务组件

| 文件:行号 | 导出 | 用途 / 关键实现 |
|---|---|---|
| `AnimatedNumber.tsx:6-12` | `interface AnimatedNumberProps` | `value: number`、`unit?: string`（默认 `'ms'`）、`decimals?: number`、`className?`、`duration?` |
| `AnimatedNumber.tsx:14-110` | `AnimatedNumber` | 数字滚动 + 轻微弹跳；`gsap.quickTo` 建两条快速通道（35-48）；`profile.tier === 'economy'` 时禁用 scale 弹跳（83-92）；cleanup 里 `gsap.killTweensOf`（54-55，修“quickTo 底层 tween 组件卸载后空转”历史缺陷） |
| `ConfirmDialog.tsx:12-18` | `interface ConfirmDialogProps` | `open`、`title`、`message`、`onConfirm`、`onCancel` |
| `ConfirmDialog.tsx:20-48` | `ConfirmDialog` | `confirming` 本地态防双击（23）；`open` 变 false 时由 effect 复位（26-28）——历史实现是 400ms 定时复位，慢网络下提前解禁导致重复提交（已修复） |
| `ErrorBoundary.tsx:6-8` / `10-13` | `Props` / `State` | `children: ReactNode` / `hasError: boolean`、`error: Error \| null` |
| `ErrorBoundary.tsx:15-62` | `ErrorBoundary`（class 组件） | `getDerivedStateFromError`(21-23)、`componentDidCatch` 仅 DEV 打印(25-27)；渲染降级页 + “重新加载”按钮（47-55 先复位再 `window.location.reload()`） |
| `FluidBackground.tsx:8-39` | `ORB_CSS` 内嵌 keyframes | 两枚光斑 `fluid-orb-drift-a/b` 仅 transform/opacity 合成器路径 |
| `FluidBackground.tsx:41-59` | `FluidBackground` | 绝对定位铺满背景层，`background: var(--surface-main)`；按 `profile.willChangeOrbs` 渲染光斑（economy/reduced-motion 不渲染，50） |
| `LogPanel.tsx:28-41` | `interface LogPanelProps` | `api` 要求 `getLogs`/`clearLogs`/`setDebugMode` 必选，`setLogRetentionDays?`/`exportDiagnostics?` 可选；`initialDebugMode?`、`initialLogRetentionDays?`、`addToast(message, type, description?)` |
| `LogPanel.tsx:43` / `45-51` | `type LogLevel` / `interface ParsedLogLine` | `'DEBUG'\|'INFO'\|'WARN'\|'ERROR'`；解析后 `timestamp`/`level`/`module`/`message`/`raw` |
| `LogPanel.tsx:53` / `55-60` | `DEFAULT_LEVEL_CONFIG` / `LEVEL_CONFIG` | 每级图标、文字色、底色、左边框、`labelKey` |
| `LogPanel.tsx:62` | `LOG_LINE_REGEX` | `^\[(.+?)\]\s*\[(DEBUG\|INFO\|WARN\|ERROR)\]\s*\[(.+?)\]\s*(.+)$` |
| `LogPanel.tsx:64-74` | `parseLogLine(line)` | 不匹配返回 `null` |
| `LogPanel.tsx:76-81` | `LINE_OPTIONS` | 100 / 200 / 500 / 1000 四档 |
| `LogPanel.tsx:83` | `MAX_DISPLAY_LINES = 200` | 表格显示上限（超出只渲染最后 200 条，575-579 顶部提示） |
| `LogPanel.tsx:85-682` | `LogPanel`（`memo`） | 系统日志面板：5s 轮询（161-163，叠加 `document.hidden` 与 `IntersectionObserver` 双门控）、`fetchSeqRef` 竞态防护（120-138）、内容未变化不 `setState`（127-129）、调试模式开关（170-182）、保留天数乐观更新+回滚（184-195）、诊断包导出（270-283）、逐条删除动画 + `setTimeout` 兜底（285-388） |
| `MascotFigure.tsx:3` | `SIZES` | `{ sm: 'w-20', md: 'w-28', lg: 'w-40' }` |
| `MascotFigure.tsx:5` | `type MascotVariant` | `'portrait' \| 'welcome' \| 'empty' \| 'celebrate' \| 'sponsor' \| 'offline' \| 'alert'`（7 个，图源 `public/girl/mascot-*.webp`） |
| `MascotFigure.tsx:7-26` | `MascotFigure` | `variant` + `size` + `className`，输出 `loading="lazy"`、`aria-hidden="true"` 的 `<img>` |
| `RefreshButton.tsx:7-11` | `interface RefreshButtonProps` | 继承 `ButtonHTMLAttributes`，加 `isRefreshing`、`iconClassName?`、`showCheck?` |
| `RefreshButton.tsx:13-72` | `RefreshButton`（`forwardRef`） | 刷新结束触发 `refresh-shake`（18-25，`prevRefreshing` 在 effect 开头无条件归位 21-22）；旋转动画 class（44-51）；`showCheck` 时 framer-motion 弹勾（52-65） |
| `RefreshButton.tsx:74-81` | `getRefreshIconClass(isRefreshing, iconClassName?)` | 给自带按钮外壳的调用方复用旋转 class |
| `SegmentTabs.tsx:5-11` | `interface TabItem` | `key`、`label`、`icon: React.ComponentType<{className?:string}>`、`color`、`bg` |
| `SegmentTabs.tsx:13-17` | `interface SegmentTabsProps` | `tabs`、`activeKey`、`onTabChange` |
| `SegmentTabs.tsx:19-54` | `SegmentTabs` | `useId()` 生成 `layoutId` 前缀（22，防多实例共享 `"activeTab"` 错乱） |
| `SponsorCard.tsx:7-10` | `interface SponsorCardProps` | `open`、`onClose` |
| `SponsorCard.tsx:14-18` | `interface SponsorAnchor` | `top`、`right`、`arrowRight` |
| `SponsorCard.tsx:24-133` | `SponsorCard` | 锚定标题栏下方的非模态浮层；位置由打开/resize 时实测赞助按钮 `getBoundingClientRect()` 得出（33-47），锚点契约为 `data-sponsor-anchor="true"`（`TitleBar.tsx:206`）；捕获阶段 `pointerdown` 外部点击 + `Esc` 关闭（49-64）；收款码 `/sponsor-weixin.png`、`/sponsor-alipay.jpg`（113-127） |
| `ToastContainer.tsx:10-13` | `interface ToastContainerProps` | `toasts: ToastMessage[]`、`onRemove: (id) => void` |
| `ToastContainer.tsx:16-21` | `TOAST_MASCOTS` | type→variant 默认映射：`info→portrait`、`success→celebrate`、`error→offline`、`warning→alert` |
| `ToastContainer.tsx:23-28` | `TOAST_STYLES` | 四类 toast 底色（含 dark 变体） |
| `ToastContainer.tsx:30-97` | `ToastContainer`（`memo`） | `economy` 档用 duration 过渡、其余用弹簧（34-36）；celebrate 娘图首帧一次性 spring 弹入（44 `popIn`、57 `m.img`）；`aria-live="polite"` + `role="status"`（39）；读取 `/girl/mascot-${mascot}.webp`（57-65） |

> 注意：`ToastContainer` 自行拼接 mascot 图片路径（57-65），**没有**复用 `MascotFigure`。

### components/layout 布局骨架

| 文件:行号 | 导出 | 用途 / 关键实现 |
|---|---|---|
| `DockNav.tsx:37-47` | `ICON_MAP` | `NAV_ITEMS[].icon` 字符串 → lucide 组件的映射表（新增导航项必须同步此表） |
| `DockNav.tsx:49-79` | `DockItem`（内部组件） | 单图标项：等宽 `w-[52px]`（67），图标+标签竖排常显（75-76），激活态 `text-primary`（70） |
| `DockNav.tsx:81-175` | `AdapterMenu`（内部组件） | 登录/注销适配器选择浮层，仅列 `a.ip` 非空适配器（90），默认选中第一个（91） |
| `DockNav.tsx:177-324` | `ActionButtonWithMenu`（内部组件） | 按钮 + hover 150ms 开、300ms 关（209-218）；仅一张在线卡时不弹菜单直接执行（237-239），两张及以上才弹（207）；加载态 spinner GSAP 旋转（251-259）+ `loadingPulse` 边框（203,295） |
| `DockNav.tsx:326-329` | `interface DockNavProps` | `onPanelChange: (panel: PanelName) => void`、`outerRef?` |
| `DockNav.tsx:331-477` | `DockNav`（`memo`） | 读 `activePanel`/`isLoggingIn`/`isLoggingOut`/`adapters`/`enableNetworkQuality`/`adapter1,adapter2,dualAdapter`（333-342）；`scopedAdapters` 用 `resolveAdapterNames` 限定作用域（349-353）；`visibleItems` 过滤 `quality`（354）；玻璃折射 `useGlassRefraction`（359-361）；镜头系统 `useDockLens`（374-379）；渲染 nav 含放大层（424-449） |
| `TitleBar.tsx:11-22` | `interface TitleBarProps` | 10 个 props：通知开关、主题/关于/赞助回调、浅色切换、最小化/最大化/关闭、`isMaximized` |
| `TitleBar.tsx:24-48` | `MinimizeIcon`/`MaximizeIcon`/`RestoreIcon`/`CloseIcon` | 4 个内联 SVG（10×10） |
| `TitleBar.tsx:50-271` | `TitleBar`（`memo`） | 无边框窗口拖拽区：`startDragging()` 带 300ms 双击防抖（70-82）；双击切换最大化（84-86）；图标按钮组（140-267），赞助按钮标 `data-sponsor-anchor="true"`（206） |

### components/ui 原语

| 文件:行号 | 导出 | 变体 / 要点 |
|---|---|---|
| `animated-card.tsx:6-12` | `interface AnimatedCardConfig` | `glowIntensity?`、`hoverScale?`、`stiffness?`、`damping?`、`mass?` |
| `animated-card.tsx:14-21` | `interface AnimatedCardProps` | 继承 `HTMLAttributes<HTMLDivElement>`，加 `animationConfig?`、`noHover?`、`noAnimation?`、`noEnterAnimation?`、`enableTilt?`、`staggerIndex?` |
| `animated-card.tsx:25-27` | `reducedMotionQuery` | 模块级 `MediaQueryList` |
| `animated-card.tsx:29-151` | `AnimatedCard`（`memo`+`forwardRef`） | 3D 倾斜：`rotateY/rotateX` 两条 `quickTo`（44-45），`ResizeObserver` 失效 rect 缓存（47-48），RAF 节流且回调内只用同步捕获坐标（63-87）；cleanup 取消 RAF 与 delayedCall（49-61） |

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                    Tauri App Frontend                       │
│                                                             │
│  ┌──────────────┐   ┌──────────────┐   ┌────────────────┐  │
│  │   TitleBar   │   │    DockNav   │   │     Shared     │  │
│  │ TitleBar.tsx │   │ DockNav.tsx  │   │  LogPanel.tsx  │  │
│  │ 赞助按钮      │   │ 玻璃镜头 Dock│   │  ConfirmDialog │  │
│  │ data-sponsor │   │ ICON_MAP     │   │  FluidBG       │  │
│  │ anchor="true"│   │              │   │  MascotFigure  │  │
│  └──────┬───────┘   └──────┬───────┘   │  ToastContainer│  │
│         │                  │           └───────┬────────┘  │
│         ▼                  ▼                    ▼           │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                 components/ui/                      │    │
│  │  AnimatedCard (tilt) Button Dialog Select...        │    │
│  └─────────────────────────────────────────────────────┘    │
│         │                                                    │
│         ▼                                                    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │         lib/ (cn, extractErrorMessage, ...)          │    │
│  └─────────────────────────────────────────────────────┘    │
│         │                                                    │
│         ▼                                                    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                   i18n/                             │    │
│  │  zh/en.json + i18next.init                         │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

### 组件 → store → IPC（通过 hooks 层）

`shared` 与 `components` 自身不直接 `invoke`，而是订阅 [[desktop-frontend-hooks]] 的 store，例外是 `LogPanel`：它把 `api` 作为 props 收下（`LogPanel.tsx:28-41`），由调用方注入方法，因此 `LogPanel` 是“组件持有 IPC 句柄”的唯一共享组件：

1. `LogPanel` 挂载 → `fetchLogs(true)`（`LogPanel.tsx:85-87`）→ `api.getLogs(lineCount)`（123）→ 后端 `get_logs`。
2. 5s 轮询（161-163）叠加 `IntersectionObserver` 可见性（144-153）与 `document.hidden`（158-160）双门控。
3. 内容比对（127-129）→ 未变化则跳过 `setState`，避免 `parsedLines`(212-223)/`availableModules`(225-228)/`filteredLines`(230-254)/`displayedLines`(256-261)/`levelCounts`(390-396) 多层 `useMemo` 全量重算。
4. 清空：`handleClear` → `ConfirmDialog`（672-678）→ `handleClearConfirm`（285-388）→ GSAP 逐条动画 → `api.clearLogs()`（322 或 376）→ 重置 `rawLogs` 与 `logsKey`。

### Rust emit → listen → store → UI

事件链路本身在 [[desktop-frontend-hooks]]。本模块的一端是“渲染终点”：

- `useQualityStore` 的 `updateAvailable`/`latestVersion` 被 `TitleBar` 读取（`TitleBar.tsx:64-65`）显示新版本浮标（106-137）。
- `ToastContainer` 渲染 `ToastMessage[]`（`ToastContainer.tsx:30-82`），消息由 `addToast` 调用方（如 `LogPanel` 的 `addToast` prop）注入。

### 主题 → DOM

`useThemeStore` 驱动玻璃档位 `glassLevel`（`DockNav.tsx:359`），`FluidBackground` 使用 `var(--surface-main)`（`FluidBackground.tsx:47`），`TitleBar` 使用 `var(--surface-top)`（94）。`SponsorCard` 玻璃滤镜读 `var(--glass-blur)`/`var(--glass-sat)`（87-88）。

### i18n

组件内用 `useTranslation()`（如 `TitleBar.tsx:62`、`LogPanel.tsx:87`），非组件上下文用 `i18next.t`（如 `ErrorBoundary.tsx:34`、`RefreshButton.tsx:33`）。语言切换入口 `TitleBar.tsx:158` 做 `zh ↔ en` 翻转。

## Connections

- [[desktop-frontend-hooks]]：本模块全部数据来源（store/hook），`LogPanel` 注入的 api 方法、`DockNav` 使用的 `useAdapterStore`/`useAuthStore`/`useConfigStore`/`useThemeStore` 等。
- [[desktop-network-core]]：`DockNav` 用 `resolveAdapterNames`（`network/adapters.ts`）限定登录作用域（349-353）。
- [[desktop-monitor]]：`QualityStore` 的 `updateAvailable` 等由监控模块写入，被 `TitleBar` 消费。
- [[desktop-auth]]：`DockNav` 的 `ActionButtonWithMenu` 调用 `useAuthStore` 的 `doLogin`/`doLogout`（343-344,460-472）。
- [[desktop-config]]：`shared/ui-constants.ts` 的 `PASSWORD_MASK` 与配置掩码语义相关；`DockNav` 读 `useConfigStore` 的 `enableNetworkQuality`/`adapter1` 等。
- [[android-frontend]]：安卓端为独立复刻树，本模块改动需按双端同步要求处理。

## Known Issues

1. **`MascotFigure` 与 `SponsorCard` 未进桶文件**：`shared/index.ts:1-12` 不导出两者，只能 `@/shared/MascotFigure` 深路径导入（`LogPanel.tsx:20`、`SponsorCard.tsx:5` 等）。新增消费方从 `@/shared` 导入会编译失败。
2. **`ToastContainer` 未复用 `MascotFigure`**：`ToastContainer.tsx:30-97` 自行拼 `/girl/mascot-${mascot}.webp`（57-65），若 `toast.mascot` 传入 `'update'` 需存在对应资源，否则静默裂图（无 `onError` 兜底）。`MascotVariant` 含 `welcome`/`empty`/`sponsor` 而 `ToastMessage['mascot']` 无，但后者独有 `update`（`ui-types.ts:24` vs `MascotFigure.tsx:5`）。
3. **`DockNav` 的 `ICON_MAP` 硬编码同步风险**：`DockNav.tsx:37-47` 映射表须与 `shared/ui-constants.ts:7-17` 的 `NAV_ITEMS[].icon` 字符串一致，新增导航项须同时改两处，否则 `Icon` 为 `undefined` 渲染崩溃。
4. **`LogPanel.handleClearConfirm` 中 `ctx` 的 TDZ 依赖**：`finishClear`（`LogPanel.tsx:316-336`）与 `onInterrupt`（351-357）引用 `337` 行才声明的 `const ctx = gsap.context(...)`。当前仅在 `onComplete`/`failsafe`/`onInterrupt` 触发（均晚于声明）故安全；若新增同步调用路径在 337 前行会抛 `ReferenceError`。
5. **`LogPanel` 轮询无退避**：固定 5000ms（`LogPanel.tsx:85-87`）调 `getLogs`，仅靠内容比对跳过重渲染（127-129），不控制 IPC/IO 成本。
6. **`LogPanel` 的 `logsKey` 强制重挂载**：清空后 `setLogsKey`（`326,380`）使 `AnimatePresence` 整树重建（580），长列表有明显重排；`lineKey` 追加 `idx`（587）使同内容行刷新后 key 变化。
7. **`RefreshButton` 的 `prevRefreshing` 归位修复已落地**：现 effect 开头无条件归位（`RefreshButton.tsx:21-22`），不再依赖分支。
8. **`AnimatedCardConfig` 字段无消费方**：`animated-card.tsx:6-12` 的 `glowIntensity`/`hoverScale`/`stiffness`/`damping`/`mass` 仅定义，实际倾斜由 `profile.enableTilt` 与 `handleMouseMove` 中硬编码 `x*8`（`animated-card.tsx:7-8` 的 `xQuick.current?.(x * 8)` / `yQuick.current?.(-y * 8)`）决定；本文件未导出具体乘数，仅保留接口预留。
9. **`ConfirmDialog` 关闭逻辑**：`onOpenChange` 在 `confirming` 时拦截（`ConfirmDialog.tsx:30-33`），避免确认中误触取消；但 `onCancel` 仍可能在动画期间被外部状态调用，依赖调用方保证。
10. **`SponsorCard` 定位依赖 DOM 实测**：`SponsorCard.tsx:24-38` 查询 `button[data-sponsor-anchor="true"]`，若 `TitleBar` 未渲染该属性则浮层不显示（锚点为 `null` 时不渲染，`AnimatePresence` 内 `anchor &&` 条件 68）。
11. **`FluidBackground` 光斑仅 CSS 动画**：`FluidBackground.tsx:41` 条件渲染，economy 档或 `prefers-reduced-motion` 归为 economy 时不挂载节点（注释 6-7），此时背景仅纯色 `var(--surface-main)`。
12. **`ErrorBoundary` 报错信息对用户折叠**：原始错误进 `<details>`（`ErrorBoundary.tsx:15-22`），仅 DEV 打印完整栈（25-27），生产环境用户需展开查看。
