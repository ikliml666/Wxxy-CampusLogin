//! DHCP 续租、MAC 重置、netsh 接口控制
//!
//! 从 `adapter.rs` 迁移，隔离 PowerShell 调用。
//! PowerShell 参数化（`-EncodedCommand`）将在 T4.4.2 完成。

use crate::network::adapter_cache::{
    get_adapters_cached, get_adapters_force, validate_adapter_name,
};
use crate::network::subnet::is_same_subnet_18;
use crate::network::discovery::{Adapter, new_command};

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
    use std::time::{SystemTime, UNIX_EPOCH};
    let counter = MAC_SEED_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let seed = time.wrapping_add(counter.wrapping_mul(0x9E3779B97F4A7C15));
    let mut rng = seed;
    let mut next = || { rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); rng };
    let b1 = (next() & 0xFF) as u8;
    let b2 = (next() & 0xFF) as u8;
    let b3 = (next() & 0xFF) as u8;
    let b4 = (next() & 0xFF) as u8;
    let b5 = (next() & 0xFF) as u8;
    let b6 = (next() & 0xFF) as u8;
    let first = (b1 & 0xFC) | 0x02;
    format!("{first:02X}{b2:02X}{b3:02X}{b4:02X}{b5:02X}{b6:02X}")
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
    match crate::platform::elevation::run_elevated("powershell", &format!("-WindowStyle Hidden -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command \"{script}\"")) {
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
            (false, Some(format!("提权失败: {e}，请尝试以管理员身份运行应用")))
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
                Err(e) => (false, false, Some(format!("MAC地址修改失败: {e}"))),
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
        .ok_or_else(|| format!("未找到适配器: {adapter_name}"))?;

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
            Err(e) => (false, false, Some(format!("MAC地址修改失败: {e}"))),
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
