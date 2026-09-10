//! 安卓端全局状态:登录/注销复用的 wlan0 源 IP 缓存(检测阶段写入)与全量配置内存态。

use std::net::Ipv4Addr;
use std::sync::Mutex;

use crate::config_state::Settings;

#[derive(Default)]
pub struct AndroidState {
    pub cached_source_ip: Mutex<Option<Ipv4Addr>>,
    /// 明文配置内存态(密码仅存内存,落盘必经 Keystore 加密)
    pub config: Mutex<Option<Settings>>,
}
