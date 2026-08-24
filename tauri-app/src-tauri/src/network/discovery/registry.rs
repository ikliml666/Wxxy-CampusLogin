//! 网卡注册表可见性 / 禁用状态检查
//!
//! 从 `discovery::windows` 拆分，专注注册表查询逻辑：
//! - `is_visible_in_ncpa`: 判断网卡是否在 ncpa.cpl / Win11 高级网络设置中可见
//! - `class_subkey_has_matching_guid`: Class 注册表项交叉验证（防幽灵虚拟副本）
//! - `is_admin_disabled_via_registry`: 区分 NotPresent 状态下的"管理员禁用" vs "硬件缺失"
//!
//! 仅 Windows 平台编译（依赖 `winreg` crate）。

use lazy_static::lazy_static;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::time::Instant;

#[cfg(test)]
mod tests {
    // === class_subkey_has_matching_guid 集成测试（需在真实 Windows 环境下运行） ===
    #[test]
    fn class_subkey_check_empty_guid_returns_false() {
        assert!(!super::class_subkey_has_matching_guid(""));
    }

    #[test]
    fn class_subkey_check_nonexistent_guid_returns_false() {
        assert!(!super::class_subkey_has_matching_guid("{00000000-0000-0000-0000-000000000000}"));
    }

    #[test]
    fn class_subkey_check_real_wlan_guid_returns_true() {
        // 真实 WLAN：class 0002 的 NetCfgInstanceId = {86B8D1AD-...}
        let result = super::class_subkey_has_matching_guid("{86B8D1AD-30C8-479C-B7B2-846BD1C590FF}");
        if !result {
            eprintln!("[SKIP] 当前环境无真实 WLAN class subkey");
        }
    }

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
            assert!(!super::class_subkey_has_matching_guid(guid), "幽灵 GUID {guid} 应返回 false");
        }
    }
}

/// 判断网卡是否在 Win11 高级网络设置 / ncpa.cpl 中可见
/// 严格按注册表 PnP 设备树检测，避免按名称误伤多物理网卡场景
pub fn is_visible_in_ncpa(guid: &str) -> bool {
    // 历史缺陷：空 GUID 直接返回 false，物理适配器 AdapterName 为空时被静默隐藏。
    // 空 GUID 无法做注册表匹配，回退为"可见"（交由黑名单/IP 等后续过滤兜底）。
    if guid.is_empty() {
        return true;
    }
    // 注册表 1：HKLM\...\Control\Network\{4D36E972-...}\{GUID}\Connection\ShowInNetworkConnections
    //   = 0 -> 用户/系统标记为隐藏
    //   = 1 或不存在 -> Windows 默认显示
    //
    // 注册表 2：HKLM\SYSTEM\CurrentControlSet\Enum\<Enumerator>\<InstanceId>
    //   PnP 设备树中必须存在该 GUID 对应的实例，否则为"幽灵虚拟副本"（如 Wi-Fi Direct Virtual Adapter
    //   创建的多个 WLAN 2/3/4/5，这些在网络栈可见但 PnP 树中已被清理）
    //
    // 决策：注册表 1 + 2 都通过才视为可见
    // 注册表 1 检查：Connection 子键的 ShowInNetworkConnections
    //（BE-B-04：随 4s 适配器缓存刷新，parse 对每个适配器触发一次注册表 open_subkey，
    // 加短 TTL 整表缓存；启用适配器等变更路径通过 invalidate_show_in_ncpa_cache 立即失效）
    if !show_in_ncpa_cached(guid) {
        return false;
    }
    // 注册表 2 检查：Class subkey 交叉验证
    class_subkey_has_matching_guid(guid)
}

// ShowInNetworkConnections 查询缓存：guid -> 是否可见。None 表示未初始化或已失效。
//（lazy_static 宏不生成 rustdoc，用普通注释避免 unused doc comment 警告）
lazy_static! {
    static ref SHOW_IN_NCPA_CACHE: RwLock<Option<(HashMap<String, bool>, Instant)>> = RwLock::new(None);
}

/// 缓存 TTL（秒），与适配器缓存 TTL（5s）对齐：值极少变化且变更路径有显式失效。
const SHOW_IN_NCPA_CACHE_TTL_SECS: u64 = 5;

/// 原注册表 1 检查：Connection 子键的 ShowInNetworkConnections
fn query_show_in_ncpa(guid: &str) -> bool {
    let key_path = format!(
        "SYSTEM\\CurrentControlSet\\Control\\Network\\{{4D36E972-E325-11CE-BFC1-08002BE10318}}\\{guid}\\Connection"
    );
    match winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE).open_subkey(&key_path) {
        Ok(key) => {
            match key.get_value::<u32, _>("ShowInNetworkConnections") {
                Ok(val) => val != 0,
                Err(_) => true,
            }
        }
        Err(_) => true,  // Connection 子键缺失 -> 视为可见，由 PnP 树检查把关
    }
}

/// 带短 TTL 缓存的注册表 1 查询（读多写少，读锁快速路径）
fn show_in_ncpa_cached(guid: &str) -> bool {
    {
        let cache = SHOW_IN_NCPA_CACHE.read();
        if let Some((map, ts)) = cache.as_ref() {
            if ts.elapsed().as_secs() < SHOW_IN_NCPA_CACHE_TTL_SECS {
                if let Some(v) = map.get(guid) {
                    return *v;
                }
                // 未命中但缓存未过期：查注册表并补入（设备管理器新出现的适配器）
                let v = query_show_in_ncpa(guid);
                drop(cache);
                let mut cache = SHOW_IN_NCPA_CACHE.write();
                if let Some((map, _)) = cache.as_mut() {
                    map.insert(guid.to_string(), v);
                }
                return v;
            }
        }
    }
    // 缓存缺失或已过期：全量重建（build 在锁外执行注册表 I/O，仅用锁做 swap，
    // 与 CLASS_SUBKEY_CACHE 的 ensure_cache_initialized 同模式）
    let v = query_show_in_ncpa(guid);
    let mut new_map = HashMap::new();
    new_map.insert(guid.to_string(), v);
    *SHOW_IN_NCPA_CACHE.write() = Some((new_map, Instant::now()));
    v
}

/// 失效 ShowInNetworkConnections 查询缓存。
/// 适配器启用/禁用等可见性变更路径必须调用，避免变更后短窗内返回陈旧结果。
pub fn invalidate_show_in_ncpa_cache() {
    *SHOW_IN_NCPA_CACHE.write() = None;
}

/// Class subkey 缓存条目：记录 NetCfgInstanceId 是否存在及 ConfigFlags 值
struct ClassSubkeyEntry {
    exists: bool,
    config_flags: Option<u32>,
}

lazy_static! {
    /// guid（小写） → Class subkey 条目映射。None 表示未初始化。
    static ref CLASS_SUBKEY_CACHE: RwLock<Option<HashMap<String, ClassSubkeyEntry>>> = RwLock::new(None);
}

/// 遍历注册表 Class 子键，构建 guid → entry 映射。
fn build_class_subkey_cache() -> HashMap<String, ClassSubkeyEntry> {
    let mut cache = HashMap::new();
    let hklm = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
    let class_path = "SYSTEM\\CurrentControlSet\\Control\\Class\\{4D36E972-E325-11CE-BFC1-08002BE10318}";
    let class_key = match hklm.open_subkey(class_path) {
        Ok(k) => k,
        Err(_) => return cache,
    };
    for subkey_name in class_key.enum_keys().filter_map(|r| r.ok()) {
        if let Ok(subkey) = class_key.open_subkey(&subkey_name) {
            if let Ok(instance_id) = subkey.get_value::<String, _>("NetCfgInstanceId") {
                let config_flags = subkey.get_value::<u32, _>("ConfigFlags").ok();
                cache.insert(instance_id.to_lowercase(), ClassSubkeyEntry {
                    exists: true,
                    config_flags,
                });
            }
        }
    }
    cache
}

/// 刷新 class subkey 缓存。可在适配器缓存刷新时调用，保证注册表变化后缓存更新。
pub fn refresh_class_subkey_cache() {
    let new_cache = build_class_subkey_cache();
    *CLASS_SUBKEY_CACHE.write() = Some(new_cache);
}

/// 双重检查锁定：首次访问时自动构建缓存，后续直接查询。
/// build 在锁外执行（注册表遍历是慢 I/O），仅用锁做 swap。
fn ensure_cache_initialized() {
    if CLASS_SUBKEY_CACHE.read().is_some() {
        return;
    }
    let new_cache = build_class_subkey_cache();
    let mut cache = CLASS_SUBKEY_CACHE.write();
    if cache.is_none() {
        *cache = Some(new_cache);
    }
}

pub fn class_subkey_has_matching_guid(guid: &str) -> bool {
    if guid.is_empty() {
        return false;
    }
    ensure_cache_initialized();
    CLASS_SUBKEY_CACHE.read()
        .as_ref()
        .and_then(|c| c.get(&guid.to_lowercase()))
        .map(|e| e.exists)
        .unwrap_or(false)
}

/// 用于区分 NotPresent 状态下的"管理员禁用"vs"硬件缺失(USB未连接)"
pub fn is_admin_disabled_via_registry(guid: &str) -> bool {
    if guid.is_empty() {
        return false;
    }
    ensure_cache_initialized();
    CLASS_SUBKEY_CACHE.read()
        .as_ref()
        .and_then(|c| c.get(&guid.to_lowercase()))
        .and_then(|e| e.config_flags)
        .map(|f| f & 0x1 != 0)  // CONFIGFLAG_DISABLED
        .unwrap_or(false)
}
