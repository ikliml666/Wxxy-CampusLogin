---
title: 网络核心：适配器发现、缓存、DHCP 与子网
type: module
source_files:
  - tauri-app/src-tauri/src/network/mod.rs
  - tauri-app/src-tauri/src/network/adapter.rs
  - tauri-app/src-tauri/src/network/adapter_cache.rs
  - tauri-app/src-tauri/src/network/client.rs
  - tauri-app/src-tauri/src/network/dhcp.rs
  - tauri-app/src-tauri/src/network/subnet.rs
  - tauri-app/src-tauri/src/network/discovery/mod.rs
  - tauri-app/src-tauri/src/network/discovery/windows.rs
  - tauri-app/src-tauri/src/network/discovery/registry.rs
tags: [network, adapter, dhcp, mac-reset, discovery, windows, registry, tauri]
---

## Overview

本模块是网络层的"地基"：把系统适配器信息（Windows 走 Win32 `GetAdaptersAddresses` + 注册表可见性/禁用状态交叉校验）转换成稳定的 `Adapter` / `AdapterDetail` / `DisabledAdapter` 三视图，再用 5 秒 TTL 缓存 + 4 秒后台刷新把它们暴露给全应用。在此之上提供适配器选择与启用、DHCP 续租、MAC 重置（注册表 `NetworkAddress` + 重启网卡）、子网判定与网关 ICMP 探测，并托管 Portal HTTP 客户端池与进程级 Portal URL。

本模块全部内容的 `文件:行号` 引用均相对于 `tauri-app/src-tauri/src/`。平台分发的关键事实：非 Windows 平台（含安卓）**没有** `discovery::windows` / `discovery::registry` 子模块，`query_adapters_addresses()` 恒返回三个空 `Vec`，因此安卓端所有适配器枚举 API 拿到空列表；安卓端实际只用得到 `client.rs`（HTTP 客户端池）、`dhcp.rs` 的非 Windows 存根与 `subnet.rs` 的 `is_same_subnet_18`。

## Key Components

### 模块聚合与 re-export（`network/mod.rs`，共 50 行）

- `pub mod adapter / adapter_cache / client / dhcp / discovery / dns / dns_setup / quality / subnet / timing` — `network/mod.rs:1-10`，子模块声明。
- `pub use discovery::{Adapter, AdapterDetail, DisabledAdapter, is_blacklisted};` — `network/mod.rs:13-16`，注意**没有** re-export `AdapterStatus`，外部需写 `crate::network::discovery::AdapterStatus`。
- `pub use dhcp::dhcp_renew_wired_only;` — `network/mod.rs:17`，跨平台（非 Windows 也编译）。
- `#[cfg(target_os = "windows")] pub use dhcp::{dhcp_release_renew_all, dhcp_release_renew_single};` — `network/mod.rs:19-20`。
- `#[cfg(not(target_os = "windows"))] pub use dhcp::dhcp_release_renew_single;` — `network/mod.rs:22-23`，非 Windows 存根版（`skipped` 语义，供 `failure_tracker` 跨平台调用）。
- `pub use subnet::{check_gateway_reachable, check_gateway_reachable_from, is_same_subnet_18, get_wireless_ssid, get_wired_network_profile};` — `network/mod.rs:24-28`。
- `pub use adapter::{resolve_adapter_names, select_adapter, filter_operation_adapters, ensure_ethernet_ip_for_login, find_by_name, find_with_valid_ip, find_dual_adapters, is_secondary_adapter_enabled};` — `network/mod.rs:31-36`。
- `pub use adapter_cache::{get_adapters_cached, get_adapters_cached_async, get_adapters_force, get_disabled_adapters_cached, get_adapter_details_cached, get_all_adapters_cached, wait_for_adapter};` — `network/mod.rs:38-43`。
- `#[cfg(desktop)] pub use adapter_cache::enable_adapter;` — `network/mod.rs:45-46`，`desktop` 是 tauri 2 注入的 cfg 别名（= 非 android/ios）。
- `pub use client::update_portal_url;` — `network/mod.rs:48`。
- `pub use quality::check_network_quality_async;` — `network/mod.rs:50`。

**未被 re-export 的公开 API**（必须用模块全路径访问，容易踩空）：

| 符号 | 定义位置 | 调用方举例 |
|---|---|---|
| `configured_disabled_adapters` | `network/adapter.rs:53` | `monitor/adapter_watch.rs:150` |
| `validate_adapter_name` | `network/adapter_cache.rs:121` | `network/dhcp.rs:13`、`commands/network_cmd.rs:35` |
| `poll_adapter_ip_quick` | `network/adapter_cache.rs:213` | `network/adapter.rs:194` |
| `start_cache_refresh_task` | `network/adapter_cache.rs:249` | `app/startup.rs:189` |
| `create_safe_http_client` / `PORTAL_URL` | `network/client.rs:91` / `network/client.rs:8` | `auth/portal.rs:1`、`auth/protocol.rs:1` |
| `clear_client_pool` | `network/client.rs:137` | `android/src-tauri/src/protocol_cmds.rs:235` |
| `dhcp_renew` / `dhcp_release` / `netsh_disable` / `netsh_enable` | `network/dhcp.rs:35` / `52` / `193` / `204` | 模块内与 `helper/mod.rs` |
| `setup_dns_doh_admin` | `network/dns_setup.rs:13` | 见 [[desktop-network-dns]] |

### 适配器发现：跨平台类型与分发（`network/discovery/mod.rs`，共 181 行）

- `pub(crate) type AdapterQueryResult = Result<(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>), String>;` — `network/discovery/mod.rs:19`，三元组查询结果，全模块统一的返回形态。
- `pub(crate) static ref BL_REGEX: Regex` — `network/discovery/mod.rs:28`，**唯一的适配器名称/描述黑名单正则**（不区分大小写）。完整模式串：

```text
(?i)hyper-v|\bvirtual\b|vmware|veth|docker|wsl|loopback|wintun|tunnel|isatap|6to4|teredo|bluetooth|vpn|hamachi|zerotier|tailscale|wireguard|vEthernet|HNS|\bnat\b|filter.?driver|packet.?driver|npcap|qos|packet.?scheduler|wfp|lightweight.?filter|kernel.?debug|clash|v2ray|xray|sing-box|shadowsocks|ss-local|hysteria|trojan|naiveproxy|mihomo|surge|quantumult|loon|stash|surfboard|netch|proxifier|privoxy|\btor\b|i2p|tun2socks|tap-|tun0|wg0|utun|\btun\b|clash\.tun|clash\.tap|meta\.tun|sing\.tun|cloudflare.?warp|warp|本地连接|虚拟|伪|假|测试|模拟|隧道
```

  设计要点（`network/discovery/mod.rs:22-27` 注释）：`nat` / `tor` / `virtual` 加 `\b` 词边界避免误伤 `Native` / `Intel NAT Offload` / `Toronto` / `Tornado`；中文补全覆盖 `虚拟/伪/假/测试/模拟/隧道`；**刻意保留 `本地连接`**（用户特定业务规则，强制排除）。WLAN/以太网的可见性判断**不在**这里，已移到注册表检查 `is_visible_in_ncpa`，避免按名称误伤"两块都叫 WLAN 的物理网卡"。
- `pub enum AdapterStatus { Disabled, Disconnected, EnabledNoIp, Connected }` — `network/discovery/mod.rs:36-43`，`#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]` + `#[serde(rename_all = "camelCase")]`。四分类语义见 `network/discovery/mod.rs:31-35` 注释。
- `impl AdapterStatus::as_str(self) -> &'static str` — `network/discovery/mod.rs:46-54`，返回中文："已禁用" / "未连接" / "未禁用无IP" / "已连接"。
- `pub struct Adapter` — `network/discovery/mod.rs:58-68`，详见下方字段表。
- `pub struct AdapterDetail` — `network/discovery/mod.rs:71-84`。
- `pub struct DisabledAdapter` — `network/discovery/mod.rs:88-92`。
- `pub fn new_command(program: &str) -> std::process::Command` — `network/discovery/mod.rs:95-101`，**全项目创建子进程的统一入口**；Windows 上追加 `creation_flags(0x08000000)`（`CREATE_NO_WINDOW`，`network/discovery/mod.rs:99`）避免弹控制台窗口；非 Windows 只有 `allow(unused_mut)` 兜底。
- `pub fn is_blacklisted(name: &str) -> bool` — `network/discovery/mod.rs:103-105`，`BL_REGEX.is_match`。
- `#[cfg(target_os = "windows")] pub(crate) fn prefix_len_to_mask(len: u32) -> String` — `network/discovery/mod.rs:109-119`，前缀长度 → 点分十进制掩码；`len > 32` 返回空串，`len == 0` 时掩码为 `0.0.0.0`。
- `pub(crate) fn query_adapters_addresses() -> AdapterQueryResult` — `network/discovery/mod.rs:122-131`，平台分发：Windows 转 `self::windows::query_adapters_addresses()`（`network/discovery/mod.rs:125`）；**非 Windows 直接 `Ok((vec![], vec![], vec![]))`**（`network/discovery/mod.rs:128-130`）。
- 平台子模块门控：`#[cfg(target_os = "windows")] pub mod windows;`（`network/discovery/mod.rs:13-14`）与 `#[cfg(target_os = "windows")] pub mod registry;`（`network/discovery/mod.rs:16-17`）。
- 单元测试 `mod tests` — `network/discovery/mod.rs:133-181`，4 个用例：`blacklist_word_boundary_does_not_match_legit_nics`（`140-150`）、`blacklist_word_boundary_still_matches_known_virtuals`（`152-161`）、`blacklist_chinese_virtual_keywords`（`163-173`，含 `本地连接` 命中断言）、`blacklist_does_not_match_legit_chinese_nics`（`175-180`）。

### 适配器发现：Windows 实现（`network/discovery/windows.rs`，共 308 行）

- `pub fn query_adapters_addresses() -> AdapterQueryResult` — `network/discovery/windows.rs:12-63`。核心常量与流程：
  - `GAA_FLAGS = 0x0080 | 0x0100` — `network/discovery/windows.rs:16`（`GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST`），注意**未**带 `GAA_FLAG_INCLUDE_PREFIX`，前缀长度靠后续 `OnLinkPrefixLength` 字段直读（`network/discovery/windows.rs:161`）。
  - `IF_TYPE_ETHERNET_CSMACD = 6` / `IF_TYPE_IEEE80211 = 71` — `network/discovery/windows.rs:17-18`，只保留这两种 `IfType`，其余（PPP/隧道/虚拟）在 `network/discovery/windows.rs:103-106` 直接跳过。
  - 首次调用只取尺寸（`network/discovery/windows.rs:21-23`），`size == 0` 提前返回空三元组（`network/discovery/windows.rs:25-27`）。
  - `max_retries = 3` — `network/discovery/windows.rs:29`，第 0 次用 `size`，之后每次 `size + 4096`（`network/discovery/windows.rs:31`）。
  - 重试判据：返回 `111`（`ERROR_BUFFER_OVERFLOW`）或 `actual_size > buffer_size` → 记下新尺寸重试；三次仍失败返回 `Err("GetAdaptersAddresses buffer too small after 3 retries")`（`network/discovery/windows.rs:44-50`）。
- `fn parse_adapter_addresses(ptr, if_type_ethernet, if_type_wireless) -> AdapterQueryResult` — `network/discovery/windows.rs:65-281`，遍历单向链表逐项转换，详见 Data Flow。
- `unsafe fn read_pwstr(ptr: windows::core::PWSTR) -> String` — `network/discovery/windows.rs:283-304`，`PCWSTR::to_string()` 优先；失败时手工扫描 `max_len = 4096` 个 UTF-16 单元并用 `from_utf16_lossy`，超长时记 warning 并返回空串（`network/discovery/windows.rs:294-297`）。
- `unsafe fn ipv4_from_in_addr(addr: IN_ADDR) -> String` — `network/discovery/windows.rs:306-308`，`Ipv4Addr::from(addr).to_string()`。

### 适配器发现：注册表可见性 / 禁用状态（`network/discovery/registry.rs`，共 216 行）

- `pub fn is_visible_in_ncpa(guid: &str) -> bool` — `network/discovery/registry.rs:54-77`。两级注册表校验**都通过**才可见：
  1. `HKLM\SYSTEM\CurrentControlSet\Control\Network\{4D36E972-E325-11CE-BFC1-08002BE10318}\{GUID}\Connection` 的 `ShowInNetworkConnections != 0`（走缓存版 `show_in_ncpa_cached`，`network/discovery/registry.rs:72`）；
  2. PnP 设备树交叉验证 `class_subkey_has_matching_guid`（`network/discovery/registry.rs:76`），剔除 Wi-Fi Direct Virtual Adapter 造出的 `WLAN 2/3/4/5` 幽灵副本。
  空 GUID 直接返回 `true`（`network/discovery/registry.rs:55-59`，历史缺陷修复：原实现返回 `false` 导致 `AdapterName` 为空的物理网卡被静默隐藏）。
- `const SHOW_IN_NCPA_CACHE_TTL_SECS: u64 = 5` — `network/discovery/registry.rs:86`，与适配器缓存 TTL 对齐。
- `pub fn invalidate_show_in_ncpa_cache()` — `network/discovery/registry.rs:135-137`，可见性变更路径（`enable_adapter`）必须调用。
- `pub fn refresh_class_subkey_cache()` — `network/discovery/registry.rs:174-177`，重建 Class 子键映射（`enable_adapter` 与 15s 监听循环调用）。
- `pub fn class_subkey_has_matching_guid(guid: &str) -> bool` — `network/discovery/registry.rs:192-202`，空 GUID 返回 false；查 `CLASS_SUBKEY_CACHE` 的 `exists`。
- `pub fn is_admin_disabled_via_registry(guid: &str) -> bool` — `network/discovery/registry.rs:205-216`，判定 `ConfigFlags & 0x1 != 0`（`CONFIGFLAG_DISABLED`，`network/discovery/registry.rs:214`）。
- 私有辅助：`query_show_in_ncpa`（`network/discovery/registry.rs:89-102`，`ShowInNetworkConnections` 缺失或读取失败都视为可见）、`show_in_ncpa_cached`（`105-131`）、`build_class_subkey_cache`（`151-171`，遍历 Class 下全部子键取 `NetCfgInstanceId`（小写）+ `ConfigFlags`）、`ensure_cache_initialized`（`181-190`，双重检查锁定，build 在锁外）。
- 单元测试 `mod tests` — `network/discovery/registry.rs:15-50`，4 个用例：空 GUID 返回 false（`18-21`）、不存在的 GUID 返回 false（`23-26`）、真实 WLAN GUID 为 true（`28-35`，环境不满足时仅打印 `[SKIP]` 不失败）、幽灵 GUID 数组返回 false（`37-49`）。

### 适配器选择与登录前辅助（`network/adapter.rs`，共 382 行）

- `pub fn find_by_name<'a>(adapters: &'a [Adapter], name: &str) -> Option<&'a Adapter>` — `network/adapter.rs:16-18`。
- `pub fn find_with_valid_ip<'a>(adapters, name) -> Option<&'a Adapter>` — `network/adapter.rs:21-23`，名称匹配 **且** `!ip.is_empty()`。
- `pub fn find_dual_adapters<'a>(adapters, config, adapter1_name, adapter2_name) -> (Option<&Adapter>, Option<&Adapter>)` — `network/adapter.rs:26-39`，adapter2 仅在 `config.dual_adapter && !adapter2_name.is_empty() && adapter2_name != adapter1_name` 时查找（`network/adapter.rs:33`）。
- `pub fn is_secondary_adapter_enabled(config, adapter2_name) -> bool` — `network/adapter.rs:44-46`，仅 `dual_adapter && !adapter2_name.is_empty()`；**不**判断与 adapter1 是否相同、**不**判断是否为 `AUTO_DETECT_ADAPTER`（注释 `network/adapter.rs:41-43` 明确要求调用方自行附加条件）。
- `pub fn configured_disabled_adapters<'a>(config: &Config, disabled: &'a [DisabledAdapter]) -> Vec<&'a DisabledAdapter>` — `network/adapter.rs:53-64`，"手选且当前处于禁用列表"的适配器；"手选" = 非空串且 `!= AUTO_DETECT_ADAPTER`（`network/adapter.rs:57-60`）。
- `pub fn resolve_adapter_names(adapters: &[Adapter], config: &Config) -> (String, String)` — `network/adapter.rs:66-118`，**所有适配器选择的唯一真源**。自动检测优先级：有线且有 IP → 任意有 IP → 第一个（`network/adapter.rs:68-75`）；配置名不在当前可见列表时记 warning 并降级到自动检测（`network/adapter.rs:82-88`、`101-112`）；`dual_adapter` 关闭时 adapter2 固定为空串（`network/adapter.rs:113-115`）。
- `pub fn filter_operation_adapters(adapters, adapter1_name, adapter2_name) -> Vec<Adapter>` — `network/adapter.rs:123-128`，把操作范围收窄到 resolve 出的主/副适配器（检测/优化/DHCP/MAC 操作只作用于这两张网卡，UI 展示仍用全量列表）。
- `pub fn select_adapter(adapters, config) -> (String, String)` — `network/adapter.rs:130-142`，返回 `(ip, name)`；适配器列表为空直接返回两个空串（`network/adapter.rs:131`）；只从 `resolve_adapter_names` 解析出的主适配器取 IP，**不回退**到配置范围外的适配器（注释 `network/adapter.rs:133-135` 记录了原实现会静默选中任意外部有 IP 适配器的缺陷）。
- `pub fn ensure_ethernet_ip_for_login(app_handle, adapters, config, is_quitting)` — `network/adapter.rs:144-222`，登录前补偿：对"有线、已连接（在列表中）但 IP 为空"的候选网卡执行 `ipconfig /renew <name>`（`network/adapter.rs:182-185`），随后 `poll_adapter_ip_quick(name, 5000, is_quitting)`（`network/adapter.rs:194`）等待，最后 `c.kill()` + `c.wait()` 回收子进程（`network/adapter.rs:199-201`），成功/超时各发一条登录日志（`network/adapter.rs:204-220`）。
- 单元测试 `mod tests` — `network/adapter.rs:224-382`，含 `make_test_config`（`229-276`，注意它必须与 `crate::config::Config` 字段保持同步）、`make_test_adapter`（`278-294`）、`resolve_adapter_names_falls_back_when_config_name_missing`（`296-306`）、`resolve_adapter_names_uses_config_when_present`（`308-318`）、`resolve_adapter_names_auto_detect_prefers_wired_with_ip`（`320-329`）、`configured_disabled_adapters_manual_only`（`339-381`）。

### 适配器缓存与访问 API（`network/adapter_cache.rs`，共 279 行）

- `static ref ADAPTER_CACHE: RwLock<Option<AdapterCacheEntry>>` — `network/adapter_cache.rs:18`，全局唯一缓存。
- `const ADAPTER_CACHE_TTL_SECS: u64 = 5` — `network/adapter_cache.rs:21`。
- `fn query_adapters_cached_inner() -> AdapterQueryResult` — `network/adapter_cache.rs:23-38`，读锁命中即返回三元组克隆；未命中转 `query_adapters_addresses()` 并整体写缓存。
- `pub fn get_all_adapters_cached() -> AdapterQueryResult` — `network/adapter_cache.rs:44-46`，`adapter_watch` 15s 周期用（BE-B-05，避免与 4s 刷新叠加重复全量查询）。
- `pub fn get_adapters_cached() -> Result<Vec<Adapter>, String>` — `network/adapter_cache.rs:50-65`，命中路径只克隆 `adapters`（BE-B-04）。
- `pub async fn get_adapters_cached_async() -> Result<Vec<Adapter>, String>` — `network/adapter_cache.rs:72-86`，快速路径命中直接返回；慢路径 `tokio::task::spawn_blocking(get_adapters_cached)`（`network/adapter_cache.rs:83`）。
- `pub fn get_disabled_adapters_cached()` — `network/adapter_cache.rs:88-100`，只克隆 `disabled`。
- `pub fn get_adapters_force()` — `network/adapter_cache.rs:102-105`，先 `ADAPTER_CACHE.write().take()` 清缓存再读。
- `pub fn get_adapter_details_cached()` — `network/adapter_cache.rs:107-119`，只克隆 `details`。
- `pub fn validate_adapter_name(name: &str) -> Result<(), String>` — `network/adapter_cache.rs:121-127`，非空、长度 ≤ 128、禁止字符集 `& | ; \` $ ( ) < > " ' \n \r \0`（`network/adapter_cache.rs:124`）。**所有把适配器名拼进命令行的路径的前置校验**。
- `#[cfg(desktop)] pub fn enable_adapter(adapter_name: &str, allow_uac_prompt: bool) -> Result<(), String>` — `network/adapter_cache.rs:135-190`。管理员直接跑 `netsh interface set interface <name> enable`（`network/adapter_cache.rs:144-147`）；非管理员走 `platform::elevation::shell_exec_elevated("netsh", &netsh_args, true)` COM 静默提权（`network/adapter_cache.rs:160`），`allow_uac_prompt == false` 时 COM 失败即返回不弹 UAC（`network/adapter_cache.rs:166-168`），为 true 时降级 `run_elevated`（弹 UAC，`network/adapter_cache.rs:171`）。成功后三重失效：清适配器缓存（`179`）、`refresh_class_subkey_cache()`（`181-182`）、`invalidate_show_in_ncpa_cache()`（`185-186`）。
- `pub fn wait_for_adapter(max_wait_ms: u64, is_quitting: &AtomicBool) -> Result<Vec<Adapter>, String>` — `network/adapter_cache.rs:192-211`，指数退避 `1000ms → ×2 → cap 5000ms`；退出标志置位返回 `Ok(vec![])`；超时后返回最后一次 `get_adapters_cached()`。
- `pub fn poll_adapter_ip_quick(adapter_name: &str, timeout_ms: u64, is_quitting: &AtomicBool) -> bool` — `network/adapter_cache.rs:213-238`，间隔固定 `300ms`（BE-A-04，原 100ms）；先记录初始 IP，只有"IP 非空 **且** 与初始值不同"才算成功（`network/adapter_cache.rs:220-233`）。
- `const CACHE_REFRESH_INTERVAL_SECS: u64 = 4` — `network/adapter_cache.rs:242`。
- `pub fn start_cache_refresh_task(task_manager: &BackgroundTaskManager) -> Result<(), String>` — `network/adapter_cache.rs:249-279`，用 `tokio::time::interval` 每 4 秒 `spawn_blocking(query_adapters_cached_inner)`；刷新失败只记 warning 保留旧缓存；`cancel_token.cancelled()` 时退出。

### HTTP 客户端池与 Portal URL（`network/client.rs`，共 144 行）

- `pub(crate) static ref PORTAL_URL: ArcSwap<String>` — `network/client.rs:8`，进程级 Portal 地址，初值 `crate::config::model::default_portal_url()`。
- `static ref CLIENT_POOL: DashMap<ClientPoolKey, (reqwest::Client, Instant)>` — `network/client.rs:9`。
- `type ClientPoolKey = (Option<IpAddr>, u8, u64)` — `network/client.rs:14`，`(本机绑定地址, TLS 版本标识, 超时毫秒)`，热路径零堆分配。
- `fn tls_version_id(v: reqwest::tls::Version) -> u8` — `network/client.rs:18-26`，TLS_1_0..TLS_1_3 → 0..3，`non_exhaustive` 通配 → 4。
- `const CLIENT_POOL_TTL_SECS: u64 = 600` — `network/client.rs:29`（与 `dns.rs` 的 `DNS_CACHE_TTL_SECS` 对齐）。
- `const CLIENT_POOL_MAX_ENTRIES: usize = 32` — `network/client.rs:31`。
- `pub fn update_portal_url(url: &str)` — `network/client.rs:33-37`，空串忽略（不清空）。
- `fn build_client(timeout, local_addr, min_tls) -> Result<reqwest::Client, String>` — `network/client.rs:43-70`。固定默认头 `Cache-Control: no-store` + `Pragma: no-cache`（`network/client.rs:44-52`）；`min_tls_version`、`timeout`、`connect_timeout = 3s`、`no_proxy()`、重定向上限 5、`pool_max_idle_per_host = 4`、`pool_idle_timeout = 90s`、`tcp_keepalive = 30s`（`network/client.rs:54-63`）；有 bind 地址时 `local_address(ip)`（`network/client.rs:65-67`）。
- `pub fn create_safe_http_client(timeout, local_addr) -> Result<reqwest::Client, String>` — `network/client.rs:91-127`。命中顺序：TLS1.3 键（`92-95`）→ TLS1.2 键（`97-100`）→ 新建 TLS1.3（`102-105`），新建失败才建 TLS1.2（`107-112`）；插入后按 `Instant` 做 LRU 容量清理至 ≤ 32（`117-125`）。
- `#[allow(dead_code)] pub fn clear_client_pool() -> usize` — `network/client.rs:137-144`，清空并返回条数。**只有安卓端调用**（`android/src-tauri/src/protocol_cmds.rs:235`），因为安卓 `bindProcessToNetwork` 的 fwmark 只在 socket 创建时由 netd eBPF 打标，池中 keep-alive 连接仍走绑定前路由（注释 `network/client.rs:130-136`）。

### DHCP 与 MAC 重置（`network/dhcp.rs`，共 497 行）

- `fn ipconfig_output_failed(stdout: &[u8], stderr: &[u8]) -> bool` — `network/dhcp.rs:24-33`，`ipconfig` 失败时退出码常仍为 0，故按关键字兜底：`["error", "cannot", "failed", "失败", "错误", "无法"]`（`network/dhcp.rs:30`）出现在合并输出中即判失败。
- `pub fn dhcp_renew(adapter_name: &str) -> Result<bool, String>` — `network/dhcp.rs:35-50`，`ipconfig /renew <name>`；非零退出码返回 `Ok(false)`；输出异常时 warning 并返回 `Ok(false)`。
- `pub fn dhcp_release(adapter_name: &str) -> Result<bool, String>` — `network/dhcp.rs:52-67`，`ipconfig /release`。
- `pub fn dhcp_renew_wired_only(targets: &[String]) -> Result<Vec<serde_json::Value>, String>` — `network/dhcp.rs:71-93`，只对 `!wireless` 且在 `targets` 名单内的适配器续租，结果元素形如 `{"name": .., "success": ..}`。
- `#[cfg(target_os = "windows")] static MAC_SEED_COUNTER: AtomicU64` — `network/dhcp.rs:96`，仅在 `getrandom` 失败降级时使用。
- `#[cfg(target_os = "windows")] fn generate_random_mac() -> String` — `network/dhcp.rs:99-122`。优先 `getrandom::fill(&mut bytes)`（`network/dhcp.rs:104`）；失败降级为"时间 ns + 计数器"种子跑 LCG（`network/dhcp.rs:106-115`）；**强制单播 + 本地管理位**：`bytes[0] = (bytes[0] & 0xFC) | 0x02`（`network/dhcp.rs:117`）；输出 12 位大写无分隔十六进制。
- `#[cfg(target_os = "windows")] fn mac_with_dashes(mac: &str) -> String` — `network/dhcp.rs:125-131`，每 2 字节插 `-`。
- `#[cfg(target_os = "windows")] fn is_access_denied(e: &std::io::Error) -> bool` — `network/dhcp.rs:134-136`，`raw_os_error() == Some(5)`。
- `#[cfg(target_os = "windows")] pub fn set_mac_via_registry(adapter_guid, mac_no_dash) -> Result<(), String>` — `network/dhcp.rs:139-163`，在 `HKLM\SYSTEM\CurrentControlSet\Control\Class\{4D36E972-E325-11CE-BFC1-08002BE10318}` 下按 `NetCfgInstanceId` 匹配 GUID（`eq_ignore_ascii_case`）后写 `NetworkAddress`；`Access Denied(5)` 转成用户可读提示"修改MAC地址需要管理员权限，请以管理员身份运行应用"（`network/dhcp.rs:145-149`）。
- `#[cfg(target_os = "windows")] pub fn remove_mac_from_registry(adapter_guid) -> Result<(), String>` — `network/dhcp.rs:166-191`，同路径删 `NetworkAddress`；删除失败仅 warning 并继续返回 `Ok(())`（`network/dhcp.rs:182-185`）。
- `pub fn netsh_disable(adapter_name) -> bool` — `network/dhcp.rs:193-202`，`netsh interface set interface name=<name> admin=disable`；名字非法直接 false。
- `pub fn netsh_enable(adapter_name) -> bool` — `network/dhcp.rs:204-213`，`admin=enable`。
- `pub fn poll_ip_change(adapter_name, old_ip, timeout_ms) -> Option<String>` — `network/dhcp.rs:215-230`，300ms 间隔、`get_adapters_force()` 强刷，返回新 IP。
- `pub fn poll_adapter_has_ip(adapter_name, timeout_ms) -> bool` — `network/dhcp.rs:232-247`，300ms 间隔，仅要求 IP 非空。
- `#[cfg(target_os = "windows")] pub fn apply_mac_change_via_registry(adapter_guid, adapter_name, mac_no_dash) -> Result<(), String>` — `network/dhcp.rs:252-273`，等价 `Set-NetAdapter -MacAddress` 底层行为：写注册表 → `dhcp_release` → `netsh_disable` → sleep **500ms** → `netsh_enable`（**失败即整体失败并返回错误**，`network/dhcp.rs:266-270`，因为网卡停在停用态会静默断网）→ `dhcp_renew`。调用方是提权 helper（`helper/mod.rs:163`）。
- `#[cfg(target_os = "windows")] fn try_modify_mac(adapter, fake_mac, _mac_dashed) -> (bool, bool, Option<String>)` — `network/dhcp.rs:277-322`。返回 `(reg_ok, elevated_done, message)`。管理员：直接 `set_mac_via_registry`（`278-285`）；非管理员：`platform::helper_spawn::spawn_elevated_helper("mac", [guid, mac], result_path, 25s)`（`290-295`），helper 成功后再 `poll_ip_change(&adapter.name, &adapter.ip, 25_000)`（`network/dhcp.rs:308`），25 秒内 IP 未变则给出"可能网卡驱动不支持MAC伪装"（`311-314`）。
- `#[cfg(target_os = "windows")] fn renew_adapter_with_mac(adapter, campus_gateway) -> serde_json::Value` — `network/dhcp.rs:326-435`。两道前置过滤：虚拟网卡黑名单命中 → `skipped: true, reason: "虚拟适配器，跳过 MAC 重置"`（`328-337`）；有 IP 且与 `campus_gateway` 不在同一 /18 → `skipped: true, reason: "非校园网子网，跳过"`（`338-347`）。三条分支：注册表写失败 → 只做 release/renew（`358-367`）；helper 提权路径已完成 → 只刷新 IP 状态（`368-381`，注册表清理由 helper 在提权上下文内完成，本进程不再尝试以免 Access Denied 误导告警，注释 `377-378`）；管理员直写成功 → release → disable → sleep 500ms → enable → `poll_adapter_has_ip(3000)` → renew → `poll_ip_change(5000)` → `remove_mac_from_registry`（`382-424`）。返回 JSON 字段全部见下方"返回结构"。
- `#[cfg(target_os = "windows")] pub fn dhcp_release_renew_all(campus_gateway, targets) -> Result<Vec<Value>, String>` — `network/dhcp.rs:441-456`，`campus_gateway` 为空直接 `Err("校园网网关为空，无法判断子网")`（`442-444`）；`targets` 为空返回空 Vec（`449`）。
- `#[cfg(target_os = "windows")] pub fn dhcp_release_renew_single(adapter_name, campus_gateway) -> Result<Value, String>` — `network/dhcp.rs:459-464`，按名找到适配器后走同一内部函数。
- `#[cfg(not(target_os = "windows"))] pub fn dhcp_release_renew_single(_adapter_name, _campus_gateway)` — `network/dhcp.rs:467-475`，恒返回 `{name, success: false, skipped: true, reason: "非桌面平台不支持MAC重置"}`，保证 `failure_tracker` 跨平台可编译可调用。
- 单元测试 `mod tests` — `network/dhcp.rs:477-497`：`blacklist_filters_known_virtual_adapters`（`481-488`）、`blacklist_preserves_physical_adapters`（`490-496`）。

### 子网、SSID 与网关可达性（`network/subnet.rs`，共 222 行）

- `const NETSH_QUERY_CACHE_TTL_SECS: u64 = 60` — `network/subnet.rs:12`，SSID / 有线 Profile 的 netsh 查询 TTL。
- `static ref SSID_CACHE` / `static ref WIRED_PROFILE_CACHE: NetshCache` — `network/subnet.rs:18-19`，**只缓存 `Ok` 结果**，`Err` 不缓存以便下次重试（`network/subnet.rs:14`、`41-44`、`83-86`）。
- `pub fn get_wireless_ssid() -> Result<Option<String>, String>` — `network/subnet.rs:37-46`，带缓存入口。
- `fn get_wireless_ssid_uncached()` — `network/subnet.rs:48-77`，`netsh wlan show interfaces`，逐行找 `starts_with("SSID")` 且非 `BSSID` 的行取冒号后内容（`network/subnet.rs:61-72`），排除包含 `不在` / `not connected` / `disconnected` 的值。
- `pub fn get_wired_network_profile() -> Result<Option<String>, String>` — `network/subnet.rs:79-88`，带缓存入口。
- `fn get_wired_network_profile_uncached()` — `network/subnet.rs:90-117`，`netsh lan show interfaces`，行内匹配 `profile` / `配置文件` / `設定檔`（`network/subnet.rs:103-105`）。
- `pub fn check_gateway_reachable(gateway: &str) -> bool` — `network/subnet.rs:119-121`，`check_gateway_reachable_from(gateway, None)` 的薄封装。
- `async fn gateway_reachable_async(gateway, source_ip) -> bool` — `network/subnet.rs:126-161`。`surge_ping` 异步实现（替代 spawn `ping` 子进程）；按目标地址族选 `ICMP::V4/V6`（`131-134`）；`source_ip` 非空且可解析时 `config.bind(SocketAddr::new(src_ip, 0))`（`135-141`，对应原 `ping -S <src>`）；`PingIdentifier` 取系统时间 `subsec_nanos & 0xFFFF`（`146-149`）；`pinger.timeout(2000ms)`（`151`）外加 `tokio::time::timeout(2000ms)` 硬上限兜底（`153-160`）。语义等同原 `ping -n 1 -w 2000`（`network/subnet.rs:125`）。
- `pub fn check_gateway_reachable_from(gateway: &str, source_ip: Option<&str>) -> bool` — `network/subnet.rs:163-171`，空网关直接 false；经 `crate::infra::async_util::block_on_sync` 驱动 async 探测。注释 `network/subnet.rs:167-169` 明确：调用链（`campus_check` / `failure_tracker` / `background_check`）都运行在 `spawn_blocking` 线程内，在 async worker 线程上直接调用会 panic。
- `pub fn is_same_subnet_18(ip_str: &str, gateway_str: &str) -> bool` — `network/subnet.rs:174-185`，掩码 `0xFFFF_C000`（`255.255.192.0`，`network/subnet.rs:183`）；任一 IP 解析失败返回 false。安卓端 `campus_detect.rs:73` 也调用此函数。
- 单元测试 `mod tests` — `network/subnet.rs:187-222`：`is_same_subnet_18_same_subnet_returns_true`（`192-197`）、`..._different_subnet_returns_false`（`199-207`）、`..._invalid_ip_returns_false`（`209-214`）、`..._invalid_gateway_returns_false`（`216-221`）。

## 结构体与字段

### `Adapter`（`network/discovery/mod.rs:58-68`）

| 字段 | 类型 | 含义 |
|---|---|---|
| `name` | `String` | 适配器友好名（`FriendlyName`，如"以太网"/"WLAN"），全项目用它做身份标识 |
| `ip` | `String` | 首个 IPv4 单播地址；`is_up == false` 或 169.254 APIPA 时为空串 |
| `wireless` | `bool` | `IfType == 71`（IEEE 802.11） |
| `guid` | `String` | `AdapterName`，强制带花括号（`{...}`） |
| `mac` | `String` | 物理地址，大写冒号分隔；`PhysicalAddressLength < 6` 时为空串 |
| `if_index` | `u32` | 接口索引（`Anonymous1.Anonymous.IfIndex`） |
| `status` | `AdapterStatus` | 四分类状态 |
| `link_speed` | `u64` | 连接速率 bit/s；`u64::MAX` 哨兵归 0 表示未知 |

### `AdapterDetail`（`network/discovery/mod.rs:71-84`）

| 字段 | 类型 | 含义 |
|---|---|---|
| `name` | `String` | 同 `Adapter.name` |
| `ip` | `String` | 同 `Adapter.ip`（`EnabledNoIp` 分支强制为空串） |
| `wireless` | `bool` | 同 `Adapter.wireless` |
| `subnet_mask` | `String` | `OnLinkPrefixLength` 经 `prefix_len_to_mask` 转换；`EnabledNoIp` 分支为空串 |
| `gateway` | `String` | `FirstGatewayAddress` 中首个 IPv4 网关 |
| `dhcp_server` | `String` | `Dhcpv4Server` 的 IPv4 地址 |
| `mac` | `String` | 同 `Adapter.mac` |
| `if_index` | `u32` | 同 `Adapter.if_index` |
| `status` | `AdapterStatus` | 同 `Adapter.status` |
| `link_speed` | `u64` | 同 `Adapter.link_speed` |

### `DisabledAdapter`（`network/discovery/mod.rs:88-92`）

| 字段 | 类型 | 含义 |
|---|---|---|
| `name` | `String` | 适配器友好名 |
| `status` | `String` | 取 `AdapterStatus::as_str()` 的中文串（当前仅"已禁用"会进此列表） |
| `description` | `String` | `Description` 字段，用于 UI 消歧 |

### `AdapterStatus`（`network/discovery/mod.rs:36-43`）

| 变体 | 判定条件 | 序列化值 |
|---|---|---|
| `Disabled` | 非 `Up` 且 `OperStatus == IfOperStatusNotPresent` 且 `ConfigFlags & 0x1 != 0` | `disabled` |
| `Disconnected` | 非 `Up` 且（`NotPresent` 但非管理员禁用，或 Down/LowerLayerDown/Dormant/Unknown/Testing） | `disconnected` |
| `EnabledNoIp` | `OperStatus == IfOperStatusUp` 且 IP 为空 | `enabledNoIp` |
| `Connected` | `OperStatus == IfOperStatusUp` 且有 IP | `connected` |

### `ClassSubkeyEntry`（`network/discovery/registry.rs:140-143`）

| 字段 | 类型 | 含义 |
|---|---|---|
| `exists` | `bool` | 该 GUID 是否在 Class 子键中出现（构建时恒为 `true`，不存在即不在 map 中） |
| `config_flags` | `Option<u32>` | 注册表 `ConfigFlags` 原值；`None` = 值缺失 |

### `AdapterCacheEntry`（`network/adapter_cache.rs:15`）

类型别名 `(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>, Instant)`，即 `(全量适配器, 详情, 禁用列表, 写入时刻)`，整体装在 `RwLock<Option<..>>` 中。

### `ClientPoolKey`（`network/client.rs:14`）

`(Option<IpAddr>, u8, u64)` = `(绑定源地址, TLS 版本标识, 超时毫秒)`；`u8` 由 `tls_version_id` 映射（0=TLS1.0 … 3=TLS1.3，4=其他）。

### `renew_adapter_with_mac` 返回结构（`network/dhcp.rs:426-434`、`328-347`）

| 键 | 类型 | 含义 |
|---|---|---|
| `name` | string | 适配器名 |
| `wireless` | bool | 是否无线 |
| `ip` | string | 操作后观测到的 IP |
| `regOk` | bool | 注册表 `NetworkAddress` 是否写入成功 |
| `success` | bool | **等于 `ip_changed`**，即 IP 是否真的变了（不是"操作是否执行"） |
| `skipped` | bool | 虚拟网卡 / 非校园网子网 / 非桌面平台被跳过 |
| `reason` | string / null | 失败或跳过原因 |

## Data Flow

### 适配器发现 → 缓存 → 选择 → 操作

```text
系统调用（Win32 GetAdaptersAddresses + 注册表）
  → discovery::query_adapters_addresses()                 [network/discovery/mod.rs:122]
    → windows::query_adapters_addresses()                 [network/discovery/windows.rs:12]
      → parse_adapter_addresses()                         [network/discovery/windows.rs:65]
          ├─ IfType 只留 6/71                              [:103]
          ├─ PhysicalAddressLength == 0 跳过               [:108]
          ├─ is_visible_in_ncpa(guid) 过滤                 [registry.rs:54]
          │    ├─ show_in_ncpa_cached (TTL 5s)             [registry.rs:105]
          │    └─ class_subkey_has_matching_guid           [registry.rs:192]
          ├─ is_blacklisted(name) || is_blacklisted(desc)  [:123]
          ├─ OperStatus + IP → AdapterStatus 四分类        [:203-222]
          └─ 分发：adapters(全部) / details(Connected|EnabledNoIp) / disabled(Disabled)
  → adapter_cache::ADAPTER_CACHE (RwLock, TTL 5s)         [network/adapter_cache.rs:18-21]
    ├─ 4s 后台刷新：start_cache_refresh_task              [network/adapter_cache.rs:249]
    └─ 15s 监听：get_all_adapters_cached + registry 刷新   [monitor/adapter_watch.rs:43-51]
       （周期常量 ADAPTER_WATCH_INTERVAL = 15000，[monitor/adapter_watch.rs:8]）
  → 选择：resolve_adapter_names(adapters, config)          [network/adapter.rs:66]
    → select_adapter / filter_operation_adapters / find_dual_adapters
  → 操作（范围限定在 resolve 出的主/副适配器）
    ├─ 登录前补偿：ensure_ethernet_ip_for_login → ipconfig /renew + poll_adapter_ip_quick
    ├─ DHCP 续租：dhcp_renew_wired_only → dhcp_renew
    ├─ MAC 重置：dhcp_release_renew_all/single → renew_adapter_with_mac
    │     ├─ 管理员：set_mac_via_registry → release → disable → 500ms → enable → renew
    │     └─ 非管理员：spawn_elevated_helper("mac") → helper 内 apply_mac_change_via_registry
    └─ 启用网卡：enable_adapter → netsh(+COM/UAC 提权) → 三重缓存失效
```

### 子网 / SSID / 网关

```text
campus_check（后台巡检）
  ├─ get_wireless_ssid()      → netsh wlan show interfaces → SSID_CACHE(TTL 60s)
  ├─ get_wired_network_profile() → netsh lan show interfaces → WIRED_PROFILE_CACHE(TTL 60s)
  ├─ is_same_subnet_18(ip, gateway) → 掩码 255.255.192.0 前缀比对
  └─ check_gateway_reachable(gw) → surge_ping（ICMP，2s 超时，可 bind 源 IP）
```

### Portal HTTP 客户端

```text
auth::protocol / auth::portal
  → client::create_safe_http_client(timeout, local_addr)   [network/client.rs:91]
      → CLIENT_POOL 查 (local_addr, TLS1.3, timeout_ms)     [:92]
      → 未命中查 (local_addr, TLS1.2, timeout_ms)           [:97]
      → 未命中 build_client(TLS1.3)，失败再 build_client(TLS1.2)
      → 插入池 + LRU 淘汰至 ≤32 条                          [:115-125]
  → PORTAL_URL.load() 取当前 Portal 地址                    [:8]
  （安卓端：路由切换后 protocol_cmds.rs:235 调 clear_client_pool() 丢弃旧路由连接）
```

## Connections

- [[desktop-auth]] — `auth::service` 用 `wait_for_adapter` / `select_adapter` / `ensure_ethernet_ip_for_login` / `find_dual_adapters` 组织登录流程；`auth::protocol` 与 `auth::portal` 从 `network/client` 取 `PORTAL_URL` 与 `create_safe_http_client`。
- [[desktop-monitor]] — `monitor/adapter_watch.rs` 每 15s 读 `get_all_adapters_cached` 并刷新注册表缓存、按 `configured_disabled_adapters` 自动启用网卡；`monitor/campus_check.rs` 用 SSID/Profile/子网/网关探测；`monitor/background_check.rs` 与 `monitor/auto_auth.rs` 用 `filter_operation_adapters`。
- [[desktop-commands]] — `commands/network_cmd.rs` 是 DHCP 续租、MAC 重置、网卡启用、适配器列表/详情查询的 Tauri 命令入口；`commands/system.rs` 汇总适配器状态。
- [[desktop-platform]] — `platform::elevation`（`is_admin` / `shell_exec_elevated` / `run_elevated` / `parse_guid`）、`platform::helper_spawn`（`--helper` 提权重启自身）、`platform::console_output::decode_console_bytes`、`platform::dns_config` 均为本模块依赖。
- [[desktop-network-dns]] — DNS/DoH 解析、分段计时与一键设置 DNS 的模块，`timing.rs` 反过来复用本模块的 `is_same_subnet_18` 之外的探测能力。
- [[desktop-network-quality]] — 质量检测把本模块的 `Adapter`/`AdapterDetail` 作为输入，并用 `adapter_ip` 作为 TCP/TLS 源绑定地址。

## Known Issues

1. **`AdapterStatus` 未在 `network/mod.rs` re-export** — 定义在 `network/discovery/mod.rs:38`，但 `network/mod.rs:13-16` 只导出了 `Adapter`/`AdapterDetail`/`DisabledAdapter`/`is_blacklisted`。外部模块要用它只能写 `crate::network::discovery::AdapterStatus`（例如 `network/adapter.rs:227` 的测试），属于模块面不一致，不是功能缺陷。
2. **非 Windows 平台适配器枚举恒为空** — `network/discovery/mod.rs:127-130` 直接返回 `Ok((vec![], vec![], vec![]))`；`discovery::windows` / `discovery::registry` 整模块 `#[cfg(target_os = "windows")]`（`network/discovery/mod.rs:13-17`）。安卓端任何依赖适配器列表的路径都拿不到数据，安卓侧只用 `network::client`、`is_same_subnet_18`、`quality`、非 Windows 的 `dhcp_release_renew_single` 存根。
3. **`CLASS_SUBKEY_CACHE` 不会自动过期** — `network/discovery/registry.rs:181-190`（`ensure_cache_initialized`）只在首次访问构建；生命周期内仅由 `refresh_class_subkey_cache`（`network/discovery/registry.rs:174-177`）重建，调用点只有 `network/adapter_cache.rs:181-182`（启用网卡）与 `monitor/adapter_watch.rs:43-45`（15s 周期，已包 `spawn_blocking`）。历史缺陷见 `monitor/adapter_watch.rs:37-42` 注释：曾因只在 `enable_adapter` 刷新，运行期在设备管理器禁用/拔插网卡后可见性与禁用分类永久陈旧。
4. **`ShowInNetworkConnections` 缓存过期后的重建是"单条"策略** — `network/discovery/registry.rs:124-130`：过期后只把当前 GUID 一条写进新 map，其余 GUID 在 TTL 内逐条补入。设备管理器新增适配器时会多打几次注册表，功能正确、性能略退化。
5. **黑名单永久隐藏名为"本地连接"的网卡** — `network/discovery/mod.rs:28` 的模式串含 `本地连接`，且 `network/discovery/mod.rs:171-172` 的测试断言 `is_blacklisted("本地连接")` 为 true。这是有意的业务规则（Win11 高级网络设置不可见），但对真实命名为"本地连接"的物理网卡是硬过滤。
6. **`wait_for_adapter` 与 `poll_adapter_ip_quick` 用阻塞 sleep** — `network/adapter_cache.rs:206`、`network/adapter_cache.rs:228` 均为 `std::thread::sleep`。它们必须在阻塞线程（如 `spawn_blocking` / 同步登录流程）内调用，直接放进 async 任务会占住 worker 线程。`poll_adapter_ip_quick` 的 300ms 间隔意味着 IP 变更检测最多滞后 300ms（`network/adapter_cache.rs:215-217` 注释）。
7. **`get_adapters_force` 无条件清缓存** — `network/adapter_cache.rs:102-105`，调用后紧接的查询必然走慢路径（真实 Win32 调用）。在 `poll_ip_change` / `poll_adapter_has_ip` 的轮询循环里（`network/dhcp.rs:220`、`237`）会以 300ms 频率反复触发全量系统查询。
8. **`create_safe_http_client` 的 TLS 1.2 降级结果会被长期复用** — `network/client.rs:97-100` 先查 TLS1.2 键。一旦因 TLS1.3 构建失败而生成过 TLS1.2 客户端，它在 `CLIENT_POOL_TTL_SECS = 600` 秒内命中即返回（`network/client.rs:29`、`77-80`），期间不会主动尝试升级回 TLS1.3。
9. **`clear_client_pool` 在桌面端是死代码** — `network/client.rs:136` 显式 `#[allow(dead_code)]`；唯一调用方是安卓端 `android/src-tauri/src/protocol_cmds.rs:235`。桌面 bin 不走该路径（`network/client.rs:135` 注释）。
10. **`generate_random_mac` 的降级路径可预测性较弱** — `network/dhcp.rs:104-115`：`getrandom::fill` 失败时退回"时间 ns + 原子计数器"的 LCG 序列（`MAC_SEED_COUNTER` 在 `network/dhcp.rs:96`）。注释明确这是低概率兜底，正常路径走 `BCryptGenRandom`。
11. **`renew_adapter_with_mac` 的 `success` 语义是"IP 是否变化"** — `network/dhcp.rs:431`。MAC 写入成功但 DHCP 服务器分回同一 IP 时返回 `success: false` + `reason`（`network/dhcp.rs:421-423`）；`regOk: true` 与 `success` 需要分开读。
12. **注册表清理在非提权路径被刻意跳过** — `network/dhcp.rs:377-378` 注释说明：helper 路径下 `NetworkAddress` 的清理已由提权 helper 完成（`helper/mod.rs:168` 调 `remove_mac_from_registry`），主进程再清只会 Access Denied 并产生误导性告警。
13. **`PhysicalAddressLength == 0` 的适配器完全不可见** — `network/discovery/windows.rs:108-111` 直接 `continue`，既不进 `adapters` 也不进 `disabled`。某些虚拟/半初始化网卡属于此类。
14. **`subnet.rs` 的 netsh 解析依赖中英文本地化关键字** — `network/subnet.rs:61-72`（SSID）、`network/subnet.rs:103-105`（`profile` / `配置文件` / `設定檔`）。未覆盖的其它语言 Windows 会返回 `Ok(None)`，功能静默降级。
15. **`check_gateway_reachable_from` 对运行时有硬要求** — `network/subnet.rs:163-171` 经 `block_on_sync` → `Handle::block_on`；在 async worker 线程直接调用会 panic（`network/subnet.rs:167-169` 注释给明约束），调用方必须处于 `spawn_blocking` 线程。
