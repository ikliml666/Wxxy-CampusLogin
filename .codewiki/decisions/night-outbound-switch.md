---
title: "夜间出站自动切换（出站层夜切）"
type: module
source_files:
  - tauri-app/src-tauri/src/config/outbound_switch.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/night_switch.rs
  - tauri-app/src-tauri/src/monitor/outbound_switch.rs
  - tauri-app/src-tauri/src/monitor/outbound_night.rs
  - tauri-app/src-tauri/src/platform/metric.rs
  - tauri-app/frontend/src/network/outboundOrder.ts
tags: [决策, 夜间出站, 出站切换, metric, 夜切, 双端同构]
---

## Overview

本模块实现**夜间出站自动切换**功能，解决校园网（无锡学院）在特定时间段（周日至周四 23:00、周五六 23:30）整体断网的问题。由于断网是运营商层面的，传统的「自动切换运营商」（`night-operator-switch`）无法生效。本模块工作在**网络接口层**，通过修改网卡跃点（Metric）、禁用校园网卡，将系统默认出站流量切换至非校园网（如热点、物联网卡）；历史兜底路由能力已废弃，仅剩残留清理。

核心特性：
1.  **桌面端**：基于 Windows API 读写接口跃点，结合 `SetIpInterfaceEntry` 持久化配置。采用「禁用校园网卡 + 保留 Metric」两件套方案——禁用是主力（网卡级下线后路由/DNS/跃点竞争全部消失，代理内核只能改绑热点；metric=1 单独压不住 metric=0 的 TUN 默认路由），确保即使代理软件（如 Mihomo/TUN）退出后也能正确路由。原第三步「兜底默认路由」已于 2026-10-18 事故后废弃删除：该路由与目标卡自身 DHCP 默认路由同「下一跳+接口」只差跃点，Windows 判重不看跃点 → route add 恒失败每夜重放，删除侧则可能误删用户自己的默认路由；`outbound_standby_route` 历史快照仅由还原步骤①按四元组精确清理（`standby_cleanup_decision`）。
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
*   **函数 `evaluate_night_outbound`** (`outbound_switch.rs:39`)
    *   **签名**: `pub fn evaluate_night_outbound(enabled: bool, weekday: u32, now_minutes: u32, restore_active: bool) -> NightOutboundAction`
    *   **逻辑**:
        *   若 `!enabled` → `None`。
        *   若 `restore_active` 且 `now_minutes ∈ [450, 1380)` (07:30-23:00) → `Restore`。
        *   若 `!restore_active` 且 `now_minutes < 450` (凌晨) → `Switch` (凌晨补切，处理睡眠错过切换时)。
        *   若 `!restore_active` 且已过当日切换时刻 → `Switch` (过点补触发)。
        *   其余 → `None`。
*   **函数 `is_night_outbound_guard_window`** (`outbound_switch.rs:61`)
    *   **签名**: `pub fn is_night_outbound_guard_window(enabled: bool, weekday: u32, now_minutes: u32) -> bool`
    *   **逻辑**: 返回 `true` 表示处于守护窗口，从当日切换时刻起至次日 07:30 恢复窗开止。用于防止巡检自动启用目标卡。
*   **函数 `outbound_restore_active`** (`outbound_switch.rs:78`)
    *   **签名**: `pub fn outbound_restore_active(metric_restore: &str, disabled_adapters: &str, standby_route: &str) -> bool`
    *   **逻辑**: 判断是否处于切换态。若三者任一非空即视为 `true`。

### 2. `config/model.rs`：配置模型定义
定义 `Config` 结构体中的夜间出站切换相关字段。

*   **结构体 `Config`** (`model.rs`)
    *   `enable_night_outbound_switch: bool`: 开关，默认 `false`。
    *   `outbound_priority: Vec<String>`: 用户排序的网卡名称列表。空则默认无线优先。
    *   `outbound_metric_restore: String`: JSON 快照，存储目标卡 Metric 信息（仅桌面）。
    *   `outbound_disabled_adapters: String`: JSON 快照，存储需禁用的校园网卡 GUID 列表（仅桌面）。
    *   `outbound_standby_route: String`: JSON 快照，存储历史兜底路由信息（仅桌面，兼容清理用）。
    *   `outbound_manual_hold_day: i32`: 手动立即切换的冻结标记（`num_days_from_ce`），0 表示无。
    *   `night_outbound_restore: String`: 安卓标记，恒为 `"logged_out"` 时表示已注销待重检。

### 3. `config/night_switch.rs`：时间常数与工具
提供昼夜切换共用的时间常量和解析工具。

*   **常量**:
    *   `RESTORE_START_MINUTES`: `450` (07:30)。
    *   `RESTORE_END_MINUTES`: `1380` (23:00)。
*   **函数 `switch_time_for`** (`night_switch.rs:30`)
    *   **签名**: `pub(crate) fn switch_time_for(weekday: u32) -> Option<u32>`
    *   **逻辑**: 返回切换分钟数。周日至周四 (0..=4) 返回 `Some(1380)` (23:00)；周五六 (5|6) 返回 `Some(1410)` (23:30)。
*   **函数 `parse_chkstatus`** (`night_switch.rs:86`)
    *   **签名**: `pub fn parse_chkstatus(body: &str) -> Option<ChkStatusInfo>`
    *   **逻辑**: 解析 eportal JSONP 响应，提取 `online`, `uid`, `oltime`。容错 BOM 头和残缺壳。
*   **函数 `uid_matches`** (`night_switch.rs:105`)
    *   **签名**: `pub fn uid_matches(uid: &str, user: &str, expected_operator: &str) -> bool`
    *   **逻辑**: 核对在线账号 UID 与期望账号（忽略大小写和空白）。注意源码中该函数签名为 `uid_matches(uid: &str, user: &str, expected_operator: &str)`，用于匹配 `学号+运营商后缀`。

### 4. `monitor/outbound_switch.rs`：桌面侧动作执行与网卡选择
负责具体的网卡筛选、校园网判定、快照序列化及路由清理决策。

*   **函数 `is_campus_adapter`** (`outbound_switch.rs:20`)
    *   **签名**: `pub fn is_campus_adapter(ip: &str, gateway: &str, campus_gateway: &str, portal_host: &str, gateway_probe: impl Fn(&str, &str) -> bool) -> bool`
    *   **逻辑**: 逐卡判定是否在校园网。
        *   IP 同 /18 网段。
        *   绑源 IP 可达校园网关（两发去抖）。
        *   绑源 IP 可达 Portal 主机私网 IPv4（若配置了域名解析出的私网 IP）。
*   **函数 `portal_probe_host`** (`outbound_switch.rs:47`)
    *   **签名**: `pub fn portal_probe_host(portal_url: &str) -> String`
    *   **逻辑**: 从 portal URL 提取私网 IPv4 作为探测目标，过滤公网/域名。
*   **函数 `effective_outbound_priority`** (`outbound_switch.rs:67`)
    *   **签名**: `pub fn effective_outbound_priority(priority: &[String], details: &[(Adapter, String)]) -> Vec<String>`
    *   **逻辑**: 生效优先级列表。用户已排序则透传；为空则按“无线优先、有线随后”生成默认序。
*   **函数 `select_outbound_candidate`** (`outbound_switch.rs:93`)
    *   **签名**: `pub fn select_outbound_candidate(priority: &[String], details: &[(Adapter, String)], campus_gateway: &str, portal_host: &str, gateway_probe: impl Fn(&str, &str) -> bool) -> Option<Adapter>`
    *   **逻辑**: 选出第一个非校园、有 IP、网关可达的适配器作为切换目标。
*   **函数 `snapshot_json`** (`outbound_switch.rs:131`)
    *   **签名**: `pub fn snapshot_json(guid: &str, rows: &[crate::platform::metric::MetricRow]) -> String`
    *   **逻辑**: 序列化接口跃点快照 `[{guid, family, automatic, metric}]`。
*   **函数 `select_campus_to_disable`** (`outbound_switch.rs:208`)
    *   **签名**: `pub fn select_campus_to_disable(priority: &[String], details: &[(Adapter, String)], campus_gateway: &str, portal_host: &str, exclude_guid: &str, gateway_probe: impl Fn(&str, &str) -> bool, bus_guard: impl Fn(&str) -> bool) -> Vec<DisabledRow>`
    *   **逻辑**: 选出需临时禁用的校园网卡名单。排除目标卡、无 IP 卡、非校园卡、总线不可判定卡 (`bus_guard`)。
*   **函数 `standby_cleanup_decision`** (`outbound_switch.rs:175`)
    *   **签名**: `pub fn standby_cleanup_decision(same_key_metrics: &[u32], standby_metric: u32, rows_outside_exact: usize) -> StandbyCleanup`
    *   **逻辑**: 历史兜底路由清理决策。返回 `StandbyCleanup` 枚举 (AlreadyGone, ForeignRowKept, LastDefaultPath, Safe)。基于同网关+接口的跃点列表精确匹配，防止误删用户默认路由。
*   **函数 `standby_route_cleanup`** (`outbound_switch.rs:196`)
    *   **签名**: `#[cfg(target_os = "windows")] pub fn standby_route_cleanup(route: &StandbyRoute) -> Result<StandbyCleanup, String>`
    *   **逻辑**: Windows 平台实时路由表查询，调用上述决策逻辑进行安全清理。

### 5. `monitor/outbound_night.rs`：生命周期管理与执行
夜间出站切换的完整生命周期管理，包括切换、还原、重放、看门狗、启动对账及手动命令入口。

*   **全局原子状态** (`outbound_night.rs`)
    *   `OUTBOUND_SWITCH_LAST_FAIL_MS`: 切换动作最后失败时间戳。
    *   `OUTBOUND_SWITCH_FAIL_COUNT`: 切换动作失败计数。
    *   `OUTBOUND_RESTORE_LAST_FAIL_MS`: 还原动作最后失败时间戳。
    *   `OUTBOUND_RESTORE_FAIL_COUNT`: 还原动作失败计数。
    *   `OUTBOUND_CHANNEL_FAIL_COUNT`: 提权通道失败计数。
    *   `OUTBOUND_ACTION_BUSY`: `AtomicBool`，单飞闸，防止并发冲突。
*   **结构体 `OutboundActionGuard`** (`outbound_night.rs:118-L133`)
    *   RAII 锁结构，确保互斥且 panic 安全释放。
*   **函数 `apply_outbound_switch`** (`outbound_night.rs:256-L415`)
    *   **逻辑**: 执行切换。流程：获取单飞闸 → 检查退避闸 → 选目标卡 → 读跃点快照 → **落盘快照** → 写 Metric=1 → 禁用校园网卡 → 路由验证。失败不清快照，由重放路径重试。
*   **函数 `apply_outbound_restore`** (`outbound_night.rs:446-L635`)
    *   **逻辑**: 执行还原。流程：获取单飞闸 → 解析快照 → ①清理历史兜底路由 → ②写回原跃点 → ③启用校园网卡 → 收尾清快照。支持损坏快照的降级处理（放弃阀机制）。
*   **函数 `replay_outbound_switch`** (`outbound_night.rs:335-L443`)
    *   **逻辑**: 补齐未完成的切换。分支①补写跃点，分支②补做禁用。幂等操作，无新增失败时清除切换失败计数。
*   **函数 `watchdog_re_disable_campus`** (`outbound_night.rs:417-L444`)
    *   **逻辑**: 夜间看门狗。在守护窗口内复核禁用名单，若校园网卡被外部重新启用则自动再次禁用。
*   **函数 `manual_outbound_switch`** (`outbound_night.rs:663-L680`)
    *   **逻辑**: 手动立即切换入口。覆盖退避闸，设置 `outbound_manual_hold_day` 标记。
*   **函数 `manual_outbound_restore`** (`outbound_night.rs:682-L693`)
    *   **逻辑**: 手动立即还原入口。清除 hold 标记，走标准还原流程。
*   **函数 `reconcile_outbound_on_startup`** (`outbound_night.rs:695-L735`)
    *   **逻辑**: 启动对账。残留切换态时，若在恢复窗口或功能关闭则立即还原；若在夜间窗口且目标卡可用则重放切换，否则还原。
*   **函数 `run_helper_op`** (`outbound_night.rs:216-L244`)
    *   **逻辑**: 通用提权 Helper 调用器。区分 `channel: true` (通道失败) 和 `channel: false` (业务失败)。
*   **函数 `enable_campus_adapter`** (`outbound_night.rs:246-L263`)
    *   **逻辑**: 启用网卡。桌面端走 `adapter_cache::enable_adapter` 以解决 USB 卡 PnP 禁用问题；非桌面端返回错误。
*   **函数 `write_outbound_metric`** (`outbound_night.rs`)
    *   **逻辑**: 封装 Helper 调用写 Metric=1，含通知逻辑（仅首次切换成功发通知）。
*   **函数 `needs_replay`** (`outbound_night.rs`)
    *   **逻辑**: 判断是否需要重放（切换态 + 有失败历史）。
*   **函数 `outbound_restore_alert_due`** (`outbound_night.rs`)
    *   **逻辑**: 计算是否应发送还原失败告警。
*   **函数 `notify_outbound_issue_once_per_day`** (`outbound_night.rs`)
    *   **逻辑**: 每日限频通知去重。
*   **函数 `finish_outbound_restore`** (`outbound_night.rs`)
    *   **逻辑**: 还原收尾，清快照并重置 hold 标记。
*   **函数 `clear_outbound_snapshot`** (`outbound_night.rs`)
    *   **逻辑**: 清空三份快照字段并持久化。
*   **函数 `set_outbound_manual_hold` / `clear_outbound_manual_hold`** (`outbound_night.rs`)
    *   **逻辑**: 管理“立即切换”当日保持标记。
*   **常量**:
    *   `OUTBOUND_HELPER_TIMEOUT_SECS`: `30`。
    *   `OUTBOUND_BACKOFF_START_MS`: `60,000`。
    *   `OUTBOUND_BACKOFF_MAX_MS`: `300,000`。
    *   `OUTBOUND_RESTORE_ALERT_FAILS`: `3`。
    *   `OUTBOUND_RESTORE_ALERT_REMIND_EVERY`: `20`。
    *   `OUTBOUND_RESTORE_GIVE_UP_FAILS`: `40`。

### 6. `platform/metric.rs`：Windows 平台接口跃点读取
通过 Windows API 读取接口跃点信息。

*   **结构体 `MetricRow`** (`metric.rs`)
    *   `family: u16`: IP 族 (IPv4/IPv6)。
    *   `automatic: bool`: 是否自动跃点。
    *   `metric: u32`: 跃点值。
*   **函数 `interface_rows_for_guid`** (`metric.rs`)
    *   **逻辑**: 内部函数，直接调用 Win32 API 枚举并过滤 GUID。
*   **函数 `read_interface_metrics`** (`metric.rs`)
    *   **签名**: `fn read_interface_metrics(guid: &str) -> Result<Vec<MetricRow>, String>`
    *   **逻辑**: 公开接口，转换 `MIB_IPINTERFACE_ROW` 为 `MetricRow`。使用 `AF_UNSPEC` 同时获取 IPv4 和 IPv6 栈的行。

### 7. `frontend/src/network/outboundOrder.ts`：前端排序逻辑
前端网络面板中用于拖拽排序网卡的逻辑。

*   **函数 `buildOutboundOrder`** (`outboundOrder.ts`)
    *   **签名**: `export function buildOutboundOrder(priority: string[] | undefined, detected: string[], wirelessNames?: string[]): string[]`
    *   **逻辑**:
        *   若 `priority` 非空：保留 `priority` 中存在的网卡，追加 `detected` 中剩余网卡。
        *   若 `priority` 为空但有 `wirelessNames`：无线网卡优先，其余按发现顺序。
        *   否则：直接返回 `detected` 顺序。

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