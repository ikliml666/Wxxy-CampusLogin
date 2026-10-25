---
title: Windows 适配器发现与监控——四分类决策、幽灵设备修复与自动启用守护
type: learning
source_files:
  - tauri-app/src-tauri/src/network/discovery/windows.rs
  - tauri-app/src-tauri/src/monitor/adapter_watch.rs
tags: [教训, 适配器, 禁用分类, 自动启用, API语义, 历史缺陷]
---

## Overview

本模块负责 Windows 平台下的网络适配器状态发现（Discovery）与运行时监控（Monitoring）。核心职责包括：通过 Win32 `GetAdaptersAddresses` 获取适配器列表（仅 Windows 编译，见 windows.rs 第 4 行注释），结合注册表可见性（`is_visible_in_ncpa`、`is_admin_disabled_via_registry`）、`GetIfEntry2` 的 `AdminStatus` 以及 PnP 设备树状态，将适配器严格划分为四类状态（`Connected`、`EnabledNoIp`、`Disabled`、`Disconnected`）。监控端 (`adapter_watch.rs`) 每 15 秒轮询一次（第 9 行 `ADAPTER_WATCH_INTERVAL`），检测适配器变更并触发 UI 更新、禁用警告、后台检查或自动启用尝试。

**核心教训：** 旧实现仅依赖 `OperStatus == NotPresent` 且注册表 `ConfigFlags` 含 `DISABLED` 位来判定“已禁用”，导致大量“禁用后 OperStatus 报 Down”的网卡被误判为“未连接”，使得自动启用功能静默失效。修复方案引入了文档化的 `AdminStatus` 字段作为主判定依据（windows.rs 第 205 行、第 317-328 行），并增加了 PnP 设备树存活检查以修复“拔出 USB 网卡后残影误判为已禁用”的问题（windows.rs 第 212-217 行）。

## Key Components

### `tauri-app/src-tauri/src/network/discovery/windows.rs`

- **`query_adapters_addresses()`** (第 12-63 行, `pub fn`): 调用 Win32 `GetAdaptersAddresses`（`AF_INET`，标志 `GAA_FLAGS` 在第 16 行定义为 `0x0080 | 0x0100`）获取原始数据。包含重试逻辑（最多 3 次，第 29 行 `max_retries`）以处理缓冲区不足（`result == 111` 或 `actual_size > buffer_size`，第 44 行）。成功则返回 `parse_adapter_addresses` 结果（第 41 行）。
- **`parse_adapter_addresses(ptr, if_type_ethernet, if_type_wireless)`** (第 65-284 行, `fn` 私有): 解析 `IP_ADAPTER_ADDRESSES_LH` 链表。提取名称（第 81 行 `read_pwstr`）、GUID（第 83-98 行）、类型过滤（仅以太网 6 / 无线 71，第 100-104 行）、物理地址长度过滤（第 106-109 行）、NCPA 可见性过滤（第 114-117 行 `is_visible_in_ncpa`）、黑名单过滤（第 121-124 行 `super::is_blacklisted`）。提取 MAC（第 136-141 行）、连接速度（第 131-134 行，`ReceiveLinkSpeed` 为 `u64::MAX` 归 0）、IP/前缀/网关/DHCP（仅 `is_up` 时，第 147-198 行）。计算辅助状态：
  - `admin_down` (第 205 行): 非 Up 时调用 `is_admin_status_down(if_index)`。
  - `registry_disabled` (第 206 行): 非 Up 时调用 `is_admin_disabled_via_registry(&guid)`。
  - `pnp_present` (第 212-217 行): 非 Up 时通过 `super::devnode::read_pnp_instance_id` 与 `devnode_problem` 判定设备树存活。
  随后调用 `classify_adapter_status` (第 218-225 行) 得到 `status`。所有适配器推入 `adapters` (第 228-237 行)；`Connected`/`EnabledNoIp` 推入 `details` (第 242-269 行)；`Disabled` 推入 `disabled` (第 270-276 行)。
- **`read_pwstr(ptr)`** (第 286-307 行, `unsafe fn` 私有): 安全读取 Windows `PWSTR` 字符串，超 4096 UTF-16 单元时记日志并返回空串（第 293-299 行）。
- **`ipv4_from_in_addr(addr)`** (第 309-311 行, `unsafe fn` 私有): 将 `IN_ADDR` 转换为 IPv4 字符串。
- **`is_admin_status_down(if_index)`** (第 317-328 行, `fn` 私有): 调用 `GetIfEntry2` 查询 `MIB_IF_ROW2.AdminStatus` 是否为 `NET_IF_ADMIN_STATUS_DOWN`（第 319、327 行）。失败或接口不存在返回 `None`（第 324-326 行）。
- **`classify_adapter_status(is_up, has_ip, oper_status, admin_down, registry_disabled, pnp_present)`** (第 345-367 行, `fn` 纯函数): 四分类决策核心。
  - 判定优先级：
    1. `is_up` (第 353-355 行): 有 IP → `Connected`；无 IP → `EnabledNoIp`。
    2. `pnp_present == Some(false)` (第 357-359 行): 幽灵设备 → `Disconnected`（六期修复）。
    3. `admin_down == Some(true)` (第 360-362 行): 管理性禁用 → `Disabled`（一期修复）。
    4. `oper_status == IfOperStatusNotPresent && registry_disabled` (第 363-365 行): 旧行为回退 → `Disabled`。
    5. 默认 (第 366 行) → `Disconnected`。
- **`tests` 模块** (第 369-479 行, `#[cfg(test)]`): 包含 11 个单元测试，覆盖 `classify_adapter_status` 各种组合（Up/Down/NotPresent × Admin/PnP/Registry），如 `down_with_admin_down_is_disabled` (第 397-402 行)、`phantom_device_with_admin_down_is_disconnected` (第 452-457 行) 等。

### `tauri-app/src-tauri/src/monitor/adapter_watch.rs`

- **`ADAPTER_WATCH_INTERVAL`** (第 9 行, `const u64 = 15000`): 监控轮询间隔毫秒。
- **`CLASS_SUBKEY_REFRESH_ROUNDS`** (第 15 行, `const u32 = 4`): 缓存刷新轮数（15s×4=60s）。
- **`start_adapter_watch(app_handle)`** (第 17-288 行, `pub fn`): 启动后台监控任务（使用 `task_manager.spawn`，第 19 行）。内部 `tokio::time::interval` 每 15s tick（第 28-32 行，`MissedTickBehavior::Delay`）。逻辑：
  - 每 4 轮调用 `refresh_class_subkey_cache` (第 56-62 行，在 `spawn_blocking` 中)。
  - 调用 `get_all_adapters_cached` (第 68 行，`spawn_blocking`)。
  - 比较 `adapters` 与 `last_adapters` 按 name 排序后名称/IP 变化 (第 71-78 行)；比较 `disabled` 类似 (第 82-89 行)。首轮强制推送 (第 26、78、89 行 `first_round`)。
  - 变更时发事件 `emit_adapters_changed` / `emit_adapter_details_changed` (第 92-99 行)。
  - `disabled_changed` 时检测恢复 (第 103-117 行)，触发 `spawn_background_check_once` (第 115 行)；发送禁用警告 toast (第 118-163 行)，过滤夜间切换名单 (第 146-149、150-162 行)。
  - 自动启用逻辑 (第 166-272 行)：计算夜间守护窗 `guard_window` (第 184-195 行)，获取候选 `configured_disabled_adapters` (第 196 行)，过滤夜切名单与守护窗得到 `targets` (第 211-215 行)，按退避尝试 `enable_adapter` (第 237-269 行，在 `spawn_blocking` 中)。成功清零失败计数 (第 247 行)，失败递增退避 (第 259-260 行)。
- **`auto_enable_backoff_ms(failure_count)`** (第 292-299 行, `fn` 私有纯函数): 退避策略：0→0ms, 1→60_000, 2→120_000, ≥3→300_000。
- **`tests` 模块** (第 301-314 行): 测试 `auto_enable_backoff_ladder` 验证退避阶梯（第 305-313 行）。

## Architecture

```text
┌──────────────────────────────────────────────────────────────────────┐
│                   adapter_watch (15s loop, L17-288)                   │
│  ┌────────────────────┐   ┌────────────────────────┐                 │
│  │ get_all_adapters_  │──►│ parse_adapter_addresses │                 │
│  │ cached (L68)       │   │ (windows.rs L65-284)    │                 │
│  └────────────────────┘   └────────────────────────┘                 │
│         ▲                   │ calls                  │                │
│         │                   ▼                        ▼                │
│  ┌────────────────────┐   ┌────────────────────────┐ ┌──────────────┐│
│  │ refresh_class_     │──►│ is_admin_status_down   │ │ classify_    ││
│  │ subkey_cache (L59) │   │ (L317-328)             │ │ adapter_     ││
│  └────────────────────┘   └────────────────────────┘ │ status L345  ││
│                                                     └──────────────┘│
│         │                                  │                       │
│         ▼                                  ▼                       ▼
│  ┌────────────────────┐   ┌────────────────────────┐ ┌──────────────────┐
│  │ auto_enable_attempt│──►│ enable_adapter         │ │ emit_* events    │
│  │ (L237-269)         │   │ (adapter_cache)        │ │ (L92-99,158)     │
│  └────────────────────┘   └────────────────────────┘ └──────────────────┘
│         │                                  │
│         ▼                                  ▼
│  ┌────────────────────┐   ┌────────────────────────┐
│  │ spawn_background_  │   │ refresh_class_subkey_  │
│  │ check_once (L115)  │   │ cache (registry L60)   │
│  └────────────────────┘   └────────────────────────┘
└──────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **发现阶段**: `get_all_adapters_cached` 最终调用 `query_adapters_addresses` (windows.rs 第 12 行)，通过 `GetAdaptersAddresses` 获取原始适配器数据，经 `parse_adapter_addresses` (第 65 行) 处理。
2. **分类阶段**: `parse_adapter_addresses` 遍历每个适配器，调用 `is_admin_status_down` (第 205 行、第 317 行) 和 `is_admin_disabled_via_registry` (第 206 行) 及 PnP 检查 (第 212-217 行)，再调用 `classify_adapter_status` (第 218 行、第 345 行) 得出最终四分类状态，填充 `adapters`/`details`/`disabled` 三列表 (第 228-278 行)。
3. **监控阶段**: `adapter_watch` 每 15s (第 9 行、第 28 行) 调用 `get_all_adapters_cached` (第 68 行)，对比前后状态（按 name 排序，第 71-89 行）。
4. **响应阶段**:
   - **变更**: 通过 `EventBus` 推送前端 (第 92-99 行)。
   - **禁用**: 若为手选适配器且非夜间守护期 (第 146-215 行)，尝试 `enable_adapter` (第 243 行)；失败则记录退避时间 (第 228-270 行)。
   - **恢复**: 适配器从禁用恢复时触发 `spawn_background_check_once` (第 115 行、第 254 行)。

## Connections

- 依赖注册表可见性与禁用标志：[[registry-module]]（windows.rs 第 9 行引用 `super::registry`）。
- 启用适配器调用：[[adapter-cache]]（`enable_adapter` 在 adapter_watch.rs 第 243 行）。
- 一次性后台检查：[[background-task]]（`spawn_background_check_once` 第 115 行、第 254 行）。
- 夜间出站切换守护：[[outbound-switch]]（第 146-149 行、第 184-195 行、第 207-210 行）。
- 事件总线：[[event-bus]]（`EventBus::new` 第 92 行、第 96 行、第 158 行）。
- 平台分发入口：[[discovery-mod]]（windows.rs 第 4 行提及非 Windows 返回空列表）。

## Known Issues

- **历史缺陷一 (已修复)**: 旧实现只认 `OperStatus == NotPresent` 且 `ConfigFlags` 含 `DISABLED` 位来判定禁用（见 windows.rs 第 341-343 行注释）。现使用 `AdminStatus == Down` 作为主判定（第 360-362 行），兼容文档明文禁用后 OperStatus 可能为 Down 的情况。
- **历史缺陷二 (已修复)**: 拔出 USB 网卡后接口行与注册表残值误判“未连接”为“已禁用”（第 344 行、第 449-465 行）。现通过 PnP 设备树存活检查（`pnp_present == Some(false)` → `Disconnected`，第 357-359 行）修复。
- **缓存陈旧性 (已缓解)**: `CLASS_SUBKEY_CACHE` 曾仅首次构建（adapter_watch.rs 第 49-52 行注释），现每 60s 刷新（第 56-62 行）。
- **启动竞态 (已缓解)**: 前端初始加载可能早于首轮监测，首轮无条件推送对齐（第 23-26 行、第 78 行）。
- **PnP 查询不可用回退**: 当 `read_pnp_instance_id` 返回 `None`（极旧设备）时，`pnp_present` 为 `None`，回退旧行为（第 212-217 行、第 468-478 行测试）。
- **字符串长度限制**: `read_pwstr` 对超过 4096 UTF-16 单元的适配器名称跳过并返回空（第 293-299 行），可能导致该适配器信息缺失。
- **非 Windows 平台**: 本模块不编译，由 `discovery::mod` 平台分发返回空列表（windows.rs 第 4 行注释）。
