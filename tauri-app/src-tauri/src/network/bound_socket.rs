//! Android SO_BINDTODEVICE 物理网卡旁路:全量 VPN(Clash/UU 等,未排除本应用)接管流量时,
//! 让 socket 探测与登录请求从物理网络(WiFi)直连校园内网,而不是被 tun 劫持。
//!
//! 技术依据(内核 ≥5.7,Android 12+):新建 socket **首次** `setsockopt(SOL_SOCKET,
//! SO_BINDTODEVICE, 接口名)` 不需要 CAP_NET_RAW,普通应用可用;且它不经 netd 的
//! ip rule 路由(RULE_PRIORITY_SECURE_VPN=13000 先于网络绑定规则),由内核直接按
//! 设备查路由,可绕开全量 VPN 的 uid 劫持。ROM 可能封堵(GrapheneOS 同款封堵仅限
//! lockdown VPN),因此必须**探针判能力 + 失败自动回退**:对 "lo" 试绑定,Ok→Supported、
//! EPERM/EACCES→Denied、其他错→Unavailable,OnceLock 缓存(ROM 能力运行期不变)。
//!
//! 诊断日志统一 `[bind-dev]` 前缀(真机 logcat 抓取用)。
//!
//! 结构:纯函数(能力分类/接口过滤/重定向目标解析)全平台编译、桌面单测覆盖;
//! socket 探测/TCP 连接/HTTP GET 通道仅 Android 编目编译,桌面零编译、零行为变化。

// ===== 纯函数(全平台编译;桌面单测覆盖) =====

/// SO_BINDTODEVICE 能力判定结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindCapability {
    /// 新建 socket 可绑定物理网卡(内核 ≥5.7 未封堵)
    Supported,
    /// 被 ROM/SELinux 封堵(EPERM/EACCES),旁路不可用,一律走原通道
    Denied,
    /// 其他错误(如 ENODEV/EOPNOTSUPP),视为不可用
    Unavailable,
}

/// 探针结果 → 能力分类(纯函数,便于单测)
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn classify_probe_result(probe: &std::io::Result<()>) -> BindCapability {
    match probe {
        Ok(()) => BindCapability::Supported,
        Err(e) => match e.kind() {
            std::io::ErrorKind::PermissionDenied => BindCapability::Denied,
            _ => BindCapability::Unavailable,
        },
    }
}

/// VPN tun 接口宽松匹配(tun*/utun*/ppp*)。Clash 等的 tun 可能叫 utun/tun/自定义名,
/// 宽松匹配即可——这是提示用途,误报无害。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn is_vpn_tun(name: &str) -> bool {
    name.starts_with("tun") || name.starts_with("utun") || name.starts_with("ppp")
}

/// 物理网卡选择时的排除名单:VPN tun、蜂窝(rmnet*/ccmni*,与安卓端
/// campus_detect::pick_campus_source_ip 的蜂窝排除同源语义)、回环、dummy 虚拟口。
/// 说明:不与 campus_detect 抽公共函数——两者的输入形状不同(IP 对 vs 接口名),
/// 公共部分仅蜂窝前缀一行,复制同款规则并互相注释,避免为一行耦合跨模块。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn is_excluded_interface(name: &str) -> bool {
    is_vpn_tun(name)
        || name.starts_with("rmnet")
        || name.starts_with("ccmni")
        || name == "lo"
        || name.starts_with("dummy")
}

/// 从接口名列表选出旁路绑定的物理网卡:排除 VPN tun/蜂窝/回环/dummy;
/// wlan0 优先,其余按枚举序取首个。按"接口名"而非"接口 IP"选择——无 IP 的
/// 物理口也可按名绑定(SO_BINDTODEVICE 走名字/索引,不要求已有地址)。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn pick_physical_interface(names: &[String]) -> Option<String> {
    let candidates: Vec<&str> = names.iter().map(String::as_str).filter(|n| !is_excluded_interface(n)).collect();
    candidates
        .iter()
        .find(|n| **n == "wlan0")
        .or_else(|| candidates.first())
        .map(|s| (*s).to_string())
}

// ===== Android 专用(socket 能力探针 / TCP 连接 / 本地旁路代理) =====

/// 能力探针:对一次性 socket 尝试绑定 "lo"(恒存在),按 errno 分类,OnceLock 缓存。
#[cfg(target_os = "android")]
pub fn bind_capability() -> BindCapability {
    use std::sync::OnceLock;
    static CAP: OnceLock<BindCapability> = OnceLock::new();
    *CAP.get_or_init(|| {
        let probe = (|| -> std::io::Result<()> {
            use socket2::{Domain, Socket, Type};
            let sock = Socket::new(Domain::IPV4, Type::STREAM, None)?;
            sock.bind_device(Some(b"lo".as_slice()))
        })();
        let cap = classify_probe_result(&probe);
        eprintln!("[bind-dev] 能力探针(lo) = {cap:?} (errno kind: {:?})", probe.err().map(|e| e.kind()));
        cap
    })
}

/// 枚举网络接口名(无 IP 的接口也返回——绑定按名字)。
#[cfg(target_os = "android")]
fn list_interface_names() -> std::io::Result<Vec<String>> {
    use network_interface::NetworkInterfaceConfig;
    Ok(network_interface::NetworkInterface::show()
        .map_err(|e| std::io::Error::other(format!("网卡枚举失败: {e}")))?
        .into_iter()
        .map(|i| i.name)
        .collect())
}

/// 是否存在 VPN tun 接口(tun*/utun*/ppp*,宽松匹配,误报无害——仅提示用途)。
#[cfg(target_os = "android")]
pub fn vpn_tun_present() -> bool {
    match list_interface_names() {
        Ok(names) => names.iter().any(|n| is_vpn_tun(n)),
        Err(e) => {
            eprintln!("[bind-dev] 枚举网卡失败(vpn_tun_present): {e}");
            false
        }
    }
}

/// 选出旁路绑定的物理网卡(见 pick_physical_interface)。
#[cfg(target_os = "android")]
pub fn physical_interface() -> Option<String> {
    match list_interface_names() {
        Ok(names) => pick_physical_interface(&names),
        Err(e) => {
            eprintln!("[bind-dev] 枚举网卡失败(physical_interface): {e}");
            None
        }
    }
}

/// 解析目标为 IPv4 SocketAddr:IPv4 字面量直用;域名走系统解析取首个 A 记录。
/// 已知边界:域名解析不经旁路 socket(getaddrinfo → netd),全量 VPN 下可能拿到
/// tun DNS 的结果——校园 Portal 配置通常为 IP 直填,域名场景以真机实测为准。
#[cfg(target_os = "android")]
async fn resolve_v4(host: &str, port: u16) -> std::io::Result<std::net::SocketAddr> {
    if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
        return Ok(std::net::SocketAddr::from((ip, port)));
    }
    let mut addrs = tokio::net::lookup_host((host, port)).await?;
    addrs.find(|a| a.is_ipv4()).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, format!("[bind-dev] {host} 无 IPv4 解析结果"))
    })
}

/// 创建绑定到物理网卡的非阻塞 socket 并发起非阻塞 connect:
/// socket 创建/绑定层错误向上传(调用方回退普通连接);connect 发起失败同样上传
/// (connect 层失败**不回退**——物理网不通时重试原通道也必失败,只会翻倍超时)。
#[cfg(target_os = "android")]
fn create_bound_stream(iface: &str, addr: std::net::SocketAddr) -> std::io::Result<tokio::net::TcpStream> {
    use socket2::{Domain, Socket, Type};
    let sock = Socket::new(Domain::IPV4, Type::STREAM, None)?;
    // 新建 socket 首次绑定不需 CAP_NET_RAW(内核 ≥5.7);改绑已绑过的 socket 才 EPERM
    sock.bind_device(Some(iface.as_bytes()))?;
    // tokio::net::TcpStream::from_std 要求非阻塞 socket,必须在 connect 前设置
    sock.set_nonblocking(true)?;
    match sock.connect(&addr.into()) {
        Ok(()) => {}
        // 非阻塞 connect 返回 EINPROGRESS(115)/EWOULDBLOCK 都是"进行中",完成态由
        // writable().await + take_error 收割。注意 std 不把 EINPROGRESS 映射为
        // WouldBlock(真机 2026-09-14 实证:误判失败导致旁路整体回退),须按 raw errno 判
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.raw_os_error() == Some(libc::EINPROGRESS) => {}
        Err(e) => return Err(e),
    }
    let std_stream: std::net::TcpStream = sock.into();
    tokio::net::TcpStream::from_std(std_stream)
}

/// 普通连接(回退路径,与安卓端 campus_detect::portal_reachable 原实现同语义)
#[cfg(target_os = "android")]
async fn plain_connect(host: &str, port: u16, timeout: std::time::Duration) -> std::io::Result<tokio::net::TcpStream> {
    tokio::time::timeout(timeout, tokio::net::TcpStream::connect((host, port)))
        .await
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, format!("[bind-dev] 普通连接 {host}:{port} 超时")))?
}

/// 经物理网卡直连的 TCP connect(自动旁路):
/// - 能力不受支持或无物理网卡 → 内部回退普通连接(调用方无感);
/// - socket 创建/SO_BINDTODEVICE 层出错 → 记诊断后回退普通连接;
/// - connect 层失败(含超时)→ **不回退**,错误如实上报(避免翻倍超时)。
#[cfg(target_os = "android")]
pub async fn tcp_connect_bounded(
    host: &str,
    port: u16,
    timeout: std::time::Duration,
) -> std::io::Result<tokio::net::TcpStream> {
    if bind_capability() != BindCapability::Supported {
        return plain_connect(host, port, timeout).await;
    }
    let Some(iface) = physical_interface() else {
        eprintln!("[bind-dev] 无可用物理网卡,回退普通连接: {host}:{port}");
        return plain_connect(host, port, timeout).await;
    };
    let addr = resolve_v4(host, port).await?;
    let stream = match create_bound_stream(&iface, addr) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[bind-dev] socket 绑定 {iface} 失败: {e},回退普通连接");
            return plain_connect(host, port, timeout).await;
        }
    };
    match tokio::time::timeout(timeout, stream.writable()).await {
        Err(_) => Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!("[bind-dev] connect {addr} 超时(绑定 {iface})"),
        )),
        Ok(Err(e)) => Err(e),
        Ok(Ok(())) => match stream.take_error()? {
            None => Ok(stream),
            Some(e) => Err(e),
        },
    }
}

// ===== Android 本地旁路代理:reqwest 经 127.0.0.1 转发,传输层走 SO_BINDTODEVICE =====
//
// 为什么是转发代理而不是手写 HTTP:真机对照实验(2026-09-14)证实同一 URL 下
// reqwest 200 而 hyper 手写请求被网关 nginx 400(补齐 accept/cache-control/pragma、
// 去掉 SO_BINDTODEVICE 均无法消除,见 CHANGELOG 排查记录),请求头/编码差异无法
// 穷举定位。改为本地起单请求 HTTP 转发器:reqwest 发代理请求(absolute-form)到
// 127.0.0.1,转发器解析出目标后在旁路 socket 上以 origin-form 重放,响应原样回写——
// HTTP 层(头/重定向/charset)100% 复用 reqwest,只有传输层被替换。
//
// 单请求语义:每连接处理一个请求-响应后关闭,响应注入 connection: close 令
// reqwest 不复用连接;上游请求带 connection: close 使服务器响应后断开,转发
// 读到 EOF 即完整响应。

/// 取(或惰性启动)本地旁路代理地址;旁路不可用(能力被封堵/无物理网卡)返回 None。
#[cfg(target_os = "android")]
pub fn bypass_proxy_addr() -> Option<&'static std::net::SocketAddr> {
    use std::sync::OnceLock;
    static ADDR: OnceLock<Option<std::net::SocketAddr>> = OnceLock::new();
    ADDR.get_or_init(|| {
        if bind_capability() != BindCapability::Supported {
            eprintln!("[bind-dev] 能力不受支持,旁路代理不启动");
            return None;
        }
        if physical_interface().is_none() {
            eprintln!("[bind-dev] 无可用物理网卡,旁路代理不启动");
            return None;
        }
        let listener = std::net::TcpListener::bind("127.0.0.1:0").ok()?;
        listener.set_nonblocking(true).ok()?;
        let addr = listener.local_addr().ok()?;
        tokio::spawn(async move {
            let listener = match tokio::net::TcpListener::from_std(listener) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("[bind-dev] 代理监听接管失败: {e}");
                    return;
                }
            };
                loop {
                    match listener.accept().await {
                        Ok((down, _)) => {
                            tokio::spawn(proxy_one(down));
                        }
                        Err(e) => eprintln!("[bind-dev] 代理 accept 失败: {e}"),
                    }
                }
        });
        eprintln!("[bind-dev] 旁路代理已启动: {addr}");
        Some(addr)
    })
    .as_ref()
}

/// 代理单连接:读一个请求头块,旁路连接上游并重放,响应回写后关闭。
#[cfg(target_os = "android")]
async fn proxy_one(mut down: tokio::net::TcpStream) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // 1. 读请求头块(到 \r\n\r\n,上限 32KB)
    let mut head: Vec<u8> = Vec::with_capacity(1024);
    let mut chunk = [0u8; 512];
    loop {
        match down.read(&mut chunk).await {
            Ok(0) => return,
            Ok(n) => {
                head.extend_from_slice(&chunk[..n]);
                if head.ends_with(b"\r\n\r\n") {
                    break;
                }
                if head.len() > 32 * 1024 {
                    return;
                }
            }
            Err(_) => return,
        }
    }
    let head_str = String::from_utf8_lossy(&head).into_owned();
    let mut lines = head_str.split("\r\n");
    let req_line = lines.next().unwrap_or("");
    let mut parts = req_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let abs_uri = parts.next().unwrap_or("");
    let version = parts.next().unwrap_or("HTTP/1.1");

    // 2. 解析目标(http:// 明文;校园请求即此形态,https/CONNECT 不支持)
    let Some((host, port, origin_form)) = parse_proxy_target(abs_uri) else {
        let _ = down.write_all(b"HTTP/1.1 502 Bad Gateway\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").await;
        return;
    };

    // 3. 旁路连接上游(connect 失败→502,由 reqwest 侧按失败处理,不回退原通道)
    let mut up = match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tcp_connect_bounded(&host, port, std::time::Duration::from_secs(10)),
    )
    .await
    {
        Ok(Ok(s)) => s,
        _ => {
            let _ = down.write_all(b"HTTP/1.1 502 Bad Gateway\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").await;
            return;
        }
    };

    // 4. 改写请求行(absolute-form → origin-form)+ 原样透传其余头,追加 connection: close
    let rest = head_str.split_once("\r\n").map(|(_, r)| r).unwrap_or("");
    let rest = rest.strip_suffix("\r\n\r\n").unwrap_or(rest);
    let mut req: Vec<u8> = Vec::with_capacity(head.len() + 32);
    req.extend_from_slice(format!("{method} {origin_form} {version}\r\n").as_bytes());
    req.extend_from_slice(rest.as_bytes());
    req.extend_from_slice(b"connection: close\r\n\r\n");
    if up.write_all(&req).await.is_err() {
        return;
    }

    // 5. 上游响应(带 connection: close,读到 EOF 即完整)原样回写
    let _ = tokio::io::copy(&mut up, &mut down).await;
    // drop 时两端自然关闭
}

/// 代理请求行目标解析:http:// 的 absolute-form → (host, port, origin-form 路径)。
/// 纯函数,桌面单测覆盖。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn parse_proxy_target(abs_uri: &str) -> Option<(String, u16, String)> {
    let rest = abs_uri.strip_prefix("http://")?;
    let (authority, path) = match rest.split_once('/') {
        Some((a, p)) => (a, format!("/{p}")),
        None => (rest, "/".to_string()),
    };
    if authority.is_empty() {
        return None;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() && !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) => {
            (h.to_string(), p.parse().ok()?)
        }
        _ => (authority.to_string(), 80),
    };
    Some((host, port, path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn 代理目标解析_绝对形式() {
        assert_eq!(
            parse_proxy_target("http://10.1.99.100:801/eportal/portal/login?a=1"),
            Some(("10.1.99.100".to_string(), 801, "/eportal/portal/login?a=1".to_string()))
        );
        assert_eq!(
            parse_proxy_target("http://10.1.99.100/"),
            Some(("10.1.99.100".to_string(), 80, "/".to_string()))
        );
        // 无 path 的 absolute-form 同样合法,origin-form 归一化为 "/"
        assert_eq!(
            parse_proxy_target("http://10.1.99.100"),
            Some(("10.1.99.100".to_string(), 80, "/".to_string()))
        );
        assert_eq!(parse_proxy_target("https://10.1.99.100/"), None);
        assert_eq!(parse_proxy_target("/eportal/"), None);
    }

    #[test]
    fn vpn_tun_前缀识别() {
        for n in ["tun0", "utun3", "ppp0", "tun_clash"] {
            assert!(is_vpn_tun(n), "{n} 应识别为 VPN tun");
        }
        for n in ["wlan0", "eth0", "rmnet_data0", "lo"] {
            assert!(!is_vpn_tun(n), "{n} 不应识别为 VPN tun");
        }
    }

    #[test]
    fn 物理网卡选择_wlan0优先() {
        assert_eq!(
            pick_physical_interface(&names(&["rmnet_data0", "tun0", "wlan0", "dummy0"])),
            Some("wlan0".to_string())
        );
    }

    #[test]
    fn 无_wlan0_时取枚举序首个非排除接口() {
        assert_eq!(
            pick_physical_interface(&names(&["tun0", "lo", "eth0", "wlan1_bak"])),
            Some("eth0".to_string())
        );
    }

    #[test]
    fn 蜂窝与回环与虚拟接口全排除() {
        assert_eq!(pick_physical_interface(&names(&["rmnet0", "ccmni0", "lo", "dummy0", "tun0"])), None);
    }

    #[test]
    fn 无_ip_接口名也可被选中() {
        // 绑定按名字,选择只看名字不看地址
        assert_eq!(pick_physical_interface(&names(&["wlan0"])), Some("wlan0".to_string()));
    }

    #[test]
    fn 能力分类_按_errno_kind() {
        assert_eq!(classify_probe_result(&Ok(())), BindCapability::Supported);
        assert_eq!(
            classify_probe_result(&Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))),
            BindCapability::Denied
        );
        assert_eq!(
            classify_probe_result(&Err(std::io::Error::from(std::io::ErrorKind::Unsupported))),
            BindCapability::Unavailable
        );
    }
}
