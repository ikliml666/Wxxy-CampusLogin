---
title: android-frontend-ui-primitives
type: module
source_files:
  - android/frontend/src/components/ui/animated-card.tsx
  - android/frontend/src/components/ui/badge.tsx
  - android/frontend/src/components/ui/button.tsx
  - android/frontend/src/components/ui/card.tsx
  - android/frontend/src/components/ui/dialog.tsx
  - android/frontend/src/components/ui/empty-state.tsx
  - android/frontend/src/components/ui/input.tsx
  - android/frontend/src/components/ui/label.tsx
  - android/frontend/src/components/ui/select.tsx
  - android/frontend/src/components/ui/separator.tsx
  - android/frontend/src/components/ui/switch.tsx
  - android/frontend/src/components/ui/tooltip.tsx
tags: [ui, primitives, tailwind, radix, gsap, accessibility]
---

## Overview
本模块 `android-frontend-ui-primitives` 是 Android 前端应用的基础 UI 组件库。它基于 **Tailwind CSS** 进行样式构建，利用 **Radix UI** 处理无头（Headless）交互逻辑与无障碍访问（A11y），并结合 **Class Variance Authority (CVA)** 实现变体管理。此外，模块引入了 **GSAP** 用于高性能动画处理。

该模块提供了按钮、卡片、对话框、输入框、选择器、开关等原子级组件，旨在为上层业务页面提供一致的设计语言、交互反馈和视觉规范。所有组件均支持深色模式（Dark Mode），并通过 CSS 变量（如 `bg-card`, `text-primary`）适配主题系统。

## Key Components

### AnimatedCard (`animated-card.tsx`)
具有 3D 倾斜效果和入场动画的卡片容器。
- **接口**: `AnimatedCardProps` (14|)
  - `animationConfig`: 配置发光强度、缩放比例、物理参数 (7|)
  - `noHover`: 禁用悬停效果 (16|)
  - `noAnimation`: 完全禁用动画 (17|)
  - `enableTilt`: 启用 3D 倾斜 (19|)
  - `staggerIndex`: 交错动画索引 (20|)
- **核心逻辑**:
  - **MediaQuery 缓存**: 在模块级别缓存 `prefers-reduced-motion` 查询结果，避免重复创建 `MediaQueryList` (25-27|)。
  - **3D Tilt 引擎**: 使用 `gsap.quickTo` 监听鼠标移动，计算相对坐标并应用 `rotateX/Y` 变换 (44-45|, 82-85|)。
  - **RAF 节流**: 使用 `requestAnimationFrame` 限制 DOM 读写频率，防止布局抖动 (71-86|)。
  - **内存泄漏修复**: 在 `useEffect` 清理函数中显式取消 RAF、kill GSAP tween 及重置 `willChange` 样式，防止组件卸载后操作已销毁节点 (49-60|)。
  - **Rect 缓存**: 使用 `ResizeObserver` 和 ref 缓存 `getBoundingClientRect` 结果，减少强制重排 (47-48|, 77-81|)。

### Button (`button.tsx`)
多功能按钮，支持多种变体和加载状态。
- **接口**: `ButtonProps` (36-41|)
  - `asChild`: 是否作为 Radix Slot 渲染以继承父元素样式 (39|)
  - `isLoading`: 显示加载旋转图标 (40|)
- **变体定义**: `buttonVariants` (6-34|)
  - **Variants**: `default`, `destructive`, `outline`, `secondary`, `ghost`, `link`, `glass`, `soft` (10-19|)
  - **Sizes**: `default`, `sm`, `lg`, `icon`, `icon-sm`, `icon-lg` (20-27|)
- **特性**:
  - **Pointer Follow Effect**: 通过 RAF 节流更新 CSS 变量 `--mouse-x` 和 `--mouse-y`，配合 CSS 实现背景跟随鼠标的效果 (94-136|)。
  - **Motion Props 剥离**: 从 props 中提取拖拽和动画相关事件，避免传递给原生 button 导致警告或冲突 (86-91|)。
  - **Loading State**: 渲染 SVG 旋转图标，并禁用按钮点击 (55-81|, 140-166|)。

### Badge (`badge.tsx`)
轻量级标签组件。
- **接口**: `BadgeProps` (32-34|)
- **变体定义**: `badgeVariants` (5-30|)
  - **Variants**: `default`, `secondary`, `destructive`, `outline`, `success`, `warning`, `info`, `ghost` (9-18|)
  - **Sizes**: `default`, `sm`, `lg` (19-23|)

### Card Family (`card.tsx`)
组合式卡片组件，分为 Header, Title, Description, Content, Icon 五个部分。
- **CardHeader**: 头部容器，带间距和内边距 (4-14|)
- **CardTitle**: 标题，语义化为 h3 (16-26|)
- **CardDescription**: 描述文本，使用 muted 颜色 (28-38|)
- **CardContent**: 内容区域，顶部内边距为 0 (40-46|)
- **CardIcon**: 图标容器，固定宽高比和背景色 (48-64|)

### Dialog (`dialog.tsx`)
基于 Radix Dialog 模态框。
- **DialogOverlay**: 遮罩层，包含进入动画类名 (11-24|)
- **DialogContent**: 内容容器，包含关闭按钮 (26-54|)
  - `showClose`: 控制是否显示右上角关闭叉号 (28|)
  - 集成 `i18next` 提供无障碍屏幕阅读器文本 (46|)
- **DialogHeader/Title/Description**: 内部结构组件 (56-95|)

### Input (`input.tsx`)
带图标和错误状态的输入框。
- **接口**: `InputProps` (4-8|)
  - `icon`: 左侧图标节点 (6|)
  - `error`: 错误提示信息字符串 (7|)
- **样式**:
  - 默认背景为 `muted/40`，聚焦时 ring 高亮 (21-22|)
  - 错误状态下强制添加 `border-destructive` 和 `focus-visible:ring-destructive` (25|)

### Select (`select.tsx`)
基于 Radix Select 的下拉菜单。
- **SelectTrigger**: 触发器，支持左侧图标 (10-34|)
- **SelectContent**: 下拉面板，默认使用 `popper` 定位以避开视口边界 (36-64|)
- **SelectItem**: 选项项，使用 `data-[highlighted]` 属性处理键盘导航高亮，而非 `focus:` (72-87|)
- **SelectSeparator**: 分隔线 (90-100|)

### Switch (`switch.tsx`)
开关控件。
- **Size 支持**: `default` (h-5 w-9), `sm` (h-4 w-7), `lg` (h-6 w-11) (14-16|)
- **Thumb 动画**: 根据 checked 状态平移 thumb，尺寸随 size prop 变化 (22-29|)

### Tooltip (`tooltip.tsx`)
悬浮提示框。
- **TooltipContent**: 内容组件，包含进入/退出动画类名 (13-29|)

### EmptyState (`empty-state.tsx`)
统一空状态展示组件。
- **Props**: `icon` (LucideIcon), `title`, `description`, `action`, `compact` (8-16|)
- **设计**: tonal 底色圆角容器，线性细描边图标 (21-22|)

### Label (`label.tsx`)
可访问性标签。
- **Required Indicator**: 自动在末尾插入红色星号 `*` (21|)

### Separator (`separator.tsx`)
分割线。
- **Orientation**: 支持 `horizontal` 和 `vertical` (10|)

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                     android-frontend-ui-primitives           │
├──────────────────────────────────────────────────────────────┤
│  Dependencies: Tailwind CSS | Radix UI | CVA | GSAP         │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────┐   ┌─────────────┐   ┌──────────────────┐  │
│  │  Layout      │   │  Form       │   │  Feedback        │  │
│  │  Components  │   │  Controls   │   │  & Overlay       │  │
│  ├─────────────┤   ├─────────────┤   ├──────────────────┤  │
│  │ • CardFamily │   │ • Button    │   │ • Dialog         │  │
│  │ • Separator  │   │ • Input     │   │ • Tooltip        │  │
│  │              │   │ • Select    │   │                  │  │
│  │              │   │ • Switch    │   │                  │  │
│  │              │   │ • Label     │   │                  │  │
│  └─────────────┘   └─────────────┘   └──────────────────┘  │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Specialized / Enhanced                              │   │
│  ├──────────────────────────────────────────────────────┤   │
│  │ • AnimatedCard (GSAP + RAF Throttle + Tilt)          │   │
│  │ • Badge (Variant Management)                         │   │
│  │ • EmptyState (Design System Primitive)               │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **Theme Resolution**:
    *   All components rely on Tailwind CSS classes mapped to semantic tokens (e.g., `bg-primary`, `text-muted-foreground`).
    *   These tokens are defined in the global `index.css` (not shown in source but referenced in comments like 103|).
    *   Dark mode is handled automatically by Tailwind's `dark:` variants or CSS variable overrides.

2.  **Interaction Handling (Button Example)**:
    *   User moves mouse over `Button`.
    *   `onMouseMove` event fires (110|).
    *   Debounce check: if displacement < 3px, return (116|).
    *   Capture current coordinates and element reference synchronously (119-124|).
    *   Cancel previous RAF if exists (128|).
    *   Schedule new RAF callback (129|).
    *   In RAF: calculate relative percentage positions, set CSS variables `--mouse-x` and `--mouse-y` on the element (130-135|).
    *   CSS uses these variables for background gradient positioning.

3.  **Animation Lifecycle (AnimatedCard)**:
    *   **Mount**: `useEffect` sets up `ResizeObserver` and GSAP `quickTo` functions (41-61|).
    *   **Hover**: `handleMouseMove` calculates tilt angles using cached rect and client coordinates (63-87|).
    *   **Leave**: `handleMouseLeave` resets rotation to 0 and schedules a delayed cleanup of `willChange` style (89-101|).
    *   **Unmount**: Cleanup function cancels all pending animations, observers, and refs to prevent memory leaks (49-60|).

## Connections

*   [[android-frontend-lib-utils]]: Provides the `cn` utility function used across all components for conditional class merging.
*   [[android-frontend-hooks-use-animation-profile]]: Used by `AnimatedCard` to determine system-level animation preferences (tilt enablement).
*   [[android-frontend-i18n]]: Used by `Dialog` for accessible close button text via `i18next.t`.

## Known Issues

1.  **AnimatedCard Memory Leaks (Fixed)**:
    *   Historically, `tiltRafRef` was not cancelled on unmount, leading to writes on unmounted nodes (Comment at 52|).
    *   `resetWillChangeTimerRef` (delayedCall) was not saved as a ref, causing multiple callbacks to stack and execute after unmount (Comment at 93-94|).
    *   **Fix**: Added explicit cancellation in `useEffect` return (53-55|) and ref management for the timer (95-97|).

2.  **React Event Pooling / CurrentTarget Null**:
    *   In both `Button` (121-123|) and `AnimatedCard` (65-67|), comments note that React may nullify `e.currentTarget` before the RAF callback executes.
    *   **Mitigation**: Both components synchronously capture `e.currentTarget` and coordinates inside the event handler before scheduling the RAF.

3.  **Select Highlighting**:
    *   Radix `SelectItem` does not apply `focus:` styles because items are not focused in the DOM; they use `data-highlighted`.
    *   **Fix**: Styles must use `data-[highlighted]:bg-accent` instead of `focus:bg-accent` (Comment at 73-74|).