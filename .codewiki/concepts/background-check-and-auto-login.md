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
  - tauri-app/src-tauri/src/monitor/scheduled.rs
  - tauri-app/src-tauri/src/monitor/campus_check.rs
  - tauri-app/src-tauri/src/monitor/latency.rs
  - tauri-app/src-tauri/src/monitor/portal_check.rs
  - tauri-app/src-tauri/src/auth/failure_tracker.rs
  - tauri-app/src-tauri/src/infra/lifecycle.rs
  - tauri-app/src-tauri/src/infra/notification.rs
  - tauri-app/src-tauri/src/infra/state/mod.rs
  - tauri-app/src-tauri/src/commands/background.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/validate.rs
  - tauri-app/src-tauri/src/config/outbound_switch.rs
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/quality_cmds.rs
tags: [概念, 后台巡检, 自动登录, 断线重连, 生命周期, 通知]
---

## Overview

后台巡检是一条"启动装配 → 周期 tick → 出站切换态闸门 → 校园网判定 → Portal 探测 → 状态机 → 自动登录 / 断线重连 → 生命周期倒计时 / 通知"的长链路：桌面实现在 `monitor/` 模块群（`watcher` 门面 + `background_check` 单拍主体 + `scheduled` 定时动作循环），安卓实现在单个 `monitor_loop.rs`（1607 行）。两端共享同一套阈值语义（预备登录失败 5 次熔断、断线重连上限默认 3、冷却 60s、注销保护期 60s），后台检测间隔默认值两端已对齐为 60000ms（桌面随 v3→v4 迁移 15s→60s，`config/model.rs:258`；安卓 `config_state.rs:138`），但间隔下限、检测时段门控与省电分档策略不同。定时动作循环（`scheduled.rs`，1905 行）承载定时登录/注销、运营商夜切与完整的**夜间出站切换**编排（切换/还原/重放/看门狗/手动/启动对账），安卓在 `monitor_loop.rs` 内有同构实现（注销 → 程序化 `network_avoid_bad_wifi` + 触发系统 WiFi 重检切蜂窝 → 晨间登录还原）。

## 机制说明

### 完整调用链

```text
【桌面 tauri-app】
app/startup.rs:229 run_startup_tasks（setup_app 内直挂）
 └ monitor/watcher.rs:15-59 按 config 分四路 spawn：
    ├ :20-30  "startup_bg_check"     → background_task.rs:7-64 start_background_check_inner
    │           （写 enable_background_check=true 并落盘 :8-13，spawn 循环 :16-55）
    │           background_task.rs:29-53 周期循环：每 tick 重读间隔 :30-38，
    │           interval_timer+select(cancel) :39-47，is_quitting/任务被取消则退出 :48-51
    │           → 单拍主体 background_check.rs:370-378 run_background_check（spawn_blocking 包装）
    │             → background_check.rs:15-365 run_background_check_blocking
    │                → background_emit.rs:109-164 emit_background_check_result
    │                → background_emit.rs:166-202 update_network_state
    │                → auto_auth.rs:29-107 try_auto_login_on_preparation
    │                → auto_auth.rs:110-218 try_disconnect_reconnect
    ├ :32-42  "startup_latency"      → latency.rs:53-123 spawn_latency_test_loop
    ├ :44-49  "startup_auto_login"   → auto_auth.rs:220-469 run_auto_login_on_start
    └ :51-58  "scheduled_actions"    → scheduled.rs:116-233 run_scheduled_action_loop（30s 一拍）

【安卓 android】
lib.rs:44-55 setup 内调 monitor_loop.rs:419-466 run_startup_tasks
 └（睡 500ms :421 后按持久化配置并行 spawn）
    ├ enable_background_check → start_background_check :193-228（FGS start_monitor 成功才置 running :211-217）
    │    └ monitor_loop.rs:580-615 monitor_tick_loop
    │         ├ :593 run_scheduled_actions（:748-950，先于分档与静默期执行）
    │         ├ :596-601 间隔热更新（desired_interval_ms 变化重建计时器）
    │         └ :604-611 巡检分档（亮屏+WiFi 走基础间隔，其余走闲时间隔跳拍）
    │              → 单拍主体 run_check_once :1245-1471
    ├ enable_network_quality → quality_cmds.rs:52-67 start_latency_test（关则单次 check_network_quality）
    ├ update_cmds::start_update_check_loop :455
    └ auto_login_on_start → auto_login_on_start :506-549（探测失败 3s 重试一次 :472-490）
WiFi 变化事件（mobile）：monitor_loop.rs:244-329 start_wifi_watcher/handle_wifi_event，
  去抖 1000ms + 连上延迟 2500ms/断开 0ms，风暴内过期自杀，即时触发一次 run_check_once
手动检测：commands/background.rs:27-41（桌面）/ monitor_loop.rs:331-335（安卓）
```

### 周期常量与阈值（具体值）

| 项目 | 桌面 | 安卓 |
| --- | --- | --- |
| 检测间隔默认 | 60000ms（config/model.rs:258） | 60000ms（config_state.rs:138） |
| 检测间隔下限 | 10000ms（background_task.rs:30-38 每拍钳制；命令启动路径 <10000 拨正 15000，background_task.rs:8-13） | 5000ms（monitor_loop.rs:196、:582） |
| 闲时巡检间隔 | 轻量化模式下限 300000ms（lightweight.rs:78-80，纯函数 max 语义） | background_check_idle_interval 默认 300000ms（config_state.rs:140；分档 monitor_loop.rs:83-90、:604-611） |
| 质量检测间隔默认 | 600000ms（model.rs:280） | 600000ms（config_state.rs:153） |
| 质量间隔上限 | 轻量化下限 1800000ms（lightweight.rs:83-85） | good 连稳 5 拍翻倍、封顶 1800000ms（quality_cmds.rs:74-85） |
| 定时动作节拍 | 独立循环 30000ms（scheduled.rs:49） | 无独立循环，借检测拍且先于静默期（monitor_loop.rs:593） |
| 检测静默窗默认 | 460-1380 分钟（model.rs:295-296） | 460-1380 分钟（config_state.rs:168-171） |
| 校园网退出生效窗默认 | 480-1380 分钟（model.rs:293-294） | —（无此链路） |
| 注销保护期 | 60s（scheduled.rs:1526-1532 置值；auto_auth.rs:57-61、:130-134 消费） | 60s（protocol_cmds::do_logout 置值；monitor_loop.rs:1372 消费，:1354-1358、:1000-1001、:533-534 清零） |
| 预备登录失败上限 | 5 次，达上限本会话停用（auto_auth.rs:19、:48-55） | 连续失败熔断 5 次（monitor_loop.rs:1373、:1407/:1414） |
| 掉线重连上限 | max_disconnect_reconnect 默认 3（model.rs:302；计数 auto_auth.rs:152-155） | 同名字段（monitor_loop.rs:1376-1385），达上限即时通知（:1390-1392） |
| 自动重连提醒 | 每 10 次提醒一次（auto_auth.rs:14、:196-210） | 达上限单次通知（monitor_loop.rs:1390-1392） |
| 注销退出/最小化/自动退出 | 60000/30000/20000ms（lifecycle.rs:12-13、state/mod.rs:13） | — |
| shutdown 上限 | timeout 10s 后强制 exit(0)（lifecycle.rs:311-320） | 前台服务常驻，ExitRequested 一律 prevent_exit（lib.rs:114-121） |
| 夜切验证时序 | 等 15s、重试 5s×3、HTTP 15s（scheduled.rs:53-56） | 同值（monitor_loop.rs:1109-1112） |
| 出站还原失败上限 | 连续 3 次告警、40 次放弃收尾（scheduled.rs:75、:81） | 连续 3 拍放弃自动还原并清标记（monitor_loop.rs:19、:819-829） |

### 各环节行为细节

**闸门顺序（每拍先于一切探测）**：桌面单拍入口 `run_background_check_blocking`（background_check.rs:15）先做 is_quitting/cancel 早退（:16-18）与单飞保护 `is_checking.try_acquire`（:19-21），随后按序过两道闸：①出站切换态闸 `outbound_restore_active`（background_check.rs:50-57，非空快照即切换态，直接 return，见 [[decisions/night-outbound-switch]]）；②检测时段静默闸（:58-82）。安卓 `run_check_once`（monitor_loop.rs:1245）同样先查出站切换态整轮跳过（:1260-1263，注释明确周期拍/WiFi 事件/手动检测三路径共用此闸 :1255-1259），再查静默期（:1269-1289）；且 `run_scheduled_actions` 刻意不经过此闸（晨间还原须在切换态下照常执行）。

**检测时段静默**：桌面纯函数 `is_campus_check_silent`（campus_check.rs:272-280）——start=0 视为禁用（恒不静默）、now<start 静默、`end>start && now>=end` 静默，即窗口 [start,end)，end<=start 退化为单边；静默拍伪造 `CampusCheckResult{on_campus:true}` 并 `cancel_campus_exit`（background_check.rs:75）。安卓内联同退化规则（monitor_loop.rs:1269-1274），静默拍把常驻通知置为状态码 3「已暂停检测(非检测时段)」（仅翻转时重建 :1281-1285）。

**校园网判定**：桌面 `check_campus_network`（campus_check.rs:43-266）三级判据——SSID/有线 profile 名称匹配优先（:100-107、:182-189），/18 子网 `is_same_subnet_18` 次之（:112-122、:144-156、:193-203），网关可达兜底（闭包带缓存 :79-95），且仅当该类网卡有 IP 才归因（:159-165、:206-212）；名称名单支持多分隔符（:35-41）。判定失败且开了名称检查时置 `campus_check_failed`（background_check.rs:90），网络态同步 `any_adapter_online=false、last_a1_online=false、has_logged_online=false`（:91-103），并进入校园网退出分支：emit 结果、无配置 IP（`a1.is_none()&&a2.is_none()` :138）则跳过退出，否则 `start_campus_exit`（:143）。安卓判定在 `campus_detect::probe_campus_with_ssid`（monitor_loop.rs:1307，2026-09-20 SSID 感知），失败仅记日志退出本轮。

**Portal 探测**：桌面 `check_adapter_portal`（portal_check.rs:59-93，request_failed 区分 :65-72）；双卡且两卡均有 IP 时 `block_on` 内两个 spawn_blocking 并行 `join!`（background_check.rs:163-175），否则顺序/跳过（:177-190）；任一卡请求级失败走 `handle_portal_request_failure`（:194-220 → failure_tracker.rs:204-281），探测成功则重置对应卡认证失败计数（:222-247）。安卓 Portal 探测跑在绑小核的专用短命线程（monitor_loop.rs:697-734，先取 reactor Handle 线程内 enter 防裸线程 panic，catch_unwind 兜底，线程创建失败降级共享阻塞池），且有两级短路：非校园网或 WiFi 未连接直接给出原因不再空转（:1326-1330，2026-10-02 离网拍 11s→3s）。

**状态机与在线判定**：桌面 `online=a1_has_ip&&primary_online`（background_check.rs:260），双卡聚合 `any_online=online||secondary_online==Some(true)`（:310）；运营商徽标从 Portal uid 推导后缀（主卡 :262-266，副卡聚合 :277-293）。`handle_status_change`（background_emit.rs:68-107）只在线→离线翻转且允许通知时弹通知，60s 节流（:92-103）。`emit_background_check_result`（:109-164）内 checkCount 用 CAS 自增（:116-119），注销保护期内强制 `online=false、secondaryOnline=Some(false)` 且运营商置 null（:127-134、:146-147）；`update_network_state`（:166-202）保护期跳过写回（:177-183），在线时清 `disconnect_reconnect_count`（:185-191），`reachable&&!has_logged_online&&online` 时置 has_logged_online 并可触发 auto_exit（:193-201）。安卓为三态消费：仅 Portal `error_kind=None` 的确定判定才翻转在线，Unknown/Failed 保持上一拍记忆；非校园网一律判离线（monitor_loop.rs:1341-1351，防 2026-09-13「断网仍显示在线」）。在线拍清重连计数/熔断/保护期（:1354-1358），`was_online` swap（:1359）翻转且在校园网时弹掉线通知（:1361-1363）。

**自动登录与断线重连**：桌面三条路——预备登录 `try_auto_login_on_preparation`（auto_auth.rs:29-107）、掉线重连 `try_disconnect_reconnect`（:110-218）、启动登录 `run_auto_login_on_start`（:220-469）。共同纪律：注销保护期（:57-61、:130-134）与冷却（:63-67、:136-139）判定都在拿登录锁（is_logging_in.try_acquire，:70、:142-148、:419-432）之前；重连计数用 CAS 自增（:152-155），`within_limit=count<=max`（:156）；成功走 `append_login_history(...,"reconnect")` 并上报 `reconnected=true`（:180-195、:217，判据 `reconnect_should_report` :25-27）；超限先释放登录锁再按 `RECONNECT_REMINDER_INTERVAL=10` 提醒（:196-210）。安卓在 `run_check_once` 内联同构逻辑（monitor_loop.rs:1369-1420）：两道闸为保护期（:1372）+ 连续失败熔断 5 次（:1373），判定复用纯函数 `should_attempt_login`（:94-115，掉线重连不受自动登录开关管 :108-110），登录成功清保护期/失败计入熔断，历史落 login_history "auto"（:1399-1401）。

**通知出口**：桌面 `emit_notification`（notification.rs:16-65）——enable_notification 门（:17-23）、主窗口可见且未最小化则不弹（:29-34）、Windows 走系统 toast 失败降级插件（:45-55）、文案中文硬编码（:14-15 注释说明理由）。安卓系统通知 `notify_system`（monitor_loop.rs:161-178，mascot 大图失败降级纯文本），探测/登录失败统一追加 VPN 接管提示 `with_vpn_hint`（:184-190）；常驻 FGS 通知仅在线状态翻转时重建（:1461-1470），状态码表 `notify_state`：1=在线、2=未连接、3=非检测时段、4=WiFi 未连接（:1475-1483，2026-10-02 新增 4）。

**生命周期倒计时（桌面专属）**：`start_campus_exit`（lifecycle.rs:23-135）——campus_exit_on_fail 与生效时段校验后，60s 倒计时（CAMPUS_EXIT_DELAY_MS，lifecycle.rs:13）内先 30s 隐藏窗口再 30s 校验 deadline 后 `shutdown_and_exit`；`start_auto_exit`（:183-264）20s 窗口（state/mod.rs:13）；两者均注册取消快捷键 `CommandOrControl+Shift+C`、取消入口 `cancel_campus_exit`（:138-154）与 `cancel_auto_exit_inner`（:266-290）；`shutdown_and_exit`（:311-320）限时 10s 收尾任务后 `exit(0)`。

**质量循环与告警**：桌面 `spawn_latency_test_loop`（latency.rs:53-123）每轮重读轻量化间隔（:61-82），就绪等待（选卡出 IP 且在线，2s 重试 :100-111）后执行质量检测并经 `classify_quality_change`（:17-37）分级——恶化到 bad 经 quality_scheduler 复核后弹「网络拥堵」，从 bad 恢复弹「网络恢复」（:43-51）。安卓 `latency_loop`（quality_cmds.rs:87-113）带 good 连稳 5 拍翻倍的退避（:79-85），结果落 quality_history（:39-43）。

**定时动作与夜间出站切换**：桌面 30s 循环（scheduled.rs:116-233）同拍顺序为「出站动作 → 运营商夜切 → 定时登录/注销」：`outbound_action_for`（:276-301）先看还原态（restore_active 且未开夜出站则 Restore 清理优先 :290-292）与手动冻结日（:297-299），Switch 走 `apply_outbound_switch`（:538-772，快照三字段**先落盘再执行** :654-664，禁用循环 :690-703+兜底路由 :706-733+路由级验证 :739-766），Restore 走 `apply_outbound_restore`（:969-1204，删路由 :1008-1043→写回跃点 :1046-1092→重启用校园卡 :1099-1170，损坏快照按 give-up 阀 40 次收尾 :1103-1104），None 且 needs_replay 则 `replay_outbound_switch` 三分支补做（:787-888）+ `watchdog_re_disable_campus`（:897-957）；运营商夜切被切换态门控（`gated_night_action` :1403-1417，定时注销不受门控），执行后独立任务验证 `verify_night_switch`（:1540-1577，chkstatus 核对 uid :1610-1634）；定时登录在切换态下跳过但不消耗当日标记（`evaluate_and_mark` :1376-1396，过点补触发），注销入口 `perform_logout` 统一置 60s 保护期并复位计数（:1505-1534）。安卓完全同构于 `run_scheduled_actions`（monitor_loop.rs:748-950）：出站判定先于运营商夜切（:753-758 硬约束注释），切换靠注销 + `network_avoid_bad_wifi` 程序化置 1 + `report_wifi_unusable` 触发系统切蜂窝（:780-799、:1016-1078），还原走 `night_switch_login` 成功才清标记、连续 3 拍放弃（:801-838）；出站动作后**重读**新鲜配置再判运营商夜切（:843 防旧快照复活标记）；切换态下定时登录跳过不耗标记（:903-911）。

**失败追踪与 MAC 重置（桌面专属）**：`update_auth_failure_count`（failure_tracker.rs:47-95）按认证失败码（["ac_auth_failed","1","4"] :9）CAS 计数，单卡/双卡/Portal 请求失败三条路（:112-179、:204-281）计数达 5 时执行 `dhcp_release_renew_single` 释放 MAC 并 DHCP 续租（:73-93、:154-178、:246-280）；网关不可达时不计数并清零（:213-226）。`reset_all`（:182-191）在手动注销时全清。

## 2026-09-20 增补：轻量化间隔延长与动态重读

- 桌面轻量化模式（lightweight.rs:11 全局标志）下，检测间隔下限抬到 300000ms、质量间隔下限抬到 1800000ms（lightweight.rs:78-85，纯函数 max 语义，互不覆盖更大值）。
- 间隔为动态值：桌面检测循环每 tick 重读 `effective_background_interval_ms`（background_task.rs:30-38），质量循环同款（latency.rs:61-82）；命令启动路径 <10000ms 拨正 15000ms 并落盘（background_task.rs:8-13）。
- 安卓等效实现为「闲时巡检分档」：`effective_interval_ms`（monitor_loop.rs:83-90）亮屏+WiFi 走基础间隔，蜂窝/灭屏走 idle 间隔（默认 300000ms，config_state.rs:140），由 power_state 每拍查询（:604-611）；质量侧改为 good 连稳翻倍退避（quality_cmds.rs:79-85）。
- 后续演进（2026-10-02）：安卓 Portal 探测两级短路（离网拍 11s→3s，monitor_loop.rs:1326-1330）与常驻通知状态码 4（WiFi 未连接，:1475-1483）。

## 关键约束

- 单飞保护：桌面单拍入口 `is_checking.try_acquire`（background_check.rs:19-21），手动触发复用同一锁（commands/background.rs:27-41）；安卓靠循环任务唯一性（start 幂等只刷间隔，monitor_loop.rs:201-204）。
- 登录互斥：`is_logging_in` 锁在所有自动登录路径生效（auto_auth.rs:70、:142-148、:419-432；scheduled.rs:1441-1448）。
- 冷却与注销保护期判定必须先于获取登录锁（auto_auth.rs:57-67 → :70），否则保护期会被锁等待绕过。
- 重连计数用 CAS（`update_with_result`，auto_auth.rs:152-155），避免与后台检测/前端并发丢失自增。
- campus fail 必须同步重置 `has_logged_online=false`（background_check.rs:101），否则退出流程与预备登录判定会引用过期在线记忆。
- 只有重连成功才上报 `reconnected=true`（auto_auth.rs:25-27、:217），失败静默等待下一拍。
- 间隔是动态值：每 tick 重读而非缓存（background_task.rs:30-38、latency.rs:61-82、monitor_loop.rs:596-611）。
- 出站切换态是最高优先级闸门：单拍跳过（background_check.rs:50-57、monitor_loop.rs:1260-1263），且同拍动作顺序硬约束为「出站先于运营商夜切」（scheduled.rs:1-21 模块注释、monitor_loop.rs:753-758）。
- 出站快照先落盘再执行（scheduled.rs:654-664），安卓切换标记同样"落盘成功才触发系统重检"（monitor_loop.rs:787-788）；还原侧"重读新鲜配置"防旧快照复活已清字段（scheduled.rs:212-221、monitor_loop.rs:843）。
- 通知重复抑制三件套：掉线通知 60s 节流（background_emit.rs:92-103）、出站故障每日一次去重（scheduled.rs:91-97）、安卓常驻通知仅状态码翻转时重建（monitor_loop.rs:1461-1470）。
- 桌面 shutdown 限时 10s 强制退出（lifecycle.rs:311-320）；安卓前台服务常驻、ExitRequested 一律 prevent_exit（lib.rs:114-121）。
- 通知文案中文硬编码为有意决策（notification.rs:14-15）。

## Data Flow

```text
启动装配     桌面 startup.rs:229 → watcher.rs:15-59 四路 spawn
            安卓 lib.rs:44-55 → monitor_loop.rs:419-466（500ms 后并行 spawn）
     │
周期驱动     桌面 background_task.rs:29-53（interval+select cancel）
            安卓 monitor_loop.rs:580-615（定时动作先行 + 分档跳拍 + 间隔热更新
            + WiFi 变化事件即时触发 :244-329）
     │
单拍入口     run_background_check（background_check.rs:370-378, spawn_blocking）
            run_check_once（monitor_loop.rs:1245-1471）
     │
闸门        出站切换态：background_check.rs:50-57 / monitor_loop.rs:1260-1263
            检测静默期：background_check.rs:58-82（伪 on_campus=true）
                       monitor_loop.rs:1269-1289（常驻通知码 3）
     │
校园网判定   campus_check.rs:43-266（名称>/18 子网/网关，IP 归因门槛）
            campus_detect::probe_campus_with_ssid（SSID 感知）
     │
Portal 探测  portal_check.rs:59-93；双卡并行（background_check.rs:163-175）
            小核线程 + 短路（monitor_loop.rs:697-734、:1326-1330）
     │
状态机       background_check.rs:260-316 + background_emit.rs:109-164
            （注销保护期强制离线 :127-134）
            monitor_loop.rs:1341-1363（三态消费 + was_online swap）
     │
自动登录     auto_auth.rs:29-107 / :110-218（CAS 计数+冷却+保护期先行）
            monitor_loop.rs:1369-1420（熔断 5 次+冷却+上限通知）
     │
出口        emit "background-check-result"（桌面 23 字段 background_emit.rs:136-161 /
            安卓适用子集 monitor_loop.rs:1432-1452）→ 系统通知 / 常驻通知 /
            "login-log" / login_history / update_network_state
```

### 安卓侧对应实现的差异（`android/src-tauri/src/monitor_loop.rs`）

| 环节 | 桌面 | 安卓 |
| --- | --- | --- |
| 启动装配 | startup.rs:229 直挂 watcher | lib.rs:44-55 → run_startup_tasks（monitor_loop.rs:419-466） |
| 循环宿主 | background_task.rs 独立任务 | monitor_tick_loop（monitor_loop.rs:580-615） |
| 间隔下限 | 10000ms | 5000ms（monitor_loop.rs:196、:582） |
| 闲时分档 | 全局轻量化开关（lightweight.rs:27-29） | 每拍查询 power_state（:604-611） |
| 事件触发 | 无 WiFi 事件监听 | WiFi 事件即时触发检测（:244-329） |
| 校园网判定 | campus_check.rs 三级判据 | campus_detect 探针 + SSID 感知（:1307） |
| Portal 探测 | spawn_blocking，双卡并行（:163-175） | 绑小核裸线程单卡 + 两级短路（:697-734、:1326-1330） |
| 在线判定 | 按卡 online/secondary 聚合（:310） | 三态消费 error_kind（:1341-1351） |
| 掉线重连 | try_disconnect_reconnect（10 参数） | should_attempt_login 纯函数 + 熔断（:94-115、:1369-1420） |
| 失败追踪 | failure_tracker.rs，≥5 次 MAC 重置 | 无 MAC 重置链路，仅熔断（:1373、:1407/:1414） |
| 结果事件 | 23 字段含每卡明细（background_emit.rs:14-34、:136-161） | 适用子集含 campusMessage/currentSsid/sourceIp（:1432-1452） |
| 状态命令 | get_background_status_value（commands/background.rs:46-113） | status_value（monitor_loop.rs:118-138，展平最近一拍关键字段） |
| 定时动作 | 独立 30s 循环（scheduled.rs:49） | 借检测拍、先于静默期（:593） |
| 夜间出站切换 | 改跃点/禁用网卡/兜底路由（helper 提权） | 注销 + avoid_bad_wifi + 系统重检切蜂窝（:780-799、:1016-1078） |
| 夜切验证 | scheduled.rs:1540-1634 | verify_night_switch（monitor_loop.rs:1135-1242，同时序参数） |
| 通知 | emit_notification（桌面 toast） | notify_system + FGS 常驻通知状态机（:161-178、:1461-1483） |
| 退出语义 | 60s/30s/20s 倒计时 + 10s shutdown（lifecycle.rs） | 前台服务常驻不退出（lib.rs:114-121） |

## Connections

- [[desktop-monitor]]：桌面后台检测/质量循环的宿主与任务管理。
- [[android-backend]]：安卓单文件监控循环与前台服务保活。
- [[desktop-auth]]：Portal 协议、full_login/full_logout 与 uid 解析。
- [[desktop-app-lifecycle]]：托盘/退出守卫/轻量化模式与倒计时退出。
- [[config-and-persistence]]：配置模型、迁移链与加密落盘（两端字段对齐）。
- [[security-model]]：凭据加密、portal 白名单校验与提权 helper。
- [[ipc-command-surface]]：检测/状态命令面与事件契约。
- [[desktop-network-quality]]：质量检测分级、告警与 history。

## Known Issues

- 安卓无 MAC 重置链路：认证失败计数达 5 时桌面走 `dhcp_release_renew_single`（failure_tracker.rs:73-93、:154-178、:246-280），安卓只有 `consecutive_failures` 熔断（monitor_loop.rs:1373、:1407/:1414），凭据型失败无法自愈。
- 安卓重连上限两处重复判：纯函数 `should_attempt_login`（monitor_loop.rs:111-113）与通知侧 `count >= max_disconnect_reconnect`（:1390-1392）各判一次，语义靠约定保持一致。
- `CAMPUS_MINIMIZE_DELAY_MS=30000`、`CAMPUS_EXIT_DELAY_MS=60000`、`AUTO_EXIT_DELAY_MS=20000` 硬编码不可配（lifecycle.rs:12-13、state/mod.rs:13）。
- 更新检查循环装配位置两端不同：桌面 setup_app 直挂（startup.rs:227），安卓在 run_startup_tasks 内（monitor_loop.rs:455）。
- 检测静默期语义差异：桌面静默拍伪造 `on_campus=true` 并撤销校园网退出（background_check.rs:58-82），安卓整拍跳过仅置通知码 3（monitor_loop.rs:1269-1289）；在线记忆的保持方式不同。
- 状态命令不携带每卡校园网明细：桌面 payload 对应字段恒 Null（commands/background.rs:106-111），安卓 payload 无 adapterStatuses（monitor_loop.rs:1432-1452），前端逐卡明细已无后端来源。
- `try_disconnect_reconnect` 参数多达 10 个（auto_auth.rs:109-121），靠 `#[allow(clippy::too_many_arguments)]` 压制告警。
- 检测时段门控判定双端重复实现：桌面纯函数 `is_campus_check_silent`（campus_check.rs:272-280），安卓内联同规则（monitor_loop.rs:1269-1274），未像定时判定那样下沉共享 crate，改动需双端同步。
