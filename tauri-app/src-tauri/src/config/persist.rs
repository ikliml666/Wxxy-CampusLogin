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

pub fn list_account_names(app_handle: &tauri::AppHandle) -> Vec<String> {
    let data_dir = get_data_dir(app_handle);
    let accounts_dir = get_accounts_dir(&data_dir);

    if !accounts_dir.exists() {
        return vec![];
    }

    let mut accounts = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&accounts_dir) {
        for entry in entries.flatten() {
            if entry.path().extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(name) = entry.path().file_stem().and_then(|n| n.to_str()) {
                    // 过滤隐藏文件（. 前缀）和空名，与 get_init_data 行为对齐
                    if name.starts_with('.') || name.is_empty() {
                        continue;
                    }
                    accounts.push(name.to_string());
                }
            }
        }
    }

    accounts.sort();
    accounts
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

pub fn save_config_to_disk_encrypted(data_dir: &Path, config: &Config) -> Result<(), String> {
    let mut disk_config = config.clone();
    // 任何非空密码一律 DPAPI 加密落盘。
    // 注意：不得排除 PASSWORD_MASK("***")——若用户真实密码恰为 "***"，
    // 排除判断会使其明文落盘。占位符语义由上层 save_config 负责替换为真实密码，
    // 此处只负责"非空即加密"。
    if !disk_config.password.is_empty() {
        disk_config.password = crypto::encrypt(&disk_config.password)?;
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
}
