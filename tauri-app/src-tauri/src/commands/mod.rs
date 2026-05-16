// [架构说明] commands 模块间耦合关系
//
// 依赖链（箭头表示 "调用/依赖"）：
//
//   background ──→ auto_login ──→ auto_exit
//       │              │              │
//       │              └──→ system (emit_notification)
//       │
//       ├──→ latency ──→ system (emit_notification)
//       │
//       └──→ auto_exit
//
//   login ──→ system (append_login_history)
//
//   auto_exit ──→ system (emit_notification)
//
//   adapter_watch ── (无跨模块调用，仅依赖 state + network)
//
// 耦合问题：
//   1. background 是核心调度器，同时依赖 auto_login/auto_exit/latency 三个子模块，
//      任何子模块的接口变更都会影响 background
//   2. auto_login 同时调用 auto_exit 和 system，形成 background→auto_login→auto_exit
//      的三层调用链，中间层的变更会向上传播
//   3. emit_notification 被 auto_login/auto_exit/latency 三处调用，是事实上的共享工具，
//      但定义在 system 模块中，语义上不够清晰
//
// 所有模块通过 AppState 共享状态（见 state.rs），状态一致性依赖原子操作和 ArcSwap 保证

pub mod state;
pub mod config_cmd;
pub mod login;
pub mod background;
pub mod auto_exit;
pub mod auto_login;
pub mod latency;
pub mod adapter_watch;
pub mod network_cmd;
pub mod account;
pub mod system;
pub mod updater;

pub use state::{AppState, CANCEL_EXIT_SHORTCUT};
pub use config_cmd::load_config_from_disk_or_default;
pub use login::full_login_inner;
pub use background::{run_background_check, run_startup_tasks};
pub use auto_exit::{start_auto_exit, cancel_auto_exit_inner};
pub use adapter_watch::start_adapter_watch;
pub use updater::start_update_check_loop;
