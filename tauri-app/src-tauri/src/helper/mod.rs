//! 提权辅助子进程（--helper 命令行模式 / --helper-task 计划任务 worker 模式）
//!
//! 两种提权执行形态共用本模块的 op 分发与结果回写：
//! - `--helper <op> [参数] --result <文件名>`：主进程经提权（CMSTPLUA 静默/runas）
//!   启动管理员副本执行单次操作；
//! - `--helper-task`：计划任务代理（`platform/task_proxy.rs` 注册的 SYSTEM 主体任务）
//!   拉起本 exe，worker 扫描请求目录逐个执行 op，无需任何 UAC。
//!
//! 结果一律写入固定目录 `results\` 下的纯文件名（P0-2 收口：worker 以 SYSTEM 身份
//! 运行，绝不接受来自请求方的任意路径，杜绝任意路径写原语）。
//!
//! helper 进程不初始化 logger（避免与主进程跨进程写同一日志文件竞争），
//! 诊断信息收集进 [`HelperResult::logs`]，由主进程读取后统一落日志。

use serde::Serialize;

/// 代理根目录（请求/结果/自检标记的固定位置）。
pub fn proxy_base_dir() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("PROGRAMDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(r"C:\ProgramData"))
            .join("CampusLogin")
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::temp_dir().join("campus-login-proxy")
    }
}

/// 结果文件固定目录。worker 以 SYSTEM 身份只在此目录内写结果（P0-2 收口）。
pub fn helper_results_dir() -> std::path::PathBuf {
    proxy_base_dir().join("results")
}

/// 提权请求文件目录（计划任务 worker 扫描此目录）。
pub fn helper_requests_dir() -> std::path::PathBuf {
    proxy_base_dir().join("requests")
}

/// 校验结果文件名并拼接到固定结果目录。
///
/// 只接受纯文件名（拒绝路径分隔符、`..`、空串），杜绝请求方把 SYSTEM 写重定向到
/// 任意路径（如覆盖 hosts/系统 DLL）。非法时返回 Err。
pub fn resolve_result_path(name: &str) -> Result<std::path::PathBuf, String> {
    let invalid = || format!("结果文件名非法: {name:?}（要求纯文件名）");
    if name.is_empty() || name.len() > 128 {
        return Err(invalid());
    }
    if name.contains(['/', '\\']) || name.contains("..") || name.starts_with('.') {
        return Err(invalid());
    }
    if !name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.')) {
        return Err(invalid());
    }
    Ok(helper_results_dir().join(name))
}

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
    /// 恢复DNS自动获取（清除适配器级 DNS；`targets` 为适配器 GUID 名单，
    /// 主进程 resolve 后传入，复用 Dns 的位置参数传入方式）
    ClearDns { targets: Vec<String> },
    /// 修改适配器 MAC（注册表 NetworkAddress + 重启网卡）
    Mac { guid: String, mac_no_dash: String },
    /// 启用被禁用的网络适配器（netsh interface set interface enable）
    #[cfg(target_os = "windows")]
    EnableAdapter { name: String },
    /// PnP 设备级启用（pnputil /enable-device，含 problem 22 解除复核）
    #[cfg(target_os = "windows")]
    EnableDevice { instance_id: String },
    /// 设置接口跃点（出站切换）：`rows` 为 "{guid}:{family}:{automatic}:{metric}"
    /// 编码条目（family 2=IPv4/23=IPv6；automatic 1=恢复自动跃点、0=静态 metric）。
    /// 对 guid 匹配到的接口逐条 SetIpInterfaceEntry（SitePrefixLength 置 0 防
    /// ERROR_INVALID_PARAMETER，EasyTier/mullvad 同款实现）
    #[cfg(target_os = "windows")]
    SetMetric { rows: Vec<String> },
    /// 注册计划任务提权代理（仅计划任务 worker 内执行，本身处于提权上下文）
    #[cfg(target_os = "windows")]
    RegisterTaskProxy,
    /// 通道自检（写结果即通过，供代理注册后验证全链路）
    SelfCheck,
}

/// 从 op 名 + 参数数组构造 HelperOp（命令行 `--helper` 与计划任务请求 JSON 共用）。
///
/// `args` 内含位置参数与 `--family <v>` 开关（顺序无关，沿用命令行扫描语义）。
fn build_op_from_args(op: &str, args: &[String]) -> Result<HelperOp, String> {
    // 收集 op 之后、第一个 `--` 开头参数之前的所有位置参数（如 dns 的适配器名单）
    let mut positional: Vec<String> = Vec::new();
    let mut family = "both".to_string();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--family" => {
                family = args
                    .get(i + 1)
                    .ok_or_else(|| "--family 缺少取值".to_string())?
                    .clone();
                i += 2;
            }
            a if a.starts_with("--") => i += 1,
            a => {
                positional.push(a.to_string());
                i += 1;
            }
        }
    }
    let parsed = match op {
        "dns" => HelperOp::Dns { targets: positional, family },
        "clear_dns" => HelperOp::ClearDns { targets: positional },
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
        #[cfg(target_os = "windows")]
        "enable_adapter" => {
            let name = positional
                .first()
                .ok_or_else(|| "helper enable_adapter 缺少适配器名".to_string())?
                .clone();
            HelperOp::EnableAdapter { name }
        }
        #[cfg(target_os = "windows")]
        "enable_device" => {
            let instance_id = positional
                .first()
                .ok_or_else(|| "helper enable_device 缺少设备实例ID".to_string())?
                .clone();
            HelperOp::EnableDevice { instance_id }
        }
        #[cfg(target_os = "windows")]
        "set_metric" => HelperOp::SetMetric { rows: positional },
        #[cfg(target_os = "windows")]
        "register_task" => HelperOp::RegisterTaskProxy,
        "selfcheck" => HelperOp::SelfCheck,
        other => return Err(format!("未知 helper 操作: {other}")),
    };
    Ok(parsed)
}

/// 解析 `--helper <op> [op参数...] [--family <v>] --result <文件名>` 参数。
/// 未命中 helper 模式返回 Ok(None)；命中但参数非法返回 Err（调用方不应启动正常应用）。
pub fn parse_helper_args(args: &[String]) -> Result<Option<(HelperOp, Option<String>)>, String> {
    let Some(pos) = args.iter().position(|a| a == "--helper") else {
        return Ok(None);
    };
    let op = args
        .get(pos + 1)
        .ok_or_else(|| "helper 缺少操作类型".to_string())?;
    // op 之后到 --result 之间的参数交给统一构造器扫描（位置参数 + --family）
    let mut rest: Vec<String> = Vec::new();
    let mut result_path = None;
    let mut i = pos + 2;
    while i < args.len() {
        match args[i].as_str() {
            "--result" => {
                result_path = Some(
                    args.get(i + 1)
                        .ok_or_else(|| "--result 缺少路径".to_string())?
                        .clone(),
                );
                i += 2;
            }
            a => {
                rest.push(a.to_string());
                i += 1;
            }
        }
    }
    let parsed = build_op_from_args(op, &rest)?;
    Ok(Some((parsed, result_path)))
}

/// 执行 helper 操作并退出。返回进程退出码（0 成功 / 非 0 失败）。
pub fn run_helper(op: HelperOp, result_path: Option<String>) -> i32 {
    let mut logs: Vec<String> = Vec::new();
    let result = match &op {
        HelperOp::Dns { targets, family } => run_dns(targets, family, &mut logs),
        HelperOp::ClearDns { targets } => run_clear_dns(targets, &mut logs),
        HelperOp::Mac { guid, mac_no_dash } => run_mac(guid, mac_no_dash, &mut logs),
        #[cfg(target_os = "windows")]
        HelperOp::EnableAdapter { name } => run_enable_adapter(name, &mut logs),
        #[cfg(target_os = "windows")]
        HelperOp::EnableDevice { instance_id } => run_enable_device(instance_id, &mut logs),
        #[cfg(target_os = "windows")]
        HelperOp::SetMetric { rows } => run_set_metric(rows, &mut logs),
        #[cfg(target_os = "windows")]
        HelperOp::RegisterTaskProxy => run_register_task(&mut logs),
        HelperOp::SelfCheck => HelperResult {
            success: true,
            message: "selfcheck ok".to_string(),
            op: "selfcheck".to_string(),
            logs,
            details: None,
        },
    };
    if let Some(name) = result_path {
        match resolve_result_path(&name) {
            Ok(path) => write_result_file(&path, &result),
            Err(e) => eprintln!("helper 结果路径非法: {e}"),
        }
    }
    if result.success { 0 } else { 1 }
}

// === 计划任务 worker（--helper-task） ===

/// 计划任务代理的请求 JSON（worker 与主进程共享的文件协议）。
#[derive(Debug, serde::Deserialize)]
struct TaskRequest {
    op: String,
    #[serde(default)]
    args: Vec<String>,
    /// 结果文件名（纯文件名，worker 经 [`resolve_result_path`] 收口到固定目录）
    result: String,
}

/// 计划任务 worker 入口（main.rs 拦截 `--helper-task` 后调用）。
///
/// 扫描请求目录逐个执行（取最旧优先），处理完后再空扫两轮（兜住连续触发时
/// MultipleInstances=IgnoreNew 下第二个请求在首个实例运行期落盘的竞态），
/// 然后退出。绝不初始化 logger。
pub fn run_helper_task() -> i32 {
    let req_dir = helper_requests_dir();
    let mut exit_code = 0;
    let mut idle_rounds = 0;
    loop {
        let next = oldest_request(&req_dir);
        match next {
            Some(path) => {
                idle_rounds = 0;
                if process_request(&path) == 1 {
                    exit_code = 1;
                }
            }
            None => {
                idle_rounds += 1;
                if idle_rounds >= 3 {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(400));
            }
        }
    }
    exit_code
}

/// 取请求目录中最旧的 `req-*.json`（无则 None）。
fn oldest_request(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut entries: Vec<(std::time::SystemTime, std::path::PathBuf)> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let n = e.file_name();
            let n = n.to_string_lossy();
            n.starts_with("req-") && n.ends_with(".json")
        })
        .filter_map(|e| {
            let m = e.metadata().ok()?.modified().ok()?;
            Some((m, e.path()))
        })
        .collect();
    entries.sort_by_key(|(t, _)| *t);
    entries.into_iter().next().map(|(_, p)| p)
}

/// 处理单个请求：读入内存 → 立即删请求文件（防重复执行）→ 执行 → 写结果。
/// 返回该请求的退出码语义（0/1）。
fn process_request(path: &std::path::Path) -> i32 {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("helper-task 读取请求失败 {}: {e}", path.display());
            let _ = std::fs::remove_file(path);
            return 1;
        }
    };
    // 读入内存后立即删请求文件：之后的篡改/重建不影响本次执行
    let _ = std::fs::remove_file(path);
    let req: TaskRequest = match serde_json::from_str(&content) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("helper-task 请求 JSON 非法: {e}");
            return 1;
        }
    };
    let result = match (build_op_from_args(&req.op, &req.args), resolve_result_path(&req.result)) {
        (Ok(op), Ok(_result_path)) => {
            let mut logs: Vec<String> = Vec::new();
            let r = match &op {
                HelperOp::Dns { targets, family } => run_dns(targets, family, &mut logs),
                HelperOp::ClearDns { targets } => run_clear_dns(targets, &mut logs),
                HelperOp::Mac { guid, mac_no_dash } => run_mac(guid, mac_no_dash, &mut logs),
                #[cfg(target_os = "windows")]
                HelperOp::EnableAdapter { name } => run_enable_adapter(name, &mut logs),
                #[cfg(target_os = "windows")]
                HelperOp::EnableDevice { instance_id } => run_enable_device(instance_id, &mut logs),
                #[cfg(target_os = "windows")]
                HelperOp::SetMetric { rows } => run_set_metric(rows, &mut logs),
                #[cfg(target_os = "windows")]
                HelperOp::RegisterTaskProxy => run_register_task(&mut logs),
                HelperOp::SelfCheck => HelperResult {
                    success: true,
                    message: "selfcheck ok".to_string(),
                    op: "selfcheck".to_string(),
                    logs,
                    details: None,
                },
            };
            r
        }
        (Err(e), _) | (_, Err(e)) => HelperResult {
            success: false,
            message: e,
            op: req.op.clone(),
            logs: vec![],
            details: None,
        },
    };
    if let Ok(result_path) = resolve_result_path(&req.result) {
        write_result_file(&result_path, &result);
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

/// 恢复DNS自动获取（清除适配器级 DNS）。`targets` 为适配器 GUID 名单（主进程 resolve 后传入）。
/// 逐适配器结果明细写入 details.restored / details.failed（"名字: 错误"），供主进程解析失败明细；
/// 三态汇总文案（全成功 / 部分失败 / 全失败）与管理员路径（commands/network_cmd.rs reset_dns）保持一致。
fn run_clear_dns(targets: &[String], logs: &mut Vec<String>) -> HelperResult {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (targets, logs);
        HelperResult {
            success: false,
            message: "仅支持Windows".to_string(),
            op: "clear_dns".to_string(),
            logs: Vec::new(),
            details: None,
        }
    }
    #[cfg(target_os = "windows")]
    {
        logs.push(format!(
            "helper: 开始恢复DNS自动获取（目标适配器: {}）",
            if targets.is_empty() { "无".to_string() } else { targets.join("、") }
        ));
        let adapters = crate::network::get_adapters_force().unwrap_or_default();
        let mut restored: Vec<String> = Vec::new();
        let mut failed: Vec<String> = Vec::new();
        for guid in targets {
            // 日志与明细优先用适配器名，查不到时以 GUID 兜底（清除操作本身只依赖 GUID）
            let name = adapters
                .iter()
                .find(|a| a.guid.eq_ignore_ascii_case(guid))
                .map(|a| a.name.clone())
                .unwrap_or_else(|| guid.clone());
            match crate::platform::dns_config::clear_adapter_dns_via_api(guid) {
                Ok(()) => {
                    logs.push(format!("helper: 恢复DNS自动获取成功: {name}"));
                    restored.push(name);
                }
                Err(e) => {
                    logs.push(format!("helper: 恢复DNS自动获取失败: {name} - {e}"));
                    failed.push(format!("{name}: {e}"));
                }
            }
        }
        let success = failed.is_empty();
        let message = if failed.is_empty() {
            format!("已恢复DNS自动获取（{} 个适配器）", restored.len())
        } else if restored.is_empty() {
            format!("恢复DNS自动获取失败: {}", failed.join("；"))
        } else {
            format!("部分适配器恢复失败: {}", failed.join("；"))
        };
        HelperResult {
            success,
            message,
            op: "clear_dns".to_string(),
            logs: std::mem::take(logs),
            details: Some(serde_json::json!({ "restored": restored, "failed": failed })),
        }
    }
}

fn run_mac(guid: &str, mac_no_dash: &str, logs: &mut Vec<String>) -> HelperResult {
    // NetworkAddress 注册表值直接写裸 MAC，非法格式会静默写坏网卡配置
    if !(mac_no_dash.len() == 12 && mac_no_dash.bytes().all(|b| b.is_ascii_hexdigit())) {
        return HelperResult {
            success: false,
            message: format!("MAC 格式非法: {mac_no_dash}（要求 12 位十六进制字符、无分隔符）"),
            op: "mac".to_string(),
            logs: std::mem::take(logs),
            details: None,
        };
    }
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
        Ok(()) => {
            // 注册表 NetworkAddress 是持久伪装值：运行中的 MAC 不受清除影响，重启后恢复物理 MAC。
            // 必须在 helper 的管理员上下文内清除——主进程非提升时对 HKLM Class 键无写权限，
            // 仅靠主进程清理会让伪装 MAC 每次重启后持续生效且用户无从恢复。
            if let Err(e) = crate::network::dhcp::remove_mac_from_registry(guid) {
                logs.push(format!("helper: 清除MAC注册表伪装值失败: {e}（重启后将维持伪装MAC）"));
            }
            HelperResult {
                success: true,
                message: format!("MAC已修改并重启网卡: {}", adapter.name),
                op: "mac".to_string(),
                logs: std::mem::take(logs),
                details: None,
            }
        }
        Err(e) => HelperResult {
            success: false,
            message: e,
            op: "mac".to_string(),
            logs: std::mem::take(logs),
            details: None,
        },
    }
}

/// 解码一条 set_metric 编码条目 `"{guid}:{family}:{automatic}:{metric}"`，返回
/// （原始带花括号 GUID、协议栈 AF_INET=2/AF_INET6=23、是否自动跃点、静态跃点值）。
/// 条目来自 ProgramData 下任何本地用户可写的请求文件，且执行方是 SYSTEM：
/// 每个字段严格校验后才交给 Win32。
fn decode_set_metric_row(row: &str) -> Result<(String, u16, bool, u32), String> {
    let parts: Vec<&str> = row.split(':').collect();
    if parts.len() != 4 {
        return Err(format!(
            "set_metric 条目格式非法: {row:?}（要求 guid:family:automatic:metric）"
        ));
    }
    let family = match parts[1] {
        "2" => 2u16,
        "23" => 23u16,
        other => {
            return Err(format!(
                "set_metric 协议栈非法: {other:?}（要求 2=IPv4 / 23=IPv6）"
            ))
        }
    };
    let automatic = match parts[2] {
        "1" => true,
        "0" => false,
        other => {
            return Err(format!("set_metric automatic 标志非法: {other:?}（要求 0 或 1）"))
        }
    };
    let metric = parts[3]
        .parse::<u32>()
        .map_err(|e| format!("set_metric 跃点值非法: {:?} - {e}", parts[3]))?;
    Ok((parts[0].to_string(), family, automatic, metric))
}

/// 设置接口跃点（夜间出站切换）：逐条解码 → 复用读路径的接口枚举按 (guid, family)
/// 定位当前行 → 改副本后 SetIpInterfaceEntry。提权上下文内执行（主进程无接口写权限）。
/// 任一条失败即整体失败，失败明细汇总进 message 与 logs。
#[cfg(target_os = "windows")]
fn run_set_metric(rows: &[String], logs: &mut Vec<String>) -> HelperResult {
    use windows::Win32::Foundation::{BOOLEAN, WIN32_ERROR};
    use windows::Win32::NetworkManagement::IpHelper::SetIpInterfaceEntry;

    let fail = |message: String, logs: &mut Vec<String>| HelperResult {
        success: false,
        message,
        op: "set_metric".to_string(),
        logs: std::mem::take(logs),
        details: None,
    };
    // 空条目等于什么都没做，报成功会让调用方误判切换已生效
    if rows.is_empty() {
        return fail("set_metric 缺少条目".to_string(), logs);
    }
    logs.push(format!("helper: 开始设置接口跃点（{} 条）", rows.len()));

    let mut applied = 0usize;
    let mut failed: Vec<String> = Vec::new();
    for row in rows {
        let (guid, family, automatic, metric) = match decode_set_metric_row(row) {
            Ok(v) => v,
            Err(e) => {
                logs.push(format!("helper: {e}"));
                failed.push(e);
                continue;
            }
        };
        let candidates = match crate::platform::metric::interface_rows_for_guid(&guid) {
            Ok(rows) => rows,
            Err(e) => {
                let e = format!("接口枚举失败: guid={guid} - {e}");
                logs.push(format!("helper: {e}"));
                failed.push(e);
                continue;
            }
        };
        // SetIpInterfaceEntry 要求 Family/InterfaceLuid/InterfaceIndex 等为接口当前值，
        // 故直接取枚举到的整行改副本（等价 mullvad 的 GetIpInterfaceEntry + Set）
        let Some(mut target) = candidates.into_iter().find(|r| r.Family.0 == family) else {
            let e = format!("未找到接口行: guid={guid} family={family}");
            logs.push(format!("helper: {e}"));
            failed.push(e);
            continue;
        };
        target.UseAutomaticMetric = BOOLEAN(automatic as u8);
        target.Metric = metric;
        // SitePrefixLength 必须为 0，否则 SetIpInterfaceEntry 报 ERROR_INVALID_PARAMETER
        target.SitePrefixLength = 0;
        let rc = unsafe { SetIpInterfaceEntry(&mut target) };
        if rc != WIN32_ERROR(0) {
            let e = format!(
                "SetIpInterfaceEntry 失败: guid={guid} family={family} metric={metric} 错误码 {}",
                rc.0
            );
            logs.push(format!("helper: {e}"));
            failed.push(e);
        } else {
            applied += 1;
            logs.push(format!(
                "helper: 接口跃点已设置: guid={guid} family={family} automatic={automatic} metric={metric}"
            ));
        }
    }

    if failed.is_empty() {
        let message = format!("接口跃点已设置（{applied} 条）");
        logs.push(format!("helper: {message}"));
        HelperResult {
            success: true,
            message,
            op: "set_metric".to_string(),
            logs: std::mem::take(logs),
            details: None,
        }
    } else {
        fail(
            format!("接口跃点设置失败（{} 条失败）: {}", failed.len(), failed.join("；")),
            logs,
        )
    }
}

/// 启用被禁用的适配器（netsh enable）。worker 以 SYSTEM 身份运行。
/// 双校验：既有 validate_adapter_name + 必须存在于本机适配器列表（防参数注入）。
#[cfg(target_os = "windows")]
fn run_enable_adapter(name: &str, logs: &mut Vec<String>) -> HelperResult {
    if let Err(e) = crate::network::adapter_cache::validate_adapter_name(name) {
        return HelperResult {
            success: false,
            message: format!("适配器名校验失败: {e}"),
            op: "enable_adapter".to_string(),
            logs: std::mem::take(logs),
            details: None,
        };
    }
    let known = crate::network::get_adapters_force()
        .map(|list| list.iter().any(|a| a.name == name))
        .unwrap_or(false);
    if !known {
        return HelperResult {
            success: false,
            message: format!("未找到本机适配器: {name}"),
            op: "enable_adapter".to_string(),
            logs: std::mem::take(logs),
            details: None,
        };
    }
    logs.push(format!("helper: netsh 启用适配器: {name}"));
    let output = crate::network::discovery::new_command("netsh")
        .args(["interface", "set", "interface", name, "enable"])
        .output();
    let ok = matches!(&output, Ok(o) if o.status.success());
    let message = match output {
        Ok(o) if o.status.success() => format!("netsh 启用适配器成功: {name}"),
        Ok(o) => {
            let detail = crate::platform::console_output::decode_console_bytes(&o.stderr);
            let detail = detail.trim();
            if detail.is_empty() {
                "netsh 返回非零退出码但未输出错误信息".to_string()
            } else {
                format!("netsh 失败: {detail}")
            }
        }
        Err(e) => format!("netsh 执行失败: {e}"),
    };
    if ok {
        logs.push(format!("helper: {message}"));
    }
    HelperResult {
        success: ok,
        message,
        op: "enable_adapter".to_string(),
        logs: std::mem::take(logs),
        details: None,
    }
}

/// PnP 设备级启用（pnputil /enable-device），worker 内完成 problem 22 解除复核。
/// 双校验：设备实例 ID 字符集白名单（防 pnputil 开关注入，如 /remove-device）+
/// CM_Locate_DevNodeW(PHANTOM) 存在性复核。
#[cfg(target_os = "windows")]
fn run_enable_device(instance_id: &str, logs: &mut Vec<String>) -> HelperResult {
    // 字符集白名单：设备实例 ID 形如 USB\VID_0BDA&PID_8156\4013000001——
    // 拒绝 '/'（pnputil 开关前缀）、空格与控制字符；禁止以 '-' 开头
    let valid = !instance_id.is_empty()
        && instance_id.len() <= 200
        && !instance_id.starts_with('-')
        && instance_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '\\' | '&' | '.' | '_' | '-' | '*'));
    if !valid {
        return HelperResult {
            success: false,
            message: format!("设备实例 ID 非法: {instance_id:?}"),
            op: "enable_device".to_string(),
            logs: std::mem::take(logs),
            details: None,
        };
    }
    if crate::network::discovery::devnode::devnode_problem(instance_id).is_err() {
        return HelperResult {
            success: false,
            message: format!("设备实例不存在或不可定位: {instance_id}"),
            op: "enable_device".to_string(),
            logs: std::mem::take(logs),
            details: None,
        };
    }
    logs.push(format!("helper: pnputil 启用设备: {instance_id}"));
    let output = crate::network::discovery::new_command("pnputil")
        .args(["/enable-device", instance_id])
        .output();
    if let Err(e) = output {
        return HelperResult {
            success: false,
            message: format!("pnputil 执行失败: {e}"),
            op: "enable_device".to_string(),
            logs: std::mem::take(logs),
            details: None,
        };
    }
    // 复核：不信 pnputil 返回值，轮询 problem 22 真正解除（3s 上限，与主进程同参数）
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(3000);
    loop {
        match crate::network::discovery::devnode::devnode_problem(instance_id) {
            Ok(problem) if problem != 22 => {
                logs.push("helper: 设备级启用复核通过（已脱离 problem 22）".to_string());
                return HelperResult {
                    success: true,
                    message: format!("设备级启用成功: {instance_id}"),
                    op: "enable_device".to_string(),
                    logs: std::mem::take(logs),
                    details: None,
                };
            }
            _ if std::time::Instant::now() >= deadline => {
                return HelperResult {
                    success: false,
                    message: format!("pnputil 已执行但设备仍处于 PnP 禁用态: {instance_id}"),
                    op: "enable_device".to_string(),
                    logs: std::mem::take(logs),
                    details: None,
                };
            }
            _ => std::thread::sleep(std::time::Duration::from_millis(200)),
        }
    }
}

/// 注册计划任务提权代理（仅计划任务/提权副本上下文内调用——COM 注册需要管理员）。
#[cfg(target_os = "windows")]
fn run_register_task(logs: &mut Vec<String>) -> HelperResult {
    logs.push("helper: 注册计划任务提权代理".to_string());
    match crate::platform::task_proxy::register_task_via_com() {
        Ok(()) => HelperResult {
            success: true,
            message: "计划任务提权代理注册成功".to_string(),
            op: "register_task".to_string(),
            logs: std::mem::take(logs),
            details: None,
        },
        Err(e) => HelperResult {
            success: false,
            message: format!("计划任务注册失败: {e}"),
            op: "register_task".to_string(),
            logs: std::mem::take(logs),
            details: None,
        },
    }
}

/// 原子写结果文件（先写临时文件再 rename），避免主进程读到半截内容。
/// 结果目录首次可能不存在（固定目录而非 %TEMP%），写入前确保存在。
fn write_result_file(path: &std::path::Path, result: &HelperResult) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_vec(result) {
        let tmp = path.with_extension("json.tmp");
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
    fn parse_clear_dns_with_targets() {
        let args = vec![
            "--helper".to_string(),
            "clear_dns".to_string(),
            "{4D36E972-E325-11CE-BFC1-08002BE10318}".to_string(),
            "{ABC-002}".to_string(),
            "--result".to_string(),
            "r.json".to_string(),
        ];
        let (op, path) = parse_helper_args(&args).unwrap().unwrap();
        assert_eq!(
            op,
            HelperOp::ClearDns {
                targets: vec![
                    "{4D36E972-E325-11CE-BFC1-08002BE10318}".to_string(),
                    "{ABC-002}".to_string(),
                ]
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

    // === 结果路径收口（P0-2：SYSTEM worker 绝不接受任意路径写） ===

    #[test]
    fn resolve_result_path_accepts_plain_name() {
        let p = resolve_result_path("r-1234-1695.json").unwrap();
        assert!(p.starts_with(helper_results_dir()));
        assert!(p.to_string_lossy().ends_with("r-1234-1695.json"));
    }

    #[test]
    fn resolve_result_path_rejects_traversal_and_absolute() {
        for bad in [
            r"C:\Windows\System32\drivers\etc\hosts",
            r"..\..\evil.json",
            "sub/evil.json",
            r"sub\evil.json",
            "..",
            ".hidden.json",
            "",
        ] {
            assert!(resolve_result_path(bad).is_err(), "应拒绝: {bad:?}");
        }
    }

    // === build_op_from_args：命令行与计划任务请求共用 ===

    #[test]
    fn build_op_enable_adapter() {
        let op = build_op_from_args("enable_adapter", &["以太网 2".to_string()]).unwrap();
        assert_eq!(op, HelperOp::EnableAdapter { name: "以太网 2".to_string() });
    }

    #[test]
    fn build_op_enable_device() {
        let op = build_op_from_args(
            "enable_device",
            &["USB\\VID_0BDA&PID_8156\\4013000001".to_string()],
        )
        .unwrap();
        assert_eq!(
            op,
            HelperOp::EnableDevice {
                instance_id: "USB\\VID_0BDA&PID_8156\\4013000001".to_string()
            }
        );
    }

    #[test]
    fn build_op_selfcheck_and_register_task() {
        assert_eq!(build_op_from_args("selfcheck", &[]).unwrap(), HelperOp::SelfCheck);
        assert_eq!(
            build_op_from_args("register_task", &[]).unwrap(),
            HelperOp::RegisterTaskProxy
        );
    }

    #[test]
    fn build_op_unknown_is_err() {
        assert!(build_op_from_args("bogus", &[]).is_err());
    }

    // === set_metric（夜间出站切换）：编码条目 round-trip 与非法输入拒绝 ===

    #[test]
    fn set_metric_round_trip_and_reject() {
        let args = vec![
            "set_metric".to_string(),
            "{ABC-DEF}:2:0:1".to_string(),
            "{ABC-DEF}:23:1:0".to_string(),
        ];
        let (op, _) = parse_helper_args(
            &["--helper".to_string()]
                .iter()
                .chain(args.iter())
                .cloned()
                .collect::<Vec<_>>(),
        )
        .unwrap()
        .unwrap();
        match op {
            HelperOp::SetMetric { rows } => {
                assert_eq!(
                    rows,
                    vec!["{ABC-DEF}:2:0:1".to_string(), "{ABC-DEF}:23:1:0".to_string()]
                );
                // 解码器直接吃编码串（run_set_metric 内部同款解码逻辑抽成独立纯函数以便测试）
                let (guid, family, automatic, metric) = decode_set_metric_row(&rows[0]).unwrap();
                assert_eq!(
                    (guid.as_str(), family, automatic, metric),
                    ("{ABC-DEF}", 2u16, false, 1u32)
                );
                let (_, family6, automatic6, metric6) = decode_set_metric_row(&rows[1]).unwrap();
                assert_eq!((family6, automatic6, metric6), (23u16, true, 0u32));
            }
            other => panic!("wrong op: {other:?}"),
        }

        // 非法条目一律拒绝：字段数不足/多余、协议栈非 2|23、automatic 非 0|1、跃点非 u32
        for bad in [
            "{ABC-DEF}:2:0",
            "{ABC-DEF}:2:0:1:9",
            "{ABC-DEF}:6:0:1",
            "{ABC-DEF}:2:true:1",
            "{ABC-DEF}:2:0:-1",
            "{ABC-DEF}:2:0:abc",
            "{ABC-DEF}:2:0:99999999999",
            "",
        ] {
            assert!(decode_set_metric_row(bad).is_err(), "应拒绝: {bad:?}");
        }
    }

    #[test]
    fn task_request_json_roundtrip() {
        let req: TaskRequest = serde_json::from_str(
            r#"{"op":"enable_adapter","args":["以太网 2"],"result":"r-1-2.json"}"#,
        )
        .unwrap();
        assert_eq!(req.op, "enable_adapter");
        assert_eq!(req.args, vec!["以太网 2".to_string()]);
        assert_eq!(req.result, "r-1-2.json");
        // 缺省 args 允许
        let req: TaskRequest =
            serde_json::from_str(r#"{"op":"selfcheck","result":"r.json"}"#).unwrap();
        assert!(req.args.is_empty());
    }
}
