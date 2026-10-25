//! CAS 协议层错误类型（移植自 Wxxy-CampusHub crates/campus-auth/src/error.rs，
//! 2026-10-25：本仓库仅消费 CAS 登录与换票，教务/门户会话专属变体裁剪）。

/// CAS 协议核心错误。
#[derive(Debug, thiserror::Error)]
pub enum CasAuthError {
    #[error("RSA 加密失败: {0}")]
    Rsa(String),
    #[error("HTTP 请求失败: {0}")]
    Http(#[from] reqwest::Error),
    #[error("响应解析失败: {0}")]
    Parse(String),
}

/// CAS 登录错误 → 用户可读中文（不含任何凭据内容）。
pub fn map_cas_login_error(e: crate::vpn::cas::CasLoginError) -> String {
    match e {
        crate::vpn::cas::CasLoginError::WrongUserOrPwd => "CAS 登录失败：账号或密码错误（请检查保存的 VPN 凭据）".to_string(),
        crate::vpn::cas::CasLoginError::WrongCaptcha => "CAS 登录失败：服务端要求验证码（异常风控，请稍后重试）".to_string(),
        crate::vpn::cas::CasLoginError::UserLocked => "CAS 登录失败：账号已被锁定".to_string(),
        crate::vpn::cas::CasLoginError::NeedTwoVerify(raw) => format!("CAS 登录失败：需要二次验证（{raw}）"),
        crate::vpn::cas::CasLoginError::Unknown(raw) => format!("CAS 登录失败：{raw}"),
        crate::vpn::cas::CasLoginError::Network(raw) => format!("CAS 登录网络错误：{raw}"),
    }
}

/// WebVPN 协议错误 → 用户可读中文（不含任何凭据内容）。
pub fn map_webvpn_error(e: crate::vpn::WebVpnError) -> String {
    match e {
        crate::vpn::WebVpnError::Network(raw) => format!("WebVPN 网关连接失败：{raw}"),
        crate::vpn::WebVpnError::Sso(raw) => format!("WebVPN 单点登录失败：{raw}"),
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
        let s = map_cas_login_error(crate::vpn::cas::CasLoginError::WrongUserOrPwd);
        assert!(s.contains("账号或密码错误"), "{s}");
        assert!(s.contains("VPN 凭据"), "{s}");
        let s = map_cas_login_error(crate::vpn::cas::CasLoginError::Unknown("PASSERROR".to_string()));
        assert!(s.contains("PASSERROR"), "{s}");
        let s = map_webvpn_error(crate::vpn::WebVpnError::InvalidIv);
        assert!(s.contains("WebVPN"), "{s}");
        let s = map_cas_auth_error(CasAuthError::Rsa("非 ASCII".to_string()));
        assert!(s.contains("RSA"), "{s}");
    }
}
