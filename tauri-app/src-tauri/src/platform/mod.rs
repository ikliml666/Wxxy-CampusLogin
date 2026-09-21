// 跨平台:控制台输出解码(协议响应/子网查询共用,安卓侧同样需要 GBK 解码)
pub mod console_output;

// 桌面专属(Windows 为主):安卓 target 不编译
#[cfg(desktop)]
pub mod autostart;
// Windows 效率模式（EcoQoS）：轻量化模式期间启用，非 Windows 为空实现
#[cfg(desktop)]
pub mod ecoqos;
#[cfg(desktop)]
pub mod dns_config;
#[cfg(desktop)]
pub mod elevation;
#[cfg(desktop)]
pub mod gpu;
#[cfg(desktop)]
pub mod helper_spawn;
// 计划任务提权代理（SYSTEM 主体哑任务 + 请求文件协议）：提权通道的首选层
#[cfg(all(desktop, target_os = "windows"))]
pub mod task_proxy;
#[cfg(desktop)]
pub mod identity;
// RTSS(MSI Afterburner) hook 注入致 WebView 白屏崩溃的预防(写排除 profile)
#[cfg(all(desktop, target_os = "windows"))]
pub mod rtss_compat;
#[cfg(all(desktop, target_os = "windows"))]
pub mod toast;
