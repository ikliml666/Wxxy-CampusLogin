//! 同步上下文驱动 async future 的公共工具
//!
//! 从 auth/portal.rs 的 block_on_http 抽取（BE-B-07）：
//! 原先 portal.rs / protocol.rs / dual_adapter_executor.rs / subnet.rs 各自裸调
//! `tauri::async_runtime::block_on`，行为不一致且未说明约束，统一到本入口。
//!
//! 优先使用当前 Tokio runtime handle（Handle::block_on 会设置 reactor guard，
//! 使 reqwest / surge_ping 等 future 能正常注册 IO 事件，这也是
//! run_background_check_blocking 内 block_on 驱动 spawn_blocking 任务的同款模式）。
//! 如果当前线程无 runtime 上下文（如纯测试线程），fallback 到 tauri 全局 runtime。

/// 在同步函数中运行一个 async future。
///
/// 注意：不能在 async worker 线程上直接调用（Handle::block_on 会 panic）。
/// 所有调用者必须通过 spawn_blocking 或在同步线程中调用；当前项目内调用点
/// （认证协议、Portal 检测、网关 ICMP 探测）均已验证运行在 spawn_blocking 线程内，
/// 此工具为统一入口，保证后续新增调用点行为一致。
pub fn block_on_sync<F: std::future::Future>(future: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.block_on(future),
        Err(_) => tauri::async_runtime::block_on(future),
    }
}
