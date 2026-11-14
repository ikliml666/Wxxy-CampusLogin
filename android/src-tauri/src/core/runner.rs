//! :core 进程的核心生命周期:由 Kotlin `CoreService` 经 JNI 驱动。
//!
//! 启动只依赖持久化配置(campus_core_logic::boot::boot_plan),不依赖 Activity ——
//! START_STICKY 重建服务时进程里没有 Activity,核心仍能恢复巡检与 WiFi watcher。
//!
//! 状态存 `Mutex<Option<Arc<CoreRuntime>>>":服务销毁(`nativeStop`)时 take 出来
//! 停巡检并释放 runtime,同一进程内再次 `nativeStart` 才能重建核心。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use campus_core_logic::boot::{boot_plan, BootPlan};

use crate::core::platform::{CoreCtx, CorePlatform};

struct CoreRuntime {
    rt: tokio::runtime::Runtime,
    ctx: CoreCtx,
}

static CORE: Mutex<Option<Arc<CoreRuntime>>> = Mutex::new(None);
/// boot 的探测/启动登录尚未取得 LOGIN_ATTEMPT_LOCK，不能用该锁代替 boot 生命周期。
static BOOT_ACTIVE: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(false));

struct BootGuard;
impl Drop for BootGuard {
    fn drop(&mut self) {
        BOOT_ACTIVE.store(false, Ordering::Release);
    }
}

fn should_keep_service(is_running: bool, login_in_progress: bool, boot_active: bool) -> bool {
    is_running || login_in_progress || boot_active
}

/// :core 进程入口(Kotlin `CoreService.onCreate` → JNI)。
pub fn start(data_dir: PathBuf, platform: Arc<dyn CorePlatform>) {
    let mut guard = match CORE.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    if guard.is_some() {
        eprintln!("[core] nativeStart 重复调用,忽略(旧 runtime 未释放)");
        return;
    }
    if let Err(e) = campus_login_lib::infra::logger::init_logger(data_dir.join("logs")) {
        eprintln!("[core] 日志初始化失败: {e}");
    }
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("campus-core")
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("[core] tokio runtime 创建失败: {e}");
            return;
        }
    };
    let ctx = CoreCtx::new(data_dir, platform);
    // 旁路代理只在确有 VPN 接管时启用：判据走 Android API（TRANSPORT_VPN），
    // 未开 VPN 时直连——代理会把请求源抹成 127.0.0.1，反而丢掉"本机从哪张卡出去"。
    {
        let p = ctx.platform_arc();
        campus_login_lib::network::bound_socket::set_vpn_active_check(move || p.is_vpn_active());
    }
    ctx.mark_status(|s| {
        s.core_alive = true;
        s.is_running = false;
        s.last_tick_ms = crate::core::platform::now_ms();
    });
    BOOT_ACTIVE.store(true, Ordering::Release);
    let core = Arc::new(CoreRuntime { rt, ctx: ctx.clone() });
    *guard = Some(core.clone());
    drop(guard);
    campus_login_lib::log_info!("monitor", ":core 进程核心已初始化");
    core.rt.spawn(async move {
        let _guard = BootGuard;
        boot(ctx).await;
    });
}

/// 启动恢复:只按持久化开关决定是否拉起巡检,再做启动自动登录与出站兜底。
async fn boot(ctx: CoreCtx) {
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let settings = match ctx.migrate_settings_storage().await {
        Ok(s) => s,
        Err(e) => {
            campus_login_lib::log_error!("monitor", "core 启动恢复读取配置失败: {e}");
            ctx.update_notification(campus_core_logic::notify::TEXT_CONFIG_UNAVAILABLE);
            ctx.emit(
                "startup-failed",
                serde_json::json!({ "reason": format!("读取配置失败: {e}") }),
            );
            return;
        }
    };
    campus_login_lib::infra::logger::set_log_retention_days(settings.log_retention_days);
    campus_login_lib::infra::logger::set_log_level(
        campus_login_lib::infra::logger::log_level_from_config(&settings.log_level),
    );
    let running = boot_plan(settings.enable_background_check) == BootPlan::StartMonitor;
    if running {
        if let Err(e) = crate::core::monitor::start_background_check(ctx.clone()).await {
            campus_login_lib::log_error!("monitor", "core 启动后台检测失败: {e}");
        }
    } else {
        campus_login_lib::log_info!("monitor", "后台检测已关闭,core 待命");
        ctx.update_notification(campus_core_logic::notify::TEXT_CORE_IDLE);
    }
    if settings.night_outbound_restore.is_empty() {
        ctx.restore_avoid_bad_wifi().await;
    }
    if settings.auto_login_on_start {
        crate::core::monitor::auto_login_on_start_pub(ctx.clone()).await;
    }
    if !should_keep_service(
        running,
        crate::core::login::login_in_progress(),
        false,
    ) {
        ctx.stop_service();
    }
}

fn with_core<R>(f: impl FnOnce(&CoreRuntime) -> R) -> Option<R> {
    let guard = match CORE.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    let core = guard.as_ref()?.clone();
    drop(guard);
    Some(f(&core))
}

/// Kotlin `CoreService` 经 Binder 收到的命令(在 IO 线程同步执行)。
pub fn command(action: &str, args_json: &str) -> String {
    let args: serde_json::Value =
        serde_json::from_str(args_json).unwrap_or(serde_json::Value::Null);
    let action = action.to_string();
    let result = with_core(|core| {
        let ctx = core.ctx.clone();
        core.rt.block_on(async move {
            let result = match action.as_str() {
                "start" => crate::core::monitor::start_background_check(ctx.clone()).await,
                "stop" => crate::core::monitor::stop_background_check(ctx.clone()).await,
                "trigger" => crate::core::monitor::trigger_background_check(&ctx)
                    .await
                    .map(|_| serde_json::json!({ "triggered": true })),
                "status" => Ok(crate::core::monitor::status_value(&ctx)),
                // UI 的 check_portal_status 走这里:与巡检共用 net_state 的单次探测
                "portalStatus" => {
                    let force = args["force"].as_bool().unwrap_or(false);
                    crate::core::net_state::portal_status(&ctx, force)
                        .await
                        .map(|s| serde_json::to_value(s).unwrap_or_default())
                }
                "restoreAvoidBadWifi" => {
                    ctx.restore_avoid_bad_wifi().await;
                    Ok(serde_json::json!({ "restored": true }))
                }
                "login" => login(&ctx, &args).await,
                "logout" => {
                    let user = args["user"].as_str().map(|s| s.to_string());
                    crate::core::login::run_logout(&ctx, user, "manual").await
                }
                other => Err(format!("未知 core 命令: {other}")),
            };
            let keep_service = should_keep_service(
                crate::core::monitor::is_running(),
                crate::core::login::login_in_progress(),
                BOOT_ACTIVE.load(Ordering::Acquire),
            );
            (result, keep_service)
        })
    });
    let (result, keep_service) = match result {
        Some(pair) => (Some(pair.0), pair.1),
        None => (None, true),
    };
    let result = match result {
        Some(Ok(v)) => serde_json::to_string(&v).unwrap_or_else(|e| error_json(&e.to_string())),
        Some(Err(e)) => error_json(&e),
        None => error_json("core 未初始化"),
    };
    serde_json::json!({ "result": result, "keepService": keep_service }).to_string()
}

async fn login(ctx: &CoreCtx, args: &serde_json::Value) -> Result<serde_json::Value, String> {
    let Some(attempt) = crate::core::login::try_begin_login() else {
        return Ok(serde_json::json!({
            "success": false,
            "message": "登录正在进行中",
            "inProgress": true
        }));
    };
    ctx.ensure_wifi_bound().await;
    let settings = ctx.settings().await?;
    let user = args["user"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or(settings.user.clone());
    let password = args["password"]
        .as_str()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or(settings.password.clone());
    let operator = args["operator"]
        .as_str()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or(settings.operator.clone());
    if user.is_empty() || password.is_empty() {
        return Err("账号与密码不能为空".to_string());
    }
    campus_login_lib::log_info!(
        "protocol",
        "core 手动登录: user_len={} operator_len={}",
        user.len(),
        operator.len()
    );
    let result = crate::core::login::run_login(ctx, &user, &password, &operator, attempt).await?;
    let message = result["message"].as_str().unwrap_or("").to_string();
    let success = result["success"].as_bool().unwrap_or(false);
    if let Err(e) = crate::login_history::append_async(
        ctx.data_dir().to_path_buf(),
        success,
        message,
        user,
        "manual".to_string(),
    )
    .await
    {
        campus_login_lib::log_warn!("protocol", "手动登录历史记录失败: {e}");
    }
    Ok(result)
}

fn error_json(msg: &str) -> String {
    serde_json::json!({ "error": msg }).to_string()
}

/// WiFi 变化事件(Kotlin CoreService 的网络回调 → JNI)
pub fn wifi_event(event: &str) {
    let event = event.to_string();
    let ctx = with_core(|core| core.ctx.clone());
    let Some(ctx) = ctx else { return };
    let _ = with_core(|core| {
        core.rt
            .spawn(async move { crate::core::monitor::handle_wifi_event(ctx, event).await });
    });
}

/// 服务侧定时心跳:不依赖巡检间隔,避免用户设长闲时周期后活性被误判为陈旧
pub fn heartbeat() {
    let ctx = {
        let guard = match CORE.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.as_ref().map(|core| core.ctx.clone())
    };
    if let Some(ctx) = ctx {
        ctx.mark_status(|status| {
            status.last_tick_ms = crate::core::platform::now_ms();
        });
    }
}

/// 服务销毁:停巡检与 watcher、释放 runtime;之后同进程再次 nativeStart 可重建
pub fn shutdown() {
    let core = {
        let mut guard = match CORE.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.take()
    };
    let Some(core) = core else { return };
    let ctx = core.ctx.clone();
    let _ = core
        .rt
        .block_on(async { crate::core::monitor::stop_background_check(ctx).await });
    core.ctx.mark_status(|status| {
        status.core_alive = false;
        status.is_running = false;
    });
    match Arc::try_unwrap(core) {
        Ok(CoreRuntime { rt, .. }) => rt.shutdown_background(),
        Err(shared) => {
            // 仍有命令在途:把 runtime 交给最后一个持有者,避免阻塞服务销毁
            drop(shared);
        }
    }
    campus_login_lib::log_info!("monitor", ":core 进程核心已释放");
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    #[test]
    fn 服务运行态读取巡检原子量() {
        let monitor = &crate::core::monitor::MONITOR;
        let previous = monitor.running.load(Ordering::Relaxed);
        monitor.running.store(false, Ordering::Relaxed);
        assert!(!super::should_keep_service(
            crate::core::monitor::is_running(),
            false,
            false,
        ));
        monitor.running.store(true, Ordering::Relaxed);
        assert!(super::should_keep_service(
            crate::core::monitor::is_running(),
            false,
            false,
        ));
        monitor.running.store(previous, Ordering::Relaxed);
    }

    #[test]
    fn 服务在无巡检无登录且启动完成时退出() {
        assert!(super::should_keep_service(true, false, false));
        assert!(super::should_keep_service(false, true, false));
        assert!(super::should_keep_service(false, false, true));
        assert!(!super::should_keep_service(false, false, false));
    }
}
