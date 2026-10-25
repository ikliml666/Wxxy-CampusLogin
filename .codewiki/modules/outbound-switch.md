---
title: 夜间出站自动切换判定（outbound_switch）
type: module
source_files:
  - tauri-app/src-tauri/src/config/outbound_switch.rs
  - tauri-app/src-tauri/src/config/night_switch.rs
tags: [夜间出站, 夜切, 纯函数, 跨平台, 时间表, config]

## Overview

`config/outbound_switch.rs` 是「夜间出站自动切换」的跨平台判定纯函数层：与运营商夜切（`night_switch`，见 [[night-operator-switch|夜间运营商切换决策]]）共用同一时间表，但动作语义正交——本模块只产出 `NightOutboundAction::Switch`/`Restore`/`None` 决策，"切到哪张卡（桌面：metric 压至 1 + 禁用校园网卡）"与"注销+重检（安卓）"由各端调用方实现。切换态由配置自身承载：`outbound_metric_restore` / `outbound_disabled_adapters` / `outbound_standby_route` 三份快照任一非空即切换态（经 `outbound_restore_active` 折算，见 `outbound_switch.rs:106`），幂等无需去重标记。

恢复时刻现已可配置：生产代码一律调用 `evaluate_night_outbound_with_restore`（`outbound_switch.rs:37`）与 `is_night_outbound_guard_window_with_restore`（`outbound_switch.rs:85`）并显式传入 `nightRestoreMinutes` 原始配置值，函数内部经 `night_switch::restore_start_minutes`（`night_switch.rs:18`）归一化（0/越界回落默认 07:30）。仅测试期保留无 `restore_start` 参数的包装 `evaluate_night_outbound`（`outbound_switch.rs:25`）与 `is_night_outbound_guard_window`（`outbound_switch.rs:73`），固定使用 `DEFAULT_RESTORE_START_MINUTES`（`night_switch.rs:11`）以保持用例可读。

六期起切换动作=写 metric + 禁校园网卡一次落盘；兜底路由自 2026-10-18 事故后不再创建，但 `outbound_restore_active` 仍将其空字符串字段纳入判定——历史残留要能被识别出来走精确清理，避免只删 metric 快照漏掉待还原动作（`outbound_switch.rs:101-105`）。守护窗口用于桌面巡检闸门：用户在切换前手动禁用的手选适配器无 IP、进不了闸门名单，名单为空时闸门失效，巡检自动启用会顶掉切换；窗口内巡检一律不出自动启用目标，还原交给恢复窗的调度（`outbound_switch.rs:63-69`）。

## Key Components

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `outbound_switch.rs:9-17` | pub enum | `NightOutboundAction { None, Switch, Restore }` | 本拍动作；`None` 含"切换态凌晨/夜间保持"或无需动作；`Switch` 桌面写 metric 快照+禁网卡、安卓注销+report_wifi_unusable 写标记；`Restore` 桌面按快照还原 metric、安卓登录清标记 |
| `outbound_switch.rs:25-33` | pub fn (`#[cfg(test)]`) | `evaluate_night_outbound(enabled, weekday, now_minutes, restore_active)` | 测试期包装：调用 `evaluate_night_outbound_with_restore` 并传入 `DEFAULT_RESTORE_START_MINUTES` (450)；生产不应调用 |
| `outbound_switch.rs:37-61` | pub fn | `evaluate_night_outbound_with_restore(enabled, weekday, now_minutes, restore_active, restore_start)` | 生产判定纯函数：先经 `restore_start_minutes` 归一化 `restore_start`；`!enabled`→`None`；`restore_active` 且 `now ∈ [restore_start, RESTORE_END_MINUTES)`→`Restore`；`restore_active` 其他→`None`；`!restore_active` 且 `now < restore_start`→`Switch`（凌晨补切前一晚窗尾）；否则 `switch_time_for(weekday)` 有值且 `now >= t`→`Switch`（过点补触发），否则 `None` |
| `outbound_switch.rs:73-80` | pub fn (`#[cfg(test)]`) | `is_night_outbound_guard_window(enabled, weekday, now_minutes)` | 测试期守护窗口包装，用默认恢复起点 450 |
| `outbound_switch.rs:85-99` | pub fn | `is_night_outbound_guard_window_with_restore(enabled, weekday, now_minutes, restore_start)` | 守护窗口判定（桌面巡检闸门）：归一化后 `!enabled`→false；`now < restore_start`→true（凌晨分支覆盖前一晚切换时刻后全部时段，跨日天然成立）；否则 `switch_time_for(weekday)` 有值且 `now >= t`→true |
| `outbound_switch.rs:106-108` | pub fn | `outbound_restore_active(metric_restore, disabled_adapters, standby_route)` | 切换态判定：三份快照（metric 快照 / 禁用名单 / 兜底路由）任一非空 → true |
| `night_switch.rs:11` | pub const | `DEFAULT_RESTORE_START_MINUTES: u32 = 450` | 恢复窗口起点默认值 07:30（含），测试包装与归一化回落用 |
| `night_switch.rs:13` | pub(crate) const | `RESTORE_END_MINUTES: u32 = 1380` | 恢复窗口终点 23:00（不含，恰为最早切换时刻，两窗口无缝衔接） |
| `night_switch.rs:18-23` | pub fn | `restore_start_minutes(configured: u32) -> u32` | 生效的恢复窗口起点：配置值 0 或 ≥终点回落默认 450，归一化幂等 |
| `night_switch.rs:37-43` | pub(crate) fn | `switch_time_for(weekday: u32) -> Option<u32>` | 时间表：weekday 0..=4 → 1380（23:00），5|6 → 1410（23:30），其余 None |

## Architecture

```text
┌──────────────────────────────┐       ┌──────────────────────────────┐
│ outbound_switch.rs           │       │ night_switch.rs              │
│                              │       │                              │
│ NightOutboundAction          │◄──────│ switch_time_for()            │
│ evaluate_night_outbound_     │ uses  │ DEFAULT_RESTORE_START_       │
│ with_restore()               │       │ MINUTES / RESTORE_END_       │
│ is_night_outbound_guard_     │       │ MINUTES / restore_start_     │
│ window_with_restore()        │       │ minutes()                    │
│ outbound_restore_active()    │       │                              │
└──────────────┬───────────────┘       └──────────────────────────────┘
               │
               │ consumed by
               ▼
┌──────────────────────────────┐
│ 各端调用方 (桌面/安卓)        │
│ 桌面: metric+禁用网卡         │
│ 安卓: 注销+重检              │
└──────────────────────────────┘
```

## Data Flow

1. **状态输入**：调用方读取配置中的三份快照（`outbound_metric_restore`, `outbound_disabled_adapters`, `outbound_standby_route`），通过 `outbound_restore_active`（`outbound_switch.rs:106`）折算为布尔值 `restore_active`。
2. **时间与配置输入**：获取当前星期几 `weekday`、当日分钟数 `now_minutes`，以及恢复起点配置值 `restore_start`（生产来自 `nightRestoreMinutes`，测试可用默认 450）。
3. **判定逻辑**（以 `evaluate_night_outbound_with_restore` `outbound_switch.rs:37` 为例）：
   - 若 `!enabled`，返回 `None`。
   - 若处于切换态 (`restore_active=true`)：检查 `now_minutes` 是否落在归一化后的恢复窗口 `[restore_start, RESTORE_END_MINUTES)` 内。若在，返回 `Restore`；否则返回 `None`（含切换态凌晨保持）。
   - 若未处于切换态 (`restore_active=false`)：
     - 若 `now_minutes < restore_start`（凌晨），返回 `Switch`（补切机制：机器在切点前后睡眠/关机错过切换时，凌晨视为前一晚切换窗尾部）。
     - 否则，查询 `switch_time_for(weekday)` 获取当日切换时刻。若返回 `Some(t)` 且 `now_minutes >= t`，返回 `Switch`（过点补触发）；否则返回 `None`。
4. **守护窗口判定**：`is_night_outbound_guard_window_with_restore`（`outbound_switch.rs:85`）返回布尔，逻辑类似但用于桌面巡检闸门——窗内抑制自动启用适配器。
5. **输出执行**：根据 `NightOutboundAction` 枚举，调用方执行相应平台操作（桌面修改 metric/禁用网卡，安卓注销账号）。

## Connections

- [[night-operator-switch|夜间运营商切换决策]]：共用 `night_switch.rs` 中的时间表常量与函数（`switch_time_for`、`RESTORE_END_MINUTES`、`DEFAULT_RESTORE_START_MINUTES`、`restore_start_minutes`）。
- [[desktop-platform]]：桌面端 metric 读取与写入接口（读 `platform::metric::read_interface_metrics`、写经 helper 提权 `SetIpInterfaceEntry`）。
- [[desktop-helper-update]]：桌面端提权执行网络配置变更。
- [[set-ip-interface-entry-metric]]：metric 设置的具体字段对照与注意事项。