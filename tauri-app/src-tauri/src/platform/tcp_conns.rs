//! 已建立 TCP 连接按本地地址强制关闭（IP Helper：GetExtendedTcpTable + SetTcpEntry）。
//!
//! 出站切换只改接口跃点，已建立连接的 TCB 仍钉在原网卡上：原卡（校园网）断网后
//! 这些连接只会僵死到 TCP 重传超时，应用层既收不到网络切换事件、也不会重连到新
//! 出口。切换成功后按原出口卡的本地 IPv4 删 TCB（对两端发 RST），让应用立即重建
//! 连接走上新出口。
//!
//! SetTcpEntry 对任意进程的 TCB 生效，需要管理员权限——仅供提权 helper 调用。
//! IPv6 无等价 API（SetTcpEntry 只接受 IPv4 MIB_TCPROW_LH），v6 连接不处理。

use windows::Win32::Foundation::BOOL;
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, SetTcpEntry, MIB_TCPROW_LH, MIB_TCPROW_LH_0, MIB_TCPROW_OWNER_PID,
    MIB_TCP_STATE_DELETE_TCB, MIB_TCP_STATE_LISTEN, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_ALL,
};
use windows::Win32::Networking::WinSock::AF_INET;

/// ERROR_INSUFFICIENT_BUFFER：GetExtendedTcpTable 首查表大小的预期返回。
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;

/// 枚举本机 IPv4 TCP 连接（TCP_TABLE_OWNER_PID_ALL 含监听行）。读表不需要管理员
/// 权限，删除 TCB 才需要。
fn tcp_connection_rows() -> Result<Vec<MIB_TCPROW_OWNER_PID>, String> {
    let address_family = AF_INET.0 as u32;
    let mut size: u32 = 0;
    unsafe {
        let rc = GetExtendedTcpTable(None, &mut size, BOOL(0), address_family, TCP_TABLE_OWNER_PID_ALL, 0);
        if rc != ERROR_INSUFFICIENT_BUFFER {
            return Err(format!("GetExtendedTcpTable 取表大小失败: 错误码 {rc}"));
        }
    }
    // 连接数在两次调用之间可能增长：缓冲不足时按新尺寸重取，最多三次
    for attempt in 0..3 {
        let mut buffer = vec![0u8; size as usize];
        let mut read_size = size;
        let rc = unsafe {
            GetExtendedTcpTable(
                Some(buffer.as_mut_ptr() as *mut core::ffi::c_void),
                &mut read_size,
                BOOL(0),
                address_family,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        };
        if rc == ERROR_INSUFFICIENT_BUFFER && attempt < 2 {
            size = read_size.max(size);
            continue;
        }
        if rc != 0 {
            return Err(format!("GetExtendedTcpTable 读表失败: 错误码 {rc}"));
        }
        let table = unsafe { &*(buffer.as_ptr() as *const MIB_TCPTABLE_OWNER_PID) };
        let count = table.dwNumEntries as usize;
        return Ok(unsafe { std::slice::from_raw_parts(table.table.as_ptr(), count) }.to_vec());
    }
    Err("GetExtendedTcpTable 读表失败: 连接数持续变化".to_string())
}

/// 关闭本地地址属于 `local_ips` 的全部 TCP 连接（跳过 LISTEN 行），返回成功删除
/// 的 TCB 数。本地地址按网络字节序比对（dwLocalAddr 的内存布局即 IPv4 四字节）。
pub fn close_tcp_connections(local_ips: &[String]) -> Result<usize, String> {
    let targets: Vec<u32> = local_ips
        .iter()
        .filter_map(|ip| ip.trim().parse::<std::net::Ipv4Addr>().ok())
        .map(|ip| u32::from_ne_bytes(ip.octets()))
        .collect();
    if targets.is_empty() {
        return Ok(0);
    }
    let mut closed = 0usize;
    for row in tcp_connection_rows()? {
        if row.dwState == MIB_TCP_STATE_LISTEN.0 as u32 || !targets.contains(&row.dwLocalAddr) {
            continue;
        }
        let entry = MIB_TCPROW_LH {
            Anonymous: MIB_TCPROW_LH_0 {
                dwState: MIB_TCP_STATE_DELETE_TCB.0 as u32,
            },
            dwLocalAddr: row.dwLocalAddr,
            dwLocalPort: row.dwLocalPort,
            dwRemoteAddr: row.dwRemoteAddr,
            dwRemotePort: row.dwRemotePort,
        };
        if unsafe { SetTcpEntry(&entry) } == 0 {
            closed += 1;
        }
    }
    Ok(closed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真机冒烟：枚举本机 IPv4 TCP 表成功即可，验证两段式取表与结构布局
    /// （读路径不需要管理员权限；不删除任何连接）。
    #[test]
    fn 枚举本机tcp连接表_冒烟() {
        let rows = tcp_connection_rows().expect("GetExtendedTcpTable 枚举应成功");
        println!("本机 IPv4 TCP 连接行数: {}", rows.len());
    }

    #[test]
    fn 空地址列表直接返回零_不触发表枚举() {
        assert_eq!(close_tcp_connections(&[]).unwrap(), 0);
        assert_eq!(close_tcp_connections(&["not-an-ip".to_string()]).unwrap(), 0);
    }
}
