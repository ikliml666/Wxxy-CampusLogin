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

    /// 审计 A10：DPAPI additional entropy。与机器+用户作用域叠加，使从本机拷出的
    /// 密文（含同机其他用户会话经卷影/磁盘镜像获取）无法直接解密。
    /// 旧密文（无 entropy）在 decrypt 中兼容读取，下次保存自然升级为新格式。
    const DPAPI_ENTROPY: &[u8] = b"Wxxy-CampusLogin::dpapi-v1";

    /// 调用 DPAPI 函数（CryptProtectData/CryptUnprotectData）的通用 helper，
    /// 统一 DataBlob 构造、结果检查、输出数据读取与 LocalFree 释放。
    fn call_dpapi<F>(input_data: &[u8], dpapi_call: F, error_msg: &str, empty_error_msg: &str) -> Result<Vec<u8>, String>
    where
        F: FnOnce(&mut DataBlob, &mut DataBlob) -> i32,
    {
        let mut input_owned = input_data.to_vec();
        let mut input = DataBlob {
            cb_data: input_owned.len() as u32,
            pb_data: input_owned.as_mut_ptr(),
        };
        let mut output = DataBlob {
            cb_data: 0,
            pb_data: ptr::null_mut(),
        };

        let result = dpapi_call(&mut input, &mut output);

        if result == 0 {
            // 失败路径也要释放输出缓冲：多数实现失败时 pb_data 为 null，
            // 但契约上不保证，判空后与成功路径同款清理
            if !output.pb_data.is_null() {
                unsafe { LocalFree(output.pb_data as *mut std::ffi::c_void) };
            }
            return Err(error_msg.to_string());
        }

        if output.pb_data.is_null() || output.cb_data == 0 {
            unsafe { LocalFree(output.pb_data as *mut std::ffi::c_void) };
            return Err(empty_error_msg.to_string());
        }
        let data = unsafe { std::slice::from_raw_parts(output.pb_data, output.cb_data as usize).to_vec() };
        unsafe { LocalFree(output.pb_data as *mut std::ffi::c_void) };
        Ok(data)
    }

    /// 用 optional_entropy 构造 DataBlob（调用期间指针必须保持有效）。
    fn entropy_blob() -> DataBlob {
        DataBlob {
            cb_data: DPAPI_ENTROPY.len() as u32,
            pb_data: DPAPI_ENTROPY.as_ptr() as *mut u8,
        }
    }

    pub fn encrypt(plaintext: &[u8]) -> Result<Vec<u8>, String> {
        call_dpapi(
            plaintext,
            |input, output| unsafe {
                let mut entropy = entropy_blob();
                CryptProtectData(
                    input,
                    ptr::null(),
                    &mut entropy,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    0,
                    output,
                )
            },
            "DPAPI加密失败",
            "DPAPI加密返回空数据",
        )
    }

    pub fn decrypt(data: &[u8]) -> Result<Vec<u8>, String> {
        // 审计 A10：先按新格式（带 entropy）解密；失败则回退旧格式（无 entropy），
        // 兼容升级前的存量密文。读旧格式成功后，下次保存自然升级。
        let with_entropy = call_dpapi(
            data,
            |input, output| unsafe {
                let mut entropy = entropy_blob();
                CryptUnprotectData(
                    input,
                    ptr::null_mut(),
                    &mut entropy,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    0,
                    output,
                )
            },
            "DPAPI解密失败，可能需要重新输入密码",
            "DPAPI解密返回空数据",
        );
        if with_entropy.is_ok() {
            return with_entropy;
        }
        call_dpapi(
            data,
            |input, output| unsafe {
                CryptUnprotectData(
                    input,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    0,
                    output,
                )
            },
            "DPAPI解密失败，可能需要重新输入密码",
            "DPAPI解密返回空数据",
        )
    }

    /// 仅测试用：无 entropy 的旧格式加密，模拟升级前存量密文（审计 A10 回退路径验证）。
    #[cfg(test)]
    pub fn encrypt_legacy_no_entropy(plaintext: &[u8]) -> Result<Vec<u8>, String> {
        call_dpapi(
            plaintext,
            |input, output| unsafe {
                CryptProtectData(
                    input,
                    ptr::null(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    0,
                    output,
                )
            },
            "DPAPI加密失败",
            "DPAPI加密返回空数据",
        )
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

// 非桌面平台无 DPAPI:阶段 1 安卓密码不落盘,加密路径不触达;阶段 2 换 Android Keystore
#[cfg(not(target_os = "windows"))]
pub fn encrypt(_plaintext: &str) -> Result<String, String> {
    Err("加密存储仅桌面端支持".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn decrypt(_encrypted_base64: &str) -> Result<String, String> {
    Err("加密存储仅桌面端支持".to_string())
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    /// 审计 A10：新格式（带 entropy）加解密往返。
    #[test]
    fn dpapi_roundtrip_with_entropy() {
        let plain = "p@ssw0rd-熵-测试";
        let enc = encrypt(plain).unwrap();
        assert_eq!(decrypt(&enc).unwrap(), plain);
    }

    /// 审计 A10：旧格式（无 entropy，历史 CryptProtectData(null entropy) 产出）
    /// 必须仍可解密——decrypt 先试新 entropy 失败后回退无 entropy 路径。
    #[test]
    fn dpapi_decrypt_falls_back_to_legacy_no_entropy() {
        let plain = "legacy-password";
        // 直接对明文做无 entropy 的 CryptProtectData，模拟升级前存量密文
        let legacy_ciphertext = dpapi_encrypt_no_entropy(plain.as_bytes()).unwrap();
        let enc = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &legacy_ciphertext);
        assert_eq!(decrypt(&enc).unwrap(), plain);
    }

    /// 直接调用 CryptProtectData（无 entropy）生成旧格式密文。
    fn dpapi_encrypt_no_entropy(data: &[u8]) -> Result<Vec<u8>, String> {
        // 复用 dpapi::call_dpapi 的通用路径，但绕开 entropy
        dpapi::encrypt_legacy_no_entropy(data)
    }
}
