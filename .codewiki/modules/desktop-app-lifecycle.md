---
title: 桌面端应用生命周期（启动装配/托盘/窗口/快捷键/心跳/退出/WebView 恢复）
type: module
source_files:
  - tauri-app/src-tauri/src/main.rs
  - tauri-app/src-tauri/src/lib.rs
  - tauri-app/src-tauri/src/app/mod.rs
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/app/tray.rs
  - tauri-app/src-tauri/src/app/window.rs
  - tauri-app/src-tauri/src/app/shortcut.rs
  - tauri-app/src-tauri/src/app/heartbeat.rs
  - tauri-app/src-tauri/src/app/shutdown.rs
  - tauri-app/src-tauri/src/app/webview_recovery.rs
tags: [桌面端, 应用生命周期, 启动装配, 托盘, 窗口, 全局快捷键, 心跳, 优雅退出, WebView2, 崩溃恢复]
---

## Overview

本模块掌管桌面端进程从 `main()` 到 `app.run()` 的全部装配与运行期维护：`main.rs` 负责早期环境注入（WebView2 浏览器参数、Crashpad 转储目录）、helper 提权模式拦截、Tokio runtime 创建与收尾；`app/startup.rs` 的 `run()` 完成 5 个插件的注册、`AppState` 托管、命令注册表（`invoke_handler`）与 `setup_app` 中的一整套后台服务启动；`app/` 其余文件分别承担托盘、窗口焦点内存策略、全局快捷键、前端心跳与窗口兜底显示、优雅退出、WebView2 崩溃订阅与自愈。

模块整体被 `tauri-app/src-tauri/src/lib.rs:11` 的 `#[cfg(desktop)] pub mod app;` 限定为**桌面专属**，安卓 target 不编译（安卓端有独立的 `android/src-tauri/src/lib.rs`）。进程入口是 `tauri-app/src-tauri/src/main.rs`，它不通过 `campus_login_lib` 复用模块，而是在 `main.rs:3-14` 自行 `mod` 声明一份完整的模块树（见 Known Issues 第 1 条）。

## Key Components

### main.rs（进程入口）

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `main.rs:1` | crate 属性 | `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` | release 下隐藏控制台窗口 |
| `main.rs:3-14` | 私有 mod | `app` / `commands` / `config` / `network` / `auth` / `monitor` / `account` / `platform` / `update` / `infra` / `helper` / `self_service` | 二进制 crate 自带的模块树（共 12 个） |
| `main.rs:16` | fn | `main()` | 进程入口 |
| `main.rs:20-24` | 闭包 | `std::panic::set_hook` 注册 | 崩在途中先 `log_error!("panic", ...)` → `logger::flush_quick()` → `eprintln!`（`panic=abort` 下 hook 仍执行） |
| `main.rs:29-39` | match | helper 模式拦截 | `helper::parse_helper_args(&args)`：`Ok(Some((op, path)))` 直接 `exit(run_helper(...))`；`Ok(None)` 继续正常启动；`Err` 打印并 `exit(2)`（不启动正常应用） |
| `main.rs:43-55` | 语句块 | WebView2 参数注入 | `platform::gpu::build_browser_args()` + `--enable-crash-reporter --crash-dumps-dir=<data_dir>/com.campus.login/crashdumps` → `std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", ...)`；必须在建 Tokio runtime 之前（`main.rs:41-42` 注释说明 `set_var` 非线程安全） |
| `main.rs:57-59` | 语句块 | `core_count` | `std::thread::available_parallelism()`，失败退 2 |
| `main.rs:61-63` | 语句块 | runtime 装配 | `app::startup::build_runtime(core_count)` → `runtime.handle().clone()` → `tauri::async_runtime::set(handle)` |
| `main.rs:64` | 调用 | `app::startup::run(core_count)` | 阻塞直到事件循环结束 |
| `main.rs:65-67` | 语句块 | 进程收尾 | `logger::flush()` → `logger::shutdown()` → `runtime.shutdown_timeout(5s)` |

### lib.rs（跨平台协议核心 crate 根）

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `lib.rs:2` | pub mod | `account` | 仅 crypto 子模块，内部 `cfg(windows)` 分支 |
| `lib.rs:3` | pub mod | `auth` | 协议实现（安卓经此复用） |
| `lib.rs:4` | pub mod | `config` | 配置模型与持久化 |
| `lib.rs:5` | pub mod | `infra` | 全局基础设施（见 [[desktop-infra]]） |
| `lib.rs:6` | pub mod | `network` | 网络探测与适配器 |
| `lib.rs:7` | pub mod | `platform` | `mod.rs` 内已按文件拆分，`console_output` 跨平台、其余 desktop |
| `lib.rs:8` | pub mod | `self_service` | 自助服务 |
| `lib.rs:11-12` | pub mod（`#[cfg(desktop)]`） | `app` | 本文章主体，安卓不编译 |
| `lib.rs:13-14` | pub mod（`#[cfg(desktop)]`） | `commands` | Tauri 命令层 |
| `lib.rs:15-16` | pub mod（`#[cfg(desktop)]`） | `helper` | 提权 helper 模式 |
| `lib.rs:17-18` | pub mod（`#[cfg(desktop)]`） | `monitor` | 后台监控 |
| `lib.rs:19-20` | pub mod（`#[cfg(desktop)]`） | `update` | 更新 |
| `tauri-app/src-tauri/Cargo.toml:8-10` | — | `[lib] name = "campus_login_lib"`, `crate-type = ["cdylib", "rlib"]` | 安卓端以 `campus_login_lib::...` 引用（如 `android/src-tauri/src/lib.rs:48`） |

### app/mod.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `app/mod.rs:1` | pub mod | `startup` | runtime 与 Tauri 装配 |
| `app/mod.rs:2` | pub mod | `tray` | 托盘图标与菜单 |
| `app/mod.rs:3` | pub mod | `shortcut` | 全局快捷键事件处理 |
| `app/mod.rs:4` | pub mod | `heartbeat` | 前端心跳与窗口兜底显示 |
| `app/mod.rs:5` | pub mod | `shutdown` | 优雅退出与窗口关闭事件 |
| `app/mod.rs:6` | pub mod | `webview_recovery` | ProcessFailed 订阅与恢复动作 |
| `app/mod.rs:7` | pub mod | `window` | 窗口显示/聚焦与焦点内存策略 |

### app/startup.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `startup.rs:6` | pub fn | `build_runtime(core_count: usize) -> tokio::runtime::Runtime` | worker = `clamp(2, 8)`，max_blocking = `core_count*4 clamp(8, 64)`，线程名前缀 `campus-worker`，`enable_all()`；建失败打印并 `exit(1)`（`startup.rs:16-19`） |
| `startup.rs:23` | pub fn | `run(core_count: usize)` | 构造 `tauri::Builder`、注册插件/命令、`app.run(generate_context!())`；run 失败记 ERROR + `logger::flush()` + `exit(1)`（`startup.rs:122-126`） |
| `startup.rs:129` | 私有 fn | `setup_app(app, core_count) -> Result<(), Box<dyn Error>>` | `setup` 回调实体，见 Data Flow 启动顺序 |
| `startup.rs:63-120` | 宏调用 | `invoke_handler(tauri::generate_handler![...])` | **唯一的命令注册表**（`generate_handler!` 内共 56 条，行号区间 64-119），详见下方"命令注册表" |

`run()` 内的装配点逐行：

| 位置 | 内容 |
| --- | --- |
| `startup.rs:25` | `.plugin(tauri_plugin_shell::init())` |
| `startup.rs:26` | `.plugin(tauri_plugin_notification::init())` |
| `startup.rs:27-30` | `.plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))` |
| `startup.rs:31-35` | `.plugin(global_shortcut::Builder::new().with_handler(|app, shortcut, event| app::shortcut::handle_shortcut_event(...)).build())` |
| `startup.rs:36-49` | `.plugin(tauri_plugin_single_instance::init(...))`：已有 main 窗口则 `show_and_focus_main`，否则延迟 2s 重试一次（NSIS 安装器自动启动场景） |
| `startup.rs:50` | `.manage(AppState::new())` |
| `startup.rs:51-53` | `.setup(move |app| setup_app(app, core_count))` |
| `startup.rs:54-62` | `.on_window_event(...)`：`CloseRequested` → `app::shutdown::handle_window_close_event`；`Focused` → `app::window::handle_window_focus_event`（后者整体再套 `#[cfg(target_os = "windows")]`） |
| `startup.rs:122-126` | `app.run(tauri::generate_context!())` |

### app/tray.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `tray.rs:10` | pub fn | `build_tray(app: &AppHandle, install_dir: &Path) -> Result<(), Box<dyn Error>>` | 建菜单项、菜单、图标并注册托盘 |
| `tray.rs:11-13` | 语句块 | 菜单项 | `Menu` 项 id 与文案：`show`="显示主窗口"、`quick-login`="快速登录"、`quit`="退出" |
| `tray.rs:15-20` | 语句块 | `MenuBuilder` | 顺序：show → quick-login → separator → quit |
| `tray.rs:22-34` | 语句块 | 图标三级回退 | `app.default_window_icon()` → `<install_dir>/icons/icon.ico` → 嵌入 `include_bytes!("../../icons/icon.ico")` → 空图标 |
| `tray.rs:36-43` | 语句块 | `TrayIconBuilder` | `show_menu_on_left_click(false)`、`on_menu_event(handle_tray_menu_event)`、`on_tray_icon_event(handle_tray_icon_event)`、tooltip "校园网登录助手"；`build` 的错误被 `let _ =` 忽略（`tray.rs:36` 标注 `[忽略错误]`） |
| `tray.rs:49` | 私有 fn | `handle_tray_menu_event(app, event)` | 菜单分发：`"show"` → `window::show_and_focus_main`；`"quick-login"` → `spawn_blocking` 内抢 `is_logging_in` 后 `auth::service::full_login` 并发 `auto-login-result`（`tray.rs:56-81`）；`"quit"` → `shutdown::graceful_exit`（`tray.rs:82-85`）；其他 id 忽略 |
| `tray.rs:91` | 私有 fn | `handle_tray_icon_event(tray, event)` | 左键单击托图标 → `window::show_and_focus_main`（`tray.rs:92-97`） |

### app/window.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `window.rs:5` | pub fn（`#[cfg(target_os = "windows")]`） | `handle_window_focus_event(window: &Window, event: &WindowEvent)` | 仅处理 `Focused(_)`；经 `ICoreWebView2_19::SetMemoryUsageTargetLevel` 在聚焦时设 `NORMAL`、失焦时设 `LOW`（`window.rs:17-21`） |
| `window.rs:37` | pub fn（`#[cfg(not(target_os = "windows"))]`） | `handle_window_focus_event(_window: &tauri::WebviewWindow, _event: &WindowEvent)` | 空实现，**签名参数类型与 Windows 版不同**（见 Known Issues 第 2 条） |
| `window.rs:41` | pub fn | `show_and_focus_main<M: tauri::Manager<Wry>>(app: &M)` | `show()` + `set_focus()` + `unminimize()`，全部忽略错误（"窗口不存在也静默"） |

### app/shortcut.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `shortcut.rs:6` | 私有 static | `CANCEL_KEY: OnceLock<Option<Shortcut>>` | `CANCEL_EXIT_SHORTCUT` 的解析缓存，首次按键解析一次，失败则永久视为无快捷键 |
| `shortcut.rs:9` | pub fn | `handle_shortcut_event(app, shortcut: &Shortcut, event: ShortcutEvent)` | 只处理 `Pressed`；不匹配 `CANCEL_KEY` 直接返回；命中后 `spawn_blocking` 内先 `cancel_auto_exit_inner` 再 `cancel_campus_exit_with_notification`（`shortcut.rs:26-31`） |

### app/heartbeat.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `heartbeat.rs:9` | pub fn | `spawn_heartbeat_thread(app_handle: AppHandle)` | 注册 `"heartbeat"` 后台任务：每 5s 检查 `last_render_heartbeat_ms`，超 20s 累计 3 次即调 `webview_recovery::attempt_webview_recovery` |
| `heartbeat.rs:12` | 私有 const | `check_interval = 5s` | 检测周期 |
| `heartbeat.rs:13` | 私有 const | `crash_threshold_ms: u64 = 20_000` | 心跳陈旧阈值 |
| `heartbeat.rs:19-58` | 循环体 | 检测逻辑 | `monitorable = is_visible && !is_minimized`（`heartbeat.rs:24-25`，不可见即清零计数）；`last == 0` 跳过（`:39-41`）；`consecutive_stale >= 3` 才触发恢复（`:45-53`） |
| `heartbeat.rs:66` | pub fn | `spawn_window_safety_thread(app_handle: AppHandle)` | 注册 `"window_safety"` 后台任务：启动 3s 后检查主窗口可见性，不可见则强制 `show_and_focus_main`，最多 3 次（间隔 3s），全部失败即放弃 |
| `heartbeat.rs:73-88` | 循环体 | 兜底显示 | 每次失败记 WARN（`heartbeat.rs:79`），第 3 次后不再重试 |

### app/shutdown.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `shutdown.rs:8` | pub fn | `graceful_exit(app_handle: &AppHandle, _state: &AppState)` | `spawn` 异步任务 → 重新取 `AppState` → `infra::lifecycle::shutdown_and_exit`（注意 `_state` 参数未使用，内部从 `app_h` 重新取） |
| `shutdown.rs:19` | pub fn | `handle_window_close_event(window: &Window, event: &WindowEvent)` | `CloseRequested` 分支：`config.minimize_to_tray` 为真则 `prevent_close()` + `hide()`；为假则 `prevent_close()` + `graceful_exit`（`shutdown.rs:36-38`，修复"窗口关闭后 run loop 立即退出导致异步排空被截断"的历史缺陷） |

### app/webview_recovery.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `webview_recovery.rs:28` | 私有 const | `RECOVERY_WINDOW_MS = 5 * 60 * 1000` | reload 限流窗口 |
| `webview_recovery.rs:29` | 私有 const | `RECOVERY_MAX_PER_WINDOW: u32 = 3` | 窗口内最多 3 次 reload |
| `webview_recovery.rs:33` | 私有 const | `RESTART_GUARD_WINDOW_MS = 10 * 60 * 1000` | 自重启限流窗口 |
| `webview_recovery.rs:34` | 私有 const | `RESTART_GUARD_MAX: usize = 2` | 10 分钟内最多 2 次自重启 |
| `webview_recovery.rs:35` | 私有 const | `RESTART_GUARD_FILE: &str = "webview_restart_guard"` | 跨进程限流记录文件名（落在 `app_data_dir()`） |
| `webview_recovery.rs:39` | 私有 fn | `recovery_gate(window_start_ms, count, now_ms) -> (bool, u64, u32)` | 纯函数限流：窗口过期重置、未超限放行并 +1、超限拒绝且状态不变 |
| `webview_recovery.rs:49` | 私有 fn | `epoch_ms() -> u64` | 当前 epoch 毫秒 |
| `webview_recovery.rs:58` | pub fn | `attempt_webview_recovery(app: &AppHandle, reason: &str)` | 恢复唯一入口（心跳路径与 ProcessFailed 路径共用）：读 `UpdateStats` 两个字段 → `recovery_gate` → 写回 → 放行则 `window.eval("window.location.reload()")`（`webview_recovery.rs:97-106`），超限只写 ERROR（`:81-88`） |
| `webview_recovery.rs:118` | 私有 fn | `restart_guard_decide(timestamps: &[u64], now_ms) -> (bool, Vec<u64>)` | 纯函数：丢弃窗口外记录，未达上限则追加 `now_ms` |
| `webview_recovery.rs:132` | 私有 fn | `restart_guard_read(path) -> Vec<u64>` | 逐行解析 epoch ms，缺失/损坏按空 |
| `webview_recovery.rs:140` | 私有 fn | `restart_guard_allow(app, now_ms) -> bool` | 读判定写回一体；`app_data_dir()` 取不到或 IO 失败均按放行（宁可多试一次） |
| `webview_recovery.rs:157` | pub fn | `attempt_app_restart(app: &AppHandle, reason: &str)` | 跨进程限流通过后：`logger::flush()`（先刷诊断证据）→ `app.restart()` |
| `webview_recovery.rs:180` | pub fn（`#[cfg(target_os = "windows")]`） | `subscribe_process_failed(app: &AppHandle)` | 经 `with_webview` 拿 `ICoreWebView2_4::add_ProcessFailed`；缺 `ICoreWebView2_4` 时记 WARN 跳过（`webview_recovery.rs:199-208`），订阅失败记 ERROR（`:227-229`） |
| `webview_recovery.rs:246` | 私有 fn（`#[cfg(target_os = "windows")]`） | `handle_process_failed(app, kind, reason, exit_code)` | 分级：`BROWSER_PROCESS_EXITED` → `attempt_app_restart`；`RENDER_PROCESS_EXITED` / `FRAME_RENDER_PROCESS_EXITED` → `attempt_webview_recovery`；其余只记 WARN 交运行时自愈（`webview_recovery.rs:258-268`） |
| `webview_recovery.rs:274` | 私有 fn（`#[cfg(target_os = "windows")]`） | `describe_process_failed(kind, reason, exit_code) -> String` | 组装 `"<种类>, reason=<n> (<语义>), exitCode=<n>"` |
| `webview_recovery.rs:290` | 私有 fn（`#[cfg(target_os = "windows")]`） | `describe_process_failed_reason(reason: i32) -> &'static str` | Reason → 中文语义（崩溃/启动失败/内存不足/用户数据目录被删除/被外部终止/未预期退出/无响应/原因未知） |
| `webview_recovery.rs:321` | 私有 fn（`#[cfg(target_os = "windows")]`） | `describe_process_failed_kind(kind) -> String` | Kind → 中文语义 + `kind=<n>`（10 种 + fallback） |
| `webview_recovery.rs:363` | pub fn | `record_webview2_runtime_version()` | 非 Windows 为空函数；Windows 下查注册表三处（`HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\...`、`HKLM\...`、`HKCU\...` 的 EdgeUpdate Clients GUID）读 `pv`，缺失则回退扫 `C:\Program Files (x86)\Microsoft\EdgeWebView\Application` 取最高版本 |
| `webview_recovery.rs:369` | 私有 const（函数内） | `RUNTIME_DIR` | 固定版本分发的安装根目录 |
| `webview_recovery.rs:374` / `:376` | 私有 const（函数内） | `SUB_KEY` / `WOW_SUB_KEY` | EdgeUpdate 客户端注册表路径 |
| `webview_recovery.rs:420` | 私有 fn（`#[cfg(target_os = "windows")]`） | `pick_runtime_version(entries: io::Result<Vec<String>>) -> Option<String>` | 只取四段纯数字目录名，按版本段 `max_by_key` |
| `webview_recovery.rs:435-514` | `#[cfg(test)]` mod | `tests` | 6 个测试：reload 窗口放行/拒绝/重置、重启防护放行至上限/窗口过期丢弃、`pick_runtime_version` 选最高并忽略非版本条目 |

### 命令注册表（`startup.rs:63-120`，共 56 条）

| 归属模块 | 命令 | 行号 |
| --- | --- | --- |
| `commands::config_cmd` | `get_config`, `show_window`, `save_config` | `startup.rs:64-66` |
| `commands::login` | `do_login`, `do_logout` | `startup.rs:67-68` |
| `commands::network_cmd` | `get_adapters`, `get_adapter_details`, `check_campus_status`, `check_portal_status`, `get_disabled_adapters`, `enable_adapter`, `dhcp_renew_all`, `dhcp_release_renew`, `dhcp_release_renew_adapter`, `check_network_quality`, `start_latency_test`, `stop_latency_test`, `check_dns_doh_status`, `setup_dns_doh` | `startup.rs:69-82` |
| `commands::account` | `list_accounts`, `switch_account`, `save_current_as_account`, `delete_account`, `get_active_account` | `startup.rs:83-87` |
| `commands::background` | `start_background_check`, `stop_background_check`, `trigger_background_check`, `get_background_status` | `startup.rs:88-91` |
| `commands::system` | `get_auto_launch`, `set_auto_launch`, `get_notification_enabled`, `set_notification_enabled`, `cancel_auto_exit`, `minimize_window`, `close_window`, `open_external`, `get_logs`, `clear_logs`, `get_init_data`, `render_heartbeat`, `get_gpu_info`, `set_log_retention_days`, `get_log_retention_days` | `startup.rs:92-106` |
| `commands::updater` | `check_update`, `download_update`, `install_update`, `get_mirror_urls` | `startup.rs:107-110` |
| `commands::self_service` | `bind_operator`, `query_bind_status`, `verify_windows_identity`, `reveal_operator_credential`, `query_self_dashboard`, `query_self_online_log`, `self_offline_session` | `startup.rs:111-117` |
| `infra::logger` | `set_debug_mode`, `get_debug_mode` | `startup.rs:118-119` |

## 结构体与字段

`app/` 目录下**没有定义任何 struct / enum**（`mod.rs` 只有 7 个 `pub mod`，各文件只含函数）。本模块承载状态的载体是两处进程级静态、一个磁盘文件格式，以及它读写的 `infra` 侧字段：

### 进程级静态

| 位置 | 类型 | 含义 |
| --- | --- | --- |
| `app/shortcut.rs:6` | `OnceLock<Option<tauri_plugin_global_shortcut::Shortcut>>` | 取消退出快捷键的解析缓存；`Some(None)` 表示解析失败（永久禁用），`None` 表示尚未解析 |
| `app/webview_recovery.rs:144`（局部） | `PathBuf` = `app_data_dir()/webview_restart_guard` | 自重启限流记录文件，每行一个 epoch ms（`webview_recovery.rs:147-149` 写入，`:132-136` 读取） |

### 本模块读写的 `AppState` 字段（定义在 [[desktop-infra]]）

| 字段（含定义行） | 类型 | 本模块的读/写点 |
| --- | --- | --- |
| `UpdateStats::last_render_heartbeat_ms`（`infra/state/mod.rs:91`） | `AtomicU64` | 读 `app/heartbeat.rs:38`；写 `commands/system.rs:160`（命令侧） |
| `UpdateStats::webview_recovery_window_start_ms`（`infra/state/mod.rs:94`） | `AtomicU64` | 读 `app/webview_recovery.rs:62-65`，写 `:71-74` |
| `UpdateStats::webview_recovery_count`（`infra/state/mod.rs:96`） | `AtomicU32` | 读 `app/webview_recovery.rs:66-69`，写 `:75-78` |
| `ExitStateStore::is_quitting`（`infra/state/exit.rs:8`） | `Arc<AtomicBool>` | 由 `infra/lifecycle.rs:312` 置位；`infra/lifecycle.rs:87` 用于跳过最小化 |
| `ConfigStore`（`infra/state/mod.rs:126`） | `ArcSwap<Config>` | `app/shutdown.rs:25` 读 `minimize_to_tray`；`app/startup.rs:176` 写入首份配置 |

### runtime 参数（`app/startup.rs:6-19`）

| 参数 | 取值 | 行号 |
| --- | --- | --- |
| `worker_threads` | `core_count.clamp(2, 8)` | `startup.rs:7` |
| `max_blocking_threads` | `(core_count * 4).clamp(8, 64)` | `startup.rs:8` |
| `thread_name` | `"campus-worker"` | `startup.rs:13` |
| `enable_all` | true（timer + IO） | `startup.rs:14` |

### 时间常量

| 常量 | 值 | 位置 | 含义 |
| --- | --- | --- | --- |
| `check_interval` | 5s | `app/heartbeat.rs:12` | 心跳检测周期 |
| `crash_threshold_ms` | 20_000 | `app/heartbeat.rs:13` | 心跳陈旧阈值 |
| 连续计数阈值 | 3 | `app/heartbeat.rs:45` | 需连续 3 次超阈值才恢复 |
| 窗口兜底首检延迟 | 3s | `app/heartbeat.rs:71` | 启动后 3s 第一次检查可见性 |
| 窗口兜底重试 | 3 次，间隔 3s | `app/heartbeat.rs:73,82-86` | 最多共 3 次 |
| single-instance 重试延迟 | 2s | `app/startup.rs:43` | 主窗口尚未创建时的补偿 |
| reload 限流 | 5 分钟内 3 次 | `app/webview_recovery.rs:28-29` | 超限停止自动恢复 |
| 自重启限流 | 10 分钟内 2 次 | `app/webview_recovery.rs:33-34` | 跨进程落盘，防闪屏循环 |
| runtime 收尾超时 | 5s | `main.rs:67` | `Runtime::shutdown_timeout` |

## Data Flow

### 启动链路（`main()` → 事件循环）

```text
main() (main.rs:16)
  1. panic hook 注册 (main.rs:20-24)
  2. helper 模式拦截 (main.rs:29-39) —— 命中即 exit，不进入后续装配
  3. platform::gpu::build_browser_args() + crash-dumps-dir → WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS (main.rs:43-55)
  4. core_count = available_parallelism() (main.rs:57-59)
  5. build_runtime(core_count) (main.rs:61) → tauri::async_runtime::set(handle) (main.rs:63)
  6. app::startup::run(core_count) (main.rs:64)
       ├─ plugin × 5（顺序固定）: shell(25) → notification(26) → autostart(27) →
       │   global_shortcut(31, 带 with_handler) → single_instance(36)
       ├─ .manage(AppState::new()) (startup.rs:50)
       ├─ .setup(setup_app) (startup.rs:51-53) —— 见下
       ├─ .on_window_event: CloseRequested → shutdown::handle_window_close_event (54-57)
       │                    Focused(仅 Windows) → window::handle_window_focus_event (58-61)
       ├─ invoke_handler: 58 条命令 (63-120)
       └─ app.run(generate_context!()) (122)
  7. logger::flush() → logger::shutdown() → runtime.shutdown_timeout(5s) (main.rs:65-67)
```

`setup_app` 内部顺序（`startup.rs:129-219`，顺序即依赖关系）：

```text
1. data_dir = app.path().app_data_dir() 或 dirs::data_dir() (130-135)
2. install_dir = current_exe().parent() (137-140)
3. 日志目录探测：install_dir/logs 可写（create_dir_all + 写 .log_probe）则用之，
   否则回退 data_dir/logs (143-154)
4. logger::init_logger(log_dir) (158) —— 必须先于 config 加载，否则加载日志丢失
5. commands::config_cmd::load_config_from_disk_or_default(&app_handle) (162)
6. log_info! 启动横幅：CPU 核心/安装目录/日志目录/版本/WebView2 参数 (165-172)
7. CommandContext::from_app(app.handle()) (174)
8. state.config.store(config.clone()) (176) → network::update_portal_url(&config.portal_url) (177)
9. logger::set_log_retention_days(config.log_retention_days) (181)
10. app::tray::build_tray(&install_dir) (183) —— `?` 传播失败，会中断启动
11. monitor::adapter_watch::start_adapter_watch (186)
12. network::adapter_cache::start_cache_refresh_task (189)
13. update::updater::start_update_check_loop (193)
14. monitor::watcher::run_startup_tasks (195)
15. app::webview_recovery::record_webview2_runtime_version() (199)
16. app::webview_recovery::subscribe_process_failed() (201，仅 Windows)
17. app::heartbeat::spawn_heartbeat_thread (203)
18. app::heartbeat::spawn_window_safety_thread (204)
19. std::thread "gpu-warmup"：detect_gpu_info + detect_display_refresh_rate (208-216)
```

第 11、12 步返回 `Result` 且失败只记 WARN，不中断（`startup.rs:186-191`）；第 13、14 步返回 `()`，没有失败出口（`startup.rs:193,195`）；第 10 步失败会返回 `Err`（`startup.rs:183` 用 `?`）。第 10-18 步启动的所有后台任务都注册进同一个 `AppState::task_manager`（`infra/state/mod.rs:128`），因此退出时被统一取消。

### 退出链路

```text
窗口关闭: WindowEvent::CloseRequested (startup.rs:54-57) → shutdown::handle_window_close_event (app/shutdown.rs:19)
   ├─ config.minimize_to_tray == true  → api.prevent_close() + window.hide() (shutdown.rs:26-30)
   └─ false → api.prevent_close() + graceful_exit (shutdown.rs:36-38)
托盘 "quit": app/tray.rs:82-85 → shutdown::graceful_exit
graceful_exit (app/shutdown.rs:8) → spawn → infra::lifecycle::shutdown_and_exit (infra/lifecycle.rs:311)
   → is_quitting = true (312)
   → tokio::time::timeout(10s, task_manager.shutdown()) (315)
       → 全部任务 cancel_token.cancel() + await (infra/task_manager.rs:86-98)
   → app_handle.exit(0) (319)
→ main.rs:65-67 logger::flush() → logger::shutdown() → runtime.shutdown_timeout(5s)
```

注意：`handle_window_close_event` 未按窗口 label 过滤（`app/shutdown.rs:19-24`），当前只有 `main` 一个窗口所以无差异。

### 托盘与快捷键链路

```text
托盘菜单 (app/tray.rs:49)
  "show"        → window::show_and_focus_main (tray.rs:51-53)
  "quick-login" → spawn_blocking (tray.rs:56) → tasks.is_logging_in.try_acquire
                    ├─ 抢不到 → emit auto-login-result("登录正在进行中，请稍候") (tray.rs:62-66)
                    └─ 抢到 → auth::service::full_login (tray.rs:70) → emit auto-login-result (71-75)
                              → 成功则 auth::service::post_login_handler (77-79)
  "quit"        → shutdown::graceful_exit (tray.rs:82-85)
托盘左键单击 (app/tray.rs:91) → window::show_and_focus_main

全局快捷键 (startup.rs:31-35 注册 handler → app/shortcut.rs:9)
  Pressed + 等于 Ctrl+Shift+C (shortcut.rs:15-23)
  → spawn_blocking: cancel_auto_exit_inner (shortcut.rs:29) → cancel_campus_exit_with_notification (30)
     （两条取消都实现于 infra/lifecycle.rs:266 / :157）
```

### 心跳与 WebView 恢复链路

```text
前端 useHeartbeat (frontend/src/hooks/useHeartbeat.ts:11-15, setInterval 5000ms,
  document.hidden 或 isRenderLoopAlive() 为假时跳过)
  → invoke render_heartbeat (frontend/src/hooks/tauriApi.ts:219)
  → commands/system.rs:154 → update_stats.last_render_heartbeat_ms.store(now) (system.rs:160)

后端心跳线程 (app/heartbeat.rs:11)
  每 5s → 主窗口 monitorable? (heartbeat.rs:24-25, 不可见/最小化即跳过并清零)
        → now - last > 20s → consecutive_stale += 1 (heartbeat.rs:43-44)
        → 连续 3 次 → attempt_webview_recovery (heartbeat.rs:45-52)

WebView2 崩溃路径 (仅 Windows)
  ICoreWebView2_4::add_ProcessFailed (app/webview_recovery.rs:227)
  → handler 取 kind + Reason + ExitCode (webview_recovery.rs:210-224)
  → handle_process_failed (webview_recovery.rs:246)
      ├─ KIND_BROWSER_PROCESS_EXITED → attempt_app_restart (259)
      │     → restart_guard_allow (140)：10 分钟窗口落盘限流（≤2 次）
      │     → logger::flush() (174) → app.restart() (175)
      ├─ KIND_RENDER_PROCESS_EXITED / FRAME_RENDER_PROCESS_EXITED → attempt_webview_recovery (261)
      └─ 其余（GPU/Utility/PPAPI/沙箱/无响应等）→ 只记 WARN（263-267）

attempt_webview_recovery (webview_recovery.rs:58)
  → 读 UpdateStats 两字段 → recovery_gate (39)
  → 超限：ERROR 日志 + return（不重启，重启是用户决策，注释见 :98-99）
  → 放行：window.eval("window.location.reload()") (100)，失败记 ERROR (101-106)；
         主窗口不存在则记 ERROR (108-112)
```

### 扩展点（改动前必读）

- **新增 Tauri 命令**：在 `tauri-app/src-tauri/src/commands/` 下写 `#[tauri::command]` 函数，**必须**加入 `tauri-app/src-tauri/src/app/startup.rs:63-120` 的 `invoke_handler(tauri::generate_handler![...])`，否则前端 `invoke` 会报 "command not found"；安卓端命令注册在 `android/src-tauri/src/lib.rs`（独立文件），双端需各注册一次。前端包装加在 `tauri-app/frontend/src/hooks/tauriApi.ts`。
- **新增插件**：`.plugin(...)` 链在 `app/startup.rs:25-35`；`single_instance` 必须保持在最后（`startup.rs:36-49` 注释说明其回调依赖窗口已存在）。
- **新增启动期服务**：在 `setup_app` 的 `startup.rs:183-204` 区间内追加，并在前后保持"先 logger 再 config、先 config 再依赖 config 的服务"的顺序约束。
- **新增托盘菜单项**：`app/tray.rs:11-20` 建菜单项 + `app/tray.rs:50-86` 的 `match` 加分支；id 是字符串字面量，没有集中常量表，改名需两处同步。
- **新增平台专属窗口事件处理**：`app/startup.rs:54-62` 的 `on_window_event` 闭包内追加 `WindowEvent` 分支；Windows 专属处理需再套 `#[cfg(target_os = "windows")]`（参照 `startup.rs:58-61`），并注意 `app/window.rs:5` 与 `:37` 两个签名必须一致才能被同一调用点使用。
- **新增启动期后台任务**：一律用 `state.task_manager.spawn("唯一名字", ...)`（参照 `app/heartbeat.rs:11`），不要裸起线程——裸线程（如 `startup.rs:208` 的 `gpu-warmup`）不会被 `shutdown_and_exit` 等待。

## Connections

- [[desktop-infra]]：本模块是 `AppState` 的唯一托管点（`startup.rs:50`）；`shutdown_and_exit`、`BackgroundTaskManager`、`EventBus`、`TaskLock`、`emit_notification` 全在 infra。
- [[desktop-config]]：`load_config_from_disk_or_default`（`startup.rs:162`）、`state.config.store`（`startup.rs:176`）、`minimize_to_tray`（`app/shutdown.rs:25`）、`log_retention_days`（`startup.rs:181`）。
- [[desktop-commands]]：命令表注册点 `startup.rs:63-120` 是命令层与生命周期的接缝；`commands/system.rs:154` 的 `render_heartbeat`、`:8` 的 `minimize_window`、`:13` 的 `close_window` 直接服务本模块。
- [[desktop-monitor]]：`setup_app` 启动 `adapter_watch`（`startup.rs:186`）、`adapter_cache`（`:189`）、`run_startup_tasks`（`:195`）；托盘快速登录调用 `auth::service::full_login`。
- [[desktop-auth]]：`app/tray.rs:70-79` 的快速登录链路（`full_login` + `post_login_handler`）。
- [[desktop-helper-update]]：`helper::parse_helper_args` / `run_helper` 在 `main.rs:29-39` 拦截；`update::updater::start_update_check_loop` 在 `startup.rs:193`。
- [[desktop-platform]]：`platform::gpu::build_browser_args` / `detect_gpu_info`（`main.rs:43`、`startup.rs:211-212`）、`platform::toast`（系统通知，经 [[desktop-infra]] 的 `emit_notification`）。
- [[desktop-network-core]]：`network::update_portal_url`（`startup.rs:177`）、`network::adapter_cache`（`startup.rs:189`）。
- [[android-backend]]：安卓端不编译 `app/`（`lib.rs:11` 的 `#[cfg(desktop)]`），有独立的启动装配与命令注册。
- [[desktop-frontend-hooks]]：前端侧的对应实现是 `useHeartbeat.ts`（心跳）、`tauriApi.ts:153-219`（事件监听与 `render_heartbeat` 调用）。
- [[desktop-frontend-shared]]：托盘快速登录路径不经过前端，但结果通过 `auto-login-result` 事件（`tauriApi.ts:154`）回到前端。

## Known Issues

1. **二进制与 lib 各编译一份模块树**：`main.rs:3-14` 用 `mod` 自行声明 12 个模块，`lib.rs:2-20` 另声明一份；`main.rs` 不通过 `campus_login_lib::` 复用。后果是同一份源码被编译两遍（构建时间翻倍），且 `#[macro_export]` 宏在 bin 与 lib 各有一份实例（`crate::log_info!` 与 `campus_login_lib::log_info!` 是不同实例）。改动公共模块时两端都会重新编译，无法只改一端。
2. **`app/window.rs:37` 的非 Windows 空实现签名与 Windows 版不一致**：Windows 版签名是 `(&Window, &WindowEvent)`（`app/window.rs:5`），非 Windows 版写成 `(&tauri::WebviewWindow, &WindowEvent)`（`app/window.rs:37`）。唯一调用点 `app/startup.rs:58-61` 被 `#[cfg(target_os = "windows")]` 包裹，因此该分支从未被类型检查；一旦其他平台要处理焦点事件，直接调用会编译失败。
3. **心跳检测对"从未发过心跳"不成立**：`last == 0` 时 `continue`（`app/heartbeat.rs:39-41`），若前端首屏即崩在 `render_heartbeat` 之前，心跳路径永远不会触发恢复，只能等 `ProcessFailed` 事件（该事件仅 Windows 有）。
4. **心跳恢复的最坏时延是 30-35s**：5s 检测周期 × 连续 3 次 + 20s 阈值（`app/heartbeat.rs:12-13,45`），注释（`webview_recovery.rs:5`）也承认旧行为最坏 20-35s。这是 ProcessFailed 订阅（2026-09-11）要解决的性能问题。
5. **reload 限流状态在内存、自重启限流在磁盘**：`webview_recovery_count` / `webview_recovery_window_start_ms` 存在 `AppState`（`infra/state/mod.rs:94,96`），`app.restart()` 或进程重启后归零；自重启路径另有落盘文件 `webview_restart_guard`（`webview_recovery.rs:35`）。改限流策略时不要假设两者一致。
6. **`attempt_app_restart` 前的 `logger::flush()` 是最后一次机会**：`app/webview_recovery.rs:174` 之后立即 `app.restart()`（`:175`），此路径**不会**经过 `main.rs:65-67` 的 `logger::shutdown()` 与 `runtime.shutdown_timeout(5s)`，也不会走 `shutdown_and_exit` 的任务排空；崩溃链日志靠那一次 `flush()` 保住。
7. **`now_ms.saturating_sub(window_start_ms)` 对系统时钟回拨敏感**：`recovery_gate`（`app/webview_recovery.rs:40`）与 `restart_guard_decide`（`:122`）都用饱和减法比较时间；时钟被回拨时差值变小甚至为 0，会延长限流窗口（拒绝服务时间变长），时钟前跳则窗口提前过期。
8. **`restart_guard_decide` 只写回"放行时的列表"**（`app/webview_recovery.rs:140-152`）：被拒绝时不落盘，磁盘上的旧记录会保留到下一次放行才被清理；`read` 失败按空列表处理即视为放行（`:133-135`）。
9. **`record_webview2_runtime_version` 在非 Windows 是空函数但调用点无 cfg**（`app/startup.rs:199` vs `app/webview_recovery.rs:363-364`）：非 Windows 编译时该行是无副作用的空调用。
10. **`app/startup.rs:144-149` 的日志目录探测依赖 `.log_probe` 写入**：`create_dir_all` 对已存在目录恒返回 `Ok`，真正判定可写的是写空文件那一步；若写入成功但随后被 ACL 拒绝（或磁盘满），日志仍会静默走 `writer=None` 分支（`infra/logger.rs:79-81` 只 `eprintln!` 一行警告）。
11. **`build_tray` 的错误粒度不一致**：`TrayIconBuilder::build` 的错误被忽略（`app/tray.rs:36` 的 `let _ =`，注释标注"托盘图标创建失败不影响应用运行"），但菜单构建用 `?`（`tray.rs:20`）会向上传播并在 `app/startup.rs:183` 中断整个启动。改这里的容错策略需要同时看两处。
12. **托盘菜单 id 是裸字符串**：`"show"` / `"quick-login"` / `"quit"` 分散在 `app/tray.rs:11-13` 与 `:51,54,82`，没有常量表；`_ => {}`（`tray.rs:86`）会静默吞掉拼写错误。
13. **`graceful_exit` 的 `_state` 参数被忽略**（`app/shutdown.rs:8`）：内部在异步任务里重新 `app_h.state::<AppState>()`（`:11`），调用方传入的 `&AppState` 只用于保持签名一致；改造时别以为传进去的状态会被使用。
14. **`on_window_event` 未处理 `WindowEvent::Destroyed`，也无 `RunEvent::ExitRequested` 兜底**：进程退出的正常路径只有 `infra/lifecycle.rs:319` 的 `app_handle.exit(0)` 与 `app/webview_recovery.rs:175` 的 `app.restart()`，任何其他方式结束进程都会跳过任务排空与日志 flush。
15. **`spawn_window_safety_thread` 3 次失败后彻底放弃**（`app/heartbeat.rs:73-88`）：此后若窗口仍不可见，没有任何重试机制，只能重建进程。
16. **`startup.rs:208-216` 的 `gpu-warmup` 是裸线程**：不注册进 `BackgroundTaskManager`，`shutdown_and_exit` 不会等待它，进程退出时该线程被直接终止（其日志可能丢失）。
17. **`render_heartbeat` 的心跳在窗口隐藏/最小化时不由后端补偿**：前端 `useHeartbeat` 在 `document.hidden` 时暂停（`frontend/src/hooks/useHeartbeat.ts:8-13`），后端靠 `monitorable` 判定清零计数（`app/heartbeat.rs:24-25`）避免误重载；两边判定必须同时成立，改任一侧都会引入误判抖动。
