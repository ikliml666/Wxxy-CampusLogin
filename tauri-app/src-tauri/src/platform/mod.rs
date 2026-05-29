pub mod elevation;
pub mod dns_config;
pub mod autostart;
pub mod gpu;

pub use elevation::{is_admin, run_elevated, shell_exec_elevated};
pub use dns_config::{set_dns_via_api, set_doh_via_api, read_adapter_dns_from_registry, PRIMARY_DNS, SECONDARY_DNS, DOH_SERVERS};
pub use autostart::{set_auto_start, remove_auto_start, get_auto_launch_enabled};
pub use gpu::*;
