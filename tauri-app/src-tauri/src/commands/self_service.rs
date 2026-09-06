//! 自助服务系统相关命令（绑定运营商账号）

use std::net::IpAddr;

use serde_json::json;
use tauri::{Manager, State};

use crate::infra::state::{AppState, CommandResult};
use crate::self_service::{self, BindParams};

/// 改变外部状态的命令（绑定运营商 / 注销在线会话）的后端验证门：Hello 开启时
/// 要求后端 TTL 内验证通过（与 reveal 同源时间戳，webview 无法伪造）。查询类
/// 命令有意不设门——总览卡自动刷新依赖免验证拉取（数据本就存于本机，掩码属
/// 渲染层），补门会破坏该体验且不构成实际防线。
fn ensure_identity_gate(state: &AppState) -> Option<String> {
    if !state.config.load().self_hello_enabled {
        return None;
    }
    if crate::platform::identity::identity_verified_recently() {
        return None;
    }
    Some("Windows 身份验证已过期，请重新验证后再操作".to_string())
}

/// 解析校园网适配器源 IP（与登录同源规则：配置名有效 → 有线优先 → 任意有 IP），
/// 多网卡场景下保证自助服务请求从校园网侧发出
fn resolve_campus_bind_addr(state: &AppState) -> Option<IpAddr> {
    let adapters = crate::network::get_adapters_cached().ok()?;
    let config = state.config.load_full();
    let (a1_name, _a2_name) = crate::network::resolve_adapter_names(&adapters, &config);
    crate::network::find_with_valid_ip(&adapters, &a1_name)
        .and_then(|a| a.ip.parse().ok())
}

/// 严格 YYYY-MM-DD（月份 01-12、日期 01-31）：旧实现只查分隔符位置，
/// "----------" 也能通过并构造无效请求打到自助服务系统
fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    b.iter().enumerate().all(|(i, c)| if i == 4 || i == 7 { *c == b'-' } else { c.is_ascii_digit() })
        && matches!(s[5..7].parse::<u8>(), Ok(m) if (1..=12).contains(&m))
        && matches!(s[8..10].parse::<u8>(), Ok(d) if (1..=31).contains(&d))
}

/// 解析自助服务密码：前端传入非 MASK 明文优先（用户刚重输的新密码）；
/// 空/MASK 占位符时回退已保存的 config.self_password（加载时已 DPAPI 解密的
/// 内存明文，磁盘上加密存储）。两者皆无返回 None（调用方提示输入密码）。
fn resolve_self_password(state: &AppState, password: &str) -> Option<String> {
    let p = password.trim();
    if !p.is_empty() && p != crate::config::model::PASSWORD_MASK {
        return Some(p.to_string());
    }
    let saved = state.config.load_full().self_password.clone();
    let saved = saved.trim();
    if !saved.is_empty() && saved != crate::config::model::PASSWORD_MASK {
        return Some(saved.to_string());
    }
    None
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
    let operator = operator.trim();
    let phone = phone.trim();
    let sms_password = sms_password.trim();

    if account.is_empty() {
        return Ok(CommandResult::err("请输入学号"));
    }
    if let Some(msg) = ensure_identity_gate(&state) {
        return Ok(CommandResult::err(&msg));
    }
    // 自助服务密码：前端未重输时回退已保存值（MASK/空串语义）
    let Some(password) = resolve_self_password(&state, &password) else {
        return Ok(CommandResult::err("请输入自助服务系统密码"));
    };
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

    let params = BindParams { account, password: &password, operator, phone, sms_password };
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
    if account.is_empty() {
        return Ok(CommandResult::err("请输入学号"));
    }
    let Some(password) = resolve_self_password(&state, &password) else {
        return Ok(CommandResult::err("请输入自助服务系统密码"));
    };

    let local_addr = resolve_campus_bind_addr(&state);
    match self_service::query_bind_status(account, &password, local_addr).await {
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
    if account.is_empty() {
        return Ok(CommandResult::err("请输入学号"));
    }
    let Some(password) = resolve_self_password(&state, &password) else {
        return Ok(CommandResult::err("请输入自助服务系统密码"));
    };

    let local_addr = resolve_campus_bind_addr(&state);
    match self_service::query_dashboard(account, &password, local_addr).await {
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

/// 查询自助服务"上网记录"账单页数据（/Self/bill/userOnlineLog，日期范围 + 汇总
/// + 明细行，原始 JSON 透传，展示格式化由前端完成）。密码空/MASK 回退已保存值。
#[tauri::command]
pub async fn query_self_online_log(
    state: State<'_, AppState>,
    account: String,
    password: String,
    start_time: String,
    end_time: String,
) -> Result<CommandResult, String> {
    let account = account.trim();
    if account.is_empty() {
        return Ok(CommandResult::err("请输入学号"));
    }
    let Some(password) = resolve_self_password(&state, &password) else {
        return Ok(CommandResult::err("请输入自助服务系统密码"));
    };
    let start_time = start_time.trim();
    let end_time = end_time.trim();
    if !is_iso_date(start_time) || !is_iso_date(end_time) {
        return Ok(CommandResult::err("日期格式应为 YYYY-MM-DD"));
    }
    // 零填充 ISO 日期字符串比较即日期先后
    if start_time > end_time {
        return Ok(CommandResult::err("开始日期不能晚于结束日期"));
    }

    let local_addr = resolve_campus_bind_addr(&state);
    match self_service::query_online_log(account, &password, start_time, end_time, local_addr).await {
        Ok(log) => Ok(CommandResult { success: true, message: None, data: Some(log) }),
        Err(e) => {
            crate::log_warn!("self", "查询自助服务上网记录失败: {e}");
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
    let session_id = session_id.trim();
    if account.is_empty() {
        return Ok(CommandResult::err("请输入学号"));
    }
    if let Some(msg) = ensure_identity_gate(&state) {
        return Ok(CommandResult::err(&msg));
    }
    let Some(password) = resolve_self_password(&state, &password) else {
        return Ok(CommandResult::err("请输入自助服务系统密码"));
    };
    if session_id.is_empty() {
        return Ok(CommandResult::err("缺少会话标识"));
    }

    let local_addr = resolve_campus_bind_addr(&state);
    match self_service::offline_session(account, &password, session_id, local_addr).await {
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

/// Windows 本地身份验证（仅 Windows Hello，设备未配置时返回引导文案）。
/// 弹窗文案由前端按场景传入（i18n）；主路径用官方 interop 接口把 Consent
/// 对话框绑定到主窗口 HWND（Win11 天然置前，见 identity.rs 模块注释）；
/// 通过后记录后端验证时间戳（时效 IDENTITY_VERIFY_TTL_SECS，reveal 等敏感
/// 操作在后端校验，防 webview 绕过）。
#[tauri::command]
pub async fn verify_windows_identity(
    app: tauri::AppHandle,
    consent_message: Option<String>,
) -> Result<CommandResult, String> {
    // 主窗口带到前台（为兜底路径提供前台进程上下文）+ 取 HWND 供 Consent UI 绑定。
    // HWND 取 isize 原始值：tauri 与本项目 windows crate 版本可能不同，跨类型不直通
    let owner_hwnd = app.get_webview_window("main").and_then(|win| {
        let _ = win.show();
        let _ = win.set_focus();
        win.hwnd().ok().map(|h| h.0 as isize)
    });
    let message = consent_message.unwrap_or_default();
    match crate::platform::identity::verify_identity(&message, owner_hwnd).await {
        Ok(()) => {
            crate::platform::identity::note_identity_verified();
            Ok(CommandResult::ok())
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
    let operator = operator.trim();
    if account.is_empty() {
        return Ok(CommandResult::err("请输入学号"));
    }
    let Some(password) = resolve_self_password(&state, &password) else {
        return Ok(CommandResult::err("请先填写自助服务系统密码"));
    };
    if !crate::platform::identity::identity_verified_recently() {
        return Ok(CommandResult::err(
            "Windows 身份验证已过期，请重新验证后再查看",
        ));
    }

    let local_addr = resolve_campus_bind_addr(&state);
    match self_service::reveal_credential(account, &password, operator, local_addr).await {
        Ok((phone, sms_password)) => Ok(CommandResult {
            success: true,
            message: None,
            data: Some(json!({ "phone": phone, "smsPassword": sms_password })),
        }),
        Err(e) => Ok(CommandResult::err(&e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_date_strict() {
        assert!(is_iso_date("2026-09-06"));
        // 旧宽松实现可放行的样本
        assert!(!is_iso_date("----------"));
        assert!(!is_iso_date("2026-99-06"));
        // 月份/日期越界、分隔符错误、位数不足
        assert!(!is_iso_date("2026-13-01"));
        assert!(!is_iso_date("2026-09-32"));
        assert!(!is_iso_date("2026-09-00"));
        assert!(!is_iso_date("2026/09/06"));
        assert!(!is_iso_date("2026-9-06"));
        assert!(!is_iso_date(""));
    }
}
