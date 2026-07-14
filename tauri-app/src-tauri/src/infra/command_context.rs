use serde::Serialize;
use tauri::{AppHandle, Manager};
use crate::infra::events::EventBus;
use crate::infra::state::AppState;

/// 命令层公共上下文
///
/// Phase 1 中用于集中传递 AppHandle 与 AppState，减少命令函数反复调用
/// `app_handle.state::<AppState>()` 的样板代码。后续可在此基础上扩展依赖注入。
pub struct CommandContext<'a> {
    pub app: &'a AppHandle,
    pub state: &'a AppState,
}

impl<'a> CommandContext<'a> {
    pub fn new(app: &'a AppHandle, state: &'a AppState) -> Self {
        Self { app, state }
    }

    /// 从 AppHandle 便捷构造（自动获取 state 引用）。
    /// 注意：返回的 CommandContext 借用了 app_handle 的生命周期。
    pub fn from_app(app: &'a AppHandle) -> Self {
        let state = app.state::<AppState>();
        Self { app, state: state.inner() }
    }
}

/// 让 CommandContext 可以直接访问 AppState 的字段/方法
/// 如 `ctx.config` 等同于 `ctx.state.config`
impl<'a> std::ops::Deref for CommandContext<'a> {
    type Target = AppState;
    fn deref(&self) -> &Self::Target { self.state }
}

/// AppHandle 扩展 trait
///
/// 封装常用事件发射方法，所有实现均委托给 `EventBus`，保持事件名/payload 与重构前一致。
pub trait AppHandleExt {
    fn notify_config_changed<C: Serialize + Clone>(&self, config: C) -> Result<(), String>;
    fn notify_update_download_progress<P: Serialize + Clone>(&self, progress: P) -> Result<(), String>;
}

impl AppHandleExt for AppHandle {
    fn notify_config_changed<C: Serialize + Clone>(&self, config: C) -> Result<(), String> {
        EventBus::new(self).emit_config_changed(config)
    }

    fn notify_update_download_progress<P: Serialize + Clone>(&self, progress: P) -> Result<(), String> {
        EventBus::new(self).emit_update_download_progress(progress)
    }
}

/// 在阻塞线程池中执行闭包，统一 `spawn_blocking` + `map_err(JoinError)` 样板。
///
/// 闭包返回 `Result<T, String>`（内部已 map_err），调用方使用
/// `run_blocking(|| { ... }).await?` 即可同时展开 JoinError 与内部 Result。
#[allow(dead_code)] // 第一批仅新增 helper，调用点迁移留给后续批次
pub async fn run_blocking<F, T>(f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}
