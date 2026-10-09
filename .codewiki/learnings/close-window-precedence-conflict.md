---
title: 关闭按钮轻量化被短路——命令层与事件层的配置优先级矛盾（重复分支所有权的代价）
type: learning
source_files:
  - tauri-app/src-tauri/src/commands/system.rs
  - tauri-app/src-tauri/src/app/shutdown.rs
tags: [教训, 轻量化, 配置优先级, Tauri, 重复所有权, is_quitting]
---

## 现象

用户配置 `lightweight_mode=true + minimize_to_tray=true`，日常点标题栏**关闭按钮**，轻量化从未触发（日志多日无 lightweight 记录、WebView2 进程组常驻）。v2.4.1 安装版复现确认。

## 根因

窗口「关闭」语义存在**两个 owner**，且优先级相反：

1. **命令层**（标题栏关闭按钮）：`TitleBar.tsx` → `App.tsx:401` → `invoke('close_window')` → `system.rs close_window` 先判 `minimize_to_tray` → `hide()`。`CloseRequested` 不触发，`shutdown.rs` 的轻量化分支永远不可达。
2. **事件层**（Alt+F4 / 系统关闭）：直达 `CloseRequested` → `shutdown::handle_window_close_event`，轻量化优先（放行销毁 + 退出守卫）。

同一组配置，两条路径行为相反。更隐蔽的一层：只开轻量化的用户点关闭按钮会走命令层 else 分支**先置位 `is_quitting`**（全局真退出标志，被 `auth/protocol.rs` 重试循环、`monitor/*` 调度、`network/quality.rs` 等几十处后台任务消费）再 `close()`——`startup.rs:148` 的 `should_prevent_exit(expecting, is_quitting)` 被压制，窗口销毁后进程直接退出而非轻量化待机。即关闭按钮在任何配置组合下都进不了轻量化待机。

## 解决

`close_window` 加轻量化优先分支：`lightweight_mode=true` 直接 `window.close()` 委托 `CloseRequested` → `shutdown.rs` 轻量化分支（不置 `is_quitting`）；托盘分流与 else 分支原样保留。修复后行为矩阵：关闭按钮/Alt+F4 → 轻量化待机；最小化按钮 → 托盘隐藏/任务栏最小化；托盘退出 → 真退出。

## 教训

- **「窗口关闭」语义只能有一个 owner**：命令层复制事件层的分支判断，漂移（优先级相反）只是时间问题。命令要做的是触发 `window.close()`，让 `CloseRequested` 处理器独占地决定语义。
- **全局退出标志（`is_quitting`）不是普通的 boolean**：它被几十个后台循环当取消信号消费。任何提前置位都会把「待机」变成「真退出」。进入轻量化/托盘路径时绝不能碰它。
- **配置组合的优先级要在设计期显式定案**，并让所有执行路径共享同一份判定；两条路径各自 if-else 时，矛盾必然出现在「组合配置」这种测试盲区。
