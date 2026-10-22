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

后台巡检是一条"启动装配 → 周期 tick → 校园网判定 → Portal 探测 → 状态机 → 自动登录 / 断线重连 → 生命周期倒计时 / 通知"的长链路。桌面端实现在 `tauri-app/src-tauri/src/monitor/` 模块群（`watcher` 门面 + `background_task` 调度 + `background_check` 主体），安卓端实现在单个 `monitor_loop.rs`。两端共享同一套阈值语义（连续失败 5 次熔断、重连上限 3、冷却 60s、注销保护 60s），但周期常量、检测时段门控与省电分档策略不同。

2026-10-22 修复了两个让"整夜休眠后到点不自动登录、只能手动点"的缺口，构成本文的两个核心新增点：

1. **会话失效重置**（`monitor/background_check.rs:334`）：巡检在调用 `try_auto_login_on_preparation` **之前**，若判定"服务可达 + 主副卡都不在线 + 此前登录过"，就把 `has_logged_online` 复位。此前该标记只在登录成功时置位、永不复位（唯二的复位点只覆盖校园网检测失败与用户主动注销），整夜休眠或夜间出站切换后旧会话已失效，`auto_auth.rs:42` 的"已登录过"门控会把自动登录永久拦住。放在评估之前是为了同一次巡检就能真正登录，而不是再等一拍。
2. **还原后立即补检**（`monitor/background_task.rs:145`）：夜间出站还原刚把校园网卡从设备级禁用中启用，Windows 还要数秒到数十秒才拿到 DHCP 地址。此刻立刻巡检只会读到"配置的适配器均无 IP"空转，随后要等一整个巡检周期——轻量化模式下该周期下限 300s（`app/lightweight.rs:77`）。新增钩子在 `finish_outbound_restore`（`monitor/outbound_night.rs:1096`）尾部触发，先轮询等 IP 就绪（最长 45s、3s 一轮），再发起一次性巡检。

## Key Components

### 核心结构体与枚举

**`BackgroundCheckResult`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:14-34`)
后台检测结果聚合，替代原先 16 个独立参数，避免调用方传参错位：
- `online: bool`: 主适配器在线状态
- `reachable: bool`: Portal 服务可达性
- `login_available: bool`: 是否可登录
- `message: &'a str`: 主适配器消息
- `adapter1_name: &'a str`, `adapter2_name: &'a str`: 主/副适配器名称
- `secondary_online: Option<bool>`, `secondary_message: &'a str`: 副适配器状态与消息
- `online_operator: Option<String>`: 主适配器在线时由 Portal uid 推导的运营商后缀（`""`=无锡学院、`@telecom`/`@unicom`/`@cmcc`；`None`=离线/未知）
- `secondary_online_operator: Option<String>`: 副适配器同口径运营商后缀
- `dual_adapter: bool`: 是否双适配器模式
- `config: &'a crate::config::model::Config`: 配置引用
- `campus_result: &'a CampusCheckResult`: 校园网检测结果
- `a1_campus_msg: Option<&'a str>`, `a2_campus_msg: Option<&'a str>`: 各适配器校园网消息
- `a1_on_campus: Option<bool>`, `a2_on_campus: Option<bool>`: 各适配器是否判定在校园网

**`PortalCheckResult`** (`tauri-app/src-tauri/src/monitor/portal_check.rs:6-19`)
Portal 探测结果枚举：
- `Success { online, message, reachable, login_available, uid }`: 探测成功；`uid` 是 Portal 页面携带的完整在线账号（账号+运营商后缀），用于推导 `online_operator`
- `Error { is_request_failed }`: 探测异常，`is_request_failed` 区分"请求层面失败"（可触发 MAC 重置熔断）与其他异常
- `NotFound`: 未找到适配器

其 impl 方法（同文件）：
- `online()` (`:22-27`): 仅 `Success` 时返回内部 `online`，其余一律 false
- `message()` (`:29-35`): 取展示消息，非 `Success` 返回固定文案
- `reachable()` (`:37-42`): 服务可达性
- `login_available()` (`:44-49`): 是否可登录
- `uid()` (`:51-57`): 取 `Success` 的 uid（仅主适配器用）

**`CampusCheckResult`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:15-21`)
校园网检测结果：
- `wifi: Option<ConnectionCampusStatus>`: WiFi 侧状态
- `wired: Option<ConnectionCampusStatus>`: 有线侧状态
- `on_campus: bool`: 是否在校园网（= wifi.on_campus || wired.on_campus）
- `current_ssid: Option<String>`: 当前 SSID 或有线 profile 名
- `message: String`: 描述消息

**`ConnectionCampusStatus`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:7-11`)
单连接（WiFi/有线）的校园网判定：
- `on_campus: bool`
- `name: Option<String>`
- `message: String`

**`NetworkSnapshot`** (`tauri-app/src-tauri/src/infra/state/network.rs:7-28`)
巡检状态机读写的唯一状态载体（`ArcSwap` 整份快照 + CAS 更新），字段与巡检链路一一对应：
- `server_available: bool` (`:8`): 最近一次 Portal 探测的服务可达性
- `any_adapter_online: bool` (`:9`): 任一适配器在线（`prev_online` 与通知翻转判定均用它）
- `last_a1_online: bool` (`:10`), `last_a2_online: bool` (`:11`): 主/副适配器最近在线态
- `has_logged_online: bool` (`:12`): **本次会话已登录过**的标记，自动登录门控与本次会话失效重置的对象
- `disconnect_reconnect_count: u32` (`:13`): 断线重连计数（上限 `config.max_disconnect_reconnect`）
- `background_check_count: u32` (`:14`): 巡检轮次（`update_with_result` 内自增后返回，随事件下发）
- `last_auto_login_attempt: Instant` (`:15`): 上次自动登录/重连尝试时刻，冷却判定基准
- `last_network_quality: Option<Arc<str>>` (`:18`): 最近质量档位
- `current_ssid: Option<Arc<str>>` (`:19`): 当前 SSID/profile
- `on_campus_network: bool` (`:20`): 最近校园网判定结果
- `logout_protected_until: Instant` (`:21`): 注销保护期截止点，保护期内跳过自动登录/重连并强制事件 `online=false`
- `portal_failure_count: u32` (`:22`): Portal 请求失败累计（熔断 MAC 重置）
- `a1_auth_failure_count: u32` (`:23`), `a2_auth_failure_count: u32` (`:24`): 各适配器认证失败计数，Portal 探测成功即清零
- `prep_login_failures: u32` (`:27`): 准备自动登录连续失败次数，达上限后本会话停止自动登录

### 核心函数与常量

**`setup_app`** (`tauri-app/src-tauri/src/app/startup.rs:153`)
桌面启动装配入口。其中 `:235` 单独启动更新检查循环 `start_update_check_loop`，`:240` 调用 `crate::monitor::watcher::run_startup_tasks(&app_h)` 装配巡检链路。

**`run_startup_tasks`** (`tauri-app/src-tauri/src/monitor/watcher.rs:16-69`)
启动装配门面（watcher 只保留装配与 re-export）：
- `config.enable_background_check` 为真 → spawn `startup_bg_check`（`:21-31`），内部调 `start_background_check_inner`
- `enable_network_quality && enable_latency_test` → spawn `startup_latency`（`:33-42`）
- 仅 `enable_network_quality` → spawn `startup_quality_once`（`:43-52`），启动即测一次
- 无条件 spawn `startup_auto_login` → `run_auto_login_on_start`（`:54-59`）
- 无条件 spawn `scheduled_actions` → `run_scheduled_action_loop`（`:63-68`）；定时登录/注销与巡检是两个功能，不受 `enable_background_check` 管

**`start_background_check_inner`** (`tauri-app/src-tauri/src/monitor/background_task.rs:7-64`)
启动周期巡检循环：
- 幂等置 `enable_background_check=true`；`background_check_interval < 10000` 时补 15000（`:10-12`）
- spawn `background_check` 跟踪任务，首拍直接执行（不忙等信号量，并发由 `is_checking.try_acquire` 兜住，`:20-24`）
- 循环内**每拍重读** `config.background_check_interval.max(10000)`，再经 `effective_background_interval_ms` 施加轻量化下限（`:29-38`），使设置面板改间隔即时生效
- `tokio::select!` 同时等 tick 与取消信号（`:41-47`）；每拍前复查任务仍在跑且进程未退出（`:49-51`）
- 尾部走 `persist::save_config_and_broadcast` 落盘并广播 `config-changed`（`:59-61`）

**`spawn_background_check_once`** (`tauri-app/src-tauri/src/monitor/background_task.rs:74-94`)
一次性巡检的统一发起入口：一律经 `task_manager.spawn("background_check_once")` 注册，使取消令牌全程可达（退出/关机时可统一取消）。`spawn` 失败（已有同名一次性巡检在跑）时复用其已注册令牌再裸 spawn 一轮（`:82-93`），并发轮次由 `run_background_check` 内部 `is_checking.try_acquire` 安全跳过。

**`POST_RESTORE_IP_WAIT_MAX_MS` = 45_000 / `POST_RESTORE_IP_POLL_MS` = 3_000** (`tauri-app/src-tauri/src/monitor/background_task.rs:108`、`:109`)
夜间出站还原后补检的等待上限与轮询间隔。注释给出实证：2026-10-22 08:11:20 夜间出站还原完成（校园网卡此后才重新启用），至 08:12:17 用户手动登录之间没有任何 `[background]`/`[auto_login]`/`[login]` 日志；轻量化下限 300s 意味着下一拍最早落在 08:16——还原动作本身不带"还原后立即评估"的钩子，这就是缺口所在。

**`post_restore_wait_should_continue`** (`tauri-app/src-tauri/src/monitor/background_task.rs:115-117`)
纯函数，返回 `waited_ms < max_ms`。严格小于才继续轮询：达到上限即收尾，不再多睡一轮。`max_ms=0` 表示不等待，首轮直接跳过循环。

**`post_restore_adapters_have_ip`** (`tauri-app/src-tauri/src/monitor/background_task.rs:123-133`)
判定"还原后网络是否就绪"：取配置 → **`get_adapters_force()`**（`:126`，不用 cached：缓存带 TTL，网卡刚启用时会返回启用前的旧快照，轮询因此永远等不到就绪）→ `resolve_adapter_names` + `find_dual_adapters` → `a1.is_some() || a2.is_some()`，即主或副至少一张已拿到 IP。

**`spawn_background_check_after_outbound_restore`** (`tauri-app/src-tauri/src/monitor/background_task.rs:145-177`)
**本次新增**的还原后补检入口：`tauri::async_runtime::spawn` 内把阻塞枚举送 `spawn_blocking`，循环 `while post_restore_wait_should_continue(waited_ms, MAX) && !post_restore_adapters_have_ip(...)` 每轮 `thread::sleep(3s)` 并 `saturating_add` 计时（`:149-158`）。join 失败按"等满上限"处理（`:160-162`，宁可 warn 让排障有据，也不谎报"IP 已就绪（等待 0ms）"）。等满上限记 `log_warn`（`:164-169`），就绪记 `log_info`（`:170-175`）。**未读到 IP 也照发巡检**（`:177`）：让巡检自己给出"配置的适配器均无 IP"的结论并走周期重试，比静默不发更好排障。

**`should_reset_logged_online`** (`tauri-app/src-tauri/src/monitor/background_check.rs:25-27`)
**本次新增**的纯函数会话失效判定：`reachable && !any_online && has_logged_online`。三个条件缺一不可，doc 注释（`:15-24`）写明理由：
- `reachable`：Portal 服务可达才有依据确认"当前确实未在线"；探测失败/无 IP 时不复位，避免瞬时抖动把刚建立的会话判死
- `!any_online`：主副适配器都不在线（`any_online` 语义见调用点）
- `has_logged_online`：此前确实登录过，才谈得上复位

**`run_background_check`** (`tauri-app/src-tauri/src/monitor/background_check.rs:400-408`)
async 入口：`spawn_blocking` 包阻塞主体，join 失败记 `log_error`。注释（`:397-399`）说明巡检只负责连通性/Portal/重连，质量检测由定时测试循环独占，避免双定时器叠加。

**`run_background_check_blocking`** (`tauri-app/src-tauri/src/monitor/background_check.rs:29-395`)
巡检阻塞主体，顺序即语义：
- `exit.is_quitting` / `cancel_token` 早退（`:30-32`）
- `tasks.is_checking.try_acquire()` 单飞保护，抢不到直接返回（`:33-35`）
- `get_adapters_cached()` 为空/失败回退 `get_adapters_force()`（`:43-52`）
- `resolve_adapter_names` + `find_dual_adapters` 解析主/副（`:56-58`）
- **出站切换态整体跳过**：`outbound_restore_active` 为真时直接 return（`:64-71`）——切换态下 Portal 检测必失败，会累计触发 MAC 重置提权与整夜告警（见 [[night-outbound-switch]]）
- **静默期**：`is_campus_check_silent` 为真则构造 `on_campus: true` 的伪结果、`cancel_campus_exit`、记 info（`:72-96`），不跳过整拍
- `filter_operation_adapters` 后调 `check_campus_network`（`:99-100`）
- 状态写回 `current_ssid`/`on_campus_network`；`campus_check_failed` 时同时清 `any_adapter_online`/`last_a1_online`/**`has_logged_online`**（`:104-117`，`:115` 是两处复位点之一：覆盖"离校后再回校"场景）
- `campus_check_failed` 分支：emit 结果（`online=false`）→ 主副均无 IP 则只记 info 跳过退出（`:151-154`）→ 否则 `start_campus_exit`（`:157`）→ 提前 return（`:160`）
- 校园网通过即 `cancel_campus_exit`（`:164`），再查一次取消信号（`:166-168`）
- Portal 探测：`dual_adapter` 且主副都在 → 两个 `spawn_blocking` + `tokio::join!` 并行（`:171-190`）；否则单卡顺序探测（`:191-204`）
- `request_failed` → 按适配器分别 `handle_portal_request_failure`（`:213-234`，阈值 5 次触发 MAC 重置）
- 任一 `Success` → 按适配器独立重置认证失败计数（`:242-261`，不依赖对方结果）
- 汇总 `online`/`reachable`/`login_available`，主卡无 IP 时 `online` 强制 false（`:263-274`）；`online_operator` 由 Portal uid 推导（`:275-280`）
- `prev_online` = `any_adapter_online` 快照（`:282`），`any_online = online || secondary_online == Some(true)`（`:324`）——历史缺陷修复：曾用仅主卡的 `online` 与 `any_adapter_online` 比较，"主断副通"时每拍误报"在线→离线"通知
- **会话失效重置**（`:334-340`）：`should_reset_logged_online(reachable, any_online, has_logged_online)` 为真 → 置 false + 记 info「巡检判定未在线（服务可达），已重置登录标记，允许自动登录重新介入」。位置**必须**在 `try_auto_login_on_preparation`（`:380`）之前：本次巡检就能真正登录，不必再等下一拍（否则还原后补检钩子触发的那一拍仍会被旧标记挡回）
- `handle_status_change`（`:341-346`）→ 组织 a1/a2 校园网消息与状态（`:348-355`）→ `emit_background_check_result`（`:357-378`）
- `try_auto_login_on_preparation`（`:380`）→ `try_disconnect_reconnect`（`:382-386`）
- 未重连成功才 `update_network_state`（`:390-392`），避免用重连前旧 Portal 快照覆盖 `any_adapter_online`

**`try_auto_login_on_preparation`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:29-107`)
准备自动登录。门控链（任一不过即 return）：
- `!login_available || online || !config.auto_login_on_preparation`（`:36-38`）
- **`snap.has_logged_online`（`:42-44`）**——本次会话失效重置要解开的就是这道门
- `prep_login_failures >= PREP_LOGIN_MAX_FAILURES`（`:48-55`，达 5 次本会话停手，防错误凭据下无限重试 + 周期性 DHCP 断网）
- 注销保护期（`:57-61`）
- 冷却 `auto_login_cooldown_secs`（`:63-67`）
- `is_logging_in.try_acquire()` 登录锁（`:70`）
成功后 `has_logged_online=true` + `prep_login_failures=0`（`:89-93`）并发系统通知；失败累加 `prep_login_failures`（`:100`）。

**`try_disconnect_reconnect`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:110-218`)
断线重连（10 个参数，`#[allow(clippy::too_many_arguments)]`）：
- 闸门（`:126-128`）：`!any_adapter_online || !any_offline || !reachable || !login_available || !auto_login_on_preparation` 即返回 false
- 注销保护期（`:130-134`）、冷却（`:136-139`）
- **先拿登录锁再自增计数**（`:142-155`），读-增-取值在单次 `update_with_result` CAS 内完成，保证通知文案计数与实际自增一致
- `within_limit = reconnect_count <= config.max_disconnect_reconnect`（`:156`）；限内发"检测到断线"通知 + 登录日志，再 `full_login`（`:159-195`）
- 成功：清计数、置 `any_adapter_online`/`has_logged_online`、清 `prep_login_failures`，写 `login-history("reconnect")` 并发结果事件（`:180-195`）
- 超限：释放登录锁；第 `max+1` 次发"断线重连失败"，之后每 `RECONNECT_REMINDER_INTERVAL`（10）次发"网络仍断线"（`:196-210`）
- 返回 `reconnect_should_report(reconnect_success, within_limit)`（`:217`）；历史缺陷：所有路径返回 false 会让调用方用旧快照覆盖，单适配器用户重连成功后被立即重新标离线并永久困住

**`reconnect_should_report`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:25-27`)
纯函数 `login_success && within_limit`：只有登录成功且未超限才向上报告，调用方据此跳过"用重连前旧 Portal 快照覆盖在线状态"。

**`RECONNECT_REMINDER_INTERVAL` = 10 / `PREP_LOGIN_MAX_FAILURES` = 5** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:14`、`:19`)
断线重连超限后的提醒间隔；准备自动登录连续失败上限（本会话停止自动登录，用户手动登录成功后重置）。

**`run_auto_login_on_start`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:220-469`)
开机自启登录流程：
- `auto_login_on_start` 关闭直接返回（`:224-226`）
- `initial_delay`：`--autostart` 5000ms，否则 1500ms（`:228-229`）
- spawn `auto_login_on_start` 任务；退出中或 `has_logged_online` 已真则直接返回（`:240-242`）
- `get_adapters_force` 取适配器，失败即终止并 emit 事件（`:245-256`）
- `--autostart` 场景适配器无 IP 时最多重试 3 次 × 3000ms（`:266-279`）
- 校园网检测（静默期跳过 / 失败触发 `start_campus_exit`）、Portal 探测（双卡并行）
- 已在线：置 `any_adapter_online`/`has_logged_online`/`prep_login_failures`，emit "已在线" 并 return（`:381-406`）
- 未在线：`spawn_blocking` 内再查一次 `has_logged_online`（`:423-425`）→ 拿登录锁 → `full_login`（`:431`）
- 成功：置 `has_logged_online`（`:453`）+ 清失败计数 + 可选通知；`auto_exit_after_login` 时 `start_auto_exit`（`:460-463`）

**`handle_status_change`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:68-107`)
在线↔离线翻转时的通知器：`current_online != prev_online` 才动作；**仅离线方向**且 `enable_notification` 时弹系统通知，并用 `update_stats.last_network_change_notification_ms` 做 **60s 节流**（`:92-103`）；未翻转只记 debug。

**`emit_background_check_result`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:109-164`)
构造并发出 `background-check-result` 事件：`background_check_count` 在单次 `update_with_result` CAS 内自增并返回新值（`:116-119`，避免 load+update 两次快照重复计数）；注销保护期内强制 `online=false`、`secondaryOnline=Some(false)`、两个 `*Operator` 置 null（`:126-147`），与后续 `effective_online` 口径一致；事件字段含适配器名、校园网两侧状态、`currentSsid`、`onCampusNetwork`、`isRunning` 等。

**`update_network_state`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:166-202`)
巡检收尾的状态写回：先写 `server_available`；注销保护期内直接返回（`:180-183`）；否则写 `any_adapter_online`/`last_a1_online`，`any_online` 时清断线计数（`:185-191`）。最后 `reachable && !has_logged_online && online` → 置 `has_logged_online=true` + 清 `prep_login_failures`，并在 `auto_exit_on_online` 时 `start_auto_exit`（`:193-201`）——这是 `has_logged_online` 的置位点之一（另一处在 `try_auto_login_on_preparation`，另两处在 `run_auto_login_on_start`）。

**`build_adapter_details`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:51-65`)
把主/副适配器消息拼成 `"名: 消息, 名: 消息"`；副适配器仅在 `is_secondary_adapter_enabled` 时计入。

**`adapter_status_entry` / `adapter_disabled_entry` / `adapter_disconnected_entry`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:36-41`、`:43-45`、`:47-49`)
适配器状态 JSON 构造器；后两者是"已禁用/未找到"与"未连接"的降级文案包装（`watcher.rs:14` 统一 re-export，保持 `watcher::X` 调用路径不变）。

**`check_campus_network`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:44-300`)
校园网判定主体：`enable_network_name_check` 关闭时退化为"网关可达 ∨ portal 可达"（`:48-55`）；开启时按名称名单匹配 WiFi SSID / 有线 profile → 子网检查（/18）→ 网关可达性 → portal 可达性；`on_campus = wifi.on_campus || wired.on_campus`；末尾在未通过时记 `log_warn` 便于排障（`:288-291`）。

**`is_campus_check_silent`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:306-314`)
静默期判定（now/start/end 均为当日分钟数）：`start == 0` 禁用门控；`now < start` 静默；`end > start && now >= end` 静默。`end <= start` 退化为仅受开始时间限制，与 `lifecycle::is_within_campus_exit_window` 的退化规则一致，防误设导致整天静默失效。

**`campus_name_list`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:36-42`)
校园网名称名单解析（内部函数）：按顿号/全半角逗号/分号/空格/Tab 切分、trim、滤空项。

**`check_adapter_portal`** (`tauri-app/src-tauri/src/monitor/portal_check.rs:59-93`)
单适配器 Portal 探测，只读、绝不携带凭据；结果封装为 `PortalCheckResult`。

**`emit_notification`** (`tauri-app/src-tauri/src/infra/notification.rs:16-65`)
系统通知统一出口：`enable_notification` 为假直接返回；窗口**可见且未最小化**（`monitorable`，`:29-34`）时不弹——前台信息走应用内事件/日志通道，双通道 toast 重复的历史缺陷由此根除；Windows 桌面优先自组 toast 带看板娘，失败降级 `notify-rust` 纯文本（`:45-55`）；其他平台直接走插件通知。

**`effective_background_interval_ms` / `effective_quality_interval_ms`** (`tauri-app/src-tauri/src/app/lightweight.rs:77-79`、`:82-84`)
轻量化模式下的间隔下限纯函数：巡检 `base.max(300_000)`、质量检测 `base.max(1_800_000)`；非轻量化原样返回。`is_lightweight_active` (`:26-28`) 是 `LIGHTWEIGHT_ACTIVE` 原子标志的读口。这正是"还原后不补检就要等 300s"的直接原因。

**`finish_outbound_restore`** (`tauri-app/src-tauri/src/monitor/outbound_night.rs:1096-1112`)
还原收尾，也是**自动还原 / 手动还原 / 启动对账三条路径的唯一汇合点**：先 `clear_outbound_snapshot`（`:1116-1134`，三份快照一起清、落盘成功才 store 内存），失败则 `mark_outbound_failure` 并返回 false（`:1097-1100`，让退避闸继续关住重放）；清成功才清退避状态与"立即切换"当日 hold（`:1101-1104`）；**尾部调用 `spawn_background_check_after_outbound_restore`**（`:1110`）：此时切换态已清（巡检不再被"出站切换态"闸门跳过）、校园网卡已启用，立刻补一拍巡检让自动登录第一时间评估。钩子只挂这一处。

**`apply_outbound_restore`** (`tauri-app/src-tauri/src/monitor/outbound_night.rs:776`)
还原主流程（`OutboundActionGuard` 互斥）；跃点快照损坏时按"已还原"继续、只每天提醒一次（`:781-797`）；三份快照全空的非切换态直接走 `finish_outbound_restore` 收尾（`:798-808`）；全部步骤成功的正常收尾同样汇到 `finish_outbound_restore`（`:1084-1088`）。

**`manual_outbound_restore`** (`tauri-app/src-tauri/src/monitor/outbound_night.rs:1184-1197`)
手动还原：先校验处于切换态；**先清 hold 再清失败状态**（`:1189-1191`，否则还原失败后还原窗内的自动重试会被 hold 一并冻结），再调 `apply_outbound_restore`。

**`reconcile_outbound_on_startup`** (`tauri-app/src-tauri/src/monitor/outbound_night.rs:1204`)
启动对账（循环启动前一次性、须在 `spawn_blocking` 线程内）：三份快照任一非空说明上次留下了切换态（崩溃/被杀/睡眠跨过恢复点）——恢复窗口内或功能已关闭则立即还原；夜间窗口内只有"目标卡仍在用且判定为非校园网"才重放切换。`monitor/scheduled.rs:58` 在 `run_scheduled_action_loop` 启动前调用它。

**`run_scheduled_action_loop`** (`tauri-app/src-tauri/src/monitor/scheduled.rs:51`)
定时动作循环（30s tick，`SCHEDULED_TICK_MS` `:41`）：先做启动对账（`:58`）与定时注销对账（`:60`）；循环内按 `NightOutboundAction` 分派，`Restore` 分支经 `run_outbound_blocking` 调 `apply_outbound_restore`（`:127-134`）。注销成功分支置 `has_logged_online=false` + 清重连计数 + 设 60s 注销保护（`:378-384`）——`has_logged_online` 的第二类复位点。

**`start_campus_exit` / `cancel_campus_exit` / `cancel_campus_exit_with_notification` / `start_auto_exit`** (`tauri-app/src-tauri/src/infra/lifecycle.rs:23`、`:146`、`:165`、`:191`)
生命周期倒计时：校园网验证失败 → 30s 后最小化（`CAMPUS_MINIMIZE_DELAY_MS` `:12`）→ 60s 后退出（`CAMPUS_EXIT_DELAY_MS` `:13`）；登录成功且 `auto_exit_after_login` → 20s 后退出（`AUTO_EXIT_DELAY_MS` `infra/state/mod.rs:13`）。

**配置常量** (`tauri-app/src-tauri/src/config/model.rs`)
- `background_check_interval` (`:51`)，默认 60000 (`:261`)
- `auto_login_on_preparation` (`:53`)，默认 true (`:262`)
- `auto_exit_on_online` (`:109`)，默认 false (`:277`)
- `campus_check_start_minutes` (`:156`)，默认 460（07:40，`:206`）
- `max_disconnect_reconnect` (`:169`)，默认 3 (`:222`)
- `auto_login_cooldown_secs` (`:171`)，默认 60 (`:224`)
- `outbound_restore_active` (`config/outbound_switch.rs:68`)：三份快照任一非空即为切换态

### 测试模块

**`background_task.rs` tests** (`tauri-app/src-tauri/src/monitor/background_task.rs:181-199`)
- `post_restore_wait_stops_at_cap` (`:187`): 0 与 42000 继续、45000 与 90000 停止——达到上限即收尾，不多睡一轮空等
- `post_restore_wait_zero_budget_never_waits` (`:196`): `max_ms=0` 时首轮就不进循环，直接发起巡检

**`background_check.rs` tests** (`tauri-app/src-tauri/src/monitor/background_check.rs:410-439`)
- `no_reset_while_online` (`:416`): 在线时不重置（`has_logged_online` 真假均是）
- `no_reset_when_unreachable` (`:424`): 服务不可达时不重置——探测失败不能当作会话失效
- `no_reset_when_never_logged_in` (`:430`): 未登录过时无需重置，避免无谓写状态与日志噪音
- `reset_when_reachable_offline_and_logged_before` (`:436`): 整夜休眠场景——可达 + 都不在线 + 曾登录 → 必须复位，允许自动登录介入

**`auto_auth.rs` tests** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:471-492`)
- `reconnect_success_within_limit_reports_true` (`:476`): 重连成功且未超限 → 必须返回 true
- `reconnect_failure_never_reports_true` (`:482`): 失败 never 返回 true
- `reconnect_success_over_limit_reports_false` (`:488`): 超过重连上限即使成功也不再上报

**`campus_check.rs` tests** (`tauri-app/src-tauri/src/monitor/campus_check.rs:316-355`)
- `名称名单_解析多分隔符与空项` (`:321`): 顿号/逗号/分号/Tab 混排、全空输入、单项输入
- `静默期判定_开始时间单边` (`:332`): `start=0` 禁用；早于 start 静默
- `静默期判定_时间段双边` (`:341`): 生效窗口 [07:40, 23:00) 外静默
- `静默期判定_结束不晚于开始时退化为单边` (`:350`): `end <= start` 时仅受 start 限制

## Architecture

```text
setup_app (app/startup.rs:153)
  └─ run_startup_tasks (monitor/watcher.rs:16)
       ├─ start_background_check_inner ──► 周期 tick (background_task.rs:7)
       ├─ startup_auto_login ────────────► run_auto_login_on_start (auto_auth.rs:220)
       └─ scheduled_actions ─────────────► run_scheduled_action_loop (scheduled.rs:51)
                                            └─ reconcile_outbound_on_startup (outbound_night.rs:1204)

run_background_check (background_check.rs:400)
  └─ run_background_check_blocking (:29)
       ├─ 出站切换态? (:64) / 静默期? (:72) ──► 不同的跳过粒度
       ├─ check_campus_network (:100) ── fail ──► has_logged_online=false (:115) → start_campus_exit (:157)
       ├─ check_adapter_portal ×N (:183 双卡并行 / :192 单卡)
       ├─ any_online = online || secondary (:324)
       ├─ should_reset_logged_online (:334) ──► has_logged_online=false   ← 2026-10-22 新增
       ├─ handle_status_change (:341) / emit_background_check_result (:357)
       ├─ try_auto_login_on_preparation (:380) ── has_logged_online 门控见 auto_auth.rs:42
       └─ try_disconnect_reconnect (:382) ──► update_network_state (:391)

finish_outbound_restore (outbound_night.rs:1096)
  └─ spawn_background_check_after_outbound_restore (:1110)              ← 2026-10-22 新增
       └─ 等适配器 IP（≤45s，3s/轮，get_adapters_force）──► spawn_background_check_once (background_task.rs:74)
```

## Data Flow

```text
配置(enable_background_check / interval)
  → 启动装配 run_startup_tasks（无条件的 auto_login / scheduled_actions 两条独立分支）
    → 周期 tick（桌面每拍重读间隔，轻量化下限 300s）
      → 出站切换态? → 整拍跳过（防 MAC 重置提权与整夜告警）
      → 适配器解析（主/副，含 disabled/disconnected 降级文案）
      → 校园网判定（静默期伪通过 / SSID·有线 profile·子网·网关·portal）
         ├─ fail → has_logged_online=false → campus-exit-countdown → 30000ms 最小化 → 60000ms 退出
         └─ pass → cancel_campus_exit
      → Portal 探测（只读，绝不携带凭据）
         ├─ request_failed ×5 → 该适配器 MAC 重置 + DHCP 续租
         └─ Success → 清零该适配器的认证失败计数
      → any_online = 主卡在线 || 副卡在线
      → 【会话失效重置】reachable && !any_online && has_logged_online → 复位标记（同拍即可登录）
      → 状态翻转通知（仅离线方向，60s 节流）+ background-check-result 事件
      → 情况分支
         ├─ 未在线且 login 可用 → 准备自动登录（已登录过 → 熔断 → 保护期 → 冷却 → 登录锁）
         │    └─ 成功置 has_logged_online / 失败累加 prep_login_failures
         ├─ 曾在线且现离线 → 断线重连（计数 +1，≤3 次；超限第 4 次起每 10 次提醒）
         └─ 在线恢复 → update_network_state → auto_exit_on_online? → auto-exit-countdown(20000ms)

夜间出站还原（自动排程 / 手动命令 / 启动对账三路）
  → apply_outbound_restore → finish_outbound_restore（唯一收尾汇合点）
    → 【还原后补检】轮询等主/副卡取得 IP（≤45s，3s/轮，force 枚举避开 cached TTL）
      → spawn_background_check_once → 同一次巡检即可触发自动登录，不必等下一个巡检周期
```

## Connections

- [[desktop-monitor]] — `monitor/` 模块群逐文件详解
- [[android-backend]] — `monitor_loop.rs` 状态机与前台服务关系
- [[desktop-auth]] — `auth::service::full_login` 与失败计数器的实现
- [[desktop-app-lifecycle]] — `infra/lifecycle.rs` 退出流程与 `task_manager` 清理
- [[night-outbound-switch]] — 夜间出站切换/还原的完整决策与三份快照语义
- [[lightweight-mode-desktop]] — 轻量化模式的 300s / 1800s 间隔下限
- [[deliberate-background-check-skips]] — "出站切换态整拍跳过"等刻意跳过点清单
- [[config-and-persistence]] — 间隔 / 阈值 / 时段字段的定义与迁移
- [[security-model]] — 自动登录使用内存明文凭据的边界
- [[ipc-command-surface]] — `background-check-result` / `auto-login-result` 事件的消费侧
- [[desktop-network-quality]] — `quality_scheduler` 与质量告警档位

## Known Issues

- **会话失效重置只覆盖"服务可达"这一支**：`should_reset_logged_online`（`background_check.rs:25`）要求 `reachable` 为真才复位。若整夜休眠后 Portal 探测本身也失败（无 IP、网关不可达），`reachable=false`，标记保持不动，自动登录仍被 `auto_auth.rs:42` 拦住，只能等下一拍或网络恢复。这是刻意的保守选择（避免瞬时抖动把刚建立的会话判死），代价是"最坏情况多等一个周期"。
- **复位点分散在三处、语义各不相同**：`background_check.rs:115`（校园网检测失败）、`:335`（本次新增的会话失效）、`commands/login.rs:174` 与 `monitor/scheduled.rs:380`（用户主动注销）。四处都写同一个 bool，但只有前两处属于"系统判定会话失效"，读代码时容易误以为是同一套语义。
- **还原后补检用同步 `thread::sleep` 轮询**（`background_task.rs:149-158`）：每轮 3s、最多 15 轮，整段占用一个阻塞线程池线程。当前只在还原成功时触发一次，代价可接受；若将来给其他路径复用需改成异步定时器。
- **补检钩子的等待上限 45s 是经验值**：`POST_RESTORE_IP_WAIT_MAX_MS`（`background_task.rs:108`）按 2026-10-22 单次实证定档，DHCP 更慢的机器上会在上限处放弃并发出一拍"无 IP"巡检，仍要靠周期巡检兜底。
- **安卓端不需要这两个补丁，但原因与桌面不同**：安卓**有**夜间出站切换（`NightOutboundAction`，`monitor_loop.rs:846` 引入、`:866` Switch 分支、`:887` Restore 分支），但 `Restore` 分支本身就是直接 `night_switch_login(app, &settings)`（`monitor_loop.rs:887-890`）——还原动作即登录动作，不存在"还原后还要等巡检评估"的缺口，故补检钩子无对应场景。安卓也没有 `has_logged_online` 这个自动登录门控：它的在线记忆是 `MONITOR.was_online`（`monitor_loop.rs:748` 置 false，`:745-746` 注释说明语义），只影响巡检跳过与通知，不拦自动重登，故"会话失效重置"也无对应对象。若"整夜休眠后到点不登录"在安卓侧复现，需另找门控点（`should_attempt_login` `monitor_loop.rs:107`、`reconnect_count >= max_reconnect` `:124`、`failures_capped` `:1467` 是仅有的几道闸）。
- **安卓无 MAC 重置链路**：桌面在认证失败 5 次（`auth/failure_tracker.rs:6`）与 Portal 请求失败 5 次（`:198`）时触发 DHCP 释放续租，安卓完全没有对应实现——安卓的 `consecutive_failures`（`monitor_loop.rs:1467`）只用于停止自动重登。
- **静默期语义两端不同**：桌面静默期构造 `on_campus: true` 并继续走完整 Portal 探测（`background_check.rs:72-96`），安卓直接整拍 `return` 不探测（`monitor_loop.rs:1363-1383`），但会把常驻通知重建为"已暂停检测(非检测时段)"（状态码 3，`:1375`）。在线状态本身的新鲜度两端仍不可比。
- **两端默认间隔已统一、迁移语义仍在**：桌面 `background_check_interval` 默认 60000（`config/model.rs:261`），安卓同为 60_000（`config_state.rs:155`），旧分歧已消除；但安卓只在 `config_schema_version < 1` 且旧值恰为 15000 时刷成 60000（`config_state.rs:344-345`），其余历史取值不动。
- **`CAMPUS_MINIMIZE_DELAY_MS` / `CAMPUS_EXIT_DELAY_MS` / `AUTO_EXIT_DELAY_MS` 硬编码**（`infra/lifecycle.rs:12-13`、`infra/state/mod.rs:13`）：用户只能整体开关（`campus_exit_on_fail` / `auto_exit_after_login` / `auto_exit_on_online`），不能调倒计时长度。
- **`try_disconnect_reconnect` 参数多达 10 个**（`auto_auth.rs:110-121`），`#[allow(clippy::too_many_arguments)]` 压不住可读性；`background_check.rs:382-386` 的调用点需逐个对齐位置参数，改动风险高。