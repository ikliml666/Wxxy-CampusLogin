#![cfg(mobile)]
//! 移动插件:把进程网络绑定到 WLAN(ConnectivityManager.bindProcessToNetwork),
//! 经 netd FwmarkServer 对进程内全部 socket(含 Rust native)生效,强制登录流量走 WiFi。

use tauri::{
    plugin::{Builder, PluginHandle, TauriPlugin},
    Manager, Runtime,
};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "com.campuslogin.plugin.networkbind";

pub struct CampusNetworkBind<R: Runtime>(PluginHandle<R>);

type Result<T> = std::result::Result<T, tauri::plugin::mobile::PluginInvokeError>;

impl<R: Runtime> CampusNetworkBind<R> {
    /// 把进程绑定到当前具备 WiFi 传输能力的 Network;返回 Kotlin 侧结果 {"bound": bool}
    pub fn bind_to_wifi(&self) -> Result<serde_json::Value> {
        self.0.run_mobile_plugin("bindToWifi", ())
    }

    /// 让系统"接受"这张无互联网的 WiFi(校园网认证前的 captive portal 场景),
    /// 免去用户手动在系统弹窗点"仍然连接";
    /// 返回 {"accepted": bool, "path": String, "reason"?: String}
    pub fn accept_wifi_network(&self) -> Result<serde_json::Value> {
        self.0.run_mobile_plugin("acceptWifiNetwork", ())
    }

    /// 注册 WiFi 变化监听：channel 经基类 registerListener 绑定为 "wifiChanged"
    /// 事件的接收端，Kotlin 侧 NetworkCallback 触发时 send {"event": ...}
    pub fn start_wifi_watcher(
        &self,
        channel: tauri::ipc::Channel<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        self.0
            .run_mobile_plugin::<serde_json::Value>(
                "registerListener",
                serde_json::json!({ "event": "wifiChanged", "handler": channel }),
            )?;
        self.0
            .run_mobile_plugin::<serde_json::Value>("startWifiWatcher", ())
    }

    /// 注销监听：移除 listener channel + 注销系统回调。
    /// removeListener 失败不阻断（插件可能尚未注册过 listener）
    pub fn stop_wifi_watcher(&self, channel_id: u32) -> Result<serde_json::Value> {
        let _ = self.0.run_mobile_plugin::<serde_json::Value>(
            "removeListener",
            serde_json::json!({ "event": "wifiChanged", "channelId": channel_id as u64 }),
        );
        self.0
            .run_mobile_plugin::<serde_json::Value>("stopWifiWatcher", ())
    }

    /// 取当前 WiFi SSID（只读，Kotlin 侧绝不弹权限框）：
    /// `{"granted": bool, "ssid": String}`——未授权/低版本/未连 WiFi 时 ssid 为空串。
    /// JNI 阻塞调用，调用方须走 spawn_blocking（同 bind_to_wifi）。
    pub fn get_wifi_ssid(&self) -> Result<serde_json::Value> {
        self.0.run_mobile_plugin("getWifiSsid", ())
    }

    /// 触发 NEARBY_WIFI_DEVICES 运行时权限请求（幂等）：已授权/低版本无感 resolve；
    /// 未授权时弹系统授权框（须用户前台，由前端 UI 显式调用）。
    pub fn request_wifi_ssid_permission(&self) -> Result<serde_json::Value> {
        self.0.run_mobile_plugin("requestWifiSsidPermission", ())
    }

    /// 报告当前 WiFi 网络不可用：触发系统 NetworkMonitor 对该网络重新验证，
    /// 验证失败后由系统决定是否把默认网络切到蜂窝（network_avoid_bad_wifi）；
    /// 返回 {"reported": bool, "hasWifi": bool, "reason"?: String}。
    /// JNI 阻塞调用，调用方须走 spawn_blocking（同 bind_to_wifi）。
    pub fn report_wifi_unusable(&self) -> Result<serde_json::Value> {
        self.0.run_mobile_plugin("reportWifiUnusable", ())
    }

    /// 程序化确保 network_avoid_bad_wifi=1（夜间出站切换的系统前提）：
    /// 需 WRITE_SECURE_SETTINGS（用户 adb pm grant 授权）；已=1 幂等返回 changed=false，
    /// ≠1 时插件记录原值快照后写 1。
    /// 返回 {"ensured": bool, "changed"?: bool, "previous"?: i64, "reason"?: String}。
    /// JNI 阻塞调用，调用方须走 spawn_blocking（同 bind_to_wifi）。
    pub fn ensure_avoid_bad_wifi(&self) -> Result<serde_json::Value> {
        self.0.run_mobile_plugin("ensureAvoidBadWifi", ())
    }

    /// 按插件快照还原 network_avoid_bad_wifi（夜间出站切换晨间收尾）。
    /// 返回 {"restored": bool, "previous"?: i64, "reason"?: String}；无快照幂等 restored=false。
    /// JNI 阻塞调用，调用方须走 spawn_blocking（同 bind_to_wifi）。
    pub fn restore_avoid_bad_wifi(&self) -> Result<serde_json::Value> {
        self.0.run_mobile_plugin("restoreAvoidBadWifi", ())
    }

    /// 还原本应用写入的全部系统网络设置（手动出口：captive_portal_mode + avoid_bad_wifi）。
    /// 返回 {"restored": {key: bool}, "reason"?: String}。
    /// JNI 阻塞调用，调用方须走 spawn_blocking（同 bind_to_wifi）。
    pub fn restore_written_settings(&self) -> Result<serde_json::Value> {
        self.0.run_mobile_plugin("restoreWrittenSettings", ())
    }

    /// 查询 Settings.Global 写通道状态（前端引导块显示）。
    /// 返回 {"granted": bool, "avoidBadWifi": i64, "pendingRestoreAvoid": bool,
    /// "pendingRestoreCaptive": bool}。
    /// JNI 阻塞调用，调用方须走 spawn_blocking（同 bind_to_wifi）。
    pub fn get_secure_settings_status(&self) -> Result<serde_json::Value> {
        self.0.run_mobile_plugin("getSecureSettingsStatus", ())
    }
}

/// Extensions to [`tauri::App`], [`tauri::AppHandle`], [`tauri::WebviewWindow`], [`tauri::Webview`] and [`tauri::Window`] to access the network bind APIs.
pub trait CampusNetworkBindExt<R: Runtime> {
    fn campus_network_bind(&self) -> &CampusNetworkBind<R>;
}

impl<R: Runtime, T: Manager<R>> CampusNetworkBindExt<R> for T {
    fn campus_network_bind(&self) -> &CampusNetworkBind<R> {
        self.state::<CampusNetworkBind<R>>().inner()
    }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("campus-network-bind")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "NetworkBindPlugin")?;
            app.manage(CampusNetworkBind(handle));
            Ok(())
        })
        .build()
}
