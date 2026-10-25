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
