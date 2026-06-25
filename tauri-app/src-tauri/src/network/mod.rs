pub mod adapter;
pub mod adapter_cache;
pub mod client;
pub mod dhcp;
pub mod discovery;
pub mod dns;
pub mod quality;
pub mod subnet;
pub mod timing;

pub use adapter::{
    Adapter, AdapterDetail, DisabledAdapter,
    resolve_adapter_names, select_adapter,
    dhcp_renew_wired_only, dhcp_release_renew_all, dhcp_release_renew_single,
    ensure_ethernet_ip_for_login,
    is_blacklisted, check_gateway_reachable, check_gateway_reachable_from,
    is_same_subnet_18,
    get_wireless_ssid,
    get_wired_network_profile,
    find_by_name, find_with_valid_ip, find_dual_adapters,
    is_secondary_adapter_enabled,
};

pub use adapter_cache::{
    get_adapters_cached, get_adapters_cached_async, get_adapters_force,
    get_disabled_adapters_cached, get_adapter_details_cached,
    get_all_adapters_force, enable_adapter,
    wait_for_adapter,
};

pub use client::update_portal_url;

pub use quality::check_network_quality_async;
