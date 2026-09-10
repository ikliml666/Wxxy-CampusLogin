//! 登录历史:data_dir/login-history.json,头插法上限 100 条,损坏备份后重置
//! (与桌面 config/persist.rs append_login_history 同构;安卓无多适配器,adapter 固定 wlan0)。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static LOGIN_HISTORY_LOCK: Mutex<()> = Mutex::new(());

pub const LOGIN_HISTORY_MAX: usize = 100;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LoginHistoryEntry {
    pub time: String,
    pub success: bool,
    pub message: String,
    pub adapter: String,
    pub user: String,
    #[serde(rename = "type")]
    pub login_type: String,
}

fn history_path(dir: &Path) -> PathBuf {
    dir.join("login-history.json")
}

/// 读取历史;损坏时备份 .corrupt-*.bak 后重置,不让一条坏记录卡死登录流程。
pub fn read(dir: &Path) -> Vec<LoginHistoryEntry> {
    let path = history_path(dir);
    match std::fs::read(&path) {
        Ok(raw) => match serde_json::from_str(&String::from_utf8_lossy(&raw)) {
            Ok(entries) => entries,
            Err(_) => {
                let bak = dir.join(format!("login-history.json.corrupt-{}.bak", chrono_epoch_millis()));
                let _ = std::fs::rename(&path, &bak);
                Vec::new()
            }
        },
        Err(_) => Vec::new(),
    }
}

fn chrono_epoch_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// 追加一条登录历史(头插法,上限 100),原子写(tmp+rename)。
pub fn append(dir: &Path, success: bool, message: &str, user: &str, login_type: &str) -> Result<(), String> {
    let _guard = LOGIN_HISTORY_LOCK
        .lock()
        .map_err(|_| "登录历史锁损坏".to_string())?;
    let mut entries = read(dir);
    entries.insert(
        0,
        LoginHistoryEntry {
            time: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            success,
            message: message.to_string(),
            adapter: "wlan0".to_string(),
            user: user.to_string(),
            login_type: login_type.to_string(),
        },
    );
    entries.truncate(LOGIN_HISTORY_MAX);

    std::fs::create_dir_all(dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
    let json = serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())?;
    let path = history_path(dir);
    let tmp = dir.join("login-history.json.tmp");
    std::fs::write(&tmp, json).map_err(|e| format!("写登录历史失败: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("替换登录历史失败: {e}"))?;
    Ok(())
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    fn tmp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("campus-history-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn 头插法_最新在前() {
        let dir = tmp_dir("head");
        append(&dir, true, "第一次", "u1", "manual").unwrap();
        append(&dir, false, "第二次", "u1", "manual").unwrap();
        let entries = read(&dir);
        assert_eq!(entries.len(), 2);
        assert!(!entries[0].success, "最新记录应在头部");
        assert_eq!(entries[0].message, "第二次");
        assert_eq!(entries[1].message, "第一次");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 上限_100条截断() {
        let dir = tmp_dir("cap");
        for i in 0..110 {
            append(&dir, true, &format!("第{i}条"), "u1", "auto").unwrap();
        }
        let entries = read(&dir);
        assert_eq!(entries.len(), LOGIN_HISTORY_MAX);
        assert_eq!(entries[0].message, "第109条", "最新在头");
        assert_eq!(entries[99].message, "第10条", "最老保留第 10 条");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 损坏文件_备份后重置() {
        let dir = tmp_dir("corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("login-history.json"), "not-json{{{").unwrap();
        let entries = read(&dir);
        assert!(entries.is_empty());
        let baks: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".corrupt-"))
            .collect();
        assert_eq!(baks.len(), 1, "损坏文件应被备份");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 记录字段_序列化形状对齐桌面() {
        let dir = tmp_dir("shape");
        append(&dir, true, "认证成功", "2024001", "manual").unwrap();
        let raw = std::fs::read_to_string(dir.join("login-history.json")).unwrap();
        assert!(raw.contains("\"type\""), "type 字段名(桌面契约)");
        assert!(raw.contains("\"adapter\": \"wlan0\""));
        assert!(raw.contains("\"user\": \"2024001\""));
        assert!(!raw.contains("password"), "历史不得含密码字段");
        std::fs::remove_dir_all(&dir).ok();
    }
}
