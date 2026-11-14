//! 常驻提权 worker（命名管道）的真机冒烟（默认 ignored）。
//!
//! 不能在本测试进程里调 ensure_running：它用 current_exe() 拉起提权副本，而
//! 测试进程的 exe 是 libtest harness，不走 main.rs 的 --helper-daemon 拦截。
//! 这里直接用主 bin 拉起 daemon，再用客户端协议请求。
//!
//! 运行：cargo test --test elevated_daemon_smoke -- --ignored --nocapture

use campus_login_lib::platform::elevated_daemon;
use campus_login_lib::platform::elevation::shell_exec_elevated;
use std::time::{Duration, Instant};

#[test]
#[ignore = "真实拉起管理员常驻进程并走命名管道，仅本机手动执行"]
fn daemon_roundtrip_via_main_exe() {
    let main_exe = env!("CARGO_BIN_EXE_campus-login");
    // 清掉可能残留的发现文件与旧 daemon
    elevated_daemon::shutdown();

    shell_exec_elevated(main_exe, "--helper-daemon", true).expect("CMSTPLUA 拉起 daemon 应成功");

    let deadline = Instant::now() + Duration::from_secs(20);
    let mut last_err;
    let reply = loop {
        match elevated_daemon::request("selfcheck", &[], Duration::from_secs(5)) {
            Ok(v) => break v,
            Err(e) => last_err = e,
        }
        assert!(
            Instant::now() < deadline,
            "daemon 20s 内未就绪，最后一次错误: {last_err}"
        );
        std::thread::sleep(Duration::from_millis(200));
    };
    assert_eq!(
        reply.get("success").and_then(|s| s.as_bool()),
        Some(true),
        "selfcheck 应成功: {reply}"
    );

    elevated_daemon::shutdown();
}
