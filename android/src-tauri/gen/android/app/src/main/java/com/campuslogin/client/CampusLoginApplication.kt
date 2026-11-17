package com.campuslogin.client

import android.app.Application
import android.content.Intent
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.util.Log
import moe.shizuku.api.BinderContainer
import rikka.shizuku.Shizuku
import rikka.shizuku.ShizukuProvider

/**
 * 应用级初始化：让 Shizuku 的 binder 在**每个进程**都可用。
 *
 * 本项目的网络巡检跑在 `:core` 独立进程（CoreService），而 ShizukuProvider 只在主进程
 * 绑定 binder 并向其它进程广播一次。`:core` 与主进程几乎同时启动（真机日志仅差 0.9s），
 * 注册接收器时那次广播已经过去了，于是 `:core` 里 `pingBinder()` 恒为 false——后台守则的
 * `cmd wifi` 与 SSID 读取全部静默失效。
 *
 * 修法：主进程拿到 binder 后延迟 3 秒按 Shizuku 同一协议给本包补播一次，`:core` 那时
 * 已注册好接收器。只依赖 Shizuku 的广播 action 与 extra key（多年未变），不改其实现。
 */
class CampusLoginApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        ShizukuProvider.enableMultiProcessSupport(true)
        if (!isMainProcess()) return
        Shizuku.addBinderReceivedListenerSticky { forwardBinderToSecondaryProcesses() }
    }

    private fun isMainProcess(): Boolean {
        if (android.os.Build.VERSION.SDK_INT < android.os.Build.VERSION_CODES.P) return true
        return Application.getProcessName() == packageName
    }

    private fun forwardBinderToSecondaryProcesses() {
        Handler(Looper.getMainLooper()).postDelayed({
            try {
                val uri = Uri.parse("content://" + packageName + ".shizuku")
                val bundle = contentResolver.call(uri, "getBinder", null, null)
                if (bundle == null) {
                    Log.i("CampusApp", "补播 binder：provider 未返回")
                    return@postDelayed
                }
                bundle.classLoader = BinderContainer::class.java.classLoader
                @Suppress("DEPRECATION")
                val container = bundle.getParcelable<BinderContainer>("binder")
                if (container == null) {
                    Log.i("CampusApp", "补播 binder：bundle 无 binder")
                    return@postDelayed
                }
                sendBroadcast(
                    Intent("moe.shizuku.api.action.BINDER_RECEIVED")
                        .setPackage(packageName)
                        .putExtra("moe.shizuku.privileged.api.intent.extra.BINDER", container)
                )
                Log.i("CampusApp", "已向本包其它进程补播 Shizuku binder")
            } catch (e: Throwable) {
                Log.w("CampusApp", "补播 binder 失败: " + e.javaClass.simpleName + ":" + e.message)
            }
        }, 3000)
    }
}
