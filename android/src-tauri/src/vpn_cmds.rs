//! WebVPN 凭据命令与自助服务自动共存路由支撑（2026-10-25,桌面 commands/vpn.rs 同构）:
//! 校园网内直连自助服务(现状);校外且已保存 VPN 凭据且总开关开启时,
//! 自动经 WebVPN 网关包装访问。
//!
//! 凭据加密的平台差异:vpn_password_enc 的语义是「平台加密器密文」——桌面为
//! Windows DPAPI 密文,安卓为 AndroidKeyStore(tauri-plugin-campus-keystore)
//! 密文,两侧密文互不可解、字段语义一致;内存态与落盘态均为密文,使用时解密,
//! 出站掩码打码(见 config_state::masked_for_display),日志只出打码账号。

use serde_json::json;
use std::net::IpAddr;
use tauri::Manager;

use campus_login_lib::self_service::SelfServiceChannel;
use campus_login_lib::vpn::cache::{
    ensure_alive, reset as reset_vpn_session, store as store_vpn_session,
};
use campus_login_lib::vpn::cas::CasClient;
use campus_login_lib::vpn::error::{map_cas_auth_error, map_cas_login_error, map_webvpn_error};
use campus_login_lib::vpn::{NetZone, WebVpnSession};

use crate::config_state::{current_settings, CryptoBridge, Settings};
use crate::self_service_cmds::CommandResult;

/// VPN 凭据是否已保存(学号与加密密码均非空;桌面 has_vpn_credential 同构)。
fn has_vpn_credential(settings: &Settings) -> bool {
    !settings.vpn_account.is_empty() && !settings.vpn_password_enc.is_empty()
}

/// 解出已存 VPN 密码(vpn_password_enc 内存态为 Keystore 密文;桌面 load_vpn_password
/// 同构,仅加密器不同)。
async fn load_vpn_password(app: &tauri::AppHandle, settings: &Settings) -> Result<String, String> {
    if settings.vpn_password_enc.is_empty() {
        return Err("尚未保存 VPN 密码".to_string());
    }
    let bridge = CryptoBridge::from_app(app);
    (bridge.decrypt)(&settings.vpn_password_enc).map_err(|e| format!("VPN 密码解密失败: {e}"))
}

/// 网络归属探测:安卓侧复用 campus_detect 探针(网卡枚举 + SSID 名单 + 网关/Portal
/// TCP 探测),失败(拿不到适配器信息等)按 Unknown 处理——与桌面 detect_net_zone
/// 及 route 决策表一致:Unknown 走直连,不做二次尝试。
async fn detect_net_zone(app: &tauri::AppHandle, settings: &Settings) -> NetZone {
    let (probe, _ssid) = crate::campus_detect::probe_campus_with_ssid(app, settings).await;
    match probe {
        Ok(p) if p.on_campus => NetZone::Campus,
        Ok(_) => NetZone::OffCampus,
        Err(_) => NetZone::Unknown,
    }
}

/// 校外直连失败的附加提示(无凭据或总开关关闭时给出配置指引;桌面同构)。
fn offcampus_hint(zone: &NetZone, settings: &Settings) -> Option<String> {
    if !matches!(zone, NetZone::OffCampus) {
        return None;
    }
    if !settings.self_service_via_vpn {
        Some("检测到校外网络：已关闭「校外经 WebVPN 访问」，可在设置中开启".to_string())
    } else if !has_vpn_credential(settings) {
        Some("检测到校外网络：保存 WebVPN 凭据后可在校外访问自助服务".to_string())
    } else {
        None
    }
}

/// 失败文案附加校外提示(self_service 命令层共用;桌面同构)。
pub(crate) fn append_hint(mut message: String, hint: Option<String>) -> String {
    if let Some(h) = hint {
        if !message.contains(&h) {
            message.push_str("；");
            message.push_str(&h);
        }
    }
    message
}

/// 自助服务执行通道决策(自动共存路由核心,self_service 命令层共用;桌面
/// resolve_self_service_channel 同构):
/// - Campus / Unknown → 直连(现状行为,Unknown 不拦截避免校内误拦);
/// - OffCampus + 总开关开 + 已存凭据 → 建/复用 WebVPN 会话走包装通道
///   (会话失效由 ensure_alive 静默重登);
/// - OffCampus + 开关关或无凭据 → 直连(校外直连内网地址必然失败,
///   调用方用返回的第三项在错误文案上附加配置指引)。
/// 第二项返回是否走了 WebVPN(供日志)。
pub(crate) async fn resolve_self_service_channel(
    app: &tauri::AppHandle,
    local_addr: Option<IpAddr>,
) -> Result<(SelfServiceChannel, bool, Option<String>), String> {
    let settings = current_settings(app).await?;
    let zone = detect_net_zone(app, &settings).await;
    let want_vpn = matches!(zone, NetZone::OffCampus)
        && settings.self_service_via_vpn
        && has_vpn_credential(&settings);
    if !want_vpn {
        let hint = offcampus_hint(&zone, &settings);
        campus_login_lib::log_info!("vpn", "自助服务路由: {:?} → 直连", zone);
        let channel = SelfServiceChannel::direct(local_addr)?;
        return Ok((channel, false, hint));
    }
    let account = settings.vpn_account.clone();
    let password = load_vpn_password(app, &settings).await?;
    let session = ensure_alive(&account, &password).await?;
    campus_login_lib::log_info!(
        "vpn",
        "自助服务路由: OffCampus → WebVPN 包装（账号 {}）",
        campus_login_lib::self_service::mask_account(&account)
    );
    let channel = SelfServiceChannel::via_webvpn(&session)?;
    Ok((channel, true, None))
}

/// 保存 WebVPN 凭据:密码经 Keystore 加密进 config 落盘,成功后自动开启
/// 「校外经 WebVPN 访问」总开关并重置缓存会话(桌面同构;改状态命令,设验证门)。
#[tauri::command]
pub async fn vpn_save_credential(
    app: tauri::AppHandle,
    account: String,
    password: String,
) -> Result<CommandResult, String> {
    let settings = current_settings(&app).await?;
    if let Some(msg) = crate::self_service_cmds::ensure_identity_gate(&settings) {
        return Ok(CommandResult::err(&msg));
    }
    let account = account.trim().to_string();
    if account.is_empty() {
        return Ok(CommandResult::err("请输入学号"));
    }
    if password.is_empty() {
        return Ok(CommandResult::err("请输入统一身份认证密码"));
    }
    let bridge = CryptoBridge::from_app(&app);
    let enc = (bridge.encrypt)(&password).map_err(|e| format!("密码加密失败: {e}"))?;
    // 先落盘再改内存态(桌面同则):落盘失败命令报错且运行态不变;全程持配置
    // IO 锁覆盖 load→merge→save,防与 save_config/账号命令并发丢更新
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取数据目录失败: {e}"))?;
    let _io_guard = crate::config_state::config_io_lock().await;
    let mut merged = crate::config_state::load_from(&dir, &bridge).await?;
    merged.vpn_account = account.clone();
    merged.vpn_password_enc = enc;
    merged.self_service_via_vpn = true;
    crate::config_state::save_to(&dir, &bridge, &merged).await?;
    let state = app.state::<crate::android_state::AndroidState>();
    if let Ok(mut cache) = state.config.lock() {
        *cache = Some(merged.clone());
    }
    reset_vpn_session();
    crate::config_state::emit_config_changed(&app, &merged).await;
    campus_login_lib::log_info!(
        "vpn",
        "VPN 凭据已保存（账号 {}，密码已加密），总开关自动开启",
        campus_login_lib::self_service::mask_account(&account)
    );
    Ok(CommandResult::ok_msg("VPN 凭据已保存，校外访问将自动经 WebVPN 连接"))
}

/// 查询 VPN 凭据状态与「校外经 WebVPN 访问」总开关(查询类,不设验证门)。
#[tauri::command]
pub async fn vpn_credential_status(app: tauri::AppHandle) -> Result<CommandResult, String> {
    let settings = current_settings(&app).await?;
    let data = json!({
        "saved": has_vpn_credential(&settings),
        "accountMasked": if settings.vpn_account.is_empty() {
            String::new()
        } else {
            campus_login_lib::self_service::mask_account(&settings.vpn_account)
        },
        "viaVpn": settings.self_service_via_vpn,
    });
    Ok(CommandResult { success: true, message: None, data: Some(data) })
}

/// 清除 WebVPN 凭据并把总开关置回关闭(改状态命令,设验证门;桌面无门是桌面差异,
/// 安卓侧按 self_service_cmds 纪律改状态一律设门)。
#[tauri::command]
pub async fn vpn_clear_credential(app: tauri::AppHandle) -> Result<CommandResult, String> {
    let settings = current_settings(&app).await?;
    if let Some(msg) = crate::self_service_cmds::ensure_identity_gate(&settings) {
        return Ok(CommandResult::err(&msg));
    }
    let bridge = CryptoBridge::from_app(&app);
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取数据目录失败: {e}"))?;
    let _io_guard = crate::config_state::config_io_lock().await;
    let mut merged = crate::config_state::load_from(&dir, &bridge).await?;
    merged.vpn_account.clear();
    merged.vpn_password_enc.clear();
    merged.self_service_via_vpn = false;
    crate::config_state::save_to(&dir, &bridge, &merged).await?;
    let state = app.state::<crate::android_state::AndroidState>();
    if let Ok(mut cache) = state.config.lock() {
        *cache = Some(merged.clone());
    }
    reset_vpn_session();
    crate::config_state::emit_config_changed(&app, &merged).await;
    campus_login_lib::log_info!("vpn", "VPN 凭据已清除，总开关关闭");
    Ok(CommandResult::ok_msg("已清除 VPN 凭据"))
}

/// 「校外经 WebVPN 访问」总开关(仅控制校外自动回退,校内始终直连;改状态命令,
/// 设验证门;桌面无门是桌面差异,安卓侧按 self_service_cmds 纪律设门)。
#[tauri::command]
pub async fn set_self_service_via_vpn(
    app: tauri::AppHandle,
    enabled: bool,
) -> Result<CommandResult, String> {
    let settings = current_settings(&app).await?;
    if let Some(msg) = crate::self_service_cmds::ensure_identity_gate(&settings) {
        return Ok(CommandResult::err(&msg));
    }
    if settings.self_service_via_vpn == enabled {
        return Ok(CommandResult::ok_msg(if enabled {
            "已处于开启状态"
        } else {
            "已处于关闭状态"
        }));
    }
    let bridge = CryptoBridge::from_app(&app);
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取数据目录失败: {e}"))?;
    let _io_guard = crate::config_state::config_io_lock().await;
    let mut merged = crate::config_state::load_from(&dir, &bridge).await?;
    merged.self_service_via_vpn = enabled;
    crate::config_state::save_to(&dir, &bridge, &merged).await?;
    let state = app.state::<crate::android_state::AndroidState>();
    if let Ok(mut cache) = state.config.lock() {
        *cache = Some(merged.clone());
    }
    reset_vpn_session();
    crate::config_state::emit_config_changed(&app, &merged).await;
    campus_login_lib::log_info!(
        "vpn",
        "校外经 WebVPN 访问总开关: {}",
        if enabled { "开" } else { "关" }
    );
    Ok(CommandResult::ok_msg(if enabled {
        "已开启校外经 WebVPN 访问"
    } else {
        "已关闭校外经 WebVPN 访问"
    }))
}

/// 连接测试:保存的凭据走完整链路(CAS 登录 → WebVPN 网关登录 → 探活),
/// message 带分步中文结果;成功后会话入缓存供后续业务直接复用(查询类,不设门)。
#[tauri::command]
pub async fn vpn_test_connection(app: tauri::AppHandle) -> Result<CommandResult, String> {
    let settings = current_settings(&app).await?;
    if !has_vpn_credential(&settings) {
        return Ok(CommandResult::err("尚未保存 VPN 凭据，请先填写学号与密码"));
    }
    let account = settings.vpn_account.clone();
    let password = match load_vpn_password(&app, &settings).await {
        Ok(p) => p,
        Err(e) => return Ok(CommandResult::err(&e)),
    };
    match run_full_chain(&account, &password).await {
        Ok(steps) => Ok(CommandResult::ok_msg(&steps)),
        Err(e) => Ok(CommandResult::err(&e)),
    }
}

/// 完整链路执行:①CAS 统一身份认证 → ②WebVPN 网关静默登录 → ③网关会话探活。
/// 任一步失败即返回带步骤前缀的错误(桌面同构)。
async fn run_full_chain(account: &str, password: &str) -> Result<String, String> {
    let cas =
        CasClient::new().map_err(|e| format!("① CAS 客户端创建失败: {}", map_cas_auth_error(e)))?;
    let pwd_hex = campus_login_lib::vpn::rsa::rsa_encrypt_hex(password)
        .map_err(|e| format!("① 密码加密失败: {e}"))?;
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
        // 第二次追加被 contains 挡住:提示不重复出现
        assert_eq!(twice.matches("WebVPN").count(), 1, "{twice}");
    }

    #[test]
    fn append_hint_without_hint_keeps_message() {
        let base = "直连失败".to_string();
        assert_eq!(append_hint(base.clone(), None), base);
    }

    #[test]
    fn offcampus_hint_semantics() {
        let mut settings = Settings::default();
        // 校外 + 总开关关
        settings.self_service_via_vpn = false;
        let h = offcampus_hint(&NetZone::OffCampus, &settings).unwrap();
        assert!(h.contains("已关闭"), "{h}");
        // 校外 + 开 + 无凭据
        settings.self_service_via_vpn = true;
        let h = offcampus_hint(&NetZone::OffCampus, &settings).unwrap();
        assert!(h.contains("保存 WebVPN 凭据"), "{h}");
        // 校内永远无提示
        assert!(offcampus_hint(&NetZone::Campus, &settings).is_none());
        // 校外 + 开 + 有凭据(由 want_vpn 拦截,正常不会到 hint)
        settings.vpn_account = "20250001".to_string();
        settings.vpn_password_enc = "enc".to_string();
        assert!(offcampus_hint(&NetZone::OffCampus, &settings).is_none());
    }
}
