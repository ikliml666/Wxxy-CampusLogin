---
title: 安卓白屏：renderer 优先级放行被回收后 chromium 静默 SIGKILL 宿主
type: learning
source_files:
  - android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/MainActivity.kt
tags: [教训, 安卓, 白屏, WebView, 生命周期, 后台内存]
---

## 现象

长时间挂后台后回前台整页空白且无法操作，无任何崩溃日志（无 tombstone、无 crash 文件）。

## 根因

旧版本在 `MainActivity.onWebViewCreate` 调用了 `setRendererPriorityPolicy(RENDERER_PRIORITY_BOUND, waivedWhenNotVisible=true)`（v2.3.8 的“后台内存优化”）：waived=true 时不可见态 renderer importance 与 WAIVED 同档，长后台被系统/OEM 回收；而当时 wry 0.55.1 无 `onRenderProcessGone` 处理，chromium 随即 `SIGKILL` 静默杀掉宿主进程；START_STICKY 前台服务只拉起 Service 不重建 WebView，回前台即白屏冻结（tauri #15671 一族）。

在当前代码 `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/MainActivity.kt` 中，第 8-10 行注释明确记载：

> 有意不设 renderer 优先级策略（wry 默认 IMPORTANT 常驻）：降权放行会让后台渲染进程被系统回收，而 wry 0.55.1 无 onRenderProcessGone 处理，chromium 随即 SIGKILL 静默杀宿主进程（回前台白屏冻结），见 .codewiki/decisions/android-exit-guard-renderer-policy.md。

即当前代码已确认根因并禁用/移除了降权策略（第 19-22 行的 `onWebViewCreate` 中仅将传入的 `webView` 赋给 `appWebView`，不再调用 `setRendererPriorityPolicy`）。

## 解决

整体摘除降权策略段，回退 wry 默认（renderer 不分可见性 IMPORTANT 常驻）。

当前 `MainActivity.kt` 为维护后台体验与省电，采用了**不触发渲染进程回收**的替代方案：

- **定时器管控（`onStart` / `onStop`）**：第 24-27 行注释说明 `WebView.onPause()` 不暂停 JS 定时器，后台 `setInterval` 仍耗电；因此在第 28-31 行 `onStart` 调 `appWebView?.resumeTimers()`，第 33-36 行 `onStop` 调 `appWebView?.pauseTimers()`（应用级全局停表）。挂载在完全不可见的 `onStop/onStart` 而非 `onPause`，避免分屏/悬浮窗下误停。
- **内存压力转发（`onTrimMemory`）**：第 38-41 行注释说明，系统内存压力通过第 42-48 行 `onTrimMemory` 转发给前端 `main.tsx` 的全局钩子 `window.__onTrimMemory` 以释放人脸引擎等大缓存；阈值判断在本侧完成（仅当 `level >= ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW` 才 `evaluateJavascript` 注入，不受 `pauseTimers` 停表影响）。

### 关键组件与行号（`MainActivity.kt`）

- `class MainActivity : TauriActivity()`（第 11 行）：承载 WebView 的宿主 Activity。
- `private var appWebView: WebView? = null`（第 12 行）：持有 WebView 实例。
- `onCreate(savedInstanceState: Bundle?)`（第 14-17 行）：调用 `enableEdgeToEdge()` 后 `super.onCreate`。
- `onWebViewCreate(webView: WebView)`（第 19-22 行）：覆写并调用 `super`，将 `webView` 赋给 `appWebView`。**此处未设 renderer 优先级策略。**
- `onStart()`（第 28-31 行）：`super.onStart()` 后调用 `appWebView?.resumeTimers()`。
- `onStop()`（第 33-36 行）：`super.onStop()` 后调用 `appWebView?.pauseTimers()`。
- `onTrimMemory(level: Int)`（第 42-48 行）：若 `level < ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW` 直接 return；否则执行 `appWebView?.evaluateJavascript("window.__onTrimMemory && window.__onTrimMemory($level)", null)`。

### 架构与数据流

```text
┌──────────────────────────┐  onStop/onStart  ┌──────────────────────┐
│ MainActivity (kt:11)     │ ───────────────► │ WebView              │
│ onStop (kt:33)           │  pause/resume    │ pauseTimers (kt:35)  │
│ onStart (kt:28)          │ ◄─────────────── │ resumeTimers (kt:30) │
└───────────┬──────────────┘                  └──────────────────────┘
            │ onTrimMemory (level>=RUNNING_LOW) (kt:42)
            ▼ evaluateJavascript("window.__onTrimMemory(level)") (kt:45)
┌───────────┴──────────────┐                  ┌──────────────────────┐
│ System (ComponentCallbacks2) │ ──────────► │ main.tsx             │
│ TRIM_MEMORY_RUNNING_LOW  │   内存压力转发    │ window.__onTrimMemory│
└──────────────────────────┘                  └──────────────────────┘
```

### 后台优化手段对比

| 后台优化手段 | 实现位置 | 风险 |
| --- | --- | --- |
| `setRendererPriorityPolicy(..., waived=true)`（旧，已移除） | 原 `onWebViewCreate` | 触发系统回收，无 `onRenderProcessGone` 导致 SIGKILL 白屏 |
| `pauseTimers()` / `resumeTimers()`（当前） | `MainActivity.kt` 第 33-36 / 28-31 行 | 无回收风险，仅暂停 JS 定时器省电 |
| `evaluateJavascript` 内存压力转发 | `MainActivity.kt` 第 42-48 行 | 无回收风险，仅通知前端释放大缓存 |

取证指纹：`adb logcat -d | grep -i "killing application"` 出现 "Render process … wasn't handed by all associated webviews, killing application." 即此路径，修复后（当前代码保持默认优先级）不应再出现。

## 教训

**不要动 WebView renderer 优先级，除非配套 Termination Handling**（官方 Managing WebView 文档 Warning；wry/Capacitor 上游零调用，Cordova 只做 onPause+pauseTimers）。`reload()` 救不活死掉的 WebView，必须销毁重建。优化类改动的真机验证要覆盖它引入的风险场景（本例：长后台 → 回前台），只看内存数字等于没验。

后台省电若需限制 WebView 资源，应使用 `pauseTimers()`（全局停表，如 `MainActivity.kt` 第 35 行）与 `onTrimMemory` 转发（第 42-48 行），**绝不可**通过降权 renderer 优先级实现，否则会触发静默 SIGKILL。

## Connections

[[android-exit-guard-renderer-policy]]、[[android-white-screen-missing-init-fields]]
