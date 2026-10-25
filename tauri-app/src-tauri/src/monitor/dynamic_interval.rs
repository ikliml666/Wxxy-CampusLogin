//! 动态检测间隔状态机（2026-10-25）：后台巡检间隔不再固定为配置值，而是随
//! 登录状态自适应——未登录失败逐步退避、已登录拉长、已登录但门户不可达快速排查。
//!
//! 三档语义（用户需求）：
//! - 未登录：从配置基础间隔起 ×2 逐步退避，封顶 [`BACKOFF_CAP_MS`]（5 分钟）；
//! - 已登录（门户可达）：间隔固定 [`ONLINE_INTERVAL_MS`]（10 分钟）；
//! - 已登录但无法访问门户（http://10.1.99.100/）：间隔降为 [`FAST_INTERVAL_MS`]
//!   （1 分钟）快速排查，并由巡检联动一次网络质量检测（是否连上互联网）。
//!
//! 另：MAC 重置检测（门户不可达触发）只在每日 [`PORTAL_RESET_START_MINUTES`]
//! （08:30）之后启用——可登录时段之前门户拒绝属预期，不应累计触发换 MAC。

/// 未登录退避封顶：5 分钟
pub const BACKOFF_CAP_MS: u64 = 300_000;
/// 已登录检测间隔：10 分钟
pub const ONLINE_INTERVAL_MS: u64 = 600_000;
/// 已登录但门户不可达的快速排查间隔：1 分钟
pub const FAST_INTERVAL_MS: u64 = 60_000;
/// MAC 重置检测启用时间：每日 08:30 之后（8*60+30=510 分钟）
pub const PORTAL_RESET_START_MINUTES: u16 = 8 * 60 + 30;

/// 当前时间（当日分钟数）是否已进入 MAC 重置检测启用窗口
pub fn portal_reset_window_open(now_min: u16) -> bool {
    now_min >= PORTAL_RESET_START_MINUTES
}

/// 一轮巡检的结局分类，决定下一拍间隔
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckOutcome {
    /// 未在线（含校园网检测未通过、门户可达但未登录等全部失败路径）
    NotOnline,
    /// 在线且门户可达：健康状态
    Online,
    /// 此前在线但本轮门户不可达：疑似"已登录但门户异常"，进入快速排查
    OnlineButPortalUnreachable,
}

/// 检测间隔退避状态：跟踪连续未在线轮数
#[derive(Debug, Default)]
pub struct DynamicInterval {
    offline_streak: u32,
}

impl DynamicInterval {
    /// 根据本轮结局与配置基础间隔，给出下一拍间隔（毫秒）。
    /// 状态转换（登录成功 / 门户恢复）自动复位退避计数。
    pub fn next_ms(&mut self, outcome: CheckOutcome, base_ms: u64) -> u64 {
        match outcome {
            CheckOutcome::OnlineButPortalUnreachable => {
                self.offline_streak = 0;
                FAST_INTERVAL_MS
            }
            CheckOutcome::Online => {
                self.offline_streak = 0;
                ONLINE_INTERVAL_MS
            }
            CheckOutcome::NotOnline => {
                self.offline_streak = self.offline_streak.saturating_add(1);
                backoff_ms(base_ms, self.offline_streak)
            }
        }
    }

    /// 当前连续未在线轮数（测试用）
    #[cfg(test)]
    pub fn offline_streak(&self) -> u32 {
        self.offline_streak
    }
}

/// 未登录退避：基础间隔 ×2^(streak-1)，封顶 [`BACKOFF_CAP_MS`]；
/// 用户配置的基础间隔大于封顶时尊重用户值（不缩短）。
pub fn backoff_ms(base_ms: u64, streak: u32) -> u64 {
    let base = base_ms.max(10_000);
    let mut ms = base;
    for _ in 1..streak {
        if ms >= BACKOFF_CAP_MS {
            break;
        }
        ms = ms.saturating_mul(2);
    }
    ms.min(BACKOFF_CAP_MS).max(base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 未登录按2的幂退避封顶5分钟() {
        assert_eq!(backoff_ms(15_000, 1), 15_000);
        assert_eq!(backoff_ms(15_000, 2), 30_000);
        assert_eq!(backoff_ms(15_000, 3), 60_000);
        assert_eq!(backoff_ms(15_000, 5), 240_000);
        assert_eq!(backoff_ms(15_000, 6), 300_000);
        assert_eq!(backoff_ms(15_000, 100), 300_000);
    }

    #[test]
    fn 用户基础间隔大于封顶时不缩短() {
        assert_eq!(backoff_ms(600_000, 10), 600_000);
    }

    #[test]
    fn 已登录间隔10分钟且复位退避() {
        let mut d = DynamicInterval::default();
        for _ in 0..8 {
            d.next_ms(CheckOutcome::NotOnline, 15_000);
        }
        assert_eq!(d.next_ms(CheckOutcome::Online, 15_000), ONLINE_INTERVAL_MS);
        // 登录后退避复位：再次掉线从基础间隔重新爬坡
        assert_eq!(d.next_ms(CheckOutcome::NotOnline, 15_000), 15_000);
        assert_eq!(d.next_ms(CheckOutcome::NotOnline, 15_000), 30_000);
    }

    #[test]
    fn 已登录但门户不可达降为1分钟() {
        let mut d = DynamicInterval::default();
        d.next_ms(CheckOutcome::Online, 15_000);
        assert_eq!(
            d.next_ms(CheckOutcome::OnlineButPortalUnreachable, 15_000),
            FAST_INTERVAL_MS
        );
        // 门户恢复（在线且可达）回到 10 分钟
        assert_eq!(d.next_ms(CheckOutcome::Online, 15_000), ONLINE_INTERVAL_MS);
    }

    #[test]
    fn mac重置检测窗口8点30分整点开启() {
        assert!(!portal_reset_window_open(509)); // 08:29
        assert!(portal_reset_window_open(510)); // 08:30
        assert!(portal_reset_window_open(23 * 60 + 59));
        assert!(!portal_reset_window_open(0)); // 凌晨
    }
}
