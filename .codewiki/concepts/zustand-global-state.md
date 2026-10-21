---
type: concept
title: zustand-global-state
source_files:
  - android/frontend/src/hooks/useQualityStore.ts
  - android/frontend/src/hooks/useThemeStore.ts
  - android/frontend/src/hooks/tauriApi.ts
  - android/frontend/src/hooks/useAdapterStore.ts
  - android/frontend/src/hooks/useAppStore.ts
  - android/frontend/src/hooks/useAuthStore.ts
  - android/frontend/src/hooks/useConfigStore.ts
  - android/frontend/src/hooks/useLogToastStore.ts
tags:
  - state-management
  - zustand
  - android-frontend
  - hooks
---

## Overview

`zustand-global-state` 是 Android 前端应用的核心状态管理范式，基于 `zustand` 库构建。它并非单一模块，而是分布在 `android/frontend/src/hooks/` 目录下的一系列原子化 Store（如 `useAuthStore`, `useConfigStore`）及其配套的 Hook。该概念定义了数据如何在后端 IPC、本地持久化与 React UI 组件之间流动，是连接底层 Tauri API 与上层业务逻辑（认证、配置、监控、主题等）的基础设施。

其核心特征包括：
1. **细粒度订阅**：UI 组件通过 Selector 精确订阅所需状态切片，避免全局重渲染。
2. **脏数据冲突解决**：在配置同步中引入 `dirtyFields` 机制及失败计数限制，防止后端旧快照覆盖本地未落盘的新值。
3. **平台适配隔离**：通过 `desktopOnly` 拦截器和 noop 实现，确保同一套 Zustand Store 在 Android 端稳定运行，屏蔽桌面专属逻辑。
4. **生命周期保护**：结合异步锁和引用检查，处理并发请求竞态及进程退出时的数据一致性。

## Involved Modules

该概念涉及以下模块，主要位于 Android 前端基础设施层：

*   **`android-frontend-hooks-and-stores`**: 核心定义层。包含所有 Zustand Store 的实现文件 (`useAuthStore.ts`, `useConfigStore.ts`, `useQualityStore.ts`, `useThemeStore.ts`, `useLogToastStore.ts`, `useAdapterStore.ts`, `useAppStore.ts`) 以及底层 IPC 适配器 `tauriApi.ts`。这是状态管理的“事实来源”。
*   **`android-frontend-app-shell`**: 状态消费层。`App.tsx` 及布局组件广泛引用各 Store 以驱动全局 UI 行为（如 Tab 顺序、主题切换）。
*   **`android-frontend-auth`**: 认证状态消费者。`DashboardPanel` 和 `useAuth` Hook 依赖 `useAuthStore` 获取认证状态。
*   **`android-frontend-settings`**: 设置与引导状态消费者。`SettingsPanel` 和 `OnboardingWizard` 依赖 `useConfigStore` 和 `useThemeStore`。
*   **`android-frontend-account`**: 账号管理状态消费者。`AccountPanel` 依赖 `useConfigStore` 获取设备别名和密码保存状态。
*   **`android-frontend-monitor`**: 监控状态消费者。`useMonitor` Hook 依赖 `useQualityStore` 进行网络质量探测，依赖 `useAuthStore` 和 `useConfigStore`。
*   **`android-frontend-entry-config`**: 初始化层。`main.tsx` 负责在各 Store 挂载前或初期进行必要的上下文初始化（如主题初始化 `initTheme`）。

## Flow

数据与控制流在 `zustand-global-state` 概念下遵循以下路径：

1.  **初始化与加载 (Init & Load)**:
    *   `main.tsx` 启动后，调用 `useAppInit`。
    *   `useInitialDataLoad` 触发首次数据获取，通过 `tauriApi` 调用后端命令（如 `get_config`, `list_accounts`）。
    *   后端返回数据后，对应的 Store Action（如 `useConfigStore.mergeConfigFromBackend`）更新 Zustand 内部状态。
    *   `useThemeStore.initTheme` 根据配置初始化主题状态，并同步 CSS 变量。

2.  **状态读写 (Read/Write)**:
    *   **读取**: UI 组件（如 `DashboardPanel`, `StatusBar`）通过 `useSelector` 从特定 Store 中订阅状态。例如，`useAuthStore(state => state.isAuthenticated)`。Zustand 的细粒度选择器确保只有相关字段变化时组件才重新渲染。
    *   **写入**: 用户交互触发 Store Action。例如，点击登录按钮调用 `useAuthStore.doLogin`。
    *   **IPC 转发**: Store Action 内部调用 `tauriApi`（来自 `tauriApi.ts`），后者封装了 Tauri 的 `invoke` 方法，将请求发送至 Rust 后端。

3.  **同步与冲突解决 (Sync & Conflict Resolution)**:
    *   当配置被修改时，`useConfigStore.updateConfig` 不仅更新内存状态，还标记 `dirtyFields`。
    *   系统尝试将变更同步回后端（`save_config`）。
    *   若同步失败，对应字段的连续失败计数递增；达到阈值后放弃脏标记，允许后端回传重新同步，避免永久脱节。
    *   后端推送新配置时，`mergeConfigFromBackend` 会对比 `dirtyFields`，仅合并非脏字段，防止后端旧数据覆盖用户未保存的本地修改。

4.  **生命周期与清理 (Lifecycle)**:
    *   Store 中的副作用由 Hook 管理。
    *   `useAuthStore.checkOnline` 使用 `_checkOnlineLockFlag` 串行化执行，防止并发调用产生冗余 IPC 请求。
    *   `useConfigStore.saveConfigDirect` 使用 `saveConfigInFlight` Promise 引用跟踪，确保窗口关闭时能等待正在进行的保存操作完成，防止数据丢失。
    *   `tauriApi` 中的重试逻辑（`tauriApiWithRetry`）处理网络抖动，确保关键操作（如配置保存）的可靠性。

## Implementation Map

具体实现位置如下：

*   **核心 Store 定义**:
    *   `android/frontend/src/hooks/useAuthStore.ts`: 认证状态 (`doLogin`, `doLogout`, `isAuthenticated`)。
    *   `android/frontend/src/hooks/useConfigStore.ts`: 全局配置状态 (`updateConfig`, `mergeConfigFromBackend`, `dirtyFields` 机制)。
    *   `android/frontend/src/hooks/useQualityStore.ts`: 网络质量状态 (`refreshQuality`)。
    *   `android/frontend/src/hooks/useThemeStore.ts`: 主题状态 (`initTheme`, 主题切换)。
    *   `android/frontend/src/hooks/useLogToastStore.ts`: 日志与通知状态 (`addLog`, `addToast`)。
    *   `android/frontend/src/hooks/useAdapterStore.ts`: 适配器状态。
    *   `android/frontend/src/hooks/useAppStore.ts`: 应用级状态。

*   **IPC 适配层**:
    *   `android/frontend/src/hooks/tauriApi.ts`: 核心 IPC 对象，暴露所有后端调用方法 (`tauriApi`) 及带重试的包装 (`tauriApiWithRetry`)。行号参考：L190-324 (核心), L356-361 (重试)。