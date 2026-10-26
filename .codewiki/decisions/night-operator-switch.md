---
title: "自动切换运营商（原晚间断网自动切换）"
type: decision
source_files:
  - tauri-app/src-tauri/src/config/night_switch.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/validate.rs
  - tauri-app/src-tauri/src/config/outbound_switch.rs
  - tauri-app/frontend/src/settings/types.ts
tags: [决策, 夜切, 运营商, 定时任务, 双端同构]
---

## 背景

**改名说明（2026-09-22）**：本功能原 UI 名「晚间断网自动切换」，随「夜间出站自动切换」功能落地而更名为「自动切换运营商」——整体断网场景由出站切换接管，本功能只覆盖运营商线路断网，名字与语义对齐。**仅改文案与本文档标题，配置字段名 `enableNightOperatorSwitch` 等一律不变**（兼容性）；时间表与 `config/outbound_switch.rs` 共用（见 `night_switch.rs:3-7` 模块注释与 `outbound_switch.rs:7` 导入 `switch_time_for`）。

校园网（无锡学院）的运营商线路有固定断网规律：**周日至周四 23:00 与周五、周六 23:30** 之后，电信/移动/联通服务下线无法上网（需求方更正时间表见 `night_switch.rs:3-7` 模块注释与 `switch_time_for` 实现 `night_switch.rs:37-43`：`0..=4 => Some(1380)`、`5 | 6 => Some(1410)`），需把登录运营商切到"无锡学院"（`config.operator = ''`，即空串，见 `night_switch.rs:5-7` 切换态定义）才能继续上网；次日早晨线路恢复后应切回原运营商。形态为**自动定时切换**：一个开关（默认关，见 `model.rs:287-289` `Config::default()` 中 `enable_night_operator_switch: false`，注释 `model.rs:287` 指明 2026-10-02 起默认改回关闭、存量落盘 true 不迁移）与前端 `types.ts:36-37` `enableNightOperatorSwitch: boolean`，开启后到点自动切换，无需手动。

## 设计

### 配置字段（桌面与前端）

| 字段 | 默认 | 含义 |
| --- | --- | --- |
| `enableNightOperatorSwitch` | `false`（2026-10-02 起默认改回关闭；`model.rs:56-57` 字段定义，`model.rs:56-58` `Config::default()` 赋值，注释 `model.rs:56` 指明存量落盘 true 不迁移） | 夜切总开关 |
| `nightOperatorRestore` | `''`（`model.rs:59-60` 字段定义，`model.rs:59` `Config::default()` 赋值） | 切至无锡学院前暂存的原运营商；非空即"处于切换态" |
| `nightRestoreMinutes` | `0`（`model.rs:69-70` 字段定义，`model.rs:69` `Config::default()` 赋值；0 或越界回落默认 07:30，见 `night_switch.rs:18-23` `restore_start_minutes`） | 夜间切换恢复时刻（当日分钟数，如 450=07:30）；恢复窗自此刻开启，把运营商切回。2026-10-26 起可配置（`night_switch.rs:9-10`、`model.rs:69-72`） |

- 桌面字段定义：`config/model.rs` `enable_night_operator_switch`（serde rename `enableNightOperatorSwitch`，`model.rs:56-57`）、`night_operator_restore`（rename `nightOperatorRestore`，`model.rs:59-60`）、`night_restore_minutes`（rename `nightRestoreMinutes`，`model.rs:69-70`）；`Config::default()` 中相应赋值见 `model.rs:289-290, 293`。前端镜像 `frontend/src/settings/types.ts` `enableNightOperatorSwitch: boolean`（`types.ts:37`）、`nightOperatorRestore: string`（`types.ts:39`）、`nightRestoreMinutes: number`（`types.ts:42`）。
- 字段校验与迁移：`config/validate.rs` 中 `validate_operator`（`validate.rs:23-29`）限定合法值为 `""`、`@telecom`、`@unicom`、`@cmcc`；`migrate_operator`（`validate.rs:73-79`）将旧值 `@ctcc`/`@cucc` 迁移为 `@telecom`/`@unicom`。`validate_config` 内含 v4→v5 迁移块（`validate.rs:163-174`）：若 `config_version < 5` 且 `!enable_night_operator_switch` 则强制置 true（历史默认变更，显式 false 会被误刷，先例语义），但当前 `Config::default()` 的 `config_version` 已为 5（`model.rs:341`），新配置不触发。测试锁定：桌面 `validate.rs:648` 断言默认关（注释 `"夜切默认关闭(2026-10-02)"`），`validate.rs:654-673` 测试 v4→v5 迁移刷开且用户显式值不被覆盖（该测试构造 `config_version=4` 验证迁移）。

### 时间表与判定（共享纯函数）

判定集中在桌面 crate 跨平台模块 `config/night_switch.rs::evaluate_night_switch_with_restore(enabled, weekday, now_minutes, current_operator, restore_operator, restore_start)`（`night_switch.rs:90-114`），生产代码调用此函数并显式传入 `nightRestoreMinutes` 配置值；测试期包装 `evaluate_night_switch`（`night_switch.rs:66-82`，`#[cfg(test)]`）固定传默认 450。纯函数无副作用。

公开常量与函数清单（均位于 `night_switch.rs`）：

- `DEFAULT_RESTORE_START_MINUTES: u32 = 450`（`night_switch.rs:9-11`）：恢复窗口起点默认值 07:30（含）。2026-10-26 起可经 `nightRestoreMinutes` 配置覆盖。
- `RESTORE_END_MINUTES: u32 = 1380`（`night_switch.rs:12-13`，`pub(crate)`）：恢复窗口终点 23:00（不含，恰为最早切换时刻，两窗口无缝衔接）。
- `restore_start_minutes(configured: u32) -> u32`（`night_switch.rs:18-23`）：生效的恢复窗口起点。配置值 0（未设置/旧配置）或 ≥ `RESTORE_END_MINUTES` 回落默认 450；否则原样透传。幂等。
- `NightSwitchAction` 枚举（`night_switch.rs:25-33`）：`None` / `SwitchToCampus`（暂存 operator 进 `night_operator_restore`、`operator` 置空串并触发登录）/ `Restore`（operator 恢复为暂存值、restore 清空并触发登录）。
- `switch_time_for(weekday: u32) -> Option<u32>`（`night_switch.rs:37-43`）：当日切换时刻。weekday（`0=周日 … 6=周六`）`0..=4` → `Some(1380)` 分钟（23:00）、`5 | 6` → `Some(1410)` 分钟（23:30），其它 → `None`。
- `is_switchable_operator(operator: &str) -> bool`（`night_switch.rs:48-50`）：白名单 `@telecom`/`@unicom`/`@cmcc`；空串（已切无锡学院）或非白名单脏值（如 `"校园"`、`@foo`）均返回 false。
- `evaluate_night_switch_with_restore(...)`（`night_switch.rs:90-114`）：带自定义恢复起点的判定。逻辑：
  - `night_switch.rs:98`：先经 `restore_start_minutes` 归一化 `restore_start`。
  - `night_switch.rs:99-101`：`!enabled` → `None`。
  - `night_switch.rs:102-107`：恢复条件——`!restore_operator.is_empty()`（处于切换态）且 `current_operator.is_empty()`（确实在无锡学院态，用户手动切回的场景不误恢复）且 `now_minutes` 落在 `[restore_start, RESTORE_END_MINUTES)` → `Restore`。
  - `night_switch.rs:108-112`：切换条件——当日有切换时刻（`switch_time_for` 返回 `Some`）且 `now_minutes >= 时刻`（含过点补触发）且 `is_switchable_operator(current_operator)` 为真 → `SwitchToCampus`。
  - `night_switch.rs:113`：其余 → `None`。
- `evaluate_night_switch(...)`（`night_switch.rs:98-114`，`#[cfg(test)]`）：测试包装，固定传 `DEFAULT_RESTORE_START_MINUTES` 作 `restore_start`，仅为让既有用例可读；生产禁用。
- `ChkStatusInfo` 结构体（`night_switch.rs:119-126`）：eportal 在线状态查询结果，`online: bool`、`uid: String`（学号+运营商后缀，离线/缺失为空串）、`oltime: Option<i64>`（会话在线秒数）。
- `parse_chkstatus(body: &str) -> Option<ChkStatusInfo>`（`night_switch.rs:132-145`）：剥 JSONP 壳（`dr1003(...)`，带 BOM/空白容错）解析，`online = result==1`，任何输入不 panic。
- `uid_matches(uid: &str, user: &str, expected_operator: &str) -> bool`（`night_switch.rs:149-151`）：uid trim、忽略 ASCII 大小写后与 `学号 + 期望运营商后缀` 精确相等。

出站切换模块 `config/outbound_switch.rs:7` 直接 import 本模块的 `switch_time_for` 与 `RESTORE_END_MINUTES`，测试 `outbound_switch.rs:197-203` 锁定 1380/1410 时间表共用；`outbound_switch.rs:181-194` 测试自定义恢复起点驱动 Restore、`outbound_switch.rs:251-286` 测试预判定窗口同样复用 `restore_start_minutes`。

### 架构与数据流

```text
┌─────────────┐   ┌────────────────────────┐   ┌────────────────┐
│ Config      │   │ evaluate_night_switch_ │   │ 调用方 monitor │
│ model.rs    │──►│ with_restore           │──►│ 循环(改op/登录)│
│ enableSwitch│   │ night_switch.rs:90     │   └───────┬────────┘
│ operator    │   └────────────────────────┘           │切换态由配置
│ restore     │                                        ▼
│ nightRestore│                              ┌────────────────┐
└──────┬──────┘                              │restore非空+op空│
       │共享时间表                           │ =已切换态(幂等)│
       ▼                                     └────────────────┘
switch_time_for(night_switch.rs:37) ◄────► outbound_switch.rs:7
```

### 幂等：切换态由配置自身承载

不引入"当日已触发"去重标记：依据 `night_switch.rs:5-7` 与 `evaluate_night_switch_with_restore` 实现 `night_switch.rs:98-113`，`restore_operator` 非空 + `current_operator` 空 = 已切换态，纯函数重复判定自然返回 `None`；调用方在恢复成功后清空 restore 使下一拍回到 `None`，保证写配置只发生一次（测试 `night_switch.rs:284-291` 模拟"窗口内每拍持续判定为恢复、幂等不重复写配置"）。比定时登录的当日标记更简单，且跨天正确。

### 接入点（动作语义一致，调用方未含于本次源码）

`evaluate_night_switch_with_restore` 仅产出 `NightSwitchAction` 枚举（`night_switch.rs:25-33`），具体落盘与登录由调用方（各端 monitor 循环）实现。桌面应在定时循环中以 tick 调用，依 `SwitchToCampus`/`Restore` 改 `operator`/`night_operator_restore` 并触发登录；安卓端复用同一纯函数（未在本次提供源码中）。注意：运营商夜切**没有**出站切换那样的凌晨补切逻辑（对比 `outbound_switch.rs:198-200` 的 `now_minutes < restore_start` 补切分支，以及 `outbound_switch.rs:198-210` 测试 `predawn_missed_switch_is_replayed`），其判定严格依赖 `switch_time_for` 与恢复窗口（`night_switch.rs:90-114`）。

### UI 呈现（夜切临时态徽标）

前端 `types.ts` 暴露 `operator`（`types.ts:22`）与 `nightOperatorRestore`（`types.ts:39`），UI 可据此检测 `operator === '' && nightOperatorRestore !== ''` 显示临时态。具体组件未含于本次源码。

### 配套修复

本次提供源码中，`model.rs:350-365` 的 `masked_for_display`/`mask_in_place` 确保配置向外发送时密码与 VPN 密文掩码，避免明文泄露，与夜切配置广播相关。历史安卓广播修复等未含于给定文件。

## 已知边界（有意不修）

- **午夜后不补触发**：依据 `night_switch.rs:90-114`，00:00~07:30 之间启动 App 不会切换（切换条件需 `now_minutes >= switch_at` 且 `switch_at` 为 1380/1410；恢复条件需窗口 `[restore_start, 1380)`），若未处于切换态则无任何动作，07:30 恢复窗口自愈（若已切换态则窗口内恢复）。影响极小，不引入跨天状态。出站切换 `outbound_switch.rs:37-61` 有凌晨补切（其 `evaluate_night_outbound_with_restore` 在 `now_minutes < restore_start` 时直接 `Switch`，见 `outbound_switch.rs:54-56`），但运营商夜切 intentionally 无。
- **桌面 `autoExitAfterLogin` 默认已为 false**（`model.rs:36-37` 字段定义，`model.rs:36` `Config::default()` 赋值，注释 `model.rs:36-37` 说明 2026-09-20 起默认关闭，配合夜切进程常驻），夜切后进程常驻，恢复不被推迟。存量显式 true 不迁移。
- **功能依赖调用方循环**：若各端 monitor 循环未启动，则判定不运行。
- **恢复时刻可配置但越界回落**：`nightRestoreMinutes` 若设 ≥1380 或 0，经 `restore_start_minutes`（`night_switch.rs:18-23`）回落默认 450，避免恢复窗为空永不还原（配置错误而非用户意图，见 `night_switch.rs:15-17` 注释）。

## Connections

[[config-schedule-pure-function]]、[[scheduled-actions-outside-silent-window]]、[[dual-platform-sharing]]、[[config-field-sets-bidirectional-sync]]、[[night-outbound-switch]]、[[lightweight-mode-desktop]]、[[learnings/gen-schemas-crlf-diff-noise]]
