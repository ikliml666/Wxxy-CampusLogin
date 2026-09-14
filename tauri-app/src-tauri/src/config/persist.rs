use std::io::Write;
use std::path::{Path, PathBuf};
use parking_lot::Mutex;
use tauri::Manager;
use crate::config::model::Config;
use crate::account::crypto;

// 登录历史的读-改-写全程互斥：自动登录与手动登录并发追加时，
// 无锁的后写者会用旧快照覆盖先写者丢历史。锁内不调用本函数（无重入）。
static LOGIN_HISTORY_LOCK: Mutex<()> = Mutex::new(());

pub fn atomic_write(path: &std::path::Path, content: &str) -> Result<(), String> {
    // 临时文件名带纳秒时间戳，避免并发保存时互相覆盖
    let tmp_path = path.with_extension(format!(
        "json.tmp.{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    // 用 File 句柄写入并 sync_all：断电时保证临时文件内容完整落盘，
    // 避免半截文件被 rename 成正式配置（std::fs::write 无此保证）
    let tmp_file = std::fs::File::create(&tmp_path)
        .map_err(|e| format!("创建临时文件失败: {e}"))?;
    {
        let mut writer = std::io::BufWriter::new(&tmp_file);
        writer.write_all(content.as_bytes())
            .map_err(|e| format!("写入临时文件失败: {e}"))?;
        writer.flush().map_err(|e| format!("写入临时文件失败: {e}"))?;
    }
    tmp_file.sync_all().map_err(|e| format!("刷盘临时文件失败: {e}"))?;
    for attempt in 0..3 {
        if std::fs::rename(&tmp_path, path).is_ok() {
            return Ok(());
        }
        if attempt < 2 {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    crate::log_warn!("config", "原子写入重命名失败，已清理临时文件: {:?}", tmp_path);
    let _ = std::fs::remove_file(&tmp_path);
    Err("重命名临时文件失败（重试3次后）".to_string())
}

pub fn get_data_dir(app_handle: &tauri::AppHandle) -> PathBuf {
    let tauri_dir = app_handle.path().app_data_dir().unwrap_or_else(|_| {
        // 极端回退也落在应用子目录，避免与其他应用共享的根数据目录污染
        dirs::data_dir().map(|d| d.join("campus-login")).unwrap_or_else(|| PathBuf::from("."))
    });

    if !tauri_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&tauri_dir) {
            crate::log_warn!("config", "创建Tauri数据目录失败: {}", e);
        }
    }

    tauri_dir
}

pub fn get_config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("config.json")
}

pub fn get_accounts_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("accounts")
}

/// 账号档案文件路径（<data_dir>/accounts/<id>.json，id 为文件名 stem）
pub fn get_account_path(data_dir: &Path, account_id: &str) -> PathBuf {
    get_accounts_dir(data_dir).join(format!("{account_id}.json"))
}

/// 读取账号档案（扁平 Config JSON），密码解密为明文返回。
/// 文件不存在 / 读盘失败 / 解析失败 / 解密失败均返回 Err（调用方按需区分文案）。
pub fn load_account_config(data_dir: &Path, account_id: &str) -> Result<Config, String> {
    let account_path = get_account_path(data_dir, account_id);
    if !account_path.exists() {
        return Err("账号不存在".to_string());
    }
    let content = std::fs::read_to_string(&account_path)
        .map_err(|e| format!("读取账号配置失败: {e}"))?;
    let mut config: Config = serde_json::from_str(&content)
        .map_err(|e| format!("解析账号配置失败: {e}"))?;

    if !config.password.is_empty() {
        match crypto::decrypt(&config.password) {
            Ok(decrypted) => config.password = decrypted,
            Err(e) => {
                crate::log_error!("account", "账号密码解密失败: {}", e);
                return Err("账号密码解密失败".to_string());
            }
        }
    }

    Ok(config)
}

/// 写入账号档案（扁平 Config JSON）：非空密码 DPAPI 加密后落盘（与主配置
/// save_config_to_disk_encrypted 同一加密约定；self_password 沿用既有账号文件
/// 行为透传，见 list_account_items / load_account_config 的既有语义）。
pub fn save_account_config(data_dir: &Path, account_id: &str, config: &Config) -> Result<(), String> {
    let accounts_dir = get_accounts_dir(data_dir);
    std::fs::create_dir_all(&accounts_dir).map_err(|e| format!("创建账号目录失败: {e}"))?;
    let mut disk_config = config.clone();
    if !disk_config.password.is_empty() {
        disk_config.password = crypto::encrypt(&disk_config.password)
            .map_err(|e| format!("密码加密失败: {e}"))?;
    }
    let json = serde_json::to_string_pretty(&disk_config)
        .map_err(|e| format!("序列化账号配置失败: {e}"))?;
    atomic_write(&get_account_path(data_dir, account_id), &json)
}

/// 枚举账号档案为 AccountItem（id + display_name）。
/// - 过滤：`.` 前缀与空名跳过（与旧 list_account_names 一致），按 id 排序；
/// - display_name：为空（含纯空白）兜底为 id；文件读取/解析失败同样兜底为 id 并 log_warn。
pub fn list_account_items(data_dir: &Path) -> Vec<crate::infra::state::AccountItem> {
    let accounts_dir = get_accounts_dir(data_dir);
    if !accounts_dir.exists() {
        return vec![];
    }

    let mut items = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&accounts_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|n| n.to_str()) else {
                continue;
            };
            // 过滤隐藏文件（. 前缀）和空名，与 get_init_data 行为对齐
            if id.starts_with('.') || id.is_empty() {
                continue;
            }
            let display_name = match std::fs::read_to_string(entry.path())
                .map_err(|e| e.to_string())
                .and_then(|c| serde_json::from_str::<Config>(&c).map_err(|e| e.to_string()))
            {
                Ok(config) if !config.display_name.trim().is_empty() => config.display_name,
                Ok(_) => id.to_string(),
                Err(e) => {
                    crate::log_warn!("account", "读取账号文件失败，显示名兜底为 id: {} ({})", id, e);
                    id.to_string()
                }
            };
            items.push(crate::infra::state::AccountItem { id: id.to_string(), display_name });
        }
    }

    items.sort_by(|a, b| a.id.cmp(&b.id));
    items
}

pub fn get_login_history_path(data_dir: &Path) -> PathBuf {
    data_dir.join("login-history.json")
}

pub fn append_login_history(app_handle: &tauri::AppHandle, success: bool, message: &str, adapter: &str, user: &str, login_type: &str) -> Result<(), String> {
    let _guard = LOGIN_HISTORY_LOCK.lock();
    let data_dir = get_data_dir(app_handle);
    let history_path = get_login_history_path(&data_dir);
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("创建数据目录失败: {e}"))?;

    let mut history: Vec<serde_json::Value> = if history_path.exists() {
        let content = match std::fs::read_to_string(&history_path) {
            Ok(c) => c,
            Err(e) => {
                crate::log_warn!("system", "读取登录历史失败，备份后重置: {}", e);
                let _ = std::fs::rename(&history_path, format!("{}.bak", history_path.display()));
                String::new()
            }
        };
        if content.is_empty() {
            vec![]
        } else {
            match serde_json::from_str(&content) {
                Ok(v) => v,
                Err(e) => {
                    crate::log_warn!("system", "解析登录历史失败，备份后重置: {}", e);
                    let _ = std::fs::rename(&history_path, format!("{}.bak", history_path.display()));
                    vec![]
                }
            }
        }
    } else {
        vec![]
    };

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    history.insert(0, serde_json::json!({
        "time": now,
        "success": success,
        "message": message,
        "adapter": adapter,
        "user": user,
        "type": login_type
    }));

    if history.len() > 100 {
        history.truncate(100);
    }

    let json = serde_json::to_string_pretty(&history)
        .map_err(|e| format!("序列化登录历史失败: {e}"))?;

    atomic_write(&history_path, &json)?;

    Ok(())
}

// 质量历史的读-改-写全程互斥（理由同 LOGIN_HISTORY_LOCK）。
static QUALITY_HISTORY_LOCK: Mutex<()> = Mutex::new(());

pub fn get_quality_history_path(data_dir: &Path) -> PathBuf {
    data_dir.join("quality_history.json")
}

/// 追加一条网络质量历史（头插法，上限 100，损坏备份后重置；与 append_login_history 同款）。
/// 延迟 <0（检测失败/未执行，源数据为 -1）落盘为 null，区分"未测"与 0ms。
pub fn append_quality_history(app_handle: &tauri::AppHandle, result: &crate::network::quality::NetworkQualityResult) -> Result<(), String> {
    append_quality_history_to(&get_data_dir(app_handle), result)
}

pub fn append_quality_history_to(data_dir: &Path, result: &crate::network::quality::NetworkQualityResult) -> Result<(), String> {
    let _guard = QUALITY_HISTORY_LOCK.lock();
    let history_path = get_quality_history_path(data_dir);
    std::fs::create_dir_all(data_dir).map_err(|e| format!("创建数据目录失败: {e}"))?;

    let mut history: Vec<serde_json::Value> = if history_path.exists() {
        let content = match std::fs::read_to_string(&history_path) {
            Ok(c) => c,
            Err(e) => {
                crate::log_warn!("system", "读取质量历史失败，备份后重置: {}", e);
                let _ = std::fs::rename(&history_path, format!("{}.bak", history_path.display()));
                String::new()
            }
        };
        if content.is_empty() {
            vec![]
        } else {
            match serde_json::from_str(&content) {
                Ok(v) => v,
                Err(e) => {
                    crate::log_warn!("system", "解析质量历史失败，备份后重置: {}", e);
                    let _ = std::fs::rename(&history_path, format!("{}.bak", history_path.display()));
                    vec![]
                }
            }
        }
    } else {
        vec![]
    };

    history.insert(0, serde_json::json!({
        "timestamp": result.timestamp,
        "gatewayLatency": (result.gateway_latency >= 0).then_some(result.gateway_latency),
        "externalLatency": (result.external_latency >= 0).then_some(result.external_latency),
        "quality": result.quality,
    }));

    if history.len() > 100 {
        history.truncate(100);
    }

    let json = serde_json::to_string_pretty(&history)
        .map_err(|e| format!("序列化质量历史失败: {e}"))?;

    atomic_write(&history_path, &json)?;

    Ok(())
}

pub fn save_config_to_disk_encrypted(data_dir: &Path, config: &Config) -> Result<(), String> {
    let mut disk_config = config.clone();
    // 任何非空密码一律 DPAPI 加密落盘。
    // 注意：不得排除 PASSWORD_MASK("***")——若用户真实密码恰为 "***"，
    // 排除判断会使其明文落盘。占位符语义由上层 save_config 负责替换为真实密码，
    // 此处只负责"非空即加密"。
    if !disk_config.password.is_empty() {
        disk_config.password = crypto::encrypt(&disk_config.password)?;
    }
    if !disk_config.self_password.is_empty() {
        disk_config.self_password = crypto::encrypt(&disk_config.self_password)?;
    }
    let config_path = get_config_path(data_dir);
    let json = serde_json::to_string_pretty(&disk_config).map_err(|e| format!("序列化配置失败: {e}"))?;
    atomic_write(&config_path, &json)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_data_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cl_persist_test_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn literal_star_password_is_encrypted_on_disk() {
        // 用户真实密码恰为 PASSWORD_MASK("***") 时必须 DPAPI 加密，
        // 不得因"等于占位符"判断而明文落盘
        let cfg = Config {
            password: "***".to_string(),
            ..Default::default()
        };
        let dir = temp_data_dir("star");
        save_config_to_disk_encrypted(&dir, &cfg).unwrap();
        let raw = std::fs::read_to_string(get_config_path(&dir)).unwrap();
        assert!(
            !raw.contains("\"password\":\"***\""),
            "真实密码 *** 不得明文落盘，磁盘内容: {raw}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_password_stays_empty() {
        let cfg = Config {
            password: String::new(),
            ..Default::default()
        };
        let dir = temp_data_dir("empty");
        save_config_to_disk_encrypted(&dir, &cfg).unwrap();
        let raw = std::fs::read_to_string(get_config_path(&dir)).unwrap();
        assert!(raw.contains("\"password\": \"\""), "空密码应原样落盘");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn quality_result(ts: u64, gw: i64, ext: i64, quality: &str) -> crate::network::quality::NetworkQualityResult {
        crate::network::quality::NetworkQualityResult {
            gateway_latency: gw,
            external_latency: ext,
            average_external_latency: ext,
            gateway: "192.168.1.1".to_string(),
            quality: quality.to_string(),
            timestamp: ts,
            details: serde_json::Value::Object(serde_json::Map::new()),
            metrics: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    #[test]
    fn quality_history_head_insert_and_shape() {
        let dir = temp_data_dir("qh_shape");
        append_quality_history_to(&dir, &quality_result(1000, 12, 45, "good")).unwrap();
        append_quality_history_to(&dir, &quality_result(2000, -1, 30, "poor")).unwrap();
        let raw = std::fs::read_to_string(get_quality_history_path(&dir)).unwrap();
        let entries: Vec<serde_json::Value> = serde_json::from_str(&raw).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["timestamp"], 2000, "最新在头");
        assert_eq!(entries[0]["quality"], "poor");
        assert!(entries[0]["gatewayLatency"].is_null(), "负延迟(-1)应落盘为 null");
        assert_eq!(entries[0]["externalLatency"], 30);
        assert_eq!(entries[1]["gatewayLatency"], 12);
        assert_eq!(entries[1]["externalLatency"], 45);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn quality_history_caps_at_100() {
        let dir = temp_data_dir("qh_cap");
        for i in 0..110 {
            append_quality_history_to(&dir, &quality_result(i as u64, 1, 1, "good")).unwrap();
        }
        let entries: Vec<serde_json::Value> =
            serde_json::from_str(&std::fs::read_to_string(get_quality_history_path(&dir)).unwrap()).unwrap();
        assert_eq!(entries.len(), 100);
        assert_eq!(entries[0]["timestamp"], 109, "最新在头");
        assert_eq!(entries[99]["timestamp"], 10, "最老保留第 10 条");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn quality_history_corrupt_backup_and_reset() {
        let dir = temp_data_dir("qh_corrupt");
        std::fs::write(get_quality_history_path(&dir), "not-json{{{").unwrap();
        append_quality_history_to(&dir, &quality_result(1, 1, 1, "good")).unwrap();
        let entries: Vec<serde_json::Value> =
            serde_json::from_str(&std::fs::read_to_string(get_quality_history_path(&dir)).unwrap()).unwrap();
        assert_eq!(entries.len(), 1, "损坏文件备份后从空重写");
        let baks: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".bak"))
            .collect();
        assert_eq!(baks.len(), 1, "损坏文件应被备份");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 账号目录不存在时返回空列表（不 panic、不创建目录）
    #[test]
    fn list_account_items_missing_dir_is_empty() {
        let dir = temp_data_dir("acc_missing");
        assert!(list_account_items(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// display_name 兜底规则：空 → id；读取/解析失败 → id；按 id 排序；
    /// `.` 前缀与非 json 文件过滤
    #[test]
    fn list_account_items_fallback_and_filter() {
        use crate::infra::state::AccountItem;
        let dir = temp_data_dir("acc_list");
        let accounts_dir = get_accounts_dir(&dir);
        std::fs::create_dir_all(&accounts_dir).unwrap();

        // 有显示名
        let mut cfg_a = Config::default();
        cfg_a.display_name = "我的甲".to_string();
        std::fs::write(accounts_dir.join("acc-a.json"), serde_json::to_string_pretty(&cfg_a).unwrap()).unwrap();
        // 显示名为空 → 兜底为 id
        let cfg_b = Config::default();
        std::fs::write(accounts_dir.join("acc-b.json"), serde_json::to_string(&cfg_b).unwrap()).unwrap();
        // 非法 JSON → 兜底为 id
        std::fs::write(accounts_dir.join("acc-c.json"), "not-json{{{").unwrap();
        // 隐藏文件与非 json 文件过滤
        std::fs::write(accounts_dir.join(".hidden.json"), "{}").unwrap();
        std::fs::write(accounts_dir.join("acc-d.txt"), "{}").unwrap();

        let items = list_account_items(&dir);
        assert_eq!(items, vec![
            AccountItem { id: "acc-a".to_string(), display_name: "我的甲".to_string() },
            AccountItem { id: "acc-b".to_string(), display_name: "acc-b".to_string() },
            AccountItem { id: "acc-c".to_string(), display_name: "acc-c".to_string() },
        ], "应按 id 排序、空显示名与损坏文件兜底为 id、过滤隐藏/非json");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 账号档案读写往返：密码落盘加密、读回解密还原；空密码原样
    #[cfg(target_os = "windows")]
    #[test]
    fn account_config_roundtrip_encrypts_password() {
        let dir = temp_data_dir("acc_roundtrip");
        let mut cfg = Config::default();
        cfg.user = "u1".to_string();
        cfg.password = "secret-pwd".to_string();
        cfg.display_name = "显示名".to_string();
        save_account_config(&dir, "acc-rt", &cfg).unwrap();

        // 磁盘上不得出现明文密码
        let raw = std::fs::read_to_string(get_account_path(&dir, "acc-rt")).unwrap();
        assert!(!raw.contains("secret-pwd"), "账号文件明文泄露密码: {raw}");

        let loaded = load_account_config(&dir, "acc-rt").unwrap();
        assert_eq!(loaded.user, "u1");
        assert_eq!(loaded.password, "secret-pwd");
        assert_eq!(loaded.display_name, "显示名");

        // 文件不存在 → Err("账号不存在")，不 panic
        assert_eq!(load_account_config(&dir, "no-such").unwrap_err(), "账号不存在");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
