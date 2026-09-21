//! 提权 helper 启动与结果轮询（主进程侧）
//!
//! 非管理员需要提权的操作（启用网卡 / 改 MAC / 设 DNS+DoH）有三条执行通道，
//! 按可用性自动降级：
//! 1. **计划任务代理**（首选，全程无 UAC）：`platform/task_proxy.rs` 注册的
//!    SYSTEM 主体任务拉起自身 exe 进 `--helper-task` worker 模式；
//! 2. **CMSTPLUA COM 静默提权**：代理不可用时以管理员身份重启当前 exe 进入
//!    `--helper` 模式（`crate::helper`）；
//! 3. **ShellExecuteW runas**（弹 UAC 兜底）。
//!
//! 三条通道共用同一套结果回写协议：结果写入固定目录 `results\` 下的纯文件名
//! （P0-2 收口：worker/提权副本以 SYSTEM 或管理员身份运行，绝不接受任意路径），
//! 本模块负责生成结果文件名、启动提权副本、轮询结果文件并解析返回。

use std::path::Path;
use std::time::{Duration, Instant};

use crate::platform::task_proxy::RegistrationProbe;

/// 生成唯一的结果文件名（`r-<pid>-<ts>.json`，写入固定结果目录）。
pub fn new_result_name() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("r-{}-{ts}.json", std::process::id())
}

/// 解析 helper 结果文件内容：并入诊断日志、删除结果文件并返回 JSON。
fn read_helper_result(content: &str, result_path: &Path) -> Result<serde_json::Value, String> {
    let v: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| format!("解析helper结果文件失败: {e}"))?;
    if let Some(logs) = v.get("logs").and_then(|l| l.as_array()) {
        for l in logs {
            if let Some(s) = l.as_str() {
                crate::log_info!("helper", "{s}");
            }
        }
    }
    let _ = std::fs::remove_file(result_path);
    Ok(v)
}

/// 启动提权 helper 执行 `op`，轮询结果文件返回 JSON 结果。
///
/// 三级通道按可用性自动降级：计划任务代理（无 UAC）→ CMSTPLUA 静默 → runas 弹 UAC。
/// `allow_uac_prompt=false`（自动启用首试）时通道 2/3 的 runas 降级被禁用，
/// 失败即返回 Err 交调用方退避重试；代理通道本身从不弹 UAC。
/// `args` 为 helper 操作的位置参数（不含操作名本身，`--family` 等开关也放这里）。
pub fn spawn_elevated_helper(
    op: &str,
    args: &[&str],
    result_name: &str,
    timeout: Duration,
    allow_uac_prompt: bool,
) -> Result<serde_json::Value, String> {
    // 三级通道：计划任务代理（无 UAC）→ CMSTPLUA 静默 → runas 弹 UAC
    let proxy_usable = crate::platform::task_proxy::proxy_usable();
    if proxy_usable {
        let args_owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        match crate::platform::task_proxy::run_via_task(op, &args_owned, timeout) {
            Ok(v) => return Ok(v),
            Err(e) => {
                crate::log_warn!("helper", "计划任务代理触发失败: {e}，降级到 COM 提权");
                // 触发失败说明通道状态可疑：清缓存让下次重新检测
                crate::platform::task_proxy::invalidate_proxy_cache();
            }
        }
    } else if !matches!(
        crate::platform::task_proxy::check_registration_state(),
        RegistrationProbe::Disabled
    ) {
        // 未注册/不匹配：先尝试注册（内部经 CMSTPLUA/runas 提权一次），成功则走代理
        match crate::platform::task_proxy::ensure_registered() {
            Ok(()) => {
                let args_owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
                match crate::platform::task_proxy::run_via_task(op, &args_owned, timeout) {
                    Ok(v) => return Ok(v),
                    Err(e) => {
                        crate::log_warn!("helper", "代理注册成功但触发失败: {e}，降级到 COM 提权");
                        crate::platform::task_proxy::invalidate_proxy_cache();
                    }
                }
            }
            Err(e) => {
                crate::log_warn!("helper", "计划任务代理注册失败: {e}，降级到 COM 提权");
            }
        }
    }
    spawn_elevated_raw(op, args, result_name, timeout, allow_uac_prompt)
}

/// 直走「CMSTPLUA 静默 → runas 弹 UAC」提权链执行 helper（绕过计划任务代理）。
///
/// 供代理注册动作自身（`register_task` op，避免递归）与代理不可用时兜底使用。
/// `allow_uac_prompt=false`：COM 静默失败即返回 Err（自动启用首试不弹 UAC）。
pub fn spawn_elevated_raw(
    op: &str,
    args: &[&str],
    result_name: &str,
    timeout: Duration,
    allow_uac_prompt: bool,
) -> Result<serde_json::Value, String> {
    let exe = std::env::current_exe().map_err(|e| format!("获取当前exe路径失败: {e}"))?;
    let exe_str = exe
        .to_str()
        .ok_or_else(|| "当前exe路径非UTF-8".to_string())?;

    let mut params = format!("--helper {op}");
    for a in args {
        params.push(' ');
        // 参数统一加引号：适配器名可含空格（如 "以太网 2"），不加引号会被命令行拆碎
        params.push_str(&format!("\"{a}\""));
    }
    // 结果文件名经 worker 侧 resolve_result_path 收口到固定目录，杜绝任意路径写
    params.push_str(&format!(" --result \"{result_name}\""));

    // 保留现状降级链：COM ICMLuaUtil 静默提权优先，失败降级 ShellExecuteW runas（弹 UAC）
    if let Err(com_err) = crate::platform::elevation::shell_exec_elevated(exe_str, &params, true) {
        crate::log_warn!("helper", "COM提权失败: {}，降级到ShellExecuteW runas", com_err);
        if !allow_uac_prompt {
            return Err(format!("COM静默提权失败(首试不弹UAC，重试将降级): {com_err}"));
        }
        crate::platform::elevation::run_elevated(exe_str, &params)?;
    }

    // 轮询结果文件（固定目录收口；helper 原子写入后即被观察到）
    let result_path = crate::helper::resolve_result_path(result_name)?;
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(content) = std::fs::read_to_string(&result_path) {
            return read_helper_result(&content, &result_path);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    // 超时后追加一次结果文件检查：UAC 等待/DHCP 慢路径可能恰好越过 deadline
    // 才完成写入，此时操作实际已成功，按正常结果返回而非误报超时失败
    if let Ok(content) = std::fs::read_to_string(&result_path) {
        return read_helper_result(&content, &result_path);
    }
    Err("提权操作超时，未收到helper结果".to_string())
}
