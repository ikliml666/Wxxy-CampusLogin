//! Windows 本地身份验证：查看敏感信息（运营商账户密码）前的用户身份确认。
//!
//! 主路径：Windows Hello（指纹/面部/Hello PIN），走 WinRT `UserConsentVerifier`；
//! 设备未配置 Hello 时回退：Windows 凭据对话框（`CredUIPromptForCredentialsW`）
//! 收集 Windows 账号密码，再用 SSPI NTLM 往返校验密码正确性。
//! 注意：不能用 `LogonUser` 做校验——它要求调用进程持有 SE_TCB_NAME 特权，
//! 普通桌面进程必然失败；SSPI `AcceptSecurityContext` 是无特权校验的标准做法。

use windows::core::HSTRING;
use windows::Win32::Security::Credentials::SecHandle;
use windows::Win32::Security::Authentication::Identity::{
    DeleteSecurityContext, FreeCredentialsHandle,
};

const CONSENT_MESSAGE: &str = "查看运营商账户密码，请完成身份验证";

/// 验证当前 Windows 用户身份。返回 Ok 表示通过；Err 为可展示给用户的失败原因。
pub fn verify_identity() -> Result<(), String> {
    match verify_hello() {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(e) => crate::log_warn!("identity", "Windows Hello 验证异常，转凭据对话框: {e}"),
    }
    verify_by_password_dialog()
}

/// Windows Hello（指纹/面部/Hello PIN）。Ok(false) = 设备未配置/不可用。
fn verify_hello() -> Result<bool, String> {
    use windows::Security::Credentials::UI::{
        UserConsentVerificationResult, UserConsentVerifier, UserConsentVerifierAvailability,
    };

    let availability = {
        UserConsentVerifier::CheckAvailabilityAsync()
            .map_err(|e| format!("Hello 可用性查询失败: {e}"))?
            .get()
            .map_err(|e| format!("Hello 可用性查询失败: {e}"))?
    };
    if availability != UserConsentVerifierAvailability::Available {
        return Ok(false);
    }

    let result = {
        UserConsentVerifier::RequestVerificationAsync(&HSTRING::from(CONSENT_MESSAGE))
            .map_err(|e| format!("Hello 验证请求失败: {e}"))?
            .get()
            .map_err(|e| format!("Hello 验证请求失败: {e}"))?
    };
    if result == UserConsentVerificationResult::Verified {
        Ok(true)
    } else {
        Err("Windows Hello 验证未通过".to_string())
    }
}

/// 回退路径：Windows 凭据对话框收集账号密码 + SSPI NTLM 本地校验
fn verify_by_password_dialog() -> Result<(), String> {
    let (username, password) = credui_collect_credentials()?;
    sspi_verify_credentials(&username, &password)
}

/// 弹出 Windows 凭据对话框收集用户名/密码
fn credui_collect_credentials() -> Result<(String, String), String> {
    use windows::Win32::Foundation::WIN32_ERROR;
    use windows::Win32::Graphics::Gdi::HBITMAP;
    use windows::Win32::Security::Credentials::{
        CredUIPromptForCredentialsW, CREDUI_FLAGS_ALWAYS_SHOW_UI,
        CREDUI_FLAGS_DO_NOT_PERSIST, CREDUI_FLAGS_GENERIC_CREDENTIALS, CREDUI_INFOW,
    };
    use windows::Win32::Foundation::HWND;

    const CREDUI_MAX_USERNAME_LEN: usize = 513;
    const CREDUI_MAX_PASSWORD_LEN: usize = 256;
    const ERROR_CANCELLED: WIN32_ERROR = WIN32_ERROR(1223u32);

    unsafe {
        let mut username = vec![0u16; CREDUI_MAX_USERNAME_LEN];
        let mut password = vec![0u16; CREDUI_MAX_PASSWORD_LEN];
        let mut save = windows::Win32::Foundation::BOOL::default();
        let message = HSTRING::from(CONSENT_MESSAGE);
        let caption = HSTRING::from("Windows 身份验证");
        let cred_info = CREDUI_INFOW {
            cbSize: std::mem::size_of::<CREDUI_INFOW>() as u32,
            hwndParent: HWND::default(),
            pszMessageText: windows::core::PCWSTR(message.as_ptr()),
            pszCaptionText: windows::core::PCWSTR(caption.as_ptr()),
            hbmBanner: HBITMAP::default(),
        };

        let status = CredUIPromptForCredentialsW(
            Some(&cred_info),
            &HSTRING::from("CampusLogin"),
            None,
            0,
            &mut username,
            &mut password,
            Some(&mut save),
            CREDUI_FLAGS_GENERIC_CREDENTIALS
                | CREDUI_FLAGS_DO_NOT_PERSIST
                | CREDUI_FLAGS_ALWAYS_SHOW_UI,
        );

        if status == ERROR_CANCELLED {
            return Err("已取消身份验证".to_string());
        }
        if status != WIN32_ERROR(0) {
            return Err(format!("凭据对话框失败 (0x{:08X})", status.0));
        }

        Ok((wide_to_string(&username), wide_to_string(&password)))
    }
}

/// SSPI NTLM 往返校验 Windows 账号密码（本地 SAM，无特权要求）。
/// 密码错误时服务端 AcceptSecurityContext 返回 SEC_E_LOGON_DENIED。
fn sspi_verify_credentials(username: &str, password: &str) -> Result<(), String> {
    use windows::Win32::Foundation::SEC_E_OK;
    use windows::Win32::Security::Authentication::Identity::{
        AcquireCredentialsHandleW, SECPKG_CRED_INBOUND, SECPKG_CRED_OUTBOUND,
    };
    use windows::Win32::System::Rpc::{
        SEC_WINNT_AUTH_IDENTITY_UNICODE, SEC_WINNT_AUTH_IDENTITY_W,
    };

    unsafe {
        // 拆分 "域\用户"（本地账户域可省略，NTLM 按本地 SAM 解析）
        let (domain, user) = match username.split_once('\\') {
            Some((d, u)) => (d.to_string(), u.to_string()),
            None => (String::new(), username.to_string()),
        };
        let mut user_w = to_wide(&user);
        let mut domain_w = to_wide(&domain);
        let mut pass_w = to_wide(password);

        let mut identity = SEC_WINNT_AUTH_IDENTITY_W {
            User: user_w.as_mut_ptr(),
            UserLength: user_w.len().saturating_sub(1) as u32,
            Domain: domain_w.as_mut_ptr(),
            DomainLength: domain_w.len().saturating_sub(1) as u32,
            Password: pass_w.as_mut_ptr(),
            PasswordLength: pass_w.len().saturating_sub(1) as u32,
            Flags: SEC_WINNT_AUTH_IDENTITY_UNICODE,
        };

        let pkg = HSTRING::from("NTLM");
        let target = HSTRING::from("NTLM");
        let mut cred_client = SecHandle::default();
        let mut cred_server = SecHandle::default();
        let mut expiry: i64 = 0;

        AcquireCredentialsHandleW(
            None,
            &pkg,
            SECPKG_CRED_OUTBOUND,
            None,
            Some(&mut identity as *mut SEC_WINNT_AUTH_IDENTITY_W as *const std::ffi::c_void),
            None,
            None,
            &mut cred_client,
            Some(&mut expiry),
        )
        .map_err(|e| format!("凭据句柄获取失败: {e}"))?;
        AcquireCredentialsHandleW(
            None,
            &pkg,
            SECPKG_CRED_INBOUND,
            None,
            None,
            None,
            None,
            &mut cred_server,
            Some(&mut expiry),
        )
        .map_err(|e| format!("凭据句柄获取失败: {e}"))?;

        let mut ctx_client = SecHandle::default();
        let mut ctx_server = SecHandle::default();
        let mut client_has_ctx = false;
        let mut server_has_ctx = false;

        let result = (|| -> Result<(), String> {
            // 客户端第一轮：生成 NTLM Type1
            let (_, mut token) = client_step(
                &cred_client, &mut ctx_client, &mut client_has_ctx,
                &target, None, &mut expiry,
            )?;
            let mut rounds = 0u8;

            loop {
                rounds += 1;
                if rounds > 8 {
                    return Err("SSPI 校验轮次异常".to_string());
                }
                // 服务端消费客户端令牌
                let (st_server, server_out) = server_step(
                    &cred_server, &mut ctx_server, &mut server_has_ctx,
                    token.as_deref(), &mut expiry,
                )?;
                if st_server == SEC_E_OK && token.is_none() {
                    return Ok(());
                }
                // 客户端消费服务端令牌
                let (st_client, client_out) = client_step(
                    &cred_client, &mut ctx_client, &mut client_has_ctx,
                    &target, server_out.as_deref(), &mut expiry,
                )?;
                token = client_out;
                if st_client == SEC_E_OK && token.is_none() {
                    // 客户端完成且无新令牌：服务端需再收尾一次
                    let (st_final, final_out) = server_step(
                        &cred_server, &mut ctx_server, &mut server_has_ctx,
                        None, &mut expiry,
                    )?;
                    if st_final == SEC_E_OK && final_out.is_none() {
                        return Ok(());
                    }
                    return Err("SSPI 校验未按预期完成".to_string());
                }
            }
        })();

            let _ = DeleteSecurityContext(&mut ctx_client);
            let _ = DeleteSecurityContext(&mut ctx_server);
            let _ = FreeCredentialsHandle(&mut cred_client);
            let _ = FreeCredentialsHandle(&mut cred_server);
        result
    }
}

unsafe fn client_step(
    cred: &SecHandle,
    ctx: &mut SecHandle,
    has_ctx: &mut bool,
    target: &HSTRING,
    input: Option<&[u8]>,
    expiry: &mut i64,
) -> Result<(windows::core::HRESULT, Option<Vec<u8>>), String> {
    use windows::Win32::Security::Authentication::Identity::{
        FreeContextBuffer, InitializeSecurityContextW, SecBuffer, SecBufferDesc,
        ISC_REQ_ALLOCATE_MEMORY, ISC_REQ_CONNECTION, ISC_REQ_FLAGS, SECBUFFER_TOKEN,
        SECBUFFER_VERSION,
        SECURITY_NATIVE_DREP,
    };
    use windows::Win32::Foundation::SEC_E_OK;

    fn make_in_desc(data: &[u8]) -> SecBufferDesc {
        SecBufferDesc {
            ulVersion: SECBUFFER_VERSION,
            cBuffers: 1,
            pBuffers: &mut SecBuffer {
                cbBuffer: data.len() as u32,
                BufferType: SECBUFFER_TOKEN,
                pvBuffer: data.as_ptr() as *mut _,
            },
        }
    }

    unsafe {
        let in_desc = input.map(make_in_desc);
        let mut out_buf = SecBuffer {
            cbBuffer: 0,
            BufferType: SECBUFFER_TOKEN,
            pvBuffer: std::ptr::null_mut(),
        };
        let mut out_desc = SecBufferDesc {
            ulVersion: SECBUFFER_VERSION,
            cBuffers: 1,
            pBuffers: &mut out_buf,
        };
        let mut attrs = 0u32;

        let st = InitializeSecurityContextW(
            Some(cred as *const SecHandle),
            if *has_ctx { Some(ctx as *const SecHandle) } else { None },
            Some(target.as_ptr()),
            ISC_REQ_FLAGS(ISC_REQ_ALLOCATE_MEMORY.0 | ISC_REQ_CONNECTION.0),
            0,
            SECURITY_NATIVE_DREP,
            in_desc.as_ref().map(|d| d as *const SecBufferDesc),
            0,
            Some(ctx as *mut SecHandle),
            Some(&mut out_desc),
            &mut attrs,
            Some(expiry),
        );
        *has_ctx = true;

        let payload = if out_buf.cbBuffer > 0 && !out_buf.pvBuffer.is_null() {
            let slice = std::slice::from_raw_parts(out_buf.pvBuffer as *const u8, out_buf.cbBuffer as usize);
            let out = slice.to_vec();
            let _ = FreeContextBuffer(out_buf.pvBuffer);
            Some(out)
        } else {
            None
        };

        if st != SEC_E_OK && st != windows::Win32::Foundation::SEC_I_CONTINUE_NEEDED {
            return Err(sspi_error("客户端", st));
        }
        Ok((st, payload))
    }
}

unsafe fn server_step(
    cred: &SecHandle,
    ctx: &mut SecHandle,
    has_ctx: &mut bool,
    input: Option<&[u8]>,
    expiry: &mut i64,
) -> Result<(windows::core::HRESULT, Option<Vec<u8>>), String> {
    use windows::Win32::Security::Authentication::Identity::{
        AcceptSecurityContext, FreeContextBuffer, SecBuffer, SecBufferDesc,
        ASC_REQ_ALLOCATE_MEMORY, ASC_REQ_CONNECTION, ASC_REQ_FLAGS, SECBUFFER_TOKEN,
        SECBUFFER_VERSION,
        SECURITY_NATIVE_DREP,
    };
    use windows::Win32::Foundation::SEC_E_OK;

    fn make_in_desc(data: &[u8]) -> SecBufferDesc {
        SecBufferDesc {
            ulVersion: SECBUFFER_VERSION,
            cBuffers: 1,
            pBuffers: &mut SecBuffer {
                cbBuffer: data.len() as u32,
                BufferType: SECBUFFER_TOKEN,
                pvBuffer: data.as_ptr() as *mut _,
            },
        }
    }

    unsafe {
        let in_desc = input.map(make_in_desc);
        let mut out_buf = SecBuffer {
            cbBuffer: 0,
            BufferType: SECBUFFER_TOKEN,
            pvBuffer: std::ptr::null_mut(),
        };
        let mut out_desc = SecBufferDesc {
            ulVersion: SECBUFFER_VERSION,
            cBuffers: 1,
            pBuffers: &mut out_buf,
        };
        let mut attrs = 0u32;

        let st = AcceptSecurityContext(
            Some(cred as *const SecHandle),
            if *has_ctx { Some(ctx as *const SecHandle) } else { None },
            in_desc.as_ref().map(|d| d as *const SecBufferDesc),
            ASC_REQ_FLAGS(ASC_REQ_ALLOCATE_MEMORY.0 | ASC_REQ_CONNECTION.0),
            SECURITY_NATIVE_DREP,
            Some(ctx as *mut SecHandle),
            Some(&mut out_desc),
            &mut attrs,
            Some(expiry),
        );
        *has_ctx = true;

        let payload = if out_buf.cbBuffer > 0 && !out_buf.pvBuffer.is_null() {
            let slice = std::slice::from_raw_parts(out_buf.pvBuffer as *const u8, out_buf.cbBuffer as usize);
            let out = slice.to_vec();
            let _ = FreeContextBuffer(out_buf.pvBuffer);
            Some(out)
        } else {
            None
        };

        if st != SEC_E_OK && st != windows::Win32::Foundation::SEC_I_CONTINUE_NEEDED {
            return Err(sspi_error("服务端", st));
        }
        Ok((st, payload))
    }
}

fn sspi_error(step: &str, st: windows::core::HRESULT) -> String {
    if st.0 == 0x8009030Cu32 as i32 {
        // SEC_E_LOGON_DENIED
        return "Windows 账号密码验证失败，请检查后重试".to_string();
    }
    format!("SSPI {step}步骤失败 (0x{:08X})", st.0 as u32)
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wide_to_string(wide: &[u16]) -> String {
    let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..end])
}
