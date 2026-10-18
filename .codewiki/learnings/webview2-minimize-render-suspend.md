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

#### `HIDDEN_BY_DESIGN`
- **位置**: L10-L11
- **类型**: `static AtomicBool`
- **用途**: 标记窗口「被应用自身隐藏」（如隐藏到托盘、静默启动）。用于区分「该显示却没显示」与「用户就是要它不可见」，避免后者被强拉回前台（v2.4.2 缺陷修复）。

#### `mark_hidden_by_design(hidden: bool)`
- **位置**: L13-L15
- **用途**: 设置 `HIDDEN_BY_DESIGN` 标志为指定状态。

#### `is_hidden_by_design() -> bool`
- **位置**: L17-L19
- **用途**: 读取 `HIDDEN_BY_DESIGN` 标志状态。

#### `set_main_webview_suspended(app: &tauri::AppHandle, suspended: bool)`
- **位置**: L22-L36
- **用途**: 挂起 / 恢复主 WebView2 渲染进程（最小化、隐藏到托盘、轻量化关闭共用）。
- **实现细节**:
    - Windows 平台：获取 "main" 窗口，通过 COM 调用执行挂起/恢复逻辑。
    - 挂起流程：先 `controller.SetIsVisible(false)`（TrySuspend 硬约束），再 `TrySuspend`。这会导致渲染进程停派帧、暂停脚本定时器，内存允许被 OS 回收。
    - 恢复流程：`Resume` 再置可见。
    - 仅靠 `SetIsVisible(false)` 只停合成、不停定时器，CSS 无限动画仍在空转。
    - 非 Windows 平台为空实现。

#### `apply_webview_suspended(pw: &tauri::webview::PlatformWebview, suspended: bool)`
- **位置**: L40-L87
- **类型**: `unsafe fn` (Windows only)
- **用途**: 实际执行 WebView2 COM 调用的底层函数。
- **关键逻辑**:
    - **内存目标级别解耦** (L47-L59): 挂起期设置为 `LOW` (尽快归还内存)，恢复后设置为 `NORMAL`。通过 `ICoreWebView2_19::SetMemoryUsageTargetLevel` 实现。
    - **挂起操作** (L61-L79): 顺序不可颠倒：`SetIsVisible(false)` -> `TrySuspend`。`TrySuspend` 接受回调，记录挂起成功/失败日志。若页面仍有脚本在执行，挂起请求可能未被接受。
    - **恢复操作** (L80-L82): `Resume` -> `SetIsVisible(true)`。

#### `handle_window_focus_event(window: &Window, event: &WindowEvent)`
- **位置**: L91-L121 (Windows), L123-L124 (Non-Windows)
- **用途**: 窗口焦点变化时调整 WebView2 内存使用目标级别。
- **实现细节**:
    - 监听 `WindowEvent::Focused`。
    - 失去焦点时设置 `LOW`，获得焦点时设置 `NORMAL`。
    - 通过 `ICoreWebView2_19` 接口调用 `SetMemoryUsageTargetLevel`。

#### `LAST_MINIMIZED`
- **位置**: L128-L129
- **类型**: `static AtomicBool` (Windows only)
- **用途**: 记录上次同步给 WebView2 的最小化状态。用于去重，避免 Resized 事件高频触发（拖拽调整尺寸逐像素触发）时频繁触碰 COM。

#### `handle_window_resized_event(window: &Window, event: &WindowEvent)`
- **位置**: L138-L165 (Windows), L167-L168 (Non-Windows)
- **用途**: 窗口最小化状态翻转时同步 WebView2 可见性：最小化挂起渲染，还原时恢复。
- **背景注释**: wry 只在显式 hide()/show() 时调 `controller.SetIsVisible`，窗口最小化不会触发任何通知；WebView2 默认也不做原生遮挡检测。结果页面长期处于「半挂起」：定时器被节流但渲染合成继续派发，最小化后 rAF/CSS 动画/合成器全速空转，v2.4.1 为 .glass-dock 加 backdrop-filter 后每帧成本陡增，问题显性化。
- **关键逻辑**:
    - 仅处理 `WindowEvent::Resized`。
    - 读取 `window.is_minimized()`。
    - 与 `LAST_MINIMIZED` 比较，未翻转则直接返回。
    - 若未最小化且 `is_hidden_by_design()` 为真，直接返回（隐藏到托盘的窗口不归尺寸事件管，它本就挂起，别被还原事件顺手唤醒）。
    - 调用 `set_main_webview_suspended` 同步状态。

#### `note_minimized_synced()`
- **位置**: L170-L172
- **用途**: 标注「最小化挂起已同步」，供 `commands::system::minimize_window` 在主动挂起后调用，避免随后的 Resized 事件重复触碰 COM。非 Windows 平台为空实现。

#### `show_and_focus_main<M: tauri::Manager<tauri::Wry>>(app: &M)`
- **位置**: L175-L188
- **用途**: 显示主窗口并聚焦。窗口不存在或操作失败时静默忽略。
- **修正说明**: 对已最小化的窗口会 unminimize（SW_RESTORE）还原到前台——这是托盘单击、single_instance 二次启动等用户主动唤起路径的期望语义。前端启动期自动显示不允许打扰用户最小化，最小化保护在该命令内做（commands/config_cmd.rs show_window）。
- **实现细节**:
    - 标记 `HIDDEN_BY_DESIGN` 为 false。
    - 先恢复渲染 (`set_main_webview_suspended(..., false)`) 再显示：Resume 是挂起的显式解药，能立刻解冻，避免首帧等待。
    - 依次调用 `unminimize()`, `show()`, `set_focus()`。

#### `show_or_rebuild_main(app: &tauri::AppHandle)`
- **位置**: L193-L203
- **用途**: 统一入口：窗口在则显示聚焦；不在（轻量化已销毁）则重建。托盘「显示主窗口」、托盘左键单击、single_instance 回调三处共用。
- **依赖**: 接 AppHandle：重建路径需要 available_monitors（AppHandle 固有方法，不在 Manager trait 上）。

#### `rebuild_main_window(app: &tauri::AppHandle)`
- **位置**: L208-L252
- **类型**: `fn` (内部私有)
- **用途**: 从 tauri.conf.json 窗口配置重建主窗口。
- **关键细节**:
    - 绝不用 `WindowBuilder`——会得到无 webview 的白窗口，tauri issue #9307。使用 `WebviewWindowBuilder::from_config`。
    - `visible(false)` 起步，等前端 `notify_window_ready` 或 5s 超时兜底才 show，防重建白闪。
    - 几何恢复：离屏（拔显示器）则回退配置默认（center）。检查记忆的位置是否仍在可用显示器范围内。
    - 退出轻量化态：重置标志并关闭 EcoQoS。
    - 异步等待前端就绪：`tauri::async_runtime::spawn` 调用 `wait_window_ready_and_show`。

### `src/app/startup.rs`

#### `run(core_count: usize)`
- **位置**: L22-L136
- **用途**: 运行 Tauri 应用，构建 Builder 并启动。
- **关键配置**:
    - 注册插件：shell, notification, autostart, global_shortcut, single_instance。
    - `.on_window_event` 回调 (L67-L76):
        - `CloseRequested`: 转发给 `shutdown::handle_window_close_event`。
        - `Focused` (Windows): 转发给 `window::handle_window_focus_event`。
        - `Resized` (Windows): 转发给 `window::handle_window_resized_event`。
    - `.invoke_handler`: 注册所有系统命令。
    - `RunEvent::ExitRequested`: 拦截轻量化关闭导致的最后一个窗口销毁触发的进程退出（托盘退出 graceful_exit 不拦截）。

#### `setup_app(app: &mut tauri::App, core_count: usize)`
- **位置**: L138-L216
- **用途**: 应用初始化设置。
- **关键逻辑**:
    - 创建数据目录、日志目录（探测安装目录可写性，失败 fallback 到用户数据目录）。
    - 初始化 Logger，加载配置。
    - RTSS hook 冲突预防：此处只留痕，实际处理在 main 极早期。
    - 记录 WebView2 浏览器参数 (L183)，用于诊断 GPU 加速相关问题。
    - 存储配置，更新 portal URL。
    - 应用日志保留天数配置。
    - **静默启动处理** (L198-L202): 若 `hidden_start` 且命令行含 `--autostart`，标记 `HIDDEN_BY_DESIGN` 为 true，防止兜底显示线程强拉前台。
    - 构建托盘、启动适配器监听、缓存刷新、更新检查循环、清理残留安装包、启动后台监控任务。
    - 记录 WebView2 运行时版本，订阅 ProcessFailed 崩溃事件。
    - 启动心跳线程、窗口安全线程。
    - GPU/刷新率检测预热（后台线程）。

### `src/commands/system.rs`

#### `minimize_window(window: Window, state: State<'_, AppState>)`
- **位置**: L12-L22
- **用途**: 处理最小化命令。
- **逻辑**:
    - 如果配置 `minimize_to_tray` 为真：
        - 标记 `HIDDEN_BY_DESIGN` 为 true。
        - 立即挂起 WebView2 (`set_main_webview_suspended(..., true)`)，实现「点下即停」。
        - 调用 `window.hide()`。
    - 否则：调用 `window.minimize()`。此时需依赖 `handle_window_resized_event` 中的逻辑来同步可见性。

#### `close_window(window: Window, state: State<'_, AppState>)`
- **位置**: L25-L42
- **用途**: 处理关闭窗口命令。
- **逻辑**:
    - 优先遵循 `minimize_to_tray` 配置：改为隐藏到托盘而非真正关闭，以保留页面状态并节省重建开销。秒恢复、状态全保留。
    - 仅在轻量化模式开启或非托盘模式下才执行真正的窗口关闭。
    - 若非托盘且非轻量化，设置 `state.exit.is_quitting` 为 true，然后关闭窗口。

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