use std::time::Instant;
use std::net::IpAddr;
use std::sync::Arc;
use lazy_static::lazy_static;
use arc_swap::ArcSwap;

use super::adapter::{Adapter, AdapterDetail, DisabledAdapter};

pub const MAX_RESPONSE_SIZE: usize = 16 * 1024;
pub const CACHE_TTL_MS: u64 = 15000;

pub(crate) struct AdapterCache {
    adapters: Arc<Vec<Adapter>>,
    details: Arc<Vec<AdapterDetail>>,
    disabled: Arc<Vec<DisabledAdapter>>,
    time: Instant,
}

pub(crate) struct GatewayCacheEntry {
    pub time: Instant,
    pub gateway: String,
    pub adapter_name: String,
}

pub(crate) struct PortalCacheEntry {
    pub time: Instant,
    pub status: super::portal::PortalStatus,
    pub adapter_ip: String,
}

pub(crate) struct NetworkCache {
    pub adapter: ArcSwap<Option<AdapterCache>>,
    pub gateway: ArcSwap<Option<GatewayCacheEntry>>,
    pub portal: ArcSwap<Option<PortalCacheEntry>>,
    pub portal_url: ArcSwap<String>,
}

impl NetworkCache {
    fn new() -> Self {
        Self {
            adapter: ArcSwap::from(Arc::new(None)),
            gateway: ArcSwap::from(Arc::new(None)),
            portal: ArcSwap::from(Arc::new(None)),
            portal_url: ArcSwap::from(Arc::new("http://10.1.99.100".to_string())),
        }
    }

    pub fn clear_all(&self) {
        self.adapter.store(Arc::new(None));
        self.gateway.store(Arc::new(None));
        self.portal.store(Arc::new(None));
    }

    pub fn clear_adapter_only(&self) {
        self.adapter.store(Arc::new(None));
    }

    pub fn clear_portal_only(&self) {
        self.portal.store(Arc::new(None));
    }
}

lazy_static! {
    pub(crate) static ref NET_CACHE: NetworkCache = NetworkCache::new();
}

pub fn update_portal_url(url: &str) {
    if !url.is_empty() {
        NET_CACHE.portal_url.store(Arc::new(url.to_string()));
    }
}

pub fn clear_adapter_cache() {
    NET_CACHE.clear_all();
}

pub fn clear_adapter_cache_only() {
    NET_CACHE.clear_adapter_only();
}

pub fn clear_portal_cache() {
    NET_CACHE.clear_portal_only();
}

pub(crate) fn get_cached_adapters() -> Option<(Arc<Vec<Adapter>>, Arc<Vec<AdapterDetail>>, Arc<Vec<DisabledAdapter>>)> {
    let cache_arc = NET_CACHE.adapter.load();
    match cache_arc.as_ref() {
        Some(c) if c.time.elapsed().as_millis() < CACHE_TTL_MS as u128 => {
            Some((c.adapters.clone(), c.details.clone(), c.disabled.clone()))
        }
        _ => None,
    }
}

pub(crate) fn set_adapters_cache(adapters: Vec<Adapter>, details: Vec<AdapterDetail>, disabled: Vec<DisabledAdapter>) {
    NET_CACHE.adapter.store(Arc::new(Some(AdapterCache {
        adapters: Arc::new(adapters),
        details: Arc::new(details),
        disabled: Arc::new(disabled),
        time: Instant::now(),
    })));
}

pub(crate) fn create_safe_http_client(timeout: std::time::Duration, local_addr: Option<IpAddr>) -> Result<reqwest::blocking::Client, String> {
    build_http_client(timeout, local_addr)
}

fn build_http_client(timeout: std::time::Duration, local_addr: Option<IpAddr>) -> Result<reqwest::blocking::Client, String> {
    let mut builder = reqwest::blocking::Client::builder()
        .min_tls_version(reqwest::tls::Version::TLS_1_3)
        .timeout(timeout)
        .connect_timeout(std::time::Duration::from_secs(5))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::limited(5))
        .pool_max_idle_per_host(4)
        .pool_idle_timeout(std::time::Duration::from_secs(60));

    if let Some(ip) = local_addr {
        builder = builder.local_address(ip);
    }

    match builder.build() {
        Ok(c) => Ok(c),
        Err(_) => {
            let mut fallback = reqwest::blocking::Client::builder()
                .min_tls_version(reqwest::tls::Version::TLS_1_2)
                .timeout(timeout)
                .connect_timeout(std::time::Duration::from_secs(5))
                .no_proxy()
                .redirect(reqwest::redirect::Policy::limited(5))
                .pool_max_idle_per_host(4)
                .pool_idle_timeout(std::time::Duration::from_secs(60));
            if let Some(ip) = local_addr {
                fallback = fallback.local_address(ip);
            }
            fallback.build().map_err(|e| format!("创建HTTP客户端失败: {}", e))
        }
    }
}
