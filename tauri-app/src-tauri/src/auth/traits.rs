use crate::config::model::Config;
use crate::network::Adapter;

/// 主/副适配器名称解析抽象
///
/// 将 `network::resolve_adapter_names` 抽象为 trait，便于 Phase 3 统一登录/注销的适配器选择逻辑。
pub trait AdapterResolver: Send + Sync {
    fn resolve_adapter_names(&self, adapters: &[Adapter], config: &Config) -> (String, String);
}

pub struct DefaultAdapterResolver;

impl AdapterResolver for DefaultAdapterResolver {
    fn resolve_adapter_names(&self, adapters: &[Adapter], config: &Config) -> (String, String) {
        crate::network::resolve_adapter_names(adapters, config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub struct MockAdapterResolver {
        primary: String,
        secondary: String,
    }

    impl MockAdapterResolver {
        pub fn new(primary: &str, secondary: &str) -> Self {
            Self { primary: primary.to_string(), secondary: secondary.to_string() }
        }
    }

    impl AdapterResolver for MockAdapterResolver {
        fn resolve_adapter_names(&self, _adapters: &[Adapter], _config: &Config) -> (String, String) {
            (self.primary.clone(), self.secondary.clone())
        }
    }

    #[test]
    fn mock_adapter_resolver_returns_names() {
        let resolver = MockAdapterResolver::new("以太网", "WLAN");
        let (a1, a2) = resolver.resolve_adapter_names(&[], &Config::default());
        assert_eq!(a1, "以太网");
        assert_eq!(a2, "WLAN");
    }

    #[test]
    fn default_adapter_resolver_trait_object() {
        let resolver: Box<dyn AdapterResolver> = Box::new(DefaultAdapterResolver);
        let _ = resolver;
    }
}
