mod cache;
mod adapter;
mod portal;
mod login_request;
mod quality;

pub use adapter::{
    Adapter, AdapterDetail, DisabledAdapter,
    get_adapters_cached, get_adapters_force, get_disabled_adapters_cached,
    get_disabled_adapters_force, get_adapter_details_cached, get_all_adapters_cached,
    enable_adapter, resolve_adapter_names, select_adapter,
    wait_for_adapter, dhcp_renew_wired_only,
};

pub use portal::check_portal_full;

pub use login_request::do_login_with_retry;

pub use quality::check_network_quality_async;

pub use cache::{
    update_portal_url, clear_adapter_cache,
    cleanup_expired_http_clients,
};
