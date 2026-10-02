---
title: 安卓常驻监控通知在线状态被离线护栏静默钉死
type: learning
source_files:
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/campus_detect.rs
  - tauri-app/src-tauri/src/monitor/background_check.rs
tags: [教训, 安卓, 状态机, 常驻通知, 在线判定]
---

## 现象

安卓真机断开校园网 WiFi 后（进程走蜂窝或家用网络），常驻监控通知长时间仍显示「监控运行中 · 在线」，直到手动重启监控才翻转。

## 根因（两因叠加，缺一不复发；描述 2026-10-02 短路修复前的历史机制）

1. **在线判定护栏把"探测失败"折叠成"沿用上一拍在线"**：三态消费原实现是 `Ok(s) if error_kind.is_none() => s.online, _ => prev_online`——本意是反误报（校园网内 Portal 页面特征间歇失配时不翻转，2026-09-09 的修复），但 `_ => prev_online` 分支不区分"校园网内探测失败"与"根本不在校园网"。
2. **常驻通知只在状态翻转时重建**：`notified_online` 记忆位（`monitor_loop.rs:49`）+ 仅翻转时 `update_notification`（翻转更新点 `monitor_loop.rs:1459-1470`；省电设计，不每拍重建）。状态不翻 ⇒ 通知永不更新。

放大器：WiFi 断开后进程默认路由走蜂窝，`pick_campus_source_ip` 排除蜂窝接口（`campus_detect.rs:15-37`；`rmnet*`/`ccmni*` 蜂窝判定在 `campus_detect.rs:17`）返回 None，`probe_campus` 的三判据（/18 子网 ∥ 校园网关 TCP ∥ Portal TCP，`campus_detect.rs:97-119`）全部不成立且 Portal HTTP 探测必失败 ⇒ `error_kind = Some("request_failed")`（变体构造见桌面 `tauri-app/src-tauri/src/auth/portal.rs:177`）恒成立 ⇒ 每拍都掉进 `_ => prev_online` 分支。而 `on_campus=false` 此前**不参与在线判定**（只用于自动重登门槛 `should_attempt_login` 的 `!on_campus` 短路 `monitor_loop.rs:104-105` 与掉线通知条件 `monitor_loop.rs:1361`），无法纠正。（2026-10-02 后续：该放大器已被两道短路结构性消除——① WiFi **彻底关闭**（无非蜂窝 IPv4 接口）由 `CampusProbe.wifi_off`（`campus_detect.rs:63`）建模，`probe_campus` 枚举网卡后立即短路返回（`campus_detect.rs:83-95`），不再空转 2×3s TCP 探测，通知走独立状态码 4「WiFi 未连接」（`notify_state`，`monitor_loop.rs:1475-1483`）；② `on_campus=false` 的拍子 Portal HTTP 探测同样短路（`monitor_loop.rs:1326-1330`），离网拍耗时 11s→3s。见 [[deliberate-background-check-skips]] 第 7 条。三态护栏本身不变：在线状态仍只在"确定判定"（error_kind=None）时翻转。）

## 修复

`monitor_loop.rs:1347-1351`（`run_check_once` 内，函数起于 `monitor_loop.rs:1245`）三态护栏（当前代码逐字）：

```rust
let online = match &portal {
    Ok(s) if s.error_kind.is_none() => s.online,
    _ if on_campus => prev_online,
    _ => false,
};
```

- `Ok + error_kind=None`：Portal 确定判定，取 `s.online`；
- 其余（已在线页面特征失配 / 探测超时 / 探测 Err）且 `on_campus=true`：校园网内探针失配，保持上一拍（反误报保留）；
- 其余且 `on_campus=false`：非校园网不存在"校园网在线"，判离线。

语义依据：`on_campus=false` 是**确定结论**而非"无结论"——三判据全否即"确定不在校园网"（蜂窝接口被排除后不存在误判源）。桌面同语义早已存在（`tauri-app/src-tauri/src/monitor/background_check.rs:90-96`，`campus_check_failed` 即置 `any_adapter_online=false`，并连带重置 `last_a1_online`/`has_logged_online`），本轮安卓补齐对齐。2026-09-09 反误报修复的适用范围被正确收窄到"校园网内探针失配"（`on_campus=true` 分支），真掉线由 `Determined(false)`（Portal 返回登录页）翻转的路径也不受影响。

## 教训与易踩点

- **"无结论就不翻转"的护栏必须回答：旧值会在什么情况下被推翻？** 三态护栏（Unknown/Failed 保持 `prev_online`）与"仅翻转时更新通知"（省电）组合起来，一旦出现"每拍都是 Unknown/Failed"的稳态场景，状态就被静默钉死，且通知面没有任何手段打破。任何状态机引入"保持旧值"分支时，要枚举"旧值永远不被推翻"的输入组合是否存在——本例中"离开校园网 + 探测必失败"正是这个组合。
- **"无结论"与"反向确定结论"要分开建模**：`error_kind=Some` 是"探测没做成"（无结论），`on_campus=false` 是"确定不在校园网"（反向结论）。把后者也塞进"无结论"分支，等于丢弃已知信息。
- **静默期整拍 `return` 同样会让通知陈旧**：`campus_check_start/end_minutes` 门外整拍跳过探测（时间判定 `monitor_loop.rs:1269-1275`），上一拍的"在线"通知会一直挂着到次日。修复同日补齐：静默期 return 前把常驻通知重建为「监控运行中 · 已暂停检测(非检测时段)」（`notified_online` 状态码 3；`monitor_loop.rs:1276-1288`，`swap(3)` 判重在 1281、`update_notification` 在 1284），跳过的是探测、不是状态保鲜。
- 排查此类问题先看**通知为什么不翻**而不是"为什么判在线"：状态记忆位（`was_online`，`monitor_loop.rs:35`）与通知记忆位（`notified_online`，`monitor_loop.rs:49`）是两级缓存，任一级不翻转都会冻结展示。

## 真机验证（2026-09-13，小米 25060RK16C/HyperOS）

- **无 VPN 环境**：关 WiFi 走蜂窝后约 **5 秒**通知翻转为「未连接」（当时事件时序：去抖 1s + 连上方向 `WIFI_EVENT_DELAY_MS=2500` 等路由稳定 + 探测耗时；现为 `monitor_loop.rs:61`）。此后断开方向改为零延迟 `WIFI_LOST_EVENT_DELAY_MS=0`（`monitor_loop.rs:66`，2026-09-14 用户要求断开感知 ≤1-2s），去抖窗口 `WIFI_EVENT_DEBOUNCE_MS=1000`（`monitor_loop.rs:69`）。此前版本在此场景被钉死为「在线」；2026-10-02 起关 WiFi 场景文案细化为「WiFi 未连接」（状态码 4，单测 `通知状态_wifi短路独立码` 锁定，`monitor_loop.rs:1492-1499`）。连回后恢复。
- **用户家中特殊拓扑（曾误诊为"VPN 打穿"）**：用户家 WiFi 路由器上游级联校园网（路由器 WAN 口 10.2.x.x 自己挂着 Dr.COM 会话），手机连该 WiFi 时 Portal 经路由器 NAT **真实可达**，且 Portal 返回的是路由器 WAN 会话的「已在线」页（`uid='…@cmcc'; v4ip='10.2.79.90'; oltime≈48h`）→ 判「在线」。这在"能上网"语义下是真实判定（手机确实免登录可上网），不是误报。教训：**可达性 = 物理拓扑（级联/NAT）**，不等于"本机在校园 /18 内"；「在线」的产品语义（能上网 vs 本机在校园网）是两条不同的判定线，改动前要先确认用户要哪条。
- **secure 全量 VPN 接管本应用时**（用户实测：开 Clash 不排除本应用则无法登录）：socket 类判定（网关/Portal TCP、Portal HTTP、登录请求）全部经隧道，判定与登录均不可用——这是平台边界（见 [[android-vpn-bypass-infeasible|不做"绕过 VPN 直连 WiFi"]]），唯一官方出口是 VPN 侧排除本应用。

## Connections

- [[android-backend]] — `MonitorState` 的 `was_online`（`monitor_loop.rs:35`）/`notified_online`（`monitor_loop.rs:49`）与 `run_check_once`（`monitor_loop.rs:1245`）的三态消费（`monitor_loop.rs:1347-1351`）
- [[background-check-and-auto-login]] — 两端在线判定与静默期语义对照
- [[android-keepalive-fgs-architecture]] — 前台服务常驻通知与"仅翻转时重建"的省电约束（重建点 `monitor_loop.rs:1276-1288`、`monitor_loop.rs:1459-1470`）
- [[android-vpn-bypass-infeasible|不做"绕过 VPN 直连 WiFi"]] — secure VPN 下判定不可信的平台边界与缓解路线
