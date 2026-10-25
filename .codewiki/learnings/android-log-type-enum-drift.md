---
title: android-log-type-enum-drift
type: learning
source_files:
  - android/src-tauri/src/monitor_loop.rs
  - android/frontend/src/shared/ui-types.ts
tags: [教训, 安卓, 双端同步, 枚举, 白屏]
---

## Overview

本文记录安卓端 `login-log` 事件 `type` 字段在后端与前端之间的字符串字面量漂移问题及修复过程。后端 `emit_login_log`（位于 `android/src-tauri/src/monitor_loop.rs:176-180`）构造 `{ "message": ..., "type": ... }` 并通过 Tauri 事件 `login-log` 广播；前端 `LogType`（`android/frontend/src/shared/ui-types.ts:6`）严格定义为 `'info' | 'success' | 'error' | 'warning'`。历史上后端曾使用非标准 `"warn"` 导致前端图标映射取 `undefined` 引发白屏。当前代码后端所有 `emit_login_log` 调用均使用合法四值，前端通过消费侧兜底防止同类问题重演。

## Key Components

### 后端：android/src-tauri/src/monitor_loop.rs

*   **`MONITOR`** (`:15-17`)：惰性全局监控状态单例，类型为 `MonitorState`。
*   **`OUTBOUND_RESTORE_FAIL_COUNT`** (`:21`)：夜间出站切换连续还原失败计数，原子类型 `AtomicU32`，初始 0。
*   **`OUTBOUND_RESTORE_MAX_FAILS`** (`:23`)：连续还原失败上限，常量 `u32`，值为 `3`。
*   **`OUTBOUND_SWITCH_FAIL_COUNT`** (`:26`)：夜间出站切换连续注销失败计数，原子类型 `AtomicU32`，初始 0。
*   **`OUTBOUND_SWITCH_MAX_FAILS`** (`:28`)：连续注销失败上限，常量 `u32`，值为 `3`。
*   **`MONITOR_GENERATION`** (`:32`)：监控循环代际 token，原子类型 `AtomicU64`，防 start/stop 竞速下标志误清。
*   **`LOGIN_ATTEMPT_LOCK`** (`:37`)：自动登录读-判-写 attempt 段互斥锁，`tokio::sync::Mutex<()>` 常量。
*   **`FORCE_CHECK_AFTER_RESTORE`** (`:47`)：夜间出站还原后无视静默期的一次性放行标记，`AtomicBool`。
*   **`MonitorState`** (`:49-79`)：监控循环全局状态结构体，派生 `Default`。
    *   `running: AtomicBool` (`:52`)：监控是否运行。
    *   `check_count: AtomicU64` (`:53`)：检测总次数。
    *   `consecutive_failures: AtomicU32` (`:54`)：连续失败次数（掉线重连熔断）。
    *   `reconnect_count: AtomicU32` (`:56`)：本次在线周期内自动重连次数。
    *   `last_login_attempt_ms: AtomicU64` (`:57`)：最近一次登录尝试时间戳。
    *   `was_online: AtomicBool` (`:58`)：上一拍在线状态记忆。
    *   `last_result: Mutex<Option<serde_json::Value>>` (`:59`)：最近一次检测结果。
    *   `desired_interval_ms: AtomicU64` (`:62`)：运行中目标检测间隔（ms）。
    *   `idle_interval_ms: AtomicU64` (`:64`)：闲时巡检间隔（ms）。
    *   `logout_protected_until_ms: AtomicU64` (`:66`)：注销保护期截止（epoch ms）。
    *   `wifi_event_ms: AtomicU64` (`:68`)：最近一次 WiFi 变化事件时间戳。
    *   `notified_online: AtomicU8` (`:72`)：常驻通知状态码（0=未展示,1=在线,2=未连接,3=非检测时段,4=WiFi未连接，详见 `notify_state` `:1619-1627`）。
    *   `scheduled_login_day: AtomicI32` (`:76`)：定时登录当日已触发标记（当日序号）。
    *   `scheduled_logout_day: AtomicI32` (`:78`)：定时注销当日已触发标记。
*   **`WIFI_EVENT_DELAY_MS`** (`:84`)：WiFi 事件触发检测的延迟毫秒数，常量 `u64`，值 `2500`。
*   **`WIFI_LOST_EVENT_DELAY_MS`** (`:89`)：WiFi 断开事件延迟毫秒数，常量 `u64`，值 `0`。
*   **`WIFI_EVENT_DEBOUNCE_MS`** (`:92`)：WiFi 事件去抖窗口毫秒数，常量 `u64`，值 `1000`。
*   **`wifi_event_is_burst_start`** (`:97-99`)：纯函数，判定 WiFi 事件是否为风暴起点（首事件或超去抖窗口）。
*   **`effective_interval_ms`** (`:106-113`)：纯函数，根据屏幕/WiFi 算有效间隔，基础间隔下限 5000ms。
*   **`should_attempt_login`** (`:117-138`)：纯函数，判定是否自动登录；条件含 online/was_online/on_campus/auto_login_on_preparation/reconnect_count/max_reconnect/cooldown。
*   **`status_value`** (`:141-162`)：构造后台状态 JSON，展平 `last_result` 关键字段（online/message/onlineOperator/serverAvailable/onCampusNetwork）到顶层。
*   **`is_running`** (`:165-167`)：检查监控是否运行。
*   **`epoch_ms`** (`:169-174`)：获取当前纪元毫秒。
*   **`emit_login_log`** (`:176-180`)：核心日志发射。构造 `{ "message": message, "type": log_type }` 并 `app.emit("login-log", payload)`。`log_type` 应为 `"info"|"success"|"error"|"warning"`。
*   **`notify_system`** (`:185-202`)：系统通知（区别于常驻通知），支持看板娘大图，缺失降级纯文本。
*   **`with_vpn_hint`** (`:208-214`)：失败消息追加 VPN 接管提示。
*   **`start_background_check`** (`:217-266`)：启动后台监控命令，幂等刷新间隔、保活 FGS、启 WiFi 监听。
*   **`stop_background_check`** (`:268-281`)：停止后台监控命令，推进代际、停服务、停 WiFi 监听。
*   **`WIFI_WATCHER_CHANNEL_ID`** (`:285`)：WiFi 监听 channel id 原子量。
*   **`WIFI_WATCHER_ACTIVE`** (`:287`)：WiFi 监听激活布尔原子量。
*   **`start_wifi_watcher`** (`:292-314`)：注册 WiFi 变化监听，失败置 `WATCHER_FAILED` 降级。
*   **`stop_wifi_watcher`** (`:318-330`)：注销 WiFi 变化监听。
*   **`handle_wifi_event`** (`:335-373`)：WiFi 事件处理，去抖+延迟后 `run_check_once`。
*   **`trigger_background_check`** (`:375-384`)：手动触发检测命令（仅运行中耳许）。
*   **`get_background_status`** (`:386-389`)：返回 `status_value()`。
*   **`get_boot_autostart`** (`:393-407`)：获取开机自启开关。
*   **`set_boot_autostart`** (`:409-432`)：设置开机自启并落盘配置。
*   **`get_notification_enabled`** (`:436-439`)：获取通知开关。
*   **`set_notification_enabled`** (`:440-456`)：设置通知开关并落盘。
*   **`run_startup_tasks`** (`:468-539`)：启动恢复，并行拉起后台检测/质量首测/自动登录/更新检查。
*   **`probe_with_retry`** (`:545-563`)：启动探测，首次失败 3s 后重试一次。
*   **`emit_auto_login_result`** (`:569-575`)：发射应用内 toast 结果事件 `auto-login-result`。
*   **`auto_login_on_start`** (`:579-640`)：启动自动登录，含静默期跳过、绑 WiFi、探测、登录。
*   **`ProbeWindowGuard`** (`:644-653`)：探针窗口 guard，Drop 释放 WifiLock/WakeLock。
*   **`power_state`** (`:658-669`)：查询电源状态（屏幕亮、WiFi 连），失败保守 `(true,true)`。
*   **`monitor_tick_loop`** (`:671-730`)：监控循环主体，代际校验、定时动作、间隔热更、分档跳拍。
*   **`outbound_switch_active`** (`:734-739`)：判断是否处于夜间出站切换态。
*   **`latest_settings`** (`:744-751`)：取最新配置避免 stale 快照。
*   **`logout_for_outbound_switch`** (`:759-784`)：切换侧注销，返回离线是否生效。
*   **`switch_logout_unconfirmed`** (`:789-806`)：注销未确认上限判定。
*   **`portal_probe_on_little_cores`** (`:812-849`)：Portal 探测绑小核短命线程。
*   **`run_scheduled_actions`** (`:863-1072`)：定时登录/注销/夜切/出站切换编排。
*   **`persist_settings`** (`:1077-1100`)：落盘配置并刷缓存、广播 `config-changed`。
*   **`night_switch_login`** (`:1106-1132`)：夜切登录内核，返回成功否。
*   **`trigger_wifi_recheck`** (`:1138-1159`)：触发系统 WiFi 重检。
*   **`ensure_avoid_bad_wifi`** (`:1164-1200`)：程序化设 `network_avoid_bad_wifi=1`。
*   **`restore_avoid_bad_wifi`** (`:1203-1227`)：还原 `network_avoid_bad_wifi`。
*   **`NIGHT_VERIFY_WAIT_SECS`** (`:1231`)：夜切验证等待 15s。
*   **`NIGHT_VERIFY_RETRY_SECS`** (`:1232`)：重试间隔 5s。
*   **`NIGHT_VERIFY_ATTEMPTS`** (`:1233`)：最大尝试 3 次。
*   **`NIGHT_VERIFY_HTTP_TIMEOUT_SECS`** (`:1234`)：HTTP 超时 15s。
*   **`portal_origin`** (`:1237-1249`)：取 portal URL 的 scheme+host。
*   **`expected_operator_display`** (`:1252-1258`)：空串显“无锡学院”。
*   **`verify_night_switch`** (`:1264-1318`)：夜切登录生效验证任务。
*   **`night_switch_verify_round`** (`:1322-1339`)：一轮验证多次重试。
*   **`night_switch_check_once`** (`:1342-1371`)：单次 chkstatus 核对 uid。
*   **`run_check_once`** (`:1374-1615`)：单次检测全流程，发射 `background-check-result`。
*   **`notify_state`** (`:1619-1627`)：常驻通知状态码纯函数（1在线/2未连接/4WiFi未连接/3非检测时段）。
*   **测试模块** (`:1629-1751`)：含 `notify_state` 状态码、`should_attempt_login` 各分支、`wifi_event_is_burst_start`、`effective_interval_ms`、`portal_origin`、`expected_operator_display`、`status_value` 展平测试。

### 类型定义：android/frontend/src/shared/ui-types.ts

*   **`StatusState`** (`:1`)：`'loading' | 'online' | 'offline' | 'error'`。
*   **`PanelName`** (`:2`)：面板名枚举，含 dashboard/account/selfservice/network/monitor/quality/settings/log/speedtest。
*   **`ThemeName`** (`:3`)：主题枚举，含 default/vibrant/forest/midnight/ocean/cherry/custom。
*   **`GlassLevel`** (`:5`)：玻璃质感档位 `'off' | 'standard' | 'ultra'`。
*   **`LogType`** (`:6`)：日志类型枚举 `'info' | 'success' | 'error' | 'warning'`，前后端契约关键。
*   **`LogEntry`** (`:8-15`)：日志条目接口，含 `id,time,message,type:LogType,count?`。
*   **`ToastMessage`** (`:17-29`)：Toast 接口，含 `id,title,description?,type,mascot?,action?`。
*   **`AdapterDisabledWarningData`** (`:31-34`)：适配器禁用警告。
*   **`AutoExitCountdownData`** (`:36-39`)：自动退出倒计时。
*   **`SaveConfigResult`** (`:41-45`)：保存配置结果。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│ Android Backend (monitor_loop.rs)                           │
│  run_check_once/:1374 ─┐                                    │
│  run_scheduled_actions/:863 ─┤                              │
│  auto_login_on_start/:579 ──► emit_login_log/:176          │
│                            │  builds {message,type}         │
│                            ▼                                │
│                     app.emit("login-log")                   │
└────────────────────────────┬────────────────────────────────┘
                             │ Tauri IPC
                             ▼
┌─────────────────────────────────────────────────────────────┐
│ Frontend (ui-types.ts / listeners)                          │
│  LogType = 'info'|'success'|'error'|'warning'  (:6)         │
│  onLoginLog: data.type as LogType || 'info' ─► addLog()     │
│  LogEntry.type ─► icon/color render (no crash on drift)     │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **后端生成**：`run_check_once`（`:1374`）、`run_scheduled_actions`（`:863`）、`auto_login_on_start`（`:579`）等调用 `emit_login_log(app, message, log_type)`（`:176`）。
2. **事件发射**：`emit_login_log` 构造 `{"message":..., "type":"warning"}` 等合法值，经 `app.emit("login-log", payload)`（`:179`）发送。
3. **前端接收**：事件监听器捕获 `login-log`，取 `data.type`。
4. **类型转换**：`(data.type as LogType) || 'info'` 兜底转换，存入日志 store。
5. **UI 渲染**：`LogEntry` 按 `type` 匹配图标；因枚举与后端一致且前端兜底，漂移不白屏。

## Known Issues

* **历史白屏根因**：后端曾发 `"warn"` 不在 `LogType`（`:6`）中，前端图标映射 `undefined` 致 React 崩溃。
* **当前状态**：后端全部 `emit_login_log` 调用均使用四合法值，具体行号与传值如下：
  * `:264` `"info"`（后台监控已启动）
  * `:279` `"info"`（后台监控已停止）
  * `:597` `"info"`（启动检测静默期跳过）
  * `:607` `"warning"`（校园网判定失败）
  * `:613` `"info"`（不在校园网跳过）
  * `:616` `"info"`（检测到校园网）
  * `:620` `"success"`（启动自动登录成功）
  * `:630` `"error"`（启动自动登录失败）
  * `:635` `"error"`（启动自动登录执行失败）
  * `:764` `"info"`（未检测到在线跳过注销）
  * `:799` `"warning"`（注销未确认生效）
  * `:913` `"info"`（已注销等待切换）
  * `:932` `"success"`（已恢复校园网登录）
  * `:947` `"error"`（自动还原失败放弃）
  * `:991` `"info"`（运营商切至无锡学院）
  * `:1004` `"info"`（已恢复原运营商）
  * `:1039` `"info"`（定时登录触发）
  * `:1053` `"info"`（定时注销触发）
  * `:1067` `"success"`/`"error"`（定时注销结果）
  * `:1069` `"error"`（定时注销执行失败）
  * `:1082` `"error"`（配置落盘失败）
  * `:1087` `"error"`（配置落盘失败）
  * `:1108` `"warning"`（账号密码未配置）
  * `:1118` `"success"`/`"error"`（夜切登录结果）
  * `:1128` `"error"`（夜切登录失败）
  * `:1379` `"error"`（读取配置失败）
  * `:1449` `"error"`（校园网检测失败）
  * `:1537` `"info"`/`"error"`（自动登录结果）
  * `:1558` `"error"`（自动登录执行失败）
* **防御机制**：前端消费侧 `as LogType || 'info'` 兜底；`ui-types.ts` 严格枚举。
* **桌面端风险**：桌面端 `tauri-app` 部分 `LOG_ICONS` 访问缺 `?? info` 兜底，存同类隐患。

## Connections

[[tablet-layout-alignment-audit]]、[[android-white-screen-missing-init-fields]]
