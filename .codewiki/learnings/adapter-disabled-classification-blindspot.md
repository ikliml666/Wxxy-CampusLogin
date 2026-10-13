---
title: Windows 适配器发现与监控——四分类决策、幽灵设备修复与自动启用守护
type: learning
source_files:
  - tauri-app/src-tauri/src/network/discovery/windows.rs
  - tauri-app/src-tauri/src/monitor/adapter_watch.rs
tags: [教训, 适配器, 禁用分类, 自动启用, API语义, 历史缺陷]
---

## Overview

本模块负责 Windows 平台下的网络适配器状态发现（Discovery）与运行时监控（Monitoring）。核心职责包括：通过 Win32 `GetAdaptersAddresses` 获取适配器列表，结合注册表可见性、`GetIfEntry2` 的 `AdminStatus` 以及 PnP 设备树状态，将适配器严格划分为四类状态（Connected, EnabledNoIp, Disabled, Disconnected）。监控端 (`adapter_watch`) 每 15 秒轮询一次，检测适配器变更并触发 UI 更新、禁用警告、后台检查或自动启用尝试。

**核心教训：** 旧实现仅依赖 `OperStatus == NotPresent` 且注册表 `ConfigFlags` 含 `DISABLED` 位来判定“已禁用”，导致大量“禁用后 OperStatus 报 Down”的网卡被误判为“未连接”，使得自动启用功能静默失效。修复方案引入了文档化的 `AdminStatus` 字段作为主判定依据，并增加了 PnP 设备树存活检查以修复“拔出 USB 网卡后残影误判为已禁用”的问题。

## Key Components

### `discovery/windows.rs` (Windows 专有适配器发现)

- **`query_adapters_addresses()`**: 调用 Win32 `GetAdaptersAddresses` 获取原始数据。包含重试逻辑以处理缓冲区不足 (`ERROR_BUFFER_OVERFLOW`, code 111)。
- **`parse_adapter_addresses(...)`**: 解析 `IP_ADAPTER_ADDRESSES_LH` 链表，提取名称、GUID、MAC、IP、网关、DHCP 服务器等信息，并根据 `classify_adapter_status` 的结果填充 `adapters`, `details`, `disabled` 三个列表。
    - **关键逻辑行 (L180-L195)**: 计算 `admin_down` (通过 `is_admin_status_down`)、`registry_disabled` (通过 `is_admin_disabled_via_registry`) 和 `pnp_present` (通过 `read_pnp_instance_id` + `devnode_problem`)，然后调用 `classify_adapter_status`。
    - **关键逻辑行 (L200-L225)**: 根据 `status` 将适配器推入不同列表：`Connected`/`EnabledNoIp` -> `details`; `Disabled` -> `disabled`; 所有适配器均加入 `adapters`。
- **`classify_adapter_status(is_up, has_ip, oper_status, admin_down, registry_disabled, pnp_present)`**: 纯函数，四分类决策核心。
    - **判定优先级**:
        1. `is_up`: 有 IP -> `Connected`; 无 IP -> `EnabledNoIp`。
        2. `pnp_present == Some(false)`: 幽灵设备 -> `Disconnected` (修复六期)。
        3. `admin_down == Some(true)`: 管理性禁用 -> `Disabled` (修复一期)。
        4. `oper_status == NotPresent && registry_disabled`: 旧行为回退 -> `Disabled`。
        5. 默认 -> `Disconnected`。
- **`is_admin_status_down(if_index)`**: 调用 `GetIfEntry2` 查询 `MIB_IF_ROW2.AdminStatus` 是否为 `NET_IF_ADMIN_STATUS_DOWN`。失败返回 `None`。
- **`read_pwstr(ptr)`**: 安全读取 Windows PWSTR 字符串。
- **`ipv4_from_in_addr(addr)`**: 将 `IN_ADDR` 转换为 IPv4 字符串。
- **测试模块 `tests`**: 包含 11 个单元测试用例，覆盖 Up/Down/NotPresent 与 AdminStatus/PnP/Registry 的各种组合。

### `monitor/adapter_watch.rs` (适配器监控与自动启用)

- **`start_adapter_watch(app_handle)`**: 启动后台监控任务。
    - **间隔**: 15s (`ADAPTER_WATCH_INTERVAL`)。
    - **缓存刷新**: 每 4 轮 (60s) 刷新一次 CLASS_SUBKEY 注册表缓存 (`refresh_class_subkey_cache`)，确保禁用分类检测的准确性。
    - **变更检测**: 对比当前与上次的 `adapters` 和 `disabled` 列表（按 name 排序），检测名称/IP/状态变化。
- **`auto_enable_backoff_ms(failure_count)`**: 自动启用失败退避策略：0次->0ms, 1次->60s, 2次->120s, ≥3次->300s。
- **关键业务逻辑 (L150-L250)**:
    - **禁用警告**: 当手选适配器被禁用时，若不在夜间出站切换守护名单内，发送 Toast 警告。
    - **自动启用**: 当手选适配器被禁用时，尝试静默启用（首次）或弹 UAC 启用（后续失败后）。受夜间出站切换守护窗口 (`guard_window`) 保护，避免在切换期间误启用。
    - **恢复触发**: 适配器从禁用恢复时，若后台巡检已停止，触发一次性后台检查。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                         adapter_watch (15s loop)                            │
│  ┌──────────────┐   ┌──────────────────┐   ┌────────────────────────────┐  │
│  │ get_all_     │──►│ classify_        │──►│ emit_adapters_changed      │  │
│ │ adapters_    │   │ adapter_status   │   │ emit_adapter_details       │  │
│ │ cached       │   │ (windows.rs)     │   │ emit_disabled_adapters     │  │
│  └──────────────┘   └──────────────────┘   └────────────────────────────┘  │
│         ▲                   ▲                       │                      │
│         │                   │                       ▼                      │
│  ┌──────────────┐   ┌──────────────────┐   ┌────────────────────────────┐  │
│  │ refresh_     │──►│ is_admin_        │   │ auto_enable_attempt        │  │
│ │ class_subkey │   │ status_down      │   │ (spawn_blocking)             │  │
│ │ cache (60s)  │   │ (GetIfEntry2)    │   └────────────────────────────┘  │
│  └──────────────┘   └──────────────────┘           │                      │
│                                                    ▼                      │
│                                           ┌────────────────────────────┐  │
│                                           │ spawn_background_check_once│  │
│                                           └────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **发现阶段**: `get_all_adapters_cached` 调用 `query_adapters_addresses`，后者通过 `GetAdaptersAddresses` 获取原始适配器数据。
2. **分类阶段**: `parse_adapter_addresses` 遍历每个适配器，调用 `is_admin_status_down` 和 `is_admin_disabled_via_registry` 获取辅助状态，再调用 `classify_adapter_status` 得出最终四分类状态。
3. **监控阶段**: `adapter_watch` 每 15s 调用 `get_all_adapters_cached`，对比前后状态。
4. **响应阶段**:
    - **变更**: 通过 `EventBus` 推送给前端。
    - **禁用**: 若为手选适配器且非夜间守护期，尝试 `enable_adapter`；失败则记录退避时间。
    - **恢复**: 触发一次性后台检查以恢复网络连接。

## Known Issues

- **历史缺陷一 (已修复)**: 旧实现只认 `OperStatus == NotPresent` 且 `ConfigFlags` 含 `DISABLED` 位来判定禁用。微软文档指出禁用后 `OperStatus` 可能是 `Down` 或 `NotPresent`，导致大量禁用网卡被误判为“未连接”，自动启用不触发。
- **历史缺陷二 (已修复)**: 拔出 USB 网卡后，接口行 `AdminStatus` 残留 `Down`，注册表 `ConfigFlags` 残留 `DISABLED` 位，旧判定将“未连接”误报为“已禁用”。修复方案引入 PnP 设备树检查 (`read_pnp_instance_id` + `devnode_problem`)，若设备不在活动状态，一律视为“未连接”。
- **缓存陈旧性**: `CLASS_SUBKEY_CACHE` 曾仅在首次访问构建，导致运行期禁用状态检测永久陈旧。现改为每 60s 刷新一次。
- **竞态条件**: 启动首轮监测前，前端可能已拉取快照。`adapter_watch` 首轮无条件推送一次，强制对齐状态。