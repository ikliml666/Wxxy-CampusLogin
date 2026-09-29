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

#[cfg(test)]
mod tests {
    use super::dest_addr_dword;

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
}
