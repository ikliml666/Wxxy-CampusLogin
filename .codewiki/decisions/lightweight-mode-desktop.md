---
title: 桌面轻量化模式：销毁 WebView、EcoQoS 与退出守卫
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

# 桌面轻量化模式：销毁 WebView、EcoQoS 与退出守卫

## 背景

桌面端此前对「关闭窗口」只有两种语义：隐藏到托盘（`minimize_to_tray`，WebView2 与整条渲染栈继续常驻内存）或优雅退出进程。配合夜间接入器 23:00/07:30 的定时自动切换（见 [[night-operator-switch]]），进程需要常驻，但用户游戏时对后台检测的调度抖动与资源占用敏感——隐藏窗口无法释放 WebView2 进程组的渲染资源，直接退出又断了常驻。

2026-09-20（v2.3.8）引入「轻量化模式」：点关闭按钮把窗口**真销毁**（WebView2 进程组退出、窗口对象析构），Rust 后端核心功能（检测循环、自动登录、托盘、单实例）与托盘图标保留；唤回时经统一入口重建窗口。轻量化与「自动登录后退出」正交：`auto_exit_after_login` 默认值于同期从 true 调整为 false（`config/model.rs:249-251`），此后夜切总开关 `enable_night_operator_switch` 默认也调回关闭（`config/model.rs:260-262`，2026-10-02 起），轻量化作为独立的关闭按钮运行期行为继续存在。

## 决策

### 1. 轻量化 = 窗口真销毁（不 hide、不挂起）

字段定义在 `config/model.rs:43-46`：`lightweight_mode`（serde rename `lightweightMode`，`default = "default_true"`，注释标明桌面专属，双端同构配置容器见 [[config-and-persistence]]，容器级 `#[serde(default)]` 在 `model.rs:9`）。`Default` 实现给 `true`（`model.rs:253`），即新配置默认开启。

窗口关闭事件处理在 `app/shutdown.rs:20-56` 的 `handle_window_close_event`，三分支互斥：

1. **轻量化分支（`shutdown.rs:27-40`）**：`config.lightweight_mode` 为 true 时——`capture_geometry` 记录窗口几何（31-33）→ `arm_lightweight_exit_guard` 置单次退出守卫（34）→ `set_lightweight_active(true)`（35）→ `#[cfg(windows)] set_ecoqos(true)`（36-37）→ **不调用 `api.prevent_close()`**，直接 return 让 Tauri 真销毁窗口（WebView2 进程组随之退出）。
2. `minimize_to_tray` 分支（41-46）：`prevent_close` + `hide`。
3. 否则（47-55）：`prevent_close` + `graceful_exit`（`shutdown.rs:8-14`，spawn `shutdown_and_exit` 排空后台任务后退出；47-55 注释记录历史缺陷：直接放行关闭不排空后台任务）。

**排除项**（有意不做）：
- 不用 `hide()` 替代——WebView2 渲染进程不退出，内存不释放；
- 不用 `TrySuspend`/`IGNORE_TIMER_RESOLUTION` 等 WebView2 实验性 API——非公开稳定接口；
- 不禁 GPU 加速——影响前台体验，收益不稳定；
- 不用 `IDLE_PRIORITY_CLASS`——EcoQoS（决策 4）是官方支持的等效手段。

### 2. 单次退出守卫（`expecting && !is_quitting`）

窗口真销毁后，最后一个窗口关闭时 Tauri 会发 `RunEvent::ExitRequested`——**隐式退出**，与用户主动退出无法区分。轻量化状态全部是内存态、不落盘（模块注释 `app/lightweight.rs:1-4`）：

- `LIGHTWEIGHT_ACTIVE`（`lightweight.rs:11`，AtomicBool）
- `EXPECT_LIGHTWEIGHT_EXIT`（`lightweight.rs:15`，AtomicBool，单次守卫）
- `GEOMETRY`（`lightweight.rs:17`，Mutex<Option<WindowGeometry>>；`WindowGeometry` 定义 19-25，存逻辑像素 x/y/width/height）
- `READY_SIGNALS`（`lightweight.rs:88`，Mutex<Vec<(String, u64)>>，前端就绪信号）

守卫是**单次语义**：`arm_lightweight_exit_guard` 置位（35-37），`take_lightweight_exit_guard` 用 `swap` 取走并清零（39-41）；`should_prevent_exit(expecting, is_quitting)` = `expecting && !is_quitting`（45-47）——托盘「退出」菜单走 `is_quitting=true`，照常放行。

接入点在 `app/startup.rs:130-149`：`build` 先起 `tauri::Builder`（130-134），`app.run` 回调（135-149）里 `ExitRequested` → `take_lightweight_exit_guard()`（139）→ 从 `try_state::<AppState>()` 读 `exit.is_quitting`（140-143）→ `should_prevent_exit`（144）→ 拦截则 `api.prevent_exit()`（145-146）。两段式是必须的：`on_window_event`/托盘都需要 builder 先存在（参见 zebar/tauri #13511 的同类踩坑）。

**关闭入口与守卫的交汇（当前代码实际行为）**：前端 X 按钮走 `close_window` 命令（`TitleBar.tsx` onClick → `api.closeWindow` → `commands/system.rs:13-21`）——该命令**不感知 `lightweight_mode`**：`minimize_to_tray=true` 时仅 `hide()`（不触发 CloseRequested，轻量化不介入）；否则先置 `exit.is_quitting=true`（18 行）再 `window.close()`（19 行）→ CloseRequested 仍进轻量化分支（记几何/置守卫/开 ecoqos/不 prevent_close）→ 窗口销毁 → ExitRequested 时 `expecting=true` 但 `is_quitting=true` → `should_prevent_exit=false` → 不拦截，进程优雅退出。即**点 X 在两种 `minimize_to_tray` 取值下都达不到「轻量化常驻」**；常驻实际由不经 `close_window` 的原生关闭路径触发（Alt+F4，以及 `useEventListeners.ts` 的 `onCloseRequested` 处理完待存配置后调 `getCurrentWindow().close()`——两者 `is_quitting=false`，守卫拦截生效）。此出入与 CHANGELOG_v2.3.8「点 X 常驻」的描述不一致，属待修复项；`desktop-app-lifecycle` 模块对 `close_window` 的语义记载（tray 隐藏 / 置 `is_quitting` 后关闭）与此一致。

### 3. 重建路径：`show_or_rebuild_main` 唯一入口

唤回统一走 `app/window.rs:53-62` 的 `show_or_rebuild_main`：窗口存在 → `show_and_focus_main`（41-47，show + set_focus + unminimize）；不存在 → `rebuild_main_window`（67-113）。**五处接线**已验证：托盘「显示主窗口」菜单（`tray.rs:124`）、托盘左键单击（`tray.rs:217`）、`single_instance` 主路径（`startup.rs:38`）与其 2s 延迟分支（`startup.rs:45`，给首实例留初始化时间）、`show_window` 命令（`config_cmd.rs:209-213`，接线 211 行）。

`rebuild_main_window` 关键点：
- **必须 `WebviewWindowBuilder::from_config`**（77 行）——`tauri.conf.json` main 窗口配置 `visible:false`，从重建起就是隐藏的，避免白闪；
- 几何恢复（79-97）：`take_geometry` 取回决策 1 存的几何，`AppHandle::available_monitors`（80-91）校验 `geometry_is_on_screen`（`lightweight.rs:69-75`，与任一显示器矩形相交才算有效）——显示器拓扑变了（拔扩展屏）则离屏回退，`log_warn`（95 行）放弃恢复由 `from_config` 用默认位置；
- 重建成功即**清轻量化标志**（102-104）：`set_lightweight_active(false)` + `set_ecoqos(false)`；
- **ready 门**（106-111）：spawn 等待任务调 `wait_window_ready_and_show`（`lightweight.rs:113-131`）——前端挂载完成 invoke `notify_window_ready`（`commands/system.rs:284-288`，`signal_window_ready(window.label())` 记 `(label, unix_ms)`，90-98 行；注册于 `startup.rs:114`；前端封装 `frontend/src/hooks/tauriApi.ts:244`），等待方 `take_window_ready` 取走即删（100-108）；每 500ms 轮询、deadline 5s（113-131）——超时则 `log_warn "等待前端就绪超时(5s)"` 兜底 show + set_focus + unminimize，防止前端异常时窗口永远不可见。

后台线程对「窗口不存在」的兼容：心跳线程窗口拿不到时 `None → continue`（`monitor/heartbeat.rs:20-32`）；`window_safety` 线程（66-92）首查等 3s、此后每 3s 重试共 3 次（≈9s 内结束）兜底调 `show_and_focus_main`。

### 4. EcoQoS：Windows 效率模式随轻量化启停

`platform/ecoqos.rs:79 行`，纯函数 `throttling_state(enable) -> POWER_THROTTLING_STATE`（38-53）：`Version = POWER_THROTTLING_PROCESS_VERSION`（CURRENT_VERSION）；开 = `ControlMask = StateMask = POWER_THROTTLING_EXECUTION_SPEED`（`EXECUTION_SPEED` 以裸 u32 常量定义，46-47）；关 = 双 mask 归零。`set_ecoqos`（15-30）对 `GetCurrentProcess()` 伪句柄调 `SetProcessInformation(ProcessPowerThrottling)`——伪句柄不 CloseHandle；非 Windows 空实现（32-33），双端同构不破编译。windows crate `0.58` 已内置所需 API（`Cargo.toml:63-76` 的 `Win32_System_Threading` feature），未引新依赖。tests 55-79（windows-only）覆盖三态。

启用时机由调用点控制：仅轻量化分支进入时开（`shutdown.rs:36-37`）、重建清标志时关（`window.rs:102-104`）——**不长期开启**，避免影响前台渲染性能。写法参考 sunshine-control-panel。

### 5. 检测间隔延长（下限语义，非覆盖）

轻量化期间网络状态检测 ≥300s、质量检测 ≥1800s，实现在 `lightweight.rs:78-85` 两个纯函数：`effective_background_interval_ms`（78-80，`configured.max(300_000)`）、`effective_quality_interval_ms`（83-85，`configured.max(1_800_000)`）——**max 下限语义**，用户自设更长间隔不被覆盖；非轻量化时原样返回。

两个消费循环均每轮重读配置与轻量化系数（各自带「interval 在 spawn 时捕获一次、运行中改间隔不生效」的历史缺陷修复注释）：后台检测 `monitor/background_task.rs:34-36`（`cfg.background_check_interval.max(10_000)`，注释 25-29）；质量检测 `monitor/latency.rs:69-71`（`latency_test_interval.max(10_000)`，计时器按需重建 `desired_ms != current_interval_ms` + `MissedTickBehavior::Delay`，重建后首个 tick 吞掉，注释 58-61）。夜切循环 30s 节奏不受此影响（见 [[night-operator-switch]]）。

## 备选方案

- **hide 窗口**：实现最简，但 WebView2 进程组不退出，与「轻量化」目标相悖，故只作为 `minimize_to_tray` 分支保留。
- **TrySuspend 挂起 WebView2**：ICoreWebView2_3 实验性 API，非稳定接口，跨 WebView2 Runtime 版本风险大。
- **独立 helper 进程承载后台任务**：架构改动最大，托盘/单实例/配置状态都要跨进程同步，收益不成比例。
- **EcoQoS + BELOW_NORMAL 优先级组合**：效果叠加有限，且改优先级影响退出前的收尾速度，未采用。

## 影响与约束

- **双端例外**：`notify_window_ready` 仅桌面注册；安卓端 `android/frontend/src/hooks/tauriApi.ts:320` 对其 `desktopOnly` reject，双端共享 `tauriApi.ts` 靠可选命令面兼容。
- **存量配置不迁移**：`lightweight_mode` 依赖容器级 `#[serde(default)]`（`model.rs:9`）+ 字段级 `default_true`（45 行），旧配置文件反序列化即得 true，无需迁移脚本。
- **设置页开关**：`frontend/src/settings/SettingsPanel.tsx:413-423`（「窗口与界面」卡，400-459），`checked={config.lightweightMode || false}`，切换即 `onUpdateConfig({lightweightMode})` 走统一配置持久化。
- **当前关闭入口的行为出入**：X 按钮经 `close_window`（`system.rs:13-21`）达不到轻量化常驻（见决策 2 的交汇分析），真机验收时需同时核对 Alt+F4 路径；修复前 `minimize_to_tray=true` 的 X 行为是隐藏托盘。
- 真机验证清单（WebView2 进程消失、任务管理器效率模式叶标、托盘重建）见 `changelogs/CHANGELOG_v2.3.8.md`。

## Connections

- [[desktop-app-lifecycle]]——窗口关闭事件、退出状态机与托盘是轻量化的宿主流程。
- [[config-and-persistence]]——`lightweight_mode` 字段与 serde default 迁移策略。
- [[background-check-and-auto-login]]——检测循环的间隔下限消费方。
- [[night-operator-switch]]——轻量化与夜切常驻的互补关系。
- [[android-exit-guard-renderer-policy]]——安卓端的退出拦截与渲染策略对照。
