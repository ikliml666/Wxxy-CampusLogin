//! 安卓端 Tauri 入口(UI 进程)。
//!
//! 后台核心(巡检/自动登录/夜间切换/通知/WiFi watcher)只跑在 `:core` 进程,
//! 由 `core::runner` 在 `CoreService.onCreate` 经 JNI 拉起;本进程只做 UI 命令面,
//! 启动/停止/检测/登录/注销经 Binder 转发(见 `core_bridge_cmds`)。
//! 协议核心仍只在 `campus-login` 单点实现。

mod account_cmds;
mod android_state;
mod battery_cmds;
mod campus_detect;
mod config_state;
mod core;
mod core_bridge_cmds;
mod cpu_affinity;
mod face_model_cmds;
mod identity_gate;
mod login_history;
mod protocol_cmds;
mod quality_cmds;
mod quality_history;
mod self_service_cmds;
mod system_cmds;
mod update_cmds;
mod vpn_cmds;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(mobile)]
    let builder = builder
        .plugin(tauri_plugin_campus_network_bind::init())
        .plugin(tauri_plugin_campus_keystore::init())
        .plugin(tauri_plugin_campus_monitor_service::init())
        .plugin(tauri_plugin_biometric::init()) // 该 crate #![cfg(mobile)],host 编译为空
        .plugin(tauri_plugin_notification::init())
        .manage(android_state::AndroidState::default());
    #[cfg(not(mobile))]
    let builder = builder.manage(android_state::AndroidState::default());
    let builder = builder
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
        // 安卓日志落应用私有目录(get_log_dir 的 android 分支与之一致)
        let log_dir = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("logs");
        if let Err(e) = campus_login_lib::infra::logger::init_logger(log_dir) {
            eprintln!("[startup] 日志初始化失败,日志将不可用: {e}");
        }
        // UI 进程事件桥:core 广播 → WebView 事件
        crate::core::ui_bridge::set_app_handle(app.handle().clone());
        // UI 侧启动任务(质量首测/更新检查);后台检测等由 :core 自行恢复
        ui_startup(app.handle().clone());
        if let Ok(dir) = app.path().app_data_dir() {
            let _ = tauri::async_runtime::spawn(config_state::cleanup_stale_config_tmp(dir));
        }
        Ok(())
    });
    builder
        .invoke_handler(tauri::generate_handler![
            protocol_cmds::ping_test,
            protocol_cmds::bind_to_wifi,
            protocol_cmds::accept_wifi_network,
            protocol_cmds::get_avoid_bad_wifi_status,
            protocol_cmds::restore_written_settings,
            protocol_cmds::check_portal_status,
            campus_detect::check_campus_status,
            campus_detect::request_wifi_ssid_permission,
            core_bridge_cmds::do_login,
            core_bridge_cmds::do_logout,
            core_bridge_cmds::start_background_check,
            core_bridge_cmds::stop_background_check,
            core_bridge_cmds::trigger_background_check,
            core_bridge_cmds::get_background_status,
            core_bridge_cmds::get_boot_autostart,
            core_bridge_cmds::set_boot_autostart,
            core_bridge_cmds::get_notification_enabled,
            core_bridge_cmds::set_notification_enabled,
            config_state::get_config,
            config_state::save_config,
            self_service_cmds::verify_biometric_identity,
            self_service_cmds::bind_operator,
            self_service_cmds::query_bind_status,
            self_service_cmds::query_self_dashboard,
            self_service_cmds::query_self_online_log,
            self_service_cmds::self_offline_session,
            self_service_cmds::change_self_password,
            self_service_cmds::reveal_operator_credential,
            vpn_cmds::vpn_save_credential,
            vpn_cmds::vpn_credential_status,
            vpn_cmds::vpn_clear_credential,
            vpn_cmds::set_self_service_via_vpn,
            vpn_cmds::vpn_test_connection,
            vpn_cmds::vpn_verify_credential,
            account_cmds::list_accounts,
            account_cmds::switch_account,
            account_cmds::rename_account,
            account_cmds::save_current_as_account,
            account_cmds::create_account,
            account_cmds::delete_account,
            account_cmds::get_active_account,
            system_cmds::get_init_data,
            system_cmds::get_logs,
            system_cmds::clear_logs,
            system_cmds::get_log_retention_days,
            system_cmds::set_log_retention_days,
            system_cmds::set_log_level,
            system_cmds::set_refresh_rate_hint,
            battery_cmds::get_battery_optimization_info,
            battery_cmds::request_ignore_battery_optimizations,
            battery_cmds::open_vendor_battery_settings,
            battery_cmds::open_notification_settings,
            quality_cmds::check_network_quality,
            quality_cmds::start_latency_test,
            quality_cmds::stop_latency_test,
            update_cmds::check_update,
            update_cmds::download_update,
            update_cmds::get_mirror_urls,
            update_cmds::install_update,
            face_model_cmds::face_models_state,
            face_model_cmds::face_models_download,
        ])
        .build(tauri::generate_context!())
        .unwrap_or_else(|e| panic!("tauri 应用构建失败: {e:#}"))
        // UI 进程允许正常退出:后台核心由 :core 进程自持,WebView/UI 交给系统回收
        .run(|_app, _event| {});
}

/// UI 进程启动任务:质量首测与更新检查(后台检测/自动登录由 :core 负责)
fn ui_startup(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
        let settings = match crate::config_state::current_settings(&app).await {
            Ok(s) => s,
            Err(e) => {
                campus_login_lib::log_error!("monitor", "UI 启动任务读取配置失败: {e}");
                return;
            }
        };
        campus_login_lib::infra::logger::set_log_retention_days(settings.log_retention_days);
        // 日志等级以配置为准(默认 warning):init_logger 用默认等级起步,读到配置即应用
        campus_login_lib::infra::logger::set_log_level(
            campus_login_lib::infra::logger::log_level_from_config(&settings.log_level),
        );
        if settings.enable_network_quality {
            let enable_latency = settings.enable_latency_test;
            let app2 = app.clone();
            tauri::async_runtime::spawn(async move {
                if enable_latency {
                    let _ = crate::quality_cmds::start_latency_test(app2).await;
                } else {
                    let _ = crate::quality_cmds::check_network_quality(app2).await;
                }
            });
        }
        crate::update_cmds::start_update_check_loop(app.clone());
        crate::update_cmds::cleanup_stale_apks(&app).await;
    });
}