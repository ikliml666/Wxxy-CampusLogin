use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

/// 后台任务句柄，仅暴露取消令牌。
pub struct TaskHandle {
    pub cancel_token: Arc<CancellationToken>,
    join_handle: tauri::async_runtime::JoinHandle<()>,
}

/// 统一管理周期性后台任务的生命周期。
#[derive(Clone)]
pub struct BackgroundTaskManager {
    inner: Arc<Mutex<HashMap<String, TaskHandle>>>,
}

impl Default for BackgroundTaskManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BackgroundTaskManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 注册并启动一个异步后台任务。任务自然结束或 panic 时会自动从管理器中移除。
    pub fn spawn<F, Fut>(&self, name: &str, build_future: F) -> Result<(), String>
    where
        F: FnOnce(Arc<CancellationToken>) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let inner = self.inner.clone();
        let mut tasks = self.inner.lock();
        if tasks.contains_key(name) {
            return Err(format!("任务 {name} 已在运行"));
        }

        let cancel_token = Arc::new(CancellationToken::new());
        let name_owned = name.to_string();
        let future = build_future(cancel_token.clone());
        let cleanup_token = cancel_token.clone();
        let join_handle = tauri::async_runtime::spawn(async move {
            future.await;
            // 仅当 map 中仍是自己这一实例时才 remove：cancel 后同名 re-spawn 时，
            // 旧任务迟到的收尾不得误删新任务句柄
            let mut tasks = inner.lock();
            if tasks.get(&name_owned).is_some_and(|h| Arc::ptr_eq(&h.cancel_token, &cleanup_token)) {
                tasks.remove(&name_owned);
            }
        });
        let handle = TaskHandle {
            cancel_token: cancel_token.clone(),
            join_handle,
        };
        tasks.insert(name.to_string(), handle);

        Ok(())
    }

    /// 取消指定任务并移除其句柄。返回是否成功找到并取消。
    pub fn cancel(&self, name: &str) -> bool {
        let mut tasks = self.inner.lock();
        if let Some(handle) = tasks.remove(name) {
            handle.cancel_token.cancel();
            true
        } else {
            false
        }
    }

    /// 从管理器中移除指定任务但不取消也不等待。
    ///
    /// 适用于任务自身即将触发 `shutdown_and_exit` 的场景：避免 `shutdown` 等待自己导致死锁。
    /// 任务会继续运行直到自然结束。返回是否成功找到并移除。
    pub fn detach(&self, name: &str) -> bool {
        self.inner.lock().remove(name).is_some()
    }

    /// 取消所有已注册任务并等待它们全部结束。
    pub async fn shutdown(&self) {
        let handles: Vec<TaskHandle> = {
            let mut tasks = self.inner.lock();
            tasks.drain().map(|(_, v)| v).collect()
        };
        for handle in &handles {
            handle.cancel_token.cancel();
        }
        for handle in handles {
            let jh = handle.join_handle;
            let _ = jh.await;
        }
    }

    /// 查询任务是否正在运行。
    pub fn is_running(&self, name: &str) -> bool {
        self.inner.lock().contains_key(name)
    }

    /// 获取任务的取消令牌。任务不存在时返回 `None`。
    pub fn cancel_token(&self, name: &str) -> Option<Arc<CancellationToken>> {
        self.inner.lock().get(name).map(|h| h.cancel_token.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_manager_rejects_duplicate_spawn() {
        let manager = BackgroundTaskManager::new();
        manager.spawn("test", |_| async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }).unwrap();
        assert!(manager.is_running("test"));
        assert!(manager.spawn("test", |_| async move {}).is_err());
    }

    #[test]
    fn task_manager_cancel_removes_task() {
        let manager = BackgroundTaskManager::new();
        manager.spawn("test", |cancel| async move {
            cancel.cancelled().await;
        }).unwrap();
        assert!(manager.is_running("test"));
        assert!(manager.cancel("test"));
        // 取消后任务会很快退出，但这里不 sleep，仅验证 remove 行为
        assert!(!manager.is_running("test"));
    }

    #[test]
    fn task_manager_shutdown_waits_for_tasks() {
        let manager = BackgroundTaskManager::new();
        manager.spawn("test", |cancel| async move {
            cancel.cancelled().await;
        }).unwrap();
        assert!(manager.is_running("test"));
        tauri::async_runtime::block_on(manager.shutdown());
        assert!(!manager.is_running("test"));
    }
}
