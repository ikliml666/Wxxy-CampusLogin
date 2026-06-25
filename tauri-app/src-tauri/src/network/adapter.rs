//! 适配器缓存、DHCP/MAC 重置、子网/SSID 工具
//!
//! 本模块保留 adapter.rs 的历史职责（缓存访问、DHCP/MAC、子网/SSID），
//! 适配器发现与注册表检查已迁移到 `network::discovery`。

use lazy_static::lazy_static;
use parking_lot::Mutex;
use std::time::Instant;
use std::sync::atomic::AtomicBool;
use tauri::AppHandle;
use crate::config::model::Config;
use crate::infra::events::EventBus;

// 从 discovery 模块 re-export 公共类型与函数，保持外部调用方不变
pub use crate::network::discovery::{
    Adapter, AdapterDetail, AdapterStatus, DisabledAdapter,
    is_blacklisted, new_command,
};

/// 缓存条目：(adapters, details, disabled, timestamp)
type AdapterCacheEntry = (Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>, Instant);

lazy_static! {
    static ref ADAPTER_CACHE: Mutex<Option<AdapterCacheEntry>> = Mutex::new(None);
}

const ADAPTER_CACHE_TTL_SECS: u64 = 5;

fn query_adapters_cached_inner() -> crate::network::discovery::AdapterQueryResult {
    {
        let cache = ADAPTER_CACHE.lock();
        if let Some((adapters, details, disabled, ts)) = cache.as_ref() {
            if ts.elapsed().as_secs() < ADAPTER_CACHE_TTL_SECS {
                return Ok((adapters.clone(), details.clone(), disabled.clone()));
            }
        }
    }
    let result = crate::network::discovery::query_adapters_addresses()?;
    {
        let mut cache = ADAPTER_CACHE.lock();
        *cache = Some((result.0.clone(), result.1.clone(), result.2.clone(), Instant::now()));
    }
    Ok(result)
}

pub fn get_all_adapters_force() -> crate::network::discovery::AdapterQueryResult {
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
        .map_err(|e| format!("适配器查询任务失败: {e}"))?
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
    let netsh_args = format!("interface set interface \"{adapter_name}\" enable");

    if crate::platform::elevation::is_admin() {
        // 管理员：直接执行 netsh
        crate::log_info!("adapter", "管理员直写启用适配器: {}", adapter_name);
        let output = new_command("netsh")
            .args(["interface", "set", "interface", adapter_name, "enable"])
            .output()
            .map_err(|e| format!("启用适配器失败: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stderr_trimmed = stderr.trim();
            return Err(if stderr_trimmed.is_empty() {
                "启用适配器失败：netsh 返回非零退出码但未输出错误信息".to_string()
            } else {
                format!("启用适配器失败: {stderr_trimmed}")
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
                    .map_err(|e| format!("提权启用适配器失败（COM 和 UAC 均失败）: COM错误={com_err}; UAC错误={e}"))?;
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
    // netsh语法要求 "name=适配器名" 作为单个参数传递，无法拆分为独立args
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
    // netsh语法要求 "name=适配器名" 作为单个参数传递，无法拆分为独立args
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

pub fn get_wireless_ssid() -> Result<Option<String>, String> {
    let output = new_command("netsh")
        .args(["wlan", "show", "interfaces"])
        .output()
        .map_err(|e| format!("获取无线网络信息失败: {e}"))?;

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
        .map_err(|e| format!("获取有线网络信息失败: {e}"))?;

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
            &format!("检测到以太网 {name} 已连接但未获取IP，正在尝试DHCP续租..."),
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
                &format!("以太网 {name} DHCP续租成功，IP: {ip}"),
                "success",
            );
        } else {
            let event_bus = EventBus::new(app_handle);
            let _ = event_bus.emit_login_log(
                &format!("以太网 {name} DHCP续租超时仍未获得IP，跳过该网卡"),
                "warning",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            guid: format!("{{{name}}}"),
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
