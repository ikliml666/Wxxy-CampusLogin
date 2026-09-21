//! 计划任务提权代理（Windows）
//!
//! 以 SYSTEM 主体 + RunLevel=Highest 注册一个哑任务（action 固定为自身 exe +
//! `--helper-task`），主进程经「写请求文件 + `schtasks /run` + 轮询结果文件」
//! 完成提权操作，全程无 UAC。注册本身需要管理员：经现有提权链（CMSTPLUA
//! 静默 → runas）拉起自身 exe 执行 `--helper register_task`，在提权副本内跑
//! COM 注册。
//!
//! 安全要点（架构评审定稿）：
//! - 注册用 `ITaskFolder::RegisterTaskDefinition` + 显式 SDDL
//!   `D:P(A;;GRGX;;;BU)(A;;FA;;;BA)(A;;FA;;;SY)`——普通用户（BU）仅可触发（GRGX），
//!   不可 `/change`、`/delete`（实测验证）；零触发器（仅按需运行，无过期语义）；
//! - 每次使用前校验任务 action 指向当前 exe 且参数为 `--helper-task`，
//!   防止旧路径被占位后 SYSTEM 执行任意代码；
//! - 请求/结果文件均在固定目录内、文件名随机，worker 侧另有路径收口
//!   （见 `helper::resolve_result_path`）。

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use windows::core::{BSTR, GUID, Interface, VARIANT};
use windows::Win32::Foundation::VARIANT_BOOL;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
use windows::Win32::System::TaskScheduler::{
    IActionCollection, IExecAction, ITaskDefinition, ITaskFolder, IRegisteredTask, ITaskService,
    ITaskSettings, TASK_ACTION_EXEC, TASK_CREATE_OR_UPDATE, TASK_INSTANCES_IGNORE_NEW,
    TASK_LOGON_SERVICE_ACCOUNT, TASK_RUNLEVEL_HIGHEST,
};
/// 计划任务名（根文件夹下，无层级）。
pub const TASK_NAME: &str = "CampusLoginPowerOps";
/// worker 模式的固定参数（注册进任务 action，查询时校验一致）。
pub const TASK_ARGS: &str = "--helper-task";
/// 任务 SDDL：BU（Users）仅触发权（GR=generic read GX=generic execute），
/// BA/SYSTEM 完全控制；P=受保护 DACL（拒绝继承注入）。
const TASK_SDDL: &str = "D:P(A;;GRGX;;;BU)(A;;FA;;;BA)(A;;FA;;;SY)";
/// CLSID_TaskScheduler（{0f87369f-a4e5-4cfc-bd3e-73e6154572dd}，taskschd.dll）。
const CLSID_TASK_SCHEDULER: GUID =
    GUID::from_values(0x0f87369f, 0xa4e5, 0x4cfc, [0xbd, 0x3e, 0x73, 0xe6, 0x15, 0x45, 0x72, 0xdd]);
/// 通道可用性缓存 TTL（ms）：避免每次提权操作都做 COM 查询。
const CHECK_TTL_MS: u64 = 30_000;
/// 注册失败后的退避（ms）：期间直接走降级链，禁止反复弹 UAC。
const REGISTER_RETRY_BACKOFF_MS: u64 = 600_000;

/// 代理通道当前可用性（进程内缓存）。
#[derive(Debug, Clone, Copy, PartialEq)]
enum ProxyState {
    /// 未检测。
    Unknown,
    /// 已注册且 action 匹配，可直接触发。
    Ready,
    /// 缺失/不匹配/触发失败——until 为可重试的 epoch ms（注册类失败给长退避）。
    Disabled { until_ms: u64 },
}

static PROXY_STATE: OnceLock<parking_lot::RwLock<(ProxyState, u64)>> = OnceLock::new();

fn proxy_cell() -> &'static parking_lot::RwLock<(ProxyState, u64)> {
    PROXY_STATE.get_or_init(|| parking_lot::RwLock::new((ProxyState::Unknown, 0)))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 生成代理通道内使用的唯一文件名（请求/结果/自检共用，防并发覆盖与预写伪造）。
pub fn unique_file_name(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{prefix}-{}-{nanos}.json", std::process::id())
}

/// 代理通道是否可用（供提权入口快速分路）。
///
/// Unknown 时做一次只读检测（COM 查询无需管理员）；Disabled 且未过退避期直接 false。
pub fn proxy_usable() -> bool {
    let cell = proxy_cell();
    let (state, _at) = *cell.read();
    let now = now_ms();
    match state {
        ProxyState::Ready => true,
        ProxyState::Disabled { until_ms } => now >= until_ms,
        ProxyState::Unknown => {
            drop(cell.read());
            // 只读检测（COM 查询无需管理员）：任务存在且 action 匹配 → Ready
            let usable = check_task_action().is_ok();
            let mut w = cell.write();
            if w.0 == ProxyState::Unknown {
                *w = (
                    if usable { ProxyState::Ready } else { ProxyState::Disabled { until_ms: now + CHECK_TTL_MS } },
                    now,
                );
            }
            matches!(w.0, ProxyState::Ready)
        }
    }
}

/// 经现有提权链注册任务并自检。注册与自检都通过才算通道可用。
///
/// 在普通（非管理员）主进程调用：内部提权拉起自身 exe 走 `register_task` op；
/// 已是管理员（罕见）则直接进程内注册。成功后把缓存置为 Ready。
pub fn ensure_registered() -> Result<(), String> {
    let reg = if crate::platform::elevation::is_admin() {
        register_task_via_com()
    } else {
        // 复用 helper 提权框架，但注册 op 强制走 CMSTPLUA/runas（此时代理尚未可用，
        // 不存在递归）；结果文件走固定目录收口。注册是提权基础设施，允许弹 UAC
        // 兜底（成功一次后长期免打扰）。
        let name = unique_file_name("r");
        let v = crate::platform::helper_spawn::spawn_elevated_raw(
            "register_task",
            &[],
            &name,
            Duration::from_secs(60),
            true,
        )?;
        if v.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
            Ok(())
        } else {
            Err(v
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("注册结果缺少 message")
                .to_string())
        }
    };
    reg?;

    // 自检：验证「普通进程触发 → SYSTEM worker 回写」全链路
    match run_via_task("selfcheck", &[], Duration::from_secs(20)) {
        Ok(v) if v.get("success").and_then(|s| s.as_bool()).unwrap_or(false) => {
            *proxy_cell().write() = (ProxyState::Ready, now_ms());
            Ok(())
        }
        Ok(v) => {
            disable_for_backoff();
            Err(format!(
                "代理自检未通过: {}",
                v.get("message").and_then(|m| m.as_str()).unwrap_or("无 message")
            ))
        }
        Err(e) => {
            disable_for_backoff();
            Err(format!("代理自检失败: {e}"))
        }
    }
}

/// 代理通道探测结果（供提权入口决定是否尝试注册）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RegistrationProbe {
    /// 已就绪可直接触发。
    Ready,
    /// 未注册/不匹配/退避到期——可尝试注册。
    NeedsRegister,
    /// 注册类失败退避中——跳过注册直接走降级链（防 UAC 弹窗风暴）。
    Disabled,
}

/// 探测代理注册状态（只读，不提权；Unknown 时做一次只读 COM 检测并缓存）。
pub fn check_registration_state() -> RegistrationProbe {
    let cell = proxy_cell();
    let (state, _at) = *cell.read();
    match state {
        ProxyState::Ready => RegistrationProbe::Ready,
        ProxyState::Disabled { until_ms } => {
            if now_ms() >= until_ms {
                RegistrationProbe::NeedsRegister
            } else {
                RegistrationProbe::Disabled
            }
        }
        ProxyState::Unknown => {
            drop(cell.read());
            match check_task_action() {
                Ok(()) => {
                    *cell.write() = (ProxyState::Ready, now_ms());
                    RegistrationProbe::Ready
                }
                Err(_) => RegistrationProbe::NeedsRegister,
            }
        }
    }
}

/// 清空代理状态缓存（触发失败等可疑场景下让下次重新检测）。
pub fn invalidate_proxy_cache() {
    *proxy_cell().write() = (ProxyState::Unknown, 0);
}

fn disable_for_backoff() {
    *proxy_cell().write() = (ProxyState::Disabled { until_ms: now_ms() + REGISTER_RETRY_BACKOFF_MS }, now_ms());
}

/// 触发任务执行一次 op（写请求 → schtasks /run → 轮询结果）。
/// 调用方需已确认通道可用（Ready）。
pub fn run_via_task(op: &str, args: &[String], timeout: Duration) -> Result<serde_json::Value, String> {
    let req_dir = crate::helper::helper_requests_dir();
    std::fs::create_dir_all(&req_dir).map_err(|e| format!("创建请求目录失败: {e}"))?;
    let result_name = unique_file_name("r");
    let request = serde_json::json!({ "op": op, "args": args, "result": result_name });
    // 请求文件唯一名 + 同目录 tmp rename 原子落盘
    let req_path = req_dir.join(unique_file_name("req"));
    let tmp = req_dir.join(unique_file_name("tmp"));
    std::fs::write(&tmp, request.to_string()).map_err(|e| format!("写请求文件失败: {e}"))?;
    std::fs::rename(&tmp, &req_path).map_err(|e| format!("请求文件落盘失败: {e}"))?;

    // 普通权限触发（SDDL 已授 BU 触发权）；被拒即通道失效，交调用方降级
    let output = crate::network::discovery::new_command("schtasks")
        .args(["/run", "/tn", TASK_NAME])
        .output()
        .map_err(|e| format!("schtasks 执行失败: {e}"))?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&req_path);
        let detail = crate::platform::console_output::decode_console_bytes(&output.stderr);
        let detail = detail.trim();
        return Err(if detail.is_empty() {
            format!("schtasks /run 返回非零退出码: {}", output.status)
        } else {
            format!("schtasks /run 失败: {detail}")
        });
    }

    // 轮询结果文件（worker 写完即被观察到；超时后兜底再查一次，防越过 deadline）
    let result_path = crate::helper::helper_results_dir().join(&result_name);
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(content) = std::fs::read_to_string(&result_path) {
            let _ = std::fs::remove_file(&result_path);
            let v: serde_json::Value = serde_json::from_str(&content)
                .map_err(|e| format!("解析代理结果失败: {e}"))?;
            if let Some(logs) = v.get("logs").and_then(|l| l.as_array()) {
                for l in logs {
                    if let Some(s) = l.as_str() {
                        crate::log_info!("task_proxy", "{s}");
                    }
                }
            }
            return Ok(v);
        }
        if Instant::now() >= deadline {
            let _ = std::fs::remove_file(&req_path);
            return Err("代理触发超时，未收到 worker 结果".to_string());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// 只读查询任务（不提权）。Ok(()) 表示任务存在且 action 匹配当前 exe。
fn check_task_action() -> Result<(), String> {
    let _com = ComScope::new();
    unsafe {
    let task = query_task()?;
    let def: ITaskDefinition = task
        .Definition()
        .map_err(|e| format!("读取任务定义失败: {e}"))?;
    let actions: IActionCollection = def.Actions().map_err(|e| format!("读取任务动作失败: {e}"))?;
    let mut count = 0i32;
    actions.Count(&mut count).map_err(|e| format!("读取动作数量失败: {e}"))?;
    if count < 1 {
        return Err("任务无动作".to_string());
    }
    let action = actions
        .get_Item(1)
        .map_err(|e| format!("读取首个动作失败: {e}"))?;
    let exec: IExecAction = action
        .cast()
        .map_err(|_| "任务动作不是 ExecAction".to_string())?;
    let mut path = BSTR::new();
    exec.Path(&mut path).map_err(|e| format!("读取动作路径失败: {e}"))?;
    let mut args_bstr = BSTR::new();
    exec.Arguments(&mut args_bstr)
        .map_err(|e| format!("读取动作参数失败: {e}"))?;
    let exe = std::env::current_exe()
        .map_err(|e| format!("获取当前exe路径失败: {e}"))?
        .to_string_lossy()
        .to_string();
    if !path.to_string().eq_ignore_ascii_case(&exe) || args_bstr.to_string() != TASK_ARGS {
        return Err(format!(
            "任务 action 与当前 exe 不匹配: path={path} args={args_bstr}"
        ));
    }
    Ok(())
    }
}

/// 只读查询任务对象（不提权）。Err 表示任务不存在或服务不可达。
fn query_task() -> Result<IRegisteredTask, String> {
    let _com = ComScope::new();
    unsafe {
        let svc: ITaskService = CoCreateInstance(&CLSID_TASK_SCHEDULER, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| format!("创建 TaskScheduler COM 实例失败: {e}"))?;
        // 空 VARIANT = 本地机器 + 当前令牌
        svc.Connect(&VARIANT::default(), &VARIANT::default(), &VARIANT::default(), &VARIANT::default())
            .map_err(|e| format!("连接任务计划服务失败: {e}"))?;
        let folder: ITaskFolder = svc
            .GetFolder(&BSTR::from("\\"))
            .map_err(|e| format!("打开任务根文件夹失败: {e}"))?;
        folder
            .GetTask(&BSTR::from(TASK_NAME))
            .map_err(|e| format!("任务不存在: {e}"))
    }
}

/// 在提权上下文（管理员）内注册任务：SYSTEM 主体 + Highest + 零触发器 + 显式 SDDL。
/// 由 helper `register_task` op 调用（已处管理员上下文），或管理员直跑路径直接调用。
pub fn register_task_via_com() -> Result<(), String> {
    let _com = ComScope::new();
    unsafe {
        let svc: ITaskService = CoCreateInstance(&CLSID_TASK_SCHEDULER, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| format!("创建 TaskScheduler COM 实例失败: {e}"))?;
        svc.Connect(&VARIANT::default(), &VARIANT::default(), &VARIANT::default(), &VARIANT::default())
            .map_err(|e| format!("连接任务计划服务失败: {e}"))?;

        let def: ITaskDefinition = svc.NewTask(0).map_err(|e| format!("NewTask 失败: {e}"))?;
        let principal = def.Principal().map_err(|e| format!("读取 Principal 失败: {e}"))?;
        principal
            .SetUserId(&BSTR::from("S-1-5-18"))
            .map_err(|e| format!("设置任务主体失败: {e}"))?;
        principal
            .SetLogonType(TASK_LOGON_SERVICE_ACCOUNT)
            .map_err(|e| format!("设置 LogonType 失败: {e}"))?;
        principal
            .SetRunLevel(TASK_RUNLEVEL_HIGHEST)
            .map_err(|e| format!("设置 RunLevel 失败: {e}"))?;

        let settings: ITaskSettings = def.Settings().map_err(|e| format!("读取 Settings 失败: {e}"))?;
        settings
            .SetAllowDemandStart(VARIANT_BOOL(1))
            .map_err(|e| format!("设置 AllowDemandStart 失败: {e}"))?;
        settings
            .SetStopIfGoingOnBatteries(VARIANT_BOOL(0))
            .map_err(|e| format!("设置 StopIfGoingOnBatteries 失败: {e}"))?;
        settings
            .SetDisallowStartIfOnBatteries(VARIANT_BOOL(0))
            .map_err(|e| format!("设置 DisallowStartIfOnBatteries 失败: {e}"))?;
        settings
            .SetExecutionTimeLimit(&BSTR::from("PT5M"))
            .map_err(|e| format!("设置 ExecutionTimeLimit 失败: {e}"))?;
        settings
            .SetMultipleInstances(TASK_INSTANCES_IGNORE_NEW)
            .map_err(|e| format!("设置 MultipleInstances 失败: {e}"))?;

        let actions: IActionCollection = def.Actions().map_err(|e| format!("读取 Actions 失败: {e}"))?;
        actions.Clear().map_err(|e| format!("清空动作失败: {e}"))?;
        let action = actions
            .Create(TASK_ACTION_EXEC)
            .map_err(|e| format!("创建 ExecAction 失败: {e}"))?;
        let exec: IExecAction = action.cast().map_err(|e| format!("动作类型转换失败: {e}"))?;
        let exe = std::env::current_exe()
            .map_err(|e| format!("获取当前exe路径失败: {e}"))?;
        exec.SetPath(&BSTR::from(exe.to_string_lossy().as_ref()))
            .map_err(|e| format!("设置动作路径失败: {e}"))?;
        exec.SetArguments(&BSTR::from(TASK_ARGS))
            .map_err(|e| format!("设置动作参数失败: {e}"))?;

        let folder: ITaskFolder = svc
            .GetFolder(&BSTR::from("\\"))
            .map_err(|e| format!("打开任务根文件夹失败: {e}"))?;
        folder
            .RegisterTaskDefinition(
                &BSTR::from(TASK_NAME),
                &def,
                TASK_CREATE_OR_UPDATE.0,
                &VARIANT::default(),
                &VARIANT::default(),
                TASK_LOGON_SERVICE_ACCOUNT,
                &VARIANT::from(TASK_SDDL),
            )
            .map_err(|e| format!("注册任务失败: {e}"))?;
    }
    Ok(())
}

/// COM 初始化作用域：构造时 CoInitializeEx(APARTMENTTHREADED)，Drop 时配对释放。
struct ComScope {
    initialized: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真实链路冒烟（默认忽略，本机手动执行）：
    /// `cargo test task_proxy_register_and_selfcheck_smoke -- --ignored --nocapture`
    /// 经 CMSTPLUA 提权注册 SYSTEM 主体任务 → 自检（普通进程触发 + SYSTEM worker 回写）。
    /// 验证后如需清理：`schtasks /delete /tn CampusLoginPowerOps /f`（管理员）。
    #[test]
    #[ignore = "真实注册计划任务 + 自检，仅本机手动执行"]
    fn task_proxy_register_and_selfcheck_smoke() {
        ensure_registered().expect("计划任务代理注册 + 自检应成功");
        // 二次检测应直接命中 Ready 缓存
        assert!(proxy_usable(), "注册成功后 proxy_usable 应为 true");
    }
}

impl ComScope {
    fn new() -> Self {
        // 已初始化（S_FALSE 也算成功）才在 Drop 时释放；RPC_E_CHANGED_MODE 不释放
        let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
        Self { initialized }
    }
}

impl Drop for ComScope {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { CoUninitialize() };
        }
    }
}

/// 删除计划任务（卸载清理）。需管理员；失败返回 Err 由调用方决定是否告警。
pub fn delete_task() -> Result<(), String> {
    let output = crate::network::discovery::new_command("schtasks")
        .args(["/delete", "/tn", TASK_NAME, "/f"])
        .output()
        .map_err(|e| format!("schtasks 执行失败: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let detail = crate::platform::console_output::decode_console_bytes(&output.stderr);
        Err(format!("删除任务失败: {}", detail.trim()))
    }
}
