//! 绑源 TCP connect 探测：给「校园侧是否可达」补一档明确否定应答。
//!
//! ICMP 只有「收到回显」与「超时」两种结局，目标禁 ICMP 时只能等满超时；TCP connect
//! 多出第 3 种结局——RST（端口不可达但主机在线），它证明报文确实到达了目标主机，
//! 因此「在网」不再取决于 ICMP 是否被放行。SYN 被静默丢弃时仍走超时，与 ICMP 同价。
//!
//! 绑出接口（Windows IP_UNICAST_IF）是必需的：只绑源地址时路由仍由系统默认路由决定，
//! TUN/代理接管默认路由后探测会被劫持到代理出口（同 network::timing 的说明）。

use std::net::SocketAddr;
use std::time::Duration;

/// 夜间切换的探测端口：Portal 是 web 入口，443 命中概率最高。
pub const CAMPUS_PROBE_PORT: u16 = 443;

/// 绑源/绑接口 TCP connect。返回 true 包含两种情况，对「可达」而言等价：
/// - SYN-ACK（端口开放）
/// - RST（主机在线、端口关）
/// 超时、网络不可达、绑定失败一律 false。
pub fn tcp_probe(
    dest: &str,
    source: Option<&str>,
    if_index: Option<u32>,
    port: u16,
    timeout: Duration,
) -> bool {
    let Ok(dest_ip) = dest.parse::<std::net::IpAddr>() else {
        return false;
    };
    let addr = SocketAddr::new(dest_ip, port);
    let source_ip = source
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<std::net::IpAddr>().ok());
    if source_ip.is_some_and(|src| src.is_ipv4() != dest_ip.is_ipv4()) {
        return false;
    }
    crate::infra::async_util::block_on_sync(async move {
        let socket = match addr {
            SocketAddr::V4(_) => tokio::net::TcpSocket::new_v4(),
            SocketAddr::V6(_) => tokio::net::TcpSocket::new_v6(),
        };
        let Ok(socket) = socket else {
            return false;
        };
        #[cfg(target_os = "windows")]
        if let Some(idx) = if_index {
            if set_unicast_if(&socket, idx).is_err() {
                return false;
            }
        }
        #[cfg(not(target_os = "windows"))]
        let _ = if_index;
        if let Some(src) = source_ip {
            if socket.bind(SocketAddr::new(src, 0)).is_err() {
                return false;
            }
        }
        match tokio::time::timeout(timeout, socket.connect(addr)).await {
            Ok(Ok(_)) => true,
            // RST：目标主机在线并明确拒绝，与 SYN-ACK 同属「报文到达了目标」。
            Ok(Err(e)) => e.kind() == std::io::ErrorKind::ConnectionRefused,
            Err(_) => false,
        }
    })
}

/// 把出接口钉在指定 ifIndex（Windows IP_UNICAST_IF，入参是 4 字节网络字节序 IF_INDEX）。
#[cfg(target_os = "windows")]
fn set_unicast_if(socket: &tokio::net::TcpSocket, if_index: u32) -> Result<(), String> {
    use std::os::windows::io::AsRawSocket;
    use windows::Win32::Networking::WinSock::{setsockopt, IPPROTO_IP, IP_UNICAST_IF, SOCKET};
    let bytes: [u8; 4] = [0, 0, (if_index >> 8) as u8, (if_index & 0xFF) as u8];
    let rc = unsafe {
        setsockopt(
            SOCKET(socket.as_raw_socket() as usize),
            IPPROTO_IP.0,
            IP_UNICAST_IF,
            Some(&bytes),
        )
    };
    if rc != 0 {
        return Err(format!("IP_UNICAST_IF 绑定出接口失败(ifIndex={if_index}): 错误码 {rc}"));
    }
    Ok(())
}
