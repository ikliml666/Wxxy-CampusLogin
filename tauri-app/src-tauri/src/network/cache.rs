use std::net::IpAddr;
use std::sync::Arc;
use arc_swap::ArcSwap;

lazy_static::lazy_static! {
    pub(crate) static ref PORTAL_URL: ArcSwap<String> = ArcSwap::from(Arc::new("http://10.1.99.100".to_string()));
}

pub fn update_portal_url(url: &str) {
    if !url.is_empty() {
        PORTAL_URL.store(Arc::new(url.to_string()));
    }
}

pub fn create_safe_http_client(timeout: std::time::Duration, local_addr: Option<IpAddr>) -> Result<reqwest::blocking::Client, String> {
    let mut builder = reqwest::blocking::Client::builder()
        .min_tls_version(reqwest::tls::Version::TLS_1_3)
        .timeout(timeout)
        .connect_timeout(std::time::Duration::from_secs(5))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::limited(5));

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
                .redirect(reqwest::redirect::Policy::limited(5));
            if let Some(ip) = local_addr {
                fallback = fallback.local_address(ip);
            }
            fallback.build().map_err(|e| format!("创建HTTP客户端失败: {}", e))
        }
    }
}
