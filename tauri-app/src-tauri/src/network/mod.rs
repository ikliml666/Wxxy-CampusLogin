mod cache;
pub mod adapter;
mod portal;
mod login_request;
mod quality;

pub use adapter::{
    Adapter, AdapterDetail, DisabledAdapter,
    get_adapters_cached, get_adapters_force, get_disabled_adapters_cached,
    get_disabled_adapters_force, get_adapter_details_cached, get_all_adapters_cached,
    enable_adapter, resolve_adapter_names, select_adapter,
    wait_for_adapter, dhcp_renew_wired_only, dhcp_release_renew_all,
    is_blacklisted, check_gateway_reachable,
    is_same_subnet_18,
    get_connected_network_names,
    set_mac_via_registry, remove_mac_from_registry,
    dhcp_release, dhcp_renew, netsh_disable, netsh_enable,
    poll_ip_change, poll_adapter_has_ip,
};

pub use portal::check_portal_full;

pub use login_request::do_login_with_retry;
pub use login_request::do_logout_with_retry;

pub use quality::check_network_quality_async;

pub use cache::update_portal_url;
