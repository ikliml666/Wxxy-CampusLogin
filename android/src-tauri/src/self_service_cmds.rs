//! 自助服务命令面:六个命令包装 campus_login_lib::self_service 协议(纯 HTTP,跨平台),
//! 命令名/参数/返回形状与桌面 commands/self_service.rs 同构。
//! 生物识别验证门:改状态命令(bind/offline/reveal)设门,查询类不设门;reveal 不受总开关影响。

use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct CommandResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl CommandResult {
    pub fn ok_msg(msg: &str) -> Self {
        Self { success: true, message: Some(msg.to_string()), data: None }
    }
    pub fn err(msg: &str) -> Self {
        Self { success: false, message: Some(msg.to_string()), data: None }
    }
}

/// 自助密码回退:前端传 MASK/空时用已存值;两者皆无返回 None(提示输入)
fn resolve_self_password(incoming: &str, settings: &crate::config_state::Settings) -> Option<String> {
    if !incoming.is_empty() && incoming != crate::config_state::PASSWORD_MASK {
        return Some(incoming.to_string());
    }
    if settings.self_password.is_empty() {
        None
    } else {
        Some(settings.self_password.clone())
    }
}

/// 验证门:总开关关闭 → 放行;未在 TTL 内验证 → 拦截文案(桌面 ensure_identity_gate 同构)
fn ensure_identity_gate(settings: &crate::config_state::Settings) -> Option<String> {
    if !settings.self_hello_enabled {
        return None;
    }
    if crate::identity_gate::identity_verified_recently() {
        None
    } else {
        Some("生物识别验证已过期,请重新验证".to_string())
    }
}

async fn load_settings(app: &tauri::AppHandle) -> Result<crate::config_state::Settings, String> {
    crate::config_state::current_settings(app).await
}

fn local_addr_of(state: &tauri::State<'_, crate::android_state::AndroidState>) -> Option<std::net::IpAddr> {
    state
        .cached_source_ip
        .lock()
        .ok()
        .and_then(|cached| cached.map(std::net::IpAddr::from))
}

/// 前端 BiometricPrompt 认证成功后调用;后端记 TTL 时间戳
#[tauri::command]
pub async fn verify_biometric_identity(_consent_message: Option<String>) -> Result<CommandResult, String> {
    crate::identity_gate::note_identity_verified();
    Ok(CommandResult::ok_msg("验证通过"))
}

#[tauri::command]
pub async fn bind_operator(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::android_state::AndroidState>,
    account: String,
    password: String,
    operator: String,
    phone: String,
    sms_password: String,
) -> Result<CommandResult, String> {
    let settings = load_settings(&app).await?;
    let account = account.trim().to_string();
    let operator = operator.trim().to_string();
    let phone = phone.trim().to_string();
    let sms_password = sms_password.trim().to_string();

    if account.is_empty() {
        return Ok(CommandResult::err("请输入学号"));
    }
    if let Some(msg) = ensure_identity_gate(&settings) {
        return Ok(CommandResult::err(&msg));
    }
    let Some(password) = resolve_self_password(&password, &settings) else {
        return Ok(CommandResult::err("请输入自助服务系统密码"));
    };
    if phone.len() != 11 || !phone.chars().all(|c| c.is_ascii_digit()) {
        return Ok(CommandResult::err("请输入 11 位手机号"));
    }
    if sms_password.is_empty() {
        return Ok(CommandResult::err("请输入运营商账户密码"));
    }

    let params = campus_login_lib::self_service::BindParams {
        account: &account,
        password: &password,
        operator: &operator,
        phone: &phone,
        sms_password: &sms_password,
    };
    match campus_login_lib::self_service::bind_operator(&params, local_addr_of(&state)).await {
        Ok(msg) => Ok(CommandResult::ok_msg(&msg)),
        Err(e) => Ok(CommandResult::err(&e)),
    }
}

#[tauri::command]
pub async fn query_bind_status(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::android_state::AndroidState>,
    account: String,
    password: String,
) -> Result<CommandResult, String> {
    let settings = load_settings(&app).await?;
    let account = account.trim().to_string();
    if account.is_empty() {
        return Ok(CommandResult::err("请输入学号"));
    }
    let Some(password) = resolve_self_password(&password, &settings) else {
        return Ok(CommandResult::err("请输入自助服务系统密码"));
    };
    match campus_login_lib::self_service::query_bind_status(&account, &password, local_addr_of(&state)).await {
        Ok(bindings) => {
            let to_json = |b: &Option<campus_login_lib::self_service::OperatorBinding>| match b {
                Some(b) => serde_json::json!({ "account": b.masked_account, "passwordSet": b.password_set }),
                None => serde_json::Value::Null,
            };
            let data = serde_json::json!({
                "cmcc": to_json(&bindings[0]),
                "telecom": to_json(&bindings[1]),
                "unicom": to_json(&bindings[2]),
            });
            Ok(CommandResult { success: true, message: None, data: Some(data) })
        }
        Err(e) => Ok(CommandResult::err(&e)),
    }
}

#[tauri::command]
pub async fn query_self_dashboard(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::android_state::AndroidState>,
    account: String,
    password: String,
) -> Result<CommandResult, String> {
    let settings = load_settings(&app).await?;
    let account = account.trim().to_string();
    if account.is_empty() {
        return Ok(CommandResult::err("请输入学号"));
    }
    let Some(password) = resolve_self_password(&password, &settings) else {
        return Ok(CommandResult::err("请输入自助服务系统密码"));
    };
    match campus_login_lib::self_service::query_dashboard(&account, &password, local_addr_of(&state)).await {
        Ok((online, history)) => Ok(CommandResult {
            success: true,
            message: None,
            data: Some(serde_json::json!({ "onlineList": online, "loginHistory": history })),
        }),
        Err(e) => Ok(CommandResult::err(&e)),
    }
}

/// YYYY-MM-DD 严格校验(桌面 is_iso_date 同构)
fn is_iso_date(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes.iter().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
}

#[tauri::command]
pub async fn query_self_online_log(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::android_state::AndroidState>,
    account: String,
    password: String,
    start_time: String,
    end_time: String,
) -> Result<CommandResult, String> {
    let settings = load_settings(&app).await?;
    let account = account.trim().to_string();
    if account.is_empty() {
        return Ok(CommandResult::err("请输入学号"));
    }
    let Some(password) = resolve_self_password(&password, &settings) else {
        return Ok(CommandResult::err("请输入自助服务系统密码"));
    };
    if !is_iso_date(&start_time) || !is_iso_date(&end_time) {
        return Ok(CommandResult::err("日期格式应为 YYYY-MM-DD"));
    }
    if start_time > end_time {
        return Ok(CommandResult::err("开始日期不能晚于结束日期"));
    }
    match campus_login_lib::self_service::query_online_log(&account, &password, &start_time, &end_time, local_addr_of(&state)).await {
        Ok(log) => Ok(CommandResult { success: true, message: None, data: Some(log) }),
        Err(e) => Ok(CommandResult::err(&e)),
    }
}

#[tauri::command]
pub async fn self_offline_session(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::android_state::AndroidState>,
    account: String,
    password: String,
    session_id: String,
) -> Result<CommandResult, String> {
    let settings = load_settings(&app).await?;
    let account = account.trim().to_string();
    if account.is_empty() {
        return Ok(CommandResult::err("请输入学号"));
    }
    if let Some(msg) = ensure_identity_gate(&settings) {
        return Ok(CommandResult::err(&msg));
    }
    let Some(password) = resolve_self_password(&password, &settings) else {
        return Ok(CommandResult::err("请输入自助服务系统密码"));
    };
    if session_id.trim().is_empty() {
        return Ok(CommandResult::err("缺少会话标识"));
    }
    match campus_login_lib::self_service::offline_session(&account, &password, &session_id, local_addr_of(&state)).await {
        Ok(()) => Ok(CommandResult::ok_msg("注销成功")),
        Err(e) => Ok(CommandResult::err(&e)),
    }
}

/// 明文揭示手机号/短信密码:无论总开关如何都强制验证门(桌面同构,防 webview 侧绕过)
#[tauri::command]
pub async fn reveal_operator_credential(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::android_state::AndroidState>,
    account: String,
    password: String,
    operator: String,
) -> Result<CommandResult, String> {
    let settings = load_settings(&app).await?;
    let account = account.trim().to_string();
    let operator = operator.trim().to_string();
    if account.is_empty() {
        return Ok(CommandResult::err("请输入学号"));
    }
    if !crate::identity_gate::identity_verified_recently() {
        return Ok(CommandResult::err("生物识别验证已过期,请重新验证"));
    }
    let Some(password) = resolve_self_password(&password, &settings) else {
        return Ok(CommandResult::err("请输入自助服务系统密码"));
    };
    match campus_login_lib::self_service::reveal_credential(&account, &password, &operator, local_addr_of(&state)).await {
        Ok((phone, sms_password)) => Ok(CommandResult {
            success: true,
            message: None,
            data: Some(serde_json::json!({ "phone": phone, "smsPassword": sms_password })),
        }),
        Err(e) => Ok(CommandResult::err(&e)),
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    fn settings_with_self_password(pw: &str) -> crate::config_state::Settings {
        crate::config_state::Settings {
            self_password: pw.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn 自助密码回退_掩码与空回退_新值直用() {
        let s = settings_with_self_password("stored");
        assert_eq!(resolve_self_password("***", &s), Some("stored".into()));
        assert_eq!(resolve_self_password("", &s), Some("stored".into()));
        assert_eq!(resolve_self_password("fresh", &s), Some("fresh".into()));
        // 已存为空 → None(提示输入)
        assert_eq!(resolve_self_password("***", &settings_with_self_password("")), None);
    }

    #[test]
    fn 验证门_总开关关闭放行_开启走TTL() {
        crate::identity_gate::reset_for_tests();
        // 开关关 → 放行
        let mut s = settings_with_self_password("");
        s.self_hello_enabled = false;
        assert_eq!(ensure_identity_gate(&s), None);
        // 开关开 + 未验证 → 拦截
        s.self_hello_enabled = true;
        assert!(ensure_identity_gate(&s).is_some());
        // 验证后 → 放行
        crate::identity_gate::note_identity_verified();
        assert_eq!(ensure_identity_gate(&s), None);
    }

    #[test]
    fn is_iso_date_严格校验() {
        assert!(is_iso_date("2026-09-07"));
        assert!(!is_iso_date("2026-9-07"));
        assert!(!is_iso_date("2026/09/07"));
        assert!(!is_iso_date("2026-09-0a"));
        assert!(!is_iso_date("20260907"));
    }
}
