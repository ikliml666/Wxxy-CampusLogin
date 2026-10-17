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
`frontend-shared-ui` 是前端应用的核心 UI 基础库，提供跨模块复用的原子组件、动画工具、状态边界处理及二次元看板娘（Mascot）系统。该模块解耦了具体业务逻辑与展示层，确保了全应用视觉风格的一致性（如 GSAP/Framer Motion 动画配置）和交互模式的统一（如确认弹窗、Toast 通知）。

核心功能包括：
1. **动态数字显示**：基于 GSAP 的高性能数值滚动。
2. **安全交互控件**：防重复提交的确认对话框、带状态反馈的刷新按钮。
3. **看板娘系统**：每日轮换的二次元角色展示，支持多种变体（欢迎、错误、赞助等）。
4. **错误恢复机制**：React Class Component 实现的 Error Boundary，配合 Mascot 引导用户重载。
5. **日志面板**：高性能日志查看器，支持多级筛选、搜索、自动滚动及批量删除动画。

## Key Components

### AnimatedNumber (`android/frontend/src/shared/AnimatedNumber.tsx`)
实现数值的平滑过渡动画，常用于延迟、速度等实时数据的展示。

*   **Props**:
    *   `value`: `number` (Line 9) - 目标数值。
    *   `unit`: `string` (Line 10) - 单位后缀，默认 `'ms'`。
    *   `decimals`: `number` (Line 11) - 小数位数，默认 `0`。
    *   `className`: `string` (Line 12) - CSS 类名。
    *   `duration`: `number` (Line 13) - 动画时长（毫秒），若未提供则使用 `useAnimationProfile().numberDuration`。

*   **Implementation Details**:
    *   Line 28-48: `useEffect` 初始化 GSAP `quickTo` 实例。监听 `decimals`, `unit`, `resolvedDuration` 变化重建 tween。清理函数中调用 `gsap.killTweensOf` 防止内存泄漏。
    *   Line 50-86: `useEffect` 处理值更新。若为首次渲染直接设置文本。若值变化且动画启用，触发 `valueQuickToRef` 和 `scaleQuickToRef`。针对 `economy` 档位禁用 scale 弹跳以保证一致性。
    *   Line 73-79: 历史缺陷修复：在注册新的 `delayedCall` 复位 scale 之前，先 kill 旧的 timer，防止快速连续变化导致的跳变。

### ConfirmDialog (`android/frontend/src/shared/ConfirmDialog.tsx`)
通用确认对话框，内置防重复提交保护。

*   **Props**:
    *   `open`: `boolean` (Line 16) - 控制显隐。
    *   `title`: `string` (Line 17) - 标题。
    *   `message`: `string` (Line 18) - 内容。
    *   `onConfirm`: `() => void` (Line 19) - 确认回调。
    *   `onCancel`: `() => void` (Line 20) - 取消回调。
    *   `confirmLabel`: `string` (Line 22) - 自定义确认按钮文案。
    *   `confirmVariant`: `'default' | 'destructive'` (Line 23) - 按钮样式，默认 `'destructive'`（红色）。

*   **Key Logic**:
    *   Line 26-27: 内部状态 `confirming` 用于禁用按钮，防止双击导致破坏性操作（如删除账号）。
    *   Line 29-31: `useEffect` 监听 `open` 变化，窗口关闭时强制复位 `confirming` 状态，解决慢网络下定时器提前解禁的问题。

### dailyMascot (`android/frontend/src/shared/dailyMascot.ts`)
管理每日轮换的看板娘图片资源。

*   **Constants**:
    *   `SIDE_MASCOT_POOL`: `readonly string[]` (Line 5) - 包含 14 个侧边立绘标识符，如 `'side-wave'`, `'side-laptop'` 等。

*   **Functions**:
    *   `pickDailyMascot(offset?: number): string` (Line 11):
        *   根据当前 UTC 天数计算池索引。
        *   公式：`pool[(((day + offset) % pool.length) + pool.length) % pool.length]`。
        *   `offset` 用于确保同屏多处组件在同一天显示不同的角色。
    *   `useDailyMascots(offsets: number[]): string[]` (Line 19):
        *   React Hook，返回对应 offsets 的角色列表。
        *   Line 26-35: 每分钟检查一次日期变化，若跨天则更新状态。使用 `every` 比较避免不必要的重渲染。

### ErrorBoundary (`android/frontend/src/shared/ErrorBoundary.tsx`)
捕获子树渲染错误的 Class Component。

*   **State**:
    *   `hasError`: `boolean`
    *   `error`: `Error | null`

*   **Methods**:
    *   `getDerivedStateFromError(error: Error)` (Line 24): 更新 state 以触发错误 UI。
    *   `componentDidCatch(error: Error, errorInfo: React.ErrorInfo)` (Line 29): 开发环境下打印错误日志。
    *   `render()` (Line 32):
        *   若 `hasError`，展示 `MascotFigure(variant="alert")`、错误信息及“重载”按钮。
        *   重载按钮点击后执行 `window.location.reload()`。

### FluidBackground (`android/frontend/src/shared/FluidBackground.tsx`)
简单的背景占位组件，使用 CSS 变量 `--surface-main` 填充。

*   **Props**: None.
*   **Render**: Absolute positioned div, z-index 0, pointer-events none.

### IspMark (`android/frontend/src/shared/IspMark.tsx`)
运营商/学校标识徽标。

*   **Logic**:
    *   `ispMarkSrc(suffix)`: 映射后缀到 WebP 路径。`''` 或 `null` 映射到学校徽标 `/isp/school.webp`。
    *   `IspMark` Component:
        *   Line 22-33: 若为学校徽标（`suffix === ''`），包裹在白色圆底容器中以保证浅色模式可读性。
        *   Line 34-37: 运营商 Logo 原色直出。
        *   使用 `memo` 优化性能。

### LogPanel (`android/frontend/src/shared/LogPanel.tsx`)
高级日志查看面板，支持实时轮询、过滤、搜索及动画清除。

*   **Props**:
    *   `api`: Object containing `getLogs`, `clearLogs`, `getDebugMode`, `setDebugMode`, etc. (Line 32).
    *   `addToast`: Function for notifications (Line 38).

*   **Internal State**:
    *   `rawLogs`: `string`
    *   `isLoading`, `isClearing`: Boolean flags.
    *   `lineCount`, `filterLevel`, `searchText`, `filterModule`: Filter controls.
    *   `debugMode`, `retentionDays`: Settings.
    *   `scrollRef`, `isAutoScrollRef`, `isVisibleRef`: Scroll and visibility management.

*   **Key Features**:
    *   **Parsing**: Regex `/^\[(.+?)\]\s*\[(DEBUG|INFO|WARN|ERROR)\]\s*\[(.+?)\]\s*(.+)$/` (Line 53).
    *   **Performance**:
        *   Line 82-96: `fetchLogs` 使用 `fetchSeqRef` 竞态保护，并比对 `rawLogsRef.current` 避免无变化时的 setState。
        *   Line 136-145: `parsedLines` 单次遍历解析，减少中间数组。
    *   **Android Build Optimization**: Line 56 detects `VITE_PLATFORM === 'android'` to switch log line layout to flex-col, preventing message truncation on MIUI large font modes.
    *   **Clear Animation**:
        *   Line 166-236: 使用 GSAP 对可见条目执行 stagger 删除动画。
        *   Line 223-228: `onInterrupt` 处理动画中断（如组件卸载），重置 `isClearing` 状态。
        *   Line 233-237: `setTimeout` 兜底机制，防止 WebView 后台 rAF 停摆导致动画卡死。

### MascotFigure (`android/frontend/src/shared/MascotFigure.tsx`)
统一的看板娘图片渲染组件。

*   **Variants**: `'portrait' | 'welcome' | 'empty' | 'celebrate' | 'sponsor' | 'offline' | 'alert'` (Line 5).
*   **Sizes**: `sm` (w-20), `md` (w-28), `lg` (w-40) (Line 3).
*   **Source**: `/girl/mascot-{variant}.webp`.

### RefreshButton (`android/frontend/src/shared/RefreshButton.tsx`)
带状态反馈的刷新按钮。

*   **Props**:
    *   `isRefreshing`: `boolean`
    *   `showCheck`: `boolean` - 完成后显示勾选图标。
*   **Animations**:
    *   Line 22-30: 当 `isRefreshing` 从 true 变为 false 时，添加 `refresh-shake` class 并启动定时器移除。
    *   Line 45-52: Framer Motion `AnimatePresence` 显示 Check 图标，带有 scale 弹入效果。

### SegmentTabs (`android/frontend/src/shared/SegmentTabs.tsx`)
带动画滑块的标签页导航。

*   **Props**:
    *   `tabs`: Array of `{ key, label, icon, color, bg }`.
    *   `activeKey`: String.
*   **Animation**:
    *   Line 25-29: 使用 `layoutId` 结合 `m.div` 实现背景滑块跟随活跃标签的移动动画。
    *   Line 24: 使用 `useId` 生成唯一前缀，防止多实例间布局冲突。

### SponsorCard (`android/frontend/src/shared/SponsorCard.tsx`)
移动端赞助弹窗。

*   **Props**: `open`, `onClose`.
*   **Content**: 展示微信/支付宝二维码图片，使用 `MascotFigure(variant="sponsor")`。

### ToastContainer (`android/frontend/src/shared/ToastContainer.tsx`)
全局 Toast 通知容器。

*   **Props**: `toasts: ToastMessage[]`, `onRemove: (id) => void`.
*   **Mascots**: 根据 Toast 类型自动匹配看板娘变体 (`TOAST_MASCOTS` map, Line 22)。
*   **Performance**:
    *   Line 29-31: Economy 模式下使用简单的 duration/ease 替代 spring 动画以提升性能。
    *   Line 40-42: `celebrate` 变体有特殊的 pop-in 动画。

### Types & Constants

*   **`types.ts`**:
    *   `UpdateAvailableData`, `UpdateInfo`, `DownloadProgress`, `MirrorSource`.
*   **`ui-types.ts`**:
    *   `StatusState`, `PanelName`, `ThemeName`, `LogType`.
    *   `LogEntry`: 包含 `count` 字段用于折叠重复日志。
    *   `ToastMessage`: 包含 `mascot` 字段允许自定义看板娘。
*   **`ui-constants.ts`**:
    *   `MAX_LOG_ENTRIES`: 300.
    *   `APP_VERSION`: '2.4.1'.
    *   `NAV_ITEMS`: 定义导航栏结构。

## Architecture

```text
┌──────────────────────────────────────────────────────┐
│                   App Container                      │
│                                                      │
│  ┌─────────────┐   ┌──────────────┐   ┌────────────┐│
│  │ Dashboard   │   │ Settings     │   │ Monitor    ││
│  │ (uses       │   │ (uses       │   │ (uses      ││
│  │  RefreshBtn)│   │  SegTabs)    │   │  LogPanel) ││
│  └──────┬──────┘   └──────┬───────┘   └──────┬─────┘│
│         │                 │                   │      │
│         ▼                 ▼                   ▼      │
│  ┌────────────────────────────────────────────────┐ │
│  │            Shared UI Primitives                │ │
│  │                                                │ │
│  │  ┌────────────┐  ┌────────────┐  ┌──────────┐ │ │
│  │  │ Animated   │  │ Confirm    │  │ Error    │ │ │
│  │  │ Number     │  │ Dialog     │  │ Boundary │ │ │
│  │  └────────────┘  └────────────┘  └──────────┘ │ │
│  │  ┌────────────┐  ┌────────────┐  ┌──────────┐ │ │
│  │  │ Toast      │  │ Sponsor    │  │ Update   │ │ │
│  │  │ Container  │  │ Card       │  │ Dialog   │ │ │
│  │  └────────────┘  └────────────┘  └──────────┘ │ │
│  └────────────────────────────────────────────────┘ │
│         ▲                 ▲                   ▲      │
│         │                 │                   │      │
│  ┌─────────────┐   ┌──────────────┐           │      │
│  │ Mascot      │   │ Daily        │           │      │
│  │ Figure      │   │ Mascot Hook  │           │      │
│  └─────────────┘   └──────────────┘           │      │
└──────────────────────────────────────────────────────┘
```

1.  **Presentation Layer**: `LogPanel`, `SponsorCard`, `UpdateAvailableDialog` 是较复杂的组合组件，它们消费底层的 `ConfirmDialog`, `MascotFigure`, `RefreshButton` 等原子组件。
2.  **Animation Engine**: `AnimatedNumber` 依赖 GSAP，而 `ToastContainer`, `RefreshButton`, `SegmentTabs`, `LogPanel` 大量使用 Framer Motion (`m.div`, `AnimatePresence`)。两者通过 `useAnimationProfile` hook 协调性能策略（如 economy 档降级）。
3.  **State Management**: `dailyMascot` 提供纯函数和 Hook，不依赖 Redux/Zustand，仅基于时间戳计算状态，易于测试和复用。
4.  **Error Handling**: `ErrorBoundary` 作为 React 组件树的父级包裹者，捕获渲染错误，并通过 `MascotFigure` 提供友好的故障转移 UI。

## Data Flow

1.  **Mascot Rotation**:
    *   `useDailyMascots` 获取当前 UTC 天数。
    *   通过 `pickDailyMascot` 映射到 `SIDE_MASCOT_POOL` 中的特定角色。
    *   每 60 秒检查一次日期变更，若跨天则触发重渲染。
    *   结果传递给 `MascotFigure` 渲染对应的 WebP 图片。

2.  **Log Panel Fetching**:
    *   `LogPanel` 初始化时调用 `api.getLogs`。
    *   设置 `setInterval(5000)` 轮询。
    *   **Visibility Gate**: 利用 `IntersectionObserver` 和 `document.hidden` 判断面板是否可见且前台运行，不可见时跳过轮询。
    *   **Deduplication**: 新数据到达时，比对 `rawLogsRef.current`，若无变化则不触发 `setState`，从而避免下游 `useMemo` (parsedLines, filteredLines 等) 的全量重算。

3.  **Toast Notification**:
    *   外部 Store 或 Hook 产生 `ToastMessage`。
    *   `ToastContainer` 接收 `toasts` 数组。
    *   `AnimatePresence` 监听数组变化，为新条目执行进入动画，为移除条目执行退出动画。
    *   用户点击关闭或超时后，调用 `onRemove(id)` 从数组中剔除。

## Connections

*   [[frontend-dashboard]]: 使用 `RefreshButton`, `AnimatedNumber`, `MascotFigure`。
*   [[frontend-settings]]: 使用 `SegmentTabs`, `ConfirmDialog`, `SponsorCard`。
*   [[frontend-monitor]]: 使用 `LogPanel`, `ErrorBoundary`。
*   [[hooks-use-animation-profile]]: 被 `AnimatedNumber`, `ToastContainer`, `LogPanel` 引用以获取性能配置。
*   [[components-ui-dialog]]: 被 `ConfirmDialog`, `SponsorCard`, `UpdateAvailableDialog` 封装使用。

## Known Issues

1.  **GSAP Cleanup in AnimatedNumber**:
    *   File: `android/frontend/src/shared/AnimatedNumber.tsx`, Lines 40-48.
    *   Issue: `quickTo` 创建的底层 Tween 在组件卸载后可能仍存活，尝试写入已卸载的 DOM。
    *   Mitigation: Cleanup 函数中显式调用 `gsap.killTweensOf`。注释指出这是历史缺陷的修复，当前代码已包含保护逻辑（`if (!ref.current) return`）。

2.  **ConfirmDialog Double Submit**:
    *   File: `android/frontend/src/shared/ConfirmDialog.tsx`, Lines 26-27.
    *   Issue: 快速双击可能导致两次破坏性操作。
    *   Mitigation: 引入 `confirming` 状态禁用按钮。

3.  **Scale Bounce Glitch in AnimatedNumber**:
    *   File: `android/frontend/src/shared/AnimatedNumber.tsx`, Lines 73-79.
    *   Issue: 快速连续变化时，多个 `delayedCall` 堆积导致 scale 跳变。
    *   Mitigation: 在注册新 timer 前 kill 旧 timer。

4.  **Log Clear Animation Freeze in Android WebView**:
    *   File: `android/frontend/src/shared/LogPanel.tsx`, Lines 233-237.
    *   Issue: WebView 后台运行时 rAF 停摆，GSAP tween 冻结，导致 `isClearing` 状态卡死且日志重现。
    *   Mitigation: 使用 `setTimeout` 作为兜底，强制结算并 revert context，与动画时序解耦。

5.  **Layout ID Collision in SegmentTabs**:
    *   File: `android/frontend/src/shared/SegmentTabs.tsx`, Line 24.
    *   Issue: 多个 `SegmentTabs` 实例并存时，共享相同的 `layoutId` 会导致 Framer Motion 错误共享布局动画。
    *   Mitigation: 使用 `useId` 生成实例唯一的前缀。