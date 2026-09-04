//! 提权辅助子进程（--helper）
//!
//! 需要管理员权限的操作（改 MAC、设 DNS+DoH）通过"提权重启自身"完成：
//! 主进程用 ShellExecuteW(runas)/COM ICMLuaUtil 以管理员身份启动当前 exe，
//! 并附加 `--helper <op>` 参数。本模块在主进程入口（main.rs 顶部）最先拦截该参数，
//! 以 Rust 直调 Win32/winreg 完成操作（不再依赖 PowerShell），
//! 结果写入结果文件供主进程轮询读取。
//!
//! helper 进程不初始化 logger（避免与主进程跨进程写同一日志文件竞争），
//! 诊断信息收集进 [`HelperResult::logs`]，由主进程读取后统一落日志。

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct HelperResult {
    pub success: bool,
    pub message: String,
    pub op: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub logs: Vec<String>,
    /// 操作返回的完整明细（如 DNS 设置的 dnsSuccess/dnsFailed/dohAdded 等），供主进程透传给前端
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// helper 支持的操作。
#[derive(Debug, Clone, PartialEq)]
pub enum HelperOp {
    /// 设置 DNS + 启用 DoH（只处理 `targets` 名单内的适配器，主进程 resolve 后传入；
    /// `family` 为优化目标 "ipv4"/"ipv6"/"both"）
    Dns { targets: Vec<String>, family: String },
    /// 修改适配器 MAC（注册表 NetworkAddress + 重启网卡）
    Mac { guid: String, mac_no_dash: String },
}

/// 解析 `--helper <op> [op参数...] [--family <v>] --result <path>` 参数。
/// 未命中 helper 模式返回 Ok(None)；命中但参数非法返回 Err（调用方不应启动正常应用）。
pub fn parse_helper_args(args: &[String]) -> Result<Option<(HelperOp, Option<String>)>, String> {
    let Some(pos) = args.iter().position(|a| a == "--helper") else {
        return Ok(None);
    };
    let op = args
        .get(pos + 1)
        .ok_or_else(|| "helper 缺少操作类型".to_string())?;
    // 收集 op 之后、第一个 `--` 开头参数之前的所有位置参数（如 dns 的适配器名单）
    let mut positional: Vec<String> = Vec::new();
    let mut i = pos + 2;
    while i < args.len() && !args[i].starts_with("--") {
        positional.push(args[i].clone());
        i += 1;
    }
    // 遍历剩余参数，找 --family <v> 与 --result <path>（顺序无关，便于扩展）
    let mut result_path = None;
    let mut family = "both".to_string();
    while i < args.len() {
        match args[i].as_str() {
            "--family" => {
                family = args
                    .get(i + 1)
                    .ok_or_else(|| "--family 缺少取值".to_string())?
                    .clone();
                i += 2;
            }
            "--result" => {
                result_path = Some(
                    args.get(i + 1)
                        .ok_or_else(|| "--result 缺少路径".to_string())?
                        .clone(),
                );
                i += 2;
            }
            _ => i += 1,
        }
    }
    let parsed = match op.as_str() {
        "dns" => HelperOp::Dns { targets: positional, family },
        "mac" => {
            let guid = positional
                .first()
                .ok_or_else(|| "helper mac 缺少 GUID".to_string())?
                .clone();
            let mac_no_dash = positional
                .get(1)
                .ok_or_else(|| "helper mac 缺少 MAC".to_string())?
                .clone();
            HelperOp::Mac { guid, mac_no_dash }
        }
        other => return Err(format!("未知 helper 操作: {other}")),
    };
    Ok(Some((parsed, result_path)))
}

/// 执行 helper 操作并退出。返回进程退出码（0 成功 / 非 0 失败）。
pub fn run_helper(op: HelperOp, result_path: Option<String>) -> i32 {
    let mut logs: Vec<String> = Vec::new();
    let result = match &op {
        HelperOp::Dns { targets, family } => run_dns(targets, family, &mut logs),
        HelperOp::Mac { guid, mac_no_dash } => run_mac(guid, mac_no_dash, &mut logs),
    };
    if let Some(path) = result_path {
        write_result_file(&path, &result);
    }
    if result.success { 0 } else { 1 }
}

fn run_dns(targets: &[String], family: &str, logs: &mut Vec<String>) -> HelperResult {
    logs.push(format!(
        "helper: 开始设置DNS+DoH（目标适配器: {}，优化目标: {family}）",
        if targets.is_empty() { "无".to_string() } else { targets.join("、") }
    ));
    let v = crate::network::dns_setup::setup_dns_doh_admin(targets, family);
    let success = v.get("success").and_then(|s| s.as_bool()).unwrap_or(false);
    let message = v
        .get("message")
        .and_then(|m| m.as_str())
        .unwrap_or("设置DNS+DoH完成")
        .to_string();
    HelperResult {
        success,
        message,
        op: "dns".to_string(),
        logs: std::mem::take(logs),
        details: Some(v),
    }
}

fn run_mac(guid: &str, mac_no_dash: &str, logs: &mut Vec<String>) -> HelperResult {
    logs.push(format!("helper: 开始修改MAC guid={guid}"));
    let adapters = match crate::network::get_adapters_force() {
        Ok(a) => a,
        Err(e) => {
            return HelperResult {
                success: false,
                message: format!("枚举适配器失败: {e}"),
                op: "mac".to_string(),
                logs: std::mem::take(logs),
                details: None,
            }
        }
    };
    let adapter = match adapters.iter().find(|a| a.guid.eq_ignore_ascii_case(guid)) {
        Some(a) => a,
        None => {
            return HelperResult {
                success: false,
                message: format!("未找到GUID对应的适配器: {guid}"),
                op: "mac".to_string(),
                logs: std::mem::take(logs),
                details: None,
            }
        }
    };
    match crate::network::dhcp::apply_mac_change_via_registry(guid, &adapter.name, mac_no_dash) {
        Ok(()) => HelperResult {
            success: true,
            message: format!("MAC已修改并重启网卡: {}", adapter.name),
            op: "mac".to_string(),
            logs: std::mem::take(logs),
            details: None,
        },
        Err(e) => HelperResult {
            success: false,
            message: e,
            op: "mac".to_string(),
            logs: std::mem::take(logs),
            details: None,
        },
    }
}

/// 原子写结果文件（先写临时文件再 rename），避免主进程读到半截内容。
fn write_result_file(path: &str, result: &HelperResult) {
    if let Ok(json) = serde_json::to_vec(result) {
        let tmp = format!("{path}.tmp");
        if std::fs::write(&tmp, &json).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_no_helper_returns_none() {
        let args = vec!["--autostart".to_string()];
        assert!(matches!(parse_helper_args(&args), Ok(None)));
    }

    #[test]
    fn parse_dns_with_result() {
        let args = vec![
            "--helper".to_string(),
            "dns".to_string(),
            "--result".to_string(),
            r"C:\Users\test\Temp\r.json".to_string(),
        ];
        let (op, path) = parse_helper_args(&args).unwrap().unwrap();
        assert_eq!(op, HelperOp::Dns { targets: vec![], family: "both".to_string() });
        assert_eq!(path.as_deref(), Some(r"C:\Users\test\Temp\r.json"));
    }

    #[test]
    fn parse_dns_with_targets_and_family() {
        let args = vec![
            "--helper".to_string(),
            "dns".to_string(),
            "以太网".to_string(),
            "Wi-Fi".to_string(),
            "--family".to_string(),
            "ipv6".to_string(),
            "--result".to_string(),
            "r.json".to_string(),
        ];
        let (op, path) = parse_helper_args(&args).unwrap().unwrap();
        assert_eq!(
            op,
            HelperOp::Dns {
                targets: vec!["以太网".to_string(), "Wi-Fi".to_string()],
                family: "ipv6".to_string(),
            }
        );
        assert_eq!(path.as_deref(), Some("r.json"));
    }

    #[test]
    fn parse_mac() {
        let args = vec![
            "--helper".to_string(),
            "mac".to_string(),
            "{ABC}".to_string(),
            "02A1B2C3D4E5".to_string(),
            "--result".to_string(),
            "r.json".to_string(),
        ];
        let (op, path) = parse_helper_args(&args).unwrap().unwrap();
        assert_eq!(
            op,
            HelperOp::Mac { guid: "{ABC}".to_string(), mac_no_dash: "02A1B2C3D4E5".to_string() }
        );
        assert_eq!(path.as_deref(), Some("r.json"));
    }

    #[test]
    fn parse_mac_missing_args_is_err() {
        let args = vec!["--helper".to_string(), "mac".to_string(), "{ABC}".to_string()];
        assert!(parse_helper_args(&args).is_err());
    }

    #[test]
    fn parse_unknown_op_is_err() {
        let args = vec!["--helper".to_string(), "bogus".to_string()];
        assert!(parse_helper_args(&args).is_err());
    }

    #[test]
    fn helper_result_roundtrip() {
        let r = HelperResult {
            success: true,
            message: "ok".to_string(),
            op: "dns".to_string(),
            logs: vec!["a".to_string(), "b".to_string()],
            details: Some(serde_json::json!({ "dnsFailed": ["WLAN: err"] })),
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["success"], true);
        assert_eq!(v["message"], "ok");
        assert_eq!(v["op"], "dns");
        assert_eq!(v["logs"][0], "a");
        assert_eq!(v["details"]["dnsFailed"][0], "WLAN: err");
    }

    #[test]
    fn helper_result_omits_empty_logs_and_details() {
        let r = HelperResult {
            success: true,
            message: "ok".to_string(),
            op: "mac".to_string(),
            logs: vec![],
            details: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert!(v.get("logs").is_none());
        assert!(v.get("details").is_none());
    }
}
