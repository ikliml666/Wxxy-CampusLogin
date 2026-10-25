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
1. **质量检测回退机制**：`select_adapter_for_quality` (adapter.rs L150) 在配置的主适配器离线时，回退到任意有 IP 的非黑名单适配器（通过 `discovery::is_blacklisted` 判断），确保拔掉网线后仍能对 WLAN 测速，而非静默返回 `unknown`。
2. **SSID 查询失败告警**：`get_wireless_ssid` (subnet.rs L47) 现在能识别 Win11 位置服务关闭导致的 `netsh` 静默失败（stdout 含“位置权限”或“location permission”），并通过 `warn_ssid_query_failed` (subnet.rs L41) 记录一次警告日志（进程生命周期内仅一次，见 `SSID_QUERY_WARNED` L39），避免排障时“断腿无感知”。

## Key Components

### `tauri-app/src-tauri/src/network/adapter.rs`

**公开函数：**

- `find_by_name<'a>(adapters: &'a [Adapter], name: &str) -> Option<&'a Adapter>` (L16): 按名称查找适配器。
- `find_with_valid_ip<'a>(adapters: &'a [Adapter], name: &str) -> Option<&'a Adapter>` (L21): 按名称查找具有有效 IP 的适配器。
- `find_dual_adapters<'a>(...)` (L26): 查找双适配器（a1, a2），a2 仅在 dual_adapter 且名称非空且与 a1 不同时查找。
- `is_secondary_adapter_enabled(config: &Config, adapter2_name: &str) -> bool` (L44): 判断副适配器是否启用（dual_adapter 开关为 true 且 adapter2 名称非空）。
- `configured_disabled_adapters<'a>(config: &Config, disabled: &'a [DisabledAdapter]) -> Vec<&'a DisabledAdapter>` (L53): 找出用户手选（非空且非 AUTO_DETECT_ADAPTER）且在禁用列表中的适配器。
- `resolve_adapter_names(adapters: &[Adapter], config: &Config) -> (String, String)` (L66): 解析主/副适配器名称。支持自动检测（优先有线、其次有 IP、最后任意）、配置名匹配及降级。
- `filter_operation_adapters(adapters: &[Adapter], adapter1_name: &str, adapter2_name: &str) -> Vec<Adapter>` (L123): 过滤出解析后的主/副适配器，用于 DHCP/检测等操作范围限制。
- `select_adapter(adapters: &[Adapter], config: &Config) -> (String, String)` (L130): 登录出口语义选择。只从解析出的主适配器取 IP，**不回退**。若主适配器无 IP 则返回空。
- `select_adapter_for_quality(adapters: &[Adapter], config: &Config) -> (String, String)` (L150): **质量检测专用选择**。先尝试 `select_adapter`；若主适配器无 IP，则回退到首个**有 IP 且未被黑名单命中**（`discovery::is_blacklisted`）的适配器。黑名单过滤 VMware/蓝牙/TUN 等虚拟网卡，防止选中 NAT 内网口产出垃圾数据。
- `ensure_ethernet_ip_for_login(...)` (L161): 登录前以太网 IP 检查。检测到以太网已连接但未获取 IP 时，尝试 `ipconfig /renew` 并轮询等待，修复了 spawn 失败静默吞掉等历史缺陷。

**测试模块 (`mod tests`)：**
- `resolve_adapter_names_falls_back_when_config_name_missing` (L330): 验证配置名失效时降级到自动检测。
- `resolve_adapter_names_uses_config_when_present` (L342): 验证配置名存在时使用配置名。
- `resolve_adapter_names_auto_detect_prefers_wired_with_ip` (L354): 验证自动检测优先选有线且有 IP。
- `configured_disabled_adapters_manual_only` (L373): 验证仅手选适配器纳入禁用列表。
- `quality_select_uses_primary_when_ip_present` (L417): 验证主适配器有 IP 时直用。
- `quality_select_falls_back_to_active_when_primary_offline` (L428): **关键测试**：复刻 2026-10-11 场景，配置以太网断开时回退到 WLAN。
- `quality_select_fallback_skips_virtual_adapters` (L440): **关键测试**：验证回退时跳过恒有 IP 的 VMware VMnet1。
- `quality_select_returns_empty_when_nothing_active` (L453): 验证全无活动网卡时返回空。

### `tauri-app/src-tauri/src/network/subnet.rs`

**常量与缓存：**
- `NETSH_QUERY_CACHE_TTL_SECS: u64 = 60` (L12): netsh 查询结果缓存有效期（秒）。
- `SSID_QUERY_WARNED` (L39): 静态原子布尔，保证 SSID 查询失败告警只发一次。

**公开函数：**

- `get_wireless_ssid() -> Result<Option<String>, String>` (L47): 获取当前 WiFi SSID。带 TTL 缓存（L48）。若查询失败（退出码非 0 或 stdout 含位置权限提示），记录一次警告并返回 `Ok(None)`。
- `get_wireless_ssid_uncached()` (L58): 内部未缓存查询，解析 `netsh wlan show interfaces` 输出。
- `get_wired_network_profile() -> Result<Option<String>, String>` (L95): 获取有线网络 Profile 名称。带 TTL 缓存。
- `get_wired_network_profile_uncached()` (L106): 内部未缓存查询，解析 `netsh lan show interfaces`。
- `check_gateway_reachable(gateway: &str) -> bool` (L135): 网关可达性检查同步包装器。
- `check_gateway_reachable_from(gateway: &str, source_ip: Option<&str>) -> bool` (L179): 底层实现。Windows 下 IPv4 优先使用 `icmp_probe::icmp_probe_v4`（官方探测通道，绑源可靠，L187-L191）；其他走 `gateway_reachable_async` (L142) 的 surge_ping 实现。
- `is_same_subnet_18(ip_str: &str, gateway_str: &str) -> bool` (L199): 判断 IP 与网关是否在同一 /18 子网（掩码 255.255.192.0）。

**测试模块 (`mod tests`)：**
- `is_same_subnet_18_same_subnet_returns_true` (L216): 验证同 /18 子网返回 true。
- `is_same_subnet_18_different_subnet_returns_false` (L224): 验证不同子网返回 false。
- `is_same_subnet_18_invalid_ip_returns_false` (L234): 验证无效 IP 返回 false。
- `is_same_subnet_18_invalid_gateway_returns_false` (L241): 验证无效网关返回 false。

### `tauri-app/src-tauri/src/commands/network_cmd.rs`

**公开 Command：**

- `get_adapters(force: Option<bool>)` (L19): 获取适配器列表（强制或缓存）。
- `enable_adapter(adapter_name: String)` (L27): 启用适配器，允许 COM 提权失败后降级弹 UAC。
- `get_adapter_details()` (L38): 获取适配器详情。
- `outbound_switch_now(app_handle: AppHandle)` (L44): 立即切换出站，调用 `manual_outbound_switch`。
- `outbound_restore_now(app_handle: AppHandle)` (L56): 手动立即还原出站，调用 `manual_outbound_restore`。
- `get_current_outbound_name()` (L70): 获取当前真实出站接口别名（默认路由 metric 求和最小者），失败返回 None 软降级。
- `check_campus_status(app_handle: AppHandle)` (L83): 检查校园网状态（SSID、子网、Portal 等）。
- `check_portal_status(adapter_ip: String, app_handle: AppHandle)` (L105): 检查 Portal 状态，含注销保护期判断（115-122行）。
- `dhcp_renew_all(app_handle: AppHandle)` (L146): DHCP 续租（仅限解析后主/副适配器中的有线网卡）。
- `dhcp_release_renew(app_handle: AppHandle)` (L165): DHCP 释放/续租（MAC 重置 + 续租）。
- `dhcp_release_renew_adapter(adapter_name: String, app_handle: AppHandle)` (L188): 单适配器 DHCP 释放/续租。
- `check_network_quality(app_handle: AppHandle)` (L206): **手动质量检测**。调用 `select_adapter_for_quality` 获取适配器，执行检测并落盘历史。
- `start_latency_test(app_handle: AppHandle, _state: State<'_, AppState>)` (L238): 启动延迟测试循环，校验总开关。
- `stop_latency_test(app_handle: AppHandle, state: State<'_, AppState>)` (L266): 停止延迟测试循环。
- `check_dns_doh_status()` (L279): 检测 DNS/DoH 状态。
- `setup_dns_doh(app_handle: AppHandle, family: Option<String>)` (L294): 设置 DNS+DoH。
- `reset_dns(app_handle: AppHandle)` (L382): 恢复 DNS 自动获取。

### `tauri-app/src-tauri/src/monitor/latency.rs`

**常量：**
- `BAD_LEVELS: &[&str] = &["poor", "bad"]` (L12): 质量档位恶化阈值。
- `GOOD_LEVELS: &[&str] = &["excellent", "great", "good"]` (L13): 质量档位恢复阈值。

**公开/内部函数：**

- `classify_quality_change(last: Option<&str>, current: &str, enable_notification: bool) -> Option<&'static str>` (L18): 比较前后质量档位，返回 `"bad"`（恶化）、`"good"`（恢复）或 `None`。
- `record_last_quality(state: &AppState, current: &str)` (L40): 记录上次质量档位。
- `notify_quality_change(app_handle: &AppHandle, kind: &str)` (L44): 发送“网络拥堵”或“网络恢复”通知。
- `wait_for_adapter_ready(app_handle: &AppHandle, cancel_token: &CancellationToken) -> Option<(String, String)>` (L58): **就绪等待**。轮询直到 `select_adapter_for_quality` 返回非空 IP 且适配器在线。每 2s 重试。
- `spawn_latency_test_loop(app_handle: &AppHandle) -> Result<(), String>` (L78): 启动定时检测循环。动态读取间隔，调用 `wait_for_adapter_ready` 获取适配器后执行 `run_quality_check`。
- `run_startup_quality_once(app_handle: AppHandle, cancel_token: Arc<CancellationToken>)` (L144): 启动首测。应用启动后等网络就绪执行一次全量检测。

**测试模块 (`mod tests`)：**
- `degraded_to_bad_level_triggers_pending_bad` (L166): 验证恶化到 bad 级别触发 pending。
- `recovery_from_bad_level_triggers_good` (L172): 验证从 bad 恢复触发 good。
- `same_level_or_unknown_last_never_notifies` (L178): 验证同级别或未知不通知。
- `middle_levels_crossing_never_notifies` (L185): 验证中间档位互转不通知。
- `notification_disabled_never_classifies` (L194): 验证关闭通知时不分类。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│                        Network Quality Flow                         │
└─────────────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────▼───────────────┐
              │       latency.rs              │
              │  spawn_latency_test_loop      │
              │  run_startup_quality_once     │
              └───────────────┬───────────────┘
                              │ wait_for_adapter_ready
              ┌───────────────▼───────────────┐
              │       adapter.rs              │
              │  select_adapter_for_quality   │
              │  ├─ select_adapter (primary)  │
              │  └─ fallback (non-blacklist)  │
              └───────────────┬───────────────┘
                              │ (ip, name)
              ┌───────────────▼───────────────┐
              │    network_cmd.rs             │
              │  check_network_quality        │
              │  check_campus_status          │
              └───────────────┬───────────────┘
                              │
              ┌───────────────▼───────────────┐
              │       subnet.rs               │
              │  get_wireless_ssid            │
              │  check_gateway_reachable      │
              │  is_same_subnet_18            │
              └───────────────────────────────┘
```

## Data Flow

1. **适配器选择**：
    - `latency.rs:wait_for_adapter_ready` (L58) 或 `network_cmd.rs:check_network_quality` (L206) 调用 `adapter.rs:select_adapter_for_quality` (L150)。
    - `select_adapter_for_quality` 首先调用 `select_adapter` (L130) 获取配置主适配器的 IP。
    - 若主适配器无 IP（如拔线），则遍历所有适配器，查找第一个 `!a.ip.is_empty() && !crate::network::discovery::is_blacklisted(&a.name)` (L155) 的适配器作为回退。
    - 若无任何活动适配器，返回空字符串。

2. **SSID 查询**：
    - `subnet.rs:get_wireless_ssid` (L47) 检查缓存（TTL 60s，L12）。
    - 未命中则执行 `get_wireless_ssid_uncached` (L58) 调用 `netsh wlan show interfaces`。
    - 若退出码非 0 或 stdout 包含 "位置权限"/"location permission"，记录警告日志（进程内仅一次，L39-L45）并返回 `Ok(None)`。
    - 成功解析 SSID 后写入缓存。

3. **质量检测执行**：
    - `network_cmd.rs:check_network_quality` (L206) 获取 `(ip, name)` 后，调用 `check_network_quality_async`。
    - 结果序列化并通过 EventBus 推送前端，同时追加到质量历史文件（L232）。

## Connections

[[adapter-operation-scope]]、[[background-check-and-auto-login]]
