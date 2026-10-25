//! CAS 统一身份认证客户端（最小集）：账密登录拿 TGT + TGT 换 ST。
//!
//! 移植自 Wxxy-CampusHub crates/campus-auth/src/cas.rs（2026-10-25），按本校
//! 「自助服务经 WebVPN 访问」需求裁剪：本校 CAS 实测无验证码，kaptcha 相关
//! （CaptchaInfo/parse_kaptcha）与门户探测（portal_probe/portal_*）、头像上传、
//! csrf_token 全部裁掉；`sso_ticket` 抽自同仓库 crates/campus-auth/src/jwglxt.rs
//! 的 `impl CasClient`（TGT 换任意 service 的 ST，WebVPN 登录要用）。
//!
//! 协议事实全部来自 Wxxy-CampusHub docs/cas-recon/REPORT.md（实测打通）：
//! - 无 Cookie 依赖即可完成 CAS 登录；成功响应顶层即 `{tgt, ticket}`
//! - `POST /v1/tickets`：x-www-form-urlencoded，头 `token` = RSA("lyasp"+毫秒)
//! - 错误码全集 16 个，映射见 [`map_error_code`]

use super::error::CasAuthError;
use super::jar::RecordingJar;
use super::rsa::cas_token_header;
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub const CAS_BASE: &str = "https://wxcas.cwxu.edu.cn/lyuapServer";
/// CAS 登录默认注册的 service（门户）；本校 WebVPN 场景实际用 [`WEBVPN_SERVICE`]。
pub const PORTAL_SERVICE: &str = "https://my.cwxu.edu.cn/shiro-cas";
/// WebVPN 网关在 CAS 端注册的 service（session 层 sso_ticket 换票用）。
pub const WEBVPN_SERVICE: &str = "https://webvpn.cwxu.edu.cn/login?cas_login=true";

/// 真实 Chrome UA（cas.js 同款）。
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

/// CAS 客户端（内部 `.cookie_provider(Arc<RecordingJar>)` 挂载记录型 Jar）。
/// Clone 廉价（reqwest::Client 为 Arc 包装）。
#[derive(Clone)]
pub struct CasClient {
    http: reqwest::Client,
}

/// CAS 登录成功结果。
#[derive(Debug, Clone)]
pub struct CasLoginOk {
    pub tgt: String,
    /// 协议响应完整形态（REPORT.md 三节：顶层 `{tgt, ticket}`）；
    /// 当前流程仅消费 tgt，ticket 由 parse 测试锚定。
    #[allow(dead_code)]
    pub ticket: String,
}

/// CAS 登录失败（错误码全集与映射以 REPORT.md 为准）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CasLoginError {
    /// NOUSER：账号或密码错误
    WrongUserOrPwd,
    /// CODEFALSE：验证码错误（本校实测无验证码，保留映射以兼容服务端风控）
    WrongCaptcha,
    /// USERLOCK：账号锁定
    UserLocked,
    /// TWOVERIFY：需要二次验证（携带服务端 data 原文，含可能的错误次数提示）
    NeedTwoVerify(String),
    /// 其余错误码（USERDISABLED / PASSERROR / ISMODIFYPASS / … 原样上抛）
    Unknown(String),
    /// 网络层错误
    Network(String),
}

impl CasClient {
    pub fn new() -> Result<Self, CasAuthError> {
        let jar = Arc::new(RecordingJar::new());
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .cookie_provider(jar.clone())
            .build()?;
        Ok(Self { http })
    }

    /// POST {CAS_BASE}/v1/tickets：CAS 账密登录。
    ///
    /// `password_rsa_hex` 为 [`super::rsa::rsa_encrypt_hex`] 的密文；头 `token` =
    /// [`cas_token_header`]（毫秒时间戳在方法内取当前时刻）。
    /// 本校实测无验证码：body 的 id/code 字段传空串（服务端不校验）。
    pub async fn login(
        &self,
        username: &str,
        password_rsa_hex: &str,
    ) -> Result<CasLoginOk, CasLoginError> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| CasLoginError::Network(e.to_string()))?
            .as_millis() as u64;
        let token = cas_token_header(now_ms).map_err(|e| CasLoginError::Network(e.to_string()))?;
        let body = build_login_body(username, password_rsa_hex, PORTAL_SERVICE, "", "");
        let text = self
            .http
            .post(format!("{CAS_BASE}/v1/tickets"))
            .header("token", token)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .header(
                reqwest::header::REFERER,
                format!("{CAS_BASE}/login?service={}", form_encode(PORTAL_SERVICE)),
            )
            .header(reqwest::header::ORIGIN, "https://wxcas.cwxu.edu.cn")
            .body(body)
            .send()
            .await
            .map_err(|e| CasLoginError::Network(e.to_string()))?
            .text()
            .await
            .map_err(|e| CasLoginError::Network(e.to_string()))?;
        parse_login_response(&text)
    }

    /// CAS REST v1 换票：`POST {CAS_BASE}/v1/tickets/{tgt}`（x-www-form-urlencoded，
    /// body `service=<service>`）→ 响应体纯文本即新 ST。
    ///
    /// 实测 200 + `ST-…`；非 `ST-` 开头（含错误页/JSON）→ [`CasAuthError::Parse`]。
    /// TGT 来自 `CasLoginOk.tgt`，**必须随会话持久化**——CAS 服务端
    /// 不种任何登录 cookie（jar 实测登录后为空），会话复用全靠客户端保存 TGT。
    /// （抽自 Wxxy-CampusHub crates/campus-auth/src/jwglxt.rs `impl CasClient`。）
    pub async fn sso_ticket(&self, tgt: &str, service: &str) -> Result<String, CasAuthError> {
        let body = self
            .http
            .post(format!("{CAS_BASE}/v1/tickets/{tgt}"))
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .form(&[("service", service)])
            .send()
            .await?
            .text()
            .await?;
        let st = body.trim();
        if st.starts_with("ST-") {
            Ok(st.to_string())
        } else {
            Err(CasAuthError::Parse(format!(
                "CAS 换票响应非 ST（前缀 {:?}，长度 {}）",
                st.chars().take(3).collect::<String>(),
                st.len()
            )))
        }
    }
}

/// 构造 CAS 登录请求体（application/x-www-form-urlencoded），纯函数供离线单测。
///
/// 字段与 cas.js 参照实现一致（REPORT.md 一节协议体，共 7 项）：
/// username / password(密文) / service / loginType(空) / id=验证码uid / code=验证码答案 /
/// otpcode(空)。
pub fn build_login_body(
    username: &str,
    password_rsa_hex: &str,
    service: &str,
    captcha_uid: &str,
    captcha_code: &str,
) -> String {
    let pairs = [
        ("username", username),
        ("password", password_rsa_hex),
        ("service", service),
        ("loginType", ""),
        ("id", captcha_uid),
        ("code", captcha_code),
        ("otpcode", ""),
    ];
    pairs
        .iter()
        .map(|(k, v)| format!("{k}={}", form_encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// application/x-www-form-urlencoded 值编码：字母数字与 `* - . _` 保留、空格→`+`、其余 %XX。
fn form_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => {
                out.push(*b as char);
            }
            b' ' => out.push('+'),
            _ => {
                let _ = write!(out, "%{b:02X}");
            }
        }
    }
    out
}

/// 解析 /v1/tickets 响应（纯函数供离线单测）。
///
/// 成功：顶层 `{tgt, ticket}`（REPORT.md 三节实测形态；兼容 data 包裹与 data 为纯字符串）。
/// 失败：`{"meta":...,"data":{"code":"..."}}` → [`map_error_code`]。
pub fn parse_login_response(body: &str) -> Result<CasLoginOk, CasLoginError> {
    let v: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| CasLoginError::Unknown(format!("响应非 JSON: {e}")))?;
    if let Some(code) = v
        .get("data")
        .and_then(|d| d.get("code"))
        .and_then(|c| c.as_str())
    {
        let data_raw = v
            .get("data")
            .map(|d| d.to_string())
            .unwrap_or_else(|| code.to_string());
        return Err(map_error_code(code, &data_raw));
    }
    let str_of = |obj: &serde_json::Value, key: &str| {
        obj.get(key).and_then(|x| x.as_str()).map(str::to_string)
    };
    let tgt = str_of(&v, "tgt").or_else(|| v.get("data").and_then(|d| str_of(d, "tgt")));
    // cas.js 兼容：data 为纯字符串时也把它当 ticket
    let ticket = str_of(&v, "ticket")
        .or_else(|| v.get("data").and_then(|d| str_of(d, "ticket")))
        .or_else(|| v.get("data").and_then(|d| d.as_str()).map(str::to_string));
    match (tgt, ticket) {
        (Some(tgt), Some(ticket)) if !tgt.is_empty() && !ticket.is_empty() => {
            Ok(CasLoginOk { tgt, ticket })
        }
        (tgt, ticket) => Err(CasLoginError::Unknown(format!(
            "响应缺少 tgt/ticket（tgt={tgt:?} ticket={ticket:?}）"
        ))),
    }
}

/// 错误码映射：REPORT.md 全集 16 码 —— NOUSER / CODEFALSE / USERLOCK / TWOVERIFY
/// 专属映射，其余 12 码归 Unknown(原样码)。
fn map_error_code(code: &str, data_raw: &str) -> CasLoginError {
    match code {
        "NOUSER" => CasLoginError::WrongUserOrPwd,
        "CODEFALSE" => CasLoginError::WrongCaptcha,
        "USERLOCK" => CasLoginError::UserLocked,
        "TWOVERIFY" => CasLoginError::NeedTwoVerify(data_raw.to_string()),
        other => CasLoginError::Unknown(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// build_login_body 七字段形态锁（form 编码规则：字母数字与 *-._ 保留、
    /// 空格→+、其余 %XX；占位值非真实数据）。
    #[test]
    fn login_body_seven_fields_and_form_encoding() {
        let body = build_login_body("2023001", "aef123", PORTAL_SERVICE, "", "");
        assert_eq!(
            body,
            "username=2023001&password=aef123&service=https%3A%2F%2Fmy.cwxu.edu.cn%2Fshiro-cas&loginType=&id=&code=&otpcode="
        );
        // service 含 & 时不得破坏 form 结构（& 必须编码为 %26）
        let body2 = build_login_body("u", "p", "https://x.test/a?b=1&c=2", "", "");
        assert!(body2.contains("service=https%3A%2F%2Fx.test%2Fa%3Fb%3D1%26c%3D2"));
    }

    /// 实测成功形态：顶层 {tgt, ticket}（占位值非真实数据）。
    #[test]
    fn login_response_top_level_ok() {
        let ok = parse_login_response(r#"{"tgt":"TGT-1","ticket":"ST-1"}"#).unwrap();
        assert_eq!(ok.tgt, "TGT-1");
        assert_eq!(ok.ticket, "ST-1");
    }

    /// 兼容 data 包裹与 data 为纯字符串 ticket（cas.js 兼容形态）。
    #[test]
    fn login_response_data_wrapped_variants() {
        let ok = parse_login_response(r#"{"data":{"tgt":"TGT-2","ticket":"ST-2"}}"#).unwrap();
        assert_eq!(ok.tgt, "TGT-2");
        let ok = parse_login_response(r#"{"tgt":"TGT-3","data":"ST-3"}"#).unwrap();
        assert_eq!(ok.ticket, "ST-3");
    }

    /// 错误码映射：NOUSER→WrongUserOrPwd / CODEFALSE→WrongCaptcha /
    /// USERLOCK→UserLocked / TWOVERIFY→NeedTwoVerify(原文) / 其余 Unknown。
    #[test]
    fn login_error_codes_map() {
        assert_eq!(
            parse_login_response(r#"{"meta":{},"data":{"code":"NOUSER"}}"#).unwrap_err(),
            CasLoginError::WrongUserOrPwd
        );
        assert_eq!(
            parse_login_response(r#"{"data":{"code":"CODEFALSE"}}"#).unwrap_err(),
            CasLoginError::WrongCaptcha
        );
        assert_eq!(
            parse_login_response(r#"{"data":{"code":"USERLOCK"}}"#).unwrap_err(),
            CasLoginError::UserLocked
        );
        match parse_login_response(r#"{"data":{"code":"TWOVERIFY","extra":1}}"#).unwrap_err() {
            CasLoginError::NeedTwoVerify(raw) => assert!(raw.contains("TWOVERIFY")),
            other => panic!("应为 NeedTwoVerify: {other:?}"),
        }
        assert!(matches!(
            parse_login_response(r#"{"data":{"code":"PASSERROR"}}"#).unwrap_err(),
            CasLoginError::Unknown(_)
        ));
        assert!(matches!(
            parse_login_response("not json").unwrap_err(),
            CasLoginError::Unknown(_)
        ));
    }
}
