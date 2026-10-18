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
`frontend-shared-components` 模块是 Android 前端应用的核心 UI 资产库，负责跨模块共享通用组件、类型定义及常量。该模块集成了复杂的动画逻辑（GSAP 与 Framer Motion）、状态管理辅助以及特定的业务 UI 元素（如二次元看板娘、运营商标识）。它不依赖具体的业务页面逻辑，而是提供原子化的 UI 构建块，供 Dashboard、Settings、Monitor 等上层模块组合使用。

## Key Components

### AnimatedNumber (`android/frontend/src/shared/AnimatedNumber.tsx`)
数字滚动动画组件，用于平滑展示数值变化。
- **Props**:
  - `value`: `number` (行 13) - 目标数值。
  - `unit`: `string` (行 14) - 单位后缀，默认 `'ms'`。
  - `decimals`: `number` (行 15) - 小数位数，默认 `0`。
  - `className`: `string` (行 16) - CSS 类名。
  - `duration`: `number` (行 17) - 动画时长，若未提供则使用 `useAnimationProfile` 中的配置。
- **实现细节**:
  - 使用 `gsap.quickTo` 驱动 DOM 文本更新 (行 38)。
  - 包含缩放弹跳效果 (`scaleQuickToRef`, 行 43)，但在 `economy` 性能档位下禁用。
  - 使用 `ref` 和 `prevRef` 避免不必要的重渲染 (行 29-30)。

### ConfirmDialog (`android/frontend/src/shared/ConfirmDialog.tsx`)
通用确认对话框，防止破坏性操作被重复触发。
- **Props**:
  - `open`: `boolean` (行 18) - 控制弹窗显隐。
  - `title`: `string` (行 19) - 标题。
  - `message`: `string` (行 20) - 描述信息。
  - `onConfirm`: `() => void` (行 21) - 确认回调。
  - `onCancel`: `() => void` (行 22) - 取消回调。
  - `confirmLabel`: `string` (行 25) - 自定义确认按钮文案。
  - `confirmVariant`: `'default' | 'destructive'` (行 26) - 按钮样式变体，默认为破坏性红色。
- **关键逻辑**:
  - `confirming` 状态 (行 29) 用于在异步操作中禁用按钮，防止双击重复提交。
  - `useEffect` (行 34-36) 监听 `open` 变化以重置 `confirming` 状态，解决慢网络下的状态残留问题。

### MascotFigure (`android/frontend/src/shared/MascotFigure.tsx`)
统一的二次元看板娘图像组件。
- **Props**:
  - `variant`: `MascotVariant` (行 15) - 变体类型，包括 `'portrait'`, `'welcome'`, `'empty'`, `'celebrate'`, `'sponsor'`, `'offline'`, `'alert'`。
  - `size`: `keyof typeof SIZES` (行 16) - 尺寸：`sm` (w-20), `md` (w-28), `lg` (w-40)。
  - `className`: `string` (行 17) - 额外样式。
- **数据源**:
  - 图片路径固定为 `/girl/mascot-${variant}.webp` (行 21)。

### RefreshButton (`android/frontend/src/shared/RefreshButton.tsx`)
带刷新动画和成功反馈的按钮。
- **Props**:
  - `isRefreshing`: `boolean` - 是否处于刷新中。
  - `showCheck`: `boolean` - 刷新完成后是否显示绿色对勾。
  - `iconClassName`: `string` - 图标样式覆盖。
- **动画逻辑**:
  - 使用 Framer Motion `AnimatePresence` 显示成功对勾 (行 56-67)。
  - 刷新结束时触发 CSS 抖动类 `refresh-shake` (行 43)。
- **导出函数**:
  - `getRefreshIconClass(isRefreshing, iconClassName)`: (行 75-82) 返回图标动态类名，支持组内悬停旋转。

### LogPanel (`android/frontend/src/shared/LogPanel.tsx`)
系统日志查看面板，功能最复杂的组件。
- **Props**:
  - `api`: `object` (行 56) - 包含 `getLogs`, `clearLogs`, `getDebugMode`, `setDebugMode`, `getLogRetentionDays`, `setLogRetentionDays`。
  - `addToast`: `(message, type, description?) => void` (行 57) - 通知触发器。
- **内部状态**:
  - `rawLogs`, `parsedLines`, `filteredLines`, `displayedLines`: 日志数据处理管道 (行 200-240)。
  - `filterLevel`, `filterModule`, `searchText`: 筛选条件。
  - `debugMode`, `retentionDays`: 调试与保留策略设置。
- **关键特性**:
  - **防抖与轮询**: 5秒自动轮询，结合 `IntersectionObserver` 和 `visibilitychange` 优化后台性能 (行 140-156)。
  - **清空动画**: 使用 GSAP 逐条删除可见日志，包含 `onInterrupt` 处理中断情况 (行 250-320)。
  - **Android 适配**: 针对 MIUI 大字体优化布局，消息独占一行 (行 95-97)。
  - **性能优化**: 使用 `useMemo` 缓存解析结果，避免全表重算 (行 200-215)。

### ToastContainer (`android/frontend/src/shared/ToastContainer.tsx`)
全局通知容器。
- **Props**:
  - `toasts`: `ToastMessage[]` - 通知列表。
  - `onRemove`: `(id: string) => void` - 移除通知。
- **样式映射**:
  - `TOAST_STYLES`: 根据类型 (`info`, `success`, `error`, `warning`) 分配背景色 (行 22-26)。
  - `TOAST_MASCOTS`: 默认关联看板娘变体 (行 16-20)。
- **动画**:
  - 经济模式下使用简单过渡，非经济模式使用 Spring 物理动画 (行 33-35)。
  - `celebrate` 类型的看板娘有特殊的入场缩放动画 (行 46)。

### SegmentTabs (`android/frontend/src/shared/SegmentTabs.tsx`)
带滑动指示器的分段标签页。
- **Props**:
  - `tabs`: `TabItem[]` - 标签配置，包含 `key`, `label`, `icon`, `color`, `bg`。
  - `activeKey`: `string` - 当前激活项。
  - `onTabChange`: `(key: string) => void` - 切换回调。
- **技术点**:
  - 使用 `useId` 生成唯一的 `layoutIdPrefix` (行 18)，防止多实例间 Framer Motion 布局动画冲突。
  - 使用 `m.div` 配合 `layoutId` 实现平滑的背景滑块移动 (行 33-36)。

### SponsorCard (`android/frontend/src/shared/SponsorCard.tsx`)
移动端赞助弹窗。
- **Props**:
  - `open`: `boolean`
  - `onClose`: `() => void`
- **内容**:
  - 显示微信和支付宝二维码图片 (行 42-50)。
  - 使用 Radix Dialog 实现居中小窗 (行 30)。

### UpdateAvailableDialog (`android/frontend/src/shared/UpdateAvailableDialog.tsx`)
新版本发现提醒弹窗。
- **Props**:
  - `onGoUpdate`: `() => void` - 跳转更新页面的回调。
- **数据来源**:
  - 从 `useQualityStore` 获取 `updatePromptOpen`, `latestVersion`, `releaseNotes` (行 18-21)。

### IspMark (`android/frontend/src/shared/IspMark.tsx`)
运营商标识徽标。
- **Props**:
  - `suffix`: `string | null | undefined` - 运营商后缀，如 `'@telecom'`, `'@unicom'`, `'@cmcc'`, `''` (校徽)。
  - `className`: `string`
- **逻辑**:
  - `ispMarkSrc(suffix)`: (行 14-18) 映射后缀到 WebP 图片路径。
  - 校徽 (`''` 或 `'__default__'`) 带有白色圆形背景和边框 (行 28-35)，其他运营商直接显示原图 (行 37-40)。

### ErrorBoundary (`android/frontend/src/shared/ErrorBoundary.tsx`)
React 错误边界类组件。
- **State**:
  - `hasError`: `boolean`
  - `error`: `Error | null`
- **方法**:
  - `static getDerivedStateFromError(error)`: (行 25) 捕获错误并更新状态。
  - `componentDidCatch(error, errorInfo)`: (行 29) 开发环境下打印错误日志。
- **渲染**:
  - 出错时显示 `MascotFigure` 变体 `'alert'` 和重载按钮 (行 33-48)。

### dailyMascot (`android/frontend/src/shared/dailyMascot.ts`)
每日轮换看板娘逻辑。
- **导出**:
  - `SIDE_MASCOT_POOL`: `string[]` (行 6) - 14 张立绘名称数组。
  - `pickDailyMascot(offset)`: (行 12) 根据 UTC 天数和偏移量选择今日娘。
  - `useDailyMascots(offsets)`: (行 20) Hook，返回多个偏移量对应的每日娘数组，每分钟检查日期变更 (行 28-32)。

### FluidBackground (`android/frontend/src/shared/FluidBackground.tsx`)
流体背景占位符。
- **渲染**:
  - 绝对定位层，使用 `var(--surface-main)` 作为背景 (行 4-8)。

### Types & Constants
- **`types.ts`**:
  - `UpdateAvailableData`, `UpdateInfo`, `DownloadProgress`, `MirrorSource` (行 1-26)。
- **`ui-types.ts`**:
  - `StatusState`, `PanelName`, `ThemeName`, `LogType` (行 1-4)。
  - `LogEntry`: 包含 `id`, `time`, `message`, `type`, `count` (行 6-13)。
  - `ToastMessage`: 包含 `id`, `title`, `description`, `type`, `duration`, `mascot`, `action` (行 15-27)。
  - `AdapterDisabledWarningData`, `AutoExitCountdownData`, `SaveConfigResult` (行 29-38)。
- **`ui-constants.ts`**:
  - `MAX_LOG_ENTRIES`: 300 (行 1)。
  - `APP_VERSION`: '2.4.1' (行 2)。
  - `NAV_ITEMS`: 导航项配置数组 (行 6-14)。

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
│       │        │                                     │
│       │        └─► Uses [ConfirmDialog],             │
│       │           [MascotFigure], [AnimatedCard]     │
│       ├─► [ToastContainer.tsx] (Framer Motion)       │
│       │        │                                     │
│       │        └─► Uses [MascotFigure],              │
│       │           [ToastMessage] from types          │
│       ├─► [RefreshButton.tsx] (Framer Motion)        │
│       ├─► [SegmentTabs.tsx] (Layout Animation)       │
│       ├─► [MascotFigure.tsx] (Static Asset)          │
│       ├─► [dailyMascot.ts] (Date-based Logic)        │
│       ├─► [IspMark.tsx] (Memoized Image)             │
│       ├─► [ErrorBoundary.tsx] (Class Component)      │
│       ├─► [SponsorCard.tsx] (Dialog Wrapper)         │
│       ├─► [UpdateAvailableDialog.tsx] (Store Hook)   │
│       │        │                                     │
│       │        └─► Uses [useQualityStore]            │
│       ├─► [types.ts] / [ui-types.ts]                 │
│       └─► [ui-constants.ts]                          │
└──────────────────────────────────────────────────────┘
```

## Data Flow
1. **日志流 (LogPanel)**:
   - `api.getLogs()` 获取原始字符串 -> `parseLogLine()` 解析为 `ParsedLogLine[]` -> `filteredLines` 应用筛选 -> `displayedLines` 截取最新 200 条 -> 渲染为 `m.div` 或 `div`。
   - 用户点击清除 -> `handleClearConfirm` -> GSAP 动画隐藏 DOM -> `api.clearLogs()` -> 重置状态。
2. **通知流 (ToastContainer)**:
   - 外部通过 `addToast` 注入 `ToastMessage` 对象 -> 存入全局状态 (未在源码中显示存储逻辑，但接口暗示) -> `ToastContainer` 遍历渲染 -> 每个 Toast 根据 `type` 匹配 `TOAST_MASCOTS` 显示对应娘头像。
3. **每日娘流 (dailyMascot)**:
   - `useDailyMascots` 计算基于 UTC 天数的索引 -> 返回数组 -> 子组件 (如 `LogPanel` 底部) 使用静态 `<img>` 展示，每 60 秒重新计算。

## Connections
- [[quality]]: `UpdateAvailableDialog` 直接从 `useQualityStore` 读取更新状态。
- [[animation-profile]]: `AnimatedNumber`, `LogPanel`, `ToastContainer` 均使用 `useAnimationProfile` hook 获取性能档位和缓动曲线。
- [[ui-dialog]]: `ConfirmDialog`, `SponsorCard`, `UpdateAvailableDialog` 均依赖底层的 Radix UI Dialog 组件。
- [[i18n]]: 所有涉及文本的组件均使用 `useTranslation` hook。

## Known Issues
1. **GSAP Tween 泄漏风险 (AnimatedNumber.tsx)**:
   - 位置: `AnimatedNumber.tsx` 行 48-54。
   - 描述: `quickTo` 创建的底层 tween 在组件卸载后可能仍存活。虽然 `onUpdate` 中有 `ref.current` 保护，但仍会空转 CPU。Cleanup 函数中已添加 `gsap.killTweensOf`，但需确保在 React StrictMode 下正确执行。
2. **快速连续变化的 Scale 跳变 (AnimatedNumber.tsx)**:
   - 位置: `AnimatedNumber.tsx` 行 68-75。
   - 描述: 历史缺陷指出 `delayedCall` 覆盖前不 kill 旧 call 会导致回调堆积。代码已修复为先 `resetTimerRef.current.kill()` 再注册新 call。
3. **GSAP rAF 冻结导致 Clear 卡死 (LogPanel.tsx)**:
   - 位置: `LogPanel.tsx` 行 305-315。
   - 描述: 安卓 WebView 后台时 rAF 停摆，导致 GSAP 动画 onComplete 不触发，`isClearing` 状态卡死。代码引入了 `failsafe` setTimeout 兜底，并在 `onInterrupt` 中复位状态。
4. **SegmentTabs 布局动画冲突**:
   - 位置: `SegmentTabs.tsx` 行 18。
   - 描述: 如果多个 `SegmentTabs` 实例共存且不使用唯一 ID，Framer Motion 的 `layoutId` 会跨实例混淆。代码使用 `useId` 生成前缀解决此问题。
5. **ConfirmDialog 按钮状态复位**:
   - 位置: `ConfirmDialog.tsx` 行 34-36。
   - 描述: 历史缺陷是 `onConfirm` 异步完成前按钮不禁用，或慢网络下定时器提前解禁。现改为由 `open` 状态驱动复位，确保一致性。