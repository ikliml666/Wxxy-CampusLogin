use tauri::{AppHandle, Emitter, Manager};
use std::sync::atomic::Ordering;
use std::time::Duration;
use crate::network::{Adapter, DisabledAdapter, get_adapters_force, get_disabled_adapters_force};
use crate::infra::state::AppState;

const ADAPTER_WATCH_INTERVAL: u64 = 15000;

pub fn start_adapter_watch(app_handle: &AppHandle, cancel_token: std::sync::Arc<tokio_util::sync::CancellationToken>) {
    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let s = app_h.state::<AppState>();
        let _guard = match s.tasks.adapter_watch_running.acquire_guard() {
            Some(g) => g,
            None => return,
        };
        let mut last_adapters: Vec<Adapter> = Vec::new();
        let mut last_disabled: Vec<DisabledAdapter> = Vec::new();
        let mut interval_timer = tokio::time::interval(Duration::from_millis(ADAPTER_WATCH_INTERVAL));
        interval_timer.tick().await;

        loop {
            tokio::select! {
                _ = interval_timer.tick() => {}
                _ = cancel_token.cancelled() => {
                    break;
                }
            }

            let s = app_h.state::<AppState>();
            if s.exit.is_quitting.load(Ordering::Acquire) {
                break;
            }

            crate::network::dns::cleanup_expired_dns_cache();

            let adapters_result = tauri::async_runtime::spawn_blocking(|| {
                get_adapters_force()
            }).await;

            if let Ok(Ok(adapters)) = adapters_result {
                let changed = adapters.len() != last_adapters.len()
                    || adapters.iter().zip(last_adapters.iter()).any(|(a, b)| a.name != b.name || a.ip != b.ip);

                if changed {
                    if let Err(e) = app_h.emit("adapters-changed", &adapters) {
                        crate::log_warn!("adapter_watch", "发送适配器变更事件失败: {}", e);
                    }

                    let disabled_result = tauri::async_runtime::spawn_blocking(|| {
                        get_disabled_adapters_force()
                    }).await;

                    if let Ok(Ok(disabled)) = disabled_result {
                        let disabled_changed = disabled.len() != last_disabled.len()
                            || disabled.iter().zip(last_disabled.iter()).any(|(a, b)| a.name != b.name);

                        if disabled_changed {
                            if let Err(e) = app_h.emit("disabled-adapters-changed", &disabled) {
                                crate::log_warn!("adapter_watch", "发送禁用适配器变更事件失败: {}", e);
                            }
                            let should_notify = {
                                let s = app_h.state::<AppState>();
                                let last_ms = s.last_disabled_notification_ms.load(Ordering::Relaxed);
                                if last_ms == 0 {
                                    true
                                } else {
                                    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                                    now.as_millis() as u64 - last_ms >= 60000
                                }
                            };
                            if should_notify {
                                let s = app_h.state::<AppState>();
                                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                                s.last_disabled_notification_ms.store(now.as_millis() as u64, Ordering::Relaxed);
                                let config = {
                                    let c = s.config.load();
                                    (c.adapter1.clone(), c.adapter2.clone(), c.dual_adapter)
                                };
                                let (adapter1, adapter2, dual_adapter) = config;
                                let configured_names: Vec<&str> = if dual_adapter && !adapter2.is_empty() && adapter2.as_str() != "自动检测" {
                                    vec![&adapter1, &adapter2]
                                } else if !adapter1.is_empty() && adapter1.as_str() != "自动检测" {
                                    vec![&adapter1]
                                } else {
                                    vec![]
                                };
                                for da in &disabled {
                                    if !last_disabled.iter().any(|ld| ld.name == da.name) {
                                        if configured_names.iter().any(|n| *n == da.name) {
                                            let message = format!("适配器{} 当前{}，请检查后重试", da.name, da.status);
                                            if let Err(e) = app_h.emit("adapter-disabled-warning", serde_json::json!({
                                                "name": da.name,
                                                "message": message,
                                            })) {
                                                crate::log_warn!("adapter_watch", "发送适配器禁用警告失败: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        last_disabled = disabled;
                    }

                    last_adapters = adapters;
                }
            }
        }
    });
}
