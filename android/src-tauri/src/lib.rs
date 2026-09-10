//! 安卓端 Tauri 入口:命令面与桌面版同名对齐,协议核心 path 依赖 campus-login 单点共享。
//! 模块分工:
//! - protocol_cmds:登录/注销/Portal 探测/WiFi 绑定
//! - campus_detect:校园网探针(检测卡 + check_campus_status)
//! - config_state:全量配置(Keystore 加密落盘/掩码出口)
//! - self_service_cmds:自助服务六命令 + 生物识别验证门
//! - identity_gate / login_history:验证门 TTL 与登录历史
//! - android_state:进程态(源 IP 缓存/配置内存态/监控循环句柄)

mod campus_detect;
mod android_state;
mod config_state;
mod cpu_affinity;
mod identity_gate;
mod login_history;
mod protocol_cmds;
mod self_service_cmds;
mod monitor_loop;
mod account_cmds;
mod system_cmds;
mod quality_cmds;
mod update_cmds;

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
        let _ = campus_login_lib::infra::logger::init_logger(log_dir);
        // 启动恢复:按持久化配置拉起后台检测/定时质量测试循环并执行启动自动登录
        monitor_loop::run_startup_tasks(app.handle().clone());
        Ok(())
    });
    builder
        .invoke_handler(tauri::generate_handler![
            protocol_cmds::ping_test,
            protocol_cmds::bind_to_wifi,
            protocol_cmds::do_login,
            protocol_cmds::do_logout,
            protocol_cmds::check_portal_status,
            campus_detect::detect_campus,
            campus_detect::check_campus_status,
            config_state::get_config,
            config_state::save_config,
            self_service_cmds::verify_biometric_identity,
            self_service_cmds::bind_operator,
            self_service_cmds::query_bind_status,
            self_service_cmds::query_self_dashboard,
            self_service_cmds::query_self_online_log,
            self_service_cmds::self_offline_session,
            self_service_cmds::reveal_operator_credential,
            account_cmds::list_accounts,
            account_cmds::switch_account,
            account_cmds::save_current_as_account,
            account_cmds::delete_account,
            account_cmds::get_active_account,
            system_cmds::get_init_data,
            system_cmds::get_soc_info,
            system_cmds::get_logs,
            system_cmds::clear_logs,
            system_cmds::get_log_retention_days,
            system_cmds::set_log_retention_days,
            system_cmds::get_debug_mode,
            system_cmds::set_debug_mode,
            monitor_loop::start_background_check,
            monitor_loop::stop_background_check,
            monitor_loop::trigger_background_check,
            monitor_loop::get_background_status,
            monitor_loop::get_boot_autostart,
            monitor_loop::set_boot_autostart,
            monitor_loop::get_notification_enabled,
            monitor_loop::set_notification_enabled,
            quality_cmds::check_network_quality,
            quality_cmds::start_latency_test,
            quality_cmds::stop_latency_test,
            update_cmds::check_update,
            update_cmds::download_update,
            update_cmds::get_mirror_urls,
            update_cmds::install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
