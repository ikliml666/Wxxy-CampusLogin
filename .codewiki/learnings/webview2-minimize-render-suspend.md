---
title: WebView2 最小化渲染不挂起——宿主必须同步 PutIsVisible（wry 缺口 + backdrop-filter 放大）
type: learning
source_files:
  - tauri-app/src-tauri/src/app/window.rs
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/commands/system.rs
  - tauri-app/frontend/src/index.css
tags: [教训, WebView2, 最小化, GPU 占用, wry, backdrop-filter, 轻量化]
---

## 现象

窗口最小化后 WebView2 进程组仍持续占 CPU 4.9%（渲染进程 2.2% + GPU 进程 2.7%）与 GPU 45.6%（v2.4.1 之后用户报「轻量化模式没有按设计工作」）；两日运行日志无任何轻量化记录——用户日常点的是最小化按钮，而轻量化设计只覆盖关闭窗口（真销毁 WebView）。

## 根因

1. **wry 不在最小化时通知 WebView2**：wry 0.55.1 的 `controller.SetIsVisible` 仅两处调用——创建时 `attributes.visible`（webview2/mod.rs L544）与显式 `set_visible()`（L1477-1487，由 Tauri `hide()`/`show()` 触发）。窗口最小化（Win32 尺寸变化）不触发任何 `PutIsVisible`。
2. **WebView2 默认不做原生遮挡检测**：微软文档要求宿主在窗口不可见时主动 `PutIsVisible(false)`。缺省行为导致页面长期「半挂起」：定时器被节流（页面判定 hidden，60s 兜底轮询失效），但渲染合成仍在派发——rAF/CSS 动画/合成器全速空转。
3. **v2.4.1 的 `backdrop-filter: blur(16px) saturate(150%)`（`.glass-dock`）放大成本**：每帧动画都要重算背后内容的模糊，GPU 陡增到可感知（45.6%），让既有缺口显性化。即「修 webview 问题引出新 bug」的真正机制是同提交的 blur 改动放大旧缺口，`syncOnRestore` 本身无害。

## 解决

- **A · Resized 事件同步可见性**：`on_window_event` 加 `WindowEvent::Resized` 分发 → `window.rs` `handle_window_resized_event`：`is_minimized()` 翻转时（`LAST_MINIMIZED` AtomicBool 去重逐像素高频事件）`with_webview` 调 `controller.SetIsVisible(!minimized)`。最小化挂起渲染，还原恢复。与 `hide()` 路径无冲突（wry 已自带 `SetIsVisible(false)`，且隐藏不触发 Resized）。
- **B · 最小化按钮分流**：`minimize_window` 命令在 `minimize_to_tray=true` 时改 `hide()`（进托盘，贴合配置语义），否则保持最小化（A 兜底）。

## 教训

- **WebView2 的可见性是宿主的义务**：最小化/遮挡时宿主不调 `PutIsVisible(false)`，Chromium 就认为页面可渲染。定时器节流 ≠ 渲染挂起——两者通道不同，「页面半挂起」是 WebView2 长驻应用的常态陷阱。
- **给常驻 UI 加 `backdrop-filter` 前先确认最小化渲染路径**：blur 把每帧合成成本放大数倍，任何「最小化后还在产帧」的缺口都会被它放大到用户可感知。
- **`Resized` 事件逐像素高频触发**：任何挂上去的处理器都要先做状态翻转去重（AtomicBool swap），否则拖拽窗口时 COM 调用风暴。
