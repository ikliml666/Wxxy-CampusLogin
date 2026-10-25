//! WebVPN 路由决策（纯函数，无 IO）：网络归属 × 目标地址 × 会话状态 → 走法。
//!
//! # 决策表（[`route`]）
//!
//! | zone | 目标 | 有 WebVPN 会话 | 决策 |
//! |---|---|---|---|
//! | Campus / Unknown | 任意 | 任意 | [`RouteDecision::Direct`] |
//! | OffCampus | 公网域名（非内网目标） | 任意 | `Direct` |
//! | OffCampus | 内网目标 | true | `Wrapped(wrap_url 结果)` |
//! | OffCampus | 内网目标 | false | [`RouteDecision::NeedLogin`] |
//!
//! - **Unknown 按直连处理**：归属不明时不拦截用户（校内为主场景，误拦成本高于误放）；
//!   直连失败由上层以中文报错兜底，**本轮不做「先直连失败再自动改走 WebVPN」的二次尝试**
//!   （读路径可承受一次失败重试，但写路径二次尝试触碰资金安全红线，宁可不自动化）。
//! - **公网域名永远 Direct**：门户 `my.cwxu.edu.cn`、CAS `wxcas.cwxu.edu.cn` 校外本就可达
//!   （学校公网入口），WebVPN 包装公网域名的收益为 0，反而引入网关 cookie 域隔离与
//!   service 白名单复杂度，故不进包装范围。
//! - **内网判据（保守）**：目标 host 是 IP 字面量且 ∈ 10/8，或命中已知内网 host 表
//!   [`INTERNAL_HOSTS`]。`.cwxu.edu.cn` 通配等更宽的判据留待有实锤服务再放。
//! - [`RouteDecision::Unreachable`] 本函数**不产生**（预留枚举位）：「无可用路径」类判定
//!   （如网关不可达）需要网络观测，属上层职责；route 只做静态决策。
//!
//! # [`NetZone`] 与应用层的同构关系
//!
//! - [`NetZone`] 与应用层：上层（self_service 命令层）自行探测网络归属后逐变体
//!   映射为本枚举（Campus/Unknown/OffCampus 三态同名同义），底层不反向依赖应用层。

use super::wrap::{wrap_url, GATEWAY};
use std::net::Ipv4Addr;

/// 网络归属三态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetZone {
    /// 在校园网内。
    Campus,
    /// 确定在校外。
    OffCampus,
    /// 无法判定（按直连处理）。
    Unknown,
}

/// 一条请求的走法。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteDecision {
    /// 直连原始 URL（现状行为）。
    Direct,
    /// 走 WebVPN 网关：值为包装后的完整 URL（`wrap_url` 结果，幂等保证不二次包装）。
    Wrapped(String),
    /// 校外 + 内网目标 + 无 WebVPN 会话：需要先登录（上层给「请重新登录」类文案）。
    NeedLogin,
    /// 无可用路径（**本函数不产生**，预留位，见模块头注）。
    Unreachable(String),
}

/// 已知内网 host 表（非 IP 形态的内网域名将来加在这里；`10.1.80.200` 同时被
/// 10/8 IP 判定覆盖，列出只为显式钉住当前唯一目标——自助服务 SELF_BASE_URL 的
/// host，防将来改判据时漏掉）。
const INTERNAL_HOSTS: &[&str] = &["10.1.80.200"];

/// 目标是否为「校外必须经网关才可达」的内网地址（纯函数，单测覆盖）。
///
/// 已是网关域的 URL（包装形态）视为内网目标：[`wrap_url`] 对网关 host 幂等原样返回，
/// 上层因此拿到 `Wrapped(原样)` 而不会二次包装（幂等决策的落点）。
fn is_internal_target(raw_url: &str) -> bool {
    let Ok(u) = url::Url::parse(raw_url) else {
        return false;
    };
    let Some(host) = u.host_str() else {
        return false;
    };
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        return ip.octets()[0] == 10;
    }
    let gw_host = url::Url::parse(GATEWAY)
        .ok()
        .and_then(|g| g.host_str().map(str::to_string));
    if let Some(gw) = gw_host {
        if host.eq_ignore_ascii_case(&gw) {
            return true;
        }
    }
    INTERNAL_HOSTS.iter().any(|h| host.eq_ignore_ascii_case(h))
}

/// 路由决策（纯函数，无 IO；决策表见模块头注）。
pub fn route(raw_url: &str, zone: NetZone, has_vpn_session: bool) -> RouteDecision {
    match zone {
        NetZone::Campus | NetZone::Unknown => RouteDecision::Direct,
        NetZone::OffCampus => {
            if !is_internal_target(raw_url) {
                // 公网域名永远直连（校外可达，包装无收益，见模块头注）
                return RouteDecision::Direct;
            }
            if !has_vpn_session {
                return RouteDecision::NeedLogin;
            }
            match wrap_url(raw_url, GATEWAY) {
                Ok(w) => RouteDecision::Wrapped(w),
                // 包装失败（非 http/https 等畸形输入）：内网目标不该以这种形态出现，
                // 保守直连并交由上层中文报错，绝不做静默二次尝试
                Err(_) => RouteDecision::Direct,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BERSERKER: &str = "http://10.3.100.110";
    /// wrap_url 的内网 golden 前缀（host 段 = encrypt_host("10.3.100.110")）。
    const HEX_103: &str = "77726476706e69737468656265737421a1a70fcf696138003059d8fc";
    /// 本项目自助服务 host 的包装 golden（host 段 = encrypt_host("10.1.80.200")，
    /// 2026-10-25 移植时实测）。
    const SELF_HEX: &str = "77726476706e69737468656265737421a1a70fcd6968381e2c58d9";

    /// 本项目真实目标（自助服务 SELF_BASE_URL host:port）校外+有会话必须包装。
    #[test]
    fn self_service_host_wraps_offcampus() {
        let base = "http://10.1.80.200:8080/Self";
        let d = route(base, NetZone::OffCampus, true);
        assert_eq!(
            d,
            RouteDecision::Wrapped(format!("{GATEWAY}/http-8080/{SELF_HEX}/Self")),
            "10.1.80.200:8080 非默认端口必须以 http-8080 scheme 段包装"
        );
        // 带业务 path 的完整 URL 同样包装
        let d = route(&format!("{base}/bill/queryUserAccountInfo"), NetZone::OffCampus, true);
        assert!(matches!(d, RouteDecision::Wrapped(w) if w.ends_with("/bill/queryUserAccountInfo")));
    }

    // ---------- 决策表：Campus / Unknown ----------

    #[test]
    fn campus_always_direct() {
        assert_eq!(route(BERSERKER, NetZone::Campus, false), RouteDecision::Direct);
        assert_eq!(route(BERSERKER, NetZone::Campus, true), RouteDecision::Direct);
        assert_eq!(
            route("https://my.cwxu.edu.cn/login", NetZone::Campus, false),
            RouteDecision::Direct
        );
    }

    #[test]
    fn unknown_direct_by_design() {
        // Unknown 不拦截（校内为主场景）；直连失败由上层兜底，不做自动二次尝试
        assert_eq!(route(BERSERKER, NetZone::Unknown, false), RouteDecision::Direct);
        assert_eq!(route(BERSERKER, NetZone::Unknown, true), RouteDecision::Direct);
    }

    // ---------- 决策表：OffCampus × 内网 ----------

    #[test]
    fn offcampus_internal_with_session_wraps() {
        let d = route(&format!("{BERSERKER}/charge/feeitem"), NetZone::OffCampus, true);
        assert_eq!(
            d,
            RouteDecision::Wrapped(format!(
                "{GATEWAY}/http/{HEX_103}/charge/feeitem"
            ))
        );
        // 裸 base（无 path）也要可包装（命令层拿它当业务 base 拼 path）
        let d = route(BERSERKER, NetZone::OffCampus, true);
        assert!(matches!(d, RouteDecision::Wrapped(w) if w.starts_with(&format!("{GATEWAY}/http/{HEX_103}"))));
    }

    #[test]
    fn offcampus_internal_without_session_needs_login() {
        assert_eq!(
            route(&format!("{BERSERKER}/charge/feeitem"), NetZone::OffCampus, false),
            RouteDecision::NeedLogin
        );
        assert_eq!(route(BERSERKER, NetZone::OffCampus, false), RouteDecision::NeedLogin);
    }

    // ---------- 决策表：OffCampus × 公网 ----------

    #[test]
    fn offcampus_public_domain_always_direct() {
        for url in [
            "https://my.cwxu.edu.cn/login",
            "https://wxcas.cwxu.edu.cn/lyuapServer/login",
            "https://example.com/x",
        ] {
            assert_eq!(route(url, NetZone::OffCampus, true), RouteDecision::Direct, "{url}");
            assert_eq!(route(url, NetZone::OffCampus, false), RouteDecision::Direct, "{url}");
        }
    }

    // ---------- 幂等：已包装 URL 原样 Wrapped，不二次包装 ----------

    #[test]
    fn wrapped_url_routes_back_wrapped_verbatim() {
        let wrapped = format!("{GATEWAY}/http/{HEX_103}/berserker-app/ykt/tsm/queryCurrentCard");
        assert_eq!(
            route(&wrapped, NetZone::OffCampus, true),
            RouteDecision::Wrapped(wrapped.clone()),
            "已包装 URL 必须原样通过（host 是网关域 → 视为内网目标 → wrap_url 幂等）"
        );
    }

    // ---------- 内网判据 ----------

    #[test]
    fn internal_target_judgement() {
        // 10/8 IP 字面量
        assert!(is_internal_target("http://10.3.100.110/x"));
        assert!(is_internal_target("http://10.3.100.110:8080/x"));
        assert!(is_internal_target("http://10.0.0.1/"));
        // 非 10/8 一律不算内网目标（私网 192.168/172.16 也不是本校服务）
        assert!(!is_internal_target("http://192.168.1.1/"));
        assert!(!is_internal_target("http://172.16.0.1/"));
        // 公网域名
        assert!(!is_internal_target("https://my.cwxu.edu.cn/"));
        // 网关域（已包装形态）
        assert!(is_internal_target(&format!("{GATEWAY}/http/{HEX_103}/x")));
        // 畸形输入不 panic
        assert!(!is_internal_target("::not a url::"));
        // scheme 不参与内网判定（host 即事实）；不可包装的 scheme 在 route 层兜底直连
        assert!(is_internal_target("ftp://10.3.100.110/f"));
    }

    #[test]
    fn offcampus_internal_bad_scheme_falls_back_to_direct() {
        // 非 http/https 的「内网目标」包装不了：保守直连（上层报错），不 NeedLogin 不 panic
        assert_eq!(
            route("ftp://10.3.100.110/file", NetZone::OffCampus, true),
            RouteDecision::Direct
        );
    }

    // ---------- Unreachable 预留位 ----------

    #[test]
    fn unreachable_is_never_produced_by_route() {
        // 决策表全组合扫描：Unreachable 不该由本函数产生（预留枚举位，见模块头注）
        for zone in [NetZone::Campus, NetZone::OffCampus, NetZone::Unknown] {
            for has in [false, true] {
                for url in [BERSERKER, "https://my.cwxu.edu.cn/", "ftp://10.3.100.110/f"] {
                    assert!(
                        !matches!(route(url, zone, has), RouteDecision::Unreachable(_)),
                        "route({url}, {zone:?}, {has}) 不应产生 Unreachable"
                    );
                }
            }
        }
    }
}
