//! 夜间出站切换的桌面侧动作层：目标卡选择、逐卡校园网判定、快照序列化。
//! 判定规则与 campus_check 同源但按适配器粒度：/18 网段匹配 || 绑该卡源 IP 的
//! 网关可达 || 绑该卡源 IP 的 portal 主机可达（check_gateway_reachable_from；
//! 不绑源会多卡归因错位）。SSID 不参与
//! 逐卡判定——netsh wlan 只报当前连接的单个 SSID，多无线卡下归因不可靠。
//!
//! 网关可达探测经 `gateway_probe` 闭包注入：生产调用方传
//! `|gw, src| check_gateway_reachable_from(gw, Some(src))`（须运行在 spawn_blocking
//! 线程内，见 network/subnet.rs 的调用约束），单测传纯闭包避免真实触网。

use crate::network::discovery::Adapter;

/// 逐卡判定该适配器是否处于校园网内：
/// IP 与校园网关同 /18 网段，或绑该卡源 IP 可达校园网关/portal 主机（经 probe 注入）。
/// IP 为空直接判否；campus_gateway 为空时 is_same_subnet_18 解析失败返回 false、
/// probe 传空网关也判否，行为安全。
pub fn is_campus_adapter(
    ip: &str,
    gateway: &str,
    campus_gateway: &str,
    portal_host: &str,
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
    // 校园网关之外再加 portal 主机（portal_probe_host 保证仅私网 IPv4 非空）：
    // 宿舍路由器 NAT 下的卡不在校园 /18、网关也常禁 ICMP（10.2.127.254 实测
    // 三源全丢包），但 portal 可达同样是「流量从校园侧出」的证据。
    (!gateway.is_empty()
        && (gateway_probe(campus_gateway, ip) || gateway_probe(campus_gateway, ip)))
        || (!portal_host.is_empty()
            && (gateway_probe(portal_host, ip) || gateway_probe(portal_host, ip)))
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
    if host.parse::<std::net::Ipv4Addr>().is_ok() && crate::auth::portal::is_nat_private_ip(host) {
        host.to_string()
    } else {
        String::new()
    }
}

/// 出站优先级的生效列表：用户已排序（非空）按原序透传；未排序（空）默认
/// 「无线卡优先、有线卡随后」，组内保持发现顺序——未拖动排序时夜间切换目标
/// 默认落在 WLAN/无线出口上，校园有线出口不会成为首选目标。该顺序同时是
/// [`select_campus_to_disable`] 的禁用白名单：未排序时在网校园卡同样纳入禁用，
/// 与已排序语义一致。
pub fn effective_outbound_priority(
    priority: &[String],
    details: &[(Adapter, String)],
) -> Vec<String> {
    if !priority.is_empty() {
        return priority.to_vec();
    }
    let mut order: Vec<String> = details
        .iter()
        .filter(|(a, _)| a.wireless)
        .map(|(a, _)| a.name.clone())
        .collect();
    order.extend(
        details
            .iter()
            .filter(|(a, _)| !a.wireless)
            .map(|(a, _)| a.name.clone()),
    );
    order
}

/// 按优先级名序选出出站目标卡：跳过不在优先级列表、无 IP、处于校园网内的卡，
/// 返回第一张可用卡。`details` 为（适配器, 该卡网关）对——Adapter 无 gateway 字段，
/// 由调用方经 adapter_cache 的明细查询逐卡补齐。判定口径见 [`is_campus_adapter`]。
pub fn select_outbound_candidate(
    priority: &[String],
    details: &[(Adapter, String)],
    campus_gateway: &str,
    portal_host: &str,
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
        if !is_campus_adapter(&adapter.ip, gateway, campus_gateway, portal_host, &gateway_probe)
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

/// 待切换状态标签：目标网卡尚未登录校园网，切换挂起等待登录确认。
pub const PENDING_TAG_WAIT_LOGIN: &str = "waiting-login";

/// 待切换状态行（登录闸挂起记录：目标网卡名 + 挂起原因标签 + 记录日）。
/// `day` = 记录当天的 `num_days_from_ce`，跨日即失效（切换窗口只属当晚，
/// 次日残留由排程驱动清理，防徽标常驻）。旧版本行缺 day → 反序列化为 0，
/// 视同已过期，同样走清理后重判。
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PendingSwitchRow {
    pub target: String,
    pub tag: String,
    #[serde(default)]
    pub day: i32,
}

/// 序列化待切换状态为 JSON。纯数据结构序列化不会失败，兜底返回空串（= 无挂起）。
pub fn pending_switch_json(target: &str, tag: &str, day: i32) -> String {
    serde_json::to_string(&PendingSwitchRow {
        target: target.to_string(),
        tag: tag.to_string(),
        day,
    })
    .unwrap_or_default()
}

/// 解析待切换状态（空串/非法 JSON → `None`）。挂起态只影响切换时序，解析失败
/// 视同无挂起即可——下一拍重判会重新写入覆盖，无需像禁用名单那样把损坏当错误。
pub fn parse_pending_switch(json: &str) -> Option<PendingSwitchRow> {
    if json.trim().is_empty() {
        return None;
    }
    serde_json::from_str(json.trim()).ok()
}

/// 登录闸匹配（纯函数）：目标卡是主/副监测卡时返回监测模块的在线结论
/// （`last_a1_online`/`last_a2_online`，即"等待监测模块返回已登录消息"），
/// 非监测卡返回 `None`，由调用方用监测同款原语绑源直查 Portal 兜底。
pub fn monitored_adapter_online(
    target: &str,
    a1_name: &str,
    a2_name: &str,
    a1_online: bool,
    a2_online: bool,
) -> Option<bool> {
    if !a1_name.is_empty() && target == a1_name {
        return Some(a1_online);
    }
    if !a2_name.is_empty() && target == a2_name {
        return Some(a2_online);
    }
    None
}

/// 历史兜底默认路由快照（v2.4.1 及更早版本切换时写入的 runtime 路由）。
/// 2026-10-18 事故后该路由不再创建：它与目标卡自己的 DHCP 默认路由共享
/// 「下一跳 + 接口」、只差跃点，而 Windows 判重与删除都不看跃点——既加不上（同键
/// 重复），删的时候又会误删用户自己的默认路由。本类型只保留"解析历史配置"能力，
/// 供还原步骤①按四元组精确清理残留。
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StandbyRoute {
    pub dest: String,
    pub mask: String,
    pub gateway: String,
    pub metric: u32,
    #[serde(rename = "ifIndex")]
    pub if_index: u32,
}

/// 解析兜底路由快照（空串/非法 JSON → `None`）。
pub fn parse_standby_route(json: &str) -> Option<StandbyRoute> {
    serde_json::from_str(json.trim()).ok()
}

/// 历史兜底路由的现存状态（还原时据此决定删不删）。
/// 2026-10-18 事故后不再创建该路由（同网关+接口无法与 DHCP 默认路由共存，见
/// `platform/route_table` 模块注释），本判定只用于清理历史残留与「绝不误删」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandbyCleanup {
    /// 表里没有这一行：从未创建成功（route add 恒报「已存在」）或已被系统移除
    AlreadyGone,
    /// 同网关+接口有默认路由，但跃点与本应用记录的不符 → 不是我们创建的，保留不动
    ForeignRowKept,
    /// 精确匹配存在，但它是当前唯一的默认路由 → 删除等于直接断网，保留
    LastDefaultPath,
    /// 精确匹配存在且删除后仍有其他默认路由 → 可安全删除
    Safe,
}

/// 兜底路由删除判定（纯函数，跨端可测）。
/// `same_key_metrics` = 同（网关, 接口）的默认路由跃点列表：DHCP 默认路由与我们
/// 记录的行共享网关+接口、只差跃点，因此必须按跃点列表判定"哪一行是我们的"——
/// route.exe 与 DeleteIpForwardEntry2 的匹配键都不含跃点，正是它们误删用户路由的根源。
/// `rows_outside_exact` = 除"我们那一行"以外的默认路由条数（删除后还剩几条）。
pub fn standby_cleanup_decision(
    same_key_metrics: &[u32],
    standby_metric: u32,
    rows_outside_exact: usize,
) -> StandbyCleanup {
    if !same_key_metrics.contains(&standby_metric) {
        return if same_key_metrics.is_empty() {
            StandbyCleanup::AlreadyGone
        } else {
            StandbyCleanup::ForeignRowKept
        };
    }
    if rows_outside_exact == 0 {
        StandbyCleanup::LastDefaultPath
    } else {
        StandbyCleanup::Safe
    }
}

/// 读实时路由表判定历史兜底路由状态（Windows）。
#[cfg(target_os = "windows")]
pub fn standby_route_cleanup(route: &StandbyRoute) -> Result<StandbyCleanup, String> {
    let gateway: std::net::Ipv4Addr = route
        .gateway
        .trim()
        .parse()
        .map_err(|_| format!("兜底路由快照网关非法: {:?}", route.gateway))?;
    // 目的前缀也参与身份判定（与 `route_table::delete_route_exact` 同一口径）：
    // 历史快照恒为 0.0.0.0/0，但快照若被改坏，按 0.0.0.0/0 去删就会落到别人的默认
    // 路由上——宁可报错让还原保留状态重试（有放弃阀兜底）。
    let dest: std::net::Ipv4Addr = route
        .dest
        .trim()
        .parse()
        .map_err(|_| format!("兜底路由快照目的地址非法: {:?}", route.dest))?;
    let mask: std::net::Ipv4Addr = route
        .mask
        .trim()
        .parse()
        .map_err(|_| format!("兜底路由快照掩码非法: {:?}", route.mask))?;
    let prefix_len = crate::platform::route_table::mask_to_prefix_len(mask)?;
    let rows = crate::platform::route_table::ipv4_forward_rows()?;
    let mut same_key_metrics: Vec<u32> = Vec::new();
    let mut rows_outside_exact = 0usize;
    for row in rows
        .iter()
        .filter(|r| r.prefix_len == prefix_len && r.dest == dest)
    {
        let same_key = row.next_hop == gateway && row.if_index == route.if_index;
        if same_key {
            same_key_metrics.push(row.metric);
        }
        if !(same_key && row.metric == route.metric) {
            rows_outside_exact += 1;
        }
    }
    Ok(standby_cleanup_decision(
        &same_key_metrics,
        route.metric,
        rows_outside_exact,
    ))
}

/// 非 Windows 平台没有该机制（切换态快照只由 Windows 桌面写入），按"已不存在"处理。
#[cfg(not(target_os = "windows"))]
pub fn standby_route_cleanup(_route: &StandbyRoute) -> Result<StandbyCleanup, String> {
    Ok(StandbyCleanup::AlreadyGone)
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
    portal_host: &str,
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
        if !is_campus_adapter(&adapter.ip, gateway, campus_gateway, portal_host, &gateway_probe) {
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
        assert!(is_campus_adapter("10.64.1.2", "10.64.60.1", "10.64.60.1", "", no_probe()));
    }

    #[test]
    fn campus_adapter_off_subnet_probe_reachable() {
        // 不同 /18，但校园网关从该卡源 IP 可达（probe 注入 true）→ 仍判校园
        assert!(is_campus_adapter("192.168.43.10", "192.168.43.1", "10.64.60.1", "", |_, _| true));
    }

    #[test]
    fn campus_adapter_off_subnet_probe_unreachable() {
        // 不同 /18 且 probe 判不可达 → 非校园
        assert!(!is_campus_adapter("192.168.43.10", "192.168.43.1", "10.64.60.1", "", |_, _| false));
    }

    #[test]
    fn campus_adapter_empty_ip_is_false() {
        assert!(!is_campus_adapter("", "", "10.64.60.1", "", no_probe()));
    }

    #[test]
    fn campus_adapter_empty_gateway_skips_probe() {
        // 不同段且该卡无网关 → 无法做绑源可达判定，直接判否（probe 不应触达）
        assert!(!is_campus_adapter("192.168.43.10", "", "10.64.60.1", "", no_probe()));
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
        let picked = select_outbound_candidate(&priority, &details, "10.64.60.1", "", probe).unwrap();
        assert_eq!(picked.name, "WLAN");
        // 列表外不参与：优先级只含校园网卡 → None
        let only_campus = vec![(adapter("以太网", "{G1}", "10.64.1.2", false), "10.64.60.1".to_string())];
        assert!(select_outbound_candidate(&["以太网".to_string()], &only_campus, "10.64.60.1", "", probe).is_none());
    }

    #[test]
    fn candidate_probe_reachable_means_campus_skipped() {
        // 唯一候选不同段但绑源可达校园网关 → 视为校园网卡 → 跳过 → None
        let only = (adapter("WLAN", "{G2}", "192.168.43.10", true), "192.168.43.1".to_string());
        assert!(select_outbound_candidate(&["WLAN".to_string()], &[only], "10.64.60.1", "", |_, _| true).is_none());
    }

    #[test]
    fn candidate_priority_name_absent_from_details_skips() {
        // 优先级名在当前适配器列表里查不到（被拔出/禁用）→ 顺延到下一项
        let hotspot = (adapter("WLAN", "{G2}", "192.168.43.10", true), "192.168.43.1".to_string());
        let priority = vec!["已拔出的卡".to_string(), "WLAN".to_string()];
        let picked = select_outbound_candidate(&priority, &[hotspot], "10.64.60.1", "", |gw: &str, _: &str| gw == "192.168.43.1")
            .unwrap();
        assert_eq!(picked.name, "WLAN");
    }

    #[test]
    fn candidate_empty_priority_returns_none() {
        // 未排序（空列表）：没有白名单内的候选，不切换
        // （调用方 scheduled.rs 会先经 effective_outbound_priority 展开默认序再传入）
        let hotspot = (adapter("WLAN", "{G2}", "192.168.43.10", true), "192.168.43.1".to_string());
        assert!(select_outbound_candidate(&[], &[hotspot], "10.64.60.1", "", no_probe()).is_none());
    }

    #[test]
    fn effective_priority_non_empty_passthrough() {
        // 用户已排序：原序透传，不做无线优先重排
        let details = vec![
            (adapter("WLAN", "{G2}", "192.168.43.10", true), "192.168.43.1".to_string()),
            (adapter("以太网", "{G1}", "10.64.1.2", false), "10.64.60.1".to_string()),
        ];
        let priority = vec!["以太网".to_string(), "WLAN".to_string()];
        assert_eq!(effective_outbound_priority(&priority, &details), priority);
    }

    #[test]
    fn effective_priority_empty_defaults_wireless_first() {
        // 未排序：无线卡优先（组内保发现序），有线卡随后（保发现序）
        let details = vec![
            (adapter("以太网", "{G1}", "10.64.1.2", false), "10.64.60.1".to_string()),
            (adapter("以太网 2", "{G3}", "192.168.6.109", false), "192.168.6.1".to_string()),
            (adapter("WLAN", "{G2}", "192.168.6.115", true), "192.168.6.1".to_string()),
        ];
        assert_eq!(
            effective_outbound_priority(&[], &details),
            vec!["WLAN".to_string(), "以太网".to_string(), "以太网 2".to_string()]
        );
    }

    #[test]
    fn effective_priority_empty_multiple_wireless_keep_discovery_order() {
        // 多张无线卡同样按发现顺序排列在有线卡之前
        let details = vec![
            (adapter("以太网", "{G1}", "10.64.1.2", false), "10.64.60.1".to_string()),
            (adapter("WLAN 2", "{G4}", "192.168.7.10", true), "192.168.7.1".to_string()),
            (adapter("WLAN", "{G2}", "192.168.6.115", true), "192.168.6.1".to_string()),
        ];
        assert_eq!(
            effective_outbound_priority(&[], &details),
            vec!["WLAN 2".to_string(), "WLAN".to_string(), "以太网".to_string()]
        );
    }

    #[test]
    fn effective_priority_empty_no_adapters_is_empty() {
        // 没有适配器：默认序为空，select 侧自然得到 None（后续走无候选告警）
        assert!(effective_outbound_priority(&[], &[]).is_empty());
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
            "",
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
        assert!(is_campus_adapter("192.168.43.10", "192.168.43.1", "10.64.60.1", "", flaky));
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
    fn pending_switch_json_round_trip() {
        let json = pending_switch_json("WLAN", PENDING_TAG_WAIT_LOGIN, 739_000);
        assert_eq!(
            parse_pending_switch(&json),
            Some(PendingSwitchRow {
                target: "WLAN".to_string(),
                tag: "waiting-login".to_string(),
                day: 739_000,
            })
        );
        // 空串/纯空白 → 无挂起；非法 JSON → 同样视同无挂起（下拍重写覆盖）
        assert_eq!(parse_pending_switch(""), None);
        assert_eq!(parse_pending_switch("   "), None);
        assert_eq!(parse_pending_switch("not json"), None);
        // 旧版本行（无 day 字段）→ day 反序列化为 0，补驱动按跨日失效清理后重判
        let legacy = r#"{"target":"WLAN","tag":"waiting-login"}"#;
        assert_eq!(
            parse_pending_switch(legacy),
            Some(PendingSwitchRow {
                target: "WLAN".to_string(),
                tag: "waiting-login".to_string(),
                day: 0,
            })
        );
    }

    #[test]
    fn monitored_adapter_online_matches_by_name() {
        use monitored_adapter_online as f;
        // 主卡：直接返回监测结论，无论真假
        assert_eq!(f("WLAN", "WLAN", "eth1", true, false), Some(true));
        assert_eq!(f("WLAN", "WLAN", "eth1", false, true), Some(false));
        // 副卡：返回副卡监测结论
        assert_eq!(f("eth1", "WLAN", "eth1", false, true), Some(true));
        // 非监测卡 → None（调用方绑源直查 Portal 兜底）
        assert_eq!(f("eth2", "WLAN", "eth1", true, true), None);
        // 名字为空不参与匹配（适配器名缺失时不能把任意目标当成监测卡）
        assert_eq!(f("", "", "", true, true), None);
        assert_eq!(f("WLAN", "", "", true, true), None);
    }

    #[test]
    fn standby_route_snapshot_parses_legacy_config_json() {
        // 历史配置的实际形态（2026-10-18 事故机器上的 outboundStandbyRoute 原文）：
        // 旧版本只读这个字段，本版本仍要能解析出四元组供精确清理
        let json = r#"{"dest":"0.0.0.0","mask":"0.0.0.0","gateway":"10.196.145.27","metric":2,"ifIndex":12}"#;
        let route = parse_standby_route(json).expect("历史快照应可解析");
        assert_eq!(
            (
                route.dest.as_str(),
                route.mask.as_str(),
                route.gateway.as_str(),
                route.metric,
                route.if_index,
            ),
            ("0.0.0.0", "0.0.0.0", "10.196.145.27", 2u32, 12u32)
        );
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
            "",
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
            "",
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
            "",
            "{GT}",
            |_, _| false,
            |_| true,
        );
        assert!(rows.is_empty());
    }

    #[test]
    fn campus_adapter_portal_probe_reachable() {
        // 宿舍路由器 NAT 场景：不同 /18、绑源网关不可达，但绑源 portal 可达 → 校园
        // （2026-10-03 实测：10.2.127.254 三源全丢包，portal 10.1.99.100 绑源可达）
        let probe = |target: &str, _: &str| target == "10.1.99.100";
        assert!(is_campus_adapter(
            "192.168.6.109",
            "192.168.6.1",
            "10.2.127.254",
            "10.1.99.100",
            probe
        ));
    }

    #[test]
    fn campus_adapter_portal_unreachable_stays_non_campus() {
        // 两目标都不可达 → 非校园：portal 只是新增证据，不放宽判定
        let probe = |_: &str, _: &str| false;
        assert!(!is_campus_adapter(
            "192.168.6.109",
            "192.168.6.1",
            "10.2.127.254",
            "10.1.99.100",
            probe
        ));
    }

    #[test]
    fn campus_adapter_portal_probe_single_loss_still_counts() {
        // portal 目标同样享受两发去抖：网关两发都不可达、portal 首发丢包次发可达
        let tried = std::sync::atomic::AtomicBool::new(false);
        let flaky = move |target: &str, _: &str| {
            if target != "10.1.99.100" {
                return false; // 网关两发都不可达
            }
            !tried.swap(true, std::sync::atomic::Ordering::SeqCst) || true
        };
        assert!(is_campus_adapter(
            "192.168.6.109",
            "192.168.6.1",
            "10.2.127.254",
            "10.1.99.100",
            flaky
        ));
    }

    #[test]
    fn campus_adapter_empty_gateway_portal_reachable() {
        // 无网关卡（NAT 下未下发网关等）portal 可达仍判校园
        assert!(is_campus_adapter("192.168.6.109", "", "10.2.127.254", "10.1.99.100", |_, _| true));
    }

    #[test]
    fn portal_probe_host_extracts_private_ipv4() {
        assert_eq!(portal_probe_host("http://10.1.99.100"), "10.1.99.100");
        assert_eq!(portal_probe_host("http://10.1.99.100:801/eportal/"), "10.1.99.100");
        assert_eq!(portal_probe_host("10.1.99.100"), "10.1.99.100");
        // 域名/公网地址/空值不产生探测目标（公网 portal 会把任意外网卡误判成校园）
        assert_eq!(portal_probe_host("http://portal.wxu.edu.cn"), "");
        assert_eq!(portal_probe_host("http://8.8.8.8"), "");
        assert_eq!(portal_probe_host(""), "");
    }

    #[test]
    fn standby_cleanup_never_deletes_foreign_or_last_default_route() {
        use super::{standby_cleanup_decision, StandbyCleanup};
        // 表里没有我们那行（本次事故的现场：route add 从未成功过）→ 不删
        assert_eq!(standby_cleanup_decision(&[], 2, 1), StandbyCleanup::AlreadyGone);
        // 同网关+接口只有 DHCP 默认路由（跃点 60）→ 不是我们的，不删
        assert_eq!(
            standby_cleanup_decision(&[60], 2, 1),
            StandbyCleanup::ForeignRowKept
        );
        // 只剩我们那一行 → 删除即断网（2026-10-18 用户 WLAN 被断的直接成因）
        assert_eq!(
            standby_cleanup_decision(&[2], 2, 0),
            StandbyCleanup::LastDefaultPath
        );
        // 我们的行 + 目标卡自己的默认路由 → 删我们那行安全
        assert_eq!(standby_cleanup_decision(&[2, 60], 2, 1), StandbyCleanup::Safe);
    }
}
