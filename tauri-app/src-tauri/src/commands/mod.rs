pub mod state;
pub mod config_cmd;
pub mod login;
pub mod background;
pub mod network_cmd;
pub mod account;
pub mod system;

pub use state::{AppState, CANCEL_EXIT_SHORTCUT};
pub use config_cmd::load_config_from_disk_or_default;
pub use login::full_login_inner;
pub use background::{run_background_check, start_adapter_watch, run_startup_tasks, cancel_auto_exit_inner, start_auto_exit};
