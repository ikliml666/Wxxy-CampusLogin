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
  - tauri-app/src-tauri/src/app/shutdown.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
tags: [桌面端, 应用生命周期, 启动装配, 托盘, 窗口, 全局快捷键, 优雅退出, WebView2, 崩溃恢复]
---

## Overview

本模块掌管桌面端进程从 `main()` 到 `app.run()` 的全部装配与运行期维护：`main.rs` 负责早期环境注入（WebView2 浏览器参数、Crashpad 转储目录）、helper 提权模式拦截、Tokio runtime 创建与收尾；`app/startup.rs` 的 `run()` 完成插件注册、`AppState` 托管、命令注册表（`invoke_handler`）与 `setup_app` 中的一整套后台服务启动；`app/` 其余文件分别承担托盘、窗口焦点内存策略、全局快捷键、优雅退出、窗口兜底显示。

模块整体被 `tauri-app/src-tauri/src/lib.rs:11-20` 的 `#[cfg(desktop)]` 限定为**桌面专属**，安卓 target 不编译（安卓端有独立的 `android/src-tauri/src/lib.rs`）。进程入口是 `tauri-app/src-tauri/src/main.rs`，它不通过 `campus_login_lib` 复用模块，而是在 `main.rs:3-14` 自行 `mod` 声明一份完整的模块树（见 Known Issues 第 1 条）。

## Key Components

### main.rs（进程入口）

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `main.rs:1` | crate 属性 | `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` | release 下隐藏控制台窗口 |
| `main.rs:3-14` | 私有 mod | `app` / `commands` / `config` / `network` / `auth` / `monitor` / `account` / `platform` / `update` / `infra` / `helper` / `self_service` | 二进制 crate 自带的模块树（共 12 个） |
| `main.rs:16` | fn | `main()` | 进程入口 |
| `main.rs:20-24` | 闭包 | `std::panic::set_hook` 注册 | 崩在途中先 `log_error!("panic", ...)` → `logger::flush_quick()` → `eprintln!`（`panic=abort` 下 hook 仍执行） |
| `main.rs:30-32` | 语句块 | `--helper-task` 拦截 | 计划任务 worker 模式：SYSTEM 主体任务拉起本 exe 处理提权请求目录，命中即 `exit(run_helper_task())`（优先于 `--helper` 拦截，无请求/目录不存在时空转安全退出） |
| `main.rs:42-51` | match | helper 模式拦截 | `helper::parse_helper_args(&args)`：`Ok(Some((op, result_path)))` 直接 `exit(run_helper(op, result_path))`；`Ok(None)` 继续正常启动；`Err` 打印并 `exit(2)`（不启动正常应用） |
| `main.rs:51-52` | 语句块 | RTSS preinit 检测 | `platform::rtss_compat::preinit_detect()`（`#[cfg(all(desktop, target_os = "windows"))]`，行51-52）：RTSS hook 注入风险检测，在 WebView2 环境创建前执行，结果暂存、待 startup 里 logger 就绪后留痕 |
| `main.rs:56-68` | 语句块 | WebView2 参数注入 | `platform::gpu::build_browser_args()`（56）+ `--enable-crash-reporter --crash-dumps-dir=<data_dir>/com.campus.login/crashdumps`（60-67）→ `std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", ...)`（68）；必须在建 Tokio runtime 之前（main.rs:54-55 注释说明 `set_var` 非线程安全） |
| `main.rs:70-72` | 语句块 | `core_count` | `std::thread::available_parallelism()`，失败退 2 |
| `main.rs:75-77` | 语句块 | runtime 装配 | `app::startup::build_runtime(core_count)`（74）→ `runtime.handle().clone()`（75）→ `tauri::async_runtime::set(handle)`（76） |
| `main.rs:77` | 调用 | `app::startup::run(core_count)` | 阻塞直到事件循环结束 |
| `main.rs:80-81` | 语句块 | 进程收尾 | `logger::flush()`（78） → `logger::shutdown()`（79） → `runtime.shutdown_timeout(5s)`（80） |

### lib.rs（跨平台协议核心 crate 根）

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `lib.rs:2` | pub mod | `account` | 仅 crypto 子模块，内部 `cfg(windows)` 分支 |
| `lib.rs:3` | pub mod | `auth` | 协议实现（安卓经此复用） |
| `lib.rs:4` | pub mod | `config` | 配置模型与持久化 |
| `lib.rs:5` | pub mod | `infra` | 全局基础设施 |
| `lib.rs:6` | pub mod | `network` | 网络探测与适配器 |
| `lib.rs:7` | pub mod | `platform` | `mod.rs` 内已按文件拆分，`console_output` 跨平台、其余 desktop |
| `lib.rs:8` | pub mod | `self_service` | 自助服务 |
| `lib.rs:12-20` | pub mod（`#[cfg(desktop)]`） | `app` / `commands` / `helper` / `monitor` / `update` | 桌面专属模块，安卓不编译 |

### app/mod.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `app/mod.rs:1` | pub mod | `startup` | runtime 与 Tauri 装配 |
| `app/mod.rs:2` | pub mod | `tray` | 托盘图标与菜单 |
| `app/mod.rs:3` | pub mod | `shortcut` | 全局快捷键事件处理 |
| `app/mod.rs:4` | pub mod | `heartbeat` | 前端心跳与窗口兜底显示 |
| `app/mod.rs:5` | pub mod | `shutdown` | 优雅退出与窗口关闭事件 |
| `app/mod.rs:6` | pub mod | `lightweight` | 轻量化模式运行态（活动标志/退出守卫/几何缓存/ready 信号表） |
| `app/mod.rs:7` | pub mod | `webview_recovery` | ProcessFailed 订阅与恢复动作 |
| `app/mod.rs:8` | pub mod | `window` | 窗口显示/聚焦与焦点内存策略 |

### app/startup.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `startup.rs:14-28` | pub fn | `build_runtime(core_count: usize) -> tokio::runtime::Runtime` | worker = `clamp(2, 8)`，max_blocking = `core_count*4 clamp(8, 64)`，线程名前缀 `campus-worker`，`enable_all()`；建失败打印并 `exit(1)` |
| `startup.rs:23-153` | pub fn | `run(core_count: usize)` | 构造 `tauri::Builder`、注册插件/命令；两段式收尾：`app.build`（133-137，失败 `log_error!` + `logger::flush()` + `exit(1)`）→ `app.run` 回调处理 `RunEvent::ExitRequested`（138-152，轻量化退出守卫拦截） |
| `startup.rs:155-266` | 私有 fn | `setup_app(app, core_count) -> Result<(), Box<dyn Error>>` | `setup` 回调实体，见 Data Flow 启动顺序 |
| `startup.rs:66-131` | 宏调用 | `invoke_handler(tauri::generate_handler![...])` | **唯一的命令注册表**（`generate_handler!` 内共 64 项命令，行号区间 67-130），详见下方"命令注册表" |

`run()` 内的装配点逐行：

| 位置 | 内容 |
| --- | --- |
| `startup.rs:25` | `.plugin(tauri_plugin_shell::init())` |
| `startup.rs:26` | `.plugin(tauri_plugin_notification::init())` |
| `startup.rs:27-30` | `.plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))` |
| `startup.rs:31-35` | `.plugin(global_shortcut::Builder::new().with_handler(|app, shortcut, event| app::shortcut::handle_shortcut_event(...)).build())` |
| `startup.rs:36-48` | `.plugin(tauri_plugin_single_instance::init(...))`：已有 main 窗口则 `show_or_rebuild_main`（37-38），否则延迟 2s 重建（42-47；NSIS 安装器自动启动或窗口已被轻量化销毁场景） |
| `startup.rs:49` | `.manage(AppState::new())` |
| `startup.rs:50-52` | `.setup(move |app| setup_app(app, core_count))` |
| `startup.rs:54-66` | `.on_window_event(...)`：`CloseRequested` → `app::shutdown::handle_window_close_event`（54-56）；`Focused` → `app::window::handle_window_focus_event`（57-60，整体再套 `#[cfg(target_os = "windows")]`）；`Resized` → `app::window::handle_window_resized_event`（61-64，仅 Windows） |
| `startup.rs:133-152` | `app.build(...)`（失败 `exit(1)`）+ `app.run(...)` 回调（`RunEvent::ExitRequested` 轻量化退出拦截） |

### app/tray.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `tray.rs:216` | 私有 const | `TRAY_ID: &str = "main-tray"` | 托盘图标 id，动态刷新菜单时经 `app.tray_by_id` 定位 |
| `tray.rs:14` | 私有 const | `SWITCH_ITEM_PREFIX: &str = "switch-account:"` | 切换账号菜单项的事件 id 前缀，后缀是**账号 id**（稳定不变；展示用 displayName） |
| `tray.rs:25-73` | 私有 fn | `build_tray_menu(app) -> tauri::Result<Menu<Wry>>` | 构建完整菜单：`show`="显示主窗口"（:34）、`quick-login`="快速登录"（:35，**始终启用**）、`quick-logout`="快速注销"（:36-38，`config.user` 非空才启用）、`switch-account` 子菜单（:41-59，列 `list_account_items` 的账号，当前账号显示名+"（当前）"并禁用，列表为空时显示禁用占位项且整个子菜单禁用）、`quit`="退出"（:39） |
| `tray.rs:75-88` | pub fn | `refresh_tray_menu_state(app)` | 重建菜单（`build_tray_menu` + `set_menu`，内部 `spawn_blocking`）；账号增删/改名/切换后由命令层调用 |
| `tray.rs:91-118` | pub fn | `build_tray(app, install_dir) -> Result<(), Box<dyn Error>>` | `build_tray_menu` + 图标三级回退：`app.default_window_icon()`（94）→ `<install_dir>/icons/icon.ico`（96-99）→ 嵌入 `include_bytes!`（100-105）→ 空图标；`TrayIconBuilder::with_id(TRAY_ID)`、`show_menu_on_left_click(false)`、`on_menu_event(handle_tray_menu_event)`、`on_tray_icon_event(handle_tray_icon_event)`、tooltip "校园网登录助手"；`build` 的错误被 `let _ =` 忽略（:108 标注 `[忽略错误]`） |
| `tray.rs:121-210` | 私有 fn | `handle_tray_menu_event(app, event)` | 菜单分发：`"show"` → `window::show_or_rebuild_main`（:124）；`"quick-login"` → `spawn_blocking` 抢 `is_logging_in` 后 `auth::service::full_login`（:126-153）；`"quick-logout"` → 同构调 `commands::login::perform_full_logout_sync`（:154-172）；`SWITCH_ITEM_PREFIX` 前缀 → 复用 `commands::account::perform_switch_account_sync`（:173-203）；`"quit"` → `shutdown::graceful_exit`（:204-207）；`_ => {}` 静默 |
| `tray.rs:213-220` | 私有 fn | `handle_tray_icon_event(tray, event)` | 左键单击托盘图标（`TrayIconEvent::Click` + `MouseButton::Left`，:214-215）→ `window::show_or_rebuild_main`（:217） |

### app/window.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `window.rs:8` | static | `HIDDEN_BY_DESIGN: AtomicBool` | 窗口「被应用自身隐藏」标记 |
| `window.rs:10-12` | pub fn | `mark_hidden_by_design(hidden: bool)` | 设置隐藏标记 |
| `window.rs:14-16` | pub fn | `is_hidden_by_design() -> bool` | 读取隐藏标记 |
| `window.rs:22-24` | pub fn | `is_user_dismissed(window: &tauri::WebviewWindow) -> bool` | 用户已主动收起窗口（最小化或隐藏 by design） |
| `window.rs:38-48` | pub fn | `set_main_webview_suspended(app: &tauri::AppHandle, suspended: bool)` | 挂起/恢复主 WebView2（仅 `SetIsVisible`，绝不 `TrySuspend`），Windows 有效 |
| `window.rs:53-56` | pub fn | `reset_suspend_state_for_new_webview()` | 新 WebView 建立后复位 `LAST_MINIMIZED` |
| `window.rs:59-88` | 私有 fn（`#[cfg(windows)]`） | `apply_webview_suspended(pw, suspended)` | 调 `ICoreWebView2_19::SetMemoryUsageTargetLevel` + `SetIsVisible(!suspended)` |
| `window.rs:92-120` | pub fn（`#[cfg(windows)]`） | `handle_window_focus_event(window: &Window, event: &WindowEvent)` | 聚焦设 `NORMAL` 内存级别，失焦设 `LOW` |
| `window.rs:123-124` | pub fn（`#[cfg(not(windows))]`） | `handle_window_focus_event(_window: &tauri::WebviewWindow, _event: &WindowEvent)` | 空实现，**签名参数类型与 Windows 版不同** |
| `window.rs:129` | static（`#[cfg(windows)]`） | `LAST_MINIMIZED: AtomicBool` | 上次同步给 WebView2 的「最小化」取值 |
| `window.rs:140-159` | pub fn（`#[cfg(windows)]`） | `handle_window_resized_event(window: &Window, event: &WindowEvent)` | 窗口最小化状态翻转时同步 WebView2 可见性 |
| `window.rs:163-166` | pub fn | `note_minimized_synced()` | 标注「最小化挂起已同步」，避免随后的 Resized 重复触碰 COM |
| `window.rs:169-170` | pub fn（`#[cfg(not(windows))]`） | `handle_window_resized_event(_window, _event)` | 空实现 |
| `window.rs:178-189` | pub fn | `show_and_focus_main<M: tauri::Manager<Wry>>(app: &M)` | `show()` + `set_focus()` + `unminimize()`，先清隐藏标记再恢复 |
| `window.rs:195-204` | pub fn | `show_or_rebuild_main(app: &tauri::AppHandle)` | 统一入口：窗口存在→`show_and_focus_main`；不存在→`rebuild_main_window` |
| `window.rs:209-258` | 私有 fn | `rebuild_main_window(app) -> Result<(), Box<dyn Error>>` | 从 `tauri.conf.json` 找 main 窗口配置 → `WebviewWindowBuilder::from_config` → 几何恢复（离屏校验回退居中）→ `visible(false)` 起步 `build()` → 退出轻量化态 → spawn ready 门 `wait_window_ready_and_show` |
| `window.rs:266-272` | mod tests | `never_calls_try_suspend` | 回归护栏：`window.rs` 不得调用 `TrySuspend` |

### app/shortcut.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `shortcut.rs:6` | 私有 static | `CANCEL_KEY: OnceLock<Option<Shortcut>>` | `CANCEL_EXIT_SHORTCUT` 解析缓存，首次按键解析一次 |
| `shortcut.rs:9-31` | pub fn | `handle_shortcut_event(app, shortcut: &Shortcut, event: ShortcutEvent)` | 只处理 `Pressed`；匹配缓存快捷键后 `spawn_blocking` 内先 `cancel_auto_exit_inner` 再 `cancel_campus_exit_with_notification` |

### app/shutdown.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `shutdown.rs:8-14` | pub fn | `graceful_exit(app_handle: &AppHandle, _state: &AppState)` | `spawn` 异步任务 → 重新取 `AppState` → `infra::lifecycle::shutdown_and_exit`（注意 `_state` 参数未使用） |
| `shutdown.rs:20-60` | pub fn | `handle_window_close_event(window: &Window, event: &WindowEvent)` | `CloseRequested` 三分支：`lightweight_mode` 为真 → 记几何 + 置单次守卫 + `set_lightweight_active(true)` + `set_ecoqos(true)`，**不 prevent_close**（:27-40）；`minimize_to_tray` 为真则 `prevent_close()` + `hide()`（:41-51）；为假则 `prevent_close()` + `graceful_exit`（:52-60） |

### 命令注册表（`startup.rs:67-130`，共 64 条命令）

| 归属模块 | 命令 | 行号 |
| --- | --- | --- |
| `commands::config_cmd` | `get_config`, `show_window`, `save_config`, `export_config`, `import_config` | `startup.rs:67-71` |
| `commands::login` | `do_login`, `do_logout` | `startup.rs:72-73` |
| `commands::network_cmd` | `get_adapters`, `get_adapter_details`, `check_campus_status`, `check_portal_status`, `enable_adapter`, `dhcp_renew_all`, `dhcp_release_renew`, `dhcp_release_renew_adapter`, `check_network_quality`, `start_latency_test`, `stop_latency_test`, `check_dns_doh_status`, `setup_dns_doh`, `reset_dns`, `outbound_switch_now`, `outbound_restore_now`, `get_current_outbound_name` | `startup.rs:74-90` |
| `commands::account` | `list_accounts`, `switch_account`, `rename_account`, `save_current_as_account`, `create_account`, `delete_account`, `get_active_account` | `startup.rs:91-97` |
| `commands::background` | `start_background_check`, `stop_background_check`, `trigger_background_check`, `get_background_status` | `startup.rs:98-101` |
| `commands::system` | `set_auto_launch`, `set_core_autostart`, `get_core_autostart`, `set_notification_enabled`, `cancel_auto_exit`, `minimize_window`, `close_window`, `open_external`, `get_logs`, `clear_logs`, `get_init_data`, `render_heartbeat`, `set_log_retention_days`, `export_diagnostics`, `notify_window_ready` | `startup.rs:102-116` |
| `commands::updater` | `check_update`, `download_update`, `cancel_download_update`, `install_update`, `get_mirror_urls` | `startup.rs:117-121` |
| `commands::self_service` | `bind_operator`, `query_bind_status`, `verify_windows_identity`, `reveal_operator_credential`, `query_self_dashboard`, `query_self_online_log`, `self_offline_session`, `change_self_password` | `startup.rs:122-129` |
| `infra::logger` | `set_debug_mode` | `startup.rs:130` |

## Architecture

```text
┌───────────────────────┐      ┌──────────────────────────────────────────────────┐
│    main.rs            │      │          app/startup.rs (Tauri Builder)           │
│                       │      │                                                  │
│ 1. panic hook         │      │  Plugins: shell, notification, autostart,          │
│ 2. --helper-task      │      │        global_shortcut, single_instance           │
│ 3. --helper args      │      │                                                  │
│ 4. RTSS detect        │      │  .manage(AppState)                                 │
│ 5. WebView2 args      │      │  .setup(setup_app) ──────────────────────────────► │
│ 6. build_runtime      │      │     ├─ data_dir/logs probe                        │
│ 7. run(core_count)    │      │     ├─ logger init → config load                  │
│ 8. flush/shutdown     │      │     ├─ tray::build_tray                           │
└───────────────────────┘      │     ├─ adapter_watch / adapter_cache              │
                               │     ├─ updater start_loop                         │
                               │     ├─ webview_recovery::record_version           │
                               │     ├─ webview_recovery::subscribe_process_failed │
                               │     ├─ heartbeat::spawn_heartbeat_thread          │
                               │     └─ heartbeat::spawn_window_safety_thread      │
                               └──────────────────────────────────────────────────┘
                                       │
                       ┌───────────────▼──────────────────────────────────────────┐
                       │               app/ (运行期维护)                          │
                       │                                                          │
                       │  tray.rs: handle_tray_menu_event (show/login/logout/switch)│
                       │  window.rs: show_or_rebuild_main / rebuild_main_window     │
                       │  shortcut.rs: handle_shortcut_event (cancel_exit)          │
                       │  shutdown.rs: handle_window_close_event (轻量/min/graceful)│
                       │  lightweight.rs: ACTIVE flag / EXIT guard / geometry       │
                       │  webview_recovery.rs: attempt_webview_recovery / restart   │
                       └──────────────────────────────────────────────────────────┘
```

## Data Flow

### 启动链路（`main()` → 事件循环）

```text
main() (main.rs:16)
  1. panic hook 注册 (main.rs:20-24)
  2. --helper-task 拦截 (main.rs:30-32) —— 计划任务 worker 模式，命中即 exit
  3. helper 模式拦截 (main.rs:37-46) —— 命中即 exit，不进入后续装配
  4. RTSS preinit 检测 (main.rs:51-52) + build_browser_args() + crash-dumps-dir →
     WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS (main.rs:56-68)
  5. core_count = available_parallelism() (main.rs:70-72)
  6. build_runtime(core_count) (main.rs:74) → tauri::async_runtime::set(handle) (main.rs:76)
  7. app::startup::run(core_count) (main.rs:77)
       ├─ plugin × 5（顺序固定）: shell(25) → notification(26) → autostart(27) →
       │   global_shortcut(31, 带 with_handler) → single_instance(36)
       ├─ .manage(AppState::new()) (startup.rs:49)
       ├─ .setup(setup_app) (startup.rs:50-52) —— 见下
       ├─ .on_window_event: CloseRequested → shutdown::handle_window_close_event (53-56)
       │                    Focused(仅 Windows) → window::handle_window_focus_event (57-60)
       │                    Resized(仅 Windows) → window::handle_window_resized_event (61-64)
       ├─ invoke_handler: 64 项 (66-131)
       └─ app.build(失败 exit(1)) (133-137) + app.run 回调拦截 ExitRequested (138-152)
  8. logger::flush() (78) → logger::shutdown() (79) → runtime.shutdown_timeout(5s) (80)
```

`setup_app` 内部顺序（`startup.rs:155-266`，顺序即依赖关系）：

```text
1. data_dir = app.path().app_data_dir() 或 dirs::data_dir() (156-158)，创建失败仅 WARN
2. install_dir = current_exe().parent() (163-166)
3. 日志目录探测：install_dir/logs 可写（create_dir_all + 写 .log_probe）则用之，
   否则回退 data_dir/logs (169-180)
4. logger::init_logger(log_dir) (184) —— 必须先于 config 加载，否则加载日志丢失
5. commands::config_cmd::load_config_from_disk_or_default(&app_handle) (188)
6. log_info! 启动横幅：CPU 核心/安装目录/日志目录 (191)、版本 (192)
7. RTSS log_preinit_outcome 留痕 (199，仅 Windows；实际处理已在 main 入口完成)
8. 补记 WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS 实际值 (201-205，含 crash 转储参数)
9. CommandContext::from_app(app.handle()) (207)
10. state.config.store(config.clone()) (209) → network::update_portal_url(&config.portal_url) (210)
11. logger::set_log_retention_days(config.log_retention_days) (215)
12. 静默启动判定：hidden_start && --autostart 则 mark_hidden_by_design(true) (220-225)
13. app::tray::build_tray(&install_dir) (227) —— `?` 传播失败，会中断启动
14. monitor::adapter_watch::start_adapter_watch (230-232)
15. network::adapter_cache::start_cache_refresh_task (233-235)
16. update::updater::start_update_check_loop (237)
17. update::updater::cleanup_stale_update_dir (240)
18. monitor::watcher::run_startup_tasks (242)
19. app::webview_recovery::record_webview2_runtime_version() (246)
20. app::webview_recovery::subscribe_process_failed() (248，仅 Windows)
21. app::heartbeat::spawn_heartbeat_thread (250)
22. app::heartbeat::spawn_window_safety_thread (251)
23. std::thread "gpu-warmup"：detect_gpu_info + detect_display_refresh_rate (255-263)
```

第 14、15 步返回 `Result` 且失败只记 WARN，不中断；第 16-18 步返回 `()`；第 13 步失败会返回 `Err`。

### 退出链路

```text
窗口关闭: WindowEvent::CloseRequested (startup.rs:53-56) → shutdown::handle_window_close_event (app/shutdown.rs:20)
   ├─ config.lightweight_mode == true → 记几何 + 置单次守卫 + 轻量化标志 + EcoQoS，
   │     不 prevent_close 让窗口真销毁 (shutdown.rs:27-40)；
   │     最后一个窗口销毁触发 ExitRequested，由 startup.rs:138-152 的 run 回调消费守卫拦截退出
   ├─ config.minimize_to_tray == true  → api.prevent_close() + window.hide() (shutdown.rs:41-51)
   └─ false → api.prevent_close() + graceful_exit (shutdown.rs:52-60)
托盘 "quit": app/tray.rs:204-207 → shutdown::graceful_exit
graceful_exit (app/shutdown.rs:8) → spawn → infra::lifecycle::shutdown_and_exit
   → 取消并等待所有后台任务 → app_handle.exit(0)
→ main.rs:78-80 logger::flush() → logger::shutdown() → runtime.shutdown_timeout(5s)
```

注意：`handle_window_close_event` 未按窗口 label 过滤（`app/shutdown.rs:20-23`），当前只有 `main` 一个窗口所以无差异。

### 托盘与快捷键链路

```text
托盘菜单 (app/tray.rs:121)
  "show"        → window::show_or_rebuild_main (tray.rs:124)
  "quick-login" → spawn_blocking (tray.rs:128) → tasks.is_logging_in.try_acquire
                    ├─ 抢不到 → emit auto-login-result("登录正在进行中，请稍候") (133-140)
                    └─ 抢到 → auth::service::full_login (tray.rs:142) → emit auto-login-result (143-147)
                              → 成功则 auth::service::post_login_handler (149-151)
  "quick-logout"→ spawn_blocking (156) → perform_full_logout_sync (158)
                    → emit_login_log (160-163) → 成功/失败 emit_notification (166-170)
  "switch-account:<id>" → perform_switch_account_sync (tray.rs:180)
  "quit"        → shutdown::graceful_exit (tray.rs:204-207)
托盘左键单击 (app/tray.rs:213-220) → window::show_or_rebuild_main

全局快捷键 (startup.rs:31-35 注册 handler → app/shortcut.rs:9)
  Pressed + 等于 CANCEL_EXIT_SHORTCUT（shortcut.rs:11-23，值解析自 CANCEL_KEY）
  → spawn_blocking: cancel_auto_exit_inner (shortcut.rs:29) → cancel_campus_exit_with_notification (30)
```

### 心跳与 WebView 恢复链路（调用点摘要）

```text
后端心跳线程 (app/heartbeat.rs，由 startup.rs:250 启动)
  → 周期性检测前端心跳，异常时调用 webview_recovery 恢复逻辑
WebView2 崩溃路径 (仅 Windows，由 startup.rs:248 订阅)
  → ICoreWebView2 ProcessFailed 事件 → 按进程类型恢复或重启
上述具体实现位于 app/heartbeat.rs 与 app/webview_recovery.rs（不在本文源码范围内），
本文仅记录 startup.rs 中的启动与订阅点。
```

### 扩展点（改动前必读）

- **新增 Tauri 命令**：在 `tauri-app/src-tauri/src/commands/` 下写 `#[tauri::command]` 函数，**必须**加入 `tauri-app/src-tauri/src/app/startup.rs:66-131` 的 `invoke_handler(tauri::generate_handler![...])`，否则前端 `invoke` 会报 "command not found"；安卓端命令注册在 `android/src-tauri/src/lib.rs`（独立文件），双端需各注册一次。
- **新增插件**：`.plugin(...)` 链在 `app/startup.rs:25-35`；`single_instance` 必须保持在最后（`startup.rs:36-48` 回调依赖窗口已存在）。
- **新增启动期服务**：在 `setup_app` 的 `startup.rs:227-263` 区间内追加，并保持"先 logger 再 config、先 config 再依赖 config 的服务"的顺序约束。
- **新增托盘菜单项**：`app/tray.rs:34-39` 建菜单项 + `app/tray.rs:121-210` 的 `match` 加分支；id 是字符串字面量，没有集中常量表（除 `SWITCH_ITEM_PREFIX` 外），改名需两处同步。
- **新增平台专属窗口事件处理**：`app/startup.rs:53-65` 的 `on_window_event` 闭包内追加 `WindowEvent` 分支；Windows 专属处理需再套 `#[cfg(target_os = "windows")]`（参照 `startup.rs:53-56`），并注意 `app/window.rs:92` 与 `:123-124` 两个签名必须一致才能被同一调用点使用。
- **新增启动期后台任务**：一律用 `state.task_manager.spawn(...)`（参照 `app/heartbeat.rs`），不要裸起线程——裸线程（如 `startup.rs:207-215` 的 `gpu-warmup`）不会被 `shutdown_and_exit` 等待。

## Connections

- [[desktop-infra]]：本模块是 `AppState` 的唯一托管点（`startup.rs:49`）；`shutdown_and_exit`、`BackgroundTaskManager`、`EventBus`、`TaskLock`、`emit_notification` 全在 infra。
- [[desktop-config]]：`load_config_from_disk_or_default`（`startup.rs:188`）、`state.config.store`（`startup.rs:209`）、`minimize_to_tray`（`app/shutdown.rs:41`）、`log_retention_days`（`startup.rs:215`）。
- [[desktop-commands]]：命令表注册点 `startup.rs:113-176` 是命令层与生命周期的接缝；`commands/system.rs` 的 `render_heartbeat`、`minimize_window`、`close_window` 直接服务本模块；`commands/config_cmd.rs` 的 `show_window` 走 `show_or_rebuild_main`。
- [[desktop-monitor]]：`setup_app` 启动 `adapter_watch`（`startup.rs:230`）、`adapter_cache`（`startup.rs:233`）、`run_startup_tasks`（`startup.rs:242`）；托盘快速登录调用 `auth::service::full_login`。
- [[desktop-auth]]：`app/tray.rs:126-153` 的快速登录链路（`full_login` + `post_login_handler`）。
- [[desktop-helper-update]]：`helper::parse_helper_args` / `run_helper` 在 `main.rs:37-46` 拦截；`update::updater::start_update_check_loop` 在 `startup.rs:237`。
- [[desktop-platform]]：`platform::gpu::build_browser_args` / `detect_gpu_info`（`main.rs:56`、`startup.rs:258`）、`platform::rtss_compat::preinit_detect` / `log_preinit_outcome`（`main.rs:52`、`startup.rs:199`）、`platform::ecoqos`（`app/shutdown.rs:37`、`app/window.rs:249`）。
- [[desktop-network-core]]：`network::update_portal_url`（`startup.rs:210`）、`network::adapter_cache`（`startup.rs:233`）。
- [[android-backend]]：安卓端不编译 `app/`（`lib.rs:11-20` 的 `#[cfg(desktop)]`），有独立的启动装配与命令注册。
- [[desktop-frontend-hooks]]：前端侧的对应实现是 `useHeartbeat.ts`、`tauriApi.ts:243`（`render_heartbeat` 调用）。
- [[desktop-frontend-shared]]：托盘快速登录路径不经过前端，但结果通过 `auto-login-result` 事件回到前端。
- [[lightweight-mode-desktop]]：轻量化模式运行态在 `app/lightweight.rs`，由 `shutdown.rs:34-37` 与 `window.rs:247-249` 等调用。

## 2026-09-20 增补：轻量化模式（关闭/守卫/重建/EcoQoS）

详细决策见 [[lightweight-mode-desktop]]，此处只记结构位置：

- **关闭三分支**（`app/shutdown.rs:20-60` 的 `handle_window_close_event`）：`lightweight_mode=true`（:27-40）→ 记几何（`capture_geometry`）+ 置单次守卫（`arm_lightweight_exit_guard`）+ `set_lightweight_active(true)`（:35）+ `set_ecoqos(true)`（:36-37），**不 prevent_close** 让窗口真销毁（WebView2 进程组退出）；否则按 `minimize_to_tray`（:41-51，hide）或 `graceful_exit`（:52-60）。
- **退出守卫**（`app/startup.rs:138-152`）：run 改 `build()` + `app.run(callback)` 两段式，`RunEvent::ExitRequested` 中 `should_prevent_exit(expecting, is_quitting)` 判定（单次守卫由 `app/lightweight.rs` 的 `arm/take_lightweight_exit_guard` 管理），仅拦截轻量化关闭导致的隐式退出；托盘"退出"与系统关机不拦。
- **重建入口**（`app/window.rs:195` 的 `show_or_rebuild_main`，接具体 `&AppHandle`——`available_monitors` 是 AppHandle 固有方法）：窗口在则 show，不在则 `rebuild_main_window`（`window.rs:209-258`）：`WebviewWindowBuilder::from_config` 复刻 tauri.conf.json 窗口配置（:219）+ 几何恢复（离屏校验回退居中，:221-239）+ `.visible(false)` + ready 门（新命令 `notify_window_ready` → `wait_window_ready_and_show`，:252-256）；重建成功清轻量化标志并关 EcoQoS（:247-249）。接线五处：托盘 show 菜单（tray.rs:124）、托盘左键（tray.rs:217）、single_instance 主路径与 2s 延迟分支（startup.rs:38/45）、`show_window` 命令。
- **EcoQoS**（`platform/ecoqos.rs::set_ecoqos`，`#[cfg(desktop)]` 挂载、非 Windows 空实现）：仅轻量化期间开启（WebView2 子进程继承宿主节流状态）。
- **运行态**（`app/lightweight.rs`，全部内存态）：活动标志/退出守卫/几何缓存/ready 信号表；心跳线程对"窗口不存在"空转安全；`window_safety` 兜底线程仅在启动后活动，与运行期销毁/重建无冲突。

## Known Issues

1. **二进制与 lib 各编译一份模块树**：`main.rs:3-14` 用 `mod` 自行声明 12 个模块，`lib.rs:2-20` 另声明一份；`main.rs` 不通过 `campus_login_lib::` 复用。后果是同一份源码被编译两遍，且 `#[macro_export]` 宏在 bin 与 lib 各有一份实例（`crate::log_info!` 与 `campus_login_lib::log_info!` 是不同实例）。改动公共模块时两端都会重新编译，无法只改一端。
2. **`app/window.rs:123-124` 的非 Windows 空实现签名与 Windows 版不一致**：Windows 版签名是 `(&Window, &WindowEvent)`（`app/window.rs:92`），非 Windows 版写成 `(&tauri::WebviewWindow, &WindowEvent)`（`app/window.rs:123-124`）。唯一调用点 `app/startup.rs:57-60` 被 `#[cfg(target_os = "windows")]` 包裹，因此该分支从未被类型检查；一旦其他平台要处理焦点事件，直接调用会编译失败。
3. **`build_tray` 的错误粒度不一致**：`TrayIconBuilder::build` 的错误被忽略（`app/tray.rs:108` 的 `let _ =`，注释标注"托盘图标创建失败不影响应用运行"），但菜单构建用 `?`（`build_tray_menu` 内多处）会向上传播并在 `app/startup.rs:227` 中断整个启动。改这里的容错策略需要同时看两处。
4. **托盘菜单 id 是裸字符串**：`"show"` / `"quick-login"` / `"quick-logout"` / `"quit"` 分散在 `app/tray.rs:34-39` 与 `:123,126,154,204`，没有常量表（仅 `SWITCH_ITEM_PREFIX` 有常量，`tray.rs:14`）；`_ => {}`（`tray.rs:208`）会静默吞掉拼写错误。
5. **`graceful_exit` 的 `_state` 参数被忽略**（`app/shutdown.rs:8`）：内部在异步任务里重新 `app_h.state::<AppState>()`（:11），调用方传入的 `&AppState` 只用于保持签名一致；改造时别以为传进去的状态会被使用。
6. **`RunEvent::ExitRequested` 回调只做轻量化守卫拦截，不做任务排空**（`app/startup.rs:138-152`）：进程退出的正常路径只有 `infra/lifecycle.rs` 的 `app_handle.exit(0)`（带任务排空与 flush）与 `app/webview_recovery.rs` 的 `app.restart()`（只 flush），任何其他方式结束进程（系统强杀、panic 之外的中断）都会跳过任务排空与日志 flush；`on_window_event` 也未处理 `WindowEvent::Destroyed`。
7. **`startup.rs:255-263` 的 `gpu-warmup` 是裸线程**：不注册进 `BackgroundTaskManager`，`shutdown_and_exit` 不会等待它，进程退出时该线程被直接终止（其日志可能丢失）。
8. **`startup.rs:169-180` 的日志目录探测依赖 `.log_probe` 写入**：`create_dir_all` 对已存在目录恒返回 `Ok`，真正判定可写的是写空文件那一步（:171）；若写入成功但随后被 ACL 拒绝（或磁盘满），日志仍会静默走 `writer=None` 分支。
9. **`single_instance` 回调延迟重建逻辑**：`startup.rs:36-48` 在窗口尚未创建时延迟 2s 后调 `show_or_rebuild_main`，若轻量化已销毁窗口则走重建；该延迟是固定值，无重试机制。
10. **`handle_window_close_event` 未按窗口 label 过滤**（`app/shutdown.rs:20-23`），当前只有 `main` 一个窗口所以无差异，若未来增加其他窗口需加过滤。
11. **`show_or_rebuild_main` 重建路径要求 `tauri.conf.json` 含 main 窗口配置**：`rebuild_main_window` 中若找不到则返回 Err（`window.rs:209-217`），导致轻量化后无法恢复界面。
12. **`set_main_webview_suspended` 在非 Windows 为空操作**（`window.rs:46-47`）：其他平台无 WebView2 挂起优化，直接忽略。
