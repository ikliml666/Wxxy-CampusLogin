//! 系统网络变更事件（Windows）：注册 IP 接口与地址变更通知，把「用户在系统里
//! 启用/禁用网卡、改跃点、DHCP 更换地址」即时推给前端。此前前端只能等 60s 兜底
//! 轮询，在系统设置里改网卡后界面会长时间停在旧状态。
//!
//! 只做一件事：收到内核通知后 emit 一个 `network-changed`，前端收到即强制重探。
//! 具体刷新内容（适配器列表 / 跃点 / 当前出站）由前端决定，后端不猜测。

/// 前端监听的事件名（与 `tauriApi.onNetworkChanged` 对应）。
pub const NETWORK_CHANGED_EVENT: &str = "network-changed";

#[cfg(target_os = "windows")]
mod imp {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::OnceLock;
    use tauri::{AppHandle, Emitter};

    /// 合并窗口：一次网卡启停会连发多个通知（接口、地址、路由各一条），
    /// 窗口内合并成一次刷新，避免前端被刷屏。
    const DEBOUNCE_MS: u64 = 300;

    static APP: OnceLock<AppHandle> = OnceLock::new();
    static LAST_EMIT_MS: AtomicU64 = AtomicU64::new(0);
    /// 窗口内是否还有未发出的变化（必须补发，丢了界面会停在中间态）
    static PENDING: AtomicBool = AtomicBool::new(false);
    /// 补发线程是否已在排队（避免每个事件都开线程）
    static TRAILING: AtomicBool = AtomicBool::new(false);

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    pub(super) fn emit_changed() {
        if APP.get().is_none() {
            return;
        }
        let now = now_ms();
        if now.saturating_sub(LAST_EMIT_MS.load(Ordering::Relaxed)) < DEBOUNCE_MS {
            // 窗口内的变化不能丢：标记待补发，由窗口收尾统一发这一次
            PENDING.store(true, Ordering::Release);
            return;
        }
        emit_now(now);
        schedule_trailing();
    }

    fn emit_now(now: u64) {
        LAST_EMIT_MS.store(now, Ordering::Release);
        if let Some(app) = APP.get() {
            let _ = app.emit(super::NETWORK_CHANGED_EVENT, ());
        }
    }

    /// 窗口收尾时补发被合并掉的变化——「合并成一次」而非「丢掉后面的」。
    /// 收尾后若又攒了新变化，续排下一轮。
    fn schedule_trailing() {
        if TRAILING.swap(true, Ordering::AcqRel) {
            return;
        }
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(DEBOUNCE_MS + 20));
            if PENDING.swap(false, Ordering::AcqRel) {
                emit_now(now_ms());
            }
            TRAILING.store(false, Ordering::Release);
            if PENDING.load(Ordering::Acquire) {
                schedule_trailing();
            }
        });
    }

    pub(super) fn register(app: AppHandle) {
        use windows::Win32::Foundation::{HANDLE, WIN32_ERROR};
        use windows::Win32::NetworkManagement::IpHelper::{
            MIB_IPINTERFACE_ROW, MIB_NOTIFICATION_TYPE, MIB_UNICASTIPADDRESS_ROW,
            NotifyIpInterfaceChange, NotifyUnicastIpAddressChange,
        };
        use windows::Win32::Networking::WinSock::AF_UNSPEC;

        // 接口级通知：启用/禁用、跃点改动
        unsafe extern "system" fn on_ip_interface_change(
            _caller: *const core::ffi::c_void,
            _row: *const MIB_IPINTERFACE_ROW,
            _kind: MIB_NOTIFICATION_TYPE,
        ) {
            emit_changed();
        }
        // 地址级通知：DHCP 换租、静态地址被改。纯地址变化不保证触发接口级回调，
        // 两条都订才覆盖「系统里改了网卡」的全部情形。
        unsafe extern "system" fn on_unicast_address_change(
            _caller: *const core::ffi::c_void,
            _row: *const MIB_UNICASTIPADDRESS_ROW,
            _kind: MIB_NOTIFICATION_TYPE,
        ) {
            emit_changed();
        }

        if APP.set(app).is_err() {
            return;
        }
        let mut iface_handle = HANDLE::default();
        let iface_rc = unsafe {
            NotifyIpInterfaceChange(
                AF_UNSPEC,
                Some(on_ip_interface_change),
                None::<*const core::ffi::c_void>,
                false,
                &mut iface_handle,
            )
        };
        let mut addr_handle = HANDLE::default();
        let addr_rc = unsafe {
            NotifyUnicastIpAddressChange(
                AF_UNSPEC,
                Some(on_unicast_address_change),
                None::<*const core::ffi::c_void>,
                false,
                &mut addr_handle,
            )
        };
        // 句柄故意不关闭：进程生命周期内保持订阅，退出时由系统回收
        for (what, rc) in [("接口", iface_rc), ("地址", addr_rc)] {
            if rc == WIN32_ERROR(0) {
                crate::log_info!(
                    "network",
                    "已注册系统{what}变更通知（前端事件 {}）",
                    super::NETWORK_CHANGED_EVENT
                );
            } else {
                crate::log_warn!("network", "注册系统{what}变更通知失败: 错误码 {}", rc.0);
            }
        }
    }
}

/// 注册系统网络变更监听（进程内只生效一次）。非 Windows 平台为空实现。
pub fn watch_network_changes(app: tauri::AppHandle) {
    #[cfg(target_os = "windows")]
    imp::register(app);
    #[cfg(not(target_os = "windows"))]
    let _ = app;
}
