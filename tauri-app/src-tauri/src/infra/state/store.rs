use std::sync::Arc;
use arc_swap::ArcSwap;
use crate::config::model::Config;

/// 配置存储，封装 `ArcSwap<Config>` 并提供 CAS 更新能力。
pub struct ConfigStore {
    inner: ArcSwap<Config>,
}

impl ConfigStore {
    pub fn new(config: Config) -> Self {
        Self {
            inner: ArcSwap::from(Arc::new(config)),
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
        let new_arc = Arc::new(config);
        self.inner.store(new_arc.clone());
        new_arc
    }

    /// 使用 CAS 循环原子更新配置，避免 TOCTOU 竞态条件。
    pub fn update<F>(&self, f: F) -> Arc<Config>
    where
        F: Fn(&mut Config),
    {
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
}
