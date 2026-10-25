---
title: "定时动作判定置于静默期与巡检分档之外"
type: decision
source_files:
  - tauri-app/src-tauri/src/config/schedule.rs
  - tauri-app/src-tauri/src/monitor/watcher.rs
tags: [决策, 定时动作, 静默期, 双端, 调度]
---

## 背景

新增「每日定时登录 / 定时注销」时，最自然的落点是复用既有每拍巡检循环（桌面 `run_check_once`、安卓 `monitor_tick_loop` 内的 `run_check_once`）。但既有巡检有两道"省电/省流"闸门：

1. **校园网检测静默期**（`campus_check_start_minutes` / `campus_check_end_minutes`）：时段外整拍跳过校园网验证。该判定现由共享面纯函数 `config::schedule::is_campus_check_silent` 承载（`tauri-app/src-tauri/src/config/schedule.rs:41-49`，于 2026-10-26 从桌面 `monitor::campus_check` 下沉到共享面，见 `schedule.rs:38-40`）。
2. **安卓巡检分档**（`effective_interval_ms`）：亮屏+WiFi 才用基础间隔，否则取 `idle_ms.max(base)`；跳拍判定在 `monitor_tick_loop`。

静默期判定虽下沉至共享模块，但作为巡检闸门的本质未变；定时动作判定不调用该函数，故依然独立。

## 决策

定时动作的判定**独立于上述两道闸门**：

- **桌面**用独立循环 `monitor::scheduled::run_scheduled_action_loop`，由 `run_startup_tasks` 无条件 spawn（`tauri-app/src-tauri/src/monitor/watcher.rs:16-20`），其前方注释明确不受 `enable_background_check` 开关管（`watcher.rs:64-66`），"定时动作与巡检是两个功能"。循环内部逐拍读配置，判定由共享纯函数完成。
- **安卓**与桌面共用同一判定实现：模块 `config::schedule` 在跨平台可见面（`lib.rs` 无 cfg 门控），桌面 `monitor::scheduled` 与安卓 `monitor_loop` 共用同一实现与同一组测试（`tauri-app/src-tauri/src/config/schedule.rs:4-5`）。因此安卓侧同样将定时动作判定置于静默期与分档跳拍之外，语义单点一致。
- 两端判定共用同一纯函数 `config::schedule::should_fire_scheduled_action`（`tauri-app/src-tauri/src/config/schedule.rs:22-30`），其文档注释声明跨平台共用、禁用哨兵与过点补触发语义。
- 静默期函数 `is_campus_check_silent`（`schedule.rs:41-49`）虽同在本模块，但定时动作判定函数不调用它，二者解耦。

（旧文提及定时动作循环"同时承载夜间出站切换编排"，因相关文件 `scheduled.rs` 未在本轮提供源码，该细节暂不重写，以当前提供源码为准仅确认独立循环与判定纯函数。）

## 理由

定时动作的典型用户场景恰恰落在这些闸门覆盖的时段内——例如「23:30 自动注销」天然处于夜间（静默期 + 灭屏闲时档）。若把判定放进 `run_check_once`：

- 静默期命中 → 整拍 return，定时注销当天静默失效，用户看不到任何反馈
- 灭屏跳拍 → 拍点间隔从基础值拉长到闲时档级，虽有过点补触发兜底，但失效窗口被放大

定时动作是**用户显式配置的一次性时点行为**，与"省电省流"目标不冲突（判定本身是纯内存计算，不产生网络请求——只有命中时才走一次登录/注销）。因此它不应被巡检的节流闸门约束。共享纯函数 `should_fire_scheduled_action` 的判定成本极低，且禁用哨兵 `target_minutes >= SCHEDULED_DISABLED_MINUTES`（1440）时直接返回 false（`schedule.rs:23-25`），无任何副作用。

## 影响与约束

- **判定纯函数与禁用哨兵**：`pub const SCHEDULED_DISABLED_MINUTES: u16 = 1440;`（`schedule.rs:20`）。`pub fn should_fire_scheduled_action(now_minutes: u16, target_minutes: u16, last_fired_day: i32, today_day: i32) -> bool`（`schedule.rs:22-30`）：
  - `target_minutes >= SCHEDULED_DISABLED_MINUTES` 禁用哨兵永不触发（`schedule.rs:23-25`；2026-09-20 起 0 表示真实的 00:00 时刻、禁用改用哨兵 1440，存量 0 由双端 schema 迁移一次性刷为 1440，`schedule.rs:7-8,18-19`）；
  - `now_minutes < target_minutes` 未到点不触发（`schedule.rs:26-28`）；
  - 已到点（`now_minutes >= target_minutes`，含过点补触发）且 `last_fired_day != today_day` 即触发（`schedule.rs:29`，跨天重置靠日期序号天然实现，`schedule.rs:13-14`）。
- **过点补触发**是配套设计：`now_minutes >= target_minutes` 即触发（而非等值比较），因为循环拍间隔、系统休眠都可能错过精确分钟；等值比较会让功能整天静默失效（`schedule.rs:16-17`）。
- **当日去重**靠日期序号标记：`last_fired_day == today_day` 表示当日已执行过，不重复触发（`schedule.rs:10-11,29`）。调用方（桌面 `monitor::scheduled` / 安卓 `monitor_loop`）需在触发后写入 `last_fired_day`，本模块不负责存储。
- **静默期门控无关**：`pub fn is_campus_check_silent(now_minutes: u16, start: u16, end: u16) -> bool`（`schedule.rs:41-49`）负责巡检静默期判定，其规则为 `start == 0` 禁用门控（`schedule.rs:42-44`）、`end <= start` 退化为单边（`schedule.rs:45-48` 及注释 `schedule.rs:34-36`）、`end > start` 为双边窗口（`schedule.rs:48`）。该函数与 `should_fire_scheduled_action` 并列，定时动作不调用它，故不受静默期影响。
- **桌面启动编排**：`pub fn run_startup_tasks(app_handle: &AppHandle)`（`watcher.rs:16`）在启动阶段注册跟踪任务：后台检测（`watcher.rs:21-31`）、启动即测一次（`watcher.rs:36-43`）、定时测试循环（`watcher.rs:45-55`）、启动自动登录（`watcher.rs:57-62`）、以及独立的 `scheduled_actions` 循环（`watcher.rs:67-71`）。定时动作循环与 `enable_background_check` 开关无关，仅依赖自身配置目标值；注释提及"两目标均 0=禁用时零开销"应与 `schedule.rs` 中 0 为真实 00:00 且禁用用 1440 的迁移语义对照理解——判定逻辑以 `should_fire_scheduled_action` 为准。
- **测试同文件**：`schedule.rs:51-126` 包含 `mod tests`，覆盖 `is_campus_check_silent`（起点0禁用 `schedule.rs:59-65`、单边语义 `schedule.rs:67-75`、双边窗口 `schedule.rs:77-84`）与 `should_fire_scheduled_action`（禁用哨兵1440 `schedule.rs:86-91`、目标00点0分即触发 `schedule.rs:93-99`、未到点不触发 `schedule.rs:101-105`、到点当日未执行触发 `schedule.rs:107-113`、同日已执行不重复 `schedule.rs:115-119`、跨天重置 `schedule.rs:121-125`）。
- **双端单点**：模块注释明确无 cfg 门控、桌面与安卓共用实现与测试（`schedule.rs:3-4`）；静默期函数亦从桌面巡检、安卓巡检、安卓开机自启、无头 core 四处重复下沉为单点（`schedule.rs:38-40`）。

## Connections

[[config-schedule-pure-function]]、[[background-check-and-auto-login]]、[[deliberate-background-check-skips]]、[[config-field-sets-bidirectional-sync]]
