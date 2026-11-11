package com.campuslogin.plugin.monitorservice

/**
 * UI 进程的 core 事件桥:Rust 侧 `core::ui_bridge` 的 JNI 目标。
 * 广播在 [CoreClient.registerEventReceiver] 里接收后转给 WebView。
 */
object CoreEvents {
    init {
        System.loadLibrary("campus_login_android_lib")
    }

    @JvmStatic
    external fun nativeEmitToUi(event: String, payload: String)
}
