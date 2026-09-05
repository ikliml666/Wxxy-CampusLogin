//! 同步上下文驱动 async future 的公共工具
//!
//! 从 auth/portal.rs 的 block_on_http 抽取（BE-B-07）：
//! 原先 portal.rs / protocol.rs / dual_adapter_executor.rs / subnet.rs 各自裸调
//! `tauri::async_runtime::block_on`，行为不一致且未说明约束，统一到本入口。
//!
//! 优先使用当前 Tokio runtime handle（Handle::block_on 会设置 reactor guard，
//! 使 reqwest / surge_ping 等 future 能正常注册 IO 事件，这也是
//! run_background_check_blocking 内 block_on 驱动 spawn_blocking 任务的同款模式）。
//!
//! ⚠️ Err 分支（当前线程完全无 runtime 上下文，如 std::thread::scope 裸子线程）：
//! 必须用自持的兜底 Runtime 的 `Runtime::block_on` 驱动。不能退回
//! `tauri::async_runtime::block_on`——它在 `set(handle)` 注入后（应用启动即如此）
//! 内部退化为 tokio `Handle::block_on`，实测其驱动的 future 访问不到 runtime 的
//! timer/IO 资源：reqwest 在 send 首次 poll 时构造超时计时器
//! （`tokio::time::sleep` → `Handle::current()`）会直接 panic
//! "there is no reactor running"，叠加 panic=abort 使整个进程崩溃
//! （2026-09-05 注销流程崩溃事故根因）。

use std::sync::OnceLock;
use tokio::runtime::Runtime;

/// 无 runtime 上下文线程上的兜底驱动器（多线程 Runtime，enable_all）。
fn fallback_runtime() -> &'static Runtime {
    static FALLBACK: OnceLock<Runtime> = OnceLock::new();
    FALLBACK.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("创建兜底 Tokio runtime 失败")
    })
}

/// 在同步函数中运行一个 async future。
///
/// 注意：Ok 分支（当前线程持有 runtime context，如 Tokio worker / spawn_blocking
/// 线程）使用 `Handle::block_on`，该上下文内不能发生真正的阻塞等待嵌套；
/// 所有调用者必须通过 spawn_blocking 或在同步线程中调用。
pub fn block_on_sync<F: std::future::Future>(future: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.block_on(future),
        Err(_) => fallback_runtime().block_on(future),
    }
}
