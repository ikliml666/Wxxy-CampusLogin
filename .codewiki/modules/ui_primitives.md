---
title: ui_primitives
type: module
source_files:
  - tauri-app/frontend/src/components/ui/button.tsx
  - tauri-app/frontend/src/components/ui/button.test.tsx
  - tauri-app/frontend/src/components/ui/badge.tsx
  - tauri-app/frontend/src/components/ui/card.tsx
  - tauri-app/frontend/src/components/ui/dialog.tsx
  - tauri-app/frontend/src/components/ui/input.tsx
  - tauri-app/frontend/src/components/ui/label.tsx
  - tauri-app/frontend/src/components/ui/select.tsx
  - tauri-app/frontend/src/components/ui/separator.tsx
  - tauri-app/frontend/src/components/ui/setting-row.tsx
  - tauri-app/frontend/src/components/ui/switch.tsx
  - tauri-app/frontend/src/components/ui/tooltip.tsx
  - tauri-app/frontend/src/components/ui/animated-card.tsx
  - tauri-app/frontend/src/lib/utils.ts
  - tauri-app/frontend/src/hooks/useAnimationProfile.ts
tags: [ui, components, primitives, react, radix, tailwind]
---

## Overview

`ui_primitives` 是前端可复用的低层 UI 原子组件集合，位于 `tauri-app/frontend/src/components/ui/`。它不承载任何业务状态，只提供样式统一、可组合、可访问（a11y）的基础控件，被 `src/components/layout/`、`src/settings/`、`src/account/`、`src/monitor/`、`src/network/` 等上层模块直接消费。

设计上有三条贯穿原则：

1. **基于 Radix UI 原语 + Tailwind**：所有交互类控件（Dialog/Select/Switch/Label/Tooltip/Separator）都是对 `@radix-ui/*` 的薄封装，只加 className 与少量 props；纯展示类（Badge/Card/Input/SettingRow/AnimatedCard）用原生元素 + `cn()` 拼样式。
2. **统一样式语汇**：输入框与下拉统一用 `bg-card + border border-input`（见 `input.tsx:22-25` 与 `select.tsx:17-20` 的注释），卡片统一走 `.bg-card` 而非写死颜色（`animated-card.tsx:103-105`），避免浅/深色下控件边界糊成一片。
3. **刻意收敛动画与降级**：`Button`、`AnimatedCard` 的鼠标跟踪用 RAF 节流 + 位置去抖，并在卸载时 `cancelAnimationFrame`；`AnimatedCard` 还读取 `prefers-reduced-motion` 与 `useAnimationProfile()` 决定是否开启倾斜/入场动画。

模块内 **没有** 任何禁用/注释掉的整组件；已知的「历史缺陷」都集中在 RAF 清理逻辑上（详见 Known Issues）。

## Key Components

### Button — `button.tsx`

- `buttonVariants`（6–34）：用 `cva` 生成的样式工厂。`variant` 含 `default`/`destructive`/`outline`/`secondary`/`ghost`/`link`/`glass`/`soft`（11–18）；`size` 含 `default`/`sm`/`lg`/`icon`/`icon-sm`/`icon-lg`（21–26）。默认 `variant='default'`、`size='default'`。
- `ButtonProps`（36–41）：继承 `React.ButtonHTMLAttributes<HTMLButtonElement>` + `VariantProps<typeof buttonVariants>`，额外含 `asChild?: boolean`、`isLoading?: boolean`。
- `Button`（43–170）：`React.forwardRef`。`asChild=true` 时用 `@radix-ui/react-slot` 的 `Slot` 把 props 透传到一个子元素上（45、47–84）；否则渲染 `<button>`。
  - 加载态 `isLoading`：插入一个 `animate-spin` 的 SVG（57–76、142–161），并把 `disabled` 置为 `props.disabled || isLoading`（52、109）。
  - 鼠标 spotlight：非 `asChild` 分支重写 `onMouseMove`（110–137）。先按 3px 做位置去抖（116），再同步捕获 `e.currentTarget` 节点（124），用 `requestAnimationFrame` 节流（128–136），把光标相对卡片的百分比写入 CSS 变量 `--mouse-x`/`--mouse-y`（133–134）。同时剥离一批 `onDrag*`/`onAnimation*` 事件名避免与 Radix Slot 冲突（86–91）。
  - 卸载清理：`useEffect`（99–103）在 cleanup 时 `cancelAnimationFrame(rafRef.current)`，修复「RAF 未取消导致卸载后仍写节点」的历史缺陷。
- 导出（173）：`Button`、`buttonVariants`。
- 对应测试 `button.test.tsx`（1–25）验证 RAF 回调内不访问被 React 置空的 `currentTarget`，且 `--mouse-x/--mouse-y` 被正确设置（12–24）。

### Badge — `badge.tsx`

- `badgeVariants`（5–30）：`cva` 工厂。`variant`：`default`/`secondary`/`destructive`/`outline`/`success`/`warning`/`info`/`ghost`（10–17）；`size`：`default`/`sm`/`lg`（20–22）。
- `BadgeProps`（32–34）：`React.HTMLAttributes<HTMLDivElement>` + `VariantProps`。
- `Badge`（36–40）：纯函数组件，渲染 `<div>` 并套 `badgeVariants` + `className`。导出 `Badge`、`badgeVariants`（42）。

### Card — `card.tsx`

5 个 `forwardRef` 子组件（无 props 扩展，均为 `React.HTMLAttributes<HTMLDivElement>/<HTMLHeadingElement>`）：

- `CardHeader`（4–14）：`flex flex-col space-y-1.5 p-5` 的 `<div>`。
- `CardTitle`（16–26）：`<h3>`，`font-semibold leading-none tracking-tight text-base`。
- `CardDescription`（28–38）：`<p>`，`text-sm text-muted-foreground`。
- `CardContent`（40–46）：`<div>`，`p-5 pt-0`。
- `CardIcon`（50–60）：`w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center shrink-0`，统一卡片头图标容器，替代历史手写 `w-10 h-10` 时 `rounded-full`/`rounded-xl` 混用（48–49）。
- 导出（62）：上述 5 个。

### Dialog — `dialog.tsx`

对 `@radix-ui/react-dialog` 的封装：

- `Dialog`（7）= `DialogPrimitive.Root`；`DialogPortal`（9）= `DialogPrimitive.Portal`。
- `DialogOverlay`（11–24）：`fixed inset-0 z-50 bg-black/60 dialog-overlay-enter`。
- `DialogContent`（26–53）：`forwardRef`，额外 prop `showClose?: boolean`（默认 `true`）。内部用 `DialogPortal` 渲染 `DialogOverlay` 与一个居中定位包装层（32–51），关闭按钮 `DialogPrimitive.Close` 用 lucide `X` 图标（44–45），其 `sr-only` 文案来自 `i18next.t('common.close')`（46）。
- `DialogHeader`（56–68）：无 ref 普通组件，`flex flex-col space-y-1.5 text-center sm:text-left`。
- `DialogTitle`（70–83）/`DialogDescription`（85–95）：分别转发到 `DialogPrimitive.Title`/`Description`。
- 导出（97–103）：`Dialog`、`DialogContent`、`DialogHeader`、`DialogTitle`、`DialogDescription`。

### Input — `input.tsx`

- `InputProps`（4–8）：继承 `React.InputHTMLAttributes<HTMLInputElement>`，额外含 `icon?: React.ReactNode`、`error?: string`。
- `Input`（10–40）：`forwardRef`。渲染一个 `relative` 包裹层：可选 `icon` 绝对定位在 `left-3`（14–18）；`<input>` 用 `bg-card border border-input`（22–25），`icon` 时加 `pl-10`，`error` 时边框与 focus ring 切到 `destructive`（27–28）；`error` 文案以 `text-xs text-destructive` 显示在下方（34–36）。

### Label — `label.tsx`

- `labelVariants`（6–8）：`cva`，`text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70`。
- `Label`（10–24）：`forwardRef` 到 `@radix-ui/react-label` 的 `LabelPrimitive.Root`，额外 prop `required?: boolean`；`required` 时在内容后追加红色 `*`（`*`，21）。导出 `Label`（26）。

### Select — `select.tsx`

对 `@radix-ui/react-select` 的封装：

- `Select`（6）= `SelectPrimitive.Root`；`SelectValue`（8）= `SelectPrimitive.Value`。
- `SelectTrigger`（10–37）：`forwardRef`，额外 `icon?: React.ReactNode`。样式统一为 `bg-card border border-input`（20），`icon` 时 `pl-10` 并绝对定位图标（21、26–30），右侧 `ChevronDown` 箭头（32–34）。
- `SelectContent`（39–67）：`forwardRef`，`position` 默认 `'popper'`（42），经 `SelectPrimitive.Portal` 渲染，带 `--radix-select-trigger-height/width` 匹配（55–61）。
- `SelectItem`（69–91）：`forwardRef`，高亮用 `data-[highlighted]` 而非 `focus:`（78，注释说明 Radix Select Item 不把 DOM focus 给当前项）。
- `SelectSeparator`（93–103）：`forwardRef`，`-mx-1 my-1 h-px bg-muted`。
- 导出（105–112）：`Select`、`SelectValue`、`SelectTrigger`、`SelectContent`、`SelectItem`、`SelectSeparator`。

### Separator — `separator.tsx`

- `Separator`（5–26）：`forwardRef` 到 `@radix-ui/react-separator` 的 `SeparatorPrimitive.Root`。默认 `orientation='horizontal'`、`decorative=true`（10）；横向 `h-[1px] w-full`、纵向 `h-full w-[1px]`（19）。导出 `Separator`（28）。

### SettingRow — `setting-row.tsx`

- `SettingRowProps`（5–14）：`React.HTMLAttributes<HTMLDivElement>` + `label`、`description?`、`icon?`、`htmlFor?`。
- `SettingRow`（21–34）：`forwardRef` 到 `HTMLDivElement`。左 `Label`（来自本模块 `label.tsx`）+ 可选 `icon` + `description`，右侧放 `children` 控件；窄屏用 `min-w-0` + `truncate` 截断（24–31）。这是统一「标签左、控件右」参数行的组合件（16–20）。导出 `SettingRow`（35）。

### Switch — `switch.tsx`

- `Switch`（5–32）：`forwardRef` 到 `@radix-ui/react-switch` 的 `SwitchPrimitives.Root`，额外 `size?: 'default' | 'sm' | 'lg'`（默认 `default`）。尺寸决定 `h/w` 与 thumb 的 `translate-x`（14–27）。导出 `Switch`（34）。

### Tooltip — `tooltip.tsx`

对 `@radix-ui/react-tooltip` 的封装：

- `TooltipProvider`（5）= `TooltipPrimitive.Provider`；`Tooltip`（7）= `TooltipPrimitive.Root`；`TooltipTrigger`（9）= `TooltipPrimitive.Trigger`；`TooltipPortal`（11）= `TooltipPrimitive.Portal`。
- `TooltipContent`（13–29）：`forwardRef`，默认 `sideOffset=4`（16），经 `TooltipPortal` 渲染，`bg-popover` 配入场动画。
- 导出（31）：`Tooltip`、`TooltipTrigger`、`TooltipContent`、`TooltipProvider`。

### AnimatedCard — `animated-card.tsx`

- `AnimatedCardConfig`（6–12）：`glowIntensity?`/`hoverScale?`/`stiffness?`/`damping?`/`mass?`。
- `AnimatedCardProps`（14–21）：`React.HTMLAttributes<HTMLDivElement>` + `animationConfig?`/`noHover?`/`noAnimation?`/`noEnterAnimation?`/`enableTilt?`/`staggerIndex?`。
- 模块级缓存：`reducedMotionQuery = window.matchMedia('(prefers-reduced-motion: reduce)')`（25–27），避免每次渲染重建 MediaQueryList。
- `AnimatedCard`（29–151）：`React.memo(React.forwardRef(...))`。
  - `tiltEnabled`（34）：由 `enableTilt ?? profile.enableTilt` 决定，且 `!noHover && !noAnimation`。
  - Tilt 初始化 `useEffect`（41–61）：用 `gsap.quickTo` 注册 `rotateY`/`rotateX`（44–45），`ResizeObserver` 失效 rect 缓存（47–48）；cleanup 时 `ro.disconnect()`、`cancelAnimationFrame`、`gsap.delayedCall`/`killTweensOf` 清理（49–60）。
  - `handleMouseMove`（63–87）：RAF 节流 + 同步捕获 `e.currentTarget`（67），用缓存 `rect` 避免强制同步布局（77–81），调用 `xQuick/yQuick`（84–85）。
  - `handleMouseLeave`（89–101）：归位并 `gsap.delayedCall(0.4)` 复位 `willChange`（96）。
  - 渲染：`noAnimation` 时直接返回静态 `<div>`（113–119）；否则外层 `animated-card-interactive` + 可选 `card-enter`（122–133），内层 `bg-card text-card-foreground rounded-2xl`（106–109、140），倾斜时设 `perspective`/`transformStyle: preserve-3d`（127–129、141–143）。从 `useAnimationProfile()` 取 `profile`（31）。
- 导出 `AnimatedCard`（152），`displayName='AnimatedCard'`（153）。

### 共享依赖

- `cn(...)`（`lib/utils.ts:4-6`）：`clsx` + `tailwind-merge`，所有组件拼 className 的统一入口。
- `useAnimationProfile()`（`hooks/useAnimationProfile.ts:51-57`）：仅 `AnimatedCard` 使用，提供 `enableTilt`、`refreshRate`、easing 等动画画像；v2.4.0 起恒为最高档（31–49）。

## Architecture

UI 原子组件分两层：薄封装 Radix 原语（a11y/行为由 Radix 负责）与纯展示/组合组件（样式靠 `cn` + Tailwind）。所有组件都通过一个共享入口 `cn()` 拼样式，动画类组件额外依赖 `useAnimationProfile()` 与 GSAP。

```text
                上层模块 (settings/account/monitor/network/layout)
                                │ 消费
                                ▼
 ┌──────────────────────── ui_primitives (src/components/ui/) ───────────────────────┐
 │                                                                                   │
 │  薄封装 Radix 原语          纯展示 / 组合件           动画组件                       │
 │  ┌──────────────────┐      ┌──────────────────┐      ┌──────────────────────────┐ │
 │  │ Dialog           │      │ Badge            │      │ AnimatedCard              │ │
 │  │ Select           │      │ Card*            │      │  ├─ GSAP quickTo/tilt    │ │
 │  │ Switch           │      │ Input            │      │  └─ useAnimationProfile ─┼─┐
 │  │ Label            │      │ SettingRow       │      └──────────────────────────┘ │ │
 │  │ Tooltip          │      │ (组合 Label+...) │                                     │ │
 │  │ Separator        │      └──────────────────┘                                     │ │
 │  └──────────────────┘                                                              │ │
 │         │                          │                          │                   │ │
 │         └───────── 统一 className 拼装 ─────────┐             │                   │ │
 │                                              ▼             │                   │ │
 │                                    ┌──────────────┐   ┌─────┴──────────┐        │ │
 │                                    │ cn()         │   │ useQualityStore │◄───────┘│
 │                                    │ (lib/utils)  │   │ (refreshRate)  │        │
 │                                    └──────────────┘   └────────────────┘        │
 └─────────────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **渲染期**：上层把 props（如 `variant`、`size`、`icon`、`error`、`isLoading`）传给组件；组件用 `cn(buttonVariants({variant,size,className}), className)`（`button.tsx:50`、`button.tsx:107`）或 `badgeVariants`/`cardVariants` 拼出最终 className，再交给 Radix 原语或原生元素。
2. **动画期（Button spotlight）**：用户移动鼠标 → `onMouseMove`（`button.tsx:110`）先 3px 去抖（116），再 `requestAnimationFrame`（129）把光标位置百分比写入 `style['--mouse-x'/'--mouse-y']`（133–134）；下次渲染前 RAF 被取消复位（128、135）。
3. **动画期（AnimatedCard tilt）**：`AnimatedCard` 挂载后 `useEffect`（41–61）用 `gsap.quickTo` 注册旋转 tween；`handleMouseMove`（63–87）RAF 节流 + 缓存 `rect` 计算倾斜量并写入 `cardRef` 的 transform；`handleMouseLeave`（89–101）归位并延迟复位 `willChange`。降级开关：`noAnimation` → 直接静态渲染（113–119）；`reducedMotionQuery.matches` 或 `noEnterAnimation` → 跳过入场动画（111）；`tiltEnabled` 由 `profile.enableTilt` 决定（34）。
4. **卸载清理**：`Button` 在 `useEffect` cleanup 取消未完成 RAF（`button.tsx:99-103`）；`AnimatedCard` 在 cleanup 断开 `ResizeObserver`、取消 RAF、kill GSAP tween/delayedCall（49–60）。

## Connections

- 上层消费方：`src/components/layout/`（DockNav/RightPanel/TitleBar）、`src/settings/`、`src/account/`、`src/monitor/`、`src/network/` 直接 import 本模块组件。
- 样式体系：所有 `bg-card`/`border-input`/`text-muted-foreground` 等 token 由 `src/index.css` 与 `tailwind.config.js` 定义（组件只消费，不定义）。
- 工具函数：[[lib-utils]]（`cn`、`safeStorage`、`extractErrorMessage`）—— `cn` 被本模块每个文件使用。
- 动画能力：[[hooks-useAnimationProfile]]（`useAnimationProfile`）、`src/lib/easing-config.ts`（`getEasingConfig`）、`src/hooks/useQualityStore.ts`（`refreshRate`）—— 仅 `AnimatedCard` 依赖链路长。
- 第三方原语：[[radix-ui]]（`@radix-ui/react-dialog|react-select|react-switch|react-label|react-tooltip|react-separator|react-slot`）。
- 动画引擎：GSAP（`gsap.quickTo`/`killTweensOf`/`delayedCall`），仅 `AnimatedCard` 使用。
- 国际化：Dialog 关闭按钮文案走 `i18next.t('common.close')`（`dialog.tsx:46`），关联 [[i18n]] 模块。

## Known Issues

1. **Button RAF 未取消（已修复）**：`button.tsx:97-103` 注释说明历史缺陷——RAF 回调在按钮卸载后仍执行、写已卸载节点。`useEffect` cleanup 中已 `cancelAnimationFrame(rafRef.current)`（99–103）。对应回归测试见 `button.test.tsx:12-24`，验证 RAF 回调内不读被 React 置空的 `e.currentTarget`（React 19 的 `executeDispatch` finally 把 `event.currentTarget` 置 null，见测试 9–11 行注释），否则会抛 `TypeError` 并刷屏 console。

2. **AnimatedCard tilt RAF/GSAP 未清理（已修复）**：`animated-card.tsx:41-42` 与 `93-95` 注释记录历史缺陷——tilt RAF 与 `willChange` 复位 `delayedCall` 不保存引用，卸载后仍写节点、连续 enter/leave 堆积回调。当前在 `useEffect` cleanup（49–60）统一 disconnect `ResizeObserver`、cancel RAF、`kill` delayedCall 与 `killTweensOf(el, 'rotateX'|'rotateY')`；`handleMouseLeave`（95）改为先 kill 旧 timer 再注册。

3. **SelectItem 高亮挂错状态（已修复）**：`select.tsx:76-78` 注释——Radix Select Item 用 `data-highlighted` 标记当前项、不把 DOM focus 给该项，因此高亮必须挂 `data-[highlighted]`，`focus:` 前缀永不命中。2026-10-20 修复悬停无底色。

4. **Input/Select 底色不一致历史问题（已整改）**：`input.tsx:22-25` 与 `select.tsx:17-20` 注释——早期 Select 用 `bg-muted/40` 半透明灰，与弹窗底 `--background` 几乎同明度，而 Input 用纯白 `bg-card`，读作两套控件。现统一改为 `bg-card + border border-input`；Input 还改用 `--card` 填底 + 加深的 `--input` 边框，使边界一眼可辨（深色同样成立）。

5. **CardIcon 形状历史混用（已整改）**：`card.tsx:50-51` 注释——各面板曾手写 `w-10 h-10` 图标容器时 `rounded-full` 与 `rounded-xl` 混用，现由 `CardIcon`（`card.tsx:50-60`）统一为 `rounded-xl bg-primary/10`。

6. **AnimatedCard 写死底色绕过卡片材质（已整改）**：`animated-card.tsx:103-105` 注释——曾写死 `bg-white / dark:bg-[#14161b]`，使最大几张卡绕过 `index.css` 的分层阴影 + 上缘高光材质，暗色下无论选哪档都是死板实底。现卡面统一走 `.bg-card`（`animated-card.tsx:106-109`）。

7. **无整体禁用/注释的组件**：本模块所有 12 个组件均处于启用状态，无 `disabled` 或被注释掉的功能块。

注：以上「已修复/已整改」均为代码注释中明确记载的历史缺陷，其防御代码（RAF 取消、状态修正、底色统一）当前已就位。
