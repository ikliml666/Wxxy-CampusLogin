//! iOS 端 Tauri 入口:命令面与安卓/桌面版同名对齐,协议核心 path 依赖 campus-login 单点共享。
//! M0 骨架:壳工程可编译可启动;命令面注册与 iOS 平台件按 specs/ios-port-spec.md §5 在 M1 填充。
//! 分工(规划,M1 起):campus_detect(TCP 探测)/config_state(Keychain 加密落盘)/
//! self_service_cmds/monitor_loop(前台监测模式)/update_cmds(降级版本检查)。

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    // biometric 是 #![cfg(mobile)] 空 crate,host 下无 init,须同安卓放 cfg(mobile) 块
    #[cfg(mobile)]
    let builder = builder.plugin(tauri_plugin_biometric::init());
    let builder = builder
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // iOS 日志落应用容器目录(协议核心 init_logger,与安卓同款接入)
            let log_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join("logs");
            let _ = campus_login_lib::infra::logger::init_logger(log_dir);
            Ok(())
        });
    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
