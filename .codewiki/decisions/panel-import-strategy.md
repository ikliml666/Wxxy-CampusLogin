---
title: App.tsx 面板加载策略与组件架构
type: decision
source_files:
  - tauri-app/frontend/src/App.tsx
tags: [决策, 前端, 代码分割, 首屏, 性能优化]
---

## Overview

`App.tsx` 是 Tauri 应用的前端入口，负责初始化全局状态、管理窗口生命周期、协调面板切换逻辑以及定义懒加载策略。核心决策在于**放弃 `React.lazy` 对常用面板的分包**，转而采用静态导入以消除切换延迟，仅对低频对话框保留懒加载并通过启动预取优化体验。

## Key Components

### 常量与配置
- **`PANEL_TITLES`** (64|74): 定义所有面板的国际化标题键 (`titleKey`) 和描述键 (`descKey`)。包含 `dashboard`, `account`, `selfservice`, `network`, `monitor`, `quality`, `speedtest`, `settings`, `log`。
- **`SPONSOR_SHOW_DELAY_MS`** (77): 赞助弹窗启动后弹出延迟，固定为 `1_000` ms。
- **`SPONSOR_SHOW_INTERVAL_MS`** (78): 赞助弹窗自动弹出的频控间隔，固定为 `7 * 24 * 60 * 60 * 1000` ms (7天)。
- **`PANEL_CONTAINER_STYLE`** (82): 面板容器 CSS 样式对象。关键属性：`contain: 'layout style'`（防止 paint 溢出裁剪编辑模式按钮），`willChange: 'transform'`，`transform: 'translateZ(0)'`（强制 GPU 加速）。
- **`SIDE_MASCOT_POOL`** (85|89): 看板娘图片池，包含 14 个字符串常量（如 `'side-wave'`, `'side-laptop'` 等）。
- **`PanelSkeleton`** (102|112): 懒加载期间的骨架屏组件，使用 `animate-pulse` 动画。

### 核心 Hooks 与工具函数
- **`pickDailySideMascots(dayKey: string)`** (91|99): 根据日期字符串生成当日固定的左右两侧看板娘组合。算法基于 `(Math.floor(Date.parse(dayKey) / 86_400_000) % CYCLE)` 轮转，确保连续 182 天不重样。
- **`preloadPanels()`** (59|62): 启动时并行预加载所有懒加载对话框 chunk 的工具函数。调用 `Promise.allSettled` 确保即使某个 loader 失败也不阻塞其他。

### AppInner 组件内部状态与方法
- **`activePanel`** (130): 从 `useAdapterStore` 订阅当前激活的面板名称。
- **`deferredPanel`** (133): 通过 `useDeferredValue(activePanel)` 包装，实现低优先级渲染，避免快速连续切换时的挂载阻塞。
- **`configUser/EnableNetworkQuality/AutoLaunch/EnableNotification`** (142|147): 粒度订阅 `useConfigStore` 中的四个特定字段，避免全局 config 变化（如文本输入）触发 App 外壳重渲染。
- **`handlePanelChange(p: PanelName)`** (320|329): 面板切换处理函数。使用 `panelChangeLock` ref 和 60ms 定时器防止动画冲突，更新 `safeStorage` 记录偏好。
- **`handleCloseWindow()`** (295|314): 关窗处理。调用 `flushPendingConfig()` 保存配置（最多等待 2s），然后调用 `api.closeWindow()`。**注意**：前端不再注册 `onCloseRequested`，销毁权回归 Rust。
- **`handleToggleMaximize()`** (279|287): 窗口最大化切换，带错误捕获。
- **`dailySideMascots`** (118): 看板娘组合状态，每分钟比对日期跨天切换。

### 懒加载模块
- **`loadAboutDialog`** (50): 加载 `@/auth/AboutDialog`。
- **`loadThemeDialog`** (51): 加载 `@/settings/ThemeDialog`。
- **`loadOnboardingWizard`** (52): 加载 `@/settings/OnboardingWizard`。
- **`AboutDialog`**, **`ThemeDialog`**, **`OnboardingWizard`** (54|56): 上述 loader 的 `lazy` 实例。

### 静态导入面板
以下面板在 L30-L37 直接静态导入，**不使用** `React.lazy`：
- `AccountPanel` (`@/account/AccountPanel`)
- `SelfServicePanel` (`@/account/SelfServicePanel`)
- `NetworkPanel` (`@/network/NetworkPanel`)
- `MonitorPanel` (`@/monitor/MonitorPanel`)
- `QualityPanel` (`@/monitor/QualityPanel`)
- `SpeedTestPanel` (`@/monitor/SpeedTestPanel`)
- `SettingsPanel` (`@/settings/SettingsPanel`)
- `LogPanel` (`@/shared/LogPanel`) - **注意**：此前曾尝试 lazy，因切换延迟恶化回归静态导入。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                        App (Root)                            │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                      AppInner                           │ │
│  │  ┌─────────────┐  ┌──────────────┐  ┌────────────────┐ │ │
│  │  │ TitleBar    │  │ StatusBar    │  │ RightPanel     │ │ │
│  │  │ (L25|428)   │  │ (L26|443)    │  │ (Logs/Toasts)  │ │ │
│  │  └──────┬──────┘  └──────┬───────┘  └────────┬───────┘ │ │
│  │         │                │                    │         │ │
│  │  ┌──────▼────────────────▼────────────────────▼───────┐ │ │
│  │  │                  Main Content Area                 │ │ │
│  │  │  ┌──────────────────────────────────────────────┐  │ │ │
│  │  │  │  AnimatePresence (mode="wait")               │  │ │ │
│  │  │  │  ┌────────────────────────────────────────┐  │  │ │ │
│  │  │  │  │  Suspense + ErrorBoundary              │  │  │ │ │
│  │  │  │  │  ┌──────────────────────────────────┐  │  │  │ │ │
│  │  │  │  │  │  DeferredPanelContent            │  │  │  │ │ │
│  │  │  │  │  │  (Static Import for Speed)       │  │  │  │ │ │
│  │  │  │  │  │  - DashboardPanel                │  │  │  │ │ │
│  │  │  │  │  │  - AccountPanel                  │  │  │  │ │ │
│  │  │  │  │  │  - LogPanel (Static, not Lazy)   │  │  │  │ │ │
│  │  │  │  │  │  ...                             │  │  │  │ │ │
│  │  │  │  │  └──────────────────────────────────┘  │  │  │ │ │
│  │  │  │  └────────────────────────────────────────┘  │  │ │ │
│  │  │  └──────────────────────────────────────────────┘  │ │ │
│  │  └────────────────────────────────────────────────────┘ │ │
│  │                                                         │ │
│  │  ┌─────────────┐  ┌──────────────┐  ┌────────────────┐ │ │
│  │  │ DockNav     │  │ SponsorCard  │  │ Mascot Images  │ │ │
│  │  │ (L27|517)   │  │ (L23|524)    │  │ (L500|515)     │ │ │
│  │  └─────────────┘  └──────────────┘  └────────────────┘ │ │
│  │                                                         │ │
│  │  ┌──────────────────────────────────────────────────┐  │ │
│  │  │  Lazy Dialogs (Preloaded at Startup)             │  │ │
│  │  │  - AboutDialog (Suspense)                        │  │ │
│  │  │  - ThemeDialog (Suspense)                        │  │ │
│  │  │  - OnboardingWizard (Suspense)                   │  │ │
│  │  └──────────────────────────────────────────────────┘  │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **初始化**: `App` 包裹 `AnimationActiveProvider` -> `AppInner` 执行 `useAppInit`。
2.  **启动序列**: `requestAnimationFrame` 触发 `runStartupSequence()`，同时调用 `preloadPanels()` 异步下载懒加载 chunk。
3.  **窗口就绪**: 挂载完成后调用 `api.notifyWindowReady()` 通知后端。
4.  **面板切换**:
    *   用户点击 `DockNav` -> `handlePanelChange`。
    *   设置 `panelChangeLock`，更新 `useAdapterStore.activePanel`。
    *   `deferredPanel` 延迟值更新，触发 `AnimatePresence` 退出旧面板，进入新面板。
    *   新面板内容直接从静态导入的组件树渲染，无网络请求延迟。
5.  **配置同步**: `AppInner` 仅订阅 `config` 中的四个关键字段。各 Panel 自行订阅完整 store。关闭时 `handleCloseWindow` 调用 `flushPendingConfig()` 确保落盘。

## Connections

[[panel-import-strategy]] (本决策的详细背景)、[[tauri-window-management]]、[[zustand-store-architecture]]

## Known Issues

*   **注释不一致**: `App.tsx:30` 注释称"仅低频的 LogPanel/对话框保留懒加载"，但实际实现中 `LogPanel` (L21) 已回归静态导入。注释未同步更新。
*   **赞助弹窗频控**: 赞助弹窗自动弹出依赖 `localStorage` (`sponsor-last-shown`)，若用户清除浏览器数据或更换环境，会重新触发。手动入口不受频控影响。
*   **关窗超时**: `handleCloseWindow` 中 `flushPendingConfig` 最多等待 2s，若保存耗时超过此时长，窗口将强制关闭，可能导致配置丢失。