//! PnP 设备级禁用检测与启用（cfgmgr32 + pnputil）。
//!
//! Windows 的「网卡被禁用」是两层独立状态：
//! - NDIS admin 层：`netsh interface set interface ... enable` 只动这层；
//! - PnP 设备层：设备管理器式禁用（`CM_PROB_DISABLED`，problem 22），
//!   两层经 `MSFT_NetAdapter.State`（PnP 状态）与 `InterfaceAdminStatus`（RFC 2863）
//!   并列暴露，互相独立。
//!
//! 外接 USB 网卡经系统设置/设备管理器禁用后处于 PnP problem 22 态，
//! 此时 netsh enable 只能触发 NDIS miniport 重连、清不掉 PnP 禁用——
//! 表现为「网卡反复重新连接却始终未启用」。
//! 本模块在启用前检测设备级禁用：命中时改走 `pnputil /enable-device`
//! （SetupAPI 设备级启用的官方命令行，管理员权限），并以 `CM_Get_DevNode_Status`
//! 复核禁用态真正解除——不信命令返回值，problem 22 可能滞留。

use winreg::enums::HKEY_LOCAL_MACHINE;
use winreg::RegKey;
use windows::core::PCWSTR;
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Get_DevNode_Status, CM_Locate_DevNodeW, CM_LOCATE_DEVNODE_PHANTOM, CM_PROB_DISABLED,
    CM_DEVNODE_STATUS_FLAGS, CM_PROB, CR_SUCCESS, DN_HAS_PROBLEM,
};

/// Net 设备类 GUID：Control\Network 键按此组织，也是 GUID_DEVCLASS_NET。
const NET_CLASS_GUID: &str = "{4D36E972-E325-11CE-BFC1-08002BE10318}";
const CONTROL_NETWORK_ROOT: &str = "SYSTEM\\CurrentControlSet\\Control\\Network";

/// 设备级启用复核轮询：总时长与单次间隔。
/// 实测参考：PnP 状态落盘与 devnode problem 状态存在百毫秒级时差（96~330ms），
/// USB 设备启用伴随重枚举，状态更新更慢，3 秒足够覆盖正常窗口。
const ENABLE_VERIFY_TOTAL_MS: u64 = 3000;
const ENABLE_VERIFY_INTERVAL_MS: u64 = 200;

/// 读取设备实例的 problem code（无 problem 时返回 0）。
/// Err 表示 devnode 不可定位或状态查询失败（含 USB 重枚举重建窗口）。
pub(crate) fn devnode_problem(instance_id: &str) -> Result<u32, String> {
    let id_wide: Vec<u16> = instance_id
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut devinst = 0u32;
    let cr = unsafe {
        CM_Locate_DevNodeW(&mut devinst, PCWSTR(id_wide.as_ptr()), CM_LOCATE_DEVNODE_PHANTOM)
    };
    if cr != CR_SUCCESS {
        return Err(format!("CM_Locate_DevNodeW 失败: 0x{:08X}", cr.0));
    }
    let mut status = CM_DEVNODE_STATUS_FLAGS(0);
    let mut problem = CM_PROB(0);
    let cr = unsafe { CM_Get_DevNode_Status(&mut status, &mut problem, devinst, 0) };
    if cr != CR_SUCCESS {
        return Err(format!("CM_Get_DevNode_Status 失败: 0x{:08X}", cr.0));
    }
    if status.0 & DN_HAS_PROBLEM.0 != 0 {
        Ok(problem.0)
    } else {
        Ok(0)
    }
}

/// 从注册表读接口 GUID 对应的 PnP 设备实例 ID（未文档化注册表值，多款工具在用）。
/// `guid` 为带花括号的接口 GUID（IP_ADAPTER_ADDRESSES.AdapterName 格式）。
fn read_pnp_instance_id(guid: &str) -> Option<String> {
    let hk = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hk
        .open_subkey(format!("{CONTROL_NETWORK_ROOT}\\{NET_CLASS_GUID}\\{guid}\\Connection"))
        .ok()?;
    key.get_value::<String, _>("PnPInstanceId").ok()
}

/// 回退定位：适配器不在 GetAdaptersAddresses 列表（接口行缺失）时，
/// 遍历 Control\Network 按 Connection\Name 匹配接口 GUID。
fn find_guid_by_name(adapter_name: &str) -> Option<String> {
    let hk = RegKey::predef(HKEY_LOCAL_MACHINE);
    let root = hk
        .open_subkey(format!("{CONTROL_NETWORK_ROOT}\\{NET_CLASS_GUID}"))
        .ok()?;
    for sub in root.enum_keys().flatten() {
        let Ok(conn) = root.open_subkey(format!("{sub}\\Connection")) else {
            continue;
        };
        if conn.get_value::<String, _>("Name").ok().as_deref() == Some(adapter_name) {
            return Some(sub);
        }
    }
    None
}

/// 检测适配器是否处于 PnP 设备级禁用（problem 22）。
///
/// 返回 `Some(instance_id)` 表示需要走设备级启用；`None` 表示未命中
/// （接口定位失败 / 无 PnPInstanceId / 非 problem 22），调用方维持 netsh 原路径。
pub fn find_disabled_device(adapter_name: &str) -> Result<Option<String>, String> {
    // 优先从适配器列表取 guid（接口行在线），回退注册表按名称匹配（接口行缺失）
    let guid = crate::network::adapter_cache::get_adapters_force()
        .ok()
        .and_then(|list| list.iter().find(|a| a.name == adapter_name).map(|a| a.guid.clone()))
        .or_else(|| find_guid_by_name(adapter_name));
    let Some(guid) = guid else {
        return Ok(None);
    };

    let Some(instance_id) = read_pnp_instance_id(&guid) else {
        return Ok(None);
    };

    match devnode_problem(&instance_id) {
        Ok(problem) if problem == CM_PROB_DISABLED.0 => Ok(Some(instance_id)),
        // 非 problem 22（含无 problem / 查询失败）：netsh 覆盖 NDIS admin 层即可，
        // 不对已启动设备做设备级操作——那会触发一次不必要的重枚举
        _ => Ok(None),
    }
}

/// 设备级启用：管理员进程内直跑 `pnputil /enable-device` + 复核；非管理员经
/// helper 框架（计划任务代理 → CMSTPLUA → runas）由 SYSTEM worker 执行
/// （worker 内含相同的字符集/存在性校验与 problem 22 解除复核）。
///
/// 完成后轮询复核 problem 22 确实解除——不信命令返回值。
pub fn enable_device(instance_id: &str, allow_uac_prompt: bool) -> Result<(), String> {
    if !crate::platform::elevation::is_admin() {
        let result_name = crate::platform::helper_spawn::new_result_name();
        let v = crate::platform::helper_spawn::spawn_elevated_helper(
            "enable_device",
            &[instance_id],
            &result_name,
            std::time::Duration::from_secs(30),
            allow_uac_prompt,
        )?;
        if !v.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
            return Err(v
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("设备级启用失败（helper 无 message）")
                .to_string());
        }
        return Ok(());
    }

    let output = super::new_command("pnputil")
        .args(["/enable-device", instance_id])
        .output()
        .map_err(|e| format!("pnputil 执行失败: {e}"))?;
    if !output.status.success() {
        let stderr = crate::platform::console_output::decode_console_bytes(&output.stderr);
        let detail = if stderr.trim().is_empty() {
            crate::platform::console_output::decode_console_bytes(&output.stdout)
        } else {
            stderr
        };
        return Err(format!("pnputil /enable-device 返回非零: {}", detail.trim()));
    }

    // 复核：不信 pnputil 返回值。轮询期间 CM 查询失败（重枚举重建窗口）继续等，
    // 只认 problem 离开 22；超时仍 22 或持续查询失败则报错交上层重试。
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(ENABLE_VERIFY_TOTAL_MS);
    loop {
        match devnode_problem(instance_id) {
            Ok(problem) if problem != CM_PROB_DISABLED.0 => return Ok(()),
            _ if std::time::Instant::now() >= deadline => {
                return Err("pnputil 已执行但设备仍处于 PnP 禁用态(problem 22)".to_string());
            }
            _ => std::thread::sleep(std::time::Duration::from_millis(ENABLE_VERIFY_INTERVAL_MS)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_guid_by_name_missing_returns_none() {
        assert_eq!(find_guid_by_name("__no_such_adapter__"), None);
    }

    #[test]
    fn read_pnp_instance_id_missing_guid_returns_none() {
        assert_eq!(read_pnp_instance_id("{00000000-0000-0000-0000-000000000000}"), None);
    }

    #[test]
    fn devnode_problem_nonexistent_instance_errors() {
        // 不存在的设备实例：CM_Locate_DevNodeW 返回非 CR_SUCCESS（PHANTOM 允许幽灵，
        // 但完全虚构的 ID 仍不可定位）——除非系统恰好存在该 ID
        let result = devnode_problem("ROOT\\__NO_SUCH_DEVICE__\\0000");
        if let Ok(problem) = result {
            // 极端环境下若存在同 ID 设备，problem 也不应为 22
            assert_ne!(problem, CM_PROB_DISABLED.0);
        }
    }

    #[test]
    fn find_disabled_device_missing_adapter_returns_none() {
        assert_eq!(find_disabled_device("__no_such_adapter__").unwrap(), None);
    }
}
