#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod network;
mod http_timing;
mod crypto_utils;
mod logger;

use commands::AppState;
use commands::start_auto_exit;
use tauri::{Manager, Emitter};
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

fn main() {
    let core_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2);

    let worker_threads = core_count.clamp(2, 8);
    let max_blocking_threads = (core_count * 4).clamp(8, 64);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .max_blocking_threads(max_blocking_threads)
        .thread_name("campus-worker")
        .enable_all()
        .build()
        .unwrap_or_else(|e| {
            eprintln!("Failed to create Tokio runtime: {}", e);
            std::process::exit(1);
        });
    let handle = runtime.handle().clone();
    tauri::async_runtime::set(handle);
    run_app(core_count);
    crate::logger::flush();
    crate::logger::shutdown();
    std::thread::sleep(std::time::Duration::from_millis(200));
    runtime.shutdown_timeout(std::time::Duration::from_secs(5));
}

fn run_app(core_count: usize) {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new()
            .with_handler(|app, shortcut, event| {
                use tauri_plugin_global_shortcut::ShortcutState;
                if event.state() == ShortcutState::Pressed {
                    if let Ok(cancel_key) = commands::CANCEL_EXIT_SHORTCUT.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                        if *shortcut == cancel_key {
                            let app_h = app.clone();
                            tauri::async_runtime::spawn_blocking(move || {
                                let s = app_h.state::<AppState>();
                                let _ = commands::cancel_auto_exit_inner(&app_h, &s); // [忽略错误] 取消自动退出失败不影响快捷键处理
                            });
                        }
                    }
                }
            })
            .build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();        // [忽略错误] 窗口可能尚未初始化完成
                let _ = window.set_focus();   // [忽略错误] 窗口可能尚未初始化完成
                let _ = window.unminimize();  // [忽略错误] 窗口可能尚未初始化完成
            }
        }))
        .manage(AppState::new())
        .setup(move |app| {
            let data_dir = app.path().app_data_dir().unwrap_or_else(|_| {
                dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from("."))
            });
            if let Err(e) = std::fs::create_dir_all(&data_dir) {
                crate::log_warn!("main", "创建数据目录失败: {}", e);
            }

            let install_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            let log_dir = install_dir.join("logs");

            let config = std::thread::scope(|s| {
                let log_dir_clone = log_dir.clone();
                s.spawn(move || {
                    crate::logger::init_logger(log_dir_clone);
                });
                let app_handle = app.handle().clone();
                let config = s.spawn(move || {
                    commands::load_config_from_disk_or_default(&app_handle)
                });
                config.join().unwrap_or_default()
            });

            crate::log_info!("startup", "应用启动, CPU核心: {}, 安装目录: {:?}, 日志目录: {:?}", core_count, install_dir, log_dir);

            let state = app.state::<AppState>();

            state.config.store(Arc::new(config.clone()));
            crate::network::update_portal_url(&config.portal_url);

            let is_auto_start = std::env::args().any(|a| a == "--autostart");
            let show_window = !(is_auto_start && config.hidden_start);

            if show_window {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();      // [忽略错误] 窗口可能尚未初始化完成
                    let _ = window.set_focus(); // [忽略错误] 窗口可能尚未初始化完成
                }
            } else {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();      // [忽略错误] 窗口可能尚未初始化完成
                }
            }

            let show_item = MenuItemBuilder::with_id("show", "显示主窗口").build(app)?;
            let quick_login_item = MenuItemBuilder::with_id("quick-login", "快速登录").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "退出").build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&show_item)
                .item(&quick_login_item)
                .separator()
                .item(&quit_item)
                .build()?;

            let tray_icon = app.default_window_icon()
                .cloned()
                .or_else(|| {
                    let icon_path = install_dir.join("icons").join("icon.ico");
                    tauri::image::Image::from_path(&icon_path).ok()
                })
                .unwrap_or_else(|| {
                    tauri::image::Image::from_bytes(include_bytes!("../icons/icon.ico")).expect("embedded tray icon")
                });

            let _ = TrayIconBuilder::new() // [忽略错误] 托盘图标创建失败不影响应用运行
                .icon(tray_icon)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| {
                    match event.id().as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();      // [忽略错误] 窗口可能已关闭
                                let _ = window.set_focus(); // [忽略错误] 窗口可能已关闭
                            }
                        }
                        "quick-login" => {
                            let app_h = app.clone();
                            tauri::async_runtime::spawn_blocking(move || {
                                let s = app_h.state::<AppState>();
                                let _guard = match s.tasks.is_logging_in.try_acquire() {
                                    Some(g) => g,
                                    None => return,
                                };
                                let result = commands::full_login_inner(&s, &app_h, None);
                                let _ = app_h.emit("auto-login-result", serde_json::json!({
                                    "success": result.success,
                                    "message": result.message.clone().unwrap_or_default(),
                                }));

                                if result.success {
                                    let app_h2 = app_h.clone();
                                    tauri::async_runtime::spawn(async move {
                                        tokio::time::sleep(Duration::from_millis(500)).await;
                                        let s = app_h2.state::<AppState>();
                                        let cancel_token = s.tasks.bg_check_cancel.load().clone();
                                        commands::run_background_check(&app_h2, cancel_token).await;
                                    });

                                    let auto_exit = s.config.load().auto_exit_after_login;
                                    if auto_exit {
                                        let s2 = app_h.state::<AppState>();
                                        start_auto_exit(&app_h, &s2);
                                    }
                                }
                            });
                        }
                        "quit" => {
                            let s = app.state::<AppState>();
                            s.exit.is_quitting.store(true, Ordering::Release);
                            s.tasks.bg_check_cancel.load().cancel();
                            s.tasks.latency_cancel.load().cancel();
                            s.tasks.adapter_watch_cancel.load().cancel();
                            let app_h = app.clone();
                            tauri::async_runtime::spawn(async move {
                                // 短暂等待后台任务响应取消
                                tokio::time::sleep(Duration::from_millis(200)).await;
                                let s = app_h.state::<AppState>();
                                s.tasks.background_running.force_release();
                                s.tasks.latency_running.force_release();
                                s.tasks.adapter_watch_running.force_release();
                                app_h.exit(0);
                            });
                        }
                        _ => {}
                    }
                })
                .tooltip("校园网登录助手")
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { button, .. } = event {
                        if button == tauri::tray::MouseButton::Left {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                                let _ = window.unminimize();
                            }
                        }
                    }
                })
                .build(app);

            let app_handle = app.handle().clone();
            let app_h = app_handle.clone();
            let s = app_h.state::<AppState>();
            let adapter_watch_cancel = s.tasks.adapter_watch_cancel.load().clone();
            commands::start_adapter_watch(&app_h, adapter_watch_cancel);
            commands::start_update_check_loop(&app_h);
            commands::run_startup_tasks(&app_h);

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let s = window.state::<AppState>();
                let minimize_to_tray = s.config.load().minimize_to_tray;
                if minimize_to_tray {
                    api.prevent_close();
                    let _ = window.hide(); // [忽略错误] 窗口可能已关闭
                } else {
                    s.exit.is_quitting.store(true, Ordering::Release);
                    s.tasks.latency_cancel.load().cancel();
                    s.tasks.bg_check_cancel.load().cancel();
                    s.tasks.adapter_watch_cancel.load().cancel();
                    let app_h = window.app_handle().clone();
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        let s = app_h.state::<AppState>();
                        s.tasks.background_running.force_release();
                        s.tasks.latency_running.force_release();
                        s.tasks.adapter_watch_running.force_release();
                        app_h.exit(0);
                    });
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::config_cmd::get_config,
            commands::config_cmd::show_window,
            commands::config_cmd::save_config,
            commands::login::do_login,
            commands::login::do_logout,
            commands::network_cmd::get_adapters,
            commands::network_cmd::get_adapter_details,
            commands::network_cmd::check_portal_status,
            commands::network_cmd::get_disabled_adapters,
            commands::network_cmd::enable_adapter,
            commands::network_cmd::dhcp_renew_all,
            commands::network_cmd::dhcp_release_renew,
            commands::network_cmd::check_network_quality,
            commands::network_cmd::start_latency_test,
            commands::network_cmd::stop_latency_test,
            commands::network_cmd::check_dns_doh_status,
            commands::network_cmd::enable_doh_for_dns,
            commands::network_cmd::setup_dns_doh,
            commands::account::list_accounts,
            commands::account::switch_account,
            commands::account::save_current_as_account,
            commands::account::delete_account,
            commands::account::get_active_account,
            commands::background::start_background_check,
            commands::background::stop_background_check,
            commands::background::trigger_background_check,
            commands::background::get_background_status,
            commands::system::get_auto_launch,
            commands::system::set_auto_launch,
            commands::system::get_notification_enabled,
            commands::system::set_notification_enabled,
            commands::system::send_notification,
            commands::system::cancel_auto_exit,
            commands::system::minimize_window,
            commands::system::close_window,
            commands::system::window_move,
            commands::system::open_external,
            commands::system::get_logs,
            commands::system::clear_logs,
            commands::system::get_init_data,
    commands::updater::check_update,
    commands::updater::download_update,
    commands::updater::install_update,
    commands::updater::get_mirror_urls,
            crate::logger::set_debug_mode,
            crate::logger::get_debug_mode,
]);

    app.run(tauri::generate_context!()).unwrap_or_else(|e| {
        crate::log_error!("startup", "TAURI 运行错误: {}", e);
        crate::logger::flush();
        std::process::exit(1);
    });
}
