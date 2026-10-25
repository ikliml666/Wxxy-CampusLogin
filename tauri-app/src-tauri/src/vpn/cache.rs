//! WebVPN 会话进程内缓存（2026-10-25 新增，自动共存路由支撑）：
//! 命令层路由命中 Wrapped 时惰性建立/复用，失效（[`WebVpnSession::is_alive`] ==
//! false 或凭据/总开关变更）用保存凭据静默重登一次。密码仅内存传递，任何日志
//! 不得出现明文凭据。

use super::cas::{CasClient, CasLoginError};
use super::error::CasAuthError;
use super::rsa::rsa_encrypt_hex;
use super::session::WebVpnSession;
use super::WebVpnError;
use lazy_static::lazy_static;
use parking_lot::RwLock;
use std::sync::Arc;

lazy_static! {
    static ref VPN_SESSION: RwLock<Option<Arc<WebVpnSession>>> = RwLock::new(None);
}

/// 取当前缓存会话克隆（不校验活性：活性判断是 async 调用，不得持锁跨 await）。
pub fn cached() -> Option<Arc<WebVpnSession>> {
    VPN_SESSION.read().clone()
}

/// 重置缓存（保存/清除凭据、切换总开关、会话失效时调用）。
pub fn reset() {
    *VPN_SESSION.write() = None;
}

/// 取有效会话：缓存命中且探活通过直接复用；否则静默重登一次（CAS 登录 →
/// TGT 换 ST → 网关 302 链，无验证码无交互）。`account`/`password` 为 CAS
/// 明文凭据，由调用方从加密存储解出，仅内存传递。
pub async fn ensure_alive(account: &str, password: &str) -> Result<Arc<WebVpnSession>, String> {
    if let Some(session) = cached() {
        if session.is_alive().await {
            return Ok(session);
        }
        reset();
    }
    login_and_cache(account, password).await
}

/// 建立全新会话并存入缓存。
pub async fn login_and_cache(account: &str, password: &str) -> Result<Arc<WebVpnSession>, String> {
    let session = login_new_session(account, password).await?;
    let arc = Arc::new(session);
    *VPN_SESSION.write() = Some(arc.clone());
    Ok(arc)
}

/// 把外部建立的会话存入缓存（连接测试走完整链路成功后复用其登录态，
/// 免去紧接着的业务请求再登一次）。
pub fn store(session: WebVpnSession) {
    *VPN_SESSION.write() = Some(Arc::new(session));
}

/// 一次性建立全新 WebVPN 会话（不入缓存）：CAS 账密登录拿 TGT → WebVPN 静默
/// 登录。连接测试的分步展示也走本函数。
pub async fn login_new_session(account: &str, password: &str) -> Result<WebVpnSession, String> {
    let cas = CasClient::new().map_err(|e| format!("CAS 客户端创建失败: {e}"))?;
    let pwd_hex = rsa_encrypt_hex(password).map_err(|e| format!("密码加密失败: {e}"))?;
    let ok = cas.login(account, &pwd_hex).await.map_err(map_cas_login_error)?;
    WebVpnSession::login(&cas, &ok.tgt).await.map_err(map_webvpn_error)
}

/// CAS 登录错误 → 用户可读中文（不含任何凭据内容）。
pub fn map_cas_login_error(e: CasLoginError) -> String {
    match e {
        CasLoginError::WrongUserOrPwd => "CAS 登录失败：账号或密码错误（请检查保存的 VPN 凭据）".to_string(),
        CasLoginError::WrongCaptcha => "CAS 登录失败：服务端要求验证码（异常风控，请稍后重试）".to_string(),
        CasLoginError::UserLocked => "CAS 登录失败：账号已被锁定".to_string(),
        CasLoginError::NeedTwoVerify(raw) => format!("CAS 登录失败：需要二次验证（{raw}）"),
        CasLoginError::Unknown(raw) => format!("CAS 登录失败：{raw}"),
        CasLoginError::Network(raw) => format!("CAS 登录网络错误：{raw}"),
    }
}

/// WebVPN 协议错误 → 用户可读中文（不含任何凭据内容）。
pub fn map_webvpn_error(e: WebVpnError) -> String {
    match e {
        WebVpnError::Network(raw) => format!("WebVPN 网关连接失败：{raw}"),
        WebVpnError::Sso(raw) => format!("WebVPN 单点登录失败：{raw}"),
        other => format!("WebVPN 会话建立失败：{other}"),
    }
}

/// [`CasAuthError`] → 用户可读中文。
pub fn map_cas_auth_error(e: CasAuthError) -> String {
    format!("CAS 请求失败：{e}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cas_error_messages_are_chinese_without_credentials() {
        let s = map_cas_login_error(CasLoginError::WrongUserOrPwd);
        assert!(s.contains("账号或密码错误"), "{s}");
        assert!(s.contains("VPN 凭据"), "{s}");
        let s = map_cas_login_error(CasLoginError::Unknown("PASSERROR".to_string()));
        assert!(s.contains("PASSERROR"), "{s}");
        let s = map_webvpn_error(WebVpnError::InvalidIv);
        assert!(s.contains("WebVPN"), "{s}");
        let s = map_cas_auth_error(CasAuthError::Rsa("非 ASCII".to_string()));
        assert!(s.contains("RSA"), "{s}");
    }

    #[tokio::test]
    async fn cache_reset_and_relogin_semantics() {
        // 重置后无缓存；登录成功后缓存可用且 clone 共享（is_alive 属网络调用，
        // 这里只验证缓存语义，不做真实登录）
        reset();
        assert!(cached().is_none());
    }
}
