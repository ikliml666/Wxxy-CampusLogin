---
title: "安卓后台留存：退出护栏、质量循环稳态退避与 renderer 优先级放行"
type: decision
source_files:
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/quality_cmds.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/MainActivity.kt
tags: [决策, 安卓, 后台内存, 保活, 前台服务, 双端同构]
---

## 背景

安卓端后台内存与功耗优化（2026-09-20，与桌面轻量化模式同一轮）。此前
[[android-keepalive-fgs-architecture]] 已定 FGS 保活架构与省电细节，遗留两项
"未实施备查"：质量循环稳态退避（12+ 外网目标/60s 为后台功耗流量主力）与
WifiLock 降级；本轮调研新增两项系统性修复。

## 决策

### 1. 退出护栏：`RunEvent::ExitRequested + api.prevent_exit()`（常置）

Tauri/tao 默认"最后窗口关闭 → `process::exit(0)`"会把前台服务一起带走
（tauri #15671 根因分析：tao `src/platform_impl/android/mod.rs` run 循环末尾
`process::exit(exit_code)`）。用户从最近任务划掉 Activity 时 Activity 销毁即触发
该路径，巡检/夜切/定时动作全部消失。修复：`android/src-tauri/src/lib.rs` 的 run
从 `Builder::run` 单段式改为 `build()` + `app.run(callback)`，`ExitRequested` 一律
`prevent_exit()`。安卓语义：划掉任务只关 UI，进程随前台服务常驻；彻底退出走
系统设置（桌面侧的对应机制是轻量化单次守卫，见 [[lightweight-mode-desktop]]，
两端语义不同故实现不同）。

### 2. 质量循环稳态退避

`quality_cmds.rs::latency_loop`：连续 5 拍质量 ∈ {excellent, great, good} → 间隔
翻倍递进（60s→120s→240s→…），封顶 1800s（与桌面轻量化质量间隔下限对齐）；
fair/poor/bad/unknown/busy 任何一拍即清零恢复基础间隔。参数常量
`GOOD_LEVELS`/`BACKOFF_STABLE_STREAK=5`/`BACKOFF_MAX_MS=1_800_000`，计算抽纯函数
`next_backoff_interval_ms`（单测覆盖翻倍/封顶/不变/不低于基础间隔）。
循环改为每轮新建 `tokio::time::interval`（60s~1800s 一拍，重建成本可忽略），
新建后首 tick 吞掉避免连发。

### 3. renderer 优先级放行（BOUND 温和档）

`MainActivity.onWebViewCreate`（wry `WryActivity.kt:56` 开放钩子）调
`webView.setRendererPriorityPolicy(RENDERER_PRIORITY_BOUND, waivedWhenNotVisible=true)`
（API 26+）。WebView 默认策略不分可见性一律 IMPORTANT，不设置则系统永远不会
主动回收 renderer——它是 WebView 内存大头（独立 sandboxed_processX 进程）。

**风险语义重析**（此为放弃计划中"onRenderProcessGone 兜底"路径的依据）：
- AOSP：不覆写 `onRenderProcessGone` 时 renderer 被杀 → 宿主进程陪葬；
  但 wry 生成类 `RustWebViewClient` 为**非 open**（不可继承覆写）且未覆写该回调，
  `setWebViewClient` 替换会丢失 wry 全部关键回调（IPC/URL 拦截），不可行；
- renderer **自身 crash** → 宿主崩溃：与是否设置本策略**无关**（现状即如此，
  策略不改变 crash 概率）——BOUND 不新增任何 crash 面；
- BOUND 新增场景仅是"极端内存压力下 renderer 被系统 OOM 杀 → 宿主连带被杀 →
  `START_STICKY` 前台服务自动重启（冷启动自愈，巡检中断数秒后恢复）"，
  相对 WAIVED（"strong targets for out of memory killing"）被杀概率显著更低。
- 回退条件：真机验证出现异常（频繁冷启动/通知丢失）则整体摘除 MainActivity
  的策略段，回退"退出护栏 + 稳态退避"两件套。

### 4. 排除项（生态无先例，不翻案）

- "后台销毁 WebView、可见重建"：无生态先例（Tauri/Capacitor/Ionic 均无），且踩
  tauri #15671 未修 bug（进程活过 Activity 后重建 WebView 白屏，正是本项目
  "前台服务保活进程"同款架构场景）；
- `freeMemory()`（API 18 起 no-op）、`largeHeap`（与后台省内存无关）、
  `onStop` 销毁 WebView（同上无先例）。

### 5. 配套默认值迁移

`config_schema_version` v5→v6：`latency_test_interval == 60_000`（旧默认）→
`600_000`，用户显式设过的其他值不动；迁移后用户设回不被覆盖（回归测试
`迁移_v5质量间隔旧默认刷v6且用户值不被覆盖`）。安卓前端的
`autoExitAfterLogin`/`autoExitOnOnline` 幽灵常量（后端 Settings 无此字段）与
桌面新默认对齐为 false，防两套前端默认值分叉。

## 影响与约束

- 网络探测频率**不影响**进程被杀概率（官方进程优先级排名因素里没有网络活动），
  稳态退避的收益在功耗与流量，不要宣称改善驻留。
- 平台专属能力（桌面无对应物），双端同步铁律例外条款。
- 验证：`dumpsys meminfo com.campuslogin.client` 对比 app 进程与
  `sandboxed_processX` 前后台内存；划掉任务后常驻通知仍在、巡检日志继续。

## Connections

[[android-keepalive-fgs-architecture]]、[[android-power-three-fixes]]、
[[lightweight-mode-desktop]]、[[config-and-persistence]]
