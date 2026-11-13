//! 核心对平台能力的全部依赖(同步 JNI 调用)。:core 进程由 `jni_platform` 实现;
//! 这里不出现 tauri 类型,保证核心可以脱离 Activity 运行。

use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use campus_core_logic::status::CoreStatus;

use crate::campus_detect::CampusProbe;
use crate::config_state::{self, CryptoBridge, Settings};

pub trait CorePlatform: Send + Sync {
    fn decrypt(&self, cipher: &str) -> Result<String, String>;
    fn encrypt(&self, plain: &str) -> Result<String, String>;
    fn emit(&self, event: &str, payload: serde_json::Value);
    fn notify_system(&self, title: &str, body: &str, mascot: &str);
    fn update_notification(&self, text: &str);
    fn begin_probe_window(&self);
    fn end_probe_window(&self);
    fn power_state(&self) -> (bool, bool);
    /// VPN 是否处于激活状态（安卓 ConnectivityManager + TRANSPORT_VPN）。
    fn is_vpn_active(&self) -> bool;
    fn start_wifi_watcher(&self) -> Result<(), String>;
    fn stop_wifi_watcher(&self);
    fn stop_service(&self);
    fn bind_to_wifi(&self) -> Result<serde_json::Value, String>;
    fn accept_wifi_network(&self) -> Result<serde_json::Value, String>;
    fn wifi_ssid(&self) -> Option<String>;
    fn report_wifi_unusable(&self) -> Result<serde_json::Value, String>;
    fn ensure_avoid_bad_wifi(&self) -> Result<serde_json::Value, String>;
    fn restore_avoid_bad_wifi(&self) -> Result<serde_json::Value, String>;
    fn read_status(&self) -> CoreStatus;
    fn write_status(&self, status: &CoreStatus);
}

/// 核心上下文:配置读写、跨进程状态、登录历史与平台能力。
#[derive(Clone)]
pub struct CoreCtx {
    data_dir: PathBuf,
    platform: Arc<dyn CorePlatform>,
    cached_source_ip: Arc<Mutex<Option<Ipv4Addr>>>,
    /// 状态文件读改写的进程内互斥:周期巡检、WiFi 事件与停止命令共用
    status_lock: Arc<Mutex<()>>,
}

impl CoreCtx {
    pub fn new(data_dir: PathBuf, platform: Arc<dyn CorePlatform>) -> Self {
        Self {
            data_dir,
            platform,
            cached_source_ip: Arc::new(Mutex::new(None)),
            status_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn platform(&self) -> &dyn CorePlatform {
        self.platform.as_ref()
    }

    /// 共享平台能力的引用计数副本：供需要跨线程长期持有的注入点使用
    /// （如把 VPN 判据注入共享协议的旁路代理门控）。
    pub fn platform_arc(&self) -> Arc<dyn CorePlatform> {
        self.platform.clone()
    }

    pub fn stop_service(&self) {
        self.platform.stop_service();
    }

    fn crypto(&self) -> CryptoBridge {
        let p = self.platform.clone();
        let q = self.platform.clone();
        CryptoBridge {
            encrypt: Arc::new(move |text| p.encrypt(text)),
            decrypt: Arc::new(move |data| q.decrypt(data)),
        }
    }

    /// 读最新配置(每次读盘,不做内存缓存:配置由 UI 进程写,缓存会读到旧值)
    pub async fn settings(&self) -> Result<Settings, String> {
        config_state::load_from(&self.data_dir, &self.crypto()).await
    }

    /// 启动时通过统一事务写口落盘旧 schema 迁移;不在 load_from 内无锁回写
    pub async fn migrate_settings_storage(&self) -> Result<Settings, String> {
        config_state::update_at_dir(&self.data_dir, &self.crypto(), |_| {}).await
    }

    /// 配置写在 IO 锁内 load→mutate→save(core 是后台运行态的唯一写者;
    /// UI 的写口也走同一个锁,见 config_state::update_at_dir)
    pub async fn update_settings<F>(&self, mutate: F) -> Result<Settings, String>
    where
        F: FnOnce(&mut Settings),
    {
        config_state::update_at_dir(&self.data_dir, &self.crypto(), mutate).await
    }

    pub fn emit(&self, event: &str, payload: serde_json::Value) {
        self.platform.emit(event, payload);
    }

    pub fn emit_log(&self, message: &str, log_type: &str) {
        self.emit(
            "login-log",
            serde_json::json!({ "message": message, "type": log_type }),
        );
    }

    pub fn notify_system(&self, enabled: bool, title: &str, body: &str, mascot: &str) {
        if enabled {
            self.platform.notify_system(title, body, mascot);
        }
    }

    fn update_notification_locked(&self, text: &str) {
        self.platform.update_notification(text);
        let mut status = self.platform.read_status();
        status.notification_text = text.to_string();
        status.notification_updated_ms = now_ms();
        self.platform.write_status(&status);
    }

    /// 更新常驻通知并记录新鲜度(状态文件供 UI 判定 stale)
    pub fn update_notification(&self, text: &str) {
        let _guard = self.status_lock.lock().unwrap_or_else(|e| e.into_inner());
        self.update_notification_locked(text);
    }

    /// 仅巡检仍运行时更新;与 stop 共用 status_lock,防止在途拍把“已停止”改回运行中
    #[cfg(mobile)]
    pub fn update_running_notification(&self, text: &str) {
        let _guard = self.status_lock.lock().unwrap_or_else(|e| e.into_inner());
        if !crate::core::monitor::is_running() {
            return;
        }
        if self.platform.read_status().notification_text == text {
            return;
        }
        self.update_notification_locked(text);
    }

    pub fn mark_status<F>(&self, mutate: F)
    where
        F: FnOnce(&mut CoreStatus),
    {
        let _guard = self.status_lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut status = self.platform.read_status();
        mutate(&mut status);
        self.platform.write_status(&status);
    }


    pub fn begin_probe_window(&self) {
        self.platform.begin_probe_window();
    }

    pub fn end_probe_window(&self) {
        self.platform.end_probe_window();
    }

    pub fn power_state(&self) -> (bool, bool) {
        self.platform.power_state()
    }

    pub fn cache_source_ip(&self, source: Option<Ipv4Addr>) {
        if let Ok(mut cached) = self.cached_source_ip.lock() {
            *cached = source;
        }
    }

    /// 登录/注销要用的源 IPv4:按当前接口实时算,再回写缓存。
    /// 缓存只在探测那一刻成立,WiFi 换网后会滞留旧网段地址——拿它绑定本地
    /// 地址时请求发不出去,要等满连接超时才报 `error sending request`。
    pub fn cached_source_ip(&self) -> Option<String> {
        let current = crate::campus_detect::current_source_ip();
        self.cache_source_ip(current);
        current.map(|ip| ip.to_string())
    }

    /// 绑定本进程(实际发起登录的 :core 进程)网络到 WiFi;JNI 阻塞调用走 spawn_blocking
    pub async fn bind_to_wifi(&self) -> Result<serde_json::Value, String> {
        let p = self.platform.clone();
        tokio::task::spawn_blocking(move || p.bind_to_wifi())
            .await
            .map_err(|e| format!("WiFi 绑定任务异常: {e}"))?
    }

    async fn accept_wifi_network(&self) -> Result<serde_json::Value, String> {
        let p = self.platform.clone();
        tokio::task::spawn_blocking(move || p.accept_wifi_network())
            .await
            .map_err(|e| format!("WiFi 接受任务异常: {e}"))?
    }

    pub async fn wifi_ssid(&self) -> Option<String> {
        let p = self.platform.clone();
        tokio::task::spawn_blocking(move || p.wifi_ssid())
            .await
            .ok()
            .flatten()
    }

    /// 登录/注销/探测前确保本(:core)进程已绑 WiFi:绑定成功后清空协议侧连接池
    /// (既有 keep-alive 连接的 fwmark 是绑前打的),再让系统接受未认证校园 WiFi。
    pub async fn ensure_wifi_bound(&self) {
        if campus_login_lib::network::bound_socket::bind_capability()
            == campus_login_lib::network::bound_socket::BindCapability::Supported
        {
            return;
        }
        match self.bind_to_wifi().await {
            Ok(v) if v["bound"].as_bool().unwrap_or(false) => {
                if v["path"].as_str() != Some("already_bound") {
                    campus_login_lib::network::client::clear_client_pool();
                }
                campus_login_lib::log_info!(
                    "wifi-bind",
                    "core 进程网络已绑定 WiFi（path={}, wifi={}）",
                    v["path"].as_str().unwrap_or("?"),
                    v["reason"].as_str().unwrap_or("?")
                );
                self.accept_campus_wifi().await;
            }
            Ok(v) => campus_login_lib::log_warn!(
                "wifi-bind",
                "core 未绑定 WiFi（reason={}），本次回落系统默认路由",
                v["reason"].as_str().unwrap_or("unknown")
            ),
            Err(e) => {
                campus_login_lib::log_warn!("wifi-bind", "core WiFi 绑定调用失败（回落默认路由）: {e}")
            }
        }
    }

    pub async fn accept_campus_wifi(&self) {
        match self.accept_wifi_network().await {
            Ok(v) if v["accepted"].as_bool().unwrap_or(false) => {
                campus_login_lib::log_info!("wifi-accept", "系统已接受该 WiFi")
            }
            Ok(_) => campus_login_lib::log_warn!("wifi-accept", "未能让系统接受该 WiFi"),
            Err(e) => campus_login_lib::log_warn!("wifi-accept", "网络接受调用失败: {e}"),
        }
    }

    pub async fn probe_campus_with_ssid(&self, settings: &Settings) -> (Result<CampusProbe, String>, Option<String>) {
        let mut probe = crate::campus_detect::probe_campus(&settings.campus_gateway, &settings.portal_url).await;
        let mut ssid = None;
        if settings.enable_network_name_check {
            ssid = self.wifi_ssid().await;
            if let (Ok(p), Some(s)) = (probe.as_mut(), ssid.as_deref()) {
                if crate::campus_detect::ssid_matches(s, &settings.required_network_name) {
                    p.on_campus = true;
                }
            }
        }
        (probe, ssid)
    }

    pub async fn report_wifi_unusable(&self) {
        let p = self.platform.clone();
        match tokio::task::spawn_blocking(move || p.report_wifi_unusable()).await {
            Ok(Ok(v)) if v["reported"].as_bool().unwrap_or(false) => campus_login_lib::log_info!(
                "monitor",
                "夜间出站切换: 已触发系统 WiFi 重检 {v}"
            ),
            Ok(Ok(v)) => campus_login_lib::log_warn!(
                "monitor",
                "夜间出站切换: 触发系统 WiFi 重检未生效({})",
                v["reason"].as_str().unwrap_or("未知原因")
            ),
            Ok(Err(e)) => campus_login_lib::log_warn!("monitor", "触发 WiFi 重检失败: {e}"),
            Err(e) => campus_login_lib::log_warn!("monitor", "WiFi 重检任务异常: {e}"),
        }
    }

    pub async fn ensure_avoid_bad_wifi(&self) {
        let p = self.platform.clone();
        match tokio::task::spawn_blocking(move || p.ensure_avoid_bad_wifi()).await {
            Ok(Ok(v)) if v["ensured"].as_bool().unwrap_or(false) => {
                campus_login_lib::log_info!(
                    "monitor",
                    "夜间出站切换: network_avoid_bad_wifi 已确保(changed={})",
                    v["changed"].as_bool().unwrap_or(false)
                );
            }
            Ok(Ok(v)) => campus_login_lib::log_warn!(
                "monitor",
                "夜间出站切换: 无法开启 network_avoid_bad_wifi({})",
                v["reason"].as_str().unwrap_or("未知原因")
            ),
            Ok(Err(e)) => campus_login_lib::log_warn!("monitor", "ensure_avoid_bad_wifi 失败: {e}"),
            Err(e) => campus_login_lib::log_warn!("monitor", "ensure_avoid_bad_wifi 任务异常: {e}"),
        }
    }

    pub async fn restore_avoid_bad_wifi(&self) {
        let p = self.platform.clone();
        match tokio::task::spawn_blocking(move || p.restore_avoid_bad_wifi()).await {
            Ok(Ok(v)) if v["restored"].as_bool().unwrap_or(false) => campus_login_lib::log_info!(
                "monitor",
                "夜间出站切换: network_avoid_bad_wifi 已还原(原值 {})",
                v["previous"].as_i64().unwrap_or(-1)
            ),
            Ok(_) => {}
            Err(e) => campus_login_lib::log_warn!("monitor", "restore_avoid_bad_wifi 任务异常: {e}"),
        }
    }

}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
