---
title: DisabledAdapter.status 枚举序列化与适配器状态轮询机制
type: learning
source_files:
  - tauri-app/src-tauri/src/network/discovery/mod.rs
  - tauri-app/src-tauri/src/network/discovery/windows.rs
  - tauri-app/src-tauri/src/monitor/adapter_watch.rs
  - tauri-app/frontend/src/network/types.ts
  - android/frontend/src/network/types.ts
tags: [教训, i18n, 枚举序列化, 启动竞态, 事件推送, Radix Select, 跨端契约]
---

## Overview

本文档记录了 `DisabledAdapter` 状态字段从中文字符串改为枚举序列化的修复过程，以及 `adapter_watch` 模块中解决启动竞态、差分推送漏发和自动启用逻辑的技术细节。核心问题在于后端 `DisabledAdapter.status` 曾存储中文文案导致前端 i18n 键值断裂，同时 `adapter_watch` 的差分推送机制在应用启动初期因基线为空导致禁用列表快照永久冻结。

## Key Components

### `tauri-app/src-tauri/src/network/discovery/mod.rs`

- **`AdapterStatus` 枚举** (行 37-42): 四分类状态：`Disabled`（已禁用）、`Disconnected`（未连接）、`EnabledNoIp`（未禁用无IP）、`Connected`（已连接）。使用 `#[serde(rename_all = "camelCase")]` 确保序列化后为英文 key（如 `'disabled'`），供前端 i18n 映射。
- **`Adapter` struct** (行 44-56): 包含 `name`, `ip`, `wireless`, `guid`, `mac`, `if_index`, `status`, `link_speed`。所有适配器（含禁用）均推入此列表。
- **`AdapterDetail` struct** (行 58-70): 仅包含 `Connected` 和 `EnabledNoIp` 状态的适配器详情。
- **`DisabledAdapter` struct** (行 72-79): 仅包含 `Disabled` 状态的适配器。**关键变更**：`status` 字段类型从 `String` 改为 `AdapterStatus`，确保与 `Adapter` 同源序列化。
- **`BL_REGEX`** (行 23-26): 名称/描述黑名单正则，用于过滤虚拟网卡、VPN 等。
- **`is_blacklisted(name: &str)`** (行 96-98): 检查适配器名称是否命中黑名单。
- **`query_adapters_addresses()`** (行 109-117): 平台分发函数，Windows 调用 `windows::query_adapters_addresses`，其他平台返回空。

### `tauri-app/src-tauri/src/network/discovery/windows.rs`

- **`query_adapters_addresses()`** (行 22-66): 调用 Win32 `GetAdaptersAddresses` 获取适配器列表。处理缓冲区扩容重试逻辑。
- **`parse_adapter_addresses()`** (行 68-196): 解析 C API 返回的数据。
    - **适配流程**：遍历 `IP_ADAPTER_ADDRESSES_LH` 链表，过滤非以太网/WLAN 类型、物理地址长度为 0 的项。
    - **可见性检查**：调用 `super::registry::is_visible_in_ncpa` 检查注册表可见性。
    - **黑名单检查**：调用 `super::is_blacklisted` 检查名称和描述。
    - **状态计算**：调用 `classify_adapter_status` 计算 `AdapterStatus`。
    - **列表分配**：
        - 所有通过过滤的适配器均加入 `adapters` 列表（行 178-186）。
        - `Connected` 和 `EnabledNoIp` 加入 `details`（行 188-206）。
        - `Disabled` 加入 `disabled` 列表（行 208-212）。
- **`classify_adapter_status()`** (行 233-256): 纯函数，根据 `is_up`, `has_ip`, `oper_status`, `admin_down`, `registry_disabled`, `pnp_present` 判定状态。
    - **幽灵设备修复**：若 `pnp_present == Some(false)`（PnP 设备树无活动节点），强制返回 `Disconnected`，避免将拔出 USB 网卡的残影误判为 `Disabled`。
    - **AdminStatus 优先**：若 `admin_down == Some(true)`，直接返回 `Disabled`，解决旧实现中 `OperStatus=Down` 但非 `NotPresent` 时被误判为 `Disconnected` 的问题。
- **`auto_enable_backoff_ms()`** (行 265-272): 自动启用失败退避算法：0→0ms, 1→60s, 2→120s, ≥3→300s。

### `tauri-app/src-tauri/src/monitor/adapter_watch.rs`

- **`start_adapter_watch()`** (行 15-266): 启动适配器监控后台任务。
    - **首轮强制推送** (行 54-56): 引入 `first_round` 标志，首轮查询结果无条件触发 `adapters_changed` 和 `disabled_changed` 事件，解决启动竞态导致的快照冻结。
    - **注册表缓存刷新** (行 73-81): 每 `CLASS_SUBKEY_REFRESH_ROUNDS` (4轮，即 60s) 刷新一次 `CLASS_SUBKEY_CACHE`，确保禁用状态检测新鲜度。
    - **差分推送逻辑** (行 88-116): 对比当前与上一轮 (`last_adapters`, `last_disabled`) 的名称/IP/状态变化。排序后比较以避免 `GetAdaptersAddresses` 返回顺序不稳定导致的误报。
    - **禁用恢复触发重检** (行 123-135): 当检测到适配器从禁用恢复时，触发一次性后台检查，而非重启用户停止的巡检。
    - **禁用警告通知** (行 136-175): 每分钟限制发送一次禁用警告。排除夜间出站切换期间本功能亲手禁用的卡。
    - **自动启用逻辑** (行 180-245):
        - **夜间守护窗口** (行 193-203): 在夜间出站切换守护窗口内，跳过手选适配器的自动启用，防止顶掉切换。
        - **退避策略** (行 218-225): 基于 `auto_enable_failure_count` 计算退避时间。
        - **提权降级** (行 227-230): 首次尝试静默提权，失败后允许弹 UAC。
        - **成功处理** (行 232-242): 成功后清零失败计数，并触发一次性后台检查。

### `tauri-app/frontend/src/network/types.ts` & `android/frontend/src/network/types.ts`

- **`AdapterStatus`** (行 1): 联合类型 `'disabled' | 'disconnected' | 'enabledNoIp' | 'connected'`。
- **`Adapter`** (行 3-13): 对应后端 `Adapter` struct。
- **`DisabledAdapter`** (行 15-19): 对应后端 `DisabledAdapter` struct，`status` 字段类型为 `AdapterStatus` 而非 `string`。
- **`AdapterDetail`** (行 21-32): 对应后端 `AdapterDetail` struct。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                     Frontend (React)                        │
│  ┌──────────────┐   ┌──────────────┐   ┌────────────────┐  │
│  │ NetworkPanel │──▶│ SelectItems  │   │ Store (adapters│  │
│  │              │   │ (value=unique)│   │  disabledAdapters│  │
│  └──────────────┘   └──────────────┘   └────────────────┘  │
│         ▲                    ▲                   ▲          │
│         │                    │                   │          │
│         │       ┌────────────▼────────────┐      │          │
│         │       │     EventBus            │      │          │
│         │       │ onAdaptersChanged       │      │          │
│         │       │ onDisabledAdaptersChgd  │      │          │
│         │       └────────────┬────────────┘      │          │
└─────────┼────────────────────┼───────────────────┼──────────┘
          │                    │                   │
          │                    ▼                   │
┌─────────┼────────────────────────────────────────┼──────────┐
│         │           Rust Backend (Tauri)         │          │
│         │                                        │          │
│         │  ┌─────────────────────────────────┐   │          │
│         │  │ adapter_watch (monitor)         │◀┼──┐        │
│         │  │ - first_round push              │   │        │
│         │  │ - diff detection                │   │        │
│         │  │ - auto-enable (backoff/UAC)     │   │        │
│         │  └──────────────┬──────────────────┘   │        │
│         │                 │                      │        │
│         │  ┌──────────────▼──────────────────┐   │        │
│         │  │ discovery (network)             │   │        │
│         │  │ - query_adapters_addresses()    │   │        │
│         │  │ - classify_adapter_status()     │   │        │
│         │  │ - AdapterStatus enum serialization│   │        │
│         │  └──────────────┬──────────────────┘   │        │
│         │                 │                      │        │
│         │  ┌──────────────▼──────────────────┐   │        │
│         │  │ Windows API / Registry          │   │        │
│         │  │ GetAdaptersAddresses            │   │        │
│         │  │ is_admin_disabled_via_registry  │   │        │
│         │  └─────────────────────────────────┘   │        │
│         └────────────────────────────────────────┘        │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **发现阶段**: `adapter_watch` 定时调用 `get_all_adapters_cached` → `windows::query_adapters_addresses` → `parse_adapter_addresses`。
2. **状态判定**: `parse_adapter_addresses` 对每个适配器调用 `classify_adapter_status`，结合 `OperStatus`、`AdminStatus`、注册表 `ConfigFlags` 和 PnP 设备树状态，得出 `AdapterStatus` 枚举。
3. **分类存储**: 结果分为 `adapters` (全部)、`details` (Connected/EnabledNoIp)、`disabled` (Disabled) 三个向量。
4. **差分推送**: `adapter_watch` 对比本轮与上轮数据。若 `first_round` 或数据变化，通过 `EventBus` 发出 `onAdaptersChanged` 和 `onDisabledAdaptersChanged` 事件。
5. **前端更新**: 前端 Store 接收事件，更新 `adapters` 和 `disabledAdapters`。下拉菜单从 `adapters` 派生禁用项，确保 value 唯一。
6. **自动启用**: 若检测到配置的手选适配器进入 `disabled` 状态，且不在夜间守护窗口，`adapter_watch` 尝试调用 `enable_adapter` 恢复。失败则记录计数并退避。

## Connections

- [[NetworkPanel]]: 前端网络面板组件，消费适配器数据。
- [[adapter_cache]]: 负责适配器缓存刷新。
- [[dhcp]]: 负责 DHCP 租约管理。
- [[subnet]]: 负责子网掩码计算。

## Known Issues

- **历史缺陷已修复**: `DisabledAdapter.status` 曾存中文文案导致 i18n 断裂，现已改为枚举序列化。
- **历史缺陷已修复**: `adapter_watch` 启动竞态导致禁用列表冻结，现已通过首轮强制推送解决。
- **历史缺陷已修复**: Radix Select value 撞车导致双勾选，现已改为单源派生。
- **幽灵设备误判**: 拔出 USB 网卡后，若 PnP 设备树查询失败（`None`），会回退到旧行为（可能误判为 `Disabled`）。代码注释指出这是已知回退路径，但在大多数现代 Windows 系统上 PnP 查询应能正常工作。