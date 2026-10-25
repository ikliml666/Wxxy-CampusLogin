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

**改名说明（2026-09-22）**：本功能原 UI 名「晚间断网自动切换」，随「夜间出站自动切换」功能落地而更名为「自动切换运营商」——整体断网场景由出站切换接管，本功能只覆盖运营商线路断网，名字与语义对齐。**仅改文案与本文档标题，配置字段名 `enableNightOperatorSwitch` 等一律不变**（兼容性）；时间表与下文 [[night-outbound-switch]] 共用。

校园网（无锡学院）的运营商线路有固定断网规律：**周日至周四 23:00 与周五、周六 23:30** 之后，电信/移动/联通服务下线无法上网（需求方更正时间表见 `night_switch.rs:1-7` 模块注释与 `switch_time_for` 实现 `night_switch.rs:26-32`：`0..=4 => Some(1380)`、`5 | 6 => Some(1410)`），需把登录运营商切到"无锡学院"（`config.operator = ''`）才能继续上网；次日早晨线路恢复后应切回原运营商。形态为**自动定时切换**：一个开关（默认关，见 `model.rs:269-271` `Config::default()` 中 `enable_night_operator_switch: false`，注释指明存量落盘 true 不迁移）与前端 `types.ts:36-39`，开启后到点自动切换，无需手动。

## 设计

### 配置字段（桌面与前端）

| 字段 | 默认 | 含义 |
| --- | --- | --- |
| `enableNightOperatorSwitch` | `false`（2026-10-02 起默认改回关闭；`model.rs:271` `Config::default()` 中 `enable_night_operator_switch: false`，字段定义见 `model.rs:56-57`，注释指明存量落盘 true 不迁移） | 夜切总开关 |
| `nightOperatorRestore` | `''`（字段定义 `model.rs:59-60`，`Config::default()` 中赋值见 `model.rs:59`） | 切至无锡学院前暂存的原运营商；非空即"处于切换态" |

- 桌面字段定义：`config/model.rs` `enable_night_operator_switch`（serde rename `enableNightOperatorSwitch`，`default` 属性见 `model.rs:56-57`）、`night_operator_restore`（rename `nightOperatorRestore`，`model.rs:59-60`）；`Config::default()` 中相应赋值见 `model.rs:271-272`。前端镜像 `frontend/src/settings/types.ts` `enableNightOperatorSwitch: boolean`（`types.ts:37` 字段）、`nightOperatorRestore: string`（`types.ts:39` 字段）。
- 字段校验与迁移：`config/validate.rs` 中 `validate_operator`（`validate.rs:23-29`）限定合法值为 `""`、`@telecom`、`@unicom`、`@cmcc`；`migrate_operator`（`validate.rs:73-79`）将旧值 `@ctcc`/`@cucc` 迁移为 `@telecom`/`@unicom`。`validate_config` 内含 v4→v5 迁移块（`validate.rs:163-174`）：若 `config_version < 5` 且 `!enable_night_operator_switch` 则强制置 true（历史默认变更，显式 false 会被误刷，先例语义），但当前 `Config::default()` 的 `config_version` 已为 5（`model.rs:317`），新配置不触发。测试锁定：桌面 `validate.rs:648` 断言默认关（注释 `"夜切默认关闭(2026-10-02)"`），`validate.rs:654-673` 测试 v4→v5 迁移刷开且用户显式值不被覆盖（该测试构造 `config_version=4` 验证迁移）。

### 时间表与判定（共享纯函数）

判定集中在桌面 crate 跨平台模块 `config/night_switch.rs::evaluate_night_switch(enabled, weekday, now_minutes, current_operator, restore_operator)`（`night_switch.rs:51-73`），纯函数无副作用：

- 切换时刻：`switch_time_for(weekday)`（`night_switch.rs:26-32`）——weekday（`0=周日 … 6=周六`）`0..=4` → `Some(1380)` 分钟（23:00）、`5 | 6` → `Some(1410)` 分钟（23:30），每天均有切换时刻。
- 恢复窗口：常量 `RESTORE_START_MINUTES = 450`（`night_switch.rs:10`）、`RESTORE_END_MINUTES = 1380`（`night_switch.rs:12`），每日 `[450, 1380)`（07:30 起，至 23:00 不含，与切换时刻无缝衔接）。
- 切换条件：开关开 + 当日有切换时刻 + `now_minutes >= 时刻` + **当前 operator 在白名单**（`is_switchable_operator` `night_switch.rs:37-39`：`@telecom`/`@unicom`/`@cmcc`）→ `NightSwitchAction::SwitchToCampus`（调用方应将当前值写入 restore、operator 置空并登录）。
- 恢复条件：开关开 + 恢复窗口内 + `restore_operator` 非空 + 当前 `current_operator` 为空 → `NightSwitchAction::Restore`（调用方取回并清空 restore）。
- 白名单拦截账号档案脏值（如 `"校园"` 或非白名单后缀），见 `night_switch.rs:34-39` 注释与 `evaluate_night_switch` 逻辑 `night_switch.rs:67-69`。
- 辅助纯函数：`parse_chkstatus`（`night_switch.rs:91-104`，剥 JSONP 壳解析 eportal 在线表 `ChkStatusInfo` 结构体 `night_switch.rs:78-85`）与 `uid_matches`（`night_switch.rs:108-110`，学号+运营商后缀精确比对）供双端验证链复用；内联测试 `night_switch.rs:112-291` 锁定窗口边界、白名单、幂等与 chkstatus 样例。出站切换模块 `config/outbound_switch.rs:7` 直接 import 本模块的时间表常量与 `switch_time_for`，测试 `outbound_switch.rs:96-103` 锁定 1380/1410 时间表共用。

### 幂等：切换态由配置自身承载

不引入"当日已触发"去重标记：依据 `night_switch.rs:5-7` 与 `evaluate_night_switch` 实现 `night_switch.rs:61-72`，`restore_operator` 非空 + `current_operator` 空 = 已切换态，纯函数重复判定自然返回 `None`；调用方在恢复成功后清空 restore 使下一拍回到 `None`，保证写配置只发生一次（测试 `night_switch.rs:207-215` 模拟）。比定时登录的当日标记更简单，且跨天正确。

### 接入点（动作语义一致，调用方未含于本次源码）

`evaluate_night_switch` 仅产出 `NightSwitchAction` 枚举（`night_switch.rs:14-22`），具体落盘与登录由调用方（各端 monitor 循环）实现。桌面应在定时循环中以 tick 调用，依 `SwitchToCampus`/`Restore` 改 `operator`/`night_operator_restore` 并触发登录；安卓端复用同一纯函数（未在本次提供源码中）。注意：运营商夜切**没有**出站切换那样的凌晨补切逻辑（对比 `outbound_switch.rs:98-100` 的 `now_minutes < RESTORE_START_MINUTES` 补切），其判定严格依赖 `switch_time_for` 与恢复窗口。

### UI 呈现（夜切临时态徽标）

前端 `types.ts` 暴露 `operator`（`types.ts:22`）与 `nightOperatorRestore`（`types.ts:39`），UI 可据此检测 `operator === '' && nightOperatorRestore !== ''` 显示临时态。具体组件未含于本次源码。

### 配套修复

本次提供源码中，`model.rs:326-337` 的 `masked_for_display`/`mask_in_place` 确保配置向外发送时密码掩码，避免明文泄露，与夜切配置广播相关。历史安卓广播修复等未含于给定文件。

## 已知边界（有意不修）

- **午夜后不补触发**：依据 `night_switch.rs:58-72`，00:00~07:30 之间启动 App 不会切换（切换条件需 `now_minutes >= switch_at` 且 `switch_at` 为 1380/1410；恢复条件需窗口 `[450, 1380)`），若未处于切换态则无任何动作，07:30 恢复窗口自愈（若已切换态则窗口内恢复）。影响极小，不引入跨天状态。出站切换 `outbound_switch.rs:37-39` 有凌晨补切，但运营商夜切 intentionally 无。
- **桌面 `autoExitAfterLogin` 默认已为 false**（`model.rs:36-37` 字段定义，`Config::default()` 行258-260 注释 2026-09-20 起默认关闭），夜切后进程常驻，恢复不被推迟。存量显式 true 不迁移。
- **功能依赖调用方循环**：若各端 monitor 循环未启动，则判定不运行。

## Connections

[[config-schedule-pure-function]]、[[scheduled-actions-outside-silent-window]]、[[dual-platform-sharing]]、[[config-field-sets-bidirectional-sync]]、[[night-outbound-switch]]、[[lightweight-mode-desktop]]、[[learnings/gen-schemas-crlf-diff-noise]]
