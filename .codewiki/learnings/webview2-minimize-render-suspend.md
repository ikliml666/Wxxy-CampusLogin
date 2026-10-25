---
title: WebView2 最小化渲染挂起与内存目标管理
type: learning
source_files:
  - tauri-app/src-tauri/src/app/window.rs
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/commands/system.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
tags: [教训, WebView2, 最小化, GPU 占用, wry, backdrop-filter, 轻量化]
---

## Overview

本文档记录了 Tauri (wry) 应用在 Windows 平台上，针对 WebView2 引擎在窗口最小化、隐藏到托盘及轻量化关闭场景下导致的 CPU/GPU 空转与高内存占用的根因分析及解决方案。核心问题在于 wry 库未自动处理窗口最小化时的 `SetIsVisible` 调用，导致 Chromium 渲染进程持续工作。同时引入了基于 `ICoreWebView2_19` 接口的内存使用目标级别（Memory Usage Target Level）动态调整机制，以配合焦点变化与挂起状态进一步优化资源使用。

关键修复点包括：
1. **弃用 `TrySuspend`**：彻底移除 `ICoreWebView2_3::TrySuspend` 调用（见 `window.rs` L26-L37 文档注释与 L58-L88 实现）。该 API 会冻结渲染进程，导致 React 并发调度器回调丢失，恢复后页面永久停止渲染（按钮失效、切页无反应），仅合成器线程的滚动可用。
2. **可见性挂起替代**：改用 `controller.SetIsVisible(false)` 实现挂起（L86）。此方法让 WebView 进入隐藏态，停止 rAF 和定时器降频，但保持渲染进程与 React 调度器存活，恢复后立即可用。
3. **状态标记与去抖**：引入 `HIDDEN_BY_DESIGN`（L8）区分用户意图隐藏与应用逻辑隐藏；引入 `LAST_MINIMIZED`（L128-L129）对高频 Resized 事件进行去抖，避免重复触碰 COM。
4. **轻量化重建状态复位**：在窗口轻量化销毁后重建时，必须重置进程级挂起标志（L53-L56），避免旧状态污染新窗口。

## Key Components

### `tauri-app/src-tauri/src/app/window.rs`

#### `HIDDEN_BY_DESIGN`
- **位置**: L8
- **类型**: `static AtomicBool`
- **用途**: 标记窗口「被应用自身隐藏」（如隐藏到托盘、静默启动）。用于区分「该显示却没显示」与「用户就是要它不可见」，避免后者被强拉回前台（v2.4.2 缺陷修复）。

#### `mark_hidden_by_design(hidden: bool)`
- **位置**: L10-L12
- **用途**: 设置 `HIDDEN_BY_DESIGN` 标志为指定状态（Release 序）。

#### `is_hidden_by_design() -> bool`
- **位置**: L14-L16
- **用途**: 读取 `HIDDEN_BY_DESIGN` 标志状态（Acquire 序）。

#### `is_user_dismissed(window: &tauri::WebviewWindow) -> bool`
- **位置**: L22-L24
- **用途**: 判断用户是否已主动收起窗口（最小化或隐藏到托盘 / 静默启动中）。后台任务看到 true 就应静默完成，不得把窗口拉回前台。托盘单击、single_instance、通知点击等用户主动唤起路径不走本判定。

#### `set_main_webview_suspended(app: &tauri::AppHandle, suspended: bool)`
- **位置**: L38-L48
- **用途**: 挂起 / 恢复主 WebView2（最小化、隐藏到托盘、轻量化关闭共用）。
- **实现细节**:
    - Windows 平台（L39-L45）：获取 "main" 窗口，通过 `with_webview` 调用 `apply_webview_suspended` 执行 COM 逻辑。
    - **绝不调用 `TrySuspend`**：仅使用 `SetIsVisible` 控制可见性，确保 React 调度器不被冻结（见 L26-L37 注释）。
    - 非 Windows 平台空实现（L46-L47）：`#[cfg(not(target_os = "windows"))] let _ = (app, suspended);`

#### `reset_suspend_state_for_new_webview()`
- **位置**: L53-L56
- **用途**: 新 WebView 建立/销毁后复位挂起相关静态态：`LAST_MINIMIZED`。防止轻量化重建窗口后残留旧值，导致新窗口的首个 Resized 事件被去抖吞掉。
- **细节**: Windows 下执行 `LAST_MINIMIZED.store(false, Ordering::Release)`（L54-L55）；非 Windows 下函数体为空。

#### `apply_webview_suspended(pw: &tauri::webview::PlatformWebview, suspended: bool)`
- **位置**: L58-L88 (Windows only, `#[cfg(target_os = "windows")]` 于 L58)
- **用途**: 实际执行 WebView2 COM 调用的底层函数。
- **关键逻辑**:
    - **内存目标级别解耦** (L73-L82): 挂起期设置为 `COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW` (尽快归还内存)，恢复后设置为 `COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL`。通过 `ICoreWebView2_19::SetMemoryUsageTargetLevel` 实现。
    - **可见性切换** (L86): 仅调用 `controller.SetIsVisible(!suspended)`。渲染进程保持存活，React 调度器与 MessageChannel 都不受影响。绝不调用 `TrySuspend`。

#### `handle_window_focus_event(window: &Window, event: &WindowEvent)`
- **位置**: L92-L120 (Windows), L123-L124 (Non-Windows)
- **用途**: 窗口焦点变化时调整 WebView2 内存使用目标级别。
- **实现细节 (Windows)**:
    - 匹配 `WindowEvent::Focused` (L93-L95)。
    - 失去焦点设置 `LOW`，获得焦点设置 `NORMAL` (L104-L108)。
    - 获取 main webview 控制器，通过 `ICoreWebView2_19::SetMemoryUsageTargetLevel` 应用 (L110-L119)。
- **非 Windows 空实现**: L123-L124 `pub fn handle_window_focus_event(_window: &tauri::WebviewWindow, _event: &WindowEvent) {}`。

#### `LAST_MINIMIZED`
- **位置**: L128-L129 (`#[cfg(target_os = "windows")]` 于 L128)
- **类型**: `static AtomicBool`
- **用途**: 上次同步给 WebView2 的「最小化」取值。Resized 事件高频（拖拽调整尺寸逐像素触发），仅在 `is_minimized` 翻转时才触碰 COM。

#### `handle_window_resized_event(window: &Window, event: &WindowEvent)`
- **位置**: L139-L159 (Windows), L168-L170 (Non-Windows)
- **用途**: 窗口最小化状态翻转时同步 WebView2 可见性：最小化挂起渲染，还原时恢复。
- **背景注释**: wry 只在显式 hide()/show() 时调 `controller.SetIsVisible`（wry 0.55.1），窗口最小化不会触发任何通知；WebView2 默认也不做原生遮挡检测。结果页面长期处于「半挂起」：定时器被节流但渲染合成继续派发，最小化后 rAF/CSS 动画/合成器全速空转，v2.4.1 为 `.glass-dock` 加 `backdrop-filter` 后每帧成本陡增，问题显性化（见 L131-L138）。
- **关键逻辑 (Windows)**:
    - 仅处理 `WindowEvent::Resized` (L141-L143)。
    - 读取 `window.is_minimized()` (L145)。
    - 隐藏到托盘的窗口不归尺寸事件管：若未最小化且 `is_hidden_by_design()` 为真，直接返回，避免虚事件污染 `LAST_MINIMIZED`（L149-L151）。
    - 与 `LAST_MINIMIZED` 比较，未翻转则直接返回（L153-L155）。
    - 翻转时调用 `set_main_webview_suspended` 同步状态（L158）。
- **非 Windows 空实现**: L168-L170 `pub fn handle_window_resized_event(_window: &tauri::WebviewWindow, _event: &WindowEvent) {}`。

#### `note_minimized_synced()`
- **位置**: L163-L166
- **用途**: 标注「最小化挂起已同步」，供 `commands::system::minimize_window` 在主动挂起后调用，避免随后的 Resized 事件重复触碰 COM。非 Windows 下仅 `LAST_MINIMIZED.store(true, ...)`（L164-L165），若非 Windows 则 `LAST_MINIMIZED` 未定义故函数体为空（L163 函数定义跨平台，但 store 在 cfg 内）。

#### `show_and_focus_main<M: tauri::Manager<tauri::Wry>>(app: &M)`
- **位置**: L178-L189
- **用途**: 显示主窗口并聚焦。窗口不存在或操作失败时静默忽略。
- **修正说明**: 对已最小化的窗口会 unminimize（SW_RESTORE）还原到前台——这是托盘单击、single_instance 二次启动等用户主动唤起路径的期望语义。前端启动期自动显示不允许打扰用户最小化，最小化保护在前端命令内做（commands/config_cmd.rs show_window）。
- **实现细节**:
    - 先清标记 `mark_hidden_by_design(false)` 再 show（L182）：show 触发的 Resized 事件靠 `is_hidden_by_design` 区分「用户主动收起」与「该恢复」，标记晚清会让这次恢复被去抖吞掉。
    - 依次调用 `unminimize()` (L183), `show()` (L184), `set_main_webview_suspended(&window.app_handle(), false)` (L186), `set_focus()` (L187)。
    - 恢复渲染放在 show 之后：窗口可见时 Resume 才不会被渲染进程判定为「不可见」而忽略。

#### `show_or_rebuild_main(app: &tauri::AppHandle)`
- **位置**: L195-L204
- **用途**: 统一入口：窗口在则显示聚焦；不在（轻量化已销毁）则重建。托盘「显示主窗口」、托盘左键单击、single_instance 回调三处共用。
- **实现**: `match` 获取 main 窗口，存在调 `show_and_focus_main`，不存在调 `rebuild_main_window` (L196-L203)。

#### `rebuild_main_window(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>>`
- **位置**: L209-L258 (私有函数)
- **用途**: 从 `tauri.conf.json` 窗口配置重建主窗口（绝不用 `WindowBuilder`——会得到无 webview 的白窗口，tauri issue #9307）。
- **关键细节**:
    - `WebviewWindowBuilder::from_config` 构建（L219）。
    - `visible(false)` 起步（由调用链保证），等前端 `notify_window_ready` 或 5s 超时兜底才 show，防重建白闪（L251-L256）。
    - 几何恢复：离屏（拔显示器）则回退配置默认 center（L221-L239）。
    - 新 WebView 是全新的 controller/进程：进程级挂起标志必须归零（调用 `reset_suspend_state_for_new_webview()`，L242），否则旧窗口残留的 `LAST_MINIMIZED=true` 会吞掉新窗口首个 Resized 恢复事件。
    - 退出轻量化态：重置标志并关闭 EcoQoS（L247-L249）。
    - 异步等待前端就绪：`tauri::async_runtime::spawn` 调用 `wait_window_ready_and_show`（L254-L256）。

#### `tests` 模块
- **位置**: L260-L272
- **用途**: 单元测试护栏。
- **`never_calls_try_suspend`** (L265-L271): 回归护栏，断言 `window.rs` 源码中不得包含 `.TrySuspend(` 字符串（拼接 `concat!(".Try", "Suspend(")` 防误匹配），防止再次引入冻结渲染进程的缺陷。

### `tauri-app/src-tauri/src/app/startup.rs`

#### `run(core_count: usize)`
- **位置**: L23-L153
- **用途**: 运行 Tauri 应用，构建 Builder 并启动。
- **关键配置**:
    - 注册插件：shell, notification, autostart, global_shortcut, single_instance（L25-L48）。其中 `single_instance` 在窗口不存在时延迟 2s 走 `show_or_rebuild_main` 重建入口（L36-L48）。
    - `.on_window_event` 回调 (L53-L65):
        - `CloseRequested` (L54-L56): 转发给 `shutdown::handle_window_close_event`。
        - `Focused` (Windows, L57-L60): 转发给 `window::handle_window_focus_event`。
        - `Resized` (Windows, L61-L64): 转发给 `window::handle_window_resized_event`。
    - `.invoke_handler`: 注册所有系统命令，包含 `minimize_window`、`close_window`、`notify_window_ready` 等（L66-L131）。
    - `RunEvent::ExitRequested` (L138-L152): 拦截轻量化关闭导致的最后一个窗口销毁触发的进程退出（托盘退出 `graceful_exit` 不拦截）。

#### `setup_app(app: &mut tauri::App, core_count: usize)`
- **位置**: L155-L266
- **用途**: 应用初始化设置。
- **关键逻辑**:
    - 创建数据目录、日志目录（探测安装目录可写性，失败 fallback 到用户数据目录）（L156-L180）。
    - 初始化 Logger，加载配置（L182-L189）。
    - RTSS hook 冲突预防留痕（L194-L199，实际处理在 main 极早期）。
    - 记录 WebView2 浏览器参数（L200-L205）。
    - 存储配置，更新 portal URL（L207-L210）。
    - 应用日志保留天数配置（L212-L215）。
    - **静默启动处理** (L217-L225): 若 `config.hidden_start` 且命令行含 `--autostart`，标记 `mark_hidden_by_design(true)`（L222-L223），防止兜底显示线程强拉前台（v2.4.2 强拉缺陷）。
    - 构建托盘、启动适配器监听、缓存刷新、更新检查循环、清理残留安装包（L227-L242）。
    - 记录 WebView2 运行时版本，订阅 ProcessFailed 崩溃事件（L244-L248）。
    - 启动心跳线程、窗口安全线程（L250-L251）。
    - GPU/刷新率检测预热（后台线程）（L253-L263）。

### `tauri-app/src-tauri/src/commands/system.rs`

#### `minimize_window(window: Window, state: State<'_, AppState>)`
- **位置**: L7-L21 (`#[tauri::command]` 于 L7, `pub fn` 于 L8)
- **用途**: 处理最小化命令。
- **逻辑**:
    - 立即挂起 WebView2 (`set_main_webview_suspended(&window.app_handle(), true)`，L12)，实现「点下即停」，不等 Resized 事件。
    - 如果配置 `minimize_to_tray` 为真（L13）：标记 `mark_hidden_by_design(true)`（L14），调用 `window.hide()`（L15）。
    - 否则（L16-L19）：调用 `window.minimize()`，并调用 `note_minimized_synced()`（L18）标注去抖态，让随后的 Resized 事件不再重复触碰 COM。

#### `close_window(window: Window, state: State<'_, AppState>)`
- **位置**: L23-L42 (`#[tauri::command]` 于 L23, `pub fn` 于 L24)
- **用途**: 处理关闭窗口命令。
- **逻辑**:
    - 优先遵循 `minimize_to_tray` 配置（L31-L36）：标记 `mark_hidden_by_design(true)` 并 `set_main_webview_suspended(true)` 后 `window.hide()`，改为隐藏到托盘而非真正关闭，以保留页面状态并节省重建开销（秒恢复、状态全保留）。
    - 若 `lightweight_mode` 开启（L37-L39）：直接 `window.close()` 走轻量化销毁路径。
    - 若非托盘且非轻量化（L40-L41）：设置 `state.exit.is_quitting.store(true, Ordering::Release)` 然后 `window.close()` 真退出。

#### `notify_window_ready(window: tauri::Window)`
- **位置**: L387-L392 (`#[tauri::command]` 于 L388, `pub fn` 于 L389)
- **用途**: 前端就绪信号（轻量化重建窗口的 ready 门用；首次启动路径由 window_safety 兜底不受影响）。幂等：多次调用只多记一条信号，等待方取走即清。内部调用 `crate::app::lightweight::signal_window_ready(window.label())`（L390）。

### `tauri-app/src-tauri/src/commands/config_cmd.rs`

#### `show_window(app_handle: AppHandle)`
- **位置**: L208-L224 (`#[tauri::command]` 于 L208, `pub fn` 于 L209)
- **用途**: 启动期自动显示路径（前端 getInitData 完成即调，v2.4.2 修复①）的最小化保护：用户在启动后短时间内最小化窗口时，不把窗口弹回前台。
- **逻辑**:
    - 若 main 窗口存在且 `crate::app::window::is_user_dismissed(&window)` 返回 true（L214-L220），直接返回 Ok(())，不弹回前台。
    - 否则调用 `crate::app::window::show_or_rebuild_main(&app_handle)`（L222）显示或重建主窗口。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                       Tauri App Runtime                      │
│                                                              │
│  ┌─────────────────┐     ┌───────────────────────────────┐   │
│  │  Window Events   │────►│  src/app/window.rs            │   │
│  │  (Startup.rs     │     │                               │   │
│  │   L53-L65)       │     │  handle_window_resized_event  │───┼──► SetIsVisible (!minimized)
│  │  Resized/Focused │     │  (Syncs Visibility L139-L159) │   │      (CoreWebView2 Controller)
│  │                  │     │  handle_window_focus_event    │───┼──► SetMemoryUsageTargetLevel
│  │                  │     │  (Adjusts Memory L92-L120)    │   │      (ICoreWebView2_19)
│  └─────────────────┘     └───────────────────────────────┘   │
│                          ┌───────────────────────────────┐   │
│  │  System Cmds    │     │  src/commands/system.rs       │   │
│  │  (L7-L42)       │────►│  minimize_window/close_window │───┼──► set_main_webview_suspended
│  └─────────────────┘     └───────────────────────────────┘   │
│                          ┌───────────────────────────────┐   │
│  │  Lightweight     │    │  rebuild_main_window          │   │
│  │  Rebuild (L209)  │────►│  reset_suspend_state_for_    │   │
│  │                  │    │  new_webview (L53-L56)        │   │
│  └─────────────────┘     └───────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **用户操作**：用户点击最小化按钮、隐藏到托盘或关闭按钮（触发 `system.rs` 命令 L7-L42），或系统窗口事件（触发 `startup.rs` 的 `on_window_event` L53-L65 转发），或前端启动完成后调 `show_window`（`config_cmd.rs` L208-L224）。
2. **路由**：
   - 事件路径：`startup.rs` 捕获 `Resized`/`Focused`，转发给 `window.rs` 的 `handle_window_resized_event` (L139-L159) / `handle_window_focus_event` (L92-L120)。
   - 命令路径：`system.rs` 的 `minimize_window` (L7-L21) / `close_window` (L23-L42) 直接调用 `window.rs` 的 `set_main_webview_suspended` (L38-L48)。
   - 前端自动显示路径：`config_cmd.rs` 的 `show_window` 调 `window::show_or_rebuild_main` (L222)。
3. **去重与判断**：
   - `handle_window_resized_event` 读取 `window.is_minimized()` (L145)，与 `LAST_MINIMIZED` (L128-L129) 比较；若未翻转则返回 (L153-L155)。
   - 隐藏到托盘的窗口不归尺寸事件管：`!minimized && is_hidden_by_design()` 时直接返回 (L149-L151)，避免虚事件污染去抖状态。
4. **WebView2 同步**：
   - 获取 WebView 控制器，`apply_webview_suspended` (L58-L88) 调用 `controller.SetIsVisible(!suspended)` (L86)。
   - 内存目标：通过 `ICoreWebView2_19::SetMemoryUsageTargetLevel` 设置 LOW/NORMAL (L73-L82, L104-L108)，与可见性解耦。
5. **焦点/恢复**：
   - `show_and_focus_main` (L178-L189) 先清 `mark_hidden_by_design(false)` (L182) 再 show，避免恢复被去抖吞掉；随后主动 `set_main_webview_suspended(..., false)` (L186) 确保渲染进程恢复。

## Connections

- 与 [[learnings/webview2-recovery]] 相关：最小化后的恢复依赖于正确的可见性状态同步，避免崩溃后无法恢复（见 `startup.rs` L244-L248 崩溃订阅与 `window.rs` 恢复逻辑）。
- 与 [[learnings/lightweight-mode]] 相关：轻量化模式下的窗口销毁与重建流程 (`rebuild_main_window` L209-L258) 与此处的可见性管理互补，重建时必须调用 `reset_suspend_state_for_new_webview` (L53-L56)。
- 与 [[learnings/gpu-performance]] 相关：`backdrop-filter` 的性能问题（v2.4.1 显性化）在此处通过强制挂起渲染（`SetIsVisible(false)`）得到缓解（见 `window.rs` L131-L138 注释）。

## Known Issues

- **v2.4.2 强拉缺陷（已规避）**：启动兜底显示线程与前端 `show_window` 命令若未区分「该显示却没显示」与「用户就是要它不可见」，会将隐藏到托盘/静默启动的窗口强拉回前台。现由 `HIDDEN_BY_DESIGN` (L8-L16) 与 `is_hidden_by_design()` 在 `setup_app` (L222-L223) 及 `show_and_focus_main` (L182) 中严谨控制。
- **v2.4.2 TrySuspend 冻结缺陷（已规避）**：曾使用 `ICoreWebView2_3::TrySuspend` 按微软休眠范式挂起，导致渲染进程冻结、React 并发调度器 `MessageChannel` 回调丢失、`pendingLanes` 待办但 `callbackNode` 空，页面永久停止渲染（按钮失效、切页无反应）。现严格只用 `SetIsVisible` (L26-L37, L86)，并加单元测试 `never_calls_try_suspend` (L265-L271) 护栏。
- **wry 0.55.1 可见性通知缺失（已缓解）**：wry 仅在显式 `hide()`/`show()` 时调 `controller.SetIsVisible`，窗口最小化不触发通知，WebView2 默认不做原生遮挡检测，导致最小化后 rAF/CSS/合成器空转。v2.4.1 引入 `.glass-dock` 的 `backdrop-filter` 后每帧成本陡增显性化。现由 `handle_window_resized_event` 主动同步 (L131-L138)。
- **tauri issue #9307 白窗口（已规避）**：轻量化重建若误用 `WindowBuilder` 会得到无 webview 的白窗口，必须用 `WebviewWindowBuilder::from_config` (L206-L219) 从配置重建。
