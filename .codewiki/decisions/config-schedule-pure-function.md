---
title: "定时动作判定收敛为跨平台纯函数单点共享"
type: decision
source_files:
  - tauri-app/src-tauri/src/config/schedule.rs
  - tauri-app/src-tauri/src/config/mod.rs
tags: [决策, 定时动作, 纯函数, 双端, 单点共享]
---

## 背景

「每日定时登录 / 定时注销」双端都要判定"这一拍该不该触发"。两端循环节奏不同（桌面 30s 独立循环、安卓每拍巡检），若各写一份判定，语义漂移几乎必然——尤其"过点补触发""当日去重""禁用值"这三条边界，两端任何一处理解不同就会表现为"安卓能触发桌面不能"这类难查的行为分叉。

## 决策

判定收敛为**跨平台纯函数** `config/schedule.rs::should_fire_scheduled_action(now_minutes, target_minutes, last_fired_day, today_day) -> bool`，放在 `config` 模块（`config/mod.rs` 声明 `pub mod schedule;`），**无 `#[cfg]` 门控**——它随共享 crate 的跨平台可见面一起进安卓（安卓经 `campus_login_lib::config::schedule::...` 直接引用）。

判定语义：

| 条件 | 结果 |
|---|---|
| `target_minutes == 0` | 不触发（禁用，与 `campus_check_start_minutes` 的 0=禁用约定一致） |
| `now_minutes < target_minutes` | 不触发（未到点） |
| `now_minutes >= target_minutes` 且 `last_fired_day != today_day` | 触发（含过点补触发） |
| `last_fired_day == today_day` | 不触发（当日已执行） |

跨天重置由「`last_fired_day` 记录的是日期序号」天然实现——新的一天 `today_day` 变化后旧标记不再相等。

## 理由

- **纯函数 = 可测可共享**：判定与"谁调用、什么时候调用、状态存在哪"完全解耦，两端共用同一组测试，边界语义不可能漂移。测试覆盖：禁用值 0、未到点、恰好到点、过点补触发、同日不重复、跨天重置。
- **放 `config` 模块而非 `monitor`**：`monitor` 是桌面专属（`#[cfg(desktop)]`），放进它安卓就不可见；`config` 是跨平台共享面。这与项目"协议/共享逻辑单点、平台能力各端自理"的分层一致。
- **过点补触发而非等值比较**：循环拍间隔、系统休眠都可能错过精确分钟；等值比较会让定时动作整天静默失效（用户配置了却没执行，且无任何提示）。

## 影响与约束

- 调用方（桌面 `monitor/scheduled.rs`、安卓 `monitor_loop.rs::run_scheduled_actions`）各自负责：取本地时间的分钟数与当日序号、维护 `last_fired_day` 标记、在命中时执行登录/注销。判定本身不碰任何状态。
- 标记必须**命中即置**（而非执行成功后置）：否则凭据错误/非校园网环境下会每拍重发请求。
- 新增任何跨平台的判定/计算逻辑，优先考虑这种"纯函数放 config/infra 共享面"的形态，而不是在两端各写一份。

## Connections

[[scheduled-actions-outside-silent-window]]、[[protocol-core-single-source]]、[[config-field-sets-bidirectional-sync]]
