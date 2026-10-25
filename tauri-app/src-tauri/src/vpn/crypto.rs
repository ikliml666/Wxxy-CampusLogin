//! AES-128-CFB128 host 加解密（深澜 WebVPN 核心算法）。

use super::WebVpnError;
use cfb_mode::cipher::{AsyncStreamCipher, KeyIvInit};
use cfb_mode::{Decryptor, Encryptor};

/// 深澜 WebVPN 固定 128 位密钥。IV 由本密钥派生（恒等复制），
/// 输出前缀 hex(IV) 必须经这里计算得出，禁止在调用方硬编码该 hex 字符串。
const KEY: [u8; 16] = *b"wrdvpnisthebest!";

type Aes128CfbEnc = Encryptor<aes::Aes128>;
// 解密侧无生产调用方（本端只做 URL 包装加密），保留为算法对称实现，
// 由测试（golden 向量 / roundtrip / 坏 IV 拒绝）锚定回归。
#[allow(dead_code)]
type Aes128CfbDec = Decryptor<aes::Aes128>;

/// 加密 host（域名或 IP），返回 `hex(IV) + hex(密文)`。
///
/// CFB 为流式模式：密文长度恒等于明文字节长度，无 padding。
pub fn encrypt_host(host: &str) -> String {
    let mut buf = host.as_bytes().to_vec();
    Aes128CfbEnc::new((&KEY).into(), (&KEY).into()).encrypt(&mut buf);
    // 前缀 hex(IV) 由 key 派生计算，不硬编码字符串
    hex::encode(KEY) + &hex::encode(buf)
}

/// 解密网关 URL 中抠出的 `hex(IV)+hex(密文)` 整段，还原 host。
///
/// IV 段须等于固定网关 IV（本网关恒为 key），不符时报 [`WebVpnError::InvalidIv`]。
#[allow(dead_code)] // 对称 API，仅测试锚定（见 Aes128CfbDec 处注释）
pub fn decrypt_host(hex_text: &str) -> Result<String, WebVpnError> {
    let buf = hex::decode(hex_text)?;
    if buf.len() <= KEY.len() || buf[..KEY.len()] != KEY {
        return Err(WebVpnError::InvalidIv);
    }
    let mut ct = buf[KEY.len()..].to_vec();
    Aes128CfbDec::new((&KEY).into(), (&KEY).into()).decrypt(&mut ct);
    String::from_utf8(ct).map_err(WebVpnError::InvalidUtf8)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// golden 向量：前两条为本校活样本（docs/cas-recon/REPORT.md:68-69），
    /// 其余为社区通行的深澜 WebVPN 算法向量，最后一条为本项目预测回归基线。
    /// 存完整 `hex(IV)+hex(密文)`，同时校验 IV 派生前缀。
    #[test]
    fn golden_vectors() {
        let cases = [
            (
                "my.cwxu.edu.cn",
                "77726476706e69737468656265737421fdee0f9f30287d1e7b0c9ce29b5b",
            ),
            (
                "wxcas.cwxu.edu.cn",
                "77726476706e69737468656265737421e7ef429d347e6b47661dc7a99c406d3676",
            ),
            (
                "202.204.48.66",
                "77726476706e69737468656265737421a2a713d275603c1e2a50c7face",
            ),
            (
                "space.bilibili.com",
                "77726476706e69737468656265737421e3e7409f227e6a5972018ba5945c6d36db05",
            ),
            (
                "219.216.96.4",
                "77726476706e69737468656265737421a2a618d275613e1e275ec7f8",
            ),
            (
                "10.3.100.110",
                "77726476706e69737468656265737421a1a70fcf696138003059d8fc",
            ),
        ];
        for (host, want) in cases {
            assert_eq!(encrypt_host(host), want, "encrypt {host}");
            assert_eq!(decrypt_host(want).unwrap(), host, "decrypt {host}");
        }
    }

    #[test]
    fn roundtrip() {
        for host in ["jwgl.cwxu.edu.cn", "a.b", "10.0.0.1", "中文.example.test"] {
            assert_eq!(decrypt_host(&encrypt_host(host)).unwrap(), host);
        }
    }

    #[test]
    fn cfb_stream_length_no_padding() {
        // 密文长度必须等于明文字节长度：hex(IV) 32 字符 + 2*len(host)
        assert_eq!(encrypt_host("my.cwxu.edu.cn").len(), 32 + 14 * 2);
        assert_eq!(encrypt_host("219.216.96.4").len(), 32 + 12 * 2);
    }

    #[test]
    fn decrypt_rejects_bad_iv() {
        // 长度不足 / IV 段不符 → InvalidIv
        assert!(matches!(decrypt_host("abcd"), Err(WebVpnError::InvalidIv)));
        assert!(matches!(
            decrypt_host(&format!("{}{}", hex::encode([0u8; 16]), "aabb")),
            Err(WebVpnError::InvalidIv)
        ));
    }

    #[test]
    fn decrypt_rejects_bad_hex() {
        assert!(matches!(
            decrypt_host("zz"),
            Err(WebVpnError::InvalidHex(_))
        ));
    }
}
