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
  - tauri-app/src-tauri/src/discovery/devnode.rs
  - tauri-app/src-tauri/src/discovery/windows.rs
  - tauri-app/src-tauri/src/platform/metric.rs
  - tauri-app/src-tauri/src/helper/mod.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
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

- **桌面**：到点把排序中的非校园网网卡（热点/物联网卡/随身 WiFi）的接口跃点（metric）调至 1，使其成为系统默认出站；次日恢复窗口按快照还原。
- **安卓**：到点注销校园网认证并触发系统 WiFi 连通性重检，让系统 `network_avoid_bad_wifi` 机制自动把默认网络切到蜂窝；恢复窗口重登当前账号。
- **改名**：原「晚间断网自动切换」更名为「自动切换运营商」（UI 文案与 wiki 标题，配置字段名 `nightOperatorSwitch` 不变），与新功能在名字上区分。

时间表与运营商夜切**共用**（`config/night_switch.rs::switch_time_for`：0..=4→1380、5|6→1410；恢复窗口 `[390, 1380)`，2026-09-22 起提升为 `pub(crate)` 供出站侧复用，禁两处硬编码漂移）。

## 与运营商夜切的动作顺序（硬约束）

两者判定在同一循环相邻执行，动作语义正交，但顺序固定（spec §2）：

- **切换侧**：先评出站切换——切入切换态成功则运营商夜切**跳过**（从热点出站登录校园 portal 必然失败，切了白切）；出站没切上（无可用候选）则运营商夜切照常。
- **恢复侧**：先还原出站——还原成功才允许运营商夜切恢复执行；还原未完成（重试中）则同拍跳过运营商恢复。
- **定时登录**：切换态期间跳过，且**不消耗当日标记**（桌面 `gated_night_action` 返回 false、安卓短路在 `should_fire_scheduled_action` 之前）——"过点补触发"语义要求还原成功后的下一拍仍能补登。

互斥不靠提前 return：桌面与安卓都以"动作后重读切换态"门控后续夜切判定（桌面 `scheduled.rs:156-172`，安卓 `monitor_loop.rs` 的 `outbound_active` 局部变量），保证"没切上则夜切照常"与"切上则夜切让位"同时成立。

## 桌面机制

### 判定纯函数

`config/outbound_switch.rs::evaluate_night_outbound(enabled, weekday, now_minutes, restore_active)`（`:24`）：`!enabled` → None；切换态（restore 非空）且 now ∈ 恢复窗口 → Restore；非切换态且过当日切换时刻 → Switch（过点补触发）；其余 None。**切换态由配置自身承载，纯函数天然幂等防重**（快照非空不再 Switch、空不再 Restore），与运营商夜切同构语义。桌面在 `scheduled.rs::outbound_action_for`（`:214`）之上多包一层兜底：功能已关但快照残留 → 按 Restore 还原（残留快照的出口；安卓同款逻辑内联在 `run_scheduled_actions`）。

### 目标卡选择与逐卡判定

`monitor/outbound_switch.rs`（`:34` `select_outbound_candidate`）：候选只来自用户排序列表 `outboundPriority`（白名单天然规避虚拟网卡），要求有 IP 且判定为**非校园网**。逐卡判定（`:16` `is_campus_adapter`）与 `campus_check.rs` 同源：与 `campus_gateway` 同 /18 网段，或**绑该卡源 IP** 的网关可达（`check_gateway_reachable_from`，避免多卡归因错位）。**SSID 不参与逐卡判定**——`netsh wlan` 只报当前连接的单个 SSID，多无线卡下归因不可靠。无可用候选不进退避，30s 循环天然支持热点半夜开启后自动补切。

### metric 读写底座

- **读**（免提权）：`platform/metric.rs::read_interface_metrics`（`:54`）走 Iphlpapi `GetIpInterfaceTable(AF_UNSPEC)` 运行时值，**不读持久层注册表**——metric 注册表设置系统重启自动还原，与"重启即还原"的还原策略一致，永远无永久性破坏。
- **写**（提权）：helper 新 op `set_metric`（`HelperOp::SetMetric`，条目编码 `"{guid}:{family}:{automatic}:{metric}"`，GUID 不含冒号故编码安全），worker 内逐条取整行改副本（`UseAutomaticMetric`/`Metric`/`SitePrefixLength`）再 `SetIpInterfaceEntry`。已知坑：**`SitePrefixLength` 必须置 0**，非 0 直接 `ERROR_INVALID_PARAMETER` 且错误信息不指向具体字段（EasyTier/mullvad 同款实现佐证，细节见 [[set-ip-interface-entry-metric]]）；且 `Family`/`InterfaceLuid`/`InterfaceIndex` 等必须取接口当前值，构造残缺结构体同样参数校验失败。
- 目标卡 IPv4+IPv6 两族设 `UseAutomaticMetric=false, Metric=1`（自动跃点通常 ≥25，差距大避免平手抖动）。

### 快照与切换态时序

切换前先读目标卡两族 metric → 快照 JSON（`[{guid, family, automatic, metric}]`）落盘 `outboundMetricRestore` → **落盘成功才认定切换态成立**（"落盘先行"语义，沿运营商夜切先例）→ 再调 helper 写 metric。helper 失败**不清快照**：若 v4 已改成功而 v6 失败，清快照会让已改的跃点无从还原——切换态保留 + 退避重试补写（`needs_replay`：快照非空**且**切换侧有失败历史才补，稳态夜间约 900 拍不重复提权）。切换态下不重选目标卡（保持既有目标，还原后再按新排序选）。

### 三重还原保障 + 终态出口

1. **运行时修改天然兜底**：重启自动还原，无永久性破坏；
2. **06:30 恢复窗口主动还原**（`apply_outbound_restore`）：按快照逐族写回原值（`automatic=true` 时显式恢复自动跃点），成功后清快照；
3. **启动对账**（`reconcile_outbound_on_startup`，`scheduled.rs:672`）：残留切换态（崩溃/被杀/睡眠跨过恢复点）在恢复窗口内或功能已关 → 立即还原；夜间窗口内只有"目标卡仍在且判非校园网"才重放切换（只写目标值不动快照原值），目标卡消失或已回校园网 → 直接还原、放弃本夜切换。
4. **终态出口**：目标卡 GUID 查不到（拔出/禁用）或跃点行消失 → **视为已还原，清空快照**（无对象可写、系统重建协议栈按默认值即还原语义）；快照 JSON 损坏同样清空（坏数据无还原信息，且不清会永久压住巡检与运营商夜切）。

### 退避与提权降级

- 失败退避 `outbound_backoff_ms`：0 → 首试、60s → 120s → 240s → 300s 封顶（切换/还原独立计数，成功清零）；首次切换失败发一次通知，避免 30s 循环整夜刷日志。
- UAC 降级照 adapter_watch 先例：首试静默（CMSTPLUA 通道零打扰），仅**确因提权通道失败**（没拿到 helper 结果）才允许弹 UAC（`outbound_allow_uac`）；helper 正常跑完但逐条失败属业务失败，弹 UAC 无解、不抬高放行计数。
- 还原连续失败达 3 次 → 告警通知"请手动恢复跃点"并**保留快照**（用户可见可干预），不无限刷。

### 冲突协调（桌面）

- **巡检整轮跳过**（`background_check.rs:48-53`）：切换态期间 `run_background_check_blocking` 直接 return——portal 检测走热点出站必失败，15s 一拍会累计触发 MAC 重置提权 + 整夜告警。30s 后自然恢复。已知例外见已知限制①。
- 切换态与账号无关（metric 是系统态），`switch_account` 不清 `outboundMetricRestore`。
- **导入配置清空两个切换态字段**（`config_cmd.rs:181-183`）：切换态是本机系统状态，不可随配置迁移。
- DNS 有意不随 metric 走（多网卡并发解析下热点 DNS 通常胜出；一期观察，实测夜间解析仍命中校园网 DNS 再做二期）。

## 安卓机制

### 动作序列（切换侧）

`monitor_loop.rs::run_scheduled_actions` 出站分支：

1. **前置在线检查**：上一拍巡检判定不在线则跳过注销请求（`do_logout` 无在线前置，盲发必失败）并清失败计数（离线即"确认离线"，不消耗失败预算）；
2. **注销**：在线则走 `do_logout`，"离线已生效"判据取 **`radiusOk || unbindOk`**（`auth/protocol.rs:379-380` 新增两个子步骤信号，只增字段向后兼容）——`success` 只取 Radius 单边，但 MAC 解绑（ePortal 4.1.x 按 `wlan_user_ip` 踢）同样是破坏性踢下线，解绑成功即本机已离线；只看 `success` 会把 `(false, true)` 组合误判为注销失败，导致掉线重连→再注销的整夜摆动；
3. **落标记**：`nightOutboundRestore = "logged_out"`（纯标记，不暂存账号名；恢复登**当前活跃账号**，无跨账号残留）落盘成功才触发重检；
4. **触发系统重检**：`trigger_wifi_recheck` 经 network-bind 插件新命令 `reportWifiUnusable`（Kotlin `:409`）→ `ConnectivityManager.reportNetworkConnectivity(wifi, false)`（公开 API 无需权限）→ 系统 NetworkMonitor 重验证，失败后 `network_avoid_bad_wifi` 开启时系统自动把默认网络切到蜂窝。**应用侧不做任何 metric/开关写入**（改跃点属桌面能力，安卓无权限）。

### 判据与上限

- **注销失败上限 3**（`OUTBOUND_SWITCH_MAX_FAILS`）：注销请求在网络断开时本来就发不出去，无限重试无意义——达上限按"断网规律"照常建态，交系统侧重检验证接管。
- **还原失败上限 3**（`OUTBOUND_RESTORE_MAX_FAILS`）：晨间 `night_switch_login` 连续 3 拍失败 → 放弃自动还原、清标记恢复巡检并发 error 日志要求手动登录——不止住的话标记会永久压住巡检且每拍都发起注定失败的登录。
- **重试只保留最后一次结果**（`do_logout_with_retry` 语义）：交替型失败（第 N 轮 unbind 成功→后续轮全败）最终 `unbindOk=false`，由 3 次上限兜底建态，不摆动（完全消除需在重试聚合处做 OR 累积，属共享 crate 语义改动，本轮不做）。

### 防 stale 与全入口巡检闸

- **`latest_settings` 落盘前重读**（`monitor_loop.rs:617`）：切换/还原写盘若基于拍首快照克隆，会把期间已改的字段用旧值覆盖回去（典型：还原刚清掉的标记被同拍运营商夜切块复活写回）。出站动作后运营商夜切与定时登录也改用重读的新鲜配置。
- **巡检闸全入口**（`run_check_once` 开头，`monitor_loop.rs` 约 `:1176-1182`）：闸放在 `run_check_once` 而非 tick 循环——周期拍、WiFi 变化事件（`handle_wifi_event`）、手动检测三条路径共用这一个闸。**`run_scheduled_actions` 不经过此闸**（晨间还原判定必须在切换态下照常执行）。
- **`verify_night_switch` 双守卫**：运营商夜切验证的入口与复验轮（"注销→重登"）之前各查一次切换态——等待窗内新建的切换态会被复验轮把账号登回，打断出站等待窗口。
- **切账号/删除当前账号清 `nightOutboundRestore`**（`account_cmds.rs`）：纯标记无恢复目标语义，但残留会让巡检永久跳过；对齐 `night_operator_restore` 的同位先例。

## 前端（双端）

- **桌面 NetworkPanel** 新「夜间出站切换」卡：开关（默认关——改系统路由属侵入性动作）+ 网卡排序列表 + **排第一的卡常显「夜间出站目标」徽标**（不受开关影响，开关只控夜间自动动作）+ 管理员权限提示。排序落 `outboundPriority`（完整顺序列表，新出现的网卡追加尾部）。一期用 ↑/↓ 按钮；2026-09-23 起改为长按拖拽排序并与适配器卡合并（见「桌面卡片合并与拖拽排序（2026-09-23 三期）」节），同日四期撤销把手通道（见「桌面排序仅保留长按拖拽（2026-09-23 四期）」节）。
- **安卓 NetworkPanel** 新同名卡：开关 + `network_avoid_bad_wifi` 引导块（原理说明 + adb 命令复制按钮 + 边界提示：部分 ROM 有私有等价开关优先用系统自带；蜂窝关闭/无 SIM 无路可切；常开白天卡顿也切流量的副作用如实说明）。
- i18n zh/en 双语言包双端同步；「晚间断网自动切换」文案改名「自动切换运营商」。

## 已知限制（有意不修/待办，记录避免重查）

1. **`auto_login_on_start` 未过出站闸**（双端同构限制）：启动自动登录链路（桌面 `monitor/auto_auth.rs::run_auto_login_on_start`、安卓 `monitor_loop.rs::auto_login_on_start`）不判切换态——切换态内重启应用会重登校园网。桌面登录失败无害（启动对账会在夜间窗口重放切换）；安卓重登成功则流量回 WiFi、切换态名存实亡但标记仍在，06:30 恢复窗口会再登一次并清标记自愈。未修原因：启动登录是独立编排，加闸要动两端的启动路径，收益（用户恰在切换态内重启的低频场景）不抵评审面扩大。
2. ~~**`network_avoid_bad_wifi` 写通道不可用**~~（2026-09-22 已解决，见「WRITE_SECURE_SETTINGS 自动写增强」节）：一期只引导手动开启（adb/系统设置），且既有 `acceptWifiNetwork` 命令的 `applyNetworkSettingsCompat`（NetworkBindPlugin.kt:279-292）写 `captive_portal_mode=0` 与 `network_avoid_bad_wifi=0` 不回滚——两件事已随自动写增强一并落地（快照还原机制同时覆盖兜底路径的回滚出口）。
3. **桌面 `nightOutboundRestore` 字段是死字段**：为满足「字段集双端同构」纪律（[[config-field-sets-bidirectional-sync]]）而存在；桌面真实切换态字段是 `outboundMetricRestore`，桌面代码不消费 `nightOutboundRestore`（反之安卓不消费 `outboundMetricRestore`/`outboundPriority`）。仅导入配置清空逻辑触达它。
4. **安卓同拍双登可能**：出站 Restore 成功当拍置 `outbound_active=false`，同拍随后的定时登录判定（`!outbound_active && should_fire_scheduled_action`）可能再登一次——恢复窗口恰好压着定时登录时刻时出现。后果轻（重复登录幂等），不为此引入拍内去重。桌面无此问题（定时登录判定在出站动作之前取拍首状态）。
5. **功能已关+态残留的兜底判据内联不重构**：桌面抽为可测纯函数 `outbound_action_for`（`scheduled.rs:214`，10 个单测），安卓在 `run_scheduled_actions` 内联同款三行——刻意不抽公共纯函数（安卓编排本就独立于桌面 monitor 模块，抽取需扩大桌面 crate 的 pub 面，收益小）。两端行为必须保持同步，改动时人工对照。
6. **captive portal 白名单误判风险**：个别校园网部署放行系统探测 URL——安卓注销后系统验证仍"通过"，`avoid_bad_wifi` 不切流量。真机实测项；命中则二期改"注销+提示"降级。
7. **切换时机由系统探测周期决定**（安卓）：`reportNetworkConnectivity` 后系统按自身节奏重验证，不保证 23:00 整点切蜂窝，与"自动衔接"语义相容但非秒级。
8. ~~**与 TUN 代理（mihomo auto-route）并存时 metric=1 不接管整机流量**~~（2026-09-27 六期已解决，机制与并存语义见「禁用+兜底路由方案落地（六期）」节）：纯跃点切换确实被 TUN 压制，六期改用禁用校园网网卡产生接口/路由事件迫使 mihomo 重选出站。

## WRITE_SECURE_SETTINGS 自动写增强（2026-09-22 二期）

一期已知限制②的落地（设计同日批准，方案：还原**原值**而非默认值，语义同桌面 metric 快照）：

- **manifest**：`AndroidManifest.xml` 声明 `WRITE_SECURE_SETTINGS`（`tools:ignore="ProtectedPermissions"`）。仍受 signature|privileged 保护，需用户 adb `pm grant com.campuslogin.client android.permission.WRITE_SECURE_SETTINGS` **一次性授权**；授权后应用可静默写 `Settings.Global`，`network_avoid_bad_wifi=1` 免手动。
- **快照还原机制**（`NetworkBindPlugin.kt`）：`applyNetworkSettingsCompat` 写两键（`network_avoid_bad_wifi`/`captive_portal_mode`）前先记原值到插件私有 SharedPreferences（`network_settings_guard`，`prev_` 前缀；原值=1 视为系统默认不记录；已有快照不覆盖——首次写入为准；读取异常按 1 处理）。这同时给出 `acceptWifiNetwork` 兜底路径写 `captive_portal_mode=0 + avoid_bad_wifi=0` 的**回滚出口**（原已知限制②后半）。
- **新命令 4 个**（Kotlin @Command + 插件 Rust 封装）：`ensureAvoidBadWifi`（权限检查→幂等写 1→记快照）、`restoreAvoidBadWifi`（快照写回；写失败保留快照待重试）、`restoreWrittenSettings`（两键全量手动还原）、`getSecureSettingsStatus`（授权/当前值/待还原查询）。插件 `build.rs` 的 `COMMANDS` 数组必须同步补命令名（tauri_plugin 据此生成权限脚手架，见 [[plugin-permission-dangling-refs]]——手写 ACL 三件套会被构建再生覆盖，`default.toml` 才是权限集 source of truth，kebab-case 引用经归一化解析）。
- **Rust 编排四触发点**（`monitor_loop.rs`/`account_cmds.rs`）：①Switch 分支落标记成功后自动 ensure（替代原「无法程序化确保」warn）；②Restore 两处清标记后自动还原；③启动兜底——标记空但有快照即还原（覆盖崩溃/强杀错过恢复窗口）；④切账号/删当前账号清标记后还原。
- **前端**（`NetworkPanel.tsx`）：引导块升级状态感知——已授权绿框「已授权自动管理」；未授权显示 `pm grant` 一键复制（原 `settings put` 手动命令降为备选）；「还原系统设置」按钮（成功/无操作/失败 toast）。
- 桌面不涉及（Settings.Global 为安卓特有）。

## 已排除路线（避免重查）

`WRITE_SECURE_SETTINGS` 写 `Settings.Global.WIFI_ON`（AOSP 源码证伪：仅 @Readable，"Only the Wi-Fi service should touch this"）；Shizuku 提权 `svc wifi disable`（需第三方应用+重启重激活）；Device Owner 豁免（已有账号设备被拒）；VpnService tun 转发 / 无障碍模拟点击（过重/脆弱）。

## 桌面卡片合并与拖拽排序（2026-09-23 三期）

一期前端节里「不引入拖拽」的决策撤销（当时顾虑安卓 WebView 滚动冲突与新增依赖——实际仅桌面引入，且 framer-motion ^12.38.0 桌面 frontend 已有、`Reorder.Group/Item + useDragControls` 是其内置能力，零新依赖；安卓排序 UI 不存在，滚动冲突前提不成立）。用户需求：网络适配器卡与夜间出站切换卡合并为一张卡；↑/↓ 按钮改长按拖拽。**仅桌面**（安卓端 NetworkPanel 不动）。

- **合并卡结构**：`tauri-app/frontend/src/network/NetworkPanel.tsx`——头部左「网络适配器 + 检测数」、右 MoonStar 图标 + Switch（夜间出站切换总开关，title/aria-label=nightOutboundSwitch）；头部下方整行 nightOutboundSwitchDesc 小字（沿 DNS 卡描述下移先例）；列表 = 适配器统一列表，每行把手（GripVertical 按钮）+ 图标（Wifi/Cable）+ 名称/IP(mono)/速度 + 徽标（主适配器/副适配器/无线/状态/夜间出站目标）+ 行按钮（启用/获取新IP/刷新DHCP）。原「主适配器排最前」的自动排序取消——主/副由徽标表达，顺序完全由用户拖拽决定（`outboundPriority` 语义不变：出站优先序 ∪ 新检测网卡追加尾部）。
- **顺序构建纯函数** `outboundOrder.ts::buildOutboundOrder(priority, detected)`：priority 非空 → 按 priority 过滤掉已拔出网卡后排序、detected 中新卡按发现顺序追加尾部；priority 空 → 直接用发现顺序。7 个 vitest 用例（空 priority/乱序/失效过滤/新卡追加/全失效/去重/纯函数不变参）。
- **拖拽交互双通道**：①行体任意处长按 250ms（`DRAG_LONG_PRESS_MS`）起拖，移动超 8px 死区（`DRAG_DEAD_ZONE_PX`）取消长按——保护滚动与点击；②左侧把手 pointerdown 立即起拖（stopPropagation+preventDefault，不需长按）。`Reorder.Item` 挂 `dragListener={false}` + `dragControls`，行级 pointer 处理器统一管理；行内按钮区包一层 `onPointerDown stopPropagation` 防点按钮误触长按。拖拽中用本地 state（`dragOrder`）渲染，`onDragEnd` 才一次性提交 `onUpdateConfig({outboundPriority})`（拖拽期间外部顺序同步被 `isDraggingRef` 屏蔽，异步回显不打断手势）；`whileDrag` 抬起态（scale 1.02 + 阴影）。（四期注：通道②已撤销，仅保留长按通道，见下节。）
- **验证**：tsc 0 错误、vitest 96/96、vite build 通过；Playwright（仓库外临时环境 + 临时 dev mock shim，已删）真浏览器实测 4 场景全过——初始序渲染、长按拖 WLAN→顶（`save_config` 恰一次、`outboundPriority=["WLAN","以太网","以太网 2"]`）、快速滑动不重排不提交、把手拖以太网 2→顶正确。
- 顺序提交语义与夜间出站切换的候选选择（`select_outbound_candidate` 按列表顺序）天然衔接：拖到第一位的卡即夜间出站目标徽标行。

## 桌面排序仅保留长按拖拽（2026-09-23 四期）

三期双通道交互上线当日的用户反馈迭代：去掉「点击特定卡片（把手）进行排列」，只留长按条目任意位置起拖；功能描述改「切换为非校园网」，并要求提示用户通过拖动决定切换目标。仅桌面（安卓端无排序 UI，不涉及）。

- **把手通道撤销**（`tauri-app/frontend/src/network/NetworkPanel.tsx`）：删除行内 GripVertical 把手按钮（`pointerdown` 立即 `startDrag` 通道）——「点击特定卡片排列」的操作模型认知负担高，且把手挤占行首视觉空间。长按 250ms（`DRAG_LONG_PRESS_MS`，`:52`）+ 8px 死区（`DRAG_DEAD_ZONE_PX`，`:54`）+ 行内按钮区 `stopPropagation` 三件套原样保留；`whileDrag` 抬起态、本地 `dragOrder` 渲染、`onDragEnd` 一次性提交语义全部不变。`GripVertical` import 保留——列表底部提示行装饰图标仍用它（`:514`）。
- **文案三层收敛**（zh/en 同步，zh.json/en.json `:297-300`）：①`nightOutboundSwitchDesc`「…自动把出站切换为非校园网（热点/物联网卡）…」——从「切到排序最前的非校园网网卡」的机制描述改为面向效果；②`outboundDragHint`「长按条目任意位置拖动调整顺序，排最前的非校园网网卡即夜间切换目标」——删把手操作提示，把「拖动 → 排最前 → 成为切换目标」的因果写进提示（回应用户「增加提示用户通过拖动切换」）；③`outboundAdminHint` 删去「排第一的网卡即夜间出站目标」（与常显徽标、新拖拽提示重复）。
- **验证**：tsc 0 错误、vitest 96/96；Playwright 真浏览器实测 6 场景全过——三处新文案、行内无把手按钮、首行 WLAN+徽标、快速拖不重排不落盘、长按「以太网 2」到顶（`save_config` 恰一次、`outboundPriority=["以太网 2","WLAN","以太网"]`）、原地长按不落盘；0 page error。测试环境同三期（仓库外 Playwright + 临时 dev mock shim `window.__TAURI_INTERNALS__`/`__TAURI_EVENT_PLUGIN_INTERNALS__`，测后完整还原）；实测教训：`Reorder.Item` 默认 `as="li"` 渲染（Group 才显式 `as="div"`），行选择器与等待信号（「夜间出站目标」徽标唯一，避免命中 Dock/快捷按钮同文案）据此选取。

## 文案精简与夜间断网实测复盘（2026-09-27 五期）

用户两项文案要求落地（`nightOutboundSwitchDesc` zh/en `:297`，现为「周日至周四 23:00、周五六 23:30 自动把出站切换为非校园网，次日 6:30 起自动还原」——去括号补充、去「需保持应用在后台运行」；后者描述的是真实前提，仅按用户要求移除文案）。同日复盘 2026-09-26（周五）夜间实测「时间到了没切换、无法上网」，日志取证（安装目录 `logs/app-2026-09-26.log` / `app-2026-09-27.log`）：

- **切换动作本身成功**：23:30:19 评出目标卡 WLAN（热点 192.168.6.109，判非校园网正确），helper 两族 metric=1 写入成功、快照落盘、通知「已切换出站到 WLAN」。23:31:35 adapter_watch 自动 pnputil 启用两张被 PnP 禁用的有线卡。
- **6:30 恢复窗口无进程执行**：09-27 日志首行 07:43:23「应用启动」，还原由启动对账补做（07:43:24 成功）。后台运行前提昨晚被违反（PC 关机或应用退出，原因未定位——「适配器缓存后台刷新任务已停止」经 09-25 对照是任务重启语义，非应用退出标志）。
- **流量未跟随的机制**（本机路由表实测 + 同类实现调研）：FlClash（mihomo 内核）TUN auto-route 以 `0.0.0.0/0 → 198.18.0.2` RouteMetric=0（Protocol=NetMgmt）持有默认路由，物理卡 metric=1 在「应用→出口」这一段永远赢不了它；metric 切换只可能影响 FlClashCore 自身出站选卡——本机 FlClash 配置无 `interface-name` 绑定（全目录 grep 无命中），走 auto-detect-interface 跟随默认路由物理卡。跃点修改不产生路由增删事件，探测若非轮询式重算则感知不到（有效跃点 = 接口跃点 + 路由跃点，wireguard-windows netquirk 文档口径），出站焊死在已断的校园网卡即整夜断网——与用户观察「连接还在其他网卡上」吻合。独立候选：热点无上游（手机休眠/欠费）、DNS 不随 metric 走（见冲突协调节一期观察）。
- 用户原假设「适配器没被选择就不会被操作」对选择器成立（`select_outbound_candidate` 只操作列表内、有 IP、判非校园网的第一张卡），但本夜 WLAN 恰被选中并操作成功，断网另有原因。

二期候选方案（待用户拍板后实施）：

1. **压制竞争接口**：切换时把其他在连接口（含 TUN 虚拟卡）接口 metric 抬至 5000，快照从「仅目标卡」扩展为多接口，还原统一写回——TUN 0+5000 > 目标卡 1+0，默认路由真正易主；代价是夜间 FlClash 全局分流失效（流量不再进 TUN），需用户确认取舍。
2. **切换后验证 + 通知**：写入成功的下一拍读回默认路由接口或对目标卡真实探测，未接管即告警通知——避免用户半夜才发现。
3. **DNS 跟随**（冲突协调节二期欠账）：切换时把目标卡 DNS 置顶、系统解析跟随。
4. **合并卡显示当前生效出站接口**（所见即所得）。

## 禁用+兜底路由方案落地（2026-09-27 六期）

用户 v2 提案（2026-09-27）否定纯跃点路线（原话大意：直接修改路由表新增一条排在 TUN 后面的路由，再增加操作把连接校园网的网卡禁用后再启用）。拍板方案与硬约束：

- **方案**：「禁用校园网网卡 + 兜底路由 + 保留 metric」三件套（替换五期候选 1）；启用（还原）时机=6:30 恢复窗口（原机制不变）。
- **硬约束①**：USB 网卡绝不禁用——禁用会导致 USB 网卡下次开机无法正常启用（Windows 提示「请安装网卡」）。
- **硬约束②**：软件对 USB 网卡状态判断不准（截图实证：以太网 2 实为未连接却显示已禁用+启用按钮）→ 禁用名单只记**亲手禁用的 GUID**，绝不靠状态推断；状态误判本身一并修（见下「幽灵设备误判修复」）。

### 切换动作（`apply_outbound_switch` 重写）

目标卡选择与两族 metric=1 写入不变，新增两步：

1. **禁用校园网网卡**（`monitor/outbound_switch.rs::select_campus_to_disable`）：候选=Connected + 判校园网 + priority 白名单内 + 有 GUID/IP + **非目标卡**（排除 `exclude_guid`，目标卡禁了就没人出站）+ **USB 守卫过滤**（`unsafe_to_disable`：PnPInstanceId 前缀 `USB\`（大小写不敏感）或读取失败一律 true=跳过，保守）。逐张 helper `disable_adapter`；名单 `DisabledRow{guid,name}` 序列化 `outboundDisabledAdapters`（`[{guid,name}]` JSON），**只记成功下发禁用的卡**。与 metric 快照、路由快照**一次落盘**（三快照成立切换态，落盘先行语义不变），再执行动作——失败不清快照退避重试的既有语义保留（`needs_replay` 三快照+失败计数联合判定，稳态夜间约 900 拍不重复提权）。
2. **兜底路由**（`StandbyRoute{dest,mask,gateway,metric,if_index}` → `outboundStandbyRoute`）：`0.0.0.0/0` 指向目标卡网关、metric=2（`OUTBOUND_STANDBY_ROUTE_METRIC`）、IF 目标卡，helper `route_add` 下发。网关为空则不下发（None，纯 metric+禁用兜底）。

通知文案：`write_outbound_metric` 加 `notify_suffix` 参数——「已切换出站到 X，已临时禁用 N 张校园网网卡」（N=0 时无后缀）。

**为什么这套动作能接管流量**：FlClash TUN（0.0.0.0/0 metric 0）仍持有默认路由——兜底路由 metric=2 永远抢不了它，**唯一角色是 FlClash 退出后的 failover**。真正的主力是禁用：校园网卡禁用产生接口/路由删除事件，mihomo `auto-detect-interface` 重选出站接口——已禁的校园网卡从候选消失，TUN 出站落目标卡（热点）；路由/DNS/接口竞争同时全消。目标卡 metric=1 保留，是 mihomo 出站选择的依据（TUN 退出后默认路由由同卡的 metric 2 兜底路由顶上）。夜间并存语义：**FlClash 运行中**流量仍进 TUN、出口=热点（代理路径恰好可用）；**FlClash 退出**后默认路由=兜底路由（目标卡），直连热点。

### 还原动作（`apply_outbound_restore` 重写）

顺序固定：① `route_delete` 兜底路由——失败仅 warn **不阻塞还原**（运行时路由重启即清，非持久破坏）；② metric 按快照写回（终态出口语义不变：卡不存在/跃点行消失视为已还原；**metric 快照 JSON 损坏 → 清快照按已还原收尾**——禁用名单可能仍在但收尾优先，防坏数据永久压住巡检，与一期口径一致）；③ 逐卡 `enable_adapter`：仅名单内 GUID（名单外被禁的卡不碰）、卡已不存在跳过；**enable 失败=还原失败整体重试**（名单保留——禁用的卡不能放着不管）；④ 成功通知：`enabled_count>0` →「已还原网络设置（启用 N 张校园网网卡）」，否则沿用「已还原网卡跃点设置」。`clear_outbound_snapshot` 清三字段。

### replay 三分支幂等补做

`replay_outbound_switch`（原 `replay_outbound_switch_metric` 改名）三分支：metric 补写 / 禁用补做（卡不存在跳过）/ 路由补加——提权 gate 开头一次，各分支独立判定、全部幂等（重复禁用无害、同条目重复添加无害）。

### helper 新 op（`helper/mod.rs`）

`HelperOp::DisableAdapter{name}` / `RouteAdd{rows}` / `RouteDelete{rows}`（均 cfg windows）：`disable_adapter` 镜像 `enable_adapter`（validate + 强制重扫存在性校验 + `netsh interface set interface name=X admin=disable`）；路由行编码 `"{dest}|{mask}|{gateway}|{metric}|{if_index}"`，`Ipv4Addr` 严格解析（非法字符直接拒绝，防参数注入 netsh）；`route_delete` 找不到目标按成功处理（幂等）。`SetMetricFailure` 更名 `HelperFailure`（`run_set_metric_helper` 泛化为 `run_helper_op(op, args, allow_uac)`，三 op 共用 UAC 降级链）。

### 幽灵设备误判修复（`discovery/windows.rs`）

USB 网卡拔出后设备节点成幽灵（phantom）：`AdminStatus` 或 Class 注册表 `ConfigFlags` 残留 DISABLED 位 → 误分类「已禁用」（自动启用逻辑对着不存在的卡提权——用户「USB 判断不准」反馈的根源）。`classify_adapter_status` 加第 6 参 `pnp_present: Option<bool>`——`Some(false)`（经 `discovery/devnode.rs::read_pnp_instance_id`（改 `pub(crate)`）+ `devnode_problem` 判定）直接判 `Disconnected`；查询不可用（None）回退旧行为。

### adapter_watch 冲突修复（09-26 实证）

`adapter_watch.rs` 两处过滤 `night_disabled` 名单（`parse_disabled_adapters` 的 name 集合）：①禁用警告通知块——本功能夜间禁用不再告警；②自动启用块——名单内的卡不自动启用（09-26 23:31:35 实证 PnP problem 22 自动启用与夜切冲突，把刚禁的卡启回去）；名单外的卡照常自动启用（用户手动禁用行为不变）。

### 配套状态

- 切换态判定三快照联合（`config/outbound_switch.rs::outbound_restore_active(metric, disabled, standby)` 任一非空）：`scheduled.rs` 循环/启动对账、`background_check.rs` 巡检跳过均改用；`config_cmd.rs` 导入配置清三字段；`config/model.rs` 加 `outbound_disabled_adapters`/`outbound_standby_route`（默认空串，双端同构；安卓不消费）。
- 前端：tauri-app/android 的 types.ts+constants.ts 补 `outboundDisabledAdapters`/`outboundStandbyRoute`；i18n `nightOutboundSwitchDesc`（桌面 zh/en `:297`）加「并临时禁用校园网网卡」语义（android 同 key 语义不同未动）。

### 五期候选落实情况

- **候选 1（压制竞争接口抬 metric 5000）**→ 被禁用方案替代：禁用比抬跃点彻底（路由/DNS/竞争全消），且 mihomo 事件感知可靠。
- **候选 2（切换后验证+通知）**→ 未实施（禁用动作自带强信号，mihomo 接口事件必然触发；后续可加默认路由读回校验）。
- **候选 3（DNS 跟随）**→ 未实施：校园网卡禁用后 DNS 竞争卡消失，热点 DNS 天然胜出，五期观察项基本消解。
- **候选 4（合并卡显示生效出站）**→ 未实施。

## Connections

[[night-operator-switch]]、[[dual-platform-sharing]]、[[scheduled-actions-outside-silent-window]]、[[config-field-sets-bidirectional-sync]]、[[logout-radius-first]]、[[set-ip-interface-entry-metric]]、[[windows-task-proxy-elevation]]、[[outbound-switch]]、[[desktop-config]]
