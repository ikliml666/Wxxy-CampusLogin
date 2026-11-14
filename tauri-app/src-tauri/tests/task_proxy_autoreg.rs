//! 计划任务代理状态机的端到端契约验证（默认 ignored，本机手动执行）。
//!
//! 覆盖历史缺陷：状态机把「退避过期」当成「通道可用」，导致 ensure_registered
//! 永无执行机会——任务在一台机器上从未被自动建立过，每次提权都直接落到
//! CMSTPLUA/runas。
//!
//! 注意：不能在本测试进程里断言 check_registration_state()==Ready——action
//! 一致性校验以「当前进程 exe」为基准，测试进程的 exe 是 libtest harness，
//! 与任务 action 的主 exe 必然不同（真实场景由应用自身检测并重注册）。
//!
//! 运行：cargo test --test task_proxy_autoreg -- --ignored --nocapture
//! 验证后任务保留供应用日常使用；如需清理：管理员运行
//! schtasks /delete /tn CampusLoginPowerOps /f

use campus_login_lib::helper;
use campus_login_lib::platform::elevation::shell_exec_elevated;
use campus_login_lib::platform::task_proxy::{self, RegistrationProbe};
use std::time::{Duration, Instant};

fn task_exists() -> bool {
    std::process::Command::new("schtasks")
        .args(["/query", "/tn", "CampusLoginPowerOps"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
#[ignore = "真实删除并重注册计划任务，仅本机手动执行"]
fn missing_task_reports_needs_register_then_recovers() {
    let main_exe = env!("CARGO_BIN_EXE_campus-login");

    // 1. 删除任务并确认不存在（删除是异步的，轮询到 schtasks 查不到）
    shell_exec_elevated("schtasks.exe", "/delete /tn CampusLoginPowerOps /f", true)
        .expect("提权删除任务应成功");
    let deadline = Instant::now() + Duration::from_secs(10);
    while task_exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(300));
    }
    assert!(!task_exists(), "任务应已删除");

    // 2. 任务缺失必须报 NeedsRegister——spawn_elevated_helper 只在非 Disabled
    //    分支调用 ensure_registered；若这里返回 Disabled，代理永远不会被建立
    //    （历史缺陷正是如此）。
    task_proxy::invalidate_proxy_cache();
    assert_eq!(
        task_proxy::check_registration_state(),
        RegistrationProbe::NeedsRegister,
        "任务缺失时应为 NeedsRegister"
    );

    // 3. 经主 bin 注册（提权副本必须是主 exe 才能进 --helper 分支）
    let result_name = format!("r-autoreg-register-{}.json", std::process::id());
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
    assert!(task_exists(), "注册后任务应存在");
}
