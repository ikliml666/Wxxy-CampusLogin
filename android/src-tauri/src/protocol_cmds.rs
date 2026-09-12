//! 协议命令面:登录/注销/Portal 探测,命令名与参数与桌面版对齐(前端近零适配)。
//! 登录/注销复用桌面协议核心(auth::protocol),源 IP 绑定用检测阶段缓存的 wlan0 地址。
//! 敏感纪律:任何日志、错误信息、事件 payload 不得携带 password。

use std::sync::atomic::AtomicBool;
use tauri::Manager;

/// 兼容桌面契约:前端 invoke('do_login', {adapterName}) 仅传适配器,凭据由后端配置回退
#[tauri::command]
pub async fn do_login(
    user: Option<String>,
    password: Option<String>,
    operator: Option<String>,
    adapter: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::android_state::AndroidState>,
) -> Result<serde_json::Value, String> {
    let _ = adapter;
    // 手动登录此前漏了 ensure_wifi_bound（只有启动自动登录与 monitor 每拍有）——
    // WiFi+流量同开时默认路由落蜂窝，用户手动点登录必然失败
    ensure_wifi_bound(&app).await;
    // 用户名/密码为空时都回退已保存配置(总览一键登录免输凭据;与桌面取配置语义一致)
    let user = match user {
        Some(u) if !u.trim().is_empty() => u.trim().to_string(),
        _ => crate::config_state::current_settings(&app).await?.user,
    };
    let operator = operator.unwrap_or_default();
    let password = match password {
        Some(p) if !p.is_empty() => p,
        _ => crate::config_state::current_settings(&app).await?.password,
    };
    if user.is_empty() || password.is_empty() {
        eprintln!("[do_login][v2] reject: user_empty={} password_empty={}", user.is_empty(), password.is_empty());
        return Err("账号与密码不能为空".to_string());
    }
    eprintln!("[do_login] start: user_len={} operator={}", user.len(), operator);
    let operator = if operator.is_empty() {
        crate::config_state::current_settings(&app).await?.operator
    } else {
        operator
    };

    let result = run_login(&user, &password, &operator, &state).await?;
    // 登录历史落盘(桌面 session.rs 同构;此处为手动登录)
    if let Ok(dir) = app.path().app_data_dir() {
        let message = result["message"].as_str().unwrap_or("");
        let success = result["success"].as_bool().unwrap_or(false);
        let _ = crate::login_history::append(&dir, success, message, &user, "manual");
    }
    // 手动登录成功:清除自动登录熔断与注销保护(否则手动救回来后自动重登仍被闸住)
    if result["success"].as_bool().unwrap_or(false) {
        use crate::monitor_loop::MONITOR;
        MONITOR.consecutive_failures.store(0, std::sync::atomic::Ordering::Relaxed);
        MONITOR.logout_protected_until_ms.store(0, std::sync::atomic::Ordering::Relaxed);
    }
    Ok(result)
}

/// 登录执行体(monitor_loop 自动重登复用;返回协议 JSON {code,message,success,retryable})
pub async fn run_login(
    user: &str,
    password: &str,
    operator: &str,
    state: &tauri::State<'_, crate::android_state::AndroidState>,
) -> Result<serde_json::Value, String> {
    let adapter_ip = cached_adapter_ip(state);
    let user = user.to_string();
    let password = password.to_string();
    let operator = operator.to_string();
    // do_login_with_retry 是同步函数(内部 block_on_http 桥接),禁止在 async 上下文直接调用
    tauri::async_runtime::spawn_blocking(move || {
        let is_quitting = AtomicBool::new(false);
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
    .map_err(|e| format!("登录任务执行失败: {e}"))?
}

/// 注销:两步注销(Radius + MAC 解绑)复用桌面协议核心
#[tauri::command]
pub async fn do_logout(
    user: Option<String>,
    adapter: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::android_state::AndroidState>,
) -> Result<serde_json::Value, String> {
    // 注销协议请求同样必须走 WiFi(WiFi+流量同开时默认路由可能落蜂窝)
    ensure_wifi_bound(&app).await;
    let _ = adapter;
    let user = match user.map(|u| u.trim().to_string()) {
        Some(u) if !u.is_empty() => u,
        // 桌面注销仅需学号;user 空时回退已存账号
        _ => crate::config_state::current_settings(&app).await?.user,
    };
    if user.is_empty() {
        return Err("注销需要学号,请先在账号输入框填写".to_string());
    }
    let adapter_ip = cached_adapter_ip(&state);
    let user_for_history = user.clone();
    let inner = tauri::async_runtime::spawn_blocking(move || {
        let is_quitting = AtomicBool::new(false);
        campus_login_lib::auth::protocol::do_logout_with_retry(
            &user,
            adapter_ip.as_deref(),
            3,
            &is_quitting,
        )
    })
    .await
    .map_err(|e| format!("注销任务执行失败: {e}"))??;

    if let Ok(dir) = app.path().app_data_dir() {
        let message = inner["message"].as_str().unwrap_or("");
        let success = inner["success"].as_bool().unwrap_or(false);
        let _ = crate::login_history::append(&dir, success, message, &user_for_history, "manual");
    }
    // 注销保护期 60s:后台检测此前会在冷却后把用户自动登回,违背注销意图
    // (桌面 logout_protected_until 同语义;登录成功时清除)
    {
        use crate::monitor_loop::MONITOR;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        MONITOR.logout_protected_until_ms.store(now + 60_000, std::sync::atomic::Ordering::Relaxed);
    }
    Ok(inner)
}

/// 桌面同名命令:Portal 状态页探测(端口 80 页面特征判在线,协议走 :801)
#[tauri::command]
pub async fn check_portal_status(
    adapter_ip: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::android_state::AndroidState>,
) -> Result<campus_login_lib::auth::portal::PortalStatus, String> {
    // 手动探测同样先绑 WiFi（此前漏绑：探测请求走系统默认路由，双网同开时落蜂窝）
    ensure_wifi_bound(&app).await;
    let ip = match adapter_ip {
        Some(ip) if !ip.is_empty() => ip,
        _ => cached_adapter_ip(&state).unwrap_or_default(),
    };
    // check_portal_full 为同步函数(内部 block_on_http),必须 spawn_blocking
    tauri::async_runtime::spawn_blocking(move || {
        campus_login_lib::auth::portal::check_portal_full(&ip, None)
    })
    .await
    .map_err(|e| format!("Portal 探测任务执行失败: {e}"))?
}

fn cached_adapter_ip(
    state: &tauri::State<'_, crate::android_state::AndroidState>,
) -> Option<String> {
    state
        .cached_source_ip
        .lock()
        .ok()
        .and_then(|cached| cached.map(|ip| ip.to_string()))
}

#[tauri::command]
pub fn ping_test() -> &'static str {
    "pong"
}

/// 把进程网络绑定到 WLAN;返回 {"bound": bool}。前端 invoke("bind_to_wifi") 调用。
#[tauri::command]
pub fn bind_to_wifi(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    #[cfg(mobile)]
    {
        use tauri_plugin_campus_network_bind::CampusNetworkBindExt;
        app.campus_network_bind()
            .bind_to_wifi()
            .map_err(|e| e.to_string())
    }
    #[cfg(not(mobile))]
    {
        let _ = app;
        Err("网络绑定仅安卓端支持".to_string())
    }
}

/// 让系统"接受"当前无互联网的 WiFi（校园网认证前的 captive portal 场景），
/// 免去用户手动在系统弹窗点"仍然连接"。前端可通过 invoke("accept_wifi_network") 手动触发。
#[tauri::command]
pub fn accept_wifi_network(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    #[cfg(mobile)]
    {
        use tauri_plugin_campus_network_bind::CampusNetworkBindExt;
        app.campus_network_bind()
            .accept_wifi_network()
            .map_err(|e| e.to_string())
    }
    #[cfg(not(mobile))]
    {
        let _ = app;
        Err("网络接受仅安卓端支持".to_string())
    }
}

/// 全链路(启动自动登录/周期检测/断线重连/注销/手动登录/手动探测)执行前确保进程已绑 WiFi。
///
/// 根因：WiFi 未认证时被安卓网络评分降权，WiFi+流量同开下默认路由可能落到蜂窝，
/// 探测(TcpStream)/登录/注销全部走错网络；`bindProcessToNetwork` 是进程级 fwmark
/// （netd eBPF 在 **socket 创建时** 打标），对 tokio socket 与 Rust native 调用都生效。
///
/// **绑定成功后必须清空 HTTP 客户端池**：既有 keep-alive 连接的 fwmark 是绑前打的，
/// 复用它们等于绑定没生效——这是"绑了仍走错网络"的关键一环。
///
/// 结果只记日志不阻断：无 WiFi/绑定失败均回落默认路由，不改变流程语义；但日志
/// **必须落盘**（原先用 eprintln 只进 logcat，用户拿到的日志文件里看不到绑定成败，
/// 无从定位"为什么还是登录不上"）。
pub(crate) async fn ensure_wifi_bound(app: &tauri::AppHandle) {
    let cloned = app.clone();
    // JNI 调用是阻塞的,走 spawn_blocking 不占用 async 线程;复用 bind_to_wifi
    // 命令的 cfg(mobile) 门控,非移动端编译期消除为空实现,调用点无需配对门控
    let outcome = tauri::async_runtime::spawn_blocking(move || bind_to_wifi(cloned))
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
    match outcome {
        Ok(v) if v["bound"].as_bool().unwrap_or(false) => {
            let cleared = campus_login_lib::network::client::clear_client_pool();
            // reason 在此为所选 WiFi 的能力摘要（net/nonet + val/unval + cp），
            // 用于区分"绑到了正常 WiFi"还是"绑到了未认证的校园网 captive portal"
            campus_login_lib::log_info!(
                "wifi-bind",
                "进程网络已绑定 WiFi（path={}, wifi={}，清理旧连接 {} 条）",
                v["path"].as_str().unwrap_or("?"),
                v["reason"].as_str().unwrap_or("?"),
                cleared
            );
            // 真机排障通道:release 包无 root 读不到私有目录日志文件，logcat 是
            // 唯一免 root 可见的输出（同 do_login 的 eprintln 诊断）
            eprintln!(
                "[wifi-bind] ok path={} wifi={} cleared={}",
                v["path"].as_str().unwrap_or("?"),
                v["reason"].as_str().unwrap_or("?"),
                cleared
            );
            // 校园网认证前的 WiFi 会被系统判为"无互联网"，需用户在系统弹窗点
            // "仍然连接"才会被当作可用网络——绑定成功后顺带代劳，免去手动确认。
            // 是否真有动作由 Kotlin 侧按 NetworkCapabilities 判定（已验证则直接返回）
            accept_campus_wifi(app).await;
        }
        Ok(v) => {
            eprintln!(
                "[wifi-bind] not_bound reason={} path={}",
                v["reason"].as_str().unwrap_or("unknown"),
                v["path"].as_str().unwrap_or("?")
            );
            campus_login_lib::log_warn!(
                "wifi-bind",
                "未绑定 WiFi（reason={}），本次回落系统默认路由",
                v["reason"].as_str().unwrap_or("unknown")
            );
        }
        Err(e) => {
            eprintln!("[wifi-bind] call_failed: {e}");
            campus_login_lib::log_warn!(
                "wifi-bind",
                "WiFi 绑定调用失败（本次回落默认路由）: {e}"
            );
        }
    }
}

/// 让系统接受"无互联网"的 WiFi（校园网认证前的 captive portal 场景）。
///
/// 背景：AOSP ConnectivityService 只在 `explicitlySelected && !acceptUnvalidated` 时
/// 弹"此网络无法访问互联网 / 仍然连接"，而这两个字段属 NetworkAgent 侧、应用无公开
/// API 可写。这里按三条路径尝试（详见 NetworkBindPlugin.acceptWifiNetwork）：
/// 反射 hidden API `setAcceptUnvalidated` → 回退写 `Settings.Global`（captive_portal_mode=0
/// + network_avoid_bad_wifi=0）。无实际动作时 Kotlin 侧直接返回 already_validated。
///
/// 结果只记日志不阻断：拿不到 WRITE_SETTINGS 授权或 ROM 拦下写入时，用户仍可按系统
/// 弹窗手动确认（原有路径不变）。
pub(crate) async fn accept_campus_wifi(app: &tauri::AppHandle) {
    let cloned = app.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || accept_wifi_network(cloned))
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
    match outcome {
        Ok(v) if v["accepted"].as_bool().unwrap_or(false) => {
            eprintln!(
                "[wifi-accept] accepted path={} hiddenApi={} reason={}",
                v["path"].as_str().unwrap_or("?"),
                v["hiddenApi"].as_bool().unwrap_or(false),
                v["reason"].as_str().unwrap_or("?")
            );
            campus_login_lib::log_info!(
                "wifi-accept",
                "系统已接受该 WiFi（path={}, hiddenApi可达={}）",
                v["path"].as_str().unwrap_or("?"),
                v["hiddenApi"].as_bool().unwrap_or(false)
            )
        }
        Ok(v) => {
            eprintln!(
                "[wifi-accept] rejected path={} hiddenApi={} reason={}",
                v["path"].as_str().unwrap_or("?"),
                v["hiddenApi"].as_bool().unwrap_or(false),
                v["reason"].as_str().unwrap_or("?")
            );
            campus_login_lib::log_warn!(
                "wifi-accept",
                "未能让系统接受该 WiFi（path={}, hiddenApi可达={}, reason={}）",
                v["path"].as_str().unwrap_or("?"),
                v["hiddenApi"].as_bool().unwrap_or(false),
                v["reason"].as_str().unwrap_or("?")
            );
        }
        Err(e) => {
            eprintln!("[wifi-accept] call_failed: {e}");
            campus_login_lib::log_warn!("wifi-accept", "网络接受调用失败: {e}");
        }
    }
}
