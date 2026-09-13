#![cfg(mobile)]
//! 移动插件:前台服务保活(常驻通知 + WifiLock + PARTIAL_WAKE_LOCK)与开机自启开关。
//! 只负责"让进程活着",监控/自动重登的业务逻辑全在 Rust 侧 monitor_loop(协议不双实现)。

use serde::Deserialize;
use tauri::{
    plugin::{Builder, PluginHandle, TauriPlugin},
    Manager, Runtime,
};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "com.campuslogin.plugin.monitorservice";

#[derive(Deserialize)]
struct BoolResult {
    enabled: bool,
}

pub struct CampusMonitorService<R: Runtime>(PluginHandle<R>);

type Result<T> = std::result::Result<T, tauri::plugin::mobile::PluginInvokeError>;

impl<R: Runtime> CampusMonitorService<R> {
    /// 启动前台服务(常驻通知);text 为通知文案
    pub fn start_monitor(&self, title: &str) -> Result<serde_json::Value> {
        self.0.run_mobile_plugin("startMonitor", serde_json::json!({ "title": title }))
    }

    pub fn stop_monitor(&self) -> Result<serde_json::Value> {
        self.0.run_mobile_plugin("stopMonitor", ())
    }

    /// 更新前台通知文案(如登录结果)
    pub fn update_notification(&self, text: &str) -> Result<serde_json::Value> {
        self.0.run_mobile_plugin("updateNotification", serde_json::json!({ "text": text }))
    }

    /// 进入探针窗口:窗口内持有 WifiLock + 唤醒锁(巡检每拍开始调用)
    pub fn begin_probe_window(&self) -> Result<serde_json::Value> {
        self.0.run_mobile_plugin("beginProbeWindow", ())
    }

    /// 退出探针窗口:释放窗口锁(幂等;由 run_check_once 的 drop guard 保证必达)
    pub fn end_probe_window(&self) -> Result<serde_json::Value> {
        self.0.run_mobile_plugin("endProbeWindow", ())
    }

    /// 开关开机自启(BOOT_COMPLETED receiver 组件启停 + SharedPreferences 记忆)
    pub fn set_boot_autostart(&self, enabled: bool) -> Result<serde_json::Value> {
        self.0.run_mobile_plugin("setBootAutostart", serde_json::json!({ "enabled": enabled }))
    }

    pub fn is_boot_autostart_enabled(&self) -> Result<bool> {
        let r: BoolResult = self.0.run_mobile_plugin("isBootAutostartEnabled", ())?;
        Ok(r.enabled)
    }

    /// 安装 APK(系统包安装器):filePath 须位于 app filesDir/update/ 内,Kotlin 侧
    /// 以文件名重新拼路径防逃逸;FileProvider content:// URI(桌面 exe/msi 不适用)
    pub fn install_apk(&self, file_path: &str) -> Result<serde_json::Value> {
        self.0.run_mobile_plugin("installApk", serde_json::json!({ "text": file_path }))
    }
}

pub trait CampusMonitorServiceExt<R: Runtime> {
    fn campus_monitor_service(&self) -> &CampusMonitorService<R>;
}

impl<R: Runtime, T: Manager<R>> CampusMonitorServiceExt<R> for T {
    fn campus_monitor_service(&self) -> &CampusMonitorService<R> {
        self.state::<CampusMonitorService<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("campus-monitor-service")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "MonitorServicePlugin")?;
            app.manage(CampusMonitorService(handle));
            Ok(())
        })
        .build()
}
