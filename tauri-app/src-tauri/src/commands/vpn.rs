//! WebVPN 凭据命令与自动共存路由支撑（2026-10-25）：
//! 校园网内直连自助服务（现状）；校外且已保存 VPN 凭据且总开关开启时，
//! 自动经 WebVPN 网关包装访问。密码以 DPAPI 密文存 config（内存态亦为密文，
//! 使用时解密，日志只出打码账号）。

use serde_json::json;
use std::net::IpAddr;
use tauri::State;

use crate::config::model::Config;
use crate::infra::state::{AppState, CommandResult};
use crate::self_service::{SelfServiceChannel, SELF_BASE_URL};
use crate::vpn::cache::{ensure_alive, reset as reset_vpn_session, store as store_vpn_session};
use crate::vpn::cas::CasClient;
use crate::vpn::error::{map_cas_auth_error, map_cas_login_error, map_webvpn_error};
use crate::vpn::{route as route_decision, NetZone, RouteDecision, WebVpnSession};

/// VPN 凭据是否已保存（学号与加密密码均非空）。
fn has_vpn_credential(config: &Config) -> bool {
    !config.vpn_account.is_empty() && !config.vpn_password_enc.is_empty()
}

/// 解出已存 VPN 密码（config.vpn_password_enc 内存态为 DPAPI 密文）。
fn load_vpn_password(config: &Config) -> Result<String, String> {
    if config.vpn_password_enc.is_empty() {
        return Err("尚未保存 VPN 密码".to_string());
    }
    crate::account::crypto::decrypt(&config.vpn_password_enc).map_err(|e| format!("VPN 密码解密失败: {e}"))
}

/// 网络归属探测：校园网判定（SSID 名单 / 网关与 Portal ICMP）为同步阻塞实现，
/// 移入 spawn_blocking；探测失败（拿不到适配器信息等）按 Unknown 处理
/// （与 route 决策表一致：Unknown 走直连，不做二次尝试）。
async fn detect_net_zone(state: &AppState) -> NetZone {
    let config = state.config.load();
    let on_campus = tauri::async_runtime::spawn_blocking(move || -> Option<bool> {
        let adapters = crate::network::get_adapters_cached().ok()?;
        Some(crate::monitor::campus_check::check_campus_network(&config, &adapters).on_campus)
    })
    .await
    .ok()
    .flatten();
    match on_campus {
        Some(true) => NetZone::Campus,
        Some(false) => NetZone::OffCampus,
        None => NetZone::Unknown,
    }
}

/// 校外直连失败的附加提示（无凭据或总开关关闭时给出配置指引）。
fn offcampus_hint(zone: &NetZone, config: &Config) -> Option<String> {
    if !matches!(zone, NetZone::OffCampus) {
        return None;
    }
    if !config.self_service_via_vpn {
        Some("检测到校外网络：已关闭「校外经 WebVPN 访问」，可在设置中开启".to_string())
    } else if !has_vpn_credential(config) {
        Some("检测到校外网络：保存 WebVPN 凭据后可在校外访问自助服务".to_string())
    } else {
        None
    }
}

/// 失败文案附加校外提示（self_service 命令层共用）。
pub(crate) fn append_hint(mut message: String, hint: Option<String>) -> String {
    if let Some(h) = hint {
        if !message.contains(&h) {
            message.push_str("；");
            message.push_str(&h);
        }
    }
    message
}

/// 自助服务执行通道决策（自动共存路由核心，self_service 命令层共用）：
/// 决策统一走 [`crate::vpn::route::route`] 决策表（单一决策源），命令层只做
/// 「决策 → 通道」的落地映射：
/// - `Direct`（Campus/Unknown/公网目标，或无凭据/开关关时决策表给出的
///   `NeedLogin`——无凭据则无法建会话，直连让上层报真实错误并附提示）→ 直连；
/// - `Wrapped`（OffCampus + 内网目标 + 有凭据且开关开）→ 建/复用 WebVPN
///   会话走包装通道（会话失效由 [`ensure_vpn_session_alive`] 静默重登）。
/// 第二项返回是否走了 WebVPN（供日志），第三项为校外直连的配置指引。
pub(crate) async fn resolve_self_service_channel(
    state: &AppState,
    local_addr: Option<IpAddr>,
) -> Result<(SelfServiceChannel, bool, Option<String>), String> {
    let config = state.config.load();
    let zone = detect_net_zone(state).await;
    // 决策表的 has_vpn_session 在此语义为「具备建立 WebVPN 会话的条件」：
    // 总开关开且已存凭据（ensure_alive 会静默重登拿会话）。
    let can_establish = config.self_service_via_vpn && has_vpn_credential(&config);
    match route_decision(SELF_BASE_URL, zone, can_establish) {
        RouteDecision::Wrapped(_) => {
            let account = config.vpn_account.clone();
            let password = load_vpn_password(&config)?;
            drop(config);
            let session = ensure_alive(&account, &password).await?;
            crate::log_info!("vpn", "自助服务路由: OffCampus → WebVPN 包装（账号 {}）", crate::self_service::mask_account(&account));
            let channel = SelfServiceChannel::via_webvpn(&session)?;
            Ok((channel, true, None))
        }
        // NeedLogin 仅在「内网目标 + 无法建会话」时出现：直连让上层报真实错误，
        // hint 给出补凭据/开开关的指引
        RouteDecision::NeedLogin | RouteDecision::Direct | RouteDecision::Unreachable(_) => {
            let hint = offcampus_hint(&zone, &config);
            crate::log_info!("vpn", "自助服务路由: {:?} → 直连", zone);
            drop(config);
            let channel = SelfServiceChannel::direct(local_addr)?;
            Ok((channel, false, hint))
        }
    }
}

/// 配置落盘 + 内存更新 + 重置缓存会话（改 VPN 相关配置的统一收尾：
/// 先落盘再更新内存——落盘失败时命令报错且运行态不变，与 save_config 同则）。
async fn persist_config_and_reset_session(
    app_handle: &tauri::AppHandle,
    state: &State<'_, AppState>,
    new_config: Config,
) -> Result<(), String> {
    let handle = app_handle.clone();
    let config_for_disk = new_config.clone();
    tauri::async_runtime::spawn_blocking(move || crate::config::persist::save_config_and_broadcast(&handle, &config_for_disk))
        .await
        .map_err(|e| format!("配置落盘任务失败: {e}"))??;
    state.config.store(new_config);
    reset_vpn_session();
    Ok(())
}

/// 保存 WebVPN 凭据：密码 DPAPI 加密进 config 落盘，成功后自动开启
/// 「校外经 WebVPN 访问」总开关并重置缓存会话。
#[tauri::command]
pub async fn vpn_save_credential(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    account: String,
    password: String,
) -> Result<CommandResult, String> {
    if let Some(msg) = crate::commands::self_service::ensure_identity_gate(&state) {
        return Ok(CommandResult::err(&msg));
    }
    let account = account.trim().to_string();
    if account.is_empty() {
        return Ok(CommandResult::err("请输入学号"));
    }
    if password.is_empty() {
        return Ok(CommandResult::err("请输入统一身份认证密码"));
    }
    let enc = crate::account::crypto::encrypt(&password).map_err(|e| format!("密码加密失败: {e}"))?;
    let mut new_config = (*state.config.load()).clone();
    new_config.vpn_account = account.clone();
    new_config.vpn_password_enc = enc;
    new_config.self_service_via_vpn = true;
    persist_config_and_reset_session(&app_handle, &state, new_config).await?;
    crate::log_info!("vpn", "VPN 凭据已保存（账号 {}，密码已加密），总开关自动开启", crate::self_service::mask_account(&account));
    Ok(CommandResult::ok_msg("VPN 凭据已保存，校外访问将自动经 WebVPN 连接"))
}

/// 查询 VPN 凭据状态与「校外经 WebVPN 访问」总开关。
#[tauri::command]
pub fn vpn_credential_status(state: State<'_, AppState>) -> Result<CommandResult, String> {
    let config = state.config.load();
    let data = json!({
        "saved": has_vpn_credential(&config),
        "accountMasked": if config.vpn_account.is_empty() { String::new() } else { crate::self_service::mask_account(&config.vpn_account) },
        "viaVpn": config.self_service_via_vpn,
    });
    Ok(CommandResult { success: true, message: None, data: Some(data) })
}

/// 清除 WebVPN 凭据并把总开关置回关闭。
#[tauri::command]
pub async fn vpn_clear_credential(app_handle: tauri::AppHandle, state: State<'_, AppState>) -> Result<CommandResult, String> {
    let mut new_config = (*state.config.load()).clone();
    new_config.vpn_account.clear();
    new_config.vpn_password_enc.clear();
    new_config.self_service_via_vpn = false;
    persist_config_and_reset_session(&app_handle, &state, new_config).await?;
    crate::log_info!("vpn", "VPN 凭据已清除，总开关关闭");
    Ok(CommandResult::ok_msg("已清除 VPN 凭据"))
}

/// 「校外经 WebVPN 访问」总开关（仅控制校外自动回退，校内始终直连）。
#[tauri::command]
pub async fn set_self_service_via_vpn(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<CommandResult, String> {
    let mut new_config = (*state.config.load()).clone();
    if new_config.self_service_via_vpn == enabled {
        return Ok(CommandResult::ok_msg(if enabled { "已处于开启状态" } else { "已处于关闭状态" }));
    }
    new_config.self_service_via_vpn = enabled;
    persist_config_and_reset_session(&app_handle, &state, new_config).await?;
    crate::log_info!("vpn", "校外经 WebVPN 访问总开关: {}", if enabled { "开" } else { "关" });
    Ok(CommandResult::ok_msg(if enabled { "已开启校外经 WebVPN 访问" } else { "已关闭校外经 WebVPN 访问" }))
}

/// 连接测试：保存的凭据走完整链路（CAS 登录 → WebVPN 网关登录 → 探活），
/// message 带分步中文结果；成功后会话入缓存供后续业务直接复用。
#[tauri::command]
pub async fn vpn_test_connection(state: State<'_, AppState>) -> Result<CommandResult, String> {
    let config = state.config.load();
    if !has_vpn_credential(&config) {
        return Ok(CommandResult::err("尚未保存 VPN 凭据，请先填写学号与密码"));
    }
    let account = config.vpn_account.clone();
    let password = match load_vpn_password(&config) {
        Ok(p) => p,
        Err(e) => return Ok(CommandResult::err(&e)),
    };
    drop(config);
    match run_full_chain(&account, &password).await {
        Ok(steps) => Ok(CommandResult::ok_msg(&steps)),
        Err(e) => Ok(CommandResult::err(&e)),
    }
}

/// 完整链路执行：①CAS 统一身份认证 → ②WebVPN 网关静默登录 → ③网关会话探活。
/// 任一步失败即返回带步骤前缀的错误。
async fn run_full_chain(account: &str, password: &str) -> Result<String, String> {
    let cas = CasClient::new().map_err(|e| format!("① CAS 客户端创建失败: {}", map_cas_auth_error(e)))?;
    let pwd_hex = crate::vpn::rsa::rsa_encrypt_hex(password).map_err(|e| format!("① 密码加密失败: {e}"))?;
    let ok = cas
        .login(account, &pwd_hex)
        .await
        .map_err(|e| format!("① CAS 统一身份认证失败: {}", map_cas_login_error(e)))?;
    let session = WebVpnSession::login(&cas, &ok.tgt)
        .await
        .map_err(|e| format!("② WebVPN 网关登录失败: {}", map_webvpn_error(e)))?;
    if !session.is_alive().await {
        return Err("③ 网关会话探活失败：网关返回登录页，请稍后重试".to_string());
    }
    store_vpn_session(session);
    Ok("① CAS 统一身份认证成功；② WebVPN 网关登录成功；③ 网关会话有效".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_hint_adds_once() {
        let base = "自助服务请求失败: 超时".to_string();
        let hint = Some("检测到校外网络：保存 WebVPN 凭据后可在校外访问自助服务".to_string());
        let once = append_hint(base.clone(), hint.clone());
        assert!(once.contains("超时"), "{once}");
        assert!(once.contains("WebVPN"), "{once}");
        let twice = append_hint(once, hint);
        // 第二次追加被 contains 挡住：提示不重复出现
        assert_eq!(twice.matches("WebVPN").count(), 1, "{twice}");
    }

    #[test]
    fn append_hint_without_hint_keeps_message() {
        let base = "直连失败".to_string();
        assert_eq!(append_hint(base.clone(), None), base);
    }

    #[test]
    fn offcampus_hint_semantics() {
        let mut config = Config::default();
        // 校外 + 总开关关
        config.self_service_via_vpn = false;
        let h = offcampus_hint(&NetZone::OffCampus, &config).unwrap();
        assert!(h.contains("已关闭"), "{h}");
        // 校外 + 开 + 无凭据
        config.self_service_via_vpn = true;
        let h = offcampus_hint(&NetZone::OffCampus, &config).unwrap();
        assert!(h.contains("保存 WebVPN 凭据"), "{h}");
        // 校内永远无提示
        assert!(offcampus_hint(&NetZone::Campus, &config).is_none());
        // 校外 + 开 + 有凭据（由 want_vpn 拦截，正常不会到 hint）
        config.vpn_account = "20250001".to_string();
        config.vpn_password_enc = "enc".to_string();
        assert!(offcampus_hint(&NetZone::OffCampus, &config).is_none());
    }
}
