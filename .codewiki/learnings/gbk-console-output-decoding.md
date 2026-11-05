---
title: 校园网网络诊断模块 (Subnet & DHCP)
type: learning
source_files:
  - tauri-app/src-tauri/src/network/subnet.rs
  - tauri-app/src-tauri/src/network/dhcp.rs
tags: [网络, Windows, netsh, DHCP, MAC, 编码, surge_ping]
---

## Overview

本模块负责校园网环境下的网络诊断与配置管理，包括 SSID/有线配置文件查询、网关可达性探测、DHCP 续租/释放以及 MAC 地址重置。所有命令行工具输出均通过统一的编码解码层处理，以兼容不同 Windows 代码页（如 UTF-8 与 GBK）。

## Key Components

### subnet.rs: 子网计算与网关探测

-   **常量**:
    -   `NETSH_QUERY_CACHE_TTL_SECS` (行 14): 设置 netsh 查询结果缓存有效期为 60 秒，避免高频 spawn 子进程。
-   **函数**:
    -   `get_wireless_ssid() -> Result<Option<String>, String>` (行 63): 查询当前 WiFi SSID。命中缓存则直接返回；否则调用 `netsh wlan show interfaces`，并使用 `decode_console_bytes` 解析输出。支持 Win11 位置服务关闭时的错误提示检测。
    -   `get_wired_network_profile() -> Result<Option<String>, String>` (行 105): 查询有线网络连接的网络配置文件名。调用 `netsh lan show interfaces`，解析多语言（英/简中/繁中）"profile" 关键字。
    -   `check_gateway_reachable(gateway: &str) -> bool` (行 133): 检查网关 ICMP 可达性。Windows 下优先使用 `icmp_probe_v4` (绑定源 IP)，非 Windows 或其他情况使用 `surge_ping` 异步探测。
    -   `is_same_subnet_18(ip_str: &str, gateway_str: &str) -> bool` (行 176): 判断 IP 与网关是否在同一个 /18 子网 (掩码 `255.255.192.0`)。用于过滤非校园网适配器。

### dhcp.rs: DHCP 操作与 MAC 重置

-   **函数**:
    -   `dhcp_renew(adapter_name: &str) -> Result<bool, String>` (行 40): 执行 `ipconfig /renew`。通过 `ipconfig_output_failed` 检查输出文本中的错误关键字（中英双语），而非仅依赖退出码。
    -   `dhcp_release(adapter_name: &str) -> Result<bool, String>` (行 56): 执行 `ipconfig /release`。同样进行输出文本错误检查。
    -   `dhcp_renew_wired_only(targets: &[String]) -> Result<Vec<serde_json::Value>, String>` (行 71): 仅对目标列表中的有线适配器执行 DHCP 续租。
    -   `generate_random_mac() -> String` (行 103): 生成随机 MAC 地址。使用 `getrandom` (BCryptGenRandom) 填充 6 字节，确保单播/本地管理位正确。低概率失败时降级回时间+计数器种子。
    -   `set_mac_via_registry(adapter_guid: &str, mac_no_dash: &str) -> Result<(), String>` (行 126): 通过注册表修改网卡 MAC 地址 (`NetworkAddress` 值)。需要管理员权限。
    -   `remove_mac_from_registry(adapter_guid: &str) -> Result<(), String>` (行 148): 清理注册表中的 MAC 地址伪装值。
    -   `netsh_disable/enable(adapter_name: &str) -> bool` (行 163, 173): 使用 `netsh interface set interface admin=disable/enable` 停用/启用网卡。
    -   `poll_ip_change(...)` / `poll_adapter_has_ip(...)` (行 183, 196): 轮询适配器 IP 变更，等待 DHCP 分配新 IP。
    -   `apply_mac_change_via_registry(...)` (行 213): 原子性地执行 MAC 修改流程：写注册表 -> 释放 DHCP -> 停用网卡 -> 启用网卡 -> 续租 DHCP。
    -   `try_modify_mac(...)` (行 243): 尝试修改 MAC。若为管理员则直写注册表；若非管理员则通过 `--helper` 提权重启自身执行。
    -   `renew_adapter_with_mac(...)` (行 286): 对单个适配器执行完整的 MAC 重置 + DHCP 流程，返回 JSON 结果。包含虚拟适配器黑名单过滤和非校园网子网跳过逻辑。
    -   `dhcp_release_renew_all(...)` / `dhcp_release_renew_single(...)` (行 356, 373): 批量或单个执行 MAC 重置 + DHCP 流程。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                     Network Module                          │
├──────────────────────┬──────────────────────────────────────┤
│   subnet.rs          │   dhcp.rs                            │
│                      │                                      │
│  ┌────────────────┐  │  ┌────────────────────────────────┐  │
│  │ Netsh Queries  │  │  │ DHCP Operations                │  │
│  │ (SSID/Profile) │  │  │ (Renew/Release)                │  │
│  └───────┬────────┘  │  └───────────────┬────────────────┘  │
│          │           │                  │                   │
│  ┌───────▼────────┐  │  ┌───────────────▼────────────────┐  │
│  │ Cache Layer    │  │  │ MAC Address Management         │  │
│  │ (TTL 60s)      │  │  │ (Registry + netsh disable/en)  │  │
│  └────────────────┘  │  └───────────────┬────────────────┘  │
│                      │                  │                   │
│  ┌────────────────┐  │  ┌───────────────▼────────────────┐  │
│  │ Gateway Check  │  │  │ Helper Spawn (Elevation)       │  │
│  │ (surge_ping /  │  │  │ (--helper mac)                 │  │
│  │ icmp_probe)    │  │  └────────────────────────────────┘  │
│  └────────────────┘  │                                      │
├──────────────────────┴──────────────────────────────────────┤
│  Platform Console Output Decoder (decode_console_bytes)     │
│  Handles UTF-8 / OEM (GBK) conversion for CLI tool output  │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

### MAC 地址重置流程 (`renew_adapter_with_mac`)

1.  **过滤**: 检查适配器是否在虚拟网卡黑名单中 (`is_blacklisted`) 或非校园网子网 (`is_same_subnet_18`)。
2.  **生成 MAC**: 调用 `generate_random_mac()` 生成新 MAC。
3.  **尝试修改**:
    -   **管理员模式**: 直接调用 `set_mac_via_registry`。
    -   **非管理员模式**: 通过 `helper_spawn::spawn_elevated_helper` 启动提权辅助进程执行修改。
4.  **网卡重启**:
    -   调用 `dhcp_release` 释放旧 IP。
    -   调用 `netsh_disable` 停用网卡。
    -   等待 500ms。
    -   调用 `netsh_enable` 启用网卡。
    -   轮询等待新 IP (`poll_ip_change`)。
5.  **清理**: 调用 `remove_mac_from_registry` 清除伪装 MAC（若成功修改）。
6.  **结果**: 返回包含成功状态、新 IP 和原因的消息。

### 网关可达性探测 (`check_gateway_reachable`)

1.  **Windows 优化**: 若为 IPv4 网关，优先使用 `icmp_probe_v4`，支持绑定源 IP，避免 TUN 环境下 `surge_ping` 的绑源失败问题。
2.  **通用路径**: 构建 `surge_ping::Config`，指定 ICMP 版本 (V4/V6) 和源 IP (可选)。
3.  **异步探测**: 使用 `tokio::time::timeout` 包裹 `pinger.ping`，超时设为 2000ms。
4.  **同步封装**: 外部调用者通常在 `spawn_blocking` 线程中通过 `block_on_sync` 调用此异步函数。

## Known Issues

-   **Win11 位置服务**: 若关闭位置服务，`netsh wlan show interfaces` 可能永久失败或返回空 SSID。代码通过检测 "位置权限" 关键字并仅告警一次 (`SSID_QUERY_WARNED`) 来处理此情况。
-   **IPCONFIG 退出码**: `ipconfig` 失败时退出码常为 0，必须依赖 `ipconfig_output_failed` 检查输出文本中的错误关键字。
-   **MAC 伪装兼容性**: 部分网卡驱动不支持 MAC 伪装，即使注册表修改成功且 DHCP 续租完成，IP 也可能不变。代码通过检查 `ip_changed` 标志来识别此情况并返回警告。

## Connections

-   [[adapter.rs]]: 原适配器管理模块，已迁移至本模块。
-   [[discovery.rs]]: 适配器发现与黑名单定义。
-   [[helper/mod.rs]]: 提权辅助进程实现。
-   [[platform/console_output.rs]]: 控制台输出解码器实现。
-   [[platform/icmp_probe.rs]]: Windows 专用 ICMP 探测实现。
