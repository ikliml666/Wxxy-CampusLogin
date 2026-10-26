---
title: android-frontend-shared-ui
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
tags: [ui, shared, react, animation, android]
---

## Overview
`android-frontend-shared-ui` 模块是 Android 前端应用的核心 UI 资产库，封装了跨面板复用的原子组件、布局容器、数据展示逻辑以及全局常量与类型定义。该模块深度集成了 `gsap` 和 `framer-motion` 动画库，针对 Android WebView 环境（如 rAF 挂起、内存泄漏、MIUI 大字体布局）进行了特定的性能优化和缺陷修复。同时，模块引入了二次元吉祥物（Mascot）系统以增强用户交互体验，并提供了完整的日志面板（含清空动画）、错误边界和更新提示机制。所有公开导出集中在 `index.ts`（1-12）中统一对外暴露。

## Key Components

### 1. AnimatedNumber (数字动画)
**文件**: `android/frontend/src/shared/AnimatedNumber.tsx`
用于展示带有平滑过渡效果的数值，常用于延迟、速度等动态指标。

*   **Props 接口** (`AnimatedNumberProps`, 6-12):
    *   `value: number` (当前数值)
    *   `unit?: string` (单位后缀，默认 `'ms'`, 16)
    *   `decimals?: number` (小数位数，默认 0, 17)
    *   `className?: string` (CSS 类名, 18)
    *   `duration?: number` (动画时长 ms，可选，默认取自 profile, 19, 23)
*   **核心逻辑**:
    *   函数组件 `AnimatedNumber` (14-110)。
    *   使用 `useAnimationProfile` (21) 与 `useAnimationActive` (22) 控制动画档位与页面空闲降级。
    *   `resolvedDuration` = `duration ?? profile.numberDuration` (23)。
    *   Refs: `ref` (24), `prevRef` (25), `isFirstRender` (26), `objRef` (27), `valueQuickToRef` (28), `scaleQuickToRef` (29), `resetTimerRef` (30)。
    *   **初始化 effect** (32-59): 创建 `gsap.quickTo` 对 `objRef.current.value` 插值 (35-43)，并对 DOM `scale` 缩放 (44-48)。cleanup 中 kill 底层 tween (54-55) 防内存泄漏。
    *   **数值更新 effect** (61-96): 首次渲染直接写文本 (62-69)；后续变化若 `animActive` 且 quickTo 存在则播动画 (72-94)，否则直写 (73-77)。`economy` 档禁用 scale 弹跳 (83-84)。`gsap.delayedCall` 重置 scale (88-91) 前先 kill 旧 timer (87)。
    *   渲染 `<span>` 带 `tabular-nums` (98-109)。

### 2. ConfirmDialog (确认框)
**文件**: `android/frontend/src/shared/ConfirmDialog.tsx`
通用的破坏性或引导性操作确认弹窗。

*   **Props 接口** (`ConfirmDialogProps`, 12-21):
    *   `open: boolean`
    *   `title: string`
    *   `message: string`
    *   `onConfirm: () => void`
    *   `onCancel: () => void`
    *   `confirmLabel?: string` (自定义文案, 19)
    *   `confirmVariant?: 'default' | 'destructive'` (默认 `'destructive'`, 20-23)
*   **组件** `ConfirmDialog` (23-51):
    *   `confirming` state (26) 防重复提交。
    *   `useEffect` 监听 `open`，关闭时复位 `confirming` (29-31)。
    *   `Dialog` `onOpenChange` 在 confirming 时阻止取消 (33-36)。
    *   取消/确认按钮均 `disabled={confirming}` (43,44)，确认按钮 `onClick` 置 confirming 并调用 `onConfirm` (44-47)。

### 3. dailyMascot (每日吉祥物)
**文件**: `android/frontend/src/shared/dailyMascot.ts`
根据 UTC 日期轮换展示不同的二次元娘立绘，用于装饰目的。

*   **常量** `SIDE_MASCOT_POOL` (5-9): 14 张竖版立绘名称只读元组。
*   **函数** `pickDailyMascot(offset = 0): string` (11-15): 按 `Math.floor(Date.now()/86400000)` 加 offset 取模选图。
*   **Hook** `useDailyMascots(offsets: number[]): string[]` (18-34): 用 `key` 记忆 offsets (20)，`useState(compute)` 初始 (22)，`useEffect` 中立即计算一次并 `setInterval` 每分钟比对日期跨天切换 (23-33)。

### 4. ErrorBoundary (错误边界)
**文件**: `android/frontend/src/shared/ErrorBoundary.tsx`
基于 Class Component 实现的渲染错误捕获器。

*   **接口** `Props` (6-8): `children: ReactNode`。
*   **接口** `State` (10-13): `hasError: boolean`, `error: Error | null`。
*   **类** `ErrorBoundary` (15-52):
    *   `constructor(props)` (16-19) 初始化 state。
    *   `static getDerivedStateFromError(error)` (21-23) 返回 `{hasError:true, error}`。
    *   `componentDidCatch(error, errorInfo)` (25-27) DEV 下 `console.error`。
    *   `render()` (29-52) 出错时显示 `MascotFigure variant="alert"` (33) 和重载按钮 (38-46)，否则渲染 `children` (51)。

### 5. FluidBackground (流体背景)
**文件**: `android/frontend/src/shared/FluidBackground.tsx`
简单的占位背景组件，使用 CSS 变量 `--surface-main` (6)。

*   **组件** `FluidBackground()` (1-10): 绝对定位 `inset-0 z-0 pointer-events-none` 的 div。

### 6. index.ts (桶导出)
**文件**: `android/frontend/src/shared/index.ts`
导出 `LogPanel` (1), `ConfirmDialog` (2), `AnimatedNumber` (3), `RefreshButton`+`getRefreshIconClass` (4), `SegmentTabs` (5), `ToastContainer` (6), `FluidBackground` (7), `ErrorBoundary` (8)；以及 `types` (10), `ui-types` (11), `ui-constants` (12) 通配。

### 7. IspMark (ISP 标记)
**文件**: `android/frontend/src/shared/IspMark.tsx`
显示运营商或学校标识的小图标。

*   **常量** `ISP_MARK_SRC` (5-10): 映射 suffix 到 `/isp/*.webp`，`__default__` 为校徽。
*   **函数** `ispMarkSrc(suffix)` (12-15): null/undefined 返回 undefined，否则取映射或默认。
*   **组件** `IspMark` (memo, 18-42):
    *   `suffix` 为空或 `__default__` 时渲染白圆底 `<span>` 包裹 img (27-38)。
    *   其他运营商直接渲染 `<img>` (39-42)。

### 8. LogPanel (日志面板)
**文件**: `android/frontend/src/shared/LogPanel.tsx`
功能最复杂的组件，包含日志获取、过滤、搜索、清空动画及调试模式控制。

*   **Props 接口** `LogPanelProps` (27-37):
    *   `api`: `{ getLogs, clearLogs, getDebugMode, setDebugMode, getLogRetentionDays?, setLogRetentionDays? }`
    *   `addToast`: `(message, type, description?) => void`
*   **类型** `LogLevel` (39) = `'DEBUG'|'INFO'|'WARN'|'ERROR'`
*   **接口** `ParsedLogLine` (41-47): `timestamp, level, module, message, raw`
*   **常量**:
    *   `DEFAULT_LEVEL_CONFIG` (49)
    *   `LEVEL_CONFIG` (51-56) 各级样式/图标
    *   `LOG_LINE_REGEX` (58) 解析正则
    *   `isAndroidBuild` (62) 由 `VITE_PLATFORM==='android'` 决定两行布局
    *   `LINE_OPTIONS` (76-81)
    *   `MAX_DISPLAY_LINES = 200` (83)
*   **组件** `LogPanel` (memo, 85-677):
    *   **状态**: `rawLogs`(89), `isLoading`(90), `isClearing`(91), `lineCount`(92), `filterLevel`(93), `searchText`(94), `filterModule`(95), `debugMode`(96), `retentionDays`(97), `showClearConfirm`(98), `logsKey`(213)
    *   **Refs**: `scrollRef`(99), `isAutoScrollRef`(100), `isVisibleRef`(101), `fetchSeqRef`(102), `mountedRef`(103), `rawLogsRef`(105)
    *   **核心函数**:
        *   `fetchLogs(manual?)` (118-137): `fetchSeqRef` 竞态保护 (119)，手动才 loading (120)，内容变才 setState (126-129)。
        *   `toggleDebugMode` (179-191): 乐观更新调试模式。
        *   `handleRetentionChange` (193-204): 乐观更新保留天数，失败回滚。
        *   `handleClear` (272-275) 打开确认。
        *   `handleClearConfirm` (277-380): GSAP 可视区逐条删除动画 (329-351)，`onInterrupt` 复位 (343-349)，`setTimeout` failsafe (356-361)。
        *   `handleScroll` (215-219) 自动滚动判断。
    *   **Memo 选择器**:
        *   `parsedLines` 单遍解析 (221-232)
        *   `availableModules` (234-237)
        *   `filteredLines` 级别/模块/搜索过滤 (239-263)
        *   `displayedLines` 截断至 MAX (265-270)
        *   `levelCounts` (382-388)
    *   **渲染**: 工具栏 (392-463)，搜索/模块选择 (465-495)，级别筛选 (496-533)，日志滚动区 (535-655)，清空确认 `ConfirmDialog` (659-665)，背景看板娘 img (668-675)。

### 9. MascotFigure (吉祥物图)
**文件**: `android/frontend/src/shared/MascotFigure.tsx`
统一出口的图片组件，加载 `public/girl/mascot-*.webp`。

*   **常量** `SIZES` (3): `sm/md/lg` 对应 width class。
*   **类型** `MascotVariant` (5): `'portrait'|'welcome'|'empty'|'celebrate'|'sponsor'|'offline'|'alert'`
*   **组件** `MascotFigure` (7-26): 按 `variant`+`size` 渲染 lazy `<img>`。

### 10. RefreshButton (刷新按钮)
**文件**: `android/frontend/src/shared/RefreshButton.tsx`
带旋转动画和完成反馈的刷新按钮。

*   **接口** `RefreshButtonProps` (7-11): 扩展 ButtonHTMLAttributes，加 `isRefreshing`, `iconClassName?`, `showCheck?`。
*   **组件** `RefreshButton` (forwardRef, 13-72):
    *   `prevRefreshing` ref (15) 与 `shakeClass` state (16)。
    *   effect 在刷新结束时加 `refresh-shake` (18-28)。
    *   `AnimatePresence` 展示 Check (52-65)。
*   **函数** `getRefreshIconClass(isRefreshing, iconClassName?)` (74-81): 返回图标动画 class。

### 11. SegmentTabs (分段标签)
**文件**: `android/frontend/src/shared/SegmentTabs.tsx`
带动画滑块的分段选择器。

*   **接口** `TabItem` (5-11): `key, label, icon, color, bg`。
*   **接口** `SegmentTabsProps` (13-17): `tabs, activeKey, onTabChange`。
*   **组件** `SegmentTabs` (19-53): 用 `useId()` 生成 `layoutIdPrefix` (22-23)，活跃 tab 用 `m.div` 共享 `layoutId={`${layoutIdPrefix}-activeTab`}` (42) 实现滑块。

### 12. SponsorCard (赞助卡)
**文件**: `android/frontend/src/shared/SponsorCard.tsx`
移动端适配的赞助弹窗，使用 Radix Dialog。

*   **接口** `SponsorCardProps` (6-9): `open, onClose`。
*   **组件** `SponsorCard` (13-51): 居中 Dialog 展示微信/支付宝二维码 (34-48) 与 `MascotFigure variant="sponsor"` (24)。

### 13. ToastContainer (Toast 容器)
**文件**: `android/frontend/src/shared/ToastContainer.tsx`
管理全局 Toast 消息的容器。

*   **接口** `ToastContainerProps` (10-13): `toasts: ToastMessage[], onRemove`。
*   **常量** `TOAST_MASCOTS` (16-21) 类型→变体映射；`TOAST_STYLES` (23-28) 类型→背景。
*   **组件** `ToastContainer` (memo, 30-96):
    *   `enterTransition` 根据 `profile.tier==='economy'` 简化 (34-36)。
    *   `AnimatePresence mode="popLayout"` 渲染 toast (40-94)。
    *   `popIn` 仅 celebrate 非 economy 有 spring 弹入 (44,62-63)。

### 14. UpdateAvailableDialog (更新提示对话框)
**文件**: `android/frontend/src/shared/UpdateAvailableDialog.tsx`
新版本发现时的提示弹窗。

*   **接口** `UpdateAvailableDialogProps` (14-16): `onGoUpdate`。
*   **组件** `UpdateAvailableDialog` (18-38):
    *   从 `useQualityStore` 读 `updatePromptOpen`(20), `latestVersion`(21), `releaseNotes`(22), `setUpdatePromptOpen`(23)。
    *   Dialog 提供“稍后”/“立即前往” (33-34)。

### 15. Types & Constants (共享类型/常量)

**文件** `android/frontend/src/shared/types.ts`
*   `UpdateAvailableData` (1-5): `hasUpdate`, `latestVersion`, `releaseNotes?`
*   `UpdateInfo` (7-17): `hasUpdate`, `latestVersion`, `releaseNotes`, `assets: {name,url,size}[]`, `sha256Checksum?`, `lastCheckError: string|null`, `lastCheckTime: number|null`
*   `DownloadProgress` (19-24): `downloaded`, `total`, `speed`, `percent`
*   `MirrorSource` (26-30): `name`, `url`, `description`

**文件** `android/frontend/src/shared/ui-types.ts`
*   `StatusState` (1): `'loading'|'online'|'offline'|'error'`
*   `PanelName` (2): 面板枚举
*   `ThemeName` (3): 主题枚举
*   `GlassLevel` (5): `'off'|'standard'|'ultra'`
*   `LogType` (6): `'info'|'success'|'error'|'warning'`
*   `LogEntry` (8-15): `id`, `time`, `message`, `type`, `count?`
*   `ToastMessage` (17-29): `id`, `title`, `description?`, `type`, `duration?`, `mascot?: 'portrait'|'celebrate'|'offline'|'alert'|'update'`, `action?: {label, onClick}`
*   `AdapterDisabledWarningData` (31-34): `name`, `message`
*   `AutoExitCountdownData` (36-39): `delay`, `shortcut`
*   `SaveConfigResult` (41-45): `success`, `message?`, `data?`

**文件** `android/frontend/src/shared/ui-constants.ts`
*   `MAX_LOG_ENTRIES = 300` (1)
*   `APP_VERSION = '2.4.2'` (2)
*   `APP_NAME = '校园网登录助手'` (3)
*   `PASSWORD_MASK = '***'` (4)
*   `NAV_ITEMS` (7-16): 数组含 `id, labelKey, icon, shortcut` (dashboard/account/selfservice/monitor/quality/speedtest/settings/log)

## Architecture

```text
┌──────────────────────────────────────────────┐
│        android-frontend-shared-ui           │
│  Constants(TS)  Types(TS)  Assets(webp)     │
│       │             │           │           │
│  ┌────▼─────────────▼───────────▼────────┐  │
│  │           Shared Components          │  │
│  │ Layout: ErrorBoundary,FluidBg,       │  │
│  │   SegmentTabs,RefreshButton,IspMark  │  │
│  │ Feedback: ConfirmDialog,SponsorCard, │  │
│  │   ToastContainer,UpdateAvailableDlg  │  │
│  │ Data: AnimatedNumber,LogPanel,       │  │
│  │   MascotFigure,dailyMascot           │  │
│  └───────────────┬──────────────────────┘  │
│   Animation: GSAP + Framer Motion engines  │
└──────────────────────────────────────────────┘
```

## Data Flow

1.  **日志流 (LogPanel)**:
    *   `api.getLogs()` (122) → `rawLogs` state (128) / `rawLogsRef` (127) → `parsedLines` (memo, 221-232) → `filteredLines` (memo, 239-263) → `displayedLines` (memo, 265-270)。
    *   轮询定时器每 5s 调用 `fetchLogs()` (170-172)，受 `isVisibleRef` (IntersectionObserver, 147) 和 `document.hidden` (167-169) 门控。
    *   用户操作 (过滤/搜索/清空) 触发对应 state 变更，重算 memo 并自动滚动 (206-211)。
    *   清空流程：`handleClearConfirm` (277) 用 GSAP 对视口内 `.log-line` 做 stagger 删除 (330-350)，成功后 `api.clearLogs()` 并置空 `rawLogs` (314-318)。

2.  **更新提示流 (UpdateAvailableDialog)**:
    *   `useQualityStore` 设置 `updatePromptOpen=true` (20) → Dialog 弹出 (26)。
    *   用户点击 "Go" → `onGoUpdate()` 并 `setOpen(false)` (34)。

3.  **每日吉祥物流 (dailyMascot)**:
    *   `useDailyMascots` Hook 启动 `setInterval` 60s (25-30)。
    *   计算 UTC 天数 → `pickDailyMascot(offset)` (11-15) → 更新 `mascots` state (26-29)。
    *   消费者（如背景装饰）通过 `MascotFigure` 渲染对应 webp。

## Connections
*   [[android-frontend-hooks]]: 依赖 `useAnimationProfile` (AnimatedNumber 21, LogPanel 86, ToastContainer 32), `usePageIdle` (AnimatedNumber 22), `useQualityStore` (UpdateAvailableDialog 20-23)。
*   [[android-frontend-components-ui]]: 依赖基础 UI 组件 `Button` (ConfirmDialog 2, LogPanel 3, etc.), `Dialog` (ConfirmDialog 3-9, SponsorCard 3, UpdateAvailableDialog 4-10), `Card` (LogPanel 1-2), `Badge` (LogPanel 4), `Select` (LogPanel 5)。
*   [[android-frontend-lib-utils]]: 依赖 `cn` (IspMark 2, LogPanel 17, etc.), `extractErrorMessage` (LogPanel 17, 132, 202, 325)。
*   [[android-frontend-assets]]: 引用 `public/girl/mascot-*.webp` (MascotFigure 18, ToastContainer 58, LogPanel 669), `public/isp/*.webp` (IspMark 5-10)。

## Known Issues

1.  **GSAP Tween 内存泄漏 (AnimatedNumber)**:
    *   **位置**: `android/frontend/src/shared/AnimatedNumber.tsx` 第 51-53 行注释。
    *   **描述**: 历史缺陷，`quickTo` 底层 tween 在组件卸载后仍存活，持续占用 CPU 并尝试写入 null ref。
    *   **修复**: cleanup 中调用 `gsap.killTweensOf(objRef.current, 'value')` 与 `gsap.killTweensOf(el, 'scale')` (54-55)。

2.  **ConfirmDialog 重复提交**:
    *   **位置**: `android/frontend/src/shared/ConfirmDialog.tsx` 第 25 行注释。
    *   **描述**: 快速双击可能导致两次破坏性操作。
    *   **修复**: 引入 `confirming` state 并禁用按钮 (26, 44-47)。

3.  **ConfirmDialog 按钮锁定**:
    *   **位置**: `android/frontend/src/shared/ConfirmDialog.tsx` 第 27-28 行注释。
    *   **描述**: `onConfirm` 为 async 时，固定定时器可能在慢网络下提前解禁按钮。
    *   **修复**: 改为由 `open` prop 驱动 `useEffect` 复位 `confirming` (29-31)。

4.  **Scale 动画跳变 (AnimatedNumber)**:
    *   **位置**: `android/frontend/src/shared/AnimatedNumber.tsx` 第 85-86 行注释。
    *   **描述**: `delayedCall` 覆盖前不 kill 旧 call，导致快速连续变化时回调堆积，中途弹回 1 造成跳变。
    *   **修复**: 注册新 call 前 `resetTimerRef.current.kill()` (87)。

5.  **LogPanel 清空动画卡死**:
    *   **位置**: `android/frontend/src/shared/LogPanel.tsx` 第 339-342 行注释。
    *   **描述**: Tween 被中断时 `onComplete` 不执行，`isClearing` 残留导致清空按钮永久禁用。
    *   **修复**: 增加 `onInterrupt` 回调复位状态并 `ctx.revert()` (343-349)。

6.  **Android WebView rAF 停摆**:
    *   **位置**: `android/frontend/src/shared/LogPanel.tsx` 第 352-355 行注释。
    *   **描述**: 后台/遮挡时 rAF 停止，tween 冻结，`onComplete/onInterrupt` 均不触发，且 framer-motion 重渲染可能拉回隐藏条目。
    *   **修复**: 增加 `setTimeout` failsafe 兜底 (356-361)，先 `finishClear()` 再 `ctx.revert()`。

7.  **SegmentTabs 布局动画冲突**:
    *   **位置**: `android/frontend/src/shared/SegmentTabs.tsx` 第 20-21 行注释。
    *   **描述**: 多处并存时共享相同 `layoutId` 会导致 framer-motion 跨实例错误共享动画。
    *   **修复**: 使用 `useId()` 生成实例前缀 `layoutIdPrefix` (22-23)，拼接至 layoutId (42)。

8.  **LogPanel 渲染性能 (FE-B-10)**:
    *   **位置**: `android/frontend/src/shared/LogPanel.tsx` 第 104 行注释。
    *   **描述**: 5s 轮询若每次都触发 setState，会导致 `parsedLines`/`filteredLines`/`displayedLines`/`levelCounts` 四层 useMemo 串行全量重算。
    *   **修复**: 使用 `rawLogsRef` 比较内容，未变化时跳过 setState (126-129)；另将解析合并为单遍遍历 (221-232) 减少中间数组。
