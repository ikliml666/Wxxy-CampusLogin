---
title: 后台巡检与自动登录全链路
type: concept
source_files:
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/app/lightweight.rs
  - tauri-app/src-tauri/src/monitor/watcher.rs
  - tauri-app/src-tauri/src/monitor/background_task.rs
  - tauri-app/src-tauri/src/monitor/background_check.rs
  - tauri-app/src-tauri/src/monitor/background_emit.rs
  - tauri-app/src-tauri/src/monitor/auto_auth.rs
  - tauri-app/src-tauri/src/monitor/portal_check.rs
  - tauri-app/src-tauri/src/monitor/campus_check.rs
  - tauri-app/src-tauri/src/monitor/outbound_night.rs
  - tauri-app/src-tauri/src/monitor/scheduled.rs
  - tauri-app/src-tauri/src/infra/notification.rs
  - tauri-app/src-tauri/src/infra/state/network.rs
  - tauri-app/src-tauri/src/infra/lifecycle.rs
  - tauri-app/src-tauri/src/commands/login.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/outbound_switch.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/config_state.rs
tags: [概念, 后台巡检, 自动登录, 断线重连, 生命周期, 通知, 会话失效]
---

## Overview

后台巡检是一条「启动装配 → 周期 tick → 校园网判定 → Portal 探测 → 状态机 → 自动登录 / 断线重连 → 生命周期倒计时 / 通知」的长链路。桌面端实现在 `tauri-app/src-tauri/src/monitor/` 模块群（`watcher` 启动装配门面 + `background_task` 调度 + `background_check` 巡检主体 + `auto_auth` 自动登录决策 + `background_emit` 事件出口），安卓端实现在单个 `android/src-tauri/src/monitor_loop.rs`（决策逻辑抽成纯函数便于 TDD）。两端共享同一套阈值语义（连续失败 5 次熔断、重连上限 3、冷却 60s、注销保护 60s），但周期常量、检测时段门控与省电分档策略不同。

2026-10-22 修复了两个让「整夜休眠后到点不自动登录、只能手动点」的缺口，构成本文的两个核心新增点：

1. **会话失效重置**（`monitor/background_check.rs:25-27` 纯函数，`:334-340` 调用点）：巡检在调用 `try_auto_login_on_preparation` **之前**，若判定「服务可达 + 主副卡都不在线 + 此前登录过」，就把 `has_logged_online` 复位。此前该标记只在登录成功时置位、永不复位（唯二的复位点只覆盖校园网检测失败与用户主动注销），整夜休眠或夜间出站切换后旧会话已失效，`auto_auth.rs:42-44` 的「已登录过」门控会把自动登录永久拦住。放在评估之前是为了同一次巡检就能真正登录，而不是再等一拍。
2. **还原后立即补检**（`monitor/background_task.rs:145-179`，钩子挂在 `monitor/outbound_night.rs:1110`）：夜间出站还原刚把校园网卡从设备级禁用中启用，Windows 还要数秒到数十秒才拿到 DHCP 地址。此刻立刻巡检只会读到「配置的适配器均无 IP」空转，随后要等一整个巡检周期——轻量化模式下该周期下限 300s（`app/lightweight.rs:77-79`）。新增钩子在 `finish_outbound_restore`（`monitor/outbound_night.rs:1096-1112`）尾部触发，先轮询等 IP 就绪（最长 45s、3s 一轮），再发起一次性巡检。

## Key Components

### 桌面端核心结构体与枚举

**`BackgroundCheckResult`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:14-34`)
后台检测结果聚合，替代原先 16 个独立参数，避免调用方传参错位：

- `online: bool` (`:21`): 主适配器在线状态
- `reachable: bool` (`:22`): Portal 服务可达性
- `login_available: bool` (`:23`): 是否可登录
- `message: &'a str` (`:24`): 主适配器消息
- `adapter1_name: &'a str` / `adapter2_name: &'a str` (`:25-26`): 主/副适配器名称
- `secondary_online: Option<bool>` (`:27`), `secondary_message: &'a str` (`:28`): 副适配器状态与消息
- `online_operator: Option<String>` (`:29`): 主适配器在线时由 Portal uid 推导的运营商后缀（`""`=无锡学院、`@telecom`/`@unicom`/`@cmcc`；`None`=离线/未知）
- `secondary_online_operator: Option<String>` (`:30`): 副适配器同口径运营商后缀
- `dual_adapter: bool` (`:31`): 是否双适配器模式
- `config: &'a crate::config::model::Config` (`:32`): 配置引用
- `campus_result: &'a CampusCheckResult` (`:33`): 校园网检测结果
- `a1_campus_msg: Option<&'a str>` / `a2_campus_msg: Option<&'a str>` (`:35-36`): 各适配器校园网消息
- `a1_on_campus: Option<bool>` / `a2_on_campus: Option<bool>` (`:37-38`): 各适配器是否判定在校园网

**`PortalCheckResult`** (`tauri-app/src-tauri/src/monitor/portal_check.rs:6-19`)
Portal 探测结果枚举：

- `Success { online, message, reachable, login_available, uid }` (`:7-13`): 探测成功；`uid: Option<String>` 是 Portal 页面携带的完整在线账号（账号+运营商后缀），用于推导 `online_operator`
- `Error { is_request_failed }` (`:14-16`): 探测异常，`is_request_failed` 区分「请求层面失败」（可触发 MAC 重置熔断）与其他异常
- `NotFound` (`:17-19`): 未找到适配器

impl 访问器（同文件）：`online()`、`message()`、`reachable()`、`login_available()`、`uid()`——均仅在 `Success` 分支返回真实值，其余分支返回 false/固定文案/None。

**`CampusCheckResult`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:15-21`) 与 **`ConnectionCampusStatus`** (`:7-11`)
校园网判定结果（serde camelCase）：`wifi: Option<ConnectionCampusStatus>`、`wired: Option<ConnectionCampusStatus>`、`on_campus: bool`（= wifi ∨ wired 任一判定在校园）、`current_ssid: Option<String>`、`message: String`。`ConnectionCampusStatus` 携带单连接的 `on_campus`/`name`/`message`。

**`NetworkSnapshot`** (`tauri-app/src-tauri/src/infra/state/network.rs:7-28`)
巡检状态机读写的唯一状态载体（`NetworkState = ArcSwap<NetworkSnapshot>`，`:55-57`；`update` CAS 循环 `:78-92`，`update_with_result` CAS 并返回闭包结果防 TOCTOU `:94-113`）。16 个字段与巡检链路一一对应：

- `server_available: bool` (`:8`): 最近一次 Portal 探测的服务可达性
- `any_adapter_online: bool` (`:9`): 任一适配器在线（状态翻转通知与复位判定均用它）
- `last_a1_online: bool` (`:10`), `last_a2_online: bool` (`:11`): 主/副适配器最近在线态
- `has_logged_online: bool` (`:12`): **本次会话已登录过**标记，自动登录门控与 2026-10-22 会话失效重置的对象
- `disconnect_reconnect_count: u32` (`:13`): 断线重连计数（上限 `config.max_disconnect_reconnect`）
- `background_check_count: u32` (`:14`): 巡检轮次（`update_with_result` 内 CAS 自增后返回，随事件下发）
- `last_auto_login_attempt: Instant` (`:15`): 上次自动登录/重连尝试时刻，冷却判定基准
- `last_network_quality: Option<Arc<str>>` (`:18`), `current_ssid: Option<Arc<str>>` (`:19`): 用 `Arc<str>` 防 CAS 循环整份快照深拷贝
- `on_campus_network: bool` (`:20`): 最近校园网判定结果
- `logout_protected_until: Instant` (`:21`): 注销保护期截止点，保护期内跳过自动登录/重连并强制事件 `online=false`
- `portal_failure_count: u32` (`:22`): Portal 请求失败累计（熔断 MAC 重置）
- `a1_auth_failure_count: u32` (`:23`), `a2_auth_failure_count: u32` (`:24`): 各适配器认证失败计数，Portal 探测成功即清零
- `prep_login_failures: u32` (`:27`): 准备自动登录连续失败次数，达上限后本会话停止自动登录（防错误凭据无限重试 + 周期性 DHCP 断网）

**`AdapterOnlineStatus`** (`tauri-app/src-tauri/src/commands/login.rs:9-16`)
注销后的在线复核结果：`any_online`/`a1_online`/`a2_online`/`probe_ok`。`probe_ok=false` 表示存在探测异常——离线结论不可信，不得断言「已确认离线」（审计 A8）。

### 桌面端启动与调度

**`setup_app`** (`tauri-app/src-tauri/src/app/startup.rs:153`)
桌面启动装配入口。日志目录先探测安装目录 `logs/` 可写、否则回退数据目录（`:167-187`）；logger 先于 config 初始化（`:182-186`）；`set_log_retention_days(config.log_retention_days)`（`:213`）；`silent_start = config.hidden_start && args 含 --autostart`（`:218-223`）；随后依次 build_tray、`start_adapter_watch`（`:228`）、`start_cache_refresh_task`（`:231`）、`start_update_check_loop` + `cleanup_stale_update_dir`（`:235-238`）、**`run_startup_tasks`（`:240`）**、`record_webview2_runtime_version`/`subscribe_process_failed`（`:244-246`）、`spawn_heartbeat_thread`/`spawn_window_safety_thread`（`:248-249`）、GPU-warmup 后台线程（`:253-261`）。

**`run()` 命令注册** (`tauri-app/src-tauri/src/app/startup.rs:98-101`)
`background` 命令组 4 个命令 `start_background_check`/`stop_background_check`/`trigger_background_check`/`get_background_status` 在 `startup.rs:98-101` 注册；`RunEvent::ExitRequested` 拦截轻量化退出（`take_lightweight_exit_guard` + `should_prevent_exit`，`:136-159`）。

**`run_startup_tasks`** (`tauri-app/src-tauri/src/monitor/watcher.rs:16-69`)
启动装配门面（watcher 只保留装配与 re-export，`monitor/background_task.rs`/`campus_check.rs`/`background_emit.rs` 的入口统一经 `watcher.rs:14` re-export）：

- `config.enable_background_check` 为真 → spawn `startup_bg_check`（`:21-31`），内部调 `start_background_check_inner`
- `enable_network_quality && enable_latency_test` → spawn `startup_latency` → `spawn_latency_test_loop`（`:33-42`）
- 仅 `enable_network_quality` → spawn `startup_quality_once` → `run_startup_quality_once`（`:43-52`），启动即测一次（v2.4.2）
- 无条件 spawn `startup_auto_login` → `run_auto_login_on_start`（`:55-59`）
- 无条件 spawn `scheduled_actions` → `run_scheduled_action_loop`（`:61-68`）；定时登录/注销是独立功能（P2-32），不受 `enable_background_check` 管

**`start_background_check_inner`** (`tauri-app/src-tauri/src/monitor/background_task.rs:7-64`)
启动周期巡检循环：

- 幂等置 `enable_background_check=true`；`background_check_interval < 10000` 时补 15000（`:9-12`）
- spawn `background_check` 跟踪任务：首拍直接 `run_background_check`（不忙等信号量，并发由 `is_checking.try_acquire` 兜住），循环内**每拍重读** `config.background_check_interval.max(10000)` 再经 `effective_background_interval_ms` 施加轻量化下限（`:30-38`），使设置面板改间隔即时生效
- `tokio::select!` 同时等 tick 与取消信号；每拍前复查任务仍在跑且进程未退出
- 尾部 `persist::save_config_and_broadcast` 落盘并广播 `config-changed`（`:59-61`）

**`spawn_background_check_once`** (`tauri-app/src-tauri/src/monitor/background_task.rs:74-94`)
一次性巡检统一发起入口：一律经 `task_manager.spawn("background_check_once")` 注册使取消令牌全程可达（退出/关机统一取消）。`spawn` 失败（已有同名一次性巡检在跑）时复用其已注册令牌再裸 spawn 一轮（`:82-93`），并发轮次由 `run_background_check` 内部 `is_checking.try_acquire` 安全跳过。

### 还原后补检钩子（2026-10-22 新增）

**`POST_RESTORE_IP_WAIT_MAX_MS = 45_000` / `POST_RESTORE_IP_POLL_MS = 3_000`** (`tauri-app/src-tauri/src/monitor/background_task.rs:108-109`)
夜间出站还原后补检的等待上限与轮询间隔。45s 是按 2026-10-22 单次实证定档的经验值（夜间出站还原完成到用户手动登录之间没有任何 `[background]`/`[auto_login]`/`[login]` 日志；轻量化下限 300s 意味着下一拍最早在 4 分钟后）。

**`post_restore_wait_should_continue`** (`tauri-app/src-tauri/src/monitor/background_task.rs:115-117`)
纯函数 `waited_ms < max_ms`。严格小于才继续轮询：达到上限即收尾不多睡一轮；`max_ms=0` 表示不等待，首轮直接跳过循环。

**`post_restore_adapters_have_ip`** (`tauri-app/src-tauri/src/monitor/background_task.rs:123-133`)
判定「还原后网络是否就绪」：`get_adapters_force()`（`:126`，不用 cached——缓存带 TTL，网卡刚启用时会返回启用前的旧快照）→ `resolve_adapter_names` + `find_dual_adapters` → `a1.is_some() || a2.is_some()`。

**`spawn_background_check_after_outbound_restore`** (`tauri-app/src-tauri/src/monitor/background_task.rs:145-179`)
补检入口：`tauri::async_runtime::spawn` 内把阻塞枚举送 `spawn_blocking`，循环 `while post_restore_wait_should_continue(waited_ms, MAX) && !post_restore_adapters_have_ip(...)` 每轮 `thread::sleep(3s)` 并 `saturating_add` 计时（`:149-158`）。join 失败按「等满上限」处理（`unwrap_or(POST_RESTORE_IP_WAIT_MAX_MS)`，`:160-162`），宁可 warn 排障也不谎报就绪。**未读到 IP 也照发巡检**（`:177`）：让巡检自己给出「配置的适配器均无 IP」结论并走周期重试，比静默不发好排障。等满上限记 warn、就绪记 info。钩子只挂在 `finish_outbound_restore` 尾部一处（`outbound_night.rs:1110`）。

### 巡检主体

**`should_reset_logged_online`** (`tauri-app/src-tauri/src/monitor/background_check.rs:25-27`)
**2026-10-22 新增**的纯函数会话失效判定：`reachable && !any_online && has_logged_online`。三个条件缺一不可（doc `:15-24`）：`reachable`——Portal 可达才有依据确认「当前确实未在线」，探测失败/无 IP 时不复位避免瞬时抖动判死刚建立的会话；`!any_online`——主副都不在线；`has_logged_online`——此前登录过才谈得上复位。

**`run_background_check`** (`tauri-app/src-tauri/src/monitor/background_check.rs:400-408`)
async 入口：`spawn_blocking` 包阻塞主体，join 失败记 `log_error`。巡检只负责连通性/Portal/重连，质量检测由定时测试循环独占，避免双定时器叠加。

**`run_background_check_blocking`** (`tauri-app/src-tauri/src/monitor/background_check.rs:29-395`)
巡检阻塞主体，顺序即语义：

- `exit.is_quitting` / `cancel_token` 早退（`:30-32`）；`tasks.is_checking.try_acquire()` 单飞保护，抢不到直接返回（`:33-35`）
- `get_adapters_cached()` 为空/失败回退 `get_adapters_force()`（`:43-52`）
- `resolve_adapter_names` + `find_dual_adapters` 解析主/副（`:56-58`）
- **出站切换态整体跳过**：`outbound_restore_active` 为真直接 return（`:64-71`）——切换态下 Portal 检测必失败，会累计触发 MAC 重置提权与整夜告警
- **静默期**（`:72` 起）：`is_campus_check_silent` 为真则构造 `on_campus: true` 的伪结果、`cancel_campus_exit`，不跳过整拍
- `filter_operation_adapters` 后调 `check_campus_network`（`:99-100`）；写回 `current_ssid`/`on_campus_network`；`campus_check_failed = enable_network_name_check && !on_campus`（`:104`）时同时清 `any_adapter_online`/`last_a1_online`/**`has_logged_online`**（`:105-117`，是 `has_logged_online` 的复位点之一：覆盖「离校后再回校」场景）
- `campus_check_failed` 分支：emit 全 false 结果；`no_configured_ip = a1.is_none() && a2.is_none()` 时只记 info 跳过退出等网络恢复，否则 `start_campus_exit`（`:151-158`）
- 校园网通过即 `cancel_campus_exit`
- Portal 探测：`Handle::current().block_on` + 两个 `spawn_blocking` + `tokio::join!` 并行（`:170-189`）；单卡顺序探测
- `request_failed` 容错：任一适配器 `Error{is_request_failed:true}` → `handle_portal_request_failure`（`:213-234`，连续 5 次触发 MAC 重置 + DHCP 续租，阈值见 `auth/failure_tracker.rs` 的 `PORTAL_REQUEST_FAILURE_THRESHOLD`；按 `AdapterFailureCounter::A1/A2` 分适配器计数）
- 任一 `Success` → 按适配器独立清零 `a1_auth_failure_count`/`a2_auth_failure_count`（单次 `update`，`:243-261`）
- 汇总 `online`/`reachable`/`login_available`；`online_operator` 由 Portal uid 经 `operator_suffix_from_uid` 推导（`:276-280`）；`secondary_online`/`secondary_message`/`secondary_online_operator` 同口径（`:291-315`）
- `any_online = online || secondary_online == Some(true)`（`:324`）——历史缺陷修复：曾用仅主卡的 `online` 与 `any_adapter_online` 比较，「主断副通」时每拍误报「在线→离线」通知
- **会话失效重置**（`:334-340`）：`should_reset_logged_online(reachable, any_online, has_logged_online)` 为真 → 置 false + 记 info「巡检判定未在线（服务可达），已重置登录标记，允许自动登录重新介入」。位置**必须**在 `try_auto_login_on_preparation`（`:380`）之前：本次巡检就能真正登录，不必再等下一拍
- `handle_status_change`（`:341-346`）→ 组织 a1/a2 校园网消息 → `emit_background_check_result`（`:357-378`）
- `try_auto_login_on_preparation`（`:380`）→ `try_disconnect_reconnect`（`:382-386`）
- 未重连成功才 `update_network_state`（`:390-392`），避免用重连前旧 Portal 快照覆盖 `any_adapter_online`

### 自动登录决策

**`try_auto_login_on_preparation`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:29-107`)
准备自动登录，门控链（任一不过即 return）：

- `!login_available || online || !config.auto_login_on_preparation`（`:36-38`）
- **`snap.has_logged_online`（`:42-44`）**——2026-10-22 会话失效重置要解开的就是这道门
- `prep_login_failures >= PREP_LOGIN_MAX_FAILURES`（`:48-55`，达 5 次本会话停手）
- 注销保护期（`:57-61`）、冷却 `auto_login_cooldown_secs`（`:63-67`）
- `is_logging_in.try_acquire()` 登录锁（`:70`）→ 更新 `last_auto_login_attempt` → `crate::auth::service::full_login(state, app_handle, None)` → `emit_auto_login_result`

成功置 `has_logged_online=true` + `prep_login_failures=0` 并发系统通知（mascot-celebrate）；失败累加 `prep_login_failures`（日志带 `[n/5]`）；锁被占记日志跳过。

**`try_disconnect_reconnect`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:110-218`)
断线重连（10 参数）：

- `any_offline = (!online && a1.is_some()) || secondary_online == Some(false)`（`:122`）
- 闸门（`:126-128`）：`!any_adapter_online || !any_offline || !reachable || !login_available || !auto_login_on_preparation` 即返回 false
- 注销保护期（`:130-134`）、冷却（`:136-139`）
- **先拿登录锁再自增计数**（`:141-155`），读-增-取值在单次 `update_with_result` CAS 内完成，保证通知文案计数与实际自增一致
- `within_limit = reconnect_count <= config.max_disconnect_reconnect`；限内发「检测到断线」通知（mascot-alert）+ 登录日志，再 `full_login`
- 成功：清计数、置 `any_adapter_online`/`has_logged_online`、清 `prep_login_failures`，写 `login-history("reconnect")` 并发结果事件（`:180-195`）
- 超限：第 `max+1` 次发「断线重连已达上限」+「请手动登录」，之后每 `RECONNECT_REMINDER_INTERVAL`（10）次发「网络仍断线」（`:196-210`）
- 返回 `reconnect_should_report(reconnect_success, within_limit)`（`:212-217`）——历史缺陷：所有路径返回 false 会让调用方用旧快照覆盖，单适配器用户重连成功后被立即重新标离线并永久困住

**`reconnect_should_report`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:25-27`)
纯函数 `login_success && within_limit`：只有登录成功且未超限才向上报告，调用方据此跳过「用重连前旧 Portal 快照覆盖在线状态」。

**`RECONNECT_REMINDER_INTERVAL = 10` / `PREP_LOGIN_MAX_FAILURES = 5`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:14`、`:19`)
断线重连超限后的提醒间隔；准备自动登录连续失败上限（本会话停止自动登录，手动登录成功后重置）。`PREP_LOGIN_MAX_FAILURES` 对应的历史缺陷：错误凭据下无限重试 + 每 5 次认证失败触发全适配器 DHCP 释放续租，整夜断网。

**`run_auto_login_on_start`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:220-469`)
开机自启登录流程：

- `auto_login_on_start` 关闭直接返回（`:224-226`）
- `initial_delay`：`--autostart` 5000ms，否则 1500ms（`:229`）
- spawn `auto_login_on_start` 任务；退出中或 `has_logged_online` 已真则直接返回（`:240-242`）
- `get_adapters_force`（spawn_blocking）；`--autostart` 场景适配器无 IP 时最多重试 3 次 × 3000ms（`:266-281`）
- 校园网检测：静默期置 `on_campus_network=true` 跳过；检测任务 panic/取消 → 跳过本次防误踢出（`:297-309`）；不在校园 → emit 失败事件，无 IP 跳过退出否则 `start_campus_exit` + return
- Portal 探测：`check_portal_full` **只读探测不带凭据**（防错误凭据触发账号锁定，`:351-353`），双卡并行 `spawn_blocking`（`:353-366`）
- 已在线：置 `any_adapter_online`/`has_logged_online`/`prep_login_failures=0`，emit「已在线」并 return（`:381-405`）
- 未在线：`spawn_blocking` 内再查一次 `has_logged_online`（`:423-425`）→ `is_logging_in.try_acquire` → 更新 `last_auto_login_attempt` → `full_login`（`:421-432`）
- 成功：置 `has_logged_online=true` + 清失败计数；`auto_exit_after_login` 时 `start_auto_exit`（`:453-463`）

### 事件出口与状态写回

**`handle_status_change`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:68-107`)
在线↔离线翻转时的通知器：`current_online != prev_online` 才动作；**仅离线方向**且 `enable_notification` 时弹系统通知，用 `update_stats.last_network_change_notification_ms` 做 **60s 节流**（`:99-102`）；未翻转只记 debug。

**`emit_background_check_result`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:109-164`)
构造并发出 `background-check-result` 事件：`background_check_count` 在单次 `update_with_result` CAS 内自增并返回新值（`:116-119`，BE-A-10 修复：避免 load+update 两次快照重复计数）；注销保护期内强制 `online=false`、`secondaryOnline=Some(false)`、两个 `*Operator` 置 null（`:127-134`）。事件 JSON 字段：`serverAvailable`/`loginAvailable`/`online`/`message`/`adapter1Name`/`adapter2Name`/`secondaryOnline`/`secondaryMessage`/`onlineOperator`/`secondaryOnlineOperator`/`timestamp`/`checkCount`/`isRunning`/`currentSsid`/`onCampusNetwork`/`enableNetworkNameCheck`/`requiredNetworkName`/`campusWifi`/`campusWired`/`a1CampusMessage`/`a2CampusMessage`/`a1OnCampus`/`a2OnCampus`。

**`update_network_state`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:166-202`)
巡检收尾状态写回：先写 `server_available`；注销保护期内直接返回（`:180-183`）；否则写 `any_adapter_online`/`last_a1_online`，`any_online` 时清 `disconnect_reconnect_count`。最后 `reachable && !has_logged_online && online` → 置 `has_logged_online=true` + 清 `prep_login_failures`，并在 `auto_exit_on_online` 时 `start_auto_exit`（`:193-201`）——`has_logged_online` 的置位点之一。

**`build_adapter_details`** / **`adapter_status_entry`** / **`adapter_disabled_entry`** / **`adapter_disconnected_entry`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:51-65`、`:36-41`、`:43-45`、`:47-49`)
适配器详情拼接（`"名: 消息, …"`，副适配器仅 `is_secondary_adapter_enabled` 时计入）与状态 JSON 构造器（后两者是「已禁用/未找到」「未连接」的降级文案包装）。

### 校园网判定与 Portal 探测

**`check_campus_network`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:44-300`)
校园网判定主体：`enable_network_name_check` 关闭时退化为「网关可达 ∨ portal 可达」（网关禁 ping 时 portal 探测仍可判在校园）；开启时按名称名单匹配 WiFi SSID（大小写不敏感）/ 有线 profile → /18 子网检查 `is_same_subnet_18` → 网关可达性 → portal 可达性三级兜底（网关/portal 探测每轮只做一次，闭包缓存）；仅当该类网卡有 IP 才信任网关可达性（避免其他网卡可达性错误归因，`:185-187`/`:235-237`）；`on_campus = wifi.on_campus || wired.on_campus`；未通过时记 `log_warn` 便于排障（`:288-291`）。默认名单 `i-wxxy、iwxxy-2、iwxxy-3`。

**`campus_name_list`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:36-42`)
内部函数：按顿号/全半角逗号/分号/空格/Tab 切分、trim、滤空项。

**`is_campus_check_silent`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:306-314`)
静默期判定（now/start/end 均为当日分钟数）：`start == 0` 禁用门控恒 false；`now < start` 静默；`end > start && now >= end` 静默；`end <= start` 退化为仅受开始时间限制（与 `lifecycle::is_within_campus_exit_window` 退化规则一致，防误设导致整天静默）。

**`check_adapter_portal`** (`tauri-app/src-tauri/src/monitor/portal_check.rs:59-93`)
单适配器 Portal 探测，只读、绝不携带凭据；`check_portal_full(&adapter.ip, Some(&adapter.name))`，`error_kind=="request_failed"` → `Error{is_request_failed:true}` + emit login-log error；`Ok` → `Success`；`Err` → `Error{false}`。

### 夜间出站联动与定时动作

**`finish_outbound_restore`** (`tauri-app/src-tauri/src/monitor/outbound_night.rs:1096-1112`)
还原收尾，**自动还原 / 手动还原 / 启动对账三条路径的唯一汇合点**：`clear_outbound_snapshot`（`:1116-1134`，三份快照一起清、落盘成功才 store 内存）失败则 `mark_outbound_failure` 并返回 false（`:1097-1100`，让退避闸继续关住重放）；清成功才清退避状态与「立即切换」当日 hold（`:1101-1104`）；**尾部调用 `spawn_background_check_after_outbound_restore`（`:1110`）**：此时切换态已清（巡检不再被出站切换态闸门跳过）、校园网卡已启用，立刻补一拍巡检让自动登录第一时间评估。钩子只挂这一处。

**`apply_outbound_switch`** (`tauri-app/src-tauri/src/monitor/outbound_night.rs:391-583`)
切换主流程（`OutboundActionGuard` RAII 单飞闸）：选择有 IP 且边界探测判非校园的出站候选卡；**快照落盘先行**（`outbound_metric_restore`/`outbound_disabled_adapters` 先写配置再动系统，崩溃也能凭快照还原）；`write_outbound_metric` 改跃点、禁用校园卡、`best_route_if_index_v4` 验证默认路由已离开校园网关，失败进重放。

**`apply_outbound_restore`** (`tauri-app/src-tauri/src/monitor/outbound_night.rs:776-1089`)
还原主流程三步反向：待命路由清理（精确四元组匹配才删，非末条默认路由才动）、跃点写回（目标卡已消失按已还原处理）、启用校园卡（`enable_campus_adapter`，问题 22 时走 pnputil 分支）；快照损坏按「已还原」继续、只每天提醒一次（`:781-797`）；三份快照全空的非切换态直接走 `finish_outbound_restore`（`:798-808`）；全部步骤成功的正常收尾同样汇到 `finish_outbound_restore`（`:1084-1088`）。

**`manual_outbound_restore`** (`tauri-app/src-tauri/src/monitor/outbound_night.rs:1184-1197`)
手动还原：先校验处于切换态；**先清 hold 再清失败状态**（`:1189-1191`，否则还原失败后还原窗内的自动重试会被 hold 一并冻结），再调 `apply_outbound_restore`。

**`reconcile_outbound_on_startup`** (`tauri-app/src-tauri/src/monitor/outbound_night.rs:1204-1262`)
启动对账（`run_scheduled_action_loop` 启动前一次性、`spawn_blocking` 线程内，`monitor/scheduled.rs:57-58`）：三份快照任一非空说明上次留下了切换态（崩溃/被杀/睡眠跨过恢复点）——恢复窗口内或功能已关闭则立即还原；夜间窗口内只有「目标卡仍在用且判定为非校园网」才重放切换。

**`run_scheduled_action_loop`** (`tauri-app/src-tauri/src/monitor/scheduled.rs:51-593`)
定时动作循环（30s tick，`SCHEDULED_TICK_MS` `:41`）：

- 启动对账（`:57-58`）与定时注销对账 `reconcile_scheduled_logout_on_startup`（`:60`）——冷启动当日注销时刻已过只消耗额度不执行（防刚开应用被踢下线，`:201-216`）
- 循环内 `evaluate_scheduled`（共享 `evaluate_scheduled_actions` 双端单点）→ `(fire_login, fire_logout)`；**出站切换态下 `fire_login=false` 且不消耗当日标记**（还原成功后下一拍过点补触发，`:84-91`）
- 夜间出站动作 `evaluate_outbound_action` → `Switch`/`Restore`/`None`（`None` 拍做 `needs_replay` 补齐 + `watchdog_re_disable_campus`）
- 运营商夜切 `evaluate_night_switch_gated` 用**拍后** `outbound_active` 门控（切侧从热点登录 portal 必失败让位，`:161-183`）
- 注销成功分支 `perform_logout`（`:352-386`）：`is_logging_out` 互斥；成功后取消自动退出 + `failure_tracker::reset_all` + 60s 注销保护期（`has_logged_online=false`、清重连计数、`last_auto_login_attempt=now`、`logout_protected_until`，`:374-397`）——`has_logged_online` 的第二类复位点
- 运营商夜切验证 `verify_night_switch`（`:392-429`）：等 15s → GET `{portal_origin}/drcom/chkstatus?callback=dr1003` 核对 `uid_matches`（3 次重试隔 5s，`:441-486`），仍不符 → 注销 → 复登 → 复验，最终不符通知「夜切验证失败」（mascot-alert）；`portal_origin`（`:490-500`）portal_url 空时回退默认门户；`expected_operator_display`（`:503-509`）空串即「无锡学院」

**`NightOutboundAction` / `evaluate_night_outbound` / `is_night_outbound_guard_window` / `outbound_restore_active`** (`tauri-app/src-tauri/src/config/outbound_switch.rs:9-17`、`:27-44`、`:53-61`、`:68-70`)
夜间出站决策纯函数（双端单点）：`evaluate_night_outbound` 含凌晨补切（`!restore_active && now < RESTORE_START_MINUTES` → Switch）与过点补触发；守护窗口=当日切换时刻起至次日 07:30 恢复窗开，窗口内巡检不出自动启用目标；`outbound_restore_active` 三份快照（metric/disabled/standby）任一非空即切换态（兜底路由快照自 2026-10-18 事故后不再创建但仍参与判定）。

### 生命周期与通知

**`start_campus_exit` / `cancel_campus_exit` / `cancel_campus_exit_with_notification` / `start_auto_exit`** (`tauri-app/src-tauri/src/infra/lifecycle.rs:23-143`、`:146-162`、`:165-189`、`:191-272`)
生命周期倒计时（`CAMPUS_MINIMIZE_DELAY_MS=30000` / `CAMPUS_EXIT_DELAY_MS=60000`，`:12-13`）：校园网验证失败 → 先 CAS `campus_exit_started` 再设 deadline（原顺序会永久卡死，`:45-50` 注释）→ emit 倒计时 + 通知（mascot-alert）+ 注册 Ctrl+Shift+C 取消快捷键 → 30s 后 `mark_hidden_by_design` + suspend + hide（防后台空转）→ 再 30s 最终校验 deadline → detach → `shutdown_and_exit`；spawn 失败回滚 CAS + deadline（`:137-142`）。`start_auto_exit`：`auto_exit_cancelled` 已置跳过；锁内 deadline 互斥；`AUTO_EXIT_DELAY_MS` 导入自 `infra/state`（`lifecycle.rs:5`，定义文件不在本文源码集）；spawn 失败回滚 deadline（`:267-271`）。`is_within_campus_exit_window`（`:17-19`）`end <= start` 退化为仅开始限制。`shutdown_and_exit`（`:319-328`）：`is_quitting=true` → 10s 超时等 `task_manager.shutdown()`（防卡在不响应取消的 DHCP/长 HTTP）→ `app_handle.exit(0)`。

**`emit_notification`** (`tauri-app/src-tauri/src/infra/notification.rs:16-65`)
系统通知统一出口：`enable_notification` 为假直接返回；主窗口**可见且未最小化**（`monitorable = is_visible && !is_minimized`，`:25-28`；仅看 `is_focused` 会误弹、仅 `is_visible` 因 Win32 最小化仍 true）时不弹——前台信息走应用内事件/日志通道，双通道 toast 重复的历史缺陷由此根除；Windows 桌面优先自组 toast 带看板娘头像（notify-rust 不暴露图片），失败降级插件纯文本（`:45-55`）；不向前端发事件；文案中文硬编码（`:14-15`）。

### 配置常量（桌面）

`tauri-app/src-tauri/src/config/model.rs`（`Config` 全字段 `:10-176`；容器级 `#[serde(default)]` `:7-9` 缺字段补默认防整体反序列化失败丢配置）与本链路相关的默认值：

- `auto_login_on_start=true`、`enable_background_check=true`、`auto_login_on_preparation=true`、`lightweight_mode=true`
- `background_check_interval=60000`（2026-09-20 15s→60s，v3→v4 迁移）
- `max_disconnect_reconnect=3`、`auto_login_cooldown_secs=60`
- `campus_check_start_minutes=460`（07:40）、`campus_check_end_minutes=1380`（23:00 不含；2026-09-13 起 end=1380，`:208-210`）
- `campus_exit_start_minutes=480`（8:00）、`campus_exit_end_minutes=1380`
- `scheduled_login_minutes`/`scheduled_logout_minutes=1440`（禁用哨兵，0=真实 00:00，v4→v5 迁移）
- `auto_exit_after_login=false`（2026-09-20 起默认关，配合夜切）、`auto_exit_on_online=false`
- `enable_night_operator_switch=false`（2026-10-02 默认改回关）、`enable_night_outbound_switch`、`portal_url=http://10.1.99.100`、`campus_gateway=10.2.127.254`、`required_network_name=i-wxxy、iwxxy-2、iwxxy-3`、`enable_network_name_check=true`、`log_retention_days=7`、`config_version=5`
- `masked_for_display`/`mask_in_place`（`:315-330`）：密码打码唯一出口

### 安卓端（android/src-tauri/src/monitor_loop.rs）

安卓后台监控与桌面同构但独立实现，决策逻辑抽成纯函数（TDD）。模块 doc `:1-4`：tick = 校园网判定 → Portal 探测 → 掉线自动重登（cooldown + 次数上限）→ emit `background-check-result`/`login-log`。

**并发结构与状态**：

- `OUTBOUND_RESTORE_FAIL_COUNT`（`:21`）/ `OUTBOUND_RESTORE_MAX_FAILS=3`（`:23`）：自动还原失败达上限放弃、清标记恢复巡检；`OUTBOUND_SWITCH_FAIL_COUNT`（`:26`）/ `MAX_FAILS=3`（`:28`）：注销未确认达上限按断网规律建态交系统侧验证
- `MONITOR_GENERATION`（AtomicU64，`:32`）：代际 token 防 stop→start 竞速误清 running
- `LOGIN_ATTEMPT_LOCK`（tokio Mutex，`:37`）：自动登录 attempt 段互斥（tick / WiFi 事件 / 手动三入口并发），仅覆盖「判在线→should_attempt_login→run_login→计数」段、探测 IO 不持锁
- `MonitorState`（`:41-69`）：`running`/`check_count`/`consecutive_failures`/`reconnect_count`（回在线清零）/`last_login_attempt_ms`/`was_online`/`last_result`/`desired_interval_ms`（逐 tick 热更新）/`idle_interval_ms`/`logout_protected_until_ms`/`wifi_event_ms`（去抖）/`notified_online`（AtomicU8：0 未展示 1 在线 2 未连接 3 非检测时段，仅翻转 notify——每拍重建是稳态功耗点）/`scheduled_login_day`/`scheduled_logout_day`（当日序号 `num_days_from_ce`，0=从未，跨天自然重置）
- WiFi 事件常量：`WIFI_EVENT_DELAY_MS=2500`（`:74`，DHCP 未就绪 + 风暴合并窗口）、`WIFI_LOST_EVENT_DELAY_MS=0`（`:79`，断开即事实）、`WIFI_EVENT_DEBOUNCE_MS=1000`（`:82`）；`wifi_event_is_burst_start`（`:87-89`）

**决策纯函数**：

- `effective_interval_ms` (`:96-103`)：巡检分档——`base = base_ms.max(5000)`；亮屏 + WiFi 走基础间隔（默认 60s），蜂窝/息屏走闲时间隔（默认 5min）。拉长不改正确性：WiFi 事件仍即时触发、无离线证据保持记忆
- `should_attempt_login` (`:107-128`)：`online || !on_campus` → false；`!was_online && !auto_login_on_preparation` → false（启动即离线需显式开启，掉线重连只受总开关管）；`reconnect_count >= max` → false；`elapsed >= cooldown*1000`
- `status_value` (`:131-152`)：`getInitData` 的 `backgroundStatus` 与 `get_background_status` 共用出站形状：`isRunning`/`checkCount`/`consecutiveFailures`/`reconnectCount`/`lastResult`/`watcherFailed` + 展平 `online`/`message`/`onlineOperator`/`serverAvailable`/`onCampusNetwork`（首轮 emit 先于 WebView 监听建立，getInitData 是唯一启动初值来源）
- `with_vpn_hint` (`:198-204`)：VPN tun 接管时 socket 旁路失效（SO_BINDTODEVICE 在 secure VPN 或 ROM 封堵时 Denied），失败通知追加「请在 VPN 中排除本应用或临时关闭 VPN」

**命令与生命周期**：

- `start_background_check` (`:208-256`)：`interval.max(5000)` → desired；已运行 → 仅刷新间隔 + [mobile] 重发 `start_monitor` 保活（Rust 循环活着 ≠ FGS 活着，START_STICKY 幂等）；未运行 → 先启 FGS 成功才置 running（不残留假「运行中」）；`start_wifi_watcher`；`gen=GENERATION.fetch_add` → spawn `monitor_tick_loop`
- `stop_background_check` (`:259-271`)：先推进代际再清标志
- WiFi watcher（`:282-363`）：`start_wifi_watcher`（`:282-304`）失败置 `WATCHER_FAILED=true` 退化纯周期检测不静默；`handle_wifi_event`（`:325-363`）is_running 兜底 + 去抖 + lost 零延迟/其余 2500ms + 时间戳过期自杀 + `run_check_once`
- `run_startup_tasks` (`:458-529`)：对齐桌面 `watcher::run_startup_tasks`；2026-09-09 提速：三条启动链（后台检测/质量首测/自动登录）并行 spawn（质量首测 12 外网域名数秒级），就绪窗 1.5s→500ms；`current_settings` 失败 → log + emit `startup-failed` + 停 FGS（BootReceiver 可能已拉起 FGS，「运行中」通知是假的，B4h）；`set_log_retention_days`（此前重启后恒默认 7）；`night_outbound_restore` 空 → `restore_avoid_bad_wifi`（快照兜底）；质量：`enable_latency_test ? start_latency_test : check_network_quality` 一次（定时测试关闭时唯一首屏来源）
- `auto_login_on_start` (`:569-612`)：凭据空 return；`ensure_wifi_bound`（WiFi+流量同开默认路由可能落蜂窝）；`probe_with_retry`（`:535-553`，失败 3s 重试一次）→ `cache_source_ip`；不在校园跳过；在校园 → `run_login`；成功 → `was_online=true`（登录成功是在线权威证据：首拍 Unknown/Failed 保持记忆时为在线）+ `logout_protected_until_ms=0` + emit true；失败 → 系统通知「启动自动登录失败」带 VPN 提示 + emit false
- `monitor_tick_loop` (`:643` 起)：`current=max(5000)`（防误配超小间隔打爆探测）；**启动对账**：当日注销时刻已过 → 只消耗今日额度不执行（防首拍把刚打开监控的用户踢下线）；定时登录不消耗（补登录=补上线是预期）；循环：代际校验 → `run_scheduled_actions`（P2-32 置于分档与静默期之前，否则被静默期整拍吞掉）→ 间隔热更新（重建计时器）→ 分档（闲时跳拍省整轮 Portal 探测）→ `run_check_once`；退出仅代际匹配才清 running
- `ProbeWindowGuard` (`:617-626`)：Drop 必释放 WifiLock/WakeLock（含 panic unwind）；`power_state` (`:631-641`) 查询失败保守 (true,true)（漏检是缺陷）

**单次巡检 `run_check_once`** (`android/src-tauri/src/monitor_loop.rs:1339-1574`)：

- 出站切换态 → 整轮跳过（闸在 `run_check_once` 内，周期拍/WiFi 事件/手动触发三路径共用；`run_scheduled_actions` 不经此闸——晨间还原判定须在切换态下照常执行）
- 校园检测静默期（`start>0`，`now<start || (end>start && now>=end)` → 整拍跳过；状态码 3「已暂停检测(非检测时段)」仅首次进入重建通知，2026-09-13 通知停在「在线」）
- `ProbeWindowGuard` 持锁省电 → `ensure_wifi_bound` → ① 校园网判定 `probe_campus_with_ssid`（源 IP 缓存同步更新——后台链路此前不缓存，首次重登拿过期 IP 被协议拒；SSID 感知 2026-09-20）
- ② Portal 探测短路：`(Some,false)` → 「不在校园网，跳过 Portal 探测」、`(None,_)` → 「WiFi 未连接，跳过 Portal 探测」（2026-10-02：离网拍 11s→3s、WiFi 关闭直接给原因；校内 Portal 宕机仍走 HTTP 由页面裁决）
- ③ 三态消费：仅 `error_kind=None` 翻转在线——Unknown（已在线页面特征失配 GBK 乱码/严格串匹配）/Failed 非可信离线证据保持上一拍记忆（2026-09-09 修复状态点每拍打灰 + 自动重登「已经在线」）；`on_campus=false → online=false`（否则「在线」永久钉住，2026-09-13）
- ④ 状态机：在线 → `reconnect_count`/`consecutive_failures`/`logout_protected` 清零；在线翻离线且在校园 → 通知「校园网连接掉线」带 VPN 提示；自动登录两道闸：注销保护期 + `consecutive_failures>=5` 熔断（达 5 停，手动成功清零）；should → `LOGIN_ATTEMPT_LOCK`（覆盖记账→登录→计数收尾整段）→ `reconnect_count+1`；`count>=max` → 只通知「自动重连已达上限」不再 `run_login`；否则 `run_login` → 成功 log + `login_history::append(...,"auto")` + emit（系统通知仅失败侧）；协议/执行失败 → `consecutive_failures+1` + 通知
- ⑤ payload：`serverAvailable`/`loginAvailable`/`online`/`message`（在线→「在线」、wifi_off→「WiFi 未连接」）/`onlineOperator`（uid → `operator_suffix_from_uid`，与桌面 `background_emit` 同口径，安卓此前漏后端字段）/`timestamp`/`checkCount`/`isRunning`/`onCampusNetwork`/`campusMessage`/`currentSsid`/`sourceIp`/`autoLogin` → `MONITOR.last_result` → emit `background-check-result` → 常驻通知 `notify_state`（`:1578-1586`，1 在线/2 未连接/4 WiFi 未连接；3=非检测时段）仅翻转才更新
- 探测线程：`portal_probe_on_little_cores` (`:784-821`)——专用短命线程绑小核（共享池会把登录等前台任务拖到小核）；裸线程须先取 Handle 线程内 `enter`（缺 reactor context panic "there is no reactor running"，2026-09-11 真机每拍必现；与桌面 `commands/login.rs:28-33` 2026-09-05 崩溃同款处理）；`catch_unwind` 捕 panic 转 Err 回传；线程创建失败降级 `spawn_blocking`

**出站切换安卓侧**（与桌面 `outbound_night.rs` 决策共享纯函数，动作安卓自有）：

- `logout_for_outbound_switch` (`:731-756`)：前置在线检查（离线时注销必失败，跳过请求照常建态且计数清零防跨夜残留）；「离线生效」判据 `radiusOk || unbindOk`（MAC 解绑=破坏性踢下线已离线；只看 success 会把 `(false,true)` 误判失败 → 掉线重连 → 再注销整夜摆动）
- `switch_logout_unconfirmed` (`:761-778`)：fails>=3 → 清零 + warn「注销未确认生效，按断网规律建态，系统侧验证接管」→ true
- `run_scheduled_actions` (`:835-1037`)：夜间出站判定先于运营商夜切（spec §2 顺序硬约束，互斥由 `outbound_active` 门控达成）；`evaluate_outbound_action(..., 0 /* hold 安卓恒 0 */, ...)` 共享编排函数双端单点；Switch 分支（`:866-885`）：注销 → `night_outbound_restore="logged_out"` → persist → `ensure_avoid_bad_wifi`（WRITE_SECURE_SETTINGS，pm grant 后程序化确保=1，插件记原值快照晨间还原）→ `trigger_wifi_recheck`（`report_wifi_unusable`，应用侧唯一合法「让出站口切蜂窝」通道，改跃点属桌面能力）；Restore 分支（`:887-923`）：`night_switch_login` 成功 → 清标记 + `restore_avoid_bad_wifi`；失败计 3 次放弃自动还原清标记恢复巡检；动作后重读 `latest_settings`（`:927-929`，防 stale 快照复活刚清标记）
- 运营商夜切：`evaluate_night_switch_gated`（出站切换态让位）→ SwitchToCampus/Restore → `persist_settings`（`:1042-1065`，save_to 重新加密落盘 + 缓存 + R2 自动建号 + `emit_config_changed`，否则 UI 停留旧运营商后续保存用旧快照覆盖回去）→ `night_switch_login`（`:1071-1097`，直连 `run_login` 内核，无校园网探测闸——断网窗口内探测不通过会被 `auto_login_on_start` 的「不在校园网跳过」闸无声吞掉）→ `verify_night_switch`（`:1229-1283`，入口 `outbound_switch_active` → 放弃；复验轮前再查切换态；`night_switch_verify_round` `:1287-1304` 3×5s；`night_switch_check_once` `:1307-1336` GET `{origin}/drcom/chkstatus?callback=dr1003` 超时 15s）

### 安卓端配置（android/src-tauri/src/config_state.rs）

**`Settings`** (`android/src-tauri/src/config_state.rs:28-128`)
安卓全量配置（camelCase IPC 契约，对齐桌面 `Config` 可适用子集）；`password` 经 AndroidKeyStore AES-GCM 加密落盘（替代桌面 DPAPI），出站一律掩码。关键字段：`background_check_interval=60_000`、`background_check_idle_interval=300_000`（闲时巡检 5min，2026-09-13 起亮屏+WiFi 走 60s 其余拉长）、`max_disconnect_reconnect=3`、`auto_login_cooldown_secs=60`、`campus_check_start_minutes=460`/`end=1380`、`scheduled_login/logout_minutes=1440`（禁用哨兵）、`enable_night_operator_switch=false`（2026-10-02 改回）、`enable_night_outbound_switch=false`（修改系统路由属侵入性）、`night_outbound_restore`（值恒 `"logged_out"`；非空=切换态：巡检整轮跳过、运营商夜切让位、定时登录跳过）、`config_schema_version=7`。

**存储与迁移**：

- `CONFIG_IO_LOCK`（`:17-24`）：配置 IO 串行（防 save_config/switch/delete 读改写竞态；guard 跨 await 持有覆盖 load→merge→save 全序列）
- `CryptoBridge`（`:204-239`）：mobile → `campus_keystore` encrypt/decrypt；host 测试注入可逆假实现
- `EncodedSettings`（`:244-248`）：磁盘形态密文（加密失败置空绝不落明文）；`load_file`（`:250-263`）损坏/解密失败降级不崩；`password_saved_at`（`:270-276`）判据必须落 disk 形态密文位（落盘时外露 password 恒空，照 settings.password 判空永远「未保存」）
- `save_file`（`:278-308`）：tmp 名带 pid（遗留 tmp 只能被 `cleanup_stale_config_tmp` `:313-324` 清走，不参与 rename 竞争）→ write → rename
- `load_from`（`:326-336`）：migrate + 旧单值 "i-wxxy" 扩为三 SSID（与桌面 validate 2026-09-27 同语义）
- `migrate_legacy_defaults`（`:355-417`）：v0→1 15s→60s；v1→2（2026-09-10）自动化开关刷开；v2→3（2026-09-12）质量→false；v3→4（2026-09-13）补闲时间隔 300_000；v4→5（2026-09-13）检测终点 0→1380；v5→6（2026-09-20）质量间隔 60s→600s 刷等于旧默认的值；v6→7（2026-09-20）夜切 false→true + scheduled 0→1440；迁移结果含版本落盘，用户改回不再覆盖
- `masked_for_display`（`:424-430`）非空密码→`***`；`emit_config_changed`（`:436-439`）payload=`{"config":masked}`（掩码必须复用 `masked_for_display` 唯一出口）；`resolve_password_field`（`:442-451`）：clear→空、空/MASK→回退已存值、否则新值；`get_config`（`:454-466`）、`save_config`（`:469-499`，全程持 `CONFIG_IO_LOCK`）、`current_settings`（`:503-520`，缓存优先未命中读盘）

### 测试模块

**`background_task.rs` tests** (`tauri-app/src-tauri/src/monitor/background_task.rs:181-199`)
- `post_restore_wait_stops_at_cap`：0 与 42000 继续、45000 与 90000 停止——达上限即收尾不多睡一轮
- `post_restore_wait_zero_budget_never_waits`：`max_ms=0` 首轮就不进循环直接发巡检

**`background_check.rs` tests** (`tauri-app/src-tauri/src/monitor/background_check.rs:410-439`)
- `no_reset_while_online` / `no_reset_when_unreachable`（探测失败不能当会话失效）/ `no_reset_when_never_logged_in`（避免无谓写状态与日志噪音）/ `reset_when_reachable_offline_and_logged_before`（整夜休眠场景必须复位）

**`auto_auth.rs` tests** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:471-492`)
- `reconnect_success_within_limit_reports_true` / `reconnect_failure_never_reports_true` / `reconnect_success_over_limit_reports_false`

**`campus_check.rs` tests** (`tauri-app/src-tauri/src/monitor/campus_check.rs:316-355`)
- `名称名单_解析多分隔符与空项` / `静默期判定_开始时间单边` / `静默期判定_时间段双边`（[07:40, 23:00) 外静默）/ `静默期判定_结束不晚于开始时退化为单边`

**`outbound_switch.rs` tests** (`tauri-app/src-tauri/src/config/outbound_switch.rs:112-150`)
- `switch_fires_after_time_when_not_in_switched_state`（周日 1380→Switch、1379→None、已切换态→None、关闭→None）
- `restore_fires_in_restore_window_only`（450→Restore、449 保持过夜不提前还原、已还原→None）
- `predawn_missed_switch_is_replayed`（00:00-06:29 未切换→Switch 补切、切换态凌晨保持→None）

**`scheduled.rs` tests** (`tauri-app/src-tauri/src/monitor/scheduled.rs:511-593`)
- 锁定过点补触发（跨天重置次日再触发）、切换态定时登录跳过不消耗当日标记而注销照常、还原后过点补触发

**`monitor_loop.rs` tests** (`android/src-tauri/src/monitor_loop.rs:1588-1709`)
- `通知状态_wifi短路独立码`、`在线或非校园网_永不尝试`、`掉线重连_不受自动登录开关管`、`启动即离线_需显式开启自动登录`、`重连达上限_暂停`、`cooldown_未到_不重试`/`_为零_立即重试`、`wifi事件_首事件与窗口外为起点`、`巡检分档_*`（闲时小于基础取基础、下限 5s）、`portal_origin_取scheme加host`、`期望运营商展示`、`status_value_展平最近一次检测含运营商`

**`config_state.rs` tests** (`android/src-tauri/src/config_state.rs:522-826`)
- 加密落盘（raw 不含明文、读回等于原文）、解密失败置空配置仍可加载、旧默认单值 SSID 载入扩为三值、默认值业务语义、`迁移_旧默认15s升60s且落盘后用户值不被覆盖`、`迁移_非旧默认值保持不动`、camelCase 序列化契约、掩码出口、密码字段回退语义与桌面同构、空密码不写密文位、密码已保存判据落盘后仍为已保存

## Architecture

```text
setup_app (app/startup.rs:153)
  └─ run_startup_tasks (monitor/watcher.rs:16)
       ├─ enable_background_check ──► start_background_check_inner (background_task.rs:7)
       ├─ startup_auto_login ───────► run_auto_login_on_start (auto_auth.rs:220)
       └─ scheduled_actions ────────► run_scheduled_action_loop (scheduled.rs:51)
                                      └─ reconcile_outbound_on_startup (outbound_night.rs:1204)

run_background_check (background_check.rs:400)
  └─ run_background_check_blocking (:29)
       ├─ 出站切换态? (:64) / 静默期? (:72) ──► 不同粒度跳过
       ├─ check_campus_network (:100) ── fail ─► has_logged_online=false (:115) → start_campus_exit
       ├─ check_adapter_portal ×N (block_on + tokio::join! 并行, :170-189)
       ├─ any_online = online || secondary_online (:324)
       ├─ should_reset_logged_online (:334) ─► has_logged_online=false   ← 2026-10-22 新增
       ├─ emit_background_check_result (:357)
       ├─ try_auto_login_on_preparation (:380) / try_disconnect_reconnect (:382)
       └─ update_network_state (:391)

finish_outbound_restore (outbound_night.rs:1096)
  └─ spawn_background_check_after_outbound_restore (:1110)              ← 2026-10-22 新增
       └─ 等适配器 IP（≤45s，3s/轮，get_adapters_force）──► spawn_background_check_once (:74)
```

安卓端同构：`run_startup_tasks (monitor_loop.rs:458)` → `start_background_check (:208)` → `monitor_tick_loop (:643)` → `run_check_once (:1339)`，出站/夜切动作在 `run_scheduled_actions (:835)`。

## Data Flow

```text
配置(enable_background_check / interval)
  → 启动装配 run_startup_tasks（auto_login / scheduled_actions 无条件独立分支）
    → 周期 tick（桌面每拍重读间隔，轻量化下限 300s；安卓分档 60s / 5min）
      → 出站切换态? → 整拍跳过（防 MAC 重置提权与整夜告警）
      → 适配器解析（主/副，含 disabled/disconnected 降级文案）
      → 校园网判定（静默期伪通过 / SSID·有线 profile·子网·网关·portal 三级兜底）
         ├─ fail → has_logged_online=false → campus-exit-countdown → 30s 最小化 → 60s 退出
         └─ pass → cancel_campus_exit
      → Portal 探测（只读，绝不携带凭据）
         ├─ request_failed ×5 → 该适配器 MAC 重置 + DHCP 续租
         └─ Success → 清零该适配器的认证失败计数
      → any_online = 主卡在线 || 副卡在线
      → 【会话失效重置】reachable && !any_online && has_logged_online → 复位标记（同拍即可登录）
      → 状态翻转通知（仅离线方向，60s 节流）+ background-check-result 事件
      → 情况分支
         ├─ 未在线且 login 可用 → 准备自动登录（已登录过 → 熔断 → 保护期 → 冷却 → 登录锁）
         │    └─ 成功置 has_logged_online / 失败累加 prep_login_failures（≤5）
         ├─ 曾在线且现离线 → 断线重连（计数 +1，≤3 次；超限第 4 次起每 10 次提醒）
         └─ 在线恢复 → update_network_state → auto_exit_on_online? → 自动退出倒计时

夜间出站还原（自动排程 / 手动命令 / 启动对账三路）
  → apply_outbound_restore → finish_outbound_restore（唯一收尾汇合点）
    → 【还原后补检】轮询等主/副卡取得 IP（≤45s，3s/轮，force 枚举避开 cached TTL）
      → spawn_background_check_once → 同一次巡检即可触发自动登录，不必等下一个巡检周期

安卓（monitor_loop.rs）：
  启动并行装配 → FGS + monitor_tick_loop（30s 对账 → 分档 tick）
    → run_check_once：静默期/切换态闸 → 校园判定（SSID+子网+网关+portal）→ Portal 探测短路
      → 三态消费（仅 Determined 翻转在线）→ 状态机 → 两道闸（保护期+熔断）→ run_login
      → background-check-result + 常驻通知按翻转更新
```

## Connections

- [[desktop-monitor]] — `monitor/` 模块群逐文件详解
- [[android-backend]] — `monitor_loop.rs` 状态机与前台服务关系
- [[desktop-auth]] — `auth::service::full_login` 与失败计数器的实现
- [[desktop-app-lifecycle]] — `infra/lifecycle.rs` 退出流程与 `task_manager` 清理
- [[night-outbound-switch]] — 夜间出站切换/还原的完整决策与三份快照语义
- [[lightweight-mode-desktop]] — 轻量化模式的 300s / 1800s 间隔下限
- [[deliberate-background-check-skips]] — 「出站切换态整拍跳过」等刻意跳过点清单
- [[config-and-persistence]] — 间隔 / 阈值 / 时段字段的定义与迁移
- [[security-model]] — 自动登录使用内存明文凭据的边界
- [[ipc-command-surface]] — `background-check-result` / `auto-login-result` 事件的消费侧
- [[desktop-network-quality]] — `quality_scheduler` 与质量告警档位

## Known Issues

- **会话失效重置只覆盖「服务可达」这一支**：`should_reset_logged_online`（`background_check.rs:25-27`）要求 `reachable` 为真才复位。若整夜休眠后 Portal 探测本身也失败（无 IP、网关不可达），`reachable=false`，标记保持不动，自动登录仍被 `auto_auth.rs:42-44` 拦住，只能等下一拍或网络恢复。这是刻意的保守选择（避免瞬时抖动把刚建立的会话判死），代价是「最坏情况多等一个周期」。
- **`has_logged_online` 复位点分散、语义各不相同**：`background_check.rs:115`（校园网检测失败）、`background_check.rs:334-340`（2026-10-22 会话失效重置）、`commands/login.rs`（全量注销分支，`perform_full_logout_sync` `:148-178`）与 `monitor/scheduled.rs:374-397`（用户主动注销）都写同一个 bool，但只有前两处属于「系统判定会话失效」，读代码时容易误以为是同一套语义。
- **还原后补检用同步 `thread::sleep` 轮询**（`background_task.rs:149-158`）：每轮 3s、最多 15 轮，整段占用一个阻塞线程池线程。当前只在还原成功时触发一次，代价可接受；若将来给其他路径复用需改成异步定时器。
- **补检钩子的等待上限 45s 是经验值**：`POST_RESTORE_IP_WAIT_MAX_MS`（`background_task.rs:108`）按 2026-10-22 单次实证定档，DHCP 更慢的机器上会在上限处放弃并发出一拍「无 IP」巡检，仍要靠周期巡检兜底。
- **安卓端不需要这两个补丁，但原因与桌面不同**：安卓**有**夜间出站切换（`run_scheduled_actions` 内 Switch/Restore 分支，`monitor_loop.rs:866-885`/`:887-923`），但 Restore 分支本身就是直接 `night_switch_login`（`:887-890`）——还原动作即登录动作，不存在「还原后还要等巡检评估」的缺口，故补检钩子无对应场景。安卓也没有 `has_logged_online` 这个自动登录门控：它的在线记忆是 `MONITOR.was_online`（`monitor_loop.rs:41-69` 的 MonitorState 字段），只影响「启动即离线需显式开启自动登录」判定（`should_attempt_login` `:107-128`）与通知，不拦自动重登，故「会话失效重置」无对应对象。若「整夜休眠后到点不登录」在安卓侧复现，需另找门控点（`should_attempt_login` `:107`、`reconnect_count >= max_reconnect`、`consecutive_failures >= 5` 熔断是仅有的几道闸）。
- **安卓无 MAC 重置链路**：桌面在认证失败/Portal 请求失败连续 5 次时触发 DHCP 释放续租提权修复，安卓完全没有对应实现——安卓的 `consecutive_failures`（`monitor_loop.rs` run_check_once 内）只用于停止自动重登熔断。
- **静默期语义两端不同**：桌面静默期构造 `on_campus: true` 并继续走完整 Portal 探测（`background_check.rs:72` 起），安卓直接整拍 `return` 不探测（`monitor_loop.rs` run_check_once 内），但会把常驻通知重建为「已暂停检测(非检测时段)」（状态码 3）。在线状态的新鲜度两端不可比。
- **两端默认间隔已统一、迁移语义仍在**：桌面 `background_check_interval` 默认 60000（`config/model.rs`），安卓同为 60_000（`config_state.rs:131-199` Default）；但安卓只在 `config_schema_version < 1` 且旧值恰为 15000 时刷成 60000（`migrate_legacy_defaults` v0→v1，`config_state.rs:355-417`），其余历史取值不动。
- **倒计时常量硬编码**：`CAMPUS_MINIMIZE_DELAY_MS`/`CAMPUS_EXIT_DELAY_MS`（`infra/lifecycle.rs:12-13`）与 `AUTO_EXIT_DELAY_MS`（导入于 `lifecycle.rs:5`，定义在 `infra/state` 模块，不在本文源码集内）：用户只能整体开关（`campus_exit_on_fail`/`auto_exit_after_login`/`auto_exit_on_online`），不能调倒计时长度。
- **`try_disconnect_reconnect` 参数多达 10 个**（`auto_auth.rs:110-121`），`#[allow(clippy::too_many_arguments)]` 压不住可读性；`background_check.rs:382-386` 的调用点需逐个对齐位置参数，改动风险高。
