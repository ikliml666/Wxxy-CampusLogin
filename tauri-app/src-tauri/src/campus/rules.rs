//! 校园网判定的跨平台纯规则层：双端（桌面 `monitor::campus_check` /
//! `monitor::outbound_switch`，安卓 `campus_detect`）共用的判定口径，不含任何
//! 网络 IO 与平台 API——探测方式（桌面绑源 ICMP、安卓 TCP）由调用方经闭包注入。
//!
//! 抽出的起因（2026-11-10）：同一套判定在仓库里有 4 份各自演化的实现，
//! 名称名单解析、portal 主机提取、去抖次数三处口径已经不一致，
//! 其中安卓 `ssid_matches` 对三值默认名单整体比较字符串，名称腿恒假。

/// portal_url 的 origin（chkstatus 与 portal 同源，eportal 按请求源 IP 判定本机）；
/// 空值回退默认 portal 地址（`config::model::default_portal_url`）。
///
/// 双端共用同一份：桌面 `monitor::scheduled` 的夜切验证与安卓
/// `monitor_loop::night_switch_check_once` 拼的是同一个 `chkstatus` URL，
/// 各自维护会在 origin 提取口径上漂移（路径/端口/无 scheme 三个分支）。
pub fn portal_origin(portal_url: &str) -> String {
    let url = if portal_url.is_empty() {
        crate::config::model::default_portal_url()
    } else {
        portal_url.to_string()
    };
    match url.split_once("://") {
        Some((scheme, rest)) => format!("{scheme}://{}", rest.split('/').next().unwrap_or("")),
        None => url.split('/').next().unwrap_or("").to_string(),
    }
}

/// 期望运营商的展示名：空串=校方默认线路（无锡学院），其余原样（@telecom 等）。
/// 双端共用于夜切验证的"在线账号与期望运营商不符"文案。
pub fn expected_operator_display(expected_operator: &str) -> &str {
    if expected_operator.is_empty() {
        "无锡学院"
    } else {
        expected_operator
    }
}

/// 单腿探测的去抖次数：单发丢包会把在网校园卡误判成非校园，导致禁用步骤静默
/// 跳过、整夜只切一半。任一发可达即判该腿命中，容忍瞬时抖动。
///
/// 调用方必须让这 N 次探测**真的发出**：若 probe 闭包带惰性缓存（同一 (target,
/// source) 第二次直接命中上一次结果），去抖会被静默退回成 1 次尝试。生产调用方
/// 一律先经预热批次并行把 N 份探测发完，判定阶段只查表——
/// 见 `monitor/campus_check.rs::warm_probe_cache` 与
/// `monitor/outbound_night.rs::warm_probe_cache`。
pub const CAMPUS_PROBE_ATTEMPTS: usize = 2;

/// 校园网名称名单解析：配置值支持顿号/全半角逗号/分号/空白分隔多个名称
/// （默认 `i-wxxy、iwxxy-2、iwxxy-3`），空项忽略。
pub fn campus_name_list(value: &str) -> Vec<&str> {
    value
        .split(['、', '，', ',', '；', ';', ' ', '\t'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// 名称是否命中配置的校园网名单（大小写不敏感，逐项比较）。
pub fn matches_campus_name(name: &str, configured: &str) -> bool {
    !name.is_empty()
        && campus_name_list(configured)
            .iter()
            .any(|n| name.eq_ignore_ascii_case(n))
}

/// 从 portal_url 提取可作绑源探测目标的私网 IPv4 主机（`http://10.1.99.100` →
/// `10.1.99.100`）。域名、公网地址、IPv6、空值一律返回空串：portal 可达性是
/// 「从校园侧出口」的证据，公网 portal 会把任意外网网卡都误判成校园。
pub fn portal_probe_host(portal_url: &str) -> String {
    let rest = portal_url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(portal_url);
    let host_port = rest.split('/').next().unwrap_or("");
    let host = host_port.rsplit_once(':').map(|(h, _)| h).unwrap_or(host_port);
    if host.parse::<std::net::Ipv4Addr>().is_ok() && is_nat_private_ip(host) {
        host.to_string()
    } else {
        String::new()
    }
}

/// 私网/特殊用途 IPv4 判定（RFC1918 三段 + 链路本地 + CGNAT）。
/// 原本只在 `auth::portal` 内部使用，2026-11-10 上移为共用规则，
/// `auth::portal` 转导出本函数以免两份实现漂移。
pub fn is_nat_private_ip(ip: &str) -> bool {
    if ip.starts_with("10.") {
        return true;
    }
    if ip.starts_with("192.168.") || ip.starts_with("169.254.") {
        return true;
    }
    if let Some(rest) = ip.strip_prefix("172.") {
        if let Some(second) = rest.split('.').next() {
            if let Ok(o) = second.parse::<u8>() {
                return (16..=31).contains(&o);
            }
        }
    }
    if let Some(rest) = ip.strip_prefix("100.") {
        if let Some(second) = rest.split('.').next() {
            if let Ok(o) = second.parse::<u8>() {
                return (64..=127).contains(&o);
            }
        }
    }
    false
}

/// 单腿探测：去抖 [`CAMPUS_PROBE_ATTEMPTS`] 次，任一发可达即命中。
/// `probe` 由调用方注入（桌面绑源 ICMP / 安卓 TCP），空目标直接判否。
pub fn probe_with_retry(target: &str, mut probe: impl FnMut(&str) -> bool) -> bool {
    if target.is_empty() {
        return false;
    }
    (0..CAMPUS_PROBE_ATTEMPTS).any(|_| probe(target))
}

/// 命中的判定腿。UI 文案与 `matchedBy` 字段需要区分「靠网关命中」还是
/// 「靠 Portal 命中」，故判定函数把腿一起返回。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CampusLeg {
    Gateway,
    Portal,
}

/// 判定要探的目标与所属腿，按优先级排列（**Portal 在前、网关最后兜底**）。
///
/// 优先级理由：Portal 主机是该校园网内的私网入口，可达即说明「流量从校园侧出」，
/// 归属证据比网关强；网关可能被子网边界设备代答、或按策略禁 ICMP，
/// 命中它并不等于该卡真的在校园网内，因此降为最后一道兜底。
///
/// 两腿仍是「任一命中即判校园」的或关系，改的只是谁先命中、谁兜底，
/// 判定结果不变；先探 Portal 也让常见的在网校园卡少发一轮网关探测。
///
/// 空目标腿整体跳过：`campus_gateway` 未配置时只剩 Portal 腿；`portal_host`
/// 为空（公网/域名 portal）时只剩网关腿。
///
/// 双端都从这里取「探哪些目标、优先级如何」：桌面逐腿同步探测（Portal 命中即
/// 短路），安卓把同一批目标并行发 TCP（无 ICMP，串行会拉长通知翻转），
/// 但腿的定义与优先级只有这一份。
#[allow(dead_code)] // 桌面 bin 已改为经 judge_campus_health 直接探测；安卓端并行探测仍用这份腿定义
pub fn campus_probe_targets<'a>(
    gateway: &'a str,
    portal_host: &'a str,
) -> Vec<(&'a str, CampusLeg)> {
    let mut targets = Vec::with_capacity(2);
    if !portal_host.is_empty() {
        targets.push((portal_host, CampusLeg::Portal));
    }
    if !gateway.is_empty() {
        targets.push((gateway, CampusLeg::Gateway));
    }
    targets
}

/// 按 [`campus_probe_targets`] 的优先级逐腿探测，返回首个命中的腿。
/// 每腿去抖 [`CAMPUS_PROBE_ATTEMPTS`] 次，任一发可达即算该腿命中。
/// 桌面（绑源 ICMP）与安卓（TCP，结果先并行探好再查表）共用此函数。
pub fn judge_campus_leg(
    gateway: &str,
    portal_host: &str,
    mut probe: impl FnMut(&str) -> bool,
) -> Option<CampusLeg> {
    // 与 judge_campus_health 同一份级联：腿是三态的投影，各写一遍迟早会分叉
    match judge_campus_health(gateway, portal_host, &mut probe) {
        CampusHealth::Online => Some(CampusLeg::Portal),
        CampusHealth::AuthBroken => Some(CampusLeg::Gateway),
        CampusHealth::OffCampus => None,
    }
}

/// 校园网连通状态：在校园网内「认证链路是否正常」。
///
/// 与 CampusLeg 的区别：那个回答「靠哪条腿判定在校园网」，这个回答「在校园网里
/// 认证是否可用」。网关可达但 Portal 不可达，说明流量确实进了校园网、但认证/会话
/// 侧有问题（Portal 未响应、被踢下线、绑定异常），此时需要重新认证，必要时换 MAC
/// 重新获取 IP——这不是「不在校园网」，不能与真正的离网混为一谈。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CampusHealth {
    /// Portal 可达：在校园网内且认证入口正常
    Online,
    /// 仅网关可达、Portal 不可达：在校园网内但认证异常
    AuthBroken,
    /// 两腿都不通：不在校园网
    OffCampus,
}

/// 判定校园网连通状态：Portal 优先（强证据），网关仅作兜底辅助。
///
/// 顺序有语义：先 Portal 才能把「认证正常」与「进了校园网但认证不通」区分开；
/// 若按网关优先，网关可达会先短路，认证异常就被当成正常在线。
pub fn judge_campus_health(
    gateway: &str,
    portal_host: &str,
    mut probe: impl FnMut(&str) -> bool,
) -> CampusHealth {
    if !portal_host.is_empty() && probe_with_retry(portal_host, &mut probe) {
        return CampusHealth::Online;
    }
    if !gateway.is_empty() && probe_with_retry(gateway, &mut probe) {
        return CampusHealth::AuthBroken;
    }
    CampusHealth::OffCampus
}

/// judge_campus_leg 的布尔版：只关心是否在校园网。
pub fn judge_campus(gateway: &str, portal_host: &str, probe: impl FnMut(&str) -> bool) -> bool {
    judge_campus_leg(gateway, portal_host, probe).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn campus_name_list_splits_all_separators() {
        assert_eq!(
            campus_name_list("i-wxxy、iwxxy-2、iwxxy-3"),
            vec!["i-wxxy", "iwxxy-2", "iwxxy-3"]
        );
        assert_eq!(campus_name_list("a, b；c\td"), vec!["a", "b", "c", "d"]);
        assert_eq!(campus_name_list("  、，; "), Vec::<&str>::new());
        assert_eq!(campus_name_list("i-wxxy"), vec!["i-wxxy"]);
    }

    #[test]
    fn matches_campus_name_accepts_any_listed_name() {
        let configured = "i-wxxy、iwxxy-2、iwxxy-3";
        assert!(matches_campus_name("i-wxxy", configured));
        assert!(matches_campus_name("I-WXXY", configured));
        assert!(matches_campus_name("iwxxy-2", configured));
        assert!(matches_campus_name("IWXXY-3", configured));
        assert!(!matches_campus_name("home-wifi", configured));
        assert!(!matches_campus_name("i-wxx", configured));
        assert!(!matches_campus_name("", configured));
        // 单值配置不能被当成整串比较（安卓旧实现的缺陷）
        assert!(matches_campus_name("i-wxxy", "i-wxxy"));
        assert!(!matches_campus_name("iwxxy-2", "i-wxxy"));
    }

    #[test]
    fn portal_probe_host_extracts_private_ipv4_only() {
        assert_eq!(portal_probe_host("http://10.1.99.100"), "10.1.99.100");
        assert_eq!(portal_probe_host("http://10.1.99.100:801/eportal/"), "10.1.99.100");
        assert_eq!(portal_probe_host("10.1.99.100"), "10.1.99.100");
        // 域名/公网地址/空值不产生探测目标（公网 portal 会把任意外网卡误判成校园）
        assert_eq!(portal_probe_host("http://portal.wxu.edu.cn"), "");
        assert_eq!(portal_probe_host("http://8.8.8.8"), "");
        assert_eq!(portal_probe_host(""), "");
    }

    #[test]
    fn portal_origin_keeps_scheme_and_host_only() {
        assert_eq!(portal_origin("http://10.1.99.100"), "http://10.1.99.100");
        assert_eq!(portal_origin("http://10.1.99.100/"), "http://10.1.99.100");
        // 端口保留、路径丢弃（chkstatus 与 portal 同源，但端口是 origin 的一部分）
        assert_eq!(
            portal_origin("http://10.1.99.100:8090/drcom/login"),
            "http://10.1.99.100:8090"
        );
        assert_eq!(portal_origin("https://p.njupt.edu.cn/a79.htm"), "https://p.njupt.edu.cn");
        // 无 scheme 时按首个 / 截断
        assert_eq!(portal_origin("10.1.99.100/eportal/"), "10.1.99.100");
    }

    #[test]
    fn portal_origin_falls_back_to_default_on_empty() {
        // 空 origin 会让 chkstatus 请求落空，故回退默认 portal 地址
        assert_eq!(portal_origin(""), crate::config::model::default_portal_url());
    }

    #[test]
    fn expected_operator_display_defaults_to_school_line() {
        assert_eq!(expected_operator_display(""), "无锡学院");
        assert_eq!(expected_operator_display("@cmcc"), "@cmcc");
    }

    #[test]
    fn probe_with_retry_tolerates_single_loss() {
        // 首发不可达、次发可达 → 命中
        let mut n = 0;
        assert!(probe_with_retry("10.1.1.1", |_| {
            n += 1;
            n >= 2
        }));
        // 全部不可达 → 不命中
        assert!(!probe_with_retry("10.1.1.1", |_| false));
        // 空目标不发探测
        assert!(!probe_with_retry("", |_| panic!("空目标不应发起探测")));
    }

    #[test]
    fn campus_probe_targets_puts_portal_first_and_gateway_last() {
        assert_eq!(
            campus_probe_targets("10.1.1.1", "10.1.99.100"),
            vec![
                ("10.1.99.100", CampusLeg::Portal),
                ("10.1.1.1", CampusLeg::Gateway)
            ],
            "Portal 必须是第一腿、网关必须是最后兜底"
        );
        assert_eq!(
            campus_probe_targets("10.1.1.1", ""),
            vec![("10.1.1.1", CampusLeg::Gateway)]
        );
        assert_eq!(
            campus_probe_targets("", "10.1.99.100"),
            vec![("10.1.99.100", CampusLeg::Portal)]
        );
        assert!(campus_probe_targets("", "").is_empty());
    }

    #[test]
    fn judge_campus_prefers_portal_and_falls_back_to_gateway() {
        // 两腿都可达：Portal 先命中即短路，网关不再探测
        let probed = std::cell::RefCell::new(Vec::new());
        let leg = judge_campus_leg("10.1.1.1", "10.1.99.100", |target| {
            probed.borrow_mut().push(target.to_string());
            true
        });
        assert_eq!(leg, Some(CampusLeg::Portal));
        assert_eq!(*probed.borrow(), vec!["10.1.99.100".to_string()], "Portal 命中后不应再探网关");

        // Portal 不可达：网关作为最后兜底仍能命中
        assert_eq!(
            judge_campus_leg("10.1.1.1", "10.1.99.100", |target| target == "10.1.1.1"),
            Some(CampusLeg::Gateway)
        );
    }

    #[test]
    fn judge_campus_gateway_and_portal_each_hit() {
        assert!(judge_campus("10.1.1.1", "10.1.99.100", |t| t == "10.1.1.1"));
        assert!(judge_campus("10.1.1.1", "10.1.99.100", |t| t == "10.1.99.100"));
        assert!(!judge_campus("10.1.1.1", "10.1.99.100", |_| false));
    }

    #[test]
    fn judge_campus_skips_empty_target_legs() {
        // 网关未配置 → 只剩 Portal 腿，不向空目标发探测
        assert!(judge_campus("", "10.1.99.100", |t| t == "10.1.99.100"));
        // portal 主机不产生（公网/域名）→ 只剩网关腿
        assert!(judge_campus("10.1.1.1", "", |t| t == "10.1.1.1"));
        assert!(!judge_campus("", "", |_| panic!("两腿都空不应发起探测")));
    }

    #[test]
    fn judge_campus_health_portal_wins_and_gateway_only_means_auth_broken() {
        // Portal 可达 = 认证正常（Portal 是比网关更强的证据）
        assert_eq!(
            judge_campus_health("10.1.1.1", "10.1.99.100", |t| t == "10.1.99.100"),
            CampusHealth::Online
        );
        // 只有网关可达 = 进了校园网但认证不通
        assert_eq!(
            judge_campus_health("10.1.1.1", "10.1.99.100", |t| t == "10.1.1.1"),
            CampusHealth::AuthBroken
        );
        // 两腿都不通 = 不在校园网
        assert_eq!(
            judge_campus_health("10.1.1.1", "10.1.99.100", |_| false),
            CampusHealth::OffCampus
        );
        // 网关未配置时 Portal 不通只能是离网，不会误判成认证异常
        assert_eq!(
            judge_campus_health("", "10.1.99.100", |_| false),
            CampusHealth::OffCampus
        );
    }
}
