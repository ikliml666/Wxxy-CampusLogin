//! 外网连通状态：缓存「网络适配器检测模块」最近一次公网可达性探测的结论，
//! 供界面展示。
//!
//! 数据源是适配器侧的公网探测（`network::quality::check_internet_access_with_if`，
//! 目标取自 `internet_probe_hosts`，由「网络检测参数」控制），**不是**质量检测
//! 模块的延迟测试——后者是独立的定时测速，与"现在能不能上外网"不是一回事。
//!
//! 只记录**当前默认出口**那张卡的结论：切换/故障判定会同时探测多张卡，
//! 其中未当选的卡探测失败是正常的，不能拿它把状态涂成"不通"。

use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

/// 一次公网可达性探测的结论。
#[derive(Clone, serde::Serialize)]
pub struct InternetSnapshot {
    /// 当前出口是否可达公网
    pub online: bool,
    /// 探测时刻（Unix 毫秒）
    pub checked_at_ms: u64,
    /// 被探测的网卡名（当前默认出口）
    pub adapter: String,
}

static SNAPSHOT: Mutex<Option<InternetSnapshot>> = Mutex::new(None);

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 记录一次当前出口的公网探测结论；结论有变化或首次记录时返回快照（供广播）。
/// 相同结论不重复广播，避免每拍刷一次事件。
pub fn record_probe(adapter: &str, online: bool) -> Option<InternetSnapshot> {
    let mut guard = SNAPSHOT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(previous) = guard.as_ref() {
        if previous.online == online && previous.adapter == adapter {
            // 结论没变，只更新时间，不广播
            let refreshed = InternetSnapshot {
                online,
                checked_at_ms: now_ms(),
                adapter: adapter.to_string(),
            };
            *guard = Some(refreshed);
            return None;
        }
    }
    let snapshot = InternetSnapshot {
        online,
        checked_at_ms: now_ms(),
        adapter: adapter.to_string(),
    };
    *guard = Some(snapshot.clone());
    Some(snapshot)
}

/// 读取最近一次结论；进程启动后尚未探测过则为 None。
pub fn current() -> Option<InternetSnapshot> {
    SNAPSHOT.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// 记录并广播（`internet-status-changed`），供各探测调用点复用。
pub fn record_and_emit(app_handle: &AppHandle, adapter: &str, online: bool) {
    if let Some(snapshot) = record_probe(adapter, online) {
        let _ = app_handle.emit("internet-status-changed", &snapshot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset() {
        *SNAPSHOT.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    #[test]
    fn 首次探测落状态并广播() {
        reset();
        let snapshot = record_probe("WLAN", true).expect("首次应广播");
        assert!(snapshot.online);
        assert_eq!(snapshot.adapter, "WLAN");
        assert_eq!(current().map(|s| s.online), Some(true));
    }

    #[test]
    fn 结论不变只更新时间不重复广播() {
        reset();
        record_probe("WLAN", true).expect("首次");
        assert!(
            record_probe("WLAN", true).is_none(),
            "同一结论不该每拍重复推送事件"
        );
        assert!(record_probe("WLAN", false).is_some(), "结论翻转必须广播");
    }

    #[test]
    fn 出口换到另一张卡时即使都是连通也要广播() {
        reset();
        record_probe("以太网", true).expect("首次");
        let snapshot = record_probe("WLAN", true).expect("换卡应广播");
        assert_eq!(snapshot.adapter, "WLAN");
    }
}
