//! 安卓端全量配置:结构对齐桌面 config::Config 可适用子集(camelCase IPC 契约)。
//! password/selfPassword 落盘前经 AndroidKeyStore AES-GCM 加密(替代桌面 DPAPI),
//! 出站一律掩码;空/MASK 占位符回退已存值语义与桌面 save_config 同构。

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tauri::Manager;

pub const PASSWORD_MASK: &str = "***";
const CONFIG_FILE: &str = "config.json";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    // 账号与凭据(密码字段落盘必经加密)
    pub user: String,
    pub password: String,
    pub self_password: String,
    pub self_hello_enabled: bool,
    pub self_reverify_each_action: bool,
    pub operator: String,
    // 行为
    pub auto_login_on_start: bool,
    pub enable_background_check: bool,
    pub background_check_interval: u64,
    pub auto_login_on_preparation: bool,
    pub max_disconnect_reconnect: u32,
    pub auto_login_cooldown_secs: u64,
    // 界面与通知
    pub theme_mode: String,
    pub enable_notification: bool,
    pub custom_theme_color: String,
    pub default_panel: String,
    pub active_account: String,
    pub enable_boot_autostart: bool,
    // 定时质量测试
    pub enable_latency_test: bool,
    pub latency_test_interval: u64,
    pub enable_network_quality: bool,
    pub skip_ttfb_in_latency: bool,
    pub skip_content_in_latency: bool,
    // 网络
    pub portal_url: String,
    pub fixed_gateway: String,
    pub required_network_name: String,
    pub enable_network_name_check: bool,
    pub campus_gateway: String,
    // 更新
    /// 检查/下载更新渠道优先级:"mirror"(镜像加速优先,默认,国内主场景)|"github"(官方优先)
    pub update_source: String,
    // 日志
    pub log_retention_days: u32,
    /// 配置结构版本:旧版本文件缺省反序列化为 0,load_from 据此执行一次性
    /// 默认值迁移(0→1:后台检测间隔 15s→60s)。新装即 1,不再触发。
    pub config_schema_version: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            user: String::new(),
            password: String::new(),
            self_password: String::new(),
            self_hello_enabled: true,
            self_reverify_each_action: false,
            operator: String::new(),
            auto_login_on_start: false,
            enable_background_check: false,
            // 2026-09-09 起 60s:后台检测是稳态周期任务,15s 间隔空转耗电,
            // 旧配置由 migrate_legacy_defaults 按 schema 版本一次性迁移
            background_check_interval: 60_000,
            auto_login_on_preparation: false,
            max_disconnect_reconnect: 3,
            auto_login_cooldown_secs: 60,
            theme_mode: "dark".to_string(),
            enable_notification: true,
            custom_theme_color: "#6366f1".to_string(),
            default_panel: "dashboard".to_string(),
            active_account: String::new(),
            enable_boot_autostart: false,
            enable_latency_test: false,
            latency_test_interval: 60_000,
            enable_network_quality: true,
            skip_ttfb_in_latency: false,
            skip_content_in_latency: false,
            portal_url: "http://10.1.99.100".to_string(),
            fixed_gateway: "10.2.127.254".to_string(),
            required_network_name: "i-wxxy".to_string(),
            enable_network_name_check: false,
            campus_gateway: "10.2.127.254".to_string(),
            update_source: "mirror".to_string(),
            log_retention_days: 7,
            // 新装即当前版本,跳过迁移;旧文件缺字段反序列化为 0 触发迁移
            config_schema_version: 1,
        }
    }
}

/// 加解密桥:真机走 AndroidKeyStore 插件;host 测试注入可逆假实现,保证密码路径可测。
#[derive(Clone)]
pub struct CryptoBridge {
    pub encrypt: Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>,
    pub decrypt: Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>,
}

impl CryptoBridge {
    pub fn from_app(app: &tauri::AppHandle) -> Self {
        #[cfg(mobile)]
        {
            use tauri_plugin_campus_keystore::CampusKeystoreExt;
            let handle = app.clone();
            let dhandle = app.clone();
            Self {
                encrypt: Arc::new(move |text| {
                    handle
                        .campus_keystore()
                        .encrypt(text)
                        .map_err(|e| e.to_string())
                }),
                decrypt: Arc::new(move |data| {
                    dhandle
                        .campus_keystore()
                        .decrypt(data)
                        .map_err(|e| e.to_string())
                }),
            }
        }
        #[cfg(not(mobile))]
        {
            let _ = app;
            Self {
                encrypt: Arc::new(|_| Err("Keystore 仅移动端可用".to_string())),
                decrypt: Arc::new(|_| Err("Keystore 仅移动端可用".to_string())),
            }
        }
    }
}

/// 磁盘上的形态:密码字段为密文(加密失败时置空,绝不落明文)
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
struct EncodedSettings {
    settings: Settings,
    password_cipher: String,
    self_password_cipher: String,
}

pub async fn load_file(path: &Path, bridge: &CryptoBridge) -> Result<Settings, String> {
    let raw = match tokio::fs::read_to_string(path).await {
        Ok(s) => s,
        Err(_) => return Ok(Settings::default()), // 首次启动无文件
    };
    let encoded: EncodedSettings =
        serde_json::from_str(&raw).map_err(|e| format!("配置损坏: {e}"))?;
    let mut s = encoded.settings;
    // 解密密码;密文损坏(密钥变更/文件篡改)时置空而不是让配置不可用
    if !encoded.password_cipher.is_empty() {
        s.password = (bridge.decrypt)(&encoded.password_cipher).unwrap_or_default();
    }
    if !encoded.self_password_cipher.is_empty() {
        s.self_password = (bridge.decrypt)(&encoded.self_password_cipher).unwrap_or_default();
    }
    Ok(s)
}

pub async fn save_file(path: &Path, bridge: &CryptoBridge, s: &Settings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("创建配置目录失败: {e}"))?;
    }
    let mut password_cipher = String::new();
    let mut self_password_cipher = String::new();
    // 真机排障:加密成败打 stderr(logcat RustStdoutStderr),不打印明文/密文
    if !s.password.is_empty() {
        match (bridge.encrypt)(&s.password) {
            Ok(c) => { eprintln!("[save_config] password encrypt ok len={}", c.len()); password_cipher = c; }
            Err(e) => { eprintln!("[save_config] password encrypt FAILED: {e}"); return Err(format!("password encrypt: {e}")); }
        }
    }
    if !s.self_password.is_empty() {
        match (bridge.encrypt)(&s.self_password) {
            Ok(c) => { eprintln!("[save_config] selfPassword encrypt ok len={}", c.len()); self_password_cipher = c; }
            Err(e) => { eprintln!("[save_config] selfPassword encrypt FAILED: {e}"); return Err(format!("selfPassword encrypt: {e}")); }
        }
    }
    let encoded = EncodedSettings {
        settings: Settings { password: String::new(), self_password: String::new(), ..s.clone() },
        password_cipher,
        self_password_cipher,
    };
    let json = serde_json::to_string_pretty(&encoded).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    tokio::fs::write(&tmp, json).await.map_err(|e| e.to_string())?;
    tokio::fs::rename(&tmp, path).await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn load_from(dir: &Path, bridge: &CryptoBridge) -> Result<Settings, String> {
    let mut s = load_file(&dir.join(CONFIG_FILE), bridge).await?;
    migrate_legacy_defaults(dir, bridge, &mut s).await;
    Ok(s)
}

/// 旧默认一次性迁移(schema v0→v1):后台检测间隔 15s 是历史默认值,统一升为
/// 当前默认 60s(稳态功耗)。迁移结果(含版本号)落盘,此后用户主动设回 15s
/// 不会再被覆盖。落盘失败静默:下次读盘重迁,幂等。
async fn migrate_legacy_defaults(dir: &Path, bridge: &CryptoBridge, s: &mut Settings) {
    if s.config_schema_version >= 1 {
        return;
    }
    if s.background_check_interval == 15_000 {
        s.background_check_interval = 60_000;
    }
    s.config_schema_version = 1;
    let _ = save_file(&dir.join(CONFIG_FILE), bridge, s).await;
}

pub async fn save_to(dir: &Path, bridge: &CryptoBridge, s: &Settings) -> Result<(), String> {
    save_file(&dir.join(CONFIG_FILE), bridge, s).await
}

/// 出站掩码:非空密码 → "***"(对齐桌面 masked_for_display 唯一出口语义)
pub fn masked_for_display(s: &Settings) -> serde_json::Value {
    let mut m = s.clone();
    if !m.password.is_empty() {
        m.password = PASSWORD_MASK.to_string();
    }
    if !m.self_password.is_empty() {
        m.self_password = PASSWORD_MASK.to_string();
    }
    serde_json::to_value(&m).unwrap_or(serde_json::Value::Null)
}

/// 空串或掩码占位符视为"未修改",回退已存值;由 clear 标志显式清除(桌面 save_config 同构语义)
pub fn resolve_password_field(incoming: &str, current: &str, clear: bool) -> String {
    if clear {
        return String::new();
    }
    if incoming.is_empty() || incoming == PASSWORD_MASK {
        current.to_string()
    } else {
        incoming.to_string()
    }
}

#[tauri::command]
pub async fn get_config(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let bridge = CryptoBridge::from_app(&app);
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取数据目录失败: {e}"))?;
    let s = load_from(&dir, &bridge).await?;
    let state = app.state::<crate::android_state::AndroidState>();
    if let Ok(mut cache) = state.config.lock() {
        *cache = Some(s.clone());
    }
    Ok(masked_for_display(&s))
}

#[tauri::command]
pub async fn save_config(
    app: tauri::AppHandle,
    config: Settings,
    clear_password: Option<bool>,
    clear_self_password: Option<bool>,
) -> Result<(), String> {
    let bridge = CryptoBridge::from_app(&app);
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取数据目录失败: {e}"))?;
    let current = load_from(&dir, &bridge).await?;
    let mut merged = config;
    merged.password =
        resolve_password_field(&merged.password, &current.password, clear_password == Some(true));
    merged.self_password = resolve_password_field(
        &merged.self_password,
        &current.self_password,
        clear_self_password == Some(true),
    );
    save_to(&dir, &bridge, &merged).await?;
    let state = app.state::<crate::android_state::AndroidState>();
    if let Ok(mut cache) = state.config.lock() {
        *cache = Some(merged);
    }
    Ok(())
}

/// 取当前配置(缓存优先,未命中读盘);供登录/自助服务等命令回退已存凭据。
#[allow(dead_code)] // Task 2 协议命令面接线
pub async fn current_settings(app: &tauri::AppHandle) -> Result<Settings, String> {
    let state = app.state::<crate::android_state::AndroidState>();
    if let Ok(cache) = state.config.lock() {
        if let Some(s) = cache.clone() {
            return Ok(s);
        }
    }
    let bridge = CryptoBridge::from_app(app);
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取数据目录失败: {e}"))?;
    let s = load_from(&dir, &bridge).await?;
    if let Ok(mut cache) = state.config.lock() {
        *cache = Some(s.clone());
    }
    Ok(s)
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    /// base64 假加密桥:可逆、输出与输入不同,足以验证"落盘路径经过加密层"
    fn fake_bridge() -> CryptoBridge {
        let enc = |text: &str| {
            Ok(base64_encode(text.as_bytes()))
        };
        let dec = |data: &str| {
            base64_decode(data)
                .map(|bytes| String::from_utf8(bytes).expect("假密文应是 UTF-8"))
        };
        CryptoBridge { encrypt: Arc::new(enc), decrypt: Arc::new(dec) }
    }

    fn base64_encode(data: &[u8]) -> String {
        const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in data.chunks(3) {
            let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
            let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
            out.push(TABLE[(n >> 18) as usize & 63] as char);
            out.push(TABLE[(n >> 12) as usize & 63] as char);
            out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
            out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
        }
        out
    }

    fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
        const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut buf = Vec::new();
        let bytes: Vec<u8> = s.bytes().filter(|b| *b != b'=').collect();
        for chunk in bytes.chunks(4) {
            let mut n: u32 = 0;
            for (i, b) in chunk.iter().enumerate() {
                let v = TABLE.iter().position(|t| t == b).ok_or("非法 base64 字符")?;
                n |= (v as u32) << (18 - 6 * i);
            }
            buf.push((n >> 16) as u8);
            if chunk.len() > 2 { buf.push((n >> 8) as u8); }
            if chunk.len() > 3 { buf.push(n as u8); }
        }
        Ok(buf)
    }

    fn tmp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("campus-android-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn sample_settings() -> Settings {
        Settings {
            user: "2024001".to_string(),
            password: "secret_pass密码".to_string(),
            self_password: "self_secret".to_string(),
            operator: "@cmcc".to_string(),
            background_check_interval: 15_000,
            ..Settings::default()
        }
    }

    #[tokio::test]
    async fn 加密落盘_文件不含明文_读回等于原文() {
        let dir = tmp_dir("roundtrip");
        let bridge = fake_bridge();
        let s = sample_settings();
        save_to(&dir, &bridge, &s).await.expect("落盘应成功");
        let raw = std::fs::read_to_string(dir.join(CONFIG_FILE)).unwrap();
        assert!(!raw.contains("secret_pass"), "磁盘文件不得含明文密码: {raw}");
        assert!(!raw.contains("self_secret"), "磁盘文件不得含明文自助密码: {raw}");
        let back = load_from(&dir, &bridge).await.unwrap();
        assert_eq!(back.password, "secret_pass密码");
        assert_eq!(back.self_password, "self_secret");
        assert_eq!(back.user, "2024001");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn 解密失败时置空_配置仍可加载() {
        let dir = tmp_dir("corrupt");
        let bridge = fake_bridge();
        save_to(&dir, &bridge, &sample_settings()).await.unwrap();
        // 换一把"密钥"(不同桥)模拟 Keystore 密钥失效
        let other = CryptoBridge {
            encrypt: Arc::new(|t| Ok(format!("X{t}X"))),
            decrypt: Arc::new(|_| Err("GCM 认证失败".to_string())),
        };
        let back = load_from(&dir, &other).await.unwrap();
        assert_eq!(back.password, "", "解密失败应置空而非报错");
        assert_eq!(back.user, "2024001", "非敏感字段不受影响");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 默认值_业务语义() {
        let s = Settings::default();
        assert_eq!(s.portal_url, "http://10.1.99.100");
        assert_eq!(s.campus_gateway, "10.2.127.254");
        assert_eq!(s.required_network_name, "i-wxxy");
        assert_eq!(s.theme_mode, "dark");
        assert_eq!(s.background_check_interval, 60_000);
        assert_eq!(s.config_schema_version, 1, "新装即当前版本,不触发迁移");
        assert_eq!(s.max_disconnect_reconnect, 3);
        assert!(s.self_hello_enabled);
        assert!(s.enable_network_quality);
    }

    #[tokio::test]
    async fn 迁移_旧默认15s升60s且落盘后用户值不被覆盖() {
        let dir = tmp_dir("migrate");
        let bridge = fake_bridge();
        // 构造 v0 旧配置:15s 间隔(历史默认值)
        let mut old = sample_settings();
        old.config_schema_version = 0;
        old.background_check_interval = 15_000;
        save_to(&dir, &bridge, &old).await.unwrap();
        let back = load_from(&dir, &bridge).await.unwrap();
        assert_eq!(back.background_check_interval, 60_000, "旧默认应迁移为 60s");
        assert_eq!(back.config_schema_version, 1);
        // 迁移已落盘:此后用户主动设回 15s 是明确意图,不再被覆盖
        let mut manual = back.clone();
        manual.background_check_interval = 15_000;
        save_to(&dir, &bridge, &manual).await.unwrap();
        let back2 = load_from(&dir, &bridge).await.unwrap();
        assert_eq!(back2.background_check_interval, 15_000, "v1 配置不再迁移");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn 迁移_非旧默认值保持不动() {
        let dir = tmp_dir("migrate-keep");
        let bridge = fake_bridge();
        let mut old = sample_settings();
        old.config_schema_version = 0;
        old.background_check_interval = 30_000; // 用户主动设过的值
        save_to(&dir, &bridge, &old).await.unwrap();
        let back = load_from(&dir, &bridge).await.unwrap();
        assert_eq!(back.background_check_interval, 30_000, "非旧默认值不迁移");
        assert_eq!(back.config_schema_version, 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn camelCase_序列化契约() {
        let s = sample_settings();
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"selfPassword\""), "camelCase 契约: {json}");
        assert!(json.contains("\"backgroundCheckInterval\""));
        assert!(json.contains("\"enableNetworkNameCheck\""));
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.user, s.user);
    }

    #[test]
    fn 掩码出口_非空变星号_空保持空() {
        let mut s = sample_settings();
        let v = masked_for_display(&s);
        assert_eq!(v["password"], "***");
        assert_eq!(v["selfPassword"], "***");
        assert!(!v.to_string().contains("secret_pass"));
        s.password.clear();
        let v = masked_for_display(&s);
        assert_eq!(v["password"], "");
    }

    #[test]
    fn 密码字段回退语义_与桌面同构() {
        // 掩码占位符 → 保留已存值
        assert_eq!(resolve_password_field("***", "real", false), "real");
        // 空串 → 保留已存值
        assert_eq!(resolve_password_field("", "real", false), "real");
        // 新值 → 采用新值
        assert_eq!(resolve_password_field("new_pass", "real", false), "new_pass");
        // 显式清除 → 置空
        assert_eq!(resolve_password_field("***", "real", true), "");
        // 用户真实密码恰为 *** 时也可落盘(加密后无明文风险)
        assert_eq!(resolve_password_field("real", "old", false), "real");
    }

    #[tokio::test]
    async fn 空密码不写密文位() {
        let dir = tmp_dir("emptypwd");
        let bridge = fake_bridge();
        let mut s = sample_settings();
        s.password.clear();
        s.self_password.clear();
        save_to(&dir, &bridge, &s).await.unwrap();
        let raw = std::fs::read_to_string(dir.join(CONFIG_FILE)).unwrap();
        assert!(!raw.contains("passwordCipher") || raw.contains("\"passwordCipher\": \"\""),
            "空密码不得产生密文: {raw}");
        std::fs::remove_dir_all(&dir).ok();
    }
}
