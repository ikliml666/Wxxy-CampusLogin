---
title: 适配器可见性/禁用分类缓存陈旧（注册表缓存刷新策略）
type: learning
source_files:
  - tauri-app/src-tauri/src/network/discovery/registry.rs
  - tauri-app/src-tauri/src/monitor/adapter_watch.rs
tags: [教训, 缓存, 适配器, 时效性, 历史缺陷]
---

## Overview

本模块负责在 Windows 平台上准确识别网络适配器的**可见性**（是否在 `ncpa.cpl` / Win11 高级网络设置中可见）和**禁用状态分类**（区分"管理员禁用"与"硬件缺失/NotPresent"）。

核心痛点在于：底层依赖 Windows 注册表查询，而注册表状态（如设备管理器中的禁用操作、物理拔插）在运行期会发生变化。旧的实现存在严重的**缓存陈旧**问题——`CLASS_SUBKEY_CACHE` 仅在首次访问时构建，且仅在 `enable_adapter`（启用网卡）操作路径中刷新。这导致用户在系统侧进行的禁用/拔插操作无法被软件感知，造成可见性与禁用分类永久陈旧。

修复方案引入了基于时间周期的自动刷新机制（`adapter_watch.rs` 每 60 秒通过 `spawn_blocking` 调用 `refresh_class_subkey_cache`），将注册表缓存的刷新频率与适配器轮询周期解耦，确保检测灵敏度。同时 `ShowInNetworkConnections` 查询也引入了 5 秒短 TTL 缓存并支持显式失效。

## Key Components

### `tauri-app/src-tauri/src/network/discovery/registry.rs`

该文件专注注册表查询逻辑，从 `discovery::windows` 拆分而出（仅 Windows 平台编译，依赖 `winreg` crate）。

#### 常量与配置

- `SHOW_IN_NCPA_CACHE_TTL_SECS: u64` (行 85-86)：值为 `5`。`ShowInNetworkConnections` 查询缓存的 TTL（秒），与适配器缓存 TTL（5s）对齐：值极少变化且变更路径有显式失效。

#### 数据结构

- `ClassSubkeyEntry` (行 140-143)
  - `exists: bool` (行 141)：记录 `NetCfgInstanceId` 是否存在于 Class subkey 中。
  - `config_flags: Option<u32>` (行 142)：记录 `ConfigFlags` 值，用于判断禁用标志。

#### 全局缓存状态

- `SHOW_IN_NCPA_CACHE` (行 81-83)：类型 `RwLock<Option<(HashMap<String, bool>, Instant)>>`。缓存 `guid -> 是否可见`。`None` 表示未初始化或已失效。
- `CLASS_SUBKEY_CACHE` (行 145-148)：类型 `RwLock<Option<HashMap<String, ClassSubkeyEntry>>>`。缓存 `guid（小写） -> ClassSubkeyEntry`。`None` 表示未初始化。

#### 公开函数

- `is_visible_in_ncpa(guid: &str) -> bool` (行 54-77)：判断网卡是否在 Win11 高级网络设置 / ncpa.cpl 中可见。空 GUID 回退为 `true`（行 57-59）；先检查 `show_in_ncpa_cached`（行 72），再交叉验证 `class_subkey_has_matching_guid`（行 76）以排除幽灵虚拟副本。
- `invalidate_show_in_ncpa_cache()` (行 135-137)：失效 `ShowInNetworkConnections` 查询缓存。适配器启用/禁用等可见性变更路径必须调用，避免变更后短窗内返回陈旧结果。
- `refresh_class_subkey_cache()` (行 174-177)：遍历注册表 Class 子键构建新缓存并替换 `CLASS_SUBKEY_CACHE`，保证注册表变化后缓存更新。
- `class_subkey_has_matching_guid(guid: &str) -> bool` (行 192-202)：空 GUID 返回 `false`（行 193-195）；调用 `ensure_cache_initialized`（行 196）后读 `CLASS_SUBKEY_CACHE` 判断 `exists` 字段（行 200）。
- `is_admin_disabled_via_registry(guid: &str) -> bool` (行 205-216)：区分 NotPresent 状态下的"管理员禁用" vs "硬件缺失(USB未连接)"。空 GUID 返回 `false`（行 206-208）；读缓存中 `config_flags`（行 213）判断 `f & 0x1 != 0`（行 214, `CONFIGFLAG_DISABLED`）。

#### 内部函数

- `query_show_in_ncpa(guid: &str) -> bool` (行 89-102)：原注册表 1 检查，查询 `SYSTEM\CurrentControlSet\Control\Network\{...}\{GUID}\Connection` 下的 `ShowInNetworkConnections` 值。
- `show_in_ncpa_cached(guid: &str) -> bool` (行 105-131)：带短 TTL 缓存的注册表 1 查询。读多写少，读锁快速路径；未命中时查注册表并补入（行 114-120）。
- `build_class_subkey_cache() -> HashMap<String, ClassSubkeyEntry>` (行 151-171)：遍历注册表 `HKLM\SYSTEM\CurrentControlSet\Control\Class\{...}` 子键，构建 `guid -> entry` 映射，执行同步注册表 I/O。
- `ensure_cache_initialized()` (行 181-190)：双重检查锁定，确保 `CLASS_SUBKEY_CACHE` 已初始化。`build` 在锁外执行（行 185），仅用锁做 swap（行 187-189）。

#### 测试模块 (`tests`, 行 15-50)

- `class_subkey_check_empty_guid_returns_false` (行 18-21)：断言空 GUID 返回 `false`。
- `class_subkey_check_nonexistent_guid_returns_false` (行 23-26)：断言不存在的 GUID 返回 `false`。
- `class_subkey_check_real_wlan_guid_returns_true` (行 28-35)：真实 WLAN GUID 检查，无环境则 skip。
- `class_subkey_check_ghost_wlan_guids_return_false` (行 37-49)：断言幽灵虚拟副本 GUID 返回 `false`。

### `tauri-app/src-tauri/src/monitor/adapter_watch.rs`

负责适配器状态的后台监控与变更分发。

#### 常量与配置

- `ADAPTER_WATCH_INTERVAL: u64` (行 9)：值为 `15000`。适配器轮询间隔（毫秒）。
- `CLASS_SUBKEY_REFRESH_ROUNDS: u32` (行 15)：值为 `4`。Class subkey 缓存刷新周期倍数（`15s × 4 = 60s`）。选择放宽周期而非"仅适配器集合变化时刷新"，因为禁用分类检测依赖缓存新鲜度。

#### 公开函数

- `start_adapter_watch(app_handle: &AppHandle) -> Result<(), String>` (行 17-288)：启动适配器监控任务。包含完整的轮询循环（行 34-285）、变更检测（行 71-89）、事件分发（行 91-164）、自动启用逻辑（行 166-272）及夜间出站切换守护窗处理（行 184-195）。

#### 内部函数

- `auto_enable_backoff_ms(failure_count: u32) -> u64` (行 292-299)：自动启用失败退避计算。纯函数。0次→0ms；1次→60_000ms；2次→120_000ms；≥3次→300_000ms 封顶。

#### 测试模块 (`tests`, 行 301-314)

- `auto_enable_backoff_ladder` (行 305-313)：验证退避阶梯逻辑，包括 `u32::MAX` 封顶。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│                        adapter_watch.rs                             │
│  start_adapter_watch (Loop every 15s, 行 17-288)                   │
│  ├── get_all_adapters_cached (max 4s stale)                       │
│  ├── [Every 4th round (60s), 行 56-62]                            │
│  │   spawn_blocking ──► registry.rs: refresh_class_subkey_cache   │
│  │   (行 174-177) ──► build_class_subkey_cache (行 151-171)       │
│  │   └──► Updates CLASS_SUBKEY_CACHE (行 145-148)                 │
│  ├── Detect Changes ──► EventBus.emit_* / spawn_background_check   │
│  └── Auto-Enable Logic (行 166-272) ──► enable_adapter             │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                   registry.rs (Visibility/Disable)                  │
│  is_visible_in_ncpa (行 54-77)                                     │
│  ├── show_in_ncpa_cached (行 105-131) ── SHOW_IN_NCPA_CACHE (行 81) │
│  └── class_subkey_has_matching_guid (行 192-202) ─ CLASS_SUBKEY_CACHE│
│  is_admin_disabled_via_registry (行 205-216) ── CLASS_SUBKEY_CACHE  │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **初始化**: `CLASS_SUBKEY_CACHE` (行 145-148) 初始为 `None`。首次调用 `class_subkey_has_matching_guid` (行 192) 或 `is_admin_disabled_via_registry` (行 205) 时触发 `ensure_cache_initialized` (行 181)，执行全量注册表遍历构建缓存。
2. **周期性刷新**: `start_adapter_watch` (行 17) 每 15 秒（`ADAPTER_WATCH_INTERVAL` 行 9）轮询一次（行 28, 36）。每 4 轮（`CLASS_SUBKEY_REFRESH_ROUNDS` 行 15，即 60 秒），通过 `spawn_blocking` (行 59-61) 调用 `refresh_class_subkey_cache` (行 174)，重新遍历注册表 Class 子键并替换缓存 (行 176)。这是解决缓存陈旧的关键步骤。
3. **可见性检查**: `is_visible_in_ncpa` (行 54) 依次检查 `show_in_ncpa_cached` (行 105，TTL 5s 行 85-86) 与 `class_subkey_has_matching_guid` (行 192) 中是否存在对应 GUID（防幽灵虚拟副本）。
4. **禁用分类**: `is_admin_disabled_via_registry` (行 205) 读取 `CLASS_SUBKEY_CACHE` 中的 `config_flags` (行 213)，判断是否为管理员禁用 (行 214)。
5. **变更响应**: 如果检测到适配器或禁用列表变化（行 91, 102），通过 `EventBus` 通知前端，并可能触发自动启用（行 237-269）或背景检查（行 115, 254）。

## Known Issues

- **缓存陈旧历史缺陷**: `CLASS_SUBKEY_CACHE` 曾仅在首次访问构建、只在 `enable_adapter` 刷新（`adapter_watch.rs` 行 49-52 注释说明）。导致运行期设备管理器禁用/拔插操作无法被感知。现已通过 60s 周期刷新解决（`adapter_watch.rs` 行 56-62）。
- **注册表 I/O 阻塞**: `build_class_subkey_cache` (行 151) 是同步 I/O 操作，必须包在 `spawn_blocking` 中执行（`adapter_watch.rs` 行 59-61），避免阻塞 async 任务线程。
- **可见性误判**: 空 GUID 的网卡在旧逻辑中被静默隐藏（`registry.rs` 行 55 注释），现回退为"可见"（`registry.rs` 行 57-59），交由后续黑名单/IP 过滤兜底。

## Connections

- [[adapter-linkspeed-u64-max]]
- [[adapter-operation-scope]]
