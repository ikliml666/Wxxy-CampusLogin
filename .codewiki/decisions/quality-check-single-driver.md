你是 CodeWiki 文章作者。文章全文用中文书写。

任务：更新过期文章 decisions/quality-check-single-driver.md：以当前代码为准重写其内容，保留章节结构与 title/type/tags

以下是你需要的全部源码（路径相对仓库根）：

### tauri-app/src-tauri/src/monitor/background_check.rs

```
use tauri::{AppHandle, Manager};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use chrono::Timelike;
use crate::network::{get_adapters_cached, get_adapters_force};
use crate::infra::state::AppState;
use crate::infra::lifecycle::{start_campus_exit, cancel_campus_exit};
use crate::auth::failure_tracker::AdapterFailureCounter;
use super::auto_auth::{try_auto_login_on_preparation, try_disconnect_reconnect};
use super::campus_check::{CampusCheckResult, adapter_campus_status, adapter_campus_message, check_campus_network};
use super::portal_check::{PortalCheckResult, check_adapter_portal};
use crate::auth::failure_tracker::handle_portal_request_failure;
use super::background_emit::{handle_status_change, emit_background_check_result, update_network_state, BackgroundCheckResult};

pub(crate) fn run_background_check_blocking(app_handle: &AppHandle, state: &AppState, cancel_token: &tokio_util::sync::CancellationToken) {
    if state.exit.is_quitting.load(Ordering::Acquire) || cancel_token.is_cancelled() {
        return;
    }
    let Some(_check_guard) = state.tasks.is_checking.try_acquire() else {
        return;
    };
    let t_total = std::time::Instant::now();

    let config = state.config.load_full();
    crate::log_debug!("background", "开始后台检测 (dualAdapter={}, interval={}ms)",
        config.dual_adapter, config.background_check_interval);

    let t_adapters = std::time::Instant::now();
    let adapters = match get_adapters_cached() {
        Ok(a) if !a.is_empty() => a,
        _ => match get_adapters_force() {
            Ok(a) => a,
            Err(e) => {
                crate::log_error!("background", "获取适配器列表失败: {}", e);
                return;
            }
        }
    };

    crate::log_debug!("background", "适配器列表: {}个 (耗时{}ms)", adapters.len(), t_adapters.elapsed().as_millis());

    let (adapter1_name, adapter2_name) = crate::network::resolve_adapter_names(&adapters, &config);

    let (a1, a2) = crate::network::find_dual_adapters(&adapters, &config, &adapter1_name, &adapter2_name);

    let now = chrono::Local::now();
    let now_min = now.hour() as u16 * 60 + now.minute() as u16;
    // 出站切换态（三份快照任一非空）：巡检整体跳过（portal 检测走热点出站必失败，
    // 会累计触发 MAC 重置提权与整夜告警；见 decisions/night-outbound-switch）。下一拍（30s）恢复。
    if crate::config::outbound_switch::outbound_restore_active(
        &config.outbound_metric_restore,
        &config.outbound_disabled_adapters,
        &config.outbound_standby_route,
    ) {
        crate::log_debug!("background", "出站切换态，跳过本轮巡检");
        return;
    }
    let campus_result = if crate::monitor::campus_check::is_campus_check_silent(now_min, config.campus_check_start_minutes, config.campus_check_end_minutes) {
        let hour = config.campus_check_start_minutes / 60;
        let minute = config.campus_check_start_minutes % 60;
        let end_hour = config.campus_check_end_minutes / 60;
        let end_minute = config.campus_check_end_minutes % 60;
        let (window, log_window) = if config.campus_check_end_minutes > config.campus_check_start_minutes {
            (
                format!("不在{hour}:{minute:02}–{end_hour}:{end_minute:02}检测时段内"),
                format!("不在检测时段 {:02}:{:02}–{:02}:{:02}", hour, minute, end_hour, end_minute),
            )
        } else {
            (
                format!("早于{hour}:{minute:02}"),
                format!("早于 {:02}:{:02}", hour, minute),
            )
        };
        crate::log_info!("background", "校园网检测静默期（当前时间{}），跳过校园网环境验证", log_window);
        cancel_campus_exit(app_handle, state);
        CampusCheckResult {
            wifi: None,
            wired: None,
            on_campus: true,
            current_ssid: None,
            message: format!("校园网检测静默期（{window}），跳过验证"),
        }
    } else {
        // 校园网检测只看解析出的主/副适配器：其他适配器的连接不属本应用管理范围
        let op_adapters = crate::network::filter_operation_adapters(&adapters, &adapter1_name, &adapter2_name);
        check_campus_network(&config, &op_adapters)
    };
    // 始终更新 on_campus_network（静默期内 campus_result.on_campus=true，确保 emit 字段一致）
    // 合并条件分支内的 any_adapter_online/last_a1_online 重置，减少 CAS 循环次数
    let campus_check_failed = config.enable_network_name_check && !campus_result.on_campus;
    state.network.update(|s| {
        s.current_ssid = campus_result.current_ssid.as_deref().map(Arc::from);
        s.on_campus_network = campus_result.on_campus;
        if campus_check_failed {
            s.any_adapter_online = false;
            s.last_a1_online = false;
            // 历史缺陷：has_logged_online 仅在登录成功时置位、永不重置，
            // 用户离开校园网（campus fail）再重新进入后，准备自动登录被
            // has_logged_online 永久拦截，必须手动登录。此处随离线一并重置，
            // 使重新进入校园网后自动登录恢复生效。
            s.has_logged_online = false;
        }
    });

    if campus_check_failed {
        crate::log_debug!("background", "校园网检测未通过: {}", campus_result.message);
        let a1_campus = adapter_campus_message(&adapter1_name, &adapters, &campus_result);
        let a2_campus = if crate::network::is_secondary_adapter_enabled(&config, &adapter2_name) {
            adapter_campus_message(&adapter2_name, &adapters, &campus_result)
        } else { None };
        let a1_on_campus = adapter_campus_status(&adapter1_name, &adapters, &campus_result).map(|s| s.on_campus);
        let a2_on_campus = if crate::network::is_secondary_adapter_enabled(&config, &adapter2_name) {
            adapter_campus_status(&adapter2_name, &adapters, &campus_result).map(|s| s.on_campus)
        } else { None };
        emit_background_check_result(
            app_handle, state,
            &BackgroundCheckResult {
                online: false,
                online_operator: None,
                secondary_online_operator: None,
                reachable: false,
                login_available: false,
                message: a1_campus.as_deref().unwrap_or(&campus_result.message),
                adapter1_name: &adapter1_name,
                adapter2_name: &adapter2_name,
                secondary_online: None,
                secondary_message: a2_campus.as_deref().unwrap_or(""),
                dual_adapter: config.dual_adapter,
                config: &config,
                campus_result: &campus_result,
                a1_campus_msg: a1_campus.as_deref(),
                a2_campus_msg: a2_campus.as_deref(),
                a1_on_campus,
                a2_on_campus,
            },
        );
        // 如果配置的适配器均无IP（完全无网络），跳过退出，等待网络恢复
        let no_configured_ip = a1.is_none() && a2.is_none();
        if no_configured_ip {
            crate::log_info!("background", "配置的适配器均无IP地址，跳过校园网退出，等待网络恢复");
        } else {
            // 校园网验证不通过：触发最小化+退出流程
            start_campus_exit(app_handle, state);
        }
        crate::log_debug!("background", "后台检测周期完成(校园网检测未通过), 总耗时{}ms", t_total.elapsed().as_millis());
        return;
    }

    // 校园网验证通过：取消之前的退出流程（如果有的话）
    cancel_campus_exit(app_handle, state);

    if cancel_token.is_cancelled() {
        return;
    }

    let t_portal = std::time::Instant::now();
    let (primary_result, secondary_result) = if config.dual_adapter {
        if let (Some(adapter1), Some(adapter2)) = (a1, a2) {
            // 改用 tauri::async_runtime::spawn_blocking + tokio::join! 并行检测双适配器，
            // 替代 std::thread::scope 创建 OS 线程的方式（参考 auto_auth.rs 同场景实现）。
            // run_background_check_blocking 运行在 spawn_blocking 线程内，通过 Handle::current().block_on
            // 进入 async 上下文，使 spawn_blocking 提交的任务能被 await。
            let runtime_handle = tokio::runtime::Handle::current();
            let a1_owned = adapter1.clone();
            let a2_owned = adapter2.clone();
            let app_h1 = app_handle.clone();
            let app_h2 = app_handle.clone();
            runtime_handle.block_on(async {
                let h1 = tauri::async_runtime::spawn_blocking(move || check_adapter_portal(&a1_owned, &app_h1));
                let h2 = tauri::async_runtime::spawn_blocking(move || check_adapter_portal(&a2_owned, &app_h2));
                let (r1, r2) = tokio::join!(h1, h2);
                let r1 = r1.unwrap_or(PortalCheckResult::Error { is_request_failed: false });
                let r2 = r2.unwrap_or(PortalCheckResult::Error { is_request_failed: false });
                (r1, Some(r2))
            })
        } else {
            let primary = match a1 {
                Some(adapter) => check_adapter_portal(adapter, app_handle),
                None => PortalCheckResult::NotFound,
            };
            let secondary = a2.map(|a| check_adapter_portal(a, app_handle));
            (primary, secondary)
        }
    } else {
        let primary = match a1 {
            Some(adapter) => check_adapter_portal(adapter, app_handle),
            None => PortalCheckResult::NotFound,
        };
        (primary, None)
    };

    let portal_elapsed = t_portal.elapsed();

    // Portal 请求失败容错：累加失败计数，连续5次 request_failed 时触发 MAC 重置（阈值见 failure_tracker.rs::PORTAL_REQUEST_FAILURE_THRESHOLD）
    let primary_is_request_failed = matches!(&primary_result, PortalCheckResult::Error { is_request_failed: true });
    let secondary_is_request_failed = secondary_result.as_ref().map(|r| matches!(r, PortalCheckResult::Error { is_request_failed: true })).unwrap_or(false);
    let any_request_failed = primary_is_request_failed || secondary_is_request_failed;

    if any_request_failed {
        // 按适配器分别检查网关可达性：每个适配器从自己的 IP 绑定 ping 网关
        let campus_gw = &config.campus_gateway;
        let a1_ip = a1.map(|a| a.ip.as_str());
        let a2_ip = a2.map(|a| a.ip.as_str());

        // 适配器1 失败处理
        if primary_is_request_failed {
            handle_portal_request_failure(
                state, app_handle, a1, a1_ip,
                campus_gw, AdapterFailureCounter::A1, "适配器1",
            );
        }

        // 适配器2 失败处理
        if secondary_is_request_failed {
            handle_portal_request_failure(
                state, app_handle, a2, a2_ip,
                campus_gw, AdapterFailureCounter::A2, "适配器2",
            );
        }
    }

    // 按适配器独立重置：某适配器本次 Success 即重置该适配器的失败计数，
    // 不依赖对方检测结果（若对方 request_failed 也一样重置，对齐
    // failure_tracker.rs "登录成功即重置"语义，避免残留计数导致下次真实失败提前触发 MAC 重置）
    let primary_success = matches!(&primary_result, PortalCheckResult::Success { .. });
    let secondary_success = secondary_result.as_ref().map(|r| matches!(r, PortalCheckResult::Success { .. })).unwrap_or(false);

    // 合并 a1/a2 失败计数重置为单次 update，减少 CAS 循环次数
    if primary_success || secondary_success {
        let snap = state.network.load();
        let prev_a1 = snap.a1_auth_failure_count;
        let prev_a2 = snap.a2_auth_failure_count;
        state.network.update(|s| {
            if primary_success {
                s.a1_auth_failure_count = 0;
            }
            if secondary_success {
                s.a2_auth_failure_count = 0;
            }
        });
        if primary_success && prev_a1 > 0 {
            crate::log_debug!("background", "适配器1 Portal检测恢复正常，重置失败计数(原值={})", prev_a1);
        }
        if secondary_success && prev_a2 > 0 {
            crate::log_debug!("background", "适配器2 Portal检测恢复正常，重置失败计数(原值={})", prev_a2);
        }
    }

    let primary_online = primary_result.online();
    let reachable = primary_result.reachable();
    let login_available = primary_result.login_available();
    let a1_has_ip = a1.is_some();

    let message: String = if a1_has_ip {
        primary_result.message().to_string()
    } else {
        adapter_campus_message(&adapter1_name, &adapters, &campus_result)
            .unwrap_or_else(|| primary_result.message().to_string())
    };
    let online = if a1_has_ip { primary_online } else { false };
    // 在线会话运营商口径：仅主适配器在线时，从 Portal 页 uid（完整在线账号）推导后缀
    let online_operator = if online {
        primary_result.uid().and_then(crate::auth::portal::operator_suffix_from_uid).map(str::to_string)
    } else {
        None
    };

    let prev_online = state.network.load().any_adapter_online;

    crate::log_debug!("background", "Portal检测完成({}ms): 主[{}]={}/{} |副={:?}",
        portal_elapsed.as_millis(),
        adapter1_name,
        if online { "online" } else if reachable { "offline" } else { "unreachable" },
        message,
        secondary_result.as_ref().map(|r| format!("{}/{}", if r.online() {"online"} else {r.message()}, r.reachable())));

    let a2_has_ip = a2.is_some();
    let (secondary_online, secondary_message) = match &secondary_result {
        Some(PortalCheckResult::Success { online, message: msg, .. }) => (Some(*online), msg.clone()),
        _ => {
            if crate::network::is_secondary_adapter_enabled(&config, &adapter2_name) && !a2_has_ip {
                let msg = adapter_campus_message(&adapter2_name, &adapters, &campus_result);
                match msg {
                    Some(ref m) => (Some(false), m.clone()),
                    None => (None, String::new()),
                }
            } else if crate::network::is_secondary_adapter_enabled(&config, &adapter2_name) && a2_has_ip {
                (None, secondary_result.as_ref().map(|r| r.message().to_string()).unwrap_or_default())
            } else {
                (None, String::new())
            }
        }
    };

    // 副适配器在线会话运营商口径：与主适配器同源（Portal uid 推导），仅副适配器在线时有值。
    // 双适配器可用同一账号在不同时机登录，会话运营商可能瞬时不同（如夜切/手动重登交替期）。
    let secondary_online_operator = if secondary_online == Some(true) {
        secondary_result.as_ref().and_then(|r| r.uid()).and_then(crate::auth::portal::operator_suffix_from_uid).map(str::to_string)
    } else {
        None
    };

    state.network.update(|s| s.last_a2_online = secondary_online == Some(true));

    // 历史缺陷：handle_status_change 传入的是仅主适配器的 online，
    // 而 prev_online 是 any_adapter_online（主+副任意在线即 true）。
    // 双适配器"主断副通"时 online=false 与 prev_online=true 比较，
    // 每个巡检周期都误报"在线→离线"通知。
    // 修复：状态变更检测统一用 any_online（任一适配器在线即在线）。
    let any_online = online || secondary_online == Some(true);
    handle_status_change(
        prev_online, any_online, reachable, login_available,
        &adapter1_name, &message,
        &adapter2_name, if secondary_message.is_empty() { None } else { Some(secondary_message.as_str()) },
        &config, app_handle,
    );

    let a1_campus = adapter_campus_message(&adapter1_name, &adapters, &campus_result);
    let a2_campus = if crate::network::is_secondary_adapter_enabled(&config, &adapter2_name) {
        adapter_campus_message(&adapter2_name, &adapters, &campus_result)
    } else { None };
    let a1_on_campus = adapter_campus_status(&adapter1_name, &adapters, &campus_result).map(|s| s.on_campus);
    let a2_on_campus = if crate::network::is_secondary_adapter_enabled(&config, &adapter2_name) {
        adapter_campus_status(&adapter2_name, &adapters, &campus_result).map(|s| s.on_campus)
    } else { None };

    emit_background_check_result(
        app_handle, state,
        &BackgroundCheckResult {
            online,
            online_operator,
            secondary_online_operator,
            reachable,
            login_available,
            message: &message,
            adapter1_name: &adapter1_name,
            adapter2_name: &adapter2_name,
            secondary_online,
            secondary_message: &secondary_message,
            dual_adapter: config.dual_adapter,
            config: &config,
            campus_result: &campus_result,
            a1_campus_msg: a1_campus.as_deref(),
            a2_campus_msg: a2_campus.as_deref(),
            a1_on_campus,
            a2_on_campus,
        },
    );

    try_auto_login_on_preparation(app_handle, state, login_available, online, &config);

    let reconnected = try_disconnect_reconnect(
        app_handle, state, online, secondary_online,
        a1, &adapter1_name, &adapter2_name,
        reachable, login_available, &config,
    );

    // 重连成功时状态已在 try_disconnect_reconnect 内设置，
    // 跳过 update_network_state 避免用重连前旧 Portal 结果覆盖 any_adapter_online
    if !reconnected {
        update_network_state(state, online, secondary_online, reachable, app_handle);
    }

    crate::log_debug!("background", "后台检测周期完成, 总耗时{}ms", t_total.elapsed().as_millis());
}

/// 后台巡检：只负责连通性/Portal/重连，质量检测由定时测试循环独占
/// （latency.rs 的 spawn_latency_test_loop → quality_scheduler::run_quality_check），
/// 避免双定时器叠加触发全量外网检测、60s 节流吞掉用户自定义间隔（2026-09-04 收敛）
pub async fn run_background_check(app_handle: &AppHandle, cancel_token: std::sync::Arc<tokio_util::sync::CancellationToken>) {
    let app_h = app_handle.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let s = app_h.state::<AppState>();
        run_background_check_blocking(&app_h, &s, &cancel_token)
    }).await.unwrap_or_else(|e| {
        crate::log_error!("background", "后台检测异常: {}", e);
    });
}

```

### tauri-app/src-tauri/src/monitor/latency.rs

```
use tauri::{AppHandle, Manager};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use crate::network::get_adapters_cached_async;
use crate::infra::state::AppState;
use crate::infra::notification::emit_notification;
use super::quality_scheduler::run_quality_check;

/// 触发"网络拥堵"通知的质量档位（quality_scheduler 复核同样以此为准）
pub(super) const BAD_LEVELS: &[&str] = &["poor", "bad"];
const GOOD_LEVELS: &[&str] = &["excellent", "great", "good"];

/// 与上次质量档位比较判断是否需要通知（纯判断，不落状态不发通知）：
/// Some("bad") = 恶化到 poor/bad，需经 quality_scheduler 复核确认后才通知；
/// Some("good") = 从 poor/bad 恢复。
pub(super) fn classify_quality_change(last: Option<&str>, current: &str, enable_notification: bool) -> Option<&'static str> {
    if !enable_notification {
        return None;
    }
    let last_q = last?;
    if current == last_q {
        return None;
    }
    let was_bad = BAD_LEVELS.contains(&last_q);
    let is_bad = BAD_LEVELS.contains(&current);
    let was_good = GOOD_LEVELS.contains(&last_q);
    let is_good = GOOD_LEVELS.contains(&current);

    if is_bad && !was_bad {
        Some("bad")
    } else if is_good && !was_good && was_bad {
        Some("good")
    } else {
        None
    }
}

pub(super) fn record_last_quality(state: &AppState, current: &str) {
    state.network.update(|s| s.last_network_quality = Some(Arc::from(current)));
}

pub(super) fn notify_quality_change(app_handle: &AppHandle, kind: &str) {
    if kind == "bad" {
        emit_notification(app_handle, "网络拥堵", "校园网延迟升高，网络可能拥堵", "mascot-busy");
        let _ = crate::infra::events::EventBus::new(app_handle).emit_login_log("校园网延迟升高，网络可能拥堵", "warning");
    } else {
        emit_notification(app_handle, "网络恢复", "校园网延迟已恢复正常", "mascot-celebrate");
        let _ = crate::infra::events::EventBus::new(app_handle).emit_login_log("校园网延迟已恢复正常", "info");
    }
}

/// 就绪等待：适配器未解析出 IP、或 Portal 尚未认证（此时外网 HTTPS
/// 必被拦截全超时，检测结果无意义且会误报"网络拥堵"）时无法产出
/// 有效质量数据，每 2s 短重试。条件一旦满足立即返回，让启动后首次
/// 结果缩短到数秒内。返回 None 表示取消令牌触发，调用方应直接退出。
pub(super) async fn wait_for_adapter_ready(
    app_handle: &AppHandle,
    cancel_token: &CancellationToken,
) -> Option<(String, String)> {
    loop {
        let s = app_handle.state::<AppState>();
        let config = s.config.load();
        let adapters = get_adapters_cached_async().await.unwrap_or_default();
        let (ip, name) = crate::network::select_adapter_for_quality(&adapters, &config);
        if !ip.is_empty() && s.network.load().any_adapter_online {
            return Some((name, ip));
        }
        drop(s);
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(2)) => {}
            _ = cancel_token.cancelled() => return None,
        }
    }
}

pub fn spawn_latency_test_loop(app_handle: &AppHandle) -> Result<(), String> {
    let app_h = app_handle.clone();
    app_handle.state::<AppState>().task_manager.spawn("latency_test", move |cancel_token| {
        async move {
            // 间隔动态读取：每轮重读配置与轻量化系数（轻量化下限 1800s，
            // 见 app/lightweight），设置页改间隔/进出轻量化即时生效。
            // 既有缺陷修复：interval 在 spawn 时捕获一次，运行中修改直到
            // 重启才生效。计时器按需重建，Delay 错过不补发
            let mut current_interval_ms: u64 = 0;
            let mut interval_timer = tokio::time::interval(Duration::from_millis(1));
            interval_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            let mut first_run = true;
            loop {
                let desired_ms = {
                    let s = app_h.state::<AppState>();
                    let cfg = s.config.load();
                    crate::app::lightweight::effective_quality_interval_ms(
                        cfg.latency_test_interval.max(10_000),
                        crate::app::lightweight::is_lightweight_active(),
                    )
                };
                if desired_ms != current_interval_ms {
                    current_interval_ms = desired_ms;
                    interval_timer = tokio::time::interval(Duration::from_millis(desired_ms));
                    interval_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                    if !first_run {
                        // 重建后的首 tick 立即到期，吞掉避免连发
                        interval_timer.tick().await;
                    }
                }
                if !first_run {
                    tokio::select! {
                        _ = interval_timer.tick() => {}
                        _ = cancel_token.cancelled() => break,
                    }
                }
                first_run = false;
                let s = app_h.state::<AppState>();
                if !s.task_manager.is_running("latency_test")
                    || s.exit.is_quitting.load(Ordering::Acquire) {
                    break;
                }
                // 就绪等待复用 wait_for_adapter_ready（此前未就绪走 continue，
                // 会立刻耗尽 interval 的即时首 tick，之后干等完整周期）
                let (adapter_ip, adapter_name) = {
                    match wait_for_adapter_ready(&app_h, &cancel_token).await {
                        Some(pair) => pair,
                        None => return,
                    }
                };
                // 检测前等待1秒，避免网络未稳定时HTTPS测试延迟异常
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {}
                    _ = cancel_token.cancelled() => break,
                }
                // 调用统一的 quality_scheduler 执行检测（含 semaphore 互斥、emit、通知）；
                // 传循环取消令牌，停止定时测试后正在执行的一轮提前返回
                run_quality_check(&app_h, &adapter_name, &adapter_ip, Some(&cancel_token)).await;
            }
        }
    })
}

/// 启动即测一次（v2.4.2）：定时测试关闭但质量检测总开关开启时，应用启动后
/// 等网络就绪执行一次全量检测（结果经 EventBus 推前端），然后任务退出。
/// 定时周期检测仍由 spawn_latency_test_loop 负责；此处只补"启动首测"。
pub(super) async fn run_startup_quality_once(app_handle: AppHandle, cancel_token: Arc<CancellationToken>) {
    // 就绪等待与主循环一致：适配器无 IP / Portal 未认证时重试
    let Some((adapter_name, adapter_ip)) = wait_for_adapter_ready(&app_handle, &cancel_token).await else {
        return;
    };
    // 退出/任务被取消时不做无意义检测
    let s = app_handle.state::<AppState>();
    if s.exit.is_quitting.load(Ordering::Acquire) {
        return;
    }
    // 与主循环一致：检测前等待1秒，避免网络未稳定时 HTTPS 测试延迟异常
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(1)) => {}
        _ = cancel_token.cancelled() => return,
    }
    run_quality_check(&app_handle, &adapter_name, &adapter_ip, Some(&cancel_token)).await;
}

#[cfg(test)]
mod tests {
    use super::classify_quality_change;

    #[test]
    fn degraded_to_bad_level_triggers_pending_bad() {
        assert_eq!(classify_quality_change(Some("good"), "bad", true), Some("bad"));
        assert_eq!(classify_quality_change(Some("excellent"), "poor", true), Some("bad"));
    }

    #[test]
    fn recovery_from_bad_level_triggers_good() {
        assert_eq!(classify_quality_change(Some("bad"), "good", true), Some("good"));
        assert_eq!(classify_quality_change(Some("poor"), "excellent", true), Some("good"));
    }

    #[test]
    fn same_level_or_unknown_last_never_notifies() {
        assert_eq!(classify_quality_change(Some("bad"), "bad", true), None);
        assert_eq!(classify_quality_change(Some("good"), "good", true), None);
        assert_eq!(classify_quality_change(Some(None, "bad", true), None);
    }

    #[test]
    fn middle_levels_crossing_never_notifies() {
        // fair/unknown 等中间档位与任意档位互转均不通知（与历史行为一致）
        assert_eq!(classify_quality_change(Some("good"), "fair", true), None);
        assert_eq!(classify_quality_change(Some("fair"), "bad", true), Some("bad"));
        assert_eq!(classify_quality_change(Some("bad"), "fair", true), None);
        assert_eq!(classify_quality_change(Some("poor"), "poor", true), None);
    }

    #[test]
    fn notification_disabled_never_classifies() {
        assert_eq!(classify_quality_change(Some("good"), "bad", false), None);
        assert_eq!(classify_quality_change(Some("bad"), "good", false), None);
    }
}

```


这篇文章的当前内容（基于旧版代码，供保留章节结构与 title/type/tags）：

---
title: 质量检测驱动者收敛为定时测试循环独占
type: decision
source_files:
  - tauri-app/src-tauri/src/monitor/background_check.rs
  - tauri-app/src-tauri/src/monitor/latency.rs
tags: [决策, 质量检测, 巡检, 定时器]
---

## 背景

曾有两个定时器叠加驱动质量检测，导致高频重复探测。

## 决策

2026-09-04：质量检测的驱动者**收敛为定时测试循环独占**（后台巡检不再触发全量质量检测）。

## 理由

消除双定时器叠加造成的高频重复探测；代价是"不开定时测试则质量面板无周期数据"，这是有意接受的。

## 备选方案

双定时器叠加驱动（后台巡检 + 定时测试）——因高频重复探测被弃用。

## 影响与约束

不开定时测试时质量面板没有周期数据，由**手动检测按钮兜底**。新增质量数据来源前先确认是否会引入第二个驱动者。

## Connections

[[quality-detail-key-contract]]、[[network-quality-default-off]]、[[deliberate-background-check-skips]]