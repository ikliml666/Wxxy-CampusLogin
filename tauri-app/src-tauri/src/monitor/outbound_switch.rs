//! 夜间出站切换的桌面侧动作层：目标卡选择、逐卡校园网判定、快照序列化。
//! 判定规则与 campus_check 同源但按适配器粒度：/18 网段匹配 || 绑该卡源 IP 的
//! 网关可达（check_gateway_reachable_from；不绑源会多卡归因错位）。SSID 不参与
//! 逐卡判定——netsh wlan 只报当前连接的单个 SSID，多无线卡下归因不可靠。
//!
//! 网关可达探测经 `gateway_probe` 闭包注入：生产调用方传
//! `|gw, src| check_gateway_reachable_from(gw, Some(src))`（须运行在 spawn_blocking
//! 线程内，见 network/subnet.rs 的调用约束），单测传纯闭包避免真实触网。

use crate::network::discovery::Adapter;

/// 逐卡判定该适配器是否处于校园网内：
/// IP 与校园网关同 /18 网段，或该卡有网关且从该卡源 IP 可达校园网关（经 probe 注入）。
/// IP 为空直接判否；campus_gateway 为空时 is_same_subnet_18 解析失败返回 false、
/// probe 传空网关也判否，行为安全。
pub fn is_campus_adapter(
    ip: &str,
    gateway: &str,
    campus_gateway: &str,
    gateway_probe: impl Fn(&str, &str) -> bool,
) -> bool {
    if ip.is_empty() {
        return false;
    }
    if crate::network::is_same_subnet_18(ip, campus_gateway) {
        return true;
    }
    // 绑源探测做两次尝试：单发 ICMP 丢包会把在网校园卡误判成非校园——禁用步骤
    // 静默跳过、整夜只切一半。任一次可达即判校园，容忍瞬时抖动。
    !gateway.is_empty() && (gateway_probe(campus_gateway, ip) || gateway_probe(campus_gateway, ip))
}

/// 按优先级名序选出出站目标卡：跳过不在优先级列表、无 IP、处于校园网内的卡，
/// 返回第一张可用卡。`details` 为（适配器, 该卡网关）对——Adapter 无 gateway 字段，
/// 由调用方经 adapter_cache 的明细查询逐卡补齐。判定口径见 [`is_campus_adapter`]。
pub fn select_outbound_candidate(
    priority: &[String],
    details: &[(Adapter, String)],
    campus_gateway: &str,
    gateway_probe: impl Fn(&str, &str) -> bool,
) -> Option<Adapter> {
    for name in priority {
        let Some((adapter, gateway)) = details.iter().find(|(a, _)| a.name == *name) else {
            continue; // 该优先级名当前无对应卡（被禁用/拔出等），顺延下一优先级
        };
        // 无 IP 的卡不作出站目标；无网关的卡（APIPA/未完成 DHCP）出不了站，同样跳过
        if adapter.ip.is_empty() || gateway.is_empty() {
            continue;
        }
        // 校园网卡跳过，只切非校园出口；且候选自身网关必须可达——有 IP 有网关但
        // 网关失联（热点开着上游已断）的卡切过去也是死路
        if !is_campus_adapter(&adapter.ip, gateway, campus_gateway, &gateway_probe)
            && gateway_probe(gateway, &adapter.ip)
        {
            return Some(adapter.clone());
        }
    }
    None
}

/// 快照行：MetricRow 本身无 guid 字段，序列化时逐行补上所属卡 guid，
/// 供还原阶段对号入座。
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct SnapshotRow {
    guid: String,
    family: u16,
    automatic: bool,
    metric: u32,
}

/// 序列化选中卡的接口跃点快照为 JSON（`[{guid, family, automatic, metric}]`）。
/// 纯数据结构序列化不会失败，兜底返回 "[]"（合法空数组）防呆。
pub fn snapshot_json(guid: &str, rows: &[crate::platform::metric::MetricRow]) -> String {
    let snapshot: Vec<SnapshotRow> = rows
        .iter()
        .map(|r| SnapshotRow {
            guid: guid.to_string(),
            family: r.family,
            automatic: r.automatic,
            metric: r.metric,
        })
        .collect();
    serde_json::to_string(&snapshot).unwrap_or_else(|_| "[]".to_string())
}

/// 待禁用校园网卡行（切换快照 `[{guid, name}]`）。
/// 名单只记本功能亲手禁用的卡：还原与 adapter_watch 闸门都以它为准，绝不按
/// 「看到禁用状态」推断归属——USB 网卡状态判定不可靠（未连接会被误判为已禁用）。
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DisabledRow {
    pub guid: String,
    pub name: String,
}

/// 序列化待禁用校园网卡名单为 JSON。纯数据结构序列化不会失败，兜底返回 "[]"。
pub fn disabled_adapters_json(rows: &[DisabledRow]) -> String {
    serde_json::to_string(rows).unwrap_or_else(|_| "[]".to_string())
}

/// 解析禁用名单快照。空串 → 空名单（无禁用动作）；非法 JSON → Err——
/// 「解析失败」≠「没有禁用动作」：按空处理会在快照损坏时静默放走仍被
/// netsh 持久禁用的校园卡（还原流程会收尾清快照，此后无人认领），必须由
/// 调用方区分处理（告警并保留状态重试）。
pub fn parse_disabled_adapters(json: &str) -> Result<Vec<DisabledRow>, String> {
    if json.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(json.trim()).map_err(|e| format!("禁用名单快照损坏: {e}"))
}

/// 兜底默认路由快照（route add 的 runtime 路由，重启即清，落盘仅供程序内还原对账）。
/// 写完目标卡接口跃点=1 后，目标卡自身 DHCP 默认路由的有效跃点（路由 metric 0 +
/// 接口 metric 1 = 1）优于本路由（路由 metric 2 + 接口 metric 1 = 3），因此本路由
/// 平时不接管流量，仅在目标卡默认路由消失（DHCP 租约失效等）而链路仍在的窄场景
/// 作为 failover 兜底——这是保险而非主路径，主出站路径由接口跃点决定。
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StandbyRoute {
    pub dest: String,
    pub mask: String,
    pub gateway: String,
    pub metric: u32,
    #[serde(rename = "ifIndex")]
    pub if_index: u32,
}

/// 序列化兜底路由快照（`None` → 空串）。
pub fn standby_route_json(route: Option<&StandbyRoute>) -> String {
    match route {
        Some(r) => serde_json::to_string(r).unwrap_or_default(),
        None => String::new(),
    }
}

/// 解析兜底路由快照（空串/非法 JSON → `None`）。
pub fn parse_standby_route(json: &str) -> Option<StandbyRoute> {
    serde_json::from_str(json.trim()).ok()
}

/// 该卡是否不能参与夜间禁用：实例 ID 都读不到的卡（总线无法判定）保守跳过。
/// 注：USB 总线网卡不再排除——用户明确要求夜间禁用副适配器（本机副卡即 USB
/// 2.5G 网卡），运行期 netsh disable/enable 对称已实证（2026-10-01 夜切日志），
/// 7:30 还原/启动对账/看门狗/手动启用按钮（含 pnputil 设备级启用兜底）构成
/// 安全网；跨重启残留由启动对账收敛。
#[cfg(target_os = "windows")]
pub(crate) fn unsafe_to_disable(guid: &str) -> bool {
    crate::network::discovery::devnode::read_pnp_instance_id(guid).is_none()
}

/// 选出本次切换要临时禁用的校园网卡：在优先级列表内、已连接（有 IP）、判定为
/// 校园网、有 GUID、非目标卡、总线可判定（经 `bus_guard` 注入：生产传
/// [`unsafe_to_disable`]，单测传闭包），按优先级名序去重。
pub fn select_campus_to_disable(
    priority: &[String],
    details: &[(Adapter, String)],
    campus_gateway: &str,
    exclude_guid: &str,
    gateway_probe: impl Fn(&str, &str) -> bool,
    bus_guard: impl Fn(&str) -> bool,
) -> Vec<DisabledRow> {
    let mut rows: Vec<DisabledRow> = Vec::new();
    for name in priority {
        if rows.iter().any(|r| r.name == *name) {
            continue; // 同名卡只取第一张
        }
        let Some((adapter, gateway)) = details.iter().find(|(a, _)| a.name == *name) else {
            continue;
        };
        if adapter.guid == exclude_guid || adapter.guid.is_empty() {
            continue;
        }
        // 无 IP（已断开/被禁用）的卡不值得禁用，跳过
        if adapter.ip.is_empty() {
            continue;
        }
        if !is_campus_adapter(&adapter.ip, gateway, campus_gateway, &gateway_probe) {
            continue;
        }
        if bus_guard(&adapter.guid) {
            continue;
        }
        rows.push(DisabledRow { guid: adapter.guid.clone(), name: adapter.name.clone() });
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::discovery::AdapterStatus;
    use crate::platform::metric::MetricRow;

    /// 按 discovery::Adapter 实际字段构造测试卡（status 取已连接，其余填零值）
    fn adapter(name: &str, guid: &str, ip: &str, wireless: bool) -> Adapter {
        Adapter {
            name: name.to_string(),
            ip: ip.to_string(),
            wireless,
            guid: guid.to_string(),
            mac: String::new(),
            if_index: 0,
            status: AdapterStatus::Connected,
            link_speed: 0,
        }
    }

    /// panic 闭包：验证同段/无 IP/无网关分支短路后不会触达网关探测
    fn no_probe() -> impl Fn(&str, &str) -> bool {
        |gw, ip| panic!("不应触达网关探测: gw={gw} ip={ip}")
    }

    #[test]
    fn campus_adapter_by_subnet() {
        // 10.64.1.2 与 10.64.60.1 同 /18（10.64.0.0 - 10.64.63.255）→ 校园网卡
        assert!(is_campus_adapter("10.64.1.2", "10.64.60.1", "10.64.60.1", no_probe()));
    }

    #[test]
    fn campus_adapter_off_subnet_probe_reachable() {
        // 不同 /18，但校园网关从该卡源 IP 可达（probe 注入 true）→ 仍判校园
        assert!(is_campus_adapter("192.168.43.10", "192.168.43.1", "10.64.60.1", |_, _| true));
    }

    #[test]
    fn campus_adapter_off_subnet_probe_unreachable() {
        // 不同 /18 且 probe 判不可达 → 非校园
        assert!(!is_campus_adapter("192.168.43.10", "192.168.43.1", "10.64.60.1", |_, _| false));
    }

    #[test]
    fn campus_adapter_empty_ip_is_false() {
        assert!(!is_campus_adapter("", "", "10.64.60.1", no_probe()));
    }

    #[test]
    fn campus_adapter_empty_gateway_skips_probe() {
        // 不同段且该卡无网关 → 无法做绑源可达判定，直接判否（probe 不应触达）
        assert!(!is_campus_adapter("192.168.43.10", "", "10.64.60.1", no_probe()));
    }

    #[test]
    fn candidate_skips_campus_and_ipless_and_out_of_list() {
        let campus = (adapter("以太网", "{G1}", "10.64.1.2", false), "10.64.60.1".to_string());
        let hotspot = (adapter("WLAN", "{G2}", "192.168.43.10", true), "192.168.43.1".to_string());
        let offline = (adapter("以太网 2", "{G3}", "", false), String::new());
        let priority = vec!["以太网 2".to_string(), "WLAN".to_string(), "以太网".to_string()];
        let details = vec![campus, offline, hotspot];
        // 探测按网关区分：自身网关（192.168.43.1）可达、校园网关（10.64.60.1）不可达
        let probe = |gw: &str, _: &str| gw == "192.168.43.1";
        // 排序第一张无 IP → 跳过；第二张非校园网有 IP 且自身网关可达 → 选中
        let picked = select_outbound_candidate(&priority, &details, "10.64.60.1", probe).unwrap();
        assert_eq!(picked.name, "WLAN");
        // 列表外不参与：优先级只含校园网卡 → None
        let only_campus = vec![(adapter("以太网", "{G1}", "10.64.1.2", false), "10.64.60.1".to_string())];
        assert!(select_outbound_candidate(&["以太网".to_string()], &only_campus, "10.64.60.1", probe).is_none());
    }

    #[test]
    fn candidate_probe_reachable_means_campus_skipped() {
        // 唯一候选不同段但绑源可达校园网关 → 视为校园网卡 → 跳过 → None
        let only = (adapter("WLAN", "{G2}", "192.168.43.10", true), "192.168.43.1".to_string());
        assert!(select_outbound_candidate(&["WLAN".to_string()], &[only], "10.64.60.1", |_, _| true).is_none());
    }

    #[test]
    fn candidate_priority_name_absent_from_details_skips() {
        // 优先级名在当前适配器列表里查不到（被拔出/禁用）→ 顺延到下一项
        let hotspot = (adapter("WLAN", "{G2}", "192.168.43.10", true), "192.168.43.1".to_string());
        let priority = vec!["已拔出的卡".to_string(), "WLAN".to_string()];
        let picked = select_outbound_candidate(&priority, &[hotspot], "10.64.60.1", |gw: &str, _: &str| gw == "192.168.43.1")
            .unwrap();
        assert_eq!(picked.name, "WLAN");
    }

    #[test]
    fn candidate_empty_priority_returns_none() {
        // 未排序（空列表）：没有白名单内的候选，不切换
        let hotspot = (adapter("WLAN", "{G2}", "192.168.43.10", true), "192.168.43.1".to_string());
        assert!(select_outbound_candidate(&[], &[hotspot], "10.64.60.1", no_probe()).is_none());
    }

    #[test]
    fn candidate_skips_gatewayless_and_dead_gateway_cards() {
        // 无网关（APIPA/未完成 DHCP）的卡出不了站 → 跳过；
        // 自身网关不可达（热点开着上游已断）的卡切过去也是死路 → 跳过
        let apipa = (adapter("以太网 2", "{G3}", "169.254.10.20", false), String::new());
        let dead = (adapter("以太网 3", "{G4}", "192.168.7.10", false), "192.168.7.1".to_string());
        let hotspot = (adapter("WLAN", "{G2}", "192.168.43.10", true), "192.168.43.1".to_string());
        let priority = vec!["以太网 2".to_string(), "以太网 3".to_string(), "WLAN".to_string()];
        let picked = select_outbound_candidate(
            &priority,
            &[apipa, dead, hotspot],
            "10.64.60.1",
            |gw: &str, _: &str| gw == "192.168.43.1",
        )
        .unwrap();
        assert_eq!(picked.name, "WLAN", "无网关卡与网关失联卡都应顺延到健康候选");
    }

    #[test]
    fn campus_probe_single_loss_still_counts_as_campus() {
        // 绑源探测两发去抖：首发丢包、次发可达 → 仍判校园（降瞬时抖动误判）
        let tried = std::sync::atomic::AtomicBool::new(false);
        let flaky = move |_: &str, _: &str| {
            !tried.swap(true, std::sync::atomic::Ordering::SeqCst) // 首发 false
                || true // 次发 true
        };
        assert!(is_campus_adapter("192.168.43.10", "192.168.43.1", "10.64.60.1", flaky));
    }

    #[test]
    fn snapshot_json_round_trip_with_guid() {
        let rows = vec![
            MetricRow { family: 2, automatic: true, metric: 55 },
            MetricRow { family: 23, automatic: false, metric: 256 },
        ];
        let json = snapshot_json("{G1}", &rows);
        assert!(json.contains("\"guid\":\"{G1}\""), "每行应携带 guid 字段: {json}");
        // 反序列化 round-trip：字段值逐行一致
        let back: Vec<SnapshotRow> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[0], SnapshotRow { guid: "{G1}".to_string(), family: 2, automatic: true, metric: 55 });
        assert_eq!(back[1], SnapshotRow { guid: "{G1}".to_string(), family: 23, automatic: false, metric: 256 });
    }

    #[test]
    fn snapshot_json_empty_rows() {
        // GUID 无匹配行时 read_interface_metrics 返回空表，快照应为合法空数组
        assert_eq!(snapshot_json("{G1}", &[]), "[]");
    }

    #[test]
    fn disabled_adapters_json_round_trip() {
        let rows = vec![
            DisabledRow { guid: "{G1}".to_string(), name: "以太网".to_string() },
            DisabledRow { guid: "{G2}".to_string(), name: "以太网 2".to_string() },
        ];
        let json = disabled_adapters_json(&rows);
        assert_eq!(parse_disabled_adapters(&json).unwrap(), rows);
        // 空串 → 空名单（无禁用动作）；非法 JSON → Err（快照损坏，调用方须告警）
        assert!(parse_disabled_adapters("").unwrap().is_empty());
        assert!(parse_disabled_adapters("not json").is_err());
        assert_eq!(disabled_adapters_json(&[]), "[]");
    }

    #[test]
    fn standby_route_json_round_trip() {
        let route = StandbyRoute {
            dest: "0.0.0.0".to_string(),
            mask: "0.0.0.0".to_string(),
            gateway: "192.168.6.1".to_string(),
            metric: 2,
            if_index: 14,
        };
        let json = standby_route_json(Some(&route));
        assert!(json.contains("\"ifIndex\":14"), "ifIndex 序列化应保持驼峰: {json}");
        assert_eq!(parse_standby_route(&json), Some(route));
        // 序列化字段名与 helper 编码字段一一对应
        let back = parse_standby_route(&json).unwrap();
        assert_eq!(
            (back.dest.as_str(), back.mask.as_str(), back.gateway.as_str(), back.metric, back.if_index),
            ("0.0.0.0", "0.0.0.0", "192.168.6.1", 2u32, 14u32)
        );
        assert_eq!(standby_route_json(None), "");
        assert!(parse_standby_route("").is_none());
        assert!(parse_standby_route("bad").is_none());
    }

    #[test]
    fn select_campus_to_disable_filters_everything_non_campus() {
        // 场景：priority=[WLAN(目标), 以太网(校园), 以太网 2(校园,guard 命中),
        // 以太网 3(非校园)]；目标卡/非校园卡/无 IP 卡/guard 命中卡都不入名单
        let target = (adapter("WLAN", "{GT}", "192.168.43.10", true), "192.168.43.1".to_string());
        let campus = (adapter("以太网", "{G1}", "10.64.1.2", false), "10.64.60.1".to_string());
        let campus_guarded = (adapter("以太网 2", "{G2}", "10.64.1.3", false), "10.64.60.1".to_string());
        let off_campus = (adapter("以太网 3", "{G3}", "192.168.6.107", false), "192.168.6.1".to_string());
        let ipless = (adapter("以太网 4", "{G4}", "", false), String::new());
        let priority = vec![
            "WLAN".to_string(),
            "以太网".to_string(),
            "以太网 2".to_string(),
            "以太网 3".to_string(),
            "以太网 4".to_string(),
        ];
        let details = vec![target, campus, campus_guarded, off_campus, ipless];
        let rows = select_campus_to_disable(
            &priority,
            &details,
            "10.64.60.1",
            "{GT}",
            |_, _| false, // 非同段卡绑源探测不可达 → 非校园
            |g| g == "{G2}", // bus guard：{G2} 视为总线不可判定
        );
        assert_eq!(rows.len(), 1, "只应选中非目标的在网校园卡: {rows:?}");
        assert_eq!(rows[0], DisabledRow { guid: "{G1}".to_string(), name: "以太网".to_string() });
    }

    #[test]
    fn select_campus_to_disable_probe_reachable_counts_and_dedups() {
        // 不同 /18 但绑源可达校园网关 → 视为校园入选；重复名只取第一张
        let a1 = (adapter("以太网", "{G1}", "192.168.6.109", false), "192.168.6.1".to_string());
        let priority = vec!["以太网".to_string(), "以太网".to_string()];
        let rows = select_campus_to_disable(
            &priority,
            &[a1],
            "10.64.60.1",
            "{GT}",
            |_, _| true,
            |_| false,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].guid, "{G1}");
    }

    #[test]
    fn select_campus_to_disable_unknown_bus_is_skipped() {
        // 实例 ID 读不到的卡（总线无法判定）保守跳过：bus_guard 返回 true → 不入名单
        let campus = (adapter("以太网", "{G1}", "10.64.1.2", false), "10.64.60.1".to_string());
        let rows = select_campus_to_disable(
            &["以太网".to_string()],
            &[campus],
            "10.64.60.1",
            "{GT}",
            |_, _| false,
            |_| true,
        );
        assert!(rows.is_empty());
    }
}
