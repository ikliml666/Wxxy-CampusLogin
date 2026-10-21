---
type: concept
title: adaptive-form-factor-layout
source_files:
  - android/frontend/src/App.tsx
  - android/frontend/src/components/tablet/TabletShell.tsx
  - android/frontend/src/hooks/useFormFactor.ts
tags: [responsive, layout, tablet, mobile, state-management]
---

## Overview

`adaptive-form-factor-layout` 是一种跨设备形态的响应式布局架构模式。它不依赖于传统的媒体查询（Media Queries）作为唯一的决策依据，而是通过感知设备的物理维度特征（如屏幕比例、长宽比），动态切换整个应用的**渲染外壳（Shell）**与**导航范式**。

该模式的核心目标是在“手机（Mobile/Phone）”与“平板（Tablet）”两种极端形态之间建立平滑过渡：
1.  **移动端**：采用单栏流式布局，底部固定导航（Bottom Nav），强调单手操作效率。
2.  **平板端**：采用多栏分屏布局（Split View），侧边或顶部悬浮导航（Dock/DockNav），最大化利用横向空间展示内容。

该概念涉及数据流从底层硬件传感器向上层 UI 组件的单向流动，确保状态管理（Store）与视图层（View）解耦，同时保持业务逻辑的一致性。

## Involved Modules

本概念的实现横跨以下三个核心模块，分别承担检测、决策与渲染职责：

1.  **`android-frontend-hooks-stores`** (基础设施层)
    *   **职责**：提供设备形态感知的原始数据源。
    *   **关键接口**：
        *   `useFormFactor`: 封装了设备形态的判断逻辑（基于 `sw600dp` 语义，短边 ≥ 600px 视为平板）。
        *   `useOrientation`: 监听屏幕方向变化（Portrait/Landscape）。注意：源码中该 Hook 定义在 `useFormFactor.ts` 中，而非独立的 `useOrientation.ts`。
        *   `useAdapterStore`: 管理面板适配器的全局状态。

2.  **`android-frontend-app-shell`** (路由与外壳层)
    *   **职责**：作为应用入口，根据形态检测结果决定挂载哪个顶级容器组件。
    *   **关键文件**：
        *   `src/App.tsx`: 核心路由分发器，执行形态判断并渲染 `AppInner` 或 `TabletShell`。
        *   `src/main.tsx`: 初始化全局样式，为玻璃拟态等跨形态特效提供基础 CSS 变量。（注：源码中未直接引用 `main.tsx` 的具体逻辑，但它是应用入口；主要逻辑集中在 `App.tsx`）。

3.  **`android-frontend-ui` / `components`** (视图层)
    *   **职责**：实现具体的形态差异化 UI 组件。
    *   **关键组件**：
        *   `TabletShell`: 平板专用的多栏布局容器，包含侧边栏与主内容区。
        *   `DockNav`: 悬浮式导航栏，通常用于平板或大屏模式。
        *   `BottomNav`: 底部固定导航栏，通常用于手机模式。

## Flow

数据与控制流在 `adaptive-form-factor-layout` 中遵循“检测 -> 决策 -> 渲染”的单向循环：

1.  **状态采集 (State Collection)**
    *   `useFormFactor` Hook 在应用启动时（或通过 ResizeObserver/OrientationChange）读取当前窗口尺寸与比例。
    *   若检测到短边（Min Edge）≥ 600px，`formFactor` 状态更新为 `tablet`；否则为 `mobile`。此逻辑确保了横屏手机仍被识别为手机，竖屏平板仍被识别为平板。
    *   此状态被写入 React 组件状态，触发重渲染。

2.  **外壳切换 (Shell Switching)**
    *   在 `src/App.tsx` (行 364) 中，根组件 `App` 订阅 `formFactor` 状态。
    *   **分支 A (Mobile)**: 渲染 `<AppInner>` (行 52-359)。内部结构通常为：`Header` + `MainContent` + `BottomNav`。
    *   **分支 B (Tablet)**: 渲染 `<TabletShell>` (行 74-380)。内部结构通常为：`TitleBar` + `StatusBar` + `MainContent` + `RightPanel` (可选) + `DockNav`。

3.  **导航适配 (Navigation Adaptation)**
    *   在 Mobile 模式下，`BottomNav` 激活，隐藏侧边抽屉。
    *   在 Tablet 模式下，`DockNav` 激活，支持并行显示多个面板（如监控+设置）。
    *   `useMonitor` 等模块中的组件（如 `NetworkQualityCapsule`）会根据当前可用的横向空间调整自身宽度或折叠状态。

4.  **持久化与恢复 (Persistence)**
    *   用户的手动强制覆盖可能优先于自动检测，具体取决于 `useConfigStore` 中的配置（若存在）。
    *   布局状态的变化会触发 `useThemeStore` 重新计算视觉效果，确保在不同分辨率下正确渲染。

## Implementation Map

以下是实现该概念的关键代码位置映射：

| 功能环节 | 文件路径 | 行号参考 | 说明 |
| :--- | :--- | :--- | :--- |
| **形态检测 Hook** | `android/frontend/src/hooks/useFormFactor.ts` | L38-40 | 定义 `useFormFactor`，基于短边 ≥ 600px 判定平板。 |
| **方向监听 Hook** | `android/frontend/src/hooks/useFormFactor.ts` | L43-45 | 定义 `useOrientation`，监听宽高比变化。**注意：该函数定义在 `useFormFactor.ts` 中，而非独立的 `useOrientation.ts`。** |
| **核心路由分发** | `android/frontend/src/App.tsx` | L363-375 | **关键文件**。`App` 组件根据 `formFactor` 决定渲染 `AppInner` 还是 `TabletShell`。 |
| **平板外壳组件** | `android/frontend/src/components/tablet/TabletShell.tsx` | L74-380 | 实现平板端的 Grid/Flex 布局结构，包含 TitleBar, StatusBar, DockNav 及右侧日志栏。 |
| **底部导航** | `android/frontend/src/components/layout/BottomNav.tsx` | N/A | 移动端专用导航组件，由 `AppInner` 渲染。 |
| **悬浮导航** | `android/frontend/src/components/layout/DockNav.tsx` | N/A | 平板/大屏专用导航组件，由 `TabletShell` 渲染。 |
| **状态管理 Store** | `android/frontend/src/hooks/useAdapterStore.ts` | N/A | 管理面板适配逻辑，`TabletShell` 使用 `useAdapterStore` 控制活跃面板 (`activePanel`)。 |
| **全局样式初始化** | `android/frontend/src/index.css` | N/A | 定义 CSS 变量，确保不同形态下的视觉一致性。 |