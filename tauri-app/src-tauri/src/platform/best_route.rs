//! 路由级切换验证：GetBestRoute 查「到指定目的地的最优路由出接口」。
//! 用途（夜间出站切换 P2-2）：禁用校园卡的动作成功 ≠ 效果达成——切换完成后
//! 校验到校园网关的最优路由已不再指向校园卡 ifIndex，抓「禁用未产生预期路由
//! 效果 / 校园卡被误判漏禁」两类静默失败。不校验「出站=目标卡」：TUN 活跃时
//! 最优默认路由始终是 TUN，会误报。
//!
//! 注意 GetBestRoute 是纯路由表查询，不产生流量、不依赖公网连通性，确定性强。

use windows::Win32::NetworkManagement::IpHelper::{GetBestRoute, MIB_IPFORWARDROW};

/// 查询到 `dest` 的最优路由出接口 ifIndex。查询失败（如目的地非法）返回 Err，
/// 调用方按「无法验证」跳过而非判失败。
pub(crate) fn best_route_if_index_v4(dest: std::net::Ipv4Addr) -> Result<u32, String> {
    let mut row: MIB_IPFORWARDROW = unsafe { std::mem::zeroed() };
    // 源地址传 0：让协议栈自行选源（与实际出站路径判定一致）
    let rc = unsafe { GetBestRoute(u32::from(dest), 0, &mut row) };
    if rc != 0 {
        return Err(format!("GetBestRoute 失败: 错误码 {rc}"));
    }
    Ok(row.dwForwardIfIndex)
}
