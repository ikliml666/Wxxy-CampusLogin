// 跨平台:控制台输出解码(协议响应/子网查询共用,安卓侧同样需要 GBK 解码)
pub mod console_output;

// 桌面专属(Windows 为主):安卓 target 不编译
#[cfg(desktop)]
pub mod autostart;
#[cfg(desktop)]
pub mod dns_config;
#[cfg(desktop)]
pub mod elevation;
#[cfg(desktop)]
pub mod gpu;
#[cfg(desktop)]
pub mod helper_spawn;
#[cfg(desktop)]
pub mod identity;
