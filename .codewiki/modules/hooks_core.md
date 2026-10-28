---
title: hooks_core (app module)
type: module
source_files:
  - tauri-app/src-tauri/src/app/mod.rs
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/app/window.rs
  - tauri-app/src-tauri/src/app/shutdown.rs
  - tauri-app/src-tauri/src/app/tray.rs
  - tauri-app/src-tauri/src/app/shortcut.rs
  - tauri-app/src-tauri/src/app/heartbeat.rs
  - tauri-app/src-tauri/src/app/lightweight.rs
  - tauri-app/src-tauri/src/app/webview_recovery.rs
  - tauri-app/src-tauri/src/main.rs
  - tauri-app/src-tauri/src/infra/events.rs
  - tauri-app/src-tauri/src/infra/lifecycle.rs
  - tauri-app/src-tauri/src/infra/command_context.rs
  - tauri-app/src-tauri/src/infra/state/mod.rs
  - tauri-app/src-tauri/src/infra/state/exit.rs
tags: [tauri, desktop, app-hooks, window, tray, lifecycle, webview-recovery, heartbeat, lightweight]
---

## Overview

`hooks_core`（Rust 侧即 `tauri-app/src-tauri/src/app` 模块，仅在 `#[cfg(desktop)]` 编译，见 `tauri-app/src-tauri/src/lib.rs` 第 12–13 行）是桌面端 Tauri 应用的**核心应用钩子层**：把 Tauri 的 API/生命周期、设备（GPU/WebView2 进程）能力、初始化流程与事件总线串成一条启动—运行—退出链路。

模块名到代码路径的对应关系：本仓库把这一层命名为 `hooks_core`，实际就是桌面专属的 `app` crate 模块，内部再按职责拆成 9 个文件：

- `startup.rs` — 应用入口装配：构建 Tokio runtime、挂载 Tauri 插件、`.setup()` 启动时初始化、窗口事件钩子、`invoke_handler` 命令表、`RunEvent::ExitRequested` 拦截。
- `window.rs` — 主窗口显隐/挂起控制（`SetIsVisible` 而非 `TrySuspend`）、窗口事件处理、轻量化销毁后的重建。
- `shutdown.rs` — 窗口关闭事件分支与优雅退出。
- `tray.rs` — 托盘图标/菜单构建、菜单与图标点击事件。
- `shortcut.rs` — 全局快捷键（取消退出）事件处理。
- `heartbeat.rs` — 前端心跳监测线程 + 窗口兜底显示线程。
- `lightweight.rs` — 轻量化运行态：状态标志、窗口几何缓存、单次退出守卫、间隔系数、前端就绪信号。
- `webview_recovery.rs` — WebView2 `ProcessFailed` 订阅与崩溃分级恢复（reload / 应用重启）、运行时版本记录。

外部协作（非本模块文件，但被大量引用，行号以读到的源码为准）：

- `tauri-app/src-tauri/src/main.rs` — 进程级 preinit（panic hook、RTSS 检测、`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` env、runtime 构建），在 WebView2 创建之前执行（第 17–82 行）。
- `tauri-app/src-tauri/src/infra/lifecycle.rs` — 校园网退出/自动退出/统一退出编排（被 `shutdown.rs`、`shortcut.rs` 调用）。
- `tauri-app/src-tauri/src/infra/events.rs` — `EventBus` 统一事件总线（被 `tray.rs`、`lifecycle.rs` 等使用）。
- `tauri-app/src-tauri/src/infra/command_context.rs` — `CommandContext` / `AppHandleExt`，命令层公共上下文（被 `shutdown.rs`、`tray.rs` 使用）。
- `tauri-app/src-tauri/src/infra/state/mod.rs` 与 `exit.rs` — `AppState` 及其 `exit` 子状态，承载本模块所有跨线程共享标志。

## Key Components

以下按文件列出本模块所有公开函数/常量，行号取自 `read_code` 返回前缀。

### `app/mod.rs`
模块声明（第 1–9 行），依次暴露子模块：`startup`、`tray`、`shortcut`、`heartbeat`、`shutdown`、`lightweight`、`webview_recovery`、`window`。无独立函数。

### `app/startup.rs`
- `build_runtime(core_count: usize) -> tokio::runtime::Runtime`（第 6–20 行）：构建多线程 Tokio runtime；`worker_threads = core_count.clamp(2, 8)`，`max_blocking_threads = (core_count*4).clamp(8, 64)`，线程名 `campus-worker`，`enable_all()`；失败直接 `std::process::exit(1)`。
- `run(core_count: usize)`（第 23–159 行）：应用总装配。`Builder::default()` 后依次挂插件：`tauri_plugin_shell`、`tauri_plugin_notification`、`tauri_plugin_autostart`（`MacosLauncher::LaunchAgent`）、`tauri_plugin_global_shortcut`（`.with_handler` 转发到 `shortcut::handle_shortcut_event`，第 31–35 行）、`tauri_plugin_single_instance`（二次启动转发到 `window::show_or_rebuild_main`，第 36–48 行）；`.manage(AppState::new())`；`.setup(setup_app)`（第 50–52 行）；`.on_window_event`（第 53–65 行，分发 `shutdown::handle_window_close_event`、`window::handle_window_focus_event`、`window::handle_window_resized_event`）；`.invoke_handler(generate_handler![...])`（第 66–137 行，命令表，含全部前台可调命令）；`app.run(|_app, event| ...)`（第 144–158 行）拦截 `RunEvent::ExitRequested`，用 `lightweight::take_lightweight_exit_guard()` + `AppState.exit.is_quitting` 经 `lightweight::should_prevent_exit` 判定是否 `prevent_exit()`。
- `setup_app(app: &mut tauri::App, core_count) -> Result<(), ...>`（第 161–272 行）：`.setup` 回调。流程：解析 `app_data_dir` 与安装目录 → 探测日志可写并 `logger::init_logger`（第 162–186 行）→ `config_cmd::load_config_from_disk_or_default` 加载配置（第 194 行）→ `rtss_compat::log_preinit_outcome`（第 205 行，Windows desktop）→ 记录 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`（第 207–211 行）→ `CommandContext::from_app(app)` 并通过 `state.config.store` 写入（第 213–215 行）→ `network::update_portal_url` → `logger::set_log_retention_days`（第 221 行）→ 静默启动判定（`hidden_start && --autostart` → `window::mark_hidden_by_design(true)`，第 226–231 行）→ `tray::build_tray`（第 233 行）→ `monitor::adapter_watch::start_adapter_watch` / `network::adapter_cache::start_cache_refresh_task`（第 236–241 行）→ `update::updater::start_update_check_loop` / `cleanup_stale_update_dir`（第 243–246 行）→ `monitor::watcher::run_startup_tasks`（第 248 行）→ `webview_recovery::record_webview2_runtime_version` / `subscribe_process_failed`（第 252–254 行）→ `heartbeat::spawn_heartbeat_thread` / `spawn_window_safety_thread`（第 256–257 行）→ GPU 预热后台线程（第 261–269 行）。

### `app/window.rs`
- `HIDDEN_BY_DESIGN: AtomicBool`（第 8 行）：静态标志，标记「窗口被应用自身隐藏（托盘/静默启动）」，用于区分用户意图隐藏与被误强拉前台。
- `mark_hidden_by_design(hidden: bool)`（第 10–12 行）/ `is_hidden_by_design() -> bool`（第 14–16 行）：读写该标志。
- `is_user_dismissed(window) -> bool`（第 22–24 行）：`is_minimized() || is_hidden_by_design()`，后台任务据此判定是否静默完成。
- `set_main_webview_suspended(app, suspended: bool)`（第 38–48 行）：**仅用 `controller.SetIsVisible`，绝不调用 `TrySuspend`**（见文档注释第 26–37 行：v2.4.2 缺陷根因——`TrySuspend` 冻结渲染进程使 React 调度器死锁）。
- `reset_suspend_state_for_new_webview()`（第 53–56 行）：重建窗口后清零进程级 `LAST_MINIMIZED` 标志。
- `apply_webview_suspended(pw, suspended)`（第 58–88 行，`#[cfg(windows)]`）：实际 COM 调用，`ICoreWebView2_19::SetMemoryUsageTargetLevel`（LOW/NORMAL）配合 `SetIsVisible(!suspended)`。
- `handle_window_focus_event(window, event)`（第 92–124 行，Windows 实现 + 非 Windows 空实现）：焦点变化时调整 WebView2 内存目标级别。
- `LAST_MINIMIZED: AtomicBool`（第 129 行，`#[cfg(windows)]`）：上次同步给 WebView2 的最小化取值，去抖用。
- `handle_window_resized_event(window, event)`（第 140–170 行，含非 Windows 空实现）：`Resized` 事件下仅在 `is_minimized` 翻转时同步 WebView2 可见性；隐藏到托盘的窗口直接跳过以免被唤醒。
- `note_minimized_synced()`（第 163–166 行）：主动挂起后调用，`LAST_MINIMIZED.store(true)`，避免随后 `Resized` 重复触碰 COM。
- `show_and_focus_main<M: Manager<Wry>>(app)`（第 178–189 行）：清 `hidden_by_design` → `unminimize` → `show` → `set_main_webview_suspended(false)` → `set_focus`。
- `show_or_rebuild_main(app: &AppHandle)`（第 195–204 行）：窗口在则 `show_and_focus_main`，否则 `rebuild_main_window`（托盘/单击/single_instance 三处共用入口）。
- `rebuild_main_window(app) -> Result<(), ...>`（第 209–258 行，私有但被上者调用）：从 `tauri.conf.json` 的 `main` 窗口配置经 `WebviewWindowBuilder::from_config` 重建（**绝不用 `WindowBuilder`**，避免 tauri issue #9307 白窗）；恢复几何（离屏回退居中）、归零挂起态、退出轻量化态、经 `wait_window_ready_and_show` 显示。
- `tests::never_calls_try_suspend`（第 265–271 行）：回归护栏，`include_str!("window.rs")` 断言不含 `TrySuspend(`。

### `app/shutdown.rs`
- `graceful_exit(app_handle, _state)`（第 8–14 行）：spawn 异步任务，经 `lifecycle::shutdown_and_exit` 取消并等待后台任务后 `exit(0)`。
- `handle_window_close_event(window, event)`（第 20–61 行）：`CloseRequested` 三分支——① `lightweight_mode`：记录几何、置单次退出守卫、开轻量化与 EcoQoS、不 `prevent_close`（让 Tauri 真销毁，由 `RunEvent::ExitRequested` 守卫拦截，第 27–40 行）；② `minimize_to_tray`：`prevent_close` + 挂起 + `hide`（第 41–51 行）；③ 否则：`prevent_close` + `graceful_exit`（修复历史缺陷——直接退出会中断异步排空，第 52–60 行）。

### `app/tray.rs`
- `TRAY_ID = "main-tray"`（第 11 行）、`SWITCH_ITEM_PREFIX = "switch-account:"`（第 14 行）。
- `build_tray_menu(app) -> tauri::Result<Menu<Wry>>`（第 23–70 行）：按配置构建菜单——显示主窗口、快速登录、快速注销（仅 `config.user` 非空时启用）、切换账号子菜单（列出已保存账号，当前账号禁用并标「（当前）」）、退出。
- `refresh_tray_menu_state(app)`（第 75–88 行）：经 `spawn_blocking` 重新构建并 `set_menu`（账号/配置变化后调用）。
- `build_tray(app, install_dir)`（第 91–118 行）：加载托盘图标（默认图标 → `icons/icon.ico` → 内嵌 `include_bytes!("../../icons/icon.ico")`），`TrayIconBuilder::with_id(TRAY_ID).on_menu_event(handle_tray_menu_event).on_tray_icon_event(handle_tray_icon_event)`。
- `handle_tray_menu_event(app, event)`（第 121–210 行）：菜单分发——`show`→`window::show_or_rebuild_main`；`quick-login`→`spawn_blocking` 取 `is_logging_in` 锁后 `auth::service::full_login` + `post_login_handler`；`quick-logout`→`commands::login::perform_full_logout_sync`；`switch-account:<id>`→`commands::account::perform_switch_account_sync`；`quit`→`shutdown::graceful_exit`。
- `handle_tray_icon_event(tray, event)`（第 213–220 行）：左键单击 → `window::show_or_rebuild_main`。

### `app/shortcut.rs`
- `CANCEL_KEY: OnceLock<Option<Shortcut>>`（第 6 行）：取消退出快捷键解析缓存。
- `handle_shortcut_event(app, shortcut, event)`（第 9–32 行）：仅处理 `ShortcutState::Pressed`；解析并比对 `CANCEL_EXIT_SHORTCUT`（`CommandOrControl+Shift+C`，定义在 `state/mod.rs` 第 14 行）；命中后 `spawn_blocking` 同时调 `lifecycle::cancel_auto_exit_inner` 与 `lifecycle::cancel_campus_exit_with_notification`。

### `app/heartbeat.rs`
- `spawn_heartbeat_thread(app_handle)`（第 9–61 行）：每 5s 检查一次前端心跳（`state.update_stats.last_render_heartbeat_ms`，定义见 `state/mod.rs` 第 151 行）；`is_visible() && !is_minimized()` 才监测；失联超 `crash_threshold_ms = 20_000` 且连续 3 次则调 `webview_recovery::attempt_webview_recovery`。
- `spawn_window_safety_thread(app_handle)`（第 66–95 行）：启动 3s 后检查主窗口可见性，不可见且非 `is_user_dismissed` 则 `show_and_focus_main`；最多重试 3 次。

### `app/lightweight.rs`
全部内存态（不落盘）。
- `LIGHTWEIGHT_ACTIVE: AtomicBool`（第 10 行）：轻量化生效中标志。
- `EXPECT_LIGHTWEIGHT_EXIT: AtomicBool`（第 14 行）：单次退出守卫（CloseRequested 置位、ExitRequested 取走，单次语义）。
- `GEOMETRY: Mutex<Option<WindowGeometry>>`（第 16 行）：销毁前记录的窗口几何。
- `struct WindowGeometry { x: f64, y: f64, width: f64, height: f64 }`（第 18–24 行）：窗口几何；4 个 `f64` 字段（位置与尺寸，已按 `scale_factor` 归一化）。
- `is_lightweight_active() -> bool`（第 26–28 行）/ `set_lightweight_active(active)`（第 30–32 行）。
- `arm_lightweight_exit_guard()`（第 34–36 行）/ `take_lightweight_exit_guard() -> bool`（第 38–40 行，单次消费 `swap`）。
- `should_prevent_exit(expecting, is_quitting) -> bool`（第 44–46 行，纯函数）：`expecting && !is_quitting`。
- `capture_geometry(window)`（第 48–61 行）：记录外置位置/尺寸/缩放并归一到 `GEOMETRY`。
- `take_geometry() -> Option<WindowGeometry>`（第 63–65 行）：取走并清空。
- `geometry_is_on_screen(geo, monitors) -> bool`（第 68–74 行，纯函数）：与任一显示器矩形相交即有效。
- `effective_background_interval_ms(base_ms, lightweight) -> u64`（第 77–79 行，纯函数）：轻量化时下限 300_000ms。
- `effective_quality_interval_ms(base_ms, lightweight) -> u64`（第 82–84 行，纯函数）：轻量化时下限 1_800_000ms。
- `READY_SIGNALS: Mutex<Vec<(String, u64)>>`（第 87 行）：前端 `notify_window_ready` 信号。
- `signal_window_ready(label)`（第 89–97 行）：记录就绪信号。
- `take_window_ready(label) -> bool`（第 99–107 行，私有）：取走对应 label 信号。
- `wait_window_ready_and_show(app, label)`（第 112–127 行）：轮询（500ms）等待 ready 信号或 5s 超时，再 `window::show_and_focus_main`。
- 测试（第 130–167 行）：守卫判定、间隔下限、几何离屏、单次守卫语义。

### `app/webview_recovery.rs`
- 常量：`RECOVERY_WINDOW_MS = 5*60*1000`、`RECOVERY_MAX_PER_WINDOW = 3`（第 28–29 行，reload 滑动窗口）；`RESTART_GUARD_WINDOW_MS = 10*60*1000`、`RESTART_GUARD_MAX = 2`、`RESTART_GUARD_FILE = "webview_restart_guard"`（第 33–35 行，重启跨进程限流）。
- `recovery_gate(window_start_ms, count, now_ms) -> (bool, u64, u32)`（第 39–47 行，纯函数）：reload 节流判定。
- `epoch_ms() -> u64`（第 49–54 行）。
- `attempt_webview_recovery(app, reason)`（第 58–114 行）：reload 唯一入口；经 `recovery_gate` 限流后 `window.eval("window.location.reload()")`；`eval` 失败记 ERROR（WebView 已失效，但不自动重启）。
- `restart_guard_decide(timestamps, now_ms) -> (bool, Vec<u64>)`（第 118–129 行，纯函数）。
- `restart_guard_read(path) -> Vec<u64>`（第 132–136 行）。
- `restart_guard_allow(app, now_ms) -> bool`（第 140–152 行）：跨进程限流 + 落盘（`webview_restart_guard` 文件）。
- `attempt_app_restart(app, reason)`（第 157–176 行）：浏览器进程退出 → `flush()` 后 `app.restart()`。
- `subscribe_process_failed(app)`（第 180–237 行，`#[cfg(windows)]`）：经 `ICoreWebView2_4::add_ProcessFailed` 订阅崩溃事件。
- `handle_process_failed(app, kind, reason, exit_code)`（第 246–269 行，`#[cfg(windows)]`）：分级——浏览器进程退出→`attempt_app_restart`；渲染/iframe 渲染进程退出→`attempt_webview_recovery`；其余记日志交运行时自愈。
- `describe_process_failed(kind, reason, exit_code) -> String`（第 274–286 行）、`describe_process_failed_reason(reason) -> &str`（第 290–317 行）、`describe_process_failed_kind(kind) -> String`（第 321–360 行）：诊断串构建（含 Reason 语义文本：崩溃/启动失败/内存不足/目录被删/外部终止/未预期/无响应）。
- `record_webview2_runtime_version()`（第 363–415 行）：从 EdgeUpdate 注册表（HKLM WOW6432Node / HKLM / HKCU）读 `pv`，回退到 `C:\Program Files (x86)\Microsoft\EdgeWebView\Application\<version>` 安装目录最高版本。
- `pick_runtime_version(entries) -> Option<String>`（第 420–433 行，Windows，纯函数）：从目录条目取最高四段版本号。
- 测试（第 435–514 行）：recovery_gate 计数/限流/窗口过期、restart_guard 上限/窗口过期、版本目录回退。

### 外部协作结构（本模块频繁引用）
- `AppState`（`infra/state/mod.rs` 第 184–192 行）：字段 `config`、`tasks`、`task_manager`、`network`、`exit`、`update_stats`、`scheduled`。其中 `update_stats.webview_recovery_window_start_ms` / `webview_recovery_count`（第 154–156 行）与 `last_render_heartbeat_ms`（第 151 行）被本模块直接读写。
- `ExitStateStore`（`infra/state/exit.rs` 第 7–13 行）：`is_quitting`、`auto_exit_deadline`、`auto_exit_cancelled`、`campus_exit_started`、`campus_exit_deadline`，配套 `deadline()` / `set_deadline()` / `campus_exit_deadline()` / `set_campus_exit_deadline()`（第 32–46 行）。
- `CommandContext`（`infra/command_context.rs` 第 10–33 行）：`app` + `state` 引用，含 `Deref` 到 `AppState`；`AppHandleExt` trait（第 38–51 行）封装 `notify_config_changed` / `notify_update_download_progress`。
- `EventBus`（`infra/events.rs` 第 8–126 行）：统一事件名与 payload（如 `login-log`、`background-check-result`、`config-changed`、`auto-exit-countdown`、`campus-exit-countdown` 等）。

## Architecture

组件 ≥3 且流程非平凡，下面是启动—运行—退出的装配与事件流框图（文件/函数标注在后）。

```text
                 main.rs (preinit: panic hook / RTSS / WEBVIEW2 env / runtime)
                                    │
                                    ▼
   ┌──────────────────────────────────────────────────────────────────┐
   │ app/startup.rs  run() + setup_app()                                │
   │  - plugins: shell/notification/autostart/global_shortcut/single    │
   │  - .manage(AppState)  .setup(setup_app)  .on_window_event          │
   │  - invoke_handler[...所有命令...]                                   │
   └───────┬───────────────┬──────────────┬──────────────┬────────────┘
           │               │              │              │
           ▼               ▼              ▼              ▼
   ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌─────────────────┐
   │ app/tray.rs  │ │ app/window.rs│ │ app/shortcut │ │ app/heartbeat   │
   │ build_tray   │ │ show_or_     │ │ .rs          │ │ + window_safety │
   │ refresh_menu │ │ rebuild_main │ │ cancel-exit  │ │ 监测前端心跳    │
   └──────┬───────┘ └──────┬───────┘ └──────┬───────┘ └────────┬────────┘
          │                │                │                   │
          │                │                │                   ▼ 失联>20s
          │                │                │          app/webview_recovery.rs
          │                │                │          attempt_webview_recovery
          │                │                │          (reload 限流)
          │                │                │                   │
          ▼ 菜单/图标点击   │                ▼ 快捷键            │
   auth::service / cmds    │      infra/lifecycle.rs           ▼
   app/shutdown.graceful   │      cancel_auto/campus_exit     subscribe_process_failed
                           │      (ExitStateStore)            handle_process_failed
                           ▼                                   │ browser exit
                   app/shutdown.rs                              ▼
                   handle_window_close_event            attempt_app_restart (restart())
                   ├ lightweight_mode → lightweight.rs
                   │   (arm guard / capture_geometry)
                   ├ minimize_to_tray → window hide+suspend
                   └ else → graceful_exit → lifecycle.shutdown_and_exit
```

关键装配点（带行号）：

- 全局快捷键 handler 在 `startup.rs` 第 31–35 行通过 `tauri_plugin_global_shortcut::Builder::new().with_handler(...)` 注册，仅转发到 `shortcut::handle_shortcut_event`。
- `single_instance` 回调（第 36–48 行）仅在主窗口存在时 `show_or_rebuild_main`，否则延迟 2s 重建（处理 NSIS 自启窗口尚未创建的竞态）。
- 窗口事件分发（第 53–65 行）：`CloseRequested`→`shutdown::handle_window_close_event`；Windows 下 `Focused`→`window::handle_window_focus_event`、`Resized`→`window::handle_window_resized_event`。
- 退出拦截（第 144–158 行）：`RunEvent::ExitRequested` 只在「轻量化关闭导致的最后一个窗口销毁」且非主动退出时 `prevent_exit()`，托盘/关机不拦截。
- Windows 设备能力：`webview_recovery::subscribe_process_failed`（`startup.rs` 第 253–254 行调用）经 `ICoreWebView2_4` 订阅进程崩溃；`window::set_main_webview_suspended` 经 `ICoreWebView2_19` 控内存级别与可见性。

## Data Flow

以下描述三条核心数据流：

### 1. 启动初始化流（main.rs → startup.rs → 子模块）
1. `main.rs` 设 panic hook、`rtss_compat::preinit_detect`、`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`，构建 Tokio runtime（main.rs 第 21–77 行）。
2. `startup::run` 装配 Builder 并 `build(generate_context!)`（第 139 行）。
3. `setup_app` 在 `.setup` 中：解析日志目录→`logger::init_logger`→`load_config_from_disk_or_default`→`CommandContext::from_app`→`state.config.store`→`network::update_portal_url`→`set_log_retention_days`（startup.rs 第 162–221 行）。
4. 静默启动判定（`hidden_start && --autostart`→`window::mark_hidden_by_design(true)`，第 226–231 行）。
5. 后台设施启动：tray、adapter_watch、adapter_cache 刷新、update_check_loop、watcher 启动任务、webview_recovery 录版本+订阅 ProcessFailed、heartbeat 两线程、GPU 预热（第 233–269 行）。

### 2. 窗口事件 → 轻量化/退出流
1. 用户点关闭 → Tauri 发 `CloseRequested`。
2. `shutdown::handle_window_close_event`（shutdown.rs 第 20–61 行）按配置分流：
   - 轻量化：捕获几何（`lightweight::capture_geometry`）→ 置单次守卫（`arm_lightweight_exit_guard`）→ `set_lightweight_active(true)` + EcoQoS，不阻止关闭。
   - 最小化到托盘：`prevent_close` + `window::mark_hidden_by_design(true)` + `set_main_webview_suspended(true)` + `hide`。
   - 默认：`prevent_close` + `graceful_exit`。
3. 轻量化销毁导致最后一个窗口消失 → Tauri 发 `RunEvent::ExitRequested` → `startup::run` 回调取 `lightweight::take_lightweight_exit_guard()`（单次语义）+ `exit.is_quitting`，经 `should_prevent_exit` 决定是否 `prevent_exit()`（startup.rs 第 144–158 行；`lightweight.rs` 第 38–46 行）。
4. 重建路径：托盘/单击/single_instance 调 `window::show_or_rebuild_main` → 窗口不在则 `rebuild_main_window`（从 `tauri.conf.json` 的 main 配置新建，恢复几何）→ `lightweight::wait_window_ready_and_show`（轮询 5s 等前端 ready 信号）后 `show_and_focus_main`。

### 3. WebView2 崩溃恢复流（Windows）
1. 启动期 `webview_recovery::subscribe_process_failed` 订阅 `ICoreWebView2_4::add_ProcessFailed`（webview_recovery.rs 第 180–237 行）。
2. 进程崩溃回调 `handle_process_failed`（第 246–269 行）按 kind 分级：
   - 浏览器进程退出 → `attempt_app_restart`（`restart_guard_allow` 跨进程限流落盘 → `flush` → `app.restart`）。
   - 渲染/iframe 渲染进程退出 → `attempt_webview_recovery`（reload 滑动窗口限流后 `eval("window.location.reload()")`）。
   - 其余 → 仅记日志交运行时自愈。
3. 心跳路径：`heartbeat::spawn_heartbeat_thread` 监测 `last_render_heartbeat_ms`，失联 >20s 连续 3 次也调 `attempt_webview_recovery`，与 ProcessFailed 路径共用同一限流器（heartbeat.rs 第 43–53 行；webview_recovery.rs 第 58–114 行）。

### 4. 退出编排流（lifecycle）
`shutdown::graceful_exit` / `shortcut::handle_shortcut_event` / `lifecycle::start_auto_exit` / `start_campus_exit` 最终都经 `lifecycle::shutdown_and_exit`（infra/lifecycle.rs 第 319–328 行）：置 `is_quitting` → `task_manager.shutdown()`（10s 超时）→ `app_handle.exit(0)`。自动/校园网退出各持 `auto_exit_deadline` / `campus_exit_deadline` 互斥截止时间，统一快捷键 `CommandOrControl+Shift+C` 经 `shortcut.rs` 同时取消两者。

## Connections

本模块（钩子装配层）与下列仓库内其他模块紧密耦合，建议交叉阅读：

- [[infra-lifecycle]]（即 `infra/lifecycle.rs`）— 校园网退出、自动退出、统一 `shutdown_and_exit` 编排，被 `shutdown.rs`、`shortcut.rs` 直接调用；退出状态机（`ExitStateStore`）的权威所有者。
- [[infra-events]]（即 `infra/events.rs`）— `EventBus` 统一事件名与 payload，被 `tray.rs`、`lifecycle.rs` 用于向前端广播登录日志、退出倒计时、适配器变更等。
- [[infra-state]]（即 `infra/state/mod.rs` + `exit.rs`）— `AppState`、`UpdateStats`（心跳/`webview_recovery` 限流字段）、`ExitStateStore` 的定义与字段，本模块几乎所有跨线程标志都落在这里。
- [[infra-command-context]]（即 `infra/command_context.rs`）— `CommandContext` 与 `AppHandleExt`，`shutdown.rs`、`tray.rs` 借其取 `AppState` 与发事件。
- [[commands]]（即 `commands/*`）— `invoke_handler` 命令表（startup.rs 第 66–137 行）落地的实现层；托盘快速登录/注销、切换账号复用 `commands::login::perform_full_logout_sync`、`commands::account::perform_switch_account_sync`、`commands::config_cmd::*` 等。
- [[config]]（即 `config/*`）— `Config` 模型与持久化；轻量化/最小化到托盘/静默启动等行为都读 `config.lightweight_mode` / `minimize_to_tray` / `hidden_start` 等。
- [[monitor]]（即 `monitor/*`）— `adapter_watch::start_adapter_watch`、`watcher::run_startup_tasks` 在 `setup_app` 中启动。
- [[network]]（即 `network/*`）— `update_portal_url`、`adapter_cache::start_cache_refresh_task` 在启动期调用。
- [[update]]（即 `update/*`）— `updater::start_update_check_loop` / `cleanup_stale_update_dir` 在启动期调用。
- [[auth]]（即 `auth/*`）— 托盘快速登录走 `auth::service::full_login` / `post_login_handler`。
- [[platform]]（即 `platform/*`）— `rtss_compat`（RTSS 检测，main.rs 与 startup.rs 引用）、`gpu`（WebVIEW2 env、GPU 预热）、`ecoqos`（轻量化效率模式）等平台设备能力。
- [[infra-logger]]（即 `infra/logger.rs`）— `init_logger`、`set_log_retention_days`、`flush`、`set_debug_mode` 均在本模块启动/运行期调用。

## Known Issues

1. **`TrySuspend` 禁用（永久）** — `app/window.rs` 第 26–37 行文档明确：v2.4.2 曾引入 `ICoreWebView2_3::TrySuspend`/`Resume`，会把渲染进程整体冻结导致 React 调度器死锁（`pendingLanes` 有待办、`callbackNode` 永空），表现为按钮/切页失效、仅合成器滚动可用。本模块**只用 `SetIsVisible`**，并有回归护栏 `window.rs` 第 265–271 行的 `never_calls_try_suspend` 测试（`include_str!("window.rs")` 断言不含 `TrySuspend(`）。

2. **RTSS hook 注入必须在 main 极早期** — `startup.rs` 第 200–205 行注释：RTSS 冲突处理提前到 `main.rs` 第 52–53 行（`rtss_compat::preinit_detect`，在 WebView2 环境创建之前）；放在 `.setup` 已太晚（浏览器进程已创建，3/6 个 WebView2 进程已被注入）。`setup_app` 仅调用 `rtss_compat::log_preinit_outcome` 留痕。

3. **历史默认退出缺陷已修** — `shutdown.rs` 第 53–56 行注释：旧实现直接 `graceful_exit` 而不 `prevent_close`，窗口关闭成为最后一个窗口会立即终止 run loop，异步排空被中断（DHCP 子进程/登录请求/日志刷盘被截断）。现改为先 `prevent_close()`，待后台任务排空后再 `exit(0)`。

4. **`campus_exit`/`auto_exit` 死锁与卡死风险（已用 CAS + 回滚修复）** — `infra/lifecycle.rs` 第 45–54、137–142、267–270 行：原实现「先置 deadline 再 CAS」会让周期性重触发的第二次把 deadline 推后、CAS 失败 return 后运行任务校验发现 deadline 未到期而 return，`campus_exit_started` 永久为 true，退出流程卡死。改为 CAS 成功后再 `set_deadline`；spawn 失败时回滚 `campus_exit_started` / `set_campus_exit_deadline` / `set_deadline`，避免后续同名校验任务永远短路。

5. **轻量化单次守卫语义** — `lightweight.rs` 第 11–14、38–46 行：`EXPECT_LIGHTWEIGHT_EXIT` 用 `swap(false, AcqRel)` 单次消费；若用长活标志会连同系统关机一起拦截，故必须单次语义（参考 SwitchHosts `EXPECT_LIGHTWEIGHT_EXIT`）。

6. **`auto_exit_cancelled` 重置时机** — `infra/lifecycle.rs` 第 202 行：`start_auto_exit` 在持锁置 deadline 时会 `store(false)` 重置取消标志，确保下次 `do_login` 之前用户取消不再阻止重新触发。

7. **`RECOVERY_WINDOW_MS`/`RESTART_GUARD` 限流边界** — `webview_recovery.rs` 第 27–35、80–88 行：reload 滑动窗口 5 分钟内最多 3 次，超限只留 ERROR 日志不自动重启；应用重启防护 10 分钟内最多 2 次且计数**落盘**（`webview_restart_guard` 文件），否则必现故障会因内存限流归零而无限重启闪屏。

8. **非 Windows 平台空实现** — `window::handle_window_focus_event` / `handle_window_resized_event` 及 `subscribe_process_failed`/`describe_*` 等均有 `#[cfg(not(target_os = "windows"))]` 空实现或 `#[cfg(target_os = "windows")]` 限定，移动端（lib.rs 第 12–21 行 `app` 模块 `#[cfg(desktop)]` 不编译）完全没有这些钩子。

9. **静默启动强拉缺陷（已规避）** — `window.rs` 第 4–7 行、`heartbeat.rs` 第 75–79 行：启动兜底显示线程与前端 `show_window` 须靠 `is_hidden_by_design` 区分「该显示却没显示」与「用户就是要隐藏」，否则 v2.4.2 会把静默/托盘隐藏窗口强拉回前台。

10. **日志目录可写探测** — `startup.rs` 第 173–186 行：Program Files 下安装目录日志不可写（writer=None 静默丢日志），须先探测安装目录 `logs/` 可写，失败回退到用户数据目录 `data_dir/logs`。
