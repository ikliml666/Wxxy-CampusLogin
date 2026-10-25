---
title: 安卓后台监控循环架构：FGS 保活 + Rust tokio 巡检 + 夜间切换
type: decision
tags: [决策, 安卓, 保活, 监控循环, 夜间切换, 省电]
source_files:
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/battery_cmds.rs
  - android/plugins/foreground-service/src/lib.rs
---

## 背景

Tauri v2 官方明确不支持后台任务，移动端必须另起炉灶。巡检需要 60s 级别的周期检测，且必须在高厂商定制的国产 ROM 后台存活。

**决策定调**：维持「Kotlin 前台服务（FGS）保活 + Rust tokio 循环跑检测」。Rust 侧（`monitor_loop.rs`）负责所有业务逻辑（校园网判定、Portal 探测、自动重登、夜间出站切换、定时登录/注销、运营商夜切、WiFi 事件即时触发、探针窗口锁），Kotlin 侧仅通过 `foreground-service` 插件提供系统级保活能力（常驻通知、WifiLock/WakeLock、电池白名单引导、开机自启、APK 安装）。`battery_cmds.rs` 作为 Rust 命令面经插件转发 Kotlin 实现，属平台专属能力（非 mobile 端返回 Err）。

## 核心组件与职责

### 1. 前台服务插件 (Foreground Service Plugin)
**文件**: `android/plugins/foreground-service/src/lib.rs`

- **`PLUGIN_IDENTIFIER`** (行 12): 常量 `com.campuslogin.plugin.monitorservice`。
- **`BoolResult`** (行 15-17): 反序列化结构体，字段 `enabled` (行 16)。
- **`PowerStateResult`** (行 20-25): 反序列化结构体，字段 `screen_on` (行 22, `#[serde(rename = "screenOn")]`), `wifi_connected` (行 24, `#[serde(rename = "wifiConnected")]`)。
- **`CampusMonitorService<R>`** (行 27): 封装 `PluginHandle<R>` 的插件结构体。
- **`Result<T>`** (行 29): 类型别名 `std::result::Result<T, tauri::plugin::mobile::PluginInvokeError>`。
- **`CampusMonitorService<R>` 方法** (行 31-98):
  - `start_monitor(&self, title: &str)` (行 33-35): 启动前台服务（常驻通知）。
  - `stop_monitor(&self)` (行 37-39): 停止前台服务。
  - `update_notification(&self, text: &str)` (行 42-44): 更新前台通知文案。
  - `begin_probe_window(&self)` (行 47-49): 进入探针窗口，持 WifiLock + PARTIAL_WAKE_LOCK。
  - `end_probe_window(&self)` (行 52-54): 退出探针窗口，释放锁（幂等）。
  - `get_power_state(&self)` (行 58-61): 返回 `(screen_on, wifi_connected)` 供巡检分档。
  - `set_boot_autostart(&self, enabled: bool)` (行 64-66): 开关开机自启。
  - `is_boot_autostart_enabled(&self)` (行 68-71): 查询开机自启状态。
  - `install_apk(&self, file_path: &str)` (行 75-77): 安装 APK（系统包安装器）。
  - `get_battery_optimization_info(&self)` (行 80-82): 获取电池优化白名单信息。
  - `request_ignore_battery_optimizations(&self)` (行 85-87): 申请加入电池优化白名单。
  - `open_vendor_battery_settings(&self)` (行 90-92): 跳转厂商自启/省电页（降级链）。
  - `open_notification_settings(&self)` (行 95-97): 跳转应用通知设置页（降级链）。
- **`CampusMonitorServiceExt<R>`** (行 100-102): Tauri `Manager` 扩展 trait，方法 `campus_monitor_service()` (行 101) 获取插件实例。
- **`init<R>()`** (行 110-119): 插件初始化，注册 Android 插件 `MonitorServicePlugin` 并 `app.manage` 注入状态。

### 2. 电池优化命令 (Battery Commands)
**文件**: `android/src-tauri/src/battery_cmds.rs`

- **`BatteryOptimizationInfo`** (行 8-17): 序列化结构体（`#[serde(rename_all = "camelCase")]`），字段 `ignoring` (行 12), `brand` (行 14), `has_vendor_target` (行 16)。
- **`get_battery_optimization_info(app)`** (行 20-41): 返回 `BatteryOptimizationInfo`；`#[cfg(not(mobile))]` 分支 (行 36-40) 返回 `Err("电池优化白名单仅安卓端可用")`。
- **`request_ignore_battery_optimizations(app)`** (行 46-61): 一次性申请白名单，返回 `opened` 布尔；非 mobile 端 (行 56-60) 返回 Err。
- **`open_vendor_battery_settings(app)`** (行 65-81): 跳厂商页，返回 `{path, target, tried}`；非 mobile 端 (行 76-80) 返回 Err。
- **`open_notification_settings(app)`** (行 87-101): 跳通知设置页，返回 `opened`；非 mobile 端 (行 97-101) 返回 Err。

### 3. 监控循环核心 (Monitor Loop)
**文件**: `android/src-tauri/src/monitor_loop.rs`

**全局状态与常量**:
- **`MONITOR`** (行 15-17): `lazy_static` 持有 `MonitorState::default()`。
- **`MonitorState`** (行 49-79, 标注 `#[allow(dead_code)] // Task 4 循环体接线`): 字段含 `running` (行 52), `check_count` (行 53), `consecutive_failures` (行 54), `reconnect_count` (行 56), `last_login_attempt_ms` (行 57), `was_online` (行 58), `last_result` (行 59, `Mutex<Option<serde_json::Value>>`), `desired_interval_ms` (行 62), `idle_interval_ms` (行 64), `logout_protected_until_ms` (行 66), `wifi_event_ms` (行 68), `notified_online` (行 72, `AtomicU8` 状态码), `scheduled_login_day` (行 76), `scheduled_logout_day` (行 78)。
- **`OUTBOUND_RESTORE_FAIL_COUNT`** (行 21), **`OUTBOUND_RESTORE_MAX_FAILS`** (行 23, 值 3)。
- **`OUTBOUND_SWITCH_FAIL_COUNT`** (行 26), **`OUTBOUND_SWITCH_MAX_FAILS`** (行 28, 值 3)。
- **`MONITOR_GENERATION`** (行 32): 代际 token 防 stop→start 竞速。
- **`LOGIN_ATTEMPT_LOCK`** (行 37): tokio `Mutex` 串行化登录 attempt 段。
- **`FORCE_CHECK_AFTER_RESTORE`** (行 47): 还原后无视静默期的一次性放行标记。
- **`WIFI_EVENT_DELAY_MS`** (行 84, 2500), **`WIFI_LOST_EVENT_DELAY_MS`** (行 89, 0), **`WIFI_EVENT_DEBOUNCE_MS`** (行 92, 1000)。
- **`WIFI_WATCHER_CHANNEL_ID`** (行 285), **`WIFI_WATCHER_ACTIVE`** (行 287): 均 `#[cfg(mobile)]`。

**纯函数/工具**:
- `wifi_event_is_burst_start(last_event_ms, now_ms, debounce_ms)` (行 97-99)
- `effective_interval_ms(base_ms, idle_ms, screen_on, wifi_connected)` (行 106-113)
- `should_attempt_login(...)` (行 117-138, 标注 `#[allow(dead_code)]`)
- `status_value()` (行 141-162): 展平 `online`/`message`/`onlineOperator`/`serverAvailable`/`onCampusNetwork` 到顶层。
- `is_running()` (行 165-167, 标注 `#[allow(dead_code)]`)
- `epoch_ms()` (行 169-174)
- `emit_login_log(app, message, log_type)` (行 176-180)
- `notify_system(app, enabled, title, body, mascot)` (行 185-202)
- `with_vpn_hint(body)` (行 208-214)
- `portal_origin(portal_url)` (行 1233-1245)
- `expected_operator_display(expected_operator)` (行 1248-1254)
- `notify_state(online, wifi_off)` (行 1614-1622, 返回 (u8, &str): 1=在线, 2=未连接, 4=WiFi未连接；3=非检测时段为 `notified_online` 状态码，不由此函数返回)

**命令/入口**:
- `start_background_check(app)` (行 218-266): 幂等启动，刷间隔→起 FGS→置 running→重置通知→`#[cfg(mobile)]` 起 WiFi watcher→spawn `monitor_tick_loop`。
- `stop_background_check(app)` (行 269-281): 推进代际→清 running→停 watcher→停 FGS。
- `start_wifi_watcher(app)` / `stop_wifi_watcher(app)` (行 292-314 / 318-330, `#[cfg(mobile)]`)
- `handle_wifi_event(app, body)` (行 335-373, `#[cfg(mobile)]`): 去抖+延迟触发 `run_check_once`。
- `trigger_background_check(app)` (行 376-384)
- `get_background_status()` (行 387-389)
- `get_boot_autostart(app)` / `set_boot_autostart(app, enabled)` (行 393-407 / 410-432)
- `get_notification_enabled(app)` / `set_notification_enabled(app, enabled)` (行 436-438 / 441-456)

**启动与自动登录**:
- `run_startup_tasks(app)` (行 468-539): 并行 spawn 后台检测/质量首测/自动登录，500ms 就绪窗口。
- `probe_with_retry(app, settings)` (行 545-563): 首探测失败 3s 后重试。
- `emit_auto_login_result(app, success, message)` (行 569-575)
- `auto_login_on_start(app, settings)` (行 579-637): 静默期闸→绑 WiFi→探测→登录→置 `was_online`。

**循环体与调度**:
- `ProbeWindowGuard` (行 642-650, `#[cfg(mobile)]`): Drop 释放探针窗口锁。
- `power_state(app)` (行 656-661 mobile / 664-666 not-mobile 返回 `(true, true)`)
- `monitor_tick_loop(app, interval_ms, gen)` (行 668-727): tokio 循环，代际校验→`run_scheduled_actions`→间隔热更新→分档→`run_check_once`。
- `outbound_switch_active(app)` (行 731-736)
- `latest_settings(app, fallback)` (行 741-748)
- `logout_for_outbound_switch(app)` (行 756-781)
- `switch_logout_unconfirmed(app, reason)` (行 786-803)
- `portal_probe_on_little_cores(ip)` (行 809-846): 绑小核短命线程探测，捕获 panic。
- `run_scheduled_actions(app)` (行 860-1068): 夜间出站切换/运营商夜切/定时登录注销。
- `persist_settings(app, s)` (行 1073-1096)
- `night_switch_login(app, settings)` (行 1102-1128)
- `trigger_wifi_recheck(app)` (行 1134-1155)
- `ensure_avoid_bad_wifi(app)` (行 1160-1196)
- `restore_avoid_bad_wifi(app)` (行 1199-1223)

**夜切验证**:
- 常量 `NIGHT_VERIFY_WAIT_SECS` (行 1227), `NIGHT_VERIFY_RETRY_SECS` (行 1228), `NIGHT_VERIFY_ATTEMPTS` (行 1229), `NIGHT_VERIFY_HTTP_TIMEOUT_SECS` (行 1230)
- `verify_night_switch(app, expected_operator)` (行 1260-1314)
- `night_switch_verify_round(settings, expected_operator)` (行 1318-1335)
- `night_switch_check_once(client, portal_url, user, expected_operator)` (行 1338-1367)

**单次检测**:
- `run_check_once(app)` (行 1370-1610): 出站切换态闸→静默期闸(`FORCE_CHECK_AFTER_RESTORE` 例外)→探针窗口→绑 WiFi→`probe_campus_with_ssid`→Portal 短路/探测→三态护栏→状态机→自动登录(注销保护期+熔断+`LOGIN_ATTEMPT_LOCK`)→emit `background-check-result`→常驻通知翻转更新。

**测试模块** (`#[cfg(test)] mod tests`, 行 1624-1746):
覆盖 `notify_state` (行 1632), `should_attempt_login` (行 1641-1673), `wifi_event_is_burst_start` (行 1676), `effective_interval_ms` (行 1693-1713), `portal_origin` (行 1716), `expected_operator_display` (行 1727), `status_value` (行 1732) 等纯函数。

## 架构流程

```text
┌─────────────────────────────────────────────────────────────┐
│                         Android System                      │
│  ┌──────────────┐   ┌──────────────┐   ┌─────────────────┐  │
│  │ Battery Opt  │   │ WiFi Callback│   │ Notification    │  │
│  └──────┬───────┘   └──────┬───────┘   └────────┬────────┘  │
│         │                  │                     │           │
│  ┌──────▼───────┐   ┌──────▼───────┐   ┌────────▼────────┐  │
│  │battery_cmds  │   │ WiFi Watcher │   │ CampusMonitor   │  │
│  │.rs           │   │ monitor_loop │   │ Service(lib.rs) │  │
│  └──────┬───────┘   │ .rs          │   └────────┬────────┘  │
│         │           └──────┬───────┘            │           │
│         │           ┌──────▼───────┐   ┌────────▼────────┐  │
│         │           │ Foreground   │   │ ProbeWindowGuard│  │
│         │           │ Service      │   │ (WifiLock+Wake) │  │
│         │           │ Plugin       │   └────────┬────────┘  │
│         │           └──────┬───────┘            │           │
│         │           ┌──────▼───────┐   ┌────────▼────────┐  │
│         │           │ MonitorLoop  │   │ run_check_once  │  │
│         │           │ (tokio loop) │   │ (Campus Detect) │  │
│         │           └──────┬───────┘   └────────┬────────┘  │
│         │           ┌──────▼───────┐   ┌────────▼────────┐  │
│         │           │ Scheduled    │   │ Night Switch    │  │
│         │           │ Actions      │   │ (Operator/Out)  │  │
│         │           └──────┬───────┘   └─────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## 数据流

1. **启动恢复**: `run_startup_tasks` (`monitor_loop.rs:468-539`) 并行 spawn 后台检测、质量首测、自动登录；启动即应用日志保留天数与 `avoid_bad_wifi` 快照兜底。
2. **巡检循环**: `monitor_tick_loop` (`monitor_loop.rs:668-727`) 每 tick 先跑 `run_scheduled_actions` (`monitor_loop.rs:860-1068`)，再分档间隔后调 `run_check_once` (`monitor_loop.rs:1370-1610`)。
3. **WiFi 事件**: `handle_wifi_event` (`monitor_loop.rs:335-373`) 经 `wifi_event_is_burst_start` 去抖，lost 零延迟/available 延迟 2500ms 后触发 `run_check_once`。
4. **定时与切换**: `run_scheduled_actions` 复用共享纯函数 `evaluate_outbound_action_with_restore` / `evaluate_night_switch_gated_with_restore` / `evaluate_scheduled_actions` 决策，落盘标记经 `persist_settings` (`monitor_loop.rs:1073-1096`)。
5. **通知**: 常驻通知由 `notified_online` (行 72) 状态码翻转驱动 (`update_notification` via `lib.rs:42-44`)；系统通知由 `notify_system` (`monitor_loop.rs:185-202`) 在掉线/失败等关键事件触发；应用内 toast 经 `auto-login-result` 事件。

## 影响与约束

- **代际防竞速**: `MONITOR_GENERATION` (`monitor_loop.rs:32`) 确保旧循环退出时不会误清新一轮 `running` 标志 (`monitor_loop.rs:695`, `724-726`)。
- **登录互斥**: `LOGIN_ATTEMPT_LOCK` (`monitor_loop.rs:37`) 覆盖 tick/WiFi/手动三条入口的记账→登录→计数段 (`monitor_loop.rs:1519`)。
- **省电策略**: `effective_interval_ms` (行 106-113) 分档（亮屏 WiFi 基础间隔，否则闲时间隔）；`notified_online` 仅翻转时 notify；探针窗口 `ProbeWindowGuard` (行 642-650) 按需持锁；`portal_probe_on_little_cores` (行 809-846) 绑小核。
- **熔断与保护**: `consecutive_failures` 达 5 暂停自动重登 (`monitor_loop.rs:1503`, `1545`); `logout_protected_until_ms` 手动注销后 60s 内不重登 (`monitor_loop.rs:1502`)。
- **三态护栏**: `run_check_once` 中仅 `error_kind.is_none()` 才翻转 `online` (`monitor_loop.rs:1477-1481`)，Unknown/Failed 保持 `was_online` 记忆。
- **平台限制**: `#[cfg(mobile)]` 标注的 `start_wifi_watcher` (行 291), `handle_wifi_event` (行 334), `ProbeWindowGuard` (行 641), `power_state` mobile 版 (行 655) 等不在桌面编译；`battery_cmds.rs` 非 mobile 分支 (行 36-40 等) 直接返回 Err。
- **出站切换态**: `night_outbound_restore` 标记非空时 `run_check_once` 整轮跳过 (`monitor_loop.rs:1385-1388`)，但 `run_scheduled_actions` 照常执行还原判定。

## Connections

- [[android-power-three-fixes]]
- [[rAF-blocks-compositor-idle]]
- [[android-persistent-notification-standard]]
- [[night-outbound-switch]]
