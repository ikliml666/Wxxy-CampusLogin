---
type: concept
title: animation-system
source_files:
- android/frontend/src/hooks/useStartupBoost.ts
- android/frontend/src/hooks/useAdaptiveFramePace.ts
- android/frontend/src/hooks/useAnimationProfile.ts
- android/frontend/src/hooks/useBreatheAnimation.ts
- android/frontend/src/hooks/useGlowAnimation.ts
- android/frontend/src/hooks/usePageIdle.ts
- android/frontend/src/hooks/usePulseAnimation.ts
- android/frontend/src/lib/animations.ts
- android/frontend/src/lib/easing-config.ts
tags: [animation, performance, framer-motion, gsap, gpu-acceleration]
---

## Overview

`animation-system` 是 Android Tauri 前端应用的核心视觉与性能优化层，旨在平衡复杂的 UI 交互体验与低端设备的资源限制。该系统基于 `framer-motion` 处理组件级布局动画（如面板切换、弹窗显隐），并集成 `gsap` 处理高性能全局时序控制（如启动序列、帧率自适应）。其核心设计哲学是“按需计算”与“资源回收”，通过页面空闲检测、GPU 图层预热/冷却机制以及自适应帧率策略，确保应用在后台静默或前台高负载场景下的流畅性与低功耗。

## Involved Modules

该概念横跨以下模块，形成从基础设施到具体实现的完整链路：

*   **frontend-hooks-store**: 提供核心的动画控制 Hooks (`useStartupBoost`, `useAdaptiveFramePace`)、状态上下文 (`AnimationActiveProvider`) 及生命周期挂钩。这是动画系统的逻辑中枢。
*   **frontend-core-lib**: 提供底层工具函数库，包括 Framer Motion 动画变体工厂 (`createLogEntryVariants`)、缓动曲线配置 (`getEasingConfig`) 以及渲染存活探测 (`renderLiveness`)，为上层动画提供标准化参数。
*   **frontend-account**: 消费动画系统实现复杂的 UI 交互，如 `SelfOnlineStack` 中的 Coverflow 拖拽效果及堆叠视图动画。
*   **frontend-monitoring**: 利用动画原子组件 (`SignalGlowDot`) 和 Hook (`useGlowAnimation`) 实现网络质量状态的实时可视化反馈。
*   **design-prototypes**: 定义视觉基准，HTML 原型中的 CSS 变量与布局类为动画系统的样式实现提供参考约束。

## Flow

数据与控制流在模块间遵循“配置下发 -> 状态驱动 -> 硬件加速”的模式：

1.  **配置初始化**:
    *   `frontend-core-lib` 的 `easing-config.ts` 根据当前设备刷新率生成缓动曲线 (`getEasingConfig`)。
    *   `frontend-hooks-store` 的 `useAnimationProfile` 读取这些配置，作为 GSAP ticker 和 Framer Motion 变体的基础参数。

2.  **启动与预热 (Boot Phase)**:
    *   应用启动时，`useStartupBoost` (hooks) 激活，通过设置 CSS `willChange` 属性主动触发 GPU 合成层创建（Pre-warming）。
    *   `useAppInit` 串联初始化流程，此时 `AnimationActiveProvider` 上下文生效，标记动画系统处于活跃状态。

3.  **运行时自适应 (Runtime Phase)**:
    *   **帧率控制**: `useAdaptiveFramePace` 监听用户交互、页面可见性 (`document.hidden`) 及 `usePageIdle` 状态。当检测到空闲或非交互状态时，动态降低 GSAP ticker 帧率（0/30/60 FPS），减少 CPU/GPU 占用。
    *   **渲染存活**: `renderLiveness.ts` (core-lib) 以每 4s 开启 2 帧 rAF 窗口的模式探测渲染循环，防止因长时间后台运行导致的 UI 假死，同时避免常驻监听带来的功耗浪费。

4.  **UI 交互与反馈**:
    *   **组件动画**: `AccountPanel` 等组件调用 `createLogEntryVariants` 获取 Framer Motion 变体，执行列表项进入/退出动画。
    *   **持续效果**: `MonitorPanel` 使用 `useGlowAnimation` 或 `usePulseAnimation` 驱动状态指示灯的呼吸或脉冲效果，这些 Hook 直接操作 DOM 样式或依赖 React 状态重渲染。
    *   **复杂手势**: `SelfOnlineStack` 结合 `suppressClick` 防抖逻辑与 Coverflow 拖拽，利用 GSAP 或原生事件处理实现平滑的物理模拟。

5.  **资源回收 (Teardown Phase)**:
    *   页面卸载或进入后台时，`useStartupBoost` 清理 `willChange` 属性，释放 GPU 内存。
    *   `useAdaptiveFramePace` 将帧率重置或暂停，停止不必要的 ticker 轮询。

## Implementation Map

| 功能域 | 关键文件 | 行号/位置 | 职责说明 |
| :--- | :--- | :--- | :--- |
| **启动加速** | `android/frontend/src/hooks/useStartupBoost.ts` | L17 | 启动时预热 GPU 图层，动画结束后立即清理 `willChange`。 |
| **帧率自适应** | `android/frontend/src/hooks/useAdaptiveFramePace.ts` | L27-L75 | 基于 GSAP ticker，结合用户交互、可见性及空闲检测动态调整帧率 (0/30/60)。 |
| **空闲检测** | `android/frontend/src/hooks/usePageIdle.ts` | L4-L45 | 检测页面是否处于用户非活跃状态，用于触发低帧率模式。 |
| **动画上下文** | `android/frontend/src/hooks/useAdapterStore.ts` | L86-L96 | 提供 `AnimationActiveProvider`，管理动画系统的整体活性状态。 |
| **视觉效果** | `android/frontend/src/hooks/useBreatheAnimation.ts` | L9-L56 | 实现呼吸灯效果的 Hook。 |
| | `android/frontend/src/hooks/useGlowAnimation.ts` | L9-L56 | 实现发光效果的 Hook。 |
| | `android/frontend/src/hooks/usePulseAnimation.ts` | L9-L84 | 实现脉冲效果的 Hook。 |
| **动画配置** | `android/frontend/src/hooks/useAnimationProfile.ts` | N/A | 集中管理动画参数配置。 |
| **变体工厂** | `android/frontend/src/lib/animations.ts` | L1 | 导出 `createLogEntryVariants` / `createPanelAppleVariants`，封装 Framer Motion 变体逻辑。 |
| **缓动曲线** | `android/frontend/src/lib/easing-config.ts` | L1 | 根据刷新率获取适配的缓动曲线 (`getEasingConfig`)。 |
| **渲染探测** | `android/frontend/src/lib/renderLiveness.ts` | L1 | 轻量级渲染循环存活探测，平衡功耗与崩溃检测灵敏度。 |