---
title: 质量检测回退活动适配器与 SSID 位置权限告警
type: learning
source_files:
  - tauri-app/src-tauri/src/network/adapter.rs
  - tauri-app/src-tauri/src/network/subnet.rs
  - tauri-app/src-tauri/src/commands/network_cmd.rs
  - tauri-app/src-tauri/src/monitor/latency.rs
tags: [踩坑, 网络, 质量检测, SSID, Win11 位置权限]
---

## Overview

本模块负责 Windows 客户端的网络质量检测（Quality Check）与校园网状态判定。核心逻辑集中在 `network::adapter`（适配器选择）、`network::subnet`（子网/SSID/网关探测）以及 `monitor::latency`（定时检测循环）。

关键行为变更：
1. **质量检测回退机制**：`select_adapter_for_quality` (adapter.rs L150-L159) 在配置的主适配器离线时，回退到任意有 IP 的非黑名单适配器（通过 `discovery::is_blacklisted` 判断，见 L155），确保拔掉网线后仍能对 WLAN 测速，而非静默返回 `unknown`。
2. **SSID 查询失败告警**：`get_wireless_ssid` (subnet.rs L47-L56) 现在能识别 Win11 位置服务关闭导致的 `netsh` 静默失败（stdout 含“位置权限”或“location permission”，见 L71-L74），并通过 `warn_ssid_query_failed` (subnet.rs L41-L45) 记录一次警告日志（进程生命周期内仅一次，见 `SSID_QUERY_WARNED` L39），避免排障时“断腿无感知”。
3. **网关探测通道变更**：`check_gateway_reachable_from` (subnet.rs L175-L192) 在 Windows IPv4 下优先使用 `icmp_probe::icmp_probe_v4` 官方探测通道（L183-L187），避免 `surge_ping` 绑源在 TUN 环境下系统性失败。

## Key Components

### `tauri-app/src-tauri/src/network/adapter.rs`

**公开函数：**

- `find_by_name<'a>(adapters: &'a [Adapter], name: &str) -> Option<&'a Adapter>` (L16-L18): 按名称查找适配器。
- `find_with_valid_ip<'a>(adapters: &'a [Adapter], name: &str) -> Option<&'a Adapter>` (L21-L23): 按名称查找具有有效 IP 的适配器。
- `find_dual_adapters<'a>(...)` (L26-L39): 查找双适配器（a1, a2），a2 仅在 dual_adapter 且名称非空且与 a1 不同时查找。
- `is_secondary_adapter_enabled(config: &Config, adapter2_name: &str) -> bool` (L44-L46): 判断副适配器是否启用（dual_adapter 开关为 true 且 adapter2 名称非空）。
- `configured_disabled_adapters<'a>(config: &Config, disabled: &'a [DisabledAdapter]) -> Vec<&'a DisabledAdapter>` (L53-L64): 找出用户手选（非空且非 `AUTO_DETECT_ADAPTER`）且在禁用列表中的适配器；adapter2 需 dual_adapter 开启且手选才纳入（L59-L60）。
- `resolve_adapter_names(adapters: &[Adapter], config: &Config) -> (String, String)` (L66-L118): 解析主/副适配器名称。支持自动检测（优先有线、其次有 IP、最后任意，L68-L75）、配置名匹配及失效降级（L77-L88、L90-L115）。
- `filter_operation_adapters(adapters: &[Adapter], adapter1_name: &str, adapter2_name: &str) -> Vec<Adapter>` (L123-L128): 过滤出解析后的主/副适配器，用于 DHCP/检测等操作范围限制。
- `select_adapter(adapters: &[Adapter], config: &Config) -> (String, String)` (L130-L142): 登录出口语义选择。只从解析出的主适配器取 IP，**不回退**（L137-L139），若主适配器无 IP 则返回空（L141）。
- `select_adapter_for_quality(adapters: &[Adapter], config: &Config) -> (String, String)` (L150-L159): **质量检测专用选择**。先尝试 `select_adapter`；若主适配器无 IP，则回退到首个**有 IP 且未被黑名单命中**（`discovery::is_blacklisted`，L155）的适配器。黑名单过滤 VMware/蓝牙/TUN 等虚拟网卡，防止选中 NAT 内网口产出垃圾数据。
- `ensure_ethernet_ip_for_login(...)` (L161-L239): 登录前以太网 IP 检查。检测到以太网已连接但未获取 IP 时（L176），尝试 `ipconfig /renew` 并轮询等待（L199-L211），修复了 spawn 失败静默吞掉等历史缺陷（L204-L209）。

**测试模块 (`mod tests`，L241-L466)：**
- `resolve_adapter_names_falls_back_when_config_name_missing` (L334-L344): 验证配置名失效时降级到自动检测。
- `resolve_adapter_names_uses_config_when_present` (L346-L356): 验证配置名存在时使用配置名。
- `resolve_adapter_names_auto_detect_prefers_wired_with_ip` (L358-L367): 验证自动检测优先选有线且有 IP。
- `configured_disabled_adapters_manual_only` (L377-L419): 验证仅手选适配器纳入禁用列表（含哨兵与 dual 开关组合）。
- `quality_select_uses_primary_when_ip_present` (L421-L430): 验证主适配器有 IP 时直用。
- `quality_select_falls_back_to_active_when_primary_offline` (L432-L442): **关键测试**：复刻 2026-10-11 场景，配置以太网断开时回退到 WLAN。
- `quality_select_fallback_skips_virtual_adapters` (L444-L455): **关键测试**：验证回退时跳过恒有 IP 的 VMware VMnet1。
- `quality_select_returns_empty_when_nothing_active` (L457-L466): 验证全无活动网卡时返回空。

### `tauri-app/src-tauri/src/network/subnet.rs`

**常量与缓存：**
- `NETSH_QUERY_CACHE_TTL_SECS: u64 = 60` (L12): netsh 查询结果缓存有效期（秒）。
- `NetshCache` 类型别名 (L15): `(Instant, Option<String>)` 的互斥缓存。
- `SSID_CACHE` / `WIRED_PROFILE_CACHE` (L17-L20): 懒初始化缓存实例。
- `SSID_QUERY_WARNED` (L39): 静态原子布尔，保证 SSID 查询失败告警只发一次。

**公开函数：**

- `netsh_cache_get` (L23-L31): 命中未过期缓存时返回 `Some(缓存值)`。
- `netsh_cache_put` (L33-L35): 写入缓存（仅 Ok 结果）。
- `warn_ssid_query_failed(reason: &str)` (L41-L45): 进程内首次失败时记录 WiFi 名称匹配腿失效警告。
- `get_wireless_ssid() -> Result<Option<String>, String>` (L47-L56): 获取当前 WiFi SSID。带 TTL 缓存（L48-L50）；未命中则调 `get_wireless_ssid_uncached` 并缓存 Ok 值。
- `get_wireless_ssid_uncached()` (L58-L93): 内部未缓存查询，解析 `netsh wlan show interfaces` 输出；若退出码非 0（L64-L67）或 stdout 含位置权限提示（L71-L74）记录告警并返回 `Ok(None)`。
- `get_wired_network_profile() -> Result<Option<String>, String>` (L95-L104): 获取有线网络 Profile 名称。带 TTL 缓存。
- `get_wired_network_profile_uncached()` (L106-L133): 内部未缓存查询，解析 `netsh lan show interfaces`。
- `gateway_reachable_async(gateway: &str, source_ip: Option<&str>) -> bool` (L138-L173): surge_ping 异步 ICMP 实现（v6/非 Windows 路径）。
- `check_gateway_reachable_from(gateway: &str, source_ip: Option<&str>) -> bool` (L175-L192): 底层实现。Windows 下 IPv4 优先使用 `icmp_probe::icmp_probe_v4`（L183-L187）；其他走 `block_on_sync(gateway_reachable_async)`（L191）。
- `is_same_subnet_18(ip_str: &str, gateway_str: &str) -> bool` (L195-L206): 判断 IP 与网关是否在同一 /18 子网（掩码 255.255.192.0，L204-L205）。

**测试模块 (`mod tests`，L208-L242)：**
- `is_same_subnet_18_same_subnet_returns_true` (L212-L218)
- `is_same_subnet_18_different_subnet_returns_false` (L220-L228)
- `is_same_subnet_18_invalid_ip_returns_false` (L230-L235)
- `is_same_subnet_18_invalid_gateway_returns_false` (L237-L242)

### `tauri-app/src-tauri/src/commands/network_cmd.rs`

**公开 Command：**

- `empty_quality_json(quality: &str)` (L15-L17): 构造空质量 JSON（gatewayLatency 等字段为 -1）。
- `get_adapters(force: Option<bool>)` (L19-L25): 获取适配器列表（强制或缓存）。
- `enable_adapter(adapter_name: String)` (L27-L36): 启用适配器，允许 COM 提权失败后降级弹 UAC。
- `get_adapter_details()` (L38-L41): 获取适配器详情。
- `outbound_switch_now(app_handle: AppHandle)` (L44-L53): 立即切换出站，调用 `manual_outbound_switch`。
- `outbound_restore_now(app_handle: AppHandle)` (L56-L65): 手动立即还原出站，调用 `manual_outbound_restore`。
- `get_current_outbound_name()` (L69-L80): 获取当前真实出站接口别名（默认路由 metric 求和最小者），失败返回 None 软降级。
- `check_campus_status(app_handle: AppHandle)` (L82-L105): 检查校园网状态（SSID、子网、Portal 等），读取 `enable_network_name_check` 与 `required_network_name` 透传前端。
- `check_portal_status(adapter_ip: String, app_handle: AppHandle)` (L107-L137): 检查 Portal 状态，含注销保护期判断（L118-L125）。
- `get_campus_gateway(state: &AppState) -> String` (L139-L143): 读取校园网网关配置，空时取默认。
- `wrap_dhcp_result(results: Vec<serde_json::Value>)` (L145-L147): 包装 DHCP 结果为 `{success:true, results}`。
- `dhcp_renew_all(app_handle: AppHandle)` (L149-L166): DHCP 续租（仅限解析后主/副适配器中的有线网卡，L153-L158）。
- `dhcp_release_renew(app_handle: AppHandle)` (L168-L189): DHCP 释放/续租（MAC 重置 + 续租，范围同 L176-L181）。
- `dhcp_release_renew_adapter(adapter_name: String, app_handle: AppHandle)` (L191-L207): 单适配器 DHCP 释放/续租。
- `check_network_quality(app_handle: AppHandle)` (L209-L239): **手动质量检测**。总开关关闭返回 `disabled`（L213-L215），忙返回 `busy`（L216-L218）；调用 `select_adapter_for_quality` 获取适配器（L226），执行 `check_network_quality_async`（L232）并落盘历史（L235-L237）。
- `start_latency_test(app_handle: AppHandle, _state: State<'_, AppState>)` (L241-L267): 启动延迟测试循环，**校验总开关** `enable_network_quality`（L248-L254），未启用则回写 `enable_latency_test=false` 并返回；否则 spawn 循环并持久化开关（L256-L261）。
- `stop_latency_test(app_handle: AppHandle, state: State<'_, AppState>)` (L269-L280): 停止延迟测试循环并持久化开关为 false。
- `check_dns_doh_status()` (L282-L295): 检测 DNS/DoH 状态（非 Windows 返回不支持）。
- `setup_dns_doh(app_handle: AppHandle, family: Option<String>)` (L297-L383): 设置 DNS+DoH，操作范围限定 `dns_optimize_adapters` 名单且过滤黑名单（L318-L326）。
- `reset_dns(app_handle: AppHandle)` (L385-L476): 恢复 DNS 自动获取，范围同 setup_dns_doh（L398-L407）。

### `tauri-app/src-tauri/src/monitor/latency.rs`

**常量：**
- `BAD_LEVELS: &[&str] = &["poor", "bad"]` (L12): 质量档位恶化阈值。
- `GOOD_LEVELS: &[&str] = &["excellent", "great", "good"]` (L13): 质量档位恢复阈值。

**公开/内部函数：**

- `classify_quality_change(last: Option<&str>, current: &str, enable_notification: bool) -> Option<&'static str>` (L18-L38): 比较前后质量档位，返回 `"bad"`（恶化）、`"good"`（恢复）或 `None`。
- `record_last_quality(state: &AppState, current: &str)` (L40-L42): 记录上次质量档位到 state。
- `notify_quality_change(app_handle: &AppHandle, kind: &str)` (L44-L52): 发送“网络拥堵”或“网络恢复”通知及日志。
- `wait_for_adapter_ready(app_handle: &AppHandle, cancel_token: &CancellationToken) -> Option<(String, String)>` (L58-L76): **就绪等待**。轮询直到 `select_adapter_for_quality` 返回非空 IP 且 `any_adapter_online` 为真（L66-L68），每 2s 重试（L72）。
- `spawn_latency_test_loop(app_handle: &AppHandle) -> Result<(), String>` (L78-L139): 启动定时检测循环。动态读取间隔（L91-L107），调用 `wait_for_adapter_ready` 获取适配器后执行 `run_quality_check`（L135）。
- `run_startup_quality_once(app_handle: AppHandle, cancel_token: Arc<CancellationToken>)` (L144-L160): 启动首测。应用启动后等网络就绪执行一次全量检测（L146-L159）。

**测试模块 (`mod tests`，L162-L198)：**
- `degraded_to_bad_level_triggers_pending_bad` (L166-L170)
- `recovery_from_bad_level_triggers_good` (L172-L176)
- `same_level_or_unknown_last_never_notifies` (L178-L183)
- `middle_levels_crossing_never_notifies` (L185-L192)
- `notification_disabled_never_classifies` (L194-L198)

## Architecture

```text
┌─────────────────────────────────────────────┐
│        monitor/latency.rs                   │
│  spawn_latency_test_loop (L78)              │
│  run_startup_quality_once (L144)            │
│  wait_for_adapter_ready (L58)               │
└───────────────────┬─────────────────────────┘
                    │ calls
┌───────────────────▼─────────────────────────┐
│     network/adapter.rs                      │
│  select_adapter_for_quality (L150)          │
│  ├─ select_adapter (L130)                   │
│  └─ fallback non-blacklist (L155)           │
└───────────────────┬─────────────────────────┘
                    │ returns (ip, name)
┌───────────────────▼─────────────────────────┐
│   commands/network_cmd.rs                   │
│  check_network_quality (L209)               │
│  check_campus_status (L82)                  │
└───────────────────┬─────────────────────────┘
                    │ uses
┌───────────────────▼─────────────────────────┐
│     network/subnet.rs                       │
│  get_wireless_ssid (L47)                    │
│  check_gateway_reachable_from (L175)        │
│  is_same_subnet_18 (L195)                   │
└─────────────────────────────────────────────┘
```

## Data Flow

1. **适配器选择**：
   - `latency.rs:wait_for_adapter_ready` (L58-L76) 或 `network_cmd.rs:check_network_quality` (L209-L239) 调用 `adapter.rs:select_adapter_for_quality` (L150-L159)。
   - `select_adapter_for_quality` 首先调用 `select_adapter` (L130-L142) 获取配置主适配器的 IP（L151-L154）。
   - 若主适配器无 IP（如拔线），则遍历所有适配器，查找第一个 `!a.ip.is_empty() && !crate::network::discovery::is_blacklisted(&a.name)` (L155) 的适配器作为回退。
   - 若无任何活动适配器，返回空字符串（L157-L158）。

2. **SSID 查询**：
   - `subnet.rs:get_wireless_ssid` (L47-L56) 检查缓存（TTL 60s，L12、L23-L31）。
   - 未命中则执行 `get_wireless_ssid_uncached` (L58-L93) 调用 `netsh wlan show interfaces`。
   - 若退出码非 0（L64-L67）或 stdout 包含 "位置权限"/"location permission"（L71-L74），记录警告日志（进程内仅一次，L39-L45）并返回 `Ok(None)`。
   - 成功解析 SSID 后写入缓存（L53-L54）。

3. **质量检测执行**：
   - `network_cmd.rs:check_network_quality` (L209-L239) 总开关关闭返回 `disabled`（L213-L215），忙返回 `busy`（L216-L218）；获取 `(ip, name)` 后，调用 `check_network_quality_async`（L232）。
   - 结果序列化并通过 EventBus 推送前端，同时追加到质量历史文件（L235-L237）。

## Connections

[[adapter-operation-scope]]、[[background-check-and-auto-login]]

## Known Issues

- **DHCP 续租 spawn 失败静默吞掉**：`adapter.rs` 中 `ensure_ethernet_ip_for_login` 原实现 `ipconfig /renew` spawn 失败被 `if let Ok` 静默吞掉仍走 poll 并误报“续租超时”；现于 L204-L209 改为 `log_warn` 并记录 `child=None` 避免错误 kill。
- **surge_ping 绑源在 TUN 环境失败**：`subnet.rs` 中 `check_gateway_reachable_from` 原统一走 `surge_ping` 的 socket2 绑源 DGRAM，在 TUN 环境下系统性失败导致校园网卡误判非校园（夜间禁用名单漏卡）；Windows IPv4 路径已于 L183-L187 改为 `icmp_probe::icmp_probe_v4` 官方通道，v6/非 Windows 仍走 `surge_ping`（L191）。
- **定时延迟测试不校验总开关**：`network_cmd.rs` 中 `start_latency_test` 历史缺陷为运行路径不校验 `enable_network_quality` 总开关，关总开关后定时测试仍做全量外网检测；现于 L248-L254 开头校验，未启用时回写 `enable_latency_test=false` 并拒绝启动。
- **轻量化间隔动态读取**：`latency.rs` 中 `spawn_latency_test_loop` 原 `interval` 在 spawn 时捕获一次，运行中修改无效；现于 L91-L107 每轮重读配置与轻量化系数，计时器按需重建。
