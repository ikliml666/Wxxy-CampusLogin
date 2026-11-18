//! 外网连通状态：把「最近一次外网探测的结论」缓存成可读状态，供界面展示。
//!
//! 唯一写入点是质量检测（`monitor::quality_scheduler::run_quality_check`）——
//! 定时测试循环与后台快排查都汇聚到那里，不额外造探测。规则：
//! - `quality == "unknown"`：这轮没测成（互斥/取消），保持上一次结论不动；
//! - 外网站点平均延迟 > 0：至少一个外网目标可达 → 连通；
//! - 其余（有结论但没有任何外网目标成功）→ 不通。

use std::sync::Mutex;

/// 一次外网探测的结论。
#[derive(Clone, serde::Serialize)]
pub struct InternetSnapshot {
    /// 是否可访问外网
    pub online: bool,
    /// 外网站点平均延迟（毫秒）；-1 表示本轮无有效样本
    pub latency_ms: i64,
    /// 探测时刻（Unix 毫秒）
    pub checked_at_ms: u64,
    /// 探测所绑定的网卡名
    pub adapter: String,
}

static SNAPSHOT: Mutex<Option<InternetSnapshot>> = Mutex::new(None);

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 记录一次质量检测的结论并返回是否产生了新状态（供调用方决定是否广播）。
/// 参数是质量检测的原始 JSON：`quality` 与 `averageExternalLatency`。
pub fn record_quality_check(adapter: &str, value: &serde_json::Value) -> Option<InternetSnapshot> {
    let quality = value.get("quality").and_then(|v| v.as_str()).unwrap_or("unknown");
    if quality == "unknown" {
        // 本轮没测成：保留上一次结论，别把"未知"当成"不通"
        return None;
    }
    let latency_ms = value
        .get("averageExternalLatency")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(-1);
    let snapshot = InternetSnapshot {
        online: latency_ms > 0,
        latency_ms,
        checked_at_ms: now_ms(),
        adapter: adapter.to_string(),
    };
    *SNAPSHOT.lock().unwrap_or_else(|e| e.into_inner()) = Some(snapshot.clone());
    Some(snapshot)
}

/// 读取最近一次结论；进程启动后尚未检测过则为 None。
pub fn current() -> Option<InternetSnapshot> {
    SNAPSHOT.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(quality: &str, avg: i64) -> serde_json::Value {
        serde_json::json!({ "quality": quality, "averageExternalLatency": avg })
    }

    #[test]
    fn 未知结论不覆盖已有状态() {
        // 先落一条确定的"不通"，再来一条 unknown：必须保持"不通"，不能被抹成未知
        let first = record_quality_check("WLAN", &value("bad", -1)).expect("应记录");
        assert!(!first.online);
        assert!(record_quality_check("WLAN", &value("unknown", -1)).is_none());
        assert_eq!(current().map(|s| s.online), Some(false));
    }

    #[test]
    fn 外网延迟为正即连通并带上延迟与网卡() {
        let snapshot = record_quality_check("以太网", &value("good", 37)).expect("应记录");
        assert!(snapshot.online);
        assert_eq!(snapshot.latency_ms, 37);
        assert_eq!(snapshot.adapter, "以太网");
        assert!(snapshot.checked_at_ms > 0);
    }

    #[test]
    fn 有结论但无外网样本判为不通() {
        let snapshot = record_quality_check("WLAN", &value("bad", -1)).expect("应记录");
        assert!(!snapshot.online);
        assert_eq!(snapshot.latency_ms, -1);
    }
}
