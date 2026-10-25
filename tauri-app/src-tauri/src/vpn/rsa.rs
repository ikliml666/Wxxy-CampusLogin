//! CAS 密码/token 加密：textbook RSA（Shapiro RSA.js 系，无 padding）。
//!
//! 协议事实见 `docs/cas-recon/REPORT.md` 第二节：
//! - 公钥 e = 0x010001，n 为 1024 位（258 hex 含前导 00）
//! - chunkSize = 126 字节；明文按 latin1 字节流、块内 little-endian 组成整数
//!   （实现：块尾补 0x00 至 126 字节后整块反转，即得大端字节序）
//! - 密文 c = m^65537 mod n；输出 hex 小写、不补前导零，块间直接拼接
//! - 密码 < 126 字节（现实必然）恒为单块；token（"lyasp"+毫秒，18 字节）同理

use super::error::CasAuthError;
use num_bigint_dig::BigUint;
use num_traits::Num;
use std::fmt::Write as _;

/// CAS 线上 RSA modulus（258 hex，含前导 00），源自 `docs/cas-recon/cas.js` 的 MOD。
pub const CAS_RSA_N_HEX: &str = "00b5eeb166e069920e80bebd1fea4829d3d1f3216f2aabe79b6c47a3c18dcee5fd22c2e7ac519cab59198ece036dcf289ea8201e2a0b9ded307f8fb704136eaeb670286f5ad44e691005ba9ea5af04ada5367cd724b5a26fdb5120cc95b6431604bd219c6b7d83a6f8f24b43918ea988a76f93c333aa5a20991493d4eb1117e7b1";

/// RSA 公钥指数 e = 65537。
const RSA_E: u32 = 0x010001;

/// 组块大小：chunkSize = 2 × biHighIndex(n) = 126 字节。
const CHUNK_SIZE: usize = 126;

/// CAS 线上 RSA 加密（`rsa30.js` 的 `rsa.a(131); rsa.b(e,'',n); rsa.c(key, plain)` 等价实现）。
///
/// 输入含非 ASCII 时返回 Err：JS `charCodeAt` 语义对非 ASCII 不等价（UTF-16 码元 vs latin1
/// 字节流），为避免静默产出与线上不一致的密文，显式拒绝（计划 P2-11 修正项）。
pub fn rsa_encrypt_hex(plaintext: &str) -> Result<String, CasAuthError> {
    if !plaintext.is_ascii() {
        return Err(CasAuthError::Rsa(
            "明文含非 ASCII 字符，CAS RSA 加密拒绝（JS charCodeAt 语义不等价）".to_string(),
        ));
    }
    let n = parse_modulus();
    let e = BigUint::from(RSA_E);
    let bytes = plaintext.as_bytes();
    let mut out = String::with_capacity(bytes.len().div_ceil(CHUNK_SIZE) * 256);
    for chunk in bytes.chunks(CHUNK_SIZE) {
        // 块尾补 0x00 至 126 字节 → 反转成大端 → 组成整数（等价于 JS little-endian 组块）
        let mut block = [0u8; CHUNK_SIZE];
        block[..chunk.len()].copy_from_slice(chunk);
        block.reverse();
        let m = BigUint::from_bytes_be(&block);
        let c = m.modpow(&e, &n);
        // hex 小写、不补前导零，块间直接拼接（计划冻结口径；现实输入恒单块）
        let _ = write!(out, "{c:x}");
    }
    Ok(out)
}

/// CAS 请求头 `token` 的值：RSA("lyasp" + 毫秒时间戳)（TAG=lyasp）。
pub fn cas_token_header(now_ms: u64) -> Result<String, CasAuthError> {
    rsa_encrypt_hex(&format!("lyasp{now_ms}"))
}

/// modulus 解析（258 hex 含前导 00；`from_str_radix` 数值上忽略前导零）。
fn parse_modulus() -> BigUint {
    BigUint::from_str_radix(CAS_RSA_N_HEX, 16)
        .expect("CAS_RSA_N_HEX 为固化常量，解析不可能失败")
}
