use tauri::{AppHandle, Manager};
use chrono::{Datelike, Timelike};
use crate::config::model::Config;
use crate::infra::state::AppState;
use crate::network::Adapter;

const REQUIRED_FAILURE_ROUNDS: u32 = 2;

/// 切回原出口所需的连续健康轮数（滞回两轮，避免原卡闪断时来回切）。
/// 与 REQUIRED_FAILURE_ROUNDS 同值但语义不同：那个是「连续失败多少轮判定故障」，
/// 这个是「连续健康多少轮判定恢复」，借用会让两处改动互相牵连。
const SWITCH_BACK_HEALTHY_ROUNDS: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecoveryOutcome {
    Succeeded,
    Failed,
    NotNeeded,
    Busy,
}

#[derive(Default)]
struct FailoverState {
    adapter: Option<String>,
    failures: u32,
    recovery_running: bool,
    recovery_adapter: Option<String>,
    recovery_result: Option<RecoveryOutcome>,
    recovery_attempted: bool,
    /// 切换态下正在判定"能否切回"的原校园网出口
    switch_back_adapter: Option<String>,
    /// 该出口连续健康的轮数
    switch_back_rounds: u32,
    /// 切换态下轮转尝试登录的游标（多受管卡时依次轮换，不固定首张）
    recovery_cursor: usize,
}

static STATE: std::sync::Mutex<FailoverState> = std::sync::Mutex::new(FailoverState {
    adapter: None,
    failures: 0,
    recovery_running: false,
    recovery_adapter: None,
    recovery_result: None,
    recovery_attempted: false,
    switch_back_adapter: None,
    switch_back_rounds: 0,
    recovery_cursor: 0,
});

fn next_failure_round(state: &FailoverState, adapter: &str, healthy: bool) -> u32 {
    if healthy {
        0
    } else if state.adapter.as_deref() == Some(adapter) {
        state.failures.saturating_add(1).min(REQUIRED_FAILURE_ROUNDS)
    } else {
        1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecoveryAction {
    Wait,
    Recovered,
    AttemptLogin,
    Switch,
}

fn recovery_action(result: Option<RecoveryOutcome>, managed: bool, attempted: bool) -> RecoveryAction {
    match result {
        Some(RecoveryOutcome::Busy) => RecoveryAction::Wait,
        Some(RecoveryOutcome::Succeeded) => RecoveryAction::Recovered,
        Some(RecoveryOutcome::Failed) | Some(RecoveryOutcome::NotNeeded) => RecoveryAction::Switch,
        None if managed && !attempted => RecoveryAction::AttemptLogin,
        None => RecoveryAction::Switch,
    }
}

fn failover_pending(failures: u32, recovery_running: bool) -> bool {
    failures >= REQUIRED_FAILURE_ROUNDS || recovery_running
}

fn failover_hold_day(today: i32, now_minutes: u32, restore_start: u32) -> i32 {
    if now_minutes < restore_start {
        today - 1
    } else {
        today
    }
}

fn is_managed_adapter(config: &Config, adapter_name: &str, adapter1: &str, adapter2: &str) -> bool {
    adapter_name == adapter1 || (config.dual_adapter && adapter_name == adapter2)
}

fn should_attempt_login(managed: bool, reachable: bool, login_available: bool, online: bool) -> bool {
    managed && reachable && login_available && !online
}

fn reset_state() {
    *STATE.lock().unwrap_or_else(|e| e.into_inner()) = FailoverState::default();
}

/// 运营商夜切在出口故障待决时让位，避免在切换前发起必然失败的校园认证。
pub(super) fn outbound_failover_pending() -> bool {
    let state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    failover_pending(state.failures, state.recovery_running)
}

/// 并行执行逐卡双目标 HTTPS，返回 source IP -> 公网可达。
/// 当前物理出口允许系统路由复核以兼容 TUN/代理，候选始终严格绑源。不经过质量检测单飞锁，不写历史或推送 UI。
pub(super) fn probe_internet_access(
    app_handle: &AppHandle,
    config: &Config,
    candidates: &[(Adapter, String)],
    current_adapter: Option<&str>,
    timeout: std::time::Duration,
) -> std::collections::HashMap<String, bool> {
    let state = app_handle.state::<AppState>();
    let quitting = state.exit.is_quitting.clone();
    let skip_ttfb = config.skip_ttfb_in_latency;
    let skip_content = config.skip_content_in_latency;
    let current_adapter = current_adapter.map(str::to_string);
    let candidates: Vec<Adapter> = candidates
        .iter()
        .filter(|(adapter, _)| !adapter.ip.is_empty())
        .map(|(adapter, _)| adapter.clone())
        .collect();
    // 探测目标随配置进 spawn（JoinSet 要求 'static，不能借 config 引用）
    let probe_hosts = config.internet_probe_hosts.clone();
    crate::infra::async_util::block_on_sync(async move {
        let mut tasks = tokio::task::JoinSet::new();
        for adapter in candidates {
            let quitting = quitting.clone();
            let probe_hosts = probe_hosts.clone();
            let allow_fallback = current_adapter.as_deref() == Some(adapter.name.as_str());
            tasks.spawn(async move {
                let ok = crate::network::quality::check_internet_access_with_if(
                    &adapter.ip,
                    Some(adapter.if_index),
                    skip_ttfb,
                    skip_content,
                    quitting,
                    allow_fallback,
                    timeout,
                    &probe_hosts,
                )
                .await;
                (adapter.ip, ok)
            });
        }
        let mut results = std::collections::HashMap::new();
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok((ip, ok)) => {
                    crate::log_info!("outbound", "公网探测结果: {ip} -> {ok}");
                    results.insert(ip, ok);
                }
                Err(e) => crate::log_warn!("outbound", "网卡公网探测任务失败: {e}"),
            }
        }
        results
    })
}

#[cfg(target_os = "windows")]
fn current_outbound_name() -> Option<String> {
    match crate::platform::best_route::current_outbound_v4() {
        Ok(current) => Some(current.alias),
        Err(e) => {
            crate::log_debug!("outbound", "读取当前出口失败，本轮不评估故障切换: {e}");
            None
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn current_outbound_name() -> Option<String> {
    None
}

fn spawn_recovery(app_handle: &AppHandle, config: &Config, adapter_name: &str) {
    {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        state.recovery_running = true;
        state.recovery_attempted = true;
        state.recovery_adapter = Some(adapter_name.to_string());
        state.recovery_result = None;
    }
    let app_h = app_handle.clone();
    let config = config.clone();
    let adapter_name = adapter_name.to_string();
    tauri::async_runtime::spawn(async move {
        let name = adapter_name.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            recover_current_adapter(&app_h, &config, &adapter_name)
        })
        .await
        .unwrap_or(RecoveryOutcome::Failed);
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        state.recovery_running = false;
        state.recovery_adapter = Some(name);
        state.recovery_result = Some(result);
    });
}

fn recover_current_adapter(app_handle: &AppHandle, config: &Config, adapter_name: &str) -> RecoveryOutcome {
    let state = app_handle.state::<AppState>();
    if state.network.load().last_auto_login_attempt.elapsed().as_secs() < config.auto_login_cooldown_secs {
        return RecoveryOutcome::Busy;
    }
    let adapters = match crate::network::get_adapters_cached() {
        Ok(adapters) => adapters,
        Err(e) => {
            crate::log_warn!("outbound", "登录恢复获取适配器失败: {e}");
            return RecoveryOutcome::Busy;
        }
    };
    let Some(adapter) = adapters.iter().find(|adapter| adapter.name == adapter_name && !adapter.ip.is_empty()) else {
        return RecoveryOutcome::NotNeeded;
    };
    let (adapter1, adapter2) = crate::network::resolve_adapter_names(&adapters, config);
    let managed = is_managed_adapter(config, adapter_name, &adapter1, &adapter2);
    let Some(_check_guard) = state.tasks.is_checking.try_acquire() else {
        return RecoveryOutcome::Busy;
    };
    let Some(_login_guard) = state.tasks.is_logging_in.try_acquire() else {
        return RecoveryOutcome::Busy;
    };
    // 获取适配器和等待锁期间其他登录可能已更新冷却时间，获锁后必须再次复核。
    if state.network.load().last_auto_login_attempt.elapsed().as_secs() < config.auto_login_cooldown_secs {
        return RecoveryOutcome::Busy;
    }
    let portal = match crate::auth::portal::check_portal_full(&adapter.ip, Some(&adapter.name)) {
        Ok(portal) => portal,
        Err(e) => {
            crate::log_warn!("outbound", "登录恢复前 Portal 检测失败: {e}");
            return RecoveryOutcome::Busy;
        }
    };
    if !should_attempt_login(managed, portal.reachable, portal.login_available, portal.online) {
        return RecoveryOutcome::NotNeeded;
    }
    // 两条路径都会进这里：故障路径的「当前出口」与切换态下轮转的「原卡」。
    // 文案必须中性——写成「当前出口 X」在切换态路径下会把正在恢复的原卡误报成当前出口。
    crate::log_info!("outbound", "登录恢复: {} 开始 Portal 复核", adapter_name);
    state.network.update(|network| network.last_auto_login_attempt = std::time::Instant::now());
    let result = crate::auth::service::full_login(&state, app_handle, Some(adapter_name));
    if !result.success {
        return RecoveryOutcome::Failed;
    }
    if super::auto_auth::verify_adapter_portal_online(adapter_name, state.exit.is_quitting.as_ref()) {
        crate::auth::service::mark_login_success(&state);
        RecoveryOutcome::Succeeded
    } else {
        RecoveryOutcome::Failed
    }
}

/// 指定网卡的 Portal 是否在线（切回前的认证前提）。取 IP 失败或探测出错一律按
/// 未在线处理——没有正面证据就不动出口。
fn adapter_portal_online(adapter_name: &str) -> bool {
    let Ok(adapters) = crate::network::get_adapters_cached() else {
        return false;
    };
    let Some(adapter) = adapters.iter().find(|a| a.name == adapter_name && !a.ip.is_empty()) else {
        return false;
    };
    match crate::auth::portal::check_portal_full(&adapter.ip, Some(&adapter.name)) {
        Ok(portal) => portal.online,
        Err(e) => {
            crate::log_debug!("outbound", "切回前 Portal 复核失败: {e}");
            false
        }
    }
}

fn persist_failover_hold(app_handle: &AppHandle, config: &Config) {
    let now = chrono::Local::now();
    let hold_day = failover_hold_day(
        now.date_naive().num_days_from_ce(),
        now.hour() * 60 + now.minute(),
        crate::config::night_switch::restore_start_minutes(config.night_restore_minutes),
    );
    let state = app_handle.state::<AppState>();
    if let Err(e) = crate::config::persist::mutate_config_and_save(app_handle, &state, move |fresh| {
        fresh.outbound_manual_hold_day = hold_day;
        Ok(())
    }) {
        // 快照已存在时不能在下一拍立即还原；落盘失败仅在内存保持并明确告警。
        let mut fresh = (*state.config.load()).clone();
        fresh.outbound_manual_hold_day = hold_day;
        state.config.store(fresh);
        crate::log_warn!("outbound", "故障切换保持标记落盘失败: {e}；已在当前进程内保持");
    }
}

/// 切换态下的切回判定：原校园网出口连续 [`REQUIRED_FAILURE_ROUNDS`] 轮公网可达时，
/// 写回跃点快照把出口还原。滞回两轮，避免原卡闪断时在两块卡之间来回切。
fn evaluate_switch_back(app_handle: &AppHandle, config: &Config) {
    // 夜间守护窗口（当日切换时刻起至次日还原时刻）内不切回：到点切换后原卡可能
    // 仍有短暂连通（学校断网时刻不精确）或探测偶发误判，两轮"健康"就切回会让
    // 夜切变成「切了又切回」的抖动；切回统一交给还原时刻的还原动作。
    let now = chrono::Local::now();
    if crate::config::outbound_switch::is_night_outbound_guard_window_with_restore(
        config.enable_night_outbound_switch,
        now.weekday().num_days_from_sunday(),
        crate::infra::local_now_minutes() as u32,
        config.night_restore_minutes,
    ) {
        return;
    }
    let Some(current_name) = current_outbound_name() else {
        reset_state();
        return;
    };
    let adapters = match crate::network::get_adapters_cached() {
        Ok(adapters) => adapters,
        Err(e) => {
            crate::log_debug!("outbound", "切回判定获取适配器失败: {e}");
            reset_state();
            return;
        }
    };
    let (adapter1, adapter2) = crate::network::resolve_adapter_names(&adapters, config);
    let managed: Vec<(Adapter, String)> = adapters
        .iter()
        .filter(|adapter| !adapter.ip.is_empty() && adapter.name != current_name)
        .filter(|adapter| is_managed_adapter(config, &adapter.name, &adapter1, &adapter2))
        .map(|adapter| (adapter.clone(), String::new()))
        .collect();
    if managed.is_empty() {
        reset_state();
        return;
    }
    let health = probe_internet_access(
        app_handle,
        config,
        &managed,
        Some(&current_name),
        std::time::Duration::from_secs(3),
    );
    // 切换后仍要尝试登录原卡：Portal 旁路路由已把去 Portal 的流量送回原卡，
    // 这里按受管卡逐张发起一次恢复（内部会先做 Portal 检测，未在线才真正认证）。
    // 不阻塞本拍：spawn_recovery 异步执行，结果由下一拍的 STATE 读取。
    // 登录原卡依赖旁路路由：首次添加失败时快照为空，这里补一次。缺路由就到不了
    // Portal，登录前置判定会把恢复降级成 NotNeeded，于是整夜不再尝试登录原卡
    if !super::outbound_night::ensure_portal_bypass_route(app_handle, config) {
        crate::log_warn!("outbound", "切换态旁路路由未就绪，原卡登录可能失败");
    }
    let recovery_busy = STATE.lock().unwrap_or_else(|e| e.into_inner()).recovery_running;
    if !recovery_busy {
        // 轮转选卡：固定取首张会让其余受管卡永远轮不到，而多卡时首张未必是刚被
        // 切走的那张（真正需要恢复登录的是它）
        let pick = {
            let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
            let pick = state.recovery_cursor % managed.len();
            state.recovery_cursor = state.recovery_cursor.wrapping_add(1);
            pick
        };
        let (adapter, _) = &managed[pick];
        crate::log_debug!("outbound", "切换态下尝试登录原卡 {}", adapter.name);
        spawn_recovery(app_handle, config, &adapter.name);
    }
    let recovered = managed
        .iter()
        .find(|(adapter, _)| health.get(&adapter.ip).copied().unwrap_or(false))
        .map(|(adapter, _)| adapter.name.clone());
    let (recovered_name, rounds) = {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        match recovered {
            Some(name) if state.switch_back_adapter.as_deref() == Some(name.as_str()) => {
                state.switch_back_rounds = state.switch_back_rounds.saturating_add(1);
                (name, state.switch_back_rounds)
            }
            Some(name) => {
                state.switch_back_adapter = Some(name.clone());
                state.switch_back_rounds = 1;
                (name, 1)
            }
            None => {
                state.switch_back_adapter = None;
                state.switch_back_rounds = 0;
                return;
            }
        }
    };
    if rounds < SWITCH_BACK_HEALTHY_ROUNDS {
        crate::log_debug!(
            "outbound",
            "原出口 {} 恢复中（{}/{} 轮）",
            recovered_name,
            rounds,
            SWITCH_BACK_HEALTHY_ROUNDS
        );
        return;
    }
    // 公网可达不等于已认证：登录恢复失败时链路照样通，此时切回会让用户断网。
    // Portal 在线才是切回的前提。
    if !adapter_portal_online(&recovered_name) {
        crate::log_info!(
            "outbound",
            "原出口 {} 连续 {} 轮公网可达但 Portal 未在线，暂不切回",
            recovered_name,
            rounds
        );
        return;
    }
    crate::log_info!(
        "outbound",
        "原出口 {} 连续 {} 轮公网可达，写回跃点切回原出口",
        recovered_name,
        rounds
    );
    match super::outbound_night::apply_outbound_restore(app_handle, config) {
        Ok(()) => {
            reset_state();
            crate::log_info!("outbound", "已切回原出口 {}", recovered_name);
        }
        Err(e) => crate::log_warn!("outbound", "切回原出口失败: {e}"),
    }
}

/// scheduled 30s 编排的统一入口：夜间切换时刻只是本函数的一次额外触发。
/// 登录恢复跨拍完成，scheduled 不会因认证请求阻塞数十秒。
pub(super) fn evaluate_outbound_failover(app_handle: &AppHandle, config: &Config) {
    let today = chrono::Local::now().date_naive().num_days_from_ce();
    if !config.enable_night_outbound_switch || config.outbound_manual_hold_day == today {
        reset_state();
        return;
    }
    // 已在切换态：只判定"原校园网出口是否已恢复、该切回"，不再触发新的故障切换。
    if super::outbound_night::outbound_switch_active(app_handle) {
        evaluate_switch_back(app_handle, config);
        return;
    }
    if STATE.lock().unwrap_or_else(|e| e.into_inner()).recovery_running {
        return;
    }

    let adapters = match crate::network::get_adapters_cached() {
        Ok(adapters) => adapters,
        Err(e) => {
            crate::log_warn!("outbound", "出口健康评估获取适配器失败: {e}");
            reset_state();
            return;
        }
    };
    let details = match crate::network::get_adapter_details_cached() {
        Ok(details) => details,
        Err(e) => {
            crate::log_warn!("outbound", "出口健康评估获取网关详情失败: {e}");
            reset_state();
            return;
        }
    };
    let candidates: Vec<(Adapter, String)> = adapters
        .iter()
        .map(|adapter| {
            let gateway = details
                .iter()
                .find(|detail| detail.name == adapter.name)
                .map(|detail| detail.gateway.clone())
                .unwrap_or_default();
            (adapter.clone(), gateway)
        })
        .collect();
    let Some(current_name) = current_outbound_name() else {
        reset_state();
        return;
    };
    if !candidates.iter().any(|(adapter, _)| adapter.name == current_name) {
        reset_state();
        return;
    }
    let (adapter1, adapter2) = crate::network::resolve_adapter_names(&adapters, config);
    let priority = super::outbound_switch::effective_outbound_priority(&config.outbound_priority, &candidates);
    let probe_candidates: Vec<(Adapter, String)> = candidates
        .iter()
        .filter(|(adapter, _)| {
            adapter.name == current_name
                || adapter.name == adapter1
                || (config.dual_adapter && adapter.name == adapter2)
                || priority.iter().any(|name| name == &adapter.name)
        })
        .cloned()
        .collect();
    let health = probe_internet_access(
        app_handle,
        config,
        &probe_candidates,
        Some(&current_name),
        std::time::Duration::from_secs(3),
    );
    if app_handle.state::<AppState>().exit.is_quitting.load(std::sync::atomic::Ordering::Acquire) {
        return;
    }
    let Some(current_healthy) = probe_candidates
        .iter()
        .find(|(adapter, _)| adapter.name == current_name)
        .and_then(|(adapter, _)| health.get(&adapter.ip).copied())
    else {
        crate::log_warn!("outbound", "当前出口 {} 公网探测未完成，本轮不计为断网", current_name);
        reset_state();
        return;
    };

    let (failures, previous_failures) = {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let previous_failures = state.failures;
        let adapter_changed = state.adapter.as_deref() != Some(current_name.as_str());
        let failures = next_failure_round(&state, &current_name, current_healthy);
        state.adapter = Some(current_name.clone());
        state.failures = failures;
        if current_healthy {
            *state = FailoverState::default();
        } else if adapter_changed {
            state.recovery_result = None;
            state.recovery_attempted = false;
        }
        (failures, previous_failures)
    };
    if current_healthy {
        if previous_failures > 0 {
            crate::log_info!("outbound", "当前出口 {} 公网连通性已恢复", current_name);
        }
        return;
    }
    if failures != previous_failures {
        crate::log_warn!("outbound", "当前出口 {} 公网不可达（连续 {}/{} 轮）", current_name, failures, REQUIRED_FAILURE_ROUNDS);
    }
    if failures < REQUIRED_FAILURE_ROUNDS {
        return;
    }

    let (recovery_result, recovery_attempted) = {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let result = if state.recovery_adapter.as_deref() == Some(current_name.as_str()) {
            state.recovery_result.take()
        } else {
            None
        };
        (result, state.recovery_attempted)
    };
    let managed = is_managed_adapter(config, &current_name, &adapter1, &adapter2);
    match recovery_action(recovery_result, managed, recovery_attempted) {
        RecoveryAction::Wait => {
            STATE.lock().unwrap_or_else(|e| e.into_inner()).recovery_attempted = false;
            return;
        }
        RecoveryAction::Recovered => {
            crate::log_info!("outbound", "当前出口 {} 登录与 Portal 复核已恢复，本轮不切换", current_name);
            reset_state();
            return;
        }
        RecoveryAction::AttemptLogin => {
            spawn_recovery(app_handle, config, &current_name);
            return;
        }
        RecoveryAction::Switch => {}
    }

    let switch_result = super::outbound_night::apply_outbound_switch_with_health(app_handle, config, Some(health), None);
    if super::outbound_night::outbound_switch_active(app_handle) {
        persist_failover_hold(app_handle, config);
    }
    match switch_result {
        Ok(()) => {
            crate::log_info!("outbound", "当前出口连续两轮公网不可达，已完成故障自动出站切换");
            let _ = crate::infra::events::EventBus::new(app_handle)
                .emit_login_log("当前出口连续两轮公网不可达，已完成故障自动出站切换", "success");
            reset_state();
        }
        Err(e) => crate::log_warn!("outbound", "故障自动出站切换未完成: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 当前出口健康或换卡会复位失败轮次() {
        let state = FailoverState { adapter: Some("以太网".into()), ..Default::default() };
        assert_eq!(next_failure_round(&state, "以太网", true), 0);
        assert_eq!(next_failure_round(&state, "WLAN", false), 1);
    }

    #[test]
    fn 连续两轮失败才进入恢复与切换() {
        let mut state = FailoverState::default();
        assert_eq!(next_failure_round(&state, "以太网", false), 1);
        state.adapter = Some("以太网".into());
        state.failures = 1;
        assert_eq!(next_failure_round(&state, "以太网", false), REQUIRED_FAILURE_ROUNDS);
        state.failures = REQUIRED_FAILURE_ROUNDS;
        assert_eq!(next_failure_round(&state, "以太网", false), REQUIRED_FAILURE_ROUNDS);
    }

    #[test]
    fn 登录恢复成功直接结束_busy可重试_失败才切换() {
        assert_eq!(recovery_action(Some(RecoveryOutcome::Succeeded), true, true), RecoveryAction::Recovered);
        assert_eq!(recovery_action(Some(RecoveryOutcome::Busy), true, true), RecoveryAction::Wait);
        assert_eq!(recovery_action(None, true, false), RecoveryAction::AttemptLogin);
        assert_eq!(recovery_action(Some(RecoveryOutcome::Failed), true, true), RecoveryAction::Switch);
        assert_eq!(recovery_action(None, true, true), RecoveryAction::Switch);
        assert_eq!(recovery_action(None, false, false), RecoveryAction::Switch);
    }

    #[test]
    fn 连续两轮或恢复进行中才抑制运营商切侧() {
        assert!(!failover_pending(0, false));
        assert!(!failover_pending(1, false));
        assert!(failover_pending(REQUIRED_FAILURE_ROUNDS, false));
        assert!(failover_pending(0, true));
    }

    #[test]
    fn 凌晨切换不挡住当天还原_白天和夜间保持当日() {
        const TODAY: i32 = 739_776;
        assert_eq!(failover_hold_day(TODAY, 120, 450), TODAY - 1);
        assert_eq!(failover_hold_day(TODAY, 449, 450), TODAY - 1);
        assert_eq!(failover_hold_day(TODAY, 450, 450), TODAY);
        assert_eq!(failover_hold_day(TODAY, 720, 450), TODAY);
        assert_eq!(failover_hold_day(TODAY, 1380, 450), TODAY);
    }

    #[test]
    fn 只有受管且门户可登录才尝试恢复() {
        assert!(should_attempt_login(true, true, true, false));
        assert!(!should_attempt_login(true, false, false, false), "夜间Portal不可达不等于登录失败");
        assert!(!should_attempt_login(true, true, false, true), "已在线但公网断不再重复登录");
        assert!(!should_attempt_login(false, true, true, false));
    }
}
