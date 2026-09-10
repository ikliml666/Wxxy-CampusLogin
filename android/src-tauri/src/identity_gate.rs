//! 生物识别验证门:进程内 TTL 时间戳,语义与桌面 platform/identity.rs 同构
//! (TTL 600s;时钟回拨视为过期——回拨守卫,消除回拨场景的死锁残留)。

use std::sync::atomic::{AtomicU64, Ordering};

static LAST_VERIFY_EPOCH_SECS: AtomicU64 = AtomicU64::new(0);

pub const IDENTITY_VERIFY_TTL_SECS: u64 = 600;

fn now_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 前端 BiometricPrompt 认证成功后调用,写入验证时间戳。
pub fn note_identity_verified() {
    LAST_VERIFY_EPOCH_SECS.store(now_epoch_secs(), Ordering::Relaxed);
}

/// TTL 内已验证才返回 true;从未验证(0)或时钟回拨一律过期。
pub fn identity_verified_recently() -> bool {
    let last = LAST_VERIFY_EPOCH_SECS.load(Ordering::Relaxed);
    if last == 0 {
        return false;
    }
    let now = now_epoch_secs();
    if now < last {
        return false; // 时钟回拨:视为过期,安全方向
    }
    now - last <= IDENTITY_VERIFY_TTL_SECS
}

#[cfg(test)]
pub(crate) fn reset_for_tests() {
    LAST_VERIFY_EPOCH_SECS.store(0, Ordering::Relaxed);
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn 从未验证_为假() {
        LAST_VERIFY_EPOCH_SECS.store(0, Ordering::Relaxed);
        assert!(!identity_verified_recently());
    }

    #[test]
    fn TTL_内为真_外为假() {
        let now = now_epoch_secs();
        LAST_VERIFY_EPOCH_SECS.store(now - 100, Ordering::Relaxed);
        assert!(identity_verified_recently(), "100s < TTL 600s");
        LAST_VERIFY_EPOCH_SECS.store(now - (IDENTITY_VERIFY_TTL_SECS + 1), Ordering::Relaxed);
        assert!(!identity_verified_recently(), "601s > TTL");
    }

    #[test]
    fn 时钟回拨_视为过期() {
        let now = now_epoch_secs();
        LAST_VERIFY_EPOCH_SECS.store(now + 3600, Ordering::Relaxed);
        assert!(!identity_verified_recently(), "回拨守卫:last > now 应过期");
    }
}
