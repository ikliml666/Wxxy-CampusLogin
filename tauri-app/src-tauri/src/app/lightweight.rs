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

/// 窗口几何：位置为**物理像素**外框左上角（`outer_position`），尺寸为
/// **物理像素客户区**（`inner_size`，与恢复端 `set_size` 同一口径）。
///
/// 不要用 `outer_size`：decorations:false + shadow:true 时外框含阴影边框，
/// 与 `set_size` 的客户区口径不一致，每轮恢复都会把窗口放大一点。
/// 恢复端必须用 `PhysicalPosition`/`PhysicalSize`，不要再乘/除 `scale_factor`；
/// builder 的 `position`/`inner_size` 是逻辑像素，高 DPI 下混用会偏移。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowGeometry {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// 上次退出/销毁时窗口处于最大化。恢复时先还原普通几何，show 后再 maximize
    /// （show 前 maximize 会让窗口提前显示，show 后 set_size 又会清掉最大化）。
    /// 旧版落盘文件无此字段，按 false 处理。
    #[serde(default)]
    pub maximized: bool,
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

/// 记录窗口几何（物理像素：`outer_position` + `inner_size`）。
///
/// 最小化时 Win32 的窗口矩形是 (-32000,-32000)/异常尺寸，直接捕获会污染记忆值，
/// 因此保留上一次好值；最大化时只置状态位、几何沿用最大化前的普通窗口值
/// （最大化尺寸不能当作还原尺寸，否则退出最大化后窗口会变成全屏大小）。
pub fn capture_geometry(window: &tauri::WebviewWindow) {
    if window.is_minimized().unwrap_or(false) {
        return;
    }
    let Ok(pos) = window.outer_position() else { return; };
    let Ok(size) = window.inner_size() else { return; };
    let captured = WindowGeometry {
        x: pos.x as f64,
        y: pos.y as f64,
        width: size.width as f64,
        height: size.height as f64,
        maximized: window.is_maximized().unwrap_or(false),
    };
    if let Ok(mut g) = GEOMETRY.lock() {
        if let Some(next) = next_geometry(*g, captured) {
            *g = Some(next);
        }
    }
}

/// 决定下一次落盘的几何（纯函数）。
///
/// 最大化尺寸不能当成还原尺寸：已有普通几何时只把状态位置为最大化、几何沿用旧值。
/// 首次启动就最大化再关闭时没有旧值——此时不能落盘最大化几何（否则下次「还原」
/// 得到全屏尺寸），返回 None 让调用方沿用磁盘上已有的普通几何或保持默认居中。
pub fn next_geometry(
    prev: Option<WindowGeometry>,
    captured: WindowGeometry,
) -> Option<WindowGeometry> {
    if !captured.maximized {
        return Some(captured);
    }
    prev.map(|mut prev| {
        prev.maximized = true;
        prev
    })
}

pub fn take_geometry() -> Option<WindowGeometry> {
    GEOMETRY.lock().ok().and_then(|mut g| g.take())
}

/// 待办的最大化：恢复几何时置位，窗口首次 show 后由 `apply_pending_maximize`
/// 消费（单次语义）。
static PENDING_MAXIMIZE: AtomicBool = AtomicBool::new(false);

pub fn request_maximize_on_show() {
    PENDING_MAXIMIZE.store(true, Ordering::Release);
}

/// show 之后调用：有待办最大化则执行。
pub fn apply_pending_maximize(window: &tauri::WebviewWindow) {
    if PENDING_MAXIMIZE.swap(false, Ordering::AcqRel) {
        if let Err(e) = window.maximize() {
            crate::log_warn!("lightweight", "恢复最大化失败: {}", e);
        }
    }
}

/// 把几何应用到窗口（物理像素口径）。返回 `Ok(maximized)` 表示已应用；
/// `Err` 表示未应用（离屏或 set_position/set_size 失败），调用方保持配置默认。
pub fn restore_geometry(
    window: &tauri::WebviewWindow,
    geo: &WindowGeometry,
    monitors: &[MonitorRect],
) -> Result<bool, String> {
    if !geometry_is_on_screen(geo, monitors) {
        return Err("几何已不在任何显示器上".to_string());
    }
    window
        .set_position(tauri::PhysicalPosition::new(geo.x as i32, geo.y as i32))
        .map_err(|e| format!("set_position 失败: {e}"))?;
    window
        .set_size(tauri::PhysicalSize::new(geo.width as u32, geo.height as u32))
        .map_err(|e| format!("set_size 失败: {e}"))?;
    Ok(geo.maximized)
}

/// 显示器矩形，物理像素：(x, y, width, height)
pub type MonitorRect = (i32, i32, u32, u32);

/// 当前所有显示器矩形；取不到时为空，恢复端据此判定离屏并回退默认几何。
pub fn available_monitor_rects(app: &tauri::AppHandle) -> Vec<MonitorRect> {
    app.available_monitors()
        .map(|list| {
            list.iter()
                .map(|m| {
                    let p = m.position();
                    let s = m.size();
                    (p.x, p.y, s.width, s.height)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 把已恢复/已捕获的几何放回内存缓存：后续最大化捕获才有普通几何可沿用，
/// 否则首次启动直接最大化会把最大化尺寸当成还原尺寸落盘。
pub fn remember_geometry(geo: WindowGeometry) {
    if let Ok(mut g) = GEOMETRY.lock() {
        *g = Some(geo);
    }
}

/// 记录并落盘主窗口几何；窗口已销毁时沿用内存值（轻量化常驻后被系统关闭）。
pub fn persist_window_geometry(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(ww) = app.get_webview_window("main") {
        capture_geometry(&ww);
    }
    save_geometry(app);
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
pub fn geometry_is_on_screen(geo: &WindowGeometry, monitors: &[MonitorRect]) -> bool {
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

/// 清掉某个 label 的既有就绪信号。首次启动的 notify_window_ready 会一直留在
/// READY_SIGNALS 里，重建窗口若不清理，ready 门会立刻消费这条旧信号、在前端
/// 挂载完成前显示窗口（白闪）。
pub fn clear_window_ready(label: &str) {
    if let Ok(mut v) = READY_SIGNALS.lock() {
        v.retain(|(l, _)| l != label);
    }
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
        let on = WindowGeometry { x: 100.0, y: 100.0, width: 1360.0, height: 768.0, maximized: false };
        let off = WindowGeometry { x: 5000.0, y: 100.0, width: 1360.0, height: 768.0, maximized: false };
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
        let geo = WindowGeometry { x: 120.0, y: 80.0, width: 2040.0, height: 1152.0, maximized: false };
        let body = serde_json::to_string(&geo).unwrap();
        let back: WindowGeometry = serde_json::from_str(&body).unwrap();
        assert_eq!(back, geo);
    }

    #[test]
    fn 几何序列化_最大化状态往返() {
        let geo = WindowGeometry { x: 120.0, y: 80.0, width: 2040.0, height: 1152.0, maximized: true };
        let body = serde_json::to_string(&geo).unwrap();
        let back: WindowGeometry = serde_json::from_str(&body).unwrap();
        assert_eq!(back, geo);
        assert!(back.maximized);
    }

    #[test]
    fn 几何序列化_旧文件缺maximized字段按false() {
        // 上一版落盘文件没有 maximized 字段，升级后不能反序列化失败
        let geo: WindowGeometry =
            serde_json::from_str(r#"{"x":10.0,"y":20.0,"width":800.0,"height":600.0}"#).unwrap();
        assert!(!geo.maximized);
        assert_eq!((geo.x, geo.y, geo.width, geo.height), (10.0, 20.0, 800.0, 600.0));
    }

    #[test]
    fn 几何离屏校验_必须用物理值_逻辑值在屏内也不算在屏() {
        // 150% DPI：显示器物理 1920 宽；窗口物理 x=2000 已在屏外，
        // 若按旧口径存逻辑值（x≈1333）会被误判为在屏。
        let monitors = vec![(0i32, 0i32, 1920u32, 1080u32)];
        let physical = WindowGeometry { x: 2000.0, y: 100.0, width: 2040.0, height: 1536.0, maximized: false };
        assert!(!geometry_is_on_screen(&physical, &monitors));
        let logical_would_be = WindowGeometry { x: 1333.0, y: 66.0, width: 1360.0, height: 1024.0, maximized: false };
        assert!(
            geometry_is_on_screen(&logical_would_be, &monitors),
            "逻辑口径会误判在屏，本断言锁定恢复端必须按物理像素判断"
        );
    }

    #[test]
    fn 几何离屏校验_副屏负坐标() {
        let monitors = vec![(-1920i32, 0i32, 1920u32, 1080u32), (0, 0, 1920, 1080)];
        let on_left = WindowGeometry { x: -1800.0, y: 50.0, width: 1360.0, height: 768.0, maximized: false };
        assert!(geometry_is_on_screen(&on_left, &monitors));
        let far_left = WindowGeometry { x: -4000.0, y: 50.0, width: 1360.0, height: 768.0, maximized: false };
        assert!(!geometry_is_on_screen(&far_left, &monitors));
    }

    #[test]
    fn 最大化捕获_有普通几何时只置状态位() {
        let prev = WindowGeometry { x: 100.0, y: 80.0, width: 1200.0, height: 800.0, maximized: false };
        let maximized_capture = WindowGeometry { x: 0.0, y: 0.0, width: 2560.0, height: 1440.0, maximized: true };
        let next = super::next_geometry(Some(prev), maximized_capture).expect("应保留普通几何");
        assert_eq!(next.x, 100.0);
        assert_eq!(next.width, 1200.0);
        assert!(next.maximized);
    }

    #[test]
    fn 首次即最大化_不落盘最大化尺寸() {
        let maximized_capture = WindowGeometry { x: 0.0, y: 0.0, width: 2560.0, height: 1440.0, maximized: true };
        assert_eq!(super::next_geometry(None, maximized_capture), None);
    }

    #[test]
    fn 普通窗口捕获_原样落盘() {
        let captured = WindowGeometry { x: 10.0, y: 20.0, width: 900.0, height: 700.0, maximized: false };
        assert_eq!(super::next_geometry(None, captured), Some(captured));
    }

    fn 几何文件_固定在数据目录() {
        let path = geometry_path(std::path::Path::new("data"));
        assert!(path.ends_with(GEOMETRY_FILE));
    }

    #[test]
    fn 就绪信号_清理后不消费首次启动的旧信号() {
        // 用独立 label，避免与并行测试互扰
        signal_window_ready("stale-ready-test");
        clear_window_ready("stale-ready-test");
        assert!(!take_window_ready("stale-ready-test"), "清理后旧信号不得被消费");
        // 清理只作用于指定 label
        signal_window_ready("other-ready-test");
        clear_window_ready("stale-ready-test");
        assert!(take_window_ready("other-ready-test"));
    }
}
