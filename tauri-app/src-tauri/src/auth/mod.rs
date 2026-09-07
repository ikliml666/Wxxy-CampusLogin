pub mod failure_tracker;
pub mod portal;
pub mod protocol;

// 依赖桌面适配器发现(GetAdaptersAddresses),安卓 target 不编译
#[cfg(desktop)]
pub mod dual_adapter_executor;
#[cfg(desktop)]
pub mod session;
#[cfg(desktop)]
pub mod service;
