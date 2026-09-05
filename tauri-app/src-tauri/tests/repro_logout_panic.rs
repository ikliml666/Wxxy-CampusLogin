//! 回归锁定：注销后"sleep(1s) + check_any_adapter_online 页面检测"路径的
//! scope 裸子线程必须先进入 runtime context（commands/login.rs check_one 修复），
//! 否则 reqwest send() 构造超时计时器时 Handle::current() panic
//! "there is no reactor running"，叠加 panic=abort 整进程崩溃。
//!
//! 需要校园网环境（真实 GET http://10.1.99.100/ 页面探测，只读无副作用）。

use std::time::Duration;

#[test]
fn repro_logout_page_check_panic() {
    // 模拟 main.rs build_runtime + tauri::async_runtime::set(handle)
    let rt = tokio::runtime::Runtime::new().unwrap();
    tauri::async_runtime::set(rt.handle().clone());

    // 模拟 do_logout 命令：async 上下文 → spawn_blocking 执行同步注销后处理
    let result = rt.block_on(async {
        tauri::async_runtime::spawn_blocking(move || {
            // 模拟 full_logout 成功后的 sleep(1s) + check_any_adapter_online
            std::thread::sleep(Duration::from_secs(1));

            // 模拟 check_any_adapter_online 的 scope 双子线程并行检测
            let (r1, r2) = std::thread::scope(|s| {
                // 与 commands/login.rs check_one 修复后一致：子线程先进入 runtime context
                let h1 = s.spawn(|| {
                    let _ctx = tauri::async_runtime::handle().inner().enter();
                    campus_login_lib::auth::portal::check_portal_full("10.2.106.187", Some("以太网"))
                        .map(|s| s.online)
                });
                let h2 = s.spawn(|| {
                    let _ctx = tauri::async_runtime::handle().inner().enter();
                    campus_login_lib::auth::portal::check_portal_full("10.2.106.187", Some("以太网 2"))
                        .map(|s| s.online)
                });
                (h1.join(), h2.join())
            });
            (r1, r2)
        })
        .await
        .unwrap()
    });

    println!("r1 = {:?}, r2 = {:?}", result.0, result.1);
    println!("复现测试完成（未 panic 则说明该结构本身安全，panic 源在其他新增改动）");
}
