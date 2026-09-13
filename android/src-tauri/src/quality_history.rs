//! 网络质量历史:data_dir/quality_history.json,头插法上限 100 条,损坏备份后重置
//! (与 login_history.rs 同构;记录形状双端一致:
//! {timestamp, gatewayLatency, externalLatency, quality},负延迟落盘为 null)。

use campus_login_lib::network::quality::NetworkQualityResult;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static QUALITY_HISTORY_LOCK: Mutex<()> = Mutex::new(());

pub const QUALITY_HISTORY_MAX: usize = 100;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QualityHistoryEntry {
    pub timestamp: u64,
    /// 检测失败/未执行(源数据 -1)时为 null,区分"未测"与 0ms
    pub gateway_latency: Option<i64>,
    pub external_latency: Option<i64>,
    pub quality: String,
}

fn history_path(dir: &Path) -> PathBuf {
    dir.join("quality_history.json")
}

/// 读取历史;损坏时备份 .corrupt-*.bak 后重置,不让一条坏记录卡死检测流程。
pub fn read(dir: &Path) -> Vec<QualityHistoryEntry> {
    let path = history_path(dir);
    match std::fs::read(&path) {
        Ok(raw) => match serde_json::from_str(&String::from_utf8_lossy(&raw)) {
            Ok(entries) => entries,
            Err(_) => {
                let bak = dir.join(format!("quality_history.json.corrupt-{}.bak", epoch_millis()));
                let _ = std::fs::rename(&path, &bak);
                Vec::new()
            }
        },
        Err(_) => Vec::new(),
    }
}

fn epoch_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// 追加一条质量历史(头插法,上限 100),原子写(tmp+rename)。
pub fn append(dir: &Path, result: &NetworkQualityResult) -> Result<(), String> {
    let _guard = QUALITY_HISTORY_LOCK
        .lock()
        .map_err(|_| "质量历史锁损坏".to_string())?;
    let mut entries = read(dir);
    entries.insert(
        0,
        QualityHistoryEntry {
            timestamp: result.timestamp,
            gateway_latency: (result.gateway_latency >= 0).then_some(result.gateway_latency),
            external_latency: (result.external_latency >= 0).then_some(result.external_latency),
            quality: result.quality.clone(),
        },
    );
    entries.truncate(QUALITY_HISTORY_MAX);

    std::fs::create_dir_all(dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
    let json = serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())?;
    let path = history_path(dir);
    let tmp = dir.join("quality_history.json.tmp");
    std::fs::write(&tmp, json).map_err(|e| format!("写质量历史失败: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("替换质量历史失败: {e}"))?;
    Ok(())
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    fn tmp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("campus-quality-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn result(ts: u64, gw: i64, ext: i64, quality: &str) -> NetworkQualityResult {
        NetworkQualityResult {
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
    fn 头插法_最新在前() {
        let dir = tmp_dir("head");
        append(&dir, &result(1000, 12, 45, "good")).unwrap();
        append(&dir, &result(2000, -1, 30, "poor")).unwrap();
        let entries = read(&dir);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].timestamp, 2000, "最新记录应在头部");
        assert_eq!(entries[0].quality, "poor");
        assert_eq!(entries[0].gateway_latency, None, "负延迟应落盘为 null");
        assert_eq!(entries[0].external_latency, Some(30));
        assert_eq!(entries[1].gateway_latency, Some(12));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 上限_100条截断() {
        let dir = tmp_dir("cap");
        for i in 0..110 {
            append(&dir, &result(i as u64, 1, 1, "good")).unwrap();
        }
        let entries = read(&dir);
        assert_eq!(entries.len(), QUALITY_HISTORY_MAX);
        assert_eq!(entries[0].timestamp, 109, "最新在头");
        assert_eq!(entries[99].timestamp, 10, "最老保留第 10 条");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 损坏文件_备份后重置() {
        let dir = tmp_dir("corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("quality_history.json"), "not-json{{{").unwrap();
        let entries = read(&dir);
        assert!(entries.is_empty());
        let baks: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".corrupt-"))
            .collect();
        assert_eq!(baks.len(), 1, "损坏文件应被备份");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 记录字段_序列化形状双端一致() {
        let dir = tmp_dir("shape");
        append(&dir, &result(1234567890, 12, 45, "good")).unwrap();
        let raw = std::fs::read_to_string(dir.join("quality_history.json")).unwrap();
        assert!(raw.contains("\"timestamp\""), "camelCase 键名(桌面契约)");
        assert!(raw.contains("\"gatewayLatency\": 12"));
        assert!(raw.contains("\"externalLatency\": 45"));
        assert!(raw.contains("\"quality\": \"good\""));
        assert!(!raw.contains("gateway_latency"), "不得出现 snake_case 键名");
        std::fs::remove_dir_all(&dir).ok();
    }
}
