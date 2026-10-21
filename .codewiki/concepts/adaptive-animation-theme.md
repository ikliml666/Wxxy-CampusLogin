---
type: concept
title: adaptive-animation-theme
source_files:
  - android/frontend/src/hooks/useReducedMotion.ts
  - android/frontend/src/hooks/useThemeStore.ts
  - android/frontend/src/hooks/useAdaptiveFramePace.ts
  - android/frontend/src/hooks/useAnimationProfile.ts
  - android/frontend/src/hooks/usePageIdle.ts
  - android/frontend/src/lib/animations.ts
  - android/frontend/src/lib/easing-config.ts
tags:
  - animation
  - theme
  - performance
  - accessibility
  - zustand
---

## Overview

`adaptive-animation-theme` 并非单一组件，而是 Android 前端应用中一套**自适应动画与主题系统**。它整合了视觉主题（Theme）、无障碍动效控制（Reduced Motion）、性能自适应帧率（Adaptive Frame Pace）以及物理动画参数生成（Easing/Motion Variants）。

该系统的核心目标是：在提供丰富视觉反馈的同时，严格保障低端设备的渲染性能，并尊重用户的无障碍偏好。它通过 Zustand Store 集中管理状态，利用 CSS 变量驱动全局视觉切换，并结合 `framer-motion` 与自定义弹簧物理引擎实现高性能的 UI 交互。

## Involved Modules

1.  **`android-frontend-hooks-and-stores`**: 核心状态管理与性能控制层。负责定义主题、动效开关、帧率策略及页面空闲状态的 Hook。
2.  **`android-frontend-lib-and-i18n`**: 基础工具与动画变体库。提供颜色转换、贝塞尔曲线配置、日志/面板动画变体生成及底层物理仿真内核。
3.  **`android-frontend-app-shell`**: 视觉应用层。消费主题与动效配置，执行实际的布局渲染、玻璃拟态效果及转场动画。
4.  **`android-frontend-monitor`**: 监控可视化层。依赖上述系统实现网络质量胶囊、延迟时间轴等数据可视化的平滑过渡。
5.  **`android-frontend-entry-config`**: 基础设施层。通过全局 CSS 和 HTML 脚本确立首帧主题基线。

## Flow

数据与控制流在模块间呈分层流动：

1.  **初始化与状态确立 (`Entry Config` -> `Hooks & Stores`)**:
    *   `index.html` 中的内联脚本根据本地存储或系统偏好设置初始 `data-light`/`theme-*` 属性。
    *   `main.tsx` 启动后，`useThemeStore.initTheme` 读取配置并同步 Zustand Store。
    *   `useAppInit` 编排加载流程，同时激活 `useAdaptiveFramePace` 和 `usePageIdle` 以建立性能监控上下文。

2.  **运行时状态响应 (`Hooks & Stores` -> `UI Components`)**:
    *   **主题切换**: 当用户更改主题时，`useThemeStore` 更新状态，触发全局 CSS 变量重绘。`AppShell` 中的玻璃拟态组件实时响应 `--glass-blur` 等变量变化。
    *   **动效适配**:
        *   `useReducedMotion` 监听系统级“减少动效”设置。若启用，所有基于 `framer-motion` 的动画将降级为无动画或极短过渡。
        *   `useAnimationProfile` 在安卓端强制返回最高档配置，但在桌面端可能根据硬件调整。
        *   `useAdaptiveFramePace` 结合 `usePageIdle` 动态调整渲染循环。当页面不可见或 CPU 负载高时，降低动画刷新率以节省电量。

3.  **动画执行与物理计算 (`Lib & i18n` <-> `UI Components`)**:
    *   UI 组件（如 `NetworkQualityCapsule`, `LatencyTimeline`）调用 `animations.ts` 中的工厂函数（如 `createPanelAppleVariants`）。
    *   `getEasingConfig` 根据当前设备刷新率动态生成贝塞尔曲线，确保动画速度在不同屏幕下感知一致。
    *   对于高频交互（如 Dock 栏透镜效果），`dock-lens.ts` 直接操作 DOM Style 或使用 `Motion` 类进行半隐式欧拉积分，绕过 React State 更新开销，实现 60fps+ 流畅度。

4.  **性能反馈闭环 (`Hooks & Stores`)**:
    *   `useAdaptiveFramePace` 持续监测渲染性能，若检测到掉帧，自动降低非关键动画的质量或频率。
    *   `usePageIdle` 提供 `AnimationActiveProvider` Context，允许子组件判断当前是否应暂停复杂动画以响应后台挂起事件。

## Implementation Map

### 1. 主题与无障碍状态管理
*   **文件**: `android/frontend/src/hooks/useThemeStore.ts`
    *   **行号**: 34-54 (`initTheme`)
    *   **描述**: 主题初始化的入口，负责从持久化存储加载用户偏好并应用到全局 Store。
*   **文件**: `android/frontend/src/hooks/useReducedMotion.ts`
    *   **描述**: 封装对 `window.matchMedia('(prefers-reduced-motion: reduce)')` 的监听，向 UI 层暴露布尔值信号，用于禁用或简化动画。

### 2. 性能自适应与帧率控制
*   **文件**: `android/frontend/src/hooks/useAdaptiveFramePace.ts`
    *   **描述**: 实现动态帧率控制逻辑，修复原常驻 rAF 导致页面无法休眠的问题。它是“自适应”概念中性能维度的核心。
*   **文件**: `android/frontend/src/hooks/usePageIdle.ts`
    *   **描述**: 提供 `AnimationActiveProvider` 和 `useAnimationActive()` Hook。用于检测页面可见性（Visibility API）和空闲状态，指导动画组件是否应暂停渲染。
*   **文件**: `android/frontend/src/hooks/useAnimationProfile.ts`
    *   **描述**: 定义动画质量配置文件。在安卓端恒定为最高档，但在桌面端可依据硬件能力进行分级。

### 3. 动画变体与物理引擎
*   **文件**: `android/frontend/src/lib/animations.ts`
    *   **行号**: 3-17 (`createLogEntryVariants`), 22-42 (`createPanelAppleVariants`)
    *   **描述**: 生成 `framer-motion` 所需的 `variants` 对象。这些变体是连接“主题”（视觉风格）与“动画”（运动行为）的桥梁，定义了不同场景下的入场/退出效果。
*   **文件**: `android/frontend/src/lib/easing-config.ts`
    *   **行号**: 27-29 (`getEasingConfig`)
    *   **描述**: 根据设备刷新率动态计算贝塞尔曲线参数，确保动画的时间感知在不同硬件上保持一致，体现“自适应”特性。
*   **文件**: `android/frontend/src/lib/dock-spring.ts`
    *   **描述**: 实现轻量级弹簧物理仿真（`Motion` 类）。用于 Dock 栏等高频交互场景，采用半隐式欧拉积分替代 CSS Transition，避免拖动顿挫，是高性能动画的具体实现。

### 4. 视觉应用与集成
*   **文件**: `android/frontend/src/App.tsx`
    *   **行号**: 117-118, 281-295 (面板转场动画), 199-260 (顶栏玻璃拟态)
    *   **描述**: 主容器组件，广泛引用 `useThemeStore` 和 `useAuthStore`。通过 CSS 变量和数据属性（如 `[data-glass]`）将主题状态映射到具体的玻璃拟态视觉效果。
*   **文件**: `android/frontend/src/components/layout/DockNav.tsx`
    *   **行号**: 191-195
    *   **描述**: 在低配模式下禁用镜头放大效果，体现了动画系统与性能监控模块的联动——当检测到性能瓶颈时，主动降级视觉特效。