---
title: app_entry
type: module
source_files:
  - tauri-app/src-tauri/src/main.rs
  - tauri-app/src-tauri/src/lib.rs
  - tauri-app/src-tauri/src/app/mod.rs
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/app/webview_recovery.rs
  - tauri-app/src-tauri/src/app/lightweight.rs
  - tauri-app/src-tauri/src/app/window.rs
  - tauri-app/src-tauri/src/app/shutdown.rs
  - tauri-app/src-tauri/src/app/heartbeat.rs
  - tauri-app/src-tauri/src/app/tray.rs
  - tauri-app/src-tauri/src/app/shortcut.rs
  - tauri-app/src-tauri/src/infra/state/mod.rs
  - tauri-app/src-tauri/src/infra/state/exit.rs
  - tauri-app/src-tauri/src/infra/logger.rs
  - tauri-app/src-tauri/src/infra/lifecycle.rs
  - tauri-app/src-tauri/src/platform/gpu.rs
  - tauri-app/src-tauri/src/platform/rtss_compat.rs
  - tauri-app/src-tauri/src/helper/mod.rs
tags: [桌面端, 应用入口, 全局初始化, 崩溃恢复, 轻量化, 托盘, 生命周期]
---

## Overview

`app_entry` 模块负责桌面端 Tauri 应用的**进程入口、全局初始化与崩溃恢复**三大职责。代码分布在两个层面：

- **进程入口层**：`main.rs`（`tauri-app/src-tauri/src/main.rs`）是 `cfg(desktop)` 下唯一的二进制入口（见 `lib.rs` 第 12–21 行的 `#[cfg(desktop)] pub mod app` 等声明）。它做的第一件事是安装 panic hook、拦截 `--helper` / `--helper-task` 提权模式、在 WebView2 环境创建前做 RTSS 冲突检测与浏览器参数注入，最后构建 Tokio runtime 并交棒给 `app::startup`。
- **初始化与恢复层**：`src/app/` 目录下的子模块在 `app::startup::run`（`tauri-app/src-tauri/src/app/startup.rs`）中逐一装配——Tauri Builder 注册插件/命令/窗口事件、`.setup()` 里完成数据目录探测、日志初始化、配置加载、托盘构建、后台任务启动、WebView2 运行时版本记录与进程崩溃订阅、心跳与窗口兜底线程。

**崩溃恢复**是本模块的核心关注点，集中在 `app/webview_recovery.rs`：
- WebView2 渲染/浏览器进程崩溃原本只能靠前端心跳超时（20–35s）推断，且 `eval reload` 对「浏览器进程已退出（WebView 进入 Closed）」无效。模块通过 `webview2-com` 直接订阅 `ICoreWebView2_4::add_ProcessFailed`，按 `Kind + Reason + ExitCode` 分级处理：浏览器进程退出 → 应用自重启（`attempt_app_restart`，带跨进程限流防闪屏循环）；渲染进程退出 → reload（`attempt_webview_recovery`，带滑动窗口限流）；其余进程交给运行时自愈只记日志。
- **轻量化模式**（`app/lightweight.rs` + `app/shutdown.rs` + `app/window.rs`）是另一类「生存态」恢复：关闭按钮销毁 WebView 仅留后端与托盘，下次唤起重建窗口并恢复几何。

模块整体遵循「**早拦截、先日志、后 UI、事件驱动恢复、限流防抖**」的设计哲学，所有恢复动作均带限流，避免「必现故障 → 无限重启/空转」。

## Key Components

### `main.rs`（进程二进制入口）

- `main()` — `tauri-app/src-tauri/src/main.rs:17`：进程总入口。
  - 第 21–25 行：安装 panic hook，panic 时写 `log_error!("panic", ...)`、调用 `flush_quick()`（500ms 超时，避免 `panic=abort` 下阻塞 5s 终止）、`eprintln`。
  - 第 27–33 行：若含 `--helper-task` 参数，调用 `helper::run_helper_task()` 并以退出码 `exit()`（计划任务 worker 模式，优先于正常流程）。
  - 第 38–47 行：`helper::parse_helper_args` 解析 `--helper <op> --result <file>`；命中且合法则 `helper::run_helper` 后退出；解析失败 `exit(2)`；参数非法时**不进入正常应用流程**（防止 `--helper` 误传拉起 UI）。
  - 第 52–53 行：`platform::rtss_compat::preinit_detect()`，在 WebView2 环境创建前检测 RTSS hook 注入风险（结果暂存，待 logger 就绪后留痕）。
  - 第 57–69 行：`platform::gpu::build_browser_args()` 构造浏览器参数，并在 `data_dir/com.campus.login/crashdumps` 开启 WebView2 Crashpad 转储（`--enable-crash-reporter --crash-dumps-dir=...`），经 `std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", ...)` 先于 Tokio runtime 设置（注释说明 `set_var` 与 worker 线程并发读 env 的竞态）。
  - 第 75–81 行：构建 runtime（`build_runtime`）、设全局 async handle、调 `app::startup::run`，结束后 `flush()` / `shutdown()` 日志并 `runtime.shutdown_timeout(5s)`。

### `app/startup.rs`（Tauri 装配与 setup）

- `build_runtime(core_count: usize)` — `startup.rs:6`：多线 runtime，`worker_threads = clamp(2,8)`、`max_blocking_threads = clamp(8,64)`、线程名 `campus-worker`、失败 `exit(1)`。
- `run(core_count: usize)` — `startup.rs:23`：组装 `tauri::Builder`，注册插件（`tauri_plugin_shell` / `notification` / `autostart` / `global_shortcut` 含 `app::shortcut::handle_shortcut_event` / `single_instance` 含 `app::window::show_or_rebuild_main` 二次唤起）；`.manage(AppState::new())`；`.setup(setup_app)`（第 50–52 行）；`.on_window_event`（CloseRequested→`shutdown::handle_window_close_event`、Windows 下 Focused→`window::handle_window_focus_event`、Resized→`window::handle_window_resized_event`）；`generate_handler!` 注册全部命令（第 66–137 行）；`build()` 失败 `exit(1)`；`app.run` 的 `ExitRequested` 回调里消费轻量化退出守卫（第 144–158 行，引用 `lightweight::take_lightweight_exit_guard` / `should_prevent_exit` 与 `state.exit.is_quitting`）。
- `setup_app(app, core_count)` — `startup.rs:161`：数据目录探测（第 162–167 行）；**日志目录选择**（第 169–186 行：优先安装目录 `logs`，用 `.log_probe` 探测可写，失败回退 `data_dir/logs`）；先 `logger::init_logger` 再 `config_cmd::load_config_from_disk_or_default`（第 190–195 行）；RTSS 结果留痕 `rtss_compat::log_preinit_outcome`（第 205 行）与浏览器参数补记（第 207–211 行）；`CommandContext::from_app` 取 state、`state.config.store`、`network::update_portal_url`（第 213–216 行）；`logger::set_log_retention_days` 应用日志保留天数（第 221 行，修复「仅保存不生效」历史缺陷）；**静默启动**判定 `--autostart` + `config.hidden_start`（第 226–231 行，调 `window::mark_hidden_by_design(true)`）；`tray::build_tray`（第 233 行）；启动适配器监听、缓存刷新、更新检查循环、启动残留包清理、启动期后台任务（第 236–248 行）；WebView2 运行时版本记录 `webview_recovery::record_webview2_runtime_version` 与 Windows 下 `subscribe_process_failed`（第 252–254 行）；心跳线程 `heartbeat::spawn_heartbeat_thread` / `spawn_window_safety_thread`（第 256–257 行）；GPU 预热后台线程（第 261–269 行，调 `gpu::detect_gpu_info` / `detect_display_refresh_rate`）。

### `app/webview_recovery.rs`（WebView2 进程崩溃订阅与恢复）

- 常量：`RECOVERY_WINDOW_MS=300_000` / `RECOVERY_MAX_PER_WINDOW=3`（第 28–29 行，reload 滑动窗口）；`RESTART_GUARD_WINDOW_MS=600_000` / `RESTART_GUARD_MAX=2` / `RESTART_GUARD_FILE="webview_restart_guard"`（第 33–35 行，跨进程重启限流）。
- `recovery_gate(window_start_ms, count, now_ms)` — `webview_recovery.rs:39`：纯函数限流判定，返回 `(allowed, new_start, new_count)`，窗口过期即重置计数。
- `attempt_webview_recovery(app, reason)` — `webview_recovery.rs:58`：reload 唯一入口，读 `state.update_stats.webview_recovery_*` 原子量限流，限流超限则只记 ERROR；`window.eval("window.location.reload()")` 失败即记 ERROR 证据。
- `restart_guard_decide(timestamps, now_ms)` — `webview_recovery.rs:118`：纯函数，窗口外旧记录丢弃，放行时追加 `now_ms`。
- `restart_guard_read(path)` / `restart_guard_allow(app, now_ms)` — `webview_recovery.rs:132` / `:140`：读/写 `app_data_dir/webview_restart_guard` 落盘（读写失败按放行，宁可多试一次）。
- `attempt_app_restart(app, reason)` — `webview_recovery.rs:157`：浏览器进程退出专用，先 `restart_guard_allow` 限流，超限记 ERROR 停重启；放行则 `flush()` 后 `app.restart()`。
- `subscribe_process_failed(app)` — `webview_recovery.rs:180`（仅 Windows）：`with_webview` 经 `ICoreWebView2_4::add_ProcessFailed` 订阅，回调取 `Kind + Reason + ExitCode`（v2 接口）后调 `handle_process_failed`。
- `handle_process_failed(app, kind, reason, exit_code)` — `webview_recovery.rs:246`：分级——`BROWSER_PROCESS_EXITED`→`attempt_app_restart`；`RENDER_PROCESS_EXITED`/`FRAME_RENDER_PROCESS_EXITED`→`attempt_webview_recovery`；其余记 WARN 交运行时自愈。
- `describe_process_failed_kind/reason` — `webview_recovery.rs:321` / `:290`：把枚举转语义文本（Reason 区分 CRASHED/OUT_OF_MEMORY/TERMINATED/…）。
- `record_webview2_runtime_version()` — `webview_recovery.rs:363`：从 EdgeUpdate 注册表（HKLM WOW6432Node / HKLM / HKCU）或 `C:\Program Files (x86)\Microsoft\EdgeWebView\Application` 安装目录回退读取 WebView2 运行时版本。
- `pick_runtime_version(entries)` — `webview_recovery.rs:420`：从目录条目选最高四段版本号。

### `app/lightweight.rs`（轻量化运行态）

- 静态量：`LIGHTWEIGHT_ACTIVE: AtomicBool`（第 10 行）、`EXPECT_LIGHTWEIGHT_EXIT: AtomicBool`（单次退出守卫，第 14 行）、`GEOMETRY: Mutex<Option<WindowGeometry>>`（第 16 行）、`READY_SIGNALS: Mutex<Vec<(String,u64)>>`（第 87 行）。
- `struct WindowGeometry { x, y, width, height: f64 }` — `lightweight.rs:18`：销毁前记录的窗口几何，重建时应用。
- `is_lightweight_active`/`set_lightweight_active`（第 26/30 行）、`arm_lightweight_exit_guard`/`take_lightweight_exit_guard`（单次语义 `swap`，第 34/38 行）、`should_prevent_exit(expecting, is_quitting) -> expecting && !is_quitting`（第 44 行纯函数）、`capture_geometry`（第 48 行）、`take_geometry`（第 63 行）、`geometry_is_on_screen`（第 68 行纯函数）、`effective_background_interval_ms`/`effective_quality_interval_ms`（第 77/82 行纯函数，轻量化下限 300s/1800s）、`signal_window_ready`/`take_window_ready`（第 89/99 行）、`wait_window_ready_and_show(app, label)`（第 112 行，等 ready 信号或 5s 超时后 `window::show_and_focus_main`）。

### `app/window.rs`（窗口显示/挂起/重建）

- `HIDDEN_BY_DESIGN: AtomicBool`（第 8 行）：应用自身隐藏标记（隐藏到托盘/静默启动）。`mark_hidden_by_design`/`is_hidden_by_design`（第 10/14 行）、`is_user_dismissed(window)`（第 22 行：`is_minimized || is_hidden_by_design`）。
- `set_main_webview_suspended(app, suspended)` — `window.rs:38`：**只用 `controller.SetIsVisible`，绝不 `TrySuspend`**（注释详解 v2.4.2 缺陷：`TrySuspend` 冻结渲染进程使 React 调度器死锁）。`reset_suspend_state_for_new_webview`（第 53 行，重置 `LAST_MINIMIZED`）、`apply_webview_suspended`（第 59 行，Windows，设 `ICoreWebView2_19::SetMemoryUsageTargetLevel` + `SetIsVisible`）。
- `handle_window_focus_event`（第 92 行，焦点变化时调内存级别）、`handle_window_resized_event`（第 140 行，最小化翻转时去抖同步挂起）、`note_minimized_synced`（第 163 行）、`LAST_MINIMIZED: AtomicBool`（第 129 行）。
- `show_and_focus_main<M: Manager>`（第 178 行：清除 `HIDDEN_BY_DESIGN`→`unminimize`→`show`→恢复挂起→`set_focus`）、`show_or_rebuild_main(app)`（第 195 行：窗口在则显示、不在则 `rebuild_main_window`）、`rebuild_main_window(app)`（第 209 行：从 `tauri.conf.json` 的 `main` 窗口配置重建，`visible(false)` 起步，恢复几何（离屏则回退居中），退出轻量化态，起 ready 门等待 `notify_window_ready` 或 5s 超时）。

### `app/shutdown.rs`（优雅退出与关闭事件）

- `graceful_exit(app_handle, _state)` — `shutdown.rs:8`：`spawn` 调 `lifecycle::shutdown_and_exit`。
- `handle_window_close_event(window, event)` — `shutdown.rs:20`：三分支——`lightweight_mode` 开启：捕获几何→`arm_lightweight_exit_guard`→`set_lightweight_active(true)`→Windows 下开 EcoQoS，不 `prevent_close`（让 Tauri 真销毁，由 `startup::run` 的 ExitRequested 守卫拦截退出）；`minimize_to_tray`：`prevent_close` + 挂起 + `hide`；否则：`prevent_close` + `graceful_exit`（修复「直接退出导致后台任务未排空」历史缺陷）。

### `app/heartbeat.rs`（心跳与窗口兜底）

- `spawn_heartbeat_thread(app_handle)` — `heartbeat.rs:9`：每 5s 检测前端心跳（`update_stats.last_render_heartbeat_ms`），丢失 >20s 连续 3 次调 `webview_recovery::attempt_webview_recovery`；最小化/不可见窗口排除（避免误触发）。
- `spawn_window_safety_thread(app_handle)` — `heartbeat.rs:66`：启动 3s 后检查窗口可见性，不可见且非 `is_user_dismissed` 则 `show_and_focus_main`，最多 3 次（防 WebView2 初始化时序黑屏，但不反驳用户隐藏意图）。

### `app/tray.rs`（托盘构建与事件）

- 常量 `TRAY_ID="main-tray"`（第 11 行）、`SWITCH_ITEM_PREFIX="switch-account:"`（第 14 行）。
- `build_tray_menu(app)` — `tray.rs:23`：构建菜单（显示主窗口 / 快速登录 / 快速注销[仅配置账号时启用] / 切换账号子菜单 / 退出）。
- `refresh_tray_menu_state(app)` — `tray.rs:75`：`spawn_blocking` 重建并应用菜单（配置变化后保持同步）。
- `build_tray(app, install_dir)` — `tray.rs:91`：构建并注册托盘图标（默认图标→安装目录 `icons/icon.ico`→内嵌 `icon.ico` 兜底），注册菜单与图标点击事件。
- `handle_tray_menu_event`（第 121 行：show→`window::show_or_rebuild_main`、quick-login→`auth::service::full_login`、quick-logout→`commands::login::perform_full_logout_sync`、switch→`commands::account::perform_switch_account_sync`、quit→`shutdown::graceful_exit`）、`handle_tray_icon_event`（第 213 行：左键单击→`show_or_rebuild_main`）。

### `app/shortcut.rs`（全局快捷键）

- `CANCEL_KEY: OnceLock<Option<Shortcut>>`（第 6 行，解析缓存）。
- `handle_shortcut_event(app, shortcut, event)` — `shortcut.rs:9`：仅处理 `Pressed`，解析 `CANCEL_EXIT_SHORTCUT="CommandOrControl+Shift+C"`，命中则 `spawn_blocking` 调 `lifecycle::cancel_auto_exit_inner` + `cancel_campus_exit_with_notification`。

### 支撑模块（非本模块但被直接调用，行号备查）

- `infra/state/mod.rs`：`AppState`（第 184 行）含 `config`/`tasks`/`task_manager`/`network`/`exit`/`update_stats`/`scheduled`；`UpdateStats`（第 147 行）含 `webview_recovery_window_start_ms`/`webview_recovery_count` 等恢复限流原子量；`AUTO_EXIT_DELAY_MS=20000`（第 13 行）、`CANCEL_EXIT_SHORTCUT`（第 14 行）。
- `infra/state/exit.rs`：`ExitStateStore`（第 7 行）含 `is_quitting: Arc<AtomicBool>`、`auto_exit_deadline`/`campus_exit_deadline: Mutex<Option<Instant>>`、`auto_exit_cancelled`/`campus_exit_started: AtomicBool`；`deadline()`/`set_deadline()`/`campus_exit_deadline()`/`set_campus_exit_deadline()` 方法。
- `infra/logger.rs`：`init_logger(log_dir)`（第 82 行，有界通道 + worker 线程）、`set_log_retention_days`（第 29 行，应用即清理）、`flush_quick()`（第 501 行，500ms 超时，panic hook 专用）、`flush()`（第 489 行）、`shutdown()`（第 511 行）、`LOG_CHANNEL_CAPACITY=8192`（第 17 行）。
- `infra/lifecycle.rs`：`shutdown_and_exit(app, state)`（第 319 行，置 `is_quitting`、10s 超时等待后台任务清空、`exit(0)`）、`start_campus_exit`/`cancel_*`、`start_auto_exit`/`cancel_auto_exit_inner`（自动退出 20s 倒计时 + `Ctrl+Shift+C` 取消）。
- `platform/gpu.rs`：`build_browser_args()`（第 236 行，返回 `--js-flags=--max-old-space-size=512`）、`detect_gpu_info()`（第 232 行）、`detect_display_refresh_rate()`（第 250 行）。
- `platform/rtss_compat.rs`：`preinit_detect()`（第 68 行）、`log_preinit_outcome()`（第 73 行）、`RtssOutcome` enum（第 23 行）、`profiles_dir_candidates()`（第 31 行）。
- `helper/mod.rs`：`parse_helper_args`（第 260 行）、`run_helper`（第 292 行）、`run_helper_task()`（第 350 行，计划任务 worker）、`HelperOp` enum（第 116 行）。

## Architecture

```text
┌──────────────────────┐   ┌─────────────────────────┐
│ main.rs: main()      │──►│ startup.rs: setup_app   │
│ panic hook/--helper  │   │ 目录→logger→config→tray │
│ 拦截/RTSS预检/参数注入│   │ →adapter_watch/update   │
│ →build_runtime→run   │   │ →webview_recovery→hb   │
└──────────────────────┘   └───────────┬─────────────┘
                                       │
            ┌──────────────────────────┼──────────────────────────┐
            ▼                          ▼                          ▼
┌──────────────────┐   ┌───────────────────────┐   ┌─────────────────────────┐
│ tray.rs 菜单/唤起 │   │ shutdown.rs 关闭三分支 │   │ webview_recovery.rs     │
│ show/quick/login │   │ 轻量化→守卫/最小化/退出│   │ ProcessFailed→分级恢复  │
└────────┬─────────┘   └───────────┬───────────┘   └────────────┬────────────┘
         │                         │                            │
         ▼                         ▼                            ▼
┌─────────────────────────────────────────────┐   ┌────────────────────────────┐
│ lightweight.rs + window.rs                  │   │ heartbeat.rs 心跳丢失→     │
│ 显示/挂起/重建 + 退出守卫/几何缓存           │   │ attempt_webview_recovery   │
└──────────────────────┬──────────────────────┘   └────────────┬───────────────┘
                       ▼                                       ▼
              ┌────────────────────────────────────────────────────┐
              │ AppState/UpdateStats (恢复限流原子量) + ExitStateStore │
              │ restart_guard 落盘文件 webview_restart_guard          │
              └────────────────────────────────────────────────────┘
```

组件 ≥3 且流程非平凡（入口早拦截 → 装配 → 事件驱动恢复 → 限流防抖），故以框图表示。关键关系：
- `main.rs` 的 `--helper` 拦截**优先于** Tauri Builder 装配，避免提权参数误触 UI（见 `main.rs:31-47`）。
- `app::shutdown::handle_window_close_event` 在轻量化分支**不** `prevent_close`，而是置 `EXPECT_LIGHTWEIGHT_EXIT` 守卫；真正的退出拦截发生在 `startup::run` 的 `ExitRequested` 回调（`startup.rs:144-158`），两者通过单次语义守卫衔接。
- 两条恢复路径（心跳超时 `heartbeat.rs:47` 与 `ProcessFailed` 事件 `webview_recovery.rs:261`）**共用同一 reload 限流器**（`state.update_stats.webview_recovery_*`），避免叠加空转。

## Data Flow

### 启动数据流（冷启动）

```text
std::env::args
  └─ main.rs: main()
       ├─ panic hook 安装
       ├─ --helper-task? → helper::run_helper_task() → exit(码)
       ├─ --helper <op>?  → helper::run_helper()      → exit(0/1/2)
       ├─ rtss_compat::preinit_detect()          // 结果存 PREINIT_OUTCOME
       ├─ gpu::build_browser_args() + crashdumps  → set_var(WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS)
       ├─ build_runtime(core_count)              // startup.rs:6
       └─ startup::run() → tauri::Builder 装配 → app.build() → app.run()
                                │
                                ▼ .setup(setup_app)
           数据目录 → logger::init_logger(log_dir)
                       └─ load_config_from_disk_or_default → state.config.store
            rtss_compat::log_preinit_outcome()    // 把 preinit 结果留痕
            set_log_retention_days(config.log_retention_days)
            静默启动判定(--autostart && hidden_start) → mark_hidden_by_design(true)
            tray::build_tray() → adapter_watch / update_check / startup_tasks
            webview_recovery::record_webview2_runtime_version()
            [Win] subscribe_process_failed() → add_ProcessFailed 订阅
            heartbeat::spawn_* / gpu-warmup 线程
```

### WebView2 进程崩溃恢复数据流（事件驱动）

```text
WebView2 进程崩溃
  └─ ICoreWebView2_4::ProcessFailed 回调 (webview_recovery.rs:210)
       ├─ kind=BROWSER_PROCESS_EXITED → handle_process_failed → attempt_app_restart
       │     └─ restart_guard_allow (读/写 webview_restart_guard 落盘)
       │           ├─ 放行 → logger::flush() → app.restart()
       │           └─ 限流(10min/2次) → 记 ERROR 停重启
       ├─ kind=RENDER/FRAME_RENDER_PROCESS_EXITED → attempt_webview_recovery
       │     └─ recovery_gate (state.update_stats.webview_recovery_*) 限流(5min/3次)
       │           ├─ 放行 → window.eval("window.location.reload()")
       │           └─ 限流 → 记 ERROR 停 reload
       └─ 其他(GPU/Utility/PPAPI) → 记 WARN，交运行时自愈
```

### 轻量化模式数据流（关闭→重建）

```text
窗口关闭 (shutdown::handle_window_close_event, lightweight_mode)
  ├─ capture_geometry() → GEOMETRY 缓存
  ├─ arm_lightweight_exit_guard()        // 单次守卫置位
  ├─ set_lightweight_active(true) + Win EcoQoS
  └─ 不 prevent_close → Tauri 真销毁窗口 → 成为最后一窗 → ExitRequested
        └─ startup::run 回调: take_lightweight_exit_guard && !is_quitting
              → api.prevent_exit()  // 进程常驻，仅后端+托盘

托盘/左键单击/二次启动 (show_or_rebuild_main)
  ├─ 窗口存在 → show_and_focus_main
  └─ 窗口不存在(已销毁) → rebuild_main_window
        ├─ take_geometry() + 几何在屏校验 → 重建 visible(false) 窗口
        ├─ reset_suspend_state_for_new_webview()
        ├─ set_lightweight_active(false) + 关 EcoQoS
        └─ wait_window_ready_and_show (前端 notify_window_ready 或 5s 超时)
```

## Connections

- [[config]] — `setup_app` 经 `config_cmd::load_config_from_disk_or_default` 加载 `Config`，并用 `config.hidden_start`/`config.lightweight_mode`/`config.minimize_to_tray`/`config.log_retention_days`/`config.portal_url` 等字段驱动初始化与恢复行为。
- [[commands]] — `startup.rs:66-137` 的 `generate_handler!` 注册全部 `commands::*` 命令（config/login/network/account/background/system/updater/self_service/vpn）。
- [[infra]] — 日志（`infra/logger`）、状态（`infra/state`）、生命周期（`infra/lifecycle::shutdown_and_exit`）、事件总线（`infra/events`）、通知（`infra/notification`）是本模块的底座。
- [[monitor]] — `setup_app` 启动 `monitor::adapter_watch::start_adapter_watch`、`monitor::watcher::run_startup_tasks`（后台检测/质量/延迟/自动登录）。
- [[network]] — `setup_app` 调 `network::update_portal_url`、启动 `network::adapter_cache::start_cache_refresh_task`；`helper::run_helper` 内调 `network::dns_setup`/`dhcp` 等提权操作。
- [[account]] — `tray.rs` 菜单经 `config::persist::list_account_items` 列账号，`commands::account::perform_switch_account_sync` 切换。
- [[auth]] — 托盘「快速登录」经 `auth::service::full_login` 登录、`post_login_handler` 收尾。
- [[update]] — `setup_app` 启动 `update::updater::start_update_check_loop` 与 `cleanup_stale_update_dir`。
- [[platform]] — `platform::gpu`（浏览器参数/GPU 预热）、`platform::rtss_compat`（冲突检测）、`platform::ecoqos`（轻量化效率模式）被本模块在入口与 setup 调用。
- [[helper]] — `--helper` / `--helper-task` 提权模式由 `main.rs` 极早期拦截后交由 `helper` 模块执行，绝不进入正常应用流程。
- [[self_service]] / [[vpn]] — 命令经 `generate_handler!` 注册，托盘/快捷键间接触发其底层逻辑。

## Known Issues

1. **RTSS 排除配置实测无效**（`platform/rtss_compat.rs:7-13`）：向 RTSS `Profiles` 目录写入排除 profile 被 RTSS 7.3.7 忽略，模块只做检测与日志告知（`log_preinit_outcome`），要求用户手动在 RTSS 界面排除。无代码层面修复。
2. **`TrySuspend` 被列为永久禁用**（回归护栏 `window.rs:266-271` 的 `never_calls_try_suspend` 测试）：`ICoreWebView2_3::TrySuspend` 会冻结渲染进程导致 React 调度器死锁（按钮/切页失效、仅合成器滚动可用）。`set_main_webview_suspended` 只用 `controller.SetIsVisible`（`window.rs:266` 文档详述）。
3. **安装目录日志不可写**（非缺陷但需注意，`startup.rs:173-186`）：per-machine 安装到 Program Files 时安装目录 `logs` 不可写，writer 静默为 `None`，自动回退到用户数据目录 `data_dir/logs`。
4. **`logRetentionDays` 历史缺陷已修复但仅启动应用**（`startup.rs:218-221`）：旧版仅保存不生效，现已在启动时 `set_log_retention_days` 应用即清理；若用户在面板改后重启前未触发，仍依赖本启动接线。
5. **helper 误传 `--helper` 会被当作非法退出**（`main.rs:43-46`）：`parse_helper_args` 解析失败 `exit(2)`，且 `Ok(None)` 才会继续正常流程，确保非法提权参数不拉起 UI。
6. **轻量化重建依赖前端 `notify_window_ready`**（无显式禁用但需注意，`window.rs:251-256`）：新 WebView 以 `visible(false)` 起步，靠 5s 超时兜底显示；若该前端事件丢失，最长 5s 黑窗。
7. **Crashpad 转储默认禁用**（`main.rs:58-68`）：`--enable-crash-reporter` 转储默认关，`crashdumps` 目录多数为空；开启后浏览器崩溃落盘 minidump 与 `ProcessFailed` 的 Reason/ExitCode 组成诊断链。
