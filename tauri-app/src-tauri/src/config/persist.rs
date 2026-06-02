use std::path::PathBuf;
use tauri::Manager;

pub fn atomic_write(path: &std::path::Path, content: &str) -> Result<(), String> {
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, content)
        .map_err(|e| format!("写入临时文件失败: {}", e))?;
    for attempt in 0..3 {
        if std::fs::rename(&tmp_path, path).is_ok() {
            return Ok(());
        }
        if attempt < 2 {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    crate::log_warn!("config", "原子写入重命名失败，保留临时文件: {:?}", tmp_path);
    let _ = std::fs::remove_file(&tmp_path);
    Err(format!("重命名临时文件失败（重试3次后）"))
}

pub fn get_data_dir(app_handle: &tauri::AppHandle) -> PathBuf {
    let tauri_dir = app_handle.path().app_data_dir().unwrap_or_else(|_| {
        dirs::data_dir().unwrap_or_else(|| PathBuf::from("."))
    });

    if !tauri_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&tauri_dir) {
            crate::log_warn!("config", "创建Tauri数据目录失败: {}", e);
        }
    }

    tauri_dir
}

pub fn get_config_path(data_dir: &PathBuf) -> PathBuf {
    data_dir.join("config.json")
}

pub fn get_accounts_dir(data_dir: &PathBuf) -> PathBuf {
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
                    accounts.push(name.to_string());
                }
            }
        }
    }

    accounts.sort();
    accounts
}

pub fn get_login_history_path(data_dir: &PathBuf) -> PathBuf {
    data_dir.join("login-history.json")
}
