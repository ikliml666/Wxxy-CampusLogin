use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// 统一事件总线
///
/// Phase 1 中封装所有 `app_handle.emit` 调用，保证事件名与 payload 与重构前完全一致。
/// 后续 Phase 可通过 trait 化进一步支持 mock 注入。
pub struct EventBus<'a> {
    app_handle: &'a AppHandle,
}

impl<'a> EventBus<'a> {
    pub fn new(app_handle: &'a AppHandle) -> Self {
        Self { app_handle }
    }

    fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) -> Result<(), String> {
        self.app_handle.emit(event, payload).map_err(|e| e.to_string())
    }

    /// 登录/注销操作日志事件（前端 LogPanel 显示）
    pub fn emit_login_log(&self, message: &str, log_type: &str) -> Result<(), String> {
        self.emit("login-log", serde_json::json!({
            "message": message,
            "type": log_type,
        }))
    }

    /// 后台检测综合结果事件
    pub fn emit_background_check_result<S: Serialize + Clone>(&self, payload: S) -> Result<(), String> {
        self.emit("background-check-result", payload)
    }

    /// 自动登录/断线重连结果事件
    pub fn emit_auto_login_result(&self, success: bool, message: &str, skipped: bool) -> Result<(), String> {
        self.emit("auto-login-result", serde_json::json!({
            "success": success,
            "message": message,
            "skipped": skipped,
        }))
    }

    /// 网络质量检测结果事件
    pub fn emit_network_quality_result<S: Serialize + Clone>(&self, payload: S) -> Result<(), String> {
        self.emit("network-quality-result", payload)
    }

    /// 配置变更事件（无 payload 简略版）
    pub fn emit_config_changed_empty(&self) -> Result<(), String> {
        self.emit("config-changed", serde_json::json!({}))
    }

    /// 配置变更事件（含完整配置对象）
    pub fn emit_config_changed<S: Serialize + Clone>(&self, payload: S) -> Result<(), String> {
        self.emit("config-changed", payload)
    }

    /// 适配器列表/详情/禁用列表变更事件
    pub fn emit_adapters_changed<S: Serialize + Clone>(&self, adapters: S) -> Result<(), String> {
        self.emit("adapters-changed", adapters)
    }

    /// 系统通知事件
    pub fn emit_system_notification(&self, title: &str, body: &str) -> Result<(), String> {
        self.emit("system-notification", serde_json::json!({
            "title": title,
            "body": body,
        }))
    }

    /// 更新相关事件
    pub fn emit_update_download_progress<S: Serialize + Clone>(&self, payload: S) -> Result<(), String> {
        self.emit("update-download-progress", payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // EventBus 目前依赖 AppHandle，无法在单元测试中直接构造。
    // 本模块保留结构测试占位，确保 trait/struct 编译正确。
    // 后续 Phase 可将 EventBus 抽象为 trait，并在此提供手写 mock。

    #[test]
    fn event_bus_struct_size_check() {
        // 编译期保证：EventBus 只持有引用，大小为指针大小
        assert_eq!(std::mem::size_of::<EventBus<'_>>(), std::mem::size_of::<&AppHandle>());
    }
}
