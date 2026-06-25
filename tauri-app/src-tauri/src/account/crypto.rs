#[cfg(target_os = "windows")]
mod dpapi {
    use std::ptr;

    #[repr(C)]
    struct DataBlob {
        cb_data: u32,
        pb_data: *mut u8,
    }

    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            data_in: *mut DataBlob,
            data_descr: *const u16,
            optional_entropy: *mut DataBlob,
            reserved: *mut std::ffi::c_void,
            prompt_struct: *mut std::ffi::c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;

        fn CryptUnprotectData(
            data_in: *mut DataBlob,
            data_descr: *mut *mut u16,
            optional_entropy: *mut DataBlob,
            reserved: *mut std::ffi::c_void,
            prompt_struct: *mut std::ffi::c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(h_mem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }

    pub fn encrypt(plaintext: &[u8]) -> Result<Vec<u8>, String> {
        let mut plaintext_owned = plaintext.to_vec();
        let mut input = DataBlob {
            cb_data: plaintext_owned.len() as u32,
            pb_data: plaintext_owned.as_mut_ptr(),
        };
        let mut output = DataBlob {
            cb_data: 0,
            pb_data: ptr::null_mut(),
        };

        let result = unsafe {
            CryptProtectData(
                &mut input,
                ptr::null(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                &mut output,
            )
        };

        if result == 0 {
            return Err("DPAPI加密失败".to_string());
        }

        if output.pb_data.is_null() || output.cb_data == 0 {
            unsafe { LocalFree(output.pb_data as *mut std::ffi::c_void) };
            return Err("DPAPI加密返回空数据".to_string());
        }
        let encrypted = unsafe { std::slice::from_raw_parts(output.pb_data, output.cb_data as usize).to_vec() };
        unsafe { LocalFree(output.pb_data as *mut std::ffi::c_void) };
        Ok(encrypted)
    }

    pub fn decrypt(data: &[u8]) -> Result<Vec<u8>, String> {
        let mut data_owned = data.to_vec();
        let mut input = DataBlob {
            cb_data: data_owned.len() as u32,
            pb_data: data_owned.as_mut_ptr(),
        };
        let mut output = DataBlob {
            cb_data: 0,
            pb_data: ptr::null_mut(),
        };

        let result = unsafe {
            CryptUnprotectData(
                &mut input,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                &mut output,
            )
        };

        if result == 0 {
            return Err("DPAPI解密失败，可能需要重新输入密码".to_string());
        }

        if output.pb_data.is_null() || output.cb_data == 0 {
            unsafe { LocalFree(output.pb_data as *mut std::ffi::c_void) };
            return Err("DPAPI解密返回空数据".to_string());
        }
        let decrypted = unsafe { std::slice::from_raw_parts(output.pb_data, output.cb_data as usize).to_vec() };
        unsafe { LocalFree(output.pb_data as *mut std::ffi::c_void) };
        Ok(decrypted)
    }
}

#[cfg(target_os = "windows")]
pub fn encrypt(plaintext: &str) -> Result<String, String> {
    let bytes = plaintext.as_bytes();
    let encrypted = dpapi::encrypt(bytes)?;
    Ok(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &encrypted,
    ))
}

#[cfg(target_os = "windows")]
pub fn decrypt(encrypted_base64: &str) -> Result<String, String> {
    let data = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encrypted_base64)
        .map_err(|e| format!("Base64解码失败: {e}"))?;
    let decrypted = dpapi::decrypt(&data)?;
    String::from_utf8(decrypted).map_err(|e| format!("UTF8转换失败: {e}"))
}
