package com.campuslogin.client

import android.os.Bundle
import androidx.activity.enableEdgeToEdge

// 有意不设 renderer 优先级策略（wry 默认 IMPORTANT 常驻）：降权放行会让后台渲染进程被
// 系统回收，而 wry 0.55.1 无 onRenderProcessGone 处理，chromium 随即 SIGKILL 静默杀宿主
// 进程（回前台白屏冻结），见 .codewiki/decisions/android-exit-guard-renderer-policy.md。
class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }
}
