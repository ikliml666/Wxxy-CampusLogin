//! 桌面轻量化模式运行态：状态标志、窗口几何缓存、退出守卫与间隔系数。
//!
//! 全部为内存态（不落盘）：轻量化是「关闭按钮的运行期行为」，重启后
//! 由 `lightweight_mode` 配置决定行为，无需跨进程记忆。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// 轻量化生效中：无 WebView 窗口、EcoQoS 已开启、检测间隔延长
static LIGHTWEIGHT_ACTIVE: AtomicBool = AtomicBool::new(false);
/// 单次退出守卫：由「轻量化关闭导致最后一个窗口销毁」的 CloseRequested 置位，
/// 被 RunEvent::ExitRequested 取走消费（SwitchHosts EXPECT_LIGHTWEIGHT_EXIT 同款）。
/// 仅用长活标志会连系统关机一起拦截，必须单次语义
static EXPECT_LIGHTWEIGHT_EXIT: AtomicBool = AtomicBool::new(false);
/// 销毁前记录的窗口几何，重建时应用
static GEOMETRY: Mutex<Option<WindowGeometry>> = Mutex::new(None);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowGeometry {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub fn is_lightweight_active() -> bool {
    LIGHTWEIGHT_ACTIVE.load(Ordering::Acquire)
}

pub fn set_lightweight_active(active: bool) {
    LIGHTWEIGHT_ACTIVE.store(active, Ordering::Release);
}

pub fn arm_lightweight_exit_guard() {
    EXPECT_LIGHTWEIGHT_EXIT.store(true, Ordering::Release);
}

pub fn take_lightweight_exit_guard() -> bool {
    EXPECT_LIGHTWEIGHT_EXIT.swap(false, Ordering::AcqRel)
}

/// 退出拦截判定（纯函数）：仅当「本次退出来自轻量化关闭」且不在主动退出
/// 流程（托盘退出/系统关机走 graceful_exit，is_quitting 已置位）时拦截
pub fn should_prevent_exit(expecting: bool, is_quitting: bool) -> bool {
    expecting && !is_quitting
}

pub fn capture_geometry(window: &tauri::WebviewWindow) {
    if let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) {
        let scale = window.scale_factor().unwrap_or(1.0);
        let geo = WindowGeometry {
            x: pos.x as f64,
            y: pos.y as f64,
            width: size.width as f64 / scale,
            height: size.height as f64 / scale,
        };
        if let Ok(mut g) = GEOMETRY.lock() {
            *g = Some(geo);
        }
    }
}

pub fn take_geometry() -> Option<WindowGeometry> {
    GEOMETRY.lock().ok().and_then(|mut g| g.take())
}

/// 几何与任一显示器矩形相交即认为在屏上（纯函数）
pub fn geometry_is_on_screen(geo: &WindowGeometry, monitors: &[(i32, i32, u32, u32)]) -> bool {
    monitors.iter().any(|&(mx, my, mw, mh)| {
        let (mx, my) = (mx as f64, my as f64);
        let (mw, mh) = (mw as f64, mh as f64);
        geo.x < mx + mw && geo.x + geo.width > mx && geo.y < my + mh && geo.y + geo.height > my
    })
}

/// 轻量化期间后台检测间隔下限 300s（纯函数；非轻量化原样返回）
pub fn effective_background_interval_ms(base_ms: u64, lightweight: bool) -> u64 {
    if lightweight { base_ms.max(300_000) } else { base_ms }
}

/// 轻量化期间质量检测间隔下限 1800s（纯函数；非轻量化原样返回）
pub fn effective_quality_interval_ms(base_ms: u64, lightweight: bool) -> u64 {
    if lightweight { base_ms.max(1_800_000) } else { base_ms }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 守卫判定_仅有当次期待且未在退出流程时才拦截() {
        assert!(should_prevent_exit(true, false));
        assert!(!should_prevent_exit(false, false));
        assert!(!should_prevent_exit(true, true));
        assert!(!should_prevent_exit(false, true));
    }

    #[test]
    fn 轻量化间隔_下限语义() {
        assert_eq!(effective_background_interval_ms(60000, true), 300_000);
        assert_eq!(effective_background_interval_ms(60000, false), 60000);
        // 用户显式设了更大值则保持更大值
        assert_eq!(effective_background_interval_ms(900_000, true), 900_000);
        assert_eq!(effective_quality_interval_ms(600000, true), 1_800_000);
        assert_eq!(effective_quality_interval_ms(600000, false), 600000);
        assert_eq!(effective_quality_interval_ms(3_600_000, true), 3_600_000);
    }

    #[test]
    fn 几何离屏校验_与任一显示器相交即有效() {
        let monitors = vec![(0i32, 0i32, 1920u32, 1080u32), (1920, 0, 1920, 1080)];
        let on = WindowGeometry { x: 100.0, y: 100.0, width: 1360.0, height: 768.0 };
        let off = WindowGeometry { x: 5000.0, y: 100.0, width: 1360.0, height: 768.0 };
        assert!(geometry_is_on_screen(&on, &monitors));
        assert!(!geometry_is_on_screen(&off, &monitors));
    }

    #[test]
    fn 退出守卫_单次语义_取走即清零() {
        arm_lightweight_exit_guard();
        assert!(take_lightweight_exit_guard());
        assert!(!take_lightweight_exit_guard(), "第二次取走应为 false（单次消费）");
    }
}
