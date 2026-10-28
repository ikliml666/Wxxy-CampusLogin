---
title: webview
type: module
source_files:
  - tauri-app/src-tauri/src/app/webview_recovery.rs
  - tauri-app/src-tauri/src/app/window.rs
  - tauri-app/src-tauri/src/app/heartbeat.rs
  - tauri-app/src-tauri/src/app/lightweight.rs
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/app/shutdown.rs
  - tauri-app/src-tauri/src/app/tray.rs
  - tauri-app/src-tauri/src/main.rs
  - tauri-app/src-tauri/src/commands/system.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
  - tauri-app/src-tauri/src/platform/gpu.rs
  - tauri-app/src-tauri/src/platform/ecoqos.rs
  - tauri-app/src-tauri/src/infra/state/mod.rs
  - tauri-app/src-tauri/tauri.conf.json
tags: [webview2, tauri, windows, crash-recovery, suspension]
---

## Overview

本模块负责桌面端 Tauri 2 应用中 **WebView2（Windows）组件的生命周期治理**：崩溃恢复、挂起/恢复、最小化/隐藏到托盘、轻量化销毁与重建、以及启动期浏览器参数与运行时版本诊断。它不实现 WebView 本身的渲染，而是围绕 Tauri 的 `WebviewWindow`（标签恒为 `"main"`，见 `tauri.conf.json` 第 13–30 行 `app.windows[0]` 配置）与底层 `webview2-com` / `webview2_com_sys` COM 接口做"宿主侧"操作。

核心文件与职责：

- `app/webview_recovery.rs`：WebView2 `ProcessFailed` 事件订阅与分级恢复。Tauri 2 在 Windows 下不暴露该事件（官方 PR #15162 仅 Apple 平台等价），故本模块经 `webview2-com` 直接订阅 `ICoreWebView2_4::add_ProcessFailed`，按崩溃种类 + Reason/ExitCode 记日志并立即走恢复入口（比心跳超时推断快 20–35s）。提供 reload 路径（`attempt_webview_recovery`，仅 WebView 仍有效时）与重启路径（`attempt_app_restart`，浏览器进程退出、WebView 已 Closed 专用）。
- `app/window.rs`：主 WebView 的挂起/恢复、最小化/还原可见性同步、窗口兜底显示与轻量化重建。关键约束：**只用 `controller.SetIsVisible`，绝不调用 `ICoreWebView2_3::TrySuspend`**（v2.4.2 缺陷根因）。
- `app/heartbeat.rs`：前端心跳监控线程（丢失则 reload）与启动期窗口兜底显示线程（防黑屏）。
- `app/lightweight.rs`：轻量化运行态（几何缓存、ready 门、退出守卫）。
- `app/startup.rs` / `main.rs`：装配 WebView 相关钩子、设置浏览器参数、记录运行时版本、订阅崩溃事件。
- `commands/system.rs` / `commands/config_cmd.rs`：前端 IPC 命令（`minimize_window` / `close_window` / `show_window` / `notify_window_ready` / `render_heartbeat`）桥接上述逻辑。

> 注：android 端（`android/src-tauri`）为独立复刻后端，浏览器为系统 WebView/WebView2 之外实现，与本模块无共享代码（见 AGENTS.md 第 11 行双端约定）。本模块函数绝大多数以 `#[cfg(target_os = "windows")]` 或非 Windows 空实现门控。

## Key Components

### 常量（app/webview_recovery.rs）

- `RECOVERY_WINDOW_MS` (第 28 行)：reload 恢复滑动窗口 5 分钟（`5 * 60 * 1000`）。
- `RECOVERY_MAX_PER_WINDOW` (第 29 行)：窗口内最多 3 次 reload，超限即停止自动恢复避免空转。
- `RESTART_GUARD_WINDOW_MS` (第 33 行)：自动重启防护窗口 10 分钟。
- `RESTART_GUARD_MAX` (第 34 行)：窗口内最多 2 次自动重启。
- `RESTART_GUARD_FILE` (第 35 行)：重启计数落盘文件名 `"webview_restart_guard"`，位于 `app_data_dir`；跨进程持久化防"必现故障 → 无限重启闪屏循环"。
- `RUNTIME_DIR` (第 369 行，Windows)：WebView2 Runtime 固定版本分发安装根目录 `C:\Program Files (x86)\Microsoft\EdgeWebView\Application`，注册表缺失时的版本回退来源。
- `SUB_KEY` / `WOW_SUB_KEY` (第 373–376 行，Windows)：EdgeUpdate 安装记录 GUID 注册表路径（`{F3017226-FAC6-4E36-9A38-E4524AA30166}`），分别用于 64 位进程 `HKLM` 与 `WOW6432Node` 视图、以及用户级 `HKCU`。

### 函数（app/webview_recovery.rs）

- `recovery_gate(window_start_ms: u64, count: u32, now_ms: u64) -> (bool, u64, u32)` (第 39–47 行)：纯函数限流判定。窗口过期（≥5min）则重置为 `(true, now, 1)`；否则计数未达上限放行并 `count+1`；达上限返回 `(false, ...)`。
- `epoch_ms() -> u64` (第 49–54 行)：当前 Unix epoch 毫秒。
- `attempt_webview_recovery(app: &AppHandle, reason: &str)` (第 58–114 行)：恢复唯一入口（reload 路径）。从 `AppState.update_stats` 读 `webview_recovery_window_start_ms` / `webview_recovery_count`（第 62–69 行）经 `recovery_gate` 限流；超限只留 ERROR 日志。放行后 `get_webview_window("main").eval("window.location.reload()")`（第 100 行）；eval 失败（浏览器进程已退出）留 ERROR 证据但不自动重启。
- `restart_guard_decide(timestamps: &[u64], now_ms: u64) -> (bool, Vec<u64>)` (第 118–129 行)：纯函数判定，丢弃窗口外旧记录，放行时把 `now_ms` 追加入列表。
- `restart_guard_read(path: &Path) -> Vec<u64>` (第 132–136 行)：读取重启记录文件（每行一个 epoch ms；缺失/损坏按空处理）。
- `restart_guard_allow(app: &AppHandle, now_ms: u64) -> bool` (第 140–152 行)：跨进程限流 + 落盘。文件读写失败按放行处理（宁可多试一次也不因磁盘问题放弃自愈）。
- `attempt_app_restart(app: &AppHandle, reason: &str)` (第 157–176 行)：WebView 已整体失效（浏览器进程退出、Closed 态）时的恢复动作，reload 无效，唯一可靠自愈是 `app.restart()`；先 `logger::flush()` 再重启。受 `restart_guard_allow` 限流。
- `subscribe_process_failed(app: &AppHandle)` (第 179–237 行，`#[cfg(target_os = "windows")]`)：webview 创建后调用一次。经 `with_webview` 取 `controller.CoreWebView2()` 并 `cast::<ICoreWebView2_4>()`，用 `ProcessFailedEventHandler::create` 订阅 `add_ProcessFailed`；回调读取 `ProcessFailedKind` 及 v2 接口 `Reason`/`ExitCode`（`ICoreWebView2ProcessFailedEventArgs2`，第 217–220 行），转交 `handle_process_failed`。
- `handle_process_failed(app, kind, reason: i32, exit_code: i32)` (第 245–269 行，Windows)：崩溃分级。浏览器进程退出（`BROWSER_PROCESS_EXITED`）→ `attempt_app_restart`；渲染/`FRAME_RENDER` 进程退出 → `attempt_webview_recovery`；其余（GPU/Utility/PPAPI 等）由运行时自愈，仅记日志。
- `describe_process_failed(kind, reason, exit_code) -> String` (第 273–286 行，Windows)：组装 kind+reason+exitCode 诊断串。
- `describe_process_failed_reason(reason: i32) -> &'static str` (第 289–317 行，Windows)：Reason 枚举 → 语义文本（崩溃/启动失败/内存不足/用户数据目录被删除/进程被外部终止/未预期退出/进程无响应/原因未知；`-1` 表示接口取不到）。
- `describe_process_failed_kind(kind) -> String` (第 320–360 行，Windows)：崩溃种类 → 日志描述（11 种 kind，含 "浏览器进程退出（WebView 已整体失效）" 等语义层）。
- `record_webview2_runtime_version()` (第 363–415 行)：启动时记录 WebView2 运行时版本（Windows 真实实现，非 Windows 空实现）。依次查 `HKLM`/`WOW6432Node`/`HKCU` 的 `pv` 值，回退到 `RUNTIME_DIR` 安装目录取最高版本（`pick_runtime_version`）。
- `pick_runtime_version(entries: std::io::Result<Vec<String>>) -> Option<String>` (第 419–433 行，Windows，纯函数)：从形如 `151.0.4129.107` 的四段版本号中选最高，过滤非版本条目（如 `Locales`）；`Err` 返回 `None`。

### 函数（app/window.rs）

- `HIDDEN_BY_DESIGN: AtomicBool` (第 8 行)：窗口"被应用自身隐藏"标记（托盘/静默启动用户意图）。
- `mark_hidden_by_design(hidden: bool)` (第 10–12 行) / `is_hidden_by_design() -> bool` (第 14–16 行)：读写上述标记。
- `is_user_dismissed(window: &WebviewWindow) -> bool` (第 22–24 行)：`is_minimized() || is_hidden_by_design()`，供后台任务判断是否静默完成。
- `set_main_webview_suspended(app, suspended: bool)` (第 38–48 行，Windows)：挂起/恢复主 WebView。**只用 `controller.SetIsVisible`，绝不调用 `TrySuspend`**（见模块注释第 26–37 行）。
- `reset_suspend_state_for_new_webview()` (第 53–56 行，Windows)：新 WebView 建立/销毁后复位 `LAST_MINIMIZED`，避免去抖吞掉首个 Resized 恢复事件。
- `apply_webview_suspended(pw, suspended)` (第 58–88 行，Windows)：实际 COM 调用。`ICoreWebView2_19::SetMemoryUsageTargetLevel`（隐藏 LOW / 恢复 NORMAL，第 73–82 行）+ `controller.SetIsVisible(!suspended)`（第 86 行）。
- `handle_window_focus_event(window, event)` (第 91–124 行)：焦点变化时调 `ICoreWebView2_19::SetMemoryUsageTargetLevel`（聚焦 NORMAL / 失焦 LOW）。非 Windows 空实现在第 123–124 行。
- `LAST_MINIMIZED: AtomicBool` (第 129 行，Windows)：上次同步给 WebView2 的"最小化"取值。
- `handle_window_resized_event(window, event)` (第 139–159 行，Windows)：最小化翻转时同步 `SetIsVisible` 挂起/恢复。非 Windows 空实现第 169–170 行。
- `note_minimized_synced()` (第 163–166 行，Windows)：主动挂起后标注 `LAST_MINIMIZED=true`，避免随后 Resized 事件重复触碰 COM。
- `show_and_focus_main<M: Manager<Wry>>(app)` (第 178–189 行)：显示并聚焦主窗口；先清 `hidden_by_design` 标记再 `unminimize`/`show`/`set_main_webview_suspended(false)`/`set_focus`。
- `show_or_rebuild_main(app)` (第 195–204 行)：托盘"显示主窗口"/托盘左键单击/`single_instance` 三处共用。窗口在则 `show_and_focus_main`，不在（轻量化已销毁）则 `rebuild_main_window`。
- `rebuild_main_window(app) -> Result<(), Box<dyn Error>>` (第 209–258 行)：从 `tauri.conf.json` 的 `main` 窗口配置经 `WebviewWindowBuilder::from_config` 重建（**绝不用 `WindowBuilder`**，否则得到无 webview 白窗，tauri issue #9307）。`visible(false)` 起步，等前端 `notify_window_ready` 或 5s 超时兜底才 show；几何恢复（离屏回退居中）；调用 `reset_suspend_state_for_new_webview`、退出轻量化态、`wait_window_ready_and_show`。

### 函数（app/heartbeat.rs）

- `spawn_heartbeat_thread(app_handle)` (第 9–61 行)：每 5s 检查 `update_stats.last_render_heartbeat_ms`；超过 `crash_threshold_ms = 20000` 且连续 3 次 stale 则调 `webview_recovery::attempt_webview_recovery`（第 48–51 行）。可见且非最小化才计入（排除 `document.hidden` 暂停心跳场景）。与 ProcessFailed 路径共用同一限流器。
- `spawn_window_safety_thread(app_handle)` (第 66–95 行)：启动 3s 后检查窗口可见性，不可见且非 `is_user_dismissed` 则 `show_and_focus_main`；最多重试 3 次，防 WebView2 初始化时序黑屏。

### 函数 / 类型（app/lightweight.rs）

- `LIGHTWEIGHT_ACTIVE: AtomicBool` (第 10 行)：轻量化生效中。
- `EXPECT_LIGHTWEIGHT_EXIT: AtomicBool` (第 14 行)：单次退出守卫（CloseRequested 置位、`ExitRequested` 取走消费）。
- `GEOMETRY: Mutex<Option<WindowGeometry>>` (第 16 行)：销毁前记录的窗口几何。
- `WindowGeometry { x, y, width, height: f64 }` (第 18–24 行)：窗口几何结构体（销毁时记录，重建时应用）。
- `is_lightweight_active()` / `set_lightweight_active(bool)` (第 26–32 行)。
- `arm_lightweight_exit_guard()` / `take_lightweight_exit_guard() -> bool` (第 34–40 行)：单次语义守卫。
- `should_prevent_exit(expecting, is_quitting) -> bool` (第 44–46 行)：纯函数，仅当"来自轻量化关闭"且不在主动退出流程时拦截退出。
- `capture_geometry(window)` (第 48–61 行) / `take_geometry() -> Option<WindowGeometry>` (第 63–65 行)。
- `geometry_is_on_screen(geo, monitors) -> bool` (第 68–74 行，纯函数)：几何与任一显示器矩形相交即有效。
- `effective_background_interval_ms` / `effective_quality_interval_ms` (第 77–84 行)：轻量化期间检测间隔下限（300s / 1800s）。
- `READY_SIGNALS: Mutex<Vec<(String, u64)>>` (第 87 行)：前端 ready 信号。
- `signal_window_ready(label)` (第 89–97 行) / `take_window_ready(label) -> bool` (第 99–107 行)。
- `wait_window_ready_and_show(app, label)` (第 112–127 行)：等 ready 信号或 5s 超时后 `show_and_focus_main`，防重建白闪。

### 状态字段（infra/state/mod.rs）

- `UpdateStats` 结构体（第 147–161 行）含 WebView 恢复相关字段：
  - `webview_recovery_window_start_ms: AtomicU64` (第 154 行)：reload 滑动窗口起点 epoch ms，0 表示未触发。
  - `webview_recovery_count: AtomicU32` (第 156 行)：当前窗口内已执行 reload 次数。
  - `last_render_heartbeat_ms: AtomicU64` (第 151 行)：前端心跳时间戳，供 heartbeat 线程检测。
  - `UpdateStats::new()` 构造器（第 169–182 行）将上述原子字段初始化为 0。

### IPC 命令（commands/system.rs、commands/config_cmd.rs）

- `minimize_window(window, state)` (system.rs 第 8–21 行)：先 `set_main_webview_suspended(true)` 同步挂起，再按 `minimize_to_tray` 走 hide 或 minimize。
- `close_window(window, state)` (system.rs 第 24–42 行)：`minimize_to_tray` 优先（隐藏到托盘）；否则 `lightweight_mode` 走 `window.close()`；否则置 `is_quitting` 后 close。
- `render_heartbeat(state)` (system.rs 第 261–275 行)：更新 `last_render_heartbeat_ms`，供心跳线程检测。
- `notify_window_ready(window)` (system.rs 第 389–393 行)：前端就绪信号，调 `signal_window_ready`。
- `show_window(app_handle)` (config_cmd.rs 第 212–228 行)：启动期自动显示路径，`is_user_dismissed` 时跳过 unminimize 保护，否则 `show_or_rebuild_main`。

### 浏览器参数与运行时（platform/gpu.rs、main.rs）

- `build_browser_args() -> String` (gpu.rs 第 236–248 行)：返回 `"--js-flags=--max-old-space-size=512"`；在 `main.rs` 第 57 行调用，并经 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 环境变量追加 Crashpad 转储参数（`--enable-crash-reporter --crash-dumps-dir=...`，main.rs 第 61–68 行），于 `set_var` 第 69 行写入，**必须在 Tokio runtime 创建前设置**（main.rs 第 55–56 行注释）。
- `record_webview2_runtime_version()` 在 startup.rs 第 252 行调用，`subscribe_process_failed` 在 startup.rs 第 253–254 行（Windows）调用。

## Architecture

组件关系（仅 Windows 桌面端；非 Windows 同名函数为空实现）：

```text
┌─────────────────────────┐   ┌──────────────────────────────┐
│ main.rs / startup.rs     │   │ tauri.conf.json (main window)│
│ setup: 设置浏览器参数     │   │ visible=false, label="main"  │
│ 记录 Runtime 版本 + 订阅  │   └──────────────┬───────────────┘
│ ProcessFailed            │                  │ WebviewWindow("main")
└───────────┬─────────────┘                  │
            │                                │
┌───────────▼─────────────┐   ProcessFailed  │  ┌────────────────────────────┐
│ webview_recovery.rs      │◄────────────────┼──│ webview2-com / COM          │
│ subscribe_process_failed │  事件订阅         │  │ ICoreWebView2_4::add_...    │
│ handle_process_failed    │                  │  └────────────────────────────┘
│  ├ BROWSER_EXITED →      │                  │
│  │  attempt_app_restart  │                  │
│  └ RENDER_EXITED →       │   reload(eval)   │
│     attempt_webview_      │──────────┐       │
│     recovery              │          │       │
└───────────┬─────────────┘          │       │
            │ (心跳超时路径)           ▼       │
┌───────────▼─────────────┐   ┌────────────────────┐   ┌──────────────────────┐
│ heartbeat.rs             │   │ window.rs          │   │ lightweight.rs       │
│ spawn_heartbeat_thread   │   │ set_main_webview_  │   │ rebuild_main_window  │
│ → attempt_webview_       │──►│  suspended(Set...  │   │ (ready 门/几何缓存)  │
│   recovery               │   │  Visible, 内存级别)│   └──────────┬───────────┘
│ spawn_window_safety_     │   │ show_or_rebuild_   │              │ 重建走
│   thread                 │   │  main              │              │ WebviewWindowBuilder
└─────────────────────────┘   └─────────┬─────────┘              │
                                       │                        │
                          minimize/close/show (commands/system.rs,
                          commands/config_cmd.rs)               │
                                       │                        │
                          ┌────────────▼─────────┐   ┌──────────▼──────────┐
                          │ shutdown.rs           │   │ tray.rs / 托盘事件   │
                          │ 轻量化销毁 or 隐藏托盘 │   │ 显示/重建主窗口       │
                          └───────────────────────┘   └─────────────────────┘
```

数据流要点：

- **崩溃分级**：WebCore 进程事件经由 `with_webview` 拿到 `controller.CoreWebView2()` 并 `cast::<ICoreWebView2_4>()`，回调拿 `ProcessFailedKind` + v2 接口的 `Reason`/`ExitCode`，`handle_process_failed` 据 kind 分流到「重启」或「reload」。
- **挂起只用 `SetIsVisible`**：`window.rs` 的 `apply_webview_suspended` 调 `controller.SetIsVisible(!suspended)` 并配 `ICoreWebView2_19::SetMemoryUsageTargetLevel`，**刻意不调 `TrySuspend`**（冻结渲染进程会让 React 调度器死锁，window.rs 第 26–37 行）。
- **限流两级**：reload 限流在内存原子（`update_stats.webview_recovery_*`，5min/3 次）；自动重启限流落盘（`webview_restart_guard` 文件，10min/2 次，重启后仍有效）。

## Data Flow

### 1. 启动期装配（main.rs → startup.rs）

1. `main.rs` 第 57 行 `gpu::build_browser_args()` 取基础参数，第 61–68 行追加 Crashpad 转储目录参数，第 69 行 `std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", ...)`——**必须在 `build_runtime` 前**（第 55–56 行）。
2. `startup.rs` `setup_app` 第 252 行 `record_webview2_runtime_version()` 记录运行时版本；第 253–254 行（Windows）`subscribe_process_failed(&app_h)` 建立崩溃订阅（webview 创建后，贯穿生命周期）。
3. 第 256–257 行启动心跳线程与窗口兜底显示线程。

### 2. 崩溃恢复路径（事件驱动，比心跳快 20–35s）

`WebView2 进程退出` → `subscribe_process_failed` 的 `ProcessFailedEventHandler` 回调（webview_recovery.rs 第 210–224 行）→ `handle_process_failed`（第 245 行）：
- `BROWSER_PROCESS_EXITED`（第 258 行）→ `attempt_app_restart`（第 157 行）→ `restart_guard_allow` 落盘限流（10min/2 次）→ `logger::flush()` 后 `app.restart()`。
- `RENDER_PROCESS_EXITED` / `FRAME_RENDER_PROCESS_EXITED`（第 260 行）→ `attempt_webview_recovery`（第 58 行）→ `recovery_gate` 内存限流（5min/3 次）→ `eval("window.location.reload()")`。
- 其余 kind → 仅 WARN 日志，交由 WebView2 运行时自愈。

### 3. 心跳恢复路径（兜底推断）

`react useHeartbeat` 每周期调 `render_heartbeat`（system.rs 第 261 行）→ 更新 `update_stats.last_render_heartbeat_ms`。`spawn_heartbeat_thread`（heartbeat.rs 第 9 行）每 5s 检测，超时 20s 且连续 3 次 stale（第 43–53 行）→ `attempt_webview_recovery`，与路径 2 共用 `recovery_gate` 限流器。

### 4. 最小化 / 隐藏到托盘（挂起）

- 用户最小化：`window.rs` `handle_window_resized_event`（第 140 行）检测到最小化翻转 → `set_main_webview_suspended(true)`（第 38 行）→ `apply_webview_suspended` 调 `SetIsVisible(false)` + 内存级别 LOW。
- `minimize_window` 命令（system.rs 第 8 行）点下即停：`set_main_webview_suspended(true)` 后 hide/minimize。
- `close_window`（第 24 行）`minimize_to_tray` 优先：隐藏 + `set_main_webview_suspended(true)`。
- 还原：`show_and_focus_main`（window.rs 第 178 行）清 `hidden_by_design` → `unminimize`/`show`/`set_main_webview_suspended(false)`（NORMAL 内存级别）/`set_focus`。

### 5. 轻量化销毁与重建

- `shutdown.rs` `handle_window_close_event`（第 20 行）`lightweight_mode` 分支：`capture_geometry` → `arm_lightweight_exit_guard` → `set_lightweight_active(true)` → 不 `prevent_close`，让 Tauri 真销毁窗口；最后一个窗口销毁时 `startup.rs` run 回调（第 144–157 行）拦截进程退出常驻。
- 托盘"显示主窗口"/左键单击/`single_instance` → `show_or_rebuild_main`（window.rs 第 195 行）→ 窗口不在则 `rebuild_main_window`（第 209 行）：`WebviewWindowBuilder::from_config` + `visible(false)`，等前端 `notify_window_ready`（system.rs 第 389 行）→ `signal_window_ready`（lightweight.rs 第 89 行）或 5s 超时 → `wait_window_ready_and_show`（第 112 行）→ `show_and_focus_main`。重启后 `restart_guard_allow` 的限流计数从落盘文件恢复（restart_guard_read 第 132 行）。

## Connections

- [[app-module]]：`webview_recovery.rs` / `window.rs` / `heartbeat.rs` / `lightweight.rs` / `startup.rs` / `shutdown.rs` / `tray.rs` 均在 `src/app/`，由 `app/mod.rs` 统一 export；`startup.rs` 在 `lib.rs` 第 12–13 行的 `#[cfg(desktop)] pub mod app` 下编译。
- [[state-module]]：`infra/state/mod.rs` 的 `AppState.update_stats`（`webview_recovery_window_start_ms` / `webview_recovery_count` / `last_render_heartbeat_ms`）是限流与心跳检测的状态存储；`lightweight` 的 `READY_SIGNALS` / `GEOMETRY` 用于重建。
- [[commands-module]]：`commands/system.rs`（`minimize_window`/`close_window`/`render_heartbeat`/`notify_window_ready`）与 `commands/config_cmd.rs`（`show_window`）是前端 IPC 桥接本模块逻辑的唯一入口。
- [[platform-gpu]]：`platform/gpu.rs` 的 `build_browser_args()`（`--js-flags=--max-old-space-size=512` + Crashpad 参数）经 `main.rs` 写入 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`，决定 WebView2 浏览器进程启动参数；`detect_display_refresh_rate` 经 `get_init_data` 暴露给前端。
- [[platform-ecoqos]]：轻量化销毁窗口后 `window.rs`/`startup.rs` 调 `ecoqos::set_ecoqos(true)`，把进程标记 EcoQoS（WebView2 子进程继承节流）。
- [[tauri-conf]]：`tauri.conf.json` 的 `app.windows[0]`（label `"main"`，`visible=false`，禁用 `browserExtensionsEnabled`）是本模块所有 `get_webview_window("main")` 的目标；`webviewInstallMode: downloadBootstrapper` 决定 WebView2 Runtime 分发方式。
- [[logger-module]]：`attempt_app_restart` 重启前 `logger::flush()` 刷盘诊断链；所有模块的 `log_*` 宏写入带 `"webview_recovery"`/`"lightweight"`/`"gpu"` 等 scope 的日志，是白屏诊断证据来源（配合 `record_webview2_runtime_version` 与 Crashpad 转储）。

## Known Issues

- **Tauri 2 不暴露 WebView2 `ProcessFailed` 事件（仅 Apple 平台有等价钩子，官方 PR #15162）**：导致渲染/浏览器进程崩溃只能靠前端心跳超时推断（最坏 20–35s 才触发第一次 reload），且 `eval reload` 对已退出/无响应进程无效。本模块经 `webview2-com` 直接订阅 `ICoreWebView2_4::add_ProcessFailed` 作为补偿（webview_recovery.rs 第 1–21 行背景说明、第 179–237 行实现）。
- **`TrySuspend` 冻结 React 调度器死锁（v2.4.2 缺陷根因）**：`window.rs` 第 26–37 行注释详述——`ICoreWebView2_3::TrySuspend` 冻结渲染进程会让 React 的 `MessageChannel` 调度回调丢失，`pendingLanes` 有更新但 `callbackNode` 永远为空，表现为按钮/切页全失效、仅合成器滚动可用。约束：**仅用 `controller.SetIsVisible`**，且 `window.rs` 第 260–272 行单测 `never_calls_try_suspend` 用 `include_str!` 静态断言源码不含 `TrySuspend(`。
- **最小化不触发原生遮挡检测（wry 0.55.1 行为）**：`window.rs` 第 131–139 行注释——wry 仅在 `hide()`/`show()` 时调 `SetIsVisible`，窗口最小化不发任何通知；WebView2 默认不做原生遮挡检测，导致最小化后 rAF/CSS 动画/合成器全速空转（v2.4.1 加 `backdrop-filter` 后问题显性化）。`handle_window_resized_event` 主动同步 `SetIsVisible` 兜底。
- **浏览器进程退出后 reload 无效（WebView 进入 Closed 态）**：实测 2026-09-11——杀掉浏览器进程后窗口整体变黑，`eval reload` 返回成功但页面不恢复，必须重建 WebView。故分流到 `attempt_app_restart`（`app.restart()`，webview_recovery.rs 第 14–18 行、第 154–176 行）。
- **必现故障的无限重启 / 空转风险**：两条恢复路径都有对应限流——reload 受内存滑动窗口 `recovery_gate`（5min/3 次，第 39–47 行），自动重启受落盘 `restart_guard_*`（10min/2 次，第 118–152 行）。`restart_guard` 必须落盘，否则进程重启后内存限流归零，必现故障会变成无限重启闪屏循环（第 31–32 行注释）。
- **RTSS（MSI Afterburner）hook 注入竞态**：`startup.rs` 第 200–205 行注释——WebView2 环境创建后注入已晚（6 个 WebView2 进程 3 个已被注入），故 RTSS 检测在 `main.rs` 第 52–53 行 `preinit_detect()`（早于 WebView2 环境创建）完成，`rtss_compat` 模块处理。
- **白屏类故障的版本相关性**：Evergreen 随微软自动更新，特定版本才存在白屏（WebView2Feedback#5692：4191.47 导航后白屏）。`record_webview2_runtime_version`（第 363 行）在启动期留痕版本号作为第一证据；Crashpad 转储（`main.rs` 第 61–68 行）与 `ProcessFailed` 的 Reason/ExitCode 组成完整诊断链。
- **无禁用/注释掉的功能**：本模块所有公开函数均在使用中，无被 `#[allow(dead_code)]` 或注释禁用的分支；非 Windows 平台仅以 `#[cfg(not(target_os = "windows"))]` 提供空实现（如 `handle_window_focus_event` 第 123 行、`handle_window_resized_event` 第 169 行、`record_webview2_runtime_version` 第 363 行非 Windows 空实现）。
