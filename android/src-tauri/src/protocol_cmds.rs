//! 协议命令面:登录/注销/Portal 探测,命令名与参数与桌面版对齐(前端近零适配)。
//! 登录/注销复用桌面协议核心(auth::protocol),源 IP 绑定用检测阶段缓存的 wlan0 地址。
//! 敏感纪律:任何日志、错误信息、事件 payload 不得携带 password。


/// 桌面同名命令:Portal 状态页探测(端口 80 页面特征判在线,协议走 :801)
///
/// 探测收归 :core 单点(`core::net_state`):UI 不再自建旁路连接打同一个网关。
/// 此前 UI 与巡检各探一次,一次网络抖动会被记成两条失败,界面还会在
/// 「在线/不可达」之间闪;源地址与绑定也统一由 :core 按当前接口实时取。
#[tauri::command]
pub async fn check_portal_status(
    adapter_ip: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::android_state::AndroidState>,
) -> Result<serde_json::Value, String> {
    let _ = (adapter_ip, &state);
    crate::core_bridge_cmds::call(&app, "portalStatus", "{\"force\":false}").await
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

/// 查询 Settings.Global 写通道状态（前端夜间出站切换引导块显示）：
/// WRITE_SECURE_SETTINGS 是否已授权（adb pm grant 后）、当前 avoid_bad_wifi 值、
/// 插件是否有待还原快照。JNI 阻塞调用走 spawn_blocking。
#[tauri::command]
pub async fn get_avoid_bad_wifi_status(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    #[cfg(mobile)]
    {
        use tauri_plugin_campus_network_bind::CampusNetworkBindExt;
        let outcome = tauri::async_runtime::spawn_blocking(move || {
            app.campus_network_bind().get_secure_settings_status()
        })
        .await
        .map_err(|e| e.to_string())?;
        outcome.map_err(|e| e.to_string())
    }
    #[cfg(not(mobile))]
    {
        let _ = app;
        Err("仅安卓端支持".to_string())
    }
}

/// 还原本应用写入的系统网络设置（手动出口：captive_portal_mode=0 /
/// network_avoid_bad_wifi=0 / 出站切换写的 avoid_bad_wifi 覆盖，按插件快照写回原值）。
#[tauri::command]
pub async fn restore_written_settings(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    #[cfg(mobile)]
    {
        use tauri_plugin_campus_network_bind::CampusNetworkBindExt;
        let outcome = tauri::async_runtime::spawn_blocking(move || {
            app.campus_network_bind().restore_written_settings()
        })
        .await
        .map_err(|e| e.to_string())?;
        outcome.map_err(|e| e.to_string())
    }
    #[cfg(not(mobile))]
    {
        let _ = app;
        Err("仅安卓端支持".to_string())
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
    // 旁路代理生效时跳过:传输出口由 SO_BINDTODEVICE 物理网卡绑定决定(内核设备级,
    // secure VPN 也拦不住),bindProcessToNetwork 冗余;且 secure VPN 全量接管本应用时
    // 它会被 netd EPERM 拒绝,每拍打误导性"未绑定 WiFi"警告(2026-09-14 用户反馈:
    // "日志一直警告未绑定 WiFi,但实际可用")。跳过同时省去每次的绑定尝试耗时。
    #[cfg(target_os = "android")]
    if campus_login_lib::network::bound_socket::bind_capability()
        == campus_login_lib::network::bound_socket::BindCapability::Supported
    {
        return;
    }
    let cloned = app.clone();
    // JNI 调用是阻塞的,走 spawn_blocking 不占用 async 线程;复用 bind_to_wifi
    // 命令的 cfg(mobile) 门控,非移动端编译期消除为空实现,调用点无需配对门控
    let outcome = tauri::async_runtime::spawn_blocking(move || bind_to_wifi(cloned))
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
    match outcome {
        Ok(v) if v["bound"].as_bool().unwrap_or(false) => {
            // 同一网络重复绑定（path=already_bound）时不清池：fwmark 已指向该网络，
            // 池中旧连接的标记仍然正确。启动瞬间多条链路会在一秒内并发调用本函数
            // （真机 1 秒 6 次），每次都清池会让首批请求反复重建连接（多付 TLS 握手）。
            let cleared = if v["path"].as_str() == Some("already_bound") {
                0
            } else {
                campus_login_lib::network::client::clear_client_pool()
            };
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
            campus_login_lib::log_warn!("wifi-bind", "WiFi 绑定调用失败（本次回落默认路由）: {e}");
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
