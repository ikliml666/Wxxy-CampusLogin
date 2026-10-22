---
title: "夜间出站自动切换（出站层夜切）"
type: module
source_files:
  - tauri-app/src-tauri/src/config/outbound_switch.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/night_switch.rs
  - tauri-app/src-tauri/src/platform/metric.rs
  - tauri-app/frontend/src/network/outboundOrder.ts
tags: [决策, 夜间出站, 出站切换, metric, 夜切, 双端同构]
---

## Overview

本模块实现**夜间出站自动切换**功能，解决校园网（无锡学院）在特定时间段（周日至周四 23:00、周五六 23:30）整体断网的问题。由于断网是运营商层面的，传统的「自动切换运营商」（`night-operator-switch`）无法生效。本模块工作在**网络接口层**，通过修改网卡跃点（Metric）、禁用校园网卡，将系统默认出站流量切换至非校园网（如热点、物联网卡）。

核心特性：
1.  **桌面端**：基于 Windows API 读写接口跃点，结合 `SetIpInterfaceEntry` 持久化配置。采用「禁用校园网卡 + 保留 Metric」两件套方案——禁用是主力（网卡级下线后路由/DNS/跃点竞争全部消失，代理内核只能改绑热点；metric=1 单独压不住 metric=0 的 TUN 默认路由），确保即使代理软件（如 Mihomo/TUN）退出后也能正确路由。兜底路由能力已废弃。
2.  **安卓端**：通过注销认证触发系统 `network_avoid_bad_wifi` 机制，自动切换至蜂窝网络。
3.  **状态机与幂等性**：纯函数判定逻辑与有副作用的执行层分离。通过三份快照（Metric、禁用名单、兜底路由）承载切换态，支持崩溃恢复、启动对账与重放（Replay）。
4.  **守护与防干扰**：引入守护窗口（Guard Window）防止后台巡检误操作；退避策略（Backoff）处理提权失败；看门狗（Watchdog）复核禁用状态。

## Key Components

### 1. `config/outbound_switch.rs`：纯函数判定层
提供无副作用的状态机逻辑，决定是否需要执行 Switch 或 Restore。

*   **枚举 `NightOutboundAction`** (`outbound_switch.rs`)
    *   `None`: 无需动作。
    *   `Switch`: 执行切换（切非校园网）。
    *   `Restore`: 执行还原（恢复校园网）。
*   **函数 `evaluate_night_outbound`** (`outbound_switch.rs:27`)
    *   **签名**: `pub fn evaluate_night_outbound(enabled: bool, weekday: u32, now_minutes: u32, restore_active: bool) -> NightOutboundAction`
    *   **逻辑**:
        *   若 `!enabled` → `None` (行 28)。
        *   若 `restore_active` 且 `now_minutes ∈ [450, 1380)` (07:30-23:00) → `Restore` (行 32-34)。
        *   若 `!restore_active` 且 `now_minutes < 450` (凌晨) → `Switch` (凌晨补切，处理睡眠错过切换时，行 37-39)。
        *   若 `!restore_active` 且已过当日切换时刻 → `Switch` (过点补触发，行 40-42)。
        *   其余 → `None`。
*   **函数 `is_night_outbound_guard_window`** (`outbound_switch.rs:53`)
    *   **签名**: `pub fn is_night_outbound_guard_window(enabled: bool, weekday: u32, now_minutes: u32) -> bool`
    *   **逻辑**: 返回 `true` 表示处于守护窗口，从当日切换时刻起至次日 07:30 恢复窗开止。用于防止巡检自动启用目标卡 (行 60)。
*   **函数 `outbound_restore_active`** (`outbound_switch.rs:68`)
    *   **签名**: `pub fn outbound_restore_active(metric_restore: &str, disabled_adapters: &str, standby_route: &str) -> bool`
    *   **逻辑**: 判断是否处于切换态。若三者任一非空即视为 `true` (行 68-70)。

### 2. `config/model.rs`：配置模型定义
定义 `Config` 结构体中的夜间出站切换相关字段。

*   **常量**:
    *   `PASSWORD_MASK`: `"***"` (行 3)。
    *   `AUTO_DETECT_ADAPTER`: `"自动检测"` (行 4)。
*   **结构体 `Config`** (`model.rs`)
    *   容器级 `#[serde(default)]` (行 9)，防止缺字段导致重置丢配置。
    *   **夜切相关字段**:
        *   `enable_night_operator_switch: bool`: 总开关，默认 false (行 56)。
        *   `night_operator_restore: String`: 暂存原运营商，非空=切换态 (行 59)。
        *   `enable_night_outbound_switch: bool`: 出站切换总开关，默认 false (行 63)。
        *   `outbound_priority: Vec<String>`: 出站网卡优先级列表，空=未排序 (行 69)。
        *   `outbound_metric_restore: String`: 目标卡 metric 快照 JSON，非空=桌面切换态 (行 74)。
        *   `outbound_disabled_adapters: String`: 禁用校园网卡快照 JSON (行 80)。
        *   `outbound_standby_route: String`: 兜底默认路由快照 JSON (行 85)。
        *   `outbound_manual_hold_day: i32`: “立即切换”当日保持标记，防自动切换叠加 (行 92)。
        *   `night_outbound_restore: String`: 安卓切换态标记，恒 "logged_out" (行 96)。
    *   **其他关键字段**: `self_hello_enabled` (行 17), `lightweight_mode` (行 42), `scheduled_login/logout_minutes` (行 161-165)。
*   **辅助函数**:
    *   `deserialize_non_empty_or` (行 178): 空串回退到默认函数值。
    *   `default_true` (行 204), `default_campus_check_end_minutes` (行 210, 1380), `default_portal_url` (行 226, "http://10.1.99.100"), `default_fixed_gateway` (行 216, "10.2.127.254") 等。
*   **`impl Default for Config`** (行 238-313):
    *   初始化所有字段。注意 `auto_exit_after_login` 默认 false (行 254)，`background_check_interval` 默认 60000ms (行 261)，`config_version` 默认 5 (行 310)。
*   **`impl Config`** (行 315-330):
    *   `masked_for_display` / `mask_in_place`: 唯一出口约定，将密码替换为 MASK，防止明文泄露。

### 3. `config/night_switch.rs`：时间常数与工具
提供昼夜切换共用的时间常量和解析工具。

*   **常量**:
    *   `RESTORE_START_MINUTES`: `450` (07:30) (行 10)。
    *   `RESTORE_END_MINUTES`: `1380` (23:00) (行 12)。
*   **枚举 `NightSwitchAction`** (`night_switch.rs`)
    *   `None`: 无动作。
    *   `SwitchToCampus`: 暂存 operator 到 restore，operator 置空，触发登录。
    *   `Restore`: operator 恢复为 restore 值，清空 restore，触发登录。
*   **函数 `switch_time_for`** (`night_switch.rs:26`)
    *   **签名**: `pub(crate) fn switch_time_for(weekday: u32) -> Option<u32>`
    *   **逻辑**: 返回切换分钟数。周日至周四 (0..=4) 返回 `Some(1380)` (23:00)；周五六 (5|6) 返回 `Some(1410)` (23:30) (行 26-32)。
*   **函数 `evaluate_night_switch`** (`night_switch.rs:51`)
    *   **签名**: `pub fn evaluate_night_switch(enabled: bool, weekday: u32, now_minutes: u32, current_operator: &str, restore_operator: &str) -> NightSwitchAction`
    *   **逻辑**:
        *   `!enabled` → `None` (行 58)。
        *   恢复条件: `restore_operator` 非空 且 `current_operator` 为空串 (确实在无锡学院态) 且 `now_minutes ∈ [450, 1380)` → `Restore` (行 61-65)。
        *   切换条件: 当日有切换时刻 且 `now_minutes >= 时刻` 且 `current_operator` 为白名单 (`@telecom`/`@unicom`/`@cmcc`) → `SwitchToCampus` (行 67-70)。
        *   其余 → `None`。
    *   幂等性: 重复调用不会重复写配置，因为状态由 `restore_operator` 和 `operator` 共同承载。
*   **函数 `parse_chkstatus`** (`night_switch.rs:91`)
    *   **签名**: `pub fn parse_chkstatus(body: &str) -> Option<ChkStatusInfo>`
    *   **逻辑**: 解析 eportal JSONP 响应 (`dr1003({...})`)。剥 BOM/空白/callback 壳。返回 `ChkStatusInfo { online, uid, oltime }`。任何输入都不 panic，非法文本返回 None (行 91-104)。
*   **函数 `uid_matches`** (`night_switch.rs:108`)
    *   **签名**: `pub fn uid_matches(uid: &str, user: &str, expected_operator: &str) -> bool`
    *   **逻辑**: 核对在线账号 uid 与期望账号：trim、忽略 ASCII 大小写后精确相等 (行 108-110)。

### 4. `platform/metric.rs`：Windows 平台接口跃点读取
通过 Windows API 读取接口跃点信息。供快照读取和状态判定使用。

*   **结构体 `MetricRow`** (`metric.rs`)
    *   `family: u16`: IP 族 (IPv4/IPv6) (行 16)。
    *   `automatic: bool`: 是否自动跃点 (行 17)。
    *   `metric: u32`: 跃点值 (行 18)。
*   **函数 `interface_rows_for_guid`** (`metric.rs:25`)
    *   **逻辑**: 底层实现。调用 `GetIpInterfaceTable` 枚举所有接口，通过 LUID→GUID 转换匹配指定 GUID 的行。返回原始 `MIB_IPINTERFACE_ROW`。注意内存释放 `FreeMibTable` (行 25-53)。
*   **函数 `read_interface_metrics`** (`metric.rs:56`)
    *   **签名**: `pub fn read_interface_metrics(guid: &str) -> Result<Vec<MetricRow>, String>`
    *   **逻辑**: 高层封装。调用 `interface_rows_for_guid` 并转换为 `Vec<MetricRow>`。无需管理员权限 (行 56-65)。

### 5. `monitor/outbound_switch.rs`：桌面侧动作执行与网卡选择
负责具体的网卡筛选、校园网判定、快照序列化及路由清理决策。

*   **函数 `is_campus_adapter`** (`outbound_switch.rs:17`)
    *   **签名**: `pub fn is_campus_adapter(ip: &str, gateway: &str, campus_gateway: &str, portal_host: &str, gateway_probe: impl Fn(&str, &str) -> bool) -> bool`
    *   **逻辑**: 逐卡判定是否在校园网。
        *   IP 同 /18 网段。
        *   绑源 IP 可达校园网关（两发去抖）。
        *   绑源 IP 可达 Portal 主机私网 IPv4（若配置了域名解析出的私网 IP）(行 17-39)。
*   **函数 `portal_probe_host`** (`outbound_switch.rs:44`)
    *   **签名**: `pub fn portal_probe_host(portal_url: &str) -> String`
    *   **逻辑**: 从 portal URL 提取私网 IPv4 作为探测目标，过滤公网/域名 (行 44-56)。
*   **函数 `effective_outbound_priority`** (`outbound_switch.rs:63`)
    *   **签名**: `pub fn effective_outbound_priority(priority: &[String], details: &[(Adapter, String)]) -> Vec<String>`
    *   **逻辑**: 生效优先级列表。用户已排序则透传；为空则按“无线优先、有线随后”生成默认序 (行 63-82)。
*   **函数 `select_outbound_candidate`** (`outbound_switch.rs:87`)
    *   **签名**: `pub fn select_outbound_candidate(priority: &[String], details: &[(Adapter, String)], campus_gateway: &str, portal_host: &str, gateway_probe: impl Fn(&str, &str) -> bool) -> Option<Adapter>`
    *   **逻辑**: 选出第一个非校园、有 IP、网关可达的适配器作为切换目标 (行 87-111)。
*   **数据结构与序列化**:
    *   `SnapshotRow` (行 116): `guid`, `family`, `automatic`, `metric`。
    *   `snapshot_json` (行 125): 序列化选中卡的接口跃点快照为 JSON。
    *   `DisabledRow` (行 142): `guid`, `name`。
    *   `disabled_adapters_json` (行 148): 序列化待禁用校园网卡名单。
    *   `parse_disabled_adapters` (行 156): 解析禁用名单。空串→空；非法 JSON→Err（调用方须告警）。
    *   `StandbyRoute` (行 169): `dest`, `mask`, `gateway`, `metric`, `if_index`。历史兜底路由快照。
    *   `parse_standby_route` (行 179): 解析兜底路由快照。
*   **兜底路由清理逻辑**:
    *   `StandbyCleanup` 枚举 (行 187): `AlreadyGone`, `ForeignRowKept`, `LastDefaultPath`, `Safe`。
    *   `standby_cleanup_decision` (行 203-220): 纯函数判定。基于 `same_key_metrics` (同网关+接口的跃点列表) 和 `rows_outside_exact` 决定能否安全删除。
    *   `standby_route_cleanup` (行 224-270): Windows 平台读取实时路由表进行判定；非 Windows 返回 `AlreadyGone`。
*   **函数 `select_campus_to_disable`** (`outbound_switch.rs:285`)
    *   **签名**: `pub fn select_campus_to_disable(priority: &[String], details: &[(Adapter, String)], campus_gateway: &str, portal_host: &str, exclude_guid: &str, gateway_probe: impl Fn(&str, &str) -> bool, bus_guard: impl Fn(&str) -> bool) -> Vec<DisabledRow>`
    *   **逻辑**: 选出本次切换要临时禁用的校园网卡。过滤条件: 在优先级列表内、有 IP、判定为校园网、有 GUID、非目标卡、总线可判定 (`bus_guard` 注入) (行 285-318)。
*   **函数 `unsafe_to_disable`** (`outbound_switch.rs:278`)
    *   **签名**: `pub fn unsafe_to_disable(guid: &str) -> bool`
    *   **逻辑**: Windows 特有：实例 ID 读不到的卡保守跳过 (行 278-280)。

### 6. `monitor/outbound_night.rs`：生命周期管理与执行
夜间出站切换的完整生命周期管理，包括切换、还原、重放、看门狗、启动对账及手动命令入口。

*   **全局原子状态** (`outbound_night.rs`)
    *   `OUTBOUND_SWITCH_LAST_FAIL_MS`: 切换动作最后失败时间戳 (行 55)。
    *   `OUTBOUND_SWITCH_FAIL_COUNT`: 切换动作失败计数 (行 56)。
    *   `OUTBOUND_RESTORE_LAST_FAIL_MS`: 还原动作最后失败时间戳 (行 58)。
    *   `OUTBOUND_RESTORE_FAIL_COUNT`: 还原动作失败计数 (行 59)。
    *   `OUTBOUND_CHANNEL_FAIL_COUNT`: 提权通道失败计数 (行 63)。
    *   `OUTBOUND_ACTION_BUSY`: `AtomicBool`，单飞闸，防止并发冲突 (行 76)。
    *   每日告警去重标记: `OUTBOUND_NO_CANDIDATE_WARN_DAY`, `OUTBOUND_EMPTY_DISABLE_WARN_DAY`, `OUTBOUND_METRIC_SNAPSHOT_WARN_DAY`, `OUTBOUND_LAST_PATH_WARN_DAY` (行 67-70)。
*   **结构体 `OutboundActionGuard`** (`outbound_night.rs:79`)
    *   RAII 锁结构，确保互斥且 panic 安全释放 (行 79-94)。
*   **函数 `run_outbound_blocking`** (`outbound_night.rs:98`)
    *   **逻辑**: 异步包装器，将阻塞操作送入线程池执行 (行 98-110)。
*   **函数 `outbound_switch_active`** (`outbound_night.rs:114`)
    *   **逻辑**: 判定当前是否处于切换态（三份快照任一非空）(行 114-121)。
*   **函数 `in_outbound_restore_window`** (`outbound_night.rs:125`)
    *   **逻辑**: 判定是否在 [07:30, 23:00) 恢复窗口 (行 125-127)。
*   **退避逻辑**:
    *   `outbound_backoff_ms` (行 137-144): 计算退避时长（60s起步，翻倍，封顶300s）。
    *   `backoff_gate` (行 148-153): 判断是否超过退避时间。
    *   `switch_gate_open` / `restore_gate_open` (行 155-169): 分别检查切换/还原的退避闸。
*   **函数 `write_outbound_metric`** (`outbound_night.rs:304`)
    *   **逻辑**: 写跃点 Metric=1。处理无跃点行、helper 成功/失败、通知发送 (行 304-365)。
*   **函数 `apply_outbound_switch`** (`outbound_night.rs:391`)
    *   **逻辑**: 切换主流程。获取单飞锁 → 选目标卡 → 选需禁用的校园卡 → 落盘快照 → 执行 helper: 写跃点 + 禁用校园卡 → 路由验证 (行 391-583)。
*   **函数 `replay_outbound_switch`** (`outbound_night.rs:600`)
    *   **逻辑**: 补齐/重放。针对切换未完成的拍次，补写跃点和补做禁用。幂等执行 (行 600-688)。
*   **函数 `watchdog_re_disable_campus`** (`outbound_night.rs:697`)
    *   **逻辑**: 看门狗。复核禁用名单，若校园卡被外部重新启用，则再次禁用 (行 697-762)。
*   **函数 `apply_outbound_restore`** (`outbound_night.rs:776`)
    *   **逻辑**: 还原主流程。清理历史兜底路由 → 恢复跃点快照 → 重新启用校园网卡 → 收尾清快照 → 特殊处理：快照损坏时的放弃阀 (行 776-1089)。
*   **函数 `finish_outbound_restore`** (`outbound_night.rs:1096`)
    *   **逻辑**: 清快照、清零退避、清除手动 Hold，最后调用 `background_task::spawn_background_check_after_outbound_restore` 补一次巡检 (行 1096-1112)。
    *   **为什么补检**：还原刚把校园网卡从设备级禁用里启用，DHCP 要几秒到几十秒才下发地址；不补检就只能等下一个巡检周期——轻量化下下限 300s（`app/lightweight::effective_background_interval_ms`），这正是「整夜休眠后到点不自动登录、只能手动点」的次因。补检钩子先轮询适配器 IP（`POST_RESTORE_IP_POLL_MS`=3000，上限 `POST_RESTORE_IP_WAIT_MAX_MS`=45000，用 `get_adapters_force` 以绕过 cached 的 TTL 旧快照），再走 `spawn_background_check_once` 发起一次性巡检。
    *   **挂载点唯一性**：自动还原、手动还原、启动对账三条路径都汇到本函数，钩子只挂一处；且必须排在 `clear_outbound_snapshot` 之后——巡检读到的 `outbound_restore_active` 已是 false，否则会被「出站切换态跳过」闸门挡回。
*   **手动命令入口**:
    *   `manual_outbound_switch` (行 1162-1180): 用户立即切换。
    *   `manual_outbound_restore` (行 1184-1197): 用户立即还原。
*   **启动对账**:
    *   `reconcile_outbound_on_startup` (行 1204-1262): 进程启动时检查残留切换态，决定是还原还是重放切换。
*   **内部结构体**:
    *   `OutboundSnapshotRow` (行 210-215): 跃点快照解析结构。
    *   `HelperFailure` (行 226-233): Helper 调用失败分类（channel vs business）。
*   **常量**:
    *   `OUTBOUND_HELPER_TIMEOUT_SECS`: `30` (行 20)。
    *   `OUTBOUND_BACKOFF_START_MS`: `60_000` (行 24)。
    *   `OUTBOUND_BACKOFF_MAX_MS`: `300_000` (行 25)。
    *   `OUTBOUND_RESTORE_ALERT_FAILS`: `3` (行 30)。
    *   `OUTBOUND_RESTORE_ALERT_REMIND_EVERY`: `20` (行 32)。
    *   `OUTBOUND_RESTORE_GIVE_UP_FAILS`: `40` (行 36)。

### 7. `frontend/src/network/outboundOrder.ts`：前端排序逻辑
前端网络面板中用于拖拽排序网卡的逻辑，与后端 `effective_outbound_priority` 语义对齐。

*   **函数 `buildOutboundOrder`** (`outboundOrder.ts`)
    *   **签名**: `export function buildOutboundOrder(priority: string[] | undefined, detected: string[], wirelessNames?: string[]): string[]`
    *   **逻辑**:
        *   若 `priority` 存在：优先保留 priority 中存在的网卡，其余按检测顺序追加 (行 11-30)。
        *   若 `wirelessNames` 存在：优先无线网卡，其余按检测顺序。
        *   否则：直接返回检测顺序。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                         User / Scheduler / Startup                          │
└───────────────────────────┬─────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                     monitor/outbound_night.rs                               │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────────┐   │
│  │ ApplySwitch  │ │ ApplyRestore │ │ ReplaySwitch │ │ Watchdog         │   │
│  └──────┬───────┘ └──────┬───────┘ └──────┬───────┘ └────────┬─────────┘   │
└─────────┼────────────────┼────────────────┼──────────────────┼─────────────┘
          │                │                │                  │
          ▼                ▼                ▼                  ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                      config/outbound_switch.rs                              │
│  ┌──────────────────────┐ ┌────────────────────────────┐                   │
│  │ evaluate_night_      │ │ is_night_outbound_guard    │                   │
│  │ outbound             │ │ window                     │                   │
│  └──────────────────────┘ └────────────────────────────┘                   │
└─────────────────────────────────────────────────────────────────────────────┘
          │                │                │                  │
          ▼                ▼                ▼                  ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                    monitor/outbound_switch.rs                               │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────────────┐   │
│  │ select_candidate │ │ select_campus    │ │ standby_cleanup_decision │   │
│  │                  │ │ _to_disable      │ │                          │   │
│  └────────┬─────────┘ └────────┬─────────┘ └──────────┬───────────────┘   │
└───────────┼────────────────────┼──────────────────────┼───────────────────┘
            │                    │                      │
            ▼                    ▼                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                   platform/metric.rs / Helper Spawn                         │
│  ┌──────────────────┐ ┌────────────────────────────────────────────────┐   │
│  │ read_interface   │ │ run_helper_op (set_metric/disable/route_delete)│   │
│  │ metrics          │ └────────────────────────────────────────────────┘   │
│  └──────────────────┘                                                      │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **触发**: `outbound_night.rs` 中的定时拍、启动对账或手动命令触发 `apply_outbound_switch` 或 `apply_outbound_restore`。
2.  **状态检查**: 通过 `outbound_restore_active` 检查 `AppState` 中的三份快照（`metric_restore`, `disabled_adapters`, `standby_route`）是否非空。
3.  **执行动作**:
    *   **切换**: 读取当前跃点 (`read_interface_metrics`) → 生成快照 JSON → 写入 `Config` 并持久化 (`save_config_and_broadcast`) → 调用 `run_helper_op("set_metric")` 和 `"disable_adapter"`。
    *   **还原**: 解析现有快照 → 调用 `run_helper_op("route_delete")` (如有兜底路由) → 调用 `run_helper_op("set_metric")` 写回原值 → 调用 `enable_campus_adapter` 启用网卡 → 清空快照字段并持久化。
4.  **Helper 调用**: `run_helper_op` 封装了 `helper_spawn::spawn_elevated_helper`，处理提权、超时和结果解析。区分通道失败和业务失败，影响 `OUTBOUND_CHANNEL_FAIL_COUNT`。
5.  **反馈**: 动作结果更新退避状态 (`mark_outbound_failure` / `clear_outbound_failure`)，并根据情况发送通知 (`emit_notification`)。

## Connections

[[night-operator-switch]]、[[dual-platform-sharing]]、[[scheduled-actions-outside-silent-window]]、[[config-field-sets-bidirectional-sync]]、[[logout-radius-first]]、[[set-ip-interface-entry-metric]]、[[windows-task-proxy-elevation]]、[[outbound-switch]]、[[desktop-config]]

## Known Issues

1.  **`auto_login_on_start` 未过出站闸**: 启动自动登录链路不判切换态——切换态内重启应用会重登校园网。桌面登录失败无害（启动对账会在夜间窗口重放切换）；安卓重登成功则流量回 WiFi，切换态名存实亡但标记仍在，07:30 恢复窗口会再登一次并清标记自愈。
2.  **桌面 `nightOutboundRestore` 字段是死字段**: 为满足「字段集双端同构」纪律而存在；桌面真实切换态字段是 `outboundMetricRestore` 三快照，桌面代码不消费 `nightOutboundRestore`。
3.  **安卓同拍双登可能**: 出站 Restore 成功当拍置 `outbound_active=false`，同拍随后的定时登录判定可能再登一次——恢复窗口恰好压着定时登录时刻时出现。后果轻（重复登录幂等），不为此引入拍内去重。
4.  **功能已关+态残留的兜底判据内联不重构**: 桌面抽为可测纯函数 `outbound_action_for`，安卓在 `run_scheduled_actions` 内联同款三行——刻意不抽公共纯函数（安卓编排本就独立于桌面 monitor 模块，抽取需扩大桌面 crate 的 pub 面，收益小）。两端行为必须保持同步，改动时人工对照。
5.  **captive portal 白名单误判风险**: 个别校园网部署放行系统探测 URL——安卓注销后系统验证仍"通过"，`avoid_bad_wifi` 不切流量。真机实测项；命中则改"注销+提示"降级。
6.  **切换时机由系统探测周期决定**: `reportNetworkConnectivity` 后系统按自身节奏重验证，不保证 23:00 整点切蜂窝，与"自动衔接"语义相容但非秒级。
7.  **`config/model.rs` 与 `metric.rs` 注释口径冲突**: 前者仍称「运行时修改重启即还原」，后者实证 `SetIpInterfaceEntry` 落持久层。文档以 `metric.rs` 为准，代码注释待顺手修（行为本身已由快照还原兜住）。