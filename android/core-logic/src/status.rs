//! 跨进程核心状态(:core 写,UI 读)。
//!
//! 状态写 `core_status.json`(不可用时 UI 只能看到旧值,所以带心跳时间戳);
//! `notification_stale` 是"通知是否已陈旧"的唯一判据,由 Rust 与 UI 共用。

use serde::{Deserialize, Serialize};

/// 心跳/通知允许的最大静默时长。
/// 闲时间隔上限按默认 5min 计,留 3 拍余量;超过即视为核心已停或通知未更新。
pub const MAX_STATUS_AGE_MS: u64 = 15 * 60 * 1000;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CoreStatus {
    /// 巡检循环是否在跑(与 UI 的"后台检测"开关一致)
    pub is_running: bool,
    /// 核心是否已初始化(:core 进程 nativeStart 成功)
    pub core_alive: bool,
    /// 已完成检测次数
    pub check_count: u64,
    /// 本次在线周期内自动重连次数
    pub reconnect_count: u32,
    /// 连续失败次数
    pub consecutive_failures: u32,
    /// WiFi watcher 是否注册失败(退化为纯周期检测)
    pub watcher_failed: bool,
    /// 最近一拍或服务定时心跳时间(epoch ms)
    pub last_tick_ms: u64,
    /// 最近一次写入常驻通知的文案
    pub notification_text: String,
    /// 最近一次写入常驻通知的时间(epoch ms)
    pub notification_updated_ms: u64,
    /// 最近一次检测结果(原样透传给 UI,形状与 background-check-result 一致)
    pub last_result: Option<serde_json::Value>,
}

/// 核心是否存活:进程被杀不会走 onDestroy,必须用心跳年龄识别遗留的 coreAlive=true
pub fn core_is_alive(status: &CoreStatus, now_ms: u64, max_age_ms: u64) -> bool {
    status.core_alive
        && status.last_tick_ms != 0
        && now_ms.saturating_sub(status.last_tick_ms) <= max_age_ms
}

/// 通知是否陈旧:核心已死、从未心跳、或通知超过 `max_age_ms` 未更新。
pub fn notification_stale(status: &CoreStatus, now_ms: u64, max_age_ms: u64) -> bool {
    if !core_is_alive(status, now_ms, max_age_ms) {
        return true;
    }
    now_ms.saturating_sub(status.notification_updated_ms) > max_age_ms
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alive(now: u64, notification_ms: u64) -> CoreStatus {
        CoreStatus {
            is_running: true,
            core_alive: true,
            check_count: 1,
            last_tick_ms: now,
            notification_text: "监控运行中 · 在线".to_string(),
            notification_updated_ms: notification_ms,
            ..Default::default()
        }
    }

    #[test]
    fn 核心未启动时通知视为陈旧() {
        // 服务被 START_STICKY 复活但核心没起来:通知写着"运行中"却是死的
        let status = CoreStatus::default();
        assert!(notification_stale(&status, 1_000, MAX_STATUS_AGE_MS));
    }

    #[test]
    fn 心跳与通知都在窗口内时不陈旧() {
        let now = 10_000_000;
        assert!(!notification_stale(&alive(now, now - 60_000), now, MAX_STATUS_AGE_MS));
    }

    #[test]
    fn 心跳过期时核心与通知都视为死亡() {
        let now = 10_000_000;
        let mut status = alive(now, now);
        assert!(core_is_alive(&status, now, MAX_STATUS_AGE_MS));
        status.last_tick_ms = now - MAX_STATUS_AGE_MS - 1;
        assert!(!core_is_alive(&status, now, MAX_STATUS_AGE_MS));
        assert!(notification_stale(&status, now, MAX_STATUS_AGE_MS));
    }

    #[test]
    fn 通知超过上限未更新即陈旧() {
        let now = 10_000_000;
        let status = alive(now, now - MAX_STATUS_AGE_MS - 1);
        assert!(notification_stale(&status, now, MAX_STATUS_AGE_MS));
    }

    #[test]
    fn 状态文件形状可跨进程往返() {
        let status = alive(1_000, 900);
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"coreAlive\""));
        let back: CoreStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, status);
    }
}
