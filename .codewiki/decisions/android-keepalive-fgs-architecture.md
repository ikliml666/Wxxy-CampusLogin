---
title: 安卓省电调研定调：巡检架构维持 Kotlin FGS 保活 + Rust tokio 循环
type: decision
source_files:
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/battery_cmds.rs
  - android/plugins/foreground-service/src/lib.rs
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/ForegroundService.kt
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt
  - android/frontend/src/settings/KeepAliveSettingsCard.tsx
  - android/src-tauri/gen/android/app/src/main/AndroidManifest.xml
tags: [决策, 安卓, 省电, 前台服务, 保活]
---

## 背景

后台巡检要长期存活，同时要控制耗电，需要决定保活架构与省电落点。

## 决策

2026-09-12：巡检架构**维持**"Kotlin FGS 保活 + Rust tokio 循环"，省电做在细节：

- 服务侧 `NetworkCallback` nudge 唤醒锁按**关注字段翻转**去重（`ForegroundService.kt:198-206`：TRANSPORT_WIFI / TRANSPORT_CELLULAR / NET_CAPABILITY_VALIDATED 位掩码不变则不 nudge，与 network-bind 插件 watcher 同款），**5s 节流**降为第二道闸（`onCapabilitiesChanged` 信号强度波动时每秒多条连发，每次持 3s 锁抑制 suspend）；
- 常驻通知**仅在线状态翻转时** notify（原每拍重建，文案不再带逐拍检测次数）。

2026-09-13 补第四环——**电池优化白名单**（此前列为"未实施备查"，本轮落地）：

- 命令面三条 `battery_cmds.rs`（`get_battery_optimization_info` / `request_ignore_battery_optimizations` / `open_vendor_battery_settings`，`:19` / `:45` / `:64`），经 foreground-service 插件转发 Kotlin；`AndroidManifest.xml` 声明 `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`（一次性引导跳转，非静默加入）；
- 设置页保活区块 `KeepAliveSettingsCard.tsx`：白名单状态徽标 + 一键申请（先弹自家 ConfirmDialog 再跳系统确认框）+ 厂商自启/省电页跳转，首次进入只提示一次（`campus-keepalive-prompted`）；中英双语文案 `settings.keepAlive*`。

## 理由

Tauri 官方口径（tauri-apps discussion #14615）：后台常驻只能靠前台服务，且 WebView/后端进程随时可被杀。本项目 FGS `specialUse` 类型（非 `dataSync`，无 6h/24h 上限）+ `START_STICKY` + 服务死即循环死的架构已被官方口径验证为正确。WorkManager 最小周期 15min，覆盖不了 60s 检测。

2026-09-13（白名单一环）：FGS 保活只能防"服务被回收"，防不了厂商 ROM 的息屏省电策略；电池白名单是标准 API、成本最低的补充手段。

## 备选方案

- 迁移 WorkManager —— 最小周期 15min 覆盖不了 60s 检测，弃用；
- 保活对抗类手法（无声音乐 / 1px Activity / 双进程）—— **明确不用**：解决"不被杀"却增加耗电，与目标相反；
- 电池白名单静默加入 —— **弃用**：应用商店对直接请求 `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` 有审核要求，必须有明确的用户确认步骤（前端先弹自家说明框再调命令），申请结果由系统 UI 决定、前端延迟 1.5s 重查状态刷新；
- 厂商页 `resolveActivity` 预探测 —— **弃用**：Android 11+ 包可见性限制会误判页面不存在而直接降级，改逐个 `startActivity` + `catch` 试下一个（`MonitorServicePlugin.kt:156-212`，10 品牌候选表 + 应用详情页/系统设置两级降级，返回实际落点）。

**未实施备查**：质量循环稳态退避（12+ 外网目标/60s 为最大功耗主力）、`WifiLock FULL_HIGH_PERF` 降级。

## 影响与约束

保活对抗类手法不得引入。电池白名单属**平台专属能力**（桌面无对应 API，不参与双端同步，是双端同步铁律的例外条款），厂商跳转只能走标准 Intent + 降级链，不得写厂商私有 extras。省电优化只能落在"减少唤醒/减少派发"方向。

## Connections

[[android-power-three-fixes]]、[[rAF-blocks-compositor-idle]]、[[android-persistent-notification-standard]]
