# 安卓 WebView 后台停表与内存压力钩子（android-webview-background-timers）

- 日期：2026-10-25
- 分支：perf/android-webview-background

## 背景（System WebView 机制调研结论）

- wry 壳在 Activity `onPause/onResume` 已调 `webView.onPause()/onResume()`，但官方文档明确 `onPause()` **不暂停 JS 定时器**（仅 best-effort 停动画/地理定位等）；后台时页面全部 `setInterval` 仍按 Chromium 节流拍（≥1s）唤醒，是后台空耗电主因。
- 真正停表是 `pauseTimers()`：**应用级全局**（暂停本应用所有 WebView 的 layout/parsing/JS timers）。本应用单 WebView 单 Activity，全局等价于本应用，无误伤面。
- 内存压力：Android WebView 没有 WebView2 式 `MemoryUsageTargetLevel`；系统信号是 `onTrimMemory`，需要原生侧转发给前端。
- 渲染进程降权（waive/bo）不做：已有定案 android-exit-guard-renderer-policy（降权→后台被杀→回前台白屏冻结）。

## 决策

1. **停表挂 `onStop/onStart` 而非 `onPause/onResume`**：onStop 才是"完全不可见"；分屏/悬浮窗下可见但不获焦（只走 onPause）时不误停，后台省电目标场景全覆盖。
2. **`onTrimMemory` 经 `evaluateJavascript` 调 `window.__onTrimMemory(level)`**：不引入插件级 channel 管线；evaluateJavascript 直接派发执行，不受 pauseTimers 停表影响。前端钩子在 main.tsx，level≥10（RUNNING_LOW）动态 import faceService 释放人脸引擎（最大常驻缓存，TFJS 权重）；动态 import 保证不用人脸功能时不拉 chunk（FE-A-04）。
3. 前端既有机制保持不变：心跳/rAF 探测已按 visibility 门控、GSAP 后台 pause、后端检测循环亮屏 60s/闲时 5min 分档——本次只补上"定时器仍按节流拍空转"这块缺口。

## 风险与边界

- `resumeTimers` 必须与 `pauseTimers` 成对；若未来出现第二个 WebView（如多窗口），全局停表需重审。
- 停表期间前端定时器不跑：后台检测本来就在 Rust 侧（前台服务）跑，不受影响；回前台 `onStart` 先恢复定时器再可见。
- Kotlin `onWebViewCreate` 捕获 WebView 引用，MainActivity 是非生成文件（可安全修改）；gen 目录其余文件不得手改。
