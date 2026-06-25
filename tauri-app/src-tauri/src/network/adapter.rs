use regex::Regex;
use serde::Serialize;
use lazy_static::lazy_static;
use parking_lot::Mutex;
use std::time::Instant;
use std::sync::atomic::AtomicBool;
use tauri::AppHandle;
use crate::config::model::Config;
use crate::infra::events::EventBus;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

lazy_static! {
    // 名称/描述黑名单：作为 is_visible_in_ncpa 的纵深防御层
    // 规则：nat/tor/virtual 加 \b 词边界，避免误伤 "Native/Intel NAT Offload/Toronto/Tornado" 等合法名
    //      中文补全：覆盖 "虚拟/伪/假/测试/模拟/隧道"，避免漏过滤中文命名的虚拟网卡
    //      保留 "本地连接"：用户特定业务规则（Win11 高级网络设置不可见，强制排除）
    // 注意：WLAN/以太网的具体可见性判断已移到 is_visible_in_ncpa（注册表检查），
    //      避免按名称误伤多物理网卡场景（如 2 块真实无线网卡可能都叫 "WLAN"）
    static ref BL_REGEX: Regex = Regex::new(r"(?i)hyper-v|\bvirtual\b|vmware|veth|docker|wsl|loopback|tunnel|isatap|6to4|teredo|bluetooth|vpn|hamachi|zerotier|tailscale|wireguard|vEthernet|HNS|\bnat\b|filter.?driver|packet.?driver|npcap|qos|packet.?scheduler|wfp|lightweight.?filter|kernel.?debug|clash|v2ray|xray|sing-box|shadowsocks|ss-local|hysteria|trojan|naiveproxy|mihomo|surge|quantumult|loon|stash|surfboard|netch|proxifier|privoxy|\btor\b|i2p|tun2socks|tap-|tun0|wg0|utun|clash\.tun|clash\.tap|meta\.tun|sing\.tun|cloudflare.?warp|warp|本地连接|虚拟|伪|假|测试|模拟|隧道").expect("BL_REGEX compilation failed");
    static ref ADAPTER_CACHE: Mutex<Option<(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>, Instant)>> = Mutex::new(None);
}

const ADAPTER_CACHE_TTL_SECS: u64 = 5;

/// 适配器状态四分类（基于 IF_OPER_STATUS 枚举 + IP 是否为空）
/// - Disabled: 已禁用（OperStatus Down 或 NotPresent，管理员禁用或硬件缺失）
/// - Disconnected: 未连接（OperStatus LowerLayerDown 或 Dormant，线缆未插或等待外部事件）
/// - EnabledNoIp: 未禁用无IP（OperStatus Up 但无有效 IP，含 169.254 APIPA 清空后）
/// - Connected: 已连接（OperStatus Up 且有有效 IP）
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AdapterStatus {
    Disabled,
    Disconnected,
    EnabledNoIp,
    Connected,
}

impl AdapterStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            AdapterStatus::Disabled => "已禁用",
            AdapterStatus::Disconnected => "未连接",
            AdapterStatus::EnabledNoIp => "未禁用无IP",
            AdapterStatus::Connected => "已连接",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Adapter {
    pub name: String,
    pub ip: String,
    pub wireless: bool,
    pub guid: String,
    pub mac: String,
    pub if_index: u32,
    pub status: AdapterStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterDetail {
    pub name: String,
    pub ip: String,
    pub wireless: bool,
    pub subnet_mask: String,
    pub gateway: String,
    pub dhcp_server: String,
    pub mac: String,
    pub if_index: u32,
    pub status: AdapterStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisabledAdapter {
    pub name: String,
    pub status: String,
    pub description: String,
}

pub fn new_command(program: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);
    cmd
}

pub fn is_blacklisted(name: &str) -> bool {
    BL_REGEX.is_match(name)
}

#[cfg(target_os = "windows")]
fn is_visible_in_ncpa(guid: &str) -> bool {
    // 判断网卡是否在 Win11 高级网络设置 / ncpa.cpl 中可见
    // 严格按注册表 PnP 设备树检测，避免按名称误伤多物理网卡场景
    //
    // 注册表 1：HKLM\...\Control\Network\{4D36E972-...}\{GUID}\Connection\ShowInNetworkConnections
    //   = 0 → 用户/系统标记为隐藏
    //   = 1 或不存在 → Windows 默认显示
    //
    // 注册表 2：HKLM\SYSTEM\CurrentControlSet\Enum\<Enumerator>\<InstanceId>
    //   PnP 设备树中必须存在该 GUID 对应的实例，否则为"幽灵虚拟副本"（如 Wi-Fi Direct Virtual Adapter
    //   创建的多个 WLAN 2/3/4/5，这些在网络栈可见但 PnP 树中已被清理）
    //
    // 决策：注册表 1 + 2 都通过才视为可见
    //   - ShowInNetworkConnections 显式隐藏 → 不可见
    //   - PnP 树中找不到 InstanceId → 幽灵副本 → 不可见
    //   - 其他 → 可见
    if guid.is_empty() {
        return false;
    }
    // 注册表 1 检查：Connection 子键的 ShowInNetworkConnections
    let key_path = format!(
        "SYSTEM\\CurrentControlSet\\Control\\Network\\{{4D36E972-E325-11CE-BFC1-08002BE10318}}\\{}\\Connection",
        guid
    );
    let show_in_ncpa = match winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE).open_subkey(&key_path) {
        Ok(key) => {
            match key.get_value::<u32, _>("ShowInNetworkConnections") {
                Ok(val) => val != 0,
                Err(_) => true,
            }
        }
        Err(_) => true,  // Connection 子键缺失 → 视为可见，由 PnP 树检查把关
    };
    if !show_in_ncpa {
        return false;
    }
    // 注册表 2 检查：Class subkey 交叉验证
    // 真实物理网卡一定在 HKLM\...\Control\Class\{4D36E972-...}\<XXXX> 中有对应条目
    // （NetCfgInstanceId = 当前 GUID）
    // 幽灵虚拟副本（Wi-Fi Direct Virtual 多份副本）虽然 Connection 子键存在，
    // 但 Class subkey 中没有对应条目
    class_subkey_has_matching_guid(guid)
}

#[cfg(target_os = "windows")]
fn class_subkey_has_matching_guid(guid: &str) -> bool {
    if guid.is_empty() {
        return false;
    }
    let hklm = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
    let class_path = "SYSTEM\\CurrentControlSet\\Control\\Class\\{4D36E972-E325-11CE-BFC1-08002BE10318}";
    let class_key = match hklm.open_subkey(class_path) {
        Ok(k) => k,
        Err(_) => return false,
    };
    // 遍历 class subkey（0000-9999），查找 NetCfgInstanceId 匹配当前 GUID 的条目
    for sub in class_key.enum_keys().filter_map(|n| n.ok()) {
        if let Ok(sub_key) = class_key.open_subkey(&sub) {
            if let Ok(net_cfg_id) = sub_key.get_value::<String, _>("NetCfgInstanceId") {
                if net_cfg_id == guid {
                    return true;
                }
            }
        }
    }
    false
}

/// 判断适配器是否被管理员在设备管理器中手动禁用
/// 读 Class subkey 的 ConfigFlags，CONFIGFLAG_DISABLED (0x1) 表示手动禁用
/// 用于区分 NotPresent 状态下的"管理员禁用"vs"硬件缺失(USB未连接)"
#[cfg(target_os = "windows")]
fn is_admin_disabled_via_registry(guid: &str) -> bool {
    if guid.is_empty() {
        return false;
    }
    let hklm = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
    let class_path = "SYSTEM\\CurrentControlSet\\Control\\Class\\{4D36E972-E325-11CE-BFC1-08002BE10318}";
    let class_key = match hklm.open_subkey(class_path) {
        Ok(k) => k,
        Err(_) => return false,
    };
    for sub in class_key.enum_keys().filter_map(|n| n.ok()) {
        if let Ok(sub_key) = class_key.open_subkey(&sub) {
            if let Ok(net_cfg_id) = sub_key.get_value::<String, _>("NetCfgInstanceId") {
                if net_cfg_id == guid {
                    // 找到匹配条目，读 ConfigFlags
                    if let Ok(flags) = sub_key.get_value::<u32, _>("ConfigFlags") {
                        return flags & 0x1 != 0;  // CONFIGFLAG_DISABLED
                    }
                    return false;  // ConfigFlags 不存在，视为未禁用
                }
            }
        }
    }
    false
}



fn prefix_len_to_mask(len: u32) -> String {
    if len > 32 { return String::new(); }
    let mask: u32 = if len == 0 { 0 } else { !0u32 << (32 - len) };
    format!(
        "{}.{}.{}.{}",
        (mask >> 24) & 0xFF,
        (mask >> 16) & 0xFF,
        (mask >> 8) & 0xFF,
        mask & 0xFF,
    )
}

#[cfg(target_os = "windows")]
fn query_adapters_addresses() -> Result<(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>), String> {
    use windows::Win32::NetworkManagement::IpHelper::*;
    use windows::Win32::Networking::WinSock::*;

    const GAA_FLAGS: GET_ADAPTERS_ADDRESSES_FLAGS = GET_ADAPTERS_ADDRESSES_FLAGS(0x0080 | 0x0100);
    const IF_TYPE_ETHERNET_CSMACD: u32 = 6;
    const IF_TYPE_IEEE80211: u32 = 71;

    let mut size: u32 = 0;
    unsafe {
        GetAdaptersAddresses(AF_INET.0 as u32, GAA_FLAGS, None, None, &mut size);
    };

    if size == 0 {
        return Ok((vec![], vec![], vec![]));
    }

    let max_retries = 3;
    for attempt in 0..max_retries {
        let buffer_size = if attempt == 0 { size as usize } else { (size as usize) + 4096 };
        let mut buffer = vec![0u8; buffer_size];
        let ptr = buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;
        let mut actual_size = buffer_size as u32;

        let result = unsafe {
            GetAdaptersAddresses(AF_INET.0 as u32, GAA_FLAGS, None, Some(ptr), &mut actual_size)
        };

        if result == 0 {
            return parse_adapter_addresses(ptr, IF_TYPE_ETHERNET_CSMACD, IF_TYPE_IEEE80211);
        }

        if result == 111 || actual_size as usize > buffer_size {
            size = actual_size;
            if attempt < max_retries - 1 {
                continue;
            }
            return Err(format!("GetAdaptersAddresses buffer too small after {} retries", max_retries));
        }

        if attempt < max_retries - 1 {
            unsafe {
                GetAdaptersAddresses(AF_INET.0 as u32, GAA_FLAGS, None, None, &mut size);
            }
            continue;
        }

        return Err(format!("GetAdaptersAddresses failed: {}", result));
    }

    Ok((vec![], vec![], vec![]))
}

#[cfg(target_os = "windows")]
fn parse_adapter_addresses(
    ptr: *mut windows::Win32::NetworkManagement::IpHelper::IP_ADAPTER_ADDRESSES_LH,
    if_type_ethernet: u32,
    if_type_wireless: u32,
) -> Result<(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>), String> {
    use windows::Win32::NetworkManagement::Ndis::{
        IfOperStatusUp, IfOperStatusNotPresent,
    };
    use windows::Win32::Networking::WinSock::*;

    let mut adapters = Vec::new();
    let mut details = Vec::new();
    let mut disabled = Vec::new();

    let mut current = ptr;
    while !current.is_null() {
        let addr = unsafe { &*current };

        let name = unsafe { read_pwstr(addr.FriendlyName) };

        let guid_raw = unsafe {
            if addr.AdapterName.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(addr.AdapterName.0 as *const i8)
                    .to_string_lossy()
                    .into_owned()
            }
        };
        let guid = if guid_raw.starts_with('{') {
            guid_raw
        } else if !guid_raw.is_empty() {
            format!("{{{}}}", guid_raw)
        } else {
            guid_raw
        };

        let if_type = addr.IfType;
        if if_type != if_type_ethernet && if_type != if_type_wireless {
            current = addr.Next;
            continue;
        }

        if addr.PhysicalAddressLength == 0 {
            current = addr.Next;
            continue;
        }

        let is_up = addr.OperStatus == IfOperStatusUp;
        // NotPresent（已禁用）适配器不跳过，进入 else 分支加入 disabled 列表

        if !is_visible_in_ncpa(&guid) {
            current = addr.Next;
            continue;
        }

        let description = unsafe { read_pwstr(addr.Description) };

        if is_blacklisted(&name) || is_blacklisted(&description) {
            current = addr.Next;
            continue;
        }

        let is_wireless = if_type == if_type_wireless;
        let if_index = unsafe { addr.Anonymous1.Anonymous.IfIndex };

        let mac = if addr.PhysicalAddressLength >= 6 {
            let bytes = unsafe { std::slice::from_raw_parts(addr.PhysicalAddress.as_ptr(), 6) };
            format!("{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}", bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5])
        } else {
            String::new()
        };

        let oper_status = addr.OperStatus;
        // 计算 IP / prefix_len（仅 is_up 时尝试拿，否则保持空）
        let mut ip = String::new();
        let mut prefix_len: u8 = 0;
        if is_up {
            let mut ua = addr.FirstUnicastAddress;
            while !ua.is_null() {
                let u = unsafe { &*ua };
                if u.Address.lpSockaddr.is_null() {
                    ua = unsafe { (*ua).Next };
                    continue;
                }
                let sa = unsafe { &*u.Address.lpSockaddr };
                if sa.sa_family == AF_INET {
                    let sin = unsafe { &*(u.Address.lpSockaddr as *const SOCKADDR_IN) };
                    ip = unsafe { ipv4_from_in_addr(sin.sin_addr) };
                    prefix_len = u.OnLinkPrefixLength;
                    break;
                }
                ua = u.Next;
            }
            // 169.254 APIPA 视为无 IP（DHCP 失败的自配地址）
            if ip.starts_with("169.254.") {
                ip.clear();
            }
        }

        // 网关 / DHCP 服务器（仅 is_up 时查询，否则保持空）
        let mut gateway = String::new();
        let mut dhcp_server = String::new();
        if is_up {
            let mut ga = addr.FirstGatewayAddress;
            while !ga.is_null() {
                let g = unsafe { &*ga };
                if g.Address.lpSockaddr.is_null() {
                    ga = unsafe { (*ga).Next };
                    continue;
                }
                let sa = unsafe { &*g.Address.lpSockaddr };
                if sa.sa_family == AF_INET {
                    let sin = unsafe { &*(g.Address.lpSockaddr as *const SOCKADDR_IN) };
                    gateway = unsafe { ipv4_from_in_addr(sin.sin_addr) };
                    break;
                }
                ga = g.Next;
            }

            let dhcp_sa = addr.Dhcpv4Server;
            if !dhcp_sa.lpSockaddr.is_null() {
                let sa = unsafe { &*dhcp_sa.lpSockaddr };
                if sa.sa_family == AF_INET {
                    let sin = unsafe { &*(dhcp_sa.lpSockaddr as *const SOCKADDR_IN) };
                    dhcp_server = unsafe { ipv4_from_in_addr(sin.sin_addr) };
                }
            }
        }

        // 严格四分类判定
        let status = if is_up {
            if ip.is_empty() {
                AdapterStatus::EnabledNoIp
            } else {
                AdapterStatus::Connected
            }
        } else if oper_status == IfOperStatusNotPresent {
            // NotPresent 可能是管理员禁用或硬件缺失(USB未连接)
            // 用 ConfigFlags 区分：CONFIGFLAG_DISABLED (0x1) 才是管理员禁用
            if is_admin_disabled_via_registry(&guid) {
                AdapterStatus::Disabled
            } else {
                // USB 网卡未连接 / 硬件缺失 / 驱动未加载
                AdapterStatus::Disconnected
            }
        } else {
            // Down / LowerLayerDown / Dormant / Unknown / Testing 归为未连接
            // Down 在 Windows 上实际语义是"接口未就绪"（媒体断开/未认证），不是管理员禁用
            AdapterStatus::Disconnected
        };

        // 所有适配器都推入 adapters 列表（带状态，便于前端统一展示和启用操作）
        adapters.push(Adapter {
            name: name.clone(),
            ip: ip.clone(),
            wireless: is_wireless,
            guid: guid.clone(),
            mac: mac.clone(),
            if_index,
            status,
        });

        // Connected 和 EnabledNoIp 状态推入 details（EnabledNoIp 保留 dhcp_server 供诊断）
        // 仅 Disabled 状态推入 disabled（保留 DisabledAdapter 兼容旧 API）
        match status {
            AdapterStatus::Connected => {
                details.push(AdapterDetail {
                    name,
                    ip,
                    wireless: is_wireless,
                    subnet_mask: prefix_len_to_mask(prefix_len as u32),
                    gateway,
                    dhcp_server,
                    mac,
                    if_index,
                    status,
                });
            }
            AdapterStatus::EnabledNoIp => {
                details.push(AdapterDetail {
                    name,
                    ip: String::new(),
                    wireless: is_wireless,
                    subnet_mask: String::new(),
                    gateway,
                    dhcp_server,
                    mac,
                    if_index,
                    status,
                });
            }
            AdapterStatus::Disabled => {
                disabled.push(DisabledAdapter {
                    name,
                    status: status.as_str().to_string(),
                    description,
                });
            }
            _ => {}
        }

        current = addr.Next;
    }

    Ok((adapters, details, disabled))
}

#[cfg(target_os = "windows")]
unsafe fn read_pwstr(ptr: windows::core::PWSTR) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let pcwstr = windows::core::PCWSTR(ptr.0 as *const u16);
    pcwstr.to_string().unwrap_or_else(|_| {
        let mut len = 0;
        let max_len = 4096;
        while len < max_len && *ptr.0.add(len) != 0 {
            len += 1;
        }
        if len == max_len {
            crate::log_warn!("network", "read_pwstr: 适配器名称超过{}个UTF-16单元，跳过该适配器", max_len);
            return String::new();
        }
        if len == 0 {
            return String::new();
        }
        let slice = std::slice::from_raw_parts(ptr.0, len);
        String::from_utf16_lossy(slice)
    })
}

#[cfg(target_os = "windows")]
unsafe fn ipv4_from_in_addr(addr: windows::Win32::Networking::WinSock::IN_ADDR) -> String {
    std::net::Ipv4Addr::from(addr).to_string()
}

fn query_adapters_cached_inner() -> Result<(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>), String> {
    {
        let cache = ADAPTER_CACHE.lock();
        if let Some((adapters, details, disabled, ts)) = cache.as_ref() {
            if ts.elapsed().as_secs() < ADAPTER_CACHE_TTL_SECS {
                return Ok((adapters.clone(), details.clone(), disabled.clone()));
            }
        }
    }
    let result = query_adapters_addresses()?;
    {
        let mut cache = ADAPTER_CACHE.lock();
        *cache = Some((result.0.clone(), result.1.clone(), result.2.clone(), Instant::now()));
    }
    Ok(result)
}

pub fn get_all_adapters_force() -> Result<(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>), String> {
    ADAPTER_CACHE.lock().take();
    query_adapters_cached_inner()
}

pub fn get_adapters_cached() -> Result<Vec<Adapter>, String> {
    let (adapters, _, _) = query_adapters_cached_inner()?;
    Ok(adapters)
}

/// 异步版本的 get_adapters_cached，供 async 上下文调用。
///
/// 快速路径：缓存命中时仅 Mutex lock + clone（非阻塞），直接返回，避免 spawn_blocking 开销。
/// 慢路径：缓存未命中时，通过 spawn_blocking 把阻塞的 Win32 GetAdaptersAddresses 调用
///        转移到阻塞线程池，避免阻塞 async 运行时。
/// 注意：spawn_blocking 内部会再次调用 get_adapters_cached，其内部 query_adapters_cached_inner
///       会二次检查缓存（可能已被其他线程填充），命中即返回，未命中才真正调用 Win32。
pub async fn get_adapters_cached_async() -> Result<Vec<Adapter>, String> {
    // 快速路径：缓存命中直接返回（仅 Mutex lock + clone，非阻塞）
    {
        let cache = ADAPTER_CACHE.lock();
        if let Some((adapters, _details, _disabled, ts)) = cache.as_ref() {
            if ts.elapsed().as_secs() < ADAPTER_CACHE_TTL_SECS {
                return Ok(adapters.clone());
            }
        }
    }
    // 慢路径：缓存未命中，spawn_blocking 执行阻塞的 Win32 GetAdaptersAddresses 调用
    tokio::task::spawn_blocking(get_adapters_cached)
        .await
        .map_err(|e| format!("适配器查询任务失败: {}", e))?
}

pub fn get_disabled_adapters_cached() -> Result<Vec<DisabledAdapter>, String> {
    let (_, _, disabled) = query_adapters_cached_inner()?;
    Ok(disabled)
}

pub fn get_adapters_force() -> Result<Vec<Adapter>, String> {
    ADAPTER_CACHE.lock().take();
    get_adapters_cached()
}

pub fn validate_adapter_name(name: &str) -> Result<(), String> {
    if name.is_empty() { return Err("适配器名称不能为空".to_string()); }
    if name.len() > 128 { return Err("适配器名称过长".to_string()); }
    const FORBIDDEN: &[char] = &['&', '|', ';', '`', '$', '(', ')', '<', '>', '"', '\'', '\n', '\r', '\0'];
    if name.chars().any(|c| FORBIDDEN.contains(&c)) { return Err("适配器名称包含非法字符".to_string()); }
    Ok(())
}

pub fn enable_adapter(adapter_name: &str) -> Result<(), String> {
    validate_adapter_name(adapter_name)?;

    // netsh 命令行参数（适配器名含空格时需双引号包裹）
    let netsh_args = format!("interface set interface \"{}\" enable", adapter_name);

    if crate::platform::elevation::is_admin() {
        // 管理员：直接执行 netsh
        crate::log_info!("adapter", "管理员直写启用适配器: {}", adapter_name);
        let output = new_command("netsh")
            .args(["interface", "set", "interface", adapter_name, "enable"])
            .output()
            .map_err(|e| format!("启用适配器失败: {}", e))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stderr_trimmed = stderr.trim();
            return Err(if stderr_trimmed.is_empty() {
                "启用适配器失败：netsh 返回非零退出码但未输出错误信息".to_string()
            } else {
                format!("启用适配器失败: {}", stderr_trimmed)
            });
        }
    } else {
        // 非管理员：COM 静默提权执行 netsh（不弹 UAC）
        crate::log_info!("adapter", "非管理员运行，COM ShellExec 提权启用适配器: {}", adapter_name);
        match crate::platform::elevation::shell_exec_elevated("netsh", &netsh_args, true) {
            Ok(()) => {
                crate::log_info!("adapter", "COM ShellExec 提权启用适配器成功: {}", adapter_name);
            }
            Err(com_err) => {
                // COM 失败：降级 ShellExecuteW runas（会弹 UAC）
                crate::log_warn!("adapter", "COM ShellExec 失败: {}，降级到 ShellExecuteW runas", com_err);
                crate::platform::elevation::run_elevated("netsh", &netsh_args)
                    .map_err(|e| format!("提权启用适配器失败（COM 和 UAC 均失败）: COM错误={}; UAC错误={}", com_err, e))?;
                crate::log_info!("adapter", "ShellExecuteW runas 启用适配器成功: {}", adapter_name);
            }
        }
    }

    // 启用后强制清缓存，让下次查询拿到最新状态
    ADAPTER_CACHE.lock().take();
    crate::log_info!("adapter", "已清空适配器缓存");

    Ok(())
}

pub fn get_adapter_details_cached() -> Result<Vec<AdapterDetail>, String> {
    let (_, details, _) = query_adapters_cached_inner()?;
    Ok(details)
}

pub fn resolve_adapter_names(adapters: &[Adapter], config: &crate::config::Config) -> (String, String) {
    // 自动检测：优先选有线网卡，其次任意有 IP 的网卡，最后任意第一个
    let auto_detect_a1 = || -> String {
        adapters.iter()
            .find(|a| !a.wireless && !a.ip.is_empty())
            .or_else(|| adapters.iter().find(|a| !a.ip.is_empty()))
            .or_else(|| adapters.first())
            .map(|a| a.name.clone())
            .unwrap_or_default()
    };

    let adapter1 = if config.adapter1.is_empty() || config.adapter1 == crate::config::model::AUTO_DETECT_ADAPTER {
        auto_detect_a1()
    } else if adapters.iter().any(|a| a.name == config.adapter1) {
        config.adapter1.clone()
    } else {
        // 配置名不在过滤后的可见列表中：降级到自动检测
        // 防止用户在 pre-1709 Win10 上配置"本地连接"等已被黑名单过滤的网卡后静默选错
        crate::log_warn!(
            "network",
            "配置中的 adapter1 '{}' 不在当前可见适配器列表中，降级到自动检测",
            config.adapter1
        );
        auto_detect_a1()
    };

    let adapter2 = if config.dual_adapter {
        if config.adapter2.is_empty() || config.adapter2 == crate::config::model::AUTO_DETECT_ADAPTER {
            adapters.iter()
                .find(|a| a.name != adapter1 && !a.wireless && !a.ip.is_empty())
                .or_else(|| adapters.iter().find(|a| a.name != adapter1 && !a.ip.is_empty()))
                .or_else(|| adapters.iter().find(|a| a.name != adapter1))
                .map(|a| a.name.clone())
                .unwrap_or_default()
        } else if adapters.iter().any(|a| a.name == config.adapter2) {
            config.adapter2.clone()
        } else {
            crate::log_warn!(
                "network",
                "配置中的 adapter2 '{}' 不在当前可见适配器列表中，降级到自动检测",
                config.adapter2
            );
            adapters.iter()
                .find(|a| a.name != adapter1 && !a.wireless && !a.ip.is_empty())
                .or_else(|| adapters.iter().find(|a| a.name != adapter1 && !a.ip.is_empty()))
                .or_else(|| adapters.iter().find(|a| a.name != adapter1))
                .map(|a| a.name.clone())
                .unwrap_or_default()
        }
    } else {
        String::new()
    };

    (adapter1, adapter2)
}

pub fn select_adapter(adapters: &[Adapter], config: &crate::config::Config) -> (String, String) {
    if adapters.is_empty() { return (String::new(), String::new()); }

    if !config.adapter1.is_empty() && config.adapter1 != "自动检测" {
        if let Some(a) = adapters.iter().find(|a| a.name == config.adapter1 && !a.ip.is_empty()) {
            return (a.ip.clone(), a.name.clone());
        }
    }

    if let Some(wired) = adapters.iter().find(|a| !a.ip.is_empty() && !a.wireless) {
        return (wired.ip.clone(), wired.name.clone());
    }

    if let Some(with_ip) = adapters.iter().find(|a| !a.ip.is_empty()) {
        return (with_ip.ip.clone(), with_ip.name.clone());
    }

    (String::new(), String::new())
}

pub fn dhcp_renew(adapter_name: &str) -> Result<bool, String> {
    validate_adapter_name(adapter_name)?;
    let output = new_command("ipconfig")
        .args(["/renew", adapter_name])
        .output()
        .map_err(|e| format!("DHCP续租失败: {}", e))?;
    Ok(output.status.success())
}

pub fn dhcp_release(adapter_name: &str) -> Result<bool, String> {
    validate_adapter_name(adapter_name)?;
    let output = new_command("ipconfig")
        .args(["/release", adapter_name])
        .output()
        .map_err(|e| format!("DHCP释放失败: {}", e))?;
    Ok(output.status.success())
}

pub fn dhcp_renew_wired_only() -> Result<Vec<serde_json::Value>, String> {
    let adapters = get_adapters_cached()?;
    let wired: Vec<&Adapter> = adapters.iter().filter(|a| !a.wireless).collect();
    if wired.is_empty() { return Ok(vec![]); }

    let mut results = Vec::new();
    for adapter in wired {
        let success = match dhcp_renew(&adapter.name) {
            Ok(s) => s,
            Err(e) => {
                crate::log_warn!("adapter", "DHCP续租失败({}): {}", adapter.name, e);
                false
            }
        };
        results.push(serde_json::json!({
            "name": adapter.name,
            "success": success
        }));
    }
    Ok(results)
}

static MAC_SEED_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn generate_random_mac() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let counter = MAC_SEED_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let seed = time.wrapping_add(counter.wrapping_mul(0x9E3779B97F4A7C15)); // 黄金比例混合
    // 使用简单的线性同余生成器替代 _rdtsc，避免平台依赖
    let mut rng = seed;
    let mut next = || { rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); rng };
    let b1 = (next() & 0xFF) as u8;
    let b2 = (next() & 0xFF) as u8;
    let b3 = (next() & 0xFF) as u8;
    let b4 = (next() & 0xFF) as u8;
    let b5 = (next() & 0xFF) as u8;
    let b6 = (next() & 0xFF) as u8;
    let first = (b1 & 0xFC) | 0x02;
    format!("{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}", first, b2, b3, b4, b5, b6)
}

fn mac_with_dashes(mac: &str) -> String {
    mac.as_bytes()
        .chunks(2)
        .filter_map(|c| std::str::from_utf8(c).ok())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(target_os = "windows")]
fn is_access_denied(e: &std::io::Error) -> bool {
    e.raw_os_error() == Some(5)
}

#[cfg(target_os = "windows")]
pub fn set_mac_via_registry(adapter_guid: &str, mac_no_dash: &str) -> Result<(), String> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_ALL_ACCESS};
    let class_path = r"SYSTEM\CurrentControlSet\Control\Class\{4D36E972-E325-11CE-BFC1-08002BE10318}";
    let hklm = winreg::RegKey::predef(HKEY_LOCAL_MACHINE);
    let class_key = hklm.open_subkey_with_flags(class_path, KEY_ALL_ACCESS)
        .map_err(|e| {
            if is_access_denied(&e) {
                "修改MAC地址需要管理员权限，请以管理员身份运行应用".to_string()
            } else {
                format!("打开网卡注册表失败: {}", e)
            }
        })?;
    for subkey_name in class_key.enum_keys().filter_map(|r| r.ok()) {
        if let Ok(subkey) = class_key.open_subkey_with_flags(&subkey_name, KEY_ALL_ACCESS) {
            if let Ok(instance_id) = subkey.get_value::<String, _>("NetCfgInstanceId") {
                if instance_id.eq_ignore_ascii_case(adapter_guid) {
                    subkey.set_value("NetworkAddress", &mac_no_dash)
                        .map_err(|e| format!("写入NetworkAddress失败: {}", e))?;
                    return Ok(());
                }
            }
        }
    }
    Err("未找到适配器注册表项".to_string())
}

#[cfg(target_os = "windows")]
pub fn remove_mac_from_registry(adapter_guid: &str) -> Result<(), String> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_ALL_ACCESS};
    let class_path = r"SYSTEM\CurrentControlSet\Control\Class\{4D36E972-E325-11CE-BFC1-08002BE10318}";
    let hklm = winreg::RegKey::predef(HKEY_LOCAL_MACHINE);
    let class_key = hklm.open_subkey_with_flags(class_path, KEY_ALL_ACCESS)
        .map_err(|e| {
            if is_access_denied(&e) {
                "清理MAC地址需要管理员权限".to_string()
            } else {
                format!("打开网卡注册表失败: {}", e)
            }
        })?;
    for subkey_name in class_key.enum_keys().filter_map(|r| r.ok()) {
        if let Ok(subkey) = class_key.open_subkey_with_flags(&subkey_name, KEY_ALL_ACCESS) {
            if let Ok(instance_id) = subkey.get_value::<String, _>("NetCfgInstanceId") {
                if instance_id.eq_ignore_ascii_case(adapter_guid) {
                    if let Err(e) = subkey.delete_value("NetworkAddress") {
                        crate::log_warn!("adapter", "清理MAC地址注册表项失败(guid={}): {}", adapter_guid, e);
                    }
                    return Ok(());
                }
            }
        }
    }
    Err("未找到适配器注册表项".to_string())
}

pub fn netsh_disable(adapter_name: &str) -> bool {
    if validate_adapter_name(adapter_name).is_err() {
        return false;
    }
    // netsh语法要求 "name=适配器名" 作为单个参数传递，无法拆分为独立args
    new_command("netsh")
        .args(["interface", "set", "interface", &format!("name={}", adapter_name), "admin=disable"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn netsh_enable(adapter_name: &str) -> bool {
    if validate_adapter_name(adapter_name).is_err() {
        return false;
    }
    // netsh语法要求 "name=适配器名" 作为单个参数传递，无法拆分为独立args
    new_command("netsh")
        .args(["interface", "set", "interface", &format!("name={}", adapter_name), "admin=enable"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn poll_ip_change(adapter_name: &str, old_ip: &str, timeout_ms: u64) -> Option<String> {
    let start = std::time::Instant::now();
    let interval = std::time::Duration::from_millis(300);
    let timeout = std::time::Duration::from_millis(timeout_ms);
    while start.elapsed() < timeout {
        if let Ok(adapters) = get_adapters_force() {
            if let Some(a) = adapters.iter().find(|a| a.name == adapter_name) {
                if !a.ip.is_empty() && a.ip != old_ip {
                    return Some(a.ip.clone());
                }
            }
        }
        std::thread::sleep(interval);
    }
    None
}

pub fn poll_adapter_has_ip(adapter_name: &str, timeout_ms: u64) -> bool {
    let start = std::time::Instant::now();
    let interval = std::time::Duration::from_millis(300);
    let timeout = std::time::Duration::from_millis(timeout_ms);
    while start.elapsed() < timeout {
        if let Ok(adapters) = get_adapters_force() {
            if let Some(a) = adapters.iter().find(|a| a.name == adapter_name) {
                if !a.ip.is_empty() {
                    return true;
                }
            }
        }
        std::thread::sleep(interval);
    }
    false
}

pub fn escape_ps_single_quote(s: &str) -> String {
    s.replace("'", "''")
}

fn try_elevated_mac_script(adapter_name: &str, _guid: &str, mac_no_dash: &str, old_ip: &str) -> (bool, Option<String>) {
    let mac_dashed = mac_with_dashes(mac_no_dash);
    let script = format!(
        "$name='{name}';$mac='{mac}';\
         Set-NetAdapter -Name $name -MacAddress $mac -Confirm:$false -ErrorAction Stop;\
         ipconfig /release $name;\
         Start-Sleep -Seconds 1;\
         ipconfig /renew $name",
        mac = mac_dashed, name = escape_ps_single_quote(adapter_name)
    );
    crate::log_info!("adapter", "尝试提权修改MAC(Set-NetAdapter): adapter={}, mac={}", adapter_name, mac_dashed);
    match crate::platform::elevation::run_elevated("powershell", &format!("-WindowStyle Hidden -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command \"{}\"", script)) {
        Ok(()) => {
            crate::log_info!("adapter", "提权脚本已启动，等待IP变更...");
            if let Some(changed_ip) = poll_ip_change(adapter_name, old_ip, 25_000) {
                crate::log_info!("adapter", "提权修改MAC成功: 新IP={}", changed_ip);
                (true, None)
            } else {
                crate::log_warn!("adapter", "提权修改MAC超时: 25秒内IP未变更");
                (false, Some("提权脚本已执行但IP未变更，可能网卡驱动不支持MAC伪装".to_string()))
            }
        }
        Err(e) => {
            crate::log_warn!("adapter", "提权执行MAC修改失败: {}", e);
            (false, Some(format!("提权失败: {}，请尝试以管理员身份运行应用", e)))
        }
    }
}

pub fn dhcp_release_renew_all(campus_gateway: &str) -> Result<Vec<serde_json::Value>, String> {
    if campus_gateway.is_empty() {
        return Err("校园网网关为空，无法判断子网".to_string());
    }
    let adapters = get_adapters_cached()?;
    if adapters.is_empty() { return Ok(vec![]); }

    let mut results = Vec::new();
    for adapter in &adapters {
        if !adapter.ip.is_empty() && !is_same_subnet_18(&adapter.ip, campus_gateway) {
            results.push(serde_json::json!({
                "name": adapter.name,
                "wireless": adapter.wireless,
                "ip": adapter.ip,
                "success": false,
                "skipped": true,
                "reason": "非校园网子网，跳过"
            }));
            continue;
        }

        let fake_mac = generate_random_mac();
        let mac_dashed = mac_with_dashes(&fake_mac);

        let (reg_ok, elevated_done, elevate_msg) = if crate::platform::elevation::is_admin() {
            match set_mac_via_registry(&adapter.guid, &fake_mac) {
                Ok(()) => {
                    crate::log_info!("adapter", "管理员直写注册表成功: guid={}", adapter.guid);
                    (true, false, None)
                }
                Err(e) => (false, false, Some(format!("MAC地址修改失败: {}", e))),
            }
        } else {
            crate::log_info!("adapter", "非管理员运行，跳过注册表直写，直接COM ShellExec提权: guid={}", adapter.guid);
            let ps_cmd = format!(
                "-WindowStyle Hidden -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command \"Set-NetAdapter -Name '{}' -MacAddress '{}' -Confirm:$false; ipconfig /release '{}'; Start-Sleep -Seconds 1; ipconfig /renew '{}'\"",
                escape_ps_single_quote(&adapter.name), mac_dashed, escape_ps_single_quote(&adapter.name), escape_ps_single_quote(&adapter.name)
            );
            match crate::platform::elevation::shell_exec_elevated("powershell", &ps_cmd, true) {
                Ok(()) => {
                    crate::log_info!("adapter", "COM ShellExec提权成功，等待IP变更...");
                    if let Some(changed_ip) = poll_ip_change(&adapter.name, &adapter.ip, 25_000) {
                        crate::log_info!("adapter", "COM提权修改MAC成功: 新IP={}", changed_ip);
                        (true, true, None)
                    } else {
                        crate::log_warn!("adapter", "COM提权修改MAC超时: 25秒内IP未变更");
                        (true, true, Some("COM提权已执行但IP未变更，可能网卡驱动不支持MAC伪装".to_string()))
                    }
                }
                Err(com_err) => {
                    crate::log_warn!("adapter", "COM ShellExec失败: {}，降级到ShellExecuteW", com_err);
                    let (ok, msg) = try_elevated_mac_script(&adapter.name, &adapter.guid, &fake_mac, &adapter.ip);
                    (ok, ok, msg)
                }
            }
        };

        let old_ip = adapter.ip.clone();
        let mut new_ip = old_ip.clone();
        let mut ip_changed = false;
        let mut message: Option<String> = elevate_msg;

        if !reg_ok {
            if let Err(e) = dhcp_release(&adapter.name) {
                crate::log_warn!("adapter", "DHCP释放失败({}): {}", adapter.name, e);
            }
            if let Err(e) = dhcp_renew(&adapter.name) {
                crate::log_warn!("adapter", "DHCP续租失败({}): {}", adapter.name, e);
            }
            if message.is_none() {
                message = Some("MAC地址修改失败，仅执行了DHCP释放/续租".to_string());
            }
        } else if elevated_done {
            // 提权脚本通过 Set-NetAdapter 修改了 MAC（写入注册表 NetworkAddress 键值，跨重启持久化）
            // 脚本本身未清理注册表，这里补做清理
            if let Ok(refreshed) = get_adapters_force() {
                if let Some(a) = refreshed.iter().find(|a| a.name == adapter.name) {
                    if !a.ip.is_empty() {
                        new_ip = a.ip.clone();
                        ip_changed = new_ip != old_ip;
                    }
                }
            }
            if let Err(e) = remove_mac_from_registry(&adapter.guid) {
                crate::log_warn!("adapter", "清理MAC注册表失败({}): {}", adapter.guid, e);
            }
            if !ip_changed && message.is_none() {
                message = Some("提权脚本已执行但IP未变更，可能网卡驱动不支持MAC伪装".to_string());
            }
        } else {
            if let Err(e) = dhcp_release(&adapter.name) {
                crate::log_warn!("adapter", "DHCP释放失败({}): {}", adapter.name, e);
            }
            let disable_ok = netsh_disable(&adapter.name);
            if disable_ok {
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            let enable_ok = netsh_enable(&adapter.name);
            if enable_ok {
                poll_adapter_has_ip(&adapter.name, 3000);
            }
            let renew_ok = match dhcp_renew(&adapter.name) {
                Ok(s) => s,
                Err(e) => {
                    crate::log_warn!("adapter", "DHCP续租失败({}): {}", adapter.name, e);
                    false
                }
            };
            if renew_ok {
                if let Some(changed_ip) = poll_ip_change(&adapter.name, &old_ip, 5000) {
                    new_ip = changed_ip;
                    ip_changed = true;
                } else if let Ok(refreshed) = get_adapters_force() {
                    if let Some(a) = refreshed.iter().find(|a| a.name == adapter.name) {
                        if !a.ip.is_empty() {
                            new_ip = a.ip.clone();
                            ip_changed = new_ip != old_ip;
                        }
                    }
                }
            }
            if let Err(e) = remove_mac_from_registry(&adapter.guid) {
                crate::log_warn!("adapter", "清理MAC注册表失败({}): {}", adapter.guid, e);
            }
            if !ip_changed && message.is_none() {
                message = Some("MAC已修改但IP未变更，可能网卡驱动不支持MAC伪装或DHCP服务器分配了相同IP".to_string());
            }
        }

        results.push(serde_json::json!({
            "name": adapter.name,
            "wireless": adapter.wireless,
            "ip": new_ip,
            "regOk": reg_ok,
            "success": ip_changed,
            "skipped": false,
            "reason": message
        }));
    }
    Ok(results)
}

pub fn dhcp_release_renew_single(adapter_name: &str, campus_gateway: &str) -> Result<serde_json::Value, String> {
    let adapters = get_adapters_cached()?;
    let adapter = adapters.iter().find(|a| a.name == adapter_name)
        .ok_or_else(|| format!("未找到适配器: {}", adapter_name))?;

    if !adapter.ip.is_empty() && !is_same_subnet_18(&adapter.ip, campus_gateway) {
        return Ok(serde_json::json!({
            "name": adapter.name,
            "wireless": adapter.wireless,
            "ip": adapter.ip,
            "success": false,
            "skipped": true,
            "reason": "非校园网子网，跳过"
        }));
    }

    let fake_mac = generate_random_mac();
    let mac_dashed = mac_with_dashes(&fake_mac);

    let (reg_ok, elevated_done, elevate_msg) = if crate::platform::elevation::is_admin() {
        match set_mac_via_registry(&adapter.guid, &fake_mac) {
            Ok(()) => {
                crate::log_info!("adapter", "管理员直写注册表成功: guid={}", adapter.guid);
                (true, false, None)
            }
            Err(e) => (false, false, Some(format!("MAC地址修改失败: {}", e))),
        }
    } else {
        crate::log_info!("adapter", "非管理员运行，跳过注册表直写，直接COM ShellExec提权: guid={}", adapter.guid);
        let ps_cmd = format!(
            "-WindowStyle Hidden -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command \"Set-NetAdapter -Name '{}' -MacAddress '{}' -Confirm:$false; ipconfig /release '{}'; Start-Sleep -Seconds 1; ipconfig /renew '{}'\"",
            escape_ps_single_quote(&adapter.name), mac_dashed, escape_ps_single_quote(&adapter.name), escape_ps_single_quote(&adapter.name)
        );
        match crate::platform::elevation::shell_exec_elevated("powershell", &ps_cmd, true) {
            Ok(()) => {
                crate::log_info!("adapter", "COM ShellExec提权成功，等待IP变更...");
                if let Some(changed_ip) = poll_ip_change(&adapter.name, &adapter.ip, 25_000) {
                    crate::log_info!("adapter", "COM提权修改MAC成功: 新IP={}", changed_ip);
                    (true, true, None)
                } else {
                    crate::log_warn!("adapter", "COM提权修改MAC超时: 25秒内IP未变更");
                    (true, true, Some("COM提权已执行但IP未变更，可能网卡驱动不支持MAC伪装".to_string()))
                }
            }
            Err(com_err) => {
                crate::log_warn!("adapter", "COM ShellExec失败: {}，降级到ShellExecuteW", com_err);
                let (ok, msg) = try_elevated_mac_script(&adapter.name, &adapter.guid, &fake_mac, &adapter.ip);
                (ok, ok, msg)
            }
        }
    };

    let old_ip = adapter.ip.clone();
    let mut new_ip = old_ip.clone();
    let mut ip_changed = false;
    let mut message: Option<String> = elevate_msg;

    if !reg_ok {
        let _ = dhcp_release(&adapter.name);
        let _ = dhcp_renew(&adapter.name);
        if message.is_none() {
            message = Some("MAC地址修改失败，仅执行了DHCP释放/续租".to_string());
        }
    } else if elevated_done {
        // 提权脚本通过 Set-NetAdapter 修改了 MAC（写入注册表 NetworkAddress 键值，跨重启持久化）
        // 脚本本身未清理注册表，这里补做清理
        if let Ok(refreshed) = get_adapters_force() {
            if let Some(a) = refreshed.iter().find(|a| a.name == adapter.name) {
                if !a.ip.is_empty() {
                    new_ip = a.ip.clone();
                    ip_changed = new_ip != old_ip;
                }
            }
        }
        let _ = remove_mac_from_registry(&adapter.guid);
        if !ip_changed && message.is_none() {
            message = Some("提权脚本已执行但IP未变更，可能网卡驱动不支持MAC伪装".to_string());
        }
    } else {
        let _ = dhcp_release(&adapter.name);
        let disable_ok = netsh_disable(&adapter.name);
        if disable_ok {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        let enable_ok = netsh_enable(&adapter.name);
        if enable_ok {
            poll_adapter_has_ip(&adapter.name, 3000);
        }
        let renew_ok = dhcp_renew(&adapter.name).unwrap_or(false);
        if renew_ok {
            if let Some(changed_ip) = poll_ip_change(&adapter.name, &old_ip, 5000) {
                new_ip = changed_ip;
                ip_changed = true;
            } else if let Ok(refreshed) = get_adapters_force() {
                if let Some(a) = refreshed.iter().find(|a| a.name == adapter.name) {
                    if !a.ip.is_empty() {
                        new_ip = a.ip.clone();
                        ip_changed = new_ip != old_ip;
                    }
                }
            }
        }
        let _ = remove_mac_from_registry(&adapter.guid);
        if !ip_changed && message.is_none() {
            message = Some("MAC已修改但IP未变更，可能网卡驱动不支持MAC伪装或DHCP服务器分配了相同IP".to_string());
        }
    }

    Ok(serde_json::json!({
        "name": adapter.name,
        "wireless": adapter.wireless,
        "ip": new_ip,
        "regOk": reg_ok,
        "success": ip_changed,
        "skipped": false,
        "reason": message
    }))
}

pub fn get_wireless_ssid() -> Result<Option<String>, String> {
    let output = new_command("netsh")
        .args(["wlan", "show", "interfaces"])
        .output()
        .map_err(|e| format!("获取无线网络信息失败: {}", e))?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("SSID") && !trimmed.starts_with("BSSID") {
            let after = trimmed;
            if let Some(colon) = after.find(':') {
                let ssid = after[colon + 1..].trim();
                if !ssid.is_empty()
                    && !ssid.contains("不在")
                    && !ssid.contains("not connected")
                    && !ssid.contains("disconnected")
                {
                    return Ok(Some(ssid.to_string()));
                }
            }
        }
    }

    Ok(None)
}

pub fn get_wired_network_profile() -> Result<Option<String>, String> {
    let output = new_command("netsh")
        .args(["lan", "show", "interfaces"])
        .output()
        .map_err(|e| format!("获取有线网络信息失败: {}", e))?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        let is_profile_line = trimmed.to_lowercase().contains("profile")
            || trimmed.contains("配置文件")
            || trimmed.contains("設定檔");
        if is_profile_line {
            if let Some(colon) = trimmed.find(':') {
                let name = trimmed[colon + 1..].trim();
                if !name.is_empty() {
                    return Ok(Some(name.to_string()));
                }
            }
        }
    }

    Ok(None)
}

pub fn check_gateway_reachable(gateway: &str) -> bool {
    check_gateway_reachable_from(gateway, None)
}

/// 从指定源 IP 检查网关可达性（Windows ping -S 参数绑定源地址）
/// source_ip 为 None 或无效时回退到系统默认路由
pub fn check_gateway_reachable_from(gateway: &str, source_ip: Option<&str>) -> bool {
    if gateway.is_empty() {
        return false;
    }
    let mut cmd = new_command("ping");
    cmd.args(["-n", "1", "-w", "2000"]);
    if let Some(src) = source_ip {
        if !src.is_empty() && src.parse::<std::net::IpAddr>().is_ok() {
            cmd.args(["-S", src]);
        }
    }
    cmd.arg(gateway);
    match cmd.output() {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

pub fn is_same_subnet_18(ip_str: &str, gateway_str: &str) -> bool {
    let ip: u32 = match ip_str.parse::<std::net::Ipv4Addr>() {
        Ok(addr) => u32::from(addr),
        Err(_) => return false,
    };
    let gw: u32 = match gateway_str.parse::<std::net::Ipv4Addr>() {
        Ok(addr) => u32::from(addr),
        Err(_) => return false,
    };
    let mask: u32 = 0xFFFF_C000;
    (ip & mask) == (gw & mask)
}

pub fn wait_for_adapter(max_wait_ms: u64, is_quitting: &std::sync::atomic::AtomicBool) -> Result<Vec<Adapter>, String> {
    let start = std::time::Instant::now();
    let mut delay_ms: u64 = 1000;

    while start.elapsed().as_millis() < max_wait_ms as u128 {
        if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(vec![]);
        }

        let adapters = get_adapters_force()?;
        if !adapters.is_empty() {
            return Ok(adapters);
        }

        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        delay_ms = (delay_ms * 2).min(5000);
    }

    get_adapters_cached()
}

pub fn poll_adapter_ip_quick(adapter_name: &str, timeout_ms: u64, is_quitting: &AtomicBool) -> bool {
    let start = std::time::Instant::now();
    let interval = std::time::Duration::from_millis(100);
    let timeout = std::time::Duration::from_millis(timeout_ms);
    // 记录初始 IP，只有 IP 变为非空且与初始值不同时才认为续租成功
    let initial_ip = get_adapters_force()
        .ok()
        .and_then(|list| list.iter().find(|a| a.name == adapter_name).map(|a| a.ip.clone()))
        .unwrap_or_default();
    while start.elapsed() < timeout {
        if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
            return false;
        }
        std::thread::sleep(interval);
        if let Ok(adapters) = get_adapters_force() {
            if let Some(a) = adapters.iter().find(|a| a.name == adapter_name) {
                if !a.ip.is_empty() && a.ip != initial_ip {
                    return true;
                }
            }
        }
    }
    false
}

pub fn ensure_ethernet_ip_for_login(
    app_handle: &AppHandle,
    adapters: &[Adapter],
    config: &Config,
    is_quitting: &AtomicBool,
) {
    let (a1_name, a2_name) = resolve_adapter_names(adapters, config);

    let candidates: Vec<String> = [&a1_name, &a2_name]
        .iter()
        .filter_map(|name| {
            if name.is_empty() {
                return None;
            }
            let adapter = adapters.iter().find(|a| a.name == **name)?;
            if !adapter.wireless && adapter.ip.is_empty() {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect();

    if candidates.is_empty() {
        return;
    }

    for name in &candidates {
        if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
            return;
        }

        let event_bus = EventBus::new(app_handle);
        let _ = event_bus.emit_login_log(
            &format!("检测到以太网 {} 已连接但未获取IP，正在尝试DHCP续租...", name),
            "info",
        );

        let child = new_command("ipconfig")
            .args(["/renew", name])
            .spawn();

        let got_ip = poll_adapter_ip_quick(name, 5000, is_quitting);

        if let Ok(mut c) = child {
            let _ = c.kill();
            let _ = c.wait();
        }

        if got_ip {
            let ip = get_adapters_force()
                .ok()
                .and_then(|list| list.iter().find(|a| a.name == *name).map(|a| a.ip.clone()))
                .unwrap_or_default();
            let event_bus = EventBus::new(app_handle);
            let _ = event_bus.emit_login_log(
                &format!("以太网 {} DHCP续租成功，IP: {}", name, ip),
                "success",
            );
        } else {
            let event_bus = EventBus::new(app_handle);
            let _ = event_bus.emit_login_log(
                &format!("以太网 {} DHCP续租超时仍未获得IP，跳过该网卡", name),
                "warning",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === is_blacklisted 词边界回归测试 ===
    // 防止 nat/tor/virtual 误伤 "Native/National/Toronto" 等合法物理网卡名
    #[test]
    fn blacklist_word_boundary_does_not_match_legit_nics() {
        // 包含 "nat" 但不是独立的 nat 单词
        assert!(!is_blacklisted("Intel Native Ethernet Adapter"));
        assert!(!is_blacklisted("National Semiconductor NIC"));
        assert!(!is_blacklisted("NATO Secure Adapter"));
        assert!(!is_blacklisted("Native Ethernet"));

        // 包含 "tor" 但不是独立的 tor 单词
        assert!(!is_blacklisted("Toronto Office Ethernet"));
        assert!(!is_blacklisted("Tornado Net Bridge"));
        assert!(!is_blacklisted("Vector Network"));
        assert!(!is_blacklisted("Mentor Lab Network"));

        // 包含 "virtual" 但不是独立的 virtual 单词
        // "Tutorial" 中含 "tut" 不会命中；"Active virtualized" 才会命中
        // 真实物理网卡几乎不会用 "Virtualization" 单独成词，所以应安全
        assert!(!is_blacklisted("Tutorial Lab NIC"));
    }

    #[test]
    fn blacklist_word_boundary_still_matches_known_virtuals() {
        // nat 独立词
        assert!(is_blacklisted("NAT Network"));
        assert!(is_blacklisted("nat"));
        assert!(is_blacklisted("My NAT Adapter"));

        // tor 独立词
        assert!(is_blacklisted("tor"));
        assert!(is_blacklisted("Tor Service"));

        // virtual 独立词
        assert!(is_blacklisted("Virtual Ethernet"));
        assert!(is_blacklisted("Hyper-V Virtual NIC"));
    }

    // === 中文虚拟网卡补全回归测试 ===
    #[test]
    fn blacklist_chinese_virtual_keywords() {
        assert!(is_blacklisted("虚拟网卡"));
        assert!(is_blacklisted("伪 VPN"));
        assert!(is_blacklisted("假测试网卡"));
        assert!(is_blacklisted("测试虚拟连接"));
        assert!(is_blacklisted("模拟网络"));
        assert!(is_blacklisted("IPv6 隧道"));

        // "本地连接" 仍应被命中（用户特定业务规则）
        assert!(is_blacklisted("本地连接"));
        assert!(is_blacklisted("本地连接 2"));
    }

    #[test]
    fn blacklist_does_not_match_legit_chinese_nics() {
        // 中文真实网卡名应不被命中
        assert!(!is_blacklisted("以太网"));
        assert!(!is_blacklisted("WLAN"));
        assert!(!is_blacklisted("校园网认证"));
    }

    // === 真实物理 WLAN/以太网过滤决策：完全交给 is_visible_in_ncpa（注册表检测） ===
    // 见 is_visible_in_ncpa 实现：基于 HKLM\SYSTEM\CurrentControlSet\Control\Network
    //                             {4D36E972-...}\{GUID}\Connection 注册表判断，
    //                             + HKLM\SYSTEM\CurrentControlSet\Enum\<Enumerator>\{GUID}
    //                             的 PnP 设备树双重检查
    //                             完全不依赖名称模式，不会误伤多物理网卡场景
    //
    // 关键场景：用户系统有 2 块物理 Wi-Fi 网卡（Intel BE200 + Realtek），
    //          真实命名可能是 "WLAN" 和 "WLAN 2"（或 "Wi-Fi" 和 "WLAN"）
    //          这种情况下"按名称 wlan\s+[2-9]\d* 过滤"会误伤第二块真实网卡
    //          而 PnP 设备树检查会保留所有真实网卡（每块都有 PnP Enum 实例）
    //          同时过滤 Wi-Fi Direct Virtual Adapter 创建的 WLAN 2/3/4/5 幽灵副本
    //          （这些在网络栈可见但 PnP Enum 中无对应实例）

    // === class_subkey_has_matching_guid 集成测试（需在真实 Windows 环境下运行） ===
    #[cfg(target_os = "windows")]
    #[test]
    fn class_subkey_check_empty_guid_returns_false() {
        assert!(!class_subkey_has_matching_guid(""));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn class_subkey_check_nonexistent_guid_returns_false() {
        assert!(!class_subkey_has_matching_guid("{00000000-0000-0000-0000-000000000000}"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn class_subkey_check_real_wlan_guid_returns_true() {
        // 真实 WLAN：class 0002 的 NetCfgInstanceId = {86B8D1AD-...}
        let result = class_subkey_has_matching_guid("{86B8D1AD-30C8-479C-B7B2-846BD1C590FF}");
        if !result {
            eprintln!("[SKIP] 当前环境无真实 WLAN class subkey");
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn class_subkey_check_ghost_wlan_guids_return_false() {
        // 幽灵虚拟副本：NetCfgInstanceId 在 class subkey 中无匹配
        let ghost_guids = [
            "{DA918853-570D-45C6-8AE1-A841D9A0D978}",  // WLAN 2
            "{C1CE50FF-65E7-46BD-9106-4E00A7C49AB6}",  // WLAN 3
            "{723CE6A0-D1BD-45F0-86C7-1FECE96D18ED}",  // WLAN 4
            "{DADC7A44-5EBF-4DED-BC80-EB66136A8BB0}",  // WLAN 5
        ];
        for guid in ghost_guids {
            assert!(!class_subkey_has_matching_guid(guid), "幽灵 GUID {} 应返回 false", guid);
        }
    }

    // === resolve_adapter_names else 分支降级测试 ===
    fn make_test_config(adapter1: &str, dual: bool, adapter2: &str) -> crate::config::Config {
        crate::config::Config {
            user: String::new(),
            password: String::new(),
            operator: String::new(),
            adapter1: adapter1.to_string(),
            adapter2: adapter2.to_string(),
            dual_adapter: dual,
            auto_login_on_start: false,
            auto_exit_after_login: false,
            minimize_to_tray: false,
            hidden_start: false,
            auto_launch: false,
            enable_background_check: false,
            background_check_interval: 60,
            auto_login_on_preparation: false,
            auto_exit_on_online: false,
            theme_mode: "light".to_string(),
            enable_notification: false,
            active_account: String::new(),
            enable_latency_test: false,
            latency_test_interval: 300,
            custom_theme_color: String::new(),
            default_panel: "login".to_string(),
            enable_network_quality: false,
            skip_ttfb_in_latency: true,
            skip_content_in_latency: true,
            portal_url: String::new(),
            fixed_gateway: String::new(),
            required_network_name: String::new(),
            enable_network_name_check: false,
            campus_gateway: String::new(),
            campus_exit_on_fail: true,
            campus_check_start_minutes: 480,
            log_retention_days: 7,
            config_version: 2,
        }
    }

    fn make_test_adapter(name: &str, wireless: bool, ip: &str) -> Adapter {
        let status = if ip.is_empty() {
            AdapterStatus::EnabledNoIp
        } else {
            AdapterStatus::Connected
        };
        Adapter {
            name: name.to_string(),
            ip: ip.to_string(),
            wireless,
            guid: format!("{{{}}}", name),
            mac: String::new(),
            if_index: 1,
            status,
        }
    }

    #[test]
    fn resolve_adapter_names_falls_back_when_config_name_missing() {
        // 配置中写了"本地连接"，但该网卡已被黑名单过滤
        // resolve_adapter_names 必须降级到自动检测，不能静默返回"本地连接"
        let adapters = vec![
            make_test_adapter("以太网", false, "10.2.0.1"),
            make_test_adapter("WLAN", true, ""),
        ];
        let config = make_test_config("本地连接", false, "");
        let (a1, a2) = resolve_adapter_names(&adapters, &config);
        // 降级到自动检测：选有线有 IP 的"以太网"
        assert_eq!(a1, "以太网");
        assert_eq!(a2, "");
    }

    #[test]
    fn resolve_adapter_names_uses_config_when_present() {
        // 配置名存在于过滤后列表中 → 直接使用
        let adapters = vec![
            make_test_adapter("以太网", false, "10.2.0.1"),
            make_test_adapter("WLAN", true, "10.2.0.2"),
        ];
        let config = make_test_config("WLAN", true, "以太网");
        let (a1, a2) = resolve_adapter_names(&adapters, &config);
        assert_eq!(a1, "WLAN");
        assert_eq!(a2, "以太网");
    }

    #[test]
    fn resolve_adapter_names_auto_detect_prefers_wired_with_ip() {
        // 配置为"自动检测" → 优先选有线有 IP
        let adapters = vec![
            make_test_adapter("WLAN", true, "10.2.0.2"),
            make_test_adapter("以太网", false, "10.2.0.1"),
        ];
        let config = make_test_config("自动检测", false, "");
        let (a1, _) = resolve_adapter_names(&adapters, &config);
        assert_eq!(a1, "以太网");
    }
}
