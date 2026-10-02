//! ICMP 绑源可达性探测（IcmpSendEcho2Ex 直调，仅 Windows）。
//!
//! 为什么不用 surge_ping：其 create_socket 走 socket2 RAW/DGRAM 绑定。非提权
//! 进程 RAW 被拒后回退 DGRAM，而 DGRAM ICMP 在 TUN（clash 类常驻）环境下绑源
//! 发送会系统性失败（用户机上表现为 is_campus_adapter 把校园网卡误判为非校园、
//! 夜间禁用名单漏卡、后台「网关不可达」刷屏；同时段 OS `ping -S` 全部正常）。
//! IcmpSendEcho2Ex 是 IP Helper 的官方探测通道（OS ping 同源），支持绑定源
//! 地址，无管理员要求。
//!
//! IPAddr 参数按 inet_addr 约定取网络字节序：DWORD 的**内存字节**必须恰为
//! 地址的点分十进制字节序，小端平台上即 `u32::from_le_bytes(octets)`
//! （与 [`super::best_route::dest_addr_dword`] 同一约定）；传 0（IPADDR_ANY）
//! 表示不绑定源。

use windows::Win32::Foundation::HANDLE;
use windows::Win32::NetworkManagement::IpHelper::{
    IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho2Ex, ICMP_ECHO_REPLY, IP_SUCCESS,
};

/// IPv4 地址 → IPAddr 参数（网络字节序，见模块注释）。
fn ipaddr_dword(addr: std::net::Ipv4Addr) -> u32 {
    u32::from_le_bytes(addr.octets())
}

/// 从 `source`（None = 不绑定源）对 `dest` 发一次 ICMP echo，超时 `timeout_ms`。
/// 收到应答且 Status == IP_SUCCESS 才算可达（超时/目的不可达均 false）。
/// 必须在阻塞线程上调用（同步等待最长 timeout_ms）。
pub fn icmp_probe_v4(
    dest: std::net::Ipv4Addr,
    source: Option<std::net::Ipv4Addr>,
    timeout_ms: u32,
) -> bool {
    let diag = icmp_probe_v4_diag(dest, source, timeout_ms);
    diag.replies != 0 && diag.status == IP_SUCCESS
}

/// 诊断变体：保留 IcmpSendEcho2Ex 的原始返回值、应答 Status 与 GetLastError，
/// 供真机排查（测试断言失败时打印这三个数即可定位：replies==0 且 last_error
/// 非零 = 调用本身被拒；replies==0 且 last_error==0 = 超时；status != IP_SUCCESS
/// = 收到了应答但非成功状态）。
pub(crate) struct ProbeDiag {
    pub replies: u32,
    pub status: u32,
    /// 生产路径只读 replies/status；last_error 供真机诊断测试打印。
    #[allow(dead_code)]
    pub last_error: u32,
}

pub(crate) fn icmp_probe_v4_diag(
    dest: std::net::Ipv4Addr,
    source: Option<std::net::Ipv4Addr>,
    timeout_ms: u32,
) -> ProbeDiag {
    // 应答缓冲区：MSDN 要求至少 sizeof(ICMP_ECHO_REPLY) + RequestSize + 8 字节
    // （IO_STATUS_BLOCK 余量）。本探测无载荷，取最小合规尺寸。
    let mut reply = [0u8; std::mem::size_of::<ICMP_ECHO_REPLY>() + 8];
    // RequestOptions 必须传 None（用系统默认 TTL/TOS）：全零 IP_OPTION_INFORMATION
    // 的 Ttl=0 会被原样发出去，包在第一跳即 TTL 过期（真机诊断 status=11013
    // IP_TTL_EXPIRED_TRANSIT，OS ping 同参数正常）。
    unsafe {
        let Ok(handle) = IcmpCreateFile() else {
            return ProbeDiag { replies: 0, status: u32::MAX, last_error: 0xFFFF_0001 };
        };
        let replies = IcmpSendEcho2Ex(
            handle,
            HANDLE::default(), // 同步调用：事件与 APC 均为空
            None,              // PIO_APC_ROUTINE
            None,
            source.map(ipaddr_dword).unwrap_or(0),
            ipaddr_dword(dest),
            std::ptr::null(),
            0, // 无载荷
            None,
            reply.as_mut_ptr().cast(),
            reply.len() as u32,
            timeout_ms,
        );
        let last_error = windows::Win32::Foundation::GetLastError().0;
        let status = if replies != 0 {
            reply.as_ptr().cast::<ICMP_ECHO_REPLY>().read().Status
        } else {
            u32::MAX
        };
        let _ = IcmpCloseHandle(handle);
        ProbeDiag { replies, status, last_error }
    }
}

#[cfg(test)]
mod tests {
    use super::{icmp_probe_v4, icmp_probe_v4_diag, ipaddr_dword, IP_SUCCESS};

    /// FFI 网络字节序约定：DWORD 的小端内存字节必须还原出地址的 octets。
    #[test]
    fn ipaddr_dword_uses_network_byte_order() {
        let ip = std::net::Ipv4Addr::new(10, 64, 60, 1);
        assert_eq!(ipaddr_dword(ip).to_le_bytes(), ip.octets());
        assert_eq!(ipaddr_dword(ip), u32::from(ip).swap_bytes());
        assert_eq!(ipaddr_dword(std::net::Ipv4Addr::LOCALHOST), 0x0100_007F);
    }

    /// 真机验证（`cargo test --lib icmp_probe -- --ignored --nocapture`）：
    /// 校园网关 + 绑副卡源地址必须可达（surge_ping 在同环境系统性失败），
    /// 绑一个不存在的本机源地址必须不可达。地址随环境失效时改常量即可。
    #[test]
    #[ignore = "真机网络环境相关"]
    fn real_machine_bound_source_probe() {
        let gw = std::net::Ipv4Addr::new(10, 2, 127, 254);
        let src = std::net::Ipv4Addr::new(192, 168, 6, 109);
        let diag = icmp_probe_v4_diag(gw, Some(src), 2000);
        println!("probe {gw} from {src}: replies={} status={:#x} gle={}", diag.replies, diag.status, diag.last_error);
        assert!(diag.replies != 0 && diag.status == IP_SUCCESS, "绑定真实源地址探测校园网关应可达");

        let bogus_src = std::net::Ipv4Addr::new(192, 0, 2, 123); // TEST-NET，本机不存在
        let rejected = icmp_probe_v4(gw, Some(bogus_src), 2000);
        println!("probe {gw} from {bogus_src}: {rejected}");
        assert!(!rejected, "绑定不存在的源地址应失败");

        let unbound = icmp_probe_v4(gw, None, 2000);
        println!("probe {gw} unbound: {unbound}");
        assert!(unbound, "不绑源探测校园网关应可达");
    }
}
