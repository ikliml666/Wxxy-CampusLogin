//! DHCP 续租、MAC 重置、netsh 接口控制
//!
//! 从 `adapter.rs` 迁移，隔离 PowerShell 调用。
//! T4.4.2: 提权操作不再依赖 PowerShell —— 通过 --helper 以管理员身份重启自身
//!   （见 crate::helper / crate::platform::helper_spawn），由 Rust 直调注册表 + netsh，
//!   彻底移除 Set-NetAdapter 脚本与 -EncodedCommand Base64 编码。
//! T4.4.3 历史决策记录：曾评估 Win32 API（DeviceIoControl + OID_802_3_CURRENT_ADDRESS）
//!   替代 PowerShell，结论是需引入 unsafe 代码和驱动依赖，收益不抵成本；
//!   最终采用"提权重启自身 + 注册表 NetworkAddress + 重启网卡"方案（等价于
//!   Set-NetAdapter -MacAddress 的底层行为），见 apply_mac_change_via_registry。

use crate::network::adapter_cache::{
    get_adapters_cached, get_adapters_force, validate_adapter_name,
};
use crate::network::subnet::is_same_subnet_18;
use crate::network::discovery::{is_blacklisted, Adapter, new_command};

pub fn dhcp_renew(adapter_name: &str) -> Result<bool, String> {
    validate_adapter_name(adapter_name)?;
    let output = new_command("ipconfig")
        .args(["/renew", adapter_name])
        .output()
        .map_err(|e| format!("DHCP续租失败: {e}"))?;
    Ok(output.status.success())
}

pub fn dhcp_release(adapter_name: &str) -> Result<bool, String> {
    validate_adapter_name(adapter_name)?;
    let output = new_command("ipconfig")
        .args(["/release", adapter_name])
        .output()
        .map_err(|e| format!("DHCP释放失败: {e}"))?;
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
    // BE-A-06: 原实现用"系统时间+计数器"种子自制 LCG，同一毫秒内 MAC 可被推算，可预测。
    // 改用成熟随机源 getrandom（Windows 走 BCryptGenRandom）填充 6 字节。
    // 低概率失败时降级回时间+计数器种子，保证函数总能返回合法单播/本地管理 MAC。
    let mut bytes = [0u8; 6];
    if getrandom::fill(&mut bytes).is_err() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let counter = MAC_SEED_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let mut rng = time.wrapping_add(counter.wrapping_mul(0x9E3779B97F4A7C15));
        for b in bytes.iter_mut() {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            *b = (rng & 0xFF) as u8;
        }
    }
    bytes[0] = (bytes[0] & 0xFC) | 0x02; // 第 1 字节：最低位=0 单播，次低位=1 本地管理
    format!(
        "{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5]
    )
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
                format!("打开网卡注册表失败: {e}")
            }
        })?;
    for subkey_name in class_key.enum_keys().filter_map(|r| r.ok()) {
        if let Ok(subkey) = class_key.open_subkey_with_flags(&subkey_name, KEY_ALL_ACCESS) {
            if let Ok(instance_id) = subkey.get_value::<String, _>("NetCfgInstanceId") {
                if instance_id.eq_ignore_ascii_case(adapter_guid) {
                    subkey.set_value("NetworkAddress", &mac_no_dash)
                        .map_err(|e| format!("写入NetworkAddress失败: {e}"))?;
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
                format!("打开网卡注册表失败: {e}")
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
    new_command("netsh")
        .args(["interface", "set", "interface", &format!("name={adapter_name}"), "admin=disable"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn netsh_enable(adapter_name: &str) -> bool {
    if validate_adapter_name(adapter_name).is_err() {
        return false;
    }
    new_command("netsh")
        .args(["interface", "set", "interface", &format!("name={adapter_name}"), "admin=enable"])
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
            if let Some(a) = crate::network::find_by_name(&adapters, adapter_name) {
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
            if let Some(a) = crate::network::find_by_name(&adapters, adapter_name) {
                if !a.ip.is_empty() {
                    return true;
                }
            }
        }
        std::thread::sleep(interval);
    }
    false
}

/// 通过注册表修改 MAC 并重启网卡（管理员 / 提权 helper 共用）。
/// 等价于 `Set-NetAdapter -MacAddress` 的底层行为：写 NetworkAddress + 重启网卡。
#[cfg(target_os = "windows")]
pub fn apply_mac_change_via_registry(
    adapter_guid: &str,
    adapter_name: &str,
    mac_no_dash: &str,
) -> Result<(), String> {
    set_mac_via_registry(adapter_guid, mac_no_dash)?;
    let _ = dhcp_release(adapter_name);
    let disable_ok = netsh_disable(adapter_name);
    if disable_ok {
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    let _ = netsh_enable(adapter_name);
    let _ = dhcp_renew(adapter_name);
    Ok(())
}

/// 尝试修改适配器 MAC 地址：管理员直写注册表，非管理员通过 --helper 提权重启自身
fn try_modify_mac(adapter: &Adapter, fake_mac: &str, _mac_dashed: &str) -> (bool, bool, Option<String>) {
    if crate::platform::elevation::is_admin() {
        match set_mac_via_registry(&adapter.guid, fake_mac) {
            Ok(()) => {
                crate::log_info!("adapter", "管理员直写注册表成功: guid={}", adapter.guid);
                (true, false, None)
            }
            Err(e) => (false, false, Some(format!("MAC地址修改失败: {e}"))),
        }
    } else {
        crate::log_info!("adapter", "非管理员运行，通过 --helper 提权修改MAC: guid={}", adapter.guid);
        let result_path = crate::platform::helper_spawn::unique_result_path();
        let args = [adapter.guid.as_str(), fake_mac];
        match crate::platform::helper_spawn::spawn_elevated_helper(
            "mac",
            &args,
            &result_path,
            std::time::Duration::from_secs(25),
        ) {
            Ok(v) => {
                let success = v.get("success").and_then(|s| s.as_bool()).unwrap_or(false);
                if !success {
                    let msg = v
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("helper执行失败")
                        .to_string();
                    crate::log_warn!("adapter", "helper修改MAC失败: {}", msg);
                    return (false, true, Some(msg));
                }
                // helper 已完成写注册表 + 重启网卡，主进程侧等待 IP 变更
                if let Some(changed_ip) = poll_ip_change(&adapter.name, &adapter.ip, 25_000) {
                    crate::log_info!("adapter", "helper修改MAC成功: 新IP={}", changed_ip);
                    (true, true, None)
                } else {
                    crate::log_warn!("adapter", "helper修改MAC完成但25秒内IP未变更");
                    (true, true, Some("MAC已修改但IP未变更，可能网卡驱动不支持MAC伪装".to_string()))
                }
            }
            Err(e) => {
                crate::log_warn!("adapter", "helper提权执行失败: {}", e);
                (false, true, Some(format!("提权失败: {e}，请尝试以管理员身份运行应用")))
            }
        }
    }
}

/// 对单个适配器执行 MAC 修改 + DHCP 释放/续租流程，返回结果 JSON
fn renew_adapter_with_mac(adapter: &Adapter, campus_gateway: &str) -> serde_json::Value {
    // T4.4.1: 虚拟适配器白名单过滤，跳过虚拟/软件网卡避免误操作
    if is_blacklisted(&adapter.name) {
        return serde_json::json!({
            "name": adapter.name,
            "wireless": adapter.wireless,
            "ip": adapter.ip,
            "success": false,
            "skipped": true,
            "reason": "虚拟适配器，跳过 MAC 重置"
        });
    }
    if !adapter.ip.is_empty() && !is_same_subnet_18(&adapter.ip, campus_gateway) {
        return serde_json::json!({
            "name": adapter.name,
            "wireless": adapter.wireless,
            "ip": adapter.ip,
            "success": false,
            "skipped": true,
            "reason": "非校园网子网，跳过"
        });
    }

    let fake_mac = generate_random_mac();
    let mac_dashed = mac_with_dashes(&fake_mac);
    let (reg_ok, elevated_done, elevate_msg) = try_modify_mac(adapter, &fake_mac, &mac_dashed);

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
        if let Ok(refreshed) = get_adapters_force() {
            if let Some(a) = crate::network::find_by_name(&refreshed, &adapter.name) {
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
                if let Some(a) = crate::network::find_by_name(&refreshed, &adapter.name) {
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

    serde_json::json!({
        "name": adapter.name,
        "wireless": adapter.wireless,
        "ip": new_ip,
        "regOk": reg_ok,
        "success": ip_changed,
        "skipped": false,
        "reason": message
    })
}

pub fn dhcp_release_renew_all(campus_gateway: &str) -> Result<Vec<serde_json::Value>, String> {
    if campus_gateway.is_empty() {
        return Err("校园网网关为空，无法判断子网".to_string());
    }
    let adapters = get_adapters_cached()?;
    if adapters.is_empty() { return Ok(vec![]); }

    let mut results = Vec::new();
    for adapter in &adapters {
        results.push(renew_adapter_with_mac(adapter, campus_gateway));
    }
    Ok(results)
}

pub fn dhcp_release_renew_single(adapter_name: &str, campus_gateway: &str) -> Result<serde_json::Value, String> {
    let adapters = get_adapters_cached()?;
    let adapter = crate::network::find_by_name(&adapters, adapter_name)
        .ok_or_else(|| format!("未找到适配器: {adapter_name}"))?;
    Ok(renew_adapter_with_mac(adapter, campus_gateway))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blacklist_filters_known_virtual_adapters() {
        // T4.4.1: 验证虚拟适配器会被白名单过滤
        assert!(is_blacklisted("Virtual Ethernet"));
        assert!(is_blacklisted("Hyper-V Virtual NIC"));
        assert!(is_blacklisted("NAT Network"));
        assert!(is_blacklisted("虚拟网卡"));
    }

    #[test]
    fn blacklist_preserves_physical_adapters() {
        // T4.4.1: 验证物理网卡不会被误过滤
        assert!(!is_blacklisted("以太网"));
        assert!(!is_blacklisted("WLAN"));
        assert!(!is_blacklisted("Intel Ethernet Adapter"));
    }
}
