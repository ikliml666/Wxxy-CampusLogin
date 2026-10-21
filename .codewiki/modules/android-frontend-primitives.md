---
title: android-frontend-primitives
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
tags: [android, frontend, ui, primitives, react, shadcn]
---

## Overview

本模块 `android-frontend-primitives` 是 Android 前端应用的基础 UI 原语集合。它基于 React 和 Tailwind CSS 构建，旨在提供一套可复用、风格统一且具备良好交互体验的原子组件。该模块封装了常见的用户界面元素，包括卡片、按钮、输入框、对话框、选择器、开关及提示工具等。所有组件均遵循无障碍（Accessibility）标准，支持深色模式，并集成了 GSAP 动画库以增强视觉反馈。

## Key Components

### AnimatedCard (`animated-card.tsx`)
一个具有高级交互效果的卡片容器，支持 3D 倾斜、入场动画及性能优化。

- **接口**: `AnimatedCardConfig` (6-12|)
  - `glowIntensity`: 光晕强度 (可选)
  - `hoverScale`: 悬停缩放比例 (可选)
  - `stiffness`, `damping`, `mass`: 物理动画参数 (可选)
- **Props**: `AnimatedCardProps` (14-21|)
  - `animationConfig`: 动画配置对象
  - `noHover`: 禁用悬停效果
  - `noAnimation`: 禁用所有动画
  - `noEnterAnimation`: 禁用入场动画
  - `enableTilt`: 启用 3D 倾斜效果
  - `staggerIndex`: 交错动画索引
- **常量**: `reducedMotionQuery` (25-27|)
  - 模块级缓存的 `MediaQueryList`，用于检测用户是否偏好减少运动，避免每次渲染重复查询。
- **函数**: `AnimatedCard` (29-151|)
  - 核心组件实现，使用 `React.memo` 和 `forwardRef` 优化。
  - **Effect**: `useEffect` (41-61|)
    - 初始化 GSAP `quickTo` 用于旋转动画。
    - 设置 `ResizeObserver` 监听尺寸变化。
    - Cleanup 逻辑：取消 RAF、kill Tween、重置引用，防止内存泄漏或操作已卸载节点。
  - **Callback**: `handleMouseMove` (63-87|)
    - 处理鼠标移动事件，计算光标相对于卡片的归一化坐标。
    - 使用 RAF (RequestAnimationFrame) 节流 DOM 读写。
    - 缓存 `DOMRect` 以避免强制同步布局。
  - **Callback**: `handleMouseLeave` (89-101|)
    - 处理鼠标离开事件，重置旋转并延迟清除 `willChange` 样式。
    - 修复历史缺陷：保存 `delayedCall` 引用以便在卸载时 kill。
  - **Render**: (113-149|)
    - 根据 `noAnimation` 决定返回静态 div 或带动画类的 div。
    - 内部嵌套两层 div：外层处理交互类名，内层处理背景色和 3D 变换 (`preserve-3d`)。

### Badge (`badge.tsx`)
显示状态、计数或标签的小组件。

- **Variants**: `badgeVariants` (5-30|)
  - 使用 `cva` 定义变体。
  - `variant`: `default`, `secondary`, `destructive`, `outline`, `success`, `warning`, `info`, `ghost`。
  - `size`: `default`, `sm`, `lg`。
- **Props**: `BadgeProps` (32-34|)
  - 扩展 `HTMLAttributes<HTMLDivElement>` 和 `VariantProps<typeof badgeVariants>`。
- **函数**: `Badge` (36-39|)
  - 基础渲染组件，合并 className 和变体样式。
- **导出**: `badgeVariants` (42|)
  - 允许外部访问变体逻辑以自定义样式。

### Button (`button.tsx`)
多功能按钮，支持加载状态、图标插槽及鼠标跟随光影效果。

- **Variants**: `buttonVariants` (6-34|)
  - `variant`: `default`, `destructive`, `outline`, `secondary`, `ghost`, `link`, `glass`, `soft`。
  - `size`: `default`, `sm`, `lg`, `icon`, `icon-sm`, `icon-lg`。
- **Props**: `ButtonProps` (36-41|)
  - `asChild`: 是否作为子组件传递（使用 Radix Slot）。
  - `isLoading`: 是否显示加载 spinner。
- **函数**: `Button` (43-171|)
  - **asChild 分支** (47-84|): 使用 `<Slot>` 渲染，注入 loading SVG 和 children。
  - **非 asChild 分支** (105-168|):
    - 过滤掉拖拽和动画相关的事件属性 (`motionProps`)。
    - **RAF 节流与去抖** (94-136|):
      - 使用 `rafRef` 和 `lastPosRef` 实现鼠标位置去抖（<3px 跳过）和 RAF 节流。
      - 动态设置 CSS 变量 `--mouse-x` 和 `--mouse-y` 供 CSS 使用。
      - Cleanup 中 cancel RAF。
    - **Loading State**: 显示旋转 SVG 图标。

### Card (`card.tsx`)
卡片容器及其语义化子组件。

- **组件**:
  - `CardHeader` (4-14|): 头部区域，flex 布局，垂直间距 1.5，padding 5。
  - `CardTitle` (16-26|): 标题，h3 标签，粗体，无行高追踪。
  - `CardDescription` (28-38|): 描述文本，小字号，muted 颜色。
  - `CardContent` (40-46|): 内容区域，padding 5，顶部 padding 0。
  - `CardIcon` (48-64|): 图标容器，固定宽高 10x10，圆角，primary 背景透明度 10%。

### Dialog (`dialog.tsx`)
基于 Radix UI 的模态对话框。

- **组件**:
  - `Dialog` (7|): Root 别名。
  - `DialogPortal` (9|): Portal 别名。
  - `DialogOverlay` (11-24|): 遮罩层，固定定位，z-50，黑色半透明背景，入场动画类 `dialog-overlay-enter`。
  - `DialogContent` (26-54|):
    - 包含 Overlay 和居中容器。
    - 实际内容区 `DialogPrimitive.Content` 拥有圆角、阴影、滚动行为控制。
    - 关闭按钮：右上角绝对定位，显示 X 图标，国际化文案 `common.close`。
    - Props: `showClose` (默认 true)。
  - `DialogHeader` (56-68|): 对话框头部，flex 列布局。
  - `DialogTitle` (70-83|): 对话框标题，大字号，粗体。
  - `DialogDescription` (85-95|): 对话框描述，小字号，muted 颜色。

### EmptyState (`empty-state.tsx`)
统一的数据为空状态展示组件。

- **Props**: `EmptyStateProps` (8-16|)
  - `icon`: Lucide 图标组件。
  - `title`: 标题字符串。
  - `description`: 可选描述。
  - `action`: 可选动作节点。
  - `compact`: 紧凑模式，用于卡内嵌入。
- **函数**: `EmptyState` (18-31|)
  - 根据 `compact` 调整间距、图标大小、字体大小。
  - 图标容器使用 `bg-primary/10` tonal 底色，线性细描边图标。

### Input (`input.tsx`)
带图标和错误状态的输入框。

- **Props**: `InputProps` (4-8|)
  - `icon`: 左侧图标节点。
  - `error`: 错误信息字符串。
- **函数**: `Input` (10-38|)
  - 结构：相对容器 -> 绝对图标 -> Input 元素 -> 错误文本。
  - 错误态：添加 `border-destructive` 和 `focus-visible:ring-destructive`。
  - 图标存在时增加左侧 padding (`pl-10`)。

### Label (`label.tsx`)
表单标签。

- **Variants**: `labelVariants` (6-8|)
  - 小字号，中等字重，禁用状态下 cursor 不可用且不透明度降低。
- **Props**: `Label` (10-24|)
  - `required`: 是否在右侧添加红色星号。
  - 使用 Radix Label Primitive。

### Select (`select.tsx`)
下拉选择器。

- **组件**:
  - `Select` (6|): Root。
  - `SelectValue` (8|): Value。
  - `SelectTrigger` (10-34|):
    - 支持 `icon` prop，左侧绝对定位图标。
    - 右侧 ChevronDown 图标。
  - `SelectContent` (36-64|):
    - Portal 渲染。
    - 支持 `position` ('popper' 默认)。
    - 包含 Viewport 以保持宽度一致。
  - `SelectItem` (66-88|):
    - 使用 `data-[highlighted]` 而非 `focus:` 来处理键盘/鼠标高亮。
    - 左侧 Check 图标指示器。
  - `SelectSeparator` (90-100|): 分隔线。

### Separator (`separator.tsx`)
分隔线。

- **Props**:
  - `orientation`: 'horizontal' (默认) 或 'vertical'。
  - `decorative`: 默认为 true。
- **样式**:
  - 水平：高度 1px，全宽。
  - 垂直：宽度 1px，全高。

### Switch (`switch.tsx`)
开关控件。

- **Props**:
  - `size`: 'default', 'sm', 'lg'。
- **样式**:
  - Track: 圆角全屏，checked 时 primary 背景，unchecked 时 input 背景。
  - Thumb: 圆形，checked 时 translate-x 偏移，尺寸随 size 变化。

### Tooltip (`tooltip.tsx`)
悬浮提示框。

- **组件**:
  - `TooltipProvider` (5|): Provider。
  - `Tooltip` (7|): Root。
  - `TooltipTrigger` (9|): Trigger。
  - `TooltipPortal` (11|): Portal。
  - `TooltipContent` (13-29|):
    - 默认 sideOffset 4。
    - 包含进入/退出动画类 (`animate-in`, `fade-in-0`, `zoom-in-95` 等)。

## Architecture

```text
┌──────────────────────────────────────────────────────┐
│                   android-frontend-primitives        │
│                                                      │
│  ┌─────────────┐   ┌─────────────┐   ┌────────────┐ │
│  │  Layout     │   │  Form       │   │  Feedback  │ │
│  │             │   │             │   │            │ │
│  │ CardHeader  │   │ Input       │   │ Badge      │ │
│  │ CardTitle   │   │ Label       │   │ EmptyState │ │
│  │ CardContent │   │ Select      │   │ Dialog     │ │
│  │ CardIcon    │   │ Switch      │   │ Tooltip    │ │
│  │             │   │             │   │            │ │
│  │ AnimatedCard│   │             │   │ Separator  │ │
│  └──────┬──────┘   └──────┬──────┘   └─────┬──────┘ │
│         │                 │                │        │
│         ▼                 ▼                ▼        │
│  ┌──────────────────────────────────────────────┐   │
│  │              Shared Utilities                │   │
│  │  cn() from @/lib/utils                       │   │
│  │  cva (class-variance-authority)              │   │
│  │  Radix UI Primitives (@radix-ui/react-*)     │   │
│  │  Lucide Icons                                │   │
│  │  GSAP (for AnimatedCard & Button hover)      │   │
│  └──────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────┘
```

## Data Flow

1.  **User Interaction**:
    *   User hovers over `Button` or `AnimatedCard`.
    *   `onMouseMove` event triggers.
    *   Coordinates are captured and throttled via `requestAnimationFrame`.
    *   CSS variables (`--mouse-x`, `--mouse-y`) or GSAP tweens update the visual state.
2.  **Form Submission/Validation**:
    *   `Input` receives `error` prop.
    *   If `error` is present, it renders a destructive border and error message below the input.
    *   `Label` with `required` prop injects a red asterisk.
3.  **Modal/Dialog Opening**:
    *   `Dialog.Trigger` activates `Dialog.Root`.
    *   `Dialog.Portal` mounts the overlay and content into the DOM body.
    *   `DialogContent` handles focus trapping and close button logic.
4.  **Selection**:
    *   `Select.Trigger` opens `Select.Content`.
    *   `Select.Item` updates `Select.Value` upon click.
    *   Keyboard navigation uses `data-highlighted` for visual feedback.

## Connections

*   [[android-frontend-lib-utils]]: Provides `cn()` utility for conditional class merging.
*   [[android-frontend-hooks]]: Provides `useAnimationProfile` hook used by `AnimatedCard`.
*   [[android-frontend-i18n]]: Used by `Dialog` for closing button text translation.
*   [[android-frontend-themes]]: Defines color tokens (primary, background, muted, etc.) used across all components.

## Known Issues

1.  **Memory Leaks in Animation Cleanup**:
    *   **Location**: `animated-card.tsx` lines 52-55, `button.tsx` lines 97-103.
    *   **Issue**: Historical defects where RAF callbacks or GSAP delayed calls were not properly cancelled on component unmount, potentially writing to unmounted nodes.
    *   **Status**: Fixed in current codebase by saving refs to timers/calls and killing them in `useEffect` cleanup. However, developers must ensure all animation-related refs are checked before execution.
2.  **Event CurrentTarget Nullification**:
    *   **Location**: `animated-card.tsx` lines 65-67, `button.tsx` lines 121-124.
    *   **Issue**: React 19 (and synthetic event pooling in older versions) nullifies `e.currentTarget` after the handler returns if the callback is asynchronous (like RAF).
    *   **Status**: Mitigated by synchronously capturing `e.currentTarget` and coordinates within the synchronous part of the event handler before passing them to the RAF callback.
3.  **Select Highlighting Logic**:
    *   **Location**: `select.tsx` line 75.
    *   **Issue**: Radix Select Item does not apply `focus:` styles because it doesn't set DOM focus.
    *   **Status**: Fixed by using `data-[highlighted]` attribute for styling active items instead of `:focus`.