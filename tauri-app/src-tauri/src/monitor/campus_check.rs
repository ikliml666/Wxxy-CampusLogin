use std::collections::HashMap;
use serde::Serialize;
use crate::network::Adapter;
use super::outbound_switch::portal_probe_host;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionCampusStatus {
    pub on_campus: bool,
    pub name: Option<String>,
    pub message: String,
}

/// 逐卡判定结果：每张适配器独立走「名称 → 网段 → 网关 → Portal」成本顺序短路，
/// 各自携带 `on_campus` 与 `message`。
///
/// 2026-10-26 改造背景：此前 `wifi`/`wired` 是**整类聚合**状态，只要该类里有一张卡
/// 有 IP 且网关可达，成功文案就被套到该类**每一张卡**上（含无 IP 的卡），
/// 用户机上表现为「以太网 2 无IP地址 / 有线通过网关10.2.127.254连接校园网」的错乱归因；
/// 同时网关/Portal 探测走 `check_gateway_reachable`（不绑源），多网卡下探测从哪张卡
/// 出去不受控，同一进程内结论会自相矛盾（日志实测 09:03:42 判非校园、09:03:43 判校园）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterCampusStatus {
    pub name: String,
    pub wireless: bool,
    pub on_campus: bool,
    /// 命中的判定腿：`name` / `subnet` / `gateway` / `portal`；全部失败为 None
    pub matched_by: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CampusCheckResult {
    pub wifi: Option<ConnectionCampusStatus>,
    pub wired: Option<ConnectionCampusStatus>,
    pub on_campus: bool,
    pub current_ssid: Option<String>,
    pub message: String,
    /// 逐卡判定明细（UI 按适配器名取用，替代按 wifi/wired 整类取用）
    pub adapters: Vec<AdapterCampusStatus>,
}

pub(super) fn adapter_campus_status<'a>(adapter_name: &str, adapters: &'a [Adapter], campus_result: &'a CampusCheckResult) -> Option<&'a AdapterCampusStatus> {
    // 以适配器列表为权威：卡已不存在（被禁用/拔线）时不返回任何判定，避免残留结论
    crate::network::find_by_name(adapters, adapter_name)?;
    campus_result.adapters.iter().find(|s| s.name == adapter_name)
}

pub(super) fn adapter_campus_message(adapter_name: &str, adapters: &[Adapter], campus_result: &CampusCheckResult) -> Option<String> {
    adapter_campus_status(adapter_name, adapters, campus_result).map(|s| s.message.clone())
}

/// 校园网名称名单解析：配置值支持顿号/全半角逗号/分号/空白分隔多个名称
/// （默认 "i-wxxy、iwxxy-2、iwxxy-3"），空项忽略
fn campus_name_list(value: &str) -> Vec<&str> {
    value
        .split(['、', '，', ',', '；', ';', ' ', '\t'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// 判定上下文：一次检测内共享的只读输入（名称名单、网关、Portal 主机、全局名称信号）。
struct CampusContext<'a> {
    /// 名称检查开关；关闭时「名称」腿整体不参与判定
    enable_name_check: bool,
    name_list: Vec<&'a str>,
    campus_gw: &'a str,
    portal_host: String,
    /// 无线侧全局名称信号（netsh wlan 只报当前连接的一个 SSID）
    wifi_ssid: Option<String>,
    /// 有线侧全局名称信号（netsh lan，dot3svc 未运行时为 None）
    wired_profile: Option<String>,
}

impl CampusContext<'_> {
    /// 名称腿（成本最低，netsh 结果有缓存）：按卡类型套用——
    /// SSID 只对无线卡生效、有线 profile 只对有线卡生效，其余卡继续往下走。
    fn name_hit(&self, adapter: &Adapter) -> Option<String> {
        if !self.enable_name_check {
            return None;
        }
        if adapter.wireless {
            let ssid = self.wifi_ssid.as_ref()?;
            self.name_list
                .iter()
                .any(|n| ssid.eq_ignore_ascii_case(n))
                .then(|| format!("已连接到校园WiFi({ssid})"))
        } else {
            let profile = self.wired_profile.as_ref()?;
            self.name_list
                .iter()
                .any(|n| profile.eq_ignore_ascii_case(n))
                .then(|| format!("已连接到校园有线网络({profile})"))
        }
    }
}

/// 逐卡判定：按探测成本从低到高短路，任一腿命中即该卡在校园网，
/// 网关与 Portal 都失败才判该卡非校园网。
///
/// `probe(target, source)` 负责绑 `source`（该卡 IP）探 `target` 的可达性，由调用方
/// 注入以便单测替换为桩函数，并共享 (target, source) 维度的结果缓存。
fn judge_adapter<F>(adapter: &Adapter, ctx: &CampusContext, probe: &mut F) -> AdapterCampusStatus
where
    F: FnMut(&str, &str) -> bool,
{
    let build = |on_campus: bool, matched_by: Option<&str>, message: String| AdapterCampusStatus {
        name: adapter.name.clone(),
        wireless: adapter.wireless,
        on_campus,
        matched_by: matched_by.map(str::to_string),
        message,
    };

    // 0. 无 IP 的卡：任何探测都无法绑定它的源地址，且它本就无法通信。
    //    显式判非校园，杜绝此前「蹭」同类型其他卡的判定结论（截图错乱归因的直接来源）。
    if adapter.ip.is_empty() {
        return build(false, None, "无IP地址，无法判定".to_string());
    }

    // 1. 名称（成本最低：netsh 结果已缓存，无网络 IO）
    if let Some(msg) = ctx.name_hit(adapter) {
        return build(true, Some("name"), msg);
    }

    // 2. 网段（纯位运算，零 IO）
    if crate::network::is_same_subnet_18(&adapter.ip, ctx.campus_gw) {
        return build(
            true,
            Some("subnet"),
            format!("{}与网关{}在同一/18网段", adapter.ip, ctx.campus_gw),
        );
    }

    // 3. 网关（绑该卡源的一次 ICMP，超时 2s）
    if probe(ctx.campus_gw, &adapter.ip) {
        return build(
            true,
            Some("gateway"),
            format!("通过网关{}连接校园网", ctx.campus_gw),
        );
    }

    // 4. Portal（绑该卡源；portal_url 无私网 IPv4 主机时该腿恒为否）
    if !ctx.portal_host.is_empty() && probe(&ctx.portal_host, &adapter.ip) {
        return build(
            true,
            Some("portal"),
            format!("通过 portal {} 连接校园网", ctx.portal_host),
        );
    }

    // 名称/网段/网关/Portal 全部未命中
    build(
        false,
        None,
        format!("{}未连接校园网(名称/网段/网关/Portal 均未命中)", adapter.ip),
    )
}

/// 按卡类型聚合出 `wifi`/`wired` 兼容状态（IPC 旧字段，前端仍读）。
/// 该类无卡时为 None；有卡时 `on_campus` 取该类任一卡命中，`message` 优先取命中卡文案。
fn aggregate_class(cards: &[&AdapterCampusStatus], name: Option<String>) -> Option<ConnectionCampusStatus> {
    if cards.is_empty() {
        return None;
    }
    let on_campus = cards.iter().any(|s| s.on_campus);
    let message = join_messages(cards);
    Some(ConnectionCampusStatus { on_campus, name, message })
}

/// 命中卡文案优先；全未命中时退化为各卡文案（保持旧字段的可读性）。
fn join_messages(cards: &[&AdapterCampusStatus]) -> String {
    let hits: Vec<&str> = cards
        .iter()
        .filter(|s| s.on_campus)
        .map(|s| s.message.as_str())
        .collect();
    if !hits.is_empty() {
        return hits.join("；");
    }
    cards.iter().map(|s| s.message.as_str()).collect::<Vec<_>>().join("；")
}

pub fn check_campus_network(config: &crate::config::model::Config, adapters: &[crate::network::Adapter]) -> CampusCheckResult {
    crate::log_debug!("campus", "[校园网检测] enable_network_name_check={}, required_network_name='{}', campus_gateway='{}', 适配器数={}",
        config.enable_network_name_check, config.required_network_name, config.campus_gateway, adapters.len());

    let name_list = campus_name_list(&config.required_network_name);
    let portal_host = portal_probe_host(&config.portal_url);
    // 名称检查禁用时名称腿不参与判定，也就不必付 netsh 的调用成本
    let (wifi_ssid, wired_profile) = if config.enable_network_name_check {
        (
            crate::network::get_wireless_ssid().ok().flatten(),
            crate::network::get_wired_network_profile().ok().flatten(),
        )
    } else {
        (None, None)
    };

    crate::log_debug!("campus", "[校园网检测] wifi_ssid={:?}, wired_profile={:?}, portal_host={:?}", wifi_ssid, wired_profile, portal_host);

    let ctx = CampusContext {
        enable_name_check: config.enable_network_name_check,
        name_list,
        campus_gw: &config.campus_gateway,
        portal_host,
        wifi_ssid: wifi_ssid.clone(),
        wired_profile: wired_profile.clone(),
    };

    // (target, source) → 可达性，避免同一张卡对同一目标重复探测
    let mut cache: HashMap<(String, String), bool> = HashMap::new();
    let mut probe = |target: &str, source: &str| -> bool {
        if target.is_empty() || source.is_empty() {
            return false;
        }
        let key = (target.to_string(), source.to_string());
        if let Some(&hit) = cache.get(&key) {
            crate::log_debug!("campus", "[校园网检测] 复用探测缓存: {} → {} = {}", source, target, hit);
            return hit;
        }
        let ok = crate::network::check_gateway_reachable_from(target, Some(source));
        cache.insert(key, ok);
        if ok {
            crate::log_debug!("campus", "[校园网检测] 绑源探测可达: {} → {}", source, target);
        } else {
            crate::log_warn!("campus", "[校园网检测] 绑源探测不可达: {} → {}", source, target);
        }
        ok
    };

    let per_adapter: Vec<AdapterCampusStatus> = adapters
        .iter()
        .map(|a| judge_adapter(a, &ctx, &mut probe))
        .collect();

    let wireless_cards: Vec<&AdapterCampusStatus> = per_adapter.iter().filter(|s| s.wireless).collect();
    let wired_cards: Vec<&AdapterCampusStatus> = per_adapter.iter().filter(|s| !s.wireless).collect();

    // 兼容字段：wifi 的 name 沿用 SSID、wired 的 name 沿用有线 profile
    let wifi_status = aggregate_class(&wireless_cards, wifi_ssid.clone());
    let wired_status = aggregate_class(&wired_cards, wired_profile.clone());

    let on_campus = per_adapter.iter().any(|s| s.on_campus);
    let message = if on_campus {
        let hits: Vec<&str> = per_adapter
            .iter()
            .filter(|s| s.on_campus)
            .map(|s| s.message.as_str())
            .collect();
        if hits.is_empty() { "已连接校园网".to_string() } else { hits.join("；") }
    } else {
        let misses: Vec<&str> = per_adapter.iter().map(|s| s.message.as_str()).collect();
        if misses.is_empty() { "未连接到校园网络".to_string() } else { misses.join("；") }
    };

    if on_campus {
        crate::log_debug!("campus", "[校园网检测] 结果: on_campus=true, 逐卡={:?}, message={}",
            per_adapter.iter().map(|s| (&s.name, s.on_campus, s.matched_by.as_deref())).collect::<Vec<_>>(), message);
    } else {
        // 判定异常（未在校园网）：保留 warn 级别便于排障
        crate::log_warn!("campus", "[校园网检测] 判定未在校园网: 逐卡={:?}, message={}",
            per_adapter.iter().map(|s| (&s.name, s.on_campus, s.matched_by.as_deref())).collect::<Vec<_>>(), message);
    }

    CampusCheckResult {
        wifi: wifi_status,
        wired: wired_status,
        on_campus,
        current_ssid: wifi_ssid.or(wired_profile),
        message,
        adapters: per_adapter,
    }
}

/// 校园网检测静默期判定——实现已下沉到双端共享面
/// [`crate::config::schedule::is_campus_check_silent`]（2026-10-26：此前桌面巡检、
/// 安卓巡检、安卓开机自启、无头 core 各写一份同语义边界判定，共享 crate 无此函数，
/// 违反「新增跨平台判定优先放共享面」的约定）。此处仅保留 crate 内转出，
/// 调用方可继续按 monitor 模块路径引用。
pub use crate::config::schedule::is_campus_check_silent;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::discovery::AdapterStatus;

    fn adapter(name: &str, ip: &str, wireless: bool) -> Adapter {
        Adapter {
            name: name.to_string(),
            ip: ip.to_string(),
            wireless,
            guid: String::new(),
            mac: String::new(),
            if_index: 0,
            status: AdapterStatus::Connected,
            link_speed: 0,
        }
    }

    fn ctx<'a>(name_list: Vec<&'a str>, gw: &'a str, portal: &str) -> CampusContext<'a> {
        CampusContext {
            enable_name_check: true,
            name_list,
            campus_gw: gw,
            portal_host: portal.to_string(),
            wifi_ssid: None,
            wired_profile: None,
        }
    }

    #[test]
    fn 名称名单_解析多分隔符与空项() {
        assert_eq!(
            campus_name_list("i-wxxy、iwxxy-2、iwxxy-3"),
            vec!["i-wxxy", "iwxxy-2", "iwxxy-3"]
        );
        assert_eq!(campus_name_list("a, b；c\td"), vec!["a", "b", "c", "d"]);
        assert_eq!(campus_name_list("  、，; "), Vec::<&str>::new());
        assert_eq!(campus_name_list("i-wxxy"), vec!["i-wxxy"]);
    }

    #[test]
    fn 无IP的卡_直接判非校园且不探测() {
        let c = ctx(vec!["i-wxxy"], "10.2.127.254", "");
        let mut probes = 0;
        let mut probe = |_t: &str, _s: &str| {
            probes += 1;
            true
        };
        let status = judge_adapter(&adapter("以太网 2", "", false), &c, &mut probe);
        assert!(!status.on_campus);
        assert_eq!(status.matched_by, None);
        assert_eq!(probes, 0, "无 IP 的卡不应触发任何探测");
    }

    #[test]
    fn 名称命中_按卡类型套用且短路不探测() {
        let mut c = ctx(vec!["i-wxxy"], "10.2.127.254", "10.1.99.100");
        c.wifi_ssid = Some("i-wxxy".to_string());
        let mut probes = 0;
        let mut probe = |_t: &str, _s: &str| {
            probes += 1;
            false
        };
        let wifi = judge_adapter(&adapter("WLAN", "10.2.1.5", true), &c, &mut probe);
        assert!(wifi.on_campus);
        assert_eq!(wifi.matched_by.as_deref(), Some("name"));
        // SSID 是无线信号，不套用到有线卡
        let wired = judge_adapter(&adapter("以太网", "10.2.1.6", false), &c, &mut probe);
        assert!(!wired.on_campus);
        assert_eq!(wired.matched_by, None);
        assert!(probes >= 1, "有线卡应继续往下走网关/Portal 探测");
    }

    #[test]
    fn 网段命中_零探测短路() {
        let c = ctx(vec![], "10.2.127.254", "");
        let mut probes = 0;
        let mut probe = |_t: &str, _s: &str| {
            probes += 1;
            false
        };
        // 10.2.39.163 与 10.2.127.254 不同 /18；10.2.100.5 与 10.2.127.254 同属 10.2.64.0/18
        let hit = judge_adapter(&adapter("以太网", "10.2.100.5", false), &c, &mut probe);
        assert!(hit.on_campus);
        assert_eq!(hit.matched_by.as_deref(), Some("subnet"));
        assert_eq!(probes, 0);
    }

    #[test]
    fn 路由器私网_网关腿绑源命中() {
        let c = ctx(vec!["i-wxxy"], "10.2.127.254", "");
        let mut seen: Vec<(String, String)> = Vec::new();
        let mut probe = |t: &str, s: &str| {
            seen.push((t.to_string(), s.to_string()));
            true
        };
        let status = judge_adapter(&adapter("以太网 2", "192.168.6.114", false), &c, &mut probe);
        assert!(status.on_campus);
        assert_eq!(status.matched_by.as_deref(), Some("gateway"));
        assert_eq!(seen, vec![("10.2.127.254".to_string(), "192.168.6.114".to_string())],
            "探测必须绑定该卡自己的源 IP");
    }

    #[test]
    fn 网关失败但Portal命中_判校园() {
        let c = ctx(vec![], "10.2.127.254", "10.1.99.100");
        let mut probe = |t: &str, _s: &str| t == "10.1.99.100";
        let status = judge_adapter(&adapter("以太网", "192.168.6.114", false), &c, &mut probe);
        assert!(status.on_campus);
        assert_eq!(status.matched_by.as_deref(), Some("portal"));
    }

    #[test]
    fn 网关与Portal都失败_判非校园() {
        let c = ctx(vec![], "10.2.127.254", "10.1.99.100");
        let mut probe = |_t: &str, _s: &str| false;
        let status = judge_adapter(&adapter("以太网", "192.168.6.114", false), &c, &mut probe);
        assert!(!status.on_campus);
        assert_eq!(status.matched_by, None);
        assert!(status.message.contains("均未命中"));
    }

    #[test]
    fn 名称检查禁用时_名称腿不参与() {
        let mut c = ctx(vec!["i-wxxy"], "10.2.127.254", "");
        c.enable_name_check = false;
        c.wired_profile = Some("i-wxxy".to_string());
        let mut probe = |_t: &str, _s: &str| false;
        let status = judge_adapter(&adapter("以太网", "192.168.6.114", false), &c, &mut probe);
        assert!(!status.on_campus, "禁用名称检查时不能靠 profile 名称判校园");
    }

    #[test]
    fn 聚合_命中卡文案优先_无卡返回None() {
        let cards = vec![
            AdapterCampusStatus { name: "以太网".into(), wireless: false, on_campus: false, matched_by: None, message: "失败A".into() },
            AdapterCampusStatus { name: "以太网 2".into(), wireless: false, on_campus: true, matched_by: Some("gateway".into()), message: "成功B".into() },
        ];
        let refs: Vec<&AdapterCampusStatus> = cards.iter().collect();
        let agg = aggregate_class(&refs, None).expect("有卡时应返回聚合状态");
        assert!(agg.on_campus);
        assert_eq!(agg.message, "成功B");

        assert!(aggregate_class(&[], None).is_none(), "无该类卡时返回 None");
    }

    // 静默期三条边界用例已随函数迁移到 config/schedule.rs 的同名测试模块
    // （双端共享单点，避免同一语义的断言两处维护）。
}
