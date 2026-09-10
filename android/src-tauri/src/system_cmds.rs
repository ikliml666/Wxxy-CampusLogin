//! 聚合初始化与日志命令:与桌面 commands/system.rs 同构的安卓子集。


/// 启动聚合数据:前端 useInitialDataLoad 一次拉全(config 已掩码,明文不出后端)
#[tauri::command]
pub async fn get_init_data(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let settings = crate::config_state::current_settings(&app).await?;
    let dir = crate::account_cmds::accounts_dir(&app)?;
    let accounts = crate::account_cmds::list_account_names_sync(&dir);

    Ok(serde_json::json!({
        "config": crate::config_state::masked_for_display(&settings),
        "accounts": accounts,
        "version": env!("CARGO_PKG_VERSION"),
        // 桌面专属字段补空默认,避免前端 useInitialDataLoad 读到 undefined 崩溃
        "autoLaunch": settings.enable_boot_autostart,
        "gpuInfo": serde_json::Value::Null,
        "refreshRate": 60,
        "adapters": [],
        "adapterDetails": [],
        "disabledAdapters": [],
        "activeAccount": settings.active_account,
        "notificationEnabled": settings.enable_notification,
        "isAutoStart": settings.enable_boot_autostart,
        "backgroundStatus": crate::monitor_loop::status_value(),
    }))
}

#[tauri::command]
pub async fn get_logs(app: tauri::AppHandle, lines: Option<usize>) -> Result<String, String> {
    let lines = lines.unwrap_or(200);
    tauri::async_runtime::spawn_blocking(move || {
        campus_login_lib::infra::logger::read_recent_logs(&app, lines)
    })
    .await
    .map_err(|e| format!("读取日志任务失败: {e}"))?
}

#[tauri::command]
pub async fn clear_logs(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || campus_login_lib::infra::logger::clear_logs(&app))
        .await
        .map_err(|e| format!("清理日志任务失败: {e}"))?
}

#[tauri::command]
pub fn get_log_retention_days() -> u32 {
    campus_login_lib::infra::logger::get_log_retention_days()
}

#[tauri::command]
pub fn set_log_retention_days(days: u32) {
    campus_login_lib::infra::logger::set_log_retention_days(days);
}

#[tauri::command]
pub fn get_debug_mode() -> Result<bool, String> {
    campus_login_lib::infra::logger::get_debug_mode()
}

#[tauri::command]
pub fn set_debug_mode(enabled: bool) -> Result<bool, String> {
    campus_login_lib::infra::logger::set_debug_mode(enabled)
}

/// 设备 SoC/性能信息:前端据此分档调帧率。
/// ro.soc.model 为 Android 12+ CDD 强制系统属性(骁龙 8 Gen3=SM8650、天玑 9300=MT6989);
/// 低于 12 无值时回退大核/内存启发式。仅回传前端消费的字段(tier + 调试用型号),
/// 大核/内存只参与 tier 计算,不过线。
#[tauri::command]
pub async fn get_soc_info() -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(read_soc_info)
        .await
        .map_err(|e| format!("读取设备信息任务失败: {e}"))?
}

fn read_soc_info() -> Result<serde_json::Value, String> {
    #[cfg(target_os = "android")]
    let (soc, device_model) = {
        let props = android_system_properties::AndroidSystemProperties::new();
        (
            props.get("ro.soc.model").unwrap_or_default(),
            props.get("ro.product.model").unwrap_or_default(),
        )
    };
    // 桌面/host 测试环境:型号补空,tier 走大核/内存启发式
    #[cfg(not(target_os = "android"))]
    let (soc, device_model) = (String::new(), String::new());
    let big_cores = count_big_cores();
    let mem_total_mb = read_mem_total_mb();
    Ok(serde_json::json!({
        "socModel": soc,
        "deviceModel": device_model,
        // 0 未知 / 1 入门 / 2 中高 / 3 旗舰
        "tier": tier_of(&soc, big_cores, mem_total_mb),
    }))
}

/// 大核计数:逐核读最大频率,≥1.8GHz 视为大核(小核普遍 ≤1.5GHz)
fn count_big_cores() -> u32 {
    let mut big = 0u32;
    for i in 0..16u32 {
        let path = format!("/sys/devices/system/cpu/cpu{i}/cpufreq/cpuinfo_max_freq");
        if let Ok(s) = std::fs::read_to_string(&path) {
            if let Ok(khz) = s.trim().parse::<u64>() {
                if khz >= 1_800_000 {
                    big += 1;
                }
            }
        }
    }
    big
}

fn read_mem_total_mb() -> u64 {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .next()?
                .split_whitespace()
                .nth(1)?
                .parse::<u64>()
                .ok()
        })
        .map(|kb| kb / 1024)
        .unwrap_or(0)
}

/// 分档:SoC 型号映射优先,无属性值时按大核/内存启发式兜底。
/// 覆盖边界:仅骁龙 8 系/天玑 9000 后旗舰与骁龙 7 系/天玑 8-9 系中高档;
/// 麒麟/Exynos/Tensor 等未覆盖,落启发式/入门档(帧率偏保守,无正确性影响)。
fn tier_of(soc: &str, big_cores: u32, mem_mb: u64) -> u32 {
    let s = soc.to_ascii_uppercase();
    // 骁龙 8 系全代逐型号列出:865/870=SM8250 … 8 Elite=SM8750;
    // 8s Gen3(SM8635)是中端,不能按 SM86 前缀误判旗舰
    let flagship = ["SM8250", "SM8350", "SM8450", "SM8475", "SM8550", "SM8650", "SM8750"]
        .iter()
        .any(|m| s.starts_with(m))
        || s.starts_with("MT6989") // 天玑 9300
        || s.starts_with("MT6991"); // 天玑 9400
    let upper_mid = s.starts_with("SM7") // 骁龙 7 系全代
        || s.starts_with("MT6985") || s.starts_with("MT6983") || s.starts_with("MT689")
        || s.starts_with("MT698");
    if flagship {
        3
    } else if upper_mid {
        2
    } else if s.is_empty() {
        if big_cores >= 4 && mem_mb >= 8 * 1024 {
            2
        } else if big_cores >= 2 || mem_mb >= 4 * 1024 {
            1
        } else {
            0
        }
    } else {
        1
    }
}
