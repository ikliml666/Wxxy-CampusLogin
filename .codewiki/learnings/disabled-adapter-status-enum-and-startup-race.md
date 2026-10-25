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

## Overview

本文档以当前代码为准，记录 `DisabledAdapter.status` 采用 `AdapterStatus` 枚举序列化（而非历史中文文案）的跨端契约，以及 `adapter_watch` 后台任务中的启动竞态修复、差分推送、禁用恢复触发、自动启用退避与夜间守护窗口逻辑。核心后端状态四分类由 `classify_adapter_status` 纯函数决策，综合 `OperStatus`、`AdminStatus`、注册表 `ConfigFlags` 与 PnP 设备树存活状态，修复了禁用网卡报 `Down` 漏判及幽灵 USB 网卡误报禁用等历史缺陷。

## Key Components

### `tauri-app/src-tauri/src/network/discovery/mod.rs`

- **`AdapterStatus` 枚举** (行 39-46): 四分类 `Disabled`、`Disconnected`、`EnabledNoIp`、`Connected`，派生 `Serialize` 并 `#[serde(rename_all = "camelCase")]`，序列化为英文 key（如 `disabled`）供前端 i18n 映射。
- **`Adapter` struct** (行 48-60): 字段 `name: String`、`ip: String`、`wireless: bool`、`guid: String`、`mac: String`、`if_index: u32`、`status: AdapterStatus`、`link_speed: u64`（连接速度 bit/s，0 未知）。所有通过过滤的适配器均推入此列表。
- **`AdapterDetail` struct** (行 62-76): 字段 `name: String`、`ip: String`、`wireless: bool`、`subnet_mask: String`、`gateway: String`、`dhcp_server: String`、`mac: String`、`if_index: u32`、`status: AdapterStatus`、`link_speed: u64`。仅 `Connected` 与 `EnabledNoIp` 推入。
- **`DisabledAdapter` struct** (行 78-86): 字段 `name: String`、`status: AdapterStatus`、`description: String`。仅 `Disabled` 状态推入，与 `Adapter.status` 同源序列化。
- **`BL_REGEX`** (行 24-32): `lazy_static` 黑名单正则，过滤虚拟网卡/VPN/中文虚拟关键词（如 `虚拟|伪|假|测试|模拟|隧道|本地连接`）。
- **`is_blacklisted(name: &str)`** (行 97-99): 调用 `BL_REGEX.is_match` 检查名称/描述。
- **`prefix_len_to_mask(len: u32)`** (行 102-113): Windows 平台将 IPv4 前缀长度转点分十进制掩码。
- **`new_command(program: &str)`** (行 89-95): 创建子进程命令，Windows 附加 `0x08000000` 隐藏窗口标志。
- **`query_adapters_addresses()`** (行 116-125): 平台分发，Windows 调 `windows::query_adapters_addresses`，其他返回空。
- **测试模块** (行 127-191): 含 `blacklist_word_boundary_does_not_match_legit_nics` (行 133-144)、`blacklist_word_boundary_still_matches_known_virtuals` (行 146-155)、`blacklist_chinese_virtual_keywords` (行 157-167)、`blacklist_does_not_match_legit_chinese_nics` (行 169-174)、`disabled_adapter_status_serializes_as_enum_key` (行 180-190) 锁住 `status` 序列化为 `"disabled"` 契约。

### `tauri-app/src-tauri/src/network/discovery/windows.rs`

- **`query_adapters_addresses()`** (行 12-63): 调用 Win32 `GetAdaptersAddresses`（标志 `0x0080|0x0100`），含缓冲区扩容重试（最多 3 次）。
- **`parse_adapter_addresses()`** (行 65-284): 遍历 `IP_ADAPTER_ADDRESSES_LH` 链表，过滤非以太网/WLAN 类型（行 101-104）及零物理地址（行 106-109）；调用 `is_visible_in_ncpa` (行 114)、`is_blacklisted` (行 121)；计算 `link_speed` 归零哨兵 (行 131-134)；提取 IP/网关/DHCP (行 145-198)；调用 `classify_adapter_status` (行 218-225)；按状态推入 `adapters` (行 228-237)、`details` (行 241-269) 或 `disabled` (行 270-276)。
- **`read_pwstr(ptr)`** (行 286-307): 安全读取 Windows PWSTR 适配器名。
- **`ipv4_from_in_addr(addr)`** (行 309-311): 转换 `IN_ADDR` 为 IPv4 字符串。
- **`is_admin_status_down(if_index: u32)`** (行 317-328): 通过 `GetIfEntry2` 查 `AdminStatus == Down` 判管理性禁用，失败返回 `None`。
- **`classify_adapter_status()`** (行 345-367): 纯函数，入参 `is_up, has_ip, oper_status, admin_down, registry_disabled, pnp_present`。逻辑：
  - `is_up` → `Connected`/`EnabledNoIp`（行 353-355）
  - `pnp_present == Some(false)` → `Disconnected`（幽灵设备，行 357-359）
  - `admin_down == Some(true)` → `Disabled`（行 360-362）
  - `oper_status == NotPresent && registry_disabled` → `Disabled`（行 363-365）
  - 否则 `Disconnected`（行 366）
- **测试模块** (行 369-479): 覆盖上述决策表，如 `down_with_admin_down_is_disabled` (行 396-402)、`phantom_device_with_admin_down_is_disconnected` (行 451-457) 等。

### `tauri-app/src-tauri/src/monitor/adapter_watch.rs`

- **`start_adapter_watch(app_handle: &AppHandle)`** (行 17-288): 后台任务。
  - 常量 `ADAPTER_WATCH_INTERVAL = 15000` (行 9)；`CLASS_SUBKEY_REFRESH_ROUNDS = 4` (行 15)。
  - `first_round` 首轮强制推送 (行 26, 274, 78, 89) 化解启动竞态。
  - 每 4 轮（60s）`spawn_blocking` 刷新注册表缓存 (行 56-62)。
  - 读缓存 `get_all_adapters_cached` (行 68) 而非强制重查。
  - 差分比较 `adapters_changed` (行 71-78) 与 `disabled_changed` (行 82-89)，按 `name` 排序防顺序误报。
  - 禁用恢复触发一次性检查 `spawn_background_check_once` (行 102-117)。
  - 禁用警告限频 60s (行 118-164)，排除夜间切换亲手禁用卡（`night_disabled` 解析自 `outbound_disabled_adapters`）。
  - 自动启用 (行 166-272)：计算 `configured_disabled_adapters`，夜间守护窗口 `guard_window` (行 184-203) 内跳过；退避由 `auto_enable_backoff_ms` 控制；首次静默提权失败弹 UAC；成功后清零计数并 5s 后触发一次性检查。
- **`auto_enable_backoff_ms(failure_count: u32)`** (行 292-299): 返回 0/60000/120000/300000 封顶。
- **测试模块** (行 301-314): `auto_enable_backoff_ladder` 验证退避阶梯。

### `tauri-app/frontend/src/network/types.ts` & `android/frontend/src/network/types.ts`

- **`AdapterStatus` 类型** (行 1): 联合类型 `'disabled' | 'disconnected' | 'enabledNoIp' | 'connected'`。
- **`Adapter` interface** (行 3-13): 字段 `name: string`、`ip: string`、`wireless: boolean`、`guid?: string`、`mac: string`、`ifIndex: number`、`status: AdapterStatus`、`linkSpeed?: number`。
- **`AdapterDetail` interface** (行 15-27): 字段 `name: string`、`ip: string`、`wireless: boolean`、`subnetMask: string`、`gateway: string`、`dhcpServer: string`、`mac: string`、`ifIndex: number`、`status: AdapterStatus`、`linkSpeed?: number`。
- 注：当前前端未单独声明 `DisabledAdapter` interface；后端 `DisabledAdapter` 含 `description` 字段，前端若消费需自行映射（或复用 `Adapter` 忽略多余字段）。

## Architecture

```text
┌──────────────────────────────────────────┐
│ Frontend Store / EventBus (types.ts 行1) │
│  adapters, disabledAdapters, onChanged   │
└───────────────┬──────────────────────────┘
                │ diff push (行71-89)
┌───────────────▼──────────────────────────┐
│ adapter_watch.rs (monitor, 行17)         │
│ first_round(26) auto-enable(166,292)     │
└───────────────┬──────────────────────────┘
                │ get_all_adapters_cached
┌───────────────▼──────────────────────────┐
│ discovery mod.rs(116)/windows.rs(12,345) │
│ AdapterStatus enum(39) classify(345)     │
└───────────────┬──────────────────────────┘
                │ Win32/Registry/PnP
┌───────────────▼──────────────────────────┐
│ Windows API GetAdaptersAddresses 等       │
└──────────────────────────────────────────┘
```

## Data Flow

1. **发现阶段**: `adapter_watch` 定时（15s）调 `get_all_adapters_cached` (行 68) → `windows::query_adapters_addresses` (行 12) → `parse_adapter_addresses` (行 65)。
2. **状态判定**: `parse_adapter_addresses` 对每个适配器调 `classify_adapter_status` (行 218)，结合 `OperStatus`、`AdminStatus` (行 205, 317)、注册表 (行 206) 和 PnP 设备树 (行 212-217)，得出 `AdapterStatus` 枚举。
3. **分类存储**: 结果分 `adapters` (全部，行 228)、`details` (Connected/EnabledNoIp，行 242-269)、`disabled` (Disabled，行 270-276) 三向量。
4. **差分推送**: `adapter_watch` 对比本轮与上轮（行 71-89）。若 `first_round` 或变化，通过 `EventBus` 发 `emit_adapters_changed` / `emit_adapter_details_changed` / `emit_adapter_disabled_warning` (行 92-162)。
5. **前端更新**: 前端 Store 收事件更新 `adapters` 与 `disabledAdapters`，下拉菜单派生禁用项，value 唯一。
6. **自动启用**: 若手选适配器进 `disabled` 且非夜间守护窗，`adapter_watch` 调 `enable_adapter` (行 243) 恢复，退避由 `auto_enable_backoff_ms` (行 292) 控制。

## Connections

- [[NetworkPanel]]: 前端网络面板组件，消费适配器数据。
- [[adapter_cache]]: 负责适配器缓存刷新（`get_all_adapters_cached` 来源）。
- [[dhcp]]: 负责 DHCP 租约管理（后端 `dhcp_server` 字段采集）。
- [[subnet]]: 负责子网掩码计算（`prefix_len_to_mask` 关联）。
- [[registry]]: `is_visible_in_ncpa` / `is_admin_disabled_via_registry` 注册表可见性与禁用检查。
- [[devnode]]: PnP 设备树存活判定（`read_pnp_instance_id` / `devnode_problem`）。

## Known Issues

- **历史缺陷已修复**: `DisabledAdapter.status` 曾存中文文案导致 i18n 断裂，现已改为枚举序列化（测试锁契约见 mod.rs 行 180-190）。
- **历史缺陷已修复**: `adapter_watch` 启动竞态导致禁用列表冻结，通过 `first_round` 强制推送解决（行 26, 78, 89）。
- **幽灵设备回退路径**: 拔出 USB 网卡后若 PnP 实例 ID 读不到（`pnp_present == None`），`classify_adapter_status` 回退旧行为（行 357, 468-478），可能误判；现代 Windows 通常可查 PnP 树。
- **前端类型缺失**: 当前 `frontend/src/network/types.ts` 未声明 `DisabledAdapter` interface，若后端扩展 `description` 前端需同步，否则契约仅靠 `Adapter` 结构隐式兼容。