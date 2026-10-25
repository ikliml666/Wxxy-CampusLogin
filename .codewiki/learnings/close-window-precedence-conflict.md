---
title: 窗口关闭与最小化行为决策（minimize_to_tray / lightweight_mode 优先级）
type: learning
source_files:
  - tauri-app/src-tauri/src/commands/system.rs
  - tauri-app/src-tauri/src/app/shutdown.rs
  - tauri-app/src-tauri/src/platform/autostart.rs
  - tauri-app/src-tauri/src/app/lightweight.rs
  - tauri-app/src-tauri/src/app/window.rs
tags: [教训, 轻量化, 配置优先级, Tauri, WebView2, minimize_to_tray]
---

## Overview

本文档记录了 `close_window`（关闭按钮触发，见 `tauri-app/src-tauri/src/commands/system.rs` L24-L42）与 `minimize_window`（最小化按钮触发，L8-L21）在 `minimize_to_tray` 和 `lightweight_mode` 两种配置下的行为决策与实现，以及 `tauri-app/src-tauri/src/app/shutdown.rs` 中 `handle_window_close_event`（L20-L60）对 `CloseRequested` 事件（Alt+F4/系统关闭路径）的分支处理。核心结论是：**关闭按钮在命令层应优先遵循 `minimize_to_tray` 语义（隐藏到托盘），仅当 `lightweight_mode` 开启时才执行轻量化销毁；而在事件层（Alt+F4 路径）`lightweight_mode` 优先级高于 `minimize_to_tray`**。这一决策基于 2026-10-11 的用户实测数据（见 system.rs L25-L30 注释），权衡了资源节省与用户体验。此外，`minimize_window` 与 `close_window` 在隐藏前均会同步调用 `set_main_webview_suspended` 立即挂起 WebView2（L12、L34、shutdown.rs L49 等），避免隐藏后 CSS 动画或脚本仍运行数秒。`window.rs` 中明令禁止调用 `TrySuspend`（L29-L34 注释及 L266-L271 测试），仅使用 `SetIsVisible(false)` 配合内存级别 LOW 实现挂起。

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

- **`get_init_data(state: State<'_, AppState>, app_handle: AppHandle) -> Result<serde_json::Value, String>`** (L213-L258)
  - 获取初始化数据，供前端启动。
  - **返回值包含**：掩码后 `config`（L217，出站掩码唯一出口 `masked_for_display()`，严禁明文密码）、`accounts`（L219-L222）、`version`（L224）、`autoLaunch`（L225）、`refreshRate`（L227）、`adapters` 与 `adapterDetails`（L229-L230）、`activeAccount`（L231）、`notificationEnabled`（L232）、`isAutoStart`（L234）、`logRetentionDays` 与 `debugMode`（L237-L238）、`backgroundStatus`（L241）等。

- **`notify_window_ready(window: tauri::Window) -> CommandResult`** (L390-L392)
  - 前端就绪信号（轻量化重建窗口的 ready 门用）。幂等：调用 `crate::app::lightweight::signal_window_ready(window.label())` (L391) 记录信号，等待方取走即清。

### `tauri-app/src-tauri/src/app/shutdown.rs`

- **`graceful_exit(app_handle: &AppHandle, _state: &AppState)`** (L8-L14)
  - 应用优雅退出。通过 `spawn` 异步任务 (L10) 调用 `crate::infra::lifecycle::shutdown_and_exit` (L12) 取消并等待所有后台任务结束后退出进程。

- **`handle_window_close_event(window: &Window, event: &WindowEvent)`** (L20-L60)
  - 处理 `CloseRequested` 事件（如 Alt+F4 / 系统关闭路径）。
  - **逻辑分支**：
    1. 若 `config.lightweight_mode` 为真 (L27-L40)：记录几何（`capture_geometry`，L32）、置单次退出守卫（`arm_lightweight_exit_guard`，L34）、开效率模式与轻量化标志（`set_lightweight_active(true)`，L35；Windows 下 `set_ecoqos(true)`，L37）、`return` 不 `prevent_close`，让 Tauri 真销毁窗口，由 startup.rs 的 run 回调消费守卫拦截退出。
    2. 若 `config.minimize_to_tray` 为真 (L41-L51)：`api.prevent_close()` (L43) 并 `mark_hidden_by_design(true)` (L48)、`set_main_webview_suspended(...)` (L49) 后调用 `ww.hide()` (L50)。
    3. 否则 (L52-L60)：历史缺陷修复（L53-L56 注释）——先 `api.prevent_close()` (L57) 阻止默认关闭，随后调用 `graceful_exit` (L59) 等待后台任务排空完成后主动 exit(0)，避免异步排空被中断导致日志刷盘截断。

### `tauri-app/src-tauri/src/app/lightweight.rs`

- **`WindowGeometry`** (L18-L24): 结构体，字段 `x: f64` (L20), `y: f64` (L21), `width: f64` (L22), `height: f64` (L23)。用于记录销毁前窗口几何。
- **`LIGHTWEIGHT_ACTIVE`** (L10): `static LIGHTWEIGHT_ACTIVE: AtomicBool`，轻量化生效中标志（无 WebView 窗口、EcoQoS 已开、间隔延长）。
- **`EXPECT_LIGHTWEIGHT_EXIT`** (L14): `static EXPECT_LIGHTWEIGHT_EXIT: AtomicBool`，单次退出守卫。
- **`GEOMETRY`** (L16): `static GEOMETRY: Mutex<Option<WindowGeometry>>`，销毁前几何缓存。
- **`is_lightweight_active()`** (L26-L28) / **`set_lightweight_active(active: bool)`** (L30-L32)
- **`arm_lightweight_exit_guard()`** (L34-L36) / **`take_lightweight_exit_guard()`** (L38-L40)
- **`should_prevent_exit(expecting: bool, is_quitting: bool) -> bool`** (L44-L46): 纯函数，仅当 `expecting && !is_quitting` 时拦截退出。
- **`capture_geometry(window: &tauri::WebviewWindow)`** (L48-L61) / **`take_geometry()`** (L63-L65)
- **`geometry_is_on_screen(geo: &WindowGeometry, monitors: &[(i32, i32, u32, u32)]) -> bool`** (L68-L74)
- **`effective_background_interval_ms(base_ms: u64, lightweight: bool) -> u64`** (L77-L79): 轻量化期间下限 300s。
- **`effective_quality_interval_ms(base_ms: u64, lightweight: bool) -> u64`** (L82-L84): 轻量化期间下限 1800s。
- **`READY_SIGNALS`** (L87): `static READY_SIGNALS: Mutex<Vec<(String, u64)>>`，前端就绪信号队列。
- **`signal_window_ready(label: &str)`** (L89-L97) / **`wait_window_ready_and_show(app: &tauri::AppHandle, label: &str)`** (L112-L127): 等待前端就绪信号或 5s 超时后显示窗口。

### `tauri-app/src-tauri/src/app/window.rs`

- **`HIDDEN_BY_DESIGN`** (L8): `static HIDDEN_BY_DESIGN: AtomicBool`，应用自身隐藏标记。
- **`mark_hidden_by_design(hidden: bool)`** (L10-L12) / **`is_hidden_by_design()`** (L14-L16)
- **`is_user_dismissed(window: &tauri::WebviewWindow) -> bool`** (L22-L24)
- **`set_main_webview_suspended(app: &tauri::AppHandle, suspended: bool)`** (L38-L48): 核心挂起函数，**只用 `controller.SetIsVisible`，绝不调用 `TrySuspend`**（L28-L34 注释说明根因）。隐藏期内存级别 LOW，恢复 NORMAL (L73-L82)。
- **`reset_suspend_state_for_new_webview()`** (L53-L56)
- **`handle_window_focus_event(window: &Window, event: &WindowEvent)`** (L92-L124): 焦点变化时调整内存级别。
- **`LAST_MINIMIZED`** (L129): `static LAST_MINIMIZED: AtomicBool`，去抖标志。
- **`handle_window_resized_event(window: &Window, event: &WindowEvent)`** (L140-L159): 最小化翻转时同步挂起，隐藏到托盘时不归尺寸事件管 (L149-L151)。
- **`note_minimized_synced()`** (L163-L166): 标注最小化挂起已同步。
- **`show_and_focus_main<M: tauri::Manager<tauri::Wry>>(app: &M)`** (L178-L189): 显示主窗口并聚焦，先清 `HIDDEN_BY_DESIGN` 再 show (L182)。
- **`show_or_rebuild_main(app: &tauri::AppHandle)`** (L195-L204): 窗口在则显示，不在（轻量化销毁）则重建。
- **`rebuild_main_window(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>>`** (L209-L258): 从配置重建主窗口，恢复几何，退出轻量化态，等待 ready 信号。

### `tauri-app/src-tauri/src/platform/autostart.rs`

- **`AUTOSTART_REG_KEY`** (L1) / **`AUTOSTART_REG_VALUE`** (L2): 注册表路径与键值常量。
- **`get_auto_launch_enabled() -> bool`** (L4-L15): 读取注册表判断自启。
- **`set_auto_start(exe_path: &str) -> Result<(), String>`** (L17-L30): 写入自启注册表项，附带 `--autostart` 参数 (L25)。
- **`remove_auto_start() -> Result<(), String>`** (L32-L45): 删除自启注册表项，NotFound 视为成功 (L42)。

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
         │                        │
         ▼                        ▼
┌──────────────┐          ┌──────────────────┐
│ window.rs    │          │ lightweight.rs   │
│ suspend/hide │          │ capture_geometry │
│ (L38-48)     │          │ guard/active     │
└──────────────┘          └──────────────────┘
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

- [[tauri-app/src-tauri/src/infra/state.rs]]: 包含 `AppState` 等类型（被各命令的 `State<'_, AppState>` 参数使用，如 system.rs L8；本次更新未获取到该文件内容，但类型依赖依然存在）。
- [[tauri-app/src-tauri/src/platform/autostart.rs]]: 处理自启设置，由 `set_auto_launch` (system.rs L60) 等命令调用，与关闭/最小化无直接耦合但同属 system 命令模块。
- [[tauri-app/src-tauri/src/app/lightweight.rs]]: 提供 `capture_geometry`、`arm_lightweight_exit_guard`、`set_lightweight_active`、`signal_window_ready` 等轻量化模式核心函数（在 shutdown.rs L32-L35 与 system.rs L391 调用）。
- [[tauri-app/src-tauri/src/app/window.rs]]: 提供 `mark_hidden_by_design`、`set_main_webview_suspended`、`note_minimized_synced`、`show_and_focus_main` 等窗口挂起/隐藏工具函数（在 system.rs L12-L34 与 shutdown.rs L48-L49 调用）。