---
title: "AnimatedCard 组件：3D 倾斜、入场动画与性能优化"
type: learning
source_files:
  - tauri-app/frontend/src/index.css
  - tauri-app/frontend/src/components/ui/animated-card.tsx
tags: [教训, 前端, react, gsap, css, contain, 动画, 性能]
---

## Overview

`AnimatedCard` 是 Tauri 应用前端的核心 UI 容器组件，提供带有物理反馈的 3D 倾斜效果（Tilt）、交错入场动画以及高性能的悬停交互。该组件深度集成了 GSAP 库以实现流畅的基于物理的动画，并通过 `requestAnimationFrame` (RAF) 节流确保渲染性能。

本文档记录了组件的内部实现机制、CSS 样式架构、性能优化策略以及已知的设计约束。特别需要注意的是，由于 CSS `contain` 属性的使用，卡片内部的绝对定位元素会被裁剪，浮层内容需通过 Portal 机制渲染。

## Key Components

### `AnimatedCard` (src/components/ui/animated-card.tsx)

**接口定义**:
- **`AnimatedCardConfig`** (第 6-12 行): 配置对象接口。
    - `glowIntensity?: number`: 发光强度。
    - `hoverScale?: number`: 悬停缩放比例。
    - `stiffness?: number`: 弹簧刚度。
    - `damping?: number`: 阻尼系数。
    - `mass?: number`: 质量。
    *注：当前代码中这些字段已定义但未被直接用于核心逻辑，主要依赖 `useAnimationProfile` 和硬编码参数。*
- **`AnimatedCardProps`** (第 14-21 行): 继承自 `React.HTMLAttributes<HTMLDivElement>`。
    - `animationConfig?: AnimatedCardConfig` (第 15 行): 动画配置。
    - `noHover?: boolean` (第 16 行): 禁用悬停交互（包括倾斜和视觉反馈）。
    - `noAnimation?: boolean` (第 17 行): 禁用所有动画效果（包括倾斜和入场）。
    - `noEnterAnimation?: boolean` (第 18 行): 仅禁用入场动画，保留悬停交互。
    - `enableTilt?: boolean` (第 19 行): 强制启用或禁用 3D 倾斜效果。
    - `staggerIndex?: number` (第 20 行): 用于计算交错动画延迟的索引。

**内部状态与引用**:
- **`profile`** (第 33 行): 通过 `useAnimationProfile()` 获取全局动画偏好设置（如是否启用倾斜）。
- **`resetWillChangeTimerRef`** (第 34 行): `React.RefObject<gsap.core.Tween | null>`。存储 GSAP `delayedCall` 的引用，用于在鼠标离开后重置 `will-change` 样式。
- **`cardRef`** (第 37 行): `React.RefObject<HTMLDivElement | null>`。指向内部实际承载内容的 DOM 节点，用于 GSAP 操作。
- **`xQuick` / `yQuick`** (第 38-39 行): `React.RefObject<gsap.QuickToFunc | null>`。GSAP 的 `quickTo` 函数实例，分别绑定到 `rotateY` 和 `rotateX`，用于高性能实时更新旋转角度。
- **`tiltRafRef`** (第 40 行): `React.RefObject<number>`。存储 `requestAnimationFrame` 的 ID，用于取消上一帧的倾斜计算。
- **`rectCacheRef`** (第 41 行): `React.RefObject<DOMRect | null>`。缓存 `getBoundingClientRect` 的结果，避免在 RAF 回调中触发强制重排（Layout Thrashing）。

**核心逻辑**:
- **初始化 Effect (第 43-63 行)**:
    - 检查 `tiltEnabled`（由 `enableTilt` prop 或 `profile.enableTilt` 决定，且受 `noHover`/`noAnimation` 限制）。
    - 创建 `gsap.quickTo` 实例，绑定到 `cardRef.current` 的 `rotateY` 和 `rotateX` 属性，设置持续时间 0.35s 和 `expo.out` 缓动。
    - 注册 `ResizeObserver` (第 49-50 行)：当卡片尺寸变化时，清除 `rectCacheRef`，确保下一帧重新计算准确的边界矩形。
    - **Cleanup (第 51-62 行)**:
        - 断开 `ResizeObserver`。
        - 取消倾斜 RAF (`cancelAnimationFrame`)。
        - Kill `resetWillChangeTimerRef` 对应的 GSAP tween，防止卸载后执行。
        - `gsap.killTweensOf(el, 'rotateY/X')` 终止所有进行中的旋转动画。
        - 清空 `xQuick` 和 `yQuick` 引用。
- **事件处理**:
    - **`handleMouseMove` (第 65-89 行)**:
        - 同步捕获 `currentTarget` 和鼠标坐标 (`clientX`, `clientY`)，因为 React 合成事件在异步回调中可能失效。
        - 取消之前的 RAF，注册新的 RAF 回调。
        - 在 RAF 回调中：
            - 设置 `el.style.willChange = 'transform'` 以提示浏览器提升图层。
            - 从 `rectCacheRef` 读取边界矩形，若为空则调用 `getBoundingClientRect()` 并缓存。
            - 计算归一化偏移量 `(x, y)`，范围约为 -0.5 到 0.5。
            - 调用 `xQuick(x * 8)` 和 `yQuick(-y * 8)` 更新旋转角度（最大倾斜 8 度）。
    - **`handleMouseLeave` (第 91-103 行)**:
        - 立即调用 `xQuick(0)` 和 `yQuick(0)` 重置旋转。
        - 如果存在旧的 `resetWillChangeTimerRef`，先 kill 掉。
        - 注册新的 `gsap.delayedCall(0.4, ...)`，在 0.4 秒后将 `el.style.willChange` 重置为空字符串，释放浏览器优化资源。
- **渲染逻辑**:
    - **`cardClassName` (第 105-108 行)**: 组合类名，默认包含 `bg-white text-card-foreground rounded-2xl dark:bg-[#14161b]`。
    - **`showEntryAnim` (第 110 行)**: 判断是否显示入场动画。条件：未禁用动画、未禁用入场动画、且用户未开启“减少运动”偏好 (`prefers-reduced-motion: reduce`)。
    - **无动画模式 (第 112-118 行)**: 返回简单的 `div`，应用静态阴影 `REST_SHADOW`。
    - **标准模式 (第 120-149 行)**:
        - 外层 `div`: 应用 `animated-card-interactive` 类（来自 CSS），添加 `card-enter` 类（如果显示入场动画）。设置 CSS 变量 `--stagger-i` 用于交错延迟。设置 `perspective: 800`（如果启用倾斜）以建立 3D 空间。绑定 `onMouseMove` 和 `onMouseLeave`。
        - 内层 `div` (ref: `cardRef`): 应用 `cardClassName`，设置静态阴影 `REST_SHADOW`。如果启用倾斜，设置 `transformStyle: 'preserve-3d'`。

### CSS 样式 (src/index.css)

**关键类**:
- **`.animated-card-interactive`** (第 536-557 行):
    - **性能隔离**: 设置 `contain: layout style paint` (第 542 行)。这告诉浏览器卡片的布局、样式和绘制不依赖于外部，也不影响外部。
    - **堆叠上下文**: 设置 `isolation: isolate` (第 541 行)，创建新的堆叠上下文。
    - **交互过渡**: 定义 `border-color` 和 `transform` 的过渡效果 (第 537 行)。
    - **悬停效果**: 悬停时边框颜色变淡 (`rgba(0,0,0,0.06)`) 并轻微上移 (`translateY(-2px)`) (第 545-548 行)。
    - **焦点可见**: 聚焦时显示 `ring` 光晕 (第 550-553 行)。
    - **暗色模式**: 悬停边框颜色调整为白色低透明度 (第 555-557 行)。
- **`.card-enter`** (第 629-632 行):
    - 定义入场动画 `cardEnter`，结合 CSS 变量 `--stagger-i` 实现交错延迟 (`calc(var(--stagger-i, 0) * 0.05s)`)。
- **`.dark .animated-card-interactive:hover`** (第 555-557 行): 暗色主题下的悬停边框颜色调整。
- **`REST_SHADOW`** (JS 常量, animated-card.tsx 第 23 行): `'0 1px 3px rgba(0,0,0,0.03), 0 1px 2px rgba(0,0,0,0.02)'`。轻量级阴影，用于卡片默认状态。

## Architecture

```text
┌──────────────────────────────────────────────────────┐
│  Browser Compositor Layer                            │
│                                                      │
│  ┌────────────────────────────────────────────────┐  │
│  │  Outer Container (div)                         │  │
│  │  Class: animated-card-interactive              │  │
│  │  Contain: layout style paint                   │  │
│  │  Isolation: isolate                            │  │
│  │  Perspective: 800 (if tilt enabled)            │  │
│  │                                                │  │
│  │  ┌──────────────────────────────────────────┐  │  │
│  │  │  Inner Card (div, cardRef)               │  │  │
│  │  │  Style: boxShadow, transform-style       │  │  │
│  │  │           preserve-3d                    │  │  │
│  │  │                                          │  │  │
│  │  │  [Children Content]                      │  │  │
│  │  │  (Clipped by parent's containment)       │  │  │
│  │  └──────────────────────────────────────────┘  │  │
│  └────────────────────────────────────────────────┘  │
│                                                      │
│  Note: Absolute children are clipped due to          │
│  contain: paint & isolation: isolate                 │
└──────────────────────────────────────────────────────┘
```

## Data Flow

1.  **初始化**:
    - 组件挂载，`useAnimationProfile` 获取全局配置。
    - 根据 `enableTilt`、`noHover`、`noAnimation` 计算 `tiltEnabled`。
    - 如果 `tiltEnabled` 为真，初始化 GSAP `quickTo` 实例，并注册 `ResizeObserver`。
2.  **渲染**:
    - 根据 `noAnimation` 决定渲染简单结构还是完整交互结构。
    - 如果显示入场动画，应用 `.card-enter` 类，CSS 动画根据 `--stagger-i` 延迟启动。
3.  **用户交互 (Tilt)**:
    - **鼠标进入**: 无特殊处理，等待移动事件。
    - **鼠标移动**: `handleMouseMove` 触发，取消旧 RAF，注册新 RAF。在 RAF 中计算偏移量，调用 GSAP `quickTo` 更新 `rotateX/Y`，并设置 `will-change: transform`。
    - **鼠标离开**: `handleMouseLeave` 触发，立即重置旋转为 0，安排 0.4 秒后清除 `will-change` 样式。
4.  **清理**:
    - 组件卸载时，Effect 的 cleanup 函数执行：取消 RAF、Kill GSAP tweens 和 timers、断开 ResizeObserver、清空引用。

## Known Issues

1.  **Containment Clipping**:
    - **现象**: `AnimatedCard` 内部的绝对定位元素（如下拉菜单、Tooltip）会被卡片边界裁剪，无法显示在卡片外部。
    - **根因**: CSS 类 `.animated-card-interactive` (index.css 第 542 行) 设置了 `contain: layout style paint`，同时 `isolation: isolate` (第 541 行) 创建了新的堆叠上下文。这限制了后代元素的溢出区域。
    - **解决方案**: 任何需要超出卡片边界的浮层组件必须使用 `createPortal` 将内容渲染到 `document.body` 或其他不受限制的容器中。
2.  **Memory Leaks in Cleanup**:
    - **历史缺陷**: 早期版本中，`gsap.delayedCall` 和 RAF 未在 cleanup 中正确释放，导致组件卸载后仍尝试修改 DOM。
    - **修复**: 当前代码 (animated-card.tsx 第 51-62 行) 已显式 kill 所有 GSAP 对象并 cancel RAF。
3.  **Reduced Motion Support**:
    - **检测**: 使用模块级 `matchMedia` 查询 `prefers-reduced-motion` (animated-card.tsx 第 27-29 行)。
    - **行为**: 如果用户偏好减少运动，`showEntryAnim` 设为 false，且 GSAP tilt 效果被禁用 (通过 `tiltEnabled` 判断)。CSS 中也通过 `@media (prefers-reduced-motion: reduce)` 禁用了相关动画 (index.css 第 849-882 行)。

## Connections

[[tailwind-space-y-margin-specificity]]、[[radix-select-empty-string-value]]