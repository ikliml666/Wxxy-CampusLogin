---
title: 面板转场动画与并发渲染策略
type: decision
source_files:
  - tauri-app/frontend/src/App.tsx
  - tauri-app/frontend/src/components/layout/DockNav.tsx
  - tauri-app/frontend/src/lib/animations.ts
tags: [决策, 前端, 面板转场, 并发渲染, 动画性能]
---

## 背景

在 120Hz 屏幕下，快速连续切换面板时出现“内容与标题错位”及“短卡顿”。由于 React 并发渲染会跳过中间面板的 mount，若转场逻辑直接消费高频更新的 `activePanel`，会导致视觉上的不同步。此外，旧方案中固定 500ms 的切换锁会吞掉快速连续点击，且 LogPanel 等低频模块曾保留 `lazy` 导致切换延迟恶化至 ~366ms。

## 决策

### 1. 面板内容渲染统一使用 `useDeferredValue`

- **核心机制**：`App.tsx` 中定义 `const deferredPanel = useDeferredValue(activePanel)`。
- **应用范围**：
    - 面板内容 `switch` 分支（`App.tsx:289-337`）消费 `deferredPanel`。
    - `AnimatePresence` 的 `key` 属性（`App.tsx:417, 429`）使用 `deferredPanel`。
    - 标题/描述区域的 `AnimatePresence` `key`（`App.tsx:417`）使用 `deferredPanel`。
    - `PANEL_TITLES` 查找（`App.tsx:339`）使用 `deferredPanel`。
- **DockNav 高亮**：仍消费 `activePanel`（`DockNav.tsx:416`），因此在并发延迟期间会出现“Dock 已切换、内容仍是旧面板”的预期状态。

### 2. 动画变体与参数优化 (`lib/animations.ts`)

- **进入动画**：使用 Spring 物理模拟，`stiffness: 320, damping: 24, mass: 0.7`，对应 Apple HIG 推荐的自然轻微弹性区间（`animations.ts:13`）。
- **退出动画**：强制缩短为 `0.04s`（`animations.ts:22`），降低“等待税”，使切换更跟手。
- **Dock 激活药丸**：`layoutId="dock-active-pill"` 使用 `stiffness: 250, damping: 28` 的弹簧过渡，让跨项滑动过程可感知（`DockNav.tsx:106`）。

### 3. 切换锁与防抖策略

- **切换锁时长**：从固定的 500ms 缩减为 60ms（`App.tsx:303`），仅覆盖 `mode="wait"` 的 0.04s 退出动画及少量余量，避免吞掉快速连续点击。
- **LogPanel 导入策略**：回归静态导入。实测 `React.lazy` 即使 chunk 命中缓存，也会因 Suspense 挂载与动画重入导致切换从 ~65ms 恶化到 ~366ms（`App.tsx:31-35` 注释说明）。
- **启动预加载**：`preloadPanels()` 在启动序列开始时并行预加载剩余懒加载 chunk（`App.tsx:133-135`），消除首次切换卡顿。

## 理由

1.  **解决卡顿与错位**：`useDeferredValue` 允许 React 在快速更新时丢弃中间状态，确保最终面板的 mount 不被阻塞，解决 120Hz 下的帧预算压力。
2.  **视觉一致性**：标题与内容共用 `panelVariants` 实现同向滑入，避免旧方案中因方向计算错误导致的视觉跳跃。
3.  **性能权衡**：静态导入 LogPanel 虽然增加了初始包体积，但消除了运行时 Suspense 带来的显著延迟，收益大于代价。

## 备选方案

- **保留 `lazy` 加载**：已验证会导致显著的性能退化，被放弃。
- **更长切换锁**：原 500ms 锁会严重损害用户体验，被缩减至 60ms。

## 影响与约束

1.  **Dock 与内容不同步**：`DockNav` 高亮基于 `activePanel`，而内容基于 `deferredPanel`。在快速切换时，用户会看到 Dock 图标先变，内容后变。这是为了保持 UI 响应性的预期取舍。
2.  **硬耦合风险**：`App.tsx:287` 的 60ms 锁与 `lib/animations.ts:22` 的 0.04s 退出动画强相关。若修改动画时长，需同步调整锁的时间，否则可能导致静默失配或重复点击。
3.  **磁吸动画性能**：`DockNav.tsx` 中的磁吸效果依赖 `gsap.quickTo` 和 RAF 节流。在 `economy` 动画配置下会自动禁用，以节省性能（`DockNav.tsx:336`）。

## Connections

[[quality-detail-key-contract]]、[[panel-import-strategy]]、[[tauri-window-management]]