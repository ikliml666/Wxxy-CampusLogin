//! 每日定时登录 / 定时注销（P2-32）。
//!
//! 独立轻量循环（30s 一拍，分钟粒度判定），判定复用跨平台纯函数
//! `config::schedule::should_fire_scheduled_action`（桌面与安卓同语义），
//! 动作复用既有入口 `auth::service::full_login` / `full_logout`——不复制任何协议逻辑。
//!
//! 与既有机制的隔离：
//! - 与后台检测解耦：随 run_startup_tasks 无条件启动，`enable_background_check`
//!   关闭时定时动作仍生效（两者是独立功能）；
//! - 互斥：登录走 `tasks.is_logging_in`、注销走 `tasks.is_logging_out`（与手动登录/
//!   托盘快速登录/掉线重连同一把锁，抢不到即本拍跳过，下一拍「过点补触发」兜底）；
//! - 标记独立：当日触发标记存 `state.scheduled`（ScheduledFired），不与 auto_auth 的
//!   冷却时间/重连计数/has_logged_online 共享，互不干扰；
//! - 单次语义：到点即置当日标记（无论动作成败），当日不重试——避免凭据错误或
//!   非校园网环境下每拍重发请求刷日志/通知；失败由登录日志暴露。

use std::sync::atomic::Ordering;
use std::time::Duration;
use chrono::{Datelike, Timelike};
use tauri::{AppHandle, Manager};
use crate::config::night_switch::{evaluate_night_switch, parse_chkstatus, uid_matches, NightSwitchAction};
use crate::config::schedule::should_fire_scheduled_action;
use crate::infra::events::EventBus;
use crate::infra::state::AppState;

/// 判定拍间隔：分钟粒度判定，30s 保证最多半分钟偏差；错过（休眠/拍间隔）由
/// 「过点补触发」语义兜底，不会整天静默失效
const SCHEDULED_TICK_MS: u64 = 30_000;

/// 夜切验证时序参数（与安卓端同构）：登录编排完成后等 15s 再查（eportal 在线表
/// 非即时，立即查会读到切换前状态）；查询失败重试间隔 5s、最多 3 次，单次请求 15s 超时
const NIGHT_VERIFY_WAIT_SECS: u64 = 15;
const NIGHT_VERIFY_RETRY_SECS: u64 = 5;
const NIGHT_VERIFY_ATTEMPTS: u32 = 3;
const NIGHT_VERIFY_HTTP_TIMEOUT_SECS: u64 = 15;

/// 循环体：由 watcher::run_startup_tasks 经 task_manager.spawn 拉起并跟踪
pub async fn run_scheduled_action_loop(app_handle: &AppHandle, cancel_token: std::sync::Arc<tokio_util::sync::CancellationToken>) {
    let app_h = app_handle.clone();
    let mut tick = tokio::time::interval(Duration::from_millis(SCHEDULED_TICK_MS));
    loop {
        tokio::select! {
            _ = tick.tick() => {}
            _ = cancel_token.cancelled() => {
                crate::log_debug!("scheduled", "定时动作循环收到取消信号，退出");
                break;
            }
        }
        let s = app_h.state::<AppState>();
        if s.exit.is_quitting.load(Ordering::Acquire) {
            break;
        }
        let now = chrono::Local::now();
        let now_minutes = now.hour() as u16 * 60 + now.minute() as u16;
        let today_day = now.date_naive().num_days_from_ce();
        let (fire_login, fire_logout) = evaluate_and_mark(&s, now_minutes, today_day);
        if fire_login {
            run_scheduled_login(&app_h, now_minutes);
        }
        if fire_logout {
            run_scheduled_logout(&app_h, now_minutes);
        }
        // 晚间断网切换：判定独立于当日标记（切换态由配置自身承载，纯函数幂等），
        // 与定时动作同循环、同"独立于静默期闸门"的取位（见
        // .codewiki/decisions/scheduled-actions-outside-silent-window）
        let night_action = {
            let config = s.config.load_full();
            evaluate_night_switch(
                config.enable_night_operator_switch,
                now.weekday().num_days_from_sunday(),
                now_minutes as u32,
                &config.operator,
                &config.night_operator_restore,
            )
        };
        if night_action != NightSwitchAction::None {
            apply_night_switch_action(&app_h, night_action);
        }
    }
}

/// 单拍判定并置当日标记：命中即标记（无论后续动作成败），当日不重试；
/// 跨天后 today_day 变化，旧标记不再相等，视为未执行（纯函数语义）。
/// 返回 (应登录, 应注销)。
fn evaluate_and_mark(state: &AppState, now_minutes: u16, today_day: i32) -> (bool, bool) {
    let config = state.config.load_full();
    let fire_login = should_fire_scheduled_action(
        now_minutes,
        config.scheduled_login_minutes,
        state.scheduled.login_day.load(Ordering::Acquire),
        today_day,
    );
    if fire_login {
        state.scheduled.login_day.store(today_day, Ordering::Release);
    }
    let fire_logout = should_fire_scheduled_action(
        now_minutes,
        config.scheduled_logout_minutes,
        state.scheduled.logout_day.load(Ordering::Acquire),
        today_day,
    );
    if fire_logout {
        state.scheduled.logout_day.store(today_day, Ordering::Release);
    }
    (fire_login, fire_logout)
}

fn format_minutes(minutes: u16) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

/// 定时登录：复用 full_login（与手动/托盘登录同一互斥锁），成功后走
/// post_login_handler（托盘快速登录同款编排，含解除保护期/延迟检测/自动退出）
fn run_scheduled_login(app_handle: &AppHandle, target_minutes: u16) {
    spawn_login_action(
        app_handle,
        &format!("定时登录触发 (目标 {})", format_minutes(target_minutes)),
        "定时登录",
    );
}

/// 登录动作共用执行体（定时登录与晚间断网切换登录同款）：抢 `is_logging_in`
/// 互斥锁，抢不到本拍跳过；日志/通知复用 emit_login_log（"success"/"error"）
fn spawn_login_action(app_handle: &AppHandle, trigger_log: &str, result_prefix: &str) {
    crate::log_info!("scheduled", "{trigger_log}");
    let app_h = app_handle.clone();
    let result_prefix = result_prefix.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let s = app_h.state::<AppState>();
        let _guard = match s.tasks.is_logging_in.try_acquire() {
            Some(g) => g,
            // 已有登录任务在跑：不重复发起
            None => {
                crate::log_warn!("scheduled", "登录动作跳过：已有登录任务进行中");
                return;
            }
        };
        let result = crate::auth::service::full_login(&s, &app_h, None);
        let message = result.message.clone().unwrap_or_default();
        let _ = EventBus::new(&app_h).emit_login_log(
            &format!("{result_prefix}: {message}"),
            if result.success { "success" } else { "error" },
        );
        if result.success {
            crate::auth::service::post_login_handler(&app_h, &s);
        }
    });
}

/// 晚间断网切换动作：改内存配置 → 落盘（复用 save_config_to_disk_encrypted，
/// 含 config-changed 事件与托盘刷新）→ 触发登录（复用定时登录的登录路径）。
/// 切换态由配置自身承载，配置改完下一拍判定即回 None，无需去重标记。
fn apply_night_switch_action(app_handle: &AppHandle, action: NightSwitchAction) {
    let s = app_handle.state::<AppState>();
    let mut config = (*s.config.load_full()).clone();
    let trigger_log = match action {
        NightSwitchAction::None => return,
        NightSwitchAction::SwitchToCampus => {
            let original = std::mem::take(&mut config.operator);
            config.night_operator_restore = original;
            "晚间断网切换: 运营商已切至无锡学院".to_string()
        }
        NightSwitchAction::Restore => {
            config.operator = std::mem::take(&mut config.night_operator_restore);
            "晚间断网切换: 已恢复原运营商".to_string()
        }
    };
    if let Err(e) = crate::commands::config_cmd::save_config_to_disk_encrypted(app_handle, &config) {
        // 落盘失败不 store 内存（避免"内存已切换、重启后回退"的错位），下拍重试
        crate::log_warn!("scheduled", "晚间断网切换配置落盘失败: {e}");
        return;
    }
    s.config.store(config);
    crate::log_info!("scheduled", "{trigger_log}");
    spawn_login_action(app_handle, &format!("{trigger_log}, 触发登录"), "晚间断网切换");
    // 独立 tokio 任务验证切换是否生效，不阻塞监控循环；期望值取落盘后的 config.operator
    // （SwitchToCampus 后为空串=无锡学院，Restore 后为恢复的后缀）
    let expected_operator = app_handle.state::<AppState>().config.load_full().operator.clone();
    tauri::async_runtime::spawn(verify_night_switch(app_handle.clone(), expected_operator));
}

/// 定时注销：复用 full_logout（与手动注销同一互斥锁）。成功后对齐全量注销后处理
/// （commands::login_cmd::do_logout 的 adapter_name=None 分支）：取消登录后自动退出、
/// 重置登录/重连状态并启动 60s 注销保护期——防止定时注销后被准备自动登录/
/// 掉线重连立即登回（安卓端 do_logout 内置同款保护期语义）。
fn run_scheduled_logout(app_handle: &AppHandle, target_minutes: u16) {
    crate::log_info!("scheduled", "定时注销触发 (目标 {})", format_minutes(target_minutes));
    let app_h = app_handle.clone();
    tauri::async_runtime::spawn_blocking(move || perform_logout(&app_h, "定时注销"));
}

/// 注销执行体（定时注销与夜切验证复登前的注销同款）：抢 `is_logging_out` 互斥锁，
/// 抢不到本拍跳过；成功后取消登录后自动退出、重置登录/重连状态并启动 60s 注销保护期。
fn perform_logout(app_handle: &AppHandle, result_prefix: &str) {
    let s = app_handle.state::<AppState>();
    let result = {
        let _guard = match s.tasks.is_logging_out.try_acquire() {
            Some(g) => g,
            None => {
                crate::log_warn!("scheduled", "{result_prefix}跳过：已有注销任务进行中");
                return;
            }
        };
        crate::auth::service::full_logout(&s, app_handle, None)
    };
    let message = result.message.clone().unwrap_or_default();
    let _ = EventBus::new(app_handle).emit_login_log(
        &format!("{result_prefix}: {message}"),
        if result.success { "success" } else { "error" },
    );
    if result.success {
        s.exit.auto_exit_cancelled.store(true, Ordering::Release);
        s.exit.set_deadline(None);
        crate::auth::failure_tracker::reset_all(&s);
        let protected_until = std::time::Instant::now() + Duration::from_secs(60);
        s.network.update(|n| {
            n.has_logged_online = false;
            n.disconnect_reconnect_count = 0;
            n.last_auto_login_attempt = std::time::Instant::now();
            n.logout_protected_until = protected_until;
        });
    }
}

/// 夜切后的登录生效自动验证（与安卓端 verify_night_switch 同构，独立 tokio 任务
/// 不阻塞监控循环）：等 15s → 查 eportal chkstatus(JSONP) 核对在线账号 uid 与期望
/// 运营商后缀（重试 3 次、间隔 5s）→ 仍不符则注销重登一轮再复验 → 最终不符发告警
/// 日志 + 系统通知，生效写 success 日志。uid 核对复用共享纯函数 config::night_switch。
async fn verify_night_switch(app_handle: AppHandle, expected_operator: String) {
    tokio::time::sleep(Duration::from_secs(NIGHT_VERIFY_WAIT_SECS)).await;
    let Some(config) = night_verify_config(&app_handle, &expected_operator) else {
        return;
    };
    if let Ok(uid) = night_switch_verify_round(&config, &expected_operator).await {
        let _ = EventBus::new(&app_handle)
            .emit_login_log(&format!("夜切验证: 已生效, 在线账号 {uid}"), "success");
        return;
    }
    // 未生效：注销 → 再登录 → 复验一轮。复登前复查 operator，用户已手动改走则止步，
    // 避免把用户刚改的配置又注销掉
    let _ = EventBus::new(&app_handle)
        .emit_login_log("夜切验证: 未生效, 注销重登后复验", "warning");
    let Some(config) = night_verify_config(&app_handle, &expected_operator) else {
        return;
    };
    let app_h = app_handle.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || perform_logout(&app_h, "夜切验证注销")).await;
    spawn_login_action(&app_handle, "夜切验证复登", "夜切验证");
    tokio::time::sleep(Duration::from_secs(NIGHT_VERIFY_WAIT_SECS)).await;
    match night_switch_verify_round(&config, &expected_operator).await {
        Ok(uid) => {
            let _ = EventBus::new(&app_handle)
                .emit_login_log(&format!("夜切验证: 已生效, 在线账号 {uid}"), "success");
        }
        Err(e) => {
            let _ = EventBus::new(&app_handle)
                .emit_login_log(&format!("夜切验证: {e}, 请手动检查"), "warning");
            crate::infra::notification::emit_notification(
                &app_handle,
                "夜切验证失败",
                "夜切后在线账号与期望运营商不符, 请手动检查",
                "mascot-alert",
            );
        }
    }
}

/// 读取验证所需的配置快照；operator 已不等于期望值（用户手动改走其他运营商）时
/// 返回 None 跳过验证——验证前提不成立，继续核对只会误报。
fn night_verify_config(app_handle: &AppHandle, expected_operator: &str) -> Option<crate::config::model::Config> {
    let s = app_handle.state::<AppState>();
    let config = (*s.config.load_full()).clone();
    (config.operator == expected_operator).then_some(config)
}

/// 一轮验证（NIGHT_VERIFY_ATTEMPTS 次重试、间隔 5s）：Ok(uid)=已生效，
/// Err=最后一轮失败原因。HTTP/解析失败不 panic，收敛为 Err。
async fn night_switch_verify_round(config: &crate::config::model::Config, expected_operator: &str) -> Result<String, String> {
    // 复用桌面既有 reqwest 客户端池构造（超时 15s 作池 key，与 protocol.rs 同款）
    let client = crate::network::client::create_safe_http_client(
        Duration::from_secs(NIGHT_VERIFY_HTTP_TIMEOUT_SECS),
        None,
    )
    .map_err(|e| format!("chkstatus 客户端创建失败: {e}"))?;
    let mut last_err = String::new();
    for attempt in 0..NIGHT_VERIFY_ATTEMPTS {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_secs(NIGHT_VERIFY_RETRY_SECS)).await;
        }
        match night_switch_check_once(&client, &config.portal_url, &config.user, expected_operator).await {
            Ok(uid) => return Ok(uid),
            Err(e) => last_err = e,
        }
    }
    Err(last_err)
}

/// 单次 chkstatus 查询 + uid 核对：Ok=在线账号(uid)，Err=失败原因（不符时含观测 uid）
async fn night_switch_check_once(
    client: &reqwest::Client,
    portal_url: &str,
    user: &str,
    expected_operator: &str,
) -> Result<String, String> {
    let url = format!("{}/drcom/chkstatus?callback=dr1003", portal_origin(portal_url));
    let body = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("chkstatus 请求失败: {e}"))?
        .text()
        .await
        .map_err(|e| format!("chkstatus 响应读取失败: {e}"))?;
    match parse_chkstatus(&body) {
        Some(info) if info.online && uid_matches(&info.uid, user, expected_operator) => Ok(info.uid),
        Some(info) => Err(format!(
            "在线账号 {} 与期望 {} 不符",
            info.uid,
            expected_operator_display(expected_operator)
        )),
        None => Err("chkstatus 响应解析失败".to_string()),
    }
}

/// portal_url 的 origin（chkstatus 与 portal 同源，eportal 按请求源 IP 判定本机）；
/// 空值回退默认 portal 地址（config::model 的既有默认值）
fn portal_origin(portal_url: &str) -> String {
    let url = if portal_url.is_empty() {
        crate::config::model::default_portal_url()
    } else {
        portal_url.to_string()
    };
    match url.split_once("://") {
        Some((scheme, rest)) => format!("{scheme}://{}", rest.split('/').next().unwrap_or("")),
        None => url.split('/').next().unwrap_or("").to_string(),
    }
}

/// 期望运营商的展示名：空串=无锡学院，其余原样（@telecom 等）
fn expected_operator_display(expected_operator: &str) -> &str {
    if expected_operator.is_empty() {
        "无锡学院"
    } else {
        expected_operator
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering as AOrd;

    const TODAY: i32 = 739_776;

    #[test]
    fn 禁用目标_不触发_不置标记() {
        let state = AppState::new();
        let (login, logout) = evaluate_and_mark(&state, 600, TODAY);
        assert!(!login);
        assert!(!logout);
        assert_eq!(state.scheduled.login_day.load(AOrd::Acquire), i32::MIN);
        assert_eq!(state.scheduled.logout_day.load(AOrd::Acquire), i32::MIN);
    }

    #[test]
    fn 到点触发_置当日标记() {
        let state = AppState::new();
        state.config.update(|c| {
            c.scheduled_login_minutes = 7 * 60 + 40;
            c.scheduled_logout_minutes = 23 * 60 + 30;
        });
        // 未到点
        assert!(!evaluate_and_mark(&state, 460 - 1, TODAY).0);
        // 过点补触发（循环拍错过精确分钟）
        assert!(evaluate_and_mark(&state, 505, TODAY).0);
        assert_eq!(state.scheduled.login_day.load(AOrd::Acquire), TODAY);
        // 同日不重复
        assert!(!evaluate_and_mark(&state, 600, TODAY).0);
        // 注销同款
        assert!(!evaluate_and_mark(&state, 23 * 60 + 29, TODAY).1);
        assert!(evaluate_and_mark(&state, 23 * 60 + 30, TODAY).1);
        assert_eq!(state.scheduled.logout_day.load(AOrd::Acquire), TODAY);
    }

    #[test]
    fn 跨天重置_次日再触发() {
        let state = AppState::new();
        state.config.update(|c| {
            c.scheduled_login_minutes = 7 * 60 + 40;
        });
        state.scheduled.login_day.store(TODAY - 1, AOrd::Release);
        assert!(evaluate_and_mark(&state, 460, TODAY).0);
    }

    #[test]
    fn format_minutes_补零() {
        assert_eq!(format_minutes(460), "07:40");
        assert_eq!(format_minutes(1439), "23:59");
    }
}
