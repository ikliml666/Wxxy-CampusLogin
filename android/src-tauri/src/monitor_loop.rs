//! 后台监控循环:tick = 校园网判定 → Portal 探测 → 掉线自动重登(cooldown + 次数上限)
//! → emit background-check-result / login-log。决策逻辑抽纯函数便于 TDD,
//! tokio 循环体与前台服务保活在下方(Task 4)。

use chrono::Timelike;
use lazy_static::lazy_static;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
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
    /// 闲时巡检间隔(ms):蜂窝/灭屏时生效,start_background_check 从配置刷新
    pub idle_interval_ms: AtomicU64,
    /// 注销保护期截止(epoch ms):手动注销后一段时间内不自动重登,登录成功清零
    pub logout_protected_until_ms: AtomicU64,
    /// 最近一次 WiFi 变化事件的 epoch ms:去抖与"风暴内最后事件生效"判定
    pub wifi_event_ms: AtomicU64,
    /// 常驻通知已展示的在线状态(0=未展示,1=在线,2=未连接):仅状态翻转时才
    /// notify 重建通知——每拍重建常驻通知是稳态功耗点(60s 一拍 IPC + notify),
    /// 文案不再携带逐拍递增的检测次数(检测次数前端状态页有)
    pub notified_online: AtomicU8,
}

/// WiFi 事件触发检测的延迟:连上瞬间 DHCP/路由往往未就绪,立即探测必失败;
/// 延迟等网络稳定再跑,同时充当"事件风暴合并窗口"(连上 WiFi 会连发
/// onAvailable + onCapabilitiesChanged 等多条事件,窗口内最后一条生效)
const WIFI_EVENT_DELAY_MS: u64 = 2500;

/// WiFi 事件去抖窗口:距上次事件不足该值视为同一次风暴的后续事件,不再重复安排
const WIFI_EVENT_DEBOUNCE_MS: u64 = 1000;

/// WiFi 事件是否为一次风暴的起点(应安排一次检测)。纯函数,单测锁定:
/// 首事件(last=0)或距上次事件 >= 去抖窗口 → 起点;窗口内 → 后续事件,
/// 由已安排的任务执行(风暴内只跑一次)
fn wifi_event_is_burst_start(last_event_ms: u64, now_ms: u64, debounce_ms: u64) -> bool {
    last_event_ms == 0 || now_ms.saturating_sub(last_event_ms) >= debounce_ms
}

/// 巡检分档(纯函数,单测锁定):WiFi 且屏幕亮着 → 基础间隔(默认 60s);
/// 蜂窝网络或屏幕熄灭 → 闲时间隔(默认 5min)。
/// 拉长间隔不改变正确性:WiFi 变化事件仍即时触发检测(handle_wifi_event),
/// 且无明确离线证据时在线状态保持上一拍记忆(三态护栏)。
/// 闲时间隔不足基础间隔时以基础间隔为准,避免配置误配成更频繁。
pub fn effective_interval_ms(base_ms: u64, idle_ms: u64, screen_on: bool, wifi_connected: bool) -> u64 {
    let base = base_ms.max(5000);
    if screen_on && wifi_connected {
        base
    } else {
        idle_ms.max(base)
    }
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
pub(crate) fn notify_system(app: &tauri::AppHandle, enabled: bool, title: &str, body: &str, mascot: &str) {
    if !enabled {
        return;
    }
    use tauri_plugin_notification::NotificationExt;
    // mascot 为 drawable 资源名（如 mascot_alert），作为通知大图显示看板娘；
    // 资源缺失时插件抛 Resources.NotFoundException，降级为不带大图的纯文本。
    let result = app
        .notification()
        .builder()
        .title(title)
        .body(body)
        .large_icon(mascot)
        .show();
    if result.is_err() {
        let _ = app.notification().builder().title(title).body(body).show();
    }
}

/// 启动后台监控:Rust tokio 循环 + 前台服务保活(幂等)
#[tauri::command]
pub async fn start_background_check(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let settings = crate::config_state::current_settings(&app).await?;
    let want = settings.background_check_interval.max(5000);
    MONITOR.desired_interval_ms.store(want, Ordering::Relaxed);
    MONITOR
        .idle_interval_ms
        .store(settings.background_check_idle_interval, Ordering::Relaxed);
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
    // 重置通知状态记忆:服务通知刚以"校园网监控运行中"重建,首拍需刷新为实时状态
    MONITOR.notified_online.store(0, Ordering::Relaxed);

    // WiFi 变化监听随后台检测起停:变化事件即时触发一次完整检测(不等下一拍)
    #[cfg(mobile)]
    start_wifi_watcher(&app);

    tauri::async_runtime::spawn(monitor_tick_loop(app.clone(), want));
    emit_login_log(&app, "后台监控已启动", "info");
    Ok(serde_json::json!({ "isRunning": true }))
}

#[tauri::command]
pub async fn stop_background_check(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    MONITOR.running.store(false, Ordering::Relaxed);
    stop_wifi_watcher(&app);
    #[cfg(mobile)]
    {
        use tauri_plugin_campus_monitor_service::CampusMonitorServiceExt;
        let _ = app.campus_monitor_service().stop_monitor();
    }
    emit_login_log(&app, "后台监控已停止", "info");
    Ok(serde_json::json!({ "isRunning": false }))
}

/// WiFi 变化监听的注册 channel id:注销 removeListener 需要
#[cfg(mobile)]
static WIFI_WATCHER_CHANNEL_ID: AtomicU32 = AtomicU32::new(0);
#[cfg(mobile)]
static WIFI_WATCHER_ACTIVE: AtomicBool = AtomicBool::new(false);

/// 注册 WiFi 变化监听(Kotlin NetworkCallback → channel 事件):随后台检测开关
/// 起停——start_background_check 成功后调用;失败只记日志,退化为纯周期检测
#[cfg(mobile)]
pub(crate) fn start_wifi_watcher(app: &tauri::AppHandle) {
    use tauri_plugin_campus_network_bind::CampusNetworkBindExt;

    let app_h = app.clone();
    let channel = tauri::ipc::Channel::<serde_json::Value>::new(move |body| {
        handle_wifi_event(app_h.clone(), body)
    });
    let id = channel.id();
    match app.campus_network_bind().start_wifi_watcher(channel) {
        Ok(_) => {
            WIFI_WATCHER_CHANNEL_ID.store(id, Ordering::Relaxed);
            WIFI_WATCHER_ACTIVE.store(true, Ordering::Relaxed);
            campus_login_lib::log_info!("monitor", "WiFi 变化监听已启动(WiFi 变化即时触发检测)");
        }
        Err(e) => {
            campus_login_lib::log_warn!("monitor", "WiFi 变化监听启动失败,退化为纯周期检测: {e}")
        }
    }
}

/// 注销 WiFi 变化监听(随 stop_background_check 调用)
#[cfg(mobile)]
pub(crate) fn stop_wifi_watcher(app: &tauri::AppHandle) {
    use tauri_plugin_campus_network_bind::CampusNetworkBindExt;

    if !WIFI_WATCHER_ACTIVE.swap(false, Ordering::Relaxed) {
        return;
    }
    let id = WIFI_WATCHER_CHANNEL_ID.load(Ordering::Relaxed);
    if let Err(e) = app.campus_network_bind().stop_wifi_watcher(id) {
        campus_login_lib::log_warn!("monitor", "WiFi 变化监听注销失败: {e}");
    } else {
        campus_login_lib::log_info!("monitor", "WiFi 变化监听已停止");
    }
}

/// WiFi 变化事件处理:去抖 + 延迟后执行一次完整检测(run_check_once)。
/// channel 消息体为 {"event": "available|lost|validated|unvalidated"}
#[cfg(mobile)]
fn handle_wifi_event(
    app: tauri::AppHandle,
    body: tauri::ipc::InvokeResponseBody,
) -> tauri::Result<()> {
    // 事件只在后台监控运行时有意义(watcher 生命周期与其绑定,此处兜底防竞态)
    if !is_running() {
        return Ok(());
    }
    let event = body
        .deserialize::<serde_json::Value>()
        .ok()
        .and_then(|v| v["event"].as_str().map(str::to_string));
    let Some(event) = event else {
        return Ok(());
    };
    let now = epoch_ms();
    if !wifi_event_is_burst_start(
        MONITOR.wifi_event_ms.load(Ordering::Relaxed),
        now,
        WIFI_EVENT_DEBOUNCE_MS,
    ) {
        return Ok(());
    }
    MONITOR.wifi_event_ms.store(now, Ordering::Relaxed);
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(WIFI_EVENT_DELAY_MS)).await;
        // 风暴内更新的起点已覆盖时间戳:本任务过期自杀,由最后安排的任务执行
        if MONITOR.wifi_event_ms.load(Ordering::Relaxed) != now {
            return;
        }
        campus_login_lib::log_info!("monitor", "检测到 WiFi 变化({event}),立即执行网络状态检测");
        run_check_once(&app).await;
    });
    Ok(())
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
    // 探测/登录前强制绑 WiFi:WiFi+流量同开时默认路由可能落蜂窝,探测与登录走错网络
    crate::protocol_cmds::ensure_wifi_bound(app).await;
    let state = app.state::<crate::android_state::AndroidState>();
    let probe = match probe_with_retry(settings).await {
        Ok(p) => p,
        Err(e) => {
            emit_login_log(app, &format!("启动自动登录:校园网判定失败 {e}"), "warning");
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
            notify_system(app, settings.enable_notification, "启动自动登录失败", &msg, "mascot_offline");
        }
        Err(e) => {
            emit_login_log(app, &format!("启动自动登录失败: {e}"), "error");
            notify_system(app, settings.enable_notification, "启动自动登录失败", &e, "mascot_offline");
        }
    }
}

/// 探针窗口 guard:进入窗口持 WifiLock/WakeLock,离开(Drop,含 early return 与 panic
/// unwind)必释放。Android 8+ 后台 startService 受限,故经插件命令直调服务静态入口。
#[cfg(mobile)]
struct ProbeWindowGuard(tauri::AppHandle);

#[cfg(mobile)]
impl Drop for ProbeWindowGuard {
    fn drop(&mut self) {
        use tauri_plugin_campus_monitor_service::CampusMonitorServiceExt;
        let _ = self.0.campus_monitor_service().end_probe_window();
    }
}

/// 电源状态:(屏幕交互中, 当前活动网络为 WiFi)。
/// 查询失败按保守值 (true, true) 处理——按基础间隔巡检,不因查询异常漏检测
/// (省电是优化项,漏检是功能缺陷)。
#[cfg(mobile)]
fn power_state(app: &tauri::AppHandle) -> (bool, bool) {
    use tauri_plugin_campus_monitor_service::CampusMonitorServiceExt;
    app.campus_monitor_service()
        .get_power_state()
        .unwrap_or((true, true))
}

#[cfg(not(mobile))]
fn power_state(_app: &tauri::AppHandle) -> (bool, bool) {
    (true, true)
}

async fn monitor_tick_loop(app: tauri::AppHandle, interval_ms: u64) {
    // 下限 5s:防误配超小间隔打爆探测
    let mut current = interval_ms.max(5000);
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(current));
    let mut last_probe_ms: u64 = 0;
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
        // 分档:唤醒周期恒为基础间隔(保证亮屏/回 WiFi 后最迟一拍恢复),
        // 是否真正跑探针由闲时间隔决定——蜂窝/灭屏时跳拍,省掉整轮 Portal 探测
        let idle_ms = MONITOR.idle_interval_ms.load(Ordering::Relaxed);
        let (screen_on, wifi_connected) = power_state(&app);
        let effective = effective_interval_ms(current, idle_ms, screen_on, wifi_connected);
        let now = epoch_ms();
        if last_probe_ms != 0 && now.saturating_sub(last_probe_ms) < effective {
            continue;
        }
        last_probe_ms = now;
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
    // 裸线程不在 Tokio runtime 上下文内:check_portal_full 内部经 block_on_sync 驱动的
    // reqwest,构造超时计时器(tokio::time::sleep → Handle::current)时必须有 reactor
    // context,缺失即 panic "there is no reactor running"(2026-09-11 真机每拍必现)。
    // 先在本 async 上下文取 handle,线程内 enter——与桌面 commands/login.rs 的
    // scope 裸线程同款处理(2026-09-05 注销流程崩溃事故根因);绑小核意图不变。
    let probe_handle = tokio::runtime::Handle::try_current().ok();
    let spawned = std::thread::Builder::new()
        .name("portal-probe".to_string())
        .spawn(move || {
            let _guard = probe_handle.as_ref().map(|h| h.enter());
            crate::cpu_affinity::pin_current_thread_to_little_cores();
            // panic 兜底:裸线程 panic 直接 unwind 时 rx.await 只会得到"探测线程提前
            // 退出",真实原因(reactor context 缺失)就此丢失、每拍静默失败。捕获后
            // 转为 Err 结果回传,默认 panic hook 仍会把消息打到 logcat。
            let probed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                campus_login_lib::auth::portal::check_portal_full(&ip_for_thread, None)
            }));
            let result = probed.unwrap_or_else(|_| Err("探测线程 panic".to_string()));
            let _ = tx.send(result);
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

    // 校园网检测静默期(时间门控,桌面 background_check 同语义):早于配置的开始时间
    // (当日分钟数,0=禁用)或晚于结束时间(0/<=开始时间=不限制,与桌面
    // is_campus_check_silent 同退化规则)整拍跳过,避免非在校时段反复探测与误报
    // 掉线通知;在线状态保持上一拍记忆,不构成误判
    if settings.campus_check_start_minutes > 0 {
        let now = chrono::Local::now();
        let minutes_now = now.hour() as u16 * 60 + now.minute() as u16;
        let end = settings.campus_check_end_minutes;
        if minutes_now < settings.campus_check_start_minutes
            || (end > settings.campus_check_start_minutes && minutes_now >= end)
        {
            return;
        }
    }

    // 探针窗口:本轮巡检期间持 WifiLock(防 WiFi 省电断流)+ 唤醒锁,
    // 窗口外全部释放(省电)。guard 保证任何 early return / panic 都释放。
    #[cfg(mobile)]
    let _probe_window = {
        use tauri_plugin_campus_monitor_service::CampusMonitorServiceExt;
        let _ = app.campus_monitor_service().begin_probe_window();
        ProbeWindowGuard(app.clone())
    };

    // 每拍探测/Portal 探测/掉线自动重登前强制绑 WiFi(同 auto_login_on_start;
    // WiFi 未认证被降分后默认路由可能落蜂窝)
    crate::protocol_cmds::ensure_wifi_bound(app).await;

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
        notify_system(app, settings.enable_notification, "校园网连接掉线", &format!("将自动重登: {portal_message}"), "mascot_alert");
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
            notify_system(app, settings.enable_notification, "自动重连已达上限", "本轮在线周期内不再自动重登,请手动登录", "mascot_offline");
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
                    notify_system(app, settings.enable_notification, "自动重登失败", &message, "mascot_offline");
                }
                login_result = Some(v);
            }
            Err(e) => {
                MONITOR.consecutive_failures.fetch_add(1, Ordering::Relaxed);
                emit_login_log(app, &format!("自动登录执行失败: {e}"), "error");
                notify_system(app, settings.enable_notification, "自动重登失败", &e, "mascot_offline");
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

    // 常驻通知同步实时状态:仅在线状态翻转时 notify(Chronometer 起点由服务侧
    // 固定,时长不被打断)。每拍重建是稳态功耗点,2026-09-12 调研后改为按需更新
    #[cfg(mobile)]
    {
        use tauri_plugin_campus_monitor_service::CampusMonitorServiceExt;
        let state_code = if online { 1u8 } else { 2 };
        if MONITOR.notified_online.swap(state_code, Ordering::Relaxed) != state_code {
            let state = if online { "在线" } else { "未连接" };
            let _ = app
                .campus_monitor_service()
                .update_notification(&format!("监控运行中 · {state}"));
        }
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

    #[test]
    fn wifi事件_首事件与窗口外为起点_窗口内不算() {
        assert!(wifi_event_is_burst_start(0, 1_000_000, WIFI_EVENT_DEBOUNCE_MS));
        // 距上次事件 >= 去抖窗口 → 新风暴起点
        assert!(wifi_event_is_burst_start(
            1_000_000,
            1_000_000 + WIFI_EVENT_DEBOUNCE_MS,
            WIFI_EVENT_DEBOUNCE_MS
        ));
        // 窗口内的后续事件(同一次连上 WiFi 的连发)不安排新检测
        assert!(!wifi_event_is_burst_start(
            1_000_000,
            1_000_000 + WIFI_EVENT_DEBOUNCE_MS - 1,
            WIFI_EVENT_DEBOUNCE_MS
        ));
    }

    #[test]
    fn 巡检分档_wifi且亮屏走基础间隔_其余走闲时() {
        // WiFi + 亮屏 → 基础间隔
        assert_eq!(effective_interval_ms(60_000, 300_000, true, true), 60_000);
        // 蜂窝 → 闲时间隔
        assert_eq!(effective_interval_ms(60_000, 300_000, true, false), 300_000);
        // 灭屏 → 闲时间隔
        assert_eq!(effective_interval_ms(60_000, 300_000, false, true), 300_000);
        // 灭屏 + 蜂窝 → 闲时间隔
        assert_eq!(effective_interval_ms(60_000, 300_000, false, false), 300_000);
    }

    #[test]
    fn 巡检分档_闲时间隔小于基础间隔时取基础间隔() {
        assert_eq!(effective_interval_ms(120_000, 30_000, false, false), 120_000);
    }

    #[test]
    fn 巡检分档_基础间隔下限5s() {
        assert_eq!(effective_interval_ms(1_000, 300_000, true, true), 5_000);
        assert_eq!(effective_interval_ms(1_000, 0, false, false), 5_000);
    }
}
