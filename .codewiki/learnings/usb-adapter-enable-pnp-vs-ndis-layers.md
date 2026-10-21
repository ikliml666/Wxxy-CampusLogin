---
title: USB 网卡启用失效——NDIS admin 层与 PnP 设备层禁用互不覆盖
type: learning
source_files:
  - tauri-app/src-tauri/src/network/adapter_cache.rs
  - tauri-app/src-tauri/src/network/discovery/devnode.rs
  - tauri-app/src-tauri/src/network/discovery/mod.rs
tags: [教训, 适配器, 启用, 两层状态模型, netsh, PnP, USB, 历史缺陷]
---

## Overview

本文档记录了解决 Windows 外接 USB 网络适配器（如 RTL8156）在“幽灵态”或物理禁用后，常规 `netsh` 启用命令失效问题的根因分析与修复方案。核心发现是 Windows 的“禁用”概念横跨两个独立层级：**NDIS 管理层**（`netsh` 操作）与 **PnP 设备层**（设备管理器/pnputil 操作）。

针对 USB 设备的特殊性（重枚举耗时、驱动切换），系统实现了基于 `cfgmgr32.dll` 的设备级状态检测与启用逻辑，并通过 `helper` 模块提供安全的提权执行通道。

## Key Components

### `tauri-app/src-tauri/src/network/adapter_cache.rs`
**职责**：适配器缓存管理与启用入口。封装 TTL 缓存逻辑，并作为 NDIS/PnP 启用的统一调度器。

*   **常量**:
    *   `ADAPTER_CACHE_TTL_SECS`: `5` (秒)，行 21。
    *   `CACHE_REFRESH_INTERVAL_SECS`: `4` (秒)，行 272。
    *   `FORBIDDEN`: 非法字符集 `['&', '|', ';', ...]`，行 110。
*   **数据结构**:
    *   `AdapterCacheEntry`: `(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>, Instant)`，行 15。
    *   `ADAPTER_CACHE`: `RwLock<Option<AdapterCacheEntry>>`，全局静态缓存，行 18。
*   **关键函数**:
    *   `query_adapters_cached_inner()`: 内部私有方法。检查 TTL，命中返回克隆；未命中调用 `discovery::query_adapters_addresses()` 并写锁更新，行 23。
    *   `get_all_adapters_cached()`: 公共入口，委托给 `query_adapters_cached_inner`，行 44。
    *   `get_adapters_cached()`: 优化读取。命中时仅克隆 `adapters` 字段 (BE-B-04 优化)，行 50。
    *   `get_adapters_cached_async()`: 异步入口。快速路径直接返回；慢路径通过 `tokio::task::spawn_blocking` 执行同步查询，行 72。
    *   `get_adapters_force()`: 强制刷新。`take()` 清空缓存后调用 `get_adapters_cached`，触发立即重查，行 88。
    *   `get_adapter_details_cached()`: 仅获取 `details` 字段，行 93。
    *   `validate_adapter_name()`: 校验名称非空、长度<=128、无非法字符，行 107。
    *   `enable_adapter(adapter_name, allow_uac_prompt)`: **核心业务逻辑**。
        *   参数：`adapter_name`, `allow_uac_prompt` (控制非管理员下是否允许弹 UAC)，行 122。
        *   **PnP 分流逻辑** (行 129-168):
            *   调用 `find_disabled_device` 检测设备状态。
            *   若为 `DeviceState::Disabled` (Problem 22): 调用 `enable_device` (pnputil)，然后清缓存。
            *   若为 `DeviceState::Unlocatable` (幽灵态): 调用 `reenumerate_device` (总线重扫)，随后再次检查是否仍禁用，若是则再调 `enable_device`。
            *   若为 `None` (正常或无 PnP 实例): 走传统 `netsh` 路径。
        *   **提权逻辑**:
            *   管理员: 直调 `netsh interface set interface ... enable`，行 173。
            *   非管理员: 调用 `helper_spawn::spawn_elevated_helper`，经计划任务/CMSTPLUA 提权，行 191。
        *   **后置清理** (行 209-216): 清 `ADAPTER_CACHE`，刷新注册表缓存 (`refresh_class_subkey_cache`)，失效 `ShowInNetworkConnections` 缓存。
    *   `wait_for_adapter()`: 轮询等待适配器出现。初始延迟 1s，指数退避至 5s。使用 `get_adapters_force` 强刷，行 222。
    *   `poll_adapter_ip_quick()`: 快速 IP 续租检测。300ms 间隔，对比初始 IP 与新 IP，行 243。
    *   `start_cache_refresh_task()`: 启动后台任务。每 4 秒 `spawn_blocking` 执行 `query_adapters_cached_inner`，失败则保留旧缓存并记录 warn，行 279。

### `tauri-app/src-tauri/src/network/discovery/devnode.rs`
**职责**：通过 Windows SetupAPI (`cfgmgr32.dll`) 直接操作 PnP 设备树，解决 `netsh` 无法处理的设备级禁用（Problem 22）和幽灵态问题。

*   **常量**:
    *   `NET_CLASS_GUID`: `"{4D36E972-E325-11CE-BFC1-08002BE10318}"`，行 26。
    *   `CONTROL_NETWORK_ROOT`: `"SYSTEM\\CurrentControlSet\\Control\\Network"`，行 27。
    *   `ENABLE_VERIFY_TOTAL_MS`: `3000` ms，普通设备启用复核窗口，行 32。
    *   `USB_VERIFY_TOTAL_MS`: `15000` ms，USB 设备启用复核窗口（因物理重枚举慢），行 39。
    *   `REENUMERATE_VERIFY_TOTAL_MS`: `15000` ms，总线重枚举复核窗口，行 43。
    *   `ENABLE_VERIFY_INTERVAL_MS`: `200` ms，复核轮询间隔，行 33。
*   **数据结构**:
    *   `DeviceState`: 枚举 `Disabled(String instance_id)` 或 `Unlocatable(String instance_id)`，行 120。
*   **关键函数**:
    *   `verify_window_ms(instance_id)`: 根据 `instance_id` 前缀判断是否为 USB 设备，返回对应复核窗口时长，行 46。
    *   `locate_devinst()`: 调用 `CM_Locate_DevNodeW` (带 `PHANTOM` 标志)，定位设备实例句柄，行 55。
    *   `devnode_problem()`: 调用 `CM_Get_DevNode_Status`，返回 Problem Code (0 表示无问题，22 表示禁用)，行 72。
    *   `read_pnp_instance_id()`: 从注册表 `Control\Network\{GUID}\Connection` 读取 `PnPInstanceId`，行 92。
    *   `find_guid_by_name()`: 回退查找。遍历注册表键，按 `Name` 匹配适配器名获取 GUID，行 102。
    *   `find_disabled_device()`: **状态检测核心**。
        *   优先从 `adapter_cache` 取 GUID，否则用 `find_guid_by_name`，行 137。
        *   读取 `PnPInstanceId`。
        *   调用 `devnode_problem`：
            *   返回 22 -> `Ok(Some(Disabled))`。
            *   返回其他成功值 -> `Ok(None)` (正常)。
            *   返回错误 (如 0x0D CR_NO_SUCH_DEVNODE) -> `Ok(Some(Unlocatable))`。**关键修正**：查询失败不再被视为“未禁用”，而是“不可定位”，行 158。
    *   `disable_device()`: 调用 `CM_Disable_DevNode` (非持久化)，然后轮询复核 Problem 22 出现，行 175。
    *   `disable_adapter_nonpersistent()`: 按名禁用。若无 PnP 实例（虚拟卡），返回 `Ok(None)` 供调用方回退 netsh，行 199。
    *   `enable_device()`: **设备级启用核心**。
        *   非管理员：走 `helper_spawn` 提权。
        *   管理员：调用 `pnputil /enable-device`。
        *   **复核**：轮询 `devnode_problem`，直到不为 22。USB 设备使用更长窗口，行 219。
    *   `reenumerate_device()`: **总线重枚举**。
        *   非管理员：走 `helper_spawn` 提权。
        *   管理员：调用 `reenumerate_device_raw`。
        *   逻辑：获取父 DevInst，调用 `CM_Reenumerate_DevNode(SYNCHRONOUS)`，行 285。
    *   `reenumerate_device_raw()`: 执行重枚举并复核设备重新可定位（`devnode_problem` 不再报错），行 310。

### `tauri-app/src-tauri/src/network/discovery/mod.rs`
**职责**：定义适配器发现的基础类型、黑名单规则及平台分发入口。

*   **数据结构**:
    *   `AdapterQueryResult`: `Result<(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>), String>`，行 22。
    *   `AdapterStatus`: 枚举 `Disabled`, `Disconnected`, `EnabledNoIp`, `Connected`，行 41。
    *   `Adapter`: 包含 `name`, `ip`, `wireless`, `guid`, `mac`, `if_index`, `status`, `link_speed`，行 50。
    *   `AdapterDetail`: 比 `Adapter` 多 `subnet_mask`, `gateway`, `dhcp_server`，行 64。
    *   `DisabledAdapter`: 包含 `name`, `status`, `description`。注意 `status` 序列化为英文 key，行 80。
*   **关键函数**:
    *   `BL_REGEX`: 大型正则表达式，用于过滤虚拟/代理/隧道网卡。包含英文关键词 (`hyper-v`, `virtual`, `vpn`, `clash`, `tailscale` 等) 和中文关键词 (`虚拟`, `伪`, `本地连接` 等)。使用了 `\b` 词边界防止误伤 (如 "Native")，行 31。
    *   `new_command()`: 创建子进程命令，Windows 下附加 `CREATE_NO_WINDOW` 标志 (`0x08000000`)，行 89。
    *   `is_blacklisted()`: 基于 `BL_REGEX` 判断，行 97。
    *   `prefix_len_to_mask()`: IPv4 前缀转掩码字符串，行 103。
    *   `query_adapters_addresses()`: 平台分发。Windows 调用 `self::windows::query_adapters_addresses()`，其他平台返回空，行 116。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Tauri App (Main Process)                            │
│  ┌─────────────────────────┐  ┌──────────────────────────┐                  │
│  │ adapter_cache.rs        │  │ discovery/devnode.rs     │                  │
│  │                         │  │                          │                  │
│  │ enable_adapter()        │──► find_disabled_device()  │                  │
│  │ (Row 122)               │  │ (Row 137)                │                  │
│  │                         │  │                          │                  │
│  │ ┌─────────────────────┐ │  │ ┌──────────────────────┐ │                  │
│  │ │ PnP State Check     │ │  │ │ devnode_problem()    │ │                  │
│  │ │ (Row 129-168)       │ │  │ │ (Row 72)             │ │                  │
│  │ └─────────┬───────────┘ │  │ └──────────┬───────────┘ │                  │
│  │           ▼             │  │            ▼             │                  │
│  │ ┌─────────────────────┐ │  │ ┌──────────────────────┐ │                  │
│  │ │ DeviceState::       │ │  │ │ CM_Get_DevNode_Status│ │                  │
│  │ │ Disabled/Unlocat.   │ │  │ │ (Problem 22 / Error) │ │                  │
│  │ └─────────┬───────────┘ │  │ └──────────┬───────────┘ │                  │
│  │           ▼             │  │            ▼             │                  │
│  │ ┌─────────────────────┐ │  │ ┌──────────────────────┐ │                  │
│  │ │ enable_device()     │ │  │ │ read_pnp_instance_id │ │                  │
│  │ │ (Row 219)           │ │  │ │ (Row 92)             │ │                  │
│  │ └─────────┬───────────┘ │  │ └──────────┬───────────┘ │                  │
│  │           ▼             │  │            ▼             │                  │
│  │ ┌─────────────────────┐ │  │ ┌──────────────────────┐ │                  │
│  │ │ pnputil /enable     │ │  │ │ Registry Query       │ │                  │
│  │ │ (Admin) or          │ │  │ │ HKLM...\Network\...  │ │                  │
│  │ │ helper_spawn()      │ │  │ └──────────────────────┘ │                  │
│  │ │ (Non-Admin)         │ │  │                          │                  │
│  │ └─────────┬───────────┘ │  │                          │                  │
│  │           ▼             │  │                          │                  │
│  │ ┌─────────────────────┐ │  │                          │                  │
│  │ │ Poll Verify         │ │  │                          │                  │
│  │ │ (Row 219+)          │ │  │                          │                  │
│  │ └─────────┬───────────┘ │  │                          │                  │
│  │           ▼             │  │                          │                  │
│  │ ┌─────────────────────┐ │  │                          │                  │
│  │ │ Clear Cache &       │ │  │                          │                  │
│  │ │ Refresh Reg         │ │  │                          │                  │
│  │ │ (Row 209-216)       │ │  │                          │                  │
│  │ └─────────────────────┘ │  │                          │                  │
│  └─────────────────────────┘  └──────────────────────────┘                  │
└─────────────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Helper Process (Elevated)                           │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ helper/mod.rs                                                         │  │
│  │                                                                       │  │
│  │ run_enable_device (Row 963)                                           │  │
│  │ ├─ valid_instance_id (Row 950)                                        │  │
│  │ ├─ devnode_problem (Existence Check)                                  │  │
│  │ ├─ pnputil /enable-device                                             │  │
│  │ └─ Poll Verify (Row 974-998)                                          │  │
│  │                                                                       │  │
│  │ run_reenumerate_device (Row 1030)                                     │  │
│  │ └─ devnode::reenumerate_device_raw                                    │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **入口层**: `adapter_cache.rs` 是上层业务（如监控循环、用户按钮）的主要交互点。
2.  **缓存层**:
    *   `adapter_cache::get_adapters_cached` / `..._async` 调用 `adapter_cache::query_adapters_cached_inner`。
    *   `query_adapters_cached_inner` 在缓存未命中时调用 `discovery::query_adapters_addresses`。
    *   `discovery::query_adapters_addresses` 分发到 `discovery::windows::query_adapters_addresses` (本批未展示 windows.rs 源码，但依赖其存在)。
3.  **启用流程**:
    *   `adapter_cache::enable_adapter` 是总控。
    *   首先调用 `discovery::devnode::find_disabled_device` 进行 PnP 状态探测。
    *   **分支 A (PnP 禁用)**: 调用 `discovery::devnode::enable_device` (pnputil)。
    *   **分支 B (幽灵态)**: 调用 `discovery::devnode::reenumerate_device` (总线重扫)，可能再次调用 `enable_device`。
    *   **分支 C (NDIS 层)**: 调用 `netsh` 命令或通过 `platform::helper_spawn` 提权执行 netsh。
    *   所有分支最终都调用 `adapter_cache` 内部的 `ADAPTER_CACHE.write().take()` 清除缓存，并调用 `discovery::registry::refresh_class_subkey_cache` 等同步注册表缓存。
4.  **注册表依赖**:
    *   `devnode.rs` 中的 `read_pnp_instance_id` 和 `find_guid_by_name` 直接读取 Windows 注册表 (`winreg`)。
    *   `adapter_cache.rs` 中的启用后置操作依赖 `discovery::registry` 模块来刷新相关缓存。
5.  **Helper 执行流**:
    *   非管理员路径触发 `helper_spawn::spawn_elevated_helper`。
    *   Helper 进程解析 `--helper` 参数，构建 `HelperOp::EnableDevice`。
    *   执行 `run_enable_device`，通过 `pnputil` 操作并轮询验证。
    *   结果写入 `{ProgramData}/CampusLogin/results/<result_name>.json`。

## Connections

[[adapter-disabled-classification-blindspot]]、[[adapter-visibility-cache-staleness]]、
[[cmstplua-elevation-bind-opts3-and-vtable-slot]]（CMSTPLUA 静默提权链的坑与修复）