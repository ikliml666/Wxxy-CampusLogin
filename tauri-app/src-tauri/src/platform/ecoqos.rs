//! Windows 效率模式（EcoQoS / Power Throttling）。
//!
//! 语义：进入轻量化（无任何 WebView 窗口）时把本进程标记为 EcoQoS，
//! 系统调度到 E-core 并降频；重建窗口时交还系统管理。WebView2 子进程
//! 会继承宿主节流状态，因此仅在窗口全部销毁后开启。
//!
//! 硬约束（调研结论，不得违反）：
//! - 不叠加 IDLE/BELOW_NORMAL 优先级类——IDLE 在游戏满载时会饿死自身巡检；
//! - 不设置 PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION——Win11 已自动
//!   处理，显式设置会让检测循环计时器被合并漂移；
//! - Version 必须传 CURRENT_VERSION（传 0 会静默失败）；
//! - GetCurrentProcess() 返回伪句柄，禁止 CloseHandle。

#[cfg(windows)]
pub fn set_ecoqos(enable: bool) {
    let state = throttling_state(enable);
    unsafe {
        let handle = windows::Win32::System::Threading::GetCurrentProcess();
        let result = windows::Win32::System::Threading::SetProcessInformation(
            handle,
            windows::Win32::System::Threading::ProcessPowerThrottling,
            &state as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<windows::Win32::System::Threading::PROCESS_POWER_THROTTLING_STATE>()
                as u32,
        );
        if let Err(e) = result {
            crate::log_warn!("ecoqos", "设置效率模式({})失败: {}", enable, e);
        }
    }
}

#[cfg(not(windows))]
pub fn set_ecoqos(_enable: bool) {}

/// 官方三态写法：开 = ControlMask=StateMask=EXECUTION_SPEED；
/// 关 = ControlMask=StateMask=0（交还系统管理）
#[cfg(windows)]
fn throttling_state(
    enable: bool,
) -> windows::Win32::System::Threading::PROCESS_POWER_THROTTLING_STATE {
    use windows::Win32::System::Threading::{
        PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_STATE,
    };
    // Win32 头文件定义 #define PROCESS_POWER_THROTTLING_EXECUTION_SPEED 0x1，
    // windows crate 以裸 u32 常量导出
    const EXECUTION_SPEED: u32 =
        windows::Win32::System::Threading::PROCESS_POWER_THROTTLING_EXECUTION_SPEED;
    PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: if enable { EXECUTION_SPEED } else { 0 },
        StateMask: if enable { EXECUTION_SPEED } else { 0 },
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::throttling_state;

    #[test]
    fn 节流状态_开启时控制与状态位均为执行速度() {
        let s = throttling_state(true);
        assert_eq!(
            s.ControlMask,
            windows::Win32::System::Threading::PROCESS_POWER_THROTTLING_EXECUTION_SPEED
        );
        assert_eq!(
            s.StateMask,
            windows::Win32::System::Threading::PROCESS_POWER_THROTTLING_EXECUTION_SPEED
        );
        assert_eq!(s.Version, windows::Win32::System::Threading::PROCESS_POWER_THROTTLING_CURRENT_VERSION);
    }

    #[test]
    fn 节流状态_关闭时交还系统管理() {
        let s = throttling_state(false);
        assert_eq!(s.ControlMask, 0);
        assert_eq!(s.StateMask, 0);
    }
}
