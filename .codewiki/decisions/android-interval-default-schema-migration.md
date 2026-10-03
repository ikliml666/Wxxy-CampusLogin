---
title: 安卓后台检测间隔默认 15s→60s + config_schema_version 迁移机制
type: decision
source_files:
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/frontend/src/settings/constants.ts
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/validate.rs
tags: [决策, 安卓, 配置, 默认值, 迁移]
---

## 背景

安卓稳态周期任务按 15s 运行属空转耗电。

## 决策

2026-09-09：后台检测间隔默认值 **15s → 60s**（`config_state.rs:138`），并引入 `config_schema_version` 做**一次性默认值迁移**（`config_state.rs:314-366` `migrate_legacy_defaults`）。

迁移语义：只把**等于旧默认值**的字段刷为新默认（用户显式设过的其他值不动），迁移结果（含版本号）落盘后用户显式设回**不再覆盖**；落盘失败静默、下次读盘重迁幂等（`config_state.rs:312-313`）。后续默认值变更沿用该机制。

2026-09-13：schema **v3→v4** —— 新增**闲时巡检间隔** `background_check_idle_interval`（默认 300_000，`config_state.rs:140`；旧文件缺该字段反序列化为 0，迁移里补 300_000，`config_state.rs:332-338`）。同时间隔改为**按网络类型/屏幕状态分档**：亮屏 + WiFi 走 `background_check_interval`（60s），蜂窝或灭屏走闲时间隔（5min，闲时间隔小于基础间隔时取基础间隔）——判定为纯函数 `effective_interval_ms(base_ms, idle_ms, screen_on, wifi_connected)`（`monitor_loop.rs:83-90`，基础间隔下限 5s）。循环机制：tokio tick 恒按基础间隔唤醒（间隔热更新 `monitor_loop.rs:596-601`），每拍经 `power_state` 查插件 `get_power_state`（`monitor_loop.rs:568-573`，查询失败按保守值 `(true, true)` —— 宁多检不漏检）取 `(screen_on, wifi_connected)`，算出 `effective` 后按 `last_probe_ms` 距今是否不足 `effective` 决定跳拍与否（`monitor_loop.rs:602-611`）——即"唤醒节奏恒为基础间隔，探针执行频率才是分档对象"。两档间隔在 `start_background_check` 一起读入 `MONITOR.desired_interval_ms` / `MONITOR.idle_interval_ms`（`monitor_loop.rs:195-200`，字段 `monitor_loop.rs:39-41`）。新装即当前版本，跳过迁移。

同日追加 schema **v4→v5**：**检测时段终点** `campus_check_end_minutes` 旧默认 `0`（仅开始时间限制）→ `1380`（23:00，`config_state.rs:171`），迁移把存量等于旧默认 0 的值一并刷为新默认（`config_state.rs:339-345`；测试 `config_state.rs:649` 断言迁移生效、`:660-664` 断言迁移后用户显式设回 0 不被二次覆盖）。**桌面侧同日做了同义迁移**（`config_version` v2→v3）：serde `default = "default_campus_check_end_minutes"` 与 `Config::default` 均为 1380（`model.rs:156`、`:208`、`:298`），`validate_config` 内 `config_version < 3` 时把等于旧默认 0 的终点刷为 1380（`validate.rs:141-145`，测试 `validate.rs:588-602`）——桌面此前没有版本化默认值迁移机制，这次是第一次补第二段（v1→v2 之外），语义与安卓 schema 迁移对齐但编号体系独立。

2026-09-20 起机制两端继续复用（编号各自 +1 对齐）：

- 安卓 **v5→v6**：质量测试间隔旧默认 60s→600s（`config_state.rs:153`、迁移 `:346-352`；专项测试 `:685-713`）。
- 安卓 **v6→v7**：夜切开关旧默认 false→true（上线一天即改默认）；定时登录/注销禁用值 0→1440（0 变为真实的 00:00 时刻，禁用改用哨兵 1440）（迁移 `config_state.rs:353-365`）。新装 `config_schema_version: 7`（`config_state.rs:180`）。注意 2026-10-02 起新装默认又改回 false（`config_state.rs:125`），按"只前迁不回迁"原则存量已刷开的 true 不动；v6 旧文件此后升级仍会被 v6→v7 刷为 true，属链的既定语义。
- 桌面 **v3→v4**：`background_check_interval` 旧默认 15000→60000、质量间隔 60000→600000（`model.rs:260`、`:282`，迁移 `validate.rs:151-158`，测试 `validate.rs:605-623`）——**两端基础间隔自此同值 60000**。
- 桌面 **v4→v5**：夜切开关 false→true + 定时禁用 0→1440 哨兵（`model.rs:301`、`:308` `config_version: 5`，迁移 `validate.rs:163-173`，测试 `validate.rs:654-673`、新默认断言 `:638-651`）。

另有一类**不走版本号的值迁移**：旧默认单值 `"i-wxxy"` 载入时幂等扩为三值 `"i-wxxy、iwxxy-2、iwxxy-3"`（安卓 `config_state.rs:291-293`，桌面 `validate.rs:121-123`），与桌面 2026-09-27 validate 同语义。

## 理由

稳态周期任务 15s 空转耗电，需要降频；直接改 `Settings::default` 对存量用户无效，因此引入版本化迁移。

v4 追加分档的理由：蜂窝环境探针必失败、灭屏时用户不在看状态，拉长间隔不改正确性（WiFi 变化事件仍即时触发检测——`handle_wifi_event` `monitor_loop.rs:291-329`，断开方向零延迟 `monitor_loop.rs:66`；无明确离线证据时在线状态保持上一拍记忆，`monitor_loop.rs:1343-1359` 三态护栏）。

## 备选方案

旧文档未记录（只记录机制的演进方向）。

## 影响与约束

改默认值必须走后端 `Settings::default` **+** schema 迁移双源覆盖——只改前端无效；前端 `DEFAULT_CONFIG.configSchemaVersion` 须与后端 `config_schema_version` 同步（现为 7，`constants.ts:58`；v3→v4 轮曾由漂移值 2 对齐到 4）。前端常量：`backgroundCheckInterval: 60000`（`constants.ts:27`）、`backgroundCheckIdleInterval: 300000`（`constants.ts:29`）、`campusCheckEndMinutes: 1380`（`constants.ts:50`）。~~桌面 `background_check_interval` 默认 15000 与安卓 60000 不一致~~——已消除：桌面 2026-09-20 v3→v4 迁移后同为 60000（`model.rs:260`）。桌面 Config 无闲时间隔字段——分档是安卓省电专属，不参与双端字段同步。桌面 `config_version`（现 5）与安卓 `config_schema_version`（现 7）仍是两套独立编号，见 [[config-field-sets-bidirectional-sync]]。

## Connections

[[network-quality-default-off]]、[[config-field-sets-bidirectional-sync]]、[[android-power-three-fixes]]
