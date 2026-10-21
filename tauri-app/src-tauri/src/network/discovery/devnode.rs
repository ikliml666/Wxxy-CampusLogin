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
    CM_Disable_DevNode, CM_Get_DevNode_Status, CM_Get_Parent, CM_Locate_DevNodeW,
    CM_Reenumerate_DevNode, CM_LOCATE_DEVNODE_PHANTOM, CM_PROB_DISABLED, CM_REENUMERATE_SYNCHRONOUS,
    CM_DEVNODE_STATUS_FLAGS, CM_PROB, CR_SUCCESS, DN_HAS_PROBLEM,
};

/// Net 设备类 GUID：Control\Network 键按此组织，也是 GUID_DEVCLASS_NET。
const NET_CLASS_GUID: &str = "{4D36E972-E325-11CE-BFC1-08002BE10318}";
const CONTROL_NETWORK_ROOT: &str = "SYSTEM\\CurrentControlSet\\Control\\Network";

/// 设备级启用复核轮询：总时长与单次间隔。
/// 实测参考：PnP 状态落盘与 devnode problem 状态存在百毫秒级时差（96~330ms），
/// 非 USB 设备 3 秒足够覆盖正常窗口。
const ENABLE_VERIFY_TOTAL_MS: u64 = 3000;
const ENABLE_VERIFY_INTERVAL_MS: u64 = 200;

/// USB 设备的复核窗口：USB 网卡从设备级禁用恢复要走物理重枚举（等价拔插），
/// RTL8156 还会在 CD-ROM(8151) 与 NIC(8156) 两种形态间来回切换，耗时远超 3 秒。
/// 2026-10-21 08:46 事故实证：3 秒窗口把「正在重枚举」误判为失败，后续重试改走
/// netsh 死路，网卡整夜未恢复。
const USB_VERIFY_TOTAL_MS: u64 = 15000;

/// 总线重枚举（`pnputil /scan-devices`）的复核窗口：全机硬件重扫，
/// 比单设备启用慢得多，给足 15 秒。
const REENUMERATE_VERIFY_TOTAL_MS: u64 = 15000;

/// 按总线选复核窗口：USB 设备重枚举慢一个数量级。
pub(crate) fn verify_window_ms(instance_id: &str) -> u64 {
    if instance_id.to_ascii_uppercase().starts_with("USB\\") {
        USB_VERIFY_TOTAL_MS
    } else {
        ENABLE_VERIFY_TOTAL_MS
    }
}

/// 定位设备实例的 devinst（允许幽灵设备——已移除/已禁用的设备仍可定位）。
fn locate_devinst(instance_id: &str) -> Result<u32, String> {
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
    Ok(devinst)
}

/// 读取设备实例的 problem code（无 problem 时返回 0）。
/// Err 表示 devnode 不可定位或状态查询失败（含 USB 重枚举重建窗口）。
pub(crate) fn devnode_problem(instance_id: &str) -> Result<u32, String> {
    let devinst = locate_devinst(instance_id)?;
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
/// pub(crate)：monitor 出站切换用实例 ID 判总线可判定性（USB 卡自 2026-10 起不再
/// 豁免夜间禁用，见 outbound_switch::unsafe_to_disable），windows.rs 幽灵误判修复
/// 也用它定位设备树。
pub(crate) fn read_pnp_instance_id(guid: &str) -> Option<String> {
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

/// 适配器的设备级状态判定结果（启用前的分流依据）。
#[derive(Debug, PartialEq, Eq)]
pub enum DeviceState {
    /// 确定处于 PnP problem 22（设备管理器式禁用），走 pnputil 设备级启用。
    Disabled(String),
    /// 设备实例不可定位（幽灵态 / 重枚举重建窗口）：接口行缺失、netsh 必然失败，
    /// 需触发总线重扫让设备重新枚举。
    Unlocatable(String),
}

/// 检测适配器的 PnP 设备级状态。
///
/// 返回 `Ok(None)` 表示确定不是设备级问题（设备已正常启动，或该卡没有 PnP 实例 ID
/// 的虚拟卡），调用方维持 netsh 原路径。
///
/// 2026-10-21 事故教训：旧实现把「`devnode_problem` 查询失败」与「problem 非 22」
/// 一起折成 `Ok(None)`，于是 USB 网卡在重枚举窗口内（CM 定位返回 0x0D）被判定
/// 为「未禁用」→ 降级 netsh → 接口行不存在 → 连续 7 次「返回非零退出码但未输出
/// 错误信息」→ 整夜无法恢复。查询失败绝不能被当作「未禁用」。
pub fn find_disabled_device(adapter_name: &str) -> Result<Option<DeviceState>, String> {
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
        Ok(problem) if problem == CM_PROB_DISABLED.0 => Ok(Some(DeviceState::Disabled(instance_id))),
        // 已正常启动：netsh 覆盖 NDIS admin 层即可，不对已启动设备做设备级操作
        // ——那会触发一次不必要的重枚举（USB 重扫会短暂中断同 hub 的所有设备）
        Ok(_) => Ok(None),
        // 不可定位 ≠ 未禁用：USB 设备禁用/启用都会经历 PnP 重建窗口，此刻接口行
        // 不在枚举树里，netsh 只会返回非零且无输出。交给重枚举兜底。
        Err(_) => Ok(Some(DeviceState::Unlocatable(instance_id))),
    }
}

/// 设备级禁用：PnP 设备层禁用，**非持久化**（不传 `CM_DISABLE_PERSIST`）。
///
/// 与 [`enable_device`] 对称，用于夜间出站切换临时下线校园网卡。
///
/// 为什么不用 `netsh interface set interface ... disable`：netsh 的禁用会跨重启
/// 残留（2026-10-19 23:00 netsh 禁用两张卡 → 次日 08:36 重启后双双 problem 22，
/// 日志实证），而 USB 网卡在禁用期间驱动不加载，重启上电时 RTL8156 停在出厂
/// CD-ROM 模式，Windows 把它当新的大容量存储设备、弹出「正在安装设备驱动程序」
/// 提示且需点击才能用（同一序列号下 USBSTOR\CDROM 幽灵设备为证）。改用非持久化
/// 设备级禁用后，重启时设备已是启用态，驱动正常加载并切回 NIC 模式。
///
/// 官方语义（`CM_Disable_DevNode`）：默认只禁用一次、重启后自动启用；Windows 10
/// 起才需显式传 `CM_DISABLE_PERSIST` 才能跨重启。
pub fn disable_device(instance_id: &str) -> Result<(), String> {
    let devinst = locate_devinst(instance_id)?;
    let cr = unsafe { CM_Disable_DevNode(devinst, 0) };
    if cr != CR_SUCCESS {
        return Err(format!("CM_Disable_DevNode 失败: 0x{:08X}", cr.0));
    }
    // 复核：不信返回值，轮询 problem 22 真正出现（USB 重枚举窗口内查询可能失败，继续等）
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(ENABLE_VERIFY_TOTAL_MS);
    loop {
        match devnode_problem(instance_id) {
            Ok(problem) if problem == CM_PROB_DISABLED.0 => return Ok(()),
            _ if std::time::Instant::now() >= deadline => {
                return Err("CM_Disable_DevNode 已调用但设备未进入 PnP 禁用态(problem 22)".to_string());
            }
            _ => std::thread::sleep(std::time::Duration::from_millis(ENABLE_VERIFY_INTERVAL_MS)),
        }
    }
}

/// 按适配器名做非持久化设备级禁用（夜间出站切换用）。
///
/// 返回 `Ok(Some(instance_id))` = 已走设备级禁用；`Ok(None)` = 该适配器没有
/// PnP 实例 ID（Wintun/TAP 等虚拟卡），调用方回退 netsh——虚拟卡不涉及重启
/// 驱动安装问题，回退安全。
pub fn disable_adapter_nonpersistent(adapter_name: &str) -> Result<Option<String>, String> {
    let guid = crate::network::get_adapters_force()
        .ok()
        .and_then(|list| list.iter().find(|a| a.name == adapter_name).map(|a| a.guid.clone()))
        .or_else(|| find_guid_by_name(adapter_name));
    let Some(guid) = guid else {
        return Ok(None);
    };
    let Some(instance_id) = read_pnp_instance_id(&guid) else {
        return Ok(None);
    };
    disable_device(&instance_id)?;
    Ok(Some(instance_id))
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
    // USB 设备给更长窗口：设备级启用对它等价物理拔插，3 秒必然误判。
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_millis(verify_window_ms(instance_id));
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

/// 触发设备所在总线的重枚举（`CM_Reenumerate_DevNode(SYNCHRONOUS)`），
/// 等效「拔插一次」：让停在幽灵态（设备实例不可定位）的 USB 设备重新上线。
///
/// 为什么需要：USB 网卡被设备级禁用后，端口复位会让它在 PnP 树里彻底消失
/// （2026-10-21 实测：`CM_Locate_DevNodeW(PHANTOM)` 对
/// `USB\VID_0BDA&PID_8156\4013000001` 返回 0x0D CR_NO_SUCH_DEVNODE），
/// 此时 pnputil /enable-device 无从下手、netsh 找不到接口行，只有重扫总线才能
/// 把设备重新枚举出来。用户当时的「解法」正是在设备管理器里点「扫描检测硬件改动」。
///
/// 重枚举作用在**父设备**（USB hub）上，因此先取父 devinst；父不可得时退回
/// 本实例（对已消失的实例会失败，交由调用方报错）。
///
/// 只传 `CM_REENUMERATE_SYNCHRONOUS`，绝不传 `CM_REENUMERATE_RETRY_INSTALLATION`
/// ——官方警告后者可能弹出驱动安装提示。
///
/// 与 [`enable_device`] 对称：非管理员经 helper 框架由 SYSTEM worker 执行。
pub fn reenumerate_device(instance_id: &str, allow_uac_prompt: bool) -> Result<(), String> {
    if !crate::platform::elevation::is_admin() {
        let result_name = crate::platform::helper_spawn::new_result_name();
        let v = crate::platform::helper_spawn::spawn_elevated_helper(
            "reenumerate_device",
            &[instance_id],
            &result_name,
            // 15s 重枚举复核 + 通道触发开销；与 enable_device 的 30s 留出余量
            std::time::Duration::from_secs(60),
            allow_uac_prompt,
        )?;
        if !v.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
            return Err(v
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("总线重枚举失败（helper 无 message）")
                .to_string());
        }
        return Ok(());
    }

    reenumerate_device_raw(instance_id)
}

/// 重枚举执行体（管理员/SYSTEM 上下文内直接调用，不再走提权链）。
pub(crate) fn reenumerate_device_raw(instance_id: &str) -> Result<(), String> {
    let devinst = locate_devinst(instance_id)?;
    let mut parent = devinst;
    let cr = unsafe { CM_Get_Parent(&mut parent, devinst, 0) };
    if cr != CR_SUCCESS {
        // 父设备取不到（例如实例本身已是根节点）：对自身重枚举
        parent = devinst;
    }
    let cr = unsafe { CM_Reenumerate_DevNode(parent, CM_REENUMERATE_SYNCHRONOUS) };
    if cr != CR_SUCCESS {
        return Err(format!("CM_Reenumerate_DevNode 失败: 0x{:08X}", cr.0));
    }

    // 复核：等设备实例重新可定位（重枚举完成后 devnode 才回来）。
    // 这里不校验 problem——设备回来后可能仍处于 problem 22（禁用态被保留），
    // 那一步由调用方紧接着的 pnputil 设备级启用处理。
    let deadline = std::time::Instant::now()
        + std::time::Duration::from_millis(REENUMERATE_VERIFY_TOTAL_MS);
    loop {
        if devnode_problem(instance_id).is_ok() {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err("已触发总线重枚举，但设备实例仍未重新出现".to_string());
        }
        std::thread::sleep(std::time::Duration::from_millis(ENABLE_VERIFY_INTERVAL_MS));
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

    #[test]
    fn disable_adapter_nonpersistent_missing_adapter_returns_none() {
        // 不存在的适配器必须返回 Ok(None)（交给调用方回退 netsh），绝不能误伤真设备
        assert_eq!(disable_adapter_nonpersistent("__no_such_adapter__").unwrap(), None);
    }

    #[test]
    fn disable_device_nonexistent_instance_errors() {
        assert!(disable_device("ROOT\\__NO_SUCH_DEVICE__\\0000").is_err());
    }

    #[test]
    fn verify_window_usb_is_longer_than_generic() {
        // USB 设备等价物理拔插，复核窗口必须显著长于普通设备
        assert_eq!(verify_window_ms("USB\\VID_0BDA&PID_8156\\4013000001"), USB_VERIFY_TOTAL_MS);
        assert_eq!(verify_window_ms("usb\\vid_0bda&pid_8156\\4013000001"), USB_VERIFY_TOTAL_MS);
        assert_eq!(verify_window_ms("PCI\\VEN_10EC&DEV_8168\\0000"), ENABLE_VERIFY_TOTAL_MS);
        assert!(USB_VERIFY_TOTAL_MS > ENABLE_VERIFY_TOTAL_MS);
    }

    #[test]
    fn reenumerate_nonexistent_instance_errors() {
        // 幽灵态重枚举的前提是实例可定位；虚构 ID 必须先失败而非静默成功
        assert!(reenumerate_device_raw("ROOT\\__NO_SUCH_DEVICE__\\0000").is_err());
    }
}
