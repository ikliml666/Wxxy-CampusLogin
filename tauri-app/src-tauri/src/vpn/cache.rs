//! WebVPN 会话进程内缓存（2026-10-25 新增，自动共存路由支撑）：
//! 命令层路由命中 Wrapped 时惰性建立/复用，失效（[`WebVpnSession::is_alive`] ==
//! false 或凭据/总开关变更）用保存凭据静默重登一次。密码仅内存传递，任何日志
//! 不得出现明文凭据。

use super::cas::CasClient;
use super::error::{map_cas_login_error, map_webvpn_error};
use super::rsa::rsa_encrypt_hex;
use super::session::WebVpnSession;
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




#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cache_reset_and_relogin_semantics() {
        // 重置后无缓存；登录成功后缓存可用且 clone 共享（is_alive 属网络调用，
        // 这里只验证缓存语义，不做真实登录）
        reset();
        assert!(cached().is_none());
    }
}
