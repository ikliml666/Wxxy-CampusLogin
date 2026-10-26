---
title: frontend-shared-ui
type: module
source_files:
  - android/frontend/src/shared/AnimatedNumber.tsx
  - android/frontend/src/shared/ConfirmDialog.tsx
  - android/frontend/src/shared/dailyMascot.ts
  - android/frontend/src/shared/ErrorBoundary.tsx
  - android/frontend/src/shared/FluidBackground.tsx
  - android/frontend/src/shared/IspMark.tsx
  - android/frontend/src/shared/LogPanel.tsx
  - android/frontend/src/shared/MascotFigure.tsx
  - android/frontend/src/shared/RefreshButton.tsx
  - android/frontend/src/shared/SegmentTabs.tsx
  - android/frontend/src/shared/SponsorCard.tsx
  - android/frontend/src/shared/ToastContainer.tsx
  - android/frontend/src/shared/UpdateAvailableDialog.tsx
  - android/frontend/src/shared/index.ts
  - android/frontend/src/shared/types.ts
  - android/frontend/src/shared/ui-constants.ts
  - android/frontend/src/shared/ui-types.ts
tags: [ui, shared, primitives, animation, mascot]
---

## Overview
`frontend-shared-ui` 是安卓前端应用（`android/frontend`）的核心 UI 基础库，位于 `src/shared/` 目录，通过 `index.ts` 统一导出。该模块提供跨模块复用的原子组件、组合组件、动画工具、错误边界及二次元看板娘（Mascot）系统，解耦了具体业务逻辑与展示层，确保全应用视觉风格一致（GSAP / Framer Motion 动画配置经 `useAnimationProfile` 协调）与交互模式统一（确认弹窗、Toast、标签页等）。

核心功能包括：
1. **动态数字显示**：`AnimatedNumber` 基于 GSAP `quickTo` 的高性能数值滚动与缩放弹跳。
2. **安全交互控件**：`ConfirmDialog` 防重复提交、`RefreshButton` 带状态反馈与完成抖动。
3. **看板娘系统**：`MascotFigure` 统一渲染 `/girl/mascot-*.webp`；`dailyMascot.ts` 提供每日轮换侧边立绘；各 Toast / 空态 / 错误态引用不同 variant。
4. **错误恢复**：`ErrorBoundary` Class Component 捕获渲染错误并引导重载。
5. **日志面板**：`LogPanel` 高性能日志查看器，支持多级筛选、搜索、5s 轮询、自动滚动及 GSAP 批量删除动画。
6. **基础类型与常量**：`types.ts`、`ui-types.ts`、`ui-constants.ts` 定义跨模块共享接口与导航配置。

## Key Components

### AnimatedNumber (`android/frontend/src/shared/AnimatedNumber.tsx`)
数值平滑过渡动画组件，常用于延迟、速度等实时数据展示。

*   **Props（接口 `AnimatedNumberProps`，行 6-12）**：
    *   `value: number`（行 7）- 目标数值。
    *   `unit?: string`（行 8）- 单位后缀，默认 `'ms'`（行 16）。
    *   `decimals?: number`（行 9）- 小数位数，默认 `0`（行 17）。
    *   `className?: string`（行 10）- CSS 类名，默认 `''`（行 18）。
    *   `duration?: number`（行 11）- 动画时长（毫秒），未提供则用 `profile.numberDuration`（行 23）。
*   **实现细节**：
    *   行 14-20：函数签名与默认值解构。
    *   行 21-30：hooks 调用与 ref 声明（`profile`、`animActive`、`resolvedDuration`、DOM ref、prev/isFirst/obj ref、两个 `quickTo` ref、reset timer ref）。
    *   行 32-59：`useEffect` 初始化 `gsap.quickTo` 写 `objRef.current.value`（onUpdate 写 textContent，行 38-42）与 `el` 的 scale（行 44-48）；清理函数（行 49-58）kill 底层 tween 防内存泄漏（历史缺陷见 Known Issues 1）。
    *   行 61-96：`useEffect` 处理值更新；首次渲染（行 62-69）直接设文本；值变化且动画可用时（行 71-95）触发数值与 scale（economy 档禁 scale，行 83-84），先 kill 旧 resetTimer 再注册新 `delayedCall`（行 87-91，历史缺陷见 Known Issues 3）。
    *   行 98-109：渲染 `span`，`tabular-nums` + `inline-block`，初始子文本 `{value.toFixed(decimals)}{unit}`（行 107）。

### ConfirmDialog (`android/frontend/src/shared/ConfirmDialog.tsx`)
通用确认对话框，内置防重复提交。

*   **Props（接口 `ConfirmDialogProps`，行 12-21）**：
    *   `open: boolean`（行 13）
    *   `title: string`（行 14）
    *   `message: string`（行 15）
    *   `onConfirm: () => void`（行 16）
    *   `onCancel: () => void`（行 17）
    *   `confirmLabel?: string`（行 19）- 自定义确认文案
    *   `confirmVariant?: 'default' | 'destructive'`（行 20）- 默认 `'destructive'`（行 23）
*   **Key Logic**：
    *   行 24：`useTranslation` 取 `t`。
    *   行 26：`confirming` state 防双击（历史缺陷见 Known Issues 2）。
    *   行 29-31：`useEffect` 监听 `open`，关窗复位 `confirming`。
    *   行 33-36：`Dialog` `onOpenChange` 在 confirming 时拦截取消。
    *   行 42-48：取消/确认按钮均 `disabled={confirming}`，确认按钮置 `confirming=true` 后调 `onConfirm`。

### dailyMascot (`android/frontend/src/shared/dailyMascot.ts`)
每日轮换看板娘资源与 Hook。

*   **常量**：
    *   `SIDE_MASCOT_POOL`（行 5-9）：14 个只读字符串 `'side-wave'` … `'side-think'`。
*   **函数**：
    *   `pickDailyMascot(offset = 0): string`（行 11-15）：按 UTC 天 `Math.floor(Date.now()/86_400_000)` 取模（行 13-14），offset 使同屏不同图。
    *   `useDailyMascots(offsets: number[]): string[]`（行 18-34）：state 初值 `compute()`（行 22）；`useEffect`（行 23-33）每分钟 `setInterval` 比对日期跨天更新（行 25-30），依赖 `key=offsets.join(',')`（行 20）。

### ErrorBoundary (`android/frontend/src/shared/ErrorBoundary.tsx`)
Class Component 错误边界。

*   **Props（行 6-8）**：`children: ReactNode`。
*   **State（行 10-13）**：`hasError: boolean`、`error: Error | null`。
*   **方法**：
    *   行 16-19：constructor 初始化 state。
    *   行 21-23：`static getDerivedStateFromError` 返回错误 state。
    *   行 25-27：`componentDidCatch` DEV 下 `console.error`。
    *   行 29-52：`render` 出错时显 `MascotFigure variant="alert"`（行 33）+ 错误信息 + 重载按钮（行 38-46 `window.location.reload`）。

### FluidBackground (`android/frontend/src/shared/FluidBackground.tsx`)
背景占位组件。

*   行 1-10：导出函数返回 `absolute inset-0 z-0 pointer-events-none` 的 div，`background: var(--surface-main)`。无 props。

### IspMark (`android/frontend/src/shared/IspMark.tsx`)
运营商/校徽标识。

*   `ISP_MARK_SRC`（行 5-10）：映射 `''`/`__default__`→`/isp/school.webp`，`@telecom`/`@unicom`/`@cmcc`→对应 webp。
*   `ispMarkSrc(suffix)`（行 12-15）：null/undefined 返回 undefined，否则查表回退 default。
*   `IspMark = memo(...)`（行 18-41）：`suffix` 为 `''`或`'__default__'`时白圆底承载（行 27-38），其余原色 img（行 39-41）。

### LogPanel (`android/frontend/src/shared/LogPanel.tsx`)
高级日志面板。

*   **Props（接口 `LogPanelProps`，行 27-37）**：`api`（含 `getLogs`/`clearLogs`/`getDebugMode`/`setDebugMode`/`getLogRetentionDays?`/`setLogRetentionDays?`），`addToast`。
*   **类型**：`LogLevel`（行 39）、`ParsedLogLine`（行 41-47）、`LEVEL_CONFIG`（行 51-56）、`LOG_LINE_REGEX`（行 58）、`LINE_OPTIONS`（行 76-81）、`MAX_DISPLAY_LINES=200`（行 83）。
*   **状态（行 89-105）**：`rawLogs`/`isLoading`/`isClearing`/`lineCount=200`/`filterLevel='ALL'`/`searchText`/`filterModule='ALL'`/`debugMode`/`retentionDays=7`/`showClearConfirm`，refs：`scrollRef`/`isAutoScrollRef`/`isVisibleRef`/`fetchSeqRef`/`mountedRef`/`rawLogsRef`；另 `logsKey` state（行 213）。
*   **关键逻辑**：
    *   行 62：`isAndroidBuild = VITE_PLATFORM==='android'` 控制两行布局。
    *   行 107-114：StrictMode mountedRef 保护 effect。
    *   行 118-137：`fetchLogs` 竞态保护 + rawLogsRef 比对边增 setState。
    *   行 139-141：初始 `fetchLogs(true)`。
    *   行 143-152：`IntersectionObserver` 可见性门控。
    *   行 154-162：`getDebugMode` 与 `getLogRetentionDays` 初始化 effect。
    *   行 164-177：5s 轮询 + `visibilitychange` 门控。
    *   行 179-191：toggleDebugMode。
    *   行 193-204：handleRetentionChange 乐观更新。
    *   行 206-211：自动滚动 effect（依赖 rawLogs, filterLevel, filterModule, searchText）。
    *   行 215-219：handleScroll 回调更新 isAutoScrollRef。
    *   行 221-232：parsedLines 单遍解析 memo。
    *   行 234-237：availableModules memo。
    *   行 239-263：filteredLines memo（级别/模块/搜索筛选）。
    *   行 265-270：displayedLines 截断 200 条 memo。
    *   行 272-275：handleClear 打开确认。
    *   行 277-380：handleClearConfirm GSAP 视口内 stagger 删除（含 onInterrupt 复位、setTimeout 兜底，见 Known Issues 4）。
    *   行 382-388：`levelCounts` memo。
    *   行 390-678：UI 渲染（筛选栏、滚动区、`AnimatePresence` 列表、背景看板娘 img）。

### MascotFigure (`android/frontend/src/shared/MascotFigure.tsx`)
看板娘图片统一出口。

*   `SIZES`（行 3）：`sm:'w-20'`、`md:'w-28'`、`lg:'w-40'`。
*   `MascotVariant`（行 5）：`'portrait'|'welcome'|'empty'|'celebrate'|'sponsor'|'offline'|'alert'`。
*   行 7-26：函数按 `variant` 拼 `/girl/mascot-${variant}.webp`，`loading="lazy"`。

### RefreshButton (`android/frontend/src/shared/RefreshButton.tsx`)
带反馈刷新按钮。

*   **Props（接口 `RefreshButtonProps`，行 7-11）**：`isRefreshing: boolean`、`iconClassName?`、`showCheck?` 及原生 button 属性。
*   行 13-72：`forwardRef` 组件；`prevRefreshing` ref（行 15）检测 true→false 加 `refresh-shake`（行 18-28）；`AnimatePresence` 显 Check（行 52-65）。
*   行 74-81：`getRefreshIconClass` 导出辅助。

### SegmentTabs (`android/frontend/src/shared/SegmentTabs.tsx`)
动画滑块标签页。

*   `TabItem`（行 5-11）：`key`/`label`/`icon`/`color`/`bg`。
*   `SegmentTabsProps`（行 13-17）：`tabs`/`activeKey`/`onTabChange`。
*   行 19-53：`useId` 前缀防 layoutId 冲突（行 22），`m.div` layoutId 滑块（行 41-45）。

### SponsorCard (`android/frontend/src/shared/SponsorCard.tsx`)
移动端赞助弹窗。

*   行 13-51：`Dialog` 展示微信/支付宝二维码 + `MascotFigure variant="sponsor"`（行 24）。

### ToastContainer (`android/frontend/src/shared/ToastContainer.tsx`)
全局 Toast 容器。

*   `TOAST_MASCOTS`（行 16-21）：类型→variant 映射。
*   `TOAST_STYLES`（行 23-28）：各类型背景类。
*   行 30-97：`memo` 组件；economy 档简动画（行 33-36），`celebrate` popIn（行 44-63），`AnimatePresence` 列表（行 40-94）。

### UpdateAvailableDialog (`android/frontend/src/shared/UpdateAvailableDialog.tsx`)
新版本提醒弹窗。

*   行 18-38：从 `useQualityStore` 取 `updatePromptOpen`/`latestVersion`/`releaseNotes`（行 20-23），跳转 `onGoUpdate`。

### index.ts (`android/frontend/src/shared/index.ts`)
*   行 1-8：导出 `LogPanel`/`ConfirmDialog`/`AnimatedNumber`/`RefreshButton`(+`getRefreshIconClass`)/`SegmentTabs`/`ToastContainer`/`FluidBackground`/`ErrorBoundary`。
*   行 10-12：`export * from './types'`、`'./ui-types'`、`'./ui-constants'`。

### types.ts (`android/frontend/src/shared/types.ts`)
*   `UpdateAvailableData`（行 1-5）：`hasUpdate`/`latestVersion`/`releaseNotes?`。
*   `UpdateInfo`（行 7-17）：含 `assets`/`sha256Checksum?`/`lastCheckError`/`lastCheckTime`。
*   `DownloadProgress`（行 19-24）：`downloaded`/`total`/`speed`/`percent`。
*   `MirrorSource`（行 26-29）：`name`/`url`/`description`。

### ui-types.ts (`android/frontend/src/shared/ui-types.ts`)
*   `StatusState`（行 1）、`PanelName`（行 2）、`ThemeName`（行 3）、`GlassLevel`（行 5）、`LogType`（行 6）。
*   `LogEntry`（行 8-15）：`id`/`time`/`message`/`type`/`count?`。
*   `ToastMessage`（行 17-29）：`id`/`title`/`description?`/`type`/`duration?`/`mascot?`（`'portrait' | 'celebrate' | 'offline' | 'alert' | 'update'`）/ `action?`。
*   `AdapterDisabledWarningData`（行 31-34）、`AutoExitCountdownData`（行 36-39）、`SaveConfigResult`（行 41-45）。

### ui-constants.ts (`android/frontend/src/shared/ui-constants.ts`)
*   行 1：`MAX_LOG_ENTRIES = 300`。
*   行 2：`APP_VERSION = '2.4.3'`。
*   行 3：`APP_NAME = '校园网登录助手'`。
*   行 4：`PASSWORD_MASK = '***'`。
*   行 7-15：`NAV_ITEMS` 数组（dashboard/account/selfservice/monitor/quality/speedtest/settings/log）。

## Architecture
```text
┌──────────────────────────────────────────────────────┐
│  App (Dashboard/Settings/Monitor/Log)                │
│       │        │         │          │                │
▼       ▼        ▼         ▼          ▼                │
┌──────────┐ ┌────────┐ ┌─────────┐ ┌──────────────┐  │
│RefreshBtn│ │SegTabs│ │LogPanel │ │Sponsor/Update│  │
│(RefreshB)│ │(SegTab)│ │(LogPanel)│ │(Dialogs)     │  │
└────┬─────┘ └───┬────┘ └────┬────┘ └──────┬───────┘  │
     │           │           │             │          │
     ▼           ▼           ▼             ▼          │
┌──────────────────────────────────────────────────┐ │
│ Shared Primitives: AnimatedNumber, ConfirmDialog, │ │
│ ErrorBoundary, MascotFigure, IspMark, ToastCont,  │ │
│ FluidBackground, SegmentTabs, RefreshButton       │ │
└──────────────────────────────────────────────────┘ │
     │           │           │                       │
     ▼           ▼           ▼                       │
┌──────────┐ ┌────────┐ ┌─────────┐                  │
│dailyMascot│ │useAnimP│ │useQuality│                 │
│(ts)       │ │rofile  │ │Store     │                 │
└──────────┘ └────────┘ └─────────┘                  │
└──────────────────────────────────────────────────────┘
```
1. **Presentation Layer**：`LogPanel`、`SponsorCard`、`UpdateAvailableDialog` 组合 `ConfirmDialog`/`MascotFigure`/`RefreshButton` 等原子组件。
2. **Animation Engine**：`AnimatedNumber` 用 GSAP；`ToastContainer`/`RefreshButton`/`SegmentTabs`/`LogPanel` 用 Framer Motion；均经 `useAnimationProfile`（行 21 AnimatedNumber、行 86 LogPanel、行 32 ToastContainer）协调 economy 降级。
3. **State**：`dailyMascot` 纯函数 + Hook，无外部 store；`UpdateAvailableDialog` 用 `useQualityStore`（行 20-23）。
4. **Error Handling**：`ErrorBoundary` 包裹子树，出错显 Mascot alert + reload。

## Data Flow

1. **Mascot Rotation**：
   * `useDailyMascots`（dailyMascot.ts 行 18）取 UTC 天 → `pickDailyMascot`（行 11）映射 `SIDE_MASCOT_POOL`（行 5）。
   * 每 60s 查跨天（行 25-30）更新；结果送 `MascotFigure` 渲染 webp。
2. **Log Panel Fetching**：
   * 初始化 `fetchLogs(true)`（LogPanel.tsx 行 139-141），5s 轮询（行 170-172）。
   * 可见性门控：`IntersectionObserver` (行 143-152) + `visibilitychange` (行 167-169)。
   * 去重：新结果比 `rawLogsRef.current`（行 126）避免无变化 setState。
3. **Toast Notification**：
   * 外部传 `toasts: ToastMessage[]` 给 `ToastContainer`（行 30），`AnimatePresence` 进退场（行 40-94），`onRemove(id)` 剔除。

## Connections
* [[frontend-dashboard]]：用 `RefreshButton`、`AnimatedNumber`、`MascotFigure`。
* [[frontend-settings]]：用 `SegmentTabs`、`ConfirmDialog`、`SponsorCard`。
* [[frontend-monitor]]：用 `LogPanel`、`ErrorBoundary`。
* [[hooks-use-animation-profile]]：被 `AnimatedNumber`(行 21)、`ToastContainer`(行 32)、`LogPanel`(行 86) 引用。
* [[hooks-use-quality-store]]：被 `UpdateAvailableDialog`(行 20) 引用。
* [[components-ui-dialog]]：被 `ConfirmDialog`、`SponsorCard`、`UpdateAvailableDialog` 封装。
* [[components-ui-button]]：被 `ConfirmDialog`(行 2)、`LogPanel`(行 3)、`ToastContainer`(行 6) 等引用。

## Known Issues

1. **GSAP Cleanup in AnimatedNumber**
   * File: `AnimatedNumber.tsx`, 行 49-58（cleanup 中 `gsap.killTweensOf`）。
   * Issue: `quickTo` 底层 tween 卸载后仍存活写已卸载 DOM（历史缺陷，行 51-52 注释）。
   * Mitigation: cleanup 显式 kill（行 54-55），onUpdate 有 `if(ref.current)` 保护（行 39）。

2. **ConfirmDialog Double Submit**
   * File: `ConfirmDialog.tsx`, 行 26、行 44（按钮 `disabled={confirming}`）。
   * Issue: 快速双击执行两次破坏操作（行 25 注释）。
   * Mitigation: `confirming` state 禁用。

3. **Scale Bounce Glitch in AnimatedNumber**
   * File: `AnimatedNumber.tsx`, 行 87-91（先 kill 旧 resetTimer 再注册）。
   * Issue: 旧 `delayedCall` 堆积导致 scale 跳变（行 85-86 注释）。
   * Mitigation: 注册前 `resetTimerRef.current.kill()`（行 87）。

4. **Log Clear Animation Freeze in Android WebView**
   * File: `LogPanel.tsx`, 行 329-361（GSAP context + failsafe setTimeout）。
   * Issue: 后台 rAF 停摆 tween 冻结，`isClearing` 卡死（行 352-355 注释）。
   * Mitigation: `setTimeout` 兜底 `finishClear`+`ctx.revert()`（行 356-361）。

5. **Layout ID Collision in SegmentTabs**
   * File: `SegmentTabs.tsx`, 行 22（`useId` 前缀）。
   * Issue: 多实例共享 `layoutId` 错享动画（行 20-21 注释）。
   * Mitigation: `layoutId={`${layoutIdPrefix}-activeTab`}`（行 42）。
