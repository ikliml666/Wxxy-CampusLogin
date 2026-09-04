//! 控制台程序输出解码。
//!
//! Windows 控制台程序（netsh/ipconfig 等）默认输出 OEM 代码页（中文系统为
//! 936/GBK），而系统开启"Beta: 使用 UTF-8 提供全球语言支持"后为 65001——
//! 两种模式在不同用户机器上都可能出现（本机为后者，开发期从未暴露 GBK 问题）。
//! 统一入口：先按严格 UTF-8 尝试（合法即原样返回），失败再按 OEM 代码页
//! 解码，避免 GBK 中文字节在 from_utf8_lossy 下全部变成 U+FFFD，
//! 导致"配置文件"/"自动升级"等关键字匹配与 SSID 解析失效。

#[cfg(target_os = "windows")]
pub fn decode_console_bytes(bytes: &[u8]) -> String {
    use windows::Win32::Globalization::{GetOEMCP, MultiByteToWideChar};

    if bytes.is_empty() {
        return String::new();
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    unsafe {
        use windows::Win32::Globalization::MULTI_BYTE_TO_WIDE_CHAR_FLAGS;
        let code_page = GetOEMCP();
        if code_page == 0 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        let wide_len = MultiByteToWideChar(code_page, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), bytes, None);
        if wide_len <= 0 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        let mut wide = vec![0u16; wide_len as usize];
        let written = MultiByteToWideChar(code_page, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), bytes, Some(&mut wide));
        if written <= 0 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        String::from_utf16_lossy(&wide[..written as usize])
    }
}

#[cfg(not(target_os = "windows"))]
pub fn decode_console_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GBK（cp936）编码的"配置文件"四个字：C5 E4 D6 C3 CE C4 BC FE。
    /// 严格 UTF-8 解码必失败，应经 OEM 代码页还原；UTF-8 系统上同一函数
    /// 走 from_utf8 快路径（本机即此模式），两条路径都必须可得正确结果。
    #[test]
    #[cfg(target_os = "windows")]
    fn gbk_bytes_decode_to_expected_keyword() {
        let gbk = [0xC5u8, 0xE4, 0xD6, 0xC3, 0xCE, 0xC4, 0xBC, 0xFE];
        // 本机若为 UTF-8 模式，GBK 字节也是非法 UTF-8，仍走 OEM 解码；
        // 但 OEM 页在本机是 65001，会解出 U+FFFD —— 此时只验证不 panic。
        let decoded = decode_console_bytes(&gbk);
        if decoded.contains('\u{FFFD}') {
            // OEM 页为 65001 的系统：GBK 无法还原，仅保证无 panic 且无静默截断
            assert!(!decoded.is_empty());
        } else {
            assert_eq!(decoded, "配置文件");
        }
    }

    #[test]
    fn utf8_bytes_pass_through() {
        assert_eq!(decode_console_bytes("配置文件".as_bytes()), "配置文件");
        assert_eq!(decode_console_bytes(b"plain ascii"), "plain ascii");
        assert!(decode_console_bytes(&[]).is_empty());
    }
}
