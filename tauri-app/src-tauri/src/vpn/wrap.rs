//! WebVPN 网关 URL 拼装（[`wrap_url`]）。

use super::crypto::encrypt_host;
use super::WebVpnError;
use url::Url;

/// 本校 WebVPN 网关基址。后续批次（会话与路由）统一引用本常量，勿在别处复制。
pub const GATEWAY: &str = "https://webvpn.cwxu.edu.cn";

/// 把任意 http/https URL 包装为 WebVPN 网关 URL。
///
/// - `gateway`：网关基址，一般传 [`GATEWAY`]；
/// - 非默认端口写作 `{scheme}-{port}`（如 `/http-8080/`），默认端口（443/80）省略；
/// - path、query、fragment 原样保留；
/// - 输入 host 已是网关域（幂等）时原样返回，不做二次包装。
pub fn wrap_url(raw: &str, gateway: &str) -> Result<String, WebVpnError> {
    let gw = Url::parse(gateway)?;
    let gw_host = gw.host_str().ok_or(WebVpnError::MissingHost)?;

    let url = Url::parse(raw)?;
    let scheme = url.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(WebVpnError::UnsupportedScheme(scheme.to_string()));
    }
    let host = url.host_str().ok_or(WebVpnError::MissingHost)?;
    if host.eq_ignore_ascii_case(gw_host) {
        return Ok(raw.to_string());
    }

    let default_port = if scheme == "https" { 443 } else { 80 };
    let seg = match url.port() {
        Some(p) if p != default_port => format!("{scheme}-{p}"),
        _ => scheme.to_string(),
    };

    let mut tail = url.path().to_string();
    if let Some(q) = url.query() {
        tail.push('?');
        tail.push_str(q);
    }
    if let Some(f) = url.fragment() {
        tail.push('#');
        tail.push_str(f);
    }

    Ok(format!(
        "{}/{}/{}{}",
        gw.as_str().trim_end_matches('/'),
        seg,
        encrypt_host(host),
        tail
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // hex(IV) 固定前缀（= hex(key)），golden 期望值用完整常量对照
    const HEX_MY: &str = "77726476706e69737468656265737421fdee0f9f30287d1e7b0c9ce29b5b";
    const HEX_103: &str = "77726476706e69737468656265737421a1a70fcf696138003059d8fc";

    #[test]
    fn wrap_path_and_query() {
        assert_eq!(
            wrap_url("http://10.3.100.110/charge/feeitem", GATEWAY).unwrap(),
            format!("{GATEWAY}/http/{HEX_103}/charge/feeitem")
        );
        assert_eq!(
            wrap_url("http://10.3.100.110/charge/feeitem?a=1&b=%E4%B8%AD", GATEWAY).unwrap(),
            format!("{GATEWAY}/http/{HEX_103}/charge/feeitem?a=1&b=%E4%B8%AD")
        );
    }

    #[test]
    fn wrap_port_forms() {
        // 非默认端口 → {scheme}-{port}
        assert_eq!(
            wrap_url("http://10.3.100.110:8080/x", GATEWAY).unwrap(),
            format!("{GATEWAY}/http-8080/{}/x", encrypt_host("10.3.100.110"))
        );
        // 默认端口显式给出 → 省略
        assert_eq!(
            wrap_url("https://my.cwxu.edu.cn:443/login", GATEWAY).unwrap(),
            format!("{GATEWAY}/https/{HEX_MY}/login")
        );
        // 无端口 → 省略
        assert_eq!(
            wrap_url("https://my.cwxu.edu.cn/login", GATEWAY).unwrap(),
            format!("{GATEWAY}/https/{HEX_MY}/login")
        );
    }

    #[test]
    fn wrap_keeps_fragment() {
        assert_eq!(
            wrap_url("http://10.3.100.110/app#/home", GATEWAY).unwrap(),
            format!("{GATEWAY}/http/{HEX_103}/app#/home")
        );
    }

    #[test]
    fn wrap_root_path_gets_trailing_slash() {
        assert_eq!(
            wrap_url("http://10.3.100.110", GATEWAY).unwrap(),
            format!("{GATEWAY}/http/{HEX_103}/")
        );
    }

    #[test]
    fn wrap_is_idempotent_for_gateway_host() {
        let wrapped = format!("{GATEWAY}/https/{HEX_MY}/login?next=%2Fx");
        assert_eq!(wrap_url(&wrapped, GATEWAY).unwrap(), wrapped);
    }

    #[test]
    fn wrap_rejects_ftp() {
        assert!(matches!(
            wrap_url("ftp://10.3.100.110/file", GATEWAY),
            Err(WebVpnError::UnsupportedScheme(s)) if s == "ftp"
        ));
    }

    #[test]
    fn wrap_rejects_garbage() {
        assert!(matches!(
            wrap_url("::not a url::", GATEWAY),
            Err(WebVpnError::InvalidUrl(_))
        ));
    }
}
