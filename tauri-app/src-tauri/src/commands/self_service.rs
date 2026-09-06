//! 自助服务系统相关命令（绑定运营商账号）

use std::net::IpAddr;

use serde_json::json;
use tauri::{Manager, State};

use crate::infra::state::{AppState, CommandResult};
use crate::self_service::{self, BindParams};

/// 解析校园网适配器源 IP（与登录同源规则：配置名有效 → 有线优先 → 任意有 IP），
/// 多网卡场景下保证自助服务请求从校园网侧发出
fn resolve_campus_bind_addr(state: &AppState) -> Option<IpAddr> {
    let adapters = crate::network::get_adapters_cached().ok()?;
    let config = state.config.load_full();
    let (a1_name, _a2_name) = crate::network::resolve_adapter_names(&adapters, &config);
    crate::network::find_with_valid_ip(&adapters, &a1_name)
        .and_then(|a| a.ip.parse().ok())
}

/// 绑定运营商账号（新手教程"绑定运营商账号"步骤 / 账户管理页绑定卡片）。
/// 凭据仅本次请求内存传递，不写入配置、不落盘、不写日志。
#[tauri::command]
pub async fn bind_operator(
    state: State<'_, AppState>,
    account: String,
    password: String,
    operator: String,
    phone: String,
    sms_password: String,
) -> Result<CommandResult, String> {
    let account = account.trim();
    let password = password.trim();
    let operator = operator.trim();
    let phone = phone.trim();
    let sms_password = sms_password.trim();

    if account.is_empty() {
        return Ok(CommandResult::err("请输入学号"));
    }
    if password.is_empty() {
        return Ok(CommandResult::err("请输入自助服务系统密码"));
    }
    if phone.len() != 11 || !phone.chars().all(|c| c.is_ascii_digit()) {
        return Ok(CommandResult::err("请输入 11 位手机号"));
    }
    if sms_password.is_empty() {
        return Ok(CommandResult::err("请输入运营商账户密码"));
    }

    let local_addr = resolve_campus_bind_addr(&state);
    if local_addr.is_none() {
        crate::log_warn!("self", "绑定运营商账号：未解析到有 IP 的校园网适配器，走系统默认路由");
    }

    let params = BindParams { account, password, operator, phone, sms_password };
    match self_service::bind_operator(&params, local_addr).await {
        Ok(msg) => {
            crate::log_info!("self", "绑定运营商账号成功: operator={}", operator);
            Ok(CommandResult::ok_msg(&msg))
        }
        Err(e) => {
            crate::log_warn!("self", "绑定运营商账号失败: operator={}, reason={}", operator, e);
            Ok(CommandResult::err(&e))
        }
    }
}

/// 查询各运营商绑定状态（手机号掩码返回，密码不返回明文，仅回是否设置）
#[tauri::command]
pub async fn query_bind_status(
    state: State<'_, AppState>,
    account: String,
    password: String,
) -> Result<CommandResult, String> {
    let account = account.trim();
    let password = password.trim();
    if account.is_empty() {
        return Ok(CommandResult::err("请输入学号"));
    }
    if password.is_empty() {
        return Ok(CommandResult::err("请输入自助服务系统密码"));
    }

    let local_addr = resolve_campus_bind_addr(&state);
    match self_service::query_bind_status(account, password, local_addr).await {
        Ok(bindings) => {
            let to_json = |b: &Option<self_service::OperatorBinding>| match b {
                Some(b) => json!({ "account": b.masked_account, "passwordSet": b.password_set }),
                None => serde_json::Value::Null,
            };
            let data = json!({
                "cmcc": to_json(&bindings[0]),
                "telecom": to_json(&bindings[1]),
                "unicom": to_json(&bindings[2]),
            });
            Ok(CommandResult { success: true, message: None, data: Some(data) })
        }
        Err(e) => {
            crate::log_warn!("self", "查询绑定状态失败: {e}");
            Ok(CommandResult::err(&e))
        }
    }
}

/// 查询自助服务 dashboard（在线信息 + 近期上网记录，一次登录拉两个接口，
/// 原始数组结构透传，展示格式化由前端完成）
#[tauri::command]
pub async fn query_self_dashboard(
    state: State<'_, AppState>,
    account: String,
    password: String,
) -> Result<CommandResult, String> {
    let account = account.trim();
    let password = password.trim();
    if account.is_empty() {
        return Ok(CommandResult::err("请输入学号"));
    }
    if password.is_empty() {
        return Ok(CommandResult::err("请输入自助服务系统密码"));
    }

    let local_addr = resolve_campus_bind_addr(&state);
    match self_service::query_dashboard(account, password, local_addr).await {
        Ok((online, history)) => Ok(CommandResult {
            success: true,
            message: None,
            data: Some(json!({ "onlineList": online, "loginHistory": history })),
        }),
        Err(e) => {
            crate::log_warn!("self", "查询自助服务在线信息失败: {e}");
            Ok(CommandResult::err(&e))
        }
    }
}

/// 注销指定自助服务在线会话（dashboard 在线信息卡片操作列）。
/// 凭据仅本次请求内存传递，不写入配置、不落盘、不写日志。
#[tauri::command]
pub async fn self_offline_session(
    state: State<'_, AppState>,
    account: String,
    password: String,
    session_id: String,
) -> Result<CommandResult, String> {
    let account = account.trim();
    let password = password.trim();
    let session_id = session_id.trim();
    if account.is_empty() {
        return Ok(CommandResult::err("请输入学号"));
    }
    if password.is_empty() {
        return Ok(CommandResult::err("请输入自助服务系统密码"));
    }
    if session_id.is_empty() {
        return Ok(CommandResult::err("缺少会话标识"));
    }

    let local_addr = resolve_campus_bind_addr(&state);
    match self_service::offline_session(account, password, session_id, local_addr).await {
        Ok(()) => {
            crate::log_info!("self", "自助服务注销会话成功");
            Ok(CommandResult::ok_msg("注销成功"))
        }
        Err(e) => {
            crate::log_warn!("self", "自助服务注销会话失败: {e}");
            Ok(CommandResult::err(&e))
        }
    }
}

/// 验证期间临时置顶主窗口的 RAII guard：Win11 下系统 Hello 弹窗（broker 进程）
/// 不主动抢前台，会留在应用窗口后面需要手动从任务栏点开（实测缺陷）；置顶主窗口
/// 把 Consent UI 顶到最前，验证结束（完成/取消/异常）自动恢复。
struct TopmostGuard(Option<tauri::WebviewWindow>);

impl TopmostGuard {
    fn new(win: Option<tauri::WebviewWindow>) -> Self {
        if let Some(w) = &win {
            let _ = w.set_always_on_top(true);
        }
        Self(win)
    }
}

impl Drop for TopmostGuard {
    fn drop(&mut self) {
        if let Some(w) = &self.0 {
            let _ = w.set_always_on_top(false);
        }
    }
}

/// Windows 本地身份验证（Windows Hello，未配置时回退 Windows 凭据对话框 +
/// SSPI 本地校验）。弹窗文案由前端按场景传入（i18n）；通过后记录后端验证时间戳
/// （时效 IDENTITY_VERIFY_TTL_SECS，reveal 等敏感操作在后端校验，防 webview 绕过）。
/// data.helloUsed=false 表示走的是凭据对话框回退（设备未配置 Hello），前端据此
/// 提示推荐开启 Windows Hello。
#[tauri::command]
pub async fn verify_windows_identity(
    app: tauri::AppHandle,
    consent_message: Option<String>,
) -> Result<CommandResult, String> {
    // 先把主窗口带到前台：系统弹窗的前台行为依赖调用方窗口状态
    let win = app.get_webview_window("main");
    if let Some(w) = &win {
        let _ = w.show();
        let _ = w.set_focus();
    }
    let message = consent_message.unwrap_or_default();
    // 弹窗/校验为阻塞调用，放独立线程避免占用 Tauri 异步运行时
    let result = tauri::async_runtime::spawn_blocking(move || {
        // 验证期间置顶主窗口（RAII，结束自动恢复），同时把主窗口 HWND 传给
        // 凭据对话框作模态父窗口（HWND 非 Send，跨线程传原始值）
        let _topmost = TopmostGuard::new(win.clone());
        let parent_hwnd = win.as_ref().and_then(|w| w.hwnd().ok()).map(|h| h.0 as isize);
        crate::platform::identity::verify_identity(&message, parent_hwnd)
    })
    .await
    .map_err(|e| format!("身份验证任务失败: {e}"))?;
    match result {
        Ok(hello_used) => {
            crate::platform::identity::note_identity_verified();
            Ok(CommandResult {
                success: true,
                message: None,
                data: Some(json!({ "helloUsed": hello_used })),
            })
        }
        Err(e) => Ok(CommandResult::err(&e)),
    }
}

/// 查看某运营商的明文凭据（手机号 + 运营商账户密码）。
/// 必须先通过 verify_windows_identity（后端校验时效 IDENTITY_VERIFY_TTL_SECS，
/// 时间戳由 verify_windows_identity 成功路径写入——验证与明文返回在后端关联，
/// 不依赖前端编排，webview 层无法绕过）；凭据仅本次响应返回，不落盘不写日志。
#[tauri::command]
pub async fn reveal_operator_credential(
    state: State<'_, AppState>,
    account: String,
    password: String,
    operator: String,
) -> Result<CommandResult, String> {
    let account = account.trim();
    let password = password.trim();
    let operator = operator.trim();
    if account.is_empty() || password.is_empty() {
        return Ok(CommandResult::err("请先填写学号与自助服务系统密码"));
    }
    if !crate::platform::identity::identity_verified_recently() {
        return Ok(CommandResult::err(
            "Windows 身份验证已过期，请重新验证后再查看",
        ));
    }

    let local_addr = resolve_campus_bind_addr(&state);
    match self_service::reveal_credential(account, password, operator, local_addr).await {
        Ok((phone, sms_password)) => Ok(CommandResult {
            success: true,
            message: None,
            data: Some(json!({ "phone": phone, "smsPassword": sms_password })),
        }),
        Err(e) => Ok(CommandResult::err(&e)),
    }
}
