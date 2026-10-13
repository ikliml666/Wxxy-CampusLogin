---
title: 窗口关闭与最小化行为决策（minimize_to_tray / lightweight_mode 优先级）
type: learning
source_files:
  - tauri-app/src-tauri/src/commands/system.rs
  - tauri-app/src-tauri/src/app/shutdown.rs
tags: [教训, 轻量化, 配置优先级, Tauri, WebView2, minimize_to_tray]
---

## Overview

本文档记录了 `close_window`（关闭按钮/Alt+F4）与 `minimize_window`（最小化按钮）在 `minimize_to_tray` 和 `lightweight_mode` 两种配置下的行为决策与实现。核心结论是：**关闭按钮应优先遵循 `minimize_to_tray` 语义（隐藏到托盘），仅当 `lightweight_mode` 开启时才执行轻量化销毁**。这一决策基于 2026-10-11 的用户实测数据，权衡了资源节省与用户体验。

## Key Components

### `tauri-app/src-tauri/src/commands/system.rs`

- **`close_window(window: Window, state: State<'_, AppState>) -> Result<(), String>`** (L35-L49)
  - 处理窗口关闭请求。
  - **逻辑分支**：
    1. 若 `state.config.load().minimize_to_tray` 为真，调用 `window.hide()`。这是最高优先级路径。
    2. 否则，若 `state.config.load().lightweight_mode` 为真，调用 `window.close()`，触发 `CloseRequested` 事件进入轻量化流程。
    3. 否则，置位 `state.exit.is_quitting` 并调用 `window.close()`，执行真退出。
  - **设计 rationale**：`minimize_to_tray` 优先。方案 A 落地后，隐藏到托盘可实现 WebView2 树自动 trim 到 ~26.7MB + 主进程 ~15.2MB、0% CPU/GPU。而销毁重建只能再省 ~27MB，代价却是 1-2s 重载 + 页面状态丢失，得不偿失。因此关闭按钮改为隐藏到托盘（秒恢复、状态全保留）。

- **`minimize_window(window: Window, state: State<'_, AppState>) -> Result<(), String>`** (L13-L22)
  - 处理窗口最小化请求。
  - **逻辑分支**：
    1. 若 `state.config.load().minimize_to_tray` 为真，调用 `window.hide()`。注释指出 wry 在 `hide()` 时会 `SetIsVisible(false)` 挂起 WebView2 渲染，贴合「不可见即省资源」语义。
    2. 否则，调用 `window.minimize()`，保持任务栏最小化。

- **`get_init_data(state: State<'_, AppState>, app_handle: AppHandle) -> Result<serde_json::Value, String>`** (L218-L257)
  - 获取初始化数据。
  - **返回值包含**：`config`（掩码后）、`accounts`、`version`、`autoLaunch`、`gpuInfo`、`refreshRate`、`adapters`、`adapterDetails`、`disabledAdapters`、`activeAccount`、`notificationEnabled`、`isAutoStart`、`backgroundStatus` 等。

### `tauri-app/src-tauri/src/app/shutdown.rs`

- **`handle_window_close_event(window: &Window, event: &WindowEvent)`** (L20-L57)
  - 处理 `CloseRequested` 事件。
  - **逻辑分支**：
    1. 若 `config.lightweight_mode` 为真：记录几何（`capture_geometry`）、置单次退出守卫（`arm_lightweight_exit_guard`）、开效率模式与轻量化标志（`set_lightweight_active(true)`）、不 `prevent_close`，让 Tauri 真销毁窗口。窗口销毁成为最后一个窗口时 Tauri 发 ExitRequested，由 startup.rs 的 run 回调消费守卫拦截退出。
    2. 若 `config.minimize_to_tray` 为真：`api.prevent_close()` 并调用 `ww.hide()`。
    3. 否则：`api.prevent_close()` 并调用 `graceful_exit`，等待后台任务排空后主动 exit(0)。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                      User Action                           │
└───────────────┬───────────────────────────────┬─────────────┘
                ▼                               ▼
   ┌─────────────────────┐          ┌─────────────────────┐
   │  Minimize Button    │          │  Close Button /     │
   │                     │          │  Alt+F4 / System    │
   └─────────┬───────────┘          └─────────┬───────────┘
             ▼                                ▼
┌─────────────────────┐          ┌─────────────────────────────────┐
│ system.rs           │          │ system.rs                       │
│ minimize_window()   │          │ close_window()                  │
│                     │          │                                 │
│ if minimize_to_tray │          │ if minimize_to_tray             │
│   → window.hide()   │          │   → window.hide()               │
│ else                │          │ else if lightweight_mode        │
│   → window.minimize()│         │   → window.close()              │
└─────────────────────┘          │ else                            │
                                 │   → is_quitting=true            │
                                 │   → window.close()              │
                                 └──────────┬──────────────────────┘
                                            ▼
                              ┌─────────────────────────────┐
                              │ shutdown.rs                 │
                              │ handle_window_close_event() │
                              │                             │
                              │ if lightweight_mode         │
                              │   → capture_geometry()      │
                              │   → arm_lightweight_exit_guard()│
                              │   → set_lightweight_active(true)│
                              │   → allow destroy           │
                              │                             │
                              │ else if minimize_to_tray    │
                              │   → prevent_close()         │
                              │   → hide()                  │
                              │                             │
                              │ else                        │
                              │   → prevent_close()         │
                              │   → graceful_exit()         │
                              └─────────────────────────────┘
```

## Data Flow

1. **Minimize Path**:
   - User clicks minimize button.
   - `minimize_window` command is invoked.
   - If `minimize_to_tray` is true, `window.hide()` is called. WebView2 rendering is suspended.
   - Otherwise, `window.minimize()` is called, keeping the window in the taskbar.

2. **Close Path (Command Layer)**:
   - User clicks close button or presses Alt+F4.
   - `close_window` command is invoked (for close button) or `CloseRequested` event is fired (for Alt+F4/System).
   - **If `minimize_to_tray` is true**: `window.hide()` is called immediately. No `CloseRequested` event handler logic is needed for this path as it bypasses the window destruction flow.
   - **If `minimize_to_tray` is false and `lightweight_mode` is true**: `window.close()` is called. This triggers the `CloseRequested` event.
   - **If both are false**: `is_quitting` flag is set to true, then `window.close()` is called, triggering `CloseRequested`.

3. **Close Path (Event Layer - `CloseRequested`)**:
   - `handle_window_close_event` receives the event.
   - **If `lightweight_mode` is true**: Captures geometry, arms exit guard, sets lightweight active flag, allows window destruction. The window is destroyed, but the process stays alive in a lightweight mode.
   - **If `minimize_to_tray` is true**: Prevents default close, hides the window.
   - **Otherwise**: Prevents default close, initiates `graceful_exit`, waiting for background tasks to drain before exiting the process.

## Connections

- [[learnings/close-window-precedence-conflict]]: Previous version of this article, documenting the conflict between command layer and event layer priorities. The current implementation resolves this by prioritizing `minimize_to_tray` over `lightweight_mode` in the command layer, effectively unifying the behavior for the close button with the minimize button when `minimize_to_tray` is enabled.
- [[tauri-app/src-tauri/src/infra/state.rs]]: Contains `AppState` and `CommandResult`, used in the commands.
- [[tauri-app/src-tauri/src/platform/autostart.rs]]: Handles autostart settings, related to `get_auto_launch` and `set_auto_launch` commands.