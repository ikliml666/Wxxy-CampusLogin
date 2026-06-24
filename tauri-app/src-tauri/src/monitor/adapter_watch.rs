use tauri::{AppHandle, Emitter, Manager};
use std::sync::atomic::Ordering;
use std::time::Duration;
use crate::network::{Adapter, DisabledAdapter, get_all_adapters_force};
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

            let result = tauri::async_runtime::spawn_blocking(|| {
                get_all_adapters_force()
            }).await;

            if let Ok(Ok((adapters, details, disabled))) = result {
                let adapters_changed = {
                    let mut sorted_current: Vec<&Adapter> = adapters.iter().collect();
                    let mut sorted_last: Vec<&Adapter> = last_adapters.iter().collect();
                    sorted_current.sort_by(|a, b| a.name.cmp(&b.name));
                    sorted_last.sort_by(|a, b| a.name.cmp(&b.name));
                    sorted_current.len() != sorted_last.len()
                        || sorted_current.iter().zip(sorted_last.iter()).any(|(a, b)| a.name != b.name || a.ip != b.ip)
                };

                let disabled_changed = disabled.len() != last_disabled.len()
                    || disabled.iter().zip(last_disabled.iter()).any(|(a, b)| a.name != b.name || a.status != b.status);

                if adapters_changed {
                    if let Err(e) = app_h.emit("adapters-changed", &adapters) {
                        crate::log_warn!("adapter_watch", "发送适配器变更事件失败: {}", e);
                    }
                    if !details.is_empty() {
                        if let Err(e) = app_h.emit("adapter-details-changed", &details) {
                            crate::log_warn!("adapter_watch", "发送适配器详情变更事件失败: {}", e);
                        }
                    }
                }

                if disabled_changed {
                    if let Err(e) = app_h.emit("disabled-adapters-changed", &disabled) {
                        crate::log_warn!("adapter_watch", "发送禁用适配器变更事件失败: {}", e);
                    }
                    let adapter_recovered = last_disabled.iter().any(|ld| {
                        !disabled.iter().any(|d| d.name == ld.name)
                    });
                    if adapter_recovered {
                        let s = app_h.state::<AppState>();
                        if !s.network.load().any_adapter_online {
                            crate::log_info!("adapter_watch", "适配器从禁用恢复，触发重新检测");
                            let _ = crate::monitor::watcher::start_background_check_inner(&app_h, &s);
                        }
                    }
                    let should_notify = {
                        let s = app_h.state::<AppState>();
                        let last_ms = s.last_disabled_notification_ms.load(Ordering::Relaxed);
                        if last_ms == 0 {
                            true
                        } else {
                            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                            (now.as_millis() as u64).saturating_sub(last_ms) >= 60000
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
                            if !last_disabled.iter().any(|ld| ld.name == da.name) && configured_names.iter().any(|n| *n == da.name) {
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

                last_adapters = adapters;
                last_disabled = disabled;
            }
        }
    });
}
