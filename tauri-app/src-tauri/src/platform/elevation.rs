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

#[cfg(target_os = "windows")]
pub fn run_elevated(cmd: &str, args: &str) -> Result<(), String> {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::core::PCWSTR;

    let verb: Vec<u16> = "runas\0".encode_utf16().collect();
    let file: Vec<u16> = format!("{}\0", cmd).encode_utf16().collect();
    let params: Vec<u16> = format!("{}\0", args).encode_utf16().collect();

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
            return Err(format!("ShellExecuteW runas 失败，错误码: {}", val));
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
    use windows::Win32::System::Com::CoInitializeEx;
    use windows::Win32::System::Com::COINIT_APARTMENTTHREADED;

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let moniker_name = "Elevation:Administrator!new:{3E5FC7F9-9A51-4367-9063-A120244FBEC7}";
        let moniker_wide: Vec<u16> = moniker_name.encode_utf16().chain(std::iter::once(0)).collect();

        let iid_icmluautil: windows::core::GUID = windows::core::GUID::from_values(
            0x6EDD6D74, 0xC007, 0x4E75, [0xB7, 0x6A, 0xE5, 0x74, 0x09, 0x95, 0xE2, 0x4C],
        );

        let mut bind_opts: windows::Win32::System::Com::BIND_OPTS = std::mem::zeroed();
        bind_opts.cbStruct = std::mem::size_of::<windows::Win32::System::Com::BIND_OPTS>() as u32;

        let mut p_unknown: *mut std::ffi::c_void = std::ptr::null_mut();

        let hr = co_get_object_raw(
            moniker_wide.as_ptr(),
            &mut bind_opts as *mut _ as *mut _,
            &iid_icmluautil as *const _ as *const _,
            &mut p_unknown as *mut _ as *mut _,
        );

        if hr != 0 || p_unknown.is_null() {
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
            return Err(format!("ShellExec 失败: {:?}", result));
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
        return Err(format!("GUID格式无效: {}", s));
    }
    let data1 = u32::from_str_radix(parts[0], 16).map_err(|e| format!("GUID data1解析失败: {}", e))?;
    let data2 = u16::from_str_radix(parts[1], 16).map_err(|e| format!("GUID data2解析失败: {}", e))?;
    let data3 = u16::from_str_radix(parts[2], 16).map_err(|e| format!("GUID data3解析失败: {}", e))?;
    let data4_str = &parts[3..=4].join("");
    if data4_str.len() != 16 {
        return Err(format!("GUID data4长度无效: {}", data4_str));
    }
    let mut data4 = [0u8; 8];
    for i in 0..8 {
        data4[i] = u8::from_str_radix(&data4_str[i * 2..i * 2 + 2], 16)
            .map_err(|e| format!("GUID data4[{}]解析失败: {}", i, e))?;
    }
    Ok(windows::core::GUID::from_values(data1, data2, data3, data4))
}

#[repr(C)]
struct ICMLuaUtilVtbl {
    _query_interface: usize,
    _add_ref: usize,
    release: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    _method1: usize,
    _method2: usize,
    _method3: usize,
    _method4: usize,
    _method5: usize,
    _method6: usize,
    shell_exec: unsafe extern "system" fn(*mut std::ffi::c_void, *const u16, *const u16, *const u16, u32, u32) -> windows::core::HRESULT,
}
