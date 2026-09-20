package com.campuslogin.client

import android.os.Build
import android.os.Bundle
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
      // 后台留存优化：不可见时渲染进程降为 bound 优先级，允许系统在内存压力下
      // 回收（默认策略不分可见性一律 IMPORTANT，不设置则享受不到；渲染进程是
      // WebView 内存大头，独立 sandboxed_processX 进程）。
      // 选 BOUND 温和档而非 WAIVED：被杀概率显著更低。
      // 风险语义（决策文档 decisions/android-exit-guard-renderer-policy）：
      // renderer 自身 crash → 宿主崩溃，与是否设置本策略无关（现状即如此）；
      // 极端内存压力下 renderer 被系统杀 → 宿主连带被杀 → START_STICKY 前台
      // 服务自动重启（冷启动自愈）。真机验证异常则整体摘除本段（回退三件套）
      webView.setRendererPriorityPolicy(
        WebView.RENDERER_PRIORITY_BOUND,
        /* waivedWhenNotVisible = */ true
      )
    }
  }
}
