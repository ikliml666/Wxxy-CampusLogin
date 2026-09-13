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

/// 重定向目标解析:支持绝对 URL 与根相对路径(以 / 开头);其余形式(协议相对等)
/// 返回 None 停止跟随。ponytail: 仅服务 Portal 页面探测的 302 场景,更完整的目标
/// 解析(相对路径/协议相对)等真实 Portal 出现该形态再加。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn redirect_target(current: &str, location: &str) -> Option<String> {
    if location.starts_with("http://") || location.starts_with("https://") {
        return Some(location.to_string());
    }
    if let Some(path) = location.strip_prefix('/') {
        if path.starts_with('/') {
            return None; // 协议相对形式 //host/x:不支持,停止跟随
        }
        let (scheme, rest) = current.split_once("://")?;
        let authority = rest.split('/').next()?;
        if authority.is_empty() {
            return None;
        }
        return Some(format!("{scheme}://{authority}/{path}"));
    }
    None
}

/// 两个 URL 的 authority(host:port)是否一致:旁路通道只在同 authority 间跟随重定向
/// (跨主机需重建绑定连接,不值得——返回 3xx 由上层按探测失败处理)。
/// 端口缺省按 80 归一化(显式 :80 与缺省等价)。手写解析而非 hyper::Uri:本函数是
/// 纯函数,需在桌面(无 hyper 依赖)编译供单测。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn same_authority(a: &str, b: &str) -> bool {
    let auth = |u: &str| -> Option<(String, u16)> {
        let rest = u.split_once("://")?.1;
        let authority = rest.split('/').next()?;
        if authority.is_empty() {
            return None;
        }
        match authority.rsplit_once(':') {
            Some((h, p)) if !h.is_empty() && !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) => {
                Some((h.to_string(), p.parse().ok()?))
            }
            _ => Some((authority.to_string(), 80)),
        }
    };
    match (auth(a), auth(b)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

// ===== Android 专用(socket 能力探针 / TCP 连接 / HTTP GET 通道) =====

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
        // 非阻塞 connect 的标准返回:EINPROGRESS(std 映射为 WouldBlock),完成态由 writable().await + take_error 收割
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
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

/// 旁路 HTTP GET 的响应(1MB 上限已在请求层执行)
#[cfg(target_os = "android")]
pub struct HttpReply {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

#[cfg(target_os = "android")]
impl HttpReply {
    /// 与 auth::protocol::read_bounded_body 同语义的响应体解码(Content-Type charset → GBK/UTF-8)。
    /// 响应解析层(GBK 解码、特征匹配)由此继续复用现有代码,旁路只换传输层。
    pub fn decoded_body(&self) -> String {
        let charset = self.content_type.as_deref().and_then(|ct| {
            ct.split(';').find_map(|p| p.trim().strip_prefix("charset=").map(str::to_string))
        });
        crate::platform::console_output::decode_charset_bytes(&self.body, charset.as_deref())
    }
}

/// Android 旁路 HTTP GET(自动旁路),三态返回:
/// - `Ok(Some(reply))` = 旁路通道已完成请求(物理网卡直连);
/// - `Ok(None)` = 旁路不可用(能力被 ROM 封堵 / 无物理网卡 / 非 HTTP 目标),调用方走原通道;
/// - `Err(e)` = 旁路已接管但请求失败(连接失败**不回退**,物理网不通重试原通道只会翻倍超时)。
///
/// 仅支持 http:// 明文(校园 Portal 即此形态);响应体 1MB 上限与
/// auth::protocol::MAX_HTTP_BODY 一致;同 authority 重定向最多跟随 5 次(对齐
/// reqwest Policy::limited(5)),跨主机重定向不跟随(需重建绑定连接,返回 3xx 由上层判定)。
#[cfg(target_os = "android")]
pub async fn http_get_bounded(url: &str, timeout: std::time::Duration) -> std::io::Result<Option<HttpReply>> {
    if bind_capability() != BindCapability::Supported {
        return Ok(None);
    }
    if physical_interface().is_none() {
        eprintln!("[bind-dev] 无可用物理网卡,HTTP 走原通道");
        return Ok(None);
    }
    let uri: hyper::Uri = url
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("[bind-dev] URL 解析失败: {e}")))?;
    if uri.scheme_str() != Some("http") {
        eprintln!("[bind-dev] 非 HTTP 目标({:?}),旁路不支持,走原通道", uri.scheme_str());
        return Ok(None);
    }
    let Some(host) = uri.host().map(str::to_string) else {
        return Ok(None);
    };
    let port = uri.port_u16().unwrap_or(80);
    let stream = tcp_connect_bounded(&host, port, timeout).await?;
    bounded_http_get_over(stream, url, timeout).await.map(Some)
}

/// 在已建立的旁路 TCP 连接上执行最小 HTTP/1.1 GET(hyper http1 握手 + connection: close),
/// 同 authority 重定向最多 5 次。整个请求-响应过程受 timeout 约束。
#[cfg(target_os = "android")]
async fn bounded_http_get_over(
    stream: tokio::net::TcpStream,
    url: &str,
    timeout: std::time::Duration,
) -> std::io::Result<HttpReply> {
    let (mut sender, conn) = hyper::client::conn::http1::handshake(hyper_util::rt::TokioIo::new(stream))
        .await
        .map_err(|e| std::io::Error::other(format!("[bind-dev] http 握手失败: {e}")))?;
    // 连接驱动:connection: close 下服务端响应后即结束;sender drop 后亦随之结束
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            eprintln!("[bind-dev] http 连接结束: {e}");
        }
    });

    let mut current = url.to_string();
    let mut redirects = 0u8;
    loop {
        let request = hyper::Request::builder()
            .method(hyper::Method::GET)
            .uri(current.as_str())
            .header(hyper::header::CONNECTION, "close")
            .body(http_body_util::Empty::<hyper::body::Bytes>::new())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("[bind-dev] 请求构造失败: {e}")))?;

        let response = tokio::time::timeout(timeout, sender.send_request(request))
            .await
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "[bind-dev] http 请求超时"))?
            .map_err(|e| std::io::Error::other(format!("[bind-dev] http 发送失败: {e}")))?;

        let status = response.status();
        if status.is_redirection() && redirects < 5 {
            if let Some(next) = response
                .headers()
                .get(hyper::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|loc| redirect_target(&current, loc))
            {
                if same_authority(&current, &next) {
                    // connection: close 下复用连接前必须消费完上一个响应体,否则后续请求可能失败
                    let _ = http_body_util::BodyExt::collect(response.into_body()).await;
                    redirects += 1;
                    current = next;
                    continue;
                }
                eprintln!("[bind-dev] 跨主机重定向不跟随: {next}");
            }
        }

        // 预检上限:读 CONTENT_LENGTH 头(与 reqwest 路径 content_length 预检同语义;
        // chunked 无此头时由收集后的 len 复检兜底)
        let content_type = response
            .headers()
            .get(hyper::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let content_length: Option<u64> = response
            .headers()
            .get(hyper::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok());
        if content_length.map(|len| len > MAX_HTTP_BODY).unwrap_or(false) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("[bind-dev] 响应体过大(Content-Length={:?})", content_length),
            ));
        }
        let body_in = response.into_body();
        let body = tokio::time::timeout(timeout, http_body_util::BodyExt::collect(body_in))
            .await
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "[bind-dev] 响应体读取超时"))?
            .map_err(|e| std::io::Error::other(format!("[bind-dev] 响应体读取失败: {e}")))?
            .to_bytes();
        if body.len() as u64 > MAX_HTTP_BODY {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "[bind-dev] 响应体超限"));
        }
        return Ok(HttpReply {
            status: status.as_u16(),
            content_type,
            body: body.to_vec(),
        });
    }
}

/// 响应体上限:与 auth::protocol::MAX_HTTP_BODY 同值(1MB),旁路通道独立常量
/// (不引入 auth → network 反向依赖;两侧语义已在注释互指)。
#[cfg(target_os = "android")]
const MAX_HTTP_BODY: u64 = 1024 * 1024;

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
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

    #[test]
    fn 重定向目标_绝对与根相对() {
        assert_eq!(
            redirect_target("http://10.1.99.100/", "http://10.1.99.100/login.html"),
            Some("http://10.1.99.100/login.html".to_string())
        );
        assert_eq!(
            redirect_target("http://10.1.99.100/", "/index_2.html?x=1"),
            Some("http://10.1.99.100/index_2.html?x=1".to_string())
        );
        // 协议相对/纯相对路径:不支持,停止跟随
        assert_eq!(redirect_target("http://10.1.99.100/", "//other.host/x"), None);
        assert_eq!(redirect_target("http://10.1.99.100/a/b", "c/d"), None);
    }

    #[test]
    fn authority_一致性判定() {
        assert!(same_authority("http://10.1.99.100/a", "http://10.1.99.100:80/b"));
        assert!(!same_authority("http://10.1.99.100/a", "http://10.1.99.200/b"));
        assert!(!same_authority("http://10.1.99.100/a", "http://10.1.99.100:8080/b"));
    }
}
