//! 登录/注销编排(:core 进程唯一入口)。
//!
//! 协议实现仍只在 `campus_login_lib::auth::protocol`;这里只做编排与登录所有权。
//! UI 进程的手动登录/注销必须经 Binder 转发到本进程,不得在 UI 进程另起一套,
//! 否则两进程各持一把锁会并发登录互相踢下线。

use std::sync::atomic::Ordering;
use std::sync::LazyLock;

use campus_login_lib::infra::state::{TaskGuard, TaskLock};

use crate::core::platform::CoreCtx;

static LOGIN_ATTEMPT_LOCK: LazyLock<TaskLock> = LazyLock::new(TaskLock::new);

pub struct LoginAttempt {
    _guard: TaskGuard<'static>,
}

/// 登录所有权:巡检/WiFi 事件/手动登录三条入口共用(进程内唯一)
pub fn try_begin_login() -> Option<LoginAttempt> {
    LOGIN_ATTEMPT_LOCK
        .try_acquire()
        .map(|guard| LoginAttempt { _guard: guard })
}

pub fn login_in_progress() -> bool {
    LOGIN_ATTEMPT_LOCK.is_active()
}

pub fn logout_protected() -> bool {
    crate::core::platform::now_ms()
        < crate::core::monitor::MONITOR
            .logout_protected_until_ms
            .load(Ordering::Relaxed)
}

impl LoginAttempt {
    pub fn finish(self, result: &serde_json::Value) {
        if result["success"].as_bool().unwrap_or(false) {
            let monitor = &crate::core::monitor::MONITOR;
            monitor.was_online.store(true, Ordering::Relaxed);
            monitor.logout_protected_until_ms.store(0, Ordering::Relaxed);
            monitor.consecutive_failures.store(0, Ordering::Relaxed);
        }
    }
}

/// 登录执行体(巡检自动重登/手动登录共用;返回协议 JSON {code,message,success,retryable})
pub async fn run_login(
    ctx: &CoreCtx,
    user: &str,
    password: &str,
    operator: &str,
    attempt: LoginAttempt,
) -> Result<serde_json::Value, String> {
    let adapter_ip = ctx.cached_source_ip();
    let user = user.to_string();
    let password = password.to_string();
    let operator = operator.to_string();
    // do_login_with_retry 是同步函数(内部 block_on_http 桥接),禁止在 async 上下文直接调用
    let result = tokio::task::spawn_blocking(move || {
        let is_quitting = std::sync::atomic::AtomicBool::new(false);
        campus_login_lib::auth::protocol::do_login_with_retry(
            &user,
            &password,
            &operator,
            adapter_ip.as_deref(),
            3,
            &is_quitting,
        )
    })
    .await
    .map_err(|e| format!("登录任务执行失败: {e}"))??;
    attempt.finish(&result);
    Ok(result)
}

/// 注销:两步注销(Radius + MAC 解绑),成功后写 60s 保护期
pub async fn run_logout(
    ctx: &CoreCtx,
    user: Option<String>,
    source: &str,
) -> Result<serde_json::Value, String> {
    // 注销与所有登录入口共用同一所有权;Binder 只串行 UI 命令,不能挡住 tokio 巡检
    let _attempt = try_begin_login().ok_or_else(|| "登录/注销进行中,请稍后".to_string())?;
    ctx.ensure_wifi_bound().await;
    let user = match user.map(|u| u.trim().to_string()) {
        Some(u) if !u.is_empty() => u,
        _ => ctx.settings().await?.user,
    };
    if user.is_empty() {
        return Err("注销需要学号,请先在账号输入框填写".to_string());
    }
    let adapter_ip = ctx.cached_source_ip();
    let user_for_history = user.clone();
    let monitor = &crate::core::monitor::MONITOR;
    let previous_protection = monitor.logout_protected_until_ms.load(Ordering::Relaxed);
    // 在发起协议前启用保护期;即使巡检此刻醒来,也会因所有权/保护期双重约束而不登回
    monitor.logout_protected_until_ms.store(
        crate::core::platform::now_ms() + 60_000,
        Ordering::Relaxed,
    );
    let inner = match tokio::task::spawn_blocking(move || {
        let is_quitting = std::sync::atomic::AtomicBool::new(false);
        campus_login_lib::auth::protocol::do_logout_with_retry(
            &user,
            adapter_ip.as_deref(),
            3,
            &is_quitting,
        )
    })
    .await
    {
        Ok(Ok(inner)) => inner,
        Ok(Err(e)) => {
            monitor.logout_protected_until_ms.store(previous_protection, Ordering::Relaxed);
            return Err(e);
        }
        Err(e) => {
            monitor.logout_protected_until_ms.store(previous_protection, Ordering::Relaxed);
            return Err(format!("注销任务执行失败: {e}"));
        }
    };
    // 协议可能重试超过 60s:保护期必须从注销完成重新起算,而不是只从发起时计算
    monitor.logout_protected_until_ms.store(
        crate::core::platform::now_ms() + 60_000,
        Ordering::Relaxed,
    );

    let message = inner["message"].as_str().unwrap_or("").to_string();
    let success = inner["success"].as_bool().unwrap_or(false);
    if let Err(e) = crate::login_history::append_async(
        ctx.data_dir().to_path_buf(),
        success,
        message,
        user_for_history,
        source.to_string(),
    )
    .await
    {
        campus_login_lib::log_warn!("protocol", "注销历史记录失败: {e}");
    }
    Ok(inner)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    #[test]
    fn 登录尝试单飞并统一成功记账() {
        struct RestoreMonitor {
            was_online: bool,
            logout_protected_until_ms: u64,
            consecutive_failures: u32,
        }
        impl Drop for RestoreMonitor {
            fn drop(&mut self) {
                let monitor = &crate::core::monitor::MONITOR;
                monitor.was_online.store(self.was_online, Ordering::Relaxed);
                monitor.logout_protected_until_ms.store(self.logout_protected_until_ms, Ordering::Relaxed);
                monitor.consecutive_failures.store(self.consecutive_failures, Ordering::Relaxed);
            }
        }
        let monitor = &crate::core::monitor::MONITOR;
        let _restore = RestoreMonitor {
            was_online: monitor.was_online.load(Ordering::Relaxed),
            logout_protected_until_ms: monitor.logout_protected_until_ms.load(Ordering::Relaxed),
            consecutive_failures: monitor.consecutive_failures.load(Ordering::Relaxed),
        };

        let attempt = super::try_begin_login().expect("首个登录尝试应取得所有权");
        assert!(super::try_begin_login().is_none(), "持有期间不得进入第二次登录");

        crate::core::monitor::MONITOR.was_online.store(false, Ordering::Relaxed);
        crate::core::monitor::MONITOR.logout_protected_until_ms.store(123, Ordering::Relaxed);
        crate::core::monitor::MONITOR.consecutive_failures.store(4, Ordering::Relaxed);
        attempt.finish(&serde_json::json!({ "success": true }));
        assert!(crate::core::monitor::MONITOR.was_online.load(Ordering::Relaxed));
        assert_eq!(crate::core::monitor::MONITOR.logout_protected_until_ms.load(Ordering::Relaxed), 0);
        assert_eq!(crate::core::monitor::MONITOR.consecutive_failures.load(Ordering::Relaxed), 0);

        let failed = super::try_begin_login().expect("成功收尾后应释放登录所有权");
        failed.finish(&serde_json::json!({ "success": false }));
        let next = super::try_begin_login().expect("失败收尾后也应释放登录所有权");
        drop(next);
    }
}
