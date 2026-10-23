---
title: "安卓后台留存：退出护栏、质量循环稳态退避与 renderer 优先级放行"
type: decision
source_files:
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/quality_cmds.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/quality_history.rs
  - android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/MainActivity.kt
tags: [决策, 安卓, 后台内存, 保活, 前台服务, 双端同构]
---

## 背景

安卓端后台内存与功耗优化（2026-09-20，与桌面轻量化模式同一轮）。此前 [[android-keepalive-fgs-architecture]] 已定 FGS 保活架构与省电细节，遗留两项 "未实施备查"：质量循环稳态退避（12+ 外网目标/60s 为后台功耗流量主力）与 WifiLock 降级；本轮调研新增两项系统性修复。

## 决策

### 1. 退出护栏：`RunEvent::ExitRequested + api.prevent_exit()`（常置）

Tauri/tao 默认"最后窗口关闭 → `process::exit(0)`"会把前台服务一起带走（tauri #15671 根因分析：tao `src/platform_impl/android/mod.rs` run 循环末尾 `process::exit(exit_code)`）。用户从最近任务划掉 Activity 时 Activity 销毁即触发该路径，巡检/夜切/定时动作全部消失。修复：`android/src-tauri/src/lib.rs` 的 `run()`（`#[cfg_attr(mobile, tauri::mobile_entry_point)]`，lib.rs:30-31）由 `Builder::run` 单段式改为 `build()` + `run(callback)`：`builder.build(tauri::generate_context!())`（lib.rs:123）后链式 `.run(|_app, event| …)`（lib.rs:125-132），回调内 `if let tauri::RunEvent::ExitRequested { api, .. } = event { api.prevent_exit(); }`（lib.rs:129-131），代码处留 3 行注释说明缘由（lib.rs:126-128）。安卓语义：划掉任务只关 UI，进程随前台服务常驻；彻底退出走系统设置（桌面侧的对应机制是轻量化单次守卫，见 [[lightweight-mode-desktop]]，两端语义不同故实现不同）。

### 2. 质量循环稳态退避

`quality_cmds.rs::latency_loop`（quality_cmds.rs:97-128）：连续 5 拍质量 ∈ {excellent, great, good} → 间隔翻倍递进，封顶 1800s（与桌面轻量化质量间隔下限对齐）；fair/poor/bad/unknown/busy 任何一拍即清零恢复基础间隔。参数常量 `GOOD_LEVELS`（quality_cmds.rs:89）/`BACKOFF_STABLE_STREAK=5`（quality_cmds.rs:97）/ `BACKOFF_MAX_MS=1_800_000`（quality_cmds.rs:97），计算抽纯函数 `next_backoff_interval_ms`（quality_cmds.rs:95-101：stable_good_streak 每达 5 拍对 current_ms 翻倍 `current_ms.saturating_mul(2).min(BACKOFF_MAX_MS).max(base_ms)`，未达 5 拍维持当前间隔）。单测覆盖翻倍/封顶/不变/不低于基础间隔（quality_cmds.rs:97-130 四个用例：`稳定计数达5_间隔翻倍_上限封顶`、`未达5拍_间隔不变`、`翻倍不低于基础间隔`、`等级判定_good及以上算稳定`）。

循环数据流（当前代码）：

- 基础间隔 = `settings.latency_test_interval.max(10_000)`（quality_cmds.rs:104），来自 `config_state::current_settings`；递进序列随配置值而变（旧默认 60s 时 60s→120s→240s→…；现默认 600s 时 600s→1200s→1800s 封顶）。
- 启停与代际守卫：`start_latency_test` 幂等（`LATENCY_RUNNING.swap(true)`，quality_cmds.rs:59），读当前配置（quality_cmds.rs:59-67，失败则复位标志并返回错误，quality_cmds.rs:67）后取 `LATENCY_GENERATION.fetch_add(1)` 代际值（quality_cmds.rs:71）并 `tauri::async_runtime::spawn(latency_loop(app, settings, gen))`（quality_cmds.rs:59）；`stop_latency_test` 先推进代际再清标志（quality_cmds.rs:80-81），避免旧循环误清新一轮运行标志；循环退出前仅当启动代际仍匹配才 `LATENCY_RUNNING.store(false)` 收尾（quality_cmds.rs:59-61）。运行标志为 `lazy_static` 的 `AtomicBool LATENCY_RUNNING`（quality_cmds.rs:11-13），代际为 `static AtomicU64 LATENCY_GENERATION`（quality_cmds.rs:59）。
- 循环每轮「先检测、后 sleep 当前间隔」（quality_cmds.rs:107-110 注释）：不再重建 `tokio::time::interval`，而是 `loop` 内先查运行标志与代际（quality_cmds.rs:111），调用 `run_quality_once`（quality_cmds.rs:114），再据退避状态机更新 `current_ms` 后 `tokio::time::sleep`（quality_cmds.rs:127）。该结构天然支持退避变周期，无首 tick 陷阱。
- 每拍检测汇聚于 `run_quality_once`（quality_cmds.rs:19-49）：读当前配置（quality_cmds.rs:25-27）+ `AndroidState.cached_source_ip`（quality_cmds.rs:25-31）→ 构造恒 false 的 `is_quitting` 退出哨兵（quality_cmds.rs:30）→ `check_network_quality_async("wlan0", &adapter_ip, settings.skip_ttfb_in_latency, settings.skip_content_in_latency, &settings.fixed_gateway, is_quitting, Some(app), false)`（quality_cmds.rs:31-41；quality 内部经 EventBus emit `network-quality-result`，见 quality_cmds.rs:114 调用处注释）→ 质量历史落盘 `quality_history::append`（quality_cmds.rs:25-29，命令与定时循环共此一条路径，每次真实检测记一条供趋势回溯）。手动命令 `check_network_quality`（quality_cmds.rs:25-28）与定时循环共用该函数。
- 退避状态机（quality_cmds.rs:116-126）：good 及以上 `stable_good_streak` 累加并经纯函数更新间隔；波动/失败一拍即清零恢复基础间隔（累计已满 5 拍时额外打一条 info 日志"网络质量波动，检测间隔 … 恢复 …"，quality_cmds.rs:119-122）。

### 3. renderer 优先级放行（BOUND 温和档）——**2026-10-01 已整体摘除，见文末「摘除记录」**

`MainActivity.onWebViewCreate`（wry `WryActivity.kt:56` 开放钩子）调 `webView.setRendererPriorityPolicy(RENDERER_PRIORITY_BOUND, waivedWhenNotVisible=true)`（API 26+）。WebView 默认策略不分可见性一律 IMPORTANT，不设置则系统永远不会主动回收 renderer——它是 WebView 内存大头（独立 sandboxed_processX 进程）。

**风险语义重析**（此为放弃计划中"onRenderProcessGone 兜底"路径的依据）：
- AOSP：不覆写 `onRenderProcessGone` 时 renderer 被杀 → 宿主进程陪葬；但 wry 生成类 `RustWebViewClient` 为**非 open**（不可继承覆写）且未覆写该回调，`setWebViewClient` 替换会丢失 wry 全部关键回调（IPC/URL 拦截），不可行；
- renderer **自身 crash** → 宿主崩溃：与是否设置本策略**无关**（现状即如此，策略不改变 crash 概率）——BOUND 不新增任何 crash 面；
- BOUND 新增场景仅是"极端内存压力下 renderer 被系统 OOM 杀 → 宿主连带被杀 → `START_STICKY` 前台服务自动重启（冷启动自愈，巡检中断数秒后恢复）"，相对 WAIVED（"strong targets for out of memory killing"）被杀概率显著更低。
- 回退条件：真机验证出现异常（频繁冷启动/通知丢失）则整体摘除 MainActivity 的策略段，回退"退出护栏 + 稳态退避"两件套。

### 4. 排除项（生态无先例，不翻案）

- "后台销毁 WebView、可见重建"：无生态先例（Tauri/Capacitor/Ionic 均无），且踩 tauri #15671 未修 bug（进程活过 Activity 后重建 WebView 白屏，正是本项目 "前台服务保活进程"同款架构场景）；
- `freeMemory()`（API 18 起 no-op）、`largeHeap`（与后台省内存无关）、`onStop` 销毁 WebView（同上无先例）。

### 5. 配套默认值迁移

`config_schema_version` 当前新装即 `7`（config_state.rs:197）。迁移链：`migrate_legacy_defaults`（config_state.rs:355-417）由 `load_from` 触发（config_state.rs:326-336）。

- v5→v6（config_state.rs:393-401）：`latency_test_interval == 60_000`（旧默认）→ `600_000`，用户显式设过的其他值不动。当前默认 `latency_test_interval: 600_000`（config_state.rs:170）。回归测试 `迁移_v5质量间隔旧默认刷v6且用户值不被覆盖`（config_state.rs:727-756）验证迁移及用户值不被覆盖。
- v6→v7（config_state.rs:402-416）：夜切开关 `enable_night_operator_switch` 旧默认 false→true（config_state.rs:403-405）；定时登录/注销禁用值 `0`→`1440`（config_state.rs:406-411）。注意：2026-10-02 起 `Settings::default` 中 `enable_night_operator_switch` 已改回 `false`（config_state.rs:141），但存量已落盘的 true 不迁移（无新 schema 版本），由用户在设置页自行关闭；默认值断言见 `默认值_业务语义` 测试（config_state.rs:632-664，其中 line 647 断言 `!enable_night_operator_switch`）。
- 其他历史迁移（v0→v1 间隔 15s→60s，v1→v2 自动化开关刷开，v2→v3 质量检测默认关，v3→v4 闲时巡检间隔补 300_000，v4→v5 检测时段终点 0→1380）见 config_state.rs:355-392，不在本决策核心但同函数内。

安卓前端的 `autoExitAfterLogin`/`autoExitOnOnline` 幽灵常量（后端 Settings 无此字段）与桌面新默认对齐为 false，防两套前端默认值分叉。

## 摘除记录（2026-10-01，v2.4.0——第 3 节决策反转）

**触发**：v2.3.8 发布后用户真机实测「长时间挂后台 → 回前台整页空白且无法操作」，无任何崩溃日志。取证指纹确认走 renderer 死亡路径：`adb logcat -d | grep -i "killing application"` 出现 chromium tag 的 "Render process (pid) kill (OOM or update) wasn't handed by all associated webviews, killing application."。

**第 3 节三个风险假设全部证伪**（本地逐文件核实源码证据）：

1. **「BOUND 温和档，被杀概率显著更低」不成立**：AOSP `AwContents.java:4106-4123` 中 `waivedWhenNotVisible=true` 时不可见态 renderer importance 与 WAIVED 同档（NORMAL），BOUND 的保护只存在于可见态——而后台回收恰恰发生在不可见态。HyperOS 等 OEM 后台回收激进，长挂后台几乎必现。
2. **「renderer 自身 crash 与本策略无关」误导**：策略改变的是 renderer 被系统 OOM 回收的概率，而 wry 0.55.1 全链路没有 `onRenderProcessGone` 处理（`~/.cargo/registry/.../wry-0.55.1/src/android/kotlin/RustWebViewClient.kt` 仅 5 个覆写、`RustWebView.kt` 亦无；生成类非 open 且不可换 `setWebViewClient`），于是任何 renderer 死亡（含策略新增的 OOM 回收面）都落入 chromium `aw_browser_terminator.cc:127` 的 `kill(getpid(), SIGKILL)`——宿主进程被静默自杀，无 tombstone。
3. **「START_STICKY 冷启动自愈」不成立**：`ForegroundService.kt:245` 的 START_STICKY 只重启 Service，不重建 Activity/WebView；半残进程里回前台命中 tauri #15671 一族白屏冻结。自动恢复 renderer 是 iOS 独有行为。

**生态旁证**：wry 上游、Capacitor 从不调用 `setRendererPriorityPolicy`（Capacitor 仅提供 onRenderProcessGone 钩子），Cordova 退后台只 `onPause`+`pauseTimers`；官方 Managing WebView 文档 Warning：不要动 renderer 优先级除非配套 Termination Handling API，且 `reload()` 救不活、必须销毁重建。退后台 CPU/功耗抑制 wry 已内建（`WryActivity.kt:130-141` 调 `mWebView.onPause()/onResume()`），本策略买到的只有 renderer 常驻内存，代价即上述死亡链。

**处置**：整体摘除 `MainActivity.kt` 的 `onWebViewCreate` 策略段（连同 Build/WebView import），回退为 wry 默认 IMPORTANT 常驻。当前文件仅 14 行：`class MainActivity : TauriActivity()` 只保留 `onCreate` 内 `enableEdgeToEdge()`（MainActivity.kt:10-13，import 仅 `android.os.Bundle` 与 `androidx.activity.enableEdgeToEdge`，MainActivity.kt:3-4），代码处留 3 行注释指向本文档防回添（MainActivity.kt:4-6）。回退目标「退出护栏 + 稳态退避」两件套不受影响。上游具备 renderer 恢复机制（tauri #15678，RunEvent::Resumed 重建 webview，未合并）之前不再考虑降优先级省内存。

**教训**：①优化 WebView 内核行为必须配套死亡处理链，官方文档 Warning 是硬约束不是建议；②真机验证当时只对比了 `dumpsys meminfo` 内存数字，未覆盖「长后台 → 回前台」生命周期场景——内存收益的验证设计漏掉了该优化唯一的风险面。

## 影响与约束

- 网络探测频率**不影响**进程被杀概率（官方进程优先级排名因素里没有网络活动），稳态退避的收益在功耗与流量，不要宣称改善驻留。
- 平台专属能力（桌面无对应物），双端同步铁律例外条款。
- 验证：`dumpsys meminfo com.campuslogin.client` 对比 app 进程与 `sandboxed_processX` 前后台内存；划掉任务后常驻通知仍在、巡检日志继续（见 lib.rs:125-132 护栏与 quality_cmds.rs:103-134 循环）。

## Connections

[[android-keepalive-fgs-architecture]]、[[android-power-three-fixes]]、[[lightweight-mode-desktop]]、[[config-and-persistence]]