//! 后台监控循环:tick = 校园网判定 → Portal 探测 → 掉线自动重登(cooldown + 次数上限)
//! → emit background-check-result / login-log。决策逻辑抽纯函数便于 TDD,
//! tokio 循环体与前台服务保活在下方(Task 4)。

use lazy_static::lazy_static;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::Manager;

lazy_static! {
    pub(crate) static ref MONITOR: MonitorState = MonitorState::default();
}

#[derive(Default)]
#[allow(dead_code)] // Task 4 循环体接线
pub struct MonitorState {
    pub running: AtomicBool,
    pub check_count: AtomicU64,
    pub consecutive_failures: AtomicU32,
    /// 本次在线周期内自动重连次数(达 max_disconnect_reconnect 暂停,回到在线清零)
    pub reconnect_count: AtomicU32,
    pub last_login_attempt_ms: AtomicU64,
    pub was_online: AtomicBool,
    pub last_result: Mutex<Option<serde_json::Value>>,
    /// 运行中循环的目标检测间隔(ms):start_background_check 每次调用刷新,
    /// 循环体逐 tick 对比实现"改间隔立即生效"(否则幂等分支吞掉新间隔)
    pub desired_interval_ms: AtomicU64,
    /// 注销保护期截止(epoch ms):手动注销后一段时间内不自动重登,登录成功清零
    pub logout_protected_until_ms: AtomicU64,
}

/// 本 tick 是否应尝试登录(纯函数,全量条件显式入参)
#[allow(dead_code)] // Task 4 循环体接线
pub fn should_attempt_login(
    online: bool,
    was_online: bool,
    on_campus: bool,
    auto_login_on_preparation: bool,
    reconnect_count: u32,
    max_reconnect: u32,
    millis_since_last_attempt: u64,
    cooldown_secs: u64,
) -> bool {
    if online || !on_campus {
        return false;
    }
    // 非掉线场景(启动即离线)需显式开启自动登录;掉线重连只受后台检测总开关管
    if !was_online && !auto_login_on_preparation {
        return false;
    }
    if reconnect_count >= max_reconnect {
        return false;
    }
    millis_since_last_attempt >= cooldown_secs.saturating_mul(1000)
}

/// get_init_data 的 backgroundStatus 与前端 get_background_status 命令共用的出站形状
pub fn status_value() -> serde_json::Value {
    let last = MONITOR.last_result.lock().ok().and_then(|guard| guard.clone());
    let mut v = serde_json::json!({
        "isRunning": MONITOR.running.load(Ordering::Relaxed),
        "checkCount": MONITOR.check_count.load(Ordering::Relaxed),
        "consecutiveFailures": MONITOR.consecutive_failures.load(Ordering::Relaxed),
        "reconnectCount": MONITOR.reconnect_count.load(Ordering::Relaxed),
        "lastResult": last.as_ref(),
    });
    // 展平最近一次检测的关键字段到顶层:前端启动时读 bgStatus.online 驱动
    // 状态点(首轮 emit 先于 WebView 监听建立,事件会丢,getInitData 是唯一
    // 可靠的启动初值来源)
    if let Some(last) = &last {
        for key in ["online", "message", "serverAvailable", "onCampusNetwork"] {
            if let Some(val) = last.get(key) {
                v[key] = val.clone();
            }
        }
    }
    v
}

#[allow(dead_code)] // Task 4 循环体接线
pub fn is_running() -> bool {
    MONITOR.running.load(Ordering::Relaxed)
}

fn epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn emit_login_log(app: &tauri::AppHandle, message: &str, log_type: &str) {
    use tauri::Emitter;
    let payload = serde_json::json!({ "message": message, "type": log_type });
    let _ = app.emit("login-log", payload);
}

/// 系统通知(区别于常驻通知):掉线/重连失败/达上限/新版本等关键事件。
/// 此前安卓关键事件零提醒(桌面 emit_notification 系同语义),锁屏时用户
/// 对掉线完全无感知;enable_notification=false 时静默。
pub(crate) fn notify_system(app: &tauri::AppHandle, enabled: bool, title: &str, body: &str) {
    if !enabled {
        return;
    }
    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder().title(title).body(body).show();
}

/// 启动后台监控:Rust tokio 循环 + 前台服务保活(幂等)
#[tauri::command]
pub async fn start_background_check(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let settings = crate::config_state::current_settings(&app).await?;
    let want = settings.background_check_interval.max(5000);
    MONITOR.desired_interval_ms.store(want, Ordering::Relaxed);
    if is_running() {
        // 已在跑:仅刷新目标间隔,循环体下 tick 重建计时器(改间隔立即生效)
        return Ok(serde_json::json!({ "isRunning": true }));
    }

    #[cfg(mobile)]
    {
        use tauri_plugin_campus_monitor_service::CampusMonitorServiceExt;
        let service = app.campus_monitor_service();
        // 先启动 FGS,成功后才置运行标志:启动失败会 reject 给前端且不残留假"运行中"
        service
            .start_monitor("校园网监控运行中")
            .map_err(|e| e.to_string())?;
        // 自启设置与监控开关独立;此处仅同步当前配置到插件
        let _ = service.set_boot_autostart(settings.enable_boot_autostart);
    }
    MONITOR.running.store(true, Ordering::Relaxed);

    tauri::async_runtime::spawn(monitor_tick_loop(app.clone(), want));
    emit_login_log(&app, "后台监控已启动", "info");
    Ok(serde_json::json!({ "isRunning": true }))
}

#[tauri::command]
pub async fn stop_background_check(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    MONITOR.running.store(false, Ordering::Relaxed);
    #[cfg(mobile)]
    {
        use tauri_plugin_campus_monitor_service::CampusMonitorServiceExt;
        let _ = app.campus_monitor_service().stop_monitor();
    }
    emit_login_log(&app, "后台监控已停止", "info");
    Ok(serde_json::json!({ "isRunning": false }))
}

#[tauri::command]
pub async fn trigger_background_check(app: tauri::AppHandle) -> Result<(), String> {
    run_check_once(&app).await;
    Ok(())
}

#[tauri::command]
pub fn get_background_status() -> serde_json::Value {
    status_value()
}

/// 开机自启开关:插件组件启停 + 配置字段同步(设置页 set_auto_launch 等价物)
#[tauri::command]
pub async fn get_boot_autostart(app: tauri::AppHandle) -> Result<bool, String> {
    #[cfg(mobile)]
    {
        use tauri_plugin_campus_monitor_service::CampusMonitorServiceExt;
        return app
            .campus_monitor_service()
            .is_boot_autostart_enabled()
            .map_err(|e| e.to_string());
    }
    #[cfg(not(mobile))]
    {
        let _ = &app;
        Ok(false)
    }
}

#[tauri::command]
pub async fn set_boot_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    #[cfg(mobile)]
    {
        use tauri_plugin_campus_monitor_service::CampusMonitorServiceExt;
        app.campus_monitor_service()
            .set_boot_autostart(enabled)
            .map_err(|e| e.to_string())?;
    }
    let _io = crate::account_cmds::config_io_lock().await;
    let bridge = crate::config_state::CryptoBridge::from_app(&app);
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取数据目录失败: {e}"))?;
    let mut settings = crate::config_state::current_settings(&app).await?;
    settings.enable_boot_autostart = enabled;
    crate::config_state::save_to(&dir, &bridge, &settings).await?;
    drop(_io);
    if let Ok(mut cache) = app.state::<crate::android_state::AndroidState>().config.lock() {
        *cache = Some(settings);
    }
    Ok(())
}

/// 通知开关:配置字段读写(设置页 set_notification_enabled 等价物)
#[tauri::command]
pub async fn get_notification_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    Ok(crate::config_state::current_settings(&app).await?.enable_notification)
}

#[tauri::command]
pub async fn set_notification_enabled(app: tauri::AppHandle, enabled: bool) -> Result<bool, String> {
    let _io = crate::account_cmds::config_io_lock().await;
    let bridge = crate::config_state::CryptoBridge::from_app(&app);
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取数据目录失败: {e}"))?;
    let mut settings = crate::config_state::current_settings(&app).await?;
    settings.enable_notification = enabled;
    crate::config_state::save_to(&dir, &bridge, &settings).await?;
    drop(_io);
    if let Ok(mut cache) = app.state::<crate::android_state::AndroidState>().config.lock() {
        *cache = Some(settings);
    }
    Ok(enabled)
}

/// 启动恢复(对齐桌面 startup.rs→watcher::run_startup_tasks):按持久化配置
/// 拉起后台检测/定时质量测试循环并执行启动自动登录。此前安卓三者均只在用户
/// 手动切换时启动,重启 app 后"开关开着却不运行"——即自动登录无效/定时测试
/// 不跑/后台检测无效三个反馈的共性根因。
///
/// 2026-09-09 启动提速:三条互不依赖的启动链(后台检测/质量首测/自动登录)
/// 由串行 await 改为并行 spawn——质量首测要跑完 12 个外网域名(数秒级),
/// 串行时自动登录被压在最后,冷启动登录完成时间被成倍拖长;并行后即 tokio
/// multi-thread 运行时的多核利用。就绪窗口从桌面同款 1.5s 收窄到 500ms,
/// 网络未就绪的极端场景由自动登录的一次重试与后台检测下一拍兜底。
pub fn run_startup_tasks(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let settings = match crate::config_state::current_settings(&app).await {
            Ok(s) => s,
            Err(_) => return,
        };
        if settings.enable_background_check {
            let app2 = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = start_background_check(app2).await;
            });
        }
        if settings.enable_network_quality {
            let enable_latency = settings.enable_latency_test;
            let app2 = app.clone();
            tauri::async_runtime::spawn(async move {
                if enable_latency {
                    let _ = crate::quality_cmds::start_latency_test(app2).await;
                } else {
                    // 定时测试关着时的唯一首屏来源:启动做一次质量检测(emit
                    // network-quality-result),否则顶部胶囊/质量页永远 pending
                    let _ = crate::quality_cmds::check_network_quality(app2).await;
                }
            });
        }
        // 自动更新检查循环(桌面 24h 同语义):有新版本 emit + 系统通知
        crate::update_cmds::start_update_check_loop(app.clone());
        if settings.auto_login_on_start {
            let app2 = app.clone();
            tauri::async_runtime::spawn(async move {
                match crate::config_state::current_settings(&app2).await {
                    Ok(s) => auto_login_on_start(&app2, &s).await,
                    Err(_) => {}
                }
            });
        }
    });
}

/// 启动探测:未确认校园网(探测失败/非校园网)时 3s 后重试一次——开机自启等
/// 场景网络/DHCP 可能尚未就绪,启动就绪窗口收窄后更易撞上;重试仍不通过按
/// 最后一次结果返回,语义与原单次探测一致(Err→判定失败,Ok 非 campus→跳过)。
async fn probe_with_retry(
    settings: &crate::config_state::Settings,
) -> Result<crate::campus_detect::CampusProbe, String> {
    let first =
        crate::campus_detect::probe_campus(&settings.campus_gateway, &settings.portal_url).await;
    if first.as_ref().is_ok_and(|p| p.on_campus) {
        return first;
    }
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    let retry =
        crate::campus_detect::probe_campus(&settings.campus_gateway, &settings.portal_url).await;
    if retry.as_ref().is_ok_and(|p| p.on_campus) {
        return retry;
    }
    retry.or(first)
}

/// 自动登录成功走应用内通知(前端 auto-login-result 监听→应用内 toast):
/// 启动自动登录与首拍自动重登常连续发生,系统通知重复轰炸(2026-09-09 真机
/// 反馈);失败仍保留系统通知(锁屏感知)。日志记录由 login-log 负责,不重复。
fn emit_auto_login_result(app: &tauri::AppHandle, success: bool, message: &str) {
    use tauri::Emitter;
    let _ = app.emit(
        "auto-login-result",
        serde_json::json!({ "success": success, "message": message }),
    );
}

/// 启动自动登录(对齐桌面 auto_auth::run_auto_login_on_start 的安卓精简版):
/// 有凭据 + 判定在校园网才登录;非校园网不登录由 probe_campus 的 on_campus 保证
async fn auto_login_on_start(app: &tauri::AppHandle, settings: &crate::config_state::Settings) {
    if settings.user.is_empty() || settings.password.is_empty() {
        return;
    }
    let state = app.state::<crate::android_state::AndroidState>();
    let probe = match probe_with_retry(settings).await {
        Ok(p) => p,
        Err(e) => {
            emit_login_log(app, &format!("启动自动登录:校园网判定失败 {e}"), "warn");
            return;
        }
    };
    crate::campus_detect::cache_source_ip(&state, probe.source);
    if !probe.on_campus {
        emit_login_log(app, "启动检测:不在校园网环境,跳过自动登录", "info");
        return;
    }
    emit_login_log(app, "检测到校园网环境,执行启动自动登录", "info");
    match crate::protocol_cmds::run_login(&settings.user, &settings.password, &settings.operator, &state).await {
        Ok(v) if v["success"].as_bool().unwrap_or(false) => {
            let msg = v["message"].as_str().unwrap_or("").to_string();
            emit_login_log(app, &format!("启动自动登录成功: {msg}"), "success");
            // 登录成功是在线的权威证据:预置 was_online,首拍探测 Unknown/Failed
            // 保持记忆时为"在线"而非初始 false(FGS 通知文案/掉线检测起点正确);
            // 同时清注销保护期,防止跨会话残留影响本轮掉线重登
            MONITOR.was_online.store(true, Ordering::Relaxed);
            MONITOR.logout_protected_until_ms.store(0, Ordering::Relaxed);
            emit_auto_login_result(app, true, &msg);
        }
        Ok(v) => {
            let msg = v["message"].as_str().unwrap_or("").to_string();
            emit_login_log(app, &format!("启动自动登录失败: {msg}"), "error");
            notify_system(app, settings.enable_notification, "启动自动登录失败", &msg);
        }
        Err(e) => {
            emit_login_log(app, &format!("启动自动登录失败: {e}"), "error");
            notify_system(app, settings.enable_notification, "启动自动登录失败", &e);
        }
    }
}

async fn monitor_tick_loop(app: tauri::AppHandle, interval_ms: u64) {
    // 下限 5s:防误配超小间隔打爆探测
    let mut current = interval_ms.max(5000);
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(current));
    loop {
        tick.tick().await;
        if !is_running() {
            break;
        }
        // 间隔热更新:运行中改检测间隔(start_background_check 刷新 desired),
        // 重建计时器;tokio interval 重建后首个 tick 立即返回,吃掉保持节奏
        let want = MONITOR.desired_interval_ms.load(Ordering::Relaxed);
        if want != 0 && want != current {
            current = want;
            tick = tokio::time::interval(std::time::Duration::from_millis(current));
            tick.tick().await;
        }
        run_check_once(&app).await;
    }
    MONITOR.running.store(false, Ordering::Relaxed);
}

/// 周期检测的 Portal 全量探测:专用短命线程执行并绑定小核(检测是 60s 一拍的
/// 稳态周期任务,线程创建 ~1ms 可忽略)。tokio worker 与 spawn_blocking 池都是
/// 共享的,直接绑会把登录等前台任务一并拖到小核——所以用独立线程。绑核失败
/// (节点缺失/无大小核拓扑)线程照常运行,仅回落内核自动调度。
async fn portal_probe_on_little_cores(
    ip: String,
) -> Result<campus_login_lib::auth::portal::PortalStatus, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let ip_for_thread = ip.clone();
    let spawned = std::thread::Builder::new()
        .name("portal-probe".to_string())
        .spawn(move || {
            crate::cpu_affinity::pin_current_thread_to_little_cores();
            let _ = tx.send(campus_login_lib::auth::portal::check_portal_full(&ip_for_thread, None));
        });
    match spawned {
        Ok(_) => rx.await.map_err(|e| format!("探测线程提前退出: {e}"))?,
        Err(e) => {
            eprintln!("[portal_probe] 线程创建失败,降级共享阻塞池: {e}");
            // 线程创建失败降级回共享阻塞池(仅保正确性;原始 ip 未被 move)
            tauri::async_runtime::spawn_blocking(move || {
                campus_login_lib::auth::portal::check_portal_full(&ip, None)
            })
            .await
            .unwrap_or_else(|join| Err(format!("探测任务失败: {join}")))
        }
    }
}

/// 单次检测:校园网判定 → Portal 探测 → 状态机 → 自动重登 → emit
pub async fn run_check_once(app: &tauri::AppHandle) {
    MONITOR.check_count.fetch_add(1, Ordering::Relaxed);
    let settings = match crate::config_state::current_settings(app).await {
        Ok(s) => s,
        Err(e) => {
            emit_login_log(app, &format!("读取配置失败,跳过本轮检测: {e}"), "error");
            return;
        }
    };

    // 1. 校园网判定(复用探针;源 IP 缓存同步更新——自动重登/质量测试绑定用,
    //    此前后台链路不缓存,首次重登拿空/过期 IP 被协议拒)
    let probe = crate::campus_detect::probe_campus(&settings.campus_gateway, &settings.portal_url).await;
    let (on_campus, source_ip) = match probe {
        Ok(p) => {
            crate::campus_detect::cache_source_ip(&app.state::<crate::android_state::AndroidState>(), p.source);
            (p.on_campus, p.source)
        }
        Err(e) => {
            emit_login_log(app, &format!("校园网检测失败: {e}"), "error");
            return;
        }
    };

    // 2. Portal 探测(同步函数 → 绑小核的专用短命线程,稳态功耗)
    let ip = source_ip.map(|i| i.to_string()).unwrap_or_default();
    let portal = portal_probe_on_little_cores(ip).await;
    let (portal_reachable, portal_login_available, portal_message) = match &portal {
        Ok(s) => (s.reachable, s.login_available, s.message.clone()),
        Err(e) => (false, false, e.clone()),
    };
    // 三态消费:仅"确定判定"(error_kind=None)才翻转在线状态——Unknown(已在线
    // 页面特征失配)/Failed(探测超时)不构成可信离线证据,保持上一拍记忆。
    // 此前无条件折叠成 online=false,Portal 已在线页面特征(GBK 中文乱码、
    // 严格串匹配)间歇失配即误判掉线:状态点每拍打灰 + 自动重登"已经在线"
    // (真机 2026-09-09 反馈:不手动登录不变绿、过一会又变灰)。真掉线由
    // Determined(false)(Portal 返回登录页)正常翻转,不受此护栏影响。
    let prev_online = MONITOR.was_online.load(Ordering::Relaxed);
    let online = match &portal {
        Ok(s) if s.error_kind.is_none() => s.online,
        _ => prev_online,
    };

    // 3. 状态机:在线清零计数,记录 was_online
    if online {
        MONITOR.reconnect_count.store(0, Ordering::Relaxed);
        MONITOR.consecutive_failures.store(0, Ordering::Relaxed);
        MONITOR.logout_protected_until_ms.store(0, Ordering::Relaxed);
    }
    let was_online = MONITOR.was_online.swap(online, Ordering::Relaxed);
    // 掉线通知:从在线翻离线且在校园网(桌面 background_emit 同语义)
    if was_online && !online && on_campus {
        notify_system(app, settings.enable_notification, "校园网连接掉线", &format!("将自动重登: {portal_message}"));
    }

    // 4. 自动登录判定与执行(敏感纪律:payload/日志只含结果不含密码)
    //    调用侧两道闸(纯函数不动):
    //    ①注销保护期——手动注销后 60s 内不自动重登,否则注销即被登回(真机反馈语义);
    //    ②连续失败熔断——凭据错误时无限期每冷却重试刷日志耗流量,达 5 次停,手动登录成功清零
    let mut login_result: Option<serde_json::Value> = None;
    let now = epoch_ms();
    let since = now.saturating_sub(MONITOR.last_login_attempt_ms.load(Ordering::Relaxed));
    let logout_protected = now < MONITOR.logout_protected_until_ms.load(Ordering::Relaxed);
    let failures_capped = MONITOR.consecutive_failures.load(Ordering::Relaxed) >= 5;
    let should = !logout_protected
        && !failures_capped
        && should_attempt_login(
        online,
        was_online,
        on_campus,
        settings.auto_login_on_preparation,
        MONITOR.reconnect_count.load(Ordering::Relaxed),
        settings.max_disconnect_reconnect,
        since,
        settings.auto_login_cooldown_secs,
    );
    if should && !settings.user.is_empty() && !settings.password.is_empty() {
        MONITOR.last_login_attempt_ms.store(now, Ordering::Relaxed);
        MONITOR.reconnect_count.fetch_add(1, Ordering::Relaxed);
        let count = MONITOR.reconnect_count.load(Ordering::Relaxed);
        if count >= settings.max_disconnect_reconnect {
            notify_system(app, settings.enable_notification, "自动重连已达上限", "本轮在线周期内不再自动重登,请手动登录");
        }
        let state = app.state::<crate::android_state::AndroidState>();
        match crate::protocol_cmds::run_login(&settings.user, &settings.password, &settings.operator, &state).await {
            Ok(v) => {
                let success = v["success"].as_bool().unwrap_or(false);
                let message = v["message"].as_str().unwrap_or("").to_string();
                emit_login_log(app, &format!("自动登录: {message}"), if success { "info" } else { "error" });
                if let Ok(dir) = app.path().app_data_dir() {
                    let _ = crate::login_history::append(&dir, success, &message, &settings.user, "auto");
                }
                if success {
                    // 成功走应用内 toast(auto-login-result);系统通知仅保留失败场景
                    emit_auto_login_result(app, true, &message);
                } else {
                    // 协议返回失败(凭据错误等)与执行失败同计入熔断,达 5 次停止本会话自动重登
                    MONITOR.consecutive_failures.fetch_add(1, Ordering::Relaxed);
                    notify_system(app, settings.enable_notification, "自动重登失败", &message);
                }
                login_result = Some(v);
            }
            Err(e) => {
                MONITOR.consecutive_failures.fetch_add(1, Ordering::Relaxed);
                emit_login_log(app, &format!("自动登录执行失败: {e}"), "error");
                notify_system(app, settings.enable_notification, "自动重登失败", &e);
            }
        }
    }

    // 5. 组装 payload emit(字段名对齐桌面 background-check-result 可适用子集)
    let campus_message = if on_campus { "已连接校园网" } else { "未连接校园网" };
    let payload = serde_json::json!({
        "serverAvailable": portal_reachable,
        "loginAvailable": portal_login_available,
        "online": online,
        "message": if online { "在线".to_string() } else { portal_message.clone() },
        "timestamp": now,
        "checkCount": MONITOR.check_count.load(Ordering::Relaxed),
        "isRunning": is_running(),
        "onCampusNetwork": on_campus,
        "campusMessage": campus_message,
        "sourceIp": source_ip.map(|i| i.to_string()),
        "autoLogin": login_result,
    });
    if let Ok(mut last) = MONITOR.last_result.lock() {
        *last = Some(payload.clone());
    }
    use tauri::Emitter;
    let _ = app.emit("background-check-result", payload);

    // 常驻通知同步实时状态(仅更新文案,Chronometer 起点由服务侧固定,时长不被打断)
    #[cfg(mobile)]
    {
        use tauri_plugin_campus_monitor_service::CampusMonitorServiceExt;
        let count = MONITOR.check_count.load(Ordering::Relaxed);
        let state = if online { "在线" } else { "未连接" };
        let _ = app
            .campus_monitor_service()
            .update_notification(&format!("监控运行中 · {state} · 已检测 {count} 次"));
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    const COOLDOWN: u64 = 60;

    #[test]
    fn 在线或非校园网_永不尝试() {
        assert!(!should_attempt_login(true, false, true, true, 0, 3, u64::MAX, COOLDOWN));
        assert!(!should_attempt_login(false, false, false, true, 0, 3, 0, COOLDOWN));
    }

    #[test]
    fn 掉线重连_不受自动登录开关管() {
        // was_online=true:检测到在线→掉线,自动重连
        assert!(should_attempt_login(false, true, true, false, 0, 3, u64::MAX, COOLDOWN));
    }

    #[test]
    fn 启动即离线_需显式开启自动登录() {
        assert!(!should_attempt_login(false, false, true, false, 0, 3, u64::MAX, COOLDOWN));
        assert!(should_attempt_login(false, false, true, true, 0, 3, u64::MAX, COOLDOWN));
    }

    #[test]
    fn 重连达上限_暂停() {
        assert!(!should_attempt_login(false, true, true, true, 3, 3, u64::MAX, COOLDOWN));
        assert!(should_attempt_login(false, true, true, true, 2, 3, u64::MAX, COOLDOWN));
    }

    #[test]
    fn cooldown_未到_不重试() {
        assert!(!should_attempt_login(false, true, true, true, 0, 3, 59_999, COOLDOWN));
        assert!(should_attempt_login(false, true, true, true, 0, 3, 60_000, COOLDOWN));
    }

    #[test]
    fn cooldown_为零_立即重试() {
        assert!(should_attempt_login(false, true, true, true, 0, 3, 0, 0));
    }
}
