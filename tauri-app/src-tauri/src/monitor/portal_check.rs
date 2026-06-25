use tauri::AppHandle;
use crate::network::Adapter;
use crate::auth::portal::check_portal_full;
use crate::infra::events::EventBus;

pub(super) enum PortalCheckResult {
    Success {
        online: bool,
        message: String,
        reachable: bool,
        login_available: bool,
    },
    Error {
        is_request_failed: bool,
    },
    NotFound,
}

impl PortalCheckResult {
    pub(super) fn online(&self) -> bool {
        match self {
            PortalCheckResult::Success { online, .. } => *online,
            _ => false,
        }
    }

    pub(super) fn message(&self) -> &str {
        match self {
            PortalCheckResult::Success { message, .. } => message,
            PortalCheckResult::Error { .. } => "检测失败",
            PortalCheckResult::NotFound => "未找到主适配器",
        }
    }

    pub(super) fn reachable(&self) -> bool {
        match self {
            PortalCheckResult::Success { reachable, .. } => *reachable,
            _ => false,
        }
    }

    pub(super) fn login_available(&self) -> bool {
        match self {
            PortalCheckResult::Success { login_available, .. } => *login_available,
            _ => false,
        }
    }
}

pub(super) fn check_adapter_portal(
    adapter: &Adapter,
    app_handle: &AppHandle,
) -> PortalCheckResult {
    match check_portal_full(&adapter.ip, Some(&adapter.name), None, None, None) {
        Ok(ps) => {
            if ps.error_kind.as_deref() == Some("request_failed") {
                crate::log_warn!("network", "{} Portal页面检测请求失败: {}", adapter.name, ps.message);
                let event_bus = EventBus::new(app_handle);
                let _ = event_bus.emit_login_log(
                    &format!("{} Portal页面检测请求失败: {}", adapter.name, ps.message),
                    "error",
                );
                PortalCheckResult::Error { is_request_failed: true }
            } else {
                PortalCheckResult::Success {
                    online: ps.online,
                    message: ps.message,
                    reachable: ps.reachable,
                    login_available: ps.login_available,
                }
            }
        }
        Err(e) => {
            crate::log_warn!("background", "{} Portal页面检测异常: {}", adapter.name, e);
            let event_bus = EventBus::new(app_handle);
            let _ = event_bus.emit_login_log(
                &format!("{} Portal页面检测异常: {}", adapter.name, e),
                "error",
            );
            PortalCheckResult::Error { is_request_failed: false }
        }
    }
}
