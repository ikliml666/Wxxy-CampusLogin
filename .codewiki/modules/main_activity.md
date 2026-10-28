---
title: main_activity
type: module
source_files:
  - android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/MainActivity.kt
  - android/src-tauri/gen/android/app/src/main/AndroidManifest.xml
  - android/frontend/src/main.tsx
  - .codewiki/decisions/android-exit-guard-renderer-policy.md
tags: [安卓, 应用入口, 生命周期, TauriActivity, 后台省电, WebView]
---

## Overview

`MainActivity` 是安卓端 Tauri 应用唯一的 `Activity` 入口（`AndroidManifest.xml:32-44` 声明为 `LAUNCHER` / `LEANBACK_LAUNCHER`，`launchMode="singleTask"`）。它继承 Tauri 生成的 `TauriActivity()`，自身只做三件事：(1) 在 `onCreate` 开启全面屏沉浸式 `enableEdgeToEdge()`；(2) 在 `onWebViewCreate` 持有 `WebView` 引用；(3) 重写 `onStart`/`onStop`/`onTrimMemory` 三个生命周期钩子做后台省电与内存压力转发。

本模块是安卓「前台服务保活 + 后台省电」架构在 Java/Kotlin 侧的落点之一。它刻意**不设** renderer 优先级降权策略（保持 wry 默认 `IMPORTANT` 常驻），以避免长后台后回前台白屏冻结（见 [[android-exit-guard-renderer-policy]]）。

## Key Components

### 类 `MainActivity`
- 声明位置：`MainActivity.kt:11`
- 继承：`TauriActivity()`（Tauri 生成类；`MainActivity.kt:11` 的 `class MainActivity : TauriActivity()`）
- 字段：
  - `appWebView: WebView?`（MainActivity.kt:30）——可空的 WebView 引用，由 `onWebViewCreate` 注入，供 `onStart`/`onStop`/`onTrimMemory` 调用 `resumeTimers()` / `pauseTimers()` / `evaluateJavascript(...)`。

### 公开方法（均为 `override` 生命周期回调）
- `onCreate(savedInstanceState: Bundle?)`（MainActivity.kt:14-17）——先调 `enableEdgeToEdge()`（MainActivity.kt:15），再 `super.onCreate(savedInstanceState)`（MainActivity.kt:16）。顺序为「先开沉浸式，再交回父类初始化」。
- `onWebViewCreate(webView: WebView)`（MainActivity.kt:19-22）——先 `super.onWebViewCreate(webView)`（MainActivity.kt:20），再把传入的 `webView` 存进 `appWebView`（MainActivity.kt:21）。这是获取 WebView 实例的唯一钩子，后续所有后台操作都依赖它。
- `onStart()`（MainActivity.kt:28-31）——先 `super.onStart()`（MainActivity.kt:29），再 `appWebView?.resumeTimers()`（MainActivity.kt:30）。Activity 完全可见时恢复 JS 定时器。
- `onStop()`（MainActivity.kt:33-36）——先 `super.onStop()`（MainActivity.kt:34），再 `appWebView?.pauseTimers()`（MainActivity.kt:25）。Activity 完全不可见时全局停表，抑制后台 `setInterval` 空耗电。
- `onTrimMemory(level: Int)`（MainActivity.kt:42-48）——先 `super.onTrimMemory(level)`（MainActivity.kt:43），当 `level < ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW` 直接 return（MainActivity.kt:44），否则 `appWebView?.evaluateJavascript("window.__onTrimMemory && window.__onTrimMemory($level)", null)`（MainActivity.kt:45-47）把系统内存压力转发给前端全局钩子。

### 被注释/禁用的策略
- **renderer 优先级降权策略（已摘除）**：MainActivity.kt:8-10 以 3 行注释显式写明「有意不设 renderer 优先级策略」。原计划调用 `WebView.setRendererPriorityPolicy(RENDERER_PRIORITY_BOUND, waivedWhenNotVisible=true)` 被整体删除，回退为 wry 默认 `IMPORTANT` 常驻（详见 [[android-exit-guard-renderer-policy]] 第 3 节与「摘除记录」）。

## Architecture

```text
┌───────────────────────────┐  LAUNCHER intent  ┌──────────────────────────┐
│ AndroidManifest.xml:32-44 │ ────────────────► │ MainActivity : TauriActivity│
│ (singleTask, LAUNCHER)    │                    │ MainActivity.kt:11        │
└───────────────────────────┘                    └──────────────────────────┘
        │                                                  │
        │ onCreate (15) enableEdgeToEdge()                │ onWebViewCreate (19-22)
        ▼                                                  ▼
┌───────────────────────┐                  ┌──────────────────────────────┐
│ Wry/Chromium WebView   │ ◄── appWebView ─ │ resumeTimers (onStart :30)   │
│ (IMPORTANT 常驻, 不降权)│                  │ pauseTimers  (onStop  :35)   │
└───────────────────────┘                  │ evaluateJavascript (onTrim :45)│
        ▲                                   └──────────────────────────────┘
        │ window.__onTrimMemory(level)                │ level≥RUNNING_LOW (44)
        └─────────────────────────────────────────────┘
                          ▼
              ┌──────────────────────────┐
              │ frontend src/main.tsx:123│ 释放人脸引擎最大缓存
              └──────────────────────────┘
```

## Data Flow

1. **启动**：系统派发 `LAUNCHER` intent → `MainActivity.onCreate`（MainActivity.kt:14）调 `enableEdgeToEdge()` → 父类创建 WebView 并触发 `onWebViewCreate`（MainActivity.kt:19），`appWebView` 被赋值（MainActivity.kt:21）。
2. **前后台切换（省电）**：Activity 进入完全可见态 → `onStart` → `appWebView?.resumeTimers()`（MainActivity.kt:30）；进入完全不可见态（如切后台、锁屏）→ `onStop` → `appWebView?.pauseTimers()`（MainActivity.kt:35）。`pauseTimers()` 是**应用级全局停表**，本应用单 WebView 单 Activity，全局即本应用；挂 `onStop`/`onStart` 而非 `onPause`，是为了在分屏/悬浮窗可见但不获焦时不误停定时器（MainActivity.kt:24-27 注释）。
3. **内存压力转发**：系统内存紧张回调 `onTrimMemory(level)`（MainActivity.kt:42）。本侧先判断 `level >= TRIM_MEMORY_RUNNING_LOW`（MainActivity.kt:44），低压力级别不值得唤醒后台 WebView 跑一次 JS 注入（与省电目标一致）。达标后 `evaluateJavascript("window.__onTrimMemory($level)", null)`（MainActivity.kt:45-47）直接派发执行，该调用**不受** `pauseTimers` 停表影响。
4. **前端消费**：`window.__onTrimMemory(level)` 在 `android/frontend/src/main.tsx:123-129` 定义。阈值 `level < 10` 直接 return（main.tsx:125）；若无 `window.__faceEngineLoaded` 或存在 `window.__faceSessionActive` 也 return（main.tsx:125，避免拉空 chunk 或误杀会话中的轮询）；否则动态 `import('./face/faceService')` 调 `releaseFaceEngine()` 释放 TFJS 模型权重这一最大常驻缓存（main.tsx:126-128，best-effort 不重试）。

## Connections

- 与 [[android-exit-guard-renderer-policy]] 强相关：`MainActivity.kt:8-10` 注释、`:42-48` 的 `onTrimMemory` 转发、`:33-36` 的 `pauseTimers` 均为该决策在 Kotlin 侧的落地；renderer 降权策略的摘除记录见该文档第 3 节与「摘除记录」。
- 与 `AndroidManifest.xml`（`android/src-tauri/gen/android/app/src/main/AndroidManifest.xml:32-44`）：声明 `MainActivity` 为入口、配置 `configChanges`、`launchMode="singleTask"`。
- 与前端 `main.tsx`（`android/frontend/src/main.tsx:123-129`）：`window.__onTrimMemory` 全局钩子是 `onTrimMemory` 的接收端。
- 与安卓保活前台服务（[[android-keepalive-fgs-architecture]]）：同 Manifest 内 `ForegroundService`（AndroidManifest.xml:47-54）与 `BootReceiver`（AndroidManifest.xml:57-64）共同构成「进程常驻 + UI 可销毁」架构，`MainActivity` 仅负责 UI 侧生命周期省电。
- 与 Rust 侧退出护栏（lib.rs `RunEvent::ExitRequested` + `api.prevent_exit()`）：划掉任务只关 UI，进程随前台服务常驻（详见 [[android-exit-guard-renderer-policy]] 第 1 节）。

## Known Issues

- **renderer 降权策略已摘除的历史教训**（记录在 [[android-exit-guard-renderer-policy]]「摘除记录」）：v2.3.8 真机实测「长时间挂后台 → 回前台整页空白且无法操作」，无崩溃日志；取证指纹为 chromium 杀 renderer 后 wry 0.55.1 无 `onRenderProcessGone` 处理，宿主进程被 `SIGKILL` 静默自杀。因此 `MainActivity.kt:8-10` 显式禁止回添 `setRendererPriorityPolicy` 降权。上游具备 renderer 恢复机制（tauri #15678，未合并）之前不再考虑降优先级省内存。
- `onTrimMemory` 阈值判断在 Kotlin 侧与前端 `main.tsx:124` 双重兜底：Kotlin 侧以 `ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW` 常量（MainActivity.kt:44）拦截低压力级别，前端再以字面量 `level < 10`（main.tsx:124）兜底。两处语义档需保持一致——如需调整阈值，必须同时改 Kotlin 常量与前端字面量，否则会有「Kotlin 放行但前端直接 return」或反之的错位。
- `appWebView` 为可空 `WebView?`，所有调用均用安全调用 `?.`，在 `onWebViewCreate` 未被触发前（理论上不会）为空时无副作用。
