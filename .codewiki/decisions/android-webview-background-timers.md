---
title: "安卓 WebView 后台停表与内存压力钩子"
type: decision
source_files:
  - android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/MainActivity.kt
  - android/frontend/src/main.tsx
  - android/frontend/src/face/faceService.ts
tags:
  - android
  - webview
  - background
  - power
---

# 安卓 WebView 后台停表与内存压力钩子（android-webview-background-timers）

- 日期：2026-10-25
- 分支：perf/android-webview-background

## 背景（System WebView 机制调研结论）

- wry 壳在 Activity `onPause` 调 `WebView.onPause()`（见 MainActivity.kt 第 24 行注释：WebView.onPause()（wry 在 Activity onPause 调）），但官方文档明确 `onPause()` **不暂停 JS 定时器**（仅 best-effort 停动画/地理定位等）；后台时页面全部 `setInterval` 仍按 Chromium 节流拍唤醒，是后台空耗电主因（MainActivity.kt 第 24-25 行）。
- 真正停表是 `pauseTimers()`：**应用级全局**（暂停本应用所有 WebView 的 layout/parsing/JS timers）。本应用单 WebView 单 Activity，全局等价于本应用，无误伤面（MainActivity.kt 第 26-27 行）。
- 内存压力：Android WebView 没有 WebView2 式 `MemoryUsageTargetLevel`；系统信号是 `onTrimMemory`，需要原生侧转发给前端（MainActivity.kt 第 38-39 行注释）。
- 渲染进程降权不做：已有定案 android-exit-guard-renderer-policy（MainActivity.kt 第 8-10 行注释：有意不设 renderer 优先级策略（wry 默认 IMPORTANT 常驻）：降权放行会让后台渲染进程被系统回收，而 wry 0.55.1 无 onRenderProcessGone 处理，chromium 随即 SIGKILL 静默杀宿主进程（回前台白屏冻结），见 .codewiki/decisions/android-exit-guard-renderer-policy.md）。

## 决策

1. **停表挂 `onStop/onStart` 而非 `onPause/onResume`**：`onStop` 才是“完全不可见”；分屏/悬浮窗下可见但不获焦（只走 onPause）时不误停，后台省电目标场景全覆盖。代码见 MainActivity.kt 第 28-31 行（`onStart` 中 `appWebView?.resumeTimers()`）与第 33-36 行（`onStop` 中 `appWebView?.pauseTimers()`）。`onWebViewCreate` 捕获 WebView 引用存 `appWebView`（MainActivity.kt 第 19-22 行）。
2. **`onTrimMemory` 经 `evaluateJavascript` 调 `window.__onTrimMemory(level)`**：不引入插件级 channel 管线；`evaluateJavascript` 直接派发执行，不受 `pauseTimers` 停表影响（MainActivity.kt 第 38-39 行注释，第 42-48 行实现）。前端钩子在 main.tsx 第 123-129 行定义，`level` 为系统 TRIM 常量；阈值判断在 Kotlin 侧已完成（MainActivity.kt 第 44 行：`if (level < ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW) return`），仅高压力级别会到达前端，与后台省电目标一致（MainActivity.kt 第 40-41 行）。前端钩子动态 `import('./face/faceService')` 释放人脸引擎（最大常驻缓存，TFJS 权重）；动态 import 保证不用人脸功能时不拉 chunk（FE-A-04，见 main.tsx 第 121-122 行注释）。`faceService` 维护 `__faceEngineLoaded`（faceService.ts 第 28 行声明，第 121 行赋值）/`__faceSessionActive`（faceService.ts 第 31 行声明，第 274/310 行设置）两个 window 标志，钩子据此跳过“从未加载引擎”与“会话进行中”两种场景（main.tsx 第 125 行；后者释放会让轮询打到已 reset 实例直至静默超时，见 main.tsx 第 122-123 行注释、faceService.ts 第 29-30 行注释）。
3. 前端既有机制保持不变：main.tsx 中 `setupCrashRecovery` 已实现按 `visibilitychange` 门控的渲染心跳探测（`isVisible` 判断，第 93-103 行、第 105-113 行 `setInterval` 检测 `isRenderLoopAlive` 及 elapsed）、GSAP 后台暂停（`gsap.globalTimeline.pause()`，第 89 行）等，本次只补上“定时器仍按节流拍空转”这块缺口（通过原生侧 `pauseTimers` 解决）。

## 风险与边界

- `resumeTimers` 必须与 `pauseTimers` 成对（MainActivity.kt 第 30 与 35 行）；若未来出现第二个 WebView（如多窗口），全局停表需重审。
- 停表期间前端定时器不跑：后台检测本来就在 Rust 侧（前台服务）跑，不受影响；回前台 `onStart` 先恢复定时器再可见（MainActivity.kt 第 28-31 行）。
- Kotlin `onWebViewCreate` 捕获 WebView 引用（MainActivity.kt 第 19-22 行），MainActivity 是非生成文件（可安全修改）；gen 目录其余文件不得手改。
