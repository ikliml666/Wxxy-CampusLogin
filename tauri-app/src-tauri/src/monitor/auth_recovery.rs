//! 校园网认证异常（网关可达、Portal 不可达）时的自助恢复。
//!
//! 为什么需要：网关可达说明流量确实进了校园网，但 Portal 不通意味着认证入口或
//! 会话已经失效（被踢下线、绑定记录过期、地址池冲突）。此时重复登录只有失败，
//! 换一个 DHCP 身份（随机 MAC）重新取址才能恢复；网卡重启与租约等待都在
//! network::dhcp 里，这里只负责节流、单飞与恢复后的重新认证。

use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, Manager};

use crate::config::model::Config;
use crate::infra::state::AppState;

/// 上次执行的时间点。进程内状态：重启本身就是新的会话环境，写进配置反而污染用户配置。
static LAST_RUN: Mutex<Option<Instant>> = Mutex::new(None);
/// 当日已执行次数与其所属日期（num_days_from_ce）。
static DAILY_COUNT: AtomicU32 = AtomicU32::new(0);
static DAILY_DAY: AtomicI32 = AtomicI32::new(0);

/// 冷却与每日上限判定（纯函数，便于单测）。
fn renew_allowed(
    enabled: bool,
    cooldown_secs: u64,
    daily_limit: u32,
    elapsed_since_last: Option<u64>,
    today: i32,
    count_day: i32,
    count: u32,
) -> bool {
    if !enabled || daily_limit == 0 {
        return false;
    }
    if let Some(elapsed) = elapsed_since_last {
        if elapsed < cooldown_secs {
            return false;
        }
    }
    let used_today = if count_day == today { count } else { 0 };
    used_today < daily_limit
}

/// 占一个执行配额：通过则记录时间戳与当日计数并返回 true。
/// 先占位再执行——失败也不退还，否则坏链路会让每一拍都重启一次网卡。
fn reserve_slot(config: &Config) -> bool {
    use chrono::Datelike;
    let today = chrono::Local::now().date_naive().num_days_from_ce();
    let elapsed = LAST_RUN
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .map(|t| t.elapsed().as_secs());
    let allowed = renew_allowed(
        config.enable_mac_renew_on_auth_error,
        config.mac_renew_cooldown_secs,
        config.mac_renew_daily_limit,
        elapsed,
        today,
        DAILY_DAY.load(Ordering::Acquire),
        DAILY_COUNT.load(Ordering::Acquire),
    );
    if !allowed {
        return false;
    }
    *LAST_RUN.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
    if DAILY_DAY.load(Ordering::Acquire) != today {
        DAILY_DAY.store(today, Ordering::Release);
        DAILY_COUNT.store(0, Ordering::Release);
    }
    DAILY_COUNT.fetch_add(1, Ordering::AcqRel);
    true
}

/// 认证异常时的恢复入口：换 MAC 取新 IP，然后重新走一次完整登录。
/// 单飞：与巡检/登录共用同一组锁——换 MAC 会重启网卡，绝不能在别的探测或登录
/// 进行时动手。冷却期内的调用直接被丢弃。
pub(super) fn spawn_auth_recovery(app_handle: &AppHandle, config: &Config, adapter_name: &str) {
    if !reserve_slot(config) {
        crate::log_info!(
            "auth_recovery",
            "认证异常恢复被节流或关闭，本拍跳过（{}）",
            adapter_name
        );
        return;
    }
    crate::log_info!(
        "auth_recovery",
        "校园网认证异常（Portal 不可达、网关可达）：对 {} 换 MAC 重新获取 IP",
        adapter_name
    );
    let app_h = app_handle.clone();
    let config = config.clone();
    let name = adapter_name.to_string();
    tauri::async_runtime::spawn(async move {
        let _ = tauri::async_runtime::spawn_blocking(move || run_recovery(&app_h, &config, &name)).await;
    });
}

#[cfg(target_os = "windows")]
fn run_recovery(app_handle: &AppHandle, _config: &Config, adapter_name: &str) -> bool {
    let state = app_handle.state::<AppState>();
    let Some(_check) = state.tasks.is_checking.try_acquire() else {
        crate::log_debug!("auth_recovery", "巡检进行中，本拍跳过认证恢复");
        return false;
    };
    let Some(_login) = state.tasks.is_logging_in.try_acquire() else {
        crate::log_debug!("auth_recovery", "登录进行中，本拍跳过认证恢复");
        return false;
    };
    match crate::network::dhcp::dhcp_release_renew_single(adapter_name) {
        Ok(value) => {
            let success = value.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
            let skipped = value.get("skipped").and_then(|v| v.as_bool()).unwrap_or(false);
            let reason = value.get("reason").and_then(|v| v.as_str()).unwrap_or("");
            if skipped {
                crate::log_info!("auth_recovery", "{} 跳过换 MAC：{}", adapter_name, reason);
                return false;
            }
            if !success {
                crate::log_warn!("auth_recovery", "{} 换 MAC 取新 IP 未成功：{}", adapter_name, reason);
                return false;
            }
            let ip = value.get("ip").and_then(|v| v.as_str()).unwrap_or("");
            crate::log_info!("auth_recovery", "{} 已换 MAC 取到新 IP {}", adapter_name, ip);
        }
        Err(e) => {
            crate::log_warn!("auth_recovery", "{} 换 MAC 失败: {e}", adapter_name);
            return false;
        }
    }
    // 换址后立刻重新认证：Portal 此时通常已恢复响应
    let result = crate::auth::service::full_login(&state, app_handle, Some(adapter_name));
    if result.success {
        crate::auth::service::mark_login_success(&state);
        crate::log_info!("auth_recovery", "{} 认证恢复成功", adapter_name);
        true
    } else {
        crate::log_warn!("auth_recovery", "{} 换址后重新认证仍失败", adapter_name);
        false
    }
}

#[cfg(not(target_os = "windows"))]
fn run_recovery(_app_handle: &AppHandle, _config: &Config, _adapter_name: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::renew_allowed;

    #[test]
    fn renew_allowed_respects_switch_cooldown_and_daily_limit() {
        // 关闭开关或上限为 0 时永不允许
        assert!(!renew_allowed(false, 900, 3, None, 100, 0, 0));
        assert!(!renew_allowed(true, 900, 0, None, 100, 0, 0));
        // 首次执行允许
        assert!(renew_allowed(true, 900, 3, None, 100, 100, 0));
        // 冷却期内拒绝
        assert!(!renew_allowed(true, 900, 3, Some(300), 100, 100, 1));
        // 冷却结束且未达上限允许
        assert!(renew_allowed(true, 900, 3, Some(901), 100, 100, 1));
        // 已达当日上限拒绝
        assert!(!renew_allowed(true, 900, 3, Some(901), 100, 100, 3));
        // 跨日重新计数
        assert!(renew_allowed(true, 900, 3, Some(901), 101, 100, 3));
    }
}