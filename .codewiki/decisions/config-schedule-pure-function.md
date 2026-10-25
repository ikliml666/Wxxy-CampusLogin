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

判定收敛为**跨平台纯函数** `config/schedule.rs::should_fire_scheduled_action(now_minutes, target_minutes, last_fired_day, today_day) -> bool`（定义于 `tauri-app/src-tauri/src/config/schedule.rs` 第 22| 行），放在 `config` 模块（`tauri-app/src-tauri/src/config/mod.rs` 第 5| 行声明 `pub mod schedule;`），**无 `#[cfg]` 门控**——它随共享 crate 的跨平台可见面一起进安卓（安卓经 `campus_login_lib::config::schedule::...` 直接引用）。

判定语义（以 `tauri-app/src-tauri/src/config/schedule.rs` 第 6|–30| 行文档注释与实现为准）：

| 条件 | 结果 |
|---|---|
| `target_minutes >= SCHEDULED_DISABLED_MINUTES`（即 `>= 1440`） | 不触发（禁用哨兵，永不触发；自 2026-09-20 起 `0` 是真实的 `00:00` 时刻，禁用改用哨兵 `1440`，存量 `0` 由双端 schema 迁移一次性刷为 `1440`） |
| `now_minutes < target_minutes` | 不触发（未到点） |
| `now_minutes >= target_minutes` 且 `last_fired_day != today_day` | 触发（含过点补触发） |
| `last_fired_day == today_day` | 不触发（当日已执行） |

跨天重置由「`last_fired_day` 记录的是日期序号」天然实现——新的一天 `today_day` 变化后旧标记不再相等（`tauri-app/src-tauri/src/config/schedule.rs` 第 13|–14| 行）。

同时，校园网检测静默期判定也下沉为共享纯函数 `is_campus_check_silent(now_minutes, start, end) -> bool`（定义于 `tauri-app/src-tauri/src/config/schedule.rs` 第 41| 行），自 2026-10-26 从桌面 `monitor::campus_check` 下沉到共享面，此前同一段边界判定在桌面巡检、安卓巡检、安卓开机自启、无头 core 四处各写一份（三份逐字重复 + 一份注释声称"同语义"的私有副本）。

`is_campus_check_silent` 语义（`tauri-app/src-tauri/src/config/schedule.rs` 第 32|–49| 行）：

| 条件 | 结果 |
|---|---|
| `start == 0` | 不静默（禁用门控，含 `00:00` 与任意时刻） |
| `now_minutes < start` | 静默 |
| `end == 0` 或 `end <= start` | 只受开始时间限制（单边语义，防误设整天失效） |
| `end > start` 且 `now_minutes >= end` | 静默（双边窗口，终点不含） |

## 理由

- **纯函数 = 可测可共享**：判定与"谁调用、什么时候调用、状态存在哪"完全解耦，两端共用同一组测试（`tauri-app/src-tauri/src/config/schedule.rs` 第 51|–126| 行），边界语义不可能漂移。测试覆盖：禁用哨兵 1440、目标 00:00 到点即触发、未到点、恰好到点、过点补触发、同日不重复、跨天重置、静默期起点 0 禁用、单边语义、双边窗口。
- **放 `config` 模块而非 `monitor`**：`monitor` 是桌面专属（`#[cfg(desktop)]`），放进它安卓就不可见；`config` 是跨平台共享面（`tauri-app/src-tauri/src/config/mod.rs` 第 1|–9| 行无门控声明）。这与项目"协议/共享逻辑单点、平台能力各端自理"的分层一致。
- **过点补触发而非等值比较**：循环拍间隔、系统休眠都可能错过精确分钟；等值比较会让定时动作整天静默失效（用户配置了却没执行，且无任何提示）（`tauri-app/src-tauri/src/config/schedule.rs` 第 16|–17| 行）。
- **静默期下沉**：同一段边界判定四处重复易退化不一致，下沉后双端单点（`tauri-app/src-tauri/src/config/schedule.rs` 第 38|–40| 行）。

## 影响与约束

- 调用方（桌面 `monitor/scheduled.rs`、安卓 `monitor_loop.rs::run_scheduled_actions`）各自负责：取本地时间的分钟数与当日序号、维护 `last_fired_day` 标记、在命中时执行登录/注销。判定本身不碰任何状态（`tauri-app/src-tauri/src/config/schedule.rs` 第 22|–30| 行）。
- 标记必须**命中即置**（而非执行成功后置）：否则凭据错误/非校园网环境下会每拍重发请求。
- 禁用哨兵常量 `SCHEDULED_DISABLED_MINUTES: u16 = 1440`（`tauri-app/src-tauri/src/config/schedule.rs` 第 20| 行）双端共享单点，调用方勿再硬编码 `1440`（第 19| 行）。
- 新增任何跨平台的判定/计算逻辑，优先考虑这种"纯函数放 config/infra 共享面"的形态，而不是在两端各写一份。

## Connections

[[scheduled-actions-outside-silent-window]]、[[protocol-core-single-source]]、[[config-field-sets-bidirectional-sync]]
