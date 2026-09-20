//! 网络质量命令:直接复用跨平台 network::quality(TCP/DNS/DoH/HTTPS 分段计时;
//! surge_ping ICMP 兜底在安卓运行时静默失败,TCP 优先策略下不影响结果)。

use lazy_static::lazy_static;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Manager;

use campus_login_lib::network::quality::{check_network_quality_async, NetworkQualityResult};

lazy_static! {
    static ref LATENCY_RUNNING: AtomicBool = AtomicBool::new(false);
}

async fn run_quality_once(app: &tauri::AppHandle) -> NetworkQualityResult {
    let settings = crate::config_state::current_settings(app)
        .await
        .unwrap_or_default();
    let adapter_ip = app
        .state::<crate::android_state::AndroidState>()
        .cached_source_ip
        .lock()
        .ok()
        .and_then(|cached| cached.map(|ip| ip.to_string()))
        .unwrap_or_default();
    let is_quitting = Arc::new(AtomicBool::new(false));
    let result = check_network_quality_async(
        "wlan0",
        &adapter_ip,
        settings.skip_ttfb_in_latency,
        settings.skip_content_in_latency,
        &settings.fixed_gateway,
        is_quitting,
        Some(app),
        false,
    )
    .await;
    // 质量历史落盘：命令与定时循环都汇聚于此函数，每次真实检测记一条，供趋势回溯
    if let Ok(dir) = app.path().app_data_dir() {
        if let Err(e) = crate::quality_history::append(&dir, &result) {
            campus_login_lib::log_warn!("quality", "写入网络质量历史失败: {e}");
        }
    }
    result
}

#[tauri::command]
pub async fn check_network_quality(app: tauri::AppHandle) -> Result<NetworkQualityResult, String> {
    Ok(run_quality_once(&app).await)
}

#[tauri::command]
pub async fn start_latency_test(app: tauri::AppHandle) -> Result<(), String> {
    // 幂等:已在跑直接返回
    if LATENCY_RUNNING.swap(true, Ordering::Relaxed) {
        return Ok(());
    }
    let settings = crate::config_state::current_settings(&app).await?;
    tauri::async_runtime::spawn(latency_loop(app, settings));
    Ok(())
}

#[tauri::command]
pub fn stop_latency_test() -> Result<(), String> {
    LATENCY_RUNNING.store(false, Ordering::Relaxed);
    Ok(())
}

/// 稳态退避：连续 5 拍「good 及以上」间隔翻倍，上限 1800s（与桌面轻量化
/// 质量间隔下限对齐）；任何波动/失败立即恢复基础间隔。背景：每 60s 打
/// 12+ 外网目标是后台功耗/流量主力（decisions/android-keepalive-fgs-architecture
/// 的"未实施备查"项，2026-09-20 落地）
const GOOD_LEVELS: [&str; 3] = ["excellent", "great", "good"];
const BACKOFF_STABLE_STREAK: u32 = 5;
const BACKOFF_MAX_MS: u64 = 1_800_000;

/// 退避间隔计算（纯函数）：稳定计数每达 5 拍翻倍一次（封顶 BACKOFF_MAX_MS、
/// 不低于基础间隔）；未达 5 拍维持当前间隔
fn next_backoff_interval_ms(current_ms: u64, stable_good_streak: u32, base_ms: u64) -> u64 {
    if stable_good_streak > 0 && stable_good_streak % BACKOFF_STABLE_STREAK == 0 {
        current_ms.saturating_mul(2).min(BACKOFF_MAX_MS).max(base_ms)
    } else {
        current_ms
    }
}

async fn latency_loop(app: tauri::AppHandle, settings: crate::config_state::Settings) {
    let base_ms = settings.latency_test_interval.max(10_000);
    let mut current_ms = base_ms;
    let mut stable_good_streak: u32 = 0;
    while LATENCY_RUNNING.load(Ordering::Relaxed) {
        let mut tick = tokio::time::interval(std::time::Duration::from_millis(current_ms));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        tick.tick().await; // 新建计时器的首 tick 立即到期，吞掉避免连发
        if !LATENCY_RUNNING.load(Ordering::Relaxed) {
            break;
        }
        let result = run_quality_once(&app).await; // quality 内部经 EventBus emit network-quality-result
        // 退避状态机：good 及以上累计，波动/失败（fair/poor/bad/unknown/busy）清零
        if GOOD_LEVELS.contains(&result.quality.as_str()) {
            stable_good_streak = stable_good_streak.saturating_add(1);
            current_ms = next_backoff_interval_ms(current_ms, stable_good_streak, base_ms);
        } else if stable_good_streak >= BACKOFF_STABLE_STREAK {
            campus_login_lib::log_info!("quality", "网络质量波动，检测间隔 {}ms 恢复 {}ms", current_ms, base_ms);
            stable_good_streak = 0;
            current_ms = base_ms;
        } else {
            stable_good_streak = 0;
            current_ms = base_ms;
        }
    }
    LATENCY_RUNNING.store(false, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::{next_backoff_interval_ms, GOOD_LEVELS, BACKOFF_MAX_MS, BACKOFF_STABLE_STREAK};

    #[test]
    fn 稳定计数达5_间隔翻倍_上限封顶() {
        assert_eq!(next_backoff_interval_ms(60_000, 5, 60_000), 120_000);
        assert_eq!(next_backoff_interval_ms(120_000, 10, 60_000), 240_000);
        // 翻倍后超上限则封顶
        assert_eq!(next_backoff_interval_ms(1_440_000, 20, 60_000), BACKOFF_MAX_MS);
        // 已到上限不再增长
        assert_eq!(next_backoff_interval_ms(BACKOFF_MAX_MS, 25, 60_000), BACKOFF_MAX_MS);
    }

    #[test]
    fn 未达5拍_间隔不变() {
        assert_eq!(next_backoff_interval_ms(60_000, 4, 60_000), 60_000);
        assert_eq!(next_backoff_interval_ms(240_000, 7, 60_000), 240_000);
    }

    #[test]
    fn 翻倍不低于基础间隔() {
        assert_eq!(next_backoff_interval_ms(60_000, 5, 100_000), 100_000);
        assert_eq!(BACKOFF_STABLE_STREAK, 5);
    }

    #[test]
    fn 等级判定_good及以上算稳定() {
        assert!(GOOD_LEVELS.contains(&"excellent"));
        assert!(GOOD_LEVELS.contains(&"good"));
        assert!(!GOOD_LEVELS.contains(&"fair"));
        assert!(!GOOD_LEVELS.contains(&"unknown"));
        assert!(!GOOD_LEVELS.contains(&"busy"));
    }
}
