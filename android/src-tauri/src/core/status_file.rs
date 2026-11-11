//! 核心状态文件(`core_status.json`):`:core` 唯一写者,UI 只读。

use std::path::{Path, PathBuf};

pub use campus_core_logic::status::CoreStatus;

pub const CORE_STATUS_FILE: &str = "core_status.json";

pub fn path_of(dir: &Path) -> PathBuf {
    dir.join(CORE_STATUS_FILE)
}

/// 文件不存在是首启的正常态，返回默认；存在但读/解析失败必须留痕，
/// 否则「核心从未写过状态」与「状态文件损坏」在 UI 上完全无法区分。
pub fn read(dir: &Path) -> CoreStatus {
    let path = path_of(dir);
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return CoreStatus::default(),
        Err(e) => {
            campus_login_lib::log_warn!("monitor", "读取核心状态文件失败: {e}");
            return CoreStatus::default();
        }
    };
    match serde_json::from_str(&raw) {
        Ok(status) => status,
        Err(e) => {
            campus_login_lib::log_warn!("monitor", "核心状态文件损坏: {e}");
            CoreStatus::default()
        }
    }
}

/// 原子写:tmp(带 pid) → rename,避免 UI 读到半截 JSON
pub fn write(dir: &Path, status: &CoreStatus) {
    let path = path_of(dir);
    let tmp = dir.join(format!("{CORE_STATUS_FILE}.{}.tmp", std::process::id()));
    let json = match serde_json::to_string(status) {
        Ok(json) => json,
        Err(e) => {
            campus_login_lib::log_error!("monitor", "序列化核心状态失败: {e}");
            return;
        }
    };
    if let Err(e) = std::fs::write(&tmp, json) {
        campus_login_lib::log_error!("monitor", "写入核心状态临时文件失败: {e}");
        return;
    }
    if let Err(e) = std::fs::rename(&tmp, &path) {
        campus_login_lib::log_error!("monitor", "提交核心状态文件失败: {e}");
        let _ = std::fs::remove_file(&tmp);
    }
}