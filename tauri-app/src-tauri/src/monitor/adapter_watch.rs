use tauri::{AppHandle, Manager};
use std::sync::atomic::Ordering;
use std::time::Duration;
use crate::network::{Adapter, DisabledAdapter, get_all_adapters_cached};
use crate::infra::state::AppState;
use crate::infra::events::EventBus;

const ADAPTER_WATCH_INTERVAL: u64 = 15000;

pub fn start_adapter_watch(app_handle: &AppHandle) -> Result<(), String> {
    let app_h = app_handle.clone();
    app_handle.state::<AppState>().task_manager.spawn("adapter_watch", move |cancel_token| {
        async move {
            let mut last_adapters: Vec<Adapter> = Vec::new();
            let mut last_disabled: Vec<DisabledAdapter> = Vec::new();
            let mut interval_timer = tokio::time::interval(Duration::from_millis(ADAPTER_WATCH_INTERVAL));
            // 单轮检测耗时超过周期时默认 Burst 会连续补发错过的 tick 造成连发，
            // 改为 Delay 保持固定周期、错过的不补发
            interval_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
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

                // 历史缺陷：CLASS_SUBKEY_CACHE 仅首次访问构建、只在 enable_adapter 刷新，
                // 运行期设备管理器禁用/拔插适配器的可见性与禁用分类永久陈旧。
                // 随 15s 监听周期轻量刷新（注册表遍历在后台线程执行）。
                // BE-B-01: refresh_class_subkey_cache 内部是 winreg 同步遍历 HKLM Class 子键，
                // 包进 spawn_blocking 避免在 async 任务线程上执行同步注册表 I/O
                // （与下方适配器查询同一模式）。
                let _ = tauri::async_runtime::spawn_blocking(|| {
                    crate::network::discovery::registry::refresh_class_subkey_cache();
                }).await;

                // BE-B-05: 原为 get_all_adapters_force 每 15s 强制清缓存重查，与 4s 后台
                // 常驻刷新叠加造成重复全量 GetAdaptersAddresses。改读缓存（数据最多陈旧
                // 4s，对本周期只需检测"名称/IP 是否变化"的变更监测粒度足够；真正需要
                // 即时数据的路径仍可用 force 变体）。
                let result = tauri::async_runtime::spawn_blocking(get_all_adapters_cached).await;

                if let Ok(Ok((adapters, details, disabled))) = result {
                let adapters_changed = {
                    let mut sorted_current: Vec<&Adapter> = adapters.iter().collect();
                    let mut sorted_last: Vec<&Adapter> = last_adapters.iter().collect();
                    sorted_current.sort_by(|a, b| a.name.cmp(&b.name));
                    sorted_last.sort_by(|a, b| a.name.cmp(&b.name));
                    sorted_current.len() != sorted_last.len()
                        || sorted_current.iter().zip(sorted_last.iter()).any(|(a, b)| a.name != b.name || a.ip != b.ip)
                };

                // 历史缺陷：GetAdaptersAddresses 返回顺序不稳定，disabled 直接 zip 比较会
                // 因顺序变化误报 changed；与上方 adapters 一致按 name 排序后再比较。
                let disabled_changed = {
                    let mut sorted_cur: Vec<&DisabledAdapter> = disabled.iter().collect();
                    let mut sorted_last: Vec<&DisabledAdapter> = last_disabled.iter().collect();
                    sorted_cur.sort_by(|a, b| a.name.cmp(&b.name));
                    sorted_last.sort_by(|a, b| a.name.cmp(&b.name));
                    sorted_cur.len() != sorted_last.len()
                        || sorted_cur.iter().zip(sorted_last.iter()).any(|(a, b)| a.name != b.name || a.status != b.status)
                };

                if adapters_changed {
                    if let Err(e) = EventBus::new(&app_h).emit_adapters_changed(&adapters) {
                        crate::log_warn!("adapter_watch", "发送适配器变更事件失败: {}", e);
                    }
                    if !details.is_empty() {
                        if let Err(e) = EventBus::new(&app_h).emit_adapter_details_changed(&details) {
                            crate::log_warn!("adapter_watch", "发送适配器详情变更事件失败: {}", e);
                        }
                    }
                }

                if disabled_changed {
                    if let Err(e) = EventBus::new(&app_h).emit_disabled_adapters_changed(&disabled) {
                        crate::log_warn!("adapter_watch", "发送禁用适配器变更事件失败: {}", e);
                    }
                    let adapter_recovered = last_disabled.iter().any(|ld| {
                        !disabled.iter().any(|d| d.name == ld.name)
                    });
                    if adapter_recovered {
                        let s = app_h.state::<AppState>();
                        if !s.network.load().any_adapter_online {
                            crate::log_info!("adapter_watch", "适配器从禁用恢复，触发重新检测");
                            // 历史缺陷：此处原调用 trigger_background_check（实为 start_background_check_inner
                            // 的别名），会无条件重开用户已停止的后台巡检并持久化 enable_background_check=true，
                            // 用户明确关闭的巡检被适配器恢复事件悄悄重新开启。
                            // 改为只触发一次性检查（内部 is_checking 信号量防止并发）。
                            let cancel = s.task_manager
                                .cancel_token("background_check")
                                .unwrap_or_else(|| std::sync::Arc::new(tokio_util::sync::CancellationToken::new()));
                            let app_h_check = app_h.clone();
                            tauri::async_runtime::spawn(async move {
                                super::background_check::run_background_check(&app_h_check, cancel).await;
                            });
                        }
                    }
                    let should_notify = {
                        let s = app_h.state::<AppState>();
                        let last_ms = s.update_stats.last_disabled_notification_ms.load(Ordering::Relaxed);
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
                        s.update_stats.last_disabled_notification_ms.store(now.as_millis() as u64, Ordering::Relaxed);
                        let c = s.config.load();
                        let adapter1 = &c.adapter1;
                        let adapter2 = &c.adapter2;
                        let configured_names: Vec<&str> = if crate::network::is_secondary_adapter_enabled(&c, adapter2) && adapter2.as_str() != crate::config::model::AUTO_DETECT_ADAPTER {
                            vec![adapter1, adapter2]
                        } else if !adapter1.is_empty() && adapter1.as_str() != crate::config::model::AUTO_DETECT_ADAPTER {
                            vec![adapter1]
                        } else {
                            vec![]
                        };
                        for da in &disabled {
                            if !last_disabled.iter().any(|ld| ld.name == da.name) && configured_names.iter().any(|n| *n == da.name) {
                                let message = format!("适配器{} 当前{}，请检查后重试", da.name, da.status);
                                if let Err(e) = EventBus::new(&app_h).emit_adapter_disabled_warning(&da.name, &message) {
                                    crate::log_warn!("adapter_watch", "发送适配器禁用警告失败: {}", e);
                                }
                            }
                        }
                    }
                }

                // 自动启用：用户手选的具体适配器被禁用时，静默尝试启用以恢复登录。
                // "自动检测"模式不参与（configured_disabled_adapters 已过滤空串与哨兵）。
                // 提权路径不弹 UAC（enable_adapter(…, false)），失败按退避阶梯重试。
                {
                    let s = app_h.state::<AppState>();
                    let c = s.config.load();
                    let targets = crate::network::adapter::configured_disabled_adapters(&c, &disabled);
                    let stats = &s.update_stats;
                    if targets.is_empty() {
                        // 手选适配器全部恢复（或用户改回自动检测）：清零失败计数，退避从头计
                        if stats.auto_enable_failure_count.load(Ordering::Relaxed) != 0 {
                            stats.auto_enable_failure_count.store(0, Ordering::Relaxed);
                            crate::log_debug!("adapter_watch", "无手选禁用适配器，自动启用失败计数清零");
                        }
                    } else {
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        let backoff = auto_enable_backoff_ms(stats.auto_enable_failure_count.load(Ordering::Relaxed));
                        if now_ms.saturating_sub(stats.auto_enable_last_attempt_ms.load(Ordering::Relaxed)) >= backoff {
                            // 先记录尝试时间再执行，失败退避以本时刻为基准
                            stats.auto_enable_last_attempt_ms.store(now_ms, Ordering::Relaxed);
                            for da in targets {
                                let name = da.name.clone();
                                crate::log_info!("adapter_watch", "检测到手选适配器 {} 被禁用，尝试自动启用", name);
                                // netsh + 提权为阻塞调用，包进 spawn_blocking 不进 async 上下文
                                let app_h_enable = app_h.clone();
                                tauri::async_runtime::spawn_blocking(move || {
                                    match crate::network::adapter_cache::enable_adapter(&name, false) {
                                        Ok(()) => {
                                            crate::log_info!("adapter_watch", "自动启用适配器成功: {}", name);
                                            let s = app_h_enable.state::<AppState>();
                                            s.update_stats.auto_enable_failure_count.store(0, Ordering::Relaxed);
                                            // 稍等适配器就绪后触发一次性检查以尽快恢复登录
                                            //（沿用禁用恢复路径模式：不重开用户已停止的后台巡检）
                                            let cancel = s.task_manager
                                                .cancel_token("background_check")
                                                .unwrap_or_else(|| std::sync::Arc::new(tokio_util::sync::CancellationToken::new()));
                                            let app_h_check = app_h_enable.clone();
                                            tauri::async_runtime::spawn(async move {
                                                tokio::time::sleep(Duration::from_secs(5)).await;
                                                super::background_check::run_background_check(&app_h_check, cancel).await;
                                            });
                                        }
                                        Err(e) => {
                                            let s = app_h_enable.state::<AppState>();
                                            let failures = s.update_stats.auto_enable_failure_count.fetch_add(1, Ordering::Relaxed) + 1;
                                            let next = auto_enable_backoff_ms(failures);
                                            crate::log_warn!(
                                                "adapter_watch",
                                                "自动启用适配器失败: {}，按退避 {}s 后重试: {}",
                                                name, next / 1000, e
                                            );
                                        }
                                    }
                                });
                            }
                        }
                    }
                }

                last_adapters = adapters;
                last_disabled = disabled;
                } else {
                    // 查询失败整段跳过会静默丢一轮变更检测，记警告便于排查
                    match result {
                        Ok(Err(e)) => crate::log_warn!("adapter_watch", "适配器查询失败: {}", e),
                        Err(e) => crate::log_warn!("adapter_watch", "适配器查询任务失败: {}", e),
                        _ => {}
                    }
                }
            }
        }
    })
}

/// 自动启用失败退避（ms）：0 次→立即可试；1→60s；2→120s；≥3→300s 封顶。
/// 纯函数，便于单测。
fn auto_enable_backoff_ms(failure_count: u32) -> u64 {
    match failure_count {
        0 => 0,
        1 => 60_000,
        2 => 120_000,
        _ => 300_000,
    }
}

#[cfg(test)]
mod tests {
    use super::auto_enable_backoff_ms;

    #[test]
    fn auto_enable_backoff_ladder() {
        assert_eq!(auto_enable_backoff_ms(0), 0);
        assert_eq!(auto_enable_backoff_ms(1), 60_000);
        assert_eq!(auto_enable_backoff_ms(2), 120_000);
        assert_eq!(auto_enable_backoff_ms(3), 300_000);
        // 封顶：更多失败不再增长
        assert_eq!(auto_enable_backoff_ms(u32::MAX), 300_000);
    }
}
