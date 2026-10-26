use tauri::{AppHandle, Manager};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use crate::network::{get_adapters_cached, get_adapters_force};
use crate::infra::state::AppState;
use crate::infra::lifecycle::{start_campus_exit, cancel_campus_exit};
use crate::auth::failure_tracker::AdapterFailureCounter;
use super::auto_auth::{try_auto_login_on_preparation, try_disconnect_reconnect};
use super::campus_check::{adapter_campus_status, adapter_campus_message, check_campus_network};
use super::portal_check::{PortalCheckResult, check_adapter_portal};
use crate::auth::failure_tracker::handle_portal_request_failure;
use super::dynamic_interval::{portal_reset_window_open, CheckOutcome};
use super::background_emit::{handle_status_change, emit_background_check_result, update_network_state, BackgroundCheckResult};

/// 巡检是否应把「已登录过」标记复位（会话失效判定，纯函数便于单测）。
///
/// 三个条件缺一不可：
/// - `reachable`：Portal 服务可达，才有依据确认「当前确实未在线」；探测失败/
///   适配器无 IP 时 reachable=false，保持标记不动，避免瞬时抖动把刚建立的会话判死。
/// - `!any_online`：主副适配器都不在线（any_online 语义见调用点）。
/// - `has_logged_online`：此前确实登录过，才谈得上「复位」。
///
/// 历史缺陷：该标记只在登录成功时置位、永不复位，整夜休眠唤醒后自动登录被
/// 永久门控拦截，用户只能手动点登录（2026-10-22 修复）。
pub(crate) fn should_reset_logged_online(reachable: bool, any_online: bool, has_logged_online: bool) -> bool {
    reachable && !any_online && has_logged_online
}

/// 「夜间出站还原后」静默期持续放行标记（见 `request_check_after_outbound_restore`）。
///
/// 背景：还原窗口起点自 2026-10-26 起可配置（`nightRestoreMinutes`，默认 07:30），
/// 而校园网检测静默期起点是另一项设置（`campusCheckStartMinutes`，默认 07:40）。
/// 默认配置下还原时刻（07:30）本身就落在静默期内，还原完成后的补检会被整体跳过，
/// 于是校园网卡虽已启用、IP 已就绪，Portal 登录却要等到静默期结束才判、才登——
/// 用户 07:30 打开电脑就是断网态（2026-10-26 用户点名「还原后登录判定要生效」）。
///
/// 语义（2026-10-26 用户定，替代原「一次性放行」）：还原后**在本轮静默期的剩余时段内
/// 持续放行**，直到检测时段开始（静默期结束）才恢复常规门控。理由：只放一轮的话，
/// 若那一轮因校园网尚未恢复而登录失败，后续轮次又被静默期挡回去，用户会看到
/// 「已还原但整段静默期都不再重试」。放行期由「离开静默期」自动终结，不跨日残留
/// （进入检测时段即清除，当日 23:00 起的新静默期不会再被误放行）。
static RESTORE_SILENT_PASS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// 请求「还原后在静默期内持续放行」（由出站还原收尾调用）。
pub(crate) fn request_check_after_outbound_restore() {
    RESTORE_SILENT_PASS.store(true, Ordering::Release);
}

pub(crate) fn run_background_check_blocking(app_handle: &AppHandle, state: &AppState, cancel_token: &tokio_util::sync::CancellationToken) -> CheckOutcome {
    if state.exit.is_quitting.load(Ordering::Acquire) || cancel_token.is_cancelled() {
        return CheckOutcome::Skipped;
    }
    let Some(_check_guard) = state.tasks.is_checking.try_acquire() else {
        return CheckOutcome::Skipped;
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
                return CheckOutcome::Skipped;
            }
        }
    };

    crate::log_debug!("background", "适配器列表: {}个 (耗时{}ms)", adapters.len(), t_adapters.elapsed().as_millis());

    let (adapter1_name, adapter2_name) = crate::network::resolve_adapter_names(&adapters, &config);

    let (a1, a2) = crate::network::find_dual_adapters(&adapters, &config, &adapter1_name, &adapter2_name);

    let now_min = crate::infra::local_now_minutes();
    // 出站切换态（三份快照任一非空）：巡检整体跳过（portal 检测走热点出站必失败，
    // 会累计触发 MAC 重置提权与整夜告警；见 decisions/night-outbound-switch）。下一拍（30s）恢复。
    if crate::config::outbound_switch::outbound_restore_active(
        &config.outbound_metric_restore,
        &config.outbound_disabled_adapters,
        &config.outbound_standby_route,
    ) {
        crate::log_debug!("background", "出站切换态，跳过本轮巡检");
        return CheckOutcome::Skipped;
    }
    let campus_silent = crate::monitor::campus_check::is_campus_check_silent(
        now_min,
        config.campus_check_start_minutes,
        config.campus_check_end_minutes,
    );
    // 还原后静默期放行（持续到本轮静默期结束）：离开静默期即清除标记，放行期到此为止
    let mut forced = RESTORE_SILENT_PASS.load(Ordering::Acquire);
    if forced && !campus_silent {
        RESTORE_SILENT_PASS.store(false, Ordering::Release);
        forced = false;
    }
    // 校园网检测静默期（检测时段外）：整体跳过本轮巡检。
    //
    // 历史缺陷（2026-10-25 修复）：静默期原本只跳过「校园网环境验证」，Portal 登录
    // 检测、自动登录、断连重连照跑——校园网 23:00 断网后每分钟一轮、整夜向登录端点
    // 空转认证请求，日志刷 `Authentication fail`（auth/portal.rs:127 注释明示有账号
    // 锁定风险），并让「检测时段 07:40–23:00」这个设置项对用户完全失效。设置项文案
    // 「仅此时段内执行校园网检测」才是正确语义，故按文档语义收敛。
    //
    // 保留动作：清 on_campus_network（不误报在线）、取消可能残留的最小化+退出流程
    // （静默期本就不判定、不退出），然后按 Skipped 交还节奏控制——不累计退避、
    // 不更新 online 状态，避免静默期把「已登录」刷成离线并触发状态变更通知。
    //
    // 例外（2026-10-26）：夜间出站还原后，本轮静默期的剩余时段全部放行（forced）。还原
    // 把校园网卡从设备级禁用里启用、DHCP 已就绪，此刻不判定就等于让用户等静默期结束才
    // 恢复登录——默认还原时刻 07:30 就在静默期内（检测时段 07:40 起），这条路径必被踩到。
    // 放行持续到静默期结束，让还原后的每一拍都能判定/自动登录，直到常规节奏接管。
    if campus_silent && !forced {
        cancel_campus_exit(app_handle, state);
        state.network.update(|s| {
            s.current_ssid = None;
            s.on_campus_network = true;
        });
        crate::log_debug!("background", "校园网检测静默期，跳过本轮巡检（Portal 检测/自动登录/断连重连均不执行）");
        return CheckOutcome::Skipped;
    }
    if campus_silent && forced {
        crate::log_debug!(
            "background",
            "夜间出站还原后静默期放行：照常执行本轮巡检（放行持续到静默期结束）"
        );
    }
    // 校园网检测只看解析后的主/副适配器：其他适配器的连接不属本应用管理范围
    let op_adapters = crate::network::filter_operation_adapters(&adapters, &adapter1_name, &adapter2_name);
    let campus_result = check_campus_network(&config, &op_adapters);
    // 始终更新 on_campus_network，确保 emit 字段一致
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
        return CheckOutcome::NotOnline;
    }

    // 校园网验证通过：取消之前的退出流程（如果有的话）
    cancel_campus_exit(app_handle, state);

    if cancel_token.is_cancelled() {
        return CheckOutcome::Skipped;
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
        // MAC 重置检测（门户不可达触发）只在每日 08:30 后启用：可登录时段之前
        // 门户拒绝/断连属预期，不应累计触发换 MAC/IP；窗口未开时清零残留计数，
        // 避免夜间失败在 8:31 第一拍立刻触发。
        if portal_reset_window_open(now_min) {
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
        } else {
            let (prev_a1, prev_a2) = {
                let snap = state.network.load();
                (snap.a1_auth_failure_count, snap.a2_auth_failure_count)
            };
            if prev_a1 > 0 || prev_a2 > 0 {
                state.network.update(|s| {
                    s.a1_auth_failure_count = 0;
                    s.a2_auth_failure_count = 0;
                });
                crate::log_info!(
                    "background",
                    "Portal失败但未到08:30（MAC重置检测窗口未开），重置失败计数 a1={}→0 a2={}→0",
                    prev_a1, prev_a2
                );
            }
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
    // 会话失效判定（本轮修复）：服务可达但主副卡都判定为未在线，说明此前建立的
    // 登录会话已失效（整夜休眠唤醒后网卡重连未登录、校园网卡被夜间出站切换禁用后
    // 又被启用、Portal 掉线）。此时必须重置 has_logged_online，否则下方
    // try_auto_login_on_preparation 的「已登录过」门控会把自动登录永久拦住——
    // 用户看到的就是「到点不自动登录，只能手动点」。
    // 本文件 :101 的重置只覆盖校园网检测失败（SSID/网络名变化）这一种失效，
    // 覆盖不到「网络名没变、只是会话超时或网卡被禁用后重启」的整夜休眠场景。
    // 必须在 try_auto_login_on_preparation 之前重置：本次巡检就能真正登录，
    // 不必再等下一拍（否则还原后补检钩子触发的那一拍仍会被旧标记挡回）。
    if should_reset_logged_online(reachable, any_online, state.network.load().has_logged_online) {
        state.network.update(|s| s.has_logged_online = false);
        crate::log_info!(
            "background",
            "巡检判定未在线（服务可达），已重置登录标记，允许自动登录重新介入"
        );
    }
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

    // 动态间隔结局分类（dynamic_interval.rs）：
    // - 门户可达且任一适配器在线 → Online（10 分钟慢拍）
    // - 门户不可达但此前在线 → OnlineButPortalUnreachable（1 分钟快排查），
    //   并联动一次网络质量检测（确认是否仍连着互联网），5 分钟节流防重复
    let outcome = if reachable {
        if any_online { CheckOutcome::Online } else { CheckOutcome::NotOnline }
    } else if prev_online {
        CheckOutcome::OnlineButPortalUnreachable
    } else {
        CheckOutcome::NotOnline
    };
    if outcome == CheckOutcome::OnlineButPortalUnreachable {
        trigger_fast_quality_check(app_handle, state, &adapters, &config);
    }

    crate::log_debug!("background", "后台检测周期完成, 总耗时{}ms", t_total.elapsed().as_millis());
    outcome
}

/// 「已登录但门户不可达」快排查模式的联动质量检测：全量外网连通性确认，
/// 结果照常推前端并落质量历史。5 分钟节流（snapshot.last_fast_quality_at），
/// 避免 1 分钟快拍下每拍都跑 19 项外网目标。
fn trigger_fast_quality_check(
    app_handle: &AppHandle,
    state: &AppState,
    adapters: &[crate::network::Adapter],
    config: &crate::config::model::Config,
) {
    let allowed = state
        .network
        .update_with_result(|s| {
            let now = std::time::Instant::now();
            let ok = s.last_fast_quality_at.map(|t| now.duration_since(t).as_secs() >= 300).unwrap_or(true);
            if ok {
                s.last_fast_quality_at = Some(now);
            }
            ok
        });
    if !allowed {
        crate::log_debug!("background", "门户不可达快排查：质量检测5分钟节流中，跳过本轮联动");
        return;
    }
    let (ip, name) = crate::network::select_adapter_for_quality(adapters, config);
    if ip.is_empty() {
        crate::log_debug!("background", "门户不可达快排查：无可质量检测适配器，跳过联动");
        return;
    }
    crate::log_info!(
        "background",
        "已登录但门户不可达：降为1分钟快排查，并联动网络质量检测（适配器[{}]，确认互联网连通性）",
        name
    );
    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let result = super::quality_scheduler::run_quality_check(&app_h, &name, &ip, None).await;
        // 明确回答"能否访问互联网"：外网站点平均延迟 > 0 即至少一个外网目标成功，
        // 互联网可达；全部失败则外网中断（此时门户+外网都不通，问题在本地网络/被踢下线）。
        // 结论写运行日志并推前端登录日志；系统通知仅在外网中断时发（可达不发，免打扰）。
        let internet_ok = result
            .as_ref()
            .and_then(|v| v.get("averageExternalLatency"))
            .and_then(serde_json::Value::as_i64)
            .map(|ms| ms > 0)
            .unwrap_or(false);
        let event_bus = crate::infra::events::EventBus::new(&app_h);
        match result {
            Some(_) if internet_ok => {
                crate::log_info!("background", "快排查结论：互联网可访问（外网站点可达）");
                let _ = event_bus.emit_login_log(
                    "门户暂不可达，但互联网访问正常（外网站点可达），继续观察",
                    "info",
                );
            }
            Some(_) => {
                crate::log_warn!("background", "快排查结论：互联网不可访问（全部外网站点失败）");
                let _ = event_bus.emit_login_log(
                    "门户与外网站点均不可达，互联网已中断，可能已被踢下线或网络故障",
                    "warning",
                );
                crate::infra::notification::emit_notification(
                    &app_h,
                    "互联网不可访问",
                    "校园网门户与外网站点均不可达，可能已被踢下线或网络故障，请检查网络",
                    "mascot-warn",
                );
            }
            None => {
                crate::log_debug!("background", "快排查：质量检测未能执行（与手动检测互斥），下轮快拍重试");
            }
        }
    });
}

/// 后台巡检：只负责连通性/Portal/重连，质量检测由定时测试循环独占
/// （latency.rs 的 spawn_latency_test_loop → quality_scheduler::run_quality_check），
/// 避免双定时器叠加触发全量外网检测、60s 节流吞掉用户自定义间隔（2026-09-04 收敛）
pub async fn run_background_check(app_handle: &AppHandle, cancel_token: std::sync::Arc<tokio_util::sync::CancellationToken>) -> CheckOutcome {
    let app_h = app_handle.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let s = app_h.state::<AppState>();
        run_background_check_blocking(&app_h, &s, &cancel_token)
    }).await.unwrap_or_else(|e| {
        crate::log_error!("background", "后台检测异常: {}", e);
        CheckOutcome::Skipped
    })
}

#[cfg(test)]
mod tests {
    use super::should_reset_logged_online;

    /// 在线时不重置：任何一张卡在线都说明会话有效
    #[test]
    fn no_reset_while_online() {
        assert!(!should_reset_logged_online(true, true, true));
        // 即使标记是 false，在线也不该「重置」（保持 false 即可，判定同样为否）
        assert!(!should_reset_logged_online(true, true, false));
    }

    /// 服务不可达时不重置：探测失败不能当作会话失效
    #[test]
    fn no_reset_when_unreachable() {
        assert!(!should_reset_logged_online(false, false, true));
    }

    /// 未登录过时无需重置：避免无谓写状态与日志噪音
    #[test]
    fn no_reset_when_never_logged_in() {
        assert!(!should_reset_logged_online(true, false, false));
    }

    /// 整夜休眠场景：可达 + 都不在线 + 曾登录 → 必须复位，允许自动登录介入
    #[test]
    fn reset_when_reachable_offline_and_logged_before() {
        assert!(should_reset_logged_online(true, false, true));
    }
}
