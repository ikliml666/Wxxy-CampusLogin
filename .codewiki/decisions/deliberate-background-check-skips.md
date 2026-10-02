---
title: 后台巡检中被主动跳过的逻辑清单（易被误认为缺陷，实为设计）
type: decision
source_files:
  - tauri-app/src-tauri/src/monitor/background_check.rs
  - tauri-app/src-tauri/src/monitor/campus_check.rs
  - tauri-app/src-tauri/src/monitor/adapter_watch.rs
tags: [决策, 巡检, 监控, 有意为之]
---

## 背景

后台巡检/自动登录链路里有多处"看似该做却没做"的行为，容易被后来者当缺陷"修掉"。

## 决策

以下八条是**主动跳过**、有意为之：

1. 校园网名称检查关闭时只做网关探测、`wifi`/`wired`/`current_ssid` 全 `None`（`monitor/campus_check.rs:37-57`）；
2. 校园网检测静默期内跳过验证并强制 `on_campus=true`，同时 `cancel_campus_exit`（`monitor/background_check.rs:46-70`）。2026-09-13 起安卓侧静默期仍整拍跳过探测（`monitor_loop.rs:695-715`），但 return 前会同步一次常驻通知（「监控运行中 · 已暂停检测(非检测时段)」，`notified_online` 状态码 3，`monitor_loop.rs:702-712`）——静默期跳过的是**探测**，不是通知面的状态保鲜；否则上一拍遗留的「在线」常驻通知会在非检测时段一直展示到次日；
3. 校园网不通过但主副适配器均无 IP 时不退出、等待网络恢复（`monitor/background_check.rs:124-126`、`monitor/auto_auth.rs:322-324`）；
4. 后台巡检不再触发全量质量检测（2026-09-04 收敛）（`monitor/background_check.rs:337-339`）；
5. "自动检测"模式（适配器名为空或哨兵值）不参与自动启用（`monitor/adapter_watch.rs:145-151`，过滤在 `network/adapter.rs:53`）；
6. 提权自动启用不弹 UAC（`enable_adapter(..., false)`）（`monitor/adapter_watch.rs:173`）；
7. （安卓，2026-10-02 起）WiFi 关闭（`pick_campus_source_ip` 无非蜂窝 IPv4 接口）时整拍短路：`probe_campus` 枚举网卡后直接返回全 false 不做网关/Portal TCP 探测，`run_check_once` 的 Portal 探测短路为 Err（`campus_detect.rs:83-95`、`monitor_loop.rs:1319-1329`）——WiFi 关闭不是「校园网探测失败」，没有可探测的对端；状态与通知走 `campus_status_message`/`notify_state` 的 wifi_off 分支（「WiFi 未连接」，通知状态码 4）；
8. （安卓，2026-10-02 起）非校园网（`on_campus=false`）时跳过 Portal HTTP 探测（`monitor_loop.rs:1319-1329`）：`on_campus=false` ⇔ 子网未命中且网关/Portal 两个 3s TCP 探测已并行执行且全败（子网命中强制 gateway_ok=true、SSID 命中早退 on_campus=true，均不会落此分支），同一目标 HTTP 请求必败，不再空转至多 8s（`portal_config::CLIENT_TIMEOUT`）——离网拍最坏 11s→3s。校内 Portal 宕机（on_campus=true）不在此列，仍由 HTTP 页面请求裁决在线与否。

## 理由

旧文档未记录逐条理由（第 2 条"静默期"与第 3 条"等网络恢复"属有意设计的判断来自 wiki 模块文章的显式声明）。

## 备选方案

旧文档未记录。

## 影响与约束

改动这些分支前先确认它们是设计而非缺陷。已知代价：静默期语义两端不同（桌面构造 `on_campus: true` 并继续走完整 Portal 探测，安卓整拍 `return` 不探测），非在校时段的**在线状态本身**两端都保持上一拍、新鲜度不可比；但 2026-09-13 起安卓的**通知面**不再继承陈旧在线——静默期进入即把常驻通知重建为「已暂停检测(非检测时段)」（`monitor_loop.rs:702-712`），用户看到的不是过期状态。

## Connections

[[quality-check-single-driver]]、[[mask-placeholder-persisted-as-plaintext]]
