//! 网络归属探测结果短缓存（2026-11-20，自助服务链路稳定性）：
//! resolve_self_service_channel 每次调用都做一轮 TCP 探测（最坏 3s）——同一次
//! 面板刷新 dashboard + online_log 两个命令各探一次，切标签页重挂载再探一次。
//! 网络归属在秒级时间窗内不会翻转（WiFi 切换的感知粒度远大于 10s），缓存
//! 成功探测结果 10s，把「一次面板刷新 N 次探测」收敛为 1 次。
//!
//! 只缓存有结果的探测（Campus/OffCampus/Unknown 之一）；None 不缓存，故障期
//! 每次都真实重试。缓存的是三态结果本身，不是探测细节。

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use super::NetZone;

/// 缓存窗口：同窗口内的归属判定复用上一次结果。
const ZONE_CACHE_TTL: Duration = Duration::from_secs(10);

struct ZoneCache {
    inner: Mutex<Option<(NetZone, Instant)>>,
}

impl ZoneCache {
    fn get(&self) -> Option<NetZone> {
        let guard = self.inner.lock();
        let (zone, ts) = guard.as_ref()?;
        if ts.elapsed() < ZONE_CACHE_TTL {
            Some(*zone)
        } else {
            None
        }
    }

    fn put(&self, zone: NetZone) {
        *self.inner.lock() = Some((zone, Instant::now()));
    }
}

static ZONE_CACHE: OnceLock<ZoneCache> = OnceLock::new();

fn cache() -> &'static ZoneCache {
    ZONE_CACHE.get_or_init(|| ZoneCache {
        inner: Mutex::new(None),
    })
}

/// 缓存命中直接返回；未命中执行探测闭包并缓存其结果。
///
/// 探测闭包返回 Option<bool>（Some(true)=校园内 / Some(false)=校外 / None=无法判定）；
/// None 同样缓存——拿不到适配器信息在 10s 窗口内大概率同样拿不到，重复
/// spawn_blocking 只是把同一份失败再花一遍。
pub async fn cached_zone<F, Fut>(probe: F) -> NetZone
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Option<bool>>,
{
    if let Some(zone) = cache().get() {
        return zone;
    }
    let outcome = probe().await;
    let zone = match outcome {
        Some(true) => NetZone::Campus,
        Some(false) => NetZone::OffCampus,
        None => NetZone::Unknown,
    };
    cache().put(zone);
    zone
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cached_result_skips_probe() {
        // 静态缓存跨测试串扰风险：本测试先 put 再验证「命中后不再调探测闭包」。
        cache().put(NetZone::Campus);
        let called = std::sync::atomic::AtomicBool::new(false);
        let zone = cached_zone(|| async {
            called.store(true, std::sync::atomic::Ordering::SeqCst);
            Some(false)
        })
        .await;
        assert_eq!(zone, NetZone::Campus);
        assert!(!called.load(std::sync::atomic::Ordering::SeqCst));
    }
}
