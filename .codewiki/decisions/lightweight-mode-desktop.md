---
title: "桌面轻量化模式：销毁 WebView、EcoQoS 与退出守卫"
type: decision
source_files:
  - tauri-app/src-tauri/src/app/lightweight.rs
  - tauri-app/src-tauri/src/app/shutdown.rs
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/app/window.rs
  - tauri-app/src-tauri/src/platform/ecoqos.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/commands/system.rs
  - tauri-app/frontend/src/settings/SettingsPanel.tsx
tags: [决策, 轻量化, EcoQoS, 托盘, WebView2, 双端同构]
---

## 背景

夜切功能（[[night-operator-switch]]）要求进程整夜常驻（23:00 切换、06:30 恢复），但
`autoExitAfterLogin` 默认 true 会让夜切登录成功后进程退出；普通用户打游戏时后台
WebView 占数百 MB 且调度可能影响游戏延迟。2026-09-20 需求方确认引入「轻量化模式」：
点 X 销毁前端界面、Rust 后端 + 托盘常驻、自动开 Windows 效率模式并延长检测间隔，
从托盘重建（接受 1~3s 启动延迟）。

## 决策

### 1. 轻量化 = 窗口真销毁（不 hide、不挂起）

`lightweight_mode` 配置字段（默认 true，桌面独有平台能力）开启时，`handle_window_close_event`
的关闭分支**不 `prevent_close`**，让 Tauri 真销毁窗口——WebView2 渲染进程、浏览器进程
全部退出（Microsoft 官方：`CoreWebView2Controller.Close` 语义，最后一个 webview 销毁时
browser/GPU/audio 进程组一并退出）。销毁前 `capture_geometry` 记窗口几何，置位
`LIGHTWEIGHT_ACTIVE` 与单次退出守卫，`set_ecoqos(true)`。

**排除项**（调研结论，不得翻案）：
- `TrySuspend`（`ICoreWebView2_3`，非 `_6`）：与本项目已用的 `SetMemoryUsageTargetLevel`
  （`ICoreWebView2_19`）混用被官方明确禁止（`put_MemoryUsageTargetLevel` 文档原文
  "It is not advisable to mix them"）；且收益不如销毁（renderer 进程仍在）。
- `IGNORE_TIMER_RESOLUTION`：Win11 已自动处理，显式设置会让检测循环计时器合并漂移。
- 禁用 WebView2 GPU：官方性能文档反对（仅排障用）。
- IDLE/BELOW_NORMAL 优先级类：IDLE 定义"系统空闲才运行"，游戏满载时巡检会被饿死。

### 2. 单次退出守卫（双标志语义）

Tauri 默认"所有窗口销毁即退出"；无脑对每个 `ExitRequested` 调 `prevent_exit()` 会破坏
系统关机（zebar 作者实测反馈，tauri #13511）。`app/lightweight.rs` 两个标志分工：
- `EXPECT_LIGHTWEIGHT_EXIT`（单次）：由轻量化关闭的 CloseRequested 置位，
  `take_lightweight_exit_guard()` swap 消费；
- `ExitRequested` 拦截条件：`should_prevent_exit(expecting, is_quitting) = expecting && !is_quitting`
  ——托盘"退出"走 `graceful_exit`（`is_quitting=true`）不拦，系统关机同理。

接入点：`startup.rs` 的 run 从 `Builder::run(generate_context!)` 单段式改为
`build()` + `app.run(callback)` 两段式（`Builder::run` 没有 RunEvent 回调入口）。

### 3. 重建路径

`show_or_rebuild_main(app: &AppHandle)` 统一入口，五处接线（托盘"显示主窗口"、托盘
左键单击、single_instance 主路径与 2s 延迟分支、`show_window` 命令）：窗口在则
`show_and_focus_main`，不在则 `rebuild_main_window`：
- **必须 `WebviewWindowBuilder::from_config`**（用 `WindowBuilder` 会得到无 webview 的
  白窗口，tauri #9307 维护者确认）；
- `available_monitors` 是 AppHandle 固有方法（不在 Manager trait 上），故签名接
  `&AppHandle` 而非泛型 Manager；
- 几何恢复 + 离屏校验（`geometry_is_on_screen` 纯函数，与任一显示器矩形相交即有效，
  否则回退配置默认居中）；
- 防白闪 ready 门：`.visible(false)` 起步（tauri.conf.json 的 main 窗口本就 visible:false），
  前端挂载完成 invoke 新命令 `notify_window_ready`（`signal_window_ready` 置位），后端
  500ms 轮询收信号或 5s 超时兜底才 `show()`；
- 重建成功即清 `LIGHTWEIGHT_ACTIVE` + `set_ecoqos(false)`。

心跳线程对"窗口不存在"天然空转安全（`get_webview_window` None → continue）；
`window_safety` 兜底线程只在启动后 9s 内活动，与运行期销毁/重建无冲突。

### 4. EcoQoS（`platform/ecoqos.rs`）

内联 ~30 行 `SetProcessInformation(GetCurrentProcess(), ProcessPowerThrottling, ...)`，
官方三态写法（开 = `ControlMask=StateMask=EXECUTION_SPEED`，关 = 全 0 交还系统），
`Version = PROCESS_POWER_THROTTLING_CURRENT_VERSION`（传 0 静默失败）、伪句柄不
`CloseHandle`。`windows` crate 0.58 已有 `Win32_System_Threading` feature，未加依赖。
**仅在轻量化期间开启**：WebView2 子进程继承宿主节流状态，窗口存在时开启会拖慢前端。
参考实现 qiin2333/sunshine-control-panel `src-tauri/src/power.rs`。

### 5. 检测间隔延长（下限语义）

`effective_background_interval_ms`（下限 300s）/`effective_quality_interval_ms`（下限
1800s）纯函数：`max(配置值, 下限)`——用户显式设过更大值则保持更大值。后台检测循环本就
每拍重读，质量循环顺带从"spawn 时固定 interval"改为每轮重读（修复"设置页改质量间隔
不重启不生效"的既有缺陷）。夜切 30s 循环不变（纯本地时钟判定，零网络开销）。

## 备选方案

- 点 X 保持 hide（minimize_to_tray 现状）：WebView2 全进程组驻留内存，收益不足；
- TrySuspend 挂起：见排除项；
- 独立 helper 进程跑后端：双份 tokio runtime、进程间状态同步成本远超收益；
- EcoQoS 叠加 BELOW_NORMAL：任务管理器绿叶标需要两者同时成立（OpenAnime-Desktops
  实测注释），但 IDLE 语义会饿死巡检，产品上选择"无叶子标但不被饿死"。

## 影响与约束

- 双端同步铁律例外条款：`lightweight_mode`/EcoQoS/托盘重建/ready 门均为桌面平台专属，
  安卓 Settings 无此字段；`notify_window_ready` 在安卓 `tauriApi` 走 `desktopOnly` reject。
- 存量配置不迁移 `lightweight_mode`（容器级 serde default 自动补 true）。
- 真机验证清单（点 X 后 WebView2 进程组消失、叶子标出现、托盘重建、间隔恢复）见
  changelog CHANGELOG_v2.3.8。

## Connections

[[desktop-app-lifecycle]]、[[config-and-persistence]]、[[background-check-and-auto-login]]、
[[night-operator-switch]]、[[android-exit-guard-renderer-policy]]
