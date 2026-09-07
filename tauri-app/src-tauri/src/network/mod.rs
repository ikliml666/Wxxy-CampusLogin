pub mod adapter;
pub mod adapter_cache;
pub mod client;
pub mod dhcp;
pub mod discovery;
pub mod dns;
pub mod dns_setup;
pub mod quality;
pub mod subnet;
pub mod timing;

// 从源模块直接 re-export（消除 adapter.rs 中转层，与职责迁移后的结构对齐）
pub use discovery::{
    Adapter, AdapterDetail, DisabledAdapter,
    is_blacklisted,
};
pub use dhcp::dhcp_renew_wired_only;
// MAC 重置链路依赖注册表/提权,仅桌面
#[cfg(target_os = "windows")]
pub use dhcp::{dhcp_release_renew_all, dhcp_release_renew_single};
// 非 Windows 存根版(skipped 语义),供 failure_tracker 跨平台调用
#[cfg(not(target_os = "windows"))]
pub use dhcp::dhcp_release_renew_single;
pub use subnet::{
    check_gateway_reachable, check_gateway_reachable_from,
    is_same_subnet_18,
    get_wireless_ssid, get_wired_network_profile,
};

// adapter.rs 原生符号（适配器选择职责）
pub use adapter::{
    resolve_adapter_names, select_adapter, filter_operation_adapters,
    ensure_ethernet_ip_for_login,
    find_by_name, find_with_valid_ip, find_dual_adapters,
    is_secondary_adapter_enabled,
};

pub use adapter_cache::{
    get_adapters_cached, get_adapters_cached_async, get_adapters_force,
    get_disabled_adapters_cached, get_adapter_details_cached,
    get_all_adapters_cached,
    wait_for_adapter,
};
// 适配器启用走 netsh+提权,仅桌面
#[cfg(desktop)]
pub use adapter_cache::enable_adapter;

pub use client::update_portal_url;

pub use quality::check_network_quality_async;
