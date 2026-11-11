//! IP 地址冲突检测（读 Windows 的 DAD 结论，不额外发包）。
//!
//! DHCP 客户端拿到地址后会先做 ARP 探测（DAD，地址重复检测）确认同网段没有
//! 另一台主机占用该地址；探测结论落在 IP Helper 的
//! `MIB_UNICASTIPADDRESS_ROW.DadState` 上。直接读它比自己发 ARP 探测更可靠：
//! 不需要管理员、不产生额外广播、与系统「检测到 IP 冲突」弹窗同源，且能覆盖
//! 静态地址与 DHCP 地址两种来源。
//!
//! 只用于诊断「有 IP 但上不了网」这一场景——冲突时该地址上的流量会被交换机
//! 按后到的 ARP 应答改写，表现为时通时断，与网关/Portal 不可达高度混淆。

#[cfg(target_os = "windows")]
use std::net::Ipv4Addr;

/// 该 IPv4 地址是否已被系统判定为冲突（同网段有另一台主机也在用）。
///
/// 参数是已经落在某张网卡上的地址；解析失败返回 false（不是冲突，是调用方
/// 传错了数据，由调用方自行保证）。
#[cfg(target_os = "windows")]
pub fn is_ip_conflicted(ip: &str) -> bool {
    let Ok(want) = ip.parse::<Ipv4Addr>() else {
        return false;
    };
    use windows::Win32::Foundation::NO_ERROR;
    use windows::Win32::NetworkManagement::IpHelper::{
        FreeMibTable, GetUnicastIpAddressTable, MIB_UNICASTIPADDRESS_TABLE,
    };
    use windows::Win32::Networking::WinSock::{IpDadStateDuplicate, AF_INET};

    unsafe {
        let mut table: *mut MIB_UNICASTIPADDRESS_TABLE = std::ptr::null_mut();
        // 返回 NO_ERROR 才是成功；查询失败不等于冲突，宁可漏报也不把正常网络判成冲突
        if GetUnicastIpAddressTable(AF_INET, &mut table) != NO_ERROR || table.is_null() {
            return false;
        }
        let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize);
        let hit = rows.iter().any(|row| {
            // S_addr 是网络字节序，按内存字节直接还原成 Ipv4Addr
            let candidate = Ipv4Addr::from(row.Address.Ipv4.sin_addr.S_un.S_addr.to_ne_bytes());
            candidate == want && row.DadState == IpDadStateDuplicate
        });
        FreeMibTable(table as *const _);
        hit
    }
}

/// 非 Windows 平台无 DAD 结论可读（安卓没有按卡地址冲突的对应语义）
#[cfg(not(target_os = "windows"))]
pub fn is_ip_conflicted(_ip: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::is_ip_conflicted;

    /// 非法/空地址不报冲突：调用方传错数据时不能把正常网络判成冲突
    #[test]
    fn 非法地址不判冲突() {
        assert!(!is_ip_conflicted(""));
        assert!(!is_ip_conflicted("not-an-ip"));
        assert!(!is_ip_conflicted("10.2.38"));
    }

    /// 未被使用的地址不可能处于 Duplicate 态
    #[test]
    fn 未使用地址不判冲突() {
        assert!(!is_ip_conflicted("192.0.2.123"));
    }

    /// 真机验证：走一遍 GetUnicastIpAddressTable 的读表路径，确认不崩溃、
    /// 正常在网地址不会被误报成冲突（表结构解析错误通常在这里暴露）。
    #[test]
    #[ignore = "真机网络环境相关"]
    fn 真机读取_在网卡地址不误报冲突() {
        let adapters = crate::network::get_adapters_force().unwrap_or_default();
        let Some(ip) = adapters.iter().find(|a| !a.ip.is_empty()).map(|a| a.ip.clone()) else {
            return;
        };
        println!("probe {ip}: conflicted={}", is_ip_conflicted(&ip));
        assert!(!is_ip_conflicted(&ip));
        assert!(!is_ip_conflicted("255.255.255.255"));
    }
}
