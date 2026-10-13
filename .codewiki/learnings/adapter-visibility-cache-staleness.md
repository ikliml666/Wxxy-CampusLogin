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

修复方案引入了基于时间周期的自动刷新机制，将注册表缓存的刷新频率与适配器轮询周期解耦，确保检测灵敏度。

## Key Components

### `tauri-app/src-tauri/src/network/discovery/registry.rs`

该文件专注注册表查询逻辑，从 `discovery::windows` 拆分而出。

#### 常量与配置

- `SHOW_IN_NCPA_CACHE_TTL_SECS`: `5` (第 78 行)
  - ShowInNetworkConnections 查询缓存的 TTL（秒），与适配器缓存 TTL 对齐。

#### 数据结构

- `ClassSubkeyEntry` (第 136–139 行)
  - `exists: bool`: 记录 NetCfgInstanceId 是否存在于 Class subkey 中。
  - `config_flags: Option<u32>`: 记录 ConfigFlags 值，用于判断禁用标志。

#### 全局缓存状态

- `SHOW_IN_NCPA_CACHE` (第 81–82 行): `RwLock<Option<(HashMap<String, bool>, Instant)>>`
  - 缓存 `guid -> 是否可见`。`None` 表示未初始化或已失效。
- `CLASS_SUBKEY_CACHE` (第 142–143 行): `RwLock<Option<HashMap<String, ClassSubkeyEntry>>>`
  - 缓存 `guid (小写) -> ClassSubkeyEntry`。`None` 表示未初始化。

#### 公开函数

- `is_visible_in_ncpa(guid: &str) -> bool` (第 56–80 行)
  - 判断网卡是否在 Win11 高级网络设置 / ncpa.cpl 中可见。
  - 逻辑：空 GUID 回退为 true；检查 `ShowInNetworkConnections` 注册表项；交叉验证 Class subkey 以排除幽灵虚拟副本。
- `invalidate_show_in_ncpa_cache()` (第 126–128 行)
  - 失效 ShowInNetworkConnections 查询缓存。在适配器启用/禁用等可见性变更路径调用。
- `refresh_class_subkey_cache()` (第 131–133 行)
  - 刷新 class subkey 缓存。在适配器缓存刷新时调用，保证注册表变化后缓存更新。
- `class_subkey_has_matching_guid(guid: &str) -> bool` (第 159–165 行)
  - 双重检查锁定：首次访问自动构建缓存，后续直接查询。判断 guid 是否在 Class subkey 中存在。
- `is_admin_disabled_via_registry(guid: &str) -> bool` (第 168–175 行)
  - 用于区分 NotPresent 状态下的"管理员禁用" vs "硬件缺失(USB未连接)"。读取 `ConfigFlags` 并检查 `0x1` (CONFIGFLAG_DISABLED)。

#### 内部函数

- `query_show_in_ncpa(guid: &str) -> bool` (第 85–100 行)
  - 原注册表 1 检查：查询 `SYSTEM\CurrentControlSet\Control\Network\{...}\{GUID}\Connection` 下的 `ShowInNetworkConnections` 值。
- `show_in_ncpa_cached(guid: &str) -> bool` (第 103–123 行)
  - 带短 TTL 缓存的注册表 1 查询。读多写少，读锁快速路径；未命中时查注册表并补入。
- `build_class_subkey_cache() -> HashMap<String, ClassSubkeyEntry>` (第 146–160 行)
  - 遍历注册表 Class 子键，构建 guid → entry 映射。执行同步注册表 I/O。
- `ensure_cache_initialized()` (第 168–175 行)
  - 双重检查锁定：确保 `CLASS_SUBKEY_CACHE` 已初始化。build 在锁外执行，仅用锁做 swap。

#### 测试模块 (`tests`)

- `class_subkey_check_empty_guid_returns_false` (第 17–19 行)
- `class_subkey_check_nonexistent_guid_returns_false` (第 21–23 行)
- `class_subkey_check_real_wlan_guid_returns_true` (第 25–31 行)
- `class_subkey_check_ghost_wlan_guids_return_false` (第 33–45 行)

### `tauri-app/src-tauri/src/monitor/adapter_watch.rs`

负责适配器状态的后台监控与变更分发。

#### 常量与配置

- `ADAPTER_WATCH_INTERVAL`: `15000` ms (第 9 行)
  - 适配器轮询间隔。
- `CLASS_SUBKEY_REFRESH_ROUNDS`: `4` (第 15 行)
  - Class subkey 缓存刷新周期倍数。`15s × 4 = 60s`。选择放宽周期而非"仅适配器集合变化时刷新"，因为禁用分类检测依赖缓存新鲜度，而设备管理器禁用未必改变适配器 name/ip。

#### 公开函数

- `start_adapter_watch(app_handle: &AppHandle) -> Result<(), String>` (第 17–260 行)
  - 启动适配器监控任务。
  - 包含完整的轮询循环、变更检测、事件分发、自动启用逻辑及夜间出站切换守护窗处理。

#### 内部函数

- `auto_enable_backoff_ms(failure_count: u32) -> u64` (第 262–270 行)
  - 自动启用失败退避计算。0次→0ms；1次→60s；2次→120s；≥3次→300s 封顶。

#### 测试模块 (`tests`)

- `auto_enable_backoff_ladder` (第 274–282 行)

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│                        adapter_watch.rs                             │
│                                                                     │
│  start_adapter_watch (Loop every 15s)                               │
│  ├── get_all_adapters_cached () ───────────────────────────────────► │
│  │   (Reads cached adapter list, max 4s stale)                     │
│  │                                                                  │
│  ├── [Every 4th round (60s)]                                        │
│  │   spawn_blocking ( refresh_class_subkey_cache )                 │
│  │   │                                                             │
│  │   ▼                                                             │
│  │   registry.rs: build_class_subkey_cache ()                      │
│  │   │  (Sync I/O: Enum HKLM\...\Class\{...} subkeys)              │
│  │   └──► Updates CLASS_SUBKEY_CACHE                               │
│  │                                                                  │
│  ├── Detect Changes (sorted by name)                               │
│  │   ├── adapters_changed ──► EventBus.emit_adapters_changed       │
│  │   └── disabled_changed ──► EventBus.emit_disabled_adapters      │
│  │       ├── emit_adapter_disabled_warning                         │
│  │       └── (if recovered) spawn_background_check_once            │
│  │                                                                  │
│  └── Auto-Enable Logic                                             │
│      ├── Check Night Guard Window                                  │
│      ├── Filter Candidates (configured but disabled)               │
│      └── enable_adapter (with UAC fallback & backoff)              │
│          │                                                         │
│          ▼                                                         │
│     registry.rs / adapter_cache.rs                                 │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **初始化**: `CLASS_SUBKEY_CACHE` 初始为 `None`。首次调用 `class_subkey_has_matching_guid` 或 `is_admin_disabled_via_registry` 时触发 `ensure_cache_initialized`，执行全量注册表遍历构建缓存。
2. **周期性刷新**: `adapter_watch` 每 15 秒轮询一次。每 4 轮（60 秒），通过 `spawn_blocking` 调用 `refresh_class_subkey_cache`，重新遍历注册表 Class 子键并替换缓存。这是解决缓存陈旧的关键步骤。
3. **可见性检查**: 当需要判断网卡可见性时，`is_visible_in_ncpa` 依次检查：
   - `ShowInNetworkConnections` 缓存（TTL 5s）。
   - `CLASS_SUBKEY_CACHE` 中是否存在对应的 GUID（防幽灵虚拟副本）。
4. **禁用分类**: `is_admin_disabled_via_registry` 读取 `CLASS_SUBKEY_CACHE` 中的 `ConfigFlags`，判断是否为管理员禁用。
5. **变更响应**: 如果检测到适配器或禁用列表变化，通过 `EventBus` 通知前端，并可能触发自动启用或背景检查。

## Known Issues

- **缓存陈旧历史缺陷**: `CLASS_SUBKEY_CACHE` 曾仅在 `enable_adapter` 时刷新，导致运行期设备管理器禁用/拔插操作无法被感知。现已通过 60s 周期刷新解决（`adapter_watch.rs:43-45`）。
- **注册表 I/O 阻塞**: `build_class_subkey_cache` 是同步 I/O 操作，必须包在 `spawn_blocking` 中执行，避免阻塞 async 任务线程（`adapter_watch.rs:47-49`）。
- **可见性误判**: 空 GUID 的网卡在旧逻辑中被静默隐藏，现回退为"可见"，交由后续黑名单/IP 过滤兜底（`registry.rs:62-64`）。

## Connections

[[adapter-linkspeed-u64-max]]、[[adapter-operation-scope]]