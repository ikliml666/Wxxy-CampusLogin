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
    !gateway.is_empty() && gateway_probe(campus_gateway, ip)
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
        // 无 IP 的卡不作出站目标；校园网卡跳过，只切非校园出口
        if !adapter.ip.is_empty() && !is_campus_adapter(&adapter.ip, gateway, campus_gateway, &gateway_probe) {
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
        // 排序第一张无 IP → 跳过；第二张非校园网有 IP → 选中
        let picked = select_outbound_candidate(&priority, &details, "10.64.60.1", |_, _| false).unwrap();
        assert_eq!(picked.name, "WLAN");
        // 列表外不参与：优先级只含校园网卡 → None
        let only_campus = vec![(adapter("以太网", "{G1}", "10.64.1.2", false), "10.64.60.1".to_string())];
        assert!(select_outbound_candidate(&["以太网".to_string()], &only_campus, "10.64.60.1", |_, _| false).is_none());
    }

    #[test]
    fn candidate_probe_reachable_means_campus_skipped() {
        // 唯一候选不同段但绑源可达校园网关 → 视为校园网卡 → 跳过 → None
        let only = (adapter("WLAN", "{G2}", "192.168.43.10", true), "192.168.43.1".to_string());
        assert!(select_outbound_candidate(&["WLAN".to_string()], &[only], "10.64.60.1", |_, _| true).is_none());
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
}
