//! 自助服务系统相关命令（绑定运营商账号）

use std::net::IpAddr;

use tauri::State;

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

/// 绑定运营商账号（新手教程"绑定运营商账号"步骤）。
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
        return Ok(CommandResult::err("请输入运营商发送的短信密码"));
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
