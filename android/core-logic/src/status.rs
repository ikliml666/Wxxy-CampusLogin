//! 跨进程核心状态(:core 写,UI 读)。
//!
//! 状态写 `core_status.json`(不可用时 UI 只能看到旧值,所以带心跳时间戳);
//! `notification_stale` 是"通知是否已陈旧"的唯一判据,由 Rust 与 UI 共用。

use serde::{Deserialize, Serialize};

/// 活性窗口下限：巡检间隔很短时也不小于 3 拍，
/// 防止一拍内的调度抖动被误判成核心已死。
pub const MIN_ALIVE_WINDOW_MS: u64 = 3 * 60 * 1000;

/// 活性窗口：必须由巡检间隔推导，不能写死。
/// 用户可把间隔配到 600s，固定窗口会把"两拍之间的正常等待"判成已停止，
/// 于是 UI 每 5 秒对账把运行中的核心刷成已停止（假活修成假死）。
pub fn alive_window_ms(interval_ms: u64) -> u64 {
    interval_ms.saturating_mul(3).max(MIN_ALIVE_WINDOW_MS)
}

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
    /// 最近一次真实巡检开始时间(epoch ms)。服务心跳不得刷新它，
    /// 否则通知定时器会掩盖已经停止的 tokio 巡检循环
    pub last_tick_ms: u64,
    /// 当前生效的巡检间隔(ms)。活性窗口按它推导，UI 与通知不得各写一份阈值
    pub interval_ms: u64,
    /// 最近一次写入常驻通知的文案
    pub notification_text: String,
    /// 最近一次写入常驻通知的时间(epoch ms)
    pub notification_updated_ms: u64,
    /// 最近一次检测结果(原样透传给 UI,形状与 background-check-result 一致)
    pub last_result: Option<serde_json::Value>,
}

/// 最近一次检测结果的派生字段→UI 形状的单点透传。
/// :core 的状态快照与 UI 的 core_status 读取共用同一 key 列表，避免两处各写一份。
pub fn copy_last_result_fields(target: &mut serde_json::Value, last: Option<&serde_json::Value>) {
    let Some(last) = last else { return };
    for key in [
        "online",
        "message",
        "onlineOperator",
        "serverAvailable",
        "onCampusNetwork",
    ] {
        if let Some(val) = last.get(key) {
            target[key] = val.clone();
        }
    }
}

/// 核心是否存活:进程被杀不会走 onDestroy,必须用心跳年龄识别遗留的 coreAlive=true。
/// 窗口由状态里记录的巡检间隔推导，调用方不再各传一个魔法数字。
pub fn core_is_alive(status: &CoreStatus, now_ms: u64) -> bool {
    status.core_alive
        && status.last_tick_ms != 0
        && now_ms.saturating_sub(status.last_tick_ms) <= alive_window_ms(status.interval_ms)
}

/// 通知是否陈旧:核心已死、从未心跳、或通知超过窗口未更新。
pub fn notification_stale(status: &CoreStatus, now_ms: u64) -> bool {
    if !core_is_alive(status, now_ms) {
        return true;
    }
    now_ms.saturating_sub(status.notification_updated_ms) > alive_window_ms(status.interval_ms)
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
            interval_ms: 60_000,
            notification_text: "监控运行中 · 在线".to_string(),
            notification_updated_ms: notification_ms,
            ..Default::default()
        }
    }

    #[test]
    fn 核心未启动时通知视为陈旧() {
        // 服务被 START_STICKY 复活但核心没起来:通知写着"运行中"却是死的
        let status = CoreStatus::default();
        assert!(notification_stale(&status, 1_000));
    }

    #[test]
    fn 心跳与通知都在窗口内时不陈旧() {
        let now = 10_000_000;
        assert!(!notification_stale(&alive(now, now - 60_000), now));
    }

    #[test]
    fn 心跳过期时核心与通知都视为死亡() {
        let now = 10_000_000;
        let mut status = alive(now, now);
        assert!(core_is_alive(&status, now));
        status.last_tick_ms = now - alive_window_ms(status.interval_ms) - 1;
        assert!(!core_is_alive(&status, now));
        assert!(notification_stale(&status, now));
    }

    #[test]
    fn 通知超过窗口未更新即陈旧() {
        let now = 10_000_000;
        let status = alive(now, now - alive_window_ms(60_000) - 1);
        assert!(notification_stale(&status, now));
    }

    #[test]
    fn 长间隔巡检不会被活性窗口误判为停止() {
        // 用户把间隔配到 600s:固定 3min 窗口会在两拍之间判死，
        // 这里锁定窗口必须随间隔放大
        let now = 10_000_000;
        let mut status = alive(now, now);
        status.interval_ms = 600_000;
        status.last_tick_ms = now - 400_000;
        assert!(core_is_alive(&status, now));
    }

    #[test]
    fn 窗口下限不小于三分钟() {
        assert_eq!(alive_window_ms(5_000), MIN_ALIVE_WINDOW_MS);
        assert_eq!(alive_window_ms(60_000), 180_000);
        assert_eq!(alive_window_ms(600_000), 1_800_000);
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
