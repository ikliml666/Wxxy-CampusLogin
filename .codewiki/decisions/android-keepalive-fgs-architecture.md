---
title: 安卓省电调研定调：巡检架构维持 Kotlin FGS 保活 + Rust tokio 循环
type: decision
tags: [决策, 安卓, 省电, 前台服务, 保活]
source_files:
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/battery_cmds.rs
  - android/plugins/foreground-service/src/lib.rs
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/ForegroundService.kt
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt
  - android/frontend/src/settings/KeepAliveSettingsCard.tsx
  - android/src-tauri/gen/android/app/src/main/AndroidManifest.xml
---

## 背景

巡检需要 60s 级别的周期检测，且必须在高厂商定制的国产 ROM（澎湃OS/ColorOS/EMUI 等）后台存活。Tauri v2 官方明确不支持后台任务（tauri-apps discussion #14615），桌面端有 system tray + 事件循环兜底，移动端必须另起炉灶。省电调研结论：保活架构维持「Kotlin 前台服务（FGS）保活 + Rust tokio 循环跑检测」，省电做在细节。

## 决策

**2026-09-12 维持「Kotlin FGS 保活 + Rust tokio 循环」，两道省电闸：**

- **nudge 唤醒锁按关注字段翻转去重**（`ForegroundService.kt:256-263`）：`onCapabilitiesChanged` 只在 `lastFieldMask`（bit0=TRANSPORT_WIFI、bit1=TRANSPORT_CELLULAR、bit2=NET_CAPABILITY_VALIDATED，`:233`）翻转时才 nudge；信号强度波动每秒多条连发被全部滤掉，`onLost` 重置 mask=-1（`:252-254`）。与 network-bind 插件 watcher 同款去重。
- **5s 节流降为第二道闸**（`NUDGE_THROTTLE_MS=5000`，`:46`）：穿透去重的 nudge 每 5s 最多一次，每次持 `NUDGE_WAKE_MS=3000`（`:49`）的 PARTIAL_WAKE_LOCK（`campus:nudge`，`:243-246`）抑制 suspend。
- **常驻通知仅状态翻转才 notify**（`monitor_loop.rs:1472-1477`）：Rust 侧 `notified_online` 状态码表 `:49`（0=未展示、1=在线、2=未连接、4=WiFi 未连接——码 4 为 2026-10-02 新增的 WiFi 短路独立态，短路判定 `:1337`；非检测时段首次进入置 3 并 notify `:1289-1293`），`notify_state` 纯函数 `:1483-1491`。文案固定「监控运行中 · {状态}」，不带逐拍检测次数。

**2026-09-13 补环——锁改探针窗口按需持有**（不再常驻）：Rust 每拍检测前 `begin_probe_window` + `ProbeWindowGuard` Drop 兜底（含 panic unwind，`monitor_loop.rs:553-562`，进入点 `:1302-1306`）；插件命令 `beginProbeWindow`/`endProbeWindow`（`MonitorServicePlugin.kt:110-119`）直调服务静态入口 `ForegroundService.kt:62/:65`（同进程，绕开 Android 8+ 后台 startService 限制）；`acquireProbeLocks`（`:179-200`）懒建 WifiLock `WIFI_MODE_FULL_HIGH_PERF`（`campus:wifi`）+ PARTIAL_WAKE_LOCK（`campus:probe`，30s 超时 `PROBE_WINDOW_TIMEOUT_MS` `:52` 防泄漏），`releaseProbeLocks` `:204-214` 幂等。

**2026-10-01 补环——常驻通知 12h 定时刷新**：Rust 只在翻转时 notify，通知可数天不更新，澎湃OS 通知过滤按点击率/陈旧度折叠；服务侧 `refreshTick`（`ForegroundService.kt:150-157`）每 `REFRESH_INTERVAL_MS=12h`（`:93`）拨新 Chronometer 起点并复用 `lastText` 重建。`buildNotification`（`:103-121`）走标准安卓协议（ongoing+setOnlyAlertOnce+CATEGORY_SERVICE+setUsesChronometer，2026-09-08 用户决策不做厂商私有 extras，兼作 Android 16 promoted ongoing 适配），点击回应用 `mainActivityIntent`（`:128-136`，历史点击率是澎湃OS 通知过滤评分关键因子）。

**2026-09-13 补第四环——电池优化白名单**：命令面四条（`battery_cmds.rs`）经 foreground-service 插件转发 Kotlin：`get_battery_optimization_info`（`:20`）、`request_ignore_battery_optimizations`（`:46`）、`open_vendor_battery_settings`（`:65`）、`open_notification_settings`（`:87`，2026-09-21 通知权限引导落地时补入，消费方 `notificationPermission.ts:16`，与保活无直接关系但同属系统设置页跳转命令面）。Manifest 声明 `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`（`AndroidManifest.xml:11-14`，注释声明一次性引导跳转非静默加入）。前端 `KeepAliveSettingsCard.tsx`：白名单徽标+一键申请（先自家 ConfirmDialog `:130-136` 再系统确认框）+厂商页跳转，首次进入只提示一次（`campus-keepalive-prompted`，`:23`，提示逻辑 `:40-47`），申请返回后延迟 1.5s 重查（`:59`），中英双语 settings.keepAlive*。

## 理由

- Tauri 官方口径（tauri-apps discussion #14615）：v2 无稳定后台任务方案；Rust 循环死进程即死，Kotlin FGS 是唯一能长期持锁的宿主。FGS 用 `specialUse` 类型（`startForegroundWithText` `:317-338`，Manifest `:46-54` subtype=`campus-network-monitor-keepalive` `:51-53`）而非 dataSync，无 6h/24h 运行上限；`START_STICKY`（`onStartCommand` `:303`）+ 服务死即循环死。
- WorkManager 最小周期 15min，覆盖不了 60s 检测。
- 电池白名单是标准 API，成本最低的补充；锁从常驻改探针窗口后，WifiLock/WakeLock 全周期占空比从 100% 降到每拍数百毫秒，是省电收益最大的一环。

## 备选方案

- WorkManager：弃用，最小 15min 周期。
- 保活对抗类手法（无声音乐/1px Activity/双进程）：明确不用。
- 电池白名单静默加入：弃用，商店审核要求用户主动申请；前端延迟 1.5s 重查状态。
- 厂商页 resolveActivity 预探测：弃用（Android 11+ 包可见性导致误判「无厂商页」），改逐个 startActivity+catch 试下一个（`MonitorServicePlugin.kt:207-212` 注释）。`vendorTargets` 10 品牌候选表（`:156-197`，xiaomi/redmi/huawei/honor/oppo/realme/oneplus/vivo/iqoo/samsung），降级链 `openVendorBatterySettings`（`:252-305`）：厂商专用页（`:256-266`）→魅族 SHOW_APPSEC 受保护 action（`:269-283`）→应用详情页（`:285-296`）→系统设置（`:298-304`），返回实际落点 path/target/tried。

**未实施备查**：质量循环稳态退避（12+ 外网目标/60s 为最大功耗主力）。~~WifiLock FULL_HIGH_PERF 降级~~已被探针窗口化取代（2026-09-13）。

## 影响与约束

- 保活对抗手法不得引入（无声音乐/1px Activity/双进程等）。
- 电池白名单属平台专属能力，是双端同步铁律的例外条款。
- 厂商跳转只走标准 Intent+降级链，不写私有 extras（2026-09-08 用户决策）。
- 省电只能落在减少唤醒/减少派发：翻转去重、节流、探针窗口、分档间隔、常驻通知按需 notify。

## Connections

- [[android-power-three-fixes]]
- [[rAF-blocks-compositor-idle]]
- [[android-persistent-notification-standard]]
