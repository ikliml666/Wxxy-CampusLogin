//! 系统网络变更事件（Windows）：注册 IP 接口变更通知，把「用户在系统里启用/禁用
//! 网卡、改跃点、DHCP 重新拿地址」即时推给前端。此前前端只能等 60s 兜底轮询，
//! 在系统设置里改网卡后界面会长时间停在旧状态。
//!
//! 只做一件事：收到内核通知后 emit 一个 `network-changed`，前端收到即强制重探。
//! 具体刷新内容（适配器列表 / 跃点 / 当前出站）由前端决定，后端不猜测。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter};

/// 前端监听的事件名（与 `tauriApi.onNetworkChanged` 对应）。
pub const NETWORK_CHANGED_EVENT: &str = "network-changed";

/// 去抖窗口：一次网卡启停会连发多个通知（接口、地址、路由各一条），
/// 300ms 内合并成一次刷新，避免前端被刷屏。
const DEBOUNCE_MS: u64 = 300;

static APP: OnceLock<AppHandle> = OnceLock::new();
static LAST_EMIT_MS: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn emit_changed() {
    let Some(app) = APP.get() else {
        return;
    };
    let now = now_ms();
    if now.saturating_sub(LAST_EMIT_MS.load(Ordering::Relaxed)) < DEBOUNCE_MS {
        return;
    }
    LAST_EMIT_MS.store(now, Ordering::Relaxed);
    let _ = app.emit(NETWORK_CHANGED_EVENT, ());
}

/// 注册系统网络变更监听（进程内只生效一次）。
/// 订阅句柄故意不保存也不取消：进程生命周期内一直有效，退出时由系统回收。
#[cfg(target_os = "windows")]
pub fn watch_network_changes(app: AppHandle) {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::NetworkManagement::IpHelper::{
        NotifyIpInterfaceChange, MIB_IPINTERFACE_ROW, MIB_NOTIFICATION_TYPE,
    };
    use windows::Win32::Networking::WinSock::AF_UNSPEC;

    unsafe extern "system" fn on_ip_interface_change(
        _caller: *const core::ffi::c_void,
        _row: *const MIB_IPINTERFACE_ROW,
        _kind: MIB_NOTIFICATION_TYPE,
    ) {
        emit_changed();
    }

    if APP.set(app).is_err() {
        return;
    }
    let mut handle = HANDLE::default();
    let result = unsafe {
        NotifyIpInterfaceChange(
            AF_UNSPEC,
            Some(on_ip_interface_change),
            None::<*const core::ffi::c_void>,
            false,
            &mut handle,
        )
    };
    if result == windows::Win32::Foundation::WIN32_ERROR(0) {
        crate::log_info!(
            "network",
            "已注册系统网络变更通知（前端事件 {NETWORK_CHANGED_EVENT}）"
        );
    } else {
        crate::log_warn!("network", "注册系统网络变更通知失败: 错误码 {}", result.0);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn watch_network_changes(_app: AppHandle) {}
