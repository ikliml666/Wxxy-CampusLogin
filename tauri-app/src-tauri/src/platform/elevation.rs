//! 提权基础：管理员判定、UAC 环境画像、COM 静默提权（CMSTPLUA）、runas 弹 UAC。
//!
//! 两条 Win32 提权调用都可能在「UAC 等待用户确认」期间无限阻塞：安全桌面上的
//! 同意/凭据框不点就不返回，ShellExecuteW 与 CoGetObject 都会一直等。自动场景
//! （适配器被禁用后自动启用、夜间切换）不能因为一个没人点的对话框而挂死，因此
//! 每次调用都放到独立线程里带超时等待；超时后调用方按失败处理并走降级链。

use std::time::Duration;

/// 单次 COM/runas 提权调用的等待上限（调用方另有整体超时预算，通常 30~90s）。
const ELEVATION_CALL_TIMEOUT: Duration = Duration::from_secs(45);

#[cfg(target_os = "windows")]
pub fn is_admin() -> bool {
    use windows::Win32::System::Threading::{OpenProcessToken, GetCurrentProcess};
    use windows::Win32::Security::{TOKEN_QUERY, GetTokenInformation, TokenElevation};
    use windows::Win32::Foundation::HANDLE;

    unsafe {
        let mut token: HANDLE = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation: u32 = 0;
        let mut returned = 0u32;
        let result = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<u32>() as u32,
            &mut returned,
        );
        let _ = windows::Win32::Foundation::CloseHandle(token);
        result.is_ok() && elevation != 0
    }
}

/// UAC 环境画像（进程内缓存一次：注册表策略在会话内不会变）。
#[cfg(target_os = "windows")]
#[derive(Debug, Clone, Copy)]
pub struct ElevationEnv {
    /// EnableLUA（默认 1）。
    pub uac_enabled: bool,
    /// ConsentPromptBehaviorAdmin（默认 5）。
    pub consent_behavior_admin: u32,
    /// 当前账户是否属于 Administrators 组（不管令牌是否提升）。
    pub admin_member: bool,
    /// 当前令牌是否已提升。
    pub elevated: bool,
}

#[cfg(target_os = "windows")]
impl ElevationEnv {
    /// 一行可读描述，用于失败信息与日志。
    pub fn describe(&self) -> String {
        if !self.uac_enabled {
            return format!(
                "UAC=关, 账户={}, 令牌={}",
                if self.admin_member { "管理员组" } else { "标准用户" },
                if self.elevated { "已提升" } else { "未提升" },
            );
        }
        let level = match self.consent_behavior_admin {
            0 => "从不通知",
            1 => "安全桌面要求凭据",
            2 => "始终通知(安全桌面同意)",
            3 => "凭据提示",
            4 => "同意提示",
            _ => "默认(程序更改时同意)",
        };
        format!(
            "UAC=开({level}), 账户={}, 令牌={}",
            if self.admin_member { "管理员组" } else { "标准用户" },
            if self.elevated { "已提升" } else { "未提升" },
        )
    }

    /// CMSTPLUA 静默提权是否大概率不弹 UAC。
    ///
    /// 只有「UAC 开 + 管理员组 + 未提升 + 档位不是始终通知/安全桌面凭据」才静默；
    /// 标准用户与最高档都会弹框（后者在安全桌面），不能当静默通道用。
    pub fn com_silent_likely(&self) -> bool {
        self.uac_enabled
            && self.admin_member
            && !self.elevated
            && !matches!(self.consent_behavior_admin, 1 | 2)
    }
}

/// 读取 UAC 环境画像（首次调用读注册表 + 令牌，之后用缓存）。
#[cfg(target_os = "windows")]
pub fn elevation_env() -> ElevationEnv {
    use std::sync::OnceLock;
    static CACHE: OnceLock<ElevationEnv> = OnceLock::new();
    *CACHE.get_or_init(|| {
        let elevated = is_admin();
        // TokenElevationType 才是可靠的分割令牌判据：Limited = 管理员组但未提升，
        // Default = 标准用户或 UAC 关闭（后者由 elevated 兜底）。IsUserAnAdmin 在
        // 部分分割令牌下对管理员账户返回 FALSE，会把管理员误判成标准用户。
        let admin_member = match token_elevation_type() {
            Some(t) => t != windows::Win32::Security::TokenElevationTypeDefault || elevated,
            None => elevated,
        };
        ElevationEnv {
            uac_enabled: read_policy_u32("EnableLUA").unwrap_or(1) != 0,
            consent_behavior_admin: read_policy_u32("ConsentPromptBehaviorAdmin").unwrap_or(5),
            admin_member,
            elevated,
        }
    })
}

#[cfg(target_os = "windows")]
fn token_elevation_type() -> Option<windows::Win32::Security::TOKEN_ELEVATION_TYPE> {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{GetTokenInformation, TokenElevationType, TOKEN_ELEVATION_TYPE, TOKEN_QUERY};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token: HANDLE = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).ok()?;
        let mut kind: TOKEN_ELEVATION_TYPE = std::mem::zeroed();
        let mut returned = 0u32;
        let r = GetTokenInformation(
            token,
            TokenElevationType,
            Some(&mut kind as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION_TYPE>() as u32,
            &mut returned,
        );
        let _ = CloseHandle(token);
        r.ok().map(|_| kind)
    }
}

#[cfg(target_os = "windows")]
fn read_policy_u32(name: &str) -> Option<u32> {
    let key = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System")
        .ok()?;
    key.get_value::<u32, _>(name).ok()
}

/// 在独立线程里执行一次提权调用，超时即放弃（线程可能仍阻塞在 UAC 界面）。
fn run_with_timeout<T, F>(timeout: Duration, what: &str, f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(timeout) {
        Ok(r) => r,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(format!(
            "{what} 在 {} 秒内未返回（UAC 提示未响应或系统繁忙）",
            timeout.as_secs()
        )),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            Err(format!("{what} 执行线程异常退出"))
        }
    }
}

#[cfg(target_os = "windows")]
pub fn run_elevated(cmd: &str, args: &str) -> Result<(), String> {
    let cmd = cmd.to_string();
    let args = args.to_string();
    run_with_timeout(ELEVATION_CALL_TIMEOUT, "ShellExecuteW runas", move || {
        run_elevated_inner(&cmd, &args)
    })
}

#[cfg(target_os = "windows")]
fn run_elevated_inner(cmd: &str, args: &str) -> Result<(), String> {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::core::PCWSTR;

    let verb: Vec<u16> = "runas\0".encode_utf16().collect();
    let file: Vec<u16> = format!("{cmd}\0").encode_utf16().collect();
    let params: Vec<u16> = format!("{args}\0").encode_utf16().collect();

    unsafe {
        let result = ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR(params.as_ptr()),
            None,
            windows::Win32::UI::WindowsAndMessaging::SW_HIDE,
        );
        let val = result.0 as isize;
        if val <= 32 {
            return Err(format!("ShellExecuteW runas 失败，错误码: {val}"));
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn shell_exec_elevated(
    file: &str,
    params: &str,
    hide_window: bool,
) -> Result<(), String> {
    let file = file.to_string();
    let params = params.to_string();
    run_with_timeout(ELEVATION_CALL_TIMEOUT, "COM 静默提权", move || {
        shell_exec_elevated_inner(&file, &params, hide_window)
    })
}

#[cfg(target_os = "windows")]
fn shell_exec_elevated_inner(
    file: &str,
    params: &str,
    hide_window: bool,
) -> Result<(), String> {
    use windows::Win32::System::Com::CoInitializeEx;
    use windows::Win32::System::Com::CoUninitialize;
    use windows::Win32::System::Com::COINIT_APARTMENTTHREADED;

    unsafe {
        // CoInitializeEx 成功（S_OK/S_FALSE）须在每个退出路径前配对 CoUninitialize；
        // 失败（如 RPC_E_CHANGED_MODE）不得释放
        let co_initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();

        let moniker_name = "Elevation:Administrator!new:{3E5FC7F9-9A51-4367-9063-A120244FBEC7}";
        let moniker_wide: Vec<u16> = moniker_name.encode_utf16().chain(std::iter::once(0)).collect();

        let iid_icmluautil: windows::core::GUID = windows::core::GUID::from_values(
            0x6EDD6D74, 0xC007, 0x4E75, [0xB7, 0x6A, 0xE5, 0x74, 0x09, 0x95, 0xE2, 0x4C],
        );

        // elevation moniker 要求 BIND_OPTS3（x64 48 字节）：cbStruct 必须=完整结构体大小，
        // 且 dwClassContext=CLSCTX_LOCAL_SERVER——历史缺陷：传的是 BIND_OPTS（16 字节），
        // cbStruct 过小被 appinfo 按「提升激活配置无效」拒绝（0x80080017 CO_E_ELEVATION_DISABLED），
        // 静默提权从未真正通过过（此前被手动路径的 runas 降级掩盖）。
        let mut bind_opts: windows::Win32::System::Com::BIND_OPTS3 = std::mem::zeroed();
        bind_opts.Base.Base.cbStruct =
            std::mem::size_of::<windows::Win32::System::Com::BIND_OPTS3>() as u32;
        bind_opts.Base.dwClassContext = windows::Win32::System::Com::CLSCTX_LOCAL_SERVER.0;

        let mut p_unknown: *mut std::ffi::c_void = std::ptr::null_mut();

        let hr = co_get_object_raw(
            moniker_wide.as_ptr(),
            &mut bind_opts as *mut _ as *mut _,
            &iid_icmluautil as *const _ as *const _,
            &mut p_unknown as *mut _ as *mut _,
        );

        if hr != 0 || p_unknown.is_null() {
            if co_initialized {
                CoUninitialize();
            }
            return Err(format!("COM提权失败: HRESULT=0x{:08X}", hr as u32));
        }

        let vtbl = *(p_unknown as *mut *const ICMLuaUtilVtbl);

        let file_wide: Vec<u16> = file.encode_utf16().chain(std::iter::once(0)).collect();
        let params_wide: Vec<u16> = params.encode_utf16().chain(std::iter::once(0)).collect();
        let n_show: u32 = if hide_window { 0 } else { 1 };

        let result = ((*vtbl).shell_exec)(
            p_unknown,
            file_wide.as_ptr(),
            params_wide.as_ptr(),
            std::ptr::null(),
            0,
            n_show,
        );

        ((*vtbl).release)(p_unknown);

        if result.is_err() {
            if co_initialized {
                CoUninitialize();
            }
            return Err(format!("ShellExec 失败: {result:?}"));
        }

        if co_initialized {
            CoUninitialize();
        }

        Ok(())
    }
}

#[cfg(target_os = "windows")]
#[allow(non_snake_case)]
unsafe fn co_get_object_raw(
    pszname: *const u16,
    pbindoptions: *mut std::ffi::c_void,
    riid: *const std::ffi::c_void,
    ppv: *mut *mut std::ffi::c_void,
) -> i32 {
    #[link(name = "ole32")]
    extern "system" {
        fn CoGetObject(
            pszname: *const u16,
            pbindoptions: *mut std::ffi::c_void,
            riid: *const std::ffi::c_void,
            ppv: *mut *mut std::ffi::c_void,
        ) -> i32;
    }
    CoGetObject(pszname, pbindoptions, riid, ppv)
}

#[cfg(target_os = "windows")]
pub(crate) fn parse_guid(s: &str) -> Result<windows::core::GUID, String> {
    let s = s.trim().trim_start_matches('{').trim_end_matches('}');
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 {
        return Err(format!("GUID格式无效: {s}"));
    }
    let data1 = u32::from_str_radix(parts[0], 16).map_err(|e| format!("GUID data1解析失败: {e}"))?;
    let data2 = u16::from_str_radix(parts[1], 16).map_err(|e| format!("GUID data2解析失败: {e}"))?;
    let data3 = u16::from_str_radix(parts[2], 16).map_err(|e| format!("GUID data3解析失败: {e}"))?;
    let data4_str = &parts[3..=4].join("");
    if data4_str.len() != 16 {
        return Err(format!("GUID data4长度无效: {data4_str}"));
    }
    let mut data4 = [0u8; 8];
    for i in 0..8 {
        data4[i] = u8::from_str_radix(&data4_str[i * 2..i * 2 + 2], 16)
            .map_err(|e| format!("GUID data4[{i}]解析失败: {e}"))?;
    }
    Ok(windows::core::GUID::from_values(data1, data2, data3, data4))
}

#[repr(C)]
struct ICMLuaUtilVtbl {
    _query_interface: usize,
    _add_ref: usize,
    release: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    // slot 3~8：SetRasCredentials / SetRasEntryProperties / DeleteRasEntry /
    // LaunchInfSection / LaunchInfSectionEx / CreateLayerDirectory（不调用，仅占位对齐）。
    // 历史缺陷：曾被当作 6 个"占位"砍掉、shell_exec 错排到 slot 4——slot 4 实为
    // SetRasEntryProperties，参数 marshaling 不匹配报 RPC_X_BAD_STUB_DATA(0x800706F4)。
    // 真实布局以 UACMe elvint.h 的 ICMLuaUtilVtbl 为准：ShellExec 在 slot 9。
    _slots_3_to_8: [usize; 6],
    shell_exec: unsafe extern "system" fn(*mut std::ffi::c_void, *const u16, *const u16, *const u16, u32, u32) -> windows::core::HRESULT,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真实 ICMLuaUtil 布局（UACMe elvint.h 权威定义）：
    /// QueryInterface(0) AddRef(1) Release(2) SetRasCredentials(3) SetRasEntryProperties(4)
    /// DeleteRasEntry(5) LaunchInfSection(6) LaunchInfSectionEx(7) CreateLayerDirectory(8)
    /// ShellExec(9)。
    /// 历史缺陷：ShellExec 曾被错误排在 slot 4（实为 SetRasEntryProperties），
    /// 经代理调用报 RPC_X_BAD_STUB_DATA；slot 错位由 vtable 越界 UB 转为签名不匹配。
    #[test]
    fn vtbl_shell_exec_slot_is_9() {
        let slot = std::mem::offset_of!(ICMLuaUtilVtbl, shell_exec) / std::mem::size_of::<usize>();
        assert_eq!(slot, 9, "shell_exec 必须在 vtable slot 9，当前位于 slot {slot}");
    }

    /// 档位判定：只有默认/较低档且管理员未提升时才可能静默。
    #[cfg(target_os = "windows")]
    #[test]
    fn com_silent_likely_only_for_low_prompt_levels() {
        let base = ElevationEnv {
            uac_enabled: true,
            consent_behavior_admin: 5,
            admin_member: true,
            elevated: false,
        };
        assert!(base.com_silent_likely());
        assert!(!ElevationEnv { consent_behavior_admin: 2, ..base }.com_silent_likely());
        assert!(!ElevationEnv { consent_behavior_admin: 1, ..base }.com_silent_likely());
        assert!(!ElevationEnv { admin_member: false, ..base }.com_silent_likely());
        assert!(!ElevationEnv { elevated: true, ..base }.com_silent_likely());
    }

    /// 画像描述包含档位与账户类型，便于从失败信息直接判断环境。
    #[cfg(target_os = "windows")]
    #[test]
    fn env_describe_mentions_level_and_account() {
        let env = ElevationEnv {
            uac_enabled: true,
            consent_behavior_admin: 2,
            admin_member: false,
            elevated: false,
        };
        let d = env.describe();
        assert!(d.contains("始终通知"), "{d}");
        assert!(d.contains("标准用户"), "{d}");
    }

    /// 真实提权冒烟测试（默认忽略：会走真实提权通道，不进日常测试套件）：
    /// cargo test shell_exec_elevated_smoke -- --ignored --nocapture
    /// 经 CMSTPLUA 静默提权跑 cmd /c exit（无副作用），验证 BIND_OPTS3 参数修复。
    #[test]
    #[ignore = "真实提权冒烟测试，仅本机手动执行"]
    fn shell_exec_elevated_smoke() {
        shell_exec_elevated("cmd.exe", "/c exit", true)
            .expect("CMSTPLUA 静默提权应成功（BIND_OPTS3 参数修复验证）");
    }
}
