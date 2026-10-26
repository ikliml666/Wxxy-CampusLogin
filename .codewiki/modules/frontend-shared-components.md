---
title: frontend-shared-components
type: module
source_files:
  - android/frontend/src/shared/AnimatedNumber.tsx
  - android/frontend/src/shared/ConfirmDialog.tsx
  - android/frontend/src/shared/dailyMascot.ts
  - android/frontend/src/shared/ErrorBoundary.tsx
  - android/frontend/src/shared/FluidBackground.tsx
  - android/frontend/src/shared/index.ts
  - android/frontend/src/shared/IspMark.tsx
  - android/frontend/src/shared/LogPanel.tsx
  - android/frontend/src/shared/MascotFigure.tsx
  - android/frontend/src/shared/RefreshButton.tsx
  - android/frontend/src/shared/SegmentTabs.tsx
  - android/frontend/src/shared/SponsorCard.tsx
  - android/frontend/src/shared/ToastContainer.tsx
  - android/frontend/src/shared/types.ts
  - android/frontend/src/shared/ui-constants.ts
  - android/frontend/src/shared/ui-types.ts
  - android/frontend/src/shared/UpdateAvailableDialog.tsx
tags: [frontend, shared, ui, react, gsap, framer-motion]
---

## Overview
`frontend-shared-components` 模块是 Android 前端应用的核心 UI 资产库，负责跨模块共享通用组件、类型定义及常量。该模块集成了复杂的动画逻辑（GSAP 与 Framer Motion）、状态管理辅助以及特定的业务 UI 元素（如二次元看板娘、运营商标识）。它不依赖具体的业务页面逻辑，而是提供原子化的 UI 构建块，供 Dashboard、Settings、Monitor 等上层模块组合使用。当前版本 `APP_VERSION` 为 `'2.4.2'`（见 `ui-constants.ts` 行2）。

## Key Components

### AnimatedNumber (`android/frontend/src/shared/AnimatedNumber.tsx`)
数字滚动动画组件，用于平滑展示数值变化。
- **接口 `AnimatedNumberProps`** (行6-12):
  - `value: number` (行7) – 目标数值。
  - `unit?: string` (行8) – 单位后缀，默认 `'ms'`。
  - `decimals?: number` (行9) – 小数位数，默认 `0`。
  - `className?: string` (行10) – CSS 类名。
  - `duration?: number` (行11) – 动画时长，未提供则使用 `useAnimationProfile` 的 `numberDuration`。
- **导出组件 `AnimatedNumber`** (行14-109): `export function AnimatedNumber({...}: AnimatedNumberProps)`
- **实现细节**:
  - 使用 `useAnimationProfile` (行21) 与 `useAnimationActive` (行22) 钩子。
  - `gsap.quickTo` 驱动 `objRef.current.value` 并 `onUpdate` 写 DOM 文本 (行35-43)；`scaleQuickToRef` 驱动缩放 (行44-48)。
  - `economy` 档禁用 scale 弹跳：行83 `if (profile.tier !== 'economy')` 才执行 `scaleQuickToRef.current(1.08)` 及 reset 调用，否则仅数字滚动降级 (行82-92)。
  - 卸载清理 (行49-58) 中 `gsap.killTweensOf` 杀底层 tween 防空转 (行54-55)。
  - 快速连续变化：行87-91 先 `resetTimerRef.current.kill()` 再注册 `gsap.delayedCall`，防回调堆积跳变。

### ConfirmDialog (`android/frontend/src/shared/ConfirmDialog.tsx`)
通用确认对话框，防止破坏性操作重复触发。
- **接口 `ConfirmDialogProps`** (行12-21):
  - `open: boolean` (行13)
  - `title: string` (行14)
  - `message: string` (行15)
  - `onConfirm: () => void` (行16)
  - `onCancel: () => void` (行17)
  - `confirmLabel?: string` (行19) – 自定义确认文案（引导类用正色）。
  - `confirmVariant?: 'default' | 'destructive'` (行20) – 默认 `'destructive'` 红色。
- **导出组件 `ConfirmDialog`** (行23-51): `export function ConfirmDialog({...}: ConfirmDialogProps)`
- **关键逻辑**:
  - `confirming` 状态 (行26) 异步操作中禁用按钮防双击 (历史缺陷修复注释行25)。
  - `useEffect` (行29-31) 监听 `open` 变化复位 `confirming`，替代固定 400ms 定时器 (行27-28 注释)。
  - `Dialog` `onOpenChange` (行33-36) 在 `confirming` 时拦截取消。

### dailyMascot (`android/frontend/src/shared/dailyMascot.ts`)
每日轮换看板娘逻辑。
- **常量 `SIDE_MASCOT_POOL`** (行5-9): `export const` 14 张立绘键数组（`'side-wave'` 等）。
- **函数 `pickDailyMascot(offset = 0): string`** (行11-15): 按 UTC 天数 `Math.floor(Date.now() / 86_400_000)` 加偏移取模返回。
- **Hook `useDailyMascots(offsets: number[]): string[]`** (行18-34): `useState(compute)` (行22)，`useEffect` (行23-33) 每 60_000 ms 重算并跨天切换；`key` 由 `offsets.join(',')` (行20) 作依赖。

### ErrorBoundary (`android/frontend/src/shared/ErrorBoundary.tsx`)
React 错误边界类组件。
- **接口 `Props`** (行6-8): `children: ReactNode`。
- **接口 `State`** (行10-13): `hasError: boolean`, `error: Error | null`。
- **导出类 `ErrorBoundary`** (行15-52): `export class ErrorBoundary extends Component<Props, State>`
  - `constructor(props: Props)` (行16-19) 初始化 state。
  - `static getDerivedStateFromError(error: Error): State` (行21-23) 捕获错误。
  - `componentDidCatch(error: Error, errorInfo: React.ErrorInfo)` (行25-27) DEV 环境 `console.error`。
  - `render()` (行29-52) 出错时显 `MascotFigure` variant `"alert"` (行33) 与重载按钮 (行38-46)，正常返 `children`。

### FluidBackground (`android/frontend/src/shared/FluidBackground.tsx`)
流体背景占位符。
- **导出函数 `FluidBackground`** (行1-9): 返回绝对定位 `div` (行3-8) 背景 `var(--surface-main)`，`pointer-events-none`。

### index.ts (`android/frontend/src/shared/index.ts`)
导出枢纽。
- 行1-8 重导出 `LogPanel`, `ConfirmDialog`, `AnimatedNumber`, `RefreshButton` (含 `getRefreshIconClass`), `SegmentTabs`, `ToastContainer`, `FluidBackground`, `ErrorBoundary`。
- 行10 `export * from './types'`，行11 `export * from './ui-types'`，行12 `export * from './ui-constants'`。

### IspMark (`android/frontend/src/shared/IspMark.tsx`)
运营商标识徽标。
- **常量 `ISP_MARK_SRC`** (行5-10): `Record<string,string>` 映射后缀到 `/isp/*.webp`；`__default__` 与 `''` 校徽，`@telecom`/`@unicom`/`@cmcc` 运营商图。
- **函数 `ispMarkSrc(suffix: string | null | undefined): string | undefined`** (行12-15): 映射后缀，未知返默认。
- **导出组件 `IspMark`** (行18-41): `export const IspMark = memo(...)`；校徽 (`suffix === '' || '__default__'`) 白圆底 `span` 包 `img` (行27-38)，其他原色 `img` (行39-41)。

### LogPanel (`android/frontend/src/shared/LogPanel.tsx`)
系统日志查看面板，功能最复杂组件。
- **接口 `LogPanelProps`** (行27-37): `api` (含 `getLogs`, `clearLogs`, `getDebugMode`, `setDebugMode`, `getLogRetentionDays?`, `setLogRetentionDays?`)，`addToast`。
- **类型 `LogLevel`** (行39): `'DEBUG' | 'INFO' | 'WARN' | 'ERROR'`。
- **接口 `ParsedLogLine`** (行41-47): `timestamp`, `level`, `module`, `message`, `raw`。
- **常量 `DEFAULT_LEVEL_CONFIG`** (行49) 与 `LEVEL_CONFIG`** (行51-56) 级别样式/图标。
- **常量 `LOG_LINE_REGEX`** (行58) 解析正则。
- **常量 `isAndroidBuild`** (行62): `import.meta.env.VITE_PLATFORM === 'android'`。
- **函数 `parseLogLine(line: string): ParsedLogLine | null`** (行64-74) 单遍正则解析。
- **常量 `LINE_OPTIONS`** (行76-81) 与 `MAX_DISPLAY_LINES = 200` (行83)。
- **导出组件 `LogPanel`** (行85-677): `export const LogPanel = memo(function LogPanel({api, addToast})`
  - 状态：`rawLogs` (行89), `isLoading` (行90), `isClearing` (行91), `lineCount` (行92 默认200), `filterLevel` (行93), `searchText` (行94), `filterModule` (行95), `debugMode` (行96), `retentionDays` (行97 默认7), `showClearConfirm` (行98) 及 refs (行99-105)。
  - `fetchLogs` (行118-137) 手动/轮询获取，用 `fetchSeqRef`+`rawLogsRef` 跳重复 setState (行126-128)。
  - 轮询 `useEffect` (行164-177) 结合 `IntersectionObserver` (行143-152) 与 `visibilitychange` (行167-169) 门控，5s 间隔。
  - `toggleDebugMode` (行179-191), `handleRetentionChange` (行193-204) 乐观更新。
  - 管道：`parsedLines` (行221-232) 单遍解析，`availableModules` (行234-237), `filteredLines` (行239-263), `displayedLines` (行265-270) 截最新200。
  - `handleClear` (行272-275) 与 `handleClearConfirm` (行277-380)：GSAP 逐条删可见日志 (行282-365)，动态 `stagger` (行305)，`onInterrupt` 复位 (行343-349)，`failsafe` setTimeout 兜底 (行356-361)；无 DOM 直接清 (行368-379)。
  - `levelCounts` (行382-388)。
  - 渲染：工具栏 (行392-461)，搜索/模块筛选 (行465-495)，级别按钮 (行496-533)，滚动容器 (行535-655) 用 `AnimatePresence` (行561) 与 `m.div` (行607-649) 或 `div` (行570-605)（条数≤30 启动画，行565）；Android 下消息独占行 (行575,603)。底部背景娘 `img` (行668-675)。嵌 `ConfirmDialog` (行659-665)。

### MascotFigure (`android/frontend/src/shared/MascotFigure.tsx`)
统一看板娘图像组件。
- **常量 `SIZES`** (行3): `{ sm: 'w-20', md: 'w-28', lg: 'w-40' }`。
- **类型 `MascotVariant`** (行5): `'portrait' | 'welcome' | 'empty' | 'celebrate' | 'sponsor' | 'offline' | 'alert'`。
- **导出函数 `MascotFigure`** (行7-25): 渲染 `<img src={'/girl/mascot-${variant}.webp'}>` 懒加载。

### RefreshButton (`android/frontend/src/shared/RefreshButton.tsx`)
带刷新动画和成功反馈按钮。
- **接口 `RefreshButtonProps`** (行7-11): 扩 `ButtonHTMLAttributes`，含 `isRefreshing: boolean`, `iconClassName?: string`, `showCheck?: boolean`。
- **导出组件 `RefreshButton`** (行13-72): `React.forwardRef`；刷新结束触 `refresh-shake` 类 (行23-27)；`AnimatePresence` 显成功对勾 (行52-65)。
- **导出函数 `getRefreshIconClass(isRefreshing, iconClassName?)`** (行74-81): 返回图标类，支持 `group-hover:rotate-180` 或 `animate-refresh-spin`。

### SegmentTabs (`android/frontend/src/shared/SegmentTabs.tsx`)
带滑动指示器分段标签页。
- **接口 `TabItem`** (行5-11): `key`, `label`, `icon`, `color`, `bg`。
- **接口 `SegmentTabsProps`** (行13-17): `tabs`, `activeKey`, `onTabChange`。
- **导出函数 `SegmentTabs`** (行19-53): `useId()` 生成 `layoutIdPrefix` (行22) 防多实例冲突；`m.div` 配 `layoutId` (行41-45) 滑块。

### SponsorCard (`android/frontend/src/shared/SponsorCard.tsx`)
移动端赞助弹窗。
- **接口 `SponsorCardProps`** (行6-9): `open: boolean`, `onClose: () => void`。
- **导出函数 `SponsorCard`** (行13-51): Radix `Dialog` (行16-21) 居中 `max-w-[340px]`；显 `MascotFigure` `"sponsor"` (行24) 与微信/支付宝码 `img` (行34-47)。

### ToastContainer (`android/frontend/src/shared/ToastContainer.tsx`)
全局通知容器。
- **接口 `ToastContainerProps`** (行10-13): `toasts: ToastMessage[]`, `onRemove: (id: string) => void`。
- **常量 `TOAST_MASCOTS`** (行16-21): 类型默认娘变体（`info:'portrait'`, `success:'celebrate'`, `error:'offline'`, `warning:'alert'`）。
- **常量 `TOAST_STYLES`** (行23-28): 类型背景样式。
- **导出组件 `ToastContainer`** (行30-97): `memo`；`useAnimationProfile` 取 `tier` (行33)；economy 用简过渡 (行34-35) 否则 spring (行36)；`celebrate` 娘首帧弹入仅非 economy (行44)；渲 `m.div` 列表 (行46-91) 含 `m.img` 娘 (行57-65)。

### UpdateAvailableDialog (`android/frontend/src/shared/UpdateAvailableDialog.tsx`)
新版本提醒弹窗。
- **接口 `UpdateAvailableDialogProps`** (行14-16): `onGoUpdate: () => void`。
- **导出函数 `UpdateAvailableDialog`** (行18-38): 从 `useQualityStore` 取 `updatePromptOpen` (行20), `latestVersion` (行21), `releaseNotes` (行22)；Radix Dialog 展示。

### types.ts (`android/frontend/src/shared/types.ts`)
- **接口 `UpdateAvailableData`** (行1-5): `hasUpdate: boolean`, `latestVersion: string`, `releaseNotes?: string`。
- **接口 `UpdateInfo`** (行7-17): `hasUpdate`, `latestVersion`, `releaseNotes`, `assets: {name,url,size}[]`, `sha256Checksum?`, `lastCheckError: string | null`, `lastCheckTime: number | null`。
- **接口 `DownloadProgress`** (行19-24): `downloaded`, `total`, `speed`, `percent`。
- **接口 `MirrorSource`** (行26-29): `name`, `url`, `description`。

### ui-constants.ts (`android/frontend/src/shared/ui-constants.ts`)
- **常量 `MAX_LOG_ENTRIES = 300`** (行1)
- **常量 `APP_VERSION = '2.4.2'`** (行2)
- **常量 `APP_NAME = '校园网登录助手'`** (行3)
- **常量 `PASSWORD_MASK = '***'`** (行4)
- **常量 `NAV_ITEMS`** (行7-15): 数组含 `id` (如 `'dashboard'`), `labelKey`, `icon`, `shortcut`。

### ui-types.ts (`android/frontend/src/shared/ui-types.ts`)
- **类型 `StatusState`** (行1): `'loading' | 'online' | 'offline' | 'error'`
- **类型 `PanelName`** (行2): 联合面板名。
- **类型 `ThemeName`** (行3): 主题名联合。
- **类型 `GlassLevel`** (行5): `'off' | 'standard' | 'ultra'`
- **类型 `LogType`** (行6): `'info' | 'success' | 'error' | 'warning'`
- **接口 `LogEntry`** (行8-15): `id: string`, `time: string`, `message: string`, `type: LogType`, `count?: number` (重复折叠累计)。
- **接口 `ToastMessage`** (行17-29): `id`, `title`, `description?`, `type: LogType`, `duration?`, `mascot?: 'portrait'|'celebrate'|'offline'|'alert'|'update'`, `action?: { label: string; onClick: () => void }`。
- **接口 `AdapterDisabledWarningData`** (行31-34): `name`, `message`。
- **接口 `AutoExitCountdownData`** (行36-39): `delay`, `shortcut`。
- **接口 `SaveConfigResult`** (行41-44): `success`, `message?`, `data?`。

## Architecture

```text
┌──────────────────────────────────────────────────────┐
│                   frontend-shared-components         │
├──────────────────────────────────────────────────────┤
│  [index.ts] (Export Hub)                             │
│       │                                              │
│       ├─► [AnimatedNumber.tsx] (GSAP Animation)      │
│       ├─► [ConfirmDialog.tsx] (State Guard)          │
│       ├─► [LogPanel.tsx] (Complex Logic & GSAP)      │
│       │        └─► Uses [ConfirmDialog],             │
│       │           [MascotFigure], [AnimatedCard]     │
│       ├─► [ToastContainer.tsx] (Framer Motion)      │
│       │        └─► Uses [MascotFigure],              │
│       │           [ToastMessage] from ui-types       │
│       ├─► [RefreshButton.tsx] (Framer Motion)        │
│       ├─► [SegmentTabs.tsx] (Layout Animation)       │
│       ├─► [MascotFigure.tsx] (Static Asset)          │
│       ├─► [dailyMascot.ts] (Date-based Logic)        │
│       ├─► [IspMark.tsx] (Memoized Image)             │
│       ├─► [ErrorBoundary.tsx] (Class Component)      │
│       ├─► [SponsorCard.tsx] (Dialog Wrapper)         │
│       ├─► [UpdateAvailableDialog.tsx] (Store Hook)   │
│       │        └─► Uses [useQualityStore]            │
│       ├─► [FluidBackground.tsx] (Placeholder)        │
│       └─► [types.ts] / [ui-types.ts] / [ui-constants.ts] │
└──────────────────────────────────────────────────────┘
```

## Data Flow
1. **日志流 (LogPanel)**:
   - `api.getLogs(lineCount)` (行122) 获原始串 → `parseLogLine()` (行64) 单遍解析为 `ParsedLogLine[]` (行221-232) → `filteredLines` 应用级别/模块/搜索筛选 (行239-263) → `displayedLines` 截最新200条 (行265-270) → 渲 `m.div` (行607) 或 `div` (行570)，动画仅当条数≤30 (行565)。
   - 用户点清除 → `handleClear` (行272) 开 `ConfirmDialog` → `handleClearConfirm` (行277) → GSAP 逐条隐 DOM (行282-365) → `api.clearLogs()` (行314) → 重置 `rawLogs` 与 `logsKey` (行317-318)。
2. **通知流 (ToastContainer)**:
   - 外部注入 `ToastMessage[]` 到 `toasts` prop → `ToastContainer` 遍历渲 (行41) → 每 Toast 按 `type` 匹配 `TOAST_MASCOTS` (行42) 显对应娘 (`/girl/mascot-${mascot}.webp`, 行58)；economy 档降级过渡 (行33-36)。
3. **每日娘流 (dailyMascot)**:
   - `useDailyMascots(offsets)` (行18) 算 UTC 天数索引 → 返数组 (行21) → 子组件 (如 `LogPanel` 底 `img` 行668) 静态展示，Hook 内每60秒重算跨天切 (行25-30)。

## Connections
- [[quality]]: `UpdateAvailableDialog` 直接从 `useQualityStore` 读更新状态 (行20-23)。
- [[animation-profile]]: `AnimatedNumber` (行21), `LogPanel` (行86), `ToastContainer` (行32) 均用 `useAnimationProfile` 获取档位/缓动。
- [[ui-dialog]]: `ConfirmDialog` (行3-9 导入), `SponsorCard` (行3 导入), `UpdateAvailableDialog` (行4-10 导入) 依赖 Radix UI Dialog。
- [[i18n]]: 所有文本组件用 `useTranslation` (如 `ConfirmDialog` 行24, `LogPanel` 行87, `ToastContainer` 行31)。
- [[page-idle]]: `AnimatedNumber` 用 `useAnimationActive` (行4 导入) 判动画激活。
- [[mascot-assets]]: `MascotFigure`, `IspMark`, `LogPanel` 底均引 `/girl/*.webp` 或 `/isp/*.webp` 静态资源。

## Known Issues
1. **GSAP Tween 泄漏风险 (AnimatedNumber.tsx)**:
   - 位置: `AnimatedNumber.tsx` 行51-55（注释历史缺陷），清理函数行49-58 中已加 `gsap.killTweensOf(objRef.current, 'value')` (行54) 与 `gsap.killTweensOf(el, 'scale')` (行55)。StrictMode 下需确保 cleanup 执行。
2. **快速连续变化的 Scale 跳变 (AnimatedNumber.tsx)**:
   - 位置: `AnimatedNumber.tsx` 行85-87（注释历史缺陷），原 `delayedCall` 覆盖前不 kill 旧 call 致堆积；现修复为行87 `if (resetTimerRef.current) resetTimerRef.current.kill()` 再注册 (行88-91)。
3. **ConfirmDialog 按钮状态复位与重复提交 (ConfirmDialog.tsx)**:
   - 位置: `ConfirmDialog.tsx` 行25（历史缺陷双击两次），行27-28（固定400ms定时器提前解禁）；现用 `confirming` 状态 (行26) 禁用按钮，并由 `useEffect` 监听 `open` (行29-31) 复位。
4. **GSAP rAF 冻结导致 Clear 卡死 (LogPanel.tsx)**:
   - 位置: `LogPanel.tsx` 行339-342（tween 中断 onComplete 不执行），行352-355（安卓 WebView 后台 rAF 停摆）；现增 `onInterrupt` 复位 `isClearing` (行343-349) 与 `failsafe` setTimeout 兜底 (行356-361)。
5. **SegmentTabs 布局动画冲突 (SegmentTabs.tsx)**:
   - 位置: `SegmentTabs.tsx` 行20-21（注释多实例 layoutId 共享）；用 `useId()` 生成 `layoutIdPrefix` (行22) 拼至 `layoutId` (行42) 解决。
6. **Android 大字体布局挤压 (LogPanel.tsx)**:
   - 位置: `LogPanel.tsx` 行60-62（注释 MIUI 大字体单行 flex 压消息），`isAndroidBuild` 时日志行改两行 (`flex-col`, 行575,603)，消息独占整行。
7. **RefreshButton ref 归位 (RefreshButton.tsx)**:
   - 位置: `RefreshButton.tsx` 行19-20（注释 ref 滞留），`useEffect` 无条件同步 `prevRefreshing.current = isRefreshing` (行22) 避免隐式依赖。
