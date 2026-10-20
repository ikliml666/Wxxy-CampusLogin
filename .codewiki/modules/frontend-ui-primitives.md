---
title: frontend-ui-primitives
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
tags: [ui, primitives, tailwind, radix, gsap]
---

## Overview
`frontend-ui-primitives` 是前端应用的基础 UI 原子组件库。该模块封装了按钮、卡片、对话框、输入框、选择器、开关等标准控件，旨在提供一致的设计语言和交互体验。底层基于 Tailwind CSS 进行样式构建，利用 `class-variance-authority` (CVA) 管理变体（variants），并集成 Radix UI primitives 处理无头组件的状态管理与无障碍访问（A11y）。此外，部分高级组件（如 `AnimatedCard` 和 `Button`）集成了 GSAP 动画库以实现高性能的 3D 倾斜和鼠标跟随效果。

## Key Components

### AnimatedCard (`android/frontend/src/components/ui/animated-card.tsx`)
实现带有 3D 倾斜效果和入场动画的卡片容器。

- **接口**:
  - `AnimatedCardConfig`: 定义动画参数，包括 `glowIntensity`, `hoverScale`, `stiffness`, `damping`, `mass`（Line 6-12）。
  - `AnimatedCardProps`: 继承自 `React.HTMLAttributes<HTMLDivElement>`，包含：
    - `animationConfig`: 自定义动画配置（Line 15）。
    - `noHover`: 禁用悬停效果（Line 16）。
    - `noAnimation`: 完全禁用动画（Line 17）。
    - `noEnterAnimation`: 禁用入场动画（Line 18）。
    - `enableTilt`: 启用/禁用 3D 倾斜（Line 19）。
    - `staggerIndex`: 交错动画索引（Line 20）。

- **导出常量**:
  - `REST_SHADOW`: 静态阴影样式字符串 `'0 1px 3px rgba(0,0,0,0.03), 0 1px 2px rgba(0,0,0,0.02)'`（Line 23）。
  - `reducedMotionQuery`: 模块级缓存的 `matchMedia` 查询对象，用于检测用户是否偏好减少运动（Line 27-29）。

- **函数/组件**:
  - `AnimatedCard` (Line 31-151): 核心组件。
    - 使用 `React.memo` 和 `forwardRef` 优化渲染。
    - `useAnimationProfile()`: 获取全局动画偏好设置（Line 33）。
    - `handleMouseMove` (Line 65-89): 处理鼠标移动，计算光标相对于卡片的位置，并通过 RAF 节流更新旋转角度。
    - `handleMouseLeave` (Line 91-103): 重置旋转角度，并延迟重置 `will-change` 属性。
    - `cardClassName` (Line 105-108): 使用 `cn` 合并基础类名。
    - 逻辑分支：若 `noAnimation` 为真，返回静态 `div`；否则返回带有 `perspective` 和事件监听器的复杂结构。

### Badge (`android/frontend/src/components/ui/badge.tsx`)
显示状态标签或计数的小组件。

- **类型**:
  - `BadgeProps`: 继承 `React.HTMLAttributes<HTMLDivElement>` 和 `VariantProps<typeof badgeVariants>`（Line 32-34）。

- **函数**:
  - `Badge` (Line 36-40): 渲染 `div`，应用 CVA 生成的类名。

- **导出常量**:
  - `badgeVariants` (Line 5-30): CVA 配置。
    - `variant`: `default`, `secondary`, `destructive`, `outline`, `success`, `warning`, `info`, `ghost`。
    - `size`: `default`, `sm`, `lg`。

### Button (`android/frontend/src/components/ui/button.tsx`)
可点击的操作按钮，支持加载状态和鼠标跟随光效。

- **类型**:
  - `ButtonProps`: 继承 `React.ButtonHTMLAttributes<HTMLButtonElement>` 和 `VariantProps<typeof buttonVariants>`，新增 `asChild?: boolean` 和 `isLoading?: boolean`（Line 36-41）。

- **函数**:
  - `Button` (Line 43-170): 核心组件。
    - 若 `asChild` 为真，使用 `@radix-ui/react-slot` 的 `Slot` 组件包裹子元素（Line 47-84）。
    - 若 `isLoading` 为真，渲染 SVG 旋转图标（Line 55-78, 140-163）。
    - 非 `asChild` 模式下，拦截拖拽和动画相关的事件属性（Line 86-91），并将剩余属性传递给 `button`。
    - `onMouseMove` (Line 110-137): 实现鼠标位置去抖（<3px 跳过）和 RAF 节流，动态设置 CSS 变量 `--mouse-x` 和 `--mouse-y` 以驱动背景光效。
    - Cleanup (Line 99-103): 在卸载时取消未完成的 RAF。

- **导出常量**:
  - `buttonVariants` (Line 6-34): CVA 配置。
    - `variant`: `default`, `destructive`, `outline`, `secondary`, `ghost`, `link`, `glass`, `soft`。
    - `size`: `default`, `sm`, `lg`, `icon`, `icon-sm`, `icon-lg`。

### Card (`android/frontend/src/components/ui/card.tsx`)
卡片布局的结构化组件集合。

- **函数**:
  - `CardHeader` (Line 4-14): 卡片头部，flex 列布局，内边距 p-5。
  - `CardTitle` (Line 16-26): 卡片标题，h3 标签，semibold 字体。
  - `CardDescription` (Line 28-38): 卡片描述，text-sm muted 颜色。
  - `CardContent` (Line 40-46): 卡片内容区，p-5 pt-0。
  - `CardIcon` (Line 48-64): 图标容器，固定尺寸 h-10 w-10，primary 背景色半透明。

- **导出**:
  - `CardIcon`, `CardHeader`, `CardTitle`, `CardDescription`, `CardContent`。
  - *注意*: 未导出根 `Card` 组件，通常由 `AnimatedCard` 或直接使用 div 替代。

### Dialog (`android/frontend/src/components/ui/dialog.tsx`)
模态对话框组件，基于 Radix UI。

- **函数/组件**:
  - `Dialog` (Line 7): `DialogPrimitive.Root` 别名。
  - `DialogPortal` (Line 9): `DialogPrimitive.Portal` 别名。
  - `DialogOverlay` (Line 11-24): 遮罩层，固定定位 z-50，bg-black/60。
  - `DialogContent` (Line 26-54): 内容容器。
    - 接受 `showClose?: boolean` 属性。
    - 内部包含 `DialogPortal` -> `DialogOverlay` -> 居中容器 -> `DialogPrimitive.Content`。
    - 关闭按钮使用 `lucide-react` 的 `X` 图标，并集成 i18next 翻译。
  - `DialogHeader` (Line 56-68): 对话框头部布局。
  - `DialogTitle` (Line 70-83): 对话框标题。
  - `DialogDescription` (Line 85-95): 对话框描述。

- **导出**:
  - `Dialog`, `DialogContent`, `DialogHeader`, `DialogTitle`, `DialogDescription`。

### EmptyState (`android/frontend/src/components/ui/empty-state.tsx`)
统一空状态展示组件。

- **接口**:
  - `EmptyStateProps`:
    - `icon`: Lucide 图标组件。
    - `title`: 标题字符串。
    - `description?`: 描述字符串。
    - `action?`: 可选动作节点。
    - `compact?`: 紧凑模式标志。
    - `className?`: 额外类名。

- **函数**:
  - `EmptyState` (Line 18-31):
    - 根据 `compact` 调整间距和尺寸。
    - 图标容器使用 `rounded-2xl bg-primary/10`。
    - 标题使用 `font-medium text-foreground/80`。
    - 描述使用 `text-muted-foreground/70` 并限制最大宽度。

### Input (`android/frontend/src/components/ui/input.tsx`)
带图标和错误状态的输入框。

- **接口**:
  - `InputProps`: 继承 `React.InputHTMLAttributes<HTMLInputElement>`，新增 `icon?: React.ReactNode` 和 `error?: string`（Line 4-8）。

- **函数**:
  - `Input` (Line 10-37):
    - 相对定位容器。
    - 若存在 `icon`，在左侧绝对定位显示（Line 14-18）。
    - 若存在 `error`，添加红色边框和下方错误文本提示（Line 25, 31-33）。
    - 输入框本身使用 `bg-muted/40` 背景。

### Label (`android/frontend/src/components/ui/label.tsx`)
表单标签。

- **接口**:
  - `LabelProps`: 继承 Radix Label 属性，新增 `required?: boolean`（Line 10-13）。

- **函数**:
  - `Label` (Line 10-24):
    - 使用 `LabelPrimitive.Root`。
    - 若 `required` 为真，在末尾追加红色 `*` 号（Line 21）。

### Select (`android/frontend/src/components/ui/select.tsx`)
下拉选择器。

- **函数/组件**:
  - `Select` (Line 6): Root 别名。
  - `SelectValue` (Line 8): Value 别名。
  - `SelectTrigger` (Line 10-34): 触发器。
    - 支持 `icon` 属性，在左侧显示图标。
    - 右侧固定显示 `ChevronDown` 图标。
  - `SelectContent` (Line 36-64): 内容面板。
    - 默认 `position="popper"`。
    - 包含进入/退出动画类。
  - `SelectItem` (Line 66-88): 单项。
    - 左侧显示 `Check` 图标作为选中指示器。
  - `SelectSeparator` (Line 90-100): 分隔线。

- **导出**:
  - `Select`, `SelectValue`, `SelectTrigger`, `SelectContent`, `SelectItem`, `SelectSeparator`。

### Separator (`android/frontend/src/components/ui/separator.tsx`)
分割线。

- **函数**:
  - `Separator` (Line 5-26):
    - 基于 `SeparatorPrimitive.Root`。
    - 支持 `orientation` ('horizontal' | 'vertical')。
    - 水平方向高度 1px，垂直方向宽度 1px。

### Switch (`android/frontend/src/components/ui/switch.tsx`)
开关控件。

- **接口**:
  - 新增 `size?: 'default' | 'sm' | 'lg'`（Line 7-9）。

- **函数**:
  - `Switch` (Line 5-32):
    - 基于 `SwitchPrimitives.Root`。
    - 根据 size 调整轨道和拇指的尺寸及位移距离。
    - 状态切换动画 duration 200ms。

### Tooltip (`android/frontend/src/components/ui/tooltip.tsx`)
工具提示。

- **函数/组件**:
  - `TooltipProvider` (Line 5): Provider 别名。
  - `Tooltip` (Line 7): Root 别名。
  - `TooltipTrigger` (Line 9): Trigger 别名。
  - `TooltipPortal` (Line 11): Portal 别名。
  - `TooltipContent` (Line 13-29): 内容层。
    - 默认 sideOffset 4。
    - 包含进入/退出动画类。

- **导出**:
  - `Tooltip`, `TooltipTrigger`, `TooltipContent`, `TooltipProvider`。

## Architecture

```text
┌─────────────────────────────────────────────────────┐
│                   frontend-ui-primitives            │
│                                                     │
│  ┌──────────────┐   ┌──────────────────────────┐    │
│  │  Layout      │   │  Form Controls           │    │
│  │              │   │                          │    │
│  │  Card        │   │  Input                   │    │
│  │  (Container) │   │  Select                  │    │
│  │              │   │  Switch                  │    │
│  │  Header      │   │  Label                   │    │
│  │  Title       │   │                          │    │
│  │  Content     │   │  Badge                   │    │
│  │  Icon        │   │                          │    │
│  └──────────────┘   └──────────────────────────┘    │
│                                                     │
│  ┌──────────────┐   ┌──────────────────────────┐    │
│  │  Feedback    │   │  Advanced / Interactive  │    │
│  │              │   │                          │    │
│  │  Dialog      │   │  Button                  │    │
│  │  (Modal)     │   │  (Mouse-follow, Loading) │    │
│  │  Overlay     │   │                          │    │
│  │  Content     │   │  AnimatedCard            │    │
│  │              │   │  (GSAP Tilt, RAF Throttle)│   │
│  │  Tooltip     │   │                          │    │
│  │  EmptyState  │   │  Utilities                 │    │
│  │  Separator   │   │  cn() from @/lib/utils   │    │
│  └──────────────┘   └──────────────────────────┘    │
└─────────────────────────────────────────────────────┘
         │                    ▲
         ▼                    │
┌──────────────────┐   ┌──────────────────┐
│ Radix UI Prims   │   │ Tailwind CSS     │
│ (State/A11y)     │   │ (Styling)        │
└──────────────────┘   └──────────────────┘
         ▲
         │
┌──────────────────┐
│ GSAP             │
│ (Animations)     │
└──────────────────┘
```

## Data Flow

1.  **样式生成**:
    - 组件通过 `cva` (Class Variance Authority) 定义 `variants` 和 `defaultVariants`。
    - 渲染时，调用 `cn(...)` 工具函数（来自 `@/lib/utils`）合并静态类、动态变体类和用户传入的 `className`。
    - 最终类名直接应用于 DOM 节点，Tailwind JIT 编译器匹配并生成 CSS。

2.  **交互数据流 (以 Button 为例)**:
    - 用户触发 `onMouseMove`。
    - 组件捕获 `clientX/Y` 并与 `lastPosRef` 比较，若位移 < 3px 则跳过（去抖）。
    - 更新 `lastPosRef`。
    - 检查 `rafRef.current`，若存在则 `cancelAnimationFrame`。
    - 注册新的 `requestAnimationFrame` 回调。
    - 在 RAF 回调中，读取 `getBoundingClientRect()` 计算相对百分比坐标。
    - 使用 `el.style.setProperty('--mouse-x', ...)` 更新 CSS 变量。
    - CSS 中的背景渐变或滤镜读取这些变量实现视觉效果。

3.  **动画数据流 (以 AnimatedCard 为例)**:
    - 初始化时，根据 `profile.enableTilt` 和 props 决定 `tiltEnabled`。
    - 若启用，创建 `gsap.quickTo` 引用 `rotateY` 和 `rotateX`。
    - `handleMouseMove` 中，计算鼠标偏移量 (-0.5 到 0.5)。
    - 通过 RAF 节流调用 `xQuick.current(x * 8)` 和 `yQuick.current(-y * 8)` 触发动画。
    - GSAP 自动处理插值和性能优化（force3D）。
    - 卸载时，清理 ResizeObserver、RAF 和 GSAP tween。

## Connections

- [[frontend-layout]]: `Card` 和 `AnimatedCard` 常被用作页面布局的基本块。
- [[frontend-forms]]: `Input`, `Select`, `Switch`, `Label` 是构建复杂表单的核心原语。
- [[frontend-dialogs]]: `Dialog` 模块可能依赖此模块中的 `Button` 和 `Badge` 来构建操作栏和状态提示。
- [[frontend-theming]]: 所有组件均使用语义化颜色变量（如 `bg-primary`, `text-muted-foreground`），依赖于全局主题配置。

## Known Issues

1.  **AnimatedCard RAF 泄漏修复历史**:
    - 文件: `android/frontend/src/components/ui/animated-card.tsx`
    - 行号: 46-52, 74-95, 97-112
    - 描述: 代码注释指出历史缺陷在于 tilt RAF 未取消，导致组件卸载后回调仍可能执行写已卸载节点。当前实现已在 cleanup 函数（Line 46-52）中添加了 `cancelAnimationFrame(tiltRafRef.current)` 和 `gsap.killTweensOf`，但在 `handleMouseMove` (Line 90) 和 `handleMouseLeave` (Line 104) 中也进行了预防性取消。需确保所有路径都正确清理了 `resetWillChangeTimerRef`。

2.  **Button RAF 泄漏修复历史**:
    - 文件: `android/frontend/src/components/ui/button.tsx`
    - 行号: 76-79
    - 描述: 代码注释指出历史缺陷在于 RAF 未取消。当前实现通过 `useEffect` 返回清理函数来 `cancelAnimationFrame`。

3.  **Input Error Border Visibility**:
    - 文件: `android/frontend/src/components/ui/input.tsx`
    - 行号: 28
    - 描述: 注释明确指出 "error 态基础类无 border 宽度，需同时给出边框宽度与颜色才可见"。因此，在 `cn` 中必须显式包含 `border border-destructive`，仅靠 `focus-visible:ring-destructive` 不足以在非聚焦状态下显示错误边框。

4.  **AnimatedCard Rect Cache**:
    - 文件: `android/frontend/src/components/ui/animated-card.tsx`
    - 行号: 84-87
    - 描述: 使用 `ResizeObserver` 清除 `rectCacheRef`。如果在 RAF 回调执行期间发生重排，可能导致使用过期的 rect 计算。目前策略是在 RAF 开始时检查缓存，若为空则重新获取。这在大多数情况下是安全的，但在高频 resize 场景下可能有微小抖动。

5.  **Dialog Close Button Accessibility**:
    - 文件: `android/frontend/src/components/ui/dialog.tsx`
    - 行号: 48-52
    - 描述: 关闭按钮使用了 `<span className="sr-only">` 配合 `i18next.t('common.close')`。这确保了屏幕阅读器能读到关闭意图，但视觉上隐藏。需确保 `i18next` 已正确初始化并提供该 key，否则 aria-label 将为空。