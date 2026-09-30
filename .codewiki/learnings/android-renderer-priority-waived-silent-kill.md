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

`MainActivity.onWebViewCreate` 调 `setRendererPriorityPolicy(RENDERER_PRIORITY_BOUND, waivedWhenNotVisible=true)`（v2.3.8 的"后台内存优化"）：waived=true 时不可见态 renderer importance 与 WAIVED 同档（AOSP `AwContents.java:4106-4123`），长后台被系统/OEM 回收；wry 0.55.1 无 `onRenderProcessGone` 处理（`RustWebViewClient.kt` 仅 5 个覆写），chromium `aw_browser_terminator.cc:127` 随即 `kill(getpid(), SIGKILL)` 静默杀掉宿主进程；START_STICKY 前台服务只拉起 Service 不重建 WebView，回前台即白屏冻结（tauri #15671 一族）。

## 解决

整体摘除策略段，回退 wry 默认（renderer 不分可见性 IMPORTANT 常驻）。取证指纹：`adb logcat -d | grep -i "killing application"` 出现 "Render process … wasn't handed by all associated webviews, killing application." 即此路径，修复后不应再出现。

## 教训

**不要动 WebView renderer 优先级，除非配套 Termination Handling**（官方 Managing WebView 文档 Warning；wry/Capacitor 上游零调用，Cordova 只做 onPause+pauseTimers）。`reload()` 救不活死掉的 WebView，必须销毁重建。优化类改动的真机验证要覆盖它引入的风险场景（本例：长后台 → 回前台），只看内存数字等于没验。

## Connections

[[android-exit-guard-renderer-policy]]、[[android-white-screen-missing-init-fields]]
