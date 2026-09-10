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
    check_network_quality_async(
        "wlan0",
        &adapter_ip,
        settings.skip_ttfb_in_latency,
        settings.skip_content_in_latency,
        &settings.fixed_gateway,
        is_quitting,
        Some(app),
    )
    .await
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

async fn latency_loop(app: tauri::AppHandle, settings: crate::config_state::Settings) {
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(
        settings.latency_test_interval.max(10_000),
    ));
    while LATENCY_RUNNING.load(Ordering::Relaxed) {
        tick.tick().await;
        if !LATENCY_RUNNING.load(Ordering::Relaxed) {
            break;
        }
        run_quality_once(&app).await; // quality 内部经 EventBus emit network-quality-result
    }
    LATENCY_RUNNING.store(false, Ordering::Relaxed);
}
