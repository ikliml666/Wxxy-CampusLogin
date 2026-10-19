---
title: 渲染管线功耗控制与合成器休眠机制
type: learning
source_files:
  - android/frontend/src/hooks/useAdaptiveFramePace.ts
  - android/frontend/src/lib/renderLiveness.ts
  - tauri-app/frontend/src/lib/renderLiveness.ts
  - android/frontend/src/index.css
tags: [性能, 省电, 渲染, rAF, 合成器, 帧率]
---

## Overview

本文档记录了 Chromium 内核（Android WebView / Tauri）环境下，前端渲染管线如何避免“常驻 rAF 阻止合成器休眠”的功耗陷阱。核心问题在于：只要页面存在 `pending` 的 `requestAnimationFrame` (rAF) 回调，Chromium 就会认为页面有持续动画，从而按屏幕刷新率满帧派发 `BeginFrame`，导致合成器永不休眠。

修复方案包含三个关键部分：
1. **动态帧率控制器** (`useAdaptiveFramePace`)：将原本的 rAF 递归轮询改为 `setInterval` 轮询 + 事件即时提帧，消除后台静置时的 pending rAF。
2. **渲染存活探测** (`renderLiveness`)：将模块级常驻 rAF 无限循环改为按需短探测（每 4s 开一个 2 帧窗口），确保在检测 GPU/合成器崩溃时不触发持续渲染。
3. **CSS 动画冻结** (`index.css`)：修复 `.anim-idle` 上下文下的动画暂停逻辑，确保无交互时雷达脉冲等视觉反馈停止驱动合成器。

修复后真机实测（25060RK16C）：前台静置 10s 渲染 0 帧，RenderThread 占用 <1%，整机功耗显著下降。

## Key Components

### `android/frontend/src/hooks/useAdaptiveFramePace.ts`

该 Hook 负责驱动 GSAP 全局 ticker 的动态帧率，实现 LTPO 类似的智能降频。

- **常量**:
    - `ACTIVE_WINDOW_MS = 2000` (:16): 交互活跃窗口时长，超过此时间未检测到交互则视为静止。
    - `PACE_POLL_MS = 250` (:18): 档位回落轮询间隔。使用 `setInterval` 而非 rAF，避免产生 pending callback。

- **状态变量 (Module Scope)**:
    - `lastActiveAt`: 最后一次标记交互的时间戳。
    - `idleFps`, `activeFps`: 当前设备配置的空闲与活跃帧率。
    - `appliedPace`: 已应用到 GSAP ticker 的帧率值，用于去重。

- **函数**:
    - `markInteraction(): void` (:26-29): 
        - **用途**: 标记交互发生。更新 `lastActiveAt` 为当前时间，并立即调用 `applyPace()` 提升帧率。
        - **触发**: 由 `pointerdown`, `touchstart`, `wheel`, `keydown`, `visibilitychange` 等事件回调调用。
    - `currentPace(): number` (:31-35): 
        - **用途**: 计算当前目标帧率。若页面隐藏返回 0；若在活跃窗口内返回 `activeFps`，否则返回 `idleFps`。
    - `applyPace(): void` (:37-45): 
        - **用途**: 刷新 GSAP ticker 帧率。若目标 pace 与 `appliedPace` 相同则空操作。注意：`gsap.ticker.fps(60)` 表示不限帧（设备恒供帧），仅在 0 < pace < 60 时设置具体数值。
    - `useAdaptiveFramePace(): void` (:51-74): 
        - **用途**: React Hook，挂载于应用根部。
        - **逻辑**: 
            1. 初始化状态。
            2. 添加 passive 事件监听器以捕获交互。
            3. 启动 `setInterval(applyPace, PACE_POLL_MS)` 负责静置后的回落。
            4. 清理函数中移除监听、清除 interval，并将 fps 重置为 60。

### `android/frontend/src/lib/renderLiveness.ts` & `tauri-app/frontend/src/lib/renderLiveness.ts`

这两个文件内容完全一致，分别服务于 Android 和 Tauri 桌面端。负责判定渲染链（GPU/合成器）是否存活。

- **常量**:
    - `PROBE_REFRESH_MS = 4_000` (:18): 距上次探测的最小间隔。小于判定阈值一半，保证正常态永不超过阈值。
    - `PROBE_TIMEOUT_MS = 500` (:20): 开窗兜底超时。防止第二帧回调丢失导致永久探测中。
    - `RENDER_STALL_THRESHOLD_MS = 10_000` (:21): 停滞判定阈值。超过此时间无 rAF 时间戳更新视为停滞。

- **状态变量**:
    - `lastRafTime`: 最近一次成功执行 rAF 回调的时间戳。
    - `probing`: 布尔值，标记当前是否正在进行 rAF 探测窗口。

- **函数**:
    - `startProbe(): void` (:24-34): 
        - **用途**: 开启一个 2 帧的 rAF 探测窗口。
        - **逻辑**: 
            1. 若已在探测中则直接返回。
            2. 设置 `probing = true`。
            3. 连续调用两次 `requestAnimationFrame`。第一帧记录时间戳，第二帧记录时间戳并复位 `probing = false`。
            4. 设置 500ms 定时器作为兜底，强制复位 `probing`。
    - `isRenderLoopAlive(): boolean` (:36-39): 
        - **用途**: 导出函数，判定渲染链是否存活。
        - **逻辑**: 
            1. 若 `document.hidden` 返回 `true`（避免节流期误判）。
            2. 若距上次探测超过 `PROBE_REFRESH_MS`，调用 `startProbe()`。
            3. 返回 `performance.now() - lastRafTime <= RENDER_STALL_THRESHOLD_MS`。

### `android/frontend/src/index.css`

负责 UI 样式及动画状态管理。

- **关键 CSS 规则**:
    - `.anim-idle .animate-pulse` (:775-777): 
        - **用途**: 在应用进入空闲状态（`.anim-idle` 类被添加）时，暂停所有 `.animate-pulse` 动画。
        - **实现**: `animation-play-state: paused !important;`。
        - **历史背景**: 曾因为逗号选择器共享声明块的问题导致失效，现已独立成规则修复。
    - `.anim-idle .signal-glow-active` (:783-785): 
        - **用途**: 同样在 idle 状态下暂停信号光效动画。
    - `.scrollbar-none` (:780-781): 移动端隐藏滚动条。

## Architecture

渲染管线功耗控制的核心架构在于**解耦“感知交互”与“维持渲染”**。传统的错误做法是使用 rAF 进行轮询或状态检查，这会导致浏览器合成器始终处于“忙碌”状态。正确的架构如下：

```text
┌──────────────────────────────────────────────────────────────┐
│                     Application Root                         │
│                                                              │
│  ┌─────────────────────┐       ┌──────────────────────────┐  │
│  │ Event Listeners     │       │ useAdaptiveFramePace Hook│  │
│  │ (pointer/touch/etc) │────►  │                          │  │
│  └─────────────────────┘       │ 1. markInteraction()     │  │
│                                 │    ► Update lastActiveAt │  │
│  ┌─────────────────────┐       │    ► applyPace() (Instant│  │
│  │ setInterval         │────►  │ 2. Polling Loop          │  │
│  │ (PACE_POLL_MS=250)  │       │    ► Check time delta    │  │
│  └─────────────────────┘       │    ► gsap.ticker.fps()   │  │
│                                 └──────────────────────────┘  │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │ Render Liveness Checker                                 │  │
│  │ (renderLiveness.ts)                                     │  │
│  │                                                         │  │
│  │ isRenderLoopAlive() ◄──► startProbe()                   │  │
│  │   (Every 4s)              (2-frame rAF window)           │  │
│  └─────────────────────────────────────────────────────────┘  │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │ CSS Animation Control                                   │  │
│  │ (index.css)                                             │  │
│  │                                                         │  │
│  │ body.anim-idle                                            │  │
│  │   .animate-pulse { play-state: paused }                 │  │
│  │   .signal-glow-active { play-state: paused }            │  │
│  └─────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

1. **交互层**: 通过被动监听器捕获用户输入，即时更新活跃状态。
2. **帧率控制层**: 使用 `setInterval` 定期检查是否需降低帧率，避免 rAF 递归。GSAP ticker 根据档位调整频率。
3. **健康检查层**: 独立的 `renderLiveness` 模块，仅在需要时开启短暂的 rAF 窗口来验证渲染进程是否响应，平时保持无 pending rAF 状态。
4. **视觉层**: CSS 类名切换控制动画播放状态，从视觉层面切断合成器对静态页面的需求。

## Data Flow

1. **用户交互**: 
   - 用户触摸屏幕。
   - `useAdaptiveFramePace` 中的 `markInteraction` 被调用。
   - `lastActiveAt` 更新，`applyPace` 将 `gsap.ticker.fps()` 设为活跃帧率（如 60）。
   
2. **静置检测**:
   - 用户停止操作超过 2s (`ACTIVE_WINDOW_MS`)。
   - `setInterval` (每 250ms) 触发 `applyPace`。
   - `currentPace()` 检测到非活跃状态，返回 `idleFps` (如 30)。
   - `gsap.ticker.fps(30)` 生效。GSAP 驱动的动画降速。
   
3. **后台/隐藏**:
   - `document.hidden` 变为 `true`。
   - `currentPace()` 返回 0。
   - `gsap.ticker.fps(60)` (代码中逻辑：pace > 0 && pace < 60 ? pace : 60，此处需注意源码注释：*“60 表示不限帧... gsap ticker 由 autoSleep(main.tsx:18)停摆”*。实际上，当页面隐藏时，浏览器会暂停 rAF，GSAP ticker 也会自动暂停，因此显式设置 fps 意义不大，但代码保持了逻辑一致性)。
   - CSS 类 `.anim-idle` 可能被父组件添加（需在 main.tsx 中配合），导致 `.animate-pulse` 等动画暂停。

4. **渲染存活探测**:
   - 业务逻辑调用 `isRenderLoopAlive()`。
   - 若距离上次探测 > 4s，调用 `startProbe()`。
   - `startProbe()` 发起两个连续的 rAF 回调。
   - 第一个 rAF 执行 `mark()`，更新 `lastRafTime`。
   - 第二个 rAF 执行 `mark()`，更新 `lastRafTime` 并清除 `probing` 标志。
   - 若 500ms 后仍未完成（如 GPU 卡死），定时器强制清除 `probing`，允许下次重试。
   - 返回 `now - lastRafTime <= 10s`。

## Connections

[[android-power-three-fixes]]: 关联的其他安卓功耗修复项。
[[android-keepalive-fgs-architecture]]: 前台服务架构，可能与后台渲染策略有关联。
[[css-comma-selector-shared-body-pitfall]]: 解释了为何 `.anim-idle` 规则必须独立，避免逗号选择器的副作用。