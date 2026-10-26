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
  - tauri-app/src-tauri/src/auth/failure_tracker.rs
  - tauri-app/src-tauri/src/auth/portal.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/config_state.rs
tags: [概念， 后台巡检， 自动登录， 断线重连， 生命周期， 通知， 会话失效]

---

## Overview

后台巡检是一条「启动装配 → 周期 tick → 校园网判定 → Portal 探测 → 状态机 → 自动登录 / 断线重连 → 生命周期倒计时 / 通知」的长链路。桌面端实现在 `tauri-app/src-tauri/src/monitor/` 模块群（`watcher` 启动装配门面 + `background_task` 调度 + `background_check` 巡检主体 + `auto_auth` 自动登录决策 + `background_emit` 事件出口），安卓端实现在单个 `android/src-tauri/src/monitor_loop.rs`（决策逻辑抽成纯函数便于 TDD，模块 doc `monitor_loop.rs:1526-1528`）。两端共享重连上限 3（`model.rs:333` / `config_state.rs:173`）、冷却 60s（`model.rs:334` / `config_state.rs:174`）、注销保护 60s（`login.rs:172`、`scheduled.rs:55`）等阈值语义，但失败重试策略、周期常量、检测时段门控与省电分档策略不同：桌面准备自动登录连续失败达 5 次后进入**指数退避降频**（封顶 5 分钟、有界且不放弃，`prep_login_backoff_secs` 见 `auto_auth.rs:40-50`），安卓连续失败 5 次直接熔断停手（`monitor_loop.rs:1526` 的 `failures_capped` 判定，doc `:1520-1521`）。

巡检间隔本身也是动态的：周期循环每拍重读配置基础间隔并按上一轮结局调整——未登录按基础间隔 ×2 逐步退避（封顶 5 分钟）、已登录 10 分钟、已登录但门户不可达降为 1 分钟快排查（`background_task.rs:24-26` 注释，结局分类在 `background_check.rs:450-460`）；快排查档会联动一次网络质量检测确认互联网连通性（`trigger_fast_quality_check`，`background_check.rs:472-541`，5 分钟节流由 `NetworkSnapshot.last_fast_quality_at` 承载，`network.rs:33`）。

2026-10-22 修复了两个让「整夜休眠后到点不自动登录、只能手动点」的缺口，构成本文的两个核心新增点：

1. **会话失效重置**（`monitor/background_check.rs:25-27` 纯函数，`:390-396` 调用点）：巡检在调用 `try_auto_login_on_preparation`（`:436`）**之前**，若判定「服务可达 + 主副卡都不在线 + 此前登录过」，就把 `has_logged_online` 复位。此前该标记只在登录成功时置位、永不复位（唯二的复位点只覆盖校园网检测失败 `:150` 与用户主动注销），整夜休眠或夜间出站切换后旧会话已失效，`auto_auth.rs:125-127` 的「已登录过」门控会把自动登录永久拦住。放在评估之前是为了同一次巡检就能真正登录，而不是再等一拍（`:381-389` 注释）。
2. **还原后立即补检**（`monitor/background_task.rs:149-183`，钩子挂在 `monitor/outbound_night.rs:1404`）：夜间出站还原刚把校园网卡从设备级禁用（PnP problem 22）中启用，Windows 还要数秒到数十秒才拿到 DHCP 地址。此刻立刻巡检只会读到「配置的适配器均无 IP」空转，随后要等一整个巡检周期——轻量化模式下该周期下限 300s（`app/lightweight.rs:77-79`）。钩子在 `finish_outbound_restore`（`monitor/outbound_night.rs:1385-1406`）尾部触发，先轮询等 IP 就绪（最长 45s、3s 一轮），再发起一次性巡检；自动还原 / 手动还原 / 启动对账三条还原路径全部汇到这一个收尾点，钩子只挂一处（`:1394-1398` 注释）。

2026-10-24 修订（`monitor/scheduled.rs:20-22` 模块注释）补上了定时登录的第三个缺口：定时登录到点失败时按 30s 拍间隔**有限补试**（`SCHEDULED_LOGIN_MAX_RETRIES = 20`，`scheduled.rs:368`，20 次 × 30s ≈ 10 分钟窗口，评审认为原 3 次 90s 窗口太短 `:365-367` doc），失败即回退当日额度标记让「过点补触发」再试（`:353-359`），补试用尽才放弃当日——原「到点即置标记、失败也当日不重试」会让登录时段刚开的服务端瞬时拒绝直接吞掉当日登录（`:323-327` 注释）。同批修订还包括：准备自动登录达失败上限后改为指数退避（`auto_auth.rs:18-20` 历史缺陷二），断线重连超限后每 10 个周期放行一次重连（`reconnect_attempt_allowed`，`auto_auth.rs:32-35`），二者都把「达上限即永久终态」改为「有界降频、必然重试」。

2026-10-25 起自动修复策略再收敛一处：**认证失败不再触发 MAC 重置**（`failure_tracker.rs:73-76`、`:136-137`）——时间窗外/账号态异常也会返回认证失败码，以此为触发条件会在不可登录时段反复换 MAC/IP；认证失败侧只保留计数与日志（达 5 次仅告警并清零，`:77-80`、`:138-141`）。MAC 重置的唯一触发源改为「门户 HTTP 不可达」，阈值由 5 收紧为 3（`PORTAL_REQUEST_FAILURE_THRESHOLD`，`failure_tracker.rs:156-158`，触发器 `handle_portal_request_failure` `:170-263`）。

当前代码在自动登录决策上还有两处精细化的收敛：其一，**门户恢复上升沿复位退避**——`NetworkSnapshot.last_login_available`（`infra/state/network.rs:30`）记录上一检测周期的 `login_available`，`try_auto_login_on_preparation` 在所有早退**之前**做沿检测（`auto_auth.rs:94-107`）：门户从不可达恢复为可用（上升沿）视为新的登录机会，直接复位 `prep_login_failures` 退出指数退避，不必等退避周期自然到期；其二，**「可登录时间段未到」类拒绝不计入退避**——`is_time_window_refusal`（`auto_auth.rs:62-73`）按中英文关键词识别时段拒绝文案，`prep_failures_after_failure`（`auto_auth.rs:78-84`）对这类失败不累加计数（失败分支 `:187-199`），因为时间窗拒绝注定发生且窗口打开后应立即恢复，指数退避反而会把窗口开启时刻拖后最多 5 分钟。

2026-10-26 起静默期放行语义由「一次性放行」改为**持续放行**：还原时刻（`nightRestoreMinutes`，默认 07:30，`model.rs:69-70`）可配置后可能早于校园网检测静默期起点（`campusCheckStartMinutes`，默认 07:40，`model.rs:168-169`），还原后的巡检若落在静默期会被整体跳过，用户 07:30 打开电脑就是断网态。桌面标记 `RESTORE_SILENT_PASS`（`background_check.rs:42-43`，doc `:29-41`）由 `finish_outbound_restore` 在发起补检前置位（`outbound_night.rs:1403` 的 `request_check_after_outbound_restore`，实现 `background_check.rs:46-48`），巡检在进入静默分支**之前**读取（`:98`），且仅在**离开静默期**时清除（`forced && !campus_silent` → `store(false)`，`:99-102`）——即本轮静默期剩余时段持续放行，直到检测时段开始恢复常规门控，不跨日残留。安卓端有同款标记 `RESTORE_SILENT_PASS`（`monitor_loop.rs:51`，doc `:39-50`），Restore 分支成功后置位（`:936`），`run_check_once` 同样按「离开静默期才清」消费（`:1418-1422`）。

2026-10-25 另有两处跳过语义收敛：**静默期整拍跳过**——静默期原本只跳过「校园网环境验证」，Portal 登录检测、自动登录、断连重连照跑，校园网 23:00 断网后每分钟一轮整夜空转认证请求（`background_check.rs:103-118` 历史缺陷注释，`:119-127` 现行为：清 `on_campus_network`、取消退出流程后按 `Skipped` 返回）；**出站切换态整体跳过巡检**（`:84-91`）——切换态下 portal 检测走热点出站必失败，会累计触发 MAC 重置提权与整夜告警。

## Key Components

### 桌面端核心结构体与枚举

**`BackgroundCheckResult`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:14-34`)
后台检测结果聚合，替代原先 16 个独立参数，避免调用方传参错位（`:12-13` doc）：

- `online: bool` (`:15`): 主适配器在线状态
- `reachable: bool` (`:16`): Portal 服务可达性
- `login_available: bool` (`:17`): 是否可登录
- `message: &'a str` (`:18`): 主适配器消息
- `adapter1_name: &'a str` / `adapter2_name: &'a str` (`:19-20`): 主/副适配器名称
- `secondary_online: Option<bool>` (`:21`), `secondary_message: &'a str` (`:22`): 副适配器状态与消息
- `online_operator: Option<String>` (`:24`，doc `:23`): 主适配器在线时由 Portal uid 推导的运营商后缀（`""`=无锡学院、`@telecom`/`@unicom`/`@cmcc`；`None`=离线/未知）
- `secondary_online_operator: Option<String>` (`:26`，doc `:25`): 副适配器同口径运营商后缀
- `dual_adapter: bool` (`:27`): 是否双适配器模式
- `config: &'a crate::config::model::Config` (`:28`): 配置引用
- `campus_result: &'a CampusCheckResult` (`:29`): 校园网检测结果
- `a1_campus_msg: Option<&'a str>` / `a2_campus_msg: Option<&'a str>` (`:30-31`): 各适配器校园网消息
- `a1_on_campus: Option<bool>` / `a2_on_campus: Option<bool>` (`:32-33`): 各适配器是否判定在校园网

**`PortalCheckResult`** (`tauri-app/src-tauri/src/monitor/portal_check.rs:6-19`)
Portal 探测结果枚举：

- `Success { online, message, reachable, login_available, uid }` (`:7-14`): 探测成功；`uid: Option<String>` (`:13`) 是 Portal 页面携带的完整在线账号（账号+运营商后缀，`:12` doc），用于推导 `online_operator`
- `Error { is_request_failed }` (`:15-17`): 探测异常，`is_request_failed` (`:16`) 区分「请求层面失败」（可累计触发 MAC 重置熔断）与其他异常
- `NotFound` (`:18`): 未找到适配器

impl 访问器（`:21-57`）：`online()` (`:22-27`)、`message()` (`:29-35`)、`reachable()` (`:37-42`)、`login_available()` (`:44-49`)、`uid()` (`:51-56`)——均仅在 `Success` 分支返回真实值，其余分支返回 false/固定文案（「检测失败」`:32`、「未找到主适配器」`:33`）/None。构造器 `check_adapter_portal`（`:59-92`）：`error_kind == request_failed` 时发 login-log 并返回 `Error { is_request_failed: true }`（`:65-72`），普通异常返回 `Error { is_request_failed: false }`（`:83-91`）。

**`CampusCheckResult`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:35-43`)、**`ConnectionCampusStatus`** (`:8-12`) 与 **`AdapterCampusStatus`** (`:24-31`)
校园网判定结果（serde camelCase `:7`、`:23`、`:34`）：`CampusCheckResult` 字段 `wifi: Option<ConnectionCampusStatus>` (`:36`)、`wired: Option<ConnectionCampusStatus>` (`:37`)、`on_campus: bool` (`:38`)、`current_ssid: Option<String>` (`:39`)、`message: String` (`:40`)、`adapters: Vec<AdapterCampusStatus>` (`:42`，doc `:41`：逐卡判定明细，UI 按适配器名取用，替代按 wifi/wired 整类取用）。`ConnectionCampusStatus` 携带 `on_campus` (`:9`)/`name` (`:10`)/`message` (`:11`)。`AdapterCampusStatus` 字段 `name`/`wireless`/`on_campus`/`matched_by: Option<String>`（命中判定腿 `name`/`subnet`/`gateway`/`portal`，`:29`，doc `:28`）/`message`，doc `:14-21` 说明 2026-10-26 逐卡改造背景：整类聚合会把有 IP 卡的结论套到无 IP 卡上，且不绑源的探测在多网卡下自相矛盾。

**`NetworkSnapshot`** (`tauri-app/src-tauri/src/infra/state/network.rs:7-34`)
巡检状态机读写的唯一状态载体（`NetworkState = ArcSwap<NetworkSnapshot>`，struct `:63-65`；`new` `:74-78`；`load` 快照读取 `:81-83`；`update` CAS 循环 `:86-100`，`update_with_result` CAS 并返回闭包结果防 TOCTOU `:106-121`，doc `:102-105`）。字段与巡检链路一一对应：

- `server_available: bool` (`:8`): 最近一次 Portal 探测的服务可达性
- `any_adapter_online: bool` (`:9`): 任一适配器在线（状态翻转通知与复位判定均用它）
- `last_a1_online: bool` (`:10`), `last_a2_online: bool` (`:11`): 主/副适配器最近在线态
- `has_logged_online: bool` (`:12`): **本次会话已登录过**标记，自动登录门控与 2026-10-22 会话失效重置的对象
- `disconnect_reconnect_count: u32` (`:13`): 断线重连计数（上限 `config.max_disconnect_reconnect`）
- `background_check_count: u32` (`:14`): 巡检轮次（`update_with_result` 内 CAS 自增后返回，随事件下发）
- `last_auto_login_attempt: Instant` (`:15`): 上次自动登录/重连尝试时刻，冷却判定基准
- `last_network_quality: Option<Arc<str>>` (`:18`), `current_ssid: Option<Arc<str>>` (`:19`): 用 `Arc<str>` 防 CAS 循环整份快照深拷贝（`:16-17` 注释）
- `on_campus_network: bool` (`:20`): 最近校园网判定结果
- `logout_protected_until: Instant` (`:21`): 注销保护期截止点，保护期内跳过自动登录/重连并强制事件 `online=false`
- `portal_failure_count: u32` (`:22`): 认证失败累计（2026-10-25 起仅供诊断，不再触发 MAC 重置）
- `a1_auth_failure_count: u32` (`:23`), `a2_auth_failure_count: u32` (`:24`): 各适配器失败计数（既承接认证失败也承接 Portal 请求失败），Portal 探测成功即清零
- `prep_login_failures: u32` (`:27`，doc `:25-26`): 准备自动登录连续失败次数（会话内计数，登录成功/重置后清零）。**注意**：`:26` 注释仍写「达到上限后本会话停止自动登录」，但当前实现已改为指数退避（见 `auto_auth.rs:40-50`），注释是旧语义残留
- `last_login_available: bool` (`:30`，doc `:28-29`): 上一检测周期的 `login_available`；`false→true` 上升沿表示门户重新可达（新的登录机会），用于复位 `prep_login_failures` 退避。`Default` 中初始为 `true`（`:56`）
- `last_fast_quality_at: Option<Instant>` (`:33`，doc `:31-32`): 「已登录但门户不可达」快排查模式下上次联动质量检测的时间，距上次不足 300s 不再触发

**`AdapterOnlineStatus`** (`tauri-app/src-tauri/src/commands/login.rs:9-16`)
注销后的在线复核结果：`any_online` (`:10`)/`a1_online` (`:11`)/`a2_online` (`:12`)/`probe_ok` (`:15`)。`probe_ok=false` 表示存在探测异常——离线结论不可信，不得断言「已确认离线」（审计 A8，`:13-14` 注释）。由 `check_any_adapter_online` (`login.rs:18-60`) 构造：并行探测双适配器 Portal（`std::thread::scope` + 裸线程先进入 tauri runtime context `:29-33`，scope 并行 `:40-50`；未启用副卡时 `a2_probe=Some(false)` 不参与判定 `:46-48`），`probe_ok = a1_probe.is_some() && a2_probe.is_some()`（`:58`）。

**`PortalStatus`** (`tauri-app/src-tauri/src/auth/portal.rs:85-95`)
`check_portal_full` 的返回结构：`reachable` (`:86`)/`login_available` (`:87`)/`online` (`:88`)/`message` (`:89`)/`uid`（`:92`，在线时 Portal 页面携带的完整账号，doc `:90-91`）/`error_kind`（`:94`，`request_failed`/`need_manual_check`）。

### 桌面端启动与调度

**`setup_app`** (`tauri-app/src-tauri/src/app/startup.rs:161-272`)
桌面启动装配入口。日志目录先探测安装目录 `logs/` 可写、否则回退数据目录（`:173-186`）；logger 先于 config 初始化（`:188-195`）；`set_log_retention_days(config.log_retention_days)`（`:221`）；`silent_start = config.hidden_start && args 含 --autostart`（`:226-231`）；随后依次 build_tray（`:233`）、`start_adapter_watch`（`:236-238`）、`start_cache_refresh_task`（`:239-241`）、`start_update_check_loop` + `cleanup_stale_update_dir`（`:243-246`）、**`run_startup_tasks`**（`:248`）、`record_webview2_runtime_version`/`subscribe_process_failed`（`:252-254`）、`spawn_heartbeat_thread`/`spawn_window_safety_thread`（`:256-257`）、GPU-warmup 后台线程（`:261-269`）。

**`run()` 命令注册** (`tauri-app/src-tauri/src/app/startup.rs:23-159`)
`background` 命令组 4 个命令 `start_background_check`/`stop_background_check`/`trigger_background_check`/`get_background_status` 在 `startup.rs:98-101` 注册；`RunEvent::ExitRequested` 拦截轻量化退出（`take_lightweight_exit_guard` `:148` + `should_prevent_exit` `:153-154`，整体 `:144-158`；纯函数判定见 `app/lightweight.rs:44-46`，退出守卫单次语义 `:34-40`）。

**轻量化运行态** (`tauri-app/src-tauri/src/app/lightweight.rs`)
全部为内存态不落盘（`:1-4` 模块 doc）。与本链路相关：`LIGHTWEIGHT_ACTIVE`（`:10`）、`is_lightweight_active`/`set_lightweight_active`（`:26-32`）、退出守卫 `EXPECT_LIGHTWEIGHT_EXIT`（`:14`）与 `arm/take_lightweight_exit_guard`（`:34-40`）、`should_prevent_exit(expecting, is_quitting)`（`:44-46`，仅当次期待且未在主动退出流程时拦截）；**检测间隔下限** `effective_background_interval_ms`（`:77-79`，轻量化时 `base_ms.max(300_000)`）与质量检测下限 `effective_quality_interval_ms`（`:82-84`，`max(1_800_000)`）；ready 信号 `signal_window_ready`/`take_window_ready`/`wait_window_ready_and_show`（`:87-127`）。

**`run_startup_tasks`** (`tauri-app/src-tauri/src/monitor/watcher.rs:16-72`)
启动装配门面（watcher 只保留装配与 re-export：`background_task::start_background_check_inner` 经 `watcher.rs:14`、`campus_check::check_campus_network` 经 `:13`、`background_emit` 的三个条目构造器经 `:14` re-export，保持外部 `watcher::X` 调用路径不变，`:7-9`/`:12` 注释）：

- `config.enable_background_check` 为真 → spawn `startup_bg_check`（`:21-31`），内部调 `start_background_check_inner`
- 仅 `enable_network_quality` → spawn `startup_quality_once` → `run_startup_quality_once`（`:36-43`），启动即测一次（2026-10-25 需求修订，`:33-35` 注释：不再以定时测试开关为前提，与定时循环首轮并发由 `is_quality_checking` 信号量互斥）
- `enable_network_quality && enable_latency_test` → spawn `startup_latency` → `spawn_latency_test_loop`（`:45-55`，间隔由循环内每轮重读 `:48` 注释）
- 无条件 spawn `startup_auto_login` → `run_auto_login_on_start`（`:57-62`）
- 无条件 spawn `scheduled_actions` → `run_scheduled_action_loop`（`:66-71`）；定时登录/注销是独立功能（P2-32），不受 `enable_background_check` 管（`:64-65` 注释）

**`start_background_check_inner`** (`tauri-app/src-tauri/src/monitor/background_task.rs:7-68`)
启动周期巡检循环：

- 幂等置 `enable_background_check=true`；`background_check_interval < 10000` 时补 15000（`:8-13`）
- spawn `background_check` 跟踪任务（`:16-59`）：首拍直接 `run_background_check`（不忙等信号量，并发由 `is_checking.try_acquire` 兜住，`:18-22`），创建 `DynamicInterval`（`:27`）并执行首轮检测（`:28`），循环内**每拍重读** `config.background_check_interval.max(10000)`（`:34`）再经 `effective_background_interval_ms` 施加轻量化下限（`:37-40`，下限只作用于退避基础值、不抬快排查档，`:35-36` 注释），交给 `dyn_interval.next_ms(last_outcome, base_eff)`（`:41`），使设置面板改间隔即时生效
- `tokio::select!` 同时等 tick 与取消信号（`:45-51`）；每拍前复查任务仍在跑且进程未退出（`:52-55`）
- 尾部 `persist::save_config_and_broadcast` 落盘并广播 `config-changed`（`:63-65`）

**`spawn_background_check_once`** (`tauri-app/src-tauri/src/monitor/background_task.rs:78-98`)
一次性巡检统一发起入口（审计 A9，`:70-77` doc）：一律经 `task_manager.spawn("background_check_once")` 注册使取消令牌全程可达（退出/关机统一取消；旧实现在三处调用点拿孤儿令牌裸 spawn，任务不可取消）。`spawn` 失败（已有同名一次性巡检在跑）时复用其已注册令牌再裸 spawn 一轮（`:86-97`），并发轮次由 `run_background_check` 内部 `is_checking.try_acquire` 安全跳过。

### 还原后补检钩子（2026-10-22 新增）

**`POST_RESTORE_IP_WAIT_MAX_MS = 45_000` / `POST_RESTORE_IP_POLL_MS = 3_000`** (`tauri-app/src-tauri/src/monitor/background_task.rs:112-113`)
夜间出站还原后补检的等待上限与轮询间隔。45s 是按 2026-10-22 单次实证定档的经验值（`:100-111` doc：还原 08:11:20 完成、08:12:17 用户手动登录，其间没有任何 `[background]`/`[auto_login]`/`[login]` 日志；轻量化下限 300s 意味着下一拍巡检最早落在 08:16）。

**`post_restore_wait_should_continue`** (`tauri-app/src-tauri/src/monitor/background_task.rs:119-121`)
纯函数 `waited_ms < max_ms`。严格小于才继续轮询：达到上限即收尾不多睡一轮（doc `:115-118`）；`max_ms=0` 表示不等待，首轮直接跳过循环（测试 `:199-202`）。

**`post_restore_adapters_have_ip`** (`tauri-app/src-tauri/src/monitor/background_task.rs:127-137`)
判定「还原后网络是否就绪」：`get_adapters_force()`（`:130`，不用 cached——缓存带 TTL，网卡刚启用时会返回启用前的旧快照，`:123-126` doc）→ `resolve_adapter_names` + `find_dual_adapters` → `a1.is_some() || a2.is_some()`（`:136`）。

**`spawn_background_check_after_outbound_restore`** (`tauri-app/src-tauri/src/monitor/background_task.rs:149-183`)
补检入口（doc `:139-148`）：`tauri::async_runtime::spawn` 内把阻塞枚举送 `spawn_blocking`，循环 `while post_restore_wait_should_continue(waited_ms, MAX) && !post_restore_adapters_have_ip(...)` 每轮 `thread::sleep(3s)` 并 `saturating_add` 计时（`:153-161`）。join 失败按「等满上限」处理（`unwrap_or(POST_RESTORE_IP_WAIT_MAX_MS)`，`:163-166`），宁可 warn 排障也不谎报就绪。**未读到 IP 也照发巡检**（`:181`）：让巡检自己给出「配置的适配器均无 IP」结论并走周期重试，比静默不发好排障（`:147-148` doc）。等满上限记 warn（`:168-173`）、就绪记 info（`:174-180`）。钩子只挂在 `finish_outbound_restore` 尾部一处（`outbound_night.rs:1404`）。

**`finish_outbound_restore`** (`tauri-app/src-tauri/src/monitor/outbound_night.rs:1385-1406`)
还原收尾：先清切换态快照（`clear_outbound_snapshot` `:1386`），确认清掉之后才清退避状态（`:1390`）与「立即切换」hold（`clear_outbound_manual_hold` `:1393`）；随后 `request_check_after_outbound_restore()`（`:1403`）+ `spawn_background_check_after_outbound_restore(app_handle)`（`:1404`）。所有还原路径（自动/手动/启动对账）都汇到此处，钩子只挂一次（`:1394-1398` 注释）；2026-10-26 起还原时刻可配置后可能早于静默期起点，先立放行标记再发起补检（`:1400-1402` 注释）。

### 巡检主体

**`should_reset_logged_online`** (`tauri-app/src-tauri/src/monitor/background_check.rs:25-27`)
**2026-10-22 新增**的纯函数会话失效判定：`reachable && !any_online && has_logged_online`。三个条件缺一不可（doc `:15-24`）：`reachable`——Portal 可达才有依据确认「当前确实未在线」，探测失败/无 IP 时不复位避免瞬时抖动判死刚建立的会话；`!any_online`——主副都不在线；`has_logged_online`——此前登录过才谈得上复位。单测锁定 `:557-586`。

**`RESTORE_SILENT_PASS` / `request_check_after_outbound_restore`** (`tauri-app/src-tauri/src/monitor/background_check.rs:42-43`、`:46-48`)
**2026-10-26 持续放行标记**（doc `:29-41`）：还原窗口起点可配置（`nightRestoreMinutes`，默认 07:30）而静默期起点是另一项设置（`campusCheckStartMinutes`，默认 07:40），默认配置下还原时刻本身就落在静默期内，还原完成后的补检会被整体跳过。语义（用户定，替代原一次性放行）：还原后**在本轮静默期的剩余时段内持续放行**，直到检测时段开始才恢复常规门控——只放一轮的话，若那轮因校园网尚未恢复而登录失败，后续轮次又被静默期挡回。消费点在巡检进入静默分支之前：`:98` 读取，`:99-102` 仅当 `forced && !campus_silent`（离开静默期）时清除，放行期由「离开静默期」自动终结、不跨日残留。

**`run_background_check`** (`tauri-app/src-tauri/src/monitor/background_check.rs:546-555`)
async 入口：`spawn_blocking` 包阻塞主体，join 失败记 `log_error` 转 `CheckOutcome::Skipped`（`:551-554`）。巡检只负责连通性/Portal/重连，质量检测由定时测试循环独占（`:543-545` doc：`latency.rs` 的 `spawn_latency_test_loop` → `quality_scheduler::run_quality_check`），避免双定时器叠加、60s 节流吞掉用户自定义间隔（2026-09-04 收敛）。

**`run_background_check_blocking`** (`tauri-app/src-tauri/src/monitor/background_check.rs:50-467`)
巡检阻塞主体，顺序即语义：

- `exit.is_quitting` / `cancel_token` 早退（`:51-53`）；`tasks.is_checking.try_acquire()` 单飞保护，抢不到直接返回（`:54-56`）
- `get_adapters_cached()` 为空/失败回退 `get_adapters_force()`（`:63-73`）
- `resolve_adapter_names` + `find_dual_adapters` 解析主/副（`:77-79`）
- **出站切换态整体跳过**：`outbound_restore_active` 为真直接 return（`:84-91`，注释 `:82-83`）——切换态下 Portal 检测必失败，会累计触发 MAC 重置提权与整夜告警（见 decisions/night-outbound-switch）
- **静默期判定与放行**（`:92-133`）：`is_campus_check_silent` 读取（`:92-96`）；`:98` 读 `RESTORE_SILENT_PASS`，离开静默期时清除（`:99-102`）；`campus_silent && !forced` → `cancel_campus_exit`（`:120`）、清 `current_ssid` 并置 `on_campus_network=true`（`:121-124`）、按 `Skipped` 返回（`:126`）——不累计退避、不更新 online 状态（`:103-118` 历史缺陷注释与保留动作说明）；`campus_silent && forced` 记 debug「夜间出站还原后静默期放行」（`:128-133`）
- `filter_operation_adapters` 后调 `check_campus_network`（`:135-136`）；写回 `current_ssid`/`on_campus_network`；`campus_check_failed = enable_network_name_check && !on_campus`（`:139`）时同时清 `any_adapter_online`/`last_a1_online`/**`has_logged_online`**（`:140-152`，复位点 `:150`，历史缺陷注释 `:146-149`：覆盖「离校后再回校」场景）
- `campus_check_failed` 分支：emit 全 false 结果（`:164-185`）；`no_configured_ip = a1.is_none() && a2.is_none()` 时只记 info 跳过退出等网络恢复（`:187-189`），否则 `start_campus_exit`（`:192`），最后 `NotOnline`（`:195`）
- 校园网通过即 `cancel_campus_exit`（`:199`）
- Portal 探测：`Handle::current().block_on` + 两个 `spawn_blocking` + `tokio::join!` 并行（`:206-224`，`:208-211` 注释解释为何不用 `std::thread::scope`）；单卡顺序探测（`:225-239`）
- `request_failed` 容错：任一适配器 `Error{is_request_failed:true}` → 仅当 `portal_reset_window_open(now_min)`（每日 08:30 后启用，夜间失败不累计、窗口未开先清零残留计数，`:249-252`、`:273-289`）时调 `handle_portal_request_failure`（主 `:259-264`、副 `:267-272`，按 `AdapterFailureCounter::A1/A2` 分适配器计数）。**2026-10-25 起这是 MAC 重置的唯一触发源**，连续 3 次触发（`:243` 注释仍写「连续5次」，已过期，见 Known Issues）
- 任一 `Success` → 按适配器独立清零 `a1_auth_failure_count`/`a2_auth_failure_count`（单次 `update`，`:299-317`，对齐 failure_tracker「登录成功即重置」语义 `:292-294` 注释）
- 汇总 `online`/`reachable`/`login_available`（`:319-321`）；`online_operator` 由 Portal uid 经 `operator_suffix_from_uid` 推导（`:331-336`）；`secondary_online`/`secondary_message`/`secondary_online_operator` 同口径（`:347-371`）
- `any_online = online || secondary_online == Some(true)`（`:380`）——历史缺陷修复注释（`:375-379`）：曾用仅主卡的 `online` 与 `any_adapter_online` 比较，「主断副通」时每拍误报「在线→离线」通知
- **会话失效重置**（`:390-396`）：`should_reset_logged_online(reachable, any_online, has_logged_online)` 为真 → 置 false + 记 info「巡检判定未在线（服务可达），已重置登录标记，允许自动登录重新介入」。位置**必须**在 `try_auto_login_on_preparation`（`:436`）之前：本次巡检就能真正登录，不必再等下一拍（`:381-389` 注释：`:150` 的重置只覆盖校园网检测失败这一种失效，覆盖不到「网络名没变、只是会话超时或网卡被禁用后重启」的整夜休眠场景）
- `handle_status_change`（`:397-402`）→ 组织 a1/a2 校园网消息（`:404-411`）→ `emit_background_check_result`（`:413-434`）
- `try_auto_login_on_preparation`（`:436`）→ `try_disconnect_reconnect`（`:438-442`）
- 未重连成功才 `update_network_state`（`:444-448`），避免用重连前旧 Portal 快照覆盖 `any_adapter_online`
- **结局分类**（`:450-463`）：`reachable && any_online` → `Online`（10 分钟慢拍）；`!reachable && prev_online` → `OnlineButPortalUnreachable`（1 分钟快排查）；其余 `NotOnline`；快排查档联动 `trigger_fast_quality_check`（`:461-463`）

**`trigger_fast_quality_check`** (`tauri-app/src-tauri/src/monitor/background_check.rs:472-541`)
「已登录但门户不可达」快排查的联动质量检测（doc `:469-471`）：先经 `update_with_result` 检查 `last_fast_quality_at` 做 **5 分钟节流**（`:478-491`）；`select_adapter_for_quality` 选卡（`:492-496`）后 spawn `quality_scheduler::run_quality_check`（`:503-504`），以 `averageExternalLatency > 0` 判定互联网可达（`:508-513`）：可达只写 login-log「继续观察」（`:516-522`）；全部外网目标失败则通知「互联网不可访问」（mascot-warn，`:523-535`）；与手动检测互斥未执行时下轮重试（`:536-539`）。

### 自动登录决策

**`RECONNECT_REMINDER_INTERVAL = 10`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:13`)
断线重连超限后的提醒/放行间隔。

**`PREP_LOGIN_MAX_FAILURES = 5` / `PREP_LOGIN_BACKOFF_CAP_SECS = 300`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:21`、`:27`)
准备自动登录连续失败上限与退避封顶（5 分钟）。`PREP_LOGIN_MAX_FAILURES` 对应的历史缺陷一：错误凭据下无限重试 + 每 5 次认证失败触发全适配器 DHCP 释放续租，应用无人值守时周期性断网持续一整夜（`:16-17` 注释）；历史缺陷二：达上限后「本会话永久停止自动登录」，可登录时间窗到达后依旧不尝试——现为指数退避、有界且不放弃（`:18-20` 注释）。

**`reconnect_attempt_allowed`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:32-35`)
纯函数 `reconnect_count <= max_reconnect || (reconnect_count - max_reconnect) % RECONNECT_REMINDER_INTERVAL == 0`：上限内照常；超限后每 10 个计数放行一次重连（降频重试而非进入「只提醒、永不重连」终态），重连成功后计数复位（`:29-31` doc）。

**`prep_login_backoff_secs`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:40-50`)
失败退避时长纯函数：未达上限用普通冷却 `base_cooldown_secs`（`:41-43`）；恰达上限即开始翻倍（`excess = failures - MAX + 1` 封顶 11，`:45`），`base << excess` 后 `min(300).max(base)`（`:46-49`）——保证登录时间窗到达后最多等一个退避周期必然重试（`:37-39` doc）。60s 基准下：5 次→120s、6 次→240s、7 次→300s……封顶 300s。

**`prep_login_cooldown_ready`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:53-55`)
纯函数：距上次尝试的耗时达到当前退避冷却即可重试（doc `:52`）。`auto_auth.rs:664-671` 的测试锁定核心断言：失败拉满后冷却期满**必须**放行重试。

**`is_time_window_refusal`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:62-73`)
纯函数：登录失败消息是否为「可登录时间段未到」类拒绝。匹配 6 个中文关键词（「时间段」「时段」「时间范围」「服务时间」「时间未到」「不在允许」，`:67`）与英文 `time range` / `not allowed`+`time`（`:71-72`）。门户检测的 `login_available` 只表示「未在线即可尝试」，不是时段信号——时间窗外检测仍报可登录，登录必然被服务端拒绝（`:57-61` doc）。

**`prep_failures_after_failure`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:78-84`)
纯函数：时间窗拒绝不累加失败计数（保持基础冷却轻探测，窗口打开后下一个检测周期即可登录），其余失败正常累加进入退避（doc `:75-77`）。

**`reconnect_should_report`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:90-92`)
纯函数 `login_success && within_limit`：只有登录成功且未超限才向上报告，调用方据此跳过「用重连前旧 Portal 快照覆盖在线状态」（`:86-89` doc）。

**`try_auto_login_on_preparation`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:94-203`)
准备自动登录，门控链：

- **`login_available` 沿检测**（`:104-117`）：在所有早退之前同步 `last_login_available`（反向沿也要记录，否则离线期间标志滞留 true、门户恢复时上升沿无从识别，`:101-103` 注释）；上升沿（门户重新可达）且此前有失败计数 → 复位 `prep_login_failures=0` 退出退避（`:107-113`）
- `!login_available || online || !config.auto_login_on_preparation`（`:119-121`）
- **`snap.has_logged_online`（`:125-127`）**——2026-10-22 会话失效重置要解开的就是这道门
- `prep_login_failures >= PREP_LOGIN_MAX_FAILURES`（`:131-138`）：**只记 debug 退避日志，不再 return**——达上限后进入指数退避降频（`:129-130` 注释明确「不再永久停止」）
- 注销保护期（`:140-144`）、冷却 `auto_login_cooldown_secs`（`:146-157`，冷却时长按当前失败计数经 `prep_login_cooldown_ready` 计算退避）
- `is_logging_in.try_acquire()` 登录锁（`:160`）→ 更新 `last_auto_login_attempt`（`:162`）→ `crate::auth::service::full_login(state, app_handle, None)`（`:163`）→ `emit_auto_login_result`（`:171-177`）

成功置 `has_logged_online=true` + `prep_login_failures=0` 并发系统通知（mascot-celebrate，`:179-186`）；失败分支（`:187-199`）**例外处理**：`is_time_window_refusal` 命中时只记 info「不计入退避」（`:192-193`），其余失败经 `prep_failures_after_failure` 累加 `prep_login_failures`（`:195`，日志带 `[n/5]` `:196-197`）；锁被占记日志跳过（`:200-202`）。

**`try_disconnect_reconnect`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:206-315`，10 参数，`#[allow(clippy::too_many_arguments)]` `:205`)
断线重连：

- `any_offline = (!online && a1.is_some()) || secondary_online == Some(false)`（`:218`）
- 闸门（`:222-224`）：`!any_adapter_online || !any_offline || !reachable || !login_available || !auto_login_on_preparation` 即返回 false
- 注销保护期（`:226-230`）、冷却（`:232-235`）
- **先拿登录锁再自增计数**（`:238-244`，`:237` 注释防锁获取失败但计数已增），读-增-取值在单次 `update_with_result` CAS 内完成（`:247-250`），保证通知文案计数与实际自增一致
- `within_limit = reconnect_attempt_allowed(reconnect_count, config.max_disconnect_reconnect)`（`:253`）
- 限内：发「检测到断线」通知+ 登录日志（`:257-266`，前台过程信息走日志通道 `:259` 注释），再 `full_login`（`:271`）；成功：清计数、置 `any_adapter_online`/`has_logged_online`、清 `prep_login_failures`（`:277-284`），写 `login-history("reconnect")`（`:285`）并发结果事件（`:289-291`）
- 超限：释放登录锁（`:295`），第 `max+1` 次发「断线重连已达上限」+「请手动登录」（`:296-299`），之后每 `RECONNECT_REMINDER_INTERVAL`（10）次发「网络仍断线」（`:300-306`）
- 返回 `reconnect_should_report(reconnect_success, within_limit)`（`:314`）——历史缺陷注释（`:309-313`）：所有路径返回 false 会让调用方用旧快照覆盖，单适配器用户重连成功后被立即重新标离线并永久困住

**`run_auto_login_on_start`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:317-574`)
开机自启登录流程：

- `auto_login_on_start` 关闭直接返回（`:321-323`）
- `initial_delay`：`--autostart` 5000ms，否则 1500ms（`:325-326`）
- spawn `auto_login_on_start` 任务（`:333-573`）；退出中或 `has_logged_online` 已真则直接返回（`:336-339`）
- `get_adapters_force`（spawn_blocking，`:341-353`）；`--autostart` 场景适配器无 IP 时最多重试 3 次 × 3000ms（`:363-378`）
- 校园网检测（`:384-443`）：静默期置 `on_campus_network=true` 跳过（`:385-400`，`:391-393` 注释：与后台巡检同口径，防止开机自启与巡检形成两条整夜空转的认证请求源）；检测任务 panic/取消 → 跳过本次防误踢出（`:401-415`，`:402-404` 注释）；不在校园 → emit 失败事件（`:417-435`），无 IP 跳过退出（`:425-430`）否则 `start_campus_exit` + return（`:432`）
- Portal 探测：`check_portal_full` **只读探测不带凭据**（防错误凭据触发账号锁定，`:456-457` 注释），双卡并行 `spawn_blocking`（`:458-471`）
- 已在线：置 `any_adapter_online`/`has_logged_online`/`prep_login_failures=0`（`:501-505`），emit「已在线」并 return（`:509-510`）
- 未找到主适配器：记 warn（`:519-522`）
- 未在线：`spawn_blocking` 内再查一次 `has_logged_online`（`:528-530`）→ `is_logging_in.try_acquire`（`:531-534`）→ 更新 `last_auto_login_attempt`（`:535`）→ `full_login`（`:536`）
- 成功：置 `has_logged_online=true` + 清失败计数（`:555-560`）；`auto_exit_after_login` 时 `start_auto_exit`（`:565-568`）

### 事件出口与状态写回

**`handle_status_change`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:68-107`)
在线↔离线翻转时的通知器：`current_online != prev_online` 才动作（`:86`）；**仅离线方向**且 `enable_notification` 时弹系统通知（`:92`），用 `update_stats.last_network_change_notification_ms` 做 **60s 节流**（`:93-102`，`:93` 注释：网络抖动快速翻转时 60s 内不重复弹）；未翻转只记 debug（`:104-106`）。

**`emit_background_check_result`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:109-167`)
构造并发出 `background-check-result` 事件：`background_check_count` 在单次 `update_with_result` CAS 内自增并返回新值（`:116-119`，BE-A-10 修复注释 `:114-115`：避免 load+update 两次快照重复计数）；注销保护期内强制 `online=false`、`secondaryOnline=Some(false)`、两个 `*Operator` 置 null（`:126-134`）。事件 JSON 字段（`:136-164`）：`serverAvailable`/`loginAvailable`/`online`/`message`/`adapter1Name`/`adapter2Name`/`secondaryOnline`/`secondaryMessage`/`onlineOperator`/`secondaryOnlineOperator`/`timestamp`/`checkCount`/`isRunning`/`currentSsid`/`onCampusNetwork`/`enableNetworkNameCheck`/`requiredNetworkName`/`campusWifi`/`campusWired`/`campusAdapters`/`a1CampusMessage`/`a2CampusMessage`/`a1OnCampus`/`a2OnCampus`。

**`update_network_state`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:169-205`)
巡检收尾状态写回：先写 `server_available`（`:176`）；注销保护期内直接返回（`:180-186`）；否则写 `any_adapter_online`/`last_a1_online`（`:188-194`），`any_online` 时清 `disconnect_reconnect_count`（`:191-193`）。最后 `reachable && !has_logged_online && online` → 置 `has_logged_online=true` + 清 `prep_login_failures`（`:196-200`），并在 `auto_exit_on_online` 时 `start_auto_exit`（`:201-203`）——`has_logged_online` 的置位点之一。

**`build_adapter_details`** / **`adapter_status_entry`** / **`adapter_disabled_entry`** / **`adapter_disconnected_entry`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:51-65`、`:36-41`、`:43-45`、`:47-49`)
适配器详情拼接（`"名: 消息, …"`，副适配器仅 `is_secondary_adapter_enabled` 时计入，`:59-63`）与状态 JSON 构造器（后两者是「适配器已禁用或未找到」「适配器未连接」的降级文案包装）。

### 校园网判定与 Portal 探测

**`check_campus_network`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:188-278`)
校园网判定主体：名称名单解析 `campus_name_list`（`:57-63`，doc `:55-56`，按顿号/全半角逗号/分号/空格/Tab 切分）；`enable_network_name_check` 关闭时名称腿整体不参与、也省掉 netsh 调用成本（`:195-202`）；构造共享上下文 `CampusContext`（`:66-76`，doc `:65`，构造点 `:206-213`）；`(target, source)` 维度探测缓存闭包（`:216-234`，复用命中记 debug、绑源探测可达记 info、不可达记 warn）；**逐卡判定** `judge_adapter`（`:106-162`，doc `:101-105`）按成本短路：无 IP 卡直接判非校园且不探测（`:120-122`，杜绝蹭同类型其他卡结论）→ 名称腿 `name_hit`（`impl` `:78-99`，SSID 只对无线卡、有线 profile 只对有线卡，`:81-98`）→ `/18` 网段 `is_same_subnet_18`（`:130-136`，零 IO）→ 绑该卡源 ping 网关（`:139-145`）→ 绑源探 Portal（`:148-154`）→ 全未命中判非校园（`:157-161`）。聚合出 `wifi`/`wired` 兼容字段（`aggregate_class` `:166-173`、`join_messages` `:176-186`，命中卡文案优先，聚合点 `:241-246`）。`on_campus = 任一卡命中`（`:248`），未通过记 `log_warn` 便于排障（`:264-268`）。默认名单 `i-wxxy、iwxxy-2、iwxxy-3`（`config/model.rs:254-256`）。单测覆盖逐卡判定与聚合（`:287-434`；静默期三条边界用例已迁移到 config/schedule.rs，`:433-434` 注释）。

**`adapter_campus_status` / `adapter_campus_message`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:45-49`、`:51-53`)
按适配器名从 `CampusCheckResult.adapters` 取逐卡判定/消息；以适配器列表为权威——卡已不存在（被禁用/拔线）时不返回任何判定，避免残留结论（`:46-47`）。

**`is_campus_check_silent`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:280-285`)
静默期判定实现已下沉到双端共享面 `crate::config::schedule::is_campus_check_silent`（2026-10-26：此前桌面巡检、安卓巡检、安卓开机自启、无头 core 各写一份同语义边界判定，违反「新增跨平台判定优先放共享面」的约定，`:280-284` doc；此处仅保留 crate 内 re-export `:285`）。安卓端 `monitor_loop.rs:1413` 与 `monitor_loop.rs:596` 也直接调共享面。

**`check_portal_full`** (`tauri-app/src-tauri/src/auth/portal.rs:97-122`)
Portal 状态探测入口：解析适配器 IP（`parse_adapter_ip` `:144-150`）→ 构建绑源 HTTP 客户端（`CLIENT_TIMEOUT=8s`，`portal.rs:8`）→ `check_portal_page`（`:208-261`，页面探测用配置原始地址、**不探测 :801**——那是 EPortal SPA 登录表单无状态特征，`:209-216` 注释；非 2xx 判 Failed `:246-249`；`read_bounded_body` 限长 1MB `:253-256`）→ `analyze_portal_page_content`（`:264-290`，页面特征字符串 `portal.rs:13-17` + 用户会话特征 `has_user_session_indicators` `:293-298`）。三种出口：`PageCheckResult::Determined` → `build_determined_status`（`:153-163`，在线时 `login_available=false`、离线时丢弃 uid）；`Unknown` → `handle_unknown_page_status`（`:131-141`，**只读安全约束**：绝不带凭据调登录端点，返回 `error_kind=need_manual_check`，doc `:124-130`）；`Failed` → `build_request_failed_status`（`:166-175`，`error_kind=request_failed`，MAC 重置熔断的输入）。

**`operator_suffix_from_uid`** (`tauri-app/src-tauri/src/auth/portal.rs:319-332`)
从完整在线账号推导运营商后缀（与 `config.operator` 同口径）：`@telecom`/`@unicom`/`@cmcc` 原样、无 `@` → `""`（无锡学院默认线路）、空 uid 或未知后缀 → `None`（前端徽标隐藏）。`extract_uid_from_html`（`:302-315`，doc `:300-301`）从页面内嵌 JS `uid='xxx'` 提取，空占位跳过。

### MAC 重置与失败计数

**`failure_tracker.rs`** (`tauri-app/src-tauri/src/auth/failure_tracker.rs`)
- `MAX_FAILURES = 5`（`:6`）与 `AUTH_FAILURE_CODES = ["ac_auth_failed", "1", "4"]`（`:9`）；`is_auth_failure`（`:19-25`）按 code 判定
- `update_auth_failure_count`（`:47-81`）：成功复位 `portal_failure_count`（`:48-55`）；CAS 自增（`update_with_result`，`:64-67`，TOCTOU 修复注释 `:61-63`）；**认证失败不再触发 MAC 重置**（`:73-76`），达 5 次仅告警并清零（`:77-80`）
- `handle_single_adapter_failure`（`:96-142`）：双适配器版本，CAS `:119-128`（doc `:117-118`），告警 `:138-141`
- `reset_all`（`:145-154`）：注销成功时清 `portal_failure_count`/`a1`/`a2`/`prep_login_failures`（`:146-152`）
- **`PORTAL_REQUEST_FAILURE_THRESHOLD = 3`**（`:158`，doc `:156-157`）：门户 HTTP 不可达连续 3 次触发——MAC 重置的唯一触发源
- **`handle_portal_request_failure`**（`:170-263`）：网关不可达时跳过计数并清零（`:179-192`，校园网断网/维护期避免误重置）；网关可达时 CAS 累加（`:196-205`）；达阈值 → emit_login-log + `emit_mac_reset_cycle`（`:221-227`）→ `dhcp_release_renew_single` MAC 重置（`:230-259`，非校园网子网时改发「请更换路由器 MAC」通知 `:235-247`）；触发后清零计数（`:262`）

### 定时登录/注销与生命周期

**`run_scheduled_action_loop`** (`tauri-app/src-tauri/src/monitor/scheduled.rs:56-220`)
30s 一拍的编排循环（`SCHEDULED_TICK_MS` `:46`）：启动时先做出站启动对账（`reconcile_outbound_on_startup`，`:62-63`）与定时注销启动对账（`:65`）；循环内判定定时动作（`evaluate_scheduled` `:93-96`，门控用**拍前**切换态 `:89-92`）、执行 `run_scheduled_login`（`:97-99`）/`run_scheduled_logout`（`:100-102`）、编排夜间出站切换/还原/预判定/重放/看门狗（`:104-193`，`evaluate_outbound_action_with_restore` `:116-125`）、运营商夜切判定与执行（`:196-218`）。定时登录/注销独立于静默期闸门（`.codewiki/decisions/scheduled-actions-outside-silent-window`，`:198-199` 注释）。

**`spawn_login_action` + `SCHEDULED_LOGIN_MAX_RETRIES`** (`tauri-app/src-tauri/src/monitor/scheduled.rs:307-363`、`:368`)
登录动作共用执行体（定时登录与晚间断网切换同款）：抢 `is_logging_in` 互斥锁，抢不到本拍跳过并对外发 `scheduled-action-skipped` 事件（`:313-322`，审计 A7）；抢锁成功后消耗当日额度（`:328-330`）→ `full_login`（`:331`）→ emit（`:333-336`）；成功走 `post_login_handler`（`:337-341`）；失败补试分支（`:342-361`）：补试计数未用尽时**回退当日额度标记**（`login_day` 置 `i32::MIN`，`:353-359`）让下一拍「过点补触发」再试，用尽（`>= SCHEDULED_LOGIN_MAX_RETRIES = 20`，`:368`）才保留当日标记停止重试（`:344-351`）。修复动机注释 `:323-327`。

**`perform_logout`** (`tauri-app/src-tauri/src/monitor/scheduled.rs:418-452`)
注销执行体（定时注销与夜切验证复登前共用）：抢 `is_logging_out` 锁（`:420-428`）→ `full_logout`（`:433`）→ 成功后取消登录后自动退出、`failure_tracker::reset_all`、启动 **60s 注销保护期**（`logout_protected_until`，`:440-450`）——防止定时注销后被准备自动登录/掉线重连立即登回。

**`reconcile_scheduled_logout_on_startup`** (`tauri-app/src-tauri/src/monitor/scheduled.rs:236-251`)
定时注销防冷启动误触发：启动时若当日注销时刻已过，只消耗当日额度（置 `logout_day`）不执行动作（`:242-249`）——「过点补触发」否则会让首拍立即注销。定时登录不消耗（启动补登录 = 补上线，正是预期，doc `:228-235`）。

**`start_campus_exit` / `cancel_campus_exit` / `start_auto_exit` / `shutdown_and_exit`** (`tauri-app/src-tauri/src/infra/lifecycle.rs`)
校园网验证不通过时的 30s 最小化 + 60s 强制退出（`CAMPUS_MINIMIZE_DELAY_MS=30000` `:12`、`CAMPUS_EXIT_DELAY_MS=60000` `:13`）：受 `campus_exit_on_fail`（`:24-28`）与生效时段窗口 `is_within_campus_exit_window`（`:17-19`，结束<=开始退化为仅起点限制，测试 `:334-352`）双闸；CAS `campus_exit_started` 防重（`:51-54`）+ deadline 二次校验（`:85-89`、`:110-121`）防取消竞态；spawn 失败回滚标志（`:136-142`）。取消入口 `cancel_campus_exit`（`:146-162`，校园网恢复时由巡检 `background_check.rs:120`/`:199` 调用）与快捷键版 `cancel_campus_exit_with_notification`（`:165-189`）。登录成功自动退出 `start_auto_exit`（`:191-272`，`AUTO_EXIT_DELAY_MS` 经 `:5` 引入）与 `cancel_auto_exit_inner`（`:274-298`）共用取消快捷键 `Ctrl+Shift+C`（`try_unregister_cancel_exit_shortcut` `:302-310`，持锁覆盖 check-then-act 消 TOCTOU `:127-131`）。统一退出 `shutdown_and_exit`（`:319-328`）：置 `is_quitting` → 10s 上限等待全部后台任务取消（`:323-325`）→ `app_handle.exit(0)`。

**`emit_notification`** (`tauri-app/src-tauri/src/infra/notification.rs:16-64`)
系统通知唯一出口：`enable_notification` 关闭直接返回（`:17-23`）；**窗口可见且未最小化即不打扰**（`monitorable` 判定 `:29-34`，仅看 is_focused 会在失焦时误弹、仅看 is_visible 在最小化时恒 true，`:25-28` 注释）；Windows 桌面自组 toast（带看板娘头像，`:45-55`），失败降级插件纯文本；本函数不再向前端发事件（双通道 toast 重复的历史缺陷由此根除，`:11-13` doc）。安卓端对应物是 `monitor_loop.rs` 的 `notify_system`（`:189-206`，看板娘 large_icon 失败降级）与 `emit_login_log`（`:180-184`）。

### 注销命令与复核

**`do_login` / `do_logout` / `perform_full_logout_sync`** (`tauri-app/src-tauri/src/commands/login.rs:63-90`、`:91-100`、`:105-199`)
`do_login` 先清 `auto_exit_cancelled` 与残留倒计时（`:64-66`）再 spawn_blocking 抢锁执行 `full_login`（`:71-81`），成功走 `post_login_handler`（`:84-86`）。`do_logout` 与托盘「快速注销」共享 `perform_full_logout_sync`：抢 `is_logging_out` 锁（`:107-113`）→ `full_logout`（`:115`）→ 1s 后逐适配器复核在线（`check_any_adapter_online`，`:117-146`）：仍在线发「注销可能未完全生效」（`:120-126`）；`probe_ok` 全部成功且离线才断言「已确认离线」（`:126-133`）；探测异常降级为「未能确认」（`:133-142`）。全量注销成功后重置全局状态 + 60s 注销保护期（`:148-178`，`has_logged_online=false`/`disconnect_reconnect_count=0`/`last_auto_login_attempt` 重置/`logout_protected_until` `:172-178`）；单适配器注销仅重置各卡状态（`:179-196`）。

### 配置字段

**桌面 `Config`** (`tauri-app/src-tauri/src/config/model.rs`)
本链路消费：`auto_login_on_start`（`:34-35`）、`enable_background_check`（`:48-49`）、`background_check_interval`（`:50-51`）、`auto_login_on_preparation`（`:52-53`）、`campus_check_start_minutes`（`:168-169`，默认 460=07:40，`:326`）、`campus_check_end_minutes`（`:170-172`，默认 1380=23:00，`:327`）、`scheduled_login_minutes`/`scheduled_logout_minutes`（`:173-178`，默认禁用哨兵 1440，`:330-331`）、`max_disconnect_reconnect`（`:181-182`，默认 3，`:333`）、`auto_login_cooldown_secs`（`:183-184`，默认 60，`:334`）、`nightRestoreMinutes`（`:69-70`，0 回落 07:30，`:293`）。默认值全部在 `Default` impl（`:262-344`）；`default_required_network_name` `:254-256`、`default_campus_gateway` `:258-260`。

**安卓 `Settings`** (`android/src-tauri/src/config_state.rs`)
同口径字段：`auto_login_on_start`（`:78`）、`enable_background_check`（`:79`）、`background_check_interval`（`:80`，默认 60s `:169`）、`background_check_idle_interval`（`:85`，默认 300_000=5min `:171`，doc `:81-84`）、`auto_login_on_preparation`（`:86`，默认 true `:172`）、`max_disconnect_reconnect`（`:87`，默认 3 `:173`）、`auto_login_cooldown_secs`（`:88`，默认 60 `:174`）、`campus_check_start_minutes`（`:113`，默认 460 `:199`）、`campus_check_end_minutes`（`:115`，默认 1380 `:202`）、`scheduled_login_minutes`/`scheduled_logout_minutes`（`:117-119`，默认哨兵 1440 `:206-207`）、`night_outbound_restore`（`:68`，安卓出站切换态标记 `logged_out`，doc `:64-67`）、`night_restore_minutes`（`:51`）。

### 安卓端实现

**`MonitorState`** (`android/src-tauri/src/monitor_loop.rs:55-83`)
进程级巡检状态（lazy_static `MONITOR` `:15-17`）：`running`/`check_count`/`consecutive_failures`/`reconnect_count`/`last_login_attempt_ms`/`was_online`（`:56-62`）、`desired_interval_ms`（`:66`，改间隔立即生效）、`idle_interval_ms`（`:68`，闲时分档）、`logout_protected_until_ms`（`:70`）、`wifi_event_ms`（`:72`）、`notified_online`（`:76`，常驻通知状态记忆，避免每拍重建通知的功耗点，doc `:73-75`）、`scheduled_login_day`/`scheduled_logout_day`（`:80-82`，定时动作当日标记）。代际 token `MONITOR_GENERATION`（`:32`）与 attempt 段互斥锁 `LOGIN_ATTEMPT_LOCK`（`:37`）防多入口竞速。

**`should_attempt_login`** (`android/src-tauri/src/monitor_loop.rs:121-142`)
纯函数登录判定（doc `:119-120`）：`online || !on_campus` 永不尝试（`:131-133`）；掉线重连（`was_online=true`）不受 `auto_login_on_preparation` 管、启动即离线需显式开启（`:134-136`）；`reconnect_count >= max_reconnect` 暂停（`:137-139`）；冷却判定（`:140-141`）。

**`effective_interval_ms` / `wifi_event_is_burst_start`** (`android/src-tauri/src/monitor_loop.rs:110-117`、`:101-103`)
巡检分档纯函数（WiFi+亮屏 → 基础间隔，蜂窝/灭屏 → 闲时间隔，基础下限 5s；拉长不改变正确性——WiFi 变化事件仍即时触发，doc `:104-109`）与 WiFi 事件风暴起点判定（`:96` 去抖窗口 1000ms）。

**`run_check_once`** (`android/src-tauri/src/monitor_loop.rs:1379-1633`)
单次检测全入口共用体（周期拍、WiFi 变化事件 `handle_wifi_event` `:339-377`、手动 `trigger_background_check` `:380-388` 三条路径）：

- 出站切换态（`night_outbound_restore` 非空）整轮跳过（`:1394-1397`，doc `:1389-1393`；闸在此处而非 tick 循环，保证三入口共用，晨间还原判定 `run_scheduled_actions` 不经过此闸）
- 校园网检测静默期整拍跳过（`:1399-1442`）：`silent_now` 判定 `:1412-1417`、`RESTORE_SILENT_PASS` 消费 `:1418-1422`（离开静默期才清）、跳过时同步常驻通知为「已暂停检测(非检测时段)」（状态码 3，`:1425-1434`）、放行拍记 debug `:1437-1442`
- 探针窗口持 WifiLock/WakeLock（`ProbeWindowGuard` `:649-657`，RAII 保证 panic 释放，`:1446-1451`）；`ensure_wifi_bound` 强制绑 WiFi（`:1453-1455`）
- 校园网判定（`probe_campus_with_ssid`，`:1457-1470`）→ Portal 探测（WiFi 未连接/非校园网短路 `:1479-1483`，绑小核专用线程 `portal_probe_on_little_cores` `:816-853`，线程内 enter runtime handle + panic 捕获 `:826-838`）
- **三态消费**（`:1489-1504`）：仅 `error_kind=None` 的确定判定才翻转在线状态——Unknown/Failed 不构成可信离线证据，保持上一拍记忆；非校园网（`on_campus=false`）时无条件判 false，防止「在线」被永久钉住（`:1495-1499` 注释）
- 状态机（`:1506-1516`）：在线清零重连/熔断计数与注销保护期（`:1507-1511`）；`was_online` 翻转且在校园网时发掉线通知（`:1514-1516`）
- 自动登录（`:1518-1582`）：两道闸——注销保护期（`:1526`）+ 连续失败熔断 5 次（`failures_capped`，`:1527`，doc `:1520-1521`）+ `should_attempt_login`（`:1528-1538`）；`LOGIN_ATTEMPT_LOCK` 互斥下记账→登录→计数（`:1542-1545`）；达上限只发通知不登录（`:1546-1548`）；成功走 `emit_auto_login_result` 应用内 toast、失败计熔断 + 系统通知（`:1563-1571`）；执行失败同计熔断（`:1574-1579`）
- payload emit `background-check-result`（`:1584-1619`，字段为桌面可适用子集 + `campusMessage`/`sourceIp`/`autoLogin`）；常驻通知仅状态翻转时更新（`:1621-1632`，状态码表 `notify_state` `:1637-1645`）

**`monitor_tick_loop`** (`android/src-tauri/src/monitor_loop.rs:675-734`)
巡检循环：启动对账（定时注销防冷启动误触发，与桌面 `scheduled.rs` 同语义，`:680-697`）；代际校验退出（`:700-704`）；`run_scheduled_actions` 置于分档跳拍与静默期**之前**（`:705-708`——定时动作常配置在静默期/灭屏闲时段，放进 `run_check_once` 会被静默期整拍吞掉）；间隔热更新（`:709-716`）；分档跳拍（`:717-727`）。

**`run_scheduled_actions`** (`android/src-tauri/src/monitor_loop.rs:867-1077`)
安卓编排：夜间出站切换/还原先行（spec §2 顺序硬约束，`:872-963`）——Switch 分支注销并落 `night_outbound_restore="logged_out"` 标记（`:900-919`，注销生效判据 `logout_for_outbound_switch` `:763-788`、失败收敛 `switch_logout_unconfirmed` `:793-810`）；Restore 分支登录成功才清标记并置 `RESTORE_SILENT_PASS`（`:921-938`），失败达 3 次（`OUTBOUND_RESTORE_MAX_FAILS` `:23`）放弃自动还原清标记恢复巡检（`:939-961`）；运营商夜切让位由 `evaluate_night_switch_gated_with_restore` 的 `outbound_active` 门控（`:977-1020`）；定时登录/注销判定走共享纯函数 `evaluate_scheduled_actions`（`:1033-1041`），切换态下定时登录跳过但**不消耗当日标记**（`:1029-1032` 注释），复用 `auto_login_on_start` 全编排（`:1042-1054`）与 `do_logout`（`:1056-1076`）。

**`auto_login_on_start`** (`android/src-tauri/src/monitor_loop.rs:583-644`)
凭据检查（`:584-586`）→ 静默期跳过（`:593-604`，共享面 `is_campus_check_silent`，doc `:588-592`）→ `ensure_wifi_bound`（`:606`）→ `probe_with_retry`（`:549-567`，3s 后重试一次）→ 非校园网跳过（`:616-619`）→ `run_login`（`:621-643`）；成功预置 `was_online=true` 并清注销保护期（`:626-630`）。系统通知仅失败侧保留（`:571-572` doc），结果走 `emit_auto_login_result`（`:573-579`）。

**`run_startup_tasks` / `start_background_check` / `stop_background_check`** (`android/src-tauri/src/monitor_loop.rs:472-543`、`:222-270`、`:273-285`)
启动恢复（500ms 就绪窗口 `:474`，对齐桌面 `watcher::run_startup_tasks`）：并行拉起后台检测（`:503-508`）、质量首测（`:515-527`）、启动自动登录（`:533-541`），读配置失败 emit `startup-failed` 并停 FGS（`:476-499`）。`start_background_check` 幂等：已在跑仅刷新目标间隔（`desired_interval_ms` `:225`）并重发 FGS 保活（`:229-243`）；新启动先起前台服务再置 running（`:246-256`），随后注册 WiFi watcher（`:262`）与代际 spawn（`:266-267`）。`stop_background_check` 先推进代际再清标志（`:274-276`）。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│ 启动装配 startup.rs setup_app(:161-272)                        │
│   └► run_startup_tasks(watcher.rs:16-72)                      │
│        ├► start_background_check_inner 周期巡检(bg_task:7-68)  │
│        ├► run_auto_login_on_start 开机登录(auto_auth:317-574)  │
│        └► run_scheduled_action_loop 定时动作(scheduled:56-220) │
└──────────────────────────┬───────────────────────────────────┘
                           ▼
┌──────────────────────────────────────────────────────────────┐
│ 巡检主体 run_background_check_blocking(bg_check:50-467)        │
│  campus_check ► portal_check ► background_emit 状态/事件       │
│  ► auto_auth 自动登录/断线重连 ► outcome 动态间隔              │
└───────┬──────────────────────────────┬───────────────────────┘
        ▼                              ▼
┌────────────────────┐   ┌────────────────────────────────────┐
│ NetworkSnapshot     │◄──┤ 还原后补检: outbound_night:1403-04  │
│ (network.rs:7-34)   │   │ ─► background_task:149-183         │
└────────────────────┘   └────────────────────────────────────┘
```

两端巡检策略对比：

| 维度 | 桌面 | 安卓 |
|---|---|---|
| 基础间隔 | `background_check_interval` 默认 60s（`model.rs:285`），循环每拍重读（`background_task.rs:34`） | 默认 60s（`config_state.rs:169`），`desired_interval_ms` 热更新（`monitor_loop.rs:711-716`） |
| 省电分档 | 无（轻量化 300s 下限，`lightweight.rs:77-79`） | 蜂窝/灭屏走 `background_check_idle_interval` 5min（`monitor_loop.rs:228-235`） |
| 未登录退避 | 基础 ×2 封顶 5 分钟（`background_task.rs:24-26`） | 无（固定 cooldown） |
| 门户不可达 | 1 分钟快排查 + 联动质量检测（`background_check.rs:452-453`） | 无 |
| 准备登录失败 | 指数退避封顶 300s（`auto_auth.rs:40-50`） | 连续 5 次熔断（`monitor_loop.rs:1527`） |
| 检测时段门控 | `is_campus_check_silent`（共享面，`campus_check.rs:285`） | 同左（`monitor_loop.rs:1413`） |

## Data Flow

**周期巡检 tick（桌面）**：`start_background_check_inner` 的循环（`background_task.rs:7-34`）按 `DynamicInterval` 间隔触发 → `run_background_check`（`background_check.rs:546-555`）送 `spawn_blocking` → `run_background_check_blocking`：适配器枚举（`:63-73`）→ 出站切换态/静默期闸（`:84-133`）→ `check_campus_network` 逐卡判定（`:136`）→ Portal 探测（`:206-239`）→ 状态写回 `NetworkSnapshot`（`:140-152`、`:373`）与事件 emit（`:413-434`）→ `try_auto_login_on_preparation`/`try_disconnect_reconnect`（`:436-442`）→ `update_network_state`（`:444-448`）→ 结局分类回填 `last_outcome` 驱动下一拍间隔（`:450-463` → `background_task.rs:41,56`）。

**开机自启登录（桌面）**：`startup.rs:248` → `watcher.rs:58-62` spawn → `run_auto_login_on_start`（`auto_auth.rs:317-574`）延时 1.5s/5s → 强枚举适配器（`--autostart` 时最多重试 3 次）→ 校园网检测/静默期闸（`:384-443`）→ 只读 Portal 探测（`:456-471`）→ 未在线则抢锁 `full_login`（`:526-537`）→ 成功置 `has_logged_online` 并按 `auto_exit_after_login` 启动自动退出（`:555-568`）。

**夜间还原补检（桌面）**：出站还原完成 → `finish_outbound_restore`（`outbound_night.rs:1385-1406`）清快照/退避/hold → `:1403` 置 `RESTORE_SILENT_PASS` → `:1404` `spawn_background_check_after_outbound_restore`：`spawn_blocking` 内 3s 轮询 `post_restore_adapters_have_ip`（`background_task.rs:153-161`）→ 就绪（或 45s 超时）→ `spawn_background_check_once`（`:181`）→ 一次性巡检（可被统一取消，`task_manager` 注册）→ 巡检静默分支被放行标记豁免（`background_check.rs:98-133`）→ 自动登录当拍评估。

**注销与保护期（桌面）**：`do_logout` → `perform_full_logout_sync`（`login.rs:105-199`）→ `full_logout` → 1s 复检（`:117-146`）→ 60s 保护期（`:172-178`）；保护期内巡检 emit 强制 `online=false`（`background_emit.rs:126-134`）、自动登录/断线重连跳过（`auto_auth.rs:140-144`、`:226-230`），到期后自动恢复。

**安卓巡检 tick**：`monitor_tick_loop`（`:675-734`）→ 分档跳拍 → `run_check_once`（`:1379-1633`）：出站态/静默期闸 → 校园网探针 → Portal 探测 → 三态消费翻转 `was_online` → 两道闸（保护期/熔断）+ `should_attempt_login` → `LOGIN_ATTEMPT_LOCK` 下 `run_login` → payload emit → 常驻通知按需更新。

## Connections

- [[night-outbound-switch]]：出站切换/还原决策与编排（`monitor/outbound_night.rs`、`config/outbound_switch.rs`）。`finish_outbound_restore`（`outbound_night.rs:1385-1406`）是本文补检钩子与 `RESTORE_SILENT_PASS` 置位的唯一挂点；切换态（`outbound_restore_active`，`outbound_switch.rs:142-144`）是巡检整体跳过闸（`background_check.rs:84-91`）。
- [[scheduled-actions-outside-silent-window]]：定时动作独立于静默期闸门的设计决策（`scheduled.rs:198-199` 注释引用），定时登录/注销与巡检解耦（`watcher.rs:64-71`）。
- [[portal-protocol]]：`auth::portal::check_portal_full`（`portal.rs:97-122`）是巡检探测的唯一只读入口；`auth::service::full_login`/`full_logout` 是自动登录/重连/定时动作共用的协议执行体。
- [[failure-tracker-mac-reset]]：`auth/failure_tracker.rs` 的双计数器（认证失败仅诊断 `:73-80`、Portal 请求失败 3 次触发 MAC 重置 `:158-263`）；`reset_all`（`:145-154`）被注销流程调用。
- [[lightweight-mode]]：`app/lightweight.rs` 的 300s/1800s 间隔下限（`:77-84`）作用于巡检退避基础值（`background_task.rs:37-40`）与退出拦截（`startup.rs:144-158`）。
- [[network-quality]]：`quality_scheduler::run_quality_check` 被 `trigger_fast_quality_check`（`background_check.rs:503-504`）与定时测试循环（`watcher.rs:45-55`）调用，与巡检互不叠加。

## Known Issues

- **过期注释（登录语义）**：`infra/state/network.rs:26` 的 `prep_login_failures` 字段 doc 仍写「达到上限后本会话停止自动登录」，但实现自 2026-10-24 起已改为指数退避降频、有界且不放弃（`auto_auth.rs:18-20`、`:129-138`）——注释是旧语义残留。
- **过期注释（MAC 阈值）**：`monitor/background_check.rs:243` 注释写「连续5次 request_failed 时触发 MAC 重置」，实际阈值已是 3（`failure_tracker.rs:158` 的 `PORTAL_REQUEST_FAILURE_THRESHOLD`，2026-10-25 由 5 调整）。
- **两端重试策略不统一**：桌面准备自动登录失败采用指数退避（`auto_auth.rs:40-50`），安卓仍是硬编码「连续 5 次熔断、本会话停手」（`monitor_loop.rs:1527`，doc `:1521`；只有登录成功或注销才清零，`:1507-1510`）——阈值 5 未提取为常量，且用户改对密码后需手动登录一次解锁安卓端自动重登。
- **时间窗拒绝识别依赖文案关键词**：`auto_auth.rs:62-73` 的 `is_time_window_refusal` 按中英文关键词匹配门户拒绝文案（6 个中文关键词 + `time range`/`not allowed`+`time`），门户端文案改版会导致该类失败被误计入退避，最坏把登录窗口开启后的重试拖后 300s（`PREP_LOGIN_BACKOFF_CAP_SECS`）。
- **启动后改注销时刻的不可判定边界**：`scheduled.rs:234-235` 注释明示——启动对账后当天再把注销时刻改成另一个已过时刻，下一拍仍会立即触发（与「改完就想立刻注销」无法区分意图），留待用户反馈。
- **补检等待的极端成本**：`background_task.rs:163-166` 在补检探测线程 join 失败时按「等满 45s 上限」处理（宁可 warn 排障不谎报就绪），探测线程异常时会引入一次性的最长 45s 延迟——刻意取舍而非缺陷，排障时注意 `:168-173` 的 warn 日志。
