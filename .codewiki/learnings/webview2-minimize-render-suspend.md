---
title: WebView2 最小化渲染挂起与内存目标管理
type: learning
source_files:
  - tauri-app/src-tauri/src/app/window.rs
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/commands/system.rs
tags: [教训, WebView2, 最小化, GPU 占用, wry, backdrop-filter, 轻量化]
---

## Overview

本文档记录了 Tauri (wry) 应用在 Windows 平台上，针对 WebView2 引擎在窗口最小化场景下导致的 CPU/GPU 空转与高内存占用的根因分析及解决方案。核心问题在于 wry 库未自动处理窗口最小化时的 `SetIsVisible` 调用，导致 Chromium 渲染进程持续工作。同时，引入了基于 `ICoreWebView2_19` 接口的内存使用目标级别（Memory Usage Target Level）动态调整机制，以配合焦点变化进一步优化资源使用。

## Key Components

### `src/app/window.rs`

#### `handle_window_focus_event`
- **位置**: L6-L38 (Windows), L40-L41 (Non-Windows)
- **用途**: 监听窗口焦点变化 (`WindowEvent::Focused`)。当窗口失去焦点时，将 WebView2 的内存使用目标级别设置为 `LOW`；获得焦点时恢复为 `NORMAL`。
- **实现细节**:
    - 使用 `webview2_com_sys` 获取 `ICoreWebView2_19` 接口。
    - 调用 `SetMemoryUsageTargetLevel`。
    - 非 Windows 平台为空实现。

#### `handle_window_resized_event`
- **位置**: L53-L82 (Windows), L84-L85 (Non-Windows)
- **用途**: 监听窗口大小变化事件 (`WindowEvent::Resized`)。用于检测窗口最小化状态翻转，并同步 WebView2 的可见性。
- **关键逻辑**:
    - 使用静态原子布尔值 `LAST_MINIMIZED` (L53) 记录上次同步的最小化状态。
    - 仅在 `is_minimized()` 返回值发生翻转时执行后续操作，避免拖拽窗口时的高频 COM 调用风暴。
    - 通过 `controller.SetIsVisible(!minimized)` 通知 WebView2 挂起或恢复渲染。
    - **背景注释**: 解释了为何需要此手动干预——wry 仅在显式 hide/show 时调用 SetIsVisible，最小化不会触发任何通知，且 WebView2 默认无原生遮挡检测。

#### `show_and_focus_main`
- **位置**: L87-L93
- **用途**: 显示主窗口并聚焦。包含 unminimize 操作，以还原窗口到前台。
- **修正说明**: 原文描述中称“若窗口已最小化...不执行 unminimize”，这与源码不符。源码明确调用了 `window.unminimize()`。注释指出前端启动期不允许打扰用户最小化（由其他命令保护），但此函数作为通用入口会执行还原操作。

#### `show_or_rebuild_main`
- **位置**: L98-L108
- **用途**: 高级入口。如果窗口存在则显示聚焦；如果窗口不存在（例如被轻量化销毁），则重建窗口。
- **依赖**: 使用 `AppHandle` 来获取显示器信息以恢复窗口几何位置。

#### `rebuild_main_window`
- **位置**: L113-L165
- **用途**: 从 `tauri.conf.json` 配置重建主窗口。
- **关键细节**:
    - 绝不使用 `WindowBuilder` 直接创建，而是使用 `WebviewWindowBuilder::from_config` 以避免白屏 (Tauri issue #9307)。
    - 窗口初始状态为 `visible(false)`，等待前端发送 `notify_window_ready` 信号或 5 秒超时后才显示，防止重建时出现白闪。
    - 恢复窗口几何位置：检查记忆的位置是否仍在可用显示器范围内。
    - 退出轻量化态：重置标志并关闭 EcoQoS。

### `src/app/startup.rs`

#### `run`
- **位置**: L22-L136
- **用途**: 应用启动入口，注册全局窗口事件处理器。
- **关键配置**:
    - 在 `.on_window_event` 回调中 (L67-L76)，分发 `WindowEvent::Focused` 和 `WindowEvent::Resized` 到 `window.rs` 中的对应处理函数。
    - 处理 `single_instance` 回调，确保单实例启动时能正确显示或重建主窗口。

#### `setup_app`
- **位置**: L138-L216
- **用途**: 应用初始化设置。
- **关键日志**: 记录 WebView2 浏览器参数 (L183)，用于诊断 GPU 加速相关问题。

### `src/commands/system.rs`

#### `minimize_window`
- **位置**: L12-L22
- **用途**: 处理最小化命令。
- **逻辑**:
    - 如果配置 `minimize_to_tray` 为真，调用 `window.hide()`。这会导致 wry 内部调用 `SetIsVisible(false)`，从而挂起 WebView2 渲染。
    - 否则调用 `window.minimize()`。此时需依赖 `handle_window_resized_event` 中的逻辑来同步可见性。

#### `close_window`
- **位置**: L25-L42
- **用途**: 处理关闭窗口命令。
- **逻辑**:
    - 优先遵循 `minimize_to_tray` 配置，改为隐藏到托盘而非真正关闭，以保留页面状态并节省重建开销。
    - 仅在轻量化模式开启或非托盘模式下才执行真正的窗口关闭。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                       Tauri App Runtime                      │
│                                                              │
│  ┌─────────────────┐     ┌───────────────────────────────┐   │
│  │  Window Events   │────►│  src/app/window.rs            │   │
│  │  (Startup.rs)    │     │                               │   │
│  │                  │     │  handle_window_resized_event  │───┼──► SetIsVisible (!minimized)
│  │  Resized Event   │     │  (Syncs Visibility on Min/Unm)│   │      (CoreWebView2 Controller)
│  │  Focused Event   │     │                               │   │
│  └─────────────────┘     │  handle_window_focus_event    │───┼──► SetMemoryUsageTargetLevel
│                          │  (Adjusts Memory Target)        │   │      (ICoreWebView2_19)
│  ┌─────────────────┐     └───────────────────────────────┘   │
│  │  System Cmds    │                                         │
│  │                  │     ┌───────────────────────────────┐   │
│  │  minimize()     │────►│  wry / WebView2 Engine        │   │
│  │  hide()         │────►│                               │   │
│  └─────────────────┘     │  - Renders if Visible=true    │   │
│                          │  - Suspends if Visible=false  │   │
│                          └───────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **用户操作**: 用户点击最小化按钮或系统触发最小化。
2.  **Tauri 事件**: 触发 `WindowEvent::Resized`。
3.  **路由**: `startup.rs` 中的 `on_window_event` 捕获该事件，转发给 `window.rs` 的 `handle_window_resized_event`。
4.  **去重与判断**:
    *   读取 `window.is_minimized()`。
    *   与 `LAST_MINIMIZED` 原子变量比较。
    *   如果状态未翻转（例如只是调整大小），直接返回，不消耗 COM 资源。
    *   如果状态翻转（最小化 -> 非最小化，或反之），更新 `LAST_MINIMIZED`。
5.  **WebView2 同步**:
    *   获取 WebView 控制器。
    *   调用 `controller.SetIsVisible(!minimized)`。
    *   若最小化，`SetIsVisible(false)` 告诉 Chromium 停止渲染合成，挂起定时器。
6.  **焦点优化 (并行)**:
    *   若触发 `WindowEvent::Focused`，`handle_window_focus_event` 根据焦点状态调用 `SetMemoryUsageTargetLevel`，在后台运行时降低内存优先级。

## Connections

- 与 [[learnings/webview2-recovery]] 相关：最小化后的恢复依赖于正确的可见性状态同步，避免崩溃后无法恢复。
- 与 [[learnings/lightweight-mode]] 相关：轻量化模式下的窗口销毁与重建流程 (`rebuild_main_window`) 与此处的可见性管理互补。
- 与 [[learnings/gpu-performance]] 相关：`backdrop-filter` 的性能问题在此处通过强制挂起渲染得到缓解。