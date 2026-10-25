use tauri::{AppHandle, Manager};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use crate::network::get_adapters_force;
use crate::auth::portal::check_portal_full;
use crate::infra::state::{AppState, CommandResult};
use crate::infra::command_context::CommandContext;
use crate::infra::events::EventBus;
use crate::infra::notification::emit_notification;
use crate::infra::lifecycle::{start_auto_exit, start_campus_exit};

const RECONNECT_REMINDER_INTERVAL: u32 = 10;

/// 准备自动登录连续失败上限：达到后退避降频（见 [`prep_login_backoff_secs`]）。
/// 历史缺陷一：自动登录无限重试，错误凭据下每 ~60s 重试一次且每 5 次认证失败
/// 触发全适配器 DHCP 释放续租（断网），应用无人值守时周期性断网持续一整夜。
/// 历史缺陷二（本次修复）：达上限后"本会话永久停止自动登录"——在可登录
/// 时间段之外多次登录失败把计数拉满后，到了可登录时间段依旧不再尝试，
/// 只有手动登录成功/注销才能复位。现为指数退避降频、有界且不放弃。
const PREP_LOGIN_MAX_FAILURES: u32 = 5;

/// 失败退避封顶（秒）：检测报可登录（离线）期间降频到最长 5 分钟一次。
/// 封顶必须远小于登录时段间隔（小时级），保证"非登录时段开机、到了
/// 可登录时段"场景最坏等一个封顶周期必然恢复登录；时间窗拒绝类失败
/// 不计入退避（见 [`prep_failures_after_failure`]），实际通常 ≤ 一个基础冷却。
const PREP_LOGIN_BACKOFF_CAP_SECS: u64 = 300;

/// 断线重连尝试是否放行（纯函数便于单测）：
/// 未达上限照常；达到上限后每 [`RECONNECT_REMINDER_INTERVAL`] 个周期再放行一次
/// （降频重试而非进入"只提醒、永不重连"终态），重连成功后计数复位。
pub fn reconnect_attempt_allowed(reconnect_count: u32, max_reconnect: u32) -> bool {
    reconnect_count <= max_reconnect
        || (reconnect_count - max_reconnect) % RECONNECT_REMINDER_INTERVAL == 0
}

/// 准备自动登录的重试冷却时长（纯函数便于单测）：
/// 失败次数未达上限时用普通冷却；达到上限后按超出次数指数翻倍，封顶
/// [`PREP_LOGIN_BACKOFF_CAP_SECS`]——保证登录时间窗到达后最多等一个退避周期必然重试。
pub fn prep_login_backoff_secs(failures: u32, base_cooldown_secs: u64) -> u64 {
    if failures < PREP_LOGIN_MAX_FAILURES {
        return base_cooldown_secs;
    }
    // 恰达上限即开始翻倍：excess=1 → 2×base，之后每多失败一次翻一倍
    let excess = (failures - PREP_LOGIN_MAX_FAILURES + 1).min(11);
    base_cooldown_secs
        .saturating_mul(1u64 << excess)
        .min(PREP_LOGIN_BACKOFF_CAP_SECS)
        .max(base_cooldown_secs)
}

/// 冷却是否已就绪（纯函数便于单测）：距上次尝试的耗时达到当前退避冷却即可重试
pub fn prep_login_cooldown_ready(failures: u32, elapsed_secs: u64, base_cooldown_secs: u64) -> bool {
    elapsed_secs >= prep_login_backoff_secs(failures, base_cooldown_secs)
}

/// 登录失败消息是否为"可登录时间段未到"类拒绝（纯函数便于单测）。
///
/// 门户检测的 `login_available` 只表示"未在线即可尝试"，不是时段信号——
/// 时间窗外检测仍报可登录，登录必然被服务端拒绝。这类失败重试无害且
/// 窗口打开后应立即恢复，因此不参与指数退避（见失败处理分支）。
pub fn is_time_window_refusal(msg: &str) -> bool {
    let m = msg.trim();
    if m.is_empty() {
        return false;
    }
    const CN_KEYWORDS: [&str; 6] = ["时间段", "时段", "时间范围", "服务时间", "时间未到", "不在允许"];
    if CN_KEYWORDS.iter().any(|k| m.contains(k)) {
        return true;
    }
    let lower = m.to_lowercase();
    lower.contains("time range") || lower.contains("not allowed") && lower.contains("time")
}

/// 失败后准备自动登录计数的更新（纯函数便于单测）：
/// 时间窗拒绝不累加（保持基础冷却轻探测，窗口打开后下一个检测周期即可登录），
/// 其余失败正常累加进入退避。
pub fn prep_failures_after_failure(failures: u32, failure_msg: &str) -> u32 {
    if is_time_window_refusal(failure_msg) {
        failures
    } else {
        failures.saturating_add(1)
    }
}

/// 断线重连是否应向上报告"已重连成功"。
///
/// 只有登录成功且未超过重连次数上限时才返回 true，调用方据此
/// 跳过用重连前旧 Portal 快照覆盖 `any_adapter_online` 的逻辑。
pub fn reconnect_should_report(login_success: bool, within_limit: bool) -> bool {
    login_success && within_limit
}

pub fn try_auto_login_on_preparation(
    app_handle: &AppHandle,
    state: &AppState,
    login_available: bool,
    online: bool,
    config: &crate::config::model::Config,
) {
    // 先同步 login_available 沿检测（须在早退之前：反向沿也要记录，
    // 否则离线期间标志滞留 true，门户恢复时的上升沿无从识别）。
    // 上升沿（门户重新可达）视为新的登录机会，复位失败计数退出退避。
    {
        let prev = state.network.load();
        if prev.last_login_available != login_available {
            if login_available && prev.prep_login_failures > 0 {
                crate::log_info!("auto_login", "门户检测恢复可用（login_available 上升沿），复位准备自动登录失败计数 {}", prev.prep_login_failures);
                state.network.update(|s| {
                    s.last_login_available = login_available;
                    s.prep_login_failures = 0;
                });
            } else {
                state.network.update(|s| s.last_login_available = login_available);
            }
        }
    }

    if !login_available || online || !config.auto_login_on_preparation {
        return;
    }

    // 单次 load 快照，复用读取 has_logged_online / logout_protected_until / last_auto_login_attempt
    let snap = state.network.load();
    if snap.has_logged_online {
        return;
    }

    // 连续失败达上限：不再永久停止，改为指数退避降频（prep_login_backoff_secs），
    // 保证可登录时间窗到达后最多等一个退避周期必然重试；成功/注销仍会复位计数。
    if snap.prep_login_failures >= PREP_LOGIN_MAX_FAILURES {
        crate::log_debug!(
            "auto_login",
            "准备自动登录连续失败 {} 次，进入退避重试（冷却{}秒）",
            snap.prep_login_failures,
            prep_login_backoff_secs(snap.prep_login_failures, config.auto_login_cooldown_secs)
        );
    }

    let protected_until = snap.logout_protected_until;
    if std::time::Instant::now() < protected_until {
        crate::log_debug!("auto_login", "注销保护期内，跳过自动登录");
        return;
    }

    // 冷却按当前失败计数退避（上限内=普通冷却，达上限=指数退避退避封顶 5 分钟）
    let failures = snap.prep_login_failures;
    let last_attempt = snap.last_auto_login_attempt;
    if !prep_login_cooldown_ready(failures, last_attempt.elapsed().as_secs(), config.auto_login_cooldown_secs) {
        crate::log_debug!(
            "auto_login",
            "自动登录冷却中（{}秒内不重复，失败计数={}），跳过",
            prep_login_backoff_secs(failures, config.auto_login_cooldown_secs),
            failures
        );
        return;
    }

    crate::log_info!("auto_login", "触发自动登录: loginAvailable={}, online={}", login_available, online);
    if let Some(_login_guard) = state.tasks.is_logging_in.try_acquire() {
        let t0 = std::time::Instant::now();
        state.network.update(|s| s.last_auto_login_attempt = std::time::Instant::now());
        let login_result = crate::auth::service::full_login(state, app_handle, None);
        let elapsed = t0.elapsed();

        crate::log_info!("login", "自动登录完成: success={}, message={}, 耗时{}ms",
            login_result.success,
            login_result.message.clone().unwrap_or_else(|| "无消息".to_string()),
            elapsed.as_millis());

        if let Err(e) = EventBus::new(app_handle).emit_auto_login_result(
            login_result.success,
            &login_result.message.clone().unwrap_or_default(),
            false,
        ) {
            crate::log_warn!("auto_login", "发送自动登录结果失败: {}", e);
        }

        if login_result.success {
            state.network.update(|s| {
                s.has_logged_online = true;
                s.prep_login_failures = 0;
            });
            if config.enable_notification {
                emit_notification(app_handle, "自动登录成功", &login_result.message.unwrap_or_default(), "mascot-celebrate");
            }
        } else {
            // 登录失败（含认证失败）：累加准备自动登录失败计数进入退避。
            // 例外："可登录时间段未到"类拒绝不计入——这类失败注定发生且窗口打开后
            // 立即应恢复，保持基础冷却轻探测即可，避免退避把窗口开启时刻拖后 30 分钟。
            let failure_msg = login_result.message.clone().unwrap_or_default();
            if is_time_window_refusal(&failure_msg) {
                crate::log_info!("auto_login", "准备自动登录被拒（可登录时间段未到），不计入退避: {}", failure_msg);
            } else {
                state.network.update(|s| s.prep_login_failures = prep_failures_after_failure(s.prep_login_failures, &failure_msg));
                let failures = state.network.load().prep_login_failures;
                crate::log_info!("auto_login", "准备自动登录失败 [{}/{}]", failures, PREP_LOGIN_MAX_FAILURES);
            }
        }
    } else {
        crate::log_debug!("auto_login", "自动登录跳过：已有登录任务在进行");
    }
}

#[allow(clippy::too_many_arguments)]
pub fn try_disconnect_reconnect(
    app_handle: &AppHandle,
    state: &AppState,
    online: bool,
    secondary_online: Option<bool>,
    a1: Option<&crate::network::Adapter>,
    adapter1_name: &str,
    adapter2_name: &str,
    reachable: bool,
    login_available: bool,
    config: &crate::config::model::Config,
) -> bool {
    let any_offline = (!online && a1.is_some()) || secondary_online == Some(false);

    // 单次 load 快照，复用读取 any_adapter_online / logout_protected_until / last_auto_login_attempt / disconnect_reconnect_count
    let snap = state.network.load();
    if !snap.any_adapter_online || !any_offline || !reachable || !login_available || !config.auto_login_on_preparation {
        return false;
    }

    let protected_until = snap.logout_protected_until;
    if std::time::Instant::now() < protected_until {
        crate::log_debug!("auto_login", "注销保护期内，跳过断线重连");
        return false;
    }

    let last_attempt = snap.last_auto_login_attempt;
    if last_attempt.elapsed().as_secs() < config.auto_login_cooldown_secs {
        return false;
    }

    // 先尝试获取登录锁，成功后再增加重连计数，避免锁获取失败但计数器已增加
    let login_guard = match state.tasks.is_logging_in.try_acquire() {
        Some(g) => g,
        None => {
            crate::log_debug!("auto_login", "断线重连：登录锁被占用，跳过本次");
            return false;
        }
    };

    // 历史缺陷：读-增-取值非原子（对齐 failure_tracker.rs 的 update_with_result 模式）
    let reconnect_count = state.network.update_with_result(|s| {
        s.disconnect_reconnect_count += 1;
        s.disconnect_reconnect_count
    });
    // 上限内照常重连；超限后每 RECONNECT_REMINDER_INTERVAL 个周期再放行一次
    // （修复：原"超限即只提醒、永不重连"的终态会让断线后到可登录时段也无法恢复）
    let within_limit = reconnect_attempt_allowed(reconnect_count, config.max_disconnect_reconnect);
    let mut reconnect_success = false;

    if within_limit {
        let offline_adapter = if !online { adapter1_name } else { adapter2_name };
        emit_notification(app_handle, "检测到断线", &format!("{offline_adapter} 已离线，正在自动重连 ({reconnect_count}/{})", config.max_disconnect_reconnect), "mascot-alert");
        // emit_notification 仅覆盖非前台系统通知，前台过程信息走日志通道
        let _ = EventBus::new(app_handle).emit_login_log(
            &format!("检测到断线: {offline_adapter} 已离线，正在自动重连 ({reconnect_count}/{})", config.max_disconnect_reconnect),
            "warning",
        );

        crate::log_info!("auto_login", "断线重连 [{}/{}]: 离线适配器={}, online={}, secondaryOnline={}",
            reconnect_count, config.max_disconnect_reconnect, offline_adapter, online, secondary_online.unwrap_or(true));

        let _login_guard = login_guard;
        let t0 = std::time::Instant::now();
        state.network.update(|s| s.last_auto_login_attempt = std::time::Instant::now());
        let reconnect_result = crate::auth::service::full_login(state, app_handle, None);
        let elapsed = t0.elapsed();

        crate::log_info!("login", "断线重连结果 [{}/{}]: success={}, 耗时{}ms",
            reconnect_count, config.max_disconnect_reconnect, reconnect_result.success, elapsed.as_millis());

        if reconnect_result.success {
            reconnect_success = true;
            state.network.update(|s| {
                s.disconnect_reconnect_count = 0;
                s.any_adapter_online = true;
                s.has_logged_online = true;
                s.prep_login_failures = 0;
            });
            if let Err(e) = crate::config::persist::append_login_history(app_handle, true, "断线重连成功", offline_adapter, &config.user, "reconnect") {
                crate::log_warn!("auto_login", "记录重连历史失败: {}", e);
            }
            let reconnect_msg = format!("断线重连成功: {}", reconnect_result.message.unwrap_or_default());
            if let Err(e) = EventBus::new(app_handle).emit_auto_login_result(true, &reconnect_msg, false) {
                crate::log_warn!("auto_login", "发送重连结果失败: {}", e);
            }
        }
    } else {
        // 超限：重连次数已达上限，释放登录锁，避免阻塞用户手动登录
        drop(login_guard);
        if reconnect_count == config.max_disconnect_reconnect + 1 {
            crate::log_warn!("auto_login", "断线重连已达上限({}), 停止自动重连", config.max_disconnect_reconnect);
            emit_notification(app_handle, "断线重连失败", "已达到最大重连次数，请手动登录", "mascot-offline");
            let _ = EventBus::new(app_handle).emit_login_log("断线重连失败: 已达到最大重连次数，请手动登录", "error");
        } else if reconnect_count > config.max_disconnect_reconnect + 1 && (reconnect_count - config.max_disconnect_reconnect - 1) % RECONNECT_REMINDER_INTERVAL == 0 {
            emit_notification(app_handle, "网络仍断线", &format!("{} 仍处于离线状态，请手动登录或检查网络", if !online { adapter1_name } else { adapter2_name }), "mascot-offline");
            let _ = EventBus::new(app_handle).emit_login_log(
                &format!("网络仍断线: {} 仍处于离线状态，请手动登录或检查网络", if !online { adapter1_name } else { adapter2_name }),
                "warning",
            );
        }
    }

    // 历史缺陷：此函数所有路径均返回 false，导致调用方 background_check.rs 的
    // "if !reconnected" 分支永远用重连前旧 Portal 快照覆盖 any_adapter_online，
    // 单适配器用户重连成功后立即被重新标记为离线，且下一周期因 !any_adapter_online
    // 提前返回、不再尝试重连，用户被永久困在离线状态。
    // 修复：重连成功（且未超限）时返回 true，让调用方跳过旧快照覆盖。
    reconnect_should_report(reconnect_success, within_limit)
}

pub fn run_auto_login_on_start(app_handle: &AppHandle) {
    let s = CommandContext::from_app(app_handle);
    let config = s.config.load_full();

    if !config.auto_login_on_start {
        return;
    }

    let is_auto_start = std::env::args().any(|a| a == "--autostart");
    let initial_delay = if is_auto_start { 5000u64 } else { 1500u64 };

    crate::log_info!("auto_login", "开机自启登录流程启动: isAutoStart={}, initialDelay={}ms, dualAdapter={}",
        is_auto_start, initial_delay, config.dual_adapter);

    let app_h = app_handle.clone();
    let task_manager = s.task_manager.clone();
    if let Err(e) = task_manager.spawn("auto_login_on_start", move |_cancel| async move {
        tokio::time::sleep(Duration::from_millis(initial_delay)).await;

        let s = app_h.state::<AppState>();
        if s.exit.is_quitting.load(Ordering::Acquire) || s.network.load().has_logged_online {
            return;
        }

        let t_adapters = std::time::Instant::now();
        let mut adapters = match tauri::async_runtime::spawn_blocking(move || {
            get_adapters_force()
        }).await {
            Ok(Ok(a)) => a,
            _ => {
                crate::log_error!("auto_login", "获取适配器列表失败，终止开机自启登录");
                if let Err(e) = EventBus::new(&app_h).emit_auto_login_result(false, "获取适配器列表失败，终止开机自启登录", false) {
                    crate::log_warn!("auto_login", "发送启动登录失败事件失败: {}", e);
                }
                return;
            },
        };

        crate::log_info!("auto_login", "适配器列表获取完成: {}个适配器, 耗时{}ms",
            adapters.len(), t_adapters.elapsed().as_millis());

        for (i, a) in adapters.iter().enumerate() {
            crate::log_debug!("auto_login", "  [{}] name={}, ip={}, wireless={}",
                i, a.name, a.ip, a.wireless);
        }

        if is_auto_start {
            for retry in 0..3 {
                let has_ip = adapters.iter().any(|a| !a.ip.is_empty());
                if has_ip { break; }
                crate::log_debug!("auto_login", "开机自启：适配器未就绪，等待重试 ({}/3)", retry + 1);
                tokio::time::sleep(Duration::from_secs(3)).await;
                let s = app_h.state::<AppState>();
                if s.exit.is_quitting.load(Ordering::Acquire) { return; }
                adapters = match tauri::async_runtime::spawn_blocking(|| {
                    get_adapters_force()
                }).await {
                    Ok(Ok(a)) => a,
                    _ => return,
                };
            }
        }

        let s = app_h.state::<AppState>();
        let config = s.config.load_full();
        let (adapter1_name, adapter2_name) = crate::network::resolve_adapter_names(&adapters, &config);

        if config.enable_network_name_check {
            let now_min = crate::infra::local_now_minutes();
            let skip_campus = crate::monitor::campus_check::is_campus_check_silent(now_min, config.campus_check_start_minutes, config.campus_check_end_minutes);

            if skip_campus {
                let hour = config.campus_check_start_minutes / 60;
                let minute = config.campus_check_start_minutes % 60;
                // 静默期（检测时段外）连自动登录一并跳过：与后台巡检同口径
                // （2026-10-25 修复，见 background_check.rs 同名注释）——开机自启若仍在
                // 静默期发起 Portal 检测/登录，会与巡检形成两条整夜空转的认证请求源。
                crate::log_info!(
                    "auto_login",
                    "开机自启: 校园网检测静默期（不在 {:02}:{:02} 起的检测时段内），跳过校园网环境验证与自动登录",
                    hour, minute
                );
                s.network.update(|s| s.on_campus_network = true);
                return;
            } else {
                // 校园网检测任务异常（panic 或被取消）时不能误判为"不在校园网"，
                // 否则会触发 start_campus_exit 将用户错误踢出。此处直接跳过本次检测。
                // 检测范围只含解析后的主/副适配器，与其他操作的作用范围一致
                let campus_result = match tauri::async_runtime::spawn_blocking({
                    let cfg = config.clone();
                    let adps = crate::network::filter_operation_adapters(&adapters, &adapter1_name, &adapter2_name);
                    move || crate::monitor::watcher::check_campus_network(&cfg, &adps)
                }).await {
                    Ok(result) => result,
                    Err(e) => {
                        crate::log_error!("auto_login", "校园网检测任务异常，跳过本次检测以避免误触发退出: {}", e);
                        return;
                    }
                };

                if !campus_result.on_campus {
                    crate::log_info!("auto_login", "开机自启: 校园网检测未通过，跳过自动登录 - {}", campus_result.message);
                    s.network.update(|s| {
                        s.current_ssid = campus_result.current_ssid.as_deref().map(Arc::from);
                        s.on_campus_network = campus_result.on_campus;
                    });
                    let _ = EventBus::new(&app_h).emit_auto_login_result(false, &campus_result.message, true);
                    // 如果配置的适配器均无IP（完全无网络），跳过退出，等待网络恢复
                    let a1_has_ip = adapters.iter().any(|a| a.name == adapter1_name && !a.ip.is_empty());
                    let a2_has_ip = crate::network::is_secondary_adapter_enabled(&config, &adapter2_name)
                        && adapters.iter().any(|a| a.name == adapter2_name && !a.ip.is_empty());
                    if !a1_has_ip && !a2_has_ip {
                        crate::log_info!("auto_login", "配置的适配器均无IP地址，跳过校园网退出，等待网络恢复");
                    } else {
                        // 校园网验证不通过：触发最小化+退出流程
                        start_campus_exit(&app_h, &s);
                    }
                    return;
                }

                s.network.update(|s| {
                    s.current_ssid = campus_result.current_ssid.as_deref().map(Arc::from);
                    s.on_campus_network = true;
                });
                crate::log_info!("auto_login", "开机自启: 校园网检测通过 - {}", campus_result.message);
            }
        }

        let (a1_found, a2_ref) = crate::network::find_dual_adapters(&adapters, &config, &adapter1_name, &adapter2_name);
        if let Some(a1) = a1_found {
            let ip1 = a1.ip.clone();
            let name1 = a1.name.clone();
            let ip1_for_log = ip1.clone();
            let a2_opt = a2_ref.cloned();

            let mut name2_opt = None;
            let name1_for_msg = name1.clone();

            let t_portal = std::time::Instant::now();
            // 状态探测为只读操作（check_portal_full 不再接受/使用凭据），
            // 不向登录端点发送账号密码，避免错误凭据触发锁定或静默登录
            let portal_result = if let Some(a2) = a2_opt {
                let ip2 = a2.ip.clone();
                name2_opt = Some(a2.name.clone());
                // 双适配器并行 Portal 检测：先 spawn 两个 handle，再分别 await
                // 原 spawn->await->spawn->await 串行，改为并行可显著缩短双适配器检测耗时
                let h1 = tauri::async_runtime::spawn_blocking(move || check_portal_full(&ip1, Some(&name1)));
                let h2 = tauri::async_runtime::spawn_blocking(move || check_portal_full(&ip2, Some(&a2.name)));
                let r1 = h1.await;
                let r2 = h2.await;
                (r1, Some(r2))
            } else {
                let r1 = tauri::async_runtime::spawn_blocking(move || check_portal_full(&ip1, Some(&name1))).await;
                (r1, None)
            };

            let portal_elapsed = t_portal.elapsed();

            if let (Ok(Ok(portal_status)), sec_res) = &portal_result {
                crate::log_info!("auto_login", "Portal检测结果: {}[ip={}] → online={}, reachable={}, loginAvailable={}, 耗时{}ms; 副适配器→ {:?}",
                    name1_for_msg, ip1_for_log, portal_status.online, portal_status.reachable,
                    portal_status.login_available,
                    portal_elapsed.as_millis(),
                    sec_res.as_ref().map(|r| match r {
                        Ok(Ok(s)) => format!("online={}, msg={}", s.online, s.message),
                        Ok(Err(e)) => format!("err={e}"),
                        Err(e) => format!("join_err={e}"),
                    }));

                if portal_status.online {
                    let mut adapter_names = vec![name1_for_msg.clone()];
                    let _sec_online = match sec_res {
                        Some(Ok(Ok(sec_status))) => {
                            if sec_status.online {
                                if let Some(ref n2) = name2_opt {
                                    adapter_names.push(n2.clone());
                                }
                            }
                            sec_status.online
                        },
                        _ => false,
                    };
                    let msg = format!("已在线（{}）", adapter_names.join("、"));

                    s.network.update(|s| {
                        s.any_adapter_online = true;
                        s.has_logged_online = true;
                        s.prep_login_failures = 0;
                    });

                    crate::log_info!("auto_login", "已在线，跳过登录: 适配器=[{}]", adapter_names.join(", "));

                    let _ = EventBus::new(&app_h).emit_auto_login_result(true, &msg, true).map_err(|e| crate::log_warn!("auto_login", "发送跳过登录结果失败: {}", e));
                    return;
                }
            } else {
                match &portal_result {
                    (Err(e), _) => crate::log_warn!("auto_login", "主适配器Portal检测异常: {}, 耗时{}ms", e, portal_elapsed.as_millis()),
                    (_, Some(Err(e))) => crate::log_warn!("auto_login", "副适配器Portal检测异常: {}", e),
                    _ => {}
                }
            }
        } else {
            crate::log_warn!("auto_login", "未找到可用主适配器: 目标={}, 可用适配器数={}",
                adapter1_name, adapters.iter().filter(|a| !a.ip.is_empty()).count());
        }

        let t_login = std::time::Instant::now();
        let app_h_login = app_h.clone();
        let login_result = tauri::async_runtime::spawn_blocking(move || {
            let s = app_h_login.state::<AppState>();
            if s.network.load().has_logged_online {
                return CommandResult { success: true, message: Some("已在线，跳过登录".to_string()), data: None };
            }
            let _guard = match s.tasks.is_logging_in.try_acquire() {
                Some(g) => g,
                None => return CommandResult::err("登录正在进行中"),
            };
            s.network.update(|s| s.last_auto_login_attempt = std::time::Instant::now());
            crate::auth::service::full_login(&s, &app_h_login, None)
        }).await;

        let login_elapsed = t_login.elapsed();

        if let Ok(login_result) = login_result {
            crate::log_info!("login", "开机自启登录结果: success={}, message={}, 耗时{}ms",
                login_result.success,
                login_result.message.clone().unwrap_or_else(|| "无".to_string()),
                login_elapsed.as_millis());

            if let Err(e) = EventBus::new(&app_h).emit_auto_login_result(
                login_result.success,
                &login_result.message.clone().unwrap_or_default(),
                false,
            ) {
                crate::log_warn!("auto_login", "发送自动登录结果失败: {}", e);
            }

            if login_result.success {
                let s = app_h.state::<AppState>();
                s.network.update(|s| {
                    s.has_logged_online = true;
                    s.prep_login_failures = 0;
                });
                if config.enable_notification {
                    emit_notification(&app_h, "自动登录成功", &login_result.message.unwrap_or_default(), "mascot-celebrate");
                }

                if config.auto_exit_after_login {
                    let s = app_h.state::<AppState>();
                    start_auto_exit(&app_h, &s);
                }
            }
        }
    }) {
        crate::log_warn!("auto_login", "注册 auto_login_on_start 跟踪任务失败: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_success_within_limit_reports_true() {
        // 重连成功且未超限 → 必须返回 true，调用方才不会用旧 Portal 快照覆盖在线状态
        assert!(reconnect_should_report(true, true));
    }

    #[test]
    fn time_window_refusal_classification() {
        // 中文时段拒绝文案 → 识别为时间窗拒绝
        assert!(is_time_window_refusal("当前不在可登录时间段内"));
        assert!(is_time_window_refusal("不在允许的登录时段"));
        assert!(is_time_window_refusal("登录时间未到，请稍后再试"));
        assert!(is_time_window_refusal("超出服务时间范围"));
        // 认证/凭据/传输类失败 → 不是时间窗拒绝
        assert!(!is_time_window_refusal("认证失败：密码错误"));
        assert!(!is_time_window_refusal("AC认证失败"));
        assert!(!is_time_window_refusal("登录请求失败: connection reset"));
        assert!(!is_time_window_refusal("账号已过期"));
        assert!(!is_time_window_refusal(""));
    }

    #[test]
    fn time_window_refusal_does_not_grow_backoff() {
        // 时间窗拒绝不累加计数 → 冷却保持基础值，窗口打开后下一个检测周期即可重试
        assert_eq!(prep_failures_after_failure(0, "当前不在可登录时间段内"), 0);
        assert_eq!(prep_failures_after_failure(5, "不在允许的登录时段"), 5);
        // 其他失败正常累加进入退避
        assert_eq!(prep_failures_after_failure(0, "认证失败：密码错误"), 1);
        assert_eq!(prep_failures_after_failure(9, "登录请求失败: timeout"), 10);
    }

    #[test]
    fn reconnect_failure_never_reports_true() {
        assert!(!reconnect_should_report(false, true));
        assert!(!reconnect_should_report(false, false));
    }

    #[test]
    fn reconnect_success_over_limit_reports_false() {
        // 超过重连次数上限：即使登录成功也不再上报（停止自动重连）
        assert!(!reconnect_should_report(true, false));
    }

    /// 上限内每次都放行重连
    #[test]
    fn reconnect_allowed_within_limit() {
        assert!(reconnect_attempt_allowed(0, 3));
        assert!(reconnect_attempt_allowed(3, 3));
    }

    /// 超限后不再每拍放行（防刷），但每 REMINDER_INTERVAL 周期放行一次（不放弃）
    #[test]
    fn reconnect_allowed_periodically_after_limit() {
        assert!(!reconnect_attempt_allowed(4, 3));
        assert!(!reconnect_attempt_allowed(9, 3));
        assert!(reconnect_attempt_allowed(13, 3), "超限后每10个周期应放行一次重连");
        assert!(reconnect_attempt_allowed(23, 3));
    }

    /// 失败未达上限：冷却即普通冷却，时间窗到达后按原节奏重试
    #[test]
    fn prep_login_backoff_below_limit_uses_base_cooldown() {
        assert_eq!(prep_login_backoff_secs(0, 60), 60);
        assert_eq!(prep_login_backoff_secs(PREP_LOGIN_MAX_FAILURES - 1, 60), 60);
    }

    /// 达上限后指数退避：60 → 120 → 240，封顶 300s 截断第三次翻倍
    #[test]
    fn prep_login_backoff_grows_exponentially_after_limit() {
        assert_eq!(prep_login_backoff_secs(PREP_LOGIN_MAX_FAILURES, 60), 120);
        assert_eq!(prep_login_backoff_secs(PREP_LOGIN_MAX_FAILURES + 1, 60), 240);
        assert_eq!(prep_login_backoff_secs(PREP_LOGIN_MAX_FAILURES + 2, 60), 300);
    }

    /// 退避封顶 5 分钟：失败再多也不超过 PREP_LOGIN_BACKOFF_CAP_SECS，
    /// 保证检测报可登录时最坏等一个封顶周期必然重试
    #[test]
    fn prep_login_backoff_capped() {
        assert_eq!(prep_login_backoff_secs(PREP_LOGIN_MAX_FAILURES + 10, 60), 300);
        assert_eq!(prep_login_backoff_secs(1000, 60), 300);
    }

    /// 修复核心断言：失败拉满后冷却期满（elapsed ≥ 退避时长）必须放行重试——
    /// 原"达上限永久停止"导致时间窗到达后依旧不登录
    #[test]
    fn prep_login_ready_after_backoff_elapsed_even_at_limit() {
        let failures = 8; // 远超上限
        assert!(!prep_login_cooldown_ready(failures, 60, 60));
        let backoff = prep_login_backoff_secs(failures, 60);
        assert!(prep_login_cooldown_ready(failures, backoff, 60), "退避期满必须放行重试");
        assert!(prep_login_cooldown_ready(failures, backoff + 1, 60));
    }
}
