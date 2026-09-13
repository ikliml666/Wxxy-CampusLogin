---
title: 代码库模式集
type: patterns
source_files:
  - tauri-app/src-tauri/src/lib.rs
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
  - tauri-app/src-tauri/src/commands/login.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/persist.rs
  - tauri-app/src-tauri/src/infra/events.rs
  - tauri-app/src-tauri/src/infra/command_context.rs
  - tauri-app/src-tauri/src/infra/state/mod.rs
  - tauri-app/src-tauri/src/infra/state/store.rs
  - tauri-app/src-tauri/src/infra/state/network.rs
  - tauri-app/src-tauri/src/infra/task_manager.rs
  - tauri-app/src-tauri/src/monitor/background_task.rs
  - tauri-app/src-tauri/src/monitor/background_emit.rs
  - tauri-app/src-tauri/src/monitor/adapter_watch.rs
  - tauri-app/src-tauri/src/monitor/latency.rs
  - tauri-app/src-tauri/src/network/client.rs
  - tauri-app/src-tauri/src/platform/mod.rs
  - tauri-app/src-tauri/src/self_service/mod.rs
  - tauri-app/frontend/src/hooks/useAppStore.ts
  - tauri-app/frontend/src/hooks/useQualityStore.ts
  - tauri-app/frontend/src/hooks/useAdapterStore.ts
  - tauri-app/frontend/src/hooks/useAnimationProfile.ts
  - tauri-app/frontend/src/hooks/tauriApi.ts
  - android/src-tauri/Cargo.toml
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/account_cmds.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/frontend/src/hooks/useDeviceProfile.ts
  - android/frontend/src/hooks/useAdaptiveFramePace.ts
  - android/frontend/src/lib/renderLiveness.ts
  - tauri-app/frontend/src/lib/renderLiveness.ts
  - android/plugins/foreground-service/src/lib.rs
tags: [模式, 约定]
---

## Overview

本文收录这个代码库里**被反复遵守的工程模式**：每条给出定义、出现位置（`文件:行号`）、设计理由，以及新增代码要遵循的动作。末尾「反模式与已发现的偏差」列出没遵守这些模式的地方——那是本文最有价值的部分，因为模式本身在 `CODE_WIKI.md` 里已有记载，而偏差只能靠逐处核对发现。

## 模式清单

### 领域 store 拆分（zustand 按域拆分 + 兼容壳）

**是什么**：前端不维护单个巨型 store，而是按业务域拆成 6 个独立 zustand store；拆分后保留一个只做 re-export 的兼容壳文件，避免一次性改动所有调用点。

**在哪里出现**：

- 桌面 `tauri-app/frontend/src/hooks/` 的 6 个 store：`useAuthStore.ts`（:127）、`useConfigStore.ts`（:53）、`useAdapterStore.ts`（:48）、`useQualityStore.ts`（:43）、`useLogToastStore.ts`（:28）、`useThemeStore.ts`（:19）。
- 兼容壳：`tauri-app/frontend/src/hooks/useAppStore.ts:1-3` —— 全文仅 3 行，首行注释「useAppStore 已拆分为领域 store，此文件仅保留 re-export 以兼容旧引用」，只 re-export `useAppInit`、`hasPendingConfig`、`flushPendingConfig`。安卓端有**逐字相同**的 `android/frontend/src/hooks/useAppStore.ts:1-3`。
- 跨 store 的模块级协作状态也放在 store 文件顶层而非 React state：`_checkOnlineLockFlag`（`useAuthStore.ts:19`）、`_adapterLockFlag`（`useAdapterStore.ts:9`）、`_qualityLockFlag`（`useQualityStore.ts:12`）；跨模块读取用 `useXxxStore.getState().abc()`（如 `useInitialDataLoad.ts:30` 取 `api`）。
- 主题域的副作用也挂在 store 上：`useThemeStore.subscribe(...)`（`useThemeStore.ts:53-86`）同步 `<html>` 的 class 与 `--primary` 等 CSS 变量。

**为什么**：`App.tsx` 是唯一装配点，store 数量一多就必须按域切分才能控制重渲染面；副作用与状态同文件保证"改主题只改一个文件"。共用类名/常量放在 `shared/ui-constants.ts`（`MAX_LOG_ENTRIES`、`PASSWORD_MASK`）让两个 store 不互相 import。

**新增代码应如何遵循**：新领域就新增 `useXxxStore.ts`，不要往现有 store 里塞；对外暴露的旧路径若被删除，先降级为 re-export 壳（参照 `useAppStore.ts`）。

### IPC 命令面的双端同名对齐

**是什么**：命令名 = Rust 函数名（snake_case），两端逐字相同；前端各自维护一份 `tauriApi.ts` 提供**同形接口**，差异只允许出现在三个已知映射点上。

**在哪里出现**：

- 两端唯一注册表：桌面 `tauri-app/src-tauri/src/app/startup.rs:63-120`（56 项）、安卓 `android/src-tauri/src/lib.rs:55-104`（48 项）。
- 前端出口：桌面 `tauri-app/frontend/src/hooks/tauriApi.ts`（`interface TauriApi` :27-100，实现 :133-223）、安卓 `android/frontend/src/hooks/tauriApi.ts`（接口 :56-133，实现 :166-285）。
- 差异抹平：安卓用 `desktopOnly<T>(name)`（`android/frontend/src/hooks/tauriApi.ts:14`）让桌面专属命令显式 reject、用 `noopListener`（:18）让桌面专属事件返回空清理函数，从而让两端组件代码可复用。
- 三处刻意异名/异返回：`verify_windows_identity` ↔ `verify_biometric_identity`、`get/set_auto_launch` ↔ `get/set_boot_autostart`、`get_config` 返回 `Config` ↔ 返回掩码后的 `serde_json::Value`（`android/src-tauri/src/config_state.rs:296-309`），由前端 `tauriApi` 转调抹平。

**为什么**：Tauri 命令名是字符串契约，没有编译期校验；把差异压缩到 `tauriApi.ts` 一层后，面板/组件代码可以两端逐字相同，`git diff` 也更易比对。

**新增代码应如何遵循**：一次改 5 处——Rust `#[tauri::command]` 函数、`generate_handler!`（两端各一份）、`interface TauriApi`、实现对象、调用点；若能力是桌面专属，安卓侧写 `desktopOnly`，不要从接口里删掉（删了就破坏同形契约）。

### 协议单点共享 + cfg 门控（`#[cfg(desktop)]`）

**是什么**：协议实现只存在于桌面 crate，安卓以 Cargo path 依赖复用；桌面专属模块在 `lib.rs` 顶层用 `#[cfg(desktop)]` 整体排除，平台文件内部再用 `target_os = "windows"` 细分。

**在哪里出现**：

- 共享边界：`tauri-app/src-tauri/src/lib.rs:2-8` 为跨平台可见面（`account`/`auth`/`config`/`infra`/`network`/`platform`/`self_service`），`:11-20` 的 `app`/`commands`/`helper`/`monitor`/`update` 全是 `#[cfg(desktop)]`。
- 依赖声明：`android/src-tauri/Cargo.toml:34` 的 `campus-login = { path = "../../tauri-app/src-tauri" }`。
- 平台内门控：`tauri-app/src-tauri/src/platform/mod.rs:1-18`（只有 `console_output` 跨平台，`toast` 是唯一的 `all(desktop, target_os = "windows")`）；`network/mod.rs:17-25` 用 `#[cfg(target_os = "windows")]` / `#[cfg(not(...))]` 给 `dhcp_release_renew_single` 提供真实实现与跨平台 stub。
- 安卓插件反向门控：三个插件 crate 首行都是 `#![cfg(mobile)]`（`android/plugins/keystore/src/lib.rs:1` 等），host 编译为空。
- 平台分支桩：`account/crypto.rs:128-136` 在非 Windows 返回 `Err("加密存储仅桌面端支持")`，安卓改用 Keystore 插件。

**为什么**：复制协议代码必然分叉（登录协议改一次要改两处且容易漏）；`cfg` 门控让"只在桌面编译"成为编译期事实而非文档约定。

**新增代码应如何遵循**：往共享 crate 加模块前先自问"安卓能不能编译"——若引用了 Windows API 或 Tauri 桌面插件，必须 `#[cfg(desktop)]` 门控或在 `mod.rs` 里按 `target_os` 分发；安卓侧禁止复制协议逻辑，只允许包装。

### 状态用 ArcSwap/CAS 快照而非全局锁

**是什么**：可变的全局状态用 `ArcSwap<T>` 保存不可变快照，写入走 CAS 循环（`compare_and_swap` + `Arc::ptr_eq` 判胜出），读侧只取 `Arc` 快照不做加锁；需要"读-改-判定"时把判定放进 CAS 闭包并用 `update_with_result` 返回结果。

**在哪里出现**：

- 配置：`ConfigStore`（`infra/state/store.rs:6-8`）持 `ArcSwap<Config>`，写入 `update()`（`state/store.rs:35-49`）——CAS 循环 + `Arc::ptr_eq`。
- 网络状态：`NetworkState`（`infra/state/network.rs:53-55`），`update()`（:76-90）与 `update_with_result()`（:96-111，"读-改-判定"同一次 CAS，避免 TOCTOU）。
- 进程级单值：`network/client.rs:8` 的 `PORTAL_URL: ArcSwap<String>`；`infra/logger.rs:59` 的 `LOGGER_SENDER` 与 `:62` 的 `MIN_LOG_LEVEL`。
- 纯标志位用 `AtomicBool` CAS：`TaskLock::try_acquire()`（`infra/state/mod.rs:35-41`）的 `compare_exchange(false, true, Acquire, Relaxed)`，RAII 在 `Drop for TaskGuard`（`infra/state/mod.rs:54-58`）里 `Release` 清位。
- 一次性门控：`update_stats.update_notified` 的 `compare_exchange(false, true, ...)`（`update/updater.rs:292`）。

**为什么**：巡检循环、命令、事件发射在多个线程/异步任务中并发读写同一状态，全局 `Mutex` 会让读路径互相阻塞，且持锁跨 `await` 会死锁。`ArcSwap` 的快照语义让读侧零锁、写侧由 CAS 保证不丢更新。

**新增代码应如何遵循**：新状态字段优先加到 `NetworkSnapshot`（`infra/state/network.rs:7-26`）或 `UpdateStats`（`infra/state/mod.rs:86-101`），通过 `update`/`update_with_result` 改；**要在一次 CAS 内完成的语义必须用 `update_with_result`**，不要写成 `load()` 后 `update()`（历史缺陷：断线重连计数曾因两次快照双重触发 MAC 重置）。

### 后台任务统一走 BackgroundTaskManager + CancellationToken

**是什么**：所有周期性/长生命周期后台任务都用 `state.task_manager.spawn("唯一名字", |cancel_token| async move { ... })` 注册，同名任务拒绝重复注册；退出时由 `shutdown()` 统一 cancel + await。

**在哪里出现**：

- 管理器：`infra/task_manager.rs`（`spawn` :33、`cancel` :67、`detach` :81、`shutdown` :86-98、`is_running` :101、`cancel_token` :106）。
- 注册点（全仓 13 处生产 spawn）：`app/heartbeat.rs:11`（`heartbeat`）与 `:68`（`window_safety`）、`infra/lifecycle.rs:78`（`campus_exit`）与 `:228`（`auto_exit`）、`monitor/adapter_watch.rs:12`（`adapter_watch`）、`network/adapter_cache.rs:252`（`adapter_cache_refresh`）、`monitor/watcher.rs:22/34/49`（`startup_bg_check`/`startup_latency`/`startup_auto_login`）、`monitor/background_task.rs:16`（`background_check`）、`monitor/latency.rs:54`（`latency_test`）、`monitor/auto_auth.rs:235`（`auto_login_on_start`）、`update/updater.rs:249`（`update_check_loop`）。任务名清单与 `app/startup.rs:183-204` 的启动顺序一一对应。
- 循环体退出判据统一是"cancel token + `exit.is_quitting` + `task_manager.is_running(自己)`"三选一（`monitor/background_task.rs:37-47`）。
- `detach` 的专用场景：任务自身即将调用 `shutdown_and_exit` 时必须先摘除自己，否则 shutdown 会等自己造成死锁（`infra/lifecycle.rs:126`、`:256`）。

**为什么**：任务表是唯一的"什么在跑"的真源，前端 `get_background_status` 的 `isRunning` 字段直接读它；进程退出若靠各任务自己响应信号，会出现"某些任务永远等不到"的悬挂。

**新增代码应如何遵循**：一律走 `spawn`，不要裸 `std::thread::spawn` 或 `tokio::spawn`；`tokio::select!` 里必须处理 `cancel_token.cancelled()`；需要"立即停"的场景用 `cancel_token.cancel()` 而不是只改标志。

### 事件统一走 EventBus（`.emit(` 只出现在一处）

**是什么**：后端向前端推送的唯一通道是 `infra/events.rs` 的 `EventBus`，16 个业务事件各有一个 `emit_*` 方法；底层 `app_handle.emit` 只允许出现在 `events.rs` 内。

**在哪里出现**：

- 唯一底层调用：`tauri-app/src-tauri/src/infra/events.rs:18` —— `grep -rn "\.emit(" tauri-app/src-tauri/src/` 只命中 `events.rs`（1 处定义 + 16 处 `self.emit(...)`）。
- 快捷入口：`AppHandleExt`（`infra/command_context.rs:38-51`）把 `notify_config_changed` / `notify_update_download_progress` 挂到 `AppHandle` 上，供命令层直接调用。
- 前端消费：`createEventListener`（`tauri-app/frontend/src/hooks/tauriApi.ts:102-131`）→ 14 个事件在 `useEventListeners.ts` 集中注册，2 个按需在组件内注册。
- 共享 crate 的业务代码也遵守：`network/quality.rs:507/583/637` 用 `EventBus::new(ah).emit_network_quality_result(...)` 增量推送，而不是自己 `emit`。

**为什么**：事件名是字符串常量、无编译期校验；集中一处才能回答"系统会发哪些事件、payload 长什么样"，并把"掩码后再发"这类安全约束固定在出口上（`config-changed` 的载荷必须是 `masked_for_display()` 的结果）。

**新增代码应如何遵循**：在 `events.rs:117` 之后追加 `emit_xxx` 方法 → 业务处调用 → 前端 `tauriApi.ts` 加 `onXxx` + `useEventListeners.ts` 注册；**不要绕过 EventBus**（桌面端当前无此先例）。

### 敏感信息出站唯一出口（masked_for_display）

**是什么**：任何把配置结构发往前端的路径都必须先经 `masked_for_display()`；掩码语义固定为"非空 → `"***"`，空串原样保留"，并有"空串/`***` 表示未修改 → 回退内存旧值"的写侧对偶。

**在哪里出现**：

- 桌面出口：`config/model.rs:214-230`（注释明确"唯一出口约定：不得手工逐字段打码——漏一个字段就是一次明文泄露，account 三命令即前车之鉴"），`masked_for_display()` 走 clone 后掩码，`mask_in_place()` 就地掩码。
- 桌面调用点：`commands/config_cmd.rs:15`（`config-changed` 载荷）与 `:88`（`get_config`）、`commands/system.rs:117`（`get_init_data`）、`commands/account.rs:47/188/227`（账号三命令）。
- 安卓出口：`config_state.rs:273` 的 `masked_for_display(&Settings) -> serde_json::Value`，调用点 `config_state.rs:308`、`account_cmds.rs:111`、`account_cmds.rs:190`。
- 写侧对偶（掩码回退）：桌面 `commands/config_cmd.rs:109-123` 的三态处理；安卓收敛为纯函数 `config_state.rs:285` 的 `resolve_password_field(incoming, current, clear)`。
- 回归锁：桌面 `commands/config_cmd.rs:143-147` 的测试注释记录了历史缺陷 `fe000de`（修 `get_init_data`/`get_config` 时漏掉 account 三命令，明文 `selfPassword` 随 IPC 出站），测试体在 `:149-164`；安卓对应 `config_state.rs:548-558`。
- 相关约定：落盘"非空即加密"，**不得排除 `"***"`**（`config/persist.rs:155-158` 注释说明：若真实密码恰为 `***`，排除判断会让它明文落盘）。

**为什么**：`Config` 会经 IPC、事件、日志三条路出站，手工逐字段打码在加字段时必然漏。把掩码收敛到类型自身的方法上，新字段只需改一处（但要改对两处：桌面 `mask_in_place` + 安卓 `masked_for_display`）。

**新增代码应如何遵循**：新增敏感字段要同时改 `model.rs:223-230` 与 `config_state.rs:273-282`；新增任何"把配置发给前端"的路径一律走掩码方法；不要新增第三个出口。

### 配置原子写（atomic_write + fsync + 重试）

**是什么**：配置落盘必须"写临时文件 → fsync → rename 覆盖"，rename 失败重试，临时文件名带纳秒时间戳避免并发撞名。

**在哪里出现**：

- 桌面唯一实现：`config/persist.rs:12-43` —— 纳秒时间戳临时名（`:14-20`）、`BufWriter` 写入并 `flush`（`:26-30`）、`tmp_file.sync_all()`（`:31`，注释说明这是与 `std::fs::write` 的关键差别）、`rename` 重试 3 次 × 100ms（`:32-39`），最终失败清理临时文件并返回 Err（`:41-42`）。
- 调用面：`config/persist.rs:153-168` 的 `save_config_to_disk_encrypted`（先加密两个密码字段再原子写）与 `:99-151` 的 `append_login_history`（加 `LOGIN_HISTORY_LOCK` 串行化读-改-写）。
- 提权子进程也遵守：`helper/mod.rs:190-197` 写结果文件先写 `.tmp` 再 `rename`。
- 安卓本端实现：`config_state.rs:219-221`（`.json.tmp` + rename）、`login_history.rs:72-74`。

**为什么**：配置是单文件全量覆盖，写到一半断电/崩溃会得到半截 JSON，整个配置被 `Config::default()` 取代（用户丢配置）。

**新增代码应如何遵循**：任何新落盘文件都走 `persist::atomic_write`；安卓侧没有导出等价 helper，新增写路径需自己保证"tmp + rename"并考虑并发保护。

### 前端动画与性能分级（deviceProfile / animationProfile）

**是什么**：动画不是全开或全关，而是由"设备能力 + 用户偏好"解析成一个档位（`high`/`standard`/`economy`），组件按档位字段决定是否加 `will-change`、是否禁用倾斜/入场动画、数字滚动时长等；两端各有**不同**的分级输入与降级链。

**在哪里出现**：

- 桌面档位接口与解析：`tauri-app/frontend/src/hooks/useAnimationProfile.ts:9-27`（`AnimationProfile` 字段）、`:30-48`（`HIGH_PROFILE` 满配基线）、`:52-58`（`ECONOMY_OVERRIDES`，注释声明只覆盖"当前被组件消费的高开销字段"）、`:60-65`（`resolveTier`：reduced-motion 或 `low-igpu` → economy；`discrete`/`high-igpu` → high；其余 standard）、`:73-95`（hook 组装，输入是 `refreshRate` + `gpuInfo` + reduced-motion）。
- 消费点：`enableTilt`（`components/ui/animated-card.tsx:36`）、`startupBoost` 与 `startupStaggerDelay`（`useStartupBoost.ts:40/48/66/69`）、`numberDuration`（`shared/AnimatedNumber.tsx:23`）、`willChangeOrbs` 与 `easing`（`monitor/LatencyComponents.tsx:135/144/161`）。
- 空闲/可见性闸门：`usePageIdle.ts:83-97` 的 `AnimationActiveProvider` + `useAnimationActive()`（顶层单例注册可见性/焦点/空闲判定，避免每个调用点各注册 8-9 个监听器）。
- 帧率/缓动派生：`lib/easing-config.ts:27-29` 按 `refreshRate >= 120` 选两套贝塞尔基线。
- 安卓的**另一条链路**：`useDeviceProfile.ts:93`（WebGL renderer + 核数/内存同步分级，异步用 `get_soc_info` 只升不降）→ `useAdaptiveFramePace.ts:38-45` 的 `applyPace()`（把 idle/active fps 落到 `gsap.ticker.fps()`，档位未变则空操作）被两处驱动：`setInterval(applyPace, PACE_POLL_MS=250)`（`:64`，常量 `:18`）负责松手后的静止回落，交互事件 `markInteraction()`（`:26-29`）负责即时提帧；移动端另有 `useFormFactor.ts:38` 的外壳二选一。

**为什么**：低端设备上满配动画会掉帧；`prefers-reduced-motion` 是用户明确的无障碍要求。把决策集中到 hook、把消费变成读字段，新动画只需读档位字段而不用自己探测设备。

**新增代码应如何遵循**：新动画先问"economy 档要不要降级"，要降级就在 `ECONOMY_OVERRIDES`（或消费点）读取对应字段；需要长驻动画的组件必须经 `useAnimationActive()` 判空闲，并保证 GSAP tween 的生命周期与挂载/卸载对齐。

### 按需驱动替代常驻驱动（rAF 与系统锁都只在"用得到的那一刻"存在）

**是什么**：凡是"为了持续感知状态"而常驻的驱动源都要改成按需——渲染侧不在页面里留 pending `requestAnimationFrame`，系统锁不在进程生命周期内常驻持有，改为"需要那一刻开窗、窗口结束即关"；周期任务的"唤醒周期"与"真正执行间隔"也解耦，唤醒保持最短档、执行按场景拉长。

**在哪里出现**：

- 渲染存活判定（双端逐字相同，提交 `266624b`）：`lib/renderLiveness.ts:36-40` 的 `isRenderLoopAlive()` 距上次探测超过 `PROBE_REFRESH_MS = 4_000`（`:18`）才经 `startProbe()`（`:24-34`）开一个 2 帧（≈33ms）的 rAF 窗口刷新时间戳；`probing` 防重入、`PROBE_TIMEOUT_MS = 500`（`:20`）兜底复位，10s 停滞判定语义不变。桌面 `tauri-app/frontend/src/lib/renderLiveness.ts`、安卓 `android/frontend/src/lib/renderLiveness.ts`。
- 帧率轮询（安卓独有，提交 `02b0a74`）：`android/frontend/src/hooks/useAdaptiveFramePace.ts:64` 用 `window.setInterval(applyPace, PACE_POLL_MS)`（`PACE_POLL_MS = 250`，`:18`）替代原 rAF 递归轮询；交互起始由 passive 监听回调 `markInteraction()`（`:26-29`）即时提帧，轮询只承担松手后的静止回落（档位未变时空操作，`:38-45`）。
- 系统锁按需持有（安卓，提交 `fac6407`）：巡检每拍 `monitor_loop.rs:630-635` 调 `begin_probe_window()` 并挂 `ProbeWindowGuard`，Drop 必调 `end_probe_window()`（guard 定义 `:497-508`）；Kotlin 侧 `ForegroundService.acquireProbeLocks()`（`ForegroundService.kt:121-142`）持 `WIFI_MODE_FULL_HIGH_PERF` WifiLock + 带 30s 上限的 `PARTIAL_WAKE_LOCK`，`releaseProbeLocks()`（`:146-156`）在窗口外全部释放——原实现在 `onCreate` 里常驻 acquire。
- 事件唤醒去重（安卓）：`ForegroundService.kt:198-206` 仅在"关注字段"（`TRANSPORT_WIFI` / `TRANSPORT_CELLULAR` / `NET_CAPABILITY_VALIDATED` 位掩码）翻转时才短持唤醒锁，5s 节流（`:36`）降为第二道闸（`onCapabilitiesChanged` 在弱信号下每秒多条连发）。
- 巡检频率分档（安卓，提交 `07d7bf5`）：`monitor_loop.rs:61-68` 的纯函数 `effective_interval_ms(base, idle, screen_on, wifi_connected)`——唤醒周期恒为基础间隔（保证亮屏/回 WiFi 后最迟一拍恢复），蜂窝或灭屏时由循环体 `continue` 跳拍（`:544-553`）；电源状态经插件 `get_power_state()` 查询，失败按保守值 `(true, true)`（`monitor_loop.rs:510-524`）。
- 空闲冻结（安卓 CSS）：`.anim-idle .animate-pulse` 独立成规则（`android/frontend/src/index.css:718-720`），保证用户 2s 无输入后脉冲动画真的停（该规则此前因逗号选择器共享 body 而失效，见 [[css-comma-selector-shared-body-pitfall]]）。

**为什么**：常驻 rAF 会让 Chromium 视页面为"有活跃动画"、按刷新率持续派发 BeginFrame，合成器永不休眠——**用 rAF 做的帧率控制器本身阻止了合成器休眠**；常驻 `WIFI_MODE_FULL_HIGH_PERF` 则让系统永不进入 WiFi 省电（CDD 对持锁期另有要求）。正确性护栏：能拉长/跳过的只能是"可重新发现"的检测——WiFi 事件仍即时触发检测、无明确离线证据时在线状态保持上一拍记忆、电源状态查询失败一律按最保守档处理（省电是优化项，漏检是功能缺陷）。

**新增代码应如何遵循**：（a）要"持续感知"先问能不能改成"事件 + 按需探测"；（b）页面内必须跑循环时用 `setInterval` 或短窗 rAF，**禁止模块级/递归 `requestAnimationFrame`**（回归判据：若再观察到静置满帧，先 grep 常驻 rAF）；（c）持系统锁一律配 RAII guard + 超时兜底，禁止常驻 acquire；（d）省电类改动必须真机实测（静置 10s 帧数与全线程占用），不能只看代码意图。

### 后台检测的定时循环与节流（周期常量）

**是什么**：周期循环用 `tokio::time::interval` 但间隔值**每轮从配置重读**（改设置不重启任务即生效）；通知类副作用一律带节流时间戳常量，避免事件风暴刷屏。

**在哪里出现**：

- 动态间隔：桌面 `monitor/background_task.rs:29-49`（每轮读 `cfg.background_check_interval.max(10000)` 重建计时器）、安卓 `monitor_loop.rs:538-543`（比对 `desired_interval_ms` 重建）。
- 固定周期常量：`monitor/adapter_watch.rs:8` 的 `ADAPTER_WATCH_INTERVAL = 15000`、`network/adapter_cache.rs:242` 的 4s 缓存刷新、`app/heartbeat.rs:12-13` 的 5s 检测 + 20s 陈旧阈值。
- 抗漂移/背压选择：`monitor/latency.rs:59` 用 `MissedTickBehavior::Delay`（避免耗时超周期时连发补 tick），`:79-90` 是"就绪前每 2s 短重试、不消耗周期"的内循环。
- 节流常量：网络状态变更通知 60000ms（`monitor/background_emit.rs:95`，`Acquire`/`Release` 读写）、适配器禁用警告 60000ms（`monitor/adapter_watch.rs:110-116`，`Relaxed` 读写——见偏差清单）、自动启用退避阶梯 `0/60_000/120_000/300_000`（`monitor/adapter_watch.rs:223-230`）。
- 复核窗口：`monitor/quality_scheduler.rs:12-13` 的 `SPIKE_CONFIRM_COUNT = 2` + `SPIKE_CONFIRM_INTERVAL_SECS = 15`（质量转差要连续两次确认才告警）。
- 安卓侧同类：`monitor_loop.rs:44/47` 的 WiFi 事件延迟 2500ms + 去抖 1000ms、`:61-68` 的 `effective_interval_ms` 分档（基础间隔 + 闲时间隔双档）、`:163-166` 在 `start_background_check` 里把两个间隔一起从配置刷进 `MONITOR`（`desired_interval_ms` / `idle_interval_ms`），循环体逐 tick 重读（`:538-553`）。

**为什么**：巡检是长驻循环，配置热更新与用户可感知的节流是"能不能长期挂着跑"的前提；`Delay` 语义与就绪内循环是"网络没起来时不刷屏、不空转计数"的取舍。

**新增代码应如何遵循**：新周期任务（a）间隔从配置读并每轮重读；（b）节流阈值提为命名常量并写清读写 Ordering；（c）循环退出判据照抄三选一模板；（d）必要时用 `MissedTickBehavior::Delay`。

## 反模式与已发现的偏差

按严重程度排序，全部带 `文件:行号`。

1. **安卓 5 处裸 `app.emit` 绕过 EventBus 模式**：`android/src-tauri/src/monitor_loop.rs:133`（`login-log`）、`:446`（`auto-login-result`）、`:760`（`background-check-result`）、`android/src-tauri/src/update_cmds.rs:251`（`update-available`）、`:355`（`update-download-progress`）。安卓没有 `events.rs`，事件名只以字面量形式散落在这些位置，没有集中登记点；而同一 crate 对 `network-quality-result` 又经共享 crate 走 `EventBus`（`tauri-app/src-tauri/src/network/quality.rs:507`）。事件面因此是"半统一"状态。
2. **安卓 `save_config` 不持 `CONFIG_IO_LOCK`**：锁定义与导出在 `android/src-tauri/src/account_cmds.rs:17/21`，`switch_account`（`:127`）、`save_current_as_account`（`:156`）、`delete_account`（`:179`）、`set_boot_autostart`（`monitor_loop.rs:331`）、`set_notification_enabled`（`monitor_loop.rs:355`）都取锁，唯独做全量配置读改写的 `config_state.rs:312-338` 的 `save_config` 没取——它正是该锁注释里点名的竞态对象之一。
3. **安卓不广播 `config-changed`**：`config_state.rs:311-338` 只落盘 + 刷 `AndroidState.config` 缓存。安卓前端却注册了监听（`android/frontend/src/hooks/useEventListeners.ts:350`），导致 `mergeConfigFromBackend`（`useConfigStore.ts:110`）的脏字段合并分支在安卓永不触发——"配置回流"这条模式在安卓只靠命令返回值维持。
4. **前端锁释放方式"同仓两制"，且两端都没修干净**：桌面 `tauri-app/frontend/src/hooks/useAdapterStore.ts:65-69` 已改成"实际工作完成 + 最短展示 500ms 后释放"（注释自述这是对历史缺陷 `setTimeout(500)` 的修复，`useAuthStore.checkOnline` 同法），而 `tauri-app/frontend/src/hooks/useQualityStore.ts:68-72` 的 `refreshQuality` 仍是 `finally { setTimeout(() => { _qualityLockFlag = false; ... }, 500) }`；安卓复刻树同名同错（`android/frontend/src/hooks/useQualityStore.ts:72-77`）。`check_network_quality` 耗时超过 500ms 时锁提前释放，允许并发重入。
5. **裸线程绕过 `BackgroundTaskManager`（3 处）**：`tauri-app/src-tauri/src/app/startup.rs:208-216` 的 `gpu-warmup` 预热线程（`shutdown_and_exit` 不会等它，进程退出时被直接终止）、`commands/login.rs:16-54` 的 `check_any_adapter_online`（两个裸 `std::thread::spawn` + 先 `tauri::async_runtime::handle().inner().enter()` 再发包）、`android/src-tauri/src/monitor_loop.rs:563-600` 的 `portal_probe_on_little_cores`（裸线程 + `catch_unwind` + 失败降级 `spawn_blocking`）。前两处没有任务名，无法被 `stop_*` 或退出流程管理。
6. **`AnimationProfile` 近半字段无消费方**：`useAnimationProfile.ts:12-22` 的 `magneticOffset`、`magneticDuration`、`springStiffness`、`springDamping`、`powerPreference`、`prefersCssAnimation`、`enableGpuCompositing`、`enablePageSlide`、`enableBackdropBlur` 只出现在定义处；`components/ui/animated-card.tsx:6-12` 的 `AnimatedCardConfig`（`glowIntensity`/`hoverScale`/`stiffness`/`damping`/`mass`）同样全字段无消费方。`ECONOMY_OVERRIDES`（`:52-58`）的注释只声明它覆盖"被消费的字段"，未声明其余是死字段——改它们不会有任何效果。
7. **安卓前端存在成片死代码（复刻载荷）**：`android/frontend/src/hooks/useStartupBoost.ts:15`（无 import，其 refs 指向手机外壳根本不渲染的桌面元素）、`shared/FluidBackground.tsx:1`（无渲染点）、`network/NetworkPanel.tsx:47` 与 `network/useNetwork.ts:39`（面板层整体无入口）、`hooks/useAppStore.ts:1-3` 兼容壳（**两端**都无 import，可直接删除）。这些是"复刻后按平台裁剪"留下的尾巴，与桌面端 `useAppStore.ts` 的同类残留一致。
8. **错误串未脱敏就进日志与返回值（掩码模式只覆盖了配置结构本身）**：脱敏函数 `auth/portal.rs:69` 的 `redact_credentials` 全仓只有一个调用点（`auth/protocol.rs:129`，登录请求失败路径）；自助服务的错误一路进 `CommandResult.message`，全是 `format!("...: {e}")` 形式——`self_service/mod.rs:143/179/185/186/284/285/294/296/308/343` 等十余处；Portal 探测失败时把 reqwest 错误文本原样写进日志与 `login-log` 事件（`monitor/portal_check.rs:56-62`）。结论：**新增出站路径若不显式脱敏，没有任何机制替你兜底**。
9. **`persist::save_config_to_disk_encrypted` 的"非空即加密"依赖调用方把占位符换回真值**：`config/persist.rs:153-168` 刻意不排除 `"***"`（理由正当：真实密码可能就是 `***`），代价是任何把掩码值直接落盘的调用方，重启后会得到明文密码 `"***"`。当前只有 `commands/config_cmd.rs:107-123` 一处调用方，正确性靠它维持，属"约定无护栏"。
10. **节流用 `Relaxed` 的两处与其它处不一致**：`monitor/adapter_watch.rs:111/116` 读写 `last_disabled_notification_ms` 用 `Ordering::Relaxed`，而同类节流 `monitor/background_emit.rs:95-96` 用 `Acquire`/`Release`；并发下前者节流失效会表现为 60 秒内重复弹禁用警告。
11. **`useAppStore` 兼容壳的"约定"没人执行**：两端 `useAppStore.ts:1-3` 都自述"仅保留 re-export 以兼容旧引用"，但全仓零引用（`grep -rn "useAppStore"` 只命中文件自身）。壳文件本身是模式，**无人引用的壳是债务**——拆分完成时应删除。
12. **插件 `build.rs` 的 `COMMANDS` 与 Kotlin `@Command`/权限清单三处不同步**：`android/plugins/network-bind/build.rs:1` 只声明 3 个命令，Kotlin 侧还有 `startWifiWatcher`/`stopWifiWatcher`（`NetworkBindPlugin.kt:307/345`）无权限文件；`foreground-service/permissions/default.toml:8-19` 列 **18 项**而 `build.rs:1` 只注册 6 个——差额在 2026-09-13 由探针窗口/电源状态/电池白名单 6 个新命令继续拉大（新命令只进了 default.toml 与 `permissions/autogenerated/reference.md`，`build.rs` 的 `COMMANDS` 未同步）。因为 `capabilities/default.json:8-13` 未引用这些权限集（只有 `core`/`biometric`/`opener`/`notification` 四个 `:default`），不一致不会在构建期暴露——权限模式（default.toml + build.rs + autogenerated）三处必须同时改，当前缺守门。
13. **（历史偏差，已修复，保留作回归判据）常驻 rAF 阻止合成器休眠**：`useAdaptiveFramePace` 的 rAF 递归轮询 + `renderLiveness` 的模块级 rAF 无限循环，让页面永远存在 pending `requestAnimationFrame`，Chromium 据此按刷新率满帧派发 BeginFrame（真机实测前台静置 10s 渲染 2438 帧、RenderThread 40~44%）。已在 `266624b`（`renderLiveness` 按需短探测，双端）与 `02b0a74`（帧率轮询改 `setInterval`，安卓）修复，现状见模式清单「按需驱动替代常驻驱动」一条；该偏差不再计入当前问题清单，但新增渲染侧循环时仍按此条自检。
14. **插件权限集 description 未随锁语义更新**：`android/plugins/foreground-service/permissions/default.toml:3`（及生成器写入的 `permissions/schemas/schema.json`）仍写「前台服务(常驻通知+WifiLock+WakeLock)」，而 2026-09-13（`fac6407`）起两个锁只在探针窗口内持有。该文本会进 IDE 的权限提示，属会误导的文档漂移；改文案需同时动 default.toml 与 autogenerated/schema 产物。

## Connections

- [[ipc-command-surface]] —— 命令注册点、命名对齐与 16 个事件的双端矩阵（模式 2/6 的完整清单）
- [[dual-platform-sharing]] —— path 依赖、cfg 门控边界与两端差集（模式 3 的约束面）
- [[security-model]] —— 掩码出口、落盘加密与验证门（模式 7 的安全语义）
- [[config-and-persistence]] —— 配置字段、磁盘布局与迁移（模式 8 的调用面）
- [[background-check-and-auto-login]] —— 巡检↔自动登录链路与全部周期/阈值常量（模式 11 的消费面）
- [[desktop-infra]] —— `AppState`/`EventBus`/`BackgroundTaskManager`/日志（模式 4/5/6 的实现处）
- [[desktop-frontend-hooks]] —— 6 个领域 store、`tauriApi.ts`、`useEventListeners`（模式 1/9 的桌面端）
- [[android-frontend-core]] —— 安卓 store、双外壳、`useDeviceProfile`/`useAdaptiveFramePace`（模式 1/9 的安卓端）
- [[desktop-config]] —— `config/model.rs` 与 `config/persist.rs` 的逐函数详解
- [[_architecture]] —— 分层架构、依赖图与数据流总览
- [[rAF-blocks-compositor-idle]] / [[android-keepalive-fgs-architecture]] —— 常驻驱动为何耗电的真机实测教训、「探针窗口 + 按需锁」的前台服务架构决策（模式 10 的来源）
