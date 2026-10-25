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

安卓端后台内存与功耗优化（2026-09 起多轮迭代）。此前 [[android-keepalive-fgs-architecture]] 已定 FGS 保活架构与省电细节，遗留两项 "未实施备查"：质量循环稳态退避（12+ 外网目标/60s 为后台功耗流量主力）与 WifiLock 降级；后续调研新增退出护栏与 renderer 优先级策略的系统修复及摘除。

## 决策

### 1. 退出护栏：`RunEvent::ExitRequested + api.prevent_exit()`（常置）

Tauri/tao 默认"最后窗口关闭 → `process::exit(0)`"会把前台服务一起带走（tauri #15671 根因分析：tao `src/platform_impl/android/mod.rs` run 循环末尾 `process::exit(exit_code)`）。用户从最近任务划掉 Activity 时 Activity 销毁即触发该路径，巡检/夜切/定时动作全部消失。修复：`android/src-tauri/src/lib.rs` 的 `run()`（`#[cfg_attr(mobile, tauri::mobile_entry_point)]`，lib.rs:32-33）由 `Builder::run` 单段式改为 `build()` + `run(callback)`：`builder.build(tauri::generate_context!())`（lib.rs:131）后链式 `.run(|_app, event| …)`（lib.rs:133-140），回调内 `if let tauri::RunEvent::ExitRequested { api, .. } = event { api.prevent_exit(); }`（lib.rs:137-139），代码处留注释说明缘由（lib.rs:134-136）。安卓语义：划掉任务只关 UI，进程随前台服务常驻；彻底退出走系统设置（桌面侧的对应机制是轻量化单次守卫，见 [[lightweight-mode-desktop]]，两端语义不同故实现不同）。

### 2. 质量循环稳态退避

`quality_cmds.rs::latency_loop`（quality_cmds.rs:97-128）：连续 5 拍质量 ∈ {excellent, great, good} → 间隔翻倍递进，封顶 1800s（与桌面轻量化质量间隔下限对齐）；fair/poor/bad/unknown/busy 任何一拍即清零恢复基础间隔。参数常量 `GOOD_LEVELS`（quality_cmds.rs:89）/`BACKOFF_STABLE_STREAK=5`（quality_cmds.rs:97）/`BACKOFF_MAX_MS=1_800_000`（quality_cmds.rs:97），计算抽纯函数 `next_backoff_interval_ms`（quality_cmds.rs:95-101：stable_good_streak 每达 5 拍对 current_ms 翻倍 `current_ms.saturating_mul(2).min(BACKOFF_MAX_MS).max(base_ms)`，未达 5 拍维持当前间隔）。单测覆盖翻倍/封顶/不变/不低于基础间隔（quality_cmds.rs:97-130 用例：`稳定计数达5_间隔翻倍_上限封顶` 140、`未达5拍_间隔不变` 150、`翻倍不低于基础间隔` 156、`等级判定_good及以上算稳定` 162）。

循环数据流（当前代码）：

- 基础间隔 = `settings.latency_test_interval.max(10_000)`（quality_cmds.rs:104），来自 `config_state::current_settings`（quality_cmds.rs:19-22 调用）；递进序列随配置值而变（旧默认 60s 时 60s→120s→240s→…；现默认 600s 时 600s→1200s→1800s 封顶，见 config_state.rs:178 默认 `latency_test_interval: 600_000`）。
- 启停与代际守卫：`start_latency_test`（quality_cmds.rs:59-77）幂等（`LATENCY_RUNNING.swap(true)`，quality_cmds.rs:59），读当前配置（quality_cmds.rs:59-67，失败则复位标志并返回错误，quality_cmds.rs:59-64）后取 `LATENCY_GENERATION.fetch_add(1)` 代际值（quality_cmds.rs:71）并 `tauri::async_runtime::spawn(latency_loop(app, settings, gen))`（quality_cmds.rs:59）；`stop_latency_test`（quality_cmds.rs:59-66）先推进代际再清标志（quality_cmds.rs:80-81），避免旧循环误清新一轮运行标志；循环退出前仅当启动代际仍匹配才 `LATENCY_RUNNING.store(false)` 收尾（quality_cmds.rs:59-61）。运行标志为 `lazy_static` 的 `AtomicBool LATENCY_RUNNING`（quality_cmds.rs:11-13），代际为 `static AtomicU64 LATENCY_GENERATION`（quality_cmds.rs:59）。
- 循环每轮「先检测、后 sleep 当前间隔」（quality_cmds.rs:110-128）：不再重建 `tokio::time::interval`，而是 `loop` 内先查运行标志与代际（quality_cmds.rs:111），调用 `run_quality_once`（quality_cmds.rs:114），再据退避状态机更新 `current_ms` 后 `tokio::time::sleep`（quality_cmds.rs:127）。该结构天然支持退避变周期，无首 tick 陷阱。
- 每拍检测汇聚于 `run_quality_once`（quality_cmds.rs:19-49）：读当前配置（quality_cmds.rs:25-27）+ `AndroidState.cached_source_ip`（quality_cmds.rs:25-31）→ 构造恒 false 的 `is_quitting` 退出哨兵（quality_cmds.rs:30）→ `check_network_quality_async("wlan0", &adapter_ip, settings.skip_ttfb_in_latency, settings.skip_content_in_latency, &settings.fixed_gateway, is_quitting, Some(app), false)`（quality_cmds.rs:31-41；quality 内部经 EventBus emit `network-quality-result`，见 quality_cmds.rs:114 调用处注释）→ 质量历史落盘 `quality_history::append`（quality_cmds.rs:25-29，命令与定时循环共此一条路径，每次真实检测记一条供趋势回溯；落盘上限 100 条、负延迟转 null 见 quality_history.rs:12、52-75）。手动命令 `check_network_quality`（quality_cmds.rs:25-28）与定时循环共用该函数。
- 退避状态机（quality_cmds.rs:116-126）：good 及以上 `stable_good_streak` 累加并经纯函数更新间隔；波动/失败一拍即清零恢复基础间隔（累计已满 5 拍时额外打一条 info 日志"网络质量波动，检测间隔 … 恢复 …"，quality_cmds.rs:119-122）。

```text
┌────────────────────┐  start_latency_test (56)  ┌────────────────────┐
│ LATENCY_RUNNING    │ ─── swap(true) (59) ───►  │ LATENCY_GENERATION │
│ (AtomicBool 11-13) │ ─── fetch_add(1) (71) ──► │ (AtomicU64 17)     │
└────────────────────┘                          └────────────────────┘
                                                       │ spawn (72)
                                                       ▼
┌────────────────────┐  loop (110)  ┌──────────────────────────────────┐
│ stop_latency_test  │ ── gen+1(80) │ latency_loop (103)               │
│ (76)              │ ── RUNNING=F │ ├─ check RUNNING & gen (111)      │
└────────────────────┘             │ ├─ run_quality_once (114)         │
                                    │ ├─ backoff state machine (116-126)│
                                    │ └─ sleep(current_ms) (127)        │
                                    └──────────────────────────────────┘
```

### 3. renderer 优先级放行（IMPORTANT 常驻，不降权）——**已整体摘除降权策略，见文末「摘除记录」**

`MainActivity.kt` 当前在 8-10 行以注释形式固定决策："有意不设 renderer 优先级策略（wry 默认 IMPORTANT 常驻）：降权放行会让后台渲染进程被系统回收，而 wry 0.55.1 无 onRenderProcessGone 处理，chromium 随即 SIGKILL 静默杀宿主进程（回前台白屏冻结），见 .codewiki/decisions/android-exit-guard-renderer-policy.md。" 不再调用 `setRendererPriorityPolicy` 降权。

当前 `MainActivity.kt` 共 49 行（1-49），除保留 wry 默认 IMPORTANT 常驻外，补充了后台省电生命周期钩子：
- `onCreate`（MainActivity.kt:14-17）调 `enableEdgeToEdge()`（MainActivity.kt:15）；
- `onWebViewCreate`（MainActivity.kt:19-22）持有 `appWebView` 引用供后续暂停/恢复；
- `onStart` / `onStop`（MainActivity.kt:28-36）调 `resumeTimers()`（MainActivity.kt:30）/`pauseTimers()`（MainActivity.kt:35）抑制后台 JS 定时器空耗电；
- `onTrimMemory`（MainActivity.kt:42-48）在 `TRIM_MEMORY_RUNNING_LOW`（MainActivity.kt:44）以上时 `evaluateJavascript("window.__onTrimMemory...")`（MainActivity.kt:45-47）转发内存压力给前端释放大缓存。

原计划的 `WebView.setRendererPriorityPolicy(RENDERER_PRIORITY_BOUND, waivedWhenNotVisible=true)` 风险语义重析（见文末摘除记录）：wry 0.55.1 无 `onRenderProcessGone` 处理，renderer 死亡即宿主 SIGKILL 白屏冻结；回退目标「退出护栏 + 稳态退避 + 新增 pauseTimers/onTrimMemory 省电」不受影响。

### 4. 排除项（生态无先例，不翻案）

- "后台销毁 WebView、可见重建"：无生态先例（Tauri/Capacitor/Ionic 均无），且踩 tauri #15671 未修 bug（进程活过 Activity 后重建 WebView 白屏，正是本项目 "前台服务保活进程"同款架构场景）；
- `freeMemory()`（API 18 起 no-op）、`largeHeap`（与后台省内存无关）、`onStop` 销毁 WebView（同上无先例）。

### 5. 配套默认值迁移

`Settings` 结构体（config_state.rs:26-136）采用 `#[serde(rename_all = "camelCase", default)]`，涵盖账号凭据（user/password/operator 等，config_state.rs:30-43）、行为界面（auto_login_on_start/theme_mode 等，config_state.rs:74-95）、网络定时（portal_url/scheduled_login_minutes 等，config_state.rs:103-115）、更新日志（update_source/log_retention_days 等，config_state.rs:117-128）；`Default` 实现见 config_state.rs:138-211。`config_schema_version` 当前新装即 `7`（config_state.rs:208）。迁移链：`migrate_legacy_defaults`（config_state.rs:366-428）由 `load_from` 触发（config_state.rs:337-347，调用在 339）。

- v5→v6（config_state.rs:404-412）：`latency_test_interval == 60_000`（旧默认）→ `600_000`，用户显式设过的其他值不动。当前默认 `latency_test_interval: 600_000`（config_state.rs:178）。回归测试 `迁移_v5质量间隔旧默认刷v6且用户值不被覆盖`（config_state.rs:752-781，函数名 753）验证迁移及用户值不被覆盖。
- v6→v7（config_state.rs:413-427）：夜切开关 `enable_night_operator_switch` 旧默认 false→true（config_state.rs:414-416）；定时登录/注销禁用值 `0`→`1440`（config_state.rs:417-422）。注意：2026-10-02 起 `Settings::default` 中 `enable_night_operator_switch` 已改回 `false`（config_state.rs:149），但存量已落盘的 true 不迁移（无新 schema 版本），由用户在设置页自行关闭；默认值断言见 `默认值_业务语义` 测试（config_state.rs:653-689，其中 668 行断言 `!enable_night_operator_switch`）。
- 其他历史迁移（v0→v1 间隔 15s→60s 在 368-370，v1→v3 自动化开关刷开与质量默认关在 371-385，v3→v4 闲时巡检间隔补 300_000 在 386-394，v4→v5 检测时段终点 0→1380 在 395-403）见 config_state.rs:366-428，不在本决策核心但同函数内。

## 摘除记录（2026-10-01 起演进，v2.4.0+——第 3 节决策反转与配套补全）

**触发**：v2.3.8 发布后用户真机实测「长时间挂后台 → 回前台整页空白且无法操作」，无任何崩溃日志。取证指纹确认走 renderer 死亡路径：`adb logcat -d | grep -i "killing application"` 出现 chromium tag 的 "Render process (pid) kill (OOM or update) wasn't handed by all associated webviews, killing application."。

**第 3 节三个风险假设全部证伪**（本地逐文件核实源码证据）：

1. **「BOUND 温和档，被杀概率显著更低」不成立**：AOSP `AwContents.java:4106-4123` 中 `waivedWhenNotVisible=true` 时不可见态 renderer importance 与 WAIVED 同档（NORMAL），BOUND 的保护只存在于可见态——而后台回收恰恰发生在不可见态。HyperOS 等 OEM 后台回收激进，长挂后台几乎必现。
2. **「renderer 自身 crash 与本策略无关」误导**：策略改变的是 renderer 被系统 OOM 回收的概率，而 wry 0.55.1 全链路没有 `onRenderProcessGone` 处理（`~/.cargo/registry/.../wry-0.55.1/src/android/kotlin/RustWebViewClient.kt` 仅 5 个覆写、`RustWebView.kt` 亦无；生成类非 open 且不可换 `setWebViewClient`），于是任何 renderer 死亡（含策略新增的 OOM 回收面）都落入 chromium `aw_browser_terminator.cc:127` 的 `kill(getpid(), SIGKILL)`——宿主进程被静默自杀，无 tombstone。
3. **「START_STICKY 冷启动自愈」不成立**：`ForegroundService.kt:245` 的 START_STICKY 只重启 Service，不重建 Activity/WebView；半残进程里回前台命中 tauri #15671 一族白屏冻结。自动恢复 renderer 是 iOS 独有行为。

**生态旁证**：wry 上游、Capacitor 从不调用 `setRendererPriorityPolicy`（Capacitor 仅提供 onRenderProcessGone 钩子），Cordova 退后台只 `onPause`+`pauseTimers`；官方 Managing WebView 文档 Warning：不要动 renderer 优先级除非配套 Termination Handling API，且 `reload()` 救不活、必须销毁重建。退后台 CPU/功耗抑制 wry 已内建（`WryActivity.kt:130-141` 调 `mWebView.onPause()/onResume()`），本策略买到的只有 renderer 常驻内存，代价即上述死亡链。

**处置**：整体摘除 `MainActivity.kt` 中 `setRendererPriorityPolicy` 降权策略段，回退为 wry 默认 IMPORTANT 常驻。当前 `MainActivity.kt` 为 49 行（1-49）：`class MainActivity : TauriActivity()`（MainActivity.kt:11）保留 `onCreate` 内 `enableEdgeToEdge()`（MainActivity.kt:14-17），并保留 `onWebViewCreate` 钩子（MainActivity.kt:19-22）以获取 WebView 引用；新增/保留了 `onStop`→`pauseTimers()`（MainActivity.kt:33-36）与 `onTrimMemory`→`evaluateJavascript` 转发（MainActivity.kt:42-48）作为后台定时器与内存压力省电补充。第 8-10 行以 3 行注释指向本文档防回添降权策略。回退目标「退出护栏 + 稳态退避」两件套不受影响，且因 `pauseTimers` 与 `onTrimMemory` 的加入，后台功耗抑制更完备。上游具备 renderer 恢复机制（tauri #15678，RunEvent::Resumed 重建 webview，未合并）之前不再考虑降优先级省内存。

**教训**：①优化 WebView 内核行为必须配套死亡处理链，官方文档 Warning 是硬约束不是建议；②真机验证当时只对比了 `dumpsys meminfo` 内存数字，未覆盖「长后台 → 回前台」生命周期场景——内存收益的验证设计漏掉了该优化唯一的风险面。

## 影响与约束

- 网络探测频率**不影响**进程被杀概率（官方进程优先级排名因素里没有网络活动），稳态退避的收益在功耗与流量，不要宣称改善驻留。
- 平台专属能力（桌面无对应物），双端同步铁律例外条款。
- 验证：`dumpsys meminfo com.campuslogin.client` 对比 app 进程与 `sandboxed_processX` 前后台内存；划掉任务后常驻通知仍在、巡检日志继续（见 lib.rs:133-140 护栏与 quality_cmds.rs:103-134 循环）。

## Connections

[[android-keepalive-fgs-architecture]]、[[android-power-three-fixes]]、[[lightweight-mode-desktop]]、[[config-and-persistence]]