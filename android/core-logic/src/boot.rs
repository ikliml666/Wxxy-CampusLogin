//! 核心启动决策。
//!
//! 约束:核心的启动只由**持久化配置**决定,与 Activity 是否创建、进程由谁拉起无关。
//! 前台服务被系统杀死后由 START_STICKY 重建时,进程里没有 Activity,
//! 但核心仍必须按同一决策恢复巡检与 WiFi watcher。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootPlan {
    /// 后台检测关闭:核心待命,不注册 watcher、不起循环
    Idle,
    /// 拉起巡检循环 + WiFi 变化监听
    StartMonitor,
}

/// 启动决策:唯一输入是持久化开关。
///
/// 不接收 "是否有 Activity" 之类的入参,是刻意的:一旦入参里出现 Activity 状态,
/// "无 Activity 核心不启动" 的缺陷就会重新长回来。
pub fn boot_plan(background_check_enabled: bool) -> BootPlan {
    if background_check_enabled {
        BootPlan::StartMonitor
    } else {
        BootPlan::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 持久化开关开启时_无Activity也启动核心() {
        // 服务 START_STICKY 重生、BootReceiver 拉起时都没有 Activity,
        // 决策只看持久化开关 —— 必须是 StartMonitor。
        assert_eq!(boot_plan(true), BootPlan::StartMonitor);
    }

    #[test]
    fn 持久化开关关闭时_核心待命() {
        assert_eq!(boot_plan(false), BootPlan::Idle);
    }
}
