#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;
mod config;
mod network;
mod auth;
mod monitor;
mod account;
mod platform;
mod update;
mod infra;
mod helper;

fn main() {
    // 注册 panic hook：panic=abort 时 hook 仍会执行，确保日志 flush
    // 使用 flush_quick（500ms 超时）避免 panic=abort 模式下阻塞进程终止 5s
    // release 模式 windows_subsystem=windows，eprintln 不可见，必须用 log_error 写入日志文件
    std::panic::set_hook(Box::new(|info| {
        crate::log_error!("panic", "进程崩溃: {}", info);
        crate::infra::logger::flush_quick();
        eprintln!("panic: {info}");
    }));

    // helper 模式：主进程以管理员身份重启自身执行提权操作（改 MAC / 设 DNS+DoH）。
    // 必须在 Tauri Builder / 单实例 / 托盘等装配之前拦截并退出，不进入正常应用流程。
    // 参数非法时不启动正常应用（避免 --helper 参数被误传导致正常 UI 启动）。
    let args: Vec<String> = std::env::args().collect();
    match helper::parse_helper_args(&args) {
        Ok(Some((op, result_path))) => {
            std::process::exit(helper::run_helper(op, result_path));
        }
        Ok(None) => {}
        Err(e) => {
            eprintln!("helper 参数解析失败: {e}");
            std::process::exit(2);
        }
    }

    // 必须在 Tokio runtime 创建前设置：set_var 与 worker 线程并发读 env 存在竞态
    // （std::env::set_var 非线程安全），先设 env 再起线程
    let browser_args = crate::platform::gpu::build_browser_args();
    std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", &browser_args);

    let core_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2);

    let runtime = app::startup::build_runtime(core_count);
    let handle = runtime.handle().clone();
    tauri::async_runtime::set(handle);
    app::startup::run(core_count);
    crate::infra::logger::flush();
    crate::infra::logger::shutdown();
    runtime.shutdown_timeout(std::time::Duration::from_secs(5));
}
