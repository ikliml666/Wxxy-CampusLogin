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
    wait_for_adapter, dhcp_renew_wired_only, dhcp_release_renew_all, dhcp_release_renew_single,
    ensure_ethernet_ip_for_login,
    is_blacklisted, check_gateway_reachable,
    is_same_subnet_18,
    get_wireless_ssid,
    get_wired_network_profile,
};

pub use client::update_portal_url;

pub use quality::check_network_quality_async;
