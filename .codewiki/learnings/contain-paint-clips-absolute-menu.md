---
title: "contain: paint 导致的绝对定位浮层裁剪与修复"
type: learning
source_files:
  - tauri-app/frontend/src/index.css
  - tauri-app/frontend/src/components/ui/animated-card.tsx
tags: [教训, 前端, css, contain, portal, 定位]
---

## Overview

在 Tauri 应用的 UI 组件中，`AnimatedCard` 组件用于提供带有入场动画、悬停交互及可选 3D 倾斜效果的卡片容器。由于 CSS `contain: paint` 属性的特性，卡片内部绝对定位（absolute）的子元素（如下拉菜单、浮动按钮）会被卡片边界裁剪，导致显示不全。

本文记录了该问题的根因分析、基于 `createPortal` 的解决方案，以及相关的 CSS 架构设计细节。

## Key Components

### `AnimatedCard` (src/components/ui/animated-card.tsx)

**接口定义**:
- `AnimatedCardConfig`: 配置对象，包含 `glowIntensity`, `hoverScale`, `stiffness`, `damping`, `mass` (第 6-12 行)。
- `AnimatedCardProps`: 继承自 `React.HTMLAttributes<HTMLDivElement>`，扩展字段包括：
    - `animationConfig?: AnimatedCardConfig` (第 15 行)
    - `noHover?: boolean` (第 16 行): 禁用悬停效果。
    - `noAnimation?: boolean` (第 17 行): 禁用所有动画。
    - `noEnterAnimation?: boolean` (第 18 行): 禁用入场动画。
    - `enableTilt?: boolean` (第 19 行): 启用 3D 倾斜效果。
    - `staggerIndex?: number` (第 20 行): 用于交错动画的时间索引。

**核心逻辑**:
- **引用处理**: 使用 `React.forwardRef` 转发 ref 到内部 `div` (第 31 行)。
- **状态管理**:
    - `cardRef`: 引用外层容器 DOM 节点 (第 37 行)。
    - `xQuick`, `yQuick`: GSAP `quickTo` 函数引用，用于高性能旋转更新 (第 38-39 行)。
    - `tiltRafRef`: 存储 `requestAnimationFrame` ID，用于节流倾斜计算 (第 40 行)。
    - `rectCacheRef`: 缓存 `getBoundingClientRect` 结果，避免强制重排 (第 41 行)。
    - `resetWillChangeTimerRef`: 存储 GSAP `delayedCall` 引用，用于重置 `will-change` 样式 (第 34 行)。
- **Effect Hook (第 43-63 行)**:
    - 初始化 GSAP `quickTo` 绑定到 `rotateY` 和 `rotateX`。
    - 使用 `ResizeObserver` 监听尺寸变化并清除矩形缓存 (第 49-50 行)。
    - Cleanup 阶段取消 RAF、kill GSAP tween、清理定时器，防止内存泄漏和对已卸载节点的写入 (第 51-62 行)。
- **事件处理**:
    - `handleMouseMove` (第 65-89 行): 计算鼠标相对于卡片中心的偏移量，通过 RAF 节流调用 GSAP 更新旋转角度。设置 `will-change: transform` 以提示浏览器优化合成层。
    - `handleMouseLeave` (第 91-103 行): 重置旋转到 0，并使用 GSAP 延迟调用在 0.4 秒后清除 `will-change`。
- **渲染逻辑**:
    - 如果 `noAnimation` 为真，返回简单的 `div` (第 112-118 行)。
    - 否则，返回嵌套结构：外层容器应用 `animated-card-interactive` 类（包含 `contain: layout style paint`），内层应用具体样式和阴影 (第 120-149 行)。

### CSS 样式 (src/index.css)

**关键类**:
- `.animated-card-interactive` (第 514-521 行):
    - 定义了卡片的交互过渡效果 (`transition`)。
    - **关键点**: 设置了 `contain: layout style paint` (第 520 行)。这是导致子元素被裁剪的根本原因。
    - 包含 `isolation: isolate` (第 519 行)，创建新的堆叠上下文。
- `.card-enter` (第 607-610 行): 定义卡片入场动画，支持 `--stagger-i` 变量实现交错延迟。
- `.dark .animated-card-interactive:hover` (第 533-535 行): 暗色模式下的悬停边框颜色调整。

## Architecture

```text
┌──────────────────────────────────────────────────────┐
│  App Window / Body                                   │
│                                                      │
│  ┌────────────────────────────────────────────────┐  │
│  │  AnimatedCard (Container)                      │  │
│  │  Class: animated-card-interactive              │  │
│  │  Contain: layout style paint                   │  │
│  │  Isolation: isolate                            │  │
│  │                                                │  │
│  │  ┌──────────────────────────────────────────┐  │  │
│  │  │  Inner Card Div                          │  │  │
│  │  │  Style: boxShadow, perspective           │  │  │
│  │  │                                          │  │  │
│  │  │  [Content]                               │  │  │
│  │  │  └─ Button / Menu Trigger                │  │  │
│  │  └──────────────────────────────────────────┘  │  │
│  │                                                │  │
│  │  ❌ Problem: Absolute children clipped here   │  │
│  └────────────────────────────────────────────────┘  │
│                                                      │
│  ┌────────────────────────────────────────────────┐  │
│  │  Portal Target (document.body)                 │  │
│  │  (Rendered via createPortal)                   │  │
│  │                                                │  │
│  │  ┌──────────────────────────────────────────┐  │  │
│  │  │  Floating Menu / Dropdown                │  │  │
│  │  │  Position: absolute/fixed                │  │  │
│  │  │  Visible & Unclipped                     │  │  │
│  │  └──────────────────────────────────────────┘  │  │
│  └────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────┘
```

## Data Flow

1.  **初始化**: `AnimatedCard` 挂载时，检查 `profile.enableTilt` 和 props 决定 `tiltEnabled`。
2.  **GSAP Setup**: 如果启用倾斜，创建 `gsap.quickTo` 实例绑定到 DOM 元素的 `rotateY` 和 `rotateX` 属性。
3.  **用户交互**:
    *   **鼠标移动**: `handleMouseMove` 触发，计算归一化坐标 `(x, y)`。
    *   **RAF Throttle**: 取消之前的 RAF，注册新的 RAF。在帧回调中，读取缓存的 `rect`，计算最终旋转值并调用 `xQuick`/`yQuick`。
    *   **鼠标离开**: `handleMouseLeave` 触发，立即重置旋转为 0，并安排一个 0.4 秒后的回调来清除 `will-change` 样式。
4.  **Cleanup**: 组件卸载时，取消所有 RAF、kill GSAP tweens 和 timers，确保无残留副作用。

## Known Issues

1.  **Containment Clipping**:
    *   **现象**: 当 `AnimatedCard` 内部包含绝对定位的元素（如下拉菜单、Tooltip）时，这些元素会被卡片边界裁剪，无法完整显示。
    *   **根因**: CSS 属性 `contain: paint` (在 `.animated-card-interactive` 中定义，index.css 第 520 行) 创建了内容 containment，限制了后代元素的绘制区域。此外，`isolation: isolate` 也创建了新的堆叠上下文，进一步限制了视觉溢出。
    *   **现状**: `AnimatedCard` 组件本身并未内置 Portal 机制。任何需要超出边界的浮层组件必须在父级或自身内部使用 `createPortal` 将内容渲染到 `document.body`。

2.  **Memory Leaks in Cleanup**:
    *   **历史缺陷**: 早期版本中，`gsap.delayedCall` 和 RAF 未在 cleanup 中正确释放，导致组件卸载后仍尝试修改 DOM。
    *   **修复**: 当前代码 (animated-card.tsx 第 51-62 行) 已显式 kill 所有 GSAP 对象并 cancel RAF。

3.  **Reduced Motion Support**:
    *   **检测**: 使用模块级 `matchMedia` 查询 `prefers-reduced-motion` (animated-card.tsx 第 27-29 行)。
    *   **行为**: 如果用户偏好减少运动，`showEntryAnim` 设为 false，且 GSAP tilt 效果被禁用 (通过 `tiltEnabled` 判断)。CSS 中也通过 `@media (prefers-reduced-motion: reduce)` 禁用了相关动画 (index.css 第 827-860 行)。

## Connections

[[tailwind-space-y-margin-specificity]]、[[radix-select-empty-string-value]]