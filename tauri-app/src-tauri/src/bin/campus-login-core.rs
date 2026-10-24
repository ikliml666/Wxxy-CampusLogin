//! 校园网无头登录核心（campus-login-core.exe）
//!
//! 由计划任务 `CampusLoginCoreBoot`（BOOT 触发 + SYSTEM 主体）开机拉起：
//! 在用户尚未登录 Windows（锁屏/停在登录界面）时完成校园网 Portal 登录。
//! 流程：读 `%ProgramData%\CampusLoginCore\core-config.json` → 机器作用域
//! DPAPI 解密密码 → 循环（总时长上限 10 分钟，每轮 5-10s 间隔）等适配器
//! 有 IP → 探测 Portal 在线状态 → 已在线退出 / 未在线且可登录则登录 →
//! 成功退出。全程只写独立日志 `core.log`，不触碰桌面端日志与配置。
//!
//! 不初始化桌面 logger（lib 内 log 宏在未初始化时安全降级），协议层日志
//! 因此静默——本进程自身的关键节点直接追加写 core.log。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::Write;
use chrono::Timelike;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use campus_login_lib::auth::portal::check_portal_full;
use campus_login_lib::auth::protocol::do_login_with_retry;
use campus_login_lib::platform::core_autostart::{
    core_config_path, core_log_path, select_core_adapters, CoreConfig,
};

/// 总循环时长上限：BOOT 后延迟 30s 启动，再留 10 分钟覆盖 DHCP 慢/Portal 冷启
const MAX_RUN_DURATION: Duration = Duration::from_secs(600);
/// 每轮探测间隔
const POLL_INTERVAL: Duration = Duration::from_secs(7);
/// do_login_with_retry 内部重试次数（沿用桌面端会话登录同款）
const LOGIN_RETRIES: u32 = 3;
/// is_quitting 常驻 false：core 以自然退出（成功/超时/ExecutionTimeLimit 兜底）为契约
static RUNNING: AtomicBool = AtomicBool::new(true);

fn log_line(msg: &str) {
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("[{ts}] [core] {msg}\n");
    // 追加失败无补救手段（无 UI、无通知），静默即可
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(core_log_path()) {
        let _ = f.write_all(line.as_bytes());
    }
}

/// 校园检查静默窗判断（与 monitor::campus_check::is_campus_check_silent 同语义：
/// start==0 无窗口；now<start 或 (end>start 且 now>=end) 视为静默）。
fn in_silent_window(now_minutes: u16, start: u16, end: u16) -> bool {
    if start == 0 {
        return false;
    }
    if now_minutes < start {
        return true;
    }
    end > start && now_minutes >= end
}

fn now_minutes() -> u16 {
    let t = chrono::Local::now();
    (t.hour() as u16) * 60 + t.minute() as u16
}

fn load_config() -> Result<CoreConfig, String> {
    let path = core_config_path();
    let content = std::fs::read_to_string(&path).map_err(|e| format!("读取配置失败 {}: {e}", path.display()))?;
    let cfg: CoreConfig = serde_json::from_str(&content).map_err(|e| format!("配置解析失败: {e}"))?;
    if cfg.user.trim().is_empty() || cfg.password_enc.is_empty() {
        return Err("配置缺少账号或密码密文".to_string());
    }
    Ok(cfg)
}

fn main() {
    log_line("==== 无头登录核心启动 ====");
    let code = run();
    log_line(&format!("==== 无头登录核心退出（code={code}）===="));
    // 简单按轮截断：日志超过 ~512KB 时重写为空，防长期开机累积
    if let Ok(meta) = std::fs::metadata(core_log_path()) {
        if meta.len() > 512 * 1024 {
            let _ = std::fs::write(core_log_path(), "");
        }
    }
    std::process::exit(code);
}

/// 返回进程退出码：0=已在线或登录成功，1=配置/不可恢复错误，2=超时未成功
fn run() -> i32 {
    let cfg = match load_config() {
        Ok(c) => c,
        Err(e) => {
            log_line(&format!("配置不可用，退出: {e}"));
            return 1;
        }
    };
    let password = match campus_login_lib::account::crypto::decrypt_machine(&cfg.password_enc) {
        Ok(p) => p,
        Err(e) => {
            log_line(&format!("密码机器作用域解密失败（可能需在桌面端重新开关一次以重写密文）: {e}"));
            return 1;
        }
    };
    // 协议层 Portal 地址是进程级全局（桌面端由配置加载写入），此处对齐
    campus_login_lib::network::client::update_portal_url(&cfg.portal_url);
    log_line(&format!(
        "配置就绪: user={} operator={} portal={} adapter1={:?} adapter2={:?} dual={} 静默窗={}-{}",
        cfg.user, cfg.operator, cfg.portal_url, cfg.adapter1, cfg.adapter2,
        cfg.dual_adapter, cfg.campus_check_start_minutes, cfg.campus_check_end_minutes
    ));

    let deadline = Instant::now() + MAX_RUN_DURATION;
    loop {
        if !RUNNING.load(Ordering::Acquire) {
            return 1;
        }
        if Instant::now() >= deadline {
            log_line("总时长上限已到，未完成登录，退出");
            return 2;
        }

        let adapters = campus_login_lib::network::adapter_cache::get_adapters_force()
            .unwrap_or_default();
        let picked = select_core_adapters(
            &adapters,
            &cfg.adapter1,
            &cfg.adapter2,
            cfg.dual_adapter,
        );
        if picked.is_empty() {
            log_line("暂无带 IP 的适配器，等待下一轮");
        } else {
            for a in &picked {
                log_line(&format!("探测适配器 {} ({})", a.name, a.ip));
                match check_portal_full(&a.ip, Some(&a.name)) {
                    Ok(status) if status.online => {
                        log_line(&format!("已在线（{}），无需登录，退出", status.message));
                        return 0;
                    }
                    Ok(status) if !status.login_available => {
                        log_line(&format!(
                            "Portal 不可登录（reachable={} online={} msg={}），等待下一轮",
                            status.reachable, status.online, status.message
                        ));
                    }
                    Ok(_) => {
                        if in_silent_window(
                            now_minutes(),
                            cfg.campus_check_start_minutes,
                            cfg.campus_check_end_minutes,
                        ) {
                            log_line("处于校园检查静默窗，本轮不发起登录");
                            break;
                        }
                        log_line(&format!("未在线，发起登录（适配器 {}）", a.name));
                        let result = do_login_with_retry(
                            &cfg.user,
                            &password,
                            &cfg.operator,
                            Some(a.ip.as_str()),
                            LOGIN_RETRIES,
                            &RUNNING,
                        );
                        // 与桌面端消费方式对齐：失败降级为含 success=false 的 Value
                        let result = result.unwrap_or_else(|e| {
                            serde_json::json!({ "success": false, "message": e })
                        });
                        let success = result.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
                        let message = result.get("message").and_then(|v| v.as_str()).unwrap_or("");
                        if success {
                            log_line(&format!("登录成功: {message}"));
                            return 0;
                        }
                        log_line(&format!("登录失败: {message}"));
                    }
                    Err(e) => {
                        log_line(&format!("Portal 探测异常: {e}"));
                    }
                }
            }
        }

        std::thread::sleep(POLL_INTERVAL);
    }
}
