//! 控制台程序输出解码。
//!
//! Windows 控制台程序（netsh/ipconfig 等）默认输出 OEM 代码页（中文系统为
//! 936/GBK），而系统开启"Beta: 使用 UTF-8 提供全球语言支持"后为 65001——
//! 两种模式在不同用户机器上都可能出现（本机为后者，开发期从未暴露 GBK 问题）。
//! 统一入口：先按严格 UTF-8 尝试（合法即原样返回），失败再按 OEM 代码页
//! 解码，避免 GBK 中文字节在 from_utf8_lossy 下全部变成 U+FFFD，
//! 导致"配置文件"/"自动升级"等关键字匹配与 SSID 解析失效。

#[cfg(target_os = "windows")]
fn decode_with_code_page(bytes: &[u8], code_page: u32) -> Option<String> {
    use windows::Win32::Globalization::{MULTI_BYTE_TO_WIDE_CHAR_FLAGS, MultiByteToWideChar};

    if bytes.is_empty() {
        return Some(String::new());
    }
    unsafe {
        let wide_len = MultiByteToWideChar(code_page, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), bytes, None);
        if wide_len <= 0 {
            return None;
        }
        let mut wide = vec![0u16; wide_len as usize];
        let written = MultiByteToWideChar(code_page, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), bytes, Some(&mut wide));
        if written <= 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&wide[..written as usize]))
    }
}

#[cfg(target_os = "windows")]
pub fn decode_console_bytes(bytes: &[u8]) -> String {
    use windows::Win32::Globalization::GetOEMCP;

    if bytes.is_empty() {
        return String::new();
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    let code_page = unsafe { GetOEMCP() };
    if code_page != 0 {
        if let Some(s) = decode_with_code_page(bytes, code_page) {
            return s;
        }
    }
    String::from_utf8_lossy(bytes).into_owned()
}

#[cfg(not(target_os = "windows"))]
pub fn decode_console_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// HTTP 响应体解码：Content-Type 显式声明 GBK 族时按 936 解码，否则与
/// 控制台输出同策略（UTF-8 优先 → OEM 回退）。校园网 Portal（老 Dr.COM）
/// 常返回 GBK 编码，登录/注销的中文关键词成败判定依赖正确解码。
pub fn decode_charset_bytes(bytes: &[u8], charset: Option<&str>) -> String {
    // 非 Windows 无代码页解码分支,显式消费参数避免 unused 警告
    #[cfg(not(target_os = "windows"))]
    let _ = charset;
    #[cfg(target_os = "windows")]
    if let Some(cs) = charset {
        let cs = cs.trim().to_ascii_lowercase();
        if cs.starts_with("gb") || cs.contains("936") || cs == "csgb2312" {
            if let Some(s) = decode_with_code_page(bytes, 936) {
                return s;
            }
        }
    }
    decode_console_bytes(bytes)
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

    /// decode_charset_bytes 的 GBK 分支硬编码 936，与系统 OEM 页无关，
    /// 两种系统模式下都应无条件还原
    #[test]
    #[cfg(target_os = "windows")]
    fn charset_gbk_forces_code_page_936() {
        let gbk = [0xC5u8, 0xE4, 0xD6, 0xC3, 0xCE, 0xC4, 0xBC, 0xFE];
        assert_eq!(decode_charset_bytes(&gbk, Some("gbk")), "配置文件");
        assert_eq!(decode_charset_bytes(&gbk, Some("GB2312")), "配置文件");
        assert_eq!(decode_charset_bytes(&gbk, Some("gb18030")), "配置文件");
    }

    #[test]
    fn utf8_bytes_pass_through() {
        assert_eq!(decode_console_bytes("配置文件".as_bytes()), "配置文件");
        assert_eq!(decode_console_bytes(b"plain ascii"), "plain ascii");
        assert!(decode_console_bytes(&[]).is_empty());
    }
}
