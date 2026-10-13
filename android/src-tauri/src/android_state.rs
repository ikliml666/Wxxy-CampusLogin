//! 安卓端全局状态:登录/注销复用的 wlan0 源 IP 缓存(检测阶段写入)与全量配置内存态。
//! watcher_failed 独立为进程级静态(而非 AndroidState 字段):status_value 可在
//! tauri .manage 之前被调用(单测/启动早期),State 不可得;AtomicBool 免锁。

use std::net::Ipv4Addr;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use crate::config_state::Settings;

/// WiFi watcher 注册失败降级标志(退化为纯周期检测):start 成功置 false,
/// 注册失败置 true,经 get_background_status("watcherFailed")暴露给前端。
/// pub(crate) 供 monitor_loop 直接读写(status_value 是同步 fn,不走 async 访问器)
pub(crate) static WATCHER_FAILED: AtomicBool = AtomicBool::new(false);

#[derive(Default)]
pub struct AndroidState {
    pub cached_source_ip: Mutex<Option<Ipv4Addr>>,
    /// 明文配置内存态(密码仅存内存,落盘必经 Keystore 加密)
    pub config: Mutex<Option<Settings>>,
}
