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
`android-frontend-shared-ui` 模块是 Android 前端应用的核心 UI 资产库。它封装了跨页面复用的原子组件、布局容器、数据展示逻辑以及全局常量与类型定义。该模块深度集成了 `gsap` 和 `framer-motion` 动画库，针对 Android WebView 环境（如 rAF 挂起、内存泄漏）进行了特定的性能优化和缺陷修复。同时，模块引入了二次元吉祥物（Mascot）系统以增强用户交互体验，并提供了完整的日志面板、错误边界和更新提示机制。

## Key Components

### 1. AnimatedNumber (数字动画)
**文件**: `android/frontend/src/shared/AnimatedNumber.tsx`
用于展示带有平滑过渡效果的数值，常用于延迟、速度等动态指标。

*   **Props 接口**:
    *   `value`: number (当前数值)
    *   `unit`: string (单位后缀，默认 'ms')
    *   `decimals`: number (小数位数，默认 0)
    *   `className`: string (CSS 类名)
    *   `duration`: number (动画时长，可选，默认取自 profile)
*   **核心逻辑**:
    *   使用 `gsap.quickTo` 对内部对象 `objRef.current.value` 进行插值动画 (35|)。
    *   同步对 DOM 元素执行 `scale` 缩放动画以产生视觉反馈 (44|)。
    *   通过 `useAnimationProfile` 和 `useAnimationActive` 控制动画档位和页面空闲状态下的降级策略 (21-22|)。
    *   **清理逻辑**: 在 useEffect 返回的 cleanup 函数中，显式调用 `gsap.killTweensOf` 销毁底层 Tween 实例，防止组件卸载后继续占用 CPU 或操作已卸载 DOM (54-55|)。
    *   **状态重置**: 使用 `gsap.delayedCall` 在动画结束后将 scale 恢复为 1，并在触发新变化前 kill 旧 timer 以避免跳变 (87-91|)。

### 2. ConfirmDialog (确认框)
**文件**: `android/frontend/src/shared/ConfirmDialog.tsx`
通用的破坏性或引导性操作确认弹窗。

*   **Props 接口**:
    *   `open`: boolean (是否显示)
    *   `title`: string (标题)
    *   `message`: string (内容)
    *   `onConfirm`: () => void (确认回调)
    *   `onCancel`: () => void (取消回调)
    *   `confirmLabel`: string (自定义确认按钮文案，可选)
    *   `confirmVariant`: 'default' | 'destructive' (按钮样式，默认 destructive)
*   **关键特性**:
    *   **防重复提交**: 引入 `confirming` 状态，在 `onConfirm` 执行期间禁用按钮 (26|, 44-47|)。
    *   **状态复位**: 监听 `open` prop，当弹窗关闭时强制将 `confirming` 置为 false，防止因网络延迟导致按钮永久锁定 (29-31|)。

### 3. dailyMascot (每日吉祥物)
**文件**: `android/frontend/src/shared/dailyMascot.ts`
根据 UTC 日期轮换展示不同的二次元娘立绘，用于装饰目的。

*   **常量**:
    *   `SIDE_MASCOT_POOL`: const string[] (14 张竖版立绘名称列表，5-9|)
*   **函数**:
    *   `pickDailyMascot(offset?: number): string`: 根据当前天数和偏移量从池中选取一张图片名 (11-15|)。
    *   `useDailyMascots(offsets: number[]): string[]`: React Hook，返回一组每日吉祥物。每分钟检查一次日期切换，支持多个独立的 offset (18-35|)。

### 4. ErrorBoundary (错误边界)
**文件**: `android/frontend/src/shared/ErrorBoundary.tsx`
基于 Class Component 实现的渲染错误捕获器。

*   **Class Methods**:
    *   `constructor(props)`: 初始化 state (16-19|)。
    *   `static getDerivedStateFromError(error)`: 捕获渲染错误并更新 state 为 hasError=true (21-23|)。
    *   `componentDidCatch(error, errorInfo)`: 在 DEV 模式下打印错误日志 (25-27|)。
    *   `render()`: 若发生错误，展示 MascotFigure(alert) 和重载按钮；否则渲染 children (29-52|)。

### 5. FluidBackground (流体背景)
**文件**: `android/frontend/src/shared/FluidBackground.tsx`
简单的占位背景组件，使用 CSS 变量 `--surface-main`。

*   **Component**:
    *   `FluidBackground()`: 绝对定位覆盖全屏，z-index 0，pointer-events none (1-10|)。

### 6. IspMark (ISP 标记)
**文件**: `android/frontend/src/shared/IspMark.tsx`
显示运营商或学校标识的小图标。

*   **函数**:
    *   `ispMarkSrc(suffix)`: 映射 ISP 后缀到 WebP 图片路径 (12-15|)。
*   **Component**:
    *   `IspMark`: memo 包裹的组件。
        *   若 suffix 为空或 `__default__` (无锡学院)，渲染带白色圆底和黑色环的 `<span>` 包裹图片 (27-38|)。
        *   其他运营商直接渲染 `<img>` (39-42|)。

### 7. LogPanel (日志面板)
**文件**: `android/frontend/src/shared/LogPanel.tsx`
功能最复杂的组件，包含日志获取、过滤、搜索、清空动画及调试模式控制。

*   **Props 接口**:
    *   `api`: 包含 `getLogs`, `clearLogs`, `getDebugMode`, `setDebugMode`, `getLogRetentionDays`, `setLogRetentionDays` 的方法对象 (28-35|)。
    *   `addToast`: 消息通知回调 (36|)。
*   **内部状态**:
    *   `rawLogs`, `isLoading`, `isClearing`, `lineCount`, `filterLevel`, `searchText`, `filterModule`, `debugMode`, `retentionDays`, `showClearConfirm` (89-98|)。
    *   `scrollRef`, `isAutoScrollRef`, `isVisibleRef`, `fetchSeqRef`, `mountedRef`, `rawLogsRef` (99-105|)。
*   **核心方法**:
    *   `fetchLogs(manual?)`: 异步获取日志。处理 StrictMode 双重挂载 (107-114|)。使用 `fetchSeqRef` 竞态保护 (119-137|)。若内容未变则跳过 setState 优化 (126-129|)。
    *   `toggleDebugMode()`: 切换调试模式，乐观更新 UI，失败回滚 (179-191|)。
    *   `handleRetentionChange(days)`: 修改日志保留天数，支持乐观更新与回滚 (193-204|)。
    *   `handleClearConfirm()`: 执行清空操作。
        *   筛选可视区域内的 DOM 元素 (287-294|)。
        *   不可见元素直接隐藏 (298-300|)。
        *   可见元素使用 GSAP stagger 动画逐条删除 (330-350|)。
        *   **缺陷修复**: 增加 `onInterrupt` 处理动画中断导致的 `isClearing` 卡死 (343-349|)。
        *   **缺陷修复**: 增加 `setTimeout` failsafe 兜底，防止 Android WebView 后台 rAF 停摆导致动画永不完成 (356-361|)。
*   **Memoized Selectors**:
    *   `parsedLines`: 正则解析日志行 (221-232|)。
    *   `availableModules`: 提取所有模块名 (234-237|)。
    *   `filteredLines`: 应用级别、模块、搜索过滤 (239-263|)。
    *   `displayedLines`: 限制最大显示行数 MAX_DISPLAY_LINES (20|, 265-270|)。
    *   `levelCounts`: 统计各级别日志数量 (382-388|)。

### 8. MascotFigure (吉祥物图)
**文件**: `android/frontend/src/shared/MascotFigure.tsx`
统一出口的图片组件，加载 `public/girl/mascot-*.webp`。

*   **Types**:
    *   `MascotVariant`: 'portrait' | 'welcome' | 'empty' | 'celebrate' | 'sponsor' | 'offline' | 'alert' (5|)。
*   **Component**:
    *   `MascotFigure`: 根据 variant 和 size 渲染 img，lazy loading，draggable false (7-26|)。

### 9. RefreshButton (刷新按钮)
**文件**: `android/frontend/src/shared/RefreshButton.tsx`
带旋转动画和完成反馈的刷新按钮。

*   **Props 接口**:
    *   `isRefreshing`: boolean
    *   `iconClassName`: string
    *   `showCheck`: boolean (完成后显示对勾)
*   **Component**:
    *   `RefreshButton`: forwardRef 组件。
        *   监听 `isRefreshing` 变化，结束时添加 `refresh-shake` 类并清除 (18-28|)。
        *   使用 `AnimatePresence` 和 `m.span` 展示 Check 动画 (52-65|)。
*   **Helper**:
    *   `getRefreshIconClass`: 纯函数，生成图标 CSS 类 (74-81|)。

### 10. SegmentTabs (分段标签)
**文件**: `android/frontend/src/shared/SegmentTabs.tsx`
带动画滑块的分段选择器。

*   **Props 接口**:
    *   `tabs`: TabItem[] (key, label, icon, color, bg)
    *   `activeKey`: string
    *   `onTabChange`: (key: string) => void
*   **Component**:
    *   `SegmentTabs`: 使用 `useId` 生成唯一 layoutId 前缀，防止 framer-motion 跨实例动画冲突 (22-23|)。活跃 Tab 使用 `m.div` layoutId 动画跟随 (41-46|)。

### 11. SponsorCard (赞助卡)
**文件**: `android/frontend/src/shared/SponsorCard.tsx`
移动端适配的赞助弹窗，使用 Radix Dialog。

*   **Props 接口**:
    *   `open`: boolean
    *   `onClose`: () => void
*   **Component**:
    *   `SponsorCard`: 展示赞助商二维码（微信/支付宝），居中显示 (13-52|)。

### 12. ToastContainer (Toast 容器)
**文件**: `android/frontend/src/shared/ToastContainer.tsx`
管理全局 Toast 消息的容器。

*   **Props 接口**:
    *   `toasts`: ToastMessage[]
    *   `onRemove`: (id: string) => void
*   **常量**:
    *   `TOAST_MASCOTS`: 映射 Toast 类型到吉祥物变体 (16-21|)。
    *   `TOAST_STYLES`: 映射 Toast 类型到背景色样式 (23-28|)。
*   **Component**:
    *   `ToastContainer`: 使用 `AnimatePresence mode="popLayout"` 管理入场/出场动画 (40-95|)。
    *   根据 `profile.tier` (economy) 调整动画复杂度 (34-36|)。
    *   Celebrate 类型的 mascot 有额外的 spring 弹入效果 (44|, 62-63|)。

### 13. UpdateAvailableDialog (更新提示对话框)
**文件**: `android/frontend/src/shared/UpdateAvailableDialog.tsx`
新版本发现时的提示弹窗。

*   **Props 接口**:
    *   `onGoUpdate`: () => void
*   **逻辑**:
    *   从 `useQualityStore` 读取 `updatePromptOpen`, `latestVersion`, `releaseNotes` (20-23|)。
    *   提供“稍后”和“立即前往”按钮 (33-35|)。

### 14. Types & Constants (共享类型/常量)
**文件**: `android/frontend/src/shared/types.ts`
*   `UpdateAvailableData`: hasUpdate, latestVersion, releaseNotes (1-5|)。
*   `UpdateInfo`: 包含 assets, sha256Checksum, lastCheckError, lastCheckTime (7-17|)。
*   `DownloadProgress`: downloaded, total, speed, percent (19-24|)。
*   `MirrorSource`: name, url, description (26-30|)。

**文件**: `android/frontend/src/shared/ui-types.ts`
*   `StatusState`: 'loading' | 'online' | 'offline' | 'error' (1|)。
*   `PanelName`: 各面板名称枚举 (2|)。
*   `ThemeName`: 主题名称枚举 (3|)。
*   `GlassLevel`: 'off' | 'standard' | 'ultra' (5|)。
*   `LogType`: 'info' | 'success' | 'error' | 'warning' (6|)。
*   `LogEntry`: id, time, message, type, count (8-15|)。
*   `ToastMessage`: id, title, description, type, duration, mascot, action (17-29|)。
*   `AdapterDisabledWarningData`: name, message (31-34|)。
*   `AutoExitCountdownData`: delay, shortcut (36-39|)。
*   `SaveConfigResult`: success, message, data (41-45|)。

**文件**: `android/frontend/src/shared/ui-constants.ts`
*   `MAX_LOG_ENTRIES`: 300 (1|)。
*   `APP_VERSION`: '2.4.1' (2|)。
*   `APP_NAME`: '校园网登录助手' (3|)。
*   `PASSWORD_MASK`: '***' (4|)。
*   `NAV_ITEMS`: 导航项数组，包含 id, labelKey, icon, shortcut (7-16|)。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                   android-frontend-shared-ui                │
│                                                             │
│  ┌──────────────┐   ┌──────────────────┐   ┌─────────────┐ │
│  │   Constants  │   │     Types        │   │  Assets     │ │
│  │ ui-constants │   │  ui-types.ts     │   │ girl/*.webp │ │
│  │              │   │  types.ts        │   │ isp/*.webp  │ │
│  └──────┬───────┘   └────────┬─────────┘   └─────────────┘ │
│         │                    │                              │
│  ┌──────▼────────────────────▼──────────────────────────┐  │
│  │                  Shared Components                   │  │
│  │  ┌─────────────┐ ┌──────────────┐ ┌───────────────┐ │  │
│  │  │  Layout/UI  │ │  Feedback    │ │  Data Display │ │  │
│  │  │             │ │             │ │               │ │  │
│  │  │ ErrorBound. │ │ ConfirmDlog │ │ AnimatedNum.  │ │  │
│  │  │ FluidBackgr │ │ SponsorCard │ │ LogPanel      │ │  │
│  │  │ SegmentTabs │ │ ToastCont.  │ │ IspMark       │ │  │
│  │  │ RefreshBtn  │ │ UpdateAvail │ │ MascotFigure  │ │  │
│  │  └─────────────┘ └──────────────┘ └───────────────┘ │  │
│  │                                                     │  │
│  │  ┌─────────────────────────────────────────────┐    │  │
│  │  │           Animation Engines                 │    │  │
│  │  │  GSAP (quickTo, killTweensOf, delayedCall)  │    │  │
│  │  │  Framer Motion (AnimatePresence, m.div)     │    │  │
│  │  └─────────────────────────────────────────────┘    │  │
│  └─────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **日志流 (LogPanel)**:
    *   `api.getLogs()` -> `rawLogs` state -> `parsedLines` (memo) -> `filteredLines` (memo) -> `displayedLines` (memo).
    *   轮询定时器每 5s 调用 `fetchLogs`，受 `isVisibleRef` (IntersectionObserver) 和 `document.hidden` 门控 (170-172|)。
    *   用户操作 (过滤/搜索) 触发 `filteredLines` 重算，进而触发 `displayedLines` 重算和自动滚动 (211|)。

2.  **更新提示流 (UpdateAvailableDialog)**:
    *   后端/Store 设置 `updatePromptOpen = true` -> `UpdateAvailableDialog` 弹出。
    *   用户点击 "Go" -> `onGoUpdate()` 跳转 -> Store 设置 `open = false`。

3.  **每日吉祥物流 (dailyMascot)**:
    *   `useDailyMascots` Hook 启动 `setInterval` (60s)。
    *   计算当前 UTC 天数 -> `pickDailyMascot` -> 更新 `mascots` state。
    *   子组件通过 `MascotFigure` 渲染对应图片。

## Connections

*   [[android-frontend-hooks]]: 依赖 `useAnimationProfile`, `usePageIdle`, `useQualityStore`。
*   [[android-frontend-components-ui]]: 依赖基础 UI 组件 `Button`, `Dialog`, `Card`, `Badge`, `Select`。
*   [[android-frontend-lib-utils]]: 依赖 `cn`, `extractErrorMessage`。
*   [[android-frontend-assets]]: 引用 `public/girl/mascot-*.webp` 和 `public/isp/*.webp`。

## Known Issues

1.  **GSAP Tween 内存泄漏 (AnimatedNumber)**:
    *   **位置**: `android/frontend/src/shared/AnimatedNumber.tsx` 第 51-53 行注释。
    *   **描述**: 历史缺陷，`quickTo` 底层 tween 在组件卸载后仍存活，持续占用 CPU 并尝试写入 null ref。
    *   **修复**: 已在 cleanup 函数中调用 `gsap.killTweensOf` (54-55|)。

2.  **ConfirmDialog 重复提交**:
    *   **位置**: `android/frontend/src/shared/ConfirmDialog.tsx` 第 25 行注释。
    *   **描述**: 快速双击可能导致两次破坏性操作。
    *   **修复**: 引入 `confirming` 状态并禁用按钮 (26|, 44-47|)。

3.  **ConfirmDialog 按钮锁定**:
    *   **位置**: `android/frontend/src/shared/ConfirmDialog.tsx` 第 27-28 行注释。
    *   **描述**: `onConfirm` 为 async 时，固定定时器可能在慢网络下提前解禁按钮。
    *   **修复**: 改为由 `open` prop 驱动复位 `confirming` (29-31|)。

4.  **Scale 动画跳变 (AnimatedNumber)**:
    *   **位置**: `android/frontend/src/shared/AnimatedNumber.tsx` 第 85-86 行注释。
    *   **描述**: `delayedCall` 覆盖前不 kill 旧 call，导致快速连续变化时回调堆积，中途弹回 1 造成跳变。
    *   **修复**: 在注册新 call 前 kill 旧 timer (87|)。

5.  **LogPanel 清空动画卡死**:
    *   **位置**: `android/frontend/src/shared/LogPanel.tsx` 第 339-342 行注释。
    *   **描述**: Tween 被中断时 `onComplete` 不执行，`isClearing` 残留。
    *   **修复**: 增加 `onInterrupt` 回调复位状态 (343-349|)。

6.  **Android WebView rAF 停摆**:
    *   **位置**: `android/frontend/src/shared/LogPanel.tsx` 第 352-355 行注释。
    *   **描述**: 后台/遮挡时 rAF 停止，tween 冻结，`onComplete/onInterrupt` 均不触发，且 framer-motion 重渲染可能拉回隐藏条目。
    *   **修复**: 增加 `setTimeout` failsafe 兜底，确保最终结算 (356-361|)。

7.  **SegmentTabs 布局动画冲突**:
    *   **位置**: `android/frontend/src/shared/SegmentTabs.tsx` 第 20-21 行注释。
    *   **描述**: 多处并存时共享相同 layoutId 会导致 framer-motion 跨实例错误共享动画。
    *   **修复**: 使用 `useId` 生成实例前缀 (22|)。

8.  **LogPanel 渲染性能**:
    *   **位置**: `android/frontend/src/shared/LogPanel.tsx` 第 104 行注释 (FE-B-10)。
    *   **描述**: 5s 轮询若每次都触发 setState，会导致 parsedLines/filteredLines/displayedLines/levelCounts 四层 useMemo 串行全量重算。
    *   **修复**: 使用 `rawLogsRef` 比较内容，未变化时跳过 setState (126-129|)。