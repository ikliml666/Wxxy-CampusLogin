---
title: subnet 模块：子网计算、SSID 处理与网关可达性检查
type: module
source_files:
  - tauri-app/src-tauri/src/network/subnet.rs
tags: [网络诊断, Windows, netsh, ICMP, 缓存, 子网掩码]
---

## Overview

`subnet.rs` 模块负责集中处理校园网环境下的网络诊断逻辑。它从 `adapter.rs` 迁移而来，主要职责包括：通过 `netsh` 命令获取无线 SSID 和有线网络 Profile（带 TTL 缓存以避免高频进程创建）、检测网关是否可达（使用异步 ICMP 探测替代阻塞式 `ping` 命令），以及判断 IP 地址与网关是否处于同一 /18 子网内。

该模块特别针对 Windows 平台特性进行了优化，例如处理 `netsh` 在 Win11 位置服务关闭时的行为，以及在 TUN 环境下优先使用官方 `IcmpSendEcho2Ex` API 进行 IPv4 探测，以避免 `surge_ping` 绑源失败的误判问题。

## Key Components

### 常量与全局状态

*   **`NETSH_QUERY_CACHE_TTL_SECS: u64 = 60`** (Line 15): netsh 查询结果的缓存有效期，单位秒。注释中说明后台巡检每 15s 固定调用，加 TTL 缓存避免高频 spawn 子进程。
*   **`NetshCache`** (Line 20): 类型别名，定义为 `parking_lot::Mutex<Option<(Instant, Option<String>)>>`，用于存储 `(写入时间, 查询结果)`。仅缓存 `Ok` 结果，失败不缓存以便重试。
*   **`SSID_CACHE`** (Line 23): 静态全局缓存 (`lazy_static`)，用于存储无线 SSID 查询结果。
*   **`WIRED_PROFILE_CACHE`** (Line 24): 静态全局缓存 (`lazy_static`)，用于存储有线网络 Profile 查询结果。
*   **`SSID_QUERY_WARNED`** (Line 37): `std::sync::atomic::AtomicBool`，用于确保 SSID 查询失败告警在进程生命周期内只发送一次，防止日志刷爆。

### 公共函数

*   **`pub fn get_wireless_ssid() -> Result<Option<String>, String>`** (Line 49):
    *   获取当前 WiFi 连接的 SSID。
    *   首先检查 `SSID_CACHE` 是否命中且未过期；若未命中，调用 `get_wireless_ssid_uncached`。
    *   成功结果会被缓存。
*   **`pub fn get_wired_network_profile() -> Result<Option<String>, String>`** (Line 80):
    *   获取当前有线网络的 Profile 名称。
    *   首先检查 `WIRED_PROFILE_CACHE`；若未命中，调用 `get_wired_network_profile_uncached`。
    *   成功结果会被缓存。
*   **`pub fn check_gateway_reachable(gateway: &str) -> bool`** (Line 113):
    *   便捷方法，调用 `check_gateway_reachable_from`，不指定源 IP。
*   **`pub fn is_same_subnet_18(ip_str: &str, gateway_str: &str) -> bool`** (Line 137):
    *   判断两个 IPv4 地址是否在同一个 /18 子网（掩码 `255.255.192.0`）内。
    *   如果 IP 解析失败，返回 `false`。

### 私有辅助函数

*   **`fn netsh_cache_get(cache: &NetshCache) -> Option<Option<String>>`** (Line 27):
    *   内部缓存读取逻辑。检查缓存是否存在且时间戳距离现在小于 `NETSH_QUERY_CACHE_TTL_SECS`。
*   **`fn netsh_cache_put(cache: &NetshCache, val: Option<String>)`** (Line 35):
    *   内部缓存写入逻辑，记录当前 `Instant`。
*   **`fn warn_ssid_query_failed(reason: &str)`** (Line 41):
    *   一次性告警逻辑。如果 `SSID_QUERY_WARNED` 为 false，则设置其为 true 并记录警告日志，说明 WiFi SSID 匹配判定可能失效。
*   **`fn get_wireless_ssid_uncached() -> Result<Option<String>, String>`** (Line 55):
    *   执行 `netsh wlan show interfaces`。
    *   如果 `netsh` 退出码非 0，记录警告并返回 `Ok(None)`。
    *   解码控制台输出后，检查是否包含 "位置权限" 或 "location permission"（Win11 特有现象），若是则记录警告并返回 `Ok(None)`。
    *   遍历输出行，查找以 "SSID" 开头但不以 "BSSID" 开头的行，提取冒号后的值。过滤掉空值及包含 "不在"/"not connected"/"disconnected" 的行。
*   **`fn get_wired_network_profile_uncached() -> Result<Option<String>, String>`** (Line 88):
    *   执行 `netsh lan show interfaces`。
    *   如果 `netsh` 退出码非 0，返回 `Ok(None)`。
    *   遍历输出行，查找包含 "profile"、"配置文件" 或 "設定檔" 的行，提取冒号后的值。
*   **`async fn gateway_reachable_async(gateway: &str, source_ip: Option<&str>) -> bool`** (Line 118):
    *   异步 ICMP 探测核心逻辑。
    *   解析目标网关 IP，构建 `surge_ping::Config`。
    *   如果提供了源 IP (`source_ip`)，则绑定到该源 IP。
    *   创建 `surge_ping::Client` 和 `Pinger`。
    *   设置超时为 2000ms。使用 `tokio::time::timeout` 包裹 `pinger.ping`，等待响应。
*   **`pub fn check_gateway_reachable_from(gateway: &str, source_ip: Option<&str>) -> bool`** (Line 131):
    *   网关可达性探测的入口。
    *   如果网关为空，返回 `false`。
    *   **Windows 特殊处理**: 如果是 Windows 平台且目标是 IPv4，调用 `crate::platform::icmp_probe::icmp_probe_v4` 使用官方 API 进行探测，避免 TUN 环境下 `surge_ping` 绑源失败导致的误判。
    *   **其他平台/IPv6**: 调用 `crate::infra::async_util::block_on_sync` 运行 `gateway_reachable_async`。注意此函数通常在 `spawn_blocking` 线程中调用，以避免在 async worker 线程上 `block_on` 导致 panic。
*   **`is_same_subnet_18` 实现细节 (Line 137-149)**:
    *   将 IP 和网关字符串解析为 `u32`。
    *   使用掩码 `0xFFFF_C000` (/18) 进行按位与运算，比较结果是否相等。

### 测试用例 (Lines 153-182)

*   **`mod tests`**:
    *   `is_same_subnet_18_same_subnet_returns_true`: 验证同 /18 子网内的 IP 对（如 10.2.0.1 和 10.2.63.254）。
    *   `is_same_subnet_18_different_subnet_returns_false`: 验证不同子网（如 10.2.0.1 和 10.2.64.1）及跨边界情况（10.2.63.255 和 10.2.64.0）。
    *   `is_same_subnet_18_invalid_ip_returns_false`: 验证无效 IP 格式返回 `false`。
    *   `is_same_subnet_18_invalid_gateway_returns_false`: 验证无效网关格式返回 `false`。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│                     subnet.rs Module                                │
│                                                                     │
│  ┌──────────────────┐     ┌──────────────────────────┐             │
│  │ Netsh Queries    │     │ Gateway Reachability     │             │
│  │                  │     │                          │             │
│  │ • get_wireless_  │     │ • check_gateway_reach-   │             │
│ │   ssid()         │     │   able_from()            │             │
│  │ • get_wired_     │     │                          │             │
│ │   network_       │     │   [Windows IPv4]:        │             │
│ │   profile()      │     │   ► icmp_probe_v4()      │             │
│  │                  │     │     (official API)       │             │
│  │ Cache:           │     │   [Others/IPv6]:        │             │
│  │ • SSID_CACHE     │     │   ► block_on_sync()      │             │
│  │ • WIRED_PROFILE_ │     │     ► gateway_reach-     │             │
│ │   CACHE          │     │       able_async()       │             │
│  │ TTL: 60s         │     │       (surge_ping)       │             │
│  └────────┬─────────┘     └──────────┬───────────────┘             │
│           │                         │                               │
│           ▼                         ▼                               │
│  ┌──────────────────┐     ┌──────────────────────────┐             │
│  │ netsh commands   │     │ surge_ping / icmp_probe  │             │
│  │ (wlan/lan show   │     │                          │             │
│ │  interfaces)     │     │ Timeout: 2000ms           │             │
│  └──────────────────┘     └──────────────────────────┘             │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **SSID/Profile 查询**:
    *   调用 `get_wireless_ssid()` 或 `get_wired_network_profile()`。
    *   检查对应全局缓存 (`SSID_CACHE` / `WIRED_PROFILE_CACHE`)。
    *   若缓存命中且未过期 (TTL < 60s)，直接返回缓存值。
    *   若缓存未命中，执行 `netsh` 命令。
    *   解析输出文本，提取 SSID 或 Profile 名称。
    *   若解析成功，更新缓存并返回；若失败（如 netsh 报错或位置服务关闭），返回 `Ok(None)` 且不更新缓存（以便下次重试）。

2.  **网关可达性检查**:
    *   调用 `check_gateway_reachable(gateway)`。
    *   进入 `check_gateway_reachable_from`。
    *   **路径 A (Windows + IPv4)**: 调用 `crate::platform::icmp_probe::icmp_probe_v4`，使用底层 `IcmpSendEcho2Ex` API 进行快速、可靠的 ICMP 探测，支持绑源 IP。
    *   **路径 B (非 Windows 或 IPv6)**: 调用 `crate::infra::async_util::block_on_sync`，在同步上下文中驱动异步的 `gateway_reachable_async`。
    *   `gateway_reachable_async` 使用 `surge_ping` 库构造 ICMP 请求，设置 2000ms 超时，等待响应。
    *   返回 `true` (收到回应) 或 `false` (超时或错误)。

3.  **子网判断**:
    *   调用 `is_same_subnet_18(ip, gateway)`。
    *   解析 IP 和网关为 `u32`。
    *   应用 `/18` 掩码 (`0xFFFF_C000`) 进行按位与操作。
    *   比较结果，相等则在同一子网。

## Connections

*   [[gbk-console-output-decoding]]: `get_wireless_ssid_uncached` 和 `get_wired_network_profile_uncached` 中使用 `crate::platform::console_output::decode_console_bytes` 处理 netsh 输出的 GBK 编码，防止乱码导致解析失败。
*   `tauri-app/src-tauri/src/network/discovery.rs`: `new_command` 函数被导入用于构建 netsh 命令。
*   `tauri-app/src-tauri/src/platform/icmp_probe.rs`: 在 Windows 平台上，当需要 IPv4 网关探测时，委托给此模块的 `icmp_probe_v4` 函数，以解决 TUN 环境下的绑源问题。
*   `tauri-app/src-tauri/src/infra/async_util.rs`: 提供 `block_on_sync` 工具，用于在非异步上下文（如 `spawn_blocking` 线程）中运行异步 ICMP 探测代码。