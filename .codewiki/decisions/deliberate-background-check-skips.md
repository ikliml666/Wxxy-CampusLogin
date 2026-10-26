---
title: 后台巡检中被主动跳过的逻辑清单（易被误认为缺陷，实为设计）
type: decision
source_files:
  - tauri-app/src-tauri/src/monitor/background_check.rs
  - tauri-app/src-tauri/src/monitor/campus_check.rs
  - tauri-app/src-tauri/src/monitor/adapter_watch.rs
  - tauri-app/src-tauri/src/monitor/auto_auth.rs
  - tauri-app/src-tauri/src/monitor/latency.rs
  - tauri-app/src-tauri/src/monitor/quality_scheduler.rs
tags: [决策, 巡检, 监控, 有意为之, 自动登录, 会话失效]

## 背景

`run_background_check_blocking`（`tauri-app/src-tauri/src/monitor/background_check.rs:50-467`）每拍依次做：取适配器（:64-73 `get_adapters_cached` / `get_adapters_force`）→ 出站切换态/静默期判定（:84-91 `outbound_restore_active`、:92-133 `is_campus_check_silent` 与 `RESTORE_SILENT_PASS` 放行逻辑）→ 校园网验证（:136-196 `check_campus_network`）→ Portal 检测（:206-239 双适配器 `tokio::join!` + `spawn_blocking` 调 `check_adapter_portal`）→ 会话状态判定与失败计数处理（:243-317 MAC 重置容错与计数重置、:319-380 状态推导与 `any_online` 计算）→ 自动登录（:436 `try_auto_login_on_preparation`）→ 断线重连（:438-442 `try_disconnect_reconnect`）→ 状态落盘与事件上报（:373 `last_a2_online` 更新、:397-402 `handle_status_change`、:413-434 `emit_background_check_result`、:447 `update_network_state`）以及动态间隔结局分类（:454-466 `CheckOutcome` 返回）。

这条链路里有多处“看似该做却没做”的行为，容易被后来者当缺陷“修掉”。本文档以当前代码为准，记录这些**主动跳过**的逻辑及其设计理由；末尾另记一条**相邻的非跳过决策**（会话失效时重置登录标记），它和“跳过”只共享同一个症状面——“到点不自动登录”——但根因与修法完全不同，不要混为一谈。

## 决策

以下条目是**主动跳过**、有意为之：

1. **校园网名称检查关闭时名称腿不参与判定，但仍走网段/网关/Portal 绑源探测**

   `tauri-app/src-tauri/src/monitor/campus_check.rs:66-99`（`CampusContext` 与 `name_hit`）：当 `config.enable_network_name_check` 为 `false` 时（:67 `enable_name_check` 字段，:81-84 `name_hit` 直接返回 `None`），不执行 SSID / 有线 Profile 匹配。逐卡判定 `judge_adapter`（:106-162）仍按成本短路：无 IP 的卡直接判非校园（:120-122）；网段纯位运算（:130-136）；网关绑源探测（:139-145）；Portal 绑源探测（:148-154，私网 IPv4 主机为空时该腿恒否）。`check_campus_network`（:188-278）通过 `portal_probe_host`（:193）取主机，`on_campus` 取逐卡任一命中（:248）。此分支下 `wifi` / `wired` 的 `name` 可能为 `None`（:245-246），但不再依赖名称信号。

2. **校园网检测静默期内整体跳过本轮巡检（含 Portal 检测/自动登录/断连重连）**

   `tauri-app/src-tauri/src/monitor/background_check.rs:92-133`：当 `is_campus_check_silent(now_min, config.campus_check_start_minutes, config.campus_check_end_minutes)` 返回 `true`（:92-96）且非还原后强制补检（`forced`，:98-102 通过 `RESTORE_SILENT_PASS.load` / `store` 消费持续放行标记，离开静默期即清除）时，执行保留动作 `cancel_campus_exit(app_handle, state)`（:120）并更新状态 `current_ssid = None`、`on_campus_network = true`（:121-124），打印日志「校园网检测静默期，跳过本轮巡检（Portal 检测/自动登录/断连重连均不执行）」（:125），直接 `return CheckOutcome::Skipped`（:126）。静默期判定本体已下沉到双端共享面 `crate::config::schedule::is_campus_check_silent`，`campus_check.rs:285` 仅保留 `pub use` 转发出去（2026-10-26 重构，原实现与边界语义已迁移），调用方仍可按 `monitor::campus_check` 路径引用。

   *理由*：静默期跳过的是**整轮探测**，不是仅状态面的保鲜。历史缺陷（2026-10-25 修复，见 `background_check.rs:104-118` 注释）原本只跳过「校园网环境验证」，Portal 登录检测、自动登录、断连重连照跑——校园网 23:00 断网后每分钟一轮空转认证请求，有账号锁定风险且使设置项失效。收敛后按文档语义「仅此时段内执行校园网检测」整体跳过，不累计退避、不更新 online 状态，避免静默期把「已登录」刷成离线并触发状态变更通知。例外：夜间出站还原后静默期放行（`forced`，见 `background_check.rs:42-48` 定义 `request_check_after_outbound_restore` 与 `RESTORE_SILENT_PASS`、:98-102 消费、:128-133 日志）可无视静默期照常执行本轮巡检，放行持续到静默期结束。

3. **校园网不通过但配置的适配器均无 IP 时不退出、等待网络恢复**

   `tauri-app/src-tauri/src/monitor/background_check.rs:187-193`：`let no_configured_ip = a1.is_none() && a2.is_none();`（:187）为真时打印「配置的适配器均无IP地址，跳过校园网退出，等待网络恢复」（:189），跳过 `start_campus_exit`；否则才触发最小化 + 退出（:192）。

   *理由*：完全无网络时触发退出没有意义——退出是“离开校园网”的处置，而“根本没连上任何网络”不是离开，应等网络恢复后重新检测。

4. **后台巡检不再触发全量质量检测**

   `tauri-app/src-tauri/src/monitor/background_check.rs:543-545`（`run_background_check` 的文档注释与实现）：质量检测由 `latency.rs` 的 `spawn_latency_test_loop`（:78-139） → `quality_scheduler::run_quality_check`（:56-122）独占。避免双定时器叠加触发全量外网检测、节流吞掉用户自定义间隔（2026-09-04 收敛）。仅当「已登录但门户不可达」快排查模式联动时触发一次性质量检测（`background_check.rs:462` 调用 `trigger_fast_quality_check`、:472-541 函数实现，内部仍走 `quality_scheduler::run_quality_check` 见 :504），非巡检自身发起。

5. **“自动检测”模式不参与自动启用**

   `tauri-app/src-tauri/src/monitor/adapter_watch.rs:166-172`（注释）与 `configured_disabled_adapters`（该函数在 `crate::network::adapter` 中由 :196 调用，已过滤空串与 `AUTO_DETECT_ADAPTER` 哨兵）：“自动检测”模式下没有用户手选的具体适配器，自动启用对它无意义，不参与。

6. **自动启用首次尝试静默提权不弹 UAC**

   `tauri-app/src-tauri/src/monitor/adapter_watch.rs:236`：首次尝试保持静默（`allow_uac = false`，因 `stats.auto_enable_failure_count.load(Ordering::Relaxed) > 0` 为假），兼容 CMSTPLUA 静默提权可用的环境零打扰；失败计数 ≥1 后才允许弹 UAC 降级（`allow_uac = ... > 0`，:236）。

   *理由*：CMSTPLUA 已被系统封堵的环境（0x80080017「未将类配置为支持提升的激活」）下静默提权不可得，UAC 是唯一恢复通道。频率由退避阶梯限制：`auto_enable_backoff_ms`（`adapter_watch.rs:292-299`）0 次→立即可试、1→60s、2→120s、≥3→300s 封顶，尝试时间戳先写后执行（:231 `stats.auto_enable_last_attempt_ms.store(now_ms, Ordering::Relaxed)` 在先，:242 起 `spawn_blocking` 执行启用）。

   *额外过滤*：

   - 夜间出站切换亲手禁用的卡不自动启用（:146-149 与 :207-210 解析 `night_disabled` 名单过滤；快照损坏时按“全部拦截”保守处理，:213 `night_disabled.as_ref().is_some_and(|nd| !nd.contains(&da.name))`）。
   - 夜间守护窗（时间窗 ∧ 切换态）内一律不出自动启用目标（:184-195 计算 `guard_window`，:211-215 `targets` 过滤条件中含 `filter(|_| !guard_window)`）。理由是七期实证：用户在切换前手动禁用的手选适配器无 IP、进不了六期闸门名单，自动启用会顶掉切换。

7. **（安卓，2026-10-02 起）WiFi 关闭时整拍短路**

   *注：安卓侧源码未在当前输入的源码清单中提供行号，此处只记录逻辑，不编造行号。* WiFi 关闭（`pick_campus_source_ip` 无非蜂窝 IPv4 接口）时，`probe_campus` 枚举网卡后直接返回全 false，不做网关 / Portal TCP 探测；`run_check_once` 的 Portal 探测短路为 Err。状态与通知走 `campus_status_message`「WiFi 未连接」与 `notify_state` 的 wifi_off 分支。

   *理由*：WiFi 关闭不是「校园网探测失败」，没有可探测的对端。

8. **（安卓，2026-10-02 起）非校园网时跳过 Portal HTTP 探测**

   *注：同上，安卓侧未提供行号，仅记录逻辑。* 非校园网（`on_campus=false`）时跳过 Portal HTTP 探测。`on_campus=false` ⇔ 子网未命中且网关 / Portal 两个 3s TCP 探测已并行执行且全败，同一目标 HTTP 请求必败，不再空转至多 8s。离网拍最坏 11s→3s。校内 Portal 宕机（`on_campus=true`）不在此列，仍由 HTTP 页面请求裁决在线与否。

9. **出站切换态下整拍跳过巡检**

   `tauri-app/src-tauri/src/monitor/background_check.rs:84-91`：`outbound_restore_active(&config.outbound_metric_restore, &config.outbound_disabled_adapters, &config.outbound_standby_route)` 三份快照任一非空即打印「出站切换态，跳过本轮巡检」（:89）并直接 `return CheckOutcome::Skipped`（:90），不等校园网检测、不 Portal 探测。下一拍（30s）自然恢复。

   *理由*：切换态下出站走热点，Portal 探测注定失败，会累计触发 MAC 重置提权、并回放整夜告警；详见 [[night-outbound-switch]] 的六期/七期语义与 `adapter_watch.rs` 的同源守护窗（第 6 条）。

## 相邻的非跳过决策：巡检发现会话失效时重置 `has_logged_online`

这一条**不是跳过**，而是本轮补记的反向动作——新增了一个重置点，用来修「整夜休眠 / 夜间出站切换后到点不自动登录」。

- **症状**：用户合盖过夜或夜间出站切换把校园网卡禁用再启用后，到检测时段不再自动登录，只能手动点。
- **根因**：`tauri-app/src-tauri/src/monitor/auto_auth.rs`（经 `background_check.rs:8` 引入 `try_auto_login_on_preparation`）中该函数开头读快照，`if snap.has_logged_online { return; }`（:125-127）——该标记一旦置位就永久拦住自动登录。而此前它只被三种事件改写：校园网检测失败（`background_check.rs:139-151`，仅覆盖 SSID / 网络名变化这一种失效，重置在 :150 `s.has_logged_online = false;`）、注销、登录成功。夜间出站切换禁用校园网卡把会话中断时，**没有任何代码路径复位它**，于是自动登录被“已登录过”这个陈旧事实长期挡住。
- **决策**：在 `try_auto_login_on_preparation` 调用（`background_check.rs:436`）**之前**新增会话失效判定（`background_check.rs:380-396`）：

  ```rust
  let any_online = online || secondary_online == Some(true);
  if should_reset_logged_online(reachable, any_online, state.network.load().has_logged_online) {
      state.network.update(|s| s.has_logged_online = false);
      crate::log_info!("background", "巡检判定未在线（服务可达），已重置登录标记，允许自动登录重新介入");
  }
  ```

  判据本身抽成了纯函数 `should_reset_logged_online(reachable, any_online, has_logged_online) -> bool`（`background_check.rs:25-27`，实现即 `reachable && !any_online && has_logged_online`），便于单测锁定边界；`mod tests`（`background_check.rs:557-586`）用四条用例覆盖：在线不重置（:562-567）、不可达不重置（:571-573）、从未登录不重置（:577-579）、可达+均未在线+曾登录则重置（:583-585）。

  判据是“服务可达 + 主副卡都未在线 + 标记还被置着”：服务可达说明不是探测失败的假阴性，主副都未在线说明此前建立的登录会话确实已失效（整夜休眠唤醒后网卡重连未登录、校园网卡被夜间切换禁用后又启用、Portal 掉线）。
- **为什么必须在 `try_auto_login_on_preparation` 之前**：同一拍就要能真正登录。若放在其后，还原后补检钩子触发的那一拍仍会被旧标记挡回，用户还要多等一拍（甚至多等一个间隔）。
- **为什么带 `reachable` 前置**：避免探测失败或配置的适配器均无 IP 时的抖动触发误重置。无 IP 时 `login_available` 本来也是 `false`（:321 等推导），重置没有任何收益，只会白丢掉标记；而 `reachable` 为假时的“未在线”是假阴性，不能据此判定会话失效。
- **为什么不放宽静默期 / 切换态跳过（第 2、9 条）**：那两类跳过本身是正确的——静默期没有探测依据，切换态探测注定失败且会触发 MAC 重置提权。真正的缺陷是“标记没有随会话失效而复位”。放宽跳过等于用更多的无效探测去换一个标记问题，副作用远大于收益。
- **本轮没有放宽任何跳过条件**：第 1-9 条全部保留不变，本轮只是在巡检链路里补了一个重置点。

## 理由

- 第 1 条：名称检查关掉是用户显式选择，“不按名字判”不等于“不判在不在校园网”，仍需独立可达性证据；网段、网关与 portal 绑源探测互为兜底（一方禁 ping 不等于离线），`portal_probe_host` 无有效主机时该腿退化为否而不会误报在线。
- 第 2 条：静默期整体跳过整轮探测，不是状态保鲜——见其上 *理由* 与 `background_check.rs:104-118` 注释；判定实现已迁移至共享面，monitor 侧仅转发。
- 第 3 条：退出是把“离开校园网”落地，而“完全无网络”不是离开，等网络恢复再判才不误伤。
- 第 5、6 条：见 `adapter_watch.rs:166-172`、`adapter_watch.rs:236` 行内注释与退避函数（静默优先、UAC 降级为兜底、退避限制频率）。
- 会话失效重置：见上节逐项（症状 / 根因 / 为何前置 / 为何带 `reachable`）。

## 备选方案

本轮评估过但不采用的做法：

| 方案 | 不采用的理由 |
| --- | --- |
| 放宽静默期跳过（第 2 条），让它“顺手把标记清掉” | 静默期本就没有校园网探测依据，把语义塞进跳过分支会让“静默”变成有副作用的动作；而且静默期整段不跑 Portal 探测，本来也无从判断会话是否失效 |
| 放宽出站切换态跳过（第 9 条），切态下也探测以“顺便重置标记” | 切态下出站走热点，Portal 探测必败，会累计触发 MAC 重置提权与整夜告警——这正是当初整拍跳过的原因；用无效探测换标记复位得不偿失 |
| 无前置条件地重置（只要 `!any_online` 就清标记） | 探测失败 / 无 IP 时“未在线”是假阴性，清标记会让自动登录在真正的网络故障期反复空转；无 IP 时 `login_available` 本就 `false`，清了也无用 |
| 在 `try_auto_login_on_preparation` 内部清标记 | 位置过晚：判断已经发生了才能清，本次巡检仍被拦，用户多等一拍 |

## 影响与约束

改动这些分支前先确认它们是设计而非缺陷。已知约束：

- **跳过语义不变**：第 1-9 条行为与判定条件本轮全部保留，未放宽任何一条。
- **重置只在服务可达时生效**：`reachable && !any_online && has_logged_online`（`background_check.rs:25-27` 纯函数，调用点 :390）三个条件同时成立才复位标记，探测失败路径不会造成抖动误重置。
- **重置在两处判据上各有分工**：`background_check.rs:139-151` 的复位负责“校园网检测失败”（SSID / 网络名变化）；`background_check.rs:380-396` 负责“网络名没变、只是会话超时或网卡被禁用后重启”。两者不可互相替代。
- **静默期语义两端不同**：桌面端整体跳过巡检（含 Portal 探测），安卓整拍 `return` 不探测；非在校时段的**在线状态本身**两端都保持上一拍、新鲜度不可比。但 2026-09-13 起安卓的**通知面**不再继承陈旧在线——静默期进入即把常驻通知重建为「已暂停检测(非检测时段)」，用户看到的不是过期状态。
- **状态变更检测用 `any_online`**：`handle_status_change` 的入参在 `background_check.rs:380` 已修正为“主或副任一在线即在线”（:380 计算 `any_online`，:397-402 传入 `prev_online, any_online`），否则双适配器“主断副通”时每个周期都误报“在线→离线”。

## Connections

[[quality-check-single-driver]]、[[night-outbound-switch]]、[[mask-placeholder-persisted-as-plaintext]]