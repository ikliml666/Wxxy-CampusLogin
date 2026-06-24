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

fn main() {
    // 注册 panic hook：panic=abort 时 hook 仍会执行，确保日志 flush
    // 使用 flush_quick（500ms 超时）避免 panic=abort 模式下阻塞进程终止 5s
    std::panic::set_hook(Box::new(|info| {
        eprintln!("panic: {}", info);
        crate::infra::logger::flush_quick();
    }));

    let core_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2);

    let runtime = app::startup::build_runtime(core_count);
    let handle = runtime.handle().clone();
    tauri::async_runtime::set(handle);
    app::startup::run(core_count);
    crate::infra::logger::flush();
    crate::infra::logger::shutdown();
    std::thread::sleep(std::time::Duration::from_millis(200));
    runtime.shutdown_timeout(std::time::Duration::from_secs(5));
}
