//! 电池优化白名单:查询状态、一次性申请跳转、厂商自启/省电页跳转(含降级链)。
//! 平台专属能力(Windows 无对应 API),桌面端无本模块;安卓经 foreground-service
//! 插件命令面转发。厂商跳转候选表与降级链实现在 Kotlin 侧(插件:
//! MonitorServicePlugin.vendorTargets / openVendorBatterySettings)。

use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BatteryOptimizationInfo {
    /// 已在"不优化电池"白名单中
    pub ignoring: bool,
    /// Build.BRAND 小写(前端据此只显示对应厂商项)
    pub brand: String,
    /// 该品牌是否有专用跳转页
    pub has_vendor_target: bool,
}

#[tauri::command]
pub async fn get_battery_optimization_info(
    app: tauri::AppHandle,
) -> Result<BatteryOptimizationInfo, String> {
    #[cfg(mobile)]
    {
        use tauri_plugin_campus_monitor_service::CampusMonitorServiceExt;
        let v = app
            .campus_monitor_service()
            .get_battery_optimization_info()
            .map_err(|e| e.to_string())?;
        return Ok(BatteryOptimizationInfo {
            ignoring: v["ignoring"].as_bool().unwrap_or(false),
            brand: v["brand"].as_str().unwrap_or_default().to_string(),
            has_vendor_target: v["hasVendorTarget"].as_bool().unwrap_or(false),
        });
    }
    #[cfg(not(mobile))]
    {
        let _ = &app;
        Err("电池优化白名单仅安卓端可用".to_string())
    }
}

/// 一次性申请:返回 true 表示系统确认框已弹出(用户是否点"允许"由系统 UI 决定,
/// 前端在返回后延迟重查 get_battery_optimization_info 刷新状态)。
#[tauri::command]
pub async fn request_ignore_battery_optimizations(app: tauri::AppHandle) -> Result<bool, String> {
    #[cfg(mobile)]
    {
        use tauri_plugin_campus_monitor_service::CampusMonitorServiceExt;
        let v = app
            .campus_monitor_service()
            .request_ignore_battery_optimizations()
            .map_err(|e| e.to_string())?;
        return Ok(v["opened"].as_bool().unwrap_or(false));
    }
    #[cfg(not(mobile))]
    {
        let _ = &app;
        Err("电池优化白名单仅安卓端可用".to_string())
    }
}

/// 跳厂商自启/省电页(降级链);返回 {path, target, tried}
#[tauri::command]
pub async fn open_vendor_battery_settings(
    app: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    #[cfg(mobile)]
    {
        use tauri_plugin_campus_monitor_service::CampusMonitorServiceExt;
        return app
            .campus_monitor_service()
            .open_vendor_battery_settings()
            .map_err(|e| e.to_string());
    }
    #[cfg(not(mobile))]
    {
        let _ = &app;
        Err("厂商保活设置页仅安卓端可用".to_string())
    }
}
