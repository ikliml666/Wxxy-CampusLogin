---
title: 后台巡检中被主动跳过的逻辑清单（易被误认为缺陷，实为设计）
type: decision
source_files:
  - tauri-app/src-tauri/src/monitor/background_check.rs
  - tauri-app/src-tauri/src/monitor/campus_check.rs
  - tauri-app/src-tauri/src/monitor/adapter_watch.rs
  - tauri-app/src-tauri/src/monitor/auto_auth.rs
tags: [决策, 巡检, 监控, 有意为之]
---

## 背景

后台巡检/自动登录链路里有多处"看似该做却没做"的行为，容易被后来者当缺陷"修掉"。本文档以当前代码为准，记录这些**主动跳过**的逻辑及其设计理由。

## 决策

以下条目是**主动跳过**、有意为之：

1. **校园网名称检查关闭时只做网关 + portal 主机可达探测**
   `tauri-app/src-tauri/src/monitor/campus_check.rs:50-76`：当 `config.enable_network_name_check` 为 `false` 时，不执行 SSID/Profile 匹配，仅通过 `check_gateway_reachable` 探测 `campus_gateway` 与 `portal_probe_host(&config.portal_url)` 的 TCP 连通性。`on_campus = gateway_ok || portal_ok`。此时构造的 `CampusCheckResult` 中 `wifi`、`wired`、`current_ssid` 均为 `None`（`campus_check.rs:73-81`）。

2. **校园网检测静默期内跳过验证并强制 `on_campus=true`**
   `tauri-app/src-tauri/src/monitor/background_check.rs:49-73`：当 `is_campus_check_silent(now_min, start, end)` 返回 `true` 时，跳过校园网环境验证，调用 `cancel_campus_exit`，并构造 `on_campus=true` 的结果（`background_check.rs:66-73`）。
   *理由*：静默期跳过的是**探测**，不是通知面的状态保鲜。否则上一拍遗留的「在线」常驻通知会在非检测时段一直展示到次日。桌面端在静默期仍继续走完整 Portal 探测流程（仅跳过 campus 验证），而安卓侧整拍短路（见第 7 条）。

3. **校园网不通过但主副适配器均无 IP 时不退出、等待网络恢复**
   `tauri-app/src-tauri/src/monitor/background_check.rs:103-111`：若配置的适配器均无 IP（`no_configured_ip`），则跳过 `start_campus_exit`，等待网络恢复。同理见于 `tauri-app/src-tauri/src/monitor/auto_auth.rs:204-212`（开机自启登录流程中的校园网检测未通过分支）。
   *理由*：完全无网络时触发退出没有意义，应等待网络恢复后再重新检测。

4. **后台巡检不再触发全量质量检测**
   `tauri-app/src-tauri/src/monitor/background_check.rs:223-226`（文档注释）：质量检测由 `latency.rs` 的 `spawn_latency_test_loop → quality_scheduler::run_quality_check` 独占。避免双定时器叠加触发全量外网检测、60s 节流吞掉用户自定义间隔。

5. **"自动检测"模式不参与自动启用**
   `tauri-app/src-tauri/src/monitor/adapter_watch.rs:168-169`（注释）与 `tauri-app/src-tauri/src/network/adapter.rs:53-64`（`configured_disabled_adapters`）："自动检测"模式（适配器名为空或哨兵值 `AUTO_DETECT_ADAPTER`）不参与自动启用。过滤逻辑要求 manual1/manual2 判定为非空且 ≠ `AUTO_DETECT_ADAPTER`。

6. **自动启用首次尝试静默提权不弹 UAC**
   `tauri-app/src-tauri/src/monitor/adapter_watch.rs:231-234`：首次尝试保持静默（`allow_uac = false`），兼容 CMSTPLUA 静默提权可用的环境零打扰；失败计数 ≥1 后才允许弹 UAC 降级（`allow_uac = true`）。
   *理由*：CMSTPLUA 已被系统封堵的环境（0x80080017「未将类配置为支持提升的激活」）下静默提权不可得，UAC 是唯一恢复通道。频率由退避阶梯限制（60s→120s→300s 封顶，`adapter_watch.rs:290-299` 的 `auto_enable_backoff_ms`）。
   *额外过滤*：
   - 夜间出站切换亲手禁用的卡不自动启用（名单过滤 `adapter_watch.rs:206-212`）。
   - 夜间守护窗（时间窗 ∧ 切换态）内一律不出自动启用目标（`adapter_watch.rs:186-194,213`）。

7. **（安卓，2026-10-02 起）WiFi 关闭时整拍短路**
   *注：安卓侧源码未在当前输入中提供行号，但逻辑已在上文描述中记录。* WiFi 关闭（`pick_campus_source_ip` 无非蜂窝 IPv4 接口）时，`probe_campus` 枚举网卡后直接返回全 false 不做网关/Portal TCP 探测。`run_check_once` 的 Portal 探测短路为 Err。状态与通知走 `campus_status_message`「WiFi 未连接」与 `notify_state` 的 wifi_off 分支。
   *理由*：WiFi 关闭不是「校园网探测失败」，没有可探测的对端。

8. **（安卓，2026-10-02 起）非校园网时跳过 Portal HTTP 探测**
   *注：安卓侧源码未在当前输入中提供行号，但逻辑已在上文描述中记录。* 非校园网（`on_campus=false`）时跳过 Portal HTTP 探测。`on_campus=false` ⇔ 子网未命中且网关/Portal 两个 3s TCP 探测已并行执行且全败，同一目标 HTTP 请求必败，不再空转至多 8s。离网拍最坏 11s→3s。校内 Portal 宕机（on_campus=true）不在此列，仍由 HTTP 页面请求裁决在线与否。

## 理由

旧文档未记录逐条理由（第 2 条"静默期"与第 3 条"等网络恢复"属有意设计的判断来自 wiki 模块文章的显式声明；第 6 条的静默/UAC 降级理由见 `adapter_watch.rs:231-234` 行内注释）。

## 备选方案

旧文档未记录。

## 影响与约束

改动这些分支前先确认它们是设计而非缺陷。已知代价：静默期语义两端不同（桌面构造 `on_campus: true` 并继续走完整 Portal 探测，安卓整拍 `return` 不探测），非在校时段的**在线状态本身**两端都保持上一拍、新鲜度不可比；但 2026-09-13 起安卓的**通知面**不再继承陈旧在线——静默期进入即把常驻通知重建为「已暂停检测(非检测时段)」，用户看到的不是过期状态。

## Connections

[[quality-check-single-driver]]、[[mask-placeholder-persisted-as-plaintext]]