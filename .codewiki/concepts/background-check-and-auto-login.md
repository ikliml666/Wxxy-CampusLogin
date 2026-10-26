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
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/config_state.rs
tags: [概念， 后台巡检， 自动登录， 断线重连， 生命周期， 通知， 会话失效]

---

## Overview

后台巡检是一条「启动装配 → 周期 tick → 校园网判定 → Portal 探测 → 状态机 → 自动登录 / 断线重连 → 生命周期倒计时 / 通知」的长链路。桌面端实现在 `tauri-app/src-tauri/src/monitor/` 模块群（`watcher` 启动装配门面 + `background_task` 调度 + `background_check` 巡检主体 + `auto_auth` 自动登录决策 + `background_emit` 事件出口），安卓端实现在单个 `android/src-tauri/src/monitor_loop.rs`（决策逻辑抽成纯函数便于 TDD，模块 doc `monitor_loop.rs:1526-1528`）。两端共享重连上限 3、冷却 60s、注销保护 60s 等阈值语义，但失败重试策略、周期常量、检测时段门控与省电分档策略不同：桌面准备自动登录连续失败达 5 次后进入**指数退避降频**（封顶 5 分钟、有界且不放弃，`prep_login_backoff_secs` 见 `auto_auth.rs:40-50`），安卓连续失败 5 次直接熔断停手（`monitor_loop.rs:1526` 的 `failures_capped` 判定）。

巡检间隔本身也是动态的：周期循环每拍重读配置基础间隔并按上一轮结局调整——未登录按基础间隔 ×2 逐步退避（封顶 5 分钟）、已登录 10 分钟、已登录但门户不可达降为 1 分钟快排查（`background_task.rs:24-26` 注释，结局分类在 `background_check.rs:445-451`）；快排查档会联动一次网络质量检测确认互联网连通性（`trigger_fast_quality_check`，`background_check.rs:463-532`，5 分钟节流由 `NetworkSnapshot.last_fast_quality_at` 承载）。

2026-10-22 修复了两个让「整夜休眠后到点不自动登录、只能手动点」的缺口，构成本文的两个核心新增点：

1. **会话失效重置**（`monitor/background_check.rs:25-27` 纯函数，`:381-387` 调用点）：巡检在调用 `try_auto_login_on_preparation` **之前**，若判定「服务可达 + 主副卡都不在线 + 此前登录过」，就把 `has_logged_online` 复位。此前该标记只在登录成功时置位、永不复位（唯二的复位点只覆盖校园网检测失败与用户主动注销），整夜休眠或夜间出站切换后旧会话已失效，`auto_auth.rs:125-127` 的「已登录过」门控会把自动登录永久拦住。放在评估之前是为了同一次巡检就能真正登录，而不是再等一拍（`:372-380` 注释）。
2. **还原后立即补检**（`monitor/background_task.rs:149-183`，钩子挂在 `monitor/outbound_night.rs:1225`）：夜间出站还原刚把校园网卡从设备级禁用（PnP problem 22）中启用，Windows 还要数秒到数十秒才拿到 DHCP 地址。此刻立刻巡检只会读到「配置的适配器均无 IP」空转，随后要等一整个巡检周期——轻量化模式下该周期下限 300s（`app/lightweight.rs:77-79`）。钩子在 `finish_outbound_restore`（`monitor/outbound_night.rs:1206-1227`）尾部触发，先轮询等 IP 就绪（最长 45s、3s 一轮），再发起一次性巡检；自动还原 / 手动还原 / 启动对账三条还原路径全部汇到这一个收尾点，钩子只挂一处（`:1219-1223` 注释）。

2026-10-24 修订（`monitor/scheduled.rs:20-22` 模块注释）补上了定时登录的第三个缺口：定时登录到点失败时按 30s 拍间隔**有限补试**（`SCHEDULED_LOGIN_MAX_RETRIES = 20`，`scheduled.rs:346`，20 次 × 30s ≈ 10 分钟窗口，评审认为原 3 次 90s 窗口太短 `:343-345` doc），失败即回退当日额度标记让「过点补触发」再试（`:330-338`），补试用尽才放弃当日——原「到点即置标记、失败也当日不重试」会让登录时段刚开的服务端瞬时拒绝直接吞掉当日登录（`:301-305` 注释）。同批修订还包括：准备自动登录达失败上限后改为指数退避（`auto_auth.rs:18-20` 历史缺陷二），断线重连超限后每 10 个周期放行一次重连（`reconnect_attempt_allowed`，`auto_auth.rs:32-35`），二者都把「达上限即永久终态」改为「有界降频、必然重试」。

2026-10-25 起自动修复策略再收敛一处：**认证失败不再触发 MAC 重置**（`failure_tracker.rs:73-76`、`:136-137`）——时间窗外/账号态异常也会返回认证失败码，以此为触发条件会在不可登录时段反复换 MAC/IP；认证失败侧只保留计数与日志（达 5 次仅告警并清零，`:77-80`、`:138-141`）。MAC 重置的唯一触发源改为「门户 HTTP 不可达」，阈值由 5 收紧为 3（`PORTAL_REQUEST_FAILURE_THRESHOLD`，`failure_tracker.rs:156-158`，触发器 `handle_portal_request_failure` `:170-263`）。

当前代码在自动登录决策上还有两处精细化的收敛：其一，**门户恢复上升沿复位退避**——`NetworkSnapshot.last_login_available`（`infra/state/network.rs:30`）记录上一检测周期的 `login_available`，`try_auto_login_on_preparation` 在所有早退**之前**做沿检测（`auto_auth.rs:101-117`）：门户从不可达恢复为可用（上升沿）视为新的登录机会，直接复位 `prep_login_failures` 退出指数退避，不必等退避周期自然到期；其二，**「可登录时间段未到」类拒绝不计入退避**——`is_time_window_refusal`（`auto_auth.rs:62-73`）按中英文关键词识别时段拒绝文案，`prep_failures_after_failure`（`auto_auth.rs:78-84`）对这类失败不累加计数（失败分支 `:187-199`），因为时间窗拒绝注定发生且窗口打开后应立即恢复，指数退避反而会把窗口开启时刻拖后最多 5 分钟。

2026-10-26 起新增**还原后补检无视静默期**的一次性放行标记：还原时刻（`nightRestoreMinutes`，默认 07:30）可配置后可能早于校园网检测静默期起点（`campusCheckStartMinutes`，默认 07:40），还原后的补检若落在静默期会被整体跳过（`background_check.rs:31-39`），用户 07:10 开机就是断网态。`FORCE_CHECK_AFTER_RESTORE`（`background_check.rs:31-37`）由 `finish_outbound_restore` 在发起补检前置位（`outbound_night.rs:126`），下一轮巡检消费即清（`:94`），只放行紧接着的一轮，不影响静默期其余语义。安卓端有同款标记（`monitor_loop.rs:43`）。

2026-10-25 另有两处跳过语义收敛：**静默期整拍跳过**——静默期原本只跳过「校园网环境验证」，Portal 登录检测、自动登录、断连重连照跑，校园网 23:00 断网后整夜空转认证请求（`background_check.rs:95-109` 历史缺陷注释，`:110-118` 现行为：清 `on_campus_network`、取消退出流程后按 `Skipped` 返回）；**出站切换态整体跳过巡检**（`:80-87`）——切换态下 portal 检测走热点出站必失败，会累计触发 MAC 重置提权与整夜告警。

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

impl 访问器（`:21-57`）：`online()` (`:22-27`)、`message()` (`:29-35`)、`reachable()` (`:37-42`)、`login_available()` (`:44-49`)、`uid()` (`:51-56`)——均仅在 `Success` 分支返回真实值，其余分支返回 false/固定文案（「检测失败」`:32`、「未找到主适配器」`:33`）/None。

**`CampusCheckResult`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:13-21`) 与 **`ConnectionCampusStatus`** (`:5-11`)
校园网判定结果（serde camelCase，`:6`）：`wifi: Option<ConnectionCampusStatus>` (`:16`)、`wired: Option<ConnectionCampusStatus>` (`:17`)、`on_campus: bool` (`:18`，= wifi ∨ wired 任一判定在校园)、`current_ssid: Option<String>` (`:19`)、`message: String` (`:20`)。`ConnectionCampusStatus` 携带单连接的 `on_campus` (`:9`)/`name` (`:10`)/`message` (`:11`)。

**`NetworkSnapshot`** (`tauri-app/src-tauri/src/infra/state/network.rs:7-34`)
巡检状态机读写的唯一状态载体（`NetworkState = ArcSwap<NetworkSnapshot>`，struct `:63-65`；`new` `:74-78`；`load` 快照读取 `:81-83`；`update` CAS 循环 `:86-100`，`update_with_result` CAS 并返回闭包结果防 TOCTOU `:106-121`）。字段与巡检链路一一对应：

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
- `prep_login_failures: u32` (`:27`，doc `:25-26`): 准备自动登录连续失败次数（会话内计数，登录成功/重置后清零）。**注意**：`:26` 注释仍写「达到上限后本会话停止自动登录」，但当前实现已改为指数退避（见下文 `prep_login_backoff_secs`），注释是旧语义残留
- `last_login_available: bool` (`:30`，doc `:28-29`): 上一检测周期的 `login_available`；`false→true` 上升沿表示门户重新可达（新的登录机会），用于复位 `prep_login_failures` 退避。`Default` 中初始为 `true`（`:56`）
- `last_fast_quality_at: Option<Instant>` (`:33`，doc `:31-32`): 「已登录但门户不可达」快排查模式下上次联动质量检测的时间，距上次不足 300s 不再触发

**`AdapterOnlineStatus`** (`tauri-app/src-tauri/src/commands/login.rs:9-16`)
注销后的在线复核结果：`any_online` (`:10`)/`a1_online` (`:11`)/`a2_online` (`:12`)/`probe_ok` (`:15`)。`probe_ok=false` 表示存在探测异常——离线结论不可信，不得断言「已确认离线」（审计 A8，`:13-14` 注释）。由 `check_any_adapter_online` (`login.rs:18-60`) 构造：并行探测双适配器 Portal（`std::thread::scope` + 裸线程先进入 tauri runtime context，`:29-50`；未启用副卡时 `a2_probe=Some(false)` 不参与判定 `:46-48`），`probe_ok = a1_probe.is_some() && a2_probe.is_some()`（`:58`）。

### 桌面端启动与调度

**`setup_app`** (`tauri-app/src-tauri/src/app/startup.rs:161-272`)
桌面启动装配入口。日志目录先探测安装目录 `logs/` 可写、否则回退数据目录（`:173-186`）；logger 先于 config 初始化（`:188-195`）；`set_log_retention_days(config.log_retention_days)`（`:221`）；`silent_start = config.hidden_start && args 含 --autostart`（`:226-231`）；随后依次 build_tray（`:233`）、`start_adapter_watch`（`:236-238`）、`start_cache_refresh_task`（`:239-241`）、`start_update_check_loop` + `cleanup_stale_update_dir`（`:243-246`）、**`run_startup_tasks`**（`:248`）、`record_webview2_runtime_version`/`subscribe_process_failed`（`:252-254`）、`spawn_heartbeat_thread`/`spawn_window_safety_thread`（`:256-257`）、GPU-warmup 后台线程（`:261-269`）。

**`run()` 命令注册** (`tauri-app/src-tauri/src/app/startup.rs:23-159`)
`background` 命令组 4 个命令 `start_background_check`/`stop_background_check`/`trigger_background_check`/`get_background_status` 在 `startup.rs:98-101` 注册；`RunEvent::ExitRequested` 拦截轻量化退出（`take_lightweight_exit_guard` + `should_prevent_exit`，`:144-158`；纯函数判定见 `app/lightweight.rs:44-46`，退出守卫单次语义 `:34-40`）。

**轻量化运行态** (`tauri-app/src-tauri/src/app/lightweight.rs`)
全部为内存态不落盘（`:1-4` 模块 doc）。与本链路相关：`LIGHTWEIGHT_ACTIVE`（`:10`）、`is_lightweight_active`/`set_lightweight_active`（`:26-32`）、退出守卫 `EXPECT_LIGHTWEIGHT_EXIT`（`:14`）与 `arm/take_lightweight_exit_guard`（`:34-40`）、`should_prevent_exit(expecting, is_quitting)`（`:44-46`，仅当次期待且未在主动退出流程时拦截）；**检测间隔下限** `effective_background_interval_ms`（`:77-79`，轻量化时 `base_ms.max(300_000)`）与质量检测下限 `effective_quality_interval_ms`（`:82-84`，`max(1_800_000)`）；ready 信号 `signal_window_ready`/`take_window_ready`/`wait_window_ready_and_show`（`:87-127`）。

**`run_startup_tasks`** (`tauri-app/src-tauri/src/monitor/watcher.rs:16-72`)
启动装配门面（watcher 只保留装配与 re-export：`background_task::start_background_check_inner` 经 `watcher.rs:14`、`campus_check::check_campus_network` 经 `:13`、`background_emit` 的三个条目构造器经 `:14` re-export，保持外部 `watcher::X` 调用路径不变，`:7-12` 注释）：

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
夜间出站还原后补检的等待上限与轮询间隔。45s 是按 2026-10-22 单次实证定档的经验值（`:100-111` doc：夜间出站还原完成到用户手动登录之间没有任何 `[background]`/`[auto_login]`/`[login]` 日志；轻量化下限 300s 意味着下一拍巡检最早落在 4 分钟后）。

**`post_restore_wait_should_continue`** (`tauri-app/src-tauri/src/monitor/background_task.rs:119-121`)
纯函数 `waited_ms < max_ms`。严格小于才继续轮询：达到上限即收尾不多睡一轮；`max_ms=0` 表示不等待，首轮直接跳过循环。

**`post_restore_adapters_have_ip`** (`tauri-app/src-tauri/src/monitor/background_task.rs:127-137`)
判定「还原后网络是否就绪」：`get_adapters_force()`（`:130`，不用 cached——缓存带 TTL，网卡刚启用时会返回启用前的旧快照，`:123-126` doc）→ `resolve_adapter_names` + `find_dual_adapters` → `a1.is_some() || a2.is_some()`（`:136`）。

**`spawn_background_check_after_outbound_restore`** (`tauri-app/src-tauri/src/monitor/background_task.rs:149-183`)
补检入口：`tauri::async_runtime::spawn` 内把阻塞枚举送 `spawn_blocking`，循环 `while post_restore_wait_should_continue(waited_ms, MAX) && !post_restore_adapters_have_ip(...)` 每轮 `thread::sleep(3s)` 并 `saturating_add` 计时（`:153-161`）。join 失败按「等满上限」处理（`unwrap_or(POST_RESTORE_IP_WAIT_MAX_MS)`，`:163-166`），宁可 warn 排障也不谎报就绪。**未读到 IP 也照发巡检**（`:181`）：让巡检自己给出「配置的适配器均无 IP」结论并走周期重试，比静默不发好排障（`:147-148` doc）。等满上限记 warn（`:168-173`）、就绪记 info（`:174-180`）。钩子只挂在 `finish_outbound_restore` 尾部一处（`outbound_night.rs:1225`）。

### 巡检主体

**`should_reset_logged_online`** (`tauri-app/src-tauri/src/monitor/background_check.rs:25-27`)
**2026-10-22 新增**的纯函数会话失效判定：`reachable && !any_online && has_logged_online`。三个条件缺一不可（doc `:15-24`）：`reachable`——Portal 可达才有依据确认「当前确实未在线」，探测失败/无 IP 时不复位避免瞬时抖动判死刚建立的会话；`!any_online`——主副都不在线；`has_logged_online`——此前登录过才谈得上复位。

**`FORCE_CHECK_AFTER_RESTORE` / `request_check_after_outbound_restore`** (`tauri-app/src-tauri/src/monitor/background_check.rs:38-44`)
**2026-10-26 新增**：还原后补检无视校园网静默期的一次性放行标记（doc `:29-37`）。`request_check_after_outbound_restore`（`:42-44`）置位，由 `finish_outbound_restore`（`outbound_night.rs:1224`）在发起补检前调用；巡检在静默期判定处 `FORCE_CHECK_AFTER_RESTORE.swap(false, ...)` 消费即清（`:94`），只对紧接着这一轮生效。

**`run_background_check`** (`tauri-app/src-tauri/src/monitor/background_check.rs:537-546`)
async 入口：`spawn_blocking` 包阻塞主体，join 失败记 `log_error` 转 `CheckOutcome::Skipped`（`:542-545`）。巡检只负责连通性/Portal/重连，质量检测由定时测试循环独占（`:534-536` doc：`latency.rs` 的 `spawn_latency_test_loop` → `quality_scheduler::run_quality_check`），避免双定时器叠加、60s 节流吞掉用户自定义间隔（2026-09-04 收敛）。

**`run_background_check_blocking`** (`tauri-app/src-tauri/src/monitor/background_check.rs:46-458`)
巡检阻塞主体，顺序即语义：

- `exit.is_quitting` / `cancel_token` 早退（`:47-49`）；`tasks.is_checking.try_acquire()` 单飞保护，抢不到直接返回（`:50-52`）
- `get_adapters_cached()` 为空/失败回退 `get_adapters_force()`（`:60-69`）
- `resolve_adapter_names` + `find_dual_adapters` 解析主/副（`:73-75`）
- **出站切换态整体跳过**：`outbound_restore_active` 为真直接 return（`:80-87`）——切换态下 Portal 检测必失败，会累计触发 MAC 重置提权与整夜告警（`:78-79` 注释，见 decisions/night-outbound-switch）
- **静默期判定**（`:88-92`）：`is_campus_check_silent` 读取；`:94` 消费 `FORCE_CHECK_AFTER_RESTORE` 一次性标记；`campus_silent && !forced` → `cancel_campus_exit`（`:111`）、构造 `on_campus_network=true` 的伪状态（`:112-115`）、按 `Skipped` 返回——不累计退避、不更新 online 状态（`:95-109` 历史缺陷注释与保留动作说明）；`campus_silent && forced` 记 info「本轮无视校园网检测静默期」（`:119-124`）
- `filter_operation_adapters` 后调 `check_campus_network`（`:126-127`）；写回 `current_ssid`/`on_campus_network`；`campus_check_failed = enable_network_name_check && !on_campus`（`:130`）时同时清 `any_adapter_online`/`last_a1_online`/**`has_logged_online`**（`:131-143`，复位点在 `:141`：覆盖「离校后再回校」场景，`:137-140` 注释）
- `campus_check_failed` 分支：emit 全 false 结果（`:155-176`）；`no_configured_ip = a1.is_none() && a2.is_none()` 时只记 info 跳过退出等网络恢复（`:178-181`），否则 `start_campus_exit`（`:183`）
- 校园网通过即 `cancel_campus_exit`（`:190`）
- Portal 探测：`Handle::current().block_on` + 两个 `spawn_blocking` + `tokio::join!` 并行（`:197-223`，`:199-202` 注释解释为何不用 `std::thread::scope`）；单卡顺序探测（`:216-230`）
- `request_failed` 容错：任一适配器 `Error{is_request_failed:true}` → 仅当 `portal_reset_window_open(now_min)`（每日 08:30 后启用，夜间失败不累计、窗口未开先清零残留计数，`:243`、`:264-280`）时调 `handle_portal_request_failure`（主 `:250-255`、副 `:258-263`，按 `AdapterFailureCounter::A1/A2` 分适配器计数）。**2026-10-25 起这是 MAC 重置的唯一触发源**，连续 3 次触发（`:234` 注释仍写「连续5次」，已过期，见 Known Issues）
- 任一 `Success` → 按适配器独立清零 `a1_auth_failure_count`/`a2_auth_failure_count`（单次 `update`，`:290-308`，对齐 failure_tracker「登录成功即重置」语义 `:283-285` 注释）
- 汇总 `online`/`reachable`/`login_available`（`:310-312`）；`online_operator` 由 Portal uid 经 `operator_suffix_from_uid` 推导（`:323-327`）；`secondary_online`/`secondary_message`/`secondary_online_operator` 同口径（`:338-362`）
- `any_online = online || secondary_online == Some(true)`（`:371`）——历史缺陷修复：曾用仅主卡的 `online` 与 `any_adapter_online` 比较，「主断副通」时每拍误报「在线→离线」通知（`:366-370` 注释）
- **会话失效重置**（`:381-387`）：`should_reset_logged_online(reachable, any_online, has_logged_online)` 为真 → 置 false + 记 info「巡检判定未在线（服务可达），已重置登录标记，允许自动登录重新介入」。位置**必须**在 `try_auto_login_on_preparation`（`:427`）之前：本次巡检就能真正登录，不必再等下一拍（`:372-380` 注释：`:141` 的重置只覆盖校园网检测失败这一种失效，覆盖不到「网络名没变、只是会话超时或网卡被禁用后重启」的整夜休眠场景）
- `handle_status_change`（`:388-393`）→ 组织 a1/a2 校园网消息（`:395-402`）→ `emit_background_check_result`（`:404-425`）
- `try_auto_login_on_preparation`（`:427`）→ `try_disconnect_reconnect`（`:429-433`）
- 未重连成功才 `update_network_state`（`:437-439`），避免用重连前旧 Portal 快照覆盖 `any_adapter_online`
- **结局分类**（`:445-451`）：`reachable && any_online` → `Online`（10 分钟慢拍）；`!reachable && prev_online` → `OnlineButPortalUnreachable`（1 分钟快排查）；其余 `NotOnline`；快排查档联动 `trigger_fast_quality_check`（`:452-454`）

**`trigger_fast_quality_check`** (`tauri-app/src-tauri/src/monitor/background_check.rs:463-532`)
「已登录但门户不可达」快排查的联动质量检测（doc `:460-462`）：先经 `update_with_result` 检查 `last_fast_quality_at` 做 **5 分钟节流**（`:469-478`）；`select_adapter_for_quality` 选卡（`:483-486`）后 spawn `quality_scheduler::run_quality_check`（`:494-495`），以 `averageExternalLatency > 0` 判定互联网可达（`:499-504`）：可达只写 login-log「继续观察」（`:507-513`）；全部外网目标失败则通知「互联网不可访问」（mascot-warn，`:514-526`）；与手动检测互斥未执行时下轮重试（`:527-530`）。

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
纯函数：距上次尝试的耗时达到当前退避冷却即可重试。`auto_auth.rs:664-671` 的测试锁定核心断言：失败拉满后冷却期满**必须**放行重试。

**`is_time_window_refusal`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:62-73`)
纯函数：登录失败消息是否为「可登录时间段未到」类拒绝。匹配 6 个中文关键词（「时间段」「时段」「时间范围」「服务时间」「时间未到」「不在允许」，`:67`）与英文 `time range` / `not allowed`+`time`（`:71-72`）。门户检测的 `login_available` 只表示「未在线即可尝试」，不是时段信号——时间窗外检测仍报可登录，登录必然被服务端拒绝（`:57-61` doc）。

**`prep_failures_after_failure`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:78-84`)
纯函数：时间窗拒绝不累加失败计数（保持基础冷却轻探测，窗口打开后下一个检测周期即可登录），其余失败正常累加进入退避。

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
- 限内：发「检测到断线」通知（mascot-alert）+ 登录日志（`:257-266`，前台过程信息走日志通道 `:259` 注释），再 `full_login`（`:271`）；成功：清计数、置 `any_adapter_online`/`has_logged_online`、清 `prep_login_failures`（`:277-284`），写 `login-history("reconnect")`（`:285`）并发结果事件（`:289-291`）
- 超限：释放登录锁（`:295`），第 `max+1` 次发「断线重连已达上限」+「请手动登录」（`:296-299`），之后每 `RECONNECT_REMINDER_INTERVAL`（10）次发「网络仍断线」（`:300-306`）
- 返回 `reconnect_should_report(reconnect_success, within_limit)`（`:314`）——历史缺陷：所有路径返回 false 会让调用方用旧快照覆盖，单适配器用户重连成功后被立即重新标离线并永久困住（`:309-313` 注释）

**`run_auto_login_on_start`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:317-574`)
开机自启登录流程：

- `auto_login_on_start` 关闭直接返回（`:321-323`）
- `initial_delay`：`--autostart` 5000ms，否则 1500ms（`:325-326`）
- spawn `auto_login_on_start` 任务（`:333-573`）；退出中或 `has_logged_online` 已真则直接返回（`:337-339`）
- `get_adapters_force`（spawn_blocking，`:341-353`）；`--autostart` 场景适配器无 IP 时最多重试 3 次 × 3000ms（`:363-378`）
- 校园网检测（`:384-443`）：静默期置 `on_campus_network=true` 跳过（`:388-400`，`:391-393` 注释：与后台巡检同口径，防止开机自启与巡检形成两条整夜空转的认证请求源）；检测任务 panic/取消 → 跳过本次防误踢出（`:402-415`，`:401-404` 注释）；不在校园 → emit 失败事件（`:417-435`），无 IP 跳过退出（`:428-430`）否则 `start_campus_exit` + return（`:432`）
- Portal 探测：`check_portal_full` **只读探测不带凭据**（防错误凭据触发账号锁定，`:456-457` 注释），双卡并行 `spawn_blocking`（`:458-471`）
- 已在线：置 `any_adapter_online`/`has_logged_online`/`prep_login_failures=0`（`:501-505`），emit「已在线」并 return（`:509-510`）
- 未找到主适配器：记 warn（`:519-522`）
- 未在线：`spawn_blocking` 内再查一次 `has_logged_online`（`:526-530`）→ `is_logging_in.try_acquire`（`:531-534`）→ 更新 `last_auto_login_attempt`（`:535`）→ `full_login`（`:536`）
- 成功：置 `has_logged_online=true` + 清失败计数（`:555-560`）；`auto_exit_after_login` 时 `start_auto_exit`（`:565-568`）

### 事件出口与状态写回

**`handle_status_change`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:68-107`)
在线↔离线翻转时的通知器：`current_online != prev_online` 才动作（`:86`）；**仅离线方向**且 `enable_notification` 时弹系统通知（`:92`），用 `update_stats.last_network_change_notification_ms` 做 **60s 节流**（`:93-102`，`:93` 注释：网络抖动快速翻转时 60s 内不重复弹）；未翻转只记 debug（`:104-106`）。

**`emit_background_check_result`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:109-164`)
构造并发出 `background-check-result` 事件：`background_check_count` 在单次 `update_with_result` CAS 内自增并返回新值（`:116-119`，BE-A-10 修复：避免 load+update 两次快照重复计数）；注销保护期内强制 `online=false`、`secondaryOnline=Some(false)`、两个 `*Operator` 置 null（`:127-134`）。事件 JSON 字段（`:136-161`）：`serverAvailable`/`loginAvailable`/`online`/`message`/`adapter1Name`/`adapter2Name`/`secondaryOnline`/`secondaryMessage`/`onlineOperator`/`secondaryOnlineOperator`/`timestamp`/`checkCount`/`isRunning`/`currentSsid`/`onCampusNetwork`/`enableNetworkNameCheck`/`requiredNetworkName`/`campusWifi`/`campusWired`/`a1CampusMessage`/`a2CampusMessage`/`a1OnCampus`/`a2OnCampus`。

**`update_network_state`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:166-202`)
巡检收尾状态写回：先写 `server_available`（`:173`）；注销保护期内直接返回（`:177-183`）；否则写 `any_adapter_online`/`last_a1_online`（`:185-191`），`any_online` 时清 `disconnect_reconnect_count`（`:188-190`）。最后 `reachable && !has_logged_online && online` → 置 `has_logged_online=true` + 清 `prep_login_failures`（`:193-197`），并在 `auto_exit_on_online` 时 `start_auto_exit`（`:198-200`）——`has_logged_online` 的置位点之一。

**`build_adapter_details`** / **`adapter_status_entry`** / **`adapter_disabled_entry`** / **`adapter_disconnected_entry`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:51-65`、`:36-41`、`:43-45`、`:47-49`)
适配器详情拼接（`"名: 消息, …"`，副适配器仅 `is_secondary_adapter_enabled` 时计入，`:59-63`）与状态 JSON 构造器（后两者是「适配器已禁用或未找到」「适配器未连接」的降级文案包装）。

### 校园网判定与 Portal 探测

**`check_campus_network`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:44-300`)
校园网判定主体：`enable_network_name_check` 关闭时退化为「网关可达 ∨ portal 可达」（`:48-75`，网关禁 ping 时 portal 探测仍可判在校园，`:50` 注释；判定在 `:71`）；开启时按名称名单匹配 WiFi SSID（大小写不敏感，`:124`）/ 有线 profile（`:213`）→ /18 子网检查 `is_same_subnet_18`（WiFi 不匹配 `:138`、WiFi 无 SSID `:176`、有线 `:226`）→ 网关可达性 → portal 可达性三级兜底（网关/portal 探测每轮只做一次，闭包缓存 `:88-119`）；仅当该类网卡有 IP 才信任网关可达性（避免其他网卡可达性错误归因，wifi `:187`、wired `:237`）；`on_campus = wifi.on_campus || wired.on_campus`（`:261-262`）；未通过时记 `log_warn` 便于排障（`:287-291`）。默认名单 `i-wxxy、iwxxy-2、iwxxy-3`（`config/model.rs:254-256`）。

**`campus_name_list`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:36-42`)
内部函数：按顿号/全半角逗号/分号/空格/Tab 切分、trim、滤空项（`:34-35` doc）。

**`adapter_campus_status` / `adapter_campus_message`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:23-28`、`:30-32`)
按适配器名从 `CampusCheckResult` 取单连接状态/消息（无线查 `wifi`、有线查 `wired`，`:26`）。

**`is_campus_check_silent`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:307`)
静默期判定实现已下沉到双端共享面 `crate::config::schedule::is_campus_check_silent`（2026-10-26：此前桌面巡检、安卓巡检、安卓开机自启、无头 core 各写一份同语义边界判定，违反「新增跨平台判定优先放共享面」的约定，`:302-306` doc）。此处仅保留 crate 内转出（`pub use`），调用方可继续按 `monitor` 模块路径引用。判定本体的三条边界用例已随函数迁移到 `config/schedule.rs` 的同名测试模块（`:324-325` 注释）。

**`check_adapter_portal`** (`tauri-app/src-tauri/src/monitor/portal_check.rs:59-93`)
单适配器 Portal 探测，只读、绝不携带凭据；`check_portal_full(&adapter.ip, Some(&adapter.name))`（`:63`），`error_kind=="request_failed"` → `Error{is_request_failed:true}` + emit login-log error（`:65-72`）；`Ok` → `Success`（`:74-81`）；`Err` → `Error{false}`（`:83-92`）。

### 夜间出站联动与定时动作

**`finish_outbound_restore`** (`tauri-app/src-tauri/src/monitor/outbound_night.rs:1206-1227`)
还原收尾，**自动还原 / 手动还原 / 启动对账三条路径的唯一汇合点**：`clear_outbound_snapshot`（`:1231-1251`，三份快照加待切换状态一起清、落盘成功才 store 内存）失败则 `mark_outbound_failure` 并返回 false（`:1207-1210`，让退避闸继续关住重放，防止「无失败历史」误判造成每拍写跃点的重放风暴，`:1201-1205` doc）；清成功才清退避状态与「立即切换」当日 hold（`:1211-1214`）；**尾部先置 `FORCE_CHECK_AFTER_RESTORE`（`:1224`）再调 `spawn_background_check_after_outbound_restore`（`:1225`）**：此时切换态已清（巡检不再被出站切换态闸门跳过）、校园网卡已启用，立刻补一拍巡检让自动登录第一时间评估（`:1215-1223` 注释：否则要等一整个巡检周期——轻量化下 300s，正是「整夜休眠后到点不自动登录」的缺口）。钩子只挂这一处。

**`apply_outbound_switch`** (`tauri-app/src-tauri/src/monitor/outbound_night.rs:469-691`)
切换主流程（`OutboundActionGuard` RAII 单飞闸 `:81-96`，`OUTBOUND_ACTION_BUSY` `:78`；退避闸 `:474-477`）：选择有 IP 且绑源探测判非校园的出站候选卡（`:502-520`，无候选不进退避、每天提醒一次 `:513-518`）；**外网可达探测不阻断**（`:528-551`，2026-10-26 取代登录闸——登录闸方向相反整夜不切，`:401-413` doc）；**快照落盘先行**（`outbound_metric_restore`/`outbound_disabled_adapters` 先写配置再动系统，`:604-616`，崩溃也能凭快照还原）；`write_outbound_metric`（`:308-369`）改跃点、禁用校园卡（`:638-652`）、`best_route_if_index_v4` 路由级验证已离开校园网卡（`:658-685`），失败进重放。

**`apply_outbound_restore`** (`tauri-app/src-tauri/src/monitor/outbound_night.rs:886-1199`)
还原主流程三步反向：① 历史兜底路由清理（`:935-1026`，精确四元组匹配才删，唯一默认路径保留不删防断网 `:951-967`）；② 跃点写回（`:1029-1087`，目标卡已消失按已还原处理 `:1032-1034`）；③ 启用校园卡（`:1094-1165`，`enable_campus_adapter` `:280-291`，problem 22 时走 pnputil 分支 `:271-278` doc）；跃点快照损坏按「已还原」继续、只每天提醒一次（`:893-907`）；三份快照全空的非切换态直接走 `finish_outbound_restore`（`:908-918`）；全部步骤成功的正常收尾同样汇到 `finish_outbound_restore`（`:1194-1198`）。

**`manual_outbound_restore`** (`tauri-app/src-tauri/src/monitor/outbound_night.rs:1301-1314`)
手动还原：先校验处于切换态；**先清 hold 再清失败状态**（`:1307-1308`，否则还原失败后还原窗内的自动重试会被 hold 一并冻结），再调 `apply_outbound_restore`（`:1309-1313`）。

**`reconcile_outbound_on_startup`** (`tauri-app/src-tauri/src/monitor/outbound_night.rs:1321-1379`)
启动对账（`run_scheduled_action_loop` 启动前一次性、`spawn_blocking` 线程内，`monitor/scheduled.rs:60-61`）：三份快照任一非空说明上次留下了切换态（崩溃/被杀/睡眠跨过恢复点）——恢复窗口内或功能已关闭则立即还原（`:1335-1339`）；夜间窗口内只有「目标卡仍在用且判定为非校园网」才重放切换（`:1377-1378`），跃点快照不可用/目标卡已消失/回到校园网都按还原处理（`:1342-1376`）。

**`run_scheduled_action_loop`** (`tauri-app/src-tauri/src/monitor/scheduled.rs:54-198`)
定时动作循环（30s tick，`SCHEDULED_TICK_MS` `:44`，doc `:1-22`）：

- 启动对账（`:60-61`）与定时注销对账 `reconcile_scheduled_logout_on_startup`（`:63` 调用；函数 `:214-229`）——冷启动当日注销时刻已过只消耗额度不执行（防刚开应用被踢下线）；定时登录不消耗（补登录=补上线是预期，`:206-213` doc）
- 循环内 `evaluate_scheduled`（共享 `evaluate_scheduled_actions` 双端单点，`:238-249`）→ `(fire_login, fire_logout)`；**出站切换态下 `fire_login=false` 且不消耗当日标记**（`:87-94` 注释：还原成功后下一拍过点补触发）
- 夜间出站动作 `evaluate_outbound_action_with_restore` → `Switch`/`Restore`/`None`（`:114-171`；`None` 拍做 `needs_replay` 补齐（`:145-157`，纯函数 `outbound_night.rs:207-209`）+ 待切换残留清理 `drive_pending_switch`（`:161-163`）+ `watchdog_re_disable_campus` 夜间看门狗（`:167-169`，函数 `outbound_night.rs:805-872`））
- 运营商夜切 `evaluate_night_switch_gated_with_restore` 用**拍后** `outbound_active` 门控（切侧从热点登录 portal 必失败让位，`:178-196`）
- 注销成功分支 `perform_logout`（`:396-430`）：`is_logging_out` 互斥（`:399-406`）；成功后取消自动退出 + `failure_tracker::reset_all`（`:419-421`）+ 60s 注销保护期（`has_logged_online=false`、清重连计数、`last_auto_login_attempt=now`、`logout_protected_until`，`:422-428`）——`has_logged_online` 的第二类复位点
- 运营商夜切验证 `verify_night_switch`（`:436-473`）：等 15s → GET `{portal_origin}/drcom/chkstatus?callback=dr1003` 核对 `uid_matches`（3 次重试隔 5s，`:485-503`），仍不符 → 注销（`:454`）→ 复登（`:455`）→ 复验，最终不符通知「夜切验证失败」（mascot-alert，`:462-471`）；`night_verify_config`（`:477-481`）用户已手动改走其他运营商时跳过验证；`portal_origin`（`:534-544`）portal_url 空时回退默认门户；`expected_operator_display`（`:547-553`）空串即「无锡学院」

**`spawn_login_action` / `SCHEDULED_LOGIN_MAX_RETRIES = 20`** (`tauri-app/src-tauri/src/monitor/scheduled.rs:285-341`、`:346`)
定时登录与夜切登录共用执行体：抢 `is_logging_in` 互斥锁，抢不到本拍跳过并对外发 `scheduled-action-skipped` 事件（`:291-300`，审计 A7）；当日额度标记在抢锁成功后置位（`:306-308`）。**2026-10-24 失败补试**（`:320-339`）：登录失败且当日补试未用尽时回退额度标记（`login_day` 复位为 `i32::MIN`，`:332`），下一拍「过点补触发」再试；补试用尽（`bump_login_retry` 达 20，`:321-329`）才保留当日标记停止；成功则 `reset_login_retry` + `post_login_handler`（`:315-319`）。补试窗口 20 次 × 30s ≈ 10 分钟（`:343-345` doc）。

**`NightOutboundAction` / `evaluate_night_outbound_with_restore` / `is_night_outbound_guard_window_with_restore` / `outbound_restore_active`** (`tauri-app/src-tauri/src/config/outbound_switch.rs:10-17`、`:37-61`、`:85-99`、`:106-108`)
夜间出站决策纯函数（双端单点）：`evaluate_night_outbound_with_restore` 含凌晨补切（`!restore_active && now < restore_start` → Switch，`:54-56`）与过点补触发（`:57-60`）；恢复起点可配置（`nightRestoreMinutes`，`restore_start_minutes` 归一化，`:44`），`evaluate_night_outbound`（`:25-33`）是仅测试期可用的默认常量包装（`:19-23` doc：生产路径走默认包装等于把用户设置写死）；守护窗口=当日切换时刻起至恢复窗开（`:82-84` doc），窗口内巡检不出自动启用目标；`outbound_restore_active` 三份快照（metric/disabled/standby）任一非空即切换态（`:101-105` doc：兜底路由快照自 2026-10-18 事故后不再创建但仍参与判定）。

### 生命周期与通知

**`start_campus_exit` / `cancel_campus_exit` / `cancel_campus_exit_with_notification` / `start_auto_exit`** (`tauri-app/src-tauri/src/infra/lifecycle.rs:23-143`、`:146-162`、`:165-189`、`:191-272`)
生命周期倒计时（`CAMPUS_MINIMIZE_DELAY_MS=30000` / `CAMPUS_EXIT_DELAY_MS=60000`，`:12-13`）：校园网验证失败 → 先 CAS `campus_exit_started` 再设 deadline（`:51-54`，原顺序会永久卡死，`:45-50` 注释）→ emit 倒计时（`:59-61`）+ 通知（mascot-alert，`:63`）+ 注册 Ctrl+Shift+C 取消快捷键（`:66-74`）→ 30s 后 `mark_hidden_by_design` + suspend + hide（`:91-102`，防后台空转）→ 再 30s 最终校验 deadline（`:110-121`）→ detach → `shutdown_and_exit`（`:134-135`）；spawn 失败回滚 CAS + deadline（`:137-142`）。`start_auto_exit`：`auto_exit_cancelled` 已置跳过（`:193-196`）；锁内 deadline 互斥（`:197-206`）；spawn 失败回滚 deadline（`:267-271`）。`is_within_campus_exit_window`（`:17-19`）`end <= start` 退化为仅开始限制。`shutdown_and_exit`（`:319-328`）：`is_quitting=true` → 10s 超时等 `task_manager.shutdown()`（防卡在不响应取消的 DHCP/长 HTTP，`:322-325`）→ `app_handle.exit(0)`。

**`emit_notification`** (`tauri-app/src-tauri/src/infra/notification.rs:16-65`)
系统通知统一出口：`enable_notification` 为假直接返回（`:17-23`）；主窗口**可见且未最小化**（`monitorable = is_visible && !is_minimized`，`:29-34`；`:25-28` 注释：仅看 `is_focused` 会误弹、仅 `is_visible` 因 Win32 最小化仍 true）时不弹——前台信息走应用内事件/日志通道，双通道 toast 重复的历史缺陷由此根除（`:11-13` doc）；Windows 桌面优先自组 toast 带看板娘头像（notify-rust 不暴露图片，`:41-44` 注释），失败降级插件纯文本（`:45-55`）；不向前端发事件；文案中文硬编码（`:14-15` doc）。

### 认证失败计数与 MAC 重置（2026-10-25 重设计）

**`update_auth_failure_count` / `handle_single_adapter_failure`** (`tauri-app/src-tauri/src/auth/failure_tracker.rs:47-81`、`:96-142`)
认证失败（`is_auth_failure`，`AUTH_FAILURE_CODES = ["ac_auth_failed", "1", "4"]`，`:9`、`:19-25`）的计数器：读-增-判定全部在单次 `update_with_result` CAS 内完成（`:64-67`、`:119-128`，防并发双重触发）。**2026-10-25 重设计**：认证失败**不再触发 MAC 重置**（`:73-76`、`:136-137` 注释），连续 5 次（`MAX_FAILURES` `:6`）仅记 warn 提示检查账号状态/可登录时段并清零计数（`:77-80`、`:138-141`）。

**`PORTAL_REQUEST_FAILURE_THRESHOLD = 3` / `handle_portal_request_failure`** (`tauri-app/src-tauri/src/auth/failure_tracker.rs:156-158`、`:170-263`)
MAC 重置的唯一触发源（doc `:156-157`、`:160-169`）：网关不可达时跳过计数并重置（校园网断网/维护期避免误重置，`:179-192`）；网关可达时累加计数，连续 **3** 次触发该适配器 MAC 重置 + DHCP 续租（`:212-262`）；适配器不在校园网子网时改为通知用户更换路由器 MAC（`:235-247`）；触发前对外发 `emit_mac_reset_cycle` 事件（审计 A16，`:226-227`）。`reset_all`（`:145-154`）注销成功时清全部计数并顺带清 `prep_login_failures`（`:151`）。

### 配置常量（桌面）

`tauri-app/src-tauri/src/config/model.rs`（`Config` 全字段 `:10-200`；容器级 `#[serde(default)]` `:9` 缺字段补默认防整体反序列化失败丢配置，`:7-8` 注释）与本链路相关的默认值（`Default` impl `:262-344`）：

- `auto_login_on_start=true`（`:275`）、`enable_background_check=true`（`:283`）、`auto_login_on_preparation=true`（`:286`）、`lightweight_mode=true`（`:280`）
- `background_check_interval=60000`（`:285`，2026-09-20 15s→60s，v3→v4 迁移 `:284` 注释）
- `max_disconnect_reconnect=3`（`:333`）、`auto_login_cooldown_secs=60`（`:334`）
- `campus_check_start_minutes=460`（`:326`，07:40）、`campus_check_end_minutes=1380`（`:327`，23:00 不含；2026-09-13 起 end=1380，`:232-234`）
- `campus_exit_start_minutes=480`（`:324`，8:00）、`campus_exit_end_minutes=1380`（`:325`）
- `scheduled_login_minutes`/`scheduled_logout_minutes=1440`（`:330-331`，禁用哨兵，0=真实 00:00，v4→v5 迁移 `:328-329` 注释）
- `auto_exit_after_login=false`（`:278`，2026-09-20 起默认关，配合夜切 `:276-277` 注释）、`auto_exit_on_online=false`（`:304`）
- `enable_night_operator_switch=false`（`:289`，2026-10-02 默认改回关 `:287-288` 注释）、`enable_night_outbound_switch=false`（`:291`）、`night_restore_minutes=0`（`0`=回落默认 07:30，`:292-293`、归一化见 `config/night_switch::restore_start_minutes` `:66-70` 字段 doc）、`portal_url=http://10.1.99.100`（`:317`）、`campus_gateway=10.2.127.254`（`:321`）、`required_network_name=i-wxxy、iwxxy-2、iwxxy-3`（`:319`，默认值 `:254-256`）、`enable_network_name_check=true`（`:320`）、`log_retention_days=7`（`:332`）、`config_version=5`（`:341`）
- `masked_for_display`/`mask_in_place`（`:350-365`）：密码打码唯一出口（`:347-349` 注释：所有把 Config 发往前端的路径必须经由本方法）

### 安卓端（android/src-tauri/src/monitor_loop.rs）

安卓后台监控与桌面同构但独立实现，决策逻辑抽成纯函数（TDD）。tick = 校园网判定 → Portal 探测 → 掉线自动重登（cooldown + 次数上限）→ emit `background-check-result`/`login-log`（`:1-3` 模块 doc）。

**并发结构与状态**：

- `OUTBOUND_RESTORE_FAIL_COUNT`（`:21`）/ `OUTBOUND_RESTORE_MAX_FAILS=3`（`:23`）：自动还原失败达上限放弃、清标记恢复巡检；`OUTBOUND_SWITCH_FAIL_COUNT`（`:26`）/ `OUTBOUND_SWITCH_MAX_FAILS=3`（`:28`）：注销未确认达上限按断网规律建态交系统侧验证
- `MONITOR_GENERATION`（AtomicU64，`:32`）：代际 token 防 stop→start 竞速误清 running
- `LOGIN_ATTEMPT_LOCK`（tokio Mutex，`:37`）：自动登录 attempt 段互斥（tick / WiFi 事件 / 手动三入口并发），仅覆盖「判在线→should_attempt_login→run_login→计数」段、探测 IO 不持锁（`:34-36` 注释）
- `FORCE_CHECK_AFTER_RESTORE`（AtomicBool，`:47`，doc `:39-46`）：**2026-10-26** 夜间出站还原后无视静默期的一次性放行标记（桌面 `background_check::request_check_after_outbound_restore` 同义），Restore 分支成功后置位（`:931`），`run_check_once` 消费即清（`:1402`）
- `MonitorState`（`:49-79`）：`running`（`:52`）/`check_count`（`:53`）/`consecutive_failures`（`:54`）/`reconnect_count`（`:55-56`，回在线清零）/`last_login_attempt_ms`（`:57`）/`was_online`（`:58`）/`last_result`（`:59`）/`desired_interval_ms`（`:60-62`，逐 tick 热更新）/`idle_interval_ms`（`:63-64`）/`logout_protected_until_ms`（`:65-66`）/`wifi_event_ms`（`:67-68`，去抖）/`notified_online`（`:69-72`，AtomicU8：0 未展示 1 在线 2 未连接 3 非检测时段，仅翻转 notify——每拍重建是稳态功耗点）/`scheduled_login_day`/`scheduled_logout_day`（`:73-78`，当日序号 `num_days_from_ce`，0=从未，跨天自然重置）
- WiFi 事件常量：`WIFI_EVENT_DELAY_MS=2500`（`:84`，DHCP 未就绪 + 风暴合并窗口）、`WIFI_LOST_EVENT_DELAY_MS=0`（`:89`，断开即事实）、`WIFI_EVENT_DEBOUNCE_MS=1000`（`:92`）；`wifi_event_is_burst_start`（`:97-99`）

**决策纯函数**：

- `effective_interval_ms` (`:106-113`)：巡检分档——`base = base_ms.max(5000)`（`:107`）；亮屏 + WiFi 走基础间隔（默认 60s），蜂窝/息屏走闲时间隔（默认 5min）。拉长不改正确性：WiFi 事件仍即时触发、无离线证据保持记忆（`:101-105` 注释）
- `should_attempt_login` (`:117-138`)：`online || !on_campus` → false（`:127-129`）；`!was_online && !auto_login_on_preparation` → false（`:130-133`，启动即离线需显式开启，掉线重连只受总开关管）；`reconnect_count >= max` → false（`:134-136`）；`elapsed >= cooldown*1000`（`:137`）
- `status_value` (`:141-162`)：`getInitData` 的 `backgroundStatus` 与 `get_background_status` 共用出站形状：`isRunning`/`checkCount`/`consecutiveFailures`/`reconnectCount`/`lastResult`/`watcherFailed` + 展平 `online`/`message`/`onlineOperator`/`serverAvailable`/`onCampusNetwork`（`:154-160`；首轮 emit 先于 WebView 监听建立，getInitData 是唯一启动初值来源，`:151-153` 注释）
- `notify_system` (`:185-202`)：关键事件系统通知（`:182-184` 注释：锁屏时用户对掉线完全无感知），mascot 作通知大图，资源缺失降级纯文本（`:190-191` 注释）
- `with_vpn_hint` (`:208-214`)：VPN tun 接管时 socket 旁路失效（SO_BINDTODEVICE 在 secure VPN 或 ROM 封堵时 Denied，`:204-207` 注释），失败通知追加「请在 VPN 中排除本应用或临时关闭 VPN」

**命令与生命周期**：

- `start_background_check` (`:218-266`)：`interval.max(5000)` → desired（`:220-221`）；已运行 → 仅刷新间隔 + [mobile] 重发 `start_monitor` 保活（`:226-239`，Rust 循环活着 ≠ FGS 活着，START_STICKY 幂等）；未运行 → 先启 FGS 成功才置 running（`:241-252`，不残留假「运行中」）；`start_wifi_watcher`（`:258`）；`gen=GENERATION.fetch_add` → spawn `monitor_tick_loop`（`:262-263`）
- `stop_background_check` (`:269-281`)：先推进代际再清标志（`:271-272`）
- WiFi watcher（`:284-373`）：`start_wifi_watcher`（`:292-314`）失败置 `WATCHER_FAILED=true` 退化纯周期检测不静默（`:307-312`）；`handle_wifi_event`（`:335-373`）is_running 兜底（`:340-342`）+ 去抖（`:351-357`）+ lost 零延迟/其余 2500ms（`:360`）+ 时间戳过期自杀（`:365-368`）+ `run_check_once`（`:370`）
- `run_startup_tasks` (`:468-539`)：对齐桌面 `watcher::run_startup_tasks`；2026-09-09 提速：三条启动链（后台检测/质量首测/自动登录）并行 spawn（质量首测 12 外网域名数秒级），就绪窗 1.5s→500ms（`:463-467` 注释）；`current_settings` 失败 → log + emit `startup-failed` + 停 FGS（`:473-494`，BootReceiver 可能已拉起 FGS，「运行中」通知是假的）；`set_log_retention_days`（`:498`，此前重启后恒默认 7）；`night_outbound_restore` 空 → `restore_avoid_bad_wifi`（`:508-510`，快照兜底）；质量：`enable_latency_test ? start_latency_test : check_network_quality` 一次（`:511-523`，定时测试关闭时唯一首屏来源）
- `auto_login_on_start` (`:579-640`)：凭据空 return（`:580-582`）；静默期跳过（`:589-600`，2026-10-25 与桌面同口径，`:584-588` 注释）；`ensure_wifi_bound`（`:602`，WiFi+流量同开默认路由可能落蜂窝）；`probe_with_retry`（`:604-610`，函数 `:545-563` 失败 3s 重试一次）→ `cache_source_ip`（`:611`）；不在校园跳过（`:612-615`）；在校园 → `run_login`（`:617`）；成功 → `was_online=true`（登录成功是在线权威证据，`:624-626`）+ `logout_protected_until_ms=0` + emit true；失败 → 系统通知「启动自动登录失败」带 VPN 提示 + emit false（`:628-638`）
- `monitor_tick_loop` (`:671-730`)：`current=max(5000)`（`:673`，防误配超小间隔打爆探测）；**启动对账**（`:676-693`）：当日注销时刻已过 → 只消耗今日额度不执行（防首拍把刚打开监控的用户踢下线）；定时登录不消耗（补登录=补上线是预期）；循环（`:694-730`）：代际校验（`:698-700`）→ `run_scheduled_actions`（`:704`，P2-32 置于分档与静默期之前，否则被静默期整拍吞掉，`:701-703` 注释）→ 间隔热更新（`:707-712`，重建计时器）→ 分档（`:715-721`，闲时跳拍省整轮 Portal 探测）→ `run_check_once`（`:723`）；退出仅代际匹配才清 running（`:727-729`）
- `ProbeWindowGuard` (`:644-653`)：Drop 必释放 WifiLock/WakeLock（含 panic unwind）；`power_state` (`:658-669`) 查询失败保守 (true,true)（漏检是缺陷，`:655-657` 注释）

**单次巡检 `run_check_once`** (`android/src-tauri/src/monitor_loop.rs:1374-1615`)：

- 出站切换态 → 整轮跳过（`:1389-1392`，闸在 `run_check_once` 内，周期拍/WiFi 事件/手动触发三路径共用；`run_scheduled_actions` 不经此闸——晨间还原判定须在切换态下照常执行，`:1384-1388` 注释）
- **静默期 + FORCE 标记消费**（`:1402-1424`）：`forced_after_restore = FORCE_CHECK_AFTER_RESTORE.swap(false, ...)`（`:1402`）；`start>0 && !forced` 时按 `is_campus_check_silent` 整拍跳过，常驻通知重建为状态码 3「已暂停检测(非检测时段)」仅首次进入时 notify（`:1413-1421`，doc `:1394-1401` 含 2026-10-26 例外说明）
- `ProbeWindowGuard` 持锁省电（`:1428-1433`）→ `ensure_wifi_bound`（`:1437`）→ ① 校园网判定 `probe_campus_with_ssid`（`:1442`，源 IP 缓存同步更新——后台链路此前不缓存，首次重登拿过期 IP 被协议拒 `:1439-1441` 注释；SSID 感知 2026-09-20）
- ② Portal 探测短路（`:1461-1465`）：`(Some,false)` → 「不在校园网,跳过 Portal 探测」、`(None,_)` → 「WiFi 未连接,跳过 Portal 探测」（2026-10-02：离网拍 11s→3s、WiFi 关闭直接给原因，`:1455-1460` 注释；校内 Portal 宕机仍走 HTTP 由页面裁决）
- ③ 三态消费（`:1470-1486`）：仅 `error_kind=None` 翻转在线——Unknown（已在线页面特征失配 GBK 乱码/严格串匹配）/Failed 非可信离线证据保持上一拍记忆（`:1470-1475` 注释，2026-09-09 修复状态点每拍打灰 + 自动重登「已经在线」）；`on_campus=false → online=false`（否则「在线」永久钉住，`:1477-1481` 注释，2026-09-13）
- ④ 状态机（`:1488-1564`）：在线 → `reconnect_count`/`consecutive_failures`/`logout_protected` 清零（`:1489-1493`）；在线翻离线且在校园 → 通知「校园网连接掉线」带 VPN 提示（`:1496-1498`）；自动登录两道闸（`:1504-1520`）：注销保护期 + `consecutive_failures>=5` 熔断（`:1508`，达 5 停，手动成功清零，`:1503` 注释）；should → `LOGIN_ATTEMPT_LOCK`（`:1524`，覆盖记账→登录→计数收尾整段）→ `reconnect_count+1`（`:1526`）；`count>=max` → 只通知「自动重连已达上限」不再 `run_login`（`:1528-1530`）；否则 `run_login` → 成功 log + `login_history::append(...,"auto")` + emit（`:1533-1554`，系统通知仅失败侧）；协议/执行失败 → `consecutive_failures+1` + 通知（`:1550-1561`）
- ⑤ payload（`:1576-1601`）：`serverAvailable`/`loginAvailable`/`online`/`message`（在线→「在线」、wifi_off→「WiFi 未连接」）/`onlineOperator`（`:1571-1575`，uid → `operator_suffix_from_uid`，与桌面 `background_emit` 同口径，安卓此前漏后端字段）/`timestamp`/`checkCount`/`isRunning`/`onCampusNetwork`/`campusMessage`/`currentSsid`/`sourceIp`/`autoLogin` → `MONITOR.last_result` → emit `background-check-result`（`:1601`）→ 常驻通知 `notify_state`（`:1619-1627`，1 在线/2 未连接/4 WiFi 未连接；3=非检测时段）仅翻转才更新（`:1605-1614`）
- 探测线程：`portal_probe_on_little_cores` (`:812-849`)——专用短命线程绑小核（`:808-811` 注释：共享池会把登录等前台任务拖到小核）；裸线程须先取 Handle 线程内 `enter`（`:822-826`，缺 reactor context panic "there is no reactor running"，2026-09-11 真机每拍必现；与桌面 `commands/login.rs:29-33` 2026-09-05 崩溃同款处理）；`catch_unwind` 捕 panic 转 Err 回传（`:831-834`）；线程创建失败降级 `spawn_blocking`（`:842-847`）

**出站切换安卓侧**（与桌面 `outbound_night.rs` 决策共享纯函数，动作安卓自有）：

- `logout_for_outbound_switch` (`:759-784`)：前置在线检查（`:760-766`，离线时注销必失败，跳过请求照常建态且计数清零防跨夜残留）；「离线生效」判据 `radiusOk || unbindOk`（`:772`，MAC 解绑=破坏性踢下线已离线；只看 success 会把 `(false,true)` 误判失败 → 掉线重连 → 再注销整夜摆动，`:753-758` 注释）
- `switch_logout_unconfirmed` (`:789-806`)：fails>=3 → 清零 + warn「注销未确认生效,按断网规律建态,系统侧验证接管」→ true
- `run_scheduled_actions` (`:863-1072`)：夜间出站判定先于运营商夜切（spec §2 顺序硬约束，`:868-873` 注释，互斥由 `outbound_active` 门控达成）；`evaluate_outbound_action_with_restore(..., 0 /* hold 安卓恒 0 */, ..., settings.night_restore_minutes)`（`:885-893`）共享编排函数双端单点；Switch 分支（`:896-915`）：注销（`:899`）→ `night_outbound_restore="logged_out"`（`:903`）→ persist → `ensure_avoid_bad_wifi`（`:911`，函数 `:1164-1200`，WRITE_SECURE_SETTINGS，pm grant 后程序化确保=1，插件记原值快照晨间还原）→ `trigger_wifi_recheck`（`:912`，函数 `:1138-1159`，`report_wifi_unusable`，应用侧唯一合法「让出站口切蜂窝」通道，改跃点属桌面能力）；Restore 分支（`:917-957`）：`night_switch_login`（`:920`）成功 → 清标记 + `restore_avoid_bad_wifi`（`:921-933`）+ 置 `FORCE_CHECK_AFTER_RESTORE`（`:931`）；失败计 3 次放弃自动还原清标记恢复巡检（`:934-956`）；动作后重读 `latest_settings`（`:962`，函数 `:744-751`，防 stale 快照复活刚清标记）
- 运营商夜切（`:972-1015`）：`evaluate_night_switch_gated_with_restore`（`:976-985`，出站切换态让位）→ SwitchToCampus/Restore → `persist_settings`（`:1077-1100`，save_to 重新加密落盘 + 缓存 + R2 自动建号 + `emit_config_changed`，否则 UI 停留旧运营商后续保存用旧快照覆盖回去，`:1095-1098` 注释）→ `night_switch_login`（`:1106-1132`，直连 `run_login` 内核，无校园网探测闸——断网窗口内探测不通过会被 `auto_login_on_start` 的「不在校园网跳过」闸无声吞掉，`:1102-1105` 注释）→ `verify_night_switch`（`:1264-1318`，入口 `outbound_switch_active` → 放弃（`:1266-1269`）；复验轮前再查切换态（`:1287-1289`）；`night_switch_verify_round` `:1322-1339` 3×5s；`night_switch_check_once` `:1342-1371` GET `{origin}/drcom/chkstatus?callback=dr1003` 超时 15s）；`portal_origin` 安卓版（`:1237-1249`）同样空值回退默认门户

### 安卓端配置（android/src-tauri/src/config_state.rs）

**`Settings`** (`android/src-tauri/src/config_state.rs:26-140`)
安卓全量配置（camelCase IPC 契约，对齐桌面 `Config` 可适用子集）；`password` 经 AndroidKeyStore AES-GCM 加密落盘（替代桌面 DPAPI，`:1-3` 注释），出站一律掩码。关键字段：`background_check_interval=60_000`（`:169`）、`background_check_idle_interval=300_000`（`:171`，闲时巡检 5min，2026-09-13 起亮屏+WiFi 走 60s 其余拉长，`:81-85` doc）、`max_disconnect_reconnect=3`（`:173`）、`auto_login_cooldown_secs=60`（`:174`）、`campus_check_start_minutes=460`（`:199`）/`end=1380`（`:202`）、`scheduled_login/logout_minutes=1440`（`:206-207`，禁用哨兵）、`enable_night_operator_switch=false`（`:153`，2026-10-02 改回 `:151-152` 注释）、`enable_night_outbound_switch=false`（`:155`，修改系统路由属侵入性 `:44-47` doc）、`night_restore_minutes=0`（`:156-157`，0=回落默认 07:30）、`night_outbound_restore`（`:64-68`，值恒 `"logged_out"`；非空=切换态：巡检整轮跳过、运营商夜切让位、定时登录跳过）、`config_schema_version=7`（`:214`）。

**存储与迁移**：

- `CONFIG_IO_LOCK`（`:17-24`）：配置 IO 串行（防 save_config/switch/delete 读改写竞态；guard 跨 await 持有覆盖 load→merge→save 全序列，`:13-16` 注释）
- `CryptoBridge`（`:221-257`）：mobile → `campus_keystore` encrypt/decrypt（`:234-246`）；host 测试注入可逆假实现（`:219-220` 注释）
- `EncodedSettings`（`:260-265`）：磁盘形态密文（加密失败置空绝不落明文，`:259` 注释）；`load_file`（`:267-280`）损坏/解密失败降级不崩（`:275-278`）；`password_saved_at`（`:287-293`）判据必须落 disk 形态密文位（落盘时外露 password 恒空，照 settings.password 判空永远「未保存」，`:283-286` 注释）
- `save_file`（`:295-325`）：tmp 名带 pid（`:317-321`，遗留 tmp 只能被 `cleanup_stale_config_tmp` `:330-341` 清走，不参与 rename 竞争）→ write → rename（`:322-323`）
- `load_from`（`:343-353`）：migrate + 旧单值 "i-wxxy" 扩为三 SSID（`:346-351`，与桌面 validate 2026-09-27 同语义）
- `migrate_legacy_defaults`（`:372-434`，doc `:355-371`）：v0→1 15s→60s（`:374-376`）；v1→2（2026-09-10）自动化开关刷开（`:377-391`）；v2→3（2026-09-12）质量→false（`:384-386`）；v3→4（2026-09-13）补闲时间隔 300_000（`:392-400`）；v4→5（2026-09-13）检测终点 0→1380（`:401-409`）；v5→6（2026-09-20）质量间隔 60s→600s 刷等于旧默认的值（`:410-418`）；v6→7（2026-09-20）夜切 false→true + scheduled 0→1440（`:419-433`）；迁移结果含版本落盘，用户改回不再覆盖（`:370-371` 注释）
- `masked_for_display`（`:443-452`）非空密码→`***`；`emit_config_changed`（`:458-461`）payload=`{"config":masked}`（掩码必须复用 `masked_for_display` 唯一出口，`:454-457` 注释）；`resolve_password_field`（`:464-473`）：clear→空、空/MASK→回退已存值、否则新值；`get_config`（`:475-488`）、`save_config`（`:490-526`，全程持 `CONFIG_IO_LOCK`，`:496-499`）、`current_settings`（`:529-547`，缓存优先未命中读盘）

### 测试模块

**`background_task.rs` tests** (`tauri-app/src-tauri/src/monitor/background_task.rs:185-203`)
- `post_restore_wait_stops_at_cap`（`:190-196`）：0 与 42000 继续、45000 与 90000 停止——达上限即收尾不多睡一轮
- `post_restore_wait_zero_budget_never_waits`（`:199-202`）：`max_ms=0` 首轮就不进循环直接发巡检

**`background_check.rs` tests** (`tauri-app/src-tauri/src/monitor/background_check.rs:548-577`)
- `no_reset_while_online`（`:553-558`）/ `no_reset_when_unreachable`（`:561-564`，探测失败不能当会话失效）/ `no_reset_when_never_logged_in`（`:567-570`，避免无谓写状态与日志噪音）/ `reset_when_reachable_offline_and_logged_before`（`:573-576`，整夜休眠场景必须复位）

**`auto_auth.rs` tests** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:576-672`)
- `reconnect_success_within_limit_reports_true`（`:580-584`）/ `reconnect_failure_never_reports_true`（`:611-615`）/ `reconnect_success_over_limit_reports_false`（`:617-621`）
- `time_window_refusal_classification`（`:586-599`：中文时段文案命中、认证/凭据/传输类失败与空串不命中）/ `time_window_refusal_does_not_grow_backoff`（`:601-609`：时间窗拒绝不累加计数、其余失败正常累加）
- `reconnect_allowed_within_limit`（`:623-628`）/ `reconnect_allowed_periodically_after_limit`（`:630-637`：超限后每 10 个周期放行一次重连，`(13,3)`/`(23,3)` 为 true）
- `prep_login_backoff_below_limit_uses_base_cooldown`（`:639-644`）/ `prep_login_backoff_grows_exponentially_after_limit`（`:646-652`：60→120→240……）/ `prep_login_backoff_capped`（`:654-660`：封顶 300）/ `prep_login_ready_after_backoff_elapsed_even_at_limit`（`:662-671`：失败拉满后退避期满必须放行重试——原「达上限永久停止」的回归拦截）

**`campus_check.rs` tests** (`tauri-app/src-tauri/src/monitor/campus_check.rs:309-326`)
- `名称名单_解析多分隔符与空项`（`:313-322`）；静默期三条边界用例已迁移到 `config/schedule.rs` 同名测试模块（`:324-325` 注释，不在本文源码集内）

**`outbound_switch.rs` tests** (`tauri-app/src-tauri/src/config/outbound_switch.rs:110-214`)
- `guard_window_covers_switch_time_to_restore_start`（`:116-132`）/ `guard_window_respects_custom_restore_start`（`:135-142`，2026-10-26 自定义恢复起点）/ `restore_window_follows_custom_start`（`:145-158`）/ `switch_time_follows_operator_night_schedule`（`:161-167`）/ `restore_active_covers_all_three_snapshots`（`:169-176`）
- `switch_fires_after_time_when_not_in_switched_state`（`:178-188`：周日 1380→Switch、1379→None、已切换态→None、关闭→None）
- `restore_fires_in_restore_window_only`（`:190-199`：450→Restore、449 保持过夜不提前还原、已还原→None）
- `predawn_missed_switch_is_replayed`（`:201-213`：00:00-06:29 未切换→Switch 补切、切换态凌晨保持→None）

**`scheduled.rs` tests** (`tauri-app/src-tauri/src/monitor/scheduled.rs:555-676`)
- 锁定过点补触发（`到点触发_置当日标记` `:572-592`、`跨天重置_次日再触发` `:594-602`）
- `切换态_跳过定时登录且不消耗当日标记_注销照常`（`:604-631`：还原后过点补触发 `:626-630`）
- `定时登录补试计数_累加清零跨天归零`（`:639-654`）/ `定时登录失败回退额度_补试用尽后当日停止`（`:657-675`，纯函数层模拟 `bump_login_retry` 补试语义）

**`network.rs` tests** (`tauri-app/src-tauri/src/infra/state/network.rs:124-147`)
- `network_state_loads_default_snapshot`（`:128-134`）/ `network_state_updates_snapshot_atomically`（`:136-146`）

**`lifecycle.rs` tests** (`tauri-app/src-tauri/src/infra/lifecycle.rs:330-352`)
- `campus_exit_window_default_8_to_23`（`:334-342`）/ `campus_exit_window_end_le_start_falls_back_to_start_only`（`:344-351`）

**`model.rs` tests** (`tauri-app/src-tauri/src/config/model.rs:368-433`)
- 重连/冷却默认值与 serde 契约（`:372-399`）、账号三字段 serde 契约锁（`:401-432`）

**`failure_tracker.rs` tests** (`tauri-app/src-tauri/src/auth/failure_tracker.rs:265-321`)
- `is_auth_failure_recognizes_known_codes`（`:270-280`）/ `is_auth_failure_rejects_network_error`（`:282-290`）/ `is_auth_failure_rejects_success`（`:292-300`）/ `is_auth_failure_handles_missing_code`（`:302-310`）/ `is_auth_failure_handles_missing_data`（`:312-320`）

**`lightweight.rs` tests** (`tauri-app/src-tauri/src/app/lightweight.rs:129-167`)
- `守卫判定_仅有当次期待且未在退出流程时才拦截`（`:133-139`）/ `轻量化间隔_下限语义`（`:141-150`）/ `几何离屏校验`（`:152-159`）/ `退出守卫_单次语义_取走即清零`（`:161-166`）

**`outbound_night.rs` tests** (`tauri-app/src-tauri/src/monitor/outbound_night.rs:1381-1491`)
- `还原失败告警_首达阈值触发_此后周期重复`（`:1385-1398`）/ `重放闸_仅切换态且留有失败历史时才补齐`（`:1400-1411`）/ `退避窗_起步六十秒_每失败翻倍封顶三百秒`（`:1413-1421`）/ 退避闸系列（`:1423-1457`：无失败历史恒放行、首败/第二败/封顶、时钟回拨按压制）/ `快照解析_空串为空表_非法json报错`（`:1459-1470`）/ `单飞闸_同一时刻只放一个出站动作_释放后可再取`（`:1474-1487`）

**`monitor_loop.rs` tests** (`android/src-tauri/src/monitor_loop.rs:1629-1751`)
- `通知状态_wifi短路独立码`（`:1637-1643`）、`在线或非校园网_永不尝试`（`:1646-1649`）、`掉线重连_不受自动登录开关管`（`:1652-1655`）、`启动即离线_需显式开启自动登录`（`:1658-1661`）、`重连达上限_暂停`（`:1664-1667`）、`cooldown_未到_不重试`（`:1670-1673`）/`_为零_立即重试`（`:1676-1678`）、`wifi事件_首事件与窗口外为起点_窗口内不算`（`:1681-1695`）、`巡检分档_*`（`:1698-1718`，闲时小于基础取基础、下限 5s）、`portal_origin_取scheme加host`（`:1721-1729`）、`期望运营商展示`（`:1732-1735`）、`status_value_展平最近一次检测含运营商`（`:1738-1750`）

**`config_state.rs` tests** (`android/src-tauri/src/config_state.rs:549-903`)
- 加密落盘（`:613-625`，raw 不含明文、读回等于原文）、解密失败置空配置仍可加载（`:627-641`）、旧默认单值 SSID 载入扩为三值（`:643-657`）、默认值业务语义（`:659-695`）、`迁移_旧默认15s升60s且落盘后用户值不被覆盖`（`:697-742`）、`迁移_非旧默认值保持不动`（`:744-756`）、`迁移_v5质量间隔旧默认刷v6且用户值不被覆盖`（`:758-787`）、camelCase 序列化契约（`:789-801`）、掩码出口（`:803-824`）、vpn 密文落盘回环（`:826-844`）、密码字段回退语义与桌面同构（`:846-867`）、空密码不写密文位（`:869-880`）、密码已保存判据落盘后仍为已保存（`:885-902`）

## Architecture

```text
setup_app (app/startup.rs:161)
  └─ run_startup_tasks (watcher.rs:16)
       ├─ enable_background_check ► start_background_check_inner (background_task.rs:7)
       ├─ startup_auto_login ────► run_auto_login_on_start (auto_auth.rs:317)
       └─ scheduled_actions ─────► run_scheduled_action_loop (scheduled.rs:54)
                                    └─ reconcile_outbound_on_startup (outbound_night.rs:1321)
run_background_check (background_check.rs:537) ─► run_background_check_blocking (:46)
  ├─ 出站切换态？ (:80) / 静默期?∧!forced (:110) ──► 不同粒度跳过
  ├─ check_campus_network (:127) fail ─► has_logged_online=false (:141) → start_campus_exit
  ├─ check_adapter_portal ×N（block_on + tokio::join! 并行， :197-230）
  ├─ any_online = online || secondary_online (:371)
  ├─ should_reset_logged_online (:381) ► has_logged_online=false  ← 2026-10-22 新增
  ├─ emit_background_check_result (:404)
  ├─ try_auto_login_on_preparation (:427) / try_disconnect_reconnect (:429)
  └─ update_network_state (:437)
finish_outbound_restore (outbound_night.rs:1206)
  ├─ request_check_after_outbound_restore (:1224)  ← 2026-10-26 静默期放行
  └─ spawn_background_check_after_outbound_restore (:1225)  ← 2026-10-22 新增
       └─ 等适配器 IP ≤45s ──► spawn_background_check_once (background_task.rs:78)
```

安卓端同构：`run_startup_tasks (monitor_loop.rs:468)` → `start_background_check (:218)` → `monitor_tick_loop (:671)` → `run_check_once (:1374)`，出站/夜切动作在 `run_scheduled_actions (:863)`，还原后 `FORCE_CHECK_AFTER_RESTORE` 置位于 `:931`、消费于 `:1402`。

## Data Flow

```text
配置(enable_background_check / interval)
  → 启动装配 run_startup_tasks（auto_login / scheduled_actions 无条件独立分支）
    → 周期 tick（每拍重读间隔 × 动态退避[未登录×2封顶5min / 已登录10min / 门户不可达1min]
      × 轻量化下限 300s；安卓分档 60s / 5min）
      → 出站切换态？ → 整拍跳过（防 MAC 重置提权与整夜告警）
      → 静默期？ → 整拍跳过（除非 FORCE_CHECK_AFTER_RESTORE 一次性放行）
      → 适配器解析（主/副，含 disabled/disconnected 降级文案）
      → 校园网判定（静默期伪通过 / SSID·有线 profile·子网·网关·portal 三级兜底）
         ├─ fail → has_logged_online=false → campus-exit-countdown → 30s 最小化 → 60s 退出
         └─ pass → cancel_campus_exit
      → Portal 探测（只读，绝不携带凭据）
         ├─ request_failed ×3（08:30 后）→ 该适配器 MAC 重置 + DHCP 续租
         │   （2026-10-25 起：认证失败不再触发 MAC 重置，仅计数+日志）
         └─ Success → 清零该适配器的失败计数
      → any_online = 主卡在线 || 副卡在线
      → 【会话失效重置】reachable && !any_online && has_logged_online → 复位标记（同拍即可登录）
      → 状态翻转通知（仅离线方向，60s 节流）+ background-check-result 事件
      → 情况分支
         ├─ 未在线且 login 可用 → 准备自动登录（已登录过 → 保护期 → 退避冷却 → 登录锁）
         │    ├─ login_available 上升沿（门户恢复）→ 复位失败计数，立即退出退避
         │    ├─ 成功置 has_logged_online / 失败累加 prep_login_failures
         │    │    （≥5 指数退避封顶 5min，不放弃；「时间窗未到」类拒绝不计数）
         ├─ 曾在线且现离线 → 断线重连（计数 +1，≤3 次内每次都重连；
         │    超限后每 10 个计数放行一次重连，其间按序提醒）
         └─ 在线恢复 → update_network_state → auto_exit_on_online? → 自动退出倒计时

定时登录（scheduled.rs，30s 拍）
  → evaluate_scheduled 过点补触发 → spawn_login_action 抢锁 → 置当日标记 → full_login
    ├─ 成功 → post_login_handler
    └─ 失败 → 补试 <20 次回退 login_day 下一拍再试 / 补试用尽当日停止

夜间出站还原（自动排程 / 手动命令 / 启动对账三路）
  → apply_outbound_restore → finish_outbound_restore（唯一收尾汇合点）
    → 【静默期放行】置 FORCE_CHECK_AFTER_RESTORE（还原 07:10 早于静默期 07:40 时下一轮不被跳过）
    → 【还原后补检】轮询等主/副卡取得 IP（≤45s，3s/轮，force 枚举避开 cached TTL）
      → spawn_background_check_once → 同一次巡检即可触发自动登录，不必等下一个巡检周期

安卓（monitor_loop.rs）：
  启动并行装配 → FGS + monitor_tick_loop（30s 对账 → 分档 tick）
    → run_check_once：静默期/切换态闸（FORCE 标记消费）→ 校园判定（SSID+子网+网关+portal）
      → Portal 探测短路 → 三态消费（仅 error_kind=None 翻转在线）→ 状态机
      → 两道闸（保护期+熔断 5 次）→ run_login → background-check-result + 常驻通知按翻转更新
```

## Connections

- [[desktop-monitor]] — `monitor/` 模块群逐文件详解
- [[android-backend]] — `monitor_loop.rs` 状态机与前台服务关系
- [[desktop-auth]] — `auth::service::full_login` 与失败计数器的实现
- [[desktop-app-lifecycle]] — `infra/lifecycle.rs` 退出流程与 `task_manager` 清理
- [[night-outbound-switch]] — 夜间出站切换/还原的完整决策与三份快照语义
- [[lightweight-mode-desktop]] — 轻量化模式的 300s / 1800s 间隔下限
- [[deliberate-background-check-skips]] — 「出站切换态整拍跳过」「静默期整拍跳过」等刻意跳过点清单
- [[config-and-persistence]] — 间隔 / 阈值 / 时段字段的定义与迁移
- [[security-model]] — 自动登录使用内存明文凭据的边界
- [[ipc-command-surface]] — `background-check-result` / `auto-login-result` 事件的消费侧
- [[desktop-network-quality]] — `quality_scheduler` 与快排查联动质量检测

## Known Issues

- **失败上限语义的注释残留（易误导）**：桌面准备自动登录达 5 次上限后的行为已从「会话内永久停止」改为指数退避封顶 5 分钟（`auto_auth.rs:40-50`；`:131-138` 达上限只记 debug 日志不 return，`:129-130` 注释已如实描述新行为），但 `infra/state/network.rs:26` 的 `prep_login_failures` 字段注释仍写「达到上限后本会话停止自动登录」。读代码时以 `prep_login_backoff_secs` 的实现与 `auto_auth.rs:16-21` 模块注释为准。
- **MAC 重置解耦后的注释残留（2026-10-25 新增）**：认证失败已不再触发 MAC 重置（`failure_tracker.rs:84-87`、`:136-137`），但多处旧注释未同步——`background_check.rs:234` 仍写「连续5次 request_failed 时触发 MAC 重置」（实际阈值是 3，`failure_tracker.rs:84-86`），`failure_tracker.rs:84`（`update_dual_adapter_auth_failure` doc）与 `:96`（`handle_single_adapter_failure` doc）仍写「连续 5 次触发该适配器的 MAC 重置」/「处理单个适配器的认证失败计数与 MAC 重置」。以 `:73-76`、`:136-137` 与 `handle_portal_request_failure`（`:170-263`）为准。
- **会话失效重置只覆盖「服务可达」这一支**：`should_reset_logged_online`（`background_check.rs:25-27`）要求 `reachable` 为真才复位。若整夜休眠后 Portal 探测本身也失败（无 IP、网关不可达），`reachable=false`，标记保持不动，自动登录仍被 `auto_auth.rs:125-127` 拦住，只能等下一拍或网络恢复。这是刻意的保守选择（避免瞬时抖动把刚建立的会话判死），代价是「最坏情况多等一个周期」。
- **`has_logged_online` 复位点分散、语义各不相同**：`background_check.rs:141`（校园网检测失败）、`background_check.rs:381-387`（2026-10-22 会话失效重置）、`commands/login.rs:173-178`（全量注销分支，`perform_full_logout_sync`）与 `monitor/scheduled.rs:422-428`（`perform_logout` 用户主动注销）都写同一个 bool，但只有前两处属于「系统判定会话失效」，读代码时容易误以为是同一套语义。
- **还原后补检用同步 `thread::sleep` 轮询**（`background_task.rs:153-161`）：每轮 3s、最多 15 轮，整段占用一个阻塞线程池线程。当前只在还原成功时触发一次，代价可接受；若将来给其他路径复用需改成异步定时器。
- **补检钩子的等待上限 45s 是经验值**：`POST_RESTORE_IP_WAIT_MAX_MS`（`background_task.rs:112`）按 2026-10-22 单次实证定档，DHCP 更慢的机器上会在上限处放弃并发出一拍「无 IP」巡检，仍要靠周期巡检兜底。
- **静默期放行标记是全局单例、无队列**（`background_check.rs:38-44`）：`FORCE_CHECK_AFTER_RESTORE` 是一个 AtomicBool，若还原后连续两轮巡检都抢在消费前，只有第一轮拿到放行；当前时序（补检紧跟还原发起）不会踩到，但若将来把补检拆成多拍需改语义。
- **定时登录补试用尽后当日彻底停止**（`scheduled.rs:321-329`）：`SCHEDULED_LOGIN_MAX_RETRIES=20` 次补试全部失败（约 10 分钟窗口，如登录时段刚开但服务端长时间拒绝）后保留当日标记，到可登录时段也无人再试，只能靠准备自动登录（`auto_login_on_preparation`）或手动登录兜底。
- **时间窗拒绝识别是字符串匹配**（`auto_auth.rs:62-73`）：`is_time_window_refusal` 靠 6 个中文关键词与两组英文子串判定「可登录时间段未到」类拒绝。若门户服务端更换拒绝文案（关键词失配），这类失败会被误计入指数退避，窗口打开后最多要等 5 分钟（封顶 `PREP_LOGIN_BACKOFF_CAP_SECS`）才重试；反之若正常认证失败文案恰好含关键词（如「时间」相关表述），会漏进退避而每拍轻探测。两侧都是文案耦合风险。
- **`prep_login_failures` 的复位入口有三处、时机各不同**：登录成功（`auto_auth.rs:180-183`）、门户恢复上升沿（`auto_auth.rs:107-113`，依赖 `NetworkSnapshot.last_login_available` 默认值 true 起步，`network.rs:56`）、全量注销（`commands/login.rs:154` 的 `failure_tracker::reset_all`，实现在 `failure_tracker.rs:151`，清 `prep_login_failures`）。排障「为什么退避没退出」时需逐一核对这三处是否命中。
- **安卓端不需要这两个补丁，但原因与桌面不同**：安卓**有**夜间出站切换（`run_scheduled_actions` 内 Switch/Restore 分支，`monitor_loop.rs:1512-1573`），但 Restore 分支本身就是直接 `night_switch_login`（`:920`）——还原动作即登录动作，不存在「还原后还要等巡检评估」的缺口，故补检钩子无对应场景；但静默期放行标记安卓**有**对应实现（`FORCE_CHECK_AFTER_RESTORE`，`:47`，置位 `:931`、消费 `:1402`，2026-10-26 与桌面同批）。安卓也没有 `has_logged_online` 这个自动登录门控：它的在线记忆是 `MONITOR.was_online`（`MonitorState` `:58`），只影响「启动即离线需显式开启自动登录」判定（`should_attempt_login` `:117-138`）与通知，不拦自动重登，故「会话失效重置」无对应对象；定时登录在安卓同样到点即置标记、成败不补试（`run_scheduled_actions` `:1037-1049`，无桌面侧的补试语义）。若「整夜休眠后到点不登录」在安卓侧复现，需另找门控点（`should_attempt_login` `:117`、`reconnect_count >= max_reconnect`、`consecutive_failures >= 5` 熔断是仅有的几道闸）。
- **安卓无 MAC 重置链路**：桌面唯一保留的 MAC 重置触发是「门户 HTTP 不可达」连续 3 次（`handle_portal_request_failure`，`failure_tracker.rs:170-263`；认证失败自 2026-10-25 起只计数不重置），安卓完全没有对应实现——安卓的 `consecutive_failures`（`monitor_loop.rs:1508`）只用于停止自动重登熔断，Portal 探测持续失败也不会尝试换 MAC/IP。
- **静默期语义两端不同**：桌面静默期构造 `on_campus_network: true` 后按 `Skipped` 返回（`background_check.rs:110-118`，不走 Portal 探测、不发事件），安卓直接整拍 `return` 不探测（`monitor_loop.rs:1403-1424`），但会把常驻通知重建为「已暂停检测(非检测时段)」（状态码 3，`:1413-1421`）。在线状态的新鲜度两端不可比。
- **两端默认间隔已统一、迁移语义仍在**：桌面 `background_check_interval` 默认 60000（`config/model.rs:285`），安卓同为 60_000（`config_state.rs:169`）；但安卓只在 `config_schema_version < 1` 且旧值恰为 15000 时刷成 60000（`migrate_legacy_defaults` v0→v1，`config_state.rs:374-376`），其余历史取值不动。
- **倒计时常量硬编码**：`CAMPUS_MINIMIZE_DELAY_MS`/`CAMPUS_EXIT_DELAY_MS`（`infra/lifecycle.rs:12-13`）与 `AUTO_EXIT_DELAY_MS`（导入于 `lifecycle.rs:5`，定义在 `infra/state` 模块，不在本文源码集内）：用户只能整体开关（`campus_exit_on_fail`/`auto_exit_after_login`/`auto_exit_on_online`），不能调倒计时长度。
- **`try_disconnect_reconnect` 参数多达 10 个**（`auto_auth.rs:206-217`），`#[allow(clippy::too_many_arguments)]`（`:205`）压不住可读性；`background_check.rs:429-433` 的调用点需逐个对齐位置参数，改动风险高。
- **断线重连超限分支的通知文案仍写「停止自动重连」**（`auto_auth.rs:297` 日志文案），但 `reconnect_attempt_allowed`（`:32-35`）已把超限改为每 10 个计数放行一次重连——文案与行为不一致，排障时不要被日志误导。
- **出站切换态跳过巡检会掩盖真实状态**（`background_check.rs:80-87`）：切换态整拍 `Skipped` 不更新 `any_adapter_online`，前端看到的在线状态是切换前的旧值；配合夜间 23:00 断网，用户在切换态期间打开界面会看到「在线」但实际已断。这是刻意取舍（防误报离线触发自动重登），但状态新鲜度为零。