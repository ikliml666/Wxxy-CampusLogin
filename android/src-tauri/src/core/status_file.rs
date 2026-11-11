//! 核心状态文件(`core_status.json`):`:core` 唯一写者,UI 只读。

use std::path::{Path, PathBuf};

pub use campus_core_logic::status::CoreStatus;

pub const CORE_STATUS_FILE: &str = "core_status.json";

pub fn path_of(dir: &Path) -> PathBuf {
    dir.join(CORE_STATUS_FILE)
}

pub fn read(dir: &Path) -> CoreStatus {
    std::fs::read_to_string(path_of(dir))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// 原子写:tmp(带 pid) → rename,避免 UI 读到半截 JSON
pub fn write(dir: &Path, status: &CoreStatus) {
    let path = path_of(dir);
    let tmp = dir.join(format!("{CORE_STATUS_FILE}.{}.tmp", std::process::id()));
    let Ok(json) = serde_json::to_string(status) else { return };
    if std::fs::write(&tmp, json).is_err() {
        return;
    }
    if std::fs::rename(&tmp, &path).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}