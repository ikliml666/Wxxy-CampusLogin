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
