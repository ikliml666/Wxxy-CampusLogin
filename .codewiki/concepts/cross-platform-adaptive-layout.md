---
type: concept
title: cross-platform-adaptive-layout
source_files:
  - android/frontend/src/main.tsx
  - android/frontend/src/App.tsx
  - android/frontend/src/components/layout/BottomNav.tsx
  - android/frontend/src/components/layout/DockNav.tsx
  - android/frontend/src/components/layout/RightPanel.tsx
  - android/frontend/src/components/layout/TitleBar.tsx
  - android/frontend/src/components/mobile/MobileDashboard.tsx
  - android/frontend/src/components/mobile/MobileMore.tsx
  - android/frontend/src/components/mobile/MobileQuickActions.tsx
  - android/frontend/src/components/ui/animated-card.tsx
  - android/frontend/src/hooks/useFormFactor.ts
  - android/frontend/src/settings/OnboardingWizard.tsx
  - android/frontend/src/settings/OnboardingWizardMobile.tsx
tags:
  - responsive-design
  - form-factor
  - mobile-first
  - tablet-adaptation
---

## Overview

`cross-platform-adaptive-layout` 是 Android 前端应用的核心布局策略，旨在通过统一的代码库适配不同的设备形态（手机与平板/桌面）。该概念不依赖传统的媒体查询断点硬编码，而是基于 `useFormFactor` Hook 在运行时动态判定屏幕宽度（如 sw600dp），从而在 `AppInner`（移动端视图）和 `TabletShell`（平板端外壳）之间进行切换。

这一机制确保了 UI 组件（如导航栏、面板、向导对话框）能够根据当前设备形态渲染出最适合的交互范式：移动端采用底部标签栏 (`BottomNav`) 和全屏卡片流，而平板端则采用侧边 Dock 栏 (`DockNav`)、右侧面板 (`RightPanel`) 以及分屏或独立窗口模式。同时，它要求业务逻辑层（如认证、设置、监控）保持状态一致性，仅在不同形态下通过不同的容器组件进行包装和展示。

## Involved Modules

该概念的实现涉及以下模块及其关键文件：

1. **frontend-ui-shell**
   *   **职责**: 作为应用的顶层入口和布局容器，负责初始化全局运行时环境并分发形态特定的外壳组件。
   *   **关键文件**:
       *   `android/frontend/src/App.tsx`: 包含 `AppInner` (L33-L240, 移动端主视图) 和 `App` (L243-L256, 顶层路由分发)。
       *   `android/frontend/src/main.tsx`: 应用启动入口，处理主题初始化和崩溃恢复。
       *   `android/frontend/src/components/layout/BottomNav.tsx`: 移动端底部导航 (`MobileTab`, L15; `useNavTabs`, L26-L32)。
       *   `android/frontend/src/components/layout/DockNav.tsx`: 平板端 Dock 导航 (`DockItem`, L34-L116; `ActionButton`, L119-L196)。
       *   `android/frontend/src/components/layout/RightPanel.tsx`: 平板端右侧日志/信息面板 (L45-L236)。
       *   `android/frontend/src/components/layout/TitleBar.tsx`: 标题栏组件 (L76-L226)。
       *   `android/frontend/src/components/mobile/MobileDashboard.tsx`: 移动端仪表盘及 `EXCLUDED_CARDS` 常量 (L36)。

2. **frontend-hooks-store**
   *   **职责**: 提供设备形态判定的核心 Hook，管理应用级状态以支持布局切换。
   *   **关键文件**:
       *   `android/frontend/src/hooks/useFormFactor.ts`: 定义 `useFormFactor` Hook (L12-L38)，用于判定当前设备是 Mobile 还是 Tablet/Desktop。
       *   `android/frontend/src/hooks/useAdapterStore.ts`: 管理面板导航状态，确保不同布局下的活动面板同步。

3. **frontend-settings-module**
   *   **职责**: 提供跨平台适配的用户引导流程，根据形态渲染不同的向导界面。
   *   **关键文件**:
       *   `android/frontend/src/settings/OnboardingWizard.tsx`: 桌面/平板端向导对话框 (L25-L265)。
       *   `android/frontend/src/settings/OnboardingWizardMobile.tsx`: 移动端全屏向导 (L25-L320)。
       *   `android/frontend/src/settings/useOnboardingFlow.ts`: 共享的向导逻辑 (advance, goNext, handleSkip)。

4. **frontend-shared-ui**
   *   **职责**: 提供基础原子组件，这些组件在自适应布局中需保持一致性。
   *   **关键文件**:
       *   `android/frontend/src/shared/AnimatedCard.tsx`: 动画卡片容器，被 Dashboard 和 Panel 复用。
       *   `android/frontend/src/shared/SegmentTabs.tsx`: 分段标签，用于面板内部切换。

## Flow

数据与控制流在跨平台自适应布局中的流动路径如下：

1.  **形态判定与初始化**:
    *   应用启动时，`main.tsx` 执行初始化。
    *   `App.tsx` 中的 `App` 组件挂载，内部调用 `useFormFactor()` Hook (源自 `frontend-hooks-store`)。
    *   `useFormFactor` 基于屏幕宽度（sw600dp）返回当前形态标识 (`'mobile'` 或 `'tablet'`)。

2.  **外壳分发 (Shell Dispatching)**:
    *   根据形态标识，`App.tsx` 渲染对应的顶层结构：
        *   **Mobile**: 渲染 `AppInner`，包含 `BottomNav` 和 `MobileDashboard`。
        *   **Tablet**: 渲染 `TabletShell`（在文档中隐含于 App 逻辑或相关布局组件），包含 `DockNav` (左侧)、主内容区和 `RightPanel` (右侧)。

3.  **导航与状态同步**:
    *   在 Mobile 模式下，`BottomNav` 触发路由变化，更新 `useAdapterStore` 中的 `activePanel`。
    *   在 Tablet 模式下，`DockNav` 的 `DockItem` 点击同样更新 `activePanel`，但视觉上保留侧边栏高亮。
    *   `RightPanel` 监听 `useLogToastStore` 和特定业务状态，在 Tablet 模式下常驻显示，而在 Mobile 模式下可能通过抽屉或独立页面展示（视具体实现而定，文档指出 RightPanel 为平板侧边栏组件）。

4.  **内容渲染与降级**:
    *   主内容区根据 `activePanel` 加载相应的功能模块（如 `NetworkPanel`, `DashboardPanel` 等）。
    *   对于首次启动用户，`SettingsModule` 中的 `useOnboardingFlow` 被触发。
    *   若为 Mobile，渲染 `OnboardingWizardMobile` (全屏模态)；若为 Tablet，渲染 `OnboardingWizard` (对话框形式)。两者共享相同的业务逻辑推进步骤。

5.  **响应式调整**:
    *   当用户改变窗口大小（如在平板上分屏）时，`useFormFactor` 重新计算，触发 React 重渲染，切换至对应的外壳组件，确保布局始终适配当前可用空间。

## Implementation Map

| 概念要素 | 实现位置 (文件:行号) | 说明 |
| :--- | :--- | :--- |
| **形态判定 Hook** | `android/frontend/src/hooks/useFormFactor.ts:12-38` | 核心逻辑，基于屏幕宽度判断 Mobile/Tablet。 |
| **移动端外壳** | `android/frontend/src/App.tsx:33-240` (`AppInner`) | 包含 `BottomNav` 和 `MobileDashboard` 的完整移动端视图。 |
| **平板端外壳** | `android/frontend/src/App.tsx:243-256` (`App`) / 隐含逻辑 | 顶层路由分发，根据形态选择渲染 `TabletShell` 或 `AppInner`。 |
| **移动端导航** | `android/frontend/src/components/layout/BottomNav.tsx:15-32` | `MobileTab` 类型定义及 `useNavTabs` Hook，处理底部标签切换。 |
| **平板端导航** | `android/frontend/src/components/layout/DockNav.tsx:34-196` | `DockItem` 和 `ActionButton`，处理左侧 Dock 栏交互。 |
| **平板侧边栏** | `android/frontend/src/components/layout/RightPanel.tsx:45-236` | 平板模式下右侧的信息/日志面板，利用高性能列表优化。 |
| **移动端仪表盘** | `android/frontend/src/components/mobile/MobileDashboard.tsx:36` | 定义 `EXCLUDED_CARDS`，针对移动端优化的卡片布局。 |
| **移动端向导** | `android/frontend/src/settings/OnboardingWizardMobile.tsx:25-320` | 全屏显示的移动端引导流程。 |
| **平板端向导** | `android/frontend/src/settings/OnboardingWizard.tsx:25-265` | 对话框形式的平板/桌面引导流程。 |
| **通用动画卡片** | `android/frontend/src/components/ui/animated-card.tsx:26-166` | 在所有布局中复用的卡片容器，支持入场动画。 |