//! 深澜（Srun）WebVPN 协议核心（移植自 Wxxy-CampusHub，2026-10-25）。
//!
//! # 能力
//!
//! - [`crypto`]：AES-128-CFB128 host 加密/解密（网关包装 URL 的核心算子）；
//! - [`wrap`]：校内 URL → WebVPN 网关包装 URL（含幂等与端口形态）；
//! - [`cas`]：CAS 统一身份认证（账密登录拿 TGT、TGT 换 ST）；
//! - [`session`]：WebVPN 会话（CAS TGT 静默登录 / 探活 / 业务 client 句柄）；
//! - [`route`]：网络归属 → 直连/包装 决策表（自助服务校外自动回退的路由核心）。
//!
//! 协议代码单点存在于桌面 crate（安卓经 Cargo path 依赖自动继承，禁止复制到
//! android/，见 AGENTS.md 通用改进双端同步的第 1 类例外）。本模块无 Tauri
//! 依赖、无 Windows 专属 API，依赖全部纯 Rust（aes/cfb-mode/hex/num-bigint-dig），
//! Windows/Android 双端可编译。
//!
//! # 算法（逆向自 webvpn.cwxu.edu.cn 网关，活样本见 Wxxy-CampusHub docs/cas-recon/REPORT.md）
//!
//! 包装式：`{gateway}/{scheme}/hex(IV)+hex(AES-128-CFB128(host))` + 原 path?query
//!
//! - key = IV = 16 字节 ASCII 常量（[`crypto`] 模块内单一来源）；
//! - AES-128-CFB128 为流式模式，密文长度恒等于明文字节长度（无 padding）；
//! - 输出前缀 hex(IV) 由 key **派生计算**得出，调用方禁止硬编码该 hex 字符串；
//! - scheme 段跟随目标 scheme（https → `/https/`、http → `/http/`），非默认端口写作
//!   `{scheme}-{port}`；仅支持 http/https；
//! - 输入 host 已是网关域时幂等原样返回（见 [`wrap::wrap_url`]）。

pub mod cache;
pub mod cas;
pub mod crypto;
pub mod error;
pub mod jar;
pub mod route;
pub mod rsa;
pub mod session;
pub mod wrap;

pub use error::CasAuthError;
pub use route::{route, NetZone, RouteDecision};
pub use session::WebVpnSession;
pub use wrap::{wrap_url, GATEWAY};

/// WebVPN 协议层统一错误类型（移植自 Wxxy-CampusHub crates/campus-webvpn/src/lib.rs，
/// Sso 变体由 campus_auth::CampusAuthError 改为本地 [`CasAuthError`]）。
#[derive(Debug, thiserror::Error)]
pub enum WebVpnError {
    #[error("invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("unsupported scheme `{0}`: WebVPN only wraps http/https targets")]
    UnsupportedScheme(String),
    #[error("URL has no host component")]
    MissingHost,
    #[error("invalid hex ciphertext: {0}")]
    InvalidHex(#[from] hex::FromHexError),
    #[error("IV segment missing or does not equal the fixed gateway IV")]
    // 仅 decrypt_host（测试锚定的对称实现，见 crypto.rs）构造
    #[allow(dead_code)]
    InvalidIv,
    #[error("decrypted host is not valid UTF-8: {0}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
    /// 会话层网络错误（登录 302 链 / 探活请求失败）。
    #[error("WebVPN HTTP request failed: {0}")]
    Network(#[from] reqwest::Error),
    /// CAS 换票失败（TGT 失效 / 网络 / 解析，原样透传本地 CAS 错误链）。
    #[error("WebVPN SSO failed: {0}")]
    Sso(#[from] CasAuthError),
}
