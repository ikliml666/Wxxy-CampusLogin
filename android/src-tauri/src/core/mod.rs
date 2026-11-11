//! :core 进程的后台核心。
//!
//! 核心(巡检/自动登录/夜间切换/通知/WiFi 事件)只在本模块运行,由 `:core` 进程
//! 的 `CoreService` 经 JNI `nativeStart` 拉起;UI 进程不再启动它,只经 Binder
//! 转发命令、读取共享状态文件、接收事件广播。
//!
//! 平台能力统一走 [`platform::CorePlatform`],登录协议仍只在 `campus-login`
//! 协议核心(campus_login_lib)单点实现,这里只做编排。

pub mod login;
pub mod monitor;
pub mod platform;
pub mod runner;
pub mod status_file;
pub mod ui_bridge;

#[cfg(target_os = "android")]
pub mod jni_platform;
