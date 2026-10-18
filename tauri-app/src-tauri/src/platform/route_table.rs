//! IPv4 路由表读取与「按行身份」精确删除（IpHelper：GetIpForwardTable2 /
//! DeleteIpForwardEntry2）。
//!
//! 为什么不再用 route.exe（2026-10-18 事故，用户 WLAN 被断）：
//! ① 匹配键不含跃点。`route delete <dest> mask <mask> <gateway> [metric m] if n`
//!    与 `DeleteIpForwardEntry2` 的重复/删除判定都只按「目的前缀 + 下一跳 + 接口」，
//!    **Metric 不参与匹配**（微软文档：重复判定 = DestinationPrefix + NextHop +
//!    接口上的同一条目）。切换留下的兜底默认路由与目标卡自带 DHCP 默认路由恰好
//!    共享网关与接口、只差跃点，于是「删我们那行」实际删掉了用户自己的默认路由
//!    （WLAN 失去默认路由，只能重连触发 DHCP 才能恢复）。
//! ② 同键不同跃点的新增根本不可能成功：`CreateIpForwardEntry2` 对同前缀+同下一跳
//!    +同接口的条目标为重复并返回 ERROR_OBJECT_ALREADY_EXISTS；route.exe 同报
//!    「文件已存在」。所以兜底路由在 Windows 上无法与 DHCP 默认路由共存，旧实现
//!    每夜添加失败 → 整夜重放。该能力已删除，本模块只保留「读表 + 四元组精确删除」
//!    供清理历史残留。

use std::net::Ipv4Addr;
use windows::Win32::Foundation::{WIN32_ERROR, ERROR_NOT_FOUND};
use windows::Win32::NetworkManagement::IpHelper::{
    DeleteIpForwardEntry2, FreeMibTable, GetIpForwardTable2, MIB_IPFORWARD_ROW2,
    MIB_IPFORWARD_TABLE2,
};
use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR_INET};

/// 一条 IPv4 路由（只保留判定需要的字段）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RouteV4Row {
    pub if_index: u32,
    pub dest: Ipv4Addr,
    pub prefix_len: u8,
    pub next_hop: Ipv4Addr,
    pub metric: u32,
}

/// 掩码 → 前缀长度（只接受连续掩码：255.255.255.0 → 24）。
/// 非连续掩码报错：调用方据此跳过删除，而不是猜一个前缀长度去动别人的路由。
pub(crate) fn mask_to_prefix_len(mask: Ipv4Addr) -> Result<u8, String> {
    let bits = u32::from(mask);
    let ones = bits.leading_ones() as u8;
    // 连续掩码 = 前 ones 位全 1、其余全 0（ones=0 时 expected=0，规避 32 位移出）
    let expected = u32::MAX.checked_shl(32 - u32::from(ones)).unwrap_or(0);
    if bits != expected {
        return Err(format!("非连续子网掩码: {mask}"));
    }
    Ok(ones)
}

/// 精确删除的结局。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeleteExact {
    /// 找到四元组全等的行并已删除
    Deleted,
    /// 表里没有这一行（从未创建 / 已被系统移除）：幂等成功
    AlreadyGone,
}

/// 枚举全部 IPv4 路由行（只读，免提权）。
pub(crate) fn ipv4_forward_rows() -> Result<Vec<RouteV4Row>, String> {
    unsafe {
        let mut table: *mut MIB_IPFORWARD_TABLE2 = std::ptr::null_mut();
        let rc = GetIpForwardTable2(AF_INET, &mut table);
        if rc != WIN32_ERROR(0) || table.is_null() {
            return Err(format!("GetIpForwardTable2 失败: 错误码 {}", rc.0));
        }
        let raw = std::slice::from_raw_parts(
            (*table).Table.as_ptr(),
            (*table).NumEntries as usize,
        );
        let rows: Vec<RouteV4Row> = raw
            .iter()
            .map(|row| RouteV4Row {
                if_index: row.InterfaceIndex,
                dest: read_v4(&row.DestinationPrefix.Prefix),
                prefix_len: row.DestinationPrefix.PrefixLength,
                next_hop: read_v4(&row.NextHop),
                metric: row.Metric,
            })
            .collect();
        FreeMibTable(table.cast());
        Ok(rows)
    }
}

/// 删除一条路由——只在表里找到（目的前缀, 下一跳, 接口, 跃点）**四元组全等**的行时
/// 才动手（DeleteIpForwardEntry2 的删除判定不看跃点，所以匹配必须自己做）。
/// 找不到 → `AlreadyGone`。同网关+接口但跃点不同的行（目标卡自己的默认路由）永不匹配，
/// 从根上杜绝"删掉用户路由"。
pub(crate) fn delete_route_exact(
    dest: Ipv4Addr,
    prefix_len: u8,
    next_hop: Ipv4Addr,
    if_index: u32,
    metric: u32,
) -> Result<DeleteExact, String> {
    // 表内存随 FreeMibTable 释放，待删行先整行拷出再释放
    let mut matched: Option<MIB_IPFORWARD_ROW2> = None;
    unsafe {
        let mut table: *mut MIB_IPFORWARD_TABLE2 = std::ptr::null_mut();
        let rc = GetIpForwardTable2(AF_INET, &mut table);
        if rc != WIN32_ERROR(0) || table.is_null() {
            return Err(format!("GetIpForwardTable2 失败: 错误码 {}", rc.0));
        }
        let rows = std::slice::from_raw_parts(
            (*table).Table.as_ptr(),
            (*table).NumEntries as usize,
        );
        for row in rows {
            if row.DestinationPrefix.PrefixLength != prefix_len
                || read_v4(&row.DestinationPrefix.Prefix) != dest
                || read_v4(&row.NextHop) != next_hop
                || row.InterfaceIndex != if_index
                || row.Metric != metric
            {
                continue;
            }
            let mut keep: MIB_IPFORWARD_ROW2 = std::mem::zeroed();
            std::ptr::copy_nonoverlapping(row, &mut keep, 1);
            matched = Some(keep);
            break;
        }
        FreeMibTable(table.cast());
    }
    let Some(row) = matched else {
        return Ok(DeleteExact::AlreadyGone);
    };
    let rc = unsafe { DeleteIpForwardEntry2(&row) };
    if rc == WIN32_ERROR(0) || rc == ERROR_NOT_FOUND {
        Ok(DeleteExact::Deleted)
    } else {
        Err(format!("DeleteIpForwardEntry2 失败: 错误码 {}", rc.0))
    }
}

/// SOCKADDR_INET（v4 分支）→ Ipv4Addr。S_addr 按网络字节序存放：u32 的内存字节
/// 恰为点分十进制字节序，因此按本机字节序取回再当字节数组用（与 best_route 的
/// `dest_addr_dword` 同一口径）。
fn read_v4(addr: &SOCKADDR_INET) -> Ipv4Addr {
    let raw = unsafe { addr.Ipv4.sin_addr.S_un.S_addr };
    Ipv4Addr::from(raw.to_ne_bytes())
}

#[cfg(test)]
mod tests {
    use super::mask_to_prefix_len;
    use std::net::Ipv4Addr;

    #[test]
    fn mask_to_prefix_len_accepts_contiguous_masks() {
        assert_eq!(mask_to_prefix_len(Ipv4Addr::new(0, 0, 0, 0)), Ok(0));
        assert_eq!(mask_to_prefix_len(Ipv4Addr::new(255, 255, 255, 0)), Ok(24));
        assert_eq!(mask_to_prefix_len(Ipv4Addr::new(255, 255, 255, 255)), Ok(32));
        // 非连续掩码不能猜前缀长度（删路由时猜错 = 动别人的路由）
        assert!(mask_to_prefix_len(Ipv4Addr::new(255, 0, 255, 0)).is_err());
        assert!(mask_to_prefix_len(Ipv4Addr::new(0, 255, 255, 255)).is_err());
    }

    #[test]
    fn ipv4_forward_rows_reads_live_table() {
        // 只读冒烟：GetIpForwardTable2 → 字段读取 → FreeMibTable 全链路可用，
        // 且地址按预期字节序还原（打印出来可与 `route print 0.0.0.0` 人工对账；
        // 不断言行数——无线卡机器的路由表内容随网络状态变化）
        let rows = super::ipv4_forward_rows().expect("读取本机 IPv4 路由表失败");
        for row in &rows {
            assert!(row.prefix_len <= 32, "前缀长度越界: {row:?}");
        }
        for row in rows.iter().filter(|r| r.prefix_len == 0) {
            eprintln!("默认路由: via {} if {} metric {}", row.next_hop, row.if_index, row.metric);
        }
    }
}
