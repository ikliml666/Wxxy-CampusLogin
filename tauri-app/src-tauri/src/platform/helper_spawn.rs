//! 提权 helper 启动与结果轮询（主进程侧）
//!
//! 非管理员需要提权的操作（改 MAC / 设 DNS+DoH）：以管理员身份重启当前 exe
//! 进入 `--helper` 模式（见 [`crate::helper`]），完成后把结果写入结果文件，
//! 本模块负责生成结果文件路径、启动提权副本（保留现有降级链：
//! COM ICMLuaUtil 静默优先 → 失败降级 ShellExecuteW runas 弹 UAC）、
//! 轮询结果文件并解析返回。

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// 生成唯一的临时结果文件路径（%TEMP%/campus-login-helper-<pid>-<ts>.json）。
pub fn unique_result_path() -> PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("campus-login-helper-{}-{ts}.json", std::process::id()))
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

/// 以管理员身份启动当前 exe 执行 `op`（--helper），轮询结果文件返回 helper 的 JSON 结果。
///
/// `args` 为 helper 操作的位置参数（不含操作名本身）。结果路径可能含空格，
/// 拼参数时用引号包裹。超时未收到结果返回 Err。
pub fn spawn_elevated_helper(
    op: &str,
    args: &[&str],
    result_path: &Path,
    timeout: Duration,
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
    params.push_str(&format!(" --result \"{}\"", result_path.display()));

    // 保留现状降级链：COM ICMLuaUtil 静默提权优先，失败降级 ShellExecuteW runas（弹 UAC）
    if let Err(com_err) = crate::platform::elevation::shell_exec_elevated(exe_str, &params, true) {
        crate::log_warn!("helper", "COM提权失败: {}，降级到ShellExecuteW runas", com_err);
        crate::platform::elevation::run_elevated(exe_str, &params)?;
    }

    // 轮询结果文件（helper 原子写入后即被观察到）
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(content) = std::fs::read_to_string(result_path) {
            return read_helper_result(&content, result_path);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    // 超时后追加一次结果文件检查：UAC 等待/DHCP 慢路径可能恰好越过 deadline
    // 才完成写入，此时操作实际已成功，按正常结果返回而非误报超时失败
    if let Ok(content) = std::fs::read_to_string(result_path) {
        return read_helper_result(&content, result_path);
    }
    Err("提权操作超时，未收到helper结果".to_string())
}
