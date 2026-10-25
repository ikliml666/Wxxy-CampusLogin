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

后台巡检是一条「启动装配 → 周期 tick → 校园网判定 → Portal 探测 → 状态机 → 自动登录 / 断线重连 → 生命周期倒计时 / 通知」的长链路。桌面端实现在 `tauri-app/src-tauri/src/monitor/` 模块群（`watcher` 启动装配门面 + `background_task` 调度 + `background_check` 巡检主体 + `auto_auth` 自动登录决策 + `background_emit` 事件出口），安卓端实现在单个 `android/src-tauri/src/monitor_loop.rs`（决策逻辑抽成纯函数便于 TDD，模块 doc `monitor_loop.rs:567-569`）。两端共享重连上限 3、冷却 60s、注销保护 60s 等阈值语义，但失败重试策略、周期常量、检测时段门控与省电分档策略不同：桌面准备自动登录连续失败达 5 次后进入**指数退避降频**（封顶 5 分钟、有界且不放弃，`prep_login_backoff_secs` 见 `auto_auth.rs:40-50`），安卓连续失败 5 次直接熔断停手（`monitor_loop.rs:1467` 的 `failures_capped` 判定）。

巡检间隔本身也是动态的：周期循环每拍重读配置基础间隔并按上一轮结局调整——未登录按基础间隔 ×2 逐步退避（封顶 5 分钟）、已登录 10 分钟、已登录但门户不可达降为 1 分钟快排查（`background_task.rs:24-26` 注释，结局分类在 `background_check.rs:414-424`）；快排查档会联动一次网络质量检测确认互联网连通性（`trigger_fast_quality_check`，`background_check.rs:436-505`，5 分钟节流由 `NetworkSnapshot.last_fast_quality_at` 承载）。

2026-10-22 修复了两个让「整夜休眠后到点不自动登录、只能手动点」的缺口，构成本文的两个核心新增点：

1. **会话失效重置**（`monitor/background_check.rs:25-27` 纯函数，`:354-360` 调用点）：巡检在调用 `try_auto_login_on_preparation` **之前**，若判定「服务可达 + 主副卡都不在线 + 此前登录过」，就把 `has_logged_online` 复位。此前该标记只在登录成功时置位、永不复位（唯二的复位点只覆盖校园网检测失败与用户主动注销），整夜休眠或夜间出站切换后旧会话已失效，`auto_auth.rs:94-97` 的「已登录过」门控会把自动登录永久拦住。放在评估之前是为了同一次巡检就能真正登录，而不是再等一拍。
2. **还原后立即补检**（`monitor/background_task.rs:149-183`，钩子挂在 `monitor/outbound_night.rs:1228`）：夜间出站还原刚把校园网卡从设备级禁用（PnP problem 22）中启用，Windows 还要数秒到数十秒才拿到 DHCP 地址。此刻立刻巡检只会读到「配置的适配器均无 IP」空转，随后要等一整个巡检周期——轻量化模式下该周期下限 300s（`app/lightweight.rs:77-79`）。钩子在 `finish_outbound_restore`（`monitor/outbound_night.rs:1214-1230`）尾部触发，先轮询等 IP 就绪（最长 45s、3s 一轮），再发起一次性巡检；自动还原 / 手动还原 / 启动对账三条还原路径全部汇到这一个收尾点，钩子只挂一处。

2026-10-24 修订（`monitor/scheduled.rs:20-22` 模块注释）补上了定时登录的第三个缺口：定时登录到点失败时按 30s 拍间隔**有限补试**（`SCHEDULED_LOGIN_MAX_RETRIES = 20`，`scheduled.rs:342`，20 次 × 30s ≈ 10 分钟窗口，评审认为原 3 次 90s 窗口太短），失败即回退当日额度标记让「过点补触发」再试，补试用尽才放弃当日——原「到点即置标记、失败也当日不重试」会让登录时段刚开的服务端瞬时拒绝直接吞掉当日登录。同批修订还包括：准备自动登录达失败上限后改为指数退避（`auto_auth.rs:18-20` 历史缺陷二），断线重连超限后每 10 个周期放行一次重连（`reconnect_attempt_allowed`，`auto_auth.rs:32-35`），二者都把「达上限即永久终态」改为「有界降频、必然重试」。

2026-10-25 起自动修复策略再收敛一处：**认证失败不再触发 MAC 重置**（`failure_tracker.rs:73-76`、`:136-141`）——时间窗外/账号态异常也会返回认证失败码，以此为触发条件会在不可登录时段反复换 MAC/IP；认证失败侧只保留计数与日志（达 5 次仅告警并清零，`:77-80`、`:138-141`）。MAC 重置的唯一触发源改为「门户 HTTP 不可达」，阈值由 5 收紧为 3（`PORTAL_REQUEST_FAILURE_THRESHOLD`，`failure_tracker.rs:156-158`，触发器 `handle_portal_request_failure` `:170-263`）。

当前代码在自动登录决策上还有两处精细化的收敛：其一，**门户恢复上升沿复位退避**——`NetworkSnapshot.last_login_available`（`infra/state/network.rs:30`）记录上一检测周期的 `login_available`，`try_auto_login_on_preparation` 在所有早退**之前**做沿检测（`auto_auth.rs:94-107`）：门户从不可达恢复为可用（上升沿）视为新的登录机会，直接复位 `prep_login_failures` 退出指数退避，不必等退避周期自然到期；其二，**「可登录时间段未到」类拒绝不计入退避**——`is_time_window_refusal`（`auto_auth.rs:62-73`）按中英文关键词识别时段拒绝文案，`prep_failures_after_failure`（`auto_auth.rs:78-84`）对这类失败不累加计数（失败分支 `:187-199`），因为时间窗拒绝注定发生且窗口打开后应立即恢复，指数退避反而会把窗口开启时刻拖后最多 5 分钟。

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
- `online_operator: Option<String>` (`:23-24`): 主适配器在线时由 Portal uid 推导的运营商后缀（`""`=无锡学院、`@telecom`/`@unicom`/`@cmcc`；`None`=离线/未知）
- `secondary_online_operator: Option<String>` (`:25-26`): 副适配器同口径运营商后缀
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

**`CampusCheckResult`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:13-21`) 与 **`ConnectionCampusStatus`** (`:7-11`)
校园网判定结果（serde camelCase，`:14`）：`wifi: Option<ConnectionCampusStatus>` (`:16`)、`wired: Option<ConnectionCampusStatus>` (`:17`)、`on_campus: bool` (`:18`，= wifi ∨ wired 任一判定在校园)、`current_ssid: Option<String>` (`:19`)、`message: String` (`:20`)。`ConnectionCampusStatus` 携带单连接的 `on_campus` (`:8`)/`name` (`:9`)/`message` (`:10`)。

**`NetworkSnapshot`** (`tauri-app/src-tauri/src/infra/state/network.rs:7-34`)
巡检状态机读写的唯一状态载体（`NetworkState = ArcSwap<NetworkSnapshot>`，struct `:63-65`；`new` `:74-78`；`load` 快照读取 `:81-83`；`update` CAS 循环 `:86-100`，`update_with_result` CAS 并返回闭包结果防 TOCTOU `:102-121`）。18 个字段与巡检链路一一对应：

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
- `prep_login_failures: u32` (`:27`): 准备自动登录连续失败次数（会话内计数，登录成功/重置后清零）。**注意**：`:25-26` 注释仍写「达到上限后本会话停止自动登录」，但当前实现已改为指数退避（见下文 `prep_login_backoff_secs`），注释是旧语义残留
- `last_login_available: bool` (`:30`，doc `:28-29`): 上一检测周期的 `login_available`；`false→true` 上升沿表示门户重新可达（新的登录机会），用于复位 `prep_login_failures` 退避。`Default` 中初始为 `true`（`:56`）
- `last_fast_quality_at: Option<Instant>` (`:33`，doc `:31-32`): 「已登录但门户不可达」快排查模式下上次联动质量检测的时间，距上次不足 300s 不再触发

**`AdapterOnlineStatus`** (`tauri-app/src-tauri/src/commands/login.rs:9-16`)
注销后的在线复核结果：`any_online` (`:10`)/`a1_online` (`:11`)/`a2_online` (`:12`)/`probe_ok` (`:15`)。`probe_ok=false` 表示存在探测异常——离线结论不可信，不得断言「已确认离线」（审计 A8，`:13-14` 注释）。由 `check_any_adapter_online` (`login.rs:18-60`) 构造：并行探测双适配器 Portal（`std::thread::scope` + 裸线程先进入 tauri runtime context，`:29-50`；未启用副卡时 `a2_probe=Some(false)` 不参与判定 `:46-48`），`probe_ok = a1_probe.is_some() && a2_probe.is_some()`（`:58`）。

### 桌面端启动与调度

**`setup_app`** (`tauri-app/src-tauri/src/app/startup.rs:155-266`)
桌面启动装配入口。日志目录先探测安装目录 `logs/` 可写、否则回退数据目录（`:163-180`）；logger 先于 config 初始化（`:182-189`）；`set_log_retention_days(config.log_retention_days)`（`:215`）；`silent_start = config.hidden_start && args 含 --autostart`（`:220-225`）；随后依次 build_tray（`:227`）、`start_adapter_watch`（`:230-232`）、`start_cache_refresh_task`（`:233-235`）、`start_update_check_loop` + `cleanup_stale_update_dir`（`:237-240`）、**`run_startup_tasks`**（`:242`）、`record_webview2_runtime_version`/`subscribe_process_failed`（`:244-248`）、`spawn_heartbeat_thread`/`spawn_window_safety_thread`（`:250-251`）、GPU-warmup 后台线程（`:255-263`）。

**`run()` 命令注册** (`tauri-app/src-tauri/src/app/startup.rs:23-153`)
`background` 命令组 4 个命令 `start_background_check`/`stop_background_check`/`trigger_background_check`/`get_background_status` 在 `startup.rs:98-101` 注册；`RunEvent::ExitRequested` 拦截轻量化退出（`take_lightweight_exit_guard` + `should_prevent_exit`，`:138-152`；纯函数判定见 `app/lightweight.rs:44-46`，退出守卫单次语义 `:34-40`）。

**轻量化运行态** (`tauri-app/src-tauri/src/app/lightweight.rs`)
全部为内存态不落盘（`:1-4` 模块 doc）。与本链路相关：`LIGHTWEIGHT_ACTIVE`（`:10`）、`is_lightweight_active`/`set_lightweight_active`（`:26-32`）、退出守卫 `EXPECT_LIGHTWEIGHT_EXIT`（`:14`）与 `arm/take_lightweight_exit_guard`（`:34-40`）、`should_prevent_exit(expecting, is_quitting)`（`:44-46`，仅当次期待且未在主动退出流程时拦截）；**检测间隔下限** `effective_background_interval_ms`（`:77-79`，轻量化时 `base_ms.max(300_000)`）与质量检测下限 `effective_quality_interval_ms`（`:82-84`，`max(1_800_000)`）；ready 信号 `signal_window_ready`/`take_window_ready`/`wait_window_ready_and_show`（`:87-127`）。

**`run_startup_tasks`** (`tauri-app/src-tauri/src/monitor/watcher.rs:16-69`)
启动装配门面（watcher 只保留装配与 re-export：`background_task::start_background_check_inner` 经 `watcher.rs:14`、`campus_check::check_campus_network` 经 `:13`、`background_emit` 的三个条目构造器经 `:14` re-export，保持外部 `watcher::X` 调用路径不变，`:7-9` 注释）：

- `config.enable_background_check` 为真 → spawn `startup_bg_check`（`:21-31`），内部调 `start_background_check_inner`
- `enable_network_quality && enable_latency_test` → spawn `startup_latency` → `spawn_latency_test_loop`（`:33-42`，间隔由循环内每轮重读 `:36` 注释）
- 仅 `enable_network_quality` → spawn `startup_quality_once` → `run_startup_quality_once`（`:43-52`），启动即测一次（v2.4.2，`:44-45` 注释）
- 无条件 spawn `startup_auto_login` → `run_auto_login_on_start`（`:54-59`）
- 无条件 spawn `scheduled_actions` → `run_scheduled_action_loop`（`:61-68`）；定时登录/注销是独立功能（P2-32），不受 `enable_background_check` 管（`:61-62` 注释）

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
补检入口：`tauri::async_runtime::spawn` 内把阻塞枚举送 `spawn_blocking`，循环 `while post_restore_wait_should_continue(waited_ms, MAX) && !post_restore_adapters_have_ip(...)` 每轮 `thread::sleep(3s)` 并 `saturating_add` 计时（`:153-161`）。join 失败按「等满上限」处理（`unwrap_or(POST_RESTORE_IP_WAIT_MAX_MS)`，`:163-166`），宁可 warn 排障也不谎报就绪。**未读到 IP 也照发巡检**（`:181`）：让巡检自己给出「配置的适配器均无 IP」结论并走周期重试，比静默不发好排障（`:147-148` doc）。等满上限记 warn（`:168-173`）、就绪记 info（`:174-180`）。钩子只挂在 `finish_outbound_restore` 尾部一处（`outbound_night.rs:1228`）。

### 巡检主体

**`should_reset_logged_online`** (`tauri-app/src-tauri/src/monitor/background_check.rs:25-27`)
**2026-10-22 新增**的纯函数会话失效判定：`reachable && !any_online && has_logged_online`。三个条件缺一不可（doc `:15-24`）：`reachable`——Portal 可达才有依据确认「当前确实未在线」，探测失败/无 IP 时不复位避免瞬时抖动判死刚建立的会话；`!any_online`——主副都不在线；`has_logged_online`——此前登录过才谈得上复位。

**`run_background_check`** (`tauri-app/src-tauri/src/monitor/background_check.rs:510-519`)
async 入口：`spawn_blocking` 包阻塞主体，join 失败记 `log_error` 转 `CheckOutcome::Skipped`（`:515-518`）。巡检只负责连通性/Portal/重连，质量检测由定时测试循环独占（`:507-509` doc：`latency.rs` 的 `spawn_latency_test_loop` → `quality_scheduler::run_quality_check`），避免双定时器叠加、60s 节流吞掉用户自定义间隔（2026-09-04 收敛）。

**`run_background_check_blocking`** (`tauri-app/src-tauri/src/monitor/background_check.rs:29-431`)
巡检阻塞主体，顺序即语义：

- `exit.is_quitting` / `cancel_token` 早退（`:30-32`）；`tasks.is_checking.try_acquire()` 单飞保护，抢不到直接返回（`:33-35`）
- `get_adapters_cached()` 为空/失败回退 `get_adapters_force()`（`:43-52`）
- `resolve_adapter_names` + `find_dual_adapters` 解析主/副（`:56-58`）
- **出站切换态整体跳过**：`outbound_restore_active` 为真直接 return（`:63-70`）——切换态下 Portal 检测必失败，会累计触发 MAC 重置提权与整夜告警（`:61-62` 注释，见 decisions/night-outbound-switch）
- **静默期**（`:71` 起）：`is_campus_check_silent` 为真则构造 `on_campus: true` 的伪结果（`:89-95`）、`cancel_campus_exit`（`:88`），不跳过整拍
- `filter_operation_adapters` 后调 `check_campus_network`（`:98-99`）；写回 `current_ssid`/`on_campus_network`；`campus_check_failed = enable_network_name_check && !on_campus`（`:103`）时同时清 `any_adapter_online`/`last_a1_online`/**`has_logged_online`**（`:104-116`，复位点在 `:114`：覆盖「离校后再回校」场景，`:110-113` 注释）
- `campus_check_failed` 分支：emit 全 false 结果（`:128-149`）；`no_configured_ip = a1.is_none() && a2.is_none()` 时只记 info 跳过退出等网络恢复（`:150-154`），否则 `start_campus_exit`（`:156`）
- 校园网通过即 `cancel_campus_exit`（`:163`）
- Portal 探测：`Handle::current().block_on` + 两个 `spawn_blocking` + `tokio::join!` 并行（`:170-188`，`:172-175` 注释解释为何不用 `std::thread::scope`）；单卡顺序探测（`:190-203`）
- `request_failed` 容错：任一适配器 `Error{is_request_failed:true}` → 仅当 `portal_reset_window_open(now_min)`（每日 08:30 后启用，夜间失败不累计、窗口未开先清零残留计数，`:216-219`、`:237-253`）时调 `handle_portal_request_failure`（主 `:223-228`、副 `:230-235`，按 `AdapterFailureCounter::A1/A2` 分适配器计数）。**2026-10-25 起这是 MAC 重置的唯一触发源**，连续 3 次触发（`:207` 注释仍写「连续5次」，已过期，见 Known Issues）
- 任一 `Success` → 按适配器独立清零 `a1_auth_failure_count`/`a2_auth_failure_count`（单次 `update`，`:259-281`，对齐 failure_tracker「登录成功即重置」语义 `:256-258` 注释）
- 汇总 `online`/`reachable`/`login_available`（`:283-285`）；`online_operator` 由 Portal uid 经 `operator_suffix_from_uid` 推导（`:296-300`）；`secondary_online`/`secondary_message`/`secondary_online_operator` 同口径（`:311-335`）
- `any_online = online || secondary_online == Some(true)`（`:344`）——历史缺陷修复：曾用仅主卡的 `online` 与 `any_adapter_online` 比较，「主断副通」时每拍误报「在线→离线」通知（`:339-343` 注释）
- **会话失效重置**（`:354-360`）：`should_reset_logged_online(reachable, any_online, has_logged_online)` 为真 → 置 false + 记 info「巡检判定未在线（服务可达），已重置登录标记，允许自动登录重新介入」。位置**必须**在 `try_auto_login_on_preparation`（`:400`）之前：本次巡检就能真正登录，不必再等下一拍（`:345-353` 注释：`:114` 的重置只覆盖校园网检测失败这一种失效，覆盖不到「网络名没变、只是会话超时或网卡被禁用后重启」的整夜休眠场景）
- `handle_status_change`（`:361-366`）→ 组织 a1/a2 校园网消息（`:368-375`）→ `emit_background_check_result`（`:377-398`）
- `try_auto_login_on_preparation`（`:400`）→ `try_disconnect_reconnect`（`:402-406`）
- 未重连成功才 `update_network_state`（`:410-412`），避免用重连前旧 Portal 快照覆盖 `any_adapter_online`
- **结局分类**（`:414-424`）：`reachable && any_online` → `Online`（10 分钟慢拍）；`!reachable && prev_online` → `OnlineButPortalUnreachable`（1 分钟快排查）；其余 `NotOnline`；快排查档联动 `trigger_fast_quality_check`（`:425-427`）

**`trigger_fast_quality_check`** (`tauri-app/src-tauri/src/monitor/background_check.rs:436-505`)
「已登录但门户不可达」快排查的联动质量检测（doc `:433-435`）：先经 `update_with_result` 检查 `last_fast_quality_at` 做 **5 分钟节流**（`:442-451`）；`select_adapter_for_quality` 选卡（`:456-459`）后 spawn `quality_scheduler::run_quality_check`（`:467-468`），以 `averageExternalLatency > 0` 判定互联网可达（`:472-477`）：可达只写 login-log「继续观察」（`:480-486`）；全部外网目标失败则通知「互联网不可访问」（mascot-warn，`:487-499`）；与手动检测互斥未执行时下轮重试（`:500-502`）。

### 自动登录决策

**`RECONNECT_REMINDER_INTERVAL = 10`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:13`)
断线重连超限后的提醒/放行间隔。

**`PREP_LOGIN_MAX_FAILURES = 5` / `PREP_LOGIN_BACKOFF_CAP_SECS = 300`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:22`、`:27`)
准备自动登录连续失败上限与退避封顶（5 分钟）。`PREP_LOGIN_MAX_FAILURES` 对应的历史缺陷一：错误凭据下无限重试 + 每 5 次认证失败触发全适配器 DHCP 释放续租，应用无人值守时周期性断网持续一整夜（`:16-17` 注释）；历史缺陷二：达上限后「本会话永久停止自动登录」，可登录时间窗到达后依旧不尝试——现为指数退避、有界且不放弃（`:18-20` 注释）。

**`reconnect_attempt_allowed`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:32-35`)
纯函数 `reconnect_count <= max_reconnect || (reconnect_count - max_reconnect) % RECONNECT_REMINDER_INTERVAL == 0`：上限内照常；超限后每 10 个计数放行一次重连（降频重试而非进入「只提醒、永不重连」终态），重连成功后计数复位（`:29-31` doc）。

**`prep_login_backoff_secs`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:40-50`)
失败退避时长纯函数：未达上限用普通冷却 `base_cooldown_secs`（`:41-43`）；恰达上限即开始翻倍（`excess = failures - MAX + 1` 封顶 11，`:45`），`base << excess` 后 `min(300).max(base)`（`:46-49`）——保证登录时间窗到达后最多等一个退避周期必然重试（`:37-39` doc）。60s 基准下：5 次→120s、6 次→240s、7 次→300s……封顶 300s。

**`prep_login_cooldown_ready`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:53-55`)
纯函数：距上次尝试的耗时达到当前退避冷却即可重试。`auto_auth.rs:654-663` 的测试锁定核心断言：失败拉满后冷却期满**必须**放行重试。

**`is_time_window_refusal`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:62-73`)
纯函数：登录失败消息是否为「可登录时间段未到」类拒绝。匹配 6 个中文关键词（「时间段」「时段」「时间范围」「服务时间」「时间未到」「不在允许」，`:67-68`）与英文 `time range` / `not allowed`+`time`（`:71-72`）。门户检测的 `login_available` 只表示「未在线即可尝试」，不是时段信号——时间窗外检测仍报可登录，登录必然被服务端拒绝（`:57-61` doc）。

**`prep_failures_after_failure`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:78-84`)
纯函数：时间窗拒绝不累加失败计数（保持基础冷却轻探测，窗口打开后下一个检测周期即可登录），其余失败正常累加进入退避。

**`reconnect_should_report`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:90-92`)
纯函数 `login_success && within_limit`：只有登录成功且未超限才向上报告，调用方据此跳过「用重连前旧 Portal 快照覆盖在线状态」（`:86-89` doc）。

**`try_auto_login_on_preparation`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:94-203`)
准备自动登录，门控链：

- **`login_available` 沿检测**（`:104-117`）：在所有早退之前同步 `last_login_available`（反向沿也要记录，否则离线期间标志滞留 true、门户恢复时上升沿无从识别，`:101-102` 注释）；上升沿（门户重新可达）且此前有失败计数 → 复位 `prep_login_failures=0` 退出退避（`:107-113`）
- `!login_available || online || !config.auto_login_on_preparation`（`:119-121`）
- **`snap.has_logged_online`（`:124-127`）**——2026-10-22 会话失效重置要解开的就是这道门
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

**`run_auto_login_on_start`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:317-566`)
开机自启登录流程：

- `auto_login_on_start` 关闭直接返回（`:321-323`）
- `initial_delay`：`--autostart` 5000ms，否则 1500ms（`:325-326`）
- spawn `auto_login_on_start` 任务（`:333-565`）；退出中或 `has_logged_online` 已真则直接返回（`:336-339`）
- `get_adapters_force`（spawn_blocking，`:341-353`）；`--autostart` 场景适配器无 IP 时最多重试 3 次 × 3000ms（`:363-378`）
- 校园网检测（`:384-434`）：静默期置 `on_campus_network=true` 跳过（`:388-392`）；检测任务 panic/取消 → 跳过本次防误踢出（`:395-407`，`:393-395` 注释）；不在校园 → emit 失败事件（`:415`），无 IP 跳过退出（`:420-421`）否则 `start_campus_exit` + return（`:424`）
- Portal 探测：`check_portal_full` **只读探测不带凭据**（防错误凭据触发账号锁定，`:448-449` 注释），双卡并行 `spawn_blocking`（`:453-462`）
- 已在线：置 `any_adapter_online`/`has_logged_online`/`prep_login_failures=0`（`:493-497`），emit「已在线」并 return（`:501-502`）
- 未找到主适配器：记 warn（`:512`）
- 未在线：`spawn_blocking` 内再查一次 `has_logged_online`（`:518-522`）→ `is_logging_in.try_acquire`（`:523-526`）→ 更新 `last_auto_login_attempt`（`:527`）→ `full_login`（`:528`）
- 成功：置 `has_logged_online=true` + 清失败计数（`:548-552`）；`auto_exit_after_login` 时 `start_auto_exit`（`:557-560`）

### 事件出口与状态写回

**`handle_status_change`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:68-107`)
在线↔离线翻转时的通知器：`current_online != prev_online` 才动作（`:86`）；**仅离线方向**且 `enable_notification` 时弹系统通知（`:92-93`），用 `update_stats.last_network_change_notification_ms` 做 **60s 节流**（`:94-101`，`:93` 注释：网络抖动快速翻转时 60s 内不重复弹）；未翻转只记 debug（`:104-106`）。

**`emit_background_check_result`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:109-164`)
构造并发出 `background-check-result` 事件：`background_check_count` 在单次 `update_with_result` CAS 内自增并返回新值（`:116-119`，BE-A-10 修复：避免 load+update 两次快照重复计数）；注销保护期内强制 `online=false`、`secondaryOnline=Some(false)`、两个 `*Operator` 置 null（`:126-134`）。事件 JSON 字段（`:136-161`）：`serverAvailable`/`loginAvailable`/`online`/`message`/`adapter1Name`/`adapter2Name`/`secondaryOnline`/`secondaryMessage`/`onlineOperator`/`secondaryOnlineOperator`/`timestamp`/`checkCount`/`isRunning`/`currentSsid`/`onCampusNetwork`/`enableNetworkNameCheck`/`requiredNetworkName`/`campusWifi`/`campusWired`/`a1CampusMessage`/`a2CampusMessage`/`a1OnCampus`/`a2OnCampus`。

**`update_network_state`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:166-202`)
巡检收尾状态写回：先写 `server_available`（`:173`）；注销保护期内直接返回（`:177-183`）；否则写 `any_adapter_online`/`last_a1_online`（`:185-191`），`any_online` 时清 `disconnect_reconnect_count`（`:188-190`）。最后 `reachable && !has_logged_online && online` → 置 `has_logged_online=true` + 清 `prep_login_failures`（`:193-197`），并在 `auto_exit_on_online` 时 `start_auto_exit`（`:198-200`）——`has_logged_online` 的置位点之一。

**`build_adapter_details`** / **`adapter_status_entry`** / **`adapter_disabled_entry`** / **`adapter_disconnected_entry`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:51-65`、`:36-41`、`:43-45`、`:47-49`)
适配器详情拼接（`"名： 消息， …"`，副适配器仅 `is_secondary_adapter_enabled` 时计入，`:59-63`）与状态 JSON 构造器（后两者是「适配器已禁用或未找到」「适配器未连接」的降级文案包装）。

### 校园网判定与 Portal 探测

**`check_campus_network`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:44-300`)
校园网判定主体：`enable_network_name_check` 关闭时退化为「网关可达 ∨ portal 可达」（`:48-75`，网关禁 ping 时 portal 探测仍可判在校园，`:50` 注释；判定在 `:71`）；开启时按名称名单匹配 WiFi SSID（大小写不敏感，`:124`）/ 有线 profile（`:213`）→ /18 子网检查 `is_same_subnet_18`（WiFi 不匹配 `:138`、WiFi 无 SSID `:176`、有线 `:226`）→ 网关可达性 → portal 可达性三级兜底（网关/portal 探测每轮只做一次，闭包缓存 `:88-119`）；仅当该类网卡有 IP 才信任网关可达性（避免其他网卡可达性错误归因，wifi `:185-187`、wired `:235-237`）；`on_campus = wifi.on_campus || wired.on_campus`（`:261-262`）；未通过时记 `log_warn` 便于排障（`:288-291`）。默认名单 `i-wxxy、iwxxy-2、iwxxy-3`（`config/model.rs:236-238`）。

**`campus_name_list`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:36-42`)
内部函数：按顿号/全半角逗号/分号/空格/Tab 切分、trim、滤空项（`:34-35` doc）。

**`adapter_campus_status` / `adapter_campus_message`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:23-28`、`:30-32`)
按适配器名从 `CampusCheckResult` 取单连接状态/消息（无线查 `wifi`、有线查 `wired`，`:26`）。

**`is_campus_check_silent`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:306-314`)
静默期判定（now/start/end 均为当日分钟数）：`start == 0` 禁用门控恒 false（`:307-309`）；`now < start` 静默（`:310-312`）；`end > start && now >= end` 静默（`:313`）；`end <= start` 退化为仅受开始时间限制（`:302-305` doc：与 `lifecycle::is_within_campus_exit_window` 退化规则一致，防误设导致检测整天静默失效）。

**`check_adapter_portal`** (`tauri-app/src-tauri/src/monitor/portal_check.rs:59-93`)
单适配器 Portal 探测，只读、绝不携带凭据；`check_portal_full(&adapter.ip, Some(&adapter.name))`（`:63`），`error_kind=="request_failed"` → `Error{is_request_failed:true}` + emit login-log error（`:65-72`）；`Ok` → `Success`（`:74-81`）；`Err` → `Error{false}`（`:83-92`）。

### 夜间出站联动与定时动作

**`finish_outbound_restore`** (`tauri-app/src-tauri/src/monitor/outbound_night.rs:1214-1230`)
还原收尾，**自动还原 / 手动还原 / 启动对账三条路径的唯一汇合点**：`clear_outbound_snapshot`（`:1234-1254`，三份快照加待切换状态一起清、落盘成功才 store 内存）失败则 `mark_outbound_failure` 并返回 false（`:1215-1218`，让退避闸继续关住重放，防止「无失败历史」误判造成每拍写跃点的重放风暴，`:1209-1213` doc）；清成功才清退避状态与「立即切换」当日 hold（`:1219-1222`）；**尾部调用 `spawn_background_check_after_outbound_restore`（`:1228`）**：此时切换态已清（巡检不再被出站切换态闸门跳过）、校园网卡已启用，立刻补一拍巡检让自动登录第一时间评估（`:1223-1227` 注释：否则要等一整个巡检周期——轻量化下 300s，正是「整夜休眠后到点不自动登录」的缺口）。钩子只挂这一处。

**`apply_outbound_switch`** (`tauri-app/src-tauri/src/monitor/outbound_night.rs:486-701`)
切换主流程（`OutboundActionGuard` RAII 单飞闸 `:80-95`，`OUTBOUND_ACTION_BUSY` `:77`；退避闸 `:491-494`）：选择有 IP 且绑源探测判非校园的出站候选卡（`:502-537`，无候选不进退避、每天提醒一次 `:526-537`）；**登录闸**（目标卡必须已在校园 Portal 在线，未登录挂起为待切换状态 `:543-562`）；**快照落盘先行**（`outbound_metric_restore`/`outbound_disabled_adapters` 先写配置再动系统，`:614-626`，崩溃也能凭快照还原）；`write_outbound_metric`（`:305-366`）改跃点、禁用校园卡（`:646-662`）、`best_route_if_index_v4` 路由级验证默认路由已离开校园网卡（`:663-695`），失败进重放。

**`apply_outbound_restore`** (`tauri-app/src-tauri/src/monitor/outbound_night.rs:894-1207`)
还原主流程三步反向：① 历史兜底路由清理（`:934-1034`，精确四元组匹配才删，唯一默认路径保留不删防断网 `:959-975`）；② 跃点写回（`:1035-1095`，目标卡已消失按已还原处理 `:1040-1043`）；③ 启用校园卡（`:1096-1173`，`enable_campus_adapter` `:277-288`，problem 22 时走 pnputil 分支 `:268-275` doc）；跃点快照损坏按「已还原」继续、只每天提醒一次（`:899-915`）；三份快照全空的非切换态直接走 `finish_outbound_restore`（`:916-926`）；全部步骤成功的正常收尾同样汇到 `finish_outbound_restore`（`:1202-1206`）。

**`manual_outbound_restore`** (`tauri-app/src-tauri/src/monitor/outbound_night.rs:1312-1331`)
手动还原：先校验处于切换态（挂起态也允许退出，`:1314-1321`）；**先清 hold 再清失败状态**（`:1324-1325`，否则还原失败后还原窗内的自动重试会被 hold 一并冻结），再调 `apply_outbound_restore`（`:1326-1330`）。

**`reconcile_outbound_on_startup`** (`tauri-app/src-tauri/src/monitor/outbound_night.rs:1338-1396`)
启动对账（`run_scheduled_action_loop` 启动前一次性、`spawn_blocking` 线程内，`monitor/scheduled.rs:58-59`）：三份快照任一非空说明上次留下了切换态（崩溃/被杀/睡眠跨过恢复点）——恢复窗口内或功能已关闭则立即还原（`:1352-1356`）；夜间窗口内只有「目标卡仍在用且判定为非校园网」才重放切换（`:1394-1395`），跃点快照不可用/目标卡已消失/回到校园网都按还原处理（`:1359-1393`）。

**`run_scheduled_action_loop`** (`tauri-app/src-tauri/src/monitor/scheduled.rs:52-194`)
定时动作循环（30s tick，`SCHEDULED_TICK_MS` `:42`，doc `:1-22`）：

- 启动对账（`:58-59`）与定时注销对账 `reconcile_scheduled_logout_on_startup`（`:61` 调用；函数 `:210-225`）——冷启动当日注销时刻已过只消耗额度不执行（防刚开应用被踢下线）；定时登录不消耗（补登录=补上线是预期，`:202-209` doc）
- 循环内 `evaluate_scheduled`（共享 `evaluate_scheduled_actions` 双端单点，`:234-245`）→ `(fire_login, fire_logout)`；**出站切换态下 `fire_login=false` 且不消耗当日标记**（`:85-92` 注释：还原成功后下一拍过点补触发）
- 夜间出站动作 `evaluate_outbound_action` → `Switch`/`Restore`/`None`（`:107-168`；`None` 拍做 `needs_replay` 补齐（`:136-153`，纯函数 `outbound_night.rs:204-206`）+ 待切换补驱动 `drive_pending_switch`（`:159-161`）+ `watchdog_re_disable_campus` 夜间看门狗（`:164-167`，函数 `outbound_night.rs:815-880`））
- 运营商夜切 `evaluate_night_switch_gated` 用**拍后** `outbound_active` 门控（切侧从热点登录 portal 必失败让位，`:172-192`）
- 注销成功分支 `perform_logout`（`:392-426`）：`is_logging_out` 互斥（`:395-402`）；成功后取消自动退出 + `failure_tracker::reset_all`（`:414-417`）+ 60s 注销保护期（`has_logged_online=false`、清重连计数、`last_auto_login_attempt=now`、`logout_protected_until`，`:419-424`）——`has_logged_online` 的第二类复位点
- 运营商夜切验证 `verify_night_switch`（`:432-469`）：等 15s → GET `{portal_origin}/drcom/chkstatus?callback=dr1003` 核对 `uid_matches`（3 次重试隔 5s，`:481-499`），仍不符 → 注销（`:450`）→ 复登（`:451`）→ 复验，最终不符通知「夜切验证失败」（mascot-alert，`:458-467`）；`night_verify_config`（`:473-477`）用户已手动改走其他运营商时跳过验证；`portal_origin`（`:530-540`）portal_url 空时回退默认门户；`expected_operator_display`（`:543-549`）空串即「无锡学院」

**`spawn_login_action` / `SCHEDULED_LOGIN_MAX_RETRIES = 20`** (`tauri-app/src-tauri/src/monitor/scheduled.rs:281-337`、`:342`)
定时登录与夜切登录共用执行体：抢 `is_logging_in` 互斥锁，抢不到本拍跳过并对外发 `scheduled-action-skipped` 事件（`:287-295`，审计 A7）；当日额度标记在抢锁成功后置位（`:302-304`）。**2026-10-24 失败补试**（`:316-335`）：登录失败且当日补试未用尽时回退额度标记（`login_day` 复位为 `i32::MIN`，`:328`），下一拍「过点补触发」再试；补试用尽（`bump_login_retry` 达 20，`:317-325`）才保留当日标记停止；成功则 `reset_login_retry` + `post_login_handler`（`:311-315`）。补试窗口 20 次 × 30s ≈ 10 分钟（`:339-341` doc）。

**`NightOutboundAction` / `evaluate_night_outbound` / `is_night_outbound_guard_window` / `outbound_restore_active`** (`tauri-app/src-tauri/src/config/outbound_switch.rs:10-17`、`:27-44`、`:53-61`、`:68-70`)
夜间出站决策纯函数（双端单点）：`evaluate_night_outbound` 含凌晨补切（`!restore_active && now < RESTORE_START_MINUTES` → Switch，`:37-39`）与过点补触发（`:40-42`）；守护窗口=当日切换时刻起至次日 07:30 恢复窗开（`:46-52` doc），窗口内巡检不出自动启用目标；`outbound_restore_active` 三份快照（metric/disabled/standby）任一非空即切换态（`:63-67` doc：兜底路由快照自 2026-10-18 事故后不再创建但仍参与判定）。

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

`tauri-app/src-tauri/src/config/model.rs`（`Config` 全字段 `:10-182`；容器级 `#[serde(default)]` `:9` 缺字段补默认防整体反序列化失败丢配置，`:7-8` 注释）与本链路相关的默认值（`Default` impl `:244-320`）：

- `auto_login_on_start=true`（`:257`）、`enable_background_check=true`（`:265`）、`auto_login_on_preparation=true`（`:268`）、`lightweight_mode=true`（`:262`）
- `background_check_interval=60000`（`:267`，2026-09-20 15s→60s，v3→v4 迁移 `:266` 注释）
- `max_disconnect_reconnect=3`（`:313`）、`auto_login_cooldown_secs=60`（`:314`）
- `campus_check_start_minutes=460`（`:306`，07:40）、`campus_check_end_minutes=1380`（`:307`，23:00 不含；2026-09-13 起 end=1380，`:214-216`）
- `campus_exit_start_minutes=480`（`:304`，8:00）、`campus_exit_end_minutes=1380`（`:305`）
- `scheduled_login_minutes`/`scheduled_logout_minutes=1440`（`:310-311`，禁用哨兵，0=真实 00:00，v4→v5 迁移 `:308-309` 注释）
- `auto_exit_after_login=false`（`:260`，2026-09-20 起默认关，配合夜切 `:258-259` 注释）、`auto_exit_on_online=false`（`:284`）
- `enable_night_operator_switch=false`（`:271`，2026-10-02 默认改回关 `:269-270` 注释）、`enable_night_outbound_switch=false`（`:273`）、`portal_url=http://10.1.99.100`（`:297`）、`campus_gateway=10.2.127.254`（`:301`）、`required_network_name=i-wxxy、iwxxy-2、iwxxy-3`（`:299`，默认值 `:236-238`）、`enable_network_name_check=true`（`:300`）、`log_retention_days=7`（`:312`）、`config_version=5`（`:317`）
- `masked_for_display`/`mask_in_place`（`:326-336`）：密码打码唯一出口（`:316-318` 注释：所有把 Config 发往前端的路径必须经由本方法）

### 安卓端（android/src-tauri/src/monitor_loop.rs）

安卓后台监控与桌面同构但独立实现，决策逻辑抽成纯函数（TDD）。tick = 校园网判定 → Portal 探测 → 掉线自动重登（cooldown + 次数上限）→ emit `background-check-result`/`login-log`（`:1-3` 模块 doc）。

**并发结构与状态**：

- `OUTBOUND_RESTORE_FAIL_COUNT`（`:21`）/ `OUTBOUND_RESTORE_MAX_FAILS=3`（`:23`）：自动还原失败达上限放弃、清标记恢复巡检；`OUTBOUND_SWITCH_FAIL_COUNT`（`:26`）/ `OUTBOUND_SWITCH_MAX_FAILS=3`（`:28`）：注销未确认达上限按断网规律建态交系统侧验证
- `MONITOR_GENERATION`（AtomicU64，`:32`）：代际 token 防 stop→start 竞速误清 running
- `LOGIN_ATTEMPT_LOCK`（tokio Mutex，`:37`）：自动登录 attempt 段互斥（tick / WiFi 事件 / 手动三入口并发），仅覆盖「判在线→should_attempt_login→run_login→计数」段、探测 IO 不持锁（`:34-36` 注释）
- `MonitorState`（`:39-69`）：`running`（`:42`）/`check_count`（`:43`）/`consecutive_failures`（`:44`）/`reconnect_count`（`:45-46`，回在线清零）/`last_login_attempt_ms`（`:47`）/`was_online`（`:48`）/`last_result`（`:49`）/`desired_interval_ms`（`:50-52`，逐 tick 热更新）/`idle_interval_ms`（`:53-54`）/`logout_protected_until_ms`（`:55-56`）/`wifi_event_ms`（`:57-58`，去抖）/`notified_online`（`:59-62`，AtomicU8：0 未展示 1 在线 2 未连接 3 非检测时段，仅翻转 notify——每拍重建是稳态功耗点）/`scheduled_login_day`/`scheduled_logout_day`（`:63-68`，当日序号 `num_days_from_ce`，0=从未，跨天自然重置）
- WiFi 事件常量：`WIFI_EVENT_DELAY_MS=2500`（`:74`，DHCP 未就绪 + 风暴合并窗口）、`WIFI_LOST_EVENT_DELAY_MS=0`（`:79`，断开即事实）、`WIFI_EVENT_DEBOUNCE_MS=1000`（`:82`）；`wifi_event_is_burst_start`（`:87-89`）

**决策纯函数**：

- `effective_interval_ms` (`:96-103`)：巡检分档——`base = base_ms.max(5000)`（`:97`）；亮屏 + WiFi 走基础间隔（默认 60s），蜂窝/息屏走闲时间隔（默认 5min）。拉长不改正确性：WiFi 事件仍即时触发、无离线证据保持记忆（`:91-95` 注释）
- `should_attempt_login` (`:107-128`)：`online || !on_campus` → false（`:117-119`）；`!was_online && !auto_login_on_preparation` → false（`:120-123`，启动即离线需显式开启，掉线重连只受总开关管）；`reconnect_count >= max` → false（`:124-126`）；`elapsed >= cooldown*1000`（`:127`）
- `status_value` (`:131-152`)：`getInitData` 的 `backgroundStatus` 与 `get_background_status` 共用出站形状：`isRunning`/`checkCount`/`consecutiveFailures`/`reconnectCount`/`lastResult`/`watcherFailed` + 展平 `online`/`message`/`onlineOperator`/`serverAvailable`/`onCampusNetwork`（`:144-150`；首轮 emit 先于 WebView 监听建立，getInitData 是唯一启动初值来源，`:141-143` 注释）
- `notify_system` (`:175-192`)：关键事件系统通知（`:172-174` 注释：锁屏时用户对掉线完全无感知），mascot 作通知大图，资源缺失降级纯文本（`:180-182` 注释）
- `with_vpn_hint` (`:198-204`)：VPN tun 接管时 socket 旁路失效（SO_BINDTODEVICE 在 secure VPN 或 ROM 封堵时 Denied，`:194-197` 注释），失败通知追加「请在 VPN 中排除本应用或临时关闭 VPN」

**命令与生命周期**：

- `start_background_check` (`:208-256`)：`interval.max(5000)` → desired（`:210-211`）；已运行 → 仅刷新间隔 + [mobile] 重发 `start_monitor` 保活（`:216-229`，Rust 循环活着 ≠ FGS 活着，START_STICKY 幂等）；未运行 → 先启 FGS 成功才置 running（`:231-242`，不残留假「运行中」）；`start_wifi_watcher`（`:247-248`）；`gen=GENERATION.fetch_add` → spawn `monitor_tick_loop`（`:252-253`）
- `stop_background_check` (`:259-271`)：先推进代际再清标志（`:261-262`）
- WiFi watcher（`:282-363`）：`start_wifi_watcher`（`:282-304`）失败置 `WATCHER_FAILED=true` 退化纯周期检测不静默（`:296-303`）；`handle_wifi_event`（`:325-363`）is_running 兜底（`:329-332`）+ 去抖（`:341-347`）+ lost 零延迟/其余 2500ms（`:350`）+ 时间戳过期自杀（`:355-361`）+ `run_check_once`（`:360`）
- `run_startup_tasks` (`:458-529`)：对齐桌面 `watcher::run_startup_tasks`；2026-09-09 提速：三条启动链（后台检测/质量首测/自动登录）并行 spawn（质量首测 12 外网域名数秒级），就绪窗 1.5s→500ms（`:453-457` 注释）；`current_settings` 失败 → log + emit `startup-failed` + 停 FGS（`:463-484`，BootReceiver 可能已拉起 FGS，「运行中」通知是假的）；`set_log_retention_days`（`:488`，此前重启后恒默认 7）；`night_outbound_restore` 空 → `restore_avoid_bad_wifi`（`:495-500`，快照兜底）；质量：`enable_latency_test ? start_latency_test : check_network_quality` 一次（`:501-513`，定时测试关闭时唯一首屏来源）
- `auto_login_on_start` (`:569-612`)：凭据空 return（`:570-572`）；`ensure_wifi_bound`（`:573-574`，WiFi+流量同开默认路由可能落蜂窝）；`probe_with_retry`（`:576-582`，函数 `:535-553` 失败 3s 重试一次）→ `cache_source_ip`（`:583`）；不在校园跳过（`:584-587`）；在校园 → `run_login`（`:589`）；成功 → `was_online=true`（登录成功是在线权威证据，`:592-597`）+ `logout_protected_until_ms=0` + emit true；失败 → 系统通知「启动自动登录失败」带 VPN 提示 + emit false（`:600-610`）
- `monitor_tick_loop` (`:643-702`)：`current=max(5000)`（`:645`，防误配超小间隔打爆探测）；**启动对账**（`:648-665`）：当日注销时刻已过 → 只消耗今日额度不执行（防首拍把刚打开监控的用户踢下线）；定时登录不消耗（补登录=补上线是预期）；循环（`:666-696`）：代际校验（`:670-672`）→ `run_scheduled_actions`（`:676`，P2-32 置于分档与静默期之前，否则被静默期整拍吞掉，`:673-675` 注释）→ 间隔热更新（`:679-684`，重建计时器）→ 分档（`:687-694`，闲时跳拍省整轮 Portal 探测）→ `run_check_once`（`:695`）；退出仅代际匹配才清 running（`:699-701`）
- `ProbeWindowGuard` (`:617-625`)：Drop 必释放 WifiLock/WakeLock（含 panic unwind）；`power_state` (`:630-641`) 查询失败保守 (true,true)（漏检是缺陷，`:628-629` 注释）

**单次巡检 `run_check_once`** (`android/src-tauri/src/monitor_loop.rs:1339-1574`)：

- 出站切换态 → 整轮跳过（`:1349-1357`，闸在 `run_check_once` 内，周期拍/WiFi 事件/手动触发三路径共用；`run_scheduled_actions` 不经此闸——晨间还原判定须在切换态下照常执行，`:1349-1353` 注释）
- 校园检测静默期（`:1363-1383`，`start>0`，`now<start || (end>start && now>=end)` → 整拍跳过；状态码 3「已暂停检测(非检测时段)」仅首次进入重建通知 `:1372-1380`）
- `ProbeWindowGuard` 持锁省电（`:1387-1392`）→ `ensure_wifi_bound`（`:1396`）→ ① 校园网判定 `probe_campus_with_ssid`（`:1401`，源 IP 缓存同步更新——后台链路此前不缓存，首次重登拿过期 IP 被协议拒 `:1398-1399` 注释；SSID 感知 2026-09-20）
- ② Portal 探测短路（`:1420-1424`）：`(Some,false)` → 「不在校园网，跳过 Portal 探测」、`(None,_)` → 「WiFi 未连接，跳过 Portal 探测」（2026-10-02：离网拍 11s→3s、WiFi 关闭直接给原因，`:1413-1419` 注释；校内 Portal 宕机仍走 HTTP 由页面裁决）
- ③ 三态消费（`:1435-1445`）：仅 `error_kind=None` 翻转在线——Unknown（已在线页面特征失配 GBK 乱码/严格串匹配）/Failed 非可信离线证据保持上一拍记忆（`:1429-1434` 注释，2026-09-09 修复状态点每拍打灰 + 自动重登「已经在线」）；`on_campus=false → online=false`（否则「在线」永久钉住，`:1436-1440` 注释，2026-09-13）
- ④ 状态机（`:1447-1457`）：在线 → `reconnect_count`/`consecutive_failures`/`logout_protected` 清零（`:1448-1452`）；在线翻离线且在校园 → 通知「校园网连接掉线」带 VPN 提示（`:1455-1457`）；自动登录两道闸（`:1459-1479`）：注销保护期 + `consecutive_failures>=5` 熔断（`:1467`，达 5 停，手动成功清零，`:1462` 注释）；should → `LOGIN_ATTEMPT_LOCK`（`:1483`，覆盖记账→登录→计数收尾整段）→ `reconnect_count+1`（`:1485`）；`count>=max` → 只通知「自动重连已达上限」不再 `run_login`（`:1487-1489`）；否则 `run_login` → 成功 log + `login_history::append(...,"auto")` + emit（`:1491-1513`，系统通知仅失败侧）；协议/执行失败 → `consecutive_failures+1` + 通知（`:1508-1520`）
- ⑤ payload（`:1535-1560`）：`serverAvailable`/`loginAvailable`/`online`/`message`（在线→「在线」、wifi_off→「WiFi 未连接」）/`onlineOperator`（`:1530-1534`，uid → `operator_suffix_from_uid`，与桌面 `background_emit` 同口径，安卓此前漏后端字段）/`timestamp`/`checkCount`/`isRunning`/`onCampusNetwork`/`campusMessage`/`currentSsid`/`sourceIp`/`autoLogin` → `MONITOR.last_result` → emit `background-check-result`（`:1560`）→ 常驻通知 `notify_state`（`:1578-1586`，1 在线/2 未连接/4 WiFi 未连接；3=非检测时段）仅翻转才更新（`:1564-1573`）
- 探测线程：`portal_probe_on_little_cores` (`:784-821`)——专用短命线程绑小核（`:780-783` 注释：共享池会把登录等前台任务拖到小核）；裸线程须先取 Handle 线程内 `enter`（`:794-798`，缺 reactor context panic "there is no reactor running"，2026-09-11 真机每拍必现；与桌面 `commands/login.rs:29-33` 2026-09-05 崩溃同款处理）；`catch_unwind` 捕 panic 转 Err 回传（`:803-806`）；线程创建失败降级 `spawn_blocking`（`:811-819`）

**出站切换安卓侧**（与桌面 `outbound_night.rs` 决策共享纯函数，动作安卓自有）：

- `logout_for_outbound_switch` (`:731-756`)：前置在线检查（`:732-738`，离线时注销必失败，跳过请求照常建态且计数清零防跨夜残留）；「离线生效」判据 `radiusOk || unbindOk`（`:744`，MAC 解绑=破坏性踢下线已离线；只看 success 会把 `(false,true)` 误判失败 → 掉线重连 → 再注销整夜摆动，`:727-730` 注释）
- `switch_logout_unconfirmed` (`:761-778`)：fails>=3 → 清零 + warn「注销未确认生效，按断网规律建态，系统侧验证接管」→ true
- `run_scheduled_actions` (`:835-1037`)：夜间出站判定先于运营商夜切（spec §2 顺序硬约束，`:840-845` 注释，互斥由 `outbound_active` 门控达成）；`evaluate_outbound_action(..., 0 /* hold 安卓恒 0 */, ...)`（`:856-864`）共享编排函数双端单点；Switch 分支（`:866-886`）：注销（`:869`）→ `night_outbound_restore="logged_out"`（`:873`）→ persist → `ensure_avoid_bad_wifi`（`:881`，函数 `:1129-1165`，WRITE_SECURE_SETTINGS，pm grant 后程序化确保=1，插件记原值快照晨间还原）→ `trigger_wifi_recheck`（`:882`，函数 `:1103-1124`，`report_wifi_unusable`，应用侧唯一合法「让出站口切蜂窝」通道，改跃点属桌面能力）；Restore 分支（`:887-924`）：`night_switch_login`（`:890`）成功 → 清标记 + `restore_avoid_bad_wifi`（`:891-899`）；失败计 3 次放弃自动还原清标记恢复巡检（`:901-922`）；动作后重读 `latest_settings`（`:929`，函数 `:716-723`，防 stale 快照复活刚清标记）
- 运营商夜切（`:939-980`）：`evaluate_night_switch_gated`（`:943-950`，出站切换态让位）→ SwitchToCampus/Restore → `persist_settings`（`:1042-1065`，save_to 重新加密落盘 + 缓存 + R2 自动建号 + `emit_config_changed`，否则 UI 停留旧运营商后续保存用旧快照覆盖回去，`:1060-1063` 注释）→ `night_switch_login`（`:1071-1097`，直连 `run_login` 内核，无校园网探测闸——断网窗口内探测不通过会被 `auto_login_on_start` 的「不在校园网跳过」闸无声吞掉，`:1067-1070` 注释）→ `verify_night_switch`（`:1229-1283`，入口 `outbound_switch_active` → 放弃（`:1232-1234`）；复验轮前再查切换态（`:1252-1254`）；`night_switch_verify_round` `:1287-1304` 3×5s；`night_switch_check_once` `:1307-1336` GET `{origin}/drcom/chkstatus?callback=dr1003` 超时 15s）；`portal_origin` 安卓版（`:1202-1214`）同样空值回退默认门户

### 安卓端配置（android/src-tauri/src/config_state.rs）

**`Settings`** (`android/src-tauri/src/config_state.rs:28-128`)
安卓全量配置（camelCase IPC 契约，对齐桌面 `Config` 可适用子集）；`password` 经 AndroidKeyStore AES-GCM 加密落盘（替代桌面 DPAPI，`:1-3` 注释），出站一律掩码。关键字段：`background_check_interval=60_000`（`:155`）、`background_check_idle_interval=300_000`（`:157`，闲时巡检 5min，2026-09-13 起亮屏+WiFi 走 60s 其余拉长，`:77-81` doc）、`max_disconnect_reconnect=3`（`:159`）、`auto_login_cooldown_secs=60`（`:160`）、`campus_check_start_minutes=460`（`:185`）/`end=1380`（`:188`）、`scheduled_login/logout_minutes=1440`（`:192-193`，禁用哨兵）、`enable_night_operator_switch=false`（`:141`，2026-10-02 改回 `:139-140` 注释）、`enable_night_outbound_switch=false`（`:143`，修改系统路由属侵入性 `:44-47` doc）、`night_outbound_restore`（`:60-64`，值恒 `"logged_out"`；非空=切换态：巡检整轮跳过、运营商夜切让位、定时登录跳过）、`config_schema_version=7`（`:197`）。

**存储与迁移**：

- `CONFIG_IO_LOCK`（`:17-24`）：配置 IO 串行（防 save_config/switch/delete 读改写竞态；guard 跨 await 持有覆盖 load→merge→save 全序列，`:13-16` 注释）
- `CryptoBridge`（`:203-240`）：mobile → `campus_keystore` encrypt/decrypt（`:211-223`）；host 测试注入可逆假实现（`:202` 注释）
- `EncodedSettings`（`:243-248`）：磁盘形态密文（加密失败置空绝不落明文，`:242` 注释）；`load_file`（`:250-263`）损坏/解密失败降级不崩（`:258-261`）；`password_saved_at`（`:270-276`）判据必须落 disk 形态密文位（落盘时外露 password 恒空，照 settings.password 判空永远「未保存」，`:265-269` 注释）
- `save_file`（`:278-308`）：tmp 名带 pid（`:297-304`，遗留 tmp 只能被 `cleanup_stale_config_tmp` `:313-324` 清走，不参与 rename 竞争）→ write → rename（`:305-306`）
- `load_from`（`:326-336`）：migrate + 旧单值 "i-wxxy" 扩为三 SSID（`:330-334`，与桌面 validate 2026-09-27 同语义）
- `migrate_legacy_defaults`（`:355-417`，doc `:338-354`）：v0→1 15s→60s（`:357-359`）；v1→2（2026-09-10）自动化开关刷开（`:360-374`）；v2→3（2026-09-12）质量→false；v3→4（2026-09-13）补闲时间隔 300_000（`:375-383`）；v4→5（2026-09-13）检测终点 0→1380（`:384-392`）；v5→6（2026-09-20）质量间隔 60s→600s 刷等于旧默认的值（`:393-401`）；v6→7（2026-09-20）夜切 false→true + scheduled 0→1440（`:402-416`）；迁移结果含版本落盘，用户改回不再覆盖（`:353-354` 注释）
- `masked_for_display`（`:424-430`）非空密码→`***`；`emit_config_changed`（`:436-439`）payload=`{"config":masked}`（掩码必须复用 `masked_for_display` 唯一出口，`:432-435` 注释）；`resolve_password_field`（`:442-451`）：clear→空、空/MASK→回退已存值、否则新值；`get_config`（`:453-466`）、`save_config`（`:468-499`，全程持 `CONFIG_IO_LOCK`，`:474-477`）、`current_settings`（`:502-520`，缓存优先未命中读盘）

### 测试模块

**`background_task.rs` tests** (`tauri-app/src-tauri/src/monitor/background_task.rs:185-203`)
- `post_restore_wait_stops_at_cap`（`:190-196`）：0 与 42000 继续、45000 与 90000 停止——达上限即收尾不多睡一轮
- `post_restore_wait_zero_budget_never_waits`（`:199-202`）：`max_ms=0` 首轮就不进循环直接发巡检

**`background_check.rs` tests** (`tauri-app/src-tauri/src/monitor/background_check.rs:521-550`)
- `no_reset_while_online`（`:526-531`）/ `no_reset_when_unreachable`（`:533-537`，探测失败不能当会话失效）/ `no_reset_when_never_logged_in`（`:539-543`，避免无谓写状态与日志噪音）/ `reset_when_reachable_offline_and_logged_before`（`:545-549`，整夜休眠场景必须复位）

**`auto_auth.rs` tests** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:568-664`)
- `reconnect_success_within_limit_reports_true`（`:572-576`）/ `reconnect_failure_never_reports_true`（`:603-607`）/ `reconnect_success_over_limit_reports_false`（`:609-613`）
- `time_window_refusal_classification`（`:579-591`：中文时段文案命中、认证/凭据/传输类失败与空串不命中）/ `time_window_refusal_does_not_grow_backoff`（`:593-601`：时间窗拒绝不累加计数、其余失败正常累加）
- `reconnect_allowed_within_limit`（`:615-620`）/ `reconnect_allowed_periodically_after_limit`（`:622-629`：超限后每 10 个周期放行一次重连，`(13,3)`/`(23,3)` 为 true）
- `prep_login_backoff_below_limit_uses_base_cooldown`（`:631-636`）/ `prep_login_backoff_grows_exponentially_after_limit`（`:638-644`：60→120→240……）/ `prep_login_backoff_capped`（`:646-652`：封顶 300）/ `prep_login_ready_after_backoff_elapsed_even_at_limit`（`:654-663`：失败拉满后退避期满必须放行重试——原「达上限永久停止」的回归拦截）

**`campus_check.rs` tests** (`tauri-app/src-tauri/src/monitor/campus_check.rs:316-355`)
- `名称名单_解析多分隔符与空项`（`:320-329`）/ `静默期判定_开始时间单边`（`:331-338`）/ `静默期判定_时间段双边`（`:340-347`，[07:40, 23:00) 外静默）/ `静默期判定_结束不晚于开始时退化为单边`（`:349-354`）

**`outbound_switch.rs` tests** (`tauri-app/src-tauri/src/config/outbound_switch.rs:72-150`)
- `guard_window_covers_switch_time_to_restore_start`（`:77-94`）/ `switch_time_follows_operator_night_schedule`（`:96-103`）/ `restore_active_covers_all_three_snapshots`（`:105-112`）
- `switch_fires_after_time_when_not_in_switched_state`（`:114-124`：周日 1380→Switch、1379→None、已切换态→None、关闭→None）
- `restore_fires_in_restore_window_only`（`:126-135`：450→Restore、449 保持过夜不提前还原、已还原→None）
- `predawn_missed_switch_is_replayed`（`:137-149`：00:00-06:29 未切换→Switch 补切、切换态凌晨保持→None）

**`scheduled.rs` tests** (`tauri-app/src-tauri/src/monitor/scheduled.rs:551-672`)
- 锁定过点补触发（`到点触发_置当日标记` `:568-588`、`跨天重置_次日再触发` `:590-598`）
- `切换态_跳过定时登录且不消耗当日标记_注销照常`（`:600-627`：还原后过点补触发 `:622-626`）
- `定时登录补试计数_累加清零跨天归零`（`:635-650`）/ `定时登录失败回退额度_补试用尽后当日停止`（`:652-671`，纯函数层模拟 `bump_login_retry` 补试语义）

**`network.rs` tests** (`tauri-app/src-tauri/src/infra/state/network.rs:124-147`)
- `network_state_loads_default_snapshot`（`:128-134`）/ `network_state_updates_snapshot_atomically`（`:136-146`）

**`lifecycle.rs` tests** (`tauri-app/src-tauri/src/infra/lifecycle.rs:330-352`)
- `campus_exit_window_default_8_to_23`（`:334-342`）/ `campus_exit_window_end_le_start_falls_back_to_start_only`（`:344-351`）

**`model.rs` tests** (`tauri-app/src-tauri/src/config/model.rs:339-404`)
- 重连/冷却默认值与 serde 契约（`:343-369`）、账号三字段 serde 契约锁（`:374-403`）

**`failure_tracker.rs` tests** (`tauri-app/src-tauri/src/auth/failure_tracker.rs:265-321`)
- `is_auth_failure_recognizes_known_codes`（`:270-280`）/ `is_auth_failure_rejects_network_error`（`:282-290`）/ `is_auth_failure_rejects_success`（`:292-300`）/ `is_auth_failure_handles_missing_code`（`:302-310`）/ `is_auth_failure_handles_missing_data`（`:312-320`）

**`lightweight.rs` tests** (`tauri-app/src-tauri/src/app/lightweight.rs:129-167`)
- `守卫判定_仅有当次期待且未在退出流程时才拦截`（`:133-139`）/ `轻量化间隔_下限语义`（`:141-150`）/ `几何离屏校验`（`:152-159`）/ `退出守卫_单次语义_取走即清零`（`:161-166`）

**`monitor_loop.rs` tests** (`android/src-tauri/src/monitor_loop.rs:1588-1710`)
- `通知状态_wifi短路独立码`（`:1595-1602`）、`在线或非校园网_永不尝试`（`:1604-1608`）、`掉线重连_不受自动登录开关管`（`:1610-1614`）、`启动即离线_需显式开启自动登录`（`:1616-1620`）、`重连达上限_暂停`（`:1622-1626`）、`cooldown_未到_不重试`（`:1628-1632`）/`_为零_立即重试`（`:1634-1637`）、`wifi事件_首事件与窗口外为起点_窗口内不算`（`:1639-1654`）、`巡检分档_*`（`:1656-1677`，闲时小于基础取基础、下限 5s）、`portal_origin_取scheme加host`（`:1679-1688`）、`期望运营商展示`（`:1690-1694`）、`status_value_展平最近一次检测含运营商`（`:1696-1709`）

**`config_state.rs` tests** (`android/src-tauri/src/config_state.rs:522-827`)
- 加密落盘（`:586-598`，raw 不含明文、读回等于原文）、解密失败置空配置仍可加载（`:600-614`）、旧默认单值 SSID 载入扩为三值（`:616-630`）、默认值业务语义（`:632-664`）、`迁移_旧默认15s升60s且落盘后用户值不被覆盖`（`:666-711`）、`迁移_非旧默认值保持不动`（`:713-725`）、`迁移_v5质量间隔旧默认刷v6且用户值不被覆盖`（`:727-756`）、camelCase 序列化契约（`:758-766`）、掩码出口（`:768-777`）、密码字段回退语义与桌面同构（`:779-791`）、空密码不写密文位（`:793-804`）、密码已保存判据落盘后仍为已保存（`:809-826`）

## Architecture

```text
setup_app (app/startup.rs:155)
  └─ run_startup_tasks (watcher.rs:16)
       ├─ enable_background_check ► start_background_check_inner (background_task.rs:7)
       ├─ startup_auto_login ────► run_auto_login_on_start (auto_auth.rs:317)
       └─ scheduled_actions ─────► run_scheduled_action_loop (scheduled.rs:52)
                                    └─ reconcile_outbound_on_startup (outbound_night.rs:1338)
run_background_check (background_check.rs:510) ─► run_background_check_blocking (:29)
  ├─ 出站切换态？ (:63) / 静默期？ (:71) ──► 不同粒度跳过
  ├─ check_campus_network (:99) fail ─► has_logged_online=false (:114) → start_campus_exit
  ├─ check_adapter_portal ×N（block_on + tokio::join! 并行， :170-203）
  ├─ any_online = online || secondary_online (:344)
  ├─ should_reset_logged_online (:354) ► has_logged_online=false  ← 2026-10-22 新增
  ├─ emit_background_check_result (:377)
  ├─ try_auto_login_on_preparation (:400) / try_disconnect_reconnect (:402)
  └─ update_network_state (:411)
finish_outbound_restore (outbound_night.rs:1214)
  └─ spawn_background_check_after_outbound_restore (:1228)  ← 2026-10-22 新增
       └─ 等适配器 IP ≤45s ──► spawn_background_check_once (background_task.rs:78)
```

安卓端同构：`run_startup_tasks (monitor_loop.rs:458)` → `start_background_check (:208)` → `monitor_tick_loop (:643)` → `run_check_once (:1339)`，出站/夜切动作在 `run_scheduled_actions (:835)`。

## Data Flow

```text
配置(enable_background_check / interval)
  → 启动装配 run_startup_tasks（auto_login / scheduled_actions 无条件独立分支）
    → 周期 tick（每拍重读间隔 × 动态退避[未登录×2封顶5min / 已登录10min / 门户不可达1min]
      × 轻量化下限 300s；安卓分档 60s / 5min）
      → 出站切换态？ → 整拍跳过（防 MAC 重置提权与整夜告警）
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
    → 【还原后补检】轮询等主/副卡取得 IP（≤45s，3s/轮，force 枚举避开 cached TTL）
      → spawn_background_check_once → 同一次巡检即可触发自动登录，不必等下一个巡检周期

安卓（monitor_loop.rs）：
  启动并行装配 → FGS + monitor_tick_loop（30s 对账 → 分档 tick）
    → run_check_once：静默期/切换态闸 → 校园判定（SSID+子网+网关+portal）→ Portal 探测短路
      → 三态消费（仅 error_kind=None 翻转在线）→ 状态机 → 两道闸（保护期+熔断 5 次）→ run_login
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
- [[desktop-network-quality]] — `quality_scheduler` 与快排查联动质量检测

## Known Issues

- **失败上限语义的注释残留（易误导）**：桌面准备自动登录达 5 次上限后的行为已从「会话内永久停止」改为指数退避封顶 5 分钟（`auto_auth.rs:40-50`；`:131-138` 达上限只记 debug 日志不 return，`:129-130` 注释已如实描述新行为），但 `infra/state/network.rs:25-26` 的 `prep_login_failures` 字段注释仍写「达到上限后本会话停止自动登录」。读代码时以 `prep_login_backoff_secs` 的实现与 `auto_auth.rs:16-21` 模块注释为准。
- **MAC 重置解耦后的注释残留（2026-10-25 新增）**：认证失败已不再触发 MAC 重置（`failure_tracker.rs:84-87`、`:136-141`），但多处旧注释未同步——`background_check.rs:207` 仍写「连续5次 request_failed 时触发 MAC 重置」（实际阈值是 3，`failure_tracker.rs:84`），`failure_tracker.rs:84`（`update_dual_adapter_auth_failure` doc）与 `:95`（`handle_single_adapter_failure` doc）仍写「连续 5 次触发该适配器的 MAC 重置」/「认证失败计数与 MAC 重置」。以 `:73-76`、`:136-137` 与 `handle_portal_request_failure`（`:170-263`）为准。
- **会话失效重置只覆盖「服务可达」这一支**：`should_reset_logged_online`（`background_check.rs:25-27`）要求 `reachable` 为真才复位。若整夜休眠后 Portal 探测本身也失败（无 IP、网关不可达），`reachable=false`，标记保持不动，自动登录仍被 `auto_auth.rs:124-127` 拦住，只能等下一拍或网络恢复。这是刻意的保守选择（避免瞬时抖动把刚建立的会话判死），代价是「最坏情况多等一个周期」。
- **`has_logged_online` 复位点分散、语义各不相同**：`background_check.rs:114`（校园网检测失败）、`background_check.rs:354-360`（2026-10-22 会话失效重置）、`commands/login.rs:173-178`（全量注销分支，`perform_full_logout_sync`）与 `monitor/scheduled.rs:419-424`（`perform_logout` 用户主动注销）都写同一个 bool，但只有前两处属于「系统判定会话失效」，读代码时容易误以为是同一套语义。
- **还原后补检用同步 `thread::sleep` 轮询**（`background_task.rs:153-161`）：每轮 3s、最多 15 轮，整段占用一个阻塞线程池线程。当前只在还原成功时触发一次，代价可接受；若将来给其他路径复用需改成异步定时器。
- **补检钩子的等待上限 45s 是经验值**：`POST_RESTORE_IP_WAIT_MAX_MS`（`background_task.rs:112`）按 2026-10-22 单次实证定档，DHCP 更慢的机器上会在上限处放弃并发出一拍「无 IP」巡检，仍要靠周期巡检兜底。
- **定时登录补试用尽后当日彻底停止**（`scheduled.rs:318-325`）：`SCHEDULED_LOGIN_MAX_RETRIES=20` 次补试全部失败（约 10 分钟窗口，如登录时段刚开但服务端长时间拒绝）后保留当日标记，到可登录时段也无人再试，只能靠准备自动登录（`auto_login_on_preparation`）或手动登录兜底。
- **时间窗拒绝识别是字符串匹配**（`auto_auth.rs:62-73`）：`is_time_window_refusal` 靠 6 个中文关键词与两组英文子串判定「可登录时间段未到」类拒绝。若门户服务端更换拒绝文案（关键词失配），这类失败会被误计入指数退避，窗口打开后最多要等 5 分钟（封顶 `PREP_LOGIN_BACKOFF_CAP_SECS`）才重试；反之若正常认证失败文案恰好含关键词（如「时间」相关表述），会漏进退避而每拍轻探测。两侧都是文案耦合风险。
- **`prep_login_failures` 的复位入口有三处、时机各不同**：登录成功（`auto_auth.rs:180-183`）、门户恢复上升沿（`auto_auth.rs:107-113`，依赖 `NetworkSnapshot.last_login_available` 默认值 true 起步，`network.rs:56`）、全量注销（`commands/login.rs:154` 的 `failure_tracker::reset_all`，实现在 `failure_tracker.rs:151-160`，`:151` 清 `prep_login_failures`）。排障「为什么退避没退出」时需逐一核对这三处是否命中。
- **安卓端不需要这三个补丁，但原因与桌面不同**：安卓**有**夜间出站切换（`run_scheduled_actions` 内 Switch/Restore 分支，`monitor_loop.rs:1453-1473`/`:887-924`），但 Restore 分支本身就是直接 `night_switch_login`（`:890`）——还原动作即登录动作，不存在「还原后还要等巡检评估」的缺口，故补检钩子无对应场景。安卓也没有 `has_logged_online` 这个自动登录门控：它的在线记忆是 `MONITOR.was_online`（`MonitorState` `:39-69` 的字段 `:48`），只影响「启动即离线需显式开启自动登录」判定（`should_attempt_login` `:107-128`）与通知，不拦自动重登，故「会话失效重置」无对应对象；定时登录在安卓同样到点即置标记、成败不补试（`run_scheduled_actions` `:1002-1014`，无桌面侧的补试语义）。若「整夜休眠后到点不登录」在安卓侧复现，需另找门控点（`should_attempt_login` `:107`、`reconnect_count >= max_reconnect`、`consecutive_failures >= 5` 熔断是仅有的几道闸）。
- **安卓无 MAC 重置链路**：桌面唯一保留的 MAC 重置触发是「门户 HTTP 不可达」连续 3 次（`handle_portal_request_failure`，`failure_tracker.rs:170-263`；认证失败自 2026-10-25 起只计数不重置），安卓完全没有对应实现——安卓的 `consecutive_failures`（`monitor_loop.rs:1467`）只用于停止自动重登熔断， Portal 探测持续失败也不会尝试换 MAC/IP。
- **静默期语义两端不同**：桌面静默期构造 `on_campus: true` 并继续走完整 Portal 探测（`background_check.rs:71` 起），安卓直接整拍 `return` 不探测（`monitor_loop.rs:1363-1383`），但会把常驻通知重建为「已暂停检测(非检测时段)」（状态码 3，`:1372-1380`）。在线状态的新鲜度两端不可比。
- **两端默认间隔已统一、迁移语义仍在**：桌面 `background_check_interval` 默认 60000（`config/model.rs:267`），安卓同为 60_000（`config_state.rs:155`）；但安卓只在 `config_schema_version < 1` 且旧值恰为 15000 时刷成 60000（`migrate_legacy_defaults` v0→v1，`config_state.rs:357-359`），其余历史取值不动。
- **倒计时常量硬编码**：`CAMPUS_MINIMIZE_DELAY_MS`/`CAMPUS_EXIT_DELAY_MS`（`infra/lifecycle.rs:12-13`）与 `AUTO_EXIT_DELAY_MS`（导入于 `lifecycle.rs:5`，定义在 `infra/state` 模块，不在本文源码集内）：用户只能整体开关（`campus_exit_on_fail`/`auto_exit_after_login`/`auto_exit_on_online`），不能调倒计时长度。
- **`try_disconnect_reconnect` 参数多达 10 个**（`auto_auth.rs:206-217`），`#[allow(clippy::too_many_arguments)]`（`:205`）压不住可读性；`background_check.rs:402-406` 的调用点需逐个对齐位置参数，改动风险高。
- **断线重连超限分支的通知文案仍写「停止自动重连」**（`auto_auth.rs:297` 日志文案），但 `reconnect_attempt_allowed`（`:32-35`）已把超限改为每 10 个计数放行一次重连——文案与行为不一致，排障时不要被日志误导。
