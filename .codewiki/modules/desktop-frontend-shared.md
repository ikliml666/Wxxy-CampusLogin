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
  - tauri-app/frontend/src/components/layout/RightPanel.tsx
  - tauri-app/frontend/src/components/layout/TitleBar.tsx
  - tauri-app/frontend/src/components/ui/animated-card.tsx
  - tauri-app/frontend/src/components/ui/badge.tsx
  - tauri-app/frontend/src/components/ui/button.tsx
  - tauri-app/frontend/src/components/ui/button.test.tsx
  - tauri-app/frontend/src/components/ui/card.tsx
  - tauri-app/frontend/src/components/ui/dialog.tsx
  - tauri-app/frontend/src/components/ui/input.tsx
  - tauri-app/frontend/src/components/ui/label.tsx
  - tauri-app/frontend/src/components/ui/select.tsx
  - tauri-app/frontend/src/components/ui/separator.tsx
  - tauri-app/frontend/src/components/ui/switch.tsx
  - tauri-app/frontend/src/components/ui/tooltip.tsx
  - tauri-app/frontend/src/lib/animations.ts
  - tauri-app/frontend/src/lib/color.ts
  - tauri-app/frontend/src/lib/color.test.ts
  - tauri-app/frontend/src/lib/easing-config.ts
  - tauri-app/frontend/src/lib/latency.ts
  - tauri-app/frontend/src/lib/latency.test.ts
  - tauri-app/frontend/src/lib/renderLiveness.ts
  - tauri-app/frontend/src/lib/utils.ts
  - tauri-app/frontend/src/lib/utils.test.ts
  - tauri-app/frontend/src/i18n/index.ts
  - tauri-app/frontend/src/i18n/locales/zh.json
  - tauri-app/frontend/src/i18n/locales/en.json
tags: [前端, 共享组件, UI 原语, Radix, Tailwind, framer-motion, GSAP, i18n, 工具函数, 布局]
---

## Overview

本模块是桌面前端的"展示层与工具层"：`shared/` 放跨面板复用的业务组件（日志面板、确认框、toast 容器、看板娘、赞助浮层等），`components/ui/` 是基于 Radix UI + `class-variance-authority` 的无样式原语，`components/layout/` 是应用三段式骨架（标题栏 / 左侧 Dock / 右侧面板），`lib/` 是零状态的纯函数与常量（缓动、颜色、延迟解析、渲染存活判定、cn/错误提取/安全存储），`i18n/` 是 i18next 初始化与 zh/en 双语词条（各 719 条叶子键）。

依赖方向单向：`components/*` 与 `shared/*` 从 `lib/*`、`i18n` 和 [[desktop-frontend-hooks]] 的 store/hook 取数据，反向不成立（`lib/*` 只依赖 `@/monitor`、`@/network/constants` 的类型与常量）。安卓前端是独立复刻树，本模块描述的是桌面端实现（`tauri-app/frontend/src/`）。

## Key Components

### shared/index.ts 桶文件（1-12）

| 行号 | 导出 |
|---|---|
| 1 | `LogPanel` |
| 2 | `ConfirmDialog` |
| 3 | `AnimatedNumber` |
| 4 | `RefreshButton, getRefreshIconClass` |
| 5 | `SegmentTabs, TabContent` |
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
| 7-13 | `UpdateInfo` | `hasUpdate`、`latestVersion`、`releaseNotes: string`、`assets: { name; url; size }[]`、`sha256Checksum?: string` |
| 15-20 | `DownloadProgress` | `downloaded`、`total`、`speed`、`percent`（全 `number`） |
| 22-26 | `MirrorSource` | `name`、`url`、`description`（全 `string`） |

### shared/ui-types.ts（UI 联合类型与展示模型）

| 行号 | 类型 | 取值 / 字段 |
|---|---|---|
| 1 | `StatusState` | `'loading' \| 'online' \| 'offline' \| 'error'` |
| 2 | `PanelName` | `'dashboard' \| 'account' \| 'selfservice' \| 'network' \| 'monitor' \| 'quality' \| 'settings' \| 'log' \| 'speedtest'`（9 个） |
| 3 | `ThemeName` | `'default' \| 'vibrant' \| 'forest' \| 'midnight' \| 'ocean' \| 'cherry' \| 'custom'` |
| 4 | `LogType` | `'info' \| 'success' \| 'error' \| 'warning'` |
| 5 | `GpuTier` | `'low-igpu' \| 'mid-igpu' \| 'high-igpu' \| 'discrete' \| 'unknown'` |
| 7-14 | `GpuInfo` | `vendor`、`model`、`vram_mb: number`、`is_integrated: boolean`、`tier: GpuTier`、`gpu_preference: number` |
| 16-23 | `LogEntry` | `id: string`、`time: string`、`message: string`、`type: LogType`、`count?: number`（21 注释：连续重复折叠次数，`undefined` 表示只出现一次） |
| 25-37 | `ToastMessage` | `id`、`title`、`description?`、`type: LogType`、`duration?: number`、`mascot?: 'portrait' \| 'celebrate' \| 'offline' \| 'alert' \| 'update'`（31 注释：缺省按 `type` 取默认变体）、`action?: { label: string; onClick: () => void }` |
| 39-42 | `AdapterDisabledWarningData` | `name`、`message` |
| 44-47 | `AutoExitCountdownData` | `delay: number`、`shortcut: string` |
| 49-53 | `SaveConfigResult` | `success: boolean`、`message?: string`、`data?: Record<string, unknown>` |

### shared/ui-constants.ts（常量）

| 行号 | 常量 | 值 / 用途 |
|---|---|---|
| 1 | `MAX_LOG_ENTRIES` | `300`（日志环形上限，`useLogToastStore` 与 `LogPanel` 共用） |
| 2 | `APP_VERSION` | `'2.3.6'`（`TitleBar.tsx:112,119` 展示） |
| 3 | `APP_NAME` | `'校园网登录助手'` |
| 4 | `PASSWORD_MASK` | `'***'`（配置回传掩码，`useConfigStore` 与 `useInitialDataLoad` 判定） |
| 7-17 | `NAV_ITEMS` | 9 项导航定义（`as const`）：`dashboard`/`account`/`selfservice`(快捷键 9)/`network`/`monitor`/`quality`/`speedtest`/`settings`/`log`，每项含 `id`、`labelKey`、`icon`（lucide 组件名）、`shortcut` |

### shared 业务组件

| 文件:行号 | 导出 | 用途 / 关键实现 |
|---|---|---|
| `AnimatedNumber.tsx:6-12` | `interface AnimatedNumberProps` | `value: number`、`unit?: string`（默认 `'ms'`）、`decimals?: number`、`className?`、`duration?` |
| `AnimatedNumber.tsx:14-110` | `AnimatedNumber` | 数字滚动 + 轻微弹跳；`gsap.quickTo` 建两条快速通道（35-48）；`profile.tier === 'economy'` 时禁用弹跳（83-92）；cleanup 里 `gsap.killTweensOf`（54-55，修"quickTo 底层 tween 组件卸载后空转"的历史缺陷） |
| `ConfirmDialog.tsx:12-18` | `interface ConfirmDialogProps` | `open`、`title`、`message`、`onConfirm`、`onCancel` |
| `ConfirmDialog.tsx:20-49` | `ConfirmDialog` | `confirming` 本地态防双击（22-23）；`onConfirm` 后 400ms 复位（42） |
| `ErrorBoundary.tsx:6-8` / `10-13` | `Props` / `State` | `children: ReactNode` / `hasError: boolean`、`error: Error \| null` |
| `ErrorBoundary.tsx:15-53` | `ErrorBoundary`（class 组件） | `getDerivedStateFromError`(21-23)、`componentDidCatch` 仅 DEV 打印(25-27)；渲染降级页 + "重新加载"按钮（40-43 先复位再 `window.location.reload()`） |
| `FluidBackground.tsx:1-10` | `FluidBackground` | 绝对定位铺满的背景层，`background: var(--surface-main)`；`App.tsx:378` 唯一调用点 |
| `LogPanel.tsx:28-38` | `interface LogPanelProps` | `api` 要求 6 个方法（`getLogs`/`clearLogs`/`getDebugMode`/`setDebugMode` 必选，`getLogRetentionDays`/`setLogRetentionDays` 可选）、`addToast(message, type, description?)` |
| `LogPanel.tsx:40` / `42-48` | `type LogLevel` / `interface ParsedLogLine` | `'DEBUG'\|'INFO'\|'WARN'\|'ERROR'`；解析后 `timestamp`/`level`/`module`/`message`/`raw` |
| `LogPanel.tsx:50` / `52-57` | `DEFAULT_LEVEL_CONFIG` / `LEVEL_CONFIG` | 每级图标、文字色、底色、左边框、`labelKey` |
| `LogPanel.tsx:59` | `LOG_LINE_REGEX` | `^\[(.+?)\]\s*\[(DEBUG\|INFO\|WARN\|ERROR)\]\s*\[(.+?)\]\s*(.+)$` |
| `LogPanel.tsx:61-71` | `parseLogLine(line)` | 不匹配返回 `null` |
| `LogPanel.tsx:73-78` | `LINE_OPTIONS` | 100 / 200 / 500 / 1000 四档 |
| `LogPanel.tsx:80` | `MAX_DISPLAY_LINES = 200` | 表格显示上限（超出只渲染最后 200 条，584-588 顶部提示） |
| `LogPanel.tsx:82-691` | `LogPanel`（`memo`） | 系统日志面板：5s 轮询（163-176，叠加 `document.hidden` 与 `IntersectionObserver` 双门控）、`fetchSeqRef` 竞态防护（118-135）、内容未变化不 `setState`（123-128）、调试模式开关（178-190）、保留天数乐观更新 + 回滚（192-203）、逐条删除动画 + `setTimeout` 兜底（283-391） |
| `MascotFigure.tsx:3` | `SIZES` | `{ sm: 'w-20', md: 'w-28', lg: 'w-40' }` |
| `MascotFigure.tsx:5` | `type MascotVariant` | `'portrait' \| 'welcome' \| 'empty' \| 'celebrate' \| 'sponsor' \| 'offline'`（图源 `public/girl/mascot-*.webp`，见 1-2 注释） |
| `MascotFigure.tsx:7-26` | `MascotFigure` | `variant` + `size` + `className`，输出 `loading="lazy"`、`aria-hidden="true"` 的 `<img>` |
| `RefreshButton.tsx:7-11` | `interface RefreshButtonProps` | 继承 `ButtonHTMLAttributes`，加 `isRefreshing`、`iconClassName?`、`showCheck?` |
| `RefreshButton.tsx:13-68` | `RefreshButton`（`forwardRef`） | 刷新结束后触发 `refresh-shake`（18-25）；旋转动画 class（44-45）；`showCheck` 时用 framer-motion 弹勾（49-62） |
| `RefreshButton.tsx:71-78` | `getRefreshIconClass(isRefreshing, iconClassName?)` | 给"自带按钮外壳"的调用方复用旋转 class（调用点：`auth/DashboardPanel.tsx:360`、`monitor/QualityPanel.tsx:235`、`monitor/MonitorPanel.tsx:154`） |
| `SegmentTabs.tsx:5-11` | `interface TabItem` | `key`、`label`、`icon: React.ComponentType<{className?:string}>`、`color`、`bg` |
| `SegmentTabs.tsx:13-17` | `interface SegmentTabsProps` | `tabs`、`activeKey`、`onTabChange` |
| `SegmentTabs.tsx:19-54` | `SegmentTabs` | `useId()` 生成 `layoutId` 前缀（22，防多实例共享 `"activeTab"` 导致跨实例布局动画错乱） |
| `SegmentTabs.tsx:56-58` / `60-68` | `interface TabContentProps` / `TabContent` | 仅包一层 `AnimatePresence mode="wait" initial={false}` |
| `SponsorCard.tsx:7-10` | `interface SponsorCardProps` | `open`、`onClose` |
| `SponsorCard.tsx:14-94` | `SponsorCard` | 锚定标题栏下方的非模态浮层；捕获阶段 `pointerdown` 外部点击 + `Esc` 关闭（20-33）；收款码 `/sponsor-weixin.png`、`/sponsor-alipay.jpg`（74-87） |
| `ToastContainer.tsx:10-13` | `interface ToastContainerProps` | `toasts: ToastMessage[]`、`onRemove: (id) => void` |
| `ToastContainer.tsx:16-21` | `TOAST_MASCOTS` | type→variant 默认映射：`info→portrait`、`success→celebrate`、`error→offline`、`warning→alert` |
| `ToastContainer.tsx:23-28` | `TOAST_STYLES` | 四类 toast 底色（含 dark 变体） |
| `ToastContainer.tsx:30-93` | `ToastContainer`（`memo`） | `economy` 档用 duration 过渡、其余用弹簧（34-36）；`aria-live="polite"` + `role="status"`（39）；读取 `/girl/mascot-${mascot}.webp`（55-61） |

> 注意：`ToastContainer` 自己拼接 mascot 图片路径（55-61），**没有**复用 `MascotFigure`（命名空间后缀也不同：`mascot-bg-tea.webp` 之类的水印图由 `RightPanel` 直接写 `<img>`，见 `RightPanel.tsx:241-248`）。

### components/layout 布局骨架

| 文件:行号 | 导出 | 用途 / 关键实现 |
|---|---|---|
| `DockNav.tsx:34-44` | `ICON_MAP` | `NAV_ITEMS[].icon` 字符串 → lucide 组件的映射表（新增导航项必须同步此表） |
| `DockNav.tsx:46-48` | `MAGNETIC_RANGE=80` / `MAX_SCALE=1.35` / `MAX_LIFT=-14` | 磁吸参数 |
| `DockNav.tsx:50-157` | `DockItem`（内部组件） | 单图标项：`gsap.quickTo` 建 scale/y 两条通道（77-78），订阅 `mouseX` 做距离衰减（98-116）；`visibleCount` 变化时重算磁吸中心（123） |
| `DockNav.tsx:159-164` | `interface AdapterMenuProps` | `adapters`、`selectedAdapter?`、`onSelect`、`actionLabel` |
| `DockNav.tsx:166-253` | `AdapterMenu`（内部组件） | 登录/注销的适配器选择浮层，仅列 `a.ip` 非空的适配器（168）；默认选中第一个（169-171） |
| `DockNav.tsx:255-394` | `ActionButtonWithMenu`（内部组件） | 按钮 + hover 150ms 开、300ms 关（286-295）；加载态：spinner `gsap` 旋转（323-331）+ `loadingPulse` 边框（281,363-369） |
| `DockNav.tsx:396-399` | `interface DockNavProps` | `onPanelChange: (panel: PanelName) => void`、`outerRef?` |
| `DockNav.tsx:401-545` | `DockNav`（`memo`） | 读 `activePanel`/`isLoggingIn`/`isLoggingOut`/`adapters`/`enableNetworkQuality`/`adapter1,adapter2,dualAdapter`（403-412，后三项用 `useShallow` 408）；`scopedAdapters` 用 `resolveAdapterNames` 限定作用域（419-423）；活动指示条（512-517）；`mounted` 800ms 后启用指示条测量（432-435） |
| `RightPanel.tsx:20-24` | `interface RightPanelProps` | `logs: LogEntry[]`、`onClearLogs?`、`outerRef?` |
| `RightPanel.tsx:26-31` / `33-38` / `40-45` / `47-52` | `LOG_ICONS` / `LOG_COLORS` / `LOG_BG_COLORS` / `LOG_BAR_COLORS` | 四类日志的图标/文字色/底色/左条色 |
| `RightPanel.tsx:57-58` | `RIGHT_PANEL_ANIM_THRESHOLD=50` / `RIGHT_PANEL_ANIM_KEEP_COUNT=30` | 分片阈值：>50 条时只有最后 30 条走 `m.div` |
| `RightPanel.tsx:60-81` | `getAdapterInfo(adapterName, adapterDetails, adapters)` | 优先精确匹配 `adapterDetails`，逐级回退（有线 → 任一有 IP → `adapters` 里的有线 → 任一有 IP），都无返回 `null` |
| `RightPanel.tsx:83-428` | `RightPanel`（`memo`） | 运行日志卡（217-327）+ 网络适配器卡（329-424）；适配器多网卡信息网格（407-414）；`displayAdapters` 由 `adapter1`/`adapter2`+`dualAdapter` 决定（185-195） |
| `TitleBar.tsx:11-22` | `interface TitleBarProps` | 10 个 props：通知开关、主题/关于/赞助回调、浅色切换、最小化/最大化/关闭、`isMaximized` |
| `TitleBar.tsx:24-48` | `MinimizeIcon` / `MaximizeIcon` / `RestoreIcon` / `CloseIcon` | 4 个内联 SVG（10×10，`strokeWidth` 1.2/1.4） |
| `TitleBar.tsx:50-270` | `TitleBar`（`memo`） | 无边框窗口拖拽区：`startDragging()` 带 300ms 双击防抖（70-82）；双击切换最大化（84-86）；图标按钮组（140-266） |

### components/ui 原语

| 文件:行号 | 导出 | 变体 / 要点 |
|---|---|---|
| `animated-card.tsx:6-12` | `interface AnimatedCardConfig` | `glowIntensity?`、`hoverScale?`、`stiffness?`、`damping?`、`mass?` |
| `animated-card.tsx:14-21` | `interface AnimatedCardProps` | 继承 `HTMLAttributes<HTMLDivElement>`，加 `animationConfig?`、`noHover?`、`noAnimation?`、`noEnterAnimation?`、`enableTilt?`、`staggerIndex?` |
| `animated-card.tsx:23` / `27-29` | `REST_SHADOW` / `reducedMotionQuery` | 静止阴影常量；模块级 `MediaQueryList` |
| `animated-card.tsx:31-151` | `AnimatedCard`（`memo`+`forwardRef`） | 3D 倾斜：`rotateY/rotateX` 两条 `quickTo`（46-47），`ResizeObserver` 失效 rect 缓存（49-50），RAF 节流且回调内只用同步捕获的坐标（65-89）；cleanup 取消 RAF 与 delayedCall（51-62） |
| `badge.tsx:5-30` | `badgeVariants` | 8 个 `variant`（default/secondary/destructive/outline/success/warning/info/ghost）× 3 个 `size`（default/sm/lg） |
| `badge.tsx:32-34` / `36-40` | `interface BadgeProps` / `Badge` | 渲染 `<div>` |
| `button.tsx:6-34` | `buttonVariants` | 9 个 `variant`（default/destructive/outline/secondary/ghost/link/glass/soft）/ 6 个 `size`（default/sm/lg/icon/icon-sm/icon-lg） |
| `button.tsx:36-41` | `interface ButtonProps` | 加 `asChild?`、`isLoading?` |
| `button.tsx:43-170` | `Button`（`forwardRef`） | `asChild` 走 Radix `Slot`（45-84）；普通分支 `btn-press` class（107）+ 鼠标位置写 `--mouse-x/--mouse-y`（110-137，3px 去抖、RAF 节流、cleanup 取消 RAF 99-103） |
| `card.tsx:4-14` / `16-26` / `28-38` / `40-46` | `CardHeader` / `CardTitle` / `CardDescription` / `CardContent` | 四个 `forwardRef` 容器（**没有** `Card` 与 `CardFooter`；卡片外框由 `AnimatedCard` 承担） |
| `dialog.tsx:7` / `9` / `11-24` | `Dialog` / `DialogPortal` / `DialogOverlay` | Root/Portal/Overlay（后两者**未导出**） |
| `dialog.tsx:26-54` | `DialogContent` | 加 `showClose?: boolean`（默认 true，43-48）；自己组 Portal + Overlay + 居中容器（32-33） |
| `dialog.tsx:56-68` / `70-83` / `85-95` | `DialogHeader` / `DialogTitle` / `DialogDescription` | 头部/T标题/描述 |
| `input.tsx:4-8` | `interface InputProps` | 加 `icon?: ReactNode`、`error?: string` |
| `input.tsx:10-38` | `Input`（`forwardRef`） | 有 `icon` 时 `pl-10`（23）；`error` 时补边框宽度（24-26，注释说明基础类无 border 宽度）+ 错误文案（31-33） |
| `label.tsx:6-8` | `labelVariants` | 单一样式串 |
| `label.tsx:10-24` | `Label`（`forwardRef`） | 加 `required?: boolean` → 追加红色 `*`（21） |
| `select.tsx:6` / `8` / `10-34` | `Select` / `SelectValue` / `SelectTrigger` | Trigger 加 `icon?`（12,23-27） |
| `select.tsx:36-64` / `66-86` / `88-98` | `SelectContent` / `SelectItem` / `SelectSeparator` | Content 默认 `position='popper'`（39） |
| `separator.tsx:5-26` | `Separator`（`forwardRef`） | 支持 `orientation`，默认 `horizontal`、`decorative=true` |
| `switch.tsx:5-32` | `Switch`（`forwardRef`） | 加 `size?: 'default' \| 'sm' \| 'lg'`（7-9），三档尺寸与滑块位移一一对应（24-27） |
| `tooltip.tsx:5` / `7` / `9` / `11` | `TooltipProvider` / `Tooltip` / `TooltipTrigger` / `TooltipPortal` | 前三个导出；`TooltipPortal` **未导出**（11） |
| `tooltip.tsx:13-29` | `TooltipContent`（`forwardRef`） | 内建 Portal（17），默认 `sideOffset=4`；导出清单见 31（无 `TooltipPortal`） |
| `button.test.tsx:12-24` | 用例 | 断言 `onMouseMove` 的 RAF 回调不访问 React 已置 null 的 `currentTarget`，且 `--mouse-x/--mouse-y` 被写入 |

### lib 工具与常量

| 文件:行号 | 导出 | 说明 |
|---|---|---|
| `utils.ts:4-6` | `cn(...inputs)` | `twMerge(clsx(inputs))` |
| `utils.ts:8-18` | `extractErrorMessage(e)` | 字符串原样、`Error` 取 `message`、普通对象优先取 `.message`（11-16，注释：Tauri 插件 reject 的是普通对象，`String()` 会得到 `[object Object]`）、其余 `String(e)` |
| `utils.ts:20` / `22-29` / `31` | `memoryFallback` / `safeStorage` / 导出 | `localStorage` 读写包裹 try/catch，失败降级到内存 Map |
| `color.ts:1-33` | `hexToHsl(hex)` | 支持带/不带 `#` 的 6 位 hex；非法输入（长度 <4、`NaN`、正则不匹配）统一返回 `{h:230,s:70,l:55}` |
| `easing-config.ts:3-9` | `interface EasingConfig` | `enter`/`exit`/`smooth`/`snappy`/`overshoot`，全部 `[number,number,number,number]` 贝塞尔 |
| `easing-config.ts:11-17` | `EASING_60HZ`（**未导出**） | 60Hz 基线缓动 |
| `easing-config.ts:19-25` | `EASING_120HZ` | 120Hz 缓动（更"紧"的曲线） |
| `easing-config.ts:27-29` | `getEasingConfig(refreshRate)` | `refreshRate >= 120 ? EASING_120HZ : EASING_60HZ` |
| `animations.ts:3-17` | `createLogEntryVariants(easing)` | 日志条目进出场 variants（进入 0.3s `snappy`、退出 0.15s `exit`） |
| `animations.ts:19` | `PANEL_ORDER` | 面板顺序常量（**不含 `selfservice`**：`['dashboard','account','network','monitor','quality','speedtest','settings','log']`） |
| `animations.ts:21-26` | `getPanelDirection(from, to)` | 按 `PANEL_ORDER` 下标决定左/右滑（任一不在表内返回 `1`） |
| `animations.ts:28-45` | `createPanelAppleVariants(easing)` | 面板转场 variants（进入弹簧 `stiffness 320 / damping 24 / mass 0.7`；退出 0.04s） |
| `latency.ts:4` | `type LatencyLevel`（**未导出**） | `'excellent'\|'great'\|'good'\|'fair'\|'poor'\|'bad'` |
| `latency.ts:5` | `type LatencyType` | `'gateway' \| 'external'` |
| `latency.ts:7-15` | `getLatencyLevel(latency)` | 负数→bad；≤20 excellent；≤50 great；≤100 good；≤200 fair；≤400 poor；其余 bad |
| `latency.ts:17-23` | `getLatencyColor(latency)` | 负数→rose 三件套；其余查 `QUALITY_CONFIG`（`network/constants.ts:1-11`）的 `color`/`bg`/`borderBg` |
| `latency.ts:25-29` | `mergeNetworkQuality(old, incoming)` | `disabled`/`busy` 保留旧值（26）；旧值为空或 `unknown` 时直接用新值（27）；否则合并 `details`（新覆盖旧）并保留 `metrics`（28） |
| `latency.ts:31-36` | `extractGatewayLatency(nq)` | `gatewayLatency >= 0` 优先，回退 `details['gateway'].latency`，否则 `-1` |
| `latency.ts:38-52` | `extractExternalLatency(nq)` | `averageExternalLatency >= 0` → 次之 `externalLatency` → 次之 `details` 中非 gateway 的中位数（46-50）→ 否则 `-1` |
| `latency.ts:59-69` | `resolveQualityDisplay(nq)` | 统一展示解析：`displayLatency`（外部优先，退化到网关）；`quality` 在 `unknown`/`busy` 且 `displayLatency >= 0` 时按延迟推断（64-67） |
| `renderLiveness.ts:14-15` | `lastRafTime` + `probing` | 模块级状态：上次探测时间戳 + 探测进行中标志（**无常驻 rAF 循环**，2026-09-13 功耗改造） |
| `renderLiveness.ts:18` / `20` / `21` | `PROBE_REFRESH_MS=4_000` / `PROBE_TIMEOUT_MS=500` / `RENDER_STALL_THRESHOLD_MS=10_000` | 重开探测窗口阈值（< 判定阈值一半）/ 开窗兜底复位 / 停滞判定阈值 |
| `renderLiveness.ts:24-34` | `startProbe()` | 开一个 2 帧（≈33ms）rAF 窗口刷新 `lastRafTime`；`probing` 防重入 + `setTimeout(500ms)` 兜底复位（半死状态） |
| `renderLiveness.ts:36-40` | `isRenderLoopAlive()` | `document.hidden` 直接返回 `true`（37）；距上次探测 > 4s 时 `startProbe()`（38）；返回 `performance.now() - lastRafTime <= 10s`；调用方仍须先做可见性短路（11-13 注释） |
| `latency.test.ts:23-181` | 测试 | 覆盖 `getLatencyLevel`(23-53)、`getLatencyColor`(55-79)、`mergeNetworkQuality`(81-121)、`extractGatewayLatency`(123-145)、`extractExternalLatency`(147-181) |
| `color.test.ts:4-40` | 测试 | 三原色/黑白/非法输入/短 hex/无 `#` 共 9 例 |
| `utils.test.ts:4-38` | 测试 | `cn` 4 例（5-20）、`extractErrorMessage` 4 例（22-38） |

### i18n

| 文件:行号 | 项 | 说明 |
|---|---|---|
| `i18n/index.ts:1-6` | imports | `i18next`、`initReactI18next`、`LanguageDetector`、`zh.json`、`en.json` |
| `i18n/index.ts:8-26` | `i18n.init({...})` | `resources: { zh: { translation: zh }, en: { translation: en } }`（12-15）、`fallbackLng: 'zh'`（16）、初始 `lng` 直读 `localStorage.getItem('app-language')`（17，try/catch）、`detection.order = ['localStorage','navigator']` 且 `lookupLocalStorage: 'app-language'`、`caches: ['localStorage']`（18-22）、`escapeValue: false`（23-25） |
| `i18n/index.ts:28` | `export default i18n` | 供 `main.tsx` 侧 `import './i18n'` 触发初始化 |
| `i18n/locales/zh.json:1-772` | 中文词条 | 22 个顶层命名空间、719 条叶子键 |
| `i18n/locales/en.json:1-772` | 英文词条 | 22 个顶层命名空间、719 条叶子键（与 zh 键集合完全一致，无缺失） |

**顶层命名空间（zh.json 与 en.json 行号完全一致）**：

| 行号 | 命名空间 | 叶子键数 | 主要消费方 |
|---|---|---|---|
| 2 | `notify` | 13 | `useEventListeners.ts`（自动登录/自动退出/校园网退出提示） |
| 17 | `common` | 21 | `components/ui/*`、`RefreshButton`、`LogPanel` |
| 40 | `nav` | 9 | `DockNav`（`NAV_ITEMS[].labelKey`） |
| 51 | `titlebar` | 15 | `components/layout/TitleBar.tsx` |
| 68 | `auth` | 29 | `useAuthStore`、`auth/useAuth.ts` |
| 99 | `account` | — | `account/AccountPanel.tsx`、`SelfServicePanel` |
| 209 | `network` | — | `network/NetworkPanel.tsx` |
| 280 | `monitor` | 64 | `monitor/*`、`useEventListeners` 的质量告警文案 |
| 346 | `quality` | — | 质量面板与 `network/constants.ts` 的 `labelKey` |
| 413 | `speedtest` | — | `monitor/SpeedTestPanel.tsx` |
| 439 | `settings` | — | `settings/SettingsPanel.tsx` |
| 528 | `log` | 30 | `shared/LogPanel.tsx`、`useConfigStore` 的保存失败/脏字段回退文案 |
| 566 | `onboarding` | — | `settings/OnboardingWizard.tsx` |
| 629 | `about` | — | `auth/AboutDialog.tsx` |
| 673 | `statusbar` | — | `monitor/StatusBar.tsx` |
| 680 | `dashboard` | — | `auth/DashboardPanel.tsx` |
| 717 | `confirmDialog` | 2 | `shared/ConfirmDialog.tsx` |
| 721 | `panel` | — | 各面板标题 |
| 741 | `themeDialog` | — | `settings/ThemeDialog.tsx` |
| 749 | `rightPanel` | 13 | `components/layout/RightPanel.tsx` |
| 764 | `dock` | 1 | `components/layout/DockNav.tsx`（`dock.selectAdapter`） |
| 767 | `sponsor` | 4 | `shared/SponsorCard.tsx` |

词条同时经两种方式消费：`react-i18next` 的 `useTranslation()`（组件内，如 `TitleBar.tsx:62`）与直接 `i18next.t(...)`（非组件上下文，如 `useAuthStore.ts:139`、`useEventListeners.ts:41`、`ErrorBoundary.tsx:34`）。

## 结构体与字段

### 组件 Props 结构（按文件）

| 类型 | 行号 | 字段与类型 |
|---|---|---|
| `AnimatedNumberProps` | `AnimatedNumber.tsx:6-12` | `value: number`、`unit?: string = 'ms'`、`decimals?: number = 0`、`className?: string = ''`、`duration?: number`（缺省取 `profile.numberDuration`，23） |
| `ConfirmDialogProps` | `ConfirmDialog.tsx:12-18` | `open: boolean`、`title: string`、`message: string`、`onConfirm: () => void`、`onCancel: () => void` |
| `Props` / `State`（ErrorBoundary） | `ErrorBoundary.tsx:6-8` / `10-13` | `children: ReactNode` / `hasError: boolean`、`error: Error \| null` |
| `LogPanelProps` | `LogPanel.tsx:28-38` | `api.getLogs(lines?: number): Promise<string>`、`api.clearLogs(): Promise<boolean>`、`api.getDebugMode(): Promise<boolean>`、`api.setDebugMode(enabled: boolean): Promise<boolean>`、`api.getLogRetentionDays?(): Promise<number>`、`api.setLogRetentionDays?(days: number): Promise<void>`、`addToast(message: string, type: 'info'\|'success'\|'error'\|'warning', description?: string): void` |
| `ParsedLogLine` | `LogPanel.tsx:42-48` | `timestamp: string`、`level: LogLevel`、`module: string`、`message: string`、`raw: string` |
| `RefreshButtonProps` | `RefreshButton.tsx:7-11` | `isRefreshing: boolean`、`iconClassName?: string`、`showCheck?: boolean` + `ButtonHTMLAttributes<HTMLButtonElement>` |
| `TabItem` | `SegmentTabs.tsx:5-11` | `key: string`、`label: string`、`icon: React.ComponentType<{className?: string}>`、`color: string`、`bg: string` |
| `SegmentTabsProps` | `SegmentTabs.tsx:13-17` | `tabs: TabItem[]`、`activeKey: string`、`onTabChange: (key: string) => void` |
| `TabContentProps` | `SegmentTabs.tsx:56-58` | `children: React.ReactNode` |
| `SponsorCardProps` | `SponsorCard.tsx:7-10` | `open: boolean`、`onClose: () => void` |
| `ToastContainerProps` | `ToastContainer.tsx:10-13` | `toasts: ToastMessage[]`、`onRemove: (id: string) => void` |
| `DockNavProps` | `DockNav.tsx:396-399` | `onPanelChange: (panel: PanelName) => void`、`outerRef?: (el: HTMLDivElement \| null) => void` |
| `AdapterMenuProps` | `DockNav.tsx:159-164` | `adapters: Adapter[]`、`selectedAdapter?: string`、`onSelect: (adapterName: string) => void`、`actionLabel: string` |
| `RightPanelProps` | `RightPanel.tsx:20-24` | `logs: LogEntry[]`、`onClearLogs?: () => void`、`outerRef?: (el: HTMLDivElement \| null) => void` |
| `TitleBarProps` | `TitleBar.tsx:11-22` | `notificationEnabled: boolean`、`onToggleNotification: () => void`、`onShowTheme: () => void`、`onShowAbout: () => void`、`onShowSponsor: () => void`、`onToggleLightMode: () => void`、`onMinimize: () => void`、`onToggleMaximize: () => void`、`onClose: () => void`、`isMaximized: boolean` |
| `AnimatedCardConfig` | `animated-card.tsx:6-12` | `glowIntensity?: number`、`hoverScale?: number`、`stiffness?: number`、`damping?: number`、`mass?: number`（当前无消费方，见 Known Issues 5） |
| `AnimatedCardProps` | `animated-card.tsx:14-21` | `animationConfig?: AnimatedCardConfig`、`noHover?: boolean`、`noAnimation?: boolean`、`noEnterAnimation?: boolean`、`enableTilt?: boolean`（缺省取 `profile.enableTilt`，36）、`staggerIndex?: number`（写入 `--stagger-i`，127） + `HTMLAttributes<HTMLDivElement>` |
| `BadgeProps` | `badge.tsx:32-34` | `HTMLAttributes<HTMLDivElement>` + `VariantProps<typeof badgeVariants>`（`variant`、`size`） |
| `ButtonProps` | `button.tsx:36-41` | `ButtonHTMLAttributes<HTMLButtonElement>` + `VariantProps`（`variant`、`size`）+ `asChild?: boolean`、`isLoading?: boolean` |
| `InputProps` | `input.tsx:4-8` | `InputHTMLAttributes<HTMLInputElement>` + `icon?: React.ReactNode`、`error?: string` |
| `Switch` 内联 props | `switch.tsx:7-9` | `size?: 'default' \| 'sm' \| 'lg'` |

### 常量与联合类型

| 名称 | 行号 | 值 |
|---|---|---|
| `MAX_LOG_ENTRIES` | `ui-constants.ts:1` | `300` |
| `APP_VERSION` | `ui-constants.ts:2` | `'2.3.6'` |
| `APP_NAME` | `ui-constants.ts:3` | `'校园网登录助手'` |
| `PASSWORD_MASK` | `ui-constants.ts:4` | `'***'` |
| `NAV_ITEMS` | `ui-constants.ts:7-17` | 9 项，字段 `id` / `labelKey` / `icon` / `shortcut` |
| `PanelName` | `ui-types.ts:2` | 9 个面板 id（**含 `'selfservice'`，与 `lib/animations.ts:19` 的 `PANEL_ORDER` 不一致**） |
| `MascotVariant` | `MascotFigure.tsx:5` | 6 个：`portrait`/`welcome`/`empty`/`celebrate`/`sponsor`/`offline` |
| `ToastMessage['mascot']` | `ui-types.ts:32` | 5 个：`portrait`/`celebrate`/`offline`/`alert`/`update`（比 `MascotVariant` 多 `alert`、`update`，少 `welcome`、`empty`、`sponsor`） |
| `LogLevel` | `LogPanel.tsx:40` | `'DEBUG' \| 'INFO' \| 'WARN' \| 'ERROR'` |
| `MAX_DISPLAY_LINES` | `LogPanel.tsx:80` | `200` |
| `EasingConfig` | `easing-config.ts:3-9` | 5 条贝塞尔曲线（`enter`/`exit`/`smooth`/`snappy`/`overshoot`） |
| `PANEL_ORDER` | `animations.ts:19` | 8 项（缺 `selfservice`） |
| `RIGHT_PANEL_ANIM_THRESHOLD` / `_KEEP_COUNT` | `RightPanel.tsx:57-58` | `50` / `30` |
| `TOAST_MASCOTS` | `ToastContainer.tsx:16-21` | `LogType → ToastMessage['mascot']` 映射 |
| `TOAST_STYLES` | `ToastContainer.tsx:23-28` | `LogType → 底色 class` |
| `ICON_MAP` | `DockNav.tsx:34-44` | 9 个 lucide 组件（含重复导入的 `Wifi`/`WifiIcon`，3-17） |
| `LOG_ICONS` / `LOG_COLORS` / `LOG_BG_COLORS` / `LOG_BAR_COLORS` | `RightPanel.tsx:26-31` / `33-38` / `40-45` / `47-52` | `LogEntry['type']` → 图标/文字色/底色/左条色 |

## Data Flow

### 组件 → store → IPC（通过 hooks 层）

`shared` 与 `components` 自身不直接 `invoke`，而是订阅 [[desktop-frontend-hooks]] 的 store，例外是 `LogPanel`：它把 `api` 作为 props 收下（`LogPanel.tsx:28-36`），由调用方（日志面板所在页面）注入 `tauriApiWithRetry` 的方法，因此 `LogPanel` 是"组件持有 IPC 句柄"的唯一共享组件：

1. `LogPanel` 挂载 → `fetchLogs(true)`（`LogPanel.tsx:138-140`）→ `api.getLogs(lineCount)`（121）→ `get_logs`。
2. 5s 轮询（163-176）叠加 `IntersectionObserver` 可见性（142-151）与 `document.hidden`（166-168）双门控。
3. 内容比对（125）→ 未变化则跳过 `setState`，避免 `parsedLines`(232-243)/`availableModules`(245-248)/`filteredLines`(250-274)/`displayedLines`(276-281)/`levelCounts`(393-399) 五层 `useMemo` 全量重算（注释 FE-B-10）。
4. 清空：`handleClear` → `ConfirmDialog`（681-687）→ `handleClearConfirm`（288-391）→ GSAP 逐条动画 → `api.clearLogs()`（325 或 379）→ 重置 `rawLogs` 与 `logsKey`。

### Rust emit → listen → store → UI

事件链路本身在 [[desktop-frontend-hooks]]（`tauriApi.createEventListener` + `useEventListeners`）。本模块的一端是"渲染终点"：

- `useEventListeners.ts:271-289` 把 `auto-exit-countdown` 转成 `addToastWithAction`（带"取消退出"按钮）→ `ToastContainer`（`ToastContainer.tsx:67-76`）渲染按钮 → 点击回调 `api.cancelAutoExit()`。
- `useEventListeners.ts:327-332` 把 `network-quality-result` 写进 `useQualityStore` → 质量胶囊/面板经 `resolveQualityDisplay`（`latency.ts:59-69`）与 `getLatencyColor`（17-23）渲染。
- `useEventListeners.ts:335-345` 把 `update-available` 写进 `useQualityStore` → `TitleBar`（`TitleBar.tsx:64-65`）显示新版本浮标（106-137）。

### 主题 → DOM

`useThemeStore.subscribe`（`useThemeStore.ts:53-86`）是跨模块副作用：订阅 `isLightMode`/`themeName`/`customThemeColor`，切换 `<html>` 的 `dark` class、`data-light` 属性、`theme-*` class，并在 `custom` 主题下用 `hexToHsl`（`color.ts:1-33`）换算后 `setProperty` 写入 `--primary`/`--ring`/`--accent`/`--accent-foreground`（72-75）。所有 Tailwind 语义色（`bg-background`、`text-muted-foreground` 等）与 `var(--surface-main)`、`var(--surface-top)`、`var(--surface-side)`（`FluidBackground.tsx:6`、`TitleBar.tsx:94`、`RightPanel.tsx:214`）由此驱动。

### i18n

`main.tsx` 中 `import './i18n'` 触发初始化（`i18n/index.ts:8-26`）；语言切换入口是 `useConfigStore.setLanguage`（`useConfigStore.ts:176-180`），它同时写 `localStorage['app-language']` 并调 `i18next.changeLanguage`；`TitleBar.tsx:158` 只做 `zh ↔ en` 二值翻转。安卓端语言包为独立副本（见 [[android-frontend]]）。

## Connections

- [[desktop-frontend-hooks]]：本模块的全部数据来源（store/hook），以及 `LogPanel` 注入的 `tauriApiWithRetry`。
- [[desktop-monitor]]：`RightPanel` 的 `LogEntry` 列表、`NetworkQualityCapsule`、`StatusBar` 与质量色板 `QUALITY_CONFIG`（`network/constants.ts:1-11`）。
- [[desktop-network-core]]：`DockNav` 用 `resolveAdapterNames`/`AUTO_DETECT_ADAPTER`（`network/adapters.ts:3,15`）限定登录作用域，`RightPanel` 用 `AUTO_DETECT_ADAPTER` 判定主适配器回退。
- [[desktop-config]]：`shared/ui-constants.ts` 的 `PASSWORD_MASK` 与 `Config` 的掩码语义；`settings/constants.ts:6` 的 `DEFAULT_CONFIG`、`:69` 的 `VALID_THEMES` 被 store 消费。
- [[desktop-helper-update]]：`shared/types.ts` 的 `UpdateInfo`/`DownloadProgress`/`MirrorSource` 是更新命令与进度事件的载荷类型。
- [[desktop-app-lifecycle]]：`i18n/locales/*.json` 的 `notify.*`（自动退出/校园网退出倒计时）对应后端生命周期倒计时事件。
- [[desktop-auth]] / [[desktop-account-selfservice]]：`auth/*`、`account/*` 面板消费 `lib/latency.ts`、`RefreshButton`、`MascotFigure`、`ConfirmDialog`。
- [[desktop-platform]]：`FluidBackground`/`TitleBar` 使用的 `--surface-*` 变量与平台外观相关；`ErrorBoundary` 是平台无关兜底。
- [[android-frontend]]：安卓端为独立复刻树，本模块的中文文案/组件改动需按 AGENTS.md 第 3 条双端同步。

## Known Issues

1. **`MascotFigure` 未进桶文件**：`shared/index.ts:1-12` 不导出它，只能 `@/shared/MascotFigure` 深路径导入（`auth/AboutDialog.tsx:16`、`account/AccountPanel.tsx:23`、`settings/OnboardingWizard.tsx:26`、`shared/SponsorCard.tsx:5`、`shared/LogPanel.tsx:20`）。新增消费方若从 `@/shared` 导入会编译失败。`SponsorCard` 同样不在桶文件里。
2. **`PANEL_ORDER` 与 `PanelName` 不一致**：`lib/animations.ts:19` 的 `PANEL_ORDER` 只有 8 项，缺 `'selfservice'`；`getPanelDirection('selfservice', x)` 会命中 `fromIdx === -1` 分支 → 恒返回 `1`（向右滑），方向动画与实际导航顺序不符（`shared/ui-types.ts:2` 的 `PanelName` 含 9 项）。
3. **`PanelName` 含 `'selfservice'`，但 `lib/animations.ts` 与部分滤镜逻辑未覆盖**：`useInitialDataLoad.ts:16` 的 `VALID_PANELS` 由 `NAV_ITEMS` 派生（含 `selfservice`），恢复面板逻辑正常；但任何依赖 `PANEL_ORDER` 的新逻辑都要显式补 `selfservice`。
4. **`DialogOverlay` / `DialogPortal` 声明但未导出**（`dialog.tsx:9`、`11`，导出清单 `97-103` 不含两者）；`TooltipPortal` 同理（`tooltip.tsx:11`，导出清单 `31` 不含）。`DialogContent` 内部自建 Portal + Overlay（`32-33`），所以 `DialogContent` 无法脱离内建 Overlay 使用（自定义遮罩只能用外层容器）。
5. **`AnimatedCardConfig` 全部字段无消费方**（`animated-card.tsx:6-12`）：`animationConfig`/`glowIntensity`/`hoverScale`/`stiffness`/`damping`/`mass` 在仓库内仅出现在定义处，属规格化预留；实际倾斜由 `profile.enableTilt` 与硬编码的 `x*8`（86-87）决定。
6. **`RefreshButton` 的 `prevRefreshing` 有隐式假设**（`RefreshButton.tsx:18-25`）：`prevRefreshing.current = isRefreshing`（23）只在 `if` 条件为假时执行，而触发 shake 的分支（19-23）提前 `return`，因此一旦置为 `true` 就**永不回到 `false`**，条件退化为"仅看 `!isRefreshing`"。当前调用点都走 `false → true → false` 的完整循环，行为正确；但若组件以 `isRefreshing=true` 首次挂载，挂载时的 effect 会把 `prev` 置为 `true`，随后转为 `false` 时同样会 shake——语义上不属于"本次刷新刚结束"。
7. **`LogPanel.handleClearConfirm` 对 `ctx` 存在 TDZ 依赖**（`LogPanel.tsx:319-339` 的 `finishClear` 与 `354-360` 的 `onInterrupt` 都引用 340 行才声明的 `const ctx`）：当前两者只由 `onComplete`(349)、`failsafe`(367)、`onInterrupt`(354) 触发，都在 `gsap.context` 返回之后，因此安全；但任何新增的同步调用路径（例如提前在 340 行之前调用 `finishClear`）会抛 `ReferenceError: Cannot access 'ctx' before initialization`。清空失败时 `ctx.revert()`（335）也会 kill 冻结的 tween 并同步触发 `onInterrupt`，靠 `settled`（317、320）互斥。
8. **`LogPanel` 的 `logsKey` 用 `AnimatePresence key` 强制重挂载**（`LogPanel.tsx:589`，`setLogsKey` 在 329、383）：清空后会重建整棵日志列表，长列表下会有一次明显重排；同时 `lineKey` 追加 `idx`（596）意味着同内容行在刷新后 key 变化。
9. **`LogPanel` 的轮询没有退避**（`LogPanel.tsx:169-171`）：固定 5000ms 调 `get_logs`，后端日志很多时（1000 行档）会持续拉取大文本；只靠"内容未变化不 setState"（125）控制渲染成本，不控制 IPC 与 IO 成本。
10. **`RightPanel` 的空态呼吸动画在"空 → 非空 → 空"后会失效**：`useBreatheAnimation` 的 tween 在挂载时按 `ref.current` 建立（`useBreatheAnimation.ts:30-45`，effect deps 是 7 个原始值，52 行），而 `RightPanel.tsx:258-264` 的空态容器随 `logs.length` 挂载/卸载。日志从空变非空时容器卸载，但 tween 仍指向已脱离 DOM 的旧节点（GSAP 不会因节点脱离而自动 kill），一直空跑到 `RightPanel` 卸载才在 `useBreatheAnimation.ts:48-51` 被 kill；再回到空态时 effect 因 deps 未变**不会重跑**，新节点拿不到呼吸动画。同理 `useGlowAnimation`/`usePulseAnimation` 也有"tween 只在挂载时建立"的约束（`useGlowAnimation.ts:17-39`、`usePulseAnimation.ts:21-60`）。
11. **`RightPanel` 的日志分片**（`RightPanel.tsx:199-208`）：`logs.length > 50` 时前段用普通 `div`（267-290），只有最后 30 条是 `m.div`（291-323）；因此 `AnimatePresence` 的 `exit` 动画在长日志下不覆盖旧条目，"清空动画"（103-164）与"React 卸载"之间靠 `onClearLogs()` 清空数组（123）衔接。
12. **`DockNav` 的 `ICON_MAP` 是字符串到组件的硬编码表**（`DockNav.tsx:34-44`）：`NAV_ITEMS` 新增导航项时**必须**同时改这里与 `shared/ui-constants.ts:7-17`（并同步 `lib/animations.ts:19` 的 `PANEL_ORDER`），否则 `Icon` 为 `undefined` 导致渲染崩溃（60 行直接当作组件使用）。
13. **`MascotVariant` 与 `ToastMessage['mascot']` 是两套枚举**（`MascotFigure.tsx:5` vs `ui-types.ts:32`）：前者多 `welcome`/`empty`/`sponsor`、少 `alert`/`update`；`ToastContainer` 自行拼 `/girl/mascot-${mascot}.webp`（`ToastContainer.tsx:56`），若 `toast.mascot` 传入 `'update'`，需要存在 `public/girl/mascot-update.webp` 资源，否则静默裂图（无 `onError` 兜底）。
14. **`latency.ts` 的 `LatencyLevel` 未导出**（`latency.ts:4`）：`resolveQualityDisplay` 的返回类型里裸用该类型（64），外部若要对返回值做类型标注只能重复定义。
15. **`easing-config.ts` 的 `EASING_60HZ` 未导出**（`easing-config.ts:11`）：只能经 `getEasingConfig` 获取，测试无法直接断言两套基线的具体曲线。
16. **`lib/renderLiveness.ts` 曾在模块导入时启动常驻 rAF 循环（2026-09-13 已修复）**：原实现是模块级 `rafLoop` 无限递归（导入即启动、永不停止），任何 import 该模块的页面都会留下 pending `requestAnimationFrame`，Chromium 据此按刷新率持续派发 BeginFrame、合成器永不休眠（安卓省电改造 `266624b` 双端同改）。现改为按需短探测：`isRenderLoopAlive()`（`renderLiveness.ts:36-40`）距上次探测超过 `PROBE_REFRESH_MS=4_000`（`:18`）才经 `startProbe()`（`:24-34`）开一个 2 帧窗口，10s 停滞判定语义不变。**残留约束两条**：① 隐藏态直接返回 `true`（`:37`），调用方**仍必须先做可见性短路**（当前仅 `main.tsx:11` 与 `hooks/useHeartbeat.ts:3` 两个导入点，分别有 `isVisible` 分支 `main.tsx:91` 与 `paused` 分支 `useHeartbeat.ts:13`）；② 无 rAF 环境（SSR/部分测试环境）仍会因 `startProbe` 内的 `requestAnimationFrame` 抛错，但触发点从"导入即抛"推迟到"首次需要探测时"。
17. **`i18n` 初始化读 `localStorage` 未走 `safeStorage`**（`i18n/index.ts:17`）：用裸 `try { localStorage.getItem } catch`，与 `lib/utils.ts:22-29` 的 `safeStorage` 双写内存兜底不一致；`setLanguage`（`hooks/useConfigStore.ts:178`）走 `safeStorage`，两者键名相同（`app-language`）但降级行为不同。
18. **`i18n` 键数量完全对齐但无自动化校验**：实测 `zh.json` 与 `en.json` 各 719 条叶子键、键集合零缺失；仓库内没有测试或脚本守护这一点，新增词条若只加一端不会被拦住（安卓端另有独立副本，见 AGENTS.md 第 3 条双端同步要求）。
