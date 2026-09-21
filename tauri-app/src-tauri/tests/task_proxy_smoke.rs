//! 计划任务提权代理的真实链路冒烟（默认 ignored，本机手动执行）。
//!
//! 必须用主 bin（campus-login.exe）执行 `--helper register_task`——测试二进制
//! 的入口是 libtest harness，不走 main.rs 的 --helper 拦截，无法执行注册逻辑。
//!
//! 运行：`cargo test --test task_proxy_smoke -- --ignored --nocapture`
//! （首次触发 CMSTPLUA 提权注册；验证后任务保留供应用日常使用，
//! 如需清理：管理员运行 `schtasks /delete /tn CampusLoginPowerOps /f`）

use campus_login_lib::helper;
use campus_login_lib::platform::elevation::shell_exec_elevated;
use campus_login_lib::platform::task_proxy;
use std::time::{Duration, Instant};

#[test]
#[ignore = "真实注册计划任务 + 自检，仅本机手动执行"]
fn register_and_selfcheck_smoke() {
    let main_exe = env!("CARGO_BIN_EXE_campus-login");

    // 1. 提权注册（CMSTPLUA 静默 → 本机已修复可用）
    let result_name = format!("r-smoke-register-{}.json", std::process::id());
    shell_exec_elevated(
        main_exe,
        &format!("--helper register_task --result \"{result_name}\""),
        true,
    )
    .expect("CMSTPLUA 提权发起注册应成功");
    let result_path = helper::resolve_result_path(&result_name).unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    let content = loop {
        if let Ok(c) = std::fs::read_to_string(&result_path) {
            break c;
        }
        assert!(Instant::now() < deadline, "注册结果 60s 未回写");
        std::thread::sleep(Duration::from_millis(100));
    };
    let _ = std::fs::remove_file(&result_path);
    let v: serde_json::Value = serde_json::from_str(&content).expect("注册结果应为 JSON");
    assert_eq!(
        v.get("success").and_then(|s| s.as_bool()),
        Some(true),
        "注册应成功: {v}"
    );

    // 2. 自检：普通进程触发 → SYSTEM worker 回写
    let v = task_proxy::run_via_task("selfcheck", &[], Duration::from_secs(20))
        .expect("自检触发应成功");
    assert_eq!(
        v.get("success").and_then(|s| s.as_bool()),
        Some(true),
        "自检应成功: {v}"
    );

    // 说明：不在此断言 check_registration_state()==Ready——action 一致性校验以
    // 「当前进程 exe」为基准，测试进程的 exe 与注册的主 exe 必然不同（属预期）；
    // 真实场景由应用自身检测（action 不匹配时自动重注册覆盖）。
}
