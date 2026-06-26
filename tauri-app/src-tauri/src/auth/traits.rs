use crate::config::model::Config;
use crate::network::Adapter;

/// 主/副适配器名称解析器
///
/// 封装 `network::resolve_adapter_names`，便于登录/注销流程统一调用。
pub struct DefaultAdapterResolver;

impl DefaultAdapterResolver {
    pub fn resolve_adapter_names(&self, adapters: &[Adapter], config: &Config) -> (String, String) {
        crate::network::resolve_adapter_names(adapters, config)
    }
}
