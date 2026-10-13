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

Tauri v2 官方明确不支持后台任务（tauri-apps discussion #14615），桌面端有 system tray + 事件循环兜底，移动端必须另起炉灶。巡检需要 60s 级别的周期检测，且必须在高厂商定制的国产 ROM（澎湃OS/ColorOS/EMUI 等）后台存活。

**决策定调**：维持「Kotlin 前台服务（FGS）保活 + Rust tokio 循环跑检测」。Rust 侧负责所有业务逻辑（校园网判定、Portal 探测、自动重登、夜间出站切换、定时登录/注销），Kotlin 侧仅负责系统级保活能力（常驻通知、WifiLock/WakeLock、电池白名单引导、开机自启）。

## 核心组件与职责

### 1. 前台服务插件 (Foreground Service Plugin)
**文件**: `android/plugins/foreground-service/src/lib.rs`
**定位**: Tauri 移动插件，只负责"让进程活着"，监控/自动重登的业务逻辑全在 Rust 侧 `monitor_loop`。

- **`CampusMonitorService<R>` (struct)**: 封装 `PluginHandle<R>`，通过 `run_mobile_plugin` 转发命令到 Kotlin 侧 `MonitorServicePlugin`。
    - **`start_monitor(&self, title: &str)`** (`lib.rs:38`): 启动前台服务（常驻通知）。
    - **`stop_monitor(&self)`** (`lib.rs:41`): 停止前台服务。
    - **`update_notification(&self, text: &str)`** (`lib.rs:44`): 更新前台通知文案。
    - **`begin_probe_window(&self)`** (`lib.rs:47`): 进入探针窗口，持 WifiLock + PARTIAL_WAKE_LOCK。
    - **`end_probe_window(&self)`** (`lib.rs:50`): 退出探针窗口，释放窗口锁（幂等）。
    - **`get_power_state(&self)`** (`lib.rs:53-56`): 查询电源状态 `(screen_on, wifi_connected)`，用于巡检分档。
    - **`set_boot_autostart(&self, enabled: bool)`** (`lib.rs:62`): 开关开机自启。
    - **`is_boot_autostart_enabled(&self)`** (`lib.rs:65-67`): 查询开机自启状态。
    - **`install_apk(&self, file_path: &str)`** (`lib.rs:70-73`): 安装 APK（系统包安装器）。
    - **`get_battery_optimization_info(&self)`** (`lib.rs:76`): 获取电池优化白名单信息。
    - **`request_ignore_battery_optimizations(&self)`** (`lib.rs:79`): 申请加入电池优化白名单。
    - **`open_vendor_battery_settings(&self)`** (`lib.rs:82`): 跳转厂商自启/省电页。
    - **`open_notification_settings(&self)`** (`lib.rs:85`): 跳转应用通知设置页。

- **`CampusMonitorServiceExt<R>` (trait)** (`lib.rs:88-93`): Tauri `Manager` 扩展，提供 `campus_monitor_service()` 方法获取插件实例。

- **`init<R>()`** (`lib.rs:95-104`): 插件初始化，注册 Android 插件 `com.campuslogin.plugin.monitorservice.MonitorServicePlugin`。

### 2. 电池优化命令 (Battery Commands)
**文件**: `android/src-tauri/src/battery_cmds.rs`
**定位**: Rust 侧命令面，经 foreground-service 插件转发 Kotlin 实现。平台专属能力（Windows 无对应 API）。

- **`BatteryOptimizationInfo`** (`battery_cmds.rs:7-14`): 序列化结构体 `{ignoring, brand, hasVendorTarget}`。
- **`get_battery_optimization_info(app)`** (`battery_cmds.rs:16-33`): 返回 `BatteryOptimizationInfo`。
- **`request_ignore_battery_optimizations(app)`** (`battery_cmds.rs:39-53`): 一次性申请加入白名单，返回 true 表示系统确认框已弹出。
- **`open_vendor_battery_settings(app)`** (`battery_cmds.rs:59-71`): 跳厂商自启/省电页（降级链），返回 `{path, target, tried}`。
- **`open_notification_settings(app)`** (`battery_cmds.rs:77-91`): 跳应用通知设置页，返回 true 表示页面已拉起。

### 3. 监控循环核心 (Monitor Loop)
**文件**: `android/src-tauri/src/monitor_loop.rs`
**定位**: 后台监控循环主逻辑：tick = 校园网判定 → Portal 探测 → 掉线自动重登(cooldown + 次数上限) → emit background-check-result / login-log。决策逻辑抽纯函数便于 TDD。

#### 全局状态 (`MONITOR`)
**文件**: `monitor_loop.rs:28-78`
- **`running`**: 循环运行标志。
- **`check_count`**: 累计检测次数。
- **`consecutive_failures`**: 连续失败计数（用于熔断，达 5 次暂停自动重登）。
- **`reconnect_count`**: 本次在线周期内自动重连次数。
- **`last_login_attempt_ms`**: 最近一次登录尝试时间戳。
- **`was_online`**: 上一拍在线状态记忆（三态护栏核心）。
- **`last_result`**: `Mutex<Option<serde_json::Value>>`，缓存最近一次检测结果，供前端 `getInitData` 和 `get_background_status` 读取。
- **`desired_interval_ms`**: 目标检测间隔（支持热更新）。
- **`idle_interval_ms`**: 闲时巡检间隔（蜂窝/灭屏时生效）。
- **`logout_protected_until_ms`**: 注销保护期截止（手动注销后 60s 内不自动重登）。
- **`wifi_event_ms`**: 最近一次 WiFi 变化事件时间戳（去抖用）。
- **`notified_online`**: 常驻通知已展示的状态码（0=未展示, 1=在线, 2=未连接, 3=非检测时段, 4=WiFi 未连接）。仅状态翻转时才 notify。
- **`scheduled_login_day` / `scheduled_logout_day`**: 定时登录/注销当日已触发标记（防每日重复触发）。

#### 关键常量
- **`WIFI_EVENT_DELAY_MS = 2500`** (`monitor_loop.rs:81`): WiFi 连上事件延迟（等 DHCP/路由稳定）。
- **`WIFI_LOST_EVENT_DELAY_MS = 0`** (`monitor_loop.rs:89`): WiFi 断开事件零延迟。
- **`WIFI_EVENT_DEBOUNCE_MS = 1000`** (`monitor_loop.rs:92`): WiFi 事件去抖窗口。
- **`OUTBOUND_RESTORE_MAX_FAILS = 3`** (`monitor_loop.rs:25`): 夜间出站切换连续还原失败上限。
- **`OUTBOUND_SWITCH_MAX_FAILS = 3`** (`monitor_loop.rs:32`): 夜间出站切换连续注销失败上限。

#### 核心函数

- **`effective_interval_ms(base_ms, idle_ms, screen_on, wifi_connected)`** (`monitor_loop.rs:98-107`):
    - 纯函数，单测锁定。
    - WiFi 且屏幕亮着 → 基础间隔；蜂窝或屏幕熄灭 → 闲时间隔。
    - 闲时间隔不足基础间隔时以基础间隔为准。

- **`should_attempt_login(...)`** (`monitor_loop.rs:112-128`):
    - 纯函数，全量条件显式入参。
    - 在线或非校园网 → 不尝试。
    - 非掉线场景（启动即离线）需显式开启 `auto_login_on_preparation`。
    - 掉线重连受 `max_disconnect_reconnect` 和 `cooldown` 限制。

- **`status_value()`** (`monitor_loop.rs:133-152`):
    - 构造 `backgroundStatus` JSON，展平 `online`, `message`, `onlineOperator`, `serverAvailable`, `onCampusNetwork` 到顶层，供前端启动初值使用。

- **`notify_system(...)`** (`monitor_loop.rs:178-193`):
    - 系统通知（区别于常驻通知）：掉线/重连失败/达上限等关键事件。
    - 支持 mascot 大图显示，资源缺失时降级为纯文本。

- **`with_vpn_hint(body)`** (`monitor_loop.rs:198-204`):
    - 探测/登录失败通知追加 VPN 接管提示（检测到 VPN tun 时）。

- **`start_background_check(app)`** (`monitor_loop.rs:209-243`):
    - 启动后台监控：幂等刷新间隔 → 启动 FGS → 置 running 标志 → 重置通知状态 → 注册 WiFi 监听 → spawn tokio 循环。
    - 启动失败 reject 给前端且不残留假"运行中"。

- **`stop_background_check(app)`** (`monitor_loop.rs:246-256`):
    - 停止后台监控：置 running=false → 注销 WiFi 监听 → 停止 FGS。

- **`start_wifi_watcher(app)` / `stop_wifi_watcher(app)`** (`monitor_loop.rs:273-307`):
    - WiFi 变化监听注册/注销（Kotlin NetworkCallback → channel 事件）。
    - 失败退化为纯周期检测。

- **`handle_wifi_event(app, body)`** (`monitor_loop.rs:312-345`):
    - WiFi 变化事件处理：去抖 + 延迟后执行一次完整检测 (`run_check_once`)。
    - lost 事件零延迟，available 事件延迟 2500ms。
    - 风暴内只跑一次（起点判定见 `wifi_event_is_burst_start`）。

- **`wifi_event_is_burst_start(last_event_ms, now_ms, debounce_ms)`** (`monitor_loop.rs:90-93`):
    - 纯函数，单测锁定。首事件或距上次事件 >= 去抖窗口 → 起点。

- **`run_startup_tasks(app)`** (`monitor_loop.rs:385-428`):
    - 启动恢复：并行 spawn 三条互不依赖的启动链（后台检测/质量首测/自动登录）。
    - 启动提速：就绪窗口从 1.5s 收窄到 500ms。
    - 落地日志保留天数、避免 bad wifi 快照兜底、自动更新检查循环。

- **`probe_with_retry(app, settings)`** (`monitor_loop.rs:433-452`):
    - 启动探测：未确认校园网时 3s 后重试一次（应对 DHCP 未就绪）。

- **`auto_login_on_start(app, settings)`** (`monitor_loop.rs:467-506`):
    - 启动自动登录：有凭据 + 判定在校园网才登录。
    - 强制绑 WiFi → 探测（带重试）→ 登录 → 成功则置 `was_online=true` 并清注销保护期。

- **`monitor_tick_loop(app, interval_ms)`** (`monitor_loop.rs:566-613`):
    - tokio 循环体：每拍执行 `run_scheduled_actions` → 间隔热更新 → 分档判断 → `run_check_once`。
    - 启动对账：若当日注销时刻已过，消耗额度不执行动作，防冷启动误触发。

- **`run_scheduled_actions(app)`** (`monitor_loop.rs:655-783`):
    - 定时登录/定时注销/夜间出站切换/晚间断网运营商切换判定与执行。
    - **夜间出站切换**: 判定复用 `evaluate_night_outbound`，切换侧注销（`logout_for_outbound_switch`）+ 落盘标记 + 触发系统 WiFi 重检；还原侧登录（`night_switch_login`）+ 清标记 + 还原 avoid_bad_wifi。
    - **晚间断网切换**: 判定复用 `evaluate_night_switch`，切换运营商 + 登录 + 异步验证（`verify_night_switch`）。
    - **定时登录/注销**: 复用 `should_fire_scheduled_action`，每日单次，成败不重试。

- **`logout_for_outbound_switch(app)`** (`monitor_loop.rs:618-648`):
    - 切换侧注销：前置在线检查 → 执行注销 → 判据取 `radiusOk || unbindOk`（MAC 解绑成功即离线）。
    - 未生效时按连续失败计数收敛（`switch_logout_unconfirmed`）。

- **`portal_probe_on_little_cores(ip)`** (`monitor_loop.rs:693-732`):
    - Portal 全量探测：专用短命线程绑定小核（稳态功耗）。
    - 裸线程 enter tokio handle 防 panic，捕获 panic 转为 Err 回传。
    - 线程创建失败降级回共享阻塞池。

- **`run_check_once(app)`** (`monitor_loop.rs:853-983`):
    - 单次检测全流程：
        1. 配置读取与出站切换态闸（切换态跳过整轮巡检）。
        2. 校园网检测静默期闸（非检测时段跳过，但通知状态更新）。
        3. 探针窗口持有（`ProbeWindowGuard` Drop 兜底）。
        4. 强制绑 WiFi。
        5. 校园网判定（`probe_campus_with_ssid`，含 SSID 感知）。
        6. Portal 探测（`portal_probe_on_little_cores`，WiFi 未连接/非校园网短路）。
        7. 三态消费：仅"确定判定"才翻转在线状态，Unknown/Failed 保持上一拍记忆。
        8. 状态机：在线清零计数，掉线发系统通知。
        9. 自动登录判定与执行（注销保护期 + 连续失败熔断 + cooldown）。
        10. 组装 payload emit `background-check-result`。
        11. 常驻通知同步（仅状态翻转时 notify）。

- **`night_switch_login(app, settings)`** (`monitor_loop.rs:816-848`):
    - 夜切后的登录触发：直接走 `run_login` 内核，无校园网探测闸。
    - 成功则置 `was_online=true` 并清注销保护期。

- **`verify_night_switch(app, expected_operator)`** (`monitor_loop.rs:883-936`):
    - 夜切后的登录生效自动验证：等 15s → 查 eportal chkstatus → 不符则注销重登一轮再复验。
    - 独立 tokio 任务，不阻塞监控循环。

- **`notify_state(online, wifi_off)`** (`monitor_loop.rs:993-1001`):
    - 纯函数，单测锁定。返回状态码与文案：1=在线, 2=未连接, 4=WiFi 未连接。

## 架构流程

```text
┌─────────────────────────────────────────────────────────────┐
│                         Android System                      │
│  ┌──────────────┐   ┌──────────────┐   ┌─────────────────┐  │
│  │ Battery Opt  │   │ WiFi Callback│   │ Notification    │  │
│  │ Whitelist    │   │ (NetworkMgr) │   │ (FGS Ongoing)   │  │
│  └──────┬───────┘   └──────┬───────┘   └────────┬────────┘  │
│         │                  │                     │           │
│  ┌──────▼───────┐   ┌──────▼───────┐   ┌────────▼────────┐  │
│  │ battery_cmds │   │ WiFi Watcher │   │ CampusMonitor   │  │
│  │ .rs          │   │ monitor_loop │   │ Service (Kotlin)│  │
│  └──────┬───────┘   │ .rs          │   └────────┬────────┘  │
│         │           └──────┬───────┘            │           │
│         │           ┌──────▼───────┐   ┌────────▼────────┐  │
│         │           │ Foreground   │   │ Probe Window    │  │
│         │           │ Service      │   │ (WifiLock+Wake) │  │
│         │           │ Plugin       │   └────────┬────────┘  │
│         │           └──────┬───────┘            │           │
│         │           ┌──────▼───────┐   ┌────────▼────────┐  │
│         │           │ MonitorLoop  │   │ run_check_once  │  │
│         │           │ (tokio loop) │   │ (Campus Detect) │  │
│         │           └──────┬───────┘   └────────┬────────┘  │
│         │           ┌──────▼───────┐            │           │
│         │           │ Scheduled    │            │           │
│         │           │ Actions      │            │           │
│         │           └──────┬───────┘            │           │
│         │           ┌──────▼───────┐   ┌────────▼────────┐  │
│         │           │ Auto Login   │   │ Night Switch    │  │
│         │           │ (Protocol)   │   │ (Operator/Out)  │  │
│         │           └──────────────┘   └─────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## 数据流

1. **启动**: `run_startup_tasks` 并行拉起后台检测、质量测试、自动登录。
2. **巡检循环**: `monitor_tick_loop` 每 tick 执行 `run_check_once`。
    - 读配置 → 查电源状态 → 分档间隔 → 校园网判定 → Portal 探测 → 状态机更新 → 自动重登（如需）→ emit 结果。
3. **WiFi 事件**: `handle_wifi_event` 去抖后触发 `run_check_once`。
4. **定时任务**: `run_scheduled_actions` 在 tick 循环中先于检测执行，处理夜间切换、运营商切换、定时登录/注销。
5. **通知**: 常驻通知仅在 `notified_online` 状态码翻转时更新；系统通知在掉线、重连失败等关键事件时触发。

## 影响与约束

- **保活对抗手法不得引入**（无声音乐/1px Activity/双进程等）。
- **电池白名单属平台专属能力**，是双端同步铁律的例外条款。
- **厂商跳转只走标准 Intent+降级链**，不写私有 extras。
- **省电策略**: 翻转去重、节流、探针窗口按需持有、分档间隔、常驻通知按需 notify。
- **夜间出站切换**: 切换态下整轮巡检跳过，晨间还原判定照常执行。
- **注销保护期**: 手动注销后 60s 内不自动重登。
- **连续失败熔断**: 自动重登连续失败 5 次暂停，手动登录成功清零。

## Connections

- [[android-power-three-fixes]]
- [[rAF-blocks-compositor-idle]]
- [[android-persistent-notification-standard]]
- [[night-outbound-switch]]