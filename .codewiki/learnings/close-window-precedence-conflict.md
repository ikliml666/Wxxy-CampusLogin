---
title: 窗口关闭与最小化行为决策（minimize_to_tray / lightweight_mode 优先级）
type: learning
source_files:
  - tauri-app/src-tauri/src/commands/system.rs
  - tauri-app/src-tauri/src/app/shutdown.rs
  - tauri-app/src-tauri/src/infra/state.rs
  - tauri-app/src-tauri/src/platform/autostart.rs
  - tauri-app/src-tauri/src/app/lightweight.rs
  - tauri-app/src-tauri/src/app/window.rs
tags: [教训, 轻量化, 配置优先级, Tauri, WebView2, minimize_to_tray]
---

## Overview

本文档记录了 `close_window`（关闭按钮触发，见 `tauri-app/src-tauri/src/commands/system.rs` L24-L42）与 `minimize_window`（最小化按钮触发，L8-L21）在 `minimize_to_tray` 和 `lightweight_mode` 两种配置下的行为决策与实现，以及 `tauri-app/src-tauri/src/app/shutdown.rs` 中 `handle_window_close_event`（L20-L60）对 `CloseRequested` 事件（Alt+F4/系统关闭路径）的分支处理。核心结论是：**关闭按钮在命令层应优先遵循 `minimize_to_tray` 语义（隐藏到托盘），仅当 `lightweight_mode` 开启时才执行轻量化销毁；而在事件层（Alt+F4 路径）`lightweight_mode` 优先级高于 `minimize_to_tray`**。这一决策基于 2026-10-11 的用户实测数据（见 system.rs L25-L30 注释），权衡了资源节省与用户体验。此外，`minimize_window` 与 `close_window` 在隐藏前均会同步调用 `set_main_webview_suspended` 立即挂起 WebView2（L12、L34、L49），避免隐藏后 CSS 动画或脚本仍运行数秒。

## Key Components

### `tauri-app/src-tauri/src/commands/system.rs`

- **`minimize_window(window: Window, state: State<'_, AppState>) -> Result<(), String>`** (L8-L21)
  - 处理窗口最小化请求。
  - 在 L12 同步调用 `crate::app::window::set_main_webview_suspended(&window.app_handle(), true)` 立即挂起 WebView2（注释 L9-L11 说明 `hide` 的 `SetIsVisible(false)` 只停合成，需主动挂起实现「点下即停」）。
  - **逻辑分支**：
    1. 若 `state.config.load().minimize_to_tray` 为真 (L13)，调用 `mark_hidden_by_design(true)` (L14) 和 `window.hide()` (L15)。
    2. 否则，调用 `note_minimized_synced()` (L18) 标注去抖态（让随后的 Resized 事件不再重复触碰 COM），随后调用 `window.minimize()` (L19)。

- **`close_window(window: Window, state: State<'_, AppState>) -> Result<(), String>`** (L24-L42)
  - 处理窗口关闭请求（关闭按钮）。
  - **逻辑分支**：
    1. 若 `state.config.load().minimize_to_tray` 为真 (L31-L36)，调用 `mark_hidden_by_design(true)` (L33)、`set_main_webview_suspended(...)` (L34) 并 `window.hide()` (L35)。这是命令层最高优先级路径。
    2. 否则，若 `state.config.load().lightweight_mode` 为真 (L37-L38)，调用 `window.close()`，触发 `CloseRequested` 事件进入轻量化流程（注意：轻量化分支不置 `is_quitting`，L29-L30 注释）。
    3. 否则，置位 `state.exit.is_quitting` (L40) 并调用 `window.close()` (L41)，执行真退出。
  - **设计 rationale** (L25-L30)：`minimize_to_tray` 优先。隐藏到托盘可实现 WebView2 树自动 trim 到 ~26.7MB + 主进程 ~15.2MB、0% CPU/GPU，而销毁重建代价是 1-2s 重载 + 页面状态丢失，得不偿失。

- **`get_init_data(state: State<'_, AppState>, app_handle: AppHandle) -> Result<serde_json::Value, String>`** (L206-L255)
  - 获取初始化数据，供前端启动。
  - **返回值包含**：掩码后 `config`（L210，出站掩码唯一出口，严禁明文密码）、`accounts`（L212-L215）、`version`、`autoLaunch`（L218）、`coreAutostart`（L219-L222）、`refreshRate`（L223）、`adapters` 与 `adapterDetails`（L225-L226）、`activeAccount`、`notificationEnabled`（L227-L228）、`isAutoStart`（L230）、`logRetentionDays` 与 `debugMode`（L233-L234）、`backgroundStatus`（L237）等。

### `tauri-app/src-tauri/src/app/shutdown.rs`

- **`graceful_exit(app_handle: &AppHandle, _state: &AppState)`** (L8-L14)
  - 应用优雅退出。通过 `spawn` 异步任务调用 `crate::infra::lifecycle::shutdown_and_exit` 取消并等待所有后台任务结束后退出进程。

- **`handle_window_close_event(window: &Window, event: &WindowEvent)`** (L20-L60)
  - 处理 `CloseRequested` 事件（如 Alt+F4 / 系统关闭路径）。
  - **逻辑分支**：
    1. 若 `config.lightweight_mode` 为真 (L27-L40)：记录几何（`capture_geometry`，L32）、置单次退出守卫（`arm_lightweight_exit_guard`，L34）、开效率模式与轻量化标志（`set_lightweight_active(true)`，L35；Windows 下 `set_ecoqos(true)`，L37）、`return` 不 `prevent_close`，让 Tauri 真销毁窗口，由 startup.rs 的 run 回调消费守卫拦截退出。
    2. 若 `config.minimize_to_tray` 为真 (L41-L51)：`api.prevent_close()` (L43) 并 `mark_hidden_by_design(true)` (L48)、`set_main_webview_suspended(...)` (L49) 后调用 `ww.hide()` (L50)。
    3. 否则 (L52-L60)：历史缺陷修复（L53-L56 注释）——先 `api.prevent_close()` (L57) 阻止默认关闭，随后调用 `graceful_exit` (L59) 等待后台任务排空完成后主动 exit(0)，避免异步排空被中断导致日志刷盘截断。

## Architecture

```text
┌──── Minimize ───┐ ┌──── Close/Alt+F4 ────┐
│ system.rs       │ │ system.rs            │
│ minimize (L8-21)│ │ close (L24-42)       │
│ suspend+hide/   │ │ min_tray→hide        │
│ minimize        │ │ light→close          │
└────────┬────────┘ │ else→quit+close      │
         │          └─────────┬────────────┘
         ▼                    ▼ CloseRequested
┌──────────────────────────────────────────┐
│ shutdown.rs handle_close (L20-60)         │
│ light_mode → capture+guard+destroy        │
│ min_to_tray→ prevent_close + hide         │
│ else       → prevent_close + graceful_exit│
└──────────────────────────────────────────┘
```

## Data Flow

以下表格总结了不同配置在两个处理层级的优先级与最终行为：

| 触发层 | `minimize_to_tray` = true | `lightweight_mode` = true (且 tray=false) | 两者皆 false |
| --- | --- | --- | --- |
| 命令层 `close_window` (L24-L42) | 隐藏到托盘 (L33-L35) | 调用 `close()` 进入轻量化 (L37-L38) | 置 `is_quitting` + `close()` 真退出 (L40-L41) |
| 事件层 `handle_window_close_event` (L20-L60) | `prevent_close` + 隐藏 (L42-L51) | 放行销毁，由守卫拦截退出 (L27-L40) | `prevent_close` + `graceful_exit` (L52-L60) |
| 命令层 `minimize_window` (L8-L21) | 隐藏到托盘 (L14-L15) | 最小化到任务栏 (L18-L19) | 最小化到任务栏 (L18-L19) |

1. **Minimize Path** (system.rs L8-L21):
   - User clicks minimize button.
   - `minimize_window` command is invoked.
   - L12 同步挂起 WebView2 (`set_main_webview_suspended(true)`)。
   - If `minimize_to_tray` is true (L13), `mark_hidden_by_design(true)` + `window.hide()` (L14-L15). WebView2 处于隐藏稳态。
   - Otherwise, `note_minimized_synced()` + `window.minimize()` (L18-L19), keeping the window in the taskbar.

2. **Close Path via Button (Command Layer)** (system.rs L24-L42):
   - User clicks close button.
   - `close_window` command is invoked.
   - **If `minimize_to_tray` is true** (L31): `mark_hidden_by_design(true)` + `set_main_webview_suspended(true)` + `window.hide()` (L33-L35). Bypasses window destruction flow; no `CloseRequested` event needed.
   - **If `minimize_to_tray` is false and `lightweight_mode` is true** (L37): `window.close()` (L38). This triggers the `CloseRequested` event.
   - **If both are false** (L40-L41): `is_quitting` flag is set to true, then `window.close()`, triggering `CloseRequested`.

3. **Close Path via Alt+F4/System (Event Layer - `CloseRequested`)** (shutdown.rs L20-L60):
   - `handle_window_close_event` receives the event.
   - **If `lightweight_mode` is true** (L27): Captures geometry (L32), arms exit guard (L34), sets lightweight active flag (L35), allows window destruction (L39 return). The window is destroyed, but the process stays alive in lightweight mode.
   - **If `minimize_to_tray` is true** (L42): Prevents default close (L43), hides the window after suspending WebView2 (L48-L50).
   - **Otherwise** (L52): Prevents default close (L57), initiates `graceful_exit` (L59), waiting for background tasks to drain before exiting the process.

## Connections

- [[tauri-app/src-tauri/src/infra/state.rs]]: 包含 `AppState`（被 `minimize_window`、`close_window`、`get_init_data` 等使用，见 system.rs L3-L4 引用）与 `CommandResult` 等类型。
- [[tauri-app/src-tauri/src/platform/autostart.rs]]: 处理自启设置，由 `set_auto_launch` (system.rs L60) 等命令调用。
- [[tauri-app/src-tauri/src/app/lightweight.rs]]: 提供 `capture_geometry`、`arm_lightweight_exit_guard`、`set_lightweight_active` 等轻量化模式核心函数（在 shutdown.rs L32-L35 调用）。
- [[tauri-app/src-tauri/src/app/window.rs]]: 提供 `mark_hidden_by_design`、`set_main_webview_suspended`、`note_minimized_synced` 等窗口挂起/隐藏工具函数（在 system.rs L12-L18、L33-L34 与 shutdown.rs L48-L49 调用）。
