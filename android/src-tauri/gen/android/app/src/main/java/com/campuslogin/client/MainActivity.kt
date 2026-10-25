package com.campuslogin.client

import android.content.ComponentCallbacks2
import android.os.Bundle
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge

// 有意不设 renderer 优先级策略（wry 默认 IMPORTANT 常驻）：降权放行会让后台渲染进程被
// 系统回收，而 wry 0.55.1 无 onRenderProcessGone 处理，chromium 随即 SIGKILL 静默杀宿主
// 进程（回前台白屏冻结），见 .codewiki/decisions/android-exit-guard-renderer-policy.md。
class MainActivity : TauriActivity() {
  private var appWebView: WebView? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    appWebView = webView
  }

  // WebView.onPause()（wry 在 Activity onPause 调）不暂停 JS 定时器（官方文档明确），
  // 后台时页面全部 setInterval 仍按 Chromium 节流拍唤醒，是后台空耗电主因。
  // pauseTimers 是应用级全局停表——本应用单 WebView 单 Activity，全局即本应用。
  // 挂 onStop/onStart（完全不可见）而非 onPause：分屏/悬浮窗下可见但不获焦时不误停。
  override fun onStart() {
    super.onStart()
    appWebView?.resumeTimers()
  }

  override fun onStop() {
    super.onStop()
    appWebView?.pauseTimers()
  }

  // 系统内存压力转发给前端全局钩子（main.tsx 的 __onTrimMemory）：释放人脸引擎等
  // 大缓存。evaluateJavascript 直接派发执行，不受 pauseTimers 停表影响。
  // 阈值判断在本侧完成（前端同名常量兜底）：低压力级别不值得唤醒后台 WebView
  // 跑一次 JS 注入——与后台省电目标一致。
  override fun onTrimMemory(level: Int) {
    super.onTrimMemory(level)
    if (level < ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW) return
    appWebView?.evaluateJavascript(
      "window.__onTrimMemory && window.__onTrimMemory($level)", null
    )
  }
}
