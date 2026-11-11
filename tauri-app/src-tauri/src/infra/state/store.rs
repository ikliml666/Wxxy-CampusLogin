use std::sync::Arc;
use arc_swap::ArcSwap;
use parking_lot::Mutex;
use crate::config::model::Config;

/// 配置存储，封装 `ArcSwap<Config>`。
///
/// `write_lock` 是内存写事务锁：所有 `store` / `update` / 持久化提交共用，
/// 避免持锁落盘期间被锁外 CAS 修改后又被整份快照覆盖。
pub struct ConfigStore {
    inner: ArcSwap<Config>,
    write_lock: Mutex<()>,
}

impl ConfigStore {
    pub fn new(config: Config) -> Self {
        Self {
            inner: ArcSwap::from(Arc::new(config)),
            write_lock: Mutex::new(()),
        }
    }

    /// 加载当前配置的不可变快照。
    pub fn load(&self) -> Arc<Config> {
        self.inner.load_full()
    }

    /// 兼容旧名的别名方法。
    pub fn load_full(&self) -> Arc<Config> {
        self.load()
    }

    /// 直接替换整个配置。
    pub fn store(&self, config: Config) -> Arc<Config> {
        let _guard = self.write_lock.lock();
        self.store_locked(config)
    }

    fn store_locked(&self, config: Config) -> Arc<Config> {
        let new_arc = Arc::new(config);
        self.inner.store(new_arc.clone());
        new_arc
    }

    /// 在内存写事务锁内修改配置并回调持久化；持久化成功后才替换内存。
    ///
    /// 回调期间 `update` / `store` 都会等待，因此落盘前的快照不会被锁外修改
    /// 覆盖；`persist` 失败时内存保持原值。回调内不得再调用本存储的写方法。
    pub fn update_persisted<F, P>(
        &self,
        mutate: F,
        persist: P,
    ) -> Result<Arc<Config>, String>
    where
        F: FnOnce(&mut Config) -> Result<(), String>,
        P: FnOnce(&Config) -> Result<(), String>,
    {
        let _guard = self.write_lock.lock();
        let mut fresh = (*self.load()).clone();
        mutate(&mut fresh)?;
        persist(&fresh)?;
        Ok(self.store_locked(fresh))
    }

    /// 使用 CAS 循环原子更新配置，避免 TOCTOU 竞态条件。
    /// 仅测试使用；生产写口统一走 `update_persisted`。
    #[cfg(test)]
    pub fn update<F>(&self, f: F) -> Arc<Config>
    where
        F: Fn(&mut Config),
    {
        let _guard = self.write_lock.lock();
        loop {
            let current = self.inner.load_full();
            let mut new_cfg = (*current).clone();
            f(&mut new_cfg);
            let new_arc = Arc::new(new_cfg);
            let prev = self.inner.compare_and_swap(&current, new_arc);
            if Arc::ptr_eq(&current, &prev) {
                return self.inner.load_full();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_store_loads_default() {
        let store = ConfigStore::new(Config::default());
        let cfg = store.load();
        assert!(cfg.user.is_empty());
    }

    #[test]
    fn config_store_updates_atomically() {
        let store = ConfigStore::new(Config::default());
        store.update(|cfg| cfg.user = "test".to_string());
        assert_eq!(store.load().user, "test");
    }

    #[test]
    fn 持久化提交不得覆盖并发内存更新() {
        use std::sync::{Arc, Barrier};

        let store = Arc::new(ConfigStore::new(Config::default()));
        let barrier = Arc::new(Barrier::new(2));

        let persisting = {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                store
                    .update_persisted(
                        |cfg| {
                            cfg.user = "落盘甲".to_string();
                            Ok(())
                        },
                        |_cfg| {
                            barrier.wait();
                            std::thread::sleep(std::time::Duration::from_millis(20));
                            Ok(())
                        },
                    )
                    .unwrap();
            })
        };

        let concurrent = {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                store.update(|cfg| cfg.portal_url = "并发乙".to_string());
            })
        };

        persisting.join().unwrap();
        concurrent.join().unwrap();
        let final_cfg = store.load();
        assert_eq!(final_cfg.user, "落盘甲");
        assert_eq!(final_cfg.portal_url, "并发乙", "落盘提交不得吞掉并发内存更新");
    }

    #[test]
    fn 落盘失败不得发布内存() {
        let store = ConfigStore::new(Config::default());
        let result = store.update_persisted(
            |cfg| {
                cfg.user = "不应发布".to_string();
                Ok(())
            },
            |_cfg| Err("磁盘只读".to_string()),
        );

        assert_eq!(result.unwrap_err(), "磁盘只读");
        assert!(store.load().user.is_empty(), "落盘失败时内存必须保持原值");
    }
}
