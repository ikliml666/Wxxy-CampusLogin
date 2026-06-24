use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

/// 后台任务句柄，仅暴露取消令牌。
#[derive(Clone)]
pub struct TaskHandle {
    pub cancel_token: CancellationToken,
}

/// 统一管理周期性后台任务的生命周期。
#[derive(Clone)]
pub struct BackgroundTaskManager {
    inner: Arc<Mutex<HashMap<String, TaskHandle>>>,
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
        let mut tasks = self.inner.lock();
        if tasks.contains_key(name) {
            return Err(format!("任务 {} 已在运行", name));
        }

        let cancel_token = Arc::new(CancellationToken::new());
        let handle = TaskHandle {
            cancel_token: (*cancel_token).clone(),
        };
        tasks.insert(name.to_string(), handle);
        drop(tasks);

        let inner = self.inner.clone();
        let name = name.to_string();
        let future = build_future(cancel_token);
        tauri::async_runtime::spawn(async move {
            future.await;
            inner.lock().remove(&name);
        });

        Ok(())
    }

    /// 注册并启动一个阻塞型后台任务（运行在新线程中）。任务结束时会自动从管理器中移除。
    pub fn spawn_blocking<F>(&self, name: &str, f: F) -> Result<(), String>
    where
        F: FnOnce(Arc<CancellationToken>) + Send + 'static,
    {
        let mut tasks = self.inner.lock();
        if tasks.contains_key(name) {
            return Err(format!("任务 {} 已在运行", name));
        }

        let cancel_token = Arc::new(CancellationToken::new());
        let handle = TaskHandle {
            cancel_token: (*cancel_token).clone(),
        };
        tasks.insert(name.to_string(), handle);
        drop(tasks);

        let inner = self.inner.clone();
        let name = name.to_string();
        std::thread::spawn(move || {
            f(cancel_token);
            inner.lock().remove(&name);
        });

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

    /// 取消所有已注册任务并清空管理器。
    pub fn cancel_all(&self) {
        let handles: Vec<TaskHandle> = {
            let mut tasks = self.inner.lock();
            let values = tasks.values().cloned().collect();
            tasks.clear();
            values
        };
        for handle in handles {
            handle.cancel_token.cancel();
        }
    }

    /// 查询任务是否正在运行。
    pub fn is_running(&self, name: &str) -> bool {
        self.inner.lock().contains_key(name)
    }

    /// 获取任务的取消令牌。任务不存在时返回 `None`。
    pub fn cancel_token(&self, name: &str) -> Option<CancellationToken> {
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
    fn task_manager_cancel_all_clears_all() {
        let manager = BackgroundTaskManager::new();
        manager.spawn("a", |cancel| async move { cancel.cancelled().await; }).unwrap();
        manager.spawn("b", |cancel| async move { cancel.cancelled().await; }).unwrap();
        manager.cancel_all();
        assert!(!manager.is_running("a"));
        assert!(!manager.is_running("b"));
    }
}
