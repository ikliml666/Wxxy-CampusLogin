//! 外网连通状态：缓存「网络适配器检测模块」最近一次公网可达性探测的结论，
//! **按网卡分别记录**，供卡片每一行展示自己那张卡通不通。
//!
//! 数据源是适配器侧的公网探测（`network::quality::check_internet_access_with_if`，
//! 目标取自 `internet_probe_hosts`，由「网络检测参数」控制），不是质量检测模块的
//! 定时测速——后者测的是延迟质量，与"这张卡能不能出去"不是一回事。
//!
//! 多卡场景下每张卡的结果各自独立：校园网卡夜里不通是正常状态，不能影响夜间
//! 目标卡的结论；因此这里存的是「网卡名 → 结论」的表，而不是单一全局状态。

use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

/// 单张网卡的公网可达性结论。
#[derive(Clone, serde::Serialize)]
pub struct AdapterReachability {
    /// 网卡名
    pub adapter: String,
    /// 该卡能否访问公网
    pub online: bool,
    /// 探测时刻（Unix 毫秒）
    pub checked_at_ms: u64,
}

static SNAPSHOTS: Mutex<Option<HashMap<String, AdapterReachability>>> = Mutex::new(None);

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 批量记录一次探测结果（key=网卡名，value=是否可达）。
/// 结论确有变化时返回整张表（供广播），否则返回 None——每拍都探测，
/// 不变化就不该刷事件。
pub fn record_probes(entries: &[(String, bool)]) -> Option<Vec<AdapterReachability>> {
    if entries.is_empty() {
        return None;
    }
    let at = now_ms();
    let mut guard = SNAPSHOTS.lock().unwrap_or_else(|e| e.into_inner());
    let table = guard.get_or_insert_with(HashMap::new);
    let mut changed = false;
    for (adapter, online) in entries {
        match table.get(adapter) {
            Some(previous) if previous.online == *online => {}
            _ => changed = true,
        }
        table.insert(
            adapter.clone(),
            AdapterReachability {
                adapter: adapter.clone(),
                online: *online,
                checked_at_ms: at,
            },
        );
    }
    if !changed {
        return None;
    }
    Some(table.values().cloned().collect())
}

/// 记录并广播整张表（只在结论变化时发）。
pub fn record_probes_and_emit(app_handle: &AppHandle, entries: &[(String, bool)]) {
    if let Some(snapshot) = record_probes(entries) {
        let _ = app_handle.emit("internet-status-changed", &snapshot);
    }
}

/// 读取当前所有已知网卡的结论；尚未探测过的卡不在表中。
pub fn current_all() -> Vec<AdapterReachability> {
    SNAPSHOTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|table| table.values().cloned().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 这几条用例共享进程内的全局表：cargo test 默认多线程并行，
    /// 不加串行锁会互相 reset 对方的状态。
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn reset() -> std::sync::MutexGuard<'static, ()> {
        let guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        *SNAPSHOTS.lock().unwrap_or_else(|e| e.into_inner()) = None;
        guard
    }

    fn snapshot_for(name: &str) -> Option<AdapterReachability> {
        current_all().into_iter().find(|row| row.adapter == name)
    }

    #[test]
    fn 每张卡各自记录互不影响() {
        let _guard = reset();
        let changed = record_probes(&[
            ("WLAN".to_string(), true),
            ("以太网".to_string(), false),
        ])
        .expect("首次应广播");
        assert_eq!(changed.len(), 2);
        assert_eq!(snapshot_for("WLAN").map(|r| r.online), Some(true));
        assert_eq!(snapshot_for("以太网").map(|r| r.online), Some(false));
    }

    #[test]
    fn 结论不变不重复广播() {
        let _guard = reset();
        record_probes(&[("WLAN".to_string(), true)]).expect("首次");
        assert!(record_probes(&[("WLAN".to_string(), true)]).is_none());
    }

    #[test]
    fn 只有某张卡翻转也要广播整表() {
        let _guard = reset();
        record_probes(&[("WLAN".to_string(), true), ("以太网".to_string(), true)]).expect("首次");
        let changed =
            record_probes(&[("WLAN".to_string(), true), ("以太网".to_string(), false)])
                .expect("单卡翻转应广播");
        assert_eq!(changed.len(), 2);
        assert_eq!(snapshot_for("以太网").map(|r| r.online), Some(false));
    }
}
