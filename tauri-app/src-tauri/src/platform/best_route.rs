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
/// 接口 Metric，取所有默认路由（前缀长度 0）中最小者。GetBestRoute 也能查到
/// 同一结论，但全表枚举口径不受「查询单个目的地」的歧义影响，且顺带给出接口
/// 别名——TUN 活跃时最优默认路由指向 TUN（不在适配器列表），前端需在头部展示
/// 其名字而非行内徽标。
pub(crate) struct CurrentOutbound {
    pub if_index: u32,
    pub alias: String,
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
        let mut best: Option<(u64, u32)> = None; // (有效跃点, ifIndex)
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
            if best.is_none_or(|(m, _)| effective < m) {
                best = Some((effective, row.InterfaceIndex));
            }
        }
        FreeMibTable(table.cast());
        let Some((_, if_index)) = best else {
            return Err("路由表中没有 IPv4 默认路由".to_string());
        };
        Ok(CurrentOutbound { if_index, alias: if_alias(if_index).unwrap_or_default() })
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
    use super::{current_outbound_v4, dest_addr_dword};

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
    /// 枚举真实路由表并打印当前出站接口；与 `route print 0.0.0.0`/
    /// `Get-NetRoute -DestinationPrefix 0.0.0.0/0` 的最低有效跃点项人工比对。
    #[test]
    #[ignore = "真机路由表相关"]
    fn real_machine_current_outbound() {
        let out = current_outbound_v4().expect("应能取到当前出站接口");
        println!("当前出站: ifIndex {} alias {}", out.if_index, out.alias);
        assert!(!out.alias.is_empty(), "别名不应为空");
    }
}
