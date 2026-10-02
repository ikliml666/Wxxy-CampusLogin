//! 路由级切换验证：GetBestRoute 查「到指定目的地的最优路由出接口」。
//! 用途（夜间出站切换 P2-2）：禁用校园卡的动作成功 ≠ 效果达成——切换完成后
//! 校验到校园网关的最优路由已不再指向校园卡 ifIndex，抓「禁用未产生预期路由
//! 效果 / 校园卡被误判漏禁」两类静默失败。不校验「出站=目标卡」：TUN 活跃时
//! 最优默认路由始终是 TUN，会误报。
//!
//! 注意 GetBestRoute 是纯路由表查询，不产生流量、不依赖公网连通性，确定性强。

use windows::Win32::NetworkManagement::IpHelper::{GetBestRoute, MIB_IPFORWARDROW};

/// IPv4 地址 → GetBestRoute 的 `dwDestAddr` 参数。该参数按 inet_addr 约定取
/// 网络字节序：DWORD 的**内存字节**必须恰为地址的点分十进制字节序
/// （127.0.0.1 → 内存 7F 00 00 01）。小端平台上即
/// `u32::from_le_bytes(octets)`＝0x0100_007F；而 `u32::from(Ipv4Addr)` 给的是
/// 主机序大端数值（0x7F000001），会被协议栈当成 1.0.0.127 查到错误目的地，
/// 路由验证整体失真。
fn dest_addr_dword(dest: std::net::Ipv4Addr) -> u32 {
    u32::from_le_bytes(dest.octets())
}

/// 查询到 `dest` 的最优路由出接口 ifIndex。查询失败（如目的地非法）返回 Err，
/// 调用方按「无法验证」跳过而非判失败。
pub(crate) fn best_route_if_index_v4(dest: std::net::Ipv4Addr) -> Result<u32, String> {
    let mut row: MIB_IPFORWARDROW = unsafe { std::mem::zeroed() };
    // 源地址传 0：让协议栈自行选源（与实际出站路径判定一致）
    let rc = unsafe { GetBestRoute(dest_addr_dword(dest), 0, &mut row) };
    if rc != 0 {
        return Err(format!("GetBestRoute 失败: 错误码 {rc}"));
    }
    Ok(row.dwForwardIfIndex)
}

/// 当前生效的 IPv4 默认路由出接口（GetIpForwardTable2 + 接口跃点求和取最小）。
/// 判定口径与 WireGuard/Tailscale/Mullvad 一致：有效跃点 = 路由 Metric +
/// 接口 Metric，取所有默认路由（前缀长度 0）中最小者。展示口径只认真实
/// 物理网卡：FlClash/WireGuard 等代理虚拟卡跃点为 0 恒胜但不承载真实出口，
/// 按别名命中适配器发现黑名单（discovery::is_blacklisted，与适配器列表同一
/// 过滤口径）跳过后取跃点最优的物理网卡；没有物理默认路由时返回 Err，
/// 前端隐藏「当前出站」徽标。
pub(crate) struct CurrentOutbound {
    pub if_index: u32,
    pub alias: String,
}

/// 默认路由候选（有效跃点, ifIndex, 接口别名）中选最优物理网卡出口。
/// 别名命中黑名单（TUN/TAP/代理虚拟卡）的候选不参与；别名查询失败（空串）
/// 保守视为物理网卡；全被过滤时返回 None。
fn best_physical_outbound(candidates: &[(u64, u32, String)]) -> Option<(u64, u32, &str)> {
    candidates
        .iter()
        .filter(|(_, _, alias)| !crate::network::is_blacklisted(alias))
        .min_by_key(|(metric, _, _)| *metric)
        .map(|(metric, if_index, alias)| (*metric, *if_index, alias.as_str()))
}

pub(crate) fn current_outbound_v4() -> Result<CurrentOutbound, String> {
    use windows::Win32::Foundation::WIN32_ERROR;
    use windows::Win32::NetworkManagement::IpHelper::{
        FreeMibTable, GetIpForwardTable2, GetIpInterfaceEntry, MIB_IPFORWARD_TABLE2,
        MIB_IPINTERFACE_ROW,
    };
    use windows::Win32::Networking::WinSock::AF_INET;

    unsafe {
        let mut table: *mut MIB_IPFORWARD_TABLE2 = std::ptr::null_mut();
        let rc = GetIpForwardTable2(AF_INET, &mut table);
        if rc != WIN32_ERROR(0) || table.is_null() {
            return Err(format!("GetIpForwardTable2 失败: 错误码 {}", rc.0));
        }
        let rows =
            std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize);
        let mut candidates: Vec<(u64, u32, String)> = Vec::new();
        for row in rows {
            // 仅默认路由：前缀长度 0
            if row.DestinationPrefix.PrefixLength != 0 {
                continue;
            }
            let mut iface: MIB_IPINTERFACE_ROW = std::mem::zeroed();
            iface.Family = AF_INET;
            iface.InterfaceIndex = row.InterfaceIndex;
            if GetIpInterfaceEntry(&mut iface) != WIN32_ERROR(0) {
                continue; // 接口行缺失（如接口刚移除）→ 跳过该路由
            }
            let effective = row.Metric as u64 + iface.Metric as u64;
            let alias = if_alias(row.InterfaceIndex).unwrap_or_default();
            candidates.push((effective, row.InterfaceIndex, alias));
        }
        FreeMibTable(table.cast());
        if candidates.is_empty() {
            return Err("路由表中没有 IPv4 默认路由".to_string());
        }
        let Some((_, if_index, alias)) = best_physical_outbound(&candidates) else {
            return Err("没有承载真实出口的物理网卡默认路由（仅虚拟网卡活跃）".to_string());
        };
        Ok(CurrentOutbound { if_index, alias: alias.to_string() })
    }
}

/// ifIndex → 接口别名（GetIfEntry2）。别名即 ncpa.cpl 里显示的连接名
/// （如「以太网 2」「Meta Tunnel」）。
fn if_alias(if_index: u32) -> Option<String> {
    use windows::Win32::Foundation::WIN32_ERROR;
    use windows::Win32::NetworkManagement::IpHelper::{GetIfEntry2, MIB_IF_ROW2};

    unsafe {
        let mut row: MIB_IF_ROW2 = std::mem::zeroed();
        row.InterfaceIndex = if_index;
        if GetIfEntry2(&mut row) != WIN32_ERROR(0) {
            return None;
        }
        let len = row.Alias.iter().position(|&c| c == 0).unwrap_or(row.Alias.len());
        Some(String::from_utf16_lossy(&row.Alias[..len]))
    }
}

#[cfg(test)]
mod tests {
    use super::{best_physical_outbound, current_outbound_v4, dest_addr_dword};

    /// 虚拟网卡（代理 TUN）跃点 0 恒胜时应被跳过，取跃点次优的物理网卡。
    #[test]
    fn best_physical_skips_virtual_hijack() {
        let cands = vec![
            (0u64, 45u32, "FlClash".to_string()),
            (35, 16, "以太网".to_string()),
            (50, 12, "WLAN".to_string()),
        ];
        let got = best_physical_outbound(&cands).map(|(m, i, a)| (m, i, a.to_string()));
        assert_eq!(got, Some((35, 16, "以太网".to_string())));
    }

    /// 全部候选都是虚拟网卡 → None（调用方返回 Err，前端隐藏徽标）。
    #[test]
    fn best_physical_none_when_only_virtual() {
        let cands = vec![
            (0u64, 45u32, "Meta Tunnel".to_string()),
            (1, 46, "WireGuard".to_string()),
        ];
        assert!(best_physical_outbound(&cands).is_none());
    }

    /// 别名查询失败（空串）保守视为物理网卡，不因黑名单误杀。
    #[test]
    fn best_physical_empty_alias_kept() {
        let cands = vec![(0u64, 45u32, "FlClash".to_string()), (10, 16, String::new())];
        let got = best_physical_outbound(&cands).map(|(_, i, _)| i);
        assert_eq!(got, Some(16));
    }

    /// FFI 网络字节序约定：DWORD 的小端内存字节必须还原出地址的 octets，
    /// 且与 `u32::from`（主机序大端数值）恰为字节反转关系。
    #[test]
    fn dest_addr_dword_uses_network_byte_order() {
        let ip = std::net::Ipv4Addr::new(10, 64, 60, 1);
        assert_eq!(dest_addr_dword(ip).to_le_bytes(), ip.octets());
        assert_eq!(dest_addr_dword(ip), u32::from(ip).swap_bytes());
        assert_ne!(dest_addr_dword(ip), u32::from(ip));
        assert_eq!(dest_addr_dword(std::net::Ipv4Addr::LOCALHOST), 0x0100_007F);
    }

    /// 真机验证（`cargo test --lib best_route -- --ignored --nocapture`）：
    /// 枚举真实路由表并打印当前出站接口；FlClash TUN 活跃时应取跃点次优的
    /// 物理网卡而非 TUN（与 `Get-NetRoute -DestinationPrefix 0.0.0.0/0` 比对）。
    #[test]
    #[ignore = "真机路由表相关"]
    fn real_machine_current_outbound() {
        let out = current_outbound_v4().expect("应能取到当前出站接口");
        println!("当前出站: ifIndex {} alias {}", out.if_index, out.alias);
        assert!(!out.alias.is_empty(), "别名不应为空");
        assert!(
            !crate::network::is_blacklisted(&out.alias),
            "出站徽标只认物理网卡：虚拟网卡 {} (ifIndex {}) 不应入选", out.alias, out.if_index
        );
    }
}
