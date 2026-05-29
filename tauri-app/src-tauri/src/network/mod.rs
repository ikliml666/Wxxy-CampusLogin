pub mod adapter;
pub mod client;
pub mod dns;
pub mod quality;
pub mod timing;

pub use adapter::{
    Adapter, AdapterDetail, DisabledAdapter,
    get_adapters_cached, get_adapters_force, get_disabled_adapters_cached,
    get_adapter_details_cached,
    get_all_adapters_force,
    enable_adapter, resolve_adapter_names, select_adapter,
    wait_for_adapter, dhcp_renew_wired_only, dhcp_release_renew_all,
    is_blacklisted, check_gateway_reachable,
    is_same_subnet_18,
    get_connected_network_names,
};

pub use client::update_portal_url;

pub use quality::check_network_quality_async;
