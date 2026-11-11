//! 桌面轻量化模式运行态：状态标志、窗口几何缓存、退出守卫与间隔系数。
//!
//! 标志/守卫/就绪信号是内存态；窗口几何额外落盘（`window_geometry.json`，
//! 物理像素口径），供跨进程重启恢复上次位置。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// 轻量化生效中：无 WebView 窗口、EcoQoS 已开启、检测间隔延长
static LIGHTWEIGHT_ACTIVE: AtomicBool = AtomicBool::new(false);
/// 单次退出守卫：由「轻量化关闭导致最后一个窗口销毁」的 CloseRequested 置位，
/// 被 RunEvent::ExitRequested 取走消费（SwitchHosts EXPECT_LIGHTWEIGHT_EXIT 同款）。
/// 仅用长活标志会连系统关机一起拦截，必须单次语义
static EXPECT_LIGHTWEIGHT_EXIT: AtomicBool = AtomicBool::new(false);
/// 销毁前记录的窗口几何，重建时应用
static GEOMETRY: Mutex<Option<WindowGeometry>> = Mutex::new(None);

/// 窗口几何：全部为**物理像素**（`outer_position`/`outer_size` 原始值）。
///
/// 恢复端必须用 `PhysicalPosition`/`PhysicalSize`，不要再乘/除 `scale_factor`；
/// builder 的 `position`/`inner_size` 是逻辑像素，高 DPI 下混用会偏移。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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
        let geo = WindowGeometry {
            x: pos.x as f64,
            y: pos.y as f64,
            width: size.width as f64,
            height: size.height as f64,
        };
        if let Ok(mut g) = GEOMETRY.lock() {
            *g = Some(geo);
        }
    }
}

pub fn take_geometry() -> Option<WindowGeometry> {
    GEOMETRY.lock().ok().and_then(|mut g| g.take())
}

/// 窗口几何落盘文件名（数据目录下，与 config.json 同级）
const GEOMETRY_FILE: &str = "window_geometry.json";

pub fn geometry_path(dir: &std::path::Path) -> std::path::PathBuf {
    dir.join(GEOMETRY_FILE)
}

/// 把最近一次 capture 的几何写入数据目录（物理像素，跨进程恢复用）。
pub fn save_geometry(app: &tauri::AppHandle) {
    let Some(geo) = GEOMETRY.lock().ok().and_then(|g| *g) else {
        return;
    };
    let dir = crate::config::persist::get_data_dir(app);
    match serde_json::to_string(&geo) {
        Ok(body) => {
            if let Err(e) = std::fs::write(geometry_path(&dir), body) {
                crate::log_warn!("lightweight", "保存窗口几何失败: {}", e);
            }
        }
        Err(e) => crate::log_warn!("lightweight", "序列化窗口几何失败: {}", e),
    }
}

/// 读取上次落盘的几何。文件不存在（首次启动）返回 None；损坏时告警并回退居中。
pub fn load_geometry(app: &tauri::AppHandle) -> Option<WindowGeometry> {
    let dir = crate::config::persist::get_data_dir(app);
    let body = std::fs::read_to_string(geometry_path(&dir)).ok()?;
    match serde_json::from_str(&body) {
        Ok(geo) => Some(geo),
        Err(e) => {
            crate::log_warn!("lightweight", "窗口几何文件损坏，回退居中默认: {}", e);
            None
        }
    }
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

/// 前端就绪信号：notify_window_ready 命令置位，重建窗口的 ready 门消费
static READY_SIGNALS: Mutex<Vec<(String, u64)>> = Mutex::new(Vec::new());

pub fn signal_window_ready(label: &str) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if let Ok(mut v) = READY_SIGNALS.lock() {
        v.push((label.to_string(), now));
    }
}

fn take_window_ready(label: &str) -> bool {
    if let Ok(mut v) = READY_SIGNALS.lock() {
        if let Some(pos) = v.iter().position(|(l, _)| l == label) {
            v.remove(pos);
            return true;
        }
    }
    false
}

/// 等 ready 信号或 5s 超时后显示窗口（500ms 轮询；信号早到立即显示）。
/// 防重建白闪：WebviewWindowBuilder 以 visible(false) 起步，前端挂载完成后
/// invoke notify_window_ready，收到即显示；超时兜底防止前端异常时永远黑窗
pub async fn wait_window_ready_and_show(app: &tauri::AppHandle, label: &str) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if take_window_ready(label) {
            crate::log_info!("lightweight", "前端就绪信号已收到，显示主窗口");
            break;
        }
        if std::time::Instant::now() >= deadline {
            crate::log_warn!("lightweight", "等待前端就绪超时(5s)，兜底显示主窗口");
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    // 复用统一显示路径：会先 show 窗口再 Resume 挂起的渲染、清掉 hidden_by_design 标记
    crate::app::window::show_and_focus_main(app);
    // 重建出来的是全新 WebView：旧窗口销毁时 ProcessFailed 订阅已随之失效，
    // 等前端就绪后重新订阅，否则重建后的崩溃即时恢复链路缺失（只剩心跳超时兜底）
    #[cfg(target_os = "windows")]
    crate::app::webview_recovery::subscribe_process_failed(app);
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

    #[test]
    fn 几何序列化_物理像素往返() {
        let geo = WindowGeometry { x: 120.0, y: 80.0, width: 2040.0, height: 1152.0 };
        let body = serde_json::to_string(&geo).unwrap();
        let back: WindowGeometry = serde_json::from_str(&body).unwrap();
        assert_eq!(back, geo);
    }

    #[test]
    fn 几何离屏校验_必须用物理值_逻辑值在屏内也不算在屏() {
        // 150% DPI：显示器物理 1920 宽；窗口物理 x=2000 已在屏外，
        // 若按旧口径存逻辑值（x≈1333）会被误判为在屏。
        let monitors = vec![(0i32, 0i32, 1920u32, 1080u32)];
        let physical = WindowGeometry { x: 2000.0, y: 100.0, width: 2040.0, height: 1536.0 };
        assert!(!geometry_is_on_screen(&physical, &monitors));
        let logical_would_be = WindowGeometry { x: 1333.0, y: 66.0, width: 1360.0, height: 1024.0 };
        assert!(
            geometry_is_on_screen(&logical_would_be, &monitors),
            "逻辑口径会误判在屏，本断言锁定恢复端必须按物理像素判断"
        );
    }

    #[test]
    fn 几何离屏校验_副屏负坐标() {
        let monitors = vec![(-1920i32, 0i32, 1920u32, 1080u32), (0, 0, 1920, 1080)];
        let on_left = WindowGeometry { x: -1800.0, y: 50.0, width: 1360.0, height: 768.0 };
        assert!(geometry_is_on_screen(&on_left, &monitors));
        let far_left = WindowGeometry { x: -4000.0, y: 50.0, width: 1360.0, height: 768.0 };
        assert!(!geometry_is_on_screen(&far_left, &monitors));
    }

    #[test]
    fn 几何文件_固定在数据目录() {
        let path = geometry_path(std::path::Path::new("data"));
        assert!(path.ends_with(GEOMETRY_FILE));
    }
}
