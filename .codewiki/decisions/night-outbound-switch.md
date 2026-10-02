---
title: "夜间出站自动切换（出站层夜切）"
type: decision
source_files:
  - tauri-app/src-tauri/src/config/outbound_switch.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/night_switch.rs
  - tauri-app/src-tauri/src/monitor/outbound_switch.rs
  - tauri-app/src-tauri/src/monitor/scheduled.rs
  - tauri-app/src-tauri/src/monitor/background_check.rs
  - tauri-app/src-tauri/src/monitor/adapter_watch.rs
  - tauri-app/src-tauri/src/network/discovery/devnode.rs
  - tauri-app/src-tauri/src/network/discovery/windows.rs
  - tauri-app/src-tauri/src/platform/metric.rs
  - tauri-app/src-tauri/src/helper/mod.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
  - tauri-app/src-tauri/src/commands/network_cmd.rs
  - tauri-app/src-tauri/src/auth/protocol.rs
  - tauri-app/frontend/src/network/NetworkPanel.tsx
  - tauri-app/frontend/src/network/outboundOrder.ts
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/account_cmds.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/protocol_cmds.rs
  - android/src-tauri/src/lib.rs
  - android/src-tauri/gen/android/app/src/main/AndroidManifest.xml
  - android/plugins/network-bind/build.rs
  - android/plugins/network-bind/src/lib.rs
  - android/plugins/network-bind/android/src/main/java/com/campuslogin/plugin/networkbind/NetworkBindPlugin.kt
  - android/frontend/src/network/NetworkPanel.tsx
  - android/frontend/src/hooks/tauriApi.ts
tags: [决策, 夜间出站, 出站切换, metric, 夜切, 双端同构]
---

## 背景

校园网（无锡学院）的断网是**校园网整体不可用**（周日至周四 23:00、周五六 23:30 之后），不是某条运营商线路的问题——[[night-operator-switch|自动切换运营商]]工作在登录凭据层，整体断网时切了运营商也登不上。本次在**出站层**新增自动切换（2026-09-21 设计、2026-09-22 落地）：

- **桌面**：到点把排序中的非校园网网卡（热点/物联网卡/随身 WiFi）的接口跃点（metric）调至 1，使其成为系统默认出站；次日恢复窗口按快照还原。六期起升级为「禁用校园网网卡 + 兜底路由 + 保留 metric」三件套（见六期节）。
- **安卓**：到点注销校园网认证并触发系统 WiFi 连通性重检，让系统 `network_avoid_bad_wifi` 机制自动把默认网络切到蜂窝；恢复窗口重登当前账号。
- **改名**：原「晚间断网自动切换」更名为「自动切换运营商」（UI 文案与 wiki 标题，配置字段名 `nightOperatorSwitch` 不变），与新功能在名字上区分。

时间表与运营商夜切**共用**（`config/night_switch.rs`：`switch_time_for`（`:26-32`）0..=4→1380、5|6→1410；恢复窗口常量 `RESTORE_START_MINUTES=450`（`:10`，07:30 起）、`RESTORE_END_MINUTES=1380`（`:12`）。恢复窗起点 2026-10-02（839d3a0）由 06:30 调整为 07:30，禁两处硬编码漂移）。

## 与运营商夜切的动作顺序（硬约束）

两者判定在同一循环相邻执行，动作语义正交，但顺序固定（spec §2）：

- **切换侧**：先评出站切换——切入切换态成功则运营商夜切**跳过**（从热点出站登录校园 portal 必然失败，切了白切）；出站没切上（无可用候选）则运营商夜切照常。
- **恢复侧**：先还原出站——还原成功才允许运营商夜切恢复执行；还原未完成（重试中）则同拍跳过运营商恢复。
- **定时登录**：切换态期间跳过，且**不消耗当日标记**（桌面 `gated_night_action`（`scheduled.rs:1403`）返回 false、安卓短路在 `run_scheduled_actions` 的定时登录判定（`monitor_loop.rs:903` `!outbound_active`））——"过点补触发"语义要求还原成功后的下一拍仍能补登。

互斥不靠提前 return：桌面与安卓都以"动作后重读切换态"门控后续夜切判定（桌面 `scheduled.rs` 循环内出站动作块 `:167` → 夜切判定块 `:212-231` 的 `outbound_active` 门控；安卓 `run_scheduled_actions` 内 `outbound_active` 局部变量 + 动作后 `latest_settings` 重读 `:843`），保证"没切上则夜切照常"与"切上则夜切让位"同时成立。

## 桌面机制

### 判定纯函数

`config/outbound_switch.rs::evaluate_night_outbound(enabled, weekday, now_minutes, restore_active)`（`:27`）：`!enabled` → None；切换态（restore 非空）且 now ∈ [450, 1380) → Restore；非切换态且 `now < 450`（凌晨）→ **Switch（凌晨补切**，`:37-39`；文档 `:22-24`：机器在切点前后睡眠/关机错过切换时，凌晨视为前一晚切换窗口的尾部——校园线路已断、热点可用，07:30 恢复窗自会收敛，切在 07:29 也在 1 分钟后还原，无害）；非切换态且过当日切换时刻 → Switch（过点补触发）；其余 None。**切换态由配置自身承载，纯函数天然幂等防重**（快照非空不再 Switch、空不再 Restore），与运营商夜切同构语义。桌面在 `scheduled.rs::outbound_action_for`（`:276-301`）之上多包两层兜底：①功能已关但快照残留 → 按 Restore 清理（`:290-292`，**不受手动保持标记冻结**——残留快照的出口，安卓同款逻辑内联在 `run_scheduled_actions:773-775`）；②`outbound_manual_hold_day` 匹配当日 → 冻结自动 Switch/Restore（`:297-299`，见九期节）。

### 目标卡选择与逐卡判定

`monitor/outbound_switch.rs`：`select_outbound_candidate`（`:61-85`）候选只来自用户排序列表 `outboundPriority`（白名单天然规避虚拟网卡），要求有 IP 且判定为**非校园网**，并新增两道过滤：**无网关卡跳过**（`:73-75`）与**自身网关不可达（死路）卡跳过**（`:76-82`，网关探不通的卡切过去也出不了网）。逐卡判定 `is_campus_adapter`（`:17-39`）与 `campus_check.rs` 同源：与 `campus_gateway` 同 /18 网段，或**绑该卡源 IP** 的网关可达（`check_gateway_reachable_from`，避免多卡归因错位；两发去抖，任一次可达即判校园 `:35-36`），六期后另增 **portal 域名绑源探测证据**（`:37-38`：`portal_probe_host` 解析出的私网 IPv4 从该卡路由可达即判校园）。`portal_probe_host(portal_url)`（`:44-56`）从 portal URL 提取私网 IPv4 作为探测目标，域名/公网/IPv6 返回空串。**SSID 不参与逐卡判定**——`netsh wlan` 只报当前连接的单个 SSID，多无线卡下归因不可靠。无可用候选不进退避，发**每日一次**提醒（`notify_outbound_issue_once_per_day`，`scheduled.rs:91`，三枚独立日戳原子量 `:111-113`；调用点 `apply_outbound_switch:575-587`），30s 循环天然支持热点半夜开启后自动补切。

### metric 读写底座

- **读**（免提权）：`platform/metric.rs::read_interface_metrics`（`:56-65`）走 Iphlpapi `GetIpInterfaceTable(AF_UNSPEC)` 运行时值，不读持久层注册表；`interface_rows_for_guid`（`:25`）读快照与 helper 写 metric 共用同一枚举（LUID→GUID 失败行跳过 `:42-44`）。
- **写**（提权）：helper op `set_metric`（`HelperOp::SetMetric`，`helper/mod.rs:99-104` 条目编码 `"{guid}:{family}:{automatic}:{metric}"`，GUID 不含冒号故编码安全），worker 内逐条取整行改副本（`UseAutomaticMetric`/`Metric`/`SitePrefixLength`，`run_set_metric:568-649`）再 `SetIpInterfaceEntry`。已知坑：**`SitePrefixLength` 必须置 0**（`:615-616`），非 0 直接 `ERROR_INVALID_PARAMETER` 且错误信息不指向具体字段（EasyTier/mullvad 同款实现佐证，细节见 [[set-ip-interface-entry-metric]]）；且 `Family`/`InterfaceLuid`/`InterfaceIndex` 等必须取接口当前值，构造残缺结构体同样参数校验失败。
- **持久层口径（2026-10 修正，`metric.rs:1-5` 模块注释为准）**：`SetIpInterfaceEntry` 写的是接口的**持久配置**（与 `netsh set interface` persistent 存储同层，**重启不还原**）——因此快照还原是必需步骤而非可选优化，崩溃残留由启动对账收敛（见「三重还原保障」）。`config/model.rs:72-74` 字段注释仍写旧口径「运行时修改重启即还原」，与 `metric.rs` 不一致，以 `metric.rs` 为准。
- 目标卡 IPv4+IPv6 两族设 `UseAutomaticMetric=false, Metric=1`（自动跃点通常 ≥25，差距大避免平手抖动）。

### 快照与切换态时序

切换前先读目标卡两族 metric → 快照 JSON（`[{guid, family, automatic, metric}]`）+ 禁用名单 + 兜底路由**三快照一次落盘**（`apply_outbound_switch:660` `save_config_to_disk_encrypted` + `:664` store，字段 `outboundMetricRestore`/`outboundDisabledAdapters`/`outboundStandbyRoute`）→ **落盘成功才认定切换态成立**（"落盘先行"语义，沿运营商夜切先例）→ 再执行 helper 动作。helper 失败**不清快照**：清了会让已改的跃点/已禁的卡无从还原——切换态保留 + 退避重试补写（`needs_replay`（`scheduled.rs:383`）：三快照任一非空**且**切换侧有失败历史才补，稳态夜间约 900 拍不重复提权）。切换态下不重选目标卡（保持既有目标，还原后再按新排序选）。提权封装 `run_helper_op`（`:419`，`HelperFailure{reason,channel}` `:406` 区分 channel 失败与业务失败；helper 超时 `OUTBOUND_HELPER_TIMEOUT_SECS=30`，`:59`）。

### 切换动作（`apply_outbound_switch`，`scheduled.rs:538-771`）

退避闸（`:539`）→ `get_adapters_cached` + details join 出候选（`:543-562`）→ `portal_probe_host(config.portal_url)`（`:564`）→ `select_outbound_candidate`（`:565-588`）→ `select_campus_to_disable`（`:614-622`，`bus_guard` 传 `unsafe_to_disable` 仅 Windows、非 Windows 恒 true `:610-613`）→ 空名单告警 others_present（`:627-640`）→ StandbyRoute 挂目标卡网关 if_index（`:642-653`）→ 三快照落盘（`:660/:664`）→ `write_outbound_metric`（`:684`，notify_suffix 参数拼「已临时禁用 N 张校园网网卡」）→ 逐卡 `disable_adapter`（`:690-704`）→ `route_add`（`:706-733`）→ **切换后路由级验证**（`:739-766`：`best_route_if_index_v4(dest=campus_gateway)` 读最优路由，最优 ifIndex 仍属校园卡集合 → 判失败走重放，防止"写成功但没接管"静默假成功）→ all_ok 汇总（`:767-771`）。

### replay 三分支幂等补做（`replay_outbound_switch`，`scheduled.rs:787-887`）

metric 补写（`:792-829`，只写快照有原值∧当前存在的族）/ 禁用补做（`:830-860`，present 集合过滤）/ 路由补加（`:861-882`）——提权 gate 开头一次，各分支独立判定、全部幂等（重复禁用无害、同条目重复添加无害）；全部分支无新增失败清切换失败计数（`:885-887`）。

### 夜间看门狗（`watchdog_re_disable_campus`，`scheduled.rs:897-957`）

k2.8 审计 P2-3 落地：守护窗 ∧ 切换态 ∧ 退避闸内，解析禁用名单，发现名单内卡状态非 Disabled（被用户/外部重新启用）→ 重发 `disable_adapter` + 通知一次。仅在 None 拍调用（循环 `:202-208`），不与切换/还原动作竞争。

### 还原动作（`apply_outbound_restore`，`scheduled.rs:969-1204`）

顺序固定（k2.8 P1-1/P1-2/P3-1 修订后口径）：

- **跃点快照损坏** → **按已还原继续但每天提醒一次**（`OUTBOUND_METRIC_SNAPSHOT_WARN_DAY`，`:977-984`），不再整体收尾清三快照——禁用名单/兜底路由的还原信息仍有效，必须继续走完；
- 三快照全空 → finish 收尾（`:987-997`）；restore_gate 退避（`:998-1001`）；
- ① `route_delete` 兜底路由（`:1005-1043`）——失败不阻塞本拍后续还原，但**阻止 finish 收尾**（k2.8 P1-2 推翻六期「失败不阻塞」决策：静默假成功比延迟重试危险；快照损坏跳过删除 `:1039-1043`）；② 跃点写回（`:1049-1063`，终态出口语义：目标卡不存在/跃点行消失=已还原）；③ 启用名单内校园卡（`:1105-1130`）：**名单 JSON 损坏 ≠ 空名单**——默认判失败保留状态重试+告警（防误启用名单外卡）；连续失败达 `OUTBOUND_RESTORE_GIVE_UP_FAILS=40`（`:81`）按空名单放行 `gave_up_released`（`:1103-1114`）——不止住的话标记永久压住巡检且每拍都发起注定失败的还原，放弃阀是最终逃生口；
- 收尾通知区分 `gave_up_released`（`:1177-1198`）；`finish_outbound_restore`（`:1211`，先清快照成功才清退避；并 `clear_outbound_manual_hold` `:1219`）；`clear_outbound_snapshot` `:1225`。

### 三重还原保障 + 终态出口

1. **07:30 恢复窗口主动还原**（`apply_outbound_restore`，见上）：按快照逐族写回原值（`automatic=true` 时显式恢复自动跃点），成功后清快照。**注意 metric 写入落在持久配置层、重启不还原**（`metric.rs:1-5`），本条是唯一可靠的主动收敛路径；
2. **启动对账**（`reconcile_outbound_on_startup`，`scheduled.rs:1313-1371`）：恢复窗内或功能已关 → 立即还原（`:1327-1331`）；跃点快照不可用 → 还原（`:1334-1341`）；目标卡不存在 → 还原（`:1344-1348`）；夜间窗口内用 `portal_probe_host + is_campus_adapter` 判 `back_on_campus`（`:1355-1367`），已回校园网 → 直接还原、放弃本夜切换，否则重放切换（`:1370`）；
3. **终态出口**：目标卡 GUID 查不到（拔出/禁用）或跃点行消失 → **视为已还原，清空快照**（无对象可写、系统重建协议栈按默认值即还原语义）；
4. **崩溃残留兜底**：持久层写入意味着崩溃/被杀后残留不会自愈，启动对账（上条）与次日恢复窗是仅有的收敛出口——这也是导入配置强制保留本机切换态（k2.8 P1-1）的根因：快照丢失=原始跃点永久丢失。

### 手动立即切换/还原与当日保持标记（九期，2026-09-29—10-02 迭代）

- **命令**（`commands/network_cmd.rs`，注册 `app/startup.rs:85-87`）：`outbound_switch_now`（`:50` → `scheduled.rs::manual_outbound_switch`）、`outbound_restore_now`（`:62` → `manual_outbound_restore`）、`get_current_outbound_name`（`:75-85`，`platform::best_route::current_outbound_v4` 读最优路由接口名，失败 None 软降级）。
- **`manual_outbound_switch`**（`scheduled.rs:1271`）：立即切换——清退避 → **先落 hold 再 apply**（崩溃中途切换态可被清理分支接管）；已切换态拒绝。
- **`manual_outbound_restore`**（`:1293`）：立即还原——清 hold + 还原退避，走标准 `apply_outbound_restore`。
- **`outbound_manual_hold_day`**（`config/model.rs:93-94`，i32 `num_days_from_ce`，0=无）：冻结当日自动 Switch/Restore（`outbound_action_for:297-299`，含凌晨补切，测试 `:1893`）；**清理分支（功能关+残留）不受冻结**（`:290-292`）；次日 07:30 自动还原照常（hold_day 不再匹配）或手动还原清零（`finish_outbound_restore:1219`）。导入保留本机值、安卓不消费。`set_outbound_manual_hold:1249` / `clear_outbound_manual_hold:1263`。
- **前端**（`tauri-app/frontend/src/network/NetworkPanel.tsx`）：合并卡头部出站徽标行「立即切换」「立即还原」按钮（`:547`/`:558`，Zap/RotateCcw 图标，busy 互斥 + `!enableNightOutboundSwitch` 禁用；handler `:447-468`/`:470-491`）；当前生效出站名 `get_current_outbound_name` 60s 轮询（`:268-290`），徽标 `currentOutboundBadge` 常显在排第一目标卡行（`:166-171`）。

### 退避与提权降级

- 失败退避 `outbound_backoff_ms`（`scheduled.rs:317`）：0 → 首试、60s → 120s → 240s → 300s 封顶（`OUTBOUND_BACKOFF_START_MS=60_000`/`MAX=300_000`，`:67-70`；切换/还原独立计数 `:100-113`，成功清零）；`backoff_gate` `:328`。
- 告警周期化（k2.8 P3-1）：`outbound_restore_alert_due`（`:84`，`OUTBOUND_RESTORE_ALERT_FAILS=3 :75` 首达触发、此后每 20 次（`REMIND_EVERY=20 :77`，300s 封顶约每 100 分钟）重复），三处告警点（route/metric/enable）统一。
- UAC 降级照 adapter_watch 先例：首试静默（CMSTPLUA 通道零打扰），仅**确因提权通道失败**（没拿到 helper 结果，`HelperFailure.channel` 判定）才允许弹 UAC；helper 正常跑完但逐条失败属业务失败，弹 UAC 无解、不抬高放行计数。
- 还原连续失败达 3 次 → 告警通知"请手动恢复跃点"并**保留快照**（用户可见可干预），配合 `GIVE_UP_FAILS=40` 放弃阀不无限刷。

### 冲突协调（桌面）

- **巡检整轮跳过**（`background_check.rs:48-57`）：切换态期间 `run_background_check_blocking` 直接 return——portal 检测走热点出站必失败，15s 一拍会累计触发 MAC 重置提权 + 整夜告警。30s 后自然恢复。判定用 `outbound_restore_active` 三快照（`:50-54`）。
- **adapter_watch 双过滤**（`adapter_watch.rs`）：①禁用警告通知块（`:146-157`）——名单内卡不发 `adapter_disabled_warning`，名单解析失败→None→保守全拦；②自动启用闸门（`:168-214`）——`is_night_outbound_guard_window`（`:186-189`）∧ `outbound_restore_active`（`:190-194`）双条件内不出自动启用目标，名单过滤 `:210-212`，恢复登录触发 `:247-256`；自动启用自身退避 `:292-299` + allow_uac 降级 `:231-235`。`config/outbound_switch.rs::is_night_outbound_guard_window`（`:53-61`）：窗口=[当日切换时刻，次日 07:30 恢复窗开)，凌晨 `now < 450` 分支（`:57-59`）覆盖前一晚切换后全部时段。
- 切换态与账号无关（metric/禁用是系统态），`switch_account` 不清 `outboundMetricRestore`。
- **导入配置五切换态字段一律保留本机值**（`config_cmd.rs:181-189`，k2.8 P1-1 扩为五字段）：`outboundMetricRestore`/`outboundDisabledAdapters`/`outboundStandbyRoute`/`outboundManualHoldDay`/`nightOutboundRestore` 均不可迁移——切换态是本机系统状态，顶掉活跃快照=原始跃点永久丢失。`outboundPriority` 可迁移。
- DNS 有意不随 metric 走（多网卡并发解析下热点 DNS 通常胜出；一期观察，校园网卡禁用后竞争卡消失，见六期）。

## 安卓机制

### 动作序列（切换侧）

`monitor_loop.rs::run_scheduled_actions`（`:748` 起）出站分支：

1. **前置在线检查**：上一拍巡检判定不在线则跳过注销请求（`do_logout` 无在线前置，盲发必失败）并清失败计数（`logout_for_outbound_switch:638-669`，离线即"确认离线"`:645-651`）；
2. **注销**：在线则走 `do_logout`，"离线已生效"判据取 **`radiusOk || unbindOk`**（`auth/protocol.rs:379-380` 两个子步骤信号，注释 `:374-378`；MAC 解绑收尾 `:345-365`、`mac_unbind_request:408`）——`success` 只取 Radius 单边，但 MAC 解绑（ePortal 4.1.x 按 `wlan_user_ip` 踢）同样是破坏性踢下线，解绑成功即本机已离线；只看 `success` 会把 `(false, true)` 组合误判为注销失败，导致掉线重连→再注销的整夜摆动。注销结果不确认 → `switch_logout_unconfirmed`（`:671-691`）达 3 次按断网规律建态；
3. **落标记**：`nightOutboundRestore = "logged_out"`（`config_state.rs:48-52`；纯标记，不暂存账号名；恢复登**当前活跃账号**，无跨账号残留）落盘成功才触发重检；
4. **触发系统重检**：`trigger_wifi_recheck`（`:1016-1037`）经 network-bind 插件 `reportWifiUnusable`（Kotlin `NetworkBindPlugin.kt:586`）→ `ConnectivityManager.reportNetworkConnectivity(wifi, false)`（公开 API 无需权限）→ 系统 NetworkMonitor 重验证，`network_avoid_bad_wifi` 开启时系统自动把默认网络切到蜂窝。**应用侧不做任何 metric/开关写入**（改跃点属桌面能力，安卓无权限）。落标记后自动 `ensure_avoid_bad_wifi`（`:795` → 插件 `:1042-1078`）。

Switch/Restore 分支（`:780-799`/`:801-837`）：Switch = persist（`:787-788`）→ ensure → recheck（`:795-796`）；Restore = `night_switch_login`（`:984-1010`，`ensure_wifi_bound:990`，成功置 was_online `:1000`）成功 → 清标记 + `restore_avoid_bad_wifi`（`:804-813`），失败达 `OUTBOUND_RESTORE_MAX_FAILS=3`（`:19`，计数 `:17`）放弃清标记（`:819-829`）。动作后 `latest_settings` 重读防 stale 复活（`:626-636`/`:843`）。

### 判据与上限

- **注销失败上限 3**（`OUTBOUND_SWITCH_MAX_FAILS`，`monitor_loop.rs:24`，计数 `:22`）：注销请求在网络断开时本来就发不出去，无限重试无意义——达上限按"断网规律"照常建态，交系统侧重检验证接管。
- **还原失败上限 3**（`OUTBOUND_RESTORE_MAX_FAILS`，`:19`）：晨间 `night_switch_login` 连续 3 拍失败 → 放弃自动还原、清标记恢复巡检并发 error 日志要求手动登录——不止住的话标记会永久压住巡检且每拍都发起注定失败的登录。
- **重试只保留最后一次结果**（`do_logout_with_retry` 语义）：交替型失败（第 N 轮 unbind 成功→后续轮全败）最终 `unbindOk=false`，由 3 次上限兜底建态，不摆动。

### 防 stale 与全入口巡检闸

- **`latest_settings` 落盘前重读**（`monitor_loop.rs:626-636`）：切换/还原写盘若基于拍首快照克隆，会把期间已改的字段用旧值覆盖回去（典型：还原刚清掉的标记被同拍运营商夜切块复活写回）。出站动作后运营商夜切与定时登录也改用重读的新鲜配置。
- **巡检闸全入口**（`run_check_once` 开头，`monitor_loop.rs:1255-1263`）：闸放在 `run_check_once` 而非 tick 循环——周期拍、WiFi 变化事件（`handle_wifi_event`）、手动检测三条路径共用这一个闸（`night_outbound_restore` 非空 → 跳过整轮 `:1260-1262`）。**`run_scheduled_actions` 不经过此闸**（晨间还原判定必须在切换态下照常执行）。
- **`verify_night_switch` 双守卫**（`:1135-1189`）：运营商夜切验证的入口（`:1138`）与复验轮（`:1158`，"注销→重登" `:1174`）之前各查一次切换态——等待窗内新建的切换态会被复验轮把账号登回，打断出站等待窗口。NIGHT_VERIFY 常量 15s/5s/3 次/15s（`:1109-1112`）；chkstatus 解析复用 `config/night_switch.rs::parse_chkstatus`（`:91-104`，JSONP 剥壳）+ `uid_matches`（`:108-110`，trim+忽略 ASCII 大小写；`ChkStatusInfo{online,uid,oltime}` `:78-85`；调用点 `monitor_loop.rs:1233-1234`）。
- **切账号/删除当前账号清 `nightOutboundRestore`**（`account_cmds.rs:193`/`:297`，注释 `:191-192`/`:295-296`）：纯标记无恢复目标语义，但残留会让巡检永久跳过；两处清标记后均 `restore_avoid_bad_wifi` 收尾快照（`:204`/`:300`）。对齐 `night_operator_restore`（`:190`）的同位先例。
- **启动兜底**（`monitor_loop.rs:435-440`）：标记空但插件有待还原快照 → 启动即 `restore_avoid_bad_wifi`（覆盖崩溃/强杀错过恢复窗口）。

## 前端（双端）

- **桌面 NetworkPanel**（`tauri-app/frontend/src/network/NetworkPanel.tsx`）合并卡：开关（默认关——改系统路由属侵入性动作，`:569-572`）+ 网卡排序列表 + **排第一的卡常显「夜间出站目标」徽标**（`:647-648`，不受开关影响）+ 当前出站徽标（`currentOutboundBadge`，`:166-171`）+「立即切换/立即还原」按钮（见九期节）+ 管理员权限提示（`:686`）。排序落 `outboundPriority`：`buildOutboundOrder`（`outboundOrder.ts:9-20`）——priority 非空按其过滤保序、未列入的当前卡追加尾部；拖拽期间本地态渲染（`dragOrderRef:223`），`onDragEnd` 才一次性提交（`:254-258`）；长按起拖常量 `:54-56`，`SortableAdapterRow:78`。
- **安卓 NetworkPanel**（`android/frontend/src/network/NetworkPanel.tsx`）同名卡：开关（`:357-358`）+ `network_avoid_bad_wifi` 引导块状态感知（`:354-405`）——`getAvoidBadWifiStatus` 查询（`:170-183`，授权绿框/未授权 `pm grant` 一键复制 `:185-193`/原 `settings put` 命令降为备选）+「还原系统设置」按钮（`handleRestoreWritten:195-214`，成功/无操作/失败 toast）。类型与封装在 `hooks/tauriApi.ts`（`AvoidBadWifiStatus:150-158`、`RestoreWrittenSettingsResult:161`、invoke 绑定 `:324-325`）。
- i18n zh/en 双语言包双端同步；「晚间断网自动切换」文案改名「自动切换运营商」。

## 已知限制（有意不修/待办，记录避免重查）

1. **`auto_login_on_start` 未过出站闸**（双端同构限制，2026-10-03 复核桌面 `monitor/auto_auth.rs` 仍无 outbound 判据）：启动自动登录链路（桌面 `monitor/auto_auth.rs::run_auto_login_on_start`、安卓 `monitor_loop.rs::auto_login_on_start:506`）不判切换态——切换态内重启应用会重登校园网。桌面登录失败无害（启动对账会在夜间窗口重放切换）；安卓重登成功则流量回 WiFi、切换态名存实亡但标记仍在，07:30 恢复窗口会再登一次并清标记自愈。未修原因：启动登录是独立编排，加闸要动两端的启动路径，收益（用户恰在切换态内重启的低频场景）不抵评审面扩大。
2. ~~**`network_avoid_bad_wifi` 写通道不可用**~~（2026-09-22 已解决，见「WRITE_SECURE_SETTINGS 自动写增强」节）：一期只引导手动开启（adb/系统设置），且既有 `acceptWifiNetwork` 命令的 `applyNetworkSettingsCompat`（现 `NetworkBindPlugin.kt:284-294`）写 `captive_portal_mode=0` 与 `network_avoid_bad_wifi=0` 不回滚——两件事已随自动写增强一并落地（快照还原机制同时覆盖兜底路径的回滚出口）。
3. **桌面 `nightOutboundRestore` 字段是死字段**：为满足「字段集双端同构」纪律（[[config-field-sets-bidirectional-sync]]）而存在；桌面真实切换态字段是 `outboundMetricRestore` 三快照，桌面代码不消费 `nightOutboundRestore`（反之安卓不消费 `outboundMetricRestore`/`outboundPriority`/`outboundManualHoldDay`）。仅导入保留逻辑与切账号清理触达它。
4. **安卓同拍双登可能**：出站 Restore 成功当拍置 `outbound_active=false`，同拍随后的定时登录判定（`!outbound_active && should_fire_scheduled_action`，`monitor_loop.rs:903`）可能再登一次——恢复窗口恰好压着定时登录时刻时出现。后果轻（重复登录幂等），不为此引入拍内去重。桌面无此问题（定时登录判定在出站动作之前取拍首状态）。
5. **功能已关+态残留的兜底判据内联不重构**：桌面抽为可测纯函数 `outbound_action_for`（`scheduled.rs:276`，单测 `:1861-1904`），安卓在 `run_scheduled_actions` 内联同款三行（`monitor_loop.rs:773-775`）——刻意不抽公共纯函数（安卓编排本就独立于桌面 monitor 模块，抽取需扩大桌面 crate 的 pub 面，收益小）。两端行为必须保持同步，改动时人工对照。
6. **captive portal 白名单误判风险**：个别校园网部署放行系统探测 URL——安卓注销后系统验证仍"通过"，`avoid_bad_wifi` 不切流量。真机实测项；命中则改"注销+提示"降级。
7. **切换时机由系统探测周期决定**（安卓）：`reportNetworkConnectivity` 后系统按自身节奏重验证，不保证 23:00 整点切蜂窝，与"自动衔接"语义相容但非秒级。
8. ~~**与 TUN 代理（mihomo auto-route）并存时 metric=1 不接管整机流量**~~（2026-09-27 六期已解决，机制与并存语义见「禁用+兜底路由方案落地（六期）」节）：纯跃点切换确实被 TUN 压制，六期改用禁用校园网网卡产生接口/路由事件迫使 mihomo 重选出站。
9. **`config/model.rs:72-74` 与 `metric.rs:1-5` 注释口径冲突**：前者仍称「运行时修改重启即还原」，后者实证 `SetIpInterfaceEntry` 落持久层。文档以 `metric.rs` 为准，代码注释待顺手修（行为本身已由快照还原兜住）。

## WRITE_SECURE_SETTINGS 自动写增强（2026-09-22 二期）

一期已知限制②的落地（设计同日批准，方案：还原**原值**而非默认值，语义同桌面 metric 快照）：

- **manifest**：`AndroidManifest.xml` 声明 `WRITE_SECURE_SETTINGS`（`:20-21`，`tools:ignore="ProtectedPermissions"`）。仍受 signature|privileged 保护，需用户 adb `pm grant com.campuslogin.client android.permission.WRITE_SECURE_SETTINGS` **一次性授权**；授权后应用可静默写 `Settings.Global`，`network_avoid_bad_wifi=1` 免手动。
- **快照还原机制**（`NetworkBindPlugin.kt`）：`applyNetworkSettingsCompat`（`:284-294`）写两键（`captive_portal_mode:288`、`network_avoid_bad_wifi:291`）前先经 `snapshotGlobalKey`（`:322`）记原值到插件私有 SharedPreferences（`prev_` 前缀；已有快照不覆盖——首次写入为准；读取异常按 1 处理）。写通道注释 `:300-306`（WRITE_SECURE_SETTINGS 经 adb pm grant 获得两条路径共用）。这同时给出 `acceptWifiNetwork`（`:192`，三级：`setAcceptUnvalidated:254` → 探测 `:241` → Settings.Global 兜底）兜底路径的**回滚出口**。
- **插件命令 7 个**（`build.rs:1-9` COMMANDS 数组：bindToWifi/unbind/acceptWifiNetwork/ensureAvoidBadWifi/restoreAvoidBadWifi/restoreWrittenSettings/getSecureSettingsStatus；tauri_plugin 据此生成权限脚手架，见 [[plugin-permission-dangling-refs]]——手写 ACL 三件套会被构建再生覆盖，`default.toml` 才是权限集 source of truth）。Kotlin 实现：`ensureAvoidBadWifi:366-391`（幂等写 1+快照 `:390-391`）、`restoreAvoidBadWifi:409-418`（无快照 restored=false 幂等）、`restoreWrittenSettings:429-443`（两键全量还原）、`getSecureSettingsStatus:454-460`（granted/avoidBadWifi/pendingRestore 查询）、`restoreGlobalKey:347-352`、`hasWriteSecureSettings:334-336`、`hasPendingRestore:339`。Rust 封装 `network-bind/src/lib.rs`：`ensure_avoid_bad_wifi:82`、`restore_avoid_bad_wifi:89`、`restore_written_settings:96`、`get_secure_settings_status:104`。
- **Rust 编排四触发点**（`monitor_loop.rs`/`account_cmds.rs`）：①Switch 分支落标记成功后自动 ensure（`monitor_loop.rs:795`）；②Restore 清标记后自动还原（`:804-813`）；③启动兜底——标记空但有快照即还原（`:435-440`）；④切账号/删当前账号清标记后还原（`account_cmds.rs:204`/`:300`）。Tauri 命令 `protocol_cmds.rs`：`get_avoid_bad_wifi_status:215-230`、`restore_written_settings:235-250`（均 JNI 阻塞走 spawn_blocking），注册于 `lib.rs:61-62`。
- **前端**：见「前端（双端）」安卓条目。
- 桌面不涉及（Settings.Global 为安卓特有）。

## 已排除路线（避免重查）

`WRITE_SECURE_SETTINGS` 写 `Settings.Global.WIFI_ON`（AOSP 源码证伪：仅 @Readable，"Only the Wi-Fi service should touch this"）；Shizuku 提权 `svc wifi disable`（需第三方应用+重启重激活）；Device Owner 豁免（已有账号设备被拒）；VpnService tun 转发 / 无障碍模拟点击（过重/脆弱）。

## 桌面卡片合并与拖拽排序（2026-09-23 三期）

一期前端节里「不引入拖拽」的决策撤销（当时顾虑安卓 WebView 滚动冲突与新增依赖——实际仅桌面引入，且 framer-motion ^12.38.0 桌面 frontend 已有、`Reorder.Group/Item + useDragControls` 是其内置能力，零新依赖；安卓排序 UI 不存在，滚动冲突前提不成立）。用户需求：网络适配器卡与夜间出站切换卡合并为一张卡；↑/↓ 按钮改长按拖拽。**仅桌面**（安卓端 NetworkPanel 不动）。

- **合并卡结构**：`tauri-app/frontend/src/network/NetworkPanel.tsx`——头部左「网络适配器 + 检测数」、右 MoonStar 图标 + Switch（夜间出站切换总开关）；头部下方整行 nightOutboundSwitchDesc 小字；列表 = 适配器统一列表，每行图标（Wifi/Cable）+ 名称/IP(mono)/速度 + 徽标（主适配器/副适配器/无线/状态/夜间出站目标/当前出站）+ 行按钮（启用/获取新IP/刷新DHCP）。原「主适配器排最前」的自动排序取消——主/副由徽标表达，顺序完全由用户拖拽决定（`outboundPriority` 语义不变）。
- **顺序构建纯函数** `outboundOrder.ts::buildOutboundOrder(priority, detected)`（`:9-20`）：priority 非空 → 按 priority 过滤掉已拔出网卡后排序、detected 中新卡按发现顺序追加尾部；priority 空 → 直接用发现顺序。7 个 vitest 用例（空 priority/乱序/失效过滤/新卡追加/全失效/去重/纯函数不变参）。
- **拖拽交互**：行体任意处长按 250ms（`DRAG_LONG_PRESS_MS`）起拖，移动超 8px 死区（`DRAG_DEAD_ZONE_PX`）取消长按——保护滚动与点击（常量 `:54-56`）。`Reorder.Item` 挂 `dragListener={false}` + `dragControls`；行内按钮区包一层 `onPointerDown stopPropagation` 防点按钮误触长按。拖拽中用本地 state（`dragOrderRef:223`）渲染，`onDragEnd` 才一次性提交 `onUpdateConfig({outboundPriority})`（`:254-258`，拖拽期间外部顺序同步被屏蔽，异步回显不打断手势）；`whileDrag` 抬起态（scale 1.02 + 阴影）。（四期注：把手通道已撤销，仅保留长按通道，见下节。）
- **验证**：tsc 0 错误、vitest 96/96、vite build 通过；Playwright（仓库外临时环境 + 临时 dev mock shim，已删）真浏览器实测 4 场景全过。
- 顺序提交语义与夜间出站切换的候选选择（`select_outbound_candidate` 按列表顺序）天然衔接：拖到第一位的卡即夜间出站目标徽标行。

## 桌面排序仅保留长按拖拽（2026-09-23 四期）

三期双通道交互上线当日的用户反馈迭代：去掉「点击特定卡片（把手）进行排列」，只留长按条目任意位置起拖；功能描述改「切换为非校园网」，并要求提示用户通过拖动决定切换目标。仅桌面（安卓端无排序 UI，不涉及）。

- **把手通道撤销**（`tauri-app/frontend/src/network/NetworkPanel.tsx`）：删除行内 GripVertical 把手按钮（`pointerdown` 立即 `startDrag` 通道）——「点击特定卡片排列」的操作模型认知负担高，且把手挤占行首视觉空间。长按 250ms + 8px 死区 + 行内按钮区 `stopPropagation` 三件套原样保留（现常量 `:54-56`）；`whileDrag` 抬起态、本地 `dragOrder` 渲染、`onDragEnd` 一次性提交语义全部不变。
- **文案三层收敛**（zh/en 同步）：①`nightOutboundSwitchDesc`「…自动把出站切换为非校园网（热点/物联网卡）…」——从机制描述改为面向效果；②`outboundDragHint`「长按条目任意位置拖动调整顺序，排最前的非校园网网卡即夜间切换目标」（提示行 `:684`）；③`outboundAdminHint` 删去「排第一的网卡即夜间出站目标」（与常显徽标、新拖拽提示重复，`:686`）。
- **验证**：tsc 0 错误、vitest 96/96；Playwright 真浏览器实测 6 场景全过；实测教训：`Reorder.Item` 默认 `as="li"` 渲染（Group 才显式 `as="div"`），行选择器与等待信号（「夜间出站目标」徽标唯一，避免命中 Dock/快捷按钮同文案）据此选取。

## 文案精简与夜间断网实测复盘（2026-09-27 五期）

用户两项文案要求落地（`nightOutboundSwitchDesc` zh/en，现描述周日至周四 23:00、周五六 23:30 自动切换 + 次日自动还原——恢复时点文案随 2026-10-02 839d3a0 调整为 07:30 口径）。同日复盘 2026-09-26（周五）夜间实测「时间到了没切换、无法上网」，日志取证（安装目录 `logs/app-2026-09-26.log` / `app-2026-09-27.log`）：

- **切换动作本身成功**：23:30:19 评出目标卡 WLAN（热点 192.168.6.109，判非校园网正确），helper 两族 metric=1 写入成功、快照落盘、通知「已切换出站到 WLAN」。23:31:35 adapter_watch 自动 pnputil 启用两张被 PnP 禁用的有线卡（此冲突由六期名单过滤与七期守护窗修复）。
- **6:30 恢复窗口无进程执行**：09-27 日志首行 07:43:23「应用启动」，还原由启动对账补做（07:43:24 成功）。后台运行前提昨晚被违反（PC 关机或应用退出，原因未定位）。
- **流量未跟随的机制**（本机路由表实测 + 同类实现调研）：FlClash（mihomo 内核）TUN auto-route 以 `0.0.0.0/0 → 198.18.0.2` RouteMetric=0（Protocol=NetMgmt）持有默认路由，物理卡 metric=1 在「应用→出口」这一段永远赢不了它；跃点修改不产生路由增删事件，探测若非轮询式重算则感知不到，出站焊死在已断的校园网卡即整夜断网——与用户观察「连接还在其他网卡上」吻合。独立候选：热点无上游（手机休眠/欠费）、DNS 不随 metric 走。
- 用户原假设「适配器没被选择就不会被操作」对选择器成立（`select_outbound_candidate` 只操作列表内、有 IP、判非校园网的第一张卡），但本夜 WLAN 恰被选中并操作成功，断网另有原因。

二期候选方案落实情况见六期节末「五期候选落实情况」。

## 禁用+兜底路由方案落地（2026-09-27 六期）

用户 v2 提案（2026-09-27）否定纯跃点路线（原话大意：直接修改路由表新增一条排在 TUN 后面的路由，再增加操作把连接校园网的网卡禁用后再启用）。拍板方案：

- **方案**：「禁用校园网网卡 + 兜底路由 + 保留 metric」三件套（替换五期候选 1）；启用（还原）时机=恢复窗口（现为 07:30）。
- **硬约束①（2026-10-01 撤销）**：原约束「USB 网卡绝不禁用——禁用会导致 USB 网卡下次开机无法正常启用」**已按用户明确要求撤销**：USB 副卡参与夜间禁用，`unsafe_to_disable`（`monitor/outbound_switch.rs:170-173`）删除 USB 前缀豁免，仅 PnP 实例 ID 读不到才 true（注释 `:165-169`：2026-10-01 运行期对称性已实证——禁用态 USB 卡 enable 后可正常回来）。历史上该守卫经 `bus_guard` 闭包传入 `select_campus_to_disable`（`:178-211`），现为纯 PnP 可读性守卫。
- **硬约束②（仍有效）**：软件对网卡状态判断不准 → 禁用名单只记**亲手禁用的 GUID**（`DisabledRow{guid,name}`，`monitor/outbound_switch.rs:115-119`），绝不靠状态推断；状态误判已修（见下「幽灵设备误判修复」）。

### 切换动作要点

`select_campus_to_disable`（`:178-211`）：候选=Connected + 判校园网 + priority 白名单内 + 有 GUID/IP + **非目标卡**（排除 `exclude_guid`，目标卡禁了就没人出站）+ **同名去重**（`:189-191`）+ **bus_guard 命中跳过**（`:205-207`）。逐张 helper `disable_adapter`，**只记成功下发禁用的卡**；与 metric 快照、路由快照一次落盘（三快照成立切换态），失败不清快照退避重试（`needs_replay` 三快照+失败计数联合判定）。兜底路由 `StandbyRoute{dest,mask,gateway,metric,if_index}`（`:142-150`）→ `outboundStandbyRoute`：`0.0.0.0/0` 指向目标卡网关、metric=2（`OUTBOUND_STANDBY_ROUTE_METRIC`，`scheduled.rs:61-65`）、IF 目标卡；网关为空则不下发。切换后路由级验证见桌面机制节。

**为什么这套动作能接管流量**：FlClash TUN（0.0.0.0/0 metric 0）仍持有默认路由——兜底路由 metric=2 永远抢不了它，**唯一角色是 FlClash 退出后的 failover**。真正的主力是禁用：校园网卡禁用产生接口/路由删除事件，mihomo `auto-detect-interface` 重选出站接口——已禁的校园网卡从候选消失，TUN 出站落目标卡（热点）；路由/DNS/接口竞争同时全消。目标卡 metric=1 保留，是 mihomo 出站选择的依据（TUN 退出后默认路由由同卡的 metric 2 兜底路由顶上）。夜间并存语义：**FlClash 运行中**流量仍进 TUN、出口=热点（代理路径恰好可用）；**FlClash 退出**后默认路由=兜底路由（目标卡），直连热点。

### 幽灵设备误判修复（`network/discovery/windows.rs`）

USB 网卡拔出后设备节点成幽灵（phantom）：`AdminStatus` 或 Class 注册表 `ConfigFlags` 残留 DISABLED 位 → 误分类「已禁用」（自动启用逻辑对着不存在的卡提权——用户「USB 判断不准」反馈的根源）。`classify_adapter_status`（`:345`）加第 6 参 `pnp_present: Option<bool>`（计算点 `:212-217`）——`Some(false)`（经 `network/discovery/devnode.rs::read_pnp_instance_id`（`:66`，`pub(crate)`）+ `devnode_problem`（`:36`）判定）直接判 `Disconnected`；查询不可用（None）回退旧行为。**注意**：桌面 discovery 两文件已从 `src/discovery/` 迁至 `src/network/discovery/`。

### 配套状态

- 切换态判定三快照联合（`config/outbound_switch.rs::outbound_restore_active`（`:68-70`）任一非空）：`scheduled.rs` 循环/启动对账、`background_check.rs:48-57` 巡检跳过、`adapter_watch.rs` 闸门均改用；`config/model.rs` 字段 `outbound_disabled_adapters:81-82`/`outbound_standby_route:86-87`/`outbound_manual_hold_day:93-94`（默认空/0，双端同构；安卓不消费——`android/src-tauri/src/config_state.rs` 仅 `:35-52` 五字段）。

### 五期候选落实情况

- **候选 1（压制竞争接口抬 metric 5000）**→ 被禁用方案替代：禁用比抬跃点彻底（路由/DNS/竞争全消），且 mihomo 事件感知可靠。
- **候选 2（切换后验证+通知）**→ **已实施**（九期）：`apply_outbound_switch` 切换后路由级验证（`scheduled.rs:739-766`），未接管判失败走重放。
- **候选 3（DNS 跟随）**→ 以禁用方案间接消解：校园网卡禁用后 DNS 竞争卡消失，热点 DNS 天然胜出。
- **候选 4（合并卡显示生效出站）**→ **已实施**（九期）：`get_current_outbound_name` 60s 轮询 + 当前出站徽标。

## 七期：守护窗口闸门（2026-09-28）

**现象**（09-27 夜日志）：用户切换前手动禁用校园网卡（无 IP）→ 23:04:57 / 23:12:33（重启 replay 后）adapter_watch 仍自动启用，与切换意图打架；07:43、19:11 白天亦有同款启用。

**根因**：六期闸门只按 `outboundDisabledAdapters` 名单豁免；手动禁用的卡无 IP，`select_campus_to_disable` 无 IP 跳过不入名单 → 名单空 → 闸门失效。

**修复**：`config/outbound_switch.rs` 纯函数 `is_night_outbound_guard_window`（现 `:53-61`）——窗口=[当日切换时刻（周日~四 1380、周五六 1410）, 次日恢复窗开)；凌晨分支覆盖前一晚切换后全部时段（跨日天然成立）。`monitor/adapter_watch.rs` 自动启用块先算窗口，窗口内不出目标，名单闸门与白天「意外禁用→恢复登录」行为不变。

> 2026-10-02 839d3a0：恢复窗起点由 06:30（`RESTORE_START_MINUTES=390`、`now < 390`）调整为 07:30（450，`night_switch.rs:10`）；上文本节时间表述已按现值改写。

**设计取舍**：否决「把已禁用卡也记进名单」——名单驱动还原逐卡 enable，记进去会让还原误启用用户手动禁用的卡（制造新 bug）；纯时间窗闸门即可全覆盖三场景（重启 replay、启动中途、运行中禁用——都落窗口内）。八期 P3-2 起窗口再叠加切换态条件（见下）。

## 八期：k2.8 深度审计落地（2026-09-28）

kimi-k2.8-preview 只读审计 src-tauri 后端返回 7 条，逐条源码核实后处置：

**实施 4 条**：

- **P1-1 导入配置保留本机切换态**（`commands/config_cmd.rs:181-189`）：原「导入即清快照」在切换中导入会把活跃快照顶掉——系统残留 metric=1/禁用卡/兜底路由却无还原路径，原始跃点永久丢失（次夜把 metric=1 当原值快照）。改为导入文件切换态字段一律丢弃、**保留本机当前值**（优于审计建议的「拒绝导入」：导入任何时刻可用，外部状态不迁入、本机状态不丢失）。九期起扩为五字段（+`outboundManualHoldDay`）。
- **P1-2 兜底路由删除失败阻断收尾**（`monitor/scheduled.rs:1005-1043`）：route_delete 失败原仅 warn、照常 finish 清三快照 → 孤儿 0.0.0.0/0 metric=2 路由持续改道流量而用户看到「已还原」。改为失败 mark_outbound_failure（进退避+告警）+ 阻止 finish，下一拍整体幂等重试。**推翻六期「失败不阻塞还原」决策**：当时论据「运行时路由重启即清」只对短期成立，静默假成功比延迟重试危险；卡启用/跃点写回不受影响（本拍继续执行，仅收尾被阻）。
- **P3-1 还原失败告警周期化**（`scheduled.rs::outbound_restore_alert_due:84`）：原 `count == 3` 恰好一次后无限静默；改为 `count >= 3 && (count-3) % 20 == 0`（300s 封顶时约每 100 分钟），三处告警点（route/metric/enable）统一。
- **P3-2 守护窗叠加切换态**（`adapter_watch.rs:186-194`）：七期纯时间窗在整夜休眠错过切换的凌晨会误阻塞自动启用；改为 `is_night_outbound_guard_window ∧ outbound_restore_active(三快照)`。原 bug 两实证场景切换态均成立，守护不弱化；无快照即无切换可顶掉，恢复白天行为。

**延后/否决 3 条**：

- **P2-1 helper 请求目录 ACL 加固**（`platform/task_proxy.rs` create_dir_all 继承 ProgramData ACL，同用户进程可写请求文件被 SYSTEM worker 执行）：真实提权面，但修复需设计 requests/results 两目录的属主矩阵（app 用户建 requests、SYSTEM worker 建 results，DACL 收口后互相读写均需放行），且必须真机 icacls 验证——记录方案后延，不盲改提权链。
- **P2-2 夜间手动置空运营商被次晨还原覆盖**（`config/night_switch.rs:61-66` 恢复条件无法区分应用置空与用户置空）：修复需引入哨兵值改配置语义、兼容存量快照，语义风险高，记为已知限制待专门设计。
- **P3-3 UI enable_adapter 无切换态互斥**：**否决**——用户显式点击启用是用户意图，加互斥即「与用户意图打架」；状态不一致由用户自担，不加守护。
  - 注：P3-3 的「夜间被外部重新启用」场景后由九期**看门狗**（`watchdog_re_disable_campus`，`scheduled.rs:897-957`）接管——不动用户 UI 操作，只在切换态内发现名单卡被启用时重禁用+通知一次。

**验证**：cargo test lib 410/410；clippy 三改动文件零告警。

## 九期：手动立即切换/守卫强化与持久层口径修正（2026-09-29—10-02）

八期之后的连续迭代批次（各改动彼此独立落地，合并记录）：

- **立即切换/立即还原 + 当日保持标记**：见「桌面机制 → 手动立即切换/还原与当日保持标记」节。核心决策：hold **先于** apply 落盘（崩溃中途切换态可被清理分支接管）；清理分支（功能关+残留）不受 hold 冻结——保底出口永远在线。
- **凌晨补切**（`evaluate_night_outbound:37-39`）：`!restore_active && now < 450` → Switch——机器在切点前后睡眠/关机错过切换时，凌晨视为前一晚切换窗口的尾部；07:30 恢复窗自会收敛（切在 07:29 也在 1 分钟后还原，无害）。hold 冻结凌晨补切（测试 `scheduled.rs:1893`）。
- **切换后路由级验证**（`scheduled.rs:739-766`）：`route_add` 成功后 `best_route_if_index_v4` 读最优路由，最优 ifIndex 仍属校园卡集合 → 判失败走重放——把五期候选 2 的「静默假成功」告警升级为自动纠正。
- **看门狗**（`scheduled.rs:897-957`）：八期 P2-3 衍生，见八期节注。
- **portal 域名绑源探测**（`monitor/outbound_switch.rs:44-56` + `is_campus_adapter:37-38`）：portal 走域名时逐卡判定补一条「portal 私网 IP 从该卡可达」证据，与网关探测互为备份，降低多卡归因误判。`select_outbound_candidate` 同步过滤无网关与网关死路的卡（`:73-82`）——切到一张出不了网的卡等于没切。
- **USB 禁用豁免撤销**（2026-10-01）：见六期硬约束①修订。运行期对称性实证：禁用态 USB 卡 enable 后可正常回来，「下次开机无法启用」未复现；用户明确要求副卡（USB 2.5G）参与夜间禁用。
- **metric 持久层口径修正**（`platform/metric.rs:1-5`）：`SetIpInterfaceEntry` 写持久配置、重启不还原——「重启自动还原」的旧认知作废，快照还原是必需步骤。行为未变（本就按快照还原实现），修正的是文档与风险模型（崩溃残留不会自愈 → 启动对账/恢复窗是唯一收敛路径；导入保留本机切换态从「保险」升格为「必需」）。
- **还原放弃阀**（`OUTBOUND_RESTORE_GIVE_UP_FAILS=40`，`scheduled.rs:81`/`:1103-1114`）：禁用名单损坏等无法自明的失败连续 40 次后按空名单放行 `gave_up_released`——防止坏名单把还原永久锁死（逃生口语义，与安卓 MAX_FAILS=3 同源不同值：桌面单次还原动作内重试粒度更细）。
- **恢复窗 06:30→07:30**（839d3a0，`night_switch.rs:10`）：桌面恢复窗、守护窗终点、凌晨补切边界随常量自动联动。

## Connections

[[night-operator-switch]]、[[dual-platform-sharing]]、[[scheduled-actions-outside-silent-window]]、[[config-field-sets-bidirectional-sync]]、[[logout-radius-first]]、[[set-ip-interface-entry-metric]]、[[windows-task-proxy-elevation]]、[[outbound-switch]]、[[desktop-config]]
