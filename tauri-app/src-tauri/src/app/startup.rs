use tauri::{Manager, WindowEvent};
use crate::infra::state::AppState;

/// 构建 Tokio runtime
pub fn build_runtime(core_count: usize) -> tokio::runtime::Runtime {
    let worker_threads = core_count.clamp(2, 8);
    let max_blocking_threads = (core_count * 4).clamp(8, 64);

    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .max_blocking_threads(max_blocking_threads)
        .thread_name("campus-worker")
        .enable_all()
        .build()
        .unwrap_or_else(|e| {
            eprintln!("Failed to create Tokio runtime: {e}");
            std::process::exit(1);
        })
}

/// 运行 Tauri 应用
pub fn run(core_count: usize) {
    let browser_args = crate::platform::gpu::build_browser_args();
    std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", &browser_args);

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new()
            .with_handler(|app, shortcut, event| {
                crate::app::shortcut::handle_shortcut_event(app, shortcut, event);
            })
            .build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();        // [忽略错误] 窗口可能尚未初始化完成
                let _ = window.set_focus();   // [忽略错误] 窗口可能尚未初始化完成
                let _ = window.unminimize();  // [忽略错误] 窗口可能尚未初始化完成
            } else {
                // 窗口可能尚未创建（NSIS安装器自动启动时可能出现此情况），延迟重试
                let app_h = app.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    if let Some(window) = app_h.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                });
            }
        }))
        .manage(AppState::new())
        .setup(move |app| {
            setup_app(app, core_count)
        })
        .on_window_event(|window, event| {
            if matches!(event, WindowEvent::CloseRequested { .. }) {
                crate::app::shutdown::handle_window_close_event(window, event);
            }
            #[cfg(target_os = "windows")]
            if matches!(event, WindowEvent::Focused(_)) {
                crate::app::window::handle_window_focus_event(window, event);
            }
        })
        .invoke_handler(tauri::generate_handler![
            crate::commands::config_cmd::get_config,
            crate::commands::config_cmd::show_window,
            crate::commands::config_cmd::save_config,
            crate::commands::config_cmd::reset_config,
            crate::commands::config_cmd::export_config,
            crate::commands::config_cmd::import_config,
            crate::commands::login::do_login,
            crate::commands::login::do_logout,
            crate::commands::network_cmd::get_adapters,
            crate::commands::network_cmd::get_adapter_details,
            crate::commands::network_cmd::check_campus_status,
            crate::commands::network_cmd::check_portal_status,
            crate::commands::network_cmd::get_disabled_adapters,
            crate::commands::network_cmd::enable_adapter,
            crate::commands::network_cmd::dhcp_renew_all,
            crate::commands::network_cmd::dhcp_release_renew,
            crate::commands::network_cmd::dhcp_release_renew_adapter,
            crate::commands::network_cmd::check_network_quality,
            crate::commands::network_cmd::start_latency_test,
            crate::commands::network_cmd::stop_latency_test,
            crate::commands::network_cmd::check_dns_doh_status,
            crate::commands::network_cmd::enable_doh_for_dns,
            crate::commands::network_cmd::setup_dns_doh,
            crate::commands::account::list_accounts,
            crate::commands::account::switch_account,
            crate::commands::account::save_current_as_account,
            crate::commands::account::delete_account,
            crate::commands::account::get_active_account,
            crate::commands::background::start_background_check,
            crate::commands::background::stop_background_check,
            crate::commands::background::trigger_background_check,
            crate::commands::background::get_background_status,
            crate::commands::system::get_auto_launch,
            crate::commands::system::set_auto_launch,
            crate::commands::system::get_notification_enabled,
            crate::commands::system::set_notification_enabled,
            crate::commands::system::send_notification,
            crate::commands::system::cancel_auto_exit,
            crate::commands::system::minimize_window,
            crate::commands::system::close_window,
            crate::commands::system::window_move,
            crate::commands::system::open_external,
            crate::commands::system::get_logs,
            crate::commands::system::clear_logs,
            crate::commands::system::get_init_data,
            crate::commands::system::render_heartbeat,
            crate::commands::system::get_gpu_info,
            crate::commands::system::set_log_retention_days,
            crate::commands::system::get_log_retention_days,
            crate::commands::updater::check_update,
            crate::commands::updater::download_update,
            crate::commands::updater::install_update,
            crate::commands::updater::get_mirror_urls,
            crate::infra::logger::set_debug_mode,
            crate::infra::logger::get_debug_mode,
        ]);

    app.run(tauri::generate_context!()).unwrap_or_else(|e| {
        crate::log_error!("startup", "TAURI 运行错误: {}", e);
        crate::infra::logger::flush();
        std::process::exit(1);
    });
}

fn setup_app(app: &mut tauri::App, core_count: usize) -> Result<(), Box<dyn std::error::Error>> {
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

    let config = {
        // 先完成 logger 初始化，再加载 config，避免 config 加载日志丢失
        crate::infra::logger::init_logger(log_dir.clone());
        let app_handle = app.handle().clone();
        crate::commands::config_cmd::load_config_from_disk_or_default(&app_handle)
    };

    crate::log_info!("startup", "应用启动, CPU核心: {}, 安装目录: {:?}, 日志目录: {:?}", core_count, install_dir, log_dir);
    crate::log_info!("app", "应用启动, 版本: v{}", env!("APP_VERSION"));

    let state = app.state::<AppState>();

    state.config.store(config.clone());
    crate::network::update_portal_url(&config.portal_url);

    crate::app::tray::build_tray(app.handle(), &install_dir)?;

    let app_h = app.handle().clone();
    if let Err(e) = crate::monitor::adapter_watch::start_adapter_watch(&app_h) {
        crate::log_warn!("startup", "启动适配器监听失败: {}", e);
    }
    if let Err(e) = crate::network::adapter_cache::start_cache_refresh_task(&state.task_manager) {
        crate::log_warn!("startup", "启动适配器缓存后台刷新失败: {}", e);
    }
    crate::update::updater::start_update_check_loop(&app_h);
    crate::monitor::watcher::run_startup_tasks(&app_h);

    crate::app::heartbeat::spawn_heartbeat_thread(app_h.clone());
    crate::app::heartbeat::spawn_window_safety_thread(app_h);

    Ok(())
}
