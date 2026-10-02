---
title: 代码库模式集
type: patterns
tags: [模式, 约定]
source_files:
  - tauri-app/src-tauri/src/lib.rs
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/app/heartbeat.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
  - tauri-app/src-tauri/src/commands/login.rs
  - tauri-app/src-tauri/src/auth/portal.rs
  - tauri-app/src-tauri/src/auth/protocol.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/persist.rs
  - tauri-app/src-tauri/src/infra/events.rs
  - tauri-app/src-tauri/src/infra/command_context.rs
  - tauri-app/src-tauri/src/infra/lifecycle.rs
  - tauri-app/src-tauri/src/infra/task_manager.rs
  - tauri-app/src-tauri/src/infra/state/mod.rs
  - tauri-app/src-tauri/src/infra/state/store.rs
  - tauri-app/src-tauri/src/infra/state/network.rs
  - tauri-app/src-tauri/src/monitor/background_task.rs
  - tauri-app/src-tauri/src/monitor/background_emit.rs
  - tauri-app/src-tauri/src/monitor/adapter_watch.rs
  - tauri-app/src-tauri/src/monitor/latency.rs
  - tauri-app/src-tauri/src/monitor/quality_scheduler.rs
  - tauri-app/src-tauri/src/monitor/portal_check.rs
  - tauri-app/src-tauri/src/monitor/portal_watch.rs
  - tauri-app/src-tauri/src/monitor/watcher.rs
  - tauri-app/src-tauri/src/monitor/auto_auth.rs
  - tauri-app/src-tauri/src/network/client.rs
  - tauri-app/src-tauri/src/network/adapter_cache.rs
  - tauri-app/src-tauri/src/network/mod.rs
  - tauri-app/src-tauri/src/account/crypto.rs
  - tauri-app/src-tauri/src/update/updater.rs
  - tauri-app/src-tauri/src/helper/mod.rs
  - tauri-app/src-tauri/src/self_service/mod.rs
  - tauri-app/src-tauri/src/platform/mod.rs
  - tauri-app/frontend/src/hooks/useAppStore.ts
  - tauri-app/frontend/src/hooks/useAuthStore.ts
  - tauri-app/frontend/src/hooks/useConfigStore.ts
  - tauri-app/frontend/src/hooks/useQualityStore.ts
  - tauri-app/frontend/src/hooks/useAdapterStore.ts
  - tauri-app/frontend/src/hooks/useAnimationProfile.ts
  - tauri-app/frontend/src/hooks/useEventListeners.ts
  - tauri-app/frontend/src/hooks/tauriApi.ts
  - tauri-app/frontend/src/lib/renderLiveness.ts
  - tauri-app/frontend/src/monitor/LatencyComponents.tsx
  - tauri-app/frontend/src/shared/FluidBackground.tsx
  - tauri-app/frontend/src/shared/AnimatedNumber.tsx
  - tauri-app/frontend/src/components/ui/animated-card.tsx
  - tauri-app/frontend/src/index.css
  - android/src-tauri/Cargo.toml
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/account_cmds.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/update_cmds.rs
  - android/src-tauri/capabilities/default.json
  - android/frontend/src/hooks/useDeviceProfile.ts
  - android/frontend/src/hooks/useAdaptiveFramePace.ts
  - android/frontend/src/hooks/useQualityStore.ts
  - android/frontend/src/hooks/useAdapterStore.ts
  - android/frontend/src/hooks/useAuthStore.ts
  - android/frontend/src/lib/renderLiveness.ts
  - android/frontend/src/main.tsx
  - android/plugins/foreground-service/src/lib.rs
  - android/plugins/foreground-service/build.rs
  - android/plugins/foreground-service/permissions/default.toml
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/ForegroundService.kt
  - android/plugins/network-bind/build.rs
  - android/plugins/network-bind/android/src/main/java/com/campuslogin/plugin/networkbind/NetworkBindPlugin.kt
---

# 代码库模式集

## Overview

本仓库是双端校园网认证客户端：`tauri-app/`（Windows 桌面，Tauri 2 + React）与 `android/`（安卓，Tauri 2 移动端 + 自研插件），两端共用 `campus-login` 协议 crate（path 依赖，见 `android/src-tauri/Cargo.toml:34`）。本文按**当前代码实读验证的行号**梳理 11 条核心模式与 14 条反模式/偏差；行号以文件当前版本为准，改动后需同步刷新。

## 模式清单

### 1. 领域 store 拆分与兼容壳（桌面）

`tauri-app/frontend/src/hooks/useAppStore.ts:1-3` 仅是兼容壳：`:1` 注释「已拆分为领域 store」，`:2` re-export `useAppInit`，`:3` re-export `hasPendingConfig`/`flushPendingConfig`（来自 useConfigStore）。状态按领域拆分：useAuthStore（登录态/在线状态）、useConfigStore（配置与账号）、useQualityStore（网络质量/更新/DNS）、useAdapterStore（适配器）、useThemeStore、useLogToastStore。手动刷新锁各自持模块级布尔：`useAdapterStore.ts:9` `_adapterLockFlag`、`useQualityStore.ts:12` `_qualityLockFlag`；释放策略同仓两制，见偏差 4。

### 2. IPC 命令面双端同名对齐

两端 invoke_handler 条目一一对应同名命令：桌面 `tauri-app/src-tauri/src/app/startup.rs:62-128` 共 **65 项**（条目 `:63-127`），安卓 `android/src-tauri/src/lib.rs:57-111` 共 **53 项**（条目 `:58-110`，桌面多出托盘/更新/自服务/DNS 等桌面专属命令）。前端类型面集中在 `tauri-app/frontend/src/hooks/tauriApi.ts:27-116`（exportConfig `:31`、importConfig `:33`、exportDiagnostics `:35`、renameAccount `:64-65`、outboundSwitchNow/RestoreNow `:74-77`、checkDnsDohStatus/setupDnsDoh/resetDns `:108-110`、renderHeartbeat `:111`）；重试包装仅包 saveConfig（`tauriApiWithRetry` `:280-286`，指数退避+抖动 `:262-278`）。

### 3. 协议单点共享 + cfg 门控

业务协议只在 `campus-login` crate 实现一份（`android/src-tauri/Cargo.toml:34` path 依赖；安卓插件注释亦声明「业务逻辑全在 Rust 侧 monitor_loop(协议不双实现)」，`android/plugins/foreground-service/src/lib.rs:3`）。平台差异用 `cfg` 门控收敛在模块边界：

- `tauri-app/src-tauri/src/platform/mod.rs:1-36`：无门控 `console_output` `:2`；`#[cfg(desktop)]` autostart/ecoqos/dns_config/elevation/gpu/helper_spawn/identity `:5-17`；`#[cfg(all(desktop,target_os="windows"))]` task_proxy/metric/best_route/icmp_probe/rtss_compat/toast `:19-35`。
- `tauri-app/src-tauri/src/network/mod.rs:16-28`：windows-only `dhcp_release_renew_all` `:22-23`，非 Windows 桩 `:25-26`。
- `tauri-app/src-tauri/src/account/crypto.rs:124-141`：Windows DPAPI 加解密 `:124-130`，非 Windows 桩返回「加密存储仅桌面端支持」`:133-141`。

### 4. 状态用 ArcSwap/CAS 快照

配置与网络状态都是「整体快照 + CAS 替换」：`ConfigStore` 基于 ArcSwap（`infra/state/store.rs:6-8`），`update()` CAS 循环 `:35-49`（compare_and_swap + `Arc::ptr_eq` 确认 `:44-45`）。`NetworkState`/`NetworkSnapshot`（`infra/state/network.rs:7-28`，last_network_quality/current_ssid 用 `Arc<str>` `:18-19`、prep_login_failures `:27`）经 `update()` `:78-92` 与 `update_with_result()` `:98-113` 替换；巡检计数自增同样走 CAS（`monitor/background_emit.rs:116-119` 调 update_with_result）。UpdateStats 字段定义在 `infra/state/mod.rs:125-140`。

### 5. 后台任务统一 BackgroundTaskManager + CancellationToken

所有长生命周期后台任务走 `infra/task_manager.rs`：`BackgroundTaskManager` `:15-17`，`spawn` `:33-64`（同名拒绝 `:40-42`；任务结束自动 remove 且 `ptr_eq` 防误删新实例 `:53-55`），`cancel` `:67-75`、`detach` `:81-83`、`shutdown` `:86-98`、`is_running` `:101-103`。当前 15 个命名任务：

heartbeat（`app/heartbeat.rs:11`）、window_safety（`app/heartbeat.rs:68`）、campus_exit（`infra/lifecycle.rs:78`）、auto_exit（`infra/lifecycle.rs:228`）、update_check_loop（`update/updater.rs:312`）、startup_bg_check / startup_latency / startup_auto_login / scheduled_actions（`monitor/watcher.rs:22`/`:34`/`:45`/`:54`）、portal_watch（`monitor/portal_watch.rs:59`）、latency_test（`monitor/latency.rs:55`）、background_check（`monitor/background_task.rs:16`）、adapter_cache_refresh（`network/adapter_cache.rs:267`）、adapter_watch（`monitor/adapter_watch.rs:19`）、auto_login_on_start（`monitor/auto_auth.rs:236`）。例外清单见偏差 5。

### 6. 事件统一走 EventBus（桌面）

`infra/events.rs:8-10` 定义 EventBus，唯一底层 `app_handle.emit` 调用在 `:18`；16 个 `emit_*` 方法 `:22-119`（login_log `:22`、background_check_result `:30`、auto_login_result `:35`、network_quality_result `:44`、update_available `:49`、update_notification_click `:58`、adapter_details_changed `:63`、disabled_adapters_changed `:68`、adapter_disabled_warning `:73`、campus_exit_countdown `:81`、campus_exit_cancelled `:89`、auto_exit_countdown `:94`、auto_exit_cancelled `:102`、config_changed `:107`、adapters_changed `:112`、update_download_progress `:117`）——新事件追加在 `:119` 之后。命令侧出口经 `AppHandleExt`（`infra/command_context.rs:38-51`）；质量结果推送在 `network/quality.rs:511`/`:591`/`:645` 与 `monitor/quality_scheduler.rs:41`。安卓侧无此封装，见偏差 1。

### 7. 敏感信息出站唯一出口 masked_for_display

约定注释在 `config/model.rs:312-314`，实现 `masked_for_display` `:315-319` / `mask_in_place` `:321-328`（PASSWORD_MASK 常量 `:3`）。所有跨边界返回必须先掩码：`commands/config_cmd.rs:17`（emit config-changed 前，事件体必须 `{ config: ... }` 包裹）、`:103`（导出载荷 build_config_export_payload，掩码态或 DPAPI 密文态 + passwordEncrypted 标志 `:91-113`）、`:217`（get_config）；`commands/system.rs:117`/`:240`；`commands/account.rs:28`/`:181`/`:252`/`:309`。安卓侧同约定：`android/src-tauri/src/config_state.rs:372-382` + get_config `:417` + 广播 `:390`。导入侧密码还原语义集中在 `commands/config_cmd.rs:121-131`（空/MASK 保留当前值，密文 decrypt，失败明确报错）。测试护栏：`config_cmd.rs:287-334`、`config_state.rs:726-736`。

### 8. 配置原子写

桌面统一走 `config/persist.rs:12-43` `atomic_write`：纳秒时间戳临时名 `:14-20`、BufWriter+flush `:26-29`、`sync_all()` `:31`、rename 失败重试 3×100ms `:32-39`、失败清理 `:40-42`。四个调用点：save_account_config `:111`、append_login_history `:209`、append_quality_history `:271`、save_config_to_disk_encrypted `:290`。登录历史经 `LOGIN_HISTORY_LOCK`（`:10`）串行化（append_login_history `:160-212`）；质量历史头插 100 条上限、负延迟落 null（`:223-274`）。安卓侧 `save_file`（`android/src-tauri/src/config_state.rs:252-283`）仅 tmp+rename（`:279-281`），无 fsync、无 CONFIG_IO_LOCK，见偏差 2。

### 9. 动画与性能分级恒最高档

v2.4.0 起删除 standard/economy 分档，所有设备恒按最高档供帧：桌面 `useAnimationProfile.ts:28-30` 注释 + `HIGH_PROFILE` `:31-49`（17 字段接口 `:8-26`，easing=getEasingConfig(60) `:47`、refreshRate:60 `:48`）；安卓 `useDeviceProfile.ts:1-4` 折叠注释 + `HIGH_PROFILE` `:17-23`（idleFps:30 / activeFps:60）+ `:25-27` 恒返旗舰档。全局动效基线在 `android/frontend/src/main.tsx:17-19`（gsap.defaults expo.out、autoSleep:5 `:18`、lagSmoothing）；idle 冻结 utilities 在 `tauri-app/frontend/src/index.css:713-716`（`.anim-idle` 暂停 pulse/glow，spinner 例外注释 `:711-712`）。

### 10. 按需驱动替代常驻驱动

渲染存活判定与帧率驱动都不允许常驻 rAF（功耗事故的根因，见偏差 13）：

- `renderLiveness.ts:1-13` 注释 + `PROBE_REFRESH_MS=4_000` `:18`、`PROBE_TIMEOUT_MS=500` `:20`、`RENDER_STALL_THRESHOLD_MS=10_000` `:21`；`startProbe` 开 2 帧 rAF 窗口 `:24-34`；`isRenderLoopAlive` `:36-40`（document.hidden 短路 `:37`）。桌面与安卓两份同构（`android/frontend/src/lib/renderLiveness.ts` 同为 40 行）。
- `useAdaptiveFramePace.ts:1-10` 注释 + `PACE_POLL_MS=250` `:18`：`markInteraction` `:26-29`、`currentPace` `:31-35`、`applyPace`（`gsap.ticker.fps` `:44`）、静止档 setInterval 轮询 `:64`。
- 安卓探针窗口锁链路：`android/src-tauri/src/monitor_loop.rs` `ProbeWindowGuard` `:553-562`（Drop 保证释放）→ `run_check_once` 持锁窗口 `:1293-1298` → Rust 插件 `begin/end_probe_window`（`android/plugins/foreground-service/src/lib.rs:47-54`）→ Kotlin `ForegroundService.kt:54-58` + `acquireProbeLocks` `:172-195`（WifiLock `WIFI_MODE_FULL_HIGH_PERF` `:176`、PARTIAL_WAKE_LOCK `campus:probe` `:189`）/ `releaseProbeLocks` `:197-207`。
- 巡检分档：`effective_interval_ms`（`monitor_loop.rs:83-90`，亮屏+WiFi → base.max(5000)，否则 idle_ms.max(base)）；常驻通知仅在状态翻转时 update_notification（`monitor_loop.rs:1461-1470`）。

### 11. 后台检测定时循环与节流

桌面：background_check 循环 `monitor/background_task.rs:29-53`（每轮重读配置 `:30-38`、轻量化期 effective_background_interval_ms 下限 300s `:34-37`、退出判据 is_running("background_check")+is_quitting `:49`）；latency_test 循环 `monitor/latency.rs:53-123`（`spawn("latency_test")` `:55`、每轮重读间隔 + effective_quality_interval_ms `:66-82`、退出判据 `:90-94`、就绪等待 2s 短重试 `:100-111`）；poor/bad 升档复核门控 `monitor/quality_scheduler.rs:12-13`（SPIKE_CONFIRM_COUNT=2 / SPIKE_CONFIRM_INTERVAL_SECS=15，注释 `:9-11`）。安卓：monitor_tick_loop（`monitor_loop.rs:580-615`，interval 下限 5000 `:582`、每拍 run_scheduled_actions `:593`、间隔热更新重建计时器 `:596-601`、分档跳拍 `:604-610`）。通知节流：后台检测变更通知 60s（`monitor/background_emit.rs:99-100`，Acquire/Release）、适配器禁用通知 60s（`monitor/adapter_watch.rs:124-135`，比较 `:129`、store Relaxed `:135`）；安卓自动登录有 60s 注销保护期（`monitor_loop.rs:43`）+ 连续失败熔断 5 次（判据 `:1373`，自增 `:1407`/`:1414`）。

## 反模式与已发现的偏差

1. **安卓 6 处裸 `app.emit` 绕过 EventBus**（桌面有 EventBus，安卓无对应封装）：`android/src-tauri/src/monitor_loop.rs:155`（login-log）、`:498`（auto-login-result）、`:1457`（background-check-result）、`android/src-tauri/src/config_state.rs:390`（config-changed）、`android/src-tauri/src/update_cmds.rs:323`（update-available）、`:444`（update-download-progress）。事件名与前端监听对齐，但出口分散在 4 个文件，改事件名/载荷需全局搜索。
2. **安卓 save_config 与夜切落盘不持 CONFIG_IO_LOCK**：锁定义 `account_cmds.rs:17`、`config_io_lock()` `:21-23`；持锁方 switch_account `:177`、save_current_as_account `:221`、delete_account `:287`、rename_account `:327`、set_boot_autostart（`monitor_loop.rs:369`）、set_notification_enabled（`monitor_loop.rs:393`）；**不持锁**：config_state.rs save_config `:421-453`、monitor_loop.rs persist_settings `:955-978`（save_to `:964` 直接落盘）——与账号切换并发时存在交错窗口（见 learnings/config-write-lock-coverage）。
3. **（已修复）安卓不广播 config-changed**：`emit_config_changed`（config_state.rs `:384-391`，`:390` 广播 `{ config: masked }`）已接入 save_config（`:451`）与夜切落盘（`monitor_loop.rs:976`）；前端 `useEventListeners.ts:354-374` 消费（mergeConfigFromBackend `:360`、托盘切账号同步激活账号 `:361-368`）。回归判据：安卓端改配置后前端免手动刷新。残余：set_boot_autostart/set_notification_enabled 落盘后仅刷缓存、不广播。
4. **前端锁释放同仓两制**：`setTimeout(500)` 定时释放仍在 desktop `useQualityStore.ts:69-72` 与 android `useQualityStore.ts:73-76`（执行超 500ms 锁提前释放可重入）；已修复侧用「settle 即释放 / 实际工作+最短展示」：desktop `useAdapterStore.ts:67-69`（`await Promise.all([work, 500ms])`）、desktop `useAuthStore.ts:303`（注释 `:299-302`）、android `useAdapterStore.ts:67`、android `useAuthStore.ts:237`。
5. **裸线程绕过 BackgroundTaskManager 3 处**：gpu-warmup（`app/startup.rs:242-250`，`spawn` `:244`）；check_any_adapter_online 双子线程（`commands/login.rs:15-53`，`std::thread::scope` `:38`/`:40`）；portal_probe_on_little_cores（`monitor_loop.rs:697-734`，`Builder::spawn` `:708-721`、runtime handle enter `:707-711`、catch_unwind `:716-719`、创建失败降级 spawn_blocking `:727-731`）。
6. **AnimationProfile 字段近半无消费方**：接口 `useAnimationProfile.ts:8-26` 共 17 字段，其中 9 个无任何消费方：magneticOffset `:11`、magneticDuration `:12`、springStiffness `:14`、springDamping `:15`、powerPreference `:16`、prefersCssAnimation `:17`、enableGpuCompositing `:18`、enablePageSlide `:19`、enableBackdropBlur `:21`；有消费方的仅 4 个：willChangeOrbs（`monitor/LatencyComponents.tsx:135`/`:144`/`:161`、`shared/FluidBackground.tsx:50`）、enableTilt（`components/ui/animated-card.tsx:36`）、startupStaggerDelay（`useStartupBoost.ts:66`）、numberDuration（`shared/AnimatedNumber.tsx:23`）。
7. **安卓前端成片死代码**：`android/frontend/src/hooks/useStartupBoost.ts:15` 无任何引用；`shared/FluidBackground.tsx:1` 为空壳、仅 barrel 导出（`shared/index.ts:7`）；`App.tsx:4` 注释确认桌面件（TitleBar/StatusBar/RightPanel/DockNav/FluidBackground/Onboarding/Sponsor）在手机外壳不渲染；NetworkPanel 仅 barrel 导出（`network/index.ts:1`）。
8. **错误串未脱敏（redact_credentials 唯一调用点）**：定义 `auth/portal.rs:69`（完整 URL/编码密码/明文密码统一替换）；唯一调用 `auth/protocol.rs:166`。未覆盖处：`self_service/mod.rs:181`/`:187`（bind_operator `format!("{e}")` 直拼 reqwest 错误）、`monitor/portal_check.rs:65-71`（Portal 检测失败消息未脱敏直接 emit）。
9. **save_config_to_disk_encrypted「非空即加密」约定无护栏**：约定注释 `config/persist.rs:278-281`（实现 `:276-291`），密码三态语义靠调用方自律：桌面 `commands/config_cmd.rs:238-252`（clear_password 显式清空 / 空或 MASK 保留当前值）、安卓 `config_state.rs:393-403`（resolve_password_field）+ `:434-440`；唯一护栏是测试（`persist.rs:304-320`、`config_state.rs:541-555`/`:752-764`）。
10. **通知节流原子序不一致**：`background_emit.rs:99-100` 用 Acquire/Release；`adapter_watch.rs:129`（比较）/`:135`（store）全 Relaxed——同语义两制。
11. **useAppStore 兼容壳零引用**：`useAppStore.ts:1-3` 自称「兼容旧引用」，但全仓 grep 无任何 import，属可删除的过渡壳。
12. **插件命令清单三处不同步**：foreground-service：`build.rs:1` COMMANDS 6 项 vs `MonitorServicePlugin.kt` 13 个 `@Command`（`:42`/`:57`/`:67`/`:82`/`:109`/`:115`/`:127`/`:218`/`:236`/`:251`/`:315`/`:348`/`:367`）vs `permissions/default.toml:7-20` 12 项（缺 `allow-openNotificationSettings`）；network-bind：`build.rs:1-9` 7 项 vs `NetworkBindPlugin.kt` 12 个 `@Command`。漂移靠 tauri-build 运行时报错兜底（见 learnings/tauri-build-no-per-command-toml）。
13. **（已修复）常驻 rAF 阻止合成器休眠**：真机（25060RK16C）实测前台静置 10s 渲染 2438 帧、RenderThread 40~44%；改为按需短探测 + setInterval 轮询（证据与设计见 `renderLiveness.ts:1-13`、`useAdaptiveFramePace.ts:5-10`），判定语义（10s 停滞阈值 `renderLiveness.ts:21`）不变。回归判据：静置时无 pending rAF。
14. **（已修复）插件权限 description 未随锁语义更新**：`android/plugins/foreground-service/permissions/default.toml:2-5` 现已写明「前台服务(常驻通知+WifiLock+WakeLock)…业务逻辑(检测/自动重登)全在 Rust 侧」。

## Connections

- [[concepts/ipc-command-surface|IPC 命令面]]
- [[concepts/dual-platform-sharing|双端同构与共享]]
- [[concepts/security-model|安全模型]]
- [[concepts/config-and-persistence|配置与持久化]]
- [[concepts/background-check-and-auto-login|后台检测与自动登录]]
- [[modules/desktop-infra|桌面基础设施]]
- [[modules/desktop-monitor|桌面监控]]
- [[modules/desktop-config|桌面配置]]
- [[modules/desktop-frontend-hooks|桌面端 hooks]]
- [[modules/android-backend|安卓后端]]
- [[modules/android-frontend-core|安卓前端核心]]
- [[modules/android-plugins|安卓插件]]
- [[modules/outbound-switch|出站切换]]
- [[learnings/rAF-blocks-compositor-idle|常驻 rAF 阻止合成器休眠]]
- [[learnings/config-write-lock-coverage|配置写锁覆盖面]]
