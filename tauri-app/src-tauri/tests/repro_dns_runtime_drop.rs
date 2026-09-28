//! 机制实证：hickory 同步 Resolver 内部持有 current-thread Tokio Runtime，
//! 其最后一份引用在 tokio async 上下文（worker 线程）被 drop 时，
//! tokio `blocking/shutdown.rs:51` panic（"Cannot drop a runtime in a context
//! where blocking is not allowed"）——2026-09-28 进程崩溃的根因。
//!
//! 四场景结论（tokio 1.52.2 / hickory-resolver 0.24.4 实证）：
//! 1. spawn_blocking 线程上 drop 裸 Runtime        → 安全
//! 2. spawn_blocking 线程上 drop hickory Resolver  → 安全（池化前的历史行为因此从未崩过）
//! 3. worker 线程上 drop 最后一份 Arc<Resolver>    → 必炸（池化后 dns_lookup 的触发形态）
//! 4. 普通 OS 线程上 drop                          → 安全
//!
//! 因此 network/dns.rs 的退役路径统一经 retire_resolvers 移交。
//! 本文件的"必炸"场景依赖 panic=unwind，release（panic=abort）下整体跳过。
#![cfg(not(panic = "abort"))]

use std::sync::Arc;

fn make_resolver() -> Arc<hickory_resolver::Resolver> {
    Arc::new(
        hickory_resolver::Resolver::new(
            hickory_resolver::config::ResolverConfig::default(),
            hickory_resolver::config::ResolverOpts::default(),
        )
        .unwrap(),
    )
}

/// 场景 2：spawn_blocking 线程上 drop Resolver 安全（池化前历史行为的守卫）
#[test]
fn drop_resolver_inside_spawn_blocking_is_safe() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let res = tokio::task::spawn_blocking(|| {
            let resolver = make_resolver();
            drop(resolver);
        })
        .await;
        assert!(res.is_ok(), "spawn_blocking 线程 drop 不应 panic: {res:?}");
    });
}

/// 场景 3：worker 线程上 drop 最后一份 Arc<Resolver> 必炸（守护测试——
/// 若未来 tokio 行为变化导致本测试失败，retire_resolvers 的移交前提需重新评估）
#[test]
fn drop_last_resolver_ref_on_worker_thread_panics() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let resolver = make_resolver();
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            drop(resolver);
        }));
        let err_text = res.err().map(|e| {
            e.downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default()
        });
        assert!(
            err_text
                .as_deref()
                .map(|t| t.contains("blocking"))
                .unwrap_or(false),
            "期望 blocking-not-allowed panic，实际: {err_text:?}"
        );
    });
}

/// 场景 4：普通 OS 线程上 drop 安全（retire_resolvers 无 runtime 上下文分支的前提）
#[test]
fn drop_last_resolver_ref_on_plain_thread_is_safe() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let resolver = rt.block_on(async {
        tokio::task::spawn_blocking(make_resolver).await.unwrap()
    });
    let handle = std::thread::spawn(move || drop(resolver));
    assert!(handle.join().is_ok(), "普通线程 drop 不应 panic");
}
