# 前端注册 onCloseRequested 即触发 Tauri core 无条件 prevent_close，窗口销毁被转嫁给 JS 收尾

## 结论

Tauri v2 中，**前端只要注册过 `getCurrentWindow().onCloseRequested(...)`（或任何 `tauri://close-requested` 的 JS listen），Rust 侧的 CloseRequested 就会被 core 无条件拦截**——与 JS 回调是否调用 `event.preventDefault()` 无关。窗口销毁的责任被转嫁给 JS 异步收尾链（handler resolve 且未 preventDefault 时，@tauri-apps/api 才调 `window.destroy()`）。**收尾链任何一环失败（handler 抛异常/永不 resolve/destroy invoke 失败），窗口就永久留存，且无任何日志**。

## 机制（源码级）

1. tauri-2.11.0 `src/manager/window.rs:170-174`（core 的窗口事件内部分发）：

```rust
WindowEvent::CloseRequested { api } => {
    if window.has_js_listener(WINDOW_CLOSE_REQUESTED_EVENT) {
        api.prevent_close();   // 存在即拦截，不看 JS 回调内容
    }
    window.emit_to_window(WINDOW_CLOSE_REQUESTED_EVENT, &())?;
}
```

2. `api.prevent_close()` 向 `signal_tx` 发送 true；tauri-runtime-wry `src/lib.rs:4438-4460` 的 `on_close_requested` 在同步分发完所有 Rust 处理器后 `if let Ok(true) = rx.try_recv() {} else { on_window_close(...) }`——收到 true 即跳过 `on_window_close`（唯一销毁路径）。
3. JS 侧收尾（@tauri-apps/api 2.12.1 `window.cjs` L2029-2037）：`await handler(evt); if (!evt.isPreventDefault()) await this.destroy()`。`destroy()` 走 `WindowMessage::Destroy` → `on_window_close`，绕过 CloseRequested。
4. tao-0.35.2 平台 `Window` 的 `Drop`（`platform_impl/windows/window.rs:1104`）PostMessage `DESTROY_MSG_ID` → 子类过程 `DestroyWindow`（event_loop.rs:2239）→ `WM_DESTROY` → `Destroyed` 事件 → windows store 移除 → 最后一个窗口时发 `ExitRequested`。`WryWindowDispatcher` 只持 window_id 不持 `Arc<Window>`，`on_window_close` 的 `inner = None` + `surface.take()` 归零引用计数——链路本身可靠，前提是**真的走到 on_window_close**。

## 实测症状（Wxxy-CampusLogin v2.4.2 构建，2026-10-09）

- 用户连点关闭按钮 6 次：日志 6 条「关闭窗口进入轻量化模式」（Rust 处理器每次完整执行：capture_geometry + arm_lightweight_exit_guard + ecoqos + log），最短间隔 0.2s——窗口未销毁才可能继续点。
- 无「已拦截最后一个窗口销毁触发的进程退出」（ExitRequested 未触发——窗口从未真正销毁）；无重建、无错误日志。
- JS 收尾链失败的具体环节无法从应用日志观测（前端 console 不落盘），但 6/6 全部失败 + 13 分钟零后续说明是系统性断裂而非偶发。

## 根因闭环（2026-10-09 代码审核确认：ACL 权限拒绝）

上表「具体环节」已定位——**destroy 的 IPC 调用被 Tauri ACL 拒绝**，证据链：

1. `src-tauri/capabilities/default.json` 仅授予 `core:default`（window 部分只有 getter）、`core:window:allow-start-dragging`、`core:window:allow-toggle-maximize`；desktop.json 只有 shell/global-shortcut/autostart。**无 `core:window:allow-destroy`、无 `allow-close`**。
2. 官方文档（v2.tauri.app/reference/javascript/api/namespacewindow/）：`core:window:default` 只启用 getter，「every method that changes one needs its own permission」；core-permissions 页明确 `core:window:allow-destroy`/`allow-close` 是独立权限。默认「所有插件命令都被阻止」（ACL 文档/窗口自定义文档）。
3. JS `destroy()` → `invoke('plugin:window|destroy')`（window.cjs L1109-1113）→ 未授予 → invoke reject → `onCloseRequested` 包装器的 `await this.destroy()` 抛出（unhandled rejection，仅 webview console 可见）→ 窗口永不销毁。同构案例：tauri issue #8928「webview.close not allowed」。
4. 反向印证：全应用前端唯一能用的窗口变更 API 是 `toggleMaximize()`（App.tsx:274）与 `startDragging()`（TitleBar.tsx:81）——恰好是仅有的两项授权；`isMaximized`/`onResized` 走 default getter/listen 通道。旧 onCloseRequested 的 if 分支里 `getCurrentWindow().close()` 同样未授权——该链路自引入起从未工作过。

推论：修复方向（销毁控制权收回 Rust 自定义命令）与全应用既有架构一致（最小化/隐藏/显示全部走 Rust 命令，前端只持 getter 权限）。若未来前端需要直接调用窗口变更 API，必须在 capabilities 显式加对应 `core:window:allow-*`，否则 invoke 静默拒绝（错误只见于 webview console）。

## 修复约定（本项目）

- **前端永不注册 onCloseRequested / tauri://close-requested 监听**。配置 flush 由关闭发起方就地完成（`App.tsx handleCloseWindow`：flush ≤2s → `invoke('close_window')`），另加 `window blur` 兜底 flush 覆盖 Alt+F4 时序。
- 销毁控制流唯一权威：`src-tauri/src/app/shutdown.rs` 三分支（轻量化放行 / minimize_to_tray prevent+hide / else prevent+graceful_exit）。无 JS 监听时 core 不拦截，轻量化分支「不 prevent_close → wry on_window_close → DestroyWindow → ExitRequested → 守卫拦截」纯 Rust 同步链路闭环。
- 轻量化的窗口簿记（geometry/守卫/ecoqos）在 Rust CloseRequested 处理器内同步完成，不依赖销毁成功与否。

## 排查手法备忘

- 「CloseRequested 处理器执行了但窗口没销毁」先查 `has_js_listener`：`grep -rn "onCloseRequested\|WINDOW_CLOSE_REQUESTED" frontend/src/`。
- 第二必查项：`src-tauri/capabilities/*.json` 是否授予了收尾链用到的 `core:window:allow-close`/`allow-destroy`——未授予的 invoke 拒绝只出现在 webview console（生产不落盘），极易误判为「偶发」。
- Tauri 源码在本地 `~/.cargo/registry/src/index.crates.io-*/tauri-2.11.0/`（core）与 `tauri-runtime-wry-2.11.0/`、`tao-0.35.2/`（窗口生命周期）可直接 grep，勿凭记忆猜内部实现。
- JS 侧 API 真实行为以 `frontend/node_modules/@tauri-apps/api/window.cjs` 为准（文档措辞「接受关闭将照常关闭」掩盖了「接受=JS 主动 destroy」的实现）。
